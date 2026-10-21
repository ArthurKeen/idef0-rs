//! The core type check (`docs/spec/metamodel.md` §3.3, §3.6, §6.3, §7, §9, §10, §16.3;
//! validation §2.1 stage 1; roadmap M1.3).
//!
//! [`typecheck`] reports the fatal `E-*` findings a [`Model`] can contain. Some registry
//! codes cannot occur in a `Model` because its types already exclude them, so the parser
//! reports them instead (M2.1): a malformed abbrev or key (`E-ID-01`, `E-ID-05`), a
//! duplicate concept key (`E-ID-03`, concepts are a map), an invalid node digit (`E-NODE-01`,
//! [`NodeId`] rejects it) and a call statement for a missing box number (`E-CALL-01`, calls
//! live on their box).

use std::collections::{BTreeMap, BTreeSet};

use crate::diag::{
    Diagnostic, E_CALL_02, E_ID_02, E_ID_04, E_NODE_02, E_NODE_03, E_NOTE_01, E_TYPE_01, E_TYPE_02,
    E_TYPE_03, E_TYPE_04, E_TYPE_05, SubjectRef,
};
use crate::id::{NodeId, PortCode, Side};
use crate::identity::{ArrowId, StatementIds, arrow_ids, structural_ids};
use crate::model::{
    ArrowAttrs, ArrowShape, BoundaryPort, Diagram, DiagramKind, Endpoint, Model, NoteTarget, Tunnel,
};
use crate::text::{ArrowKey, BoxKey};

/// Run the type check. The result is sorted (validation §8.1) and empty for a well-typed
/// model.
pub fn typecheck(model: &Model) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    box_keys(model, &mut out);
    arrow_keys(model, &mut out);
    for (node, diagram) in &model.diagrams {
        numbering(node, diagram, &mut out);
        parent_box(model, node, &mut out);
        let ids = arrow_ids(node, diagram);
        let edges = edges(&ids, diagram);
        typing(diagram, &edges, &mut out);
        ranks(&ids, diagram, &mut out);
        calls(node, diagram, &mut out);
        notes(node, diagram, &edges, &mut out);
    }
    out.sort();
    out.dedup();
    out
}

/// One source-to-destination edge after bundle flattening (metamodel §8). `at` is the
/// branch or leaf (or the plain arrow); `src_at` and `dst_at` are the statement elements that
/// hold each endpoint, so a bad trunk endpoint is reported once, on the trunk.
struct Edge<'a> {
    at: ArrowId,
    src_at: ArrowId,
    dst_at: ArrowId,
    src: &'a Endpoint,
    dst: &'a Endpoint,
    tunnel: Option<Tunnel>,
}

/// The id of an arrow statement, or of its `part`-th branch or leaf.
fn id_at(ids: &[StatementIds], arrow: usize, part: Option<usize>) -> ArrowId {
    let statement = &ids[arrow];
    part.map_or(&statement.trunk, |p| &statement.parts[p])
        .clone()
}

/// Every edge of a diagram: a plain arrow, each fork branch (trunk source to branch
/// destination) and each join leaf (leaf source to trunk destination). A branch or leaf
/// tunnel overrides the trunk's.
fn edges<'a>(ids: &[StatementIds], diagram: &'a Diagram) -> Vec<Edge<'a>> {
    let mut out = Vec::new();
    for (i, arrow) in diagram.arrows.iter().enumerate() {
        let trunk = arrow.attrs.tunnel;
        match &arrow.shape {
            ArrowShape::Plain { src, dst } => out.push(Edge {
                at: id_at(ids, i, None),
                src_at: id_at(ids, i, None),
                dst_at: id_at(ids, i, None),
                src,
                dst,
                tunnel: trunk,
            }),
            ArrowShape::Fork { src, fork } => {
                for (j, b) in fork.iter().enumerate() {
                    out.push(Edge {
                        at: id_at(ids, i, Some(j)),
                        src_at: id_at(ids, i, None),
                        dst_at: id_at(ids, i, Some(j)),
                        src,
                        dst: &b.dst,
                        tunnel: b.attrs.tunnel.or(trunk),
                    });
                }
            }
            ArrowShape::Join { join, dst } => {
                for (j, l) in join.iter().enumerate() {
                    out.push(Edge {
                        at: id_at(ids, i, Some(j)),
                        src_at: id_at(ids, i, Some(j)),
                        dst_at: id_at(ids, i, None),
                        src: &l.src,
                        dst,
                        tunnel: l.attrs.tunnel.or(trunk),
                    });
                }
            }
        }
    }
    out
}

