//! The core type check (roadmap M1.3; metamodel §3.3, §6.3, §7, §9, §10, §16.3). Models are
//! written as JSON; the `.idef0` forms of these cases arrive with the parser (M2.1) under
//! `fixtures/invalid/E-*/`.

use std::path::Path;

use idef0_core::diag::{Diagnostic, Layer, Severity};
use idef0_core::model::Model;
use idef0_core::typecheck::typecheck;
use serde_json::{Value, json};

fn model(diagrams: Value) -> Model {
    let mut m = json!({"meta": {"name": "Orders", "abbrev": "ORD", "retired": ["gone"]}});
    m["diagrams"] = diagrams;
    serde_json::from_value(m).expect("test model deserializes")
}

/// A-0 with the top box, and A0 with boxes `a`, `b`, `c` plus the given arrows and notes.
fn a0(arrows: Value, notes: Value) -> Model {
    let mut d = json!({
        "A-0": {"title": "Fulfill Orders", "boxes": [{"key": "top", "name": "Fulfill Orders"}]},
        "A0": {"boxes": [
            {"key": "a", "name": "Check Order"},
            {"key": "b", "name": "Ship Order"},
            {"key": "c", "name": "Bill Customer"},
        ]},
    });
    d["A0"]["arrows"] = arrows;
    d["A0"]["notes"] = notes;
    model(d)
}

fn codes(m: &Model) -> Vec<String> {
    typecheck(m).iter().map(|d| d.code.to_string()).collect()
}

fn found(m: &Model) -> Vec<(String, String)> {
    typecheck(m)
        .iter()
        .map(|d| (d.code.to_string(), d.subject.to_string()))
        .collect()
}

fn fixture(file: &str) -> Model {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/model_json")
        .join(file);
    let text = std::fs::read_to_string(path).expect("fixture exists");
    serde_json::from_str(&text).expect("fixture deserializes")
}

// The round-trip fixtures are well typed.
#[test]
fn fixtures_are_clean() {
    for f in ["factory-a0.json", "constructs.json"] {
        let found: Vec<String> = typecheck(&fixture(f))
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(found.is_empty(), "{f}: {found:#?}");
    }
}

// --- FR-ARROW-01, metamodel §6.3: every cell of the arrow-typing table ----------------------

/// The twelve kinds of arrow end: a box side, a coded boundary port, an uncoded border port.
fn ends(role: &str) -> Vec<(String, Value)> {
    let boxed = if role == "src" { "a" } else { "b" };
    let mut v = Vec::new();
    for s in ["I", "C", "O", "M"] {
        v.push((
            format!("box.{s}"),
            json!({"box": boxed, "side": s, "rank": 1}),
        ));
    }
    for s in ["I", "C", "O", "M"] {
        v.push((s.to_owned(), json!({"side": s, "icom": 1})));
    }
    for s in ["I", "C", "O", "M"] {
        v.push((format!("[{s}]"), json!({"side": s, "border": 1})));
    }
    v
}

/// The two rows of the metamodel §6.3 table, as written there.
fn allowed(src: &str, dst: &str) -> bool {
    let row1 = src == "box.O" && ["box.I", "box.C", "box.M", "O", "[O]"].contains(&dst);
    let row2 = ["I", "C", "M", "[I]", "[C]", "[M]"].contains(&src)
        && ["box.I", "box.C", "box.M"].contains(&dst);
    row1 || row2
}

