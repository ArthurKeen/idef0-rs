//! Track assignment (`docs/spec/layout.md` §7; SR-41; FR-LAYOUT-11).
//!
//! Global routing (§6) leaves every channel run on its channel's center line. This module
//! puts each run on a track. It uses hard vertical-constraint (VCG) edges where two runs leave
//! a channel on opposite sides at one row or column, doglegs that break hard cycles, soft
//! barycenter edges, the constrained left-edge algorithm, and adjacent-track sifting on exact
//! crossing counts (§7.1, §7.2).
//!
//! Vertical channels are assigned first. Where a vertical run ends in a horizontal channel
//! whose tracks are not known yet, its interval covers that channel's whole band, so the
//! assignment holds wherever those tracks land. Horizontal channels then see exact columns.
//! Frame entries that lie inside a channel's band are fixed runs: they block the tracks they
//! sit on.

#![allow(
    clippy::many_single_char_names,
    reason = "channel geometry uses the spec's own names: a, b, c, i, j, k, m, p, r, s, t"
)]

use std::collections::BTreeMap;

use crate::metrics::{LATTICE_X, LATTICE_Y, Metrics};
use crate::place::{Placement, Tracks};
use crate::route::{Channel, EdgeKey, Route, Routing};

type Point = (i32, i32);

/// The side of its channel through which a run's end leaves (§7): toward lower coordinates
/// (`Near`: left in a vertical channel, up in a horizontal one) or higher ones (`Far`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exit {
    /// Toward the lower coordinate.
    Near,
    /// Toward the higher coordinate.
    Far,
}

/// One channel segment after assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The route's edge.
    pub key: EdgeKey,
    /// Index of the route in [`Routing::routes`].
    pub route: usize,
    /// Index of the leg in the route's legs.
    pub leg: usize,
    /// 0 for a whole leg; 1 and 2 for the two halves of a dogleg-split leg.
    pub part: u8,
    /// Low end of its interval on the channel axis.
    pub lo: i32,
    /// High end of its interval on the channel axis.
    pub hi: i32,
    /// Its track: 0 is nearest the lower coordinate (the lower-numbered box, §7.1).
    pub track: u32,
}

/// The assignment of one channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelAssignment {
    /// The channel.
    pub channel: Channel,
    /// Its segments.
    pub segments: Vec<Segment>,
    /// Hard VCG edges `(a, b)`, indices into `segments`: `a` takes a lower track than `b`.
    pub hard: Vec<(usize, usize)>,
    /// Dogleg splits: route index, leg index, and the jog's coordinate on the channel axis.
    pub doglegs: Vec<(usize, usize, i32)>,
    /// Hard cycles that no free row or column could break, so one segment's hard edges were
    /// dropped. Zero on every fixture.
    pub dropped: u32,
    /// The most segments that pairwise cannot share a track.
    pub density: u32,
    /// The longest chain of hard edges, in segments.
    pub hard_path: u32,
    /// Tracks used, counting the tracks that fixed runs (frame entries in the band) occupy.
    pub tracks: u32,
}

/// Track assignment of a [`Routing`]: the final polylines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wiring {
    /// Per route, aligned with [`Routing::routes`]: the orthogonal polyline with every run on
    /// its track, every vertex on the lattice, no zero-length or collinear-interior segments.
    pub paths: Vec<Vec<Point>>,
    /// Channels `V_0 … V_n`, then `H_0 … H_n`.
    pub channels: Vec<ChannelAssignment>,
    /// Tracks per channel, for the gap sizing of §5.2.
    pub tracks: Tracks,
    /// Exact crossings between non-sibling routes (§10.1).
    pub crossings: usize,
}

/// Where a channel run ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct End {
    /// The end's exact coordinate on the channel axis; `None` while it floats in a
    /// perpendicular channel whose tracks are not assigned yet.
    at: Option<i32>,
    /// The perpendicular channel's band, for a floating end.
    band: Option<(i32, i32)>,
    /// The side the neighboring segment leaves through.
    exit: Option<Exit>,
    /// Where the neighboring segment goes on the orthogonal axis (for barycenters).
    toward: Option<i32>,
}

#[derive(Debug, Clone)]
struct Item {
    route: usize,
    leg: usize,
    part: u8,
    lo: i32,
    hi: i32,
    ends: [End; 2],
    key: (EdgeKey, usize, u8),
}

impl Item {
    fn bary(&self) -> Option<(i64, i64)> {
        let v: Vec<i64> = self
            .ends
            .iter()
            .filter_map(|e| e.toward.map(i64::from))
            .collect();
        (!v.is_empty()).then(|| (v.iter().sum(), i64::try_from(v.len()).unwrap_or(1)))
    }
}

/// A fixed run inside the band: a frame entry that runs along the channel.
#[derive(Debug, Clone, Copy)]
struct Fixed {
    coord: i32,
    lo: i32,
    hi: i32,
    ends: [End; 2],
}

/// A leg's coordinate after assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placed {
    Whole(i32),
    /// The two halves' coordinates and the jog's coordinate on the channel axis.
    Split(i32, i32, i32),
}

struct Chan {
    channel: Channel,
    vertical: bool,
    band: (i32, i32),
    pitch: i32,
    lattice: i32,
    clear: i32,
}

impl Chan {
    fn new(p: &Placement, m: &Metrics, channel: Channel) -> Self {
        match channel {
            Channel::V(c) => Self {
                channel,
                vertical: true,
                band: p.v_channel(c),
                pitch: m.pitch_x,
                lattice: LATTICE_Y,
                clear: m.clear,
            },
            Channel::H(r) => Self {
                channel,
                vertical: false,
                band: p.h_channel(r),
                pitch: m.pitch_y,
                lattice: LATTICE_X,
                clear: m.clear,
            },
        }
    }

    /// §7.2 step 3: `start + clear + t·pitch`, on the lattice by construction.
    fn coord(&self, t: u32) -> i32 {
        self.band.0 + self.clear + i32::try_from(t).unwrap_or(i32::MAX / 2) * self.pitch
    }

    /// The coordinate along the channel.
    fn axis(&self, p: Point) -> i32 {
        if self.vertical { p.1 } else { p.0 }
    }

