# Python SDK

This document covers blueprint modules **11.04** (Python SDK), **11.05** (Python benchmark
authoring SDK), **11.15** (evaluator/oracle/mutation SDK), and **11.16** (environment and pack
authoring SDK). It also documents the typed client for the metrics analytics bridge. The package
remains dependency-free and keeps Rust authoritative for canonical bytes, scientific invariants,
metric arithmetic, oracle decisions, and release gates.

The repository ships `python/prism_sdk`, a standard-library client for the Rust MCP server. It is
the integration layer above the deterministic kernel described by [ADR-001](ADR-001-language-strategy.md):
Python can orchestrate and author requests, while Rust remains the owner of canonical bytes,
domain invariants, release gates, and evidence semantics.

The artifact registry is available through typed `ArtifactRegistrationRequest`,
`ArtifactQueryRequest`, `ArtifactGetRequest`, `ArtifactRegistrationReport`, `ArtifactQueryReport`,
`ArtifactGetReport`, `ArtifactLineageReport`, and `ArtifactCrossStoreAuditReport` models. `ApiClient` exposes REST registration,
query, lookup, lineage, persistence status, and flush methods; `Workspace` and
`AsyncWorkspace` expose the MCP `artifact_registry_audit` flow. Parent presence remains an index
observation, missing parents remain explicit, and no client model treats a digest as scientific,
clinical, publication, causal, or external-effect authority.
`ApiClient.artifact_cross_store_audit()` and `Workspace.artifact_cross_store_audit()` compare
retained exact identities across the artifact, evidence, and reconciliation registries. They
return missing projections, orphaned projections, wrong-kind findings, store generations, and
checkpoint digests without implying an atomic cross-store transaction.
The Rust boundary also appends an `artifact_registry` projection to trusted mission, replay,
verified-bundle, and workflow-reconciliation responses. The projection is an audit lookup and may
report an explicit indexing or checkpoint failure; generic tool results are not auto-registered.

`DomainReportProjectRequest` and `DomainReportCoverageRequest` provide bounded Python models for
the explicit cross-domain report boundary. `ApiClient.domain_report_project()` and
`ApiClient.domain_report_coverage()` use the REST routes, while the corresponding `*_tool()`
methods exercise the same implementation through the REST/MCP dispatcher. `Workspace` and
`AsyncWorkspace` expose the typed MCP equivalents. Project reports retain the caller payload,
catalogue membership, claim posture, limitations, and exact artifact digest; coverage reports
enumerate missing capability groups. These models do not interpret report count or indexing as
execution, scientific, clinical, provenance, or release readiness.
`DomainEvidenceHarmonizeRequest`, `DomainEvidenceLink`, and
`DomainEvidenceHarmonizationReport` provide the next explicit join boundary. The sync and async
HTTP clients expose `domain_evidence_harmonize()` and `domain_evidence_harmonize_tool()`, while
`Workspace` and `AsyncWorkspace` expose the MCP helper and typed report projection. The request
requires exact same-subject reports and explicit support/qualification/contradiction/context
roles; the typed result preserves traceability state, contradiction posture, catalogue digest,
and artifact digest without treating report presence or a support link as a scientific or release
claim.
`DomainEvidenceIntakeRequest` and `DomainEvidenceIntakeReport` add the raw-envelope boundary for
every declared capability group. `ApiClient.domain_evidence_intake()` and its `*_tool()` variant,
plus the `Workspace` and `AsyncWorkspace` helpers, retain optional request JSON, required response
JSON, explicit observed/partial/refused/error/unknown outcome, separate request/response digests,
and the indexed `domain_evidence_intake` artifact. The client preserves request omission versus
supplied null and does not infer execution, provenance completeness, scientific validity, or
readiness from an intake record. `DomainEvidenceIntakeRequest.source_plan_digest` optionally binds
the envelope to a retained external-source plan; when present, the server verifies scope
compatibility before indexing and preserves the digest in the normalized intake and parent set.
`DomainEvidenceIntakeCoverageRequest` and `DomainEvidenceIntakeCoverageReport` expose the
catalogue-wide intake audit through REST, the REST/MCP dispatcher, `Workspace`, and
`AsyncWorkspace`. Coverage keeps missing groups, observed/partial/refused/error/unknown
outcomes, source tools, subjects, reported domains, declared-tool/domain gaps, optional intake
digests, and domain-level counts explicit; group, tool, and domain completeness remain separate,
and a reported envelope is not treated as executed capability or valid evidence.
`DomainEvidenceSourcePlanRequest` and `DomainEvidenceSourcePlanReport` expose the matching
external-source boundary through sync/async HTTP and workspace helpers. They retain connector and
locator classes, retrieval mode, expected content digest, parent links, and bounded policy while
keeping credentials caller-managed and retrieval `not_started`; a source plan is not provenance.
When `expected_content_digest` is supplied, a later bound intake is accepted only when the
canonical response digest matches it.
`DomainEvidenceSourceExecutionRequest` and `DomainEvidenceSourceExecutionReport` execute a
retained plan through the bounded file/plain-HTTP kernel, preserve observed/partial/refused/error
outcomes and raw-byte versus canonical-response digests, and automatically retain the result as
intake. The sync/async HTTP clients and `Workspace`/`AsyncWorkspace` expose both REST and MCP
helpers; unsupported connectors and unsafe transport policy remain explicit refusals.
Acquisition route transport rows also preserve `caller_managed_tools` separately from bounded
file/HTTP tools, so provider normalization availability is not misreported as executable network
transport.
`SourceAdapterProjectionRequest`, `SourceAdapterProjectionResult`, and
`project_source_execution()` form the local handoff from that returned envelope to a concrete
Python adapter. `ApiClient`, `AsyncApiClient`, `Workspace`, and `AsyncWorkspace` expose the same
`domain_evidence_source_project()` helper; it performs no second fetch, requires an explicit
adapter id, refuses omitted/binary/preview/truncated bodies, verifies the expected raw-byte
digest when supplied, and retains source-plan/response/raw digests in a separate transport
context rather than mislabeling them as parser provenance. A partial transport outcome remains
partial even when the nested format audit succeeds; invalid, lossy, blocked, and refused parser
states remain distinct.
`DomainEvidencePipelineRequest`, `DomainEvidencePipelineResult`, and
`project_domain_source_execution()` add the catalogue-bound variant. It requires the exact
catalogue digest, source-plan digest, group, domain, and adapter id; refuses incomplete or
truncated catalogue slices and cross-domain execution envelopes; and preserves the selected route
in the result. The four client facades expose `domain_evidence_source_project_for_domain()` as
the same local operation. This is a routing/conformance gate, not an ontology resolver or a
scientific authorization decision.
`DomainEvidenceProviderNormalizationRequest` and
`DomainEvidenceProviderNormalizationReport` cover caller-managed literature, clinical-trial,
FHIR, object-store, and provider-API payloads. `ApiClient`, `AsyncApiClient`, `Workspace`, and
`AsyncWorkspace` expose `domain_evidence_provider_normalize()`; the server preserves provider,
connector, payload, and optional request digests and feeds the result through the same indexed
intake/coverage path as bounded source reads. The typed `shape_audit` reports a structural status
(`structured`, `partial`, `refused`, or `unclassified`), the recognized connector container,
record/invalid-row counts, identifier field-presence coverage, and object-store content-digest
coverage where applicable. Its `shape_digest` is computed from those shape facts rather than
payload values; it never echoes identifiers or promotes field presence into evidence validity.
Provider authentication, signatures, retrieval, terminology expansion, and scientific/clinical
interpretation remain explicit non-claims.
`DomainEvidenceProviderReplayRequest` and
`DomainEvidenceProviderReplayVerificationReport` add the replay seam. The sync/async HTTP and
workspace facades recompute payload, request, shape, normalization-envelope, and intake digests,
return each match dimension, and retain a value-free replay artifact idempotently. A mismatch is
reported structurally; it is never converted into an observed provider result.

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
  when the caller supplies an id and target list. Its optional `ci_evidence` request is preserved
  as a separate structural check, and the explicit `ci_execution_evidence` target is available
  only when that evidence is complete, digest-bound, and passing.
- `bioatlas_publication_audit(atlas, ...)` preserves optional evidence, card, leaderboard, and
  weighting contracts and never implies publication without explicit targets.
- `BioAtlasPublicationAuditReport.from_wire(...)` plus the sync, async, and HTTP
  `bioatlas_publication_audit_report(...)` helpers type atlas aggregation, evidence-conditioned
  score gates, card/leaderboard availability, ranked/unranked counts, and explicit publication-target
  blockers. A ready target is contract eligibility, not publication, clinical authority, or network
  deployment.
- `HubLeaderboardRenderArgs` and `hub_leaderboard_render(...)` type rankable and unranked public
  entries across sync MCP, async MCP, and HTTP. Detailed projections preserve competition ranks,
  verification and disclosure labels, comparability differences, publication states, verification
  floors, evidence-scale refusals, and typed ineligible reasons; summary mode keeps detail omission
  explicit while retaining counts and the scoped non-clinical headline.
- `BioAtlasPublicationAuditArgs` and `bioatlas_publication_audit(...)` add a schema-bound composed
  projection for atlas coverage, evidence readiness, card attachment, leaderboard counts, and
  explicit release targets. `BioAtlasReleaseRequestReport` reconciles target blockers and
  fail-closed readiness; `BioAtlasCrossLayerReport` keeps numeric-score evidence requirements and
  withheld-score semantics visible. The audit is not a publisher, identity service, assay runner,
  leakage detector, or scientific/clinical approval.
- `HubSubmissionReviewArgs` and `hub_submission_review(...)` type the public-hub acceptance and
  moderation replay across sync MCP, async MCP, and HTTP. `HubModerationEventReport` preserves
  opened/transition/attestation events, actors, monotonic epochs, reasons, supersession, and
  verification movement; records retain history and withdrawal tombstones; refusal stages remain
  fail-closed. The facade does not authenticate identities, persist a ledger, or publish a page.
- `BioCapabilityEvidenceAuditRequest`, `EvidenceItem`, and `ClaimRequest` provide a bounded typed
  input for `Workspace.biocapability_evidence_audit(...)` and its async/HTTP counterparts. They
  enumerate the nine evidence dimensions, keep observed/reproduced support distinct from declared
  status, validate claim prerequisites and duplicate IDs, and preserve optional information-value,
  reference, temporal, and reproducibility subaudits without deciding scientific truth locally.
- `BioCapabilityEvidenceAuditReport.from_wire(...)` plus the sync, async, and HTTP
  `biocapability_evidence_audit_report(...)` helpers type the returned evidence inventory, dimension
  rollups, domain counts, claim blockers/assumptions, omission bounds, optional subaudits, and
  fail-closed release posture. `report.ready_for_requested_claims` remains evidence of an internally
  complete contract, never a scientific or clinical truth claim.
- `BioQlCompileRequest` and `Workspace.bioql_compile(...)` provide the same bounded entry point
  over sync MCP, async MCP, and HTTP. Query text and explicit schema JSON are size-checked locally;
  lexical, unit/frame, temporal, provenance, access-label, and cost semantics remain authoritative
  in Rust, and compilation never executes a query.
- `TokenContextPlanArgs` and `token_context_plan_report(...)` type token-context planning across
  sync MCP, async MCP, and HTTP. `TokenContextRequest`, `TokenPlanCandidate`, and `TokenEstimate`
  preserve pinned compiler identity, resolution depth, node kind, restricted-data flags, and the
  estimation ruler. `TokenContextPlanningReport` exposes mandatory closure affordability, dry-run
  handles, optional estimates, and policy-only comparison deltas; it never treats a caller-declared
  or mixed estimate as a tokenizer measurement, and it refuses mismatched candidate identity at
  the client boundary before Rust performs the authoritative plan.
- `WeaveLangCompileArgs` and `weavelang_compile_report(...)` expose deterministic WeaveLang
  compilation, whole/semantic program digests, IR disclosure, and explicit replay/live execution
  through sync MCP, async MCP, and HTTP. `WeaveLangExecutionReport` preserves not-requested,
  completed, and fail-closed refused states, local liveness holes, invariant violations, event
  counts, and optional trace digests without presenting replay as production execution or liveness
  inspection as a universal proof.
- `EpistemicVoiArgs` and `epistemic_voi_report(...)` expose explicit decision-relative information
  pricing through sync MCP, async MCP, and HTTP. `EpistemicDecisionProblemArgs`,
  `EpistemicBeliefArgs`, `EpistemicAcquisitionArgs`, and `EpistemicOutcomeArgs` keep actions,
  models, row-major losses, normalizable beliefs, assay costs, and per-model likelihood partitions
  explicit. `EpistemicValueReport` keeps gross risk reduction, declared cost, net value, outcome
  probabilities, and action identities separate; `EpistemicComplementarityReport` exposes the
  joint-versus-singleton excess for explicitly non-adaptive bundles. Improper partitions and
  exhaustive-cap failures remain typed fail-closed refusals rather than being rounded into a
  negative result or silently treated as an adaptive policy.
- `EpistemicContextAuditArgs` / `epistemic_context_audit_report(...)` audit observed-evidence
  compression with typed `EpistemicEvidenceItemArgs` and `EpistemicEvidencePoolArgs`. The report
  keeps decision identification, minimal sufficient context, exhaustive rate–distortion frontier,
  and requested subset rows separate; minimax abstention, contradictory subsets, and enumeration
  caps remain fail-closed. See [`docs/EPISTEMIC_CONTEXT_AUDIT.md`](EPISTEMIC_CONTEXT_AUDIT.md).
- `EpistemicSelectionAuditArgs` / `epistemic_selection_audit_report(...)` expose bounded observed-
  evidence selection with typed cardinality/budget constraints and protected closure. The report
  keeps plain/lazy greedy choices, exhaustive submodularity status, guarantee applicability, and
  exact small-instance comparison separate; a selection above an audit cap remains useful but does
  not receive an inferred factor or optimality claim. See
  [`docs/EPISTEMIC_SELECTION_AUDIT.md`](EPISTEMIC_SELECTION_AUDIT.md).
