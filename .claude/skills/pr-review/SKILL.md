---
name: pr-review
description: Review one pull request on ArthurKeen/idef0-rs by verification. Run every claim the PR makes rather than reading it, apply this repo's own rules (FIPS 183 conformance with labeled extensions and deviations, determinism D6, deterministic identity D14, wasm safety, agent writes need acceptance D15, the single diagnostic registry, spec authority, requirement-tagged tests, never-weakened goldens), and produce a draft review for the user to approve before anything is posted. Use when asked to review a PR, check whether a PR can merge, or re-check a PR after its author pushed fixes.
---

# PR Review

One pull request, reviewed by verification. The built-in `/code-review` reads a
diff and finds bugs. This skill runs the repo's checks against the PR's branch in
a separate worktree, then drafts a review that says which claims were executed and
which were only read.

## Invocation

- `/pr-review 8`: full review, draft only.
- `/pr-review 8 recheck`: the author pushed after a review. Re-run each finding's
  reproduction on the new head, and nothing else.
- `/pr-review 8 post`: post the approved draft, only after the user has seen it.

## The two rules that make it worth running

**Nothing is posted until the user has read the draft and said so.** Write the
draft in the reply, offer to post it in one line, and post it verbatim when told.

**Verify, don't opine.** Every finding names the command that produced it or says
"read, not executed". The failure modes below each pass CI, or would pass a
reading of the diff. The first group comes from the user's other repos. The
second group has not happened here yet, but this repo is exposed to it.

- **Seen elsewhere.**
  - A CLI ignored `.env` and returned 401 live, while every test loaded `.env`
    itself. The tests' environment is not the CLI's environment.
  - A workflow-level condition failed validation, so *no job ran*. A workflow
    that runs nothing is not green.
  - "Tenant A sees none of B's rows" passed vacuously because B had no rows.
    Every negative assertion needs a positive control.
  - About 3,000 green tests patched the one function that was wrong. A stubbed
    seam proves nothing about the seam.
- **Exposed here.**
  - A determinism test that compares two runs *in the same process* can pass
    while the output still depends on `HashMap` order or the platform. D6 needs
    separate runs and native-vs-wasm32 comparison.
  - `std::time::Instant::now()` and `std::time::SystemTime::now()` *compile* for
    `wasm32-unknown-unknown` and panic only at runtime. A green wasm build is not
    proof; the clippy bans and `wasm-bindgen-test` are. Randomness is the opposite
    case: `rand`, `getrandom` and `ulid` (whose `Ulid::new` is `Ulid::generate` in
    ulid 3) fail the wasm32 build unless something enables `getrandom`'s `wasm_js`
    feature, which only `idef0-wasm` may do (architecture §2.2).
  - A clippy ban only works if clippy finds the right `clippy.toml`.
    `scripts/check-determinism-lints.sh` proves the strict config still rejects
    its probes.
  - A snapshot updated with `cargo insta accept` makes a regression green. Every
    changed `.snap` file needs a reason in the commit or PR.
  - A rule fixture that "yields no violations" passes vacuously if the rule never
    loads. Conformance fixtures must *contain* their code (validation FR-VAL-20).

Re-run every finding before you post it.

## This repo's shape — read before anything

- **Repo:** `ArthurKeen/idef0-rs`, **private**. The base branch is **`main`**.
  Pass `--repo` to every `gh` call.
- **Merging:** merge commits, squash and rebase are all allowed, and head branches
  are not auto-deleted. PRs so far land as **merge commits** (#1), which keeps
  stacked PRs clean; a squash would leave a stacked PR carrying duplicate copies
  of its base's commits. Check `git log main` in case that has changed. A stacked PR (base is another PR's branch) keeps its
  base's commits until the base merges. Review only what it adds on top of its
  base, and say so.
- **There is no branch protection and no required review.** Nothing stops a merge
  on red. The review is the gate.
- **Who opens PRs.** The user (ArthurKeen) opens most PRs, often written by Claude
  Code. GitHub refuses `--approve` and `--request-changes` on your own PR, so post
  the user's PRs with `--comment`.
- **The PR series follows the roadmap.** PR titles are `M<n>.<k>: …`, and commit
  messages use the same prefix. Work no epic owns (tooling, skills, housekeeping)
  uses `chore: …` (CLAUDE.md step 5); a `chore:` PR that changes product code or
  a spec is a finding. Read the epic's row in `docs/roadmap.md`. Its
  **Spec** column lists the sections to read and its **Acceptance** column lists
  what must be proven. Spec-only PRs change `docs/`.
