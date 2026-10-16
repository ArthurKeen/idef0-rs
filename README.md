# idef0-rs

**AI-first, keyboard-fast IDEF0 modeling in Rust.** It turns requirements and code into verifiable function contracts.

idef0-rs is a "retro-mod" take on the classic KBSI IDEF0 tools (the DOS keyboard version and AI0Win), rebuilt for teams in which AI agents write much of the code. You model *what each function consumes, what governs it, what it must produce, and what implements it*, using the IDEF0 standard (FIPS PUB 183). idef0-rs validates those contracts deductively and keeps them bound to your PRDs and source code.

> **Status: pre-alpha, design phase.** The v0.2 specification set is written; the crates are skeletons. Everything below describes the **planned** v1. See the [roadmap](docs/roadmap.md).

---

## Why

LLM agents produce plausible code but lose track of **interface contracts** across a system. IDEF0 states those contracts precisely. Every function is a box with four kinds of interface (ICOMs), and parent and child diagrams must balance:

```
                  Control (C)                                   Control
                  conditions for correct output                    │
                       │                                           ▼
                       ▼                                    ┌─────────────┐
               ┌──────────────┐                    Input ──▶│  Schedule   │──▶ Output
  Input (I) ──▶│  Validate    │──▶ Output (O)               │  Shipment   │
  transformed  │  Order       │    produced                 │          A23│
               │           A21│▒                            └─────────────┘
               └──────────────┘▒                               ▲      │
                ▒▒▒▲▒▒▒▒▒▒▒▒▒▒▒                       Mechanism      ▼ Call: shared detail
                   │                                                    elsewhere (no shadow)
             Mechanism (M)
             code that performs it      ▒ shadow = has a detail diagram (no shadow = leaf)
```

idef0-rs makes that discipline usable by humans *and* agents:

- **Contract cards.** `idef0 contract A21` gives an agent exactly what a function must do: its ICOMs, concept definitions, traced requirements and implementing code.
- **Deductive validation.** Rules written in [Mangle](https://mangle.readthedocs.io) (Datalog) check box limits, ICOM balance, control locks, call arrows, code grounding and drift. The same rules run in the browser and in CI.
- **Traceability.** `<!-- @idef0:A21 -->` in Markdown and `// @implements idef0:@validate-order` in code link requirements ↔ functions ↔ code, with drift detection.

## Planned features (v1)

- **IDEF0 per FIPS 183:**
  - context and decomposition diagrams, node numbering, ICOM codes and balance
  - forks and joins, tunneling, call arrows, model notes, status levels, diagram form
  - a conformance matrix maps each normative clause to an enforcement point
  - `--profile fips183` is strict mode
  - documented deviations: up to 7 boxes in the default profile; off-diagonal boxes are not supported
- **Names, descriptions and concepts.** Every function and every concept (the thing an arrow carries) is named and defined in a model glossary with `kind_of` / `part_of` relations.
- **Auto layout from the standard:**
  - diagonal staircase; "up and over" control feedback; "down and under" input and mechanism feedback
  - 90° arc bends; channel routing with crossing minimization and bridge jumps; curved branches
  - **Auto-Arrange** (`=`) searches box and port orders to reduce feedback arcs and crossings. It is a previewed, undoable change, and it is applied only if the rendered diagram improves.
- **Retro-mod UX.** Fully keyboard-drivable:
  - `PgDn`/`PgUp` move through decompositions, `←/→` move between boxes, `I C O M` jump to ports
  - `O 3 I` wires an output to box 3's input
  - themes: modern SVG, the classic AI0Win look, and ASCII
- **Full-text search** (`/`) across names, descriptions, concepts, notes, PRDs and code, plus structural queries in Mangle.
- **Spec and code ingestion:** PRD tagging, tree-sitter code scanning, sync plans as reviewable patches, scaffolds and contract export.
- **AI proposals (opt-in).** Draft decompositions from requirement text are validated and repaired against the rules, and you accept them box by box. Nothing leaves your machine unless you enable AI for the workspace.
- **Agent integration:** CLI, HTTP API and an MCP server (`idef0 mcp`). Agent changes arrive as **pending proposals** for human acceptance by default (D15; the `auto_accept` opt-out is off by default).

## The model format

Models are plain text (`*.idef0`), diff-friendly and easy for both people and LLMs to write. Arrow endpoints use FIPS reference notation (`1O1` = box 1, output 1; `2C1` = box 2, control 1). Abridged example (concept definitions for the other arrows omitted):

```
idef0 0.2
model "Order Fulfillment" {
  abbrev    "OF"
  purpose   "Specify order handling so services can be implemented and verified independently."
  viewpoint "Operations architect"
}

concept customer-order "Customer Order" {
  description "A request from a customer to purchase one or more catalog items."
}
concept credit-policy "Credit Policy" {
  description "A rule set that bounds credit exposure per customer."
}

diagram A2 {
  box 1 "Validate Order" key=validate-order {
    description "Checks each customer order against credit policy and accepts or rejects it."
  }
  box 2 "Reserve Stock" key=reserve-stock { … }
  box 3 "Schedule Shipment" key=schedule-shipment { … }

  arrow @customer-order  : I1 -> 1I1
  arrow @credit-policy   : C1 -> 1C1
  arrow @validated-order : 1O1 -> 2I1
  …
}
```

And in code:

```rust
// @implements idef0:@validate-order
// @consumes    idef0:OF/A21/I1   "Customer Order"
// @governed-by idef0:OF/A21/C1   "Credit Policy"
pub fn validate_order(order: Order, policy: &CreditPolicy) -> Result<ValidatedOrder, Rejection> { … }
```

## Architecture

A Rust workspace. Core, rules and layout are pure Rust and compile to both native and `wasm32-unknown-unknown`, so the browser, CLI and CI behave identically. Only the native code scanner links C tree-sitter grammars.

| Crate | Role | State |
|---|---|---|
| `idef0-core` | Metamodel, identity, `.idef0` parser and formatter, commands and undo, keyboard FSM, lints, search index | skeleton |
| `idef0-mangle` | Fact extraction, Mangle rules, profiles, diagnostics (JSON / SARIF) | skeleton |
| `idef0-layout` | FIPS auto layout, routing, Auto-Arrange, SVG / ASCII renderers, themes | skeleton |
| `idef0-ingest` | PRD parser, tree-sitter code scanner, sync, import/export (native only) | planned (M0.2) |
| `idef0-wasm` | Leptos web app (CSR, built with Trunk) | skeleton |
| `idef0-server` | Axum backend: workspace store, API, security, proposals, AI, PNG export | skeleton |
| `idef0-cli` | The `idef0` command line ([reference](docs/cli.md)) | skeleton |

## Getting started

The crates are currently placeholders, but the workspace builds:

```sh
cargo build --workspace
cargo test --workspace
```

The workspace uses Rust edition 2024. The toolchain pin and MSRV are described in [architecture](docs/architecture.md).

Planned CLI usage ([full reference](docs/cli.md)):

```sh
idef0 init                                   # create .idef0/ and a starter model
idef0 check    models/of.idef0               # parse, type check, lints
idef0 validate --format sarif --fail-on error
idef0 render   models/of.idef0 A2 --svg > A2.svg
idef0 arrange  A2                            # preview the Auto-Arrange patch
idef0 contract A21                           # contract card for an agent
idef0 serve                                  # web UI + API on 127.0.0.1:7183
```

## Documentation

| Document | What it is |
|---|---|
| [docs/NORTHSTAR.md](docs/NORTHSTAR.md) | Vision, principles, north star metric, decision filters |
| [docs/PRD.md](docs/PRD.md) | Requirements index: journeys, scope, decisions, risks, document map |
| [docs/glossary.md](docs/glossary.md) | Terms |
| [docs/spec/](docs/spec/) | Metamodel, validation, layout, editor UX, ingestion, agents |
| [docs/architecture.md](docs/architecture.md), [docs/testing.md](docs/testing.md), [docs/cli.md](docs/cli.md) | Engineering specs |
| [docs/roadmap.md](docs/roadmap.md) | Milestones and epics |
| [docs/research/](docs/research/) | FIPS PUB 183 and reference material |
| [CLAUDE.md](CLAUDE.md) | Map and conventions for AI coding agents working in this repo |

## Lineage

idef0-rs builds on Douglas T. Ross's SADT, the USAF ICAM program (which documented IDEF0 around 1981), NIST FIPS PUB 183 (1993), and KBSI's IDEF0 tools. FIPS 183 is the freely available reference and this project's baseline.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in the work, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
