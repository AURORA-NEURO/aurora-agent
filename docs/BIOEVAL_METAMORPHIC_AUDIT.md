# Bioevaluation metamorphic-response audit

`bioeval_metamorphic_audit` exposes the `bioprism-bioevalx` metamorphic-response
kernel as a bounded, reviewable MCP projection. It checks whether a system's
response changes in the declared way when an evaluation input is transformed.
This is a response-consistency audit, not a second task evaluator and not a
biological truth oracle.

The route invokes the real `Family` and `Suite` constructors and the real
metamorphic verdict function. Consequently, duplicate trial identifiers,
relation mismatches, invalid response variants, and malformed directional
responses are rejected by the same domain rules used by the Rust library.

## Request shape

```json
{
  "families": [
    {
      "id": "invariance-under-renaming",
      "relation": "invariant",
      "trials": [
        {
          "id": "rename-001",
          "relation": "invariant",
          "response": { "response": "unchanged" }
        },
        {
          "id": "shortcut-002",
          "relation": "invariant",
          "response": { "response": "moved", "direction": "increase" }
        },
        {
          "id": "not-comparable-003",
          "relation": "invariant",
          "response": { "response": "incomparable" }
        }
      ]
    },
    {
      "id": "dose-response",
      "relation": { "directional_change": { "expected": "increase" } },
      "trials": [
        {
          "id": "dose-001",
          "relation": { "directional_change": { "expected": "increase" } },
          "response": { "response": "moved", "direction": "increase" }
        }
      ]
    }
  ],
  "max_items": 100,
  "require_both_relations": true,
  "fail_on_undetermined": false
}
```

`Response` is an internally tagged Rust enum. The JSON representation is
therefore always an object with a `response` field, including `unchanged` and
`incomparable`; these must not be sent as bare strings. A `moved` response must
also carry exactly one `direction`, either `increase` or `decrease`. The
direction is the observed response direction, not the expected direction.

The two relation encodings are intentionally different:

- `"invariant"` declares that the transformed input should preserve the
  response.
- `{ "directional_change": { "expected": "increase" } }` declares that the
  response should move in one named direction. `decrease` is the other valid
  expectation.

Each family must have a non-empty identifier, a non-empty trial set, and a
single relation shared by all of its trials. Family identifiers and trial
identifiers are unique within their respective scopes. Requests are bounded to
1024 families, 4096 trials in total, 4096 trials per family, 1000 returned
family rows, and a 20 MB encoded JSON input.

## Verdicts and failure directions

The kernel returns one verdict per trial. The audit retains the verdict rather
than replacing it with a boolean so reviewers can distinguish four materially
different observations:

| Declared relation | Observed response | Interpretation |
| --- | --- | --- |
| invariant | unchanged | evidence consistent with invariance |
| invariant | moved | false invariance; a supposedly irrelevant transformation changed the response |
| directional change | moved in expected direction | evidence consistent with the directional relation |
| directional change | unchanged | false sensitivity; the expected response movement was absent |
| directional change | moved in the opposite direction | wrong direction; the transformation moved the response against the declaration |
| either | incomparable | undetermined; no directional comparison is available |

The finding buckets are deliberately not synonyms:

- `false_sensitivity_trials` contains directional-change trials that stayed
  unchanged. This is a missed response, not proof that the model is invariant.
- `false_invariance_trials` contains invariant trials that moved. This is a
  shortcut or sensitivity to a supposedly irrelevant change.
- `wrong_direction_trials` contains directional-change trials that moved in
  the opposite direction.
- `undetermined_families` identifies families containing incomparable trials.

The response direction is only meaningful for `moved`. An `incomparable`
response is not coerced to `unchanged`, and it is not counted as a passing or
failing directional observation.

## Consistency and denominators

Every family row reports its evidential trial count, undetermined count, and a
consistency result over the evidential denominator only. Incomparable trials
are excluded from that denominator, but remain visible through their counts,
trial verdicts, and undetermined-family findings. An all-incomparable family
therefore has no evidential consistency percentage; it is not a perfect family
and it is not a zero-score family.

The route intentionally does not emit a suite-wide consistency percentage.
Families can represent different transformations, expected directions, sample
sizes, and scientific questions. Summing their pass rates would silently
choose weights and imply exchangeability that the contract does not declare.
Callers that need a release gate should define an explicit family-weighting and
matched-basis contract outside this route.

