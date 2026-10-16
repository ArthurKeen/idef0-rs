# idef0-rs North Star

> **Every function an AI agent builds has a contract a human can read, a machine can check, and code can prove.**

This document is the "why" and the decision compass for idef0-rs. The [PRD](PRD.md) says *what* we build and *how*. This says what we optimize for and what we refuse to trade away. When the PRD is silent or two good options conflict, decide here.

---

## The problem

AI coding agents write plausible code fast. They are poor at holding **compositional interface contracts**: what a unit consumes, what governs it, what it must produce, and what implements it. Requirements documents state these contracts loosely, and code states them implicitly. As systems grow, the gap between "what we meant" and "what got built" widens silently, one plausible commit at a time.

In the 1970s, the USAF ICAM program faced the same gap between large teams and complex manufacturing systems. It adopted Douglas T. Ross's SADT and documented the subset it used as **IDEF0** around 1981. NIST standardized IDEF0 in 1993 as FIPS PUB 183. In IDEF0, every function is a box, and every interface is classified as an **I**nput it transforms, a **C**ontrol that governs it, an **O**utput it guarantees, or a **M**echanism that performs it. The ICOMs balance from parent to child, recursively, down to the leaves. KBSI's tools, the keyboard-driven DOS version and AI0Win, made the method fast enough to use in real work.

idef0-rs brings that discipline to the age of AI agents.

## The north star

**A model where every leaf function is *contract-complete*, kept that way automatically as requirements and code change.**

A **leaf function** is a box with no child diagram of its own. In the classic theme, it is the box drawn without a drop shadow. A leaf is **contract-complete** when all of the following hold:

1. It has a **name** (active verb phrase) and a **description**.
2. Every ICOM arrow on it carries a **named, defined concept**.
3. Its diagram **balances** against the parent box: every ICOM is accounted for, per FIPS 183.
4. It is **traced** to at least one current (non-stale) requirement.
5. It is **grounded** in code that exists (`@implements`), either directly or through a call arrow to a box that is grounded.
6. It has **zero error-severity diagnostics**, evaluated with CI severities.

### North Star Metric

**Contract-complete leaf ratio**: contract-complete leaves ÷ all leaves, over the models in the workspace. `idef0 validate` reports it in its JSON and SARIF output, from a Mangle rule (FR-METRIC-01, [validation spec](spec/validation.md)). It rises when people model well, when agents implement against contract cards, and when CI catches drift. It falls when code or requirements change without the model, which is exactly the signal we want.

### Supporting signals

idef0-rs collects **no telemetry**. Each signal below comes from an artifact the team already owns.

| Signal | Why it matters | Data source |
|---|---|---|
| **Agent first-pass conformance**: share of agent-implemented leaves that pass `idef0 validate` on the first try when given a contract card | Proves contracts make agents more correct, not just more documented | CI history: validate results on agent-authored PRs |
| **Drift caught before merge**: stale traces, stale code refs and signature drift flagged in CI rather than found later | The model stays true without heroics | SARIF results across CI runs |
| **Keystrokes per box**: median keystrokes to add a box with name, description and wired ICOMs | DOS-era speed is a feature, not nostalgia | Scripted `.keys` scenarios in the test suite; optional local-only counters |
| **Determinism**: zero native-vs-wasm divergences in facts, layout or diagnostics | Trust in the tool depends on it | Cross-target golden tests |
| **FIPS conformance**: zero `G-*` geometry findings; clean `profile = fips183` runs on reference fixtures | We are an IDEF0 tool, not something IDEF0-flavored | Geometry checker and conformance fixtures in CI |

## Who we serve

- **The analyst/architect** who decomposes a system and must keep it coherent across levels and teams.
- **The AI coding agent** that needs precise, compositional context: one contract card, not a 40-page PRD.
- **The reviewer and CI** who need a mechanical answer to "does this change still honor the design?"

## Principles

