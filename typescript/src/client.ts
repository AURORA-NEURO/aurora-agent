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
  CapabilityRoutePlanArgs,
  CapabilityRoutePlanResult,
  CapabilityRoutePlanVerifyArgs,
  CapabilityRoutePlanVerifyResult,
  CapabilityRouteReviewArgs,
  CapabilityRouteReviewResult,
  CapabilityRouteResult,
  DomainWorkflowCatalogueResult,
  DomainWorkflowInstantiateArgs,
  DomainWorkflowInstantiateResult,
  DomainWorkflowPortfolioArgs,
  DomainWorkflowPortfolioResult,
  DomainWorkflowPortfolioVerifyArgs,
  DomainWorkflowPortfolioVerifyResult,
  DomainWorkflowVerifyArgs,
  DomainWorkflowVerifyResult,
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
  AdapterDomainReportArgs,
  AdapterDomainReportResult,
  ProviderDomainReportArgs,
  ProviderDomainReportResult,
  DomainReportCoverageOptions,
  DomainReportCoverageResult,
  DomainEvidenceHarmonizeArgs,
  DomainEvidenceHarmonizationResult,
  DomainEvidenceHarmonizationCoverageOptions,
  DomainEvidenceHarmonizationCoverageResult,
  DomainEvidenceIntakeArgs,
  DomainEvidenceIntakeResult,
  DomainEvidenceIntakeCoverageOptions,
  DomainEvidenceIntakeCoverageResult,
  DomainEvidenceSourcePlanArgs,
  DomainEvidenceSourcePlanResult,
  DomainEvidenceSourceExecutionArgs,
  DomainEvidenceSourceExecutionResult,
  DomainEvidenceProviderNormalizationArgs,
  DomainEvidenceProviderNormalizationResult,
  DomainEvidenceProviderHandoffArgs,
  DomainEvidenceProviderHandoffResult,
  DomainEvidenceProviderExternalPayloadReceiptArgs,
  DomainEvidenceProviderExternalPayloadReceiptResult,
  DomainEvidenceProviderExternalPayloadReplayVerifyArgs,
  DomainEvidenceProviderExternalPayloadReplayVerifyResult,
  DomainEvidenceProviderExternalPayloadNormalizationArgs,
  DomainEvidenceProviderExternalPayloadNormalizationResult,
  DomainEvidenceProviderExternalPayloadLineageAuditArgs,
  DomainEvidenceProviderExternalPayloadLineageAuditResult,
  DomainEvidenceProviderExternalPayloadExecutionEvidenceArgs,
  DomainEvidenceProviderExternalPayloadExecutionEvidenceResult,
  DomainEvidenceProviderExternalPayloadEvidenceQueryArgs,
  DomainEvidenceProviderExternalPayloadEvidenceQueryResult,
  DomainEvidenceProviderReplayVerifyArgs,
  DomainEvidenceProviderReplayVerifyResult,
  AdapterPlanArgs,
  AdapterPlanResult,
  AdapterExecutionEvidenceArgs,
  AdapterExecutionEvidenceResult,
  AdapterExecutionEvidenceQueryArgs,
  AdapterExecutionEvidenceQueryResult,
  DomainAcquisitionArgs,
  DomainAcquisitionResult,
  TabularIngestArgs,
  TabularIngestResult,
  ConformanceRunArgs,
  ConformanceRunResult,
  BundleVerifyArgs,
  BundleVerifyResult,
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
  EpistemicAdaptiveArgs,
  EpistemicAdaptiveResult,
  AdaptiveExecutionArgs,
  AdaptiveExecutionResult,
  AdaptiveCostedArgs,
  AdaptiveCostedResult,
  WorkflowExecutionArgs,
  WorkflowExecutionResult,
  WorkflowExecutionEvidenceArgs,
  WorkflowExecutionEvidenceImportArgs,
  WorkflowExecutionEvidenceQueryOptions,
  WorkflowExecutionEvidenceResult,
  EpistemicDecisionQuotientArgs,
  EpistemicDecisionQuotientResult,
  FiberCompileArgs,
  FiberCompileResult,
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
  DeveloperWorkbenchRegistryGetResult,
  DeveloperWorkbenchRegistryImportArgs,
  DeveloperWorkbenchRegistryImportResult,
  DeveloperWorkbenchRegistryQueryArgs,
  DeveloperWorkbenchRegistryQueryResult,
  DeveloperWorkbenchVerificationArgs,
  DeveloperWorkbenchVerificationResult,
  CiProviderNormalizationArgs,
  CiProviderNormalizationResult,
  CiProviderEvidenceArgs,
  CiProviderEvidenceResult,
  CiProviderEvidenceRegistryGetResult,
  CiProviderEvidenceRegistryImportArgs,
  CiProviderEvidenceRegistryImportResult,
  CiProviderEvidenceRegistryQueryArgs,
  CiProviderEvidenceRegistryQueryResult,
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
  ArtifactDomainEvidenceLineageOptions,
  ArtifactDomainEvidenceLineageResult,
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

