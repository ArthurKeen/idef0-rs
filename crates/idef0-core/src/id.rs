//! Node numbers, diagram references and port references (`docs/spec/metamodel.md` §3.2,
//! §3.5, §3.6; FR-ID-01, FR-ID-07, FR-ID-08).
//!
//! Parsing accepts the canonical forms and the FIPS and dotted input forms, and normalizes
//! them: `A21.3` → `A213`, `A0.3` → `A3`, `A21I1` → `A21/I1`. [`Display`] always writes the
//! canonical form; [`NodeId::dotted`] and [`PortRef::fips`] write the alternative styles.
//! Parsing never depends on a profile: box digits are 1–7 everywhere, and the profile's
//! lower limit is a separate check ([`NodeId::fits_profile`]).

use core::fmt;
use core::str::FromStr;

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::profile::Profile;

/// The highest box digit any profile accepts in a node number (metamodel §3.2).
const MAX_BOX_DIGIT: u8 = 7;

/// Why a reference failed to parse. Byte offsets are into the input string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RefError {
    /// The input was empty.
    #[error("empty reference")]
    Empty,
    /// Something else was found where the grammar requires `expected`.
    #[error("expected {expected} at byte {at}")]
    Expected {
        /// Byte offset of the problem.
        at: usize,
        /// What the grammar requires there.
        expected: &'static str,
    },
    /// A box number in a node number is not a single digit 1–7 (`E-NODE-01`).
    #[error("invalid box number at byte {at}: box numbers are single digits 1-7 (E-NODE-01)")]
    BoxDigit {
        /// Byte offset of the offending digit.
        at: usize,
    },
    /// A multi-digit number starts with `0`.
    #[error("number at byte {at} has a leading zero")]
    LeadingZero {
        /// Byte offset of the number.
        at: usize,
    },
    /// A number does not fit its field.
    #[error("number at byte {at} is out of range")]
    Overflow {
        /// Byte offset of the number.
        at: usize,
    },
    /// The model prefix is not `[A-Z][A-Z0-9]{0,7}` (FR-ID-01).
    #[error(
        "invalid model abbreviation {0:?}: expected an uppercase letter and up to 7 more uppercase letters or digits (FR-ID-01)"
    )]
    Abbrev(String),
    /// `A-n.k` names an environment box, which has no node number (metamodel §3.2).
    #[error("`A-n.k` at byte {at} names an environment box, which has no node number")]
    EnvironmentBox {
        /// Byte offset of the `.`.
        at: usize,
    },
    /// `@key` references resolve through the model's box keys, not node numbers.
    #[error("`@key` reference at byte {at} needs the model's box keys to resolve")]
    KeyRef {
        /// Byte offset of the `@`.
        at: usize,
    },
    /// Input continues after a complete reference.
    #[error("unexpected input at byte {at}")]
    Trailing {
        /// Byte offset of the first unexpected byte.
        at: usize,
    },
}

/// A model abbreviation, the model id (FR-ID-01): `[A-Z][A-Z0-9]{0,7}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelAbbrev(CompactString);

impl ModelAbbrev {
    /// The abbreviation as written, for example `"MFG"`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ModelAbbrev {
    type Err = RefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let b = s.as_bytes();
        let valid = (1..=8).contains(&b.len())
            && b[0].is_ascii_uppercase()
            && b[1..]
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if valid {
            Ok(Self(CompactString::new(s)))
        } else {
            Err(RefError::Abbrev(s.to_owned()))
        }
    }
}

impl fmt::Display for ModelAbbrev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A node number (metamodel §3.2): a context diagram `A-n`, or a box path from `A0`.
///
/// The derived order lists context diagrams first, then nodes in FIPS node-index order:
/// `A0`, `A1`, `A11`, `A12`, `A2`, …
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeId {
    /// Context diagram `A-<level>`: `A-0` is the context diagram, `A-1`, `A-2`, … are higher
    /// context diagrams (§3.4.2–3.4.3).
    Context {
        /// The `n` of `A-n`.
        level: u8,
    },
    /// Box path below `A0`: `[]` is `A0` (the top box), `[2, 1]` is `A21`. Each digit is 1–7.
    Node(SmallVec<[u8; 8]>),
}

impl NodeId {
    /// `A0`, the top box and the diagram that details it.
    pub fn a0() -> Self {
        Self::Node(SmallVec::new())
    }

