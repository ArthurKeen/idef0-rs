//! `idef0-wasm`: the Leptos (CSR) web application, built with Trunk (roadmap M7.1).
//!
//! The UI is thin (architecture P4): it holds the two sources of truth, [`doc::Doc`] and
//! [`doc::EditorState`], as signals and draws Scenes that `idef0-layout` computes. It contains
//! no model, layout or validation logic. Specification: `docs/spec/editor-ux.md`.

// The flag comes from `[target.wasm32-unknown-unknown].rustflags` in .cargo/config.toml. Cargo ignores
// that table when RUSTFLAGS or CARGO_ENCODED_RUSTFLAGS is set, so fail loudly instead of silently
// losing the web-sys File System Access API bindings (docs/architecture.md §13.1).
#[cfg(all(target_arch = "wasm32", not(web_sys_unstable_apis)))]
compile_error!(
    "idef0-wasm needs --cfg=web_sys_unstable_apis: unset RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS, or add the flag to them"
);

pub mod app;
pub mod doc;
