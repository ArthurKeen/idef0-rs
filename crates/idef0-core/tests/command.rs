//! Commands on random edit sequences (roadmap M1.5; metamodel §13).

use std::path::Path;

use idef0_core::command::{Command, Described, End, EndSpec, Env, apply, apply_ops, copy};
use idef0_core::diag::Severity;
use idef0_core::id::{NodeId, Side};
use idef0_core::identity::{SlugMinter, arrow_ids};
use idef0_core::model::{ArrowShape, CallArrow, Endpoint, KeyRef, Model, Status};
use idef0_core::text::BoxKey;
use idef0_core::text::NonEmptyString;
use idef0_core::typecheck::typecheck;
use nonempty::NonEmpty;
use proptest::prelude::*;

fn fixture(name: &str) -> Model {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let p = idef0_core::dsl::parse(&std::fs::read_to_string(path).expect("fixture"));
    p.model.expect("model")
}

fn errors(m: &Model) -> usize {
    typecheck(m)
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

fn ends(shape: &ArrowShape) -> Vec<&Endpoint> {
    match shape {
        ArrowShape::Plain { src, dst } => vec![src, dst],
        ArrowShape::Fork { src, fork } => std::iter::once(src)
            .chain(fork.iter().map(|b| &b.dst))
            .collect(),
        ArrowShape::Join { join, dst } => join
            .iter()
            .map(|l| &l.src)
            .chain(std::iter::once(dst))
            .collect(),
    }
}

const SIDES: [Side; 4] = [Side::I, Side::C, Side::O, Side::M];

/// Turn a pick into a command against the current model, if the model offers one.
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one arm per command kind; a, b, c are the pick's free choices"
)]
fn command(m: &Model, (kind, a, b, c): (u8, usize, usize, usize)) -> Option<Command> {
    let decomp: Vec<&NodeId> = m
        .diagrams
        .keys()
        .filter(|n| matches!(n, NodeId::Node(_)))
        .collect();
    let any: Vec<&NodeId> = m.diagrams.keys().collect();
    let boxes: Vec<_> = decomp
        .iter()
        .flat_map(|n| m.diagrams[*n].boxes.iter().map(|b| b.key.clone()))
        .collect();
    let pick = |v: usize, len: usize| (len > 0).then(|| v % len);
    let name = |s: String| NonEmptyString::new(s).expect("name");
    Some(match kind % 15 {
        0 => {
            let d = decomp[pick(a, decomp.len())?].clone();
            let n = m.diagrams[&d].boxes.len();
            Command::AddBox {
                diagram: d,
                at: u8::try_from(1 + b % (n + 1)).ok()?,
                name: name(format!("Box {c}")),
                description: String::new(),
            }
        }
        1 => Command::RenameBox {
            key: boxes[pick(a, boxes.len())?].clone(),
            name: name(format!("Renamed {b}")),
        },
        2 => Command::MoveBox {
            key: boxes[pick(a, boxes.len())?].clone(),
            to: u8::try_from(1 + b % 7).ok()?,
        },
        3 => Command::DeleteBox {
            key: boxes[pick(a, boxes.len())?].clone(),
        },
        4 | 5 => {
            let d = any[pick(a, any.len())?].clone();
            let ids: Vec<_> = arrow_ids(&d, &m.diagrams[&d])
                .into_iter()
                .flat_map(|s| std::iter::once(s.trunk).chain(s.parts))
                .collect();
            let id = ids[pick(b, ids.len())?].clone();
            if kind % 15 == 4 {
                Command::DeleteArrow { diagram: d, id }
            } else {
                let keys: Vec<_> = m.diagrams[&d].boxes.iter().map(|x| x.key.clone()).collect();
                let key = keys[pick(c, keys.len())?].clone();
                let end = if b % 2 == 0 { End::Src } else { End::Dst };
                let side = if end == End::Src {
                    Side::O
                } else {
                    SIDES[c % 4]
                };
                Command::Retarget {
                    diagram: d,
                    id,
                    end,
                    to: EndSpec::NewPort { key, side },
                }
            }
        }
        6 => {
            let key = boxes[pick(a, boxes.len())?].clone();
            let side = SIDES[c % 4];
            let d = m
                .diagrams
                .iter()
                .find(|(n, d)| {
                    matches!(n, NodeId::Node(_)) && d.boxes.iter().any(|x| x.key == key)
                })?
                .1;
            let mut ranks: Vec<u8> = d
                .arrows
                .iter()
                .flat_map(|ar| ends(&ar.shape))
                .filter_map(|e| match e {
                    Endpoint::Box {
                        key: k,
                        side: s,
                        rank,
                    } if *k == key && *s == side => Some(*rank),
                    _ => None,
                })
                .collect();
            ranks.sort_unstable();
            ranks.dedup();
            if ranks.is_empty() {
                return None;
            }
            let shift = b % ranks.len();
            ranks.rotate_left(shift);
            Command::ReorderPorts {
                key,
                side,
                order: ranks,
            }
        }
        7 => Command::SetDescription {
            subject: Described::Box(boxes[pick(a, boxes.len())?].clone()),
            text: "Mentions A2 and A3.1.".into(),
        },
        8 => Command::DeleteDiagram {
            diagram: decomp[pick(a, decomp.len())?].clone(),
        },
        9 => {
            let key = boxes[pick(a, boxes.len())?].clone();
            let clip = copy(m, &key, b % 2 == 0).ok()?;
            let d = decomp[pick(c, decomp.len())?].clone();
            let n = m.diagrams[&d].boxes.len();
            Command::Paste {
                diagram: d,
                at: u8::try_from(1 + b % (n + 1)).ok()?,
                clip,
            }
        }
        10 => {
            let d = decomp[pick(a, decomp.len())?].clone();
            let keys: Vec<_> = m.diagrams[&d].boxes.iter().map(|x| x.key.clone()).collect();
            let (s, t) = (
                keys[pick(b, keys.len())?].clone(),
                keys[pick(c, keys.len())?].clone(),
            );
            Command::AddArrow {
                diagram: d,
                src: EndSpec::NewPort {
                    key: s,
                    side: Side::O,
                },
                dst: EndSpec::NewPort {
                    key: t,
                    side: SIDES[[0, 1, 3][c % 3]],
                },
                concept: None,
            }
        }
        12 => Command::Decompose {
            key: boxes[pick(a, boxes.len())?].clone(),
        },
        13 => {
            let target = boxes[pick(b, boxes.len())?].clone();
            let t: KeyRef<BoxKey> = format!("@{target}").parse().ok()?;
            Command::SetCall {
                key: boxes[pick(a, boxes.len())?].clone(),
                call: CallArrow {
                    targets: NonEmpty::new(t),
                    note: None,
                    map: Vec::new(),
                },
            }
        }
        14 => Command::ClearCall {
            key: boxes[pick(a, boxes.len())?].clone(),
        },
        _ => Command::SetStatus {
            diagram: any[pick(a, any.len())?].clone(),
            status: [Status::Draft, Status::Recommended][b % 2],
        },
    })
}