1. **The standard is the contract.** IDEF0 means FIPS 183. Every rule traces to a clause. Everything idef0-rs adds is labeled: an **[Extension]** adds checks or features (concept ontology, code mechanisms, control locks), and a **[Deviation]** departs from the standard (7 boxes, `[ ]` tunnel glyph, strict staircase). The `fips183` profile switches extensions off, downgrades them to info, or keeps them as notation only (identity keys, slash port refs); deviations revert to FIPS behavior where the tool supports it (box limit 6, `( )` tunnel glyphs, box-number corner label), and the strict staircase remains a documented deviation, so FIPS semantics are never silently changed.
2. **Contracts before code.** Model the interface, then implement against it. Scaffolds, contract cards and tags all flow from the box, not the other way around.
3. **Name and define everything.** Every function and every concept has a name and a description. A label without a definition is ambiguity waiting to ship.
4. **One set of rules, everywhere.** The same Mangle rules run in the browser as you type, in the CLI and in CI. A diagnostic means the same thing wherever you see it.
5. **Deterministic or it didn't happen.** The same model bytes produce the same facts, layout, SVG and diagnostics on every platform, so golden tests and AI repair loops stay reproducible.
6. **Layout is computed, not drawn.** The standard says how a diagram looks: staircase, up-and-over control feedback, down-and-under input feedback, 90° arcs, ICOM codes at the edges. The engine owns geometry, and humans own meaning.
7. **Keyboard first, mouse welcome.** Everything is reachable at DOS speed (`O 3 I` wires box 1's output to box 3's input), and the mouse drives the same state machine.
8. **AI proposes, rules decide, humans approve.** Agents draft decompositions, descriptions, repairs and patches, including those submitted through MCP. Mangle validates them. People accept them, and accepted changes go through the undo stack. Nothing rewrites the model silently.
9. **Traceable end to end.** Requirement ↔ function ↔ concept ↔ code, in both directions, with drift detection, so search can follow any link.
10. **Rust, local first.** The codebase is Rust; only the native code scanner links C tree-sitter grammars. It runs offline in the browser, keeps secrets on the server, sends nothing to an LLM unless the workspace opts in, and holds models as plain text in git.

## Decision filters

When in doubt, ask in order:

1. **Does it keep or raise the contract-complete ratio?** If it makes contracts vaguer, it's wrong.
2. **Is it what FIPS 183 says?** If not, is it a declared, profiled extension or deviation?
3. **Is it deterministic and identical in wasm and native?**
4. **Can an agent consume it?** Prefer text, stable IDs and structured output over pixels and prose.
5. **Is it reachable from the keyboard in a few keystrokes?**
6. **Is it reviewable?** Changes to the model should surface as patches, not surprises.

## Non-goals

- A general-purpose diagramming tool. There is no free-form drawing and no arbitrary shapes.
- A full IDEF family suite (IDEF1X, IDEF3) in v1.
- Real-time multi-user co-editing in v1. Git and reviewable patches come first.
- Replacing the PRD or the code. The model *binds* them; it doesn't absorb them.
- "AI magic" that edits the model without validation and human acceptance.

## Horizon

| Horizon | What "good" looks like |
|---|---|
| **v1: Faithful & fast** | FIPS-conformant modeling, auto layout, keyboard FSM, Mangle validation, names/concepts/search, PRD and code traceability, contract cards. Text and glossary page export. An MCP server with human-accepted writes. idef0-rs models and validates *itself* in CI. |
| **v2: Agent-native** | End-to-end agent workflows in which an agent receives a contract card, implements it, tags it, validates it and opens a PR for human review. Kit-cycle review (reader notes) in the UI. FEO sheets. Legacy model import. IEEE 1320.1 / ISO/IEC/IEEE 31320-1 alignment. |
| **Beyond** | Cross-repo model federation through call arrows. Contract-complete ratio as an organization-wide engineering health metric. |

## Lineage

idef0-rs stands on the work of Douglas T. Ross (SADT), the USAF ICAM program, NIST's FIPS 183, and KBSI's DOS and AI0Win tools. Their bet was that rigorous, readable function contracts make large systems tractable. Our bet is that the same is true when half the builders are machines.
