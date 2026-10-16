# idef0-rs Validation Specification

| Field | Value |
|---|---|
| Status | DRAFT v0.2 |
| Part of | idef0-rs spec set; index: [docs/PRD.md](../PRD.md) |
| Normative references | FIPS PUB 183, *Integration Definition for Function Modeling (IDEF0)*, NIST, 21 Dec 1993 (local text: `docs/research/fipspub183.pdf`); Mangle language documentation (mangle.readthedocs.io); `mangle-rs` (codeberg.org/TauCeti/mangle-rs, pinned rev); OASIS SARIF 2.1.0 |
| Scope | The validation pipeline, fact extraction contract, Mangle schema and rules, severity resolution, the single diagnostic registry, output formats, project rules, external facts, engine backends and conformance fixtures. |

Terms (A-0 vs A0, port rank vs ICOM code, leaf, DRE, EDB/IDB) are defined in the [glossary](../glossary.md). The metamodel, DSL grammar, identity rules and the FIPS conformance matrix are in [metamodel](metamodel.md). Requirement IDs used here: `R-*` (rules), `FR-FACT-*` (extraction obligations, new), `FR-VAL-*` (pipeline requirements, new), `FR-METRIC-01`, plus the existing `FR-NAME-*` and `FR-CALL-*`.

---

## 1. Scope

This document is normative for:

- what the validator computes, and in which order (§2);
- the exact facts that `idef0-mangle::facts::extract` emits (§3), and their declarations (§4);
- every built-in rule (§5), its severity under each profile and context (§6), and its registry entry (§7);
- the shape of diagnostics in human, JSON and SARIF form (§8);
- the limits on organization-specific rules (§9) and on external facts (§10);
- the backend gate (§11) and the fixture policy that proves all of the above (§12).

Marking conventions follow the spec-set conventions in [PRD](../PRD.md): **[Extension]** marks an idef0-rs addition with no FIPS 183 basis, and **[Deviation]** marks a departure from FIPS 183. Every [Extension] rule states its `fips183` behavior. Annex A/B/C citations are *informative*. A FIPS-normative rule is never profile-gated: no profile relaxes it (decision D11).

Layout geometry checks (`G-*`) are specified in [layout](layout.md). They appear in the registry (§7) only so that all codes live in one table.

## 2. Pipeline

### 2.1 Stages

| # | Stage | Layer | Implementation | On failure |
|---|---|---|---|---|
| 1 | Parse and type check | `E-*` | `idef0-core` (Rust) | Stop. Only `E-*` diagnostics are reported, and no facts are extracted. |
| 2 | Core lints | `L-*` | `idef0-core::lint` (Rust) | Never stops. Emits diagnostics and `lint/2` facts. |
| 3 | Fact extraction | — | `idef0-mangle::facts::extract(&Model, &ExternalFacts)` | Infallible for a type-checked model (FR-FACT-01). |
| 4 | Built-in rules | `R-*` | `idef0.mg` + `severity.mg` + `profile_fips183.mg` via `RuleEngine` | Engine error is fatal (`E-RULES-01`). |
| 5 | Project rules | `R-*` (project codes) | `.idef0/rules/*.mg`, same program as stage 4 | Limit exceeded: `E-RULES-03`, then stage 4 is re-run alone (§9). |
| 6 | Diagnostics | all | `Validator` → `Report` | — |

- Stages 4 and 5 are one Mangle program and one evaluation; the split is conceptual. Project rules are appended after the built-in files in filename order.
- Stage 6 queries `violation/2`, `severity/2`, `subject_kind/2`, the detail predicates (`imbalance/4`, `lock_cycle/2`, `dup_concept/2`, `call_port_unmatched/3`, `call_port_differs/3`, `used_at/2`) and the metric predicates (`contract_complete/1`, `leaf_box/1`). It maps each tuple to a `Diagnostic` (§8).
- `G-*` checks run in `idef0-layout` on a `Scene`, outside this pipeline (`idef0 render --check`). They share the registry and the output formats.

**FR-VAL-01.** The validator is a pure function of (model bytes, `ExternalFacts`, profile, context, project rule sources). Identical inputs produce byte-identical `Report` JSON on native and `wasm32-unknown-unknown` (D6).

### 2.2 Evaluation contexts

| Context | Used by | Purpose |
|---|---|---|
| `/editor` | wasm UI, `idef0 serve` live diagnostics, `idef0 check` | Construction-phase leniency: incomplete-while-editing codes are warnings. |
| `/ci` | `idef0 validate` (default), `GET /api/models/{id}/diagnostics?context=ci` | Gate: the strict level. The contract-complete metric always uses CI severities. |

The context is an EDB fact (`eval_context/1`), so the program text never changes between contexts.

### 2.3 Profiles

| Aspect | `extended` (default) | `fips183` |
|---|---|---|
| Box limits (`profile_box_limits/2`) | 3–7 **[Deviation]** (D8) | 3–6 (FIPS §3.3.3-4) |
| [Extension] rules | On, at registry severities | Downgraded to `info`, or `off`, by `profile_fips183.mg` |
| FIPS-normative rules | Registry severities | Same, plus: `missing_a0_decomposition` and `call_interface_undocumented` become `error` |
| Project severity overrides | Any code | May not relax a FIPS-normative code below its default (`E-CONFIG-01`) |

**Mechanism (normative).** Rules are profile-agnostic: every rule derives its `violation/2` tuples under both profiles. The profile only changes EDB (`profile/1`, `profile_box_limits/2`) and severity resolution (§6). `/off` drops the diagnostic at stage 6. Two reasons favor an overlay over a `profile_extension_enabled/1` guard in every rule body:

- every rule is exercised by the same fixtures under both profiles, and the derived facts are identical, so profile bugs can only live in one small table;
- the metric (§5.13) and project rules see a stable set of derived predicates.

`profile_fips183.mg` is always loaded. Its facts are inert unless `profile(/fips183)` holds.

## 3. Fact extraction contract

`idef0-mangle::facts::extract` turns a type-checked `Model` plus `ExternalFacts` (§10) into a `FactSet`, the EDB of §4. Everything in this section is normative.

### 3.1 Identifiers

