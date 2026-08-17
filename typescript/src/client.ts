import { ApiError, ArgumentError, MissionWaitTimeoutError, ProtocolError, ResponseTooLargeError, ToolRefusalError, TransportError, isObject } from "./errors.js";
import { missionFromRoute as assembleMissionFromRoute, preflightMission } from "./mission.js";
import { parseSse } from "./sse.js";
import { ToolCatalogue } from "./tooling.js";
import type {
  ApiClientOptions,
  ApiErrorBody,
  AgentMissionArgs,
  AgentMissionReport,
  AgentMissionPolicy,
  CapabilityDiscoverArgs,
  CapabilityDiscoverResult,
  CapabilityAuditArgs,
  CapabilityAuditResult,
  CapabilityDashboardArgs,
  CapabilityDashboardResult,
  CapabilityRouteArgs,
  CapabilityRouteReviewArgs,
  CapabilityRouteReviewResult,
  CapabilityRouteResult,
  DomainWorkflowCatalogueResult,
  DomainWorkflowInstantiateArgs,
  DomainWorkflowInstantiateResult,
  DomainWorkflowScaffoldArgs,
  DomainWorkflowScaffoldResult,
  DomainWorkflowReconcileArgs,
  DomainWorkflowReconcileResult,
  DomainWorkflowReconciliationImportArgs,
  DomainWorkflowReconciliationImportResult,
  DomainWorkflowReconciliationQueryOptions,
  DomainWorkflowReconciliationQueryResult,
  DomainWorkflowReconciliationGetResult,
  DomainReportProjectArgs,
  DomainReportProjectResult,
  DomainReportCoverageOptions,
  DomainReportCoverageResult,
  DomainEvidenceHarmonizeArgs,
  DomainEvidenceHarmonizationResult,
  DomainEvidenceIntakeArgs,
  DomainEvidenceIntakeResult,
  DomainEvidenceIntakeCoverageOptions,
  DomainEvidenceIntakeCoverageResult,
  DomainEvidenceSourcePlanArgs,
  DomainEvidenceSourcePlanResult,
  AdapterPlanArgs,
  AdapterPlanResult,
  TabularIngestArgs,
  TabularIngestResult,
  ConformanceRunArgs,
  ConformanceRunResult,
  ReleaseAuditArgs,
  ReleaseAuditResult,
  OperationsCatalogArgs,
  OperationsCatalogResult,
  OperationsHandoff,
  OperationsHandoffArgs,
  OperationsDomainActivity,
  OperationsDomainGates,
  OperationsGateReview,
  OperationsGateReviewRequest,
  OperationsGateReviews,
  SafetyReleaseGateArgs,
  SafetyReleaseGateResult,
  MedicalBoundaryArgs,
  MedicalBoundaryResult,
  SafetyPostureArgs,
  SafetyPostureResult,
  HubSearchArgs,
  HubSearchResult,
  HubResolveArgs,
  HubResolveResult,
  HubLockArgs,
  HubLockResult,
  WorldClaimCheckArgs,
  WorldClaimCheckResult,
  ObservedWorldDeclareArgs,
  ObservedWorldDeclareResult,
  MeasurementCompareArgs,
  MeasurementCompareResult,
  LiteratureBindCheckArgs,
  LiteratureBindCheckResult,
  ModalitySupportCheckArgs,
  ModalitySupportCheckResult,
  ModalityTransportCheckArgs,
  ModalityTransportCheckResult,
  ModalityComparabilityCheckArgs,
  ModalityComparabilityCheckResult,
  LineageAuditArgs,
  LineageAuditResult,
  PreanalyticApplyArgs,
  PreanalyticApplyResult,
  ContradictionReviewArgs,
  ContradictionReviewResult,
  LabPlanArgs,
  LabPlanResult,
  ObligationGateCheckArgs,
  ObligationGateCheckResult,
  OncoBoundaryArgs,
  OncoBoundaryResult,
  OncoClassificationArgs,
  OncoClassificationResult,
  OncoIdentityJoinArgs,
  OncoIdentityJoinResult,
  OncoOutcomeAnalyzeArgs,
  OncoOutcomeResult,
  OncoResponseAssessArgs,
  OncoResponseResult,
  OncoWorldlineViewArgs,
  OncoWorldlineResult,
  OncoWorldsModelTransportArgs,
  OncoWorldsModelTransportResult,
  OncoWorldsMethylationClassifyArgs,
  OncoWorldsMethylationClassifyResult,
  OncoWorldsMethylationCompareArgs,
  OncoWorldsMethylationCompareResult,
  OncoWorldsRadiogenomicCheckArgs,
  OncoWorldsRadiogenomicCheckResult,
  OncoWorldsClonalHistoryCheckArgs,
  OncoWorldsClonalHistoryCheckResult,
  OncoClonalEvidenceCheckArgs,
  OncoWorldsClonalEvidenceCheckResult,
  OncoWorldsEraShiftCheckArgs,
  OncoWorldsEraShiftCheckResult,
  OncoWorldsEquityCheckArgs,
  OncoWorldsEquityCheckResult,
  OncoWorldsEntityWorldCheckArgs,
  OncoWorldsEntityWorldCheckResult,
  StressProfileArgs,
  StressProfileToolResult,
  StressReportArgs,
  StressReportToolResult,
  InfluenceAnalyzeArgs,
  InfluenceAnalyzeResult,
  RoutingDecideArgs,
  RoutingToolResult,
  RoutingLabRunArgs,
  RoutingLabRunResult,
  LabParetoAuditArgs,
  LabParetoAuditResult,
  LabBranchAuditArgs,
  LabBranchAuditResult,
  LabHoldoutAuditArgs,
  LabHoldoutAuditResult,
  LabEvolutionAuditArgs,
  LabEvolutionAuditResult,
  LabSpaceAuditArgs,
  LabSpaceAuditResult,
  ProviderCapabilityGateArgs,
  ProviderCapabilityGateResult,
  SdkRegistryCheckArgs,
  SdkRegistryCheckResult,
  OracleCombineArgs,
  OracleCombineResult,
  OracleReferencePanelArgs,
  OracleReferencePanelResult,
  OracleMissingnessArgs,
  OracleMissingnessResult,
  BioevalReferenceAuditArgs,
  BioevalReferenceAuditResult,
  BioevalAcquisitionAuditArgs,
  BioevalAcquisitionAuditResult,
  BioevalGroundingAuditArgs,
  BioevalGroundingAuditResult,
  BioevalEstimandAuditArgs,
  BioevalEstimandAuditResult,
  BioevalEvaluatorAuditArgs,
  BioevalEvaluatorAuditResult,
  BioevalPlaneAuditArgs,
  BioevalPlaneAuditResult,
  BioevalMetamorphicAuditArgs,
  BioevalMetamorphicAuditResult,
  BioevalWaiverAuditArgs,
  BioevalWaiverAuditResult,
  BioevalDesignAuditArgs,
  BioevalDesignAuditResult,
  BioevalMeshAuditArgs,
  BioevalMeshAuditResult,
  BioevalBurdenAuditArgs,
  BioevalBurdenAuditResult,
  BioevalRevealAuditArgs,
  BioevalRevealAuditResult,
  BioevalBoundaryAuditArgs,
  BioevalBoundaryAuditResult,
  EvaluationWorldlineArgs,
  EvaluationWorldlineResult,
  EvaluationReproductionArgs,
  EvaluationReproductionResult,
  EvaluationTrajectoryArgs,
  EvaluationTrajectoryResult,
  OpsAcceptanceArgs,
  OpsAcceptanceResult,
  BioAtlasPublicationAuditArgs,
  BioAtlasPublicationAuditResult,
  BioCapabilityEvidenceAuditArgs,
  BioCapabilityEvidenceAuditResult,
  CapabilitiesResponse,
  ClientRequestOptions,
  DeliveryAttemptPage,
  DeliveryAttemptsResponse,
  DeliveryMutationResponse,
  DeliveriesResponse,
  DeveloperDeliveryAuditArgs,
  DeveloperDeliveryAuditResult,
  DeveloperDeliveryReceiptArgs,
  DeveloperDeliveryReceiptResult,
  DeveloperDeliveryReceiptVerificationArgs,
  DeveloperDeliveryReceiptVerificationResult,
  DeliveryReceiptEventsResponse,
  DeliveryReceiptAttemptsResponse,
  DeveloperPlatformStatusArgs,
  DeveloperPlatformStatusResult,
  EpistemicVoiArgs,
  EpistemicVoiResult,
  EpistemicContextAuditArgs,
  EpistemicContextAuditResult,
  EpistemicSelectionAuditArgs,
  EpistemicSelectionAuditResult,
  BenchmarkTraceAnalyzeArgs,
  BenchmarkTraceAnalysisResult,
  BenchmarkDecisionAuditArgs,
  BenchmarkDecisionAuditResult,
  BenchmarkIntegrityAuditArgs,
  BenchmarkIntegrityAuditResult,
  BenchmarkCounterfactualCheckArgs,
  BenchmarkCounterfactualCheckResult,
  BenchmarkOracleReviewArgs,
  BenchmarkOracleReviewResult,
  BenchmarkCompileArgs,
  BenchmarkCompileResult,
  BenchmarkCompileReviewArgs,
  BenchmarkCompileReviewResult,
  PackCoverageAuditArgs,
  PackCoverageAuditResult,
  PackReleaseAuditArgs,
  PackReleaseAuditResult,
  FoundationContractCheckArgs,
  FoundationContractCheckResult,
  PackCatalogueArgs,
  PackCatalogueResult,
  PackHealthAssessArgs,
  PackHealthAssessmentResult,
  SecurityRedteamSimulateArgs,
  SecurityRedteamResult,
  WorldGenerateArgs,
  WorldGenerateResult,
  FactoryLifecycleSimulateArgs,
  FactoryLifecycleResult,
  StorageLifecycleSimulateArgs,
  StorageLifecycleResult,
  RegistryLifecycleSimulateArgs,
  RegistryLifecycleResult,
  CacheInvalidationSimulateArgs,
  CacheInvalidationResult,
  HubDisclosureReviewArgs,
  HubDisclosureReviewResult,
  HubCardRenderArgs,
  HubCardRenderResult,
  HubLeaderboardRenderArgs,
  HubLeaderboardRenderResult,
  HubSubmissionReviewArgs,
  HubSubmissionReviewResult,
  TokenContextPlanArgs,
  TokenContextPlanningResult,
  WeaveLangCompileArgs,
  WeaveLangCompileResult,
  DeveloperWorkbenchArgs,
  CiProviderNormalizationArgs,
  CiProviderNormalizationResult,
  CiProviderEvidenceArgs,
  CiProviderEvidenceResult,
  CiExecutionEvidenceArgs,
  CiExecutionEvidenceResult,
  ExecutionProvenanceArgs,
  ExecutionProvenanceResult,
  EventMetrics,
  EventPersistenceStatus,
  EventsResponse,
  FetchLike,
  HealthResponse,
  HttpMethod,
  JsonObject,
  JsonValue,
  MetricsAnalyticsAuditArgs,
  MetricsProfileAuditArgs,
  MissionAssembly,
  MissionClaimLineageResponse,
  MissionEvaluatorDiscoverArgs,
  MissionEvaluatorDiscoverResult,
  MissionEvaluatorReplayArgs,
  MissionEvaluatorReplayCompareArgs,
  MissionEvaluatorReplayCompareResult,
  MissionEvaluatorReplayResult,
  MissionEvaluatorReplayQueryOptions,
  MissionEvaluatorReplayQueryResult,
  MissionEvaluatorReviewArgs,
  MissionEvaluatorReviewResult,
  MissionExecutionProvenanceResponse,
  MissionEvidenceBundleOptions,
  MissionEvidenceBundleResult,
  MissionEvidenceBundleImportArgs,
  MissionEvidenceBundleImportResult,
  MissionEvidenceBundleQueryOptions,
  MissionEvidenceBundleQueryResult,
  MissionEvidenceBundleGetResult,
  MissionEvidenceBundleVerifyArgs,
  MissionEvidenceBundleVerifyResult,
  ArtifactRegistrationArgs,
  ArtifactRegistrationResult,
  ArtifactCrossStoreAuditResult,
  ArtifactQueryOptions,
  ArtifactQueryResult,
  ArtifactGetResult,
  ArtifactLineageResult,
  ArtifactRegistryPersistenceStatus,
  MissionJob,
  MissionJobStatus,
  MissionInventoryResponse,
  MissionQueueFlushResponse,
  MissionQueueInventoryResponse,
  MissionQueueLockReleaseResponse,
  MissionQueueStatus,
  MissionPersistenceStatus,
  MissionPreflightResult,
  MissionRouteSelection,
  MissionTracePage,
  MissionWaitOptions,
  OperationsSnapshot,
  RecoveryMatrix,
  RepositoryBundleArgs,
  RepositoryCatalogArgs,
  RepositoryImpactArgs,
  RestToolResponse,
  RouteReviewEvidenceResponse,
  RuntimeExecutionSimulateArgs,
  RuntimeExecutionSimulateResult,
  RuntimeEffectCheckArgs,
  RuntimeEffectCheckResult,
  RuntimeTapeVerifyArgs,
  RuntimeTapeVerifyResult,
  BioethicsActionReviewArgs,
  BioethicsActionReviewResult,
  HumanSubjectScreenArgs,
  HumanSubjectScreenResult,
  BioethicsDualUseReviewArgs,
  BioethicsDualUseReviewResult,
  BioethicsValidationCheckArgs,
  BioethicsValidationCheckResult,
  BioethicsRepresentationAuditArgs,
  BioethicsRepresentationAuditResult,
  SseSnapshot,
  SubscribeOptions,
  SubscriptionListResponse,
  SubscriptionRebindResponse,
  SubscriptionResponse,
  TelemetryProjectArgs,
  TelemetryProjectionResult,
  LedgerIngestArgs,
  LedgerIngestResult,
  QualityGateRunArgs,
  QualityGateRunResult,
  AtlasReportArgs,
  AtlasReportResult,
  AtlasSurfaceAuditArgs,
  AtlasSurfaceAuditResult,
  EngineeringManifestArgs,
  EngineeringManifestAuditResult,
  EngineeringPlanRequestArgs,
  EngineeringPlanToolResult,
  ReleasePipelineManifestArgs,
  ReleasePipelineAuditToolResult,
  OperationalReadinessManifestArgs,
  OperationalReadinessToolResult,
  SecurityPrivacyManifestArgs,
  SecurityPrivacyToolResult,
  SandboxManifestArgs,
  SandboxAdmissionToolResult,
  SandboxRuntimeManifestArgs,
  SandboxRuntimeToolResult,
  SecurityProgramManifestArgs,
  SecurityProgramToolResult,
  AdaptivePanelRunArgs,
  AdaptivePanelResult,
  PosteriorGateArgs,
  PosteriorGateResult,
  ToolCallPlan,
  ToolArguments,
  ToolsResponse,
  TraceOtelIngestArgs,
  TraceOtelIngestResult,
} from "./types.js";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_RESPONSE_BYTES = 20_000_000;
const DEFAULT_MAX_REQUEST_BYTES = 10_000_000;
const MAX_EVENT_PAGE = 1_000;
const MAX_REQUEST_ID_BYTES = 256;
const MAX_MISSION_WAIT_MS = 86_400_000;
const MAX_MISSION_POLL_INTERVAL_MS = 60_000;

