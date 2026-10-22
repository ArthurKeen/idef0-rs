use super::history::{Entry, History, Selected};
use super::*;
use crate::identity::SlugMinter;
use crate::typecheck::typecheck;

/// Five boxes on A0; box 3 is decomposed into A3, whose box 2 is decomposed into A32.
const FIVE: &str = r#"idef0 0.2
model "Five" {
  abbrev "FIV"
}
diagram A-0 "Run Five" {
  box 0 "Run Five" { description "Detailed by A0; see A3 and A3.2." }
  arrow : I1 -> 0I1
  arrow : 0O1 -> O1
}
diagram A0 {
  box 1 "Step One"
  box 2 "Step Two"
  box 3 "Step Three"
  box 4 "Step Four"
  box 5 "Step Five"
  arrow : I1 -> 1I1
  arrow : 1O1 -> 2I1
  arrow : 2O1 -> 3I1
  arrow : 1O2 -> 3I2
  arrow : 3O1 -> 4I1
  arrow : 4O1 -> 5I1
  arrow : 5O1 -> O1
}
diagram A3 {
  box 1 "Sub One"
  box 2 "Sub Two"
  box 3 "Sub Three"
  arrow : I1 -> 1I1
  arrow : I2 -> 2I1
  arrow : 1O1 -> 3I1
  arrow : 2O1 -> 3I2
  arrow : 3O1 -> O1
}
diagram A32 {
  box 1 "Deep One"
  box 2 "Deep Two"
  box 3 "Deep Three"
  arrow : I1 -> 1I1
  arrow : 1O1 -> 2I1
  arrow : 2O1 -> 3I1
  arrow : 3O1 -> O1
}
"#;

fn model(src: &str) -> Model {
    let p = crate::dsl::parse(src);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let m = p.model.expect("model");
    assert!(typecheck(&m).is_empty(), "{:?}", typecheck(&m));
    m
}

fn five() -> Model {
    model(FIVE)
}

fn n(s: &str) -> NodeId {
    s.parse().expect("node")
}

fn k(s: &str) -> BoxKey {
    s.parse().expect("key")
}

fn name(s: &str) -> NonEmptyString {
    NonEmptyString::new(s).expect("name")
}

#[allow(clippy::needless_pass_by_value, reason = "tests build commands inline")]
fn run(m: &Model, c: Command) -> (Model, Patch) {
    let mut minter = SlugMinter;
    let mut env = Env {
        minter: &mut minter,
    };
    apply(m, &c, &mut env).unwrap_or_else(|e| panic!("{c:?}: {e}"))
}

#[allow(clippy::needless_pass_by_value, reason = "tests build commands inline")]
fn try_run(m: &Model, c: Command) -> Result<(Model, Patch), CommandError> {
    let mut minter = SlugMinter;
    let mut env = Env {
        minter: &mut minter,
    };
    apply(m, &c, &mut env)
}

/// The arrows of diagram `node` in DSL endpoint form.
fn arrows(m: &Model, node: &str) -> Vec<String> {
    let node = n(node);
    let d = &m.diagrams[&node];
    arrow_ids(&node, d)
        .iter()
        .map(|i| {
            i.trunk
                .as_str()
                .split_once(':')
                .map_or(i.trunk.to_string(), |(_, r)| r.to_owned())
        })
        .collect()
}

fn names(m: &Model, node: &str) -> Vec<String> {
    m.diagrams[&n(node)]
        .boxes
        .iter()
        .map(|b| b.name.as_str().to_owned())
        .collect()
}

/// Undo restores the input, and the forward ops give the result.
fn check_inverse(before: &Model, after: &Model, p: &Patch) {
    assert_eq!(&apply_ops(after, &p.inverse), before, "undo ∘ do = id");
    assert_eq!(&apply_ops(before, &p.ops), after, "ops replay the change");
}

