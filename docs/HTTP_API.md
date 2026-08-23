# HTTP API and event delivery

`bioprism-api` is the network-facing integration layer over the existing Rust MCP server. It is
available as a library (`bioprism_api::ApiRouter`) and as the `bioprism-api` binary:

```bash
cargo run -p bioprism-api -- --root . --bind 127.0.0.1:8787 --token <at-least-16-visible-bytes> \
  --mission-state .local/mission-state.json --mission-queue-state .local/mission-queue.json \
  --event-state .local/event-state.json \
  --reconciliation-state .local/reconciliation-state.json \
  --artifact-state .local/artifact-state.json
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
| `GET /v1/capabilities/dashboard` | Bounded capability inventory with transport gaps plus advisory artifact and workflow-reconciliation evidence posture |
| `POST /v1/capabilities/route` | Raw non-executing cross-domain route proposal without an MCP response envelope |
| `POST /v1/capabilities/route/review` | Raw non-executing caller-selection review and mission handoff without an MCP response envelope |
| `POST /v1/capabilities/route/plan` | Raw non-executing composition of route review and authoritative mission preflight |
| `POST /v1/capabilities/route/plan/verify` | Raw non-executing structural/replay verification of a retained route plan |
| `GET /v1/recovery` | One operator-visible matrix of restart, secret, outbox, delivery-provenance, and external-effect boundaries |
| `GET /v1/operations/snapshot?after=N&limit=M` | One bounded operator control-plane snapshot joining event, mission, persistence, recovery, and capability summaries |
| `GET /v1/operations/domains?after=N&limit=M` | Per-domain catalogue coverage plus exact local tool activity observed in the requested event page |
| `GET /v1/operations/gates?after=N&limit=M` | Per-domain catalogue, activity, transport, pooled evaluation, domain-evaluator, safety, release, and advisory artifact evidence gates without readiness claims |
| `POST /v1/operations/handoff` | Build a content-addressed, non-executing domain-to-`capability_route` handoff |
| `GET /v1/domain-workflows` | Build one deterministic, digest-bound workflow template for every capability group |
| `POST /v1/domain-workflows/scaffold` | Select available stage tools, build an execution-disabled workflow, and run bounded preflight |
| `POST /v1/domain-workflows/instantiate` | Instantiate a group-scoped mission and attach authoritative no-dispatch preflight |
| `POST /v1/domain-workflows/portfolio` | Plan multiple explicit capability-group workflows with independent preflight and complete/partial coverage posture |
| `POST /v1/domain-workflows/portfolio/verify` | Verify a retained portfolio digest, coverage, aligned replay set, and per-item authoritative no-dispatch preflight |
| `POST /v1/domain-workflows/verify` | Verify a retained instantiation against current catalogue, contract, and mission-preflight state; optionally replay the original request |
| `POST /v1/domain-workflows/reconcile` | Reconcile retained mission results or evidence bundles against an instantiated workflow contract |
| `POST /v1/domain-reports` | Validate and index one explicit report projection for a declared capability group and source tool |
| `GET /v1/domain-reports/coverage?group_id=&domain=&report_class=&bridge_mode=&max_groups=&include_report_digests=` | Count retained structured report projections across the capability catalogue |
| `POST /v1/domain-evidence/harmonize` | Join exact same-subject domain reports into an explicit, digest-addressed traceability artifact |
| `GET /v1/domain-evidence/harmonization/coverage?subject_id=&domain=&report_class=&bridge_mode=&traceability_state=&after=&max_items=&include_report_digests=` | Query bounded retained harmonization summaries without returning full artifact bodies |
| `POST /v1/domain-evidence/intake` | Normalize and index one supplied raw request/response envelope from any declared capability-group tool |
| `POST /v1/domain-evidence/sources` | Build and index a non-fetching, digest-addressed external evidence source plan |
| `POST /v1/domain-evidence/sources/execute` | Execute a retained source plan through bounded file/plain-HTTP connectors and retain the response intake |
| `GET /v1/domain-evidence/coverage?group_id=&domain=&max_groups=&include_intake_digests=` | Count retained raw-intake envelopes plus advisory digest-verified artifact-family evidence by authoritative group |
| `GET /v1/domain-decision-readiness?subject_id=&decision_state=&policy_satisfied=&after=&limit=&include_audits=` | Query retained structural decision-readiness audits by exact state/policy posture |
| `POST /v1/control-plane-readiness` | Join explicitly supplied domain, route, operations, release, and workflow evidence into one retained structural posture |
| `GET /v1/control-plane-readiness?subject_id=&control_plane_state=&policy_satisfied=&after=&limit=&include_audits=` | Query retained control-plane projections by exact state/policy posture |
| `POST /v1/domain-workflows/reconciliations` | Verify and idempotently import one reconciliation report into the bounded audit registry |
| `GET /v1/domain-workflows/reconciliations?mission_id=&workflow_id=&mission_plan_digest=&completion_status=&decision_readiness_state=&decision_readiness_gate_satisfied=&after=&limit=&include_records=` | Query digest-ordered reconciliation index rows, including optional readiness-gate filters |
| `GET /v1/domain-workflows/reconciliations/{reconciliation_digest}` | Fetch one digest-verified reconciliation report |
| `GET /v1/domain-workflows/reconciliations/persistence` | Inspect reconciliation checkpoint integrity, generation, and retention bounds |
| `POST /v1/domain-workflows/reconciliations/persistence/flush` | Force an atomic reconciliation registry checkpoint |
| `GET /v1/tools` | The exact MCP tool definitions |
| `POST /v1/tools/{name}` | Call any tool with a JSON object body; delegates to the MCP dispatcher |
| `POST /v1/tools/brain_job_submit`, `POST /v1/tools/brain_job_status`, `POST /v1/tools/brain_job_events`, `POST /v1/tools/brain_job_approval`, `POST /v1/tools/brain_job_claim`, `POST /v1/tools/brain_job_renew`, `POST /v1/tools/brain_job_checkpoint`, `POST /v1/tools/brain_job_complete`, `POST /v1/tools/brain_job_fail`, `POST /v1/tools/brain_job_reconcile` | Value-only autonomous-brain admission, observation, approval, lease, checkpoint, settlement, retry, and external-effect reconciliation operations |
| `POST /v1/tools/brain_model_health`, `POST /v1/tools/brain_replay_evaluate` | Value-only provider/model health observations and digest-bound offline domain replay |
| `POST /v1/missions` | Validate and submit an asynchronous `agent_mission` job |
| `GET /v1/missions/persistence` | Inspect bounded checkpoint configuration and on-disk size |
| `POST /v1/missions/persistence/flush` | Force a checkpoint and verify it can be written |
| `GET /v1/missions/queue` | Inspect typed factory queue state, lease/recovery posture, and bounded job projections |
| `GET /v1/missions/queue/persistence` | Inspect mission queue checkpoint integrity and startup recovery rows |
| `POST /v1/missions/queue/persistence/flush` | Force an atomic content-addressed factory checkpoint |
| `POST /v1/missions/queue/authority/release-lock` | Attribute and audit an explicit release of an orphaned shared-authority lock |
| `GET /v1/missions/{mission_id}` | Poll job state and retrieve the authoritative mission report |
| `GET /v1/missions/{mission_id}/provenance` | Retrieve retained gate, review, domain-evaluator, and accepted-dispatch evidence |
| `GET /v1/missions/{mission_id}/claims` | Retrieve the bounded claim-to-step evidence-lineage projection |
| `GET /v1/missions/{mission_id}/evaluator-replay` | Retrieve durable full or summary-only evaluator replay evidence |
| `GET /v1/missions/{mission_id}/evaluator-replay/compare` | Compare retained evaluator provenance with the current catalogue |
| `GET /v1/missions/{mission_id}/evidence-bundle` | Export a bounded, content-addressed mission evidence bundle |
| `POST /v1/evidence-bundles/verify` | Verify a portable evidence bundle without executing a mission or evaluator |
| `POST /v1/evidence-bundles` | Independently verify and idempotently import a bundle into the bounded registry |
| `GET /v1/evidence-bundles?mission_id=&domain=&after=&limit=&include_bundles=` | Query digest-ordered mission/domain evidence index rows |
| `GET /v1/evidence-bundles/{bundle_digest}` | Fetch one verified bundle by content hash |
| `GET /v1/evidence-bundles/persistence` | Inspect restart-safe evidence registry checkpoint integrity and bounds |
| `POST /v1/evidence-bundles/persistence/flush` | Force an atomic evidence registry checkpoint |
| `POST /v1/artifacts` | Register one bounded exact-content cross-domain artifact |
| `GET /v1/artifacts?kind=&domain=&subject_id=&after=&limit=&include_artifacts=` | Query digest-ordered artifact index rows |
| `GET /v1/artifacts/cross-store` | Compare exact identities across artifact, evidence, and workflow-reconciliation registries |
| `GET /v1/artifacts/{content_digest}` | Fetch one artifact record and its verification posture |
| `GET /v1/artifacts/{content_digest}/lineage` | Traverse bounded parent lineage with explicit missing parents and cycles |
| `GET /v1/artifacts/persistence` | Inspect restart-safe artifact registry checkpoint integrity and bounds |
| `POST /v1/artifacts/persistence/flush` | Force an atomic artifact registry checkpoint |
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

## Domain workflow catalogue and instantiation

`GET /v1/domain-workflows` is the transport-neutral bridge between capability discovery and
`agent_mission`. It returns exactly one workflow template per explicit capability group, currently
29 groups. Each row carries the group domains, owning crates, CLI entrypoints, declared MCP tools,
the intersection with authoritative `tools/list`, missing definitions, per-tool schema/evidence
contracts, advisory lexical stages, and a `workflow_digest`; the catalogue also carries input and
workflow-catalogue digests. Each row's `domain_contract` makes scope review, tool availability,
argument-schema preflight, execution policy, per-step evidence retention, refusal/omission handling,
and completion review explicit. Missing tool definitions remain visible and never become executable
by implication.

`POST /v1/domain-workflows/scaffold` is the low-friction planning entrypoint for callers that have
not yet authored a complete step list. It accepts `workflow_id`, `mission_id`, and `goal`, plus
optional `tools` and an `arguments` object keyed by tool name. With no explicit tools, the kernel
selects at most one authoritative available tool for each advisory stage; explicit selection remains
bounded to the selected workflow and recursive `agent_mission` cannot be selected as a scaffold step.
The response preserves the generated `instantiation`, selected and omitted tools, bounded
`argument_contract` facts from the live tool catalogue, and the authoritative MCP preflight report.
Missing required arguments produce `preflight_status: "blocked"` with structured diagnostics rather
than a false-ready response. Scaffolding is deterministic and execution-disabled: `execution` and
`dispatch` remain `not_started`, `readiness_claimed` is always `false`, and the route never calls a
domain tool or grants permission. Callers must review sufficiency, complete arguments against the
authoritative schema, and use explicit mission preflight before any separate execution path.

`POST /v1/domain-workflows/instantiate` accepts `workflow_id`, `mission_id`, `goal`, and an explicit
`steps` array, with optional mission policy, claim requests, and evaluator review. Every selected
tool must be declared by the selected group and present in authoritative `tools/list`; policy
allow-list entries cannot escape the selected group. The kernel normalizes defaults, validates the
mission DAG, and when `policy.execute` is true derives `allowed_tools` from the selected steps only
if the caller did not provide one. The response includes the instantiated mission, selection
ledger, workflow and catalogue digests, the selected `domain_contract`, an `evidence_plan` for
every step, and the authoritative MCP `preflight_report`. `execution` and `dispatch` remain
`not_started`; this route never invokes a domain tool. Domain-specific arguments still require the
authoritative schema report, and a valid plan is not a readiness, truth, clinical, or release claim.
The returned `mission.workflow_binding` is a bounded digest-bound handoff containing the selected
workflow/catalogue/contract identities, the full domain contract, and the exact evidence plan with
its own digest. It is structural provenance carried into `agent_mission`; it is never a permission
or readiness credential.

`POST /v1/domain-workflows/portfolio` composes a bounded array of explicit instantiation requests
for callers preparing several capability groups at once. Each request is scoped and normalized
independently, then receives authoritative no-dispatch mission preflight. A request that fails
scope, availability, mission, or schema checks remains in `items` with its own digest and issue
codes; it cannot erase successful sibling plans. `require_complete_catalogue=true` requires one
successful structural request for every live workflow group, while `allow_partial=true` makes
blocked rows a deliberate `partial` portfolio rather than a transport refusal. `valid` and
`portfolio_ready` are false whenever any item is blocked or required coverage is missing. The
portfolio digest covers the full bounded projection, and both `dispatch` and `execution` remain
`not_started`; the route never executes, retries, resumes, or grants readiness.

`POST /v1/domain-workflows/portfolio/verify` accepts a retained `domain_workflow_portfolio`
report, with optional `replay_requests` aligned exactly to `items` and an optional policy containing
`allow_partial`, `require_complete_catalogue`, and `require_replay`. It recomputes and compares the
retained `portfolio_digest`, verifies every instantiated row independently, compares aligned replay
requests to retained `request_digest` values, and retains blocked rows and mismatch witnesses. The
MCP transport then reruns authoritative mission preflight for every structurally successful item;
REST response `request_id` metadata is transport-only and can be round-tripped into this route.
`portfolio_verify_digest`, item statuses, coverage, replay counts, and preflight counts are all
explicit. The route remains an audit boundary with `dispatch` and `execution` set to
`not_started`; a valid result is not execution permission or scientific, clinical, provider, or
release validity.

`POST /v1/developer-workbench/verify` verifies a retained `developer_workbench` report against the
current `StudioSession`. It recomputes the session audit, deterministically replays the retained
dashboard query, and optionally replays the original `CiRequest`; `require_dashboard`, `require_ci`,
and `require_ci_replay` make omitted projections explicit policy failures. The result preserves
retained/observed digests, mismatch codes and JSON paths, replay booleans, and a verification digest.
Both `execution` and `network_access` are always `not_started`; the route does not run notebook cells,
write generated YAML, contact GitHub, execute CI, or grant release or domain authority.

The retained workbench registry provides a bounded lookup layer for those reports:

- `POST /v1/developer-workbench/reports` imports `{ "report": <developer_workbench> }` after
  canonical digest and schema validation. Re-importing the same report is idempotent.
- `GET /v1/developer-workbench/reports` returns digest-ordered compact rows. Optional query
  parameters are `session_digest`, `domain`, `capability`, `state`, `release_ready`, `after`,
  `limit` (1–256), and `include_reports` (false by default).
- `GET /v1/developer-workbench/reports/{workbench_report_digest}` fetches one exact report by its
  lowercase SHA-256 content digest.
- `GET /v1/developer-workbench/reports/persistence` reports configured checkpoint state and
  `POST /v1/developer-workbench/reports/persistence/flush` writes the current snapshot explicitly.

The API shares this registry with MCP, so an import through one transport is immediately queryable
through the other. `--workbench-state <file>` enables atomic restart-safe persistence; startup
verifies the snapshot digest and every retained report. The registry is capped at 512 reports and
32 MiB, and all registry operations remain non-executing audit lookups.

The provider-observed CI evidence registry provides the corresponding durable join for an audited
run and its external artifact/log/attestation indexes:

- `POST /v1/ci/provider-evidence` accepts a typed `CiProviderEvidenceRequest`, re-runs the
  canonical provider-evidence audit, and imports the complete digest-addressed record idempotently.
  Failed and unknown provider runs remain retainable evidence with `conformance_ready: false`;
  malformed, unbound, or digest-inconsistent rows are rejected.
- `GET /v1/ci/provider-evidence` returns compact deterministic rows ordered by
  `provider_evidence_digest`. Filters include `provider`, `run_id`, `plan_digest`,
  `structurally_valid`, `conformance_ready`, `min_local_byte_hash_artifacts`,
  `min_local_byte_hash_logs`, `min_attestation_subject_digest_bindings` (each 0–128),
  `after`, `max_items` (1–256), and `include_records` (false by default). Rows retain the
  three binding counts for compact provenance posture searches.
- `GET /v1/ci/provider-evidence/{provider_evidence_digest}` returns one exact retained audit,
  including separate artifact/log/attestation counts and record-family digests.
- `GET /v1/ci/provider-evidence/persistence` reports checkpoint integrity and
  `POST /v1/ci/provider-evidence/persistence/flush` forces an atomic snapshot when persistence
  is configured.

The API shares this registry with MCP. `--ci-provider-evidence-state <file>` enables restart-safe
startup recovery; the bounded registry accepts at most 512 records and a 32 MiB snapshot. The
lineage fields are provider-observed joins only: the gateway does not fetch remote bytes, contact
GitHub/GitLab, verify signatures, execute CI, or establish deployment or release authority.

`POST /v1/domain-workflows/verify` checks a retained `domain_workflow_instantiate` response before
handoff or re-review. It validates the workflow, catalogue, domain-contract, mission, and binding
identities; reruns authoritative mission preflight; and, when `replay_request` is supplied, rebuilds
the instantiation from the original bounded request and compares the resulting contract, evidence,
selection, mission, and execution projections. A shape-only request is reported as
`verified_without_replay`; a replay or preflight failure is explicit and blocks verification.
The response preserves mismatch codes and compact digest witnesses, always returns
`dispatch: "not_started"` and `execution: "not_started"`, and never executes, retries, or grants
permission to a domain tool.

`POST /v1/domain-workflows/reconcile` closes the execution handoff without becoming an executor.
It accepts the exact accepted instantiation result plus either an `agent_mission` report or a
portable mission evidence bundle. The response separately reports contract/integrity validity,
per-step evidence rows, raw-output retention, trace lifecycle, refusal/block/cancellation state,
summary-only omissions, and `completion.status`. `completion.ready` is true only when every
required step succeeded with retained raw output and the correlated plan/evidence artifact is
structurally valid. A verified summary-only bundle is intentionally `unverified`, not complete;
all reports remain review-required and non-claiming.

The reconciliation registry is a bounded, content-addressed audit index layered on that projection.
`POST /v1/domain-workflows/reconciliations` accepts `{ "record": <domain_workflow_reconcile> }`,
recomputes the report digest, and returns an idempotent import report. The query route returns compact
rows ordered by `reconciliation_digest`; filters are structural (`mission_id`, `workflow_id`,
`mission_plan_digest`, and `completion_status`) and `include_records=true` is opt-in because full
reports consume the 32 MiB checkpoint bound. `GET .../{reconciliation_digest}` performs an exact
content-hash lookup. Configure `--reconciliation-state <file>` for atomic restart-safe persistence;
startup rejects a tampered snapshot or report, and `/persistence/flush` is an explicit checkpoint
operation. Neither import, query, lookup, nor restore dispatches, retries, resumes, or re-evaluates a
mission, and registry presence is not provenance, scientific validity, clinical safety, or release
approval.
Trusted boundaries automatically add `artifact_registry` projections to mission reports,
verified evidence-bundle imports, evaluator replays, and workflow reconciliations. The projection
contains the exact cross-domain content digest, verification posture, and checkpoint status where
applicable. Direct domain-tool results are not automatically registered, and a projection never
claims causal provenance, scientific validity, authorization, or release readiness.
`GET /v1/artifacts/cross-store` is a bounded consistency diagnostic. It compares retained exact
digests, reports missing source projections, orphaned projections, wrong-kind projections, store
generations, and each store's digest-protected checkpoint identity. The three registries are
sampled independently; `consistent=true` means agreement among those bounded observations, not a
transaction or proof that an omitted record never existed.

`GET /v1/domain-evidence/lineage` is the intake-specific read model over the same artifact
registry. It is bounded to retained `domain_evidence_intake` artifacts and supports an exact
`content_digest` lookup or cursor-paginated filters for `group_id`, `domain`, `subject_id`,
`source_tool`, `outcome`, `request_digest`, `response_digest`, `intake_digest`, and
`source_plan_digest`. Each row exposes the exact request/response/intake identities, direct
declared parent rows with `present` versus missing state, and optional reverse direct-child rows
found by exact parent content digest. A source-plan section reports the canonical plan digest,
any retained plan's separate content digest, and whether that content record is actually declared
as a parent. The endpoint never includes full payload bodies; use the returned artifact lookup or
`GET /v1/artifacts/{content_digest}` for the independently bounded canonical intake. This is a
structural local index projection, not proof of execution, causal provenance, source authenticity,
scientific or clinical validity, release readiness, or external-effect completion.

`POST /v1/domain-reports` is the explicit projection boundary for the workspace's 29 capability
groups. Its body requires `group_id`, one or more catalogue-declared `domains`, `subject_id`, a
catalogue-declared `source_tool`, an object `report`, and a `claim_posture` with one of
`observed`, `derived`, `review_required`, `refused`, or `not_applicable` plus a non-empty
`does_not_claim` list. The report is structurally validated, indexed under its exact JSON digest,
and returned with the artifact lookup and catalogue digest. It does not execute the source tool.
The MCP tool `domain_report_project` also accepts `operation: "from_adapter_execution"` with a
nested `adapter_execution_evidence` request. That operation validates and indexes the adapter
evidence first, composes a catalogue-checked `adapter_execution` domain report, and returns both
artifacts with the evidence digest linked as a report parent. It preserves observed, partial,
refused, and review-required execution posture and always returns `readiness_claimed: false` and
`execution: "not_started"`; it is a composition boundary, not adapter execution or a scientific,
clinical, provenance, regulatory, or release-readiness claim.
The same tool accepts operation "from_provider_normalization" and
operation "from_external_provider_normalization" with a nested provider-normalization
request. The inline route retains structural provider shape/index observations; the external
route additionally retains receipt and digest-verified materialization metadata while never
opening the locator. Both routes return one provider-domain bridge envelope containing the
normalization result and canonical domain report, with compact report evidence rather than a
second payload copy, explicit artifact parents, and readiness_claimed: false.
`GET /v1/domain-reports/coverage` reports which groups have retained structured projections,
subject/source/status summaries, missing group ids, and an exact coverage digest. Optional
`report_class` and `bridge_mode` filters narrow the retained rows before group/domain coverage is
computed, allowing adapter/provider composition audits without mixing ordinary projections.
Coverage means
local indexed projection presence only; it is not execution coverage, scientific validity,
provenance completeness, reproducibility, release readiness, or external-effect completion.
Each coverage group also reports classified projection counts for ordinary, adapter-execution,
inline-provider, and external-provider report bridges when present, plus bridge modes and lineage
parent counts. The top-level bridge summary makes those classes and linked/unlinked parent counts
auditable without treating any of them as execution or validity.
`POST /v1/domain-evidence/harmonize` accepts canonical domain-report bodies or projection wrappers,
requires exact subject identity, validates each report's source tool and domain labels against the
authoritative catalogue, and requires every report to have an explicit `supports`, `qualifies`,
`contradicts`, or `context` link. Qualification and contradiction links require caller notes;
missing required groups/domains and link coverage remain visible in the result. The operation
indexes a digest-addressed harmonization artifact with report digests as parents, always keeps
`readiness_claimed: false`, and does not choose between conflicting reports or claim scientific,
clinical, causal, provenance, publication, release, or execution validity.
The returned `harmonization.coverage.bridge_summary` reports per-report bridge class, bridge mode,
and lineage-parent count, plus aggregate class/mode counts and linked/unlinked lineage totals.
Missing bridge markers remain `ordinary`; this is structural composition telemetry and does not
infer provider authenticity, adapter correctness, payload availability, or readiness.
`GET /v1/domain-evidence/harmonization/coverage` is the retained read model for that artifact class.
It orders compact rows by content digest, supports subject/domain/bridge/traceability filters and an
exclusive `after` cursor, and optionally includes the exact report-digest list. The top-level summary
aggregates matching rows before pagination, while `matching_count`, `returned_count`, `has_more`, and
`next_after` make truncation explicit. A coverage row is an index observation only: it does not fetch
or reinterpret the harmonization body, and complete traceability does not establish truth, validity,
provenance completeness, execution, clinical safety, or release readiness.
`POST /v1/tools/domain_decision_readiness_audit` applies the same explicit structural policy to a
caller-selected set of canonical reports. Its policy can require exact groups/domains, minimum
supporting or qualifying reports, complete links, lineage parents, and fail-closed handling for
contradictions, refusals, or review-required reports. The response keeps every blocker and emits
`blocked`, `incomplete`, `review_required`, or `ready_for_human_review`; the last state means only
that the supplied structure passed the caller's policy. It never claims scientific, clinical,
causal, release, execution, or truth readiness, and remains `readiness_claimed: false` with
`execution: "not_started"`.
`GET /v1/domain-decision-readiness` queries the same retained audits without re-running
harmonization. Rows expose the audit/content digests, subject, structural state, policy result,
counts, and parent digests; full audits are opt-in. Workflow portfolio and reconciliation requests
may supply a validated audit plus `policy.require_readiness`; the resulting gate is reported beside,
not merged into, execution preflight and completion evidence.
`POST /v1/control-plane-readiness` is the next composition boundary. It accepts the exact returned
domain readiness wrapper plus optional capability-route review/plan, operations preflight and gate
review, release audit, and workflow evidence packets. Policy flags explicitly decide which packets
are required. The response reports each component's `present`, `valid`, `satisfied`, state, digest,
and authority separately, then emits `ready_for_human_review`, `incomplete`, `blocked`, or
`review_required` for the structural join. The join never calls a nested tool, authorizes a mission,
promotes operator acceptance into deployment approval, or treats a release audit as proof of an
external build. The exact projection is indexed as `control_plane_readiness`; the GET route is
digest-ordered, cursor-bounded, and restart-safe when artifact persistence is configured.
POST /v1/control-plane-readiness/compare accepts two successful, digest-verified projection
wrappers and returns a deterministic structural diff: state direction, component transitions,
policy changes, blocker additions/removals, domain and parent-digest deltas, and a next review
action. It is a replay/inspection boundary only; it does not rerun nested evidence or grant
execution, scientific, clinical, deployment, or release authority.
POST /v1/control-plane-readiness/compare-retained accepts exact before/after content digests,
resolves both records from the verified artifact registry, enforces the retained kind and shared
subject, and returns the same structural diff without requiring callers to reconstruct wrappers.
Retention is not freshness, external completeness, or authority.
`POST /v1/domain-evidence/intake` is the raw-envelope boundary for all 29 capability groups. It
requires a declared group, source tool, domain label, response JSON, explicit outcome, and claim
posture; an original request is optional and its absence is distinguished from a supplied JSON
null. Request and response bytes receive separate canonical SHA-256 digests, the normalized
envelope is embedded in a canonical domain report, and the intake artifact is indexed with any
declared artifact parents. Outcomes remain `observed`, `partial`, `refused`, `error`, or
`unknown`; the route never invokes the source tool or promotes a response into truth, execution,
provenance completeness, scientific/clinical validity, release readiness, or external effects.
An optional `source_plan_digest` binds intake to a retained source plan; the MCP boundary checks
exact group, subject, source-tool, and domain compatibility before indexing and adds that plan
identity to the intake's parent set. Unbound legacy intake remains valid and restorable.
`GET /v1/domain-evidence/coverage` audits that retained intake boundary against the authoritative
catalogue. It preserves missing groups, intake outcomes, source tools, subjects, reported domains,
declared tools, missing source tools/domains, optional exact intake digests, and per-domain counts.
The existing `complete` flag means only that an intake artifact was retained for each selected
group; `tool_coverage_complete` and `domain_coverage_complete` separately report whether every
declared source tool and domain has an intake row. None of these flags means a tool ran or that a
response is true, complete, safe, reproducible, or release-ready.
The same coverage response now includes an advisory `artifact_evidence` posture for every selected
group, joined from the current digest-verified artifact registry. It covers adapter execution,
provider, source/harmonization, domain-report, workflow/mission, and external-reference families;
reports verification states, parent linkage, exact match basis, registry generation/size, and
matching counts. This adjunct does not change the intake-only meanings of `complete`,
`tool_coverage_complete`, or `domain_coverage_complete`, and artifact presence never implies that
an adapter, provider, source, report, or workflow executed or became scientifically, clinically,
or operationally valid. Python exposes parsed artifact postures when present and keeps legacy
responses explicit; TypeScript exposes the same group and summary fields.

`GET /v1/capabilities/dashboard` now joins two additional, independently inspectable views to every
returned capability group: `artifact_evidence` from the current digest-verified artifact registry
and `workflow_reconciliation_evidence` from the bounded digest-valid reconciliation registry. The
`audit.evidence` summary reports registry generations/sizes, selected-group counts, record counts,
and a separate `evidence_digest` that binds the selected group postures and registry metadata.
`dashboard_digest` continues to identify the catalogue/schema dashboard itself; it is intentionally
not silently redefined as an execution or readiness digest. The dashboard's `capability_dashboard_ready`
flag remains transport-schema coverage only, and neither evidence posture implies that a tool ran,
a workflow reconciled successfully, or a scientific, clinical, regulatory, release, or external
effect claim is valid. Older clients may ignore these additive fields.
`POST /v1/domain-evidence/sources` builds the external-source planning boundary. It binds a
connector family, locator class, retrieval mode, bounded network/cache policy, optional expected
content digest, and domain scope into an exact plan artifact. Credentials remain caller-managed;
the route does not fetch a URI/path, follow redirects, resolve a provider, or claim that a later
retrieval will be authentic, complete, or scientifically valid. If an expected content digest is
declared, a later `source_plan_digest`-bound intake is refused unless its canonical response digest
matches that expectation.
`POST /v1/domain-evidence/sources/execute` is the controlled execution seam. It requires a retained
plan, confines file reads to the configured server root, and permits plain HTTP only when the plan
has `network: "enabled"` plus an exact `allowed_hosts` list. Reads enforce `max_bytes` and
`timeout_ms`; redirects, HTTPS, credentials, unsupported connector families, traversal, and
network-policy refusals remain explicit `refused` or `error` outcomes; the in-process kernel only
executes `retrieval_mode: "content"`, leaving reference-only and metadata-only plans caller-managed.
Successful reads expose an
exact raw-byte digest and a separate canonical JSON response digest, then automatically retain the
bounded response through `domain_evidence_intake` with both the plan identity and plan artifact
content digest as parents. A read is still not source authenticity, scientific validity, or
provenance completeness. The REST/MCP boundary intentionally returns the bounded transport
envelope rather than invoking format parsers. The dependency-free Python SDK can pass that exact
response to `domain_evidence_source_project()` for an explicit inline adapter route; this local
handoff never fetches the locator again, verifies an optional raw-byte digest, refuses
omitted/binary/preview/truncated bodies, and preserves source transport digests separately from
parser provenance. A partial source outcome remains partial even when the nested parser accepts
the available body.
The Python SDK also offers a catalogue-bound local handoff that requires the exact acquisition
catalogue digest, source-plan digest, group, domain, and declared adapter route before invoking
the parser. This prevents a valid adapter from being reused for a different declared domain or
from being selected from a truncated catalogue slice; it remains a routing/conformance check,
not ontology resolution or scientific authorization.
`POST /v1/tools/domain_acquisition_catalogue` exposes the cross-domain route registry. Its
digest-bound report returns one row for every selected declared domain, with transport and
interpretation kept separate: bounded file/plain-HTTP transport, caller-managed connector
families, native adapter matches, Python-delegated adapter matches, domain-tool-only rows, and
unmapped rows are distinct. Adapter matches are based only on declared scope-label overlap and
carry the adapter registry digest; they do not resolve ontologies, execute adapters, verify
dependencies, or claim scientific/clinical validity. The eight evidence transport/intake/provider
tools are explicitly cross-cutting memberships for all 29 groups, so existing source-plan,
provider, and intake scope checks can be used for every declared domain rather than only
infrastructure domains.
`POST /v1/tools/domain_evidence_provider_normalize` is the caller-managed counterpart: it accepts
an opaque provider-shaped payload for literature, clinical-trial, FHIR, object-store, or generic
provider-API connectors, derives separate payload/request identities, and indexes the normalized
envelope through `domain_evidence_intake`. The response also returns a structural `shape_audit`:
connector-specific container recognition, record and malformed-row counts, identifier
field-presence coverage, object-store content-digest coverage, warnings, and a shape-only digest.
`shape_audit` never includes provider identifiers or payload values and does not interpret them as
scientific or clinical facts. It does not contact, authenticate, or interpret the provider response,
and defaults an omitted outcome to `unknown`. The response additionally includes a bounded
digest-only `record_index` with row-count, omitted-row, and index-digest fields for safe
cross-provider deduplication; it does not expose row contents.
`POST /v1/tools/domain_evidence_provider_replay_verify` recomputes the same caller-managed
normalization and compares required payload, request, shape, normalization, and intake digests.
It returns `matched`/`mismatch` dimensions and registers only the value-free replay verification
artifact, so repeated verification is idempotent and a match remains an identity check rather
than proof of provider authenticity or domain validity.
`POST /v1/tools/domain_evidence_provider_connector_handoff` is the production plugin seam before
that intake: it validates a caller-managed connector manifest, requested domain scope, capability
set, authentication posture, opaque secret references, optional request/payload/source-plan
digests, and parent edges. The resulting handoff is content-addressed and idempotently retained
as `domain_evidence_provider_handoff`; it always reports `execution: not_started` and
`readiness_claimed: false`. The core never launches a plugin, resolves credentials, authenticates
or contacts a provider, and rejects credential material rather than serializing it into the
artifact.
`POST /v1/tools/domain_evidence_provider_external_payload_receipt` is the out-of-line transfer
seam for payloads too large or sensitive to send through MCP. It stores only an exact payload
digest, byte length, transfer identity, storage backend, locator class/reference, media metadata,
availability, retention, and the connector-handoff parent. The receipt is durably indexed and
snapshot-verified, but the core never opens the locator, fetches, decrypts, or inspects the bytes;
`available` and `durable` remain caller assertions until an external executor proves otherwise.
`POST /v1/tools/domain_evidence_provider_external_payload_normalize` is the explicit materialized
path: callers supply a bounded canonical JSON value, the core recomputes its digest and byte
length against the external receipt, then passes the verified value through provider shape audit
and catalogue-bound intake. A digest or byte-length mismatch is refused; the locator is never
opened and readiness remains false.
`POST /v1/tools/domain_evidence_provider_external_payload_replay_verify` re-checks the retained
receipt's handoff digest, payload digest, byte length, and canonical receipt digest against
caller-supplied expectations. It is metadata-only, never opens the locator, and records both a
matching and a mismatch as a value-free, idempotent artifact; a match still does not prove that
the external object resolves or that its bytes remain authentic.
`POST /v1/tools/domain_evidence_provider_external_payload_lineage_audit` joins the receipt to the
retained connector handoff in the local artifact registry. It reports `matched`, `partial`,
`mismatch`, or `orphaned` lineage, exposes each scope comparison plus optional payload-digest
binding, and idempotently records the audit. It performs no provider, store, locator, credential,
or payload operation; even matched lineage remains `execution: not_started` with readiness false.
`POST /v1/tools/domain_evidence_provider_external_payload_execution_evidence` retains a
caller-supplied transfer observation and compares its expected receipt, observed payload digest,
and observed byte length with the retained receipt. `matched`, `partial`, `mismatch`, and
`orphaned` remain distinct; `execution_status`, `executor_id`, and `locator_opened` are explicit
caller assertions, not cryptographic attestations. The core performs no transfer or external I/O,
and every response remains not-started and not-ready.
`POST /v1/tools/domain_evidence_provider_external_payload_evidence_query` is the bounded read-only
projection over those three retained artifact families. It joins rows by receipt digest, supports
group/domain/subject filters, deterministic digest cursors, and optional artifact bodies, and keeps
`missing_receipt`, `receipt_only`, partial-join, and `complete` states distinct. It reads one local
registry snapshot only; it never opens locators, fetches providers, resolves credentials, or turns a
complete structural join into execution or readiness.
`POST /v1/tools/adapter_execution_evidence` is the common adapter handoff for native and
Python-delegated routes. It requires a declared adapter id/version, group/domain scope, source and
input identity, and explicit execution, conformance, and semantic-loss states; bounded loss rows,
output/item/byte evidence, refusal codes, parent digests, and an idempotent evidence artifact remain
visible. The gateway checks adapter and catalogue membership but does not execute adapters, import
optional packages, fetch sources, or promote caller assertions into readiness.
`POST /v1/tools/adapter_execution_evidence_query` is the read-only companion. It cursor-pages
retained adapter rows, filters by adapter/source/status identity, and classifies only explicit
parent-digest joins to source plans, intake/external-payload projections, and workflow
reconciliations. Missing and unclassified parents remain visible; the query does not infer
provenance from labels, execute adapters, fetch sources, or change workflow state.
Each page also carries separate counts for execution, conformance, semantic-loss, join status,
missing parents, output digests, and loss entries; these are page summaries, not a composite score
or readiness claim.
An executable mission produced by instantiation is automatically reconciled at the authoritative
MCP dispatch boundary. The response includes a compact `workflow_reconciliation` object with the
record digest, completion/evidence/integrity posture, registry import result, and a REST lookup
link; the full report is indexed in the same registry used by REST gates. Automatic reconciliation
is fail-closed: a missing binding, failed reconciliation, incomplete evidence, or invalid integrity
never becomes readiness. Synchronous REST/JSON-RPC calls checkpoint this shared registry before
returning when `--reconciliation-state` is enabled, and asynchronous `/v1/missions` workers
checkpoint it before storing terminal job state.

`GET /v1/operations/snapshot` composes this registry into the control-plane read model. Its
`reconciliation_summary` reports digest-valid registry size and generation, completion-status
counts, structural-ready count, explicit review-required count, integrity-invalid count, and
evidence-invalid count, plus a per-workflow completion-status matrix; `persistence.workflow_reconciliations` reports checkpoint state. The
summary is read-only and non-claiming, and the snapshot still declares that cross-store composition
is not atomic.

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

## Mission execution queue

Pass `--mission-queue-state <file>` to persist the factory lifecycle behind asynchronous mission
execution separately from the mission read model. Each accepted mission is represented as an
`Evaluate` job with an explicit idempotency class, leased to the local API worker, and committed
only after the executor report is staged. Queue checkpoint writes are bounded, digest-verified,
cross-index validated, and atomically replaced. A failed checkpoint does not replace the live
queue projection. A hash-chained transition journal is atomically replaced with the queue image,
and cooperating API processes that point at the same local file serialize mutation through a
bounded authority lock. Status exposes the authority digest, revision, event count, lock state,
and observation-time integrity result. Configure `--mission-queue-max-jobs <n>` and
`--mission-queue-max-active-leases <n>` to apply explicit backpressure before queue mutation;
the status route reports those limits and observed active leases. Factory lease attempts are
fencing tokens, so a stale worker attempt cannot mutate a later attempt with the same worker ID.

On startup, expired leases are classified by the factory policy: idempotent jobs are requeued,
non-idempotent jobs are quarantined, and compensable jobs await compensation. This is recovery
classification, not automatic resumption. The API never dispatches a recovered job without a new
explicit submission. If a process dies while holding the authority lock, an operator can call
`POST /v1/missions/queue/authority/release-lock` with a non-empty identity and reason; the release
is refused unless the lock owner is stale and the resulting `LockReleased` transition is recorded.
This local shared-file authority does not claim multi-host consensus, network-partition tolerance,
provider authentication, tenant isolation, or external-effect completion. The queue inventory intentionally
returns job metadata and an idempotency digest while omitting the original mission specification.

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

`POST /v1/capabilities/route` includes a bounded evidence observation alongside its ranked
candidate route: each candidate capability group carries digest-verified artifact posture and
workflow-reconciliation posture, while the route exposes `evidence_digest`, registry generations,
record counts, and per-need `candidate_group_evidence`. This is a point-in-time advisory join, not
an atomic cross-store snapshot, execution proof, authorization, scientific validity, release
readiness, or external-effect claim. The digest is separate from `route_id` because retained
evidence may change while the catalogue-bound route request remains identical.

`POST /v1/capabilities/route/review` validates that modern route evidence is internally consistent
and carries its digest and scope into `evidence_binding`, the review identity, and any generated
`mission_draft`. The binding posture is `carried_forward_not_recomputed`: review preserves the
route’s retained observation but does not turn it into execution, authorization, readiness, or
scientific validity. Legacy routes without the optional evidence fields remain reviewable with an
explicit `present: false` binding.

`POST /v1/capabilities/route/plan` is the bounded composition endpoint for callers that want one
auditable handoff after making their own candidate choices. It accepts a `mission_id`, the complete
route, explicit `selections`, and optional mission policy/claim/evaluator/workflow bindings. It
reruns route review and then invokes the authoritative mission preflight with `execute` forcibly
false. A successful response contains the reviewed route, the exact generated mission request,
and its preflight; a transport-unavailable or malformed selected tool is returned as an explicit
`blocked_by_mission_preflight` projection instead of being mistaken for a dispatch. The endpoint
always returns `dispatch: "not_started"`, never chooses a candidate, and never executes a domain
tool. This applies uniformly to every current capability group because it consumes the live
catalogue rather than a domain-specific allow-list.

`POST /v1/capabilities/route/plan/verify` verifies a previously returned plan without dispatching it.
The required `plan` is checked for identity and shape, and its retained mission is sent through the
authoritative preflight again. Callers may also provide the original `route` and `selections` to
replay route review and compare the route, review, catalogue, plan, and selection digests. Omitting
those inputs is intentionally reported as `verified_without_route_replay`; it is not a claim that
the original candidates are still current. A blocked replay or preflight remains an explicit failed
verification status, and both dispatch and execution remain `not_started`.

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

Each group also includes an advisory `gates.artifact_evidence` posture from the current
digest-verified artifact registry. A record matches only through an exact case-normalized declared
registration domain intersection or an explicit `artifact.group_id`; subject text and kind names
are never used for inference. The posture reports matching counts, artifact families, verification
states, parent linkage, and match basis. `artifact_evidence` is intentionally optional: missing
records are visible, but they never pass a required gate, change `gate_state`, or create readiness.

Each group also includes `gates.reconciliation_evidence`, a bounded posture lookup joined by exact
`workflow_id` to a digest-valid retained reconciliation report. Its state is `missing`, `incomplete`,
`invalid`, or `structurally_ready`: missing is explicit and never inferred as a pass; incomplete or
invalid retained posture forces `insufficient_evidence`; and structurally-ready is still only evidence
for review. `groups_reconciliation_blocked` counts groups blocked by the latter two states. This
adjunct does not add reconciliation to the seven accepted gate names and cannot authorize release,
safety, clinical, scientific, or deployment claims; it prevents retained contradictory or incomplete
workflow evidence from disappearing behind otherwise-observed control-plane events.

The gate response includes a `gate_digest` over the normalized operations-evidence and
reconciliation projection without the digest field; non-tool review events do not change that
digest, while retained event or reconciliation changes remain visible. `POST /v1/operations/gate-reviews` records a current acceptance as a content-addressed
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

### Evaluator catalogue snapshots, comparison, and evidence bundles

`GET /v1/missions/{mission_id}/evaluator-replay/compare` accepts the same bounded
`include_fixtures` and `max_items` query parameters as replay. Its `catalog_drift` object includes
the historical and current catalogue digests, digest validity/match, historical review and
discovery IDs, compatible and missing referenced adapter IDs, current catalogue counts, and the
explicit `comparison_scope: "historical_digest_and_current_binding_compatibility"`. A matching
digest does not prove evaluator semantics; a changed digest does not identify the exact changed
row unless a caller retained that catalogue snapshot. Summary-only checkpoints use their separate
`historical_catalog_digest` rather than treating a newly recomputed current digest as historical.

Mission review provenance now retains a bounded `catalogue_snapshot` containing all 29 adapter rows,
the catalogue digest, a snapshot digest, row/group counts, and bounded-retention metadata. When that snapshot is
available, `catalog_drift` reports `comparison_scope: "exact_adapter_row_comparison"` plus deterministic
`added_adapter_ids`, `removed_adapter_ids`, `changed_adapter_ids`, `unchanged_adapter_ids`, and
`changed_adapter_fields`. Each snapshot is content-addressed twice: its row digest must match both
`snapshot_digest` and `catalog_digest`; malformed, duplicate, oversized, or tampered snapshots are
reported as invalid rather than treated as historical truth. Older summary-only checkpoints retain the
digest-level limitation and explicitly report `row_diff_status: "not_recorded"`.

`GET /v1/missions/{mission_id}/evidence-bundle` accepts `include_result` (default `false`),
`include_trace` (default `true`), `include_fixtures` (default `false`), and `max_items` (`1..512`,
default `128`). The export is capped at 2 MiB, includes a deterministic `bundle_digest`, and
preserves `result_digest`/`result_omitted` metadata even when the result body is not included.
`include_result=true` is accepted only when the authoritative result was retained; an omitted
result remains explicitly omitted. The bundle's `execution` posture is always `not_started` for
the replay/comparison sub-workflows, and the trace is observational evidence rather than a second
execution result.

Trace omission is represented as `trace: []` and `export.trace_included: false`, so consumers never
have to interpret `null` as an ambiguous omission. `POST /v1/evidence-bundles/verify` accepts
`{ "bundle": <exported bundle> }` and recomputes the canonical bundle digest, retained-result digest,
schema, retention, trace, and export-contract checks. It returns a verification report with
`valid: false` for digest tampering and a 422/413 response for malformed or oversized input. Neither
route executes a domain tool or evaluator, and neither silently truncates an oversized artifact.

`POST /v1/evidence-bundles` applies that verifier before inserting the artifact into a bounded,
content-addressed registry. Re-importing the same canonical bundle is idempotent. The indexed
`GET /v1/evidence-bundles` query is deterministic by bundle digest, supports mission/domain filters,
and returns compact rows by default; `include_bundles=true` is explicit because full bodies consume
the registry's 32 MiB checkpoint bound. Configure `--evidence-state <file>` to make the registry
restart-safe. Startup rechecks the checkpoint digest and independently re-verifies every stored
bundle; corrupted snapshots are rejected rather than partially restored. Restored evidence never
resumes a mission, reruns an evaluator, authenticates provenance, or becomes a scientific, clinical,
or release claim. The `persistence/flush` route is an explicit operator checkpoint and returns 409
when no evidence-state path was configured.

## Asynchronous missions

`POST /v1/missions/preflight` is the synchronous no-dispatch planning endpoint. It accepts the
same mission document, validates the original execution policy (including the allow-list) and
static tool schemas, then returns the authoritative `agent_mission` plan with
`preflight: true`, `execution: "planned"`, and `dispatch: "not_started"`. It never creates a
job, records an execution event, or invokes a nested domain tool. Binding-dependent arguments
are represented in the plan and are validated again only when an execution request materializes
them.
The same document may include a ready `route_review` from `/v1/capabilities/route/review`. The
gateway fails closed unless its goal and serialized mission-draft steps exactly match the submitted
mission, its review/route/catalogue digests are valid, and any `evidence_binding` remains
`carried_forward_not_recomputed`, `readiness_claimed: false`, and `execution: "not_started"`.
The returned plan exposes compact `route_review_provenance`; this is retained audit structure, not
an authorization or readiness credential. Legacy reviews without route evidence remain explicit
absent bindings.

The same `route_review` may be supplied to `domain_workflow_instantiate`; the workflow kernel
copies it into the generated mission only after the normalized steps are known, so a reviewed
route cannot silently drift while crossing from cross-domain planning into a workflow template.
When that mission is submitted, the queue checkpoint exposes only a `spec_digest` and compact
`route_review_provenance` (never the original job specification). Mission status and inventory
persist the same bounded projection. Terminal evaluator replay reports `route_review_status` as
`absent`, `valid`, or `invalid`, and `domain_workflow_reconcile` compares retained provenance with
the workflow instantiation, producing an explicit integrity finding on mismatch.

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
