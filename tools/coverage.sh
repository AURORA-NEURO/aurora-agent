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

# docs/BACKLOG.md enumerates the *uncovered* ids, so counting it would report 100% coverage.
grep -rhoE '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b' "$repo/crates" "$repo/docs" \
  --exclude=BACKLOG.md 2>/dev/null \
  | sort -u > "$work/cited"

comm -12 "$work/all" "$work/cited" > "$work/covered"

total=$(wc -l < "$work/all")
# Programme sections describe no behaviour; counting them would flatter the denominator.
prose="00 01 02 16 17 18 20 21 22 37"
prose_n=0
for s in $prose; do
  prose_n=$(( prose_n + $(grep -c "^$s\." "$work/all" || true) ))
done

# The numerator has to drop them too, and for a long time it did not. A single prose module was
# cited — 21.07, in crates/bundle, for the sentence deferring the signing scheme to an ADR nobody
# wrote — so it sat in `covered` while all twelve of section 21 were removed from the denominator.
# That overstated coverage by one module, and the evidence was already in docs/COVERAGE.md: it said
# 703 of 759 and "the remaining 57" in the same paragraph, where 759 - 703 is 56. tools/backlog.sh
# strips prose from the uncovered list before counting, which is why its figure was the right one.
cp "$work/covered" "$work/covered.all"
for s in $prose; do
  grep -v "^$s\." "$work/covered" > "$work/tmp" || true
  mv "$work/tmp" "$work/covered"
done
covered=$(wc -l < "$work/covered")

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