- `BenchmarkTraceAnalyzeArgs` and `benchmark_trace_analysis_report(...)` expose the benchmark
  compiler's causal, divergence, boundary, episode, and repetition layers through sync MCP, async
  MCP, and HTTP. `BenchmarkTraceEventArgs` preserves event kind, payload, causal parent, and
  visibility; `BenchmarkCausalScoreReport` keeps necessity, observed reference effect,
  irreversibility, and explanatory simplicity distinct from `BenchmarkCandidateScoreReport`'s
  boundary-ranking arithmetic. Typed verdicts preserve first-causal, conjunction,
  environment-divergence, no-divergence, and unlocalizable outcomes, while structured refusals
  remain fail-closed and never become fabricated blame or executable benchmark cells.
- `BenchmarkDecisionAuditArgs` and `benchmark_decision_audit_report(...)` narrow that evidence to
  one selected choice/action step and expose the reconstructed action set, hindsight-firewall
  provenance, visible-versus-validation-only coverage, causal alignment, and failure-card
  evidence ratio. Candidate actions, constraint ledgers, and claims remain caller-supplied typed
  JSON projections; future-derived actions cannot become agent-visible, uncited claims remain
  hypotheses, and bounded omission counts survive every sync/async MCP, HTTP, and workspace
  facade.
- `BenchmarkIntegrityAuditArgs` and `benchmark_integrity_audit_report(...)` expose portfolio-level
  deduplication, deterministic holdouts, declared contamination probes, calibration denominators,
  safety-veto labels, and effective diversity through the same facades. The typed report keeps
  admissible-clean counts separate from unassessed or leaked instances, raw volume separate from
  effective sample size, and every bounded row projection's omission count visible.
- `BenchmarkCounterfactualCheckArgs` and `benchmark_counterfactual_check_report(...)` validate
  one-factor `DecisionCell` pairs and grade invariant/must-change contrast outcomes. The report
  preserves changed fields, source/follow-up digests, the explicit no-realism-review limitation,
  and fail-closed unmatched-pair refusals through every sync/async MCP, HTTP, and workspace
  facade.
- `BenchmarkOracleReviewArgs` and `benchmark_oracle_review_report(...)` preserve the compiler's
  proposal-to-reviewed-oracle type gate. The report exposes named reviewer identity, review
  digest, synthesis strength, deterministic status, optional set-valued acceptance grading, and
  optional reviewed `DecisionCell` packaging. Exploits, missing blind spots, weak-oracle-alone
  proposals, empty acceptance sets, and unattributed review remain fail-closed across every
  sync/async MCP, HTTP, and workspace facade; serialized reviewed JSON is never treated as a
  trusted replacement for the kernel gate.
- `BenchmarkCompileArgs` and `benchmark_compile_report(...)` expose the assembled causal,
  hierarchical-minimization, oracle-synthesis, confidence, and provenance pipeline. The typed
  request keeps the caller-supplied `InterestSignature` observation table explicit and bounded;
  the report refuses when any subset requested by minimization is missing instead of interpolating
  a preservation proof. It returns an unreviewed proposal only, with fail-closed causal,
  nondeterministic-probe, budget, and property-loss outcomes preserved across all facades.
- `BenchmarkCompileReviewArgs` and `benchmark_compile_review_report(...)` close that path into a
  single review-gated authoring call. They require reviewer identity plus world/query `InputRef`
  values, preserve optional four-way grading, and only return a packaged cell after the real
  `Compilation::approve` gate succeeds; compiler and oracle refusals remain distinct.
- `FoundationContractCheckArgs` and `foundation_contract_check_report(...)` type the foundation
  declaration gate across sync MCP, async MCP, and HTTP. `FoundationContractGateReport`,
  `FoundationParentRelationReport`, `FoundationEnvelopeReport`, `FoundationWorldReport`, and
  `FoundationTransitionReport` keep admissibility, inheritance, applicability/maturity,
  counterfactual authority/reveal policy, and transition-plane consistency independent. The
  aggregate `admitted` property is conservative: a valid contract can still be refused by an
  optional gate, and no declaration is promoted to evidence, treatment authority, or execution.
- `PackCatalogueArgs` and `pack_catalogue_report(...)` expose the bounded section-15/section-29
  portfolio inventory across sync MCP, async MCP, and HTTP. `PackCatalogueEntryReport` keeps
  construct, capability/domain axes, decision families, oracle ceilings, execution-grounded
  posture, release-wave sequencing, and capability signatures visible; duplicate signatures are
  returned as review candidates. The report is explicitly declaration-only and never substitutes
  for observed pack calibration or the separate health/reportability gate.
- `PackCoverageAuditArgs` and `pack_coverage_audit_report(...)` project the real pack-coverage and
  capability-family matrix kernels across sync MCP, async MCP, and HTTP. `PackCoverageAuditReport`
  preserves selected-pack identity, covered/uncovered/singly-covered/weakly-covered families,
  bounded rows and matrix cells, omission counts, and the kernel's gap summary. Unknown packs,
  empty intersections, and malformed requests remain fail-closed; the projection is still
  declaration-level coverage and does not claim measured instance performance or health. See
  [`docs/PACK_COVERAGE_AUDIT.md`](PACK_COVERAGE_AUDIT.md) for the wire contract and interpretation
  rules.
- `PackReleaseAuditArgs` and `pack_release_audit_report(...)` expose the real stable portfolio
  release order and explicitly unsequenced remainder across sync MCP, async MCP, and HTTP. The
  typed report keeps selected versus global positions, wave/axis counts, omission reconciliation,
  section-incompatible selection refusals, and the non-approval limitations visible. See
  [`docs/PACK_RELEASE_AUDIT.md`](PACK_RELEASE_AUDIT.md) for the interpretation contract.
- `PackHealthAssessArgs` and `pack_health_assessment_report(...)` expose the observed pack-health
  gate across sync MCP, async MCP, and HTTP. `PackCalibrationReport` retains system-level pass and
  trial denominators; `PackDiscriminationReport` distinguishes undetermined, saturated, floored,
  and genuinely discriminating observations; and `PackHealthFindingReport` types degeneracy,
  contamination signals, grounded-oracle absence, and instance materialization separately. The
  report carries the immutable pack digest through `PackHealthReport` and `PackScoreReport`, while
  `PackScoreGateReport` withholds numeric evidence on blocking findings. Rust validation refusals
  remain structured, fail-closed `PackHealthAssessmentReport` values rather than becoming a
  fabricated zero or an uninspectable generic exception.
- `WorldClaimCheckRequest`, `LabPlanRequest`, and `RoutingDecisionRequest` expose typed envelope
  helpers for world support checks, no-execution acquisition planning, and unseen-task routing over
  sync MCP, async MCP, and HTTP. They bound serialized maps, action/evidence counts, budgets, and
  task identity while leaving provenance support, privacy crossings, reachability, abstention, and
  safe-default decisions to Rust.
- `world_claim_check_report(...)` adds a typed epistemic boundary over that request: observed,
  semi-synthetic, and mechanistic rungs, selection declarations, assumptions, unsupported
  counterfactuals, grounded claim evidence, caveats, and fail-closed refusal strings remain
  distinct. Sync MCP preserves the kernel's `ok: false` structured refusal instead of converting
  it into an uninspectable generic transport exception.
- `ObservedWorldDeclareArgs` and `observed_world_declare_report(...)` seal the preceding
  declaration boundary: source versions/access policies, controlled-source names, cohort/stratum
  reconciliation, selection, unsupported counterfactuals, outcome labels, and observed-only
  provenance are all reconciled before callers treat the declaration as a usable artifact.
- `LineageAuditArgs` and `lineage_audit_report(...)` preserve the specimen registry audit as two
  independent gates: typed ancestry/material/artifact findings and a three-state identity ledger
  where missing fingerprints never count as consistent. Bounded fingerprint/finding omissions,
  donor mismatch evidence, and `ready_for_identity_claim` remain explicit.
- `PreanalyticApplyArgs` and `preanalytic_apply_report(...)` carry admitted fault records, signed QC
  deltas, measurability loss, biology-before/after digests, family false-positive validation,
  response availability, and optional detectability floors. Structured mutation refusals remain
  fail-closed values rather than transport errors.
- `ContradictionReviewArgs` and `contradiction_review_report(...)` expose the complete multimodal
  review program: pose/validation/examination refusals, admissible hypotheses, discriminating
  actions, cue scans, expectedness evidence, and the distinct resolved, not-yet-examined, and
  unresolvable states. No helper selects a winning modality.
- `lab_plan_report(...)` types inference-lab acquisition ordering, privacy and budget exclusions,
  optional hypothesis separation, declared spend, stop reason, and the explicit `should_escalate`
  predicate. Planning remains visibly non-executing.
- `LabParetoAuditArgs` / `lab_pareto_audit_report(...)` expose the multi-objective inference-lab
  archive with typed objective directions, measured versus unmeasured profile axes, dominated
  and displaced candidates, front-only relations, unresolved holes, and ambiguous selection.
  The parser rejects partial-front refusals presented as success and reconciles every bounded row
  omission. See [`docs/LAB_PARETO_AUDIT.md`](LAB_PARETO_AUDIT.md).
- `LabBranchAuditArgs` / `lab_branch_audit_report(...)` expose ordered risk-triggered branching
  with typed policy-validation refusals, undetermined-risk escalation, branch/verifier spend,
  catches, wasted escalations, escaped harms, and the complete decision denominator. A catch's
  single-path counterfactual remains explicit; the parser does not treat a verifier invocation as
  a success claim. See [`docs/LAB_BRANCH_AUDIT.md`](LAB_BRANCH_AUDIT.md).
- `LabHoldoutAuditArgs` / `lab_holdout_audit_report(...)` expose validated architecture bundles,
  append-only holdout exposure, clean point measurements, typed contamination refusals,
  checkpoints, rollback receipts, retained exposure, and current certification budget. Measurement
  refusals remain successful audit evidence but never become clean scores; structural failures
  remain fail-closed. See [`docs/LAB_HOLDOUT_AUDIT.md`](LAB_HOLDOUT_AUDIT.md).
- `LabEvolutionAuditArgs` / `lab_evolution_audit_report(...)` form the claim boundary over the
  inference lab: only kernel-minted clean before/after measurements can produce an
  `improvement_claimed` evolution card. Contaminated attempts remain serialized contaminated
  cards, non-improvements remain `claim_refused`, and architecture/completeness/card failures
  remain fail-closed with stages and omission-aware measurement rows. See
  [`docs/LAB_EVOLUTION_AUDIT.md`](LAB_EVOLUTION_AUDIT.md).
- `LabSpaceAuditArgs` / `lab_space_audit_report(...)` validate immutable architecture bundles,
  parent lineage, required component kinds, graph integrity, protected surfaces, and cost before
  holdout work begins. The typed report separately reconciles candidate, inspection, and component
  diff rows; failed registration never becomes a usable partial space. See
  [`docs/LAB_SPACE_AUDIT.md`](LAB_SPACE_AUDIT.md).
- `OncoBoundaryArgs` and `onco_boundary_report(...)` preserve oncology's partial-release contract:
  aggregate research uses can remain released while individual clinical uses are refused and
  escalated, and direct-identifier refusals remain fail-closed without echoing request data. The
  versioned report adds reconciled disposition/count projections, escalation trigger/route, and
  explicit identifier-presence state. See `docs/ONCO_BOUNDARY_CHECK.md`.
- `OncoResponseAssessArgs`/`onco_response_report(...)` expose the criteria-aware response gate:
  the raw radiologic reading, reportable call, withheld-progression flag, surviving hypothesis
  count, and discriminating evidence requests remain separate. The typed report also carries call
  kind, criterion/treatment metadata, criterion divergence, sensitivity, and identifiability.
  A structured refusal is retained
  as a fail-closed domain value, and the parser rejects a forged `withheld_progression` value that
  does not carry the kernel's `not evaluable` call label. See `docs/ONCO_RESPONSE_ASSESS.md`.
- `OncoWorldlineViewArgs`/`onco_worldline_report(...)` reconcile biological acquisition order,
  record order, indexed four-clock timepoint rows, and the optional agent-visibility partition.
  `OncoWorldlineReport.timepoint_records` exposes typed clock and visibility projections; a cutoff
  is never inferred from a record timestamp, and visible/hidden partitions are required only when
  the caller explicitly requested filtering. See `docs/ONCO_WORLDLINE_VIEW.md`.
- `OncoClassificationArgs`/`onco_classification_report(...)` preserve the five tagged resolution
  states, typed panel observations, satisfied evidence, and prioritized assay obligations. The
  client refuses contradictory variants, forged panel/obligation counts, and uncollected assays
  represented as negatives. See `docs/ONCO_CLASSIFICATION_CHECK.md`.
- `OncoIdentityJoinArgs`/`onco_identity_join_report(...)` expose participant/lesion/specimen/
  imaging-series joins with optional identity evidence and epoch bridges. `joinable=False` is a
  successful, auditable domain verdict—not a transport exception—and `bridge_declared` remains
  explicit. `OncoOutcomeAnalyzeArgs`/`onco_outcome_report(...)` carry the predeclared estimand,
  typed endpoint strategy, at-risk days, immortal-time exposure, event/censoring split, and
  informative-bias flags; `analysis_record` retains the estimand-bound nested outcome and the
  parser rejects event/censoring, bias-count, and informative-censoring contradictions. See
  `docs/ONCO_OUTCOME_ANALYZE.md`.
- `OncoWorldsModelTransportArgs`/`oncoworlds_model_transport_report(...)` preserve model-system
  fidelity, establishment, declared sample size, transport assumptions, and the distinction
  between a supported patient-relevant research claim and a typed fail-closed transport refusal.
  The report exposes model identity, passage-specific fidelity, selection state, replication
  accounting, refusal kind, and a nested supported-claim projection. See
  `docs/ONCOWORLDS_MODEL_TRANSPORT.md`.
  `OncoWorldsMethylationClassifyArgs`/`oncoworlds_methylation_classify_report(...)` retain QC,
  calibration, threshold, and tumour-content caveats while treating an abstention as an explicit
  unclassifiable result, with typed classifier, threshold, score-coverage, outcome, and nearest
  evidence projections. `OncoWorldsMethylationCompareArgs`/
  `oncoworlds_methylation_compare_report(...)` keep classifier-version disagreement separate from
  agreement and from the case where both sides are unclassifiable, with typed divergence and
  classifier-change accounting. See `docs/ONCOWORLDS_METHYLATION.md`.
