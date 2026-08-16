# Bioevaluation evaluator-health audit

`bioeval_evaluator_audit` exposes the evaluator-health boundary in `bioprism-bioevalx`.
It prevents a timed-out grader, crashed harness, or broken fixture from being reported as a
task failure. It also prevents a healthy negative result with no diagnostic evidence from being
quietly counted, and retains hidden-data access as a separate review finding even when the task
predicate passed.

The route audits serialized records. It does not execute commands, create sandboxes, mount
fixtures, inspect a host filesystem, or authenticate that the caller's diagnostic is true.
Those are deployment responsibilities. This route makes the resulting evidence legible and
keeps the scoring boundary fail-closed.

## Request

```json
{
  "runs": [
    {
      "evaluator": "grader-a",
      "health": { "health": "healthy" },
      "reached": "met",
      "diagnostic": {
        "command": "pytest tests/test_answer.py",
        "exit_state": "0",
        "diff": "",
        "logs": [],
        "hidden_data_access": []
      }
    },
    {
      "evaluator": "grader-b",
      "health": { "health": "healthy" },
      "reached": "not_met",
      "diagnostic": {
        "command": "pytest tests/test_answer.py",
        "exit_state": "1",
        "diff": "expected calibrated output was absent",
        "logs": ["assertion failed at test_answer.py:42"],
        "hidden_data_access": []
      }
    },
    {
      "evaluator": "timeout",
      "health": { "health": "timed_out", "after": "120s" },
      "reached": null,
      "diagnostic": {
        "command": "pytest tests/test_answer.py",
        "exit_state": "timeout",
        "diff": ""
      }
    },
    {
      "evaluator": "fixture-check",
      "health": { "health": "fixture_broken", "detail": "expected file absent" },
      "reached": "met",
      "diagnostic": {
        "command": "grader",
        "exit_state": "fixture-error",
        "diff": "",
        "hidden_data_access": ["read expected_outputs/"]
      }
    }
  ],
  "max_items": 100,
  "require_task_evidence": true,
  "fail_on_hidden_data": false
}
```

`runs` is bounded at 1024 rows and the encoded request at 20,000,000 bytes. `max_items` bounds
each returned row list, diagnostic list, and identifier finding from 1 through 1000. Evaluator
identifiers are non-empty and bounded at 256 bytes. The serialized health shape is internally
tagged: `healthy`, `timed_out` with `after`, `errored` with `detail`, or `fixture_broken` with
`detail`.

## The three independent axes

Every row keeps the following fields separate:

1. `health` is a statement about the evaluator harness. Only `healthy` can produce task evidence.
2. `reached` is the task predicate reached by the evaluator: `met`, `not_met`, or
   `inapplicable`. It is never trusted by itself; the kernel calls `EvaluatorRun::task_outcome`.
3. `diagnostic` records command, exit state, relevant diff, logs, and hidden-data access. A
   healthy `not_met` without any diagnostic is refused as an unsupported negative result.

An unhealthy row may contain a `reached` field in caller data, but the route still returns a null
`task_outcome`, an explicit `task_outcome_refusal`, and an `unscored_reason`. This is intentional:
stale or accidentally populated fields cannot turn a broken harness into a task score.

The healthy task outcomes are counted only after the real evaluator kernel accepts them. Thus a
healthy `met`, a diagnostic-backed `not_met`, and `inapplicable` can be counted, while a healthy
`not_met` with an empty diagnostic remains refused. `inapplicable` is not converted to failure.

## Successful projection

