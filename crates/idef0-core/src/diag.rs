//! Diagnostics and the core part of the diagnostic registry (`docs/spec/validation.md` §7,
//! §8.1; `docs/spec/metamodel.md` §18).
//!
//! Core owns the `E-*` codes that the metamodel defines. [`E_CODES`] lists them with their
//! subject kind; a test keeps it equal to the metamodel's code table, so there is one
//! registry, not two.

use core::fmt;

use crate::id::NodeId;
use crate::text::{BoxKey, ConceptKey};

/// A diagnostic code, such as `E-TYPE-01`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(&'static str);

impl Code {
    /// The code as written in the registry.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// The kind of element a code reports on. Each code has exactly one (validation §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectKind {
    /// A diagram.
    Diagram,
    /// A function box.
    Box,
    /// An arrow (edge).
    Edge,
    /// A concept.
    Concept,
    /// The model.
    Model,
}

/// A core-owned `E-*` registry entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    /// The code.
    pub code: Code,
    /// What it reports on.
    pub subject: SubjectKind,
    /// One-line summary, as in metamodel §18.
    pub summary: &'static str,
}

macro_rules! e_codes {
    ($($konst:ident = $code:literal, $kind:ident, $summary:literal;)*) => {
        $(
            #[doc = concat!("`", $code, "`: ", $summary, ".")]
            pub const $konst: Code = Code($code);
        )*

        /// Every `E-*` code defined by the metamodel (§18), in registry order.
        pub const E_CODES: &[CodeInfo] = &[
            $(CodeInfo { code: $konst, subject: SubjectKind::$kind, summary: $summary },)*
        ];
    };
}

e_codes! {
    E_PARSE_01 = "E-PARSE-01", Model, "Syntax error";
    E_PARSE_02 = "E-PARSE-02", Model, "Unknown attribute";
    E_VERSION_01 = "E-VERSION-01", Model, "Older version needs migrate";
    E_VERSION_02 = "E-VERSION-02", Model, "Newer major refused";
    E_ID_01 = "E-ID-01", Model, "Missing or malformed abbrev";
    E_ID_02 = "E-ID-02", Box, "Duplicate box key";
    E_ID_03 = "E-ID-03", Concept, "Duplicate concept key";
    E_ID_04 = "E-ID-04", Edge, "Duplicate explicit arrow key";
    E_ID_05 = "E-ID-05", Model, "Malformed key";
    E_ID_06 = "E-ID-06", Model, "Duplicate abbrev in workspace";
    E_NODE_01 = "E-NODE-01", Diagram, "Invalid node digit";
    E_NODE_02 = "E-NODE-02", Diagram, "Bad box numbering";
    E_NODE_03 = "E-NODE-03", Diagram, "Diagram without a valid parent box";
    E_TYPE_01 = "E-TYPE-01", Edge, "Illegal source";
    E_TYPE_02 = "E-TYPE-02", Edge, "Illegal destination";
    E_TYPE_03 = "E-TYPE-03", Edge, "Boundary pass-through";
    E_TYPE_04 = "E-TYPE-04", Edge, "Rank gap or duplicate";
    E_TYPE_05 = "E-TYPE-05", Edge, "Ambiguous `tunnel=box`";
    E_TITLE_MATCH = "E-TITLE-MATCH", Diagram, "Restated title differs from parent box name";
    E_CALL_01 = "E-CALL-01", Box, "Call on a nonexistent box number";
    E_CALL_02 = "E-CALL-02", Box, "Call on the top box";
    E_NOTE_01 = "E-NOTE-01", Diagram, "Note target not on this diagram";
}

/// The registry entry for `code`, if core owns it.
pub fn code_info(code: Code) -> Option<&'static CodeInfo> {
    E_CODES.iter().find(|info| info.code == code)
}

/// A diagnostic layer (validation §1): fatal core checks, lints, rules, geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// `E-*`: fatal; no facts are extracted.
    E,
    /// `L-*`: Rust lints.
    L,
    /// `R-*`: Mangle rules.
    R,
    /// `G-*`: layout geometry.
    G,
}

/// A severity. `E-*` and `G-*` findings are always `Error` (FR-VAL-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Blocks.
    Error,
    /// Should be fixed.
    Warning,
    /// For information.
    Info,
}

/// An arrow statement, or one branch or leaf of a bundle, by position: the `arrow`-th
/// statement of `diagram` (0-based) and, for a bundle, its `part`-th branch or leaf. This
/// stands in for the structural arrow id until it exists (metamodel §3.4, roadmap M1.4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeRef {
    /// The diagram.
    pub diagram: NodeId,
    /// 0-based arrow statement index.
    pub arrow: u32,
    /// 0-based branch or leaf index; `None` for a plain arrow or a bundle trunk.
    pub part: Option<u32>,
}

impl fmt::Display for EdgeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:#{}", self.diagram, self.arrow)?;
        self.part.map_or(Ok(()), |p| write!(f, "/{p}"))
    }
}