// Roadmap M1.5 acceptance: deleting box 2 of 5 removes its arrows, compacts the ranks they
// held, renumbers boxes 3–5 with their subtrees, turns a vanished child code into a border
// tunnel (FR-EDIT-08), and records every rename (FR-EDIT-02, -04, -05).
#[test]
fn delete_box_two_of_five() {
    let m = five();
    let (after, p) = run(&m, Command::DeleteBox { key: k("step-two") });
    check_inverse(&m, &after, &p);
    assert_eq!(
        names(&after, "A0"),
        ["Step One", "Step Three", "Step Four", "Step Five"]
    );
    assert_eq!(
        arrows(&after, "A0"),
        [
            "I1->1I1#0",
            "1O1->2I1#0",
            "2O1->3I1#0",
            "3O1->4I1#0",
            "4O1->O1#0"
        ]
    );
    let nodes: Vec<String> = after.diagrams.keys().map(ToString::to_string).collect();
    assert_eq!(nodes, ["A-0", "A0", "A2", "A22"]);
    // box 3's I1 came from the deleted box: its child code becomes a border tunnel, I2 → I1
    assert_eq!(
        arrows(&after, "A2")[..2],
        ["[I1]->1I1#0".to_owned(), "I1->2I1#0".to_owned()]
    );
    let r = &p.rename;
    assert_eq!(r.boxes.get(&k("step-two")), Some(&None));
    assert!(after.meta.retired.contains(&k("step-two")), "D17");
    assert_eq!(r.nodes.get(&n("A2")), Some(&None));
    for (a, b) in [
        ("A3", "A2"),
        ("A4", "A3"),
        ("A5", "A4"),
        ("A31", "A21"),
        ("A32", "A22"),
        ("A323", "A223"),
    ] {
        assert_eq!(r.nodes.get(&n(a)), Some(&Some(n(b))), "{a}");
    }
    let port = |s: &str| s.parse::<PortRef>().expect("port");
    assert_eq!(r.ports.get(&port("A1/O2")), Some(&Some(port("A1/O1"))));
    assert_eq!(r.ports.get(&port("A3/I2")), Some(&Some(port("A2/I1"))));
    assert_eq!(r.ports.get(&port("A3/O1")), Some(&Some(port("A2/O1"))));
    assert_eq!(
        r.ports.get(&port("A3/I1")),
        Some(&Some(port("A2/[I1]"))),
        "the border tunnel"
    );
    let id = |s: &str| r.arrows.get(&ArrowId::raw(s.into())).cloned();
    assert_eq!(id("A0:2O1->3I1#0"), Some(None));
    assert_eq!(
        id("A0:1O2->3I2#0"),
        Some(Some(ArrowId::raw("A0:1O1->2I1#0".into())))
    );
    assert_eq!(
        id("A3:I2->2I1#0"),
        Some(Some(ArrowId::raw("A2:I1->2I1#0".into())))
    );
    // FR-EDIT-20: free-text node references follow, in their written style
    let desc = &after.diagrams[&n("A-0")].boxes[0].description.text;
    assert_eq!(desc, "Detailed by A0; see A2 and A2.2.");
}

#[test]
fn top_box_and_context_boxes_stay() {
    let m = five();
    assert!(matches!(
        try_run(&m, Command::DeleteBox { key: k("run-five") }),
        Err(CommandError::Rejected(_))
    ));
    assert!(matches!(
        try_run(
            &m,
            Command::AddBox {
                diagram: n("A-0"),
                at: 1,
                name: name("X"),
                description: String::new()
            }
        ),
        Err(CommandError::Rejected(_))
    ));
}

// FR-EDIT-03: insertion shifts the later boxes with their subtrees; the profile maximum holds.
#[test]
fn add_box_shifts_subtrees() {
    let m = five();
    let (after, p) = run(
        &m,
        Command::AddBox {
            diagram: n("A0"),
            at: 3,
            name: name("Step Two Half"),
            description: "new".into(),
        },
    );
    check_inverse(&m, &after, &p);
    assert_eq!(names(&after, "A0")[2], "Step Two Half");
    assert_eq!(after.diagrams[&n("A0")].boxes[2].key, k("step-two-half"));
    assert!(after.diagrams.contains_key(&n("A4")) && after.diagrams.contains_key(&n("A42")));
    assert_eq!(p.rename.nodes.get(&n("A3")), Some(&Some(n("A4"))));
    assert_eq!(arrows(&after, "A0")[2], "2O1->4I1#0");
    let (six, _) = run(
        &after,
        Command::AddBox {
            diagram: n("A0"),
            at: 7,
            name: name("Seven"),
            description: String::new(),
        },
    );
    let full = try_run(
        &six,
        Command::AddBox {
            diagram: n("A0"),
            at: 1,
            name: name("Eight"),
            description: String::new(),
        },
    );
    assert!(
        matches!(full, Err(CommandError::Rejected(_))),
        "7 is the extended maximum"
    );
}

