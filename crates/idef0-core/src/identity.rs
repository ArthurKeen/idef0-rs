//! Identity: key minting and structural arrow ids (`docs/spec/metamodel.md` §3.3, §3.4; D6,
//! D14; roadmap M1.4).
//!
//! Everything here is a pure function of its input. Nothing reads a clock or a random source,
//! so the same model bytes give the same keys and ids on every target.

use core::fmt;
use std::collections::BTreeMap;

use compact_str::CompactString;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::id::{NodeId, PortCode};
use crate::model::{ArrowShape, BoundaryPort, Diagram, Endpoint};
use crate::text::{BoxKey, ConceptKey};

/// The longest slug before a collision suffix (metamodel §3.3 step 3).
const MAX_SLUG: usize = 48;

/// The slug rule (metamodel §3.3, normative):
///
/// 1. Apply Unicode NFKD and drop combining marks.
/// 2. Lowercase. Replace every run of characters outside `[a-z0-9]` with `-`.
/// 3. Trim leading and trailing `-`. Truncate to 48 characters, at a `-` boundary when one
///    exists.
/// 4. If the result is empty, use `fallback` (`box` or `concept`).
///
/// Step 5, the collision suffix, is [`KeyMinter`]'s job.
///
/// ```
/// use idef0_core::identity::slug;
///
/// assert_eq!(slug("Validate Order", "box"), "validate-order");
/// assert_eq!(slug("Résumé Café", "box"), "resume-cafe");
/// assert_eq!(slug("注文", "concept"), "concept");
/// ```
pub fn slug(name: &str, fallback: &str) -> String {
    let mut s = String::with_capacity(name.len());
    let mut dash = false;
    for c in name
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
    {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            s.push(c);
            dash = false;
        } else if !dash {
            s.push('-');
            dash = true;
        }
    }
    let mut out = s.trim_matches('-');
    // Every byte is ASCII now, so byte and character positions agree.
    if out.len() > MAX_SLUG {
        let head = &out[..MAX_SLUG];
        out = if out.as_bytes()[MAX_SLUG] == b'-' {
            head
        } else {
            head.rfind('-').map_or(head, |i| &head[..i])
        };
        out = out.trim_end_matches('-');
    }
    if out.is_empty() {
        fallback.to_owned()
    } else {
        out.to_owned()
    }
}

/// `base`, or `base-2`, `base-3`, … : the lowest suffix that is not taken (§3.3 step 5).
fn lowest_free(base: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    (2..=u64::MAX)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| base.to_owned())
}

/// Mints new keys (architecture §4.6). The command layer and the parser (for files without
/// keys) take one by injection, so tests can substitute their own.
///
/// `taken` must answer for every key already in the model *and* every retired key (D17): a
/// minter never returns a key `taken` accepts.
pub trait KeyMinter {
    /// A key for a new box named `name`.
    fn box_key(&mut self, name: &str, taken: &dyn Fn(&BoxKey) -> bool) -> BoxKey;
    /// A key for a new concept named `name`.
    fn concept_key(&mut self, name: &str, taken: &dyn Fn(&ConceptKey) -> bool) -> ConceptKey;
}

/// The normative minter: [`slug`] plus the lowest free collision suffix. It keeps no state, so
/// it is deterministic by construction.
#[derive(Debug, Clone, Copy, Default)]
pub struct SlugMinter;

impl KeyMinter for SlugMinter {
    fn box_key(&mut self, name: &str, taken: &dyn Fn(&BoxKey) -> bool) -> BoxKey {
        let key = lowest_free(&slug(name, "box"), &|s| taken(&BoxKey::from_slug(s)));
        BoxKey::from_slug(&key)
    }

    fn concept_key(&mut self, name: &str, taken: &dyn Fn(&ConceptKey) -> bool) -> ConceptKey {
        let key = lowest_free(&slug(name, "concept"), &|s| {
            taken(&ConceptKey::from_slug(s))
        });
        ConceptKey::from_slug(&key)
    }
}

/// An arrow id (metamodel §3.4): an explicit key verbatim, or a structural id such as
/// `A0:1O1->2I1#0`. Structural ids contain `:`, which keys cannot, so the two never collide.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArrowId(CompactString);

impl ArrowId {
    /// The id as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArrowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The ids of one arrow statement: the arrow (or bundle trunk), and one per fork branch or
/// join leaf in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementIds {
    /// The plain arrow, or the bundle trunk.
    pub trunk: ArrowId,
    /// The branches or leaves; empty for a plain arrow.
    pub parts: Vec<ArrowId>,
}

/// The box number of the box at `index` on the diagram `node` (metamodel §3.2): 0 on A-0; on
/// A-n the lineage box shows 0 (A-1) or `−(n−1)`; otherwise the diagonal position, 1-based.
pub fn box_number(node: &NodeId, diagram: &Diagram, index: usize) -> i64 {
    let position = i64::try_from(index).map_or(i64::MAX, |i| i + 1);
    match node {
        NodeId::Context { level: 0 } => 0,
        NodeId::Context { level } if diagram.lineage.map(usize::from) == Some(index) => {
            1 - i64::from(*level)
        }
        _ => position,
    }
}

