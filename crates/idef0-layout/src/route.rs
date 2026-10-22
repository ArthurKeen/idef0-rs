//! Global routing (`docs/spec/layout.md` §5.4, §6; FR-LAYOUT-04, -06, -07, -11, -12, -22, -23).
//!
//! Each edge is routed by the template of its class (§6.2), choosing the template parameters
//! (channel indices) with the least §6.3 cost, in the deterministic §6.4 order, with up to
//! three rip-up passes. Runs inside a channel are placed on the channel's center line; track
//! assignment (§7, M4.4) later spreads them over tracks. The chosen parameters are the
//! channel assignment that placement freezes after iteration 1 (§5.2).

#![allow(
    clippy::many_single_char_names,
    reason = "geometry uses the spec's own names: a, b, c, r, n, x, y (layout §6)"
)]

use std::collections::BTreeMap;

use idef0_core::id::{NodeId, PortCode, Side};
use idef0_core::identity::{ArrowId, arrow_ids};
use idef0_core::model::{ArrowShape, Diagram, Endpoint};
use idef0_core::text::BoxKey;

use crate::metrics::{LATTICE_X, LATTICE_Y, Metrics, SU_PER_GU};
use crate::place::{Placement, Tracks};
use crate::ports::{call_rank, port_point};
use crate::size::PortCounts;

/// Rip-up and reroute passes (§6.4).
pub const PASSES: usize = 3;

/// What an edge is: an arrow (or a bundle's branch or leaf), or a box's call arrow.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKey {
    /// An arrow, by its id (metamodel §3.4).
    Arrow(ArrowId),
    /// The call arrow of a box.
    Call(BoxKey),
}

impl core::fmt::Display for EdgeKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Arrow(id) => write!(f, "{id}"),
            Self::Call(k) => write!(f, "call @{k}"),
        }
    }
}

/// An edge end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// Box index (box-number order), side, rank.
    Box(usize, Side, u8),
    /// A diagram boundary port: side and code; `border` for an uncoded `[S n]` port.
    Frame(Side, u8, bool),
    /// The bottom frame, below a call arrow.
    CallExit,
}

#[derive(Debug, Clone)]
struct Edge {
    key: EdgeKey,
    src: End,
    dst: End,
    /// Leaf count of the bundle the edge belongs to (1 for a plain arrow), for §6.4 weights.
    leaves: u32,
    /// Index of the arrow statement, shared by a bundle's branches or leaves.
    bundle: Option<usize>,
}

/// The template class of an edge (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// `i < j`, into I: E → `V_c` ↓ → E.
    ForwardInput,
    /// `i < j`, into C: E → ↓.
    ForwardControl,
    /// `i < j`, into M: E → `V_c` ↓ → `H_j` → E → ↑.
    ForwardMechanism,
    /// `i > j`, into C, up and over (FR-LAYOUT-07).
    FeedbackControl,
    /// `i > j`, into I, down and under (FR-LAYOUT-07).
    FeedbackInput,
    /// `i > j`, into M, down and under (FR-LAYOUT-07).
    FeedbackMechanism,
    /// `i = j`, into C: up and over the box itself.
    SelfControl,
    /// `i = j`, into I: down and under the box itself.
    SelfInput,
    /// `i = j`, into M: down and under the box itself, like a feedback mechanism.
    SelfMechanism,
    /// A boundary arrow on its matching side, in frame order: one segment.
    BoundaryStraight,
    /// A boundary arrow with a role shift, or one that frame order moves (an output exit
    /// included): via the margin channels.
    BoundaryDogLeg,
    /// `i →` boundary O where frame order keeps the port's row: straight to the right frame.
    OutputExit,
    /// A call arrow: straight down to the bottom frame (FR-LAYOUT-23).
    Call,
}

/// A channel: vertical `V_c` or horizontal `H_r` (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    /// Vertical channel `V_c`.
    V(usize),
    /// Horizontal channel `H_r`.
    H(usize),
}

/// A run of a route along a channel: the interval it covers on the channel's axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    /// The channel.
    pub channel: Channel,
    /// Start and end on the channel axis (y for `V`, x for `H`), in route order.
    pub span: (i32, i32),
    /// The index of the run's segment in the route's path (`path[seg]` to `path[seg + 1]`).
    pub seg: usize,
}

/// A routed edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The edge.
    pub key: EdgeKey,
    /// Its template class.
    pub class: Class,
    /// The chosen template parameters (channel indices), the frozen channel assignment.
    pub params: Vec<usize>,
    /// The orthogonal polyline in gu, from source to destination, every vertex on the
    /// lattice, with channel runs on their channel's center line. Track assignment
    /// ([`crate::tracks`]) moves each run onto its track.
    pub path: Vec<(i32, i32)>,
    /// The channel runs, in route order.
    pub legs: Vec<Leg>,
    /// Its §6.3 cost when chosen.
    pub cost: i64,
    /// Whether every candidate broke a §6.3 rejection rule, so the cheapest was taken anyway.
    pub forced: bool,
    /// The arrow statement a fork branch or join leaf belongs to: routes of one bundle are
    /// siblings that share a trunk. `None` for a plain arrow or a call.
    pub bundle: Option<usize>,
}

/// The global routing of one diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    /// One route per edge, in edge order (arrow statements, then calls).
    pub routes: Vec<Route>,
    /// Total cost.
    pub total_cost: i64,
    /// Rip-up passes run.
    pub passes: usize,
}

impl Routing {
    /// The tracks each channel needs if every run in it takes its own track where runs
    /// overlap: the maximum interval overlap per channel. Track assignment (M4.4) refines this.
    pub fn density(&self, n: usize) -> Tracks {
        let mut v: BTreeMap<usize, Vec<(i32, i32)>> = BTreeMap::new();
        let mut h: BTreeMap<usize, Vec<(i32, i32)>> = BTreeMap::new();
        for r in &self.routes {
            for leg in &r.legs {
                let (a, b) = (leg.span.0.min(leg.span.1), leg.span.0.max(leg.span.1));
                match leg.channel {
                    Channel::V(c) => v.entry(c).or_default().push((a, b)),
                    Channel::H(c) => h.entry(c).or_default().push((a, b)),
                }
            }
        }
        let mut out = Tracks::none(n);
        for (c, spans) in &v {
            out.v[*c] = max_overlap(spans);
        }
        for (c, spans) in &h {
            out.h[*c] = max_overlap(spans);
        }
        out
    }
}

/// The most closed intervals that share a point.
fn max_overlap(spans: &[(i32, i32)]) -> u32 {
    let mut events: Vec<(i32, i32)> = spans.iter().flat_map(|&(a, b)| [(a, -1), (b, 1)]).collect();
    events.sort_unstable();
    let (mut cur, mut best) = (0u32, 0u32);
    for (_, e) in events {
        if e < 0 {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = cur.saturating_sub(1);
        }
    }
    best
}

