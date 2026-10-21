//! Lowering (metamodel §16.3): the AST to a [`Model`] and a [`SpanMap`].
//!
//! - Box numbers are checked against statement order (`E-NODE-02`) and mapped to keys.
//! - Boxes without `key=` get the slug rule's key, in file declaration order (§3.3).
//! - Omitted port ranks are assigned in declaration order after explicit ranks (§16.3).
//! - The A-1 lineage box takes the A-0 top box's name and key (§16.3).
//! - Node-number call targets in the same model are resolved to keys (FR-CALL-12).
//! - A quoted label resolves to the concept with that name or synonym (FR-NAME-03).

#![allow(
    clippy::many_single_char_names,
    reason = "the passes name their state consistently: l = Lower, p = Pending, a/c/n = the statement"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use nonempty::NonEmpty;

use super::SpanMap;
use super::parse::{
    ArrowStmt, Ast, Attr, AttrValue, BoxStmt, CallStmt, ConceptDecl, ConceptField, Cref,
    DiagramDecl, End, MetaValue, NoteStmt, NoteTargetAst, Part, Shape, Spanned, Stmt,
};
use crate::diag::{
    Code, Diagnostic, E_CALL_01, E_ID_01, E_ID_03, E_ID_05, E_LIMIT_01, E_NODE_01, E_NODE_02,
    E_NOTE_01, E_PARSE_01, E_PARSE_02, E_TYPE_01, E_TYPE_02, Span, SubjectRef,
};
use crate::id::{ModelAbbrev, NodeId, PortCode, RefError, Side};
use crate::identity::{ArrowId, KeyMinter, SlugMinter, arrow_ids};
use crate::model::{
    Arrow, ArrowAttrs, ArrowShape, BoundaryPort, Branch, CallArrow, CallPortMap, Concept,
    ConceptRef, Diagram, DiagramKind, Endpoint, FunctionBox, KeyRef, Leaf, MechKind, Meta, Model,
    ModelNote, NoteTarget, Rel, Status, Tunnel,
};
use crate::profile::Profile;
use crate::text::{
    ArrowKey, BoxKey, CNumber, ConceptKey, Description, LangTag, NonEmptyString, Origin, Provenance,
};

/// architecture §8.3 hard limits.
pub(crate) mod limits {
    pub(crate) const DIAGRAMS: usize = 1_000;
    pub(crate) const ARROWS_PER_DIAGRAM: usize = 500;
    pub(crate) const DEPTH_BELOW_A0: usize = 16;
    pub(crate) const CONCEPTS: usize = 20_000;
    pub(crate) const DESCRIPTION_BYTES: usize = 16 * 1024;
    pub(crate) const NAME_CHARS: usize = 256;
}

pub(crate) struct Lowered {
    pub model: Option<Model>,
    pub spans: SpanMap,
    pub diagnostics: Vec<Diagnostic>,
}

struct Lower {
    out: Vec<Diagnostic>,
    spans: SpanMap,
}

impl Lower {
    fn err(&mut self, code: Code, subject: SubjectRef, span: Span, message: impl Into<String>) {
        self.out
            .push(Diagnostic::fatal(code, subject, message).at(span));
    }

    fn parse_err(&mut self, span: Span, message: impl Into<String>) {
        self.err(E_PARSE_01, SubjectRef::Model, span, message);
    }

    fn unknown_attr(&mut self, a: &Attr, on: &str) {
        let msg = format!("unknown attribute `{}` on {on}", a.name.value);
        self.err(E_PARSE_02, SubjectRef::Model, a.name.span, msg);
    }

    fn name(&mut self, s: &Spanned<String>, what: &str) -> Option<NonEmptyString> {
        if s.value.chars().count() > limits::NAME_CHARS {
            let msg = format!("{what} is longer than {} characters", limits::NAME_CHARS);
            self.err(E_LIMIT_01, SubjectRef::Model, s.span, msg);
            return None;
        }
        if let Ok(n) = NonEmptyString::new(&s.value) {
            Some(n)
        } else {
            self.parse_err(s.span, format!("{what} must not be empty"));
            None
        }
    }

    fn description(&mut self, s: &Spanned<String>, attrs: &[Attr], on: &str) -> Description {
        if s.value.len() > limits::DESCRIPTION_BYTES {
            let msg = format!(
                "description is longer than {} bytes",
                limits::DESCRIPTION_BYTES
            );
            self.err(E_LIMIT_01, SubjectRef::Model, s.span, msg);
        }
        Description {
            text: s.value.clone(),
            provenance: self.provenance(attrs, on),
        }
    }

