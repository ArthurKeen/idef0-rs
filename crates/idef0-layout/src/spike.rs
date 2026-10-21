//! **M4.0 layout spike** (roadmap M4.0; layout §4, §5.1, §6, §12.2, §16): a prototype of box
//! sizing, staircase placement, global routing and the Auto-Arrange box-order search, built
//! to measure cost, not to ship. M4.1–M4.3 and M4.7 replace it with the real engine.
//!
//! Simplifications, recorded in ADR 0002:
//! - plain arrows only (each bundle branch or leaf is routed as its own edge);
//! - channel runs use the channel's center line, because track assignment (§7, M4.4) is not
//!   prototyped; crossings are counted between those lines;
//! - title wrap uses Mono metrics only (§3.3); no labels, fillets, bridges or form;
//! - boundary dog-legs ignore the frame-order rule (FR-LAYOUT-22).
//!
//! All arithmetic is integer grid units (gu); no floats, no hash maps (D6).

#![allow(
    clippy::many_single_char_names,
    reason = "geometry uses the spec's own names: x, y, w, h, n, i, j, r, c (layout §5, §6)"
)]

use std::collections::BTreeMap;

use idef0_core::id::{NodeId, PortCode, Side};
use idef0_core::model::{ArrowShape, Diagram, Endpoint};
use idef0_core::text::BoxKey;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// layout §3.2 metrics, in gu.
mod m {
    pub(crate) const PITCH_X: i32 = 6;
    pub(crate) const PITCH_Y: i32 = 8;
    pub(crate) const PORT_PITCH: i32 = 8;
    pub(crate) const CLEAR: i32 = 4;
    pub(crate) const GLYPH_W: i32 = 2;
    pub(crate) const LINE_H: i32 = 4;
    pub(crate) const BOX_MIN_W: i32 = 40;
    pub(crate) const BOX_MIN_H: i32 = 24;
    pub(crate) const W_MAX: i32 = 80;
    pub(crate) const PAD: i32 = 2;
    pub(crate) const MARGIN: i32 = 24;
    pub(crate) const CORNER_LABEL_H: i32 = 4;
    pub(crate) const R_FILLET_GU: i32 = 2;
}

fn ceil_to(v: i32, k: i32) -> i32 {
    (v + k - 1).div_euclid(k) * k
}

