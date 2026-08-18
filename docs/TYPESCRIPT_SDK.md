# TypeScript SDK

This document is the executable integration note for blueprint modules **11.06** (TypeScript SDK)
and **40.15** (TypeScript SDK contract). The package and its tests are intentionally kept beside
the Rust workspace so those two modules have a citable, reviewable implementation rather than a
foreign-artifact placeholder.

Provider connectors use `domainEvidenceProviderConnectorHandoff()` before normalization. The
typed manifest records connector scope, capabilities, caller-asserted authentication posture, and
opaque secret references only. The client rejects credential material, validates lowercase digest
parents, and preserves the server's `not_started`/readiness-false boundary; plugin launch,
authentication, network access, and provider validity remain caller responsibilities.
`domainEvidenceProviderExternalPayloadReceipt()` provides the corresponding large-payload path:
only the payload digest, byte length, transfer identity, storage/locator metadata, and handoff
parent cross the API boundary. Payload bytes, credentials, and fetch authority remain outside the
core, while `available`, `durable`, and `not_started` stay explicitly typed.
`domainEvidenceProviderExternalPayloadReplayVerify()` compares the retained receipt digest,
handoff digest, payload digest, and byte length without opening the caller locator; its typed
result exposes each match dimension and preserves the mismatch/non-readiness boundary.
`domainEvidenceProviderExternalPayloadNormalize()` carries the bounded caller-materialization
contract into the typed SDK and exposes receipt/materialization/intake lineage while retaining
the digest, locator, and readiness boundaries.
`domainEvidenceProviderExternalPayloadLineageAudit()` reconciles the receipt against the retained
connector handoff and exposes matched, partial, mismatch, and orphaned states plus each boolean
scope comparison. The result remains registry evidence only: no provider, storage, locator, or
credential access occurs, and readiness stays false.
`domainEvidenceProviderExternalPayloadExecutionEvidence()` retains caller-reported transfer
observations, compares optional payload digest/size evidence with the receipt, and exposes the
same matched/partial/mismatch/orphaned posture. Its executor and locator fields are typed caller
assertions only; the SDK performs no transfer and exposes no readiness claim.

The repository ships `typescript/`, a small ESM package for clients that can use the standard
Fetch API. It is intentionally an integration layer over `bioprism-api`, not a second domain
implementation. The Rust MCP server remains the authority for tool schemas, refusal semantics,
canonical serialization, and scientific contracts.

## Start a gateway

```bash
cargo run -p bioprism-api -- --root . --bind 127.0.0.1:8787 --token 0123456789abcdef \
  --mission-state .local/mission-state.json --event-state .local/event-state.json
```

In another terminal:

```bash
cd typescript
npm install
npm test
```

The production package has no runtime dependency. The `fetch` implementation can be supplied for
Node, a browser, a service worker, a test double, or a platform-specific request layer:

```typescript
const api = new ApiClient({
  baseUrl: "http://127.0.0.1:8787",
  bearerToken: process.env.PRISM_TOKEN,
  timeoutMs: 15_000,
  maxResponseBytes: 2_000_000,
  fetch: globalThis.fetch,
});
```

The gateway token must contain at least sixteen visible characters. Never put it in a URL, query
parameter, browser local-storage value, or a public client bundle. Browser deployments should
normally call an operator-owned same-origin proxy so the gateway token stays server-side; the SDK
supports browser fetch for controlled, non-secret deployments and local development.

## Transport and errors

`ApiClient.request` accepts only origin-form paths and the gateway's bounded HTTP methods. JSON
payloads are checked for unsupported values, non-finite numbers, excessive nesting, control-line
breaks, and a configurable byte ceiling. Responses are read incrementally through
`ReadableStream`, stopped when `maxResponseBytes` is crossed, decoded as UTF-8, and required to be
JSON objects on JSON routes.

Errors are deliberately distinct:

| Error | Meaning | Retry implication |
|---|---|---|
| `ArgumentError` | the caller supplied an unsafe or unbounded value | fix the call; do not retry unchanged |
| `TransportError` | fetch, timeout, abort, or response-read failure | retry only under the caller's policy |
| `ResponseTooLargeError` | the response crossed the local byte ceiling | narrow the route/page or raise the bound intentionally |
| `ApiError` | the gateway returned an HTTP error and structured payload | inspect status/code; a 4xx is not a domain success |
| `ProtocolError` | malformed JSON, SSE, or response shape | investigate compatibility or corruption |
| `ToolRefusalError` | `requireToolSuccess` observed an MCP/domain refusal | preserve the refusal; do not blindly retry |

`callTool` itself does not throw for a structured tool refusal when the HTTP call succeeded. This
is important for safety, evaluation, and evidence workflows where a refusal is a valid result that
must be rendered, stored, or compared. Use `requireToolSuccess` only at a boundary whose contract
requires a successful tool result.

## Discovery and cross-domain calls

The client exposes `health`, `ready`, `capabilities`, `tools`, and `metrics` for startup and
operator discovery. `tools()` returns the server's live catalogue rather than a stale generated
list. `callTool` accepts any path-safe tool name and a JSON object, so a client can use new Rust
tools before the TypeScript package has a convenience method.

The typed artifact registry facade covers `artifactRegister`, `artifactQuery`, `artifactGet`,
`artifactLineage`, `artifactCrossStoreAudit`, `artifactRegistryPersistence`, and `flushArtifactRegistryPersistence`, plus
the generic `artifactRegistryAudit` MCP call. Artifact records are exact-content indexed and
lineage responses preserve missing parent digests and cycles; the SDK does not promote index
presence into causal provenance, scientific validity, clinical safety, publication authority, or
external-effect completion.
`artifactCrossStoreAudit` reports bounded exact-digest agreement across the artifact, evidence,
and workflow-reconciliation stores, including missing projections, orphaned projections,
wrong-kind findings, generations, and checkpoint identities. The stores are observed separately;
the result is not a transaction or a completeness claim.
`domainEvidenceHarmonize` and `domainEvidenceHarmonizeTool` expose the same-subject join boundary
with explicit report-link roles, digest-addressed artifact indexing, catalogue validation, and
always-review-required posture. The `DomainEvidenceHarmonizationResult` keeps traceability and
contradiction state typed, including per-report bridge class/mode and lineage counts, without
converting report presence into scientific, clinical, causal, publication, release, or execution
validity.
`domainEvidenceHarmonizationCoverage` and
`domainEvidenceHarmonizationCoverageTool` query the retained harmonization index with typed subject,
domain, bridge, traceability, cursor, page-size, and digest-inclusion options. The result separates
matching rows from the returned page, exposes explicit continuation state and digest-bound summaries,
and never interprets a retained join as execution, provenance completeness, validity, or readiness.
`domainEvidenceIntake` and `domainEvidenceIntakeTool` expose the raw-envelope boundary with typed
group/tool/domain membership, explicit outcome states, optional request JSON, required response
JSON, separate request/response digests, and indexed artifact posture. Request omission remains
distinct from supplied `null`; the client does not turn intake into execution or scientific,
clinical, causal, provenance, release, or readiness authority. `source_plan_digest` optionally
binds the envelope to a retained source plan; the server checks group, subject, source-tool, and
domain compatibility before indexing.
`domainEvidenceCoverage` and `domainEvidenceCoverageTool` provide the catalogue-wide raw-intake
audit. Their typed result keeps missing groups, outcome/source-tool/subject/domain rows,
declared-tool/domain gaps, optional intake digests, domain summaries, and the coverage digest
explicit; group, tool, and domain completeness remain separate, and complete retained intake is
not execution, scientific validity, provenance completeness, or release readiness.
Coverage rows optionally expose report-class counts, bridge modes, and lineage-parent counts, while
the top-level bridge summary aggregates ordinary, adapter, inline-provider, and external-provider
projections. These are retained-index diagnostics only; linked parents and bridge classification
do not prove execution, provenance completeness, or scientific validity.
`domainEvidenceSourcePlan` and `domainEvidenceSourcePlanTool` expose the corresponding typed
external-source planning boundary. The result preserves connector/locator classes, retrieval
mode, expected digest, policy, parent links, and non-fetching posture; it does not turn a URI,
path, or opaque reference into retrieved provenance.
If the plan includes an expected content digest, the server compares it against the canonical
response digest of a later bound intake before indexing.
`domainEvidenceSourceExecute` and `domainEvidenceSourceExecuteTool` consume a retained plan through
the bounded local-file/plain-HTTP connector kernel. The typed result preserves transport outcome,
raw-content digest, canonical response digest, and the automatically indexed intake; traversal,
HTTPS, redirects, unsupported connectors, and disallowed hosts remain explicit refusals.
`domainEvidenceProviderNormalize` and `domainEvidenceProviderNormalizeTool` cover the caller-
managed literature, clinical-trial, FHIR, object-store, and provider-API boundary. They require
an explicit provider-shaped object/array payload, preserve provider/payload/request identities,
default an omitted outcome to `unknown`, and feed the same catalogue-bound intake path. The SDK
returns a typed structural `shape_audit` with `structured`, `partial`, `refused`, or
`unclassified` status, recognized-container metadata, row/invalid-row counts, identifier
field-presence coverage, and optional object-store content-digest coverage. The audit digest is
based on shape facts and deliberately excludes payload values. The SDK does not contact providers
or infer authenticity, terminology, scientific, clinical, or release validity from normalized
fields. The result also carries a bounded `record_index` of canonical row digests with explicit
omission counts for digest-only deduplication; row digests do not expose identifiers or values.
`domainEvidenceProviderReplayVerify` and its `Tool` alias re-submit a caller-managed payload
against the retained payload, request, shape, normalization, and intake digests. The typed result
keeps every match dimension and the idempotent value-free replay artifact visible; a mismatch is
not promoted to a provider success or authenticity claim.
`domainEvidenceProviderExternalPayloadEvidenceQuery` and its `Tool` alias expose the joined
receipt/lineage/execution projection. The bounded request supports group/domain/subject filters,
digest cursors, and optional artifact bodies; typed rows preserve missing, receipt-only, partial,
and complete join status. The client validates cursor and page bounds and refuses credential
fields, while the server remains read-only and does not fetch, open, or authenticate external data.