// FR-EDIT-06: a move permutes positions and renumbers; keys and ports stay.
#[test]
fn move_box_renumbers() {
    let m = five();
    let (after, p) = run(
        &m,
        Command::MoveBox {
            key: k("step-three"),
            to: 1,
        },
    );
    check_inverse(&m, &after, &p);
    assert_eq!(
        names(&after, "A0"),
        [
            "Step Three",
            "Step One",
            "Step Two",
            "Step Four",
            "Step Five"
        ]
    );
    assert!(after.diagrams.contains_key(&n("A1")) && after.diagrams.contains_key(&n("A12")));
    assert_eq!(p.rename.nodes.get(&n("A3")), Some(&Some(n("A1"))));
    assert_eq!(p.rename.nodes.get(&n("A1")), Some(&Some(n("A2"))));
    assert_eq!(
        arrows(&after, "A0")[1],
        "2O1->3I1#0",
        "box one is now box 2"
    );
    assert!(typecheck(&after).is_empty());
}

// FR-EDIT-07: one ReorderPorts re-codes the child boundary with the same permutation.
#[test]
fn reorder_ports_recodes_the_child() {
    let m = five();
    let (after, p) = run(
        &m,
        Command::ReorderPorts {
            key: k("step-three"),
            side: Side::I,
            order: vec![2, 1],
        },
    );
    check_inverse(&m, &after, &p);
    assert_eq!(
        arrows(&after, "A0")[2..4],
        ["2O1->3I2#0".to_owned(), "1O2->3I1#0".to_owned()]
    );
    assert_eq!(
        arrows(&after, "A3")[..2],
        ["I2->1I1#0".to_owned(), "I1->2I1#0".to_owned()]
    );
    let port = |s: &str| s.parse::<PortRef>().expect("port");
    assert_eq!(
        p.rename.ports.get(&port("A3/I1")),
        Some(&Some(port("A3/I2")))
    );
    assert_eq!(
        p.rename.ports.get(&port("A3/I2")),
        Some(&Some(port("A3/I1")))
    );
    assert!(matches!(
        try_run(
            &m,
            Command::ReorderPorts {
                key: k("step-three"),
                side: Side::I,
                order: vec![1]
            }
        ),
        Err(CommandError::Rejected(_))
    ));
}

// FR-EDIT-19: a retargeted end takes the last rank on its new side; the old side compacts.
#[test]
fn retarget_takes_the_last_rank() {
    let m = five();
    let id = ArrowId::raw("A0:1O2->3I2#0".into());
    let (after, p) = run(
        &m,
        Command::Retarget {
            diagram: n("A0"),
            id,
            end: End::Dst,
            to: EndSpec::NewPort {
                key: k("step-four"),
                side: Side::I,
            },
        },
    );
    check_inverse(&m, &after, &p);
    assert!(
        arrows(&after, "A0").contains(&"1O2->4I2#0".to_owned()),
        "{:?}",
        arrows(&after, "A0")
    );
    // box 3 keeps I1 only; its child code I2 lost its parent port
    assert!(
        arrows(&after, "A3").contains(&"[I1]->2I1#0".to_owned()),
        "{:?}",
        arrows(&after, "A3")
    );
}

// DeleteArrow compacts the side that lost the end and re-codes the child (FR-EDIT-07, -08).
#[test]
fn delete_arrow_compacts() {
    let m = five();
    let (after, p) = run(
        &m,
        Command::DeleteArrow {
            diagram: n("A0"),
            id: ArrowId::raw("A0:2O1->3I1#0".into()),
        },
    );
    check_inverse(&m, &after, &p);
    assert!(arrows(&after, "A0").contains(&"1O2->3I1#0".to_owned()));
    assert_eq!(
        arrows(&after, "A3")[..2],
        ["[I1]->1I1#0".to_owned(), "I1->2I1#0".to_owned()]
    );
    assert!(typecheck(&after).is_empty());
}