fn count(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// Mono text width (§3.3): grapheme clusters × `unicode-width` × `glyph_w`.
fn text_w(s: &str) -> i32 {
    s.graphemes(true).map(|g| count(g.width())).sum::<i32>() * m::GLYPH_W
}

/// Greedy wrap at `limit` gu; break at spaces and after `/` and `-` (§4.2).
fn wrap(title: &str, limit: i32) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in title.chars() {
        if c == ' ' {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
            if c == '/' || c == '-' {
                words.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    let mut lines: Vec<String> = Vec::new();
    for w in words {
        match lines.last_mut() {
            Some(line) if text_w(&format!("{line} {w}")) <= limit => {
                let joiner = if line.ends_with('/') || line.ends_with('-') {
                    ""
                } else {
                    " "
                };
                line.push_str(joiner);
                line.push_str(&w);
            }
            _ => lines.push(w),
        }
    }
    lines
}

/// A placed box, in gu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// Port counts per side of one box.
#[derive(Debug, Clone, Copy, Default)]
struct Ports {
    i: i32,
    c: i32,
    o: i32,
    m: i32,
}

impl Ports {
    fn on(&self, side: Side) -> i32 {
        match side {
            Side::I => self.i,
            Side::C => self.c,
            Side::O => self.o,
            Side::M => self.m,
        }
    }
}

/// One routable edge: a plain arrow, or one branch or leaf of a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edge {
    src: Endpoint,
    dst: Endpoint,
}

fn edges(d: &Diagram) -> Vec<Edge> {
    let mut out = Vec::new();
    for a in &d.arrows {
        match &a.shape {
            ArrowShape::Plain { src, dst } => out.push(Edge {
                src: src.clone(),
                dst: dst.clone(),
            }),
            ArrowShape::Fork { src, fork } => {
                out.extend(fork.iter().map(|b| Edge {
                    src: src.clone(),
                    dst: b.dst.clone(),
                }));
            }
            ArrowShape::Join { join, dst } => {
                out.extend(join.iter().map(|l| Edge {
                    src: l.src.clone(),
                    dst: dst.clone(),
                }));
            }
        }
    }
    out
}

/// Box sizes, positions, channels and frame (layout §4.1, §4.2, §5.1 iteration 1).
#[derive(Debug, Clone)]
pub struct Placement {
    /// Boxes in box-number order.
    pub boxes: Vec<Rect>,
    /// Frame width and height.
    pub frame: (i32, i32),
    ports: Vec<Ports>,
    index: BTreeMap<BoxKey, usize>,
}

impl Placement {
    /// Center x of vertical channel `V_c` (0 = left margin, n = right margin).
    fn v(&self, c: usize) -> i32 {
        let n = self.boxes.len();
        let left = if c == 0 {
            0
        } else {
            self.boxes[c - 1].x + self.boxes[c - 1].w
        };
        let right = if c >= n {
            self.frame.0
        } else {
            self.boxes[c].x
        };
        i32::midpoint(left, right)
    }

    /// Center y of horizontal channel `H_r` (0 = top margin, n = bottom margin).
    fn h(&self, r: usize) -> i32 {
        let n = self.boxes.len();
        let top = if r == 0 {
            0
        } else {
            self.boxes[r - 1].y + self.boxes[r - 1].h
        };
        let bottom = if r >= n {
            self.frame.1
        } else {
            self.boxes[r].y
        };
        i32::midpoint(top, bottom)
    }

    /// The lattice point of rank `k` on `side` of box `b` (§4.4).
    fn port(&self, b: usize, side: Side, k: u8) -> (i32, i32) {
        let r = self.boxes[b];
        let n = self.ports[b].on(side).max(1);
        let k = i32::from(k);
        match side {
            Side::I => (r.x, r.y + snap(r.h * k / (n + 1), 4)),
            Side::O => (r.x + r.w, r.y + snap(r.h * k / (n + 1), 4)),
            Side::C => (r.x + snap(r.w * k / (n + 1), 2), r.y),
            Side::M => (r.x + snap(r.w * k / (n + 1), 2), r.y + r.h),
        }
    }
}

fn snap(v: i32, g: i32) -> i32 {
    v.div_euclid(g) * g
}

/// Size and place the boxes of `d` on the staircase with minimal gaps (§5.2 iteration 1).
pub fn place(d: &Diagram) -> Placement {
    let index: BTreeMap<BoxKey, usize> = d
        .boxes
        .iter()
        .enumerate()
        .map(|(i, b)| (b.key.clone(), i))
        .collect();
    let mut ports = vec![Ports::default(); d.boxes.len()];
    for e in edges(d) {
        for end in [&e.src, &e.dst] {
            if let Endpoint::Box { key, side, rank } = end
                && let Some(&b) = index.get(key)
            {
                let p = &mut ports[b];
                let slot = match side {
                    Side::I => &mut p.i,
                    Side::C => &mut p.c,
                    Side::O => &mut p.o,
                    Side::M => &mut p.m,
                };
                *slot = (*slot).max(i32::from(*rank));
            }
        }
    }
    let mut boxes = Vec::with_capacity(d.boxes.len());
    let (mut x, mut y) = (m::MARGIN, m::MARGIN);
    for (b, fb) in d.boxes.iter().enumerate() {
        let p = ports[b];
        let port_w = (p.c.max(p.m) + 1) * m::PORT_PITCH;
        let port_h = (p.i.max(p.o) + 1) * m::PORT_PITCH;
        let floor = m::BOX_MIN_W.max(port_w);
        let mut w = ceil_to(floor, 2);
        let mut lines = wrap(fb.name.as_str(), w - 2 * m::PAD);
        while lines.len() > 3 && w < m::W_MAX {
            w += 2;
            lines = wrap(fb.name.as_str(), w - 2 * m::PAD);
        }
        let widest = lines.iter().map(|l| text_w(l)).max().unwrap_or(0);
        w = ceil_to(w.max(widest + 2 * m::PAD), 2);
        let h = ceil_to(
            m::BOX_MIN_H
                .max(port_h)
                .max(count(lines.len()) * m::LINE_H + m::CORNER_LABEL_H + 2 * m::PAD),
            4,
        );
        boxes.push(Rect { x, y, w, h });
        x += w + 2 * m::CLEAR + m::PITCH_X;
        y += h + 2 * m::CLEAR + m::PITCH_Y;
    }
    let right = boxes.last().map_or(m::MARGIN, |r| r.x + r.w) + m::MARGIN;
    let bottom = boxes.last().map_or(m::MARGIN, |r| r.y + r.h) + m::MARGIN;
    Placement {
        boxes,
        frame: (right, bottom),
        ports,
        index,
    }
}

/// An orthogonal polyline, in gu.
type Path = Vec<(i32, i32)>;

/// A routed edge: its path and the channels it uses (for the overflow term).
#[derive(Debug, Clone)]
pub struct Route {
    /// The polyline.
    pub path: Vec<(i32, i32)>,
    v_channels: Vec<usize>,
    h_channels: Vec<usize>,
    cost: i64,
}

/// Global routing statistics for one diagram.
#[derive(Debug, Clone)]
pub struct Routing {
    /// One route per edge, in edge order.
    pub routes: Vec<Route>,
    /// Total cost after the last pass.
    pub total_cost: i64,
    /// Crossings between distinct routes.
    pub crossings: usize,
    /// Total bends.
    pub bends: usize,
    /// Candidates evaluated, over all passes.
    pub candidates: usize,
    /// Rip-up passes run (1 = no improvement after the first).
    pub passes: usize,
}

fn bends(p: &Path) -> usize {
    p.len().saturating_sub(2)
}

fn length(p: &Path) -> i64 {
    p.windows(2)
        .map(|w| i64::from((w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs()))
        .sum()
}

/// Proper crossings between the orthogonal segments of two paths.
fn crossings(a: &Path, b: &Path) -> usize {
    let mut n = 0;
    for s in a.windows(2) {
        for t in b.windows(2) {
            let (hs, vs) = if s[0].1 == s[1].1 { (s, t) } else { (t, s) };
            if hs[0].1 != hs[1].1 || vs[0].0 != vs[1].0 {
                continue; // parallel
            }
            let (x0, x1) = (hs[0].0.min(hs[1].0), hs[0].0.max(hs[1].0));
            let (y0, y1) = (vs[0].1.min(vs[1].1), vs[0].1.max(vs[1].1));
            let (x, y) = (vs[0].0, hs[0].1);
            if x0 < x && x < x1 && y0 < y && y < y1 {
                n += 1;
            }
        }
    }
    n
}

/// Whether the axis-aligned segment `a`–`b` passes strictly inside `r`. Touching a side, as a
/// segment that ends on a port does, is not inside.
fn passes_inside(a: (i32, i32), b: (i32, i32), r: Rect) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    if y0 == y1 {
        r.y < y0 && y0 < r.y + r.h && x0 < r.x + r.w && r.x < x1
    } else {
        r.x < x0 && x0 < r.x + r.w && y0 < r.y + r.h && r.y < y1
    }
}

/// §6.3 rejections: a segment through a box interior, or a segment too short to fillet.
fn rejected(p: &Path, place: &Placement) -> bool {
    let last = p.len().saturating_sub(2);
    for (i, w) in p.windows(2).enumerate() {
        let len = (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs();
        let terminal = i == 0 || i == last;
        if (terminal && len < m::CLEAR) || (!terminal && len < 2 * m::R_FILLET_GU) {
            return true;
        }
        if place.boxes.iter().any(|r| passes_inside(w[0], w[1], *r)) {
            return true;
        }
    }
    false
}

/// A template candidate: the path plus the channels and the deviation from the preferred
/// parameter.
struct Candidate {
    path: Path,
    v: Vec<usize>,
    h: Vec<usize>,
    deviation: i64,
}

fn box_of(place: &Placement, e: &Endpoint) -> Option<(usize, Side, u8)> {
    match e {
        Endpoint::Box { key, side, rank } => place.index.get(key).map(|&b| (b, *side, *rank)),
        Endpoint::Boundary(_) => None,
    }
}

fn boundary(e: &Endpoint) -> Option<(Side, u8)> {
    match e {
        Endpoint::Boundary(p) => Some((
            p.side,
            match p.code {
                PortCode::Icom(n) | PortCode::Border(n) => n,
            },
        )),
        Endpoint::Box { .. } => None,
    }
}

fn dist(a: usize, b: usize) -> i64 {
    i64::try_from(a.abs_diff(b)).unwrap_or(i64::MAX)
}

/// The §6.2 template candidates for one edge.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per §6.2 template class, kept together for the spike"
)]
fn candidates(e: &Edge, place: &Placement) -> Vec<Candidate> {
    let n = place.boxes.len();
    let mut out = Vec::new();
    let mut push = |path: Path, v: Vec<usize>, h: Vec<usize>, deviation: i64| {
        out.push(Candidate {
            path,
            v,
            h,
            deviation,
        });
    };
    match (box_of(place, &e.src), box_of(place, &e.dst)) {
        (Some((i, _, rs)), Some((j, role, rd))) => {
            let (xs, ys) = place.port(i, Side::O, rs);
            let (xd, yd) = place.port(j, role, rd);
            let vi = place.v(i + 1);
            match (i.cmp(&j), role) {
                (core::cmp::Ordering::Less, Side::I) => {
                    for c in (i + 1)..=j {
                        let vc = place.v(c);
                        push(
                            vec![(xs, ys), (vc, ys), (vc, yd), (xd, yd)],
                            vec![c],
                            vec![],
                            dist(c, j),
                        );
                    }
                }
                (core::cmp::Ordering::Less, Side::C) => {
                    push(vec![(xs, ys), (xd, ys), (xd, yd)], vec![], vec![], 0);
                }
                (core::cmp::Ordering::Less, Side::M) => {
                    for c in (i + 1)..=j {
                        let (vc, hj) = (place.v(c), place.h(j + 1));
                        push(
                            vec![(xs, ys), (vc, ys), (vc, hj), (xd, hj), (xd, yd)],
                            vec![c],
                            vec![j + 1],
                            dist(c, j),
                        );
                    }
                }
                (_, Side::C) => {
                    // feedback or self control: up and over, through H_r above box j
                    for r in 0..=j {
                        let hr = place.h(r);
                        push(
                            vec![(xs, ys), (vi, ys), (vi, hr), (xd, hr), (xd, yd)],
                            vec![i + 1],
                            vec![r],
                            dist(r, j),
                        );
                    }
                }
                (_, Side::I) => {
                    // feedback or self input: down and under, H_r below box i, up V_c left of j
                    for r in (i + 1)..=n {
                        for c in 0..=j {
                            let (hr, vc) = (place.h(r), place.v(c));
                            let dev = dist(r, i + 1) + dist(c, j);
                            push(
                                vec![(xs, ys), (vi, ys), (vi, hr), (vc, hr), (vc, yd), (xd, yd)],
                                vec![i + 1, c],
                                vec![r],
                                dev,
                            );
                        }
                    }
                }
                (_, _) => {
                    // feedback or self mechanism: down and under, H_r below box i
                    for r in (i + 1)..=n {
                        let hr = place.h(r);
                        push(
                            vec![(xs, ys), (vi, ys), (vi, hr), (xd, hr), (xd, yd)],
                            vec![i + 1],
                            vec![r],
                            dist(r, i + 1),
                        );
                    }
                }
            }
        }
        (Some((i, _, rs)), None) => {
            // output exit: along the source row to the right frame (rows below box i are free)
            let (xs, ys) = place.port(i, Side::O, rs);
            push(vec![(xs, ys), (place.frame.0, ys)], vec![], vec![], 0);
            for c in (i + 1)..=n {
                let (vc, hn) = (place.v(c), place.h(n));
                push(
                    vec![(xs, ys), (vc, ys), (vc, hn), (place.frame.0, hn)],
                    vec![c],
                    vec![n],
                    1 + dist(c, n),
                );
            }
        }
        (None, Some((j, role, rd))) => {
            let (xd, yd) = place.port(j, role, rd);
            let side = boundary(&e.src).map_or(role, |(s, _)| s);
            match (side, role) {
                (Side::I, Side::I) => push(vec![(0, yd), (xd, yd)], vec![], vec![], 0),
                (Side::C, Side::C) => push(vec![(xd, 0), (xd, yd)], vec![], vec![], 0),
                (Side::M, Side::M) => push(vec![(xd, place.frame.1), (xd, yd)], vec![], vec![], 0),
                (Side::C, Side::I) => {
                    let v0 = place.v(0);
                    push(vec![(v0, 0), (v0, yd), (xd, yd)], vec![0], vec![], 0);
                }
                (Side::I, Side::C) => {
                    let h0 = place.h(0);
                    push(vec![(0, h0), (xd, h0), (xd, yd)], vec![], vec![0], 0);
                }
                (_, _) => {
                    // other role shifts: via the bottom margin
                    let hn = place.h(n);
                    push(vec![(0, hn), (xd, hn), (xd, yd)], vec![], vec![n], 0);
                }
            }
        }
        (None, None) => {}
    }
    out
}

/// §6.4 step 1: the routing order key of an edge (lower routes first).
fn order_key(e: &Edge, place: &Placement, i: usize) -> (u8, i32, u8, usize) {
    let role_weight = |s: Side| match s {
        Side::C => 3,
        Side::I => 2,
        _ => 1,
    };
    match (box_of(place, &e.src), box_of(place, &e.dst)) {
        (None, Some((_, role, _))) if boundary(&e.src).is_some_and(|(s, _)| s == role) => {
            (0, 0, 0, i)
        }
        (Some((s, _, _)), Some((d, role, _))) => {
            let class = match (s.cmp(&d), role) {
                (core::cmp::Ordering::Less, Side::C) => 0,
                (core::cmp::Ordering::Less, Side::I) => 1,
                (_, Side::C) => 2,
                (_, Side::I) => 3,
                _ => 4,
            };
            (1, -role_weight(role), class, i)
        }
        _ => (2, 0, 0, i),
    }
}

/// Global routing (§6.2–§6.4): templates, cost, deterministic order, up to `K = 3` rip-up
/// passes.
pub fn route(d: &Diagram, place: &Placement) -> Routing {
    let edges = edges(d);
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|&i| order_key(&edges[i], place, i));
    let all: Vec<Vec<Candidate>> = edges.iter().map(|e| candidates(e, place)).collect();
    let mut chosen: Vec<Option<Route>> = vec![None; edges.len()];
    let mut evaluated = 0usize;
    let mut best_total = i64::MAX;
    let mut passes = 0;
    for _ in 0..3 {
        passes += 1;
        for &k in &order {
            chosen[k] = None;
            let mut v_load: BTreeMap<usize, i64> = BTreeMap::new();
            let mut h_load: BTreeMap<usize, i64> = BTreeMap::new();
            for r in chosen.iter().flatten() {
                for c in &r.v_channels {
                    *v_load.entry(*c).or_default() += 1;
                }
                for c in &r.h_channels {
                    *h_load.entry(*c).or_default() += 1;
                }
            }
            let mut best: Option<Route> = None;
            for cand in &all[k] {
                evaluated += 1;
                if rejected(&cand.path, place) {
                    continue;
                }
                let x: usize = chosen
                    .iter()
                    .flatten()
                    .map(|r| crossings(&cand.path, &r.path))
                    .sum();
                let overflow: i64 = cand
                    .v
                    .iter()
                    .map(|c| (v_load.get(c).copied().unwrap_or(0) + 1 - 3).max(0))
                    .sum::<i64>()
                    + cand
                        .h
                        .iter()
                        .map(|c| (h_load.get(c).copied().unwrap_or(0) + 1 - 3).max(0))
                        .sum::<i64>();
                let cost = 100 * i64::try_from(bends(&cand.path)).unwrap_or(i64::MAX / 4)
                    + length(&cand.path)
                    + 40 * i64::try_from(x).unwrap_or(i64::MAX / 4)
                    + 25 * overflow
                    + 10 * cand.deviation;
                if best.as_ref().is_none_or(|b| cost < b.cost) {
                    best = Some(Route {
                        path: cand.path.clone(),
                        v_channels: cand.v.clone(),
                        h_channels: cand.h.clone(),
                        cost,
                    });
                }
            }
            chosen[k] = best;
        }
        let total: i64 = chosen.iter().flatten().map(|r| r.cost).sum();
        if total >= best_total {
            break;
        }
        best_total = total;
    }
    let routes: Vec<Route> = chosen.into_iter().flatten().collect();
    let mut crossings_total = 0;
    for (a, ra) in routes.iter().enumerate() {
        for rb in &routes[a + 1..] {
            crossings_total += crossings(&ra.path, &rb.path);
        }
    }
    Routing {
        bends: routes.iter().map(|r| bends(&r.path)).sum(),
        total_cost: best_total,
        crossings: crossings_total,
        candidates: evaluated,
        passes,
        routes,
    }
}

