# Spec Review: PRD, NORTHSTAR, README (2026-10-15)

**Scope:** `docs/PRD.md` (about 1,870 lines), `docs/NORTHSTAR.md`, `README.md`, `claude.md`.

**Method:** three independent reviews run in parallel:
1. Accuracy against FIPS 183, checked against the PDF text.
2. Internal consistency, plus a check of the Mangle rules.
3. Readability and completeness.

The author then spot-checked the major claims. ✔ marks a finding the author re-verified against the source (FIPS text, or the PRD text plus language and crate facts). Line numbers refer to PRD.md as of this review.

## Verdict

The **core design is sound**:
- metamodel, ICOM balance and tunneling semantics
- call arrows
- the layout approach
- the Mangle rules are safe and stratified, and their arities are consistent
- the dogfood model passes, given one extractor clarification
- the fillet and bridge math

The spec is **not yet ready to implement from** because of:
- **4 FIPS misstatements**
- a **determinism-breaking identity scheme**
- **5 real Mangle rule bugs**
- **key-binding conflicts**
- **undefined editing semantics** (delete, renumber)
- **no security model** for the localhost server or for agent and LLM paths

It is also hard to navigate: about 37k tokens in one file, codes and keys scattered, suffix-numbered sections. Fixing Tier 1 below is a prerequisite for writing the implementation plan.

Finding IDs (`SR-nn`) are stable, so you can say "fix SR-01..SR-12".

**Tags:** `[FIPS]` standard accuracy · `[MG]` Mangle rules · `[CONS]` internal consistency · `[GAP]` omission · `[ENG]` engineering claim · `[READ]` readability

---

## Tier 1: Must fix before implementation

### A. FIPS 183 misstatements

| ID | Finding | Fix |
|---|---|---|
| SR-01 ✔ | `[FIPS]` **A-1 and higher context diagrams are given exactly one box.** §1.5.3 L106, rule `context_box_count` L387, grammar `BOXNO "0" (A-0 only)`. FIPS §2.16 and §3.4.2–3.4.3 say A-1 has **3–6 boxes**, one of which is box 0; on A-n (n>1) one box is −(n−1). An A-1 gets a false error and cannot be expressed in the DSL. | Restrict `context_box_count` to `"A-0"`. Add diagram kind `/higher_context` with R-BOX limits. Widen `BOXNO`. Define box ids for A-n. |
| SR-02 ✔ | `[FIPS]` **The mandatory A0 decomposition is not checked.** FIPS §3.3.4.2: "Every IDEF0 model *shall* also have at least three, but no more than six, child boxes on the A0 child diagram." Appendix C expects "no structural errors" with only A-0. | Add `missing_a0_decomposition` (error under `fips183`, warning while `working`). Fix the Appendix C expectation. |
| SR-03 ✔ | `[FIPS]` **"Controls are not consumed" is wrong.** §1.5.5 table L128; also `claude.md`. FIPS §2.17: "Data or objects modeled as controls **may be transformed** by the function, creating output." | Drop the "Consumed?" column. Define Control as "conditions required to produce correct output; may be transformed (§2.17)". |
| SR-04 | `[FIPS]` **"`fips183` = strictly per the standard" is false as written.** D11, §1.7 header, NORTHSTAR, README. These extension rules reject FIPS-valid models and are not marked as extensions: `unbound_mechanism`, `ungrounded_mechanism`, `conceptless_arrow`, `unknown_concept`, `undescribed_on_release`, `control_lock`, concept-based call matching, `call_recursive`, node-number corner label, strict staircase. FIPS's own Fig. 4 mechanism "Design Engineer" would fail. | Tag each rule **[Extension]**. Under `fips183`, extensions are off or downgraded to info. Or weaken D11 to "no FIPS rule is relaxed; extensions may add diagnostics" and list them. |
| SR-05 ✔ | `[FIPS]` **Label exemption missing.** FIPS §3.2.2.3-3: segments must be labeled "unless a single arrow label clearly applies to the arrow as a whole". FR-LAYOUT-16, §1.7 row and `G-LABEL` omit it, although `rel=all` branches rely on it. | Add the exemption in all three places. |
| SR-06 ✔ | `[FIPS]` **DRE forms incomplete, and B.2.2.1-4 misread.** FIPS allows four forms: C-number, page, node number, model-note number. B.2.2.1-4 allows "node number **or** C-number" on working copies; it does not require the C-number. | List all four forms (v1 supports node number and C-number). Reword §1.7 L605. |
| SR-07 | `[FIPS]` **Annex guidance presented as normative.** FR-LAYOUT-01, 09, 10, 20, 21 and D5 ("FIPS-optimal") cite informative Annex B and C advice as Hard rules. FR-LAYOUT-21 also **reverses** B.2.4's order: FIPS says heavy constraint arrows first, then boundary arrows. | Add a Normative/Informative column to the FR-LAYOUT and §1.7 tables. Fix or re-attribute FR-LAYOUT-21. |
| SR-08 | `[FIPS]` **Smaller attribution errors:** <br>• FR-STYLE-07 (node number in corner) is an unmarked deviation from §3.3.3-5. <br>• Used At is not from §3.3.2.10; C.4.1.6 is broader. <br>• The node index is not "the required" TOC; it is required only *when* a TOC exists. <br>• `abbrev` is not FIPS-required ("normally abbreviated"). <br>• Dropping working fields for publication must copy Used At into a model note (C.4.1.6). <br>• `A0.k` must normalize to `Ak`, not `A0k`. <br>• `A21/I1` is an idef0-rs form; FIPS uses `A21I1`. <br>• `call` is reserved only for arrow labels, not box names. <br>• A-1 is "not high level" (§3.4.3). <br>• "FOG" is a typo for FEO. <br>• Model notes are optional, not "normative". <br>• Frame order by ICOM code is an idef0-rs invariant. <br>• The FR-NAME-01 auto-template invites restating ICOMs, which §3.3.1.4 forbids. | Correct each in place. See the FIPS review findings 7–34 (kept in session; summarized here). |
| SR-09 | `[FIPS]` **History claims unverifiable in-repo:** withdrawal 2008-09-02, IEEE 1320.1-1998, ISO/IEC/IEEE 31320-1. "IDEF0 was designed by ICAM" ignores its SADT origin. NORTHSTAR dates FIPS 183 to the 1970s–80s, but it is from 1993. | Cite sources (Federal Register notice, IEEE/ISO catalogue) or soften. Fix the timeline wording. |

