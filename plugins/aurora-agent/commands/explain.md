---
description: Explain the FIBER compile plan — which passes ran, which did NOT run and why, selection ratios, and the omission manifest
argument-hint: "[world path] [query path] (defaults to the reference fixtures)"
---

Explain the FIBER compile plan for a world and query, faithfully.

1. Resolve the aurora-agent checkout (`$AURORA_AGENT_ROOT` → `~/aurora-agent`
   → `~/bioprism`); CLI at `<root>/target/release/bioprism(.exe)`.
2. Defaults if `$ARGUMENTS` is empty: world
   `fixtures/fiber-v0.1/radiogenomic_world.json`, query
   `fixtures/fiber-v0.1/leakage_query.json`.
3. Run from the checkout root:
   `bioprism --json context explain --world <world> --query <query>`
4. Report ALL of: the passes that ran (name, retained counts, notes), **the
   passes that did NOT run, each with its stated reason, verbatim** — this is
   the honest-labelling core of the output, never omit it — the selection
   ratios, the omission manifest (reason, influence class, count, bound), and
   the sufficiency-claim flag. Exit code 0 = ok; interpret failures with the
   retryability matrix (`error.retryability` in the JSON envelope).