    /// The context diagram `A-<level>`.
    pub fn context(level: u8) -> Self {
        Self::Context { level }
    }

    /// The node with this box path below `A0`, or `None` if a box number is not 1–7.
    pub fn from_boxes(boxes: &[u8]) -> Option<Self> {
        boxes
            .iter()
            .all(|&k| is_box_number(k))
            .then(|| Self::Node(SmallVec::from_slice(boxes)))
    }

    /// The box path below `A0`; empty for `A0` and for context diagrams.
    pub fn boxes(&self) -> &[u8] {
        match self {
            Self::Context { .. } => &[],
            Self::Node(path) => path,
        }
    }

    /// Box `k` on the diagram that details this node: "box 2 on diagram A25 is A252", and the
    /// children of `A0` are `A1..An` (§3.2). `None` for context diagrams or `k` outside 1–7.
    pub fn child(&self, k: u8) -> Option<Self> {
        match self {
            Self::Node(path) if is_box_number(k) => {
                let mut child = path.clone();
                child.push(k);
                Some(Self::Node(child))
            }
            _ => None,
        }
    }

    /// The node whose diagram contains this box: `A21` → `A2`, `A1` → `A0`. `None` for `A0`
    /// and context diagrams.
    pub fn parent(&self) -> Option<Self> {
        match self {
            Self::Node(path) if !path.is_empty() => {
                Some(Self::Node(SmallVec::from_slice(&path[..path.len() - 1])))
            }
            _ => None,
        }
    }

    /// Whether every box number in the path is within the profile's box limit.
    pub fn fits_profile(&self, profile: Profile) -> bool {
        self.boxes().iter().all(|&k| k <= profile.max_box_number())
    }

    /// Display in dotted style, `A21.3` for `A213` (§3.6). `A0`, single-box nodes and context
    /// diagrams have no dot.
    pub fn dotted(&self) -> Dotted<'_> {
        Dotted(self)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context { level } => write!(f, "A-{level}"),
            Self::Node(path) if path.is_empty() => f.write_str("A0"),
            Self::Node(path) => {
                f.write_str("A")?;
                path.iter().try_for_each(|k| write!(f, "{k}"))
            }
        }
    }
}

impl FromStr for NodeId {
    type Err = RefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cur = Cursor::new(s)?;
        let node = cur.node_id()?;
        cur.end()?;
        Ok(node)
    }
}

/// Dotted display of a [`NodeId`]; see [`NodeId::dotted`].
#[derive(Debug, Clone, Copy)]
pub struct Dotted<'a>(&'a NodeId);

impl fmt::Display for Dotted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            NodeId::Node(path) if path.len() >= 2 => {
                let (last, head) = path.split_last().ok_or(fmt::Error)?;
                f.write_str("A")?;
                head.iter().try_for_each(|k| write!(f, "{k}"))?;
                write!(f, ".{last}")
            }
            node => write!(f, "{node}"),
        }
    }
}

/// The page kind of a §3.3.4.3 node-reference suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SheetKind {
    /// `F`: a FOR EXPOSITION ONLY diagram (§3.3.1.5).
    Feo,
    /// `T`: a text page.
    Text,
    /// `G`: a glossary page.
    Glossary,
}

impl SheetKind {
    const fn letter(self) -> char {
        match self {
            Self::Feo => 'F',
            Self::Text => 'T',
            Self::Glossary => 'G',
        }
    }
}

/// A §3.3.4.3 suffix such as `F`, `F2` or `T1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SheetSuffix {
    kind: SheetKind,
    page: Option<u32>,
}

impl SheetSuffix {
    /// A suffix of `kind`, optionally numbered (`F` or `F2`). `None` if `page` is `Some(0)`.
    pub fn new(kind: SheetKind, page: Option<u32>) -> Option<Self> {
        (page != Some(0)).then_some(Self { kind, page })
    }

    /// The page kind.
    pub fn kind(&self) -> SheetKind {
        self.kind
    }

    /// The page number, if written.
    pub fn page(&self) -> Option<u32> {
        self.page
    }
}

impl fmt::Display for SheetSuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind.letter())?;
        self.page.map_or(Ok(()), |n| write!(f, "{n}"))
    }
}

/// A diagram reference (`DiagRef`, §3.6): optional model, node number and page suffix.
/// Examples: `A21`, `SEC/A21`, `A321F`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeRef {
    model: Option<ModelAbbrev>,
    node: NodeId,
    sheet: Option<SheetSuffix>,
}

