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

export interface BioAtlasPublicationAuditArgs extends JsonObject {
  atlas: JsonObject;
  weighting?: JsonObject;
  evidence_audit?: BioCapabilityEvidenceAuditArgs;
  card?: JsonObject;
  leaderboard?: JsonObject;
  release_request?: JsonObject;
  max_items?: number;
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
