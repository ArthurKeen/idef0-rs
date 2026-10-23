//! ICOM stubs (`docs/spec/metamodel.md` §11; FR-BAL-01).
//!
//! Boundary ports are not declared: a coded port exists while an arrow uses it. The editor
//! shows each parent ICOM that the child diagram does not wire yet as an unconnected stub on
//! the child frame, so a freshly decomposed, empty child shows every expected port. The
//! balance *rule* (`icom_imbalance`) is a Mangle rule (validation §5.5).

use std::collections::BTreeSet;

use crate::id::{NodeId, PortCode, Side};
use crate::model::{ArrowShape, BoundaryPort, Endpoint, Model, Tunnel};

/// The parent ICOMs of diagram `node` that no coded boundary arrow on it uses yet, in side
/// and code order. An arrow tunneled at the parent box is hidden on the child and has no
/// stub; the call arrow is not an ICOM (FR-CALL-09). `None` when `node` has no parent box.
pub fn stubs(model: &Model, node: &NodeId) -> Option<Vec<BoundaryPort>> {
    let (parent, index) = match node {
        NodeId::Node(path) if path.is_empty() => (NodeId::context(0), 0),
        NodeId::Node(path) => (node.parent()?, usize::from(*path.last()?).checked_sub(1)?),
        NodeId::Context { .. } => return None,
    };
    let pd = model.diagrams.get(&parent)?;
    let key = &pd.boxes.get(index)?.key;
    // the parent box's ports, except those whose arrow is tunneled at the box
    let mut want: BTreeSet<(Side, u8)> = BTreeSet::new();
    for a in &pd.arrows {
        let trunk = a.attrs.tunnel;
        let mut port = |e: &Endpoint, tunnel: Option<Tunnel>, at_src: bool| {
            if let Endpoint::Box { key: k, side, rank } = e
                && k == key
            {
                let hidden = match tunnel {
                    Some(Tunnel::Box | Tunnel::Both) => true,
                    Some(Tunnel::Src) => at_src,
                    Some(Tunnel::Dst) => !at_src,
                    None => false,
                };
                if !hidden {
                    want.insert((*side, *rank));
                }
            }
        };
        match &a.shape {
            ArrowShape::Plain { src, dst } => {
                port(src, trunk, true);
                port(dst, trunk, false);
            }
            ArrowShape::Fork { src, fork } => {
                port(src, trunk, true);
                for b in fork.iter() {
                    port(&b.dst, b.attrs.tunnel.or(trunk), false);
                }
            }
            ArrowShape::Join { join, dst } => {
                for l in join.iter() {
                    port(&l.src, l.attrs.tunnel.or(trunk), true);
                }
                port(dst, trunk, false);
            }
        }
    }
    // the coded boundary ports the child already uses
    let mut have: BTreeSet<(Side, u8)> = BTreeSet::new();
    if let Some(cd) = model.diagrams.get(node) {
        for a in &cd.arrows {
            let ends: Vec<&Endpoint> = match &a.shape {
                ArrowShape::Plain { src, dst } => vec![src, dst],
                ArrowShape::Fork { src, fork } => core::iter::once(src)
                    .chain(fork.iter().map(|b| &b.dst))
                    .collect(),
                ArrowShape::Join { join, dst } => join
                    .iter()
                    .map(|l| &l.src)
                    .chain(core::iter::once(dst))
                    .collect(),
            };
            for e in ends {
                if let Endpoint::Boundary(BoundaryPort {
                    side,
                    code: PortCode::Icom(c),
                }) = e
                {
                    have.insert((*side, *c));
                }
            }
        }
    }
    Some(
        want.difference(&have)
            .map(|&(side, c)| BoundaryPort {
                side,
                code: PortCode::Icom(c),
            })
            .collect(),
    )
}
