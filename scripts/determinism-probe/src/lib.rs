//! Each function below must be rejected by the strict clippy.toml of the wasm-safe crates.
//! Do not "fix" this file: it exists to fail.

pub fn probe_hashmap() -> std::collections::HashMap<u32, u32> {
    std::collections::HashMap::new()
}

pub fn probe_instant() -> std::time::Instant {
    std::time::Instant::now()
}

pub fn probe_env() -> Option<String> {
    std::env::var("HOME").ok()
}

pub fn probe_trig(x: f64) -> f64 {
    x.sin()
}