fn fixtures() -> Vec<Model> {
    [
        "valid/three-boxes.idef0",
        "valid/constructs.idef0",
        "dogfood/idef0-rs.idef0",
        "bench/dense-7x40.idef0",
    ]
    .into_iter()
    .map(fixture)
    .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    // Roadmap M1.5 acceptance: validity is preserved and undo ∘ do = id, for every command
    // in a random edit sequence; apply is deterministic (FR-EDIT-01).
    #[test]
    fn edits_keep_the_model_valid_and_undoable(
        start in 0usize..4,
        picks in proptest::collection::vec((0u8..15, 0usize..64, 0usize..64, 0usize..64), 1..14),
    ) {
        let mut m = fixtures().swap_remove(start);
        let before_errors = errors(&m);
        let mut applied = 0;
        for p in picks {
            let Some(cmd) = command(&m, p) else { continue };
            let mut minter = SlugMinter;
            let mut env = Env { minter: &mut minter };
            let Ok((next, patch)) = apply(&m, &cmd, &mut env) else { continue };
            prop_assert!(errors(&next) <= before_errors, "{cmd:?}: {:?}", typecheck(&next));
            prop_assert_eq!(&apply_ops(&next, &patch.inverse), &m, "undo of {:?}", cmd);
            prop_assert_eq!(&apply_ops(&m, &patch.ops), &next, "redo of {:?}", cmd);
            let mut minter = SlugMinter;
            let mut env = Env { minter: &mut minter };
            let again = apply(&m, &cmd, &mut env).expect("deterministic");
            prop_assert_eq!(&again.0, &next);
            prop_assert_eq!(&again.1, &patch);
            for (key, gone) in &patch.rename.boxes {
                prop_assert!(gone.is_none() && next.meta.retired.contains(key), "D17: {key}");
            }
            m = next;
            applied += 1;
        }
        let _ = applied;
    }
}

// Positive control: the generator is not vacuous. Over a fixed pseudo-random run every
// command kind applies at least once, and most picks apply.
#[test]
fn every_command_kind_applies() {
    let mut seed: u64 = 0x1DEF0;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(seed >> 33).unwrap_or(0)
    };
    let mut applied = [0usize; 15];
    let mut tried = 0;
    for round in 0..200 {
        let mut m = fixtures().swap_remove(round % 4);
        for _ in 0..10 {
            let kind = u8::try_from(next() % 15).unwrap_or(0);
            let Some(cmd) = command(&m, (kind, next(), next(), next())) else {
                continue;
            };
            tried += 1;
            let mut minter = SlugMinter;
            let mut env = Env {
                minter: &mut minter,
            };
            if let Ok((after, _)) = apply(&m, &cmd, &mut env) {
                applied[usize::from(kind)] += 1;
                m = after;
            }
        }
    }
    assert!(applied.iter().all(|&n| n > 0), "{applied:?}");
    let total: usize = applied.iter().sum();
    assert!(total * 2 > tried, "{total} of {tried} applied: {applied:?}");
}