/// `E-ID-02`: a box key is used by more than one box, or is retired (FR-ID-03, D17). The A-1
/// lineage box is the top box shown again, so it may repeat the A-0 box's key.
fn box_keys(model: &Model, out: &mut Vec<Diagnostic>) {
    let a_minus_0 = NodeId::context(0);
    let a_minus_1 = NodeId::context(1);
    let top = model
        .diagrams
        .get(&a_minus_0)
        .and_then(|d| d.boxes.first())
        .map(|b| &b.key);
    let mut uses: BTreeMap<&BoxKey, Vec<&NodeId>> = BTreeMap::new();
    for (node, diagram) in &model.diagrams {
        for (i, b) in diagram.boxes.iter().enumerate() {
            let is_top_again = *node == a_minus_1
                && diagram.lineage.map(usize::from) == Some(i)
                && Some(&b.key) == top;
            if !is_top_again {
                uses.entry(&b.key).or_default().push(node);
            }
            if model.meta.retired.contains(&b.key) {
                out.push(Diagnostic::fatal(
                    E_ID_02,
                    SubjectRef::Box(b.key.clone()),
                    format!(
                        "box key `{}` on {node} is retired and cannot be reused (D17)",
                        b.key
                    ),
                ));
            }
        }
    }
    for (key, nodes) in uses.into_iter().filter(|(_, n)| n.len() > 1) {
        let list: Vec<String> = nodes.iter().map(ToString::to_string).collect();
        out.push(Diagnostic::fatal(
            E_ID_02,
            SubjectRef::Box(key.clone()),
            format!(
                "box key `{key}` is used by {} boxes, on {}",
                nodes.len(),
                list.join(", ")
            ),
        ));
    }
}

/// `E-ID-04`: an explicit arrow key is used more than once in the model (metamodel §3.4).
fn arrow_keys(model: &Model, out: &mut Vec<Diagnostic>) {
    let mut first: BTreeMap<&ArrowKey, ArrowId> = BTreeMap::new();
    for (node, diagram) in &model.diagrams {
        let ids = arrow_ids(node, diagram);
        let plain = structural_ids(node, diagram);
        for (i, arrow) in diagram.arrows.iter().enumerate() {
            let parts: Vec<(Option<usize>, &ArrowAttrs)> = match &arrow.shape {
                ArrowShape::Plain { .. } => vec![],
                ArrowShape::Fork { fork, .. } => fork
                    .iter()
                    .enumerate()
                    .map(|(j, b)| (Some(j), &b.attrs))
                    .collect(),
                ArrowShape::Join { join, .. } => join
                    .iter()
                    .enumerate()
                    .map(|(j, l)| (Some(j), &l.attrs))
                    .collect(),
            };
            for (part, attrs) in std::iter::once((None, &arrow.attrs)).chain(parts) {
                let Some(key) = &attrs.key else { continue };
                let here = id_at(&plain, i, part);
                if let Some(earlier) = first.get(key) {
                    out.push(Diagnostic::fatal(
                        E_ID_04,
                        SubjectRef::Edge(id_at(&ids, i, part)),
                        format!("arrow key `{key}` on {here} is already used by {earlier}"),
                    ));
                } else {
                    first.insert(key, here);
                }
            }
        }
    }
}

