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
  permitted?: string[];
  disposition?: OncoDispositionResult;
  released?: string[];
  refused?: string[];
  terminal_action?: "stop" | "abstain" | "escalate";
  escalation?: OncoEscalationResult | null;
  research_statement?: string;
  stage?: string;
  refusal?: string;
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
  assessment?: JsonObject;
  call_label?: string;
  withheld_progression?: boolean;
  hypothesis_count?: number;
  evidence_requests?: JsonValue[];
  stage?: string;
  refusal?: string;
  fail_closed?: boolean;
  guarantee?: string;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoWorldlineViewArgs extends JsonObject {
  worldline: JsonObject;
  visible_at?: string;
}

export interface OncoWorldlineResult extends JsonObject {
  ok: boolean;
  subject?: string;
  baseline?: string;
  timepoint_count?: number;
  biological_order?: string[];
  record_order?: string[];
  record_order_differs?: boolean;
  visibility_cutoff?: string | null;
  visibility_filter_applied?: boolean;
  visible_timepoints?: string[] | null;
  hidden_from_agent?: string[] | null;
  timepoints?: JsonObject[];
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoClassificationArgs extends JsonObject {
  histology: JsonValue;
  panel: JsonObject;
}

export interface OncoClassificationResult extends JsonObject {
  ok: boolean;
  histology?: JsonValue;
  resolution?: JsonObject;
  is_integrated?: boolean;
  entity?: string | null;
  obligations?: JsonObject[];
  panel_states?: JsonObject[];
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

export interface OncoIdentityJoinResult extends JsonObject {
  ok: boolean;
  joinable?: boolean;
  report?: JsonObject;
  bridge_declared?: boolean;
  guarantees?: string[];
  limitations?: string[];
}

export interface OncoOutcomeAnalyzeArgs extends JsonObject {
  follow_up: JsonObject;
  estimand: JsonObject;
}

export interface OncoOutcomeResult extends JsonObject {
  ok: boolean;
  analysis?: JsonObject;
  at_risk_days?: number;
  immortal_time_days?: number;
  event?: boolean;
  censoring_reason?: string | null;
  informative_bias_flags?: string[];
  guarantees?: string[];
  limitations?: string[];
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
  subject?: string;
  at?: string;
  status?: "valid" | "invalid" | "underdetermined";
  underdetermined?: boolean;
  deciding_tier?: "deterministic" | "execution" | "property" | "statistical" | "judge" | null;
  judge_only?: boolean;
  suppressed_override?: boolean;
  acceptable?: boolean;
  basis?: JsonValue;
  confidence?: JsonObject | null;
  establishes?: string[];
  does_not_establish?: string[];
  contributing?: JsonValue[];
  omitted_contributing?: number;
  withheld?: JsonValue[];
  omitted_withheld?: number;
  inadmissible?: JsonValue[];
  omitted_inadmissible?: number;
  suppressed?: JsonValue[];
  omitted_suppressed?: number;
  disagreements?: JsonValue[];
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

export interface BioevalReferenceAuditResult extends JsonObject {
  ok: boolean;
  reference?: JsonObject;
  reference_kind?: string;
  can_certify_clean_pass?: boolean;
  resolution?: JsonObject | null;
  modal_state?: string | null;
  modal_mass?: number | null;
  modal_confidence?: number | null;
  entropy_bits?: number | null;
  dispersion?: string | null;
  queried_state?: string | null;
  queried_state_mass?: number | null;
  guarantees?: string[];
  limitations?: string[];
}

export interface EvaluationWorldlineArgs extends JsonObject {
  worldline: JsonObject;
  at?: string;
}

export interface EvaluationWorldlineResult extends JsonObject {
  ok: boolean;
  decisions?: number;
  leak_count?: number;
  leaks?: JsonObject[];
  dangling_count?: number;
  dangling_references?: JsonValue[];
  admissible_at?: JsonValue[] | null;
  guarantees?: string[];
  limitations?: string[];
}

export interface EvaluationReproductionArgs extends JsonObject {
  reexecution: JsonObject;
  biological_claim?: string;
}

export interface EvaluationReproductionResult extends JsonObject {
  ok: boolean;
  certificate?: JsonObject;
  reproduced?: boolean;
  first_divergence?: JsonObject | null;
  missing_outputs?: JsonValue[];
  portability_demonstrated?: boolean;
  validity_claim?: JsonObject | null;
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

export interface EvaluationTrajectoryResult extends JsonObject {
  ok: boolean;
  steps?: number;
  acts?: JsonValue[];
  properties?: JsonValue[];
  property_outcomes?: JsonObject[];
  recovery?: JsonValue[];
  bounded_suffix?: JsonObject | null;
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

export interface RuntimeTapeVerifyResult extends JsonObject {
  ok: boolean;
  run?: string;
  lineage?: JsonObject | null;
  entries?: number;
  head?: string;
  chain_verified?: boolean;
  checkpoint_results?: JsonObject[];
  artifacts?: JsonObject;
  simulated_steps?: number[];
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

export interface RuntimeExecutionSimulateResult extends JsonObject {
  ok: boolean;
  run: string;
  request_count: number;
  recorded_requests: number;
  live_outcomes: JsonValue[];
  execution_error: string | null;
  tape: JsonObject;
  world: JsonObject;
  policy_journal: JsonValue[];
  budget: JsonObject | null;
  replay: JsonObject;
  fork: JsonObject | null;
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

export interface OncoWorldsModelTransportResult extends JsonObject {
  ok: boolean;
  model_statement?: string;
  effective_biological_n?: number;
  patient_relevant_claim?: JsonObject;
  stage?: string;
  refusal?: JsonObject;
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
  classified?: boolean;
  class?: string | null;
  report?: JsonObject;
  stage?: string;
  refusal?: JsonObject;
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

export interface OncoWorldsRadiogenomicCheckResult extends JsonObject {
  ok: boolean;
  supported_claim?: JsonObject;
  stage?: string;
  refusal?: JsonObject;
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

export interface OncoWorldsClonalHistoryCheckResult extends JsonObject {
  ok: boolean;
  compatible_count: number;
  rejected_count: number;
  compatible: JsonValue[];
  rejected: JsonValue[];
  unique_history: JsonObject;
  guarantees: string[];
  limitations: string[];
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