    /// `origin=` and `source=`; any other attribute is unknown.
    fn provenance(&mut self, attrs: &[Attr], on: &str) -> Provenance {
        let mut p = Provenance::default();
        for a in attrs {
            match (a.name.value.as_str(), &a.value.value) {
                ("origin", AttrValue::Word(w)) => match w.as_str() {
                    "model" => p.origin = Origin::Model,
                    "prd" => p.origin = Origin::Prd,
                    "code" => p.origin = Origin::Code,
                    "llm" => p.origin = Origin::Llm,
                    _ => self.parse_err(
                        a.value.span,
                        format!("`origin` is model, prd, code or llm, not `{w}`"),
                    ),
                },
                ("source", AttrValue::Text(t)) => p.source = Some(t.clone()),
                ("origin" | "source", _) => {
                    self.parse_err(a.value.span, format!("bad value for `{}`", a.name.value));
                }
                _ => self.unknown_attr(a, on),
            }
        }
        p
    }

    fn key<K: core::str::FromStr>(
        &mut self,
        s: &Spanned<String>,
        subject: SubjectRef,
    ) -> Option<K> {
        if let Ok(k) = s.value.parse() {
            Some(k)
        } else {
            let msg = format!(
                "`{}` is not a valid key: lowercase letters, digits and `-`",
                s.value
            );
            self.err(E_ID_05, subject, s.span, msg);
            None
        }
    }
}

pub(crate) fn lower(ast: &Ast) -> Lowered {
    let mut l = Lower {
        out: Vec::new(),
        spans: SpanMap::new(),
    };
    let meta = ast.model.as_ref().and_then(|m| meta(&mut l, m));
    if ast.model.is_none() {
        l.parse_err(Span::default(), "the file has no `model` declaration");
    }
    let Some(meta) = meta else {
        return Lowered {
            model: None,
            spans: l.spans,
            diagnostics: l.out,
        };
    };
    let mut model = Model::new(meta);
    concepts(&mut l, ast, &mut model);
    diagrams(&mut l, ast, &mut model);
    Lowered {
        model: Some(model),
        spans: l.spans,
        diagnostics: l.out,
    }
}

// ----- model and concepts ---------------------------------------------------------------------

fn meta(l: &mut Lower, m: &super::parse::ModelDecl) -> Option<Meta> {
    l.spans.insert(SubjectRef::Model, m.span);
    let name = l.name(&m.name, "the model name");
    let mut seen = BTreeSet::new();
    let mut abbrev: Option<ModelAbbrev> = None;
    let mut fields: BTreeMap<&str, (&MetaValue, Span)> = BTreeMap::new();
    for f in &m.fields {
        if !seen.insert(f.name.value.as_str()) {
            l.parse_err(f.name.span, format!("`{}` is given twice", f.name.value));
            continue;
        }
        fields.insert(f.name.value.as_str(), (&f.value, f.name.span));
    }
    match fields.get("abbrev") {
        Some((MetaValue::Text(t), span)) => match t.parse() {
            Ok(a) => abbrev = Some(a),
            Err(_) => l.err(
                E_ID_01,
                SubjectRef::Model,
                *span,
                format!("`{t}` is not a model abbreviation: [A-Z][A-Z0-9]{{0,7}} (FR-ID-01)"),
            ),
        },
        _ => l.err(
            E_ID_01,
            SubjectRef::Model,
            m.span,
            "the model has no `abbrev` (FR-ID-01)",
        ),
    }
    let mut meta = Meta::new(name?, abbrev?);
    for (field, (value, span)) in fields {
        let text = |v: &MetaValue| match v {
            MetaValue::Text(t) => Some(t.clone()),
            _ => None,
        };
        match (field, value) {
            ("abbrev", _) => {}
            ("description", v) => meta.description = text(v).unwrap_or_default(),
            ("purpose", v) => meta.purpose = text(v).unwrap_or_default(),
            ("viewpoint", v) => meta.viewpoint = text(v).unwrap_or_default(),
            ("scope", v) => meta.scope = text(v),
            ("context", v) => meta.context = text(v),
            ("author", v) => meta.author = text(v),
            ("project", v) => meta.project = text(v),
            ("lang", MetaValue::Text(t)) => match t.parse::<LangTag>() {
                Ok(tag) => meta.lang = tag,
                Err(e) => l.parse_err(span, e.to_string()),
            },
            ("profile", MetaValue::Word(w)) => match w.as_str() {
                "extended" => meta.profile = Profile::Extended,
                "fips183" => meta.profile = Profile::Fips183,
                _ => l.parse_err(span, format!("`profile` is extended or fips183, not `{w}`")),
            },
            ("retired", MetaValue::Keys(keys)) => {
                for k in keys {
                    if let Some(key) = l.key::<BoxKey>(k, SubjectRef::Model) {
                        meta.retired.insert(key);
                    }
                }
            }
            (other, _) => l.err(
                E_PARSE_02,
                SubjectRef::Model,
                span,
                format!("unknown model field `{other}`"),
            ),
        }
    }
    Some(meta)
}