fn box_index(diagram: &Diagram, key: &BoxKey) -> Option<usize> {
    diagram.boxes.iter().position(|b| &b.key == key)
}

fn end(diagram: &Diagram, e: &Endpoint) -> Option<End> {
    match e {
        Endpoint::Box { key, side, rank } => {
            box_index(diagram, key).map(|i| End::Box(i, *side, *rank))
        }
        Endpoint::Boundary(p) => Some(match p.code {
            PortCode::Icom(n) => End::Frame(p.side, n, false),
            PortCode::Border(n) => End::Frame(p.side, n, true),
        }),
    }
}

/// The edges of `diagram` (node `node`): plain arrows, every fork branch and join leaf from
/// its true source to its true destination, and call arrows. Ends on missing boxes (type
/// errors) are skipped.
fn edges(node: &NodeId, diagram: &Diagram, counts: &[PortCounts]) -> Vec<Edge> {
    let ids = arrow_ids(node, diagram);
    let mut out = Vec::new();
    for (i, (a, id)) in diagram.arrows.iter().zip(&ids).enumerate() {
        let mut push =
            |key: ArrowId, src: &Endpoint, dst: &Endpoint, leaves: u32, bundle: Option<usize>| {
                if let (Some(src), Some(dst)) = (end(diagram, src), end(diagram, dst)) {
                    out.push(Edge {
                        key: EdgeKey::Arrow(key),
                        src,
                        dst,
                        leaves,
                        bundle,
                    });
                }
            };
        match &a.shape {
            ArrowShape::Plain { src, dst } => push(id.trunk.clone(), src, dst, 1, None),
            ArrowShape::Fork { src, fork } => {
                let n = u32::try_from(fork.len()).unwrap_or(u32::MAX);
                for (b, part) in fork.iter().zip(&id.parts) {
                    push(part.clone(), src, &b.dst, n, Some(i));
                }
            }
            ArrowShape::Join { join, dst } => {
                let n = u32::try_from(join.len()).unwrap_or(u32::MAX);
                for (l, part) in join.iter().zip(&id.parts) {
                    push(part.clone(), &l.src, dst, n, Some(i));
                }
            }
        }
    }
    for (b, fb) in diagram.boxes.iter().enumerate() {
        if let (Some(_), Some(rank)) = (&fb.call, counts.get(b).and_then(|c| call_rank(*c))) {
            out.push(Edge {
                key: EdgeKey::Call(fb.key.clone()),
                src: End::Box(b, Side::M, rank),
                dst: End::CallExit,
                leaves: 1,
                bundle: None,
            });
        }
    }
    out
}

pub(crate) fn snap_down(v: i32, g: i32) -> i32 {
    v.div_euclid(g) * g
}

/// Geometry shared by every candidate: the placement, the ports and the frame-order entries.
struct Geo<'a> {
    p: &'a Placement,
    counts: &'a [PortCounts],
    m: &'a Metrics,
    /// FR-LAYOUT-22: the frame coordinate assigned to each boundary end (by edge index).
    entry: BTreeMap<usize, i32>,
}

impl Geo<'_> {
    fn n(&self) -> usize {
        self.p.boxes.len()
    }

    /// The lattice point nearest the center of `V_c` (x).
    fn v(&self, c: usize) -> i32 {
        let (a, b) = self.p.v_channel(c);
        snap_down(i32::midpoint(a, b), LATTICE_X)
    }

    /// The lattice point nearest the center of `H_r` (y).
    fn h(&self, r: usize) -> i32 {
        let (a, b) = self.p.h_channel(r);
        snap_down(i32::midpoint(a, b), LATTICE_Y)
    }

    fn port(&self, b: usize, side: Side, k: u8) -> (i32, i32) {
        port_point(self.p.boxes[b], self.counts[b], side, k)
    }

    /// Track 0 of `V_c` (§7.2: `start(V) + clear`).
    fn v_track0(&self, c: usize) -> i32 {
        self.p.v_channel(c).0 + self.m.clear
    }

    /// Track 0 of `H_r`.
    fn h_track0(&self, r: usize) -> i32 {
        self.p.h_channel(r).0 + self.m.clear
    }
}

/// The "natural" frame coordinate of a boundary end before frame order: the port's coordinate
/// along the frame edge for a matching side, else track 0 of the margin channel the arrow
/// turns into (§6.2 dog-leg). Frame order spaces entries one pitch apart, so role-shift
/// entries land on that margin's track coordinates and run straight along it.
fn natural_entry(g: &Geo<'_>, frame_side: Side, other: End) -> i32 {
    let n = g.n();
    match other {
        End::Box(b, role, k) => {
            let (x, y) = g.port(b, role, k);
            match (frame_side, role) {
                (Side::I, Side::I) | (Side::O, _) => y,
                (Side::C, Side::C) | (Side::M, Side::M) => x,
                (Side::I, Side::C) => g.h_track0(0),
                (Side::I, _) => g.h_track0(n),
                (Side::C | Side::M, _) => g.v_track0(0),
            }
        }
        _ => 0,
    }
}

/// FR-LAYOUT-22: on each frame edge, coded boundary arrows take the edge's natural entry
/// coordinates in ICOM code order (the natural coordinates, sorted, handed out by code). When
/// codes already increase with the natural order every arrow keeps its own coordinate.
fn frame_order(g: &mut Geo<'_>, edges: &[Edge]) {
    // One boundary port is one point on the frame: the branches of a fork from it (or the
    // leaves of a join into it) share its entry. A group's natural coordinate is its
    // smallest matching-side member's, else its smallest member's.
    type Group = (i32, bool, Vec<usize>);
    let mut groups: BTreeMap<(Side, bool, u8, usize, usize), Group> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        let (frame, other) = match (e.src, e.dst) {
            (End::Frame(s, code, border), other) | (other, End::Frame(s, code, border)) => {
                ((s, code, border), other)
            }
            _ => continue,
        };
        let natural = natural_entry(g, frame.0, other);
        let matching =
            frame.0 == Side::O || matches!(other, End::Box(_, role, _) if role == frame.0);
        let key = (
            frame.0,
            frame.2,
            frame.1,
            e.bundle.unwrap_or(usize::MAX),
            if e.bundle.is_some() { 0 } else { i },
        );
        let group = groups.entry(key).or_insert((natural, matching, Vec::new()));
        if (matching && !group.1) || (matching == group.1 && natural < group.0) {
            group.0 = natural;
            group.1 = matching;
        }
        group.2.push(i);
    }
    let mut coded: BTreeMap<Side, Vec<(u8, i32, usize)>> = BTreeMap::new();
    let mut on_side: BTreeMap<Side, Vec<usize>> = BTreeMap::new();
    for (&(side, border, code, _, _), (natural, _, members)) in &groups {
        let i = members[0];
        on_side.entry(side).or_default().push(i);
        if border {
            g.entry.insert(i, *natural); // uncoded border ports have no code to order by
        } else {
            coded.entry(side).or_default().push((code, *natural, i));
        }
    }
    for list in coded.values_mut() {
        let mut coords: Vec<i32> = list.iter().map(|&(_, c, _)| c).collect();
        coords.sort_unstable();
        list.sort_unstable();
        for ((_, _, i), c) in list.iter().zip(coords) {
            g.entry.insert(*i, c);
        }
    }
    space_entries(g, on_side);
    for (_, _, members) in groups.values() {
        if let Some(&at) = g.entry.get(&members[0]) {
            for &i in &members[1..] {
                g.entry.insert(i, at);
            }
        }
    }
}

