# Bioevaluation acquisition-trace audit

`bioeval_acquisition_audit` exposes the obligation-ledger portion of the `bioprism-bioevalx`
acquisition contract. It audits a caller-supplied ordered trace of retrievals, assays, metadata
inspection, expert consultation, and analysis. The route answers:

- which required and optional obligations were closed;
- whether the final trace is admissible;
- whether the agent stopped voluntarily or merely stopped because its trace ended;
- which actions were redundant, unnecessary, or delayed a decisive obligation;
- how cost was distributed by acquisition kind; and
- how the trace compares with a named reference policy.

It does not execute an acquisition. A trace is evidence about what a caller says happened, not a
retrieval receipt, assay result, biological truth source, or clinical decision.

## Request

```json
{
  "obligations": [
    { "id": "subtype", "required": true },
    { "id": "context", "required": false }
  ],
  "actions": [
    { "id": "read-notes", "kind": "metadata", "cost": 2, "closes": ["context"] },
    { "id": "search", "kind": "retrieval", "cost": 5, "closes": [] },
    { "id": "panel", "kind": "assay", "cost": 40, "closes": ["subtype"] }
  ],
  "stopped_after": true,
  "reference_policy": {
    "name": "random-acquisition",
    "cost": 30,
    "admissible": false
  }
}
```

Obligations and action IDs are stable and unique. `kind` is one of `retrieval`, `assay`,
`metadata`, `expert`, or `analysis`. Cost is an opaque non-negative integer in the caller's unit;
the kernel does not convert tokens, money, latency, specimen burden, or expert effort into one
another. `closes` must name declared obligations. At most 512 obligations and 512 actions are
accepted, and the encoded request is bounded at 20 MB.

## Admissibility is not completeness

Required obligations gate admissibility. Optional obligations strengthen a trace but cannot make a
trace admissible when a required item remains open. This gives four useful distinctions:

- `admissible`: every required obligation was closed;
- `stopped_inadmissible`: the caller marked a voluntary stop while required work remained;
- `open`: the supplied trace ended without a voluntary-stop marker and required work remains;
- action-level `refusal`: the trace attempted to close an undeclared obligation or reused an action
  ID, so no partial trace is projected as valid.

`stopped_after` is intentionally separate from action count. An empty trace can be voluntarily
stopped, but that does not make a required-obligation decision admissible. A non-stopped trace does
not demonstrate that the agent would have stopped under diminishing returns; it only says that the
caller did not mark a voluntary stop.

## Findings derived from ordered action history

### Redundant actions

An action is redundant when it closes no obligation that was still open when the action ran. This
includes repeated closure of an already-closed obligation and actions that close nothing. The route
retains the action row and the `redundant` flag rather than deleting it from the trace.

### Unnecessary actions

An action is unnecessary when every required obligation had already been closed before it ran. It
is possible for an action to be both redundant and unnecessary: an extra analysis after the
decision was admissible can close nothing new and still consume cost.

### Deferred decisive cost

`deferred_decisive_cost` is the cost spent before the first action that closed any required
obligation. It is `null` when no required obligation was ever closed because “the decisive source”
has no referent. This is an accounting signal, not an information-gain estimate or a claim that a
cheaper action would have been scientifically better.

## Named-policy regret

The kernel refuses to invent a baseline. If `reference_policy` is supplied, the response includes:

- the policy name, declared cost, and whether it was admissible;
- signed `cost_difference` (`trace cost - reference cost`);
- whether this trace and the reference are both admissible; and
- `like_for_like`, which is the only posture in which the cost comparison is a like-for-like
  admissibility comparison.

With no reference policy, `regret` is `null` and the response says that no baseline was requested.
With `require_reference: true`, omission of a named policy is a fail-closed `reference_policy`
refusal. A trace can spend less than an inadmissible reference, but the result retains
`like_for_like: false` so that it cannot be reported as an acquisition win.

## Successful projection

Schema is `bioprism-mcp/bioeval-acquisition-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-acquisition-audit/0.1",
  "workflow": "bioeval_acquisition_audit",
  "status": "admissible",
  "stopped_after": true,
  "admissible": true,
  "required_open_count": 0,
  "optional_open_count": 0,
  "cost": 47,
  "findings": {
    "redundant_action_ids": ["search"],
    "unnecessary_action_ids": [],
    "deferred_decisive_cost": 7
  },
  "reference_policy": { "name": "random-acquisition", "cost": 30, "admissible": false },
  "regret": {
    "cost_difference": 17,
    "this_admissible": true,
    "reference_admissible": false,
    "like_for_like": false
  },
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

The projection also returns every obligation row, every action row, open-obligation rows, costs by
kind, and the bounded action count. Set-derived finding lists use canonical lexical ordering for
replay stability; action rows retain original performance order.

## Refusals and boundaries

Malformed envelopes, unknown kinds, invalid scalar types, duplicate IDs, or unbounded inputs remain
argument errors. Domain-invalid traces return `ok: false`, a stage such as `trace_validation` or
`reference_policy`, an actionable refusal, and `fail_closed: true`. An invalid action never
produces a partial admissibility claim.

The route does not estimate information gain per token, infer downstream decision improvement,
optimize adaptive/sequential acquisition, replay a counterfactual experiment, verify that an assay
was performed, retrieve an external source, or create biological/clinical authority. Those are
separate capabilities and must not be inferred from obligation closure or cost arithmetic.

## SDK surfaces

- Python exposes `BioevalAcquisitionObligationArgs`, `BioevalAcquisitionActionArgs`,
  `BioevalAcquisitionReferencePolicyArgs`, `BioevalAcquisitionAuditArgs`,
  `BioevalAcquisitionAuditReport`, and `bioeval_acquisition_audit_report(...)` through
  `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`.
- TypeScript exposes `BioevalAcquisitionAuditArgs`, the typed action kind union, and
  `bioevalAcquisitionAudit(...)`; nested obligation, finding, cost, and regret projections remain
  JSON objects so the domain's refusal and admissibility distinctions are preserved.

Use this route to audit an acquisition trace and its stopping/accounting posture. Use
`epistemic_voi` to price an unperformed information action, `epistemic_selection_audit` to retain
observed context under a constraint, and `lab_plan` for the separate reachability/privacy planning
contract.
