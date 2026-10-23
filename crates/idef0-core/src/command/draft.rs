//! The working copy a command edits, and how it becomes a [`Patch`].
//!
//! A [`Draft`] clones only the diagrams a command touches (`Arc::make_mut`). For every
//! touched diagram it records where each arrow statement came from (its statement index on
//! the base diagram, and its branches' or leaves' indices), so [`Draft::finish`] can derive
//! the `RenameMap` generically: node renames follow box keys, arrow-id renames follow
//! statement origins, and port renames follow the ends of surviving statements.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::diag::Severity;
use crate::id::{NodeId, PortCode, PortRef, Side};
use crate::identity::{ArrowId, arrow_ids};
use crate::model::{ArrowShape, Diagram, Endpoint, Model, NoteTarget, Status};
use crate::text::BoxKey;

use super::{CommandError, Op, Patch, RenameMap};

/// Where a statement on a touched diagram came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StmtOrigin {
    /// Its index on the base diagram.
    pub(super) index: usize,
    /// For each branch or leaf, its index on the base statement; `None` for a new one.
    pub(super) parts: Vec<Option<usize>>,
    /// For a plain arrow that a bundle collapsed into, the surviving branch or leaf.
    pub(super) single: Option<usize>,
}

impl StmtOrigin {
    fn identity(index: usize, shape: &ArrowShape) -> Self {
        let parts = match shape {
            ArrowShape::Plain { .. } => 0,
            ArrowShape::Fork { fork, .. } => fork.len(),
            ArrowShape::Join { join, .. } => join.len(),
        };
        Self {
            index,
            parts: (0..parts).map(Some).collect(),
            single: None,
        }
    }
}

/// A touched diagram's base node and statement origins (`None`: a new statement).
#[derive(Debug, Clone)]
pub(crate) struct Origin {
    pub(super) base: NodeId,
    pub(super) stmts: Vec<Option<StmtOrigin>>,
}

/// How much a change on a diagram matters for its status (metamodel §13.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Change {
    Minor,
    Major,
}

/// A command's working copy of the model.
pub(crate) struct Draft<'a> {
    pub(super) base: &'a Model,
    pub(super) model: Model,
    origins: BTreeMap<NodeId, Origin>,
    changes: BTreeMap<NodeId, Change>,
    /// The diagram the command is about, in the base and the result.
    pub(super) primary: (NodeId, NodeId),
}

fn status_rank(s: Status) -> u8 {
    match s {
        Status::Working => 0,
        Status::Draft => 1,
        Status::Recommended => 2,
        Status::Publication => 3,
    }
}

/// The ends of a statement, each with its role: `(part, is_source, endpoint)`. A trunk end
/// has `part = None`.
fn ends(shape: &ArrowShape) -> Vec<(Option<usize>, bool, &Endpoint)> {
    match shape {
        ArrowShape::Plain { src, dst } => vec![(None, true, src), (None, false, dst)],
        ArrowShape::Fork { src, fork } => {
            let mut v = vec![(None, true, src)];
            v.extend(
                fork.iter()
                    .enumerate()
                    .map(|(i, b)| (Some(i), false, &b.dst)),
            );
            v
        }
        ArrowShape::Join { join, dst } => {
            let mut v: Vec<_> = join
                .iter()
                .enumerate()
                .map(|(i, l)| (Some(i), true, &l.src))
                .collect();
            v.push((None, false, dst));
            v
        }
    }
}

/// The port an end occupies: a box port (by the box's node) or a boundary port of the
/// diagram `node`.
fn port_of(e: &Endpoint, node: &NodeId, boxes: &BTreeMap<BoxKey, NodeId>) -> Option<PortRef> {
    match e {
        Endpoint::Box { key, side, rank } => {
            PortRef::new(None, boxes.get(key)?.clone(), *side, PortCode::Icom(*rank))
        }
        Endpoint::Boundary(p) => PortRef::new(None, node.clone(), p.side, p.code),
    }
}

impl<'a> Draft<'a> {
    pub(super) fn new(base: &'a Model, primary: NodeId) -> Self {
        Self {
            base,
            model: base.clone(),
            origins: BTreeMap::new(),
            changes: BTreeMap::new(),
            primary: (primary.clone(), primary),
        }
    }

    pub(super) fn diagram(&self, node: &NodeId) -> Result<&Diagram, CommandError> {
        self.model
            .diagrams
            .get(node)
            .map(AsRef::as_ref)
            .ok_or_else(|| CommandError::NoDiagram(node.clone()))
    }

    fn touch(&mut self, node: &NodeId) {
        if !self.origins.contains_key(node)
            && let Some(d) = self.model.diagrams.get(node)
        {
            let stmts = d
                .arrows
                .iter()
                .enumerate()
                .map(|(i, a)| Some(StmtOrigin::identity(i, &a.shape)))
                .collect();
            self.origins.insert(
                node.clone(),
                Origin {
                    base: node.clone(),
                    stmts,
                },
            );
        }
    }