#[test]
fn arrow_typing_table() {
    let legal_src = ["box.O", "I", "C", "M", "[I]", "[C]", "[M]"];
    let legal_dst = ["box.I", "box.C", "box.M", "O", "[O]"];
    let mut cells = 0;
    let mut allowed_cells = 0;
    for (s, src) in ends("src") {
        for (d, dst) in ends("dst") {
            cells += 1;
            let got = codes(&a0(json!([{"src": src, "dst": dst}]), json!([])));
            let boundary = |k: &str| !k.starts_with("box.");
            let expected: Vec<&str> = if allowed(&s, &d) {
                allowed_cells += 1;
                vec![]
            } else if boundary(&s) && boundary(&d) {
                vec!["E-TYPE-03"]
            } else {
                let mut e = vec![];
                if !legal_src.contains(&s.as_str()) {
                    e.push("E-TYPE-01");
                }
                if !legal_dst.contains(&d.as_str()) {
                    e.push("E-TYPE-02");
                }
                e
            };
            assert!(
                allowed(&s, &d) || !expected.is_empty(),
                "{s} -> {d}: forbidden but no code"
            );
            assert_eq!(got, expected, "{s} -> {d}");
        }
    }
    assert_eq!(cells, 144);
    assert_eq!(
        allowed_cells, 23,
        "§6.3: box.O has 5 destinations, the 6 boundary sources 3 each"
    );
}

// FR-ARROW-02: output-to-self and backward arrows are legal feedback.
#[test]
fn feedback_is_legal() {
    let m = a0(
        json!([
            {"src": {"box": "a", "side": "O", "rank": 1}, "dst": {"box": "a", "side": "C", "rank": 1}},
            {"src": {"box": "c", "side": "O", "rank": 1}, "dst": {"box": "a", "side": "I", "rank": 1}},
        ]),
        json!([]),
    );
    assert_eq!(codes(&m), Vec::<String>::new());
}

// validation §12: the type check rejects ends on boxes that are not on the diagram.
#[test]
fn unknown_box_ends() {
    let m = a0(
        json!([{"src": {"box": "nope", "side": "O", "rank": 1}, "dst": {"box": "gone-too", "side": "I", "rank": 1}}]),
        json!([]),
    );
    let all = typecheck(&m);
    assert_eq!(codes(&m), ["E-TYPE-01", "E-TYPE-02"]);
    assert!(
        all[0].message.contains("@nope is not on this diagram"),
        "{}",
        all[0]
    );
}

// Bundles are typed per flattened edge; a bad trunk end is reported once, on the trunk.
#[test]
fn bundles_are_typed_per_edge() {
    let fork = a0(
        json!([{"src": {"box": "a", "side": "I", "rank": 1}, "fork": [
            {"dst": {"box": "b", "side": "I", "rank": 1}},
            {"dst": {"box": "c", "side": "O", "rank": 1}},
        ]}]),
        json!([]),
    );
    assert_eq!(
        found(&fork),
        [
            ("E-TYPE-01".into(), "edge A0:#0".into()),
            ("E-TYPE-02".into(), "edge A0:#0/1".into())
        ]
    );
    let join = a0(
        json!([{"join": [
            {"src": {"box": "a", "side": "O", "rank": 1}},
            {"src": {"side": "I", "icom": 1}},
        ], "dst": {"side": "O", "icom": 1}}]),
        json!([]),
    );
    assert_eq!(found(&join), [("E-TYPE-03".into(), "edge A0:#0/1".into())]);
}

// --- FR-ARROW-04/05: port ranks per box side ---------------------------------------------------

fn input(rank: u8) -> Value {
    json!({"src": {"side": "I", "icom": rank}, "dst": {"box": "a", "side": "I", "rank": rank}})
}

#[test]
fn ranks_without_gaps_or_duplicates() {
    assert_eq!(
        codes(&a0(json!([input(1), input(2), input(3)]), json!([]))),
        Vec::<String>::new()
    );
    let gap = a0(json!([input(1), input(3)]), json!([]));
    assert_eq!(found(&gap), [("E-TYPE-04".into(), "edge A0:#1".into())]);
    let zero = a0(json!([input(0)]), json!([]));
    assert_eq!(codes(&zero), ["E-TYPE-04"]);
    let dup = a0(
        json!([
            {"src": {"box": "a", "side": "O", "rank": 1}, "dst": {"box": "b", "side": "I", "rank": 1}},
            {"src": {"box": "a", "side": "O", "rank": 1}, "dst": {"box": "c", "side": "I", "rank": 1}},
        ]),
        json!([]),
    );
    assert_eq!(found(&dup), [("E-TYPE-04".into(), "edge A0:#1".into())]);
}

