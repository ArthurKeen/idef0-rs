//! Commands, patches and undo (`docs/spec/metamodel.md` §13; architecture §4.3; FR-EDIT).
//!
//! Editing happens only through [`Command`]s. [`apply`] is a pure function: it takes the
//! model, a command and an [`Env`] (the key minter; no clock, no randomness, FR-EDIT-01) and
//! returns the new model and a [`Patch`]. A patch holds the forward [`Op`]s, their inverse
//! and the [`RenameMap`]. An op replaces one diagram, one concept or the metadata, so undo is
//! exact by construction. A command that would add an `E-*` finding is rejected
//! ([`CommandError::Invalid`]), so validity is preserved.

mod draft;
pub mod history;
mod refs;

use std::collections::BTreeMap;
use std::sync::Arc;

use nonempty::NonEmpty;

use crate::diag::Diagnostic;
use crate::id::{NodeId, PortCode, PortRef, Side};
use crate::identity::{ArrowId, KeyMinter, arrow_ids};
use crate::model::{
    Arrow, ArrowAttrs, ArrowShape, BoundaryPort, Concept, ConceptRef, Diagram, Endpoint,
    FunctionBox, Meta, Model, NoteTarget, Status,
};
use crate::text::{BoxKey, ConceptKey, Description, NonEmptyString};

use draft::{Change, Draft, ranks_on};

/// A new arrow end: an existing endpoint, or a new port on a box side, which takes the next
/// rank there (FR-EDIT-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndSpec {
    /// A new port at the end of `side` of box `key`.
    NewPort {
        /// The box.
        key: BoxKey,
        /// The side.
        side: Side,
    },
    /// An existing endpoint: a boundary port, or a box port by rank.
    At(Endpoint),
}

/// Which end of an arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The source.
    Src,
    /// The destination.
    Dst,
}

/// What a description belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Described {
    /// A box, by key.
    Box(BoxKey),
    /// A diagram.
    Diagram(NodeId),
}

/// A copied box (FR-EDIT-13): the box, and its subtree when copied with it. Subtree diagrams
/// are keyed by their box path below the copied box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    /// The box: name, description and call arrow.
    pub root: FunctionBox,
    /// The subtree: diagrams by path below the box (`[]` is the box's own child).
    pub subtree: Vec<(Vec<u8>, Diagram)>,
}

/// An edit (metamodel §13). Boxes are named by key, so commands survive renumbering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Insert a box at position `at` (1-based) on a decomposition diagram; later boxes shift
    /// up with their subtrees (FR-EDIT-03).
    AddBox {
        /// The diagram.
        diagram: NodeId,
        /// The new box's number.
        at: u8,
        /// Its name.
        name: NonEmptyString,
        /// Its description.
        description: String,
    },
    /// Rename a box; its key stays (FR-EDIT-11).
    RenameBox {
        /// The box.
        key: BoxKey,
        /// The new name.
        name: NonEmptyString,
    },
    /// Move a box to position `to` on its diagram, renumbering (FR-EDIT-06).
    MoveBox {
        /// The box.
        key: BoxKey,
        /// Its new number.
        to: u8,
    },
    /// Delete a box with the §13.1 cascade (FR-EDIT-05).
    DeleteBox {
        /// The box.
        key: BoxKey,
    },
    /// Delete a child diagram and its descendants; the parent box becomes a leaf and keeps
    /// its ports (FR-EDIT-10).
    DeleteDiagram {
        /// The diagram.
        diagram: NodeId,
    },
    /// Add a plain arrow.
    AddArrow {
        /// The diagram.
        diagram: NodeId,
        /// Its source.
        src: EndSpec,
        /// Its destination.
        dst: EndSpec,
        /// The concept its label refers to.
        concept: Option<ConceptRef>,
    },
    /// Delete an arrow statement, or one branch or leaf of a bundle (FR-EDIT-05 steps 3, 8).
    DeleteArrow {
        /// The diagram.
        diagram: NodeId,
        /// The arrow, branch or leaf.
        id: ArrowId,
    },
    /// Move one end of an arrow, branch or leaf (FR-EDIT-19).
    Retarget {
        /// The diagram.
        diagram: NodeId,
        /// The arrow, branch or leaf.
        id: ArrowId,
        /// Which end.
        end: End,
        /// The new end.
        to: EndSpec,
    },
    /// Reorder a box side's ports: `order[i]` is the old rank that becomes rank `i + 1`. A
    /// decomposed box's child boundary is re-coded with the same permutation (FR-EDIT-07).
    ReorderPorts {
        /// The box.
        key: BoxKey,
        /// The side.
        side: Side,
        /// The old ranks in their new order.
        order: Vec<u8>,
    },
    /// Set a box or diagram description (a minor change).
    SetDescription {
        /// The box or diagram.
        subject: Described,
        /// The new text.
        text: String,
    },
    /// Set a diagram's status explicitly.
    SetStatus {
        /// The diagram.
        diagram: NodeId,
        /// The status.
        status: Status,
    },
    /// Paste a copied box at position `at` with fresh keys (FR-EDIT-13).
    Paste {
        /// The diagram.
        diagram: NodeId,
        /// The pasted box's number.
        at: u8,
        /// What was copied.
        clip: Clip,
    },
}

