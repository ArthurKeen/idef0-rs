//! JSON round-trip of whole models (roadmap M1.2 acceptance; D6).
//!
//! Each fixture in `tests/model_json/` must (1) deserialize, (2) keep every field it states,
//! which catches a misspelled field that serde would otherwise ignore, and (3) serialize back
//! to exactly the same bytes, so identical models give identical JSON.

use std::path::Path;

use idef0_core::id::NodeId;
use idef0_core::model::{DiagramKind, Dre, Model};

fn roundtrip(file: &str) -> Model {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/model_json")
        .join(file);
    let text = std::fs::read_to_string(&path).expect("fixture exists");
    let model: Model = serde_json::from_str(&text).expect("fixture deserializes");

    let written: serde_json::Value = serde_json::from_str(&text).expect("fixture is JSON");
    let kept = serde_json::to_value(&model).expect("model serializes");
    assert_eq!(
        kept, written,
        "{file}: a field was dropped or changed on the way in"
    );

    let mut again = serde_json::to_string_pretty(&model).expect("model serializes");
    again.push('\n');
    assert_eq!(
        again, text,
        "{file}: JSON is not byte-identical after a round-trip"
    );
    assert_eq!(
        serde_json::from_str::<Model>(&again).expect("re-reads"),
        model
    );
    model
}

#[test]
fn factory_a0() {
    let m = roundtrip("factory-a0.json");
    assert_eq!(m.concepts.len(), 14);
    let a0 = &m.diagrams[&"A-0".parse::<NodeId>().unwrap()];
    assert_eq!(a0.arrows.len(), 14);
}

#[test]
fn constructs() {
    let m = roundtrip("constructs.json");
    let order: Vec<String> = m.diagrams.keys().map(ToString::to_string).collect();
    assert_eq!(
        order,
        ["A-0", "A-1", "A0", "A3"],
        "diagrams in FIPS node-index order"
    );
    let kinds: Vec<DiagramKind> = m.diagrams.keys().map(DiagramKind::of).collect();
    assert_eq!(
        kinds,
        [
            DiagramKind::Context,
            DiagramKind::HigherContext,
            DiagramKind::Decomposition,
            DiagramKind::Decomposition
        ]
    );
    let a3: NodeId = "A3".parse().unwrap();
    assert!(
        matches!(m.dre(&a3), Some(Dre::NodeNumber(_))),
        "A3 is detailed"
    );
    assert_eq!(m.dre(&"A1".parse().unwrap()), None, "A1 is a leaf");
}
