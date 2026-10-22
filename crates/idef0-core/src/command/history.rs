//! Undo and redo (metamodel §13.6, FR-EDIT-17): one stack per model, across its diagrams,
//! with navigation to the change and selection restore.

use std::collections::VecDeque;

use crate::id::NodeId;
use crate::identity::{ArrowId, arrow_ids};
use crate::model::Model;
use crate::text::BoxKey;

use super::{Patch, apply_ops};

/// One selected element.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Selected {
    /// A diagram.
    Diagram(NodeId),
    /// A box.
    Box(BoxKey),
    /// An arrow, branch or leaf on a diagram.
    Arrow(NodeId, ArrowId),
}

/// A selection: what the editor had selected.
pub type Selection = Vec<Selected>;

/// One undo entry: a patch and the selections around it (a compound operation is one
/// entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The patch.
    pub patch: Patch,
    /// The selection before the change.
    pub before: Selection,
    /// The selection after it.
    pub after: Selection,
}

/// Where the editor goes after an undo or redo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The model after the step.
    pub model: Model,
    /// The diagram to show: the change's primary diagram, or its nearest surviving
    /// ancestor.
    pub diagram: NodeId,
    /// The selection to restore, without elements that no longer exist.
    pub selection: Selection,
}

/// The undo and redo stacks of one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    depth: usize,
}

impl Default for History {
    /// The default depth, 500.
    fn default() -> Self {
        Self::with_depth(500)
    }
}

impl History {
    /// Stacks keeping at most `depth` undo entries.
    pub fn with_depth(depth: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            depth: depth.max(1),
        }
    }

    /// Record a change; it clears the redo stack. The oldest entry goes past the depth.
    pub fn push(&mut self, entry: Entry) {
        self.redo.clear();
        self.undo.push_back(entry);
        while self.undo.len() > self.depth {
            self.undo.pop_front();
        }
    }

    /// Entries that can be undone.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Entries that can be redone.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Undo the last change on `model`.
    pub fn undo(&mut self, model: &Model) -> Option<Step> {
        let e = self.undo.pop_back()?;
        let model = apply_ops(model, &e.patch.inverse);
        let step = Step {
            diagram: surviving(&model, &e.patch.primary.0),
            selection: repair(&model, &e.before),
            model,
        };
        self.redo.push(e);
        Some(step)
    }

    /// Redo the last undone change on `model`.
    pub fn redo(&mut self, model: &Model) -> Option<Step> {
        let e = self.redo.pop()?;
        let model = apply_ops(model, &e.patch.ops);
        let step = Step {
            diagram: surviving(&model, &e.patch.primary.1),
            selection: repair(&model, &e.after),
            model,
        };
        self.undo.push_back(e);
        Some(step)
    }
}

/// `node`, or its nearest ancestor with a diagram; A0, then A-0, as the last resort.
fn surviving(model: &Model, node: &NodeId) -> NodeId {
    let mut n = Some(node.clone());
    while let Some(c) = n {
        if model.diagrams.contains_key(&c) {
            return c;
        }
        n = c.parent();
    }
    if model.diagrams.contains_key(&NodeId::a0()) {
        NodeId::a0()
    } else {
        NodeId::context(0)
    }
}

/// `sel` without elements `model` no longer has.
fn repair(model: &Model, sel: &Selection) -> Selection {
    sel.iter()
        .filter(|s| match s {
            Selected::Diagram(n) => model.diagrams.contains_key(n),
            Selected::Box(k) => model
                .diagrams
                .values()
                .any(|d| d.boxes.iter().any(|b| &b.key == k)),
            Selected::Arrow(n, id) => model.diagrams.get(n).is_some_and(|d| {
                arrow_ids(n, d)
                    .iter()
                    .any(|ids| &ids.trunk == id || ids.parts.contains(id))
            }),
        })
        .cloned()
        .collect()
}
