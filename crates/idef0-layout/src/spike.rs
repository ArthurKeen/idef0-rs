//! **M4.0 layout spike, Auto-Arrange step 1** (roadmap M4.0; layout §12.2, §16): the
//! box-order search prototype, measured by ADR 0002 and kept until M4.7 builds Auto-Arrange.
//! The spike's sizing, placement and routing were replaced by [`crate::size`],
//! [`crate::place`] and [`crate::route`] (M4.1–M4.3).
//!
//! Each bundle branch or leaf counts as its own arc. All arithmetic is integer (D6).

#![allow(
    clippy::many_single_char_names,
    reason = "the search uses the spec's own names: n, i, j, k, p, w (layout §12.2)"
)]

use std::collections::BTreeMap;

use idef0_core::id::{NodeId, PortCode, Side};
use idef0_core::model::{ArrowShape, Diagram, Endpoint};
use idef0_core::text::BoxKey;

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