fn concepts(l: &mut Lower, ast: &Ast, model: &mut Model) {
    for (i, c) in ast.concepts.iter().enumerate() {
        if i == limits::CONCEPTS {
            l.err(
                E_LIMIT_01,
                SubjectRef::Model,
                c.span,
                format!("more than {} concepts", limits::CONCEPTS),
            );
            break;
        }
        let Some(key) = l.key::<ConceptKey>(&c.key, SubjectRef::Model) else {
            continue;
        };
        if model.concepts.contains_key(&key) {
            l.err(
                E_ID_03,
                SubjectRef::Concept(key.clone()),
                c.key.span,
                format!("concept key `{key}` is declared twice"),
            );
            continue;
        }
        let Some(name) = l.name(&c.name, "a concept name") else {
            continue;
        };
        let concept = concept(l, c, name);
        l.spans.insert(SubjectRef::Concept(key.clone()), c.span);
        model.concepts.insert(key, concept);
    }
}

fn concept(l: &mut Lower, c: &ConceptDecl, name: NonEmptyString) -> Concept {
    let mut out = Concept::new(name);
    out.provenance = l.provenance(&c.attrs, "a concept");
    for f in &c.fields {
        match f {
            ConceptField::Description(text, attrs) => {
                out.description = l.description(text, attrs, "a description");
            }
            ConceptField::Synonyms(names) => out
                .synonyms
                .extend(names.iter().filter_map(|n| l.name(n, "a synonym"))),
            ConceptField::KindOf(keys) => out
                .kind_of
                .extend(keys.iter().filter_map(|k| l.key(k, SubjectRef::Model))),
            ConceptField::PartOf(keys) => out
                .part_of
                .extend(keys.iter().filter_map(|k| l.key(k, SubjectRef::Model))),
            ConceptField::Example(t) => out.examples.push(t.clone()),
            ConceptField::Source(t) => out.sources.push(t.clone()),
        }
    }
    out
}

// ----- diagrams ---------------------------------------------------------------------------------

/// A box number on a diagram, from the statements, mapped to the box's index.
type Numbers = BTreeMap<i64, usize>;

struct Pending {
    node: NodeId,
    decl_index: usize,
    diagram: Diagram,
    numbers: Numbers,
    /// Spans of each lowered arrow statement and its parts, aligned with `diagram.arrows`.
    arrow_spans: Vec<(Span, Vec<Span>)>,
}

fn diagrams(l: &mut Lower, ast: &Ast, model: &mut Model) {
    let mut taken: BTreeSet<BoxKey> = model.meta.retired.iter().cloned().collect();
    for d in &ast.diagrams {
        for s in &d.stmts {
            if let Stmt::Box(b) = s {
                for a in b.attrs.iter().filter(|a| a.name.value == "key") {
                    if let AttrValue::Word(w) = &a.value.value
                        && let Ok(k) = w.parse()
                    {
                        taken.insert(k);
                    }
                }
            }
        }
    }
    let mut pending: Vec<Pending> = Vec::new();
    let mut seen: BTreeSet<NodeId> = BTreeSet::new();
    for (i, d) in ast.diagrams.iter().enumerate() {
        if i == limits::DIAGRAMS {
            l.err(
                E_LIMIT_01,
                SubjectRef::Model,
                d.span,
                format!("more than {} diagrams", limits::DIAGRAMS),
            );
            break;
        }
        let Some(node) = node(l, d) else { continue };
        if !seen.insert(node.clone()) {
            l.parse_err(d.node.span, format!("diagram {node} is declared twice"));
            continue;
        }
        let (diagram, numbers) = header_and_boxes(l, d, &node, &mut taken);
        pending.push(Pending {
            node,
            decl_index: i,
            diagram,
            numbers,
            arrow_spans: Vec::new(),
        });
    }
    lineage_from_top(l, &mut pending, ast);
    let keys: BTreeMap<(NodeId, i64), BoxKey> = pending
        .iter()
        .flat_map(|p| {
            p.numbers
                .iter()
                .map(move |(n, &i)| ((p.node.clone(), *n), p.diagram.boxes[i].key.clone()))
        })
        .collect();
    for p in &mut pending {
        let decl = &ast.diagrams[p.decl_index];
        statements(l, decl, p, model, &keys);
    }
    for p in pending {
        l.spans.insert(
            SubjectRef::Diagram(p.node.clone()),
            ast.diagrams[p.decl_index].span,
        );
        for (b, stmt) in p
            .diagram
            .boxes
            .iter()
            .zip(box_stmts(&ast.diagrams[p.decl_index]))
        {
            l.spans
                .entry(SubjectRef::Box(b.key.clone()))
                .or_insert(stmt.span);
        }
        for (ids, (span, parts)) in arrow_ids(&p.node, &p.diagram)
            .into_iter()
            .zip(&p.arrow_spans)
        {
            l.spans.entry(SubjectRef::Edge(ids.trunk)).or_insert(*span);
            for (id, s) in ids.parts.into_iter().zip(parts) {
                l.spans.entry(SubjectRef::Edge(id)).or_insert(*s);
            }
        }
        model.diagrams.insert(p.node, Arc::new(p.diagram));
    }
}

