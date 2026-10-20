//! The IDEF0 model (`docs/spec/metamodel.md` §2, §4–§10, §12; roadmap M1.2).
//!
//! The types hold exactly what the `.idef0` DSL can state (metamodel §16.2). Derived values
//! are computed, never stored: a diagram's kind follows from its node number
//! ([`DiagramKind::of`]), box numbers follow from box order, a box's detail reference
//! expression follows from whether its child diagram exists ([`Model::dre`]), and
//! structural arrow ids are computed from endpoints (metamodel §3.4; M1.4).
//!
//! The JSON form is serde's: struct field order, `BTreeMap` order for maps (so diagrams are
//! listed in FIPS node-index order), identifiers as their canonical strings, and default or
//! empty fields omitted. Identical models give byte-identical JSON (D6).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use core::fmt;
use core::str::FromStr;

use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::id::{ModelAbbrev, NodeId, NodeRef, PortCode, RefError, Side};
use crate::profile::Profile;
use crate::text::{
    ArrowKey, BoxKey, CNumber, ConceptKey, Description, LangTag, NonEmptyString, Provenance,
    lang_is_en,
};

/// An IDEF0 model: metadata, concepts and diagrams (metamodel §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    /// Model metadata.
    pub meta: Meta,
    /// Model-level concepts carried by arrow labels (FR-NAME-02, FR-NAME-03).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub concepts: BTreeMap<ConceptKey, Concept>,
    /// Diagrams by node number, in FIPS node-index order. `Arc` lets a command clone only the
    /// diagrams it touches.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub diagrams: BTreeMap<NodeId, Arc<Diagram>>,
}

impl Model {
    /// A model with metadata only.
    pub fn new(meta: Meta) -> Self {
        Self {
            meta,
            concepts: BTreeMap::new(),
            diagrams: BTreeMap::new(),
        }
    }

    /// The detail reference expression shown beneath the box with node number `node`
    /// (FR-DIAG-08, metamodel §4.4): its node number when its child diagram exists, otherwise
    /// `None` (a leaf). v1 defaults to the node-number form; whether a theme shows the child's
    /// C-number instead is a rendering choice, so [`Dre::CNumber`] is never returned here.
    pub fn dre(&self, node: &NodeId) -> Option<Dre> {
        self.diagrams
            .contains_key(node)
            .then(|| Dre::NodeNumber(NodeRef::new(None, node.clone(), None)))
    }
}

/// Model metadata (metamodel §2, FR-META-01..05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// The descriptive model name (FR-META-01).
    pub name: NonEmptyString,
    /// The model id (FR-ID-01).
    pub abbrev: ModelAbbrev,
    /// The model description (FR-META-03).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Purpose (FR-META-02).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub purpose: String,
    /// Viewpoint (FR-META-02).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viewpoint: String,
    /// Scope statement (FR-META-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Context statement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Author; also the source of C-number initials (D18).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Text language (FR-META-05), default `en`.
    #[serde(default, skip_serializing_if = "lang_is_en")]
    pub lang: LangTag,
    /// Conformance profile (D8, D11).
    #[serde(default, skip_serializing_if = "is_default_profile")]
    pub profile: Profile,
    /// Retired box keys, never reused (D17).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub retired: BTreeSet<BoxKey>,
}

impl Meta {
    /// Metadata with the required fields; everything else empty or default.
    pub fn new(name: NonEmptyString, abbrev: ModelAbbrev) -> Self {
        Self {
            name,
            abbrev,
            description: String::new(),
            purpose: String::new(),
            viewpoint: String::new(),
            scope: None,
            context: None,
            author: None,
            project: None,
            lang: LangTag::default(),
            profile: Profile::default(),
            retired: BTreeSet::new(),
        }
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
fn is_default_profile(p: &Profile) -> bool {
    *p == Profile::default()
}

/// A concept (FR-NAME-02): a noun phrase shared by arrow labels across the model. Its key is
/// the key of [`Model::concepts`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Concept {
    /// The concept name.
    pub name: NonEmptyString,
    /// Provenance of the name (FR-NAME-12).
    #[serde(flatten)]
    pub provenance: Provenance,
    /// Genus–differentia definition.
    #[serde(default, skip_serializing_if = "Description::is_empty")]
    pub description: Description,
    /// Alternative names, unique model-wide with all names (`L-CONCEPT-01`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub synonyms: Vec<NonEmptyString>,
    /// Concepts this one specializes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kind_of: Vec<ConceptKey>,
    /// Concepts this one is a portion of.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub part_of: Vec<ConceptKey>,
    /// Examples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
    /// Document references (`source` in the DSL).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
}

impl Concept {
    /// A concept with a name and nothing else.
    pub fn new(name: NonEmptyString) -> Self {
        Self {
            name,
            provenance: Provenance::default(),
            description: Description::default(),
            synonyms: Vec::new(),
            kind_of: Vec::new(),
            part_of: Vec::new(),
            examples: Vec::new(),
            sources: Vec::new(),
        }
    }
}

/// The kind of a diagram, which follows from its node number (FR-DIAG-01..03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagramKind {
    /// `A-0`: one box, numbered 0.
    Context,
    /// `A-n`, n ≥ 1, with a lineage box.
    HigherContext,
    /// `A0` and every lower node.
    Decomposition,
}