/// Space the entries of `on_side` (one per boundary port) along their frame edges.
fn space_entries(g: &mut Geo<'_>, on_side: BTreeMap<Side, Vec<usize>>) {
    // Two arrows never enter a frame edge at one point: walking each edge in coordinate
    // order, push every entry at least one track pitch past the previous, then walk back from
    // the far end so none passes the frame corner's clearance. Both walks keep the order, so
    // the code order above is kept. Entries inside `V_0` on the top and bottom edges run
    // straight along `V_0` (role shifts), so the bottom edge's start one pitch past the top
    // edge's: the two never share a track.
    let (fw, fh) = g.p.frame;
    let v0_end = g.p.v_channel(0).1;
    let mut top_in_v0: Option<i32> = None;
    let mut sides: Vec<(Side, Vec<usize>)> = on_side.into_iter().collect();
    sides.sort_by_key(|(side, _)| matches!(side, Side::M));
    for (side, list) in sides {
        let (step, extent, lattice) = if matches!(side, Side::I | Side::O) {
            (g.m.pitch_y, fh, LATTICE_Y)
        } else {
            (g.m.pitch_x, fw, LATTICE_X)
        };
        let lo = -snap_down(-g.m.clear, lattice);
        let hi = snap_down(extent - g.m.clear, lattice);
        let mut order: Vec<(i32, usize)> = list
            .iter()
            .map(|&i| (g.entry.get(&i).copied().unwrap_or(0), i))
            .collect();
        order.sort_unstable();
        let mut prev: Option<i32> = if side == Side::M { top_in_v0 } else { None };
        for (c, _) in &mut order {
            *c = prev.map_or(*c, |p| (*c).max(p + step)).max(lo);
            prev = Some(*c);
        }
        let mut next: Option<i32> = None;
        for (c, i) in order.iter_mut().rev() {
            *c = next.map_or(hi, |n| n - step).min(*c);
            next = Some(*c);
            g.entry.insert(*i, *c);
            if side == Side::C && *c < v0_end {
                top_in_v0 = top_in_v0.max(Some(*c));
            }
        }
    }
}

struct Candidate {
    params: Vec<usize>,
    path: Vec<(i32, i32)>,
    legs: Vec<Leg>,
    deviation: i64,
}

fn dist(a: usize, b: usize) -> i64 {
    i64::try_from(a.abs_diff(b)).unwrap_or(i64::MAX / 8)
}

/// The class of an edge (§6.2).
fn class(e: &Edge, g: &Geo<'_>, i: usize) -> Class {
    match (e.src, e.dst) {
        (End::Box(..), End::CallExit) => Class::Call,
        (End::Box(a, ..), End::Box(b, role, _)) => match (a.cmp(&b), role) {
            (core::cmp::Ordering::Less, Side::I) => Class::ForwardInput,
            (core::cmp::Ordering::Less, Side::C) => Class::ForwardControl,
            (core::cmp::Ordering::Less, _) => Class::ForwardMechanism,
            (core::cmp::Ordering::Greater, Side::C) => Class::FeedbackControl,
            (core::cmp::Ordering::Greater, Side::I) => Class::FeedbackInput,
            (core::cmp::Ordering::Greater, _) => Class::FeedbackMechanism,
            (_, Side::C) => Class::SelfControl,
            (_, Side::I) => Class::SelfInput,
            (_, _) => Class::SelfMechanism,
        },
        (End::Box(b, side, k), End::Frame(..)) => {
            let (_, y) = g.port(b, side, k);
            if g.entry.get(&i).is_some_and(|&e| e != y) {
                Class::BoundaryDogLeg
            } else {
                Class::OutputExit
            }
        }
        (End::Frame(fs, ..), End::Box(b, role, k)) => {
            let (x, y) = g.port(b, role, k);
            let along = if matches!(fs, Side::I | Side::O) {
                y
            } else {
                x
            };
            if fs == role && g.entry.get(&i).is_none_or(|&e| e == along) {
                Class::BoundaryStraight
            } else {
                Class::BoundaryDogLeg
            }
        }
        _ => Class::BoundaryDogLeg,
    }
}