- `OncoWorldsClonalHistoryCheckArgs`/`oncoworlds_clonal_history_check_report(...)` preserve typed
  compatible histories, per-candidate refusal kinds, candidate accounting, and unique versus
  ambiguous history status. See `docs/ONCOWORLDS_CLONAL_HISTORY_CHECK.md`.
- `OncoClonalEvidenceCheckArgs`/`oncoworlds_clonal_evidence_check_report(...)` preserve asymmetric
  specimen-to-tumour promotion, set-valued recurrence explanations, declared sensitivity and
  copy-number provenance, and temporal treatment-causation refusals. See
  `docs/ONCOWORLDS_CLONAL_EVIDENCE.md`.
- `OncoWorldsEraShiftCheckArgs`/`oncoworlds_era_shift_check_report(...)` type classification-era
  mappings, site assay availability, not-collected evidence, administrative descriptor use, and
  cross-version comparability refusals. `OncoWorldsEquityCheckArgs`/
  `oncoworlds_equity_check_report(...)` require complete subgroup intervals before releasing an
  equity projection and preserve pooled-only, empty, and unquantified refusals. See
  `docs/ONCOWORLDS_SHIFT_EQUITY.md`.
- `OncoWorldsEntityWorldCheckArgs`/`oncoworlds_entity_world_check_report(...)` compose provenance,
  alteration-mechanism, rare-class benchmark, lesion-clustering, and competing-event safeguards.
  Each requested section keeps its own admissibility/refusal result and the top-level counts are
  reconciled from those sections. See `docs/ONCOWORLDS_ENTITY_WORLDS.md`.
- `OncoIdentityJoinArgs`/`onco_identity_join_report(...)` preserve the tagged join decision,
  refusal kind, identity-link count, epoch bridge warrant, and ordered checked dimensions. A
  declined join remains a typed domain result. See `docs/ONCOWORLDS_IDENTITY_JOIN.md`.
- `OncoWorldsRadiogenomicCheckArgs`/`oncoworlds_radiogenomic_check_report(...)` preserve split,
  feature-fitting, target-scope, mechanism-stratum, and transport assumptions before admitting a
  cross-modal claim. The typed report retains the versioned support/refusal state, blocked claim
  sentence, design projection, refusal kind, and nested supported claim. See
  `docs/ONCOWORLDS_RADIOGENOMIC_CHECK.md`. `OncoWorldsClonalHistoryCheckArgs`/
  `oncoworlds_clonal_history_check_report(...)` reconcile candidate histories against cellular
  fractions, preserve per-candidate typed rejection reasons, and represent multiple compatible
  histories as ambiguity rather than selecting one. The OncoWorlds workflows are available on
  sync/async MCP and HTTP facades, with matching TypeScript methods alongside the earlier oncology
  projections.
- `FiberCompileRequest`, `FiberRefineRequest`, `FiberExplainRequest`, `FiberVerifyRequest`, and
  `ProjectionBundleRequest` make the full FIBER progressive-disclosure lifecycle typed across sync
  MCP, async MCP, and HTTP. `Workspace.fiber_compile(...)` validates relative world/query paths and
  l0--l4 layers; `fiber_refine(...)` requires either a bounded content-addressed handle or an
  explicit world/query pair; `fiber_explain(...)` exposes the plan and omission rationale;
  `fiber_verify(...)` checks a certificate before trust; and `projection_bundle(...)` keeps complete
  view bodies opt-in. Rust remains authoritative for compilation, sufficiency, omission accounting,
  certificate verification, and projection fidelity.
- `compile_context(world, query, ...)` remains the compatibility helper for the lower-level mapping
  form and its policy/profile choices.
- `RepositoryCatalogRequest`, `RepositoryBundleRequest`, and `RepositoryImpactRequest` provide
  bounded repository knowledge navigation across sync MCP, async MCP, and HTTP. The helpers expose
  prefix/limit discovery, normative or exhaustive route traversal, denied-label and edge-vocabulary
  controls, opt-in markdown ceilings, and conservative changed-module impact checks without
  dumping repository bodies or treating graph impact as a semantic diff.
- `TelemetryProjectRequest` exposes the operations observability boundary across the same transports.
  It couples optional metric definitions to observations, requires an explicit redaction policy and
  trace id, and leaves unclassified emission, missing treatments, asserted-only evidence, and zero
  denominators as Rust-owned refusals rather than locally inventing telemetry truth.
  `telemetry_project_report(...)` adds a typed projection report: canonical record metadata,
  field-level dropped/coarsened loss, supported metric inputs, asserted-only metric refusals, and
  the explicit no-OTLP/no-network limitation remain inspectable through sync/async MCP and HTTP.
- `LedgerIngestArgs`, `LedgerTemporalCut`, and `ledger_ingest_report(...)` expose bounded append
  receipts, recorded/duplicate/quarantined admission variants, causal releases, chain and clock
  witnesses, temporal-cut rows, and digest-only subject projections across sync/async MCP and HTTP.
  Payload bodies remain caller-owned and the report does not imply durable storage or a server clock.
- `trace_otel_ingest(trace_id, otlp_json=... | document=..., ...)` invokes the bounded OTLP JSON
  importer and preserves its semantic-loss/readiness report. `TraceOtelIngestArgs` and
  `trace_otel_ingest_report(...)` additionally type the normalized Event IR preview, source-to-IR
  mapping counts, every loss category, and the explicit no-export/no-clock limitation across
  sync/async MCP and HTTP.
- `QualityGateRunArgs` and `quality_gate_run_report(...)` expose the serialized in-memory quality
  gate across sync/async MCP and HTTP. The typed report keeps examined-value counts, concrete
  row/column witnesses, all four not-runnable reasons, check-level outcomes, and the three-way
  passed/failed/indeterminate verdict with failed and obstructed check names separate. A Python
  `passed` value is never a score and never includes an indeterminate run.
- `AtlasReportArgs` and `atlas_report_typed(...)` expose `atlas_report` across sync/async MCP and
  HTTP. The typed projection keeps measured scores attached to depth/effective evidence,
  measured-versus-unmeasured holes, claim-blocking influence, dark families, coverage debt,
  failure inconsistencies, bounded histograms, and eligible-versus-refused composites. Omitted
  rows reconcile to authoritative totals, and no hole is converted into a numeric zero.
- `AtlasSurfaceAuditArgs` and `atlas_surface_audit_report(...)` expose the atlasx publication
  surface across sync/async MCP and HTTP. The typed projection keeps CapabilityGrid denominators,
  named holes, measured-versus-declared debt discharge, withheld FailureRecord buckets, explicit
  denominator-safe rate checks, and surface soundness separate. It preserves canonical
  hyphenated publication states and fail-closed coverage/visibility/soundness policies. See
  [ATLAS_SURFACE_AUDIT.md](ATLAS_SURFACE_AUDIT.md).
- `AdaptivePanelRunArgs` and `adaptive_panel_report(...)` expose the serialized adaptive panel
  across sync/async MCP and HTTP. The report keeps raw/scored/abstained audit totals, parent-aware
  coverage shortfalls, stopping reasons, naive and clustered intervals, inflation, optional
  bootstrap evidence, withheld estimates, deterministic selection records, capability views,
  comparison refusals, and finished-state refusals. It never treats dispatch or a withheld estimate
  as evidence.
- `PosteriorGateArgs` and `posterior_gate_report(...)` expose the clustered capability posterior,
  rationale-bearing release scalar, and capability-wise dominance comparison across sync/async
  MCP and HTTP. The typed projection preserves pass/outcome/credit axes, ICC and effective sample,
  provenance gaps, vetoes, disputes, weak-evidence counters, gate terms and sensitivity, and
  fail-closed `credit_policy`, `posterior`, and `comparison_posterior` refusals. See
  [`docs/POSTERIOR_GATE.md`](POSTERIOR_GATE.md) for the full contract.
- `developer_workbench(session, dashboard=..., ci=...)` composes the Rust authoring-session and
  notebook audit with optional hole-preserving capability queries and review-only GitHub Actions
  planning. The facade validates only the outer mappings; Rust validates digests, dependencies,
  evidence posture, release readiness, safe paths, and deterministic YAML.
- `ci_execution_evidence_audit(...)` regenerates that canonical CI plan and reconciles a run report
  against its digest and exact checks. `CiExecutionEvidenceReport` preserves per-check result
  digests, missing/unknown/duplicate/non-passing findings, provider provenance, structural-only
  verification, and the bounded `ci_evidence_ready` handoff. It never executes or authenticates a
  provider run; see [`docs/CI_EVIDENCE.md`](CI_EVIDENCE.md).
- `CiProviderNormalizationRequest` and `ci_provider_normalize(...)` convert a bounded GitHub
  Actions-shaped, GitLab CI, or generic provider payload into the canonical evidence envelope consumed by the
  audit. `CiProviderNormalizationReport` preserves provider/source, payload digest, derived-digest
  warnings, and normalized evidence across `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`; it does not contact or authenticate a provider.
- `CiProviderEvidenceRequest` and `ci_provider_evidence_audit(...)` add bounded artifact, log, and
  attestation rows to that provider handoff. `CiProviderEvidenceReport` preserves row families,
  record digests, binding findings, canonical nested evidence, and the structural-only
  `conformance_ready` signal across the same four facades. The SDK validates mapping/array bounds
  locally; Rust remains authoritative for digest syntax, identity binding, and attestation subject
  semantics.
- `developer_delivery_audit(..., ci_provider_evidence=...)` carries that report as an independent
  fail-closed release target. `DeveloperDeliveryAuditReport` preserves the target readiness and
  provider-evidence projection, while `DeveloperDeliveryReceiptReport`/verification report expose
  the content-addressed evidence row and tamper findings.
- `execution_provenance_audit(...)` reconciles an `agent_mission` report with terminal results,
  contiguous trace identity, and optional delegated checks. `ExecutionProvenanceReport` preserves
  content digests, missingness, non-passing checks, and the structural-only `provenance_ready`
  handoff across `Workspace`, `AsyncWorkspace`, and the HTTP clients; it never replays the mission.
- `developer_delivery_audit(..., ci_evidence=..., execution_provenance=...)` keeps CI evidence and
  mission provenance as separate, opt-in release targets (`ci_execution_evidence` and
  `execution_provenance`). Each target is fail-closed when its evidence is absent or not ready;
  providing either signal does not imply provider execution, deployment approval, or release authority.
- `developer_delivery_audit(..., ci_provider=...)` composes `CiProviderNormalizationRequest` into
  that same CI target end to end: the server normalizes the provider payload, audits the resulting
  evidence against the regenerated plan, and returns both `ci_provider_normalization` and
  `ci_evidence`. `ci_provider` and `ci_evidence` are mutually exclusive, and neither implies
  provider authentication or external execution.
- `developer_delivery_audit(..., ci_provider_evidence=...)` composes the complete provider
  artifact/log/attestation report into the independent `ci_provider_evidence` target; it remains
  mutually exclusive with both other CI evidence inputs and keeps canonical run evidence separate.
- `DeveloperDeliveryReceiptRequest` and `developer_delivery_receipt(...)` recompute that audit and
  return `DeveloperDeliveryReceiptReport` with canonical target rows, evidence presence/readiness,
  target/delivery/receipt digests, and structural receipt findings across `Workspace`,
  `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`. The receipt is a joinable handoff, not a
  signature, durable record, execution proof, or release approval.
- `DeveloperDeliveryReceiptVerificationRequest` and `developer_delivery_receipt_verify(...)`
  recompute a stored receipt against its completed delivery audit and preserve independent digest,
  target, evidence, and readiness mismatch findings. A verified result establishes structural
  consistency of supplied records only; it does not authenticate a provider or authorize release.
- `developer_platform_status(...)` exposes the in-repository developer-platform contract through
  sync MCP, async MCP, and HTTP. `DeveloperPlatformStatusReport` types the module classification
  ledger, every walkthrough's checkable/partly-outside/entirely-outside standing, cookbook
  verification and omission counts, declared developer-contract surfaces, diagnostic findings,
  exit-code divergences, foreign-artifact posture, and optional full-detail evidence. The report
  reconciles all bounded counts and provides `platform_checks_clean`, `claims_guarded`,
  `foreign_artifacts_present`, and `complete_summary` properties; it does not turn a clean local
  check into proof that foreign SDK, CI, gRPC, or live-debugger surfaces were executed.
- `developer_delivery_audit(...)` composes platform, repository, SDK, conformance, provider,
  governance, release, and optional CI execution evidence without executing publication or CI.
  `DeveloperDeliveryAuditReport`
  plus `Workspace.developer_delivery_audit_report(...)`, `AsyncWorkspace.developer_delivery_audit_report(...)`,
  and the HTTP counterparts expose typed readiness gates, explicit target blockers, fail-closed
  release-request state, foreign-artifact posture, and preserved check evidence. A report is release
  evidence only when a caller supplied an explicit target request; it never creates implicit approval.
- `agent_mission(mission_id, goal, steps, policy=...)` previews or executes a cross-domain tool DAG.
  `MissionStep` and `MissionPolicy` preserve domain labels, dependencies, explicit execution
  allow-lists, side-effect posture, refusal propagation, and output budgets; the server remains the
  authority for ordering and execution. Set `execution_mode="parallel_waves"` for bounded concurrent
  dispatch of independent steps in each deterministic wave (`max_parallelism` is capped at 16);
  serial execution is the default and the executor reserves the worst-case wave output budget before
  launching work. `MissionBinding` can route a JSON-pointer field from a
  successful direct prerequisite into an existing argument slot.
  A `MissionRequest` may also carry the `workflow_binding` returned inside a domain-workflow
  instantiation. The typed `MissionExecutionReport.workflow_reconciliation` property exposes the
  compact automatic reconciliation link after execution; the full digest-bound record remains
  available through the domain-workflow reconciliation helpers.
- `mission_preflight(request, catalogue=...)` adds a no-side-effect client review before mission
  dispatch. It returns a request digest, live-catalogue digest, deterministic waves, per-step
  schema reports, binding/dependency findings, execution authorization issues, and explicit
  limitations. It never turns a plan into a domain result; `agent_mission(...)` remains the Rust
  authority for execution, refusal propagation, and output accounting. Executed reports include a
  clock-free `execution_trace`; `MissionExecutionReport.from_wire()` validates its contiguous
  lifecycle sequence while preserving the raw report.
