//! Staircase placement and its iteration (`docs/spec/layout.md` §3.2 keep-out, §5.1, §5.2,
//! §5.4; FR-LAYOUT-01; SR-40).
//!
//! Placement is independent of routing: [`iterate`] takes the router as a callback that,
//! for a placement, returns the tracks each channel needs. M4.3 (global routing) and M4.4
//! (track assignment) plug into it.

use idef0_core::diag::{Diagnostic, L_LAYOUT_OVERFLOW, SubjectRef};
use idef0_core::id::{NodeId, Side};
use idef0_core::model::{ArrowShape, Diagram, Endpoint, Model};

use crate::metrics::{LATTICE_X, LATTICE_Y, Metrics};
use crate::ports::Rect;
use crate::size::{PortCounts, box_size};
use crate::text::TextMetric;

/// Placement iterations before `L-LAYOUT-OVERFLOW` (§5.2).
pub const MAX_ITERATIONS: u8 = 3;

fn ceil_to(v: i32, k: i32) -> i32 {
    (v + k - 1).div_euclid(k) * k
}

fn count(n: u32) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX / 16)
}

/// What placement needs to know about one box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxPlan {
    /// Width in gu (§4.1).
    pub w: i32,
    /// Height in gu (§4.1).
    pub h: i32,
    /// Whether the theme draws a drop shadow: the box has a child diagram (FR-STYLE-06).
    pub shadow: bool,
    /// The DRE text width in gu, when the box shows a DRE beneath its lower-right corner
    /// (§4.3).
    pub dre_w: Option<i32>,
}

impl BoxPlan {
    /// The keep-out footprint's width and height (§3.2): the box, plus the shadow band, plus
    /// the DRE text block, which sits below the box (or the shadow), right-aligned.
    pub fn footprint(&self, m: &Metrics) -> (i32, i32) {
        let sx = if self.shadow { m.shadow_x } else { 0 };
        let sy = if self.shadow { m.shadow_y } else { 0 };
        let dre = if self.dre_w.is_some() { m.line_h } else { 0 };
        (self.w + sx, self.h + sy + dre)
    }

    /// The keep-out rectangle of the box placed at `(x, y)`. A DRE wider than the box reaches
    /// left of it.
    pub fn keep_out(&self, x: i32, y: i32, m: &Metrics) -> Rect {
        let (fw, fh) = self.footprint(m);
        let left = self.dre_w.map_or(x, |d| x.min(x + fw - d));
        Rect {
            x: left,
            y,
            w: x + fw - left,
            h: fh,
        }
    }
}

/// Tracks each channel needs: `v[c]` for vertical channel `V_c`, `h[r]` for horizontal
/// channel `H_r`, each of length `n + 1` (§6.1). Channel 0 and channel n are the margins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracks {
    /// Vertical channels `V_0 … V_n`.
    pub v: Vec<u32>,
    /// Horizontal channels `H_0 … H_n`.
    pub h: Vec<u32>,
}

impl Tracks {
    /// No tracks anywhere: the iteration-1 assumption (`d = 1` in inner channels, §5.2).
    pub fn none(n: usize) -> Self {
        Self {
            v: vec![0; n + 1],
            h: vec![0; n + 1],
        }
    }
}

/// The label allowance of each frame edge in gu (§5.1): the widest input or output label, or
/// the control or mechanism cascade height, plus squiggle and ICOM code width. Labels arrive
/// with roadmap M5.4; until then this is zero and `margin` decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EdgeAllowance {
    /// Left edge (inputs).
    pub left: i32,
    /// Top edge (controls).
    pub top: i32,
    /// Right edge (outputs).
    pub right: i32,
    /// Bottom edge (mechanisms).
    pub bottom: i32,
}