Convenience methods currently cover:

- `domainWorkflowCatalogue` / `domainWorkflowCatalogueQuery`: the MCP and REST projections of
  one deterministic workflow template per capability group, including missing tool definitions,
  typed per-tool/domain contracts, advisory stages, and catalogue/workflow digests;
- `domainWorkflowInstantiate` / `domainWorkflowInstantiateQuery`: group-scoped mission
  instantiation with explicit bounded steps, selected-tool scope, typed step-level evidence plan,
  authoritative no-dispatch preflight, and `execution: "not_started"`. These methods do not infer
  domain arguments or authorize tool execution. The returned mission carries a digest-bound
  `workflow_binding` so the exact workflow/contract/evidence scope survives dispatch;
- `domainWorkflowScaffold` / `domainWorkflowScaffoldQuery`: deterministic planning helpers that
  choose one available tool per advisory stage (or honor explicit tools), preserve bounded
  per-tool argument contracts, and return structured ready/blocked preflight without dispatch.
  `DomainWorkflowScaffoldResult.readiness_claimed` is the literal `false`; the scaffold is never
  permission, evidence, clinical guidance, or a domain conclusion;
- `domainWorkflowReconcileQuery` / `domainWorkflowReconcile`: correlate a retained mission report
  or evidence bundle with an instantiation and return typed integrity, per-step evidence, trace,
  completion, and omission posture. The result remains review-required and non-executing.
- `domainWorkflowReconciliationImport` / `domainWorkflowReconciliationQuery` /
  `domainWorkflowReconciliationGet` expose the durable digest-bound audit registry over REST;
  `domainWorkflowReconciliationImportTool` / `...QueryTool` / `...GetTool` expose the same bounded
  operations through MCP. Query rows are cursor-ordered and filterable by mission, workflow, plan
digest, and completion status. Configure `--reconciliation-state` on the API for restart-safe
persistence; lookup and restore never resume execution or imply provenance, scientific, clinical,
safety, or release validity.
Trusted mission, evaluator-replay, evidence-bundle, and workflow-reconciliation responses may also
include an `artifact_registry` projection containing the exact cross-domain index digest and
explicit checkpoint posture. This is a non-claiming audit link; ordinary domain results remain
unindexed unless explicitly registered.

- `traceOtelIngest`: bounded OTLP JSON import with typed normalized events, source-to-IR mapping,
  semantic-loss categories, and compilation-readiness reporting;
- `qualityGateRun`: typed serialized Dataset/Gate/ReferenceSets with externally tagged check
  outcome and verdict unions. `Pass` keeps examined counts, `Fail` keeps a concrete witness,
  `NotRunnable` keeps missing-column/null-only/type/reference reasons, and `Failed` retains its
  separate `not_runnable` set; the client never promotes an indeterminate run to pass;
- `atlasReport`: typed capability coverage, measured-entry depth, hole/influence records, family
  and divergence histograms, coverage debt, internal inconsistencies, and eligible-versus-refused
  composite results with omission counts kept in the REST/MCP envelope;
- `atlasSurfaceAudit`: typed atlasx publication-surface coverage, named debt discharge,
  withheld failure buckets, denominator-safe rate checks, and surface soundness with fail-closed
  policy/refusal fields. See [ATLAS_SURFACE_AUDIT.md](ATLAS_SURFACE_AUDIT.md).
- `adaptivePanel`: typed panel audit totals, parent-aware coverage shortfalls, stopping verdicts,
  reportable estimates with naive/clustered intervals, deterministic selection records, optional
  capability/comparison projections, and explicit refusal/finished states;
- `metricsProfileAudit` and `metricsAnalyticsAudit`: missingness-aware capability profiles plus
  bounded scalar, paired-contrast, cost/latency, replicate, and calibration analytics;
- `telemetryProject`: canonical-event redaction with a typed `TelemetryProjectionResult`; its
  record, exact dropped/coarsened loss, and observed-supported versus asserted-refused metric
  result remain visible without claiming OTLP export or backend delivery;
- `ledgerIngest`: bounded event admission with typed recorded/duplicate/quarantined variants,
  causal release receipts, chain and clock witnesses, temporal-cut entries, and digest-only
  projections; it does not claim durable storage or read a clock;
- `bioCapabilityEvidenceAudit`: evidence posture;
- `bioAtlasPublicationAudit`: atlas, evidence, card, and leaderboard release gates;
- `developerDeliveryAudit`: developer-platform delivery evidence, with independent opt-in
  `ci_execution_evidence`, `ci_provider_evidence`, and `execution_provenance` targets;
- `developerWorkbench`: digest-bound authoring/notebook audit, capability dashboard query, and
  review-only CI workflow planning;
- `ciExecutionEvidenceAudit`: digest-bound reconciliation of a supplied CI run against a freshly
  generated workbench plan, with per-check result digests, complete/missing/non-passing findings,
  provider provenance, and structural-only verification. Its ready flag is a handoff signal, not
  proof that a runner, provider signature, deployment, or scientific workflow was verified;
- `executionProvenanceAudit`: reconciles a returned mission report with its plan, terminal trace,
  and delegated-check digests, exposing structural validity and a bounded provenance-ready signal
  without replaying the mission or contacting a provider;
- `agentMission`: deterministic cross-domain mission planning or explicitly allow-listed execution
  with refusal propagation, output budgets, and optional JSON-pointer bindings;
- `submitMission`, `missionStatus`, and `cancelMission`: typed asynchronous mission jobs with
  bounded polling, authoritative terminal reports, and cooperative cancellation between nested
calls or parallel batches;
  `deleteMission` removes only terminal jobs from the bounded registry, preserving active work.
- `runtimeExecutionSimulate`: deterministic replay, budget, fault, and fork evidence.
- `repositoryCatalog`, `repositoryBundle`, and `repositoryImpact`: bounded repository discovery,
  route-specific progressive disclosure, and changed-module impact checks;
- `telemetryProject`: redacted telemetry projection with explicit treatment policy, trace, and
  optional observed-metric evidence fields.
- `operationsCatalog`: bounded storage/topology parity, service-contract divergence, SLO-name,
  and defined-versus-undefined metric-debt evidence;
- `opsAcceptance`: typed `met`/`refuted`/`unverifiable` operational acceptance findings without
  fabricating a release percentage.
- `measurementCompare`, `hubSearch`, `hubResolve`, and `hubLock`: standards comparability and
  federated authority/freshness/dependency evidence with bounded omission accounting;
- `worldClaimCheck` and `observedWorldDeclare`: provenance-limited claims and pinned observed-world
  declarations;
- `lineageAudit`, `preanalyticApply`, and `contradictionReview`: typed specimen identity gaps,
  biology-preserving pre-analytic mutations, and set-valued multimodal contradiction review.

The complete live tool surface is also available through `toolCatalogue()`, `planTool()`, and
`toolChecked()`. The catalogue is bounded and SHA-256 addressed from `/v1/tools`; plans are
side-effect free and enforce only conservative transport-shape rules. Unsupported JSON Schema
features are warnings rather than hidden validation, and `toolChecked()` keeps structured MCP
refusals in the same raw response envelope as `callTool()`.

`missionPreflight()` applies the same contract to an `AgentMissionArgs` dependency graph, and
`assertMissionPreflight()` converts a failed report into a typed local error. The report returns
request and catalogue digests, deterministic waves, per-step schema reports, JSON-pointer binding
findings, recursion checks, execution allow-list findings, and parallel-wave budget checks without
issuing a tool call. `execution_mode: "parallel_waves"` explicitly opts into bounded concurrent
dispatch for independent steps; `max_parallelism` caps each batch at 16 or less, and serial execution
remains the default. Executed `agentMission()` responses type the authoritative clock-free
`execution_trace` with contiguous lifecycle, wave, refusal, block, and byte-accounting events. Pass
the earlier `ToolCatalogue` snapshot to guarantee that mission review and subsequent checked calls
refer to the same live schema set; the Rust `agent_mission` tool remains the execution authority.
When a mission carries an instantiation `workflow_binding`, the report additionally exposes a typed
`workflow_reconciliation` compact link after terminal execution. REST and MCP share the indexed
record, while the full digest-bound report remains available through reconciliation lookup; the
link is evidence posture only and never an authorization or readiness claim. The report may also
include an `artifact_registry` link for the exact mission-report bytes and, when available, a
parent edge to the indexed reconciliation artifact.
`MissionJob` makes queued, running, planned, succeeded, partial, failed, and cancelled states
explicit; cancellation is a request to stop future dispatch, not a force-kill or rollback claim.
Its optional `progress: MissionProgress` field provides one bounded shape for queued, live, and
terminal dashboards: phase, current wave, active/completed steps, outcome counters, returned bytes,
and the latest clock-free trace sequence/event. The terminal `result` and its execution trace remain
authoritative for replay and domain interpretation. When the gateway restores a bounded checkpoint,
`recovered_after_restart` is true and the job is an explicit failed interruption; `result_omitted`
exposes byte-count and SHA-256 metadata when a durable snapshot could not retain a large report.
`missionTrace(missionId, after, limit)` provides a typed `MissionTracePage` with ordered events,
an exclusive `next_after` cursor, and explicit retention-gap metadata.
`waitMission(missionId, { timeoutMs, pollIntervalMs, signal })` performs bounded, abortable
polling and returns only a terminal `MissionJob`; `MissionWaitTimeoutError` carries the last
authoritative live snapshot so an operator can resume or cancel without losing progress.
`missionPersistence()` and `flushMissionPersistence()` expose the same bounded checkpoint status
and explicit flush/readiness check for operator tooling.
`missionQueue()` returns a `MissionQueueInventoryResponse` whose `MissionQueueJob` rows preserve
resource class, idempotency, attempt counters, terminal state, and the deliberate
`spec_returned: false` omission. `missionQueuePersistence()` exposes `MissionQueueStatus` with
the content digest, integrity result, startup recovery rows, and explicit `automatic_resume: false`.
Its admission policy projection preserves total-job/active-lease backpressure and resource-class
occupancy. Queue attempt numbers are local fencing tokens, not distributed lease authority.
Its authority projection carries the authority digest, queue digest, revision, event count,
integrity result, and shared-file lock state.
`flushMissionQueuePersistence()` returns the atomic checkpoint byte count plus the resulting queue
status. `releaseMissionQueueLock(operator, reason)` records an attributed operator override when a
cooperating process has left an orphaned local lock. These are shared-local-file authority controls,
not multi-host consensus, network-partition tolerance, provider authentication, tenant isolation,
or external-effect completion.
`eventPersistence()` and `flushEventPersistence()` provide the event-cursor equivalent while
typing the explicit non-durability of webhook subscriptions and pending deliveries.

