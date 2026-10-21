//! Global routing (roadmap M4.3; layout §5.4, §6; FR-LAYOUT-04, -06, -07, -22, -23).

use std::fmt::Write as _;
use std::path::Path;

use idef0_core::dsl::parse;
use idef0_core::id::NodeId;
use idef0_core::model::Model;
use idef0_core::typecheck::typecheck;
use idef0_layout::metrics::Metrics;
use idef0_layout::ports::Rect;
use idef0_layout::route::{Channel, Class, EdgeKey, Leg, Routing, layout_routes, reroute};
use idef0_layout::text::TextMetric;
use proptest::prelude::*;

fn load(path: &Path) -> Model {
    let p = parse(&std::fs::read_to_string(path).expect("fixture"));
    let found: Vec<String> = p.diagnostics.iter().map(ToString::to_string).collect();
    assert!(found.is_empty(), "{}: {found:#?}", path.display());
    let m = p.model.expect("model");
    let tc: Vec<String> = typecheck(&m).iter().map(ToString::to_string).collect();
    assert!(tc.is_empty(), "{}: {tc:#?}", path.display());
    m
}

fn routes(model: &Model, node: &NodeId) -> (Vec<Rect>, (i32, i32), Routing) {
    let (placed, r) =
        layout_routes(model, node, true, TextMetric::Mono, &Metrics::default()).expect("diagram");
    (placed.placement.boxes, placed.placement.frame, r)
}

fn listing(r: &Routing) -> String {
    r.routes
        .iter()
        .map(|x| {
            let path: Vec<String> = x.path.iter().map(|(a, b)| format!("({a},{b})")).collect();
            format!(
                "{} {:?} params={:?} cost={} path={}",
                x.key,
                x.class,
                x.params,
                x.cost,
                path.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn class_for(stem: &str) -> Class {
    match stem {
        "forward-input" => Class::ForwardInput,
        "forward-control" => Class::ForwardControl,
        "forward-mechanism" => Class::ForwardMechanism,
        "feedback-control" => Class::FeedbackControl,
        "feedback-input" => Class::FeedbackInput,
        "feedback-mechanism" => Class::FeedbackMechanism,
        "self-control" => Class::SelfControl,
        "self-input" => Class::SelfInput,
        "self-mechanism" => Class::SelfMechanism,
        "boundary-straight" => Class::BoundaryStraight,
        "boundary-dog-leg" | "frame-order-dog-leg" => Class::BoundaryDogLeg,
        "output-exit" => Class::OutputExit,
        "call" => Class::Call,
        other => panic!("no class for fixture {other}"),
    }
}

// Roadmap M4.3 acceptance: a snapshot per template class. Each fixture's probe arrow routes
// with its class, unforced.
#[test]
fn template_fixtures() {
    insta::glob!("../../../fixtures/layout/templates", "*.idef0", |path| {
        let stem = path.file_stem().and_then(|s| s.to_str()).expect("name");
        let model = load(path);
        let (_, _, r) = routes(&model, &NodeId::a0());
        let probe = r
            .routes
            .iter()
            .find(|x| {
                matches!(&x.key, EdgeKey::Arrow(id) if id.as_str() == "probe")
                    || matches!(x.key, EdgeKey::Call(_))
            })
            .unwrap_or_else(|| panic!("{stem}: no probe route"));
        assert_eq!(probe.class, class_for(stem), "{stem}");
        assert!(
            r.routes.iter().all(|x| !x.forced),
            "{stem}: forced route\n{}",
            listing(&r)
        );
        insta::assert_snapshot!(listing(&r));
    });
}

// FR-LAYOUT-22: boundary arrows on one frame edge enter in ICOM code order.
#[test]
fn frame_order_by_code() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/layout/templates/frame-order-dog-leg.idef0");
    let (_, _, r) = routes(&load(&path), &NodeId::a0());
    let entry = |pick: &dyn Fn(&str) -> bool| -> i32 {
        r.routes
            .iter()
            .find(|x| pick(&x.key.to_string()))
            .map(|x| x.path[0].1)
            .expect("route")
    };
    // the probe is I1 -> 3I1 (key=probe); the other arrow is I2 -> 1I1
    let (i1, i2) = (entry(&|k| k == "probe"), entry(&|k| k.starts_with("A0:I2")));
    assert!(i1 < i2, "I1 enters above I2: {i1} vs {i2}");
}

// The dogfood A0: FIPS feedback shapes on the arrows EXPECTED.md lists.
#[test]
fn dogfood_classes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dogfood/idef0-rs.idef0");
    let (_, _, r) = routes(&load(&path), &NodeId::a0());
    let class = |id: &str| {
        r.routes
            .iter()
            .find(|x| x.key.to_string() == id)
            .map(|x| x.class)
    };
    assert_eq!(
        class("A0:3O1->2C1#0"),
        Some(Class::FeedbackControl),
        "diagnostics: up and over"
    );
    assert_eq!(
        class("A0:4O1->2C2#0"),
        Some(Class::FeedbackControl),
        "diagram scenes: up and over"
    );
    assert_eq!(
        class("A0:5O1->3I2#0"),
        Some(Class::FeedbackInput),
        "code facts: down and under"
    );
    assert_eq!(class("A0:1O1->2I1#0"), Some(Class::ForwardInput));
    assert_eq!(class("A0:I1->1I1#0"), Some(Class::BoundaryStraight));
    assert_eq!(class("A0:3O2->O1#0"), Some(Class::OutputExit));
    assert!(r.routes.iter().all(|x| !x.forced));
    assert_eq!(
        r.routes.len(),
        20,
        "14 arrow statements, 20 edges after bundle flattening"
    );
}

// §5.2: iterations k >= 2 keep the channel assignment: reroute takes the frozen parameters,
// even where a free choice would differ.
#[test]
fn reroute_keeps_frozen_parameters() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dogfood/idef0-rs.idef0");
    let model = load(&path);
    let node = NodeId::a0();
    let m = Metrics::default();
    let (placed, r) = layout_routes(&model, &node, true, TextMetric::Mono, &m).expect("A0");
    let diagram = &model.diagrams[&node];
    let counts = idef0_layout::place::port_counts(diagram);
    let id = "A0:3O1->2C1#0";
    let free = r
        .routes
        .iter()
        .find(|x| x.key.to_string() == id)
        .expect("route");
    assert_eq!(free.class, Class::FeedbackControl);
    let other = usize::from(free.params == [0]);
    let mut frozen = r.clone();
    for x in &mut frozen.routes {
        if x.key.to_string() == id {
            x.params = vec![other];
        }
    }
    let again = reroute(&frozen, &node, diagram, &placed.placement, &counts, &m);
    let kept = again
        .routes
        .iter()
        .find(|x| x.key.to_string() == id)
        .expect("route");
    assert_eq!(kept.params, vec![other]);
    assert_ne!(kept.path, free.path);
    let same = reroute(&r, &node, diagram, &placed.placement, &counts, &m);
    assert_eq!(
        listing(&same),
        listing(&r),
        "unchanged parameters, unchanged routes"
    );
}

// Channel density: the most runs that share a point of one channel.
#[test]
fn density_counts_overlapping_runs() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dogfood/idef0-rs.idef0");
    let (_, _, mut r) = routes(&load(&path), &NodeId::a0());
    let n = 5;
    let spans = [
        (Channel::V(1), (0, 40)),
        (Channel::V(1), (40, 20)),
        (Channel::V(1), (30, 60)),
        (Channel::V(1), (70, 80)),
        (Channel::H(2), (10, 20)),
        (Channel::H(2), (30, 40)),
    ];
    r.routes.truncate(spans.len());
    for (x, (channel, span)) in r.routes.iter_mut().zip(spans) {
        x.legs = vec![Leg { channel, span }];
    }
    let t = r.density(n);
    assert_eq!(t.v, vec![0, 3, 0, 0, 0, 0]);
    assert_eq!(t.h, vec![0, 0, 1, 0, 0, 0]);
}

