//! Each item below must be rejected by the strict clippy.toml of the wasm-safe crates.
//! scripts/check-determinism-lints.sh requires every `path` in that file to be reported here,
//! so a new ban needs a new probe. Do not "fix" this file: it exists to fail.

pub fn probe_collections() -> (std::collections::HashMap<u32, u32>, std::collections::HashSet<u32>) {
    (std::collections::HashMap::new(), std::collections::HashSet::new())
}

pub fn probe_time() -> (std::time::Instant, std::time::SystemTime) {
    (std::time::Instant::now(), std::time::SystemTime::now())
}

pub fn probe_env() {
    let _ = std::env::var("HOME");
    let _ = std::env::var_os("HOME");
    let _ = std::env::vars();
    let _ = std::env::vars_os();
}

pub fn probe_ulid() {
    let mut generator = ulid::Generator::new();
    let _ = ulid::Ulid::generate();
    let _ = ulid::Ulid::from_datetime(std::time::UNIX_EPOCH);
    let _ = generator.generate();
    let _ = generator.generate_from_datetime(std::time::UNIX_EPOCH);
}

pub fn probe_getrandom() {
    let mut buf = [0u8; 4];
    let mut uninit = [std::mem::MaybeUninit::<u8>::uninit(); 4];
    let _ = getrandom::fill(&mut buf);
    let _ = getrandom::fill_uninit(&mut uninit);
    let _ = getrandom::u32();
    let _ = getrandom::u64();
    let _rng: getrandom::SysRng = getrandom::SysRng;
}

pub fn probe_rand() {
    let mut buf = [0u8; 4];
    let _rng: rand::rngs::ThreadRng = rand::rng();
    let _ = rand::make_rng::<rand::rngs::StdRng>();
    let _: u8 = rand::random();
    let _ = rand::random_iter::<u8>();
    let _ = rand::random_range(0..10u8);
    let _ = rand::random_bool(0.5);
    let _ = rand::random_ratio(1, 2);
    rand::fill(&mut buf);
}

pub fn probe_float(x: f32, y: f64) {
    let _ = (x.sin(), x.cos(), x.tan(), x.sin_cos(), x.asin(), x.acos(), x.atan(), x.atan2(x));
    let _ = (y.sin(), y.cos(), y.tan(), y.sin_cos(), y.asin(), y.acos(), y.atan(), y.atan2(y));
    let _ = (x.sinh(), x.cosh(), x.tanh(), x.asinh(), x.acosh(), x.atanh());
    let _ = (y.sinh(), y.cosh(), y.tanh(), y.asinh(), y.acosh(), y.atanh());
    let _ = (x.exp(), x.exp2(), x.exp_m1(), x.ln(), x.ln_1p(), x.log(x), x.log2(), x.log10());
    let _ = (y.exp(), y.exp2(), y.exp_m1(), y.ln(), y.ln_1p(), y.log(y), y.log2(), y.log10());
    let _ = (x.powf(x), x.powi(2), x.cbrt(), x.hypot(x));
    let _ = (y.powf(y), y.powi(2), y.cbrt(), y.hypot(y));
}