fn box_stmts(d: &DiagramDecl) -> impl Iterator<Item = &BoxStmt> {
    d.stmts.iter().filter_map(|s| match s {
        Stmt::Box(b) => Some(b),
        _ => None,
    })
}

/// The node number, normalized; `E-NODE-01` for a bad box digit (reported on the diagram that
/// would hold that box), `E-LIMIT-01` past 16 levels below A0.
fn node(l: &mut Lower, d: &DiagramDecl) -> Option<NodeId> {
    match d.node.value.parse::<NodeId>() {
        Ok(n) if n.boxes().len() > limits::DEPTH_BELOW_A0 => {
            let msg = format!(
                "{n} is more than {} levels below A0",
                limits::DEPTH_BELOW_A0
            );
            l.err(E_LIMIT_01, SubjectRef::Model, d.node.span, msg);
            None
        }
        Ok(n) => Some(n),
        Err(RefError::BoxDigit { at }) => {
            let prefix = d
                .node
                .value
                .get(..at)
                .and_then(|p| p.trim_end_matches('.').parse().ok());
            let msg = format!(
                "`{}` has an invalid box digit: box numbers are 1-7",
                d.node.value
            );
            let subject = SubjectRef::Diagram(prefix.unwrap_or_else(NodeId::a0));
            l.err(E_NODE_01, subject, d.node.span, msg);
            None
        }
        Err(e) => {
            l.parse_err(
                d.node.span,
                format!("`{}` is not a node number: {e}", d.node.value),
            );
            None
        }
    }
}

/// The number the lineage box shows on A-n (metamodel §3.2): 0 on A-1, −(n−1) above.
fn lineage_number(node: &NodeId) -> Option<i64> {
    match node {
        NodeId::Context { level } if *level >= 1 => Some(1 - i64::from(*level)),
        _ => None,
    }
}

fn header_and_boxes(
    l: &mut Lower,
    d: &DiagramDecl,
    node: &NodeId,
    taken: &mut BTreeSet<BoxKey>,
) -> (Diagram, Numbers) {
    let subject = SubjectRef::Diagram(node.clone());
    let mut diagram = Diagram {
        title: d.title.as_ref().and_then(|t| l.name(t, "a diagram title")),
        ..Diagram::default()
    };
    for a in &d.attrs {
        match (a.name.value.as_str(), &a.value.value) {
            ("status", AttrValue::Word(w)) => match w.as_str() {
                "working" => diagram.status = Status::Working,
                "draft" => diagram.status = Status::Draft,
                "recommended" => diagram.status = Status::Recommended,
                "publication" => diagram.status = Status::Publication,
                _ => l.parse_err(
                    a.value.span,
                    format!("`status` is working, draft, recommended or publication, not `{w}`"),
                ),
            },
            ("c", AttrValue::Text(t)) => match t.parse::<CNumber>() {
                Ok(c) => diagram.cnumber = Some(c),
                Err(e) => l.parse_err(a.value.span, e.to_string()),
            },
            ("author", AttrValue::Text(t)) => diagram.author = Some(t.clone()),
            ("date", AttrValue::Text(t)) => diagram.date = Some(t.clone()),
            ("status" | "c" | "author" | "date", _) => {
                l.parse_err(a.value.span, format!("bad value for `{}`", a.name.value));
            }
            _ => l.unknown_attr(a, "a diagram"),
        }
    }
    let kind = DiagramKind::of(node);
    let lineage = lineage_number(node);
    let mut numbers = Numbers::new();
    for b in box_stmts(d) {
        let index = diagram.boxes.len();
        let number = b.number.value;
        let expected = match kind {
            DiagramKind::Context => 0,
            _ if Some(number) == lineage => number,
            _ => i64::try_from(index).map_or(i64::MAX, |i| i + 1),
        };
        if number != expected || numbers.contains_key(&number) {
            let msg = match kind {
                DiagramKind::Context => format!("A-0 has only box 0, not box {number}"),
                _ => format!("box {number} is box {expected} in statement order (metamodel §16.3)"),
            };
            l.err(E_NODE_02, subject.clone(), b.number.span, msg);
            continue;
        }
        if Some(number) == lineage {
            diagram.lineage = u8::try_from(index).ok();
        }
        let is_a1_lineage = *node == NodeId::context(1) && number == 0;
        let Some(fb) = function_box(l, b, &diagram, taken, is_a1_lineage) else {
            continue;
        };
        numbers.insert(number, index);
        diagram.boxes.push(fb);
    }
    (diagram, numbers)
}