These helpers type the contract's top-level shape while leaving nested domain records as JSON
objects where the Rust crate is authoritative. That keeps the client useful across all domain
families without maintaining a fragile partial clone of the 200-tool catalogue. `capabilityDiscover`
searches the explicit cross-domain catalogue and returns typed `CapabilityDiscoverResult` matches
with domains, crates, CLI/Python artifacts, ranked fields, and optional authoritative schemas;
`capabilityAudit` returns typed `CapabilityAuditResult` parity counts, schema-quality totals,
invariant flags, duplicate memberships, and optional per-group coverage.
`capabilityDashboard` returns typed `CapabilityDashboardResult` rows with callable/partial/
declared-only readiness, separate crate/CLI/Python/MCP surface counts, schema-backed tool totals,
explicit gap labels, query provenance, and bounded inventory warnings. Its ready flag describes
transport coverage only and is not permission, execution, scientific, or deployment readiness.
`capabilityDashboardQuery` reaches the dedicated `GET /v1/capabilities/dashboard` route with the
same bounded filters, so applications can use a direct REST projection without unpacking an MCP
tool envelope; the returned result shape is identical.
`capabilityRouteRest` and `capabilityRouteReviewRest` provide the corresponding raw REST planning
handoff for cross-domain needs and caller-selected tools. A ready review remains
`mission_preflight_required`; these methods never dispatch the selected tools.
`domainAcquisitionCatalogue` adds a typed cross-domain route registry. Its digest-bound rows keep
bounded file/plain-HTTP transport, caller-managed connectors, native adapter matches, and
Python-delegated adapter matches separate for every selected declared domain, with explicit
scope-match evidence and truncation/completeness flags. It is routing evidence only and never
executes a source or adapter.
`domainReportProject` and `domainReportCoverage` add typed REST access to the explicit report
projection boundary, while `domainReportProjectTool` and `domainReportCoverageTool` preserve the
same contract through the tool dispatcher. The project result includes catalogue membership,
claim posture, limitations, and an exact artifact digest; coverage enumerates missing groups.
Neither report count nor indexing is treated as execution, scientific, clinical, provenance,
release, or readiness evidence.
`bioCapabilityEvidenceAudit`
returns typed `BioCapabilityEvidenceAuditResult` evidence rows, dimension rollups, claim blockers,
omission accounting, optional subaudits, and explicit release posture; `developerDeliveryAudit`
returns typed `DeveloperDeliveryAuditResult` readiness gates, explicit target blockers, release
request state, foreign-surface posture, and optional CI/mission-provenance evidence. Requesting the
`ci_execution_evidence` target requires an explicit `ci_evidence` payload, while requesting the
`execution_provenance` target requires an explicit mission provenance payload; absence or structural
failure blocks only the requested target. Neither signal proves provider execution or deployment
approval. `developerDeliveryReceipt` returns `DeveloperDeliveryReceiptResult` with canonical target rows,
evidence presence/readiness, including the retained provider artifact/log/attestation projection,
and delivery/target/receipt digests. It recomputes the delivery audit
from the nested request so a receipt is a stable structural join key, not a signature, execution
claim, durable record, deployment approval, or release authority. `bioAtlasPublicationAudit` returns typed
`BioAtlasPublicationAuditResult` atlas aggregation, score/evidence gates, leaderboard state, and
`developerDeliveryReceiptVerify` returns `DeveloperDeliveryReceiptVerificationResult` with
independent digest, target, evidence, and readiness match dimensions for a stored receipt and its
completed delivery audit. It verifies supplied structural records only and does not authenticate
external execution or create release authority.
`DeveloperDeliveryAuditArgs.ci_provider` provides the composed provider path: the delivery route
normalizes the raw payload, audits the resulting plan-bound evidence, and returns the intermediate
normalization alongside `ci_evidence`. It is mutually exclusive with `ci_evidence` and remains
caller-supplied structural evidence only.
`DeveloperDeliveryAuditArgs.ci_provider_evidence` provides the attached-evidence path: the route
retains canonical provider artifacts, logs, attestations, and their record digests as a separate
target, then feeds only the canonical run envelope into the independent CI evidence audit. It is
mutually exclusive with both other provider-evidence inputs. Receipt verification detects changes
to the retained provider-evidence row separately from target or canonical CI changes.
`ciProviderNormalize` accepts a GitHub Actions-shaped, GitLab CI, or generic provider payload and returns
`CiProviderNormalizationResult` with a plan-bound canonical evidence object, provider/source labels,
payload and derived-check digest metadata, and warnings for missing provider digests. It is an input
normalizer only: the caller still supplies the payload, and no provider authentication, log fetch,
execution, or release approval is implied.
`developerPlatformStatus` returns typed `DeveloperPlatformStatusResult` evidence for walkthrough
standing, module classification, cookbook verification, declared contract surfaces, diagnostic
findings, exit-code divergences, foreign artifacts, and optional full details. Its bounded counts
and standing fields remain explicit; a clean local projection does not claim foreign SDK, CI, gRPC,
or live-debugger execution.
`tokenContextPlan` returns typed `TokenContextPlanningResult` baseline and optional policy-only
comparison plans, keeping node kinds, restricted flags, mandatory closure, stable handles, and
estimation methods visible. `declared_by_caller`, provider-tokenizer, and mixed totals remain
different types of evidence; the client does not promote any of them to measured provider usage.
`weavelangCompile` returns typed `WeaveLangCompileResult` program identity, whole/semantic digests,
state/liveness/invariant evidence, optional IR, and explicit local execution status. Replay,
completed, and refused outcomes stay separate, and the TypeScript surface does not imply that a
local semantic trace called a network, model, or tool.
`epistemicVoi` returns typed `EpistemicVoiResult` evidence for explicit decision problems and
beliefs, preserving gross value, declared acquisition cost, net value, outcome probabilities,
action changes, and complementarity for bounded non-adaptive bundles. The request types require
explicit row-major losses and per-model likelihood vectors; the result keeps structured
fail-closed refusals visible and does not invent an adaptive policy, hidden prior, causal effect,
or execution step.
`epistemicAdaptiveAcquisition` returns `EpistemicAdaptiveResult` with a recursive exact policy
tree. Its outcome branches preserve probabilities and posteriors, and acquire nodes preserve the
next acquisition, scalarized cost, expected terminal risk, state-node count, and selected depth.
The request is bounded to 16 acquisitions and 16 steps by the shared contract; the result remains
a plan rather than an executed assay or a causal, clinical, biological, or predictive claim. See
[`docs/EPISTEMIC_ADAPTIVE_ACQUISITION.md`](../docs/EPISTEMIC_ADAPTIVE_ACQUISITION.md).
`epistemicContextAudit` adds the observed-context boundary: explicit evidence pools, decision
identification, minimal sufficient contexts, exhaustive rate–distortion points, and bounded
subset evaluations remain distinct. Its minimax non-identification abstention and contradictory
subset refusals are typed instead of being collapsed into an efficiency score. See
[`docs/EPISTEMIC_CONTEXT_AUDIT.md`](../docs/EPISTEMIC_CONTEXT_AUDIT.md).
`epistemicSelectionAudit` exposes the complementary bounded observed-context planner. Its typed
constraints and protected closure feed plain/lazy greedy selection, while the result keeps
exhaustive submodularity status, guarantee applicability, and exact small-instance comparison
separate. Above the structural or exactness caps the client preserves `not_run` posture rather
than implying a factor or optimum. See
[`docs/EPISTEMIC_SELECTION_AUDIT.md`](../docs/EPISTEMIC_SELECTION_AUDIT.md).
`benchmarkTraceAnalyze` returns typed `BenchmarkTraceAnalysisResult` evidence for causal
candidate scores, textual divergence, decision-boundary ranks, reversibility basis, goal-anchored
episodes, repeated-action progress, and reconciled summary counts. Causal verdicts remain distinct
from boundary ranking, environment-produced divergence is not assigned to an agent, and a
fail-closed refusal does not become a benchmark cell or replay claim.
`benchmarkDecisionAudit` returns `BenchmarkDecisionAuditResult` for one selected choice/action
step, including causal alignment, recorded and caller-supplied action projections, separate
agent-visible versus future validation options, coverage counts, input digests, and the typed
failure-card projection. Future provenance, environment divergences, uncited claims, bounded
omissions, and the no-replay/no-approval boundary remain explicit in the result.
`benchmarkIntegrityAudit` returns `BenchmarkIntegrityAuditResult` for corpus-level deduplication,
deterministic holdout assignment, declared contamination, panel calibration, and effective
diversity. It preserves clean-versus-unassessed/leaking counts, raw-versus-effective denominators,
and bounded omission metadata without implying semantic deduplication, execution, or release
readiness.
`benchmarkCounterfactualCheck` returns `BenchmarkCounterfactualCheckResult` for matched
`DecisionCell` pairs, one-factor field declarations, invariant/must-change response outcomes, and
source/follow-up digests. Unmatched movement, no-realism-review status, execution nonclaims, and
fail-closed refusals remain typed rather than flattened into a pass/fail score.
`benchmarkOracleReview` returns `BenchmarkOracleReviewResult` after the kernel reviews a serialized
`ProposedOracle`. The result keeps the reviewer and digest, synthesis strength and determinism,
optional four-way acceptance outcome, and optional `DecisionCell` package visible. Callers cannot
turn serialized reviewed output into a trusted oracle, and exploit, gap-analysis, weak-oracle, or
unattributed-review refusals remain fail-closed.
`benchmarkCompile` returns `BenchmarkCompileResult` for the assembled causal-to-oracle pipeline.
Its input carries an explicit bounded `probe_observations` table; missing subsets are returned as a
fail-closed minimization-probe refusal rather than being guessed. The result keeps the unreviewed
oracle proposal, reduction ratio, confidence decomposition, unmeasured stages, provenance, and
non-execution limitations visible.
`benchmarkCompileReview` extends that request with reviewer identity and world/query references,
then returns `BenchmarkCompileReviewResult` only after the kernel review gate packages a
`DecisionCell`. Compilation-stage and oracle-stage refusals remain explicit, and optional grading
retains the typed acceptance outcome.
`foundationContractCheck` returns typed `FoundationContractCheckResult` evidence with separate
contract, parent-refinement, applicability/maturity, world-class, and transition-plane gates. A
top-level `ok` transport result does not imply admission: callers must inspect the explicit
`verdict` and gate objects, and a refused world claim or plane confusion remains visible rather
than being flattened into a generic invalid-contract error.
`packCatalogue` returns typed `PackCatalogueResult` rows for the bounded benchmark portfolio,
including section counts, capability/domain signatures, oracle ceilings, execution-grounded
flags, release-wave declarations, omitted counts, and duplicate-signature review candidates. It
is a declaration inventory; it does not claim measured performance or a reportable health score.
`packCoverageAudit` returns a typed `PackCoverageAuditResult` from the real portfolio coverage and
capability-family matrix kernels. It keeps the selected pack subset, covered and uncovered family
sets, singly/weakly covered warnings, bounded rows and matrix cells, omission counts, and gap
summary visible. Unknown identifiers and empty selections are fail-closed; coverage remains a
declaration-level portfolio projection, not measured performance. See
[`docs/PACK_COVERAGE_AUDIT.md`](PACK_COVERAGE_AUDIT.md) for the wire contract and interpretation
rules.
`packReleaseAudit` returns the declared stable release order plus the explicit unsequenced
remainder from the packs kernel. `PackReleaseAuditResult` preserves selected/global positions,
wave and axis counts, bounded rows, omission reconciliation, and fail-closed unknown or
section-incompatible selection outcomes; it is not a readiness or approval decision. See
[`docs/PACK_RELEASE_AUDIT.md`](PACK_RELEASE_AUDIT.md) for the interpretation contract.
`packHealthAssess` accepts a serialized pack, observed calibration, optional baselines and
contamination signals, and an optional health policy, returning `PackHealthAssessmentResult` with
the digest-bound health report, calibration denominators, tagged findings, and an explicit score
gate. A `reportable: false` gate carries a refusal and `fail_closed: true` with `score: null`; it is
not a zero and it must not be ranked. The TypeScript interfaces keep saturation, floors,
underdetermination, degeneracy, contamination, grounded-oracle tiers, and materialization
findings visible while leaving Rust as the authority for validation and threshold semantics.
`securityRedteamSimulate` accepts bounded finding, vulnerability, delivery, incident, audit, and
attestation rows and returns `SecurityRedteamResult`. Its nested result types keep regression-cell
eligibility, sequential disclosure, within-trial versus across-trial influence, honest containment
claims, audit-chain verification, and observation/assertion status independent. A permitted model
crossing is not an observed transfer, a containment request is not a performed action, and the
client preserves these nonclaims beside the typed evidence.
`worldGenerate` accepts a serialized `WorldSpec` and returns `WorldGenerateResult` with separate
world/query IDs, exact digests, structural counts, warning/error diagnostics, and optional bounded
documents. Parse, query-parse, and world-validation refusals retain their stage and fail-closed
state; a successful transport response is not treated as a side effect or a clinical assertion.
`factoryLifecycleSimulate` accepts serialized jobs, worker capabilities, and ordered lifecycle
actions, returning `FactoryLifecycleResult`. The nested interfaces preserve lease ownership,
heartbeat/expiry recovery outcomes, stage-versus-commit visibility, compensation, quarantine,
cancellation, final snapshots, and fail-closed trace rows without moving lifecycle semantics into
TypeScript. Unknown action kinds remain representable so the Rust authority can return an explicit
refusal; the SDK only types the bounded request shape and the evidence-bearing response.
`storageLifecycleSimulate` accepts a caller-supplied logical epoch, tiering policy, access records,
and quota accounting actions, returning `StorageLifecycleResult`. `StorageTieringResult` keeps
planning, application, pin protection, skipped moves, and truncation separate; `StorageQuotaResult`
keeps reserve-protected purpose allowances, raw class charges, independent row refusals, and
non-copyable child delegation visible. The Fetch client does not move bytes, schedule jobs, or
authorize writes in an external storage backend.
`registryLifecycleSimulate` accepts optional attested pack documents, a serialized continuation
index, a tier policy, and bounded lifecycle actions, returning `RegistryLifecycleResult`. The
interfaces keep pack preflight validity, initial/final integrity, action-level refusal, lookup and
verification results, append-only log events, and the optional continuation index separate. It is
an in-memory local registry projection; typed transport evidence does not imply signatures,
publisher identity, federation, moderation, authentication, or network publication.
`cacheInvalidationSimulate` accepts a key schema, cache entries, a dependency graph, an optional
changed resource, lookups, explicit apply epoch, and reproof rows, returning
`CacheInvalidationResult`. The types keep complete versus partial invalidation, unknown regions,
removed versus marked-unproven entries, miss-reason variants, hit proofs, and post-reproof state
visible. The client does not infer dependencies, run a scheduler, or silently serve an unproven
cache entry.
`hubDisclosureReview` accepts an optional serialized continuation ledger and ordered disclosure
actions, returning `HubDisclosureReviewResult`. The interfaces preserve immutable digest binding,
the monotonic unknown/held-out/disclosed/contaminated state, contamination witnesses, split-
integrity action results, headline labels and caveats, score-withholding refusals, and the
fail-closed action trace. The Fetch client records supplied findings only; it does not discover
leaks, turn a valid split into a secrecy certificate, or publish a network artifact.
`hubCardRender` accepts serialized moderation/card inputs and returns `HubCardRenderResult`. The
card result keeps moderation-derived publication state, access, verification, provenance,
non-claims, limitations, and score display typed. Published scores retain their disclosure label;
withheld scores retain the state and reason, while disclosure/publication-gate refusals remain
fail-closed with a null attachment. The Fetch client renders a contract object only; it does not
generate HTML or publish a page.
`hubLeaderboardRender` accepts a serialized board, entries, moderation ledger, and disclosure
ledger and returns `HubLeaderboardRenderResult`. Optional details retain ranked entries and typed
unranked reasons; summary mode still preserves rank counts, leader counts, scoped caveat text, and
the non-clinical/non-universal headline. `bioatlasPublicationAudit` returns
`BioAtlasPublicationAuditResult`, keeping atlas/evidence/card/leaderboard gates separate from an
explicit target-by-target release request. A ready target is contract eligibility, not web
publication or scientific authority.
`hubSubmissionReview` accepts serialized draft, submitter, and optional moderation inputs and
returns `HubSubmissionReviewResult`. Acceptance stage, moderation stage, append-only event count,
publication state, verification status, and fail-closed refusal fields remain visible; the raw
ledger is retained for audit consumers. It is a contract replay, not authentication, persistence,
identity verification, or network publication.
explicit publication-target blockers; `capabilityRoute`
batches named needs into a non-executing, digest-bound route proposal; `missionFromRoute` turns a
fully resolved route into a provenance-preserving explicit mission only after caller-selected
tools and arguments are supplied. The route response retains per-need candidate domains and a
`route_coverage` ledger so a caller can see which domains contributed evidence before selecting
tools. `adapterPlan` returns a typed `AdapterPlanResult` that selects native or Python-delegated
biological and clinical source routes—including FHIR—by explicit format and source shape while
preserving the complete candidate list, refusal reasons, dependency posture, conformance level,
accepted formats, scope dimensions, and semantic-loss boundaries. Its nested `plan` remains
planning evidence with `execution: "not_started"`; a selected route is not authorization or
source-specific conformance.
`tabularIngest` returns typed `TabularIngestResult` evidence for the real CSV/TSV adapter: explicit
manifest identity, conformance check outcomes, semantic-loss variant, bounded facts, and omitted
fact counts. It does not infer a format or turn adapter conformance into source truth.
`adapterExecutionEvidence` and its `Tool` alias provide the common evidence envelope for native
and Python-delegated adapters across every domain route. `AdapterExecutionEvidenceArgs` validates
declared adapter/source identity, digest bindings, execution/conformance states, bounded loss rows,
counts, and refusal codes; `AdapterExecutionEvidenceResult` retains the indexed artifact and the
literal `execution: "not_started"`/`readiness_claimed: false` boundary. The Fetch client records
caller observations only and never imports dependencies or executes an adapter.
`adapterExecutionEvidenceQuery` and its `Tool` alias provide the bounded read model for retained
adapter rows. `AdapterExecutionEvidenceQueryResult` preserves cursor state, status filters, and
explicit source/workflow parent-join posture, including missing and unclassified parents; it does
not infer provenance or convert a complete join into readiness.
Its `page_summary` keeps execution, conformance, semantic-loss, join, output, and missing-parent
counts separate and explicitly page-scoped.
`domainReportFromAdapterExecution` composes the same typed evidence request through the canonical
`domain_report_project` tool with `operation: "from_adapter_execution"`. The returned
`AdapterDomainReportResult` retains both the indexed evidence and indexed domain report, links the
evidence digest into report lineage, preserves refusal/partial posture, and keeps
`readiness_claimed: false` with `execution: "not_started"`. The facade validates the nested
evidence and optional conformance object before transport; it never runs the adapter or upgrades
structural evidence into scientific, clinical, provenance, or release validity.
domainReportFromProviderNormalization and
domainReportFromExternalProviderNormalization expose the provider-domain bridge operations.
Their ProviderDomainReportResult keeps the full normalization response typed, composes compact
domain-report evidence, preserves normalization/receipt artifact parents, and keeps inline versus
external-materialization mode explicit. The external facade retains the caller-owned locator
metadata but never opens the locator or treats a digest match as provider authenticity.
`conformanceRun` returns typed `ConformanceRunResult` suite and release evidence, including fixture
drift, test-pyramid counts, bounded case outcomes, and all unmet noncompensatory gates. A null
`results` field means case details were not requested; it is not an empty-suite claim.
`releaseAudit` accepts up to 32 typed `ReleaseAuditCheckArgs` requests and returns
`ReleaseAuditResult` with ordered check rows, required/advisory classification, delegated result
digests, refusal and fail-closed fields, blocker references, and strict release conjunction state.
`repository_impact` and `developer_platform_status` remain advisory-only with a null gate; callers
must not infer release approval from their observations or from an optional check that passed.
`operationsCatalog` returns `OperationsCatalogResult` with local/team storage classes, promise
parity, deployment planes, tenant patterns, service-contract counts, metric definitions, and
bounded omission metadata. `opsAcceptance` returns `OpsAcceptanceResult`; its `is_decidable` and
`is_release_ready` fields remain mechanical predicates over the three-way verdict counts, not
claims that unobservable checkout, CI, or external-service criteria passed.
`safetyReleaseGate` accepts a typed `RiskAssessmentArgs` with the closed nine-dimension risk
vocabulary and returns `SafetyReleaseGateResult` with the exact decision, high-risk drivers,
unrated dimensions, fail-closed flag, and conditioned controls. `medicalBoundaryCheck` accepts
typed research or clinical output labels and returns `MedicalBoundaryResult`; research use cases
may be admitted, while clinical categories remain a structured refusal with an unconditional
boundary. These result types preserve policy evidence and do not imply content classification,
runtime security, medical advice, or deployment approval.
`safetyPosture` returns `SafetyPostureResult` with count-reconciled threat populations and optional
`SafetyThreatResult` details. Its `enforced`, `declared_only`, and `absent` mitigation states stay
distinct, as do residual and unanalysed threat IDs; the explicit perimeter nonclaim remains part
of the result rather than being inferred from a green transport response.
`measurementCompare` returns `MeasurementCompareResult` with a tagged comparable/blocked verdict,
explicit conversion records, caveats, a report SHA-256, and the closed first-blocking reason
vocabulary. The input preserves caller-supplied standards declarations as JSON so the Rust
standards kernel remains authoritative for units, frames, builds, and ontology bindings.
`literatureBindCheck` returns `LiteratureBindCheckResult` with separate `bound`, `citable`, and
`outcome_kind` states. Its evidence retains source-binding refusals, historical-horizon checks,
population scope, retraction warrants, and citation refusals for unsupported biological claim
kinds. The TypeScript boundary does not turn a literature statement into a measurement; the
literature modality admits only `published_claim_support`. See
[`docs/LITERATURE_BIND_CHECK.md`](LITERATURE_BIND_CHECK.md).
`modalitySupportCheck` returns `ModalitySupportCheckResult` for the authoritative assay support
relation. It keeps modality/claim eligibility, first refusal, root refusal, and independent-unit
pseudoreplication evidence distinct while preserving custom descriptor and claim-requirement
metadata; a supported result is not a claim of truth or statistical validity. See
[`docs/MODALITY_SUPPORT_CHECK.md`](MODALITY_SUPPORT_CHECK.md).
`modalityTransportCheck` returns `ModalityTransportCheckResult` with loss ledgers,
exact-versus-estimated fidelity, scope-mapping checks, invertibility refusals, post-transport
resolution, and optional claim-support deltas. The client exposes structural transport evidence
without implying moved values, validated references, or recovery of discarded information. See
[`docs/MODALITY_TRANSPORT_CHECK.md`](MODALITY_TRANSPORT_CHECK.md).
`modalityComparabilityCheck` returns `ModalityComparabilityCheckResult` with modality-first
measurand and resolution checks, optional delegated standards evidence, typed blocked reasons, and
a canonical report digest. The client keeps standards comparability separate from biological
equality, calibration, or agreement. See [`docs/MODALITY_COMPARABILITY_CHECK.md`](MODALITY_COMPARABILITY_CHECK.md).
`obligationGateCheck` returns `ObligationGateCheckResult` with the Rust obligation kernel's typed
allowed/blocked gate, effective state and mandatory-closure evidence, bounded frontier projection,
and graph digest. It does not execute an action or authenticate authority. See
[`docs/OBLIGATION_GATE_CHECK.md`](OBLIGATION_GATE_CHECK.md).
`hubSearch` returns `HubSearchResult` with typed exact-facet matches, required `why` evidence,
near-miss exclusions, trust tier, authority provenance, freshness variants, and explicit bounded
truncation. Federation and catalog values remain JSON inputs because the Rust hubapi owns their
serialization and authority validation; the client does not invent registry membership or
semantic-search claims.
`hubResolve` and `hubLock` preserve the same federation decisions through typed resolution subjects,
accepted freshness policy, lifecycle notes, dependency requirement sources, digest provenance, and
bounded lock omissions. A lock response remains visibly partial when `omitted_entries` is nonzero;
the client does not infer a complete closure from a capped page.
`worldClaimCheck` returns a discriminated `WorldClaimCheckResult`: grounded observed-world claims
retain their caveat and rung ancestry, while mechanistic or otherwise unsupported claims remain
structured `ok: false` refusals with `fail_closed: true`. The TypeScript boundary does not promote
synthetic or simulator evidence to biological validity.
`observedWorldDeclare` types the declaration artifact that feeds that boundary: pinned sources,
public versus controlled access, cohort strata, selection, outcome labels, controlled-source
names, and the observed-only provenance projection remain explicit instead of being inferred from
an identifier or a green transport response.
`lineageAudit` types bounded specimen-registry audits, keeping ancestry/material findings separate
from incomplete fingerprint identity evidence and preserving omitted-row counts. `preanalyticApply`
types admitted biology-preserving fault signatures as well as fail-closed mutation refusals;
response checks, family null controls, and detectability evidence remain separate fields.
`contradictionReview` keeps typed hypotheses, discriminating actions, answer-cue diagnostics, and
the resolved/not-yet-examined/unresolvable state machine in the REST/MCP envelope without choosing
a correct modality. Nested domain records remain JSON objects where the Rust kernel is authoritative.
`labPlan` adds typed acquisition ordering, privacy exclusions, spend, stop reasons, and escalation
without executing a lab action. `oncoBoundaryCheck` preserves oncology's partial aggregate release
versus individual-clinical refusal and human-escalation state, including reconciled disposition
counts, escalation routing, and fail-closed identifier handling. See
`docs/ONCO_BOUNDARY_CHECK.md`.
`oncoResponseAssess` types the criteria-aware response projection, retaining the unconfirmed
reading separately from the reportable call and exposing post-treatment progression withholding,
surviving hypotheses, evidence requests, call kind, criterion divergence, treatment-window, and
sensitivity metadata. `oncoWorldlineView` keeps biological and record
orders separate and makes the agent-visibility cutoff/visible-hidden partition explicit.
`oncoClassificationCheck` preserves integrated versus unresolved molecular classification and
assay obligations; `oncoworldsIdentityJoin` returns a typed `joinable: false` domain verdict
without treating it as a transport failure; and `oncoOutcomeAnalyze` keeps estimand, event,
censoring, delayed entry, and informative-bias fields distinct. Nested domain records remain
`JsonObject` values so the Rust oncology crate remains the serialization authority, while the
top-level invariants and contradictions are checked by the SDK.
`oncoworldsModelTransport` preserves model-system fidelity, establishment, declared sample size,
transport assumptions, and typed fail-closed patient-transport refusals. Its versioned result
also carries model identity, passage-specific fidelity, establishment selection, replication
accounting, and refusal kinds. The methylation pair
(`oncoworldsMethylationClassify` and `oncoworldsMethylationCompare`) keeps QC abstention,
threshold/calibration/tumour-content caveats, classifier outcome, score coverage, classifier
change, and version-conditioned disagreement explicit.
`oncoworldsRadiogenomicCheck` retains participant-safe split, training-only fitting, target scope,
mechanism strata, and transport assumptions, plus a versioned support/refusal state, blocked claim,
design summary, refusal kind, and typed supported-claim envelope; `oncoworldsClonalHistoryCheck` returns compatible and
rejected histories with typed reasons and preserves ambiguity when more than one history survives.
`oncoworldsClonalEvidenceCheck` adds specimen-to-tumour promotion, recurrence explanation sets,
declared assay sensitivity/copy-number provenance, and an explicit refusal for temporal treatment
causation. It keeps each check section independent and does not infer a phylogeny or effect.
`oncoworldsEraShiftCheck` adds versioned cohort mapping, site-resource, and population-descriptor
evidence; `oncoworldsEquityCheck` keeps pooled-only and incomplete-interval results refused while
retaining every subgroup. These methods are projections only: they do not perform clinical
classification, patient transport, era harmonisation, or equity estimation. See
`docs/ONCOWORLDS_SHIFT_EQUITY.md`.
`oncoworldsEntityWorldCheck` composes provenance-selection, alteration-mechanism, rare-class
benchmark, lesion-clustering, and competing-event safeguards. Its result preserves independent
section outcomes and reconciled refusal counts; it does not estimate biology, survival, fairness,
or treatment effect. See `docs/ONCOWORLDS_ENTITY_WORLDS.md`.
`stressProfile` and `stressReport` add the biological-stress boundary with typed family identity,
identifiability, intensity-ladder sweep points, effective sample size, unresolved measurements,
required-versus-probed findings, generator defects, and a nullable guarded worst-family projection.
They preserve the kernel’s breaking-point semantics instead of collapsing distribution-shift and
assay-degradation evidence into a scalar robustness score.
`influenceAnalyze` carries the factor-region boundary with explicit exact versus conservative
total-variation estimates, method/validity provenance, attempted-method refusals, budget posture,
and structural-only execution. `unknown` estimates retain their reason object and never become a
numeric infinity or an implicit robustness claim.
`routingDecide` types the approved architecture, confidence score, abstention reason, considered
panel, neighbourhood evidence, and holdout posture; safe-default abstention remains distinct from
a routed win.
`routingLabRun` exposes the bounded offline routing experiment with typed task inputs and result
envelopes. Its report keeps task/regime holdout posture, fixed-default and oracle comparators,
regret, calibration, abstention, task outcome counts, and explicit row omissions visible; it is
not a production routing or deployment approval. See [`docs/ROUTING_LAB_RUN.md`](../docs/ROUTING_LAB_RUN.md).
`labParetoAudit` exposes the inference-lab multi-objective archive. Its typed result retains
dominated candidates, displaced members, measured-versus-unmeasured axes, front-only relations,
unresolved holes, and `unique`/`ambiguous`/`empty` selection without scalarizing the front. See
[`docs/LAB_PARETO_AUDIT.md`](../docs/LAB_PARETO_AUDIT.md).
`labBranchAudit` exposes the ordered risk-triggered branch ledger with undetermined-risk posture,
hard branch/verifier ceilings, trigger plans, catches, wasted escalations, escaped harms, and
explicit row omissions. It is planning evidence, not verifier execution or safety clearance. See
[`docs/LAB_BRANCH_AUDIT.md`](../docs/LAB_BRANCH_AUDIT.md).
`labHoldoutAudit` exposes validated architecture bundles, append-only exposure, clean versus
contaminated measurements, checkpoint/rollback receipts, retained burn, and certification-budget
state. It keeps measurement refusals as auditable rows rather than scores. See
[`docs/LAB_HOLDOUT_AUDIT.md`](../docs/LAB_HOLDOUT_AUDIT.md).
`labEvolutionAudit` exposes the final inference-lab claim boundary. Its result distinguishes
kernel-minted clean before/after evidence, contaminated cards, clean non-improvements, and
fail-closed architecture/completeness/card refusals while retaining bounded measurement rows,
claim obligations, direction, rollback, and defeater context. See
[`docs/LAB_EVOLUTION_AUDIT.md`](../docs/LAB_EVOLUTION_AUDIT.md).
`labSpaceAudit` validates the immutable architecture registry before experiments: required kinds,
acyclic graph and dangling-edge checks, protected surfaces, parent lineage, deterministic bundle
diffs, and separately bounded inspection/comparison rows remain explicit. A valid space is
structural admissibility, not execution or performance evidence. See
[`docs/LAB_SPACE_AUDIT.md`](../docs/LAB_SPACE_AUDIT.md).
`providerCapabilityGate` preserves untested/failed/passed provider states, conjunctive gate
outcomes, measurement counts, reproducible run evidence, and indeterminate cross-provider drift;
performance observations remain measurements rather than invented pass/fail claims.
`sdkRegistryCheck` preserves manifest-validation versus registry-registration refusal stages,
whole/core digests, capability kinds, trust evidence, deterministic resolution, and the invariant
that a refused set has no partial registry.
`oracleCombine` retains tiered decisions, underdetermination, typed oracle judgement identities,
admissibility, suppressed overrides, disagreement source/settlement/resolution, inadmissible and
withheld ledgers, and nullable deciding/confidence evidence. See
[`docs/ORACLE_COMBINE.md`](../docs/ORACLE_COMBINE.md). `oracleReferencePanel` and
`oracleMissingness` preserve reader splits, blinding posture, complete-case admissibility, and
aggregate egress boundaries. `bioevalReferenceAudit` keeps the typed
distribution/unresolved/not-evaluable reference union, mass, resolution, dispersion attribution,
and modal summaries separate; see
[`docs/BIOEVAL_REFERENCE_AUDIT.md`](../docs/BIOEVAL_REFERENCE_AUDIT.md).
`evaluationWorldlineAudit` types accessibility-clock leak witnesses and separate dangling
references; see [`docs/EVALUATION_WORLDLINE_AUDIT.md`](../docs/EVALUATION_WORLDLINE_AUDIT.md);
`evaluationReproductionCheck` types the stable schema, ordered certificate verdicts,
matched/diverged/missing summary counts, first divergence, portability posture, and explicit
biological-validity refusal, keeping reproducibility separate from biological validity; see
[`docs/EVALUATION_REPRODUCTION_CHECK.md`](../docs/EVALUATION_REPRODUCTION_CHECK.md); and
`evaluationTrajectoryCheck` types step/property ledgers, non-vacuous property outcomes, recovery
transitions, and bounded suffix completeness; see
[`docs/EVALUATION_TRAJECTORY_CHECK.md`](../docs/EVALUATION_TRAJECTORY_CHECK.md).
`bioevalAcquisitionAudit` types ordered acquisition-trace obligations, action kinds, stopping,
redundancy, deferred decisive cost, and named-policy regret. It preserves the no-execution
boundary and does not turn an admissible trace into biological validity. See
[`docs/BIOEVAL_ACQUISITION_AUDIT.md`](../docs/BIOEVAL_ACQUISITION_AUDIT.md).
`bioevalGroundingAudit` types claim/evidence declarations, locator states, support/contradiction/
adjacent edges, the five-way grounding census, freeze-relative staleness, lineage gaps, orphan
evidence, and bounded omission metadata without dereferencing external artifacts. See
[`docs/BIOEVAL_GROUNDING_AUDIT.md`](../docs/BIOEVAL_GROUNDING_AUDIT.md).
`bioevalEstimandAudit` types the five estimand elements, evidentiary basis, claim kind,
identification posture, external corroboration, and declared scope transport. It retains the
kernel-rendered claim language and fail-closed policy options. See
[`docs/BIOEVAL_ESTIMAND_AUDIT.md`](../docs/BIOEVAL_ESTIMAND_AUDIT.md).
`bioevalEvaluatorAudit` types the evaluator-health union, task outcome, diagnostic evidence,
hidden-data access, panel posture, unscored/refused findings, and bounded run projections. It
keeps a broken harness separate from a task failure and retains fail-closed panel and hidden-data
policies. See [`docs/BIOEVAL_EVALUATOR_AUDIT.md`](../docs/BIOEVAL_EVALUATOR_AUDIT.md).
`bioevalPlaneAudit` types capability tiers, dimensions, discriminated scored/unscored/inapplicable
cells, weighted fold projections, fold blockers, and bounded omission metadata. It keeps a
dimension that was not measured distinct from a capability the system could not be asked to
perform, and preserves the explicit `require_fold` refusal posture. See
[`docs/BIOEVAL_PLANE_AUDIT.md`](../docs/BIOEVAL_PLANE_AUDIT.md).
`bioevalMetamorphicAudit` types invariant and directional-change relations, internally tagged
`unchanged`/`moved`/`incomparable` responses, family verdicts, false-sensitivity, false-invariance,
wrong-direction, and undetermined findings. It exposes relation coverage and keeps suite-wide
consistency absent; `require_both_relations` and `fail_on_undetermined` remain explicit policy
gates. See [`docs/BIOEVAL_METAMORPHIC_AUDIT.md`](../docs/BIOEVAL_METAMORPHIC_AUDIT.md).
`bioevalWaiverAudit` types the eight release-gate kinds, tagged met/violated/unevaluable verdicts,
complete waiver evidence, before/after blocking posture, safety-veto findings, and unevaluable
gate counts. A waiver does not rewrite the underlying verdict; `require_releasable` and
`require_no_unevaluable` remain explicit fail-closed policies. See
[`docs/BIOEVAL_WAIVER_AUDIT.md`](../docs/BIOEVAL_WAIVER_AUDIT.md).
`bioevalDesignAudit` types factorial arms, explicit baselines, conclusion/tier vocabularies,
single-factor contrasts, missing interaction cells, unattributable multi-factor arms, and
causal-versus-descriptive attribution results. It keeps `require_contrasts`,
`require_complete_interactions`, and `require_attribution` as explicit policy gates and never
turns a design declaration into an effect estimate. See
[`docs/BIOEVAL_DESIGN_AUDIT.md`](../docs/BIOEVAL_DESIGN_AUDIT.md).