impl NodeRef {
    /// A reference to `node`, optionally in another model and to a suffixed page.
    pub fn new(model: Option<ModelAbbrev>, node: NodeId, sheet: Option<SheetSuffix>) -> Self {
        Self { model, node, sheet }
    }

    /// The model prefix, if written.
    pub fn model(&self) -> Option<&ModelAbbrev> {
        self.model.as_ref()
    }

    /// The node number.
    pub fn node(&self) -> &NodeId {
        &self.node
    }

    /// The page suffix, if written.
    pub fn sheet(&self) -> Option<SheetSuffix> {
        self.sheet
    }
}

impl fmt::Display for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(model) = &self.model {
            write!(f, "{model}/")?;
        }
        write!(f, "{}", self.node)?;
        self.sheet.map_or(Ok(()), |s| write!(f, "{s}"))
    }
}

impl FromStr for NodeRef {
    type Err = RefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cur = Cursor::new(s)?;
        let model = cur.model_prefix()?;
        let node = cur.node_id()?;
        let sheet = cur.sheet_suffix()?;
        cur.end()?;
        Ok(Self { model, node, sheet })
    }
}

/// An ICOM side (§2.2): Input, Control, Output, Mechanism. Ordered I, C, O, M.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Input.
    I,
    /// Control.
    C,
    /// Output.
    O,
    /// Mechanism.
    M,
}

impl Side {
    /// The side's letter: `I`, `C`, `O` or `M`.
    pub const fn letter(self) -> char {
        match self {
            Self::I => 'I',
            Self::C => 'C',
            Self::O => 'O',
            Self::M => 'M',
        }
    }

    /// The side for an ASCII letter `I`, `C`, `O` or `M`.
    pub const fn from_letter(b: u8) -> Option<Self> {
        match b {
            b'I' => Some(Self::I),
            b'C' => Some(Self::C),
            b'O' => Some(Self::O),
            b'M' => Some(Self::M),
            _ => None,
        }
    }
}

/// The code of a port on one side of a box. In JSON, `{"icom": 2}` or `{"border": 1}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortCode {
    /// An ICOM code rank (`I2` is rank 2), 1 or more.
    Icom(u8),
    /// An uncoded border-tunnel port `[I1]` (§3.3.2.9): a local ordinal per diagram side,
    /// not an ICOM code (FR-TUN-03).
    Border(u8),
}

impl PortCode {
    /// The rank or ordinal, 1 or more.
    pub fn number(self) -> u8 {
        match self {
            Self::Icom(n) | Self::Border(n) => n,
        }
    }
}

/// A port reference (§3.5, FR-ID-07): one side and rank of a box, which is also the
/// same-coded boundary port of the box's child diagram. Canonical form `A21/I1`
/// **[Extension]**; the FIPS form `A21I1` is accepted on input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortRef {
    model: Option<ModelAbbrev>,
    node: NodeId,
    side: Side,
    code: PortCode,
}

impl PortRef {
    /// A port of `node`, or `None` if the rank or ordinal is 0.
    pub fn new(
        model: Option<ModelAbbrev>,
        node: NodeId,
        side: Side,
        code: PortCode,
    ) -> Option<Self> {
        (code.number() != 0).then_some(Self {
            model,
            node,
            side,
            code,
        })
    }

    /// The model prefix, if written.
    pub fn model(&self) -> Option<&ModelAbbrev> {
        self.model.as_ref()
    }

    /// The node number of the box.
    pub fn node(&self) -> &NodeId {
        &self.node
    }

    /// The ICOM side.
    pub fn side(&self) -> Side {
        self.side
    }

    /// The ICOM code or border ordinal.
    pub fn code(&self) -> PortCode {
        self.code
    }

    /// Display in FIPS style (§3.3.4.5), `A21I1`. Border ports have no ICOM code and keep
    /// the canonical `A21/[I1]`.
    pub fn fips(&self) -> Fips<'_> {
        Fips(self)
    }

    fn write(&self, f: &mut fmt::Formatter<'_>, fips: bool) -> fmt::Result {
        if let Some(model) = &self.model {
            write!(f, "{model}/")?;
        }
        let side = self.side.letter();
        match self.code {
            PortCode::Icom(n) if fips => write!(f, "{}{side}{n}", self.node),
            PortCode::Icom(n) => write!(f, "{}/{side}{n}", self.node),
            PortCode::Border(n) => write!(f, "{}/[{side}{n}]", self.node),
        }
    }
}