/// The candidates of edge `i` (§6.2 templates with every parameter value).
#[allow(
    clippy::too_many_lines,
    reason = "one arm per §6.2 template class, kept side by side"
)]
fn candidates(e: &Edge, i: usize, g: &Geo<'_>) -> (Class, Vec<Candidate>) {
    let n = g.n();
    let class = class(e, g, i);
    let mut out = Vec::new();
    let v = |c: usize| Channel::V(c);
    let hc = |r: usize| Channel::H(r);
    let mut add =
        |params: Vec<usize>, path: Vec<(i32, i32)>, legs: Vec<(Channel, usize)>, deviation: i64| {
            let legs = legs
                .into_iter()
                .map(|(channel, seg)| {
                    let (a, b) = (path[seg], path[seg + 1]);
                    let span = if matches!(channel, Channel::V(_)) {
                        (a.1, b.1)
                    } else {
                        (a.0, b.0)
                    };
                    Leg { channel, span, seg }
                })
                .collect();
            out.push(Candidate {
                params,
                path,
                legs,
                deviation,
            });
        };
    let (fw, fh) = g.p.frame;
    match (e.src, e.dst) {
        (End::Box(a, _, rs), End::Box(b, role, rd)) => {
            let (xs, ys) = g.port(a, Side::O, rs);
            let (xd, yd) = g.port(b, role, rd);
            let vi = a + 1; // V_i: right of the source box
            match class {
                Class::ForwardInput => {
                    for c in (a + 1)..=b {
                        add(
                            vec![c],
                            vec![(xs, ys), (g.v(c), ys), (g.v(c), yd), (xd, yd)],
                            vec![(v(c), 1)],
                            dist(c, b),
                        );
                    }
                }
                Class::ForwardControl => add(vec![], vec![(xs, ys), (xd, ys), (xd, yd)], vec![], 0),
                Class::ForwardMechanism => {
                    for c in (a + 1)..=b {
                        let (x, y) = (g.v(c), g.h(b + 1));
                        add(
                            vec![c],
                            vec![(xs, ys), (x, ys), (x, y), (xd, y), (xd, yd)],
                            vec![(v(c), 1), (hc(b + 1), 2)],
                            dist(c, b),
                        );
                    }
                }
                Class::FeedbackControl | Class::SelfControl => {
                    let rs: Vec<usize> = if class == Class::SelfControl {
                        vec![a]
                    } else {
                        (0..=b).collect()
                    };
                    for r in rs {
                        let (x, y) = (g.v(vi), g.h(r));
                        add(
                            vec![r],
                            vec![(xs, ys), (x, ys), (x, y), (xd, y), (xd, yd)],
                            vec![(v(vi), 1), (hc(r), 2)],
                            dist(r, b),
                        );
                    }
                }
                Class::FeedbackInput | Class::SelfInput => {
                    let rows: Vec<usize> = if class == Class::SelfInput {
                        vec![a + 1]
                    } else {
                        ((a + 1)..=n).collect()
                    };
                    let cols: Vec<usize> = if class == Class::SelfInput {
                        vec![a]
                    } else {
                        (0..=b).collect()
                    };
                    for &r in &rows {
                        for &c in &cols {
                            let (x, y, xc) = (g.v(vi), g.h(r), g.v(c));
                            let path = vec![(xs, ys), (x, ys), (x, y), (xc, y), (xc, yd), (xd, yd)];
                            add(
                                vec![r, c],
                                path,
                                vec![(v(vi), 1), (hc(r), 2), (v(c), 3)],
                                dist(r, a + 1) + dist(c, b),
                            );
                        }
                    }
                }
                _ => {
                    // feedback or self mechanism: down and under, H_r below the source box
                    for r in (a + 1)..=n {
                        let (x, y) = (g.v(vi), g.h(r));
                        add(
                            vec![r],
                            vec![(xs, ys), (x, ys), (x, y), (xd, y), (xd, yd)],
                            vec![(v(vi), 1), (hc(r), 2)],
                            dist(r, a + 1),
                        );
                    }
                }
            }
        }
        (End::Box(a, _, rs), End::CallExit) => {
            let (x, y) = g.port(a, Side::M, rs);
            add(vec![], vec![(x, y), (x, fh)], vec![], 0);
        }
        (End::Box(a, _, rs), End::Frame(..)) => {
            let (xs, ys) = g.port(a, Side::O, rs);
            let exit = g.entry.get(&i).copied().unwrap_or(ys);
            if exit == ys {
                add(vec![], vec![(xs, ys), (fw, ys)], vec![], 0);
            } else {
                let x = g.v(n);
                add(
                    vec![n],
                    vec![(xs, ys), (x, ys), (x, exit), (fw, exit)],
                    vec![(v(n), 1)],
                    0,
                );
            }
        }
        (End::Frame(fs, ..), End::Box(b, role, rd)) => {
            let (xd, yd) = g.port(b, role, rd);
            let at = g.entry.get(&i).copied().unwrap_or(0);
            match (fs, role) {
                (Side::I, Side::I) if at == yd => add(vec![], vec![(0, yd), (xd, yd)], vec![], 0),
                (Side::I, Side::I) => add(
                    vec![0],
                    vec![(0, at), (g.v(0), at), (g.v(0), yd), (xd, yd)],
                    vec![(v(0), 1)],
                    0,
                ),
                (Side::C, Side::C) if at == xd => add(vec![], vec![(xd, 0), (xd, yd)], vec![], 0),
                (Side::C, Side::C) => add(
                    vec![0],
                    vec![(at, 0), (at, g.h(0)), (xd, g.h(0)), (xd, yd)],
                    vec![(hc(0), 1)],
                    0,
                ),
                (Side::M, Side::M) if at == xd => add(vec![], vec![(xd, fh), (xd, yd)], vec![], 0),
                (Side::M, Side::M) => add(
                    vec![n],
                    vec![(at, fh), (at, g.h(n)), (xd, g.h(n)), (xd, yd)],
                    vec![(hc(n), 1)],
                    0,
                ),
                (fs, role) => {
                    // role shift (§6.2 dog-leg): around the margins, whatever entry frame
                    // order assigned
                    let (path, legs) = role_shift(g, fs, at, role, (xd, yd));
                    add(vec![], path, legs, 0);
                }
            }
        }
        _ => {}
    }
    (class, out)
}

/// A channel run: its channel and the index of its segment in the path.
type ChannelSeg = (Channel, usize);

/// A role-shift boundary arrow (§6.2 dog-leg) from the frame entry `at` on side `fs` into
/// `port` on side `role`. An entry inside the margin channel it turns into, at least `clear`
/// from its boxes, (`H_0` for I into
/// C, `H_n` for I into M, `V_0` for C or M into another role) runs straight along that margin
/// as a fixed run. Any other entry, wherever frame order (FR-LAYOUT-22) put it, turns into
/// the margin beside its frame edge and travels the margin channels (`V_0`, `H_0`, `H_n`) to
/// the one beside the port's side. The margins are free of boxes, so every variant is valid.
fn role_shift(
    g: &Geo<'_>,
    fs: Side,
    at: i32,
    role: Side,
    port: (i32, i32),
) -> (Vec<(i32, i32)>, Vec<ChannelSeg>) {
    use Channel::{H, V};
    let n = g.n();
    let fh = g.p.frame.1;
    let (v0, h0, hn) = (g.v(0), g.h(0), g.h(n));
    let (xd, yd) = port;
    let clear = g.m.clear;
    let in_v0 = at + clear <= g.p.v_channel(0).1;
    match (fs, role) {
        (Side::I, Side::C) if at + clear <= g.p.h_channel(0).1 => {
            (vec![(0, at), (xd, at), (xd, yd)], vec![])
        }
        (Side::I, Side::C) => (
            vec![(0, at), (v0, at), (v0, h0), (xd, h0), (xd, yd)],
            vec![(V(0), 1), (H(0), 2)],
        ),
        (Side::I, _) if at >= g.p.h_channel(n).0 + clear => {
            (vec![(0, at), (xd, at), (xd, yd)], vec![])
        }
        (Side::I, _) => (
            vec![(0, at), (v0, at), (v0, hn), (xd, hn), (xd, yd)],
            vec![(V(0), 1), (H(n), 2)],
        ),
        (Side::C, Side::I) if in_v0 => (vec![(at, 0), (at, yd), (xd, yd)], vec![]),
        (Side::C, Side::I) => (
            vec![(at, 0), (at, h0), (v0, h0), (v0, yd), (xd, yd)],
            vec![(H(0), 1), (V(0), 2)],
        ),
        (Side::C, _) if in_v0 => (vec![(at, 0), (at, hn), (xd, hn), (xd, yd)], vec![(H(n), 1)]),
        (Side::C, _) => (
            vec![(at, 0), (at, h0), (v0, h0), (v0, hn), (xd, hn), (xd, yd)],
            vec![(H(0), 1), (V(0), 2), (H(n), 3)],
        ),
        (_, Side::I) if in_v0 => (vec![(at, fh), (at, yd), (xd, yd)], vec![]),
        (_, Side::I) => (
            vec![(at, fh), (at, hn), (v0, hn), (v0, yd), (xd, yd)],
            vec![(H(n), 1), (V(0), 2)],
        ),
        (_, _) if in_v0 => (
            vec![(at, fh), (at, h0), (xd, h0), (xd, yd)],
            vec![(H(0), 1)],
        ),
        (_, _) => (
            vec![(at, fh), (at, hn), (v0, hn), (v0, h0), (xd, h0), (xd, yd)],
            vec![(H(n), 1), (V(0), 2), (H(0), 3)],
        ),
    }
}