- `capability_discover(...)` searches the complete domain catalogue by intent, domain, group, or
  tool and can attach authoritative MCP schemas for the returned routing matches.
- `mission_evaluator_discover(...)` searches the explicit non-executing evaluator catalogue across
  every capability group. `MissionEvaluatorQuery` bounds intent, group, domain, mission level,
  adapter ID, and result count; `MissionEvaluatorSearchReport` preserves the catalogue digest,
  purpose, candidate tools, RFC 6901 pointer examples, and `candidate_only` selection posture.
  `mission_evaluator_discover_report(...)` accepts either direct MCP data or an HTTP REST envelope,
  and sync/async Workspace and HTTP clients expose the same typed helper. Discovery is routing
  evidence only: it does not execute an adapter or make a claim semantic, calibrated, clinical, or
  release-ready.
- `mission_evaluator_review(...)` validates a caller's discovery-bound selection rows without
  executing tools. `MissionEvaluatorReviewRequest` normalizes bounded claim, adapter, domain, step,
  and RFC 6901 pointer fields; `MissionEvaluatorReviewReport` exposes ready/blocked status, findings,
  candidate membership, domain support, proposed binding rows, catalog/discovery digests, and the
  explicit `execution: "not_started"` posture across sync and async Workspace/HTTP clients.
- `mission_evaluator_replay(...)` audits a retained `agent_mission` report without dispatching an
  evaluator. `MissionEvaluatorReplayRequest` bounds the mission payload, fixture inclusion, and row
  count; `MissionEvaluatorReplayReport` preserves recomputed outcome/digest counts, adapter/domain
  matches, disagreement and omission findings, represented/missing catalogue groups, and structural
  retained/refused/omitted/disagreement fixtures for every standard adapter. `ready` only means the
  structural audit found no findings; it is not semantic, scientific, clinical, causal, or release
  validation.
- `ApiClient.mission_evaluator_replay_query(...)` and its async counterpart query the durable
  mission checkpoint by ID. `MissionEvaluatorReplayQueryRequest` bounds `include_fixtures` and
  `max_items`; `MissionEvaluatorReplayQueryReport.summary_only` distinguishes a full retained
  replay from the compact digest/count/coverage summary preserved after large-result omission.
  Summary-only evidence never reconstructs raw output or executes an evaluator, and the typed
  report keeps retention metadata, links, guarantees, and limitations intact.
- `MissionEvaluatorReplayCompareRequest` and `mission_evaluator_replay_comparison_report(...)`
  expose the non-executing MCP comparison between retained evaluator provenance and the current
  catalogue. `MissionEvaluatorReplayComparisonReport.status` distinguishes unchanged, drifted,
  missing-binding, not-recorded, and invalid-digest states while preserving the explicit limitation
  that a retained digest is not an historical row-by-row catalogue snapshot. The sync/async
  `Workspace` methods and `ApiClient`/`AsyncApiClient.mission_evaluator_replay_compare(...)`
  helpers use the same contract; REST callers use
  `mission_evaluator_replay_compare_query(...)`.
- `MissionEvidenceBundleRequest` bounds result/trace/fixture inclusion and `max_items` for the
  durable `/evidence-bundle` export. `MissionEvidenceBundleReport` preserves summary-only versus
  full retention, optional result and trace, replay/catalogue-drift projections, omission metadata,
  execution provenance, links, and the 64-character content `bundle_digest`. The export is bounded
  and non-executing; oversized bundles are refused rather than truncated.
- `MissionEvaluatorReviewReport.catalogue_snapshot` preserves the bounded, content-addressed adapter
  rows retained at review time; `snapshot_retained` is a convenient retention check. Replay comparison
  reports exact adapter row additions, removals, changes, unchanged IDs, and changed top-level fields
  when the snapshot is valid, while legacy digest-only checkpoints retain their explicit limitation.
- `MissionEvidenceBundleVerifyRequest` and `MissionEvidenceBundleVerificationReport` verify an exported
  bundle's canonical digest, retained-result digest, schema, retention, trace, and export contract
  without executing anything. `Workspace`/`AsyncWorkspace` expose the typed MCP path, while
  `ApiClient`/`AsyncApiClient` expose both the durable REST route and the explicitly named MCP bridge
  (`mission_evidence_bundle_verify_tool`). REST uses `POST /v1/evidence-bundles/verify`. A tampered
  but well-formed bundle returns `valid=False` with failure codes instead of being mistaken for a
  transport error.
- `MissionEvidenceBundleImportRequest`, `MissionEvidenceBundleQueryRequest`, and
  `MissionEvidenceBundleGetRequest` expose the bounded evidence registry. The corresponding typed
  import/query/get reports preserve idempotency, digest-ordered cursor rows, mission/domain filters,
  and the explicit non-executing posture. `ApiClient`/`AsyncApiClient` use the durable REST routes;
  `Workspace`/`AsyncWorkspace` and the `*_tool` methods use MCP's process-local registry bridge.
  Configure the API's `--evidence-state` path for restart-safe persistence; restored bundles are
  reverified and never resume execution or become scientific, clinical, provenance, or release claims.
- `CapabilitySearchReport.from_wire(...)` plus `Workspace.capability_discover_report(...)`,
  `AsyncWorkspace.capability_discover_report(...)`, and the corresponding HTTP helpers validate
  ranked groups, cross-domain metadata, result counts, digest provenance, and optional tool
  schemas. `report.domains` and `report.tools` provide deterministic coverage projections.
- `capability_audit(include_groups=...)` verifies catalogue/schema parity, input-schema quality,
  coverage gaps, and intentional multi-group membership.
- `CapabilityAuditReport.from_wire(...)` plus `Workspace.capability_audit_report(...)`,
  `AsyncWorkspace.capability_audit_report(...)`, and the corresponding HTTP helpers expose typed
  parity counts, catalog-only/advertised-only gaps, duplicate memberships, invariant flags, bounded
  schema findings, and optional per-group coverage. Use `report.catalogue_complete` and
  `report.schema_quality.fully_valid` as explicit inspection signals; they are evidence for planning,
  not authorization or domain validity.
- `capability_dashboard(...)` returns a bounded, digest-bound projection of the selected catalogue
  groups. `CapabilityDashboardReport.from_wire(...)` plus the sync/async Workspace and HTTP helpers
  expose separate crate, CLI, Python, MCP-membership, and schema-backed counts, callable/partial/
  declared-only readiness, explicit gap labels, filter provenance, and truncation warnings. A ready
  dashboard is a transport-coverage signal only; it does not execute or authorize a tool.
- `capability_route(goal, needs, ...)` batches named needs into a digest-bound, non-executing route
  proposal, preserving explicit tool matches separately from ranked candidates. Its raw result also
  includes per-need candidate domains and a `route_coverage` ledger for resolved/unresolved needs.
- `CapabilityRouteReport.from_wire(...)` validates that route-level counts reconcile with per-need
  evidence, recommended-tool overflow, and the aggregate domain/group/tool ledger. The
  `capability_route_report(...)` parser accepts either a decoded stdio payload or an HTTP REST
  envelope; `Workspace.capability_route_report(...)`, `AsyncWorkspace.capability_route_report(...)`,
  `ApiClient.capability_route_report(...)`, and its async counterpart provide bounded typed views
  without executing any candidate. `report.route_coverage.fully_resolved` is routing evidence only,
  not authorization, domain validity, or scientific readiness.
- `CapabilityRouteReviewRequest` and `capability_route_review(...)` validate caller-selected
  handoff inputs, while `CapabilityRouteReviewReport.from_wire(...)` and the corresponding sync,
  async, and HTTP `capability_route_review_report(...)` helpers expose blocked/ready findings,
  candidate mismatches, missing selections, and deterministic dependency waves. A ready report
  contains a mission draft but explicitly remains `mission_preflight_required`. Pass
  `validate_schemas=True` to request authoritative per-tool schema digests and bounded issue paths
  in `report.schema_review`. The resulting `report.review_id` is a deterministic,
  content-addressed correlation key for the route provenance, selections, and validation mode.
- `DomainWorkflowInstantiateRequest`, `DomainWorkflowCatalogueReport`, and
  `DomainWorkflowInstantiationReport` expose the complete workflow-template bridge. Sync and async
  `Workspace` methods call the MCP tools; `ApiClient` and `AsyncApiClient` also expose the dedicated
  REST routes plus tool-call variants. The request validates bounded explicit steps, while typed
  reports preserve catalogue/workflow digests, missing-tool coverage, selected-tool scope,
  the selected domain contract, a step-level evidence plan, authoritative `preflight_report`, and
  `execution: "not_started"`. No client method dispatches a domain tool through this workflow layer.
- `DomainWorkflowScaffoldRequest` and `DomainWorkflowScaffoldReport` provide the deterministic
  planning shortcut for all capability groups. `ApiClient`/`AsyncApiClient` expose REST and
  `*_tool` variants, while `Workspace`/`AsyncWorkspace` expose the MCP bridge. The typed request
  preserves explicit tool and per-tool argument selection; the report preserves selected/omitted
  tools, bounded live `argument_contract` facts, structured ready/blocked preflight, and the
  invariant `readiness_claimed is False`. A scaffold never dispatches, grants permission, or makes
  a domain conclusion.
- `DomainWorkflowReconcileRequest` and `DomainWorkflowReconciliationReport` join that instantiation
  to a retained `agent_mission` report or evidence bundle. `ApiClient`, `AsyncApiClient`, and
  `Workspace` expose sync/async REST and MCP helpers; the typed report separates integrity validity,
  per-step evidence retention, completion status, and review-required posture. Summary-only bundles
  remain verifiable artifacts but cannot become completion evidence.
- `DomainWorkflowReconciliationImportRequest`, `DomainWorkflowReconciliationQueryRequest`, and
  `DomainWorkflowReconciliationGetRequest` expose the bounded durable reconciliation registry.
  `ApiClient`/`AsyncApiClient` use the REST import/query/get routes, while `Workspace`/
  `AsyncWorkspace` and the `*_tool` methods use the MCP bridge. Typed reports preserve idempotency,
  digest-ordered cursor rows, mission/workflow/plan/status filters, and exact content-hash lookup.
  Configure `--reconciliation-state` for restart-safe API persistence; restored reports remain
  non-executing audit evidence and never become provenance, scientific, clinical, safety, or release
  approval.
- `ApiClient.route_review_evidence(...)` and `AsyncApiClient.route_review_evidence(...)` expose
  bounded retained event evidence for that exact id as `RouteReviewEvidence`; `event_page(...)`,
  `event_stream(...)`, and raw `events(...)` also accept `review_id=...` for transport-native
  filtering. An empty page is explicitly “not found in the retained cursor window,” not proof that
  the review never existed.
- `ApiClient.delivery_receipt_events(...)` and its async counterpart expose the parallel
  `DeliveryReceiptEvents` join for a content-addressed delivery receipt. `event_page(...)`,
  `event_stream(...)`, and raw `events(...)` accept `receipt_id=...` as a mutually exclusive
  filter. Large receipt responses retain a bounded projection in the event row; callers must still
  use the receipt verifier for integrity.
- `mission_from_route(route, mission_id, selections, policy=...)` converts that route into a
  provenance-preserving `MissionAssembly` only after every need has one caller-selected candidate,
  explicit JSON arguments, and domain-labelled mission metadata. It refuses unresolved or
  out-of-candidate tools, performs no transport call, and is intended to feed `mission_preflight()`
  before `agent_mission()`.