/// A random, type-correct A0 with `n` boxes: box-to-box arrows of every role, boundary
/// inputs, controls and mechanisms to every role (`side % 3` is the frame side, `side / 3`
/// the role shift), outputs to the boundary, and calls. Ranks are left to the parser.
fn generated(
    n: usize,
    arrows: &[(usize, usize, u8)],
    boundary: &[(u8, usize)],
    outs: &[usize],
    calls: &[usize],
) -> String {
    let mut s =
        String::from("idef0 0.2\nmodel \"Generated\" {\n  abbrev \"GEN\"\n}\ndiagram A0 {\n");
    for i in 1..=n {
        writeln!(s, "  box {i} \"Box {i}\"").expect("write to a String");
    }
    let role = |r: u8| ["I", "C", "M"][usize::from(r % 3)];
    for &(a, b, r) in arrows {
        writeln!(s, "  arrow : {}O -> {}{}", a % n + 1, b % n + 1, role(r))
            .expect("write to a String");
    }
    let mut codes = [0u32; 3];
    for &(side, b) in boundary {
        let k = usize::from(side % 3);
        codes[k] += 1;
        writeln!(
            s,
            "  arrow : {}{} -> {}{}",
            role(side),
            codes[k],
            b % n + 1,
            role(side % 3 + side / 3)
        )
        .expect("write to a String");
    }
    for (k, &a) in outs.iter().enumerate() {
        writeln!(s, "  arrow : {}O -> O{}", a % n + 1, k + 1).expect("write to a String");
    }
    let mut seen = Vec::new();
    for &a in calls {
        let b = a % n + 1;
        if !seen.contains(&b) {
            seen.push(b);
            writeln!(s, "  call {b} -> @elsewhere").expect("write to a String");
        }
    }
    s.push_str("}\n");
    s
}