fn bends(p: &[(i32, i32)]) -> usize {
    // collinear consecutive segments are not bends
    p.windows(3)
        .filter(|w| {
            !((w[0].0 == w[1].0 && w[1].0 == w[2].0) || (w[0].1 == w[1].1 && w[1].1 == w[2].1))
        })
        .count()
}

fn length(p: &[(i32, i32)]) -> i64 {
    p.windows(2)
        .map(|w| i64::from((w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs()))
        .sum()
}

type Seg = ((i32, i32), (i32, i32));

/// Proper crossings between two polylines' orthogonal segments.
pub(crate) fn crossings(a: &[(i32, i32)], b: &[(i32, i32)]) -> usize {
    let mut n = 0;
    for s in a.windows(2) {
        for t in b.windows(2) {
            let (h, v): (Seg, Seg) = if s[0].1 == s[1].1 {
                ((s[0], s[1]), (t[0], t[1]))
            } else {
                ((t[0], t[1]), (s[0], s[1]))
            };
            if h.0.1 != h.1.1 || v.0.0 != v.1.0 {
                continue;
            }
            let (x, y) = (v.0.0, h.0.1);
            let (x0, x1) = (h.0.0.min(h.1.0), h.0.0.max(h.1.0));
            let (y0, y1) = (v.0.1.min(v.1.1), v.0.1.max(v.1.1));
            if x0 < x && x < x1 && y0 < y && y < y1 {
                n += 1;
            }
        }
    }
    n
}

/// §6.3 rejections, for the segments of `path` that are not channel runs (`legs` mark those by
/// index). Ends attach to `own` boxes, which a terminal segment may touch.
fn rejected(
    path: &[(i32, i32)],
    channel_segs: &[usize],
    own: &[usize],
    g: &Geo<'_>,
    placed: &[(usize, &Route)],
    bundle: Option<usize>,
    edges: &[Edge],
) -> bool {
    let m = g.m;
    let last = path.len().saturating_sub(2);
    for (s, w) in path.windows(2).enumerate() {
        let len = (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs();
        let terminal = s == 0 || s == last;
        if (terminal && len < m.clear) || (!terminal && len * SU_PER_GU < 2 * m.r_fillet) {
            return true;
        }
        let (x0, x1) = (w[0].0.min(w[1].0), w[0].0.max(w[1].0));
        let (y0, y1) = (w[0].1.min(w[1].1), w[0].1.max(w[1].1));
        for (b, k) in g.p.keep_out.iter().enumerate() {
            if terminal && own.contains(&b) {
                // a terminal segment touches its own box: only its interior is barred
                let r = g.p.boxes[b];
                if inside(x0, x1, y0, y1, r) {
                    return true;
                }
                continue;
            }
            let dx = (k.x - x1).max(x0 - (k.x + k.w)).max(0);
            let dy = (k.y - y1).max(y0 - (k.y + k.h)).max(0);
            if dx.max(dy) < m.clear {
                return true;
            }
        }
        if !channel_segs.contains(&s) {
            for (j, r) in placed {
                if bundle.is_some() && edges[*j].bundle == bundle {
                    continue; // siblings share the trunk
                }
                let segs = concrete_segments(r);
                if segs.iter().any(|t| collinear_overlap((w[0], w[1]), *t)) {
                    return true;
                }
            }
        }
    }
    false
}

fn inside(x0: i32, x1: i32, y0: i32, y1: i32, r: crate::ports::Rect) -> bool {
    if y0 == y1 {
        r.y < y0 && y0 < r.y + r.h && x0 < r.x + r.w && r.x < x1
    } else {
        r.x < x0 && x0 < r.x + r.w && y0 < r.y + r.h && r.y < y1
    }
}

fn concrete_segments(r: &Route) -> Vec<Seg> {
    r.path
        .windows(2)
        .enumerate()
        .filter(|(s, _)| !r.legs.iter().any(|l| l.seg == *s))
        .map(|(_, w)| (w[0], w[1]))
        .collect()
}

fn collinear_overlap(a: Seg, b: Seg) -> bool {
    let horizontal = |s: Seg| s.0.1 == s.1.1;
    if horizontal(a) && horizontal(b) && a.0.1 == b.0.1 {
        let (a0, a1) = (a.0.0.min(a.1.0), a.0.0.max(a.1.0));
        let (b0, b1) = (b.0.0.min(b.1.0), b.0.0.max(b.1.0));
        a0.max(b0) < a1.min(b1)
    } else if !horizontal(a) && !horizontal(b) && a.0.0 == b.0.0 {
        let (a0, a1) = (a.0.1.min(a.1.1), a.0.1.max(a.1.1));
        let (b0, b1) = (b.0.1.min(b.1.1), b.0.1.max(b.1.1));
        a0.max(b0) < a1.min(b1)
    } else {
        false
    }
}

fn own_boxes(e: &Edge) -> Vec<usize> {
    [e.src, e.dst]
        .iter()
        .filter_map(|x| match x {
            End::Box(b, ..) => Some(*b),
            _ => None,
        })
        .collect()
}

/// §6.4 order key: group, weight, class priority, source rank, edge key.
fn order_key(e: &Edge, class: Class) -> (u8, i64, u8, u8, EdgeKey) {
    let role = match e.dst {
        End::Box(_, Side::C, _) => 3,
        End::Box(_, Side::I, _) => 2,
        _ => 1,
    };
    let src_rank = match e.src {
        End::Box(_, _, k) | End::Frame(_, k, _) => k,
        End::CallExit => 0,
    };
    let (group, priority) = match class {
        // matching-side boundary arrows, fixed by §5.4: an output exit is always straight
        // (a displaced exit is a dog-leg)
        Class::BoundaryStraight | Class::OutputExit => (0, 0),
        Class::ForwardControl => (1, 0),
        Class::ForwardInput => (1, 1),
        Class::FeedbackControl | Class::SelfControl => (1, 2),
        Class::FeedbackInput | Class::SelfInput => (1, 3),
        Class::ForwardMechanism | Class::FeedbackMechanism | Class::SelfMechanism => (1, 4),
        Class::BoundaryDogLeg => (2, 0),
        Class::Call => (3, 0),
    };
    let weight = if group == 1 {
        -i64::from(e.leaves) * role
    } else {
        0
    };
    (group, weight, priority, src_rank, e.key.clone())
}

struct Ctx<'a> {
    edges: Vec<Edge>,
    geo: Geo<'a>,
    classes: Vec<Class>,
    candidates: Vec<Vec<Candidate>>,
    order: Vec<usize>,
}

fn context<'a>(
    node: &NodeId,
    diagram: &Diagram,
    p: &'a Placement,
    counts: &'a [PortCounts],
    m: &'a Metrics,
) -> Ctx<'a> {
    let edges = edges(node, diagram, counts);
    let mut geo = Geo {
        p,
        counts,
        m,
        entry: BTreeMap::new(),
    };
    frame_order(&mut geo, &edges);
    let (classes, candidates): (Vec<Class>, Vec<Vec<Candidate>>) = edges
        .iter()
        .enumerate()
        .map(|(i, e)| candidates(e, i, &geo))
        .unzip();
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|&i| order_key(&edges[i], classes[i]));
    Ctx {
        edges,
        geo,
        classes,
        candidates,
        order,
    }
}

