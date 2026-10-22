# idef0-rs Metamodel, Identity, Editing Semantics and DSL

| Field | Value |
|---|---|
| Status | DRAFT v0.2 (2026-10-15) |
| Part of | idef0-rs spec set; index: [docs/PRD.md](../PRD.md); terms: [glossary](../glossary.md) |
| Normative references | **FIPS PUB 183**, *Integration Definition for Function Modeling (IDEF0)*, NIST, 21 Dec 1993 ([local copy](../research/fipspub183.pdf)). "§3.3.3-8" means section 3.3.3, rule 8. FIPS Sections 1–3 are normative; Annexes A–D are informative (FIPS Introduction). |
| Scope | Model elements and their meaning, stable identity, editing semantics, persistence and workspace layout, the `.idef0` DSL, and the FIPS 183 conformance matrix. |

Requirement IDs used here: FR-META, FR-ID, FR-DIAG, FR-BOX, FR-ARROW, FR-CALL, FR-BUN, FR-TUN, FR-NOTE, FR-BAL, FR-NAME, FR-EDIT, FR-WS, FR-XM. Diagnostic codes are listed in §18 and registered in [validation §registry](validation.md#7-diagnostic-registry), which owns severities and the Mangle rules. Rust type sketches live in [architecture](../architecture.md); snippets here are illustrative.

---

## 1. Scope and profiles

### 1.1 Labels used in this document

| Label | Meaning |
|---|---|
| *(no label)* | Required by FIPS 183 normative text (Sections 2–3), cited by clause |
| *informative* | Taken from FIPS Annex A–D guidance; compliance is not required by FIPS |
| **[Extension]** | An idef0-rs addition that FIPS does not define; it never changes FIPS semantics |
| **[Deviation]** | An idef0-rs departure from FIPS; always profiled |

### 1.2 Profiles (D8, D11)

| Profile | Box limit | Extensions | Markers |
|---|---|---|---|
| `extended` (default) | 3–7 per non-context diagram **[Deviation]** | All on | idef0-rs glyphs (`[ ]` border tunnels) |
| `fips183` | 3–6 (§3.3.3-4) | Off, or downgraded to info | FIPS glyphs (`( )` at both tunnel ends) |

- **Profile rule:** no FIPS normative rule is relaxed in either profile. Extensions may add diagnostics. Under `fips183`, every extension rule that would reject a FIPS-valid model is off or info (§17.3).
- **Precedence:** the CLI flag `--profile` beats the model's `profile` field, which beats `[validate] profile` in `.idef0/config.toml`.

---

## 2. Model and metadata

```text
(illustrative)
Model = ⟨Meta, Concepts, Diagrams⟩
Meta  = ⟨name, abbrev, description, purpose, viewpoint, scope, context, author, project, lang, profile⟩
```

| ID | Requirement | Basis |
|---|---|---|
| FR-META-01 | A model has a unique descriptive `name` and an `abbrev` (the model id, FR-ID-01). | §3.4.5; abbrev required is **[Extension]** |
| FR-META-02 | `purpose` and `viewpoint` are required (`L-META-01`, error). | §3.3.1.1; Annex A.2 *informative* |
| FR-META-03 | A model `description` is expected (`L-META-03`). | **[Extension]** (D9) |
| FR-META-04 | A `scope` statement is recommended (`L-META-02`). | **[Extension]**; FIPS sets scope by the A-0 box and arrows (§3.3.1.1) |
| FR-META-05 | `lang` is a BCP 47 tag, default `en`; English-only lints are off for other languages. | **[Extension]**; see [editor-ux §i18n](editor-ux.md#12-internationalization) |

---

## 3. Identity (FR-ID)

Identity is deterministic (D6, D14). Parsing never mints random IDs, and core never reads a clock or RNG.

| ID | Requirement |
|---|---|
| FR-ID-01 | The **model id** is `abbrev`: `[A-Z][A-Z0-9]{0,7}`, unique per workspace (`E-ID-06`). |
| FR-ID-02 | Every model element is addressed by node number for display and by stable key for reference. |
| FR-ID-03 | Every box has a **box key**, written in the file as `key=<slug>`, unique per model (`E-ID-02`). |
| FR-ID-04 | Keys never change on rename, move or renumber. |
| FR-ID-05 | Every concept has a **concept key** (same slug syntax), unique per model (`E-ID-03`); box and concept keys are separate namespaces. |
| FR-ID-06 | Every arrow has a deterministic **arrow id** (§3.4). |
| FR-ID-07 | Ports are addressed by **PortRef** (§3.5). |
| FR-ID-08 | Dotted node input is normalized (§3.6). |
| FR-ID-09 | Every command that changes an identifier records old→new in the Patch's **RenameMap** (FR-EDIT-02). |

### 3.1 Model id resolution

- **Surfaces:** HTTP `/api/models/{id}`, PRD tag `model=<id>`, PRD frontmatter `idef0.model`, code tags `idef0:<id>/…`, MCP arguments and cross-model refs all use the abbrev.
- **Paths:** the workspace index maps abbrev → file path (FR-WS-03). A file path is never an identifier.
- **Rename:** changing `abbrev` is a `SetMeta` command. Its RenameMap carries `OLD/ → NEW/`. References in other files stay unresolved until `idef0 sync --apply` rewrites them (FR-XM-06).

### 3.2 Node numbers and box numbers

- `A-0` is the context diagram. Its single **top box** has box number `0` and node number `A0` (§3.3.3-2/3, §3.3.4.2).
- The diagram that details box `X` has diagram node number `X` (§3.3.4.2). Box *k* on diagram `X` is node `X·k` ("box 2 on diagram A25 is A252"). The children of `A0` are `A1..An` (§3.3.4.2).
- Box numbers are single digits: 1–7 syntactically; the profile limit is enforced by `box_count_high`.
- **Higher context diagrams** `A-n` (n ≥ 1, §3.4.2–3.4.3) hold 3–max boxes. Box numbers follow diagonal position, except the **lineage box**:
  - On `A-1` the lineage box is the top box: it shows `0` and is node `A0` (§3.4.2).
  - On `A-n` (n ≥ 2) it shows `−(n−1)` and is node `A-(n−1)`, the box detailed by diagram `A-(n−1)` (§3.4.3).
  - The other (**environment**) boxes have no node number in FIPS. idef0-rs references them as `A-1.2` (reference notation, §3.3.4.5) and by key **[Extension]**.

### 3.3 Box and concept keys

**Slug rule (normative):**
1. Apply Unicode NFKD and drop combining marks.
2. Lowercase. Replace every run of characters outside `[a-z0-9]` with `-`.
3. Trim leading and trailing `-`. Truncate to 48 characters, at a `-` boundary when one exists.
4. If the result is empty, use `box` (or `concept`).
5. On collision within the model, append `-2`, `-3`, … (lowest free suffix).

- **New boxes:** the command layer mints `slug(name)` through an injected key generator that implements the rule above.
- **Files without keys** parse with derived keys: the slug rule applied in file declaration order. A derived key is stable only once written, so `idef0 fmt` and every server save write keys back.
- **Deleted keys are retired (D17).** `DeleteBox` records the key in a model-level `retired { … }` block, which the formatter keeps sorted. Minting a retired key, or parsing a box that uses one, is error `E-ID-02`. Code tags naming a retired key report `implements_unknown_box`.

### 3.4 Arrow ids

| Case | Id | Example |
|---|---|---|
| Explicit `key=` attribute | The key, preserved verbatim | `credit-check-feed` |
| Plain arrow | `<diag>:<src>-><dst>#<n>` | `A0:1O1->2I1#0` |
| Fork trunk | `<diag>:<src>->*#<n>` | `A0:C1->*#0` |
| Join trunk | `<diag>:*-><dst>#<n>` | `A2:*->O2#0` |
| Bundle leaf | `<trunk-id>/<i>` (0-based, declaration order) | `A0:C1->*#0/1` |

- Endpoints use the canonical forms of §16 with explicit ranks: `1O1`, `2I1`, boundary `I1`, uncoded boundary `[I1]`.
- `<n>` is the 0-based occurrence index among arrows with identical endpoints on the diagram, in declaration order.
- Structural ids change when an endpoint, rank or box number changes; the RenameMap records old→new. Consumers that need a reference that survives edits add `key=`.
- `idef0 fmt` preserves explicit arrow keys and never invents them.

### 3.5 Port references

- `A21/I1` is box A21's input 1. It is also diagram A21's boundary port I1: the same logical port. This slash form is canonical in idef0-rs **[Extension]**.
- The FIPS form `A21I1` is accepted on input (§3.3.4.5 writes `2I1`, `A21.3C2`). FIPS uses `/` only as the model separator, so `MFG/A21/I1` parses as model / node / port.
- Key form: `@validate-order/I1` **[Extension]**, immune to renumbering (not to re-ranking).
- **Uncoded border-tunnel ports** (§3.3.2.9: "it does not have an ICOM code"): fact id `A21/[I1]`, where the number is a local ordinal per diagram side, not an ICOM code (FR-TUN-03).

### 3.6 Reference grammar and normalization

```ebnf
(* normative *)
Abbrev     ::= [A-Z] [A-Z0-9]{0,7}
Key        ::= [a-z0-9] [a-z0-9-]{0,63}
BoxDigit   ::= [1-7]
NodeId     ::= "A-" Nat | "A0" | "A" BoxDigit+
DiagRef    ::= (Abbrev "/")? NodeId (("F" | "T" | "G") Nat?)?     (* §3.3.4.3 suffixes *)
BoxRef     ::= (Abbrev "/")? ( NodeId | "@" Key | NodeId ("." BoxNo)+ )
PortRef    ::= BoxRef "/"? Role Nat | BoxRef "/" "[" Role Nat "]"    (* slash required after @Key *)
NoteRef    ::= (Abbrev "/")? NodeId "." "|" Nat "|"                  (* §3.3.4.5 pipe notation *)
ConceptRef ::= (Abbrev "/")? "@" Key
Role       ::= "I" | "C" | "O" | "M"
BoxNo      ::= "-"? Nat
```

**Dotted normalization** (§3.3.4.5: an embedded period means "look specifically at"):
- `X.k` → `Xk`, e.g. `A21.3` → `A213`, and `A21.3C2` → `A213/C2`.
- Special case: `A0.k` → `Ak`, because the A0 children are A1..An.
- Multi-period forms (`A2.1.3`) are an idef0-rs extension, normalized left to right with the same rule: `A2.1.3` → `A21.3` → `A213`.
- `A-n.k` names an environment box and stays a reference (§3.2).
- Normalization applies in the DSL, PRD tags, code tags, the CLI and the UI command line. Display style is configurable.

---

## 4. Diagrams

```text
(illustrative)
Diagram = ⟨node: NodeId, kind, title, description, status, cnumber, author, date,
           boxes: [Box], arrows: [Arrow], calls, notes⟩
```

| ID | Requirement | Basis |
|---|---|---|
| FR-DIAG-01 | `kind = context` iff node is `A-0`; exactly one box, numbered 0 (`context_box_count`). | §3.3.3-1..3 |
| FR-DIAG-02 | `kind = higher_context` iff node is `A-n`, n ≥ 1; 3–max boxes including the lineage box (`box_count_low/high`). | §2.16; §3.4.2–3.4.3 |
| FR-DIAG-03 | `kind = decomposition` for `A0` and every lower node; 3–max boxes (`box_count_low/high`). | §3.3.3-4 |
| FR-DIAG-04 | Every model has an A-0 (`missing_context`) and an A0 diagram (`missing_a0_decomposition`) with 3–6 boxes (R-BOX-01/R-BOX-04). | §3.3.3-2; §3.3.4.2 |
| FR-DIAG-05 | A diagram's title equals the name of the box it details (`E-TITLE-MATCH`). | §2.48; Annex C.4.4 *informative* ("must precisely match") |
| FR-DIAG-06 | `status ∈ {working, draft, recommended, publication}`; new diagrams start `working`. | Annex C.4.1.3 *informative* |
| FR-DIAG-07 | A C-number is optional; when present it is unique within the model (`L-CNUM-01`). | §2.11; §3.3.1.3-1; Annex C.4.5.1 |
| FR-DIAG-08 | A detailed box shows a DRE beneath its lower-right corner (rendered; §4.4). | §3.3.3-6 |
| FR-DIAG-09 | Parental context: A-0 → `TOP`; A0 → A-1 if present, else A-0; highest A-n → `NONE`; other A-n → A-(n+1). | §3.4.2; §3.4.3; Annex C.4.1.5 |
| FR-DIAG-10 | A-0 boundary arrows have port ids but no rendered ICOM codes and no border tunnels (`context_tunnel`). | §3.4.2 |
| FR-DIAG-11 | On A-1, the top box's ICOMs equal those on A-0 (`higher_context_mismatch`). | §3.4.2 ("still has only the single named A0 top box, with its labeled external arrows") |
| FR-DIAG-12 | v1: environment boxes on A-n cannot be decomposed or called. | idef0-rs limitation (FIPS gives them no node numbers) |

### 4.1 Titles

- Titles of decomposition diagrams are **derived** from the parent box name. The DSL may restate the title; a mismatch is `E-TITLE-MATCH`. Editing a derived title in the UI routes to `RenameBox` on the parent (FR-EDIT-11).
- The A0 diagram title is the top box name. The A-1 title is the name of box −1 on A-2 when A-2 exists.
- The A-0 title and the highest A-n title are free-form. FIPS Fig. 5 shows an A-0 title that differs from its box name.

### 4.2 Status semantics (Annex C.4.1.3, informative)

| Status | Meaning |
|---|---|
| `working` | A major change; restarts the approval sequence. New diagrams are working. |
| `draft` | A minor change from the previous version; accepted by some set of readers. |
| `recommended` | The diagram *and its supporting text* were approved by review and are not expected to change. |
| `publication` | Ready to print as is. Working fields are dropped on export. |

- Status reset rules for edits are in FR-EDIT-15.
- At `recommended` and `publication`, the description gate applies (FR-NAME-06).

### 4.3 C-numbers

- **Format:** 2–3 author initials plus a sequence, no space: `AK14` (Annex C.4.5.1). Version history: `AK15(AK14)` means AK15 replaces AK14. Input with a space (`AK 14`) is normalized.
- C-numbers are **auto-assigned (D18)**. On a status change, or on a major edit that resets status to `working`, the command layer mints `<initials><next seq>` from `meta.author` and the model's highest sequence. It records the previous number as history: `NEW(OLD)`. `SetCNumber` overrides manually. The sequence comes from the model, never from a clock, so minting stays deterministic (D6).
- At publication, a C-number may be replaced by a page number (Annex C.4.5.1); see [ingestion §export](ingestion.md#92-exports).

### 4.4 Detail reference expressions (DRE)

FIPS §3.3.1.3 allows four DRE forms: (1) C-number, (2) page number, (3) node number, (4) model-note number selecting a child version.

- v1 supports **node number** (default) and **C-number**. When the child diagram has a C-number, a theme option shows it; Annex B.2.2.1-4 (informative) allows either form on working and draft copies.
- Page numbers belong to publication export (v2). The model-note form does not apply: idef0-rs keeps one version of each child diagram.
- A **caller box** has no DRE; its call arrow label carries the reference (FR-CALL-02).

### 4.5 Diagram form

The form has working fields (author/date/project, reader notes, status, reader/date, context sketch, Used At) and identification fields (node, title, number), per Annex C.4 (informative). Rendering is in [layout §form](layout.md#144-diagram-form-fr-layout-20-annex-c4-informative).

- `author=` and `date=` are optional diagram attributes. Core never reads the clock; the server or browser passes a date from an injected clock when it creates a diagram.
- When working fields are dropped for publication, the Used At list is copied into a model note (Annex C.4.1.6; FR-CALL-04).

---

## 5. Boxes

```text
(illustrative)
Box = ⟨key, position, number (derived), node (derived), name, description, call: Option<CallArrow>⟩
```

| ID | Requirement | Basis |
|---|---|---|
| FR-BOX-01 | Every box has a name and a box number. | §3.2.1.1 |
| FR-BOX-02 | The name is an active verb or verb phrase (`L-TITLE-01`, warning, heuristic). | §3.2.1.1; §3.2.2.3-1 |
| FR-BOX-03 | Box number = diagonal position, top-left to bottom-right (lineage boxes excepted, §3.2). | §3.3.3-5; §3.3.4.1 |
| FR-BOX-04 | Every box has ≥ 1 control and ≥ 1 output (`no_control`, `no_output`). | §3.3.3-8 |
| FR-BOX-05 | A box has 0+ inputs, 0+ non-call mechanisms and 0–1 call arrows (by type). | §3.3.3-9..11 |
| FR-BOX-06 | Box names are unique among siblings (`L-NAME-01`, error) and preferably model-wide (`L-NAME-02`, warning); ≤ 60 characters. | Annex A.2 *informative* |
| FR-BOX-07 | Off-diagonal boxes are not supported **[Deviation]**: the layout is a strict staircase. | §3.3.4.1 permits them |

- **L-TITLE-01** accepts slash-joined verb alternatives ("Develop/Update a Factory"), phrasal verbs ("Lay Out Diagrams") and the FIPS "(v)" marker for words that read as nouns (Annex B.2.2.1, informative).
- **FR-BOX-07** never rejects a model, because models carry no positions. Imports that carry off-diagonal placement are re-laid out on the diagonal with an info diagnostic ([ingestion §import](ingestion.md#91-imports)).

### 5.1 Leaf (structural definition)

- A **leaf** is a box on an `A0`-or-lower diagram, or the A0 top box, that has **no child diagram**: `leaf(B) ⇔ ¬decomposed(B)`.
- **Caller boxes are leaves.** They have no child diagram of their own; they are grounded through their call (FR-CALL-10, NORTHSTAR criterion 5). Leaf-scoped rules therefore include callers, and `is:caller` distinguishes them ([editor-ux §search](editor-ux.md#9-full-text-search-fr-search)).
- **Environment boxes** on A-n diagrams are not leaves. They are outside the modeled subject and are excluded from leaf rules and metrics.
- The A0 top box is a leaf until the A0 diagram exists.
- The drop shadow is only the classic-theme rendering of "has a child diagram" ([layout FR-STYLE-06](layout.md#13-themes-fr-style)). It never defines a leaf.

---

## 6. ICOM roles and arrows

### 6.1 Roles

| Role | Side | FIPS definition | Notes |
|---|---|---|---|
| **I**nput | Left, entering | "data or objects that are transformed by the function into output" (§2.29); "transformed or consumed" (§3.2.2.1) | Optional (§3.3.3-9) |
| **C**ontrol | Top, entering | "conditions required to produce correct output. Data or objects modeled as controls may be transformed by the function, creating output." (§2.17) | ≥ 1 required (§3.3.3-8) |
| **O**utput | Right, leaving | "data or objects produced by a function" (§2.40) | ≥ 1 required (§3.3.3-8) |
| **M**echanism | Bottom, pointing up | "the means used to perform a function; includes the special case of Call Arrow" (§2.33) | Some means may be inherited from the parent box (§3.2.2.1) |
| **Call** | Bottom, pointing down | "a type of mechanism arrow that enables the sharing of detail between models … or within a model" (§2.12) | §7 |

- **"When in doubt, make it control"** (Annex B.2.4.1-2, informative). Do not split one arrow into a control and an input of the same box (B.2.4.1-3): `L-SPLIT-CI`.
- FIPS Fig. 3 draws the call arrow to the right of the mechanisms. idef0-rs follows that as a **layout convention**; it is not a FIPS rule.

### 6.2 Mechanism bindings [Extension]

- A mechanism arrow may carry `code="<CodeRef>"` and `kind=code|component|resource`. `kind=code` is implied by `code=`.
- `CodeRef = path::qualified::symbol`. A bare path in `code="path"` means file scope (`path::<file>`).
- The scanner checks `code=` targets for existence **directly**, whether or not the target is tagged ([ingestion §scanner](ingestion.md#43-scanner-pipeline-idef0-ingestcode)).
- Leaf mechanisms must be bound, or be marked `kind=resource` or `kind=component` (`unbound_mechanism`); bound leaves must be implemented (`ungrounded_mechanism`). Rules and severities: [validation §mechanisms](validation.md#511-mechanisms-and-code-grounding-r-mech-extension). Both rules are info under `fips183`: FIPS Fig. 4's mechanism "Design Engineer" is valid IDEF0.

### 6.3 Arrow typing (FR-ARROW)

| Source endpoint | Allowed destination endpoints |
|---|---|
| `box.O` | `box'.I`, `box'.C`, `box'.M`, boundary `O`, uncoded boundary `[O]` |
| boundary `I`/`C`/`M`, uncoded `[I]`/`[C]`/`[M]` | `box.I`, `box.C`, `box.M` |

| ID | Requirement | Basis |
|---|---|---|
| FR-ARROW-01 | Any other combination is a core type error (`E-TYPE-01` source, `E-TYPE-02` destination, `E-TYPE-03` boundary-to-boundary). | §3.2.2.3-2; §2.31 |
| FR-ARROW-02 | Output-to-self and backward arrows are legal feedback. | §3.3.3-12 |
| FR-ARROW-03 | Roles may change parent → child; codes match on parent side and rank (`L-ROLE-01`, info). | §3.3.2.8; Fig. 15 |
| FR-ARROW-04 | Each attached arrow has a 1-based **port rank** per box side: top→bottom on left/right, left→right on top/bottom. | §3.3.2.8 |
| FR-ARROW-05 | Ranks per side are 1..n with no gaps or duplicates, except that fork leaves share their source rank and join leaves share their destination rank (`E-TYPE-04`). | §3.3.2.8 |
| FR-ARROW-06 | A child boundary port carries the parent ICOM code `S k`; frame order follows code (I1 above I2, C1 left of C2) **[Extension]**. | §3.3.2.8 (codes); frame order is an idef0-rs invariant |
| FR-ARROW-07 | Every arrow segment except call arrows is labeled, unless a single label clearly applies to the arrow as a whole (`conceptless_arrow`; the name is historical, the rule is FIPS). | §3.2.2.3-3 |
| FR-ARROW-08 | Open boundary arrows that represent the same data are joined through a fork or join, unless that makes the diagram unreadable (`duplicate_arrow_concept`, warning). | §3.3.3-14 |

- **Port rank vs ICOM code:** rank is a property of an arrow at a box side. An ICOM code is the rank as written on the child's boundary (`C3` = third control of the parent box, §3.3.2.8). Rank is semantic and part of the model (D5). It defaults to declaration order, and the canonical file writes it explicitly.
- **Label exemption (FR-ARROW-07):** an unlabeled `rel=all` fork branch or join leaf inherits its trunk label. This is the "single arrow label clearly applies" case. Layout honors the same exemption (`G-LABEL`, [layout §labels](layout.md#11-labels-and-squiggles)).
- **Concepts on arrows [Extension]:** in `extended`, every label is a concept reference (FR-NAME-03). An unresolved label is `unknown_concept`, and `label=` gives a display alias that must be the concept's name or a synonym. Under `fips183`, an unresolved string label is free text (`unknown_concept` is info).
- **Detection for FR-ARROW-08:** concept equality in `extended`; normalized label-string equality in `fips183`.

---

## 7. Call arrows (FR-CALL)

§3.3.2.10: "A call arrow is a special case of mechanism arrow. It signifies that the caller box does not have its own child diagram to detail it, but rather is detailed entirely by another box (and its descendants) in the same or another model."

```text
(illustrative)
CallArrow  = ⟨targets: NonEmpty<CallTarget>, note: Option<NoteNo>, map: Vec<CallPortMap>⟩
CallTarget = ⟨model: Option<Abbrev>, key: BoxKey⟩         # D14: by stable key
CallPortMap = ⟨caller: (Role, k), callee: (Role, k)⟩     # [Extension]
```

| ID | Requirement | Basis |
|---|---|---|
| FR-CALL-01 | A box has 0 or 1 call arrows; it leaves the bottom side pointing down. | §3.3.3-11; §3.2.2.3-2e |
| FR-CALL-02 | A caller box has no child diagram (`call_and_decomposed`); it shows no DRE. | §3.3.2.10 |
| FR-CALL-03 | The call label is the called box's reference expression, computed for display; it carries no concept. | §3.3.2.10; §3.2.2.3-2e, -3 |
| FR-CALL-04 | Many callers may call one box. Its **Used At** list is computed from the workspace call index. | §3.3.2.10; Used At: Annex C.4.1.6 *informative* |
| FR-CALL-05 | Alternatives: the label lists every possible called box, and an attached model note states the selection conditions (`call_alternatives_without_note`). | §3.3.2.10 |
| FR-CALL-06 | When caller and callee interfaces differ, an attached model note states the correspondence (`call_interface_undocumented`). | §3.3.2.10 ("model notes … shall specify") |
| FR-CALL-07 | Targets resolve within the model or through the workspace index (`call_unresolved`). | §3.3.4.3 |
| FR-CALL-08 | A call must not expand into itself, directly or through call chains (`call_recursive`). | **[Extension]** (well-formedness) |
| FR-CALL-09 | The call arrow is not a numbered mechanism ICOM; it is excluded from M-codes. | Inference from §3.3.3-10/11 and §3.3.2.10 |
| FR-CALL-10 | Software semantics: the caller is implemented by the callee; grounding and contract cards follow the call. | **[Extension]** |
| FR-CALL-11 | Reuse detection suggests a call where two subtrees look equivalent (`L-REUSE-01`, info). | **[Extension]** |
| FR-CALL-12 | Targets are stored by key (`@key`, `ABBR/@key`); node-number input is accepted and normalized to a key by `idef0 fmt`. | D14 **[Extension]** |
| FR-CALL-13 | Callable boxes: boxes on decomposition diagrams, including another model's A0 top box. The A0 top box of the calling model cannot be a caller (`E-CALL-02`). | §3.3.4.2 (A0 must be decomposed) |

**Display label (FR-CALL-03).** The label is the node reference of the diagram that contains the called box, plus the called box number (§3.3.2.10):
- `A5.2` for box A52 in the same model, `SEC/A2.1` for box A21 of model SEC.
- An alternatives list reads `SEC/A2.1 | SEC/A2.3`, plus the note glyph.
- Theme option `call_label = fips | node`; `node` shows `SEC/A21` instead.
- Renumbering the callee changes the displayed label, never the stored target.

**Used At (FR-CALL-04).** idef0-rs uses Annex C.4.1.6's broad definition: "a list of diagrams, other than the parent context, which use or reference this diagram-form page in some way".
- Every caller's node reference is listed on the diagram that contains the called box.
- Publication export copies the list into a model note on that diagram (C.4.1.6: "must be copied into the Message field as a … model note").

**Interface differences (FR-CALL-06).**
- **FIPS form:** the interfaces differ when, for some I/C/O code, the caller's and callee's arrows do not carry the same concept (the same label under `fips183`), or one side has a code the other lacks. Ports listed in a `CallPortMap` **[Extension]** are excluded, and mechanisms are not compared. Without an attached model note this is `call_interface_undocumented`: an **error** under `fips183`. Under `extended` it is info, because concept compatibility substitutes for the note **[Deviation]**.
- **Concept form [Extension]:** a caller input or control concept with no compatible callee input or control, or a caller output that no callee output equals or refines, gives `call_interface_mismatch` (warning; info under `fips183`). `call_map_unknown_port` checks the map. Rules: [validation §calls](validation.md#59-call-arrows-r-call).
- **Cross-model calls** are resolved but not interface-checked in v1 (FR-XM-04).

**Deleting a called box** removes local targets and leaves cross-model callers unresolved (FR-EDIT-05).

---

## 8. Bundles: forks and joins (FR-BUN)

| ID | Requirement | Basis |
|---|---|---|
| FR-BUN-01 | A **fork** splits one arrow segment into branches; a **join** merges segments into one. | §2.23; §2.32 |
| FR-BUN-02 | All contents follow all branches unless a branch label says otherwise. | §3.3.2.5 |
| FR-BUN-03 | Each branch has `rel ∈ {all, part_of, kind_of}`; an unlabeled branch is `rel=all`. | **[Extension]** refinement of §3.3.2.5 |
| FR-BUN-04 | A labeled branch declares its `rel` (`L-BUN-01`). One bundle must not mix `kind_of` and `part_of` (`mixed_bundle_relation`). | **[Extension]** |
| FR-BUN-05 | Branch concepts agree with the ontology (FR-NAME-04, `bundle_concept_mismatch`). | **[Extension]** |
| FR-BUN-06 | v1 supports one level per statement: a single fork or a single join. Nested and join-then-fork bundles are v2. | idef0-rs limitation |

**Relation mapping (debatable, stated explicitly).**
- `rel=all` = "the same thing" (§3.3.2.5).
- `rel=part_of` = "portions of the same thing" (§3.3.2.5).
- `rel=kind_of` = specialization of "a more general category" (§3.3.2.5). However, §2.10 defines bundling as combining meanings "into a composite meaning", which reads as aggregation (`part_of`).
- Both readings are defensible. idef0-rs offers both relations, and under `fips183` they render identically as labeled branches with no relation glyph.

**Fact flattening** (for validation): each bundle becomes leaf edges from the true source to the true destination, plus `bundle(Trunk, Leaf)` facts. The extractor obligations are in [validation §extractor](validation.md#3-fact-extraction-contract):
- `rel=all` leaves inherit the trunk concept.
- Trunks carry a concept and `edge_in`.
- Every leaf has `edge_in`.
- Trunk-level attributes (`tunnel`, `kind`, `code`) are pushed to the leaf at the shared end.
- Join leaves share the destination rank.

---

## 9. Tunnels (FR-TUN)

| Notation (`extended`) | `fips183` theme | Position | Meaning | ICOM code |
|---|---|---|---|---|
| `( )` at a box end | `( )` | Connected end | Not shown on that box's child diagram; resolves at the parent (§3.3.2.9) | Keeps its code on the parent |
| `[ ]` at the border end **[Deviation]** | `( )` | Unconnected end | Not shown on the parent; appears on this diagram and below (§3.3.2.9) | **None** (§3.3.2.9, §3.3.3-13) |

| ID | Requirement | Basis |
|---|---|---|
| FR-TUN-01 | Box-end tunnels use `tunnel=box` when exactly one end is a box, otherwise `tunnel=src\|dst\|both` (`E-TYPE-05` if ambiguous). | §3.3.2.9 |
| FR-TUN-02 | A border-tunneled end is written as an uncoded endpoint `[I1]`, `[C2]`, `[M1]`, `[O1]`. A boundary port is either coded or uncoded, never both. | §3.3.2.9; §3.3.3-13 |
| FR-TUN-03 | The bracket number is a local ordinal, unique per diagram side; `fmt` never renumbers it; new ordinals take max + 1. | idef0-rs notation |
| FR-TUN-04 | A border-tunneled arrow may carry a model note giving the node reference and ICOM code of the tunnel's other end. | §3.3.2.9 |
| FR-TUN-05 | Border tunnels are forbidden on A-0 (`context_tunnel`). | §3.4.2 |
| FR-TUN-06 | ICOM coding resumes on lower child diagrams: on the child of a box fed from `[I1]`, the arrow has a normal code. | §3.3.2.9 |

The `[ ]` vs `( )` distinction makes the two directions visually distinct (KBSI/AI0Win convention, D13). Semantics are identical in both profiles.

---

## 10. Notes (FR-NOTE)

| ID | Requirement | Basis |
|---|---|---|
| FR-NOTE-01 | **Model notes** are optional. They are part of the diagram on which they appear and record a fact not otherwise depicted. | §2.34; §3.3.4.4 |
| FR-NOTE-02 | Model note numbers on a diagram form a consecutive sequence from 1 (`L-NOTE-01`, error). | §3.3.4.4 |
| FR-NOTE-03 | A note may attach, by squiggle, to several points on the same diagram only (`E-NOTE-01` for a foreign target). | §3.3.4.4 |
| FR-NOTE-04 | Model notes are required in two cases: call alternatives (FR-CALL-05) and call interface differences (FR-CALL-06). | §3.3.2.10 |
| FR-NOTE-05 | **Reader notes** are on the diagram, not part of it, and never affect validation. They are stored outside the model file (FR-WS-01). | Annex B.2.5.2 *informative* |

- Rendering: boxed number, or `|n|` (§3.3.4.4). Reader notes use a circled number.
- Reader-note numbers per node are permanent and never reused (B.2.5.2). v1 reserves their storage, `.idef0/review/`. The authoring UI and the Kit Cycle are v2.

---

## 11. Balance (FR-BAL)

For box *P* detailed by diagram *D*:

| ID | Requirement | Basis |
|---|---|---|
| FR-BAL-01 | **Down:** each arrow at *P* side *S* rank *k* that is not tunneled at *P* has a coded boundary port `S k` on *D*. | §3.3.2.6; §3.3.2.9 |
| FR-BAL-02 | **Up:** each coded boundary port `S k` on *D* matches an arrow at *P* side *S* rank *k*. Uncoded `[..]` ports are exempt. | §3.3.2.7; §3.3.2.9 |
| FR-BAL-03 | A bundle on one level may correspond to several branches on the other, related by `bundle` facts. | §3.3.2.5 |
| FR-BAL-04 | Concept compatibility (`balance_concept_mismatch`, warning): the concept at the attached end (the trunk concept at a fork source or join target) equals or refines the parent arrow's concept. | **[Extension]** |
| FR-BAL-05 | For A0, *P* is the top box; with an A-1 present, its A-1 ICOMs must equal its A-0 ICOMs (FR-DIAG-11). | §3.4.2 |

- Violations: `icom_imbalance`, with `imbalance` detail payloads (`missing_in_child`, `missing_in_parent`). Rules: [validation §balance](validation.md#55-icom-balance-r-bal-01).
- Boundary ports are not declared separately. A coded port exists while an arrow uses it. The editor shows each parent ICOM with no child arrow as an unconnected **stub** on the child frame, so a freshly decomposed (empty) child shows all expected ports ([editor-ux §stubs](editor-ux.md#61-global-keys-navigation-modes-only)).

---

## 12. Names, descriptions and concepts (FR-NAME)

| ID | Requirement |
|---|---|
| FR-NAME-01 | Every function (box) has a `name` and a `description`. |
| FR-NAME-02 | Every concept has a `key`, a `name` (a noun phrase, §3.2.2.2; unique model-wide including synonyms: `L-CONCEPT-01`) and a `description`. |
| FR-NAME-03 | Concepts are model-level and shared. In `extended`, every labeled arrow references exactly one concept, directly or via its `rel=all` trunk. |
| FR-NAME-04 | The ontology is authoritative for bundles: a `kind_of` (`part_of`) branch carries a concept that is transitively `kind_of` (`part_of`) the trunk concept. |
| FR-NAME-05 | The model and every diagram have a `description`. A diagram description is the text-page analogue (§3.3.1.4). |
| FR-NAME-06 | At `recommended`/`publication`, every box on the diagram and every concept on its arrows must be described (`undescribed_box_on_release`, `undescribed_concept_on_release`). |
| FR-NAME-07 | Descriptions are inline Markdown, recommended ≤ 600 characters, rendered sanitized ([agents §threat model](agents.md#7-threat-model-and-privacy)). |
| FR-NAME-08 | Names and descriptions appear in the info line, SVG `<title>`/`<desc>`, inspector, contract cards, scaffolds and proposals. |
| FR-NAME-09 | Box names and arrow labels must not consist solely of a reserved word (`L-NAME-03`, error) and should avoid the Annex B.2.4.2-3 keywords (`L-NAME-04`, warning). |
| FR-NAME-10 | Acronyms used in box names or arrow labels are defined in the glossary (`L-GLOSS-01`, warning). |
| FR-NAME-11 | A concept carried by no arrow and not a parent of a used concept is reported (`L-CONCEPT-02`, info). |
| FR-NAME-12 | Names and descriptions record their provenance `origin ∈ {model, prd, code, llm}` (default `model`) and an optional `source`; a human edit resets it to `model`. |

**Descriptions are definitions (FR-NAME-01, FR-NAME-02).**
- A function description is a glossary-style definition: 1–3 sentences stating what the function accomplishes and how it differs from its siblings.
- It must not merely restate the box's arrows. FIPS §3.3.1.4 says text "shall not be used merely to describe, redundantly, the meaning of boxes and arrows".
- Concept descriptions use genus–differentia where possible: "A ‹genus› that ‹differentia›."
- Editor drafts (`Ctrl+D`) start from the name, the parent's description and the sources. They never enumerate the ICOMs ([editor-ux §text edit](editor-ux.md#66-textedittarget)).
- Description lints (info), one code per subject kind: `L-DESC-02` box, `L-DESC-03` concept, `L-DESC-04` diagram, `L-DESC-05` model. They flag:
  - placeholder text (`TBD`, `TODO`, `FIXME`, `?`, `...`), in descriptions, `purpose` and `viewpoint`
  - a description shorter than 20 characters
  - a description that only restates the name
  - a box description that enumerates the box's ICOM labels in a "transforms ‹I› into ‹O› under ‹C›" pattern
  - a concept description with no genus

**Reserved words (FR-NAME-09).**
- Box names: *function, activity, process, input, output, control, mechanism* (§3.3.3-15).
- Arrow labels: the same words plus *call* (§3.2.2.3-5 and §3.3.3-15). The word *call* is not reserved for box names.
- Keywords: Annex B.2.4.2-3 (informative) says not to use *data, function, input, output, control, mechanism* in names or labels "unless absolutely necessary". A whole-word occurrence is `L-NAME-04` (warning) unless `L-NAME-03` already fired.
- Label findings are reported on the box the arrow attaches to (destination box, or source box for arrows that end at the boundary), so each code keeps one subject kind (box).

**Acronyms (FR-NAME-10).**
- Basis: §3.2.2.1, "When acronyms, abbreviations, key words, or phrases are used, the fully defined term(s) shall be provided in the glossary."
- Heuristic: a token matching `[A-Z][A-Z0-9]{1,7}` with ≥ 2 capitals needs one of: a concept whose name or synonym equals the token; or an expansion `TOKEN (expansion)` or `expansion (TOKEN)` in the description of the concept that carries the label.

**Gates and severities.**
- Missing descriptions are warnings while working (`undescribed_function`, `undescribed_concept`, `undescribed_diagram`; `L-META-03` for the model). Under `fips183` they are info.
- The release-gate codes are info under `fips183`: FIPS text is optional, "A diagram may or may not have associated text" (§3.2.2.1).
- Concept lifecycle commands (rename, rekey, delete, merge) are in FR-EDIT-12.

**Provenance (FR-NAME-12).** It is stored in the file as `origin=` and `source=` attributes on `box`, `concept` and `description` (§16.2), so it survives round-trips. Contract cards and the UI use it to fence and badge untrusted text ([agents §2.3](agents.md#23-untrusted-text-fencing-and-provenance)).

---

## 13. Editing semantics (FR-EDIT)

Editing happens only through **Commands**. Each command is applied by core as a pure function and yields a **Patch**: forward ops, inverse ops and a RenameMap. The UI, CLI, server, sync and agents all use the same path; agent writes become pending proposals first (D15, [agents §proposals](agents.md#63-writes-pending-proposals-d15)).

| ID | Requirement |
|---|---|
| FR-EDIT-01 | `apply(model, command, ctx)` is deterministic: `ctx` carries the key generator (slug rule) and no clock or RNG. |
| FR-EDIT-02 | The RenameMap records every identifier change: nodes, ports (coded and uncoded), arrow ids, removed box keys (`@k → ∅`), concept rekey/merge/delete, note renumbering, model abbrev. |
| FR-EDIT-03 | `AddBox` at position *p* shifts boxes *p..n* to *p+1..n+1*, with their subtrees; rejected at the profile maximum and on A-0. |
| FR-EDIT-04 | Renumbering a box renames its node, every descendant node and every port of each, with RenameMap entries for all. |
| FR-EDIT-05 | `DeleteBox` cascades as specified below; the top box and the A-0 diagram cannot be deleted. |
| FR-EDIT-06 | `MoveBox` permutes positions on one diagram and renumbers per FR-EDIT-04; ports, keys and calls are unchanged. |
| FR-EDIT-07 | `ReorderPorts` is the single path for any rank change. It re-codes the child boundary of a decomposed box atomically. Shift+↑/↓, Auto-Arrange, AddArrow-at-rank and rank compaction all use it. |
| FR-EDIT-08 | A child boundary arrow whose parent code disappears becomes an uncoded border tunnel `[Sk]` (next free ordinal); it is never silently deleted. |
| FR-EDIT-09 | `Decompose` creates an empty child (status `working`, derived title). It is rejected on caller boxes, environment boxes and already-decomposed boxes. |
| FR-EDIT-10 | `DeleteDiagram` removes a child diagram and its descendants; the parent becomes a leaf and keeps its ports. |
| FR-EDIT-11 | `RenameBox` changes the name, not the key. Editing a derived diagram title is a `RenameBox` on the parent. |
| FR-EDIT-12 | Concept commands: `RenameConcept` changes the name, not the key; `RekeyConcept`, `DeleteConcept` and `MergeConcepts` follow the rules below. |
| FR-EDIT-13 | v1 supports **copy/paste** of a box, with or without its subtree, within one model (same or another diagram). |
| FR-EDIT-14 | Cut/paste that preserves keys across diagrams ("move to another diagram") and cross-model paste are **v2**. |
| FR-EDIT-15 | Status changes automatically on edits, per the major/minor classification below. |
| FR-EDIT-16 | Notes: `AddNote` takes the next number; `DeleteNote` renumbers higher notes and updates every reference to them. |
| FR-EDIT-17 | Undo and redo are per model, cross-diagram, with navigation (below). |
| FR-EDIT-18 | `SetCall`/`ClearCall` follow FR-CALL; `SetCall` is rejected on decomposed boxes and on the top box. |
| FR-EDIT-19 | `Retarget` changes one endpoint; the new end takes the last rank, and the old side compacts (FR-EDIT-07). |
| FR-EDIT-20 | A renumbering patch also rewrites same-model node references in descriptions and model notes. |

### 13.1 DeleteBox cascade (normative order)

1. **Subtree:** remove the box's child diagram and every descendant diagram, box, arrow and note.
2. **Attached arrows:** remove every arrow on the box's diagram with an endpoint at the box. A bundle that loses leaves keeps the rest. A bundle left with one leaf collapses into a plain arrow, and one left with none is removed.
3. **Rank compaction:** on each remaining box that lost an arrow, close the rank gaps on that side, keeping relative order (FR-EDIT-07). Where that box is decomposed, re-code its child boundary. Child arrows whose code vanished become `[Sk]` tunnels (FR-EDIT-08).
4. **Boundary:** boundary arrows that reached only the deleted box are removed in step 2. The parent box's ports never change, so those ICOMs show as stubs and `icom_imbalance` until rewired.
5. **Renumber:** boxes after the deleted position move down by one, with subtrees (FR-EDIT-04).
6. **Notes:** attachments to removed elements are dropped. The notes themselves stay and are not renumbered.
7. **Calls:**
   - Local call targets that point into the removed subtree are removed. A call left with no targets is removed, and its caller becomes an ordinary leaf.
   - Calls from removed boxes vanish with them.
   - Callers in other models become `call_unresolved` there. Rewriting other files is never part of a model patch.
8. **RenameMap:** `@key → ∅` for removed boxes, `id → ∅` for removed arrows, old→new for renumbered nodes, ports and arrow ids.
9. **Confirm:** the confirm prompt lists the removed diagrams, boxes and arrows, the affected local and known cross-model callers (from the workspace index), and the ports converted to tunnels. Wording: [editor-ux §confirm](editor-ux.md#68-confirmaction).

`DeleteArrow` and `Retarget` apply steps 3 and 8 to the affected sides.

**Free-text references (FR-EDIT-20).** Tokens that match the `BoxRef`/`PortRef` grammar at word boundaries, without a model prefix or with the model's own, are rewritten when their node is in the RenameMap. The token keeps its written style (dotted or undotted). Other models' refs and PRD prose are untouched; tags and anchors are rewritten by `idef0 sync` ([ingestion §6.3](ingestion.md#63-rename-rewriting-sr-21)).

### 13.2 ReorderPorts (single path)

- **Input:** box, side, and the new order of that side's arrows.
- **Ranks:** each arrow's rank is set to its new position.
- **Child re-coding:** if the box is decomposed, every coded child boundary arrow on that side is re-coded with the same permutation. Balance is preserved by construction.
- **RenameMap:** port entries (`A21/I2 → A21/I1`), arrow-id entries on both diagrams.
- **Boundary side:** reordering ports on a child frame is `ReorderPorts` on its parent box.
- **A-0 port ids** follow the top box's ranks.
- **No warn-only path:** Shift+↑/↓ (one swap) and Auto-Arrange (full permutation, [layout §auto-arrange](layout.md#12-auto-arrange)) produce identical patches for identical permutations.

### 13.3 Concept commands

| Command | Effect |
|---|---|
| `RenameConcept` | New name; key unchanged; all labels follow. Optionally keeps the old name as a synonym. |
| `RekeyConcept` | New key; RenameMap `@old → @new` for cross-model refs and PRD `concept=` anchors. |
| `DeleteConcept` | Rejected while any arrow carries it. `--force` unbinds those arrows (they then raise `conceptless_arrow`) and drops the concept from `kind_of`/`part_of` lists. |
| `MergeConcepts(a → b)` | Rebinds a's arrows to b, adds a's name and synonyms to b's synonyms, re-points relations, deletes a; RenameMap `@a → @b`. |

### 13.4 Copy/paste (v1)

- **Copied:** name, description, call arrow (same-model key targets), and optionally the whole subtree with its internal arrows, notes and calls. Concepts are referenced, not copied.
- **Not copied:** arrows that attach the box on its source diagram.
- **Paste:** inserts after the focused box (AddBox semantics), mints fresh keys with the slug rule, and is rejected when the diagram is full.
- **Name collision:** a sibling name collision leaves `L-NAME-01` until renamed; the editor enters TextEdit immediately.
- **Why cut/move is v2:** key-preserving moves interact with trace locks, Used At, and imbalance on both diagrams. v1 users copy, paste and delete; `idef0 sync` then reports `@old` tags as unknown.

### 13.5 Major and minor changes (FR-EDIT-15)

| Class | Changes |
|---|---|
| **Major** | Add, delete or move a box; rename a box; add, delete or retarget an arrow; change an arrow's label or concept; fork, join or `rel`; tunnel; port rank; call; any change of ICOM codes on a child diagram |
| **Minor** | Box or diagram description; model note add, edit or delete; C-number; DRE change caused by decomposing or undecomposing a child |
| **None** | Model metadata, concept descriptions, synonyms and relations, reader notes |

- **Status effects:**
  - A major change on a diagram at `draft` or above resets it to `working`.
  - A minor change at `recommended` or `publication` resets it to `draft`.
  - Annex C.4.1.3: working "is a major change, restarts the approval sequence"; draft "is a minor change".
- The reset is part of the same Patch: visible in previews and undone together. An explicit `SetStatus` afterwards is allowed and recorded.
- `RenameConcept` is a major change on every diagram that carries the concept.

### 13.6 Undo and redo (FR-EDIT-17)

- **Scope:** one stack per open model, spanning all its diagrams. Undo history is not persisted across sessions in v1.
- **Entries:** each entry holds the Command, its Patch, the primary diagram, and the selection before and after. A compound operation is one entry: Auto-Arrange, paste, an accepted proposal, a sync patch.
- **Navigation:** undo applies the inverse ops, navigates to the primary diagram (or its nearest surviving ancestor), and restores the prior selection, repaired if stale. Redo restores the later selection.
- **Persistence:** saving does not clear the stack. Default depth is 500, configurable.
- **Elsewhere:** rebase after `409`, external reloads, and the rule never to overwrite a file that fails to parse are in [architecture §state](../architecture.md#6-persistence-and-concurrency). Rebase replays stored Commands, not Patches.

### 13.7 Implementation notes (M1.5)

These are idef0-rs implementation choices, not FIPS rules.

- **Commands in v1 core.** These commands exist (`idef0_core::command`):
  - box commands: `AddBox`, `RenameBox`, `MoveBox`, `DeleteBox`, `DeleteDiagram`;
  - arrow and port commands: `AddArrow` (plain), `DeleteArrow`, `Retarget`, `ReorderPorts`;
  - text and status: `SetDescription`, `SetStatus`;
  - copy and paste: `Paste`, with `copy` producing the clip.

  `Decompose`, `SetCall` and `ClearCall` arrive with M1.6. `Fork`, `Join`, `SetLabel`, `SetRel`, `SetTunnel`, `SetMeta`, the concept commands (§13.3) and the note commands (FR-EDIT-16) arrive with the editor wiring epic (M6.2), which is the first to issue them.
- **Op granularity.** An op replaces one diagram, one concept, or the metadata. The inverse holds the previous value, so undo restores the input exactly.
- **RenameMap derivation.** The map is derived, not written by each command:
  - node renames follow box keys (each box key's node before and after);
  - arrow-id renames follow the statement each arrow came from;
  - port renames follow the ends of surviving arrows.
  
  When the parent's port vanishes but the child keeps the code as a border tunnel, the port maps to that tunnel (`A3/I1 → A2/[I1]`) and not to `∅`. A compound change (`apply_all`) composes the maps, so each original identifier maps to its final one.
- **Validity.** A command whose result has more `E-*` findings than its input is rejected (`CommandError::Invalid`) and nothing changes.
- **`ReorderPorts` input.** The command takes the side's old ranks in their new order. Shift+↑/↓ and Auto-Arrange both express their permutation that way.
- **Box commands.** They act on decomposition diagrams. A box on A-0 or A-n can be renamed and described, and its ports reordered; adding, moving and deleting there are rejected. `RenameBox` renames every occurrence of the key, since the A-1 lineage box shares the A-0 box's key, and updates a stated child title equal to the old name (FR-DIAG-05).
- **Deleted keys.** A box key that a patch removes is added to the retired set (D17).

---

## 14. Persistence and workspace (FR-WS)

| ID | Requirement |
|---|---|
| FR-WS-01 | The workspace layout is as listed below; `idef0 init` creates it with a `.idef0/.gitignore`. |
| FR-WS-02 | One model per `.idef0` file. |
| FR-WS-03 | `.idef0/index.json` is a schema-versioned, regenerable cache, gitignored by default; CI never treats it as authoritative. |
| FR-WS-04 | Every model file starts with a version header line, `idef0 0.2`. |
| FR-WS-05 | Versioning and migration follow the policy below; `idef0 migrate` upgrades files. |
| FR-WS-06 | Comments round-trip through UI edits by way of a lossless CST. |
| FR-WS-07 | `idef0 fmt` produces the canonical form; `fmt(parse(fmt(m))) == fmt(m)`. |
| FR-WS-08 | Two files that declare the same abbrev are a workspace error (`E-ID-06`). |

```text
(normative)
<workspace>/
├── .idef0/
│   ├── config.toml         committed   workspace settings (schema: ingestion §config)
│   ├── trace.lock          committed   ack records: content hashes, sig_hash baselines, tag memory
│   ├── rules/*.mg          committed   project rules (validation §project rules)
│   ├── review/<ABBR>.toml  committed   reader notes (reserved; UI in v2)
│   ├── .gitignore          committed   lists index.json, cache/, proposals/, run/
│   ├── index.json          ignored*    workspace index cache (*may be committed for offline browser validation)
│   ├── proposals/          ignored     pending agent proposals (agents §proposals)
│   ├── run/                ignored     write.lock: the server run file and write lock (pid, port, token, started; architecture)
│   └── cache/              ignored     scan cache, search index, AI logs
└── models/*.idef0          committed   one model per file (globs: config [workspace] models)
```

- **`trace.lock`** is TOML with `version = 1`, sorted keys and no timestamps: `[docs."<DocRef>"]` (content hash, targets), `[code."<CodeRef>"]` (`sig_hash`, targets) and `[tags."<CodeRef>"]` (tag memory: the box key each node-number tag resolved to at the last sync, which catches tags left stale by a renumber). Shape and rules: [ingestion §3.6](ingestion.md#36-drift-and-the-trace-lockfile).
- **`index.json`** schema `idef0.index.v1` lists models (abbrev, path, name, content hash), boxes (model, key, node, name), concepts, and the call index (caller → target keys) used for Used At and cross-model resolution.

**Version policy (FR-WS-05).**
- **DSL `MAJOR.MINOR`.** Before 1.0, any minor may break, and each bump ships an `idef0 migrate` step from the previous version. From 1.0, minors are additive and majors break.
- **Reading:**
  - An older version needs `idef0 migrate` (`E-VERSION-01`).
  - A newer minor within the same major ≥ 1 parses: unknown attributes become warnings and are preserved.
  - A newer major is refused (`E-VERSION-02`).
- **No header** means legacy 0.1. It is parsed in compatibility mode with `L-FMT-01` (warning). `idef0 migrate` then:
  - adds the header and explicit keys and ranks
  - rewrites node-number call targets to keys
  - turns `tunnel=border` into `[Sk]` endpoints
  - normalizes `c="AK 014"` to `c="AK14"`
- The contract-card schema (`idef0.contract.v1`) is versioned separately ([agents §contract cards](agents.md#22-formats-and-schema-versioning)). Diagnostic codes are stable: a renamed code keeps its old ID as an alias for one minor version (validation registry).

**Comments (FR-WS-06).**
- **Attachment:** a run of comment lines attaches to the **following** element as leading trivia. A comment after code on the same line attaches to that line's element. A comment before a closing `}` attaches to the enclosing block, and one at the end of the file attaches to EOF.
- **Edits:** comments travel with their element through edits, renumbering and formatting. Deleting an element deletes its comments, and undo restores them.
- **CST:** edits are applied to the lossless CST (rowan-style), so untouched text keeps its bytes.

**Canonical form (FR-WS-07).**
- Header first, then the model block. Concepts stay in author order, and new concepts are appended. Diagrams follow in node order: `A-n…A-1`, `A-0`, `A0`, `A1`, `A11`, …
- Within a diagram:
  - description, then boxes by number, arrows in source order, calls by box number, notes by number, then layout
  - two-space indent, single spaces, explicit ranks and box keys
  - attribute order: `key`, `origin`, `source`, `label`, `rel`, `tunnel`, `kind`, `code`, `note`, `map`
  - two spaces before a trailing `#`; blank lines between statements are preserved, collapsed to at most one
- Single-line blocks (`{ description "…" }`) are preserved when they fit in 100 columns; otherwise the block is expanded one field per line.

---

## 15. Cross-model references (FR-XM)

| ID | Requirement |
|---|---|
| FR-XM-01 | The glossary is per model. An arrow may reference another model's concept as `SEC/@key`; unresolved refs are `unknown_concept`. |
| FR-XM-02 | v1: `kind_of`/`part_of` relations are intra-model only. |
| FR-XM-03 | A foreign concept is compatible only with itself (same model and key) for balance and bundle checks. |
| FR-XM-04 | Cross-model calls (`SEC/@key`) are resolved and indexed for Used At; their interfaces are not checked in v1. |
| FR-XM-05 | CI resolves cross-model refs by parsing the referenced model files at the same revision. ExternalFacts freshness: [validation §external facts](validation.md#10-external-facts). |
| FR-XM-06 | An abbrev rename leaves other models' `OLD/…` refs unresolved until `idef0 sync --apply` rewrites them as FileEdits. |

---

## 16. The `.idef0` DSL (normative)

### 16.1 Lexical rules

- UTF-8 text. CRLF is normalized to LF on write. Indentation is insignificant.
- **Statements** are newline-separated. A `{ … }` body may sit on one line or span several.
- **`STRING`** is `"…"` with escapes `\" \\ \n \t \u{…}`. **`TRIPLE`** is `"""…"""` (multi-line Markdown, common indentation stripped). `TEXT := STRING | TRIPLE`.
- **Comments** run from `#` to end of line, outside strings. Placement and attachment: FR-WS-06.
- **`#n` vs comment:** `#` directly followed by digits and then a space, a tab, a comma, `}` or the end of the line is the occurrence suffix of a note target (`1O1 -> 2I1 #1`); any other `#` starts a comment. Write comments as `# text`.
- **Tokens:**
  - `KEY` and `ABBREV`: §3.6.
  - `NUM := [0-9]+`; `INT := "-"? NUM`.
  - `BOXNO := "-"? NUM`, where `-` occurs only on A-n lineage boxes.
  - `NODE := NodeId`, dotted forms accepted (§3.6).

### 16.2 Grammar

```ebnf
(* normative *)
file          := header model_decl ( concept | diagram )*
header        := "idef0" NUM "." NUM NEWLINE                    (* first line, e.g. "idef0 0.2" *)
model_decl    := "model" STRING "{" meta_field* "}"
meta_field    := ( "abbrev" | "description" | "purpose" | "viewpoint" | "scope" | "context"
                 | "author" | "project" | "lang" ) TEXT
               | "profile" ( "extended" | "fips183" )
               | "retired" "{" KEY* "}"                           (* D17: retired box keys, sorted by the formatter *)

concept       := "concept" KEY STRING origin* "{" concept_field* "}"
concept_field := "description" TEXT origin*
               | "synonyms" STRING ( "," STRING )*
               | ( "kind_of" | "part_of" ) KEY ( "," KEY )*
               | "example" TEXT
               | "source" STRING                                 (* DocRef *)

diagram       := "diagram" NODE STRING? diag_attr* "{" diag_stmt* "}"
diag_attr     := "status=" ( "working" | "draft" | "recommended" | "publication" )
               | "c=" STRING | "author=" STRING | "date=" STRING
diag_stmt     := "description" TEXT origin* | box | arrow | call | note | layout

box           := "box" BOXNO STRING? ( "key=" KEY )? origin* ( "{" "description" TEXT origin* "}" )?
origin        := "origin=" ( "model" | "prd" | "code" | "llm" ) | "source=" STRING    (* FR-NAME-12 *)

arrow         := "arrow" cref? ":" src "->" dst arrow_attr*
               | "arrow" cref? ":" src "->" "fork" arrow_attr* "{" branch+ "}"
               | "arrow" cref? ":" "join" arrow_attr* "{" leaf+ "}" "->" dst
branch        := cref? "->" dst arrow_attr*
leaf          := cref? ":" src arrow_attr*
cref          := ( ABBREV "/" )? "@" KEY | STRING                (* concept ref, FR-XM-01 *)
src           := BOXNO "O" NUM? | ( "I" | "C" | "M" ) NUM | "[" ( "I" | "C" | "M" ) NUM "]"
dst           := BOXNO ( "I" | "C" | "M" ) NUM? | "O" NUM | "[" "O" NUM "]"
arrow_attr    := "key=" KEY | "label=" STRING | "rel=" ( "all" | "kind_of" | "part_of" )
               | "tunnel=" ( "box" | "src" | "dst" | "both" )
               | "kind=" ( "code" | "component" | "resource" ) | "code=" STRING

call          := "call" BOXNO "->" callref ( "|" callref )* ( "note=" NUM )? ( "map=" STRING )?
callref       := ( ABBREV "/" )? "@" KEY
               | ( ABBREV "/" )? NODE "." BOXNO                   (* accepted; fmt rewrites to @KEY *)

note          := "note" NUM TEXT ( "on" note_target ( "," note_target )* )?
note_target   := "box" BOXNO | "call" BOXNO | src "->" dst ( "#" NUM )? | "@" KEY   (* arrow key *)
               | ( "I" | "C" | "O" | "M" ) NUM | "[" ( "I" | "C" | "O" | "M" ) NUM "]"

layout        := "layout" "{" layout_stmt* "}"                    (* owned by layout spec; preserved *)
```

### 16.3 Static semantics

- **Box numbers** on a diagram are `1..n` in statement order. A-0 has exactly `box 0`. A-n uses the lineage numbering of §3.2 (`E-NODE-02`). `box 0` on A-1 refers to the top box: its name and key may be omitted, and must match if given.
- **Names:** a box name is required except on that A-1 lineage box. A diagram title is required on A-0 and on the highest A-n; elsewhere it is derived (FR-DIAG-05).
- **Diagrams:** a diagram for a nonexistent or environment box is `E-NODE-03`. Node digits outside 1–7 are `E-NODE-01`.
- **Ranks:** an omitted rank (`1O -> 2I`) is assigned in declaration order after explicit ranks are placed. Fork leaves share the source rank, and join leaves share the destination rank. Gaps and duplicates are `E-TYPE-04`.
- **A-0 port ids** equal the top box's ranks: `I2 -> 0I2`.
- **Labels:** a top-level `arrow` without `cref` is legal syntax and yields `conceptless_arrow`. A branch or leaf without `cref` inherits the trunk's (FR-ARROW-07). A `STRING` cref resolves by concept name or synonym (FR-NAME-03).
- **Calls:** `map="I1=I2,C1=C1"` pairs caller=callee codes **[Extension]**; mechanisms are not mapped. `note=<n>` attaches the call to model note *n* (equivalent to `note n … on call b`).
- **Unknown attributes** are `E-PARSE-02` (or warnings under a newer minor version, FR-WS-05).

### 16.4 Parser and error recovery

- The parser is `winnow`-based. Recovery is hand-written resynchronization, because winnow has no stable recovery API.
- **Resync:** on an error, skip to the next line that starts with a top-level keyword (`model`, `concept`, `diagram`). Inside a diagram block, skip to the next statement keyword (`box`, `arrow`, `call`, `note`, `layout`, `description`) or `}`.
- **Output:** all errors with spans (`E-PARSE-01`), capped at 100. The partial model is used for diagnostics only and is never written back.
- Input limits (file size, line length, element counts, depth, text length): [architecture §8.3](../architecture.md#83-input-limits-nfr-scale-01-hard-limits), reported as `E-LIMIT-01`.

### 16.5 Example (illustrative, abridged, not diagnostic-clean)

```idef0
idef0 0.2
# Abridged: concepts and most arrows omitted.
model "Order Fulfillment" {
  abbrev    "ORD"
  purpose   "Specify order processing so services can be implemented and verified independently."
  viewpoint "Order operations architect"
}

concept credit-policy "Credit Policy" {
  description "A rule set that bounds credit exposure per customer."
}

diagram A2 status=draft c="AK14" {          # title derived from box A2 "Process Order"
  description "Shows how validation gates payment and stock reservation."
  box 1 "Validate Order" key=validate-order { description "Decides whether an order may proceed." }
  box 2 "Authorize Payment" key=authorize-payment
  box 3 "Reserve Stock" key=reserve-stock

  arrow @customer-order  : I1 -> 1I1
  arrow @credit-policy   : C1 -> 1C1        # port A21/C1
  arrow @fraud-signal    : [I1] -> 1I2      # tunneled at the border: no ICOM code
  arrow @validated-order : 1O1 -> fork {
    -> 2I1
    -> 3I1
  }
  arrow @exception-notice : join {          # bundling into a more general category
    @rejection-notice : 1O2  rel=kind_of
    @backorder-notice : 3O2  rel=kind_of
  } -> O2

  call 2 -> SEC/@verify-token | SEC/@verify-hw-token  note=1
  note 1 "Use SEC/A2.3 when the request carries a hardware token; otherwise SEC/A2.1." on call 2
}
```

---

## 17. FIPS 183 conformance matrix

Abridged clause text. "N/I" means normative or informative source. Enforcement points: `E-`/`L-` in core, `R-` codes in [validation](validation.md), `G-` in [layout](layout.md).

### 17.1 Normative clauses (Sections 2–3)

| Clause | N/I | Requirement (abridged) | idef0-rs enforcement | `fips183` profile |
|---|---|---|---|---|
| §2.17 | N | Controls may be transformed into output | ICOM role table §6.1 | Same |
| §2.48 | N | Child diagram title corresponds to parent box name | `E-TITLE-MATCH`; derived titles | Same |
| §3.2.1.1, §3.2.2.3-1 | N | Box has name (active verb) and number | `L-TITLE-01` (heuristic); derived numbers | Same |
| §3.2.1.3 boxes | N | Fits name; square corners; solid lines | Layout; `G-BOX` | Same |
| §3.2.1.3 arrows | N | 90° arcs; solid; orthogonal; touch perimeter; attach at sides, not corners | Layout; `G-ARC`, `G-ORTHO`, `G-PERIM`, `G-CORNER` | Same |
| §3.2.2.1 | N | Acronyms and key words defined in the glossary | `L-GLOSS-01` (warning) | Same |
| §3.2.2.3-2 | N | I left, C top, O right, M bottom up; call bottom down, labeled with the reference | Typing §6.3; FR-CALL-01/03; `G-SIDE` | Same |
| §3.2.2.3-3 | N | Segments labeled with noun phrases except call arrows, "unless a single arrow label clearly applies to the arrow as a whole" | `conceptless_arrow` (no label after inheritance); `G-LABEL` with the same exemption | Same |
| §3.2.2.3-4 | N | Squiggle unless the label link is obvious | Layout labels | Same |
| §3.2.2.3-5 | N | Arrow labels not solely function/input/control/output/mechanism/call | `L-NAME-03` | Same |
| §3.3.1.1 | N | A-0 single box; purpose and viewpoint stated | `context_box_count`; `L-META-01` | Same |
| §3.3.1.3 | N | DRE forms: C-number, page, node number, model note | v1: node number and C-number (§4.4) | Same |
| §3.3.1.4 | N | Text not redundant with boxes and arrows; glossary defines key words | FR-NAME-01/05; `L-DESC-02`..`05` | Same (info) |
| §3.3.1.5 | N | FEO diagrams | Not in v1 (v2); `F` refs parse but do not resolve | Same |
| §3.3.2.5 | N | Branches carry the same thing or portions; contents follow all branches | Bundles §8 | `rel` ignored in rendering |
| §3.3.2.6 | N | 3–6 boxes; staircase "normally"; parent arrows appear on child unless tunneled | `box_count_*`; layout; `icom_imbalance` (`missing_in_child`) | 3–6 |
| §3.3.2.7 | N | Child boundary arrows correspond to parent arrows | `icom_imbalance` (`missing_in_parent`) | Same |
| §3.3.2.8 | N | ICOM codes by parent position; roles may change | FR-ARROW-03/04/06; `L-ROLE-01` (info) | Same |
| §3.3.2.9 | N | Tunnels at connected and unconnected ends; uncoded border ends | §9; `[I1]` endpoints | `( )` glyphs |
| §3.3.2.10 | N | Caller has no child; label = reference; many callers; alternatives and interface differences need model notes | FR-CALL-02..07; `call_and_decomposed`, `call_unresolved`, `call_alternatives_without_note`, `call_interface_undocumented` | `call_interface_undocumented` is an error |
| §3.3.3-1..3 | N | Context diagrams `A-n`; A-0 required with one box numbered 0 | NodeId grammar; `missing_context`; `context_box_count` (A-0 only) | Same |
| §3.3.3-4; §3.4.2 | N | Non-context diagrams and A-1 have 3–6 boxes | `box_count_*` on decomposition and higher_context | 3–6 |
| §3.3.3-5; §3.3.4.1 | N | Box numbers in the lower-right inside corner, diagonal order; off-diagonal numbering | Derived numbers; **[Deviation]** strict staircase (info on import) | Same |
| §3.3.3-6 | N | DRE beneath a detailed box | Renderer (§4.4) | DRE only (no shadow) |
| §3.3.3-7 | N | Horizontal and vertical segments only | `G-ORTHO` | Same |
| §3.3.3-8 | N | ≥ 1 control and ≥ 1 output | `no_control`, `no_output` | Same |
| §3.3.3-9..11 | N | 0+ inputs, 0+ mechanisms, 0–1 call | Types | Same |
| §3.3.3-12 | N | Control feedback up and over; input and mechanism feedback down and under | Layout templates; `G-FEEDBACK` | Same |
| §3.3.3-13 | N | Boundary arrow has an ICOM code or is tunneled | By construction (FR-TUN-02) | Same |
| §3.3.3-14 | N | Same-data open boundary arrows fork/join unless unreadable | `duplicate_arrow_concept` (warning) | Label-equality detection |
| §3.3.3-15 | N | Box names and labels not solely function/activity/process/input/output/control/mechanism | `L-NAME-03` | Same |
| §3.3.4.2 | N | Node numbers; A0 children A1..An; **A0 diagram with 3–6 boxes is mandatory** | NodeId; `missing_a0_decomposition` (warning in `extended`) | Error |
| §3.3.4.2.1 | N | Node index: indented node numbers with titles or box names | `idef0 index` | Same |
| §3.3.4.2.2 | N | Node tree is optional | v1 emits the index only; graphical tree in [ingestion §export](ingestion.md#92-exports) | Same |
| §3.3.4.3 | N | Node reference = abbrev `/` node, optional F/T/G suffix and page | `DiagRef` grammar | Same |
| §3.3.4.4 | N | Model notes optional; consecutive from 1; squiggle only on the same diagram | §10; `L-NOTE-01`; `E-NOTE-01` | Same |
| §3.3.4.5 | N | Reference notation (`2O2 to 3C1`, `A21.3C2`, `A42.\|3\|`) | DSL endpoints; dotted normalization | Same |
| §3.4.2 | N | A-0: no ICOM codes or tunnels at unconnected ends; optional A-1 with A0 replacing one box; parental context TOP | Renderer; `context_tunnel`; `/higher_context`; `higher_context_mismatch` | Same |
| §3.4.3 | N | A-n (n > 1) high level with box −(n−1); A-1 is a context diagram but "not high level"; highest context "NONE" | Diagram kinds; FR-DIAG-09 | Same |
| §3.4.4 | N | FEO, text and glossary pages (`F`, `T`, `G`) | T/G export v1; FEO v2 | Same |
| §3.4.5 | N | Unique model name, "normally abbreviated (uniquely)" | `abbrev` required **[Extension]** (identity) | Required |
| §3.4.6-1..3 | N | Text accompanies diagram; glossary per node, alphabetical for publication | T/G export ([ingestion §export](ingestion.md#92-exports)) | Same |
| §3.4.6-4 | N | **When a TOC is provided**, it is a node tree or node index with node numbers, titles and box names | Publication TOC = node index | Same |

### 17.2 Informative annex guidance

| Clause | N/I | Guidance (abridged) | idef0-rs use | `fips183` profile |
|---|---|---|---|---|
| A.2 | I | Uniqueness of names and labels on a diagram | `L-NAME-01` (error); `duplicate_arrow_concept` | Same |
| B.2.2.1-1, -2 | I | Diagonal layout; upper-left boxes dominate via controls | Layout; Auto-Arrange (soft) | Same |
| B.2.2.1-4 | I | Working or draft copies may show node number **or** C-number as DRE | §4.4 theme option | Same |
| B.2.4 | I | Lay out the heavy constraint arrows first, then the boundary arrows, then the rest | Routing order: [layout §routing](layout.md#6-global-routing) | Same |
| B.2.4.1-2, -3 | I | When in doubt, control; do not split control and input on one box | `L-SPLIT-CI` (warning) | Same |
| B.2.4.2-3 | I | Avoid keywords, including "data", in names and labels | `L-NAME-04` (warning) | Same |
| B.2.4.2, B.2.4.3 | I | Placement and layout heuristics | FR-LAYOUT soft rules ([layout](layout.md)) | Same |
| B.2.5.2 | I | Reader notes informative; consecutive; never reused | §10; `.idef0/review/` | Same |
| C.2 | I | Kit Cycle | v2 | — |
| C.4, C.4.1.3 | I | Diagram form; status levels | §4.2, §4.5 | Same |
| C.4.1.5 | I | Context field: TOP, parent sketch, NONE | FR-DIAG-09 | Same |
| C.4.1.6 | I | Used At; copy into a model note when working fields are cut | FR-CALL-04; publication export | Same |
| C.4.4 | I | Title "must precisely match" the parent box name | `E-TITLE-MATCH` | Same |
| C.4.5.1 | I | C-number format and version history | §4.3; `L-CNUM-01` | Same |

### 17.3 Extensions and deviations by profile

Every idef0-rs-only rule, with its behavior under each profile. Rules not listed here behave identically in both profiles.

| Rule or feature | Tag | `extended` | `fips183` |
|---|---|---|---|
| Box limit 7 | [Deviation] | 3–7 | 3–6 |
| `[ ]` border-tunnel glyph | [Deviation] | `[ ]` | `( )` |
| Strict staircase (no off-diagonal boxes) | [Deviation] | Info on import | Info on import |
| `abbrev` required; keys; `@key` refs; `A21/I1` slash form | [Extension] | On | On (notation and identity only) |
| `unknown_concept` | [Extension] | Error | Info (label is free text) |
| `bundle_concept_mismatch`, `mixed_bundle_relation`, `L-BUN-01` | [Extension] | Error / error / warning | Info / info / off |
| `balance_concept_mismatch` | [Extension] | Warning | Info |
| `undescribed_function`, `undescribed_concept`, `undescribed_diagram`, `L-META-03` (model description) | [Extension] (D9) | Warning | Info |
| `undescribed_box_on_release`, `undescribed_concept_on_release` | [Extension] | Error | Info |
| `L-META-02` (scope) | [Extension] | Warning | Info |
| `L-DESC-02`..`L-DESC-05` | [Extension] | Info | Info |
| `L-CONCEPT-02`, `L-REUSE-01` | [Extension] | Info | Off |
| `unbound_mechanism`, `ungrounded_mechanism` | [Extension] | Error in CI, warning in editor | Info |
| `stale_mech_binding`, `stale_code_ref`, `implements_unknown_box`, `implements_non_leaf`, `consumes_unknown_port`, `stale_trace` | [Extension] | Per [validation](validation.md) | Info |
| `untraced_leaf` | [Extension] | Info | Off |
| `control_lock` | [Extension] | Error | Info |
| `call_interface_mismatch` (concept matching), `call_map_unknown_port` | [Extension] | Warning / error | Info |
| `call_interface_undocumented` (concept compatibility substitutes for the note) | [Deviation] in `extended` | Info | Error (FIPS §3.3.2.10) |
| `call_recursive` | [Extension] | Error | Info |
| Node-number corner label ([layout FR-STYLE-07](layout.md#13-themes-fr-style)) | [Extension] (theme) | `classic` theme | Box number (§3.3.3-5) |
| Drop shadow for detailed boxes | [Extension] (theme) | `classic`/`modern` | DRE only |
| Frame order of boundary ports by ICOM code | [Extension] | On | On (FIPS-compatible) |

---

## 18. Codes and open items

**Codes defined or changed in this document** (severities and rules live in the [validation registry](validation.md#7-diagnostic-registry)):

| Code | Kind | Subject | Summary |
|---|---|---|---|
| `E-PARSE-01`, `E-PARSE-02` | core | model | Syntax error; unknown attribute |
| `E-VERSION-01`, `E-VERSION-02` | core | model | Older version needs migrate; newer major refused |
| `E-ID-01`, `E-ID-05`, `E-ID-06` | core | model | Missing or malformed abbrev; malformed key; duplicate abbrev in workspace |
| `E-ID-02` / `E-ID-03` / `E-ID-04` | core | box / concept / edge | Duplicate box key / concept key / explicit arrow key |
| `E-NODE-01`..`E-NODE-03` | core | diagram | Invalid node digit; bad box numbering; diagram without a valid parent box |
| `E-TYPE-01`..`E-TYPE-05` | core | edge | Illegal source; illegal destination; boundary pass-through; rank gap or duplicate; ambiguous `tunnel=box` |
| `E-TITLE-MATCH` | core | diagram | Restated title differs from parent box name |
| `E-CALL-01`, `E-CALL-02` | core | box | Call on a nonexistent box number; call on the top box |
| `E-NOTE-01` | core | diagram | Note target not on this diagram |
| `L-META-02`, `L-META-03` (new) | lint | model | Scope missing; model description missing |
| `L-FMT-01` | lint | model | Missing version header (legacy 0.1) |
| `L-NAME-03` (corrected word lists) | lint | box | Name or attached label solely a reserved word (§3.3.3-15; `call` for labels only, §3.2.2.3-5) |
| `L-NAME-04` | lint | box | Name or attached label contains a B.2.4.2-3 keyword, including "data" |
| `L-GLOSS-01` (extended to labels) | lint | box | Acronym in a box name or attached label not defined in the glossary |
| `L-NOTE-01` | lint | diagram | Model notes not consecutive |
| `L-CNUM-01` | lint | diagram | Duplicate C-number |
| `L-CONCEPT-02` | lint | concept | Unused concept |
| `L-DESC-02` (extended), `L-DESC-03`, `L-DESC-04`, `L-DESC-05` | lint | box / concept / diagram / model | Description quality, one code per subject kind |
| `missing_a0_decomposition` | R | model | A0 diagram absent (its 3–6 box count is checked by R-BOX-01/R-BOX-04) |
| `context_box_count` (narrowed) | R | diagram | A-0 only |
| `higher_context_mismatch` | R | diagram | Top box ICOMs differ between A-1 and A-0 |
| `conceptless_arrow` (redefined) | R | edge | No label after `rel=all` inheritance (FIPS §3.2.2.3-3) |
| `call_interface_undocumented` (validation) | R | box | Interfaces differ and no model note (FR-CALL-06) |

**Open items:** nested and join-then-fork bundles (v2); decomposing environment boxes on A-n. Key retirement and C-number auto-assignment are decided (D17, D18).
