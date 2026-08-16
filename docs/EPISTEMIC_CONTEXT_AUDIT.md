# Epistemic context and rate–distortion audit

`epistemic_context_audit` exposes the decision-relative context calculus in
`bioprism-epistemic`. It audits which already-observed evidence can be omitted while keeping
decision regret within a declared tolerance. The endpoint deliberately keeps four questions
separate:

1. Do the compatible models still agree on what action to take?
2. Does a sufficient context exist under the chosen distortion criterion?
3. What is the exhaustive rate–distortion Pareto frontier?
4. What happened for each caller-requested subset?

This is a decision-support calculation, not causal identification, biological validation,
clinical advice, evidence acquisition, or an adaptive policy. Its prior, loss matrix, likelihoods,
and scalarized evidence costs are explicit caller inputs.

## Request

```json
{
  "problem": {
    "actions": ["treat", "abstain"],
    "models": ["responsive", "resistant"],
    "loss": [0.0, 10.0, 10.0, 0.0]
  },
  "belief": { "mass": [0.5, 0.5] },
  "evidence_pool": {
    "items": [
      { "id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1] },
      { "id": "marker", "cost": 1.0, "likelihood": [0.1, 0.9] }
    ]
  },
  "criterion": "bayes_regret",
  "tolerance": 1.0,
  "compatibility_floor": 0.0,
  "subsets": [[0], [0, 1]],
  "include_frontier": true,
  "max_rows": 100
}
```

`problem.loss` is row-major by action and model. Evidence is already observed: each likelihood
vector conditions the belief when that item is retained. This is intentionally different from
`epistemic_voi`, where an acquisition has outcomes that have not yet happened. Evidence-pool order
is part of the identity because subset indices and deterministic tie-breaking use that order.

The kernel and SDK bound the evidence pool to 16 items when the exhaustive frontier is needed,
which caps enumeration at `2^16` subsets. The request supports up to 256 individually requested
subsets and 1,000 returned rows. A larger pool is refused rather than silently sampled: a sampled
frontier can be an upper bound, but it cannot honestly be returned as the minimum distortion at a
rate.

## Identification versus compression

The `identification` value is calculated from all available evidence and the compatible-model
floor. Its tagged states are:

- `point_identified`: every compatible model prefers the same action;
- `set_identified_within_tolerance`: models disagree, but the minimax action's worst regret is
  within tolerance;
- `non_identified`: the compatible models disagree by more than tolerance even with all evidence.

This is decision identification only. It does not establish a causal effect, a biological
mechanism, or a clinical recommendation. `non_identified` is retained even when a Bayes-regret
compression can be calculated under a caller-declared prior.

The `sufficiency` value asks for the cheapest context whose distortion is within tolerance. Under
`bayes_regret`, the prior is an explicit modeling assumption and a full context has zero regret
relative to that posterior. Under `minimax_regret`, non-identification is an abstention condition:
the endpoint will not select a context as “sufficient” when the model set cannot support the
decision at any context size. Other abstentions distinguish non-identification from an unattained
tolerance.

## Exhaustive frontier

When `include_frontier` is true, the kernel evaluates every evidence subset and retains only
non-dominated rate–distortion points. Each point reports:

- `rate`: summed scalar evidence cost;
- `distortion`: excess decision loss relative to acting with all evidence;
- `retained`: evidence indexes in the original pool order.

The frontier is not an embedding similarity curve and it is not a predictive accuracy curve. An
evidence item can be expensive but decision-irrelevant, or cheap but move the decider toward a
high-loss action. The implementation enumerates because distortion need not be monotone when an
intermediate observation pulls a posterior toward the wrong action.

## Requested subset rows

`subsets` is an optional audit list for contexts a caller wants to inspect explicitly. Every row
is one of:

- `evaluated`, with retained indexes, rate, distortion, action, reference action, and compatible
  models;
- `refused`, with a fail-closed reason for an out-of-range or repeated index, a contradictory
  posterior, or another kernel error.

Subset refusals are evidence about that proposed context, not a refusal of the entire valid
frontier. `subset_count`, `subset_refusal_count`, `subset_rows`, `subset_rows_omitted`, and
`max_rows` make the denominator and bounded disclosure explicit. A missing row is never silently
treated as a successful empty context.

## Successful projection

Schema is `bioprism-mcp/epistemic-context-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/epistemic-context-audit/0.1",
  "criterion": "bayes_regret",
  "problem": { "actions": ["treat", "abstain"], "models": ["responsive", "resistant"] },
  "evidence_pool": { "item_count": 2, "full_rate": 3.0 },
  "identification": { "status": "non_identified" },
  "sufficiency": { "outcome": "sufficient", "retained": [0, 1] },
  "frontier": { "evaluated": 4, "points": ["..."] },
  "subset_rows": ["..."],
  "subset_count": 2,
  "subset_refusal_count": 0,
  "subset_rows_omitted": 0,
  "max_rows": 100,
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

Structured refusals use `ok: false`, a stage such as `enumeration_bound`, `identification`,
`minimal_context`, or `frontier`, `fail_closed: true`, and an actionable refusal. Input problems
that cannot be represented as an epistemic domain value remain transport errors; a valid domain
calculation that cannot support the requested claim remains a typed refusal or abstention.

## SDK surfaces

- Python exposes `EpistemicEvidenceItemArgs`, `EpistemicEvidencePoolArgs`,
  `EpistemicContextAuditArgs`, `EpistemicContextAuditReport`, and
  `epistemic_context_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes `EpistemicContextAuditArgs`, `EpistemicEvidencePoolArgs`, and
  `epistemicContextAudit(...)`; nested identification, frontier, sufficiency, and subset values
  remain JSON objects whose semantics are owned by Rust.
- The route is catalogued beside `epistemic_voi`, context compilation, posterior gates, and
  evaluation audits so an agent can distinguish evidence acquisition value from context
  compression value.

Use this audit before claiming token/context efficiency or decision preservation. It does not
license a causal, scientific, patient-level, or release conclusion by itself.