impl DiagramKind {
    /// The kind of the diagram with node number `node`.
    pub fn of(node: &NodeId) -> Self {
        match node {
            NodeId::Context { level: 0 } => Self::Context,
            NodeId::Context { .. } => Self::HigherContext,
            NodeId::Node(_) => Self::Decomposition,
        }
    }
}

/// Approval status (FR-DIAG-06, Annex C.4.1.3). New diagrams start `working`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// A major change; restarts approval.
    #[default]
    Working,
    /// A minor change accepted by some readers.
    Draft,
    /// Approved with its supporting text.
    Recommended,
    /// Ready to print.
    Publication,
}

impl Status {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
    fn is_working(&self) -> bool {
        *self == Self::Working
    }
}

/// A diagram (metamodel §4). Its node number is the key of [`Model::diagrams`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Diagram {
    /// The title. Required on A-0 and the highest A-n; derived from the parent box name
    /// elsewhere, where a stated title must match (FR-DIAG-05, metamodel §4.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<NonEmptyString>,
    /// Approval status.
    #[serde(default, skip_serializing_if = "Status::is_working")]
    pub status: Status,
    /// C-number, unique in the model when present (FR-DIAG-07).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cnumber: Option<CNumber>,
    /// Author working field (metamodel §4.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Date working field, as given by an injected clock; core never reads one (D6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// The text-page analogue (FR-NAME-05).
    #[serde(default, skip_serializing_if = "Description::is_empty")]
    pub description: Description,
    /// Boxes in diagonal order: on a decomposition the box at index `i` is box `i + 1`; on
    /// A-0 the one box is box 0.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boxes: Vec<FunctionBox>,
    /// On an A-n diagram, the index in `boxes` of the lineage box (metamodel §3.2): on A-1 the
    /// top box, shown as `0`; on A-n (n ≥ 2) the box detailed by A-(n−1), shown as `−(n−1)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<u8>,
    /// Arrows in declaration order, which fixes structural-id occurrence indexes (§3.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrows: Vec<Arrow>,
    /// Model notes (FR-NOTE-01).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<ModelNote>,
}

/// A detail reference expression (metamodel §4.4). v1 uses the first two forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dre {
    /// The child diagram's node number (the default).
    NodeNumber(NodeRef),
    /// The child diagram's C-number.
    CNumber(CNumber),
}

/// A function box (metamodel §5).
///
/// A box cannot exist without a name, because the name is a [`NonEmptyString`] (SR-37):
///
/// ```
/// use idef0_core::model::FunctionBox;
/// use idef0_core::text::NonEmptyString;
///
/// let key = "validate-order".parse().unwrap();
/// let name = NonEmptyString::new("Validate Order").unwrap();
/// let b = FunctionBox::new(key, name);
/// assert_eq!(b.name.as_str(), "Validate Order");
/// ```
///
/// A plain string, empty or not, is not a name:
///
/// ```compile_fail,E0308
/// use idef0_core::model::FunctionBox;
///
/// let key = "validate-order".parse().unwrap();
/// let b = FunctionBox::new(key, "");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionBox {
    /// The stable key (FR-ID-03).
    pub key: BoxKey,
    /// The name, an active verb phrase (FR-BOX-01, FR-BOX-02).
    pub name: NonEmptyString,
    /// Provenance of the name (FR-NAME-12).
    #[serde(flatten)]
    pub provenance: Provenance,
    /// The glossary-style definition (FR-NAME-01).
    #[serde(default, skip_serializing_if = "Description::is_empty")]
    pub description: Description,
    /// The box's call arrow, if it is a caller (FR-CALL-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<CallArrow>,
}

