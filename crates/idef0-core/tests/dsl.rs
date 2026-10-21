//! The `.idef0` parser (roadmap M2.1; metamodel §14, §16; testing §9, validation FR-VAL-19/20).

use std::path::{Path, PathBuf};

use idef0_core::diag::Severity;
use idef0_core::dsl::{lex::lex, parse};
use idef0_core::identity::arrow_ids;
use idef0_core::model::Model;
use idef0_core::typecheck::typecheck;
use proptest::prelude::*;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo(rel)).expect("fixture exists")
}

fn parse_clean(rel: &str) -> Model {
    let p = parse(&read(rel));
    let found: Vec<String> = p.diagnostics.iter().map(ToString::to_string).collect();
    assert!(found.is_empty(), "{rel}: {found:#?}");
    p.model.expect("a model")
}

fn json(rel: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .expect("json fixture");
    let model: Model = serde_json::from_str(&text).expect("json fixture deserializes");
    serde_json::to_value(model).expect("serializes")
}

// The DSL and the JSON fixtures describe the same models: lowering keeps every field.
#[test]
fn dsl_equals_json_fixtures() {
    let pairs = [
        (
            "fixtures/valid/constructs.idef0",
            "tests/model_json/constructs.json",
        ),
        (
            "fixtures/reference/factory-a0.idef0",
            "tests/model_json/factory-a0.json",
        ),
    ];
    for (dsl, json_file) in pairs {
        let got = serde_json::to_value(parse_clean(dsl)).expect("serializes");
        assert_eq!(got, json(json_file), "{dsl} does not lower to {json_file}");
    }
}

// FR-VAL-19: valid fixtures parse with no findings and pass the type check.
#[test]
fn valid_fixtures_are_clean() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo("fixtures/valid"))
        .expect("fixtures/valid")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "idef0"))
        .collect();
    files.extend([
        repo("fixtures/dogfood/idef0-rs.idef0"),
        repo("fixtures/reference/factory-a0.idef0"),
    ]);
    files.sort();
    assert!(files.len() >= 4, "{files:?}");
    for f in files {
        let rel = f
            .strip_prefix(repo(""))
            .expect("in repo")
            .to_string_lossy()
            .into_owned();
        let model = parse_clean(&rel);
        let tc: Vec<String> = typecheck(&model).iter().map(ToString::to_string).collect();
        assert!(tc.is_empty(), "{rel}: {tc:#?}");
    }
}

// fixtures/dogfood/EXPECTED.md §2: the arrow ids of diagram A0, written from the spec.
#[test]
fn dogfood_arrow_ids_match_expected_md() {
    let model = parse_clean("fixtures/dogfood/idef0-rs.idef0");
    let expected = read("fixtures/dogfood/EXPECTED.md");
    let section = expected
        .split("## 2. Arrow ids")
        .nth(1)
        .expect("section 2")
        .split("\n## ")
        .next()
        .expect("body");
    let mut want: Vec<(String, usize)> = Vec::new();
    for row in section
        .lines()
        .filter(|l| l.starts_with("| ") && l.contains("`A0:"))
    {
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        let id = cells[2].trim_matches('`').to_owned();
        let leaves = cells[3].matches("`/").count();
        want.push((id, leaves));
    }
    assert_eq!(want.len(), 14, "EXPECTED.md lists 14 arrows on A0");
    let a0 = &model.diagrams[&"A0".parse().expect("A0")];
    let got: Vec<(String, usize)> = arrow_ids(&"A0".parse().expect("A0"), a0)
        .into_iter()
        .map(|s| (s.trunk.to_string(), s.parts.len()))
        .collect();
    assert_eq!(got, want);
    let a_minus_0 = &model.diagrams[&"A-0".parse().expect("A-0")];
    let ids: Vec<String> = arrow_ids(&"A-0".parse().expect("A-0"), a_minus_0)
        .into_iter()
        .map(|s| s.trunk.to_string())
        .collect();
    assert!(
        ids.contains(&"A-0:I1->0I1#0".to_owned()) && ids.contains(&"A-0:0O2->O2#0".to_owned()),
        "{ids:?}"
    );
}

// FR-VAL-20: each invalid fixture contains its directory's code. Its full diagnostics are a
// snapshot (roadmap M2.1: multi-error snapshots).
#[test]
fn invalid_fixtures_contain_their_code() {
    insta::glob!("../../../fixtures", "{invalid,lint}/*/*.idef0", |path| {
        let code = path
            .parent()
            .and_then(Path::file_name)
            .expect("code dir")
            .to_string_lossy()
            .into_owned();
        let src = std::fs::read_to_string(path).expect("fixture");
        let p = parse(&src);
        let mut found = p.diagnostics.clone();
        if let Some(m) = &p.model {
            found.extend(typecheck(m));
        }
        assert!(
            found.iter().any(|d| d.code.as_str() == code),
            "{}: no {code} in {found:#?}",
            path.display()
        );
        let errors = found
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let report: Vec<String> = found.iter().map(ToString::to_string).collect();
        insta::assert_snapshot!(format!("{code}: {errors} error(s)\n{}", report.join("\n")));
    });
}

