//! The `.idef0` DSL (metamodel §16): a lossless lexer and CST, a recovering parser, and
//! lowering to a [`Model`] with a [`SpanMap`].
//!
//! [`parse`] reports parse-stage findings only: `E-PARSE-*`, `E-VERSION-*`, `E-LIMIT-01`,
//! `L-FMT-01`, and the codes a [`Model`] cannot represent (`E-ID-01`, `E-ID-03`, `E-ID-05`,
//! `E-NODE-01`, `E-NODE-02` for statement-order numbering, `E-CALL-01`). Run
//! [`crate::typecheck::typecheck`] on the model for the rest of stage 1 (validation §2.1).

use std::collections::BTreeMap;

use crate::diag::{Diagnostic, E_LIMIT_01, E_VERSION_01, E_VERSION_02, L_FMT_01, Span, SubjectRef};
use crate::model::Model;

pub mod cst;
pub mod lex;
mod lower;
pub mod parse;

/// Where each element was declared (validation §8.1). Not part of model equality.
pub type SpanMap = BTreeMap<SubjectRef, Span>;

/// The DSL version this parser reads (FR-WS-04).
pub const VERSION: (u32, u32) = (0, 2);

/// architecture §8.3: the largest file and the longest line.
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 64 * 1024;

/// The result of [`parse`].
#[derive(Debug, Clone)]
pub struct Parsed {
    /// The model, if the file declares one with a valid name and abbrev. With errors present
    /// it is partial: use it for diagnostics only, never write it back (§16.4).
    pub model: Option<Model>,
    /// The lossless tree.
    pub cst: cst::Node,
    /// Declaration spans.
    pub spans: SpanMap,
    /// Parse-stage findings, sorted.
    pub diagnostics: Vec<Diagnostic>,
}

impl Parsed {
    /// Whether any finding is an error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == crate::diag::Severity::Error)
    }
}

/// Byte offsets of line starts, for 1-based line and column numbers.
#[derive(Debug, Clone)]
pub struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    /// Index `src`.
    pub fn new(src: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            src.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| u32::try_from(i + 1).unwrap_or(u32::MAX)),
        );
        Self { starts }
    }

    /// The span from `start` to `end`, with the line and byte column of `start`.
    pub fn span(&self, start: u32, end: u32) -> Span {
        let line = self.starts.partition_point(|&s| s <= start);
        let line_start = self
            .starts
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or(0);
        Span {
            start,
            end,
            line: u32::try_from(line).unwrap_or(u32::MAX),
            col: start - line_start + 1,
        }
    }
}

fn fatal(code: crate::diag::Code, span: Span, message: String) -> Diagnostic {
    Diagnostic::fatal(code, SubjectRef::Model, message).at(span)
}

/// Parse a `.idef0` file.
pub fn parse(src: &str) -> Parsed {
    let lines = LineIndex::new(src);
    let empty = |diagnostics| Parsed {
        model: None,
        cst: cst::Node {
            kind: cst::NodeKind::File,
            children: Vec::new(),
        },
        spans: SpanMap::new(),
        diagnostics,
    };
    if src.len() > MAX_FILE_BYTES {
        let span = lines.span(0, 0);
        return empty(vec![fatal(
            E_LIMIT_01,
            span,
            format!("the file is larger than {MAX_FILE_BYTES} bytes"),
        )]);
    }
    let mut offset = 0u32;
    for line in src.split('\n') {
        if line.len() > MAX_LINE_BYTES {
            let span = lines.span(offset, offset);
            return empty(vec![fatal(
                E_LIMIT_01,
                span,
                format!("a line is longer than {MAX_LINE_BYTES} bytes"),
            )]);
        }
        offset += u32::try_from(line.len() + 1).unwrap_or(u32::MAX);
    }
    let tokens = lex::lex(src);
    let out = parse::parse(src, &tokens, &lines);
    let mut diagnostics = out.errors;
    match out.ast.header.as_ref().map(|h| (h.value, h.span)) {
        Some((v, _)) if v == VERSION => {}
        Some(((0, minor), span)) if minor < VERSION.1 => {
            diagnostics.push(fatal(
                E_VERSION_01,
                span,
                format!("this file is DSL 0.{minor}; run `idef0 migrate` to upgrade it to 0.2"),
            ));
            return Parsed {
                model: None,
                cst: out.cst,
                spans: SpanMap::new(),
                diagnostics,
            };
        }
        Some(((major, minor), span)) => {
            diagnostics.push(fatal(
                E_VERSION_02,
                span,
                format!("DSL {major}.{minor} is newer than this idef0, which reads 0.2"),
            ));
            return Parsed {
                model: None,
                cst: out.cst,
                spans: SpanMap::new(),
                diagnostics,
            };
        }
        None => {
            let span = lines.span(0, 0);
            let msg = "no version header: read as legacy 0.1 with the 0.2 grammar; run `idef0 migrate` (FR-WS-05)";
            diagnostics.push(Diagnostic::lint_warning(L_FMT_01, SubjectRef::Model, msg).at(span));
        }
    }
    let lowered = lower::lower(&out.ast);
    diagnostics.extend(lowered.diagnostics);
    diagnostics.sort_by(|a, b| (a.span, a).cmp(&(b.span, b)));
    diagnostics.dedup();
    Parsed {
        model: lowered.model,
        cst: out.cst,
        spans: lowered.spans,
        diagnostics,
    }
}
