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

export interface DeliveryReceiptEventsResponse extends JsonObject {
  ok: boolean;
  workflow: "developer_delivery_receipt_events";
  receipt_id: string;
  found: boolean;
  page: EventPage;
}

export interface SubscriptionView extends JsonObject {
  id: string;
  endpoint: string;
  events: string[];
  active: boolean;
  created_at_sequence: number;
  secret_bound: boolean;
  rebind_required: boolean;
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

export interface SubscriptionRebindResponse extends JsonObject {
  ok: boolean;
  subscription: SubscriptionView;
  resigned_deliveries: number;
  secret_policy: string;
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
  state: "pending" | "retryable" | "failed" | "exhausted" | "secret_rebind_required";
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
  retained_delivery_attempts: number;
  dropped_delivery_attempts: number;
  next_attempt_id: number;
}

export interface DeliveryAttempt extends JsonObject {
  attempt_id: number;
  delivery_id: number;
  subscription_id: string;
  event_id: number;
  event_type: string;
  attempt: number;
  action: string;
  outcome: string;
  receiver_accepted: boolean | null;
  retryable: boolean | null;
  error: string | null;
  signature: string;
  receipt_id: string | null;
  receipt_digest: string | null;
}

export interface DeliveryAttemptPage extends JsonObject {
  attempts: DeliveryAttempt[];
  after: number;
  next_after: number;
  oldest: number | null;
  newest: number | null;
  gap: boolean;
  dropped_attempts: number;
}

export interface DeliveryAttemptsResponse extends JsonObject {
  ok: boolean;
  page: DeliveryAttemptPage;
}

export interface DeliveryReceiptAttemptsResponse extends JsonObject {
  ok: boolean;
  workflow: "developer_delivery_receipt_attempts";
  receipt_id: string;
  found: boolean;
  page: DeliveryAttemptPage;
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

export interface TraceOtelFieldLossResult extends JsonObject {
  path: string;
  detail: string;
}

export interface TraceOtelDroppedSpanResult extends JsonObject {
  path: string;
  name?: string | null;
  detail: string;
}

export interface TraceOtelLossResult extends JsonObject {
  dropped_spans: TraceOtelDroppedSpanResult[];
  dropped_span_events: TraceOtelFieldLossResult[];
  unmapped_fields: TraceOtelFieldLossResult[];
  duplicate_attributes: TraceOtelFieldLossResult[];
  inferred_kinds: TraceOtelFieldLossResult[];
  missing_start_times: TraceOtelFieldLossResult[];
  unresolved_parents: TraceOtelFieldLossResult[];
  multiple_trace_ids: TraceOtelFieldLossResult[];
}

export interface TraceOtelMappingResult extends JsonObject {
  format: string;
  resource_count: number;
  scope_count: number;
  source_span_count: number;
  accepted_span_count: number;
  span_event_count: number;
}

export type TraceOtelEventKind = "goal" | "observation" | "choice" | "action" | "result" | "claim" | "termination";

export interface TraceOtelEventResult extends JsonObject {
  step: number;
  kind: TraceOtelEventKind;
  payload: JsonObject;
  caused_by?: number;
  visible?: string[];
}

export interface TraceOtelIngestResult extends JsonObject {
  ok: true;
  schema: "bioprism-mcp/trace-otel-ingest/0.1";
  trace_id: string;
  event_count: number;
  succeeded: boolean;
  trace_sha256: string;
  valid: boolean;
  validation_error?: string | null;
  mapping: TraceOtelMappingResult;
  loss: TraceOtelLossResult;
  lossless: boolean;
  dropped_events: number;
  compilable: boolean;
  events_included: boolean;
  events: TraceOtelEventResult[] | null;
  omitted_events: number;
  guarantees: string[];
  limitations: string[];
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

export interface TelemetryLossResult extends JsonObject {
  dropped: string[];
  coarsened: string[];
}

export interface TelemetryRecordResult extends JsonObject {
  event_id: string;
  kind: string;
  trace: string;
  attributes: JsonObject;
  epoch: number;
  policy: string;
}

export interface TelemetryMetricValueResult extends JsonObject {
  metric: string;
  unit: string;
  value: number;
  supported_by: string[];
}

export interface TelemetryMetricSuccessResult extends JsonObject {
  ok: true;
  value: TelemetryMetricValueResult;
  audit_statement: string;
}

export interface TelemetryMetricRefusalResult extends JsonObject {
  ok: false;
  refusal: string;
  asserted_signals?: string[];
  observed_sample_count?: number;
}

export type TelemetryMetricResult = TelemetryMetricSuccessResult | TelemetryMetricRefusalResult;

export interface TelemetryProjectionResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/telemetry-projection/0.1";
  stage?: "telemetry_projection";
  event_id?: string;
  event_kind?: string;
  trace?: string;
  policy_version?: string;
  record: TelemetryRecordResult | null;
  loss: TelemetryLossResult | null;
  lossless?: boolean;
  metric?: TelemetryMetricResult | null;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export interface LedgerTemporalCutArgs extends JsonObject {
  as_of_valid?: string;
  as_of_record?: string;
  as_of_release?: string;
}

export interface LedgerIngestArgs extends JsonObject {
  events: JsonObject[];
  cut?: LedgerTemporalCutArgs;
  include_receipts?: boolean;
  max_items?: number;
}

export interface LedgerRecordedAdmissionResult extends JsonObject {
  admission: "recorded";
  id: string;
  seq: number;
}

export interface LedgerDuplicateAdmissionResult extends JsonObject {
  admission: "duplicate";
  id: string;
}

export interface LedgerQuarantinedAdmissionResult extends JsonObject {
  admission: "quarantined";
  key: string;
  missing: string[];
}

export type LedgerAdmissionResult = LedgerRecordedAdmissionResult | LedgerDuplicateAdmissionResult | LedgerQuarantinedAdmissionResult;

export interface LedgerAppendReceiptResult extends JsonObject {
  event_index: number;
  receipt: {
    admission: LedgerAdmissionResult;
    released: string[];
  };
}

export interface LedgerAdmissionsResult extends JsonObject {
  recorded: number;
  duplicates: number;
  quarantined: number;
  released: number;
  receipts: LedgerAppendReceiptResult[] | null;
}

export interface LedgerChainResult extends JsonObject {
  status: "intact" | "broken";
  at_seq?: number;
  reason?: string;
}

export interface LedgerClockAnomalyResult extends JsonObject {
  seq: number;
  previous_record: string;
  record: string;
}

export interface LedgerQuarantineItemResult extends JsonObject {
  key: string;
  missing: string[];
  note?: string | null;
}

export interface LedgerQuarantineResult extends JsonObject {
  count: number;
  items: LedgerQuarantineItemResult[];
  omitted: number;
}

export interface LedgerLatestFactResult extends JsonObject {
  subject: string;
  event: string;
  seq: number;
  valid: string;
  payload_digest: string;
}

export interface LedgerLatestBySubjectResult extends JsonObject {
  count: number;
  items: LedgerLatestFactResult[];
  omitted: number;
}

export interface LedgerCutEntryResult extends JsonObject {
  seq: number;
  id: string;
  class: string;
  kind: string;
  subject: string;
  valid: string;
  record: string;
  release: string;
}

export interface LedgerCutSuccessResult extends JsonObject {
  requested: LedgerTemporalCutArgs;
  ok?: true;
  count: number;
  entries: LedgerCutEntryResult[];
  omitted: number;
}

export interface LedgerCutRefusalResult extends JsonObject {
  requested: LedgerTemporalCutArgs;
  ok: false;
  refusal: string;
  fail_closed: true;
}

export type LedgerCutResult = LedgerCutSuccessResult | LedgerCutRefusalResult;

export interface LedgerBeforeRefusalResult extends JsonObject {
  recorded_entries: number;
  quarantined: number;
  next_seq: number;
  chain: LedgerChainResult;
}

export interface LedgerIngestSuccessResult extends JsonObject {
  ok: true;
  schema: "bioprism-mcp/ledger-ingest/0.1";
  entries: number;
  next_seq: number;
  head: string;
  admissions: LedgerAdmissionsResult;
  chain: LedgerChainResult;
  clock_anomalies: LedgerClockAnomalyResult[];
  quarantine: LedgerQuarantineResult;
  class_counts: Record<string, number>;
  latest_by_subject: LedgerLatestBySubjectResult;
  cut: LedgerCutResult | null;
  guarantees: string[];
}

export interface LedgerIngestRefusalResult extends JsonObject {
  ok: false;
  schema: "bioprism-mcp/ledger-ingest/0.1";
  stage: "append";
  event_index: number;
  refusal: string;
  fail_closed: true;
  ledger_before_refusal: LedgerBeforeRefusalResult;
  guarantee: string;
}

export type LedgerIngestResult = LedgerIngestSuccessResult | LedgerIngestRefusalResult;

export type QualityCheckArgs =
  | { NotNull: { column: string } }
  | { Unique: { column: string } }
  | { InRange: { column: string; min: number; max: number } }
  | { OneOf: { column: string; allowed: string[] } }
  | { RowCountAtLeast: { rows: number } }
  | { NonDecreasing: { column: string } }
  | { ForeignKey: { column: string; reference: string } };

export interface QualityDatasetArgs extends JsonObject {
  name: string;
  columns: Record<string, JsonValue[]>;
  rows: number;
}

export interface QualityGateArgs extends JsonObject {
  name: string;
  checks: Record<string, QualityCheckArgs>;
}

export interface QualityReferenceSetsArgs extends JsonObject {
  sets: Record<string, string[]>;
}

export interface QualityGateRunArgs extends JsonObject {
  dataset: QualityDatasetArgs;
  gate: QualityGateArgs;
  references?: QualityReferenceSetsArgs;
}

export interface QualityWitnessResult extends JsonObject {
  row: number;
  column: string;
  found: string;
  expected: string;
}

export type QualityNotRunnableResult =
  | { MissingColumn: { column: string } }
  | { AllValuesNull: { column: string } }
  | { NotComparable: { column: string; row: number; found: string } }
  | { MissingReferenceSet: { reference: string } };

export type QualityOutcomeResult =
  | { Pass: { examined: number } }
  | { Fail: { witness: QualityWitnessResult } }
  | { NotRunnable: { reason: QualityNotRunnableResult } };

export type QualityVerdictResult =
  | { Passed: { checks: number } }
  | { Failed: { failing: string[]; not_runnable: string[] } }
  | { Indeterminate: { not_runnable: string[] } };

export interface QualityGateReportResult extends JsonObject {
  gate: string;
  dataset: string;
  rows: number;
  outcomes: Record<string, QualityOutcomeResult>;
  verdict: QualityVerdictResult;
}

export interface QualityGateRunResult extends JsonObject {
  ok: true;
  schema: "bioprism-mcp/quality-gate/0.1";
  verdict: "passed" | "failed" | "indeterminate";
  passed: boolean;
  dataset: string;
  rows: number;
  check_count: number;
  report: QualityGateReportResult;
  guarantees: string[];
}

export interface AtlasReportArgs extends JsonObject {
  atlas: JsonObject;
  weighting?: JsonObject;
  max_items?: number;
}

export interface AtlasMeasuredEntryResult extends JsonObject {
  capability: string;
  family: string;
  score: number;
  depth: string;
  evaluable: number;
  excluded: number;
  effective_size: number;
  generated_instances: number;
  permitted_claim: string;
}

export interface AtlasHoleResult extends JsonObject {
  capability: string;
  family: string;
  reason: string;
  influence: string;
  aggregate: boolean;
  blocks_claims_for: string[];
}

export interface AtlasFamilyCoverageResult extends JsonObject {
  family: string;
  total: number;
  measured: number;
  holes: number;
}

export interface AtlasHistogramEntryResult extends JsonObject {
  depth?: string;
  stage?: string;
  count: number;
}

export interface AtlasCoverageDebtResult extends JsonObject {
  total_capabilities: number;
  measured: number;
  unmeasured: number;
  closed_by_declaration: number;
  dark_families: string[];
  unclassified_failures: number;
  undiagnosed_failures: number;
}

export interface AtlasInconsistencyResult extends JsonObject {
  kind: string;
  capability?: string;
  failure_id?: string;
  failures_recorded?: number;
  failed_trials?: number;
}

export interface AtlasCompositeValueResult extends JsonObject {
  intended_use: string;
  value: number;
  weighted_capabilities: number;
  tier: string;
}

export type AtlasCompositeResult =
  | null
  | { ok: true; value: AtlasCompositeValueResult }
  | { ok: false; refusal: string; fail_closed: true; guarantee?: string };

export interface AtlasSummaryResult extends JsonObject {
  measured: number;
  holes: number;
  families: number;
  inconsistencies: number;
  coverage_debt_ratio: number;
  has_holes: boolean;
  coverage_supports_aggregation: boolean;
}

export interface AtlasReportResult extends JsonObject {
  ok: true;
  schema: "bioprism-mcp/atlas-report/0.1";
  ontology_version: string;
  summary: AtlasSummaryResult;
  debt: AtlasCoverageDebtResult;
  measured: AtlasMeasuredEntryResult[];
  omitted_measured: number;
  holes: AtlasHoleResult[];
  omitted_holes: number;
  family_coverage: AtlasFamilyCoverageResult[];
  omitted_families: number;
  depth_histogram: AtlasHistogramEntryResult[];
  stage_histogram: AtlasHistogramEntryResult[];
  inconsistencies: AtlasInconsistencyResult[];
  omitted_inconsistencies: number;
  composite: AtlasCompositeResult;
  guarantees: string[];
  limitations: string[];
}

export type AtlasSurfaceFacet =
  | "mechanism"
  | "first_divergence_stage"
  | "severity"
  | "inducement"
  | "architecture_component";

export interface AtlasSurfaceAuditArgs extends JsonObject {
  grid: JsonObject;
  later_grid?: JsonObject;
  failures?: JsonObject[];
  failure_subject?: string;
  facet?: AtlasSurfaceFacet;
  visibility?: JsonObject[];
  rate_capabilities?: string[];
  require_no_holes?: boolean;
  require_no_blocking_debt?: boolean;
  require_no_withheld?: boolean;
  require_sound_surfaces?: boolean;
  max_items?: number;
}

export interface AtlasSurfaceCoverageResult extends JsonObject {
  subject: string;
  total_capabilities: number;
  measured: number;
  unmeasured: number;
  blocking: number;
  closed_by_declaration: number;
  vacuous: boolean;
  holes: JsonObject[];
  omitted_holes: number;
  profile_coverage: JsonObject;
}

export interface AtlasSurfaceBrowseResult extends JsonObject {
  subject: string;
  facet: AtlasSurfaceFacet;
  taxonomy_version: string;
  records_browsed: number;
  visible: number;
  withheld: number;
  contested: number;
  undiagnosed: number;
  evaluator_induced: number;
  distinct_families: number;
  shares_sum_to_one: boolean;
  buckets: JsonObject[];
  omitted_buckets: number;
}

export interface AtlasSurfaceAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/atlas-surface-audit/0.1";
  workflow?: "atlas_surface_audit";
  coverage?: AtlasSurfaceCoverageResult;
  debt_discharge?: JsonObject | null;
  failure_browse?: AtlasSurfaceBrowseResult;
  rate_checks?: JsonObject;
  surface_audits?: JsonObject;
  policies?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface AdaptivePanelRunArgs extends JsonObject {
  panel: JsonObject;
  candidates?: JsonObject[];
  batch_size?: number;
  capability?: string;
  left?: string;
  right?: string;
  max_items?: number;
}

export interface AdaptiveIntervalResult extends JsonObject {
  lo: number;
  hi: number;
  credibility: number;
}

export interface AdaptiveShortfallResult extends JsonObject {
  kind: string;
  have?: number;
  need?: number;
  parent?: string;
  share?: number;
  cap?: number;
}

export interface AdaptiveCoverageResult extends JsonObject {
  capability: string;
  trials: number;
  parents: number;
  qualifying_parents: number;
  abstentions: number;
  shortfalls: AdaptiveShortfallResult[];
}

export interface AdaptiveIccResult extends JsonObject {
  kind: string;
  rho?: number;
  raw?: number;
  assumed?: number;
  reason?: string;
}

export interface AdaptiveBetaPosteriorResult extends JsonObject {
  alpha: number;
  beta: number;
}

export interface AdaptiveEstimateResult extends JsonObject {
  capability: string;
  trials: number;
  successes: number;
  abstentions: number;
  parents: number;
  posterior_mean: number;
  icc: AdaptiveIccResult;
  design_effect: number;
  effective_trials: number;
  naive_posterior: AdaptiveBetaPosteriorResult;
  clustered_posterior: AdaptiveBetaPosteriorResult;
  naive_interval: AdaptiveIntervalResult;
  clustered_interval: AdaptiveIntervalResult;
  bootstrap_interval?: AdaptiveIntervalResult | null;
  inflation: number;
  caveat: string;
}

export interface AdaptiveStoppingResult extends JsonObject {
  capability: string;
  reason: string;
  stop: boolean;
  conclusive: boolean;
  trials: number;
  effective_trials: number;
  design_effect: number;
  remaining_budget: number;
  interval: AdaptiveIntervalResult;
  best_case_width: number;
  detail: string;
}

export interface AdaptiveCapabilityAuditResult extends JsonObject {
  capability: string;
  cost: number;
  coverage: AdaptiveCoverageResult;
  stopping: AdaptiveStoppingResult;
  estimate: AdaptiveEstimateResult | null;
  withheld?: string | null;
}

export interface AdaptivePanelAuditResult extends JsonObject {
  trials: number;
  scored_trials: number;
  abstentions: number;
  total_cost: number;
  capabilities: AdaptiveCapabilityAuditResult[];
  caveat: string;
}

export interface AdaptiveScoredCandidateResult extends JsonObject {
  instance: string;
  capability: string;
  parent: string;
  score: number;
  expected_variance_reduction: number;
  independence_weight: number;
  cost: number;
  parent_trials_before: number;
}

export interface AdaptiveSelectionRecordResult extends JsonObject {
  chosen: AdaptiveScoredCandidateResult;
  eligible: number;
  already_run: number;
  coverage_gated_out: number;
  gated_by?: JsonObject | null;
  runners_up: AdaptiveScoredCandidateResult[];
  icc_used: number;
  icc_source: string;
  caveat: string;
}

export type AdaptiveSelectionResult =
  | { ok: true; value: { mode: "next"; record: AdaptiveSelectionRecordResult } }
  | { ok: true; value: { mode: "batch"; records: AdaptiveSelectionRecordResult[]; omitted: number } }
  | { ok: false; refusal: string; fail_closed: true };

export interface AdaptiveCapabilityViewResult extends JsonObject {
  capability: string;
  coverage: AdaptiveCoverageResult;
  stopping: AdaptiveStoppingResult | null;
  stopping_refusal?: string | null;
  estimate: AdaptiveEstimateResult | null;
  estimate_refusal?: string | null;
  fail_closed: boolean;
}

export interface AdaptiveComparisonResult extends JsonObject {
  left: string;
  right: string;
  left_mean: number;
  right_mean: number;
  left_effective_trials: number;
  right_effective_trials: number;
  probability_left_exceeds_right: number;
  naive_probability_left_exceeds_right: number;
  intervals_disjoint: boolean;
  caveat: string;
}

export interface AdaptivePanelResult extends JsonObject {
  ok: true;
  schema: "bioprism-mcp/adaptive-panel/0.1";
  audit: AdaptivePanelAuditResult;
  audit_summary: JsonObject;
  audit_digest?: string | null;
  selection: AdaptiveSelectionResult | null;
  capability: AdaptiveCapabilityViewResult | null;
  comparison: { ok: true; value: AdaptiveComparisonResult } | { ok: false; refusal: string; fail_closed: true } | null;
  finished?: boolean | null;
  finished_refusal?: string | null;
  guarantees: string[];
  limitations: string[];
}

export interface PosteriorGateArgs extends JsonObject {
  observations: JsonObject[];
  credit_policy?: JsonObject;
  gate?: JsonObject;
  other_observations?: JsonObject[];
  tolerance?: number;
  min_effective?: number;
}

export type PosteriorIccResult =
  | { icc: "estimated"; value: number }
  | { icc: "not_applicable" }
  | { icc: "undefined"; reason: string };

export interface PosteriorEstimateResult extends JsonObject {
  label: string;
  mean: number;
  naive_instance_mean: number;
  instances: number;
  clusters: number;
  largest_cluster: number;
  icc: PosteriorIccResult;
  effective_sample_size: number;
  unknown_instances: number;
  unknown_fraction: number;
}

export interface PosteriorVetoResult extends JsonObject {
  kind: string;
  detail: string;
  evaluator: string;
}

export interface PosteriorCapabilityResult extends JsonObject {
  capability: string;
  pass_rate: PosteriorEstimateResult;
  credit: PosteriorEstimateResult;
  outcome_rate: PosteriorEstimateResult;
  vetoes: PosteriorVetoResult[];
  disputed: number;
  abstained: number;
  optimistic_weak_evidence: number;
  weakest_tier: string;
}

export interface PosteriorGateScalarResult extends JsonObject {
  gate: string;
  value: number;
  formula: string;
  rationale: string;
  terms: [string, number, number][];
  sensitivity: [string, number][];
  weakest_tier: string;
  min_effective_sample: number;
}

export type PosteriorGateDecisionResult =
  | null
  | { ok: true; value: PosteriorGateScalarResult }
  | { ok: false; refusal: string; fail_closed: true; guarantee?: string };

export type PosteriorDominanceResult =
  | { dominance: "dominates" }
  | { dominance: "dominated_by" }
  | { dominance: "equivalent" }
  | { dominance: "incomparable"; better: string[]; worse: string[]; uncertain: string[] };

export interface PosteriorComparisonResult extends JsonObject {
  ok: true;
  dominance: PosteriorDominanceResult;
  tolerance: number;
  min_effective: number;
}

export interface PosteriorGateSuccessResult extends JsonObject {
  ok: true;
  schema: "bioprism-mcp/posterior-gate/0.1";
  schema_version: string;
  observations: number;
  unprovenanced_observations: number;
  capabilities: Record<string, PosteriorCapabilityResult>;
  gate: PosteriorGateDecisionResult;
  comparison: PosteriorComparisonResult | null;
  guarantees: string[];
  limitations: string[];
}

export interface PosteriorGateRefusalResult extends JsonObject {
  ok: false;
  schema: "bioprism-mcp/posterior-gate/0.1";
  stage: "credit_policy" | "posterior" | "comparison_posterior" | string;
  refusal: string;
  fail_closed: true;
}

export type PosteriorGateResult = PosteriorGateSuccessResult | PosteriorGateRefusalResult;

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
  schema: "bioprism-mcp/bioatlas-publication-audit/0.1";
  workflow: "bioatlas_publication_audit";
  atlas: JsonObject;
  evidence_audit: JsonObject | null;
  card: HubCardRenderResult | null;
  leaderboard: HubLeaderboardRenderResult | null;
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
  ci_evidence?: JsonObject;
  ci_provider?: CiProviderNormalizationArgs;
  ci_provider_evidence?: CiProviderEvidenceArgs;
  execution_provenance?: JsonObject;
  release_request?: JsonObject;
}

export interface DeveloperDeliveryReceiptArgs extends JsonObject {
  receipt_id: string;
  delivery: JsonObject;
}

export interface DeveloperDeliveryReceiptTargetResult extends JsonObject {
  target: string;
  available: boolean;
  eligible: boolean;
  blockers: string[];
  notes: string[];
  ready: boolean;
}

export interface DeveloperDeliveryReceiptEvidenceResult extends JsonObject {
  name: string;
  present: boolean;
  ready: boolean;
  digest: string | null;
}

export interface DeveloperDeliveryReceiptResult extends JsonObject {
  ok: boolean;
  workflow: "developer_delivery_receipt";
  schema: "bioprism-devplat-delivery-receipt/0.1";
  receipt_id: string;
  delivery_digest: string;
  target_digest: string;
  receipt_digest: string;
  valid: boolean;
  receipt_ready: boolean;
  release_request_ready: boolean;
  structurally_valid: boolean;
  release_candidate: boolean;
  target_count: number;
  available_target_count: number;
  ready_target_count: number;
  blocked_target_count: number;
  ready_evidence_count: number;
  targets: DeveloperDeliveryReceiptTargetResult[];
  evidence: DeveloperDeliveryReceiptEvidenceResult[];
  findings: JsonObject[];
  delivery: DeveloperDeliveryAuditResult;
  guarantees: string[];
  limitations: string[];
}

export interface DeveloperDeliveryReceiptVerificationArgs extends JsonObject {
  receipt: JsonObject;
  delivery: JsonObject;
}

export interface DeveloperDeliveryReceiptVerificationResult extends JsonObject {
  ok: boolean;
  workflow: "developer_delivery_receipt_verify";
  schema: "bioprism-devplat-delivery-receipt/0.1";
  receipt_id: string;
  supplied_receipt_digest: string | null;
  recomputed_receipt_digest: string;
  delivery_digest_match: boolean;
  target_digest_match: boolean;
  receipt_digest_match: boolean;
  targets_match: boolean;
  evidence_match: boolean;
  valid: boolean;
  verified: boolean;
  structurally_valid: boolean;
  findings: JsonObject[];
  guarantees: string[];
  limitations: string[];
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
  ci_execution_evidence_ready?: boolean;
  ci_provider_evidence_ready?: boolean;
  execution_provenance_ready?: boolean;
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
  ci_evidence: JsonObject | null;
  ci_provider_normalization?: JsonObject | null;
  ci_provider_evidence?: JsonObject | null;
  execution_provenance: JsonObject | null;
  readiness: DeveloperDeliveryReadinessResult;
  external_surface_posture: DeveloperDeliveryExternalSurfaceResult;
  release_request: DeveloperDeliveryReleaseRequestResult;
  guarantees: string[];
  limitations: string[];
}

export type EngineeringTicketStatus = "planned" | "in_progress" | "blocked" | "done";
export type EngineeringAdrStatus = "proposed" | "accepted" | "superseded" | "rejected";
export type EngineeringIssueSeverity = "warning" | "blocking";

export interface EngineeringProjectIdentityArgs extends JsonObject {
  id: string;
  version: string;
  repository: string;
}

export interface EngineeringTechnologyBaselineArgs extends JsonObject {
  language: string;
  runtime: string;
  api: string;
  storage: string;
  observability: string;
  deployment: string;
  reasons?: Record<string, string>;
}

export interface EngineeringPackageSpecArgs extends JsonObject {
  id: string;
  path: string;
  language: string;
  kind: string;
  owner: string;
  depends_on?: string[];
  public?: boolean;
  test_command?: string;
}

export interface EngineeringTicketSpecArgs extends JsonObject {
  id: string;
  title: string;
  package: string;
  contract: string;
  status: EngineeringTicketStatus;
  depends_on?: string[];
  acceptance: string[];
  blocker?: string;
}

export interface EngineeringAdrSpecArgs extends JsonObject {
  id: string;
  title: string;
  status: EngineeringAdrStatus;
  decision: string;
  affects: string[];
  supersedes?: string;
}

export interface EngineeringOwnershipSpecArgs extends JsonObject {
  surface: string;
  accountable: string;
  responsible: string[];
  consulted?: string[];
  informed?: string[];
  independent_reviewer?: string;
}

export interface EngineeringPoliciesArgs extends JsonObject {
  require_acyclic_packages?: boolean;
  require_ticket_contracts?: boolean;
  require_ownership?: boolean;
  require_adr_targets?: boolean;
}

export interface EngineeringManifestArgs extends JsonObject {
  schema?: "bioprism-engineering-manifest/0.1";
  project: EngineeringProjectIdentityArgs;
  baseline: EngineeringTechnologyBaselineArgs;
  packages?: EngineeringPackageSpecArgs[];
  tickets?: EngineeringTicketSpecArgs[];
  adrs?: EngineeringAdrSpecArgs[];
  ownership?: EngineeringOwnershipSpecArgs[];
  policies?: EngineeringPoliciesArgs;
}

export interface EngineeringIssueResult extends JsonObject {
  code: string;
  severity: EngineeringIssueSeverity;
  subject: string;
  detail: string;
  remediation: string;
}

export interface EngineeringTicketReadinessResult extends JsonObject {
  ticket_id: string;
  status: EngineeringTicketStatus;
  state: "complete" | "blocked" | "waiting" | "actionable" | string;
  blocking_dependencies: string[];
  dependency_ready: boolean;
}

export interface EngineeringCountsResult extends JsonObject {
  packages: number;
  public_packages: number;
  tickets: number;
  completed_tickets: number;
  actionable_tickets: number;
  adrs: number;
  accepted_adrs: number;
  ownership_rows: number;
}

export interface EngineeringAuditResult extends JsonObject {
  schema: "bioprism-engineering-audit/0.1";
  manifest_schema: string;
  digest: string;
  valid: boolean;
  counts: EngineeringCountsResult;
  package_order: string[];
  cyclic_packages: string[][];
  ticket_readiness: EngineeringTicketReadinessResult[];
  adr_supersession: Array<{ newer: string; older: string; valid: boolean }>;
  ownership_surfaces: string[];
  issues: EngineeringIssueResult[];
  guarantees: string[];
  limitations: string[];
}

export interface EngineeringManifestAuditResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-engineering-audit/0.1";
  workflow: "engineering_manifest_audit";
  manifest_digest: string;
  valid: boolean;
  blocking_issue_count: number;
  warning_count: number;
  audit: EngineeringAuditResult;
  guarantees: string[];
  limitations: string[];
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export interface EngineeringPlanPoliciesArgs extends JsonObject {
  require_valid_manifest?: boolean;
  allow_truncation?: boolean;
  include_completed?: boolean;
  serialize_same_package?: boolean;
  max_tickets?: number;
  max_parallelism?: number;
}

export interface EngineeringPlanRequestArgs extends JsonObject {
  schema?: "bioprism-engineering-plan/0.1";
  manifest: EngineeringManifestArgs;
  policies?: EngineeringPlanPoliciesArgs;
}

export interface EngineeringTicketPlanResult extends JsonObject {
  ticket_id: string;
  package: string;
  contract: string;
  status: EngineeringTicketStatus;
  state: string;
  dependency_ids: string[];
  blocking_dependencies: string[];
  dependency_ready: boolean;
  scheduled: boolean;
  wave: number | null;
  critical_path_length: number;
}

export interface EngineeringPlanWaveResult extends JsonObject {
  index: number;
  ticket_ids: string[];
  package_ids: string[];
  depends_on_waves: number[];
  parallelism: number;
}

export interface EngineeringPlanGateResult extends JsonObject {
  name: string;
  passed: boolean;
  required: boolean;
  detail: string;
}

export interface EngineeringPlanAuditResult extends JsonObject {
  schema: "bioprism-engineering-plan-audit/0.1";
  valid: boolean;
  planning_started: boolean;
  truncated: boolean;
  ticket_count: number;
  planned_ticket_count: number;
  omitted_ticket_count: number;
  package_order: string[];
  ticket_plans: EngineeringTicketPlanResult[];
  waves: EngineeringPlanWaveResult[];
  critical_path: string[];
  gates: EngineeringPlanGateResult[];
  manifest_issues: EngineeringIssueResult[];
  issues: EngineeringIssueResult[];
  guarantees: string[];
  limitations: string[];
}

export interface EngineeringPlanToolResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-engineering-plan-audit/0.1";
  workflow: "engineering_execution_plan";
  request_digest?: string;
  manifest_digest?: string;
  plan_digest?: string;
  valid: boolean;
  engineering_plan_ready: boolean;
  blocking_issue_count: number;
  warning_count: number;
  audit?: EngineeringPlanAuditResult;
  guarantees?: string[];
  limitations?: string[];
  refusal?: string;
  fail_closed?: boolean;
}

export type ReleasePipelineEnvironmentClass = "development" | "staging" | "production";
export type ReleasePipelineStageKind = "verify" | "build" | "test" | "package" | "sign" | "publish" | "deploy" | "smoke" | "rollback";
export type ReleasePipelineArtifactKind = "source" | "binary" | "container" | "package" | "manifest" | "sbom" | "provenance";
export type ReleasePipelineAttestationKind = "test" | "provenance" | "signature" | "approval";
export type ReleasePipelinePromotionKind = "advance" | "rollback";
export type ReleasePipelineIssueSeverity = "warning" | "blocking";

export interface ReleasePipelineProjectArgs extends JsonObject {
  id: string;
  version: string;
  repository: string;
}

export interface ReleasePipelineSourceArgs extends JsonObject {
  ref_name: string;
  commit_digest: string;
  workflow: string;
}

export interface ReleasePipelineEnvironmentArgs extends JsonObject {
  id: string;
  class: ReleasePipelineEnvironmentClass;
  protected?: boolean;
  required_approvals?: number;
  secrets_allowed?: boolean;
  immutable_artifacts?: boolean;
}

export interface ReleasePipelineStageArgs extends JsonObject {
  id: string;
  kind: ReleasePipelineStageKind;
  environment: string;
  depends_on?: string[];
  command?: string;
  produces?: string[];
  required?: boolean;
}

export interface ReleasePipelineArtifactArgs extends JsonObject {
  id: string;
  kind: ReleasePipelineArtifactKind;
  digest: string;
  produced_by: string;
  inputs?: string[];
  attestations?: string[];
  immutable?: boolean;
}

export interface ReleasePipelineAttestationArgs extends JsonObject {
  id: string;
  kind: ReleasePipelineAttestationKind;
  artifact: string;
  digest: string;
  issuer: string;
  statement: string;
}

export interface ReleasePipelinePromotionArgs extends JsonObject {
  id: string;
  kind: ReleasePipelinePromotionKind;
  from: string;
  to: string;
  artifacts?: string[];
  required_attestations?: string[];
  approvals?: string[];
  rollback_target?: string;
}

export interface ReleasePipelinePoliciesArgs extends JsonObject {
  require_stage_dag?: boolean;
  require_provenance?: boolean;
  require_production_signature?: boolean;
  require_protected_production?: boolean;
  require_rollback?: boolean;
  require_approval?: boolean;
}

export interface ReleasePipelineManifestArgs extends JsonObject {
  schema?: "bioprism-release-pipeline/0.1";
  project: ReleasePipelineProjectArgs;
  source: ReleasePipelineSourceArgs;
  environments?: ReleasePipelineEnvironmentArgs[];
  stages?: ReleasePipelineStageArgs[];
  artifacts?: ReleasePipelineArtifactArgs[];
  attestations?: ReleasePipelineAttestationArgs[];
  promotions?: ReleasePipelinePromotionArgs[];
  policies?: ReleasePipelinePoliciesArgs;
}

export interface ReleasePipelineIssueResult extends JsonObject {
  code: string;
  severity: ReleasePipelineIssueSeverity;
  subject: string;
  detail: string;
  remediation: string;
}

export interface ReleasePipelineStageReadinessResult extends JsonObject {
  stage_id: string;
  state: string;
  dependency_ready: boolean;
  blocking_dependencies: string[];
}

export interface ReleasePipelineArtifactAuditResult extends JsonObject {
  artifact_id: string;
  digest_valid: boolean;
  producer_valid: boolean;
  inputs_valid: boolean;
  attestations_valid: boolean;
  provenance_present: boolean;
  signature_present: boolean;
}

export interface ReleasePipelinePromotionAuditResult extends JsonObject {
  promotion_id: string;
  from: string;
  to: string;
  valid: boolean;
  production: boolean;
  missing_attestations: string[];
  missing_approvals: string[];
  rollback_present: boolean;
}

export interface ReleasePipelineCountsResult extends JsonObject {
  environments: number;
  protected_environments: number;
  stages: number;
  required_stages: number;
  artifacts: number;
  attestations: number;
  promotions: number;
  production_promotions: number;
}

export interface ReleasePipelineAuditResult extends JsonObject {
  schema: "bioprism-release-pipeline-audit/0.1";
  manifest_schema: string;
  digest: string;
  valid: boolean;
  counts: ReleasePipelineCountsResult;
  stage_order: string[];
  cyclic_stages: string[][];
  stage_readiness: ReleasePipelineStageReadinessResult[];
  artifact_audits: ReleasePipelineArtifactAuditResult[];
  promotion_audits: ReleasePipelinePromotionAuditResult[];
  issues: ReleasePipelineIssueResult[];
  guarantees: string[];
  limitations: string[];
}

export interface ReleasePipelineAuditToolResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-release-pipeline-audit/0.1";
  workflow: "release_pipeline_audit";
  manifest_digest: string;
  valid: boolean;
  release_ready: boolean;
  blocking_issue_count: number;
  warning_count: number;
  audit: ReleasePipelineAuditResult;
  guarantees: string[];
  limitations: string[];
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export type OperationalReadinessCriticality = "critical" | "important" | "advisory";
export type OperationalReadinessContractKind = "availability" | "latency" | "durability" | "recovery" | "security" | "privacy" | "capacity";
export type OperationalReadinessIndicatorStatus = "observed" | "not_observed" | "blocked" | "not_applicable";
export type OperationalReadinessDependencyCriticality = "critical" | "important" | "advisory";
export type OperationalReadinessRunbookReviewStatus = "draft" | "reviewed" | "expired";
export type OperationalReadinessIncidentSeverity = "sev1" | "sev2" | "sev3" | "sev4";
export type OperationalReadinessIncidentState = "open" | "contained" | "resolved" | "closed";
export type OperationalReadinessIssueSeverity = "warning" | "blocking";

export interface OperationalReadinessServiceArgs extends JsonObject {
  id: string;
  version: string;
  owner: string;
  criticality: OperationalReadinessCriticality;
}

export interface OperationalReadinessContractArgs extends JsonObject {
  id: string;
  kind: OperationalReadinessContractKind;
  objective: string;
  target: string;
  required?: boolean;
}

export interface OperationalReadinessIndicatorArgs extends JsonObject {
  id: string;
  contract: string;
  metric: string;
  source: string;
  status: OperationalReadinessIndicatorStatus;
  measurement?: string;
  evidence_digest?: string;
}

export interface OperationalReadinessDependencyArgs extends JsonObject {
  id: string;
  name: string;
  owner: string;
  criticality: OperationalReadinessDependencyCriticality;
  failure_mode: string;
  fallback?: string;
}

export interface OperationalReadinessRunbookArgs extends JsonObject {
  id: string;
  trigger: string;
  owner: string;
  steps: string[];
  review_status: OperationalReadinessRunbookReviewStatus;
  incident_classes?: string[];
}

export interface OperationalReadinessIncidentArgs extends JsonObject {
  id: string;
  severity: OperationalReadinessIncidentSeverity;
  state: OperationalReadinessIncidentState;
  runbook: string;
  owner: string;
  timeline?: string[];
  postmortem?: string;
}

export interface OperationalReadinessControlsArgs extends JsonObject {
  on_call?: boolean;
  alerting?: boolean;
  tracing?: boolean;
  audit_logging?: boolean;
  backup?: boolean;
  restore_test?: boolean;
  access_review?: boolean;
}

export interface OperationalReadinessPoliciesArgs extends JsonObject {
  require_contract_evidence?: boolean;
  require_observability?: boolean;
  require_runbooks?: boolean;
  require_restore_test?: boolean;
  require_dependency_fallback?: boolean;
  require_incident_closure?: boolean;
  require_access_review?: boolean;
}

export interface OperationalReadinessManifestArgs extends JsonObject {
  schema?: "bioprism-operational-readiness/0.1";
  service: OperationalReadinessServiceArgs;
  contracts?: OperationalReadinessContractArgs[];
  indicators?: OperationalReadinessIndicatorArgs[];
  dependencies?: OperationalReadinessDependencyArgs[];
  runbooks?: OperationalReadinessRunbookArgs[];
  incidents?: OperationalReadinessIncidentArgs[];
  controls?: OperationalReadinessControlsArgs;
  policies?: OperationalReadinessPoliciesArgs;
}

export interface OperationalReadinessIssueResult extends JsonObject {
  code: string;
  severity: OperationalReadinessIssueSeverity;
  subject: string;
  detail: string;
  remediation: string;
}

export interface OperationalReadinessIndicatorAuditResult extends JsonObject {
  indicator_id: string;
  contract_valid: boolean;
  source_valid: boolean;
  observed: boolean;
  evidence_valid: boolean;
  ready: boolean;
}

export interface OperationalReadinessDependencyAuditResult extends JsonObject {
  dependency_id: string;
  owner_valid: boolean;
  failure_mode_valid: boolean;
  fallback_present: boolean;
  critical: boolean;
  ready: boolean;
}

export interface OperationalReadinessRunbookAuditResult extends JsonObject {
  runbook_id: string;
  valid: boolean;
  review_current: boolean;
  step_count: number;
  referenced_incidents: number;
}

export interface OperationalReadinessIncidentAuditResult extends JsonObject {
  incident_id: string;
  valid: boolean;
  runbook_valid: boolean;
  timeline_present: boolean;
  postmortem_present: boolean;
  closed: boolean;
}

export interface OperationalReadinessControlAuditResult extends JsonObject {
  control: string;
  enabled: boolean;
  required: boolean;
  ready: boolean;
}

export interface OperationalReadinessCountsResult extends JsonObject {
  contracts: number;
  required_contracts: number;
  indicators: number;
  observed_indicators: number;
  dependencies: number;
  critical_dependencies: number;
  runbooks: number;
  incidents: number;
  open_incidents: number;
  controls: number;
  enabled_controls: number;
}

export interface OperationalReadinessAuditResult extends JsonObject {
  schema: "bioprism-operational-readiness-audit/0.1";
  manifest_schema: string;
  digest: string;
  valid: boolean;
  service_id: string;
  counts: OperationalReadinessCountsResult;
  indicator_audits: OperationalReadinessIndicatorAuditResult[];
  dependency_audits: OperationalReadinessDependencyAuditResult[];
  runbook_audits: OperationalReadinessRunbookAuditResult[];
  incident_audits: OperationalReadinessIncidentAuditResult[];
  control_audits: OperationalReadinessControlAuditResult[];
  issues: OperationalReadinessIssueResult[];
  guarantees: string[];
  limitations: string[];
}

export interface OperationalReadinessToolResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-operational-readiness-audit/0.1";
  workflow: "operational_readiness_audit";
  manifest_digest: string;
  valid: boolean;
  operationally_ready: boolean;
  blocking_issue_count: number;
  warning_count: number;
  audit: OperationalReadinessAuditResult;
  guarantees: string[];
  limitations: string[];
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export type SecurityPrivacyClassification = "public" | "internal" | "confidential" | "restricted" | "regulated";
export type SecurityPrivacyFlowDecision = "allow" | "deny" | "conditional";
export type SecurityPrivacyThreatSeverity = "low" | "medium" | "high" | "critical";
export type SecurityPrivacyThreatStatus = "mitigated" | "accepted" | "unmitigated" | "unanalysed";
export type SecurityPrivacyReviewKind = "privacy_impact" | "security_assessment" | "red_team" | "access_review";
export type SecurityPrivacyReviewStatus = "draft" | "in_review" | "complete" | "expired";
export type SecurityPrivacyIssueSeverity = "warning" | "blocking";

export interface SecurityPrivacySystemArgs extends JsonObject { id: string; version: string; owner: string; }
export interface SecurityPrivacyAssetArgs extends JsonObject { id: string; name: string; classification: SecurityPrivacyClassification; owner: string; purpose: string; retention_days?: number; residency: string; deletion_process?: string; }
export interface SecurityPrivacyFlowArgs extends JsonObject { id: string; asset: string; source: string; destination: string; purpose: string; decision: SecurityPrivacyFlowDecision; legal_basis?: string; authorization_evidence?: string; }
export interface SecurityPrivacyIdentityArgs extends JsonObject { id: string; principal: string; role: string; authentication: string; mfa?: boolean; least_privilege?: boolean; assets?: string[]; }
export interface SecurityPrivacyThreatArgs extends JsonObject { id: string; category: string; severity: SecurityPrivacyThreatSeverity; status: SecurityPrivacyThreatStatus; control?: string; evidence_digest?: string; rationale?: string; }
export interface SecurityPrivacyReviewArgs extends JsonObject { id: string; kind: SecurityPrivacyReviewKind; scope: string; reviewer: string; status: SecurityPrivacyReviewStatus; evidence_digest?: string; expires_at?: string; findings?: string[]; }
export interface SecurityPrivacyControlsArgs extends JsonObject { access_control?: boolean; encryption_at_rest?: boolean; encryption_in_transit?: boolean; key_rotation?: boolean; audit_logging?: boolean; vulnerability_management?: boolean; backup_restore?: boolean; incident_response?: boolean; vendor_review?: boolean; data_subject_rights?: boolean; }
export interface SecurityPrivacyPoliciesArgs extends JsonObject { require_asset_purpose?: boolean; require_retention?: boolean; require_flow_authorization?: boolean; require_identity_hardening?: boolean; require_threat_treatment?: boolean; require_reviews?: boolean; require_controls?: boolean; require_mfa_for_sensitive?: boolean; }
export interface SecurityPrivacyManifestArgs extends JsonObject { schema?: "bioprism-security-privacy/0.1"; system: SecurityPrivacySystemArgs; assets?: SecurityPrivacyAssetArgs[]; flows?: SecurityPrivacyFlowArgs[]; identities?: SecurityPrivacyIdentityArgs[]; threats?: SecurityPrivacyThreatArgs[]; reviews?: SecurityPrivacyReviewArgs[]; controls?: SecurityPrivacyControlsArgs; policies?: SecurityPrivacyPoliciesArgs; }

export interface SecurityPrivacyIssueResult extends JsonObject { code: string; severity: SecurityPrivacyIssueSeverity; subject: string; detail: string; remediation: string; }
export interface SecurityPrivacyAssetAuditResult extends JsonObject { asset_id: string; purpose_valid: boolean; retention_valid: boolean; residency_valid: boolean; deletion_valid: boolean; sensitive: boolean; ready: boolean; }
export interface SecurityPrivacyFlowAuditResult extends JsonObject { flow_id: string; asset_valid: boolean; purpose_valid: boolean; legal_basis_present: boolean; authorization_present: boolean; allowed: boolean; ready: boolean; }
export interface SecurityPrivacyIdentityAuditResult extends JsonObject { identity_id: string; assets_valid: boolean; authentication_valid: boolean; mfa: boolean; least_privilege: boolean; sensitive_access: boolean; ready: boolean; }
export interface SecurityPrivacyThreatAuditResult extends JsonObject { threat_id: string; high_or_worse: boolean; treated: boolean; control_present: boolean; evidence_valid: boolean; rationale_present: boolean; ready: boolean; }
export interface SecurityPrivacyReviewAuditResult extends JsonObject { review_id: string; reviewer_independent: boolean; evidence_valid: boolean; current: boolean; complete: boolean; ready: boolean; }
export interface SecurityPrivacyControlAuditResult extends JsonObject { control: string; enabled: boolean; required: boolean; ready: boolean; }
export interface SecurityPrivacyCountsResult extends JsonObject { assets: number; sensitive_assets: number; flows: number; allowed_flows: number; identities: number; hardened_identities: number; threats: number; high_or_worse_threats: number; treated_threats: number; reviews: number; current_reviews: number; controls: number; enabled_controls: number; }
export interface SecurityPrivacyAuditResult extends JsonObject { schema: "bioprism-security-privacy-audit/0.1"; manifest_schema: string; digest: string; valid: boolean; system_id: string; counts: SecurityPrivacyCountsResult; asset_audits: SecurityPrivacyAssetAuditResult[]; flow_audits: SecurityPrivacyFlowAuditResult[]; identity_audits: SecurityPrivacyIdentityAuditResult[]; threat_audits: SecurityPrivacyThreatAuditResult[]; review_audits: SecurityPrivacyReviewAuditResult[]; control_audits: SecurityPrivacyControlAuditResult[]; issues: SecurityPrivacyIssueResult[]; guarantees: string[]; limitations: string[]; }
export interface SecurityPrivacyToolResult extends JsonObject { ok: boolean; schema: "bioprism-security-privacy-audit/0.1"; workflow: "security_privacy_audit"; manifest_digest: string; valid: boolean; security_privacy_ready: boolean; blocking_issue_count: number; warning_count: number; audit: SecurityPrivacyAuditResult; guarantees: string[]; limitations: string[]; stage?: string; refusal?: string; fail_closed?: boolean; }

export type SandboxArtifactKind = "source_code" | "notebook" | "dataset" | "model" | "container" | "package" | "plugin" | "generated_output";
export type SandboxTrust = "untrusted" | "internal" | "reviewed" | "trusted";
export type SandboxNetworkMode = "deny" | "allowlist" | "unrestricted";
export type SandboxMountMode = "read_only" | "read_write";
export type SandboxCapabilityKind = "filesystem_read" | "filesystem_write" | "network_egress" | "network_ingress" | "secret_access" | "process_spawn" | "device_access" | "kernel_access" | "clock" | "randomness" | "artifact_publish";
export type SandboxDecision = "allow" | "deny";
export type SandboxIssueSeverity = "warning" | "blocking";

export interface SandboxSystemArgs extends JsonObject { id: string; version: string; owner: string; }
export interface SandboxArtifactArgs extends JsonObject { id: string; kind: SandboxArtifactKind; digest: string; source: string; producer: string; trust: SandboxTrust; inputs?: string[]; }
export interface SandboxMountArgs extends JsonObject { id: string; source_artifact: string; target: string; mode: SandboxMountMode; }
export interface SandboxResourceLimitsArgs extends JsonObject { cpu_millis?: number; memory_mb?: number; wall_time_seconds?: number; processes?: number; output_bytes?: number; }
export interface SandboxExecutionProfileArgs extends JsonObject { id: string; artifact: string; runtime: string; image_digest?: string; environment_digest?: string; user: string; rootless: boolean; read_only_root: boolean; no_privilege_escalation: boolean; network: SandboxNetworkMode; network_allowlist?: string[]; mounts?: SandboxMountArgs[]; capabilities?: string[]; resources?: SandboxResourceLimitsArgs; output_quarantine: boolean; release_requires_review: boolean; }
export interface SandboxCapabilityArgs extends JsonObject { id: string; profile: string; kind: SandboxCapabilityKind; target: string; decision: SandboxDecision; evidence_digest?: string; }
export interface SandboxOutputArgs extends JsonObject { id: string; profile: string; artifact: string; digest: string; destination: string; quarantined: boolean; released: boolean; reviewed: boolean; review_evidence?: string; parents?: string[]; }
export interface SandboxPoliciesArgs extends JsonObject { default_deny?: boolean; require_digests?: boolean; require_lineage?: boolean; require_rootless?: boolean; require_read_only_root?: boolean; require_no_privilege_escalation?: boolean; require_network_allowlist?: boolean; require_resource_limits?: boolean; require_quarantine?: boolean; require_output_review?: boolean; require_reproducible_environment?: boolean; }
export interface SandboxManifestArgs extends JsonObject { schema?: "bioprism-sandbox/0.1"; system: SandboxSystemArgs; artifacts?: SandboxArtifactArgs[]; profiles?: SandboxExecutionProfileArgs[]; capabilities?: SandboxCapabilityArgs[]; outputs?: SandboxOutputArgs[]; policies?: SandboxPoliciesArgs; }
export interface SandboxIssueResult extends JsonObject { code: string; severity: SandboxIssueSeverity; subject: string; detail: string; remediation: string; }
export interface SandboxArtifactAuditResult extends JsonObject { artifact_id: string; digest_valid: boolean; lineage_valid: boolean; source_valid: boolean; trust: SandboxTrust; hardening_required: boolean; ready: boolean; }
export interface SandboxProfileAuditResult extends JsonObject { profile_id: string; artifact_valid: boolean; isolation_valid: boolean; network_valid: boolean; mounts_valid: boolean; capabilities_valid: boolean; resources_valid: boolean; output_valid: boolean; ready: boolean; }
export interface SandboxCapabilityAuditResult extends JsonObject { capability_id: string; profile_valid: boolean; target_valid: boolean; approved: boolean; dangerous: boolean; evidence_valid: boolean; ready: boolean; }
export interface SandboxBoundaryAuditResult extends JsonObject { profile_id: string; default_deny: boolean; network_mode: SandboxNetworkMode; allowlist_valid: boolean; host_paths_rejected: boolean; dangerous_capabilities: number; ready: boolean; }
export interface SandboxResourceAuditResult extends JsonObject { profile_id: string; cpu_bounded: boolean; memory_bounded: boolean; wall_time_bounded: boolean; processes_bounded: boolean; output_bounded: boolean; ready: boolean; }
export interface SandboxOutputAuditResult extends JsonObject { output_id: string; profile_valid: boolean; artifact_valid: boolean; digest_valid: boolean; lineage_valid: boolean; quarantined: boolean; review_valid: boolean; release_valid: boolean; ready: boolean; }
export interface SandboxCountsResult extends JsonObject { artifacts: number; untrusted_artifacts: number; profiles: number; isolated_profiles: number; capabilities: number; approved_capabilities: number; dangerous_capabilities: number; outputs: number; quarantined_outputs: number; released_outputs: number; }
export interface SandboxAuditResult extends JsonObject { schema: "bioprism-sandbox-audit/0.1"; manifest_schema: string; digest: string; valid: boolean; system_id: string; counts: SandboxCountsResult; artifact_audits: SandboxArtifactAuditResult[]; profile_audits: SandboxProfileAuditResult[]; capability_audits: SandboxCapabilityAuditResult[]; boundary_audits: SandboxBoundaryAuditResult[]; resource_audits: SandboxResourceAuditResult[]; output_audits: SandboxOutputAuditResult[]; issues: SandboxIssueResult[]; guarantees: string[]; limitations: string[]; }
export interface SandboxAdmissionToolResult extends JsonObject { ok: boolean; schema: "bioprism-sandbox-audit/0.1"; workflow: "sandbox_admission_audit"; manifest_digest: string; valid: boolean; sandbox_ready: boolean; blocking_issue_count: number; warning_count: number; audit: SandboxAuditResult; guarantees: string[]; limitations: string[]; stage?: string; refusal?: string; fail_closed?: boolean; }

export type SandboxRuntimeDecision = "simulated" | "refused" | "not_run";
export interface SandboxRuntimeRequestArgs extends JsonObject { id: string; kind: SandboxCapabilityKind; target: string; cpu_millis: number; memory_mb: number; wall_time_seconds: number; processes: number; output_bytes: number; }
export interface SandboxRuntimePoliciesArgs extends JsonObject { stop_on_refusal?: boolean; require_admission?: boolean; max_requests?: number; }
export interface SandboxRuntimeManifestArgs extends JsonObject { schema?: "bioprism-sandbox-runtime/0.1"; admission: SandboxManifestArgs; profile: string; requests?: SandboxRuntimeRequestArgs[]; policies?: SandboxRuntimePoliciesArgs; }
export interface SandboxRuntimeUsageResult extends JsonObject { cpu_millis: number; memory_mb_peak: number; wall_time_seconds: number; processes_peak: number; output_bytes: number; }
export interface SandboxRuntimeStepResult extends JsonObject { request_id: string; kind: SandboxCapabilityKind; target: string; capability_id?: string | null; capability_valid: boolean; target_valid: boolean; resource_valid: boolean; decision: SandboxRuntimeDecision; charged: boolean; usage_after: SandboxRuntimeUsageResult; refusal?: string | null; }
export interface SandboxRuntimeAuditResult extends JsonObject { schema: "bioprism-sandbox-runtime-audit/0.1"; manifest_schema: string; admission_digest: string; trace_digest: string; valid: boolean; profile_id: string; admission_valid: boolean; simulation_started: boolean; completed: boolean; stopped_on_refusal: boolean; request_count: number; simulated_count: number; refused_count: number; not_run_count: number; usage: SandboxRuntimeUsageResult; steps: SandboxRuntimeStepResult[]; admission_issues: SandboxIssueResult[]; issues: SandboxIssueResult[]; guarantees: string[]; limitations: string[]; }
export interface SandboxRuntimeToolResult extends JsonObject { ok: boolean; schema: "bioprism-sandbox-runtime-audit/0.1"; workflow: "sandbox_runtime_simulate"; manifest_digest: string; admission_digest: string; trace_digest: string; valid: boolean; sandbox_runtime_ready: boolean; blocking_issue_count: number; warning_count: number; audit: SandboxRuntimeAuditResult; guarantees: string[]; limitations: string[]; stage?: string; refusal?: string; fail_closed?: boolean; }

export type SecurityProgramScopeKind = "service" | "api" | "model" | "dataset" | "workflow" | "research_artifact" | "vendor" | "organization";
export type SecurityProgramCampaignStatus = "planned" | "running" | "completed" | "stopped" | "cancelled";
export type SecurityProgramFindingSeverity = "informational" | "low" | "medium" | "high" | "critical";
export type SecurityProgramFindingStatus = "new" | "triaged" | "accepted" | "remediated" | "closed" | "false_positive" | "duplicate";
export type SecurityProgramRemediationStatus = "open" | "in_progress" | "blocked" | "complete" | "waived";
export type SecurityProgramIncidentStatus = "open" | "contained" | "closed" | "accepted";
export type SecurityProgramDisclosureStage = "withheld" | "internal" | "advisory" | "public";
export type SecurityProgramIssueSeverity = "warning" | "blocking";

export interface SecurityProgramSystemArgs extends JsonObject { id: string; version: string; owner: string; mission: string; }
export interface SecurityProgramScopeArgs extends JsonObject { id: string; name: string; kind: SecurityProgramScopeKind; target: string; owner: string; authorization_digest?: string; allowed_methods?: string[]; forbidden_actions?: string[]; environments?: string[]; data_handling?: string; }
export interface SecurityProgramCampaignArgs extends JsonObject { id: string; scope: string; operator: string; independent_reviewer?: string; methodology: string; hypothesis: string; status: SecurityProgramCampaignStatus; started_at?: string; completed_at?: string; evidence_digest?: string; stop_conditions?: string[]; finding_ids?: string[]; }
export interface SecurityProgramFindingArgs extends JsonObject { id: string; campaign: string; title: string; severity: SecurityProgramFindingSeverity; status: SecurityProgramFindingStatus; evidence_digest?: string; reproduction_digest?: string; regression_digest?: string; discovered_at: string; affected_targets?: string[]; remediation_ids?: string[]; incident_id?: string; public_safe?: boolean; resolution_note?: string; }
export interface SecurityProgramRemediationArgs extends JsonObject { id: string; finding: string; owner: string; action: string; status: SecurityProgramRemediationStatus; due_at: string; verification_digest?: string; rationale?: string; approval_digest?: string; }
export interface SecurityProgramTimelineEventArgs extends JsonObject { epoch: number; actor: string; event: string; evidence_digest?: string; }
export interface SecurityProgramIncidentArgs extends JsonObject { id: string; finding: string; severity: SecurityProgramFindingSeverity; owner: string; status: SecurityProgramIncidentStatus; opened_at: string; contained_at?: string; closed_at?: string; containment_evidence?: string; closure_evidence?: string; notification_required?: boolean; timeline?: SecurityProgramTimelineEventArgs[]; }
export interface SecurityProgramDisclosureArgs extends JsonObject { id: string; finding: string; stage: SecurityProgramDisclosureStage; audience: string; requested_at: string; approver?: string; approval_digest?: string; advisory_digest?: string; published_at?: string; }
export interface SecurityProgramControlsArgs extends JsonObject { scope_authorization?: boolean; operator_separation?: boolean; independent_review?: boolean; evidence_retention?: boolean; remediation_tracking?: boolean; incident_response?: boolean; disclosure_review?: boolean; regression_testing?: boolean; }
export interface SecurityProgramPoliciesArgs extends JsonObject { require_scope_authorization?: boolean; require_independent_review?: boolean; require_campaign_evidence?: boolean; require_finding_evidence?: boolean; require_remediation?: boolean; require_incident_for_high?: boolean; require_disclosure_approval?: boolean; require_regression_for_closed?: boolean; require_controls?: boolean; }
export interface SecurityProgramManifestArgs extends JsonObject { schema?: "bioprism-security-program/0.1"; system: SecurityProgramSystemArgs; scopes?: SecurityProgramScopeArgs[]; campaigns?: SecurityProgramCampaignArgs[]; findings?: SecurityProgramFindingArgs[]; remediations?: SecurityProgramRemediationArgs[]; incidents?: SecurityProgramIncidentArgs[]; disclosures?: SecurityProgramDisclosureArgs[]; controls?: SecurityProgramControlsArgs; policies?: SecurityProgramPoliciesArgs; }
export interface SecurityProgramIssueResult extends JsonObject { code: string; severity: SecurityProgramIssueSeverity; subject: string; detail: string; remediation: string; }
export interface SecurityProgramScopeAuditResult extends JsonObject { scope_id: string; authorization_valid: boolean; methods_valid: boolean; guardrails_valid: boolean; environments_valid: boolean; ready: boolean; }
export interface SecurityProgramCampaignAuditResult extends JsonObject { campaign_id: string; scope_valid: boolean; operator_present: boolean; independent_review_valid: boolean; methodology_valid: boolean; evidence_valid: boolean; complete: boolean; ready: boolean; }
export interface SecurityProgramFindingAuditResult extends JsonObject { finding_id: string; campaign_valid: boolean; evidence_valid: boolean; reproduction_valid: boolean; severity_requires_action: boolean; remediation_valid: boolean; incident_required: boolean; incident_valid: boolean; regression_present: boolean; ready: boolean; }
export interface SecurityProgramRemediationAuditResult extends JsonObject { remediation_id: string; finding_valid: boolean; owner_valid: boolean; completion_valid: boolean; verification_valid: boolean; ready: boolean; }
export interface SecurityProgramIncidentAuditResult extends JsonObject { incident_id: string; finding_valid: boolean; timeline_valid: boolean; containment_valid: boolean; closure_valid: boolean; notification_valid: boolean; ready: boolean; }
export interface SecurityProgramDisclosureAuditResult extends JsonObject { disclosure_id: string; finding_valid: boolean; stage_order_valid: boolean; approval_valid: boolean; advisory_valid: boolean; publication_valid: boolean; ready: boolean; }
export interface SecurityProgramControlAuditResult extends JsonObject { control: string; enabled: boolean; required: boolean; ready: boolean; }
export interface SecurityProgramCountsResult extends JsonObject { scopes: number; authorized_scopes: number; campaigns: number; completed_campaigns: number; findings: number; high_or_worse_findings: number; actionable_findings: number; remediations: number; completed_remediations: number; incidents: number; open_incidents: number; closed_incidents: number; disclosures: number; advisory_disclosures: number; public_disclosures: number; enabled_controls: number; }
export interface SecurityProgramAuditResult extends JsonObject { schema: "bioprism-security-program-audit/0.1"; manifest_schema: string; digest: string; valid: boolean; system_id: string; counts: SecurityProgramCountsResult; scope_audits: SecurityProgramScopeAuditResult[]; campaign_audits: SecurityProgramCampaignAuditResult[]; finding_audits: SecurityProgramFindingAuditResult[]; remediation_audits: SecurityProgramRemediationAuditResult[]; incident_audits: SecurityProgramIncidentAuditResult[]; disclosure_audits: SecurityProgramDisclosureAuditResult[]; control_audits: SecurityProgramControlAuditResult[]; issues: SecurityProgramIssueResult[]; guarantees: string[]; limitations: string[]; }
export interface SecurityProgramToolResult extends JsonObject { ok: boolean; schema: "bioprism-security-program-audit/0.1"; workflow: "security_program_audit"; manifest_digest: string; valid: boolean; security_program_ready: boolean; blocking_issue_count: number; warning_count: number; audit: SecurityProgramAuditResult; guarantees: string[]; limitations: string[]; stage?: string; refusal?: string; fail_closed?: boolean; }

export interface DeveloperPlatformStatusArgs extends JsonObject {
  include_details?: boolean;
  max_items?: number;
}

export type DeveloperPlatformStandingResult =
  | { standing: "checkable_here"; claims: number }
  | { standing: "partly_outside"; here: number; outside: number }
  | { standing: "entirely_outside"; claims: number };

export interface DeveloperPlatformWalkthroughResult extends JsonObject {
  id: string;
  goal: string;
  standing: DeveloperPlatformStandingResult;
  standing_text: "checkable here" | "partly outside" | "entirely outside";
  steps: number;
  claims: number;
  guarded_claims: number;
  unguarded_claims: number;
  documents_absent_artifact: boolean;
  refuted_claims: number;
  narration_permille: number;
}

export interface DeveloperPlatformSummaryResult extends JsonObject {
  digest: string;
  verdict_counts: [number, number, number, number];
  modules_classified: number;
  implemented_count: number;
  not_implemented_count: number;
  foreign_subject_count: number;
  walkthrough_count: number;
  guarded_claims: number;
  unguarded_claims: number;
}

export interface DeveloperPlatformCookbookVerificationResult extends JsonObject {
  clean: boolean;
  crates_checked: number;
  entry_points_checked: number;
  tests_checked: number;
  quotes_checked: number;
  defect_count: number;
  defects_returned: JsonObject[];
  omitted_defects: number;
}

export interface DeveloperPlatformCookbookResult extends JsonObject {
  recipes: number;
  anti_recipes: number;
  crates: string[];
  enforcing_tests: number;
  quotes: number;
  verification: DeveloperPlatformCookbookVerificationResult;
}

export interface DeveloperPlatformContractSurfaceResult extends JsonObject {
  id: string;
  owns_count: number;
  invalidates_count: number;
  rationale: string;
}

export interface DeveloperPlatformContractResult extends JsonObject {
  surface_count: number;
  surfaces_returned: DeveloperPlatformContractSurfaceResult[];
  omitted_surfaces: number;
}

export interface DeveloperPlatformDiagnosticCatalogueResult extends JsonObject {
  clean: boolean;
  checked: number;
  errors: number;
  warnings: number;
  finding_count: number;
  findings_returned: JsonObject[];
  omitted_findings: number;
}

export interface DeveloperPlatformExitCodeAuditResult extends JsonObject {
  clean: boolean;
  retry_decision_recoverable_from_code_alone: boolean;
  divergence_count: number;
  divergences_returned: JsonObject[];
  omitted_divergences: number;
}

export interface DeveloperPlatformDetailsResult extends JsonObject {
  devplat: JsonObject;
  cookbook_verification: JsonObject;
  developer_contract: JsonObject[];
  diagnostic_findings: JsonObject[];
  exit_code_divergences: JsonObject[];
}

export interface DeveloperPlatformStatusResult extends JsonObject {
  ok: boolean;
  root: string;
  detail_mode: "summary" | "full";
  max_items: number;
  devplat: DeveloperPlatformSummaryResult;
  walkthroughs: DeveloperPlatformWalkthroughResult[];
  cookbook: DeveloperPlatformCookbookResult;
  developer_contract: DeveloperPlatformContractResult;
  diagnostic_catalogue: DeveloperPlatformDiagnosticCatalogueResult;
  exit_code_audit: DeveloperPlatformExitCodeAuditResult;
  limitations: string[];
  details?: DeveloperPlatformDetailsResult;
}

export interface TokenContextRequestResult extends JsonObject {
  world_ref: string;
  decision_ref: string;
  role: string;
  policy_id: string;
  envelope: { total: number };
  depth: "dry_run" | "l0" | "l1" | "l2" | "l3";
  compiler_version: string;
}

export type TokenEstimationMethodResult =
  | { method: "chars_per_token4" }
  | { method: "declared_by_caller" }
  | { method: "provider_tokenizer"; name: string }
  | { method: "mixed"; methods: string[] };

export interface TokenEstimateResult extends JsonObject {
  tokens: number;
  method: TokenEstimationMethodResult;
}

export interface TokenPlanCandidateResult extends JsonObject {
  node_id: string;
  kind: "invariant" | "evidence" | "contradiction" | "negative_evidence" | "uncertainty" | "policy_restriction" | "summary" | "handle" | "attested_claim";
  mandatory?: boolean;
  restricted?: boolean;
  estimate: TokenEstimateResult;
}

export interface TokenContextPlanArgs extends JsonObject {
  request: TokenContextRequestResult;
  candidates: TokenPlanCandidateResult[];
  variant_request?: TokenContextRequestResult;
  variant_candidates?: TokenPlanCandidateResult[];
}

export interface TokenContextPlanResult extends JsonObject {
  request_digest: string;
  plan_digest: string;
  candidates: string[];
  mandatory: string[];
  handles: string[];
  mandatory_estimate: TokenEstimateResult;
  optional_estimate: TokenEstimateResult;
  envelope: { total: number };
}

export interface TokenPolicyComparisonResult extends JsonObject {
  comparison_id: string;
  mode: "policy_only";
  baseline_policy: string;
  variant_policy: string;
  baseline_plan: TokenContextPlanResult;
  variant_plan: TokenContextPlanResult;
}

export interface TokenContextPlanningResult extends JsonObject {
  ok: boolean;
  plan: TokenContextPlanResult;
  comparison: TokenPolicyComparisonResult | null;
  guarantees: string[];
}

export interface WeaveLangCompileArgs extends JsonObject {
  source: string;
  execute?: boolean;
  mode?: "replay" | "live";
  thread_id?: string;
  include_ir?: boolean;
  include_trace?: boolean;
}

export interface WeaveLangInvariantViolationResult extends JsonObject {
  invariant: "authority-safety" | "delegation-attenuation" | "budget-conservation" | "commitment-accountability" | "epistemic-integrity" | "information-non-escalation" | "causal-integrity" | "replay-safety";
  detail: string;
  at_event: number;
}

export interface WeaveLangLivenessResult extends JsonObject {
  messages_left_unconsumed: number;
  commitments_left_open: string[];
  states_without_exit: string[];
  unreachable_states: string[];
  deadlock_freedom_proven: boolean;
}

export interface WeaveLangProgramResult extends JsonObject {
  program_id: string;
  digest: string;
  semantic_digest: string;
  weave_ir_version: string;
  roles: number;
  participants: number;
  interfaces: number;
  policies: number;
  state_nodes: number;
  transitions: number;
  monitors: number;
  initial_state: string;
  terminal_states: string[];
}

export interface WeaveLangExecutionResult extends JsonObject {
  status: "not_requested" | "completed" | "refused";
  mode: "replay" | "live";
  state: string;
  liveness: WeaveLangLivenessResult;
  invariant_violations: WeaveLangInvariantViolationResult[];
  event_count?: number;
  trace_digest?: string;
  trace?: JsonObject | null;
  error?: string;
  fail_closed?: boolean;
}

export interface WeaveLangCompileResult extends JsonObject {
  ok: boolean;
  program: WeaveLangProgramResult;
  execution: WeaveLangExecutionResult;
  ir: JsonObject | null;
  guarantees: string[];
}

export interface EpistemicDecisionProblemArgs extends JsonObject {
  actions: string[];
  models: string[];
  loss: number[];
}

export interface EpistemicBeliefArgs extends JsonObject {
  mass: number[];
}

export interface EpistemicOutcomeArgs extends JsonObject {
  label: string;
  likelihood: number[];
}

export interface EpistemicAcquisitionArgs extends JsonObject {
  id: string;
  cost: number;
  outcomes: EpistemicOutcomeArgs[];
}

export interface EpistemicVoiArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  acquisition?: EpistemicAcquisitionArgs;
  acquisitions?: EpistemicAcquisitionArgs[];
}