/// `E-NODE-02`: the boxes cannot be numbered as the diagram kind requires (metamodel §3.2,
/// §16.3): A-0 has only box 0; a decomposition has box numbers 1–7; an A-n diagram has a
/// lineage box, and only A-n diagrams have one.
fn numbering(node: &NodeId, diagram: &Diagram, out: &mut Vec<Diagnostic>) {
    let n = diagram.boxes.len();
    let mut bad = |why: String| {
        out.push(Diagnostic::fatal(
            E_NODE_02,
            SubjectRef::Diagram(node.clone()),
            why,
        ));
    };
    match DiagramKind::of(node) {
        DiagramKind::Context if n > 1 => {
            bad(format!("A-0 has only box 0, but {node} has {n} boxes"));
        }
        DiagramKind::Decomposition if n > 7 => {
            bad(format!("box numbers are 1-7, but {node} has {n} boxes"));
        }
        DiagramKind::HigherContext => match diagram.lineage.map(usize::from) {
            None => bad(format!("{node} has no lineage box")),
            Some(i) if i >= n => {
                bad(format!(
                    "lineage box index {i} is outside the {n} boxes of {node}"
                ));
            }
            Some(_) if n > 7 => bad(format!("{node} has {n} boxes; at most 7 can be numbered")),
            Some(_) => {}
        },
        _ => {}
    }
    if diagram.lineage.is_some() && DiagramKind::of(node) != DiagramKind::HigherContext {
        bad(format!("only A-n diagrams have a lineage box, not {node}"));
    }
}

/// `E-NODE-03`: a decomposition below A0 whose parent box does not exist (metamodel §16.3).
/// A0 details the A-0 top box; a missing A-0 is the rule `missing_context`, not this code.
fn parent_box(model: &Model, node: &NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(parent), Some(&k)) = (node.parent(), node.boxes().last()) else {
        return;
    };
    let boxes = model.diagrams.get(&parent).map_or(0, |d| d.boxes.len());
    if usize::from(k) > boxes {
        let why = if model.diagrams.contains_key(&parent) {
            format!("{node} details box {k} of {parent}, which has {boxes} boxes")
        } else {
            format!("{node} details a box of {parent}, which has no diagram")
        };
        out.push(Diagnostic::fatal(
            E_NODE_03,
            SubjectRef::Diagram(node.clone()),
            why,
        ));
    }
}

/// The ends an arrow may start at: `box.O`, or a boundary `I`, `C`, `M` port, coded or not
/// (metamodel §6.3).
fn legal_source(e: &Endpoint) -> bool {
    match e {
        Endpoint::Box { side, .. } => *side == Side::O,
        Endpoint::Boundary(p) => p.side != Side::O,
    }
}

/// The ends an arrow may finish at: `box.I`, `box.C`, `box.M`, or a boundary `O` port, coded
/// or not (metamodel §6.3).
fn legal_destination(e: &Endpoint) -> bool {
    match e {
        Endpoint::Box { side, .. } => *side != Side::O,
        Endpoint::Boundary(p) => p.side == Side::O,
    }
}

fn box_key(e: &Endpoint) -> Option<&BoxKey> {
    match e {
        Endpoint::Box { key, .. } => Some(key),
        Endpoint::Boundary(_) => None,
    }
}