- `ApiClient.submit_mission(request)` and `AsyncApiClient.submit_mission(request)` submit the same
  typed `MissionRequest` to the bounded asynchronous HTTP executor. `mission_status(mission_id)`
  returns a typed `MissionJob` with the raw authoritative report when terminal, and
  `cancel_mission(mission_id, reason=...)` requests cooperative cancellation. Cancellation stops
  future dispatch between nested calls or parallel batches; it does not force-kill an in-flight
  tool or imply rollback.
  `MissionJob.progress` is a typed `MissionProgress` snapshot with phase, wave, active/completed
  counts, outcome counters, byte totals, and the latest trace cursor/event; it is safe for bounded
  dashboards but does not replace the terminal report or claim domain success.
  `mission_trace(mission_id, after=..., limit=...)` returns a typed `MissionTracePage` with ordered
  `MissionTraceEvent` rows, an exclusive `next_after` cursor, and explicit retention-gap metadata.
  `mission_inventory(...)` returns a typed bounded `MissionInventoryPage` with progress, outcome
  counters, and lifecycle links. `wait_mission(...)` and its async counterpart poll only until a
  terminal state inside an explicit timeout/poll interval bound; `MissionWaitTimeout` retains the
  last authoritative live `MissionJob` so callers can resume, cancel, or inspect without parsing
  an exception string.
  If the gateway is started with `--mission-state`, restored jobs expose
  `recovered_after_restart`; interrupted queued/running work is an explicit failed record, and
  `result_omitted` carries bounded byte-count/SHA-256 metadata when a persisted report was too large.
  `delete_mission(mission_id)` removes a terminal job when the caller has consumed its report;
  active jobs are refused so cleanup cannot discard in-flight work.
  `mission_persistence()` and `flush_mission_persistence()` provide typed operator checks for
  the optional mission checkpoint, including its content `state_digest`, without implying durable
  mission scheduling or effect rollback. Schema-1 migration inputs report no digest before the
  gateway rewrites them as schema 2; `integrity_verified` reports whether the observed current
  file matches that digest.
  `mission_queue()` returns a typed `MissionQueueInventory` with lifecycle state, resource class,
  idempotency class, attempt counters, recovery metadata, and links while keeping the checkpointed
  job specification out of the inventory. `mission_queue_persistence()` returns a typed
  `MissionQueueStatus` with the content digest, atomic-checkpoint integrity result, and startup
  recovery rows plus admission/backpressure limits and observed lease occupancy. Its `authority`
  projection exposes the shared-file lock, revision, event count, authority digest, and queue
  digest so operators can distinguish a queue projection from the journal that authorized it.
  `flush_mission_queue_persistence()` returns a typed `MissionQueueFlushResult`. Queue attempt
  numbers are local fencing tokens; a stale attempt is refused even when a worker identity is
  reused.
  `release_mission_queue_lock(operator, reason)` and its async counterpart provide the explicit,
  attributed operator override for a stale local authority lock and return the recorded release
  revision. These methods expose cooperating-process local-file authority; `automatic_resume` is
  explicitly false and no method claims multi-host consensus, network-partition tolerance,
  authentication, tenant isolation, or external-effect completion.
  `recovery_matrix()` and its async counterpart provide one typed matrix that keeps mission,
  event, subscription, outbox, secret, and external-effect recovery boundaries separate.
  `operations_snapshot(after=..., limit=...)` and its async counterpart return a typed
  `OperationsSnapshot` that composes a bounded `EventPage`, event metrics, mission status-count
  evidence, typed mission/event/reconciliation persistence checks, the reconciliation summary,
  including its per-workflow status matrix, the recovery matrix, typed domain-group/tool
  coverage, capability flags, consistency declaration, and operator action/non-claim text. The
  domain model preserves exact missing tool names and omission counts without calling them
  runtime or scientific readiness. The cursor remains explicit in
  `snapshot.recent_events`; the
  model rejects non-reconciling mission counts and missing required metrics, and the snapshot is
  read-only evidence rather than a mission resume, webhook send, or scientific verdict.
  `event_persistence()` and `flush_event_persistence()` provide the corresponding typed event
  and outbox checkpoint checks; endpoint/filter metadata and signed pending envelopes can restore,
  while secrets remain non-durable. They expose the current checkpoint's optional `state_digest`
  for operator correlation and `integrity_verified` for the observed current file; schema-1/2
  migration files legitimately report no digest before rewrite.
  `operations_handoff(...)` and its async counterpart return a typed, content-addressed
  `OperationsHandoff` with selected domain groups, exact catalogue gaps, unresolved selectors,
  and a ready-to-submit `route_request`. It never dispatches the route; callers continue through
  `capability_route_report(...)`, `capability_route_review_report(...)`, and mission preflight.
  `operations_domain_activity(after=..., limit=...)` returns typed per-domain activity rows with
  explicit event-cursor scope, exact observed tools, and catalogue-gap/unobserved distinctions;
  it is activity evidence rather than a readiness claim.
  `operations_domain_gates(after=..., limit=...)` returns typed per-domain evidence gates for
  catalogue, activity, transport completion, pooled evaluation, domain-evaluator, safety, and
  release channels. Each `OperationsDomainGateGroup` also exposes typed
  `reconciliation_evidence`, joined by exact capability-group `workflow_id` to the bounded
  digest-valid registry. Its `missing`, `incomplete`, `invalid`, and `structurally_ready` states
  remain evidence posture only: incomplete or invalid retained posture blocks review, while a
  structurally-ready row still requires authority review. Domain-evaluator rows retain the exact
  evaluator tool/event binding without asserting scientific validity. Gate rows retain refusal/completion evidence and remain `insufficient_evidence` or `review_required`
  until a separate authority reviews them; the typed response enforces `readiness_claimed: false`.
  `OperationsGateReviewRequest` plus `create_operations_gate_review(...)` persists a current
  operator decision. `operations_gate_reviews(...)` replays the durable record by cursor or
  `review_id`; the resulting `OperationsGateAcceptance` binds an executable `MissionRequest` to
  the retained review ID, current gate digest, reviewed group IDs, reviewer, rationale, and all
  seven accepted gate names. The HTTP boundary rechecks that acceptance against the current retained evidence before queueing execution;
  `MissionRequest.to_mcp_arguments()` preserves the acceptance document for that boundary.
  Accepted executable jobs expose `MissionJob.execution_provenance` and the typed
  `mission_provenance(...)`/async helper. That projection correlates the retained review, gate
  digest, domain-evaluator evidence, bounded preflight evidence, and accepted-dispatch event;
  it remains an audit/replay record rather than a readiness claim.
  `MissionClaimRequest` lets callers attach bounded, explicit claim-to-step requirements to a
  `MissionRequest`. The terminal `MissionExecutionReport.claim_lineage` property and
  `mission_claim_lineage(...)`/async helper expose retained result digests, omission states, and
  non-claim posture. `MissionClaimEvaluatorBinding` adds explicit adapter/domain/output-pointer
  coverage with required-retention counts. Supplying the ready `mission_evaluator_review` response as
  `MissionRequest.evaluator_review` makes the Rust boundary recheck catalogue freshness and exact
  reviewed rows before dispatch. `MissionExecutionReport.evaluator_review`, lineage review
  provenance, `MissionClaimLineage.evaluator_outcome_states`, outcome counts, output source/type/size,
  refusal/blocked/cancelled/omitted/pointer-missing states, and digest groups remain available without
  semantic adjudication. `claimable` only means the requested transport evidence was retained; it is
  never a truth, safety, or release verdict.
  `rebind_subscription(...)` supplies a secret in memory,
  re-signs pending envelopes, and reactivates a paused restored subscription.
- `ToolCatalogue`, `ToolCallPlan`, and `tool_checked(...)` provide a checked escape hatch for the
  complete live MCP catalogue, including domains that do not yet have a handwritten convenience
  method. The catalogue is copied from `tools/list` or `/v1/tools`, deduplicated, bounded, and
  digest-addressed; preflight enforces only conservative JSON Schema shape (`required`, types,
  arrays, enums, bounds, and common combinators). Unsupported schema features are warnings, not
  hidden passes. A plan has no side effects, and remote refusals remain `ToolRefusal` rather than
  becoming a successful result.
- `AdapterRegistry` and `adapter_plan(...)` expose a dependency-free biological source planner.
  It matches explicit formats and source shapes to native or Python-delegated routes, optionally
  checks installed optional packages without importing them, and reports the adapter's declared
  semantic-loss and scope surface. `Workspace.adapter_plan(...)`, `ApiClient.adapter_plan(...)`,
  and their async counterparts forward the same request over MCP or HTTP. Planning never sniffs,
  fetches, parses, executes, or grants credentials; DICOM, NIfTI/BIDS, AnnData/Zarr, VCF, BAM/CRAM,
  OME-Zarr, FASTA, FASTQ, SAM, GFF3, PDB, SDF/MOL, mzML, and FHIR readers remain responsible for source-specific conformance in the Python layer.
- `AdapterPlanReport.from_wire(...)` plus `adapter_plan_report(...)`,
  `Workspace.adapter_plan_report(...)`, `AsyncWorkspace.adapter_plan_report(...)`, and the
  corresponding HTTP helpers project the complete authoritative envelope: selected descriptor,
  every candidate status/reason, optional dependency posture, conformance level, accepted formats,
  scope dimensions, declared semantic-loss kinds, and non-executing limitations. The projection
  reconciles top-level and nested `executable` state and keeps a dependency-blocked candidate
  distinguishable from an unsupported format or source shape.
- `DomainAcquisitionQuery`, `domain_acquisition_report(...)`, and the sync/async
  `Workspace`/`ApiClient` helpers expose the digest-bound `domain_acquisition_catalogue` route.
  Each selected domain keeps bounded transport (`file`/plain `generic_http` versus
  caller-managed connectors) separate from native/Python-delegated adapter interpretation,
  includes explicit scope-match evidence and limitations, and preserves truncation and
  completeness flags. This is cross-domain routing evidence only: it does not execute a source,
  import a Python reader, resolve an ontology, or promote a matched adapter to conformance.
- `TabularIngestRequest` and `tabular_ingest(...)` execute the Rust CSV/TSV adapter only after an
  explicit profile and exactly one inline string or root-confined document are supplied. The
  typed `TabularIngestReport` returned by `tabular_ingest_report(...)` preserves the source and
  profile manifest digests, bounded facts and omissions, semantic-loss variant, and every
  independent conformance check. `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`
  expose the same request/result boundary; a passed report is still mapping/accounting evidence,
  not proof that the source declarations are scientifically true.
- `ConformanceRunArgs` and `conformance_run(...)` run the shipped fixture-verified FIBER suite;
  `ConformanceRunReport` preserves fixture drift, suite digest, pyramid counts, bounded case
  outcomes, and the noncompensatory release decision. `conformance_run_report(...)` is available
  on `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`; blocked decisions retain
  every unmet gate and actionable evidence, while `results=None` explicitly means details were
  not requested rather than that the suite had no cases.
- `ReleaseAuditArgs`, `ReleaseAuditCheckRequest`, and `release_audit(...)` compose up to 32 exact
  delegated release checks across registry, bundle, conformance, research CI, quality, operations,
  pack health, repository impact, and developer-platform diagnostics. `ReleaseAuditReport` keeps
  required gates, advisory-only observations, evaluated failures, invocation refusals, result
  digests, fail-closed blockers, guarantees, and limitations distinct. Its parser rechecks ordered
  row indexes, count parity, blocker references, advisory null gates, and the Rust aggregator's
  strict conjunction, so a forged or compensating top-level `release_ready` value is rejected.
  `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient` expose the same typed boundary.
- `OperationsCatalogArgs` and `operations_catalog(...)` expose the operations/infrastructure
  contract surface: local/team storage topology, promise parity independent of technology,
  five closed data classes, nine deployment planes, tenant patterns, named SLO objectives, nine
  service-contract audits, defined-versus-undefined metric debt, and the SDK registration boundary.
  `OperationsCatalogReport` validates the closed sets, truncation counts, topology parity, service
  divergence accounting, and the explicit nonclaim that undefined metrics are not zeroes.
- `OpsAcceptanceArgs` and `ops_acceptance(...)` preserve the fourteen-condition alpha acceptance
  result as typed `met`, `refuted`, and `unverifiable` findings. `OpsAcceptanceReport` refuses to
  turn unverifiable criteria into a percentage or release pass, validates basis variants such as
  linked types and no-observer explanations, and exposes `release_ready` and `decidable` only as
  the Rust summary predicates. Both workflows are available on sync/async MCP and HTTP facades.
- `RiskAssessmentRequest`/`SafetyReleaseGateArgs` and `safety_release_gate(...)` expose the
  dual-use release gate over the nine closed risk dimensions and six sensitive categories.
  `SafetyReleaseGateReport` validates the exact cleared/conditioned/blocked decision, driver
  dimensions, conditioned controls, subject parity, and fail-closed omission rule: a successful
  report cannot contain unrated dimensions, and an unrated assessment remains a remote refusal.
- `MedicalBoundaryRequest` and `medical_boundary_check(...)` expose the research-only medical
  boundary over enumerated research use cases and prohibited clinical output categories.
  `MedicalBoundaryReport` preserves either admitted research use or structured clinical refusal,
  requires an unconditional boundary flag, and never turns a clinical refusal into a successful
  recommendation. Both safety workflows are available on sync/async MCP and HTTP facades.
- `SafetyPostureArgs` and `safety_posture(...)` expose the section-13 threat-model posture with
  separate mitigated, declared-only, unmitigated, residual, unanalysed, and unreachable
  populations. `SafetyPostureReport` reconciles coverage counts, population subsets, and optional
  full threat details; each mitigation keeps its state-specific declaration, absence reason, or
  enforcement basis. The report explicitly retains that this is a model projection, not runtime
  sandboxing or perimeter enforcement.
- `SecurityRedteamSimulateArgs` and `security_redteam_simulate_report(...)` type the complete
  section-13 contract replay across sync MCP, async MCP, and HTTP. Separate reports retain
  confirmed-finding regression eligibility, disclosure transitions and missing advisory fields,
  permitted versus refused trust-boundary deliveries, across-trial feedback, incident requests
  and containment claims, hash-linked audit verification, and observed-versus-asserted
  attestations. Per-row refusals remain fail-closed; the report explicitly preserves the endpoint's
  nonclaims about fuzzers, sandboxes, notification, credential revocation, and durable security
  infrastructure.
- `WorldGenerateArgs` and `world_generate_report(...)` type deterministic synthetic world/query
  generation across sync MCP, async MCP, and HTTP. `WorldGenerationCountsReport` retains facts,
  factors, events, subjects, distractors, relay depth, and generated targets; diagnostics preserve
  warning versus error severity; and `WorldGenerateReport` keeps world/query identifiers, exact
  content digests, optional documents, and fail-closed parse/validation stages distinct. The
  report exposes deterministic, digest-binding, and side-effect-free guarantees without treating
  generation as a clinical, network, model, or publication action.
- `FactoryLifecycleSimulateArgs` and `factory_lifecycle_report(...)` type the bounded factory
  replay across sync MCP, async MCP, and HTTP. `FactoryActionTraceReport` retains the ordered
  action index, operation kind, success/refusal, and fail-closed marker; lease results, recovery
  outcomes, staged-output invisibility, final job states, committed results, quarantine, dead
  letters, resource-class counts, and invariant guarantees remain separate. The argument envelope
  enforces the authority's 256-job, 256-worker, 2000-action, unique-worker, and 20 MB bounds while
  preserving unknown operation kinds for the Rust authority to refuse explicitly. This is an
  in-memory deterministic lifecycle projection: it does not create workers, queues, clocks,
  durable leases, filesystem state, network effects, or external side effects.
- `StorageLifecycleSimulateArgs` and `storage_lifecycle_report(...)` type deterministic storage
  planning across sync MCP, async MCP, and HTTP. `StorageTieringReport` retains the caller epoch,
  thresholds, tagged idle/recent/pin-held transition reasons, skipped-tier witnesses, bytes by
  target, input-row refusals, and applied-versus-absent counts. `StorageQuotaReport` keeps the hard
  limit, protected reserve, realized usage, purpose-specific remaining allowance, five raw storage
  classes, charge/release/delegation/absorption rows, remaining child allowances, and fail-closed
  refusals distinct. Request bounds mirror Rust: 20 MB, 1000 records/actions, 100 delegated
  children, and max-item truncation. This is a deterministic in-memory projection; it does not
  move bytes, persist an audit log, enforce a backend write, or create tenant isolation.
- `RegistryLifecycleSimulateArgs` and `registry_lifecycle_report(...)` type the local benchmark
  registry state machine across sync MCP, async MCP, and HTTP. `RegistryPackPreflightReport` keeps
  invalid attested documents as fail-closed rows; `RegistryIntegrityReport` types the pre-mutation
  serialized-index check; `RegistryActionReport` preserves publish, promote, reassess, supersede,
  withdraw, resolve, history, inspect, revisions, and verify-all results/refusals; and
  `RegistryFinalReport` retains artifact counts, append-only events, and verification rows. A
  returned serialized index remains explicit continuation state rather than an implicit durable
  registry, and the report preserves the nonclaims about signatures, identity, federation,
  moderation, authentication, and network transport.
