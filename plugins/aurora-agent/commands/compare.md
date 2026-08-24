---
description: Compare the FIBER compiler against equal-engineering baselines (graph walks, lexical retrieval) and report which preserve the reference verdict
argument-hint: "[world path] [query path] (defaults to the reference fixtures)"
---

Run the baseline comparison and report it with its honesty semantics intact.

1. Resolve the aurora-agent checkout (`$AURORA_AGENT_ROOT` → `~/aurora-agent`
   → `~/bioprism`); CLI at `<root>/target/release/bioprism(.exe)`.
2. Defaults if `$ARGUMENTS` is empty: the reference fixture pair
   (`fixtures/fiber-v0.1/radiogenomic_world.json`,
   `fixtures/fiber-v0.1/leakage_query.json`).
3. Run from the checkout root:
   `bioprism --json context compare --world <world> --query <query>`
4. Report per strategy: facts exposed, fraction of world, status,
   **verdict_preserving**, admissible, protected recall, missing witnesses.
   CRITICAL: a `"valid"` status from a strategy whose `verdict_preserving` is
   false is a FALSE verdict — judge strategies by verdict preservation, never
   by the status column alone. Name the `cheapest_admissible_strategy`
   verbatim, including when it is a baseline rather than FIBER — that honest
   headline is the point of the tool.