impl FunctionBox {
    /// A box with a key and a name, undescribed and not a caller.
    pub fn new(key: BoxKey, name: NonEmptyString) -> Self {
        Self {
            key,
            name,
            provenance: Provenance::default(),
            description: Description::default(),
            call: None,
        }
    }
}

/// An arrow statement (metamodel §6, §8, §16.2): a plain arrow, a fork or a join.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrow {
    /// Attributes of the arrow, or of the trunk for a fork or join.
    #[serde(flatten)]
    pub attrs: ArrowAttrs,
    /// The endpoints, and the branches or leaves of a bundle.
    #[serde(flatten)]
    pub shape: ArrowShape,
}

/// The shape of an arrow statement. One level per statement in v1 (FR-BUN-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArrowShape {
    /// `src -> dst`.
    Plain {
        /// Source endpoint.
        src: Endpoint,
        /// Destination endpoint.
        dst: Endpoint,
    },
    /// `src -> fork { branch+ }` (FR-BUN-01).
    Fork {
        /// The trunk's source.
        src: Endpoint,
        /// The branches, in declaration order.
        fork: NonEmpty<Branch>,
    },
    /// `join { leaf+ } -> dst` (FR-BUN-01).
    Join {
        /// The leaves, in declaration order.
        join: NonEmpty<Leaf>,
        /// The trunk's destination.
        dst: Endpoint,
    },
}

/// A fork branch: `cref? -> dst attrs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// Branch attributes; an unlabeled branch inherits the trunk concept (FR-ARROW-07).
    #[serde(flatten)]
    pub attrs: ArrowAttrs,
    /// Destination endpoint.
    pub dst: Endpoint,
}

/// A join leaf: `cref? : src attrs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaf {
    /// Leaf attributes; an unlabeled leaf inherits the trunk concept (FR-ARROW-07).
    #[serde(flatten)]
    pub attrs: ArrowAttrs,
    /// Source endpoint.
    pub src: Endpoint,
}

/// The attributes an arrow, branch or leaf may carry (metamodel §16.2 `arrow_attr`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ArrowAttrs {
    /// The concept the label refers to (FR-NAME-03). `None` on an unlabeled arrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concept: Option<ConceptRef>,
    /// Display alias: the concept's name or a synonym (`label=`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Explicit arrow key, preserved verbatim (metamodel §3.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<ArrowKey>,
    /// Branch relation (FR-BUN-03); absent means `all` on an unlabeled branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<Rel>,
    /// Box-end tunnel (FR-TUN-01). Border tunnels are uncoded endpoints instead (FR-TUN-02).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tunnel: Option<Tunnel>,
    /// Mechanism binding kind (metamodel §6.2) **[Extension]**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MechKind>,
    /// Bound code, `path::qualified::symbol` (metamodel §6.2) **[Extension]**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// A label's concept reference (`cref`, metamodel §16.2): by key, possibly in another model
/// (FR-XM-01), or by text that resolves by concept name or synonym.
///
/// In JSON a key reference is the string `"@key"` or `"ABBR/@key"`; a text reference is
/// `{"text": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConceptRef {
    /// `@key` or `ABBR/@key`.
    Key(KeyRef<ConceptKey>),
    /// A quoted label; under `fips183` an unresolved one is free text (metamodel §6.3).
    Text {
        /// The label text.
        text: String,
    },
}

/// A reference by key, optionally into another model: `@key` or `ABBR/@key`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyRef<K> {
    /// The other model, if not this one.
    pub model: Option<ModelAbbrev>,
    /// The key.
    pub key: K,
}

impl<K: fmt::Display> fmt::Display for KeyRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(m) = &self.model {
            write!(f, "{m}/")?;
        }
        write!(f, "@{}", self.key)
    }
}

/// A key reference was malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum KeyRefError {
    /// No `@` before the key.
    #[error("expected `@key` or `ABBR/@key`, found {0:?}")]
    Syntax(String),
    /// The model prefix is invalid.
    #[error(transparent)]
    Model(#[from] RefError),
    /// The key is invalid.
    #[error(transparent)]
    Key(#[from] crate::text::KeyError),
}

impl<K: FromStr<Err = crate::text::KeyError>> FromStr for KeyRef<K> {
    type Err = KeyRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (model, rest) = match s.split_once('/') {
            Some((m, rest)) => (Some(m.parse()?), rest),
            None => (None, s),
        };
        let key = rest
            .strip_prefix('@')
            .ok_or_else(|| KeyRefError::Syntax(s.to_owned()))?
            .parse()?;
        Ok(Self { model, key })
    }
}

