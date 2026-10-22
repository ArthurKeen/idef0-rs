# idef0 CLI Reference

| Field | Value |
|---|---|
| Status | DRAFT v0.2 |
| Part of | idef0-rs spec set; index: [docs/PRD.md](PRD.md) |
| Normative references | [metamodel](spec/metamodel.md) (identity, workspace layout), [validation](spec/validation.md) (diagnostics, SARIF), [ingestion](spec/ingestion.md), [agents](spec/agents.md) |
| Scope | The single authoritative list of `idef0` subcommands, global flags, model resolution rules and exit codes. Other documents link here and do not restate synopses. |

The binary is `idef0` (crate `idef0-cli`, `clap` derive). Subcommands map 1:1 to library calls. Milestone numbers are provisional; [roadmap.md](roadmap.md) is authoritative.

## 1. Conventions

- **Streams.** Results go to stdout. Logs, progress and prompts go to stderr. Stdout is never interleaved with logs, so `idef0 … --format json | jq` is always safe.
- **JSON envelope.** With `--format json`, every command emits one object: `{"schema": "idef0.cli.v1", "command": "<name>", "ok": <bool>, …payload}`. The schema is versioned with the contract and diagnostic schemas ([metamodel](spec/metamodel.md#14-persistence-and-workspace-fr-ws)).
- **Determinism.** Identical inputs give byte-identical stdout (D6). The only exceptions are `serve`, `watch`, `mcp` and network-backed `propose`.
- **Writes.** Commands that modify files do so only with an explicit flag (`--apply`, `--fix`, or `fmt` without `--check`). They always go through the command layer and the canonical formatter. A file that fails to parse is never overwritten.
- **Agent writes.** Writes from agents (`propose`, MCP) create pending proposals, accepted with `idef0 proposals accept` (D15).

## 2. Global flags

| Flag | Values (default) | Meaning |
|---|---|---|
| `--workspace <DIR>` | path (nearest ancestor containing `.idef0/`, else cwd) | Workspace root: config, model index, rules, caches. Env `IDEF0_WORKSPACE`. |
| `--model <ABBREV>`, `-m` | model abbrev | Select a model through the workspace index (§3) |
| `--format <F>` | `human` \| `json` \| `sarif` (`human`) | Output format. `sarif` only on diagnostic-producing commands. Some commands add values, listed in §5. An unsupported value exits 2. |
| `--fail-on <S>` | `error` \| `warning` \| `info` \| `never` (config `[validate] fail_on`, else `error`) | Lowest severity that makes a diagnostic-producing command exit 1 |
| `--profile <P>` | `extended` \| `fips183` (config `[validate] profile`, else `extended`) | Validation profile (D8): box limits, extensions and FIPS markers |
| `--log-format <L>` | `text` \| `json` (`text`) | Log format on stderr (`tracing` events) |
| `-v`, `-q` | repeatable | Raise or lower the log level. Env `IDEF0_LOG` takes an `EnvFilter` string. |
| `--timings` | flag | Print a span timing summary to stderr ([architecture](architecture.md#9-observability-sr-33-nfr-obs-01)) |
| `--offline` | flag | No network access. `propose` falls back to the heuristic generator. |
| `--color <C>` | `auto` \| `always` \| `never` (`auto`; `NO_COLOR` honored) | Terminal color |

`--profile` means the **validation** profile everywhere (CLI, config, UI). Performance profiling is `--timings`.

**Precedence:** command-line flag > environment variable > `.idef0/config.toml` > built-in default.

## 3. Model and node resolution

Commands with a `[MODEL]` argument resolve the model in this order. The first rule that applies wins:

| # | Rule | Example |
|---|---|---|
| 1 | A positional `MODEL` path. A token is a path if it ends in `.idef0` or contains a path separator. | `idef0 check models/order.idef0` |
| 2 | A model-qualified `NODE` (`ABBR/…`) selects that model through the workspace index | `idef0 contract FAC/A21` |
| 3 | `--model <ABBREV>`, through the workspace index (abbrev → path) | `idef0 contract -m FAC A21` |
| 4 | The only model in the workspace | `idef0 contract A21` |
| 5 | Otherwise exit 3, listing the available abbrevs | — |

- **Conflicts.** A path or qualified node that contradicts `--model` exits 2. An abbrev missing from the index exits 3.
- **Single-file mode.** With rule 1 and no enclosing workspace, the command runs on that file alone. Cross-model call targets are then reported as unresolved ([validation](spec/validation.md)).
- **`NODE` syntax:** a node number (`A21`, dotted `A2.1`, `A0`, `A-0`), a box key (`@validate-order`), or either form qualified by model (`FAC/A21`, `FAC/@validate-order`). Dotted input normalizes per [metamodel](spec/metamodel.md#32-node-numbers-and-box-numbers) (`A0.3` → `A3`).
- **One positional token** on a command that takes `[MODEL] <NODE>` is a `NODE` unless it looks like a path. So `idef0 contract A21`, `idef0 contract FAC/A21` and `idef0 contract models/fac.idef0 A21` are all valid and resolve the same box.
- **Multi-model commands** (`fmt`, `validate --all`, `search`, `scan`, `sync`, `trace`, `migrate`, `proposals list`, `export` without a model) default to every model in the workspace. `--model` restricts them to one.

## 4. Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. No diagnostic at or above `--fail-on`, and `--check` found nothing to change. |
| 1 | Rule failure: at least one diagnostic at or above `--fail-on` (`E-*`, `L-*`, `R-*`, `G-*`), or a `--check` mode found pending changes (`fmt`, `sync`, `migrate`, `render --check`). |
| 2 | Usage or parse error: bad flags or arguments, an unsupported `--format`, a model file with DSL syntax errors (nothing else can run), or an invalid `NODE`. |
| 3 | IO or workspace error: file not found or unwritable, workspace or config invalid, model resolution failed, stale proposal base that cannot be rebased, port in use, or a network or API failure. |
| 4 | Internal error: a caught panic or a violated invariant. Report it as a bug. |
| 130 | Interrupted (SIGINT) during a non-long-running command. `serve`, `watch` and `mcp` exit 0 on SIGINT or SIGTERM after a clean shutdown. |

- `E-TYPE-*` type errors are diagnostics: they exit 1, not 2. Only failures to *parse* exit 2.
- `search` with no hits exits 0.

## 5. Command reference

| Command | Synopsis | Key flags | Model resolution | Output (`--format`) | Exit | Milestone |
|---|---|---|---|---|---|---|
| `init` | `idef0 init [DIR]` | `--abbrev AB`, `--name "…"`, `--force` | Creates the workspace and its first model | human, json | 0, 2, 3 | M2.4 |
| `fmt` | `idef0 fmt [MODEL…]` | `--check`, `--stdout` | Paths, else all models | human, json | 0, 1, 2, 3 | M2.4 |
| `check` | `idef0 check [MODEL]` | `--fix` | §3 | human, json, sarif | 0–3 | M2.4 |
| `validate` | `idef0 validate [MODEL]` | `--all`, `--workspace-metric`, `--rules GLOB`, `--no-external`, `--locked`, `--context editor\|ci` (`ci`), `--sarif-timestamps` | §3; `--all` = workspace | human, json, sarif | 0–3 | M3.4 |
| `facts` | `idef0 facts [MODEL]` | `--external` | §3 | `mangle` (default), json | 0, 2, 3 | M3.2 |
| `index` | `idef0 index [MODEL]` | `--depth N` | §3 | human, json, md | 0, 2, 3 | M2.4 |
| `render` | `idef0 render [MODEL] [NODE]` | `--svg`\|`--txt`\|`--png`, `--theme`, `--charset`, `--text embed\|paths\|ref`, `--scale N` (PNG), `--publication`, `--out PATH`, `--check` | §3 | svg, txt, png; json with `--check` | 0–3 | M4.5 (txt M5.6, png M5.9) |
| `arrange` | `idef0 arrange [MODEL] <NODE>` | `--apply`, `--include merge,bundle,to-all` | §3 | human (delta + DSL diff), json (Patch) | 0, 2, 3 | M4.7 |
| `search` | `idef0 search "<QUERY>"` | `--scope model\|prd\|code\|all`, `--mangle ATOM`, `--limit N` | Workspace; `--model` restricts | human, json | 0, 2, 3 | M2.5 (PRD/code M10.5) |
| `scan` | `idef0 scan` | `--full`, `--prd-only`, `--code-only` | Workspace | human, json | 0, 3 | M9.1 / M10.1 |
| `sync` | `idef0 sync [MODEL]` | `--apply`, `--check`, `--ack DOCREF…`, `--ack-all`, `--keyify` | Workspace; `--model` restricts | human (patches + diffs), json | 0–3 | M9.3 / M10.3 |
| `trace` | `idef0 trace [MODEL]` | `--matrix` | Workspace; `--model` restricts | human, json, csv, md | 0, 2, 3 | M9.3 |
| `scaffold` | `idef0 scaffold [MODEL] <NODE>` | `--lang rust\|python\|typescript\|go\|java`, `--out PATH`, `--append`, `--force` | §3 | source text | 0, 2, 3 | M10.4 |
| `contract` | `idef0 contract [MODEL] <NODE>` | `--format md\|json`, `--schema v1` | §3 | md (default), json (`idef0.contract.v1`) | 0, 2, 3 | M10.4 |
| `propose` | `idef0 propose [MODEL] <NODE>` | `--from DOCREF…`, `--heuristic`, `--dry-run`, `--show-context` | §3 | human, json (proposal id + summary) | 0, 2, 3 | M11.1 |
| `proposals` | `idef0 proposals list\|show\|accept\|reject [ID]` | `--items N,…`, `--reason "…"`, `--origin agent\|llm\|sync\|heuristic` | Workspace; `--model` restricts `list` | human, json | 0–3 | M8.5 (UI M11.3, MCP M12.1) |
| `import` | `idef0 import <FILE>` | `--format idef0\|json` (by extension), `--as ABBREV`, `--arrange`, `--lenient` | Creates or replaces a workspace model | human, json | 0–3 | M10.6 |
| `export` | `idef0 export [MODEL]` | `--format json\|book\|svg\|png\|txt`, `--publication`, `--out PATH`, `--facts` (json), `--tg-pages`, `--trace`, `--level-names "…"` (book) | §3 | files; json summary | 0–3 | M10.6 |
| `migrate` | `idef0 migrate [MODEL…]` | `--to VERSION`, `--check` | Paths, else all models + `.idef0/` | human, json | 0–3 | M2.4 |
| `watch` | `idef0 watch` | `--validate`, `--scan` | Workspace | human, json (event stream) | 0, 3 | M8.3 |
| `serve` | `idef0 serve` | `--port N` (7183), `--bind ADDR` (127.0.0.1), `--open`, `--dev`, `--ui-dir PATH` (dev) | Workspace | web UI, HTTP API, WebSocket | 0, 3 | M8.4 |
| `mcp` | `idef0 mcp` | none | Workspace | MCP over stdio | 0, 3 | M12.1 |

Built-ins from `clap`: `idef0 help [COMMAND]`, `idef0 --version`, and `idef0 completions <SHELL>`.

## 6. Command notes

- **`init`** writes `.idef0/config.toml`, `models/<abbrev>.idef0` (A-0 with box 0, purpose and viewpoint placeholders) and `.idef0/.gitignore` (index.json, cache/, proposals/, run/; [metamodel FR-WS-01](spec/metamodel.md#14-persistence-and-workspace-fr-ws)). It refuses a directory that already has a workspace unless `--force` is given (exit 3). The workspace layout is in [metamodel](spec/metamodel.md#14-persistence-and-workspace-fr-ws).
- **`fmt`** rewrites files canonically:
  - it writes derived `key=` attributes back (D14);
  - it normalizes node-number call targets to `@key`, dotted node input, and C-numbers with spaces;
  - it preserves comments ([metamodel](spec/metamodel.md#14-persistence-and-workspace-fr-ws)).

  `--check` lists the files that would change and exits 1.
- **`check`** runs the parse, the core type check (`E-*`) and core lints (`L-*`). It does not run Mangle. `--fix` applies only fixes marked safe, such as stub concepts for unknown labels (FR-NAME-03), and writes through the formatter.
- **`validate`** runs the full pipeline (`E-*`, `L-*`, `R-*` including project rules) with code and PRD facts from the workspace index unless `--no-external` is given. JSON and SARIF include `contract_complete` (FR-METRIC-01). The details are in [validation](spec/validation.md#2-pipeline).
- **`facts`** dumps the sorted extracted facts as Mangle text, accepted by the chosen rule parser. `--external` adds code and PRD facts.
- **`index`** prints the FIPS node index: indented node numbers with diagram titles and box names. It is unrelated to the workspace model index that `scan` maintains.
- **`render`**:
  - without `NODE`, it renders every diagram into `--out DIR`, one `<node>.svg` file each (`A-0.svg`, `A21.svg`);
  - with `NODE`, it writes one diagram to stdout, or to the file `--out` names. `NODE` may also be a box key (`@validate-order`): that names the diagram detailing the box;
  - a model with errors (`E-*`) is reported and exits 1 without rendering; syntax errors exit 2;
  - M4.5 status: `--svg` with `--text ref` (the default until the bundled fonts arrive in M5.5), themes `modern`, `fips183` and `retro` (which shares the `modern` Scene), and model resolution rule 1 (a file path). `--txt`, `--png`, `--publication`, `--check`, `--text embed|paths` and `--theme classic` exit 2, naming the epic that brings them;
  - `--check` runs the geometry checker and prints `G-*` findings, which are layout bugs, exiting 1 if any are found;
  - PNG needs the rasterizer named in [architecture](architecture.md#71-capability-split) and is outside D6.
- **`arrange`** prints the delta (`feedback arcs 3→1, crossings 7→2`) and the Patch. `--apply` writes it as one atomic change, including `ReorderPorts` child re-coding and the RenameMap for `sync`. Optional proposals are included only when named in `--include` ([layout](spec/layout.md#12-auto-arrange)). v0.1 called this `:optimize-ports` and described it as "optimal"; it is neither.
- **`search`** takes the FR-SEARCH query language. `--mangle` runs a structural query (FR-SEARCH-05), equivalent to the query `mg:<atom>`. With the server or index present, PRD and code hits are grouped separately ([editor-ux §9](spec/editor-ux.md#9-full-text-search-fr-search)).
- **`scan`** incrementally indexes PRD anchors and code tags into the workspace index and cache ([ingestion](spec/ingestion.md#43-scanner-pipeline-idef0-ingestcode)).
- **`sync`** prints the SyncPlan (model patches plus file diffs).
  - `--apply` applies it.
  - `--check` exits 1 if the plan is non-empty.
  - `--ack` re-confirms stale traces.
  - v0.1's `sync --fix` is removed; use `--apply`.
- **`validate --locked`** is the CI mode. It fails if `.idef0/trace.lock` is stale relative to a fresh scan, so drift can't be hidden by an outdated lock ([ingestion](spec/ingestion.md)). `--context` selects editor or CI severities (default `ci`). `--workspace-metric` aggregates the contract-complete ratio over all models ([validation](spec/validation.md)).
- **`sync --keyify`** rewrites node-number tags in code and PRDs to key form (`idef0:@validate-order`), which survives renumbering. `--ack-all` acknowledges every stale trace at once.
- **`import` / `export`** implement FR-IO ([ingestion](spec/ingestion.md)):
  - v1 imports `.idef0` text and schema-versioned JSON. Legacy AI0Win/BPwin/IDL import is deferred.
  - An import without layout is Auto-Arranged automatically (D5).
  - `export --format book` writes the Markdown model book: node index, diagrams, and text and glossary pages per FIPS §3.4.6.
  - `--publication` drops working fields and copies Used At into a model note (C.4.1.6).
- **`propose --show-context`** prints exactly what would be sent to the LLM, after redaction, without sending it ([agents](spec/agents.md)).
- **`trace --matrix`** prints the requirement ↔ box ↔ code matrix.
- **`scaffold`** emits a contract-first stub for a leaf box, or a delegation stub for a caller (FR-CALL-10). For a decomposed box it exits 2 unless `--force` is given ([agents FR-CONTRACT-53](spec/agents.md#3-scaffolding)); `--force` writes the stub anyway, and the resulting tag raises `implements_non_leaf`.
- **`contract`** prints the contract card for one box ([agents](spec/agents.md#2-contract-cards)). The canonical forms are `idef0 contract <NODE>` inside a workspace and `idef0 contract <MODEL> <NODE>` for an explicit file. Both resolve as in §3.
- **`propose`** requests an AI decomposition for `NODE`. It needs the `[ai]` opt-in. With `--offline` or `--heuristic`, it uses the heading heuristic. The result is queued as a pending proposal (D15), and its id is printed. `--dry-run` prints the proposal without queueing it.
- **`proposals`** manages the D15 queue:

  | Subcommand | Action |
  |---|---|
  | `list` | Pending proposals: id, origin, target node, item count, base revision |
  | `show <ID>` | Items, rationale, citations and a DSL diff |
  | `accept <ID> [--items …] [--yes]` | Applies the items and their dependency closure as one change, after an interactive TTY confirmation. Without a TTY it exits 2 unless `--yes` is given and config sets `[agents] allow_noninteractive_accept = true` (default false; [agents FR-SEC-31](spec/agents.md#73-requirements-fr-sec)). A base that is stale and cannot be rebased exits 3. |
  | `reject <ID> [--items …] [--reason]` | Records the rejection |

  The `auto_accept` opt-out and the storage location are in [agents](spec/agents.md#64-proposal-lifecycle).
- **`migrate`** upgrades the `idef0 <version>` header, deprecated DSL forms and the `.idef0/` layout to `--to` (default: the current version). `--check` exits 1 if a migration is needed. The version policy is in [metamodel](spec/metamodel.md#14-persistence-and-workspace-fr-ws).
- **`watch`** watches models, PRDs and code (200 ms debounce), then rescans, refreshes facts and revalidates, printing diagnostics as they change.
- **`serve`** runs the web UI, HTTP API and WebSocket events, and includes `watch`. Its security (host allowlist, Origin checks, per-launch token) is specified in [architecture](architecture.md#8-server-security-sr-24). `--dev` enables CORS for the Trunk dev server only.
- **`mcp`** serves MCP tools over stdio ([agents](spec/agents.md#6-mcp-server-fr-mcp)). Every write tool queues a proposal (D15).

## 7. Changes from v0.1

| v0.1 | v0.2 |
|---|---|
| `idef0 search … --json` | `--format json` (global) |
| `idef0 sync --fix` | `idef0 sync --apply` |
| `idef0 contract <node>` and `idef0 contract <model> <node>` used inconsistently | Both are valid; resolution per §3 |
| `:optimize-ports` (UI) | `:arrange` / `=` / `idef0 arrange` |
| `--fail-on` with no defined exit codes | §4 exit-code table |
| No `init`, `migrate` or `proposals` | Added (SR-29, SR-19, D15) |
| No `import` / `export` | Added (FR-IO, SR-34) |
| `--profile` (observability, SR-33) | Renamed `--timings`; `--profile` is the validation profile |