/// What a diagnostic reports on. The order is the report's canonical subject order
/// (validation §8.1): diagrams in node order, then boxes, edges, concepts, the model.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectRef {
    /// A diagram.
    Diagram(NodeId),
    /// A box, by key.
    Box(BoxKey),
    /// An arrow.
    Edge(EdgeRef),
    /// A concept, by key.
    Concept(ConceptKey),
    /// The model.
    Model,
}

impl SubjectRef {
    /// The subject's kind.
    pub fn kind(&self) -> SubjectKind {
        match self {
            Self::Diagram(_) => SubjectKind::Diagram,
            Self::Box(_) => SubjectKind::Box,
            Self::Edge(_) => SubjectKind::Edge,
            Self::Concept(_) => SubjectKind::Concept,
            Self::Model => SubjectKind::Model,
        }
    }
}

impl fmt::Display for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagram(n) => write!(f, "diagram {n}"),
            Self::Box(k) => write!(f, "box @{k}"),
            Self::Edge(e) => write!(f, "edge {e}"),
            Self::Concept(k) => write!(f, "concept @{k}"),
            Self::Model => f.write_str("model"),
        }
    }
}

/// A finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Diagnostic {
    /// Severity, first so that sorting puts errors first.
    pub severity: Severity,
    /// What it reports on.
    pub subject: SubjectRef,
    /// The registry code.
    pub code: Code,
    /// The layer.
    pub layer: Layer,
    /// A human-readable message.
    pub message: String,
}

impl Diagnostic {
    /// A fatal `E-*` finding.
    ///
    /// # Panics
    ///
    /// In debug builds, if `code` is not a core `E-*` code or `subject` is not its registered
    /// subject kind: one kind per code is a registry invariant.
    pub fn fatal(code: Code, subject: SubjectRef, message: impl Into<String>) -> Self {
        debug_assert_eq!(
            code_info(code).map(|i| i.subject),
            Some(subject.kind()),
            "{code} reported on the wrong subject kind"
        );
        Self {
            severity: Severity::Error,
            subject,
            code,
            layer: Layer::E,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        write!(f, "{sev}[{}] {}: {}", self.code, self.subject, self.message)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Expand a metamodel §18 code cell: `` `E-ID-01`, `E-ID-05` `` or `` `E-NODE-01`..`E-NODE-03` ``.
    fn codes_in(cell: &str) -> Vec<String> {
        let names: Vec<&str> = cell.split('`').skip(1).step_by(2).collect();
        if cell.contains("..") && names.len() == 2 {
            let (prefix, from) = names[0].rsplit_once('-').unwrap();
            let (_, to) = names[1].rsplit_once('-').unwrap();
            let width = from.len();
            let (from, to): (u32, u32) = (from.parse().unwrap(), to.parse().unwrap());
            (from..=to)
                .map(|n| format!("{prefix}-{n:0width$}"))
                .collect()
        } else {
            names.into_iter().map(str::to_owned).collect()
        }
    }

    fn kind(word: &str) -> SubjectKind {
        match word {
            "diagram" => SubjectKind::Diagram,
            "box" => SubjectKind::Box,
            "edge" => SubjectKind::Edge,
            "concept" => SubjectKind::Concept,
            "model" => SubjectKind::Model,
            other => panic!("unknown subject kind {other:?} in metamodel §18"),
        }
    }

    // SR-57, validation §7: one registry. E_CODES equals metamodel §18's core E codes, with
    // the same subject kinds.
    #[test]
    fn registry_matches_metamodel_section_18() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/metamodel.md");
        let doc = std::fs::read_to_string(path).unwrap();
        let section = doc.split("## 18. Codes and open items").nth(1).unwrap();
        let mut spec = BTreeMap::new();
        for row in section.lines().filter(|l| l.starts_with("| `E-")) {
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            let (codes, layer, subjects) = (cells[1], cells[2], cells[3]);
            assert_eq!(layer, "core", "{row}");
            let groups: Vec<&str> = codes.split(" / ").collect();
            let kinds: Vec<&str> = subjects.split(" / ").collect();
            for (i, group) in groups.iter().enumerate() {
                let k = kind(kinds.get(i).copied().unwrap_or(kinds[0]));
                for code in codes_in(group) {
                    assert!(
                        spec.insert(code.clone(), k).is_none(),
                        "{code} twice in §18"
                    );
                }
            }
        }
        let ours: BTreeMap<String, SubjectKind> = E_CODES
            .iter()
            .map(|i| (i.code.as_str().to_owned(), i.subject))
            .collect();
        assert_eq!(ours, spec);
    }

    #[test]
    fn display() {
        let e = EdgeRef {
            diagram: "A0".parse().unwrap(),
            arrow: 3,
            part: Some(1),
        };
        let d = Diagnostic::fatal(E_TYPE_01, SubjectRef::Edge(e), "bad source");
        assert_eq!(d.to_string(), "error[E-TYPE-01] edge A0:#3/1: bad source");
    }
}
