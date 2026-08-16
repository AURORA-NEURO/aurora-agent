# Epistemic evidence-selection audit

`epistemic_selection_audit` exposes the bounded observed-context selection kernel in
`bioprism-epistemic`. It chooses a subset of already-observed evidence for a decision-relative
regret objective, then reports whether the choice has an exhaustive structural audit and an exact
small-instance comparison. It is the planner complement to `epistemic_context_audit`:

- context audit asks which subsets preserve a declared distortion tolerance;
- selection audit asks which subset the constrained greedy planner would choose and what may be
  claimed about that choice;
- `epistemic_voi` prices an acquisition whose outcome has not happened yet.

These are different states of evidence. The route never turns an observed likelihood profile into
an unperformed assay, a greedy choice into an adaptive policy, or a decision-relative regret into
a causal, biological, or clinical conclusion.

## Request

```json
{
  "problem": {
    "actions": ["treat", "defer"],
    "models": ["responsive", "resistant"],
    "loss": [0.0, 10.0, 10.0, 0.0]
  },
  "belief": { "mass": [0.4, 0.6] },
  "evidence_pool": {
    "items": [
      { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1] },
      { "id": "marker", "cost": 1.0, "likelihood": [0.8, 0.2] },
      { "id": "uninformative", "cost": 1.0, "likelihood": [1.0, 1.0] }
    ]
  },
  "constraint": { "cardinality": 2 },
  "protected": [],
  "check_submodularity": true,
  "include_lazy": true,
  "compare_optimum": true,
  "tolerance": 1e-9
}
```

`problem.loss` is row-major by action and model. The evidence pool is ordered: its indexes are
stable selection identities and tie-break inputs. A constraint may include `cardinality`,
`budget`, or both. `costs` overrides the evidence items' scalar costs when present; budgeted costs
must be positive. The scalar is caller-declared and may represent a prior scalarization of tokens,
compute, latency, privacy, specimen, or expert burden.

The route accepts at most 64 evidence items. That is a planning bound, not a claim that larger
selection problems are impossible. `protected` items are inserted before any relevance marginal
is calculated; if they do not fit the constraint, the route refuses rather than silently dropping
the protected closure.

## Three independent audit layers

### Greedy selection

The route runs the kernel's plain greedy selector. It recomputes every feasible marginal, refuses
non-positive steps, respects the cardinality/budget constraint, and returns accepted steps,
marginals, costs, total evaluations, protected items, and the selected value. A cardinality-only
selection can carry the `1 - 1/e` factor only when the exhaustive submodularity report is present
and says the objective is normalized, monotone, and submodular.

If no check was requested, or the ground set is larger than 12, the selection can still be useful,
but the guarantee is `not_checked`; it is never inferred from the objective name. Under a budget
constraint the kernel reports that its implemented cost-benefit path has no approximation factor,
even if the objective happens to look well behaved.

### Exhaustive structural check

For at most 12 items, `check_submodularity` tabulates every subset and checks:

- normalization, `F(empty) = 0`;
- monotonicity, so adding an item never lowers value;
- global diminishing returns over every `A ⊆ B` and `e ∉ B`;
- the equivalent local single-element-extension characterization.

The response retains the first/worst witnesses and the number of triples examined. A failed check
is a measured property of this concrete decision problem and pool, not a universal theorem about
regret reduction. Above 12 items the response says `not_run` with the cap rather than sampling and
calling the result exhaustive.

### Exact small-instance comparison

For at most 20 items, `compare_optimum` enumerates every feasible subset containing the protected
closure and returns the true constrained optimum, its cost, the greedy gap, and a ratio when the
optimum is non-zero. A zero optimum produces `ratio_status: "undefined_zero_optimum"` rather than
an invented `0/0` score. Above 20 items the exact comparison is `not_run` and the route does not
pretend that the greedy result is globally optimal.

`include_lazy` adds the lazy-greedy result and evaluation count. Agreement is diagnostic only:
lazy/greedy agreement does not replace the exhaustive submodularity check, and disagreement is
useful evidence that stale marginal bounds were unsafe for this objective.

## Successful projection

Schema is `bioprism-mcp/epistemic-selection-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/epistemic-selection-audit/0.1",
  "objective": "regret_reduction",
  "submodularity": { "status": "evaluated", "report": "..." },
  "greedy": {
    "chosen": [{ "index": 0, "id": "scan", "cost": 2.0 }],
    "value": 9.0,
    "guarantee": { "applicability": "does_not_apply" }
  },
  "lazy": { "chosen": [], "evaluations": 3 },
  "comparisons": {
    "greedy_lazy_agree": true,
    "exact_optimum": { "status": "evaluated", "ratio": 1.0 }
  },
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

The exact serialized `guarantee` is authoritative. Its three states distinguish an established
factor, a checked-but-failed premise, and a premise that was not checked. `submodularity.status`
and `comparisons.exact_optimum.status` separately show whether an audit was requested, evaluated,
refused, or bounded out.

## Refusals and boundaries

Valid domain inputs that cannot support the requested calculation return `ok: false`, a `stage`,
`refusal`, and `fail_closed: true`. Examples include contradictory full evidence, malformed or
incompatible likelihood vectors, protected closure exceeding a bound, invalid budget costs, and
exhaustive calculations that encounter a kernel contradiction. Input objects that cannot be
represented safely remain transport/argument errors.

The route does not implement adaptive sequential acquisition, causal identification, hidden
confounding adjustment, multi-objective cost vectors, matroid constraints, branch-and-bound, MCTS,
experiment execution, assay retrieval, or clinical recommendation. Those omissions remain visible
in the response so a planner can hand off to a different capability instead of overclaiming.

## SDK surfaces

- Python exposes `EpistemicSelectionEvidencePoolArgs`, `EpistemicSelectionConstraintArgs`,
  `EpistemicSelectionAuditArgs`, `EpistemicSelectionAuditReport`, and
  `epistemic_selection_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes `EpistemicSelectionAuditArgs`, `EpistemicSelectionConstraintArgs`, and
  `epistemicSelectionAudit(...)`; nested kernel projections remain JSON objects so the Rust
  guarantee/refusal semantics are not flattened into a score.

Use this route when the question is “which observed evidence should this bounded context retain?”
Use the context audit when the question is “which contexts satisfy this distortion tolerance?”
Use VOI only when the evidence action has not yet been performed.
