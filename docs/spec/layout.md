# Layout, Routing, Themes and Rendering

| | |
|---|---|
| **Status** | DRAFT v0.2 |
| **Part of** | idef0-rs spec set; index: [docs/PRD.md](../PRD.md) |
| **Normative references** | FIPS PUB 183 §3.2.1.3, §3.2.2.3, §3.3.2.6–3.3.2.10, §3.3.3, §3.3.4.1, §3.3.4.4, §3.4.2–3.4.3; Annex B.2.2–B.2.4 and Annex C.4 (*informative*); decisions D5, D6, D8, D12, D13 |
| **Scope** | `idef0-layout`: automatic FIPS-driven layout, routing, labels, Auto-Arrange, themes, SVG/ASCII/PNG renderers, the `G-*` geometry checker and performance budgets |

Cross-references: model, commands and identity in [metamodel](metamodel.md); diagnostics registry in [validation](validation.md); keys and preview UX in [editor-ux](editor-ux.md); CLI flags in [cli](../cli.md); fixtures and goldens in [testing](../testing.md); crate boundaries in [architecture](../architecture.md); terms (leaf, caller box, decomposed) in the [glossary](../glossary.md).

---

## 1. Scope and principles

**FR-LAYOUT-00:** idef0-rs lays out every diagram **automatically**. The user never positions boxes or draws arrow paths. The same engine produces the same Scene in the browser, the CLI and the server (D6).

### 1.1 Principles

1. **Layout is a pure function.** `layout(&Model, &NodeId, &LayoutInput) -> LayoutResult` reads only its arguments. It uses no clock, no randomness, no `HashMap` iteration and no floating point in any decision.
2. **Drags are semantic commands, not positions.** The editor maps pointer gestures to model commands (see [editor-ux](editor-ux.md)):
   - dragging a box past a neighbor is `MoveBox` (renumbering);
   - dragging a port past another port on the same side is `ReorderPorts` (re-coding);
   - dragging an arrow end to another port is `Retarget`.
   The layout is then recomputed from the changed model.
3. **Box order and port order are semantic** (D5). Box order *is* the box numbering (FIPS §3.3.3-5), and port order *is* the ICOM coding of the child diagram (§3.3.2.8). Layout never changes either; only commands do.
4. **Determinism (D6).** Identical model bytes plus identical `LayoutInput` (theme, page, metrics) produce identical Scene and SVG bytes on native and `wasm32-unknown-unknown`. PNG is excluded (§14.3).

### 1.2 Two tiers

| Tier | Decides | Runs | Changes the model? |
|---|---|---|---|
| **Geometric layout** (§3–§11) | Box sizes and positions, routes, tracks, fillets, bundles, bridges, labels, codes, form | On every model change | No |
| **Auto-Arrange** (§12) | Box order, port order, merge/bundle/"to all" proposals | On request; auto-applied only per D5 | Yes: a previewed, undoable `Patch` |

### 1.3 Rule kinds and provenance

- **Hard** rule: an invariant. The engine never emits a Scene that violates it, and the geometry checker (§15) verifies it.
- **Soft** rule: a weighted term in a cost function, or a proposal. Soft rules can lose to other soft rules.
- **Normative (N):** FIPS 183 Sections 1–3. Compliance is required by the standard.
- **Informative (I):** FIPS Annexes A–D, which "provide additional suggestions and guidance" (FIPS Introduction). idef0-rs may still adopt informative guidance as a Hard rule. The table says so.
- **[Extension]** marks an idef0-rs addition. **[Deviation]** marks a departure from FIPS. "idef0-rs invariant" marks a Hard rule with no FIPS source.

### 1.4 Out of scope

- PDF and print pipelines, and Markdown/T-G page publishing: see [ingestion](ingestion.md) (import/export) and [cli](../cli.md).
- Node-tree diagrams (§3.3.4.2.2): only the textual node index is specified in v0.2 (the graphical node tree is an open item).
- Off-diagonal boxes (§3.3.4.1): unsupported, see FR-LAYOUT-01.

---

## 2. FIPS-derived layout requirements (FR-LAYOUT)

| ID | Rule | FIPS source | N/I | Kind | § |
|---|---|---|---|---|---|
| FR-LAYOUT-01 | Boxes form a strict diagonal staircase in box-number order **[Deviation]** | §3.3.2.6 ("normally"); §3.3.3-5; B.2.2.1-1 | N + I | Hard | 5 |
| FR-LAYOUT-02 | Upper-left boxes dominate lower-right boxes through control arrows | B.2.2.1-2; B.2.4 | I | Soft (Auto-Arrange) | 12 |
| FR-LAYOUT-03 | Box fits its name; square corners; solid lines; corner number inside lower right; DRE beneath lower right | §3.2.1.3 (boxes 1–3); §3.3.3-5, -6 | N | Hard (corner label: see bullets) | 4 |
| FR-LAYOUT-04 | Arrows are horizontal/vertical; bends, forks and joins use 90° arcs | §3.2.1.3 (arrows 1–3); §3.3.3-7; B.2.4.3-9 | N (bends); I (forks/joins) | Hard | 6–9 |
| FR-LAYOUT-05 | Arrow ends touch the box perimeter, never enter it, and never attach at corners | §3.2.1.3 (arrows 4–5) | N | Hard | 4 |
| FR-LAYOUT-06 | I enters left, C enters top, O leaves right, M enters bottom pointing up, call leaves bottom pointing down | §3.2.2.3-2 | N | Hard | 6 |
| FR-LAYOUT-07 | Control feedback "up and over"; input and mechanism feedback "down and under" | §3.3.3-12; B.2.4.3-3 | N | Hard | 6 |
| FR-LAYOUT-08 | ICOM code at the unconnected end of each boundary arrow, unless tunneled; none on A-0 | §3.3.2.8; §3.3.3-13; §3.4.2; B.2.4.2-5 | N | Hard | 11 |
| FR-LAYOUT-09 | Corners, crossings and labels keep `clear` ≥ 4 gu from boxes | B.2.4.2-2 | I | Hard (idef0-rs choice) | 3, 7 |
| FR-LAYOUT-10 | Parallel arrows are at least one `pitch` apart | B.2.4.2-7 | I | Hard (idef0-rs choice) | 7 |
| FR-LAYOUT-11 | Minimize crossings | B.2.4.3-5 | I | Soft | 6, 7 |
| FR-LAYOUT-12 | Minimize curves and corners while keeping labeled segments clear | B.2.4.3-6 | I | Soft | 6 |
| FR-LAYOUT-13 | A forked arrow enters each destination at the same relative ICOM position | B.2.4.3-4 | I | Soft (Auto-Arrange) | 12 |
| FR-LAYOUT-14 | Open boundary arrows for the same data are joined through a fork or join | §3.3.3-14; B.2.4.2-6 | N ("unless unreadable") | Soft (proposal) | 12 |
| FR-LAYOUT-15 | Bundle arrows with the same source and destination; few arrows per side | B.2.3.2-1; B.2.4.3-1, -2 | I | Soft (proposal) | 12 |
| FR-LAYOUT-16 | Label each arrow segment, except call arrows and where one label applies to the whole arrow; squiggle unless obvious | §3.2.2.3-3, -4 | N | Hard (presence) / Soft (placement) | 11 |
| FR-LAYOUT-17 | Label long arrows twice; extra arrowheads on long arrows | B.2.4.2-4, -8 | I | Soft | 11 |
| FR-LAYOUT-18 | "To all" stub for an arrow that applies identically to every box | B.2.4.3-8 | I | Soft (proposal) | 9, 12 |
| FR-LAYOUT-19 | Tunnel glyph at the tunneled end; model-note glyph attached by squiggle | §3.3.2.9; §3.3.4.4 | N | Hard | 11 |
| FR-LAYOUT-20 | Standard diagram form: working fields and identification fields | Annex C.4 | I | Hard (vector themes); simplified in `retro` | 14 |
| FR-LAYOUT-21 | Box order is driven by heavy constraint arrows first, then boundary arrows, then the rest | B.2.4 | I | Soft (Auto-Arrange weights) | 12 |
| FR-LAYOUT-22 | Boundary arrows are ordered along each frame edge by ICOM code | none | idef0-rs invariant | Hard | 6 |
| FR-LAYOUT-23 | The call arrow uses the rightmost bottom port, right of all mechanisms | Fig. 3 (convention) | N (figure) | Hard (idef0-rs choice) | 4, 6 |
| FR-LAYOUT-24 | Fillet and branch radii are 6–10 px at zoom 1 | D13 (old `claude.md`) | idef0-rs invariant | Hard | 8 |
| FR-LAYOUT-25 | At a crossing, the vertical segment carries the bridge | D13 (old `claude.md`) | idef0-rs invariant | Hard | 10 |
| FR-LAYOUT-26 | Higher context diagrams (A-1 … A-n) use the ordinary staircase with box 0 or −(n−1) in one slot | §3.4.2; §3.4.3 | N | Hard (stretch) | 5.6 |

Notes on the table:

- **FR-LAYOUT-01.** FIPS says boxes are "normally" placed diagonally (§3.3.2.6), and Annex B adds that "any layout which makes clear the author's intent is acceptable" (B.2.2.1-1). §3.3.4.1 also defines numbering for off-diagonal boxes. The strict staircase is therefore an idef0-rs design choice, recorded as a **[Deviation]** because off-diagonal layouts cannot be expressed. It holds in every profile. Imported models with off-diagonal positions are re-laid out on the diagonal with an info diagnostic (defined in [ingestion](ingestion.md)).
- **FR-LAYOUT-03, corner label.** §3.3.3-5 requires the *box number* in the lower-right inside corner. The `classic` and `modern` themes show the *node number* instead (FR-STYLE-07, a **[Deviation]**). So the corner-label part of FR-LAYOUT-03 is Hard **only under the `fips183` theme**. The other parts (fit, square corners, solid lines, DRE) are Hard in every theme.
- **FR-LAYOUT-04.** The normative text (§3.2.1.3, §3.2.1.2) requires 90° arcs at *bends*. Curved forks and joins come from informative B.2.4.3-9 ("All arrows shall have curved corners at joins, forks and bends"). idef0-rs adopts both as Hard.
- **FR-LAYOUT-06 / -23.** §3.2.2.3-2e says only that call arrows point down, connect to the bottom side and carry the reference expression. Placing the call arrow to the right of all mechanisms follows FIPS Figure 3. It is a convention, not a stated rule.
- **FR-LAYOUT-08 / -22.** FIPS assigns ICOM codes by the *parent box's* port positions (§3.3.2.8) but says nothing about where the codes sit along the child's frame. Ordering boundary arrows along each frame edge by code (I1 above I2, C1 left of C2, O1 above O2, M1 left of M2) is an idef0-rs invariant. It can force a dog-leg where a straight boundary arrow would otherwise be possible (§5.4).
- **FR-LAYOUT-14.** §3.3.3-14 is normative but has an escape ("unless this results in an unreadable diagram"). Merging changes the model, so layout never merges on its own: validation flags candidates (`duplicate_arrow_concept`, see [validation](validation.md)) and Auto-Arrange offers a merge proposal.
- **FR-LAYOUT-16 (SR-05).** §3.2.2.3-3: "Arrow segments, except for call arrows, shall be labeled with a noun or noun phrase unless a single arrow label clearly applies to the arrow as a whole." An *arrow segment* here is a FIPS segment of the arrow's branching tree (between an end, a fork or a join), not a geometric polyline segment. §11.1 defines how idef0-rs decides that one label "clearly applies".
- **FR-LAYOUT-20.** Annex C is informative. idef0-rs renders the form because it carries the identification and configuration fields. Publication export drops the working fields but must copy Used At into a model note (C.4.1.6), §14.4.
- **FR-LAYOUT-21 (SR-07).** B.2.4 says: "start this layout with the most heavily used constraint arrows only… Draw all boundary arrows shown on the parent diagram and then add the remaining arrows." idef0-rs follows this order for *placement* (box order): the Auto-Arrange proxy weights constraint arcs most, boundary affinity second, everything else last (§12.2). The *routing* order in §6.4 is a separate implementation choice and is not claimed to come from FIPS.

---

## 3. Coordinate system and metrics

### 3.1 Units and lattice

- **Grid unit (gu):** layout decisions use integer gu (`i32`).
- **Sub-unit (su):** 1 su = ¼ gu. The Scene stores every coordinate in su (`i32`). At zoom 1, **1 su = 1 CSS px**, so 1 gu = 4 px.
- **Lattice:** every polyline vertex, box corner, port point, ICOM code anchor and (in monospace Scenes) label anchor lies on the lattice **x ∈ 2 gu·ℤ, y ∈ 4 gu·ℤ** (8 su horizontally, 16 su vertically). The lattice equals the ASCII cell grid (§14.2), so the retro renderer needs no snap pass.
- **Vertical metrics are multiples of 4 gu; horizontal metrics are multiples of 2 gu** (SR-42).
- **Origin:** the top-left corner of the message field (the diagram frame), y pointing down. The form bands (§14.4) sit outside the frame.
- **Zoom:** export is fixed at zoom 1. On-screen zoom is a uniform `viewBox` scale applied by the UI. The Scene and the SVG bytes never depend on zoom, and there is no screen-space clamping (SR-43).
- SVG coordinates are therefore integers in px. No float formatting occurs in geometry.

### 3.2 Metrics

| Constant | Value | px at zoom 1 | Constraint |
|---|---|---|---|
| `pitch_x` (spacing of vertical tracks) | 6 gu | 24 | ≥ 2·`r_bump` + 2 gu; even |
| `pitch_y` (spacing of horizontal tracks) | 8 gu | 32 | multiple of 4 gu; 8 gu leaves one free text row between tracks |
| `port_pitch_x`, `port_pitch_y` | 8 gu, 8 gu | 32, 32 | ≥ 2 × the lattice step of the axis |
| `clear` (keep-out to track, corner and label) | 4 gu | 16 | ≥ `r_fillet` + `head_len` |
| `r_fillet`, `r_branch`, `r_bump` | 8 su each | 8 | within [`r_min`, `r_max`] |
| `r_min`, `r_max` | 6 su, 10 su | 6, 10 | FR-LAYOUT-24 |
| `head_len`, `head_halfw` | 8 su, 4 su | 8, 4 | filled triangle |
| `glyph_w` (monospace advance) | 2 gu | 8 | one ASCII column |
| `line_h` | 4 gu | 16 | one ASCII row |
| `box_min` (w, h) | (40, 24) gu | (160, 96) | on the lattice |
| `w_max` (title wrap target) | 80 gu | 320 | soft; the box may grow past it (§4.2) |
| `pad` (text inside boxes and frame) | 2 gu | 8 | |
| `margin` (minimum frame margin) | 24 gu | 96 | |
| `shadow` (drop-shadow band) | 2 gu right, 4 gu bottom | 8, 16 | one ASCII cell |

- `Metrics` is overridable per workspace, but every override must satisfy the constraint column. `Metrics::validate()` rejects violations with `E-METRICS` (defined in [validation](validation.md)).
- **Keep-out region** of a box: the box rectangle, plus the shadow band when the theme draws one, plus the DRE text block. All clearances are measured from the keep-out region.

### 3.3 Text metrics (SR-49)

Text width is **theme-dependent**. The Scene records which metric produced it.

| Metric | Themes | Width of string `s` |
|---|---|---|
| `Mono` | `modern`, `retro`, `fips183` | Σ over grapheme clusters of `unicode-width(cluster)` × `glyph_w` |
| `Serif` | `classic` | Σ over grapheme clusters of the Liberation Serif advance of the cluster's base character, in font units, scaled to su and rounded up once per string |

- Grapheme clusters come from `unicode-segmentation`. In `Mono`, East Asian wide characters and emoji count as 2 columns (`unicode-width`); zero-width clusters count as 0.
- `Serif` uses an advance-width table compiled into `idef0-layout` from the bundled Liberation Serif Regular (SIL OFL 1.1, allowed by D12). Measurement is integer arithmetic on font units. There is no kerning and no ligature substitution. A character missing from the table measures as the font's `.notdef` advance.
- `classic` text sizes: font size 14 px, `line_h` 4 gu. Label AABBs are rounded outward to the lattice for collision tests.
- The browser must render with the same font and the same shaping assumptions:
  - the UI ships the bundled fonts through `@font-face` and never falls back to system fonts for diagram text;
  - every text element is styled `font-kerning: none; font-variant-ligatures: none; white-space: pre`.
- Exported SVG chooses one of three text modes (`--text=embed|paths|ref`, see [cli](../cli.md)):
  - `embed` (default): the bundled WOFF2 is embedded as a base64 `@font-face`; its bytes are fixed, so the output stays deterministic;
  - `paths`: glyphs are emitted as outlines read from the bundled font (`ttf-parser`); the `<title>` keeps the text accessible;
  - `ref`: font-family reference only; smallest output, correct only where the font is installed.
  - Until the bundled fonts arrive (M5.5), `ref` is the only mode and the default (§14.1, staging).
- `ascii::render` requires a `Mono` Scene (§14.2). Switching from `classic` to the ASCII view re-runs layout under the `retro` theme.

---

## 4. Box sizing and ports

### 4.1 Box size

For box *i* with `nI`, `nC`, `nO` ports, and `nB = nM + (1 if the box has a call arrow else 0)` bottom ports:

```text
(normative)
h_i = ceil_4( max( box_min.h,
                   (max(nI, nO) + 1) · port_pitch_y,
                   lines(title_i) · line_h + corner_label_h + 2·pad ) )
w_i = ceil_2( max( box_min.w,
                   (max(nC, nB) + 1) · port_pitch_x,
                   text_w(wrap(title_i, w_i)) + 2·pad ) )
```

`ceil_k` rounds up to a multiple of *k* gu. `corner_label_h` is one text row (`line_h`): the corner label never shares a row with the title.

### 4.2 Title wrap

- Break opportunities: spaces, and after `/` and `-`. "Develop/Update a Factory" may break after `Develop/`.
- Algorithm: choose the smallest lattice width `w ≥ max(box_min.w, port-driven width)` such that greedy wrapping at `w − 2·pad` gives at most 3 lines, searching up to `w_max`.
- If 3 lines do not fit at `w_max`, keep `w_max` and allow more lines; the box grows taller. A box name is **never truncated** (§3.2.1.3 boxes-1: "sufficient in size to insert box name").
- A single unbreakable word wider than `w_max − 2·pad` widens the box to fit it.
- Placement inside the box: `classic` weights the block toward the upper left (FR-STYLE-10); other themes center it. The block never overlaps the corner label.