fn cost_of(cand: &Candidate, k: usize, ctx: &Ctx<'_>, chosen: &[Option<Route>]) -> (bool, i64) {
    let placed: Vec<(usize, &Route)> = chosen
        .iter()
        .enumerate()
        .filter_map(|(j, r)| r.as_ref().map(|r| (j, r)))
        .collect();
    let e = &ctx.edges[k];
    let channel_segs: Vec<usize> = cand.legs.iter().map(|l| l.seg).collect();
    let bad = rejected(
        &cand.path,
        &channel_segs,
        &own_boxes(e),
        &ctx.geo,
        &placed,
        e.bundle,
        &ctx.edges,
    );
    let x: usize = placed
        .iter()
        .filter(|(j, _)| e.bundle.is_none() || ctx.edges[*j].bundle != e.bundle)
        .map(|(_, r)| crossings(&cand.path, &r.path))
        .sum();
    let mut overflow = 0i64;
    for leg in &cand.legs {
        let mut spans: Vec<(i32, i32)> = placed
            .iter()
            .flat_map(|(_, r)| {
                r.legs
                    .iter()
                    .filter(|l| l.channel == leg.channel)
                    .map(|l| (l.span.0.min(l.span.1), l.span.0.max(l.span.1)))
            })
            .collect();
        spans.push((leg.span.0.min(leg.span.1), leg.span.0.max(leg.span.1)));
        overflow += (i64::from(max_overlap(&spans)) - 3).max(0);
    }
    let big = |v: usize| i64::try_from(v).unwrap_or(i64::MAX / 8);
    let cost = 100 * big(bends(&cand.path))
        + length(&cand.path)
        + 40 * big(x)
        + 25 * overflow
        + 10 * cand.deviation;
    (bad, cost)
}

fn choose(
    k: usize,
    ctx: &Ctx<'_>,
    chosen: &[Option<Route>],
    only: Option<&[usize]>,
) -> Option<Route> {
    let mut best: Option<((bool, i64), usize)> = None;
    for (ci, cand) in ctx.candidates[k].iter().enumerate() {
        if only.is_some_and(|p| p != cand.params.as_slice()) {
            continue;
        }
        let score = cost_of(cand, k, ctx, chosen);
        if best.is_none_or(|(b, _)| score < b) {
            best = Some((score, ci));
        }
    }
    best.map(|((forced, cost), ci)| {
        let cand = &ctx.candidates[k][ci];
        Route {
            key: ctx.edges[k].key.clone(),
            class: ctx.classes[k],
            params: cand.params.clone(),
            path: cand.path.clone(),
            legs: cand.legs.clone(),
            cost,
            forced,
            bundle: ctx.edges[k].bundle,
        }
    })
}

/// Route every edge of diagram `node` on `placement` (§6.4): in order, the cheapest
/// candidate given the edges already placed (ties by parameter order), then up to
/// [`PASSES`] − 1 rip-up passes while the total cost falls.
pub fn route(
    node: &NodeId,
    diagram: &Diagram,
    placement: &Placement,
    counts: &[PortCounts],
    m: &Metrics,
) -> Routing {
    let ctx = context(node, diagram, placement, counts, m);
    let mut chosen: Vec<Option<Route>> = vec![None; ctx.edges.len()];
    let mut best_total = i64::MAX;
    let mut best = chosen.clone();
    let mut passes = 0;
    for _ in 0..PASSES {
        passes += 1;
        for &k in &ctx.order {
            chosen[k] = None;
            chosen[k] = choose(k, &ctx, &chosen, None);
        }
        let total: i64 = chosen.iter().flatten().map(|r| r.cost).sum();
        if total >= best_total {
            break;
        }
        best_total = total;
        best.clone_from(&chosen);
    }
    Routing {
        routes: best.into_iter().flatten().collect(),
        total_cost: best_total,
        passes,
    }
}

/// Recompute `frozen`'s routes on a new `placement` with the same template parameters: the
/// frozen channel assignment of placement iterations k ≥ 2 (§5.2).
pub fn reroute(
    frozen: &Routing,
    node: &NodeId,
    diagram: &Diagram,
    placement: &Placement,
    counts: &[PortCounts],
    m: &Metrics,
) -> Routing {
    let ctx = context(node, diagram, placement, counts, m);
    let params: BTreeMap<&EdgeKey, &[usize]> = frozen
        .routes
        .iter()
        .map(|r| (&r.key, r.params.as_slice()))
        .collect();
    let mut chosen: Vec<Option<Route>> = vec![None; ctx.edges.len()];
    for &k in &ctx.order {
        let only = params.get(&ctx.edges[k].key).copied();
        chosen[k] = choose(k, &ctx, &chosen, only).or_else(|| choose(k, &ctx, &chosen, None));
    }
    let routes: Vec<Route> = chosen.into_iter().flatten().collect();
    Routing {
        total_cost: routes.iter().map(|r| r.cost).sum(),
        routes,
        passes: 0,
    }
}

