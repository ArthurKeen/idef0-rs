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
    let x =
        layout_routes(model, node, true, TextMetric::Mono, &Metrics::default()).expect("diagram");
    (
        x.placed.placement.boxes,
        x.placed.placement.frame,
        x.routing,
    )
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
        let x = layout_routes(
            &model,
            &NodeId::a0(),
            true,
            TextMetric::Mono,
            &Metrics::default(),
        )
        .expect("A0");
        let r = &x.routing;
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
            listing(r)
        );
        // M4.4: the wired polyline under each route, and the tracks each channel uses
        let wired: Vec<String> = r
            .routes
            .iter()
            .zip(&x.wiring.paths)
            .map(|(route, p)| {
                let p: Vec<String> = p.iter().map(|(a, b)| format!("({a},{b})")).collect();
                format!("{} wired={}", route.key, p.join(" "))
            })
            .collect();
        let used: Vec<String> = x
            .wiring
            .channels
            .iter()
            .filter(|c| c.tracks > 0)
            .map(|c| format!("{:?}={}", c.channel, c.tracks))
            .collect();
        insta::assert_snapshot!(format!(
            "{}\n{}\ntracks {} crossings {}",
            listing(r),
            wired.join("\n"),
            used.join(" "),
            x.wiring.crossings
        ));
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
    let x = layout_routes(&model, &node, true, TextMetric::Mono, &m).expect("A0");
    let (placed, r) = (x.placed, x.routing);
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
        x.legs = vec![Leg {
            channel,
            span,
            seg: 0,
        }];
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

fn collinear_overlap(a: ((i32, i32), (i32, i32)), b: ((i32, i32), (i32, i32))) -> bool {
    let h = |s: ((i32, i32), (i32, i32))| s.0.1 == s.1.1;
    let span = |p: i32, q: i32| (p.min(q), p.max(q));
    if h(a) && h(b) && a.0.1 == b.0.1 {
        let (a0, a1) = span(a.0.0, a.1.0);
        let (b0, b1) = span(b.0.0, b.1.0);
        a0.max(b0) < a1.min(b1)
    } else if !h(a) && !h(b) && a.0.0 == b.0.0 {
        let (a0, a1) = span(a.0.1, a.1.1);
        let (b0, b1) = span(b.0.1, b.1.1);
        a0.max(b0) < a1.min(b1)
    } else {
        false
    }
}

