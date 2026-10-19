#!/usr/bin/env bash
# Dependency audit (roadmap M0.3; docs/architecture.md §2.2 and §3.2):
#   1. The wasm32 graphs of idef0-core, idef0-mangle, idef0-layout and idef0-wasm contain no
#      crate that compiles C or needs a native runtime (cc, tree-sitter*, tokio, mio, ring), and
#      none of the native-only workspace crates (idef0-ingest, idef0-server).
#   2. The wasm32 graphs of idef0-core, idef0-mangle and idef0-layout contain no randomness or
#      random-id crate (getrandom, rand, ulid): NFR-DET-03, D14.
#   3. idef0-mangle and idef0-layout do not depend on each other, on any target and not even
#      as a dev-dependency: tests that need both belong in idef0-server or idef0-cli.
# Build dependencies are included, because a C toolchain enters through build scripts (cc).
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
graph() { # package, target (a triple or "all"), edge kinds -> one crate name per line
  cargo tree --quiet --locked -p "$1" --target "$2" -e "$3" --prefix none --format '{p}' \
    | awk '{print $1}' | sort -u
}
forbid() { # package, target, regex, reason, [edge kinds, default normal,build]
  local g hits
  # A failed `cargo tree` must fail the audit: an empty graph would pass vacuously.
  if ! g="$(graph "$1" "$2" "${5:-normal,build}")" || [ -z "$g" ]; then
    echo "FAIL: could not resolve the dependency graph of $1 ($2)" >&2
    fail=1
    return
  fi
  hits="$(grep -E "$3" <<<"$g" || true)"
  if [ -n "$hits" ]; then
    echo "FAIL: $1 ($2) depends on: $(echo "$hits" | tr '\n' ' ')- $4" >&2
    fail=1
  fi
}

W=wasm32-unknown-unknown
for p in idef0-core idef0-mangle idef0-layout idef0-wasm; do
  forbid "$p" "$W" '^(cc|tree-sitter.*|tokio|mio|ring|idef0-ingest|idef0-server)$' "native code in the wasm32 graph"
done
for p in idef0-core idef0-mangle idef0-layout; do
  forbid "$p" "$W" '^(getrandom|rand|rand_core|ulid)$' "randomness in a deterministic crate (NFR-DET-03)"
done
# Not only wasm32: a native-only [target.'cfg(...)'.dependencies] entry would hide from $W.
forbid idef0-mangle all '^idef0-layout$' "mangle and layout must stay independent" normal,build,dev
forbid idef0-layout all '^idef0-mangle$' "mangle and layout must stay independent" normal,build,dev

if [ "$fail" -ne 0 ]; then exit 1; fi
echo "OK: dependency graph rules hold (wasm32 native-free, deterministic crates randomness-free, mangle/layout independent)"