fn passes_inside(a: (i32, i32), b: (i32, i32), r: Rect) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    if y0 == y1 {
        r.y < y0 && y0 < r.y + r.h && x0 < r.x + r.w && r.x < x1
    } else {
        r.x < x0 && x0 < r.x + r.w && y0 < r.y + r.h && r.y < y1
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    // Roadmap M4.3 acceptance: every route is orthogonal, on the lattice, attached to its
    // ports, and never enters a box; routing is deterministic.
    #[test]
    fn routes_are_orthogonal_and_avoid_boxes(
        n in 1usize..8,
        arrows in proptest::collection::vec((0usize..7, 0usize..7, 0u8..3), 0..18),
        boundary in proptest::collection::vec((0u8..9, 0usize..7), 0..10),
        outs in proptest::collection::vec(0usize..7, 0..4),
        calls in proptest::collection::vec(0usize..7, 0..3),
    ) {
        let src = generated(n, &arrows, &boundary, &outs, &calls);
        let p = parse(&src);
        prop_assert!(p.diagnostics.is_empty(), "{src}\n{:?}", p.diagnostics);
        let model = p.model.expect("model");
        prop_assert!(typecheck(&model).is_empty(), "{src}");
        let (boxes, frame, r) = routes(&model, &NodeId::a0());
        let expected = arrows.len() + boundary.len() + outs.len() + calls.iter().map(|a| a % n).collect::<std::collections::BTreeSet<_>>().len();
        prop_assert_eq!(r.routes.len(), expected, "{}", src);
        for route in &r.routes {
            prop_assert!(!route.forced, "{} forced\n{}\n{}", route.key, src, listing(&r));
            prop_assert!(route.path.len() >= 2);
            for w in route.path.windows(2) {
                prop_assert!(w[0].0 == w[1].0 || w[0].1 == w[1].1, "{} not orthogonal: {:?}", route.key, route.path);
                for b in &boxes {
                    prop_assert!(!passes_inside(w[0], w[1], *b), "{} enters a box: {:?}\n{}", route.key, route.path, src);
                }
            }
            for &(x, y) in &route.path {
                prop_assert!(x % 2 == 0 && y % 4 == 0, "{} off the lattice: {:?}", route.key, route.path);
                prop_assert!((0..=frame.0).contains(&x) && (0..=frame.1).contains(&y), "{} outside the frame", route.key);
            }
        }
        let (_, _, again) = routes(&model, &NodeId::a0());
        prop_assert_eq!(listing(&again), listing(&r));
    }
}

// FR-LAYOUT-22: a crowded frame edge. Entries keep code order, sit at least one track pitch
// apart and stay inside the frame, even when their natural coordinates all coincide.
#[test]
fn crowded_frame_edge() {
    let mut s = String::from(
        "idef0 0.2\nmodel \"G\" {\n  abbrev \"GEN\"\n}\ndiagram A0 {\n  box 1 \"Box 1\"\n",
    );
    for k in 1..=9 {
        writeln!(s, "  arrow : I{k} -> 1M").expect("write to a String");
    }
    for k in 1..=6 {
        writeln!(s, "  arrow : M{k} -> 1C").expect("write to a String");
    }
    s.push_str("}\n");
    let p = parse(&s);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let model = p.model.expect("model");
    let (_, frame, r) = routes(&model, &NodeId::a0());
    let m = Metrics::default();
    let entries = |side: &str, pick: fn((i32, i32)) -> i32| -> Vec<i32> {
        r.routes
            .iter()
            .filter(|x| x.key.to_string().starts_with(&format!("A0:{side}")))
            .map(|x| pick(x.path[0]))
            .collect()
    };
    for (side, ys, pitch, extent) in [
        ("I", entries("I", |p| p.1), m.pitch_y, frame.1),
        ("M", entries("M", |p| p.0), m.pitch_x, frame.0),
    ] {
        assert!(ys.len() > 5, "{side}: {ys:?}");
        assert!(
            ys.windows(2).all(|w| w[1] - w[0] >= pitch),
            "{side} entries in code order, a pitch apart: {ys:?}\n{}",
            listing(&r)
        );
        assert!(
            ys.iter().all(|&y| m.clear <= y && y <= extent - m.clear),
            "{side} entries inside the frame: {ys:?} (extent {extent})"
        );
    }
    assert!(r.routes.iter().all(|x| !x.forced), "{}", listing(&r));
}
