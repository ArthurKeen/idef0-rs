//! Times the M4.0 layout spike on the bench fixtures (layout §16.4).
//!
//! `layout-spike`               timings: median and p95 per operation
//! `layout-spike once <op>`     one run of `<op>`, for `valgrind --tool=callgrind`; subtract
//!                              `once noop` (which only parses the fixtures) to isolate `<op>`
//!
//! Operations: `parse`, `place`, `layout` (place + global routing), `arrange1`
//! (Auto-Arrange step 1 on the worst case), `layout-worst`.

use std::hint::black_box;
use std::time::Instant;

use idef0_core::dsl::parse;
use idef0_core::model::Model;
use idef0_layout::spike::{arrange_box_order, diagram, place, route};

const DENSE: &str = include_str!("../../../fixtures/bench/dense-7x40.idef0");
const WORST: &str = include_str!("../../../fixtures/bench/arrange-worst-7.idef0");
const DOGFOOD: &str = include_str!("../../../fixtures/dogfood/idef0-rs.idef0");

fn model(src: &str) -> Model {
    parse(src).model.expect("fixture parses")
}

fn run(op: &str, dense: &Model, worst: &Model, dogfood: &Model) {
    let d = |m| diagram(m, "A0").expect("A0");
    match op {
        "noop" => {} // baseline for instruction counts: fixture parsing in main() only
        "parse" => {
            black_box(parse(black_box(DENSE)));
        }
        "place" => {
            black_box(place(d(dense)));
        }
        "layout" => {
            let p = place(d(dense));
            black_box(route(d(dense), &p));
        }
        "layout-dogfood" => {
            let p = place(d(dogfood));
            black_box(route(d(dogfood), &p));
        }
        "layout-worst" => {
            let p = place(d(worst));
            black_box(route(d(worst), &p));
        }
        "arrange1" => {
            black_box(arrange_box_order(d(worst)));
        }
        other => panic!("unknown op {other}"),
    }
}

const OPS: [&str; 6] = ["parse", "place", "layout", "layout-dogfood", "layout-worst", "arrange1"];

fn main() {
    let (dense, worst, dogfood) = (model(DENSE), model(WORST), model(DOGFOOD));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("once") {
        run(&args[1], &dense, &worst, &dogfood);
        return;
    }
    let iters: usize = args.first().and_then(|a| a.parse().ok()).unwrap_or(200);
    // Facts about the workload, so the ADR can quote them.
    let dd = diagram(&dense, "A0").expect("A0");
    let r = route(dd, &place(dd));
    println!("dense-7x40: edges={} routed={} crossings={} bends={} candidates={} passes={}", 40, r.routes.len(), r.crossings, r.bends, r.candidates, r.passes);
    let wd = diagram(&worst, "A0").expect("A0");
    let rw = route(wd, &place(wd));
    println!("arrange-worst-7 layout: routed={} crossings={} bends={} candidates={}", rw.routes.len(), rw.crossings, rw.bends, rw.candidates);
    let a = arrange_box_order(wd);
    println!("arrange-worst-7 step1: cost {} -> {} order={:?} nodes={} leaves={}", a.current_cost, a.cost, a.order, a.nodes, a.leaves);
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