`max_items` limits returned family rows and witness identifiers only. It never
changes the kernel verdicts or the suite counts. Every bounded collection has
`total`, `returned` or `ids`, and `omitted` metadata where applicable, so an
empty returned witness list cannot be mistaken for no findings.

## Fail-closed policies

`require_both_relations: true` requires at least one invariant family and one
directional-change family. A suite that does not cover both relation types is
returned as a structured `relation_coverage` refusal with `fail_closed: true`.
This policy is useful when a benchmark claims to exercise both shortcut
resistance and expected-response sensitivity.

`fail_on_undetermined: true` rejects any suite containing an incomparable trial
at the `oracle_quality` stage. The underlying family and trial input is still
the caller's responsibility; the route does not infer comparability from raw
task outputs. Leaving the policy false preserves a reviewable report while
keeping undetermined counts explicit.

Malformed input, duplicate identifiers, relation mismatches, invalid response
tags, missing directions, extra direction fields on non-moved responses, and
unbounded requests are all fail-closed. No fallback consistency value or
imputed response is emitted after validation failure.

## Successful projection

Schema is `bioprism-mcp/bioeval-metamorphic-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-metamorphic-audit/0.1",
  "workflow": "bioeval_metamorphic_audit",
  "suite": {
    "family_count": 2,
    "trial_count": 4,
    "relation_coverage": {
      "invariant": true,
      "directional_change": true,
      "complete": true
    },
    "undetermined_trial_count": 1,
    "has_suite_wide_consistency": false
  },
  "families": {
    "rows": [
      {
        "id": "invariance-under-renaming",
        "relation": "invariant",
        "trial_count": 3,
        "evidential_trial_count": 2,
        "undetermined_trial_count": 1,
        "consistent": false,
        "consistency": 0.5,
        "verdicts": [
          { "id": "rename-001", "verdict": "consistent" },
          { "id": "shortcut-002", "verdict": "false_invariance" },
          { "id": "not-comparable-003", "verdict": "undetermined" }
        ]
      }
    ],
    "returned": 1,
    "total": 2,
    "omitted": 1
  },
  "findings": {
    "false_sensitivity_trials": { "ids": [], "total": 0, "omitted": 0 },
    "false_invariance_trials": { "ids": ["shortcut-002"], "total": 1, "omitted": 0 },
    "wrong_direction_trials": { "ids": [], "total": 0, "omitted": 0 },
    "undetermined_families": { "ids": ["invariance-under-renaming"], "total": 1, "omitted": 0 }
  },
  "guarantees": [
    "incomparable responses remain undetermined",
    "no suite-wide consistency percentage is emitted"
  ],
  "limitations": [
    "the route does not execute transformations or task evaluators",
    "the route does not establish biological, causal, or clinical validity"
  ]
}
```

The exact per-family row also preserves the serialized trial relation and
response, the real kernel verdict, evidence/consistency booleans, and bounded
witness data. A successful transport envelope means the audit ran; callers
must inspect `findings`, `relation_coverage`, and any policy posture before
treating it as acceptable evidence.

## Composition and boundaries

The metamorphic audit composes with the other bioevaluation projections:

- `bioeval_evaluator_audit` separates evaluator-health failures from task
  outcomes before a response is admitted as evidence;
- `bioeval_plane_audit` keeps a metamorphic dimension's scored, unscored, and
  inapplicable states distinct;
- `bioeval_grounding_audit` can audit claims made about the metamorphic result
  without dereferencing an external artifact; and
- `bioeval_acquisition_audit` and `bioeval_estimand_audit` can preserve the
  information obligations and interpretation scope surrounding a trial.

The route audits caller-supplied response declarations. It does not generate
mutations, execute a model, compare raw predictions, estimate effect sizes,
select a benchmark threshold, infer a causal mechanism, or grant release,
clinical, or biological authority.

## SDK surfaces

- Python exposes `BioevalMetamorphicRelationArgs`,
  `BioevalMetamorphicResponseArgs`, `BioevalMetamorphicTrialArgs`,
  `BioevalMetamorphicFamilyArgs`, `BioevalMetamorphicAuditArgs`,
  `BioevalMetamorphicAuditReport`, and
  `bioeval_metamorphic_audit_report(...)` through `Workspace`,
  `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`.
- TypeScript exposes typed relation, response, trial, family, audit-argument,
  and audit-result interfaces plus `bioevalMetamorphicAudit(...)`.

