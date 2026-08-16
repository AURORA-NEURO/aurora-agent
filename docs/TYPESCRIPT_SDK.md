# TypeScript SDK

This document is the executable integration note for blueprint modules **11.06** (TypeScript SDK)
and **40.15** (TypeScript SDK contract). The package and its tests are intentionally kept beside
the Rust workspace so those two modules have a citable, reviewable implementation rather than a
foreign-artifact placeholder.

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

Convenience methods currently cover:

- `traceOtelIngest`: bounded OTLP JSON import with typed normalized events, source-to-IR mapping,
  semantic-loss categories, and compilation-readiness reporting;
- `qualityGateRun`: typed serialized Dataset/Gate/ReferenceSets with externally tagged check
  outcome and verdict unions. `Pass` keeps examined counts, `Fail` keeps a concrete witness,
  `NotRunnable` keeps missing-column/null-only/type/reference reasons, and `Failed` retains its
  separate `not_runnable` set; the client never promotes an indeterminate run to pass;
- `atlasReport`: typed capability coverage, measured-entry depth, hole/influence records, family
  and divergence histograms, coverage debt, internal inconsistencies, and eligible-versus-refused
  composite results with omission counts kept in the REST/MCP envelope;
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
- `developerDeliveryAudit`: developer-platform delivery evidence;
- `developerWorkbench`: digest-bound authoring/notebook audit, capability dashboard query, and
  review-only CI workflow planning;
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
`eventPersistence()` and `flushEventPersistence()` provide the event-cursor equivalent while
typing the explicit non-durability of webhook subscriptions and pending deliveries.

These helpers type the contract's top-level shape while leaving nested domain records as JSON
objects where the Rust crate is authoritative. That keeps the client useful across all domain
families without maintaining a fragile partial clone of 122 tool schemas. `capabilityDiscover`
searches the explicit cross-domain catalogue and returns typed `CapabilityDiscoverResult` matches
with domains, crates, CLI/Python artifacts, ranked fields, and optional authoritative schemas;
`capabilityAudit` returns typed `CapabilityAuditResult` parity counts, schema-quality totals,
invariant flags, duplicate memberships, and optional per-group coverage; `bioCapabilityEvidenceAudit`
returns typed `BioCapabilityEvidenceAuditResult` evidence rows, dimension rollups, claim blockers,
omission accounting, optional subaudits, and explicit release posture; `developerDeliveryAudit`
returns typed `DeveloperDeliveryAuditResult` readiness gates, explicit target blockers, release
request state, and foreign-surface posture; `bioAtlasPublicationAudit` returns typed
`BioAtlasPublicationAuditResult` atlas aggregation, score/evidence gates, leaderboard state, and
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
`benchmarkTraceAnalyze` returns typed `BenchmarkTraceAnalysisResult` evidence for causal
candidate scores, textual divergence, decision-boundary ranks, reversibility basis, goal-anchored
episodes, repeated-action progress, and reconciled summary counts. Causal verdicts remain distinct
from boundary ranking, environment-produced divergence is not assigned to an agent, and a
fail-closed refusal does not become a benchmark cell or replay claim.
`foundationContractCheck` returns typed `FoundationContractCheckResult` evidence with separate
contract, parent-refinement, applicability/maturity, world-class, and transition-plane gates. A
top-level `ok` transport result does not imply admission: callers must inspect the explicit
`verdict` and gate objects, and a refused world claim or plane confusion remains visible rather
than being flattened into a generic invalid-contract error.
`packCatalogue` returns typed `PackCatalogueResult` rows for the bounded benchmark portfolio,
including section counts, capability/domain signatures, oracle ceilings, execution-grounded
flags, release-wave declarations, omitted counts, and duplicate-signature review candidates. It
is a declaration inventory; it does not claim measured performance or a reportable health score.
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

## Events and webhooks

```typescript
const page = await api.events(0, 100);
const stream = await api.eventStream(page.page.next_after, 100);
for (const event of stream.events) {
  console.log(event.id, event.event, JSON.parse(event.data));
}

const subscription = await api.subscribe(
  "https://worker.example.invalid/prism-events",
  "a-long-operator-managed-secret",
  { subscriptionId: "worker-a", events: ["tool.completed", "tool.refused"] },
);
const deliveries = await api.deliveries(subscription.subscription.id);
// An operator-owned worker sends the signed envelope, then acknowledges only accepted ids.
await api.acknowledge(subscription.subscription.id, deliveries.page.deliveries.map((d) => d.delivery_id));
// Explicit operator recovery keeps delivery IDs stable and resets selected attempts.
await api.replay(subscription.subscription.id, deliveries.page.deliveries.map((d) => d.delivery_id));
```

The SSE route is a bounded snapshot, not a streaming connection. `eventStream` returns the raw
text, parsed events, content type, and next cursor so an application can decide whether to poll,
persist, or hand off to a real EventSource implementation. The webhook methods only manage the
server-side outbox. They do not send to endpoint URLs, retry on their own, or expose subscription
secrets.

## Compatibility posture

The API's `capabilities` response is the runtime compatibility anchor. Clients should check
`tool_count`, transport flags, and limits before enabling a workflow. REST and JSON-RPC calls share
the same in-process dispatcher, but gRPC, TLS termination, durable event storage, and an external
delivery worker remain deployment responsibilities. A client must not infer those features from
the presence of an HTTP listener.