- **Authority order:** `docs/NORTHSTAR.md` > `docs/PRD.md` and the spec set
  (`docs/spec/*`, `docs/architecture.md`, `docs/testing.md`, `docs/cli.md`) >
  `CLAUDE.md`. When code and spec disagree, the PR must change the spec or the
  code. Silent divergence is a finding.
- **CI:** `.github/workflows/ci.yml` (roadmap M0.3) runs the jobs `lint`, `test (linux)`,
  `wasm32 build` and `docs links`. Check that every job actually ran
  (`gh pr checks`), not only that nothing failed. A workflow error can run no
  jobs at all. Run §2 locally anyway: CI does not exercise the PR's claims.
- **Toolchain:** `rust-toolchain.toml` pins an exact stable release with clippy,
  rustfmt and `wasm32-unknown-unknown`. Rustup applies it automatically inside
  the worktree. If a PR bumps it, that is its own PR (architecture §12.2).

## Method

`$SCRATCH` below is the session's scratchpad directory. Never use `/tmp` or the
main checkout for worktrees and target directories.

### 0. Load the context before reading the diff

```sh
R=ArthurKeen/idef0-rs
gh pr view N --repo $R \
  --json title,author,headRefName,baseRefName,headRefOid,isDraft,mergeStateStatus,files,body,reviews,comments
gh pr checks N --repo $R
gh issue list --repo $R --state open
git fetch origin "pull/N/head:refs/remotes/origin/pr/N" main && H=origin/pr/N
B=origin/$(gh pr view N --repo $R --json baseRefName --jq .baseRefName)   # stacked PRs: the real base
git log --oneline $B..$H && git diff $B...$H --stat
```

Read the existing reviews first. If one already asked for changes, the job is
`recheck`. Then read the whole diff, and every new file in full.

**A PR's code runs on this machine when you test it.** Before building anything,
read its changes to `build.rs` files, `Cargo.toml` (new dependencies, `[patch]`,
git sources), `.cargo/config.toml`, `rust-toolchain.toml`, `scripts/`,
`.github/workflows/` and `.claude/`. Proc macros and build scripts execute at
compile time.

### 1. Does it merge, really

`mergeStateStatus` lags, so do the merge yourself:

```sh
W=$SCRATCH/wt-merge && git worktree add -q --detach $W $B && cd $W \
  && (git merge -q --no-edit $H && echo clean || { git diff --name-only --diff-filter=U; git merge --abort; }) \
  ; cd - && git worktree remove --force $W
```

### 2. Run the checks, in a worktree

Use a separate target directory per review, so the PR's build never mixes with
the main checkout's artifacts:

```sh
W=$SCRATCH/pr-N && git worktree add -q --detach $W $H && cd $W
export CARGO_TARGET_DIR=$SCRATCH/target-N
rustc --version                                     # must match rust-toolchain.toml

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# the wasm-safe crates, plus idef0-wasm: its compile_error! for a missing
# --cfg=web_sys_unstable_apis fires only in a wasm32 build
for c in idef0-core idef0-mangle idef0-layout idef0-wasm; do
  cargo build  -p $c --target wasm32-unknown-unknown
  cargo clippy -p $c --target wasm32-unknown-unknown -- -D warnings
done
scripts/check-determinism-lints.sh                  # the bans still bite

scripts/check-dependency-graph.sh   # wasm safety, no randomness in core/mangle/layout, mangle ⟂ layout
cargo deny --locked check           # licenses, bans, advisories, sources
python3 scripts/check-doc-links.py  # relative links and anchors in every tracked *.md
```

- **The dependency audit needs its positive control.** A pass proves nothing if
  the graphs are empty, so run it against a planted violation:
  1. Add `idef0-ingest.workspace = true` under `[dependencies]` in
     `crates/idef0-core/Cargo.toml`. The audit must fail and name `idef0-ingest`.
  2. Add `idef0-layout.workspace = true` to idef0-mangle. The audit must fail on
     "mangle and layout must stay independent".
  3. After each plant, run `cargo update -w --offline` before the audit. The audit
     uses `--locked`, so a stale lock fails every graph with "could not resolve"
     and never reaches the check being tested.
  4. Restore with `git checkout -- crates/ Cargo.lock`.