bioevalMeshAudit types evaluator kinds, consumed and derived artifacts, transitive shared-input
classes, within-class versus across-class disagreement witnesses, abstentions, class-collapsed
ratings, and optional ladder contributions. It keeps evaluator count distinct from independent
class count and preserves require_independence and require_independent_ratings as explicit
fail-closed policies. See [docs/BIOEVAL_MESH_AUDIT.md](../docs/BIOEVAL_MESH_AUDIT.md).

bioevalBurdenAudit types integer resource pools, ordered branch inheritance, exact units,
productive versus wasted draws, residuals, nonrenewable fork feasibility, and failed-action waste.
It preserves require_joint_feasible and require_no_wasted_nonrenewable as explicit fail-closed
policies without inventing prices or utility. See
[docs/BIOEVAL_BURDEN_AUDIT.md](../docs/BIOEVAL_BURDEN_AUDIT.md).

bioevalRevealAudit types frozen commitment targets, opaque predictions, analysis plans, rubric and
commitment digests, one-shot seal/reveal locks, uncommitted outcomes, unrevealed commitments, and
selective publication. It preserves require_scoring, require_rubric_match, and require_complete as
explicit fail-closed policies. See
[docs/BIOEVAL_REVEAL_AUDIT.md](../docs/BIOEVAL_REVEAL_AUDIT.md).

