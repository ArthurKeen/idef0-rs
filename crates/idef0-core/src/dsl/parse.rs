//! The statement parser (metamodel §16.2, §16.4): tokens to a lossless CST and a small AST
//! with spans. Recovery is hand-written resynchronization: after an error, skip to the next
//! line that starts a top-level declaration, or, inside a diagram, the next statement or the
//! closing `}`. At most [`MAX_ERRORS`] syntax errors are reported.

use super::LineIndex;
use super::cst::{Builder, Node, NodeKind};
use super::lex::{Token, TokenKind};
use crate::diag::{Code, Diagnostic, E_PARSE_01, Span, SubjectRef};
use crate::id::Side;

/// The most syntax errors one parse reports (§16.4).
pub const MAX_ERRORS: usize = 100;

/// A value with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Where it was written.
    pub span: Span,
}

/// The parsed file.
#[derive(Debug, Clone, Default)]
pub struct Ast {
    /// `idef0 MAJOR.MINOR`, if present.
    pub header: Option<Spanned<(u32, u32)>>,
    /// The `model` declaration.
    pub model: Option<ModelDecl>,
    /// Concepts in file order.
    pub concepts: Vec<ConceptDecl>,
    /// Diagrams in file order.
    pub diagrams: Vec<DiagramDecl>,
}

/// `model "name" { meta_field* }`.
#[derive(Debug, Clone)]
pub struct ModelDecl {
    /// The model name.
    pub name: Spanned<String>,
    /// The metadata fields.
    pub fields: Vec<MetaField>,
    /// The declaration.
    pub span: Span,
}

/// The value of a `meta_field`.
#[derive(Debug, Clone)]
pub enum MetaValue {
    /// A `TEXT` value.
    Text(String),
    /// A word, as in `profile extended`.
    Word(String),
    /// `retired { KEY* }`.
    Keys(Vec<Spanned<String>>),
}

/// One `meta_field`.
#[derive(Debug, Clone)]
pub struct MetaField {
    /// The field name.
    pub name: Spanned<String>,
    /// The value.
    pub value: MetaValue,
}

/// An attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    /// `name=word`.
    Word(String),
    /// `name="text"`.
    Text(String),
}

/// `name=value`.
#[derive(Debug, Clone)]
pub struct Attr {
    /// The attribute name.
    pub name: Spanned<String>,
    /// Its value.
    pub value: Spanned<AttrValue>,
}

/// `concept KEY "name" origin* { concept_field* }`.
#[derive(Debug, Clone)]
pub struct ConceptDecl {
    /// The key as written.
    pub key: Spanned<String>,
    /// The name.
    pub name: Spanned<String>,
    /// `origin=` and `source=`.
    pub attrs: Vec<Attr>,
    /// The fields.
    pub fields: Vec<ConceptField>,
    /// The declaration.
    pub span: Span,
}

/// One `concept_field`.
#[derive(Debug, Clone)]
pub enum ConceptField {
    /// `description TEXT origin*`.
    Description(Spanned<String>, Vec<Attr>),
    /// `synonyms STRING, …`.
    Synonyms(Vec<Spanned<String>>),
    /// `kind_of KEY, …`.
    KindOf(Vec<Spanned<String>>),
    /// `part_of KEY, …`.
    PartOf(Vec<Spanned<String>>),
    /// `example TEXT`.
    Example(String),
    /// `source STRING`.
    Source(String),
}

/// `diagram NODE "title"? attrs { diag_stmt* }`.
#[derive(Debug, Clone)]
pub struct DiagramDecl {
    /// The node number as written.
    pub node: Spanned<String>,
    /// The title, if written.
    pub title: Option<Spanned<String>>,
    /// `status=`, `c=`, `author=`, `date=`.
    pub attrs: Vec<Attr>,
    /// The statements.
    pub stmts: Vec<Stmt>,
    /// The header (`diagram NODE … {`).
    pub span: Span,
}

/// A diagram statement.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `description TEXT origin*`.
    Description(Spanned<String>, Vec<Attr>),
    /// `box …`.
    Box(BoxStmt),
    /// `arrow …`.
    Arrow(ArrowStmt),
    /// `call …`.
    Call(CallStmt),
    /// `note …`.
    Note(NoteStmt),
    /// `layout { … }`.
    Layout(Span),
}

/// `box BOXNO "name"? key=? origin* ({ description TEXT origin* })?`.
#[derive(Debug, Clone)]
pub struct BoxStmt {
    /// The box number.
    pub number: Spanned<i64>,
    /// The name, if written.
    pub name: Option<Spanned<String>>,
    /// `key=`, `origin=`, `source=`.
    pub attrs: Vec<Attr>,
    /// The description block.
    pub description: Option<(Spanned<String>, Vec<Attr>)>,
    /// The statement.
    pub span: Span,
}