fn function_box(
    l: &mut Lower,
    b: &BoxStmt,
    _d: &Diagram,
    taken: &mut BTreeSet<BoxKey>,
    a1_lineage: bool,
) -> Option<FunctionBox> {
    let mut key = None;
    let mut rest = Vec::new();
    for a in &b.attrs {
        match (a.name.value.as_str(), &a.value.value) {
            // A malformed key is reported (E-ID-05); the box keeps a derived key so arrows that
            // name it do not cascade into more errors.
            ("key", AttrValue::Word(w)) => {
                key = l.key::<BoxKey>(
                    &Spanned {
                        value: w.clone(),
                        span: a.value.span,
                    },
                    SubjectRef::Model,
                );
            }
            _ => rest.push(a.clone()),
        }
    }
    let provenance = l.provenance(&rest, "a box");
    // The A-1 lineage box may omit its name and key; they come from the A-0 top box.
    let placeholder = || NonEmptyString::new("lineage").ok();
    let name = match &b.name {
        Some(n) => l.name(n, "a box name")?,
        None if a1_lineage => placeholder()?,
        None => {
            l.parse_err(b.span, format!("box {} needs a name", b.number.value));
            return None;
        }
    };
    let key = match key {
        Some(k) => k,
        None if a1_lineage && b.name.is_none() => BoxKey::from_slug("lineage"),
        None => {
            let k = SlugMinter.box_key(name.as_str(), &|k| taken.contains(k));
            taken.insert(k.clone());
            k
        }
    };
    let mut fb = FunctionBox::new(key, name);
    fb.provenance = provenance;
    if let Some((text, attrs)) = &b.description {
        fb.description = l.description(text, attrs, "a description");
    }
    Some(fb)
}

/// §16.3: `box 0` on A-1 is the top box; its omitted name and key come from A-0, and any it
/// states must match.
fn lineage_from_top(l: &mut Lower, pending: &mut [Pending], ast: &Ast) {
    let top = pending
        .iter()
        .find(|p| p.node == NodeId::context(0))
        .and_then(|p| p.diagram.boxes.first().cloned());
    let Some(a1) = pending.iter_mut().find(|p| p.node == NodeId::context(1)) else {
        return;
    };
    let Some(i) = a1.diagram.lineage.map(usize::from) else {
        return;
    };
    let stmt = box_stmts(&ast.diagrams[a1.decl_index]).nth(i);
    let Some(top) = top else {
        if let Some(s) = stmt.filter(|s| s.name.is_none()) {
            l.parse_err(
                s.span,
                "the A-1 lineage box takes its name from A-0, which is missing",
            );
        }
        return;
    };
    let b = &mut a1.diagram.boxes[i];
    let stated_name = stmt.and_then(|s| s.name.as_ref());
    let stated_key = stmt.is_some_and(|s| s.attrs.iter().any(|a| a.name.value == "key"));
    if stated_name.is_some_and(|n| n.value.trim() != top.name.as_str())
        || (stated_key && b.key != top.key)
    {
        let span = stmt.map_or_else(Span::default, |s| s.span);
        l.parse_err(
            span,
            format!(
                "the A-1 lineage box is the top box `{}`; its name and key must match A-0",
                top.name
            ),
        );
    }
    if stated_name.is_none() {
        b.name = top.name.clone();
    }
    if !stated_key {
        b.key = top.key.clone();
    }
}

fn statements(
    l: &mut Lower,
    d: &DiagramDecl,
    p: &mut Pending,
    model: &Model,
    keys: &BTreeMap<(NodeId, i64), BoxKey>,
) {
    let mut ends: Vec<RankEnd> = Vec::new();
    let mut call_spans: BTreeSet<usize> = BTreeSet::new();
    for s in &d.stmts {
        match s {
            Stmt::Description(text, attrs) => {
                p.diagram.description = l.description(text, attrs, "a description");
            }
            Stmt::Box(_) | Stmt::Layout(_) => {}
            Stmt::Arrow(a) => {
                if p.diagram.arrows.len() == limits::ARROWS_PER_DIAGRAM {
                    let msg = format!(
                        "more than {} arrows on {}",
                        limits::ARROWS_PER_DIAGRAM,
                        p.node
                    );
                    l.err(E_LIMIT_01, SubjectRef::Model, a.span, msg);
                    continue;
                }
                if let Some((arrow, spans)) = arrow(l, a, p, model, &mut ends) {
                    p.diagram.arrows.push(arrow);
                    p.arrow_spans.push(spans);
                }
            }
            Stmt::Call(c) => call(l, c, p, keys, &mut call_spans),
            Stmt::Note(n) => note(l, n, p),
        }
    }
    assign_ranks(&mut p.diagram, &ends);
}

/// A box end whose rank may be omitted: (arrow, part or trunk, source end?, key, side, rank).
struct RankEnd {
    arrow: usize,
    part: Option<usize>,
    src: bool,
    key: BoxKey,
    side: Side,
    rank: Option<u8>,
}