/// Every M4.4 property of one diagram's wiring, as a list of violations.
#[allow(
    clippy::many_single_char_names,
    reason = "geometry names: a, b, k, m, p, r, s, w"
)]
fn wiring_violations(model: &Model) -> Vec<String> {
    let m = Metrics::default();
    let x = layout_routes(model, &NodeId::a0(), true, TextMetric::Mono, &m).expect("A0");
    let (p, r, w) = (&x.placed.placement, &x.routing, &x.wiring);
    let mut bad = Vec::new();
    for (i, (route, path)) in r.routes.iter().zip(&w.paths).enumerate() {
        let k = &route.key;
        if path.first() != route.path.first() || path.last() != route.path.last() {
            bad.push(format!("{k}: ends moved {path:?}"));
        }
        let last = path.len().saturating_sub(2);
        for (s, seg) in path.windows(2).enumerate() {
            let (a, b) = (seg[0], seg[1]);
            if a.0 != b.0 && a.1 != b.1 {
                bad.push(format!("{k}: not orthogonal {path:?}"));
            }
            let len = (a.0 - b.0).abs() + (a.1 - b.1).abs();
            let min = if s == 0 || s == last {
                m.clear
            } else {
                2 * m.r_fillet / 4
            };
            if len < min {
                bad.push(format!("{k}: segment {s} is {len} gu {path:?}"));
            }
            for bx in &p.boxes {
                if passes_inside(a, b, *bx) {
                    bad.push(format!("{k}: enters a box {path:?}"));
                }
            }
            for (j, (other, op)) in r.routes.iter().zip(&w.paths).enumerate() {
                if j <= i || (route.bundle.is_some() && route.bundle == other.bundle) {
                    continue;
                }
                for t in op.windows(2) {
                    if collinear_overlap((a, b), (t[0], t[1])) {
                        bad.push(format!("{k} overlaps {}: {path:?} / {op:?}", other.key));
                    }
                }
            }
        }
        for &(px, py) in path {
            if px % 2 != 0
                || py % 4 != 0
                || !(0..=p.frame.0).contains(&px)
                || !(0..=p.frame.1).contains(&py)
            {
                bad.push(format!("{k}: vertex ({px},{py}) off the lattice or frame"));
            }
        }
    }
    for c in &w.channels {
        for &(a, b) in &c.hard {
            if c.segments[a].track >= c.segments[b].track {
                bad.push(format!("{:?}: hard edge {a} -> {b} violated", c.channel));
            }
        }
        if c.dropped > 0 {
            bad.push(format!(
                "{:?}: {} hard cycles dropped",
                c.channel, c.dropped
            ));
        }
        if c.tracks < c.density || c.tracks < c.hard_path {
            bad.push(format!(
                "{:?}: {} tracks < density {} or hard path {}",
                c.channel, c.tracks, c.density, c.hard_path
            ));
        }
    }
    bad
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    // Roadmap M4.4 acceptance: the VCG is respected, every channel has at least as many tracks
    // as its density, the final polylines keep the §6.3 and §8.2 rules, and the result is
    // deterministic.
    #[test]
    fn wiring_is_valid(
        n in 1usize..8,
        arrows in proptest::collection::vec((0usize..7, 0usize..7, 0u8..3), 0..18),
        boundary in proptest::collection::vec((0u8..9, 0usize..7), 0..10),
        outs in proptest::collection::vec(0usize..7, 0..4),
        calls in proptest::collection::vec(0usize..7, 0..3),
    ) {
        let src = generated(n, &arrows, &boundary, &outs, &calls);
        let model = parse(&src).model.expect("model");
        let bad = wiring_violations(&model);
        prop_assert!(bad.is_empty(), "{src}\n{bad:#?}");
        let m = Metrics::default();
        let a = layout_routes(&model, &NodeId::a0(), true, TextMetric::Mono, &m).expect("A0");
        let b = layout_routes(&model, &NodeId::a0(), true, TextMetric::Mono, &m).expect("A0");
        prop_assert_eq!(a, b);
    }
}

// The M4.4 properties on every committed fixture with an A0.
#[test]
fn fixtures_wire_cleanly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for f in [
        "bench/dense-7x40.idef0",
        "bench/arrange-worst-7.idef0",
        "dogfood/idef0-rs.idef0",
    ] {
        let bad = wiring_violations(&load(&root.join(f)));
        assert!(bad.is_empty(), "{f}: {bad:#?}");
    }
    for e in std::fs::read_dir(root.join("layout/templates")).expect("templates") {
        let path = e.expect("entry").path();
        let bad = wiring_violations(&load(&path));
        assert!(bad.is_empty(), "{}: {bad:#?}", path.display());
    }
}

// Roadmap M4.4 acceptance (testing §11): crossings stay at or below the committed baseline.
// Lower the baseline when a change improves it; raise it only with a `baseline:` reason.
#[test]
fn crossings_within_baseline() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let text =
        std::fs::read_to_string(root.join("layout/baselines/crossings.json")).expect("baseline");
    let baseline: std::collections::BTreeMap<String, usize> =
        serde_json::from_str(&text).expect("baseline JSON");
    // every benchmark diagram has a baseline
    for e in std::fs::read_dir(root.join("bench")).expect("bench") {
        let name = e.expect("entry").file_name().to_string_lossy().into_owned();
        assert!(
            baseline.contains_key(&format!("bench/{name}")),
            "no baseline for bench/{name}"
        );
    }
    let m = Metrics::default();
    for (f, &limit) in &baseline {
        let model = load(&root.join(f));
        let x = layout_routes(&model, &NodeId::a0(), true, TextMetric::Mono, &m).expect("A0");
        let got = x.wiring.crossings;
        assert!(got <= limit, "{f}: {got} crossings > baseline {limit}");
        if got < limit {
            eprintln!("{f}: {got} crossings, below the baseline {limit}: lower it");
        }
    }
}

