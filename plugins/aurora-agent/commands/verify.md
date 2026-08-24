---
description: Verify a FIBER context-certificate digest (recompute and check the canonical hash)
argument-hint: "<certificate path relative to the aurora-agent root>"
---

Verify a context certificate.

1. Resolve the aurora-agent checkout (`$AURORA_AGENT_ROOT` → `~/aurora-agent`
   → `~/bioprism`); CLI at `<root>/target/release/bioprism(.exe)`.
2. `$ARGUMENTS` must name a certificate JSON path (relative to the root). If
   none was given, ask which certificate to verify — or compile one first with
   `context compile --certificate-out <path>`.
3. Run from the checkout root:
   `bioprism --json context verify --certificate <path>`
4. Report the `verification` string verbatim ("digest verifies" or the failure
   text) plus the schema version. Exit 0 = the digest verifies; exit 1 =
   assertion_failed is the honest "it does not verify" verdict, not a tool
   error. Certificates are verifiable without the engine that produced them —
   that independence is the design point worth mentioning when relevant.