/// What a command needs besides the model: the key minter (FR-EDIT-01).
pub struct Env<'a> {
    /// Mints keys for new boxes.
    pub minter: &'a mut dyn KeyMinter,
}

impl core::fmt::Debug for Env<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Env").finish_non_exhaustive()
    }
}

/// Why a command was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// No such diagram.
    #[error("the model has no diagram {0}")]
    NoDiagram(NodeId),
    /// No such box.
    #[error("the model has no box @{0}")]
    NoBox(BoxKey),
    /// No such arrow on the diagram.
    #[error("diagram {0} has no arrow {1}")]
    NoArrow(NodeId, ArrowId),
    /// The command is not allowed here.
    #[error("{0}")]
    Rejected(String),
    /// The result would not type-check.
    #[error("the command would make the model invalid ({} finding(s))", .0.len())]
    Invalid(Vec<Diagnostic>),
}

fn reject<T>(msg: impl Into<String>) -> Result<T, CommandError> {
    Err(CommandError::Rejected(msg.into()))
}

/// One invertible mutation: replace (or remove, with `None`) one element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// A diagram.
    Diagram {
        /// Its node.
        node: NodeId,
        /// The new diagram, or `None` to remove it.
        value: Option<Arc<Diagram>>,
    },
    /// A concept.
    Concept {
        /// Its key.
        key: ConceptKey,
        /// The new concept, or `None` to remove it.
        value: Option<Box<Concept>>,
    },
    /// The model metadata (including retired keys).
    Meta(Box<Meta>),
}

/// Every identifier a patch changed (FR-EDIT-02): old → new, `None` for removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenameMap {
    /// Node numbers.
    pub nodes: BTreeMap<NodeId, Option<NodeId>>,
    /// Ports, coded and uncoded.
    pub ports: BTreeMap<PortRef, Option<PortRef>>,
    /// Arrow ids.
    pub arrows: BTreeMap<ArrowId, Option<ArrowId>>,
    /// Box keys (`@k → ∅` for removed boxes).
    pub boxes: BTreeMap<BoxKey, Option<BoxKey>>,
}

/// The result of applying commands (architecture §4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// The originating commands, kept for rebase.
    pub commands: Vec<Command>,
    /// The forward ops.
    pub ops: Vec<Op>,
    /// The inverse ops: applying them to the result restores the input.
    pub inverse: Vec<Op>,
    /// The identifier changes.
    pub rename: RenameMap,
    /// The diagram the change is about: in the input and in the result.
    pub primary: (NodeId, NodeId),
}

/// Apply `ops` to `model`.
pub fn apply_ops(model: &Model, ops: &[Op]) -> Model {
    let mut m = model.clone();
    for op in ops {
        match op {
            Op::Diagram { node, value } => match value {
                Some(d) => {
                    m.diagrams.insert(node.clone(), d.clone());
                }
                None => {
                    m.diagrams.remove(node);
                }
            },
            Op::Concept { key, value } => match value {
                Some(c) => {
                    m.concepts.insert(key.clone(), (**c).clone());
                }
                None => {
                    m.concepts.remove(key);
                }
            },
            Op::Meta(meta) => m.meta = (**meta).clone(),
        }
    }
    m
}

/// Apply one command (FR-EDIT-01).
///
/// # Errors
///
/// [`CommandError`] when the command does not apply or would make the model invalid.
pub fn apply(
    model: &Model,
    cmd: &Command,
    env: &mut Env<'_>,
) -> Result<(Model, Patch), CommandError> {
    let mut d = Draft::new(model, primary_of(model, cmd));
    run(&mut d, cmd, env)?;
    let (m, mut patch) = d.finish()?;
    patch.commands = vec![cmd.clone()];
    Ok((m, patch))
}

/// Apply commands as one compound change: one Patch, one undo entry (metamodel §13.6).
///
/// # Errors
///
/// The first command's [`CommandError`]; nothing is applied.
pub fn apply_all(
    model: &Model,
    cmds: &[Command],
    env: &mut Env<'_>,
) -> Result<(Model, Patch), CommandError> {
    let mut current = model.clone();
    let mut rename = RenameMap::default();
    let mut first = None;
    let mut last = NodeId::a0();
    for c in cmds {
        let (next, p) = apply(&current, c, env)?;
        compose(&mut rename, p.rename);
        first.get_or_insert(p.primary.0);
        last = p.primary.1;
        current = next;
    }
    let (ops, inverse) = draft::diff(model, &current);
    let first = first.unwrap_or_else(|| last.clone());
    Ok((
        current,
        Patch {
            commands: cmds.to_vec(),
            ops,
            inverse,
            rename,
            primary: (first, last),
        },
    ))
}