impl fmt::Display for PortRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f, false)
    }
}

impl FromStr for PortRef {
    type Err = RefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cur = Cursor::new(s)?;
        let model = cur.model_prefix()?;
        let node = cur.node_id()?;
        let slash = cur.eat(b'/');
        let border = cur.eat(b'[');
        if border && !slash {
            return Err(RefError::Expected {
                at: cur.pos - 1,
                expected: "`/` before `[`",
            });
        }
        let side = cur.side()?;
        let n = cur.positive_nat()?;
        if border && !cur.eat(b']') {
            return Err(cur.expected("`]`"));
        }
        cur.end()?;
        let code = if border {
            PortCode::Border(n)
        } else {
            PortCode::Icom(n)
        };
        Ok(Self {
            model,
            node,
            side,
            code,
        })
    }
}

/// FIPS-style display of a [`PortRef`]; see [`PortRef::fips`].
#[derive(Debug, Clone, Copy)]
pub struct Fips<'a>(&'a PortRef);

impl fmt::Display for Fips<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.write(f, true)
    }
}

serde_via_str!(ModelAbbrev, NodeId, NodeRef, PortRef);

const fn is_box_number(k: u8) -> bool {
    k >= 1 && k <= MAX_BOX_DIGIT
}

/// A byte cursor over an ASCII reference.
#[derive(Debug)]
struct Cursor<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Result<Self, RefError> {
        if s.is_empty() {
            return Err(RefError::Empty);
        }
        Ok(Self {
            s: s.as_bytes(),
            pos: 0,
        })
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.s.get(self.pos + offset).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        let hit = self.peek() == Some(b);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn expected(&self, expected: &'static str) -> RefError {
        RefError::Expected {
            at: self.pos,
            expected,
        }
    }

    fn end(&self) -> Result<(), RefError> {
        if self.pos == self.s.len() {
            Ok(())
        } else {
            Err(RefError::Trailing { at: self.pos })
        }
    }

    /// `Abbrev "/"`, taken only when a node number or `@key` follows the first `/`, so that
    /// `A21/I1` is node `A21`, not model `A21`.
    fn model_prefix(&mut self) -> Result<Option<ModelAbbrev>, RefError> {
        if self.peek() == Some(b'@') {
            return Err(RefError::KeyRef { at: self.pos });
        }
        let Some(slash) = self.s[self.pos..].iter().position(|&b| b == b'/') else {
            return Ok(None);
        };
        let after = self.pos + slash + 1;
        let next = self.s.get(after).copied();
        let starts_node =
            next == Some(b'A') && matches!(self.s.get(after + 1), Some(b'-' | b'0'..=b'9'));
        if !starts_node && next != Some(b'@') {
            return Ok(None);
        }
        let text = core::str::from_utf8(&self.s[self.pos..after - 1])
            .map_err(|_| self.expected("a model abbreviation"))?;
        let model = text.parse()?;
        self.pos = after;
        if next == Some(b'@') {
            return Err(RefError::KeyRef { at: self.pos });
        }
        Ok(Some(model))
    }

    /// `NodeId`, followed by any dotted `.k` box numbers (§3.6 normalization).
    fn node_id(&mut self) -> Result<NodeId, RefError> {
        if self.peek() == Some(b'@') {
            return Err(RefError::KeyRef { at: self.pos });
        }
        if !self.eat(b'A') {
            return Err(self.expected("`A`"));
        }
        if self.eat(b'-') {
            let level = self
                .nat()?
                .ok_or_else(|| self.expected("a context level"))?;
            if self.peek() == Some(b'.') {
                return Err(RefError::EnvironmentBox { at: self.pos });
            }
            return Ok(NodeId::Context { level });
        }
        let start = self.pos;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        let digits = &self.s[start..self.pos];
        let mut path: SmallVec<[u8; 8]> = SmallVec::new();
        match digits {
            [] => return Err(self.expected("`0`, box digits or `-` after `A`")),
            [b'0'] => {}
            _ => {
                for (i, &d) in digits.iter().enumerate() {
                    let k = d - b'0';
                    if !is_box_number(k) {
                        return Err(RefError::BoxDigit { at: start + i });
                    }
                    path.push(k);
                }
            }
        }
        while self.peek() == Some(b'.') {
            self.pos += 1;
            let at = self.pos;
            match (self.peek(), self.peek_at(1)) {
                (Some(d @ b'0'..=b'9'), next) if !next.is_some_and(|b| b.is_ascii_digit()) => {
                    let k = d - b'0';
                    if !is_box_number(k) {
                        return Err(RefError::BoxDigit { at });
                    }
                    path.push(k);
                    self.pos += 1;
                }
                (Some(b'0'..=b'9' | b'-'), _) => return Err(RefError::BoxDigit { at }),
                _ => return Err(self.expected("a box number after `.`")),
            }
        }
        Ok(NodeId::Node(path))
    }