// M4.4 goldens: the track of every channel segment on the benchmark and dogfood diagrams.
// Sifting keeps a swap only when crossings fall, so ties keep the left-edge order.
#[test]
fn wiring_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for f in ["bench/dense-7x40.idef0", "dogfood/idef0-rs.idef0"] {
        let model = load(&root.join(f));
        let x = layout_routes(
            &model,
            &NodeId::a0(),
            true,
            TextMetric::Mono,
            &Metrics::default(),
        )
        .expect("A0");
        let mut out = vec![format!("crossings {}", x.wiring.crossings)];
        for c in x.wiring.channels.iter().filter(|c| c.tracks > 0) {
            let segs: Vec<String> = c
                .segments
                .iter()
                .map(|s| format!("{}.{}={}", s.key, s.part, s.track))
                .collect();
            out.push(format!(
                "{:?} tracks={} {}",
                c.channel,
                c.tracks,
                segs.join(" ")
            ));
        }
        let name = f.replace(['/', '.'], "-");
        insta::assert_snapshot!(name, out.join("\n"));
    }
}

// FR-LAYOUT-22: one boundary port is one point on the frame. The branches of a fork from
// it start at the same entry, so its code takes one slot and later codes keep theirs.
#[test]
fn fork_branches_share_their_entry() {
    let src = "idef0 0.2\nmodel \"G\" {\n  abbrev \"GEN\"\n}\ndiagram A0 {\n  box 1 \"Box 1\"\n  box 2 \"Box 2\"\n  box 3 \"Box 3\"\n  arrow : M1 -> fork {\n    -> 1M\n    -> 3M\n  }\n  arrow : M2 -> 2M\n}\n";
    let p = parse(src);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let (_, _, r) = routes(&p.model.expect("model"), &NodeId::a0());
    let start = |k: &str| {
        r.routes
            .iter()
            .find(|x| x.key.to_string().starts_with(k))
            .map(|x| x.path[0])
            .expect("route")
    };
    assert_eq!(
        start("A0:M1->*#0/0"),
        start("A0:M1->*#0/1"),
        "{}",
        listing(&r)
    );
    let m2 = r
        .routes
        .iter()
        .find(|x| x.key.to_string().starts_with("A0:M2"))
        .expect("M2");
    assert_eq!(
        m2.class,
        Class::BoundaryStraight,
        "M2 keeps its own port's column"
    );
}

// A fork's shared entry follows its matching-side branch: I1 forking into 2I and 1C enters
// at box 2's input row, so that branch is straight and only the other one dog-legs.
#[test]
fn fork_entry_follows_the_matching_branch() {
    let src = "idef0 0.2\nmodel \"G\" {\n  abbrev \"GEN\"\n}\ndiagram A0 {\n  box 1 \"Box 1\"\n  box 2 \"Box 2\"\n  arrow : I1 -> fork {\n    -> 2I\n    -> 1C\n  }\n}\n";
    let p = parse(src);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let (_, _, r) = routes(&p.model.expect("model"), &NodeId::a0());
    let class = |k: &str| {
        r.routes
            .iter()
            .find(|x| x.key.to_string().starts_with(k))
            .map(|x| x.class)
    };
    assert_eq!(
        class("A0:I1->*#0/0"),
        Some(Class::BoundaryStraight),
        "{}",
        listing(&r)
    );
    assert_eq!(class("A0:I1->*#0/1"), Some(Class::BoundaryDogLeg));
}
