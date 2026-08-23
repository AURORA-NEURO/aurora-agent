#!/usr/bin/env python3
"""Mirror the workspace skills into the aurora-workspace plugin.

`.agents/skills/` is the source of truth (AGENTS.md's skill table points there).
The Claude Code plugin `plugins/aurora-workspace/` ships a mirror so the same
skills are installable from any project. Run this after editing any skill:

    python tools/sync_plugin_skills.py

The mirror gets a banner naming its origin, and known-stale figures are
corrected in the mirror only (the source is fixed separately when its owners
choose to).
"""

from pathlib import Path
import sys

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / ".agents" / "skills"

# skill name -> (destination plugin, scope-prefix applied to the description)
MIRRORED = {
    "add-module": ("aurora-workspace", True),
    "check-parity": ("aurora-workspace", True),
    "verify-crate": ("aurora-workspace", True),
    "classify-blueprint-modules": ("aurora-workspace", True),
    "measure-section-boilerplate": ("aurora-workspace", True),
    "keep-a-claim-honest": ("aurora-honesty", False),
    "prove-a-scanner-fires": ("aurora-honesty", False),
}

BANNER = (
    "<!-- Mirrored from .agents/skills/{name}/SKILL.md by tools/sync_plugin_skills.py.\n"
    "     Edit the source and re-run the sync; do not edit this copy. -->\n"
)

PORTABLE_NOTE = (
    "> Note: the crate paths, file:line anchors, and case studies below are\n"
    "> illustrations from the aurora-agent workspace where this pattern was\n"
    "> discovered. The pattern itself applies to any codebase.\n"
)

SCOPE_PREFIX = "(aurora-agent workspace only) "

CORRECTIONS = {
    # (skill, stale text, corrected text) — applied to the mirror only.
    ("verify-crate", "`ls crates | wc -l` is 77", "`ls crates | wc -l` is 79"),
    ("verify-crate", "77 crates", "79 crates"),
}


def sync() -> int:
    if not SOURCE.is_dir():
        print(f"source skills directory missing: {SOURCE}", file=sys.stderr)
        return 2
    for name, (plugin, scoped) in MIRRORED.items():
        src = SOURCE / name / "SKILL.md"
        if not src.is_file():
            print(f"missing source skill: {src}", file=sys.stderr)
            return 2
        text = src.read_text(encoding="utf-8")
        for skill, stale, fixed in CORRECTIONS:
            if skill == name and stale in text:
                text = text.replace(stale, fixed)
        if scoped:
            # Scope the trigger description so the skill does not fire outside this repo.
            marker = "description: "
            idx = text.find(marker)
            if idx >= 0 and not text[idx + len(marker):].startswith(SCOPE_PREFIX.rstrip()):
                text = text[: idx + len(marker)] + SCOPE_PREFIX + text[idx + len(marker):]
        out_dir = REPO / "plugins" / plugin / "skills" / name
        out_dir.mkdir(parents=True, exist_ok=True)
        body_start = text.find("---", 3)
        insert_at = text.find("\n", body_start) + 1 if body_start >= 0 else 0
        extra = BANNER.format(name=name) + ("" if scoped else PORTABLE_NOTE)
        mirrored = text[:insert_at] + "\n" + extra + text[insert_at:]
        (out_dir / "SKILL.md").write_text(mirrored, encoding="utf-8", newline="\n")
        print(f"synced {name} -> {plugin}")
    return 0


if __name__ == "__main__":
    raise SystemExit(sync())