    /// The coordinate across the channel.
    fn ortho(&self, p: Point) -> i32 {
        if self.vertical { p.0 } else { p.1 }
    }

    /// Whether a segment runs along this channel inside its band.
    fn holds(&self, a: Point, b: Point) -> bool {
        let along = if self.vertical {
            a.0 == b.0
        } else {
            a.1 == b.1
        };
        let o = self.ortho(a);
        along && a != b && self.band.0 <= o && o < self.band.1
    }
}

fn band_of(p: &Placement, ch: Channel) -> (i32, i32) {
    match ch {
        Channel::V(c) => p.v_channel(c),
        Channel::H(r) => p.h_channel(r),
    }
}

fn perpendicular(a: Channel, b: Channel) -> bool {
    matches!(
        (a, b),
        (Channel::V(_), Channel::H(_)) | (Channel::H(_), Channel::V(_))
    )
}

/// The end of route `ri`'s segment at path vertex `v`, whose neighbor is segment `nb`.
fn end_at(
    r: &Route,
    ri: usize,
    ch: &Chan,
    v: usize,
    nb: Option<usize>,
    placed: &BTreeMap<(usize, usize), Placed>,
    p: &Placement,
) -> End {
    let here = r.path[v];
    let Some(s) = nb.filter(|&s| s + 1 < r.path.len()) else {
        return End {
            at: Some(ch.axis(here)),
            band: None,
            exit: None,
            toward: None,
        };
    };
    let other = if s == v { r.path[s + 1] } else { r.path[s] };
    let o = ch.ortho(other);
    let exit = match o.cmp(&ch.ortho(here)) {
        core::cmp::Ordering::Less => Some(Exit::Near),
        core::cmp::Ordering::Greater => Some(Exit::Far),
        core::cmp::Ordering::Equal => None,
    };
    let leg = r
        .legs
        .iter()
        .position(|l| l.seg == s && perpendicular(l.channel, ch.channel));
    let (at, band) = match leg {
        Some(li) => match placed.get(&(ri, li)) {
            // the neighbor is an assigned run: its track is this end's coordinate
            Some(Placed::Whole(c)) => (Some(*c), None),
            Some(Placed::Split(c0, c1, _)) => (Some(if s == v { *c0 } else { *c1 }), None),
            None => (None, Some(band_of(p, r.legs[li].channel))),
        },
        None => (Some(ch.axis(here)), None),
    };
    End {
        at,
        band,
        exit,
        toward: Some(o),
    }
}

fn interval(ends: &[End; 2]) -> (i32, i32) {
    let mut v = Vec::with_capacity(4);
    for e in ends {
        match (e.at, e.band) {
            (Some(a), _) => v.push(a),
            (None, Some((a, b))) => v.extend([a, b]),
            (None, None) => {}
        }
    }
    (
        v.iter().copied().min().unwrap_or(0),
        v.iter().copied().max().unwrap_or(0),
    )
}

/// The constraint between two ends (§7.1): opposite exits at one exact coordinate. `Some(true)`
/// means the first end's run takes the lower track.
fn ordered(a: End, b: End) -> Option<bool> {
    match (a.at, b.at, a.exit, b.exit) {
        (Some(x), Some(y), Some(ea), Some(eb)) if x == y && ea != eb => Some(ea == Exit::Near),
        _ => None,
    }
}

