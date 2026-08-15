/** JSON values accepted by the dependency-free Prism client. */
export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[];
export interface JsonObject {
  [key: string]: JsonValue | undefined;
}

export type HttpMethod = "GET" | "POST" | "DELETE" | "OPTIONS";

export interface ApiErrorBody extends JsonObject {
  ok?: boolean;
  error?: {
    code?: string;
    message?: string;
    [key: string]: JsonValue | undefined;
  };
  request_id?: string;
}

export interface HealthResponse extends JsonObject {
  ok: boolean;
  ready: boolean;
  service: string;
  api_version: string;
  protocol_version: string;
  event_metrics?: EventMetrics;
  guarantees?: string[];
}

export interface CapabilitiesResponse extends JsonObject {
  api_version: string;
  mcp_protocol_version: string;
  tool_count: number;
  resource_count: number;
  workspace: JsonObject;
  transport: {
    rest_tools: boolean;
    json_rpc: boolean;
    event_cursor: boolean;
    server_sent_events_snapshot: boolean;
    signed_webhook_outbox: boolean;
    grpc: boolean;
    tls: boolean;
    external_delivery_worker: boolean;
    [key: string]: JsonValue;
  };
  limits: {
    max_header_bytes: number;
    max_body_bytes: number;
    event_capacity: number;
    webhook_filters: number;
    [key: string]: JsonValue;
  };
}

export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: JsonObject;
  [key: string]: JsonValue;
}

export interface ToolValidationIssue extends JsonObject {
  path: string;
  code: string;
  message: string;
}

export interface ToolValidationReport extends JsonObject {
  tool: string;
  schemaDigest: string;
  issues: ToolValidationIssue[];
  warnings: ToolValidationIssue[];
  ok: boolean;
  fullyChecked: boolean;
}

export interface ToolCallPlan {
  tool: string;
  definition: ToolDefinition;
  arguments: JsonObject;
  report: ToolValidationReport;
  schemaDigest: string;
}

export interface ToolsResponse extends JsonObject {
  api_version: string;
  tools: ToolDefinition[];
  call_shape: string;
}

export interface McpContentBlock extends JsonObject {
  type?: string;
  text?: string;
}

export interface McpResult<T extends JsonValue = JsonValue> extends JsonObject {
  content?: McpContentBlock[];
  structuredContent?: T;
  isError?: boolean;
  [key: string]: JsonValue | undefined;
}

export interface McpError extends JsonObject {
  code: number;
  message: string;
  data?: JsonValue;
}

export interface McpResponse<T extends JsonValue = JsonValue> extends JsonObject {
  jsonrpc?: string;
  id?: JsonValue;
  result?: McpResult<T>;
  error?: McpError;
}

export interface RestToolResponse<T extends JsonValue = JsonValue> extends JsonObject {
  ok: boolean;
  tool: string;
  request_id: string;
  mcp: McpResponse<T>;
  guarantee: string;
}

export interface ApiEvent extends JsonObject {
  id: number;
  event_type: string;
  subject: string;
  request_id: string;
  payload: JsonValue;
}

export interface EventPage extends JsonObject {
  events: ApiEvent[];
  after: number;
  next_after: number;
  oldest: number | null;
  newest: number | null;
  gap: boolean;
  dropped_events: number;
}

export interface EventsResponse extends JsonObject {
  ok: boolean;
  page: EventPage;
}

export interface RouteReviewEvidenceResponse extends JsonObject {
  ok: boolean;
  workflow: "capability_route_review_evidence";
  review_id: string;
  found: boolean;
  page: EventPage;
}

export interface SubscriptionView extends JsonObject {
  id: string;
  endpoint: string;
  events: string[];
  active: boolean;
  created_at_sequence: number;
}

export interface SubscriptionResponse extends JsonObject {
  ok: boolean;
  subscription: SubscriptionView;
  delivery?: {
    mode: string;
    poll: string;
    ack: string;
    retry: string;
    [key: string]: JsonValue;
  };
}

export interface SubscriptionListResponse extends JsonObject {
  ok: boolean;
  subscriptions: SubscriptionView[];
  secret_policy: string;
}

export interface WebhookEnvelope extends JsonObject {
  delivery_id: number;
  subscription_id: string;
  attempt: number;
  event: ApiEvent;
  signature: string;
}

export interface DeliveryView extends JsonObject {
  delivery_id: number;
  subscription_id: string;
  attempt: number;
  state: "pending" | "retryable" | "failed" | "exhausted";
  last_error: string | null;
  last_error_retryable: boolean | null;
  event_id: number;
  event_type: string;
  signature: string;
  envelope: WebhookEnvelope;
}

export interface DeliveryPage extends JsonObject {
  deliveries: DeliveryView[];
  after: number;
  next_after: number;
  pending_count: number;
  dropped_deliveries: number;
}

export interface DeliveriesResponse extends JsonObject {
  ok: boolean;
  page: DeliveryPage;
}

export interface DeliveryMutationResponse extends JsonObject {
  ok: boolean;
  acknowledged?: number[];
  retried?: DeliveryView[];
  replayed?: DeliveryView[];
}

export interface EventMetrics extends JsonObject {
  retained_events: number;
  dropped_events: number;
  subscriptions: number;
  active_subscriptions: number;
  pending_deliveries: number;
  dropped_deliveries: number;
  next_event_id: number;
  next_delivery_id: number;
}