    /// A diagram to edit, with its statement origins alongside.
    pub(super) fn edit(
        &mut self,
        node: &NodeId,
    ) -> Result<(&mut Diagram, &mut Vec<Option<StmtOrigin>>), CommandError> {
        self.touch(node);
        let d = self
            .model
            .diagrams
            .get_mut(node)
            .ok_or_else(|| CommandError::NoDiagram(node.clone()))?;
        let origin = self
            .origins
            .get_mut(node)
            .ok_or_else(|| CommandError::NoDiagram(node.clone()))?;
        Ok((Arc::make_mut(d), &mut origin.stmts))
    }

    /// Add a new diagram (pasted); its statements have no origin.
    pub(super) fn insert(&mut self, node: NodeId, d: Diagram) {
        self.origins.remove(&node);
        self.model.diagrams.insert(node, Arc::new(d));
    }

    /// Remove a diagram; its arrows' ids map to nothing.
    pub(super) fn remove(&mut self, node: &NodeId) {
        self.model.diagrams.remove(node);
        self.origins.remove(node);
    }

    /// Move diagrams to new nodes: `f` gives the new node, or `None` to keep it. The moves
    /// are applied together, so a permutation is safe.
    pub(super) fn renumber(&mut self, f: &dyn Fn(&NodeId) -> Option<NodeId>) {
        let moving: Vec<(NodeId, NodeId)> = self
            .model
            .diagrams
            .keys()
            .filter_map(|n| f(n).filter(|m| m != n).map(|m| (n.clone(), m)))
            .collect();
        for (from, _) in &moving {
            self.touch(from);
        }
        let mut lifted = Vec::with_capacity(moving.len());
        for (from, to) in moving {
            let d = self.model.diagrams.remove(&from);
            let o = self.origins.remove(&from);
            let c = self.changes.remove(&from);
            lifted.push((to, d, o, c));
        }
        for (to, d, o, c) in lifted {
            if let Some(d) = d {
                self.model.diagrams.insert(to.clone(), d);
            }
            if let Some(o) = o {
                self.origins.insert(to.clone(), o);
            }
            if let Some(c) = c {
                self.changes.insert(to, c);
            }
        }
    }

    /// Record a change on a diagram for the status rules (FR-EDIT-15).
    pub(super) fn mark(&mut self, node: &NodeId, c: Change) {
        let e = self.changes.entry(node.clone()).or_insert(c);
        *e = (*e).max(c);
    }

    /// Apply the status rules, retire removed keys, repair note attachments, rewrite
    /// free-text node references, check validity, and derive the Patch.
    pub(super) fn finish(mut self) -> Result<(Model, Patch), CommandError> {
        // FR-EDIT-15: a major change at draft or above resets to working; a minor change at
        // recommended or publication resets to draft
        for (node, c) in std::mem::take(&mut self.changes) {
            let Some(d) = self.model.diagrams.get_mut(&node) else {
                continue;
            };
            let s = d.status;
            let reset = match c {
                Change::Major if status_rank(s) >= 1 => Some(Status::Working),
                Change::Minor if status_rank(s) >= 2 => Some(Status::Draft),
                _ => None,
            };
            if let Some(r) = reset {
                Arc::make_mut(d).status = r;
            }
        }
        // D17: a removed box's key is retired
        let before = box_keys(self.base);
        let after = box_keys(&self.model);
        let removed: Vec<BoxKey> = before.difference(&after).cloned().collect();
        self.model.meta.retired.extend(removed.iter().cloned());
        self.repair_notes();
        let base_nodes = self.base.box_nodes();
        let new_nodes = self.model.box_nodes();
        let mut rename = RenameMap::default();
        for k in &removed {
            rename.boxes.insert(k.clone(), None);
        }
        for (k, n) in &base_nodes {
            match new_nodes.get(k) {
                Some(m) if m != n => {
                    rename.nodes.insert(n.clone(), Some(m.clone()));
                }
                None => {
                    rename.nodes.insert(n.clone(), None);
                }
                _ => {}
            }
        }
        // FR-EDIT-20: same-model node references in free text follow the renames
        let moved: BTreeMap<NodeId, NodeId> = rename
            .nodes
            .iter()
            .filter_map(|(a, b)| b.clone().map(|b| (a.clone(), b)))
            .collect();
        if !moved.is_empty() {
            super::refs::rewrite_model(&mut self.model, &moved);
        }
        self.arrow_and_port_renames(&base_nodes, &new_nodes, &mut rename);
        // validity is preserved: a command never adds an `E-*` finding
        let errors = |m: &Model| {
            crate::typecheck::typecheck(m)
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .collect::<Vec<_>>()
        };
        let after_errors = errors(&self.model);
        if after_errors.len() > errors(self.base).len() {
            return Err(CommandError::Invalid(after_errors));
        }
        let (ops, inverse) = diff(self.base, &self.model);
        let model = self.model;
        Ok((
            model,
            Patch {
                commands: Vec::new(),
                ops,
                inverse,
                rename,
                primary: self.primary,
            },
        ))
    }