// FR-ARROW-05: fork branches share the source rank; join leaves share the destination rank.
#[test]
fn bundles_share_their_trunk_rank() {
    let m = a0(
        json!([
            {"src": {"box": "a", "side": "O", "rank": 1}, "fork": [
                {"dst": {"box": "b", "side": "I", "rank": 1}},
                {"dst": {"box": "c", "side": "I", "rank": 1}},
            ]},
            {"join": [
                {"src": {"box": "b", "side": "O", "rank": 1}},
                {"src": {"box": "c", "side": "O", "rank": 1}},
            ], "dst": {"box": "a", "side": "C", "rank": 1}},
        ]),
        json!([]),
    );
    assert_eq!(codes(&m), Vec::<String>::new());
    let twice = a0(
        json!([{"src": {"box": "a", "side": "O", "rank": 1}, "fork": [
            {"dst": {"box": "b", "side": "I", "rank": 1}},
            {"dst": {"box": "b", "side": "I", "rank": 1}},
        ]}]),
        json!([]),
    );
    assert_eq!(found(&twice), [("E-TYPE-04".into(), "edge A0:#0/1".into())]);
}

// --- FR-TUN-01: `tunnel=box` needs exactly one box end ---------------------------------------

#[test]
fn ambiguous_box_tunnel() {
    let one_box = json!({"tunnel": "box", "src": {"side": "C", "icom": 1}, "dst": {"box": "a", "side": "C", "rank": 1}});
    assert_eq!(
        codes(&a0(json!([one_box]), json!([]))),
        Vec::<String>::new()
    );
    let two_boxes = json!({"tunnel": "box", "src": {"box": "a", "side": "O", "rank": 1}, "dst": {"box": "b", "side": "I", "rank": 1}});
    assert_eq!(codes(&a0(json!([two_boxes]), json!([]))), ["E-TYPE-05"]);
    let explicit = json!({"tunnel": "src", "src": {"box": "a", "side": "O", "rank": 1}, "dst": {"box": "b", "side": "I", "rank": 1}});
    assert_eq!(
        codes(&a0(json!([explicit]), json!([]))),
        Vec::<String>::new()
    );
    let branch = a0(
        json!([{"tunnel": "box", "src": {"box": "a", "side": "O", "rank": 1}, "fork": [
            {"dst": {"side": "O", "icom": 1}},
            {"dst": {"box": "b", "side": "I", "rank": 1}},
        ]}]),
        json!([]),
    );
    assert_eq!(
        found(&branch),
        [("E-TYPE-05".into(), "edge A0:#0/1".into())]
    );
}

// --- FR-ID-03, D17, metamodel §3.4: keys -----------------------------------------------------

#[test]
fn duplicate_and_retired_box_keys() {
    let dup = model(json!({
        "A-0": {"title": "T", "boxes": [{"key": "top", "name": "Top"}]},
        "A0": {"boxes": [{"key": "a", "name": "A"}, {"key": "b", "name": "B"}, {"key": "a", "name": "C"}]},
    }));
    assert_eq!(found(&dup), [("E-ID-02".into(), "box @a".into())]);
    let across = model(json!({
        "A-0": {"title": "T", "boxes": [{"key": "a", "name": "Top"}]},
        "A0": {"boxes": [{"key": "a", "name": "A"}]},
    }));
    assert_eq!(codes(&across), ["E-ID-02"]);
    let retired = model(json!({"A0": {"boxes": [{"key": "gone", "name": "Gone"}]}}));
    let d = typecheck(&retired);
    assert_eq!(codes(&retired), ["E-ID-02"]);
    assert!(d[0].message.contains("retired"), "{}", d[0]);
}