/// Channel widths and heights in gu: `x[c]` is the width of `V_c`, `y[r]` the height of
/// `H_r`. `x[0]`, `x[n]`, `y[0]`, `y[n]` are the margins `m_L`, `m_R`, `m_T`, `m_B`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gaps {
    /// Widths of `V_0 … V_n`.
    pub x: Vec<i32>,
    /// Heights of `H_0 … H_n`.
    pub y: Vec<i32>,
}

impl Gaps {
    /// The gaps `tracks` need (§5.1): inner channels `2·clear + max(1, d)·pitch`; margins the
    /// largest of `margin`, `2·clear + d·pitch` and the edge's label allowance. Every value is
    /// rounded up to the lattice so all coordinates stay on it.
    pub fn required(tracks: &Tracks, allowance: EdgeAllowance, m: &Metrics) -> Self {
        let n = tracks.v.len().saturating_sub(1);
        let axis = |d: &[u32], pitch: i32, lattice: i32, first: i32, last: i32| -> Vec<i32> {
            d.iter()
                .enumerate()
                .map(|(i, &t)| {
                    let tracks = count(t);
                    let g = if i == 0 || i == n {
                        let edge = if i == 0 { first } else { last };
                        m.margin.max(2 * m.clear + tracks * pitch).max(edge)
                    } else {
                        2 * m.clear + tracks.max(1) * pitch
                    };
                    ceil_to(g, lattice)
                })
                .collect()
        };
        Self {
            x: axis(
                &tracks.v,
                m.pitch_x,
                LATTICE_X,
                allowance.left,
                allowance.right,
            ),
            y: axis(
                &tracks.h,
                m.pitch_y,
                LATTICE_Y,
                allowance.top,
                allowance.bottom,
            ),
        }
    }

    /// The elementwise maximum: gaps only grow (§5.2).
    #[must_use]
    pub fn max(&self, other: &Self) -> Self {
        let pick = |a: &[i32], b: &[i32]| a.iter().zip(b).map(|(x, y)| *x.max(y)).collect();
        Self {
            x: pick(&self.x, &other.x),
            y: pick(&self.y, &other.y),
        }
    }
}

/// Boxes on the strict staircase, with the channel geometry between them (§5.1, §6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// Box rectangles in box-number order.
    pub boxes: Vec<Rect>,
    /// Keep-out rectangles, aligned with `boxes`.
    pub keep_out: Vec<Rect>,
    /// The gaps used.
    pub gaps: Gaps,
    /// Frame width and height.
    pub frame: (i32, i32),
}

impl Placement {
    /// `V_c` as `[x0, x1)`: between the keep-out of box `c` (or the frame) and box `c + 1`.
    pub fn v_channel(&self, c: usize) -> (i32, i32) {
        let x0 = if c == 0 {
            0
        } else {
            self.keep_out[c - 1].x + self.keep_out[c - 1].w
        };
        (x0, x0 + self.gaps.x[c])
    }

    /// `H_r` as `[y0, y1)`: below the keep-out of box `r` (or the frame top).
    pub fn h_channel(&self, r: usize) -> (i32, i32) {
        let y0 = if r == 0 {
            0
        } else {
            self.keep_out[r - 1].y + self.keep_out[r - 1].h
        };
        (y0, y0 + self.gaps.y[r])
    }
}

/// Place `plans` on the staircase with `gaps` (§5.1): `x_1 = m_L`, `x_{i+1} = x_i + w_i +
/// gx_i`, and likewise for y, where `w_i`, `h_i` are the keep-out footprint, because every
/// clearance is measured from the keep-out region (§3.2).
pub fn place(plans: &[BoxPlan], gaps: &Gaps, m: &Metrics) -> Placement {
    let n = plans.len();
    let (mut x, mut y) = (gaps.x[0], gaps.y[0]);
    let mut boxes = Vec::with_capacity(n);
    let mut keep_out = Vec::with_capacity(n);
    for (i, p) in plans.iter().enumerate() {
        boxes.push(Rect {
            x,
            y,
            w: p.w,
            h: p.h,
        });
        keep_out.push(p.keep_out(x, y, m));
        let (fw, fh) = p.footprint(m);
        x += fw + gaps.x[i + 1];
        y += fh + gaps.y[i + 1];
    }
    let frame = if n == 0 {
        (gaps.x[0] + gaps.x[n], gaps.y[0] + gaps.y[n])
    } else {
        (x, y)
    };
    Placement {
        boxes,
        keep_out,
        gaps: gaps.clone(),
        frame,
    }
}