/// An arrow end as written (§16.2 `src`/`dst`). Roles are checked by the type check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// `BOXNO SIDE RANK?`, such as `1O1`, `2I`, `-1O2`.
    Box {
        /// The box number.
        number: i64,
        /// The side.
        side: Side,
        /// The rank, if written.
        rank: Option<u8>,
    },
    /// `S n` or `[S n]`.
    Boundary {
        /// The side.
        side: Side,
        /// The code or border ordinal.
        n: u8,
        /// Written in brackets: an uncoded border-tunnel port.
        border: bool,
    },
}

/// A concept reference (`cref`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cref {
    /// `@key` or `ABBR/@key`, as written.
    Key(String),
    /// A quoted label.
    Text(String),
}

/// `arrow cref? : …`.
#[derive(Debug, Clone)]
pub struct ArrowStmt {
    /// The trunk's concept reference.
    pub cref: Option<Spanned<Cref>>,
    /// The shape.
    pub shape: Shape,
    /// The trunk's attributes.
    pub attrs: Vec<Attr>,
    /// The statement's first line.
    pub span: Span,
}

/// The shape of an arrow statement.
#[derive(Debug, Clone)]
pub enum Shape {
    /// `src -> dst`.
    Plain(Spanned<End>, Spanned<End>),
    /// `src -> fork { branch+ }`.
    Fork(Spanned<End>, Vec<Part>),
    /// `join { leaf+ } -> dst`.
    Join(Vec<Part>, Spanned<End>),
}

/// A fork branch (`cref? -> dst attrs`) or a join leaf (`cref? : src attrs`).
#[derive(Debug, Clone)]
pub struct Part {
    /// The concept reference.
    pub cref: Option<Spanned<Cref>>,
    /// The branch destination or leaf source.
    pub end: Spanned<End>,
    /// The attributes.
    pub attrs: Vec<Attr>,
    /// The line.
    pub span: Span,
}

/// `call BOXNO -> callref (| callref)* note=? map=?`.
#[derive(Debug, Clone)]
pub struct CallStmt {
    /// The calling box's number.
    pub number: Spanned<i64>,
    /// The targets as written.
    pub targets: Vec<Spanned<String>>,
    /// `note=`, `map=`.
    pub attrs: Vec<Attr>,
    /// The statement.
    pub span: Span,
}

/// A note target as written (§16.2 `note_target`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteTargetAst {
    /// `box BOXNO`.
    Box(i64),
    /// `call BOXNO`.
    Call(i64),
    /// `src -> dst #n`.
    Arrow(End, End, u32),
    /// `@key`, an arrow key.
    Key(String),
    /// `S n` or `[S n]`.
    Port(End),
}

/// `note NUM TEXT (on target, …)?`.
#[derive(Debug, Clone)]
pub struct NoteStmt {
    /// The note number.
    pub number: Spanned<u32>,
    /// The text.
    pub text: Spanned<String>,
    /// The targets.
    pub targets: Vec<Spanned<NoteTargetAst>>,
    /// The statement.
    pub span: Span,
}

/// The parser's result: the tree, the AST and the syntax errors.
#[derive(Debug, Clone)]
pub struct ParseOutput {
    /// The lossless tree.
    pub cst: Node,
    /// The AST.
    pub ast: Ast,
    /// `E-PARSE-*` findings.
    pub errors: Vec<Diagnostic>,
}

const TOP_KEYWORDS: [&str; 3] = ["model", "concept", "diagram"];
/// Inside a diagram, recovery also stops at a top-level keyword, in case the `}` is missing.
const RESYNC_IN_DIAGRAM: [&str; 9] = [
    "box",
    "arrow",
    "call",
    "note",
    "layout",
    "description",
    "model",
    "concept",
    "diagram",
];

/// A syntax error that aborts the current statement.
struct Fail;

type P<T> = Result<T, Fail>;

struct Parser<'s> {
    src: &'s str,
    tokens: &'s [Token],
    pos: usize,
    lines: &'s LineIndex,
    cst: Builder,
    errors: Vec<Diagnostic>,
}

/// Parse `tokens` of `src`.
pub fn parse(src: &str, tokens: &[Token], lines: &LineIndex) -> ParseOutput {
    let mut p = Parser {
        src,
        tokens,
        pos: 0,
        lines,
        cst: Builder::new(),
        errors: Vec::new(),
    };
    let ast = p.file();
    ParseOutput {
        cst: p.cst.root(),
        ast,
        errors: p.errors,
    }
}

impl<'s> Parser<'s> {
    // ----- token access ------------------------------------------------------------------

    /// Move trivia into the current node.
    fn trivia(&mut self) {
        while let Some(t) = self.tokens.get(self.pos).filter(|t| t.kind.is_trivia()) {
            self.cst.token(*t);
            self.pos += 1;
        }
    }