/// The Auto-Arrange Step 1 result (§12.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxOrder {
    /// For each position, the box index (in current order) placed there.
    pub order: Vec<usize>,
    /// The proxy cost of `order`.
    pub cost: i64,
    /// The proxy cost of the current order.
    pub current_cost: i64,
    /// Search-tree nodes visited.
    pub nodes: usize,
    /// Leaves (complete permutations) reached.
    pub leaves: usize,
}

struct Arcs {
    n: usize,
    /// `(i, j, w)` for each box-to-box arc `i.O -> j.R`.
    inner: Vec<(usize, usize, i64)>,
    /// Boxes fed by boundary I or C arrows, one entry per arrow.
    fed: Vec<usize>,
    /// Boxes feeding boundary O arrows, one entry per arrow.
    feeding: Vec<usize>,
    /// Per frame edge (I, C, O, M), `(code, box)` pairs.
    codes: [Vec<(u8, usize)>; 4],
}

fn arcs(d: &Diagram) -> Arcs {
    let index: BTreeMap<&BoxKey, usize> = d
        .boxes
        .iter()
        .enumerate()
        .map(|(i, b)| (&b.key, i))
        .collect();
    let mut a = Arcs {
        n: d.boxes.len(),
        inner: Vec::new(),
        fed: Vec::new(),
        feeding: Vec::new(),
        codes: Default::default(),
    };
    let side_ix = |s: Side| match s {
        Side::I => 0,
        Side::C => 1,
        Side::O => 2,
        Side::M => 3,
    };
    for e in edges(d) {
        let b = |end: &Endpoint| match end {
            Endpoint::Box { key, side, .. } => index.get(key).map(|&i| (i, *side)),
            Endpoint::Boundary(_) => None,
        };
        match (b(&e.src), b(&e.dst), boundary(&e.src), boundary(&e.dst)) {
            (Some((i, _)), Some((j, role)), _, _) => {
                let w = match role {
                    Side::C => 10,
                    _ => 6,
                };
                a.inner.push((i, j, w));
            }
            (None, Some((j, _)), Some((side, code)), _) => {
                if matches!(side, Side::I | Side::C) {
                    a.fed.push(j);
                }
                a.codes[side_ix(side)].push((code, j));
            }
            (Some((i, _)), None, _, Some((side, code))) => {
                a.feeding.push(i);
                a.codes[side_ix(side)].push((code, i));
            }
            _ => {}
        }
    }
    a
}