- `CacheInvalidationSimulateArgs` and `cache_invalidation_report(...)` type the deterministic cache
  invalidation workflow across sync MCP, async MCP, and HTTP. `CacheKeySchemaReport` preserves the
  declared component set and reuse rule; `CacheGraphReport` and `CacheCompletenessReport` retain
  opaque/unknown regions; `CacheInvalidationPlanReport` distinguishes invalid, proved-unaffected,
  and unproven entries; `CacheLookupReport` keeps hits, cross-build/schema/unproven/cold misses,
  proofs, and fail-closed lookup refusals separate; and `CacheApplyReport`/`CacheReproveReport`
  preserve explicit mutation and attributed restoration. The request envelope mirrors the Rust
  bounds and never turns a partial invalidation into a clean result or a missing value into zero.
- `HubDisclosureReviewArgs` and `hub_disclosure_review(...)` type the public-hub disclosure review
  across sync MCP, async MCP, and HTTP. `HubDisclosureStateReport` preserves the digest-keyed
  unknown/held-out/disclosed/contaminated ratchet and its contamination witness; headline action
  rows retain caveated labels separately from withheld, unacknowledged, or contaminated outcomes;
  the serialized ledger is available as explicit continuation state. The parser reconciles action
  counts and requires every failed action to carry a refusal plus `fail_closed=True`. It does not
  claim to detect leaks, certify secrecy from a clean split oracle, or publish a score/network page.
- `HubCardRenderArgs` and `hub_card_render(...)` type the BioAtlas card renderer across sync MCP,
  async MCP, and HTTP. `HubCardObjectReport` preserves resource identity, scope, provenance,
  access, moderation-derived publication state, verification, non-claims, attributions, and the
  limitations card. `HubCardScoreReport` keeps the published/withheld tagged union explicit;
  score attachment, disclosure-gate refusals, available-state refusals, and numeric exposure are
  separate properties. A withheld display is never parsed as zero, and the renderer is not treated
  as HTML generation or network publication.
- `MeasurementCompareArgs` and `measurement_compare(...)` preserve standards-aware comparability
  across scalar, spatial, genomic, unit, frame, reference-build, and ontology declarations.
  `MeasurementCompareReport` reconciles the boolean with the tagged verdict, records every unit
  conversion and its exact/conventional status, retains caveats, validates the report digest, and
  exposes the first typed blocking reason rather than silently coercing values.
- `LiteratureBindCheckArgs` and `literature_bind_check(...)` preserve the distinction between
  binding a source claim to a typed scope and admitting it as citation support. `LiteratureBindCheckReport`
  retains the requested tier, historical horizon, bound/citable states, citation-laundering and
  population refusals, temporal leakage, retraction warrants, and unsupported biological claim
  kinds. A bound literature claim is not parsed as a measurement; only `published_claim_support`
  can be cited through the literature modality. See
  [`docs/LITERATURE_BIND_CHECK.md`](LITERATURE_BIND_CHECK.md).
- `ModalitySupportCheckArgs` and `modality_support_check(...)` expose the modalities support
  relation for all assay families. `ModalitySupportCheckReport` keeps claim eligibility separate
  from analysis-unit independence, preserves wrapper and root refusal kinds, returns claim
  requirements and resolution states, and accepts a bounded study-specific descriptor. A
  supported modality is not parsed as statistical or biological truth. See
  [`docs/MODALITY_SUPPORT_CHECK.md`](MODALITY_SUPPORT_CHECK.md).
- `ModalityTransportCheckArgs` and `modality_transport_check(...)` expose aggregation,
  deconvolution, and imputation loss ledgers, exact-versus-estimated fidelity, invertibility,
  scope-mapping evidence, post-transport descriptors, and before/after claim-support rows. The
  parser keeps an inverse separate from value recovery and never treats a declaration audit as a
  computation. See [`docs/MODALITY_TRANSPORT_CHECK.md`](MODALITY_TRANSPORT_CHECK.md).
- `ModalityComparabilityCheckArgs` and `modality_comparability_check(...)` compare serialized
  `ModalMeasurement` values through the modality-aware kernel before standards delegation.
  `ModalityComparabilityCheckReport` keeps measurand, resolution, imputation, standards refusals,
  caveats, and the canonical report digest visible; a comparable verdict means category
  compatibility, not equality or biological agreement. See
  [`docs/MODALITY_COMPARABILITY_CHECK.md`](MODALITY_COMPARABILITY_CHECK.md).
- `ObligationGateCheckArgs` and `obligation_gate_check(...)` expose the fail-closed action gate
  over a serialized dependency-aware obligation graph. `ObligationGateCheckReport` preserves the
  typed allowed/blocked decision, effective dependency states, mandatory-closure refusal,
  prerequisite evidence, frontier rows, graph digest, and bounded omission counts. Permission is
  not execution, authority authentication, evidence acquisition, or calibrated probability. See
  [`docs/OBLIGATION_GATE_CHECK.md`](OBLIGATION_GATE_CHECK.md).
- `HubSearchArgs` and `hub_search(...)` preserve caller-supplied federation, catalog, and exact
  facet query declarations under the server's catalog/release/result bounds. `HubSearchReport`
  types every match, non-empty facet explanation, near miss, trust tier, namespace authority,
  freshness state, digest, and truncation count; mirror provenance is never collapsed into origin
  authority and omitted exclusions cannot be mistaken for an exhaustive result.
- `HubResolveArgs`/`hub_resolve(...)` and `HubLockArgs`/`hub_lock(...)` retain exact resolved
  subjects, digest identity, answering authority, freshness state, accepted policy, lifecycle
  notes, transitive `required_by` witnesses, and bounded omitted-entry counts. The typed lock
  parser reconciles every visible entry's name and answerer without pretending a partial lock is a
  complete dependency closure.
- `BidsAdapter` and `audit_bids(...)` provide a bounded, dependency-free audit of a caller-supplied
  BIDS manifest: relative paths, entity syntax, directory/entity agreement, JSON sidecar
  inheritance, equal-specificity metadata conflicts, task metadata, participant coverage, and
  derivative pipeline descriptions. The report hashes normalized input and states that it did not
  read image bytes; NIfTI/DICOM/EEG/MEG interpretation remains a separate adapter concern.
- `DicomAdapter` and `audit_dicom(...)` provide a bounded parsed-projection audit for study/series/
  SOP identity, duplicate instances, dimensions, frame-of-reference consistency, orthonormal image
  geometry, slice positions, enhanced multi-frame positions, and provenance. It returns digest-bound
  UID summaries without echoing arbitrary patient tags, and separates structural validity from
  publishability when coordinate or provenance losses are blocking. Pixel decoding and transfer
  syntax decompression remain the responsibility of the optional binary reader.
- `NiftiAdapter` and `audit_nifti(...)` provide a bounded parsed-header audit for shape, datatype,
  effective affine, qform/sform declarations, voxel-size agreement, axis codes, spatial/time units,
  series consistency, and coordinate-frame provenance. The result returns an affine digest rather
  than raw matrices in its summary, separates structural validity from publishability, and makes
  clear that image arrays, compression, extensions, and BIDS sidecars were not decoded.
- `AnnDataAdapter` and `audit_anndata(...)` provide a bounded parsed projection audit for `n_obs`/
  `n_vars`, X/layer sparse structure, obs/var index identity, annotation lengths and categories,
  obsm/varm embeddings, obsp/varp pairwise matrices, raw dimensions, and safe `uns` summaries. It
  returns index digests rather than index values, separates structural validity from provenance-
  gated publishability, and does not read HDF5/Zarr chunks or matrix payloads.
- `AlignmentAdapter` and `audit_alignments(...)` provide a bounded parsed BAM/CRAM projection audit
  using explicit 0-based half-open coordinates. It checks the reference dictionary, CIGAR query and
  reference spans, coordinate bounds, flags, primary mate pairing, coordinate sort order, mapping
  qualities, and per-reference coverage. Read identities are source-bound digests; sequences,
  qualities, auxiliary tags, indexes, and reference bases are not decoded.
- `FhirAdapter`, `audit_fhir(...)`, and `parse_fhir_json(...)` provide a dependency-free clinical
  interoperability boundary for FHIR resources and Bundles. They validate resource identity,
  Bundle structure, bounded nesting, duplicate resource keys, profile declarations, reference
  scope, unresolved local-looking references, and coded objects without terminology systems.
  Patient and resource identifiers are source-bound digests; clinical values, narratives,
  extensions, profile invariants, terminology expansion, and external-reference resolution remain
  explicit semantic-loss surfaces rather than being guessed.
- `AdapterRuntime`, `ProjectionRequest`, and `execute_projection(...)` close the planning-to-
  execution handoff for the concrete parsed projection routes across VCF, BIDS, DICOM, NIfTI,
  AnnData, alignment, FASTA, FASTQ, GFF3, PDB, SDF/MOL, mzML, OME-Zarr, and FHIR. The envelope normalizes succeeded, lossy,
  invalid, blocked, rejected, and unsupported states, carries the authoritative adapter descriptor,
  preserves the audit document digest, and returns typed unsupported outcomes when a selected raw
  reader is unavailable. Payload values are not echoed in the request envelope.
- The source-to-adapter bridge is intentionally narrower than the full local runtime registry:
  only adapters with an inline bounded body binding are eligible. Path-based and dependency-gated
  readers must be invoked through their explicit local reader contract, because the bridge never
  reopens a source locator and never silently turns a binary or incomplete transport projection
  into text.
- `ProjectionBatchRequest`, `ProjectionBatchResult`, and `execute_projection_batch(...)` compose
  heterogeneous source requests under a bounded ordered envelope. They preserve member-level
  documents, refusal/error states, status counts, adapter/failure counts, validity and publishability
  totals, semantic-loss summaries, scope dimensions, document digests, and a batch digest; optional
  stop-on-error execution reports omitted requests and marks the aggregate unaccepted instead of
  making an incomplete batch look complete.
- `read_nifti_header(...)` and `read_anndata_projection(...)` are verified optional bindings for
  installed `nibabel` and `anndata` environments. They inspect NIfTI headers with memory mapping
  and H5AD/Zarr metadata, then delegate to the same projection auditors; they never call a full
  floating-point image load or disclose matrix values. Missing packages surface as
  `OptionalDependencyUnavailable` and the runtime preserves that refusal.
- The runtime also contains dependency-gated bindings for pydicom metadata-only DICOM reads,
  pysam indexed/compressed VCF/BCF reads, and pysam BAM/CRAM record reads. These feed the DICOM,
  VCF, and alignment auditors respectively, require explicit paths and bounded record limits, and
  never turn an absent pydicom/pysam installation into a fallback parser.
- `OmeZarrAdapter`, `audit_ome_zarr(...)`, and `read_ome_zarr(...)` complete the multiscale image
  route: they validate axes, level shapes, chunks, scale/translation transforms, OME channels,
  labels, and provenance, then inspect only Zarr group/array metadata. Image chunks and pixel values
  are never loaded by the reader.
- `read_fhir_json(...)` and `read_fhir_ndjson(...)` are bounded raw-file bindings for UTF-8 FHIR
  JSON and Bulk Data NDJSON. They reject duplicate object keys and non-standard JSON numbers,
  validate every NDJSON record, and delegate to the same FHIR auditor, so a raw clinical document
  cannot silently take a different validation path than a parsed manifest.
- `FastqAdapter`, `parse_fastq(...)`, and `read_fastq(...)` provide a dependency-free sequencing
  boundary. Multiline records, sequence/quality length equality, printable quality bounds, duplicate
  read identifiers, paired-read completeness, and source-bound read/sequence/quality digests are
  retained without disclosing base or quality strings.
- `SamAdapter`, `parse_sam(...)`, and `read_sam(...)` provide a dependency-free text-alignment
  boundary. They validate SAM headers and sequence dictionaries, flag and mate consistency, CIGAR
  semantics, coordinate bounds, typed optional tags, and declared coordinate sort order while
  retaining only bounded counts, source-bound digests, and privacy-safe alignment previews. BAM/CRAM
  decoding and indexing remain separate dependency-gated capabilities.
- `FastaAdapter`, `parse_fasta(...)`, and `read_fasta(...)` provide a dependency-free reference and
  assembly boundary. They validate multiline record framing, duplicate identifiers, optional
  nucleotide/protein alphabet claims, lengths, symbol counts, and nucleotide GC totals without
  disclosing sequence strings or headers.
- `Gff3Adapter`, `parse_gff3(...)`, and `read_gff3(...)` provide a dependency-free GFF3/GTF-style
  annotation boundary. They validate coordinates, scores, strands, phases, URL-encoded attributes,
  duplicate feature IDs, Parent references and cycles, directives, and embedded FASTA boundaries
  without disclosing attribute values or feature identifiers.
- `BedAdapter`, `parse_bed(...)`, and `read_bed(...)` provide a dependency-free BED3--BED12 interval
  boundary. They validate zero-based half-open coordinates, optional score/strand/thick/RGB fields,
  block counts and non-overlap, duplicate intervals and names, and coordinate ordering while
  retaining source-bound chromosome/name digests rather than labels. BED track metadata is counted
  but never emitted, and assembly/reference-build identity remains an explicit caller responsibility.
- `PdbAdapter`, `parse_pdb(...)`, and `read_pdb(...)` provide a dependency-free structural-biology
  boundary. They validate fixed-column atoms, models, chains, residues, coordinates, alternate
  locations, crystallographic metadata, resolution, and CONECT references while retaining only
  bounded geometry summaries and source-bound structure digests.
- `SdfAdapter`, `parse_sdf(...)`, and `read_sdf(...)` provide a dependency-free small-molecule
  boundary for bounded MDL V2000 records. They validate atom/bond counts and fixed columns, element
  symbols, formal charge/isotope/radical property blocks, connectivity, coordinate summaries, and
  duplicate data fields while retaining source-bound molecule/graph digests. Molecule names,
  property values, and raw records are never emitted; V3000 records are explicitly refused.
- `MzmlAdapter`, `parse_mzml(...)`, and `read_mzml(...)` provide a dependency-free mass-spectrometry
  metadata boundary. They validate bounded XML, spectrum IDs and counts, MS levels, binary-array
  declarations, compression, precision, and encoded lengths without decoding binary arrays or
  asserting peak-level scientific interpretation.
