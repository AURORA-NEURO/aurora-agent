# Engineering execution plan

`engineering_execution_plan` is the deterministic scheduling layer over the
`engineering_manifest_audit` artifact. It converts a validated package graph and ticket
dependency graph into a bounded implementation window, executable waves, schedule gates, and a
critical path. It is deliberately an analysis route: it does not create tickets, move tickets,
run CI, inspect a checkout, call GitHub, or authorize a release.

## Request

The route accepts one `request` object:

```json
{
  "schema": "bioprism-engineering-plan/0.1",
  "manifest": { "...": "EngineeringManifest" },
  "policies": {
    "require_valid_manifest": true,
    "allow_truncation": false,
    "include_completed": false,
    "serialize_same_package": true,
    "max_tickets": 100,
    "max_parallelism": 16
  }
}
```

The manifest is audited before scheduling. `max_tickets` bounds the selected ticket window and
`max_parallelism` bounds each wave. Ticket IDs are sorted lexicographically before selection, so
the same manifest and policies produce the same window and digest. Completed tickets are excluded
from the schedule by default but remain available to dependency resolution and critical-path
calculation. Same-package serialization is enabled by default to keep package-local work ordered
unless callers explicitly opt into broader parallelism.

## Scheduling semantics

Each selected ticket receives one of these states:

- `complete`: already done and included only when `include_completed` is enabled;
- `blocked`: declared blocked or has a missing dependency;
- `waiting`: all dependencies exist but at least one is unfinished;
- `ready`: all declared dependencies are complete or outside the selected execution window.

The planner emits waves in dependency order. A ticket cannot be scheduled into a wave until its
declared dependencies have either completed before the plan or been scheduled in an earlier wave.
When `serialize_same_package` is true, tickets in the same package are placed in distinct waves.
The wave also records package IDs, dependency wave indices, and actual parallelism. A schedule is
not considered complete when any selected actionable ticket is blocked or waiting.

The critical path is the longest dependency chain among the selected actionable tickets, with
completed prerequisites retained when they explain the chain. It is explanatory evidence, not a
duration estimate: the manifest has no trusted ticket effort, staffing, or elapsed-time field.

## Gates and fail-closed behavior

The audit reports gates for manifest admission, ticket-window policy, dependency closure, and
actionable schedule completeness. A blocking manifest issue stops scheduling when
`require_valid_manifest` is true. Selecting more tickets than `max_tickets` is a blocking issue
unless `allow_truncation` is true, in which case the result carries a warning and an explicit
`truncated` flag. Missing dependency IDs always remain blocking because a partial graph cannot
support a trustworthy schedule.

The top-level result includes `valid`, `engineering_plan_ready`, issue counts, and independent
`manifest_issues` and planner `issues`. `engineering_plan_ready` is true only when the manifest
and all required planning gates pass. The canonical `request_digest`, `manifest_digest`, and
`plan_digest` make the input and derived schedule addressable in logs or later review. A refusal
must be `fail_closed: true`; SDK parsers reject an unmarked refusal.

## Guarantees and non-claims

The implementation guarantees bounded input handling, deterministic ordering, dependency-aware
waves, explicit truncation semantics, and no external side effects. It does not guarantee ticket
freshness, CI success, build success, staffing availability, wall-clock completion, GitHub/Jira
state, or deployment safety. A production execution adapter remains a separate future boundary
requiring authenticated external systems, durable run state, cancellation, resource isolation,
and evidence returned from actual commands.

## SDK surfaces

- Python: `EngineeringPlanRequestArgs`, `EngineeringPlanPoliciesArgs`,
  `engineering_execution_plan_report`, and sync/async Workspace/API-client methods.
- TypeScript: `EngineeringPlanRequestArgs`, `EngineeringPlanToolResult`, and
  `ApiClient.engineeringExecutionPlan(...)`.
- MCP: the `engineering_execution_plan` tool with the
  `bioprism-engineering-plan-audit/0.1` structured result.
