//! The layout bench fixtures (layout §16.4) are valid models: they parse and type-check clean,
//! and `dense-7x40` has the advertised 7 boxes and 40 arrows on A0.

use idef0_core::dsl::parse;
use idef0_core::typecheck::typecheck;

#[test]
fn bench_fixtures_are_valid() {
    for (file, arrows) in [("dense-7x40", 40), ("arrange-worst-7", 52)] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../fixtures/bench/{file}.idef0"));
        let parsed = parse(&std::fs::read_to_string(path).expect("fixture"));
        let found: Vec<String> = parsed.diagnostics.iter().map(ToString::to_string).collect();
        assert!(found.is_empty(), "{file}: {found:#?}");
        let model = parsed.model.expect("model");
        let tc: Vec<String> = typecheck(&model).iter().map(ToString::to_string).collect();
        assert!(tc.is_empty(), "{file}: {tc:#?}");
        let a0 = &model.diagrams[&"A0".parse().expect("A0")];
        assert_eq!((a0.boxes.len(), a0.arrows.len()), (7, arrows), "{file}");
    }
}