// A command that would break the type check is rejected, not applied.
#[test]
fn invalid_results_are_rejected() {
    let m = five();
    let bad = Command::AddArrow {
        diagram: n("A0"),
        src: EndSpec::NewPort {
            key: k("step-one"),
            side: Side::I,
        },
        dst: EndSpec::NewPort {
            key: k("step-two"),
            side: Side::I,
        },
        concept: None,
    };
    assert!(matches!(try_run(&m, bad), Err(CommandError::Invalid(_))));
    let good = Command::AddArrow {
        diagram: n("A0"),
        src: EndSpec::NewPort {
            key: k("step-five"),
            side: Side::O,
        },
        dst: EndSpec::NewPort {
            key: k("step-one"),
            side: Side::C,
        },
        concept: None,
    };
    let (after, p) = run(&m, good);
    check_inverse(&m, &after, &p);
    assert!(arrows(&after, "A0").contains(&"5O2->1C1#0".to_owned()));
}

// FR-EDIT-10: deleting a child diagram removes its descendants; the parent box keeps its
// ports, and the parent diagram's DRE change is minor.
#[test]
fn delete_diagram() {
    let mut m = five();
    Arc::make_mut(m.diagrams.get_mut(&n("A0")).expect("A0")).status = Status::Recommended;
    let (after, p) = run(&m, Command::DeleteDiagram { diagram: n("A3") });
    check_inverse(&m, &after, &p);
    assert!(!after.diagrams.contains_key(&n("A3")) && !after.diagrams.contains_key(&n("A32")));
    assert_eq!(
        arrows(&after, "A0"),
        arrows(&m, "A0"),
        "the parent keeps its ports"
    );
    assert_eq!(
        after.diagrams[&n("A0")].status,
        Status::Draft,
        "minor: recommended → draft"
    );
    assert!(after.meta.retired.contains(&k("deep-two")));
}

// FR-EDIT-15: major changes reset draft and above to working; minor changes reset
// recommended and publication to draft; an explicit SetStatus is kept.
#[test]
fn status_rules() {
    let mut m = five();
    Arc::make_mut(m.diagrams.get_mut(&n("A0")).expect("A0")).status = Status::Draft;
    let (after, _) = run(
        &m,
        Command::RenameBox {
            key: k("step-one"),
            name: name("Begin"),
        },
    );
    assert_eq!(after.diagrams[&n("A0")].status, Status::Working);
    let (minor, _) = run(
        &m,
        Command::SetDescription {
            subject: Described::Box(k("step-one")),
            text: "x".into(),
        },
    );
    assert_eq!(
        minor.diagrams[&n("A0")].status,
        Status::Draft,
        "minor at draft stays"
    );
    let (set, _) = run(
        &m,
        Command::SetStatus {
            diagram: n("A0"),
            status: Status::Publication,
        },
    );
    assert_eq!(set.diagrams[&n("A0")].status, Status::Publication);
    let (pubminor, _) = run(
        &set,
        Command::SetDescription {
            subject: Described::Diagram(n("A0")),
            text: "y".into(),
        },
    );
    assert_eq!(pubminor.diagrams[&n("A0")].status, Status::Draft);
    // a re-coded child diagram is a major change there
    Arc::make_mut(m.diagrams.get_mut(&n("A3")).expect("A3")).status = Status::Draft;
    let (re, _) = run(
        &m,
        Command::ReorderPorts {
            key: k("step-three"),
            side: Side::I,
            order: vec![2, 1],
        },
    );
    assert_eq!(re.diagrams[&n("A3")].status, Status::Working);
}

// FR-EDIT-11: a rename keeps the key; a stated child title follows.
#[test]
fn rename_box() {
    let src = FIVE.replace("diagram A3 {", "diagram A3 \"Step Three\" {");
    let m = model(&src);
    let (after, p) = run(
        &m,
        Command::RenameBox {
            key: k("step-three"),
            name: name("Third Step"),
        },
    );
    check_inverse(&m, &after, &p);
    let b = &after.diagrams[&n("A0")].boxes[2];
    assert_eq!(
        (b.key.as_str(), b.name.as_str()),
        ("step-three", "Third Step")
    );
    assert_eq!(
        after.diagrams[&n("A3")]
            .title
            .as_ref()
            .map(NonEmptyString::as_str),
        Some("Third Step")
    );
    assert!(p.rename.nodes.is_empty() && p.rename.arrows.is_empty());
}