/// `a` then `b`: chain renames, so each original identifier maps to its final one.
fn compose(a: &mut RenameMap, b: RenameMap) {
    fn chain<K: Ord + Clone>(a: &mut BTreeMap<K, Option<K>>, b: BTreeMap<K, Option<K>>) {
        let mut b = b;
        for v in a.values_mut() {
            if let Some(k) = v.clone()
                && let Some(next) = b.remove(&k)
            {
                *v = next;
            }
        }
        for (k, v) in b {
            a.entry(k).or_insert(v);
        }
        a.retain(|k, v| v.as_ref() != Some(k));
    }
    chain(&mut a.nodes, b.nodes);
    chain(&mut a.ports, b.ports);
    chain(&mut a.arrows, b.arrows);
    chain(&mut a.boxes, b.boxes);
}

/// Copy box `key`, with its subtree when `subtree` is set (FR-EDIT-13).
///
/// # Errors
///
/// [`CommandError::NoBox`] if there is no such box.
pub fn copy(model: &Model, key: &BoxKey, subtree: bool) -> Result<Clip, CommandError> {
    let (node, i) = locate(model, key)?;
    let root = model.diagrams[&node].boxes[i].clone();
    let mut tree = Vec::new();
    if subtree && let Some(child) = detail_node(&node, i) {
        for (n, d) in &model.diagrams {
            if matches!(n, NodeId::Node(_))
                && let Some(rest) = n.boxes().strip_prefix(child.boxes())
            {
                tree.push((rest.to_vec(), (**d).clone()));
            }
        }
    }
    Ok(Clip {
        root,
        subtree: tree,
    })
}

fn primary_of(model: &Model, cmd: &Command) -> NodeId {
    let of_key = |k: &BoxKey| locate(model, k).map_or_else(|_| NodeId::a0(), |(n, _)| n);
    match cmd {
        Command::AddBox { diagram, .. }
        | Command::DeleteDiagram { diagram }
        | Command::AddArrow { diagram, .. }
        | Command::DeleteArrow { diagram, .. }
        | Command::Retarget { diagram, .. }
        | Command::SetStatus { diagram, .. }
        | Command::Paste { diagram, .. } => diagram.clone(),
        Command::SetDescription { subject, .. } => match subject {
            Described::Box(k) => of_key(k),
            Described::Diagram(n) => n.clone(),
        },
        Command::RenameBox { key, .. }
        | Command::MoveBox { key, .. }
        | Command::DeleteBox { key }
        | Command::ReorderPorts { key, .. } => of_key(key),
    }
}

/// The diagram and index of box `key`: its occurrence on a decomposition diagram, else on
/// A-0 (the A-1 lineage box shares the A-0 box's key).
fn locate(model: &Model, key: &BoxKey) -> Result<(NodeId, usize), CommandError> {
    let mut found = None;
    for (n, d) in &model.diagrams {
        if let Some(i) = d.boxes.iter().position(|b| &b.key == key) {
            match n {
                NodeId::Node(_) => return Ok((n.clone(), i)),
                NodeId::Context { level: 0 } => found = Some((n.clone(), i)),
                NodeId::Context { .. } => {
                    found.get_or_insert((n.clone(), i));
                }
            }
        }
    }
    found.ok_or_else(|| CommandError::NoBox(key.clone()))
}

/// The node box `i` of diagram `node` details.
fn detail_node(node: &NodeId, i: usize) -> Option<NodeId> {
    match node {
        NodeId::Context { level: 0 } => Some(NodeId::a0()),
        NodeId::Context { .. } => None,
        NodeId::Node(_) => u8::try_from(i + 1).ok().and_then(|k| node.child(k)),
    }
}

fn decomposition(node: &NodeId) -> Result<(), CommandError> {
    match node {
        NodeId::Node(_) => Ok(()),
        NodeId::Context { .. } => reject(format!(
            "{node} is a context diagram: its boxes are fixed by the levels below"
        )),
    }
}

/// Renumber the subtrees under diagram `parent` by `f` on the first box digit below it.
fn renumber_under(d: &mut Draft<'_>, parent: &NodeId, f: &dyn Fn(u8) -> u8) {
    let depth = parent.boxes().len();
    let p = parent.clone();
    d.renumber(&move |n: &NodeId| {
        let path = n.boxes();
        if matches!(n, NodeId::Node(_)) && path.len() > depth && path.starts_with(p.boxes()) {
            let mut v = path.to_vec();
            v[depth] = f(v[depth]);
            NodeId::from_boxes(&v)
        } else {
            None
        }
    });
}

