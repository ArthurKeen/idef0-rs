# Expected results: `fixtures/reference/factory-a0.idef0`

This is the reference fixture for the A-0 diagram *Develop/Update a Factory* (model `FAC`). It was moved here from Appendix C of PRD v0.1 and updated to DSL 0.2.

- **Source image:** [docs/research/examples/factory-a0.png](../../docs/research/examples/factory-a0.png).
- **What it must reproduce:** the image's **structure**:
  - port order: C1 = Budget … C6 = Regulation; I1 = Market Information …; O1 = New or Updated Factory Specification …; M1 = Manufacturing Software System
  - label sides and the label cascade
  - markers
- **Not required:** pixel identity.
- **Placeholders:** purpose, viewpoint and descriptions are not visible in the image, so they are `TBD`. Replace them from the source model if one becomes available.
- **Rendering rules:** [layout §reference rendering](../../docs/spec/layout.md#133-theme-acceptance-factory-reference-fixture).

## 1. Expected diagnostics: `extended` profile, A-0 only

The model has only A-0, so it has **no errors**. FIPS §3.3.4.2 makes the A0 decomposition mandatory, but a model under construction legitimately starts with A-0 only, so `extended` reports `missing_a0_decomposition` as a warning.

| Code | Severity | Subject | Why |
|---|---|---|---|
| `missing_a0_decomposition` | warning | model `FAC` | No A0 diagram (§3.3.4.2). Warning in `extended`, error under `fips183` ([validation registry](../../docs/spec/validation.md#7-diagnostic-registry)). |
| `undescribed_diagram` | warning | diagram `A-0` | The A-0 diagram has no `description` (FR-NAME-05) |
| `L-META-03` | warning | model `FAC` | The model has no `description` (FR-META-03) |
| `L-META-02` | warning | model `FAC` | The model has no `scope` (FR-META-04) |
| `untraced_leaf` | info | box `A0` | The top box is a leaf until A0 exists, and no PRD section traces it |
| `L-DESC-02` | info | box `A0` | Box description is the placeholder `TBD` |
| `L-DESC-03` | info | 14 concepts | Every concept description is the placeholder `TBD` |
| `L-DESC-05` | info | model `FAC` | `purpose` and `viewpoint` are placeholders (one diagnostic listing both fields) |

**Checks that must pass:**
- `context_box_count`: A-0 has one box, numbered 0.
- `no_control`, `no_output`: six controls and four outputs.
- Dangling-endpoint checks: none. Ranks: gap-free.
- `context_tunnel`: no border tunnels on A-0.
- `L-META-01`: the `TBD` purpose and viewpoint count as present. The placeholder check is `L-DESC-05`.
- `undescribed_function`, `undescribed_concept`: `TBD` counts as a present description (`described` facts). Placeholders are reported only by the `L-DESC-*` lints.
- `unbound_mechanism`: the top box is a leaf, but its only mechanism is `kind=component`, which `mech_ok` accepts (review SR-14). It has no `code=`, so `ungrounded_mechanism` does not apply.
- `L-TITLE-01`: must accept the slash-joined verb alternatives in "Develop/Update a Factory" as an active verb phrase.
- `L-GLOSS-01`: no acronyms appear in the box name or labels.
- `L-NAME-03`, `L-NAME-04`: no reserved words or B.2.4.2-3 keywords.

## 2. `fips183` profile

| Code | Severity |
|---|---|
| `missing_a0_decomposition` | **error** |
| `undescribed_diagram`, `L-META-02`, `L-META-03`, `L-DESC-*` | info |
| `untraced_leaf` | off |

## 3. Rendering test (classic theme)

- **Shadow:** the reference image draws a drop shadow on the top box because its source model has an A0 diagram. The classic theme shadows a box only when it has a child diagram, so the **rendering test adds a 3-box stub A0 diagram** before asserting the shadow. The stub is not expected to be diagnostic-clean: it will show imbalance until all 14 ICOMs are wired.
- **Structural assertions:** see [layout §reference rendering](../../docs/spec/layout.md#133-theme-acceptance-factory-reference-fixture).
  - port order as listed above
  - label side and alignment per FR-STYLE-01..04
  - strictly increasing control-label y
  - 0 bends on boundary arrows
  - no ICOM codes on A-0 (§3.4.2)
  - node-number corner label `A0` (classic theme, [Extension])