Schema is `bioprism-mcp/bioeval-evaluator-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-evaluator-audit/0.1",
  "workflow": "bioeval_evaluator_audit",
  "runs": {
    "rows": [
      {
        "index": 0,
        "evaluator": "grader-a",
        "health": { "health": "healthy" },
        "health_label": "healthy",
        "healthy": true,
        "reached": "met",
        "task_outcome": "met",
        "task_outcome_status": "accepted",
        "task_outcome_refusal": null,
        "unscored_reason": null,
        "hidden_data_touched": false,
        "diagnostic": { "empty": false }
      }
    ],
    "returned": 1,
    "total": 4,
    "omitted": 3
  },
  "panel": {
    "run_count": 4,
    "healthy_count": 2,
    "unhealthy_count": 2,
    "task_evidence_count": 2,
    "refused_task_outcome_count": 2,
    "says_anything": true,
    "no_task_evidence": false,
    "hidden_data_touched_count": 1,
    "duplicate_evaluator_count": 0,
    "outcomes": { "met": 1, "not_met": 1, "inapplicable": 0 },
    "posture": "review_required_hidden_data"
  },
  "findings": {
    "unhealthy_evaluators": { "ids": ["fixture-check", "timeout"], "total": 2, "omitted": 0 },
    "refused_task_outcomes": { "ids": ["fixture-check", "timeout"], "total": 2, "omitted": 0 },
    "hidden_data_evaluators": { "ids": ["fixture-check"], "total": 1, "omitted": 0 },
    "duplicate_evaluator_ids": { "ids": [], "total": 0, "omitted": 0 },
    "unscored_evaluator_count": 2
  },
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

`panel.posture` is `no_task_evidence` when every row is unusable, `review_required_hidden_data`
when usable task evidence exists but a diagnostic reports hidden-data access, and
`task_evidence_available` otherwise. The posture is not a release approval and does not erase
the row-level evidence.

Duplicate evaluator IDs are retained and reported instead of being silently merged, majority
voted, or treated as independent replication. This matters when a panel accidentally runs the
same grader twice or when two records carry the same evaluator identity.

## Fail-closed policies

`require_task_evidence: true` returns `ok: false`, `stage: "panel_policy"`, and
`fail_closed: true` when no healthy run yields an accepted outcome. This distinguishes “the
benchmark says nothing” from “the system failed the benchmark.”

`fail_on_hidden_data: true` returns `ok: false`, `stage: "hidden_data_policy"`, and
`fail_closed: true` if any diagnostic records hidden-data access. With the default `false`, the
finding remains visible and the panel posture becomes `review_required_hidden_data`; it is not
silently promoted to a clean pass.

Malformed serialized runs return a `run_validation` refusal. The route also refuses unbounded
requests and invalid output limits before processing the panel. Every structured refusal carries
the schema, workflow, stage, actionable refusal, guarantees, limitations, and a true
`fail_closed` marker.

## Boundaries and composition

This route does not execute a grader, independently verify fixture integrity, inspect hidden
paths, interpret logs, estimate biological truth, calculate clinical validity, or decide release.
It is one evidence cell in a larger evaluation:

- use `bioeval_estimand_audit` to constrain what a claim means and whether its identification and
  transport posture are declared;
- use `bioeval_grounding_audit` to connect claims to support, contradiction, lineage, and locator
  state;
- use `bioeval_acquisition_audit` to inspect whether required information-seeking obligations were
  closed before stopping; and
- use `evaluation_reproduction_check`, benchmark integrity routes, or deployment-specific sandbox
  controls for execution and reproducibility evidence.

The evaluator audit is therefore useful across biological, software, policy, and scientific
domains without pretending that a caller-supplied run record is an independently verified fact.

## SDK surfaces

- Python exposes `BioevalEvaluatorHealthArgs`, `BioevalEvaluatorDiagnosticArgs`,
  `BioevalEvaluatorRunArgs`, `BioevalEvaluatorAuditArgs`, `BioevalEvaluatorAuditReport`, and
  `bioeval_evaluator_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes typed health, diagnostic, run, audit-argument, and audit-result interfaces
  plus `bioevalEvaluatorAudit(...)`. Nested rows, panel counters, and findings remain JSON
  objects so bounded omission metadata and refusal details are not discarded.