fn run(d: &mut Draft<'_>, cmd: &Command, env: &mut Env<'_>) -> Result<(), CommandError> {
    match cmd {
        Command::AddBox {
            diagram,
            at,
            name,
            description,
        } => {
            let key = mint(d, env, name.as_str());
            let mut b = FunctionBox::new(key, name.clone());
            b.description = Description::new(description.clone());
            insert_box(d, diagram, *at, b)
        }
        Command::RenameBox { key, name } => rename_box(d, key, name),
        Command::MoveBox { key, to } => move_box(d, key, *to),
        Command::DeleteBox { key } => delete_box(d, key),
        Command::DeleteDiagram { diagram } => delete_diagram(d, diagram),
        Command::AddArrow {
            diagram,
            src,
            dst,
            concept,
        } => {
            let src = resolve_end(d, diagram, src)?;
            let dst = resolve_end(d, diagram, dst)?;
            let (dg, origins) = d.edit(diagram)?;
            dg.arrows.push(Arrow {
                attrs: ArrowAttrs {
                    concept: concept.clone(),
                    ..ArrowAttrs::default()
                },
                shape: ArrowShape::Plain { src, dst },
            });
            origins.push(None);
            d.mark(diagram, Change::Major);
            Ok(())
        }
        Command::DeleteArrow { diagram, id } => {
            let at = find_arrow(d, diagram, id)?;
            let lost = remove_at(d, diagram, at)?;
            compact(d, diagram, &lost)?;
            d.mark(diagram, Change::Major);
            Ok(())
        }
        Command::Retarget {
            diagram,
            id,
            end,
            to,
        } => retarget(d, diagram, id, *end, to),
        Command::ReorderPorts { key, side, order } => reorder(d, key, *side, order),
        Command::SetDescription { subject, text } => {
            match subject {
                Described::Box(key) => {
                    let (node, i) = locate(&d.model, key)?;
                    let (dg, _) = d.edit(&node)?;
                    dg.boxes[i].description = Description::new(text.clone());
                    d.mark(&node, Change::Minor);
                }
                Described::Diagram(node) => {
                    let (dg, _) = d.edit(node)?;
                    dg.description = Description::new(text.clone());
                    d.mark(node, Change::Minor);
                }
            }
            Ok(())
        }
        Command::SetStatus { diagram, status } => {
            let (dg, _) = d.edit(diagram)?;
            dg.status = *status;
            Ok(())
        }
        Command::Paste { diagram, at, clip } => paste(d, diagram, *at, clip, env),
    }
}

fn mint(d: &Draft<'_>, env: &mut Env<'_>, name: &str) -> BoxKey {
    let taken = |k: &BoxKey| {
        d.model.meta.retired.contains(k)
            || d.model
                .diagrams
                .values()
                .any(|dg| dg.boxes.iter().any(|b| &b.key == k))
    };
    env.minter.box_key(name, &taken)
}

/// Insert `b` at position `at` on decomposition diagram `diagram` (FR-EDIT-03).
fn insert_box(
    d: &mut Draft<'_>,
    diagram: &NodeId,
    at: u8,
    b: FunctionBox,
) -> Result<(), CommandError> {
    decomposition(diagram)?;
    let n = d.diagram(diagram)?.boxes.len();
    let max = usize::from(d.model.meta.profile.max_box_number());
    if n >= max {
        return reject(format!(
            "{diagram} already has the profile maximum of {max} boxes"
        ));
    }
    if at == 0 || usize::from(at) > n + 1 {
        return reject(format!("box number {at} is outside 1..={}", n + 1));
    }
    renumber_under(d, diagram, &|k| if k >= at { k + 1 } else { k });
    let (dg, _) = d.edit(diagram)?;
    dg.boxes.insert(usize::from(at) - 1, b);
    d.mark(diagram, Change::Major);
    Ok(())
}

fn rename_box(d: &mut Draft<'_>, key: &BoxKey, name: &NonEmptyString) -> Result<(), CommandError> {
    let (node, i) = locate(&d.model, key)?;
    let old = d.diagram(&node)?.boxes[i].name.clone();
    // every occurrence: the A-1 lineage box shares the A-0 box's key
    let at: Vec<(NodeId, usize)> = d
        .model
        .diagrams
        .iter()
        .flat_map(|(n, dg)| {
            dg.boxes
                .iter()
                .enumerate()
                .filter(|(_, b)| &b.key == key)
                .map(|(j, _)| (n.clone(), j))
                .collect::<Vec<_>>()
        })
        .collect();
    for (n, j) in at {
        let (dg, _) = d.edit(&n)?;
        dg.boxes[j].name = name.clone();
        d.mark(&n, Change::Major);
    }
    // a stated child title must equal the box name (FR-DIAG-05): it follows the rename
    if let Some(child) = detail_node(&node, i)
        && d.diagram(&child)
            .is_ok_and(|c| c.title.as_ref() == Some(&old))
    {
        let (c, _) = d.edit(&child)?;
        c.title = Some(name.clone());
    }
    Ok(())
}

