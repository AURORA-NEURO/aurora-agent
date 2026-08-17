# HTTP API and event delivery

`bioprism-api` is the network-facing integration layer over the existing Rust MCP server. It is
available as a library (`bioprism_api::ApiRouter`) and as the `bioprism-api` binary:

```bash
cargo run -p bioprism-api -- --root . --bind 127.0.0.1:8787 --token <at-least-16-visible-bytes> \
  --mission-state .local/mission-state.json --event-state .local/event-state.json
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
| `GET /v1/recovery` | One operator-visible matrix of restart, secret, outbox, delivery-provenance, and external-effect boundaries |
| `GET /v1/tools` | The exact MCP tool definitions |
| `POST /v1/tools/{name}` | Call any tool with a JSON object body; delegates to the MCP dispatcher |
| `POST /v1/missions` | Validate and submit an asynchronous `agent_mission` job |
| `GET /v1/missions/persistence` | Inspect bounded checkpoint configuration and on-disk size |
| `POST /v1/missions/persistence/flush` | Force a checkpoint and verify it can be written |
| `GET /v1/missions/{mission_id}` | Poll job state and retrieve the authoritative mission report |
| `GET /v1/missions/{mission_id}/trace` | Page retained clock-free mission lifecycle events |
| `POST /v1/missions/{mission_id}/cancel` | Request cooperative cancellation between nested calls/batches |
| `DELETE /v1/missions/{mission_id}` | Remove a terminal job from the bounded in-process registry |
| `POST /v1/rpc` | JSON-RPC/MCP-compatible request envelope for tools, resources, and lifecycle |
| `GET /v1/events?after=N&limit=M&review_id=H` | Cursor page with retention-gap and dropped-event evidence; optional exact route-review or receipt filter |
| `GET /v1/events/stream?after=N&limit=M&review_id=H` | A bounded Server-Sent Events snapshot, optionally filtered by an exact route-review or receipt id |
| `GET /v1/route-reviews/{review_id}/evidence?after=N&limit=M` | Typed retained route-review evidence lookup |
| `GET /v1/events/persistence` | Inspect bounded event cursor checkpoint status |
| `POST /v1/events/persistence/flush` | Force an event cursor checkpoint and verify its write |
| `GET /v1/webhooks/subscriptions` | Subscription catalogue with secrets omitted |
| `POST /v1/webhooks/subscriptions` | Register an `http(s)` endpoint, event filters, and signing secret |
| `GET .../{id}/deliveries` | Poll signed pending envelopes by delivery cursor |
| `GET .../{id}/attempts` | Poll durable send/retry/replay/acknowledgement provenance by attempt cursor |
| `POST .../{id}/retry` | Increment an attempt and recompute the signature, bounded at ten attempts |
| `POST .../{id}/ack` | Idempotently remove acknowledged deliveries |
| `POST .../{id}/replay` | Reset selected deliveries to attempt one after operator review |
| `POST .../{id}/rebind` | Supply a secret in memory and re-sign restored pending envelopes |
| `DELETE .../{id}` | Remove a subscription and its pending outbox |

REST and MCP calls share the same in-process `bioprism-mcp::Server`. Every tool call emits a
`tool.completed`, `tool.refused`, or `tool.rpc_error` event. Large responses are replaced in the
event payload by byte count and SHA-256, so observability cannot silently turn into an unbounded
memory sink. `agent_mission` reports include a clock-free `execution_trace`; when its raw response
is omitted for size, the event retains a bounded mission-trace projection with lifecycle, refusal,
block, digest, and byte-accounting evidence.
Asynchronous jobs additionally emit one `mission.trace` event per retained trace event, with the
mission id as subject and the exact trace row under `payload.trace`. Those events enter the same
cursor, SSE, and signed webhook outbox as ordinary tool events, so an operator can monitor any
domain mission without inventing a domain-specific subscription path.

The TCP serving path shares an immutable router across connection threads and allocates request IDs
atomically. Each stateless REST/JSON-RPC dispatch uses a cloned ready MCP session; mutable mission,
event, subscription, and delivery state remains independently bounded and synchronized.

## Restart-aware mission snapshots

Pass `--mission-state <file>` to enable an optional, atomically replaced JSON checkpoint for the
asynchronous mission registry. The snapshot is bounded to 64 MiB, keeps at most 4,096 missions and
4,096 trace rows per mission, and omits terminal result bodies larger than 256 KiB in favour of
byte-count and SHA-256 metadata. This makes operator restarts inspectable without turning a local
gateway into an unbounded database.

On startup, terminal jobs are restored with their retained progress, trace, error, and (when within
the result bound) authoritative report. A queued or running job is never falsely resumed: it is
converted to `failed`, marked `recovered_after_restart: true`, and reports that execution was not
resumed. `GET /v1/missions/{mission_id}` exposes `result_omitted` when a result body was not
retained. Snapshot writes happen after acceptance, trace observation, cancellation, terminal
completion, and deletion; a write failure rejects new acceptance and leaves existing process state
intact. The snapshot is not a distributed queue: it restores bounded event rows, subscription
metadata, and signed pending outbox envelopes, but signing secrets remain process-local.
`GET /v1/missions/persistence` reports whether checkpointing is enabled, the current file size,
registry size, bounds, content `state_digest`, and the non-durable event/delivery distinction.
The writer emits mission-state schema 2 and verifies its lowercase SHA-256 digest before restoring
any job; status also reports `integrity_verified`. Schema 1 snapshots remain readable as
migration inputs and are upgraded on the next successful checkpoint. The authenticated
`POST /v1/missions/persistence/flush` route gives operators an explicit write/readiness check;
it returns `409` when no state path was configured and `503` when the checkpoint cannot be written.

Pass `--event-state <file>` to checkpoint the retained event cursor plus bounded subscription and
outbox metadata as a separate JSON document. It restores event IDs, retention-gap accounting,
retained tool/mission events, subscription endpoint/filter declarations, and signed pending
envelopes, but never writes webhook secrets. Restored subscriptions are paused and delivery rows
report `secret_rebind_required` until `POST /v1/webhooks/subscriptions/{id}/rebind` receives a
fresh secret in memory; rebind re-signs pending envelopes and reactivates that subscription.
`GET /v1/events/persistence` and its authenticated `POST /v1/events/persistence/flush` counterpart
expose the file bound, cursor metrics, durability fields, and explicit secret policy.
Both persistence status responses include `integrity_verified`: `true` means the current-schema
file digest matches at observation time, `false` means a present current-schema file is malformed
or tampered, and `null` means no file or a legacy schema without a digest.
The writer currently emits event-state schema 4, which binds every persisted field except the
digest itself to a lowercase SHA-256 `state_digest`; startup rejects a modified, truncated, or
partially rewritten current or schema-3 document before restoring any rows. Schema 1 and schema 2
snapshots remain readable as bounded migrations without a digest, and the next successful flush
upgrades them to schema 4. Schema 4 additionally persists a bounded delivery-attempt journal;
each row records the signed envelope identity and local outcome classification, while
`receiver_accepted` is only true when the operator-owned sender reports success. Event persistence
and mission persistence are independent: an operator can enable either, both, or neither.

`GET /v1/recovery` joins those independent statuses without joining their guarantees. Its
`boundaries` rows separately identify mission jobs, event rows, subscription metadata, webhook
outbox rows, delivery-attempt provenance, signing secrets, and external delivery effects. Each row reports whether the
boundary is configured, whether a checkpoint is present, what is restored, what is explicitly not
restored, whether the observed digest was verified, and the required operator action. `automatic_resume` and
`automatic_external_delivery` are always false for this gateway. The matrix is an operational
decision surface, not a distributed coordination protocol.

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

Every asynchronous mission response also carries a bounded `progress` projection. It exposes the
current phase, wave, total/completed/active step counts, outcome counts, returned bytes, the latest
clock-free trace sequence, and the latest trace event. The projection is updated from authoritative
mission trace events while work is running and reconciled from the terminal report before that
report is returned, so dashboards can use one shape for queued, live, and terminal jobs. It is an
operational view only: the terminal mission report and its trace remain authoritative for replay,
content identity, and domain interpretation.

`GET /v1/missions/{mission_id}/trace?after=<n>&limit=<n>` returns at most 1,000 retained
clock-free trace events. The first request uses `after=0`; the response's `next_after` is the
exclusive cursor for the next request. `gap` and `dropped_events` are explicit if the bounded trace
retention window no longer contains the requested prefix. Trace retrieval is available while the
mission is running and after it is terminal; it is a replay/observability surface, not a second
execution result.

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

Event pages and SSE snapshots accept either `review_id` or `receipt_id` as an exact, mutually
exclusive filter. Delivery receipts also expose
`GET /v1/delivery-receipts/{receipt_id}/events?after=...&limit=...`, which returns a typed
`developer_delivery_receipt_events` page with the same cursor and retention-gap semantics. Receipt
events retain a bounded projection (`receipt_id`, digest, readiness, and verification dimensions)
even when the complete tool response exceeds the event payload bound; the projection is a join key,
not proof of receipt validity.

Webhook records are signed HMAC-SHA256 envelopes. The secret is accepted at registration but is
never returned. A delivery worker should send the envelope, retain the `delivery_id`, retry only
when its own transport policy permits, and acknowledge exactly the IDs it has durably accepted.
The gateway provides the outbox and signature; it does not open arbitrary outbound sockets,
execute a scheduler, or claim delivery success merely because an envelope was created.
Embedded Rust deployments can use `bioprism_api::DeliverySender` and
`ApiRouter::deliver_once(...)` for the same bounded cycle: the callback receives the endpoint and
already-signed envelope, while the router acknowledges successes, advances retryable attempts up
to ten, leaves secret-unbound rows blocked, and leaves permanent/exhausted failures pending with a typed `DeliveryRunReport`. The
callback still owns HTTP/TLS, egress policy, and transport classification.
Each delivery row also carries `state` (`pending`, `retryable`, `failed`, `exhausted`, or
`secret_rebind_required`),
`last_error`, and `last_error_retryable`. A failed row remains pending for inspection; replay is
an explicit bounded reset that keeps the delivery ID stable for receiver idempotency, resets the
attempt to one, re-signs the envelope, and clears the prior failure. It never creates an
unbounded duplicate or treats operator intent as a successful send.
`GET /v1/webhooks/subscriptions/{id}/attempts?after=N&limit=M` returns the bounded durable
attempt journal. Each row identifies the delivery, event, signed-envelope identity, local action,
outcome, retryability, bounded error, and whether the operator-owned sender explicitly reported
receiver acceptance. `gap` and `dropped_attempts` make journal retention loss visible. The journal
is evidence of gateway/worker observations, not a claim that an external receiver committed an
effect beyond the sender's explicit success result.

## Explicit nonclaims

The dependency-free boundary does not implement HTTP/2 gRPC, TLS termination, an identity provider,
distributed event storage, a distributed mission queue, or a consumer-repository GitHub Action.
Those are deployment/artifact surfaces. The optional event snapshot is bounded local recovery, not
a consensus log or distributed delivery service; the mission snapshot restores bounded mission
state and explicitly fails interrupted work instead of claiming recovery. The `capabilities`
response reports these distinctions so clients can route to an operator's proxy, queue, or delivery
worker instead of inferring them from REST routes.

The Python standard-library SDK exposes the same HTTP surface through `prism_sdk.ApiClient` and
`prism_sdk.AsyncApiClient`; the stdio MCP client remains available for local, process-confined
workflows.

## Verification

The Rust crate tests parser bounds, duplicate-header rejection, REST/MCP dispatch parity, auth,
cursor gaps, signature generation, retry/ack lifecycle, and secret non-disclosure. Python tests
exercise the synchronous and asynchronous HTTP clients against a bounded local server.
