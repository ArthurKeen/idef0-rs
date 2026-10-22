//! Times layout on the bench fixtures (layout §16.4; ADR 0002): the real sizing, placement and
//! global routing (M4.1–M4.3), and the M4.0 spike's Auto-Arrange step 1.
//!
//! `layout-spike`               timings: median and p95 per operation
//! `layout-spike once <op>`     one run of `<op>`, for `valgrind --tool=callgrind`; subtract
//!                              `once noop` (which only parses the fixtures) to isolate `<op>`
//!
//! Operations: `parse`, `place` (sizing and one staircase placement), `layout` (placement
//! iteration with global routing), `layout-dogfood`, `layout-worst`, `arrange1` (Auto-Arrange
//! step 1 on the worst case).

use std::hint::black_box;
use std::time::Instant;

use idef0_core::dsl::parse;
use idef0_core::id::NodeId;
use idef0_core::model::Model;
use idef0_layout::metrics::Metrics;
use idef0_layout::place::{EdgeAllowance, Gaps, Tracks, box_plans, place};
use idef0_layout::route::{Routed, layout_routes};
use idef0_layout::spike::{arrange_box_order, diagram};
use idef0_layout::text::TextMetric;

const DENSE: &str = include_str!("../../../fixtures/bench/dense-7x40.idef0");
const WORST: &str = include_str!("../../../fixtures/bench/arrange-worst-7.idef0");
const DOGFOOD: &str = include_str!("../../../fixtures/dogfood/idef0-rs.idef0");

fn model(src: &str) -> Model {
    parse(src).model.expect("fixture parses")
}

fn layout(m: &Model) -> Routed {
    let metrics = Metrics::default();
    layout_routes(m, &NodeId::a0(), true, TextMetric::Mono, &metrics).expect("A0")
}

fn run(op: &str, dense: &Model, worst: &Model, dogfood: &Model) {
    match op {
        "noop" => {} // baseline for instruction counts: fixture parsing in main() only
        "parse" => {
            black_box(parse(black_box(DENSE)));
        }
        "place" => {
            let m = Metrics::default();
            let plans = box_plans(dense, &NodeId::a0(), true, TextMetric::Mono, &m);
            let gaps = Gaps::required(&Tracks::none(plans.len()), EdgeAllowance::default(), &m);
            black_box(place(&plans, &gaps, &m));
        }
        "layout" => {
            black_box(layout(dense));
        }
        "layout-dogfood" => {
            black_box(layout(dogfood));
        }
        "layout-worst" => {
            black_box(layout(worst));
        }
        "arrange1" => {
            black_box(arrange_box_order(diagram(worst, "A0").expect("A0")));
        }
        other => panic!("unknown op {other}"),
    }
}

/// Bends in the wired polylines: vertices where the direction turns.
fn bends(r: &Routed) -> usize {
    r.wiring
        .paths
        .iter()
        .map(|p| p.len().saturating_sub(2))
        .sum()
}

fn facts(name: &str, r: &Routed) {
    let forced = r.routing.routes.iter().filter(|x| x.forced).count();
    let doglegs: usize = r.wiring.channels.iter().map(|c| c.doglegs.len()).sum();
    let tracks: u32 = r.wiring.channels.iter().map(|c| c.tracks).sum();
    println!(
        "{name}: routed={} forced={forced} bends={} crossings={} tracks={tracks} doglegs={doglegs} iterations={} passes={}",
        r.routing.routes.len(),
        bends(r),
        r.wiring.crossings,
        r.placed.iterations,
        r.routing.passes
    );
}

const OPS: [&str; 6] = [
    "parse",
    "place",
    "layout",
    "layout-dogfood",
    "layout-worst",
    "arrange1",
];

fn main() {
    let (dense, worst, dogfood) = (model(DENSE), model(WORST), model(DOGFOOD));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("once") {
        run(&args[1], &dense, &worst, &dogfood);
        return;
    }
    let iters: usize = args.first().and_then(|a| a.parse().ok()).unwrap_or(200);
    // Facts about the workload, so the ADR can quote them. Crossings come with track
    // assignment (M4.4).
    facts("dense-7x40", &layout(&dense));
    facts("arrange-worst-7", &layout(&worst));
    facts("dogfood", &layout(&dogfood));
    let wd = diagram(&worst, "A0").expect("A0");
    let a = arrange_box_order(wd);
    println!(
        "arrange-worst-7 step1: cost {} -> {} order={:?} nodes={} leaves={}",
        a.current_cost, a.cost, a.order, a.nodes, a.leaves
    );
    println!("op,iters,median_us,p95_us,max_us");
    for op in OPS {
        for _ in 0..5 {
            run(op, &dense, &worst, &dogfood); // warm-up
        }
        let mut t: Vec<u128> = (0..iters)
            .map(|_| {
                let start = Instant::now();
                run(op, &dense, &worst, &dogfood);
                start.elapsed().as_micros()
            })
            .collect();
        t.sort_unstable();
        let pick = |q: usize| t[(t.len() * q / 100).min(t.len() - 1)];
        println!("{op},{iters},{},{},{}", pick(50), pick(95), t[t.len() - 1]);
    }
}
