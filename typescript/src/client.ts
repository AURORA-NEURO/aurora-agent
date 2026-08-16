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
  CapabilityRouteArgs,
  CapabilityRouteReviewArgs,
  CapabilityRouteReviewResult,
  CapabilityRouteResult,
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
  LineageAuditArgs,
  LineageAuditResult,
  PreanalyticApplyArgs,
  PreanalyticApplyResult,
  ContradictionReviewArgs,
  ContradictionReviewResult,
  LabPlanArgs,
  LabPlanResult,
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
  StressProfileArgs,
  StressProfileToolResult,
  StressReportArgs,
  StressReportToolResult,
  InfluenceAnalyzeArgs,
  InfluenceAnalyzeResult,
  RoutingDecideArgs,
  RoutingToolResult,
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
  DeliveryMutationResponse,
  DeliveriesResponse,
  DeveloperDeliveryAuditArgs,
  DeveloperDeliveryAuditResult,
  DeveloperPlatformStatusArgs,
  DeveloperPlatformStatusResult,
  EpistemicVoiArgs,
  EpistemicVoiResult,
  BenchmarkTraceAnalyzeArgs,
  BenchmarkTraceAnalysisResult,
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
  MissionJob,
  MissionJobStatus,
  MissionInventoryResponse,
  MissionPersistenceStatus,
  MissionPreflightResult,
  MissionRouteSelection,
  MissionTracePage,
  MissionWaitOptions,
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
  SubscriptionResponse,
  TelemetryProjectArgs,
  TelemetryProjectionResult,
  LedgerIngestArgs,
  LedgerIngestResult,
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

  /** Force a bounded event cursor checkpoint; subscriptions and deliveries remain non-durable. */
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

  async developerDeliveryAudit(args: DeveloperDeliveryAuditArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<DeveloperDeliveryAuditResult>> {
    return this.callTool<DeveloperDeliveryAuditResult>("developer_delivery_audit", args, options);
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

  async benchmarkTraceAnalyze(args: BenchmarkTraceAnalyzeArgs, options?: ClientRequestOptions): Promise<RestToolResponse<BenchmarkTraceAnalysisResult>> {
    return this.callTool<BenchmarkTraceAnalysisResult>("benchmark_trace_analyze", args, options);
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

  async capabilityDiscover(args: CapabilityDiscoverArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityDiscoverResult>> {
    return this.callTool<CapabilityDiscoverResult>("capability_discover", args, options);
  }

  async capabilityAudit(args: CapabilityAuditArgs = {}, options?: ClientRequestOptions): Promise<RestToolResponse<CapabilityAuditResult>> {
    return this.callTool<CapabilityAuditResult>("capability_audit", args, options);
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

  /** Read the current asynchronous mission status and, once terminal, its authoritative report. */
  async missionStatus(missionId: string, options?: ClientRequestOptions): Promise<MissionJob> {
    const id = pathSegment(missionId, "mission id");
    return this.request<MissionJob>("GET", `/v1/missions/${encodeURIComponent(id)}`, undefined, options);
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

  async deliveries(subscriptionId: string, after = 0, limit = 100, options?: ClientRequestOptions): Promise<DeliveriesResponse> {
    const id = pathSegment(subscriptionId, "subscription id");
    cursor(after, "after");
    pageLimit(limit);
    return this.request("GET", `/v1/webhooks/subscriptions/${encodeURIComponent(id)}/deliveries?after=${after}&limit=${limit}`, undefined, options);
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