- `parse_vcf(...)` is a bounded dependency-free text VCF reader for the first concrete biological
  adapter. It validates headers, INFO/FORMAT declarations, sample cardinality, allele indexes, and
  finite numeric values; preserves raw spellings beside typed projections; hashes the source and
  each disclosed line; and emits source-located coordinate-frame, provenance, type, and precision
  losses. It validates all records while bounding disclosed variants and loss rows, and it does not
  pretend to provide indexed/compressed/random-access functionality that belongs to `pysam`.
- `BenchmarkObservation`, `summarize_distribution(...)`, `PairedBenchmarkObservation`, and
  `paired_effect(...)` provide dependency-free notebook statistics across agent, oncology,
  multimodal, infrastructure, and coordination domains. They preserve measured versus declared,
  missing, blocked, and not-applicable rows; use deterministic quantiles and sample variance;
  optionally compute a specified-seed percentile bootstrap over observations or declared replicate
  groups; and retain limitations about exchangeability, causal interpretation, and clinical use.
- `tool(name, arguments)` remains available for every current and future MCP domain. Prefer
  `tool_checked(name, arguments)` when a live schema snapshot is available: it makes the transport
  shape inspectable before execution while preserving the server as the authority for domain
  semantics, policy, evidence, and refusal decisions. `plan_tool(...)` is the no-side-effect review
  boundary for agents that need to assemble a call graph before running it.

`ApiClient` and `AsyncApiClient` provide the same standard-library SDK posture for the HTTP
gateway: health/capability discovery, typed `capability_discover`, `capability_audit`,
`capability_route`, and `adapter_plan` helpers, REST tool calls, cursor-based event pages, and signed
webhook subscription/delivery acknowledgement. They preserve status and JSON error payloads in
`ApiError` and do not recreate Rust domain semantics. `event_page()` and `delivery_page()` add
typed `EventPage`, `ApiEvent`, and `DeliveryPage` projections with ordered cursors, retention-gap
signals, retry attempts, signatures, and pending counts; the original raw `events()` and
`deliveries()` methods remain available for forward-compatible payload inspection. `event_stream()`
parses the bounded SSE snapshot into `SseSnapshot`/`SseEvent` records and preserves `x-next-after`;
the parser rejects malformed retry fields and NUL-containing IDs before application code sees them.
Delivery rows also type `state`, `last_error`, and `last_error_retryable`; `delivery_attempts()` and
its async counterpart expose durable enqueue/send/retry/replay/acknowledgement provenance with
explicit cursor gaps and dropped-row counts. `replay()` resets selected rows to attempt one while
preserving their delivery IDs, so an operator can distinguish retryable, permanent, and exhausted
transport outcomes before choosing recovery. `delivery_receipt_attempts()` adds the cross-
subscription exact receipt join and returns a typed `DeliveryReceiptAttempts` wrapper with
`found` and the same retention evidence; the async client exposes the matching method. These
records report gateway/worker observations and never imply external receiver state beyond an
explicit sender success result.

## Metrics analytics across domains

`MetricObservation`, `PairedObservation`, and `CalibrationObservation` are small typed request
models for the Rust `metrics_analytics_audit` kernel. They deliberately use caller-owned
`dimension`, `domain`, `unit`, and `condition` strings, so one request can describe verification,
oncology, multimodal agreement, translation, runtime cost, or multi-agent coordination without a
new SDK release for every domain:

```python
from prism_sdk import (
    AnalyticsDirection,
    CalibrationObservation,
    MetricObservation,
)

report = workspace.metrics_analytics_audit(
    [MetricObservation(
        id="world-1",
        dimension="verification",
        domain="oncology",
        system="agent-a",
        value=0.82,
        direction=AnalyticsDirection.HIGHER_IS_BETTER,
        unit="fraction",
        condition="pack/4",
        replicate_group="parent-world-1",
        cost=4.2,
        latency_ms=180.0,
        evidence="reproduced",
    )],
    calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
)
```

The response preserves measured versus declared/missing/blocked populations, descriptive
performance/cost/latency summaries, replicate spread, paired deltas and retention for robustness
or cross-modal/translation/design review, and equal-width calibration bins with Brier and expected
calibration error. `Workspace` and `AsyncWorkspace` send exact wire models; they do not impute
missing rows, pool dependent trials, run an assay, infer a causal effect, or convert a contrast
into clinical evidence. An empty measured population stays `null` rather than becoming zero.

## Authoring packs, cells, and mutations

The authoring layer constructs the exact JSON shapes consumed by the Rust pack, decision-cell, and
mutation crates. It validates local invariants before a request is sent and computes the same
canonical SHA-256 content address as `bioprism-ids`:

```python
from prism_sdk import (
    DecisionCellBuilder,
    InputRef,
    PackBuilder,
    Workspace,
)

pack = (
    PackBuilder(
        pack_id="demo.pack",
        version=(1, 0, 0),
        schema_range=(1, 2),
        title="Decision evidence",
        measures="sufficiency of evidence selection",
        blueprint_module="15.01",
        axis="mechanism",
        capabilities=[{"agent": "evidence_acquisition"}],
        domains=["science"],
        owners=["aurora"],
        license="Apache-2.0",
    )
    .parent("world:demo", 3)
    .decision_family("smallest-sufficient-context")
    .mutation_relation("preserves_verdict")
    .oracle("deterministic")
    .authored_instances(8)
    .build()
)

cell = (
    DecisionCellBuilder(
        "cell-1",
        "select evidence",
        InputRef.from_document("world.json", {"world": 1}),
        InputRef.from_document("query.json", {"query": 1}),
    )
    .accepting("valid", "equivalent")
    .requiring_witness("closure")
    .build()
)
```

`PackArtifact.to_mcp_arguments()` prepares a digest-bound `pack_health_assess` request;
`Workspace.pack_health_assess()` then delegates health, calibration, contamination, and
reportability to Rust. `MutationPlan.standard()` mirrors the deterministic Rust suite and
`Workspace.mutation_family()` runs it against a root-confined world. The authoring layer never
marks a benchmark healthy, a mutation postcondition held, or a cell scientifically correct on its
own. `DecisionCell.accepts()` only evaluates the declared set-valued contract: wrong verdicts,
missing witnesses, and incomplete protected closure remain distinct failures.

## Oracle mesh and evaluation workflows

`OracleManifest`, `JudgementBuilder`, and `PositionDistribution` author the complete evidence
record used by `bioprism-oracle`: versioned `namespace:name` identities, declared versus effective
tiers, independence demotion, explicit evidential planes, fixed UTC validity windows, admissibility
states, checkable findings, abstentions, and tied distributions. A circular oracle is demoted by
the local builder before its judgement is serialized, so a caller cannot accidentally claim a
stronger rung than its declaration permits:

```python
from prism_sdk import (
    EvidenceTier,
    Independence,
    JudgementBuilder,
    OracleManifest,
    OracleRef,
    OracleVersion,
    Position,
    ValidityWindow,
)

manifest = OracleManifest(
    OracleRef("demo:checksum", OracleVersion(1, 0, 0)),
    EvidenceTier.DETERMINISTIC,
    frozenset({"artifact"}),
    frozenset({"biological", "causal"}),
    ValidityWindow("2025-01-01T00:00:00Z", "2030-01-01T00:00:00Z"),
    independence=Independence(),
)
judgement = JudgementBuilder(
    manifest, "2026-01-01T00:00:00Z", Position.SUPPORTED
).rationale("canonical artifact check passed").build()
```

`Workspace.oracle_combine()` forwards these judgements to the Rust mesh, which preserves
same-tier disagreement, suppressed weaker overrides, inadmissible evidence, and unknown states.
`oracle_reference_panel()` and `oracle_missingness()` cover independent-reader consensus and
small-cell/missingness boundaries. `bioeval_reference_audit()`, `evaluation_worldline_audit()`,
`evaluation_reproduction_check()`, and `evaluation_trajectory_check()` expose reference
uncertainty, future-evidence firewalls, reproducibility divergence, and bounded trajectory
properties. These helpers never locally choose a biological truth or convert an abstention into a
negative result.
The corresponding `*_report(...)` helpers now validate top-level invariants and preserve the
complete evidence ledgers: omitted oracle rows, nullable deciding tiers and confidence envelopes,
reader-panel refusal state, complete-case/egress determinations, distributed reference mass,
leakage versus dangling context, reproduction versus biological-validity refusal, and vacuous or
bounded trajectory properties. `OracleCombineReport` additionally types returned oracle
identities, effective/declarative tiers, positions, admissibility, findings, suppressed override
rules, disagreement sources/settlement routes/resolution, and the confidence envelope; its raw
rows remain available. The same projections are available on `Workspace`, `AsyncWorkspace`,
`ApiClient`, and `AsyncApiClient`. `BioevalReferenceAuditReport` additionally types distributed
mass, resolution, dispersion attribution, and unresolved/not-evaluable reference states. See
[`docs/ORACLE_COMBINE.md`](ORACLE_COMBINE.md) and
[`docs/BIOEVAL_REFERENCE_AUDIT.md`](BIOEVAL_REFERENCE_AUDIT.md). `EvaluationWorldlineReport`
additionally types accessibility leak witnesses and dangling-reference pairs; see
[`docs/EVALUATION_WORLDLINE_AUDIT.md`](EVALUATION_WORLDLINE_AUDIT.md). `EvaluationReproductionReport`
additionally types the certificate's ordered verdict ledger, matched/diverged/missing count
reconciliation, first-divergence row, portability posture, and fail-closed biological-validity
refusal; see [`docs/EVALUATION_REPRODUCTION_CHECK.md`](EVALUATION_REPRODUCTION_CHECK.md).
`EvaluationTrajectoryReport` additionally types step records, named path-property declarations,
held/violated/vacuous outcomes, recovery transitions, and bounded-suffix completeness; see
[`docs/EVALUATION_TRAJECTORY_CHECK.md`](EVALUATION_TRAJECTORY_CHECK.md).
`BioevalAcquisitionAuditArgs` / `bioeval_acquisition_audit_report(...)` audit ordered acquisition
traces with typed required/optional obligations, action kinds, voluntary stopping, redundancy,
deferred decisive cost, and named-policy regret. The report never treats obligation closure as
proof that a retrieval or assay executed. See
[`docs/BIOEVAL_ACQUISITION_AUDIT.md`](BIOEVAL_ACQUISITION_AUDIT.md).
`BioevalGroundingAuditArgs` / `bioeval_grounding_audit_report(...)` expose the claim-evidence graph
with five-way claim states, typed support/contradiction/adjacent edges, locator status, staleness
against an explicit freeze, lineage gaps, orphan evidence, duplicate-edge findings, and bounded
omission counts. The report does not dereference artifacts or convert citations into a score. See
[`docs/BIOEVAL_GROUNDING_AUDIT.md`](BIOEVAL_GROUNDING_AUDIT.md).
`BioevalEstimandAuditArgs` / `bioeval_estimand_audit_report(...)` preserve the five-element
estimand, association-versus-intervention claim language, evidentiary basis, not-assessed/
declared/probed identification, external-corroboration promotion, and scope transport refusals. See
[`docs/BIOEVAL_ESTIMAND_AUDIT.md`](BIOEVAL_ESTIMAND_AUDIT.md).
`BioevalEvaluatorAuditArgs` / `bioeval_evaluator_audit_report(...)` keep evaluator health,
task outcome, diagnostic completeness, hidden-data access, unscored reasons, duplicate evaluator
identities, and bounded omission counts distinct. Timed-out, errored, and broken-fixture runs cannot
become task failures, while healthy `not_met` rows without diagnostics remain refused by the real
evaluator kernel. `require_task_evidence` and `fail_on_hidden_data` are explicit fail-closed policy
gates. See [`docs/BIOEVAL_EVALUATOR_AUDIT.md`](BIOEVAL_EVALUATOR_AUDIT.md).
`BioevalPlaneAuditArgs` / `bioeval_plane_audit_report(...)` preserve scored, unscored, and
inapplicable cells, capability-tier metadata, weighted fold inclusion/exclusion, fold blockers,
and bounded dimension omission counts. The typed `BioevalScorePlaneArgs` validates cell/tier
consistency before transport; `require_fold` makes an unresolved fold fail closed. See
[`docs/BIOEVAL_PLANE_AUDIT.md`](BIOEVAL_PLANE_AUDIT.md).
`BioevalMetamorphicAuditArgs` / `bioeval_metamorphic_audit_report(...)` preserve invariant and
directional-change families, internally tagged unchanged/moved/incomparable responses, false
sensitivity, false invariance, wrong-direction, and undetermined findings. Family consistency
uses only the evidential denominator, while suite-wide consistency is intentionally absent.
`require_both_relations` and `fail_on_undetermined` are explicit fail-closed policies. See
[`docs/BIOEVAL_METAMORPHIC_AUDIT.md`](BIOEVAL_METAMORPHIC_AUDIT.md).
`BioevalWaiverAuditArgs` / `bioeval_waiver_audit_report(...)` type release-gate kinds, tagged
met/violated/unevaluable verdicts, complete authoriser/rationale/expiry/version/follow-up
waivers, before/after blocking counts, safety-veto findings, and unevaluable-gate counts. The
typed layer preserves the underlying verdict and exposes `require_releasable` and
`require_no_unevaluable` as explicit fail-closed policies. See
[`docs/BIOEVAL_WAIVER_AUDIT.md`](BIOEVAL_WAIVER_AUDIT.md).
`BioevalDesignAuditArgs` / `bioeval_design_audit_report(...)` type complete factorial arms,
explicit baseline selection, evalengine conclusions and tiers, one-factor contrasts, missing
interaction cells, unattributable multi-factor arms, and causal-versus-descriptive attribution
labels. `require_contrasts`, `require_complete_interactions`, and `require_attribution` are
explicit policy gates; the SDK never estimates an effect or claims that a controlled declaration
was independently verified. See [`docs/BIOEVAL_DESIGN_AUDIT.md`](BIOEVAL_DESIGN_AUDIT.md).

## Biological stress profiling