- **Report counts.** Compare the test counts with any the PR claims. Rust has no
  skip, but `#[ignore]` tests do not run: list every new `#[ignore]` and its
  reason. A test gated behind a feature that the default run does not enable did
  not run either.
- **Docs links:** `scripts/check-doc-links.py` covers relative links and anchors.
  External URLs are not checked: open any new ones the PR adds.
- **Clean up afterwards:** `git worktree remove --force $W && rm -rf $SCRATCH/target-N`.

### 3. Verify every claim that can be verified

| The PR says | Run |
|---|---|
| "fixes X" / "adds a regression test" | Revert the fix in the worktree (`git show ${B}:<file> > <file>`), run the test, confirm it fails, then restore |
| "removing Y breaks test Z" (a mutation claim) | Apply the mutation and run Z. Say which mutations held |
| "deterministic" / "byte-identical" (D6) | Run it twice in **separate processes** and `cmp` the outputs. For core, mangle and layout, also compare native output with the wasm32 run (`wasm-bindgen-test`, testing §4). One in-process comparison is not evidence |
| a new or updated snapshot (`.snap`, golden JSON/SVG) | Read the old-vs-new diff. Is the change intended, and does the commit say why? `cargo insta test --check` fails on pending snapshots |
| a FIPS 183 citation ("FIPS §3.3.3-8") | `pdftotext -layout docs/research/fipspub183.pdf - \| grep -n …`. Does that clause say it? Is Annex A–C (informative) cited as normative? |
| an idef0-rs-only rule or behavior | Is it labeled **[Extension]** or **[Deviation]**, and does it state its `fips183` profile behavior (metamodel conformance matrix)? |
| a new diagnostic code (`E-`/`L-`/`R-`/`G-`) | Is it in the single registry (validation §7), with one subject kind, editor/CI severities, a fips183 level and a fixture? |
| a Mangle rule | Every variable in a negated atom, comparison or head is bound positively (safety). There is no recursion through negation (stratification). Does the fixture *contain* the code? Does `fixtures/valid` stay clean? |
| a new dependency | Is it declared in root `[workspace.dependencies]` and inherited with `workspace = true`? Does `cargo tree` keep it out of the wasm32 graph if it is native? License compatible with MIT OR Apache-2.0? |
| "see `path`" | `git cat-file -e "${H}:path"` |
| a requirement ID (`FR-…`, `NFR-…`, `D…`) | It exists in the owning doc (PRD §7 index) and means what the PR says |
| "closes #N" | Read the issue. Say which bullets the PR does and does not address |
| a number in the description or docs | Recompute it |

### 4. The repository's own rules

These come from `CLAUDE.md`, `docs/NORTHSTAR.md`, the PRD decisions log and
`docs/architecture.md`:

- **D11 FIPS 183 is the baseline.** Every rule traces to a clause. Additions are
  **[Extension]**, departures are **[Deviation]**, and the `fips183` profile turns
  them off, downgrades them to info, or keeps them as notation only.
- **D6 Determinism (NFR-DET-02..04).**
  - Core, mangle and layout: no `HashMap`/`HashSet` iteration into output, no
    clock, no environment, no randomness, integer geometry, and no trig in output
    paths.
  - Every `#[expect(clippy::disallowed_types, reason = …)]` is a reviewable
    exception: is the map really never iterated into output?
  - All sorts on output data use a total order, with ties broken by element key.
- **D14 Identity.** Parsing never mints random IDs. Box keys are stable slugs,
  written back to the file. Call targets reference boxes by key. A PR that makes
  an ID depend on declaration order or on node numbers where a key is expected is
  a finding.
- **D15 Agent writes need acceptance.** No code path, flag or default auto-applies
  an agent or LLM patch. `auto_accept` is false by default and set only in
  committed config. Accept endpoints need the UI capability token (FR-SEC-15/16).
- **D17 Retired keys.** Deleting a box retires its key, and no command may reuse it.
- **Wasm safety.** `idef0-core`, `idef0-mangle` and `idef0-layout` do no file I/O,
  spawn no threads, and keep C-compiling crates out of their dependency graph.
  `idef0-mangle` and `idef0-layout` do not depend on each other.
- **`unsafe_code = "forbid"`, with no exceptions.** An `allow(unsafe_code)`
  anywhere is a must-fix.
- **Layout is computed, never drawn.** No manual coordinates in the model. Drags
  map to semantic commands. FIPS feedback shapes are hard rules: control is "up
  and over", input and mechanism are "down and under".