### B. Mangle rule bugs

The rules pass the static checks: safety ✔, stratification ✔, arity ✔. These findings are semantic.

| ID | Finding | Fix |
|---|---|---|
| SR-10 ✔ | `[MG]` **Subject namespace collision.** Box `"A1"` and diagram `"A1"` are the same string, so `described("A1")` from the box masks a missing diagram description, and the reverse. `violation/2` subjects mix 7 kinds, so `SubjectRef` can't be recovered. | Split into `described_box/1`, `described_concept/1`, `described_diagram/1`. Use one subject kind per violation code, documented in the severity table. |
| SR-11 ✔ | `[MG]` **Fact-extraction contract under-specified for bundles.** Without it, the dogfood fails with 8 false errors. The rules require: <br>(a) `arrow_concept(leaf, trunkConcept)` for unlabeled `rel=all` branches <br>(b) `arrow_concept` and `edge_in` for trunks <br>(c) `edge_in` for every leaf <br>(d) trunk-level `tunnel` / `mech_*` attributes pushed to the leaf at the shared end <br>(e) join leaves share the destination rank <br>(f) local `call` facts carry the model's own abbrev | Add these as numbered extractor obligations in §1.6.1. Add an extractor golden test. |
| SR-12 | `[MG]` **`balance_concept_mismatch` false positives with forks.** A forked parent output's leaves carry the *branch* concepts. | Compare the concept at the attached end (the trunk concept at the fork source or join target). The reviewer's corrected rules apply. |
| SR-13 | `[MG]` **`call_interface_mismatch` ignores roles, and the output direction is backwards.** For outputs the callee's concept must refine the caller's. Mechanisms shouldn't be checked. | Split the input/control check from the output check. The reviewer's corrected rules apply. State that cross-model calls are not interface-checked in v1. |
| SR-14 | `[MG]` **Smaller rule defects:** <br>• `/component` mechanisms can never be satisfied (`mech_ok` exempts only `/resource`). <br>• `control_lock` fires without a cycle and depends on `violation/2`, so a project rule that negates `violation` breaks stratification. <br>• Grounding through calls doesn't follow call chains. <br>• `lint/2` and `requirement/2` are used but undeclared. <br>• `[ ]` tunnels have two representations (`tunnel/at_border` and `tunneled_boundary`) with no linking invariant, and the DSL can't express an uncoded border port. <br>• `duplicate_arrow_concept` fires twice per pair. <br>• §5.4 prose for `ungrounded_mechanism` contradicts the rule. <br>• `code=` targets are only "existing" if tagged. | Corrected rules are in the consistency review (B5–B14). Add: "project rules must not negate `violation/2`." |
| SR-15 | `[MG]` **M3.3 acceptance cannot be met.** "Each `invalid/<code>` fixture yields exactly that violation" fails because: <br>• some codes always co-fire (`undescribed_on_release` with `undescribed_*`; `untraced_leaf` everywhere) <br>• some are unreachable from the DSL (`dangling_*` is rejected earlier by the type check; `bundle_cycle`) <br>• some are true by construction (`fork_source_mismatch`, `join_target_mismatch`) | Criterion becomes "contains the code, and no other error outside an allowlist". Add fact-level `.mg` fixtures for unreachable codes, or delete those rules. |