/// The ids of every arrow statement on a diagram, in declaration order (metamodel §3.4).
pub fn arrow_ids(node: &NodeId, diagram: &Diagram) -> Vec<StatementIds> {
    ids(node, diagram, true)
}

/// The structural ids, ignoring explicit keys: what [`arrow_ids`] would give if no arrow had
/// a `key=`. Used to name an arrow whose explicit key is duplicated.
pub fn structural_ids(node: &NodeId, diagram: &Diagram) -> Vec<StatementIds> {
    ids(node, diagram, false)
}

fn ids(node: &NodeId, diagram: &Diagram, use_keys: bool) -> Vec<StatementIds> {
    let end = |e: &Endpoint| endpoint(node, diagram, e);
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    let mut out = Vec::with_capacity(diagram.arrows.len());
    for arrow in &diagram.arrows {
        let (pattern, part_keys) = match &arrow.shape {
            ArrowShape::Plain { src, dst } => (format!("{}->{}", end(src), end(dst)), vec![]),
            ArrowShape::Fork { src, fork } => (
                format!("{}->*", end(src)),
                fork.iter().map(|b| b.attrs.key.as_ref()).collect(),
            ),
            ArrowShape::Join { join, dst } => (
                format!("*->{}", end(dst)),
                join.iter().map(|l| l.attrs.key.as_ref()).collect(),
            ),
        };
        let n = seen.entry(pattern.clone()).or_insert(0);
        let structural = format!("{node}:{pattern}#{n}");
        *n += 1;
        let trunk = match &arrow.attrs.key {
            Some(k) if use_keys => k.as_str().to_owned(),
            _ => structural,
        };
        let parts = part_keys
            .into_iter()
            .enumerate()
            .map(|(i, key)| match key {
                Some(k) if use_keys => ArrowId(CompactString::new(k.as_str())),
                _ => ArrowId(CompactString::new(format!("{trunk}/{i}"))),
            })
            .collect();
        out.push(StatementIds {
            trunk: ArrowId(CompactString::new(trunk)),
            parts,
        });
    }
    out
}

/// An endpoint in its canonical §16 form with an explicit rank: `1O1`, `2I1`, boundary `I1`,
/// uncoded `[I1]`. A box that is not on the diagram (a type error) is written `@key`.
fn endpoint(node: &NodeId, diagram: &Diagram, e: &Endpoint) -> String {
    match e {
        Endpoint::Box { key, side, rank } => {
            let number = diagram
                .boxes
                .iter()
                .position(|b| &b.key == key)
                .map_or_else(
                    || format!("@{key}"),
                    |i| box_number(node, diagram, i).to_string(),
                );
            format!("{number}{}{rank}", side.letter())
        }
        Endpoint::Boundary(p) => port(*p),
    }
}