    /// Note attachments: targets on removed boxes and calls go; arrow targets follow their
    /// statement, or go with it (metamodel §13.1 step 6).
    fn repair_notes(&mut self) {
        let keys = box_keys(&self.model);
        let callers: BTreeSet<BoxKey> = self
            .model
            .diagrams
            .values()
            .flat_map(|d| {
                d.boxes
                    .iter()
                    .filter(|b| b.call.is_some())
                    .map(|b| b.key.clone())
            })
            .collect();
        let nodes: Vec<NodeId> = self.model.diagrams.keys().cloned().collect();
        for node in nodes {
            let Some(d) = self.model.diagrams.get(&node) else {
                continue;
            };
            if d.notes.is_empty() {
                continue;
            }
            let origin = self.origins.get(&node);
            let base = origin.and_then(|o| self.base.diagrams.get(&o.base).map(|d| (&o.base, d)));
            // the base statement index of a (src, dst, occurrence) target, and the new
            // statement that came from it
            let remap = |t: &NoteTarget| -> Option<NoteTarget> {
                match t {
                    NoteTarget::Box(k) => keys.contains(k).then(|| t.clone()),
                    NoteTarget::Call(k) => callers.contains(k).then(|| t.clone()),
                    NoteTarget::Arrow {
                        src,
                        dst,
                        occurrence,
                    } => {
                        let (Some(o), Some((_, bd))) = (origin, base) else {
                            return Some(t.clone());
                        };
                        let i = plain_index(bd, src, dst, *occurrence)?;
                        let j = o.stmts.iter().position(|s| {
                            s.as_ref()
                                .is_some_and(|s| s.index == i && s.single.is_none())
                        })?;
                        let ArrowShape::Plain { src, dst } = &d.arrows[j].shape else {
                            return None;
                        };
                        let occurrence = occurrence_of(d, j);
                        Some(NoteTarget::Arrow {
                            src: src.clone(),
                            dst: dst.clone(),
                            occurrence,
                        })
                    }
                    _ => Some(t.clone()),
                }
            };
            let repaired: Vec<_> = d
                .notes
                .iter()
                .map(|n| {
                    let mut n = n.clone();
                    n.on = n.on.iter().filter_map(remap).collect();
                    n
                })
                .collect();
            if repaired != d.notes
                && let Some(d) = self.model.diagrams.get_mut(&node)
            {
                Arc::make_mut(d).notes = repaired;
            }
        }
    }

    fn arrow_and_port_renames(
        &self,
        base_nodes: &BTreeMap<BoxKey, NodeId>,
        new_nodes: &BTreeMap<BoxKey, NodeId>,
        rename: &mut RenameMap,
    ) {
        let mut seen_base: BTreeSet<NodeId> = BTreeSet::new();
        let mut port_images: BTreeMap<PortRef, BTreeSet<Option<PortRef>>> = BTreeMap::new();
        for (node, o) in &self.origins {
            let (Some(bd), Some(nd)) = (
                self.base.diagrams.get(&o.base),
                self.model.diagrams.get(node),
            ) else {
                continue;
            };
            seen_base.insert(o.base.clone());
            let old_ids = arrow_ids(&o.base, bd);
            let new_ids = arrow_ids(node, nd);
            let mut kept: BTreeSet<ArrowId> = BTreeSet::new();
            let mut map = |from: &ArrowId, to: &ArrowId, kept: &mut BTreeSet<ArrowId>| {
                kept.insert(from.clone());
                if from != to {
                    rename.arrows.insert(from.clone(), Some(to.clone()));
                }
            };
            for (j, s) in o.stmts.iter().enumerate() {
                let Some(s) = s else { continue };
                let (old, new) = (&old_ids[s.index], &new_ids[j]);
                map(&old.trunk, &new.trunk, &mut kept);
                if let Some(p) = s.single {
                    map(&old.parts[p], &new.trunk, &mut kept);
                }
                for (q, p) in s.parts.iter().enumerate() {
                    if let Some(p) = p {
                        map(&old.parts[*p], &new.parts[q], &mut kept);
                    }
                }
                // ports: pair each surviving end with its base end
                let old_ends = ends(&bd.arrows[s.index].shape);
                for (part, is_src, e) in ends(&nd.arrows[j].shape) {
                    let base_part = match part {
                        Some(q) => s.parts.get(q).copied().flatten().map(Some),
                        None => Some(None),
                    };
                    let Some(base_part) = base_part else { continue };
                    let matched = old_ends.iter().find(|(p, src, _)| {
                        *src == is_src && (*p == base_part || (*p == s.single && part.is_none()))
                    });
                    let Some((_, _, old_e)) = matched else {
                        continue;
                    };
                    if let Some(from) = port_of(old_e, &o.base, base_nodes) {
                        port_images
                            .entry(from)
                            .or_default()
                            .insert(port_of(e, node, new_nodes));
                    }
                }
            }
            for ids in &old_ids {
                for id in core::iter::once(&ids.trunk).chain(&ids.parts) {
                    if !kept.contains(id) {
                        rename.arrows.insert(id.clone(), None);
                    }
                }
            }
            // ports of base ends that no surviving end still holds
            for a in &bd.arrows {
                for (_, _, e) in ends(&a.shape) {
                    if let Some(p) = port_of(e, &o.base, base_nodes) {
                        port_images.entry(p).or_default();
                    }
                }
            }
        }
        // diagrams removed outright: every arrow id goes
        for (node, bd) in &self.base.diagrams {
            if !seen_base.contains(node) && !self.model.diagrams.contains_key(node) {
                for ids in arrow_ids(node, bd) {
                    for id in core::iter::once(ids.trunk).chain(ids.parts) {
                        rename.arrows.insert(id, None);
                    }
                }
            }
        }
        for (from, images) in port_images {
            let image = images.iter().flatten().next().cloned();
            match image {
                Some(to) if to != from => {
                    rename.ports.insert(from, Some(to));
                }
                None => {
                    rename.ports.insert(from, None);
                }
                _ => {}
            }
        }
    }
}