fn proxy_cost(a: &Arcs, pos: &[usize]) -> i64 {
    let p = |b: usize| i64::try_from(pos[b]).unwrap_or(i64::MAX / 8);
    let n = i64::try_from(a.n).unwrap_or(i64::MAX / 8);
    let mut cost = 0;
    for &(i, j, w) in &a.inner {
        if p(i) > p(j) {
            cost += 100 * w;
        }
        cost += w * (p(i) - p(j)).abs();
    }
    cost += 2 * a.fed.iter().map(|&j| p(j)).sum::<i64>();
    cost += 2 * a.feeding.iter().map(|&i| n - 1 - p(i)).sum::<i64>();
    for edge in &a.codes {
        for (x, &(ca, ba)) in edge.iter().enumerate() {
            for &(cb, bb) in &edge[x + 1..] {
                if (ca < cb && p(ba) > p(bb)) || (cb < ca && p(bb) > p(ba)) {
                    cost += 4;
                }
            }
        }
    }
    cost
}

/// Auto-Arrange Step 1 (§12.2): exhaustive branch and bound over box orders for the proxy
/// cost. Candidates are tried in current order, the incumbent starts as the current order, and
/// only a strictly lower cost replaces it.
pub fn arrange_box_order(d: &Diagram) -> BoxOrder {
    let a = arcs(d);
    let n = a.n;
    let identity: Vec<usize> = (0..n).collect();
    let current_cost = proxy_cost(&a, &identity);
    let mut st = Search {
        a: &a,
        best: identity.clone(),
        best_cost: current_cost,
        nodes: 0,
        leaves: 0,
    };
    let mut order = Vec::with_capacity(n);
    let mut used = vec![false; n];
    st.dfs(&mut order, &mut used);
    BoxOrder {
        order: st.best,
        cost: st.best_cost,
        current_cost,
        nodes: st.nodes,
        leaves: st.leaves,
    }
}