// FR-EDIT-13: paste inserts a copy with fresh keys; a subtree comes with its internal
// arrows rewritten to the new keys.
#[test]
fn copy_and_paste() {
    let m = five();
    let clip = copy(&m, &k("step-three"), true).expect("clip");
    assert_eq!(clip.subtree.len(), 2, "A3 and A32");
    let (after, p) = run(
        &m,
        Command::Paste {
            diagram: n("A0"),
            at: 6,
            clip,
        },
    );
    check_inverse(&m, &after, &p);
    let pasted = &after.diagrams[&n("A0")].boxes[5];
    assert_eq!(
        (pasted.key.as_str(), pasted.name.as_str()),
        ("step-three-2", "Step Three")
    );
    let a6 = &after.diagrams[&n("A6")];
    assert_eq!(a6.boxes[0].key, k("sub-one-2"));
    assert!(after.diagrams.contains_key(&n("A62")));
    assert_eq!(
        arrows(&after, "A6"),
        arrows(&m, "A3"),
        "same structure, fresh keys"
    );
    let flat = copy(&m, &k("step-three"), false).expect("clip");
    assert!(flat.subtree.is_empty());
    assert!(
        try_run(
            &after,
            Command::Paste {
                diagram: n("A0"),
                at: 1,
                clip: flat
            }
        )
        .is_ok()
    );
}

// §13.1 step 7: calls into removed boxes lose those targets; a call with none left goes.
#[test]
fn delete_drops_calls_into_the_subtree() {
    let src = FIVE.replace(
        "  box 5 \"Step Five\"\n",
        "  box 5 \"Step Five\"\n  call 5 -> @deep-two\n",
    );
    let m = model(&src);
    assert!(m.diagrams[&n("A0")].boxes[4].call.is_some());
    let (after, _) = run(
        &m,
        Command::DeleteBox {
            key: k("step-three"),
        },
    );
    assert!(
        after.diagrams[&n("A0")].boxes[3].call.is_none(),
        "box 5 is box 4 now, no longer a caller"
    );
}

// §13.1 step 6: attachments to removed elements go; an arrow attachment follows its arrow.
#[test]
fn notes_follow_or_drop() {
    let src = FIVE.replace(
        "  arrow : 5O1 -> O1\n}",
        "  arrow : 5O1 -> O1\n  note 1 \"Watch these.\" on box 2, 1O2 -> 3I2\n}",
    );
    let m = model(&src);
    let (after, _) = run(&m, Command::DeleteBox { key: k("step-two") });
    let note = &after.diagrams[&n("A0")].notes[0];
    assert_eq!(note.on.len(), 1, "{:?}", note.on);
    let NoteTarget::Arrow { src, dst, .. } = &note.on[0] else {
        panic!("{:?}", note.on);
    };
    assert_eq!(
        (src, dst),
        (
            &Endpoint::Box {
                key: k("step-one"),
                side: Side::O,
                rank: 1
            },
            &Endpoint::Box {
                key: k("step-three"),
                side: Side::I,
                rank: 1
            }
        )
    );
}

