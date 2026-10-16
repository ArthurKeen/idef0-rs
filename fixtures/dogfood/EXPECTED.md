# Expected results: `fixtures/dogfood/idef0-rs.idef0`

This fixture models idef0-rs itself (model `IR`). It was moved here from Appendix B of PRD v0.1 and updated to DSL 0.2: version header, explicit box keys, explicit port ranks, and definitional descriptions (FR-NAME-01). Grammar and semantics: [metamodel](../../docs/spec/metamodel.md). Rules and severities: [validation](../../docs/spec/validation.md).

## 1. Gates

| Command | Expected | When |
|---|---|---|
| `idef0 fmt --check fixtures/dogfood/idef0-rs.idef0` | Exit 0 (the file is canonical) | From M2.2 |
| `idef0 check fixtures/dogfood/idef0-rs.idef0` | Exit 0: no `E-*` and no `L-*` at warning or above | From M2.4 |
| `idef0 validate --profile extended` (structural facts only) | No structural `R-*` diagnostics | From M3.3 |
| `idef0 validate --fail-on error` (with code and PRD facts) | Fails until M10.7 (§4) | M10.7 makes it pass |

## 2. Arrow ids (extractor golden)

Structural ids per [metamodel §3.4](../../docs/spec/metamodel.md#34-arrow-ids). Diagram A0:

| Arrow | Id | Leaves |
|---|---|---|
| PRD Documents | `A0:I1->1I1#0` | — |
| Requirement Index | `A0:1O1->2I1#0` | — |
| Draft Model (fork) | `A0:2O1->*#0` | `/0` → 3I1, `/1` → 4I1 (both `rel=all`) |
| Diagnostics | `A0:3O1->2C1#0` | — |
| Diagram Scenes | `A0:4O1->2C2#0` | — |
| Source Code | `A0:I2->5I1#0` | — |
| Code Facts | `A0:5O1->3I2#0` | — |
| Validated Model | `A0:3O2->O1#0` | — |
| Contract Cards | `A0:5O2->O2#0` | — |
| IDEF0 Standard (fork) | `A0:C1->*#0` | `/0` Syntax Rules → 3C1, `/1` Diagram Conventions → 4C1 (`part_of`) |
| Project Rules | `A0:C2->3C2#0` | — |
| Tagging Standard (fork) | `A0:C3->*#0` | `/0` → 1C1, `/1` → 5C1 (both `rel=all`) |
| idef0-rs Toolchain (fork) | `A0:M1->*#0` | `/0` Ingest Engine → 1M1, `/1` Mangle Engine → 3M1, `/2` Layout Engine → 4M1, `/3` Code Scanner → 5M1 (`part_of`) |
| Analyst & AI Agent | `A0:M2->2M1#0` | — |

A-0 ids follow the same pattern, e.g. `A-0:I1->0I1#0` and `A-0:0O2->O2#0`.

**Extractor obligations relied on** ([validation §extractor](../../docs/spec/validation.md#3-fact-extraction-contract)). Without them, this fixture produces false errors (review SR-11).
1. The `rel=all` leaves `A0:2O1->*#0/0..1` carry concept `draft-model`, and `A0:C3->*#0/0..1` carry `tagging-standard` (trunk concept inherited).
2. Every trunk carries its concept and `edge_in(…, "A0")`. Every leaf carries `edge_in`.
3. Leaf-level `code=` attributes stay on their leaf, and `kind=resource` stays on `A0:M2->2M1#0`.
4. Fork leaves share the source rank: `2O1`, boundary `C1`, `C3` and `M1`.

## 3. Structural results (expected clean)

Hand-verified for v0.2:

- **Box counts:** A-0 has 1 box (box 0). A0 has 5 boxes: valid under `extended` (3–7) and `fips183` (3–6). `missing_a0_decomposition` does not fire.
- **Minimums:** every box has ≥ 1 control and ≥ 1 output:

  | Box | Controls | Outputs |
  |---|---|---|
  | A1 | C1 | O1 |
  | A2 | C1, C2 | O1 |
  | A3 | C1, C2 | O1, O2 |
  | A4 | C1 | O1 |
  | A5 | C1 | O1, O2 |
  | Top box A0 (on A-0) | C1–C3 | O1, O2 |

- **Ranks:** gap-free on every side. A3's inputs are I1 (Draft Model) and I2 (Code Facts). A2's controls are C1 (Diagnostics) and C2 (Diagram Scenes).
- **No control lock:** A3 and A4 are controlled from boundary C1, A1 and A5 from boundary C3. A2 is controlled by the outputs of A3 and A4, which are control-live.
- **Balance:** the top box's ports I1–I2, C1–C3, O1–O2 and M1–M2 all appear as coded boundary ports on A0. No uncoded ports and no tunnels.
- **Balance by concept:** at every boundary, the concept at the attached end (the trunk concept for C1, C3 and M1) equals the parent arrow's concept.
- **Bundles:**
  - C1: both leaves `part_of idef0-standard`.
  - M1: all four leaves `part_of idef0-toolchain`.
  - No bundle mixes `kind_of` and `part_of`.
  - `rel=all` leaves carry the trunk concept.
- **Concepts:** no `duplicate_arrow_concept` (bundle leaves share their root), no `concept_cycle`, no `unknown_concept`. All 20 concepts are used, so no `L-CONCEPT-02`.
- **Names and text:**
  - Every box, concept, diagram and the model is described. Purpose, viewpoint and scope are present.
  - C-numbers AK1 and AK2 are unique.
  - The A0 title matches the top box name.
- **Lints that must not fire:**
  - `L-TITLE-01`: the lexicon must accept "Model" as a verb and "Lay Out" as a phrasal verb.
  - `L-GLOSS-01`: the acronyms PRD, IDEF0, FIPS and AI in labels are expanded in the carrying concept's description.
  - `L-NAME-03`, `L-NAME-04`: no box name or label is, or contains, a reserved or B.2.4.2-3 keyword. That is why v0.2 renamed A2 to "Author Draft Model", A5 to "Trace Implementing Code" and the concept to "Validated Model".
  - `L-ROLE-01`: no role changes. `L-SPLIT-CI`: none.
  - `L-DESC-02`..`05`: descriptions are definitions, not ICOM enumerations.
- **Status:** A-0 is `working` and A0 is `draft`. The release gates (`undescribed_box_on_release`, `undescribed_concept_on_release`) do not apply.

## 4. Traceability results before the crates are tagged

These are expected failures. M10.7 clears the errors by implementing and tagging the crates, so this fixture is a live progress meter.

Given the repository as of 2026-10-15:
- `crates/idef0-mangle/src/lib.rs` and `crates/idef0-layout/src/lib.rs` exist. They resolve as file-scope CodeRefs, checked directly by the scanner.
- `crates/idef0-ingest/` does not exist yet.

| Code | Subject | CI severity | Notes |
|---|---|---|---|
| `stale_mech_binding` | edge `A0:M1->*#0/0`, `A0:M1->*#0/3` | error | `prd.rs` and `code.rs` not found |
| `unbound_mechanism` | edge `A0:M1->*#0/0`, `A0:M1->*#0/3` | error (warning in editor) | Leaf mechanisms without an existing binding |
| `ungrounded_mechanism` | box `A1`, `A3`, `A4`, `A5` | error (warning in editor) | Code-bound leaves with no `@implements` tag. A2's only mechanism is `kind=resource`. |
| `untraced_leaf` | box `A1`..`A5` | info | No PRD section traces them yet |

- **When the files change:** when `crates/idef0-ingest/src/{prd,code}.rs` are created, the two `stale_mech_binding`/`unbound_mechanism` pairs disappear. When each crate tags its entry point with `@implements idef0:IR/@<key>`, the matching `ungrounded_mechanism` disappears.
- **Contract-complete ratio:** 0/5 today (FR-METRIC-01).

## 5. `fips183` profile

Run with `--profile fips183`:
- The structural results of §3 hold unchanged. `unbound_mechanism`, `ungrounded_mechanism` and `stale_mech_binding` are info. `untraced_leaf` is off.
- **Expected:** zero errors and zero warnings.
- **Rendering:** the theme draws `( )` tunnel glyphs (none occur here) and box numbers in the corner. `kind_of`/`part_of` branches render as plain labeled branches.
