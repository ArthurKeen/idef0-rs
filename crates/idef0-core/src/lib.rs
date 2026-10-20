//! `idef0-core`: the IDEF0 metamodel, identity, `.idef0` DSL, commands and undo,
//! editor FSM, lints and the model search index.
//!
//! This crate is pure Rust and must build for `wasm32-unknown-unknown`: no file I/O,
//! no threads, no clock, no randomness (D6, `docs/architecture.md` §2.2).
//! Specification: `docs/spec/metamodel.md`.
#![deny(clippy::unwrap_used)]

pub mod id;
pub mod profile;