/// Placement, global routing and track assignment of one diagram (§5.2, §6, §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    /// The placement iteration's result.
    pub placed: crate::place::Placed,
    /// Global routing on the final placement: channel runs on center lines.
    pub routing: Routing,
    /// Track assignment on the final placement: the final polylines.
    pub wiring: crate::tracks::Wiring,
}

/// Placement, global routing and track assignment of diagram `node` together (§5.2).
/// Iteration 1 routes from scratch; later iterations keep its channel assignment and only
/// recompute geometry. Each iteration assigns tracks, and each channel's track count sizes its
/// gap; a channel keeps its previous assignment when a new one would need more tracks.
pub fn layout_routes(
    model: &idef0_core::model::Model,
    node: &NodeId,
    shadows: bool,
    metric: crate::text::TextMetric,
    m: &Metrics,
) -> Option<Routed> {
    let diagram = model.diagrams.get(node)?;
    let plans = crate::place::box_plans(model, node, shadows, metric, m);
    let counts = crate::place::port_counts(diagram);
    let mut frozen: Option<Routing> = None;
    let mut wired: Option<crate::tracks::Wiring> = None;
    let mut passes = 0;
    let placed =
        crate::place::iterate(&plans, crate::place::EdgeAllowance::default(), m, |p, _| {
            let r = if let Some(f) = &frozen {
                reroute(f, node, diagram, p, &counts, m)
            } else {
                let r = route(node, diagram, p, &counts, m);
                passes = r.passes;
                r
            };
            let w = crate::tracks::assign(&r, p, m, wired.as_ref());
            let tracks = w.tracks.clone();
            frozen = Some(r);
            wired = Some(w);
            tracks
        });
    // the final placement may postdate the last route (overflow): recompute its geometry
    let last = frozen.unwrap_or_else(|| route(node, diagram, &placed.placement, &counts, m));
    let routing = reroute(&last, node, diagram, &placed.placement, &counts, m);
    let wiring = crate::tracks::assign(&routing, &placed.placement, m, wired.as_ref());
    Some(Routed {
        placed,
        routing: Routing { passes, ..routing },
        wiring,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextMetric;
    use idef0_core::model::Model;

    /// A three-box A0 (the template fixtures' boxes) with `arrows`.
    fn three(arrows: &str) -> Model {
        let src = format!(
            "idef0 0.2\nmodel \"T\" {{\n  abbrev \"T\"\n}}\ndiagram A0 {{\n  box 1 \"Plan Work\"\n  box 2 \"Do Work\"\n  box 3 \"Check Work\"\n{arrows}}}\n"
        );
        let p = idef0_core::dsl::parse(&src);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        p.model.expect("model")
    }

    /// `f` on the routing context of A0 after placement.
    fn with_ctx<R>(model: &Model, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
        let m = Metrics::default();
        let node = NodeId::a0();
        let placed = layout_routes(model, &node, true, TextMetric::Mono, &m)
            .expect("A0")
            .placed;
        let diagram = &model.diagrams[&node];
        let counts = crate::place::port_counts(diagram);
        let mut ctx = context(&node, diagram, &placed.placement, &counts, &m);
        f(&mut ctx)
    }

    fn probe(ctx: &Ctx<'_>) -> usize {
        ctx.edges
            .iter()
            .position(|e| matches!(&e.key, EdgeKey::Arrow(id) if id.as_str() == "probe"))
            .expect("probe")
    }

    fn fake(ctx: &Ctx<'_>, j: usize, path: Vec<(i32, i32)>, legs: Vec<Leg>) -> Route {
        Route {
            key: ctx.edges[j].key.clone(),
            class: Class::ForwardControl,
            params: vec![],
            path,
            legs,
            cost: 0,
            forced: false,
            bundle: None,
        }
    }

    // §6.2: each template enumerates every channel its parameters may take (0-based box
    // indices a → b, n = 3 boxes).
    #[test]
    fn template_parameter_ranges() {
        for (arrow, class, params) in [
            ("1O -> 3I", Class::ForwardInput, vec![vec![1], vec![2]]),
            ("1O -> 3M", Class::ForwardMechanism, vec![vec![1], vec![2]]),
            ("3O -> 2C", Class::FeedbackControl, vec![vec![0], vec![1]]),
            (
                "2O -> 1I",
                Class::FeedbackInput,
                vec![vec![2, 0], vec![3, 0]],
            ),
            ("2O -> 1M", Class::FeedbackMechanism, vec![vec![2], vec![3]]),
            ("2O -> 2C", Class::SelfControl, vec![vec![1]]),
            ("2O -> 2I", Class::SelfInput, vec![vec![2, 1]]),
            ("2O -> 2M", Class::SelfMechanism, vec![vec![2], vec![3]]),
        ] {
            let model = three(&format!("  arrow : {arrow} key=probe\n"));
            with_ctx(&model, |ctx| {
                let k = probe(ctx);
                assert_eq!(ctx.classes[k], class, "{arrow}");
                let got: Vec<Vec<usize>> =
                    ctx.candidates[k].iter().map(|c| c.params.clone()).collect();
                assert_eq!(got, params, "{arrow}");
            });
        }
    }

    // §6.3 rejection rules, each against a control that differs only in the tested property.
    #[test]
    fn rejections() {
        let model = three("  arrow : 1O -> 2I key=probe\n");
        with_ctx(&model, |ctx| {
            let g = &ctx.geo;
            let (c, h0) = (g.m.clear, g.h(0));
            let x = g.v(1);
            assert!(h0 >= 8, "top margin");
            let rej =
                |path: &[(i32, i32)], chan: &[usize], own: &[usize], placed: &[(usize, &Route)]| {
                    rejected(path, chan, own, g, placed, None, &ctx.edges)
                };
            // terminal segments at least `clear` long
            assert!(!rej(
                &[(x, h0), (x + c, h0), (x + c, h0 - 8)],
                &[],
                &[],
                &[]
            ));
            assert!(rej(
                &[(x, h0), (x + c - 2, h0), (x + c - 2, h0 - 8)],
                &[],
                &[],
                &[]
            ));
            // interior segments fit two fillets
            let f = 2 * g.m.r_fillet / SU_PER_GU;
            let interior = |d: i32| [(x, h0 - 8), (x, h0), (x + d, h0), (x + d, h0 - 8)];
            assert!(!rej(&interior(f), &[], &[], &[]));
            assert!(rej(&interior(f - 2), &[], &[], &[]));
            // `clear` from every other box's keep-out region
            let k = g.p.keep_out[1];
            let beside = |d: i32| [(k.x - d, k.y), (k.x - d, k.y + k.h)];
            assert!(!rej(&beside(c), &[], &[], &[]));
            assert!(rej(&beside(c - 2), &[], &[], &[]));
            // a terminal segment may touch its own box, but not enter it
            let (xs, ys) = g.port(0, Side::O, 1);
            assert!(!rej(&[(xs, ys), (xs + 8, ys)], &[], &[0], &[]));
            assert!(rej(&[(xs, ys), (xs - 8, ys)], &[], &[0], &[]));
            assert!(
                rej(&[(xs, ys), (xs + 8, ys)], &[], &[], &[]),
                "not its own box"
            );
            // no collinear overlap with another route's fixed segments; channel runs are free
            let other = fake(ctx, 0, vec![(x, h0), (x + 40, h0)], vec![]);
            let run = fake(
                ctx,
                0,
                vec![(x, h0), (x + 40, h0)],
                vec![Leg {
                    channel: Channel::H(0),
                    span: (x, x + 40),
                    seg: 0,
                }],
            );
            let mine = [(x + 10, h0), (x + 30, h0)];
            assert!(!rej(&mine, &[], &[], &[]));
            assert!(rej(&mine, &[], &[], &[(0, &other)]));
            assert!(!rej(&mine, &[], &[], &[(0, &run)]));
            assert!(!rej(&mine, &[0], &[], &[(0, &other)]), "a channel run");
        });
    }

    // §6.3 cost terms: 40 per crossing, 25 per track beyond three in a channel.
    #[test]
    fn crossing_and_overflow_costs() {
        let model = three(
            "  arrow : 1O -> 2I key=probe\n  arrow : 2O -> 3I\n  arrow : 1O -> 3C\n  arrow : 2O -> 3M\n",
        );
        with_ctx(&model, |ctx| {
            let (h0, x) = (ctx.geo.h(0), ctx.geo.v(1));
            let k = probe(ctx);
            let others: Vec<usize> = (0..ctx.edges.len()).filter(|&j| j != k).collect();
            let cand = Candidate {
                params: vec![],
                path: vec![(x, h0), (x + 40, h0)],
                legs: vec![Leg {
                    channel: Channel::H(0),
                    span: (x, x + 40),
                    seg: 0,
                }],
                deviation: 0,
            };
            let cost = |placed: Vec<(usize, Route)>| {
                let mut chosen: Vec<Option<Route>> = vec![None; ctx.edges.len()];
                for (j, r) in placed {
                    chosen[j] = Some(r);
                }
                cost_of(&cand, k, ctx, &chosen).1
            };
            let alone = cost(vec![]);
            let cross = |dx: i32| {
                fake(
                    ctx,
                    others[0],
                    vec![(x + dx, h0 - 8), (x + dx, h0 + 8)],
                    vec![],
                )
            };
            assert_eq!(cost(vec![(others[0], cross(20))]), alone + 40);
            assert_eq!(cost(vec![(others[0], cross(60))]), alone, "no crossing");
            let run = |j: usize| {
                fake(
                    ctx,
                    j,
                    vec![],
                    vec![Leg {
                        channel: Channel::H(0),
                        span: (x + 40, x),
                        seg: 0,
                    }],
                )
            };
            let two: Vec<(usize, Route)> = others[..2].iter().map(|&j| (j, run(j))).collect();
            let three: Vec<(usize, Route)> = others.iter().map(|&j| (j, run(j))).collect();
            assert_eq!(cost(two), alone, "three tracks fit");
            assert_eq!(cost(three), alone + 25, "a fourth overflows");
        });
    }

    // §6.4: among equal costs the first candidate in parameter order wins.
    #[test]
    fn ties_go_to_parameter_order() {
        let model = three("  arrow : 3O -> 2C key=probe\n");
        with_ctx(&model, |ctx| {
            let k = probe(ctx);
            let path = ctx.candidates[k][0].path.clone();
            let legs = ctx.candidates[k][0].legs.clone();
            ctx.candidates[k] = [7, 9]
                .into_iter()
                .map(|p| Candidate {
                    params: vec![p],
                    path: path.clone(),
                    legs: legs.clone(),
                    deviation: 0,
                })
                .collect();
            let none: Vec<Option<Route>> = vec![None; ctx.edges.len()];
            assert_eq!(choose(k, ctx, &none, None).expect("route").params, vec![7]);
        });
    }

    // §6.4 order: straight boundary arrows, then internal edges (heavier bundles and
    // controls first), then dog-legs and outputs, then calls.
    #[test]
    fn routing_order() {
        let model = three("  arrow : 1O -> 2C key=probe\n");
        with_ctx(&model, |ctx| {
            let e = ctx.edges[probe(ctx)].clone();
            let keys: Vec<_> = [
                Class::BoundaryStraight,
                Class::ForwardControl,
                Class::ForwardInput,
                Class::BoundaryDogLeg,
                Class::Call,
            ]
            .into_iter()
            .map(|c| order_key(&e, c))
            .collect();
            assert!(keys.windows(2).all(|w| w[0] < w[1]), "{keys:?}");
            assert_eq!(
                order_key(&e, Class::OutputExit).0,
                order_key(&e, Class::BoundaryStraight).0,
                "a straight output exit is a matching-side boundary arrow"
            );
            let heavy = Edge {
                leaves: 3,
                ..e.clone()
            };
            assert!(
                order_key(&heavy, Class::ForwardMechanism) < order_key(&e, Class::ForwardControl)
            );
        });
    }

    // §6.4: rip-up passes run after the first while the cost falls, and never raise it.
    #[test]
    fn rip_up_passes() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/dogfood/idef0-rs.idef0"
        );
        let model = idef0_core::dsl::parse(&std::fs::read_to_string(path).expect("dogfood"))
            .model
            .expect("model");
        with_ctx(&model, |ctx| {
            let mut chosen: Vec<Option<Route>> = vec![None; ctx.edges.len()];
            for &k in &ctx.order {
                chosen[k] = choose(k, ctx, &chosen, None);
            }
            let single: i64 = chosen.iter().flatten().map(|r| r.cost).sum();
            let m = Metrics::default();
            let node = NodeId::a0();
            let placed = layout_routes(&model, &node, true, TextMetric::Mono, &m)
                .expect("A0")
                .placed;
            let diagram = &model.diagrams[&node];
            let counts = crate::place::port_counts(diagram);
            let r = route(&node, diagram, &placed.placement, &counts, &m);
            assert!((2..=PASSES).contains(&r.passes), "{}", r.passes);
            assert!(r.total_cost <= single, "{} > {single}", r.total_cost);
        });
    }
}