fn endpoint(p: &Pending, e: &End) -> Result<Endpoint, i64> {
    match *e {
        End::Box { number, side, rank } => match p.numbers.get(&number) {
            Some(&i) => Ok(Endpoint::Box {
                key: p.diagram.boxes[i].key.clone(),
                side,
                rank: rank.unwrap_or(0),
            }),
            None => Err(number),
        },
        End::Boundary { side, n, border } => Ok(Endpoint::Boundary(BoundaryPort {
            side,
            code: if border {
                PortCode::Border(n)
            } else {
                PortCode::Icom(n)
            },
        })),
    }
}

fn written(e: &End) -> String {
    match *e {
        End::Box { number, side, rank } => format!(
            "{number}{}{}",
            side.letter(),
            rank.map_or(String::new(), |r| r.to_string())
        ),
        End::Boundary {
            side,
            n,
            border: false,
        } => format!("{}{n}", side.letter()),
        End::Boundary {
            side,
            n,
            border: true,
        } => format!("[{}{n}]", side.letter()),
    }
}

fn arrow(
    l: &mut Lower,
    a: &ArrowStmt,
    p: &Pending,
    model: &Model,
    ends: &mut Vec<RankEnd>,
) -> Option<(Arrow, (Span, Vec<Span>))> {
    let index = p.diagram.arrows.len();
    let attrs = arrow_attrs(l, a.cref.as_ref(), &a.attrs, model);
    let mut resolve =
        |l: &mut Lower, e: &Spanned<End>, src: bool, part: Option<usize>| -> Option<Endpoint> {
            match endpoint(p, &e.value) {
                Ok(ep) => {
                    if let (Endpoint::Box { key, side, .. }, End::Box { rank, .. }) = (&ep, e.value)
                    {
                        ends.push(RankEnd {
                            arrow: index,
                            part,
                            src,
                            key: key.clone(),
                            side: *side,
                            rank,
                        });
                    }
                    Some(ep)
                }
                Err(number) => {
                    let (code, role) = if src {
                        (E_TYPE_01, "source")
                    } else {
                        (E_TYPE_02, "destination")
                    };
                    let id = ArrowId::raw(format!("{}:{}", p.node, written(&e.value)));
                    let msg = format!("{role} box {number} is not on {}", p.node);
                    l.err(code, SubjectRef::Edge(id), e.span, msg);
                    None
                }
            }
        };
    let (shape, parts) = match &a.shape {
        Shape::Plain(src, dst) => {
            let s = resolve(l, src, true, None);
            let d = resolve(l, dst, false, None);
            (ArrowShape::Plain { src: s?, dst: d? }, vec![])
        }
        Shape::Fork(src, branches) => {
            let s = resolve(l, src, true, None);
            let mut out = Vec::new();
            for (j, b) in branches.iter().enumerate() {
                let dst = resolve(l, &b.end, false, Some(j));
                out.push(dst.map(|dst| Branch {
                    attrs: arrow_attrs(l, b.cref.as_ref(), &b.attrs, model),
                    dst,
                }));
            }
            let fork: Vec<Branch> = out.into_iter().collect::<Option<_>>()?;
            (
                ArrowShape::Fork {
                    src: s?,
                    fork: NonEmpty::from_vec(fork)?,
                },
                part_spans(branches),
            )
        }
        Shape::Join(leaves, dst) => {
            let mut out = Vec::new();
            for (j, leaf) in leaves.iter().enumerate() {
                let src = resolve(l, &leaf.end, true, Some(j));
                out.push(src.map(|src| Leaf {
                    attrs: arrow_attrs(l, leaf.cref.as_ref(), &leaf.attrs, model),
                    src,
                }));
            }
            let d = resolve(l, dst, false, None);
            let join: Vec<Leaf> = out.into_iter().collect::<Option<_>>()?;
            (
                ArrowShape::Join {
                    join: NonEmpty::from_vec(join)?,
                    dst: d?,
                },
                part_spans(leaves),
            )
        }
    };
    Some((Arrow { attrs, shape }, (a.span, parts)))
}

fn part_spans(parts: &[Part]) -> Vec<Span> {
    parts.iter().map(|p| p.span).collect()
}