bioevalBoundaryAudit types contextual-integrity five-tuples, closed channels, authorized and
compliant-denial effects, violations, vetoes, bypasses, channel exposure, Pareto points, and
guarded composite refusals. It preserves require_no_violations and require_no_vetoes as explicit
fail-closed policies. See
[docs/BIOEVAL_BOUNDARY_AUDIT.md](../docs/BIOEVAL_BOUNDARY_AUDIT.md).

The runtime safety facade adds `runtimeEffectCheck`, `runtimeTapeVerify`, and
`runtimeExecutionSimulate`. Their argument/result types preserve effect declaration and
reversibility class, perform-versus-simulate authorization, fail-closed path/network refusals,
hash-chain and checkpoint verification, simulated provenance, budget exhaustion, partial recording,
complete replay, and counterfactual fork evidence. `runtimeTapeVerify` additionally types the
checkpoint restoration rows, artifact ledger, and reconciled verification counts; see
[`docs/RUNTIME_TAPE_VERIFY.md`](../docs/RUNTIME_TAPE_VERIFY.md). The runtime inspection and simulation surfaces
never reach a host filesystem, network, process, model, message, or payment endpoint.
`runtimeExecutionSimulate` additionally types recording/replay completeness, deterministic-world
state, budget accounting, and fork continuation evidence; see
[`docs/RUNTIME_EXECUTION_SIMULATE.md`](../docs/RUNTIME_EXECUTION_SIMULATE.md).