### C. Identity, determinism, persistence

| ID | Finding | Fix |
|---|---|---|
| SR-16 ✔ | `[CONS][GAP]` **ULID arrow and box keys break D6.** Keys minted at parse time give different facts, `data-key`s and SVG for identical bytes. The DSL has no arrow-key syntax. `Ulid::new()` and `Instant::now()` also **panic on wasm32-unknown-unknown**. | Use deterministic derived ids (`<diag>/<ordinal>`), with an optional `key=` that the formatter always writes back. Inject a clock and ID generator. Use `web-time`, and configure `getrandom` for wasm. Enforce with clippy `disallowed-methods`. |
| SR-17 | `[GAP]` **Comments are lost on the first UI edit.** The grammar allows `#` comments, but the server rewrites the canonical file on every patch. | Use a lossless CST (rowan-style), or attach comments to elements. State it in M2.2 acceptance. |
| SR-18 | `[GAP]` **Model identity undefined.** At least four schemes: API `{id}`, PRD `model=`, frontmatter `model:`, `abbrev`, plus the file path. | Canonical id = `abbrev`, unique per workspace. Define how each surface resolves it, and what happens on rename. |
| SR-19 | `[GAP]` **Workspace layout and committed state undefined.** Nothing says where the following live: ack records and sync baselines (content hashes, `sig_hash`), reader notes, the workspace call index, `index.json` and its schema. No format version or migration path. | Add a full `.idef0/` listing (committed `trace.lock` vs gitignored cache). Add an `idef0 <ver>` header, `idef0 migrate`, and semver policy for the DSL, contract schema and diagnostic codes. |

### D. Editing semantics and UX conflicts

| ID | Finding | Fix |
|---|---|---|
| SR-20 ✔ | `[CONS]` **Key conflicts.** <br>• Global `n`/`N` (search) collides with `N` (new box, and "No" in Confirm), and the tables write unshifted keys in capitals. <br>• Globals stay active in Confirm and in text overlays (Search, ConceptPicker), so typing `/`, `n`, `D` or `=` into a search box would fire commands. <br>• The state diagram, `Mode` enum and global list have drifted. <br>• `:optimize-ports` is a stale name. | State the notation (capital = unshifted). Move hit stepping to `F3`/`Shift+F3`. Exclude all text-entry states from globals. Add `OverlayKind::{Search, ArrangePreview, ProposalReview}`. Generate the diagram and help from `TRANSITIONS`, with a conflict test. |
| SR-21 | `[GAP]` **Delete and renumber semantics undefined.** For DeleteBox: what happens to the child subtree, attached arrows and boundary ports? Renumbering renames whole subtrees. **Call targets are written by node number**, so renumbering silently breaks calls in other files. | Specify cascade rules. Make call targets key-based (`@key`), or include cross-file call rewriting in the RenameMap. |
| SR-22 | `[CONS]` **Small FSM and semantics conflicts:** <br>• `Shift+↑/↓` port re-rank "warns", while Auto-Arrange re-codes the child atomically. They should use one path. <br>• `PgDn` on a caller box creates a forbidden child. It should follow the call or be rejected. <br>• Editing a derived diagram title conflicts with `E-TITLE-MATCH`; it should route to RenameBox. <br>• Wiring has no confirm key in PickTarget, and the return mode is undefined for boundary sources. The `O 0` example needs `Enter`. <br>• D5 says Auto-Arrange is always previewed, but §3.4a auto-applies it. The Decompose trigger is moot because a new child has 0 boxes. | Fix each in place. |
| SR-23 | `[GAP]` **Undo, concurrency and parse-failure behavior undefined.** <br>• Undo: scope (model or diagram), navigation on undo, the stack after a 409 or reload. <br>• Rebase needs the stored Commands, not Patches. <br>• Two tabs, and MCP and server writing the same file. <br>• A file that fails to parse on disk must never be overwritten. | Add an "Editing semantics" section. |

