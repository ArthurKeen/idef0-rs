# idef0-rs: Test Strategy

| Field | Value |
|---|---|
| Status | DRAFT v0.2 (2026-10-15) |
| Part of | idef0-rs spec set; index: [docs/PRD.md](PRD.md) |
| Normative references | [architecture](architecture.md) (determinism contract, CI, budgets); [validation](spec/validation.md) (diagnostic registry, conformance); [layout](spec/layout.md) (geometry checker); [editor UX](spec/editor-ux.md) (FSM, keymap); [metamodel](spec/metamodel.md); [ingestion](spec/ingestion.md); [agents](spec/agents.md) |
| Scope | Test layers and tools, the random valid-model generator, golden snapshots and their review workflow, cross-target determinism, rule conformance, FSM scripts, end-to-end tests, benchmarks, fuzzing, the fixture inventory, and the policy for acceptance criteria. |

---

## 1. Scope

This document consolidates testing material that the v0.1 PRD scattered across the metamodel, Mangle, layout, FSM, ingestion, AI and milestone sections (SR-31). It is normative for **how** things are tested. The behavior under test is specified in the other documents.

Two rules hold everywhere:

- **Never weaken a determinism, golden or snapshot test to make it pass.** A snapshot changes only when the behavior change is intended, and the commit says why (§5.3).
- **Every acceptance criterion is automatable** (§11). Manual review produces artifacts; it never gates.

---

## 2. Strategy

