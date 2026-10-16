# idef0-rs: Ingestion, Synchronization and Interchange

| Field | Value |
|---|---|
| Status | DRAFT v0.2 (2026-10-15) |
| Part of | idef0-rs spec set; index: [docs/PRD.md](../PRD.md) |
| Normative references | FIPS PUB 183 (IDEF0), local copy [fipspub183.pdf](../research/fipspub183.pdf); CommonMark 0.31; BLAKE3; JSON Schema 2020-12; tree-sitter grammar repositories (pinned revisions, see [architecture](../architecture.md#12-dependencies-and-toolchain-sr-32)) |
| Informative references | Presley & Liles, *The Use of IDEF0 for the Design and Specification of Methodologies*, ARRI, UT Arlington, local copy [ierc95.pdf](../research/ierc95.pdf) (cited as **[ierc95]**) |
| Scope | Workspace config, PRD and code tagging standards, the scanner, sync and rename rewriting, the trace lockfile, watch mode, the server-side search index, and import/export (FR-IO). |

Contract cards, scaffolds, AI proposals, the MCP server and the threat model live in [agents.md](agents.md). Terms (DocRef, CodeRef, RenameMap, SyncPlan, Used At) are defined in the [glossary](../glossary.md).

---

## 1. Scope and responsibilities

The `idef0-ingest` crate (D7; native only, it links C tree-sitter grammars) does four things:

1. It parses Markdown requirement documents into a `PrdIndex`.
2. It scans source code into a `CodeIndex`.
3. It computes a `SyncPlan` from (model, PrdIndex, CodeIndex, trace.lock).
4. It rewrites tags in files from a `RenameMap`.

The server (`idef0-server`) and the CLI (`idef0-cli`) are its only callers. The browser never runs ingest. It receives PRD and code facts from the server, or from a committed `.idef0/index.json` ([architecture §WASM/BFF boundary](../architecture.md#7-wasm--bff-boundary-and-api)).

```rust
// (illustrative) public surface of idef0-ingest
pub fn prd::parse(path: &RelPath, text: &str, cfg: &PrdConfig) -> PrdIndex;
pub fn code::scan(root: &Path, cfg: &ScanConfig, cache: &mut ScanCache, targets: &CodeTargets) -> CodeIndex;
pub fn sync::plan(ws: &Workspace, prd: &PrdIndex, code: &CodeIndex, lock: &TraceLock) -> SyncPlan;
pub fn rename::rewrite(map: &RenameMap, files: &FileSet) -> Vec<FileEdit>;
pub fn facts::external(prd: &PrdIndex, code: &CodeIndex, lock: &TraceLock) -> ExternalFacts;
```

`ExternalFacts` is the only thing that crosses into validation. Its predicates (`code_ref`, `code_exists`, `implements`, `consumes`, `doc_ref`, `requirement`, `traces`, `doc_stale`) are declared in [validation §Schema](validation.md#4-schema). Every function above is deterministic given its inputs (D6). Sorting and tie-breaking use byte order on repo-relative paths with `/` separators on every OS.

---

## 2. Workspace configuration (`.idef0/config.toml`)

The full `.idef0/` layout (committed vs. gitignored files, the format version header and `idef0 migrate`) is owned by [metamodel §Workspace layout](metamodel.md#14-persistence-and-workspace-fr-ws). This section defines the config keys that ingestion, sync and agents read.

```toml
# (illustrative) .idef0/config.toml; every key shown at its default unless noted
version = 1

[workspace]
models  = ["models/*.idef0"]          # each model's abbrev must be unique in the workspace
prd     = ["docs/**/*.md"]
code    = ["src/**", "crates/**"]
exclude = ["target/**", "node_modules/**", "**/vendor/**"]   # .gitignore is always honored as well

[validate]
profile = "extended"                  # or "fips183" (D8)
fail_on = "error"
rules   = [".idef0/rules/*.mg"]

[prd]
req_pattern = '[\p{Lu}\p{Lo}][\p{Lu}\p{Lo}\p{Nd}]{1,9}(?:-[\p{Lu}\p{Lo}][\p{Lu}\p{Lo}\p{Nd}]*)*-\p{Nd}{1,6}'
req_ignore  = ["UTF", "SHA", "ISO", "IEC", "RFC", "CVE", "X"]   # prefixes never treated as requirement IDs
glossary_relations = "en"             # phrase set for kind_of/part_of inference; "off" disables it

[scan]
languages     = ["rust", "python", "typescript", "go", "java"]
max_file_size = "2MiB"                # larger files are skipped with an info diagnostic

[sync]
rename_tags = "propose"               # "propose" (default) | "off"; never "auto" (NORTHSTAR principle 8)

[watch]
debounce_ms = 200
poll        = false                   # force polling (network file systems)

[search]
index_untagged_prd = true

[ai]                                  # see agents.md §5 and §7
enabled      = false                  # per-workspace opt-in; false means zero network calls
provider     = "anthropic"
model        = "claude-opus-5-5"      # default, not hard-coded; override per workspace
repair_model = "claude-sonnet-5-5"    # default for repair iterations
api_key_env  = "ANTHROPIC_API_KEY"    # name of the env var; the key itself never goes in this file
exclude      = []                     # globs never sent to the LLM provider
redact       = []                     # extra regex redaction patterns (built-ins always apply)
log          = "metadata"             # "off" | "metadata" | "full" (local only)

[agents]                              # see agents.md §6
auto_accept        = false            # D15
allow_noninteractive_accept = false   # agents FR-SEC-31: `proposals accept --yes` without a TTY
contract_max_bytes = 32768
```

| Key | Rule |
|---|---|
| `version` | Required. An unknown major version is a hard error that points to `idef0 migrate`. |
| `workspace.models` | Every matched file declares a model whose `abbrev` is unique in the workspace. A duplicate is error `E-ID-06` (metamodel owns the code). |
| `workspace.exclude` | Applies to every walk (models, PRD, code, search, AI context) in addition to `.gitignore`, `.ignore` and `.git/info/exclude`. |
| `ai.model`, `ai.repair_model` | These are configuration defaults that the tool ships with. Code never hard-codes a model id. A release may change the defaults, and the changelog says so. |
| `ai.*`, `agents.*` | Semantics are specified in [agents.md](agents.md#7-threat-model-and-privacy). |
| Unknown keys | Warning. They are never silently ignored, so a misspelled `auto_accept` does not quietly fall back to the default. |

- Config is read once per command, and re-read by `serve`/`watch` when the file changes. A config change invalidates the scan cache only when `scan.*`, `workspace.code` or `workspace.exclude` changed (the cache key includes a hash of those keys).
- Paths in config are repo-relative with `/` separators. Glob syntax is that of the `globset` crate.

---

## 3. PRD tagging standard

Markdown is parsed with `pulldown-cmark` (offset iterator; tables, footnotes and definition lists enabled). A PRD needs no tags to be indexed and searched. Tags add structure: bindings to boxes and concepts, glossary extraction, and drift tracking.

### 3.1 Section anchors

An anchor is an HTML comment of this form:

```ebnf
(* normative *)
anchor     = "<!--" ws "@idef0:" target { ws attr } ws "-->" ;
target     = boxref | "concept=" KEY | "glossary" | "end" ;
boxref     = [ ABBREV "/" ] ( NODE | "@" KEY ) ;      (* NODE accepts dotted input: A2.1 = A21 *)
attr       = "model=" ABBREV
           | "role=" ( "input" | "control" | "output" | "mechanism" )
           | "label=" STRING
           | "rank=" NUMBER
           | "id=" SLUG ;
```

| Rule | Statement |
|---|---|
| FR-PRD-01 | An anchor directly followed (blank lines allowed) by a heading binds that heading's **section**: everything up to the next heading of the same or higher level, or up to `<!-- @idef0:end -->`, whichever comes first. |
| FR-PRD-02 | An anchor not followed by a heading binds an **inline region** that runs up to the next heading of any level or the next `end` anchor. |
| FR-PRD-03 | Several anchors before one heading bind the section to every target. |
| FR-PRD-04 | A section's DocRef is `<path>#<slug>`. The slug is the explicit `id=` if one is given, otherwise the GitHub-compatible heading slug (lowercase, Unicode letters and digits kept, spaces become `-`, duplicates get `-1`, `-2`…). |
| FR-PRD-05 | `boxref` resolves against the model named by `model=`, then by the frontmatter `idef0.model`, then by the abbrev prefix. If the workspace has exactly one model, that model is used. If none of these resolves, `L-PRD-03` is raised (ambiguous model) and no trace fact is emitted. |
| FR-PRD-06 | `role=`/`label=`/`rank=` mean the section **defines an ICOM** of the box, not the function itself. `label` resolves to a concept by name or synonym. If no concept matches, a new concept is proposed. |
| FR-PRD-07 | An anchor that is unterminated, malformed or bound to an empty region raises `L-PRD-01` (warning) with a byte span. |

- Use `id=` for every anchor that matters. Without it, renaming the heading changes the DocRef. Sync then detects the move by content hash (§6.4) and proposes a trace.lock rewrite, but an explicit `id=` avoids the churn.
- Box references by key (`@validate-order`) are immune to renumbering. Node-number references are rewritten by the RenameMap (§6.3).

```markdown
<!-- (illustrative) -->
<!-- @idef0:A21 id=validate-order -->
## Validate Order
The system SHALL reject orders whose credit exposure exceeds the customer limit. (REQ-031)

<!-- @idef0:A21 role=control rank=1 label="Credit Policy" id=credit-policy -->
### Credit Policy
Orders above $50k require manual approval. (REQ-032)
```

In this example, and in every example in this document, **Validate Order is A21** (box 1 on diagram A2 "Process Order" in model `ORD`) and **Credit Policy is its C1**.

### 3.2 Frontmatter

```yaml
# (illustrative)
---
idef0:
  model: ORD                 # model abbrev (the canonical model id)
  node: A2                   # default box for REQ IDs in untagged regions of this document
  viewpoint: Operations Manager
---
```

- `model` sets the default model for every anchor in the document (FR-PRD-05).
- `node` is optional. When it is present, REQ IDs found outside any anchored region trace to that box. When it is absent, those REQ IDs are indexed and searchable, but they produce no `traces` facts.
- `viewpoint` is informative. AI context assembly passes it along as document metadata ([agents §5.2](agents.md#52-context-assembly)).
- Only the `idef0` key is read. Other frontmatter keys are ignored.

### 3.3 Fenced `idef0` blocks (proposals)

A fenced code block with info string `idef0` (optionally `idef0 model=ORD`) holds a **DSL fragment**. This fixes SR-59: the old example used a `box … input … control …` shape that the DSL grammar does not accept. A fragment uses the ordinary DSL grammar ([metamodel §DSL grammar](metamodel.md#162-grammar)) through the entry point `fragment := (concept | diagram)*`. No `model` block is needed, because the model is resolved by FR-PRD-05.

````markdown
<!-- (illustrative; abridged: only box 1 of diagram A2 is shown) -->
```idef0
concept validated-order "Validated Order" {
  description "A customer order that has passed all validation rules."
  kind_of customer-order
}

diagram A2 {
  box 1 "Validate Order" key=validate-order {
    description "Checks each customer order against the credit policy and the order schema, producing a validated order or a rejection notice."
  }
  arrow "Customer Order"   : I1 -> 1I
  arrow "Credit Policy"    : C1 -> 1C
  arrow "Order Schema"     : C2 -> 1C
  arrow @validated-order   : 1O -> O1
  arrow "Rejection Notice" : 1O -> O2
  arrow "Order Validator"  : M1 -> 1M  code="src/orders/validate.rs::validate_order"
}
```
````

Fragments are **proposals**. They are merged into a candidate copy of the model, never into the model itself:

| Rule | Statement |
|---|---|
| FR-PRD-10 | A fragment `diagram X` refers to the existing diagram X. If X does not exist and its parent box does, the proposal includes a `Decompose`. |
| FR-PRD-11 | Boxes match existing boxes **by key**. A new key means AddBox at the given number. An existing key at a different number means MoveBox. Either one produces a RenameMap. |
| FR-PRD-12 | Arrows match existing arrows by (concept, source end, destination end). Unmatched arrows become AddArrow. A fragment never deletes anything. |
| FR-PRD-13 | Arrow labels that resolve to no concept become concept-stub proposals, not `unknown_concept` errors. Concept blocks in the fragment become DefineConcept proposals or description diffs. |
| FR-PRD-14 | The candidate model is validated (type check, lints, rules) before the proposal is shown. Violations are shown alongside the proposal and do not block it. |
| FR-PRD-15 | A fragment that does not parse raises `L-PRD-02` (warning) with spans that map back into the Markdown file. |

`idef0 sync` turns fragments into reviewable proposals ([agents §6.4](agents.md#64-proposal-lifecycle)) and never applies them silently.

### 3.4 Names, descriptions and concepts from prose (FR-NAME)

| Source | Proposal |
|---|---|
| Function-anchored section (`@idef0:A21`) | The heading, verb-normalized, is proposed as the box name. The first paragraph is proposed as the description. The proposal is made only where the model lacks the text, or as a diff where the texts differ. |
| `@idef0:concept=<key>` section | The section is the concept's **definition**. The heading becomes the name and the first paragraph the description. |
| `role=… label="…"` section | The label resolves to a concept (or a new-concept proposal). The section text becomes the concept description proposal. |
| `@idef0:glossary` section | Two-column tables (`Term \| Definition`) and definition lists become concept proposals. `kind_of`/`part_of` are inferred only from explicit phrases in the configured phrase set (`en`: "is a kind of", "is a type of", "is part of"). |

- Text adopted from a PRD carries a `sources` DocRef and origin `prd` ([agents §2.3](agents.md#23-untrusted-text-fencing-and-provenance)). Drift on that DocRef (§3.6) therefore also flags descriptions that may need updating.
- FIPS §3.3.1.4 forbids text that "merely describe[s], redundantly, the meaning of boxes and arrows". Description proposals go through `L-DESC-02` like any other description.
- Glossary relation inference is turned off when `glossary_relations = "off"`, and when the model's `meta.lang` is not covered by a phrase set.

### 3.5 Requirement IDs

| Rule | Statement |
|---|---|
| FR-PRD-20 | Tokens matching `prd.req_pattern` inside a bound region become `requirement(ReqId, DocRef)`, plus `traces(DocRef, BoxId)` for each box target of the region. |
| FR-PRD-21 | The default pattern accepts Unicode uppercase letters and other letters (`\p{Lu}`, `\p{Lo}`), so `REQ-031`, `FR-NAME-01`, `ТРБ-12` and `要件-7` all match. |
| FR-PRD-22 | A match counts only at a token boundary: the characters before and after it must not be letters, digits or `-`. The check is a post-match test, because the `regex` crate has no look-around. |
| FR-PRD-23 | Matches whose first segment is listed in `prd.req_ignore` are discarded (for example `UTF-8`, `SHA-256`, `ISO-9001`). |
| FR-PRD-24 | Inline code and fenced code are not scanned for REQ IDs. |

REQ IDs feed the traceability matrix (`idef0 trace --matrix`, see [cli](../cli.md)) and the requirements section of contract cards.

### 3.6 Drift and the trace lockfile

The trace lockfile `.idef0/trace.lock` is **committed**. It records what a human last acknowledged, so drift review happens in code review.

```toml
# (normative shape; keys sorted bytewise; no timestamps, so identical acks produce identical files)
version = 1

[docs."docs/prd.md#validate-order"]
hash    = "blake3:9f2c…"                 # hash of normalize(section) at last ack
targets = ["ORD/@validate-order"]

[code."src/orders/validate.rs::validate_order"]
sig_hash = "blake3:41aa…"
targets  = ["ORD/@validate-order"]

[tags."src/orders/validate.rs::validate_order"]
"implements idef0:A21" = "ORD/@validate-order"   # key that a node-number tag resolved to at last sync
```

| Rule | Statement |
|---|---|
| FR-PRD-30 | `normalize(section)` is the plain-text event stream of the region (text and code spans), NFC-normalized, with whitespace runs collapsed and anchor comments removed. Rewrapping or reformatting text does not count as drift. |
| FR-PRD-31 | If a DocRef's current hash differs from its lock `hash`, the fact `doc_stale(DocRef)` is emitted, which derives `stale_trace` ([validation](validation.md#5-rules)). |
| FR-PRD-32 | A DocRef with no lock entry is **not** stale. `idef0 sync` proposes a lock entry for it as its baseline. |
| FR-PRD-33 | `idef0 sync --ack <docref\|coderef>…` (or `--ack-all`) writes the current hashes into trace.lock. This is the only command that writes `[docs]` and `[code]` entries. |
| FR-PRD-34 | `idef0 validate --locked` fails if trace.lock would change: a missing baseline, a stale hash, or tag memory out of date. It is the CI mode, analogous to `cargo --locked`. |
| FR-PRD-35 | trace.lock is plain TOML with sorted keys, one table per DocRef or CodeRef, so concurrent branches rarely conflict. Conflicts resolve by re-running `idef0 sync --ack`. |

The `index.json` produced by `idef0 scan` (for offline client validation) is a derived artifact. trace.lock is the authority. index.json is never read back as a baseline.

---

## 4. Code tagging standard

### 4.1 Tag forms

Tags appear in any comment the language allows. That includes Rust `//`, `///`, `//!` and `/* */`; Python `#` and the **first docstring** of a `def` or `class` (which binds to that item); TypeScript and Java `//`, `/* */` and doc comments; and Go `//`.

```rust
// (illustrative)
// @implements  idef0:@validate-order
// @consumes    idef0:@validate-order/I1  "Customer Order"
// @governed-by idef0:@validate-order/C1  "Credit Policy"
// @produces    idef0:@validate-order/O1  "Validated Order"
pub fn validate_order(order: &Order, policy: &CreditPolicy) -> Result<ValidatedOrder, RejectionNotice> { /* … */ }
```

```python
# (illustrative) node-number form, rewritten by sync on renumber
def validate_order(order, policy):
    """Validate a customer order.

    @implements idef0:ORD/A21
    """
```

```ebnf
(* normative *)
tag     = "@" kind ws "idef0:" target [ ws STRING ] ;      (* trailing STRING = informative label *)
kind    = "implements" | "consumes" | "governed-by" | "produces" ;
target  = [ ABBREV "/" ] ( NODE [ port ] | "@" KEY [ "/" side DIGITS ] ) ;
port    = [ "/" ] side DIGITS ;                              (* "A21/I1" canonical; "A21I1" FIPS form *)
side    = "I" | "C" | "O" | "M" ;
NODE    = "A0" | "A" DIGIT { DIGIT } | dotted ;              (* "A2.1" = "A21"; "A21.3" = "A213" *)
```

| Tag | Fact ([validation §Schema](validation.md#4-schema)) | Meaning |
|---|---|---|
| `@implements idef0:<box>` | `implements(C, B)` | Code element C is (part of) the implementation of box B. |
| `@consumes idef0:<box>/I<k>` | `consumes(C, B, /input, k)` | C takes the I<k> content. |
| `@governed-by idef0:<box>/C<k>` | `consumes(C, B, /control, k)` | C is parameterized or constrained by C<k>. |
| `@produces idef0:<box>/O<k>` | `consumes(C, B, /output, k)` | C yields O<k>. |

| Rule | Statement |
|---|---|
| FR-TAG-01 | Reference forms follow [metamodel §3.5](metamodel.md#35-port-references): `A21/I1` is canonical. The FIPS form `A21I1` (Annex C.4.1.6) is accepted. `ORD/A21/I1` parses as model / node / port. Dotted nodes follow FIPS §3.3.4.5 ("look specifically at"). |
| FR-TAG-02 | Key references (`@validate-order`, `ORD/@validate-order`) are an **[Extension]**. They are immune to renumbering. A port on a key reference requires the slash. |
| FR-TAG-03 | `@implements` targets a box only. A port suffix on `@implements` is malformed. Mechanism-to-code binding is the model's DSL `code=` attribute, because the model owns structure (§6.1). |
| FR-TAG-04 | The side must match the kind: `@consumes`→I, `@governed-by`→C, `@produces`→O. A mismatch, an unparseable target or an unknown kind raises `L-TAG-01` (warning; error under `--fail-on warning`). |
| FR-TAG-05 | A node-number tag without an abbrev, in a workspace with several models and no resolvable default, raises `L-TAG-02` (ambiguous model) and emits no fact. |
| FR-TAG-06 | Tag targets are canonicalized when facts are emitted: dotted→undotted, FIPS port→slash port, abbrev→model. The canonical form is what appears in trace.lock and contract cards. |
| FR-TAG-07 | `idef0 sync --keyify` proposes rewriting node-number tags to key form. Key form is recommended for long-lived code. |

**Optional attribute form.** The no-op proc-macro crate `idef0-attr` (M10.4, optional) provides `#[idef0_attr::implements("@validate-order")]` and the matching `consumes`/`governed_by`/`produces`. The scanner treats them exactly like the comment form. Unlike comments, they survive any formatter.

### 4.2 Binding rules

1. A tag binds to the **nearest following AST item** among: function, method, struct, enum, trait, impl block, class, interface, type alias and module. Doc comments, attributes, decorators and annotations in between are skipped.
2. A tag before the first item in a file, or a tag followed by `@scope file`, binds to the **file** (`symbol = "<file>"`).
3. A Python docstring tag binds to its enclosing `def` or `class`.
4. A tag followed by no item in its scope (end of block or file) binds to the enclosing item, or to the file at top level.
5. `CodeRef = "<repo-relative path>::<qualified symbol>"`. The separator is always `::`, so Python `Order.validate` is written `orders/model.py::Order::validate`. A Go method is `Receiver::Method`. Overloaded or cfg-duplicated items share one CodeRef, and its `sig_hash` covers all of them, sorted.
6. Each bound item records `lang`, byte span, `sig_hash = blake3(normalize(signature))` and the tags bound to it. The signature region per language is defined in `queries/<lang>/signature.scm`. Normalization strips comments and collapses whitespace.

### 4.3 Scanner pipeline (`idef0-ingest::code`)

| Step | Behavior |
|---|---|
| 1. Walk | `ignore` crate, parallel (`rayon`). It honors `.gitignore` and `workspace.exclude`, does not follow symlinks out of the workspace root, and skips files over `scan.max_file_size` (info diagnostic). |
| 2. Fast path | The cache entry is keyed by `(path, mtime_ns, size)`. If all three match and the mtime is older than the scan start minus 2 s (the "racily clean" guard, as in git), the cached result is reused **without reading the file**. |
| 3. Hash | Only files whose fast-path key changed are read and hashed with blake3. If the hash equals the cached hash, the cache entry's mtime and size are refreshed and the cached parse is reused. |
| 4. Pre-filter | Changed files are scanned with `memchr`/`regex` for `@(implements\|consumes\|governed-by\|produces)\s+idef0:` and `idef0_attr::`. Only matching files are parsed, plus the must-parse set (step 5). |
| 5. Must-parse set | Every file named by a DSL `code=` target in any workspace model is parsed, **whether or not it contains tags** (SR-14). |
| 6. Parse | tree-sitter with grammars pinned by exact version in `[workspace.dependencies]`: rust, python, typescript/tsx, go, java. Item queries live in `queries/<lang>/items.scm`. Each parse runs with a time budget, and a timeout produces an info diagnostic and no facts for that file. |
| 7. Emit | `CodeIndex` → facts `code_ref`, `code_exists`, `implements`, `consumes`. |
| 8. Signature drift | If an item's `sig_hash` differs from its trace.lock `[code]` entry, `L-SIG-DRIFT` (warning) is raised: the contract may have changed under the box. `idef0 sync --ack <coderef>` accepts it. |

| Rule | Statement |
|---|---|
| FR-SCAN-01 | `code_exists(C)` is emitted for every CodeRef that resolves: a bound tag item, or a `code=` target whose file exists and (if a symbol is given) whose qualified symbol is found by the item query. Existence does **not** require a tag (fixes SR-14 / B13). |
| FR-SCAN-02 | A bare path in `code="path"` means file scope (`path::<file>`). It exists if the file exists and is not excluded. |
| FR-SCAN-03 | The cache lives at `.idef0/cache/scan.bin` (gitignored). Its header holds the idef0 version, the grammar versions, the query-file hashes and the config hash. Any mismatch makes the next scan cold. |
| FR-SCAN-04 | **Performance (NFR-PERF-02):** for a 100k-file repo, the cold scan takes ≤ 10 s and the warm scan of an unchanged tree ≤ 1 s. The warm path does zero blake3 hashes and zero parses. CI asserts the counts, not the wall-clock time ([testing](../testing.md#6-benchmarks-and-performance-gates)). |
| FR-SCAN-05 | Paths in facts are repo-relative with `/` separators. Results are sorted by (path, byte offset) before emission (D6). |

- blake3 runs only in the native ingest crate, so its default (SIMD/C) features are permitted. Core, mangle and layout do not depend on it.
- The Rust attribute form (`idef0-attr`) is matched by the same pre-filter and by an `attribute_item` query.

---

## 5. Identity resolution across surfaces

Every ingest surface resolves models by **abbrev**, the canonical model id ([metamodel §Identity](metamodel.md#3-identity-fr-id)), and boxes by **key** or node number:

| Surface | Model resolution | Box resolution |
|---|---|---|
| PRD anchor | `model=` → frontmatter → abbrev prefix → single model | `@key` or NODE (dotted accepted) |
| Fenced block | `idef0 model=` info string → frontmatter → single model | Box `key=` attribute |
| Code tag | Abbrev prefix → single model | `@key` or NODE |
| DSL `code=` | The containing model | n/a |
| Call target (D14) | `SEC/@key` or the local model | Key only. The FIPS label `SEC/A2.1` is computed for display. |

If a model's abbrev is renamed, the old abbrev stays in the workspace index as an alias for one release cycle. `idef0 sync` proposes FileEdits that rewrite `OLD/` prefixes in tags and anchors.

---

## 6. Synchronization semantics

### 6.1 Sources of truth

| Artifact | Owns | Never owns |
|---|---|---|
| Model file (`.idef0`) | Structure: boxes, keys, arrows, ICOM codes, concepts, mechanism `code=` bindings | Requirement text, code existence |
| Code | The existence and signatures of implementing elements, and `@implements`-style claims | Structure |
| PRD | Requirement text and REQ IDs | Structure |
| `trace.lock` | Acknowledged baselines and tag-resolution memory | Anything else |

Sync never silently rewrites any of these. Every change is a `SyncPlan`, presented for review (`idef0 sync`, `GET /api/models/{id}/sync`, or the UI) and applied only with `--apply` or by acceptance in the UI.

```rust
// (illustrative)
pub struct SyncPlan {
    pub patches:     Vec<ProposedPatch>,  // model Commands (origin = sync), each with a reason and sources
    pub file_edits:  Vec<FileEdit>,       // tag/anchor rewrites as unified diffs
    pub lock_edits:  Vec<LockEdit>,       // baselines and tag-memory updates
    pub diagnostics: Vec<Diagnostic>,
}
pub struct FileEdit { pub path: RelPath, pub base_hash: Blake3, pub hunks: Vec<Hunk> }
```

- `--apply` refuses a FileEdit whose file no longer matches `base_hash` (stale plan). It writes each file atomically (temp file plus rename), and it refuses to touch a model file that currently fails to parse ([metamodel §Editing semantics](metamodel.md#13-editing-semantics-fr-edit)).
- Model patches are applied as Commands through the normal undo stack. A plan produced or applied on behalf of an agent (MCP or API with `origin=agent`) becomes a pending proposal instead (D15, [agents §6.3](agents.md#63-writes-pending-proposals-d15)).

### 6.2 Plan contents

| Direction | Trigger | Plan contents |
|---|---|---|
| Diagram → files | A Patch carries a RenameMap (renumber, move, Auto-Arrange port re-rank) | FileEdits rewriting node-number tags and anchors, plus port suffixes on **both** node-number and key tags (§6.3) |
| Diagram → code | A leaf box has a code-bound mechanism (`code=`) and no grounding (no `@implements` on it, nor through a call chain) | `ungrounded_mechanism`; suggestion `idef0 scaffold <node>` ([agents §3](agents.md#3-scaffolding)) |
| Diagram → code | A `code=` target does not exist | `stale_mech_binding`, with fuzzy suggestions of existing CodeRefs that have similar symbol names |
| Code → diagram | `@implements` on a box with no code-bound mechanism arrow | Proposed `AddArrow(boundary M → box M, concept from the symbol, code = CodeRef)`, or a binding of an existing unbound mechanism arrow |
| Code → diagram | A tag names an unknown node or key | `implements_unknown_box` with ranked suggestions (§6.5) |
| Code → lock | A node-number tag now resolves to a box whose key differs from the trace.lock tag memory | `L-TAG-03` (tag rebound) with a FileEdit restoring the recorded key's current node number, or `--keyify` |
| PRD → diagram | A fenced block, or a role-tagged section with no matching element | Proposal (§3.3) |
| PRD → lock | A section hash changed | `stale_trace` until `--ack` (FR-PRD-31, -33) |
| Any → lock | A new DocRef or CodeRef with no baseline | LockEdit (baseline). It is not a diagnostic. |

**`ungrounded_mechanism` (prose corrected, B11).** The rule fires only for a **leaf** box that has at least one mechanism arrow with a `code=` binding and is not grounded: no existing `@implements` CodeRef on the box, and none through call propagation (FR-CALL-10). A leaf with no mechanism arrows, or with only `kind=resource` mechanisms, does not trigger it. Such a leaf is still not contract-complete ([PRD §North Star metric](../PRD.md#6-success-metric), FR-METRIC-01). The rule definition is owned by [validation](validation.md#5-rules).

### 6.3 Rename rewriting (SR-21)

A RenameMap ([metamodel §Editing semantics](metamodel.md#13-editing-semantics-fr-edit)) holds `old NodeId → new NodeId` for every box in a renumbered subtree, `old PortRef → new PortRef` for every re-ranked port, and arrow id changes. Ingest uses the first two:

| Rule | Statement |
|---|---|
| FR-REN-01 | Node-number tags and anchors whose node is in the map are rewritten. Example: renumbering A2→A3 rewrites `idef0:A21` → `idef0:A31` and `<!-- @idef0:A21 -->` → `<!-- @idef0:A31 -->`. |
| FR-REN-02 | Key-based box references are never rewritten. |
| FR-REN-03 | Port suffixes **are** rewritten on both forms when ports are re-ranked. Keys identify boxes, not ports: re-ranking A21's controls turns `@validate-order/C2` into `@validate-order/C1`. |
| FR-REN-04 | `diagram NODE` headers in fenced blocks are rewritten. Box numbers inside fragments are left unchanged, because fragments match boxes by key (FR-PRD-11). |
| FR-REN-05 | A rewrite replaces only the target token inside the comment or anchor span and preserves all surrounding bytes. The written form keeps the user's style (dotted, FIPS port or slash port). |
| FR-REN-06 | The map is applied simultaneously, not sequentially, so swaps (A21↔A22) are correct. |
| FR-REN-07 | Rewrites are proposed by default (`sync.rename_tags = "propose"`). The UI shows "N tags in M files" with the diff after any renumbering Patch. |
| FR-REN-08 | **Call targets are key-based (D14)**, so renumbering never breaks a call, in this model or in any other. Cross-model call labels such as `SEC/A2.1` are computed for display and are never written to other models' files. No cross-file call rewriting exists or is needed. |

If FR-REN-07 rewrites are not applied, trace.lock tag memory catches the stale tags on the next scan (`L-TAG-03`). A node-number tag that silently re-binds to a different box is the main hazard of node-number tags. FR-TAG-07 removes it.

Free-text node references in descriptions and model notes (for example "see A21") are rewritten within the same model by the core ([metamodel](metamodel.md#13-editing-semantics-fr-edit)). Free text in PRD prose outside anchors is not rewritten.

### 6.4 Moved sections

If a DocRef in trace.lock no longer exists but a section with the same `hash` exists under another slug in the same or another file, sync proposes a LockEdit that moves the entry, together with FileEdits for any anchors that need it. If no such section exists, `trace_unknown_box`-style cleanup is proposed: the lock entry is removed after confirmation.

### 6.5 Unknown-node suggestions

For `implements_unknown_box` and `trace_unknown_box`, suggestions are ranked deterministically:

1. The trace.lock tag memory for this tag: the recorded key's current node.
2. Exact key match after slug normalization (for example `validateOrder` → `validate-order`).
3. Key similarity (normalized Levenshtein ≥ 0.8).
4. Similarity between the box name and the code symbol name (both tokenized and stemmed with the FR-SEARCH analyzer).

Ties break by node order, then key. At most 3 suggestions are returned, each one as a FileEdit.

---

## 7. Watch mode

`idef0 serve` and `idef0 watch` watch the model, PRD, code and config globs with `notify`.

| Rule | Statement |
|---|---|
| FR-WATCH-01 | Events are debounced (`watch.debounce_ms`, default 200 ms) and coalesced per path. Atomic-save patterns (write temp, rename) count as a single modification. |
| FR-WATCH-02 | Pipeline: incremental rescan of the changed paths only → ExternalFacts diff → revalidation of the affected models → WebSocket `IndexUpdated{generation}` and `Diagnostics` push ([architecture §API](../architecture.md#72-http-api)). |
| FR-WATCH-03 | The server's own writes are suppressed: an event whose content hash equals the hash the server last wrote for that path is ignored. |
| FR-WATCH-04 | If a model file changes on disk and fails to parse, the last good model stays loaded, `E-PARSE-*` is pushed, and the file is never overwritten until it parses again. |
| FR-WATCH-05 | If native events are unavailable (network file systems) or `watch.poll = true`, the watcher polls every 2 s using the scan fast path (§4.3). |
| FR-WATCH-06 | `idef0 watch` (CLI) prints diagnostics on each settled change and exits non-zero only on fatal errors. |

---

## 8. Server-side search index (FR-SEARCH-02, server part)

The model index is built in Rust in the browser and the CLI ([editor-ux §Search](editor-ux.md#9-full-text-search-fr-search)). The **PRD and code corpora** are indexed server-side with `tantivy` (native only). Search results merge both indexes. This section owns the server index. The query language, ranking weights and overlay are owned by editor-ux.

| Rule | Statement |
|---|---|
| FR-SEARCH-20 | A PRD document in the index is one section: path, DocRef, heading, normalized text, REQ IDs, bound targets (model, key, node), and a `tagged` flag. Untagged sections are indexed when `search.index_untagged_prd = true`. |
| FR-SEARCH-21 | A code document in the index is one bound item: CodeRef, path, symbol, lang, tag kinds, targets, the label strings in its tags, and the item's leading doc comment (≤ 1 KiB). |
| FR-SEARCH-22 | The analyzer matches FR-SEARCH-03 (case and accent folding, Unicode segmentation, English stemming). Typo tolerance uses tantivy fuzzy term queries with the same distance thresholds as FR-SEARCH-03. |
| FR-SEARCH-23 | The index is updated incrementally by watch events (§7) and committed at most every 500 ms. It lives in `.idef0/cache/search/` (gitignored) and is rebuilt from scratch whenever its schema version changes. |
| FR-SEARCH-24 | Results are re-scored into the FR-SEARCH-06 scale (PRD and code weight 0.6) and tie-broken by (kind, path, byte offset), so identical queries over an identical tree return identical lists. |
| FR-SEARCH-25 | Each hit carries its links: PRD hit → bound boxes; code hit → implemented boxes and ports. Navigating a hit in the UI jumps to the box. |
| FR-SEARCH-26 | Performance: ≤ 50 ms per query for a 100k-file repo (FR-SEARCH-10, NFR-PERF-03). |
| FR-SEARCH-27 | Surfaces: `GET /api/search?scope=model\|prd\|code\|all`, `idef0 search --scope`, and MCP `search_model` ([agents §6.2](agents.md#62-tools)). |

- I propose an `is:tagged` filter for FR-SEARCH-04 (editor-ux owns the query language).

---

## 9. Import and export (FR-IO)

This section answers SR-34. Rendering itself is owned by [layout](layout.md). This section defines the formats, the commands and what v1 includes.

### 9.1 Imports

| ID | Requirement | v1 |
|---|---|---|
| FR-IO-01 | `.idef0` text is the canonical format (D3). `idef0 fmt` normalizes it, including writing back derived keys and keyifying call targets (D14). | Yes |
| FR-IO-02 | JSON interchange: `idef0 import model.json` writes canonical DSL, and `idef0 export --format json` writes JSON. The schema is `schemas/model.v1.json` (JSON Schema 2020-12), and every document carries `"schema": "idef0.model.v1"`. | Yes |
| FR-IO-03 | JSON round-trip: `import(export(m))` formats byte-identically to `m`, comments excepted. Comments are not carried in JSON v1. | Yes |
| FR-IO-04 | An unknown major version is rejected. An unknown field in a known major version is rejected unless `--lenient` is given, in which case it is dropped with a warning. | Yes |
| FR-IO-05 | A model imported without layout hints gets Auto-Arrange applied automatically before first display (D5). The arrangement is shown as a reviewable patch. | Yes |
| FR-IO-06 | Legacy tool formats (KBSI AI0Win, BPwin/ERwin Process Modeler `.bp1`, and IDEF0 IDL as defined by IEEE Std 1320.1-1998, which is not in-repo and unverified) are **deferred**. The candidate first target is IDL, because it is a published text form. See open question OQ-I1. | No |

### 9.2 Exports

| ID | Requirement | v1 |
|---|---|---|
| FR-IO-10 | SVG per diagram (`idef0 render <model> <node> --svg`). It is byte-deterministic (D6). Embedded `<title>`/`<desc>` hold sanitized plain text ([agents §7](agents.md#7-threat-model-and-privacy), FR-SEC-23). | Yes |
| FR-IO-11 | PNG via `resvg`/`tiny-skia`, with the theme's fonts embedded. PNG is **not** byte-deterministic and is excluded from D6 and from byte-exact golden tests. Tests compare the decoded image with the perceptual tolerance in [testing §5.1](../testing.md#51-what-is-snapshotted). | Yes |
| FR-IO-12 | ASCII/Unicode text (`--txt`, charsets `unicode` and `ascii`). Deterministic. | Yes |
| FR-IO-13 | PDF via SVG→PDF (`svg2pdf`), for single diagrams and the model book. **Deferred** to after v1.0, behind a `pdf` feature. In v1, publication goes through the Markdown book or the browser's print of SVG. | No |
| FR-IO-14 | Markdown model book (`idef0 export --format book <model> <dir>`), structured per §9.3. Deterministic. | Yes |
| FR-IO-15 | JSON (FR-IO-02 schema). `--facts` exports the Mangle fact set instead (debugging, [validation](validation.md#2-pipeline)). | Yes |
| FR-IO-16 | Contract cards in Markdown or JSON ([agents §2](agents.md#2-contract-cards)). | Yes |
| FR-IO-17 | Node-tree diagram (FIPS §3.3.4.2.1: optional, content identical to the node index) as SVG. **Deferred**. v1 emits the node index as text and Markdown. | No |

### 9.3 Markdown model book (FIPS §3.4.6)

The book follows the FIPS presentation rules. FIPS §3.4.6 is normative for the rules marked (N). Annex B page-pair binding is informative.

| Rule | Statement |
|---|---|
| FR-IO-20 (N) | `index.md` is the table of contents as a **node index**: node numbers, diagram titles and box names, in node-index order (§3.4.6-4, §3.3.4.2.1). |
| FR-IO-21 (N) | One page per diagram, in node-index order. The diagram (SVG, plus PNG fallback when `--png` is given) is **immediately followed by its text**, the diagram description (FR-NAME-05), so text accompanies the graphic (§3.4.6-1). |
| FR-IO-22 (N) | **Non-publication** books (any diagram below `publication` status) attach a per-node glossary to each diagram page. It defines only the concepts carried by that diagram's arrows and the functions on it (§3.4.6-2). |
| FR-IO-23 (N) | **Publication** books (`--publication`, which requires every included diagram at `publication` status) contain a single glossary section for the whole model in alphabetical order (§3.4.6-3), collated by Unicode code point after case folding. |
| FR-IO-24 | Text and glossary pages may optionally be emitted as separate files named with FIPS node-reference suffixes (`A21T.md`, `A21G.md`, §3.4.4) using `--tg-pages`. |
| FR-IO-25 | In publication books, working fields are dropped, and Used At is copied into a model note on the diagram page (Annex C.4.1.6), together with the box description and call references. |
| FR-IO-26 | Optional sections (`--trace`): traced requirements per box, and implementing CodeRefs per leaf. |
| FR-IO-27 | All Markdown from descriptions is emitted through the sanitizer (FR-SEC-23): no raw HTML, links restricted to `http`, `https`, `mailto` and relative links. |

- [ierc95] reports that models used as methodology specifications were converted into delivery documents level by level (Ax → "Phase", Axx → "Task", Axxx → "Step"), with "minimal effort", and that most of the model text was copied directly. The book's node-index ordering and per-diagram text pages are designed for that transfer. A `--level-names "Phase,Task,Step"` option labels the headings by depth.

---

## 10. Diagnostics introduced in this document

Ingest emits these Rust-side lints. They are registered in [validation §Registry](validation.md#7-diagnostic-registry).

| Code | Subject kind | Default | Meaning |
|---|---|---|---|
| `L-PRD-01` | docref | warning | Malformed, unterminated or empty-region anchor |
| `L-PRD-02` | docref | warning | Fenced `idef0` block does not parse |
| `L-PRD-03` | docref | warning | Anchor's model cannot be resolved (ambiguous or unknown abbrev) |
| `L-TAG-01` | coderef | warning | Malformed tag (unparseable target, kind/side mismatch, port on `@implements`) |
| `L-TAG-02` | coderef | warning | Node-number tag with an ambiguous model |
| `L-TAG-03` | coderef | error in CI, warning in editor | Node-number tag rebound to a different box since the last sync |
| `L-SIG-DRIFT` | coderef | warning | Implementing item's signature changed since the last ack |

The R-codes this document refers to (`stale_trace`, `ungrounded_mechanism`, `stale_mech_binding`, `stale_code_ref`, `implements_unknown_box`, `implements_non_leaf`, `consumes_unknown_port`, `trace_unknown_box`, `untraced_leaf`) are defined in [validation](validation.md#5-rules).

---

## 11. Open questions

| ID | Question | Default until resolved |
|---|---|---|
| OQ-I1 | Which legacy format to import first: IEEE 1320.1 IDL, AI0Win or BPwin? It requires obtaining the IEEE text and sample files. | Deferred past v1 (FR-IO-06) |
| OQ-I2 | Should PDF (FR-IO-13) move into v1 for journey 5 ("reviewer publishes")? | Deferred; browser print of the Markdown book or SVG |
| OQ-I3 | Should the frontmatter `node:` default (§3.2) be removed, since it silently traces every untagged REQ ID? | Kept, opt-in |
| OQ-I4 | Do tree-sitter parse time budgets use the progress-callback API (newer tree-sitter) or `set_timeout_micros`? It depends on the pinned version. | Decided in M10.1 |
| OQ-I5 | Should `L-TAG-03` default to error everywhere once `--keyify` exists? | Error in CI only |