### E. Security and privacy

None of this is specified today.

| ID | Finding | Fix |
|---|---|---|
| SR-24 | `[GAP]` **Localhost server threats.** A 127.0.0.1 bind doesn't stop DNS rebinding, CSRF on `POST /ops`, or cross-site WebSocket hijacking. | Host allowlist, Origin checks, a per-launch token injected into `index.html`, and enforced JSON content type. |
| SR-25 | `[GAP]` **Agent and LLM paths.** <br>• MCP `apply_patch` lets an agent change the model without human acceptance, which contradicts NORTHSTAR principle 8. <br>• Second-order prompt injection: PRD text → proposals → descriptions → contract cards → coding agents. <br>• Nothing says what content leaves the machine. | `apply_patch` queues a proposal that needs acceptance. Contract cards fence untrusted text with provenance. Add `[ai]` opt-in, exclude globs and redaction. Guarantee no network calls offline. Add a threat-model section. |
| SR-26 | `[GAP]` **Untrusted inputs.** Markdown descriptions rendered in the inspector and SVG `<desc>` (XSS). Project `.mg` rules from PRs run in CI with no eval limits. No parser size or depth limits. | Sanitize Markdown and render no raw HTML. Set eval time and memory caps. Set parser limits. |

### F. Decisions blocking M0

| ID | Finding | Fix |
|---|---|---|
| SR-27 | **License TBD.** It blocks M0.2 manifests, the `cargo deny` allowlist, font bundling (OFL), and the verb-lexicon source (license unknown). | Decide (MIT/Apache-2.0 dual is the Rust norm). |
| SR-28 ✔ | **`claude.md` precedence paradox.** D10 says it takes precedence, but it contains: <br>• invalid Mangle syntax <br>• "Leptos or Dioxus" <br>• "all validation in Mangle", which contradicts the Rust lints <br>• 6 crates instead of 7 <br>• the "not consumed" error (SR-03) <br>• "Exigent" (should be "exiting") <br>The PRD's claim that the base schema is "preserved verbatim" is false. | Rewrite it now as a short `CLAUDE.md` map. Reverse the precedence: NORTHSTAR > PRD > CLAUDE.md, with conventions folded into the PRD. |

---

## Tier 2: Important omissions

Add these before or during the relevant milestone.