    /// The next significant token, without consuming it.
    fn peek(&self) -> Option<Token> {
        self.tokens[self.pos..]
            .iter()
            .find(|t| !t.kind.is_trivia())
            .copied()
    }

    /// The significant token after the next one.
    fn peek2(&self) -> Option<Token> {
        self.tokens[self.pos..]
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .nth(1)
            .copied()
    }

    fn text(&self, t: Token) -> &'s str {
        t.text(self.src)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|t| t.kind == kind)
    }

    fn at_word(&self, w: &str) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Word && self.text(t) == w)
    }

    /// Consume the next significant token (and the trivia before it).
    fn bump(&mut self) -> Option<Token> {
        self.trivia();
        let t = self.tokens.get(self.pos).copied()?;
        self.cst.token(t);
        self.pos += 1;
        Some(t)
    }

    /// Whether a newline comes before the next significant token.
    fn newline_ahead(&self) -> bool {
        self.tokens[self.pos..]
            .iter()
            .take_while(|t| t.kind.is_trivia())
            .any(|t| t.kind == TokenKind::Newline)
    }

    fn span(&self, t: Token) -> Span {
        self.lines.span(t.start, t.end)
    }

    fn span_from(&self, first: Token) -> Span {
        let last = self.tokens[..self.pos]
            .iter()
            .rev()
            .find(|t| !t.kind.is_trivia())
            .copied()
            .unwrap_or(first);
        self.lines.span(first.start, last.end.max(first.end))
    }

    fn error_at(&mut self, code: Code, span: Span, message: String) {
        if self.errors.len() < MAX_ERRORS {
            self.errors
                .push(Diagnostic::fatal(code, SubjectRef::Model, message).at(span));
        }
    }

    /// Report "expected …" at the next token and fail the statement.
    fn expected<T>(&mut self, what: &str) -> P<T> {
        let (span, found) = if let Some(t) = self.peek() {
            (self.span(t), format!("`{}`", self.text(t).escape_debug()))
        } else {
            let end = u32::try_from(self.src.len()).unwrap_or(u32::MAX);
            (self.lines.span(end, end), "end of file".to_owned())
        };
        self.error_at(E_PARSE_01, span, format!("expected {what}, found {found}"));
        Err(Fail)
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> P<Token> {
        if self.at(kind) {
            self.bump().ok_or(Fail)
        } else {
            self.expected(what)
        }
    }

    fn expect_word(&mut self, w: &str) -> P<Token> {
        if self.at_word(w) {
            self.bump().ok_or(Fail)
        } else {
            self.expected(&format!("`{w}`"))
        }
    }

    fn word(&mut self, what: &str) -> P<Spanned<String>> {
        let t = self.expect(TokenKind::Word, what)?;
        Ok(Spanned {
            value: self.text(t).to_owned(),
            span: self.span(t),
        })
    }

    /// `STRING` or `TRIPLE`, decoded.
    fn text_value(&mut self, what: &str) -> P<Spanned<String>> {
        match self.peek().map(|t| t.kind) {
            Some(TokenKind::Str | TokenKind::Triple) => {
                let t = self.bump().ok_or(Fail)?;
                let span = self.span(t);
                let raw = self.text(t);
                let value = if t.kind == TokenKind::Triple {
                    triple(raw)
                } else {
                    match unescape(&raw[1..raw.len() - 1]) {
                        Ok(v) => v,
                        Err(why) => {
                            self.error_at(E_PARSE_01, span, why);
                            return Err(Fail);
                        }
                    }
                };
                Ok(Spanned { value, span })
            }
            Some(TokenKind::UnterminatedStr) => {
                let t = self.bump().ok_or(Fail)?;
                let span = self.span(t);
                self.error_at(E_PARSE_01, span, "unterminated string".to_owned());
                Err(Fail)
            }
            _ => self.expected(what),
        }
    }

    fn string(&mut self, what: &str) -> P<Spanned<String>> {
        if self.at(TokenKind::Triple) {
            return self.expected(what);
        }
        self.text_value(what)
    }

    fn integer(&mut self, what: &str) -> P<Spanned<i64>> {
        let w = self.word(what)?;
        match w.value.parse::<i64>() {
            Ok(n) if !w.value.starts_with('+') => Ok(Spanned {
                value: n,
                span: w.span,
            }),
            _ => {
                self.error_at(
                    E_PARSE_01,
                    w.span,
                    format!("expected {what}, found `{}`", w.value),
                );
                Err(Fail)
            }
        }
    }

    /// `name=value` attributes, while the next tokens look like one.
    fn attrs(&mut self) -> P<Vec<Attr>> {
        let mut out = Vec::new();
        while self.at(TokenKind::Word) && self.peek2().is_some_and(|t| t.kind == TokenKind::Eq) {
            let name = self.word("an attribute")?;
            self.expect(TokenKind::Eq, "`=`")?;
            if self.newline_ahead() {
                let span = name.span;
                self.error_at(
                    E_PARSE_01,
                    span,
                    format!("`{}=` needs a value on the same line", name.value),
                );
                return Err(Fail);
            }
            let value = if let Some(TokenKind::Str | TokenKind::UnterminatedStr) =
                self.peek().map(|t| t.kind)
            {
                let v = self.string("a value")?;
                Spanned {
                    value: AttrValue::Text(v.value),
                    span: v.span,
                }
            } else {
                let v = self.word("a value")?;
                Spanned {
                    value: AttrValue::Word(v.value),
                    span: v.span,
                }
            };
            out.push(Attr { name, value });
        }
        Ok(out)
    }

    // ----- recovery ------------------------------------------------------------------------

    /// Whether the token at `i` is the first significant token on its line.
    fn starts_line(&self, i: usize) -> bool {
        self.tokens[..i]
            .iter()
            .rev()
            .find(|t| t.kind != TokenKind::Whitespace)
            .is_none_or(|t| t.kind == TokenKind::Newline)
    }

    /// Skip into an error node up to the next line that starts with one of `keywords`, or a
    /// `}` at brace depth 0 when `stop_at_brace`. When the failed statement consumed nothing
    /// (`stuck`), at least one token is skipped so that parsing always advances.
    fn recover(&mut self, keywords: &[&str], stop_at_brace: bool, stuck: bool) {
        self.cst.start(NodeKind::Error);
        let mut depth = 0usize;
        let mut first = stuck;
        while let Some(i) =
            (self.pos..self.tokens.len()).find(|&i| !self.tokens[i].kind.is_trivia())
        {
            let t = self.tokens[i];
            let word = (t.kind == TokenKind::Word).then(|| self.text(t));
            if !first && depth == 0 {
                if word.is_some_and(|w| keywords.contains(&w)) && self.starts_line(i) {
                    break;
                }
                if stop_at_brace && t.kind == TokenKind::RBrace {
                    break;
                }
            }
            match t.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth = depth.saturating_sub(1),
                _ => {}
            }
            first = false;
            self.bump();
        }
        self.cst.finish();
    }

    /// Whether the next token is `model`, `concept` or `diagram` at the start of a line.
    fn at_top_keyword_line(&self) -> bool {
        let Some(i) = (self.pos..self.tokens.len()).find(|&i| !self.tokens[i].kind.is_trivia())
        else {
            return false;
        };
        let t = self.tokens[i];
        t.kind == TokenKind::Word && TOP_KEYWORDS.contains(&self.text(t)) && self.starts_line(i)
    }

    // ----- file and top level --------------------------------------------------------------

    fn file(&mut self) -> Ast {
        let mut ast = Ast {
            header: self.header(),
            ..Ast::default()
        };
        while self.peek().is_some() && self.errors.len() < MAX_ERRORS {
            let start = self.pos;
            let ok = if self.at_word("model") {
                self.model().map(|m| {
                    if ast.model.is_some() {
                        let span = m.span;
                        self.error_at(
                            E_PARSE_01,
                            span,
                            "a file has one `model` declaration (FR-WS-02)".into(),
                        );
                    } else {
                        ast.model = Some(m);
                    }
                })
            } else if self.at_word("concept") {
                self.concept().map(|c| ast.concepts.push(c))
            } else if self.at_word("diagram") {
                self.diagram().map(|d| ast.diagrams.push(d))
            } else {
                self.expected("`model`, `concept` or `diagram`")
            };
            if ok.is_err() {
                let stuck = self.pos == start;
                self.recover(&TOP_KEYWORDS, false, stuck);
            }
        }
        self.trivia();
        ast
    }

    fn header(&mut self) -> Option<Spanned<(u32, u32)>> {
        if !self.at_word("idef0") {
            return None;
        }
        self.cst.start(NodeKind::Header);
        let kw = self.bump()?;
        let version = self
            .peek()
            .filter(|t| t.kind == TokenKind::Word)
            .and_then(|t| {
                let (major, minor) = self.text(t).split_once('.')?;
                Some((major.parse().ok()?, minor.parse().ok()?))
            });
        let out = if let Some(v) = version {
            self.bump();
            Some(Spanned {
                value: v,
                span: self.span_from(kw),
            })
        } else {
            let span = self.span(kw);
            self.error_at(
                E_PARSE_01,
                span,
                "expected a version such as `0.2` after `idef0`".into(),
            );
            None
        };
        self.cst.finish();
        out
    }

    fn model(&mut self) -> P<ModelDecl> {
        self.cst.start(NodeKind::Model);
        let r = self.model_inner();
        self.cst.finish();
        r
    }

    fn model_inner(&mut self) -> P<ModelDecl> {
        let kw = self.expect_word("model")?;
        let name = self.string("the model name")?;
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) && self.peek().is_some() {
            self.cst.start(NodeKind::MetaField);
            let f = self.meta_field();
            self.cst.finish();
            match f {
                Ok(f) => fields.push(f),
                Err(Fail) => self.recover_in_block(),
            }
        }
        self.expect(TokenKind::RBrace, "`}`")?;
        Ok(ModelDecl {
            name,
            fields,
            span: self.span_from(kw),
        })
    }

    /// Skip to the next line inside a `{ … }` block, or its closing `}`.
    fn recover_in_block(&mut self) {
        self.cst.start(NodeKind::Error);
        let mut first = true;
        while let Some(i) =
            (self.pos..self.tokens.len()).find(|&i| !self.tokens[i].kind.is_trivia())
        {
            let t = self.tokens[i];
            if !first && (t.kind == TokenKind::RBrace || self.starts_line(i)) {
                break;
            }
            first = false;
            self.bump();
        }
        self.cst.finish();
    }

    fn meta_field(&mut self) -> P<MetaField> {
        let name = self.word("a model field")?;
        let value = match name.value.as_str() {
            "profile" => MetaValue::Word(self.word("`extended` or `fips183`")?.value),
            "retired" => {
                self.expect(TokenKind::LBrace, "`{`")?;
                let mut keys = Vec::new();
                while self.at(TokenKind::Word) {
                    keys.push(self.word("a key")?);
                }
                self.expect(TokenKind::RBrace, "`}`")?;
                MetaValue::Keys(keys)
            }
            _ => MetaValue::Text(self.text_value("a text value")?.value),
        };
        Ok(MetaField { name, value })
    }

    fn concept(&mut self) -> P<ConceptDecl> {
        self.cst.start(NodeKind::Concept);
        let r = self.concept_inner();
        self.cst.finish();
        r
    }

    fn concept_inner(&mut self) -> P<ConceptDecl> {
        let kw = self.expect_word("concept")?;
        let key = self.word("a concept key")?;
        let name = self.string("the concept name")?;
        let attrs = self.attrs()?;
        let mut fields = Vec::new();
        let span = self.span_from(kw);
        if self.at(TokenKind::LBrace) {
            self.bump();
            while !self.at(TokenKind::RBrace) && self.peek().is_some() {
                self.cst.start(NodeKind::ConceptField);
                let f = self.concept_field();
                self.cst.finish();
                match f {
                    Ok(f) => fields.push(f),
                    Err(Fail) => self.recover_in_block(),
                }
            }
            self.expect(TokenKind::RBrace, "`}`")?;
        }
        Ok(ConceptDecl {
            key,
            name,
            attrs,
            fields,
            span,
        })
    }

    fn list(&mut self, item: fn(&mut Self) -> P<Spanned<String>>) -> P<Vec<Spanned<String>>> {
        let mut out = vec![item(self)?];
        while self.at(TokenKind::Comma) {
            self.bump();
            out.push(item(self)?);
        }
        Ok(out)
    }

    fn concept_field(&mut self) -> P<ConceptField> {
        let name = self.word("a concept field")?;
        Ok(match name.value.as_str() {
            "description" => {
                let text = self.text_value("a description")?;
                ConceptField::Description(text, self.attrs()?)
            }
            "synonyms" => ConceptField::Synonyms(self.list(|p| p.string("a synonym"))?),
            "kind_of" => ConceptField::KindOf(self.list(|p| p.word("a concept key"))?),
            "part_of" => ConceptField::PartOf(self.list(|p| p.word("a concept key"))?),
            "example" => ConceptField::Example(self.text_value("an example")?.value),
            "source" => ConceptField::Source(self.string("a source")?.value),
            other => {
                let span = name.span;
                self.error_at(
                    crate::diag::E_PARSE_02,
                    span,
                    format!("unknown concept field `{other}`"),
                );
                return Err(Fail);
            }
        })
    }

    // ----- diagrams --------------------------------------------------------------------------

    fn diagram(&mut self) -> P<DiagramDecl> {
        self.cst.start(NodeKind::Diagram);
        let r = self.diagram_inner();
        self.cst.finish();
        r
    }

    fn diagram_inner(&mut self) -> P<DiagramDecl> {
        let kw = self.expect_word("diagram")?;
        let node = self.word("a node number")?;
        let title = if self.at(TokenKind::Str) || self.at(TokenKind::UnterminatedStr) {
            Some(self.string("the title")?)
        } else {
            None
        };
        let attrs = self.attrs()?;
        self.expect(TokenKind::LBrace, "`{`")?;
        let span = self.span_from(kw);
        let mut stmts = Vec::new();
        while !self.at(TokenKind::RBrace) && self.peek().is_some() && self.errors.len() < MAX_ERRORS
        {
            if self.at_top_keyword_line() {
                break; // a missing `}`: let the diagram end here
            }
            let start = self.pos;
            if let Ok(s) = self.stmt() {
                stmts.push(s);
            } else {
                let stuck = self.pos == start;
                self.recover(&RESYNC_IN_DIAGRAM, true, stuck);
            }
        }
        self.expect(TokenKind::RBrace, "`}` to close the diagram")?;
        Ok(DiagramDecl {
            node,
            title,
            attrs,
            stmts,
            span,
        })
    }

    fn stmt(&mut self) -> P<Stmt> {
        let kind = match self
            .peek()
            .filter(|t| t.kind == TokenKind::Word)
            .map(|t| self.text(t))
        {
            Some("description") => NodeKind::Description,
            Some("box") => NodeKind::Box,
            Some("arrow") => NodeKind::Arrow,
            Some("call") => NodeKind::Call,
            Some("note") => NodeKind::Note,
            Some("layout") => NodeKind::Layout,
            _ => return self.expected("`box`, `arrow`, `call`, `note`, `layout` or `description`"),
        };
        self.cst.start(kind);
        let r = match kind {
            NodeKind::Description => self.description_stmt(),
            NodeKind::Box => self.box_stmt().map(Stmt::Box),
            NodeKind::Arrow => self.arrow_stmt().map(Stmt::Arrow),
            NodeKind::Call => self.call_stmt().map(Stmt::Call),
            NodeKind::Note => self.note_stmt().map(Stmt::Note),
            _ => self.layout_stmt(),
        };
        self.cst.finish();
        r
    }

    fn description_stmt(&mut self) -> P<Stmt> {
        self.bump();
        let text = self.text_value("a description")?;
        Ok(Stmt::Description(text, self.attrs()?))
    }

    fn layout_stmt(&mut self) -> P<Stmt> {
        let kw = self.bump().ok_or(Fail)?;
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut depth = 1usize;
        while depth > 0 {
            match self.bump() {
                Some(t) if t.kind == TokenKind::LBrace => depth += 1,
                Some(t) if t.kind == TokenKind::RBrace => depth -= 1,
                Some(_) => {}
                None => return self.expected("`}` to close the layout block"),
            }
        }
        Ok(Stmt::Layout(self.span_from(kw)))
    }

    fn box_stmt(&mut self) -> P<BoxStmt> {
        let kw = self.bump().ok_or(Fail)?;
        let number = self.integer("a box number")?;
        let name = if self.at(TokenKind::Str) || self.at(TokenKind::UnterminatedStr) {
            Some(self.string("the box name")?)
        } else {
            None
        };
        let attrs = self.attrs()?;
        let span = self.span_from(kw);
        let mut description = None;
        if self.at(TokenKind::LBrace) {
            self.bump();
            if !self.at(TokenKind::RBrace) {
                self.expect_word("description")?;
                let text = self.text_value("a description")?;
                description = Some((text, self.attrs()?));
            }
            self.expect(TokenKind::RBrace, "`}`")?;
        }
        Ok(BoxStmt {
            number,
            name,
            attrs,
            description,
            span,
        })
    }

    /// `cref?`: `@key`, `ABBR/@key` or a quoted label.
    fn cref(&mut self) -> P<Option<Spanned<Cref>>> {
        match self.peek() {
            Some(t) if t.kind == TokenKind::Word && self.text(t).contains('@') => {
                let w = self.word("a concept reference")?;
                Ok(Some(Spanned {
                    value: Cref::Key(w.value),
                    span: w.span,
                }))
            }
            Some(t) if matches!(t.kind, TokenKind::Str | TokenKind::UnterminatedStr) => {
                let s = self.string("a label")?;
                Ok(Some(Spanned {
                    value: Cref::Text(s.value),
                    span: s.span,
                }))
            }
            _ => Ok(None),
        }
    }

    /// An endpoint: `1O1`, `2I`, `-1O2`, `I1` or `[I1]`.
    fn end(&mut self, what: &str) -> P<Spanned<End>> {
        if self.at(TokenKind::LBracket) {
            let open = self.bump().ok_or(Fail)?;
            let w = self.word("a port such as `I1`")?;
            let parsed = parse_port(&w.value);
            self.expect(TokenKind::RBracket, "`]`")?;
            let span = self.span_from(open);
            return if let Some((side, n)) = parsed {
                Ok(Spanned {
                    value: End::Boundary {
                        side,
                        n,
                        border: true,
                    },
                    span,
                })
            } else {
                self.error_at(
                    E_PARSE_01,
                    w.span,
                    format!("expected a port such as `I1`, found `{}`", w.value),
                );
                Err(Fail)
            };
        }
        let w = self.word(what)?;
        if let Some(value) = parse_end(&w.value) {
            Ok(Spanned {
                value,
                span: w.span,
            })
        } else {
            self.error_at(
                E_PARSE_01,
                w.span,
                format!("expected {what}, found `{}`", w.value),
            );
            Err(Fail)
        }
    }

    fn arrow_stmt(&mut self) -> P<ArrowStmt> {
        let kw = self.bump().ok_or(Fail)?;
        let cref = self.cref()?;
        self.expect(TokenKind::Colon, "`:`")?;
        if self.at_word("join") {
            self.bump();
            let attrs = self.attrs()?;
            let span = self.span_from(kw);
            let leaves = self.parts(false)?;
            self.expect(TokenKind::Arrow, "`->`")?;
            let dst = self.end("a destination such as `O1`")?;
            return Ok(ArrowStmt {
                cref,
                shape: Shape::Join(leaves, dst),
                attrs,
                span,
            });
        }
        let src = self.end("a source such as `1O1` or `I1`")?;
        self.expect(TokenKind::Arrow, "`->`")?;
        if self.at_word("fork") {
            self.bump();
            let attrs = self.attrs()?;
            let span = self.span_from(kw);
            let branches = self.parts(true)?;
            return Ok(ArrowStmt {
                cref,
                shape: Shape::Fork(src, branches),
                attrs,
                span,
            });
        }
        let dst = self.end("a destination such as `2I1` or `O1`")?;
        let attrs = self.attrs()?;
        Ok(ArrowStmt {
            cref,
            shape: Shape::Plain(src, dst),
            attrs,
            span: self.span_from(kw),
        })
    }

    /// `{ branch+ }` (fork) or `{ leaf+ }` (join).
    fn parts(&mut self, fork: bool) -> P<Vec<Part>> {
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut out = Vec::new();
        while !self.at(TokenKind::RBrace) && self.peek().is_some() {
            self.cst.start(if fork {
                NodeKind::Branch
            } else {
                NodeKind::Leaf
            });
            let r = self.part(fork);
            self.cst.finish();
            out.push(r?);
        }
        let close = self.expect(TokenKind::RBrace, "`}`")?;
        if out.is_empty() {
            let span = self.span(close);
            let what = if fork {
                "a fork needs at least one branch"
            } else {
                "a join needs at least one leaf"
            };
            self.error_at(E_PARSE_01, span, what.into());
            return Err(Fail);
        }
        Ok(out)
    }

    fn part(&mut self, fork: bool) -> P<Part> {
        let first = self.peek().ok_or(Fail)?;
        let cref = self.cref()?;
        let end = if fork {
            self.expect(TokenKind::Arrow, "`->` to start a branch")?;
            self.end("a branch destination such as `3I1`")?
        } else {
            self.expect(TokenKind::Colon, "`:` to start a leaf")?;
            self.end("a leaf source such as `1O1`")?
        };
        let attrs = self.attrs()?;
        Ok(Part {
            cref,
            end,
            attrs,
            span: self.span_from(first),
        })
    }

    fn call_stmt(&mut self) -> P<CallStmt> {
        let kw = self.bump().ok_or(Fail)?;
        let number = self.integer("a box number")?;
        self.expect(TokenKind::Arrow, "`->`")?;
        let mut targets = vec![self.word("a call target such as `@key`")?];
        while self.at(TokenKind::Pipe) {
            self.bump();
            targets.push(self.word("a call target such as `@key`")?);
        }
        let attrs = self.attrs()?;
        Ok(CallStmt {
            number,
            targets,
            attrs,
            span: self.span_from(kw),
        })
    }

    fn note_stmt(&mut self) -> P<NoteStmt> {
        let kw = self.bump().ok_or(Fail)?;
        let n = self.integer("a note number")?;
        let number = if let Ok(v) = u32::try_from(n.value) {
            Spanned {
                value: v,
                span: n.span,
            }
        } else {
            self.error_at(E_PARSE_01, n.span, "a note number is 1 or more".into());
            return Err(Fail);
        };
        let text = self.text_value("the note text")?;
        let mut targets = Vec::new();
        if self.at_word("on") {
            self.bump();
            targets.push(self.note_target()?);
            while self.at(TokenKind::Comma) {
                self.bump();
                targets.push(self.note_target()?);
            }
        }
        Ok(NoteStmt {
            number,
            text,
            targets,
            span: self.span_from(kw),
        })
    }

    fn note_target(&mut self) -> P<Spanned<NoteTargetAst>> {
        let first = self.peek().ok_or(Fail)?;
        let value = if self.at_word("box") || self.at_word("call") {
            let is_box = self.at_word("box");
            self.bump();
            let n = self.integer("a box number")?.value;
            if is_box {
                NoteTargetAst::Box(n)
            } else {
                NoteTargetAst::Call(n)
            }
        } else if self
            .peek()
            .is_some_and(|t| t.kind == TokenKind::Word && self.text(t).starts_with('@'))
        {
            let w = self.word("an arrow key")?;
            NoteTargetAst::Key(w.value[1..].to_owned())
        } else {
            let a = self.end("a note target")?.value;
            if self.at(TokenKind::Arrow) {
                self.bump();
                let b = self.end("an arrow destination")?.value;
                let occurrence = if self.at(TokenKind::Occurrence) {
                    let t = self.bump().ok_or(Fail)?;
                    self.text(t)[1..].parse().unwrap_or(u32::MAX)
                } else {
                    0
                };
                NoteTargetAst::Arrow(a, b, occurrence)
            } else if matches!(a, End::Boundary { .. }) {
                NoteTargetAst::Port(a)
            } else {
                return self.expected("`->` after an arrow source");
            }
        };
        Ok(Spanned {
            value,
            span: self.span_from(first),
        })
    }
}