struct Search<'a> {
    a: &'a Arcs,
    best: Vec<usize>,
    best_cost: i64,
    nodes: usize,
    leaves: usize,
}

impl Search<'_> {
    fn dfs(&mut self, order: &mut Vec<usize>, used: &mut [bool]) {
        self.nodes += 1;
        let n = self.a.n;
        if order.len() == n {
            self.leaves += 1;
            let mut pos = vec![0; n];
            for (p, &b) in order.iter().enumerate() {
                pos[b] = p;
            }
            let c = proxy_cost(self.a, &pos);
            if c < self.best_cost {
                self.best_cost = c;
                self.best.clone_from(order);
            }
            return;
        }
        if self.bound(order) >= self.best_cost {
            return;
        }
        for b in 0..n {
            if !used[b] {
                used[b] = true;
                order.push(b);
                self.dfs(order, used);
                order.pop();
                used[b] = false;
            }
        }
    }

    /// The §12.2 lower bound of a partial assignment.
    fn bound(&self, order: &[usize]) -> i64 {
        let k = i64::try_from(order.len()).unwrap_or(i64::MAX / 8);
        let mut pos = vec![None; self.a.n];
        for (p, &b) in order.iter().enumerate() {
            pos[b] = Some(i64::try_from(p).unwrap_or(i64::MAX / 8));
        }
        let mut lb = 0;
        for &(i, j, w) in &self.a.inner {
            match (pos[i], pos[j]) {
                (Some(pi), Some(pj)) => {
                    if pi > pj {
                        lb += 100 * w;
                    }
                    lb += w * (pi - pj).abs();
                }
                (None, Some(pj)) => lb += 100 * w + w * (k - pj),
                (Some(pi), None) => lb += w * (k - pi),
                (None, None) => {}
            }
        }
        for &j in &self.a.fed {
            lb += 2 * pos[j].unwrap_or(k);
        }
        lb
    }
}

