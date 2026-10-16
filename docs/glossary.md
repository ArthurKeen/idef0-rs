# idef0-rs Glossary

| Field | Value |
|---|---|
| Status | DRAFT v0.2 (2026-10-15) |
| Part of | idef0-rs spec set; index: [docs/PRD.md](PRD.md) |
| Normative references | FIPS PUB 183 ([local copy](research/fipspub183.pdf)); FIPS definitions are quoted from §2 unless noted |
| Scope | Every term used across the spec set, with a pointer to the section that defines it |

Conventions:
- **§x.y** cites FIPS 183. Annex material is *informative*.
- Links point to the defining spec section. "MM" is short for [spec/metamodel.md](spec/metamodel.md).
- **[Ext]** marks an idef0-rs extension, and **[Dev]** a deviation from FIPS (MM [§1.1](spec/metamodel.md#11-labels-used-in-this-document)).

## 1. IDEF0 terms (FIPS 183)

**A-0 diagram** ("A minus zero"): the required one-box context diagram. It holds the top box, its external arrows, and the purpose and viewpoint statements (§2.1, §3.3.1.1). Not the same as A0. → MM [§4](spec/metamodel.md#4-diagrams)

**A0**: the node number of the **top box** (box 0 on A-0), and of the diagram that details it. The A0 diagram, with 3–6 boxes, is mandatory (§3.3.4.2). Its children are A1..An, not A01. → MM [§3.2](spec/metamodel.md#32-node-numbers-and-box-numbers)

**A-n / higher context diagram** (n ≥ 1): an optional context diagram with 3–6 boxes (§2.16, §3.4.2); 3–7 under the `extended` profile. On A-1, the top box (shown as 0) replaces one box. On A-n (n ≥ 2), box −(n−1) is the lineage box. A-1 is a proper parent of A0 but "not high level"; only n > 1 is high level (§3.4.3). → MM [§3.2](spec/metamodel.md#32-node-numbers-and-box-numbers)

**Lineage box / environment box** [Ext terms]: on an A-n diagram, the lineage box is the box in the model's ancestry (0 on A-1, −(n−1) on A-n). The others are environment boxes, which have no node number and are referenced as `A-1.2`. → MM [§3.2](spec/metamodel.md#32-node-numbers-and-box-numbers)

**Activation**: one performance of a box's function under particular circumstances, using some combination of its inputs and controls (§3.3.2.2).

**Arrow / arrow segment**: a directed line conveying data or objects from source to use. It has one or more segments, which end at a box side, a branch or the boundary (§2.2, §2.4).

**Boundary arrow / internal arrow**: a boundary arrow has one unconnected end. An internal arrow connects boxes at both ends (§2.5, §2.31).

**Box**: a rectangle containing a name and a number, representing a function (§2.6).

**Box name / function name**: the active verb or verb phrase in the box (§2.7, §3.2.2.2). In idef0-rs, the box `name`. → MM [§5](spec/metamodel.md#5-boxes)

**Box number**: the digit in the box's lower-right inside corner: 0 on A-0, 1..6 elsewhere (§2.8, §3.3.4.1). idef0-rs derives it from diagonal position (1..7 in `extended`).

**Branch / fork / join**: a branch is a junction of segments (§2.9). A fork divides a segment and may unbundle meaning (§2.23). A join merges segments and may bundle meaning (§2.32). → MM [§8](spec/metamodel.md#8-bundles-forks-and-joins-fr-bun)

**Bundle / bundling**: "the combining of arrow meanings into a composite meaning", expressed by joins; unbundling by forks (§2.10, §3.3.2.5). idef0-rs refines branches with `rel=all|part_of|kind_of` [Ext]. → MM [§8](spec/metamodel.md#8-bundles-forks-and-joins-fr-bun)

**C-number**: a chronological creation number: author initials plus a sequence, written `AK14` in idef0-rs. Version history is written `AK15(AK14)` (§2.11, Annex C.4.5.1). → MM [§4.3](spec/metamodel.md#43-c-numbers)

**Call arrow**: a downward mechanism arrow meaning the **caller box** has no child diagram and is detailed by the **called box** elsewhere, in this model or another (§2.12, §3.3.2.10). idef0-rs stores targets by key and computes the label (`A5.2`, `SEC/A2.1`). → MM [§7](spec/metamodel.md#7-call-arrows-fr-call)

**Context / parental context**: context is the environment a function operates in (§2.15). The parental context of A-0 is `TOP`, and that of the highest A-n is `NONE` (§3.4.2–3.4.3).

**Control** (C, top side): "conditions required to produce correct output. Data or objects modeled as controls may be transformed by the function, creating output" (§2.17). Every box needs at least one. Controls are *not* "unconsumed". → MM [§6.1](spec/metamodel.md#61-roles)

**Decomposition / parent box / child diagram**: decomposition partitions a function into component functions (§2.18). A parent box is detailed by a child diagram, which shares its node number (§2.14, §2.41, §3.3.4.2).

**DRE (detail reference expression)**: the reference beneath a detailed box's lower-right corner. FIPS allows four forms: C-number, page number, node number and model-note number (§2.19, §3.3.1.3). v1 supports node number and C-number. → MM [§4.4](spec/metamodel.md#44-detail-reference-expressions-dre)

**Dominance**: upper-left boxes "dominate" lower-right boxes through the controls that link them (Annex B.2.2.1-2, informative). Auto-Arrange uses it as a soft objective.

**Feedback, up and over / down and under**: a backward arrow from a later box to an earlier one. Control feedback is drawn "up and over"; input and mechanism feedback "down and under" (§3.3.3-12). → [layout §templates](spec/layout.md#6-global-routing)

**FEO (For Exposition Only)**: a supplementary diagram that need not follow IDEF0 rules; node suffix `F` (§2.22, §3.3.1.5, §3.4.4). Deferred to v2.

**Glossary / text**: the glossary defines the key words, phrases and acronyms used with a node or model (§2.26, §3.3.1.4). Text is a non-redundant overview of a diagram (§2.47). idef0-rs maps the glossary to **concepts** and text to **diagram descriptions**.

**Text page / glossary page**: diagram-form sheets carrying text or glossary for a node; node suffixes `T` and `G`, e.g. `A312G` (§3.4.4). Exported in v1. → [ingestion §export](spec/ingestion.md#92-exports)

**ICOM**: Input, Control, Output, Mechanism, the four arrow roles. Each side of a box has one role (§3.2.2.1, Fig. 3). → MM [§6.1](spec/metamodel.md#61-roles)

**ICOM code**: a letter and number (`I1`, `C3`) written at the unconnected end of a child diagram's boundary arrow. It names the parent box's arrow at that position (§2.27, §3.3.2.8). Compare **port rank**.

**Input** (I, left side): "data or objects that are transformed by the function into output" (§2.29); "transformed or consumed" (§3.2.2.1). Optional.

**Kit Cycle**: the FIPS review procedure, in which authors circulate kits of diagrams and readers return reader notes (Annex C.2, informative). Deferred to v2.

**Mechanism** (M, bottom side, pointing up): "the means used to perform a function", including call arrows (§2.33). Some means may be inherited from the parent (§3.2.2.1). idef0-rs can bind mechanisms to code [Ext]. → MM [§6.2](spec/metamodel.md#62-mechanism-bindings-extension)

**Model note**: an optional, numbered note that is *part of* its diagram. Numbers run consecutively from 1. It may attach by squiggle only within its own diagram (§2.34, §3.3.4.4). Required for call alternatives and call interface differences (§3.3.2.10). → MM [§10](spec/metamodel.md#10-notes-fr-note)

**Reader note**: a circled, numbered review comment. It is *on* the diagram, not part of it, and strictly informative (Annex B.2.5.2). Stored in `.idef0/review/`; UI in v2.

**Node / node number**: a node is a parent box (§2.35). A node number gives a box's position in the hierarchy: the diagram node number plus the box number, e.g. box 2 on A25 is A252 (§2.37, §3.3.4.2).

**Node reference**: model abbreviation `/` diagram node number, plus optional suffix and page, e.g. `QA/A312`, `QA/A321F1` (§2.38, §3.3.4.3).

**Node index / node tree**: the indented outline of node numbers with titles or box names (§3.3.4.2.1), and its graphical tree form, which is optional (§3.3.4.2.2). When a TOC is provided, it must be one of the two (§3.4.6-4).

**Output** (O, right side): "data or objects produced by a function" (§2.40). Every box needs at least one.

**Purpose / viewpoint**: brief statements of why the model exists and from whose perspective. Required on A-0 (§2.43, §2.50, §3.3.1.1).

**Reference notation**: FIPS shorthand for parts of diagrams: `2O2 to 3C1`, `A21.3C2`, `A42.|3|`. An embedded period means "look specifically at" (§3.3.4.5). → MM [§3.6](spec/metamodel.md#36-reference-grammar-and-normalization)

**Squiggle**: a small jagged line that links a label to its arrow segment, or a note to a diagram element (§2.45, §3.2.2.3-4).

**Staircase**: the diagonal arrangement of boxes from upper left to lower right (§3.3.2.6: "normally"). idef0-rs enforces a strict staircase [Dev]: no off-diagonal boxes.

**Status**: `working` (major change), `draft` (minor change), `recommended` (approved with its text), `publication` (Annex C.4.1.3, informative). → MM [§4.2](spec/metamodel.md#42-status-semantics-annex-c413-informative)

**Title**: the verb phrase naming a diagram. A child diagram's title corresponds to its parent box name (§2.48; Annex C.4.4: "must precisely match"). → MM [§4.1](spec/metamodel.md#41-titles)

**"To all" convention**: a labeled stub ("to all X") replaces routing to every box (Annex B.2.4.3-8, informative). → [layout](spec/layout.md)

**Tunnel (tunneled arrow)**: an arrow exempt from parent/child correspondence (§2.49, §3.3.2.9).
- Tunneled at the **connected** (box) end: hidden on the child, keeps its ICOM code. Drawn `( )`.
- Tunneled at the **unconnected** (border) end: hidden on the parent, **no ICOM code**. idef0-rs draws `[ ]` [Dev]; the `fips183` theme draws `( )`.

→ MM [§9](spec/metamodel.md#9-tunnels-fr-tun)

**Used At**: per Annex C.4.1.6, "a list of diagrams … which use or reference this diagram-form page". idef0-rs lists callers on the diagram that contains the called box, and copies the list into a model note at publication. → MM [§7](spec/metamodel.md#7-call-arrows-fr-call)

## 2. idef0-rs modeling terms

**Abbrev / model id** [Ext]: the required model abbreviation (`FAC`). It is the model's identity across the API, tags and cross-model refs, and is unique per workspace. FIPS only says the name is "normally abbreviated" (§3.4.5). → MM [§3.1](spec/metamodel.md#31-model-id-resolution)

**Box key** [Ext]: a stable slug (`validate-order`), written as `key=` in the file. It never changes on rename or renumber, and is referenced as `@validate-order`. → MM [§3.3](spec/metamodel.md#33-box-and-concept-keys)

**Arrow id**: a deterministic arrow identifier: an explicit `key=`, or the structural form `A0:1O1->2I1#0`; bundle leaves add `/<i>`. → MM [§3.4](spec/metamodel.md#34-arrow-ids)

**Box number vs node number vs NodeRef**: a box number is local to a diagram (`1`). A node number is global within a model (`A21`). A NodeRef adds the model and an optional suffix (`ORD/A21`). A BoxRef may also use a key (`@validate-order`). → MM [§3.6](spec/metamodel.md#36-reference-grammar-and-normalization)

**PortRef**: a reference to one side and rank of a box, which is also the same-coded boundary port of its child: `A21/I1` (canonical [Ext]); FIPS form `A21I1`. Uncoded border ports are written `A21/[I1]`. → MM [§3.5](spec/metamodel.md#35-port-references)

**Port rank vs ICOM code**: rank is an arrow's 1-based position on a box side (top→bottom, left→right); it is semantic and stored. The ICOM code is that rank as written on the child's boundary. Re-ranking re-codes the child atomically (ReorderPorts). → MM [§13.2](spec/metamodel.md#132-reorderports-single-path)

**Uncoded port**: a boundary end tunneled at the border (`[I1]` in the DSL). The number is a local ordinal, not an ICOM code. → MM [§9](spec/metamodel.md#9-tunnels-fr-tun)

**Stub (expected port)**: a parent ICOM with no arrow yet on the child diagram, drawn unconnected on the child frame. A new child starts with only stubs.

**Concept** [Ext]: a model-level glossary entity with a key, a name and a definitional description. Every labeled arrow carries one, and concepts are shared across diagrams. Ontology relations: `kind_of` (taxonomy) and `part_of` (partonomy). → MM [§12](spec/metamodel.md#12-names-descriptions-and-concepts-fr-name)

**Description**: a definitional text for a function, concept, diagram or model (D9). It must not merely restate the boxes and arrows (§3.3.1.4). → MM [§12](spec/metamodel.md#12-names-descriptions-and-concepts-fr-name)

**Leaf**: a box (on A0 or below, or the top box) with **no child diagram**. Caller boxes are leaves, grounded through their call. The classic-theme drop shadow is only a *rendering* of "has a child". → MM [§5.1](spec/metamodel.md#51-leaf-structural-definition)

**Grounded / traced**: a leaf is grounded when existing code `@implements` it, directly or through a call. It is traced when a current (non-stale) requirement section is tagged to it. → [validation](spec/validation.md), [ingestion](spec/ingestion.md)

**Contract card**: the per-box export (Markdown or JSON) of name, description, ICOMs with concept definitions, requirements, code and Used At, for coding agents. → [agents §contract cards](spec/agents.md#2-contract-cards)

**Contract-complete**: a leaf that meets all six NORTHSTAR criteria: name and description, defined concepts, balance, traced, grounded, and zero errors. Their ratio is the north-star metric (FR-METRIC-01). → [NORTHSTAR](NORTHSTAR.md), [validation](spec/validation.md)

**CodeRef / DocRef**: `path::qualified::symbol` (a bare path means file scope) and `path#slug`. → MM [§6.2](spec/metamodel.md#62-mechanism-bindings-extension), [ingestion](spec/ingestion.md)

**Profile**: `extended` (default; 3–7 boxes; extensions on) or `fips183` (3–6; extensions off or downgraded to info; FIPS markers). → MM [§1.2](spec/metamodel.md#12-profiles-d8-d11)

**Extension vs deviation**: an extension adds what FIPS does not define (concepts, keys, code mechanisms) and never changes FIPS semantics. A deviation departs from FIPS (7 boxes, the `[ ]` glyph, strict staircase) and is always profiled. → MM [§17.3](spec/metamodel.md#173-extensions-and-deviations-by-profile)

**Normative vs informative**: FIPS Sections 1–3 are required; Annexes A–D are guidance (FIPS Introduction). The conformance matrix labels each row. → MM [§17](spec/metamodel.md#17-fips-183-conformance-matrix)

**Major / minor change**: a major change alters structure, names, labels, ICOM codes or calls, and resets the diagram to `working`. A minor change alters descriptions, notes or the C-number, and resets `recommended` or `publication` to `draft`. → MM [§13.5](spec/metamodel.md#135-major-and-minor-changes-fr-edit-15)

**Command / Patch / RenameMap**:
- A **Command** is a semantic edit (`AddBox`, `ReorderPorts`, …).
- Applying it yields a **Patch**: forward ops, inverse ops and a RenameMap. The Patch is the unit of undo, persistence and sync.
- The **RenameMap** records every identifier change (old→new or →∅) for node, port, arrow, key, concept, note or model, so tags can be rewritten.

→ MM [§13](spec/metamodel.md#13-editing-semantics-fr-edit)

**Proposal (pending)**: an agent- or LLM-originated change held for human acceptance before it reaches the undo stack (D15). → [agents §proposals](spec/agents.md#63-writes-pending-proposals-d15)

**Auto-Arrange**: an explicit, previewed patch that chooses box order and port order for a diagram, by searching box orders against a proxy objective, then ordering ports per side. It is never implicit, except for AI proposals and imports (D5). → [layout §auto-arrange](spec/layout.md#12-auto-arrange)

**Theme**: a rendering style: `modern`, `classic` (KBSI/AI0Win look: drop shadows, node-number corner labels), `fips183` (strict FIPS markers) or `retro` (ASCII/ANSI). Themes never change semantics. → [layout §themes](spec/layout.md#13-themes-fr-style)

**Workspace / `trace.lock` / index**: a workspace is a directory with `.idef0/`. `trace.lock` is committed and holds ack hashes, `sig_hash` baselines and tag memory (the box key each node-number tag resolved to at the last sync). `index.json` is a gitignored cache that maps abbrevs to files and holds the call index. → MM [§14](spec/metamodel.md#14-persistence-and-workspace-fr-ws)

## 3. Implementation terms

**BFF (backend for frontend)**: the local Axum server (`idef0-server`) behind the browser UI: persistence, ingest, AI calls. → [architecture](architecture.md)

**CST (lossless)**: the concrete syntax tree that keeps every byte, comments included, so UI edits round-trip comments. → MM [§14](spec/metamodel.md#14-persistence-and-workspace-fr-ws)

**Mangle / EDB / IDB**: Mangle is the Datalog dialect used for deductive rules. The EDB (extensional database) is the extracted input facts. The IDB (intensional database) is the derived facts, such as `violation/2`. → [validation](spec/validation.md)

**Extractor**: the function that turns a Model plus ExternalFacts into a sorted EDB. Its obligations are numbered. → [validation §extractor](spec/validation.md#3-fact-extraction-contract)

**Diagnostic code namespaces**: `E-*` core type and parse errors, `L-*` core lints, `R-*` Mangle rules (snake_case violation names), `G-*` layout geometry checks. One registry. → [validation §registry](spec/validation.md#7-diagnostic-registry)

**FSM**: the pure keyboard state machine in `idef0-core::editor`. The mouse drives the same transitions. → [editor-ux](spec/editor-ux.md)

**Scene**: the deterministic display list produced by layout (boxes, polylines, fillets, bumps, labels, frame), which the SVG and ASCII renderers consume. → [layout](spec/layout.md)

**gu (grid unit)**: the integer layout unit. Vertical metrics are multiples of 4 gu so the retro character grid aligns; export is fixed at zoom 1. → [layout](spec/layout.md)

**Channel / track**: free corridors between staircase boxes, and the parallel lanes inside them that carry routed arrow segments. → [layout §routing](spec/layout.md#6-global-routing)

**VCG (vertical constraint graph)**: precedence edges between channel segments that share an exit column. Cycles are broken with a dogleg. → [layout](spec/layout.md)

**Dogleg**: splitting one channel segment across two tracks at a free column, to break a VCG cycle (classic VLSI channel routing). → [layout](spec/layout.md)

**Bridge (bump)**: the small arc where a vertical segment jumps a horizontal one at a crossing. The vertical always carries it. → [layout](spec/layout.md)

**Geometry checker**: `G-*` checks that verify every hard layout rule on a Scene. Any finding is a layout bug. → [layout](spec/layout.md)

**SARIF**: Static Analysis Results Interchange Format 2.1.0, the CI output format for diagnostics. → [validation](spec/validation.md)

**ExternalFacts**: code and PRD facts supplied to validation from ingestion (server, CLI or a cache). → [validation §external facts](spec/validation.md#10-external-facts)