/// `S n`: a side letter and a code from 1 to 255.
fn parse_port(s: &str) -> Option<(Side, u8)> {
    let side = Side::from_letter(*s.as_bytes().first()?)?;
    let n: u8 = s.get(1..)?.parse().ok()?;
    (n >= 1 && !s[1..].starts_with('0') && !s[1..].starts_with('+')).then_some((side, n))
}

/// `BOXNO SIDE RANK?` or `S n`.
fn parse_end(s: &str) -> Option<End> {
    if let Some((side, n)) = parse_port(s) {
        return Some(End::Boundary {
            side,
            n,
            border: false,
        });
    }
    let split = s.find(|c: char| c.is_ascii_uppercase())?;
    let (num, rest) = s.split_at(split);
    let number: i64 = num.parse().ok()?;
    if num.starts_with('+') {
        return None;
    }
    let side = Side::from_letter(*rest.as_bytes().first()?)?;
    let rank = match &rest[1..] {
        "" => None,
        r if r.starts_with('0') || r.starts_with('+') => return None,
        r => Some(r.parse::<u8>().ok().filter(|&n| n >= 1)?),
    };
    Some(End::Box { number, side, rank })
}

/// Decode `\" \\ \n \t \u{…}` (metamodel §16.1).
fn unescape(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let rest: String = chars.clone().collect();
                let body = rest
                    .strip_prefix('{')
                    .and_then(|r| r.split_once('}'))
                    .map(|(hex, _)| hex.to_owned())
                    .ok_or_else(|| "expected `\\u{…}`".to_owned())?;
                let ch = u32::from_str_radix(&body, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("`\\u{{{body}}}` is not a character"))?;
                out.push(ch);
                for _ in 0..body.len() + 2 {
                    chars.next();
                }
            }
            Some(other) => return Err(format!("unknown escape `\\{other}`")),
            None => return Err("a string cannot end with `\\`".to_owned()),
        }
    }
    Ok(out)
}