The bioethics facade adds `bioethicsActionReview`, `humanSubjectScreen`, `bioethicsDualUseReview`,
`bioethicsValidationCheck`, and `bioethicsRepresentationAudit`. These retain physical versus
in-silico action partitioning and referral-only semantics, institutional review versus consent versus
return-of-results, misuse assessment before the section-13 risk gate, missing validation evidence,
and measured versus unmeasured versus small-cell-suppressed strata. The result types intentionally
do not mint physical execution, institutional clearance, clinical approval, or biological truth.
`capabilityRouteReview` accepts the route plus caller-selected `MissionRouteSelection` values and
returns typed `CapabilityRouteReviewResult` diagnostics. Ready results include deterministic
dependency waves and a mission draft while retaining the `mission_preflight_required` boundary;
blocked results preserve structured findings without executing anything. Set `validate_schemas: true`
to receive bounded authoritative schema reports for the selected arguments; this remains review
evidence and does not authorize execution. The typed result also includes a deterministic,
content-addressed `review_id` for correlating the same handoff across transports and event records.
`routeReviewEvidence(reviewId, after, limit)` retrieves bounded retained event evidence for that
exact id as `RouteReviewEvidenceResponse`; the result preserves cursor gaps and distinguishes an
empty retained window from a claim that the review was never produced.
`deliveryReceiptEvents(receiptId, after, limit)` provides the parallel
`DeliveryReceiptEventsResponse` join for content-addressed delivery receipts. The event payload
keeps a bounded receipt projection when a full tool response is too large; it is correlation
metadata, not a replacement for `developer_delivery_receipt_verify`.

