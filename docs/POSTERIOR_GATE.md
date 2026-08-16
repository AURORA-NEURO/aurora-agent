# Posterior gate contract

`posterior_gate` is the evaluation engine's evidence-preserving boundary for turning scored
observations into capability-level release evidence. It has three deliberately separate outputs:

1. `capabilities` is the primary vector. Each capability carries clustered pass rate, partial
   credit, outcome rate, unknown share, effective sample size, ICC, vetoes, disputes, abstentions,
   optimistic weak-evidence signals, and weakest evidence tier.
2. `gate` is an optional scalar projection. It is present only when the caller supplies a named
   `ReleaseGate` with a non-empty rationale and explicit `CoverageFloor` for every capability it
   wants to collapse. A failed floor, veto, missing capability, or invalid gate is a typed,
   fail-closed refusal.
3. `comparison` is an optional capability-wise partial order against a second observation set. It
   returns `dominates`, `dominated_by`, `equivalent`, or `incomparable`; thin or one-sided
   capabilities remain `uncertain` rather than becoming ties.

The Rust `bioprism-evalengine` crate remains authoritative for aggregation, clustering, gate
arithmetic, and error wording. MCP adds the stable boundary identifier
`bioprism-mcp/posterior-gate/0.1`; the Python and TypeScript SDKs validate and project the response
without recalculating statistics.

## Input boundary

The request requires serialized `bioprism-evalengine::Observation` values and accepts at most
10,000 observations in each set. An observation contains a capability name, a parent task, a
serialized `ScoredResult`, and optional provenance. Missing provenance is counted in
`unprovenanced_observations`; it is not silently repaired or treated as evidence quality.

Optional controls are serialized `CreditPolicy`, `ReleaseGate`, a second observation set,
non-negative finite `tolerance`, and non-negative finite `min_effective`. The server rejects
invalid serialized values and safety-limit violations rather than truncating them.

## Why the vector comes first

The capability report exposes three separate clustered estimates:

- `pass_rate`: only an unqualified, evidence-supported pass counts;
- `outcome_rate`: whether the intended outcome was correct for any reason; and
- `credit`: policy-bounded partial credit.

The difference between outcome and pass is the unsupported-pass gap. It is retained because “right
answer, wrong reason” is not equivalent to either a full pass or a failure. Every estimate also
publishes the naive instance mean next to the parent-balanced mean. Consumers should read
`effective_sample_size`, `clusters`, `icc`, and `unknown_fraction` before interpreting a mean.

The implementation reports a clustered point estimate. It is not a fitted Bayesian or frequentist
probability distribution, and its intervals are not implied by this tool.

## Scalar release gates

The scalar wrapper has the following shape:

```json
{
  "ok": true,
  "value": {
    "gate": "release-a",
    "value": 0.91,
    "formula": "weighted mean of per-capability full-pass rates, cluster-balanced",
    "rationale": "named release decision",
    "terms": [["planning", 0.9, 1.0]],
    "sensitivity": [["planning", 0.9]],
    "weakest_tier": "execution",
    "min_effective_sample": 12.0
  }
}
```

The `terms` and `sensitivity` arrays make the number recomputable and expose whether one
capability carries the gate. A scalar is refused when any declared floor is unmet, a veto is
outstanding, a capability is unobserved, the unknown share is too high, the weakest tier is too
weak, or the gate has no usable coverage. The refusal includes `fail_closed: true` and never
substitutes zero, a tie, or an unrequested capability.

When no gate is supplied, `gate` is `null`. This is not a failure: it means the caller requested
the vector only.

## Comparison semantics

Comparison never collapses either side. It compares shared pass-rate means within `tolerance` and
requires each side's effective sample to meet `min_effective`. Missing or thin capabilities enter
`uncertain`. If one side is better on one capability and worse on another, the result is
`incomparable` even if a weighted average would be convenient.

## SDK projections

Python exposes `PosteriorGateArgs` and `posterior_gate_report(...)`, plus raw and typed methods on
`Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`:

```python
from prism_sdk import PosteriorGateArgs, Workspace, posterior_gate_report

request = PosteriorGateArgs(
    observations=[observation_a, observation_b],
    gate=release_gate,
    other_observations=reference_observations,
    tolerance=0.02,
    min_effective=4.0,
)
report = Workspace(client).posterior_gate_report(request)
if not report.ok:
    print(report.stage, report.refusal, report.fail_closed)
elif report.release_is_eligible:
    print(report.gate.value.value, report.gate.value.largest_sensitivity)
```

The typed Python report preserves refusal responses as `PosteriorGateReport` values, while MCP
transport errors remain transport errors. `report.capabilities` is keyed by capability name;
`report.has_provenance_gaps`, `report.has_outstanding_veto`, `report.release_is_eligible`, and
`report.comparison_is_incomparable` are convenience predicates, not replacement claims.

TypeScript exposes the matching `PosteriorGateArgs`, `PosteriorGateResult`, and
`ApiClient.posteriorGate(...)`. The result union distinguishes successful vector reports from
`credit_policy`, `posterior`, and `comparison_posterior` refusals. The nested gate union preserves
the difference between no scalar requested, an eligible scalar, and a fail-closed scalar refusal.

## Deliberate limitations

The tool does not execute evaluators, authenticate provenance, fit distributions, estimate
calibration curves, cluster failure-atlas families, model cost or latency, or resolve human
disputes. It is an evidence projection and release-gate contract. Those omitted capabilities stay
visible in `limitations` so a caller cannot mistake a clean vector or scalar for a complete
scientific or operational qualification.
