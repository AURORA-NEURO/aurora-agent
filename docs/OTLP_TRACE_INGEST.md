# OTLP trace ingestion

`trace_otel_ingest` is the local OpenTelemetry boundary for the trajectory and Decision Cell
pipeline. It accepts a recorded OTLP/JSON export and converts `resourceSpans -> scopeSpans ->
spans` into the existing `bioprism-trace::Event` IR. It is intentionally an importer: it does not
open a collector connection, export telemetry, read the clock, or infer benchmark success from
telemetry status.

## Input contract

The MCP tool accepts exactly one of:

- `otlp_json`: inline OTLP JSON;
- `document`: a root-confined OTLP JSON file.

Every request is bounded by `max_bytes` (default 10,000,000; maximum 10,000,000) and `max_spans`
(default 100,000; maximum 100,000). `max_items` bounds the optional event preview. The caller must
also supply `trace_id`; `succeeded` is caller-owned and defaults to `false`.

The adapter accepts both current `scopeSpans` and the older
`instrumentationLibrarySpans` spelling. Each valid span must provide `traceId`, `spanId`, and
`name`. Invalid spans are reported as dropped rather than guessed into an observation.

## Mapping

| OTLP input | Event IR result |
|---|---|
| span order by `startTimeUnixNano`, then source order | `Event.step` |
| `parentSpanId` resolving to an earlier span | `Event.caused_by` |
| `name` plus `prism.event.kind` / `aurora.event.kind` | `Event.kind` |
| resource and span attribute keys | `Event.visible` |
| normalized attributes, status, links, span events, and IDs | `Event.payload` |
| complete original span object | `Event.payload.raw_span` |

Explicit event kinds are `goal`, `observation`, `choice`, `action`, `result`, `claim`, and
`termination`. Without an explicit Prism/Aurora kind, the adapter uses a transparent name-based
preview but records `inferred_kinds`; that trace is not lossless or compilable. This keeps a useful
inspection result available without treating a vendor span name as an authoritative decision label.

OTLP typed values are normalized for strings, booleans, integer/double values, bytes, arrays, and
key-value lists. Duplicate attribute keys remain in source order and are reported. Unknown fields
are retained in `raw_span` but recorded as uninterpreted semantics.

## Loss and readiness

The response includes:

- `mapping`: resource, scope, source-span, accepted-span, and span-event counts;
- `loss`: dropped spans/events, uninterpreted fields, duplicate attributes, inferred kinds,
  missing timestamps, unresolved parents, and multiple source trace IDs;
- `lossless`: true only when none of those semantic or structural gaps exist;
- `compilable`: true only for a non-empty, lossless, structurally valid Event IR trace;
- `trace_sha256`: the digest of the normalized trace;
- optional bounded `events` when `include_events` is true.

Missing parent spans, parents that sort after their child, missing start timestamps, non-empty span
links, multiple source trace IDs, unknown attribute/value shapes, and unsupported fields all remain
visible. A trace can therefore be useful for diagnosis while still refusing a Decision Cell compile.

## Python facade

The dependency-free Python SDK exposes the same contract through
`Workspace.trace_otel_ingest(...)` and `AsyncWorkspace.trace_otel_ingest(...)`. Both require
exactly one inline JSON string or root-confined document path and preserve the server's structured
loss/readiness result.

## Verification

The adapter has unit coverage for explicit kinds, parent resolution, typed values, span events,
source preservation, inferred semantics, missing timestamps, duplicate attributes, multi-trace
exports, malformed roots, and span bounds. MCP protocol coverage exercises both successful mapping
and fail-closed ambiguity/bound checks.