export interface SubscribeOptions {
  subscriptionId?: string;
  events?: readonly string[];
}

export interface ClientRequestOptions {
  signal?: AbortSignal;
  headers?: Readonly<Record<string, string>>;
  requestId?: string;
}

export interface MissionWaitOptions extends ClientRequestOptions {
  /** Total wall-clock bound for status polling, in milliseconds. */
  timeoutMs?: number;
  /** Delay between non-terminal status reads, in milliseconds. */
  pollIntervalMs?: number;
}

export interface FetchLike {
  (input: string | URL, init?: RequestInit): Promise<Response>;
}

export interface ApiClientOptions {
  baseUrl: string | URL;
  bearerToken?: string;
  timeoutMs?: number;
  maxResponseBytes?: number;
  maxRequestBytes?: number;
  fetch?: FetchLike;
  defaultHeaders?: Readonly<Record<string, string>>;
}

export interface SseEvent {
  id?: string;
  event?: string;
  data: string;
  retry?: number;
}

export interface SseSnapshot {
  contentType: string;
  nextAfter: number | null;
  events: SseEvent[];
  raw: string;
}

export interface TraceOtelIngestArgs extends JsonObject {
  trace_id: string;
  otlp_json?: string;
  document?: string;
  succeeded?: boolean;
  include_events?: boolean;
  max_items?: number;
  max_spans?: number;
  max_bytes?: number;
}

export type RepositoryTraversalPolicy = "normative" | "exhaustive";

export interface RepositoryCatalogArgs extends JsonObject {
  prefix?: string;
  limit?: number;
  include_briefs?: boolean;
  include_findings?: boolean;
}

export interface RepositoryBundleArgs extends JsonObject {
  route: JsonObject;
  policy?: RepositoryTraversalPolicy;
  max_depth?: number;
  denied_labels?: string[];
  follow?: string[];
  include_markdown?: boolean;
  max_markdown_chars?: number;
}

export interface RepositoryImpactArgs extends JsonObject {
  changed: string;
  route?: JsonObject;
  routes?: JsonObject[];
}

export interface TelemetryProjectArgs extends JsonObject {
  event: JsonObject;
  policy: JsonObject;
  trace: string;
  metric?: JsonObject;
  observations?: JsonObject;
}

export interface MetricsProfileAuditArgs extends JsonObject {
  vectors: JsonValue[];
  waived_dimensions?: string[];
  weighting?: JsonObject;
  max_items?: number;
}

export interface MetricsAnalyticsAuditArgs extends JsonObject {
  observations: JsonValue[];
  pairs?: JsonValue[];
  calibration?: JsonValue[];
  calibration_bins?: number;
}

export interface BioCapabilityEvidenceAuditArgs extends JsonObject {
  metrics?: JsonObject;
  vectors?: JsonValue[];
  waived_dimensions?: string[];
  weighting?: JsonObject;
  evidence: JsonValue[];
  claim_requests: JsonValue[];
  information?: JsonObject;
  reference?: JsonObject;
  reference_state?: string;
  worldline?: JsonObject;
  at?: string;
  reexecution?: JsonObject;
  biological_claim?: string;
  max_items?: number;
}

export interface EvidenceAuditItemResult extends JsonObject {
  index: number;
  ok: boolean;
  id?: string;
  dimension?: string;
  domain?: string;
  declared_status?: string;
  effective_status?: string;
  issues: JsonObject[];
  support?: JsonObject;
  fail_closed: boolean;
  refusal?: string;
}

export interface EvidenceDimensionResult extends JsonObject {
  dimension: string;
  state: string;
  evidence_count: number;
  measured_count: number;
  declared_count: number;
  blocked_count: number;
  missing: boolean;
  measured: boolean;
}

export interface EvidenceInventoryResult extends JsonObject {
  items: EvidenceAuditItemResult[];
  omitted_items: number;
  item_count: number;
  invalid_item_count: number;
  dimensions: EvidenceDimensionResult[];
  domains: JsonObject;
}

export interface ClaimAuditRowResult extends JsonObject {
  index: number;
  ok: boolean;
  id?: string;
  claim?: string;
  requires: string[];
  allow_declared?: boolean;
  eligible?: boolean;
  blockers: JsonObject[];
  explicit_assumptions: JsonObject[];
  fail_closed: boolean;
  refusal?: string;
}

export interface ClaimInventoryResult extends JsonObject {
  rows: ClaimAuditRowResult[];
  omitted_rows: number;
  requested: number;
  eligible: number;
  all_requested_claims_eligible: boolean;
}

export interface EvidenceReleasePostureResult extends JsonObject {
  ready_for_requested_claims: boolean;
  requires_explicit_claim_request: boolean;
  numeric_scores_are_not_claims_without_evidence: boolean;
  declared_evidence_is_visible_but_not_measured_support: boolean;
}

export interface BioCapabilityEvidenceAuditResult extends JsonObject {
  ok: boolean;
  workflow: "biocapability_evidence_conditioned_profile";
  metrics: JsonObject;
  metrics_ok: boolean;
  evidence: EvidenceInventoryResult;
  claim_requests: ClaimInventoryResult;
  subaudits: {
    information_value: JsonObject | null;
    reference_quality: JsonObject | null;
    temporal_validity: JsonObject | null;
    reproducibility: JsonObject | null;
  };
  release_posture: EvidenceReleasePostureResult;
  guarantees: string[];
  limitations: string[];
}

