# idef0-rs Roadmap

| Field | Value |
|---|---|
| Status | DRAFT v0.2 |
| Part of | idef0-rs spec set; index: [PRD.md](PRD.md) |
| Scope | Milestones and epics, sized for one Claude Code session each, with the spec sections to read and the fixtures each epic needs. Replaces v0.1 PRD §6. |

## 1. Working agreement

- **One epic per session.** It ends in one or more commits. Prompt template:

  > Implement epic **M{n}.{k}** from `docs/roadmap.md`. Read the spec sections listed for it, and `CLAUDE.md`. Write the fixtures and tests first. Done when the epic's acceptance passes, plus `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. For core, mangle and layout, also run `cargo build -p <crate> --target wasm32-unknown-unknown`. Commit as `M{n}.{k}: <summary>`, citing requirement IDs.

- **Spikes** (marked ⚑) are time-boxed (1–2 sessions). Each ends in an ADR in `docs/adr/` that records measurements and the decision. A spike's gate decides how later epics are built.
- **Acceptance is automated.** Every criterion is a test or CI check ([testing §acceptance policy](testing.md)).
- **No weakening tests.** Never weaken golden, snapshot or determinism tests. Update snapshots only for intended changes, and say so in the commit.

**Spec abbreviations** used in the tables: MM [metamodel](spec/metamodel.md), VAL [validation](spec/validation.md), LAY [layout](spec/layout.md), UX [editor-ux](spec/editor-ux.md), ING [ingestion](spec/ingestion.md), AG [agents](spec/agents.md), ARCH [architecture](architecture.md), TEST [testing](testing.md), CLI [cli](cli.md).

## 2. Milestones

### M0: Foundation

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M0.1 | Remove the nested `crates/*/.git` and per-crate `.gitignore`. Run `git init` at the root. Add a root `.gitignore` (`target/`, `dist/`, `.idef0/cache/`, `.idef0/run/`, `.idef0/proposals/`). | ARCH (repo) | — | `find crates -name .git` is empty; single repository; first commit includes the docs |
| M0.2 | `[workspace.package]` (edition 2024, `license = "MIT OR Apache-2.0"`, `rust-version`). `[workspace.dependencies]` per the ARCH table. `[workspace.lints]` (`unsafe_code = "forbid"`, no exceptions). `rust-toolchain.toml`. `clippy.toml` disallowed types and methods (determinism). Create the `idef0-ingest` crate. Split `idef0-server` into lib + bin. | ARCH (dependencies, lints, determinism) | — | `cargo build --workspace`; wasm32 builds for core, mangle and layout; a disallowed `Instant::now` in core fails clippy (negative test) |
| M0.3 | CI base: fmt, clippy, test, wasm32 check, `cargo deny`, markdown link check over `docs/`. | ARCH (CI), TEST | — | CI green on skeletons; link check passes |
| M0.4 | CI cross-OS (macOS, Windows MSVC) and MSRV job. | ARCH (CI matrix) | — | Matrix green |

### M1: Core metamodel

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M1.1 | `NodeId`, `NodeRef`, `PortRef` parsing and display, with dotted normalization (`A21.3`→`A213`, `A0.k`→`Ak`) and profile digit limits | MM (identity) | — | Property `parse(display(x)) == x`; `A0.3 ≡ A3`; `A8` rejected under both profiles |
| M1.2 | Model types: `FunctionBox` (`NonEmptyString` name, description, `dre`), `Concept`, `Arrow`, `Diagram` (kinds `/context`, `/higher_context`, `/decomposition`), key-based `CallArrow`, notes; serde JSON | MM; ARCH (types) | `fixtures/valid/` | JSON round-trip; empty name is a `compile_fail` doctest |
| M1.3 | Type checker: `E-TYPE-*`, `E-ID-*`, `E-NODE-*`, `E-CALL-*`, `E-NOTE-01` | MM (arrow typing); VAL §7.3 | `fixtures/invalid/E-*/` | One test per allowed and forbidden typing cell |
| M1.4 | Identity: slug key minting via an injected `KeyMinter`, and deterministic structural arrow ids (D14) | MM (identity); ARCH (determinism) | — | Same bytes give same ids (native and wasm); no random ids |
| M1.5 | `Command`/`Patch`/undo, and RenameMap for nodes and ports. FR-EDIT: delete cascade, renumbering, single atomic `ReorderPorts` with child re-coding, copy/paste, status reset rules, per-model undo with navigation | MM (editing semantics) | — | proptest: validity is preserved; undo∘do = id; deleting box 2 of 5 renames the subtree and ports correctly |
| M1.6 | `Decompose` (0 boxes; parent ICOMs appear as stubs); `SetCall`/`ClearCall` (rejected on decomposed boxes); Used At reverse index | MM (call arrows, balance) | — | Tests per FR-CALL-01..09; `Decompose` on a caller is rejected |
| M1.7 | **Random valid-model generator** (`idef0_core::testkit`, feature-gated): multi-level, balanced, with bundles, tunnels, calls and concepts; shrinking | TEST (generator) | `fixtures/large/` (seeded) | Generated models pass the type check; deterministic per seed. Full rule-validity check added after M3.3. |

