# Inference-lab holdout and rollback audit

`lab_holdout_audit` exposes the append-only holdout and rollback boundary in `bioprism-lab`.
It is the transport surface for the rule that makes self-improvement measurable:

> A score obtained after a holdout has been used to select or search a configuration is not a
> clean measurement, even if the deployment later rolls back.

The endpoint creates validated architecture bundles and empty holdouts, runs an offline operation
program, and returns clean measurements alongside typed contamination refusals. It never executes
an architecture, reads a benchmark, deploys traffic, or changes external state.

## Request

```json
{
  "cost_ceiling": 100,
  "candidates": [
    {
      "id": "v1",
      "components": [
        { "id": "select", "kind": "context_selector" },
        { "id": "run", "kind": "executor" },
        { "id": "stop", "kind": "terminator" }
      ],
      "cost_units": 0
    },
    {
      "id": "v2",
      "derived_from": "v1",
      "components": [
        { "id": "select", "kind": "context_selector" },
        { "id": "run", "kind": "executor" },
        { "id": "stop", "kind": "terminator" }
      ],
      "cost_units": 0
    }
  ],
  "holdouts": [
    { "id": "private-a", "partition": "rotating_private_certification", "query_budget": 4 }
  ],
  "current": "v1",
  "operations": [
    { "kind": "checkpoint", "label": "before-v2" },
    { "kind": "promote", "configuration": "v2", "selected_using": "private-a", "rationale": "won panel" },
    { "kind": "rollback", "checkpoint": "before-v2" },
    { "kind": "measure", "holdout": "private-a", "configuration": "v2", "metric": "admissible_rate", "value": 0.9 }
  ],
  "max_rows": 100
}
```

Candidates are checked for required component kinds, dangling edges, cycles, protected surfaces,
cost ceiling, duplicate ids, and registered parents before the deployment is created. Holdouts
are created with empty exposure ledgers; input JSON cannot inject a pre-burned history. Operations
are bounded to 2,000 and include `checkpoint`, `promote`, `search`, `measure`, and `rollback`.

## Measurement and contamination semantics

`measure` calls the real `HoldoutLedger::measure` with the architecture's resolved lineage. A
successful operation returns `result: clean_measurement` and the serialized `CleanMeasurement`
that the kernel minted. A failed measurement returns `result: measurement_refused`, the typed
error tag, and `fail_closed: true` as a row. It never returns a numeric score in the refusal row.

The important cases remain separate:

- a development partition refuses certification even when untouched;
- a selected or searched configuration refuses later measurement;
- a descendant refuses when a selected/search exposure burned an ancestor;
- a repeated measurement is refused as already queried;
- a retired holdout refuses all later measurements;
- a non-finite score is refused rather than becoming a metric observation.

Measurement refusals do not abort the audit program because the refusal is itself the evidence a
reviewer needs. Structural operation errors—invalid architecture registration, an unknown bundle,
an unknown checkpoint, or a failed rollback—fail closed at the operation boundary instead of
returning a misleading partial deployment report.

## Rollback semantics

A checkpoint records the current complete configuration and each holdout's exposure watermark.
Rollback restores the configuration bundle and returns a receipt with:

- exposure events appended since the checkpoint;
- configurations first burned in that interval;
- holdouts retired during the interval;
- holdouts outside the checkpoint's coverage;
- `complete_restoration`, which is false whenever exposure moved.

The holdout ledger is never rewound. The output's `holdouts` rows retain the complete exposure
events, current watermark, query budget, retirement state, certification status, and partition
reuse note. `permanently_burned` keeps the configurations named by consuming exposure events
visible even after a rollback.

## Successful projection

The response uses schema `bioprism-mcp/lab-holdout-audit/0.1` and includes current configuration,
validated space ids, holdout rows, remaining certification budget, checkpoints, deployment history,
bounded operation rows, measurement/refusal/rollback counts, and guarantees/limitations.
`operations_omitted` reconciles the bounded rows to `operation_count`; a typed SDK parser rejects
that invariant when it is not true.

## Fail-closed behavior

```json
{
  "ok": false,
  "schema": "bioprism-mcp/lab-holdout-audit/0.1",
  "stage": "architecture_validation",
  "refusal": "candidate `v2` declares `benchmark_splits`, which is protected",
  "fail_closed": true
}
```

No deployment state or partial exposure report is emitted when architecture validation or a
structural operation fails. A valid audit with contaminated measurements is different: it succeeds
as an audit and keeps each measurement refusal explicit.

## SDK surfaces

- Python exposes `LabHoldoutAuditArgs`, `LabHoldoutAuditReport`, and
  `lab_holdout_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes `labHoldoutAudit(...)`, `LabHoldoutAuditArgs`, and
  `LabHoldoutAuditResult`.

This is holdout accounting, not a claim that the supplied metric is biologically valid. Pair it
with benchmark integrity/oracle reviews before interpreting a clean point as evidence for a larger
claim.
