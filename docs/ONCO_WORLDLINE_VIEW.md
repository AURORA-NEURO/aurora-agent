# Oncology Worldline View

`onco_worldline_view` is the read-only audit projection for a serialized `TumourWorldline`. It
does not infer a disease trajectory, repair timestamps, or decide a clinical endpoint. Its job is
to preserve the distinction between when a biological observation happened, when it was recorded,
when it was released, and when an evaluated agent could see it.

The successful projection is versioned as:

```text
bioprism-mcp/onco-worldline-view/0.1
```

## Four clocks

Each `timepoints[]` row contains both a nested authoritative `clocks` object and the four flat
clock fields retained for ergonomic consumers:

| Field | Meaning | Permitted ordering role |
| --- | --- | --- |
| `acquired` | Event-validity or biological acquisition time | Defines `biological_order` and `days_from_baseline` |
| `recorded` | Time the observation entered the record system | Defines `record_order` |
| `released` | Time the custodian released the record | Precondition for agent visibility |
| `visible` | Time the evaluated agent could receive the evidence | The only clock used by the visibility firewall |

The server validates the dependency chain while constructing the source worldline. The response
repeats `clock_axes` and `clock_order_guaranteed` so clients can refuse a projection that has lost
the clock contract. The SDK also rejects a row when its flat clock copies disagree with its nested
`clocks` object.

## Two orderings

`biological_order` is acquisition order and is the only order suitable for baseline-relative
trajectory reasoning. `record_order` sorts the same labels by recording time and exists for
reporting-lag and ingestion analysis. Each row carries `biological_index` and `record_index`; the
SDK requires each index set to be a complete permutation of `0..timepoint_count-1` and reconciles
both index projections against their label arrays.

`record_order_differs` is derived evidence, not an independently trusted claim. A typed client
refuses it when it does not equal the comparison of the two order arrays.

## Visibility firewall

When `visible_at` is supplied, the response contains a complete disjoint partition:

```json
{
  "visibility_partition": {
    "cutoff": "2026-01-10T12:00:00Z",
    "filter_applied": true,
    "visible": ["future"],
    "hidden": ["baseline"],
    "visible_count": 1,
    "hidden_count": 1
  }
}
```

The flat `visible_timepoints`, `hidden_from_agent`, `visible_count`, and `hidden_count` fields
remain available for existing callers. They must agree with the nested partition. Each row also
reports `visibility_state` (`visible`, `hidden_from_agent`, or `not_filtered`) and
`visible_at_cutoff` (`true`, `false`, or `null`). This makes “performed but not yet released”
evidence distinguishable from evidence that was never supplied.

Without a cutoff, both partition sides and both counts are `null`, and rows are explicitly marked
`not_filtered`. A client must not silently interpret an unfiltered worldline as an agent-visible
worldline.

## Python SDK

`OncoWorldlineReport.timepoint_records` exposes typed `OncoTimepointProjection` rows. Each row has
an `OncoClockProjection` and retains its raw mapping for forward-compatible fields. The report also
exposes `visibility_partition`, `baseline_biological_index`, `baseline_record_index`, and the
reconciled counts. Forged order indices, clock copies, partition membership, or counts raise
`ArgumentError` rather than becoming a plausible report.

```python
from prism_sdk import onco_worldline_report

report = onco_worldline_report(response)
future = report.timepoint_records[1]
assert future.clocks.axes == ("acquired", "recorded", "released", "visible")
assert future.visibility_state == "visible"
assert report.visibility_partition.hidden == ("baseline",)
```

The projection is transport-agnostic: direct MCP structured content, the HTTP envelope, and JSON
text content are normalized before the same reconciliation rules run.

## Scope and limitations

The view audits a supplied worldline; it does not impute missing observations, quantify ambiguous
dates, model treatment interruptions, establish specimen lineage, or make a patient-level clinical
recommendation. Exact timestamps remain exact source claims and must not be promoted into a broader
clinical conclusion without an appropriate analysis contract.