### M2: DSL, formatter, lints, search core

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M2.1 | Lossless CST parser: version header, resync error recovery, `SpanMap`, parser limits (`E-LIMIT-01`) | MM (grammar, workspace); ARCH (limits) | `fixtures/valid/`, `fixtures/invalid/E-PARSE-*` | Parses all valid fixtures; multi-error snapshots |
| M2.2 | Canonical formatter: comment round-trip, key write-back, `@key` call normalization | MM (formatter) | dogfood, factory | `format(parse(format(m))) == format(m)`; comments survive an edit round-trip |
| M2.3 | Core lints (`L-META-*`, `L-TITLE-01`, `L-NAME-*`, `L-CONCEPT-*`, `L-DESC-*`, `L-NOTE-01`, `L-CNUM-01`, `L-GLOSS-01`, `L-ROLE-01`, `L-BUN-01`, `L-SPLIT-CI`, `L-FMT-01`). Verb lexicon chosen (Q-15). | VAL §7.2; MM | `fixtures/lint/<code>/` | One fixture per lint |
| M2.4 | CLI `init`, `fmt`, `check`, `index`, `migrate` (0.1→0.2) | CLI; MM (versioning) | dogfood, factory | `idef0 check` clean on both fixtures (except their documented expected diagnostics); `fmt --check` exits 0 |
| M2.5 | Search core: tokenizer, stemming, sorted term array plus delta, `levenshtein_automata`, BM25 field weights, filters; `idef0 search` | UX (search) | dogfood, factory | Golden ranking tests; incremental index equals full rebuild (proptest); wasm32 build |
| M2.6 | Fuzzing harness (`cargo-fuzz`) for the DSL parser | TEST (fuzzing) | corpus from fixtures | 10 min nightly fuzz run with no crashes |

### M3: Rule engine

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M3.1 ⚑ | **Mangle backend spike.** Five checks: `mangle-rs` parses and analyzes the rule files; negation and `group_by` work; wasm32 build; native/wasm parity; **wasm latency** p50/p95 on dogfood and the generated 60-diagram model (`fixtures/large/`). ADR 0001: backend choice and Worker decision (Worker if p95 > 16 ms). | VAL (engine backends) | dogfood, `fixtures/large/` | ADR merged; the chosen backend runs a toy program natively and on wasm32 |
| M3.2 | Fact extraction (FR-FACT-01..28) with sorted output; `idef0 facts`; `invariants.mg` in debug builds | VAL (extraction) | dogfood | Extractor golden test; the dump parses with the chosen backend |
| M3.3 | `schema.mg`, `idef0.mg`, `severity.mg`, `profile_fips183.mg`; `Validator` → `Report`; registry coverage test (registry ↔ `severity.mg` ↔ rule heads match 1:1) | VAL (rules, registry, fixtures) | `fixtures/invalid/<code>/`, `fixtures/facts/` | Each fixture *contains* its code and no other CI error outside its allowlist; `fixtures/valid` is clean |
| M3.4 | Profiles; project-rule sandbox (`E-RULES-*`, eval caps); JSON/SARIF output with renumber-proof fingerprints; FR-METRIC-01 in output; `validate --fail-on/--context/--workspace-metric` | VAL; CLI | `fixtures/config/`, `fixtures/rules/` | SARIF validates against the 2.1.0 schema; fingerprints stable across renumbering |
| M3.5 | Native vs wasm determinism suite | TEST (cross-target) | all fixtures | Identical `Report` JSON on all fixtures |

