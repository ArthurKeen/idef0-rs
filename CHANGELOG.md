# Changelog

All notable changes to idef0-rs are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versioning follows the
policy in [docs/architecture.md §16](docs/architecture.md#16-release-and-versioning-sr-54).

## [Unreleased]

### Added
- CI cross-OS and MSRV (roadmap M0.4): `test (macos)` on macos-14, `test (windows, MSVC)` on windows-2022, and `msrv`, which checks the native crates on the `rust-version` from `[workspace.package]` (1.90). `.gitattributes` keeps every checkout LF.
- CI base (roadmap M0.3): `.github/workflows/ci.yml` with the `lint`, `test (linux)`, `wasm32 build` and `docs links` jobs. `deny.toml` holds the license allowlist (architecture §17.1); `scripts/check-dependency-graph.sh` is the dependency audit (architecture §2.2, §3.2), which also keeps `idef0-mangle` and `idef0-layout` independent on every target and for dev-dependencies; `scripts/check-doc-links.py` is the repo-wide link checker.
- Workspace foundation (roadmap M0.2): `[workspace.package]`, the verified
  `[workspace.dependencies]` table, workspace lints (`unsafe_code = "forbid"`),
  the pinned toolchain `1.96.0` with the `wasm32-unknown-unknown` target, and MSRV 1.90.
- Determinism lint bans for `idef0-core`, `idef0-mangle` and `idef0-layout` (hash collections,
  clock, environment, ulid/rand/getrandom entropy, `f32`/`f64` transcendental functions), with the
  negative test `scripts/check-determinism-lints.sh`, which requires every ban entry to fire.
  `idef0-wasm` bans `std::time::{Instant, SystemTime}` and refuses a wasm32 build without
  `--cfg=web_sys_unstable_apis`.
- New `idef0-ingest` crate. `idef0-server` is now a library plus a binary. The CLI binary is named `idef0`.
- `/pr-review` Claude Code skill (`.claude/skills/pr-review/`), adapted from the maintainer's review skill to this repo's checks and rules.
- v0.2 specification set, decisions D1–D20, fixtures and the FIPS PUB 183 reference copy (M0.1).