/// The result of the placement iteration (§5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The final placement.
    pub placement: Placement,
    /// Gaps after each routed iteration, for inspection: they never shrink.
    pub history: Vec<Gaps>,
    /// Iterations that ran the router (1 to 3).
    pub iterations: u8,
    /// Whether the cap was reached with a channel still short, so the required gaps were
    /// applied once more without re-routing (`L-LAYOUT-OVERFLOW`, `reason=iterations`).
    pub overflow: bool,
}

impl Placed {
    /// The `L-LAYOUT-OVERFLOW` finding for diagram `node`, when the iteration overflowed.
    pub fn overflow_diagnostic(&self, node: &NodeId) -> Option<Diagnostic> {
        self.overflow.then(|| {
            Diagnostic::lint_warning(
                L_LAYOUT_OVERFLOW,
                SubjectRef::Diagram(node.clone()),
                format!("reason=iterations: placement of {node} did not settle in {MAX_ITERATIONS} iterations"),
            )
        })
    }
}

/// The §5.2 iteration. `route(placement, iteration)` returns the tracks each channel needs at
/// that placement. The router owns the "frozen global routing" and "never more tracks" rules;
/// this driver guarantees that gaps only grow, that at most [`MAX_ITERATIONS`] iterations
/// route, and that a shortfall left at the cap is applied once more and reported.
pub fn iterate(
    plans: &[BoxPlan],
    allowance: EdgeAllowance,
    m: &Metrics,
    mut route: impl FnMut(&Placement, u8) -> Tracks,
) -> Placed {
    let n = plans.len();
    let mut gaps = Gaps::required(&Tracks::none(n), allowance, m);
    let mut placement = place(plans, &gaps, m);
    let mut history = Vec::new();
    let mut iteration = 0;
    loop {
        iteration += 1;
        let tracks = route(&placement, iteration);
        let needed = gaps.max(&Gaps::required(&tracks, allowance, m));
        history.push(gaps.clone());
        if needed == gaps {
            return Placed {
                placement,
                history,
                iterations: iteration,
                overflow: false,
            };
        }
        gaps = needed;
        placement = place(plans, &gaps, m);
        if iteration == MAX_ITERATIONS {
            history.push(gaps);
            return Placed {
                placement,
                history,
                iterations: iteration,
                overflow: true,
            };
        }
    }
}

/// Port counts per box of `diagram`: the highest rank used on each side, and whether the box
/// has a call arrow. A bundle's shared trunk end is one port.
pub fn port_counts(diagram: &Diagram) -> Vec<PortCounts> {
    let mut out: Vec<PortCounts> = diagram
        .boxes
        .iter()
        .map(|b| PortCounts {
            call: b.call.is_some(),
            ..PortCounts::default()
        })
        .collect();
    let mut note = |e: &Endpoint| {
        if let Endpoint::Box { key, side, rank } = e
            && let Some(i) = diagram.boxes.iter().position(|b| &b.key == key)
        {
            let c = &mut out[i];
            let slot = match side {
                Side::I => &mut c.i,
                Side::C => &mut c.c,
                Side::O => &mut c.o,
                Side::M => &mut c.m,
            };
            *slot = (*slot).max(*rank);
        }
    };
    for a in &diagram.arrows {
        match &a.shape {
            ArrowShape::Plain { src, dst } => {
                note(src);
                note(dst);
            }
            ArrowShape::Fork { src, fork } => {
                note(src);
                fork.iter().for_each(|b| note(&b.dst));
            }
            ArrowShape::Join { join, dst } => {
                join.iter().for_each(|l| note(&l.src));
                note(dst);
            }
        }
    }
    out
}

