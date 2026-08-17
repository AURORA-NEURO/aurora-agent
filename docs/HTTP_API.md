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
| `GET /v1/operations/snapshot?after=N&limit=M` | One bounded operator control-plane snapshot joining event, mission, persistence, recovery, and capability summaries |
| `GET /v1/operations/domains?after=N&limit=M` | Per-domain catalogue coverage plus exact local tool activity observed in the requested event page |
| `GET /v1/operations/gates?after=N&limit=M` | Per-domain catalogue, activity, transport, pooled evaluation, domain-evaluator, safety, and release evidence gates without readiness claims |
| `POST /v1/operations/handoff` | Build a content-addressed, non-executing domain-to-`capability_route` handoff |
| `GET /v1/tools` | The exact MCP tool definitions |
| `POST /v1/tools/{name}` | Call any tool with a JSON object body; delegates to the MCP dispatcher |
| `POST /v1/missions` | Validate and submit an asynchronous `agent_mission` job |
| `GET /v1/missions/persistence` | Inspect bounded checkpoint configuration and on-disk size |
| `POST /v1/missions/persistence/flush` | Force a checkpoint and verify it can be written |
| `GET /v1/missions/{mission_id}` | Poll job state and retrieve the authoritative mission report |
| `GET /v1/missions/{mission_id}/provenance` | Retrieve retained gate, review, domain-evaluator, and accepted-dispatch evidence |
| `GET /v1/missions/{mission_id}/claims` | Retrieve the bounded claim-to-step evidence-lineage projection |
| `GET /v1/missions/{mission_id}/evaluator-replay` | Retrieve durable full or summary-only evaluator replay evidence |
| `GET /v1/missions/{mission_id}/evaluator-replay/compare` | Compare retained evaluator provenance with the current catalogue |
| `GET /v1/missions/{mission_id}/evidence-bundle` | Export a bounded, content-addressed mission evidence bundle |
| `GET /v1/missions/{mission_id}/trace` | Page retained clock-free mission lifecycle events |
| `POST /v1/missions/{mission_id}/cancel` | Request cooperative cancellation between nested calls/batches |
| `DELETE /v1/missions/{mission_id}` | Remove a terminal job from the bounded in-process registry |
| `POST /v1/rpc` | JSON-RPC/MCP-compatible request envelope for tools, resources, and lifecycle |
| `GET /v1/events?after=N&limit=M&review_id=H` | Cursor page with retention-gap and dropped-event evidence; optional exact route-review or receipt filter |
| `GET /v1/events/stream?after=N&limit=M&review_id=H` | A bounded Server-Sent Events snapshot, optionally filtered by an exact route-review or receipt id |
| `GET /v1/route-reviews/{review_id}/evidence?after=N&limit=M` | Typed retained route-review evidence lookup |
| `GET /v1/delivery-receipts/{receipt_id}/attempts?after=N&limit=M` | Exact receipt-correlated delivery-attempt provenance |
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
byte-count, SHA-256 metadata, and a compact evaluator replay summary when the report is an
`agent_mission`. This makes operator restarts inspectable without turning a local gateway into an
unbounded database.

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

When a terminal mission result is omitted, `GET /v1/missions/{mission_id}/evaluator-replay` can
still return a bounded structural `mission_evaluator_replay_summary` through
`retention.mode: "summary_only"`; it never reconstructs raw output or executes an evaluator.
The sibling `/evaluator-replay/compare` route compares the retained historical catalogue digest
and referenced adapter IDs with the current catalogue. It reports `unchanged`, `drifted`,
`*_with_missing_bindings`, `not_recorded`, and invalid-digest states without claiming an exact
historical row diff when only the digest was retained. `/evidence-bundle` packages the mission
status, retention/omission metadata, optional result and trace, replay projection, catalogue-drift
comparison, execution provenance, links, and a SHA-256 `bundle_digest` into one bounded export.
Neither route executes a domain tool or evaluator; the evidence bundle rejects oversized serialized
exports with `413` rather than silently truncating them.

Pass `--event-state <file>` to checkpoint the retained event cursor plus bounded subscription and
outbox metadata as a separate JSON document. It restores event IDs, retention-gap accounting,
retained tool/mission events, subscription endpoint/filter declarations, and signed pending
envelopes, but never writes webhook secrets. Restored subscriptions are paused and delivery rows
report `secret_rebind_required` until `POST /v1/webhooks/subscriptions/{id}/rebind` receives a
fresh secret in memory; rebind re-signs pending envelopes and reactivates that subscription.
`GET /v1/events/persistence` and its authenticated `POST /v1/events/persistence/flush` counterpart
expose the file bound, cursor metrics, delivery-attempt and receipt-metadata durability fields,
and explicit secret policy.
Both persistence status responses include `integrity_verified`: `true` means the current-schema
file digest matches at observation time, `false` means a present current-schema file is malformed
or tampered, and `null` means no file or a legacy schema without a digest.
The writer currently emits event-state schema 5, which binds every persisted field except the
digest itself to a lowercase SHA-256 `state_digest`; startup rejects a modified, truncated, or
partially rewritten current or content-addressed legacy document before restoring any rows. Schema 1
and schema 2 snapshots remain readable as bounded migrations without a digest, while schema 3 and
schema 4 retain their digest checks; the next successful flush upgrades every legacy version to
schema 5. Schema 4 added a bounded delivery-attempt journal, and schema 5 additionally persists
validated receipt IDs and content digests on receipt-bearing attempts;
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

