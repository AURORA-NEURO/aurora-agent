# Offline routing lab run

`routing_lab_run` is the transport boundary for the repository's deterministic routing
laboratory. It evaluates a caller-supplied task panel through the real
`bioprism_routing::lab::run` kernel and returns a regret-oriented account, calibration evidence,
holdout posture, and bounded task-level rows. It is deliberately an offline research surface: it
does not select a model, prompt, provider, tool, or production topology, and it cannot perform a
network, filesystem, laboratory, or patient-facing action.

## Why this is a lab, not a win-rate endpoint

Routing is only useful if it is compared against a fixed baseline and an explicit retrospective
ceiling. The kernel evaluates the approved architecture panel for every task, routes with the
declared policy, and keeps the following outcomes separate:

- the router's selected architecture and utility;
- the fixed-default architecture and utility;
- the most-expensive default and oracle comparator outcomes;
- abstention and calibration behavior;
- task-level wins, losses, ties, and regret.

The oracle is retrospective: it sees the complete approved panel after the task outcome is known.
Therefore oracle agreement is a ceiling/comparator signal, not evidence that a deployable policy
can make the same choice. A positive aggregate result does not establish model quality, biological
validity, clinical utility, or production safety.

## Request

```json
{
  "tasks": [
    {
      "task_id": "task-001",
      "world": { "...": "serialized bioprism world" },
      "query": { "...": "serialized bioprism query" }
    }
  ],
  "settings": {
    "policy": { "...": "serialized RoutingPolicy" },
    "fixed_default": "full_context",
    "holdout": "task",
    "calibration_bins": 10
  },
  "include_rows": true,
  "max_rows": 100
}
```

`tasks` must contain 1–256 unique task identifiers. Worlds and queries are parsed by the Rust
domain types; the SDK does not recreate or reinterpret them. `settings` is the serialized
`LabSettings` value and remains authoritative for the approved architecture set, fixed default,
holdout strategy, and calibration bins. The server bounds serialized input to 20,000,000 bytes.

`holdout` supports:

- `task`: route each task only against evidence from other tasks;
- `regime`: route each task against evidence outside its regime, as defined by the kernel.

`include_rows` defaults to `false`. Even when rows are requested, `max_rows` defaults to 100 and
is capped at 1,000. Omitted rows are reported explicitly; they are never silently treated as
zero observations.

## Successful projection

The response uses schema `bioprism-mcp/routing-lab-run/0.1` and includes:

- `tasks`, `holdout`, `holdout_label`, `approved_architectures`, and `fixed_default`;
- `report.account`, the structured regret/accounting projection from the kernel;
- `report.calibration`, retained as calibration evidence rather than a confidence guarantee;
- `report.verdict`, one of `router_loses_to_fixed_default`, `no_achievable_gain`,
  `router_matches_fixed_default`, or `router_beats_fixed_default`;
- `abstention_rate`, `oracle_agreement_rate`, `tasks_won`, `tasks_lost`, and `tasks_tied`;
- `caveats`, bounded `task_rows`, and reconciled `task_rows_omitted`;
- `guarantees` and `limitations` that make the offline and retrospective boundaries visible.

The task outcome counts must reconcile to the task count. A typed SDK parser rejects a report that
violates this invariant, has non-finite rates, uses an unknown verdict, or claims a bounded row
projection without reconciling omitted rows.

## Fail-closed execution refusal

If the kernel cannot construct a complete routing/holdout comparison, the endpoint returns a
structured refusal instead of a partial score:

```json
{
  "ok": false,
  "schema": "bioprism-mcp/routing-lab-run/0.1",
  "stage": "lab_execution",
  "refusal": "...",
  "fail_closed": true,
  "guarantees": [
    "a routing report is not emitted when an architecture outcome is unjudged or holdout evidence cannot be constructed"
  ]
}
```

Malformed transport arguments are rejected before the kernel is called. A refused lab run is not a
zero-gain result and must not be included in a benchmark denominator.

## SDK surfaces

- Python exposes `RoutingLabRunArgs`, `RoutingLabRunReport`, and
  `routing_lab_run_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes `routingLabRun(...)`, `RoutingLabRunArgs`, and `RoutingLabRunResult`.

Pair this surface with `routing_decide` for a single decision and with benchmark/oracle audits for
claim review. The lab is evidence about the supplied task panel and settings only; it is not an
authorization to deploy a routing policy.