export interface EpistemicValueResult extends JsonObject {
  gross: number;
  cost: number;
  net: number;
  outcome_probabilities: number[];
  action_without: number;
  action_after: number[];
}

export interface EpistemicActionsResult extends JsonObject {
  without: string;
  after: string[];
}

export interface EpistemicComplementarityResult extends JsonObject {
  joint_gross: number;
  sum_of_singletons: number;
  excess: number;
}

export interface EpistemicRefusalResult extends JsonObject {
  ok: false;
  stage?: string;
  refusal: string;
  fail_closed: true;
  guarantees: string[];
}

export interface EpistemicVoiResult extends JsonObject {
  ok: boolean;
  mode?: "single" | "non_adaptive_joint_bundle";
  value?: EpistemicValueResult;
  actions?: EpistemicActionsResult;
  complementarity?: EpistemicComplementarityResult | EpistemicRefusalResult | null;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export interface EpistemicAdaptiveArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  acquisitions: EpistemicAcquisitionArgs[];
  budget: number;
  max_steps: number;
}

export interface EpistemicAdaptiveOutcomeResult extends JsonObject {
  label: string;
  probability: number;
  posterior: number[];
  next: EpistemicAdaptiveNodeResult;
}

export interface EpistemicAdaptiveNodeResult extends JsonObject {
  kind: "stop" | "acquire";
  action_index?: number;
  action?: string;
  risk?: number;
  acquisition_index?: number;
  id?: string;
  cost?: number;
  expected_total?: number;
  expected_terminal_risk?: number;
  expected_acquisition_cost?: number;
  outcomes?: EpistemicAdaptiveOutcomeResult[];
}

export interface EpistemicAdaptivePolicyResult extends JsonObject {
  expected_total: number;
  expected_terminal_risk: number;
  expected_acquisition_cost: number;
  nodes_evaluated: number;
  selected_depth: number;
  root: EpistemicAdaptiveNodeResult;
}

export interface EpistemicAdaptiveResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/epistemic-adaptive-acquisition/0.1";
  budget?: number;
  max_steps?: number;
  problem?: { actions: string[]; models: string[]; action_count: number; model_count: number };
  acquisitions?: Array<{ id: string; cost: number; outcomes: Array<{ label: string }> }>;
  policy?: EpistemicAdaptivePolicyResult;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface AdaptiveExecutionArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  acquisitions: EpistemicAcquisitionArgs[];
  budget: number;
  max_steps: number;
  mode?: "simulate" | "replay";
  provider?: string;
  authorization?: { grant_id: string; provider: string; [key: string]: JsonValue };
  observations?: Array<{ acquisition_id: string; outcome_label: string; [key: string]: JsonValue }>;
  receipt?: JsonObject;
}

