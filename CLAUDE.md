# CLAUDE.md: working in idef0-rs

idef0-rs is an AI-first, keyboard-driven IDEF0 (FIPS PUB 183) modeling tool written in Rust. This file is a **map and a short list of rules**. The specs are the source of truth.

**Authority order:** [docs/NORTHSTAR.md](docs/NORTHSTAR.md) > [docs/PRD.md](docs/PRD.md) and the spec set > this file. If this file disagrees with a spec, the spec wins; fix this file.

## Read before you code

| If you are working on… | Read |
|---|---|
| Anything (start here) | [docs/PRD.md](docs/PRD.md) (index, decisions, reading guide), [docs/glossary.md](docs/glossary.md) |
| `idef0-core` (model, DSL, commands, identity, editing) | [docs/spec/metamodel.md](docs/spec/metamodel.md) |
| `idef0-mangle` (facts, rules, diagnostics) | [docs/spec/validation.md](docs/spec/validation.md) |
| `idef0-layout` (auto-layout, routing, renderers, themes) | [docs/spec/layout.md](docs/spec/layout.md) |
| Editor FSM, keymap, search, `idef0-wasm` UI | [docs/spec/editor-ux.md](docs/spec/editor-ux.md) |
| `idef0-ingest` (PRD tags, code scanner, sync, import/export) | [docs/spec/ingestion.md](docs/spec/ingestion.md) |
| Contract cards, AI proposals, MCP, threat model | [docs/spec/agents.md](docs/spec/agents.md) |
| Crates, API, server security, dependencies, CI | [docs/architecture.md](docs/architecture.md) |
| Tests, fixtures, goldens | [docs/testing.md](docs/testing.md) |
| CLI commands | [docs/cli.md](docs/cli.md) |
| What to build next | [docs/roadmap.md](docs/roadmap.md) |
| The standard itself | [docs/research/fipspub183.pdf](docs/research/fipspub183.pdf) |

## Non-negotiable invariants

1. **IDEF0 means FIPS 183.** Every rule traces to a clause. idef0-rs additions are labeled **[Extension]** and departures **[Deviation]**. The `fips183` profile switches extensions off, downgrades them to info, or keeps them as notation only (identity keys, slash port refs); deviations revert to FIPS behavior where the tool supports it (box limit 6, `( )` tunnel glyphs, box-number corner label), and the strict staircase remains a documented deviation.
2. **Determinism (D6).** Identical model bytes produce identical facts, diagnostics, Scene and SVG on native and wasm32.
   - No `HashMap` iteration in outputs.
   - Integer grid geometry.
   - No `Instant::now`, `SystemTime` or `Ulid::new` in `idef0-core`, `idef0-mangle` or `idef0-layout`. Inject clocks and ID generators instead.
3. **Identity (D14).** Parsing never mints random IDs. Box keys are stable slugs written to the file. Call arrows target boxes by key.
4. **One rule set everywhere.** Structural checks are Rust (`E-*`, `L-*`), deductive checks are Mangle (`R-*`), and geometry checks are `G-*`. The same code runs in the browser, CLI and CI.
5. **Layout is computed, never drawn.** It follows FIPS: staircase, "up and over" control feedback, "down and under" input and mechanism feedback, 90° arcs.
6. **Agent writes need human acceptance (D15).** Never auto-apply model changes that come from an agent or an LLM.
7. **WASM safety.** `idef0-core`, `idef0-mangle` and `idef0-layout` must build for `wasm32-unknown-unknown`: no file I/O, no threads. `idef0-ingest` (tree-sitter, C) and `idef0-server` are native only.

## Conventions that are easy to get wrong

| Topic | Rule |
|---|---|
| Tunnel glyphs | `( )` at a box end means hidden on the child. `[ ]` at a border end means hidden on the parent and has no ICOM code. `fips183` renders `( )` at both ends. |
| Crossings | The **vertical** arrow carries the bridge (bump). |
| Fillets | 6–10 px radius at zoom 1. Corners are curved, forks and joins curve too, boxes keep square corners. |
| Controls | Conditions required for correct output. Controls *may* be transformed (FIPS §2.17); "not consumed" is wrong. |
| Mechanisms | They map to code, or are marked `kind=resource` or `kind=component`. |
| Drop shadow | Box has a child diagram. No shadow means a leaf (or a caller box, which is detailed elsewhere). |
| DSL endpoints | FIPS reference style: `2O2 -> 3C1`. Port reference `A21/I1` (FIPS `A21I1` is accepted). |
| Keys in tables | A capital letter means the unshifted key; `Shift+X` means shifted. |

## How to work a milestone

1. Find the epic in [docs/roadmap.md](docs/roadmap.md) and read the spec sections it lists.
2. Write tests and fixtures first: rule fixtures, layout snapshots, `.keys` FSM scripts.
3. Done means all of these pass:
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - for core, mangle and layout: `cargo build -p <crate> --target wasm32-unknown-unknown`
   - `scripts/check-determinism-lints.sh`, `scripts/check-dependency-graph.sh`, `cargo deny --locked check`, `python3 scripts/check-doc-links.py` (`cargo deny` is a separate tool: `cargo install cargo-deny --locked --version ~0.20`, as pinned in [architecture §12.1](docs/architecture.md#121-workspace-manifest))

   CI (`.github/workflows/ci.yml`) runs the same checks on every PR.
4. Never weaken a golden, snapshot or determinism test to make it pass. Update a snapshot only for an intended change, and say so in the commit.
5. Commit message: `M<n>.<k>: <summary>`, citing requirement IDs (FR-*, R-*, SR-* when fixing review findings). Work that no roadmap epic owns (tooling, Claude skills, repo housekeeping) uses `chore: <summary>` instead, and still cites requirement IDs it touches. PR titles follow the same rule.
6. Review PRs with `/pr-review <N>` ([.claude/skills/pr-review](.claude/skills/pr-review/SKILL.md)). It verifies claims in a worktree and drafts the review. Nothing is posted until the user approves.

## Repo state

Pre-alpha. The crates are skeletons. The spec set is v0.2 (split from the original monolithic PRD after the [2026-10-15 review](docs/reviews/2026-10-15-spec-review.md)). License: MIT OR Apache-2.0.
