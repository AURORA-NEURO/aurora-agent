---
name: fiber-decision-context
description: Compile, layer, compare, and verify FIBER decision contexts with the aurora-agent backend. Use when asked to compile a decision context or world/query pair, judge context-selection strategies against baselines, walk disclosure layers L0 to L4, verify a context certificate, or interpret bioprism CLI envelopes and exit codes.
---

# FIBER decision contexts

FIBER compiles a **world** (typed facts + factors) and a **query** into a
Decision Section plus a **Context Certificate** — a receipt stating exactly
what was omitted and whether it could have changed the decision. Omissions are
reported at every layer: layering hides volume, never the fact of an omission.

## Where things are

- Checkout: `$AURORA_AGENT_ROOT` → `~/aurora-agent` → `~/bioprism`.
- CLI: `<root>/target/release/bioprism(.exe)`; run from the checkout root.
- MCP tools (if the aurora-backend plugin is installed): `fiber_compile`,
  `fiber_refine`, `fiber_explain`, `fiber_verify`, `context_compare`,
  `world_validate` — same Rust implementation as the CLI.
- Reference fixtures: world `fixtures/fiber-v0.1/radiogenomic_world.json`,
  query `fixtures/fiber-v0.1/leakage_query.json`. On this pair the reference
  certificate digest is
  `c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4` — a
  checkable expectation: three independent implementations (CPython, eager
  Rust, indexed store) agree on it byte for byte.

## The workflow

1. **Validate** the world (`world validate --world W`; no `--query` flag).
2. **Explain** the plan (`context explain`) — always surface the passes that
   did NOT run with their verbatim reasons, and the omission manifest.
3. **Compile** (`context compile [--certificate-out C]`). The oracle status
   `invalid` means the oracle found integrity violations in the data (e.g.
   identity/site/temporal/preprocessing leakage) — that is the finding, not a
   failure. Report witnesses verbatim.
4. **Layer** via MCP `fiber_compile` with `layer: L0..L4` — L0 is the smallest
   honest layer (~200 estimated tokens on the reference pair vs ~1,900 full);
   each result carries a content-addressed refinement handle for `fiber_refine`.
   `estimated_tokens.method` self-discloses that it is a heuristic, not a
   tokenizer — quote it when reporting token numbers.
5. **Compare** (`context compare`) — judge strategies by `verdict_preserving`,
   never by `status` alone: a "valid" from a non-preserving strategy is a
   FALSE verdict. Report `cheapest_admissible_strategy` verbatim even when it
   is a baseline (on the reference pair it is `graph-5-hop` — an honest
   negative the project publishes deliberately).
6. **Verify** (`context verify --certificate C`) — certificates verify
   without the engine that produced them.

## CLI envelope + exit matrix

`--json` emits exactly one JSON document on stdout. Exit codes carry exactly
one retry decision each (also in `error.retryability`):

| code | name | decision | code | name | decision |
|---:|---|---|---:|---|---|
| 0 | ok | — | 5 | io | retryable_as_is |
| 1 | assertion_failed | — (a verdict) | 6 | conflict | terminal |
| 2 | usage | terminal | 7 | policy_denied | retryable_after_change |
| 3 | invalid_input | terminal | 8 | indeterminate | retryable_after_change |
| 4 | compile_failed | retryable_after_change | 9 | stale | retryable_as_is |

Codes 0 and 1 report a verdict rather than a failure, so they publish no retry
decision.

## Scale note

`world index --world big.json --store big.bpw` builds a content-addressed
store; on a 1M-fact world compile time drops from ~26.5 s to ~41.6 ms (638×),
with the identical certificate either way.