fn move_box(d: &mut Draft<'_>, key: &BoxKey, to: u8) -> Result<(), CommandError> {
    let (node, i) = locate(&d.model, key)?;
    decomposition(&node)?;
    let n = d.diagram(&node)?.boxes.len();
    if to == 0 || usize::from(to) > n {
        return reject(format!("box number {to} is outside 1..={n}"));
    }
    let from = u8::try_from(i + 1).unwrap_or(u8::MAX);
    if from == to {
        return Ok(());
    }
    renumber_under(d, &node, &|k| {
        if k == from {
            to
        } else if from < to && k > from && k <= to {
            k - 1
        } else if to < from && k >= to && k < from {
            k + 1
        } else {
            k
        }
    });
    let (dg, _) = d.edit(&node)?;
    let b = dg.boxes.remove(i);
    dg.boxes.insert(usize::from(to) - 1, b);
    d.mark(&node, Change::Major);
    Ok(())
}

/// Where an arrow id points: a statement, or a branch or leaf of one.
#[derive(Debug, Clone, Copy)]
struct At {
    stmt: usize,
    part: Option<usize>,
}

fn find_arrow(d: &Draft<'_>, diagram: &NodeId, id: &ArrowId) -> Result<At, CommandError> {
    let dg = d.diagram(diagram)?;
    for (stmt, ids) in arrow_ids(diagram, dg).iter().enumerate() {
        if &ids.trunk == id {
            return Ok(At { stmt, part: None });
        }
        if let Some(p) = ids.parts.iter().position(|x| x == id) {
            return Ok(At {
                stmt,
                part: Some(p),
            });
        }
    }
    Err(CommandError::NoArrow(diagram.clone(), id.clone()))
}

/// The box sides that lost an end, for rank compaction.
type Lost = Vec<(BoxKey, Side)>;

fn box_side(e: &Endpoint) -> Option<(BoxKey, Side)> {
    match e {
        Endpoint::Box { key, side, .. } => Some((key.clone(), *side)),
        Endpoint::Boundary(_) => None,
    }
}

/// Remove a statement, or one branch or leaf; a bundle left with one part collapses into a
/// plain arrow, and one left with none goes (metamodel §13.1 step 2).
fn remove_at(d: &mut Draft<'_>, diagram: &NodeId, at: At) -> Result<Lost, CommandError> {
    let (dg, origins) = d.edit(diagram)?;
    let mut lost = Lost::new();
    let Some(p) = at.part else {
        let a = dg.arrows.remove(at.stmt);
        origins.remove(at.stmt);
        lost.extend(all_ends(&a.shape).into_iter().filter_map(box_side));
        return Ok(lost);
    };
    let arrow = &mut dg.arrows[at.stmt];
    let origin = &mut origins[at.stmt];
    let collapsed = match &mut arrow.shape {
        ArrowShape::Fork { src, fork } => {
            let mut parts: Vec<_> = fork.iter().cloned().collect();
            let b = parts.remove(p);
            lost.extend(box_side(&b.dst));
            if let Some(o) = origin {
                o.parts.remove(p);
            }
            if parts.len() == 1 {
                let only = parts.remove(0);
                Some((
                    merge(&arrow.attrs, &only.attrs),
                    ArrowShape::Plain {
                        src: src.clone(),
                        dst: only.dst,
                    },
                ))
            } else {
                *fork = NonEmpty::from_vec(parts).ok_or_else(|| {
                    CommandError::Rejected("a fork keeps at least one branch".into())
                })?;
                None
            }
        }
        ArrowShape::Join { join, dst } => {
            let mut parts: Vec<_> = join.iter().cloned().collect();
            let l = parts.remove(p);
            lost.extend(box_side(&l.src));
            if let Some(o) = origin {
                o.parts.remove(p);
            }
            if parts.len() == 1 {
                let only = parts.remove(0);
                Some((
                    merge(&arrow.attrs, &only.attrs),
                    ArrowShape::Plain {
                        src: only.src,
                        dst: dst.clone(),
                    },
                ))
            } else {
                *join = NonEmpty::from_vec(parts).ok_or_else(|| {
                    CommandError::Rejected("a join keeps at least one leaf".into())
                })?;
                None
            }
        }
        ArrowShape::Plain { .. } => None,
    };
    if let Some((attrs, shape)) = collapsed {
        *arrow = Arrow { attrs, shape };
        if let Some(o) = origin {
            o.single = o.parts.first().copied().flatten();
            o.parts.clear();
        }
    }
    Ok(lost)
}

/// A collapsed bundle's attributes: the trunk's, with the surviving part's own values first.
fn merge(trunk: &ArrowAttrs, part: &ArrowAttrs) -> ArrowAttrs {
    ArrowAttrs {
        concept: part.concept.clone().or_else(|| trunk.concept.clone()),
        label: part.label.clone().or_else(|| trunk.label.clone()),
        key: trunk.key.clone(),
        rel: None,
        tunnel: part.tunnel.or(trunk.tunnel),
        kind: part.kind.or(trunk.kind),
        code: part.code.clone().or_else(|| trunk.code.clone()),
    }
}