/// The current box order's proxy cost (for tests and the harness).
pub fn current_proxy_cost(d: &Diagram) -> i64 {
    let a = arcs(d);
    proxy_cost(&a, &(0..a.n).collect::<Vec<_>>())
}

/// The A0-or-lower diagram `node` of a model, for the harness.
pub fn diagram<'m>(model: &'m idef0_core::model::Model, node: &str) -> Option<&'m Diagram> {
    model
        .diagrams
        .get(&node.parse::<NodeId>().ok()?)
        .map(AsRef::as_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use idef0_core::dsl::parse;

    fn fixture(name: &str) -> idef0_core::model::Model {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/{name}"));
        parse(&std::fs::read_to_string(path).unwrap())
            .model
            .unwrap()
    }

    // §5.1, §5.4: a strict staircase; boxes never overlap and each is below and right of the
    // previous one.
    #[test]
    fn staircase() {
        let m = fixture("bench/dense-7x40.idef0");
        let p = place(diagram(&m, "A0").unwrap());
        for w in p.boxes.windows(2) {
            assert!(
                w[1].x > w[0].x + w[0].w && w[1].y > w[0].y + w[0].h,
                "{w:?}"
            );
        }
        assert!(
            p.boxes
                .iter()
                .all(|r| r.x % 2 == 0 && r.y % 4 == 0 && r.w % 2 == 0 && r.h % 4 == 0)
        );
    }

    // §6: every edge gets an orthogonal route, deterministically.
    #[test]
    fn routes_every_edge_orthogonally() {
        let m = fixture("bench/dense-7x40.idef0");
        let d = diagram(&m, "A0").unwrap();
        let p = place(d);
        let r = route(d, &p);
        assert_eq!(r.routes.len(), 40);
        for route in &r.routes {
            assert!(
                route
                    .path
                    .windows(2)
                    .all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1),
                "{:?}",
                route.path
            );
        }
        let again = route(d, &place(d));
        assert_eq!(
            (again.total_cost, again.crossings, again.bends),
            (r.total_cost, r.crossings, r.bends)
        );
    }

    // §12.2: the search never returns a worse order, and keeps the current order on ties.
    #[test]
    fn box_order_search() {
        for f in [
            "bench/arrange-worst-7.idef0",
            "bench/dense-7x40.idef0",
            "dogfood/idef0-rs.idef0",
        ] {
            let m = fixture(f);
            let d = diagram(&m, "A0").unwrap();
            let r = arrange_box_order(d);
            assert!(r.cost <= r.current_cost, "{f}");
            assert!(r.leaves <= 5040, "{f}: {}", r.leaves);
            if r.cost == r.current_cost {
                assert_eq!(
                    r.order,
                    (0..d.boxes.len()).collect::<Vec<_>>(),
                    "{f}: ties keep the order"
                );
            }
        }
    }
}