### M4: Layout v1

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M4.0 ⚑ | **Layout spike:** routing templates and the Auto-Arrange proxy search, prototyped and measured on `fixtures/bench/dense-7x40.idef0` and the Auto-Arrange worst case. ADR 0002: budgets and whether layout runs in a Worker. | LAY (performance) | `fixtures/bench/` | ADR merged with instruction counts and wasm timings |
| M4.1 | `Metrics`, box sizing, port positions (text metrics depend on theme) | LAY (coordinates, sizing) | — | Unit tests |
| M4.2 | Staircase placement; gaps that only grow; 3-iteration cap with `L-LAYOUT-OVERFLOW` | LAY (placement) | dogfood | Properties: strict staircase; gaps never shrink; gap-widening lemma test |
| M4.3 | Global routing templates (FIPS feedback shapes), cost function, rip-up and reroute | LAY (global routing) | per-template fixtures | Properties: orthogonal; no box-interior hits; snapshot per class |
| M4.4 | VCG-aware constrained left-edge track assignment | LAY (tracks) | `fixtures/bench/` | VCG respected; tracks ≥ density; deterministic; crossings ≤ committed `crossings.json` baseline |
| M4.5 | `Scene` + SVG renderer + diagram form; `idef0 render --svg` | LAY (outputs) | all | `insta` SVG snapshots |
| M4.6 | Geometry checker (`G-*`) wired into all layout tests | LAY (checker) | all + generator | Zero `G-*` on fixtures; 256 generated cases per PR, 10k nightly |
| M4.7 | Auto-Arrange: box-order proxy search with branch-and-bound, per-side port order, acceptance guard; `idef0 arrange` | LAY (Auto-Arrange) | mis-ordered fixture | Reaches the proxy optimum; never worse than the input; idempotent; `R-BAL` stays clean |