## Operations snapshot

`GET /v1/operations/snapshot` is the dashboard/bootstrap surface for operators that need one
consistent read across domains. `after` defaults to `0`; `limit` defaults to `100` and is bounded
to `256`. Unknown query keys and limits outside that range are refused. The response contains:

- `recent_events`, a normal cursor page with `next_after`, `gap`, and dropped-row accounting;
- `event_metrics` and bounded mission `status_counts`, including recovered and cancellation
  counts without returning terminal reports;
- nested mission and event persistence status with file size, schema, digest, integrity, and
  durability fields;
- the complete `/v1/recovery` matrix and a compact capability/transport summary; and
- `domain_coverage`, an exact bounded comparison of workspace capability groups against advertised
  MCP tool names, including per-group missing tools, aggregate gaps, and omission counts; and
- a `consistency` declaration that makes the read model's clock-free, non-atomic cross-store
  composition explicit; and
- `operator_actions`, `guarantees`, and `non_claims` that keep follow-up work and absent
  guarantees visible to dashboards and handoff tooling.

The snapshot is clock-free and read-only. It does not dispatch a tool, resume a mission, send a
webhook, establish receiver acceptance, or validate a scientific/clinical claim. Consumers should
store `recent_events.next_after` as their next cursor and treat `recent_events.gap: true` as an
explicit retention boundary rather than silently treating the page as complete history.

`POST /v1/operations/handoff` accepts an optional `goal`, `domains`, `group_ids`,
`include_complete`, and `max_groups` body. Selectors are normalized and intersected when both
domains and group IDs are provided. The response returns selected groups, exact catalogue gaps,
unresolved selectors, a ready-to-submit `route_request`, a content-addressed `handoff_id`, and a
status of `ready_for_capability_route`, `requires_catalogue_review`, `no_actionable_gaps`, or
`unresolved_domain`. The endpoint is deliberately non-executing: callers must submit the route to
`capability_route`, review explicit selections with `capability_route_review`, and run
`/v1/missions/preflight` before any mission dispatch.

`GET /v1/operations/domains` applies the same cursor bounds to a per-domain activity projection.
Each group retains catalogue counts, missing names, observed event/tool counts, the last observed
event ID, and an `activity_state` of `catalogue_gap`, `observed_in_page`, or
`catalogued_unobserved_in_page`. Matching is exact on the tool name carried by a retained event;
the response declares that activity is limited to the requested page and never calls it runtime
health or readiness.

`GET /v1/operations/gates` applies the same cursor bounds to an evidence-gate projection. Each
group keeps catalogue, observed-activity, transport-completion, pooled evaluation,
domain-evaluator, safety, and release gates separate, including the exact observed tools and
refusal/completion counts. Domain-evaluator evidence is only a completed evaluation-channel
tool with an exact or catalogue-declared capability-group binding; it does not claim evaluator
validity, calibration, independence, or scientific adequacy. The overall
state is `catalogue_blocked`, `insufficient_evidence`, or `review_required`; even a complete
local evidence set requires domain-authority review and is returned with `readiness_claimed: false`.
The channel classifier is an exact, documented tool-name projection; it does not execute tools or
infer scientific validity, clinical safety, or deployment authorization.
Evaluation, safety, and release channels are pooled from completed control-plane tools across the
requested bounded event page and then shown on each matched domain group; catalogue, observed
activity, and transport completion remain group-specific. This is an evidence-shaping rule, not
proof that a domain-specific scientific or safety claim has been established.

The gate response includes a `gate_digest` over the normalized tool-evidence projection without the
digest field; non-tool review events do not change that digest, while retention changes remain
visible. `POST /v1/operations/gate-reviews` records a current acceptance as a content-addressed
retained event, and `GET /v1/operations/gate-reviews?review_id=...` replays it with its event
cursor. It is durable across restart only when `event_state_path` is configured; the response
exposes that boundary explicitly.
A handoff copies the required gate names and the `operations_gate_acceptance` field into each
execution prerequisite. `/v1/missions/preflight` returns `operations_evidence` for the mission’s
exact tool and domain groups. A real HTTP mission with `policy.execute: true` must supply a retained
`review_id`, visible `reviewer` and `rationale`, matching `gate_digest`, exact group IDs, and all
seven accepted gates per group; otherwise the API refuses it before queueing or dispatching any tool.
This is an operator attestation boundary, not scientific, clinical, regulatory, or deployment
approval.

