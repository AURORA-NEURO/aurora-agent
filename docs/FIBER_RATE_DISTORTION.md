# FIBER rate-distortion contract

`fiber-query/0.4` closes the remaining wire gap for the observed-context half of blueprint 43.12.
It is the first FIBER query form that can make an executable context-minimality claim, because it
binds every quantity the kernel needs instead of deriving one from a default:

- the decision loss/utility table and permitted action boundary from `fiber-query/0.3`;
- a normalized prior over the declared decision models;
- an ordered pool of already-observed evidence, each with a scalar retention cost and one
  likelihood per model;
- a compatibility floor for the surviving model set; and
- the top-level `distortion_tolerance` used by identification and minimal sufficiency.

The published schema is [`schemas/fiber-v0.4/query.schema.json`](../schemas/fiber-v0.4/query.schema.json),
and the executable reference is
[`fixtures/fiber-v0.4/rate_distortion_query.json`](../fixtures/fiber-v0.4/rate_distortion_query.json).

## Wire shape

```json
{
  "schema_version": "fiber-query/0.4",
  "query_id": "context-audit-v4",
  "targets": ["split_integrity_status"],
  "protected_tags": [],
  "decision_time": "2025-01-01T00:00:00Z",
  "budgets": { "max_facts": 64 },
  "distortion_tolerance": 0.25,
  "decision_loss": {
    "actions": ["accept", "defer"],
    "models": ["m-a", "m-b"],
    "loss": [[0.0, 4.0], [2.0, 1.0]],
    "sense": "loss"
  },
  "permitted_actions": ["accept", "defer"],
  "rate_distortion": {
    "criterion": "bayes_regret",
    "compatibility_floor": 0.05,
    "prior": [0.6, 0.4],
    "evidence_pool": {
      "items": [
        { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.2] }
      ]
    }
  }
}
```

The evidence is observed, not an acquisition. Retaining an item conditions the prior by
multiplying each model mass by that item's likelihood and renormalizing. This is context
compression; it is not value-of-information pricing and it does not execute a test.

The evidence pool is ordered because subset indexes and tie-breaking are part of deterministic
replay. The exhaustive compiler refuses more than 16 evidence items: `2^16` is the hard kernel
bound, and a sampled frontier is an upper bound rather than the minimum claim the type promises.
Identifiers are bounded at 256 bytes, decision actions/models at 1,000 entries, and every numeric
input must be finite, non-negative where it represents mass/likelihood/cost, and shape-compatible
with the decision model list.

## What FIBER executes

For every valid 0.4 query the compiler runs these distinct layers:

1. **Decision identification.** Full observed evidence is folded into the prior. The result says
   whether compatible models agree on an action, disagree within tolerance, or remain
   non-identified.
2. **Exhaustive frontier.** Every evidence subset is evaluated. Rate is summed retention cost;
   distortion is excess decision loss relative to acting with all evidence under the selected
   Bayes-regret or minimax-regret criterion. Only Pareto points are retained.
3. **Minimal sufficiency.** The cheapest context within tolerance is returned, or the result is an
   explicit abstention distinguishing non-identification from an unattained tolerance.

The result remains a full `RateDistortionTrace` in Rust. `fiber_compile` projects it at L0 as
`bioprism-mcp/epistemic-context-audit/0.2`, including identification, sufficiency, the complete
frontier, the query/certificate binding, guarantees, and limitations. `fiber_explain` exposes the
same trace beside pass receipts and deferred passes.

## Identity and SDK parity

The raw query already participates in `source_hashes.query_sha256`, so changing a prior,
likelihood, cost, criterion, floor, tolerance, loss, or evidence order changes the certificate
identity even when the selected world facts do not change. MCP, the Python SDK and the TypeScript
SDK all preserve the same schema label and binding digests. Python additionally validates the
bounded frontier/evidence counts before exposing the typed projection through
`fiber_compile_rate_distortion`; TypeScript exposes the corresponding
`fiberCompileRateDistortion` call and result type.

## Non-claims

This pass does not establish causal identification, biological mechanism, clinical safety,
predictive accuracy, transportability, or scientific equivalence. The prior, loss, likelihoods and
scalarized costs are caller-declared modelling inputs. A `non_identified` result is an honest
statement about this decision contract and model set, not a diagnosis or an assertion that the
world is unknowable. Adaptive acquisition policies, multi-objective cost vectors and evidence
generation remain separate capabilities.