/// The body of a `"""…"""` block with its common indentation stripped (metamodel §16.1): a
/// newline right after the opening quotes and a whitespace-only last line are dropped.
fn triple(raw: &str) -> String {
    let body = &raw[3..raw.len() - 3];
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
        .unwrap_or(body);
    let mut lines: Vec<&str> = body.lines().collect();
    if lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| l.get(indent..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints() {
        assert_eq!(
            parse_end("1O1"),
            Some(End::Box {
                number: 1,
                side: Side::O,
                rank: Some(1)
            })
        );
        assert_eq!(
            parse_end("2I"),
            Some(End::Box {
                number: 2,
                side: Side::I,
                rank: None
            })
        );
        assert_eq!(
            parse_end("-1O2"),
            Some(End::Box {
                number: -1,
                side: Side::O,
                rank: Some(2)
            })
        );
        assert_eq!(
            parse_end("C3"),
            Some(End::Boundary {
                side: Side::C,
                n: 3,
                border: false
            })
        );
        for bad in [
            "", "1", "O", "1X1", "1O0", "1O01", "C0", "C01", "+1O1", "1O1x", "I256",
        ] {
            assert_eq!(parse_end(bad), None, "{bad:?}");
        }
    }

    // metamodel §16.1: escapes and triple-quoted blocks.
    #[test]
    fn text_values() {
        assert_eq!(
            unescape(r#"a \"b\" \\ \n\t \u{e9}"#).unwrap(),
            "a \"b\" \\ \n\t é"
        );
        assert!(unescape(r"\q").is_err());
        assert!(unescape(r"\u{110000}").is_err());
        assert_eq!(
            triple("\"\"\"\n    First line.\n      Indented.\n    \"\"\""),
            "First line.\n  Indented."
        );
        assert_eq!(triple("\"\"\"one line\"\"\""), "one line");
    }
}
