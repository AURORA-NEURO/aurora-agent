#!/usr/bin/env bash
# Emit the crate inventory and the workspace test count, for the Status section of README.md.
#
# The README used to carry a hand-maintained table. It drifted to claiming twenty-three crates and
# 820 tests against a workspace of seventy-seven and roughly six thousand, which is the same
# hand-copy drift `crates/devx`'s exit-code audit exists to catch — so the table is generated now
# and the README says which commit it was generated at.
#
# Usage:
#   tools/status.sh            # inventory only, safe while agents are editing
#   tools/status.sh --tests    # also runs the workspace suite, which takes minutes
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

want_tests=0
[[ "${1:-}" == "--tests" ]] && want_tests=1

printf '| Crate | Blueprint | What it does |\n|---|---|---|\n'
for dir in crates/*/; do
  name="$(basename "$dir")"
  # crates/cli is a binary with no lib.rs. Skipping crates without one silently dropped it from
  # the table, which is the kind of omission this workspace refuses everywhere else.
  lib="$dir/src/lib.rs"
  [[ -f "$lib" ]] || lib="$dir/src/main.rs"
  [[ -f "$lib" ]] || continue

  # The blueprint sections a crate claims, taken from the ids it cites in its own source. This is
  # the same token rule tools/coverage.sh uses, reduced to distinct sections.
  sections="$(grep -rhoE '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b' "$dir/src" 2>/dev/null \
    | cut -d. -f1 | sort -u | paste -sd, - || true)"
  [[ -n "$sections" ]] || sections='—'

  # The one-line description from the package manifest, which every crate carries.
  desc="$(grep -m1 '^description = ' "$dir/Cargo.toml" 2>/dev/null | sed 's/^description = "//; s/"$//')"
  [[ -n "$desc" ]] || desc="$(grep -m1 '^//!' "$lib" | sed 's|^//! *||')"

  printf '| [`bioprism-%s`](crates/%s) | %s | %s |\n' "$name" "$name" "$sections" "$desc"
done

echo
echo "crates: $(find crates -mindepth 1 -maxdepth 1 -type d | wc -l)"
echo "lines:  $(find crates -name '*.rs' -print0 | xargs -0 cat | wc -l)"

if [[ "$want_tests" == 1 ]]; then
  # A blocked test binary reports `error: test failed` and cargo continues, so a naive sum silently
  # loses everything after it. Relink first; see .agents/skills/verify-crate/SKILL.md.
  find crates -name '*.rs' -path '*/tests/*' -exec touch {} +
  test_log="$(mktemp)"
  trap 'rm -f "$test_log"' EXIT
  if cargo test --workspace --offline --no-fail-fast >"$test_log" 2>&1; then
    cargo_status=0
  else
    cargo_status=$?
  fi
  cat "$test_log"
  # Cargo test binaries say "never executed"; rustdoc doctests report the policy denial inline.
  # Count both once per blocked process so the successful-test sum is never mistaken for complete.
  blocked=$(grep -Eic 'never executed|couldn.t run the test:.*application control policy.*os error 4551' "$test_log" || true)
  total=$(awk '/^test result: ok/ {s+=$4} END {print s+0}' "$test_log")
  echo "tests:  $total"
  echo "blocked binaries: $blocked"
  echo "cargo exit code: $cargo_status"
  if [[ "$blocked" != 0 ]]; then
    echo "WARNING: this test count is incomplete because one or more binaries never executed"
    grep -i -B2 -A2 -E 'never executed|couldn.t run the test:.*application control policy.*os error 4551' "$test_log" || true
  fi
  exit "$cargo_status"
fi