fn arrow_attrs(
    l: &mut Lower,
    cref: Option<&Spanned<Cref>>,
    attrs: &[Attr],
    model: &Model,
) -> ArrowAttrs {
    let mut out = ArrowAttrs {
        concept: cref.and_then(|c| concept_ref(l, c, model)),
        ..ArrowAttrs::default()
    };
    for a in attrs {
        let bad = |l: &mut Lower, allowed: &str| {
            let msg = format!("`{}` is {allowed}", a.name.value);
            l.parse_err(a.value.span, msg);
        };
        match (a.name.value.as_str(), &a.value.value) {
            ("key", AttrValue::Word(w)) => {
                out.key = l.key::<ArrowKey>(
                    &Spanned {
                        value: w.clone(),
                        span: a.value.span,
                    },
                    SubjectRef::Model,
                );
            }
            ("label", AttrValue::Text(t)) => out.label = Some(t.clone()),
            ("rel", AttrValue::Word(w)) => match w.as_str() {
                "all" => out.rel = Some(Rel::All),
                "kind_of" => out.rel = Some(Rel::KindOf),
                "part_of" => out.rel = Some(Rel::PartOf),
                _ => bad(l, "all, kind_of or part_of"),
            },
            ("tunnel", AttrValue::Word(w)) => match w.as_str() {
                "box" => out.tunnel = Some(Tunnel::Box),
                "src" => out.tunnel = Some(Tunnel::Src),
                "dst" => out.tunnel = Some(Tunnel::Dst),
                "both" => out.tunnel = Some(Tunnel::Both),
                _ => bad(l, "box, src, dst or both"),
            },
            ("kind", AttrValue::Word(w)) => match w.as_str() {
                "code" => out.kind = Some(MechKind::Code),
                "component" => out.kind = Some(MechKind::Component),
                "resource" => out.kind = Some(MechKind::Resource),
                _ => bad(l, "code, component or resource"),
            },
            ("code", AttrValue::Text(t)) => out.code = Some(t.clone()),
            ("key" | "rel" | "tunnel" | "kind", _) => bad(l, "a word"),
            ("label" | "code", _) => bad(l, "a quoted string"),
            _ => l.unknown_attr(a, "an arrow"),
        }
    }
    out
}

/// `@key` and `ABBR/@key` by key; a quoted label by concept name or synonym, else free text.
fn concept_ref(l: &mut Lower, c: &Spanned<Cref>, model: &Model) -> Option<ConceptRef> {
    match &c.value {
        Cref::Key(raw) => match raw.parse::<KeyRef<ConceptKey>>() {
            Ok(k) => Some(ConceptRef::Key(k)),
            Err(e) => {
                l.parse_err(c.span, e.to_string());
                None
            }
        },
        Cref::Text(t) => {
            let hit = model.concepts.iter().find(|(_, concept)| {
                concept.name.as_str() == t || concept.synonyms.iter().any(|s| s.as_str() == t)
            });
            Some(match hit {
                Some((key, _)) => ConceptRef::Key(KeyRef {
                    model: None,
                    key: key.clone(),
                }),
                None => ConceptRef::Text { text: t.clone() },
            })
        }
    }
}

/// §16.3: omitted ranks take the lowest free rank per box side, in declaration order, after
/// the explicit ranks are placed. A bundle's shared trunk end counts once.
fn assign_ranks(d: &mut Diagram, ends: &[RankEnd]) {
    let mut used: BTreeMap<(&BoxKey, Side), BTreeSet<u8>> = BTreeMap::new();
    for e in ends {
        if let Some(r) = e.rank {
            used.entry((&e.key, e.side)).or_default().insert(r);
        }
    }
    let mut assigned: Vec<(usize, Option<usize>, bool, u8)> = Vec::new();
    for e in ends.iter().filter(|e| e.rank.is_none()) {
        let set = used.entry((&e.key, e.side)).or_default();
        let r = (1..=u8::MAX).find(|r| !set.contains(r)).unwrap_or(u8::MAX);
        set.insert(r);
        assigned.push((e.arrow, e.part, e.src, r));
    }
    for (arrow, part, src, r) in assigned {
        let Some(a) = d.arrows.get_mut(arrow) else {
            continue;
        };
        let end = match (&mut a.shape, part, src) {
            (ArrowShape::Plain { src: e, .. } | ArrowShape::Fork { src: e, .. }, None, true)
            | (ArrowShape::Plain { dst: e, .. } | ArrowShape::Join { dst: e, .. }, None, false) => {
                e
            }
            (ArrowShape::Fork { fork, .. }, Some(j), false) => match fork.get_mut(j) {
                Some(b) => &mut b.dst,
                None => continue,
            },
            (ArrowShape::Join { join, .. }, Some(j), true) => match join.get_mut(j) {
                Some(leaf) => &mut leaf.src,
                None => continue,
            },
            _ => continue,
        };
        if let Endpoint::Box { rank, .. } = end {
            *rank = r;
        }
    }
}