// FR-EDIT-17: one stack across diagrams; undo navigates to the change (or its nearest
// surviving ancestor) and restores the selection, repaired; redo restores the later one.
#[test]
fn history() {
    let m0 = five();
    let mut h = History::with_depth(2);
    let (m1, p1) = run(
        &m0,
        Command::RenameBox {
            key: k("deep-one"),
            name: name("Deep Start"),
        },
    );
    h.push(Entry {
        patch: p1,
        before: vec![Selected::Box(k("deep-one"))],
        after: vec![],
    });
    let (m2, p2) = run(
        &m1,
        Command::DeleteBox {
            key: k("step-three"),
        },
    );
    h.push(Entry {
        patch: p2,
        before: vec![Selected::Box(k("step-three")), Selected::Diagram(n("A32"))],
        after: vec![Selected::Box(k("step-three"))],
    });
    let back = h.undo(&m2).expect("undo");
    assert_eq!(back.model, m1);
    assert_eq!(back.diagram, n("A0"));
    assert_eq!(back.selection.len(), 2);
    let again = h.redo(&back.model).expect("redo");
    assert_eq!(again.model, m2);
    assert!(
        again.selection.is_empty(),
        "the deleted box is not selectable"
    );
    let b1 = h.undo(&m2).expect("undo");
    let b0 = h.undo(&b1.model).expect("undo");
    assert_eq!(b0.model, m0);
    assert_eq!(b0.diagram, n("A32"), "the rename happened on A32");
    assert!(h.undo(&m0).is_none());
    // depth 2: a third entry drops the oldest
    let (m3, p3) = run(
        &m2,
        Command::RenameBox {
            key: k("step-one"),
            name: name("One"),
        },
    );
    let mut h2 = History::with_depth(2);
    for p in [p3.clone(), p3.clone(), p3] {
        h2.push(Entry {
            patch: p,
            before: vec![],
            after: vec![],
        });
    }
    assert_eq!(h2.undo_len(), 2);
    let _ = m3;
}

#[test]
fn compound_is_one_patch() {
    let m = five();
    let mut minter = SlugMinter;
    let mut env = Env {
        minter: &mut minter,
    };
    let cmds = [
        Command::DeleteBox { key: k("step-two") },
        Command::DeleteBox {
            key: k("step-four"),
        },
    ];
    let (after, p) = apply_all(&m, &cmds, &mut env).expect("apply");
    check_inverse(&m, &after, &p);
    assert_eq!(names(&after, "A0"), ["Step One", "Step Three", "Step Five"]);
    // composed renames map each original id to its final one
    assert_eq!(p.rename.nodes.get(&n("A5")), Some(&Some(n("A3"))));
    assert_eq!(p.rename.nodes.get(&n("A3")), Some(&Some(n("A2"))));
}

// §13.1 step 2: a bundle left with one branch collapses into a plain arrow; its ids map to
// the plain arrow's, and the removed branch's to nothing.
#[test]
fn bundle_collapses_to_plain() {
    let src = FIVE.replace(
        "  arrow : 4O1 -> 5I1\n",
        "  arrow : 4O1 -> fork {\n    -> 5I1\n    -> 5C1\n  }\n",
    );
    let m = model(&src);
    let (after, p) = run(
        &m,
        Command::DeleteArrow {
            diagram: n("A0"),
            id: ArrowId::raw("A0:4O1->*#0/1".into()),
        },
    );
    check_inverse(&m, &after, &p);
    assert!(
        arrows(&after, "A0").contains(&"4O1->5I1#0".to_owned()),
        "{:?}",
        arrows(&after, "A0")
    );
    let id = |s: &str| p.rename.arrows.get(&ArrowId::raw(s.into())).cloned();
    assert_eq!(
        id("A0:4O1->*#0"),
        Some(Some(ArrowId::raw("A0:4O1->5I1#0".into())))
    );
    assert_eq!(
        id("A0:4O1->*#0/0"),
        Some(Some(ArrowId::raw("A0:4O1->5I1#0".into())))
    );
    assert_eq!(id("A0:4O1->*#0/1"), Some(None));
}

// A new change clears redo; redo of a deletion navigates to the nearest surviving ancestor.
#[test]
fn redo_clears_and_navigates() {
    let m0 = five();
    let mut h = History::default();
    let (m1, p1) = run(&m0, Command::DeleteDiagram { diagram: n("A32") });
    h.push(Entry {
        patch: p1,
        before: vec![],
        after: vec![],
    });
    let back = h.undo(&m1).expect("undo");
    assert_eq!(back.diagram, n("A32"));
    let again = h.redo(&back.model).expect("redo");
    assert_eq!(again.diagram, n("A3"), "A32 is gone; its parent remains");
    let back = h.undo(&again.model).expect("undo");
    assert_eq!(h.redo_len(), 1);
    let (m2, p2) = run(
        &back.model,
        Command::RenameBox {
            key: k("step-one"),
            name: name("One"),
        },
    );
    h.push(Entry {
        patch: p2,
        before: vec![],
        after: vec![],
    });
    assert_eq!(h.redo_len(), 0, "a new change clears redo");
    let _ = m2;
}
