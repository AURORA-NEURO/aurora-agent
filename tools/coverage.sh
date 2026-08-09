#!/usr/bin/env bash
# Regenerate the per-section table in docs/COVERAGE.md.
#
# A module counts as covered when its id (NN.MM) appears anywhere under crates/ or docs/. That is a
# weak criterion on purpose: it measures "someone has read this and taken a position", not "this is
# implemented". Do not restate it as the latter.
set -euo pipefail

BLUEPRINT="${BLUEPRINT:-}"
if [[ -z "$BLUEPRINT" ]]; then
  echo "set BLUEPRINT to the extracted distribution root (see docs/ARCHITECTURE.md)" >&2
  exit 2
fi
if [[ ! -d "$BLUEPRINT/43_FIBER_QUERY_COMPILED_EPISTEMIC_CALCULUS" ]]; then
  echo "BLUEPRINT=$BLUEPRINT does not look like the distribution root" >&2
  exit 2
fi

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

( cd "$BLUEPRINT" && ls */[0-9][0-9]_*.md ) \
  | grep -v '00_SECTION_INDEX' \
  | sed -E 's#^([0-9]{2})_[^/]*/([0-9]{2})_.*#\1.\2#' \
  | sort -u > "$work/all"

grep -rhoE '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b' "$repo/crates" "$repo/docs" 2>/dev/null \
  | sort -u > "$work/cited"

comm -12 "$work/all" "$work/cited" > "$work/covered"

total=$(wc -l < "$work/all")
covered=$(wc -l < "$work/covered")

# Programme sections describe no behaviour; counting them would flatter the denominator.
prose="00 01 02 16 17 18 20 21 22 37"
prose_n=0
for s in $prose; do
  prose_n=$(( prose_n + $(grep -c "^$s\." "$work/all" || true) ))
done

printf '%-4s %-50s %5s %5s\n' 'sec' 'section' 'cited' 'total'
( cd "$BLUEPRINT" && ls -d [0-9][0-9]_*/ ) | sed 's#/##' | while read -r dir; do
  n=${dir%%_*}
  t=$(grep -c "^$n\." "$work/all" || true)
  c=$(grep -c "^$n\." "$work/covered" || true)
  mark=''
  case " $prose " in *" $n "*) mark=' (prose)';; esac
  printf '%-4s %-50s %5s %5s%s\n' "$n" "${dir#*_}" "$c" "$t" "$mark"
done

echo
echo "content modules:      $total"
echo "prose modules:        $prose_n"
echo "code-bearing:         $(( total - prose_n ))"
echo "cited:                $covered"
awk -v c="$covered" -v d="$(( total - prose_n ))" 'BEGIN { printf "code-bearing coverage: %.1f%%\n", 100*c/d }'