## Events and webhooks

`recoveryMatrix()` returns the typed `RecoveryMatrix` view before an operator performs restart
recovery. Its boundary rows distinguish terminal mission restoration, retained event rows,
subscription metadata, pending outbox evidence, process-local secrets, and external delivery
effects; its delivery-attempt boundary is separately queryable; `automatic_resume` and
`automatic_external_delivery` are explicit `false` values.
`missionPersistence()` and `eventPersistence()` separately expose their checkpoint schema and
optional content digests plus `integrity_verified`; a digest is an integrity correlation, not a
claim of distributed consensus.
`missionQueue()` / `missionQueuePersistence()` / `flushMissionQueuePersistence()` expose the
factory-backed mission execution boundary with typed idempotency and recovery posture. A queued
or requeued row is evidence of persisted lifecycle state only; the SDK preserves the API's
no-automatic-resume and no-external-effect claims.
`operationsSnapshot(after, limit)` returns the typed `OperationsSnapshot` control-plane view:
one bounded event cursor page, event metrics, reconciled mission status counts, nested persistence
status (including workflow-reconciliation checkpoint state), and a typed reconciliation posture
summary including its per-workflow status matrix,
the recovery matrix, typed domain-group/tool coverage, compact capability flags, and explicit operator actions and
non-claims, including the explicit non-atomic cross-store consistency declaration. It is a
read-only dashboard/bootstrap call; it does not execute tools, resume jobs,
send webhooks, or promote local observations into scientific or receiver-acceptance claims.
`operationsHandoff(args)` returns the typed `OperationsHandoff` proposal for selected domains or
groups, preserving exact catalogue gaps, unresolved selectors, a content-addressed handoff ID,
and a `CapabilityRouteArgs` proposal. The proposal remains `execution: "not_started"` until the
caller performs route review and mission preflight.
`operationsDomainActivity(after, limit)` returns the typed per-domain activity projection and
keeps the event cursor, exact observed tools, catalogue gaps, and catalogued-but-unobserved tools
separate; its `observation_policy` explicitly does not claim readiness.
`operationsDomainGates(after, limit)` returns typed evidence gates for catalogue, activity,
transport completion, pooled evaluation, domain-evaluator, safety, and release channels. The
domain-evaluator row preserves the exact evaluator tool/event binding without asserting scientific
validity, calibration, or independence. Its group state is explicitly
`catalogue_blocked`, `insufficient_evidence`, or `review_required`; the type contract preserves
`readiness_claimed: false` even when all locally observed channels are present. Each group also
exposes a typed `reconciliation_evidence` posture joined by exact `workflow_id` to the bounded
digest-valid registry. `missing` is explicit and never inferred as a pass; `incomplete` or `invalid`
retained posture blocks review; and `structurally_ready` remains review-required evidence. The
summary's `groups_reconciliation_blocked` counter makes the cross-domain impact visible without
turning reconciliation into a release or safety authorization.
`createOperationsGateReview()` persists a current review and `operationsGateReviews()` replays it
by cursor or content-addressed `review_id`. `AgentMissionArgs.operations_gate_acceptance` then
carries that retained `review_id`, matching `gate_digest`, reviewer, rationale, exact group IDs,
and accepted gate names. The HTTP API revalidates the retained review against current evidence
before accepting an executable mission.
Accepted executable jobs expose `execution_provenance`, and `missionProvenance(missionId)` reads
the same `bioprism-mission-execution-provenance/0.1` projection directly. It correlates the
retained review and gate digest, domain-evaluator evidence, bounded preflight projection, and
accepted-dispatch event; it is an audit/replay record rather than a readiness claim.
`AgentMissionArgs.claim_requests` adds bounded caller-authored claims with explicit required step
IDs and evidence mode. Reports expose `claim_lineage`, while `missionClaimLineage(missionId)` reads
the dedicated response with retained-output digests, omission/refusal states, and explicit
non-claims. `evaluator_bindings` carries explicit adapter/domain/source-pointer coverage; `claimable`
is evidence-retention posture only and never means the statement is true.
`missionEvaluatorDiscover(...)` searches the digest-bound, non-executing evaluator catalogue across
all workspace capability groups. `MissionEvaluatorDiscoverArgs` filters by intent, group, domain,
mission level, adapter ID, and bounded result count; `MissionEvaluatorDiscoverResult` returns the
candidate purpose, related tools, pointer examples, and explicit `selection_posture: "candidate_only"`.
It is a discovery aid for choosing an `evaluator_bindings.adapter_id`, not an evaluator execution or
semantic adjudication route.
`missionEvaluatorReview(...)` is the next non-executing checkpoint. It accepts the discovery payload
and bounded claim-selection rows, then returns typed ready/blocked findings for stale discovery,
unknown candidates, unsupported domains, duplicate IDs, per-claim overflow, and invalid RFC 6901
pointers. Ready rows include proposed evaluator-binding scaffolds, but `execution: "not_started"`
is preserved because `agentMission(...)` remains the execution and refusal authority. A ready review
can be passed back through `AgentMissionArgs.evaluator_review`; the server revalidates its catalogue
digest and exact claim-binding rows before nested dispatch. `MissionClaimEvaluatorEvidence` retains
outcome state, step refusal/error, output source/type/size, and digest-group disagreement evidence,
while `MissionClaimLineageProjection.evaluator_review` preserves review provenance without claiming
semantic truth.
`missionEvaluatorReplay(...)` performs the corresponding non-executing structural audit over a retained
`agent_mission` response. Its typed result replays adapter identity and domain labels, recomputes
outcome/digest counts, exposes disagreement and omission findings, reports represented and missing
catalogue groups, and can return four explicit retained/refused/omitted/disagreement fixture variants
for every standard adapter. `execution: "not_started"` and the structural-only guarantees remain
explicit: replay does not rerun an evaluator or establish scientific, clinical, causal, or release truth.

