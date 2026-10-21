//! `idef0-layout`: FIPS 183 auto layout (staircase placement, channel routing,
//! fillets, bundles, bridges, labels), Auto-Arrange, the `Scene`, SVG and ASCII
//! renderers and the geometry checker.
//!
//! Pure Rust and wasm-safe (D6). Specification: `docs/spec/layout.md`.
#![deny(clippy::unwrap_used)]

pub mod metrics;
pub mod place;
pub mod ports;
pub mod size;
pub mod spike;
pub mod text;
