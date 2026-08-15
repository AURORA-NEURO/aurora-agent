# Python SDK

The repository ships `python/prism_sdk`, a standard-library client for the Rust MCP server. It is
the integration layer above the deterministic kernel described by [ADR-001](ADR-001-language-strategy.md):
Python can orchestrate and author requests, while Rust remains the owner of canonical bytes,
domain invariants, release gates, and evidence semantics.

## Lifecycle

Both clients enforce the MCP sequence:

```text
construct argv -> start child without a shell -> initialize -> notifications/initialized
       -> tools/list / tools/call / resources/read -> close stdin -> terminate or kill child
```

`Client` is synchronous and `AsyncClient` uses `asyncio.create_subprocess_exec`. Neither accepts a
shell command string. The caller supplies an argv sequence, an optional working directory, and an
optional environment overlay.

Every frame is UTF-8 JSON followed by one newline. Outbound and inbound frames have a default
20 MB bound, and every response has a finite timeout. A malformed frame, mismatched request id,
missing result, unexpected EOF, or process exit is a protocol/transport failure—not an empty tool
result.

## Error classes

The package distinguishes:

| Class | Meaning | Retry implication |
|---|---|---|
| `ArgumentError` | local argument or frame bound failure | fix the call; no request was sent |
| `LifecycleError` | start/initialize ordering error | fix client usage |
| `TransportError` / `ProcessExited` | process or stdio failure | retry only after inspecting process state |
| `ResponseTimeout` | peer exceeded the configured bound | retry only if the operation is safe to repeat |
| `ProtocolError` | peer violated JSON-RPC/MCP shape | do not interpret the result as evidence |
| `RemoteError` | JSON-RPC method-level error | use `code`, `message`, and `data` |
| `ToolRefusal` | valid tool payload with `ok: false` or `isError` | preserve the refusal; do not treat it as success |
| `ApiError` | HTTP gateway returned a bounded structured error | inspect status/payload; do not retry a domain refusal blindly |

`ToolResult` retains the raw MCP envelope, exposes all text blocks, decodes the server's JSON
projection, and provides `require_ok()` for callers that explicitly want an exception on a refusal.
Callers that need to render partial evidence can use `value()` without discarding the raw envelope.

## Domain helpers

`Workspace` and `AsyncWorkspace` are thin, typed facades. They do not duplicate domain models or
invent defaults:

- `developer_delivery_audit(...)` forwards exact nested evidence and creates a release request only
  when the caller supplies an id and target list.
- `bioatlas_publication_audit(atlas, ...)` preserves optional evidence, card, leaderboard, and
  weighting contracts and never implies publication without explicit targets.
- `compile_context(world, query, ...)` invokes `fiber_compile` while leaving policy/profile choices
  caller-controlled.
- `trace_otel_ingest(trace_id, otlp_json=... | document=..., ...)` invokes the bounded OTLP JSON
  importer and preserves its semantic-loss/readiness report.
- `tool(name, arguments)` remains available for every current and future MCP domain.

`ApiClient` and `AsyncApiClient` provide the same standard-library SDK posture for the HTTP
gateway: health/capability discovery, REST tool calls, cursor-based event pages, and signed
webhook subscription/delivery acknowledgement. They preserve status and JSON error payloads in
`ApiError` and do not recreate Rust domain semantics.

The package deliberately does not claim to implement DICOM/NIfTI/AnnData/VCF readers, benchmark
statistics, OTLP export, a notebook UI, or CI deployment. The repository now ships a bounded REST
gateway, but gRPC, durable event storage, external webhook delivery, and those domain artifacts
remain separate contracts.

## Verification

From the repository root:

```bash
cd python
python -W error::ResourceWarning -m unittest discover -s tests -v
python -m compileall -q prism_sdk tests
```

The tests use a subprocess fake MCP peer so lifecycle, framing, protocol, remote errors, structured
refusals, sync/async parity, and cleanup are exercised through actual pipes rather than direct
function calls.