/// `E-TYPE-01`, `-02`, `-03`, `-05`: arrow typing (FR-ARROW-01) and box-end tunnels
/// (FR-TUN-01). A boundary-to-boundary edge reports `E-TYPE-03` only.
fn typing(diagram: &Diagram, edges: &[Edge<'_>], out: &mut Vec<Diagnostic>) {
    let on_diagram: BTreeSet<&BoxKey> = diagram.boxes.iter().map(|b| &b.key).collect();
    for e in edges {
        let subject = || SubjectRef::Edge(e.at.clone());
        let (src_box, dst_box) = (box_key(e.src), box_key(e.dst));
        if src_box.is_none() && dst_box.is_none() {
            out.push(Diagnostic::fatal(
                E_TYPE_03,
                subject(),
                "an arrow cannot run from boundary to boundary",
            ));
            continue;
        }
        let at_src = SubjectRef::Edge(e.src_at.clone());
        let at_dst = SubjectRef::Edge(e.dst_at.clone());
        if let Some(k) = src_box.filter(|k| !on_diagram.contains(k)) {
            let why = format!("source box @{k} is not on this diagram");
            out.push(Diagnostic::fatal(E_TYPE_01, at_src, why));
        } else if !legal_source(e.src) {
            let why = format!("an arrow cannot start at {}", end(e.src));
            out.push(Diagnostic::fatal(E_TYPE_01, at_src, why));
        }
        if let Some(k) = dst_box.filter(|k| !on_diagram.contains(k)) {
            let why = format!("destination box @{k} is not on this diagram");
            out.push(Diagnostic::fatal(E_TYPE_02, at_dst, why));
        } else if !legal_destination(e.dst) {
            let why = format!("an arrow cannot end at {}", end(e.dst));
            out.push(Diagnostic::fatal(E_TYPE_02, at_dst, why));
        }
        let box_ends = usize::from(src_box.is_some()) + usize::from(dst_box.is_some());
        if e.tunnel == Some(Tunnel::Box) && box_ends != 1 {
            out.push(Diagnostic::fatal(
                E_TYPE_05,
                subject(),
                "`tunnel=box` needs exactly one box end; use `tunnel=src`, `dst` or `both`",
            ));
        }
    }
}

fn end(e: &Endpoint) -> String {
    match e {
        Endpoint::Box { key, side, .. } => format!("the {} side of @{key}", side_name(*side)),
        Endpoint::Boundary(p) => format!("boundary {}", port(*p)),
    }
}

fn side_name(s: Side) -> &'static str {
    match s {
        Side::I => "input",
        Side::C => "control",
        Side::O => "output",
        Side::M => "mechanism",
    }
}

fn port(p: BoundaryPort) -> String {
    match p.code {
        PortCode::Icom(n) => format!("{}{n}", p.side.letter()),
        PortCode::Border(n) => format!("[{}{n}]", p.side.letter()),
    }
}

/// `E-TYPE-04`: per box side, the ranks are 1..n with no gap or duplicate. A fork's branches
/// share its source rank and a join's leaves share its destination rank (FR-ARROW-05), so a
/// trunk end counts once.
fn ranks<'a>(ids: &[StatementIds], diagram: &'a Diagram, out: &mut Vec<Diagnostic>) {
    let on_diagram: BTreeSet<&BoxKey> = diagram.boxes.iter().map(|b| &b.key).collect();
    let mut sides: BTreeMap<(&BoxKey, Side), Vec<(u8, ArrowId)>> = BTreeMap::new();
    let mut attach = |end: &'a Endpoint, at: ArrowId| {
        if let Endpoint::Box { key, side, rank } = end
            && on_diagram.contains(key)
        {
            sides.entry((key, *side)).or_default().push((*rank, at));
        }
    };
    for (i, arrow) in diagram.arrows.iter().enumerate() {
        match &arrow.shape {
            ArrowShape::Plain { src, dst } => {
                attach(src, id_at(ids, i, None));
                attach(dst, id_at(ids, i, None));
            }
            ArrowShape::Fork { src, fork } => {
                attach(src, id_at(ids, i, None));
                for (j, b) in fork.iter().enumerate() {
                    attach(&b.dst, id_at(ids, i, Some(j)));
                }
            }
            ArrowShape::Join { join, dst } => {
                for (j, l) in join.iter().enumerate() {
                    attach(&l.src, id_at(ids, i, Some(j)));
                }
                attach(dst, id_at(ids, i, None));
            }
        }
    }
    for ((key, side), ends) in sides {
        let distinct: BTreeSet<u8> = ends.iter().map(|(r, _)| *r).collect();
        let n = distinct.len();
        let mut seen = BTreeSet::new();
        for (rank, at) in ends {
            let what = format!("@{key} {} rank {rank}", side_name(side));
            if !seen.insert(rank) {
                out.push(Diagnostic::fatal(
                    E_TYPE_04,
                    SubjectRef::Edge(at),
                    format!("{what} is already used by another arrow"),
                ));
            } else if rank == 0 || usize::from(rank) > n {
                out.push(Diagnostic::fatal(
                    E_TYPE_04,
                    SubjectRef::Edge(at),
                    format!("{what} leaves a gap: ranks on a side are 1..{n}"),
                ));
            }
        }
    }
}