// The parser recovers and keeps going: one file, several independent errors.
#[test]
fn recovery_reports_every_error_and_keeps_parsing() {
    let p = parse(&read("fixtures/invalid/E-PARSE-01/recovery.idef0"));
    let lines: Vec<u32> = p
        .diagnostics
        .iter()
        .filter_map(|d| d.span.map(|s| s.line))
        .collect();
    assert!(p.diagnostics.len() >= 5, "{:#?}", p.diagnostics);
    assert!(
        lines.windows(2).all(|w| w[0] <= w[1]),
        "sorted by position: {lines:?}"
    );
    let m = p.model.expect("a partial model");
    assert!(
        m.concepts.contains_key(&"invoice".parse().expect("key")),
        "the concept after the broken one"
    );
    assert!(
        m.diagrams.contains_key(&"A1".parse().expect("A1")),
        "the diagram after the broken one"
    );
}

fn snippet(body: &str) -> Model {
    let src = format!("idef0 0.2\nmodel \"Snippet\" {{\n  abbrev \"SNP\"\n}}\n{body}");
    let p = parse(&src);
    let found: Vec<String> = p.diagnostics.iter().map(ToString::to_string).collect();
    assert!(found.is_empty(), "{found:#?}");
    p.model.expect("a model")
}

// metamodel §3.3: a derived key avoids every explicit key in the file, even one declared
// later, so the slug collision takes the lowest free suffix.
#[test]
fn derived_keys_avoid_explicit_keys() {
    let m = snippet("diagram A0 {\n  box 1 \"Check Order\"\n  box 2 \"Ship\" key=check-order\n}\n");
    let keys: Vec<&str> = m.diagrams[&"A0".parse().expect("A0")]
        .boxes
        .iter()
        .map(|b| b.key.as_str())
        .collect();
    assert_eq!(keys, ["check-order-2", "check-order"]);
    assert!(typecheck(&m).is_empty(), "no false E-ID-02");
}

// §16.2 note_target: `src -> dst #n` keeps its occurrence index; without `#n` it is 0.
#[test]
fn note_arrow_occurrences() {
    let m = snippet(concat!(
        "diagram A0 {\n  box 1 \"Plan\"\n  box 2 \"Do\"\n",
        "  arrow : 1O1 -> 2I1\n",
        "  note 1 \"n\" on 1O1 -> 2I1 #1, 1O1 -> 2I1\n}\n",
    ));
    let note = &m.diagrams[&"A0".parse().expect("A0")].notes[0];
    let occurrences: Vec<u32> = note
        .on
        .iter()
        .filter_map(|t| match t {
            idef0_core::model::NoteTarget::Arrow { occurrence, .. } => Some(*occurrence),
            _ => None,
        })
        .collect();
    assert_eq!(occurrences, [1, 0]);
}

// FR-WS-06: the tree is lossless.
#[test]
fn fixtures_round_trip_through_the_cst() {
    for f in [
        "fixtures/dogfood/idef0-rs.idef0",
        "fixtures/valid/constructs.idef0",
        "fixtures/invalid/E-PARSE-01/recovery.idef0",
    ] {
        let src = read(f);
        assert_eq!(parse(&src).cst.text(&src), src, "{f}");
    }
}

proptest! {
    // The lexer and parser never panic, and the CST holds every byte, whatever the input.
    #[test]
    fn any_input_is_lossless(src in "\\PC{0,200}") {
        let joined: String = lex(&src).iter().map(|t| t.text(&src)).collect();
        prop_assert_eq!(&joined, &src);
        prop_assert_eq!(parse(&src).cst.text(&src), src);
    }

    // Damaged real files exercise recovery: cutting out any byte range never panics and keeps
    // the CST lossless.
    #[test]
    fn damaged_fixtures_recover(cut in 0usize..4000, len in 0usize..60) {
        let src = read("fixtures/dogfood/idef0-rs.idef0");
        let start = src.char_indices().map(|(i, _)| i).find(|&i| i >= cut.min(src.len())).unwrap_or(src.len());
        let end = src.char_indices().map(|(i, _)| i).find(|&i| i >= start + len).unwrap_or(src.len());
        let damaged = format!("{}{}", &src[..start], &src[end..]);
        let p = parse(&damaged);
        prop_assert_eq!(p.cst.text(&damaged), damaged);
        prop_assert!(p.diagnostics.len() <= 101);
    }
}
