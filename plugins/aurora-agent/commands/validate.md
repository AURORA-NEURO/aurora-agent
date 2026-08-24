---
description: Validate a FIBER world file — counts, errors, warnings, diagnostics, and the world digest
argument-hint: "[world path] (defaults to the reference world)"
---

Validate a world file.

1. Resolve the aurora-agent checkout (`$AURORA_AGENT_ROOT` → `~/aurora-agent`
   → `~/bioprism`); CLI at `<root>/target/release/bioprism(.exe)`.
2. Default if `$ARGUMENTS` is empty:
   `fixtures/fiber-v0.1/radiogenomic_world.json`.
3. Run from the checkout root (NOTE: `world validate` takes no `--query`):
   `bioprism --json world validate --world <world>`
4. Report: fact/factor/event counts, error and warning totals, each
   diagnostic verbatim (severity, code, subject, message), and the
   `world_sha256`. Warnings are findings about the world, not tool noise —
   list them all.