export interface AdaptiveExecutionObservation extends JsonObject {
  provider: string;
  acquisition_id: string;
  outcome_label: string;
  evidence_digest: string;
  provenance: "observed" | "simulated" | "replayed";
}

export interface AdaptiveExecutionObservationReceipt extends JsonObject {
  sequence: number;
  request: {
    plan_digest: string;
    sequence: number;
    acquisition_id: string;
    declared_cost: number;
    [key: string]: JsonValue;
  };
  observation: AdaptiveExecutionObservation;
}

export interface AdaptiveExecutionReceipt extends JsonObject {
  schema: "bioprism-epistemic/adaptive-execution/0.1";
  plan_digest: string;
  provider: string;
  status: "completed" | "partial" | "refused";
  authorization: { granted: boolean; grant_id: string | null; provider: string | null; [key: string]: JsonValue };
  observations: AdaptiveExecutionObservationReceipt[];
  actual_acquisition_cost: number;
  terminal_action: number | null;
  terminal_risk: number | null;
  refusal: string | null;
  refusal_detail: string | null;
}

export interface AdaptiveExecutionResult extends JsonObject {
  ok: true;
  schema: "bioprism-epistemic/adaptive-execution/0.1";
  mode: "simulate" | "replay";
  plan_digest: string;
  completed: boolean;
  receipt: AdaptiveExecutionReceipt;
  provenance_counts: { observed: number; simulated: number; replayed: number; [key: string]: JsonValue };
  guarantees: string[];
  limitations: string[];
}

export interface AdaptiveCostVector extends JsonObject {
  tokens: number;
  compute_ms: number;
  latency_ms: number;
  money_usd: number;
  privacy_loss: number;
  specimen_units: number;
  expert_minutes: number;
}

export interface AdaptiveCostedAcquisitionArgs extends JsonObject {
  acquisition: EpistemicAcquisitionArgs;
  cost: AdaptiveCostVector;
}

export interface AdaptiveCostedArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  acquisitions: AdaptiveCostedAcquisitionArgs[];
  budget: AdaptiveCostVector;
  weights: AdaptiveCostVector;
  max_steps: number;
}

export interface AdaptiveCostedResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/epistemic-adaptive-costed/0.1";
  cost_dimensions: string[];
  budget?: AdaptiveCostVector;
  weights?: AdaptiveCostVector;
  max_steps?: number;
  problem?: { actions: string[]; models: string[]; action_count: number; model_count: number };
  acquisitions?: AdaptiveCostedAcquisitionArgs[];
  policy?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export type InterweaveWorkflowId =
  | "reliable_software_repair"
  | "scientific_claim_reproduction"
  | "biomedical_research_data_audit"
  | "incident_response"
  | "evidence_grounded_policy_comparison"
  | "dataset_transformation_molecule";

export interface WorkflowExecutionArgs extends JsonObject {
  workflow: InterweaveWorkflowId;
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  acquisitions: EpistemicAcquisitionArgs[];
  budget: number;
  max_steps: number;
  mode?: "simulate" | "replay";
  provider?: string;
  capabilities?: string[];
  authorization?: { grant_id: string; provider: string; [key: string]: JsonValue };
  observations?: Array<{ acquisition_id: string; outcome_label: string; [key: string]: JsonValue }>;
  receipt?: JsonObject;
  evidence?: WorkflowExecutionEvidenceConfig;
}

export interface WorkflowExecutionEvidenceConfig extends JsonObject {
  subject_id: string;
  domains: string[];
  parent_digests?: string[];
}

export interface WorkflowExecutionResult extends JsonObject {
  ok: true;
  schema: "bioprism-interweave/workflow-execution/0.1";
  mode: "simulate" | "replay";
  workflow: InterweaveWorkflowId;
  plan_digest: string;
  binding_digest: string;
  binding: JsonObject;
  completed: boolean;
  release_posture: string;
  receipt: JsonObject;
  provenance_counts: { observed: number; simulated: number; replayed: number; [key: string]: JsonValue };
  guarantees: string[];
  limitations: string[];
  workflow_execution_evidence?: WorkflowExecutionEvidenceResult;
}

export interface WorkflowExecutionEvidenceArgs extends JsonObject {
  binding: JsonObject;
  receipt: JsonObject;
  subject_id: string;
  domains: string[];
  parent_digests?: string[];
}

export interface WorkflowExecutionEvidenceImportArgs extends JsonObject {
  evidence: JsonObject;
}

export interface WorkflowExecutionEvidenceQueryOptions extends JsonObject {
  workflow_id?: InterweaveWorkflowId;
  subject_id?: string;
  domain?: string;
  plan_digest?: string;
  binding_digest?: string;
  receipt_status?: "completed" | "partial" | "refused";
  provenance_mode?: "none" | "observed_declared" | "simulated" | "replayed" | "mixed";
  after?: string;
  max_items?: number;
  include_records?: boolean;
}

export interface WorkflowExecutionEvidenceResult extends JsonObject {
  ok: true;
  schema: string;
  workflow: string;
  evidence_digest: string;
  evidence?: JsonObject;
  registry?: JsonObject;
  artifact_registry?: JsonObject;
  rows?: JsonObject[];
  next_after?: string | null;
  has_more?: boolean;
  guarantees?: string[];
  does_not_claim?: string[];
}

export interface EpistemicDecisionQuotientArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  permitted_actions: string[];
}

export interface EpistemicDecisionQuotientClass extends JsonObject {
  class_index: number;
  representative_model: string;
  members: string[];
  loss_differences: Record<string, number>;
  preferred_actions: string[];
}

export interface EpistemicDecisionQuotientProjection extends JsonObject {
  schema_version: "bioprism-epistemic-decision-quotient/0.1";
  basis: "permitted_loss_difference_profile";
  permitted_actions: string[];
  original_model_count: number;
  quotient_model_count: number;
  merged_model_count: number;
  model_to_class: Record<string, number>;
  classes: EpistemicDecisionQuotientClass[];
}

export interface EpistemicDecisionQuotientResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/epistemic-decision-quotient/0.1";
  quotient?: EpistemicDecisionQuotientProjection;
  summary?: {
    original_model_count: number;
    quotient_model_count: number;
    merged_model_count: number;
    compressed: boolean;
    compression_fraction: number;
  };
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

/** Paths into a FIBER world/query pair accepted by the progressive compiler. */
export interface FiberCompileArgs extends JsonObject {
  world: string;
  query: string;
  layer?: "l0" | "l1" | "l2" | "l3" | "l4";
}

export interface FiberDecisionQuotientSummary extends JsonObject {
  schema: "bioprism-mcp/epistemic-decision-quotient/0.1";
  basis: "permitted_loss_difference_profile";
  permitted_actions: string[];
  original_model_count: number;
  quotient_model_count: number;
  merged_model_count: number;
  compressed: boolean;
  compression_fraction: number;
  certificate_binding: {
    query_sha256: string;
    certificate_sha256: string;
    [key: string]: JsonValue;
  };
  limitations: string[];
}

export interface FiberRateDistortionSummary extends JsonObject {
  schema: "bioprism-mcp/epistemic-context-audit/0.2";
  criterion: "bayes_regret" | "minimax_regret";
  tolerance: number;
  compatibility_floor: number;
  evidence_count: number;
  full_rate: number;
  identification: JsonObject;
  sufficiency: JsonObject;
  frontier: JsonObject;
  certificate_binding: {
    query_sha256: string;
    certificate_sha256: string;
    [key: string]: JsonValue;
  };
  guarantees: string[];
  limitations: string[];
}

export interface FiberAdaptiveAcquisitionOutcome extends JsonObject {
  label: string;
  probability: number;
  posterior: number[];
  next: FiberAdaptiveAcquisitionNode;
}

export interface FiberAdaptiveAcquisitionNode extends JsonObject {
  kind: "stop" | "acquire";
  action_index?: number;
  action?: string;
  risk?: number;
  acquisition_index?: number;
  id?: string;
  cost?: number;
  expected_total?: number;
  expected_terminal_risk?: number;
  expected_acquisition_cost?: number;
  outcomes?: FiberAdaptiveAcquisitionOutcome[];
}

export interface FiberAdaptiveAcquisitionSummary extends JsonObject {
  schema: "bioprism-mcp/fiber-adaptive-acquisition/0.1";
  budget: number;
  max_steps: number;
  prior: number[];
  problem: {
    actions: string[];
    models: string[];
    action_count: number;
    model_count: number;
    [key: string]: JsonValue;
  };
  acquisitions: Array<{
    id: string;
    cost: number;
    outcomes: Array<{ label: string; likelihood: number[]; [key: string]: JsonValue }>;
    [key: string]: JsonValue;
  }>;
  policy: {
    expected_total: number;
    expected_terminal_risk: number;
    expected_acquisition_cost: number;
    nodes_evaluated: number;
    selected_depth: number;
    root: FiberAdaptiveAcquisitionNode;
    [key: string]: JsonValue;
  };
  certificate_binding: {
    query_sha256: string;
    certificate_sha256: string;
    [key: string]: JsonValue;
  };
  execution: "not_started";
  authorization: "not_granted";
  provenance: JsonObject;
  guarantees: string[];
  limitations: string[];
}

export interface FiberCompileResult extends JsonObject {
  layer?: "l0" | "l1" | "l2" | "l3" | "l4";
  decision_quotient?: FiberDecisionQuotientSummary;
  rate_distortion?: FiberRateDistortionSummary;
  adaptive_acquisition?: FiberAdaptiveAcquisitionSummary;
  certificate_sha256?: string;
}

export interface EpistemicEvidenceItemArgs extends JsonObject {
  id: string;
  cost: number;
  likelihood: number[];
}

export interface EpistemicEvidencePoolArgs extends JsonObject {
  items: EpistemicEvidenceItemArgs[];
}

export interface EpistemicContextAuditArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  evidence_pool: EpistemicEvidencePoolArgs;
  criterion: "bayes_regret" | "minimax_regret";
  tolerance: number;
  compatibility_floor: number;
  subsets?: number[][];
  include_frontier?: boolean;
  max_rows?: number;
}

export interface EpistemicContextAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/epistemic-context-audit/0.1";
  criterion?: "bayes_regret" | "minimax_regret";
  problem?: JsonObject;
  evidence_pool?: JsonObject;
  identification?: JsonObject;
  sufficiency?: JsonObject;
  frontier?: JsonObject | null;
  include_frontier?: boolean;
  subset_rows?: JsonObject[];
  subset_count?: number;
  subset_refusal_count?: number;
  subset_rows_omitted?: number;
  max_rows?: number;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface EpistemicSelectionConstraintArgs extends JsonObject {
  cardinality?: number;
  budget?: number;
  costs?: number[];
}

export interface EpistemicSelectionAuditArgs extends JsonObject {
  problem: EpistemicDecisionProblemArgs;
  belief: EpistemicBeliefArgs;
  evidence_pool: EpistemicEvidencePoolArgs;
  constraint: EpistemicSelectionConstraintArgs;
  protected?: number[];
  check_submodularity?: boolean;
  include_lazy?: boolean;
  compare_optimum?: boolean;
  tolerance?: number;
}

export interface EpistemicSelectionAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/epistemic-selection-audit/0.1";
  objective?: "regret_reduction";
  problem?: JsonObject;
  evidence_pool?: JsonObject;
  constraint?: JsonObject;
  baseline?: JsonObject;
  submodularity?: JsonObject;
  greedy?: JsonObject;
  lazy?: JsonObject | null;
  comparisons?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface BenchmarkTraceEventArgs extends JsonObject {
  step: number;
  kind: "goal" | "observation" | "choice" | "action" | "result" | "claim" | "termination";
  payload: JsonObject;
  caused_by?: number;
  visible?: string[];
}

export interface BenchmarkTraceArgs extends JsonObject {
  trace_id: string;
  events: BenchmarkTraceEventArgs[];
  succeeded: boolean;
}

export interface BenchmarkTraceAnalyzeArgs extends JsonObject {
  failing: BenchmarkTraceArgs;
  reference?: BenchmarkTraceArgs;
}

export interface BenchmarkCandidateScoreResult extends JsonObject {
  alternatives: number;
  newly_visible: number;
  downstream_steps: number;
  is_divergence: boolean;
  total: number;
}

export interface BenchmarkCausalCandidateResult extends JsonObject {
  step: number;
  kind: string;
  summary: string;
  score: BenchmarkCausalScoreResult;
  upstream_unresolved?: number;
}

export interface BenchmarkCausalScoreResult extends JsonObject {
  necessity: number;
  counterfactual_effect: number;
  irreversibility: number;
  explanatory_simplicity: number;
  total: number;
  irreversibility_declared: boolean;
}

export interface BenchmarkDivergenceResult extends JsonObject {
  kind: "identical" | "early_termination" | "diverged";
  at_step?: number;
  shorter?: string;
  longer_continued_for?: number;
  failing_step?: number;
  passing_step?: number;
  common_prefix?: number;
  failing_did?: string;
  passing_did?: string;
  visibility_gap?: string[];
}

export interface BenchmarkCausalVerdictResult extends JsonObject {
  verdict: "first_causal" | "conjunction" | "environment_divergence" | "no_divergence" | "unlocalizable";
  step?: number;
  score?: number;
  steps?: number[];
  at_step?: number;
  kind?: string;
  nearest_controlled_ancestor?: number;
  reason?: string;
}

export interface BenchmarkCausalAnalysisResult extends JsonObject {
  trace_id: string;
  textual: BenchmarkDivergenceResult;
  textual_is_actionable: boolean;
  reference?: string;
  terminal_step: number;
  ancestry: number[];
  candidates: BenchmarkCausalCandidateResult[];
  verdict: BenchmarkCausalVerdictResult;
}

export interface BenchmarkReversibilityResult extends JsonObject {
  source: "declared" | "assumed";
  irreversible: boolean;
  basis?: string;
}

export interface BenchmarkBoundaryResult extends JsonObject {
  step: number;
  summary: string;
  decision_type: string;
  type_evidence: string;
  reversibility: BenchmarkReversibilityResult;
  rank: BenchmarkCandidateScoreResult;
  no_op_reason?: string;
}

export interface BenchmarkEpisodeResult extends JsonObject {
  index: number;
  goal_step?: number;
  label: string;
  steps: number[];
}

export interface BenchmarkRepetitionResult extends JsonObject {
  summary: string;
  steps: number[];
  classification: {
    kind: "iterative_refinement" | "stuck";
    evidence_gained?: string[];
    repeats?: number;
  };
}

export interface BenchmarkTraceSummaryResult extends JsonObject {
  episode_count: number;
  boundary_count: number;
  extractable_boundaries: number;
  repetition_groups: number;
}

