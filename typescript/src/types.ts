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
  allowed_tools?: string[];
}

export interface AgentMissionArgs extends JsonObject {
  mission_id: string;
  goal: string;
  steps: AgentMissionStep[];
  policy?: AgentMissionPolicy;
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
  ok: boolean;
  fully_checked: boolean;
  ordered_steps: string[];
  waves: string[][];
  issues: string[];
  warnings: string[];
  steps: MissionStepPreflight[];
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