`missionEvaluatorReplayQuery(missionId, { include_fixtures, max_items })` reads the durable REST
projection after mission completion. Its `retention.mode` is `"full"` when the report body remains
within the checkpoint bound and `"summary_only"` when only the digest/count/coverage summary was
retained; both modes preserve `execution: "not_started"`, explicit limitations, and navigable
mission/claims/replay links.

`missionEvaluatorReplayCompare(...)` exposes the MCP comparison contract, while
`missionEvaluatorReplayCompareQuery(missionId, { include_fixtures, max_items })` reads the same
catalogue-drift evidence through REST. The typed result keeps digest validity/match, historical
review/discovery provenance, compatible and missing referenced adapters, and the explicit
historical-row-retention limitation. `missionEvidenceBundle(missionId, { include_result,
include_trace, include_fixtures, max_items })` exports one bounded content-addressed evidence
bundle with retention/omission metadata, replay and drift projections, optional trace/result,
execution provenance, and a 64-character `bundle_digest`; it never dispatches a domain tool or
evaluator and never silently truncates an oversized export.
When review provenance retains the evaluator catalogue snapshot, the comparison result also exposes
exact added, removed, changed, and unchanged adapter IDs plus changed row fields. Callers can verify
the exported artifact with `missionEvidenceBundleVerify({ bundle })` through MCP or
`missionEvidenceBundleVerifyQuery(bundle)` through REST. Both return the typed
`MissionEvidenceBundleVerifyResult`, including canonical digest recomputation, retained-result checks,
retention/trace/export checks, and explicit failure codes; verification is non-executing.
The registry routes are exposed as `missionEvidenceBundleImport(bundle)`,
`missionEvidenceBundleQuery({ mission_id, domain, after, max_items, include_bundles })`, and
`missionEvidenceBundleGet(bundleDigest)`; the corresponding `*Tool` methods use MCP directly.
Import is independently verified and idempotent, query rows are digest-ordered and bounded, and
get validates the content-hash lookup. Configure the API's `--evidence-state` file for restart-safe
REST persistence; registry presence never resumes execution or asserts scientific, clinical, or
release validity.

```typescript
const page = await api.events(0, 100);
const operations = await api.operationsSnapshot(page.page.next_after, 100);
const stream = await api.eventStream(page.page.next_after, 100);
const receiptEvents = await api.deliveryReceiptEvents("receipt-2026-08-1", 0, 100);
const receiptAttempts = await api.deliveryReceiptAttempts("receipt-2026-08-1", 0, 100);
for (const event of stream.events) {
  console.log(event.id, event.event, JSON.parse(event.data));
}

const subscription = await api.subscribe(
  "https://worker.example.invalid/prism-events",
  "a-long-operator-managed-secret",
  { subscriptionId: "worker-a", events: ["tool.completed", "tool.refused"] },
);
const deliveries = await api.deliveries(subscription.subscription.id);
const attempts = await api.deliveryAttempts(subscription.subscription.id, 0, 100);
console.log(attempts.page.attempts.map((attempt) => [attempt.attempt_id, attempt.outcome]));
console.log(receiptAttempts.found, receiptAttempts.page.attempts.length);
// An operator-owned worker sends the signed envelope, then acknowledges only accepted ids.
await api.acknowledge(subscription.subscription.id, deliveries.page.deliveries.map((d) => d.delivery_id));
// After an event-state restart, restore the signing secret in memory before retry/replay.
await api.rebindSubscription(subscription.subscription.id, "a-long-operator-managed-secret");
// Explicit operator recovery keeps delivery IDs stable and resets selected attempts.
await api.replay(subscription.subscription.id, deliveries.page.deliveries.map((d) => d.delivery_id));
```

The SSE route is a bounded snapshot, not a streaming connection. `eventStream` returns the raw
text, parsed events, content type, and next cursor so an application can decide whether to poll,
persist, or hand off to a real EventSource implementation. The webhook methods only manage the
server-side outbox. They do not send to endpoint URLs, retry on their own, or expose subscription
secrets. Event-state persistence status exposes the optional content `state_digest` and bounded
attempt metrics for operator correlation. Event-state checkpoints can restore metadata, signed
pending rows, and delivery-attempt provenance, but restored subscriptions remain paused until
`rebindSubscription` supplies the secret again. Attempt rows are local gateway/worker evidence;
they do not prove an external effect beyond an explicit sender success result.

## Compatibility posture

The API's `capabilities` response is the runtime compatibility anchor. Clients should check
`tool_count`, transport flags, and limits before enabling a workflow. REST and JSON-RPC calls share
the same in-process dispatcher, but gRPC, TLS termination, distributed event consensus, and an
external delivery worker remain deployment responsibilities. The optional event-state file is a
bounded local checkpoint with explicit migration and secret-rebind semantics; a client must not
infer distributed durability or outbound delivery from the presence of an HTTP listener.

## Engineering manifest audit

engineeringManifestAudit provides typed engineering-manifest digest, package topology, ticket
readiness, ADR supersession, ownership/RACI, and warning/blocking issue evidence. The result
remains an artifact audit and does not imply checkout inspection, CI execution, GitHub state, or
release authority. See ENGINEERING_MANIFEST_AUDIT.md.

`ApiClient.engineeringExecutionPlan(...)` accepts `EngineeringPlanRequestArgs` and returns the
typed `EngineeringPlanToolResult`: bounded ticket selection, dependency-aware waves, critical
path, gate outcomes, issue severity, and plan digests. It is derived evidence only and does not
write to a tracker, execute CI, inspect a checkout, or authorize release. See
ENGINEERING_EXECUTION_PLAN.md.

## Release-pipeline audit

`ApiClient.releasePipelineAudit(...)` accepts `ReleasePipelineManifestArgs` and returns a typed
`ReleasePipelineAuditToolResult`. The types retain stage DAG/readiness, artifact digest and
lineage checks, attestation binding, promotion order, production signature/approval requirements,
rollback presence, and explicit warning/blocking issue rows. `release_ready` is a derived artifact
projection, not proof of CI execution or deployment. The client does not verify signatures,
contact registries, or mutate release state. See RELEASE_PIPELINE_AUDIT.md.

## Operational-readiness audit

`ApiClient.operationalReadinessAudit(...)` accepts `OperationalReadinessManifestArgs` and returns
`OperationalReadinessToolResult`. The types preserve objective/indicator evidence, dependency
fallbacks, runbook review, incident closure, control audits, counts, and stable warning/blocking
issue rows. `operationally_ready` is an artifact-derived posture, not evidence of live telemetry,
reachable on-call staff, exercised fallbacks/restores, incident-system mutation, or deployment.
See OPERATIONAL_READINESS_AUDIT.md.

## Security/privacy governance audit

`ApiClient.securityPrivacyAudit(...)` accepts `SecurityPrivacyManifestArgs` and returns
`SecurityPrivacyToolResult`. Types retain classification, retention/deletion, flow authorization,
MFA/least-privilege, threat treatment, independent review, control audits, counts, and stable
warning/blocking issue rows. `security_privacy_ready` is a declaration-derived posture, not proof
of live security controls, legal compliance, authentication, erasure, or executed red-team work.
See SECURITY_PRIVACY_AUDIT.md.

## Sandbox admission audit

`ApiClient.sandboxAdmissionAudit(...)` accepts `SandboxManifestArgs` and returns
`SandboxAdmissionToolResult`. Types retain content-addressed artifact lineage, hardened execution
profiles, network/mount boundaries, exact capability decisions, resource ceilings, quarantine and
reviewed output release across six independent audit row families. `sandbox_ready` is a
declaration-derived admission posture, not proof of code execution safety, kernel/runtime
enforcement, secret isolation, quarantine operation, or output publication. See
SANDBOX_ADMISSION_AUDIT.md.

## Sandbox runtime simulation

`ApiClient.sandboxRuntimeSimulate(...)` accepts `SandboxRuntimeManifestArgs` and returns
`SandboxRuntimeToolResult`. Types retain admission and trace digests, exact capability and target
matches, per-step resource validity and charges, cumulative usage, and explicit
`simulated`/`refused`/`not_run` decisions. `sandbox_runtime_ready` requires a valid admission and
a completely simulated trace; the method is still a policy simulation and does not start a host
process or claim kernel, namespace, cgroup, credential, secret, filesystem, or network
enforcement. See SANDBOX_RUNTIME_SIMULATION.md.

## Security, safety, and red-team program audit

`ApiClient.securityProgramAudit(...)` accepts `SecurityProgramManifestArgs` and returns
`SecurityProgramToolResult`. Types retain authorized scope, independent campaign evidence,
finding/remediation closure, incident timelines, disclosure sequencing, publication safety,
regression witnesses, counts, and seven independent audit row families. `security_program_ready`
is declaration-derived, not proof that scanners ran, containment occurred, disclosures were sent,
or controls are live. See SECURITY_PROGRAM_AUDIT.md.