export interface BioAtlasPublicationAuditArgs extends JsonObject {
  atlas: JsonObject;
  weighting?: JsonObject;
  evidence_audit?: BioCapabilityEvidenceAuditArgs;
  card?: JsonObject;
  leaderboard?: JsonObject;
  release_request?: JsonObject;
  max_items?: number;
}

export interface PublicationTargetResult extends JsonObject {
  target: string;
  eligible: boolean;
  blockers: string[];
  notes: string[];
}

export interface PublicationReleaseRequestResult extends JsonObject {
  present: boolean;
  id?: string;
  targets?: PublicationTargetResult[];
  ready: boolean;
  fail_closed?: boolean;
  no_implicit_release: boolean;
  reason?: string;
}

export interface PublicationCrossLayerResult extends JsonObject {
  numeric_score_requires_evidence_audit: boolean;
  numeric_score_evidence_ready: boolean;
  atlas_aggregation_ready: boolean;
  leaderboard_ranked_count: number;
  leaderboard_unranked_count: number;
  unranked_leaderboard_entries_remain_visible: boolean;
  withheld_scores_are_not_zeroes: boolean;
}

export interface BioAtlasPublicationAuditResult extends JsonObject {
  ok: boolean;
  workflow: "bioatlas_publication_audit";
  atlas: JsonObject;
  evidence_audit: JsonObject | null;
  card: JsonObject | null;
  leaderboard: JsonObject | null;
  release_request: PublicationReleaseRequestResult;
  cross_layer: PublicationCrossLayerResult;
  guarantees: string[];
  limitations: string[];
}

export interface DeveloperDeliveryAuditArgs extends JsonObject {
  platform?: JsonObject;
  repository?: JsonObject;
  repository_impact?: JsonObject;
  sdk?: JsonObject;
  conformance?: JsonObject;
  provider?: JsonObject;
  governance?: JsonObject;
  release?: JsonObject;
  release_request?: JsonObject;
}

export interface DeveloperDeliveryTargetResult extends JsonObject {
  target: string;
  available: boolean;
  eligible: boolean;
  blockers: string[];
  notes: string[];
}

export interface DeveloperDeliveryReadinessResult extends JsonObject {
  platform_checks_clean: boolean;
  unguarded_claims: number;
  developer_claims_ready: boolean;
  repository_scope_clean: boolean;
  repository_impact_clean: boolean;
  sdk_admission_clean: boolean;
  conformance_release: boolean;
  provider_capability_gate_cleared: boolean;
  governance_document_clean: boolean;
  release_audit_ready: boolean;
  local_delivery_ready: boolean;
}

export interface DeveloperDeliveryExternalSurfaceResult extends JsonObject {
  foreign_subject_count: number;
  foreign_artifacts_present: boolean;
  foreign_artifacts_are_not_inferred: boolean;
  local_integration_foundations: JsonObject[];
  unverified_surface_families: string[];
}

export interface DeveloperDeliveryReleaseRequestResult extends JsonObject {
  present: boolean;
  id?: string;
  targets?: DeveloperDeliveryTargetResult[];
  ready: boolean;
  fail_closed?: boolean;
  no_implicit_release: boolean;
  reason?: string;
  available_target_count: number;
}

export interface DeveloperDeliveryAuditResult extends JsonObject {
  ok: boolean;
  workflow: "developer_delivery_audit";
  platform: JsonObject | null;
  repository: JsonObject | null;
  repository_impact: JsonObject | null;
  sdk: JsonObject | null;
  conformance: JsonObject | null;
  provider: JsonObject | null;
  governance: JsonObject | null;
  release: JsonObject | null;
  readiness: DeveloperDeliveryReadinessResult;
  external_surface_posture: DeveloperDeliveryExternalSurfaceResult;
  release_request: DeveloperDeliveryReleaseRequestResult;
  guarantees: string[];
  limitations: string[];
}

export interface DeveloperWorkbenchArgs extends JsonObject {
  session: JsonObject;
  dashboard?: JsonObject;
  ci?: JsonObject;
}

export interface CapabilityDiscoverArgs extends JsonObject {
  query?: string;
  group_id?: string;
  domain?: string;
  tool?: string;
  max_items?: number;
  include_tools?: boolean;
}

export interface CapabilityAuditArgs extends JsonObject {
  include_groups?: boolean;
}

export interface CapabilityGroupResult extends JsonObject {
  id: string;
  domains: string[];
  crates: string[];
  mcp_tools: string[];
  cli_entrypoints: string[];
  python_artifacts: string[];
  status: string;
}

export interface CapabilityMatchResult extends JsonObject {
  group: CapabilityGroupResult;
  score: number;
  matched_fields: string[];
  matched_tools: string[];
  tool_schemas?: JsonObject[];
}

export interface CapabilityDiscoverResult extends JsonObject {
  ok: boolean;
  workflow: "capability_discover";
  capability_schema_version: string;
  schema_version: string;
  catalog_digest: string;
  total_groups: number;
  query: JsonObject;
  result_count: number;
  matches: CapabilityMatchResult[];
  schema_attachment: JsonObject;
}

export interface CapabilityAuditGroupResult extends JsonObject {
  id: string;
  domains: string[];
  status: string;
  declared_tool_memberships: number;
  unique_tools: number;
  schemas_found: number;
  missing_schemas: string[];
}

export interface CapabilitySchemaQualityResult extends JsonObject {
  checked: number;
  valid: number;
  total_bytes: number;
  maximum_schema_bytes: number;
  findings: JsonObject[];
}