fn port(p: BoundaryPort) -> String {
    match p.code {
        PortCode::Icom(n) => format!("{}{n}", p.side.letter()),
        PortCode::Border(n) => format!("[{}{n}]", p.side.letter()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    // metamodel §3.3 steps 1-4.
    #[test]
    fn slug_rule() {
        let cases = [
            ("Validate Order", "validate-order"),
            ("Develop/Update a Factory", "develop-update-a-factory"),
            ("Résumé Café", "resume-cafe"),
            ("  --Hello__World--  ", "hello-world"),
            ("A0 Plan 2", "a0-plan-2"),
            ("Straße", "stra-e"),
            ("ﬁle Ⅻ", "file-xii"),
            ("İstanbul", "istanbul"),
        ];
        for (name, want) in cases {
            assert_eq!(slug(name, "box"), want, "{name:?}");
        }
        for empty in ["", "!!!", "注文", "   "] {
            assert_eq!(slug(empty, "box"), "box", "{empty:?}");
            assert_eq!(slug(empty, "concept"), "concept", "{empty:?}");
        }
    }

    // metamodel §3.3 step 3: 48 characters, cut at a `-` when one exists.
    #[test]
    fn slug_truncation() {
        let words = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let s = slug(words, "box");
        assert_eq!(s, "alpha-beta-gamma-delta-epsilon-zeta-eta-theta");
        assert!(s.len() <= 48);
        let one_word = "x".repeat(60);
        assert_eq!(slug(&one_word, "box"), "x".repeat(48));
        let boundary_at_48 = format!("{} tail", "y".repeat(48));
        assert_eq!(slug(&boundary_at_48, "box"), "y".repeat(48));
    }

    // metamodel §3.3 step 5, D17: the lowest free suffix; retired keys count as taken.
    #[test]
    fn minter_collisions() {
        let taken: BTreeSet<&str> =
            ["validate-order", "validate-order-2", "box", "retired-one"].into();
        let is_taken = |k: &BoxKey| taken.contains(k.as_str());
        let mut m = SlugMinter;
        assert_eq!(
            m.box_key("Validate Order", &is_taken).as_str(),
            "validate-order-3"
        );
        assert_eq!(m.box_key("Ship Order", &is_taken).as_str(), "ship-order");
        assert_eq!(m.box_key("???", &is_taken).as_str(), "box-2");
        assert_eq!(
            m.box_key("Retired One", &is_taken).as_str(),
            "retired-one-2"
        );
        let none = |_: &ConceptKey| false;
        assert_eq!(
            m.concept_key("Purchase Order", &none).as_str(),
            "purchase-order"
        );
        assert_eq!(m.concept_key("", &none).as_str(), "concept");
    }

    fn diagram(json: &str) -> Diagram {
        serde_json::from_str(json).unwrap()
    }

    fn ids_of(node: &str, d: &Diagram) -> Vec<(String, Vec<String>)> {
        arrow_ids(&node.parse().unwrap(), d)
            .into_iter()
            .map(|s| {
                (
                    s.trunk.to_string(),
                    s.parts.iter().map(ToString::to_string).collect(),
                )
            })
            .collect()
    }

    // metamodel §3.4: the table's examples.
    #[test]
    fn structural_ids_follow_section_3_4() {
        let d = diagram(
            r#"{"boxes":[{"key":"a","name":"A"},{"key":"b","name":"B"}],"arrows":[
                {"src":{"box":"a","side":"O","rank":1},"dst":{"box":"b","side":"I","rank":1}},
                {"src":{"side":"C","icom":1},"fork":[{"dst":{"box":"a","side":"C","rank":1}},{"dst":{"box":"b","side":"C","rank":1}}]},
                {"join":[{"src":{"box":"a","side":"O","rank":2}},{"src":{"box":"b","side":"O","rank":2}}],"dst":{"side":"O","icom":2}},
                {"key":"credit-check-feed","src":{"side":"I","border":1},"dst":{"box":"b","side":"I","rank":2}},
                {"src":{"box":"a","side":"O","rank":1},"dst":{"box":"b","side":"I","rank":1}}
            ]}"#,
        );
        assert_eq!(
            ids_of("A0", &d),
            [
                ("A0:1O1->2I1#0".into(), vec![]),
                (
                    "A0:C1->*#0".into(),
                    vec!["A0:C1->*#0/0".into(), "A0:C1->*#0/1".into()]
                ),
                (
                    "A0:*->O2#0".into(),
                    vec!["A0:*->O2#0/0".into(), "A0:*->O2#0/1".into()]
                ),
                ("credit-check-feed".into(), vec![]),
                ("A0:1O1->2I1#1".into(), vec![]),
            ]
        );
        let s = structural_ids(&"A0".parse().unwrap(), &d);
        assert_eq!(s[3].trunk.as_str(), "A0:[I1]->2I2#0", "keys ignored");
    }

    // Leaf keys are verbatim; a keyed trunk names its unkeyed leaves.
    #[test]
    fn keyed_bundles() {
        let d = diagram(
            r#"{"boxes":[{"key":"a","name":"A"},{"key":"b","name":"B"}],"arrows":[
                {"key":"orders","src":{"side":"I","icom":1},"fork":[
                    {"key":"to-a","dst":{"box":"a","side":"I","rank":1}},
                    {"dst":{"box":"b","side":"I","rank":1}}]}
            ]}"#,
        );
        assert_eq!(
            ids_of("A21", &d),
            [("orders".into(), vec!["to-a".into(), "orders/1".into()])]
        );
    }

    // metamodel §3.2: A-0 shows box 0; on A-n the lineage box shows 0 (A-1) or -(n-1).
    #[test]
    fn context_box_numbers() {
        let a_minus_0 = diagram(
            r#"{"boxes":[{"key":"top","name":"T"}],"arrows":[
            {"src":{"side":"I","icom":1},"dst":{"box":"top","side":"I","rank":1}}]}"#,
        );
        assert_eq!(ids_of("A-0", &a_minus_0)[0].0, "A-0:I1->0I1#0");
        let higher = r#"{"boxes":[{"key":"x","name":"X"},{"key":"l","name":"L"},{"key":"y","name":"Y"}],"lineage":1,"arrows":[
            {"src":{"box":"x","side":"O","rank":1},"dst":{"box":"l","side":"C","rank":1}},
            {"src":{"box":"l","side":"O","rank":1},"dst":{"box":"y","side":"I","rank":1}}]}"#;
        let d = diagram(higher);
        assert_eq!(ids_of("A-1", &d)[0].0, "A-1:1O1->0C1#0");
        assert_eq!(ids_of("A-1", &d)[1].0, "A-1:0O1->3I1#0");
        assert_eq!(ids_of("A-3", &d)[0].0, "A-3:1O1->-2C1#0");
    }

    // A box that is not on the diagram (a type error) still gets a deterministic id.
    #[test]
    fn unknown_box_ids() {
        let d = diagram(
            r#"{"arrows":[{"src":{"box":"nope","side":"O","rank":1},"dst":{"side":"O","icom":1}}]}"#,
        );
        assert_eq!(ids_of("A0", &d)[0].0, "A0:@nopeO1->O1#0");
    }
}