// metamodel §3.2: the A-1 lineage box is the top box shown again; other boxes may not reuse
// its key.
#[test]
fn a_minus_1_lineage_shares_the_top_key() {
    let ok = model(json!({
        "A-1": {"title": "Run", "boxes": [{"key": "plan", "name": "Plan"}, {"key": "top", "name": "Top"}], "lineage": 1},
        "A-0": {"title": "T", "boxes": [{"key": "top", "name": "Top"}]},
    }));
    assert_eq!(codes(&ok), Vec::<String>::new());
    let not_lineage = model(json!({
        "A-1": {"title": "Run", "boxes": [{"key": "top", "name": "Top"}, {"key": "x", "name": "X"}], "lineage": 1},
        "A-0": {"title": "T", "boxes": [{"key": "top", "name": "Top"}]},
    }));
    assert_eq!(codes(&not_lineage), ["E-ID-02"]);
}

#[test]
fn duplicate_arrow_keys() {
    let m = a0(
        json!([
            {"key": "feed", "src": {"side": "I", "icom": 1}, "dst": {"box": "a", "side": "I", "rank": 1}},
            {"src": {"box": "a", "side": "O", "rank": 1}, "fork": [
                {"key": "feed", "dst": {"box": "b", "side": "I", "rank": 1}},
                {"dst": {"box": "c", "side": "I", "rank": 1}},
            ]},
        ]),
        json!([]),
    );
    assert_eq!(found(&m), [("E-ID-04".into(), "edge A0:#1/0".into())]);
}

// --- metamodel §3.2, §16.3: box numbering and parent boxes -----------------------------------

#[test]
fn box_numbering() {
    let two_on_context = model(json!({"A-0": {"title": "T", "boxes": [
        {"key": "a", "name": "A"}, {"key": "b", "name": "B"}]}}));
    assert_eq!(
        found(&two_on_context),
        [("E-NODE-02".into(), "diagram A-0".into())]
    );
    let boxes: Vec<Value> = (1..=8)
        .map(|i| json!({"key": format!("b{i}"), "name": format!("Box {i}")}))
        .collect();
    let eight = model(json!({"A0": {"boxes": boxes}}));
    assert_eq!(codes(&eight), ["E-NODE-02"]);
    let no_lineage = model(json!({"A-1": {"title": "R", "boxes": [{"key": "x", "name": "X"}]}}));
    assert_eq!(codes(&no_lineage), ["E-NODE-02"]);
    let out_of_range =
        model(json!({"A-1": {"title": "R", "boxes": [{"key": "x", "name": "X"}], "lineage": 1}}));
    assert_eq!(codes(&out_of_range), ["E-NODE-02"]);
    let lineage_on_a0 = model(json!({"A0": {"boxes": [{"key": "x", "name": "X"}], "lineage": 0}}));
    assert_eq!(codes(&lineage_on_a0), ["E-NODE-02"]);
}

#[test]
fn diagrams_need_a_parent_box() {
    let m = model(json!({
        "A0": {"boxes": [{"key": "a", "name": "A"}, {"key": "b", "name": "B"}]},
        "A2": {},
        "A3": {},
        "A41": {},
    }));
    assert_eq!(
        found(&m),
        [
            ("E-NODE-03".into(), "diagram A3".into()),
            ("E-NODE-03".into(), "diagram A41".into())
        ]
    );
    // A0 details the A-0 top box; a missing A-0 is `missing_context`, a rule.
    assert_eq!(codes(&model(json!({"A0": {}}))), Vec::<String>::new());
}

// --- FR-CALL-13 ------------------------------------------------------------------------------