fn call(
    l: &mut Lower,
    c: &CallStmt,
    p: &mut Pending,
    keys: &BTreeMap<(NodeId, i64), BoxKey>,
    seen: &mut BTreeSet<usize>,
) {
    let Some(&i) = p.numbers.get(&c.number.value) else {
        let msg = format!("call on box {}, which is not on {}", c.number.value, p.node);
        // There is no such box, so it has no key; name it by its number.
        let key = BoxKey::from_slug(&format!("box-{}", c.number.value.unsigned_abs()));
        l.err(E_CALL_01, SubjectRef::Box(key), c.number.span, msg);
        return;
    };
    if !seen.insert(i) {
        l.parse_err(
            c.span,
            format!(
                "box {} already has a call arrow (FR-CALL-01)",
                c.number.value
            ),
        );
        return;
    }
    let mut targets = Vec::new();
    for t in &c.targets {
        if t.value.contains('@') {
            match t.value.parse::<KeyRef<BoxKey>>() {
                Ok(k) => targets.push(k),
                Err(e) => l.parse_err(t.span, e.to_string()),
            }
        } else if let Some(k) = node_target(&t.value, keys) {
            targets.push(k);
        } else if t.value.contains('/') {
            l.parse_err(t.span, format!("`{}` names a box of another model by node number; write `ABBR/@key` (the workspace index resolves it from M2.4)", t.value));
        } else {
            l.parse_err(
                t.span,
                format!("`{}` does not name a box of this model", t.value),
            );
        }
    }
    // Every target failed (each is reported): there is no call to record.
    let Some(targets) = NonEmpty::from_vec(targets) else {
        return;
    };
    let mut call = CallArrow {
        targets,
        note: None,
        map: Vec::new(),
    };
    for a in &c.attrs {
        match (a.name.value.as_str(), &a.value.value) {
            ("note", AttrValue::Word(w)) => match w.parse() {
                Ok(n) => call.note = Some(n),
                Err(_) => l.parse_err(a.value.span, "`note` is a note number"),
            },
            ("map", AttrValue::Text(t)) => {
                for pair in t.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    match pair.parse::<CallPortMap>() {
                        Ok(m) => call.map.push(m),
                        Err(e) => l.parse_err(a.value.span, format!("`{pair}` in `map`: {e}")),
                    }
                }
            }
            _ => l.unknown_attr(a, "a call"),
        }
    }
    p.diagram.boxes[i].call = Some(call);
}

/// `NODE.BOXNO` in this model, such as `A5.2`, to the box's key.
fn node_target(s: &str, keys: &BTreeMap<(NodeId, i64), BoxKey>) -> Option<KeyRef<BoxKey>> {
    let (node, number) = s.rsplit_once('.')?;
    let node: NodeId = node.parse().ok()?;
    let number: i64 = number.parse().ok()?;
    keys.get(&(node, number)).map(|k| KeyRef {
        model: None,
        key: k.clone(),
    })
}

fn note(l: &mut Lower, n: &NoteStmt, p: &mut Pending) {
    let subject = SubjectRef::Diagram(p.node.clone());
    let mut on = Vec::new();
    for t in &n.targets {
        let key_of = |number: i64| {
            p.numbers
                .get(&number)
                .map(|&i| p.diagram.boxes[i].key.clone())
        };
        let target = match &t.value {
            NoteTargetAst::Box(b) => key_of(*b).map(NoteTarget::Box),
            NoteTargetAst::Call(b) => key_of(*b).map(NoteTarget::Call),
            NoteTargetAst::Key(k) => {
                if let Ok(k) = k.parse::<ArrowKey>() {
                    Some(NoteTarget::ArrowKey(k))
                } else {
                    l.err(
                        E_ID_05,
                        SubjectRef::Model,
                        t.span,
                        format!("`{k}` is not a valid arrow key"),
                    );
                    continue;
                }
            }
            NoteTargetAst::Arrow(a, b, occurrence) => match (endpoint(p, a), endpoint(p, b)) {
                (Ok(src), Ok(dst)) => Some(NoteTarget::Arrow {
                    src: with_rank(src, a),
                    dst: with_rank(dst, b),
                    occurrence: *occurrence,
                }),
                _ => None,
            },
            NoteTargetAst::Port(End::Boundary { side, n, border }) => {
                Some(NoteTarget::Port(BoundaryPort {
                    side: *side,
                    code: if *border {
                        PortCode::Border(*n)
                    } else {
                        PortCode::Icom(*n)
                    },
                }))
            }
            NoteTargetAst::Port(_) => None,
        };
        if let Some(t) = target {
            on.push(t);
        } else {
            let msg = format!(
                "note {} points at a box that is not on {}",
                n.number.value, p.node
            );
            l.err(E_NOTE_01, subject.clone(), t.span, msg);
        }
    }
    p.diagram.notes.push(ModelNote {
        number: n.number.value,
        text: n.text.value.clone(),
        on,
    });
}

/// A note's arrow target must state ranks; an omitted one is taken as 1.
fn with_rank(e: Endpoint, written: &End) -> Endpoint {
    match (e, written) {
        (Endpoint::Box { key, side, .. }, End::Box { rank, .. }) => Endpoint::Box {
            key,
            side,
            rank: rank.unwrap_or(1),
        },
        (other, _) => other,
    }
}

#[cfg(test)]
mod tests {
    use super::lineage_number;
    use crate::id::NodeId;

    #[test]
    fn lineage_numbers() {
        assert_eq!(lineage_number(&NodeId::context(1)), Some(0));
        assert_eq!(lineage_number(&NodeId::context(3)), Some(-2));
        assert_eq!(lineage_number(&NodeId::context(0)), None);
        assert_eq!(lineage_number(&NodeId::a0()), None);
    }
}
