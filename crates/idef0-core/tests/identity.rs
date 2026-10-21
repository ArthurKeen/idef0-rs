//! Identity (roadmap M1.4; metamodel §3.3, §3.4; D6, D14): keys follow the slug rule and the
//! arrow ids of the JSON fixtures are pinned in `tests/model_json/<name>.ids`, so every CI
//! target (Linux, macOS, Windows) must compute the same ids from the same bytes. The wasm32
//! comparison runs with the cross-target suite (testing §4).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use idef0_core::identity::{KeyMinter, SlugMinter, arrow_ids, slug};
use idef0_core::model::Model;
use idef0_core::text::{BoxKey, ConceptKey};
use proptest::prelude::*;

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/model_json")
}

/// Every box key and arrow id of a model, one per line, in model order.
fn id_listing(model: &Model) -> String {
    let mut out = String::new();
    for (node, diagram) in &model.diagrams {
        for b in &diagram.boxes {
            writeln!(out, "box {node} @{}", b.key).expect("write to String");
        }
        for s in arrow_ids(node, diagram) {
            writeln!(out, "arrow {}", s.trunk).expect("write to String");
            for p in s.parts {
                writeln!(out, "arrow {p}").expect("write to String");
            }
        }
    }
    out
}

fn load(name: &str) -> (String, Model) {
    let bytes = std::fs::read_to_string(dir().join(format!("{name}.json"))).expect("fixture");
    let model = serde_json::from_str(&bytes).expect("fixture deserializes");
    (bytes, model)
}

// D6, D14: the same bytes give the same ids, and the ids are pinned.
#[test]
fn fixture_ids_are_pinned() {
    for name in ["factory-a0", "constructs"] {
        let (bytes, model) = load(name);
        let again: Model = serde_json::from_str(&bytes).expect("re-reads");
        let listing = id_listing(&model);
        assert_eq!(
            listing,
            id_listing(&again),
            "{name}: same bytes, different ids"
        );
        let pinned =
            std::fs::read_to_string(dir().join(format!("{name}.ids"))).expect("pinned ids");
        assert_eq!(
            listing, pinned,
            "{name}: ids differ from tests/model_json/{name}.ids"
        );
    }
}

// metamodel §3.4: in a well-typed model every arrow id is unique.
#[test]
fn fixture_ids_are_unique() {
    for name in ["factory-a0", "constructs"] {
        let (_, model) = load(name);
        let mut seen = BTreeSet::new();
        for (node, diagram) in &model.diagrams {
            for s in arrow_ids(node, diagram) {
                for id in std::iter::once(s.trunk).chain(s.parts) {
                    assert!(
                        seen.insert(id.to_string()),
                        "{name}: duplicate arrow id {id}"
                    );
                }
            }
        }
    }
}

proptest! {
    // metamodel §3.3: a slug is always a valid key of at most 48 characters, and the rule is
    // idempotent.
    #[test]
    fn slug_is_a_key(name in "\\PC{0,80}") {
        let s = slug(&name, "box");
        prop_assert!(s.len() <= 48, "{s:?}");
        prop_assert!(s.parse::<BoxKey>().is_ok(), "{s:?}");
        prop_assert!(s.parse::<ConceptKey>().is_ok(), "{s:?}");
        prop_assert_eq!(slug(&s, "box"), s.clone());
    }

    // metamodel §3.3 step 5, D17: the minter never returns a taken (or retired) key, and the
    // same inputs give the same key.
    #[test]
    fn minter_avoids_taken_keys(
        name in "[A-Za-z ]{0,12}",
        taken in proptest::collection::btree_set("[a-z]{1,4}(-[2-5])?", 0..12),
    ) {
        let is_taken = |k: &BoxKey| taken.contains(k.as_str());
        let first = SlugMinter.box_key(&name, &is_taken);
        prop_assert!(!taken.contains(first.as_str()), "{first}");
        prop_assert_eq!(SlugMinter.box_key(&name, &is_taken), first);
    }
}