| ID | Area | What to add |
|---|---|---|
| SR-29 | Product | **Five end-to-end user journeys**, each doubling as e2e acceptance: <br>1. analyst models a new system <br>2. agent implements A21 <br>3. CI blocks drift <br>4. renumber rewrites tags <br>5. reviewer publishes <br>Also: `idef0 init` and first-run onboarding. |
| SR-30 | Metric | **FR-METRIC-01:** a `contract_complete/1` Mangle rule, computed with CI severities, reported in JSON and SARIF `run.properties`. Define "active models", and a data source for the supporting signals (local opt-in counters or CI artifacts). NORTHSTAR currently claims something the PRD doesn't specify. |
| SR-31 | Testing | **Consolidated test strategy** covering the layers, tools, CI placement and time budgets. A **random valid-model generator** epic (assumed by 4 acceptance criteria). A **fixture inventory** (the "fixture repo", the 60-diagram model, PRD fixtures, L/E/G fixtures). A golden review workflow (`cargo insta review`, visual diffs). **Instruction-count benchmarks** instead of wall-clock perf gates on CI runners. |
| SR-32 | Dev setup | A `[workspace.dependencies]` table with versions. `rust-toolchain.toml` pin and MSRV policy. `Trunk.toml`, `index.html` and the `trunk serve`→axum proxy dev loop. How `idef0 serve` finds `dist/` (rust-embed?). `--cfg=web_sys_unstable_apis`. CI OS matrix (tree-sitter needs MSVC on Windows). Release, versioning and ADR process. |
| SR-33 | NFRs | **Browser matrix** (FS Access API is Chromium-only; add an `<input type=file>` fallback for open). **Offline autosave** (IndexedDB/OPFS journal, `beforeunload`). **i18n** (`meta.lang`; English-only lints off for other languages; `unicode-width` and grapheme clusters; IME `isComposing`). **Observability** (`tracing` spans, `--profile`, no telemetry by default). **Scale limits table** (50 vs 60 diagrams is inconsistent today). WCAG 2.2 AA target plus a screen-reader outline view. |
| SR-34 | Interop | **Import and export section.** "Imports" are referenced but no format is defined (AI0Win/BPwin? JSON?). PNG needs a rasterizer (resvg/tiny-skia). PDF and print. T/G page and Markdown publishing. Node-tree diagrams (a stated goal; only the textual index is specified). |
| SR-35 | Editing | v1 scope for copy/paste of boxes and subtrees, moving a box to another diagram, concept delete and merge, an unused-concept lint, and whether `RenameConcept` changes the key or the name. A definition of the "major change" that resets status to `working`. |
| SR-36 | Cross-model | Glossary scope across models (`SEC/@key`?). `ExternalFacts` type, producers and freshness rule (CI must be deterministic). |
| SR-37 | Spec hygiene | **Vague acceptance criteria:** "manual demo", "consistent with this PRD", "≤ recorded baseline" (who records it?), "side-by-side review page" (pass criterion?), "2023 laptop", "fails to type-check" on a `String` (needs a `NonEmptyString`). **Open questions to add:** telemetry, comment preservation, identity, MCP write approval, Worker default, localized lexicon. |
| SR-38 | Research | `docs/research/ierc95.pdf` (Presley & Liles, *The Use of IDEF0 for the Design and Specification of Methodologies*, UT Arlington ARRI) is unreferenced. Add it to the informative references, with a note on what it informs (IDEF0 for methodology specification → AI proposals and contract use). |

---

## Tier 3: Engineering claims to correct

