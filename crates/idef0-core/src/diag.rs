//! Diagnostics and the core part of the diagnostic registry (`docs/spec/validation.md` §7,
//! §8.1; `docs/spec/metamodel.md` §18).
//!
//! Core owns the `E-*` codes that the metamodel defines. [`E_CODES`] lists them with their
//! subject kind; a test keeps it equal to the metamodel's code table, so there is one
//! registry, not two.

use core::fmt;

use crate::id::NodeId;
use crate::identity::ArrowId;
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

/// `E-LIMIT-01`: a parser input limit was exceeded (architecture §8.3, which owns it).
pub const E_LIMIT_01: Code = Code("E-LIMIT-01");

/// `L-FMT-01`: the file has no version header and is read as legacy 0.1 (metamodel §14).
pub const L_FMT_01: Code = Code("L-FMT-01");

/// Core codes owned by other documents: `E-LIMIT-01` (architecture §8.3) and the lints core
/// emits before the lint layer exists (validation §7.2).
pub const OTHER_CODES: &[CodeInfo] = &[
    CodeInfo {
        code: E_LIMIT_01,
        subject: SubjectKind::Model,
        summary: "Parser input limit exceeded",
    },
    CodeInfo {
        code: L_FMT_01,
        subject: SubjectKind::Model,
        summary: "Missing version header (legacy 0.1)",
    },
];

/// The registry entry for `code`, if core owns it.
pub fn code_info(code: Code) -> Option<&'static CodeInfo> {
    E_CODES
        .iter()
        .chain(OTHER_CODES)
        .find(|info| info.code == code)
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

/// What a diagnostic reports on. The order is the report's canonical subject order
/// (validation §8.1): diagrams in node order, then boxes, edges, concepts, the model.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectRef {
    /// A diagram.
    Diagram(NodeId),
    /// A box, by key.
    Box(BoxKey),
    /// An arrow, by its id (metamodel §3.4).
    Edge(ArrowId),
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

/// A source location (architecture §4.4): byte offsets into the file, and the 1-based line
/// and UTF-8 byte column of `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Span {
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last byte.
    pub end: u32,
    /// 1-based line of `start`.
    pub line: u32,
    /// 1-based UTF-8 byte column of `start`.
    pub col: u32,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
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
    /// Where in the source, when the finding comes from a parsed file.
    pub span: Option<Span>,
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
            span: None,
        }
    }

    /// A lint warning from core, such as `L-FMT-01`.
    pub fn lint_warning(code: Code, subject: SubjectRef, message: impl Into<String>) -> Self {
        debug_assert!(code.as_str().starts_with("L-"), "{code} is not a lint");
        Self {
            severity: Severity::Warning,
            subject,
            code,
            layer: Layer::L,
            message: message.into(),
            span: None,
        }
    }

    /// The same finding, located at `span`.
    #[must_use]
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        if let Some(span) = self.span {
            write!(f, "{span}: ")?;
        }
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

    // validation §7.3 lists E-LIMIT-01 as "core / architecture", subject model.
    #[test]
    fn e_limit_01_is_in_validation_7_3() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/validation.md");
        let doc = std::fs::read_to_string(path).unwrap();
        let row = doc
            .lines()
            .find(|l| l.starts_with("| `E-LIMIT-01`"))
            .unwrap();
        assert!(
            row.contains("| model |") && row.contains("core / architecture"),
            "{row}"
        );
    }

    #[test]
    fn display() {
        let key: BoxKey = "check-order".parse().unwrap();
        let d = Diagnostic::fatal(E_CALL_02, SubjectRef::Box(key), "the top box cannot call");
        assert_eq!(
            d.to_string(),
            "error[E-CALL-02] box @check-order: the top box cannot call"
        );
    }
}