fn all_ends(shape: &ArrowShape) -> Vec<&Endpoint> {
    match shape {
        ArrowShape::Plain { src, dst } => vec![src, dst],
        ArrowShape::Fork { src, fork } => {
            let mut v = vec![src];
            v.extend(fork.iter().map(|b| &b.dst));
            v
        }
        ArrowShape::Join { join, dst } => {
            let mut v: Vec<_> = join.iter().map(|l| &l.src).collect();
            v.push(dst);
            v
        }
    }
}

fn all_ends_mut(shape: &mut ArrowShape) -> Vec<&mut Endpoint> {
    match shape {
        ArrowShape::Plain { src, dst } => vec![src, dst],
        ArrowShape::Fork { src, fork } => {
            let mut v = vec![src];
            v.extend(fork.iter_mut().map(|b| &mut b.dst));
            v
        }
        ArrowShape::Join { join, dst } => {
            let mut v: Vec<_> = join.iter_mut().map(|l| &mut l.src).collect();
            v.push(dst);
            v
        }
    }
}

/// Close the rank gaps on every box side that lost an end, keeping order (FR-EDIT-07).
fn compact(d: &mut Draft<'_>, diagram: &NodeId, lost: &Lost) -> Result<(), CommandError> {
    let mut sides = lost.clone();
    sides.sort();
    sides.dedup();
    for (key, side) in sides {
        let Ok(dg) = d.diagram(diagram) else { continue };
        if !dg.boxes.iter().any(|b| b.key == key) {
            continue;
        }
        let ranks = ranks_on(dg, &key, side);
        let map: BTreeMap<u8, u8> = ranks
            .iter()
            .zip(1u8..)
            .map(|(&old, new)| (old, new))
            .collect();
        recode(d, diagram, &key, side, &map)?;
    }
    Ok(())
}