### 4.3 Corner label, DRE and C-number

- **Corner label** inside the lower-right corner: the box number (`fips183`) or the node number (`classic`, `modern`, `retro`), FR-STYLE-07.
- **DRE** below the lower-right corner, right-aligned to the box's right edge (or to the shadow's right edge when a shadow is drawn), §3.3.3-6. The DRE form (node number, C-number, page, or model-note number) is chosen by the model and status (see [metamodel](metamodel.md)). The DRE text block is part of the keep-out region.
- A caller box has no DRE and no shadow; its call arrow signals that detail exists elsewhere (FR-STYLE-06).

### 4.4 Port positions

For a side of length `L` (gu) with `n` ports and rank `k ∈ 1..n`, with lattice step `g` (2 gu on top/bottom, 4 gu on left/right):

```text
(normative)
offset_k = snap_g( L · k / (n + 1) )       // exact rational, rounded half-down to a multiple of g
```

- Because `L ≥ (n+1)·port_pitch` and `port_pitch ≥ 2g`, consecutive ports differ by at least `port_pitch − g` and every port is at least `clear` from the nearest corner (FR-LAYOUT-05; checked by `G-CORNER`).
- **Rank order** (D5): C1 leftmost, I1 and O1 topmost, M1 leftmost. The **call port is always the rightmost bottom port**, after all mechanisms (FR-LAYOUT-23).
- Port ranks are semantic: on a decomposed box they are the ICOM codes of the child diagram (§3.3.2.8). Only `ReorderPorts` changes them.

---

## 5. Placement (staircase)

### 5.1 Staircase

```text
(normative)
x_1 = m_L,                  y_1 = m_T
x_{i+1} = x_i + w_i + gx_i   y_{i+1} = y_i + h_i + gy_i
```

- `w_i` and `h_i` are the width and height of box *i*'s **keep-out region** (§3.2: the box, its shadow band and its DRE block), because every clearance is measured from the keep-out region. For a box with no shadow and no DRE that is the box itself.
- `gx_i` is the width of vertical channel **V_i** (between box *i* and box *i+1*); `gy_i` is the height of horizontal channel **H_i**.
- `gx_i = 2·clear + max(1, d(V_i))·pitch_x` and `gy_i = 2·clear + max(1, d(H_i))·pitch_y`, where `d` is the number of tracks assigned in the channel (§7), plus any nudge (§5.5).
- **Margins** `m_L, m_T, m_R, m_B` are the margin channels V_0, H_0, V_n, H_n. Each is the largest of:
  - `margin`;
  - `2·clear + d·pitch` for its tracks;
  - the label allowance of the frame edge (rounded up to the lattice): the widest input label (left), the widest output label (right), the control cascade height `n_top·line_h + 2·pad` (top), the mechanism cascade height (bottom), each plus the squiggle width and the ICOM code width.

### 5.2 Placement iteration (SR-40)

The old claim that placement "terminates in exactly 2 passes" is withdrawn. The iteration is:

1. **Iteration 1.** Place with the minimal gaps (`d = 1` in every channel). Run global routing (§6), which fixes each arrow's **channel assignment**. Run track assignment (§7).
2. **Iteration k ≥ 2.** Global routing is **frozen**: the channel assignment from iteration 1 is reused. Set each gap to `max(previous gap, required gap)`, so **gaps only grow**. Re-place. Re-run track assignment on the new coordinates, and keep the new assignment for a channel only if it uses no more tracks than the previous one.
3. Stop when no gap changes, or after **3 iterations**.
4. If the cap is reached with a channel still short of space (only possible through bump-to-fillet track shifts, §10 step 4), apply the required gaps one last time *without* reassigning tracks and emit **`L-LAYOUT-OVERFLOW`** (`reason=iterations`). The Scene is still valid.

**Lemma (expansion preserves non-overlap).** Growing a gap moves every coordinate past it by a constant Δ ≥ 0. That map is monotone and expansive (`f(b) − f(a) ≥ b − a`), so two channel intervals that were at least `clear` apart stay at least `clear` apart, and the exit order at each channel side is preserved. A track assignment that was feasible stays feasible.

*Consequence:* with the "never more tracks" rule, the track count per channel is non-increasing and the gaps are non-decreasing across iterations. Iteration 2 already satisfies every gap, so iteration 3 is reached only when §10 step 4 adds a track.

### 5.3 Fit to page

When `LayoutInput.page` is set (print and PDF; see [cli](../cli.md)) and the staircase exceeds the page's message field at zoom 1:

1. reduce `pitch_y` to 4 gu (one row; `port_pitch` is not reduced, because §4.4 needs it ≥ 2 lattice steps) and re-run §5.2;
2. if it still overflows, the renderer scales the SVG uniformly through `viewBox` (the Scene is unchanged) and layout emits `L-LAYOUT-OVERFLOW` (`reason=page`).

Without a page, the frame grows to fit the content and Fit does not apply. Fit can make boundary labels collide; those cases emit `L-LABEL-FIT` (§11.4).

### 5.4 Visibility lemma

**Lemma (staircase visibility).** In a strict staircase, each box's left side is visible from the left frame, its top from the top frame, its right side from the right frame and its bottom from the bottom frame. *Proof sketch:* for box *j*'s left side, the strip `[y_j, y_j + h_j]` left of `x_j` meets only boxes `i < j`, and these satisfy `y_i + h_i < y_j`. The other sides are symmetric. ∎

*Consequence:* a boundary arrow whose role matches its side (I left, C top, O right, M bottom) *can* be a straight segment with 0 bends. It *is* straight unless FR-LAYOUT-22 (frame order by code) forces a dog-leg in the margin channel. The two agree whenever the codes on each edge increase in staircase order; the Auto-Arrange proxy rewards that (§12.2).

### 5.5 Nudges

`LayoutHints` may hold, per box key, a nudge `(dx, dy)` of 0–4 lattice steps. A nudge only **adds** space before the box (`gx_{i−1} += dx·2 gu`, `gy_{i−1} += dy·4 gu`). It therefore cannot break the staircase or the expansion lemma, and it never moves a box up or left. Nudges are an optional **[Extension]**; v1 may ship without them.

### 5.6 Higher context diagrams (FR-LAYOUT-26; stretch epic)

