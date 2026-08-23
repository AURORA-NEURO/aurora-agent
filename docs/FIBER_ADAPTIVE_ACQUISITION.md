# FIBER adaptive-acquisition contract

`fiber-query/0.5` is the versioned wire boundary for decision-relative adaptive acquisition
planning. It extends the explicit decision contract from `fiber-query/0.3` with a model prior and
caller-declared tests that have not yet been performed:

- `adaptive_acquisition.prior` is normalized against the models named by `decision_loss`;
- every acquisition has a unique ID, scalarized non-negative cost, and one likelihood vector per
  named outcome;
- outcome likelihoods form a complete partition for every model;
- `budget` is charged on every path, and `max_steps` bounds the number of distinct acquisitions;
- the exact planner is capped at 16 acquisitions, 16 steps, and 65,536 evaluated policy states.

The published schema is
[`schemas/fiber-v0.5/query.schema.json`](../schemas/fiber-v0.5/query.schema.json), and the
replay fixture is
[`fixtures/fiber-v0.5/adaptive_acquisition_query.json`](../fixtures/fiber-v0.5/adaptive_acquisition_query.json).

## Minimal contract

```json
{
  "schema_version": "fiber-query/0.5",
  "query_id": "adaptive-review-v1",
  "targets": ["decision_status"],
  "protected_tags": [],
  "decision_time": "2025-01-01T00:00:00Z",
  "budgets": {"max_facts": 64},
  "decision_loss": {
    "actions": ["accept", "defer", "reject"],
    "models": ["m0", "m1"],
    "loss": [[0, 4], [1, 1], [4, 0]],
    "sense": "loss"
  },
  "permitted_actions": ["accept", "defer", "reject"],
  "adaptive_acquisition": {
    "prior": [0.6, 0.4],
    "budget": 2.0,
    "max_steps": 2,
    "acquisitions": [
      {
        "id": "screen",
        "cost": 0.5,
        "outcomes": [
          {"label": "positive", "likelihood": [0.8, 0.2]},
          {"label": "negative", "likelihood": [0.2, 0.8]}
        ]
      }
    ]
  }
}
```

The compiler validates the nested contract before loading the world or running a FIBER pass. A
malformed prior, duplicate acquisition or outcome ID, missing model likelihood, non-partitioning
outcome set, negative/non-finite cost, over-cap horizon, or unknown nested field is a typed
refusal. A cap is never converted into a sampled policy.

## Returned projection

`fiber_compile` places the result at `adaptive_acquisition` in its L0 response. The projection
contains:

- the normalized prior and replayable decision/acquisition inputs;
- expected total objective, expected terminal risk, expected acquisition cost, evaluated-state
  count, and selected depth;
- a named policy tree where each `acquire` node contains all outcome probabilities and posteriors,
  and each branch either stops with an action or selects a different unused acquisition;
- `certificate_binding.query_sha256` and `certificate_binding.certificate_sha256`;
- `execution: "not_started"` and `authorization: "not_granted"`;
- planner provenance and explicit assumptions/limitations.

The tree is exact under the declared model and conditional-independence assumption. It is not a
test order guarantee, a provider request, an assay order, a clinical recommendation, a causal
identification result, or evidence that an outcome was observed. A caller that wants to execute a
selected acquisition must perform its own authorization, provider handoff, observation capture,
and receipt validation. That external result is a new artifact and must not be retroactively
inferred from this plan.

## Objective and boundary

At each policy state, FIBER compares stopping immediately with every affordable unused
acquisition. The selected action minimizes:

```text
expected terminal Bayes risk + expected declared scalar cost
```

The planner is conservative on ties and stops when an acquisition does not improve the objective
by the kernel tolerance. Costs are scalar by design: the caller must document how latency,
compute, specimen, privacy, money, or expert burden were scalarized. A future multi-dimensional
portfolio contract must not be smuggled into this scalar field.

The policy is content-addressed indirectly through the compiled query and certificate. Changing a
prior, likelihood, cost, decision loss, or policy input changes the query identity and therefore
the certificate binding. SDK consumers should retain both digests when persisting a plan.

## Cross-transport parity

- MCP exposes `fiber_compile` and the read-only resource
  `bioprism://schema/fiber-query/0.5`.
- Python exposes `fiber_compile_adaptive_acquisition` on synchronous and asynchronous Workspace
  and HTTP clients, with recursive branch and digest validation.
- TypeScript exposes `fiberCompileAdaptiveAcquisition` and the corresponding policy/result
  interfaces.
- The Rust FIBER tests, MCP protocol tests, Python tests, and TypeScript tests all use the same
  fixture shape and assert that planning remains distinct from execution.

The standalone `epistemic_adaptive_acquisition` route remains useful for callers that do not have
a FIBER world. The 0.5 contract is the integrated path: it binds the policy to a world/query
certificate without making the world evidence an acquisition receipt.