/// Rank map `map` (old → new) on box `key`'s `side` on `diagram`, and the same on the box's
/// child boundary. A child boundary code with no image becomes an uncoded border tunnel with
/// the next free ordinal (FR-EDIT-08).
fn recode(
    d: &mut Draft<'_>,
    diagram: &NodeId,
    key: &BoxKey,
    side: Side,
    map: &BTreeMap<u8, u8>,
) -> Result<(), CommandError> {
    let index = d.diagram(diagram)?.boxes.iter().position(|b| &b.key == key);
    if map.iter().any(|(a, b)| a != b) {
        let (dg, _) = d.edit(diagram)?;
        for a in &mut dg.arrows {
            for e in all_ends_mut(&mut a.shape) {
                if let Endpoint::Box {
                    key: k,
                    side: s,
                    rank,
                } = e
                    && k == key
                    && *s == side
                    && let Some(&r) = map.get(rank)
                {
                    *rank = r;
                }
            }
        }
    }
    let Some(child) = index.and_then(|i| detail_node(diagram, i)) else {
        return Ok(());
    };
    if d.diagram(&child).is_err() {
        return Ok(());
    }
    let (cd, _) = d.edit(&child)?;
    let mut next_border = cd
        .arrows
        .iter()
        .flat_map(|a| all_ends(&a.shape))
        .filter_map(|e| match e {
            Endpoint::Boundary(BoundaryPort {
                side: s,
                code: PortCode::Border(n),
            }) if *s == side => Some(*n),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let mut orphan: BTreeMap<u8, u8> = BTreeMap::new();
    let mut changed = false;
    for a in &mut cd.arrows {
        for e in all_ends_mut(&mut a.shape) {
            if let Endpoint::Boundary(BoundaryPort {
                side: s,
                code: code @ PortCode::Icom(_),
            }) = e
                && *s == side
            {
                let PortCode::Icom(c) = *code else { continue };
                if let Some(&n) = map.get(&c) {
                    if n != c {
                        *code = PortCode::Icom(n);
                        changed = true;
                    }
                } else {
                    // the parent port is gone: an uncoded border tunnel, never deleted
                    let n = *orphan.entry(c).or_insert_with(|| {
                        next_border += 1;
                        next_border
                    });
                    *code = PortCode::Border(n);
                    changed = true;
                }
            }
        }
    }
    if changed {
        d.mark(&child, Change::Major);
    }
    Ok(())
}

/// An end spec on `diagram` as an endpoint: a new port takes the next rank on its side.
fn resolve_end(d: &Draft<'_>, diagram: &NodeId, e: &EndSpec) -> Result<Endpoint, CommandError> {
    match e {
        EndSpec::At(e) => Ok(e.clone()),
        EndSpec::NewPort { key, side } => {
            let dg = d.diagram(diagram)?;
            if !dg.boxes.iter().any(|b| &b.key == key) {
                return Err(CommandError::NoBox(key.clone()));
            }
            let next = ranks_on(dg, key, *side).iter().max().copied().unwrap_or(0) + 1;
            Ok(Endpoint::Box {
                key: key.clone(),
                side: *side,
                rank: next,
            })
        }
    }
}

fn retarget(
    d: &mut Draft<'_>,
    diagram: &NodeId,
    id: &ArrowId,
    end: End,
    to: &EndSpec,
) -> Result<(), CommandError> {
    let at = find_arrow(d, diagram, id)?;
    let new = resolve_end(d, diagram, to)?;
    let (dg, _) = d.edit(diagram)?;
    let shape = &mut dg.arrows[at.stmt].shape;
    let slot: &mut Endpoint = match (shape, at.part, end) {
        (ArrowShape::Plain { src, .. } | ArrowShape::Fork { src, .. }, None, End::Src) => src,
        (ArrowShape::Plain { dst, .. } | ArrowShape::Join { dst, .. }, None, End::Dst) => dst,
        (ArrowShape::Fork { fork, .. }, Some(p), End::Dst) => {
            &mut fork
                .iter_mut()
                .nth(p)
                .ok_or_else(|| CommandError::NoArrow(diagram.clone(), id.clone()))?
                .dst
        }
        (ArrowShape::Join { join, .. }, Some(p), End::Src) => {
            &mut join
                .iter_mut()
                .nth(p)
                .ok_or_else(|| CommandError::NoArrow(diagram.clone(), id.clone()))?
                .src
        }
        _ => {
            return reject(format!(
                "{id}: that end is a bundle's shared trunk end or another part's; retarget the bundle"
            ));
        }
    };
    let old = std::mem::replace(slot, new);
    let lost: Lost = box_side(&old).into_iter().collect();
    compact(d, diagram, &lost)?;
    d.mark(diagram, Change::Major);
    Ok(())
}

fn reorder(d: &mut Draft<'_>, key: &BoxKey, side: Side, order: &[u8]) -> Result<(), CommandError> {
    let (node, _) = locate(&d.model, key)?;
    let ranks = ranks_on(d.diagram(&node)?, key, side);
    let mut sorted = order.to_vec();
    sorted.sort_unstable();
    if sorted != ranks.iter().copied().collect::<Vec<_>>() {
        return reject(format!(
            "the order must list @{key}'s {side:?} ranks {ranks:?} exactly once"
        ));
    }
    let map: BTreeMap<u8, u8> = order
        .iter()
        .zip(1u8..)
        .map(|(&old, new)| (old, new))
        .collect();
    recode(d, &node, key, side, &map)?;
    d.mark(&node, Change::Major);
    Ok(())
}

/// The §13.1 cascade.
fn delete_box(d: &mut Draft<'_>, key: &BoxKey) -> Result<(), CommandError> {
    let (node, i) = locate(&d.model, key)?;
    if matches!(node, NodeId::Context { .. }) {
        return reject("the top box and context-diagram boxes cannot be deleted");
    }
    // 1. subtree
    let removed_keys = drop_subtree(d, &node, i);
    // 2. attached arrows
    let mut lost = Lost::new();
    loop {
        let dg = d.diagram(&node)?;
        let hit = dg.arrows.iter().enumerate().find_map(|(s, a)| {
            let at_box = |e: &Endpoint| matches!(e, Endpoint::Box { key: k, .. } if k == key);
            match &a.shape {
                ArrowShape::Plain { src, dst } => (at_box(src) || at_box(dst)).then_some(At {
                    stmt: s,
                    part: None,
                }),
                ArrowShape::Fork { src, fork } => {
                    if at_box(src) {
                        Some(At {
                            stmt: s,
                            part: None,
                        })
                    } else {
                        fork.iter().position(|b| at_box(&b.dst)).map(|p| At {
                            stmt: s,
                            part: Some(p),
                        })
                    }
                }
                ArrowShape::Join { join, dst } => {
                    if at_box(dst) {
                        Some(At {
                            stmt: s,
                            part: None,
                        })
                    } else {
                        join.iter().position(|l| at_box(&l.src)).map(|p| At {
                            stmt: s,
                            part: Some(p),
                        })
                    }
                }
            }
        });
        let Some(at) = hit else { break };
        lost.extend(remove_at(d, &node, at)?);
    }
    // 3. compaction on the other boxes, while box positions still match their child
    //    diagrams' nodes
    lost.retain(|(k, _)| k != key);
    compact(d, &node, &lost)?;
    // the box itself
    {
        let (dg, _) = d.edit(&node)?;
        dg.boxes.remove(i);
    }
    // 5. renumber the later boxes
    let k = u8::try_from(i + 1).unwrap_or(u8::MAX);
    renumber_under(d, &node, &|n| if n > k { n - 1 } else { n });
    // 7. calls into the removed boxes
    let mut gone = removed_keys;
    gone.push(key.clone());
    drop_calls(d, &gone)?;
    d.mark(&node, Change::Major);
    Ok(())
}

/// Remove the child diagram of box `i` on `node` and every descendant; returns the keys of
/// the boxes removed with them.
fn drop_subtree(d: &mut Draft<'_>, node: &NodeId, i: usize) -> Vec<BoxKey> {
    let Some(child) = detail_node(node, i) else {
        return Vec::new();
    };
    let under: Vec<NodeId> = d
        .model
        .diagrams
        .keys()
        .filter(|n| {
            matches!(n, NodeId::Node(_))
                && n.boxes().starts_with(child.boxes())
                && n.boxes().len() >= child.boxes().len()
        })
        .cloned()
        .collect();
    let mut keys = Vec::new();
    for n in under {
        if let Some(dg) = d.model.diagrams.get(&n) {
            keys.extend(dg.boxes.iter().map(|b| b.key.clone()));
        }
        d.remove(&n);
    }
    keys
}

/// Remove local call targets that point at `gone`; a call left with none goes (§13.1 step 7).
fn drop_calls(d: &mut Draft<'_>, gone: &[BoxKey]) -> Result<(), CommandError> {
    let hits: Vec<(NodeId, usize)> = d
        .model
        .diagrams
        .iter()
        .flat_map(|(n, dg)| {
            dg.boxes
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    b.call.as_ref().is_some_and(|c| {
                        c.targets
                            .iter()
                            .any(|t| t.model.is_none() && gone.contains(&t.key))
                    })
                })
                .map(|(i, _)| (n.clone(), i))
                .collect::<Vec<_>>()
        })
        .collect();
    for (n, i) in hits {
        let (dg, _) = d.edit(&n)?;
        let b = &mut dg.boxes[i];
        if let Some(call) = b.call.take() {
            let keep: Vec<_> = call
                .targets
                .into_iter()
                .filter(|t| !(t.model.is_none() && gone.contains(&t.key)))
                .collect();
            b.call = NonEmpty::from_vec(keep).map(|targets| crate::model::CallArrow {
                targets,
                note: call.note,
                map: call.map,
            });
        }
        d.mark(&n, Change::Major);
    }
    Ok(())
}