- **Conventions that are easy to get wrong.**
  - `( )` tunnels at the box end, `[ ]` at the border end, `( )` at both under fips183.
  - The vertical arrow carries the bridge.
  - Fillets are 6–10 px at zoom 1.
  - Controls *may* be transformed.
  - Drop shadow means the box has a child diagram.
  - DSL endpoints use FIPS reference style (`2O2 -> 3C1`).
- **Tests come first, and goldens are never weakened.** Every changed behavior
  has a test that fails without it and cites its requirement ID (`// FR-CALL-05`).
  An update to a golden, snapshot or determinism test needs a stated reason.
- **The spec changes with the code.** A behavior change names its requirement. A
  new code, flag, key binding or command updates its owning doc: the registry,
  `docs/cli.md`, or the editor-ux keymap (generated from `TRANSITIONS` once M6.4
  lands).
- **Spec-only PRs.**
  - New IDs are unique and in the right family range (for example, FR-SEC-01..19
    belong to architecture and FR-SEC-20.. to agents).
  - The sections they amend exist.
  - The PRD decisions log and change log move.
  - Nothing marked resolved is still open.
- **Manifests.** Root `Cargo.toml` is the authoritative dependency list, and
  architecture §12.1 records policy, not a copy. `Cargo.lock` is committed. An
  MSRV or toolchain change is its own PR.
- **Housekeeping.** `CHANGELOG.md` gains an entry under *Unreleased*. Commit
  messages are `M<n>.<k>: …`, or `chore: …` for work no epic owns, and cite
  requirement IDs.

### 5. `recheck` — after the author pushes

Re-fetch `pull/N/head`. For each finding, re-run its reproduction on the new head
and report pass or fail. If the original review gave no reproduction, build one
before judging, and say so.

## Output

Draft the review in the reply. Say in one line who it is written for, then give
the review, ready to paste:

1. **Verdict in one sentence**: merge, merge after X, or don't merge yet, and why.
2. **Verified**: what was run and passed. Lead with this.
3. **Must fix before merge**, numbered, each with its command or `file:line`.
4. **Should fix** and **Minor**.
5. **Answers to the author's own open questions or "decisions worth reviewing"**,
   if any.
6. **Decisions for the user**, kept separate (a spec trade-off, a new decision
   for the PRD log) and never buried among the findings.

Write in plain language, and give each finding as **Context / Action / Result**.
No generic praise and no first person ("X does not hold: Y", not "I checked X").
No internal code names a reader cannot look up.

Post with `gh pr review N --repo $R --comment|--request-changes|--approve --body-file -`,
using `--comment` on the user's own PRs.

## Known gotchas

- `git fetch origin <short-sha>` fails. Fetch `pull/N/head`.
- zsh: write `"${H}:path"`, never `$H:path` (`:t` and `:r` are zsh modifiers).
- **Stacked PRs:** diff against the PR's real base (`$B`), not `main`, or the
  base PR's changes get reviewed twice.
- **Shared target dirs:** without a per-review `CARGO_TARGET_DIR`, a worktree
  build can reuse artifacts from the main checkout and mask a compile failure.
- **Clippy config lookup:** clippy reads the `clippy.toml` nearest to each
  crate's manifest. A strict file deleted or moved in a PR silently drops the
  determinism bans. The probe script catches this, so run it.
- **`RUSTFLAGS` replaces the wasm32 flags.** A `RUSTFLAGS` or
  `CARGO_ENCODED_RUSTFLAGS` value in the shell overrides
  `.cargo/config.toml`'s `[target.wasm32-unknown-unknown].rustflags`, and the
  idef0-wasm wasm32 build then stops with its `web_sys_unstable_apis` error. That
  is the environment, not the PR: `unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS`
  before §2.
- **wasm-only failures:** `cargo test` runs native only. Code under
  `cfg(target_arch = "wasm32")` is not covered unless the wasm32 build, clippy
  and `wasm-bindgen-test` run.
- macOS has no `timeout`. A mutation can turn a loop into an infinite one, so run
  mutations one at a time and restore files with `git checkout -- <file>` in the
  worktree.
- `gh pr checks --watch` can exit while a job is still pending. Loop on
  pending/queued/in_progress instead.
- `pdftotext` is at `/opt/homebrew/bin/pdftotext`. The FIPS PDF is a scan, so its
  figures extract as garbage but its text is reliable.
