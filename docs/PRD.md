# idef0-rs: Product Requirements (Index)

| Field | Value |
|---|---|
| Status | DRAFT v0.2 (2026-10-15). Split from the [v0.1 monolith](archive/PRD-v0.1.md) after the [spec review](reviews/2026-10-15-spec-review.md). |
| Owner | Arthur Keen |
| Audience | Engineers and Claude Code sessions implementing idef0-rs |
| Authority | [NORTHSTAR](NORTHSTAR.md) > this PRD and the spec set > [CLAUDE.md](../CLAUDE.md) (D13) |
| Normative reference | **FIPS PUB 183**, *Integration Definition for Function Modeling (IDEF0)*, NIST, 21 Dec 1993 ([local copy](research/fipspub183.pdf)). Citations: "FIPS §3.3.3-8" means section 3.3.3, rule 8. Annexes A–C are *informative*. |
| Informative references | [Wikipedia: IDEF0](https://en.wikipedia.org/wiki/IDEF0). A. Presley & D. Liles, *The Use of IDEF0 for the Design and Specification of Methodologies*, UT Arlington ARRI (not redistributed here: its copyright stays with the authors). Mangle docs (mangle.readthedocs.io); `mangle-rs` (codeberg.org/TauCeti/mangle-rs). |
| Standards status | Per Wikipedia (not verifiable from repo sources), FIPS 183 was withdrawn as a federal standard in 2008. The standards based on it are IEEE Std 1320.1-1998 and ISO/IEC/IEEE 31320-1. FIPS 183 is freely available and is this project's baseline (D11). Diffing against 31320-1 is open question Q-11. |

This document is the **entry point**. It states the problem, the users, the journeys, the scope, the decisions and the risks, and it indexes every requirement family to the spec that defines it. Requirement IDs are stable across documents.

---

## 1. Document map

| Document | Defines |
|---|---|
| [NORTHSTAR.md](NORTHSTAR.md) | Why: north star, principles, decision filters, metric |
| **PRD.md** (this) | What and for whom: journeys, scope, requirement index, NFR summary, decisions, risks |
| [glossary.md](glossary.md) | Every term (ICOM, DRE, C-number, node vs box number, leaf, concept…) |
| [spec/metamodel.md](spec/metamodel.md) | IDEF0 metamodel, identity, DSL grammar, editing semantics, persistence, FIPS conformance matrix |
| [spec/validation.md](spec/validation.md) | Fact extraction, Mangle schema and rules, profiles, diagnostic registry, contract-complete metric |
| [spec/layout.md](spec/layout.md) | FIPS-derived auto layout, routing, Auto-Arrange, themes, renderers, geometry checker |
| [spec/editor-ux.md](spec/editor-ux.md) | Keyboard FSM, canonical keymap, search, accessibility, browser support, offline |
| [spec/ingestion.md](spec/ingestion.md) | PRD and code tagging, scanner, sync, watch, server search index, import/export |
| [spec/agents.md](spec/agents.md) | Contract cards, scaffolds, AI proposals, MCP, threat model and privacy |
| [architecture.md](architecture.md) | Crates, types, state, persistence and concurrency, API, server security, dependencies, CI, release |
| [testing.md](testing.md) | Test strategy, fixture inventory, golden workflow, benchmarks, fuzzing |
| [cli.md](cli.md) | Every `idef0` command, flag and exit code |
| [roadmap.md](roadmap.md) | Milestones and epics, with spec sections and fixtures per epic |

### Reading guide per milestone

| Milestone | Read |
|---|---|
| M0 Foundation | architecture (dependencies, CI, lints), testing (strategy) |
| M1–M2 Core and DSL | metamodel, glossary, validation (lint registry), testing (model generator) |
| M3 Rules | validation, metamodel (identity, extraction inputs) |
| M4–M5 Layout | layout, metamodel (ports, ranks), testing (goldens, geometry checker) |
| M6–M7 Editor and web app | editor-ux, layout (themes), architecture (state, keyed rendering) |
| M8 Server | architecture (API, security, concurrency) |
| M9–M10 Ingestion and sync | ingestion, agents (contract cards), validation (ExternalFacts) |
| M11–M12 AI and agents | agents, architecture (security), editor-ux (proposal review) |

---

## 2. Problem and vision

LLM coding agents write plausible code but lose track of **compositional interface contracts**: what each unit consumes, what governs it, what it must produce, and what implements it. Requirements documents state those contracts loosely, and code states them implicitly.

IDEF0 grew out of Douglas T. Ross's SADT. The USAF ICAM program adopted it and NIST standardized it as FIPS 183. It states such contracts precisely, recursively and checkably through ICOMs (Input, Control, Output, Mechanism) and parent/child balance.

**idef0-rs** is an AI-first, keyboard-fast ("retro-mod") IDEF0 tool in the lineage of KBSI's DOS tool and AI0Win. It:

1. lets analysts author FIPS-conformant models at DOS speed, with automatic standard-driven layout and modern, classic or ASCII rendering;
2. validates models deductively with Mangle, using the same rules in the browser, CLI and CI;
3. binds every leaf function to the requirements that justify it and the code that implements it;
4. gives coding agents precise **contract cards** (CLI, HTTP, MCP);
5. proposes decompositions from requirement text. A human accepts each one.

## 3. Personas

| Persona | Jobs |
|---|---|
| **Analyst / architect** | Decompose systems; keep levels coherent; publish diagrams; review proposals |
| **AI coding agent** | Fetch a contract card; implement a leaf; tag code; validate; propose (never silently apply) model changes |
| **Reviewer / CI** | Gate PRs on rule violations, traceability gaps and drift; read SARIF; watch the contract-complete ratio |

## 4. User journeys

Each journey is end-to-end acceptance for v1, automated as an e2e or CLI scenario ([testing](testing.md)).

**J1. Analyst models a new system**
1. `idef0 init` creates `.idef0/` and a starter model with the A-0 context diagram only (box 0, purpose/viewpoint placeholders).
2. The analyst tags PRD sections (`<!-- @idef0:A1 -->`) and runs `idef0 propose A0`, which proposes the A0 decomposition (A0 is still a leaf at this point).
3. They review the ghost proposal in the UI and accept 4 of 5 boxes.
4. They refine at the keyboard (`O 3 I`, `PgDn` into A2, describe boxes with `E`).
5. Auto-Arrange (`=`) removes a feedback arc. Validation is clean.
6. They raise status to `recommended` and export SVG plus the Markdown model book.

FRs: FR-NAME, FR-CALL, FR-LAYOUT, FR-SEARCH, FR-IO; agents §proposals.

**J2. Agent implements a leaf**
1. Claude Code runs `idef0 contract A21` (or MCP `get_contract`).
2. It runs `idef0 scaffold A21 --lang rust` and implements the stub.
3. It tags the code with `@implements idef0:@validate-order` and runs `idef0 validate` until clean.
4. It opens a PR.
5. Any model change it wants is submitted as a **pending proposal** (D15).

FRs: FR-CONTRACT, FR-METRIC.

**J3. CI blocks drift**
1. A PR edits a tagged PRD section and changes a tagged function's signature.
2. CI `idef0 validate --format sarif --fail-on error` reports `stale_trace` and `L-SIG-DRIFT` on the PR diff.
3. The author updates the box description and runs `idef0 sync --ack`, which records the acknowledgement in `.idef0/trace.lock`.
4. CI goes green.

FRs: R-TRC, ingestion §drift.

**J4. Renumbering is safe**
1. The analyst moves box 2 after box 3 (`Shift+→`).
2. Node numbers in the subtree change (A2xx ↔ A3xx). `idef0 sync` proposes tag rewrites for node-number tags.
3. Key-based tags and call targets are unaffected (D14). Balance stays clean.

FRs: FR-EDIT, FR-ID.

**J5. Reviewer publishes**
1. A reviewer runs `idef0 validate --profile fips183` on a `recommended` model and gets strict FIPS output.
2. They export publication-form diagrams: working fields dropped, Used At copied into a model note, glossary alphabetical.

FRs: conformance matrix, FR-IO.

## 5. Scope (v1)

**In scope:**
- full FIPS 183 graphic syntax and semantics, except the deviations and deferrals listed below
- named and described functions and concepts
- call arrows
- Mangle validation with the `extended` and `fips183` profiles
- FIPS-derived auto layout and Auto-Arrange
- keyboard FSM
- modern, classic, fips183 and retro themes
- full-text search
- PRD and code traceability with drift detection
- contract cards and scaffolds
- AI proposals (opt-in)
- MCP server (writes become proposals)
- export to SVG, PNG, ASCII and Markdown, including text and glossary pages

**Out of scope or deferred:**

| Item | Status |
|---|---|
| Off-diagonal boxes (FIPS §3.3.4.1 permits them) | **[Deviation]**: unsupported |
| FEO sheets | v2 |
| Kit-cycle review workflow (reader notes are stored, not routed) | v2 |
| Legacy AI0Win/BPwin/IDL import | v2 (Q-10) |
| Real-time multi-user co-editing | v2+ |
| IDEF1X/IDEF3 | Not planned |
| Hosted SaaS | Not planned |
| Pixel-exact print forms | v2 |
| Layout of higher context diagrams (A-1…) | Stretch. They are parsed and validated in v1. |

## 6. Success metric

**FR-METRIC-01 (contract-complete leaf ratio).** `idef0 validate` computes contract-complete leaves ÷ leaves with CI severities. It reports the ratio in its JSON output and in SARIF `run.properties`. The definition is in [NORTHSTAR](NORTHSTAR.md#the-north-star) and the rule is in [validation](spec/validation.md). There is no telemetry; trend data comes from CI artifacts.

## 7. Requirement index

| Family | Topic | Defined in |
|---|---|---|
| FR-META, FR-ID, FR-DIAG, FR-BOX | Model metadata, identity, diagrams, boxes | [metamodel](spec/metamodel.md) |
| FR-ARROW, FR-BUN, FR-TUN, FR-NOTE, FR-BAL | Arrows, bundles, tunnels, notes, balance | [metamodel](spec/metamodel.md) |
| FR-NAME, FR-CALL | Names, descriptions, concepts; call arrows (key-based) | [metamodel](spec/metamodel.md) |
| FR-EDIT, FR-WS, FR-XM | Editing semantics, workspace and persistence, cross-model | [metamodel](spec/metamodel.md) |
| FR-FACT, FR-VAL, FR-METRIC; R-* | Fact extraction, validation pipeline, metric, Mangle rules | [validation](spec/validation.md) |
| E-*, L-*, R-*, G-* | Single diagnostic code registry | [validation §7](spec/validation.md#7-diagnostic-registry) |
| FR-LAYOUT, FR-STYLE | Auto layout, themes | [layout](spec/layout.md) |
| FR-UX, FR-SEARCH, FR-A11Y, FR-WEB, FR-I18N | FSM and keymap, search, accessibility, browser and offline, i18n | [editor-ux](spec/editor-ux.md) |
| FR-PRD, FR-TAG, FR-SCAN, FR-REN, FR-WATCH, FR-IO | PRD tagging, code tags, scanner, rename/sync, watch, import/export | [ingestion](spec/ingestion.md) |
| FR-CONTRACT, FR-AI, FR-MCP, FR-SEC-20..32 | Contract cards, AI proposals, MCP, agent threat model and privacy | [agents](spec/agents.md) |
| FR-SEC-01..19; NFR-DET, NFR-PERF, NFR-WASM, NFR-OBS, NFR-SCALE, NFR-PORT, NFR-LIC | Server hardening; non-functional requirements | [architecture](architecture.md) |
| CLI commands, flags, exit codes | — | [cli](cli.md) |

## 8. Non-functional requirements (summary)

Each row's full definition lives in the owning document named in the last column.

| ID | Requirement | Defined in |
|---|---|---|
| NFR-DET-01 | Byte-identical facts, diagnostics, Scene and SVG for identical inputs, native and wasm32 (PNG excluded) | [architecture §2.2](architecture.md#22-determinism-contract) |
| NFR-PERF-* | Keystroke to render ≤ 16 ms p95; structural validation and search budgets confirmed by the M3.1 spike; all measured on a reference CI machine with instruction-count gates | [architecture §11](architecture.md#11-performance-budgets) |
| NFR-SEC-* | Localhost hardening (Host/Origin checks, session token); sanitized Markdown; sandboxed project rules; LLM egress opt-in; agent writes need acceptance | [architecture](architecture.md#81-requirements) FR-SEC-01..19 + [agents](spec/agents.md#73-requirements-fr-sec) FR-SEC-20.. |
| NFR-PRIV-01 | No telemetry. No network calls unless AI is enabled for the workspace. | [agents](spec/agents.md#73-requirements-fr-sec) FR-SEC-24, FR-SEC-26 |
| NFR-A11Y-01 | WCAG 2.2 AA; full keyboard operation; screen-reader outline view | [editor-ux §10](spec/editor-ux.md#10-accessibility) FR-A11Y |
| NFR-BROWSER-01 | Chrome/Edge latest-2 full; Firefox/Safari latest-2 degraded (no File System Access API) | [editor-ux §11](spec/editor-ux.md#11-browser-support-and-offline-operation) FR-WEB |
| NFR-PORT-01 | CLI on macOS (arm64/x86_64), Linux (x86_64/arm64), Windows (x86_64) | [architecture §17.2](architecture.md#172-portability-nfr-port-01-sr-51) |
| NFR-LIC-01 | MIT OR Apache-2.0; dependencies and assets pass `cargo deny` with a compatible allowlist | [architecture §17.1](architecture.md#171-license-d12-nfr-lic-01) |

NFR-WASM, NFR-OBS and NFR-SCALE are defined in [architecture](architecture.md).

## 9. Decisions log

| ID | Decision | Rationale |
|---|---|---|
| D1 | Leptos 0.8 (CSR) and Trunk | Fine-grained signals with keyed rendering; the UI is a thin shell |
| D2 | Mangle via `mangle-rs` behind `RuleEngine`; M3.1 spike gate; MiniEngine fallback | One rule set in browser and CI; de-risks an unverified upstream |
| D3 | `.idef0` text DSL is canonical; JSON on the wire | Diffable; agents write text well |
| D4 | Editor logic is pure Rust in `idef0-core`; the UI is thin | Headless tests; identical behavior everywhere |
| D5 | Layout is always automatic and FIPS-driven. Box and port order are semantic. Auto-Arrange is an explicit, previewed patch, auto-applied only to AI proposals and to imports without layout. | The standard defines appearance; numbering must stay stable |
| D6 | Determinism contract | Golden tests, reproducible AI repair loops, trust |
| D7 | `idef0-ingest` crate is native-only (C tree-sitter) | Keeps C code out of WASM |
| D8 | Profiles: `extended` (default, 3–7 boxes, extensions on) and `fips183` (strict) | **[Deviation]** box limit is profiled; the standard stays reachable |
| D9 | Every function and concept has a name and a description; concepts are first-class | Definitions remove ambiguity and enable concept checks |
| D10 | *Superseded by D13* | — |
| D11 | IDEF0 = FIPS 183 baseline; every rule traces to a clause; extensions and deviations are labeled and profiled | The standard is the contract |
| D12 | License: **MIT OR Apache-2.0** | Rust ecosystem norm; OFL fonts compatible |
| D13 | Authority: NORTHSTAR > PRD and specs > CLAUDE.md. CLAUDE.md is a map; its conventions are folded into the specs. | One source of truth |
| D14 | Deterministic identity; parsing never mints random IDs; call targets reference boxes by **stable key**, with the FIPS label computed for display | Renumbering can't break calls; determinism holds |
| D15 | Agent and LLM writes (including MCP `apply_patch`) become **pending proposals** that need human acceptance; `[agents] auto_accept=false` by default | NORTHSTAR principle 8; injection safety |
| D16 | Retro-theme font is an **OFL-licensed** VGA-style font (chosen at M5.6); no CC BY-SA assets | Keeps the asset license policy uniform (was Q-16) |
| D17 | Deleted box keys are **retired** (tombstones in the model file) and can never be reused | A stale tag can never silently bind to an unrelated new box (was Q-17) |
| D18 | C-numbers are **auto-assigned** from the author's initials on status change or major-edit reset; the previous number is kept as history; manual override allowed | FIPS kit-cycle fidelity without bookkeeping (was Q-18) |
| D19 | Pending proposals (`.idef0/proposals/`) are **local only** (gitignored); only accepted changes reach git | Clean PRs (was Q-19) |
| D20 | v1 lays out labels left-to-right (RTL shaping deferred to v2); keymap remapping ships in **v1.x**, not v1 | Scope control; the FSM is table-driven so remapping is cheap later (was Q-20) |

## 10. Risks and open questions

| # | Risk or question | Mitigation or status |
|---|---|---|
| R-1 | `mangle-rs` maturity and wasm latency unverified | M3.1 spike gate; MiniEngine fallback; Worker default if over budget |
| R-2 | Routing and Auto-Arrange performance and quality unproven | Layout spike before M4.3 ([roadmap](roadmap.md)) |
| R-3 | Second-order prompt injection through PRD/code into contract cards | Provenance fencing, D15, threat model ([agents](spec/agents.md)) |
| R-4 | tree-sitter grammar ABI churn; C toolchain on Windows | Pinned grammar versions; CI matrix |
| R-5 | Leptos API churn | Thin UI (D4); pinned minor version |
| R-6 | Verb-noun lint false positives; English-only lexicon | Warning severity; auto-off for non-English `meta.lang` |
| R-7 | Browser key capture and layout-dependent keys | `KeyboardEvent.key` for letters; avoid reserved chords ([editor-ux](spec/editor-ux.md)) |
| R-8 | Description staleness | Drift-tracked sources; signature drift lint; status gate |
| Q-9 | FEO and T/G *sheets* as first-class pages? | v1 exports T/G pages; FEO deferred to v2 |
| Q-10 | Legacy import format (AI0Win, BPwin, IDEF0 IDL)? | Deferred; candidate is IDL |
| Q-11 | Differences between ISO/IEC/IEEE 31320-1 and FIPS 183 | Obtain 31320-1 and diff it against the conformance matrix before v1.0 |
| Q-12 | Should balance accept concept-compatible mappings without explicit bundles? | Default strict; revisit after dogfooding |
| Q-13 | Web Worker for validation by default? | Decide from the M3.1 measurement |
| Q-14 | Localized verb lexicons and stemming beyond English? | v2 |
| Q-15 | Source and license of the verb lexicon (about 1,500 verbs) | Must be MIT/Apache-compatible; pick at M2.3 |

## 11. Change log

| Version | Change |
|---|---|
| v0.1 | Monolithic PRD (about 1,870 lines), archived at [archive/PRD-v0.1.md](archive/PRD-v0.1.md). |
| v0.2 | Split into the spec set. Applied review findings SR-01..63 ([review](reviews/2026-10-15-spec-review.md)). Added D12–D15, user journeys, FR-METRIC-01, security and privacy, editing semantics, identity, testing strategy, CLI reference. |