| ID | Claim | Correction |
|---|---|---|
| SR-39 | Auto-Arrange "solved exactly… optimal" (§3.4a; README "optimal") | It is an exhaustive search over box orders **for a proxy objective**, followed by per-side port ordering. The two steps are sequential, not joint. "About 50 µs per permutation including routing" is inconsistent with the 5 ms layout budget: 5040 × 50 µs ≈ 250 ms native, more in wasm. Use a combinatorial proxy with branch-and-bound, measure in a spike, and show a progress indicator. |
| SR-40 ✔ | "Terminates in exactly 2 passes" (§3.3) | Re-placement can increase track demand. Either freeze pass-1 channel assignment in pass 2, or iterate with gaps that only grow, plus a cap. |
| SR-41 | Left-edge "minimum tracks = density" (§3.5) | That holds only for unconstrained intervals. Use a VCG-aware constrained left-edge, and state that track count ≥ density. |
| SR-42 ✔ | Retro grid: "integer conversion keeps both renderers on the same Scene" | `pitch = 6 gu` is not a multiple of a 4-gu row, so tracks fall between character rows. Make vertical metrics multiples of 4 gu, or add an ASCII snap pass. |
| SR-43 | Radius clamped "in screen space at every zoom" | This makes SVG bytes depend on zoom, which breaks D6. Define radii in gu and fix export at zoom 1. |
| SR-44 ✔ | "CP437" charset with `╭╮╰╯`, ⊂, ◇ | Those glyphs are not in CP437. Rename to a "Unicode box-drawing" charset; a true CP437 mode uses square corners and ASCII relation glyphs. |
| SR-45 | Incremental search on `fst` | fst maps are immutable. Use a base fst plus an in-memory delta and periodic rebuild, or brute force over about 10k terms. Use `levenshtein_automata`, not fst's own Levenshtein feature. |
| SR-46 | winnow "multi-error diagnostics" | Recovery is unstable in winnow. Specify hand-written resynchronization at the next line or top-level keyword (easy for a line-oriented DSL). |
| SR-47 ✔ | `KeyboardEvent.code`-based mapping (risk 5) | `code` is the physical position, so the I/C/O/M mnemonics break on AZERTY and Dvorak. Use `key` for letters and punctuation and `code` for navigation and F-keys. `Alt+←` and `F1` aren't reliably preventable. |
| SR-48 ✔ | `unsafe_code = "forbid"` "except where justified" | A later `allow` cannot override `forbid`. Use `deny`, or `forbid` with no exceptions. |
| SR-49 | Font determinism via a compiled advance table | That doesn't control browser kerning, ligatures or fallback. Add `font-kerning:none; font-variant-ligatures:none`, ship the font via `@font-face`, or emit text as paths. `text_w` is theme-dependent (proportional in `classic`), which §3.1/§3.2 assume away. |
| SR-50 | Label guarantees | FR-STYLE-01's "engine guarantees port_pitch ≥ width" conflicts with the Fit step that shrinks the pitch. "No label overlaps (property)" is unattainable with greedy placement. Acceptance becomes "no overlap OR an emitted `L-LABEL-FIT`". |
| SR-51 | "100% Rust" / "pure Rust" (claude.md, NORTHSTAR) | tree-sitter's runtime and grammars are C. blake3 uses C or assembly unless its `pure` feature is on. Say "Rust codebase; native ingest links C grammars". |
| SR-52 | Leptos "fine-grained" (D1) vs a per-diagram `Memo<Scene>` | It is only fine-grained with keyed `<For>` and element-level equality. Specify that, or the 16 ms keystroke budget depends on full re-renders. |
| SR-53 | SARIF for GitHub code scanning | It needs file spans, but `Model` keeps none. Keep an element → span map, and set `partialFingerprints` by element key so alerts survive renumbering. |
| SR-54 | Misc | • `cargo dist` → `dist`. <br>• Structured-output `minItems/maxItems` may not be enforced by the API (post-validation already covers it). <br>• Model IDs hard-coded in the config example should be defaults. <br>• A warm scan of 100k files ≤ 1 s can't blake3 every file; use (path, mtime, size) as the fast path. <br>• Wasm validation budget of 16 ms with full recompute is untested; measure it in M3.1 and consider making the Worker the default. |

---

## Tier 4: Consistency and readability

