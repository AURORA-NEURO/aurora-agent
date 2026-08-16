# Bioevaluation estimand and identification audit

`bioeval_estimand_audit` exposes the claim-discipline kernel in `bioprism-bioevalx`. It is the
transport boundary for a result that needs to say what intervention was compared with what, on
which unit, for which outcome and horizon, and in which scope. It also keeps three frequently
collapsed distinctions visible: association versus intervention, model-conditional evidence versus
external corroboration, and identification being named versus identification being checked.

## Request

```json
{
  "estimand": {
    "intervention": "knockdown",
    "comparator": "control",
    "unit": "cell line",
    "outcome": "viability",
    "horizon": "72h",
    "scope": "pdac-twin"
  },
  "kind": "intervention",
  "basis": {
    "evidentiary": "model_conditional",
    "model": "pdac-twin-v2"
  },
  "identification": {
    "identification": "probed",
    "strategy": "backdoor",
    "assumptions": ["no unmeasured confounding"],
    "checks": [
      { "name": "negative-control", "passed": false, "detail": "signal remained" }
    ]
  },
  "corroborations": [
    { "source": "GSE-14520", "kind": "intervention", "detail": "external replication" }
  ],
  "transport_requests": [
    { "target": "pdac-twin", "declared_scopes": ["pdac-twin"] },
    { "target": "patients", "declared_scopes": ["pdac-twin"] }
  ],
  "require_identification": true,
  "require_corroboration": true,
  "strict_transport": false
}
```

The estimand requires non-empty `intervention`, `comparator`, `unit`, `outcome`, `horizon`, and
`scope`. The route invokes `Estimand::declare`; it does not assemble a bypass object. `kind` is
`association` or `intervention`. `basis` is one of the typed `Evidentiary` variants:
`model_conditional` with `model`, `observational` with `dataset`, or `experimental` with `study`.
Corroborations and transport requests are bounded at 256 rows and textual fields at 4096 bytes.

## Claim language is a safety boundary

The kernel renders the licensed sentence rather than allowing a caller to choose a verb:

- an `association` finding uses “is associated with”;
- an `intervention` finding uses “changes”.

This prevents predictive feature importance from being presented as an intervention claim through
the ordinary success path. The rendered language also carries `identification not assessed` for an
unassessed intervention and carries `model-conditional on <model>` while a model finding has no
accepted corroboration.

The route does not rewrite the sentence supplied by a report, and it does not infer that an
intervention was randomized. It returns the kernel's language as a constrained claim projection.

## Identification posture

Identification is a tagged state, not a boolean validity claim:

- `not_assessed`: no strategy or checks were supplied;
- `declared`: a strategy and assumptions were named, but no check was recorded; and
- `probed`: a strategy, assumptions, and one or more negative-control or sensitivity checks were
  recorded, including failed checks.

The response includes the serialized identification union and an `identification_summary` with
status, strategy, assumption count, check count, failed-check count, failed-check names, and the
`probed` flag. `require_identification: true` turns `not_assessed` into a fail-closed policy
refusal. It does not turn a declared or probed record into proof that the assumptions hold.

## Model-conditional promotion

`Finding::promote` requires a named `Corroboration` object. There is no implicit promotion from
rerunning a simulator, from setting a flag, or from having a non-empty evidence list. For a
model-conditional basis, a corroboration whose source is the same model is rejected. External
corroborations are retained with their claim kind and detail, and the response reports:

- the accepted corroboration rows and count;
- whether the finding is still model-conditional; and
- the exact claim language after promotion.

`require_corroboration: true` refuses a model-conditional finding with no supplied corroboration.
The route records caller-declared external corroboration; it does not authenticate the source,
replicate the study, or establish real-world truth.

## Scope transport

`transport_requests` asks whether each target appears in the caller's `declared_scopes` set. Each
row retains `target`, declared scopes, `ok`, and a refusal. The aggregate transport status is:

- `not_requested` when no target was supplied;
- `all_declared` when every requested target is present;
- `partially_declared` when some are present and some are refused; or
- `all_refused` when none is present.

This is a declaration gate, not a scope mapper. It does not calculate transportability, population
shift, measurement loss, or target comparability. `strict_transport: true` promotes any row-level
out-of-scope result into a fail-closed `transport_policy` refusal; the default preserves every row
for review.

## Successful projection

Schema is `bioprism-mcp/bioeval-estimand-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-estimand-audit/0.1",
  "workflow": "bioeval_estimand_audit",
  "estimand": {
    "intervention": "knockdown",
    "comparator": "control",
    "unit": "cell line",
    "outcome": "viability",
    "horizon": "72h",
    "scope": "pdac-twin",
    "five_elements_complete": true
  },
  "claim": {
    "kind": "intervention",
    "basis_kind": "model_conditional",
    "basis_source": "pdac-twin-v2",
    "identification_summary": {
      "status": "probed",
      "assumption_count": 1,
      "check_count": 1,
      "failed_check_count": 1,
      "failed_check_names": ["negative-control"],
      "probed": true
    },
    "corroboration_count": 1,
    "still_model_conditional": false,
    "claim_language": "..."
  },
  "transport": {
    "status": "partially_declared",
    "requested": 2,
    "accepted": 1,
    "refused": 1,
    "rows": ["..."]
  },
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

The raw serialized basis, identification union, and corroboration rows remain in the claim
projection. A caller can therefore distinguish a declared strategy from a probed strategy and a
model-conditioned result from one with external corroboration without reconstructing lost state.

## Refusals and boundaries

Missing top-level objects, malformed arrays, invalid scalar types, duplicate transport targets, or
unbounded requests are argument errors. An invalid estimand, evidentiary union, identification
union, same-model corroboration, missing required identification/corroboration, or strict
out-of-scope transport returns `ok: false` with a stage, actionable refusal, and `fail_closed: true`.
No partially promoted finding is returned as success after a corroboration refusal.

The route does not build a causal graph, run d-separation, validate consistency of assumptions,
execute negative controls, estimate an effect, simulate bias, calculate decision regret, authenticate
external studies, map scope ontologies, or grant clinical authority. Those are separate evidence
and safety contracts.

## SDK surfaces

- Python exposes `BioevalEstimandArgs`, `BioevalBasisArgs`, `BioevalIdentificationArgs`,
  `BioevalIdentificationCheckArgs`, `BioevalCorroborationArgs`, `BioevalTransportRequestArgs`,
  `BioevalEstimandAuditArgs`, `BioevalEstimandAuditReport`, and
  `bioeval_estimand_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes typed estimand, basis, identification, corroboration, transport, and audit
  argument interfaces plus `bioevalEstimandAudit(...)`; nested claim and transport projections
  remain JSON objects so refusal and qualification state stay visible.

Use this route for claim-language and estimand discipline. Use `bioeval_grounding_audit` for the
claim-evidence graph, `bioeval_reference_audit` for reference semantics, and
`bioeval_acquisition_audit` for ordered information-acquisition obligations.
