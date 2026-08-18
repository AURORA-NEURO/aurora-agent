# Multidimensional acquisition costs

The scalar `cost` on the original adaptive wire contract remains supported for compatibility. The
epistemic kernel now also provides an exact vector-budget planner in `bioprism_epistemic::cost`.
This closes the common failure mode where a caller assigns a cheap scalar to an action that is
actually impossible because it exceeds a latency, privacy, specimen, or expert-time limit.

## Dimensions

Every `CostVector` uses this stable order and preserves each value on the wire:

| Dimension | Meaning |
|---|---|
| `tokens` | Context or communication tokens consumed |
| `compute_ms` | Declared compute time |
| `latency_ms` | End-to-end waiting or service latency |
| `money_usd` | Declared monetary cost |
| `privacy_loss` | Caller-defined privacy exposure units |
| `specimen_units` | Material or sample units consumed |
| `expert_minutes` | Human review or operator burden |

All components must be finite and non-negative. A `CostVector` is feasible only when every
component fits inside the vector budget. Component-wise feasibility happens before optimization;
no weighted sum can make an over-budget specimen or privacy action feasible.

## Scalarization

`CostWeights` is an explicit, non-negative weight vector. It is used only to compare policies that
already satisfy the component budget:

```text
objective = expected terminal Bayes risk
          + Σ dimension(weight[dimension] × expected_cost[dimension])
```

The result retains `expected_acquisition_cost` as a full vector and separately reports
`expected_scalarized_cost`. Reconstructing the scalar from the vector and weights is an invariant
tested by the kernel. Weights are not universal prices, legal thresholds, or claims about the value
of privacy or human work; they are caller policy inputs and should travel with the plan digest.

## Exact policy search

`adaptive_policy_with_cost_vectors` uses the same hard caps as scalar adaptive planning: at most
16 distinct acquisitions, 16 decisions on a path, and 65,536 evaluated states. At each state it
enumerates every unused acquisition whose vector fits the remaining budget, then conditions on all
declared outcomes. A cap is a refusal, not a sampled approximation wearing an exactness label.

The vector policy has its own recursive tree, so a caller cannot accidentally interpret a scalar
policy as having component-wise feasibility. The legacy scalar FIBER `fiber-query/0.5` contract
continues to report its scalar semantics explicitly; a future wire version can carry vector costs
once its cross-language schema and certificate binding are introduced.

## Domain use

The same contract applies to different providers without pretending their units are identical:

- a literature search can spend tokens and latency;
- a software test can spend compute and wall time;
- a biological assay can spend specimen and expert time;
- an external data join can spend privacy budget and money;
- an incident check can spend operator time and a bounded latency budget.

The vector is an accounting and feasibility contract, not a causal model, clinical recommendation,
or release decision.

## Cross-language route

`epistemic_adaptive_costed` is the MCP projection. Each row is shaped as
`{acquisition: <ordinary Acquisition>, cost: <CostVector>}` and the request carries a vector
`budget`, a vector `weights`, and `max_steps`. The response returns the canonical dimension list,
the complete vector policy tree, expected vector cost, and explicit scalarized cost. A vector-policy
refusal is data with `ok: false` and `fail_closed: true`; it is not silently downgraded to the
legacy scalar planner.

Python exposes `AdaptiveCostedRequest` and `AdaptiveCostedReport` on synchronous/asynchronous
Workspace and HTTP facades. TypeScript exposes `AdaptiveCostedArgs`, `AdaptiveCostVector`,
`AdaptiveCostedResult`, and `client.epistemicAdaptiveCosted`. These surfaces intentionally keep
the seven keys spelled out so a caller cannot accidentally omit privacy, specimen, or human-time
accounting while supplying a scalar weight.