### M5: Layout v2 and rendering

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M5.1 | Fillets (r ∈ [6,10] px at zoom 1) and arrowheads | LAY (fillets) | all | Property r_k ≥ r_min; snapshots |
| M5.2 | Bundles: trunk merge, curved branches, relation glyphs, "to all" stubs | LAY (bundles) | fork/join fixtures | No 4-way junctions (property) |
| M5.3 | Crossings and bridges (the vertical arrow carries the bump) | LAY (bridges) | all | Exact crossing counts; clearance property |
| M5.4 | Labels, squiggles, ICOM codes, tunnel glyphs (`[ ]`/`( )`) | LAY (labels) | all | No overlap OR an emitted `L-LABEL-FIT` |
| M5.5 | `classic` theme (FR-STYLE): label cascade, knockouts, drop shadow when the box has a child, node-number corner label, Liberation Serif metrics | LAY (themes) | `fixtures/reference/factory-a0.idef0` (+ stub A0) | The 10 structural assertions; side-by-side page published as a CI artifact (informative) |
| M5.6 | ASCII renderer (`unicode` and `ascii` charsets, 2×4 gu cell grid, `G-ASCII`); `--txt` | LAY (outputs) | all | Snapshots; `┼` never emitted |
| M5.7 | Performance gates: gungraun instruction-count benches against a committed baseline; criterion/wasm timings nightly (informative) | TEST (benchmarks); ARCH (budgets) | `fixtures/bench/` | `bench-gate` fails on a regression beyond the [testing §6](testing.md#6-benchmarks-and-performance-gates) thresholds (>3 %, >5 % for scan) |
| M5.8 | Soft FIPS rules: double labels, mid-arrowheads, merge/bundle proposals | LAY (soft rules) | fixture per rule | Snapshot of the proposal list |
| M5.9 | Diagram form plus publication export (working fields dropped, Used At copied to a model note); PNG via resvg | LAY (outputs); ING (FR-IO) | factory | PNG within the [testing §5.1](testing.md#51-what-is-snapshotted) tolerance; publication snapshot |
| M5.10 | (stretch) Layout for A-n higher context diagrams | LAY (A-n) | A-1 fixture | Snapshots; `G-*` clean |

### M6: Keyboard FSM (headless)

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M6.1 | `Mode`, `EditorState`, `Input`, `Effect`, `TRANSITIONS`; Normal, BoxNav, PortFocus | UX (FSM) | `fixtures/fsm/*.keys` | Table-driven tests |
| M6.2 | Wiring (chords, fork/join, `Enter` confirm), TextEdit, Command, Confirm. Core commands it is the first to issue: `Fork`, `Join`, `SetLabel`, `SetRel`, `SetTunnel`, `SetMeta`, concept commands (FR-EDIT-12) and note commands (FR-EDIT-16) ([metamodel §13.7](spec/metamodel.md#137-implementation-notes-m15)). Overlays: Help, Diagnostics, Glossary, ConceptPicker, Search, ArrangePreview, ProposalReview, Outline. Jump history. | UX | `.keys` scenarios incl. the SR-22 cases | Every transition row tested |
| M6.3 | Selection-repair invariant; proptest over random input sequences | UX; TEST | generator | 10k cases green |
| M6.4 | `cargo xtask gen-keymap --check`: generates the keymap, Help and state diagrams from `TRANSITIONS`; conflict test including reserved browser chords | UX (keymap) | — | Generated docs match the committed docs; conflict test passes |

### M7: Web application

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M7.1 | Leptos CSR and Trunk scaffold; `Doc`/`EditorState` signals; bundled fixture | ARCH (state, dev setup) | dogfood | `trunk build --release`; bundle size reported (NFR-WASM) |
| M7.2 | SVG view with keyed `<For>` (key + fingerprint); retro `<pre>` view; `` ` `` toggle. Depends on M5.6. | ARCH (state); LAY | dogfood | DOM smoke test; DOM mutation test shows only changed elements update |
| M7.3 | Key adapter (`key` vs `code`, layouts, IME `isComposing`); mouse → FSM | UX (key notation) | `.keys` | `chromiumoxide` e2e: `O 3 I` renders the arrow; AZERTY normalization tests |
| M7.4 | Status bar, info line, diagnostics panel, `]`/`[`, debounced validation | UX | dogfood | Introducing an imbalance shows a diagnostic in under 100 ms (e2e) |
| M7.5 | Open/save: File System Access API on Chromium; `<input type=file>` plus download on Firefox and Safari | UX (browser support) | — | e2e round-trip in Chromium; degraded path tested |
| M7.6 | Autosave journal (IndexedDB), quota handling, `beforeunload` guard, restore prompt | UX (offline) | — | e2e: crash and restore recovers unsaved Commands |
| M7.7 | Search UI (`/`, `F3`/`Shift+F3`), hit highlighting, concept "where used", `:query` | UX (search) | dogfood (multi-diagram) | e2e: `/diagnostics` + `Enter` selects the Diagnostics arrow on A0; `F3` advances across diagrams |

### M8: Server

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M8.1 | Axum router; `WorkspaceStore`; ops with `rev`/`409` and Command-based rebase; single-writer lock (`.idef0/run/write.lock`); read-only mode for files that fail to parse | ARCH (persistence and concurrency) | — | `axum-test` integration; concurrent-ops test; a file that fails to parse is never overwritten |
| M8.2 | Diagnostics, facts and render endpoints | ARCH (API) | dogfood | Response snapshots |
| M8.3 | WebSocket events, `notify` watcher, client wiring (persist patches, live reload) | ARCH | — | An external edit propagates to the browser |
| M8.4 | `idef0 serve`: embeds `dist/`; security hardening FR-SEC-01..14 (Host allowlist, Origin check, session token, JSON-only, path confinement, CSP, body caps) | ARCH (server security) | — | Negative tests for every FR-SEC-01..14; fuzz gate from M2.6 green |
| M8.5 | Proposal queue (D15): `/api/proposals`; agent-origin ops return `202 {proposal_id}`; `idef0 proposals list/show/accept/reject` | AG (proposals); CLI | — | Agent write → pending → accept applies it; `auto_accept=false` by default |

### M9: PRD ingestion

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M9.1 | Anchors, attributes, section extent, frontmatter (FR-PRD) | ING (PRD tagging) | `fixtures/prd/` | `PrdIndex` snapshots |
| M9.2 | Fenced DSL fragments → proposals; REQ IDs; name, description and concept proposals; glossary sections | ING | `fixtures/prd/` | A glossary table yields concept proposals |
| M9.3 | `.idef0/trace.lock`, content hashes, `--ack`, `validate --locked`, `trace --matrix` | ING (drift); VAL (external facts) | `fixtures/prd/` | Editing a tagged section flips `stale_trace`; `--locked` fails on a stale lock |
| M9.4 | Fuzzing for the Markdown anchor and tag parsers | TEST (fuzzing) | corpus | Nightly fuzz run clean |

### M10: Code scanning, sync, contracts

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M10.1 | Walker, pre-filter, tree-sitter binding for Rust and Python; direct existence check for `code=` targets | ING (scanner) | `fixtures/repo/` | `CodeIndex` snapshot |
| M10.2 | TypeScript, Go and Java grammars; (path, mtime, size) fast path | ING | `fixtures/repo/` | Warm rescan of an unchanged repo does 0 hashes and 0 parses |
| M10.3 | SyncPlan: tag rewrites, `--keyify`, rebinding detection (`L-TAG-03`), moved-section detection | ING (sync) | `fixtures/repo/`, `fixtures/prd/` | Renumbering A2↔A3 gives correct diffs; key tags untouched |
| M10.4 | `scaffold`; `contract` (schema v1, untrusted-text fencing, size limits); contract API endpoint | AG (contract cards) | dogfood | Contract snapshot; fencing test |
| M10.5 | Server search index (tantivy) over PRD sections and code tags; `/api/search` merge. Depends on M10.1. | ING (server search) | `fixtures/repo/` | PRD and code hits link to boxes; ≤ 50 ms on the fixture repo |
| M10.6 | Import/export (FR-IO): JSON interchange, Markdown model book, `import`/`export` CLI | ING (FR-IO); CLI | dogfood | JSON round-trip; book snapshot |
| M10.7 | Dogfood: tag idef0-rs crates against the dogfood model; CI runs `idef0 validate --locked --fail-on error` on itself | — | dogfood | idef0-rs validates itself; metric reported |

### M11: AI proposals (opt-in)

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M11.1 | Privacy controls (`[ai] enabled=false` by default, exclude globs, redaction, `--show-context`) and the Anthropic client (server-side key) | AG (privacy) | — | Offline mode makes zero network calls (CI test); redaction test |
| M11.2 | Proposal pipeline: structured output with Rust-side checks, repair loop ≤ 3, quote-based citations, recorded responses | AG (proposals) | `fixtures/proposals/recorded/` (7 cases) | Recorded-response tests; a seeded imbalance is repaired |
| M11.3 | ProposalReview UI for all D15 proposals (AI and agent), with per-item accept/reject and provenance badges. Depends on M8.5. | UX (ProposalReview); AG | recorded | e2e: accept 2 of 4 boxes, then assert the model |

### M12: Agents, accessibility, release

| Epic | Deliverable | Spec | Fixtures | Acceptance |
|---|---|---|---|---|
| M12.1 | `idef0 mcp`: tools, pending proposals for writes, single-writer routing through the server | AG (MCP) | dogfood | MCP protocol tests; `apply_patch` creates a pending proposal; works via `claude mcp add idef0 -- idef0 mcp` |
| M12.2 | Accessibility: WCAG 2.2 AA, ARIA live region, Outline tree view, contrast and reduced motion | UX (accessibility) | dogfood | CDP accessibility-tree assertions: every box exposes name and description |
| M12.3 | Performance pass on the 60-diagram model; Worker decisions confirmed | ARCH (budgets); TEST | `fixtures/large/` | NFR-PERF budgets met |
| M12.4 | Journey suite: J1–J5 from the PRD as automated e2e/CLI scenarios | PRD §4; TEST | all | All five journeys green |
| M12.5 | User guide (keymap generated), CHANGELOG, release binaries via `dist` | ARCH (release) | — | Tagged release artifacts for the NFR-PORT targets |

## 3. Dependencies and critical path

| Epic | Depends on |
|---|---|
| M1.7 generator | M1.4, M1.5 (full validity after M3.3) |
| M3.x | M3.1 gate; M2.1 (`SpanMap` for SARIF) |
| M4.3–M4.7 | M4.0 spike |
| M7.2 | M4.5, M5.6 |
| M7.7 | M2.5, M6.2 |
| M8.4 | M2.6 (fuzz gate) |
| M9.1–M9.3 | M8.1 |
| M10.4 contract endpoint | M8.2, M10.1 |
| M10.5 server search | M10.1 |
| M11.x | M3.3, M8.5 |
| M11.3 | M8.5, M6.2 |
| M12.1 | M8.5 |
| M12.4 journeys | M10.7, M11.3, M12.1 |

**Critical path:** M0 → M1 → M2 → M3.1 ⚑ → M3 → M4.0 ⚑ → M4 → M5.6 → M6 → M7 → M8 → M10 → M11 → M12.

**Execution order (decided 2026-10-20): SVG first.** After M1.4 the work takes the shortest path to a viewable diagram, M2.1 → M4.0 ⚑ → M4.1–M4.5 (`idef0 render --svg`), then returns to M1.5–M1.7, M2.2–M2.6 and M3. Layout depends on none of those (table above).

**Parallel tracks:**
- M5 (except M5.6) once M4.5 is done.
- M9.1–M9.3 once M8.1 is done.
- M2.6 and M9.4 fuzzing anytime after their parsers exist.