export interface CapabilityAuditResult extends JsonObject {
  ok: boolean;
  workflow: "capability_audit";
  capability_schema_version: string;
  catalog_digest: string;
  healthy: boolean;
  total_groups: number;
  catalog_tool_memberships: number;
  unique_catalog_tools: number;
  advertised_tool_count: number;
  catalog_only_tools: string[];
  advertised_only_tools: string[];
  duplicate_schema_names: string[];
  duplicate_group_memberships: JsonObject[];
  schema_quality: CapabilitySchemaQualityResult;
  invariants: JsonObject;
  groups?: CapabilityAuditGroupResult[];
}

export interface CapabilityRouteNeed extends JsonObject {
  id: string;
  query?: string;
  group_id?: string;
  domain?: string;
  tool?: string;
  max_items?: number;
}

export interface CapabilityRouteArgs extends JsonObject {
  goal: string;
  needs: CapabilityRouteNeed[];
  max_candidates_per_need?: number;
  max_tools?: number;
  include_tools?: boolean;
}

export interface CapabilityRouteNeedResult extends JsonObject {
  id: string;
  resolution: "explicit" | "ranked_candidates" | "unresolved";
  candidate_groups: string[];
  candidate_domains: string[];
  candidate_tools: string[];
  search: JsonObject;
}

export interface CapabilityRouteCoverage extends JsonObject {
  needs_total: number;
  needs_resolved: number;
  needs_unresolved: number;
  candidate_group_count: number;
  candidate_groups: string[];
  candidate_domain_count: number;
  candidate_domains: string[];
  candidate_tool_count: number;
  posture: string;
}

export interface CapabilityRouteResult extends JsonObject {
  ok: boolean;
  workflow: "capability_route";
  route_id: string;
  catalog_digest: string;
  goal: string;
  needs: CapabilityRouteNeedResult[];
  unresolved_needs: string[];
  recommended_tools: string[];
  recommended_tool_count: number;
  recommended_tool_overflow: number;
  route_coverage: CapabilityRouteCoverage;
  schema_attachment: JsonObject;
  execution: "not_started";
}

export interface CapabilityRouteReviewArgs extends JsonObject {
  route: JsonObject;
  selections: MissionRouteSelection[];
  validate_schemas?: boolean;
}

export interface CapabilityRouteReviewFinding extends JsonObject {
  code: string;
  severity: "error";
  message: string;
  need_id?: string;
}

export interface CapabilityRouteReviewResult extends JsonObject {
  ok: boolean;
  workflow: "capability_route_review";
  review_id: string;
  route_id: string;
  catalog_digest: string;
  goal: string;
  need_count: number;
  selection_count: number;
  missing_needs: string[];
  selected_tools: string[];
  selected_domains: string[];
  dependency_waves: string[][];
  findings: CapabilityRouteReviewFinding[];
  review_status: "ready" | "blocked";
  handoff_status: "mission_preflight_required" | "requires_caller_correction";
  mission_draft: JsonObject | null;
  route_coverage: JsonObject;
  schema_review: JsonObject;
  execution: "not_started";
}

export interface AdapterPlanArgs extends JsonObject {
  source_id: string;
  declared_format?: string;
  source_kind: "bytes" | "directory";
  required_conformance?: "parse" | "normalize" | "execute" | "stream" | "replay";
  available_dependencies?: string[];
}

/** One authoritative adapter route, including its declared semantic-loss boundary. */
export interface AdapterDescriptorResult extends JsonObject {
  id: string;
  version: string;
  execution: "native" | "python_delegated";
  accepted_formats: string[];
  accepts_undeclared_format: boolean;
  source_kinds: ("bytes" | "directory")[];
  conformance_level: "parse" | "normalize" | "execute" | "stream" | "replay";
  declared_loss_kinds: string[];
  scope_dimensions: string[];
  optional_dependency: string | null;
  description: string;
}

/** A candidate route and the explicit reason it is ready or refused. */
export interface AdapterPlanCandidateResult extends JsonObject {
  adapter: AdapterDescriptorResult;
  status: "ready" | "unsupported_format" | "unsupported_source_kind" | "unsupported_conformance" | "dependency_unknown" | "dependency_missing";
  reasons: string[];
}

/** Full serialized adapter plan, preserving request, candidates, and limitations. */
export interface AdapterPlanProjectionResult extends JsonObject {
  schema: string;
  request: JsonObject;
  selected_adapter: AdapterDescriptorResult | null;
  executable: boolean;
  candidates: AdapterPlanCandidateResult[];
  limitations: string[];
}