fn box_keys(model: &Model) -> BTreeSet<BoxKey> {
    model
        .diagrams
        .values()
        .flat_map(|d| d.boxes.iter().map(|b| b.key.clone()))
        .collect()
}

/// The index of the plain arrow `src -> dst` with this occurrence index on `d`.
fn plain_index(d: &Diagram, src: &Endpoint, dst: &Endpoint, occurrence: u32) -> Option<usize> {
    let mut n = 0;
    for (i, a) in d.arrows.iter().enumerate() {
        if let ArrowShape::Plain { src: s, dst: t } = &a.shape
            && s == src
            && t == dst
        {
            if n == occurrence {
                return Some(i);
            }
            n += 1;
        }
    }
    None
}

/// The occurrence index of plain arrow `j` among plain arrows with its endpoints.
fn occurrence_of(d: &Diagram, j: usize) -> u32 {
    let ArrowShape::Plain { src, dst } = &d.arrows[j].shape else {
        return 0;
    };
    let n = d.arrows[..j]
        .iter()
        .filter(
            |a| matches!(&a.shape, ArrowShape::Plain { src: s, dst: t } if s == src && t == dst),
        )
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Ops that turn `from` into `to`, and their inverse: one per changed diagram, concept, or
/// the metadata.
pub(crate) fn diff(from: &Model, to: &Model) -> (Vec<Op>, Vec<Op>) {
    let mut ops = Vec::new();
    let mut inverse = Vec::new();
    if from.meta != to.meta {
        ops.push(Op::Meta(Box::new(to.meta.clone())));
        inverse.push(Op::Meta(Box::new(from.meta.clone())));
    }
    let keys: BTreeSet<_> = from.concepts.keys().chain(to.concepts.keys()).collect();
    for k in keys {
        let (a, b) = (from.concepts.get(k), to.concepts.get(k));
        if a != b {
            ops.push(Op::Concept {
                key: k.clone(),
                value: b.cloned().map(Box::new),
            });
            inverse.push(Op::Concept {
                key: k.clone(),
                value: a.cloned().map(Box::new),
            });
        }
    }
    let nodes: BTreeSet<_> = from.diagrams.keys().chain(to.diagrams.keys()).collect();
    for n in nodes {
        let (a, b) = (from.diagrams.get(n), to.diagrams.get(n));
        if a != b {
            ops.push(Op::Diagram {
                node: n.clone(),
                value: b.cloned(),
            });
            inverse.push(Op::Diagram {
                node: n.clone(),
                value: a.cloned(),
            });
        }
    }
    (ops, inverse)
}

/// The side's ports on a box: the distinct ranks its ends use on diagram `d`.
pub(crate) fn ranks_on(d: &Diagram, key: &BoxKey, side: Side) -> BTreeSet<u8> {
    let mut out = BTreeSet::new();
    for a in &d.arrows {
        for (_, _, e) in ends(&a.shape) {
            if let Endpoint::Box {
                key: k,
                side: s,
                rank,
            } = e
                && k == key
                && *s == side
            {
                out.insert(*rank);
            }
        }
    }
    out
}