fn hard_edges(items: &[Item]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for i in 0..items.len() {
        for j in i + 1..items.len() {
            for &a in &items[i].ends {
                for &b in &items[j].ends {
                    match ordered(a, b) {
                        Some(true) => out.push((i, j)),
                        Some(false) => out.push((j, i)),
                        None => {}
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// One directed cycle of `edges` over `n` nodes, if any (deterministic DFS from node 0).
fn find_cycle(n: usize, edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    fn dfs(
        u: usize,
        succ: &[Vec<usize>],
        color: &mut [u8],
        stack: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        color[u] = 1;
        stack.push(u);
        for &w in &succ[u] {
            if color[w] == 1 {
                let at = stack.iter().position(|&x| x == w).unwrap_or(0);
                return Some(stack[at..].to_vec());
            }
            if color[w] == 0
                && let Some(c) = dfs(w, succ, color, stack)
            {
                return Some(c);
            }
        }
        stack.pop();
        color[u] = 2;
        None
    }
    let mut succ = vec![Vec::new(); n];
    for &(a, b) in edges {
        succ[a].push(b);
    }
    let mut color = vec![0u8; n];
    let mut stack: Vec<usize> = Vec::new();
    (0..n).find_map(|u| {
        if color[u] == 0 {
            dfs(u, &succ, &mut color, &mut stack)
        } else {
            None
        }
    })
}

fn reaches(n: usize, edges: &[(usize, usize)], from: usize, to: usize) -> bool {
    let mut seen = vec![false; n];
    let mut todo = vec![from];
    while let Some(u) = todo.pop() {
        if u == to {
            return true;
        }
        if !std::mem::replace(&mut seen[u], true) {
            todo.extend(edges.iter().filter(|e| e.0 == u).map(|e| e.1));
        }
    }
    false
}

/// The longest chain of `edges` (an acyclic graph), counted in nodes; 1 with no edges.
fn longest_path(n: usize, edges: &[(usize, usize)]) -> u32 {
    let mut indeg = vec![0usize; n];
    for &(_, b) in edges {
        indeg[b] += 1;
    }
    let mut depth = vec![1u32; n];
    let mut todo: Vec<usize> = (0..n).filter(|&u| indeg[u] == 0).collect();
    while let Some(u) = todo.pop() {
        for &(a, b) in edges {
            if a == u {
                depth[b] = depth[b].max(depth[u] + 1);
                indeg[b] -= 1;
                if indeg[b] == 0 {
                    todo.push(b);
                }
            }
        }
    }
    depth.into_iter().max().unwrap_or(0)
}

/// Two intervals that cannot share a track: closer than `clear`.
fn conflict(a: (i32, i32), b: (i32, i32), clear: i32) -> bool {
    a.0 < b.1 + clear && b.0 < a.1 + clear
}

/// The most intervals that pairwise conflict (they are intervals, so the most that share a
/// point once each is widened by `clear`).
fn density(spans: &[(i32, i32)], clear: i32) -> u32 {
    let mut ev: Vec<(i32, i32)> = spans
        .iter()
        .flat_map(|&(a, b)| [(a, 1), (b + clear, -1)])
        .collect();
    ev.sort_unstable_by_key(|&(x, d)| (x, d));
    let (mut cur, mut best) = (0i32, 0i32);
    for (_, d) in ev {
        cur += d;
        best = best.max(cur);
    }
    u32::try_from(best).unwrap_or(0)
}

/// Lattice coordinates that collide with a jog in this channel: any route segment that
/// crosses the band perpendicular to the channel, at its coordinate on the channel axis.
fn occupied(ch: &Chan, paths: &[Vec<Point>]) -> Vec<i32> {
    let mut out = Vec::new();
    for path in paths {
        for w in path.windows(2) {
            let across = if ch.vertical {
                w[0].1 == w[1].1
            } else {
                w[0].0 == w[1].0
            };
            if !across || w[0] == w[1] {
                continue;
            }
            let (o0, o1) = (
                ch.ortho(w[0]).min(ch.ortho(w[1])),
                ch.ortho(w[0]).max(ch.ortho(w[1])),
            );
            if o0 < ch.band.1 && ch.band.0 < o1 {
                out.push(ch.axis(w[0]));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// A free coordinate strictly inside `it`, away from its ends, from every exact end in the
/// channel, from every perpendicular segment through the band and from every perpendicular
/// band (whose tracks could land there): the jog of a dogleg (§7.1).
fn split_point(
    it: &Item,
    ch: &Chan,
    taken: &[i32],
    bands: &[(i32, i32)],
    min_len: i32,
) -> Option<i32> {
    let reach = |e: &End, low: bool| match (e.at, e.band) {
        (Some(a), _) => Some(a),
        (None, Some((a, b))) => Some(if low { b } else { a }),
        (None, None) => None,
    };
    let rep = |e: &End| e.at.or(e.band.map(|(a, b)| i32::midpoint(a, b)));
    let (r0, r1) = (rep(&it.ends[0])?, rep(&it.ends[1])?);
    let (low, high) = if r0 <= r1 {
        (&it.ends[0], &it.ends[1])
    } else {
        (&it.ends[1], &it.ends[0])
    };
    let a = reach(low, true)? + min_len;
    let b = reach(high, false)? - min_len;
    let first = -crate::route::snap_down(-a, ch.lattice);
    let mid = i32::midpoint(a, b);
    let mut best: Option<i32> = None;
    let mut c = first;
    while c <= b {
        let free = !taken.contains(&c) && !bands.iter().any(|&(x, y)| x <= c && c < y);
        if free && best.is_none_or(|q: i32| (c - mid).abs() < (q - mid).abs()) {
            best = Some(c);
        }
        c += ch.lattice;
    }
    best
}

/// Split a leg at `at` into two halves joined by a jog.
fn split(it: &Item, at: i32) -> [Item; 2] {
    let jog = End {
        at: Some(at),
        band: None,
        exit: None,
        toward: None,
    };
    let mk = |part: u8, ends: [End; 2]| {
        let (lo, hi) = interval(&ends);
        Item {
            route: it.route,
            leg: it.leg,
            part,
            lo,
            hi,
            ends,
            key: (it.key.0.clone(), it.key.1, part),
        }
    };
    [mk(1, [it.ends[0], jog]), mk(2, [jog, it.ends[1]])]
}

/// Every route's polyline with the assigned legs moved onto their tracks.
fn build_path(
    r: &Route,
    ri: usize,
    vertical: &[bool],
    placed: &BTreeMap<(usize, usize), Placed>,
) -> Vec<Point> {
    let mut pts = r.path.clone();
    let mut jogs: BTreeMap<usize, (Point, Point)> = BTreeMap::new();
    for (li, leg) in r.legs.iter().enumerate() {
        let Some(&pl) = placed.get(&(ri, li)) else {
            continue;
        };
        let s = leg.seg;
        if s + 1 >= pts.len() {
            continue;
        }
        let v = vertical[li];
        let set = |p: &mut Point, c: i32| {
            if v {
                p.0 = c;
            } else {
                p.1 = c;
            }
        };
        match pl {
            Placed::Whole(c) => {
                set(&mut pts[s], c);
                set(&mut pts[s + 1], c);
            }
            Placed::Split(c0, c1, at) => {
                set(&mut pts[s], c0);
                set(&mut pts[s + 1], c1);
                let (j0, j1) = if v {
                    ((c0, at), (c1, at))
                } else {
                    ((at, c0), (at, c1))
                };
                jogs.insert(s, (j0, j1));
            }
        }
    }
    let mut out = Vec::with_capacity(pts.len() + 2 * jogs.len());
    for (k, &q) in pts.iter().enumerate() {
        out.push(q);
        if let Some(&(j0, j1)) = jogs.get(&k) {
            out.extend([j0, j1]);
        }
    }
    simplify(out)
}

/// Drop repeated vertices and the middle of collinear runs.
fn simplify(pts: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for q in pts {
        if out.last() == Some(&q) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            if (a.0 == b.0 && b.0 == q.0) || (a.1 == b.1 && b.1 == q.1) {
                out.pop();
            }
        }
        out.push(q);
    }
    out
}

fn siblings(a: &Route, b: &Route) -> bool {
    a.bundle.is_some() && a.bundle == b.bundle
}

/// Exact crossings (§10.1) between non-sibling routes.
pub fn crossings(routes: &[Route], paths: &[Vec<Point>]) -> usize {
    let mut n = 0;
    for i in 0..paths.len() {
        for j in i + 1..paths.len() {
            if !siblings(&routes[i], &routes[j]) {
                n += crate::route::crossings(&paths[i], &paths[j]);
            }
        }
    }
    n
}

/// Crossings that involve any of `touched`.
fn crossings_of(routes: &[Route], paths: &[Vec<Point>], touched: &[usize]) -> usize {
    let mut n = 0;
    for (i, path) in paths.iter().enumerate() {
        for &j in touched {
            let counted_from_i = touched.contains(&i) && i < j;
            if i != j && !counted_from_i && !siblings(&routes[i], &routes[j]) {
                n += crate::route::crossings(path, &paths[j]);
            }
        }
    }
    n
}

/// §7.1 soft barycenter edges, added to `hard` by decreasing separation (ties by key): for
/// each pair of segments that cannot share a track, the one with the smaller barycenter goes
/// lower. An edge is skipped if it closes a cycle or makes the longest chain exceed `limit`.
fn soft_edges(
    items: &[Item],
    hard: &[(usize, usize)],
    clear: i32,
    limit: u32,
) -> Vec<(usize, usize)> {
    let k = items.len();
    let mut edges = hard.to_vec();
    let mut soft: Vec<(i128, usize, usize)> = Vec::new();
    for i in 0..k {
        for j in i + 1..k {
            if !conflict(
                (items[i].lo, items[i].hi),
                (items[j].lo, items[j].hi),
                clear,
            ) || hard
                .iter()
                .any(|&(a, b)| (a, b) == (i, j) || (a, b) == (j, i))
            {
                continue;
            }
            let (Some((si, ni)), Some((sj, nj))) = (items[i].bary(), items[j].bary()) else {
                continue;
            };
            let d = i128::from(si) * i128::from(nj) - i128::from(sj) * i128::from(ni);
            if d == 0 {
                continue;
            }
            // separation |bary_i − bary_j| = |d| / (ni·nj), compared on a common scale
            let sep = d.abs() * 720 / (i128::from(ni) * i128::from(nj));
            if d < 0 {
                soft.push((sep, i, j));
            } else {
                soft.push((sep, j, i));
            }
        }
    }
    soft.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| items[a.1].key.cmp(&items[b.1].key))
            .then_with(|| items[a.2].key.cmp(&items[b.2].key))
    });
    for (_, a, b) in soft {
        if reaches(k, &edges, b, a) {
            continue;
        }
        edges.push((a, b));
        if longest_path(k, &edges) > limit {
            edges.pop();
        }
    }
    edges
}

/// The lowest coordinate each segment may take: past every fixed run whose near side it
/// leaves through at the fixed run's own end coordinate.
fn min_bounds(items: &[Item], fixed: &[Fixed]) -> Vec<Option<i32>> {
    items
        .iter()
        .map(|it| {
            fixed
                .iter()
                .filter(|f| {
                    it.ends
                        .iter()
                        .any(|&a| f.ends.iter().any(|&b| ordered(a, b) == Some(false)))
                })
                .map(|f| f.coord)
                .max()
        })
        .collect()
}

/// Whether a fixed run blocks track `t` for `it`: within one pitch and conflicting intervals.
fn blocks(ch: &Chan, fixed: &[Fixed], t: u32, it: &Item) -> bool {
    let c = ch.coord(t);
    fixed
        .iter()
        .any(|f| (c - f.coord).abs() < ch.pitch && conflict((it.lo, it.hi), (f.lo, f.hi), ch.clear))
}

/// §7.2 step 1, the constrained left-edge algorithm: for `t = 0, 1, …`, repeatedly place the
/// unplaced segment with the smallest start that fits after the track's last occupant and
/// whose VCG predecessors all sit on lower tracks (ties by barycenter, then key).
fn left_edge(
    ch: &Chan,
    items: &[Item],
    edges: &[(usize, usize)],
    min_coord: &[Option<i32>],
    fixed: &[Fixed],
) -> Vec<u32> {
    let k = items.len();
    let bary_key = |i: usize| {
        items[i]
            .bary()
            .map(|(s, n)| (i128::from(s) * 720) / i128::from(n))
    };
    let mut track: Vec<Option<u32>> = vec![None; k];
    let mut left = k;
    let mut t = 0u32;
    let guard = u32::try_from(2 * k + fixed.len() + 8).unwrap_or(u32::MAX);
    while left > 0 {
        let mut last: Option<i32> = None;
        loop {
            let pick = (0..k)
                .filter(|&i| track[i].is_none())
                .filter(|&i| {
                    t >= guard
                        || (edges
                            .iter()
                            .filter(|e| e.1 == i)
                            .all(|e| track[e.0].is_some_and(|tp| tp < t))
                            && min_coord[i].is_none_or(|mn| ch.coord(t) > mn)
                            && !blocks(ch, fixed, t, &items[i]))
                })
                .filter(|&i| last.is_none_or(|h| items[i].lo >= h + ch.clear))
                .min_by_key(|&i| (items[i].lo, bary_key(i), items[i].key.clone()));
            let Some(i) = pick else { break };
            track[i] = Some(t);
            last = Some(items[i].hi);
            left -= 1;
        }
        t += 1;
    }
    track.into_iter().map(|x| x.unwrap_or(0)).collect()
}

/// §5.2: the previous iteration's tracks for a channel, when the new assignment needs more
/// tracks (`needed`), the previous one covers the same segments without doglegs, and it is
/// still feasible on the new coordinates.
fn previous_tracks(
    prev: Option<&ChannelAssignment>,
    items: &[Item],
    split: bool,
    needed: u32,
    feasible: &dyn Fn(&[u32]) -> bool,
) -> Option<Vec<u32>> {
    let prev = prev?;
    let same = prev.segments.len() == items.len()
        && prev
            .segments
            .iter()
            .zip(items)
            .all(|(s, it)| (&s.key, s.leg, s.part) == (&it.key.0, it.leg, it.part));
    if split || !prev.doglegs.is_empty() || prev.tracks >= needed || !same {
        return None;
    }
    let old: Vec<u32> = prev.segments.iter().map(|s| s.track).collect();
    feasible(&old).then_some(old)
}

struct Assigner<'a> {
    routes: &'a [Route],
    p: &'a Placement,
    m: &'a Metrics,
    /// Per route, per leg: whether the leg is in a vertical channel.
    vertical: Vec<Vec<bool>>,
    placed: BTreeMap<(usize, usize), Placed>,
    paths: Vec<Vec<Point>>,
}

impl Assigner<'_> {
    fn items(&self, ch: &Chan) -> (Vec<Item>, Vec<Fixed>) {
        let mut items = Vec::new();
        let mut fixed = Vec::new();
        for (ri, r) in self.routes.iter().enumerate() {
            let leg_at = |s: usize| r.legs.iter().position(|l| l.seg == s);
            for s in 0..r.path.len().saturating_sub(1) {
                let ends = [
                    end_at(r, ri, ch, s, s.checked_sub(1), &self.placed, self.p),
                    end_at(r, ri, ch, s + 1, Some(s + 1), &self.placed, self.p),
                ];
                match leg_at(s) {
                    Some(li) if r.legs[li].channel == ch.channel => {
                        let (lo, hi) = interval(&ends);
                        items.push(Item {
                            route: ri,
                            leg: li,
                            part: 0,
                            lo,
                            hi,
                            ends,
                            key: (r.key.clone(), li, 0),
                        });
                    }
                    None if ch.holds(r.path[s], r.path[s + 1]) => {
                        let (lo, hi) = interval(&ends);
                        fixed.push(Fixed {
                            coord: ch.ortho(r.path[s]),
                            lo,
                            hi,
                            ends,
                        });
                    }
                    _ => {}
                }
            }
        }
        (items, fixed)
    }

    /// Bands of the perpendicular channels: a jog there could meet their tracks.
    fn cross_bands(&self, ch: &Chan) -> Vec<(i32, i32)> {
        let n = self.p.boxes.len();
        (0..=n)
            .map(|k| {
                if ch.vertical {
                    self.p.h_channel(k)
                } else {
                    self.p.v_channel(k)
                }
            })
            .collect()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the §7.1–§7.2 steps in order, one channel"
    )]
    fn channel(&mut self, ch: &Chan, prev: Option<&ChannelAssignment>) -> ChannelAssignment {
        let (mut items, fixed) = self.items(ch);
        // §7.1 cycles: split the longest segment of a hard cycle at a free coordinate
        let mut doglegs = Vec::new();
        let mut dropped = 0;
        let mut taken: Vec<i32> = occupied(ch, &self.paths);
        for it in &items {
            taken.extend(it.ends.iter().filter_map(|e| e.at));
        }
        for f in &fixed {
            taken.extend(f.ends.iter().filter_map(|e| e.at));
        }
        let bands = self.cross_bands(ch);
        let min_len = 2 * self.m.r_fillet / crate::metrics::SU_PER_GU;
        for _ in 0..=2 * items.len() {
            let hard = hard_edges(&items);
            let Some(cycle) = find_cycle(items.len(), &hard) else {
                break;
            };
            let victim = cycle
                .iter()
                .copied()
                .max_by(|&a, &b| {
                    (items[a].hi - items[a].lo)
                        .cmp(&(items[b].hi - items[b].lo))
                        .then_with(|| items[b].key.cmp(&items[a].key))
                })
                .unwrap_or(cycle[0]);
            let at = (items[victim].part == 0)
                .then(|| split_point(&items[victim], ch, &taken, &bands, min_len))
                .flatten();
            if let Some(at) = at {
                let [a, b] = split(&items[victim], at);
                doglegs.push((a.route, a.leg, at));
                taken.push(at);
                items.splice(victim..=victim, [a, b]);
            } else {
                for e in &mut items[victim].ends {
                    e.exit = None;
                }
                dropped += 1;
            }
        }
        let k = items.len();
        let hard = hard_edges(&items);
        let spans: Vec<(i32, i32)> = items.iter().map(|it| (it.lo, it.hi)).collect();
        let dens = density(&spans, ch.clear);
        let hard_path = if k == 0 { 0 } else { longest_path(k, &hard) };
        let edges = soft_edges(&items, &hard, ch.clear, dens.max(hard_path));
        let min_coord = min_bounds(&items, &fixed);
        let blocked = |t: u32, it: &Item| blocks(ch, &fixed, t, it);
        let mut track = left_edge(ch, &items, &edges, &min_coord, &fixed);
        // §5.2: keep the previous assignment when the new one needs more tracks
        let used = |track: &[u32]| {
            let mine = track.iter().map(|&t| t + 1).max().unwrap_or(0);
            let pinned = fixed
                .iter()
                .filter(|f| f.coord >= ch.band.0 + ch.clear)
                .map(|f| {
                    let d = f.coord - ch.band.0 - ch.clear;
                    u32::try_from((d + ch.pitch - 1) / ch.pitch + 1).unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            mine.max(pinned)
        };
        // an assignment is feasible when it keeps the hard VCG, the fixed runs and the
        // clearance between runs that share a track
        let feasible = |track: &[u32]| {
            hard.iter().all(|&(a, b)| track[a] < track[b])
                && (0..k).all(|i| {
                    min_coord[i].is_none_or(|mn| ch.coord(track[i]) > mn)
                        && !blocked(track[i], &items[i])
                        && (i + 1..k).all(|j| {
                            track[i] != track[j] || !conflict(spans[i], spans[j], ch.clear)
                        })
                })
        };
        if let Some(old) =
            previous_tracks(prev, &items, !doglegs.is_empty(), used(&track), &feasible)
        {
            track = old;
        }
        // §7.2 step 2: sifting on exact crossings
        self.sift(ch, &items, &mut track, &feasible);
        apply(&mut self.placed, ch, &items, &track);
        let mut touched: Vec<usize> = items.iter().map(|it| it.route).collect();
        touched.sort_unstable();
        touched.dedup();
        for &ri in &touched {
            self.paths[ri] = build_path(&self.routes[ri], ri, &self.vertical[ri], &self.placed);
        }
        ChannelAssignment {
            channel: ch.channel,
            segments: items
                .iter()
                .zip(&track)
                .map(|(it, &t)| Segment {
                    key: it.key.0.clone(),
                    route: it.route,
                    leg: it.leg,
                    part: it.part,
                    lo: it.lo,
                    hi: it.hi,
                    track: t,
                })
                .collect(),
            hard,
            doglegs,
            dropped,
            density: dens,
            hard_path,
            tracks: used(&track),
        }
    }

    /// §7.2 step 2: adjacent-track swaps that respect the hard VCG, the fixed runs and the
    /// clearance, kept when the exact crossing count falls; at most `4·m` tries, in order.
    fn sift(
        &self,
        ch: &Chan,
        items: &[Item],
        track: &mut [u32],
        feasible: &dyn Fn(&[u32]) -> bool,
    ) {
        let k = items.len();
        let tracks = track.iter().copied().max().map_or(0, |t| t + 1);
        if k < 2 || tracks < 2 {
            return;
        }
        let mut budget = 4 * k;
        let eval = |track: &[u32], this: &Self| -> usize {
            let mut placed = this.placed.clone();
            let mut paths = this.paths.clone();
            apply(&mut placed, ch, items, track);
            let mut touched: Vec<usize> = items.iter().map(|it| it.route).collect();
            touched.sort_unstable();
            touched.dedup();
            for &ri in &touched {
                paths[ri] = build_path(&this.routes[ri], ri, &this.vertical[ri], &placed);
            }
            crossings_of(this.routes, &paths, &touched)
        };
        let mut best = eval(track, self);
        let mut improved = true;
        while improved && budget > 0 {
            improved = false;
            for t in 0..tracks - 1 {
                if budget == 0 {
                    break;
                }
                budget -= 1;
                let swapped: Vec<u32> = track
                    .iter()
                    .map(|&x| {
                        if x == t {
                            t + 1
                        } else if x == t + 1 {
                            t
                        } else {
                            x
                        }
                    })
                    .collect();
                if !feasible(&swapped) {
                    continue;
                }
                let x = eval(&swapped, self);
                if x < best {
                    best = x;
                    track.copy_from_slice(&swapped);
                    improved = true;
                }
            }
        }
    }
}

/// Record `items` on `track` as leg coordinates. A split leg's first half carries the jog.
fn apply(placed: &mut BTreeMap<(usize, usize), Placed>, ch: &Chan, items: &[Item], track: &[u32]) {
    for (it, &t) in items.iter().zip(track) {
        let c = ch.coord(t);
        match it.part {
            1 => {
                let at = it.ends[1].at.unwrap_or(0);
                placed.insert((it.route, it.leg), Placed::Split(c, c, at));
            }
            2 => {
                if let Some(Placed::Split(_, c1, _)) = placed.get_mut(&(it.route, it.leg)) {
                    *c1 = c;
                }
            }
            _ => {
                placed.insert((it.route, it.leg), Placed::Whole(c));
            }
        }
    }
}

/// Assign tracks in every channel of `routing` on `placement` (§7). `prev` is the previous
/// placement iteration's assignment: a channel keeps it when the new one needs more tracks
/// (§5.2).
pub fn assign(
    routing: &Routing,
    placement: &Placement,
    m: &Metrics,
    prev: Option<&Wiring>,
) -> Wiring {
    let n = placement.boxes.len();
    let routes = &routing.routes;
    let mut a = Assigner {
        routes,
        p: placement,
        m,
        vertical: routes
            .iter()
            .map(|r| {
                r.legs
                    .iter()
                    .map(|l| matches!(l.channel, Channel::V(_)))
                    .collect()
            })
            .collect(),
        placed: BTreeMap::new(),
        paths: routes.iter().map(|r| r.path.clone()).collect(),
    };
    let order: Vec<Channel> = (0..=n)
        .map(Channel::V)
        .chain((0..=n).map(Channel::H))
        .collect();
    let mut channels = Vec::with_capacity(order.len());
    for ch in order {
        let c = Chan::new(placement, m, ch);
        let before = prev.and_then(|w| w.channels.iter().find(|x| x.channel == ch));
        channels.push(a.channel(&c, before));
    }
    let paths: Vec<Vec<Point>> = routes
        .iter()
        .enumerate()
        .map(|(ri, r)| build_path(r, ri, &a.vertical[ri], &a.placed))
        .collect();
    let mut tracks = Tracks::none(n);
    for c in &channels {
        match c.channel {
            Channel::V(i) => tracks.v[i] = c.tracks,
            Channel::H(i) => tracks.h[i] = c.tracks,
        }
    }
    Wiring {
        crossings: crossings(routes, &paths),
        paths,
        channels,
        tracks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::layout_routes;
    use crate::text::TextMetric;
    use idef0_core::id::NodeId;

    fn wired(arrows: &str) -> crate::route::Routed {
        let src = format!(
            "idef0 0.2\nmodel \"T\" {{\n  abbrev \"T\"\n}}\ndiagram A0 {{\n  box 1 \"Plan Work\"\n  box 2 \"Do Work\"\n  box 3 \"Check Work\"\n{arrows}}}\n"
        );
        let p = idef0_core::dsl::parse(&src);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let model = p.model.expect("model");
        layout_routes(
            &model,
            &NodeId::a0(),
            true,
            TextMetric::Mono,
            &Metrics::default(),
        )
        .expect("A0")
    }

    fn end(at: i32, exit: Exit) -> End {
        End {
            at: Some(at),
            band: None,
            exit: Some(exit),
            toward: None,
        }
    }

    // §7.1: opposite exits at one coordinate order the runs; the one leaving toward the near
    // side takes the lower track. Same side, or different coordinates, give no edge.
    #[test]
    fn hard_constraints() {
        assert_eq!(ordered(end(8, Exit::Near), end(8, Exit::Far)), Some(true));
        assert_eq!(ordered(end(8, Exit::Far), end(8, Exit::Near)), Some(false));
        assert_eq!(ordered(end(8, Exit::Near), end(8, Exit::Near)), None);
        assert_eq!(ordered(end(8, Exit::Near), end(12, Exit::Far)), None);
        let floating = End {
            at: None,
            band: Some((0, 20)),
            ..end(8, Exit::Far)
        };
        assert_eq!(
            ordered(end(8, Exit::Near), floating),
            None,
            "no exact coordinate"
        );
    }

    // Graph helpers: cycles, reachability and the longest chain (in nodes).
    #[test]
    fn graph_helpers() {
        assert_eq!(find_cycle(3, &[(0, 1), (1, 2)]), None);
        let c = find_cycle(3, &[(0, 1), (1, 2), (2, 1)]).expect("cycle");
        assert_eq!(c, vec![1, 2]);
        assert!(reaches(3, &[(0, 1), (1, 2)], 0, 2));
        assert!(!reaches(3, &[(0, 1), (1, 2)], 2, 0));
        assert_eq!(longest_path(4, &[]), 1);
        assert_eq!(longest_path(4, &[(0, 1), (1, 2), (0, 3)]), 3);
    }

    // Density counts runs that cannot share a track: overlapping or closer than `clear`.
    #[test]
    fn density_and_conflict() {
        assert!(conflict((0, 10), (13, 20), 4), "3 apart: too close");
        assert!(!conflict((0, 10), (14, 20), 4), "4 apart: one track");
        assert_eq!(density(&[(0, 10), (14, 20), (30, 40)], 4), 1);
        assert_eq!(
            density(&[(0, 10), (12, 20), (16, 40)], 4),
            2,
            "(0,10) and (16,40) share"
        );
        assert_eq!(density(&[(0, 10), (12, 20), (13, 40)], 4), 3);
        assert_eq!(density(&[], 4), 0);
    }

    // Path rebuild: repeated vertices and collinear middles go.
    #[test]
    fn simplify_paths() {
        assert_eq!(
            simplify(vec![(0, 0), (0, 0), (4, 0), (8, 0), (8, 4)]),
            vec![(0, 0), (8, 0), (8, 4)]
        );
    }

    // §7.1 cycles: two runs that swap rows in V_0 form a hard cycle; the longer is split by
    // a dogleg whose halves sit on either side of the other run.
    #[test]
    fn dogleg_breaks_a_hard_cycle() {
        let x = wired("  arrow : I1 -> 3I1 key=probe\n  arrow : I2 -> 1I1\n");
        let v0 = &x.wiring.channels[0];
        assert_eq!(v0.channel, Channel::V(0));
        assert_eq!(v0.doglegs.len(), 1, "{v0:?}");
        assert_eq!(v0.dropped, 0);
        let half = |p: u8| {
            v0.segments
                .iter()
                .find(|s| s.part == p)
                .expect("half")
                .track
        };
        let whole = v0
            .segments
            .iter()
            .find(|s| s.part == 0)
            .expect("whole")
            .track;
        let (a, b) = (half(1).min(half(2)), half(1).max(half(2)));
        assert!(a < whole && whole < b, "{v0:?}");
        assert_eq!(x.wiring.crossings, 1, "the jog crosses the other run once");
    }

    // §7.2: runs that do not conflict share a track (left-edge packing).
    #[test]
    fn left_edge_packs_tracks() {
        // two feedback controls into box 1 and box 2 from box 3 leave V_3 at different rows
        // and turn into different H channels: one track holds both
        let x = wired("  arrow : 3O1 -> 1C1\n  arrow : 1O1 -> 2I1\n  arrow : 2O1 -> 3I1\n");
        for c in &x.wiring.channels {
            assert!(
                c.tracks <= u32::try_from(c.segments.len()).unwrap_or(0).max(1),
                "{c:?}"
            );
        }
        let dense = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/bench/dense-7x40.idef0"
        ))
        .expect("fixture");
        let model = idef0_core::dsl::parse(&dense).model.expect("model");
        let w = layout_routes(
            &model,
            &NodeId::a0(),
            true,
            TextMetric::Mono,
            &Metrics::default(),
        )
        .expect("A0")
        .wiring;
        let segments: usize = w.channels.iter().map(|c| c.segments.len()).sum();
        let tracks: u32 = w.channels.iter().map(|c| c.tracks).sum();
        assert!(
            usize::try_from(tracks).unwrap_or(usize::MAX) < segments,
            "{tracks} tracks for {segments} segments: some share"
        );
    }

    // §7.2 step 2: nested feedback into one box. The barycenter order alone puts the inner
    // loop on the upper track and crosses twice; sifting swaps the tracks to cross none.
    #[test]
    fn sifting_untangles_nested_feedback() {
        let x = wired("  arrow : 3O1 -> 1C1\n  arrow : 2O1 -> 1C2\n");
        assert_eq!(x.wiring.crossings, 0, "{:?}", x.wiring.paths);
        let h0 = x
            .wiring
            .channels
            .iter()
            .find(|c| c.channel == Channel::H(0))
            .expect("H_0");
        let track = |from: &str| {
            h0.segments
                .iter()
                .find(|s| s.key.to_string().starts_with(from))
                .expect("segment")
                .track
        };
        assert!(
            track("A0:3O1") < track("A0:2O1"),
            "the outer loop runs above"
        );
    }

    fn key(i: usize) -> (EdgeKey, usize, u8) {
        let k: idef0_core::text::BoxKey = format!("s{i}").parse().expect("a key");
        (EdgeKey::Call(k), 0, 0)
    }

    /// A synthetic segment `i` on `[lo, hi]` whose exits go toward `toward`.
    fn item(i: usize, lo: i32, hi: i32, toward: Option<i32>) -> Item {
        let e = |at: i32| End {
            at: Some(at),
            band: None,
            exit: None,
            toward,
        };
        Item {
            route: i,
            leg: 0,
            part: 0,
            lo,
            hi,
            ends: [e(lo), e(hi)],
            key: key(i),
        }
    }

    /// A vertical channel whose tracks sit at 104, 110, 116, …
    fn chan() -> Chan {
        Chan {
            channel: Channel::V(1),
            vertical: true,
            band: (100, 140),
            pitch: 6,
            lattice: 4,
            clear: 4,
        }
    }

    // §7.1: soft edges point from the smaller barycenter to the larger, and stop before the
    // longest chain passes max(density, hard path).
    #[test]
    fn soft_edges_by_barycenter_within_the_limit() {
        let items = [
            item(0, 0, 10, Some(1)),
            item(1, 8, 20, Some(2)),
            item(2, 18, 30, Some(3)),
        ];
        assert_eq!(
            soft_edges(&items, &[], 4, 2),
            vec![(0, 1)],
            "a third chain link is too long"
        );
        assert_eq!(soft_edges(&items, &[], 4, 3), vec![(0, 1), (1, 2)]);
        let far = [item(0, 0, 10, Some(1)), item(1, 40, 50, Some(2))];
        assert_eq!(
            soft_edges(&far, &[], 4, 2),
            vec![],
            "no edge between runs that can share"
        );
        let back = [item(0, 0, 10, Some(5)), item(1, 8, 20, Some(2))];
        assert_eq!(soft_edges(&back, &[], 4, 2), vec![(1, 0)]);
    }

    // §7.2 step 1: predecessors sit on lower tracks, a track's occupants keep `clear`
    // between them, a fixed-run bound keeps a segment past it, and fixed runs block tracks
    // within one pitch.
    #[test]
    fn left_edge_rules() {
        let ch = chan();
        let two = [item(0, 0, 10, None), item(1, 40, 50, None)];
        assert_eq!(
            left_edge(&ch, &two, &[], &[None, None], &[]),
            vec![0, 0],
            "they share"
        );
        assert_eq!(
            left_edge(&ch, &two, &[(0, 1)], &[None, None], &[]),
            vec![0, 1]
        );
        let tight = [item(0, 0, 10, None), item(1, 12, 20, None)];
        assert_eq!(
            left_edge(&ch, &tight, &[], &[None, None], &[]),
            vec![0, 1],
            "2 gu apart"
        );
        let spaced = [item(0, 0, 10, None), item(1, 14, 20, None)];
        assert_eq!(left_edge(&ch, &spaced, &[], &[None, None], &[]), vec![0, 0]);
        let one = [item(0, 0, 10, None)];
        assert_eq!(
            left_edge(&ch, &one, &[], &[Some(104)], &[]),
            vec![1],
            "past 104"
        );
        let fixed = Fixed {
            coord: 106,
            lo: 0,
            hi: 40,
            ends: [end(0, Exit::Near), end(40, Exit::Near)],
        };
        assert_eq!(
            left_edge(&ch, &one, &[], &[None], &[fixed]),
            vec![2],
            "104 and 110 blocked"
        );
        let clear_of_it = Fixed {
            lo: 60,
            hi: 80,
            ..fixed
        };
        assert_eq!(left_edge(&ch, &one, &[], &[None], &[clear_of_it]), vec![0]);
    }

    // A segment that leaves toward the far side at a fixed run's end coordinate, where the
    // fixed run leaves toward the near side, must take a track past it.
    #[test]
    fn fixed_run_bounds() {
        let f = Fixed {
            coord: 110,
            lo: 0,
            hi: 20,
            ends: [end(0, Exit::Far), end(20, Exit::Near)],
        };
        let mut far = item(0, 20, 40, None);
        far.ends[0] = end(20, Exit::Far);
        assert_eq!(min_bounds(&[far], &[f]), vec![Some(110)]);
        let mut near = item(0, 20, 40, None);
        near.ends[0] = end(20, Exit::Near);
        assert_eq!(min_bounds(&[near], &[f]), vec![None]);
    }

    // §7.1 dogleg point: the free lattice coordinate nearest the middle, at least the minimum
    // length from both ends, off taken coordinates and outside perpendicular bands.
    #[test]
    fn dogleg_split_point() {
        let ch = chan();
        let it = item(0, 0, 40, None);
        assert_eq!(split_point(&it, &ch, &[], &[], 4), Some(20));
        assert_eq!(
            split_point(&it, &ch, &[], &[(16, 24)], 4),
            Some(24),
            "the band is not free"
        );
        assert_eq!(split_point(&it, &ch, &[20, 24], &[], 4), Some(16));
        assert_eq!(
            split_point(&item(0, 0, 8, None), &ch, &[4], &[], 4),
            None,
            "nothing free"
        );
    }

    fn route(path: Vec<Point>, bundle: Option<usize>, i: usize) -> Route {
        Route {
            key: key(i).0,
            class: crate::route::Class::ForwardControl,
            params: vec![],
            path,
            legs: vec![],
            cost: 0,
            forced: false,
            bundle,
        }
    }

    // §10.1: crossings between siblings of one bundle are connections, not crossings.
    #[test]
    fn sibling_crossings_do_not_count() {
        let (a, b) = (vec![(0, 10), (40, 10)], vec![(20, 0), (20, 30)]);
        let plain = [route(a.clone(), None, 0), route(b.clone(), None, 1)];
        assert_eq!(crossings(&plain, &[a.clone(), b.clone()]), 1);
        let bundle = [route(a.clone(), Some(3), 0), route(b.clone(), Some(3), 1)];
        assert_eq!(crossings(&bundle, &[a.clone(), b.clone()]), 0);
        let other = [route(a.clone(), Some(3), 0), route(b.clone(), Some(4), 1)];
        assert_eq!(crossings(&other, &[a, b]), 1);
    }

    // §5.2: a channel keeps the previous iteration's assignment only when the new one needs
    // more tracks, the segments match, neither split a leg, and the old tracks are feasible.
    #[test]
    fn previous_assignment_rule() {
        let items = [item(0, 0, 10, None), item(1, 40, 50, None)];
        let prev = ChannelAssignment {
            channel: Channel::V(1),
            segments: items
                .iter()
                .map(|it| Segment {
                    key: it.key.0.clone(),
                    route: it.route,
                    leg: it.leg,
                    part: it.part,
                    lo: it.lo,
                    hi: it.hi,
                    track: 0,
                })
                .collect(),
            hard: vec![],
            doglegs: vec![],
            dropped: 0,
            density: 1,
            hard_path: 1,
            tracks: 1,
        };
        let yes = |_: &[u32]| true;
        let no = |_: &[u32]| false;
        assert_eq!(
            previous_tracks(Some(&prev), &items, false, 2, &yes),
            Some(vec![0, 0])
        );
        assert_eq!(
            previous_tracks(Some(&prev), &items, false, 1, &yes),
            None,
            "no more tracks"
        );
        assert_eq!(
            previous_tracks(Some(&prev), &items, false, 2, &no),
            None,
            "infeasible"
        );
        assert_eq!(
            previous_tracks(Some(&prev), &items, true, 2, &yes),
            None,
            "a new dogleg"
        );
        assert_eq!(previous_tracks(None, &items, false, 2, &yes), None);
        let other = [item(0, 0, 10, None), item(2, 40, 50, None)];
        assert_eq!(
            previous_tracks(Some(&prev), &other, false, 2, &yes),
            None,
            "other segments"
        );
        let split = ChannelAssignment {
            doglegs: vec![(0, 0, 4)],
            ..prev.clone()
        };
        assert_eq!(
            previous_tracks(Some(&split), &items, false, 2, &yes),
            None,
            "an old dogleg"
        );
    }
}