/** Typed `adapter_plan` envelope returned by MCP/REST. */
export interface AdapterPlanResult extends JsonObject {
  ok: boolean;
  workflow: "adapter_plan";
  plan_id: string;
  registry: string;
  executable: boolean;
  selected_adapter: JsonObject | null;
  plan: AdapterPlanProjectionResult;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface TabularIngestArgs extends JsonObject {
  source_id: string;
  profile: JsonObject;
  csv?: string;
  document?: string;
  format?: string;
  provenance?: JsonObject;
  include_facts?: boolean;
  max_items?: number;
  max_bytes?: number;
}

export interface TabularCheckResult extends JsonObject {
  check: string;
  status: "pass" | "fail" | "not_applicable";
  detail: string;
}

export interface TabularConformanceDetails extends JsonObject {
  adapter: string;
  adapter_version: string;
  source_id: string;
  checks: TabularCheckResult[];
}

export interface TabularConformanceResult extends JsonObject {
  report: TabularConformanceDetails;
  passed: boolean;
  verified: boolean;
  summary: string;
}

export interface TabularSemanticLossResult extends JsonObject {
  audit: "unaudited" | "lossless" | "lossy";
  mapped?: JsonValue[];
  lost?: JsonObject[];
  reason?: string;
}

export interface TabularManifestResult extends JsonObject {
  source_id: string;
  declared_format?: string;
  source_digest: string;
  byte_length?: number;
  adapter: string;
  adapter_version: string;
  profile_digest?: string;
  provenance?: JsonObject;
}

export interface TabularIngestResult extends JsonObject {
  ok: boolean;
  source_id: string;
  fact_count: number;
  ingestion_sha256: string;
  manifest: TabularManifestResult;
  semantic_loss: TabularSemanticLossResult;
  conformance: TabularConformanceResult;
  max_items: number;
  facts?: JsonObject[];
  omitted_facts?: number;
  limitations: string[];
}

export interface ConformanceRunArgs extends JsonObject {
  include_details?: boolean;
  max_items?: number;
}

export interface ConformanceOutcomeResult extends JsonObject {
  outcome: "passed" | "failed" | "unsupported" | "errored";
  expectation?: string;
  detail?: string;
  reason?: string;
}

export interface ConformanceCaseResult extends JsonObject {
  case_id: string;
  title: string;
  layer: "unit" | "property" | "golden" | "conformance" | "end_to_end";
  requirement: "must" | "should";
  enforces: string[];
  invariant: string;
  expectations: string[];
  outcome: ConformanceOutcomeResult;
}

export interface ConformancePyramidResult extends JsonObject {
  counts: JsonObject;
}

export interface ConformanceSuiteResult extends JsonObject {
  id: string;
  version: string;
  digest: string;
  fixture_manifest_id: string;
  fixture_count: number;
  synthetic_fixture_count: number;
  case_count: number;
  passed: number;
  failed: number;
  unsupported: number;
  errored: number;
  fixture_drift: JsonObject[];
  pyramid: ConformancePyramidResult;
  fully_conformant: boolean;
}

export interface ConformanceUnmetGateResult extends JsonObject {
  gate: string;
  because: string;
  evidence: string[];
}

export interface ConformanceReleaseDecisionResult extends JsonObject {
  decision: "release" | "blocked";
  suite_id: string;
  suite_version: string;
  suite_digest?: string;
  implementation?: string;
  gates?: string[];
  met?: string[];
  unmet?: ConformanceUnmetGateResult[];
}

export interface ConformanceRunResult extends JsonObject {
  ok: boolean;
  suite: ConformanceSuiteResult;
  release_decision: ConformanceReleaseDecisionResult;
  summary: string;
  results: ConformanceCaseResult[] | null;
  guarantees: string[];
}

export type ReleaseAuditCheckKind =
  | "registry_gate"
  | "bundle_verify"
  | "conformance_run"
  | "research_ci_check"
  | "quality_gate_run"
  | "ops_acceptance"
  | "pack_health_assess"
  | "repository_impact"
  | "developer_platform_status";

export interface ReleaseAuditCheckArgs extends JsonObject {
  kind: ReleaseAuditCheckKind;
  arguments?: JsonObject;
  required?: boolean;
}

export interface ReleaseAuditArgs extends JsonObject {
  checks: ReleaseAuditCheckArgs[];
  include_details?: boolean;
}

export interface ReleaseAuditCheckResult extends JsonObject {
  index: number;
  kind: ReleaseAuditCheckKind;
  required: boolean;
  advisory: boolean;
  evaluated: boolean;
  gate: boolean | null;
  passed: boolean;
  result_digest?: string;
  result_ok?: boolean;
  refusal?: string;
  fail_closed?: boolean;
  result?: JsonObject;
}

export interface ReleaseAuditBlockerResult extends JsonObject {
  index: number;
  kind: ReleaseAuditCheckKind;
  reason: string;
  fail_closed: boolean;
}

export interface ReleaseAuditResult extends JsonObject {
  ok: boolean;
  release_ready: boolean;
  required_check_count: number;
  check_count: number;
  invocation_failures: number;
  blocking_count: number;
  blockers: ReleaseAuditBlockerResult[];
  checks: ReleaseAuditCheckResult[];
  guarantees: string[];
  limitations: string[];
}

export interface OperationsCatalogArgs extends JsonObject {
  include_details?: boolean;
  max_items?: number;
}

export interface OperationsStoreResult extends JsonObject {
  name: string;
  technology: string;
  durability: "Canonical" | "Rebuildable";
  mutability: "immutable" | "append_only" | "mutable";
  rebuilt_from: string[];
}

export interface OperationsTopologyClassResult extends JsonObject {
  class: "metadata" | "artifact" | "event" | "analytics" | "search";
  name: string;
  store: OperationsStoreResult;
  promises: JsonObject;
  holds_immutable_evidence: boolean;
}

export interface OperationsTopologyResult extends JsonObject {
  deployment: string;
  technologies: string[];
  classes: OperationsTopologyClassResult[];
}

export interface OperationsPromiseParityResult extends JsonObject {
  compared: number;
  holds: boolean;
  differences: string[];
}

export interface OperationsDataClassResult extends JsonObject {
  class: "metadata" | "artifact" | "event" | "analytics" | "search";
  name: string;
  holds_immutable_evidence: boolean;
}

export interface OperationsDeploymentPlaneResult extends JsonObject {
  plane: string;
  name: string;
  control_plane: boolean;
}

export interface OperationsTenantPatternResult extends JsonObject {
  pattern: string;
  name: string;
}

export interface OperationsServiceSummaryResult extends JsonObject {
  satisfied: number;
  diverges: number;
  not_implemented: number;
  divergences: number;
  total: number;
}

export interface OperationsServiceContractResult extends JsonObject {
  module_id: string;
  title: string;
  contract: string;
  crates: string[];
  verdict: "satisfied" | "diverges" | "not_implemented";
  divergence_count: number;
  divergences: string[];
  omitted_divergences: number;
}

export interface OperationsServiceContractsResult extends JsonObject {
  summary: OperationsServiceSummaryResult;
  entries: OperationsServiceContractResult[];
  entry_count: number;
  omitted_entries: number;
}

export interface OperationsMetricDefinitionResult extends JsonObject {
  metric: string;
  blueprint_name: boolean;
  numerator: string;
  denominator: string;
  refuses: string;
}

export interface OperationsUndefinedMetricResult extends JsonObject {
  origin: string;
  module_title: string;
  metric: string;
  denominator?: string | null;
}

export interface OperationsMetricsResult extends JsonObject {
  metrics_schema_version: string;
  atlasx_schema_version: string;
  named_in_scope: number;
  named_but_undefined: number;
  defined_here: OperationsMetricDefinitionResult[];
  undefined_metrics_returned: OperationsUndefinedMetricResult[];
  omitted_undefined_metrics: number;
  undefined_is_not_zero: boolean;
}

export interface OperationsSdkResult extends JsonObject {
  registration_note: string;
  execution_and_isolation_are_not_implied: boolean;
}

export interface OperationsCatalogResult extends JsonObject {
  ok: boolean;
  detail_mode: "summary" | "full";
  max_items: number;
  topologies: {
    local: OperationsTopologyResult;
    team: OperationsTopologyResult;
    promise_parity: OperationsPromiseParityResult;
    technology_is_not_promise_parity: boolean;
  };
  data_classes: OperationsDataClassResult[];
  deployment_planes: OperationsDeploymentPlaneResult[];
  tenant_patterns: OperationsTenantPatternResult[];
  slo_objectives: string[];
  service_contracts: OperationsServiceContractsResult;
  metrics: OperationsMetricsResult;
  sdk: OperationsSdkResult;
  details?: JsonObject;
  limitations: string[];
}

export type SafetyRiskDimension =
  | "capability_uplift"
  | "actionability"
  | "scale"
  | "expertise_reduction"
  | "target_specificity"
  | "reversibility"
  | "detectability"
  | "available_safeguards"
  | "legitimate_scientific_value";

export type SafetyRating = "low" | "moderate" | "high";

export type SafetyCategory =
  | "cyber_exploitation"
  | "biological_design"
  | "surveillance_and_privacy_invasion"
  | "fraud"
  | "harmful_physical_automation"
  | "clinical_misuse";

export interface RiskAssessmentArgs extends JsonObject {
  subject: string;
  category?: SafetyCategory;
  ratings: Partial<Record<SafetyRiskDimension, SafetyRating>>;
}

export interface SafetyReleaseGateArgs extends JsonObject {
  assessment: RiskAssessmentArgs;
}

export type SafetyGateDecision = "cleared" | "conditioned" | "blocked";

export interface SafetyGateDecisionResult extends JsonObject {
  decision: SafetyGateDecision;
  subject: string;
  conditions?: string[];
  driven_by?: SafetyRiskDimension[];
}

export interface SafetyReleaseGateResult extends JsonObject {
  ok: boolean;
  subject: string;
  category?: SafetyCategory | null;
  decision: SafetyGateDecisionResult;
  cleared: boolean;
  unrated_dimensions: SafetyRiskDimension[];
  high_risk_dimensions: SafetyRiskDimension[];
  rule: string;
  fail_closed: boolean;
  limitations: string[];
}

export type MedicalResearchUse =
  | "workflow_reproducibility"
  | "data_quality_checks"
  | "paper_data_code_linkage"
  | "imaging_and_omics_metadata_reasoning"
  | "tool_use"
  | "provenance"
  | "evidence_synthesis"
  | "uncertainty_reporting"
  | "benchmark_methodology";

export type ProhibitedClinicalOutput =
  | "personalised_clinical_recommendation"
  | "urgency_classification"
  | "treatment_selection"
  | "prognosis_as_patient_advice"
  | "clinician_review_bypass";

export interface MedicalBoundaryOutputArgs extends JsonObject {
  side: "research" | "clinical";
  label: string;
  use_case?: MedicalResearchUse;
  category?: ProhibitedClinicalOutput;
}

export interface MedicalBoundaryArgs extends JsonObject {
  output: MedicalBoundaryOutputArgs;
}

export interface MedicalBoundaryResult extends JsonObject {
  ok: boolean;
  admitted: boolean;
  use_case?: MedicalResearchUse;
  refusal?: string;
  research_only_label: string;
  boundary_is_unconditional: boolean;
  clinical_output_is_never_admitted?: boolean;
  limitations?: string[];
}

export interface SafetyPostureArgs extends JsonObject {
  include_threats?: boolean;
}

export interface SafetyCoverageResult extends JsonObject {
  mitigated: number;
  declared_only: number;
  unmitigated: number;
}

export interface SafetyThreatMitigationResult extends JsonObject {
  state: "enforced" | "declared_only" | "absent";
  name: string;
  role: string;
  declared_in?: string;
  reason?: JsonObject;
  by?: JsonObject;
}

export interface SafetyThreatResult extends JsonObject {
  id: string;
  module: string;
  asset: string;
  class: string;
  requires: string[];
  surface: string;
  narrative: string;
  mitigations: SafetyThreatMitigationResult[];
}

export interface SafetyPostureResult extends JsonObject {
  ok: boolean;
  model: string;
  adversaries: number;
  threats: number;
  coverage: SafetyCoverageResult;
  coverage_summary: string;
  residual_threat_ids: string[];
  unanalysed_threat_ids: string[];
  unreachable_threat_ids: string[];
  audit_acceptances: boolean;
  perimeter_controls_are_not_claimed_as_enforced: boolean;
  threat_details?: SafetyThreatResult[];
}

export interface MeasurementCompareArgs extends JsonObject {
  left: JsonObject;
  right: JsonObject;
  require_bound_terms?: boolean;
}

export type MeasurementBlockingReason =
  | "kind_mismatch"
  | "dimension_mismatch"
  | "not_commensurable"
  | "conversion_required"
  | "unstated_frame"
  | "frame_mismatch"
  | "orientation_mismatch"
  | "space_mismatch"
  | "unstated_build"
  | "build_mismatch"
  | "convention_mismatch"
  | "contig_mismatch"
  | "unbound_term"
  | "unmapped_term"
  | "ambiguous_term"
  | "namespace_mismatch"
  | "ontology_version_drift"
  | "granularity_mismatch"
  | "term_mismatch";

export interface MeasurementConversionResult extends JsonObject {
  from: string;
  to: string;
  factor: number;
  exactness: { exactness: "exact" } | { exactness: "conventional"; convention: string };
}

export interface MeasurementBlockedReasonResult extends JsonObject {
  blocked_by: MeasurementBlockingReason;
  [key: string]: JsonValue | undefined;
}

export interface MeasurementVerdictResult extends JsonObject {
  verdict: "comparable" | "blocked";
  reason?: MeasurementBlockedReasonResult;
}

export interface MeasurementComparabilityReport extends JsonObject {
  left: string;
  right: string;
  verdict: MeasurementVerdictResult;
  conversions: MeasurementConversionResult[];
  caveats: string[];
}

export interface MeasurementCompareResult extends JsonObject {
  ok: boolean;
  comparable: boolean;
  policy: { require_bound_terms: boolean };
  report: MeasurementComparabilityReport;
  report_sha256: string;
  guarantees: string[];
  limitations: string[];
}

export interface OpsAcceptanceArgs extends JsonObject {
  max_items?: number;
}

export type OpsAcceptanceVerdict = "met" | "refuted" | "unverifiable";

export interface OpsAcceptanceBasisResult extends JsonObject {
  basis: "linked_type" | "workspace_manifest" | "author" | "no_observer";
  krate?: string;
  item?: string;
  who?: string;
  because?: string;
}

export interface OpsAcceptanceFindingResult extends JsonObject {
  criterion: string;
  verdict: OpsAcceptanceVerdict;
  basis: OpsAcceptanceBasisResult;
  detail: string;
}

export interface OpsAcceptanceSummaryResult extends JsonObject {
  met: number;
  refuted: number;
  unverifiable: number;
  total: number;
  is_release_ready: boolean;
  is_decidable: boolean;
}

export interface OpsAcceptanceResult extends JsonObject {
  ok: boolean;
  summary: OpsAcceptanceSummaryResult;
  findings: OpsAcceptanceFindingResult[];
  omitted_findings: number;
  guarantees: string[];
  limitations: string[];
}

export interface AgentMissionBinding extends JsonObject {
  from_step: string;
  source_pointer: string;
  target_pointer: string;
}

export interface AgentMissionStep extends JsonObject {
  id: string;
  domain: string;
  capability: string;
  objective: string;
  tool: string;
  arguments?: JsonObject;
  depends_on?: string[];
  required?: boolean;
  bindings?: AgentMissionBinding[];
}

export interface AgentMissionPolicy extends JsonObject {
  execute?: boolean;
  stop_on_error?: boolean;
  allow_side_effects?: boolean;
  max_steps?: number;
  max_step_output_bytes?: number;
  max_total_output_bytes?: number;
  execution_mode?: "serial" | "parallel_waves";
  max_parallelism?: number;
  allowed_tools?: string[];
}

export interface AgentMissionArgs extends JsonObject {
  mission_id: string;
  goal: string;
  steps: AgentMissionStep[];
  policy?: AgentMissionPolicy;
}

export type MissionTraceEventName =
  | "mission.started"
  | "wave.started"
  | "step.started"
  | "step.completed"
  | "step.refused"
  | "step.blocked"
  | "step.cancelled"
  | "wave.completed"
  | "mission.cancelled"
  | "mission.completed";

export interface MissionTraceEvent extends JsonObject {
  sequence: number;
  event: MissionTraceEventName;
  wave: number | null;
  step_id: string | null;
  tool: string | null;
  status: string | null;
  arguments_digest: string | null;
  bytes: number;
  detail: string | null;
}

export interface AgentMissionReport extends JsonObject {
  ok: boolean;
  workflow: "agent_mission";
  execution: "planned" | "executed";
  mission_status: "planned" | "running" | "succeeded" | "partial" | "failed" | "cancelled";
  succeeded?: number;
  refused?: number;
  blocked?: number;
  cancelled?: number;
  required_failures?: number;
  returned_bytes: number;
  execution_trace_schema_version: string;
  execution_trace: MissionTraceEvent[];
  preflight?: boolean;
  dispatch?: "not_started";
  plan: JsonObject;
  results: JsonObject[];
  [key: string]: JsonValue | undefined;
}

export type MissionJobStatus = "queued" | "running" | "planned" | "succeeded" | "partial" | "failed" | "cancelled";

export type MissionProgressPhase = MissionJobStatus | "cancellation_requested";

export interface MissionProgress extends JsonObject {
  phase: MissionProgressPhase;
  current_wave: number | null;
  total_steps: number;
  completed_steps: number;
  active_steps: number;
  succeeded: number;
  refused: number;
  blocked: number;
  cancelled: number;
  required_failures: number;
  returned_bytes: number;
  trace_sequence: number | null;
  last_event: string | null;
}

export interface MissionResultOmission extends JsonObject {
  bytes: number;
  sha256: string;
}

export interface MissionJob extends JsonObject {
  ok: boolean;
  mission_id: string;
  status: MissionJobStatus;
  cancel_requested: boolean;
  cancel_reason?: string | null;
  recovered_after_restart?: boolean;
  result?: AgentMissionReport | null;
  result_omitted?: MissionResultOmission | null;
  error?: string | null;
  progress?: MissionProgress;
  poll?: string;
  cancel?: string;
  trace?: string;
}

export interface MissionTracePage extends JsonObject {
  ok: boolean;
  mission_id: string;
  trace_schema_version: string;
  events: MissionTraceEvent[];
  after: number;
  next_after: number;
  oldest: number | null;
  newest: number | null;
  gap: boolean;
  dropped_events: number;
  terminal: boolean;
  limit: number;
  truncated: boolean;
}

export interface MissionInventorySummary extends JsonObject {
  total_steps: number;
  completed_steps: number;
  succeeded: number;
  refused: number;
  blocked: number;
  cancelled: number;
  required_failures: number;
  returned_bytes: number;
  result_available: boolean;
  result_omitted?: MissionResultOmission | null;
  recovered_after_restart?: boolean;
}

export interface MissionInventoryItem extends JsonObject {
  mission_id: string;
  status: MissionJobStatus;
  cancel_requested: boolean;
  cancel_reason?: string | null;
  recovered_after_restart?: boolean;
  progress: MissionProgress;
  summary: MissionInventorySummary;
  poll: string;
  cancel: string;
  trace: string;
}

export interface MissionInventoryResponse extends JsonObject {
  ok: boolean;
  missions: MissionInventoryItem[];
  returned: number;
  total_matching: number;
  limit: number;
  truncated: boolean;
  status_filter: MissionJobStatus | null;
}

export interface MissionPersistenceStatus extends JsonObject {
  ok: boolean;
  enabled: boolean;
  file_present: boolean;
  file_bytes: number | null;
  schema_version: number;
  max_file_bytes: number;
  max_result_bytes: number;
  registry_size: number;
  event_log_durable: false;
  webhook_deliveries_durable: false;
  recovery_policy: string;
  flush: string;
}

export interface EventPersistenceStatus extends JsonObject {
  ok: boolean;
  enabled: boolean;
  file_present: boolean;
  file_bytes: number | null;
  schema_version: number;
  max_file_bytes: number;
  retained_events: number;
  next_event_id: number;
  dropped_events: number;
  subscriptions_durable: false;
  webhook_deliveries_durable: false;
  recovery_policy: string;
  flush: string;
}

export interface MissionStepPreflight extends JsonObject {
  id: string;
  tool: string;
  depends_on: string[];
  wave: number | null;
  status: "ready" | "invalid" | "blocked";
  schema: ToolValidationReport | null;
  issues: string[];
  warnings: string[];
}

export interface MissionPreflightResult extends JsonObject {
  schema: "bioprism-typescript-mission-preflight/0.1";
  mission_id: string;
  goal: string;
  request_digest: string;
  catalogue_digest: string;
  execution: "planned" | "authorized";
  execution_mode: "serial" | "parallel_waves";
  max_parallelism: number;
  ok: boolean;
  fully_checked: boolean;
  ordered_steps: string[];
  waves: string[][];
  issues: string[];
  warnings: string[];
  steps: MissionStepPreflight[];
  limitations: string[];
}

export interface MissionRouteSelection extends JsonObject {
  need_id: string;
  tool: string;
  domain: string;
  capability: string;
  objective: string;
  arguments: JsonObject;
  depends_on?: string[];
  required?: boolean;
  bindings?: AgentMissionBinding[];
}

export interface MissionAssembly extends JsonObject {
  schema: "bioprism-typescript-mission-assembly/0.1";
  route_id: string;
  catalog_digest: string;
  mission: AgentMissionArgs;
  selected_tools: string[];
  limitations: string[];
}

export interface RuntimeExecutionSimulateArgs extends JsonObject {
  tape?: JsonObject;
  actions?: JsonValue[];
  budget?: JsonObject;
  faults?: JsonValue[];
  forks?: JsonValue[];
  max_items?: number;
}

export type ToolArguments = JsonObject;
