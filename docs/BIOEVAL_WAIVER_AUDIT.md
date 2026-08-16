# Bioevaluation release-gate waiver audit

`bioeval_waiver_audit` exposes the `bioprism-bioevalx` release-gate override
kernel as a bounded, replayable MCP projection. It applies explicitly declared
human waivers to caller-supplied gate verdicts while preserving the evidence
that caused the gate to block.

This is the narrow override contract from release policy 07.13. It is not a
gate evaluator, identity provider, signature verifier, CI system, deployment
controller, or scientific validity oracle. The route does not decide whether a
benchmark is healthy or whether a safety condition is true; it checks whether
the supplied release decision and its proposed exception satisfy the waiver
invariants.

## Request

```json
{
  "version": "release-2026.08",
  "at": "2026-08-16T12:00:00Z",
  "gates": [
    {
      "id": "health",
      "kind": "benchmark_health",
      "verdict": {
        "verdict": "violated",
        "detail": "calibration below floor"
      }
    },
    {
      "id": "unknown-rate",
      "kind": "maximum_unknown_rate",
      "verdict": {
        "verdict": "unevaluable",
        "missing": "reference panel"
      }
    },
    {
      "id": "safety",
      "kind": "safety_veto",
      "verdict": {
        "verdict": "violated",
        "detail": "forbidden action"
      }
    }
  ],
  "waivers": [
    {
      "gate": "health",
      "authoriser": "release-board",
      "rationale": "ship only the documented calibration exception",
      "expiry": "2026-09-01T00:00:00Z",
      "affected_versions": ["release-2026.08"],
      "follow_up": "recalibrate before the next release"
    }
  ],
  "max_items": 100,
  "require_releasable": false,
  "require_no_unevaluable": false
}
```

`version` is the exact release version being evaluated. `at` is required so
expiry is deterministic and replayable; the route never reads a wall clock.
Both timestamps use the workspace RFC-3339 parser and compare instants rather
than their original offset spelling.

Gate identifiers are unique and bounded. The closed gate-kind vocabulary is:

- `safety_veto`;
- `benchmark_health`;
- `capability_floor`;
- `non_inferiority`;
- `required_improvement`;
- `cost_ceiling`;
- `confidence_requirement`; and
- `maximum_unknown_rate`.

Every gate carries one internally tagged verdict:

- `{ "verdict": "met" }` does not block and cannot be waived;
- `{ "verdict": "violated", "detail": "..." }` blocks with a named
  observed failure; and
- `{ "verdict": "unevaluable", "missing": "..." }` blocks because the
  evidence needed to decide the gate was absent.

The route does not accept a bare verdict string. The tagged object keeps the
reason for a violation or the missing evidence visible in the evidence record.

## What makes a waiver complete

The real `Waiver::sign` constructor requires all four policy elements and the
scope/expiry fields:

1. `authoriser` identifies the party asserting the exception;
2. `rationale` explains why the blocking gate is being allowed through;
3. `expiry` states when the exception stops applying;
4. `affected_versions` names one or more exact release versions; and
5. `follow_up` states the work required after the exception.

Whitespace-only values are rejected. A waiver with an empty affected-version
list is not a weak waiver; it is not a waiver. The route additionally rejects
duplicate waiver declarations for one gate, because two competing exceptions
would make the release record ambiguous.

The authoriser and authority are recorded assertions. This route does not
authenticate a person, inspect an organisation directory, validate a digital
signature, or decide whether the named party is allowed to sign. That boundary
is explicit in the returned limitations rather than being hidden behind a
boolean `authorised` field.

## Application rules

The route invokes the real `ReleaseDecision::waive` and `Waiver::apply` rules.
They enforce the following order and distinctions:

- a waiver must name an existing blocking gate;
- a waiver must cover the exact requested `version`;
- a waiver whose expiry is before `at` is refused;
- a `met` gate has nothing to waive;
- a `safety_veto` is never waivable; and
- a successfully applied waiver is retained alongside the original gate.

The safety-veto rule has no force parameter or second bypass method. A waiver
that names a veto returns a fail-closed `waiver_application` refusal. A veto
therefore remains both a finding and a blocker; it cannot be converted into a
warning by adding more paperwork.

The release version check is exact. A waiver for `release-2026.08-rc1` does not
cover `release-2026.08`, and a waiver listing `latest` does not become a
standing exception for another version. This is why affected versions are a
list rather than a free-form scope note.

## A waiver does not pass a gate

The most important output invariant is that a waiver changes only the blocking
posture. The original `GateVerdict` remains in the gate row and in every
applied-waiver row. A violated gate remains violated; an unevaluable gate
remains unevaluable. The release summary reports both `blocking_before` and
`blocking_after`, so the effect of the exception is inspectable.

For the request above, the health gate is waived, but the unknown-rate and
safety gates still block:

```json
{
  "release": {
    "version": "release-2026.08",
    "gate_count": 3,
    "blocking_before": 3,
    "blocking_after": 2,
    "waived_count": 1,
    "unevaluable_count": 1,
    "releasable": false
  },
  "findings": {
    "still_blocking": { "ids": ["safety", "unknown-rate"], "total": 2, "omitted": 0 },
    "waived_gates": { "ids": ["health"], "total": 1, "omitted": 0 },
    "unevaluable_gates": { "ids": ["unknown-rate"], "total": 1, "omitted": 0 },
    "safety_vetoes": { "ids": ["safety"], "total": 1, "omitted": 0 }
  }
}
```

`releasable` is the mechanical predicate that no gate still blocks after
valid waivers. It is not an approval claim. It can be false because of a
violated gate, an unevaluable gate, or an unwaivable safety veto; those causes
remain separately visible.

## Fail-closed policies

`require_releasable: true` turns any remaining blocker into an
`ok: false`, `stage: "release_gate_policy"`, `fail_closed: true` refusal. This
is appropriate for a release workflow that requires every blocker to be
resolved or validly waived before proceeding.

`require_no_unevaluable: true` turns any remaining unevaluable gate into an
`ok: false`, `stage: "unknown_rate_policy"`, `fail_closed: true` refusal. It
does not treat a waived unknown gate as measured evidence; the gate remains in
the unevaluable count and the policy remains conservative.

Both policies are optional because a reviewer may want a complete evidence
record even while a release is blocked. A successful transport response is
not itself a release pass: callers must inspect `release.releasable`,
`findings`, and the policy options used.

## Successful projection

Schema is `bioprism-mcp/bioeval-waiver-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-waiver-audit/0.1",
  "workflow": "bioeval_waiver_audit",
  "release": {
    "version": "release-2026.08",
    "evaluated_at": "2026-08-16T12:00:00Z",
    "gate_count": 3,
    "blocking_before": 3,
    "blocking_after": 2,
    "waived_count": 1,
    "unevaluable_count": 1,
    "releasable": false
  },
  "gates": {
    "rows": [
      {
        "id": "health",
        "kind": "benchmark_health",
        "verdict": { "verdict": "violated", "detail": "calibration below floor" },
        "blocks_before": true,
        "waived": true,
        "blocks_after": false,
        "unevaluable": false
      }
    ],
    "returned": 1,
    "total": 3,
    "omitted": 2
  },
  "waivers": {
    "rows": [
      {
        "gate": "health",
        "underlying_verdict": { "verdict": "violated", "detail": "calibration below floor" },
        "waiver": {
          "gate": "health",
          "authoriser": "release-board",
          "rationale": "ship only the documented calibration exception",
          "expiry": "2026-09-01T00:00:00Z",
          "affected_versions": ["release-2026.08"],
          "follow_up": "recalibrate before the next release"
        },
        "applied_at": "2026-08-16T12:00:00Z"
      }
    ],
    "returned": 1,
    "total": 1,
    "omitted": 0
  },
  "guarantees": [
    "a waiver changes blocking posture only; the original gate verdict remains visible",
    "safety vetoes are never waivable",
    "unevaluable gates remain separately countable"
  ],
  "limitations": [
    "the route does not calculate gate verdicts or verify authoriser identity",
    "no external publication or deployment is performed"
  ]
}
```

Rows, applied waivers, and finding identifiers are bounded by `max_items`,
but their `total` and `omitted` counts remain available. Truncation therefore
cannot look like an empty release decision or an absence of blockers.

## Composition and boundaries

The waiver audit composes with other local contracts:

- `bioeval_evaluator_audit` can establish why an evaluator-dependent gate was
  unevaluable without turning harness failure into a task failure;
- `bioeval_plane_audit` can preserve scored, unscored, and inapplicable
  dimensions before a release gate consumes their posture;
- `bioeval_reference_audit`, `bioeval_grounding_audit`, and
  `bioeval_estimand_audit` can keep reference, claim, and interpretation
  uncertainty explicit; and
- `release_audit` can compose named required checks, while this route provides
  the narrower, auditable exception mechanism for a caller-supplied gate set.

The route does not calculate non-inferiority intervals, confidence, unknown
rates, benchmark health, capability floors, or safety decisions. Those inputs
must arrive from their owning kernels. It also does not publish an exception,
notify a reviewer, sign an artifact, execute follow-up, or override a safety
boundary.

## SDK surfaces

- Python exposes `BioevalWaiverGateVerdictArgs`, `BioevalWaiverGateArgs`,
  `BioevalWaiverArgs`, `BioevalWaiverAuditArgs`,
  `BioevalWaiverAuditReport`, and `bioeval_waiver_audit_report(...)` through
  `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`.
- TypeScript exposes typed gate/verdict, waiver, audit-argument, and
  audit-result interfaces plus `bioevalWaiverAudit(...)`.