    fn sheet_suffix(&mut self) -> Result<Option<SheetSuffix>, RefError> {
        let kind = match self.peek() {
            Some(b'F') => SheetKind::Feo,
            Some(b'T') => SheetKind::Text,
            Some(b'G') => SheetKind::Glossary,
            _ => return Ok(None),
        };
        self.pos += 1;
        let at = self.pos;
        let page = self.nat::<u32>()?;
        SheetSuffix::new(kind, page)
            .map(Some)
            .ok_or(RefError::Expected {
                at,
                expected: "a page number from 1",
            })
    }

    fn side(&mut self) -> Result<Side, RefError> {
        let side = self
            .peek()
            .and_then(Side::from_letter)
            .ok_or_else(|| self.expected("`I`, `C`, `O` or `M`"))?;
        self.pos += 1;
        Ok(side)
    }

    fn positive_nat(&mut self) -> Result<u8, RefError> {
        let at = self.pos;
        match self.nat::<u8>()? {
            Some(n) if n >= 1 => Ok(n),
            _ => Err(RefError::Expected {
                at,
                expected: "a number from 1",
            }),
        }
    }

    /// `NUM` without leading zeros, or `None` if no digit follows.
    fn nat<T: TryFrom<u32>>(&mut self) -> Result<Option<T>, RefError> {
        let start = self.pos;
        let mut value: u32 = 0;
        while let Some(d) = self.peek().filter(u8::is_ascii_digit) {
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u32::from(d - b'0')))
                .ok_or(RefError::Overflow { at: start })?;
            self.pos += 1;
        }
        let len = self.pos - start;
        if len == 0 {
            return Ok(None);
        }
        if len > 1 && self.s[start] == b'0' {
            return Err(RefError::LeadingZero { at: start });
        }
        T::try_from(value)
            .map(Some)
            .map_err(|_| RefError::Overflow { at: start })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(s: &str) -> NodeId {
        s.parse().unwrap()
    }

    fn port(s: &str) -> PortRef {
        s.parse().unwrap()
    }

    // metamodel §3.2: A-0 is the context diagram; its top box is A0.
    #[test]
    fn context_and_top_node() {
        assert_eq!(node("A-0"), NodeId::context(0));
        assert_eq!(node("A-12"), NodeId::context(12));
        assert_eq!(node("A0"), NodeId::a0());
        assert_eq!(node("A21"), NodeId::from_boxes(&[2, 1]).unwrap());
        for s in ["A-0", "A-1", "A0", "A21", "A7654321"] {
            assert_eq!(node(s).to_string(), s);
        }
    }

    // FR-ID-08, metamodel §3.6: dotted input is normalized.
    #[test]
    fn dotted_normalization() {
        assert_eq!(node("A21.3"), node("A213"));
        assert_eq!(node("A0.3"), node("A3"), "A0.k -> Ak");
        assert_eq!(node("A2.1.3"), node("A213"), "multi-period, left to right");
        assert_eq!(node("A0.1.2"), node("A12"));
        assert_eq!(port("A21.3C2"), port("A213/C2"));
        assert_eq!(node("A213").dotted().to_string(), "A21.3");
        assert_eq!(node("A1").dotted().to_string(), "A1");
        assert_eq!(node("A0").dotted().to_string(), "A0");
    }

    // metamodel §3.2: "box 2 on diagram A25 is A252"; the children of A0 are A1..An.
    #[test]
    fn child_and_parent() {
        assert_eq!(node("A25").child(2), Some(node("A252")));
        assert_eq!(NodeId::a0().child(3), Some(node("A3")));
        assert_eq!(NodeId::a0().child(0), None);
        assert_eq!(NodeId::a0().child(8), None);
        assert_eq!(NodeId::context(0).child(1), None);
        assert_eq!(node("A21").parent(), Some(node("A2")));
        assert_eq!(node("A1").parent(), Some(NodeId::a0()));
        assert_eq!(NodeId::a0().parent(), None);
        assert_eq!(NodeId::context(1).parent(), None);
    }

    // Roadmap M1.1 acceptance, E-NODE-01: A8 is rejected under both profiles.
    #[test]
    fn a8_rejected_under_both_profiles() {
        assert_eq!("A8".parse::<NodeId>(), Err(RefError::BoxDigit { at: 1 }));
        for profile in [Profile::Extended, Profile::Fips183] {
            assert!(8 > profile.max_box_number());
        }
        assert!(NodeId::from_boxes(&[8]).is_none());
    }

    // metamodel §1.2 and §3.2: digits are 1-7 syntactically; the fips183 limit is 6.
    #[test]
    fn profile_box_limit() {
        assert!(node("A7").fits_profile(Profile::Extended));
        assert!(!node("A7").fits_profile(Profile::Fips183));
        assert!(!node("A27").fits_profile(Profile::Fips183));
        assert!(node("A16").fits_profile(Profile::Fips183));
        assert!(NodeId::context(3).fits_profile(Profile::Fips183));
    }

    // E-NODE-01 and the NodeId grammar.
    #[test]
    fn invalid_node_numbers() {
        let cases = [
            ("", RefError::Empty),
            ("A9", RefError::BoxDigit { at: 1 }),
            ("A10", RefError::BoxDigit { at: 2 }),
            ("A01", RefError::BoxDigit { at: 1 }),
            ("A2.0", RefError::BoxDigit { at: 3 }),
            ("A2.8", RefError::BoxDigit { at: 3 }),
            ("A2.12", RefError::BoxDigit { at: 3 }),
            ("A21.-1", RefError::BoxDigit { at: 4 }),
            ("A-01", RefError::LeadingZero { at: 2 }),
            ("A-256", RefError::Overflow { at: 2 }),
            ("A-1.2", RefError::EnvironmentBox { at: 3 }),
            ("@validate-order", RefError::KeyRef { at: 0 }),
            (
                "B1",
                RefError::Expected {
                    at: 0,
                    expected: "`A`",
                },
            ),
            (
                "a1",
                RefError::Expected {
                    at: 0,
                    expected: "`A`",
                },
            ),
            ("A21 ", RefError::Trailing { at: 3 }),
            ("A21F", RefError::Trailing { at: 3 }),
        ];
        for (input, err) in cases {
            assert_eq!(input.parse::<NodeId>(), Err(err), "{input:?}");
        }
        assert!(matches!(
            "A".parse::<NodeId>(),
            Err(RefError::Expected { at: 1, .. })
        ));
        assert!(matches!(
            "A-".parse::<NodeId>(),
            Err(RefError::Expected { at: 2, .. })
        ));
        assert!(matches!(
            "A2.".parse::<NodeId>(),
            Err(RefError::Expected { at: 3, .. })
        ));
    }

    // metamodel §3.3.4.3: node reference = abbrev `/` node, optional F/T/G suffix and page.
    #[test]
    fn node_refs() {
        for s in [
            "A21",
            "SEC/A21",
            "A321F",
            "A321F2",
            "A21T",
            "MFG/A-0G3",
            "X1/A0",
        ] {
            assert_eq!(s.parse::<NodeRef>().unwrap().to_string(), s);
        }
        let r: NodeRef = "SEC/A2.1F2".parse().unwrap();
        assert_eq!(r.model().map(ModelAbbrev::as_str), Some("SEC"));
        assert_eq!(r.node(), &node("A21"));
        assert_eq!(r.sheet(), SheetSuffix::new(SheetKind::Feo, Some(2)));
        assert_eq!(r.to_string(), "SEC/A21F2");
        assert!(matches!(
            "A21F0".parse::<NodeRef>(),
            Err(RefError::Expected { at: 4, .. })
        ));
        assert_eq!(
            "A21F01".parse::<NodeRef>(),
            Err(RefError::LeadingZero { at: 4 })
        );
    }

    // FR-ID-01: the model id is [A-Z][A-Z0-9]{0,7}.
    #[test]
    fn model_abbrev() {
        for ok in ["A", "MFG", "A2", "ABCDEFGH"] {
            assert_eq!(ok.parse::<ModelAbbrev>().unwrap().as_str(), ok);
        }
        for bad in ["", "mfg", "2A", "ABCDEFGHI", "M-G"] {
            assert!(bad.parse::<ModelAbbrev>().is_err(), "{bad:?}");
        }
        assert_eq!(
            "mfg/A21".parse::<NodeRef>(),
            Err(RefError::Abbrev("mfg".into()))
        );
        assert_eq!(
            "TOOLONGABB/A1".parse::<NodeRef>(),
            Err(RefError::Abbrev("TOOLONGABB".into()))
        );
        // A model abbreviation may itself look like a node number.
        let r: NodeRef = "A2/A21".parse().unwrap();
        assert_eq!(r.model().map(ModelAbbrev::as_str), Some("A2"));
        assert_eq!(r.node(), &node("A21"));
    }

    // FR-ID-07, metamodel §3.5: canonical A21/I1; FIPS A21I1 accepted; MFG/A21/I1 is
    // model / node / port; uncoded border ports are A21/[I1].
    #[test]
    fn port_refs() {
        assert_eq!(port("A21I1"), port("A21/I1"));
        assert_eq!(port("A21I1").to_string(), "A21/I1");
        assert_eq!(port("A21/I1").fips().to_string(), "A21I1");
        let p = port("MFG/A21/I1");
        assert_eq!(p.model().map(ModelAbbrev::as_str), Some("MFG"));
        assert_eq!(p.node(), &node("A21"));
        assert_eq!((p.side(), p.code()), (Side::I, PortCode::Icom(1)));
        assert_eq!(port("MFG/A21C3"), port("MFG/A21/C3"));
        assert_eq!(port("A0/M2").to_string(), "A0/M2");
        let b = port("A21/[I1]");
        assert_eq!(b.code(), PortCode::Border(1));
        assert_eq!(b.to_string(), "A21/[I1]");
        assert_eq!(b.fips().to_string(), "A21/[I1]");
        assert_ne!(port("A21/[I1]"), port("A21/I1"));
    }

    #[test]
    fn invalid_port_refs() {
        let cases = [
            (
                "A21/I0",
                RefError::Expected {
                    at: 5,
                    expected: "a number from 1",
                },
            ),
            (
                "A21/I",
                RefError::Expected {
                    at: 5,
                    expected: "a number from 1",
                },
            ),
            ("A21/I01", RefError::LeadingZero { at: 5 }),
            ("A21/I256", RefError::Overflow { at: 5 }),
            (
                "A21/X1",
                RefError::Expected {
                    at: 4,
                    expected: "`I`, `C`, `O` or `M`",
                },
            ),
            (
                "A21",
                RefError::Expected {
                    at: 3,
                    expected: "`I`, `C`, `O` or `M`",
                },
            ),
            (
                "A21[I1]",
                RefError::Expected {
                    at: 3,
                    expected: "`/` before `[`",
                },
            ),
            (
                "A21/[I1",
                RefError::Expected {
                    at: 7,
                    expected: "`]`",
                },
            ),
            ("A21/I1x", RefError::Trailing { at: 6 }),
            (
                "A21F/I1",
                RefError::Expected {
                    at: 3,
                    expected: "`I`, `C`, `O` or `M`",
                },
            ),
            ("@validate-order/I1", RefError::KeyRef { at: 0 }),
            ("MFG/@validate-order/I1", RefError::KeyRef { at: 4 }),
        ];
        for (input, err) in cases {
            assert_eq!(input.parse::<PortRef>(), Err(err), "{input:?}");
        }
    }

    // Ordering is FIPS node-index order, so BTreeMap<NodeId, _> iterates deterministically
    // in the order a node index lists diagrams (D6).
    #[test]
    fn node_index_order() {
        let mut nodes: Vec<NodeId> = ["A2", "A11", "A-0", "A1", "A0", "A12", "A-1"]
            .into_iter()
            .map(node)
            .collect();
        nodes.sort();
        let sorted: Vec<String> = nodes.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, ["A-0", "A-1", "A0", "A1", "A11", "A12", "A2"]);
    }

    #[test]
    fn constructors_reject_zero() {
        assert!(PortRef::new(None, NodeId::a0(), Side::I, PortCode::Icom(0)).is_none());
        assert!(PortRef::new(None, NodeId::a0(), Side::I, PortCode::Border(0)).is_none());
        assert!(SheetSuffix::new(SheetKind::Text, Some(0)).is_none());
        assert!(NodeId::from_boxes(&[2, 0]).is_none());
    }
}