/**
 * Fetch-based client for the bounded Prism API.
 *
 * The client keeps the raw REST/MCP envelope intact. A 2xx response means that the HTTP
 * boundary accepted the request; it does not mean a scientific, safety, or release claim was
 * accepted. Callers can inspect `mcp.result.isError`, `mcp.error`, and structured content, or use
 * `requireToolSuccess` when a workflow explicitly wants an exception for a domain refusal.
 */
export class ApiClient {
  readonly baseUrl: URL;
  readonly bearerToken?: string;
  readonly timeoutMs: number;
  readonly maxResponseBytes: number;
  readonly maxRequestBytes: number;

  private readonly fetchImpl: FetchLike;
  private readonly defaultHeaders: Readonly<Record<string, string>>;

  constructor(options: ApiClientOptions) {
    this.baseUrl = validateBaseUrl(options.baseUrl);
    this.bearerToken = options.bearerToken;
    this.timeoutMs = positiveInteger(options.timeoutMs ?? DEFAULT_TIMEOUT_MS, "timeoutMs");
    this.maxResponseBytes = positiveInteger(options.maxResponseBytes ?? DEFAULT_MAX_RESPONSE_BYTES, "maxResponseBytes");
    this.maxRequestBytes = positiveInteger(options.maxRequestBytes ?? DEFAULT_MAX_REQUEST_BYTES, "maxRequestBytes");
    this.fetchImpl = options.fetch ?? resolveFetch();
    this.defaultHeaders = validateHeaders(options.defaultHeaders ?? {});
    if (this.bearerToken !== undefined) validateBearerToken(this.bearerToken);
  }

  /** Issue one bounded JSON request. Only JSON object responses are accepted. */
  async request<T extends JsonObject = JsonObject>(
    method: HttpMethod,
    path: string,
    payload?: JsonObject,
    options: ClientRequestOptions = {},
  ): Promise<T> {
    const response = await this.execute(method, path, payload, options);
    const text = await readResponseText(response, this.maxResponseBytes);
    const parsed = parseJsonObject(text);
    if (!response.ok) {
      throw new ApiError(response.status, parsed as ApiErrorBody, response.headers.get("x-request-id") ?? undefined);
    }
    return parsed as T;
  }

  /** Issue a bounded request whose response is intentionally text, such as an SSE snapshot. */
  async requestText(
    method: HttpMethod,
    path: string,
    options: ClientRequestOptions = {},
  ): Promise<{ response: Response; text: string }> {
    const response = await this.execute(method, path, undefined, options);
    const text = await readResponseText(response, this.maxResponseBytes);
    if (!response.ok) {
      let payload: JsonValue = text;
      try {
        payload = JSON.parse(text) as JsonValue;
      } catch {
        // Preserve a non-JSON gateway response as the error payload.
      }
      throw new ApiError(response.status, payload, response.headers.get("x-request-id") ?? undefined);
    }
    return { response, text };
  }

  async health(options?: ClientRequestOptions): Promise<HealthResponse> {
    return this.request<HealthResponse>("GET", "/healthz", undefined, options);
  }

  async ready(options?: ClientRequestOptions): Promise<HealthResponse> {
    return this.request<HealthResponse>("GET", "/readyz", undefined, options);
  }

  async capabilities(options?: ClientRequestOptions): Promise<CapabilitiesResponse> {
    return this.request<CapabilitiesResponse>("GET", "/v1/capabilities", undefined, options);
  }

  /** Read one deterministic, digest-bound workflow template for every capability group. */
  async domainWorkflowCatalogueQuery(options?: ClientRequestOptions): Promise<DomainWorkflowCatalogueResult> {
    return this.request<DomainWorkflowCatalogueResult>("GET", "/v1/domain-workflows", undefined, options);
  }