An accepted executable mission retains a `bioprism-mission-execution-provenance/0.1` projection in
its status, inventory entry, and `/v1/missions/{mission_id}/provenance`. The projection correlates
the retained review ID and event ID, current gate digest and scope, exact acceptance document,
domain-evaluator rows, bounded preflight evidence, and the `mission.execution.accepted` event.
When `mission_state_path` is configured, the bounded provenance survives restart with the mission
checkpoint; otherwise it remains process-local. This is replay and accountability evidence, not a
readiness or scientific-validity claim.

### Claim-level evidence lineage

Mission requests may include a bounded `claim_requests` array. Each row is caller-authored and
contains an `id`, the statement text, one or more domain labels, and explicit `requires_steps`.
`level` (`observation`, `evaluation`, `operational`, or `release`) and `evidence_mode`
(`completed_step` or `successful_tool_result`) are routing/evidence posture labels, not semantic
interpretations. Every referenced step must exist in the same mission; at most 64 claims and 32
step references per claim are accepted. A claim may additionally declare up to 16 explicit
`evaluator_bindings`, each naming an `adapter_id`, domain, source step, and RFC 6901 output pointer.
The binding is a coverage declaration, not an assertion that the adapter is calibrated or valid.

The terminal mission report includes `claim_lineage`, and
`GET /v1/missions/{mission_id}/claims` returns the same projection in a small dedicated envelope.
Each evidence row preserves step status, requiredness, post-binding argument digest, byte count,
and a digest of the retained nested result when available. Missing results, refusals, cancellations,
and output omission remain distinct. `claimable: true` means only that the requested transport
evidence posture was retained; `claim_status` remains `unreviewed` and `readiness_claimed` remains
false. The projection never establishes scientific, clinical, causal, operational, regulatory, or
release truth, and a 409/410 response makes live-vs-omitted result state explicit.
Evaluator rows add `evaluator_coverage` with required/retained counts and a
`required_incomplete` posture when a declared evaluator output is missing, refused, omitted, or
does not contain the requested pointer. When multiple outputs are retained, canonical output
digests additionally expose `single_observation`, `unanimous_digest`, `disagreement`, or `partial`
posture. This is a transport-level disagreement witness, not a semantic adjudication. Domain
adapters remain heterogeneous while the mission layer keeps one auditable cross-domain envelope.

### Durable evaluator replay query

`GET /v1/missions/{mission_id}/evaluator-replay` accepts two bounded query parameters:
`include_fixtures=false|true` (default `false`) and `max_items=1..512` (default `128`). When the
terminal mission report is retained, the response has `retention.mode: "full"` and embeds the
same non-executing `mission_evaluator_replay` projection available through MCP. When the report
was omitted by the 256 KiB checkpoint bound, the response has `retention.mode: "summary_only"` and
embeds a compact `mission_evaluator_replay_summary` with mission/result/catalogue digests,
outcome counts, claim-level counts, coverage, findings, and replay posture. The summary is
content-addressed evidence of what was retained, not a substitute for raw output.

The route returns `409` while no terminal evaluator evidence is available and `410` when the
mission result was omitted without a recoverable evaluator summary. Both responses preserve an
explicit refusal code rather than returning an empty successful replay. `execution` remains
`"not_started"` for both modes: this route audits stored evidence and never dispatches domain or
evaluator tools. The Python `ApiClient`/`AsyncApiClient` and TypeScript `ApiClient` expose the same
bounded query, while typed reports preserve retention mode, links, guarantees, and limitations.

### Evaluator catalogue comparison and evidence bundle export

`GET /v1/missions/{mission_id}/evaluator-replay/compare` accepts the same bounded
`include_fixtures` and `max_items` query parameters as replay. Its `catalog_drift` object includes
the historical and current catalogue digests, digest validity/match, historical review and
discovery IDs, compatible and missing referenced adapter IDs, current catalogue counts, and the
explicit `comparison_scope: "historical_digest_and_current_binding_compatibility"`. A matching
digest does not prove evaluator semantics; a changed digest does not identify the exact changed
row unless a caller retained that catalogue snapshot. Summary-only checkpoints use their separate
`historical_catalog_digest` rather than treating a newly recomputed current digest as historical.

`GET /v1/missions/{mission_id}/evidence-bundle` accepts `include_result` (default `false`),
`include_trace` (default `true`), `include_fixtures` (default `false`), and `max_items` (`1..512`,
default `128`). The export is capped at 2 MiB, includes a deterministic `bundle_digest`, and
preserves `result_digest`/`result_omitted` metadata even when the result body is not included.
`include_result=true` is accepted only when the authoritative result was retained; an omitted
result remains explicitly omitted. The bundle's `execution` posture is always `not_started` for
the replay/comparison sub-workflows, and the trace is observational evidence rather than a second
execution result.

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
`GET /v1/delivery-receipts/{receipt_id}/attempts?after=N&limit=M` applies the same cursor to all
subscriptions and returns only exact receipt-correlated attempts, including `found`, receipt
identity, and the same explicit non-claim about external effects.

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
