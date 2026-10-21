//! The lossless concrete syntax tree (metamodel §14, FR-WS-06): every token of the file,
//! trivia included, sits in exactly one node, so the tree's text is the file's text.

use super::lex::Token;

/// The kinds of syntax node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// The whole file.
    File,
    /// `idef0 0.2`.
    Header,
    /// `model "…" { … }`.
    Model,
    /// One `meta_field`.
    MetaField,
    /// `concept KEY "…" { … }`.
    Concept,
    /// One `concept_field`.
    ConceptField,
    /// `diagram NODE "…" attrs { … }`.
    Diagram,
    /// `description TEXT`.
    Description,
    /// `box …`.
    Box,
    /// `arrow …`.
    Arrow,
    /// A fork branch.
    Branch,
    /// A join leaf.
    Leaf,
    /// `call …`.
    Call,
    /// `note …`.
    Note,
    /// `layout { … }`, kept verbatim (owned by the layout spec).
    Layout,
    /// Tokens skipped while recovering from a syntax error.
    Error,
}

/// A node's child: a node or a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    /// A child node.
    Node(Node),
    /// A token.
    Token(Token),
}

/// A syntax node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The kind.
    pub kind: NodeKind,
    /// Children in source order.
    pub children: Vec<Element>,
}

impl Node {
    /// Every token below this node, in source order.
    pub fn tokens(&self) -> Vec<Token> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<Token>) {
        for child in &self.children {
            match child {
                Element::Node(n) => n.collect(out),
                Element::Token(t) => out.push(*t),
            }
        }
    }

    /// The node's source text.
    pub fn text(&self, src: &str) -> String {
        self.tokens().iter().map(|t| t.text(src)).collect()
    }

    /// Direct child nodes of `kind`.
    pub fn children_of(&self, kind: NodeKind) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(move |c| match c {
            Element::Node(n) if n.kind == kind => Some(n),
            _ => None,
        })
    }
}

/// Builds a tree from a flat token sequence, rowan-style: open a node, add tokens, close it.
#[derive(Debug)]
pub(crate) struct Builder {
    stack: Vec<Node>,
}

impl Builder {
    pub(crate) fn new() -> Self {
        Self {
            stack: vec![Node {
                kind: NodeKind::File,
                children: Vec::new(),
            }],
        }
    }

    pub(crate) fn start(&mut self, kind: NodeKind) {
        self.stack.push(Node {
            kind,
            children: Vec::new(),
        });
    }

    pub(crate) fn token(&mut self, t: Token) {
        if let Some(top) = self.stack.last_mut() {
            top.children.push(Element::Token(t));
        }
    }

    pub(crate) fn finish(&mut self) {
        if self.stack.len() > 1
            && let Some(node) = self.stack.pop()
            && let Some(parent) = self.stack.last_mut()
        {
            parent.children.push(Element::Node(node));
        }
    }

    /// Close any open nodes and return the file node.
    pub(crate) fn root(mut self) -> Node {
        while self.stack.len() > 1 {
            self.finish();
        }
        self.stack.pop().unwrap_or(Node {
            kind: NodeKind::File,
            children: Vec::new(),
        })
    }
}
