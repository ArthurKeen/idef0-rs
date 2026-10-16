#!/usr/bin/env bash
# Verifies the determinism lint configuration (roadmap M0.2; docs/architecture.md §2.2-2.3):
#   1. the strict clippy.toml is identical in idef0-core, idef0-mangle and idef0-layout;
#   2. clippy, using that exact file, reports every `path` entry in it against
#      scripts/determinism-probe/, and no entry is unresolvable.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
strict="$root/crates/idef0-core/clippy.toml"

for c in idef0-mangle idef0-layout; do
  if ! cmp -s "$strict" "$root/crates/$c/clippy.toml"; then
    echo "FAIL: crates/$c/clippy.toml differs from crates/idef0-core/clippy.toml" >&2
    exit 1
  fi
done

paths=()
while IFS= read -r p; do paths+=("$p"); done < <(grep -oE 'path = "[^"]+"' "$strict" | sed -E 's/^path = "(.*)"$/\1/')
if [ "${#paths[@]}" -eq 0 ]; then
  echo "FAIL: no disallowed paths found in $strict" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -R "$root/scripts/determinism-probe/." "$work/"
cp "$strict" "$work/clippy.toml"
cp "$root/rust-toolchain.toml" "$work/"

set +e
out="$(cd "$work" && cargo clippy --quiet --locked --target-dir "$work/target" -- -D warnings 2>&1)"
status=$?
set -e

if [ "$status" -eq 0 ]; then
  echo "FAIL: clippy accepted the determinism probe; the bans are not effective" >&2
  exit 1
fi

missing=0
if grep -q "does not refer to" <<<"$out"; then
  echo "FAIL: clippy.toml has entries that do not resolve:" >&2
  grep "does not refer to" <<<"$out" >&2
  missing=1
fi
for p in "${paths[@]}"; do
  if ! grep -qF "disallowed type \`$p\`" <<<"$out" && ! grep -qF "disallowed method \`$p\`" <<<"$out"; then
    echo "FAIL: expected clippy to report: $p" >&2
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  echo "--- clippy output ---" >&2
  echo "$out" >&2
  exit 1
fi
echo "OK: determinism lints reject all ${#paths[@]} probes"
