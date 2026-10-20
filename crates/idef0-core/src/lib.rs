//! `idef0-core`: the IDEF0 metamodel, identity, `.idef0` DSL, commands and undo,
//! editor FSM, lints and the model search index.
//!
//! This crate is pure Rust and must build for `wasm32-unknown-unknown`: no file I/O,
//! no threads, no clock, no randomness (D6, `docs/architecture.md` §2.2).
//! Specification: `docs/spec/metamodel.md`.
#![deny(clippy::unwrap_used)]

/// Serialize a type through its `Display` and deserialize it through its `FromStr`, so
/// identifiers appear in JSON as their canonical strings.
macro_rules! serde_via_str {
    ($($t:ty),* $(,)?) => {$(
        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <String as serde::Deserialize>::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    )*};
}

pub mod diag;
pub mod id;
pub mod model;
pub mod profile;
pub mod text;
pub mod typecheck;