#[test]
fn top_box_cannot_call() {
    let call = json!({"targets": ["@elsewhere"]});
    let on_a_minus_0 = model(
        json!({"A-0": {"title": "T", "boxes": [{"key": "top", "name": "Top", "call": call}]}}),
    );
    assert_eq!(
        found(&on_a_minus_0),
        [("E-CALL-02".into(), "box @top".into())]
    );
    let on_a_minus_1 = model(json!({
        "A-1": {"title": "R", "boxes": [{"key": "x", "name": "X"}, {"key": "top", "name": "Top", "call": call}], "lineage": 1},
        "A-0": {"title": "T", "boxes": [{"key": "top", "name": "Top"}]},
    }));
    assert_eq!(codes(&on_a_minus_1), ["E-CALL-02"]);
    let other_box = a0(json!([]), json!([]));
    let mut m = other_box;
    let a0 = std::sync::Arc::make_mut(m.diagrams.get_mut(&"A0".parse().unwrap()).unwrap());
    a0.boxes[1].call = serde_json::from_value(call).unwrap();
    assert_eq!(codes(&m), Vec::<String>::new());
}

// --- FR-NOTE-03: note targets on the same diagram only --------------------------------------

#[test]
fn note_targets() {
    let arrows = json!([
        {"key": "feed", "src": {"side": "I", "icom": 1}, "dst": {"box": "a", "side": "I", "rank": 1}},
        {"src": {"side": "C", "border": 1}, "dst": {"box": "a", "side": "C", "rank": 1}},
    ]);
    let ok = json!([{"number": 1, "text": "n", "on": [
        {"box": "a"},
        {"arrow_key": "feed"},
        {"arrow": {"src": {"side": "I", "icom": 1}, "dst": {"box": "a", "side": "I", "rank": 1}}},
        {"port": {"side": "C", "border": 1}},
        {"port": {"side": "I", "icom": 1}},
    ]}]);
    assert_eq!(codes(&a0(arrows.clone(), ok)), Vec::<String>::new());
    let foreign = [
        json!({"box": "top"}),
        json!({"call": "a"}),
        json!({"arrow_key": "nope"}),
        json!({"arrow": {"src": {"side": "I", "icom": 1}, "dst": {"box": "a", "side": "I", "rank": 1}, "occurrence": 1}}),
        json!({"arrow": {"src": {"side": "I", "icom": 2}, "dst": {"box": "a", "side": "I", "rank": 1}}}),
        json!({"port": {"side": "O", "icom": 1}}),
    ];
    for target in foreign {
        let m = a0(
            arrows.clone(),
            json!([{"number": 1, "text": "n", "on": [target.clone()]}]),
        );
        assert_eq!(
            found(&m),
            [("E-NOTE-01".into(), "diagram A0".into())],
            "{target}"
        );
    }
}

// validation §8.1, FR-VAL-06: E findings are errors in layer E, sorted by subject then code.
#[test]
fn findings_are_fatal_and_sorted() {
    let m = model(json!({
        "A-0": {"title": "T", "boxes": [{"key": "a", "name": "Top", "call": {"targets": ["@x"]}}, {"key": "b", "name": "B"}]},
        "A0": {"boxes": [{"key": "a", "name": "A"}], "arrows": [{"src": {"side": "I", "icom": 1}, "dst": {"side": "O", "icom": 1}}]},
    }));
    let all = typecheck(&m);
    assert!(
        all.iter()
            .all(|d: &Diagnostic| d.severity == Severity::Error && d.layer == Layer::E)
    );
    let order: Vec<(String, String)> = found(&m);
    assert_eq!(
        order,
        [
            ("E-NODE-02".into(), "diagram A-0".into()),
            ("E-CALL-02".into(), "box @a".into()),
            ("E-ID-02".into(), "box @a".into()),
            ("E-TYPE-03".into(), "edge A0:#0".into()),
        ]
    );
    let again = typecheck(&m.clone());
    assert_eq!(all, again, "deterministic");
}