function validateAdapterExecutionEvidenceArgs(args: AdapterExecutionEvidenceArgs): AdapterExecutionEvidenceArgs {
  if (!isObject(args)) throw new ArgumentError("adapter execution evidence arguments must be an object");
  for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["adapter_id", args.adapter_id], ["adapter_version", args.adapter_version], ["source_id", args.source_id], ["execution_status", args.execution_status], ["conformance_status", args.conformance_status], ["semantic_loss_status", args.semantic_loss_status]] as const) {
    if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
  }
  if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
  if (typeof args.input_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.input_digest)) throw new ArgumentError("input_digest must be a lowercase SHA-256 digest");
  if (args.output_digest !== undefined && args.output_digest !== null && (typeof args.output_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.output_digest))) throw new ArgumentError("output_digest must be a lowercase SHA-256 digest or null");
  const executionStatuses = ["planned", "started", "succeeded", "partial", "refused", "failed", "unknown"];
  const conformanceStatuses = ["verified", "partial", "refused", "not_run", "unknown"];
  const lossStatuses = ["lossless", "lossy", "unknown", "not_applicable"];
  if (!executionStatuses.includes(args.execution_status)) throw new ArgumentError("execution_status is invalid");
  if (!conformanceStatuses.includes(args.conformance_status)) throw new ArgumentError("conformance_status is invalid");
  if (!lossStatuses.includes(args.semantic_loss_status)) throw new ArgumentError("semantic_loss_status is invalid");
  const losses = args.losses ?? [];
  if (!Array.isArray(losses) || losses.length > 128 || losses.some((loss) => !isObject(loss) || typeof loss.kind !== "string" || loss.kind.trim().length === 0 || !["info", "warning", "blocking"].includes(String(loss.severity)) || typeof loss.detail !== "string" || loss.detail.trim().length === 0)) throw new ArgumentError("losses must contain at most 128 valid loss entries");
  if ((args.semantic_loss_status === "lossless" || args.semantic_loss_status === "not_applicable") && losses.length > 0) throw new ArgumentError("lossless or not_applicable evidence cannot contain losses");
  if (args.semantic_loss_status === "lossy" && losses.length === 0) throw new ArgumentError("lossy evidence must contain at least one loss");
  if (args.execution_status === "succeeded" && (typeof args.output_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.output_digest))) throw new ArgumentError("succeeded execution requires output_digest");
  if ((args.execution_status === "refused" || args.execution_status === "failed") && (typeof args.error_code !== "string" || args.error_code.trim().length === 0)) throw new ArgumentError("refused or failed execution requires error_code");
  for (const [name, value, maximum] of [["item_count", args.item_count, 2_000_000], ["byte_length", args.byte_length, 68_719_476_736]] as const) {
    if (value !== undefined && value !== null && (!Number.isSafeInteger(value) || value < 0 || value > maximum)) throw new ArgumentError(`${name} is outside its bound`);
  }
  if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
  if ("credential_material" in args || "credentials" in args) throw new ArgumentError("credential material is not accepted by the adapter evidence boundary");
  return { ...args, losses };
}

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

  /** Plan multiple explicit domain workflows with per-item authoritative no-dispatch preflight. */
  async domainWorkflowPortfolioQuery(
    args: DomainWorkflowPortfolioArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowPortfolioResult> {
    if (!isObject(args) || !Array.isArray(args.requests) || args.requests.length < 1 || args.requests.length > 64 || args.requests.some((request) => !isObject(request))) {
      throw new ArgumentError("workflow portfolio requires 1..=64 request objects");
    }
    if (args.policy !== undefined && !isObject(args.policy)) throw new ArgumentError("workflow portfolio policy must be an object");
    return this.request<DomainWorkflowPortfolioResult>("POST", "/v1/domain-workflows/portfolio", args, options);
  }

  /** Verify a retained multi-domain portfolio, including optional aligned replay. */
  async domainWorkflowPortfolioVerifyQuery(
    args: DomainWorkflowPortfolioVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowPortfolioVerifyResult> {
    if (!isObject(args) || !isObject(args.portfolio)) throw new ArgumentError("workflow portfolio verification requires a portfolio object");
    if (args.replay_requests !== undefined && (!Array.isArray(args.replay_requests) || args.replay_requests.length < 1 || args.replay_requests.length > 64 || args.replay_requests.some((request) => request !== null && !isObject(request)))) {
      throw new ArgumentError("workflow portfolio verification replay_requests must contain 1..=64 objects or nulls");
    }
    if (args.policy !== undefined && !isObject(args.policy)) throw new ArgumentError("workflow portfolio verification policy must be an object");
    const portfolio = { ...args.portfolio };
    delete portfolio.request_id;
    delete portfolio.__isError;
    return this.request<DomainWorkflowPortfolioVerifyResult>("POST", "/v1/domain-workflows/portfolio/verify", { ...args, portfolio }, options);
  }

  /** Verify a retained workflow contract and optionally replay its original request. */
  async domainWorkflowVerifyQuery(
    args: DomainWorkflowVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainWorkflowVerifyResult> {
    if (!isObject(args) || !isObject(args.instantiation)) throw new ArgumentError("workflow verification requires an instantiation object");
    if (args.replay_request !== undefined && !isObject(args.replay_request)) throw new ArgumentError("workflow verification replay_request must be an object");
    return this.request<DomainWorkflowVerifyResult>("POST", "/v1/domain-workflows/verify", args, options);
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
    for (const [name, value] of [["group_id", args.group_id], ["domain", args.domain], ["report_class", args.report_class], ["bridge_mode", args.bridge_mode]] as const) {
      if (value !== undefined && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const maxGroups = args.max_groups ?? 64;
    if (!Number.isSafeInteger(maxGroups) || maxGroups < 1 || maxGroups > 128) throw new ArgumentError("max_groups must be 1..=128");
    if (args.include_report_digests !== undefined && typeof args.include_report_digests !== "boolean") throw new ArgumentError("include_report_digests must be a boolean");
    const query = new URLSearchParams({ max_groups: String(maxGroups), include_report_digests: String(args.include_report_digests ?? false) });
    if (args.group_id !== undefined) query.set("group_id", args.group_id);
    if (args.domain !== undefined) query.set("domain", args.domain);
    if (args.report_class !== undefined) query.set("report_class", args.report_class);
    if (args.bridge_mode !== undefined) query.set("bridge_mode", args.bridge_mode);
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

  /** Query retained harmonization artifacts without returning their full bodies. */
  async domainEvidenceHarmonizationCoverage(
    args: DomainEvidenceHarmonizationCoverageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<DomainEvidenceHarmonizationCoverageResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence harmonization coverage arguments must be an object");
    for (const [name, value] of [["subject_id", args.subject_id], ["domain", args.domain], ["report_class", args.report_class], ["bridge_mode", args.bridge_mode]] as const) {
      if (value !== undefined && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (args.traceability_state !== undefined && !["complete", "requirements_missing", "links_missing"].includes(args.traceability_state)) throw new ArgumentError("traceability_state is invalid");
    if (args.after !== undefined && (typeof args.after !== "string" || !/^[0-9a-f]{64}$/.test(args.after))) throw new ArgumentError("after must be a lowercase SHA-256 digest");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    if (args.include_report_digests !== undefined && typeof args.include_report_digests !== "boolean") throw new ArgumentError("include_report_digests must be a boolean");
    const query = new URLSearchParams({ max_items: String(maxItems), include_report_digests: String(args.include_report_digests ?? false) });
    for (const name of ["subject_id", "domain", "report_class", "bridge_mode", "traceability_state", "after"] as const) {
      const value = args[name];
      if (value !== undefined) query.set(name, value);
    }
    return this.request<DomainEvidenceHarmonizationCoverageResult>("GET", `/v1/domain-evidence/harmonization/coverage?${query.toString()}`, undefined, options);
  }

  /** Invoke retained harmonization coverage through the REST tool dispatcher. */
  async domainEvidenceHarmonizationCoverageTool(
    args: DomainEvidenceHarmonizationCoverageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceHarmonizationCoverageResult>> {
    return this.callTool<DomainEvidenceHarmonizationCoverageResult>("domain_evidence_harmonization_coverage", args, options);
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
    if (args.retrieval_policy !== undefined) {
      if (!isObject(args.retrieval_policy)) throw new ArgumentError("retrieval_policy must be an object");
      const policy = args.retrieval_policy;
      const network = policy.network ?? "caller_managed";
      if (!(network === "disabled" || network === "caller_managed" || network === "enabled")) throw new ArgumentError("retrieval_policy.network is invalid");
      const maxBytes = policy.max_bytes ?? 2 * 1024 * 1024;
      if (typeof maxBytes !== "number" || !Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > 64 * 1024 * 1024) throw new ArgumentError("retrieval_policy.max_bytes is invalid");
      const timeoutMs = policy.timeout_ms ?? 5_000;
      if (typeof timeoutMs !== "number" || !Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 30_000) throw new ArgumentError("retrieval_policy.timeout_ms is invalid");
      const cache = policy.cache ?? "content_addressed";
      if (!(cache === "no_cache" || cache === "content_addressed")) throw new ArgumentError("retrieval_policy.cache is invalid");
      const allowedHosts = policy.allowed_hosts ?? [];
      if (!Array.isArray(allowedHosts) || allowedHosts.length > 32 || allowedHosts.some((host) => typeof host !== "string" || host.trim().length === 0 || /[\r\n\/?#@:\s]/.test(host))) throw new ArgumentError("retrieval_policy.allowed_hosts is invalid");
      if (network === "enabled" && allowedHosts.length === 0) throw new ArgumentError("retrieval_policy.allowed_hosts is required when network is enabled");
    }
    return this.request<DomainEvidenceSourcePlanResult>("POST", "/v1/domain-evidence/sources", args, options);
  }

  /** Invoke external source planning through the REST tool dispatcher. */
  async domainEvidenceSourcePlanTool(
    args: DomainEvidenceSourcePlanArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceSourcePlanResult>> {
    return this.callTool<DomainEvidenceSourcePlanResult>("domain_evidence_source_plan", args, options);
  }

  /** Execute a retained source plan and retain its bounded response as domain evidence. */
  async domainEvidenceSourceExecute(
    args: DomainEvidenceSourceExecutionArgs,
    options?: ClientRequestOptions,
  ): Promise<DomainEvidenceSourceExecutionResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence source execution arguments must be an object");
    if (typeof args.source_plan_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.source_plan_digest)) throw new ArgumentError("source_plan_digest must be a lowercase SHA-256 digest");
    if (args.source_tool !== undefined && args.source_tool !== null && (typeof args.source_tool !== "string" || args.source_tool.trim().length === 0)) throw new ArgumentError("source_tool must be a non-empty string or null");
    if (args.claim_posture !== undefined && !isObject(args.claim_posture)) throw new ArgumentError("claim_posture must be an object");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    return this.request<DomainEvidenceSourceExecutionResult>("POST", "/v1/domain-evidence/sources/execute", args, options);
  }

  /** Invoke retained source execution through the REST tool dispatcher. */
  async domainEvidenceSourceExecuteTool(
    args: DomainEvidenceSourceExecutionArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceSourceExecutionResult>> {
    return this.callTool<DomainEvidenceSourceExecutionResult>("domain_evidence_source_execute", args, options);
  }

  /** Normalize caller-managed provider evidence through the catalogue-bound intake path. */
  async domainEvidenceProviderNormalize(
    args: DomainEvidenceProviderNormalizationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderNormalizationResult>> {
    if (!isObject(args)) throw new ArgumentError("domain evidence provider arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (!(args.connector_kind === "literature" || args.connector_kind === "clinical_trial" || args.connector_kind === "fhir" || args.connector_kind === "object_store" || args.connector_kind === "provider_api")) throw new ArgumentError("connector_kind is invalid");
    if (!isObject(args.payload) && !Array.isArray(args.payload)) throw new ArgumentError("payload must be an object or array");
    if (args.outcome !== undefined && !(args.outcome === "observed" || args.outcome === "partial" || args.outcome === "refused" || args.outcome === "error" || args.outcome === "unknown")) throw new ArgumentError("outcome is invalid");
    if (args.claim_posture !== undefined) {
      if (!isObject(args.claim_posture)) throw new ArgumentError("claim_posture must be an object");
      if (!(args.claim_posture.status === "observed" || args.claim_posture.status === "derived" || args.claim_posture.status === "review_required" || args.claim_posture.status === "refused" || args.claim_posture.status === "not_applicable")) throw new ArgumentError("claim_posture.status is invalid");
      if (!Array.isArray(args.claim_posture.does_not_claim) || args.claim_posture.does_not_claim.length < 1 || args.claim_posture.does_not_claim.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("claim_posture.does_not_claim must be non-empty");
    }
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.source_plan_digest !== undefined && args.source_plan_digest !== null && (typeof args.source_plan_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.source_plan_digest))) throw new ArgumentError("source_plan_digest must be a lowercase SHA-256 digest or null");
    return this.callTool<DomainEvidenceProviderNormalizationResult>("domain_evidence_provider_normalize", { ...args, outcome: args.outcome ?? "unknown" }, options);
  }

  /** Explicit alias for the provider-normalization MCP tool. */
  async domainEvidenceProviderNormalizeTool(
    args: DomainEvidenceProviderNormalizationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderNormalizationResult>> {
    return this.domainEvidenceProviderNormalize(args, options);
  }

  /** Verify a caller-managed provider payload against retained value-free digest identities. */
  async domainEvidenceProviderReplayVerify(
    args: DomainEvidenceProviderReplayVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderReplayVerifyResult>> {
    if (!isObject(args)) throw new ArgumentError("domain evidence provider replay arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (!(args.connector_kind === "literature" || args.connector_kind === "clinical_trial" || args.connector_kind === "fhir" || args.connector_kind === "object_store" || args.connector_kind === "provider_api")) throw new ArgumentError("connector_kind is invalid");
    if (!isObject(args.payload) && !Array.isArray(args.payload)) throw new ArgumentError("payload must be an object or array");
    const expectedDigests = [
      ["expected_payload_digest", args.expected_payload_digest],
      ["expected_shape_digest", args.expected_shape_digest],
      ["expected_normalization_digest", args.expected_normalization_digest],
      ["expected_intake_digest", args.expected_intake_digest],
    ] as const;
    for (const [name, value] of expectedDigests) {
      if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    }
    if (args.expected_request_digest !== undefined && args.expected_request_digest !== null && (typeof args.expected_request_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.expected_request_digest))) throw new ArgumentError("expected_request_digest must be a lowercase SHA-256 digest or null");
    if (args.outcome !== undefined && !(args.outcome === "observed" || args.outcome === "partial" || args.outcome === "refused" || args.outcome === "error" || args.outcome === "unknown")) throw new ArgumentError("outcome is invalid");
    if (args.claim_posture !== undefined) {
      if (!isObject(args.claim_posture)) throw new ArgumentError("claim_posture must be an object");
      if (!(args.claim_posture.status === "observed" || args.claim_posture.status === "derived" || args.claim_posture.status === "review_required" || args.claim_posture.status === "refused" || args.claim_posture.status === "not_applicable")) throw new ArgumentError("claim_posture.status is invalid");
      if (!Array.isArray(args.claim_posture.does_not_claim) || args.claim_posture.does_not_claim.length < 1 || args.claim_posture.does_not_claim.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("claim_posture.does_not_claim must be non-empty");
    }
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.source_plan_digest !== undefined && args.source_plan_digest !== null && (typeof args.source_plan_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.source_plan_digest))) throw new ArgumentError("source_plan_digest must be a lowercase SHA-256 digest or null");
    return this.callTool<DomainEvidenceProviderReplayVerifyResult>("domain_evidence_provider_replay_verify", { ...args, outcome: args.outcome ?? "unknown" }, options);
  }

  /** Explicit alias for the provider-replay verification MCP tool. */
  async domainEvidenceProviderReplayVerifyTool(
    args: DomainEvidenceProviderReplayVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderReplayVerifyResult>> {
    return this.domainEvidenceProviderReplayVerify(args, options);
  }

  /** Declare and retain a caller-managed provider connector boundary before payload intake. */
  async domainEvidenceProviderConnectorHandoff(
    args: DomainEvidenceProviderHandoffArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderHandoffResult>> {
    if (!isObject(args)) throw new ArgumentError("domain evidence provider handoff arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const connectors = ["literature", "clinical_trial", "fhir", "object_store", "provider_api"];
    if (!connectors.includes(args.connector_kind)) throw new ArgumentError("connector_kind is invalid");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (!isObject(args.manifest)) throw new ArgumentError("manifest must be an object");
    const manifest = args.manifest;
    if (manifest.schema !== "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1") throw new ArgumentError("manifest.schema is invalid");
    if (manifest.transport !== "caller_managed") throw new ArgumentError("manifest.transport must be caller_managed");
    if (manifest.provider !== args.provider || manifest.connector_kind !== args.connector_kind) throw new ArgumentError("manifest provider scope does not match handoff");
    if (!Array.isArray(manifest.domains) || manifest.domains.length < 1 || manifest.domains.length > 64 || args.domains.some((domain) => !manifest.domains.includes(domain))) throw new ArgumentError("handoff domains must be covered by manifest.domains");
    if (!Array.isArray(manifest.capabilities) || manifest.capabilities.length < 1 || manifest.capabilities.length > 64 || manifest.capabilities.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("manifest.capabilities must contain 1..=64 strings");
    if (!isObject(manifest.auth_posture)) throw new ArgumentError("manifest.auth_posture must be an object");
    if (!(manifest.auth_posture.status === "none" || manifest.auth_posture.status === "caller_asserted" || manifest.auth_posture.status === "delegated" || manifest.auth_posture.status === "unknown")) throw new ArgumentError("manifest.auth_posture.status is invalid");
    if (manifest.auth_posture.secret_refs !== undefined && (!Array.isArray(manifest.auth_posture.secret_refs) || manifest.auth_posture.secret_refs.length > 32 || manifest.auth_posture.secret_refs.some((item) => typeof item !== "string" || item.trim().length === 0))) throw new ArgumentError("manifest.auth_posture.secret_refs is invalid");
    if (!Array.isArray(manifest.auth_posture.does_not_claim) || manifest.auth_posture.does_not_claim.length < 1 || manifest.auth_posture.does_not_claim.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("manifest.auth_posture.does_not_claim must be non-empty");
    const statuses = ["prepared", "submitted", "observed", "partial", "refused", "error", "unknown"];
    if (args.status !== undefined && !statuses.includes(args.status)) throw new ArgumentError("status is invalid");
    for (const [name, value] of [["request_digest", args.request_digest], ["payload_digest", args.payload_digest], ["source_plan_digest", args.source_plan_digest]] as const) {
      if (value !== undefined && !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    }
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.attempt_id !== undefined && (typeof args.attempt_id !== "string" || args.attempt_id.trim().length === 0)) throw new ArgumentError("attempt_id must be a non-empty string");
    if ("credential_material" in args || "credentials" in args) throw new ArgumentError("credential material is not accepted by the handoff boundary");
    return this.callTool<DomainEvidenceProviderHandoffResult>("domain_evidence_provider_connector_handoff", { ...args, status: args.status ?? "unknown" }, options);
  }

  /** Explicit alias for the connector-handoff MCP tool. */
  async domainEvidenceProviderConnectorHandoffTool(
    args: DomainEvidenceProviderHandoffArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderHandoffResult>> {
    return this.domainEvidenceProviderConnectorHandoff(args, options);
  }

  /** Retain exact metadata for a large provider payload stored outside the MCP core. */
  async domainEvidenceProviderExternalPayloadReceipt(
    args: DomainEvidenceProviderExternalPayloadReceiptArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadReceiptResult>> {
    if (!isObject(args)) throw new ArgumentError("external provider payload receipt arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider], ["handoff_digest", args.handoff_digest], ["transfer_id", args.transfer_id], ["payload_digest", args.payload_digest], ["locator", args.locator]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const connectors = ["literature", "clinical_trial", "fhir", "object_store", "provider_api"];
    if (!connectors.includes(args.connector_kind)) throw new ArgumentError("connector_kind is invalid");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    for (const [name, value] of [["handoff_digest", args.handoff_digest], ["payload_digest", args.payload_digest], ["request_digest", args.request_digest]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
    }
    if (!Number.isSafeInteger(args.byte_length) || args.byte_length < 1 || args.byte_length > 68719476736) throw new ArgumentError("byte_length must be 1..=68719476736");
    if (!(args.storage_backend === "object_store" || args.storage_backend === "file" || args.storage_backend === "database" || args.storage_backend === "caller_managed")) throw new ArgumentError("storage_backend is invalid");
    if (!(args.locator_kind === "opaque" || args.locator_kind === "uri" || args.locator_kind === "path")) throw new ArgumentError("locator_kind is invalid");
    if (args.locator.includes("\r") || args.locator.includes("\n")) throw new ArgumentError("locator must not contain control line breaks");
    const authority = args.locator.split("://")[1]?.split(/[/?#]/, 1)[0];
    if (authority?.includes("@")) throw new ArgumentError("locator must not contain embedded credentials");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.availability !== undefined && !(args.availability === "available" || args.availability === "partial" || args.availability === "missing" || args.availability === "unknown")) throw new ArgumentError("availability is invalid");
    if (args.retention !== undefined && !(args.retention === "ephemeral" || args.retention === "durable" || args.retention === "unknown")) throw new ArgumentError("retention is invalid");
    if (args.attempt_id !== undefined && args.attempt_id !== null && (typeof args.attempt_id !== "string" || args.attempt_id.trim().length === 0)) throw new ArgumentError("attempt_id must be a non-empty string or null");
    if ("payload" in args || "credential_material" in args || "credentials" in args) throw new ArgumentError("payload and credential material are not accepted by the external receipt boundary");
    return this.callTool<DomainEvidenceProviderExternalPayloadReceiptResult>("domain_evidence_provider_external_payload_receipt", { ...args, availability: args.availability ?? "unknown", retention: args.retention ?? "unknown" }, options);
  }

  /** Explicit alias for the external payload receipt MCP tool. */
  async domainEvidenceProviderExternalPayloadReceiptTool(
    args: DomainEvidenceProviderExternalPayloadReceiptArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadReceiptResult>> {
    return this.domainEvidenceProviderExternalPayloadReceipt(args, options);
  }

  /** Verify retained external payload identities without opening the caller-owned locator. */
  async domainEvidenceProviderExternalPayloadReplayVerify(
    args: DomainEvidenceProviderExternalPayloadReplayVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadReplayVerifyResult>> {
    if (!isObject(args)) throw new ArgumentError("external provider payload replay arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider], ["handoff_digest", args.handoff_digest], ["transfer_id", args.transfer_id], ["payload_digest", args.payload_digest], ["locator", args.locator], ["expected_receipt_digest", args.expected_receipt_digest], ["expected_handoff_digest", args.expected_handoff_digest], ["expected_payload_digest", args.expected_payload_digest]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const connectors = ["literature", "clinical_trial", "fhir", "object_store", "provider_api"];
    if (!connectors.includes(args.connector_kind)) throw new ArgumentError("connector_kind is invalid");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    for (const [name, value] of [["handoff_digest", args.handoff_digest], ["payload_digest", args.payload_digest], ["request_digest", args.request_digest], ["expected_receipt_digest", args.expected_receipt_digest], ["expected_handoff_digest", args.expected_handoff_digest], ["expected_payload_digest", args.expected_payload_digest]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
    }
    if (!Number.isSafeInteger(args.byte_length) || args.byte_length < 1 || args.byte_length > 68719476736) throw new ArgumentError("byte_length must be 1..=68719476736");
    if (!Number.isSafeInteger(args.expected_byte_length) || args.expected_byte_length < 1 || args.expected_byte_length > 68719476736) throw new ArgumentError("expected_byte_length must be 1..=68719476736");
    if (!(args.storage_backend === "object_store" || args.storage_backend === "file" || args.storage_backend === "database" || args.storage_backend === "caller_managed")) throw new ArgumentError("storage_backend is invalid");
    if (!(args.locator_kind === "opaque" || args.locator_kind === "uri" || args.locator_kind === "path")) throw new ArgumentError("locator_kind is invalid");
    if (args.locator.includes("\r") || args.locator.includes("\n")) throw new ArgumentError("locator must not contain control line breaks");
    const authority = args.locator.split("://")[1]?.split(/[/?#]/, 1)[0];
    if (authority?.includes("@")) throw new ArgumentError("locator must not contain embedded credentials");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.availability !== undefined && !(args.availability === "available" || args.availability === "partial" || args.availability === "missing" || args.availability === "unknown")) throw new ArgumentError("availability is invalid");
    if (args.retention !== undefined && !(args.retention === "ephemeral" || args.retention === "durable" || args.retention === "unknown")) throw new ArgumentError("retention is invalid");
    if (args.attempt_id !== undefined && args.attempt_id !== null && (typeof args.attempt_id !== "string" || args.attempt_id.trim().length === 0)) throw new ArgumentError("attempt_id must be a non-empty string or null");
    if ("payload" in args || "credential_material" in args || "credentials" in args) throw new ArgumentError("payload and credential material are not accepted by the external replay boundary");
    return this.callTool<DomainEvidenceProviderExternalPayloadReplayVerifyResult>("domain_evidence_provider_external_payload_replay_verify", { ...args, availability: args.availability ?? "unknown", retention: args.retention ?? "unknown" }, options);
  }

  /** Explicit alias for the external payload replay MCP tool. */
  async domainEvidenceProviderExternalPayloadReplayVerifyTool(
    args: DomainEvidenceProviderExternalPayloadReplayVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadReplayVerifyResult>> {
    return this.domainEvidenceProviderExternalPayloadReplayVerify(args, options);
  }

  /** Verify a bounded caller materialization against an external receipt, then normalize it. */
  async domainEvidenceProviderExternalPayloadNormalize(
    args: DomainEvidenceProviderExternalPayloadNormalizationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadNormalizationResult>> {
    if (!isObject(args)) throw new ArgumentError("external provider payload normalization arguments must be an object");
    if (!isObject(args.payload) && !Array.isArray(args.payload)) throw new ArgumentError("payload must be an object or array");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider], ["handoff_digest", args.handoff_digest], ["transfer_id", args.transfer_id], ["payload_digest", args.payload_digest], ["locator", args.locator]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const connectors = ["literature", "clinical_trial", "fhir", "object_store", "provider_api"];
    if (!connectors.includes(args.connector_kind)) throw new ArgumentError("connector_kind is invalid");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    for (const [name, value] of [["handoff_digest", args.handoff_digest], ["payload_digest", args.payload_digest], ["request_digest", args.request_digest], ["source_plan_digest", args.source_plan_digest]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
    }
    if (!Number.isSafeInteger(args.byte_length) || args.byte_length < 1 || args.byte_length > 68719476736) throw new ArgumentError("byte_length must be 1..=68719476736");
    if (!(args.storage_backend === "object_store" || args.storage_backend === "file" || args.storage_backend === "database" || args.storage_backend === "caller_managed")) throw new ArgumentError("storage_backend is invalid");
    if (!(args.locator_kind === "opaque" || args.locator_kind === "uri" || args.locator_kind === "path")) throw new ArgumentError("locator_kind is invalid");
    if (args.locator.includes("\r") || args.locator.includes("\n")) throw new ArgumentError("locator must not contain control line breaks");
    const authority = args.locator.split("://")[1]?.split(/[/?#]/, 1)[0];
    if (authority?.includes("@")) throw new ArgumentError("locator must not contain embedded credentials");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.outcome !== undefined && !(args.outcome === "observed" || args.outcome === "partial" || args.outcome === "refused" || args.outcome === "error" || args.outcome === "unknown")) throw new ArgumentError("outcome is invalid");
    if (args.availability !== undefined && !(args.availability === "available" || args.availability === "partial" || args.availability === "missing" || args.availability === "unknown")) throw new ArgumentError("availability is invalid");
    if (args.retention !== undefined && !(args.retention === "ephemeral" || args.retention === "durable" || args.retention === "unknown")) throw new ArgumentError("retention is invalid");
    if (args.attempt_id !== undefined && args.attempt_id !== null && (typeof args.attempt_id !== "string" || args.attempt_id.trim().length === 0)) throw new ArgumentError("attempt_id must be a non-empty string or null");
    if ("credential_material" in args || "credentials" in args) throw new ArgumentError("credential material is not accepted by the external normalization boundary");
    return this.callTool<DomainEvidenceProviderExternalPayloadNormalizationResult>("domain_evidence_provider_external_payload_normalize", { ...args, availability: args.availability ?? "unknown", retention: args.retention ?? "unknown", outcome: args.outcome ?? "unknown" }, options);
  }

  /** Explicit alias for the external payload normalization MCP tool. */
  async domainEvidenceProviderExternalPayloadNormalizeTool(
    args: DomainEvidenceProviderExternalPayloadNormalizationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadNormalizationResult>> {
    return this.domainEvidenceProviderExternalPayloadNormalize(args, options);
  }

  /** Audit an external receipt against a retained connector handoff without external I/O. */
  async domainEvidenceProviderExternalPayloadLineageAudit(
    args: DomainEvidenceProviderExternalPayloadLineageAuditArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadLineageAuditResult>> {
    if (!isObject(args)) throw new ArgumentError("external provider payload lineage arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider], ["handoff_digest", args.handoff_digest], ["transfer_id", args.transfer_id], ["payload_digest", args.payload_digest], ["locator", args.locator]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!(args.connector_kind === "literature" || args.connector_kind === "clinical_trial" || args.connector_kind === "fhir" || args.connector_kind === "object_store" || args.connector_kind === "provider_api")) throw new ArgumentError("connector_kind is invalid");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    for (const [name, value] of [["handoff_digest", args.handoff_digest], ["payload_digest", args.payload_digest], ["request_digest", args.request_digest]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
    }
    if (!Number.isSafeInteger(args.byte_length) || args.byte_length < 1 || args.byte_length > 68719476736) throw new ArgumentError("byte_length must be 1..=68719476736");
    if (!(args.storage_backend === "object_store" || args.storage_backend === "file" || args.storage_backend === "database" || args.storage_backend === "caller_managed")) throw new ArgumentError("storage_backend is invalid");
    if (!(args.locator_kind === "opaque" || args.locator_kind === "uri" || args.locator_kind === "path")) throw new ArgumentError("locator_kind is invalid");
    if (args.locator.includes("\r") || args.locator.includes("\n")) throw new ArgumentError("locator must not contain control line breaks");
    const authority = args.locator.split("://")[1]?.split(/[/?#]/, 1)[0];
    if (authority?.includes("@")) throw new ArgumentError("locator must not contain embedded credentials");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.availability !== undefined && !(args.availability === "available" || args.availability === "partial" || args.availability === "missing" || args.availability === "unknown")) throw new ArgumentError("availability is invalid");
    if (args.retention !== undefined && !(args.retention === "ephemeral" || args.retention === "durable" || args.retention === "unknown")) throw new ArgumentError("retention is invalid");
    if (args.attempt_id !== undefined && args.attempt_id !== null && (typeof args.attempt_id !== "string" || args.attempt_id.trim().length === 0)) throw new ArgumentError("attempt_id must be a non-empty string or null");
    if ("payload" in args || "credential_material" in args || "credentials" in args) throw new ArgumentError("payload and credential material are not accepted by the external lineage boundary");
    return this.callTool<DomainEvidenceProviderExternalPayloadLineageAuditResult>("domain_evidence_provider_external_payload_lineage_audit", { ...args, availability: args.availability ?? "unknown", retention: args.retention ?? "unknown" }, options);
  }

  /** Explicit alias for the external payload lineage audit MCP tool. */
  async domainEvidenceProviderExternalPayloadLineageAuditTool(
    args: DomainEvidenceProviderExternalPayloadLineageAuditArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadLineageAuditResult>> {
    return this.domainEvidenceProviderExternalPayloadLineageAudit(args, options);
  }

  /** Retain caller-reported transfer observations without executing external I/O. */
  async domainEvidenceProviderExternalPayloadExecutionEvidence(
    args: DomainEvidenceProviderExternalPayloadExecutionEvidenceArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadExecutionEvidenceResult>> {
    if (!isObject(args)) throw new ArgumentError("external payload execution evidence arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["subject_id", args.subject_id], ["source_tool", args.source_tool], ["provider", args.provider], ["handoff_digest", args.handoff_digest], ["transfer_id", args.transfer_id], ["payload_digest", args.payload_digest], ["locator", args.locator], ["expected_receipt_digest", args.expected_receipt_digest], ["execution_status", args.execution_status], ["executor_id", args.executor_id]] as const) {
      if (typeof value !== "string" || value.trim().length === 0) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    if (!(args.connector_kind === "literature" || args.connector_kind === "clinical_trial" || args.connector_kind === "fhir" || args.connector_kind === "object_store" || args.connector_kind === "provider_api")) throw new ArgumentError("connector_kind is invalid");
    if (!(args.execution_status === "submitted" || args.execution_status === "transferred" || args.execution_status === "partial" || args.execution_status === "refused" || args.execution_status === "error" || args.execution_status === "unknown")) throw new ArgumentError("execution_status is invalid");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    for (const [name, value] of [["handoff_digest", args.handoff_digest], ["payload_digest", args.payload_digest], ["request_digest", args.request_digest], ["expected_receipt_digest", args.expected_receipt_digest], ["observed_payload_digest", args.observed_payload_digest], ["observation_digest", args.observation_digest]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest or null`);
    }
    if (!Number.isSafeInteger(args.byte_length) || args.byte_length < 1 || args.byte_length > 68719476736) throw new ArgumentError("byte_length must be 1..=68719476736");
    if (args.observed_byte_length !== undefined && args.observed_byte_length !== null && (!Number.isSafeInteger(args.observed_byte_length) || args.observed_byte_length < 1 || args.observed_byte_length > 68719476736)) throw new ArgumentError("observed_byte_length must be 1..=68719476736 or null");
    if (args.locator_opened !== undefined && typeof args.locator_opened !== "boolean") throw new ArgumentError("locator_opened must be boolean");
    if (!(args.storage_backend === "object_store" || args.storage_backend === "file" || args.storage_backend === "database" || args.storage_backend === "caller_managed")) throw new ArgumentError("storage_backend is invalid");
    if (!(args.locator_kind === "opaque" || args.locator_kind === "uri" || args.locator_kind === "path")) throw new ArgumentError("locator_kind is invalid");
    if (args.locator.includes("\r") || args.locator.includes("\n")) throw new ArgumentError("locator must not contain control line breaks");
    const authority = args.locator.split("://")[1]?.split(/[/?#]/, 1)[0];
    if (authority?.includes("@")) throw new ArgumentError("locator must not contain embedded credentials");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if (args.availability !== undefined && !(args.availability === "available" || args.availability === "partial" || args.availability === "missing" || args.availability === "unknown")) throw new ArgumentError("availability is invalid");
    if (args.retention !== undefined && !(args.retention === "ephemeral" || args.retention === "durable" || args.retention === "unknown")) throw new ArgumentError("retention is invalid");
    if ("payload" in args || "credential_material" in args || "credentials" in args) throw new ArgumentError("payload and credential material are not accepted by the external execution evidence boundary");
    return this.callTool<DomainEvidenceProviderExternalPayloadExecutionEvidenceResult>("domain_evidence_provider_external_payload_execution_evidence", { ...args, availability: args.availability ?? "unknown", retention: args.retention ?? "unknown", locator_opened: args.locator_opened ?? false }, options);
  }

  /** Explicit alias for the external payload execution-evidence MCP tool. */
  async domainEvidenceProviderExternalPayloadExecutionEvidenceTool(
    args: DomainEvidenceProviderExternalPayloadExecutionEvidenceArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadExecutionEvidenceResult>> {
    return this.domainEvidenceProviderExternalPayloadExecutionEvidence(args, options);
  }

  /** Join retained external payload receipts, lineage audits, and execution evidence without external I/O. */
  async domainEvidenceProviderExternalPayloadEvidenceQuery(
    args: DomainEvidenceProviderExternalPayloadEvidenceQueryArgs = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadEvidenceQueryResult>> {
    if (!isObject(args)) throw new ArgumentError("external payload evidence query arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["domain", args.domain], ["subject_id", args.subject_id]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string or null`);
    }
    if (args.after !== undefined && args.after !== null && (typeof args.after !== "string" || !/^[0-9a-f]{64}$/.test(args.after))) throw new ArgumentError("after must be a lowercase SHA-256 digest or null");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 128) throw new ArgumentError("max_items must be 1..=128");
    if (args.include_artifacts !== undefined && typeof args.include_artifacts !== "boolean") throw new ArgumentError("include_artifacts must be a boolean");
    if ("credential_material" in args || "credentials" in args) throw new ArgumentError("credential material is not accepted by the external evidence query boundary");
    return this.callTool<DomainEvidenceProviderExternalPayloadEvidenceQueryResult>("domain_evidence_provider_external_payload_evidence_query", { ...args, max_items: maxItems, include_artifacts: args.include_artifacts ?? false }, options);
  }

  /** Explicit alias for the joined external payload evidence query MCP tool. */
  async domainEvidenceProviderExternalPayloadEvidenceQueryTool(
    args: DomainEvidenceProviderExternalPayloadEvidenceQueryArgs = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainEvidenceProviderExternalPayloadEvidenceQueryResult>> {
    return this.domainEvidenceProviderExternalPayloadEvidenceQuery(args, options);
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

  async epistemicAdaptiveAcquisition(args: EpistemicAdaptiveArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EpistemicAdaptiveResult>> {
    return this.callTool<EpistemicAdaptiveResult>("epistemic_adaptive_acquisition", args, options);
  }

  async epistemicAdaptiveExecute(args: AdaptiveExecutionArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdaptiveExecutionResult>> {
    if (!isObject(args)) throw new ArgumentError("adaptive execution arguments must be an object");
    if (!Number.isFinite(args.budget) || args.budget < 0) throw new ArgumentError("budget must be a finite non-negative number");
    if (!Number.isSafeInteger(args.max_steps) || args.max_steps < 0 || args.max_steps > 16) throw new ArgumentError("max_steps must be 0..=16");
    if (!Array.isArray(args.acquisitions) || args.acquisitions.length < 1 || args.acquisitions.length > 16) throw new ArgumentError("acquisitions must contain 1..=16 items");
    if (args.mode !== undefined && args.mode !== "simulate" && args.mode !== "replay") throw new ArgumentError("mode must be simulate or replay");
    return this.callTool<AdaptiveExecutionResult>("epistemic_adaptive_execute", args, options);
  }

  async epistemicAdaptiveCosted(args: AdaptiveCostedArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdaptiveCostedResult>> {
    if (!isObject(args)) throw new ArgumentError("adaptive costed arguments must be an object");
    if (!Number.isSafeInteger(args.max_steps) || args.max_steps < 0 || args.max_steps > 16) throw new ArgumentError("max_steps must be 0..=16");
    if (!Array.isArray(args.acquisitions) || args.acquisitions.length < 1 || args.acquisitions.length > 16) throw new ArgumentError("acquisitions must contain 1..=16 items");
    for (const [name, vector] of [["budget", args.budget], ["weights", args.weights]] as const) {
      if (!isObject(vector)) throw new ArgumentError(`${name} must be a seven-dimensional object`);
      const values = [vector.tokens, vector.compute_ms, vector.latency_ms, vector.money_usd, vector.privacy_loss, vector.specimen_units, vector.expert_minutes];
      if (values.some((value) => typeof value !== "number" || !Number.isFinite(value) || value < 0)) throw new ArgumentError(`${name} dimensions must be finite and non-negative`);
    }
    if ([args.weights.tokens, args.weights.compute_ms, args.weights.latency_ms, args.weights.money_usd, args.weights.privacy_loss, args.weights.specimen_units, args.weights.expert_minutes].every((value) => value === 0)) throw new ArgumentError("weights must contain at least one positive dimension");
    return this.callTool<AdaptiveCostedResult>("epistemic_adaptive_costed", args, options);
  }

  async interweaveWorkflowExecute(args: WorkflowExecutionArgs, options?: ClientRequestOptions): Promise<RestToolResponse<WorkflowExecutionResult>> {
    if (!isObject(args)) throw new ArgumentError("workflow execution arguments must be an object");
    const workflows = ["reliable_software_repair", "scientific_claim_reproduction", "biomedical_research_data_audit", "incident_response", "evidence_grounded_policy_comparison", "dataset_transformation_molecule"];
    if (!workflows.includes(args.workflow)) throw new ArgumentError("workflow must be one of the six reference workflow ids");
    if (!isObject(args.problem) || !isObject(args.belief)) throw new ArgumentError("problem and belief must be objects");
    if (!Number.isFinite(args.budget) || args.budget < 0) throw new ArgumentError("budget must be a finite non-negative number");
    if (!Number.isSafeInteger(args.max_steps) || args.max_steps < 0 || args.max_steps > 16) throw new ArgumentError("max_steps must be 0..=16");
    if (!Array.isArray(args.acquisitions) || args.acquisitions.length < 1 || args.acquisitions.length > 16) throw new ArgumentError("acquisitions must contain 1..=16 items");
    if (args.mode !== undefined && args.mode !== "simulate" && args.mode !== "replay") throw new ArgumentError("mode must be simulate or replay");
    if (args.provider !== undefined && (typeof args.provider !== "string" || args.provider.trim().length === 0)) throw new ArgumentError("provider must be a non-empty string");
    if (args.capabilities !== undefined && (!Array.isArray(args.capabilities) || args.capabilities.length > 32 || args.capabilities.some((item) => typeof item !== "string" || item.trim().length === 0))) throw new ArgumentError("capabilities must contain at most 32 non-empty strings");
    if (args.observations !== undefined && (!Array.isArray(args.observations) || args.observations.length > 16 || args.observations.some((item) => !isObject(item) || typeof item.acquisition_id !== "string" || typeof item.outcome_label !== "string"))) throw new ArgumentError("observations must contain at most 16 typed rows");
    if (args.authorization !== undefined && (!isObject(args.authorization) || typeof args.authorization.grant_id !== "string" || typeof args.authorization.provider !== "string")) throw new ArgumentError("authorization must contain grant_id and provider");
    if (args.mode === "replay" && !isObject(args.receipt)) throw new ArgumentError("receipt is required in replay mode");
    if (args.evidence !== undefined && (!isObject(args.evidence) || typeof args.evidence.subject_id !== "string" || args.evidence.subject_id.trim().length === 0 || !Array.isArray(args.evidence.domains) || args.evidence.domains.length < 1 || args.evidence.domains.length > 64 || args.evidence.domains.some((item) => typeof item !== "string" || item.trim().length === 0))) throw new ArgumentError("evidence must contain subject_id and 1..=64 non-empty domains");
    return this.callTool<WorkflowExecutionResult>("interweave_workflow_execute", args, options);
  }

  async interweaveWorkflowExecutionEvidence(args: WorkflowExecutionEvidenceArgs, options?: ClientRequestOptions): Promise<RestToolResponse<WorkflowExecutionEvidenceResult>> {
    if (!isObject(args)) throw new ArgumentError("workflow execution evidence arguments must be an object");
    if (!isObject(args.binding) || !isObject(args.receipt)) throw new ArgumentError("binding and receipt must be objects");
    if (typeof args.subject_id !== "string" || args.subject_id.trim().length === 0) throw new ArgumentError("subject_id must be a non-empty string");
    if (!Array.isArray(args.domains) || args.domains.length < 1 || args.domains.length > 64 || args.domains.some((item) => typeof item !== "string" || item.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (args.parent_digests !== undefined && (!Array.isArray(args.parent_digests) || args.parent_digests.length > 128 || args.parent_digests.some((item) => typeof item !== "string" || !/^[0-9a-f]{64}$/.test(item)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    return this.callTool<WorkflowExecutionEvidenceResult>("interweave_workflow_execution_evidence", args, options);
  }

  async interweaveWorkflowExecutionEvidenceImport(args: WorkflowExecutionEvidenceImportArgs, options?: ClientRequestOptions): Promise<RestToolResponse<WorkflowExecutionEvidenceResult>> {
    if (!isObject(args) || !isObject(args.evidence)) throw new ArgumentError("evidence import arguments must contain an evidence object");
    return this.callTool<WorkflowExecutionEvidenceResult>("interweave_workflow_execution_evidence_import", args, options);
  }

  async interweaveWorkflowExecutionEvidenceQuery(args: WorkflowExecutionEvidenceQueryOptions = {}, options?: ClientRequestOptions): Promise<RestToolResponse<WorkflowExecutionEvidenceResult>> {
    if (!isObject(args)) throw new ArgumentError("workflow execution evidence query arguments must be an object");
    if (args.max_items !== undefined && (!Number.isSafeInteger(args.max_items) || args.max_items < 1 || args.max_items > 256)) throw new ArgumentError("max_items must be 1..=256");
    for (const [name, value] of [["plan_digest", args.plan_digest], ["binding_digest", args.binding_digest], ["after", args.after]] as const) {
      if (value !== undefined && !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    }
    return this.callTool<WorkflowExecutionEvidenceResult>("interweave_workflow_execution_evidence_query", args, options);
  }

  async interweaveWorkflowExecutionEvidenceGet(evidenceDigest: string, options?: ClientRequestOptions): Promise<RestToolResponse<WorkflowExecutionEvidenceResult>> {
    if (typeof evidenceDigest !== "string" || !/^[0-9a-f]{64}$/.test(evidenceDigest)) throw new ArgumentError("evidenceDigest must be a lowercase SHA-256 digest");
    return this.callTool<WorkflowExecutionEvidenceResult>("interweave_workflow_execution_evidence_get", { evidence_digest: evidenceDigest }, options);
  }

  async epistemicDecisionQuotient(args: EpistemicDecisionQuotientArgs, options?: ClientRequestOptions): Promise<RestToolResponse<EpistemicDecisionQuotientResult>> {
    return this.callTool<EpistemicDecisionQuotientResult>("epistemic_decision_quotient", args, options);
  }

  /** Compile a FIBER query; 0.3/0.4 responses carry typed decision projections at L0. */
  async fiberCompile(args: FiberCompileArgs, options?: ClientRequestOptions): Promise<RestToolResponse<FiberCompileResult>> {
    return this.callTool<FiberCompileResult>("fiber_compile", args, options);
  }

  /** Compile a FIBER query whose 0.4 contract carries observed rate-distortion inputs. */
  async fiberCompileRateDistortion(args: FiberCompileArgs, options?: ClientRequestOptions): Promise<RestToolResponse<FiberCompileResult>> {
    return this.callTool<FiberCompileResult>("fiber_compile", args, options);
  }

  /** Compile a FIBER 0.5 query and expose its certificate-bound adaptive policy projection. */
  async fiberCompileAdaptiveAcquisition(args: FiberCompileArgs, options?: ClientRequestOptions): Promise<RestToolResponse<FiberCompileResult>> {
    return this.callTool<FiberCompileResult>("fiber_compile", args, options);
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

  /** Verify a retained authoring/notebook report, with optional deterministic CI-plan replay. */
  async developerWorkbenchVerifyQuery(
    args: DeveloperWorkbenchVerificationArgs,
    options?: ClientRequestOptions,
  ): Promise<DeveloperWorkbenchVerificationResult> {
    if (!isObject(args) || !isObject(args.session) || !Object.keys(args.session).length || !isObject(args.report) || !Object.keys(args.report).length) {
      throw new ArgumentError("developer workbench verification requires session and report objects");
    }
    if (args.expected_report_digest !== undefined && (typeof args.expected_report_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.expected_report_digest))) {
      throw new ArgumentError("expected_report_digest must be a lowercase SHA-256 digest");
    }
    if (args.ci_replay !== undefined && !isObject(args.ci_replay)) throw new ArgumentError("ci_replay must be an object");
    if (args.policy !== undefined && !isObject(args.policy)) throw new ArgumentError("workbench verification policy must be an object");
    return this.request<DeveloperWorkbenchVerificationResult>("POST", "/v1/developer-workbench/verify", args, options);
  }

  /** MCP envelope form of developerWorkbenchVerifyQuery. */
  async developerWorkbenchVerify(
    args: DeveloperWorkbenchVerificationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DeveloperWorkbenchVerificationResult>> {
    return this.callTool<DeveloperWorkbenchVerificationResult>("developer_workbench_verify", args, options);
  }

  async developerWorkbenchImportRest(
    args: DeveloperWorkbenchRegistryImportArgs,
    options?: ClientRequestOptions,
  ): Promise<DeveloperWorkbenchRegistryImportResult> {
    if (!isObject(args) || !isObject(args.report) || !Object.keys(args.report).length) {
      throw new ArgumentError("developer workbench registry import requires a non-empty report object");
    }
    return this.request<DeveloperWorkbenchRegistryImportResult>("POST", "/v1/developer-workbench/reports", args, options);
  }

  async developerWorkbenchImport(
    args: DeveloperWorkbenchRegistryImportArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DeveloperWorkbenchRegistryImportResult>> {
    return this.callTool<DeveloperWorkbenchRegistryImportResult>("developer_workbench_import", args, options);
  }

  async developerWorkbenchQueryRest(
    args: DeveloperWorkbenchRegistryQueryArgs = {},
    options?: ClientRequestOptions,
  ): Promise<DeveloperWorkbenchRegistryQueryResult> {
    if (!isObject(args)) throw new ArgumentError("developer workbench registry query arguments must be an object");
    if (args.session_digest !== undefined && (typeof args.session_digest !== "string" || !/^[0-9a-f]{64}$/.test(args.session_digest))) throw new ArgumentError("session_digest must be a lowercase SHA-256 digest");
    if (args.after !== undefined && (typeof args.after !== "string" || !/^[0-9a-f]{64}$/.test(args.after))) throw new ArgumentError("after must be a lowercase SHA-256 digest");
    if (args.max_items !== undefined && (!Number.isInteger(args.max_items) || args.max_items < 1 || args.max_items > 256)) throw new ArgumentError("max_items must be between 1 and 256");
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries({ ...args, max_items: args.max_items ?? 100, include_reports: args.include_reports ?? false })) {
      if (value !== undefined) params.set(key, String(value));
    }
    return this.request<DeveloperWorkbenchRegistryQueryResult>("GET", `/v1/developer-workbench/reports?${params.toString()}`, undefined, options);
  }

  async developerWorkbenchQuery(
    args: DeveloperWorkbenchRegistryQueryArgs = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DeveloperWorkbenchRegistryQueryResult>> {
    return this.callTool<DeveloperWorkbenchRegistryQueryResult>("developer_workbench_query", args, options);
  }

  async developerWorkbenchGetRest(
    digest: string,
    options?: ClientRequestOptions,
  ): Promise<DeveloperWorkbenchRegistryGetResult> {
    if (!/^[0-9a-f]{64}$/.test(digest)) throw new ArgumentError("workbench_report_digest must be a lowercase SHA-256 digest");
    return this.request<DeveloperWorkbenchRegistryGetResult>("GET", `/v1/developer-workbench/reports/${encodeURIComponent(digest)}`, undefined, options);
  }

  async developerWorkbenchGet(
    digest: string,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DeveloperWorkbenchRegistryGetResult>> {
    return this.callTool<DeveloperWorkbenchRegistryGetResult>("developer_workbench_get", { workbench_report_digest: digest }, options);
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

  async ciProviderEvidenceImportRest(
    args: CiProviderEvidenceRegistryImportArgs,
    options?: ClientRequestOptions,
  ): Promise<CiProviderEvidenceRegistryImportResult> {
    if (!isObject(args) || !isObject(args.ci) || !isObject(args.payload)) {
      throw new ArgumentError("CI provider evidence import requires ci and payload objects");
    }
    return this.request<CiProviderEvidenceRegistryImportResult>("POST", "/v1/ci/provider-evidence", args, options);
  }

  async ciProviderEvidenceImport(
    args: CiProviderEvidenceRegistryImportArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<CiProviderEvidenceRegistryImportResult>> {
    return this.callTool<CiProviderEvidenceRegistryImportResult>("ci_provider_evidence_import", args, options);
  }

  async ciProviderEvidenceQueryRest(
    args: CiProviderEvidenceRegistryQueryArgs = {},
    options?: ClientRequestOptions,
  ): Promise<CiProviderEvidenceRegistryQueryResult> {
    if (!isObject(args)) throw new ArgumentError("CI provider evidence query arguments must be an object");
    for (const field of ["plan_digest", "after"]) {
      const value = args[field];
      if (value !== undefined && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) {
        throw new ArgumentError(`${field} must be a lowercase SHA-256 digest`);
      }
    }
    if (args.max_items !== undefined && (!Number.isInteger(args.max_items) || args.max_items < 1 || args.max_items > 256)) {
      throw new ArgumentError("max_items must be between 1 and 256");
    }
    for (const field of ["min_local_byte_hash_artifacts", "min_local_byte_hash_logs", "min_attestation_subject_digest_bindings"] as const) {
      const value = args[field];
      if (value !== undefined && (!Number.isInteger(value) || value < 0 || value > 128)) {
        throw new ArgumentError(`${field} must be an integer between 0 and 128`);
      }
    }
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries({ ...args, max_items: args.max_items ?? 100, include_records: args.include_records ?? false })) {
      if (value !== undefined) params.set(key, String(value));
    }
    return this.request<CiProviderEvidenceRegistryQueryResult>("GET", `/v1/ci/provider-evidence?${params.toString()}`, undefined, options);
  }

  async ciProviderEvidenceQuery(
    args: CiProviderEvidenceRegistryQueryArgs = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<CiProviderEvidenceRegistryQueryResult>> {
    return this.callTool<CiProviderEvidenceRegistryQueryResult>("ci_provider_evidence_query", args, options);
  }

  async ciProviderEvidenceGetRest(
    digest: string,
    options?: ClientRequestOptions,
  ): Promise<CiProviderEvidenceRegistryGetResult> {
    if (!/^[0-9a-f]{64}$/.test(digest)) throw new ArgumentError("provider_evidence_digest must be a lowercase SHA-256 digest");
    return this.request<CiProviderEvidenceRegistryGetResult>("GET", `/v1/ci/provider-evidence/${encodeURIComponent(digest)}`, undefined, options);
  }

  async ciProviderEvidenceGet(
    digest: string,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<CiProviderEvidenceRegistryGetResult>> {
    if (!/^[0-9a-f]{64}$/.test(digest)) throw new ArgumentError("provider_evidence_digest must be a lowercase SHA-256 digest");
    return this.callTool<CiProviderEvidenceRegistryGetResult>("ci_provider_evidence_get", { provider_evidence_digest: digest }, options);
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

  /** Plan multiple explicit domain workflows through the MCP tool boundary. */
  async domainWorkflowPortfolio(
    args: DomainWorkflowPortfolioArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowPortfolioResult>> {
    if (!isObject(args) || !Array.isArray(args.requests) || args.requests.length < 1 || args.requests.length > 64 || args.requests.some((request) => !isObject(request))) {
      throw new ArgumentError("workflow portfolio requires 1..=64 request objects");
    }
    if (args.policy !== undefined && !isObject(args.policy)) throw new ArgumentError("workflow portfolio policy must be an object");
    return this.callTool<DomainWorkflowPortfolioResult>("domain_workflow_portfolio", args, options);
  }

  /** Verify a retained multi-domain portfolio through the MCP tool boundary. */
  async domainWorkflowPortfolioVerify(
    args: DomainWorkflowPortfolioVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowPortfolioVerifyResult>> {
    if (!isObject(args) || !isObject(args.portfolio)) throw new ArgumentError("workflow portfolio verification requires a portfolio object");
    if (args.replay_requests !== undefined && (!Array.isArray(args.replay_requests) || args.replay_requests.length < 1 || args.replay_requests.length > 64 || args.replay_requests.some((request) => request !== null && !isObject(request)))) {
      throw new ArgumentError("workflow portfolio verification replay_requests must contain 1..=64 objects or nulls");
    }
    if (args.policy !== undefined && !isObject(args.policy)) throw new ArgumentError("workflow portfolio verification policy must be an object");
    const portfolio = { ...args.portfolio };
    delete portfolio.request_id;
    delete portfolio.__isError;
    return this.callTool<DomainWorkflowPortfolioVerifyResult>("domain_workflow_portfolio_verify", { ...args, portfolio }, options);
  }

  /** Verify a retained workflow contract through the MCP tool boundary. */
  async domainWorkflowVerify(
    args: DomainWorkflowVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<DomainWorkflowVerifyResult>> {
    if (!isObject(args) || !isObject(args.instantiation)) throw new ArgumentError("workflow verification requires an instantiation object");
    if (args.replay_request !== undefined && !isObject(args.replay_request)) throw new ArgumentError("workflow verification replay_request must be an object");
    return this.callTool<DomainWorkflowVerifyResult>("domain_workflow_verify", args, options);
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

  async domainEvidenceLineage(
    args: ArtifactDomainEvidenceLineageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<ArtifactDomainEvidenceLineageResult> {
    if (!isObject(args)) throw new ArgumentError("domain evidence lineage arguments must be an object");
    const stringFields = ["content_digest", "group_id", "domain", "subject_id", "source_tool", "outcome", "request_digest", "response_digest", "intake_digest", "source_plan_digest", "after"] as const;
    for (const name of stringFields) {
      const value = args[name];
      if (value !== undefined && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    if (args.include_children !== undefined && typeof args.include_children !== "boolean") throw new ArgumentError("include_children must be a boolean");
    const query = new URLSearchParams({ max_items: String(maxItems), include_children: String(args.include_children ?? true) });
    for (const name of stringFields) {
      const value = args[name];
      if (value !== undefined) query.set(name, value);
    }
    return this.request<ArtifactDomainEvidenceLineageResult>("GET", `/v1/domain-evidence/lineage?${query.toString()}`, undefined, options);
  }

  async artifactDomainEvidenceLineageTool(
    args: ArtifactDomainEvidenceLineageOptions = {},
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<ArtifactDomainEvidenceLineageResult>> {
    if (!isObject(args)) throw new ArgumentError("domain evidence lineage arguments must be an object");
    const maxItems = args.max_items ?? 100;
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > 256) throw new ArgumentError("max_items must be 1..=256");
    return this.callTool<ArtifactDomainEvidenceLineageResult>("artifact_registry_audit", { operation: "domain_evidence_lineage", ...args, max_items: maxItems }, options);
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

  /** Query the capability dashboard through its cache-friendly direct REST route. */
  async capabilityDashboardQuery(args: CapabilityDashboardArgs = {}, options?: ClientRequestOptions): Promise<CapabilityDashboardResult> {
    if (!isObject(args)) throw new ArgumentError("capability dashboard arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["domain", args.domain], ["status", args.status]] as const) {
      if (value !== undefined && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string`);
    }
    const maxGroups = args.max_groups ?? 128;
    if (!Number.isSafeInteger(maxGroups) || maxGroups < 1 || maxGroups > 512) throw new ArgumentError("max_groups must be 1..=512");
    if (args.include_tools !== undefined && typeof args.include_tools !== "boolean") throw new ArgumentError("include_tools must be a boolean");
    if (args.include_gaps !== undefined && typeof args.include_gaps !== "boolean") throw new ArgumentError("include_gaps must be a boolean");
    const query = new URLSearchParams({ max_groups: String(maxGroups), include_tools: String(args.include_tools ?? false), include_gaps: String(args.include_gaps ?? true) });
    for (const name of ["group_id", "domain", "status"] as const) {
      const value = args[name];
      if (value !== undefined) query.set(name, value);
    }
    return this.request<CapabilityDashboardResult>("GET", `/v1/capabilities/dashboard?${query.toString()}`, undefined, options);
  }

  async capabilityRoute(args: CapabilityRouteArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityRouteResult>> {
    return this.callTool<CapabilityRouteResult>("capability_route", args, options);
  }

  /** Submit a non-executing capability route through the raw REST handoff. */
  async capabilityRouteRest(args: CapabilityRouteArgs, options?: ClientRequestOptions): Promise<CapabilityRouteResult> {
    if (!isObject(args)) throw new ArgumentError("capability route arguments must be an object");
    return this.request<CapabilityRouteResult>("POST", "/v1/capabilities/route", args, options);
  }

  async capabilityRouteReview(args: CapabilityRouteReviewArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityRouteReviewResult>> {
    return this.callTool<CapabilityRouteReviewResult>("capability_route_review", args, options);
  }

  /** Review explicit route selections through the raw REST handoff. */
  async capabilityRouteReviewRest(args: CapabilityRouteReviewArgs, options?: ClientRequestOptions): Promise<CapabilityRouteReviewResult> {
    if (!isObject(args)) throw new ArgumentError("capability route review arguments must be an object");
    return this.request<CapabilityRouteReviewResult>("POST", "/v1/capabilities/route/review", args, options);
  }

  async capabilityRoutePlan(args: CapabilityRoutePlanArgs, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityRoutePlanResult>> {
    return this.callTool<CapabilityRoutePlanResult>("capability_route_plan", args, options);
  }

  /** Compose an explicit route review with authoritative non-executing mission preflight. */
  async capabilityRoutePlanRest(args: CapabilityRoutePlanArgs, options?: ClientRequestOptions): Promise<CapabilityRoutePlanResult> {
    if (!isObject(args)) throw new ArgumentError("capability route plan arguments must be an object");
    return this.request<CapabilityRoutePlanResult>("POST", "/v1/capabilities/route/plan", args, options);
  }

  /** Verify a retained route plan through MCP without dispatch or execution. */
  async capabilityRoutePlanVerify(
    args: CapabilityRoutePlanVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<CapabilityRoutePlanVerifyResult>> {
    if (!isObject(args)) throw new ArgumentError("capability route plan verification arguments must be an object");
    if (!isObject(args.plan)) throw new ArgumentError("capability route plan verification plan must be an object");
    if ((args.route === undefined) !== (args.selections === undefined)) throw new ArgumentError("route and selections must be supplied together");
    if (args.route !== undefined && !isObject(args.route)) throw new ArgumentError("capability route plan verification route must be an object");
    if (args.selections !== undefined && (!Array.isArray(args.selections) || args.selections.length < 1 || args.selections.length > 128)) throw new ArgumentError("capability route plan verification selections must contain 1..=128 choices");
    if (args.validate_schemas !== undefined && typeof args.validate_schemas !== "boolean") throw new ArgumentError("validate_schemas must be a boolean");
    return this.callTool<CapabilityRoutePlanVerifyResult>("capability_route_plan_verify", args, options);
  }

  /** Verify a retained route plan through the dedicated REST endpoint. */
  async capabilityRoutePlanVerifyRest(
    args: CapabilityRoutePlanVerifyArgs,
    options?: ClientRequestOptions,
  ): Promise<CapabilityRoutePlanVerifyResult> {
    if (!isObject(args)) throw new ArgumentError("capability route plan verification arguments must be an object");
    if (!isObject(args.plan)) throw new ArgumentError("capability route plan verification plan must be an object");
    if ((args.route === undefined) !== (args.selections === undefined)) throw new ArgumentError("route and selections must be supplied together");
    if (args.route !== undefined && !isObject(args.route)) throw new ArgumentError("capability route plan verification route must be an object");
    if (args.selections !== undefined && (!Array.isArray(args.selections) || args.selections.length < 1 || args.selections.length > 128)) throw new ArgumentError("capability route plan verification selections must contain 1..=128 choices");
    if (args.validate_schemas !== undefined && typeof args.validate_schemas !== "boolean") throw new ArgumentError("validate_schemas must be a boolean");
    return this.request<CapabilityRoutePlanVerifyResult>("POST", "/v1/capabilities/route/plan/verify", args, options);
  }

  async adapterPlan(args: AdapterPlanArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdapterPlanResult>> {
    return this.callTool<AdapterPlanResult>("adapter_plan", args, options);
  }

  async adapterExecutionEvidence(args: AdapterExecutionEvidenceArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdapterExecutionEvidenceResult>> {
    return this.callTool<AdapterExecutionEvidenceResult>("adapter_execution_evidence", validateAdapterExecutionEvidenceArgs(args), options);
  }

  async adapterExecutionEvidenceTool(args: AdapterExecutionEvidenceArgs, options?: ClientRequestOptions): Promise<RestToolResponse<AdapterExecutionEvidenceResult>> {
    return this.adapterExecutionEvidence(args, options);
  }

  /** Validate and compose caller-owned adapter evidence into a canonical domain report. */
  async domainReportFromAdapterExecution(
    evidence: AdapterExecutionEvidenceArgs,
    conformance?: JsonObject,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<AdapterDomainReportResult>> {
    const normalized = validateAdapterExecutionEvidenceArgs(evidence);
    if (conformance !== undefined && !isObject(conformance)) throw new ArgumentError("conformance must be an object");
    const args: AdapterDomainReportArgs = {
      operation: "from_adapter_execution",
      evidence: normalized,
    };
    if (conformance !== undefined) args.conformance = conformance;
    return this.callTool<AdapterDomainReportResult>("domain_report_project", args, options);
  }

  /** Validate and compose inline provider normalization into a canonical domain report. */
  async domainReportFromProviderNormalization(
    normalization: DomainEvidenceProviderNormalizationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<ProviderDomainReportResult>> {
    if (!isObject(normalization)) throw new ArgumentError("provider normalization arguments must be an object");
    if (typeof normalization.group_id !== "string" || normalization.group_id.trim().length === 0) throw new ArgumentError("group_id must be a non-empty string");
    if (!Array.isArray(normalization.domains) || normalization.domains.length < 1 || normalization.domains.length > 64 || normalization.domains.some((domain) => typeof domain !== "string" || domain.trim().length === 0)) throw new ArgumentError("domains must contain 1..=64 non-empty strings");
    if (!isObject(normalization.payload) && !Array.isArray(normalization.payload)) throw new ArgumentError("payload must be an object or array");
    if (normalization.parent_digests !== undefined && (!Array.isArray(normalization.parent_digests) || normalization.parent_digests.length > 128 || normalization.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    const args: ProviderDomainReportArgs = {
      operation: "from_provider_normalization",
      normalization: { ...normalization, outcome: normalization.outcome ?? "unknown" },
    };
    if (normalization.parent_digests !== undefined) args.parent_digests = normalization.parent_digests;
    return this.callTool<ProviderDomainReportResult>("domain_report_project", args, options);
  }

  /** Validate and compose receipt-verified external provider normalization into a report. */
  async domainReportFromExternalProviderNormalization(
    normalization: DomainEvidenceProviderExternalPayloadNormalizationArgs,
    options?: ClientRequestOptions,
  ): Promise<RestToolResponse<ProviderDomainReportResult>> {
    if (!isObject(normalization)) throw new ArgumentError("external provider normalization arguments must be an object");
    if (!isObject(normalization.payload) && !Array.isArray(normalization.payload)) throw new ArgumentError("payload must be an object or array");
    if (normalization.parent_digests !== undefined && (!Array.isArray(normalization.parent_digests) || normalization.parent_digests.length > 128 || normalization.parent_digests.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)))) throw new ArgumentError("parent_digests must contain at most 128 lowercase SHA-256 digests");
    if ("credential_material" in normalization || "credentials" in normalization) throw new ArgumentError("credential material is not accepted by the external normalization boundary");
    const args: ProviderDomainReportArgs = {
      operation: "from_external_provider_normalization",
      normalization: {
        ...normalization,
        availability: normalization.availability ?? "unknown",
        retention: normalization.retention ?? "unknown",
        outcome: normalization.outcome ?? "unknown",
      },
    };
    if (normalization.parent_digests !== undefined) args.parent_digests = normalization.parent_digests;
    return this.callTool<ProviderDomainReportResult>("domain_report_project", args, options);
  }

  async adapterExecutionEvidenceQuery(args: AdapterExecutionEvidenceQueryArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<AdapterExecutionEvidenceQueryResult>> {
    if (!isObject(args)) throw new ArgumentError("adapter execution evidence query arguments must be an object");
    for (const [name, value] of [["group_id", args.group_id], ["domain", args.domain], ["subject_id", args.subject_id], ["adapter_id", args.adapter_id], ["source_id", args.source_id]] as const) {
      if (value !== undefined && value !== null && (typeof value !== "string" || value.trim().length === 0)) throw new ArgumentError(`${name} must be a non-empty string or null`);
    }
    const executionStatuses = ["planned", "started", "succeeded", "partial", "refused", "failed", "unknown"];
    const conformanceStatuses = ["verified", "partial", "refused", "not_run", "unknown"];
    const lossStatuses = ["lossless", "lossy", "unknown", "not_applicable"];
    if (args.execution_status !== undefined && args.execution_status !== null && !executionStatuses.includes(args.execution_status)) throw new ArgumentError("execution_status is invalid");
    if (args.conformance_status !== undefined && args.conformance_status !== null && !conformanceStatuses.includes(args.conformance_status)) throw new ArgumentError("conformance_status is invalid");
    if (args.semantic_loss_status !== undefined && args.semantic_loss_status !== null && !lossStatuses.includes(args.semantic_loss_status)) throw new ArgumentError("semantic_loss_status is invalid");
    if (args.after !== undefined && args.after !== null && (typeof args.after !== "string" || !/^[0-9a-f]{64}$/.test(args.after))) throw new ArgumentError("after must be a lowercase SHA-256 digest or null");
    if (args.max_items !== undefined && (!Number.isSafeInteger(args.max_items) || args.max_items < 1 || args.max_items > 128)) throw new ArgumentError("max_items must be between 1 and 128");
    if (args.include_artifacts !== undefined && typeof args.include_artifacts !== "boolean") throw new ArgumentError("include_artifacts must be boolean");
    return this.callTool<AdapterExecutionEvidenceQueryResult>("adapter_execution_evidence_query", args, options);
  }

  async adapterExecutionEvidenceQueryTool(args: AdapterExecutionEvidenceQueryArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<AdapterExecutionEvidenceQueryResult>> {
    return this.adapterExecutionEvidenceQuery(args, options);
  }

  async domainAcquisitionCatalogue(args: DomainAcquisitionArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<DomainAcquisitionResult>> {
    return this.callTool<DomainAcquisitionResult>("domain_acquisition_catalogue", args, options);
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

  async bundleVerify(args: BundleVerifyArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BundleVerifyResult>> {
    if (!isObject(args)) throw new ArgumentError("bundle verification arguments must be an object");
    const sources = [args.bundle !== undefined, args.document !== undefined, args.publicly_attested_bundle !== undefined].filter(Boolean).length;
    if (sources !== 1) throw new ArgumentError("bundle verification requires exactly one of bundle, document, or publicly_attested_bundle");
    if (args.bundle !== undefined && !isObject(args.bundle)) throw new ArgumentError("bundle must be an object");
    if (args.publicly_attested_bundle !== undefined && !isObject(args.publicly_attested_bundle)) throw new ArgumentError("publicly_attested_bundle must be an object");
    if (args.document !== undefined && (typeof args.document !== "string" || args.document.trim().length === 0)) throw new ArgumentError("document must be a non-empty path");
    if (args.publicly_attested_bundle !== undefined && args.verification_key === undefined) throw new ArgumentError("verification_key is required for publicly_attested_bundle");
    if (args.verification_key !== undefined) {
      if (!isObject(args.verification_key) || typeof args.verification_key.key_identity !== "string" || !/^ed25519:[0-9a-f]{64}$/.test(args.verification_key.public_key) || !isObject(args.verification_key.validity)) {
        throw new ArgumentError("verification_key must contain key_identity, an ed25519 public_key, and validity");
      }
      for (const field of ["not_before", "not_after"] as const) {
        const value = args.verification_key.validity[field];
        if (value !== undefined && (!Number.isSafeInteger(value) || value < 0)) throw new ArgumentError(`verification_key.validity.${field} must be a non-negative integer`);
      }
    }
    return this.callTool<BundleVerifyResult>("bundle_verify", args, options);
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
    routeReview?: JsonObject,
  ): MissionAssembly {
    return assembleMissionFromRoute(route, missionId, selections, policy, routeReview);
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
