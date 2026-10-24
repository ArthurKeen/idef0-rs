//! The browser's two sources of truth (architecture §5.1): the document and the editor
//! state. Everything visual is derived from them.

use std::sync::Arc;

use idef0_core::command::history::History;
use idef0_core::id::NodeId;
use idef0_core::model::Model;
use idef0_layout::metrics::Metrics;
use idef0_layout::scene::{LayoutInput, ThemeId, layout};
use idef0_layout::svg::{SvgOptions, render};

/// The bundled dogfood model: idef0-rs modeled in itself.
pub const BUNDLED: &str = include_str!("../../../fixtures/dogfood/idef0-rs.idef0");

/// The document: the model, its revision and its undo history. Pending server commands
/// arrive with the server epics (M8).
#[derive(Debug, Clone)]
pub struct Doc {
    /// The model. `Arc` sharing lets an untouched diagram compare equal by pointer.
    pub model: Arc<Model>,
    /// Bumped on every change.
    pub rev: u64,
    /// Undo and redo.
    pub history: History,
}

/// Why a model could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenError(pub Vec<String>);

impl Doc {
    /// A document from `.idef0` source. Syntax errors prevent opening; type-check findings
    /// do not, so a model under repair can still be looked at.
    ///
    /// # Errors
    ///
    /// [`OpenError`] with the parse diagnostics when there is no model.
    pub fn open(src: &str) -> Result<Self, OpenError> {
        let parsed = idef0_core::dsl::parse(src);
        let fatal = parsed
            .diagnostics
            .iter()
            .any(|d| d.code.as_str().starts_with("E-PARSE"));
        match parsed.model.filter(|_| !fatal) {
            Some(m) => Ok(Self {
                model: Arc::new(m),
                rev: 0,
                history: History::default(),
            }),
            None => Err(OpenError(
                parsed.diagnostics.iter().map(ToString::to_string).collect(),
            )),
        }
    }

    /// The bundled dogfood model.
    ///
    /// # Errors
    ///
    /// Only if the bundled fixture stopped parsing, which the tests prevent.
    pub fn bundled() -> Result<Self, OpenError> {
        Self::open(BUNDLED)
    }

    /// The theme the model's profile selects (§13.1).
    pub fn theme(&self) -> ThemeId {
        ThemeId::default_for(self.model.meta.profile)
    }

    /// The diagram to show first: A0 if present, else the first diagram.
    pub fn first_diagram(&self) -> Option<NodeId> {
        if self.model.diagrams.contains_key(&NodeId::a0()) {
            Some(NodeId::a0())
        } else {
            self.model.diagrams.keys().next().cloned()
        }
    }

    /// The SVG of diagram `node`, or the reason it cannot be drawn.
    ///
    /// # Errors
    ///
    /// The layout error as text.
    pub fn svg(&self, node: &NodeId) -> Result<String, String> {
        let metrics = Metrics::default();
        let input = LayoutInput {
            theme: self.theme(),
            metrics: &metrics,
        };
        layout(&self.model, node, &input)
            .map(|r| render(&r.scene, &SvgOptions::default()))
            .map_err(|e| e.to_string())
    }
}

/// The editor state. The keyboard FSM (M6) grows it; the viewer needs the diagram shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    /// The diagram on screen.
    pub diagram: NodeId,
}

#[cfg(test)]
mod tests {
    use super::*;

    // M7.1: the bundled model opens and its first diagram renders.
    #[test]
    fn bundled_model_renders() {
        let doc = Doc::bundled().expect("bundled");
        assert_eq!(doc.model.meta.abbrev.to_string(), "IR");
        let first = doc.first_diagram().expect("a diagram");
        assert_eq!(first, NodeId::a0());
        let svg = doc.svg(&first).expect("svg");
        assert!(svg.starts_with("<svg ") && svg.contains("data-key=\"A0\""));
        assert!(doc.svg(&"A7".parse().expect("A7")).is_err());
    }

    #[test]
    fn syntax_errors_do_not_open() {
        let e = Doc::open("idef0 0.2\nmodel {").expect_err("broken");
        assert!(!e.0.is_empty());
        // error recovery still yields a partial model: it is for diagnostics only, never
        // opened (metamodel §16.4)
        let src = include_str!("../../../fixtures/invalid/E-PARSE-01/recovery.idef0");
        assert!(
            idef0_core::dsl::parse(src).model.is_some(),
            "a partial model exists"
        );
        let e = Doc::open(src).expect_err("recovered, but not openable");
        assert!(e.0.iter().any(|d| d.contains("E-PARSE-01")), "{:?}", e.0);
    }
}