- An A-1 diagram has 3–max boxes, one of which is the A0 box with box number 0 (§3.4.2). An A-n diagram (n > 1) has one box numbered −(n−1) (§3.4.3). FIPS says each is "syntactically… an ordinary detail diagram".
- Layout treats them as ordinary diagrams. The special box occupies one diagonal slot like any other; its slot is semantic (author's choice), and Auto-Arrange may move it.
- Corner label: `0` or `−(n−1)` under `fips183`; the node number (`A0`, `A-1`, …) in other themes.
- The special box is decomposed (by the A0 diagram, or by A-(n−1)), so it gets a DRE and, in shadowing themes, a drop shadow. The other boxes on a context diagram are normally leaves.
- Boundary arrows: on an A-n that has a parent context (A-(n+1)), they carry ICOM codes as usual. On the highest-level context diagram (parental context "NONE", C.4.1.5) there is no parent box, so boundary arrows carry no codes, as on A-0. This reading is flagged as an open question.
- The form's Context field shows the parent sketch, or `NONE` for the highest level (§14.4).

---

## 6. Global routing

### 6.1 Channels

- Vertical channels `V_0 … V_n`: `V_0` is the left margin, `V_i` lies right of box *i*, `V_n` is the right margin.
- Horizontal channels `H_0 … H_n`: `H_0` is the top margin, `H_i` lies below box *i*, `H_n` is the bottom margin.
- The column band of box *j*, outside the box's own rows, is passable above and below the box (by §5.4).

### 6.2 Templates

Each arrow is routed by a template chosen by its class `(src i, dst j, dst role)`. `c` is a vertical channel index; `r` is a horizontal channel index.

| Class | Name | Segments | Bends | Free params |
|---|---|---|---|---|
| `i<j`, I | forward input | E from O_i → V_c ↓ to y(I_j) → E into I_j | 2 | `c ∈ [i, j−1]` (prefer `j−1`) |
| `i<j`, C | forward control | E from O_i to x(C_j) → ↓ into C_j | 1 | — |
| `i<j`, M | forward mechanism | E → V_c ↓ to H_j → E to x(M_j) → ↑ into M_j | 3 | `c ∈ [i, j−1]` (prefer `j−1`) |
| `i>j`, C | feedback control ("up and over") | E → V_i ↑ to H_r → W to x(C_j) → ↓ | 3 | `r ∈ [0, j−1]` (prefer `j−1`) |
| `i>j`, I | feedback input ("down and under") | E → V_i ↓ to H_r → W to V_c → ↑ to y(I_j) → E | 4 | `r ∈ [i, n]` (prefer `i`), `c ∈ [0, j−1]` (prefer `j−1`) |
| `i>j`, M | feedback mechanism ("down and under") | E → V_i ↓ to H_r → W to x(M_j) → ↑ | 3 | `r ∈ [i, n]` (prefer `i`) |
| `i=j`, C | self control loop | E → V_i ↑ → H_{i−1} W → ↓ | 3 | — |
| `i=j`, I | self input loop | E → V_i ↓ → H_i W → V_{i−1} ↑ → E | 4 | — |
| `i=j`, M | self mechanism loop ("down and under") | E → V_i ↓ to H_r → W to x(M_i) → ↑ | 3 | `r ∈ [i, n]` (prefer `i`) |
| boundary, matching side | straight | one segment | 0 | — |
| boundary, frame-order dog-leg | dog-leg | from the frame entry along the side's margin channel (`V_0` for I, `H_0` for C, `H_n` for M) to the port's row or column | 2 | — |
| boundary, role shift | dog-leg | an entry inside the margin channel it turns into (at least `clear` from the boxes) runs straight along that margin as a fixed run; any other entry turns into the margin channel beside its frame edge, then travels the margin channels `V_0`, `H_0`, `H_n` to the one beside the port's side, then into the port | 1–4 | — |
| `i →` boundary O | output exit | E from O_i to the right frame; if frame order moves the exit, via `V_n` | 0 or 2 | — |
| call | call (FR-LAYOUT-06, -23) | ↓ from the call port through the column band below box *i* to the bottom frame | 0 | — |
| "to all" | stub (FR-LAYOUT-18) | short stub to the label "to all ‹concept›"; no per-box routing | 0 | — |

### 6.3 Cost

```text
(normative)
cost(route) = 100·bends + 1·length_gu + 40·crossings + 25·overflow(channel) + 10·deviation_from_preferred
```

Crossings are counted between non-sibling routes; branches and leaves of one bundle share a trunk (§9). `overflow(channel) = max(0, density − 3)` spreads load across channels; gaps grow to fit any density (§5.2), so overflow is a preference, not a capacity limit.

A candidate is **rejected** (infinite cost) if it:
- enters a keep-out region or passes closer than `clear` to one;
- has an interior segment shorter than `2·r_fillet` (4 gu) or a terminal segment shorter than `clear` (FR-LAYOUT-24, §8.2);
- lies collinear with and overlapping a non-sibling segment.

The keep-out rule exempts the box at either end of the route: a terminal segment touches its own box at the port and is rejected only if it enters the box's interior (FR-LAYOUT-05). Global routing puts every channel run on its channel's center line, because track assignment (§7) spreads the runs later. The collinear rule therefore applies only to fixed segments, the ones outside channel runs; overlapping runs in a channel are what its density counts.

**Frame entries (FR-LAYOUT-22).** Before routing, each boundary arrow gets its point on the frame edge. Its *natural* coordinate is the port's row or column for a matching side. For a role shift, it is track 0 (§7.2) of the margin channel it turns into: `H_0` for I into C, `H_n` for I into M, and `V_0` for C or M into another role. The natural coordinates on one frame edge are sorted and handed out in ICOM code order. Uncoded `[ ]` border ports keep their own. Then, in coordinate order, every entry moves at least one track pitch (`pitch_y` on I and O, `pitch_x` on C and M) past the previous one, so role-shift entries land on that margin's track coordinates. Entries inside `V_0` on the bottom edge start one pitch past those on the top edge, so the two never share a `V_0` track. A backward pass then keeps every entry at least `clear` from the frame corners. These passes keep the order. An arrow whose entry differs from its port's coordinate routes as a dog-leg.

### 6.4 Search (implementation choice)

The routing order below is an idef0-rs implementation choice. It is deterministic, and it is not the FIPS B.2.4 drafting order (which governs placement, FR-LAYOUT-21).

1. **Order:**
   1. matching-side boundary arrows, straight output exits included (their geometry is fixed by §5.4);
   2. internal arrows by descending constraint weight: bundle leaf count × role weight (control 3, input 2, mechanism 1);
   3. then by class priority: forward C, forward I, feedback C, feedback I, M (a self loop ranks with feedback of its role);
   4. then the remaining boundary arrows (dog-legs), then call arrows and "to all" stubs (M5.2);
   5. ties: source rank, then arrow id ([metamodel](metamodel.md) identity rules).
2. For each arrow, enumerate the template parameters (at most n² ≤ 49 choices) and pick the minimum cost given the arrows already placed. Ties break by parameter tuple order.
3. **Rip-up and reroute:** up to `K = 3` passes; each pass removes and re-inserts every arrow in order. Stop early when the total cost does not decrease.

The result is the channel assignment that §5.2 freezes after iteration 1.

---

## 7. Track assignment (SR-41)

Each channel holds **channel segments**: the part of a route that runs along the channel, as an interval `[a, b]` on the channel axis, plus the side (near or far) through which each end exits.

### 7.1 Constraints

- **Hard VCG edges.** Two segments that exit on opposite sides at the same lattice row (vertical channel) or column (horizontal channel) would overlap collinearly unless ordered. The segment exiting toward the near side must take the nearer track: add the hard edge `A ≺ B` ("A on a lower-index track than B"). Track 0 is the one nearest the lower-numbered box.
- **Cycles.** If the hard VCG has a cycle, break it with a **dogleg**: split one segment at a free lattice row (or column) into two half-segments on different tracks, joined by a jog of one `pitch` (≥ 4 gu, so §8.2 holds). The segment to split is the one with the longest interval; ties by arrow id.
- **Soft barycenter edges.** For each pair of overlapping segments, compute `bary = mean(exit coordinates on the orthogonal axis)`. The segment with the smaller barycenter should take the lower track. Soft edges are added greedily, in order of decreasing barycenter separation (ties by arrow id). An edge is skipped if it would create a cycle, or if it would raise the longest VCG path above `max(density, longest hard path)`.

Barycenter ordering is thus an **input** to the track assigner (as soft edges), not a post-step.

### 7.2 Constrained left-edge

1. For track `t = 0, 1, …`: repeatedly choose, among unplaced segments whose VCG predecessors (hard and soft) are all on tracks `< t`, the one with the smallest `a` that starts at least `clear` after the last occupant of track `t`. Ties break by barycenter, then arrow id. When nothing fits, move to track `t+1`.
2. **Sifting:** try adjacent-track swaps that respect the hard VCG and the clearance. Keep a swap if the exact crossing count (§10.1) decreases. At most `4·m` swaps per channel, in a fixed order.
3. Track coordinates: `x_t = start(V) + clear + t·pitch_x` and `y_t = start(H) + clear + t·pitch_y`. They are on the lattice by construction.

- The track count is **≥ max(density, longest hard VCG path)**. It is not guaranteed minimal: constrained channel routing with doglegs is NP-hard in general, and this is a heuristic.
- The track count of each channel feeds the gap sizing in §5.2.

Output: every arrow becomes an orthogonal polyline `P = [p_0 … p_m]` of alternating horizontal and vertical segments, with all vertices on the lattice.

### 7.3 Implementation notes

These choices are idef0-rs implementation details, not FIPS rules.

- **Channel order.** Vertical channels are assigned first, `V_0 … V_n`, then `H_0 … H_n`. A vertical segment ending in a horizontal channel that is not assigned yet takes that channel's whole band as its interval there, so no two such segments share a track wherever the horizontal tracks land. Horizontal channels then see the exact column of every end. Hard edges come only from ends whose coordinate is exact.
- **Fixed runs.** A route segment that is not a channel run but runs along a channel inside its band is a *fixed run*: a frame entry, such as a role shift running straight along `V_0` (§6.3). A fixed run blocks any track within one `pitch` of it wherever their intervals conflict. It counts toward the channel's tracks, so the gap grows to hold it. A segment whose end leaves on the far side of a fixed run, at the fixed run's coordinate, must take a track beyond it.
- **Dogleg point.** The split coordinate is the free lattice row (or column) nearest the segment's midpoint. It is at least `2·r_fillet` from both ends. It avoids every exact end in the channel, every segment that crosses the band, and every perpendicular channel's band, where tracks not yet assigned could land. The jog spans at least one `pitch`, more when other segments sit between the two halves. If no row is free, the segment's hard edges are dropped and the channel reports it (`dropped`); no fixture does this.
- **Feasibility.** A track assignment is feasible when it keeps every hard edge, every fixed-run block and bound, and the `clear` spacing between segments that share a track. Sifting tries only feasible swaps. The §5.2 rule that keeps the previous iteration's assignment applies only when that assignment is still feasible on the new coordinates.
- **Crossings** are counted exactly between non-sibling polylines (proper crossings of a horizontal and a vertical segment). The committed baseline is `fixtures/layout/baselines/crossings.json` (testing §11).

---

## 8. Fillets

### 8.1 Geometry

For each interior vertex `p_k`, with incoming and outgoing unit axis directions `d_in`, `d_out` and adjacent segment lengths `a = |p_k − p_{k−1}|`, `b = |p_{k+1} − p_k|` (in su):

```text
(normative)
r_k   = min(r_fillet, a/2, b/2)          // a or b is the full length for a terminal segment (minus head_len at the destination)
s     = p_k − r_k · d_in                 // fillet start
e     = p_k + r_k · d_out                // fillet end
cross = d_in.x · d_out.y − d_in.y · d_out.x
sweep = if cross > 0 { 1 } else { 0 }    // SVG y-down: positive cross = clockwise turn
SVG:  L s.x s.y  A r_k r_k 0 0 sweep e.x e.y
```

- FIPS basis: "Arrows that bend shall be curved using only 90 degree arcs" (§3.2.1.3). Fillets are required by the standard. Boxes keep square corners.
- The first and last segments are not filleted at ports.
- The arrowhead is a filled triangle of length `head_len` at the destination.

### 8.2 Radius floor (FR-LAYOUT-24, SR-43)

- **Hard property:** at zoom 1, every fillet and branch radius satisfies `r_min ≤ r_k ≤ r_max` (6–10 px).
- The router enforces this by construction, not by clamping:
  - interior segments shorter than `2·r_fillet` (4 gu) are rejected in §6.3; in §7, a track choice that would create one is nudged by one `pitch`;
  - every terminal segment is at least `clear` = 4 gu = `r_fillet + head_len`;
  - because vertices lie on the lattice, every nonzero vertical difference is ≥ 4 gu; horizontal differences of 2 gu are exactly the ones the router rejects.
- In practice therefore `r_k = r_fillet`. The `min(…)` in §8.1 is a safety net, and the property test asserts `r_k ≥ r_min` for every fillet of every fixture and generated model (`G-ARC`).
- On-screen zoom scales radii uniformly (4 px at zoom 0.5). The 6–10 px requirement applies at zoom 1, which is the export scale.

---

## 9. Bundles

1. Route each **leaf** independently, with a *sharing discount*: a segment collinear with and overlapping an already-placed segment of a sibling in the same bundle costs 0 length and 0 crossings.
2. Merge the collinear overlapping prefix (fork) or suffix (join) into the **trunk**. The **branch point** `q` is the last shared vertex.
3. **Curved split** (FR-LAYOUT-04): with trunk direction `d_t` and orthogonal branch direction `d_b`, the branch starts at `q − r_branch·d_t` and draws a quarter arc of radius `r_branch` ending tangent to `d_b` at `q + r_branch·d_b`. The sweep uses the §8.1 formula with `d_in = d_t`, `d_out = d_b`. The sibling that continues straight keeps the trunk line.
4. A branch carrying `kind_of` or `part_of` shows its label at the branch with a relation glyph: `⊂` (kind-of) and `◇` (part-of) in vector themes and the `unicode` charset; `<` and `*` in the `ascii` charset.
5. The router never produces a four-way junction. T-junctions occur only at branch points (`G-JUNCTION`). In ASCII, `┼` is therefore never emitted, and a crossing is never ambiguous (§10).
6. **"To all"** (FR-LAYOUT-18): an arrow whose leaves reach every box on the diagram at the same role is drawn, when the proposal has been accepted, as one stub from its source ending in the label "to all ‹concept›" (B.2.4.3-8). No per-box routes are drawn.

---

## 10. Crossings and bridges

1. **Detection:** a sweep line over x. Events are horizontal segment starts and ends, and vertical segments. Active horizontals are kept in a `BTreeMap<y, seg>`, and each vertical queries `[y_lo, y_hi]`. Complexity `O((n + k) log n)`. Endpoints and branch points shared within one bundle are connections, not crossings.
2. **Rule (FR-LAYOUT-25):** the **vertical** segment carries the bridge and jumps the horizontal, in every Scene and theme.
3. **Bump geometry** at crossing `(x, y)` on a vertical travelling in direction `sy ∈ {+1 down, −1 up}`:

   ```text
   (normative)
   L (x, y − sy·r_bump)
   A r_bump r_bump 0 0 sweep (x, y + sy·r_bump)     sweep = if sy > 0 { 1 } else { 0 }   // always bulges toward +x
   ```

4. **Spacing:** a bump keeps at least `r_bump + clear` from any fillet, branch point, arrowhead or other bump on the same segment.
   - Two crossings closer than `2·r_bump + clear` merge into one elongated hop (arc, line, arc).
   - If a bump would still intersect a fillet, the vertical's track shifts by one `pitch` in §7. This is the only event that can force iteration 3 in §5.2.
5. **ASCII:** the vertical is drawn continuously through the crossing cell and the horizontal is interrupted (`─│─`, or `-|-` in the `ascii` charset). Since junctions are never four-way, this reads unambiguously as "vertical passes over".

---

## 11. Labels and squiggles

### 11.1 What needs a label (FR-LAYOUT-16, SR-05)

A **label unit** is a FIPS arrow segment: a maximal part of an arrow's branching tree between an end, a fork and a join.

- Every label unit needs a noun-phrase label, **unless a single arrow label clearly applies to the arrow as a whole** (§3.2.2.3-3). idef0-rs treats one label as applying to the whole arrow when:
  - the arrow has no branches; or
  - a branch has `rel=all` (all contents follow the branch, §3.3.2.5) and carries no concept of its own, so it inherits the trunk label.
- A call arrow is labeled with the called reference expression (`A42.3`, `TLS/A34.2`, or a list plus a model-note glyph), not a noun phrase (§3.2.2.3-2e).
- **Long arrows** (FR-LAYOUT-17): a label unit longer than 60% of the frame width gets a second label near its destination. A segment longer than 40% of the frame width beyond a branch point gets a mid-segment arrowhead.

### 11.2 Placement strategy

1. **Boundary labels** follow the theme's frame-edge rules (FR-STYLE-01 to -04): cascade on the top and bottom edges, aligned on the left and right edges. Their space is reserved in the margins before placement (§5.1).
2. **Internal labels** use greedy placement in routing order (§6.4) against an AABB index (`rstar`) of keep-out regions, segments, codes and placed labels. Candidates for each label, in order:
   1. above the midpoint of its longest horizontal segment, then below it (the free row between tracks that `pitch_y` provides);
   2. right of the midpoint of its longest vertical segment;
   3. near the source port, then near the destination port.
3. **Fallbacks**, in order, when no candidate is collision-free:
   1. wrap to 2 lines and retry;
   2. bounded spiral search on the lattice (radius ≤ 12 gu) and connect with a squiggle;
   3. abbreviate with `…` (the full text stays in `<title>`) and retry;
   4. place at the first candidate anyway and emit **`L-LABEL-FIT`** for that label.

### 11.3 Squiggles, codes and glyphs

- **Squiggle:** a short zig-zag polyline from the label's anchor corner to the nearest point of its label unit (`~` in ASCII). When it is drawn is theme-dependent (FR-STYLE-05). §3.2.2.3-4 requires it "unless the arrow/label relationship is obvious"; "obvious" means the label is adjacent (within one `line_h` of the segment and overlapping it along the segment axis).
- **ICOM codes** (FR-LAYOUT-08) sit at the unconnected end, inside the frame. The arrow starts or ends short of the frame by the code width:
  - `I1 ──►` at the left edge;
  - `──► O1` at the right edge;
  - `C1` above the top end;
  - `M1` below the bottom end.
  A-0 has no codes (§3.4.2).
- **Tunnel glyphs** (FR-LAYOUT-19): `( )` around the arrow end at a box. At a border end, `[ ]` (an idef0-rs glyph, **[Deviation]**, uncoded per §3.3.2.9), or `( )` under the `fips183` theme. See [metamodel](metamodel.md) for the `[I1]` notation.
- **Model notes** (§3.3.4.4): the note number in a small box, rendered `|n|` in ASCII (the FIPS alternative notation). It is attached to each point of application by a squiggle.
- **Accessibility:** each box and arrow element carries `<title>` (name) and `<desc>` (description) children (FR-NAME-08). Descriptions are converted to escaped plain text, with no raw HTML (see [agents](agents.md) threat model). The ASCII view shows them in the status-bar info line.

### 11.4 Guarantees (SR-50)

- Greedy placement cannot guarantee zero overlaps, and the Fit step (§5.3) can shrink the space the cascade needs.
- **Acceptance criterion:** for every label, either it overlaps no other label, keep-out region or unrelated segment (knockouts in FR-STYLE-11 excepted), **or** an `L-LABEL-FIT` diagnostic was emitted for it. `G-LABEL` checks this on every fixture and generated model.

---

## 12. Auto-Arrange

Auto-Arrange chooses the two degrees of freedom that FIPS leaves to the author and that change numbering: **box order** (box numbers) and **port order** (ICOM codes on child diagrams). It is not a layout phase: it produces a model `Patch` (D5).

### 12.1 What it is and is not (SR-39)

- Step 1 is an **exhaustive search over box orders for a proxy objective**, a combinatorial cost on the constraint graph. It runs **no routing pass per permutation**.
- Step 2 then orders ports **per box side**, given the box order from Step 1. It is exact for sides with ≤ 6 arrows and heuristic above that.
- The two steps are **sequential, not joint**. The result is optimal for the proxy, not for the rendered diagram's crossings or bends. The spec makes no "optimal layout" claim.

### 12.2 Step 1: box order (FR-LAYOUT-02, -21)

Build the constraint multigraph over the diagram's *n* boxes. Each arrow `i.O → j.R` adds an arc of weight `w = w_R × leaves`, with `w_C = 10`, `w_I = 6`, `w_M = 6`; `leaves` is the bundle leaf count. Call arrows add no arc. For a permutation π (positions `0 … n−1`):

```text
(normative)
cost(π) = 100 · Σ_{arcs i→j, π(i) > π(j)} w              // feedback arcs: dominance (B.2.2.1-2, B.2.4)
        +   1 · Σ_{arcs i→j} w · |π(i) − π(j)|             // keep tightly coupled boxes adjacent
        +   2 · Σ_{boundary I/C arcs → j} π(j)             // boundary-fed boxes drift up-left (B.2.4: boundary arrows next)
        +   2 · Σ_{boundary O arcs i →} (n − 1 − π(i))     // output-feeding boxes drift down-right
        +   4 · inversions(boundary codes, staircase order)  // FR-LAYOUT-22 agrees with straight arrows (§5.4)
```

- The term order mirrors B.2.4: heavy constraint arcs dominate, boundary affinity comes second, and the remaining arrows matter only through the span term (FR-LAYOUT-21).
- `inversions` counts, per frame edge, the pairs of boundary arrows whose code order disagrees with the staircase order of the boxes they reach.
- **Branch and bound** (deterministic):
  - Depth-first search fills positions 0, 1, …, trying candidate boxes in current-position order, so the first leaf reached is the current order.
  - The lower bound of a partial assignment (placed set P, next position k) is the sum of:
    - every term fixed by P;
    - `100·w` for each arc from an unplaced box to a placed one (certain feedback);
    - `w·(k − π(p))` for each arc between a placed box p and an unplaced one;
    - `2k` for each unplaced boundary-fed box.
  - A subtree is pruned when its bound is ≥ the best cost. The incumbent starts as the current order, and it is replaced only by a **strictly** lower cost.
  - Ties therefore keep the current order (stability), and otherwise resolve to the first permutation in DFS order.
- At most n! = 5040 leaves (n = 7). §16.3 has the cost arithmetic.

### 12.3 Step 2: port order (FR-LAYOUT-13)

1. Compute one geometric layout (§4–§7) of the new box order with the current port order, to get the coordinates of each arrow's far end.
2. For each box side, in order (box number, then side I, C, O, M), choose the arrow order minimizing:
   - the number of inverted pairs between port order and far-end order along the side axis (a crossing estimate), plus
   - 50 per forked arrow whose relative ICOM position differs from its position on the sibling destinations already ordered (FR-LAYOUT-13).
3. Sides with ≤ 6 arrows: all permutations (≤ 720). Larger sides: barycenter sort, then adjacent-swap sifting until no improvement. One sweep only; there is no fixpoint over sides.
4. Fixed points: the call port stays last on the bottom side. The diagram's own boundary codes are not touched, because they belong to the parent box and change only when the parent diagram is arranged.

### 12.4 Step 3: proposals

Each proposal is offered and never auto-applied, even under the automatic triggers:

- **Merge** open boundary arrows that carry the same concept into one fork or join (FR-LAYOUT-14).
- **Bundle** parallel arrows with the same source and destination under a more general concept (FR-LAYOUT-15). Use an existing `kind_of`/`part_of` parent concept if there is one; otherwise prompt for a new concept. Validation's `L-BUNDLE-SUGGEST` uses the same detector.
- **"To all"** when an arrow reaches every box at the same role (FR-LAYOUT-18).

### 12.5 Output, acceptance guard and determinism

- Output: one `Patch` of `MoveBox` and `ReorderPorts` commands, plus accepted proposals. `ReorderPorts` on a decomposed box atomically re-codes the child diagram's boundary ports (balance is preserved by construction) and records port renames (`A21/I2 → A21/I1`) in the RenameMap (see [metamodel](metamodel.md); `idef0 sync` in [ingestion](ingestion.md)).
- **Guard:** lay out the diagram before and after. Accept the patch only if the rendered metrics `(feedback arcs, crossings, bends)` strictly improve in lexicographic order. Otherwise return "no improvement" and an empty patch. This makes Auto-Arrange idempotent and never worse than the input, despite the proxy.
- The delta summary (`feedback arcs 3→1, crossings 7→2, bends 18→12`) comes from these two layouts.

### 12.6 Triggers and UX (D5)

- **Explicit:** the `=` key shows a ghost preview with the delta summary; `Enter` accepts and `Esc` discards ([editor-ux](editor-ux.md)). CLI: `idef0 arrange <model> <node> [--apply]` ([cli](../cli.md)).
- **Automatic, only:**
  - for AI decomposition proposals, before review: the reviewer sees the arranged proposal, which still needs human acceptance (D15); and
  - for imported models without layout, once at import.
- The old "child diagrams seeded by `Decompose`" trigger is dropped: a new child has 0 boxes.
- **Suggestion:** in the UI, Auto-Arrange runs in the Web Worker after 500 ms of idle time, never on the keystroke path. When the guard would accept with an improvement above a configurable threshold, the info line shows `= arrange: −2 feedback, −5 crossings`.
- A progress indicator appears when a run exceeds 200 ms (§16.3).

---

## 13. Themes (FR-STYLE)

### 13.1 Theme parameters

| Parameter | `modern` | `classic` | `fips183` | `retro` |
|---|---|---|---|---|
| Output | SVG | SVG | SVG | ASCII/ANSI |
| Text metric (§3.3) | `Mono` | `Serif` | `Mono` | `Mono` |
| Boundary label rules (FR-STYLE-01..04) | on | on | on | on |
| Squiggles (FR-STYLE-05) | when not adjacent | always | when not adjacent | when not adjacent |
| Decomposed marker (FR-STYLE-06) | shadow + DRE | shadow + DRE | DRE | shadow + DRE |
| Corner label (FR-STYLE-07) | node number | node number | box number | node number |
| Border tunnel glyph | `[ ]` | `[ ]` | `( )` | `[ ]` |
| Label knockout (FR-STYLE-11) | on | on | on | on |
| Diagram form (FR-LAYOUT-20) | full | full | full | one-line header and footer |

- `retro` and `modern` have identical layout parameters, so they share one Scene. The `` ` `` key toggles between them without re-layout ([editor-ux](editor-ux.md)). From `classic` or `fips183`, the toggle re-runs layout as `retro`.
- Default theme: `modern` under the `extended` profile, `fips183` under the `fips183` profile (D8). The user can override it with `:theme <name>`. The theme changes only rendering, never diagnostics, except the theme-dependent `G-*` checks (§15).
- The bundled fonts must be OFL or compatible with MIT OR Apache-2.0 (D12). The retro font choice is an open question.

### 13.2 Classic conventions (reference image)

The reference is [factory-a0.png](../research/examples/factory-a0.png), the A-0 diagram *Develop/Update a Factory* (KBSI/AI0Win style).

| ID | Convention | Rule | Themes |
|---|---|---|---|
| FR-STYLE-01 | Top-edge labels cascade | Label k right of arrow k, at `y = frame_top + pad + (k−1)·line_h` | all |
| FR-STYLE-02 | Input labels above the arrow, left-aligned | Anchor `(frame_left + pad, y_arrow − gap)` | all |
| FR-STYLE-03 | Output labels above the arrow, right-aligned | Anchor `(frame_right − pad, y_arrow − gap)` | all |
| FR-STYLE-04 | Bottom-edge labels cascade upward | Mirror of FR-STYLE-01 from `frame_bottom` | all |
| FR-STYLE-05 | Squiggle links each label to its arrow | `classic`: always; others: when not adjacent | per theme |
| FR-STYLE-06 | Drop shadow iff the box has a child diagram | Caller boxes and leaves get none | `modern`, `classic`, `retro` |
| FR-STYLE-07 | Corner label shows the node number **[Deviation]** | `fips183` shows the box number (§3.3.3-5) | per theme |
| FR-STYLE-08 | A-0 boundary arrows run straight from frame edge to box, without codes | 0 bends (§5.4, §3.4.2) | all |
| FR-STYLE-09 | Ports evenly spaced in rank order | §4.4 formula | all |
| FR-STYLE-10 | Box name wraps inside the box, weighted upper left; filled arrowheads | Liberation Serif in `classic` | `classic` |
| FR-STYLE-11 | A cascaded label interrupts the later arrows it crosses | Knockout gap of `pad` around the label AABB | all |

Rationale and edge cases:

- **FR-STYLE-01 / -04.** The cascade covers every boundary arrow on that frame edge, in frame order (FR-LAYOUT-22), not just those of one box. `gap` is `pad`. A label never crosses its own arrow or any arrow to its left. It may cross arrows k+1…n only through a knockout (FR-STYLE-11). A label that would cross the frame's right edge wraps; if it still does not fit, it is abbreviated and `L-LABEL-FIT` is emitted (§11.4). There is **no guarantee** that the cascade fits once Fit (§5.3) shrinks the pitch.
- **FR-STYLE-05.** FIPS §3.2.2.3-4 makes the squiggle optional when the link is obvious; the classic look draws it always.
- **FR-STYLE-06** (confirmed by the user). The shadow is drawn **iff the box has a child diagram** (`decomposed`). A caller box (FR-CALL-02) has no child diagram, so it has no shadow; its downward call arrow signals that detail exists elsewhere. "Leaf" is defined structurally in the [glossary](../glossary.md). The shadow is only how `classic`, `modern` and `retro` render "decomposed". `fips183` relies on the DRE alone (§3.3.3-6). `retro` draws the shadow as a one-cell band of `▒` (`unicode`) or `#` (`ascii`) along the right and bottom edges. Arrows end on the box perimeter (FR-LAYOUT-05) and are drawn over the shadow band.
- **FR-STYLE-07.** Under the `fips183` theme the corner label is the box number, and `G-BOX` checks it (FR-LAYOUT-03).
- **FR-STYLE-09.** In the reference, the single mechanism sits under the first control rather than centered. That is an AI0Win artifact; idef0-rs centers it per §4.4. The fixture asserts order, not offsets.
- **FR-STYLE-10.** The reference shows no DRE under A0. `classic` still draws the DRE because §3.3.3-6 is normative. The fixture does not assert DRE absence.
- **FR-STYLE-11.** The reference draws labels on an opaque background, so arrows 2…6 visibly start below the labels that cross them. idef0-rs reproduces this as a knockout: the crossed arrow segment is drawn with a gap of `pad` around the label AABB. A knockout is not a crossing and gets no bridge.

### 13.3 Theme acceptance: factory reference fixture

`fixtures/reference/factory-a0.idef0` (created by the metamodel author; see [testing](../testing.md)) models the reference. Rendering it under `classic` must satisfy these structural assertions. Pixel identity is not required.

1. **Structure:** the A-0 diagram, one box, box number 0, title "Develop/Update a Factory"; 14 boundary arrows; no ICOM codes rendered.
2. **Port order:**
   - top, left to right: Budget, Project Time, Reference Factory Information, Administrative Information, Goal, Regulation (C1–C6);
   - left, top to bottom: Market Information, Product Information, Resource Information (I1–I3);
   - right, top to bottom: New or Updated Factory Specification, Performance Result, Corrective Action & Preventive Action Plans, Standard Operating Procedures (O1–O4);
   - bottom: Manufacturing Software System (M1).
3. **Bends:** every arrow is a single segment from the frame edge to the box (0 bends).
4. **Label sides and alignment:**
   - control labels right of their arrows;
   - input labels above their arrows, left edges equal to `frame_left + pad`;
   - output labels above their arrows, right edges equal to `frame_right − pad`;
   - the mechanism label right of its arrow.
5. **Cascade:** control-label y is **strictly increasing** from C1 to C6, with step `line_h`; the C*k* label's left edge is right of arrow *k*.
6. **Squiggles:** one per label (14).
7. **Knockouts:** each crossing of label *k* with arrow *j > k* is a knockout. No label AABB intersects another label.
8. **Shadow:** the shadow is present on box 0 **iff** the fixture contains an A0 child diagram. The reference shows one. The fixture itself has only A-0 (its expected diagnostics include `missing_a0_decomposition`), so the rendering test adds a 3-box stub A0 diagram before asserting the shadow ([factory-a0.expected.md §3](../../fixtures/reference/factory-a0.expected.md#3-rendering-test-classic-theme)).
9. **Corner label:** `A0` under `classic`; `0` under `fips183`.
10. **Determinism:** byte-identical SVG on native and wasm32. CI also generates a side-by-side review page (reference PNG next to our PNG) as an informative artifact.

---

## 14. Rendering outputs

### 14.1 Scene and SVG

```rust
// (normative) public surface of idef0-layout
pub struct LayoutInput<'a> { pub theme: ThemeId, pub hints: &'a LayoutHints, pub metrics: &'a Metrics, pub page: Option<PageSize> }
pub struct LayoutResult { pub scene: Scene, pub diagnostics: Vec<Diagnostic> }   // L-LAYOUT-OVERFLOW, L-LABEL-FIT
pub fn layout(model: &Model, node: &NodeId, input: &LayoutInput) -> LayoutResult;

pub struct Scene { pub theme: ThemeId, pub text_metric: TextMetric, pub frame: Frame, pub form: FormGeom,
                   pub boxes: Vec<BoxGeom>, pub arrows: Vec<ArrowGeom>, pub labels: Vec<LabelGeom>,
                   pub squiggles: Vec<Polyline>, pub codes: Vec<CodeGeom>, pub notes: Vec<NoteGeom> }
pub struct ArrowGeom { pub id: ArrowId, pub polyline: Vec<IPoint>, pub fillets: Vec<Fillet>, pub bumps: Vec<Bump>,
                       pub branches: Vec<BranchGeom>, pub knockouts: Vec<Span>, pub head: Arrowhead, pub tunnel: TunnelGlyphs }

pub mod svg   { pub fn render(scene: &Scene, opts: &SvgOptions) -> String; }
pub mod ascii { pub fn render(scene: &Scene, charset: Charset) -> Result<Vec<String>, AsciiError>; }
pub fn check(scene: &Scene) -> Vec<Diagnostic>;   // G-* (§15)
```

- All coordinates are `i32` su. Vectors are in a canonical order: boxes by box number, arrows by arrow id, labels by owner id.
- **SVG:**
  - integer px coordinates;
  - a fixed element and attribute order;
  - every element carries `data-key` (box key or arrow id) for hit-testing and selection, and the UI renders keyed elements;
  - no timestamps and no generator comments carrying a version hash;
  - `--text` modes per §3.3.
- PNG rendering lives in the CLI and server crates (§14.3), not in `idef0-layout`.

**Staging (M4.5).** The first renderer ships the parts that exist so far, and later epics fill in the rest of the surface above:

- `layout` returns `Result<LayoutResult, LayoutError>`. The error cases are "the model has no such diagram" and "the theme is not available yet": `classic` waits for its Serif metrics (M5.5).
- `LayoutInput` carries the theme and the metrics. `hints` arrive with Auto-Arrange (M4.7) and `page` with Fit to page (§5.3).
- The Scene has the page size, the frame, the form, the boxes (title, corner label, shadow, DRE) and the arrows. Each arrow has a polyline and a filled arrowhead at its destination; arrow ids include call arrows (`call @key`). Fillets come with M5.1, bundles with M5.2, bumps with M5.3, and labels, squiggles, ICOM codes, tunnel glyphs and notes with M5.4.
- USED AT lists the callers of the diagram's boxes within the model (FR-CALL-04). Callers in other models appear once workspace rendering passes the index (M2.4).
- The form has fixed band heights: the working band is 5 rows of `line_h` and the identification band 2. Its column widths are fixed fractions of the frame width, and text that does not fit its cell is cut with an ellipsis. The CONTEXT sketch uses the scale 1/8, or a smaller power of two when that does not fit.
- SVG text mode `ref` (`font-family: monospace`) is the only mode until the bundled fonts arrive (M5.5).

### 14.2 ASCII (retro)

- **Input:** a Scene with `text_metric == Mono`; otherwise `AsciiError::ProportionalMetrics`.
- **Cell mapping (exact):** cell `(col, row)` covers `x ∈ [2col, 2col+2) gu`, `y ∈ [4row, 4row+4) gu`. A lattice point `(x, y)` maps to cell `(x/2, y/4)`.
  - A box with corners `(x0, y0)`–`(x1, y1)` draws its border on columns `x0/2 … x1/2` and rows `y0/4 … y1/4`, inclusive.
  - A label anchored at `(x, y)` starts at column `x/2` on row `y/4`. Each grapheme cluster takes `unicode-width` cells (1 or 2); zero-width clusters attach to the previous cell.
  - Fillets, bumps and branch arcs map to the cell of their vertex.
- **Charsets** (SR-44): `unicode` and `ascii`. There is no CP437 mode.

| Element | `unicode` | `ascii` |
|---|---|---|
| Box corners (square) | `┌ ┐ └ ┘` | `+` |
| Box and arrow lines | `─ │` | `- \|` |
| Arrow bends (fillets) | `╭ ╮ ╰ ╯` | `+` |
| T-junctions (branch points) | `├ ┤ ┬ ┴` | `+` |
| Arrowheads | `► ◄ ▲ ▼` | `> < ^ v` |
| Crossing (vertical over) | `─│─` | `-\|-` |
| Drop shadow | `▒` | `#` |
| Relation glyphs | `⊂ ◇` | `< *` |
| Squiggle | `~` | `~` |
| Tunnel glyphs | `( )`, `[ ]` | `( )`, `[ ]` |
| Model note | `\|n\|` | `\|n\|` |

- **Rasterization:** fixed layer order: frame, shadows, arrows, boxes, codes, labels.
  - Line cells keep a 4-bit connectivity mask (N, E, S, W), and the glyph is looked up from the mask. Mask `1111` cannot occur (`G-JUNCTION`); at a crossing the vertical wins.
  - Label cells overwrite line cells only inside a knockout span.
- **Collision check:** while rasterizing, the renderer records conflicts and reports them through `check_ascii` as `G-ASCII`:
  - a cell claimed by two text glyphs;
  - text over a line outside a knockout;
  - a four-way mask.
  On a lattice-correct Scene no conflicts occur, so no snap pass is needed.
- In the browser, the retro view is a `<pre>` with the bundled retro font ([editor-ux](editor-ux.md)). MCP `get_diagram_ascii` uses `retro` + `unicode` ([agents](agents.md)).

### 14.3 PNG

- `idef0 render --png [--scale N]` (CLI and server, native only): SVG → `usvg` → `resvg`/`tiny-skia`.
- `fontdb` is loaded **only** with the bundled fonts (`load_font_data` from embedded bytes, never `load_system_fonts`). The output is then reproducible for a given build and platform.
- **PNG is excluded from byte determinism** (D6): rasterizer versions and float paths may differ across platforms. PNG tests use the perceptual tolerance defined in [testing §5.1](../testing.md#51-what-is-snapshotted).
- Licenses: `resvg`/`usvg` (MIT OR Apache-2.0), `tiny-skia` (BSD-3-Clause), `fontdb` (MIT) pass the D12 `cargo deny` allowlist; versions are pinned in [architecture](../architecture.md).
- PNG is not built for wasm32 in v1 (bundle budget NFR-WASM-01). The UI can rasterize through a canvas if needed.

### 14.4 Diagram form (FR-LAYOUT-20, Annex C.4, informative)

- **Working information (top band):** AUTHOR/DATE/PROJECT, READER NOTES, STATUS (`working`, `draft`, `recommended`, `publication`), READER/DATE, CONTEXT, USED AT.
  - CONTEXT is a sketch of the parent diagram's box layout with the parent box highlighted. It is drawn from the parent Scene's box rectangles, integer-scaled by 1/8. Special values per C.4.1.5: `TOP` for A-0; the A-(n+1) sketch for A-n; `NONE` for the highest context level.
- **Message field:** the diagram frame.
- **Identification (bottom band):** NODE (full node reference, e.g. `FAC/A0`), TITLE (equal to the parent box name, C.4.4), NUMBER (C-number, or page number when published).
- **Publication export** (`status = publication`, or `--publication`):
  - the working band is dropped;
  - if USED AT is non-empty, its contents are copied into the message field as a model note (C.4.1.6), numbered next in the diagram's consecutive note sequence (§3.3.4.4) and placed by §11.
- `retro` renders a one-line header (NODE, TITLE, STATUS) and a one-line footer (NUMBER, USED AT).
- The band heights are fixed multiples of `line_h`. They sit outside the frame, so they do not affect placement.

---

## 15. Geometry checker (G-*)

`idef0_layout::check(&Scene) -> Vec<Diagnostic>` verifies every Hard rule on a produced Scene. It runs:
- in `idef0 render --check`;
- in every layout snapshot test;
- in every property test.

`check_ascii` covers the ASCII raster. **Any `G-*` finding is a layout-engine bug.** CI fails on it for every fixture and every generated model (case counts in [testing](../testing.md)). Codes go into the registry in [validation](validation.md); each has the subject kind shown.

| Code | Subject | Checks | Rule |
|---|---|---|---|
| `G-BOX` | box | Rectangle on the lattice; no box overlap; name block inside with `pad`; corner label inside lower right (box number under `fips183`, node number otherwise); DRE present iff decomposed | FR-LAYOUT-03, FR-STYLE-07 |
| `G-ORTHO` | edge | Every segment horizontal or vertical, alternating | FR-LAYOUT-04 |
| `G-ARC` | edge | Every interior vertex and branch point has an arc with `r_min ≤ r ≤ r_max` | FR-LAYOUT-04, -24 |
| `G-PERIM` | edge | Ends on the box perimeter; no segment enters a keep-out region | FR-LAYOUT-05 |
| `G-CORNER` | port | Port at least `clear` from the box corners | FR-LAYOUT-05 |
| `G-SIDE` | edge | Role matches side; mechanisms point up; call points down from the rightmost bottom port | FR-LAYOUT-06, -23 |
| `G-FEEDBACK` | edge | Feedback C passes above the destination top; feedback I and M pass below the source bottom | FR-LAYOUT-07 |
| `G-DIAG` | diagram | `x_{i+1} > x_i + w_i` and `y_{i+1} > y_i + h_i` in box-number order | FR-LAYOUT-01 |
| `G-CLEAR` | edge | Corners, crossings, labels at least `clear` from keep-out; parallel tracks at least `pitch` apart | FR-LAYOUT-09, -10 |
| `G-ICOM` | edge | Boundary end has a code or a tunnel glyph; no codes or tunnels on A-0 | FR-LAYOUT-08, -19 |
| `G-ICOM-ORDER` | diagram | Boundary arrows ordered by code along each frame edge | FR-LAYOUT-22 |
| `G-LABEL` | edge | Every label unit labeled, except for the §11.1 exemptions; no unreported overlap (§11.4) | FR-LAYOUT-16 |
| `G-LATTICE` | diagram | Vertices, corners, ports, codes and mono label anchors on the lattice | §3.1 |
| `G-JUNCTION` | edge | No four-way junction; T-junctions only at bundle branch points | §9 |
| `G-BRIDGE` | edge | Each crossing has one bump, on the vertical; bump spacing per §10 step 4 | FR-LAYOUT-25 |
| `G-ASCII` | diagram | No ASCII cell conflict (§14.2) | §14.2 |

- Theme-dependent checks read `Scene.theme`. Example: `G-BOX` expects the box number under `fips183` and the node number otherwise.
- **Property tests** (proptest over generated models with 3–7 boxes and up to 40 arrows):
  - zero `G-*` findings;
  - `r_k ≥ r_min` for every fillet;
  - no segment intersects a box interior;
  - the staircase is strict;
  - layout is idempotent (`layout(m) == layout(m)` across runs);
  - native and wasm32 Scenes are identical (golden JSON).
- **Snapshots:** `insta` snapshots of SVG and ASCII (both charsets) for every fixture.

---

## 16. Performance budgets and benchmarks

### 16.1 Budgets (confirmed for layout by ADR 0002)

| Operation | Size | Native | wasm32 (Chromium) |
|---|---|---|---|
| Full layout (§3–§11) | 7 boxes, 40 arrows | ≤ 5 ms p95 | ≤ 15 ms p95 |
| SVG render | same | ≤ 1 ms | ≤ 3 ms |
| ASCII render | same | ≤ 1 ms | ≤ 2 ms |
| Auto-Arrange (§12) | 7 boxes, 40 arrows, worst-case sides | ≤ 300 ms | ≤ 1 s, in the Web Worker |

- Full layout plus render fits inside the keystroke budget NFR-PERF-01 (16 ms p95), together with validation, only if the wasm numbers hold. The **routing spike** (M4.0, [ADR 0002](../adr/0002-layout-budgets-and-worker.md)) measured placement and global routing of `dense-7x40` at 0.55 ms p95 in wasm32 on the reference machine, so **layout runs on the main thread**. M4.5 re-measures full layout plus SVG; if wasm32 p95 exceeds 15 ms, layout moves to the Web Worker and the UI shows the previous Scene until the new one arrives.

### 16.2 Layout cost estimate

- Global routing: ≤ 40 arrows × ≤ 49 template choices × crossing tests against ≤ 200 placed segments ≈ 0.4 M segment tests per pass. With ≤ 3 rip-up passes, that is ≈ 1.2 M. Global routing runs only in placement iteration 1 (§5.2).
- Track assignment: O(m²) per channel for VCG construction and sifting, with m ≤ 40. Negligible.
- Labels: ≤ 80 labels × ≤ 8 candidates × `rstar` queries. Negligible next to routing.
- Native estimate: 1–3 ms. wasm is typically 1.5–3× slower.

### 16.3 Auto-Arrange cost estimate (SR-39, SR-54)

- **Step 1:** ≤ 5040 leaves × ≤ 40 arc terms ≈ 0.2 M term evaluations, before pruning. That is well under 1 ms natively. The old "50 µs per permutation including routing" (5040 × 50 µs ≈ 250 ms native, more in wasm) no longer applies, because no routing runs per permutation.
- **Step 2:** one full layout (≈ 5 ms native), plus ≤ 28 sides × ≤ 720 permutations × ≤ 15 pair checks ≈ 0.3 M operations. Sides with more than 6 arrows use the heuristic, so a 9-arrow side costs O(m²) per sift pass rather than 9! permutations.
- **Guard and delta:** two more full layouts.
- **Expected total:** ≈ 15–25 ms native and ≈ 50–100 ms wasm. The budget of ≤ 1 s wasm leaves headroom for pathological fixtures.
- The **Auto-Arrange spike** (M4.0, [ADR 0002](../adr/0002-layout-budgets-and-worker.md)) measured Step 1 on the worst-case fixture at 1.4 ms p95 in wasm32: the §12.2 bound reached 3 of 5,040 leaves. The UI still runs Auto-Arrange in a Web Worker and shows a progress indicator after 200 ms.

### 16.4 Benchmark policy

- **CI gate:** `gungraun` (formerly iai-callgrind) instruction counts (deterministic; Linux runners with valgrind) for full layout, SVG render, ASCII render and Auto-Arrange on the bench fixtures. A regression beyond the thresholds in [testing §6](../testing.md#6-benchmarks-and-performance-gates) (more than 3 %, or 5 % for the code scan) against the committed baseline fails the job. The baseline is updated only for an intended change.
- **Informative only:**
  - `criterion` wall-clock benches (nightly);
  - wasm timings in headless Chromium through `wasm-bindgen-test`.
  These are published as CI artifacts and compared with the §16.1 budgets, but they do not gate merges, because wall-clock on shared runners is noisy.
- **Bench fixtures** (in [testing](../testing.md)):
  - `fixtures/bench/dense-7x40.idef0`;
  - the dogfood model `fixtures/dogfood/idef0-rs.idef0`;
  - `fixtures/reference/factory-a0.idef0`;
  - an Auto-Arrange worst case (7 boxes, all-to-all constraint arcs, one side with 9 arrows).
