# Oncology research boundary check

`onco_boundary_check` classifies declared oncology output uses against the fixed research
boundary. It is a policy disposition, not a clinical recommendation and not an execution
authorization. The endpoint does not inspect arbitrary source text, contact a treating team, or
perform a clinical escalation.

## Versioned projection

Versioned results use:

```text
bioprism-mcp/onco-boundary-check/0.1
```

A successful result has `outcome_kind: "disposition"` and exposes the same decision at several
reconciled levels:

- `disposition_kind` is one of `release_in_full`, `release_partial`, or
  `refuse_and_escalate`;
- `disposition` is the tagged kernel decision, while `released`, `refused`, and
  `terminal_action` are projections of that decision;
- `requested_use_count`, `released_count`, and `refused_count` account for every declared use;
- `escalation_present`, `escalation_trigger`, and `escalation_route` preserve the human-process
  handoff without claiming that a handoff occurred; and
- `identifier_fields_present: false` records that the request passed the direct-identifier gate.

For a partial release, aggregate research work can remain available while an individual clinical
use is refused:

```json
{
  "ok": true,
  "schema": "bioprism-mcp/onco-boundary-check/0.1",
  "outcome_kind": "disposition",
  "disposition_kind": "release_partial",
  "released": ["cohort_analysis"],
  "refused": ["treatment_recommendation"],
  "requested_use_count": 2,
  "released_count": 1,
  "refused_count": 1,
  "escalation_present": true,
  "identifier_fields_present": false
}
```

The SDK rejects mismatched disposition tags, released/refused arrays, terminal actions,
escalation presence, or accounting counts. A top-level transport success therefore does not
silently become permission for every requested use.

## Fail-closed identifier handling

Requests with direct identifier fields return a structured refusal:

```json
{
  "ok": false,
  "schema": "bioprism-mcp/onco-boundary-check/0.1",
  "outcome_kind": "refused",
  "refusal_kind": "identifiers_present",
  "fail_closed": true,
  "requested_use_count": 1,
  "identifier_fields_present": true
}
```

The refusal deliberately does not echo the submitted request. `requested_use_count` is safe
accounting metadata; it does not disclose identifier values or permit the refused operation.
Versioned clients require `refusal_kind` and the explicit identifier-presence flag on this path.

## Python and TypeScript

Python exposes `OncoBoundaryReport` through `onco_boundary_report(...)`, including the typed
disposition, escalation, outcome, count, and identifier projections. TypeScript exposes the same
versioned fields through `OncoBoundaryResult`. Both clients retain structured domain refusals
instead of converting them into generic transport exceptions.

This contract remains a declaration audit: it does not establish that a caller is a clinician,
prove consent, validate the scientific quality of an aggregate analysis, or perform a medical
review.