export interface BenchmarkTraceAnalysisResult extends JsonObject {
  ok: boolean;
  trace_id?: string;
  succeeded?: boolean;
  event_count?: number;
  reference_trace_id?: string;
  analysis?: BenchmarkCausalAnalysisResult;
  episodes?: BenchmarkEpisodeResult[];
  boundaries?: BenchmarkBoundaryResult[];
  repetitions?: BenchmarkRepetitionResult[];
  summary?: BenchmarkTraceSummaryResult;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export interface BenchmarkDecisionAuditArgs extends JsonObject {
  trace: BenchmarkTraceArgs;
  reference?: BenchmarkTraceArgs;
  decision_step?: number;
  actions?: JsonObject[];
  constraints?: JsonObject[];
  claims?: JsonObject[];
  evaluator_dispute?: string;
  max_items?: number;
}

export interface BenchmarkDecisionCoverageResult extends JsonObject {
  total: number;
  visible_at_decision_time: number;
  validation_only: number;
  feasible: number;
  strong: number;
  plausible_wrong_alternatives: number;
  adequate: boolean;
}

export interface BenchmarkFailureCardResult extends JsonObject {
  trace_id: string;
  terminal_step: number;
  blame: JsonObject;
  recommended_cell_steps: number[];
  findings: JsonObject[];
  hypotheses: JsonObject[];
  violated_constraints: JsonObject[];
  alternative_explanations: string[];
  missing_evidence: string[];
  evidence_ratio: number;
}

export interface BenchmarkDecisionAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/benchmark-decision-audit/0.1";
  trace_id?: string;
  trace_digest?: string;
  reference_trace_id?: string;
  reference_digest?: string;
  analysis?: BenchmarkCausalAnalysisResult;
  analysis_omitted?: { ancestry: number; candidates: number };
  decision?: {
    selected_step: number;
    causal_step?: number;
    causal_alignment: "aligned" | "explicit_override";
    event_kind?: string;
    coverage: BenchmarkDecisionCoverageResult;
    action_counts: { all: number; visible_to_agent: number; validation_only: number; acceptable: number };
    actions: JsonObject[];
    visible_to_agent: JsonObject[];
    validation_only: JsonObject[];
    acceptable: JsonObject[];
    omitted: { all: number; visible_to_agent: number; validation_only: number; acceptable: number };
  };
  failure_card?: BenchmarkFailureCardResult;
  failure_card_omitted?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export interface BenchmarkIntegrityAuditArgs extends JsonObject {
  instances: JsonObject[];
  panel_runs?: JsonObject[];
  bench_instances?: JsonObject[];
  known_instances?: string[];
  safety_vetoes?: string[];
  exposure?: JsonObject;
  probes?: JsonObject;
  private_share?: number;
  rotating_panels?: number;
  max_items?: number;
}

export interface BenchmarkIntegrityAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/benchmark-integrity-audit/0.1";
  instance_digest?: string;
  counts?: { instances: number; panel_runs: number; bench_instances: number; known_instances: number; safety_vetoes: number };
  dedup?: JsonObject;
  holdout?: { private_share: number; rotating_panels: number; counts: JsonObject; rows: JsonObject[]; omitted: number };
  contamination?: { counts: JsonObject; admissible: number; inadmissible: number; rows: JsonObject[]; omitted: number };
  calibration?: JsonObject;
  effective_diversity?: { instances: number; parents: number; families: number; signatures: number; equivalence_classes: number; inflation_ratio: number; caveat: string };
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export interface BenchmarkCounterfactualCheckArgs extends JsonObject {
  source: JsonObject;
  followup: JsonObject;
  intervention: JsonObject;
  expected: JsonObject;
  source_verdict: string;
  followup_verdict: string;
}

export interface BenchmarkCounterfactualCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/benchmark-counterfactual/0.1";
  pair?: JsonObject;
  outcome?: { outcome: "as_predicted" | "spurious_sensitivity" | "missed_the_change" | "wrong_direction"; moved_to?: string; stayed_at?: string };
  satisfied?: boolean;
  source_verdict?: string;
  followup_verdict?: string;
  cell_digests?: { source: string; followup: string };
  allowed_cell_fields?: string[];
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface BenchmarkOracleReviewArgs extends JsonObject {
  proposal: JsonObject;
  reviewer: string;
  grade?: JsonObject;
  cell?: JsonObject;
}

export interface BenchmarkOracleReviewResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/benchmark-oracle-review/0.1";
  proposal?: JsonObject;
  reviewed_oracle?: JsonObject;
  reviewer?: string;
  review_digest?: string;
  strength?: "exact_state_predicate" | "execution_test" | "property_relation" | "trajectory_constraint" | "statistical_tolerance" | "model_judge";
  deterministic?: boolean;
  grade?: JsonObject;
  cell?: JsonObject;
  synthesis_order?: string[];
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface BenchmarkCompileArgs extends JsonObject {
  trace: JsonObject;
  reference?: JsonObject;
  context?: JsonObject[];
  probe_observations?: JsonObject[];
  budget?: JsonObject;
  ledger?: JsonObject[];
  claims?: JsonObject[];
}

export interface BenchmarkCompileResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/benchmark-compile/0.1";
  trace_id?: string;
  trace_digest?: string;
  reference_digest?: string | null;
  compilation?: JsonObject;
  class?: JsonObject;
  cell_step?: number | null;
  episodes?: number;
  boundary_count?: number;
  oracle?: JsonObject | null;
  minimization?: JsonObject | null;
  confidence?: JsonObject;
  limiting_stage?: JsonValue;
  unmeasured_stages?: string[];
  probe?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface BenchmarkCompileReviewArgs extends JsonObject {
  trace: JsonObject;
  reference?: JsonObject;
  context?: JsonObject[];
  probe_observations?: JsonObject[];
  budget?: JsonObject;
  ledger?: JsonObject[];
  claims?: JsonObject[];
  reviewer: string;
  world: JsonObject;
  query: JsonObject;
  grade?: JsonObject;
}

export interface BenchmarkCompileReviewResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/benchmark-compile-review/0.1";
  compile?: JsonObject;
  reviewed_oracle?: JsonObject;
  reviewer?: string;
  review_digest?: string;
  grade?: JsonObject;
  cell?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface PackCoverageAuditArgs extends JsonObject {
  section?: "all" | "15" | "29";
  pack_ids?: string[];
  max_items?: number;
}

export interface PackCoverageAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/pack-coverage-audit/0.1";
  section?: "all" | "15" | "29";
  selected_pack_count?: number;
  selected_pack_ids?: string[];
  summary?: { families: number; covered: number; uncovered: number; singly_covered: number; weakly_covered: number; coverage_fraction: number; gap_summary: string };
  rows?: JsonObject[];
  rows_omitted?: number;
  uncovered?: string[];
  uncovered_omitted?: number;
  singly_covered?: string[];
  singly_covered_omitted?: number;
  weakly_covered?: string[];
  weakly_covered_omitted?: number;
  matrix?: JsonObject[];
  matrix_omitted?: number;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface PackReleaseAuditArgs extends JsonObject {
  section?: "all" | "15" | "29";
  pack_ids?: string[];
  max_items?: number;
}

export interface PackReleaseAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/pack-release-audit/0.1";
  section?: "all" | "15" | "29";
  selected_pack_count?: number;
  selected_pack_ids?: string[];
  sequenced_count?: number;
  unsequenced_count?: number;
  release_coverage_fraction?: number;
  wave_counts?: Record<string, number>;
  axis_counts?: Record<string, number>;
  release_order?: JsonObject[];
  release_order_omitted?: number;
  unsequenced?: JsonObject[];
  unsequenced_omitted?: number;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface FoundationContractCheckArgs extends JsonObject {
  contract: JsonObject;
  parent?: JsonObject;
  envelope?: JsonObject;
  present_as_established?: boolean;
  world?: JsonObject;
  claim?: "associational" | "analysis_fork" | "injected_factor_effect" | "simulated_intervention" | "reveal_prediction" | "specified_ground_truth" | "real_treatment_effect";
  transition?: JsonObject;
}

export interface FoundationContractGateResult extends JsonObject {
  ok: boolean;
  id?: string;
  intent?: string;
  falsifier_count?: number;
  action_count?: number;
  evidence_obligation_count?: number;
  minimum_reviewers?: number;
  uncertainty_required?: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface FoundationParentRelationResult extends JsonObject {
  ok: boolean;
  relation: "refines" | "refused";
  refusal?: string;
  fail_closed?: boolean;
}

export interface FoundationEnvelopeResult extends JsonObject {
  ok: boolean;
  structure: string;
  maturity: string;
  maturity_rung: string;
  fail_closed: boolean;
}

export interface FoundationWorldResult extends JsonObject {
  ok: boolean;
  world_id: string;
  class: string;
  counterfactual_strength: string;
  reveal_policy: string;
  claim?: string;
  fail_closed: boolean;
}

export interface FoundationTransitionResult extends JsonObject {
  ok: boolean;
  verdict: "plane_consistent" | "plane_confusion";
  refusal?: string;
  fail_closed?: boolean;
}

export interface FoundationContractCheckResult extends JsonObject {
  ok: boolean;
  verdict: "admitted" | "refused";
  contract: FoundationContractGateResult;
  parent_relation?: FoundationParentRelationResult | null;
  envelope?: FoundationEnvelopeResult | null;
  world?: FoundationWorldResult | null;
  transition?: FoundationTransitionResult | null;
  guarantees: string[];
}

export interface PackCatalogueArgs extends JsonObject {
  section?: "all" | "15" | "29";
  max_items?: number;
}

export interface PackCatalogueEntryResult extends JsonObject {
  id: string;
  title: string;
  blueprint_module: string;
  axis: "mechanism" | "domain" | "platform";
  measures: string;
  capabilities: string[];
  domains: string[];
  decision_families: string[];
  oracles: string[];
  strongest_oracle?: "deterministic" | "executable" | "policy_veto" | "statistical" | "expert_review" | "rubric";
  has_execution_grounded_oracle: boolean;
  release_wave?: { wave: number } | "unsequenced";
  capability_signature: string;
}

export interface PackDuplicateSignatureResult extends JsonObject {
  signature: string;
  pack_ids: string[];
}

export interface PackCatalogueResult extends JsonObject {
  ok: boolean;
  section: "all" | "15" | "29";
  portfolio_count: number;
  section_counts: { "15": number; "29": number };
  returned: PackCatalogueEntryResult[];
  omitted: number;
  duplicate_signature_groups: PackDuplicateSignatureResult[];
  guarantees: string[];
}

export type PackHealthVerdict = "healthy" | "degraded" | "unreportable";
export type PackDiscriminationVerdict = "undetermined" | "saturated" | "floored" | "discriminating";
export type PackHealthFindingKind = "saturated" | "floored" | "not_yet_characterised" | "degenerate" | "contaminated" | "no_grounded_oracle" | "counts_not_materialized";
export type PackContaminationSignalKind = "public_answer_key" | "corpus_membership" | "released_before_cutoff" | "memorization_gap";
export type PackOracleTier = "deterministic" | "executable" | "policy_veto" | "statistical" | "expert_review" | "rubric";

export interface PackHealthAssessArgs extends JsonObject {
  pack: JsonObject;
  observations: JsonObject;
  policy?: JsonObject;
}

export interface PackSystemObservationResult extends JsonObject {
  system: string;
  trials: number;
  passes: number;
}

export interface PackCalibrationResult extends JsonObject {
  observations: PackSystemObservationResult[];
}

export interface PackDiscriminationResult extends JsonObject {
  verdict: PackDiscriminationVerdict;
  reason?: string;
  pooled_pass_rate?: number;
  systems?: number;
  lowest?: number;
  highest?: number;
  separated?: boolean;
}

export interface PackContaminationSignalResult extends JsonObject {
  signal: PackContaminationSignalKind;
  location?: string;
  corpus?: string;
  matched_instances?: number;
  pack_release?: string;
  model_cutoff?: string;
  public?: PackSystemObservationResult;
  held_out?: PackSystemObservationResult;
}

export interface PackHealthFindingResult extends JsonObject {
  finding: PackHealthFindingKind;
  pooled_pass_rate?: number;
  systems?: number;
  reason?: string;
  baseline?: string;
  baseline_pass_rate?: number;
  best_system_pass_rate?: number;
  signal?: PackContaminationSignalResult;
  tiers?: PackOracleTier[];
  declared?: number;
  validated?: number;
  materialized_fraction?: number;
}

export interface PackHealthResult extends JsonObject {
  pack: string;
  pack_digest: string;
  findings: PackHealthFindingResult[];
}

export interface PackScoreResult extends JsonObject {
  pack: string;
  pack_digest: string;
  pooled_pass_rate: number;
  discrimination: PackDiscriminationResult;
  advisories: PackHealthFindingResult[];
}

export interface PackScoreGateResult extends JsonObject {
  reportable: boolean;
  score?: PackScoreResult | null;
  refusal?: string;
  fail_closed?: boolean;
}

export interface PackHealthAssessmentResult extends JsonObject {
  ok: boolean;
  pack?: string;
  pack_digest?: string;
  verdict?: PackHealthVerdict;
  finding_count?: number;
  blocking_findings?: number;
  advisory_findings?: number;
  health?: PackHealthResult;
  calibration?: PackCalibrationResult;
  score_gate?: PackScoreGateResult;
  stage?: "pack_validation" | "pack_health_assessment";
  score?: null;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export type RedteamVulnerabilityClass = "code_vulnerability" | "sandbox_bypass" | "evaluator_bypass" | "privacy_leakage" | "benchmark_exploit" | "hidden_test_exposure" | "provenance_flaw" | "malicious_artifact" | "dependency_compromise" | "misleading_security_claim";
export type RedteamFindingStatus = "reported" | "reproduced" | "confirmed" | "not_reproduced" | "duplicate";
export type RedteamSeverity = "low" | "medium" | "high" | "critical";
export type RedteamDisclosureStage = "reported" | "triaged" | "fixed" | "disclosed" | "withdrawn" | "duplicate";
export type RedteamTrustZone = "user_client" | "public_api" | "control_plane" | "catalog" | "artifact_service" | "build_service" | "agent_sandbox" | "evaluator_sandbox" | "trusted_review" | "private_worker" | "model_provider" | "public_registry_mirror";
export type RedteamChannel = "sealed_output_bundle" | "typed_claim" | "read_only_input" | "hidden_oracle_mount" | "artifact_fetch" | "control_plane_api" | "provider_api" | "human_review" | "publication";
export type RedteamArtifactKind = "agent_output" | "hidden_oracle_asset" | "grader_claim" | "pack_manifest" | "credential" | "trace" | "published_result";
export type RedteamIncidentClass = "confidentiality_leak" | "unauthorized_effect" | "sandbox_escape" | "cross_tenant_exposure" | "malicious_pack" | "compromised_key" | "result_integrity_failure" | "benchmark_exploit" | "hidden_holdout_leak" | "evaluator_tampering" | "artifact_substitution" | "dependency_vulnerability" | "privacy_breach" | "service_compromise" | "widespread_result_invalidity";
export type RedteamContainmentAction = "stop_execution_pool" | "revoke_leases" | "revoke_credentials" | "quarantine_artifacts" | "freeze_publication" | "preserve_logs" | "rotate_keys" | "notify_federation_peers";
export type RedteamAuditEvent = "authentication" | "privilege_change" | "policy_change" | "hidden_oracle_access" | "sensitive_artifact_access" | "publication" | "result_acceptance" | "reviewer_decision" | "key_lifecycle" | "security_quarantine" | "deletion" | "federation_import";

export interface SecurityRedteamSimulateArgs extends JsonObject {
  findings?: JsonObject[];
  vulnerabilities?: JsonObject[];
  deliveries?: JsonObject[];
  incidents?: JsonObject[];
  audit_records?: JsonObject[];
  attestations?: JsonObject[];
  boundary_universe?: string[];
  include_details?: boolean;
  max_items?: number;
}

export interface RegressionGateResult extends JsonObject {
  eligible: boolean;
  cell?: JsonObject;
  public_summary?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamFindingResult extends JsonObject {
  index: number;
  ok: boolean;
  finding?: JsonObject & { id: string; campaign: string; boundary: string; class: RedteamVulnerabilityClass; status: RedteamFindingStatus; reproduction?: string };
  regression_gate?: RegressionGateResult;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RegressionCorpusResult extends JsonObject {
  sentinel_count: number;
  covered_boundaries: string[];
  unminimised_count: number;
  uncovered_boundaries: string[];
  cells: JsonObject[];
  omitted_cells: number;
}

export interface VulnerabilityTransitionResult extends JsonObject {
  index: number;
  ok: boolean;
  to?: RedteamDisclosureStage;
  epoch?: number;
  stage_after?: RedteamDisclosureStage;
  refusal?: string;
  fail_closed?: boolean;
}

export interface VulnerabilityResult extends JsonObject {
  index: number;
  ok: boolean;
  vulnerability?: JsonObject & { id: string; class: RedteamVulnerabilityClass; severity: RedteamSeverity; stage: RedteamDisclosureStage; entered_at: number; embargoed: boolean; history: JsonObject[] };
  transitions?: VulnerabilityTransitionResult[];
  transition_count?: number;
  stopped_after_refusal?: boolean;
  advisory_present?: boolean;
  advisory_missing_fields?: string[];
  independent_verification_required?: boolean;
  disclosed?: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamDeliveryResult extends JsonObject {
  index: number;
  ok: boolean;
  crossing?: JsonObject;
  honest_label?: string;
  scope?: "within_trial" | "across_trials" | null;
  requested?: JsonObject;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamBoundaryResult extends JsonObject {
  model: string;
  within_trial_agent_to_evaluator: JsonValue[];
  within_trial_evaluator_to_agent: JsonValue[];
  all_scope_agent_to_evaluator: JsonValue[];
  feedback_loops: JsonValue[];
  delivery_rows: RedteamDeliveryResult[];
  delivery_rows_omitted: number;
  allowed_delivery_count: number;
  refused_delivery_count: number;
}

export interface ContainmentRequestResult extends JsonObject {
  index: number;
  ok: boolean;
  request?: JsonObject & { action: RedteamContainmentAction; requested_at: number; requested_by: string };
  honest_label?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamTimelineResult extends JsonObject {
  index: number;
  ok: boolean;
  epoch?: number;
  refusal?: string;
  fail_closed?: boolean;
}

export interface ContainmentClaimResult extends JsonObject {
  allowed: boolean;
  report?: JsonObject;
  caveat?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamIncidentResult extends JsonObject {
  index: number;
  ok: boolean;
  incident?: JsonObject & { id: string; class: RedteamIncidentClass; opened_at: number };
  requests?: ContainmentRequestResult[];
  timeline?: RedteamTimelineResult[];
  containment_claim?: ContainmentClaimResult;
  unrequested_actions?: RedteamContainmentAction[];
  result_tainting_class?: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamAuditRowResult extends JsonObject {
  index: number;
  ok: boolean;
  linked?: JsonObject;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RedteamAuditResult extends JsonObject {
  rows: RedteamAuditRowResult[];
  rows_omitted: number;
  chain_length: number;
  head?: string | null;
  verified: boolean;
  verification_refusal?: string | null;
  assertion_count: number;
  public_view_count: number;
  records: JsonObject[];
}

export interface RedteamAttestationResult extends JsonObject {
  index: number;
  ok: boolean;
  observed?: boolean;
  attestation?: JsonObject;
  refusal?: string;
  fail_closed?: boolean;
}

export interface SecurityRedteamResult extends JsonObject {
  ok: boolean;
  workflow?: "section_13_redteam_incident_evidence";
  input_counts?: Record<string, number>;
  findings?: RedteamFindingResult[];
  findings_omitted?: number;
  regression_corpus?: RegressionCorpusResult;
  vulnerabilities?: VulnerabilityResult[];
  vulnerabilities_omitted?: number;
  boundary?: RedteamBoundaryResult;
  incidents?: RedteamIncidentResult[];
  incidents_omitted?: number;
  audit?: RedteamAuditResult;
  attestations?: RedteamAttestationResult[];
  attestations_omitted?: number;
  guarantees: string[];
  limitations?: string[];
  refusal?: string;
  fail_closed?: boolean;
}

export interface WorldGenerateArgs extends JsonObject {
  spec: JsonObject;
  include_world?: boolean;
  include_query?: boolean;
}

export interface WorldDiagnosticResult extends JsonObject {
  severity: "warning" | "error";
  code: string;
  subject: string;
  message: string;
}

export interface WorldValidationResult extends JsonObject {
  errors: number;
  warnings: number;
  diagnostics: WorldDiagnosticResult[];
}

export interface WorldGenerationCountsResult extends JsonObject {
  facts: number;
  factors: number;
  events: number;
  subjects: number;
  distractors: number;
  relay_depth: number;
  generated_query_targets: number;
}

export interface WorldGenerateResult extends JsonObject {
  ok: boolean;
  world_id?: string;
  query_id?: string;
  world_digest?: string;
  query_digest?: string;
  counts?: WorldGenerationCountsResult;
  validation?: WorldValidationResult;
  world?: JsonObject | null;
  query?: JsonObject | null;
  guarantees: string[];
  stage?: "generated_world_parse" | "generated_query_parse" | "generated_world_validation";
  refusal?: string;
  fail_closed?: boolean;
  diagnostics?: WorldDiagnosticResult[];
}

export type FactoryActionKind = "enqueue" | "lease" | "heartbeat" | "stage" | "commit" | "fail" | "recover_expired" | "compensate" | "release_quarantine" | "cancel";
export type FactoryResourceClass = "compile" | "ingest" | "sandbox" | "evaluate" | "mutate" | "index";
export type FactoryIdempotencyClass = "idempotent" | "non_idempotent" | "compensable";
export type FactoryJobState = "queued" | "leased" | "staged" | "succeeded" | "failed" | "quarantined" | "dead_lettered" | "cancelled";
export type FactoryRecoveryOutcome = "requeued" | "quarantined" | "awaiting_compensation" | "dead_lettered";

export interface FactoryLifecycleSimulateArgs extends JsonObject {
  jobs: JsonObject[];
  workers: JsonObject[];
  actions: (JsonObject & { kind?: string })[];
}

export interface FactoryRecoveryResult extends JsonObject {
  outcome: FactoryRecoveryOutcome;
  job_id: string;
  attempt?: number;
  attempts?: number;
  reason?: string;
}

export interface FactoryLeaseResult extends JsonObject {
  job_id: string;
  worker_id: string;
  attempt: number;
  granted_at: JsonValue;
  expires_at: JsonValue;
  last_heartbeat: JsonValue;
}

export interface FactoryJobSnapshotResult extends JsonObject {
  id: string;
  job?: JsonObject & {
    id: string;
    resource_class: FactoryResourceClass;
    idempotency: FactoryIdempotencyClass;
    state: FactoryJobState;
    attempts: number;
  };
  committed_result?: JsonValue | null;
}

export interface FactoryActionTraceResult extends JsonObject {
  index: number;
  kind: string;
  ok: boolean;
  result?: JsonValue;
  refusal?: string;
  fail_closed?: boolean;
}

export interface FactoryLifecycleResult extends JsonObject {
  ok: boolean;
  action_count: number;
  action_failures: number;
  trace: FactoryActionTraceResult[];
  jobs: FactoryJobSnapshotResult[];
  quarantined: (JsonObject & { id: string; state: "quarantined" })[];
  dead_lettered: (JsonObject & { id: string; state: "dead_lettered" })[];
  counts_by_class: Record<FactoryResourceClass, number>;
  guarantees: string[];
}

export type StorageTier = "Hot" | "Warm" | "Cold";
export type StorageClass = "Objects" | "Events" | "Indexes" | "Results" | "Cache";
export type StoragePurpose = "Ingest" | "EvidenceFinalization" | "Cleanup";

export interface StorageLifecycleSimulateArgs extends JsonObject {
  now: number;
  tiering_policy: JsonObject;
  records: JsonObject[];
  apply_tiering?: boolean;
  quota: JsonObject;
  charges?: JsonObject[];
  releases?: JsonObject[];
  delegations?: JsonObject[];
  absorb_delegated?: JsonValue[];
  max_items?: number;
}

export interface StorageTieringPolicyResult extends JsonObject {
  demote_to_warm_after: number;
  demote_to_cold_after: number;
  promote_after_accesses: number;
  promote_within: number;
}

export interface StorageAccessRecordResult extends JsonObject {
  object: string;
  tier: StorageTier;
  last_access: number;
  recent_accesses: number;
  bytes: number;
  pinned: boolean;
}

export interface StorageTierTransitionResult extends JsonObject {
  object: string;
  from: StorageTier;
  to: StorageTier;
  reason: JsonObject;
  skipped_a_tier: boolean;
}

export interface StorageLifecycleRowResult extends JsonObject {
  index: number;
  ok: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface StorageTieringResult extends JsonObject {
  policy: StorageTieringPolicyResult;
  plan: { now: number; transitions: StorageTierTransitionResult[] } & JsonObject;
  transition_count: number;
  bytes_by_target: JsonObject[];
  apply_requested: boolean;
  apply_report?: { applied: number; absent: number } | null;
  records: StorageAccessRecordResult[];
  omitted_records: number;
  input_rows: StorageLifecycleRowResult[];
  omitted_input_rows: number;
}

export interface StorageClassResult extends JsonObject {
  class: StorageClass;
  name: "objects" | "events" | "indexes" | "results" | "cache";
  reconstructible: boolean;
  charged: number;
}

export interface StorageQuotaResult extends JsonObject {
  limit: number;
  reserve: number;
  used: number;
  remaining: number;
  remaining_for_ingest: number;
  remaining_for_evidence_finalization: number;
  remaining_for_cleanup: number;
  classes: StorageClassResult[];
  charges: StorageLifecycleRowResult[];
  omitted_charges: number;
  releases: StorageLifecycleRowResult[];
  omitted_releases: number;
  delegations: StorageLifecycleRowResult[];
  omitted_delegations: number;
  absorptions: StorageLifecycleRowResult[];
  omitted_absorptions: number;
  remaining_children: JsonObject[];
  omitted_children: number;
}

export interface StorageLifecycleResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/storage-lifecycle/0.1";
  max_items: number;
  now: number;
  tiering: StorageTieringResult;
  quota: StorageQuotaResult;
  guarantees: string[];
  limitations: string[];
}

export type RegistryTrustTier = "unranked" | "exploratory" | "validated" | "trusted";
export type RegistryOperation = "publish" | "promote" | "reassess" | "supersede" | "withdraw" | "resolve" | "history" | "inspect" | "revisions" | "verify_all";

export interface RegistryLifecycleSimulateArgs extends JsonObject {
  packs?: JsonValue[];
  index?: JsonObject;
  policy?: JsonObject;
  actions?: JsonValue[];
  include_index?: boolean;
}

export interface RegistryPackPreflightResult extends JsonObject {
  index: number;
  valid: boolean;
  name?: string;
  artifact_digest?: string | null;
  core_digest?: string | null;
  publisher?: string;
  instance_count?: number;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RegistryBrokenArtifactResult extends JsonObject {
  digest: string;
  attestation: string;
}

export interface RegistryIntegrityResult extends JsonObject {
  artifact_count: number;
  log_count: number;
  broken_count: number;
  broken: RegistryBrokenArtifactResult[];
  operations_allowed?: boolean;
}

export interface RegistryActionResult extends JsonObject {
  index: number;
  op?: RegistryOperation;
  ok: boolean;
  result?: JsonValue;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RegistryFinalResult extends JsonObject {
  artifact_count: number;
  log_count: number;
  broken_count: number;
  integrity_clean: boolean;
  verification: RegistryBrokenArtifactResult[];
  log: JsonObject[];
}

export interface RegistryLifecycleResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/registry-lifecycle/0.1";
  policy: JsonObject;
  packs: RegistryPackPreflightResult[];
  initial_integrity: RegistryIntegrityResult;
  actions: RegistryActionResult[];
  final: RegistryFinalResult;
  registry?: JsonObject | null;
  guarantees: string[];
  limitations: string[];
}

export type CacheReuseRule = "SameBuildOnly" | "AcrossBuilds";
export type CacheMissName = "no-entry" | "schema-changed" | "cross-build" | "unproven";

export interface CacheInvalidationSimulateArgs extends JsonObject {
  schema: JsonObject;
  entries?: JsonValue[];
  graph?: JsonObject;
  changed?: string;
  lookups?: JsonValue[];
  apply?: boolean;
  apply_at?: number;
  reprove?: JsonValue[];
  max_items?: number;
}

export interface CacheKeySchemaResult extends JsonObject {
  name: string;
  components: string[];
  reuse: CacheReuseRule;
}

export interface CacheEntryRowResult extends JsonObject {
  index: number;
  ok: boolean;
  digest?: string;
  dependencies?: JsonValue;
  refusal?: string;
  fail_closed?: boolean;
}

export interface CacheEntriesResult extends JsonObject {
  accepted: number;
  submitted: number;
  rows: CacheEntryRowResult[];
  omitted_rows: number;
}

export interface CacheGraphResult extends JsonObject {
  known_resources: string[];
  known_resource_count: number;
  opaque_resources: string[];
  cycle?: string[] | null;
  cycle_is_a_scheduler_defect_not_an_invalidation_hang: boolean;
}

export interface CacheUnknownRegionResult extends JsonObject {
  opaque_resources: string[];
  unknown_resources: string[];
  entries_without_declared_dependencies: string[];
  entries_depending_on_opaque_resources: string[];
}

export type CacheCompletenessResult = "Complete" | { Partial: CacheUnknownRegionResult };

export interface CacheInvalidationPlanResult extends JsonObject {
  changed: string;
  affected_resources: string[];
  invalid_entries: string[];
  proved_unaffected: string[];
  completeness: CacheCompletenessResult;
  population: number;
}

export interface CacheApplyResult extends JsonObject {
  removed: string[];
  marked_unproven: string[];
  left_proven: string[];
  invalidation_was_complete: boolean;
}

export interface CacheLookupResult extends JsonObject {
  index: number;
  ok: boolean;
  hit?: boolean;
  value?: JsonValue;
  proof?: JsonObject;
  miss_reason?: JsonObject;
  refusal?: string;
  fail_closed?: boolean;
}

export interface CacheReproveResult extends JsonObject {
  index: number;
  ok: boolean;
  digest?: string;
  reproved_by?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export interface CacheSnapshotResult extends JsonObject {
  entry_count: number;
  unproven: string[];
  hits: number;
  misses_by_reason: { reason: CacheMissName; count: number }[];
  hit_rate: number;
  entries: JsonObject[];
  omitted_entries: number;
}

export interface CacheInvalidationResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/cache-invalidation/0.1";
  max_items: number;
  key_schema: CacheKeySchemaResult;
  entries: CacheEntriesResult;
  graph: CacheGraphResult;
  invalidation: {
    changed?: string | null;
    plan?: CacheInvalidationPlanResult | null;
    apply_requested: boolean;
    apply_report?: CacheApplyResult | null;
  };
  lookups: { pre_apply?: CacheLookupResult[] | null; post_apply: CacheLookupResult[]; omitted_post_apply: number };
  reprove: CacheReproveResult[];
  cache: CacheSnapshotResult;
  guarantees: string[];
  limitations: string[];
}

export type HubDisclosureActionKind = "declare_held_out" | "disclose" | "contaminate" | "split_integrity" | "headline_eligibility";
export type HubDisclosureState = "unknown" | "held_out" | "disclosed" | "contaminated";
export type HubHeadlineLabel = "held_out" | "computed_before_disclosure" | "disclosed_pack";
export type HubContaminationKind = "instances_published" | "solutions_published" | "training_corpus_overlap" | "submitter_authored_pack" | "grader_leak" | "split_integrity_failure";

export interface HubDisclosureReviewArgs extends JsonObject {
  ledger?: JsonObject;
  actions: (JsonObject & { kind?: HubDisclosureActionKind | string; pack?: string })[];
}

export interface HubContaminationWitnessResult extends JsonObject {
  kind: HubContaminationKind;
  detail: string;
  observed_at: number;
  reported_by: string;
}

export interface HubDisclosureStateResult extends JsonObject {
  disclosure: HubDisclosureState;
  since?: number;
  witness?: HubContaminationWitnessResult;
}

export interface HubHeadlineLabelResult extends JsonObject {
  label: HubHeadlineLabel;
  disclosed_at?: number;
  caveat: string;
}

export interface HubDisclosureActionResult extends JsonObject {
  index: number;
  kind: HubDisclosureActionKind | string;
  ok: boolean;
  result?: JsonValue;
  refusal?: string;
  fail_closed?: boolean;
}

export interface HubDisclosureEntryResult extends JsonObject {
  pack: string;
  state: HubDisclosureStateResult;
}

export interface HubDisclosureLedgerResult extends JsonObject {
  packs: Record<string, HubDisclosureStateResult>;
}

export interface HubDisclosureReviewResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/hub-disclosure/0.1";
  action_count: number;
  action_failures: number;
  trace: HubDisclosureActionResult[];
  entries: HubDisclosureEntryResult[];
  ledger: HubDisclosureLedgerResult;
  guarantees: string[];
}

export type HubCardPublicationState = "available" | "unavailable" | "controlled" | "stale" | "under-review" | "disputed" | "withdrawn" | "non-reproducible" | "not-comparable";
export type HubCardScoreDisplay = "published" | "withheld";

export interface HubCardRenderArgs extends JsonObject {
  moderation: JsonObject;
  submission: string;
  version?: string;
  score?: JsonObject;
  pack?: string;
  computed_at?: number;
  acknowledges_disclosure?: boolean;
  disclosure?: JsonObject;
  not_comparable?: JsonObject;
}

export interface HubCardLabelResult extends JsonObject {
  label: HubHeadlineLabel;
  disclosed_at?: number;
}

export interface HubCardScoreResult extends JsonObject {
  display: HubCardScoreDisplay;
  score?: JsonObject;
  label?: HubCardLabelResult;
  state?: HubCardPublicationState;
  why?: string;
}

export interface HubCardObjectResult extends JsonObject {
  resource_type: string;
  resource_id: string;
  version: string;
  submission: string;
  scope: JsonValue;
  provenance: string[];
  access: string;
  state: HubCardPublicationState;
  verification: string;
  score: HubCardScoreResult;
  non_claims: JsonValue[];
  attributions: JsonValue[];
  limitations: string;
}

export interface HubCardScoreAttachmentResult extends JsonObject {
  attached: boolean;
  pack?: string;
  computed_at?: number;
}

export interface HubCardRenderResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/hub-card/0.1";
  card: HubCardObjectResult;
  score: HubCardScoreAttachmentResult | null;
  moderation_state?: string;
  verification?: string;
  stage?: "card_disclosure_gate" | "card_publication_gate";
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export type HubUnrankableReasonKind = "not_comparable" | "not_published" | "below_verification_floor" | "ineligible";

export interface HubLeaderboardRenderArgs extends JsonObject {
  board: JsonObject;
  entries: JsonValue[];
  moderation: JsonObject;
  disclosure: JsonObject;
  include_details?: boolean;
}

export interface HubUnrankableReasonResult extends JsonObject {
  reason: HubUnrankableReasonKind;
  differences?: JsonObject[];
  state?: string | null;
  has?: string;
  floor?: string;
  detail?: string;
}

export interface HubRankedEntryResult extends JsonObject {
  rank: number;
  entry: JsonObject;
  verification: string;
  label: HubCardLabelResult;
}

export interface HubUnrankedEntryResult extends JsonObject {
  entry: JsonObject;
  reason: HubUnrankableReasonResult;
}

export interface HubRankedBoardResult extends JsonObject {
  board: string;
  conditions: JsonObject;
  ranked: HubRankedEntryResult[];
  unranked: HubUnrankedEntryResult[];
}

export interface HubLeaderboardRenderResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/hub-leaderboard/0.1";
  board: string;
  ranked_count: number;
  unranked_count: number;
  leader_count: number;
  headline: string;
  rendered: HubRankedBoardResult | null;
  guarantees: string[];
}

export type HubModerationState = "submitted" | "under-review" | "accepted" | "rejected" | "withdrawn" | "superseded";
export type HubModerationVerification = "self-reported" | "reproduced" | "verified" | "prospectively-validated";
export type HubModerationEventKind = "opened" | "transition" | "attestation";

export interface HubSubmissionReviewArgs extends JsonObject {
  draft: JsonObject;
  submitter: JsonObject;
  moderation?: JsonObject;
}

export interface HubModerationEventResult extends JsonObject {
  submission: string;
  kind: HubModerationEventKind;
  actor: string;
  at: number;
  reason?: string | null;
  superseded_by?: string | null;
  from?: string;
  to?: string;
}

export interface HubTombstoneResult extends JsonObject {
  submission: string;
  submitter: string;
  content: string;
  withdrawn_at: number;
  actor: string;
  reason: string;
  states_traversed: HubModerationState[];
}

export interface HubModerationRecordResult extends JsonObject {
  submission: JsonObject;
  state: HubModerationState;
  verification: HubModerationVerification;
  history: HubModerationEventResult[];
  tombstone?: HubTombstoneResult | null;
}

export interface HubModerationLedgerResult extends JsonObject {
  records: Record<string, HubModerationRecordResult>;
  events: HubModerationEventResult[];
  last_epoch: number;
}

export interface HubSubmissionReviewResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/hub-submission/0.1";
  stage: string;
  submission: JsonObject | null;
  limitation_card?: string | null;
  moderation?: JsonObject | null;
  state?: HubModerationState;
  verification?: HubModerationVerification;
  published?: string[];
  event_count?: number;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
}

export interface DeveloperWorkbenchArgs extends JsonObject {
  session: JsonObject;
  dashboard?: JsonObject;
  ci?: JsonObject;
}

export interface CiExecutionEvidenceArgs extends JsonObject {
  ci: JsonObject;
  evidence: JsonObject;
}

export interface CiProviderNormalizationArgs extends JsonObject {
  ci: JsonObject;
  provider: "github_actions" | "gitlab_ci" | "generic";
  payload: JsonObject;
  source?: "caller_attested" | "provider_observed";
}

export interface CiProviderArtifactArgs extends JsonObject {
  id: string;
  kind: string;
  digest: string;
  check?: string;
  run_id?: string;
  provider?: string;
  uri?: string;
}

export interface CiProviderLogArgs extends JsonObject {
  id: string;
  digest: string;
  check?: string;
  run_id?: string;
  provider?: string;
  uri?: string;
  truncated?: boolean;
}

export interface CiProviderAttestationArgs extends JsonObject {
  id: string;
  subject: string;
  issuer: string;
  statement_digest: string;
  method: string;
}

export interface CiProviderEvidenceArgs extends JsonObject {
  ci: JsonObject;
  provider: "github_actions" | "gitlab_ci" | "generic";
  payload: JsonObject;
  source?: "caller_attested" | "provider_observed";
  artifacts?: CiProviderArtifactArgs[];
  logs?: CiProviderLogArgs[];
  attestations?: CiProviderAttestationArgs[];
}

export interface CiEvidenceFindingResult extends JsonObject {
  code: string;
  severity: string;
  subject: string;
  detail: string;
}

export interface CiExecutionEvidenceAuditResult extends JsonObject {
  schema: "bioprism-devplat-ci-execution-evidence/0.1";
  workflow: string;
  plan_digest: string;
  evidence_digest: string;
  run_id: string;
  provider: string;
  source: string;
  conclusion: string;
  expected_check_count: number;
  observed_check_count: number;
  passed_check_count: number;
  failed_check_count: number;
  skipped_check_count: number;
  unknown_check_count: number;
  required_missing: string[];
  required_failed: string[];
  optional_nonpassing: string[];
  complete: boolean;
  structurally_valid: boolean;
  release_candidate: boolean;
  execution: string;
  verification: string;
  findings: CiEvidenceFindingResult[];
  guarantees: string[];
  limitations: string[];
}

export interface CiExecutionEvidenceResult extends JsonObject {
  ok: boolean;
  workflow: "ci_execution_evidence_audit";
  schema: "bioprism-devplat-ci-execution-evidence/0.1";
  valid: boolean;
  ci_evidence_ready: boolean;
  plan_digest: string;
  evidence_digest: string;
  audit: CiExecutionEvidenceAuditResult;
  guarantees: string[];
  limitations: string[];
}

export interface CiProviderNormalizationEvidenceResult extends JsonObject {
  run_id: string;
  provider: string;
  source: string;
  plan_digest: string;
  conclusion: string;
  checks: JsonObject[];
  environment_digest?: string | null;
  run_url?: string | null;
}

export interface CiProviderNormalizationResult extends JsonObject {
  ok: boolean;
  workflow: "ci_provider_normalize";
  schema: "bioprism-devplat-ci-provider-normalization/0.1";
  provider: string;
  source: string;
  payload_digest: string;
  run_id: string;
  conclusion: string;
  check_count: number;
  derived_result_digest_count: number;
  warnings: string[];
  evidence: CiProviderNormalizationEvidenceResult;
  normalization: JsonObject;
  guarantees: string[];
  limitations: string[];
}

export interface CiProviderEvidenceAuditResult extends JsonObject {
  schema: "bioprism-devplat-ci-provider-evidence/0.1";
  workflow: "ci_provider_evidence_audit";
  provider: string;
  source: string;
  run_id: string;
  payload_digest: string;
  plan_digest: string;
  evidence_digest: string;
  artifact_record_digest: string;
  log_record_digest: string;
  attestation_record_digest: string;
  evidence: JsonObject;
  artifact_count: number;
  log_count: number;
  attestation_count: number;
  linked_artifact_count: number;
  linked_log_count: number;
  attestation_subject_count: number;
  ci_evidence: JsonObject;
  artifacts: CiProviderArtifactArgs[];
  logs: CiProviderLogArgs[];
  attestations: CiProviderAttestationArgs[];
  structurally_valid: boolean;
  conformance_ready: boolean;
  execution: string;
  verification: string;
  findings: CiEvidenceFindingResult[];
  guarantees: string[];
  limitations: string[];
}

export interface CiProviderEvidenceResult extends JsonObject {
  ok: boolean;
  workflow: "ci_provider_evidence_audit";
  schema: "bioprism-devplat-ci-provider-evidence/0.1";
  valid: boolean;
  conformance_ready: boolean;
  provider: string;
  source: string;
  run_id: string;
  payload_digest: string;
  plan_digest: string;
  evidence_digest: string;
  artifact_record_digest: string;
  log_record_digest: string;
  attestation_record_digest: string;
  evidence: JsonObject;
  audit: CiProviderEvidenceAuditResult;
  guarantees: string[];
  limitations: string[];
}

export interface DelegatedCheckEvidenceArgs extends JsonObject {
  name: string;
  kind: string;
  required: boolean;
  status: "passed" | "failed" | "refused" | "not_run" | "unknown";
  result_digest: string;
  source: string;
  trace_sequence?: number;
}

export interface ExecutionProvenanceArgs extends JsonObject {
  mission: JsonObject;
  delegated_checks?: DelegatedCheckEvidenceArgs[];
}

export interface ExecutionProvenanceFindingResult extends JsonObject {
  code: string;
  severity: string;
  subject: string;
  detail: string;
}

export interface ExecutionProvenanceResult extends JsonObject {
  ok: boolean;
  workflow: "execution_provenance_audit";
  schema: "bioprism-devplat-execution-provenance/0.1";
  valid: boolean;
  provenance_ready: boolean;
  mission_id: string;
  plan_digest: string;
  trace_digest: string;
  provenance_digest: string;
  mission_execution: string;
  mission_status: string;
  planned_step_count: number;
  result_count: number;
  trace_event_count: number;
  delegated_check_count: number;
  required_failure_count: number;
  required_check_count: number;
  passed_check_count: number;
  nonpassing_required_checks: string[];
  missing_step_results: string[];
  unknown_step_results: string[];
  duplicate_trace_sequences: number[];
  trace_identity_errors: string[];
  complete: boolean;
  structurally_valid: boolean;
  release_candidate: boolean;
  execution: string;
  verification: string;
  findings: ExecutionProvenanceFindingResult[];
  guarantees: string[];
  limitations: string[];
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

export interface CapabilityDashboardArgs extends JsonObject {
  group_id?: string;
  domain?: string;
  status?: string;
  max_groups?: number;
  include_tools?: boolean;
  include_gaps?: boolean;
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

export interface MissionEvaluatorDiscoverArgs extends JsonObject {
  query?: string;
  group_id?: string;
  domain?: string;
  level?: "observation" | "evaluation" | "operational" | "release" | string;
  adapter_id?: string;
  max_items?: number;
}

export interface MissionEvaluatorAdapterResult extends JsonObject {
  id: string;
  group_id: string;
  domains: string[];
  levels: string[];
  purpose: string;
  candidate_tools: string[];
  output_pointer_examples: string[];
  status: "candidate_only" | string;
}

export interface MissionEvaluatorMatchResult extends JsonObject {
  adapter: MissionEvaluatorAdapterResult;
  score: number;
  matched_fields: string[];
}

export interface MissionEvaluatorCoverageResult extends JsonObject {
  capability_group_count: number;
  evaluator_group_count: number;
  uncovered_groups: string[];
  unbound_groups: string[];
  complete: boolean;
  posture: string;
}

export interface MissionEvaluatorDiscoverResult extends JsonObject {
  ok: boolean;
  workflow: "mission_evaluator_discover";
  mission_evaluator_schema_version: string;
  schema_version: string;
  catalog_digest: string;
  total_adapters: number;
  query: JsonObject;
  result_count: number;
  matches: MissionEvaluatorMatchResult[];
  coverage: MissionEvaluatorCoverageResult;
  selection_posture: "candidate_only";
  guarantees: string[];
  limitations: string[];
}

export interface MissionEvaluatorSelectionArgs extends JsonObject {
  id: string;
  claim_id: string;
  adapter_id: string;
  domain: string;
  step_id: string;
  output_pointer: string;
  required?: boolean;
}

export interface MissionEvaluatorReviewArgs extends JsonObject {
  discovery: JsonObject;
  selections: MissionEvaluatorSelectionArgs[];
}

export interface MissionEvaluatorBindingReviewResult extends JsonObject {
  id: string;
  claim_id: string;
  adapter_id: string;
  domain: string;
  step_id: string;
  output_pointer: string;
  required: boolean;
  candidate_found: boolean;
  domain_supported: boolean;
  binding_posture: "ready" | "blocked" | string;
  candidate_tools?: string[];
  output_pointer_examples?: string[];
  proposed_binding?: JsonObject;
}

export interface MissionEvaluatorReviewFinding extends JsonObject {
  selection_id?: string;
  claim_id?: string;
  severity: "error" | string;
  code: string;
  message: string;
}

export interface MissionEvaluatorCatalogueSnapshot extends JsonObject {
  schema: "bioprism-devplat-mission-evaluator-catalogue-snapshot/0.1" | string;
  catalog_digest: string;
  snapshot_digest: string;
  row_count: number;
  group_count: number;
  rows: JsonObject[];
  retention: JsonObject;
  execution: "not_started";
}

export interface MissionEvaluatorReviewResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "mission_evaluator_review";
  review_id: string;
  catalog_digest: string;
  discovery_digest: string;
  catalogue_snapshot?: MissionEvaluatorCatalogueSnapshot;
  selection_count: number;
  claim_count: number;
  bindings: MissionEvaluatorBindingReviewResult[];
  findings: MissionEvaluatorReviewFinding[];
  review_status: "ready" | "blocked";
  binding_posture: "ready_for_mission_claim_bindings" | "requires_caller_correction" | string;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface MissionEvaluatorReplayArgs extends JsonObject {
  mission: JsonObject;
  include_fixtures?: boolean;
  max_items?: number;
}

export interface MissionEvaluatorReplayBindingResult extends JsonObject {
  id?: string;
  claim_id: string;
  adapter_id: string;
  domain: string;
  outcome_state?: string;
  output_digest?: string;
  catalog_match: boolean;
  domain_supported: boolean;
  replay_state: string;
}

export interface MissionEvaluatorReplayClaimResult extends JsonObject {
  claim_id: string;
  binding_count: number;
  returned_binding_count: number;
  outcome_counts: JsonObject;
  distinct_output_digests: number;
  disagreement_posture: JsonValue;
}

export interface MissionEvaluatorReplayCoverageResult extends JsonObject {
  catalogue_adapter_count: number;
  catalogue_group_count: number;
  replayed_adapter_count: number;
  replayed_group_count: number;
  unrepresented_adapters: string[];
  unrepresented_groups: string[];
  complete: boolean;
}

export interface MissionEvaluatorReplayFixtureResult extends JsonObject {
  fixture_id: string;
  adapter_id: string;
  group_id: string;
  domains: string[];
  levels: string[];
  output_pointer: string;
  retained_output: JsonValue;
  retained_output_digest: string;
  variants: JsonValue[];
  guarantee: string;
}

export interface MissionEvaluatorReplayResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "mission_evaluator_replay";
  mission_id: string;
  mission_digest: string;
  mission_status: JsonValue;
  review_provenance: JsonValue;
  route_review_provenance?: JsonObject | null;
  route_review_status?: "absent" | "valid" | "invalid" | string;
  catalog_digest: string;
  binding_count: number;
  omitted_bindings: number;
  state_counts: JsonObject;
  claims: MissionEvaluatorReplayClaimResult[];
  bindings: MissionEvaluatorReplayBindingResult[];
  coverage: MissionEvaluatorReplayCoverageResult;
  findings: JsonObject[];
  replay_status: "ready" | "blocked";
  execution: "not_started";
  fixtures: MissionEvaluatorReplayFixtureResult[];
  omitted_fixtures: number;
  guarantees: string[];
  limitations: string[];
  artifact_registry?: JsonObject;
}

export interface MissionEvaluatorReplayCompareArgs extends JsonObject {
  mission: JsonObject;
  include_fixtures?: boolean;
  max_items?: number;
}

export interface MissionEvaluatorReplayCompareResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-mission-evaluator-replay-compare/0.1" | string;
  workflow: "mission_evaluator_replay_compare";
  mission_id: string;
  replay: JsonObject;
  catalog_drift: JsonObject;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
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

export interface CapabilityDashboardGroupResult extends JsonObject {
  id: string;
  domains: string[];
  status: string;
  readiness: "callable" | "partial" | "declared_only" | string;
  surfaces: { crates: number; mcp_tools: number; cli_entrypoints: number; python_artifacts: number };
  tool_count: number;
  callable_tool_count: number;
  schema_backed_tool_count: number;
  missing_transport_schemas: string[];
  invalid_transport_schemas: string[];
  tools?: string[];
  gaps?: string[];
  artifact_evidence?: OperationsArtifactEvidencePosture;
  workflow_reconciliation_evidence?: OperationsReconciliationPosture;
}

export interface CapabilityDashboardEvidenceResult extends JsonObject {
  scope: string;
  evidence_digest: string;
  artifact_registry_generation: number;
  artifact_registry_size: number;
  workflow_reconciliation_registry_generation: number;
  workflow_reconciliation_registry_size: number;
  groups_with_artifact_evidence: number;
  artifact_evidence_records: number;
  groups_with_workflow_reconciliation: number;
  workflow_reconciliation_records: number;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface CapabilityDashboardAuditResult extends JsonObject {
  schema: "bioprism-devplat-capability-dashboard/0.1";
  catalog_digest: string;
  dashboard_digest: string;
  query: JsonObject;
  total_group_count: number;
  selected_group_count: number;
  available_group_count: number;
  callable_group_count: number;
  partial_group_count: number;
  declared_only_group_count: number;
  selected_tool_memberships: number;
  selected_unique_tools: number;
  schema_backed_unique_tools: number;
  readiness_counts: Record<string, number>;
  gap_counts: Record<string, number>;
  groups: CapabilityDashboardGroupResult[];
  evidence?: CapabilityDashboardEvidenceResult;
  warnings: string[];
  guarantees: string[];
  limitations: string[];
  ready: boolean;
}

export interface CapabilityDashboardResult extends JsonObject {
  ok: boolean;
  workflow: "capability_dashboard";
  schema: "bioprism-devplat-capability-dashboard/0.1";
  catalog_digest: string;
  dashboard_digest: string;
  evidence_digest?: string;
  evidence_scope?: string;
  capability_dashboard_ready: boolean;
  audit: CapabilityDashboardAuditResult;
  duplicate_schema_names?: string[];
  guarantees?: string[];
  limitations?: string[];
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
  candidate_group_evidence?: CapabilityRouteGroupEvidenceResult[];
  search: JsonObject;
}

export interface CapabilityRouteGroupEvidenceResult extends JsonObject {
  id: string;
  artifact_evidence: JsonObject;
  workflow_reconciliation_evidence: JsonObject;
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
  candidate_group_evidence_count?: number;
  posture: string;
}

export interface CapabilityRouteEvidenceResult extends JsonObject {
  scope: string;
  evidence_digest: string;
  artifact_registry_generation: number;
  artifact_registry_size: number;
  workflow_reconciliation_registry_generation: number;
  workflow_reconciliation_registry_size: number;
  candidate_group_count: number;
  groups_with_artifact_evidence: number;
  artifact_evidence_records: number;
  groups_with_workflow_reconciliation: number;
  workflow_reconciliation_records: number;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
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
  evidence_digest?: string;
  evidence_scope?: string;
  evidence?: CapabilityRouteEvidenceResult;
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
  evidence_digest?: string;
  evidence_scope?: string;
  evidence_binding?: JsonObject;
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

export interface CapabilityRoutePlanArgs extends JsonObject {
  mission_id: string;
  route: JsonObject;
  selections: MissionRouteSelection[];
  validate_schemas?: boolean;
  policy?: JsonObject;
  claim_requests?: JsonValue[];
  evaluator_review?: JsonObject;
  workflow_binding?: JsonObject;
}

export interface CapabilityRoutePlanResult extends JsonObject {
  ok: boolean;
  workflow: "capability_route_plan";
  mission_id: string;
  route_id: string;
  review_id: string;
  catalog_digest: string;
  goal: string;
  plan_status: "preflight_pending" | "blocked_by_route_review" | "ready_for_caller_inspection" | "blocked_by_mission_preflight";
  review: CapabilityRouteReviewResult;
  mission: JsonObject | null;
  preflight: JsonObject | null;
  plan_digest?: string | null;
  route_input_digest?: string | null;
  selection_digest?: string | null;
  selection_count?: number;
  route_review_provenance?: JsonObject | null;
  dispatch: "not_started";
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface CapabilityRoutePlanVerifyArgs extends JsonObject {
  plan: JsonObject;
  route?: JsonObject;
  selections?: MissionRouteSelection[];
  validate_schemas?: boolean;
}

export interface CapabilityRoutePlanVerifyResult extends JsonObject {
  ok: boolean;
  workflow: "capability_route_plan_verify";
  mission_id: string;
  route_id: string;
  review_id: string;
  catalog_digest: string;
  plan_status: string;
  plan_digest?: string | null;
  valid: boolean;
  verification_status: "verified" | "verified_without_route_replay" | "mismatch" | "blocked_by_route_replay" | "blocked_by_mission_preflight";
  route_replay: JsonObject;
  mission_preflight: JsonObject;
  mismatches: JsonObject[];
  dispatch: "not_started";
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface DomainWorkflowInstantiateArgs extends JsonObject {
  workflow_id: string;
  mission_id: string;
  goal: string;
  steps: JsonValue[];
  policy?: JsonObject;
  claim_requests?: JsonValue[];
  evaluator_review?: JsonObject;
  route_review?: JsonObject;
}

export interface DomainWorkflowVerifyArgs extends JsonObject {
  instantiation: JsonObject;
  replay_request?: DomainWorkflowInstantiateArgs;
}

export interface DomainWorkflowScaffoldArgs extends JsonObject {
  workflow_id: string;
  mission_id: string;
  goal: string;
  tools?: string[];
  arguments?: Record<string, JsonObject>;
}

export interface DomainWorkflowReconcileArgs extends JsonObject {
  instantiation: JsonObject;
  mission_report?: JsonObject;
  evidence_bundle?: JsonObject;
}

export interface DomainWorkflowToolContract extends JsonObject {
  name: string;
  role: string;
  declared: boolean;
  available: boolean;
  schema_state: "present" | "missing" | "unavailable";
  schema_digest?: string | null;
  argument_validation: "authoritative_mcp_preflight_required";
  argument_contract?: JsonObject;
  execution_contract?: DomainWorkflowExecutionContract;
  evidence: JsonObject;
}

export interface DomainWorkflowExecutionContract extends JsonObject {
  resource_class?: "compile" | "ingest" | "sandbox" | "evaluate" | "mutate" | "index" | string;
  idempotency?: string;
  side_effects?: string;
  dispatch?: string;
  providers?: JsonObject;
  provider_boundary?: JsonObject;
  queue_resource_class?: string;
  readiness_claimed?: false;
}

export interface DomainWorkflowContract extends JsonObject {
  schema: string;
  posture: "advisory_review_gated";
  scope: JsonObject;
  readiness: JsonObject;
  pre_dispatch_gates: JsonValue[];
  evidence_contract: JsonObject;
  completion_contract: JsonObject;
  execution_boundary?: DomainWorkflowExecutionContract;
}

export interface DomainWorkflowTemplate extends JsonObject {
  workflow_id: string;
  workflow_digest: string;
  domain_contract: DomainWorkflowContract;
  domain_contract_digest: string;
  tool_contracts: DomainWorkflowToolContract[];
  tools: JsonObject;
  recommended_stages: JsonValue[];
  execution_contract?: DomainWorkflowExecutionContract;
}

export interface DomainWorkflowEvidencePlan extends JsonObject {
  schema: string;
  steps: JsonValue[];
  completion: JsonObject;
}

export interface DomainWorkflowBinding extends JsonObject {
  workflow_id: string;
  workflow_digest: string;
  catalog_digest: string;
  domain_contract_digest: string;
  domain_contract: DomainWorkflowContract;
  evidence_plan: DomainWorkflowEvidencePlan;
  evidence_plan_digest: string;
}

export interface DomainWorkflowCatalogueResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_catalogue";
  catalog_digest: string;
  workflow_catalog_digest: string;
  workflow_count: number;
  workflows: DomainWorkflowTemplate[];
  coverage: JsonObject;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface DomainWorkflowInstantiateResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_instantiate";
  workflow_id: string;
  workflow_digest: string;
  catalog_digest: string;
  mission: JsonObject & { workflow_binding?: DomainWorkflowBinding };
  selection: JsonObject;
  domain_contract: DomainWorkflowContract;
  domain_contract_digest: string;
  evidence_plan: DomainWorkflowEvidencePlan;
  preflight: JsonObject;
  preflight_report?: JsonObject;
  execution: "not_started";
  execution_contract?: DomainWorkflowExecutionContract;
  guarantees: string[];
  limitations: string[];
}

export interface DomainWorkflowVerifyResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_verify";
  workflow_id: string;
  workflow_digest: string;
  catalog_digest: string;
  domain_contract_digest: string;
  mission_id: string;
  mission_digest: string;
  structural_valid: boolean;
  valid: boolean;
  verification_status: "verified" | "verified_without_replay" | "mismatch" | "blocked_by_replay" | "blocked_by_mission_preflight";
  replay: JsonObject;
  mission_preflight: JsonObject;
  mismatches: JsonObject[];
  preflight_report: JsonObject;
  dispatch: "not_started";
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface DomainWorkflowScaffoldResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_scaffold";
  workflow_id: string;
  workflow_digest: string;
  catalog_digest: string;
  selection: JsonObject;
  instantiation: DomainWorkflowInstantiateResult;
  mission: JsonObject & { workflow_binding?: DomainWorkflowBinding };
  domain_contract: DomainWorkflowContract;
  domain_contract_digest: string;
  evidence_plan: DomainWorkflowEvidencePlan;
  execution: "not_started";
  execution_contract?: DomainWorkflowExecutionContract;
  readiness_claimed: false;
  preflight: JsonObject;
  preflight_status: "ready" | "blocked";
  preflight_report: JsonObject;
  guarantees: string[];
  limitations: string[];
  next_actions: string[];
}

export interface DomainWorkflowReconcileResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_reconcile";
  workflow_id: string;
  workflow_digest: string;
  catalog_digest: string;
  domain_contract_digest: string;
  mission_id: string;
  mission_plan_digest: string;
  reconciliation_digest: string;
  source: string;
  report: JsonObject;
  retention: JsonObject;
  bundle_verification: JsonObject;
  evidence: JsonObject;
  completion: JsonObject;
  integrity: JsonObject;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
  artifact_registry?: JsonObject;
}

/** Import a digest-bound reconciliation report into the bounded audit registry. */
export interface DomainWorkflowReconciliationImportArgs extends JsonObject {
  record: JsonObject;
}

export interface DomainWorkflowReconciliationImportResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_reconciliation_import";
  reconciliation_digest: string;
  created: boolean;
  already_present: boolean;
  registry_generation: number;
  registry_size: number;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
  artifact_registry?: JsonObject;
}

/** Bounded indexed lookup over retained domain-workflow reconciliation reports. */
export interface DomainWorkflowReconciliationQueryOptions extends JsonObject {
  mission_id?: string;
  workflow_id?: string;
  mission_plan_digest?: string;
  completion_status?: string;
  after?: string;
  max_items?: number;
  include_records?: boolean;
}

export interface DomainWorkflowReconciliationQueryResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_reconciliation_query";
  filters: DomainWorkflowReconciliationQueryOptions;
  registry_generation: number;
  registry_size: number;
  rows: JsonObject[];
  next_after: string | null;
  has_more: boolean;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface DomainWorkflowReconciliationGetResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_reconciliation_get";
  reconciliation_digest: string;
  record: JsonObject;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export type DomainReportClaimStatus =
  | "observed"
  | "derived"
  | "review_required"
  | "refused"
  | "not_applicable";

export interface DomainReportClaimPosture extends JsonObject {
  status: DomainReportClaimStatus;
  does_not_claim: string[];
  limitations?: string[];
}

export interface DomainReportProjectArgs extends JsonObject {
  operation?: "project" | "from_adapter_execution" | "from_provider_normalization" | "from_external_provider_normalization";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  report: JsonObject;
  claim_posture: DomainReportClaimPosture;
  source_plan_digest?: string | null;
  parent_digests?: string[];
  evidence?: AdapterExecutionEvidenceArgs;
  conformance?: JsonObject;
  normalization?: DomainEvidenceProviderNormalizationArgs | DomainEvidenceProviderExternalPayloadNormalizationArgs;
}

export interface AdapterDomainReportArgs extends JsonObject {
  operation: "from_adapter_execution";
  evidence: AdapterExecutionEvidenceArgs;
  conformance?: JsonObject;
}

export interface AdapterDomainReportResult extends JsonObject {
  ok: true;
  schema: "bioprism-devplat-adapter-domain-report/0.1";
  workflow: "adapter_domain_report";
  evidence: AdapterExecutionEvidenceResult;
  domain_report: DomainReportProjectResult;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface ProviderDomainReportArgs extends JsonObject {
  operation: "from_provider_normalization" | "from_external_provider_normalization";
  normalization: DomainEvidenceProviderNormalizationArgs | DomainEvidenceProviderExternalPayloadNormalizationArgs;
  parent_digests?: string[];
}

export interface ProviderDomainReportResult extends JsonObject {
  ok: true;
  schema: "bioprism-devplat-provider-domain-report/0.1";
  workflow: "provider_domain_report";
  mode: "inline" | "external_payload";
  normalization: DomainEvidenceProviderNormalizationResult | DomainEvidenceProviderExternalPayloadNormalizationResult;
  domain_report: DomainReportProjectResult;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainReportArtifactRegistryProjection extends JsonObject {
  indexed: boolean;
  kind: "domain_report";
  subject_id: string;
  content_digest: string;
  created?: boolean;
  already_present?: boolean;
  verification?: JsonObject;
  lookup?: string;
}

export interface DomainReportProjectResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-report-project/0.1";
  workflow: "domain_report_project";
  report: JsonObject;
  artifact_registry: DomainReportArtifactRegistryProjection;
  coverage: JsonObject;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainReportCoverageOptions extends JsonObject {
  operation?: "coverage";
  group_id?: string;
  domain?: string;
  report_class?: string;
  bridge_mode?: string;
  max_groups?: number;
  include_report_digests?: boolean;
}

export interface DomainReportCoverageGroup extends JsonObject {
  id: string;
  domains: string[];
  status: string;
  declared_tool_count: number;
  report_count: number;
  subject_ids: string[];
  source_tools: string[];
  claim_statuses: DomainReportClaimStatus[];
  report_classes?: JsonObject;
  bridge_modes?: string[];
  lineage_parent_count?: number;
  reports_with_lineage_parents?: number;
  report_digests?: string[];
  coverage_state: "reported" | "missing";
}

export interface DomainReportCoverageResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-report-coverage/0.1";
  workflow: "domain_report_coverage";
  catalogue_digest: string;
  coverage_digest: string;
  filters: DomainReportCoverageOptions;
  group_count: number;
  reported_group_count: number;
  missing_group_count: number;
  missing_group_ids: string[];
  complete: boolean;
  groups: DomainReportCoverageGroup[];
  domain_summary: JsonObject;
  bridge_summary?: JsonObject;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export type DomainEvidenceLinkRole = "supports" | "qualifies" | "contradicts" | "context";

export interface DomainEvidenceLinkArgs extends JsonObject {
  report_index: number;
  role: DomainEvidenceLinkRole;
  note?: string;
  report_digest?: string;
}

export interface DomainEvidenceHarmonizeArgs extends JsonObject {
  subject_id: string;
  claim: JsonObject;
  reports: JsonObject[];
  links: DomainEvidenceLinkArgs[];
  required_group_ids?: string[];
  required_domains?: string[];
}

export interface DomainEvidenceHarmonizationReportRow extends JsonObject {
  index: number;
  digest: string;
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  claim_status: DomainReportClaimStatus | null;
  parent_digests: string[];
  report_class: string;
  bridge_mode: string | null;
  lineage_parent_count: number;
  link_roles: DomainEvidenceLinkRole[];
  link_count: number;
}

export interface DomainEvidenceHarmonizationBridgeSummary extends JsonObject {
  report_classes: Record<string, number>;
  modes: Record<string, number>;
  lineage: {
    parent_digest_count: number;
    reports_with_lineage_parents: number;
    reports_without_lineage_parents: number;
  };
}

export interface DomainEvidenceHarmonizationCoverage extends JsonObject {
  all_reports_linked: boolean;
  requirements_complete: boolean;
  traceability_state: "complete" | "requirements_missing" | "links_missing";
  observed_group_count: number;
  observed_domain_count: number;
  bridge_summary: DomainEvidenceHarmonizationBridgeSummary;
}

export interface DomainEvidenceHarmonizationResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-harmonization/0.1";
  workflow: "domain_evidence_harmonize";
  harmonization: JsonObject & {
    reports?: DomainEvidenceHarmonizationReportRow[];
    coverage?: DomainEvidenceHarmonizationCoverage;
  };
  artifact_registry: JsonObject;
  catalogue_digest: string;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceHarmonizationCoverageOptions extends JsonObject {
  subject_id?: string;
  domain?: string;
  report_class?: string;
  bridge_mode?: string;
  traceability_state?: "complete" | "requirements_missing" | "links_missing";
  after?: string;
  max_items?: number;
  include_report_digests?: boolean;
}

export interface DomainEvidenceHarmonizationCoverageRow extends JsonObject {
  content_digest: string;
  subject_id: string;
  domains: string[];
  claim_id: string | null;
  report_count: number;
  link_count: number;
  traceability_state: "complete" | "requirements_missing" | "links_missing";
  requirements_complete: boolean | null;
  all_reports_linked: boolean | null;
  contradiction_declared: boolean;
  qualification_declared: boolean;
  report_classes: Record<string, number>;
  bridge_modes: Record<string, number>;
  lineage: JsonObject;
  missing_group_ids: string[] | null;
  missing_domains: string[] | null;
  report_digests?: string[];
}

export interface DomainEvidenceHarmonizationCoverageResult extends JsonObject {
  ok: true;
  schema: "bioprism-devplat-domain-evidence-harmonization-coverage/0.1";
  workflow: "domain_evidence_harmonization_coverage";
  filters: DomainEvidenceHarmonizationCoverageOptions;
  registry_size: number;
  matching_count: number;
  returned_count: number;
  has_more: boolean;
  next_after: string | null;
  rows: DomainEvidenceHarmonizationCoverageRow[];
  summary: JsonObject;
  coverage_digest: string;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export type DomainEvidenceIntakeOutcome = "observed" | "partial" | "refused" | "error" | "unknown";

export interface DomainEvidenceIntakeArgs extends JsonObject {
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  request?: JsonValue;
  response: JsonValue;
  outcome: DomainEvidenceIntakeOutcome;
  source_plan_digest: string | null;
  claim_posture: DomainReportClaimPosture;
  parent_digests?: string[];
}

export interface DomainEvidenceIntakeResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-intake/0.1";
  workflow: "domain_evidence_intake";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  request_supplied: boolean;
  request_digest: string;
  response_digest: string;
  intake_digest: string;
  outcome: DomainEvidenceIntakeOutcome;
  report: JsonObject;
  intake: JsonObject;
  artifact_registry: JsonObject;
  catalogue_digest: string;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceIntakeCoverageOptions extends JsonObject {
  group_id?: string;
  domain?: string;
  max_groups?: number;
  include_intake_digests?: boolean;
}

export interface DomainEvidenceIntakeCoverageGroup extends JsonObject {
  id: string;
  domains: string[];
  status: string;
  declared_tool_count: number;
  declared_tools: string[];
  intake_count: number;
  subject_ids: string[];
  source_tools: string[];
  outcomes: DomainEvidenceIntakeOutcome[];
  reported_domains: string[];
  missing_source_tools: string[];
  source_tool_coverage: {
    tool: string;
    intake_count: number;
    outcomes: DomainEvidenceIntakeOutcome[];
    coverage_state: "reported" | "missing";
  }[];
  missing_domains: string[];
  tool_coverage_state: "complete" | "partial" | "missing";
  domain_coverage_state: "complete" | "partial" | "missing";
  artifact_evidence: OperationsArtifactEvidencePosture;
  artifact_evidence_scope: "current_digest_verified_artifact_registry_exact_declared_matches";
  intake_digests?: string[];
  coverage_state: "reported" | "missing";
}

export interface DomainEvidenceIntakeCoverageResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-intake-coverage/0.1";
  workflow: "domain_evidence_intake_coverage";
  catalogue_digest: string;
  coverage_digest: string;
  filters: DomainEvidenceIntakeCoverageOptions;
  group_count: number;
  reported_group_count: number;
  missing_group_count: number;
  missing_group_ids: string[];
  complete: boolean;
  tool_coverage_complete: boolean;
  missing_tool_group_ids: string[];
  domain_coverage_complete: boolean;
  missing_domain_group_ids: string[];
  groups_with_artifact_evidence: number;
  artifact_evidence_records: number;
  artifact_registry_generation: number;
  artifact_registry_size: number;
  artifact_evidence_scope: "current_digest_verified_artifact_registry_exact_declared_matches";
  groups: DomainEvidenceIntakeCoverageGroup[];
  domain_summary: JsonObject;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export type DomainEvidenceSourceConnectorKind = "literature" | "clinical_trial" | "fhir" | "object_store" | "file" | "provider_api" | "generic_http";
export type DomainEvidenceSourceLocatorKind = "uri" | "path" | "opaque";
export type DomainEvidenceSourceRetrievalMode = "reference_only" | "metadata_only" | "content";

export type DomainEvidenceProviderConnectorKind = "literature" | "clinical_trial" | "fhir" | "object_store" | "provider_api";
export type DomainEvidenceProviderShapeStatus = "structured" | "partial" | "unclassified" | "refused";
export type DomainEvidenceProviderHandoffStatus = "prepared" | "submitted" | "observed" | "partial" | "refused" | "error" | "unknown";
export type DomainEvidenceProviderAuthStatus = "none" | "caller_asserted" | "delegated" | "unknown";

export interface DomainEvidenceProviderAuthPosture extends JsonObject {
  status: DomainEvidenceProviderAuthStatus;
  secret_refs?: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceProviderConnectorManifest extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1";
  connector_id: string;
  version: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  domains: string[];
  capabilities: string[];
  transport: "caller_managed";
  auth_posture: DomainEvidenceProviderAuthPosture;
}

export interface DomainEvidenceProviderHandoffArgs extends JsonObject {
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  manifest: DomainEvidenceProviderConnectorManifest;
  status?: DomainEvidenceProviderHandoffStatus;
  request_digest?: string;
  payload_digest?: string;
  source_plan_digest?: string;
  parent_digests?: string[];
  attempt_id?: string;
}

export interface DomainEvidenceProviderHandoff extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1";
  workflow: "domain_evidence_provider_connector_handoff";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  status: DomainEvidenceProviderHandoffStatus;
  manifest: DomainEvidenceProviderConnectorManifest;
  manifest_digest: string;
  request_digest: string | null;
  payload_digest: string | null;
  source_plan_digest: string | null;
  parent_digests: string[];
  attempt_id: string | null;
  handoff_digest: string;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderHandoffResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1";
  workflow: "domain_evidence_provider_connector_handoff";
  handoff: DomainEvidenceProviderHandoff;
  manifest_digest: string;
  handoff_digest: string;
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export type DomainEvidenceProviderExternalPayloadStorageBackend = "object_store" | "file" | "database" | "caller_managed";
export type DomainEvidenceProviderExternalPayloadLocatorKind = "opaque" | "uri" | "path";
export type DomainEvidenceProviderExternalPayloadAvailability = "available" | "partial" | "missing" | "unknown";
export type DomainEvidenceProviderExternalPayloadRetention = "ephemeral" | "durable" | "unknown";

export interface DomainEvidenceProviderExternalPayloadReceiptArgs extends JsonObject {
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  handoff_digest: string;
  transfer_id: string;
  payload_digest: string;
  byte_length: number;
  storage_backend: DomainEvidenceProviderExternalPayloadStorageBackend;
  locator_kind: DomainEvidenceProviderExternalPayloadLocatorKind;
  locator: string;
  content_type?: string | null;
  content_encoding?: string | null;
  request_digest?: string | null;
  parent_digests?: string[];
  availability?: DomainEvidenceProviderExternalPayloadAvailability;
  retention?: DomainEvidenceProviderExternalPayloadRetention;
  attempt_id?: string | null;
}

export interface DomainEvidenceProviderExternalPayloadReceipt extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1";
  workflow: "domain_evidence_provider_external_payload_receipt";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  handoff_digest: string;
  transfer_id: string;
  payload_digest: string;
  byte_length: number;
  storage_backend: DomainEvidenceProviderExternalPayloadStorageBackend;
  locator_kind: DomainEvidenceProviderExternalPayloadLocatorKind;
  locator: string;
  content_type: string | null;
  content_encoding: string | null;
  request_digest: string | null;
  parent_digests: string[];
  availability: DomainEvidenceProviderExternalPayloadAvailability;
  retention: DomainEvidenceProviderExternalPayloadRetention;
  attempt_id: string | null;
  receipt_digest: string;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderExternalPayloadReceiptResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1";
  workflow: "domain_evidence_provider_external_payload_receipt";
  receipt: DomainEvidenceProviderExternalPayloadReceipt;
  handoff_digest: string;
  payload_digest: string;
  receipt_digest: string;
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceProviderExternalPayloadReplayVerifyArgs extends DomainEvidenceProviderExternalPayloadReceiptArgs {
  expected_receipt_digest: string;
  expected_handoff_digest: string;
  expected_payload_digest: string;
  expected_byte_length: number;
}

export interface DomainEvidenceProviderExternalPayloadReplayVerification extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1";
  workflow: "domain_evidence_provider_external_payload_replay_verify";
  replay_status: "matched" | "mismatch";
  matched: boolean;
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  expected_receipt_digest: string;
  observed_receipt_digest: string;
  expected_handoff_digest: string;
  observed_handoff_digest: string;
  expected_payload_digest: string;
  observed_payload_digest: string;
  expected_byte_length: number;
  observed_byte_length: number;
  matches: Record<string, boolean>;
  differences: string[];
  receipt: DomainEvidenceProviderExternalPayloadReceipt;
  replay_digest: string;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderExternalPayloadReplayVerifyResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1";
  workflow: "domain_evidence_provider_external_payload_replay_verify";
  replay: DomainEvidenceProviderExternalPayloadReplayVerification;
  matched: boolean;
  replay_status: "matched" | "mismatch";
  replay_digest: string;
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceProviderExternalPayloadNormalizationArgs extends DomainEvidenceProviderExternalPayloadReceiptArgs {
  payload: JsonObject | JsonValue[];
  request?: JsonValue;
  outcome?: DomainEvidenceIntakeOutcome;
  claim_posture?: DomainReportClaimPosture;
  parent_digests?: string[];
  source_plan_digest?: string | null;
}

export interface DomainEvidenceProviderExternalPayloadNormalizationResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-normalization/0.1";
  workflow: "domain_evidence_provider_external_payload_normalize";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  provider: string;
  outcome: DomainEvidenceIntakeOutcome;
  payload_digest: string;
  request_digest: string | null;
  response: JsonObject;
  shape_audit: DomainEvidenceProviderShapeAudit;
  record_index: DomainEvidenceProviderRecordIndex;
  normalization: JsonObject;
  receipt: DomainEvidenceProviderExternalPayloadReceipt;
  receipt_digest: string;
  materialization: JsonObject;
  intake: JsonObject;
  artifact_registry: JsonObject;
  receipt_artifact_registry: JsonObject;
  catalogue_digest: string;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceProviderExternalPayloadLineageAuditArgs extends DomainEvidenceProviderExternalPayloadReceiptArgs {}

export interface DomainEvidenceProviderExternalPayloadLineageAudit extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1";
  workflow: "domain_evidence_provider_external_payload_lineage_audit";
  lineage_status: "matched" | "partial" | "mismatch" | "orphaned";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  receipt: DomainEvidenceProviderExternalPayloadReceipt;
  handoff: JsonObject | null;
  matches: Record<string, boolean>;
  differences: string[];
  payload_binding_status: "matched" | "mismatch" | "not_declared" | "not_available";
  lineage_digest: string;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderExternalPayloadLineageAuditResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1";
  workflow: "domain_evidence_provider_external_payload_lineage_audit";
  audit: DomainEvidenceProviderExternalPayloadLineageAudit;
  lineage_status: DomainEvidenceProviderExternalPayloadLineageAudit["lineage_status"];
  payload_binding_status: DomainEvidenceProviderExternalPayloadLineageAudit["payload_binding_status"];
  lineage_digest: string;
  receipt_registry: JsonObject;
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export type DomainEvidenceProviderExternalPayloadExecutionStatus = "submitted" | "transferred" | "partial" | "refused" | "error" | "unknown";

export interface DomainEvidenceProviderExternalPayloadExecutionEvidenceArgs extends DomainEvidenceProviderExternalPayloadReceiptArgs {
  expected_receipt_digest: string;
  execution_status: DomainEvidenceProviderExternalPayloadExecutionStatus;
  executor_id: string;
  observed_payload_digest?: string | null;
  observed_byte_length?: number | null;
  locator_opened?: boolean;
  observation_digest?: string | null;
}

export interface DomainEvidenceProviderExternalPayloadExecutionEvidence extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-execution-evidence/0.1";
  workflow: "domain_evidence_provider_external_payload_execution_evidence";
  evidence_status: "matched" | "partial" | "mismatch" | "orphaned";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  provider: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  expected_receipt_digest: string;
  retained_receipt_digest: string | null;
  observed_receipt_digest: string;
  execution_status: DomainEvidenceProviderExternalPayloadExecutionStatus;
  executor_id: string;
  observed_payload_digest: string | null;
  observed_byte_length: number | null;
  locator_opened: boolean;
  observation_digest: string | null;
  receipt: DomainEvidenceProviderExternalPayloadReceipt;
  matches: Record<string, boolean>;
  differences: string[];
  evidence_digest: string;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderExternalPayloadExecutionEvidenceResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-execution-evidence/0.1";
  workflow: "domain_evidence_provider_external_payload_execution_evidence";
  evidence: DomainEvidenceProviderExternalPayloadExecutionEvidence;
  evidence_status: DomainEvidenceProviderExternalPayloadExecutionEvidence["evidence_status"];
  evidence_digest: string;
  receipt_registry: JsonObject;
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceProviderExternalPayloadEvidenceQueryArgs extends JsonObject {
  group_id?: string | null;
  domain?: string | null;
  subject_id?: string | null;
  after?: string | null;
  max_items?: number;
  include_artifacts?: boolean;
}

export type DomainEvidenceProviderExternalPayloadEvidenceQueryJoinStatus = "missing_receipt" | "receipt_only" | "receipt_and_lineage" | "receipt_and_execution" | "complete";

export interface DomainEvidenceProviderExternalPayloadEvidenceQueryRow extends JsonObject {
  row_digest: string;
  receipt_digest: string;
  subject_id: string;
  group_id: string;
  domains: string[];
  receipt_present: boolean;
  lineage_status: string | null;
  lineage_digest: string | null;
  execution_evidence_status: string | null;
  execution_status: DomainEvidenceProviderExternalPayloadExecutionStatus | null;
  evidence_digest: string | null;
  join_status: DomainEvidenceProviderExternalPayloadEvidenceQueryJoinStatus;
  parent_digests: string[];
  receipt_artifact?: JsonObject | null;
  lineage_artifact?: JsonObject | null;
  execution_artifact?: JsonObject | null;
}

export interface DomainEvidenceProviderExternalPayloadEvidenceQueryResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-query/0.1";
  workflow: "domain_evidence_provider_external_payload_evidence_query";
  filters: DomainEvidenceProviderExternalPayloadEvidenceQueryArgs;
  registry_generation: number;
  registry_size: number;
  rows: DomainEvidenceProviderExternalPayloadEvidenceQueryRow[];
  next_after: string | null;
  has_more: boolean;
  query_digest: string;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderShapeCoverage extends JsonObject {
  candidate_fields: string[];
  present_record_count: number;
  missing_record_count: number;
}

export interface DomainEvidenceProviderShapeAudit extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-shape-audit/0.1";
  status: DomainEvidenceProviderShapeStatus;
  connector_kind: DomainEvidenceProviderConnectorKind;
  root_kind: "object" | "array";
  recognized_container: string | null;
  record_count: number;
  valid_record_count: number;
  invalid_record_count: number;
  identifier_coverage: DomainEvidenceProviderShapeCoverage;
  content_digest_coverage: DomainEvidenceProviderShapeCoverage | null;
  missing_fields: string[];
  warnings: string[];
  limitations: string[];
  shape_digest: string;
}

export interface DomainEvidenceProviderRecordIndex extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-record-index/0.1";
  connector_kind: DomainEvidenceProviderConnectorKind;
  recognized_container: string | null;
  record_count: number;
  indexed_record_count: number;
  omitted_record_count: number;
  row_digests: string[];
  index_digest: string;
  limitations: string[];
}

export interface DomainEvidenceProviderNormalizationArgs extends JsonObject {
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  provider: string;
  payload: JsonObject | JsonValue[];
  request?: JsonValue;
  outcome?: DomainEvidenceIntakeOutcome;
  claim_posture?: DomainReportClaimPosture;
  parent_digests?: string[];
  source_plan_digest?: string | null;
}

export interface DomainEvidenceProviderNormalizationResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-normalization/0.1";
  workflow: "domain_evidence_provider_normalize";
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  provider: string;
  outcome: DomainEvidenceIntakeOutcome;
  payload_digest: string;
  request_digest: string | null;
  response: JsonObject;
  shape_audit: DomainEvidenceProviderShapeAudit;
  record_index: DomainEvidenceProviderRecordIndex;
  normalization: JsonObject;
  intake: JsonObject;
  artifact_registry: JsonObject;
  catalogue_digest: string;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceProviderReplayVerifyArgs extends DomainEvidenceProviderNormalizationArgs {
  expected_payload_digest: string;
  expected_request_digest?: string | null;
  expected_shape_digest: string;
  expected_normalization_digest: string;
  expected_intake_digest: string;
}

export type DomainEvidenceProviderReplayStatus = "matched" | "mismatch";

export interface DomainEvidenceProviderReplayVerification extends JsonObject {
  schema: "bioprism-devplat-domain-evidence-provider-replay/0.1";
  workflow: "domain_evidence_provider_replay_verify";
  replay_status: DomainEvidenceProviderReplayStatus;
  matched: boolean;
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  connector_kind: DomainEvidenceProviderConnectorKind;
  provider: string;
  expected_payload_digest: string;
  observed_payload_digest: string;
  expected_request_digest: string | null;
  observed_request_digest: string | null;
  expected_shape_digest: string;
  observed_shape_digest: string;
  expected_normalization_digest: string;
  observed_normalization_digest: string;
  expected_intake_digest: string;
  observed_intake_digest: string;
  matches: JsonObject;
  differences: string[];
  shape_audit: DomainEvidenceProviderShapeAudit;
  record_index: DomainEvidenceProviderRecordIndex;
  replay_digest: string;
  guarantees: string[];
  limitations: string[];
}

export interface DomainEvidenceProviderReplayVerifyResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-provider-replay/0.1";
  workflow: "domain_evidence_provider_replay_verify";
  replay: DomainEvidenceProviderReplayVerification;
  matched: boolean;
  replay_status: DomainEvidenceProviderReplayStatus;
  replay_digest: string;
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceSourcePlanArgs extends JsonObject {
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool?: string | null;
  connector_kind: DomainEvidenceSourceConnectorKind;
  locator_kind: DomainEvidenceSourceLocatorKind;
  locator: string;
  retrieval_mode: DomainEvidenceSourceRetrievalMode;
  expected_content_digest?: string | null;
  parent_digests?: string[];
  retrieval_policy?: JsonObject;
  does_not_claim: string[];
}

export interface DomainEvidenceSourcePlanResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-source-plan/0.1";
  workflow: "domain_evidence_source_plan";
  plan_digest: string;
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string | null;
  connector_kind: DomainEvidenceSourceConnectorKind;
  locator_kind: DomainEvidenceSourceLocatorKind;
  locator: string;
  retrieval_mode: DomainEvidenceSourceRetrievalMode;
  expected_content_digest: string | null;
  parent_digests: string[];
  retrieval_policy: JsonObject;
  plan: JsonObject;
  artifact_registry: JsonObject;
  catalogue_digest: string;
  readiness_claimed: false;
  execution: "not_started";
  retrieval_status: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface DomainEvidenceSourceExecutionArgs extends JsonObject {
  source_plan_digest: string;
  source_tool?: string | null;
  request?: JsonValue;
  claim_posture?: JsonObject;
  parent_digests?: string[];
}

export interface DomainEvidenceSourceExecutionResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-evidence-source-execution/0.1";
  workflow: "domain_evidence_source_execute";
  source_plan_digest: string;
  group_id: string;
  domains: string[];
  subject_id: string;
  source_tool: string;
  outcome: "observed" | "partial" | "refused" | "error" | "unknown";
  retrieval_status: string;
  execution: "completed" | "refused";
  raw_content_digest: string | null;
  response_digest: string;
  byte_length: number | null;
  content_type: string | null;
  execution_result: JsonObject;
  intake: JsonObject;
  artifact_registry: JsonObject;
  catalogue_digest: string;
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export type AdapterExecutionEvidenceExecutionStatus = "planned" | "started" | "succeeded" | "partial" | "refused" | "failed" | "unknown";
export type AdapterExecutionEvidenceConformanceStatus = "verified" | "partial" | "refused" | "not_run" | "unknown";
export type AdapterExecutionEvidenceSemanticLossStatus = "lossless" | "lossy" | "unknown" | "not_applicable";

export interface AdapterExecutionLossArgs extends JsonObject {
  kind: string;
  severity: "info" | "warning" | "blocking";
  detail: string;
  source_path?: string | null;
  target_path?: string | null;
}

export interface AdapterExecutionEvidenceArgs extends JsonObject {
  group_id: string;
  domains: string[];
  subject_id: string;
  adapter_id: string;
  adapter_version: string;
  source_id: string;
  input_digest: string;
  output_digest?: string | null;
  execution_status: AdapterExecutionEvidenceExecutionStatus;
  conformance_status: AdapterExecutionEvidenceConformanceStatus;
  semantic_loss_status: AdapterExecutionEvidenceSemanticLossStatus;
  losses?: AdapterExecutionLossArgs[];
  item_count?: number | null;
  byte_length?: number | null;
  error_code?: string | null;
  parent_digests?: string[];
  attempt_id?: string | null;
}

export interface AdapterExecutionEvidence extends JsonObject {
  schema: "bioprism-devplat-adapter-execution-evidence/0.1";
  workflow: "adapter_execution_evidence";
  group_id: string;
  domains: string[];
  subject_id: string;
  adapter_id: string;
  adapter_version: string;
  source_id: string;
  input_digest: string;
  output_digest: string | null;
  execution_status: AdapterExecutionEvidenceExecutionStatus;
  conformance_status: AdapterExecutionEvidenceConformanceStatus;
  semantic_loss_status: AdapterExecutionEvidenceSemanticLossStatus;
  losses: AdapterExecutionLossArgs[];
  item_count: number | null;
  byte_length: number | null;
  error_code: string | null;
  parent_digests: string[];
  attempt_id: string | null;
  attestation_posture: "caller_asserted";
  evidence_digest: string;
}

export interface AdapterExecutionEvidenceResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-adapter-execution-evidence/0.1";
  workflow: "adapter_execution_evidence";
  evidence: AdapterExecutionEvidence;
  adapter: AdapterDescriptorResult;
  evidence_digest: string;
  attestation_posture: "caller_asserted";
  artifact_registry: JsonObject;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
}

export interface AdapterExecutionEvidenceQueryArgs extends JsonObject {
  group_id?: string | null;
  domain?: string | null;
  subject_id?: string | null;
  adapter_id?: string | null;
  source_id?: string | null;
  execution_status?: AdapterExecutionEvidenceExecutionStatus | null;
  conformance_status?: AdapterExecutionEvidenceConformanceStatus | null;
  semantic_loss_status?: AdapterExecutionEvidenceSemanticLossStatus | null;
  after?: string | null;
  max_items?: number;
  include_artifacts?: boolean;
}

export type AdapterExecutionEvidenceJoinStatus = "unbound" | "source_bound" | "workflow_bound" | "source_and_workflow_bound" | "bound_with_missing_parents" | "parents_present_unclassified";

export interface AdapterExecutionEvidenceJoinProjection extends JsonObject {
  source_plan_digests: string[];
  intake_digests: string[];
  external_payload_digests: string[];
  workflow_reconciliation_digests: string[];
  missing_parent_digests: string[];
  unclassified_parent_digests: string[];
  source_bound: boolean;
  workflow_bound: boolean;
}

export interface AdapterExecutionEvidenceQueryRow extends JsonObject {
  row_digest: string;
  content_digest: string;
  evidence_digest: string;
  subject_id: string;
  group_id: string;
  domains: string[];
  adapter_id: string;
  adapter_version: string;
  source_id: string;
  input_digest: string;
  output_digest: string | null;
  execution_status: AdapterExecutionEvidenceExecutionStatus;
  conformance_status: AdapterExecutionEvidenceConformanceStatus;
  semantic_loss_status: AdapterExecutionEvidenceSemanticLossStatus;
  loss_count: number;
  parent_digests: string[];
  attestation_posture: "caller_asserted";
  join_status: AdapterExecutionEvidenceJoinStatus;
  joins: AdapterExecutionEvidenceJoinProjection;
  evidence_artifact?: JsonObject | null;
}

export interface AdapterExecutionEvidenceQuerySummary extends JsonObject {
  page_row_count: number;
  execution_status_counts: Record<string, number>;
  conformance_status_counts: Record<string, number>;
  semantic_loss_status_counts: Record<string, number>;
  join_status_counts: Record<string, number>;
  source_bound_rows: number;
  workflow_bound_rows: number;
  rows_with_missing_parents: number;
  output_digest_present_rows: number;
  total_loss_entries: number;
}

export interface AdapterExecutionEvidenceQueryResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-adapter-execution-evidence-query/0.1";
  workflow: "adapter_execution_evidence_query";
  filters: AdapterExecutionEvidenceQueryArgs;
  registry_generation: number;
  registry_size: number;
  rows: AdapterExecutionEvidenceQueryRow[];
  page_summary: AdapterExecutionEvidenceQuerySummary;
  next_after: string | null;
  has_more: boolean;
  query_digest: string;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  limitations: string[];
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

export interface DomainAcquisitionArgs extends JsonObject {
  group_id?: string;
  domain?: string;
  include_adapters?: boolean;
  max_groups?: number;
  max_domains?: number;
}

export interface DomainAcquisitionTransportResult extends JsonObject {
  status: "bounded_file_http" | "caller_managed_plan" | "none";
  tools: string[];
  caller_managed_tools: string[];
  bounded_connector_kinds: string[];
  caller_managed_connector_kinds: string[];
  limitations: string[];
}

export interface DomainAcquisitionInterpretationResult extends JsonObject {
  status: "native" | "python_delegated" | "mixed" | "domain_tools_only" | "unmapped";
  adapter_ids: string[];
  match_basis: string[];
  declared_conformance: Array<"parse" | "normalize" | "execute" | "stream" | "replay">;
  limitations: string[];
}

export interface DomainAcquisitionAdapterResult extends JsonObject {
  id: string;
  execution: "native" | "python_delegated";
  version: string;
  accepted_formats: string[];
  source_kinds: Array<"bytes" | "directory">;
  conformance_level: "parse" | "normalize" | "execute" | "stream" | "replay";
  optional_dependency: string | null;
  scope_dimensions: string[];
  match_basis: string[];
}

export interface DomainAcquisitionRouteResult extends JsonObject {
  group_id: string;
  domain: string;
  declared_tool_count: number;
  transport: DomainAcquisitionTransportResult;
  interpretation: DomainAcquisitionInterpretationResult;
  adapters?: DomainAcquisitionAdapterResult[];
  guarantees: string[];
  limitations: string[];
}

export interface DomainAcquisitionGroupResult extends JsonObject {
  id: string;
  status: string;
  declared_domain_count: number;
  selected_domain_count: number;
  declared_tool_count: number;
  transport_status: "bounded_file_http" | "caller_managed_plan" | "none";
  interpretation_statuses: string[];
}

export interface DomainAcquisitionCatalogueResult extends JsonObject {
  schema: "bioprism-devplat-domain-acquisition/0.1";
  workflow: "domain_acquisition_catalogue";
  catalogue_digest: string;
  adapter_registry: string;
  adapter_registry_digest: string;
  query: DomainAcquisitionArgs;
  total_group_count: number;
  selected_group_count: number;
  total_domain_count: number;
  selected_domain_count: number;
  complete: boolean;
  truncated: boolean;
  groups: DomainAcquisitionGroupResult[];
  routes: DomainAcquisitionRouteResult[];
  warnings: string[];
  guarantees: string[];
  limitations: string[];
  digest: string;
}

export interface DomainAcquisitionResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-domain-acquisition/0.1";
  workflow: "domain_acquisition_catalogue";
  catalogue: DomainAcquisitionCatalogueResult;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  does_not_claim: string[];
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

export interface HubSearchArgs extends JsonObject {
  federation: JsonObject;
  catalogs: JsonObject[];
  query: JsonObject;
  max_items?: number;
}

export type HubTrustTier = "unranked" | "exploratory" | "generated_verified" | "reviewed" | "gold";

export interface HubAuthoritativeAuthorityResult extends JsonObject {
  authority: "authoritative";
  registry: string;
}

export interface HubCarriedAuthorityResult extends JsonObject {
  authority: "carried";
  mirror: string;
  origin: string;
}

export type HubAuthorityResult = HubAuthoritativeAuthorityResult | HubCarriedAuthorityResult;

export interface HubStalenessBoundResult extends JsonObject {
  max_lag_epochs: number;
}

export type HubFreshnessResult =
  | { freshness: "authoritative" }
  | { freshness: "within_bound" | "beyond_bound"; lag: number; bound: HubStalenessBoundResult; synced_at: number }
  | { freshness: "undetermined"; bound: HubStalenessBoundResult; synced_at: number }
  | { freshness: "ahead_of_reference"; synced_at: number; reference: number };

export type HubWhyResult =
  | { why: "namespace_matched"; namespace: string }
  | { why: "keyword_matched"; keyword: string }
  | { why: "term_in_name" | "term_in_summary"; term: string }
  | { why: "tier_met"; required: HubTrustTier; observed: HubTrustTier; according_to: string }
  | { why: "dependency_matched"; on: string }
  | { why: "usable_by_a_new_dependent" };

export interface HubSearchMatchResult extends JsonObject {
  name: string;
  version: string;
  digest: string;
  summary: string;
  tier: HubTrustTier;
  authority: HubAuthorityResult;
  freshness: HubFreshnessResult;
  why: HubWhyResult[];
}

export interface HubExcludedResult extends JsonObject {
  name: string;
  version: string;
  failed: string;
}

export interface HubSearchResult extends JsonObject {
  ok: boolean;
  catalog_count: number;
  release_count: number;
  requested_limit: number | null;
  effective_limit: number;
  matches: HubSearchMatchResult[];
  match_count: number;
  excluded: HubExcludedResult[];
  excluded_count: number;
  omitted_excluded: number;
  truncated: boolean;
  guarantees: string[];
  limitations: string[];
}

export interface HubResolveArgs extends JsonObject {
  federation: JsonObject;
  catalogs: JsonObject[];
  request: JsonObject;
}

export interface HubLockArgs extends HubResolveArgs {
  max_items?: number;
}

export interface HubFreshnessPolicyResult extends JsonObject {
  require_authority: boolean;
  accept_undetermined: boolean;
  accept_beyond_bound: boolean;
  max_accepted_lag: number | null;
}

export type HubLifecycleNoteResult =
  | { note: "yanked_but_pinned"; reason: string; epoch: number }
  | { note: "deprecated"; stage: string; replacement: string; reason: string };

export interface HubResolutionSubjectResult extends JsonObject {
  name: string;
  version: string;
  digest: string;
}

export interface HubResolutionResult extends JsonObject {
  subject: HubResolutionSubjectResult;
  provenance: {
    authority: HubAuthorityResult;
    freshness: HubFreshnessResult;
    accepted_under: HubFreshnessPolicyResult;
    notes: HubLifecycleNoteResult[];
  };
}

export interface HubResolveResult extends JsonObject {
  ok: boolean;
  resolution: HubResolutionResult;
  answered_by: string;
  authoritative: boolean;
  catalog_count: number;
  guarantees: string[];
  limitations: string[];
}

export type HubVersionRequirementResult =
  | { req: "exact" | "at_least" | "compatible" | "approximately"; spec: string }
  | { req: "range"; spec: { low: string; high: string } }
  | { req: "any" };

export type HubRequirementSourceResult =
  | { source: "root" }
  | { source: "pack"; name: string; version: string };

export interface HubRequirementResult extends JsonObject {
  on: string;
  req: HubVersionRequirementResult;
  source: HubRequirementSourceResult;
}

export interface HubLockEntryResult extends JsonObject {
  name: string;
  locked: {
    resolution: HubResolutionResult;
    required_by: HubRequirementResult[];
  };
}

export interface HubLockResult extends JsonObject {
  ok: boolean;
  entry_count: number;
  fully_authoritative: boolean;
  answering_registries: string[];
  remarked_entry_count: number;
  entries: HubLockEntryResult[];
  omitted_entries: number;
  max_items: number;
  guarantees: string[];
}

export interface WorldClaimCheckArgs extends JsonObject {
  provenance: JsonObject;
  claim: JsonObject;
}

export type WorldRung = "observed" | "semi_synthetic" | "mechanistic";
export type WorldClaimKind = "the_world_as_built" | "detecting_injected_structure" | "simulator_behaviour" | "biology";

export type WorldSelectionResult =
  | { selection: "consecutive"; criterion: string }
  | { selection: "convenience"; because: string }
  | { selection: "enriched"; for_what: string }
  | { selection: "undeclared" };

export interface WorldProvenanceResult extends JsonObject {
  top: WorldRung;
  stands_on: WorldRung[];
  assumptions: string[];
  unsupported_counterfactuals: string[];
  selection: WorldSelectionResult;
}

export interface WorldClaimResult extends JsonObject {
  kind: WorldClaimKind;
  quantity: string;
  counterfactual: string | null;
  population: string | null;
}

export interface GroundedWorldClaimResult extends JsonObject {
  claim: WorldClaimResult;
  stands_on: WorldRung[];
  furthest_from_observation: WorldRung;
}

export interface SupportedWorldClaimResult extends JsonObject {
  ok: true;
  supported: true;
  claim: WorldClaimResult;
  grounded: GroundedWorldClaimResult;
  caveat: string;
  provenance: WorldProvenanceResult;
}

export interface RefusedWorldClaimResult extends JsonObject {
  ok: false;
  supported: false;
  claim: WorldClaimResult;
  refusal: string;
  provenance: WorldProvenanceResult;
  fail_closed: true;
}

export type WorldClaimCheckResult = SupportedWorldClaimResult | RefusedWorldClaimResult;

export interface ObservedWorldDeclareArgs extends JsonObject {
  id: string;
  sources: JsonObject[];
  design: JsonObject;
  outcome_labels: string[];
}

export type WorldSourceAccessResult =
  | { access: "public" }
  | { access: "controlled"; policy: string };

export interface WorldSourceResult extends JsonObject {
  name: string;
  version: string | null;
  access: WorldSourceAccessResult;
  embedded: boolean;
}

export interface WorldStratumResult extends JsonObject {
  name: string;
  count: number;
}

export interface WorldStudyDesignResult extends JsonObject {
  cohort_size: number;
  strata: WorldStratumResult[];
  selection: WorldSelectionResult;
  stands_for_population: string | null;
  unsupported_counterfactuals: string[];
}

export interface ObservedWorldResult extends JsonObject {
  id: string;
  sources: WorldSourceResult[];
  design: WorldStudyDesignResult;
  outcome_labels: string[];
}

export interface ObservedWorldDeclareResult extends JsonObject {
  ok: true;
  world: ObservedWorldResult;
  provenance: WorldProvenanceResult;
  world_id: string;
  source_count: number;
  controlled_sources: string[];
  outcome_label_count: number;
  guarantees: string[];
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

export type LiteratureBindTier = "primary" | "review" | "guideline" | "database";

export type LiteratureBindOutcomeKind = "bound" | "citable" | "cite_refused" | "refused";

export type LiteratureBindingRefusalKind =
  | "citation_laundering"
  | "unstated_population"
  | "population_mismatch"
  | "temporal_leakage"
  | "retracted_source";

export interface LiteratureBindCheckArgs extends JsonObject {
  claim: JsonObject;
  target: JsonObject;
  at_tier: LiteratureBindTier;
  horizon: JsonObject;
  flag_warrant?: string | null;
  claim_kind?: string | null;
}

export interface LiteratureBindCheckResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/literature-bind-check/0.1";
  outcome_kind: LiteratureBindOutcomeKind;
  bound: boolean;
  citable: boolean | null;
  evidence: JsonObject;
  guarantees?: string[];
  limitations?: string[];
}

export type ModalitySupportOutcomeKind = "supported" | "refused";

export interface ModalitySupportCheckArgs extends JsonObject {
  modality: string;
  claim: string;
  descriptor?: JsonObject | null;
  counted_unit?: string | null;
}

export interface ModalitySupportCheckResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/modality-support-check/0.1";
  outcome_kind: ModalitySupportOutcomeKind;
  modality: string;
  claim: string;
  supported: boolean;
  claim_requirements: JsonObject;
  support: JsonObject;
  analysis_unit: JsonObject;
  descriptor: JsonObject;
  guarantees?: string[];
  limitations?: string[];
}

export type ModalityTransportOutcomeKind = "constructed" | "refused";

export interface ModalityTransportCheckArgs extends JsonObject {
  from: string;
  to: string;
  axis: string;
  transport: JsonObject;
  source_descriptor?: JsonObject | null;
  claims?: string[];
}

export interface ModalityTransportCheckResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/modality-transport-check/0.1";
  outcome_kind: ModalityTransportOutcomeKind;
  constructed: boolean;
  from: string;
  to: string;
  axis: string;
  transport: JsonObject;
  fidelity?: JsonObject;
  loss?: JsonObject;
  scope_mapping?: JsonObject;
  scope_mapping_check?: string;
  inverse?: JsonObject;
  application: JsonObject;
  applied_descriptor?: JsonObject | null;
  claims: JsonObject[];
  transport_evidence?: JsonObject;
  guarantees?: string[];
  limitations?: string[];
}

export type ModalityComparabilityOutcomeKind = "comparable" | "blocked";

export interface ModalityComparabilityCheckArgs extends JsonObject {
  left: JsonObject;
  right: JsonObject;
  policy?: JsonObject | null;
}

export interface ModalityComparabilityCheckResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/modality-comparability-check/0.1";
  outcome_kind: ModalityComparabilityOutcomeKind;
  comparable: boolean;
  policy: JsonObject;
  check_order: string[];
  left: JsonObject;
  right: JsonObject;
  report: JsonObject;
  verdict: JsonObject;
  report_sha256: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface LineageAuditArgs extends JsonObject {
  registry: JsonObject;
  max_items?: number;
}

export type LineageFingerprintState = "consistent" | "mismatch" | "no_evidence_available";

export interface LineageFingerprintResult extends JsonObject {
  fingerprint: LineageFingerprintState;
  specimen: string;
  declared_donor?: string;
  fingerprint_donor?: string;
}

export interface LineageFindingResult extends JsonObject {
  finding: "lineage_cycle" | "mass_not_conserved" | "temporal_implausibility" | "duplicate_content" | "identity_mismatch" | "artifacts_disagree";
  specimen?: string;
  child?: string;
  parent?: string;
  left?: string;
  right?: string;
  parent_mass_ug?: number;
  child_total_ug?: number;
  artifacts?: string[];
  fingerprint?: LineageFingerprintResult;
}

export interface LineageAuditResult extends JsonObject {
  ok: boolean;
  specimen_count: number;
  artifact_count: number;
  finding_count: number;
  clean: boolean;
  identity_complete: boolean;
  fingerprint_count: number;
  fingerprints: LineageFingerprintResult[];
  omitted_fingerprints: number;
  unchecked_identity_count: number;
  unchecked_identity: string[];
  finding_count_returned: number;
  findings: LineageFindingResult[];
  omitted_findings: number;
  guarantees: string[];
  limitations: string[];
}

export interface PreanalyticApplyArgs extends JsonObject {
  specimen: JsonObject;
  mutation: JsonObject;
  available_actions?: string[];
  family?: JsonObject[];
  family_name?: string;
  qc_field?: string;
  alert_at?: number;
}

export interface PreanalyticCheckResult extends JsonObject {
  ok: boolean;
  family?: string;
  refusal?: string;
  fail_closed?: boolean;
}

export interface PreanalyticDetectabilityResult extends JsonObject {
  qc_field: string;
  alert_at: number;
  intensity: number;
}

export interface PreanalyticFaultedResult extends JsonObject {
  mutation: string;
  specimen: JsonObject;
  qc_signature: Record<string, number>;
  measurability_lost: Record<string, number>;
  stage: string;
}

export interface PreanalyticApplyResult extends JsonObject {
  ok: boolean;
  applied: boolean;
  mutation: JsonObject;
  stage?: string;
  faulted?: PreanalyticFaultedResult;
  biology_digest_before: string;
  biology_digest_after?: string;
  biology_unchanged?: boolean;
  specimen_digest_before: string;
  specimen_digest_after?: string;
  has_signature?: boolean;
  response_check?: PreanalyticCheckResult | null;
  family_validation?: PreanalyticCheckResult | null;
  detectability?: PreanalyticDetectabilityResult | null;
  refusal?: string;
  fail_closed: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface ContradictionReviewArgs extends JsonObject {
  left: JsonObject;
  right: JsonObject;
  intent: "expected" | "resolvable" | "irreducible";
  hypotheses: JsonObject[];
  actions?: JsonObject[];
  missing_evidence?: JsonObject[];
  references?: JsonObject[];
  examine?: string[];
  notable_below_per_ten_thousand?: number;
  max_items?: number;
}

export type ContradictionStateName = "resolved" | "not_yet_examined" | "unresolvable";

export interface ContradictionRankedActionResult extends JsonObject {
  evidence: string;
  refutes?: string[];
  refutes_live?: number;
  cost: number;
}

export interface ContradictionStateResult extends JsonObject {
  state: ContradictionStateName;
  available?: ContradictionRankedActionResult[];
  examined?: string[];
  would_resolve?: JsonObject[];
  by?: string[];
  surviving?: JsonObject;
}

export interface ContradictionExpectednessResult extends JsonObject {
  ok: boolean;
  value?: JsonObject;
  threshold: number;
  refusal?: string;
  fail_closed?: boolean;
}

export interface ContradictionReviewResult extends JsonObject {
  ok: boolean;
  validated?: boolean;
  stage?: string;
  refusal?: string;
  fail_closed: boolean;
  contradiction?: JsonObject;
  intent?: "expected" | "resolvable" | "irreducible";
  declared_hypothesis_count?: number;
  admissible_hypothesis_count?: number;
  admissible_hypotheses?: Record<string, JsonObject>;
  validation_intent_check?: JsonObject;
  post_examination_intent_check?: JsonObject;
  examined?: string[];
  state?: ContradictionStateResult;
  state_name?: ContradictionStateName;
  live_hypothesis_count?: number;
  next_actions?: ContradictionRankedActionResult[];
  omitted_next_actions?: number;
  cue_count?: number;
  cues?: JsonObject[];
  omitted_cues?: number;
  expectedness?: ContradictionExpectednessResult | null;
  guarantees?: string[];
  limitations?: string[];
}

export interface LabPlanArgs extends JsonObject {
  graph: JsonObject;
  actions: JsonObject[];
  budget: JsonObject;
  marginal_value_floor?: number;
  hypotheses?: JsonObject;
  observations?: JsonObject;
  max_items?: number;
}

export interface LabPlanResult extends JsonObject {
  ok: boolean;
  goal?: string;
  obligation_count?: number;
  frontier?: JsonObject[];
  omitted_frontier?: number;
  separation?: JsonObject | null;
  ordered?: JsonObject[];
  omitted_ordered?: number;
  excluded?: JsonValue[];
  omitted_excluded?: number;
  spent?: JsonObject;
  stop?: JsonObject;
  should_escalate?: boolean;
  stage?: string;
  refusal?: string;
  fail_closed: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export type ObligationGateOutcomeKind = "allowed" | "blocked";

export interface ObligationGateCheckArgs extends JsonObject {
  graph: JsonObject;
  action: JsonObject;
  max_items?: number;
}

export interface ObligationGateCheckResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-mcp/obligation-gate-check/0.1";
  outcome_kind: ObligationGateOutcomeKind;
  allowed: boolean;
  goal: string;
  action: JsonObject;
  gate: JsonObject;
  refusal: JsonObject | null;
  graph: JsonObject;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoBoundaryArgs extends JsonObject {
  request: JsonObject;
  boundary?: JsonObject;
}

export interface OncoEscalationResult extends JsonObject {
  trigger: string;
  route: string;
}

export interface OncoDispositionResult extends JsonObject {
  disposition: "release_in_full" | "release_partial" | "refuse_and_escalate";
  uses?: string[];
  released?: string[];
  refused?: string[];
  escalation?: OncoEscalationResult;
}

export interface OncoBoundaryResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/onco-boundary-check/0.1";
  outcome_kind?: "disposition" | "refused";
  disposition_kind?: OncoDispositionResult["disposition"];
  permitted?: string[];
  disposition?: OncoDispositionResult;
  released?: string[];
  refused?: string[];
  terminal_action?: "stop" | "abstain" | "escalate";
  escalation?: OncoEscalationResult | null;
  escalation_present?: boolean;
  escalation_trigger?: string | null;
  escalation_route?: string | null;
  requested_use_count?: number;
  released_count?: number;
  refused_count?: number;
  identifier_fields_present?: boolean;
  research_statement?: string;
  stage?: string;
  refusal?: string;
  refusal_kind?: "identifiers_present";
  fail_closed: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoResponseAssessArgs extends JsonObject {
  criterion: JsonObject;
  baseline: JsonObject;
  current: JsonObject;
  current_acquired: string;
  baseline_clinical: JsonObject;
  current_clinical: JsonObject;
  treatment: JsonObject;
  evidence?: JsonObject;
  nadir_spd_mm2?: number;
  measurement_error_fraction?: number;
}

export interface OncoResponseResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/onco-response-assess/0.1";
  outcome_kind?: "assessment" | "refused";
  call_kind?: "complete" | "partial" | "stable" | "progression" | "not_evaluable";
  unconfirmed_reading?: "complete" | "partial" | "stable" | "progression";
  criterion?: JsonObject;
  treatment?: JsonObject;
  criterion_recognises_post_treatment_change?: boolean;
  post_treatment_window_days?: number;
  pseudoresponse_possible?: boolean;
  measurement_error_fraction?: number;
  evidence_present?: boolean;
  criterion_divergence_present?: boolean;
  sensitivity_flips?: boolean;
  hypothesis_non_identifiable?: boolean;
  assessment?: JsonObject;
  call_label?: string;
  withheld_progression?: boolean;
  hypothesis_count?: number;
  evidence_requests?: JsonValue[];
  stage?: string;
  refusal?: string;
  refusal_kind?: "assessment_error";
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldlineViewArgs extends JsonObject {
  worldline: JsonObject;
  visible_at?: string;
}

export type OncoWorldlineClockAxis = "acquired" | "recorded" | "released" | "visible";

export type OncoWorldlineVisibilityState = "visible" | "hidden_from_agent" | "not_filtered";

export interface OncoWorldlineClocksResult extends JsonObject {
  acquired: string;
  recorded: string;
  released: string;
  visible: string;
}

export interface OncoWorldlineTimepointResult extends JsonObject {
  label: string;
  biological_index: number;
  record_index: number;
  clocks: OncoWorldlineClocksResult;
  acquired?: string;
  recorded?: string;
  released?: string;
  visible?: string;
  days_from_baseline: number;
  observation: JsonObject;
  visibility_state: OncoWorldlineVisibilityState;
  visible_at_cutoff: boolean | null;
}

export interface OncoWorldlineVisibilityPartitionResult extends JsonObject {
  cutoff: string | null;
  filter_applied: boolean;
  visible: string[] | null;
  hidden: string[] | null;
  visible_count: number | null;
  hidden_count: number | null;
}

export interface OncoWorldlineResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/onco-worldline-view/0.1";
  subject?: string;
  baseline?: string;
  timepoint_count?: number;
  biological_order?: string[];
  record_order?: string[];
  record_order_differs?: boolean;
  clock_axes?: OncoWorldlineClockAxis[];
  clock_order_guaranteed?: boolean;
  baseline_biological_index?: number;
  baseline_record_index?: number;
  visibility_cutoff?: string | null;
  visibility_filter_applied?: boolean;
  visible_timepoints?: string[] | null;
  hidden_from_agent?: string[] | null;
  visibility_partition?: OncoWorldlineVisibilityPartitionResult;
  visible_count?: number | null;
  hidden_count?: number | null;
  timepoints?: OncoWorldlineTimepointResult[];
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoClassificationArgs extends JsonObject {
  histology: JsonValue;
  panel: JsonObject;
}

export type OncoClassificationResolutionKind = "integrated" | "provisional" | "unresolved" | "mixed" | "not_otherwise_resolved";

export type OncoClassificationMarker = "idh_mutation" | "codeletion1p19q" | "tert_promoter_mutation" | "egfr_amplification" | "chromosome7_gain10_loss" | "cdkn2a_cdkn2b_homozygous_deletion" | "h3k27_alteration" | "h3g34_mutation";

export type OncoClassificationMarkerCall = "present" | "absent";

export type OncoClassificationObservationStatus = "missing" | "not_collected" | "technically_failed" | "below_detection" | "not_applicable" | "redacted";

export type OncoClassificationMarkerObservationResult =
  | ({ value: OncoClassificationMarkerCall } & JsonObject)
  | ({ unobserved: OncoClassificationObservationStatus } & JsonObject);

export interface OncoClassificationPanelStateResult extends JsonObject {
  marker: OncoClassificationMarker;
  state: OncoClassificationMarkerObservationResult;
}

export interface OncoClassificationObligationResult extends JsonObject {
  marker: OncoClassificationMarker;
  role: "required" | "supportive" | "exclusionary";
  state: OncoClassificationMarkerObservationResult;
  discriminates: number;
}

export interface OncoClassificationSatisfiedEvidenceResult extends JsonObject {
  marker: OncoClassificationMarker;
  role: "required" | "supportive" | "exclusionary";
  call: OncoClassificationMarkerCall;
}

export type OncoClassificationResolutionResult =
  | ({ resolution: "integrated"; entity: string; grade: OncoClassificationMarkerObservationResult; evidence: OncoClassificationSatisfiedEvidenceResult[] } & JsonObject)
  | ({ resolution: "provisional"; candidate: string; obligations: OncoClassificationObligationResult[] } & JsonObject)
  | ({ resolution: "unresolved"; candidates: string[]; obligations: OncoClassificationObligationResult[] } & JsonObject)
  | ({ resolution: "mixed"; candidates: string[] } & JsonObject)
  | ({ resolution: "not_otherwise_resolved"; histology: "diffuse_glioma" | "outside_implemented_scope"; excluded: string[] } & JsonObject);

export interface OncoClassificationResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/onco-classification-check/0.1";
  histology?: JsonValue;
  resolution?: OncoClassificationResolutionResult;
  resolution_kind?: OncoClassificationResolutionKind;
  is_integrated?: boolean;
  entity?: string | null;
  obligations?: OncoClassificationObligationResult[];
  obligation_count?: number;
  panel_states?: OncoClassificationPanelStateResult[];
  panel_state_count?: number;
  observed_panel_state_count?: number;
  unobserved_panel_state_count?: number;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoIdentityJoinArgs extends JsonObject {
  left: JsonObject;
  right: JsonObject;
  unit: "participant" | "lesion" | "specimen" | "imaging_series";
  evidence?: JsonObject;
  epoch_bridge?: JsonObject;
}

export type OncoIdentityJoinRefusalKind = "different_participant" | "truncated_identifier" | "different_lesion" | "incompatible_epoch" | "different_specimen" | "no_identity_evidence" | "unlicensed_relation" | "undeclared_permissible_use" | "no_regional_provenance" | "incomparable_coordinates";

export type OncoIdentityJoinVerdictResult =
  | ({ verdict: "joinable" } & JsonObject)
  | ({ verdict: "declined"; reason: { refusal: OncoIdentityJoinRefusalKind } & JsonObject } & JsonObject);

export interface OncoIdentityJoinReportResult extends JsonObject {
  left: string;
  right: string;
  unit: "participant" | "lesion" | "specimen" | "imaging_series";
  verdict: OncoIdentityJoinVerdictResult;
}

export interface OncoIdentityJoinResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-identity-join/0.1";
  joinable?: boolean;
  report?: OncoIdentityJoinReportResult;
  verdict_kind?: "joinable" | "declined";
  refusal_kind?: OncoIdentityJoinRefusalKind | null;
  bridge_declared?: boolean;
  epoch_bridge?: JsonObject | null;
  identity_evidence_present?: boolean;
  identity_link_count?: number;
  bridge_warrant_present?: boolean;
  checked_dimensions?: string[];
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoOutcomeAnalyzeArgs extends JsonObject {
  follow_up: JsonObject;
  estimand: JsonObject;
}

export type OncoOutcomeEndpoint = "overall_survival" | "progression_free_survival" | "time_to_progression" | "time_to_treatment_failure";

export type OncoOutcomePopulation = "intention_to_treat" | "per_protocol" | "evaluable_for_response";

export type OncoOutcomeEventKind = "death" | "confirmed_progression" | "progression_or_death" | "treatment_failure";

export type OncoOutcomeCensoringReason = "administrative_cutoff" | "lost_to_follow_up" | "withdrew_consent" | "event_free_at_last_contact" | "competing_death" | "subsequent_therapy";

export type OncoOutcomeBias = "left_truncation" | "informative_loss_to_follow_up" | "competing_death" | "treatment_switching";

export interface OncoOutcomeEstimandResult extends JsonObject {
  endpoint: OncoOutcomeEndpoint;
  population: OncoOutcomePopulation;
  variable: string;
  summary_measure: JsonValue;
  intercurrent_event_strategies: [string, string][];
  censoring_assumption: JsonValue;
}

export type OncoOutcomeCensoredResult =
  | ({ outcome: "censored"; administrative_cutoff: null } & JsonObject)
  | ({ outcome: "censored"; lost_to_follow_up: null } & JsonObject)
  | ({ outcome: "censored"; withdrew_consent: null } & JsonObject)
  | ({ outcome: "censored"; event_free_at_last_contact: null } & JsonObject)
  | ({ outcome: "censored"; competing_death: null } & JsonObject)
  | ({ outcome: "censored"; subsequent_therapy: null } & JsonObject)
  | ({ outcome: "censored"; reason: OncoOutcomeCensoringReason } & JsonObject);

export type OncoOutcomeOutcomeResult =
  | ({ outcome: "event"; kind: OncoOutcomeEventKind } & JsonObject)
  | OncoOutcomeCensoredResult;

export interface OncoOutcomeAnalysisResult extends JsonObject {
  subject: string;
  estimand: OncoOutcomeEstimandResult;
  at_risk_days: number;
  immortal_time_days: number;
  outcome: OncoOutcomeOutcomeResult;
  bias_flags: OncoOutcomeBias[];
}

export interface OncoOutcomeResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/onco-outcome-analyze/0.1";
  analysis?: OncoOutcomeAnalysisResult;
  outcome?: OncoOutcomeOutcomeResult;
  bias_flags?: OncoOutcomeBias[];
  bias_count?: number;
  informative_bias_count?: number;
  at_risk_days?: number;
  immortal_time_days?: number;
  left_truncated?: boolean;
  event?: boolean;
  censoring_reason?: string | null;
  censoring_informative?: boolean | null;
  informative_bias_flags?: OncoOutcomeBias[];
  guarantees?: string[];
  limitations?: string[];
}

export interface OracleRefResult extends JsonObject {
  id: string;
  version: { major: number; minor: number; patch: number };
}

export interface OracleJudgementResult extends JsonObject {
  oracle: OracleRefResult;
  tier: "deterministic" | "execution" | "property" | "statistical" | "judge";
  declared_tier: "deterministic" | "execution" | "property" | "statistical" | "judge";
  position: "supported" | "contradicted" | "unresolved" | "not_evaluable";
  confidence: number;
  belief?: JsonObject | null;
  establishes: string[];
  cannot_establish: string[];
  findings: JsonValue[];
  admissibility: JsonObject;
  rationale: string;
}

export type OracleOverrideRule = "nondeterministic_over_grounded" | "lower_tier_over_higher";

export interface OracleSuppressedOverrideResult extends JsonObject {
  oracle: OracleRefResult;
  attempted_position: "supported" | "contradicted" | "unresolved" | "not_evaluable";
  attempted_tier: "deterministic" | "execution" | "property" | "statistical" | "judge";
  attempted_confidence: number;
  deciding_tier: "deterministic" | "execution" | "property" | "statistical" | "judge";
  deciding_positions: ("supported" | "contradicted" | "unresolved" | "not_evaluable")[];
  rule: OracleOverrideRule;
}

export type OracleDisagreementSourceResult =
  | { source: "version_mismatch"; id: string; versions: string[] }
  | { source: "scope_mismatch"; planes: Record<string, string[]> }
  | { source: "independence_violation"; circular: OracleRefResult[] }
  | { source: "genuine_ambiguity" };

export type OracleSettlementResult =
  | { settlement: "higher_tier_oracle"; at_least: "deterministic" | "execution" | "property" | "statistical" | "judge" }
  | { settlement: "version_alignment"; id: string }
  | { settlement: "independent_review"; reason: string }
  | { settlement: "artifact_repair"; pointer: string }
  | { settlement: "longitudinal_observation"; awaiting: string };

export type OracleResolutionResult =
  | { resolution: "open" }
  | { resolution: "upheld"; by: OracleRefResult; at: string; position: string }
  | { resolution: "overturned"; by: OracleRefResult; at: string; position: string; superseded: string[] }
  | { resolution: "unresolvable"; reason: string };

export interface OracleDisagreementResult extends JsonObject {
  tier: "deterministic" | "execution" | "property" | "statistical" | "judge";
  positions: Record<string, OracleRefResult[]>;
  source: OracleDisagreementSourceResult;
  would_be_settled_by: OracleSettlementResult[];
  resolution: OracleResolutionResult;
}

export type OracleBasisResult =
  | { basis: "decided"; tier: "deterministic" | "execution" | "property" | "statistical" | "judge" }
  | { basis: "no_admissible_oracle" }
  | { basis: "no_applicable_oracle" }
  | { basis: "below_policy_floor"; best: string; required: string };

export interface OracleConfidenceResult extends JsonObject {
  low: number;
  high: number;
}

export interface OracleCombineArgs extends JsonObject {
  subject: string;
  at: string;
  judgements: JsonObject[];
  minimum_deciding_tier?: "deterministic" | "execution" | "property" | "statistical" | "judge";
  max_items?: number;
}

export interface OracleCombineResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oracle-combine/0.1";
  subject?: string;
  at?: string;
  status?: "valid" | "invalid" | "underdetermined";
  underdetermined?: boolean;
  deciding_tier?: "deterministic" | "execution" | "property" | "statistical" | "judge" | null;
  judge_only?: boolean;
  suppressed_override?: boolean;
  acceptable?: boolean;
  basis?: OracleBasisResult | null;
  confidence?: OracleConfidenceResult | null;
  establishes?: string[];
  does_not_establish?: string[];
  contributing?: OracleJudgementResult[];
  omitted_contributing?: number;
  withheld?: OracleJudgementResult[];
  omitted_withheld?: number;
  inadmissible?: OracleJudgementResult[];
  omitted_inadmissible?: number;
  suppressed?: OracleSuppressedOverrideResult[];
  omitted_suppressed?: number;
  disagreements?: OracleDisagreementResult[];
  omitted_disagreements?: number;
  guarantees?: string[];
  limitations?: string[];
}

export interface OracleReferencePanelArgs extends JsonObject {
  panel: JsonObject;
  rule?: JsonObject;
  model_call?: string;
  max_items?: number;
}

export interface OracleReferencePanelResult extends JsonObject {
  ok: boolean;
  rule?: JsonValue;
  rule_label?: string;
  consensus?: JsonObject;
  tally?: JsonObject;
  readers?: number;
  minority_calls?: JsonValue[];
  reads?: JsonValue[];
  omitted_reads?: number;
  per_reader?: JsonObject | null;
  model_call?: string | null;
  adjudication?: JsonObject | null;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OracleMissingnessArgs extends JsonObject {
  pattern: JsonObject;
  field: JsonObject;
  boundary: JsonObject;
  small_cell_floor: number;
  mechanism?: JsonObject;
}

export interface OracleMissingnessResult extends JsonObject {
  ok: boolean;
  groups?: JsonValue[];
  informativeness?: JsonObject;
  field?: JsonObject;
  boundary?: JsonObject;
  small_cell_floor?: number;
  egress?: JsonObject;
  mechanism?: JsonObject | null;
  complete_case?: JsonObject | null;
  guarantees?: string[];
  limitations?: string[];
}

export interface BioevalReferenceAuditArgs extends JsonObject {
  reference: JsonObject;
  state?: string;
}

export interface BioevalReferenceResult extends JsonObject {
  standard: "distribution" | "unresolved" | "not_evaluable";
  mass?: Record<string, number>;
  dispersion?: JsonObject;
  reason?: string;
}

export type BioevalResolutionResult =
  | { resolution: "categorical" }
  | { resolution: "distributed"; modal_mass: number };

export interface BioevalReferenceAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-reference-audit/0.1";
  reference?: BioevalReferenceResult;
  reference_kind?: string;
  can_certify_clean_pass?: boolean;
  resolution?: BioevalResolutionResult | null;
  modal_state?: string | null;
  modal_mass?: number | null;
  modal_confidence?: number | null;
  entropy_bits?: number | null;
  dispersion?: "aleatoric" | "annotation_error" | "mixed" | "unattributed" | null;
  queried_state?: string | null;
  queried_state_mass?: number | null;
  guarantees?: string[];
  limitations?: string[];
}

export interface BioevalAcquisitionObligationArgs extends JsonObject {
  id: string;
  required: boolean;
}

export type BioevalAcquisitionKind = "retrieval" | "assay" | "metadata" | "expert" | "analysis";

export interface BioevalAcquisitionActionArgs extends JsonObject {
  id: string;
  kind: BioevalAcquisitionKind;
  cost: number;
  closes?: string[];
}

export interface BioevalAcquisitionReferencePolicyArgs extends JsonObject {
  name: string;
  cost: number;
  admissible: boolean;
}

export interface BioevalAcquisitionAuditArgs extends JsonObject {
  obligations: BioevalAcquisitionObligationArgs[];
  actions: BioevalAcquisitionActionArgs[];
  stopped_after?: boolean;
  reference_policy?: BioevalAcquisitionReferencePolicyArgs | null;
  require_reference?: boolean;
}

export interface BioevalAcquisitionAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-acquisition-audit/0.1";
  workflow?: "bioeval_acquisition_audit";
  status?: "admissible" | "stopped_inadmissible" | "open";
  stopped_after?: boolean;
  admissible?: boolean;
  obligations?: JsonObject[];
  open_obligations?: JsonObject[];
  actions?: JsonObject[];
  action_count?: number;
  cost?: number;
  cost_by_kind?: JsonObject[];
  findings?: JsonObject;
  reference_policy?: JsonObject | null;
  regret?: JsonObject | null;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface BioevalGroundingClaimArgs extends JsonObject {
  id: string;
}

export type BioevalGroundingLocator = "resolved" | "not_checked" | "unresolvable";

export interface BioevalGroundingLocatorStatusArgs extends JsonObject {
  locator: BioevalGroundingLocator;
  digest?: string;
  detail?: string;
}

export interface BioevalGroundingEvidenceArgs extends JsonObject {
  id: string;
  last_modified: string;
  lineage?: string[];
  locator_status?: BioevalGroundingLocatorStatusArgs;
}

export type BioevalGroundingEdgeKind = "supports" | "contradicts" | "adjacent";

export interface BioevalGroundingEdgeArgs extends JsonObject {
  claim: string;
  evidence: string;
  kind: BioevalGroundingEdgeKind;
}

export interface BioevalGroundingAuditArgs extends JsonObject {
  claims: BioevalGroundingClaimArgs[];
  evidence: BioevalGroundingEvidenceArgs[];
  edges: BioevalGroundingEdgeArgs[];
  stale_against?: string;
  max_items?: number;
}

export interface BioevalGroundingAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-grounding-audit/0.1";
  workflow?: "bioeval_grounding_audit";
  claims?: JsonObject;
  evidence?: JsonObject;
  edges?: JsonObject;
  census?: JsonObject;
  graph?: JsonObject;
  locator_census?: JsonObject;
  staleness?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface BioevalEstimandArgs extends JsonObject {
  intervention: string;
  comparator: string;
  unit: string;
  outcome: string;
  horizon: string;
  scope: string;
}

export type BioevalClaimKind = "association" | "intervention";
export type BioevalEvidentiaryKind = "model_conditional" | "observational" | "experimental";

export interface BioevalBasisArgs extends JsonObject {
  evidentiary: BioevalEvidentiaryKind;
  model?: string;
  dataset?: string;
  study?: string;
}

export type BioevalIdentificationState = "not_assessed" | "declared" | "probed";

export interface BioevalIdentificationCheckArgs extends JsonObject {
  name: string;
  passed: boolean;
  detail: string;
}

export interface BioevalIdentificationArgs extends JsonObject {
  identification: BioevalIdentificationState;
  strategy?: string;
  assumptions?: string[];
  checks?: BioevalIdentificationCheckArgs[];
}

export interface BioevalCorroborationArgs extends JsonObject {
  source: string;
  kind: BioevalClaimKind;
  detail: string;
}

export interface BioevalTransportRequestArgs extends JsonObject {
  target: string;
  declared_scopes: string[];
}

export interface BioevalEstimandAuditArgs extends JsonObject {
  estimand: BioevalEstimandArgs;
  kind: BioevalClaimKind;
  basis: BioevalBasisArgs;
  identification?: BioevalIdentificationArgs | null;
  corroborations?: BioevalCorroborationArgs[];
  transport_requests?: BioevalTransportRequestArgs[];
  require_identification?: boolean;
  require_corroboration?: boolean;
  strict_transport?: boolean;
}

export interface BioevalEstimandAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-estimand-audit/0.1";
  workflow?: "bioeval_estimand_audit";
  estimand?: JsonObject;
  claim?: JsonObject;
  policies?: JsonObject;
  transport?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalEvaluatorHealthState = "healthy" | "timed_out" | "errored" | "fixture_broken";
export type BioevalEvaluatorTaskOutcome = "met" | "not_met" | "inapplicable";

export interface BioevalEvaluatorHealthArgs extends JsonObject {
  health: BioevalEvaluatorHealthState;
  after?: string;
  detail?: string;
}

export interface BioevalEvaluatorDiagnosticArgs extends JsonObject {
  command: string;
  exit_state: string;
  diff: string;
  logs?: string[];
  hidden_data_access?: string[];
}

export interface BioevalEvaluatorRunArgs extends JsonObject {
  evaluator: string;
  health: BioevalEvaluatorHealthArgs;
  reached?: BioevalEvaluatorTaskOutcome | null;
  diagnostic?: BioevalEvaluatorDiagnosticArgs;
}

export interface BioevalEvaluatorAuditArgs extends JsonObject {
  runs: BioevalEvaluatorRunArgs[];
  require_task_evidence?: boolean;
  fail_on_hidden_data?: boolean;
  max_items?: number;
}

export interface BioevalEvaluatorAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-evaluator-audit/0.1";
  workflow?: "bioeval_evaluator_audit";
  runs?: JsonObject;
  panel?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalPlaneTier = "fixed_input_model" | "workflow_pipeline" | "tool_using_agent" | "human_in_the_loop" | "multi_agent_molecule";

export interface BioevalPlaneDimensionArgs extends JsonObject {
  id: string;
  required: BioevalPlaneTier;
  weight: number;
}

export interface BioevalPlaneScoredCellArgs extends JsonObject {
  state: "scored";
  score: number;
}

export interface BioevalPlaneUnscoredCellArgs extends JsonObject {
  state: "unscored";
  reason: "not_attempted" | "evaluator_unhealthy" | "no_reference_standard" | "sealed";
  evaluator?: string;
  note?: string;
  registration?: string;
}

export interface BioevalPlaneInapplicableCellArgs extends JsonObject {
  state: "inapplicable";
  required: BioevalPlaneTier;
  declared: BioevalPlaneTier;
}

export type BioevalPlaneCellArgs = BioevalPlaneScoredCellArgs | BioevalPlaneUnscoredCellArgs | BioevalPlaneInapplicableCellArgs;

export interface BioevalScorePlaneArgs extends JsonObject {
  system: string;
  tier: BioevalPlaneTier;
  dimensions: BioevalPlaneDimensionArgs[];
  cells: Record<string, BioevalPlaneCellArgs>;
}

export interface BioevalPlaneAuditArgs extends JsonObject {
  plane: BioevalScorePlaneArgs;
  max_items?: number;
  require_fold?: boolean;
}

export interface BioevalPlaneAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-plane-audit/0.1";
  workflow?: "bioeval_plane_audit";
  plane?: JsonObject;
  dimensions?: JsonObject;
  findings?: JsonObject;
  fold?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalMetamorphicDirection = "increase" | "decrease";
export type BioevalMetamorphicRelationArgs = "invariant" | { directional_change: { expected: BioevalMetamorphicDirection } };

export interface BioevalMetamorphicResponseArgs extends JsonObject {
  response: "unchanged" | "moved" | "incomparable";
  direction?: BioevalMetamorphicDirection;
}

export interface BioevalMetamorphicTrialArgs extends JsonObject {
  id: string;
  relation: BioevalMetamorphicRelationArgs;
  response: BioevalMetamorphicResponseArgs;
}

export interface BioevalMetamorphicFamilyArgs extends JsonObject {
  id: string;
  relation: BioevalMetamorphicRelationArgs;
  trials: BioevalMetamorphicTrialArgs[];
}

export interface BioevalMetamorphicAuditArgs extends JsonObject {
  families: BioevalMetamorphicFamilyArgs[];
  max_items?: number;
  require_both_relations?: boolean;
  fail_on_undetermined?: boolean;
}

export interface BioevalMetamorphicAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-metamorphic-audit/0.1";
  workflow?: "bioeval_metamorphic_audit";
  suite?: JsonObject;
  families?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalWaiverGateKind =
  | "safety_veto"
  | "benchmark_health"
  | "capability_floor"
  | "non_inferiority"
  | "required_improvement"
  | "cost_ceiling"
  | "confidence_requirement"
  | "maximum_unknown_rate";

export interface BioevalWaiverGateVerdictArgs extends JsonObject {
  verdict: "met" | "violated" | "unevaluable";
  detail?: string;
  missing?: string;
}

export interface BioevalWaiverGateArgs extends JsonObject {
  id: string;
  kind: BioevalWaiverGateKind;
  verdict: BioevalWaiverGateVerdictArgs;
}

export interface BioevalWaiverArgs extends JsonObject {
  gate: string;
  authoriser: string;
  rationale: string;
  expiry: string;
  affected_versions: string[];
  follow_up: string;
}

export interface BioevalWaiverAuditArgs extends JsonObject {
  version: string;
  at: string;
  gates: BioevalWaiverGateArgs[];
  waivers?: BioevalWaiverArgs[];
  max_items?: number;
  require_releasable?: boolean;
  require_no_unevaluable?: boolean;
}

export interface BioevalWaiverAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-waiver-audit/0.1";
  workflow?: "bioeval_waiver_audit";
  release?: JsonObject;
  gates?: JsonObject;
  waivers?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalDesignConclusion =
  | "pass"
  | "unsupported_pass"
  | "contradicted_pass"
  | "partial_credit"
  | "fail"
  | "vetoed"
  | "disputed"
  | "justification_unexamined"
  | "unknown"
  | "abstained";
export type BioevalDesignTier = "judge" | "statistical" | "property" | "execution" | "deterministic";

export interface BioevalDesignArmArgs extends JsonObject {
  id: string;
  levels: Record<string, string>;
  conclusion: BioevalDesignConclusion;
  tier: BioevalDesignTier;
}

export interface BioevalDesignAuditArgs extends JsonObject {
  cell_id: string;
  factors: string[];
  baseline: string;
  arms: BioevalDesignArmArgs[];
  controlled?: boolean;
  max_items?: number;
  require_contrasts?: boolean;
  require_complete_interactions?: boolean;
  require_attribution?: boolean;
}

export interface BioevalDesignAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-design-audit/0.1";
  workflow?: "bioeval_design_audit";
  design?: JsonObject;
  arms?: JsonObject;
  contrasts?: JsonObject;
  interactions?: JsonObject;
  attributions?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalMeshEvaluatorKind =
  | "deterministic_property"
  | "executable_analysis"
  | "metamorphic_relation"
  | "statistical_reference"
  | "prospective_reveal"
  | "expert_review"
  | "calibrated_model_judge";

export interface BioevalMeshEvaluatorArgs extends JsonObject {
  id: string;
  kind: BioevalMeshEvaluatorKind;
  inputs?: string[];
  derived_from?: string[];
}

export interface BioevalMeshVerdictArgs extends JsonObject {
  evaluator: string;
  position?: string;
  abstained?: boolean;
}

export interface BioevalMeshAuditArgs extends JsonObject {
  system_artifacts?: string[];
  evaluators: BioevalMeshEvaluatorArgs[];
  verdicts?: BioevalMeshVerdictArgs[];
  expected?: string;
  max_items?: number;
  require_independence?: boolean;
  require_independent_ratings?: boolean;
}

export interface BioevalMeshAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-mesh-audit/0.1";
  workflow?: "bioeval_mesh_audit";
  mesh?: JsonObject;
  evaluators?: JsonObject;
  classes?: JsonObject;
  verdicts?: JsonObject;
  disagreements?: JsonObject;
  independent_ratings?: JsonObject;
  contributions?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalBurdenResourceClass =
  | "tissue_aliquot"
  | "viable_cells"
  | "assay_capacity"
  | "expert_time"
  | "participant_burden"
  | "privacy_access"
  | "compute_and_money";

export type BioevalBurdenDrawOutcome = "productive" | "wasted";

export interface BioevalBurdenResourceArgs extends JsonObject {
  id: string;
  class: BioevalBurdenResourceClass;
  initial: number;
  unit: string;
}

export interface BioevalBurdenBranchArgs extends JsonObject {
  id: string;
  parent?: string;
}

export interface BioevalBurdenDrawArgs extends JsonObject {
  branch: string;
  action: string;
  resource: string;
  amount: number;
  unit: string;
  outcome?: BioevalBurdenDrawOutcome;
  destructive?: boolean;
}

export interface BioevalBurdenAuditArgs extends JsonObject {
  root: string;
  resources: BioevalBurdenResourceArgs[];
  branches?: BioevalBurdenBranchArgs[];
  draws?: BioevalBurdenDrawArgs[];
  inspect_branches?: string[];
  joint_branches?: string[];
  max_items?: number;
  require_joint_feasible?: boolean;
  require_no_wasted_nonrenewable?: boolean;
}

export interface BioevalBurdenAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-burden-audit/0.1";
  workflow?: "bioeval_burden_audit";
  burden?: JsonObject;
  resources?: JsonObject;
  branches?: JsonObject;
  draws?: JsonObject;
  joint_feasibility?: JsonObject;
  wasted_nonrenewable?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface BioevalRevealCommitmentArgs extends JsonObject {
  target: string;
  prediction: JsonValue;
  analysis_plan: string;
}

export interface BioevalRevealOutcomeArgs extends JsonObject {
  target: string;
  observed: JsonValue;
}

export interface BioevalRevealAuditArgs extends JsonObject {
  study: string;
  commitments: BioevalRevealCommitmentArgs[];
  rubric: JsonValue;
  sealed_at: string;
  outcomes?: BioevalRevealOutcomeArgs[];
  score_rubric?: JsonValue;
  require_scoring?: boolean;
  require_rubric_match?: boolean;
  require_complete?: boolean;
}

export interface BioevalRevealAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-reveal-audit/0.1";
  workflow?: "bioeval_reveal_audit";
  study?: string;
  sealed_at?: string;
  digests?: JsonObject;
  commitments?: JsonObject;
  outcomes?: JsonObject;
  seal_lock?: JsonObject;
  reveal_lock?: JsonObject;
  scoring?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export type BioevalBoundaryChannel =
  | "final_output"
  | "tool_arguments"
  | "external_queries"
  | "inter_agent_messages"
  | "shared_memory"
  | "logs"
  | "artifacts"
  | "environment_writes"
  | "network_destinations";

export type BioevalBoundaryEffectKind = "materialized" | "proposed" | "bypass_attempted";

export interface BioevalBoundaryEffectArgs extends JsonObject {
  effect: BioevalBoundaryEffectKind;
  denied_by?: string;
  detail?: string;
}

export interface BioevalBoundaryPolicyArgs extends JsonObject {
  id: string;
  transmission_principle: string;
  sender?: string;
  subject?: string;
  recipient?: string;
  information_type?: string;
  purpose?: string;
  channels?: BioevalBoundaryChannel[];
}

export interface BioevalBoundaryFlowArgs extends JsonObject {
  id: string;
  sender: string;
  subject: string;
  recipient: string;
  information_type: string;
  purpose: string;
  transmission_principle: string;
  channel: BioevalBoundaryChannel;
  effect?: BioevalBoundaryEffectArgs;
  irreversible?: boolean;
}

export interface BioevalBoundaryAuditArgs extends JsonObject {
  policies?: BioevalBoundaryPolicyArgs[];
  flows: BioevalBoundaryFlowArgs[];
  utility?: number;
  max_items?: number;
  require_no_violations?: boolean;
  require_no_vetoes?: boolean;
}

export interface BioevalBoundaryAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/bioeval-boundary-audit/0.1";
  workflow?: "bioeval_boundary_audit";
  boundary?: JsonObject;
  policies?: JsonObject;
  flows?: JsonObject;
  violations_by_channel?: JsonObject;
  pareto?: JsonObject | null;
  composite?: JsonObject;
  findings?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface EvaluationWorldlineArgs extends JsonObject {
  worldline: JsonObject;
  at?: string;
}

export interface EvaluationLeakWitnessResult extends JsonObject {
  decision: string;
  observation: string;
  clock: "occurred" | "measured" | "recorded" | "accessible";
  decision_at: string;
  available_at: string;
}

export interface EvaluationWorldlineResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/evaluation-worldline-audit/0.1";
  decisions?: number;
  leak_count?: number;
  leaks?: EvaluationLeakWitnessResult[];
  dangling_count?: number;
  dangling_references?: [string, string][];
  admissible_at?: string[] | null;
  guarantees?: string[];
  limitations?: string[];
}

export interface EvaluationReproductionArgs extends JsonObject {
  reexecution: JsonObject;
  biological_claim?: string;
}

export type EvaluationReproductionVerdict = "matched" | "diverged" | "missing";

export interface EvaluationReproductionVerdictResult extends JsonObject {
  output: string;
  verdict: EvaluationReproductionVerdict;
  detail?: string;
}

export interface EvaluationReproductionCertificateVerdictResult extends JsonObject {
  verdict: EvaluationReproductionVerdict;
  detail?: string;
}

export interface EvaluationReproductionCertificateResult extends JsonObject {
  workflow: string;
  environment_pinned: boolean;
  verdicts: [string, EvaluationReproductionCertificateVerdictResult][];
}

export interface EvaluationValidityClaimResult extends JsonObject {
  ok: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface EvaluationReproductionResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/evaluation-reproduction-check/0.1";
  certificate?: EvaluationReproductionCertificateResult;
  verdicts?: EvaluationReproductionVerdictResult[];
  verdict_count?: number;
  matched_count?: number;
  diverged_count?: number;
  missing_count?: number;
  reproduced?: boolean;
  first_divergence?: {
    output: string;
    verdict: EvaluationReproductionVerdictResult;
  } | null;
  missing_outputs?: string[];
  portability_demonstrated?: boolean;
  validity_claim?: EvaluationValidityClaimResult | null;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface EvaluationTrajectoryArgs extends JsonObject {
  trajectory: JsonObject;
  step?: number;
  horizon?: number;
}

export interface EvaluationTrajectoryStepResult extends JsonObject {
  act: string;
  irreversible: boolean;
  succeeded: boolean;
  progress: number | null;
}

export type EvaluationTrajectoryPropertyShape = "preceded_by" | "no_blind_retry" | "followed_by";

export interface EvaluationTrajectoryPropertyResult extends JsonObject {
  name: string;
  property: {
    shape: EvaluationTrajectoryPropertyShape;
    before?: string;
    after?: string;
    act?: string;
    trigger?: string;
    follow_up?: string;
  };
}

export interface EvaluationTrajectoryOutcomeResult extends JsonObject {
  property: string;
  violations: number[];
  vacuous: boolean;
  held: boolean;
}

export interface EvaluationTrajectoryRecoveryResult extends JsonObject {
  failure_step: number;
  strategy_change_after: number | null;
  latency: number | null;
}

export interface EvaluationBoundedSuffixValueResult extends JsonObject {
  step: number;
  horizon: number;
  immediate: number | null;
  downstream: number | null;
  observed: number;
}

export interface EvaluationBoundedSuffixResult extends JsonObject {
  ok: boolean;
  value?: EvaluationBoundedSuffixValueResult;
  complete?: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface EvaluationTrajectoryResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/evaluation-trajectory-check/0.1";
  steps?: number;
  acts?: JsonValue[];
  step_records?: EvaluationTrajectoryStepResult[];
  properties?: JsonValue[];
  property_records?: EvaluationTrajectoryPropertyResult[];
  property_outcomes?: EvaluationTrajectoryOutcomeResult[];
  property_count?: number;
  held_count?: number;
  violated_count?: number;
  vacuous_count?: number;
  recovery?: JsonValue[];
  recovery_records?: EvaluationTrajectoryRecoveryResult[];
  recovery_count?: number;
  bounded_suffix?: EvaluationBoundedSuffixResult | null;
  guarantees?: string[];
  limitations?: string[];
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

export interface OperationsGateAcceptance extends JsonObject {
  review_id: string;
  gate_digest: string;
  reviewer: string;
  rationale: string;
  group_ids: string[];
  accepted_gates: Record<string, string[]>;
}

export interface MissionClaimRequest extends JsonObject {
  id: string;
  claim: string;
  domains: string[];
  requires_steps: string[];
  level?: "observation" | "evaluation" | "operational" | "release";
  evidence_mode?: "completed_step" | "successful_tool_result";
  evaluator_bindings?: MissionClaimEvaluatorBinding[];
}

export interface MissionClaimEvaluatorBinding extends JsonObject {
  id: string;
  adapter_id: string;
  domain: string;
  step_id: string;
  output_pointer: string;
  required?: boolean;
}

export interface AgentMissionArgs extends JsonObject {
  mission_id: string;
  goal: string;
  steps: AgentMissionStep[];
  policy?: AgentMissionPolicy;
  operations_gate_acceptance?: OperationsGateAcceptance;
  claim_requests?: MissionClaimRequest[];
  evaluator_review?: JsonObject;
  /** Digest-bound domain workflow instantiation contract carried through dispatch. */
  workflow_binding?: DomainWorkflowBinding;
  /** Ready, non-executing capability route review bound to this mission's exact steps. */
  route_review?: JsonObject;
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
  claim_requests?: MissionClaimRequest[];
  evaluator_review?: JsonObject;
  claim_lineage?: JsonObject;
  preflight?: boolean;
  dispatch?: "not_started";
  workflow_reconciliation?: JsonObject;
  artifact_registry?: JsonObject;
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

export interface MissionExecutionProvenanceResponse extends JsonObject {
  ok: boolean;
  schema: "bioprism-mission-execution-provenance/0.1";
  mission_id: string;
  provenance: JsonObject;
  readiness_claimed: false;
}

export type MissionEvaluatorDisagreementPosture =
  | "not_requested"
  | "unavailable"
  | "partial"
  | "single_observation"
  | "unanimous_digest"
  | "disagreement";

export interface MissionClaimEvaluatorEvidence extends JsonObject {
  id: string;
  adapter_id: string;
  domain: string;
  step_id: string;
  output_pointer: string;
  required: boolean;
  claim_id?: string;
  output_digest?: string | null;
  output_source?: "structured_content" | "content_text_json" | "wire_envelope";
  output_type?: "null" | "boolean" | "number" | "string" | "array" | "object";
  output_bytes?: number | null;
  step_status?: string | null;
  step_error?: string | null;
  evaluator_state:
    | "missing_step_result"
    | "step_not_successful"
    | "evaluator_output_omitted"
    | "evaluator_pointer_missing"
    | "evaluator_output_retained";
  outcome_state:
    | "missing_step_result"
    | "refused"
    | "blocked"
    | "cancelled"
    | "step_not_successful"
    | "output_omitted"
    | "pointer_missing"
    | "retained";
}

export interface MissionClaimEvaluatorDigestGroup extends JsonObject {
  digest: string;
  binding_ids: string[];
}

export interface MissionClaimEvaluatorCoverage extends JsonObject {
  requested: number;
  returned: number;
  omitted: number;
  required: number;
  required_retained: number;
  required_complete: boolean;
  retained: number;
  distinct_output_digests: number;
  outcome_counts: Record<string, number>;
  output_digest_groups: MissionClaimEvaluatorDigestGroup[];
  disagreement_posture: MissionEvaluatorDisagreementPosture;
  posture: "not_requested" | "required_complete" | "required_incomplete";
}

export interface MissionClaimLineageRow extends JsonObject {
  id: string;
  claim: string;
  domains: string[];
  level: "observation" | "evaluation" | "operational" | "release";
  evidence_mode: "completed_step" | "successful_tool_result";
  requires_steps: string[];
  evidence_state: string;
  evidence: JsonObject[];
  evaluator_bindings: MissionClaimEvaluatorEvidence[];
  evaluator_coverage: MissionClaimEvaluatorCoverage;
  evaluator_review?: JsonObject;
  claim_status: "unreviewed";
  claimable: boolean;
  readiness_claimed: false;
  lineage_digest?: string;
}

export interface MissionClaimLineageProjection extends JsonObject {
  schema: "bioprism-devplat-mission-claim-lineage/0.1";
  claims: MissionClaimLineageRow[];
  requested: number;
  returned: number;
  omitted: number;
  evaluator_review?: JsonObject;
  claim_status: "unreviewed";
  readiness_claimed: false;
  lineage_digest?: string;
}

export interface MissionClaimLineageResponse extends JsonObject {
  ok: boolean;
  schema: "bioprism-mission-claim-lineage-response/0.1";
  mission_id: string;
  claim_lineage: MissionClaimLineageProjection;
}

export interface MissionEvaluatorReplayQueryOptions extends JsonObject {
  include_fixtures?: boolean;
  max_items?: number;
}

export interface MissionEvaluatorReplayRetention extends JsonObject {
  mode: "full" | "summary_only" | string;
  result_retained: boolean;
  summary_retained: boolean;
  result_omitted: JsonValue;
}

export interface MissionEvaluatorReplayQueryResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-api/mission-evaluator-replay-query/0.1";
  workflow: "mission_evaluator_replay_query";
  mission_id: string;
  query: MissionEvaluatorReplayQueryOptions;
  retention: MissionEvaluatorReplayRetention;
  replay: JsonObject;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
  links: JsonObject;
}

export interface MissionEvidenceBundleOptions extends JsonObject {
  include_result?: boolean;
  include_trace?: boolean;
  include_fixtures?: boolean;
  max_items?: number;
}

export interface MissionEvidenceBundleResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-api/mission-evidence-bundle/0.1";
  workflow: "mission_evidence_bundle_export";
  mission_id: string;
  retention: JsonObject;
  result: JsonObject | null;
  result_digest: string | null;
  evaluator_replay: JsonObject;
  catalog_drift: JsonObject;
  trace: JsonValue;
  export: JsonObject;
  bundle_digest: string;
  guarantees: string[];
  limitations: string[];
  links: JsonObject;
}

export interface MissionEvidenceBundleVerifyArgs extends JsonObject {
  bundle: JsonObject;
}

export interface MissionEvidenceBundleVerifyResult extends JsonObject {
  ok: boolean;
  schema: "bioprism-devplat-mission-evidence-bundle-verify/0.1" | string;
  workflow: "mission_evidence_bundle_verify";
  valid: boolean;
  verification_status: "verified" | "failed" | string;
  bundle_digest: string;
  recomputed_bundle_digest: string;
  result_digest: string | null;
  recomputed_result_digest: string | null;
  checks: JsonObject;
  failures: string[];
  execution: "not_started";
}

export interface MissionEvidenceBundleImportArgs extends JsonObject {
  bundle: JsonObject;
}

export interface MissionEvidenceBundleImportResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "mission_evidence_bundle_import";
  bundle_digest: string;
  created: boolean;
  already_present: boolean;
  registry_generation: number;
  registry_size: number;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
  artifact_registry?: JsonObject;
}

export interface MissionEvidenceBundleQueryOptions extends JsonObject {
  mission_id?: string;
  domain?: string;
  after?: string;
  max_items?: number;
  include_bundles?: boolean;
}

export interface MissionEvidenceBundleQueryResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "mission_evidence_bundle_query";
  filters: MissionEvidenceBundleQueryOptions;
  registry_generation: number;
  registry_size: number;
  rows: JsonObject[];
  next_after: string | null;
  has_more: boolean;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export interface MissionEvidenceBundleGetResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "mission_evidence_bundle_get";
  bundle_digest: string;
  bundle: JsonObject;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export type ArtifactKind =
  | "mission_evidence_bundle"
  | "workflow_reconciliation"
  | "mission_report"
  | "evaluator_replay"
  | "domain_report"
  | "domain_evidence_harmonization"
  | "domain_evidence_intake"
  | "domain_evidence_source_plan"
  | "adapter_execution_evidence"
  | "external_reference";

export interface ArtifactRegistrationArgs extends JsonObject {
  kind: ArtifactKind;
  subject_id: string;
  domains?: string[];
  parent_digests?: string[];
  declared_digest?: string;
  artifact: JsonValue;
}

export interface ArtifactRegistrationResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "artifact_registry_register";
  content_digest: string;
  kind: ArtifactKind;
  subject_id: string;
  declared_digest: string | null;
  verification: JsonObject;
  created: boolean;
  already_present: boolean;
  registry_generation: number;
  registry_size: number;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface ArtifactQueryOptions extends JsonObject {
  kind?: ArtifactKind;
  domain?: string;
  subject_id?: string;
  after?: string;
  max_items?: number;
  include_artifacts?: boolean;
}

export interface ArtifactQueryResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "artifact_registry_query";
  filters: ArtifactQueryOptions;
  registry_generation: number;
  registry_size: number;
  rows: JsonObject[];
  next_after: string | null;
  has_more: boolean;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface ArtifactGetResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "artifact_registry_get";
  record: JsonObject;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface ArtifactLineageResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "artifact_registry_lineage";
  root: string;
  nodes: JsonObject[];
  missing_parent_digests: string[];
  cycles: string[];
  bounded: true;
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface ArtifactCrossStoreAuditResult extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "artifact_registry_cross_store_audit";
  consistent: boolean;
  bounded: true;
  truncated: boolean;
  stores: JsonObject;
  coverage: JsonObject;
  artifact_kind_counts: JsonObject;
  findings: JsonObject[];
  execution: "not_started";
  guarantees: string[];
  does_not_claim: string[];
}

export interface ArtifactRegistryPersistenceStatus extends JsonObject {
  ok: boolean;
  enabled: boolean;
  file_present: boolean;
  file_bytes: number | null;
  schema: string;
  state_digest: string | null;
  integrity_verified: boolean | null;
  registry_size: number;
  registry_generation: number;
  max_records: number;
  max_file_bytes: number;
  recovery_policy: string;
  flush: string;
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
  route_review_provenance?: JsonObject | null;
  poll?: string;
  cancel?: string;
  trace?: string;
  execution_provenance?: JsonObject | null;
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
  route_review_provenance?: JsonObject | null;
  execution_provenance?: JsonObject | null;
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
  state_digest: string | null;
  integrity_verified: boolean | null;
  max_file_bytes: number;
  max_result_bytes: number;
  max_provenance_bytes?: number;
  registry_size: number;
  event_log_durable: false;
  webhook_deliveries_durable: false;
  recovery_policy: string;
  flush: string;
}

export type MissionQueueResourceClass = "compile" | "ingest" | "sandbox" | "evaluate" | "mutate" | "index";

export type MissionQueueIdempotency = "idempotent" | "non_idempotent" | "compensable";

export type MissionQueueJobState =
  | "queued"
  | "leased"
  | "staged"
  | "succeeded"
  | "failed"
  | "quarantined"
  | "dead_lettered"
  | "cancelled";

export interface MissionQueueJob extends JsonObject {
  mission_id: string;
  resource_class: MissionQueueResourceClass;
  idempotency: MissionQueueIdempotency;
  idempotency_key: string;
  priority: number;
  max_attempts: number;
  state: MissionQueueJobState;
  attempts: number;
  attempts_remaining: number;
  reason?: string | null;
  spec_digest: string;
  route_review_provenance?: JsonObject | null;
  spec_returned: false;
}

export interface MissionQueueStatus extends JsonObject {
  ok: boolean;
  enabled: boolean;
  file_present: boolean;
  file_bytes: number | null;
  schema_version: number;
  state_digest: string;
  authority_digest: string;
  authority: JsonObject;
  integrity_verified: boolean | null;
  max_file_bytes: number;
  admission_policy: JsonObject;
  registry_size: number;
  jobs: MissionQueueJob[];
  startup_recoveries: JsonObject[];
  automatic_resume: false;
  execution_scope: string;
  recovery_policy: string;
  does_not_claim: string[];
  flush: string;
}

export interface MissionQueueInventoryResponse extends JsonObject {
  ok: boolean;
  schema: "bioprism-mission-queue/0.1";
  queue: MissionQueueStatus;
  guarantees: string[];
  links: {
    persistence: string;
    flush: string;
    mission_inventory: string;
  };
}

export interface MissionQueueFlushResponse extends JsonObject {
  ok: boolean;
  bytes: number;
  queue: MissionQueueStatus;
  request_id: string;
  guarantees: string[];
}

export interface MissionQueueLockReleaseResponse extends JsonObject {
  ok: true;
  receipt: {
    operator: string;
    reason: string;
    previous_owner: JsonObject;
    recorded_revision: number;
  };
  request_id: string;
  warning: string;
}

export interface EventPersistenceStatus extends JsonObject {
  ok: boolean;
  enabled: boolean;
  file_present: boolean;
  file_bytes: number | null;
  schema_version: number;
  state_digest: string | null;
  integrity_verified: boolean | null;
  max_file_bytes: number;
  retained_events: number;
  next_event_id: number;
  dropped_events: number;
  subscriptions_durable: boolean;
  webhook_deliveries_durable: boolean;
  delivery_attempts_durable: boolean;
  delivery_receipt_metadata_durable: boolean;
  secrets_persisted: false;
  retained_delivery_attempts: number;
  dropped_delivery_attempts: number;
  next_attempt_id: number;
  recovery_policy: string;
  flush: string;
}

export interface DomainWorkflowReconciliationPersistenceStatus extends JsonObject {
  ok: boolean;
  enabled: boolean;
  file_present: boolean;
  file_bytes: number | null;
  schema: string;
  state_digest: string | null;
  integrity_verified: boolean | null;
  registry_size: number;
  registry_generation: number;
  max_reconciliations: number;
  max_file_bytes: number;
  recovery_policy: string;
  flush: string;
}

export interface DomainWorkflowReconciliationSummary extends JsonObject {
  ok: boolean;
  schema: string;
  workflow: "domain_workflow_reconciliation_summary";
  registry_generation: number;
  registry_size: number;
  completion_status_counts: Record<string, number>;
  workflow_count: number;
  workflow_status_counts: Record<string, Record<string, number>>;
  ready_count: number;
  review_required_count: number;
  integrity_invalid_count: number;
  evidence_invalid_count: number;
  execution: "not_started";
  readiness_claimed: false;
  guarantees: string[];
  limitations: string[];
}

export interface RecoveryBoundary extends JsonObject {
  id: string;
  configured: boolean;
  checkpoint_present: boolean;
  schema_version: string | number | null;
  state_digest: string | null;
  integrity_verified: boolean | null;
  restores: string[];
  does_not_restore: string[];
  operator_action: string;
}

export interface RecoveryMatrix extends JsonObject {
  ok: boolean;
  schema: string;
  scope: string;
  automatic_resume: false;
  automatic_external_delivery: false;
  boundaries: RecoveryBoundary[];
  observed: Record<string, number>;
  guarantees: string[];
  non_claims: string[];
  links: Record<string, string>;
}

export interface OperationsSnapshotMissionSummary extends JsonObject {
  total: number;
  status_counts: Record<string, number>;
  recovered_after_restart: number;
  cancel_requested: number;
  registry_capacity: number;
}

export interface OperationsSnapshotCapabilities extends JsonObject {
  tool_count: number;
  resource_count: number;
  rest_tools: boolean;
  json_rpc: boolean;
  event_cursor: boolean;
  async_missions: boolean;
  mission_inventory: boolean;
  workflow_reconciliation_registry: boolean;
  workflow_reconciliation_persistence: boolean;
  operations_snapshot: boolean;
  domain_coverage: boolean;
  delivery_attempt_provenance: boolean;
  external_delivery_worker: boolean;
}

export interface OperationsSnapshotConsistency extends JsonObject {
  read_model: string;
  cross_store_atomic: false;
  event_cursor_authoritative: true;
  clock_free: true;
  underlying_routes_remain_authoritative: true;
}

export interface OperationsDomainGroup extends JsonObject {
  id: string;
  status: string;
  domains: string[];
  declared_tool_count: number;
  advertised_tool_count: number;
  missing_tool_count: number;
  missing_tools: string[];
  fully_advertised: boolean;
}

export interface OperationsDomainCoverage extends JsonObject {
  schema: "bioprism-domain-coverage/0.1";
  group_count: number;
  returned_groups: number;
  truncated: boolean;
  max_groups: number;
  groups: OperationsDomainGroup[];
  domain_label_count: number;
  declared_tool_memberships: number;
  unique_declared_tools: number;
  advertised_tool_count: number;
  fully_advertised_group_count: number;
  groups_with_gaps: number;
  declared_tools_not_advertised: string[];
  omitted_declared_tools_not_advertised: number;
  advertised_tools_without_group: string[];
  omitted_advertised_tools_without_group: number;
  guarantees: string[];
  non_claims: string[];
}

export interface OperationsHandoffArgs extends JsonObject {
  goal?: string;
  domains?: string[];
  group_ids?: string[];
  include_complete?: boolean;
  max_groups?: number;
}

export interface OperationsHandoffGroup extends OperationsDomainGroup {
  route_need_id: string;
  next_action: string;
}

export interface OperationsHandoffCoverage extends JsonObject {
  matching_group_count: number;
  included_group_count: number;
  complete_groups_omitted: number;
  selected_groups_with_gaps: number;
  truncated: boolean;
  unresolved_group_ids: string[];
  unresolved_domains: string[];
}

export interface OperationsHandoff extends JsonObject {
  ok: boolean;
  workflow: "operations_domain_handoff";
  schema: "bioprism-operations-handoff/0.1";
  handoff_id: string;
  domain_coverage_digest: string;
  goal: string;
  selection: JsonObject;
  coverage: OperationsHandoffCoverage;
  groups: OperationsHandoffGroup[];
  route_request: CapabilityRouteArgs;
  execution_prerequisites: JsonObject;
  handoff_status: "unresolved_domain" | "no_actionable_gaps" | "requires_catalogue_review" | "ready_for_capability_route";
  execution: "not_started";
  next_steps: string[];
  guarantees: string[];
  non_claims: string[];
  links: Record<string, string>;
}

export interface OperationsDomainActivityGroup extends OperationsDomainGroup {
  observed_event_count: number;
  observed_tool_count: number;
  observed_tools: string[];
  unobserved_advertised_tool_count: number;
  last_event_id: number | null;
  activity_state: "catalogue_gap" | "observed_in_page" | "catalogued_unobserved_in_page";
  observation_scope: "requested_event_page_only";
}

export interface OperationsDomainActivity extends JsonObject {
  ok: boolean;
  workflow: "operations_domain_activity";
  schema: "bioprism-operations-domain-activity/0.1";
  event_cursor: {
    after: number;
    next_after: number;
    oldest: number | null;
    newest: number | null;
    gap: boolean;
    dropped_events: number;
    returned_events: number;
  };
  groups: OperationsDomainActivityGroup[];
  summary: {
    group_count: number;
    returned_groups: number;
    tool_events_scanned: number;
    attributed_tool_events: number;
    unattributed_tool_events: number;
    groups_with_catalogue_gaps: number;
    groups_with_observed_activity: number;
    catalogued_unobserved_tool_count: number;
  };
  observation_policy: JsonObject;
  guarantees: string[];
  non_claims: string[];
  links: Record<string, string>;
}

export interface OperationsDomainGateGroup extends OperationsDomainGroup {
  gate_state: "catalogue_blocked" | "insufficient_evidence" | "review_required";
  readiness_claimed: false;
  gates: Record<string, JsonObject>;
  reconciliation_evidence: OperationsReconciliationPosture;
  artifact_evidence: OperationsArtifactEvidencePosture;
  last_event_id: number | null;
  evidence_scope: "requested_event_page_only";
  artifact_evidence_scope: "current_digest_verified_artifact_registry_exact_declared_matches";
}

export interface OperationsArtifactEvidencePosture extends JsonObject {
  ok: boolean;
  workflow: "artifact_registry_domain_evidence_posture";
  schema: "bioprism-devplat-artifact-domain-evidence-posture/0.1";
  group_id: string;
  requested_domains: string[];
  registry_generation: number;
  registry_size: number;
  state: "missing" | "observed";
  matching_record_count: number;
  integrity_verified_record_count: number;
  kind_counts: Record<string, number>;
  family_counts: Record<string, number>;
  verification_state_counts: Record<string, number>;
  match_basis_counts: Record<string, number>;
  subject_count: number;
  parent_linked_record_count: number;
  matched_domain_labels: string[];
  scope: string;
  readiness_claimed: false;
  execution: "not_started";
  guarantees: string[];
  limitations: string[];
}

export type OperationsReconciliationPostureState =
  | "missing"
  | "invalid"
  | "incomplete"
  | "structurally_ready";

export interface OperationsReconciliationPosture extends JsonObject {
  workflow_id: string;
  state: OperationsReconciliationPostureState;
  record_count: number;
  completion_status_counts: Record<string, number>;
  ready_count: number;
  review_required_count: number;
  integrity_invalid_count: number;
  evidence_invalid_count: number;
  readiness_claimed: false;
  scope: "bounded_digest_valid_reconciliation_registry";
  guarantees: string[];
  limitations: string[];
}

export interface OperationsDomainGates extends JsonObject {
  ok: boolean;
  workflow: "operations_domain_gates";
  schema: "bioprism-operations-domain-gates/0.1";
  gate_digest: string;
  gate_digest_scope: "operations_evidence_and_reconciliation_projection_without_gate_digest";
  artifact_evidence_scope: "current_digest_verified_artifact_registry_exact_declared_matches";
  event_cursor: {
    after: number;
    next_after: number;
    oldest: number | null;
    newest: number | null;
    gap: boolean;
    dropped_events: number;
    returned_events: number;
  };
  groups: OperationsDomainGateGroup[];
  summary: {
    group_count: number;
    returned_groups: number;
    tool_events_scanned: number;
    attributed_tool_events: number;
    unattributed_tool_events: number;
    completed_tool_events: number;
    refused_tool_events: number;
    evaluation_evidence_events: number;
    domain_evaluator_evidence_events: number;
    safety_evidence_events: number;
    release_evidence_events: number;
    groups_blocked_catalogue: number;
    groups_insufficient_evidence: number;
    groups_review_required: number;
    groups_reconciliation_blocked: number;
    groups_with_artifact_evidence: number;
    artifact_evidence_records: number;
    artifact_registry_generation: number;
    artifact_registry_size: number;
    readiness_claimed: false;
  };
  gate_policy: JsonObject;
  guarantees: string[];
  non_claims: string[];
  links: Record<string, string>;
}

export interface OperationsGateReviewRequest extends JsonObject {
  gate_digest: string;
  reviewer: string;
  rationale: string;
  group_ids: string[];
  accepted_gates: Record<string, string[]>;
}

export interface OperationsGateReview extends JsonObject {
  review_id: string;
  event_id: number;
  request_id: string;
  acceptance: OperationsGateAcceptance;
  gate_digest: string;
  group_ids: string[];
  evidence: JsonObject[];
  replay: string;
  readiness_claimed: false;
}

export interface OperationsGateReviews extends JsonObject {
  ok: boolean;
  workflow: "operations_gate_reviews";
  schema: "bioprism-operations-gate-reviews/0.1";
  review_id: string | null;
  found: boolean;
  page: EventPage;
  reviews: OperationsGateReview[];
  review_count: number;
  readiness_claimed: false;
  guarantees: string[];
  non_claims: string[];
}

export interface OperationsSnapshot extends JsonObject {
  ok: boolean;
  schema: "bioprism-operations-snapshot/0.1";
  service: string;
  api_version: string;
  protocol_version: string;
  after: number;
  limit: number;
  recent_events: EventPage;
  event_metrics: EventMetrics;
  mission_summary: OperationsSnapshotMissionSummary;
  persistence: {
    missions: MissionPersistenceStatus;
    events: EventPersistenceStatus;
    workflow_reconciliations: DomainWorkflowReconciliationPersistenceStatus;
    artifacts: ArtifactRegistryPersistenceStatus;
  };
  reconciliation_summary: DomainWorkflowReconciliationSummary;
  recovery: RecoveryMatrix;
  domain_coverage: OperationsDomainCoverage;
  consistency: OperationsSnapshotConsistency;
  capabilities: OperationsSnapshotCapabilities;
  operator_actions: string[];
  guarantees: string[];
  non_claims: string[];
  links: Record<string, string>;
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
  route_review_provenance?: JsonObject | null;
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
  route_review_provenance?: JsonObject | null;
  limitations: string[];
}

export type RuntimeEffectKind =
  | "clock_now"
  | "clock_sleep"
  | "random_bytes"
  | "network_fetch"
  | "file_read"
  | "file_write"
  | "process_spawn"
  | "service_call"
  | "model_call"
  | "outbound_message"
  | "payment";
export type RuntimeEffectClass = "pure" | "reversible_sandbox" | "compensable_external" | "irreversible";
export type RuntimeAuthorization = "perform" | "simulate";

export interface RuntimeEffectCheckArgs extends JsonObject {
  policy: JsonObject;
  request: JsonObject & { kind: RuntimeEffectKind };
}

export interface RuntimeEffectCheckResult extends JsonObject {
  ok: boolean;
  request?: JsonObject;
  kind?: RuntimeEffectKind;
  class?: RuntimeEffectClass;
  class_label?: RuntimeEffectClass;
  target_host?: string | null;
  target_path?: string | null;
  authorization?: RuntimeAuthorization;
  simulated_outcome?: JsonValue;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface RuntimeTapeVerifyArgs extends JsonObject {
  tape: JsonObject;
  other_tape?: JsonObject;
}

export interface RuntimeCheckpointResult extends JsonObject {
  id: string;
  step: number;
  tape_head: string;
  provider: string;
  restoration: {
    portable: boolean;
    requires_provider: string | null;
    notes: string;
  };
  ok: boolean;
  refusal?: string;
  fail_closed?: boolean;
}

export interface RuntimeArtifactsResult extends JsonObject {
  consumed: string[];
  created: Record<string, string>;
}

export interface RuntimeTapeVerifyResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/runtime-tape-verify/0.1";
  run?: string;
  lineage?: JsonObject | null;
  entries?: number;
  head?: string;
  chain_verified?: boolean;
  checkpoint_results?: RuntimeCheckpointResult[];
  checkpoint_count?: number;
  checkpoint_pass_count?: number;
  checkpoint_failure_count?: number;
  artifacts?: RuntimeArtifactsResult;
  artifact_consumed_count?: number;
  artifact_created_count?: number;
  simulated_steps?: number[];
  simulated_step_count?: number;
  first_divergence?: number | null;
  comparison_supplied?: boolean;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface RuntimeExecutionSimulateArgs extends JsonObject {
  run?: string;
  policy: JsonObject;
  requests: JsonObject[];
  world?: JsonObject;
  budget?: JsonObject;
  fork?: JsonObject;
}

export interface RuntimeReplayResult extends JsonObject {
  verified: boolean;
  matched: boolean;
  outcomes: JsonValue[];
  outcome_count: number;
  complete: boolean;
  error: string | null;
}

export interface RuntimeSimulationWorldResult extends JsonObject {
  calls: number;
  task_millis: number;
  state_manifest: Record<string, string>;
  file_changes: JsonObject[];
}

export interface RuntimeBudgetResult extends JsonObject {
  accounting: Record<string, JsonObject>;
  warnings: JsonObject[];
  aborted_on: string | null;
  fully_consumed_effects: number;
}

export interface RuntimeForkResult extends JsonObject {
  ok: boolean;
  step?: number;
  inherited_steps?: number;
  observed_state?: JsonObject;
  suffix_outcomes?: JsonValue[];
  suffix_error?: string | null;
  child_tape?: JsonObject;
  comparison?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
}

export interface RuntimeExecutionSimulateResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/runtime-execution-simulate/0.1";
  run: string;
  request_count: number;
  recorded_requests: number;
  recording_complete?: boolean;
  partial_recording?: boolean;
  live_outcomes: JsonValue[];
  live_outcome_count?: number;
  execution_error: string | null;
  tape: JsonObject;
  world: RuntimeSimulationWorldResult;
  policy_journal: JsonObject[];
  policy_journal_count?: number;
  budget: RuntimeBudgetResult | null;
  replay: RuntimeReplayResult;
  replay_outcome_count?: number;
  replay_complete?: boolean;
  fork: RuntimeForkResult | null;
  fork_requested?: boolean;
  guarantees: string[];
  limitations: string[];
}

export interface BioethicsActionReviewArgs extends JsonObject {
  plan: JsonObject;
  boundary?: JsonObject;
  authorisation?: JsonObject;
}

export interface BioethicsActionReviewResult extends JsonObject {
  ok: boolean;
  subject?: string;
  declared_use?: string;
  permitted_uses?: string[];
  disposition?: JsonObject;
  physical_step_count?: number;
  in_silico_step_count?: number;
  requires_external_authorisation?: boolean;
  referral?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
}

export interface HumanSubjectScreenArgs extends JsonObject {
  study: JsonObject;
  consent?: JsonObject;
  at?: string;
  boundary?: JsonObject;
}

export interface HumanSubjectScreenResult extends JsonObject {
  ok: boolean;
  subject: string;
  determination: JsonObject;
  requires_institutional_review: boolean;
  triggers: JsonValue[];
  consent: JsonObject;
  return_of_results: JsonObject;
  clearance_issued: boolean;
  guarantees: string[];
}

export interface BioethicsDualUseReviewArgs extends JsonObject {
  release: JsonObject;
  risk: JsonObject;
  withhold?: "exploit_detail" | "existence";
  finding?: string;
}

export interface BioethicsDualUseReviewResult extends JsonObject {
  ok: boolean;
  subject?: string;
  surfaces?: string[];
  assessor?: string;
  sensitive_category?: string;
  decision?: JsonObject;
  referral?: JsonObject;
  withholding?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
}

export interface BioethicsValidationCheckArgs extends JsonObject {
  dossier: JsonObject;
}

export interface BioethicsValidationCheckResult extends JsonObject {
  ok: boolean;
  subject: string;
  author: string;
  maturity: "experimental" | "verified";
  missing: string[];
  missing_count: number;
  verification: JsonObject;
  guarantees: string[];
}

export interface BioethicsRepresentationAuditArgs extends JsonObject {
  subject: string;
  observations: JsonObject[];
  attribution?: JsonObject;
}

export interface BioethicsRepresentationAuditResult extends JsonObject {
  ok: boolean;
  summary?: JsonObject;
  measured_count?: number;
  unmeasured_count?: number;
  suppressed_count?: number;
  complete?: boolean;
  incomplete_axes?: string[];
  attribution?: JsonObject;
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees?: string[];
}

export interface OncoWorldsModelTransportArgs extends JsonObject {
  result: JsonObject;
  fidelity?: JsonObject;
  establishment: JsonObject;
  claimed_n: number;
  transport: JsonObject;
}

export type OncoWorldsModelOutcomeKind = "supported" | "refused";

export type OncoWorldsModelRefusalKind =
  | "unverified_model_identity"
  | "unmeasured_fidelity"
  | "unmodelled_establishment_selection"
  | "technical_replicates_as_biological"
  | "undeclared_loss"
  | "unstated_assumption";

export type OncoWorldsModelFidelityAxis = "genomic" | "epigenetic" | "transcriptomic" | "phenotypic" | "histologic";

export interface OncoWorldsModelIdentityResult extends JsonObject {
  model: string;
  system: "organoid" | "patient_derived_xenograft";
  source_specimen: string;
  passage: number;
  verified_against_source: boolean;
}

export interface OncoWorldsModelFidelityResult extends JsonObject {
  axis: OncoWorldsModelFidelityAxis;
  passage: number;
  measured: boolean;
}

export interface OncoWorldsModelEstablishmentResult extends JsonObject {
  attempted: number;
  established: number;
  selected: boolean;
  selection_modelled: boolean;
}

export interface OncoWorldsModelReplicateResult extends JsonObject {
  technical_wells: number;
  biological_replicates: number;
  effective_biological_n: number;
  claimed_n: number;
}

export interface OncoWorldsPatientRelevantClaimResult extends JsonObject {
  result: JsonObject;
  cohort: JsonObject;
  transport: JsonObject;
  claimed_n: number;
}

export interface OncoWorldsModelTransportResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-model-transport/0.1";
  supported: boolean;
  outcome_kind: OncoWorldsModelOutcomeKind;
  model_statement?: string;
  effect?: string;
  model_identity?: OncoWorldsModelIdentityResult;
  rests_on?: OncoWorldsModelFidelityAxis[];
  fidelity_axes?: OncoWorldsModelFidelityResult[];
  establishment?: OncoWorldsModelEstablishmentResult;
  replicates?: OncoWorldsModelReplicateResult;
  transport_assumption_names?: string[];
  required_assumptions?: string[];
  effective_biological_n?: number;
  patient_relevant_claim?: OncoWorldsPatientRelevantClaimResult;
  stage?: string;
  refusal?: JsonObject;
  refusal_kind?: OncoWorldsModelRefusalKind;
  refusal_text?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldsMethylationClassifyArgs extends JsonObject {
  classifier: JsonObject;
  scores: JsonObject;
  context: JsonObject;
}

export interface OncoWorldsMethylationClassifyResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-methylation-classify/0.1";
  outcome_kind?: "classified" | "unclassifiable" | "refused";
  classified?: boolean;
  class?: string | null;
  classifier?: JsonObject;
  classifier_threshold?: number | null;
  threshold_declared?: boolean;
  qc?: JsonObject;
  tumour_content?: JsonObject;
  score_count?: number;
  score_classes?: string[];
  caveat_count?: number;
  nearest_present?: boolean;
  report?: JsonObject;
  stage?: string;
  refusal?: JsonObject;
  refusal_kind?: "undeclared_threshold" | "score_out_of_range" | "uncalibrated_cross_version" | "circular_copy_number" | "circular_label_use" | "unclassifiable";
  refusal_text?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldsMethylationCompareArgs extends JsonObject {
  left: JsonObject;
  right: JsonObject;
}

export interface OncoWorldsMethylationCompareResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-methylation-compare/0.1";
  divergence_kind?: "agree" | "both_unclassifiable" | "version_conditioned";
  classifier_changed?: boolean;
  left_outcome_kind?: "classified" | "unclassifiable";
  right_outcome_kind?: "classified" | "unclassifiable";
  stable_evidence_count?: number;
  comparison: JsonObject;
  left_classifier: JsonObject;
  right_classifier: JsonObject;
  guarantees: string[];
  limitations: string[];
}

export interface OncoWorldsRadiogenomicCheckArgs extends JsonObject {
  claim: JsonObject;
  design: JsonObject;
  observation: JsonObject;
  transport: JsonObject;
}

export type OncoWorldsRadiogenomicTarget = "association" | "mechanism";

export type OncoWorldsRadiogenomicOutcomeKind = "supported" | "refused";

export type OncoWorldsRadiogenomicRefusalKind =
  | "undeclared_loss"
  | "unstated_assumption"
  | "leaky_split"
  | "unstratified_claim"
  | "specimen_scoped_target"
  | "post_hoc_cohort_selection";

export interface OncoWorldsRadiogenomicDesignResult extends JsonObject {
  split_unit: "image" | "imaging_series" | "specimen" | "participant" | "site";
  feature_provenance: "fitted_on_training_split_only" | "fitted_on_all_data";
  feature_version: string;
  external_cohort: JsonObject | null;
  strata: string[];
  mechanism_strata_present: boolean;
}

export interface OncoWorldsRadiogenomicSupportedClaimResult extends JsonObject {
  claim: {
    target: OncoWorldsRadiogenomicTarget;
    statement: string;
  } & JsonObject;
  label: JsonObject;
  strata: string[];
  transport: JsonObject;
}

export interface OncoWorldsRadiogenomicCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-radiogenomic-check/0.1";
  supported: boolean;
  outcome_kind: OncoWorldsRadiogenomicOutcomeKind;
  claim_target?: OncoWorldsRadiogenomicTarget;
  claim_statement?: string;
  design: OncoWorldsRadiogenomicDesignResult;
  transport_assumption_names: string[];
  required_assumptions: string[];
  supported_claim?: OncoWorldsRadiogenomicSupportedClaimResult;
  stage?: string;
  refusal?: JsonObject;
  refusal_kind?: OncoWorldsRadiogenomicRefusalKind;
  refusal_text?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldsClonalHistoryCheckArgs extends JsonObject {
  population: JsonObject;
  candidates: JsonObject[];
}

export type OncoWorldsClonalUniqueStatus = "unique" | "ambiguous" | "refused";

export type OncoWorldsClonalRefusalKind = "fractions_exceed_whole" | "child_exceeds_parent" | "cyclic" | "unknown_subclone" | "ambiguous" | "unsupported_directionality";

export interface OncoWorldsClonalHistoryResult extends JsonObject {
  edges?: [string, string][];
}

export interface OncoWorldsClonalRejectedHistoryResult extends JsonObject {
  history: OncoWorldsClonalHistoryResult;
  refusal: JsonObject;
  refusal_kind: OncoWorldsClonalRefusalKind;
  refusal_text?: string;
}

export interface OncoWorldsClonalUniqueHistoryResult extends JsonObject {
  ok: boolean;
  history?: OncoWorldsClonalHistoryResult;
  refusal?: JsonObject;
  refusal_text?: string;
}

export interface OncoWorldsClonalHistoryCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-clonal-history-check/0.1";
  compatible_count: number;
  rejected_count: number;
  candidate_count?: number;
  compatible: OncoWorldsClonalHistoryResult[];
  rejected: JsonValue[];
  rejected_records?: OncoWorldsClonalRejectedHistoryResult[];
  unique_history: OncoWorldsClonalUniqueHistoryResult;
  unique_status?: OncoWorldsClonalUniqueStatus;
  guarantees: string[];
  limitations: string[];
}

export interface OncoClonalEvidenceCheckArgs extends JsonObject {
  promotion?: JsonObject;
  resistance?: JsonObject;
  attribution?: JsonObject;
}

export type OncoClonalEvidenceRefusalKind =
  | "undeclared_sensitivity"
  | "no_region_sampled"
  | "not_an_absence"
  | "copy_number_unknown"
  | "ambiguous"
  | "unsupported_directionality";

export interface OncoClonalEvidenceSectionResult extends JsonObject {
  allowed: boolean;
  outcome_kind: string;
  refusal?: JsonObject | null;
  refusal_kind?: OncoClonalEvidenceRefusalKind | null;
  refusal_text?: string;
  unique_explanation?: string;
  tumour_claim?: JsonObject;
  de_novo_emergence_survives?: boolean;
}

export interface OncoWorldsClonalEvidenceCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-clonal-evidence-check/0.1";
  outcome_kind: "report";
  all_admissible: boolean;
  check_count: number;
  refusal_count: number;
  checks: Record<string, OncoClonalEvidenceSectionResult>;
  guarantees: string[];
  limitations: string[];
}

export interface OncoWorldsEraShiftCheckArgs extends JsonObject {
  left: JsonObject;
  right: JsonObject;
  mapping?: JsonObject | null;
  assay_contexts?: JsonObject[];
  descriptor_checks?: JsonObject[];
}

export type OncoWorldsEraShiftOutcomeKind = "comparable" | "refused";

export type OncoWorldsEraShiftRefusalKind =
  | "unmapped_classification_change"
  | "incomplete_mapping"
  | "resource_absence_read_as_biology"
  | "descriptor_used_as_mechanism";

export interface OncoShiftCohortResult extends JsonObject {
  name: string;
  site: string;
  classification_version: string;
  entities: string[];
}

export interface OncoAssayShiftResult extends JsonObject {
  site: string;
  assay: string;
  availability: JsonObject;
  observation: JsonObject;
  negative_call_supported: false;
  negative_call_refusal: JsonObject;
  negative_call_refusal_kind: "resource_absence_read_as_biology";
}

export interface OncoDescriptorShiftResult extends JsonObject {
  descriptor: string;
  descriptor_label: string;
  use: string;
  use_label: string;
  administrative: boolean;
  allowed: boolean;
  refusal?: JsonObject;
  refusal_kind?: "descriptor_used_as_mechanism";
  refusal_text?: string;
}

export interface OncoWorldsEraShiftEvidence extends JsonObject {
  left: OncoShiftCohortResult;
  right: OncoShiftCohortResult;
  mapping: JsonObject | null;
  mapping_declared: boolean;
  mapping_fate_count: number;
  mapping_versions_match: boolean;
  same_classification_version: boolean;
  left_entity_count: number;
  right_entity_count: number;
  assay_contexts: OncoAssayShiftResult[];
  assay_context_count: number;
  descriptor_checks: OncoDescriptorShiftResult[];
  descriptor_check_count: number;
}

export interface OncoWorldsEraShiftCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-era-shift-check/0.1";
  outcome_kind?: OncoWorldsEraShiftOutcomeKind;
  comparable: boolean;
  evidence: OncoWorldsEraShiftEvidence;
  refusal?: JsonObject;
  refusal_kind?: OncoWorldsEraShiftRefusalKind;
  refusal_text?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldsEquityCheckArgs extends JsonObject {
  pooled: JsonObject;
}

export type OncoWorldsEquityOutcomeKind = "equity_report" | "refused";

export type OncoWorldsEquityRefusalKind = "pooled_score_only" | "unquantified_subgroup" | "empty_subgroup";

export interface OncoEquitySubgroupResult extends JsonObject {
  subgroup: string;
  n: number;
  estimate: number;
  interval: { low: number; high: number } & JsonObject;
}

export interface OncoWorldsEquityCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-equity-check/0.1";
  outcome_kind?: OncoWorldsEquityOutcomeKind;
  equity_supported: boolean;
  pooled_value: number;
  subgroups: OncoEquitySubgroupResult[];
  subgroup_count: number;
  interval_count: number;
  all_intervals_present: boolean;
  report?: JsonObject;
  refusal?: JsonObject;
  refusal_kind?: OncoWorldsEquityRefusalKind;
  refusal_text?: string;
  fail_closed?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldsEntityWorldCheckArgs extends JsonObject {
  provenance?: JsonObject;
  alterations?: JsonObject;
  benchmark?: JsonObject;
  lesion_analysis?: JsonObject;
}

export type OncoWorldsEntityWorldRefusalKind =
  | "unmodelled_provenance_selection"
  | "mechanism_collapse"
  | "macro_score_without_counts"
  | "undeclared_cluster"
  | "competing_event_as_censoring";

export interface OncoEntityWorldSectionResult extends JsonObject {
  allowed: boolean;
  refusal?: JsonObject | null;
  refusal_kind?: OncoWorldsEntityWorldRefusalKind | null;
  refusal_text?: string;
  cluster_refusal?: JsonObject | null;
  cluster_refusal_kind?: "undeclared_cluster" | null;
  event_refusal?: JsonObject | null;
  event_refusal_kind?: "competing_event_as_censoring" | null;
  feasibility?: JsonObject;
  feasibility_kind?: "feasible" | "infeasible_for_classes";
}

export interface OncoWorldsEntityWorldCheckResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/oncoworlds-entity-world-check/0.1";
  outcome_kind?: "report";
  all_admissible: boolean;
  check_count: number;
  refusal_count: number;
  checks: {
    provenance?: OncoEntityWorldSectionResult;
    alterations?: OncoEntityWorldSectionResult;
    benchmark?: OncoEntityWorldSectionResult;
    lesion_analysis?: OncoEntityWorldSectionResult;
  } & JsonObject;
  guarantees?: string[];
  limitations?: string[];
}

export interface StressProfileArgs extends JsonObject {
  cohort: JsonObject;
  stress: JsonObject;
  procedures?: JsonObject[];
}

export interface StressReportArgs extends JsonObject {
  cohort: JsonObject;
  stresses: JsonObject[];
  procedures?: JsonObject[];
}

export type StressFamily = "prevalence_shift" | "batch_effect" | "assay_degradation" | "segmentation_jitter";

export interface StressIdentifiabilityResult extends JsonObject {
  identifiability: "not_applicable" | "separable" | "confounded";
  batch?: string;
  overlap?: number;
  only?: "positive" | "negative";
}

export interface StressSweepPointResult extends JsonObject {
  magnitude: number;
  effective_n: number;
  nominal_n: number;
  unresolved: number;
  analysable_prevalence: number;
  abandoned: boolean;
}

export interface StressFindingResult extends JsonObject {
  conclusion_id: string;
  character: "discriminative" | "calibrated" | "geometric";
  obligation: "required" | "probed";
  relation: string;
  rationale: string;
  held_through: number | null;
  broke_at: number | null;
  expected_at_break?: string | null;
  observed_at_break?: string | null;
}

export interface StressGeneratorDefectResult extends JsonObject {
  magnitude: number;
  invariant: string;
  expected: string;
  observed: string;
}

export interface StressProfileResult extends JsonObject {
  family: StressFamily;
  blueprint_module: string;
  stress_id: string;
  cohort_id: string;
  parent_digest: string;
  identifiability: StressIdentifiabilityResult;
  sweep: StressSweepPointResult[];
  findings: StressFindingResult[];
  generator_defects: StressGeneratorDefectResult[];
  caveat: string;
}

export interface StressProfileToolResult extends JsonObject {
  ok: boolean;
  headline?: string;
  profile?: StressProfileResult;
  guarantees?: string[];
  limitations?: string[];
  stage?: "stress_profile";
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
}

export interface StressReportBodyResult extends JsonObject {
  cohort_id: string;
  profiles: StressProfileResult[];
}

export interface StressReportToolResult extends JsonObject {
  ok: boolean;
  headline?: string;
  report?: StressReportBodyResult;
  worst_family?: StressProfileResult | null;
  guarantees?: string[];
  limitations?: string[];
  stage?: "stress_report";
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
}

export interface InfluenceAnalyzeArgs extends JsonObject {
  label: string;
  variables: { [name: string]: number };
  assumed_variables?: string[];
  factors: JsonObject[];
  free: string[];
  factor?: string;
  factor_group?: string[];
  perturbation: JsonObject;
  budget?: JsonObject;
  execute?: boolean;
}

export interface InfluenceBoundResult extends JsonObject {
  kind: "bounded";
  value: number;
  metric: "total_variation_on_normalised_answer";
  method: string;
  approximation: "exact" | "conservative_upper_bound";
  validity: string;
}

export interface InfluenceUnknownResult extends JsonObject {
  kind: "unknown";
  reason: JsonObject;
}

export type InfluenceEstimateResult = InfluenceBoundResult | InfluenceUnknownResult;

export interface InfluenceMethodOutcomeResult extends JsonObject {
  method: string;
  value?: number;
  declined?: JsonObject;
}

export interface InfluenceAnalysisBodyResult extends JsonObject {
  subject: string[];
  perturbation: JsonObject;
  estimate: InfluenceEstimateResult;
  attempted: InfluenceMethodOutcomeResult[];
}

export interface InfluenceRegionResult extends JsonObject {
  label: string;
  variables: { [name: string]: number };
  free: string[];
  bound: string[];
  factors: JsonObject[];
  has_tables: boolean;
  joint_entries: number;
  free_entries: number;
  assumed_cardinality_fraction: number;
}

export interface InfluenceAnalyzeResult extends JsonObject {
  ok: boolean;
  region: InfluenceRegionResult;
  subjects: string[];
  perturbation: JsonObject;
  execute: boolean;
  analysis: InfluenceAnalysisBodyResult;
  looseness: number | null;
  guarantees: string[];
}

export interface RoutingDecideArgs extends JsonObject {
  fingerprint: JsonObject;
  evidence: JsonObject[];
  policy: JsonObject;
  task_id?: string;
}

export interface RoutingArchitectureResult extends JsonObject {
  kind: "full_context" | "graph_k_hop" | "hypergraph_component" | "query_graph" | "lexical_top_k" | "fiber_compiled";
  depth?: number;
  k?: number;
}

export interface RoutingDecisionReasonResult extends JsonObject {
  reason: "routed" | "insufficient_coverage" | "insufficient_margin";
  margin?: number;
  supporting_tasks?: number;
  eligible_architectures?: number;
  neighbouring_observations?: number;
  runner_up?: RoutingArchitectureResult;
}

export interface RoutingArchitectureScoreResult extends JsonObject {
  architecture: RoutingArchitectureResult;
  observations: number;
  distinct_tasks: number;
  mean_utility: number;
  admissible_rate: number;
}

export interface RoutingDecisionResult extends JsonObject {
  architecture: RoutingArchitectureResult;
  confidence: number;
  abstained: boolean;
  reason: RoutingDecisionReasonResult;
  considered: RoutingArchitectureScoreResult[];
}

export interface RoutingEvidenceSummaryResult extends JsonObject {
  observations: number;
  distinct_tasks: number;
  neighbourhood_observations: number;
  neighbourhood_radius: number;
}

export interface RoutingToolResult extends JsonObject {
  ok: boolean;
  decision: RoutingDecisionResult;
  task_id: string | null;
  holdout_check: "enforced" | "caller_must_supply_unseen_identity";
  evidence: RoutingEvidenceSummaryResult;
  guarantees: string[];
}

export interface RoutingLabRunArgs extends JsonObject {
  tasks: JsonObject[];
  settings: JsonObject;
  include_rows?: boolean;
  max_rows?: number;
}

export interface RoutingLabRunResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/routing-lab-run/0.1";
  tasks?: number;
  holdout?: "task" | "regime";
  holdout_label?: string;
  approved_architectures?: string[];
  fixed_default?: RoutingArchitectureResult;
  include_rows?: boolean;
  report?: {
    account: JsonObject;
    calibration: JsonObject;
    verdict: "router_loses_to_fixed_default" | "no_achievable_gain" | "router_matches_fixed_default" | "router_beats_fixed_default";
    abstention_rate: number;
    oracle_agreement_rate?: number | null;
    tasks_won: number;
    tasks_lost: number;
    tasks_tied: number;
    caveats: string[];
    task_rows: JsonObject[];
    task_rows_omitted: number;
  };
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface LabParetoAuditArgs extends JsonObject {
  objectives: JsonObject[];
  profiles: JsonObject[];
  relations?: JsonObject[];
  max_rows?: number;
}

export interface LabParetoSelectionResult extends JsonObject {
  selection: "unique" | "ambiguous" | "empty";
  candidate?: string;
  front?: string[];
  unresolved?: JsonObject[];
}

export interface LabParetoFrontResult extends JsonObject {
  count: number;
  members: JsonObject[];
  unresolved_count: number;
  unresolved: JsonObject[];
  selection: LabParetoSelectionResult;
}

export interface LabParetoAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/lab-pareto-audit/0.1";
  objective_count?: number;
  profile_count?: number;
  objectives?: JsonObject[];
  admissions?: JsonObject[];
  admissions_omitted?: number;
  front?: LabParetoFrontResult;
  archived_count?: number;
  archived?: JsonObject[];
  archived_omitted?: number;
  relations?: JsonObject[];
  relations_omitted?: number;
  max_rows?: number;
  stage?: string;
  profile_index?: number;
  candidate?: string;
  refusal?: string;
  error?: JsonObject;
  fail_closed?: boolean;
  inserted_profiles?: number;
  guarantees: string[];
  limitations?: string[];
}

export interface LabBranchAuditArgs extends JsonObject {
  policy: JsonObject;
  decisions: JsonObject[];
  max_rows?: number;
}

export interface LabBranchYieldResult extends JsonObject {
  decisions: number;
  escalations: number;
  escalations_on_undetermined: number;
  spent: JsonObject;
  catches: number;
  wasted_escalations: number;
  escaped_after_escalation: number;
  escaped_without_escalation: number;
  branches_per_catch: number | null;
}

export interface LabBranchVerdictResult extends JsonObject {
  verdict: "nothing_triggered" | "paid_and_caught_nothing" | "mixed" | "every_escalation_caught_something";
  spent?: JsonObject;
  escalations?: number;
  catches?: number;
  wasted_escalations?: number;
}

export interface LabBranchAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/lab-branch-audit/0.1";
  policy?: JsonObject;
  decision_count?: number;
  yield?: LabBranchYieldResult;
  verdict?: LabBranchVerdictResult;
  rows?: JsonObject[];
  rows_omitted?: number;
  max_rows?: number;
  stage?: string;
  refusal?: string;
  error?: JsonObject;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface LabHoldoutAuditArgs extends JsonObject {
  cost_ceiling: number;
  candidates: JsonObject[];
  holdouts: JsonObject[];
  current: string;
  operations: JsonObject[];
  max_rows?: number;
}

export interface LabHoldoutAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/lab-holdout-audit/0.1";
  current?: string;
  space?: JsonObject;
  holdouts?: JsonObject[];
  remaining_certification_budget?: JsonObject[];
  checkpoints?: JsonObject[];
  checkpoint_count?: number;
  history?: JsonObject[];
  operations?: JsonObject[];
  operations_omitted?: number;
  operation_count?: number;
  measurement_count?: number;
  measurement_refusal_count?: number;
  rollback_count?: number;
  permanently_burned?: JsonObject[];
  max_rows?: number;
  stage?: string;
  refusal?: string;
  error?: JsonObject;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface LabEvolutionAuditArgs extends JsonObject {
  cost_ceiling: number;
  candidates: JsonObject[];
  baseline: string;
  candidate: string;
  holdout: JsonObject;
  measurements: JsonObject[];
  card_id: string;
  proposal: JsonObject;
  rollback_handle: string;
  direction: "higher_is_better" | "lower_is_better";
  would_have_to_be_true: string[];
  max_rows?: number;
}

export interface LabEvolutionAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/lab-evolution-audit/0.1";
  status?: "improvement_claimed" | "contaminated" | "claim_refused";
  claimable?: boolean;
  card?: JsonObject;
  claim?: JsonObject;
  sentence?: string;
  claim_refusal?: string;
  claim_error?: JsonObject;
  measurement_count?: number;
  measurement_rows?: JsonObject[];
  measurement_rows_omitted?: number;
  max_rows?: number;
  stage?: string;
  candidate_index?: number;
  refusal?: string;
  error?: JsonObject;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface LabSpaceAuditArgs extends JsonObject {
  cost_ceiling: number;
  candidates: JsonObject[];
  inspect?: string[];
  comparisons?: JsonObject[];
  include_components?: boolean;
  max_rows?: number;
}

export interface LabSpaceAuditResult extends JsonObject {
  ok: boolean;
  schema?: "bioprism-mcp/lab-space-audit/0.1";
  cost_ceiling?: number;
  candidate_count?: number;
  registered_count?: number;
  space_committed?: boolean;
  space?: JsonObject;
  candidate_rows?: JsonObject[];
  candidate_rows_omitted?: number;
  inspection_count?: number;
  inspection_rows?: JsonObject[];
  inspection_rows_omitted?: number;
  comparison_count?: number;
  comparison_rows?: JsonObject[];
  comparison_rows_omitted?: number;
  max_rows?: number;
  stage?: string;
  candidate_index?: number;
  refusal?: string;
  error?: JsonObject;
  fail_closed?: boolean;
  guarantees: string[];
  limitations?: string[];
}

export interface ProviderCapabilityGateArgs extends JsonObject {
  card: JsonObject;
  required: string[];
  other_card?: JsonObject;
  include_card?: boolean;
}

export type ProviderClaimStateResult =
  | { state: "untested" }
  | { state: "failed"; witness: string; run: JsonObject }
  | { state: "passed"; run: JsonObject };

export interface ProviderGateResult extends JsonObject {
  outcome: "cleared" | "blocked";
  unproven?: string[];
}

export type ProviderDriftResult =
  | { drift: "agree" }
  | { drift: "differ"; left: string; right: string }
  | { drift: "indeterminate"; untested: string[] };

export interface ProviderCapabilityGateResult extends JsonObject {
  ok: boolean;
  provider: string | null;
  required: string[];
  required_states: { [name: string]: ProviderClaimStateResult };
  gate: ProviderGateResult;
  claims: string[];
  measurement_count: number;
  differential: { [check: string]: ProviderDriftResult } | null;
  card?: JsonObject | null;
  guarantees: string[];
}

export interface SdkRegistryCheckArgs extends JsonObject {
  manifests: JsonObject[];
  policy?: JsonObject;
}

export interface SdkRegistryManifestRowResult extends JsonObject {
  index: number;
  valid: boolean;
  id?: string;
  refusal?: string;
  validation_error?: string | null;
  digest?: string | null;
  core_digest?: string | null;
  capability_kinds?: string[];
  trust?: JsonObject | null;
}

export interface SdkRegistryRegistrationResult extends JsonObject {
  id: string;
  digest: string;
  core_digest: string;
  negotiated: JsonObject;
  trust: JsonObject;
  load_bearing_selectable: boolean;
}

export interface SdkRegistryBodyResult extends JsonObject {
  registration_count: number;
  resolution: { [kind: string]: JsonObject };
  registrations: SdkRegistryRegistrationResult[];
  policy: JsonObject;
}

export interface SdkRegistryCheckResult extends JsonObject {
  ok: boolean;
  stage?: "manifest_validation" | "registry_registration";
  refusal?: string;
  fail_closed?: boolean;
  manifest_count?: number;
  manifests: SdkRegistryManifestRowResult[];
  registry: SdkRegistryBodyResult | null;
  conformance_note?: string;
  guarantees: string[];
}

export type ToolArguments = JsonObject;