| Layer | Tool | Scope | CI job ([architecture §15](architecture.md#15-ci-sr-32)) | Time budget |
|---|---|---|---|---|
| Unit | `cargo test` | Functions and modules in every crate; one test per row of normative tables (arrow typing, FSM transitions, sizing formulas) | `test-linux`, `test-macos`, `test-windows` | ≤ 3 min per OS |
| Property | `proptest` + the model generator (§3) | Invariants over random models, commands, inputs and Scenes (§3.4) | `test-linux` (PR depth), nightly (deep) | PR ≤ 4 min; nightly ≤ 60 min |
| Golden snapshots | `insta` | SVG, ASCII, facts, diagnostics (human, JSON, SARIF), contract cards, node index, help text, API responses | `test-linux` | included above |
| Cross-target determinism | `wasm-bindgen-test` in headless Chrome | Native goldens equal wasm32 output byte for byte (§4) | `wasm` | ≤ 6 min |
| Rule conformance | custom harness in `idef0-mangle/tests` | Every diagnostic code has a fixture; every backend (§10.1) | `test-linux` | ≤ 1 min |
| Geometry checker | `idef0_layout::check` | Zero `G-*` findings on every fixture and every generated model (§10.3) | `test-linux`, nightly | included above |
| FSM scenarios | `.keys` scripts | Key sequences against fixtures with expected models (§10.2) | `test-linux` | ≤ 30 s |
| API and security | `tower::ServiceExt::oneshot` (optionally `axum-test`) | Endpoints, `rev`/`409`/`423`, lock, FR-SEC-01..14 negative tests (§10.4) | `test-linux`, `test-windows` | ≤ 1 min |
| DOM smoke | `wasm-bindgen-test` (browser) | Leptos components mount, keyed rendering, `<pre>` toggle (§7.1) | `wasm` | included above |
| End to end | `chromiumoxide` over CDP, headless Chrome | User journeys through `idef0 serve`; accessibility tree (§7.2) | `e2e` | ≤ 10 min |
| Benchmarks (gate) | `gungraun` (formerly iai-callgrind), instruction counts | Hot paths vs committed baseline (§6) | `bench-gate` | ≤ 8 min |
| Benchmarks (informative) | `criterion`, wasm timing | Wall-clock budgets on the reference machine (§6) | nightly | ≤ 20 min |
| Fuzzing | `cargo-fuzz` (libFuzzer) | DSL parser, Markdown/PRD parser, code-tag parser, ops JSON (§8) | nightly (run); `test-linux` (corpus replay) | nightly 10 min per target |
| Recorded LLM | recorded JSON responses | Proposal pipeline, repair loop, citation checks; no network ([agents](spec/agents.md#57-testing)) | `test-linux` | ≤ 30 s |
| Dogfood | `idef0 validate` | The tool's own model and, from M10.7, its own code tags | `dogfood` | ≤ 3 min |

- The PR pipeline runs jobs in parallel; its wall-clock target is **≤ 20 minutes**.
- Every test is deterministic. Tests that need time use `FixedClock`; tests that need new keys use the deterministic minter ([architecture §4.6](architecture.md#46-injected-environment)). proptest uses a fixed seed source in CI (`PROPTEST_RNG_SEED` per job) and commits `proptest-regressions/` files.
- No test touches the network except the e2e job's loopback connection to `idef0 serve`. Tests that would call the Anthropic API use recorded responses and a transport that panics on real network use.

---

## 3. Random valid-model generator

Four acceptance criteria in the v0.1 roadmap assume "random valid models" (command sequences preserve validity, incremental search equals rebuild, zero `G-*` on 10k models, FSM random input). This section specifies the generator. It is **its own epic** (§12).

### 3.1 Location and API

- Module `idef0_core::testkit`, behind the non-default feature `testkit` (dependency: `proptest`). Other crates enable it in `[dev-dependencies]`. It never ships in the wasm release bundle.

```rust
// (illustrative) idef0_core::testkit
pub struct GenParams {
    pub profile: Profile,               // box limits 3..=7 or 3..=6
    pub max_depth: u8,                  // levels below A0 (default 3; large model 4)
    pub decompose_p: f32,               // probability a box is decomposed (default 0.4)
    pub arrows_per_box: RangeInclusive<u8>,
    pub feedback_p: f32, pub bundle_p: f32, pub tunnel_p: f32, pub call_p: f32,
    pub concepts: RangeInclusive<u16>,  pub ontology_depth: u8,
    pub models: u8,                     // >1 generates a workspace with cross-model calls
}
pub fn recipe(params: GenParams) -> impl Strategy<Value = Recipe>;   // shrinkable plan
pub fn build(recipe: &Recipe) -> Workspace;                          // deterministic, always valid
pub fn arb_model(params: GenParams) -> impl Strategy<Value = Model>;  // recipe(params).prop_map(build)
pub fn arb_commands(model: &Model, n: usize) -> impl Strategy<Value = Vec<Command>>;
pub fn arb_inputs(n: usize) -> impl Strategy<Value = Vec<Input>>;    // FSM inputs
```

### 3.2 Construction (valid by construction)

`build` runs top-down and repairs as it goes, so every recipe yields a valid model:

1. **Concepts first:** a concept DAG with `kind_of` and `part_of` edges (no cycles: parents are drawn only from earlier concepts), each with a non-empty name and a genus–differentia description.
2. **Tree shape:** A-0 with one box (number 0), the A0 diagram with 3..max boxes, and each box decomposed with `decompose_p` down to `max_depth`. Names are verb + noun from a fixed vocabulary, unique among siblings; descriptions are non-empty.
3. **Wiring per diagram:** every box gets at least one control and one output. Outputs connect to later boxes' inputs or controls (forward flow) and, with `feedback_p`, to earlier boxes (feedback). Every arrow carries a concept.
4. **Balance:** a child diagram's boundary ports are derived from the parent box's ICOMs, so balance holds by construction. With `tunnel_p`, a parent arrow is tunneled at the box end (omitted on the child), or a child boundary arrow is tunneled at the border (`[I1]`, no ICOM code).
5. **Bundles:** with `bundle_p`, an arrow whose concept has ontology children is forked or joined, with `rel` matching the ontology (`all`, `kind_of` or `part_of`, never mixed on one bundle).
6. **Calls:** with `call_p`, an undecomposed box gets a call arrow targeting, by key, a box outside its own subtree. Targets are chosen in a fixed order that keeps the call graph acyclic. With `models > 1`, some targets are in another model (`SEC/@key`).
7. **Higher context:** optionally an A-1 diagram with 3..max boxes, one of which is box 0.

**Validity oracle:** every generated model passes the type check, all lints at error severity, and Mangle with zero error-severity violations under its profile. A dedicated property test asserts this on every case; a failure is a generator bug or a rule bug, and both block.

### 3.3 Shrinking

- proptest shrinks the **recipe**, not the model. A recipe is a small tree of decisions (diagram count, per-diagram box counts, arrow choices as indices, feature flags). Shrinking it removes diagrams, boxes, arrows and features.
- Because `build` repairs (re-adds a missing control or output, drops arrows whose endpoints vanished), every shrunk recipe is still valid. The minimal failing case is therefore a small valid model, written to the failure message as DSL.
- The same `Recipe` implements `arbitrary::Arbitrary`, so fuzz targets (§8) can produce valid models from raw bytes.

### 3.4 Properties

| Property | Crate | PR cases | Nightly cases |
|---|---|---|---|
| `NodeId`: `parse(display(n)) == n`; dotted forms normalize (`A21.3 → A213`, `A0.k → Ak`) | core | 1,000 | 100,000 |
| Generator output is valid (oracle above) | core + mangle | 256 | 10,000 |
| `format(parse(format(m))) == format(m)`; `parse(format(m))` equals `m` semantically; comments survive a format | core | 256 | 10,000 |
| Random command sequences keep the model valid or return `CommandError` without change; `undo ∘ do = id`; concept rename relabels every carrying arrow | core | 256 | 10,000 |
| Rebase: re-applying pending commands onto a concurrently changed model either applies or reports per command, never panics, and the result is valid | core | 128 | 5,000 |
| Incremental search index after each patch equals a full rebuild | core | 128 | 5,000 |
| FSM: 200-input sequences never panic; the selection invariant holds; `undo^k ∘ do^k = id` | core | 256 | 10,000 |
| Facts are sorted and stable across 100 repeated extractions | mangle | 64 | 1,000 |
| Backends agree (`MangleRsEngine` vs `MiniEngine`) while both exist | mangle | 128 | 5,000 |
| Layout: zero `G-*` findings; all polylines orthogonal; no segment enters a box interior; strict staircase; label overlap absent **or** `L-LABEL-FIT` emitted ([layout](spec/layout.md#11-labels-and-squiggles)) | layout | 256 | 10,000 |
| Layout and SVG are identical across 100 repeated runs | layout | 32 | 1,000 |

---

## 4. Cross-target determinism

D6 requires byte-identical output on native and wasm32 ([architecture §2.2](architecture.md#22-determinism-contract)).

| Artifact | Golden (native, insta) | Checked on wasm32 |
|---|---|---|
| Fact dump | `fixtures/golden/facts/<fixture>.snap` | yes |
| Validation report (JSON) | `fixtures/golden/report/<fixture>.snap` | yes |
| Scene (JSON) per diagram | `fixtures/golden/scene/<fixture>/<node>.snap` | yes |
| SVG per diagram | `fixtures/golden/svg/<fixture>/<node>.snap` | yes |
| ASCII per diagram (both charsets) | `fixtures/golden/ascii/<fixture>/<node>.<charset>.snap` | yes |
| Search ranking for fixed queries | `fixtures/golden/search/<fixture>.snap` | yes |
| PNG | — | no (excluded from D6) |

- **Native side:** an insta test per artifact writes `.snap` files under `fixtures/golden/` (`insta::with_settings!` with `snapshot_path`).
- **wasm side:** a `wasm-bindgen-test` suite (`run_in_browser`) embeds the same fixtures and `.snap` files with `include_str!`, strips the insta YAML header, recomputes each artifact in the browser, and compares bytes. A mismatch prints the first differing line and the fixture name.
- **Regenerating:** goldens are only ever regenerated natively (`cargo insta test -p <crate> --review`). The wasm suite then must pass unchanged. A change that passes natively but fails on wasm is a determinism bug, not a reason to fork the golden.
- The fixture list is `fixtures/valid/*`, `fixtures/dogfood/*`, `fixtures/reference/*` and 32 generator seeds committed as DSL under `fixtures/generated/` (§9).

---

## 5. Golden snapshots and review workflow

### 5.1 What is snapshotted

| Kind | Location | Redactions |
|---|---|---|
| SVG and ASCII renders | `fixtures/golden/{svg,ascii}/` | none |
| Facts, Scene, report JSON | `fixtures/golden/{facts,scene,report}/` | none |
| SARIF | `crates/idef0-mangle/tests/snapshots/` | tool version |
| Contract cards (md, json) | `crates/idef0-server/tests/snapshots/` (moves with M10.4) | none |
| Parse error diagnostics | `crates/idef0-core/tests/snapshots/` | none |
| Help and hint text from `TRANSITIONS` | `crates/idef0-core/tests/snapshots/` | none |
| API responses | `crates/idef0-server/tests/snapshots/` | `rev`, timestamps |
| CLI output (human and JSON) | `crates/idef0-cli/tests/snapshots/` | paths, version |

Redactions exist only for values that are legitimately variable (versions, temp paths). Model-derived content is never redacted.

- **PNG goldens** (native only) are compared perceptually, not byte for byte: at most 0.5 % of pixels may differ by more than 8/255 in any channel. PNG is excluded from byte determinism (D6).

### 5.2 CI behavior

- CI runs `cargo insta test --check --unreferenced=reject` (via `INSTA_UPDATE=no`): a changed or orphaned snapshot fails the job.
- A **golden report** step runs when any `.snap` under `fixtures/golden/svg/` changes. It builds a static HTML page (uploaded as a workflow artifact and linked from a PR comment) showing, for each changed diagram, old SVG, new SVG and a PNG pixel diff (rasterized with `resvg`; informative only). It also always shows the `classic` render of `fixtures/reference/factory-a0.idef0` side by side with `docs/research/examples/factory-a0.png`.
- The page is a review aid. The pass criteria for the factory fixture are the structural assertions in [layout §classic theme](spec/layout.md#132-classic-conventions-reference-image) (port order, label sides, monotone control-label y, zero bends, shadow on A0), which run as ordinary tests.

### 5.3 Approving changes

1. The author runs `cargo insta review`, accepts only intended changes, and commits the `.snap` files in the same commit as the code.
2. The commit message contains a line `snapshot: <what changed and why>`.
3. `CODEOWNERS` assigns `fixtures/**` and `**/snapshots/**` to the maintainers; at least one maintainer approval is required, after viewing the golden report for SVG changes.
4. Baseline files (`benches/baseline/`, `fixtures/layout/baselines/`) follow the same rule with a `baseline:` line (§6).

---

## 6. Benchmarks and performance gates

Wall-clock timing on shared CI runners is too noisy to gate on (SR-31, SR-54). Gates use instruction counts; wall-clock is measured and reported, not enforced per PR.

| Benchmark | Gate (gungraun, PR) | Informative (criterion / wasm, nightly) | Budget ([architecture §11](architecture.md#11-performance-budgets)) |
|---|---|---|---|
| Parse + format dogfood and `fixtures/large/` | Ir regression ≤ 3 % | yes | — |
| `apply` of each command kind on `fixtures/large/` | Ir regression ≤ 3 % | yes | part of NFR-PERF-01 |
| Fact extraction + validation, `fixtures/large/` | Ir regression ≤ 3 % | native + wasm | NFR-PERF-02 |
| Layout per benchmark diagram (`fixtures/bench/`) | Ir regression ≤ 3 % | native + wasm | NFR-PERF-04 |
| SVG + ASCII render | Ir regression ≤ 3 % | yes | part of NFR-PERF-01 |
| Search query and incremental update, `fixtures/large/` | Ir regression ≤ 3 % | native + wasm | NFR-PERF-03 |
| Code scan of `fixtures/repo/` (cold and warm) | Ir regression ≤ 5 % | yes; 100k-file synthetic repo nightly | NFR-PERF-05 |
| Keystroke → re-render | — | e2e trace in Chrome (Performance domain) | NFR-PERF-01 |
| Bundle size | hard limit 2.5 MB gzip | — | NFR-WASM-01 |

- **Baseline:** instruction counts are committed in `benches/baseline/` (one file per bench target). The `bench-gate` job compares against it with gungraun's regression limits. Exact flag names are verified at M0.3.
- **Updating the baseline** requires a commit with a `baseline:` justification line (for example, "new rule adds 4 % to validation; accepted") and maintainer approval. Improvements are recorded the same way, so later regressions are measured from the better number.
- **Wall-clock budgets** are checked nightly on the reference machine ([architecture §15.3](architecture.md#153-caching-and-reference-machine)). A breach opens an issue; at a milestone boundary it must be fixed or renegotiated in an ADR.
- The wasm validation budget (NFR-PERF-02) is first measured in the M3.1 spike; the result sets the Worker default ([architecture §5.3](architecture.md#53-client-validation-and-the-web-worker)).

---

## 7. DOM smoke and end-to-end tests

### 7.1 DOM smoke (wasm-bindgen-test, in browser)

- Mounts the app with a bundled fixture; asserts the number of `[data-key]` elements per kind, that the `` ` `` toggle swaps SVG for `<pre>`, and that the status line shows the focused element's name.
- **Keyed-rendering test (SR-52):** with a `MutationObserver`, renaming one box replaces only the DOM nodes of that box, its label and its attached arrows; all other `[data-key]` nodes keep identity.

### 7.2 End to end (chromiumoxide)

- Rust test binary `crates/idef0-server/tests/e2e/` (feature `e2e`), driving headless Chrome over CDP against `idef0 serve` on a temp copy of a fixture workspace. No JavaScript test tooling.
- Each PRD user journey ([PRD §journeys](PRD.md#4-user-journeys)) has one e2e test (`journey_<n>_<name>.rs`). Additional scenarios cover the v0.1 milestone checks: typing `O 3 I` renders the arrow; on the dogfood fixture, `/diagnostics` + `Enter` selects the Diagnostics arrow on A0, and `F3` advances to the next hit across diagrams (roadmap M7.7); accepting 2 of 4 proposed boxes (recorded proposal); an external file edit propagates to the browser; a broken file on disk shows the error view and is not overwritten.
- **Accessibility assertions:** `Accessibility.getFullAXTree` from Rust. Every box exposes its name and description; the ARIA live region announces mode and selection changes; the overlay in focus traps focus. WCAG targets are owned by [editor UX §accessibility](spec/editor-ux.md#10-accessibility).
- **Flake policy:** e2e waits on DOM conditions, never fixed sleeps; timeouts are generous (5 s) and latency is not asserted in PR runs. A test that flakes twice in a week is quarantined with an issue and fixed within the milestone.

---

## 8. Fuzzing

Untrusted bytes reach several parsers (model files from PRs, PRD Markdown, code comments, API bodies). Fuzzing is part of the security posture ([architecture §8](architecture.md#8-server-security-sr-24)) and is **its own epic** (§12).

| Target | Input | Oracle |
|---|---|---|
| `dsl_parse` | arbitrary bytes | no panic, no hang (> 1 s), memory within limits; `E-LIMIT-01` on oversized input |
| `dsl_roundtrip` | `Recipe` via `Arbitrary` (§3.3) | `format(parse(format(m))) == format(m)` |
| `prd_parse` | arbitrary Markdown | no panic; anchors and spans within the input |
| `tag_parse` | arbitrary comment text in each language | no panic; extracted refs well-formed |
| `ops_json` | arbitrary JSON for `POST /ops` | deserialize-or-error; `apply` never panics |
| `mangle_facts` | generated fact sets | evaluation terminates within caps ([validation §project rules](spec/validation.md#9-project-rules)) |

- Targets live in `fuzz/` (cargo-fuzz, pinned nightly). Corpora are committed in `fuzz/corpus/<target>/`, seeded from all fixtures.
- **Corpus replay on stable:** each target's body is a plain function in a test-support module, and a stable unit test in `test-linux` runs it over the committed corpus and every past crash in `fuzz/artifacts-regressions/`. Nightly runs (10 min per target) add new interesting inputs by PR.

---

## 9. Fixture inventory

| Path | Purpose | Owning epic |
|---|---|---|
| `fixtures/valid/*.idef0` | Small valid models covering each construct (bundles, tunnels incl. `[I1]`, calls, A-1, notes, both profiles) | M2.1 (grows per epic) |
| `fixtures/invalid/<code>/*.idef0` (+ `expect.toml`) | One or more minimal fixtures per DSL-reachable `E-*`, `L-*` and `R-*` code (§10.1) | M1.3, M2.3, M3.3 |
| `fixtures/facts/<code>/*.mg` | Fact-level fixtures for codes unreachable from the DSL ([validation](spec/validation.md#12-conformance-fixtures)) | M3.3 |
| `fixtures/dogfood/idef0-rs.idef0` | idef0-rs modeled in itself | M2.4, M10.7 |
| `fixtures/reference/factory-a0.idef0` | *Develop/Update a Factory* A-0, `classic` theme reference | M5 (classic theme epic); used by search goldens from M2.5 |
| `fixtures/workspace/` | Multi-model workspace (`ORD`, `SEC`) for cross-model calls, Used At and workspace index | M1.6 |
| `fixtures/generated/seed-<n>.idef0` | 32 committed generator outputs for goldens and cross-target tests | M1.7 (generator) |
| `fixtures/large/large-60.idef0` | 60-diagram generated model (seed `0x1DEF0`, ~2k elements, 1k concepts) for budgets | M1.7 (generator) |
| `fixtures/bench/*.idef0` | Benchmark diagrams for layout budgets and crossings (dense 7×40, Auto-Arrange worst case) | M4.0 |
| `fixtures/layout/templates/*.idef0` | One diagram per routing template class, with a `key=probe` arrow; snapshots of routes, wired polylines and tracks | M4.3, M4.4 |
| `fixtures/layout/baselines/crossings.json` | Committed crossing counts per benchmark diagram | M4.4 |
| `fixtures/arrange/*.idef0` | Deliberately mis-ordered diagrams with known optimum | M4.7 |
| `fixtures/fsm/*.keys` (+ expected `.idef0`) | Key-sequence scenarios (§10.2) | M6.2 |
| `fixtures/prd/*.md` | Tagged Markdown samples: anchors, frontmatter, fenced `idef0` blocks, glossary tables, malformed tags | M9.1, M9.2 |
| `fixtures/repo/` | Small multi-language tagged repo (Rust, Python, TypeScript, Go, Java) with renamed and stale tags | M10.1, M10.2 |
| `fixtures/proposals/recorded/*.json` | Recorded LLM responses incl. a seeded imbalance and its repairs | M11.1, M11.2 |
| `fixtures/golden/**` | Native goldens shared with the wasm suite (§4) | M3.5, M4.5 |
| `fuzz/corpus/<target>/` | Fuzz corpora | fuzzing epic |
| `benches/baseline/` | Instruction-count baselines | M0.3 / perf epic |

- The large model is generated, then committed, so tests do not depend on generator changes. `cargo run -p idef0-core --features testkit --example gen-large -- --seed 0x1DEF0` regenerates it; a test asserts that the committed file equals the generator's output and fails with instructions if not.
- Oversized inputs for `E-LIMIT-01` tests are generated in the test, not committed.

---

## 10. Conformance, FSM scripts, geometry and API tests

### 10.1 Rule conformance policy (SR-15)

For each `fixtures/invalid/<code>/<case>.idef0`, on every enabled backend:

1. The report **contains** at least one diagnostic with `<code>` (and the subject in `expect.toml`, if given).
2. It contains **no other error-severity diagnostic** except codes in the fixture's own `allow = [...]` list in `expect.toml`, each entry with a one-line justification. There is no global allowlist ([validation FR-VAL-20](spec/validation.md#12-conformance-fixtures)).
3. `fixtures/valid/*` yield zero errors; warnings are allowed only if listed in the fixture's `expect.toml`.

```toml
# fixtures/invalid/icom_imbalance/missing-child-port.expect.toml   (illustrative)
code    = "icom_imbalance"
subject = { kind = "box", id = "A1" }   # icom_imbalance has subject kind box; the detail names A1/I2
profile = "extended"
allow   = []                          # co-firing codes, each with a justification comment
```

- Codes that the DSL cannot produce (rejected earlier by the type check, or true by construction) are tested with **fact-level fixtures**: `fixtures/facts/<code>/*.mg` contain EDB facts loaded directly into the engine. A code's fixture kind follows from where its fixtures live (`fixtures/invalid/` or `fixtures/facts/`); the registry has no separate column for it.
- A **coverage test** fails if any `E-*`, `L-*` or `R-*` code in the registry has no fixture. `G-*` codes cannot come from a valid layout, so each has a unit test with a hand-built bad `Scene` in `idef0-layout/tests/checker/`.
- Profile-dependent codes have one fixture per profile where severity differs.

### 10.2 FSM `.keys` scripts

```
# fixtures/fsm/wire-output-to-input.keys                            (illustrative)
model:   fixtures/valid/three-boxes.idef0
diagram: A0
keys:    O 3 I Enter
expect:  wire-output-to-input.expected.idef0
mode:    BoxNav
```

- Key notation follows [editor UX §keymap](spec/editor-ux.md#5-canonical-keymap) (capital letter = unshifted key; `Shift+X` for shifted). The runner feeds `Input`s to `editor::step` headlessly and compares the formatted model and final mode.
- Table-driven tests generate one test per row of `TRANSITIONS`, plus the key-conflict test (no chord bound twice in a mode; no global key active in a text-entry state).

### 10.3 Geometry checker

`idef0_layout::check` runs on every Scene produced in any test: fixtures, goldens, generator cases and benchmarks. Any `G-*` finding fails the test, because it is a layout-engine bug ([layout §geometry checker](spec/layout.md#15-geometry-checker-g-)).

### 10.4 API, persistence and security tests

| Area | Required tests |
|---|---|
| Concurrency | `409` on stale `base_rev`; two clients' rebase; external edit between read and write aborts the write |
| Parse failure | broken file → `423` on ops, `ModelError` on WS, file bytes unchanged; recovery after fix |
| Single writer | second `idef0 serve` refuses a live lock; stale lock takeover; CLI write routes through the server |
| FR-SEC-01..14 | one negative test each: foreign `Host`, missing or foreign `Origin`, missing or wrong token (HTTP and WS subprotocol), `text/plain` body, `../` and symlink escape, CORS absent without `--dev`, raw HTML in a description rendered inert, oversized body |
| Agent origin | `origin=agent` ops create a proposal and do not change the file (D15) |

---

## 11. Acceptance-criteria policy (SR-37)

Every epic acceptance criterion names the test, fixture or CI job that proves it. Criteria that need human judgment produce an artifact for review but never gate. The v0.1 criteria below are restated.

| v0.1 criterion | Problem | Replacement |
|---|---|---|
| M7.2 "manual demo" | not automatable | DOM smoke test (§7.1): app mounts the dogfood fixture, expected `[data-key]` counts, `<pre>` toggle |
| M4.4 "≤ recorded baseline" | nobody records it | `fixtures/layout/baselines/crossings.json`, committed; changed only with a `baseline:` justification and maintainer approval |
| NFR-PERF-01 "2023 laptop" | undefined machine | reference CI machine ([architecture §15.3](architecture.md#153-caching-and-reference-machine)) for wall-clock (nightly); gungraun instruction counts gate per PR |
| M1.2 "fails to type-check" with `String` names | a `String` can be empty | names are `NonEmptyString`; a `compile_fail` doctest shows construction without a name does not compile; deserializing `""` is a typed error |
| M3.3 "yields exactly that violation" | unattainable (co-firing, unreachable codes) | §10.1: contains the code, no other error outside the registry allowlist; fact-level fixtures |
| M5.4b "side-by-side review page" | no pass criterion | structural assertions are the gate; the page is a review artifact (§5.2) |
| M0.4 "consistent with this PRD" | not checkable | link-check test: every relative link in `CLAUDE.md` and `docs/` resolves; CLAUDE.md lists every crate in the workspace (test reads `Cargo.toml`) |
| M4.6 "10k random models" | too slow per PR | 256 cases per PR, 10,000 nightly; failing seeds committed to `proptest-regressions/` |
| M7.4 "diagnostic in under 100 ms" | timing in e2e is flaky | PR: e2e asserts the diagnostic appears (5 s timeout); nightly: latency measured against NFR-PERF-07 on the reference machine |
| M10.2 "rescan does no parses" | needs an observable | `ScanStats.parsed == 0` asserted in the test |
| M9.4 "≤ 50 ms on the fixture repo" | wall-clock gate | instruction-count gate per PR; wall-clock nightly |
| M12.1 "works when added to Claude Code" | manual | scripted stdio JSON-RPC session against a recorded transcript (MCP conformance test); manual check is a non-gating release note |
| M12.3 "budgets met" | which ones, where | the NFR-PERF rows in [architecture §11](architecture.md#11-performance-budgets) on `fixtures/large/`, measured nightly; Worker decision per §5.3 there |

---

## 12. Test-infrastructure epics (roadmap input)

The lead owns [docs/roadmap.md](roadmap.md). These epics are proposed so the criteria above have something to run on.

| Epic | Deliverable | Depends on | Needed by |
|---|---|---|---|
| Random valid-model generator (roadmap M1.7) | `idef0_core::testkit` (§3), validity oracle test, `fixtures/generated/`, `fixtures/large/` | M1.4 (identity), M1.5 (commands); oracle needs M3.3 | M1.5 properties, M2.5, M4.6, M6.3, M12.3, cross-target tests |
| Golden infrastructure | `fixtures/golden/` layout, wasm comparison suite (§4), golden report page (§5.2), CODEOWNERS | M0.3, M3.2 | M3.5, M4.5, M5.x |
| Fuzzing | `fuzz/` targets (§8), corpora, stable corpus replay, nightly job | M2.1 (DSL), M9.1 (PRD), M10.1 (tags) | M8.4 security sign-off |
| CI base | `lint`, `test-linux`, `wasm` build, `cargo deny`, dependency audit, clippy determinism config | M0.2 | everything |
| CI cross-OS and MSRV | `test-macos`, `test-windows` (MSVC + tree-sitter), `msrv` | M0.3, M10.1 for ingest | M12.4 release |
| CI browser | `wasm-bindgen-test` headless Chrome, `trunk build` + bundle size, `e2e` job | M7.1, M8.4 | M7.3, M7.6, M11.3, M12.2 |
| Perf gates | gungraun benches + baseline, nightly criterion and wasm timing on the reference machine | M4.5, M1.7 (generator) | M5.7 (replaces wall-clock gate), M12.3 |
