#!/usr/bin/env bash
# Verifies the determinism lint configuration (roadmap M0.2; docs/architecture.md §2.2-2.3):
#   1. the strict clippy.toml is identical in idef0-core, idef0-mangle and idef0-layout;
#   2. clippy, using that exact file, rejects HashMap, Instant::now, env::var and f64::sin.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
strict="$root/crates/idef0-core/clippy.toml"

for c in idef0-mangle idef0-layout; do
  if ! cmp -s "$strict" "$root/crates/$c/clippy.toml"; then
    echo "FAIL: crates/$c/clippy.toml differs from crates/idef0-core/clippy.toml" >&2
    exit 1
  fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -R "$root/scripts/determinism-probe/." "$work/"
cp "$strict" "$work/clippy.toml"
cp "$root/rust-toolchain.toml" "$work/"

set +e
out="$(cd "$work" && cargo clippy --quiet --target-dir "$work/target" -- -D warnings 2>&1)"
status=$?
set -e

if [ "$status" -eq 0 ]; then
  echo "FAIL: clippy accepted the determinism probe; the bans are not effective" >&2
  exit 1
fi

missing=0
for expect in \
  "disallowed type \`std::collections::HashMap\`" \
  "disallowed method \`std::time::Instant::now\`" \
  "disallowed method \`std::env::var\`" \
  "disallowed method \`f64::sin\`"; do
  if ! grep -qF "$expect" <<<"$out"; then
    echo "FAIL: expected clippy to report: $expect" >&2
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  echo "--- clippy output ---" >&2
  echo "$out" >&2
  exit 1
fi
echo "OK: determinism lints reject all probes (HashMap, Instant::now, env::var, f64::sin)"