impl<K: fmt::Display> Serialize for KeyRef<K> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de, K: FromStr<Err = crate::text::KeyError>> Deserialize<'de> for KeyRef<K> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// An arrow end (metamodel §6.3, §16.2 `src`/`dst`).
///
/// JSON: a box end is `{"box": "<key>", "side": "O", "rank": 1}`; a boundary end is
/// `{"side": "I", "icom": 1}`, or `{"side": "I", "border": 1}` for an uncoded border-tunnel
/// port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Endpoint {
    /// A box side at a 1-based port rank (FR-ARROW-04). The box is named by key, so ends
    /// survive renumbering.
    Box {
        /// The box.
        #[serde(rename = "box")]
        key: BoxKey,
        /// The side.
        side: Side,
        /// The port rank on that side.
        rank: u8,
    },
    /// A diagram boundary port.
    Boundary(BoundaryPort),
}

/// A boundary port of a diagram: coded (`I1`) or uncoded (`[I1]`, FR-TUN-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryPort {
    /// The side.
    pub side: Side,
    /// The ICOM code or border ordinal.
    #[serde(flatten)]
    pub code: PortCode,
}

/// A fork-branch or join-leaf relation (FR-BUN-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rel {
    /// The same thing.
    All,
    /// A specialization.
    KindOf,
    /// A portion.
    PartOf,
}

/// A box-end tunnel (FR-TUN-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tunnel {
    /// At the one box end.
    Box,
    /// At the source end.
    Src,
    /// At the destination end.
    Dst,
    /// At both ends.
    Both,
}

/// A mechanism binding kind (metamodel §6.2) **[Extension]**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechKind {
    /// Implemented by code (`code=`).
    Code,
    /// A component.
    Component,
    /// A resource.
    Resource,
}

/// A call arrow (metamodel §7): the caller box is detailed by another box, named by key
/// (FR-CALL-12, D14). The FIPS display label is computed from the target's node number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallArrow {
    /// Called boxes; more than one only with a model note (FR-CALL-05).
    pub targets: NonEmpty<KeyRef<BoxKey>>,
    /// The model note that states selection conditions or the interface correspondence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<u32>,
    /// Caller-to-callee port correspondences (`map=`) **[Extension]**.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub map: Vec<CallPortMap>,
}

/// An ICOM code: a side and a code number, written `I1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IcomCode {
    /// The side.
    pub side: Side,
    /// The code number, 1 or more.
    pub code: u8,
}

impl fmt::Display for IcomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.side.letter(), self.code)
    }
}

impl FromStr for IcomCode {
    type Err = RefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let side = s
            .bytes()
            .next()
            .and_then(Side::from_letter)
            .ok_or(RefError::Expected {
                at: 0,
                expected: "`I`, `C`, `O` or `M`",
            })?;
        let digits = &s[1..];
        let from_one = RefError::Expected {
            at: 1,
            expected: "a number from 1",
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(from_one);
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(RefError::LeadingZero { at: 1 });
        }
        match digits.parse::<u8>() {
            Ok(0) => Err(from_one),
            Ok(code) => Ok(Self { side, code }),
            Err(_) => Err(RefError::Overflow { at: 1 }),
        }
    }
}

serde_via_str!(IcomCode);

/// One `map=` pair, `caller=callee`, such as `I1=I2` (metamodel §16.3). Mechanisms are not
/// mapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallPortMap {
    /// The caller's code.
    pub caller: IcomCode,
    /// The callee's code.
    pub callee: IcomCode,
}

impl fmt::Display for CallPortMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.caller, self.callee)
    }
}

impl FromStr for CallPortMap {
    type Err = RefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (from, to) = s.split_once('=').ok_or(RefError::Expected {
            at: s.len(),
            expected: "`=`",
        })?;
        Ok(Self {
            caller: from.parse()?,
            callee: to.parse().map_err(|e| shift_up(e, from.len() + 1))?,
        })
    }
}

