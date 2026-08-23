# Exact Adaptive Acquisition Policies

`bioprism-epistemic::adaptive_policy` and the
`epistemic_adaptive_acquisition` MCP tool plan a finite-horizon acquisition policy against an
explicit decision problem. The policy is a tree, not a scalar value-of-information summary: after
each declared outcome, the next branch may stop or select a different unused acquisition.

The surface closes the gap between `epistemic_voi` and observed-context tools:

- `epistemic_voi` prices one acquisition or a fixed non-adaptive bundle before any outcome is known.
- `epistemic_adaptive_acquisition` prices branch-dependent sequences under a budget and horizon.
- `epistemic_context_audit` and `epistemic_selection_audit` operate on evidence already observed.

None of these routes executes an assay, contacts a provider, schedules a procedure, authenticates
an observation, or establishes a causal, biological, clinical, or predictive fact.

## Objective

For a belief over explicit models, the planner minimizes

```text
expected terminal Bayes risk + expected declared acquisition cost
```

At a node it compares the stop action with every affordable unused acquisition. For every outcome
of a candidate acquisition it computes the likelihood-weighted posterior and recursively plans the
remaining branch. Costs are charged once when an acquisition is selected; the child objective
contains only downstream costs. The serialized policy therefore exposes:

- `expected_total` — the scalarized objective;
- `expected_terminal_risk` — risk after the selected branch actions;
- `expected_acquisition_cost` — expected declared burden;
- `nodes_evaluated` — exact state evaluations used while comparing alternatives;
- `selected_depth` — the deepest acquisition count on the selected policy;
- `root` — the complete stop/acquire/outcome/posterior tree.

The Rust kernel uses deterministic tie handling: a candidate must improve total objective by more
than the loss epsilon before it replaces stop. A zero-probability branch is retained as a stop
node with the current posterior so the policy remains structurally total without inventing an
impossible update.

## Contract and caps

The exact planner refuses, rather than sampling or silently truncating, when any hard boundary is
exceeded:

| Boundary | Limit |
| --- | ---: |
| Distinct acquisitions | 16 |
| Decisions on one branch | 16 |
| Evaluated policy states | 65,536 |
| Problem actions/models at MCP | 1,000 / 1,000 |
| Outcome labels per acquisition at MCP | 1,000 |
| Serialized MCP input | 20,000,000 bytes |

Every acquisition must have a finite non-negative scalar cost and a complete likelihood partition:
for each model, all outcome likelihoods are finite, non-negative, and sum to one. Acquisition IDs
must be unique, and the kernel re-checks these invariants after serde deserialization. An acquisition
can be used at most once; the policy state is an explicit bit mask.

The exact state cap counts recursive comparisons, including alternatives that do not appear in the
selected tree. It is therefore an honest bound on the proof search, not a bound inferred from the
number of nodes eventually returned.

## Conditional-independence assumption

When an outcome is selected, the planner updates model mass with that acquisition's likelihood.
Across multiple acquisitions it assumes their outcomes are conditionally independent given the
caller-supplied model. Correlated assays, adaptive test interference, treatment effects on later
measurements, missing-not-at-random outcomes, and time-varying models require a richer model and
are not inferred by this route. The assumption is visible in the MCP `limitations` field and in
the Rust module documentation.

## MCP request

The request requires `problem`, `belief`, `acquisitions`, `budget`, and `max_steps`:

```json
{
  "problem": {
    "actions": ["choose-m0", "choose-m1"],
    "models": ["m0", "m1"],
    "loss": [0.0, 1.0, 1.0, 0.0]
  },
  "belief": {"mass": [0.9, 0.1]},
  "acquisitions": [
    {
      "id": "screen",
      "cost": 0.01,
      "outcomes": [
        {"label": "positive", "likelihood": [0.9, 0.2]},
        {"label": "negative", "likelihood": [0.1, 0.8]}
      ]
    },
    {
      "id": "confirm",
      "cost": 0.1,
      "outcomes": [
        {"label": "positive", "likelihood": [0.01, 0.99]},
        {"label": "negative", "likelihood": [0.99, 0.01]}
      ]
    }
  ],
  "budget": 0.11,
  "max_steps": 2
}
```

The schema is `bioprism-mcp/epistemic-adaptive-acquisition/0.1`. An accepted response names each
action and acquisition at every node, includes outcome probabilities and posterior vectors, and
keeps objective components separate. A valid refusal has `ok: false`, `fail_closed: true`, a
`stage`, `refusal`, `guarantees`, and `limitations`; it is domain data rather than a transport
success claim.

## SDKs

Python exposes `EpistemicAdaptiveArgs`, `EpistemicAdaptiveReport`, and
`epistemic_adaptive_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
`AsyncApiClient`. The report parser validates posterior normalization, branch probability sums,
objective reconciliation, action/acquisition references, path-level acquisition uniqueness, and
the returned depth/node caps.

TypeScript exposes `EpistemicAdaptiveArgs`, `EpistemicAdaptiveResult`, the recursive node/outcome
types, and `client.epistemicAdaptiveAcquisition(...)`. The TypeScript layer preserves the full
wire tree for callers that need language-specific validation or policy rendering.

## What this does not mean

An accepted policy is an exact answer only within the supplied model, loss, belief, likelihood,
cost, horizon, and enumeration contract. It does not mean that the highest-value test should be
ordered, that a patient should be treated, that an intervention is identified causally, that the
models are exhaustive, or that a declared likelihood was empirically calibrated. Integrating a
real acquisition executor would require separate authorization, provider identity, scheduling,
observation provenance, cancellation, adverse-event, privacy, and release contracts.