BioevalMeshEvaluatorArgs, BioevalMeshVerdictArgs, BioevalMeshAuditArgs, and
bioeval_mesh_audit_report(...) type evaluator kinds, consumed versus derived artifacts, transitive
shared-input classes, within-class and across-class disagreement witnesses, abstentions,
class-collapsed ratings, and optional ladder contributions. The typed layer keeps evaluator count
distinct from independent-class count and exposes require_independence and
require_independent_ratings as explicit fail-closed policies. See
[docs/BIOEVAL_MESH_AUDIT.md](BIOEVAL_MESH_AUDIT.md).

BioevalBurdenResourceArgs, BioevalBurdenBranchArgs, BioevalBurdenDrawArgs,
BioevalBurdenAuditArgs, and bioeval_burden_audit_report(...) type integer resource pools,
ordered branch inheritance, exact units, productive versus wasted draws, residuals, nonrenewable
fork feasibility, and failed-action waste. The typed layer exposes require_joint_feasible and
require_no_wasted_nonrenewable as explicit fail-closed policies without inventing prices or
utility. See [docs/BIOEVAL_BURDEN_AUDIT.md](BIOEVAL_BURDEN_AUDIT.md).

BioevalRevealCommitmentArgs, BioevalRevealOutcomeArgs, BioevalRevealAuditArgs, and
bioeval_reveal_audit_report(...) type frozen commitment targets, opaque predictions, analysis
plans, rubric and commitment digests, one-shot seal/reveal locks, uncommitted outcomes,
unrevealed commitments, and selective publication. The typed layer exposes require_scoring,
require_rubric_match, and require_complete as explicit fail-closed policies. See
[docs/BIOEVAL_REVEAL_AUDIT.md](BIOEVAL_REVEAL_AUDIT.md).

BioevalBoundaryEffectArgs, BioevalBoundaryPolicyArgs, BioevalBoundaryFlowArgs,
BioevalBoundaryAuditArgs, and bioeval_boundary_audit_report(...) type contextual-integrity
five-tuples, closed channels, authorized and compliant-denial effects, violations, vetoes,
bypasses, channel exposure, Pareto points, and guarded composite refusals. The typed layer exposes
require_no_violations and require_no_vetoes as explicit fail-closed policies. See
[docs/BIOEVAL_BOUNDARY_AUDIT.md](BIOEVAL_BOUNDARY_AUDIT.md).

`StressProfileArgs` / `stress_profile_report(...)` and `StressReportArgs` /
`stress_report_projection(...)` expose the biological-stress engine without pretending that a
single robustness score is meaningful. The typed reports retain family, blueprint module, stress
and cohort digests, identifiability, every intensity-ladder sweep point, effective rather than
nominal sample size, unresolved measurements, required-versus-probed conclusion findings, and
generator postcondition defects. A confounded batch is represented as non-informative, and a
generator defect is not converted into a fragile biological finding. `StressReportProjection`
keeps the guarded `worst_family` comparison nullable: non-identifiable or defective profiles are
not silently ranked. Inputs keep serialized Cohort, Stress, and Procedure values authoritative,
while the SDK bounds custom panels and programs to the server’s 100-entry limits. The facades are
available synchronously and asynchronously over MCP and HTTP.

`InfluenceAnalyzeArgs` / `influence_analysis_report(...)` expose caller-scoped factor-region
influence without reconstructing the graph in Python. Request validation keeps positive variable
cardinalities, assumed versus observed variables, factor scopes/tables, free variables, mutually
exclusive factor selection, perturbation class, hard budgets, and explicit execution permission
visible. `InfluenceAnalysisReport` preserves the normalized region, exact or conservative
total-variation estimate, method provenance, approximation direction, validity scope, every
attempted method, looseness, and the structural-only flag. An `unknown` estimate retains its
typed reason (for example an untabled factor) and has no numeric fallback; the SDK never promotes
unknown influence to infinity or a vacuous bound. The same projection is available on all four
sync/async MCP and HTTP facades.
`routing_decision_report(...)` adds the corresponding typed routing result to the existing bounded
`RoutingDecisionRequest`: the approved architecture tag, confidence score, abstention flag,
structured coverage/margin reason, considered architecture scores, neighbourhood evidence, task
identity, and holdout check are all retained. The parser enforces that an abstention agrees with
its reason and treats `caller_must_supply_unseen_identity` as a caller posture rather than proof
that holdout isolation was actually performed.
`RoutingLabRunArgs` / `routing_lab_run_report(...)` expose the offline multi-task routing lab. The
typed report preserves the approved panel, fixed default, task/regime holdout posture, regret
account, calibration, oracle agreement, abstention, task outcome counts, bounded task rows, and
explicit omissions. It rejects non-finite rates, unknown verdicts, unreconciled outcome counts,
and row projections that do not add back to the task count; a fail-closed lab refusal remains a
refusal rather than a zero-gain result. See [`docs/ROUTING_LAB_RUN.md`](ROUTING_LAB_RUN.md).

`ProviderCapabilityGateArgs` / `provider_capability_gate_report(...)` type the runtime-provider
evidence gate. Required checks are restricted to pass/fail capabilities; the report distinguishes
untested, failed, and passed claim states, retains reproducible run references and failure
witnesses, keeps performance measurements as measurements, and preserves per-check differential
states including indeterminate comparisons when either provider is untested. `cleared` means the
declared required checks passed; it does not execute a provider or establish general runtime
correctness.
`SdkRegistryCheckArgs` / `sdk_registry_check_report(...)` preserve plugin-manifest admission as a
two-stage fail-closed workflow: malformed declarations stop at `manifest_validation`, while valid
but conflicting or policy-incompatible sets stop at `registry_registration`; neither returns a
partial registry. Successful reports retain per-manifest whole/core digests, validation status,
capability kinds, trust evidence, normalized resolution, negotiated registrations, policy, and
load-bearing selection. Registration is evidence and deterministic resolution only; it does not
dynamically load, sign, sandbox, or execute a plugin.

## Runtime and bioethics safety workflows

The runtime projection is split into request authoring and evidence parsing so callers can inspect
the exact safety boundary without accidentally turning an inspection result into execution:

- `RuntimeEffectCheckArgs` and `runtime_effect_check_report(...)` preserve the declared effect kind,
  reversibility class, canonical path/network target, `perform` versus `simulate` authorization, and
  structured fail-closed refusals. `RuntimeEffectReport.executed` is always `False`; a successful
  authorization is not evidence that a provider exists or that any host effect occurred.
- `RuntimeTapeVerifyArgs` and `runtime_tape_verify_report(...)` verify serialized world tapes before
  trusting them, retain lineage, typed checkpoint restoration rows, artifact reads/writes,
  simulated steps, reconciled counts, and earliest digest divergence, and keep malformed-tape
  failures distinct from valid tape reports; see [`docs/RUNTIME_TAPE_VERIFY.md`](RUNTIME_TAPE_VERIFY.md).
- `RuntimeExecutionSimulateArgs` and `runtime_execution_simulate_report(...)` run bounded request
  programs only through the deterministic in-process world. The report separates complete versus
  partial recording, execution errors, budget exhaustion, replay verification/matching, and optional
  fork evidence. It additionally types deterministic-world, replay, budget, and fork projections;
  see [`docs/RUNTIME_EXECUTION_SIMULATE.md`](RUNTIME_EXECUTION_SIMULATE.md). `live_effects_reachable`
  is an explicit nonclaim, not an inferred green status.

Bioethics projections mirror the crate-level asymmetries rather than reducing them to booleans:

- `BioethicsActionReviewArgs` / `bioethics_action_review_report(...)` preserve in-silico versus
  physical partitioning and the two-act authorization referral. `physical_execution_reachable` is
  always false, including when a referral is successfully represented.
- `HumanSubjectScreenArgs` / `human_subject_screen_report(...)` keep institutional review,
  consent-at-time, and return-of-results statuses separate. An undetermined screen never becomes an
  exemption, and `clearance_issued` is required to remain false.
- `BioethicsDualUseReviewArgs` / `bioethics_dual_use_review_report(...)` require a misuse-surface
  assessment before the section-13 risk gate and retain the distinction between exploit-detail
  withholding and suppressing a finding's existence.
- `BioethicsValidationCheckArgs` / `bioethics_validation_check_report(...)` reconcile all seven
  evidence kinds and preserve experimental maturity or a fail-closed verification refusal.
- `BioethicsRepresentationAuditArgs` / `bioethics_representation_audit_report(...)` count-reconcile
  measured, unmeasured, and small-cell-suppressed strata, retaining incomplete coverage and refusing
  duplicate partitions rather than overwriting them.

All of these helpers are available on `Workspace`, `AsyncWorkspace`, `ApiClient`, and
`AsyncApiClient`. They preserve structured domain refusals in raw mode where the server uses an MCP
error envelope, so callers can choose whether to render a refusal, store it as evidence, or raise at
their own boundary. None of them provides a host sandbox, physical laboratory control, institutional
approval, clinical clearance, or biological truth.

The package deliberately does not claim to implement DICOM/NIfTI/AnnData, indexed/compressed VCF,
binary BIDS image parsing, inferential statistics, OTLP export, a notebook UI, or CI deployment. It
now ships bounded text VCF, BIDS manifest, parsed DICOM metadata, parsed NIfTI header/affine, and
parsed AnnData/Zarr matrix audits plus descriptive/cluster-bootstrap utilities above the Rust kernel.
It also ships a parsed BAM/CRAM alignment audit. The repository still keeps gRPC, durable event
storage, external webhook delivery, heavyweight binary biological readers, and statistical estimators
as separate contracts.

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

## Engineering manifest audit

EngineeringManifestArgs and engineering_manifest_audit_report expose the machine-readable
engineering manifest across sync/async MCP and HTTP. Typed nested arguments cover the technology
baseline, package graph, ticket contracts, ADR history, ownership/RACI rows, and explicit
policies. The report preserves canonical digest, dependency-first package order, cyclic
components, ticket readiness, warning/blocking issues, independent-review collisions, and
non-claims about checkout state, CI, GitHub, and release authority. See
ENGINEERING_MANIFEST_AUDIT.md.

`EngineeringPlanRequestArgs` and `engineering_execution_plan_report(...)` add a typed deterministic
execution-plan projection across sync/async MCP and HTTP. The report preserves ticket states,
dependency-aware waves, critical path, schedule gates, truncation policy, separate manifest and
planner issues, plan digests, and fail-closed readiness. It remains a planning artifact: it does
not mutate a tracker, run CI, inspect a checkout, or authorize delivery. See
ENGINEERING_EXECUTION_PLAN.md.

## Release-pipeline audit

`ReleasePipelineManifestArgs` and `release_pipeline_audit_report(...)` expose the bounded
release-pipeline contract across sync/async MCP and HTTP. Nested typed arguments cover source
identity, environments, stage dependencies, artifact digests and lineage, attestations,
promotions, approval floors, and rollback targets. `ReleasePipelineAuditReport` preserves
deterministic stage order/readiness, artifact and promotion audits, production promotions,
warning/blocking issue rows, `release_ready`, the canonical manifest digest, guarantees, and
limitations. It remains an artifact audit: it does not run CI, verify signatures, query registries,
authenticate approvals, or deploy. See RELEASE_PIPELINE_AUDIT.md.

## Operational-readiness audit

`OperationalReadinessManifestArgs` and `operational_readiness_audit_report(...)` expose the
service-operability contract across sync/async MCP and HTTP. Typed nested arguments cover service
objectives, indicator status and evidence digests, dependency fallbacks, runbook review, incident
closure, controls, and requirement policies. `OperationalReadinessAuditReport` preserves the
canonical digest, layer-specific audit rows, counts, warning/blocking issue records,
`operationally_ready`, guarantees, and limitations. It remains declaration-only and does not
query telemetry, page operators, execute runbooks, test restores, mutate incidents, or authorize
deployment. See OPERATIONAL_READINESS_AUDIT.md.

## Security/privacy governance audit

`SecurityPrivacyManifestArgs` and `security_privacy_audit_report(...)` expose typed asset,
authorized-flow, identity, threat, review, policy, and control contracts across sync/async MCP and
HTTP. `SecurityPrivacyAuditReport` preserves separate governance rows, canonical digest,
warning/blocking issues, `security_privacy_ready`, counts, guarantees, and limitations. The
projection remains declaration-only: it does not scan hosts, authenticate people, validate law,
execute red-team actions, erase records, or contact vendors. See SECURITY_PRIVACY_AUDIT.md.

## Sandbox admission audit

`SandboxManifestArgs` and `sandbox_admission_audit_report(...)` expose typed artifact lineage,
rootless/read-only/no-escalation profiles, network and mount boundaries, exact capabilities,
resource ceilings, quarantine, and reviewed output release across sync/async MCP and HTTP. The
`SandboxAuditReport` retains artifact/profile/capability/boundary/resource/output rows, canonical
digest, deterministic blocking and warning issues, `sandbox_ready`, guarantees, and limitations.
It is a declaration-only admission projection: it does not execute code, mount paths, open
sockets, inspect kernels, read or revoke secrets, operate quarantine storage, or attest runtime
enforcement. See SANDBOX_ADMISSION_AUDIT.md.

## Sandbox runtime simulation

`SandboxRuntimeManifestArgs` and `sandbox_runtime_simulate_report(...)` expose the ordered
process-side simulation across sync/async MCP and HTTP. Requests carry exact capability kinds and
targets plus positive CPU, memory, wall-time, process, and output charges. The typed
`SandboxRuntimeAuditReport` preserves admission and trace digests, per-step capability/target/
resource booleans, `simulated`/`refused`/`not_run` decisions, cumulative usage, stop-on-refusal,
and stable blocking issues. It remains a simulation artifact: no Python facade implies host
process execution, kernel enforcement, namespace/cgroup setup, secret access, network access, or
quarantine operation. See SANDBOX_RUNTIME_SIMULATION.md.

## Security, safety, and red-team program audit

`SecurityProgramManifestArgs` and `security_program_audit_report(...)` expose typed scope,
campaign, finding, remediation, incident, disclosure, and control contracts across sync/async
MCP and HTTP. `SecurityProgramAuditReport` retains seven row families, canonical digest,
deterministic blocking/warning issues, `security_program_ready`, counts, guarantees, and
limitations. The projection does not run scanners, fuzzers, probes, containment, notifications,
publication, or live-control checks. See SECURITY_PROGRAM_AUDIT.md.