  /** Build and preflight an execution-disabled scaffold for one capability group. */
  async domainWorkflowScaffoldQuery(
    args: DomainWorkflowScaffoldArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowScaffoldResult> {
    if (!isObject(args)) throw new ArgumentError("domain workflow scaffold arguments must be an object");
    if (typeof args.workflow_id !== "string" || args.workflow_id.trim().length === 0) throw new ArgumentError("workflow_id must be a non-empty string");
    if (typeof args.mission_id !== "string" || args.mission_id.trim().length === 0) throw new ArgumentError("mission_id must be a non-empty string");
    if (typeof args.goal !== "string" || args.goal.trim().length === 0) throw new ArgumentError("goal must be a non-empty string");
    if (args.tools !== undefined && (!Array.isArray(args.tools) || args.tools.length > 128 || args.tools.some((tool) => typeof tool !== "string" || tool.trim().length === 0))) {
      throw new ArgumentError("tools must contain at most 128 non-empty strings");
    }
    if (args.arguments !== undefined && !isObject(args.arguments)) throw new ArgumentError("arguments must be an object");
    return this.request<DomainWorkflowScaffoldResult>("POST", "/v1/domain-workflows/scaffold", args, options);
  }

  /** Instantiate and authoritative-preflight a group-scoped mission without dispatch. */
  async domainWorkflowInstantiateQuery(
    args: DomainWorkflowInstantiateArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowInstantiateResult> {
    if (!isObject(args)) throw new ArgumentError("domain workflow arguments must be an object");
    if (typeof args.workflow_id !== "string" || args.workflow_id.trim().length === 0) throw new ArgumentError("workflow_id must be a non-empty string");
    if (typeof args.mission_id !== "string" || args.mission_id.trim().length === 0) throw new ArgumentError("mission_id must be a non-empty string");
    if (typeof args.goal !== "string" || args.goal.trim().length === 0) throw new ArgumentError("goal must be a non-empty string");
    if (!Array.isArray(args.steps) || args.steps.length < 1 || args.steps.length > 128) throw new ArgumentError("steps must contain 1..=128 items");
    return this.request<DomainWorkflowInstantiateResult>("POST", "/v1/domain-workflows/instantiate", args, options);
  }

  /** Reconcile retained mission evidence against an instantiated workflow without dispatch. */
  async domainWorkflowReconcileQuery(
    args: DomainWorkflowReconcileArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowReconcileResult> {
    if (!isObject(args) || !isObject(args.instantiation)) {
      throw new ArgumentError("workflow reconciliation requires an instantiation object");
    }
    if (!isObject(args.mission_report) && !isObject(args.evidence_bundle)) {
      throw new ArgumentError("workflow reconciliation requires mission_report or evidence_bundle");
    }
    return this.request<DomainWorkflowReconcileResult>("POST", "/v1/domain-workflows/reconcile", args, options);
  }

  /** Import one digest-bound reconciliation report into the durable audit surface. */
  async domainWorkflowReconciliationImport(
    args: DomainWorkflowReconciliationImportArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowReconciliationImportResult> {
    if (!isObject(args) || !isObject(args.record)) throw new ArgumentError("workflow reconciliation record must be a JSON object");
    return this.request<DomainWorkflowReconciliationImportResult>("POST", "/v1/domain-workflows/reconciliations", args, options);
  }

  /** Query retained reconciliation index rows without re-executing or re-evaluating a mission. */
  async domainWorkflowReconciliationQuery(
    args: DomainWorkflowReconciliationQueryOptions = {},
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowReconciliationQueryResult> {
    if (!isObject(args)) throw new ArgumentError("workflow reconciliation query arguments must be a JSON object");
    for (const [name, value] of [["mission_id", args.mission_id], ["workflow_id", args.workflow_id], ["mission_plan_digest", args.mission_plan_digest], ["completion_status", args.completion_status], ["after", args.after]] as const) {
      if (value !== undefined && typeof value !== "string") throw new ArgumentError(`${name} must be a string`);
    }
    if (args.include_records !== undefined && typeof args.include_records !== "boolean") throw new ArgumentError("include_records must be a boolean");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    const query = new URLSearchParams({ limit: String(maxItems), include_records: String(args.include_records ?? false) });
    for (const [name, value] of [["mission_id", args.mission_id], ["workflow_id", args.workflow_id], ["mission_plan_digest", args.mission_plan_digest], ["completion_status", args.completion_status], ["after", args.after]] as const) {
      if (value !== undefined) query.set(name, value);
    }
    return this.request<DomainWorkflowReconciliationQueryResult>("GET", `/v1/domain-workflows/reconciliations?${query.toString()}`, undefined, options);
  }

  /** Fetch one retained reconciliation report by its SHA-256 content digest. */
  async domainWorkflowReconciliationGet(
    reconciliationDigest: string,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowReconciliationGetResult> {
    const digest = pathSegment(reconciliationDigest, "reconciliation digest");
    return this.request<DomainWorkflowReconciliationGetResult>("GET", `/v1/domain-workflows/reconciliations/${encodeURIComponent(digest)}`, undefined, options);
  }

  /** Project and index one caller-supplied report after checking catalogue membership. */
  async domainReportProject(
    args: DomainReportProjectArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainReportProjectResult> {
    if (!isObject(args)) throw new ArgumentError("domain report arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) {
      throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    }
    if (!isObject(args.report) || !isObject(args.claim_posture)) throw new ArgumentError("report and claim_posture must be objects");
    if (args.claim_posture.status === undefined || !["observed", "derived", "review_required", "refused", "not_applicable"].includes(args.claim_posture.status)) throw new ArgumentError("claim_posture.status is invalid");
    if (!Array.isArray(args.claim_posture.does_not_claim) || args.claim_posture.does_not_claim.length < 1 || args.claim_posture.does_not_claim.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("claim_posture.does_not_claim must be non-empty");
    if (args.source_plan_digest !== undefined && args.source_plan_digest !== null && (typeof args.source_plan_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.source_plan_digest))) throw new ArgumentError("source_plan_digest must be a lowercase SHA-256 digest or null");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    return this.request<DomainReportProjectResult>("POST", "/v1/domain-reports", args, options);
  }

  /** Audit retained structured report projections by capability group. */
  async domainReportCoverage(
    args: DomainReportCoverageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<DomainReportCoverageResult> {
    if (!isObject(args)) throw new ArgumentError("domain report coverage arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["domain", args.domain]] as const) {
      if (value !== undefined && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const maxGroups = args.max_groups ?? 64;
    if (!Number.isSafeInteger(maxGroups) || maxGroups < 1 || maxGroups > 128) throw new ArgumentError("max_groups must be 1..=128");
    if (args.include_report_digests !== undefined && typeof args.include_report_digests !== "boolean") throw new ArgumentError("include_report_digests must be a boolean");
    const query = new URLSearchParams({ max_groups: String(maxGroups), include_report_digests: String(args.include_report_digests ?? false) });
    if (args.group_id !== undefined) query.set("group_id", args.group_id);
    if (args.domain !== undefined) query.set("domain", args.domain);
    return this.request<DomainReportCoverageResult>("GET", `/v1/domain-reports/coverage?${query.toString()}`, undefined, options);
  }

  /** Invoke the same domain-report contract through the REST tool dispatcher. */
  async domainReportProjectTool(
    args: DomainReportProjectArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainReportProjectResult>> {
    return this.callTool<DomainReportProjectResult>("domain_report_project", args, options);
  }

  /** Invoke the coverage diagnostic through the REST tool dispatcher. */
  async domainReportCoverageTool(
    args: DomainReportCoverageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainReportCoverageResult>> {
    return this.callTool<DomainReportCoverageResult>("domain_report_project", { ...args, operation: "coverage" }, options);
  }

  /** Join exact domain reports into a digest-addressed, review-required traceability artifact. */
  async domainEvidenceHarmonize(
    args: DomainEvidenceHarmonizeArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainEvidenceHarmonizationResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence arguments must be an object");
    if (typeof args.subject_id !== "string" || args.subject_id.trim().length === 0) throw new ArgumentError("subject_id must be a non-empty string");
    if (!isObject(args.claim) || typeof args.claim.id !== "string" || args.claim.id.trim().length === 0) throw new ArgumentError("claim.id must be a non-empty string");
    if (!Array.isArray(args.reports) || args.reports.length < 1 || args.reports.length > 64 || args.reports.some((report) => !isObject(report))) throw new ArgumentError("reports must contain 1..=64 objects");
    if (!Array.isArray(args.links) || args.links.length < 1 || args.links.length > 256 || args.links.some((link) => !isObject(link))) throw new ArgumentError("links must contain 1..=256 objects");
    for (const [index, link] of args.links.entries()) {
      if (!Number.isSafeInteger(link.report_index) || link.report_index < 0 || link.report_index >= args.reports.length) throw new ArgumentError(`links[${index}].report_index is out of range`);
      if (!["supports", "qualifies", "contradicts", "context"].includes(link.role)) throw new ArgumentError(`links[${index}].role is invalid`);
      if (link.note !== undefined && typeof link.note !== "string") throw new ArgumentError(`links[${index}].note must be a string`);
      if ((link.role === "qualifies" || link.role === "contradicts") && (!link.note || link.note.trim().length === 0)) throw new ArgumentError(`links[${index}].note is required for ${link.role}`);
      if (link.report_digest !== undefined && !/^[0-9a-fA-F]{64}$/.test(link.report_digest)) throw new ArgumentError(`links[${index}].report_digest must be a SHA-256 digest`);
    }
    for (const [name, value] of [["required_group_ids", args.required_group_ids], ["required_domains", args.required_domains]] as const) {
      if (value !== undefined && (!Array.isArray(value) || value.length > 64 || value.some((item) => typeof item !== "string" || item.trim().length === 0))) throw new ArgumentError(`${name} must contain at most 64 non-empty strings`);
    }
    return this.request<DomainEvidenceHarmonizationResult>("POST", "/v1/domain-evidence/harmonize", args, options);
  }

  /** Invoke the harmonizer through the REST tool dispatcher. */
  async domainEvidenceHarmonizeTool(
    args: DomainEvidenceHarmonizeArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceHarmonizationResult>> {
    return this.callTool<DomainEvidenceHarmonizationResult>("domain_evidence_harmonize", args, options);
  }

  /** Normalize one supplied raw source-tool envelope into an exact-digest intake artifact. */
  async domainEvidenceIntake(
    args: DomainEvidenceIntakeArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainEvidenceIntakeResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence intake arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (!Object.prototype.hasOwnProperty.call(args, "response")) throw new ArgumentError("response is required");
    if (!("observed" === args.outcome || "partial" === args.outcome || "refused" === args.outcome || "error" === args.outcome || "unknown" === args.outcome)) throw new ArgumentError("outcome is invalid");
    if (!isObject(args.claim_posture)) throw new ArgumentError("claim_posture must be an object");
    if (!(["observed", "derived", "review_required", "refused", "not_applicable"] as const).includes(args.claim_posture.status)) throw new ArgumentError("claim_posture.status is invalid");
    if (!Array.isArray(args.claim_posture.does_not_claim) || args.claim_posture.does_not_claim.length < 1 || args.claim_posture.does_not_claim.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("claim_posture.does_not_claim must be non-empty");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    return this.request<DomainEvidenceIntakeResult>("POST", "/v1/domain-evidence/intake", args, options);
  }

  /** Invoke raw envelope intake through the REST tool dispatcher. */
  async domainEvidenceIntakeTool(
    args: DomainEvidenceIntakeArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceIntakeResult>> {
    return this.callTool<DomainEvidenceIntakeResult>("domain_evidence_intake", args, options);
  }

  /** Audit retained raw-intake envelopes by authoritative capability group. */
  async domainEvidenceCoverage(
    args: DomainEvidenceIntakeCoverageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<DomainEvidenceIntakeCoverageResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence intake coverage arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["domain", args.domain]] as const) {
      if (value !== undefined && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const maxGroups = args.max_groups ?? 64;
    if (!Number.isSafeInteger(maxGroups) || maxGroups < 1 || maxGroups > 128) throw new ArgumentError("max_groups must be 1..=128");
    if (args.include_intake_digests !== undefined && typeof args.include_intake_digests !== "boolean") throw new ArgumentError("include_intake_digests must be a boolean");
    const query = new URLSearchParams({ max_groups: String(maxGroups), include_intake_digests: String(args.include_intake_digests ?? false) });
    if (args.group_id !== undefined) query.set("group_id", args.group_id);
    if (args.domain !== undefined) query.set("domain", args.domain);
    return this.request<DomainEvidenceIntakeCoverageResult>("GET", `/v1/domain-evidence/coverage?${query.toString()}`, undefined, options);
  }

  /** Invoke intake coverage through the REST tool dispatcher. */
  async domainEvidenceCoverageTool(
    args: DomainEvidenceIntakeCoverageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceIntakeCoverageResult>> {
    return this.callTool<DomainEvidenceIntakeCoverageResult>("domain_evidence_coverage", args, options);
  }

  /** Plan and index a non-fetching external evidence connector boundary. */
  async domainEvidenceSourcePlan(
    args: DomainEvidenceSourcePlanArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainEvidenceSourcePlanResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence source plan arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["locator", args.locator]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (!("literature" === args.connector_kind || "clinical_trial" === args.connector_kind || "fhir" === args.connector_kind || "object_store" === args.connector_kind || "file" === args.connector_kind || "provider_api" === args.connector_kind || "generic_http" === args.connector_kind)) throw new ArgumentError("connector_kind is invalid");
    if (!("uri" === args.locator_kind || "path" === args.locator_kind || "opaque" === args.locator_kind)) throw new ArgumentError("locator_kind is invalid");
    if (!("reference_only" === args.retrieval_mode || "metadata_only" === args.retrieval_mode || "content" === args.retrieval_mode)) throw new ArgumentError("retrieval_mode is invalid");
    if (args.source_tool !== undefined && args.source_tool !== null && (typeof args.source_tool !== "string" || args.source_tool.trim().length === 0)) throw new ArgumentError("source_tool must be a non-empty string or null");
    if (args.expected_content_digest !== undefined && args.expected_content_digest !== null && (typeof args.expected_content_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.expected_content_digest))) throw new ArgumentError("expected_content_digest must be a lowercase SHA-256 digest or null");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (!Array.isArray(args.does_not_claim) || args.does_not_claim.length < 1 || args.does_not_claim.length > 64 || args.does_not_claim.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("does_not_claim must contain 1..=64 non-empty strings");
    if (args.retrieval_policy !== undefined && !isObject(args.retrieval_policy)) throw new ArgumentError("retrieval_policy must be an object");
    return this.request<DomainEvidenceSourcePlanResult>("POST", "/v1/domain-evidence/sources", args, options);
  }

  /** Invoke external source planning through the REST tool dispatcher. */
  async domainEvidenceSourcePlanTool(
    args: DomainEvidenceSourcePlanArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceSourcePlanResult>> {
    return this.callTool<DomainEvidenceSourcePlanResult>("domain_evidence_source_plan", args, options);
  }

  /** Inspect all restart, secret, and external-effect boundaries in one operator matrix. */
  async recoveryMatrix(options?: ClientRequestOptions): Promise<RecoveryMatrix> {
    return this.request<RecoveryMatrix>("GET", "/v1/recovery", undefined, options);
  }

  /** Inspect one bounded operator snapshot across events, missions, persistence, and recovery. */
  async operationsSnapshot(
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<OperationsSnapshot> {
    cursor(after, "after");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) {
      throw new ArgumentError("limit must be 1..=256");
    }
    return this.request<OperationsSnapshot>(
      "GET",
      `/v1/operations/snapshot?after=${after}&limit=${limit}`,
      undefined,
      options,
    );
  }

  /** Build a content-addressed, non-executing route handoff from domain coverage evidence. */
  async operationsHandoff(
    args: OperationsHandoffArgs = {},
    options?: ClientRequestOptions,
  ): Promise<OperationsHandoff> {
    if (!isObject(args)) throw new ArgumentError("operations handoff arguments must be an object");
    if (args.goal !== undefined && (typeof args.goal !== "string" || args.goal.trim().length === 0 || args.goal.length > 1024)) {
      throw new ArgumentError("goal must be a non-empty visible string of at most 1024 characters");
    }
    for (const name of ["domains", "group_ids"] as const) {
      const values = args[name];
      if (values !== undefined && (!Array.isArray(values) || values.length > 64 || values.some((value) => typeof value !== "string" || value.trim().length === 0 || value.length > 128))) {
        throw new ArgumentError(`${name} must contain at most 64 visible strings of at most 128 characters`);
      }
    }
    if (args.include_complete !== undefined && typeof args.include_complete !== "boolean") {
      throw new ArgumentError("include_complete must be a boolean");
    }
    if (args.max_groups !== undefined && (!Number.isSafeInteger(args.max_groups) || args.max_groups < 1 || args.max_groups > 64)) {
      throw new ArgumentError("max_groups must be 1..=64");
    }
    const normalized: OperationsHandoffArgs = { ...args };
    if (normalized.domains !== undefined) normalized.domains = [...new Set(normalized.domains)].sort();
    if (normalized.group_ids !== undefined) normalized.group_ids = [...new Set(normalized.group_ids)].sort();
    return this.request<OperationsHandoff>("POST", "/v1/operations/handoff", normalized, options);
  }

  /** Read bounded local tool activity grouped by domain without claiming readiness. */
  async operationsDomainActivity(
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<OperationsDomainActivity> {
    cursor(after, "after");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) {
      throw new ArgumentError("limit must be 1..=256");
    }
    return this.request<OperationsDomainActivity>(
      "GET",
      `/v1/operations/domains?after=${after}&limit=${limit}`,
      undefined,
      options,
    );
  }

  /** Read separate per-domain evidence gates without inferring readiness. */
  async operationsDomainGates(
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<OperationsDomainGates> {
    cursor(after, "after");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) {
      throw new ArgumentError("limit must be 1..=256");
    }
    return this.request<OperationsDomainGates>(
      "GET",
      `/v1/operations/gates?after=${after}&limit=${limit}`,
      undefined,
      options,
    );
  }

  /** Replay durable operations gate reviews with explicit cursor and retention evidence. */
  async operationsGateReviews(
    after = 0,
    limit = 100,
    reviewId?: string,
    options?: ClientRequestOptions,
  ): Promise<OperationsGateReviews> {
    cursor(after, "after");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) {
      throw new ArgumentError("limit must be 1..=256");
    }
    if (reviewId !== undefined) validateReviewId(reviewId);
    const suffix = reviewId === undefined ? "" : `&review_id=${encodeURIComponent(reviewId)}`;
    return this.request<OperationsGateReviews>(
      "GET",
      `/v1/operations/gate-reviews?after=${after}&limit=${limit}${suffix}`,
      undefined,
      options,
    );
  }

  /** Persist a current operations gate review before binding it to executable mission arguments. */
  async createOperationsGateReview(
    args: OperationsGateReviewRequest,
    options?: ClientRequestOptions,
  ): Promise<OperationsGateReview> {
    if (!isObject(args)) throw new ArgumentError("operations gate review arguments must be an object");
    validateReviewId(args.gate_digest);
    visible(args.reviewer, "reviewer", 256);
    visible(args.rationale, "rationale", 2048);
    if (!Array.isArray(args.group_ids) || args.group_ids.length < 1 || args.group_ids.length > 64 || args.group_ids.some((value) => typeof value !== "string" || value.trim().length === 0 || value.length > 128)) {
      throw new ArgumentError("group_ids must contain 1..=64 visible strings");
    }
    if (!isObject(args.accepted_gates)) throw new ArgumentError("accepted_gates must be an object");
    return this.request<OperationsGateReview>("POST", "/v1/operations/gate-reviews", args, options);
  }

  async tools(options?: ClientRequestOptions): Promise<ToolsResponse["tools"]> {
    const response = await this.request<ToolsResponse>("GET", "/v1/tools", undefined, options);
    if (!Array.isArray(response.tools) || response.tools.some((tool) => !isObject(tool) || typeof tool.name !== "string")) {
      throw new ProtocolError("HTTP API tools response has no object array with names");
    }
    return response.tools;
  }

  async metrics(options?: ClientRequestOptions): Promise<{ ok: boolean; metrics: EventMetrics }> {
    return this.request("GET", "/v1/metrics", undefined, options);
  }

  /** Inspect whether restart-aware event cursor snapshots are enabled and within bounds. */
  async eventPersistence(options?: ClientRequestOptions): Promise<EventPersistenceStatus> {
    return this.request<EventPersistenceStatus>("GET", "/v1/events/persistence", undefined, options);
  }

  /** Force a bounded event/outbox checkpoint; signing secrets remain memory-only. */
  async flushEventPersistence(options?: ClientRequestOptions): Promise<EventPersistenceStatus> {
    return this.request<EventPersistenceStatus>("POST", "/v1/events/persistence/flush", {}, options);
  }

  /** Call any currently advertised or future MCP tool without losing its refusal envelope. */
  async callTool<T extends JsonValue = JsonValue>(
    name: string,
    arguments_: ToolArguments = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<T>> {
    const tool = pathSegment(name, "tool name");
    if (!isObject(arguments_)) throw new ArgumentError("tool arguments must be a JSON object");
    return this.request<RestToolResponse<T>>("POST", `/v1/tools/${encodeURIComponent(tool)}`, arguments_, options);
  }

  /** Snapshot the authoritative live tool catalogue and bind it to a SHA-256 digest. */
  async toolCatalogue(options?: ClientRequestOptions): Promise<ToolCatalogue> {
    return ToolCatalogue.fromDefinitions(await this.tools(options));
  }

  /** Produce a no-side-effect, transport-shape-checked plan for any advertised tool. */
  async planTool(
    name: string,
    arguments_: ToolArguments = {},
    catalogue?: ToolCatalogue,
  ): Promise<ToolCallPlan> {
    const snapshot = catalogue ?? await this.toolCatalogue();
    return snapshot.plan(name, arguments_);
  }

  /** Execute a checked plan while preserving the raw REST/MCP refusal envelope. */
  async toolChecked<T extends JsonValue = JsonValue>(
    name: string,
    arguments_: ToolArguments = {},
    options?: ClientRequestOptions,
    catalogue?: ToolCatalogue,
  ): Promise<RestToolResponse<T>> {
    const plan = await this.planTool(name, arguments_, catalogue);
    return this.callTool<T>(plan.tool, plan.arguments, options);
  }

  /** Convert an explicit domain refusal into a typed exception; successful results remain raw. */
  requireToolSuccess<T extends JsonValue>(response: RestToolResponse<T>): RestToolResponse<T> {
    if (!response.ok || response.mcp.error || response.mcp.result?.isError) {
      throw new ToolRefusalError(response.tool, response);
    }
    return response;
  }

  async metricsProfileAudit(args: MetricsProfileAuditArgs, options?: ClientRequestOptions) {
    return this.callTool("metrics_profile_audit", args, options);
  }

  async metricsAnalyticsAudit(args: MetricsAnalyticsAuditArgs, options?: ClientRequestOptions) {
    return this.callTool("metrics_analytics_audit", args, options);
  }

  async bioCapabilityEvidenceAudit(args: BioCapabilityEvidenceAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioCapabilityEvidenceAuditResult>> {
    return this.callTool<BioCapabilityEvidenceAuditResult>("biocapability_evidence_audit", args, options);
  }

  async bioAtlasPublicationAudit(args: BioAtlasPublicationAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioAtlasPublicationAuditResult>> {
    return this.callTool<BioAtlasPublicationAuditResult>("bioatlas_publication_audit", args, options);
  }

  async traceOtelIngest(args: TraceOtelIngestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<TraceOtelIngestResult>> {
    return this.callTool<TraceOtelIngestResult>("trace_otel_ingest", args, options);
  }

  async repositoryCatalog(args: RepositoryCatalogArgs = {}, options?: ClientRequestOptions) {
    return this.callTool("repository_catalog", args, options);
  }

  async repositoryBundle(args: RepositoryBundleArgs, options?: ClientRequestOptions) {
    return this.callTool("repository_bundle", args, options);
  }

  async repositoryImpact(args: RepositoryImpactArgs, options?: ClientRequestOptions) {
    return this.callTool("repository_impact", args, options);
  }

  async telemetryProject(args: TelemetryProjectArgs, options?: ClientRequestOptions): Promise<RestToolResponse<TelemetryProjectionResult>> {
    return this.callTool<TelemetryProjectionResult>("telemetry_project", args, options);
  }

  async ledgerIngest(args: LedgerIngestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LedgerIngestResult>> {
    return this.callTool<LedgerIngestResult>("ledger_ingest", args, options);
  }

  async qualityGateRun(args: QualityGateRunArgs, options?: ClientRequestOptions): Promise<RestToolResponse<QualityGateRunResult>> {
    return this.callTool<QualityGateRunResult>("quality_gate_run", args, options);
  }

  async atlasReport(args: AtlasReportArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AtlasReportResult>> {
    return this.callTool<AtlasReportResult>("atlas_report", args, options);
  }

  async atlasSurfaceAudit(args: AtlasSurfaceAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AtlasSurfaceAuditResult>> {
    return this.callTool<AtlasSurfaceAuditResult>("atlas_surface_audit", args, options);
  }

  async engineeringManifestAudit(args: EngineeringManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EngineeringManifestAuditResult>> {
    return this.callTool<EngineeringManifestAuditResult>("engineering_manifest_audit", args, options);
  }

  async engineeringExecutionPlan(args: EngineeringPlanRequestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EngineeringPlanToolResult>> {
    return this.callTool<EngineeringPlanToolResult>("engineering_execution_plan", args, options);
  }

  async releasePipelineAudit(args: ReleasePipelineManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ReleasePipelineAuditToolResult>> {
    return this.callTool<ReleasePipelineAuditToolResult>("release_pipeline_audit", args, options);
  }

  async operationalReadinessAudit(args: OperationalReadinessManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OperationalReadinessToolResult>> {
    return this.callTool<OperationalReadinessToolResult>("operational_readiness_audit", args, options);
  }

  async securityPrivacyAudit(args: SecurityPrivacyManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<SecurityPrivacyToolResult>> {
    return this.callTool<SecurityPrivacyToolResult>("security_privacy_audit", args, options);
  }

  async sandboxAdmissionAudit(args: SandboxManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<SandboxAdmissionToolResult>> {
    return this.callTool<SandboxAdmissionToolResult>("sandbox_admission_audit", args, options);
  }

  async sandboxRuntimeSimulate(args: SandboxRuntimeManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<SandboxRuntimeToolResult>> {
    return this.callTool<SandboxRuntimeToolResult>("sandbox_runtime_simulate", args, options);
  }

  async securityProgramAudit(args: SecurityProgramManifestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<SecurityProgramToolResult>> {
    return this.callTool<SecurityProgramToolResult>("security_program_audit", args, options);
  }

  async adaptivePanel(args: AdaptivePanelRunArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdaptivePanelResult>> {
    return this.callTool<AdaptivePanelResult>("adaptive_panel", args, options);
  }

  async posteriorGate(args: PosteriorGateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<PosteriorGateResult>> {
    return this.callTool<PosteriorGateResult>("posterior_gate", args, options);
  }

  async developerDeliveryAudit(args: DeveloperDeliveryAuditArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<DeveloperDeliveryAuditResult>> {
    return this.callTool<DeveloperDeliveryAuditResult>("developer_delivery_audit", args, options);
  }

  async developerDeliveryReceipt(args: DeveloperDeliveryReceiptArgs, options?: ClientRequestOptions): Promise<RestToolResponse<DeveloperDeliveryReceiptResult>> {
    return this.callTool<DeveloperDeliveryReceiptResult>("developer_delivery_receipt", args, options);
  }

  async developerDeliveryReceiptVerify(args: DeveloperDeliveryReceiptVerificationArgs, options?: ClientRequestOptions): Promise<RestToolResponse<DeveloperDeliveryReceiptVerificationResult>> {
    return this.callTool<DeveloperDeliveryReceiptVerificationResult>("developer_delivery_receipt_verify", args, options);
  }

  async developerPlatformStatus(args: DeveloperPlatformStatusArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<DeveloperPlatformStatusResult>> {
    return this.callTool<DeveloperPlatformStatusResult>("developer_platform_status", args, options);
  }

  async tokenContextPlan(args: TokenContextPlanArgs, options?: ClientRequestOptions): Promise<RestToolResponse<TokenContextPlanningResult>> {
    return this.callTool<TokenContextPlanningResult>("token_context_plan", args, options);
  }

  async weavelangCompile(args: WeaveLangCompileArgs, options?: ClientRequestOptions): Promise<RestToolResponse<WeaveLangCompileResult>> {
    return this.callTool<WeaveLangCompileResult>("weavelang_compile", args, options);
  }

  async epistemicVoi(args: EpistemicVoiArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EpistemicVoiResult>> {
    return this.callTool<EpistemicVoiResult>("epistemic_voi", args, options);
  }

  async epistemicContextAudit(args: EpistemicContextAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EpistemicContextAuditResult>> {
    return this.callTool<EpistemicContextAuditResult>("epistemic_context_audit", args, options);
  }

  async epistemicSelectionAudit(args: EpistemicSelectionAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EpistemicSelectionAuditResult>> {
    return this.callTool<EpistemicSelectionAuditResult>("epistemic_selection_audit", args, options);
  }

  async benchmarkTraceAnalyze(args: BenchmarkTraceAnalyzeArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkTraceAnalysisResult>> {
    return this.callTool<BenchmarkTraceAnalysisResult>("benchmark_trace_analyze", args, options);
  }

  async benchmarkDecisionAudit(args: BenchmarkDecisionAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkDecisionAuditResult>> {
    return this.callTool<BenchmarkDecisionAuditResult>("benchmark_decision_audit", args, options);
  }

  async benchmarkIntegrityAudit(args: BenchmarkIntegrityAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkIntegrityAuditResult>> {
    return this.callTool<BenchmarkIntegrityAuditResult>("benchmark_integrity_audit", args, options);
  }

  async benchmarkCounterfactualCheck(args: BenchmarkCounterfactualCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkCounterfactualCheckResult>> {
    return this.callTool<BenchmarkCounterfactualCheckResult>("benchmark_counterfactual_check", args, options);
  }

  async benchmarkOracleReview(args: BenchmarkOracleReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkOracleReviewResult>> {
    return this.callTool<BenchmarkOracleReviewResult>("benchmark_oracle_review", args, options);
  }

  async benchmarkCompile(args: BenchmarkCompileArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkCompileResult>> {
    return this.callTool<BenchmarkCompileResult>("benchmark_compile", args, options);
  }

  async benchmarkCompileReview(args: BenchmarkCompileReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkCompileReviewResult>> {
    return this.callTool<BenchmarkCompileReviewResult>("benchmark_compile_review", args, options);
  }

  async packCoverageAudit(args: PackCoverageAuditArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<PackCoverageAuditResult>> {
    return this.callTool<PackCoverageAuditResult>("pack_coverage_audit", args, options);
  }

  async packReleaseAudit(args: PackReleaseAuditArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<PackReleaseAuditResult>> {
    return this.callTool<PackReleaseAuditResult>("pack_release_audit", args, options);
  }

  async foundationContractCheck(args: FoundationContractCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<FoundationContractCheckResult>> {
    return this.callTool<FoundationContractCheckResult>("foundation_contract_check", args, options);
  }

  async packCatalogue(args: PackCatalogueArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<PackCatalogueResult>> {
    return this.callTool<PackCatalogueResult>("pack_catalogue", args, options);
  }

  async packHealthAssess(args: PackHealthAssessArgs, options?: ClientRequestOptions): Promise<RestToolResponse<PackHealthAssessmentResult>> {
    return this.callTool<PackHealthAssessmentResult>("pack_health_assess", args, options);
  }

  async securityRedteamSimulate(args: SecurityRedteamSimulateArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<SecurityRedteamResult>> {
    return this.callTool<SecurityRedteamResult>("security_redteam_simulate", args, options);
  }

  async worldGenerate(args: WorldGenerateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<WorldGenerateResult>> {
    return this.callTool<WorldGenerateResult>("world_generate", args, options);
  }

  async factoryLifecycleSimulate(args: FactoryLifecycleSimulateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<FactoryLifecycleResult>> {
    return this.callTool<FactoryLifecycleResult>("factory_lifecycle_simulate", args, options);
  }

  async storageLifecycleSimulate(args: StorageLifecycleSimulateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<StorageLifecycleResult>> {
    return this.callTool<StorageLifecycleResult>("storage_lifecycle_simulate", args, options);
  }

  async registryLifecycleSimulate(args: RegistryLifecycleSimulateArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<RegistryLifecycleResult>> {
    return this.callTool<RegistryLifecycleResult>("registry_lifecycle_simulate", args, options);
  }

  async cacheInvalidationSimulate(args: CacheInvalidationSimulateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CacheInvalidationResult>> {
    return this.callTool<CacheInvalidationResult>("cache_invalidation_simulate", args, options);
  }

  async hubDisclosureReview(args: HubDisclosureReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubDisclosureReviewResult>> {
    return this.callTool<HubDisclosureReviewResult>("hub_disclosure_review", args, options);
  }

  async hubCardRender(args: HubCardRenderArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubCardRenderResult>> {
    return this.callTool<HubCardRenderResult>("hub_card_render", args, options);
  }

  async hubLeaderboardRender(args: HubLeaderboardRenderArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubLeaderboardRenderResult>> {
    return this.callTool<HubLeaderboardRenderResult>("hub_leaderboard_render", args, options);
  }

  async hubSubmissionReview(args: HubSubmissionReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubSubmissionReviewResult>> {
    return this.callTool<HubSubmissionReviewResult>("hub_submission_review", args, options);
  }

  async bioatlasPublicationAudit(args: BioAtlasPublicationAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioAtlasPublicationAuditResult>> {
    return this.callTool<BioAtlasPublicationAuditResult>("bioatlas_publication_audit", args, options);
  }

  async developerWorkbench(args: DeveloperWorkbenchArgs, options?: ClientRequestOptions) {
    return this.callTool("developer_workbench", args, options);
  }

  async ciExecutionEvidenceAudit(args: CiExecutionEvidenceArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CiExecutionEvidenceResult>> {
    return this.callTool<CiExecutionEvidenceResult>("ci_execution_evidence_audit", args, options);
  }

  async ciProviderNormalize(args: CiProviderNormalizationArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CiProviderNormalizationResult>> {
    return this.callTool<CiProviderNormalizationResult>("ci_provider_normalize", args, options);
  }

  async ciProviderEvidenceAudit(args: CiProviderEvidenceArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CiProviderEvidenceResult>> {
    return this.callTool<CiProviderEvidenceResult>("ci_provider_evidence_audit", args, options);
  }

  async executionProvenanceAudit(args: ExecutionProvenanceArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ExecutionProvenanceResult>> {
    return this.callTool<ExecutionProvenanceResult>("execution_provenance_audit", args, options);
  }

  async capabilityDiscover(args: CapabilityDiscoverArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityDiscoverResult>> {
    return this.callTool<CapabilityDiscoverResult>("capability_discover", args, options);
  }

  async domainWorkflowCatalogue(options?: ClientRequestOptions): Promise<RestToolResponse<DomainWorkflowCatalogueResult>> {
    return this.callTool<DomainWorkflowCatalogueResult>("domain_workflow_catalogue", {}, options);
  }

  async domainWorkflowScaffold(
    args: DomainWorkflowScaffoldArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowScaffoldResult>> {
    if (!isObject(args)) throw new ArgumentError("domain workflow scaffold arguments must be an object");
    if (typeof args.workflow_id !== "string" || args.workflow_id.trim().length === 0) throw new ArgumentError("workflow_id must be a non-empty string");
    if (typeof args.mission_id !== "string" || args.mission_id.trim().length === 0) throw new ArgumentError("mission_id must be a non-empty string");
    if (typeof args.goal !== "string" || args.goal.trim().length === 0) throw new ArgumentError("goal must be a non-empty string");
    if (args.tools !== undefined && (!Array.isArray(args.tools) || args.tools.length > 128 || args.tools.some((tool) => typeof tool !== "string" || tool.trim().length === 0))) {
      throw new ArgumentError("tools must contain at most 128 non-empty strings");
    }
    if (args.arguments !== undefined && !isObject(args.arguments)) throw new ArgumentError("arguments must be an object");
    return this.callTool<DomainWorkflowScaffoldResult>("domain_workflow_scaffold", args, options);
  }

  async domainWorkflowInstantiate(
    args: DomainWorkflowInstantiateArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowInstantiateResult>> {
    if (!isObject(args) || !Array.isArray(args.steps) || args.steps.length < 1 || args.steps.length > 128) {
      throw new ArgumentError("domain workflow instantiate requires 1..=128 step objects");
    }
    return this.callTool<DomainWorkflowInstantiateResult>("domain_workflow_instantiate", args, options);
  }

  async domainWorkflowReconcile(
    args: DomainWorkflowReconcileArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowReconcileResult>> {
    if (!isObject(args) || !isObject(args.instantiation)) {
      throw new ArgumentError("workflow reconciliation requires an instantiation object");
    }
    if (!isObject(args.mission_report) && !isObject(args.evidence_bundle)) {
      throw new ArgumentError("workflow reconciliation requires mission_report or evidence_bundle");
    }
    return this.callTool<DomainWorkflowReconcileResult>("domain_workflow_reconcile", args, options);
  }

  async domainWorkflowReconciliationImportTool(
    args: DomainWorkflowReconciliationImportArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowReconciliationImportResult>> {
    if (!isObject(args) || !isObject(args.record)) throw new ArgumentError("workflow reconciliation record must be a JSON object");
    return this.callTool<DomainWorkflowReconciliationImportResult>("domain_workflow_reconciliation_import", args, options);
  }

  async domainWorkflowReconciliationQueryTool(
    args: DomainWorkflowReconciliationQueryOptions = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowReconciliationQueryResult>> {
    if (!isObject(args)) throw new ArgumentError("workflow reconciliation query arguments must be a JSON object");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    return this.callTool<DomainWorkflowReconciliationQueryResult>("domain_workflow_reconciliation_query", { ...args, max_items: maxItems }, options);
  }

  async domainWorkflowReconciliationGetTool(
    reconciliationDigest: string,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowReconciliationGetResult>> {
    const digest = pathSegment(reconciliationDigest, "reconciliation digest");
    return this.callTool<DomainWorkflowReconciliationGetResult>("domain_workflow_reconciliation_get", { reconciliation_digest: digest }, options);
  }

  async missionEvaluatorDiscover(args: MissionEvaluatorDiscoverArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvaluatorDiscoverResult>> {
    return this.callTool<MissionEvaluatorDiscoverResult>("mission_evaluator_discover", args, options);
  }

  async missionEvaluatorReview(args: MissionEvaluatorReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvaluatorReviewResult>> {
    return this.callTool<MissionEvaluatorReviewResult>("mission_evaluator_review", args, options);
  }

  async missionEvaluatorReplay(args: MissionEvaluatorReplayArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvaluatorReplayResult>> {
    return this.callTool<MissionEvaluatorReplayResult>("mission_evaluator_replay", args, options);
  }

  async missionEvaluatorReplayCompare(args: MissionEvaluatorReplayCompareArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvaluatorReplayCompareResult>> {
    return this.callTool<MissionEvaluatorReplayCompareResult>("mission_evaluator_replay_compare", args, options);
  }

  async missionEvidenceBundleVerify(args: MissionEvidenceBundleVerifyArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvidenceBundleVerifyResult>> {
    return this.callTool<MissionEvidenceBundleVerifyResult>("mission_evidence_bundle_verify", args, options);
  }

  async missionEvidenceBundleImport(args: MissionEvidenceBundleImportArgs, options?: ClientRequestOptions): Promise<MissionEvidenceBundleImportResult> {
    if (!isObject(args) || !isObject(args.bundle)) throw new ArgumentError("bundle must be a JSON object");
    return this.request<MissionEvidenceBundleImportResult>("POST", "/v1/evidence-bundles", args, options);
  }

  async missionEvidenceBundleQuery(args: MissionEvidenceBundleQueryOptions = {}, options?: ClientRequestOptions): Promise<MissionEvidenceBundleQueryResult> {
    if (!isObject(args)) throw new ArgumentError("evidence bundle query arguments must be a JSON object");
    for (const [name, value] of [["mission_id", args.mission_id], ["domain", args.domain], ["after", args.after]] as const) {
      if (value !== undefined && typeof value !== "string") throw new ArgumentError(`${name} must be a string`);
    }
    if (args.include_bundles !== undefined && typeof args.include_bundles !== "boolean") throw new ArgumentError("include_bundles must be a boolean");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    const query = new URLSearchParams({ max_items: String(maxItems), include_bundles: String(args.include_bundles ?? false) });
    if (args.mission_id !== undefined) query.set("mission_id", args.mission_id);
    if (args.domain !== undefined) query.set("domain", args.domain);
    if (args.after !== undefined) query.set("after", args.after);
    return this.request<MissionEvidenceBundleQueryResult>("GET", `/v1/evidence-bundles?${query.toString()}`, undefined, options);
  }

  async missionEvidenceBundleGet(bundleDigest: string, options?: ClientRequestOptions): Promise<MissionEvidenceBundleGetResult> {
    const digest = pathSegment(bundleDigest, "bundle digest");
    return this.request<MissionEvidenceBundleGetResult>("GET", `/v1/evidence-bundles/${encodeURIComponent(digest)}`, undefined, options);
  }

  async missionEvidenceBundleImportTool(args: MissionEvidenceBundleImportArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvidenceBundleImportResult>> {
    if (!isObject(args) || !isObject(args.bundle)) throw new ArgumentError("bundle must be a JSON object");
    return this.callTool<MissionEvidenceBundleImportResult>("mission_evidence_bundle_import", args, options);
  }

  async missionEvidenceBundleQueryTool(args: MissionEvidenceBundleQueryOptions = {}, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvidenceBundleQueryResult>> {
    return this.callTool<MissionEvidenceBundleQueryResult>("mission_evidence_bundle_query", args, options);
  }

  async missionEvidenceBundleGetTool(bundleDigest: string, options?: ClientRequestOptions): Promise<RestToolResponse<MissionEvidenceBundleGetResult>> {
    const digest = pathSegment(bundleDigest, "bundle digest");
    return this.callTool<MissionEvidenceBundleGetResult>("mission_evidence_bundle_get", { bundle_digest: digest }, options);
  }

  async artifactRegistryAudit(
    args: JsonObject = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<JsonObject>> {
    if (!isObject(args)) throw new ArgumentError("artifact registry arguments must be an object");
    return this.callTool<JsonObject>("artifact_registry_audit", args, options);
  }

  async artifactRegister(
    args: ArtifactRegistrationArgs,
    options?: ClientRequestOptions,
  ): Promise<ArtifactRegistrationResult> {
    if (!isObject(args) || typeof args.kind !== "string" || typeof args.subject_id !== "string" || !("artifact" in args)) {
      throw new ArgumentError("artifact registration requires kind, subject_id, and artifact");
    }
    return this.request<ArtifactRegistrationResult>("POST", "/v1/artifacts", args, options);
  }

  async artifactQuery(
    args: ArtifactQueryOptions = {},
    options?: ClientRequestOptions,
  ): Promise<ArtifactQueryResult> {
    if (!isObject(args)) throw new ArgumentError("artifact query arguments must be an object");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    if (args.include_artifacts !== undefined && typeof args.include_artifacts !== "boolean") throw new ArgumentError("include_artifacts must be a boolean");
    const query = new URLSearchParams({ limit: String(maxItems), include_artifacts: String(args.include_artifacts ?? false) });
    for (const [name, value] of [["kind", args.kind], ["domain", args.domain], ["subject_id", args.subject_id], ["after", args.after]] as const) {
      if (value !== undefined) {
        if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
        query.set(name, value);
      }
    }
    return this.request<ArtifactQueryResult>("GET", `/v1/artifacts?${query.toString()}`, undefined, options);
  }

  async artifactGet(contentDigest: string, options?: ClientRequestOptions): Promise<ArtifactGetResult> {
    const digest = pathSegment(contentDigest, "artifact content digest");
    return this.request<ArtifactGetResult>("GET", `/v1/artifacts/${encodeURIComponent(digest)}`, undefined, options);
  }

  async artifactLineage(contentDigest: string, options?: ClientRequestOptions): Promise<ArtifactLineageResult> {
    const digest = pathSegment(contentDigest, "artifact content digest");
    return this.request<ArtifactLineageResult>("GET", `/v1/artifacts/${encodeURIComponent(digest)}/lineage`, undefined, options);
  }

  async artifactRegistryPersistence(options?: ClientRequestOptions): Promise<ArtifactRegistryPersistenceStatus> {
    return this.request<ArtifactRegistryPersistenceStatus>("GET", "/v1/artifacts/persistence", undefined, options);
  }

  async artifactCrossStoreAudit(options?: ClientRequestOptions): Promise<ArtifactCrossStoreAuditResult> {
    return this.request<ArtifactCrossStoreAuditResult>("GET", "/v1/artifacts/cross-store", undefined, options);
  }

  async flushArtifactRegistryPersistence(options?: ClientRequestOptions): Promise<ArtifactRegistryPersistenceStatus> {
    return this.request<ArtifactRegistryPersistenceStatus>("POST", "/v1/artifacts/persistence/flush", {}, options);
  }

  async capabilityAudit(args: CapabilityAuditArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityAuditResult>> {
    return this.callTool<CapabilityAuditResult>("capability_audit", args, options);
  }

  async capabilityDashboard(args: CapabilityDashboardArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityDashboardResult>> {
    return this.callTool<CapabilityDashboardResult>("capability_dashboard", args, options);
  }

  async capabilityRoute(args: CapabilityRouteArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityRouteResult>> {
    return this.callTool<CapabilityRouteResult>("capability_route", args, options);
  }

  async capabilityRouteReview(args: CapabilityRouteReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityRouteReviewResult>> {
    return this.callTool<CapabilityRouteReviewResult>("capability_route_review", args, options);
  }

  async adapterPlan(args: AdapterPlanArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdapterPlanResult>> {
    return this.callTool<AdapterPlanResult>("adapter_plan", args, options);
  }

  async tabularIngest(args: TabularIngestArgs, options?: ClientRequestOptions): Promise<RestToolResponse<TabularIngestResult>> {
    return this.callTool<TabularIngestResult>("tabular_ingest", args, options);
  }

  async conformanceRun(args: ConformanceRunArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<ConformanceRunResult>> {
    return this.callTool<ConformanceRunResult>("conformance_run", args, options);
  }

  async releaseAudit(args: ReleaseAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ReleaseAuditResult>> {
    return this.callTool<ReleaseAuditResult>("release_audit", args, options);
  }

  async operationsCatalog(args: OperationsCatalogArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<OperationsCatalogResult>> {
    return this.callTool<OperationsCatalogResult>("operations_catalog", args, options);
  }

  async safetyReleaseGate(args: SafetyReleaseGateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<SafetyReleaseGateResult>> {
    return this.callTool<SafetyReleaseGateResult>("safety_release_gate", args, options);
  }

  async medicalBoundaryCheck(args: MedicalBoundaryArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MedicalBoundaryResult>> {
    return this.callTool<MedicalBoundaryResult>("medical_boundary_check", args, options);
  }

  async safetyPosture(args: SafetyPostureArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<SafetyPostureResult>> {
    return this.callTool<SafetyPostureResult>("safety_posture", args, options);
  }

  async hubSearch(args: HubSearchArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubSearchResult>> {
    return this.callTool<HubSearchResult>("hub_search", args, options);
  }

  async hubResolve(args: HubResolveArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubResolveResult>> {
    return this.callTool<HubResolveResult>("hub_resolve", args, options);
  }

  async hubLock(args: HubLockArgs, options?: ClientRequestOptions): Promise<RestToolResponse<HubLockResult>> {
    return this.callTool<HubLockResult>("hub_lock", args, options);
  }

  async worldClaimCheck(args: WorldClaimCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<WorldClaimCheckResult>> {
    return this.callTool<WorldClaimCheckResult>("world_claim_check", args, options);
  }

  async observedWorldDeclare(args: ObservedWorldDeclareArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ObservedWorldDeclareResult>> {
    return this.callTool<ObservedWorldDeclareResult>("observed_world_declare", args, options);
  }

  async measurementCompare(args: MeasurementCompareArgs, options?: ClientRequestOptions): Promise<RestToolResponse<MeasurementCompareResult>> {
    return this.callTool<MeasurementCompareResult>("measurement_compare", args, options);
  }

  async literatureBindCheck(args: LiteratureBindCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LiteratureBindCheckResult>> {
    return this.callTool<LiteratureBindCheckResult>("literature_bind_check", args, options);
  }

  async modalitySupportCheck(args: ModalitySupportCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ModalitySupportCheckResult>> {
    return this.callTool<ModalitySupportCheckResult>("modality_support_check", args, options);
  }

  async modalityTransportCheck(args: ModalityTransportCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ModalityTransportCheckResult>> {
    return this.callTool<ModalityTransportCheckResult>("modality_transport_check", args, options);
  }

  async modalityComparabilityCheck(args: ModalityComparabilityCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ModalityComparabilityCheckResult>> {
    return this.callTool<ModalityComparabilityCheckResult>("modality_comparability_check", args, options);
  }

  async lineageAudit(args: LineageAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LineageAuditResult>> {
    return this.callTool<LineageAuditResult>("lineage_audit", args, options);
  }

  async preanalyticApply(args: PreanalyticApplyArgs, options?: ClientRequestOptions): Promise<RestToolResponse<PreanalyticApplyResult>> {
    return this.callTool<PreanalyticApplyResult>("preanalytic_apply", args, options);
  }

  async contradictionReview(args: ContradictionReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ContradictionReviewResult>> {
    return this.callTool<ContradictionReviewResult>("contradiction_review", args, options);
  }

  async labPlan(args: LabPlanArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LabPlanResult>> {
    return this.callTool<LabPlanResult>("lab_plan", args, options);
  }

  async obligationGateCheck(args: ObligationGateCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ObligationGateCheckResult>> {
    return this.callTool<ObligationGateCheckResult>("obligation_gate_check", args, options);
  }

  async oncoBoundaryCheck(args: OncoBoundaryArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoBoundaryResult>> {
    return this.callTool<OncoBoundaryResult>("onco_boundary_check", args, options);
  }

  async oncoResponseAssess(args: OncoResponseAssessArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoResponseResult>> {
    return this.callTool<OncoResponseResult>("onco_response_assess", args, options);
  }

  async oncoWorldlineView(args: OncoWorldlineViewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldlineResult>> {
    return this.callTool<OncoWorldlineResult>("onco_worldline_view", args, options);
  }

  async oncoClassificationCheck(args: OncoClassificationArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoClassificationResult>> {
    return this.callTool<OncoClassificationResult>("onco_classification_check", args, options);
  }

  async oncoworldsIdentityJoin(args: OncoIdentityJoinArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoIdentityJoinResult>> {
    return this.callTool<OncoIdentityJoinResult>("oncoworlds_identity_join", args, options);
  }

  async oncoOutcomeAnalyze(args: OncoOutcomeAnalyzeArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoOutcomeResult>> {
    return this.callTool<OncoOutcomeResult>("onco_outcome_analyze", args, options);
  }

  async oncoworldsModelTransport(args: OncoWorldsModelTransportArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsModelTransportResult>> {
    return this.callTool<OncoWorldsModelTransportResult>("oncoworlds_model_transport", args, options);
  }

  async oncoworldsMethylationClassify(args: OncoWorldsMethylationClassifyArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsMethylationClassifyResult>> {
    return this.callTool<OncoWorldsMethylationClassifyResult>("oncoworlds_methylation_classify", args, options);
  }

  async oncoworldsMethylationCompare(args: OncoWorldsMethylationCompareArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsMethylationCompareResult>> {
    return this.callTool<OncoWorldsMethylationCompareResult>("oncoworlds_methylation_compare", args, options);
  }

  async oncoworldsRadiogenomicCheck(args: OncoWorldsRadiogenomicCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsRadiogenomicCheckResult>> {
    return this.callTool<OncoWorldsRadiogenomicCheckResult>("oncoworlds_radiogenomic_check", args, options);
  }

  async oncoworldsClonalHistoryCheck(args: OncoWorldsClonalHistoryCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsClonalHistoryCheckResult>> {
    return this.callTool<OncoWorldsClonalHistoryCheckResult>("oncoworlds_clonal_history_check", args, options);
  }

  async oncoworldsClonalEvidenceCheck(args: OncoClonalEvidenceCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsClonalEvidenceCheckResult>> {
    return this.callTool<OncoWorldsClonalEvidenceCheckResult>("oncoworlds_clonal_evidence_check", args, options);
  }

  async oncoworldsEraShiftCheck(args: OncoWorldsEraShiftCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsEraShiftCheckResult>> {
    return this.callTool<OncoWorldsEraShiftCheckResult>("oncoworlds_era_shift_check", args, options);
  }

  async oncoworldsEquityCheck(args: OncoWorldsEquityCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsEquityCheckResult>> {
    return this.callTool<OncoWorldsEquityCheckResult>("oncoworlds_equity_check", args, options);
  }

  async oncoworldsEntityWorldCheck(args: OncoWorldsEntityWorldCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OncoWorldsEntityWorldCheckResult>> {
    return this.callTool<OncoWorldsEntityWorldCheckResult>("oncoworlds_entity_world_check", args, options);
  }

  async stressProfile(args: StressProfileArgs, options?: ClientRequestOptions): Promise<RestToolResponse<StressProfileToolResult>> {
    return this.callTool<StressProfileToolResult>("stress_profile", args, options);
  }

  async stressReport(args: StressReportArgs, options?: ClientRequestOptions): Promise<RestToolResponse<StressReportToolResult>> {
    return this.callTool<StressReportToolResult>("stress_report", args, options);
  }

  async influenceAnalyze(args: InfluenceAnalyzeArgs, options?: ClientRequestOptions): Promise<RestToolResponse<InfluenceAnalyzeResult>> {
    return this.callTool<InfluenceAnalyzeResult>("influence_analyze", args, options);
  }

  async routingDecide(args: RoutingDecideArgs, options?: ClientRequestOptions): Promise<RestToolResponse<RoutingToolResult>> {
    return this.callTool<RoutingToolResult>("routing_decide", args, options);
  }

  async routingLabRun(args: RoutingLabRunArgs, options?: ClientRequestOptions): Promise<RestToolResponse<RoutingLabRunResult>> {
    return this.callTool<RoutingLabRunResult>("routing_lab_run", args, options);
  }

  async labParetoAudit(args: LabParetoAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LabParetoAuditResult>> {
    return this.callTool<LabParetoAuditResult>("lab_pareto_audit", args, options);
  }

  async labBranchAudit(args: LabBranchAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LabBranchAuditResult>> {
    return this.callTool<LabBranchAuditResult>("lab_branch_audit", args, options);
  }

  async labHoldoutAudit(args: LabHoldoutAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LabHoldoutAuditResult>> {
    return this.callTool<LabHoldoutAuditResult>("lab_holdout_audit", args, options);
  }

  async labEvolutionAudit(args: LabEvolutionAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LabEvolutionAuditResult>> {
    return this.callTool<LabEvolutionAuditResult>("lab_evolution_audit", args, options);
  }

  async labSpaceAudit(args: LabSpaceAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<LabSpaceAuditResult>> {
    return this.callTool<LabSpaceAuditResult>("lab_space_audit", args, options);
  }

  async providerCapabilityGate(args: ProviderCapabilityGateArgs, options?: ClientRequestOptions): Promise<RestToolResponse<ProviderCapabilityGateResult>> {
    return this.callTool<ProviderCapabilityGateResult>("provider_capability_gate", args, options);
  }

  async sdkRegistryCheck(args: SdkRegistryCheckArgs, options?: ClientRequestOptions): Promise<RestToolResponse<SdkRegistryCheckResult>> {
    return this.callTool<SdkRegistryCheckResult>("sdk_registry_check", args, options);
  }

  async oracleCombine(args: OracleCombineArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OracleCombineResult>> {
    return this.callTool<OracleCombineResult>("oracle_combine", args, options);
  }

  async oracleReferencePanel(args: OracleReferencePanelArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OracleReferencePanelResult>> {
    return this.callTool<OracleReferencePanelResult>("oracle_reference_panel", args, options);
  }

  async oracleMissingness(args: OracleMissingnessArgs, options?: ClientRequestOptions): Promise<RestToolResponse<OracleMissingnessResult>> {
    return this.callTool<OracleMissingnessResult>("oracle_missingness", args, options);
  }

  async bioevalReferenceAudit(args: BioevalReferenceAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalReferenceAuditResult>> {
    return this.callTool<BioevalReferenceAuditResult>("bioeval_reference_audit", args, options);
  }

  async bioevalAcquisitionAudit(args: BioevalAcquisitionAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalAcquisitionAuditResult>> {
    return this.callTool<BioevalAcquisitionAuditResult>("bioeval_acquisition_audit", args, options);
  }

  async bioevalGroundingAudit(args: BioevalGroundingAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalGroundingAuditResult>> {
    return this.callTool<BioevalGroundingAuditResult>("bioeval_grounding_audit", args, options);
  }

  async bioevalEstimandAudit(args: BioevalEstimandAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalEstimandAuditResult>> {
    return this.callTool<BioevalEstimandAuditResult>("bioeval_estimand_audit", args, options);
  }

  async bioevalEvaluatorAudit(args: BioevalEvaluatorAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalEvaluatorAuditResult>> {
    return this.callTool<BioevalEvaluatorAuditResult>("bioeval_evaluator_audit", args, options);
  }

  async bioevalPlaneAudit(args: BioevalPlaneAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalPlaneAuditResult>> {
    return this.callTool<BioevalPlaneAuditResult>("bioeval_plane_audit", args, options);
  }

  async bioevalMetamorphicAudit(args: BioevalMetamorphicAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalMetamorphicAuditResult>> {
    return this.callTool<BioevalMetamorphicAuditResult>("bioeval_metamorphic_audit", args, options);
  }

  async bioevalWaiverAudit(args: BioevalWaiverAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalWaiverAuditResult>> {
    return this.callTool<BioevalWaiverAuditResult>("bioeval_waiver_audit", args, options);
  }

  async bioevalDesignAudit(args: BioevalDesignAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalDesignAuditResult>> {
    return this.callTool<BioevalDesignAuditResult>("bioeval_design_audit", args, options);
  }

  async bioevalMeshAudit(args: BioevalMeshAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalMeshAuditResult>> {
    return this.callTool<BioevalMeshAuditResult>("bioeval_mesh_audit", args, options);
  }

  async bioevalBurdenAudit(args: BioevalBurdenAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalBurdenAuditResult>> {
    return this.callTool<BioevalBurdenAuditResult>("bioeval_burden_audit", args, options);
  }

  async bioevalRevealAudit(args: BioevalRevealAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalRevealAuditResult>> {
    return this.callTool<BioevalRevealAuditResult>("bioeval_reveal_audit", args, options);
  }

  async bioevalBoundaryAudit(args: BioevalBoundaryAuditArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BioevalBoundaryAuditResult>> {
    return this.callTool<BioevalBoundaryAuditResult>("bioeval_boundary_audit", args, options);
  }

  async evaluationWorldlineAudit(args: EvaluationWorldlineArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EvaluationWorldlineResult>> {
    return this.callTool<EvaluationWorldlineResult>("evaluation_worldline_audit", args, options);
  }

  async evaluationReproductionCheck(args: EvaluationReproductionArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EvaluationReproductionResult>> {
    return this.callTool<EvaluationReproductionResult>("evaluation_reproduction_check", args, options);
  }

  async evaluationTrajectoryCheck(args: EvaluationTrajectoryArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EvaluationTrajectoryResult>> {
    return this.callTool<EvaluationTrajectoryResult>("evaluation_trajectory_check", args, options);
  }

  async opsAcceptance(args: OpsAcceptanceArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<OpsAcceptanceResult>> {
    return this.callTool<OpsAcceptanceResult>("ops_acceptance", args, options);
  }

  async agentMission(args: AgentMissionArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AgentMissionReport>> {
    return this.callTool<AgentMissionReport>("agent_mission", args, options);
  }

  /** Submit a validated mission to the cooperative asynchronous HTTP executor. */
  async submitMission(args: AgentMissionArgs, options?: ClientRequestOptions): Promise<MissionJob> {
    if (!isObject(args)) throw new ArgumentError("mission arguments must be a JSON object");
    return this.request<MissionJob>("POST", "/v1/missions", args, options);
  }

  /** Ask the Rust gateway for an authoritative mission plan without dispatching nested tools. */
  async preflightMission(args: AgentMissionArgs, options?: ClientRequestOptions): Promise<AgentMissionReport> {
    if (!isObject(args)) throw new ArgumentError("mission arguments must be a JSON object");
    return this.request<AgentMissionReport>("POST", "/v1/missions/preflight", args, options);
  }

  /** List bounded mission summaries without returning unbounded terminal reports. */
  async missions(
    status?: MissionJobStatus,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<MissionInventoryResponse> {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) throw new ArgumentError("limit must be 1..=256");
    if (status !== undefined) visible(status, "status", 32);
    const query = new URLSearchParams({ limit: String(limit) });
    if (status !== undefined) query.set("status", status);
    return this.request<MissionInventoryResponse>("GET", `/v1/missions?${query.toString()}`, undefined, options);
  }

  /** Inspect whether restart-aware mission snapshots are enabled and within their bounds. */
  async missionPersistence(options?: ClientRequestOptions): Promise<MissionPersistenceStatus> {
    return this.request<MissionPersistenceStatus>("GET", "/v1/missions/persistence", undefined, options);
  }

  /** Force a bounded mission snapshot checkpoint; the gateway returns the resulting status. */
  async flushMissionPersistence(options?: ClientRequestOptions): Promise<MissionPersistenceStatus> {
    return this.request<MissionPersistenceStatus>("POST", "/v1/missions/persistence/flush", {}, options);
  }

  /** Read queue lifecycle projections without returning checkpointed job specifications. */
  async missionQueue(options?: ClientRequestOptions): Promise<MissionQueueInventoryResponse> {
    return this.request<MissionQueueInventoryResponse>("GET", "/v1/missions/queue", undefined, options);
  }

  /** Inspect queue checkpoint integrity, startup recovery rows, and the explicit no-resume boundary. */
  async missionQueuePersistence(options?: ClientRequestOptions): Promise<MissionQueueStatus> {
    return this.request<MissionQueueStatus>("GET", "/v1/missions/queue/persistence", undefined, options);
  }

  /** Atomically flush the queue checkpoint and return its resulting status and byte count. */
  async flushMissionQueuePersistence(options?: ClientRequestOptions): Promise<MissionQueueFlushResponse> {
    return this.request<MissionQueueFlushResponse>("POST", "/v1/missions/queue/persistence/flush", {}, options);
  }

  /** Explicitly release an orphaned shared-authority lock with an auditable operator reason. */
  async releaseMissionQueueLock(
    operator: string,
    reason: string,
    options?: ClientRequestOptions,
  ): Promise<MissionQueueLockReleaseResponse> {
    if (typeof operator !== "string" || operator.trim().length === 0) {
      throw new ArgumentError("operator must be a non-empty string");
    }
    if (typeof reason !== "string" || reason.trim().length === 0) {
      throw new ArgumentError("reason must be a non-empty string");
    }
    return this.request<MissionQueueLockReleaseResponse>(
      "POST",
      "/v1/missions/queue/authority/release-lock",
      { operator, reason },
      options,
    );
  }

  /** Read the current asynchronous mission status and, once terminal, its authoritative report. */
  async missionStatus(missionId: string, options?: ClientRequestOptions): Promise<MissionJob> {
    const id = pathSegment(missionId, "mission id");
    return this.request<MissionJob>("GET", `/v1/missions/${encodeURIComponent(id)}`, undefined, options);
  }

  /** Read the retained gate, review, evaluator, and accepted-dispatch provenance for a mission. */
  async missionProvenance(
    missionId: string,
    options?: ClientRequestOptions,
  ): Promise<MissionExecutionProvenanceResponse> {
    const id = pathSegment(missionId, "mission id");
    return this.request<MissionExecutionProvenanceResponse>(
      "GET",
      `/v1/missions/${encodeURIComponent(id)}/provenance`,
      undefined,
      options,
    );
  }

  /** Read the bounded claim-to-step evidence projection for a terminal mission. */
  async missionClaimLineage(
    missionId: string,
    options?: ClientRequestOptions,
  ): Promise<MissionClaimLineageResponse> {
    const id = pathSegment(missionId, "mission id");
    return this.request<MissionClaimLineageResponse>(
      "GET",
      `/v1/missions/${encodeURIComponent(id)}/claims`,
      undefined,
      options,
    );
  }

  /** Read durable full or summary-only evaluator replay evidence for one mission. */
  async missionEvaluatorReplayQuery(
    missionId: string,
    queryOptions: MissionEvaluatorReplayQueryOptions = {},
    options?: ClientRequestOptions,
  ): Promise<MissionEvaluatorReplayQueryResult> {
    const id = pathSegment(missionId, "mission id");
    if (queryOptions.include_fixtures !== undefined && typeof queryOptions.include_fixtures !== "boolean") {
      throw new ArgumentError("include_fixtures must be a boolean");
    }
    const maxItems = queryOptions.max_items ?? 128;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 512) {
      throw new ArgumentError("max_items must be 1..=512");
    }
    const query = new URLSearchParams({
      include_fixtures: String(queryOptions.include_fixtures ?? false),
      max_items: String(maxItems),
    });
    return this.request<MissionEvaluatorReplayQueryResult>(
      "GET",
      `/v1/missions/${encodeURIComponent(id)}/evaluator-replay?${query.toString()}`,
      undefined,
      options,
    );
  }

  /** Compare durable replay evidence with the current evaluator catalogue. */
  async missionEvaluatorReplayCompareQuery(
    missionId: string,
    queryOptions: MissionEvaluatorReplayQueryOptions = {},
    options?: ClientRequestOptions,
  ): Promise<MissionEvaluatorReplayCompareResult> {
    const id = pathSegment(missionId, "mission id");
    if (queryOptions.include_fixtures !== undefined && typeof queryOptions.include_fixtures !== "boolean") {
      throw new ArgumentError("include_fixtures must be a boolean");
    }
    const maxItems = queryOptions.max_items ?? 128;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 512) {
      throw new ArgumentError("max_items must be 1..=512");
    }
    const query = new URLSearchParams({
      include_fixtures: String(queryOptions.include_fixtures ?? false),
      max_items: String(maxItems),
    });
    return this.request<MissionEvaluatorReplayCompareResult>(
      "GET",
      `/v1/missions/${encodeURIComponent(id)}/evaluator-replay/compare?${query.toString()}`,
      undefined,
      options,
    );
  }

  /** Export a bounded, content-addressed mission evidence bundle. */
  async missionEvidenceBundle(
    missionId: string,
    bundleOptions: MissionEvidenceBundleOptions = {},
    options?: ClientRequestOptions,
  ): Promise<MissionEvidenceBundleResult> {
    const id = pathSegment(missionId, "mission id");
    for (const [name, value] of [
      ["include_result", bundleOptions.include_result],
      ["include_trace", bundleOptions.include_trace],
      ["include_fixtures", bundleOptions.include_fixtures],
    ] as const) {
      if (value !== undefined && typeof value !== "boolean") throw new ArgumentError(`${name} must be a boolean`);
    }
    const maxItems = bundleOptions.max_items ?? 128;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 512) {
      throw new ArgumentError("max_items must be 1..=512");
    }
    const query = new URLSearchParams({
      include_result: String(bundleOptions.include_result ?? false),
      include_trace: String(bundleOptions.include_trace ?? true),
      include_fixtures: String(bundleOptions.include_fixtures ?? false),
      max_items: String(maxItems),
    });
    return this.request<MissionEvidenceBundleResult>(
      "GET",
      `/v1/missions/${encodeURIComponent(id)}/evidence-bundle?${query.toString()}`,
      undefined,
      options,
    );
  }

  /** Verify a portable mission evidence bundle's canonical and retained-result digests. */
  async missionEvidenceBundleVerifyQuery(
    bundle: JsonObject,
    options?: ClientRequestOptions,
  ): Promise<MissionEvidenceBundleVerifyResult> {
    if (!isObject(bundle)) throw new ArgumentError("bundle must be a JSON object");
    return this.request<MissionEvidenceBundleVerifyResult>(
      "POST",
      "/v1/evidence-bundles/verify",
      { bundle },
      options,
    );
  }

  /** Read a bounded cursor page from the authoritative clock-free mission trace. */
  async missionTrace(
    missionId: string,
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<MissionTracePage> {
    const id = pathSegment(missionId, "mission id");
    if (!Number.isSafeInteger(after) || after < 0) throw new ArgumentError("after must be a non-negative integer");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > MAX_EVENT_PAGE) throw new ArgumentError("limit must be 1..=1000");
    const query = new URLSearchParams({ after: String(after), limit: String(limit) });
    return this.request<MissionTracePage>("GET", `/v1/missions/${encodeURIComponent(id)}/trace?${query.toString()}`, undefined, options);
  }

  /** Poll a mission to a terminal state with bounded, abortable client-side waiting. */
  async waitMission(missionId: string, options: MissionWaitOptions = {}): Promise<MissionJob> {
    const id = pathSegment(missionId, "mission id");
    const timeoutMs = boundedDuration(options.timeoutMs ?? this.timeoutMs, "timeoutMs", MAX_MISSION_WAIT_MS);
    const pollIntervalMs = boundedDuration(options.pollIntervalMs ?? 250, "pollIntervalMs", MAX_MISSION_POLL_INTERVAL_MS);
    const deadline = Date.now() + timeoutMs;
    let job = await this.missionStatus(id, options);
    while (!isTerminalMissionStatus(job.status)) {
      const remaining = deadline - Date.now();
      if (remaining <= 0) throw new MissionWaitTimeoutError(id, timeoutMs, job);
      await delay(Math.min(pollIntervalMs, remaining), options.signal);
      job = await this.missionStatus(id, options);
    }
    return job;
  }

  /** Request cooperative cancellation; in-flight nested tools are allowed to return. */
  async cancelMission(missionId: string, reason?: string, options?: ClientRequestOptions): Promise<MissionJob> {
    const id = pathSegment(missionId, "mission id");
    if (reason !== undefined) visible(reason, "reason", 2_048);
    return this.request<MissionJob>("POST", `/v1/missions/${encodeURIComponent(id)}/cancel`, reason === undefined ? {} : { reason }, options);
  }

  /** Remove a terminal mission from the bounded in-process registry. */
  async deleteMission(missionId: string, options?: ClientRequestOptions): Promise<JsonObject> {
    const id = pathSegment(missionId, "mission id");
    return this.request("DELETE", `/v1/missions/${encodeURIComponent(id)}`, undefined, options);
  }

  /** Review a mission against a live or caller-supplied catalogue without issuing any tool call. */
  async missionPreflight(
    args: AgentMissionArgs,
    catalogue?: ToolCatalogue,
    options?: ClientRequestOptions,
  ): Promise<MissionPreflightResult> {
    const snapshot = catalogue ?? await this.toolCatalogue(options);
    return preflightMission(args, snapshot);
  }

  /** Assemble a route-bound mission locally; this performs no network call or tool execution. */
  missionFromRoute(
    route: JsonObject,
    missionId: string,
    selections: readonly MissionRouteSelection[],
    policy?: AgentMissionPolicy,
  ): MissionAssembly {
    return assembleMissionFromRoute(route, missionId, selections, policy);
  }

  async runtimeEffectCheck(
    args: RuntimeEffectCheckArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<RuntimeEffectCheckResult>> {
    return this.callTool("runtime_effect_check", args, options);
  }

  async runtimeTapeVerify(
    args: RuntimeTapeVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<RuntimeTapeVerifyResult>> {
    return this.callTool("runtime_tape_verify", args, options);
  }

  async runtimeExecutionSimulate(
    args: RuntimeExecutionSimulateArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<RuntimeExecutionSimulateResult>> {
    return this.callTool("runtime_execution_simulate", args, options);
  }

  async bioethicsActionReview(
    args: BioethicsActionReviewArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<BioethicsActionReviewResult>> {
    return this.callTool("bioethics_action_review", args, options);
  }

  async humanSubjectScreen(
    args: HumanSubjectScreenArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<HumanSubjectScreenResult>> {
    return this.callTool("bioethics_human_subject_screen", args, options);
  }

  async bioethicsDualUseReview(
    args: BioethicsDualUseReviewArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<BioethicsDualUseReviewResult>> {
    return this.callTool("bioethics_dual_use_review", args, options);
  }

  async bioethicsValidationCheck(
    args: BioethicsValidationCheckArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<BioethicsValidationCheckResult>> {
    return this.callTool("bioethics_validation_check", args, options);
  }

  async bioethicsRepresentationAudit(
    args: BioethicsRepresentationAuditArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<BioethicsRepresentationAuditResult>> {
    return this.callTool("bioethics_representation_audit", args, options);
  }

  async events(after = 0, limit = 100, options?: ClientRequestOptions): Promise<EventsResponse> {
    cursor(after, "after");
    pageLimit(limit);
    return this.request("GET", `/v1/events?after=${after}&limit=${limit}`, undefined, options);
  }

  /** Fetch the bounded SSE snapshot and parse each event without requiring an EventSource. */
  async eventStream(after = 0, limit = 100, options?: ClientRequestOptions): Promise<SseSnapshot> {
    cursor(after, "after");
    pageLimit(limit);
    const result = await this.requestText("GET", `/v1/events/stream?after=${after}&limit=${limit}`, options);
    return {
      contentType: result.response.headers.get("content-type") ?? "",
      nextAfter: parseUnsignedHeader(result.response.headers.get("x-next-after"), "x-next-after"),
      events: parseSse(result.text),
      raw: result.text,
    };
  }

  /** Retrieve retained event evidence for one content-addressed capability route review. */
  async routeReviewEvidence(
    reviewId: string,
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<RouteReviewEvidenceResponse> {
    validateReviewId(reviewId);
    cursor(after, "after");
    pageLimit(limit);
    return this.request(
      "GET",
      "/v1/route-reviews/" + encodeURIComponent(reviewId) + "/evidence?after=" + after + "&limit=" + limit,
      undefined,
      options,
    );
  }

  /** Retrieve retained event evidence for one content-addressed delivery receipt. */
  async deliveryReceiptEvents(
    receiptId: string,
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<DeliveryReceiptEventsResponse> {
    visible(receiptId, "receipt id", 128);
    cursor(after, "after");
    pageLimit(limit);
    return this.request(
      "GET",
      "/v1/delivery-receipts/" + encodeURIComponent(receiptId) + "/events?after=" + after + "&limit=" + limit,
      undefined,
      options,
    );
  }

  /** Retrieve durable webhook-attempt provenance correlated to one content-addressed receipt. */
  async deliveryReceiptAttempts(
    receiptId: string,
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<DeliveryReceiptAttemptsResponse> {
    visible(receiptId, "receipt id", 128);
    cursor(after, "after");
    pageLimit(limit);
    return this.request(
      "GET",
      "/v1/delivery-receipts/" + encodeURIComponent(receiptId) + "/attempts?after=" + after + "&limit=" + limit,
      undefined,
      options,
    );
  }

  async listSubscriptions(options?: ClientRequestOptions): Promise<SubscriptionListResponse> {
    return this.request("GET", "/v1/webhooks/subscriptions", undefined, options);
  }

  async subscribe(
    endpoint: string,
    secret: string,
    options_: SubscribeOptions = {},
    requestOptions?: ClientRequestOptions,
  ): Promise<SubscriptionResponse> {
    visible(endpoint, "endpoint", 2_048);
    visible(secret, "secret", 4_096);
    const payload: JsonObject = { endpoint, secret };
    if (options_.subscriptionId !== undefined) payload.id = pathSegment(options_.subscriptionId, "subscription id");
    if (options_.events !== undefined) {
      if (options_.events.length < 1 || options_.events.length > 32) {
        throw new ArgumentError("events must contain between 1 and 32 filters");
      }
      payload.events = options_.events.map((event) => visible(event, "event filter", 128));
    }
    return this.request("POST", "/v1/webhooks/subscriptions", payload, requestOptions);
  }

  /** Rebind a restored webhook secret in memory and re-sign pending envelopes. */
  async rebindSubscription(
    subscriptionId: string,
    secret: string,
    options?: ClientRequestOptions,
  ): Promise<SubscriptionRebindResponse> {
    const id = pathSegment(subscriptionId, "subscription id");
    visible(secret, "secret", 4_096);
    return this.request(
      "POST",
      `/v1/webhooks/subscriptions/${encodeURIComponent(id)}/rebind`,
      { secret },
      options,
    );
  }

  async deliveries(subscriptionId: string, after = 0, limit = 100, options?: ClientRequestOptions): Promise<DeliveriesResponse> {
    const id = pathSegment(subscriptionId, "subscription id");
    cursor(after, "after");
    pageLimit(limit);
    return this.request("GET", `/v1/webhooks/subscriptions/${encodeURIComponent(id)}/deliveries?after=${after}&limit=${limit}`, undefined, options);
  }

  /** Read bounded, durable send/retry/replay/acknowledgement provenance for a subscription. */
  async deliveryAttempts(
    subscriptionId: string,
    after = 0,
    limit = 100,
    options?: ClientRequestOptions,
  ): Promise<DeliveryAttemptsResponse> {
    const id = pathSegment(subscriptionId, "subscription id");
    cursor(after, "after");
    pageLimit(limit);
    return this.request(
      "GET",
      `/v1/webhooks/subscriptions/${encodeURIComponent(id)}/attempts?after=${after}&limit=${limit}`,
      undefined,
      options,
    );
  }

  async acknowledge(subscriptionId: string, deliveryIds: readonly number[], options?: ClientRequestOptions): Promise<DeliveryMutationResponse> {
    return this.deliveryMutation("ack", subscriptionId, deliveryIds, options);
  }

  async retry(subscriptionId: string, deliveryIds: readonly number[], options?: ClientRequestOptions): Promise<DeliveryMutationResponse> {
    return this.deliveryMutation("retry", subscriptionId, deliveryIds, options);
  }

  async replay(subscriptionId: string, deliveryIds: readonly number[], options?: ClientRequestOptions): Promise<DeliveryMutationResponse> {
    return this.deliveryMutation("replay", subscriptionId, deliveryIds, options);
  }

  async deleteSubscription(subscriptionId: string, options?: ClientRequestOptions): Promise<JsonObject> {
    const id = pathSegment(subscriptionId, "subscription id");
    return this.request("DELETE", `/v1/webhooks/subscriptions/${encodeURIComponent(id)}`, undefined, options);
  }

  private async deliveryMutation(
    operation: "ack" | "retry" | "replay",
    subscriptionId: string,
    deliveryIds: readonly number[],
    options?: ClientRequestOptions,
  ): Promise<DeliveryMutationResponse> {
    const id = pathSegment(subscriptionId, "subscription id");
    if (deliveryIds.length > 1_000 || deliveryIds.some((value) => !Number.isSafeInteger(value) || value < 1)) {
      throw new ArgumentError("deliveryIds must contain positive safe integers and be at most 1000 items");
    }
    return this.request("POST", `/v1/webhooks/subscriptions/${encodeURIComponent(id)}/${operation}`, { delivery_ids: [...deliveryIds] }, options);
  }

  private async execute(
    method: HttpMethod,
    path: string,
    payload: JsonObject | undefined,
    options: ClientRequestOptions,
  ): Promise<Response> {
    if (!["GET", "POST", "DELETE", "OPTIONS"].includes(method)) throw new ArgumentError("unsupported HTTP method");
    const target = originPath(this.baseUrl, path);
    const headers: Record<string, string> = { Accept: "application/json", ...this.defaultHeaders };
    if (payload !== undefined) {
      assertJsonSafe(payload);
      const encoded = JSON.stringify(payload);
      const bytes = new TextEncoder().encode(encoded).byteLength;
      if (bytes > this.maxRequestBytes) throw new ArgumentError(`request payload exceeded maxRequestBytes (${this.maxRequestBytes})`);
      headers["Content-Type"] = "application/json";
      return this.fetchWithTimeout(target, method, headers, encoded, options);
    }
    return this.fetchWithTimeout(target, method, headers, undefined, options);
  }

  private fetchWithTimeout(
    target: URL,
    method: HttpMethod,
    headers: Record<string, string>,
    body: string | undefined,
    options: ClientRequestOptions,
  ): Promise<Response> {
    const requestHeaders = validateHeaders({ ...headers, ...options.headers });
    if (this.bearerToken !== undefined) requestHeaders.Authorization = `Bearer ${this.bearerToken}`;
    if (options.requestId !== undefined) requestHeaders["x-request-id"] = visible(options.requestId, "requestId", MAX_REQUEST_ID_BYTES);
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      controller.abort();
    }, this.timeoutMs);
    const external = options.signal;
    const abort = () => controller.abort();
    external?.addEventListener("abort", abort, { once: true });
    return this.fetchImpl(target, {
      method,
      headers: requestHeaders,
      body,
      signal: controller.signal,
    }).catch((error: unknown) => {
      if (timedOut) throw new TransportError(`HTTP API request timed out after ${this.timeoutMs}ms`, error);
      if (external?.aborted) throw new TransportError("HTTP API request was aborted by the caller", error);
      throw new TransportError("HTTP API request failed", error);
    }).finally(() => {
      clearTimeout(timer);
      external?.removeEventListener("abort", abort);
    });
  }
}

function resolveFetch(): FetchLike {
  if (typeof globalThis.fetch !== "function") {
    throw new TransportError("No fetch implementation is available; pass fetch in ApiClient options");
  }
  return globalThis.fetch.bind(globalThis);
}

function validateBaseUrl(input: string | URL): URL {
  let url: URL;
  try {
    url = new URL(input.toString());
  } catch {
    throw new ArgumentError("baseUrl must be an absolute http(s) URL");
  }
  if (!(["http:", "https:"].includes(url.protocol)) || !url.hostname) throw new ArgumentError("baseUrl must be an http(s) URL with a host");
  if (url.pathname !== "/" || url.search || url.hash) throw new ArgumentError("baseUrl must not include a path, query, or fragment");
  return url;
}

function originPath(baseUrl: URL, path: string): URL {
  if (!path.startsWith("/") || path.startsWith("//") || path.includes("\r") || path.includes("\n")) {
    throw new ArgumentError("path must be an origin-form path without control-line breaks");
  }
  return new URL(path, baseUrl);
}

function validateBearerToken(token: string): void {
  if (token.length < 16 || /\s/.test(token) || /[\r\n]/.test(token)) throw new ArgumentError("bearerToken must contain at least 16 visible characters");
}

function validateHeaders(headers: Readonly<Record<string, string>>): Record<string, string> {
  const output: Record<string, string> = {};
  for (const [name, value] of Object.entries(headers)) {
    if (!name || /[\r\n]/.test(name) || /[\r\n]/.test(value)) throw new ArgumentError("HTTP headers must not contain control-line breaks");
    output[name] = value;
  }
  return output;
}

function positiveInteger(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) throw new ArgumentError(`${name} must be a positive safe integer`);
  return value;
}

function boundedDuration(value: number, name: string, maximum: number): number {
  if (!Number.isSafeInteger(value) || value <= 0 || value > maximum) {
    throw new ArgumentError(`${name} must be a positive safe integer no greater than ${maximum}`);
  }
  return value;
}

function isTerminalMissionStatus(status: MissionJobStatus): boolean {
  return status === "planned" || status === "succeeded" || status === "partial" || status === "failed" || status === "cancelled";
}

function delay(milliseconds: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new TransportError("mission wait was aborted by the caller"));
      return;
    }
    let settled = false;
    const timer = setTimeout(() => {
      settled = true;
      signal?.removeEventListener("abort", abort);
      resolve();
    }, milliseconds);
    const abort = () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      reject(new TransportError("mission wait was aborted by the caller"));
    };
    signal?.addEventListener("abort", abort, { once: true });
  });
}

function cursor(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 0) throw new ArgumentError(`${name} must be a non-negative safe integer`);
}

function pageLimit(value: number): void {
  if (!Number.isSafeInteger(value) || value < 1 || value > MAX_EVENT_PAGE) throw new ArgumentError("limit must be 1..=1000");
}

function validateReviewId(value: string): void {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    throw new ArgumentError("reviewId must be a 64-character hexadecimal content hash");
  }
}

function visible(value: string, name: string, maxBytes: number): string {
  if (typeof value !== "string" || value.length === 0 || /[\r\n]/.test(value) || new TextEncoder().encode(value).byteLength > maxBytes) {
    throw new ArgumentError(`${name} must be non-empty, line-safe, and at most ${maxBytes} UTF-8 bytes`);
  }
  return value;
}

function pathSegment(value: string, name: string): string {
  visible(value, name, 256);
  if (value === "." || value === ".." || value.includes("/") || value.includes("\\")) throw new ArgumentError(`${name} must be a path-safe string`);
  return value;
}

function parseUnsignedHeader(value: string | null, name: string): number | null {
  if (value === null) return null;
  if (!/^\d+$/.test(value)) throw new ProtocolError(`${name} is not an unsigned integer`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) throw new ProtocolError(`${name} exceeds JavaScript safe integer range`);
  return parsed;
}

function parseJsonObject(text: string): JsonObject {
  if (!text) return {};
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    throw new ProtocolError(`HTTP API returned invalid JSON: ${String(error)}`);
  }
  if (!isObject(parsed)) throw new ProtocolError("HTTP API response must be a JSON object");
  return parsed as JsonObject;
}

function assertJsonSafe(value: unknown, depth = 0): void {
  if (depth > 100) throw new ArgumentError("JSON payload nesting exceeds 100 levels");
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError("JSON payload contains a non-finite number");
  if (typeof value === "bigint" || typeof value === "function" || typeof value === "symbol" || value === undefined) throw new ArgumentError("JSON payload contains an unsupported value");
  if (Array.isArray(value)) {
    value.forEach((item) => assertJsonSafe(item, depth + 1));
  } else if (isObject(value)) {
    Object.entries(value).forEach(([key, item]) => {
      visible(key, "JSON object key", 1_024);
      assertJsonSafe(item, depth + 1);
    });
  }
}

async function readResponseText(response: Response, maxBytes: number): Promise<string> {
  if (!response.body) {
    const text = await response.text();
    if (new TextEncoder().encode(text).byteLength > maxBytes) throw new ResponseTooLargeError(maxBytes);
    return text;
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const chunks: string[] = [];
  let bytes = 0;
  try {
    while (true) {
      const item = await reader.read();
      if (item.done) break;
      bytes += item.value.byteLength;
      if (bytes > maxBytes) {
        await reader.cancel();
        throw new ResponseTooLargeError(maxBytes);
      }
      chunks.push(decoder.decode(item.value, { stream: true }));
    }
    chunks.push(decoder.decode());
    return chunks.join("");
  } catch (error) {
    if (error instanceof ResponseTooLargeError) throw error;
    throw new TransportError("HTTP API response could not be read", error);
  } finally {
    reader.releaseLock();
  }
}