/// `E-CALL-02`: the A0 top box cannot be a caller, because A0 must be decomposed
/// (FR-CALL-13). It sits on A-0, and again as the A-1 lineage box.
fn calls(node: &NodeId, diagram: &Diagram, out: &mut Vec<Diagnostic>) {
    let top = match DiagramKind::of(node) {
        DiagramKind::Context => diagram.boxes.first(),
        DiagramKind::HigherContext if *node == NodeId::context(1) => diagram
            .lineage
            .and_then(|i| diagram.boxes.get(usize::from(i))),
        _ => None,
    };
    if let Some(b) = top.filter(|b| b.call.is_some()) {
        out.push(Diagnostic::fatal(
            E_CALL_02,
            SubjectRef::Box(b.key.clone()),
            format!(
                "the top box @{} cannot have a call arrow; A0 must be decomposed",
                b.key
            ),
        ));
    }
}

/// `E-NOTE-01`: a model note points at something not on its diagram (FR-NOTE-03).
fn notes(node: &NodeId, diagram: &Diagram, edges: &[Edge<'_>], out: &mut Vec<Diagnostic>) {
    let has_box = |k: &BoxKey| diagram.boxes.iter().any(|b| &b.key == k);
    let has_call = |k: &BoxKey| {
        diagram
            .boxes
            .iter()
            .any(|b| &b.key == k && b.call.is_some())
    };
    let has_key = |k: &ArrowKey| {
        diagram.arrows.iter().any(|a| {
            a.attrs.key.as_ref() == Some(k)
                || match &a.shape {
                    ArrowShape::Plain { .. } => false,
                    ArrowShape::Fork { fork, .. } => {
                        fork.iter().any(|b| b.attrs.key.as_ref() == Some(k))
                    }
                    ArrowShape::Join { join, .. } => {
                        join.iter().any(|l| l.attrs.key.as_ref() == Some(k))
                    }
                }
        })
    };
    let uses_port = |p: &BoundaryPort| {
        edges.iter().any(|e| {
            [e.src, e.dst]
                .iter()
                .any(|end| matches!(end, Endpoint::Boundary(q) if q == p))
        })
    };
    for note in &diagram.notes {
        for target in &note.on {
            let missing = match target {
                NoteTarget::Box(k) => (!has_box(k)).then(|| format!("box @{k}")),
                NoteTarget::Call(k) => (!has_call(k)).then(|| format!("a call arrow on @{k}")),
                NoteTarget::Arrow {
                    src,
                    dst,
                    occurrence,
                } => {
                    let found = edges
                        .iter()
                        .filter(|e| e.src == src && e.dst == dst)
                        .count();
                    (usize::try_from(*occurrence).map_or(true, |o| o >= found))
                        .then(|| format!("arrow {} -> {} #{occurrence}", end(src), end(dst)))
                }
                NoteTarget::ArrowKey(k) => (!has_key(k)).then(|| format!("arrow @{k}")),
                NoteTarget::Port(p) => {
                    (!uses_port(p)).then(|| format!("boundary port {}", port(*p)))
                }
            };
            if let Some(what) = missing {
                out.push(Diagnostic::fatal(
                    E_NOTE_01,
                    SubjectRef::Diagram(node.clone()),
                    format!(
                        "note {} points at {what}, which is not on {node}",
                        note.number
                    ),
                ));
            }
        }
    }
}