fn shift_up(e: RefError, by: usize) -> RefError {
    match e {
        RefError::Expected { at, expected } => RefError::Expected {
            at: at + by,
            expected,
        },
        RefError::LeadingZero { at } => RefError::LeadingZero { at: at + by },
        RefError::Overflow { at } => RefError::Overflow { at: at + by },
        RefError::Trailing { at } => RefError::Trailing { at: at + by },
        other => other,
    }
}

serde_via_str!(CallPortMap);

/// A model note (FR-NOTE-01..04): numbered from 1 per diagram, attached to points on the
/// same diagram only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelNote {
    /// The note number.
    pub number: u32,
    /// The note text.
    pub text: String,
    /// What the note's squiggles point at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on: Vec<NoteTarget>,
}

/// A model-note attachment point (metamodel §16.2 `note_target`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteTarget {
    /// A box.
    Box(BoxKey),
    /// A box's call arrow.
    Call(BoxKey),
    /// An arrow by its endpoints, and its occurrence index among arrows with the same
    /// endpoints (`#n`, metamodel §3.4).
    Arrow {
        /// Source endpoint.
        src: Endpoint,
        /// Destination endpoint.
        dst: Endpoint,
        /// 0-based occurrence index.
        #[serde(default, skip_serializing_if = "is_zero")]
        occurrence: u32,
    },
    /// An arrow by its explicit key.
    ArrowKey(ArrowKey),
    /// A boundary port.
    Port(BoundaryPort),
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(s: &str) -> NodeId {
        s.parse().unwrap()
    }

    fn name(s: &str) -> NonEmptyString {
        NonEmptyString::new(s).unwrap()
    }

    // FR-DIAG-01..03: the kind follows from the node number.
    #[test]
    fn diagram_kind() {
        assert_eq!(DiagramKind::of(&node("A-0")), DiagramKind::Context);
        assert_eq!(DiagramKind::of(&node("A-1")), DiagramKind::HigherContext);
        assert_eq!(DiagramKind::of(&node("A-3")), DiagramKind::HigherContext);
        assert_eq!(DiagramKind::of(&node("A0")), DiagramKind::Decomposition);
        assert_eq!(DiagramKind::of(&node("A213")), DiagramKind::Decomposition);
    }

    // FR-DIAG-08, metamodel §4.4: a detailed box shows its node number; a leaf shows none.
    #[test]
    fn dre_follows_the_child_diagram() {
        let mut m = Model::new(Meta::new(name("Orders"), "ORD".parse().unwrap()));
        m.diagrams.insert(node("A0"), Arc::new(Diagram::default()));
        m.diagrams.insert(node("A2"), Arc::new(Diagram::default()));
        assert_eq!(
            m.dre(&node("A2")),
            Some(Dre::NodeNumber(NodeRef::new(None, node("A2"), None)))
        );
        assert_eq!(m.dre(&node("A1")), None);
    }

    // FR-XM-01, FR-CALL-12: key references are `@key` or `ABBR/@key`.
    #[test]
    fn key_refs() {
        let local: KeyRef<BoxKey> = "@verify-token".parse().unwrap();
        assert_eq!(
            (local.model.as_ref(), local.key.as_str()),
            (None, "verify-token")
        );
        let remote: KeyRef<BoxKey> = "SEC/@verify-token".parse().unwrap();
        assert_eq!(remote.to_string(), "SEC/@verify-token");
        for bad in [
            "verify-token",
            "SEC/verify-token",
            "sec/@x",
            "@Bad",
            "@",
            "SEC/@",
        ] {
            assert!(bad.parse::<KeyRef<BoxKey>>().is_err(), "{bad:?}");
        }
    }

    // metamodel §16.3: `map="I1=I2,C1=C1"` pairs caller=callee codes.
    #[test]
    fn call_port_maps() {
        let m: CallPortMap = "I1=I2".parse().unwrap();
        assert_eq!(
            m.caller,
            IcomCode {
                side: Side::I,
                code: 1
            }
        );
        assert_eq!(
            m.callee,
            IcomCode {
                side: Side::I,
                code: 2
            }
        );
        assert_eq!(
            "C12=O3".parse::<CallPortMap>().unwrap().to_string(),
            "C12=O3"
        );
        assert_eq!(
            "I1".parse::<CallPortMap>(),
            Err(RefError::Expected {
                at: 2,
                expected: "`=`"
            })
        );
        assert_eq!(
            "I1=X1".parse::<CallPortMap>(),
            Err(RefError::Expected {
                at: 3,
                expected: "`I`, `C`, `O` or `M`"
            })
        );
        assert_eq!(
            "I1=C01".parse::<CallPortMap>(),
            Err(RefError::LeadingZero { at: 4 })
        );
        for bad in ["I0=I1", "I1=I256", "[I1]=I1", "I1x=I1", "=I1", "I1="] {
            assert!(bad.parse::<CallPortMap>().is_err(), "{bad:?}");
        }
    }

    // The three arrow shapes deserialize to the shape they were written as.
    #[test]
    fn arrow_shapes() {
        let plain = r#"{"concept":"@order","src":{"side":"I","icom":1},"dst":{"box":"check","side":"I","rank":1}}"#;
        let a: Arrow = serde_json::from_str(plain).unwrap();
        assert!(matches!(a.shape, ArrowShape::Plain { .. }));
        assert_eq!(serde_json::to_string(&a).unwrap(), plain);

        let fork = r#"{"src":{"box":"check","side":"O","rank":1},"fork":[{"dst":{"box":"ship","side":"I","rank":1}},{"concept":"@refund","rel":"part_of","dst":{"side":"O","border":1}}]}"#;
        let f: Arrow = serde_json::from_str(fork).unwrap();
        assert!(matches!(f.shape, ArrowShape::Fork { .. }));
        assert_eq!(serde_json::to_string(&f).unwrap(), fork);

        let join =
            r#"{"join":[{"src":{"box":"a","side":"O","rank":1}}],"dst":{"side":"O","icom":2}}"#;
        let j: Arrow = serde_json::from_str(join).unwrap();
        assert!(matches!(j.shape, ArrowShape::Join { .. }));
        assert_eq!(serde_json::to_string(&j).unwrap(), join);

        // A bundle needs at least one branch or leaf (`branch+`, `leaf+`).
        let empty_fork = r#"{"src":{"side":"I","icom":1},"fork":[]}"#;
        assert!(serde_json::from_str::<Arrow>(empty_fork).is_err());
        assert!(serde_json::from_str::<Arrow>(r#"{"src":{"side":"I","icom":1}}"#).is_err());
    }

    // FR-CALL-05: a call has at least one target.
    #[test]
    fn call_needs_a_target() {
        let ok = r#"{"targets":["@verify-token","SEC/@verify-token"],"note":1,"map":["I1=I2"]}"#;
        let c: CallArrow = serde_json::from_str(ok).unwrap();
        assert_eq!(c.targets.len(), 2);
        assert_eq!(serde_json::to_string(&c).unwrap(), ok);
        assert!(serde_json::from_str::<CallArrow>(r#"{"targets":[]}"#).is_err());
    }

    // metamodel §3.4: `#n` picks the n-th arrow with the same endpoints; 0 is omitted.
    #[test]
    fn note_arrow_occurrence() {
        let json = r#"{"arrow":{"src":{"side":"I","icom":1},"dst":{"box":"a","side":"I","rank":1},"occurrence":2}}"#;
        let t: NoteTarget = serde_json::from_str(json).unwrap();
        assert!(matches!(t, NoteTarget::Arrow { occurrence: 2, .. }));
        assert_eq!(serde_json::to_string(&t).unwrap(), json);
        let first =
            r#"{"arrow":{"src":{"side":"I","icom":1},"dst":{"box":"a","side":"I","rank":1}}}"#;
        let t: NoteTarget = serde_json::from_str(first).unwrap();
        assert!(matches!(t, NoteTarget::Arrow { occurrence: 0, .. }));
        assert_eq!(serde_json::to_string(&t).unwrap(), first);
    }

    // SR-37: a box without a name, or with an empty one, does not deserialize.
    #[test]
    fn box_needs_a_name() {
        assert!(serde_json::from_str::<FunctionBox>(r#"{"key":"a"}"#).is_err());
        let err = serde_json::from_str::<FunctionBox>(r#"{"key":"a","name":"  "}"#).unwrap_err();
        assert!(
            err.to_string().contains("a name must not be empty"),
            "{err}"
        );
        let b: FunctionBox =
            serde_json::from_str(r#"{"key":"a","name":"Plan","origin":"llm"}"#).unwrap();
        assert_eq!(b.provenance.origin, crate::text::Origin::Llm);
    }
}