/// The placement inputs of diagram `node` of `model`: §4.1 sizes, and the shadow and DRE of
/// every box that has a child diagram (FR-STYLE-06; a caller box has neither). `shadows` is
/// the theme's choice (§13.1: off under `fips183`).
pub fn box_plans(
    model: &Model,
    node: &NodeId,
    shadows: bool,
    metric: TextMetric,
    m: &Metrics,
) -> Vec<BoxPlan> {
    let Some(diagram) = model.diagrams.get(node) else {
        return Vec::new();
    };
    let counts = port_counts(diagram);
    diagram
        .boxes
        .iter()
        .zip(counts)
        .enumerate()
        .map(|(i, (b, c))| {
            let size = box_size(b.name.as_str(), c, metric, m);
            let child = match node {
                NodeId::Node(_) => u8::try_from(i + 1).ok().and_then(|k| node.child(k)),
                NodeId::Context { level: 0 } => Some(NodeId::a0()),
                NodeId::Context { .. } => None,
            };
            // the DRE is the child's node number (§4.4 of the metamodel; v1 default)
            let dre_w = child
                .filter(|ch| model.dre(ch).is_some())
                .map(|ch| metric.width_gu(&ch.to_string(), m));
            BoxPlan {
                w: size.w,
                h: size.h,
                shadow: shadows && dre_w.is_some(),
                dre_w,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn plan(w: i32, h: i32) -> BoxPlan {
        BoxPlan {
            w,
            h,
            shadow: false,
            dre_w: None,
        }
    }

    // §5.1: minimal gaps are 2·clear + pitch inside, margin at the edges.
    #[test]
    fn minimal_gaps() {
        let m = Metrics::default();
        let g = Gaps::required(&Tracks::none(3), EdgeAllowance::default(), &m);
        assert_eq!(g.x, [24, 14, 14, 24]);
        assert_eq!(g.y, [24, 16, 16, 24]);
        let wide = Gaps::required(
            &Tracks {
                v: vec![5, 2, 0, 0],
                h: vec![0, 3, 0, 4],
            },
            EdgeAllowance {
                left: 31,
                ..EdgeAllowance::default()
            },
            &m,
        );
        assert_eq!(
            wide.x,
            [38, 20, 14, 24],
            "margin V_0: max(24, 8 + 30, 31) = 38"
        );
        assert_eq!(wide.y, [24, 32, 16, 40]);
        // a label allowance off the lattice is rounded up onto it
        let odd = Gaps::required(
            &Tracks::none(1),
            EdgeAllowance {
                left: 41,
                top: 29,
                right: 0,
                bottom: 0,
            },
            &m,
        );
        assert_eq!((odd.x[0], odd.y[0]), (42, 32));
    }

    // §3.2: the keep-out takes the shadow band and the DRE block below the box.
    #[test]
    fn keep_out_footprint() {
        let m = Metrics::default();
        let p = BoxPlan {
            w: 40,
            h: 24,
            shadow: true,
            dre_w: Some(6),
        };
        assert_eq!(p.footprint(&m), (42, 32));
        assert_eq!(
            p.keep_out(10, 20, &m),
            Rect {
                x: 10,
                y: 20,
                w: 42,
                h: 32
            }
        );
        let wide_dre = BoxPlan {
            w: 8,
            h: 24,
            shadow: false,
            dre_w: Some(12),
        };
        assert_eq!(
            wide_dre.keep_out(10, 0, &m).x,
            6,
            "a DRE wider than the box reaches left"
        );
    }

    // §5.2: the router is called once when nothing needs more space.
    #[test]
    fn settles_in_one_iteration_without_tracks() {
        let m = Metrics::default();
        let plans = [plan(40, 24), plan(40, 24), plan(40, 24)];
        let mut calls = 0;
        let r = iterate(&plans, EdgeAllowance::default(), &m, |_, _| {
            calls += 1;
            Tracks::none(3)
        });
        assert_eq!((calls, r.iterations, r.overflow), (1, 1, false));
    }

    // §5.2 step 4: still short after 3 iterations: apply once more, report L-LAYOUT-OVERFLOW.
    #[test]
    fn overflow_after_three_iterations() {
        let m = Metrics::default();
        let plans = [plan(40, 24), plan(40, 24)];
        // each iteration needs one more track in V_1 than the last: 2, 3, 4
        let r = iterate(&plans, EdgeAllowance::default(), &m, |_, k| Tracks {
            v: vec![0, u32::from(k) + 1, 0],
            h: vec![0; 3],
        });
        assert_eq!((r.iterations, r.overflow), (3, true));
        assert_eq!(
            r.placement.gaps.x[1],
            2 * 4 + 4 * 6,
            "the last required gap is applied"
        );
        let d = r.overflow_diagnostic(&NodeId::a0()).unwrap();
        assert_eq!(d.code.as_str(), "L-LAYOUT-OVERFLOW");
        assert_eq!(d.subject.to_string(), "diagram A0");
    }

    // Placement inputs from a model: the dogfood A0 (roadmap M4.2 fixture).
    #[test]
    fn dogfood_plans() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dogfood/idef0-rs.idef0");
        let model = idef0_core::dsl::parse(&std::fs::read_to_string(path).unwrap())
            .model
            .unwrap();
        let m = Metrics::default();
        let plans = box_plans(&model, &NodeId::a0(), true, TextMetric::Mono, &m);
        assert_eq!(plans.len(), 5);
        assert!(
            plans.iter().all(|p| !p.shadow && p.dre_w.is_none()),
            "no A0 box is decomposed"
        );
        let top = box_plans(&model, &NodeId::context(0), true, TextMetric::Mono, &m);
        assert_eq!(top.len(), 1);
        assert!(
            top[0].shadow && top[0].dre_w == Some(4),
            "the A0 top box is detailed by A0: DRE `A0`"
        );
        let counts = port_counts(&model.diagrams[&NodeId::a0()]);
        assert_eq!(
            (counts[2].i, counts[2].c, counts[2].o, counts[2].m),
            (2, 2, 2, 1),
            "box 3 Validate Model Rules"
        );
        let r = iterate(&plans, EdgeAllowance::default(), &m, |_, _| Tracks::none(5));
        assert_staircase(&r.placement, &plans, &m);
    }

    fn assert_staircase(p: &Placement, plans: &[BoxPlan], m: &Metrics) {
        for (i, w) in p.boxes.windows(2).enumerate() {
            let (fw, fh) = plans[i].footprint(m);
            assert!(
                w[1].x >= w[0].x + fw + 2 * m.clear + m.pitch_x,
                "{i}: {w:?}"
            );
            assert!(
                w[1].y >= w[0].y + fh + 2 * m.clear + m.pitch_y,
                "{i}: {w:?}"
            );
        }
        for r in &p.boxes {
            assert!(
                r.x % LATTICE_X == 0 && r.y % LATTICE_Y == 0,
                "{r:?} off the lattice"
            );
            assert!(
                r.x >= m.margin
                    && r.y >= m.margin
                    && r.x + r.w <= p.frame.0
                    && r.y + r.h <= p.frame.1
            );
        }
    }

    fn plans_strategy() -> impl Strategy<Value = Vec<BoxPlan>> {
        proptest::collection::vec(
            (
                20i32..60,
                6i32..20,
                any::<bool>(),
                proptest::option::of(1i32..8),
            )
                .prop_map(|(w, h, shadow, dre)| BoxPlan {
                    w: w * 2,
                    h: h * 4,
                    shadow,
                    dre_w: dre.map(|d| d * 2),
                }),
            1..8,
        )
    }

    proptest! {
        // Roadmap M4.2: a strict staircase (FR-LAYOUT-01), on the lattice, for any boxes and
        // any track demands.
        #[test]
        fn strict_staircase(plans in plans_strategy(), demand in proptest::collection::vec(0u32..6, 32)) {
            let m = Metrics::default();
            let n = plans.len();
            let r = iterate(&plans, EdgeAllowance::default(), &m, |_, k| {
                let off = usize::from(k) * (n + 1);
                Tracks { v: demand[off % 16..][..=n].to_vec(), h: demand[16 + off % 8..][..=n].to_vec() }
            });
            assert_staircase(&r.placement, &plans, &m);
            for c in 0..=n {
                let (a, b) = r.placement.v_channel(c);
                prop_assert!(b - a == r.placement.gaps.x[c] && a % 2 == 0 && b % 2 == 0);
            }
        }

        // Roadmap M4.2, §5.2: gaps never shrink across iterations, at most 3 iterations route,
        // and overflow is reported exactly when the cap is reached short of space.
        #[test]
        fn gaps_never_shrink(plans in plans_strategy(), demands in proptest::collection::vec(proptest::collection::vec(0u32..6, 9), 4)) {
            let m = Metrics::default();
            let n = plans.len();
            let r = iterate(&plans, EdgeAllowance::default(), &m, |_, k| {
                let d = &demands[usize::from(k)];
                Tracks { v: d[..=n].to_vec(), h: d[..=n].iter().rev().copied().collect() }
            });
            prop_assert!(r.iterations <= MAX_ITERATIONS);
            for w in r.history.windows(2) {
                prop_assert!(w[0].x.iter().zip(&w[1].x).all(|(a, b)| a <= b));
                prop_assert!(w[0].y.iter().zip(&w[1].y).all(|(a, b)| a <= b));
            }
            prop_assert_eq!(r.overflow, r.iterations == MAX_ITERATIONS && r.history.len() > usize::from(r.iterations));
        }

        // §5.2 lemma: growing gaps is monotone and expansive on every coordinate, so channel
        // intervals that were at least `clear` apart stay so and orders are preserved.
        #[test]
        fn gap_widening_lemma(plans in plans_strategy(), extra in proptest::collection::vec(0u32..4, 18)) {
            let m = Metrics::default();
            let n = plans.len();
            let before = Gaps::required(&Tracks::none(n), EdgeAllowance::default(), &m);
            let grown = before.max(&Gaps::required(&Tracks { v: extra[..=n].to_vec(), h: extra[9..=9 + n].to_vec() }, EdgeAllowance::default(), &m));
            let (a, b) = (place(&plans, &before, &m), place(&plans, &grown, &m));
            let coords = |p: &Placement| -> (Vec<i32>, Vec<i32>) {
                let mut xs = Vec::new();
                let mut ys = Vec::new();
                for c in 0..=n {
                    let (x0, x1) = p.v_channel(c);
                    let (y0, y1) = p.h_channel(c);
                    xs.extend([x0, x1]);
                    ys.extend([y0, y1]);
                }
                for r in &p.boxes {
                    xs.extend([r.x, r.x + r.w]);
                    ys.extend([r.y, r.y + r.h]);
                }
                (xs, ys)
            };
            let ((xa, ya), (xb, yb)) = (coords(&a), coords(&b));
            for (before, after) in [(&xa, &xb), (&ya, &yb)] {
                for i in 0..before.len() {
                    for j in 0..before.len() {
                        if before[i] <= before[j] {
                            prop_assert!(after[j] - after[i] >= before[j] - before[i], "not expansive");
                        }
                    }
                }
            }
        }
    }
}