Identity follows [metamodel §3](metamodel.md#3-identity-fr-id). Fact identifiers are strings, except roles, kinds and statuses, which are Mangle names.

| Subject kind | Fact id | Example |
|---|---|---|
| model | model `abbrev` | `"FAC"` |
| diagram | canonical node id of the diagram; the context diagram is `"A-0"` | `"A1"`, `"A-0"`, `"A-1"` |
| box | canonical node id of the box (not its key) | `"A12"`, `"A0"` |
| port (coded boundary port) | `<diag>/<code>` | `"A21/I1"`, `"A-0/C2"` |
| port (uncoded border tunnel, FIPS §3.3.2.9) | `<diag>/[<role letter><ordinal>]` | `"A21/[I1]"` |
| edge | explicit `key=` if present; else `<diag>:<src>-><dst>#<n>`; bundle leaves `<trunk>/<i>` | `"A0:1O1->2I1#0"`, `"A1:1O1->*#0/1"` |
| concept | concept key | `"customer-order"` |
| coderef | `path::qualified::symbol`; file scope `path::<file>` | `"crates/idef0-layout/src/route.rs::assign_tracks"` |
| docref | `<path>#<slug>` | `"docs/prd.md#validate-order"` |

- Box ids are node ids, so a box and the diagram that decomposes it share a string (`"A1"`). The namespaces are kept apart by predicate (`described_box/1` vs `described_diagram/1`) and by the one-subject-kind-per-code rule (§7). A subject id alone is never interpreted without its code.
- Fork trunks use `*` as the destination and join trunks use `*` as the source in the structural id (`A1:1O1->*#0`, `A1:*->O1#0`). This is an assumption about the [metamodel](metamodel.md) arrow-id grammar.
- Port ids on A-0 exist, so that ICOM ranks can be checked, but are never rendered as ICOM codes (FIPS §3.4.2).

### 3.2 Obligations

| ID | Obligation |
|---|---|
| FR-FACT-01 | Extraction is total and infallible for any model that passed stage 1. |
| FR-FACT-02 | Output is deterministic: tuples are deduplicated and sorted by (predicate name, then arguments in byte order). No `HashMap` iteration order reaches the output. *(i)* |
| FR-FACT-03 | Emit `model(abbrev)` once (the model under evaluation). |
| FR-FACT-04 | Emit `diagram(D, Kind)` per diagram, with Kind `/context` (A-0 only), `/higher_context` (A-n, n ≥ 1) or `/decomposition`. |
| FR-FACT-05 | Emit `decomposes(P, D)` for every diagram D whose node number equals a box P's node number. A-0 has none. |
| FR-FACT-06 | Emit `box(D, B, Name)` and `box_key(B, Key)` per box. The top box `A0` gets a `box/3` fact on A-0 and on A-1 if A-1 exists. |
| FR-FACT-07 | Emit `edge_in(E, D)` for **every** edge, trunks and leaves alike. *(b, c)* |
| FR-FACT-08 | For each non-bundled arrow, emit `icom(E, RoleAtDst, Src, Dst)`, plus `icom_code(E, B, Role, K)` at each box end: `/output` with the source rank, and the destination role with the destination rank. |
| FR-FACT-09 | For each coded boundary end, emit `boundary(D, PortId, Role, K)`, where Role is the ICOM code letter (the parent-side role) and K the code number, even when the child-side role differs (FIPS §3.3.2.8). |
| FR-FACT-10 | Bundle trunks get `edge_in`, `arrow_concept` (if labeled), `bundle_kind`, and tunnel and mechanism facts. They get **no** `icom`/`icom_code` facts. *(b)* |
| FR-FACT-11 | Each bundle leaf runs from the true source to the true destination and gets `icom`, `icom_code`, `edge_in`, `bundle(T, L)` and `bundle_rel(T, L, Rel)`. |
| FR-FACT-12 | All leaves of a fork share the trunk's single source rank; all leaves of a join share the trunk's single destination rank. A bundle occupies one port at its shared end. *(e)* |
| FR-FACT-13 | An unlabeled `rel=all` leaf gets `arrow_concept(L, C)`, where C is the trunk's concept (FIPS §3.2.2.3-3 exemption: one label applies to the arrow as a whole). *(a)* |
| FR-FACT-14 | A labeled arrow gets `arrow_concept(E, K)`, where K resolves the label by concept key, name or synonym. An unresolved label yields `K = slug(label)` with no `concept/2` fact, so `unknown_concept` fires. |
| FR-FACT-15 | Trunk-level `tunnel`, `tunnel_box`, `mech_binding` and `mech_kind` facts are emitted on the trunk **and** pushed to every leaf at the shared end. A leaf-level attribute overrides the trunk's for that leaf. *(d)* |
| FR-FACT-16 | A `( )` tunnel at a box end emits `tunnel(E, /at_box)` and `tunnel_box(E, B)` naming that box. A box-to-box arrow may be tunneled at either end. |
| FR-FACT-17 | A `[ ]` border tunnel emits **both** `tunnel(E, /at_border)` and `tunneled_boundary(D, PortId, Role)`, with PortId in the `A21/[I1]` form, and **no** `boundary/4` fact for that port. *(g)* |
| FR-FACT-18 | A mechanism arrow with `code=` emits `mech_binding(E, CodeRef)` and `mech_kind(E, /code)`. `kind=component` or `kind=resource` emits `mech_kind` only. No attribute emits nothing. |
| FR-FACT-19 | Call targets are resolved from box keys (and from FIPS `A5.2` notation) to callee node ids **before** emission. Cross-model keys resolve through the workspace index. An unresolvable target is emitted as the literal `"@key"`, so `call_unresolved` fires. *(j)* |
| FR-FACT-20 | Every `call(B, M, N)` fact carries a model abbrev. Local targets carry the model's **own** abbrev. One fact is emitted per listed alternative. *(f)* |
| FR-FACT-21 | Emit `call_note(B)` only if the call's `note=` names a model note that exists on the caller's diagram. Emit `call_map/5` per `map=` pair. |
| FR-FACT-22 | Emit `described_box/1`, `described_concept/1` and `described_diagram/1` for subjects whose description is non-empty after trimming whitespace. |
| FR-FACT-23 | Emit `concept/2`, `concept_rel/3` and `diagram_status/2` from the model. |
| FR-FACT-24 | `code=` targets are existence-checked **directly** by the scanner: `code_exists(C)` is emitted when the file exists and (for non-file scope) the symbol resolves, whether or not the element carries an `@implements` tag. *(h)* |
| FR-FACT-25 | `implements`, `consumes` and `traces` facts from tags that name a box by key are resolved to node ids as in FR-FACT-19. Tags that address another model are not emitted. |
| FR-FACT-26 | Emit `lint(Code, Subject)` for every core lint finding whose registry row has *Fact = yes* (§7). Code is the lint's Mangle name (`L-NAME-01` → `/lint/name_01`). |
| FR-FACT-27 | Emit the configuration EDB: `profile/1`, `eval_context/1`, `profile_box_limits/2`, `severity_override/3`, and `index_present(/workspace)` when `ExternalFacts` carries a workspace index. |
| FR-FACT-28 | Facts carry no timestamps, absolute paths or host-dependent values. All paths are repo-relative with `/` separators. |

Italic letters refer to the review's extractor obligations: (a)–(f) from SR-11, (g)–(j) added in v0.2.

**Example (illustrative).** Diagram A1 contains this arrow (fork branches are separated by newlines, per the [grammar](metamodel.md#162-grammar)):

```idef0
arrow "Order": 1O1 -> fork {
  -> 2I1
  @rush-order -> 3C1 rel=kind_of
}
```

The facts are grouped here for reading, not in emission order:

```mangle
edge_in("A1:1O1->*#0", "A1").            arrow_concept("A1:1O1->*#0", "order").
bundle_kind("A1:1O1->*#0", /fork).
bundle("A1:1O1->*#0", "A1:1O1->*#0/0").  bundle_rel("A1:1O1->*#0", "A1:1O1->*#0/0", /all).
bundle("A1:1O1->*#0", "A1:1O1->*#0/1").  bundle_rel("A1:1O1->*#0", "A1:1O1->*#0/1", /kind_of).
edge_in("A1:1O1->*#0/0", "A1").          icom("A1:1O1->*#0/0", /input, "A11", "A12").
icom_code("A1:1O1->*#0/0", "A11", /output, 1).   icom_code("A1:1O1->*#0/0", "A12", /input, 1).
arrow_concept("A1:1O1->*#0/0", "order").         # FR-FACT-13: inherited from the trunk
edge_in("A1:1O1->*#0/1", "A1").          icom("A1:1O1->*#0/1", /control, "A11", "A13").
icom_code("A1:1O1->*#0/1", "A11", /output, 1).   # FR-FACT-12: shared source rank
icom_code("A1:1O1->*#0/1", "A13", /control, 1).
arrow_concept("A1:1O1->*#0/1", "rush-order").
```

### 3.3 Extractor invariants

Some conditions are true by construction for a conforming extractor. They are not user-facing diagnostics, because no DSL input can produce them. They are stated as Mangle rules in `crates/idef0-mangle/rules/invariants.mg`, a **test-and-debug-only** file. It is loaded in the extractor golden test, in the conformance suite on every backend, and in debug builds (`debug_assertions`), where any derived `extract_invariant/2` tuple panics.

```mangle
# (normative) rules/invariants.mg — not loaded in release evaluation
# Loaded together with idef0.mg and reuses its helpers (is_box, is_port).
endpoint_ok(X) :- is_box(X).
endpoint_ok(X) :- is_port(X).
port_used(P) :- icom(_, _, P, _).
port_used(P) :- icom(_, _, _, P).
is_trunk(T)  :- bundle_kind(T, _).
has_edge_in(E) :- edge_in(E, _).
border_tunneled_port(P) :- icom(E, _, P, _), tunnel(E, /at_border), is_port(P).
border_tunneled_port(P) :- icom(E, _, _, P), tunnel(E, /at_border), is_port(P).
coded_port(P) :- boundary(_, P, _, _).
tb_port(P)    :- tunneled_boundary(_, P, _).

extract_invariant(/inv_dangling_source, E) :- icom(E, _, S, _), !endpoint_ok(S).
extract_invariant(/inv_dangling_target, E) :- icom(E, _, _, T), !endpoint_ok(T).
extract_invariant(/inv_unlinked_boundary, P) :- is_port(P), !port_used(P).
extract_invariant(/inv_nested_bundle, T) :- is_trunk(T), bundle(_, T).
extract_invariant(/inv_trunk_has_icom, T) :- is_trunk(T), icom(T, _, _, _).
extract_invariant(/inv_leaf_without_edge_in, L) :- bundle(_, L), !has_edge_in(L).
extract_invariant(/inv_fork_source_mismatch, L) :- bundle_kind(T, /fork), bundle(T, L),
    icom(L, _, S1, _), bundle(T, L2), icom(L2, _, S2, _), S1 != S2.
extract_invariant(/inv_join_target_mismatch, L) :- bundle_kind(T, /join), bundle(T, L),
    icom(L, _, _, D1), bundle(T, L2), icom(L2, _, _, D2), D1 != D2.
# FR-FACT-17: the two border-tunnel representations agree.
extract_invariant(/inv_tunnel_repr, P) :- border_tunneled_port(P), coded_port(P).
extract_invariant(/inv_tunnel_repr, P) :- border_tunneled_port(P), !tb_port(P).
extract_invariant(/inv_tunnel_repr, P) :- tb_port(P), !border_tunneled_port(P).
```

- **Invariant (FR-FACT-17).** A port is in `tunneled_boundary/3` if and only if it is the boundary endpoint of at least one edge with `tunnel(E, /at_border)`, and such a port never has a `boundary/4` fact.
- v1 bundles are single-level: the DSL grammar has no nested forks and no fork-into-join chains. So every leaf belongs to exactly one bundle and `bundle_reach` equals `bundle`. Nesting would require revisiting §5.6 and §5.8.
- These invariants replace the former user-facing codes `dangling_source`, `dangling_target`, `unlinked_boundary`, `bundle_cycle`, `fork_source_mismatch` and `join_target_mismatch` (SR-15). Each has a fact-level fixture (§12).

## 4. Schema

`crates/idef0-mangle/rules/schema.mg`. The schema extends the predicates of the original product brief (`box/3`, `icom/4`, `bundle/2`, `implements/2`), whose identifier and role semantics changed in v0.2. It is not a verbatim copy.

```mangle
# (normative) rules/schema.mg
# ---------- Structural EDB ----------
Decl model(Abbrev) descr [doc("The model under evaluation.")] bound [/string].
Decl diagram(DiagId, Kind)
  descr [doc("Kind is /context (A-0 only), /higher_context (A-n, n>=1) or /decomposition.")]
  bound [/string, /name].
Decl box(DiagId, BoxId, Name) bound [/string, /string, /string].
Decl box_key(BoxId, Key) bound [/string, /string].
Decl decomposes(BoxId, DiagId) descr [doc("DiagId details box BoxId.")] bound [/string, /string].
Decl icom(EdgeId, Type, Source, Target)
  descr [doc("Type is the role at the destination (/input, /control, /mechanism), or /output when Target is a boundary O port. Bundle leaves only, never trunks.")]
  bound [/string, /name, /string, /string].
Decl icom_code(EdgeId, BoxId, Role, K)
  descr [doc("EdgeId attaches to BoxId's side Role at port rank K; /output on the source side.")]
  bound [/string, /string, /name, /number].
Decl boundary(DiagId, PortId, Role, K)
  descr [doc("Coded boundary port; Role is the ICOM code letter (parent side).")]
  bound [/string, /string, /name, /number].
Decl tunneled_boundary(DiagId, PortId, Role)
  descr [doc("Uncoded border-tunnel port A21/[I1] (FIPS 3.3.2.9).")]
  bound [/string, /string, /name].
Decl edge_in(EdgeId, DiagId) bound [/string, /string].
Decl tunnel(EdgeId, End) descr [doc("End is /at_box or /at_border.")] bound [/string, /name].
Decl tunnel_box(EdgeId, BoxId) descr [doc("EdgeId is tunneled at its connected end on BoxId.")] bound [/string, /string].
Decl bundle(Trunk, Leaf) bound [/string, /string].
Decl bundle_kind(Trunk, Kind) descr [doc("/fork or /join.")] bound [/string, /name].
Decl bundle_rel(Trunk, Leaf, Rel) descr [doc("/all, /kind_of or /part_of.")] bound [/string, /string, /name].
Decl call(CallerBox, Model, CalleeNode)
  descr [doc("One fact per alternative; local calls carry the model's own abbrev.")]
  bound [/string, /string, /string].
Decl call_note(CallerBox) bound [/string].
Decl call_map(CallerBox, CallerRole, CallerK, CalleeRole, CalleeK)
  bound [/string, /name, /number, /name, /number].
Decl mech_binding(EdgeId, CodeRef) bound [/string, /string].
Decl mech_kind(EdgeId, Kind) descr [doc("/code, /component or /resource.")] bound [/string, /name].

# ---------- Names, descriptions, concepts (FR-NAME) ----------
Decl described_box(BoxId) bound [/string].
Decl described_concept(ConceptKey) bound [/string].
Decl described_diagram(DiagId) bound [/string].
Decl concept(ConceptKey, Name) bound [/string, /string].
Decl concept_rel(Child, Parent, Rel) descr [doc("/kind_of or /part_of.")] bound [/string, /string, /name].
Decl arrow_concept(EdgeId, ConceptKey) bound [/string, /string].
Decl diagram_status(DiagId, Status)
  descr [doc("/working, /draft, /recommended or /publication (Annex C.4.1.3, informative).")]
  bound [/string, /name].
Decl lint(Code, Subject) descr [doc("Core lint finding; Code is /lint/<name>.")] bound [/name, /string].

# ---------- External EDB (section 10) ----------
Decl code_ref(CodeRef, Path, Symbol, Lang) bound [/string, /string, /string, /name].
Decl code_exists(CodeRef) bound [/string].
Decl implements(CodeRef, BoxId) bound [/string, /string].
Decl consumes(CodeRef, BoxId, Role, K) bound [/string, /string, /name, /number].
Decl doc_ref(DocRef, Path, Slug) bound [/string, /string, /string].
Decl traces(DocRef, BoxId) bound [/string, /string].
Decl requirement(ReqId, DocRef) descr [doc("Requirement token (e.g. REQ-031) in a tagged section.")] bound [/string, /string].
Decl doc_stale(DocRef) bound [/string].
Decl ext_model(Model) bound [/string].
Decl ext_box(Model, Node) descr [doc("A box in another workspace model.")] bound [/string, /string].
Decl ext_grounded(Model, Node) descr [doc("Some box in Node's subtree in Model has existing @implements code.")] bound [/string, /string].
Decl index_present(Scope) descr [doc("/workspace when a workspace index was supplied.")] bound [/name].

# ---------- Configuration EDB ----------
Decl profile(Profile) descr [doc("/extended or /fips183.")] bound [/name].
Decl eval_context(Ctx) descr [doc("/editor or /ci.")] bound [/name].
Decl profile_box_limits(Min, Max) bound [/number, /number].
Decl severity_override(Code, Ctx, Level) descr [doc("Project override from .idef0/config.")] bound [/name, /name, /name].

# ---------- Registry facts (severity.mg, profile_fips183.mg) ----------
Decl severity_default(Code, EditorLevel, CiLevel) bound [/name, /name, /name].
Decl subject_kind(Code, Kind)
  descr [doc("/box, /diagram, /edge, /port, /concept, /coderef, /docref or /model.")]
  bound [/name, /name].
Decl fips183_severity(Code, Level) bound [/name, /name].

# ---------- Output IDB ----------
Decl violation(Code, Subject) bound [/name, /string].
Decl severity(Code, Level) descr [doc("Resolved for the current context: /error, /warning, /info or /off.")] bound [/name, /name].
Decl imbalance(Box, Role, K, Kind) bound [/string, /name, /number, /name].
Decl lock_cycle(A, B) bound [/string, /string].
Decl dup_concept(DiagId, ConceptKey) bound [/string, /string].
Decl used_at(CalleeNode, CallerBox) bound [/string, /string].
Decl call_port_unmatched(CallerBox, Role, K) bound [/string, /name, /number].
Decl call_port_differs(CallerBox, Role, K) bound [/string, /name, /number].
Decl leaf_box(BoxId) bound [/string].
Decl contract_complete(BoxId) bound [/string].
```

## 5. Rules

`crates/idef0-mangle/rules/idef0.mg` (normative). All rules are safe: every variable in a head, a negated atom or a comparison is bound by a positive atom in the same body. All rules are stratified: negation applies only to predicates in strictly lower strata. **No rule that defines `violation/2`, `lint/2` or `severity/2` may depend on a §5.13 metric predicate.** The metric stratum sits on top, and nothing reads it except stage 6.

The program uses only the MiniEngine subset (§11): positive and negated atoms, `!=`, `:lt/2`, string and name constants, and `|> do fn:group_by(…), let N = fn:count()`.

### 5.1 Helpers

```mangle
# (normative)
is_box(B)          :- box(_, B, _).
decomposed(B)      :- decomposes(B, _).
has_call(B)        :- call(B, _, _).
has_box(D)         :- box(D, _, _).
has_output(B)      :- icom_code(_, B, /output, _).
has_control(B)     :- icom(_, /control, _, B).
is_port(P)         :- boundary(_, P, _, _).
is_port(P)         :- tunneled_boundary(_, P, _).
has_port(B, R, K)  :- icom_code(_, B, R, K).
proper_diagram(D)  :- diagram(D, /decomposition).
proper_diagram(D)  :- diagram(D, /higher_context).
parent_box(C, P)   :- box(D, C, _), decomposes(P, D).
in_subtree(N, N)   :- is_box(N).
in_subtree(X, N)   :- parent_box(X, P), in_subtree(P, N).
# Leaf: a box in the A0 subtree with no child diagram of its own (glossary "leaf"; NORTHSTAR).
# Caller boxes ARE leaves: they have no child diagram; they are grounded through their callee (FR-CALL-10).
# Environment boxes on A-n context diagrams are not in the A0 subtree, so they are not leaves.
leaf_box(B)        :- in_subtree(B, "A0"), !decomposed(B).
```

### 5.2 Diagram structure (R-BOX, R-CTX)

```mangle
# (normative)
box_count(D, N) :- box(D, _, _) |> do fn:group_by(D), let N = fn:count().

# R-BOX-01 (low) and R-BOX-04 (high): proper diagrams (decomposition and A-n) have Min..Max boxes (FIPS 3.3.3-4; 3.4.2-3.4.3).
violation(/box_count_low, D)  :- proper_diagram(D), box_count(D, N), profile_box_limits(Min, _), :lt(N, Min).
violation(/box_count_high, D) :- proper_diagram(D), box_count(D, N), profile_box_limits(_, Max), :lt(Max, N).
# R-BOX-02: a diagram with no boxes (box_count has no row, so box_count_low cannot fire).
violation(/empty_diagram, D)  :- diagram(D, _), !has_box(D).
# R-BOX-03: A-0 has exactly one box (FIPS 3.3.3-2). A-n diagrams are proper diagrams, not context boxes.
violation(/context_box_count, "A-0") :- diagram("A-0", /context), box_count("A-0", N), N != 1.

# R-CTX-01..03
has_a0(M)        :- model(M), diagram("A-0", /context).
a0_decomposed(M) :- model(M), decomposes("A0", "A0").
violation(/missing_context, M)          :- model(M), !has_a0(M).                # FIPS 3.3.3-2
violation(/missing_a0_decomposition, M) :- has_a0(M), !a0_decomposed(M).         # FIPS 3.3.4.2
violation(/context_tunnel, E)           :- edge_in(E, "A-0"), tunnel(E, /at_border).   # FIPS 3.4.2
```

- On A-1 the A0 box takes the place of one numbered box. On A-n it is box −(n−1). Either way it counts toward the limits (FIPS §3.4.2–3.4.3).
- `missing_a0_decomposition`: FIPS §3.3.4.2 says every model "shall also have at least three, but no more than six, child boxes on the A0 child diagram". The count itself is R-BOX-01/R-BOX-04 on diagram `A0`; this rule fires only when the A0 diagram is absent. A model being built legitimately starts with A-0 only, so `extended` reports a warning, and `fips183` reports an error in both contexts.

### 5.3 Function minimums (R-CTL-01, R-OUT-01)

```mangle
# (normative) FIPS 3.3.3-8: at least one control and one output per box.
violation(/no_control, B) :- is_box(B), !has_control(B).
violation(/no_output, B)  :- is_box(B), !has_output(B).
```

### 5.4 Control lock (R-LOCK-01) [Extension]

```mangle
# (normative)
ctrl_live(B)     :- icom(_, /control, S, B), is_port(S).
ctrl_live(B)     :- icom(_, /control, S, B), ctrl_live(S).
ctrl_dep(A, B)   :- icom(_, /control, A, B), is_box(A).
ctrl_reach(A, B) :- ctrl_dep(A, B).
ctrl_reach(A, C) :- ctrl_reach(A, B), ctrl_dep(B, C).
locked(B)        :- has_control(B), !ctrl_live(B).
on_lock_cycle(A) :- locked(A), ctrl_reach(A, A).
violation(/control_lock, B) :- locked(B), on_lock_cycle(A), ctrl_reach(A, B).
lock_cycle(A, B) :- on_lock_cycle(A), ctrl_reach(A, B), ctrl_reach(B, A).
```

- A box is *locked* when none of its controls can be traced back to the diagram boundary through control-live boxes. The rule fires only for locked boxes on a control cycle, or downstream of one. A box that is locked only because an upstream box has no control is reported by `no_control`, not here.
- The rule considers controls only and ignores inputs: a box whose controls come only from a closed feedback loop is flagged even if fresh inputs arrive.
- It does not read `violation/2` (fixing SR-14), so project rules cannot break its stratification.
- **fips183:** `info`. FIPS 183 has no liveness rule; feedback is legal (§3.3.3-12).

### 5.5 ICOM balance (R-BAL-01)

```mangle
# (normative) FIPS 3.3.2.6-3.3.2.9
parent_port(P, R, K) :- decomposes(P, _), icom_code(E, P, R, K), !tunnel_box(E, P).
child_port(D, R, K)  :- boundary(D, _, R, K).        # uncoded border tunnels never participate
balanced_down(P, R, K) :- parent_port(P, R, K), decomposes(P, D), child_port(D, R, K).
balanced_up(D, R, K)   :- child_port(D, R, K), decomposes(P, D), parent_port(P, R, K).
imbalance(P, R, K, /missing_in_child)  :- parent_port(P, R, K), !balanced_down(P, R, K).
imbalance(P, R, K, /missing_in_parent) :- decomposes(P, D), child_port(D, R, K), !balanced_up(D, R, K).
violation(/icom_imbalance, P) :- imbalance(P, _, _, _).
```

- Balance matches on the parent's side and code, never on the child-side role. A role change is legal (FIPS §3.3.2.8, Fig. 15) and only produces the info lint `L-ROLE-01`.
- The A0 box appears on A-0 and on A-1, so the A0 diagram must balance against both views. FIPS §3.4.2 makes A-1 the proper parent of A0.
- FIPS §3.3.3-13 (a boundary arrow has a code or is tunneled) holds by construction: codes are assigned by the core.

### 5.6 Bundles (R-BUN)

```mangle
# (normative)
bundle_reach(A, B) :- bundle(A, B).
bundle_reach(A, C) :- bundle_reach(A, B), bundle(B, C).
# R-BUN-02 [Extension]: one fork/join must not mix taxonomy and partonomy siblings.
violation(/mixed_bundle_relation, T) :- bundle_rel(T, _, /kind_of), bundle_rel(T, _, /part_of).
```

- The `kind_of`/`part_of` split of FIPS §3.3.2.5's "more general category" is an idef0-rs refinement. **fips183:** `info` (both render as labeled branches).
- `bundle_concept_mismatch` (R-BUN-03) is in §5.7. Former R-BUN-01 `bundle_cycle` and the fork/join endpoint checks are now extractor invariants (§3.3).

### 5.7 Names, descriptions and concepts (R-NAME, R-BUN-03)

```mangle
# (normative)
is_concept(K)   :- concept(K, _).
has_concept(E)  :- arrow_concept(E, _).
released(D)     :- diagram_status(D, /recommended).
released(D)     :- diagram_status(D, /publication).
concept_on(D, K)      :- edge_in(E, D), arrow_concept(E, K).
box_released(B)       :- released(D), box(D, B, _).
concept_released(K)   :- released(D), concept_on(D, K), is_concept(K).

# FR-NAME-01/02/05 [Extension]: warnings while working/draft ...
violation(/undescribed_function, B) :- is_box(B), !described_box(B), !box_released(B).
violation(/undescribed_concept, K)  :- is_concept(K), !described_concept(K), !concept_released(K).
violation(/undescribed_diagram, D)  :- diagram(D, _), !described_diagram(D).
# ... FR-NAME-06 [Extension]: errors once released. Exactly one of each pair fires.
violation(/undescribed_box_on_release, B)     :- box_released(B), !described_box(B).
violation(/undescribed_concept_on_release, K) :- concept_released(K), !described_concept(K).

# FR-NAME-03
violation(/unknown_concept, E)   :- arrow_concept(E, K), !is_concept(K).      # [Extension]
violation(/conceptless_arrow, E) :- icom(E, _, _, _), !has_concept(E).       # FIPS 3.2.2.3-3

# Concept ontology [Extension]
concept_isa(C, P, R) :- concept_rel(C, P, R).
concept_isa(C, Q, R) :- concept_isa(C, P, R), concept_rel(P, Q, R).
concept_compatible(A, A) :- is_concept(A).
concept_compatible(A, B) :- concept_isa(A, B, _).
violation(/concept_cycle, C) :- concept_isa(C, C, _).

# FR-NAME-04 / R-BUN-03 [Extension]: branch relations agree with the ontology.
bundle_ok(T, L) :- bundle_rel(T, L, /all), arrow_concept(T, K), arrow_concept(L, K).
bundle_ok(T, L) :- bundle_rel(T, L, R), R != /all, arrow_concept(T, KT), arrow_concept(L, KL),
                   concept_isa(KL, KT, R).
violation(/bundle_concept_mismatch, L) :- bundle_rel(T, L, _), !bundle_ok(T, L).
```

- **Subject namespaces (SR-10).** `described/1` is split into three predicates, so a described box `A1` no longer masks an undescribed diagram `A1`. `undescribed_on_release` is split by subject kind.
- **`conceptless_arrow` is FIPS-normative.** It fires only when an arrow has no label at all after FR-FACT-13 inheritance, which is exactly FIPS §3.2.2.3-3 including its whole-arrow exemption. Call arrows are not `icom` edges and are exempt (FIPS §3.2.2.3-2e). Under `fips183` any label suffices, because a label that is not a glossary concept only produces `unknown_concept`, and that is downgraded to `info`.
- **fips183:** every [Extension] code in this block is `info`.

### 5.8 Concept balance (R-BAL-02) [Extension]

A forked parent output's leaves carry *branch* concepts, so the comparison uses the concept at the attached end: the fork trunk's concept at the source, and the join trunk's concept at the destination (SR-12).

```mangle
# (normative)
forked_leaf(L) :- bundle_kind(T, /fork), bundle(T, L).
joined_leaf(L) :- bundle_kind(T, /join), bundle(T, L).
src_concept(E, C) :- icom(E, _, _, _), arrow_concept(E, C), !forked_leaf(E).
src_concept(L, C) :- bundle_kind(T, /fork), bundle(T, L), arrow_concept(T, C).
dst_concept(E, C) :- icom(E, _, _, _), arrow_concept(E, C), !joined_leaf(E).
dst_concept(L, C) :- bundle_kind(T, /join), bundle(T, L), arrow_concept(T, C).
parent_concept(P, /output, K, C) :- decomposes(P, _), icom_code(E, P, /output, K), icom(E, _, P, _), src_concept(E, C).
parent_concept(P, R, K, C) :- decomposes(P, _), icom_code(E, P, R, K), R != /output, icom(E, _, _, P), dst_concept(E, C).
port_concept(D, R, K, C) :- boundary(D, P, R, K), icom(E, _, P, _), src_concept(E, C).
port_concept(D, R, K, C) :- boundary(D, P, R, K), icom(E, _, _, P), dst_concept(E, C).
compatible_port(P, R, K) :- decomposes(P, D), parent_concept(P, R, K, CP), port_concept(D, R, K, CC),
                            concept_compatible(CC, CP).
violation(/balance_concept_mismatch, P) :- decomposes(P, D), parent_concept(P, R, K, _),
                                           port_concept(D, R, K, _), !compatible_port(P, R, K).
```

The child port's concept must equal the parent's or refine it (`kind_of`/`part_of`, transitively). **fips183:** `info`, because FIPS balances by ICOM code only.

### 5.9 Call arrows (R-CALL)

```mangle
# (normative)
calls_local(B, N)      :- call(B, M, N), model(M).
call_resolved(B, M, N) :- call(B, M, N), model(M), is_box(N).
call_resolved(B, M, N) :- call(B, M, N), ext_box(M, N).
call_checkable(B, M, N) :- call(B, M, N), model(M).
call_checkable(B, M, N) :- call(B, M, N), index_present(/workspace).

violation(/call_and_decomposed, B) :- call(B, _, _), decomposed(B).                       # FR-CALL-02
violation(/call_unresolved, B)     :- call_checkable(B, M, N), !call_resolved(B, M, N).    # FR-CALL-07
call_alternatives(B, K) :- call(B, _, _) |> do fn:group_by(B), let K = fn:count().
violation(/call_alternatives_without_note, B) :- call_alternatives(B, K), :lt(1, K), !call_note(B).  # FR-CALL-05

# FR-CALL-08 [Extension]: a call must not expand into itself, directly or through call chains.
expands(B, X) :- calls_local(B, N), in_subtree(X, N).
expands(B, X) :- expands(B, Y), calls_local(Y, N), in_subtree(X, N).
violation(/call_recursive, B) :- expands(B, B).

# FR-CALL-04: Used At reverse index (detail query, rendered on the callee's diagram form).
used_at(N, B) :- calls_local(B, N).

# FR-CALL-06, FIPS form: interfaces that differ in number or meaning need a model note.
icom_role(/input).  icom_role(/control).  icom_role(/output).    # mechanisms are not interface-checked
call_port_mapped(B, R, K)   :- call_map(B, R, K, _, _).
callee_port_mapped(B, R, K) :- call_map(B, _, _, R, K).
caller_has(B, R, K) :- calls_local(B, _), icom_code(_, B, R, K), icom_role(R).
callee_has(B, R, K) :- calls_local(B, N), icom_code(_, N, R, K), icom_role(R).
same_concept_at(B, R, K) :- calls_local(B, N), icom_code(E1, B, R, K), icom_code(E2, N, R, K),
                            arrow_concept(E1, C), arrow_concept(E2, C).
call_port_differs(B, R, K) :- caller_has(B, R, K), !same_concept_at(B, R, K), !call_port_mapped(B, R, K).
call_port_differs(B, R, K) :- callee_has(B, R, K), !caller_has(B, R, K), !callee_port_mapped(B, R, K).
violation(/call_interface_undocumented, B) :- call_port_differs(B, _, _), !call_note(B).

# FR-CALL-06, concept form [Extension]: correspondence by role and concept direction.
in_role(/input).  in_role(/control).
callee_accepts_in(B, C) :- calls_local(B, N), icom_code(E, N, R, _), in_role(R), arrow_concept(E, C2),
                           concept_compatible(C, C2).
callee_provides(B, C)   :- calls_local(B, N), icom_code(E, N, /output, _), arrow_concept(E, C2),
                           concept_compatible(C2, C).
call_port_unmatched(B, R, K) :- calls_local(B, _), icom_code(E, B, R, K), in_role(R), arrow_concept(E, C),
                                !callee_accepts_in(B, C), !call_port_mapped(B, R, K).
call_port_unmatched(B, /output, K) :- calls_local(B, _), icom_code(E, B, /output, K), arrow_concept(E, C),
                                      !callee_provides(B, C), !call_port_mapped(B, /output, K).
violation(/call_interface_mismatch, B) :- call_port_unmatched(B, _, _), !call_note(B).

# CallPortMap sanity [Extension]
callee_has_port(B, R, K) :- calls_local(B, N), has_port(N, R, K).
violation(/call_map_unknown_port, B) :- calls_local(B, _), call_map(B, R, K, _, _), !has_port(B, R, K).
violation(/call_map_unknown_port, B) :- calls_local(B, _), call_map(B, _, _, R, K), !callee_has_port(B, R, K).
```

- **Direction (SR-13).** A caller input or control concept must equal or refine some callee input/control concept: the callee accepts the more general thing. A callee output concept must equal or refine the caller's output concept: the callee provides at least what the caller promises. Mechanisms are not checked; they are the caller's own resources.
- **Cross-model calls are not interface-checked in v1.** `calls_local` restricts every interface rule to targets in the same model. Cross-model targets are only checked for resolution (`call_unresolved`), and only when a workspace index is present (`index_present(/workspace)`). This avoids false errors in the offline client without `index.json`.
- **Alternatives.** With several targets, a port counts as matched if any alternative matches. Alternatives always need a model note anyway (`call_alternatives_without_note`).
- **Profiles.** Under `fips183`, `call_interface_undocumented` is an `error` (FIPS §3.3.2.10: model notes "shall specify the relationships"), and concept matching (`call_interface_mismatch`) is `info`. Under `extended`, concept compatibility substitutes for the note. `call_interface_undocumented` is then `info` and `call_interface_mismatch` is a `warning` **[Deviation]**.
- `call_recursive` **fips183:** `info`. FIPS does not forbid it, and recursive software functions are real. It is an error in `extended` because a recursive expansion cannot produce a finite contract card.

### 5.10 Uniqueness of arrow concepts (R-UNIQ-01)

```mangle
# (normative)
bundled_leaf(L)  :- bundle(_, L).
arrow_root(E, E) :- edge_in(E, _), !bundled_leaf(E).
arrow_root(L, T) :- bundle_reach(T, L), !bundled_leaf(T).
dup_concept(D, K) :- edge_in(E1, D), edge_in(E2, D), arrow_concept(E1, K), arrow_concept(E2, K),
                     arrow_root(E1, R1), arrow_root(E2, R2), R1 != R2.
violation(/duplicate_arrow_concept, D) :- dup_concept(D, _).
```

- Two arrows on one diagram carrying the same concept must belong to one bundle (same root). The violation fires **once per diagram**. The detail query `dup_concept/2` yields one row per (diagram, concept), and the diagnostic message lists each concept. This fixes the double firing per pair (SR-14).
- Basis: for open-ended boundary arrows, FIPS §3.3.3-14 (normative: "shall be connected through a fork… unless this results in an unreadable diagram"). For internal arrows, Annex A.2, "uniqueness of names and labels on a diagram" (informative). The escape clause makes this a warning in both profiles.

### 5.11 Mechanisms and code grounding (R-MECH) [Extension]

```mangle
# (normative)
grounded(B) :- implements(C, B), code_exists(C).
grounded(B) :- calls_local(B, N), in_subtree(X, N), grounded(X).      # follows call chains (FR-CALL-10)
grounded(B) :- call(B, M, N), ext_grounded(M, N).                     # cross-model callee (section 10)
mech_bound(E) :- mech_binding(E, C), code_exists(C).
mech_ok(E)    :- mech_bound(E).
mech_ok(E)    :- mech_kind(E, /resource).
mech_ok(E)    :- mech_kind(E, /component).
code_mech(B)  :- leaf_box(B), icom(E, /mechanism, _, B), mech_binding(E, _).

violation(/unbound_mechanism, E)      :- icom(E, /mechanism, _, B), leaf_box(B), !mech_ok(E).
violation(/ungrounded_mechanism, B)   :- code_mech(B), !grounded(B).
violation(/stale_mech_binding, E)     :- mech_binding(E, _), !mech_bound(E).
violation(/stale_code_ref, C)         :- implements(C, _), !code_exists(C).
violation(/implements_unknown_box, C) :- implements(C, B), !is_box(B).
violation(/implements_non_leaf, C)    :- implements(C, B), decomposed(B).
violation(/consumes_unknown_port, C)  :- consumes(C, B, R, K), !has_port(B, R, K).
```

- **`ungrounded_mechanism`, precisely:** a leaf box B that has at least one mechanism arrow with a `code=` binding (resolving or not), where no existing code element carries `@implements` for B, directly or through B's callee subtree. A leaf with no code-bound mechanism never triggers it. Its lack of grounding shows up in the contract-complete metric (§5.13) instead. The old §5.4 prose ("leaf box has no implements → `ungrounded_mechanism`") was wrong; [ingestion](ingestion.md) should cite this definition.
- `mech_kind(E, /component)` now satisfies `unbound_mechanism` (SR-14); before, only `/resource` did.
- `stale_code_ref` fires for an `@implements` binding accepted in `.idef0/trace.lock` whose element the fresh scan no longer finds (§10).
- **fips183:** every code here is `info`. FIPS mechanisms are any means of support; FIPS Fig. 4's "Design Engineer" is valid.

### 5.12 Requirements traceability (R-TRC) [Extension]

```mangle
# (normative)
traced(B) :- traces(_, B).
violation(/untraced_leaf, B)     :- leaf_box(B), !traced(B).
violation(/stale_trace, D)       :- traces(D, _), doc_stale(D).
violation(/trace_unknown_box, D) :- traces(D, B), !is_box(B).
```

**fips183:** `untraced_leaf` is `off`; the other two are `info`.

### 5.13 Contract-complete metric (FR-METRIC-01)

A leaf is **contract-complete** when it meets all six NORTHSTAR criteria, evaluated with **CI severities** regardless of the current context.

```mangle
# (normative) Metric stratum: depends negatively on violation/2; nothing below may read it.
finding(C, S)     :- violation(C, S).
finding(C, S)     :- lint(C, S).
ci_error(C)       :- sev_resolved(C, /ci, /error).                 # section 6
error_on(C, S)    :- finding(C, S), ci_error(C).
error_on_box(B) :- error_on(C, B), subject_kind(C, /box), is_box(B).
error_on_box(B) :- error_on(C, E), subject_kind(C, /edge), icom_code(E, B, _, _).
error_on_box(B) :- error_on(C, T), subject_kind(C, /edge), bundle(T, L), icom_code(L, B, _, _).
error_on_box(B) :- error_on(C, P), subject_kind(C, /port), icom(_, _, P, B).
error_on_box(B) :- error_on(C, P), subject_kind(C, /port), icom(_, _, B, P).
error_on_box(B) :- error_on(C, D), subject_kind(C, /diagram), box(D, B, _).
error_on_box(B) :- error_on(C, K), subject_kind(C, /concept), icom_code(E, B, _, _), arrow_concept(E, K).
error_on_box(B) :- error_on(C, X), subject_kind(C, /coderef), implements(X, B).
error_on_box(B) :- error_on(C, X), subject_kind(C, /coderef), consumes(X, B, _, _).
error_on_box(B) :- error_on(C, X), subject_kind(C, /docref), traces(X, B).
error_on_box(B) :- error_on(C, M), subject_kind(C, /model), model(M), leaf_box(B).

described_arrow(E)     :- arrow_concept(E, K), described_concept(K).
undescribed_icom(B)    :- icom_code(E, B, _, _), !described_arrow(E).
all_icoms_described(B) :- leaf_box(B), !undescribed_icom(B).
traced_current(B)      :- traces(D, B), !doc_stale(D).
unbalanced_ctx(B)      :- box(D, B, _), decomposes(P, D), imbalance(P, _, _, _).

contract_complete(B) :- leaf_box(B), described_box(B), all_icoms_described(B), traced_current(B),
                        grounded(B), !unbalanced_ctx(B), !error_on_box(B).
```

| NORTHSTAR criterion | Predicate |
|---|---|
| 1. name and description | `described_box(B)`. The name exists by construction; the verb-phrase lint is a warning and does not count. |
| 2. every ICOM carries a named, defined concept | `all_icoms_described(B)`: every arrow attached at B (any side) has a concept with a description |
| 3. balanced with its parent | `!unbalanced_ctx(B)`: B's diagram has no `imbalance/4` against its parent box |
| 4. traced to a current requirement | `traced_current(B)` |
| 5. grounded in existing code, directly or via a call | `grounded(B)`. A caller leaf is grounded when some box in its callee subtree is grounded (FR-CALL-10). |
| 6. zero error diagnostics | `!error_on_box(B)`: CI-error findings attributed to B by subject kind |

- **Attribution:** a finding counts against B when its subject is B itself, an edge or port attached to B, a trunk with a leaf at B, B's diagram, a concept on one of B's arrows, a coderef or docref bound to B, or the model.
- **Reporting:** the `Validator` computes `ratio = |contract_complete| / |leaf_box|` from the sorted query results, with no aggregation in Mangle. It reports the ratio in JSON (`metric.contract_complete`) and in SARIF `run.properties` (§8.4). With zero leaves the ratio is reported as `null`.
- **Workspace aggregate:** `idef0 validate --workspace-metric` sums numerators and denominators over all models in the workspace index, excluding models matched by `[metric] exclude` in `.idef0/config.toml`. These are the "active models". Supporting-signal data sources are specified in [PRD](../PRD.md).
- **fips183:** the metric is still computed. FIPS markers do not change it.

## 6. Severity and profile resolution

### 6.1 Levels and files

Levels: `/error`, `/warning`, `/info`, `/off`. `/off` suppresses the diagnostic (and the metric's attribution, since it is not `/error`).

- `rules/severity.mg` holds one `severity_default(Code, EditorLevel, CiLevel)` fact and one `subject_kind(Code, Kind)` fact for **every** R code and L code in the registry, plus the resolution rules below.
- `rules/profile_fips183.mg` holds one `fips183_severity(Code, Level)` fact for every code whose `fips183` column in §7 differs from its default.

```mangle
# (illustrative) excerpt of rules/severity.mg facts
severity_default(/no_control, /warning, /error).      subject_kind(/no_control, /box).
severity_default(/unbound_mechanism, /warning, /error). subject_kind(/unbound_mechanism, /edge).
severity_default(/lint/name_01, /error, /error).        subject_kind(/lint/name_01, /box).
# (illustrative) excerpt of rules/profile_fips183.mg
fips183_severity(/unbound_mechanism, /info).
fips183_severity(/missing_a0_decomposition, /error).
```

```mangle
# (normative) resolution rules in rules/severity.mg
sev_base(C, /editor, L) :- severity_default(C, L, _).
sev_base(C, /ci, L)     :- severity_default(C, _, L).
fips_overridden(C)      :- profile(/fips183), fips183_severity(C, _).
sev_profiled(C, X, L)   :- sev_base(C, X, L), !fips_overridden(C).
sev_profiled(C, X, L)   :- sev_base(C, X, _), profile(/fips183), fips183_severity(C, L).
user_overridden(C, X)   :- severity_override(C, X, _).
sev_resolved(C, X, L)   :- sev_profiled(C, X, L), !user_overridden(C, X).
sev_resolved(C, X, L)   :- severity_override(C, X, L).
severity(C, L)          :- eval_context(X), sev_resolved(C, X, L).
```

### 6.2 Requirements

| ID | Requirement |
|---|---|
| FR-VAL-02 | The set of codes in `violation/2` heads of `idef0.mg`, the set of R codes in `severity_default/3`, the set in `subject_kind/2`, and the R rows of §7 are equal. A unit test asserts this, and also parses the §7 table from this file. |
| FR-VAL-03 | Every L code in §7 has `severity_default/3` and `subject_kind/2` facts. The Rust lint layer reads its levels from the resolved `severity/2`, so there is one source of severity truth. If the engine fails, it falls back to a `const` table generated from `severity.mg` at build time. |
| FR-VAL-04 | `fips183_severity/2` may name only codes marked [Extension] or [Deviation] in §7, except the two FIPS escalations (`missing_a0_decomposition`, `call_interface_undocumented`). A test asserts this. |
| FR-VAL-05 | `.idef0/config.toml` `[severity]` overrides become `severity_override/3`. Unknown codes, unknown levels, overrides of `E-*`/`G-*` codes, and (under `fips183`) any override that lowers a FIPS-normative code below its default are rejected with `E-CONFIG-01`. |
| FR-VAL-06 | `G-*` findings are always `error` and are not overridable. `E-*` findings are always fatal. |

## 7. Diagnostic registry

This is the single registry for all four namespaces (SR-57). Columns: **Ed/CI** = default severity in the editor and CI contexts (e = error, w = warning, i = info). **fips183** = level under the `fips183` profile ("=" means unchanged). **Fact** = whether the code reaches Mangle (`violation` or `lint`). **Fixture**: `std` = `fixtures/invalid/<name>/` for R codes and `fixtures/lint/<code>/` for L codes; `f183` = the fixture runs under `fips183`; `owner` = the owning document defines it.

### 7.1 R codes (Mangle; owner crate `idef0-mangle`; Fact = yes)

| Code | Mangle name | Subject | Ed/CI | fips183 | Source | Ext? | Fixture |
|---|---|---|---|---|---|---|---|
| R-BOX-01 | `box_count_low` | diagram | w/e | = (min 3) | FIPS §3.3.3-4; D8 | — | std |
| R-BOX-02 | `empty_diagram` | diagram | w/e | = | FIPS §3.3.3-4 | — | std |
| R-BOX-03 | `context_box_count` | diagram | e/e | = | FIPS §3.3.3-2 | — | std |
| R-BOX-04 | `box_count_high` | diagram | e/e | = (max 6) | FIPS §3.3.3-4; D8 [Deviation] 7 in extended | — | std, f183 |
| R-CTX-01 | `missing_context` | model | e/e | = | FIPS §3.3.3-2 | — | std |
| R-CTX-02 | `context_tunnel` | edge | e/e | = | FIPS §3.4.2 | — | std |
| R-CTX-03 | `missing_a0_decomposition` | model | w/w | e | FIPS §3.3.4.2 | — | std, f183 |
| R-CTL-01 | `no_control` | box | w/e | = | FIPS §3.3.3-8 | — | std |
| R-OUT-01 | `no_output` | box | w/e | = | FIPS §3.3.3-8 | — | std |
| R-LOCK-01 | `control_lock` | box | e/e | i | idef0-rs liveness | [Ext] | std |
| R-BAL-01 | `icom_imbalance` | box | w/e | = | FIPS §3.3.2.6–3.3.2.9 | — | std |
| R-BAL-02 | `balance_concept_mismatch` | box | w/w | i | FR-NAME-04 | [Ext] | std |
| R-BUN-02 | `mixed_bundle_relation` | edge | e/e | i | FIPS §3.3.2.5 refinement | [Ext] | std |
| R-BUN-03 | `bundle_concept_mismatch` | edge | e/e | i | FR-NAME-04 | [Ext] | std |
| R-NAME-01 | `undescribed_function` | box | w/w | i | FR-NAME-01, D9 | [Ext] | std |
| R-NAME-02 | `undescribed_concept` | concept | w/w | i | FR-NAME-02, D9 | [Ext] | std |
| R-NAME-03 | `undescribed_diagram` | diagram | w/w | i | FR-NAME-05 | [Ext] | std |
| R-NAME-04 | `unknown_concept` | edge | e/e | i | FR-NAME-03 | [Ext] | std |
| R-NAME-05 | `conceptless_arrow` | edge | w/e | = | FIPS §3.2.2.3-3; FR-NAME-03 | — | std |
| R-NAME-06 | `undescribed_box_on_release` | box | e/e | i | FR-NAME-06 | [Ext] | std |
| R-NAME-07 | `undescribed_concept_on_release` | concept | e/e | i | FR-NAME-06 | [Ext] | std |
| R-NAME-08 | `concept_cycle` | concept | e/e | i | FR-NAME-02 | [Ext] | std |
| R-CALL-01 | `call_and_decomposed` | box | e/e | = | FIPS §3.3.2.10; FR-CALL-02 | — | std |
| R-CALL-02 | `call_unresolved` | box | e/e | = | FIPS §3.3.2.10, §3.3.4.3; FR-CALL-07 | — | std |
| R-CALL-03 | `call_alternatives_without_note` | box | e/e | = | FIPS §3.3.2.10; FR-CALL-05 | — | std |
| R-CALL-04 | `call_recursive` | box | e/e | i | FR-CALL-08 | [Ext] | std |
| R-CALL-05 | `call_interface_mismatch` | box | w/w | i | FR-CALL-06 (concept form) | [Ext] | std |
| R-CALL-06 | `call_interface_undocumented` | box | i/i | e | FIPS §3.3.2.10; FR-CALL-06 | [Deviation] in extended | f183 |
| R-CALL-07 | `call_map_unknown_port` | box | e/e | i | FR-CALL-06 (CallPortMap) | [Ext] | std |
| R-UNIQ-01 | `duplicate_arrow_concept` | diagram | w/w | = | FIPS §3.3.3-14; Annex A.2 (inf.) | — | std |
| R-MECH-01 | `unbound_mechanism` | edge | w/e | i | FR-CALL-10; mechanisms map to code | [Ext] | std |
| R-MECH-02 | `ungrounded_mechanism` | box | w/e | i | idem | [Ext] | std |
| R-MECH-03 | `stale_mech_binding` | edge | w/e | i | idem | [Ext] | std |
| R-MECH-04 | `stale_code_ref` | coderef | w/e | i | idem | [Ext] | std (+lock) |
| R-MECH-05 | `implements_unknown_box` | coderef | w/e | i | idem | [Ext] | std |
| R-MECH-06 | `implements_non_leaf` | coderef | w/w | i | idem | [Ext] | std |
| R-MECH-07 | `consumes_unknown_port` | coderef | w/e | i | idem | [Ext] | std |
| R-TRC-01 | `untraced_leaf` | box | i/i | off | traceability | [Ext] | std |
| R-TRC-02 | `stale_trace` | docref | w/w | i | traceability | [Ext] | std (+lock) |
| R-TRC-03 | `trace_unknown_box` | docref | w/w | i | traceability | [Ext] | std |
| R-CTX-04 | `higher_context_mismatch` | diagram | e/e | = | FIPS §3.4.2 (A-1 top box carries A-0's ICOMs) | — | std (rule written at M3.3; identity of the A0 placeholder on A-1 per metamodel §3.2) |

- **Editor leniency:** `box_count_low`, `empty_diagram`, `no_control`, `no_output`, `icom_imbalance` and `conceptless_arrow` are warnings in the editor because every diagram passes through these states while being built. CI applies the FIPS level.
- **Removed (SR-15):** `dangling_source`, `dangling_target`, `unlinked_boundary`, `bundle_cycle` (former R-BUN-01), `fork_source_mismatch` and `join_target_mismatch` moved to extractor invariants (§3.3). `unlinked_boundary` cannot occur: boundary ports exist only while an arrow uses them ([metamodel](metamodel.md)). A parent ICOM with no child arrow is reported as `icom_imbalance` (`missing_in_child`), which covers the "missing boundary linkage" requirement. `undescribed_on_release` was split into R-NAME-06/07.
- **Metric:** FR-METRIC-01 `contract_complete/1` is not a diagnostic; it has no registry row.

### 7.2 L codes (Rust lints)

| Code | Mangle name | Subject | Ed/CI | fips183 | Source | Owner | Fact | Fixture |
|---|---|---|---|---|---|---|---|---|
| L-META-01 | `/lint/meta_01` | model | e/e | = | FIPS §3.3.1.1 (purpose, viewpoint) | core / metamodel | yes | std |
| L-META-02 | `/lint/meta_02` | model | w/w | i | scope recommended (FR-META-04, [Extension]) | core / metamodel | yes | std |
| L-META-03 | `/lint/meta_03` | model | w/w | i | model description (FR-NAME-05) | core / metamodel | yes | std |
| L-FMT-01 | `/lint/fmt_01` | model | w/w | = | missing format version header | core / metamodel | no | std |
| L-TITLE-01 | `/lint/title_01` | box | w/w | = | FIPS §3.2.2.3-1 (heuristic) | core / metamodel | yes | std |
| L-NAME-01 | `/lint/name_01` | box | e/e | = | FR-NAME-01; Annex A.2 (inf.) | core / metamodel | yes | std |
| L-NAME-02 | `/lint/name_02` | box | w/w | = | FR-NAME-01 | core / metamodel | yes | std |
| L-NAME-03 | `/lint/name_03` | box | e/e | = | FIPS §3.2.2.3-5, §3.3.3-15 | core / validation | yes | std |
| L-NAME-04 | `/lint/name_04` | box | w/w | = | Annex B.2.4.2-3 (inf.) | core / validation | yes | std |
| L-CONCEPT-01 | `/lint/concept_01` | concept | e/e | = | FR-NAME-02 | core / metamodel | yes | std |
| L-CONCEPT-02 | `/lint/concept_02` | concept | i/i | off | unused concept | core / metamodel | yes | std |
| L-DESC-02 | `/lint/desc_02` | box | i/i | = | FR-NAME-07; FIPS §3.3.1.4 | core / validation | yes | std |
| L-DESC-03 | `/lint/desc_03` | concept | i/i | = | FR-NAME-07 (concept descriptions) | core / metamodel | yes | std |
| L-DESC-04 | `/lint/desc_04` | diagram | i/i | = | FR-NAME-07; FIPS §3.3.1.4 (diagram text) | core / metamodel | yes | std |
| L-DESC-05 | `/lint/desc_05` | model | i/i | = | FR-NAME-07 (model description) | core / metamodel | yes | std |
| L-ROLE-01 | `/lint/role_01` | edge | i/i | = | FIPS §3.3.2.8, Fig. 15 (legal) | core / metamodel | no | std |
| L-BUN-01 | `/lint/bun_01` | edge | w/w | off | FR-NAME-04 | core / metamodel | yes | std |
| L-SPLIT-CI | `/lint/split_ci` | box | w/w | = | Annex B.2.4.1-3 (inf.) | core / metamodel | no | std |
| L-REUSE-01 | `/lint/reuse_01` | box | i/i | off | FR-CALL-11 | core / metamodel | no | std |
| L-NOTE-01 | `/lint/note_01` | diagram | e/e | = | FIPS §3.3.4.4 (consecutive from 1) | core / validation | yes | std |
| L-CNUM-01 | `/lint/cnum_01` | diagram | e/e | = | FIPS §3.3.1.3 (C-number unique) | core / validation | yes | std |
| L-GLOSS-01 | `/lint/gloss_01` | box | w/w | = | FIPS §3.2.2.1 (acronyms in glossary) | core / validation | yes | std |
| L-BUNDLE-SUGGEST | `/lint/bundle_suggest` | diagram | i/i | = | FR-LAYOUT-15; B.2.3.2-1 (inf.) | layout | no | owner |
| L-LAYOUT-OVERFLOW | `/lint/layout_overflow` | diagram | w/w | = | layout Fit step | layout | no | owner |
| L-LABEL-FIT | `/lint/label_fit` | edge | w/w | = | FR-STYLE-01 | layout | no | owner |
| L-SIG-DRIFT | `/lint/sig_drift` | coderef | w/w | i | signature drift | ingest / ingestion | no | owner |
| L-PRD-01 | `/lint/prd_01` | docref | w/w | i | malformed or empty PRD anchor | ingest / ingestion | no | owner |
| L-PRD-02 | `/lint/prd_02` | docref | w/w | i | fenced `idef0` block does not parse | ingest / ingestion | no | owner |
| L-PRD-03 | `/lint/prd_03` | docref | w/w | i | anchor model cannot be resolved | ingest / ingestion | no | owner |
| L-TAG-01 | `/lint/tag_01` | coderef | w/w | i | malformed code tag | ingest / ingestion | no | owner |
| L-TAG-02 | `/lint/tag_02` | coderef | w/w | i | ambiguous model in tag | ingest / ingestion | no | owner |
| L-TAG-03 | `/lint/tag_03` | coderef | w/e | i | node-number tag silently rebound to a different box | ingest / ingestion | no | owner |
| L-INJECT-01 | `/lint/inject_01` | box | i/i | i | instruction-like text in a box description | ingest / agents | no | owner |
| L-INJECT-02 | `/lint/inject_02` | concept | i/i | i | instruction-like text in a concept description | ingest / agents | no | owner |
| L-INJECT-03 | `/lint/inject_03` | diagram | i/i | i | instruction-like text in a diagram description | ingest / agents | no | owner |
| L-SCALE-01 | `/lint/scale_01` | model | w/w | = | model soft scale limit exceeded | core / architecture | no | owner |
| L-SCALE-02 | `/lint/scale_02` | diagram | w/w | = | arrows-per-diagram soft limit (60) exceeded | core / architecture | no | owner |

An L code whose fips183 column is not `=` is an idef0-rs [Extension] (FR-VAL-04).

- **L-NAME-03 word lists (SR-08):** box names must not consist solely of *function, activity, process, input, output, control, mechanism* (FIPS §3.3.3-15). Arrow labels must not consist solely of those words or *call* (FIPS §3.2.2.3-5). *call* is reserved for labels only. The arrow-label check runs on the concept name and is reported on the box the arrow attaches to (the destination box, or the source box for arrows ending at the boundary).
- **L-NAME-04 (new):** a name or label *contains* one of B.2.4.2-3's keywords (*data, function, input, output, control, mechanism*) as a whole word, and L-NAME-03 did not already fire. Example: "Order Data".
- **L-DESC-02 (extended):** flags a description that restates the name, is under 20 characters, lacks a genus (concepts), or merely lists the box's ICOM labels ("transforms ‹inputs› into ‹outputs›"), which FIPS §3.3.1.4 calls redundant. The FR-NAME-01 draft template must not produce text that trips this lint ([metamodel](metamodel.md)).
- **L-NOTE-01 (new):** the model-note numbers on a diagram are not exactly `1..n`.
- **L-CNUM-01 (new):** two diagrams in the model carry the same C-number (version history entries `AK15(AK14)` excluded).
- **L-GLOSS-01 (new; extended to labels):** a box name or an attached arrow label contains an all-caps token of 2 or more letters that is not a concept key, name or synonym. Label findings are reported on the attached box, as for L-NAME-03 ([metamodel §18](metamodel.md#18-codes-and-open-items)). Acronyms inside concept names are covered by `undescribed_concept`.
- Owners: "core / metamodel" means the lint is implemented in `idef0-core` and specified in [metamodel](metamodel.md). "core / validation" means it is specified here.

### 7.3 E codes (fatal)

| Code | Subject | Source | Owner | Fixture |
|---|---|---|---|---|
| `E-PARSE-01`, `E-PARSE-02` | model (span) | DSL syntax errors | core / metamodel | owner |
| `E-VERSION-01`, `E-VERSION-02` | model | unsupported or missing format version | core / metamodel | owner |
| `E-ID-01`..`E-ID-06` | element | abbrev, box key, concept key, arrow key, malformed key, duplicate abbrev in workspace | core / metamodel | owner |
| `E-NODE-01`..`E-NODE-03` | diagram | Invalid node digit; bad box numbering; diagram without a valid parent box | core / metamodel | owner |
| `E-TYPE-01`..`E-TYPE-05` | edge | arrow source, destination, boundary pass-through, rank gap or duplicate, ambiguous `tunnel=box` | core / metamodel | owner |
| `E-CALL-01`, `E-CALL-02` | box | Call on a nonexistent box number; call on the top box | core / metamodel | owner |
| `E-NOTE-01` | diagram | Note target not on this diagram | core / metamodel | owner |
| `E-TITLE-MATCH` | diagram | FIPS §2.48 (child title = parent box name); Annex C.4.4 (inf.) | core / metamodel | owner |
| `E-LIMIT-01` | model | parser input limit exceeded | core / architecture | owner |
| `E-METRICS` | model | invalid `Metrics` override | layout | owner |
| `E-CONFIG-01` | model | invalid severity override (FR-VAL-05) | mangle / validation | `fixtures/config/` |
| `E-RULES-01` | model | project rule parse, safety or stratification error; engine error | mangle / validation | `fixtures/rules/` |
| `E-RULES-02` | model | project rule uses a forbidden construct (§9) | mangle / validation | `fixtures/rules/` |
| `E-RULES-03` | model | project rule evaluation limit exceeded (§9) | mangle / validation | `fixtures/rules/` |

### 7.4 G codes (layout geometry checker; owner [layout](layout.md), crate `idef0-layout`)

`G-ORTHO`, `G-ARC`, `G-PERIM`, `G-CORNER`, `G-SIDE`, `G-FEEDBACK`, `G-DIAG`, `G-CLEAR`, `G-ICOM-ORDER`, `G-LABEL`, `G-BOX`, `G-ICOM`, `G-LATTICE`, `G-JUNCTION`, `G-BRIDGE`, `G-ASCII`. All are `error` in every context and profile, and none is overridable (FR-VAL-06). Any `G-*` finding is a layout-engine bug. `G-LABEL` must honor the FIPS §3.2.2.3-3 whole-arrow exemption (SR-05). Subjects and fixtures are defined in [layout](layout.md).

## 8. Diagnostics output

### 8.1 Diagnostic and Report

```rust
// (normative) idef0-mangle::report
pub struct Diagnostic {
    pub code: Code,                 // "icom_imbalance", "L-NAME-01", "G-ARC", "E-TYPE-01"
    pub layer: Layer,               // E | L | R | G
    pub severity: Severity,         // Error | Warning | Info   (Off never reaches a Report)
    pub subject: SubjectRef,        // kind from the registry + fact id; never inferred from the id
    pub message: String,
    pub detail: Vec<DetailRow>,     // e.g. imbalance (Role, K, missing_in_child), dup_concept rows
    pub span: Option<Span>,         // from SpanMap (core) or the scanner (coderef/docref)
    pub fix: Option<FixHint>,
}
pub struct Report { pub schema_version: u32, pub profile: Profile, pub context: Context,
                    pub diagnostics: Vec<Diagnostic>, pub metric: Metric, pub facts_hash: String }
```

- **Sort order (deterministic):** severity (error first), then the subject's canonical model order (diagrams in node order, then boxes, edges, ports, concepts by key, coderefs, docrefs, model), then code, then subject id, then detail.
- **Spans (SR-53):** `idef0-core::dsl::parse` returns a `SpanMap: BTreeMap<ElementKey, Span>` alongside the `Model`. It is not part of model equality, and the [metamodel](metamodel.md) lossless CST provides it. `Span = {file, start_line, start_col, end_line, end_col}`, 1-based, in UTF-8 columns. Model-level subjects use the `model` declaration line.

### 8.2 Human

`path:line:col: <severity>[<code>] <subject-kind> <subject>: <message>`, followed by indented detail rows. Under `fips183`, the line is prefixed with the FIPS clause where one exists. Colors only on a TTY.

### 8.3 JSON

The `Report` struct serialized with serde, keys in declaration order, and `schema_version` starting at 1. The `metric` object is `{"contract_complete": n, "leaves": m, "ratio": "0.8125" | null}`. The ratio is a decimal string with exactly four places, so the bytes are deterministic. Its schema id is `idef0.report.v1`, and its JSON Schema ships at `crates/idef0-mangle/schema/report.v1.json`.

### 8.4 SARIF 2.1.0

| SARIF field | Value |
|---|---|
| `runs[0].tool.driver.rules[]` | One entry per registry code: `id` = code (R uses the Mangle name), `name`, `shortDescription`, `helpUri` = this document's §7 anchor, `defaultConfiguration.level`, `properties: {layer, rId, fipsRef, extension}` |
| `results[].ruleId` / `level` | Code; error→`error`, warning→`warning`, info→`note` |
| `results[].locations[0].physicalLocation` | `artifactLocation.uri` (repo-relative), `region` from the span |
| `results[].locations[0].logicalLocations[0]` | `fullyQualifiedName` = `FAC/A21` (box), `FAC/A21/I1` (port), etc., and `kind` = subject kind |
| `results[].partialFingerprints` | `{"idef0ElementKey/v1": blake3_hex(code ‖ 0x00 ‖ abbrev ‖ 0x00 ‖ element_key ‖ 0x00 ‖ detail_key)}` |
| `runs[0].properties` | `{"idef0.profile", "idef0.context", "idef0.contractComplete": {complete, leaves, ratio}, "idef0.factsHash"}` |
| `invocations[0]` | `executionSuccessful`; no timestamps unless `--sarif-timestamps` |

- **Element keys survive renumbering.** box → `box:<key>`; diagram → `diag:<parent box key>` (`diag:A-0` for A-0); edge → `edge:<explicit key>`, or else `edge:<diag parent key>:<src box key>.<side>-><dst box key>.<side>:<concept>`; port → `port:<diag parent key>/<code>`; concept → `concept:<key>`; coderef/docref → its id; model → `model:<abbrev>`. Node numbers and structural arrow ids never enter a fingerprint, so GitHub code-scanning alerts persist across renumbering and Auto-Arrange.
- The output validates against the OASIS SARIF 2.1.0 JSON schema (M3.4 acceptance).

### 8.5 Mangle text

`idef0 facts <model>` dumps the sorted EDB as Mangle source, loadable by any Mangle interpreter together with the rule files.

## 9. Project rules

Every `.idef0/rules/*.mg` file (in filename order) is appended to the built-in program. It may derive additional `violation/2` facts for organization-specific methodology. Example: every box under A3 must have a control carrying a security-policy concept.

| ID | Requirement |
|---|---|
| FR-VAL-07 | A project rule file may define only (a) `violation/2` rules for codes it registers, (b) its own helper predicates, and (c) `severity_default/3` and `subject_kind/2` facts for its own codes. Every project code needs both facts. |
| FR-VAL-08 | Forbidden, and rejected at load with `E-RULES-02`: negating `violation/2`; referencing `finding`, `error_on`, `error_on_box`, `contract_complete` or `ci_error`; defining any predicate declared in `schema.mg` or defined in a built-in file (so no injected `code_exists`, `implements` or `severity` facts); reusing a built-in code; constructs outside the MiniEngine subset. |
| FR-VAL-09 | Project rules must be safe and stratified together with the built-in program, or `E-RULES-01` is raised. Negating `violation/2` is forbidden because project rules also derive `violation/2`, which would make the program unstratifiable. |
| FR-VAL-10 | Evaluation is bounded deterministically: at most 2,000,000 derived tuples (default) and at most 10,000 fixpoint iterations per stratum. Both limits are configurable in `[rules]` in `.idef0/config`. |
| FR-VAL-11 | Safety nets: a wall-clock deadline (default 2 s native, 500 ms wasm) and a memory cap (default 256 MiB native; enforced in wasm through the tuple budget). Exceeding any limit aborts with `E-RULES-03`; no partial results are kept. |
| FR-VAL-12 | After `E-RULES-03` or `E-RULES-01` in a project file, the Validator re-evaluates the built-in program alone and reports its diagnostics plus the `E-RULES-*` error. CI therefore fails closed. |
| FR-VAL-13 | Project rules from an untrusted checkout are data: they cannot perform I/O (Mangle has none), and all limits apply identically in CI, the server and the wasm client. |

- The deterministic tuple and iteration budgets are the primary limits because wall-clock limits are host-dependent and would break D6 at the margin. The deadline only catches pathological engine behavior.
- Threat-model context (malicious PRs in CI) is in [agents](agents.md#7-threat-model-and-privacy) and [architecture](../architecture.md).

## 10. External facts

### 10.1 Structure

```rust
// (normative) idef0-mangle::facts
pub struct ExternalFacts {
    pub schema_version: u32,                 // bumps on any change below; mismatch ⇒ rejected
    pub source: FactSource,                  // FreshScan { trace_lock: Option<Blake3> } | Index { index_schema: u32 } | None
    pub code: Vec<CodeFact>,                 // code_ref, code_exists, implements, consumes
    pub docs: Vec<DocFact>,                  // doc_ref, traces, requirement, doc_stale
    pub workspace: Option<WorkspaceFacts>,   // present ⇒ index_present(/workspace)
}
pub struct WorkspaceFacts {
    pub models: BTreeSet<Abbrev>,                          // ext_model/1
    pub boxes: BTreeSet<(Abbrev, NodeId)>,                 // ext_box/2
    pub keys: BTreeMap<(Abbrev, BoxKey), NodeId>,          // used by FR-FACT-19 only; not emitted
    pub grounded: BTreeSet<(Abbrev, NodeId)>,              // ext_grounded/2
}
```

All collections are ordered (D6). Facts are filtered to the model under evaluation (FR-FACT-25) before extraction.

### 10.2 Producers

| Producer | Context | Contents |
|---|---|---|
| `idef0 validate` (in-process) | CI, local | Fresh scan of the working tree (code tags, PRD sections), the committed `.idef0/trace.lock`, and fresh parses of every workspace `.idef0` file |
| Server workspace indexer | editor (served) | The same, maintained incrementally by the file watcher. Pushed to the client via `GET /api/models/{id}/facts`. |
| `idef0 scan` | offline client | Writes `.idef0/index.json`, which serializes `ExternalFacts` with `index_schema` |

### 10.3 Freshness

| ID | Requirement |
|---|---|
| FR-VAL-14 | **CI is deterministic.** `idef0 validate` in the CI context uses only the committed `.idef0/trace.lock` plus a fresh scan of the checked-out tree. It never reads facts from `.idef0/cache/` or `.idef0/index.json`. The scan cache may only skip re-parsing a file whose blake3 matches; it never supplies facts for an unhashed file. |
| FR-VAL-15 | `doc_stale(D)` is emitted when the fresh section hash differs from the hash accepted in `trace.lock`. `L-SIG-DRIFT` compares the fresh `sig_hash` the same way. An `implements` binding recorded in `trace.lock` and not found by the scan is emitted as `implements(C, B)` without `code_exists(C)`, so `stale_code_ref` fires. |
| FR-VAL-16 | `ext_box`, `ext_model` and `ext_grounded` come from parsing the other workspace models at the same checkout, never from a cached index, in CI. `ext_grounded(M, N)` holds when some box in N's subtree in M has `@implements` code that the fresh scan finds. |
| FR-VAL-17 | The offline client loads `.idef0/index.json` only if its `index_schema` equals the supported version. Otherwise it validates with `ExternalFacts { source: None, workspace: None }` and shows a notice. Diagnostics computed from an index are labeled with the index's recorded commit. |
| FR-VAL-18 | Without external facts, traceability rules see no `implements`/`traces` facts. In the editor this yields warnings at most; `untraced_leaf` is info. |

The format and location of `trace.lock`, the scanner, and the sync and ack workflow are owned by [ingestion](ingestion.md). The `.idef0/` layout and versioning are owned by [metamodel](metamodel.md).

## 11. Engine backends and the M3.1 spike gate

### 11.1 RuleEngine

```rust
// (illustrative)
pub trait RuleEngine {
    fn load_program(&mut self, sources: &[RuleSource], limits: &EvalLimits) -> Result<(), RuleError>;
    fn evaluate(&mut self, facts: &FactSet) -> Result<Derived, EvalError>;
    fn query(&self, derived: &Derived, pred: &str) -> Vec<Tuple>;     // sorted
}
```

The backends are `MangleRsEngine` (feature `mangle-rs`, the default if the gate passes) and `MiniEngine` (feature `mini`). The rule files are embedded with `include_str!`. The conformance suite (§12) runs on every enabled backend.

### 11.2 Spike (M3.1)

M3.1 is time-boxed to one or two sessions. It pins `mangle-rs` by git revision and verifies five things:

1. `mangle-parse` and `mangle-analysis` accept `schema.mg`, `idef0.mg`, `severity.mg`, `profile_fips183.mg` and `invariants.mg` unchanged.
2. `mangle-interpreter` evaluates stratified negation, name constants such as `/lint/name_01`, and `|> do fn:group_by(…), let N = fn:count()`.
3. It builds for `wasm32-unknown-unknown`, directly or through the `mangle-wasm` crate API.
4. Results are identical native vs wasm on all fixtures.
5. **New: wasm validation latency is measured.** Measure full recompute of structural validation (no external facts) in headless Chromium for the dogfood model and for the generated 60-diagram model `fixtures/large/` ([testing](../testing.md#3-random-valid-model-generator) model generator; 60 is the single budgeted size, [architecture §10](../architecture.md#10-scale-limits-sr-33-nfr-scale-01)). Record p50/p95 for both backends, plus the ExternalFacts case. The 16 ms budget (NFR-PERF-02) for 60 diagrams with full recompute is unproven until then.

### 11.3 Gate

- **Backend.** If check 1, 2, 3 or 4 fails and cannot be patched upstream within the spike, implement `MiniEngine`: a semi-naive bottom-up evaluator for exactly the subset used here (positive and negated atoms, stratification, `!=`, `:lt/2`, name and string constants, `group_by` with `count`), at about 1,000 lines. It parses the same `.mg` files, with `mangle-parse` if that crate works standalone and a `winnow` subset parser otherwise. The rule files stay Mangle-valid, so users can always run them under the Go `mg` interpreter. Rules must not use features outside the subset without updating both backends.
- **Latency.** If check 5 shows p95 above 16 ms:
  - **Default fallback:** evaluation moves to a Web Worker (`gloo-worker`) with the same API. Diagnostics then arrive asynchronously, debounced at 50 ms, tagged with the model revision, and stale results are discarded.
  - **Option, per-diagram scoping:** re-evaluate only the facts of the edited diagram, its parent box and its child diagrams. This is valid for every rule except call expansion, grounding and the metric, which run on idle.
  - **Option, incremental maintenance:** delta evaluation in `MiniEngine`. Out of v1 unless the Worker plus scoping still misses the budget.
- The decision and the measurements are recorded in `docs/adr/0001-mangle-backend.md`.

## 12. Conformance fixtures

| ID | Requirement |
|---|---|
| FR-VAL-19 | `fixtures/valid/*.idef0`, each with an optional `<name>.ext.json` (ExternalFacts), yields no CI-error findings under its declared profile. The dogfood and reference fixtures are listed in [testing](../testing.md). |
| FR-VAL-20 | Each `fixtures/invalid/<code>/<case>.idef0` (optional `.ext.json`, optional `trace.lock`, optional `expect.toml`) **contains** at least one `<code>` finding, and **no other** CI-error finding outside the per-fixture `allow = [...]` list in `expect.toml`. Each allowlist entry carries a one-line justification. There is no global allowlist. |
| FR-VAL-21 | `expect.toml` may pin `subject` and `profile`. Codes whose behavior depends on the profile (`box_count_high`, `missing_a0_decomposition`, `call_interface_undocumented`) have a fixture per profile. |
| FR-VAL-22 | Coverage test: every R code in §7.1 has at least one invalid fixture, and every core L code in §7.2 has a `fixtures/lint/<code>/` fixture. |
| FR-VAL-23 | Extractor invariants (§3.3) have fact-level fixtures, `fixtures/facts/<inv>/<case>.mg`: hand-written EDB that must derive exactly that `extract_invariant` code. They run against `invariants.mg` on every backend. |
| FR-VAL-24 | Extractor golden test (SR-11): an `insta` snapshot of `idef0 facts` for the dogfood model, plus targeted fixtures for FR-FACT-12, -13, -15, -16, -17, -19, -20 and -24 (forks, joins, unlabeled `rel=all` leaves, trunk tunnels and mechanisms, border tunnels, key-based and cross-model calls, untagged `code=` targets). |
| FR-VAL-25 | Every fixture runs on every enabled backend, natively and in wasm (`wasm-bindgen-test`), and the `Report` JSON must be byte-identical (M3.5). |

Decisions per code (SR-15):

- `dangling_source`, `dangling_target`, `unlinked_boundary`: unreachable from the DSL, because the core type check rejects unknown endpoints and boundary ports only exist as arrow endpoints. They are removed from the registry and kept as invariants with `.mg` fixtures.
- `bundle_cycle`, `fork_source_mismatch`, `join_target_mismatch`: true by construction after flattening single-level bundles. Same treatment.
- Former co-firing: `undescribed_function` vs `undescribed_box_on_release` (and the concept pair) are now mutually exclusive (§5.7). `untraced_leaf` is info, so it never violates FR-VAL-20.

## 13. Open questions

1. **A-n box ids.** *Resolved* by [metamodel §3.2](metamodel.md#32-node-numbers-and-box-numbers).
2. **Box-to-box tunnel syntax.** *Resolved* by [metamodel §16.2](metamodel.md#162-grammar): `tunnel=src|dst|both`.
3. **Arrow-label reserved words.** Should L-NAME-03/04 findings on arrow labels get a concept subject (one code per subject kind) or stay attributed to the destination box, as specified here?
4. **`call_interface_undocumented` under `extended`.** It is `info` here, with concept matching substituting for the note. Alternatively it could be a `warning`.
5. **Default project-rule limits** (FR-VAL-10/11) need calibration in M3.4.
