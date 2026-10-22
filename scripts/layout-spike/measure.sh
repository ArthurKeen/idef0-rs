#!/usr/bin/env bash
# M4.0 spike measurements (ADR 0002): native timings, callgrind instruction counts (Linux with
# valgrind) and wasm32-wasip1 timings under Node. Run from the repo root.
set -euo pipefail
cd "$(dirname "$0")"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
cargo build --quiet --locked --release
cargo build --quiet --locked --release --target wasm32-wasip1
bin="$CARGO_TARGET_DIR/release/layout-spike"
echo "## machine"; uname -srm; (grep -m1 "model name" /proc/cpuinfo || sysctl -n machdep.cpu.brand_string) 2>/dev/null || true
echo "## native"; "$bin" 300
if command -v valgrind >/dev/null; then
  echo "## instructions (callgrind Ir, op minus noop)"
  ir() { valgrind --tool=callgrind --callgrind-out-file=/dev/null "$bin" once "$1" 2>&1 | sed -n 's/.*Collected : \([0-9]*\).*/\1/p'; }
  base=$(ir noop)
  echo "op,instructions"
  for op in parse place layout layout-dogfood layout-worst arrange1 render; do
    echo "$op,$(( $(ir "$op") - base ))"
  done
fi
echo "## wasm32 (node $(node --version), WASI)"
node --no-warnings run-wasi.mjs "$CARGO_TARGET_DIR/wasm32-wasip1/release/layout-spike.wasm" 300
