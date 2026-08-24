---
description: Compile a FIBER decision context (world + query) and report the oracle verdict, omission accounting, and certificate digest faithfully
argument-hint: "[world path] [query path] (relative to the aurora-agent root; defaults to the reference fixtures)"
---

Compile a decision context with the aurora-agent FIBER compiler and report the
result faithfully.

1. Resolve the aurora-agent checkout: `$AURORA_AGENT_ROOT` if set, else
   `~/aurora-agent`, else `~/bioprism`. The CLI is
   `<root>/target/release/bioprism` (`.exe` on Windows). If missing, tell the
   user to build it: `cargo build --release --offline -p bioprism-cli`.
2. Parse arguments: `$ARGUMENTS` may name a world and query path (relative to
   the root). Defaults: world `fixtures/fiber-v0.1/radiogenomic_world.json`,
   query `fixtures/fiber-v0.1/leakage_query.json`.
3. Run from the checkout root:
   `bioprism --json context compile --world <world> --query <query>`
4. Interpret the exit code with the retryability matrix: 0 ok · 1
   assertion_failed (a verdict, not an error) · 2 usage/terminal · 3
   invalid_input/terminal · 4 compile_failed/retry-after-change · 5
   io/retry-as-is · 6 conflict/terminal · 7 policy_denied/retry-after-change ·
   8 indeterminate/retry-after-change · 9 stale/retry-as-is. In `--json` mode
   the same decision is `error.retryability`.
5. Report, verbatim from the JSON envelope: the oracle status and witnesses
   (an `invalid` status is a finding about the data — integrity violations —
   not a tool failure), selected vs omitted fact counts,
   `supports_sufficiency_claim`, `protected_closure_satisfied`, and the
   `certificate_sha256`. Never summarize omissions away.
