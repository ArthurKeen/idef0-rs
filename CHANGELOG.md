# Changelog

All notable changes to idef0-rs are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versioning follows the
policy in [docs/architecture.md §16](docs/architecture.md#16-release-and-versioning-sr-54).

## [Unreleased]

### Added
- Workspace foundation (roadmap M0.2): `[workspace.package]`, the verified
  `[workspace.dependencies]` table, workspace lints (`unsafe_code = "forbid"`),
  the pinned toolchain `1.96.0` with the `wasm32-unknown-unknown` target, and MSRV 1.90.
- Determinism lint bans for `idef0-core`, `idef0-mangle` and `idef0-layout` (hash collections,
  clock, environment, ulid/rand/getrandom entropy, `f32`/`f64` transcendental functions), with the
  negative test `scripts/check-determinism-lints.sh`, which requires every ban entry to fire.
  `idef0-wasm` bans `std::time::{Instant, SystemTime}` and refuses a wasm32 build without
  `--cfg=web_sys_unstable_apis`.
- New `idef0-ingest` crate. `idef0-server` is now a library plus a binary. The CLI binary is named `idef0`.
- v0.2 specification set, decisions D1–D20, fixtures and the FIPS PUB 183 reference copy (M0.1).