fn delete_diagram(d: &mut Draft<'_>, diagram: &NodeId) -> Result<(), CommandError> {
    let NodeId::Node(path) = diagram else {
        return reject("context diagrams cannot be deleted");
    };
    d.diagram(diagram)?;
    let parent = if path.is_empty() {
        NodeId::context(0)
    } else {
        diagram.parent().unwrap_or_else(NodeId::a0)
    };
    let i = if path.is_empty() {
        0
    } else {
        usize::from(path[path.len() - 1]) - 1
    };
    let gone = drop_subtree(d, &parent, i);
    drop_calls(d, &gone)?;
    if d.diagram(&parent).is_ok() {
        d.mark(&parent, Change::Minor);
    }
    Ok(())
}

/// Paste `clip` at position `at` on `diagram`: `AddBox` semantics, fresh keys everywhere, the
/// subtree's internal arrows and notes rewritten to them (FR-EDIT-13).
fn paste(
    d: &mut Draft<'_>,
    diagram: &NodeId,
    at: u8,
    clip: &Clip,
    env: &mut Env<'_>,
) -> Result<(), CommandError> {
    let mut keys: BTreeMap<BoxKey, BoxKey> = BTreeMap::new();
    let root_key = mint(d, env, clip.root.name.as_str());
    keys.insert(clip.root.key.clone(), root_key.clone());
    let mut minted = vec![root_key.clone()];
    for (_, sd) in &clip.subtree {
        for b in &sd.boxes {
            let taken = |k: &BoxKey| {
                minted.contains(k)
                    || d.model.meta.retired.contains(k)
                    || d.model
                        .diagrams
                        .values()
                        .any(|dg| dg.boxes.iter().any(|x| &x.key == k))
            };
            let k = env.minter.box_key(b.name.as_str(), &taken);
            minted.push(k.clone());
            keys.insert(b.key.clone(), k);
        }
    }
    let mut root = clip.root.clone();
    root.key = root_key;
    insert_box(d, diagram, at, root)?;
    let Some(base) = diagram.child(at) else {
        return Ok(());
    };
    for (rel, sd) in &clip.subtree {
        let mut path = base.boxes().to_vec();
        path.extend_from_slice(rel);
        let Some(node) = NodeId::from_boxes(&path) else {
            return reject("the pasted subtree is too deep for node numbers");
        };
        let mut nd = sd.clone();
        nd.status = Status::Working;
        for b in &mut nd.boxes {
            if let Some(k) = keys.get(&b.key) {
                b.key = k.clone();
            }
        }
        for a in &mut nd.arrows {
            for e in all_ends_mut(&mut a.shape) {
                if let Endpoint::Box { key, .. } = e
                    && let Some(k) = keys.get(key)
                {
                    *key = k.clone();
                }
            }
        }
        for n in &mut nd.notes {
            for t in &mut n.on {
                match t {
                    NoteTarget::Box(k) | NoteTarget::Call(k) => {
                        if let Some(new) = keys.get(k) {
                            *k = new.clone();
                        }
                    }
                    NoteTarget::Arrow { src, dst, .. } => {
                        for e in [src, dst] {
                            if let Endpoint::Box { key, .. } = e
                                && let Some(k) = keys.get(key)
                            {
                                *key = k.clone();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        d.insert(node, nd);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
