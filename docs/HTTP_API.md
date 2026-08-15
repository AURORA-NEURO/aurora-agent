# HTTP API and event delivery

`bioprism-api` is the network-facing integration layer over the existing Rust MCP server. It is
available as a library (`bioprism_api::ApiRouter`) and as the `bioprism-api` binary:

```bash
cargo run -p bioprism-api -- --root . --bind 127.0.0.1:8787 --token <at-least-16-visible-bytes>
```

The gateway is intentionally bounded and one-request-per-connection. Headers default to 32 KiB,
bodies to 2 MiB, event retention to 4,096 entries, and every route reports an `X-Request-Id`.
When configured, bearer authentication protects every route except `/healthz`, `/readyz`, and the
OpenAPI document. The server inherits MCP root confinement for every tool that reads a path.

## Routes

| Route | Purpose |
|---|---|
| `GET /healthz`, `GET /readyz` | Liveness/readiness and retention metrics |
| `GET /v1/capabilities` | Tool/resource counts, transport support, limits, and workspace catalogue |
| `GET /v1/tools` | The exact MCP tool definitions |
| `POST /v1/tools/{name}` | Call any tool with a JSON object body; delegates to the MCP dispatcher |
| `POST /v1/missions` | Validate and submit an asynchronous `agent_mission` job |
| `GET /v1/missions/{mission_id}` | Poll job state and retrieve the authoritative mission report |
| `POST /v1/missions/{mission_id}/cancel` | Request cooperative cancellation between nested calls/batches |
| `DELETE /v1/missions/{mission_id}` | Remove a terminal job from the bounded in-process registry |
| `POST /v1/rpc` | JSON-RPC/MCP-compatible request envelope for tools, resources, and lifecycle |
| `GET /v1/events?after=N&limit=M` | Cursor page with retention-gap and dropped-event evidence |
| `GET /v1/events/stream?after=N&limit=M` | A bounded Server-Sent Events snapshot, not an unbounded connection |
| `GET /v1/webhooks/subscriptions` | Subscription catalogue with secrets omitted |
| `POST /v1/webhooks/subscriptions` | Register an `http(s)` endpoint, event filters, and signing secret |
| `GET .../{id}/deliveries` | Poll signed pending envelopes by delivery cursor |
| `POST .../{id}/retry` | Increment an attempt and recompute the signature, bounded at ten attempts |
| `POST .../{id}/ack` | Idempotently remove acknowledged deliveries |
| `DELETE .../{id}` | Remove a subscription and its pending outbox |

REST and MCP calls share the same in-process `bioprism-mcp::Server`. Every tool call emits a
`tool.completed`, `tool.refused`, or `tool.rpc_error` event. Large responses are replaced in the
event payload by byte count and SHA-256, so observability cannot silently turn into an unbounded
memory sink. `agent_mission` reports include a clock-free `execution_trace`; when its raw response
is omitted for size, the event retains a bounded mission-trace projection with lifecycle, refusal,
block, digest, and byte-accounting evidence.

## Asynchronous missions

`POST /v1/missions/preflight` is the synchronous no-dispatch planning endpoint. It accepts the
same mission document, validates the original execution policy (including the allow-list) and
static tool schemas, then returns the authoritative `agent_mission` plan with
`preflight: true`, `execution: "planned"`, and `dispatch: "not_started"`. It never creates a
job, records an execution event, or invokes a nested domain tool. Binding-dependent arguments
are represented in the plan and are validated again only when an execution request materializes
them.

`GET /v1/missions?status=<status>&limit=<n>` lists at most 256 process-local jobs in deterministic
mission-id order. It returns lifecycle links and bounded summaries (`total_steps`, completed and
refused counts, cancellation counts, required failures, and returned bytes), never the full raw
terminal report. Unknown query keys and statuses are refused so an operator cannot mistake a
partial inventory request for a complete one.

`POST /v1/missions` accepts the same JSON object as the `agent_mission` tool and returns `202`
after the complete mission graph, policy, allow-list, and safety bounds have passed validation.
Validation includes bounded authoritative JSON Schema preflight against the live `tools/list`
definitions for every static step. Steps with RFC 6901 bindings are checked again after their
upstream payloads are materialized, before serial or parallel nested dispatch. A schema refusal
contains a schema digest and bounded JSON-pointer diagnostics; it is distinct from a refusal
returned by the domain tool itself. Invalid submissions receive `422` and never enter the job
registry.
The response contains `mission_id`, `status: "queued"`, and poll/cancel links. `GET` returns
`queued`, `running`, or a terminal status (`planned`, `succeeded`, `partial`, `failed`, or
`cancelled`) plus the raw authoritative report once available. Mission IDs are unique within the
process and the bounded in-memory registry holds at most `MAX_MISSION_JOBS` entries.
Terminal jobs can be removed with `DELETE`; active jobs are refused with `409` so cleanup cannot
silently discard work that may still dispatch.

Cancellation is deliberately cooperative: the API sets a shared cancellation flag, the executor
checks it before dispatching each serial step and parallel batch, and already-running nested tools
are allowed to return. A cancelled report includes `cancelled` step results and a closed trace with
`mission.cancelled` before `mission.completed`; it never pretends that an interrupted in-flight
effect was rolled back. This is an in-process operational surface, not a durable queue, distributed
worker scheduler, or force-kill guarantee.

## Cursor and webhook guarantees

Event IDs and delivery IDs are monotonically increasing process-local cursors. If retention has
discarded entries between a consumer's `after` cursor and the oldest retained entry, the response
sets `gap: true` and reports `dropped_events`; it never presents a partial history as complete.

Webhook records are signed HMAC-SHA256 envelopes. The secret is accepted at registration but is
never returned. A delivery worker should send the envelope, retain the `delivery_id`, retry only
when its own transport policy permits, and acknowledge exactly the IDs it has durably accepted.
The gateway provides the outbox and signature; it does not open arbitrary outbound sockets,
execute a scheduler, or claim delivery success merely because an envelope was created.

## Explicit nonclaims

The dependency-free boundary does not implement HTTP/2 gRPC, TLS termination, an identity provider,
durable event storage, or a consumer-repository GitHub Action. Those are deployment/artifact
surfaces. The `capabilities` response reports these as false so clients can route to an operator's
proxy or delivery worker instead of inferring them from the REST routes.

The Python standard-library SDK exposes the same HTTP surface through `prism_sdk.ApiClient` and
`prism_sdk.AsyncApiClient`; the stdio MCP client remains available for local, process-confined
workflows.

## Verification

The Rust crate tests parser bounds, duplicate-header rejection, REST/MCP dispatch parity, auth,
cursor gaps, signature generation, retry/ack lifecycle, and secret non-disclosure. Python tests
exercise the synchronous and asynchronous HTTP clients against a bounded local server.
