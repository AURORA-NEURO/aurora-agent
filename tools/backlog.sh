#!/usr/bin/env bash
# Regenerate docs/BACKLOG.md — the enumerated form of what docs/COVERAGE.md reports as a percentage.
#
# A module leaves the backlog when a crate or design note cites its id. That is the same weak
# criterion coverage uses, so this file tracks attention rather than completeness. Do not restate it
# as the latter.
set -euo pipefail

BLUEPRINT="${BLUEPRINT:-}"
if [[ -z "$BLUEPRINT" || ! -d "$BLUEPRINT/43_FIBER_QUERY_COMPILED_EPISTEMIC_CALCULUS" ]]; then
  echo "set BLUEPRINT to the extracted distribution root (see docs/ARCHITECTURE.md)" >&2
  exit 2
fi

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

( cd "$BLUEPRINT" && ls */[0-9][0-9]_*.md ) \
  | grep -v '00_SECTION_INDEX' \
  | sed -E 's#^([0-9]{2})_[^/]*/([0-9]{2})_.*#\1.\2#' \
  | sort -u > "$work/all"

# BACKLOG.md lists the uncovered ids by definition, so scanning it would report every module as
# cited and empty the backlog on the second run. It did exactly that once.
grep -rhoE '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b' "$repo/crates" "$repo/docs" \
  --exclude=BACKLOG.md 2>/dev/null \
  | sort -u > "$work/cited"

comm -23 "$work/all" "$work/cited" > "$work/uncovered"

# Programme sections specify no behaviour; they are excluded here for the same reason they are
# excluded from the coverage denominator.
for s in 00 01 02 16 17 18 20 21 22 37; do
  grep -v "^$s\." "$work/uncovered" > "$work/tmp" || true
  mv "$work/tmp" "$work/uncovered"
done

python - "$BLUEPRINT" "$work/uncovered" "$repo/docs/BACKLOG.md" <<'PY'
import sys, pathlib, collections

blueprint, uncovered_path, out_path = (pathlib.Path(a) for a in sys.argv[1:4])
uncovered = [l.strip() for l in uncovered_path.read_text().splitlines() if l.strip()]

by_section = collections.defaultdict(list)
for module in uncovered:
    section, index = module.split('.')
    by_section[section].append(index)

directories = {d.name.split('_')[0]: d for d in blueprint.iterdir()
               if d.is_dir() and d.name[:2].isdigit()}

lines = [
    "# Remaining backlog\n",
    f"{len(uncovered)} code-bearing blueprint modules are not yet cited by any crate or design note,",
    f"across {len(by_section)} sections. This is the enumerated form of `docs/COVERAGE.md`'s",
    "percentage: a percentage says how far there is to go, a list says what is actually left.\n",
    "Regenerate with `tools/backlog.sh`. Programme sections are excluded for the same reason they",
    "are excluded from the coverage denominator — they specify no behaviour.\n",
    "A module leaves this list when a crate cites it, which is the same weak criterion coverage",
    "uses. This file tracks *attention*, not completeness.\n",
]

for section in sorted(by_section, key=lambda s: (-len(by_section[s]), s)):
    directory = directories.get(section)
    if directory is None:
        continue
    title = directory.name.split('_', 1)[1].replace('_', ' ').title()
    lines.append(f"\n## §{section} {title} — {len(by_section[section])} uncovered\n")
    for index in sorted(by_section[section]):
        module_file = next((f for f in directory.glob(f"{index}_*.md")), None)
        module_title = (module_file.name.split('_', 1)[1][:-3].replace('_', ' ').title()
                        if module_file else "?")
        lines.append(f"- `{section}.{index}` {module_title}")

out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"{out_path}: {len(uncovered)} modules across {len(by_section)} sections")
PY