| ID | Finding | Fix |
|---|---|---|
| SR-55 ✔ | **Numbering:** D11 is listed before D10; risks run 8, 10, 9; §5.2 runs (a)(b)(c)(e)(d); §4.7 sits after a `---` rule. Suffix sections: 1.5.5a, 1.5.8a, 3.4a (splits Phase 3 from Phase 4), 3.9a, M5.4b. | Renumber. Auto-Arrange becomes its own section. |
| SR-56 | **No glossary.** Terms used before definition or never defined: DRE, C-number, staircase, gu, RenameMap, EDB/IDB, BFF, FEO, Kit Cycle, dominance, Used At, Scene, Patch, VCG, dogleg. Easily confused pairs: A-0 vs A0, box number vs node number vs NodeRef, port rank vs ICOM code. | Add `docs/glossary.md`, linked from every doc. |
| SR-57 | **Scattered registries.** Diagnostic codes span 4 namespaces (`E-`, `L-`, `R-`, `G-`), several have no severity or epic (`L-SPLIT-CI`, `L-REUSE-01`, `L-BUNDLE-SUGGEST`, `L-LAYOUT-OVERFLOW`, `L-LABEL-FIT`, `L-SIG-DRIFT`, `G-BOX`…). CLI subcommands disagree between §2.1, the README and the body. Keys are spread over 6 sections. | Three canonical tables: diagnostic registry, CLI reference, keymap (generated from `TRANSITIONS`). |
| SR-58 | **Milestone dependencies are wrong:** <br>• M7.2 needs M5's ASCII renderer <br>• M8.2 serves contract cards (M10.4) <br>• M9.4 indexes code tags (M10) <br>• M2.5 uses the factory fixture (M5.4b) <br>• M7.6 `n` across diagrams needs a multi-diagram fixture <br>A-n layout has no epic. | Fix the critical path. Add "Spec §" and "Fixtures" columns to every epic table. |
| SR-59 | **Stale or contradictory examples:** <br>• the §5.2(c) fenced block doesn't parse under Appendix A <br>• Credit Policy is C2 in one place and C1 in another <br>• Validate Order is A12 vs A21 <br>• C-number forms `AK 014` / `AK14` / `AK1` <br>• port id `A1:I2` vs `A1/I2` <br>• `code="path"` vs `path::symbol` <br>• the README figure shows a shadowed box *with* a call arrow, which FR-CALL-02 forbids <br>• the README DSL example is abridged without saying so (undeclared `@validated-order`) | Fix each and mark abridged examples as abridged. |
| SR-60 | **Long table cells** (FR-STYLE-06, D5, FR-CALL-10, FR-NAME-01, FR-STYLE-01). Rust type sketches are partial and contradict the prose (no `dre`, no `Arrow`/`Diagram` structs, `Arc<Diagram>` vs `Diagram`). Appendix A is labeled "normative sketch". | Keep table cells to one-line rules, with rationale below. Label code blocks normative or illustrative. Move Appendices B and C into fixture files. |
| SR-61 | **Buried architecture rules:** sources of truth (§5.4), drags as semantic commands (FR-LAYOUT-00), persistence and offline mode, determinism details scattered in 5 places. | Add an "Architecture principles" section. |
| SR-62 | **NORTHSTAR:** <br>• defines a leaf by its drop shadow, which is theme-dependent; define it structurally <br>• criterion 3 wording <br>• supporting signals have no data source <br>• the v1/v2 horizon conflicts with the PRD (T/G pages, MCP) <br>• principle 8 vs MCP `apply_patch` <br>• "pure Rust" | Edit accordingly. |
| SR-63 | **README over-promises for pre-alpha:** "Full IDEF0", "maps every clause" (the matrix is abridged), "optimal". It lists `idef0-ingest`, which doesn't exist yet. The `claude.md` link will break at rename. | Soften the claims, mark planned items as planned, and fix the link after the rename. |

---

## Structure recommendation

The reviewer recommends **splitting the PRD**, and the author agrees. Each implementation session would then load about 8–12k tokens of relevant spec instead of 37k, and duplicated content stops drifting. Requirement IDs stay stable.

| Document | Contents |
|---|---|
| `docs/PRD.md` (about 300 lines) | Problem, vision, personas, **user journeys**, goals and non-goals, FR-METRIC, FR index (ID → doc), NFRs, decisions, risks and open questions, document map, reading guide per milestone |
| `docs/glossary.md` | All terms |
| `docs/spec/metamodel.md` | §1.5 plus identity, persistence, workspace layout, versioning, editing semantics, DSL grammar, FIPS conformance matrix |
| `docs/spec/validation.md` | Mangle schema and rules, extractor contract, severity, diagnostic registry, backend gate |
| `docs/spec/layout.md` | Layout phases, Auto-Arrange, themes and FR-STYLE, geometry checker, budgets |
| `docs/spec/editor-ux.md` | FSM, canonical keymap, search, accessibility, browser matrix, autosave |
| `docs/spec/ingestion.md` | Tagging, scanning, sync, watch, trace lockfile |
| `docs/spec/agents.md` | Contract cards, AI proposals, MCP, **threat model and privacy** |
| `docs/architecture.md` | Crates, dependency and version table, state management, API, server security, determinism contract, observability |
| `docs/testing.md` | Strategy, fixture inventory, golden workflow, model generator, perf policy |
| `docs/roadmap.md` | Milestones with Spec § and Fixtures columns, plus a dependency table |
| `docs/cli.md` | CLI reference |
| `fixtures/…` | Appendices B and C move here |

## Recommended fix order

1. **Decisions** (user): license (SR-27); `claude.md` precedence (SR-28); key-based call targets (SR-21); MCP write approval (SR-25).
2. **Split the PRD** (structure first, so fixes land in the right file): SR-55..57, SR-60, SR-61.
3. **Tier 1 fixes:** SR-01..26.
4. **Tier 2 additions:** SR-29..38.
5. **Tier 3 corrections:** SR-39..54.
6. **Residual Tier 4:** SR-58, 59, 62, 63.
7. **Re-run the consistency and Mangle review** on the split docs, then write the implementation plan.
