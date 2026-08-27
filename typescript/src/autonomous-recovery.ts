import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * A provider-free recovery handoff for a completed, held, or failed autonomous run.
 *
 * The planner is intentionally not a retry loop. It turns the small status/failure projection
 * that a queue or UI already has into explicit next actions while keeping provider, credential,
 * source, tool, evaluator, and effect authority with the caller. In particular, an unavailable
 * provider is never represented as a successful run and a retry recommendation never authorizes
 * a second external call.
 */
export const AUTONOMOUS_RECOVERY_PLAN_SCHEMA = "bioprism-typescript-autonomous-recovery-plan/0.1" as const;
export const AUTONOMOUS_RECOVERY_RETENTION = "metadata_only_recovery_handoff;task_prompt_provider_response_credentials_and_effect_values_not_retained" as const;
export const AUTONOMOUS_RECOVERY_AUTHORITY = "guidance_only;does_not_authorize_retry_provider_source_tool_evaluator_or_effect" as const;
export const AUTONOMOUS_RECOVERY_MAX_ACTIONS = 16;
export const AUTONOMOUS_RECOVERY_MAX_REASON_CODES = 16;
export const AUTONOMOUS_RECOVERY_MAX_CAPABILITY_BYTES = 256;
export const AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA = "bioprism-typescript-autonomous-recovery-handoff/0.1" as const;
export const AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-recovery-handoff-snapshot/0.1" as const;
export const AUTONOMOUS_RECOVERY_HANDOFF_RETENTION = "metadata_only_recovery_handoff;run_identity_is_digest_bound;tasks_prompts_credentials_provider_values_and_effects_not_retained" as const;
export const AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY = "review_queue_only;review_does_not_execute_retry_reconcile_provider_tool_or_effect" as const;
export const AUTONOMOUS_RECOVERY_HANDOFF_STATUSES = ["queued", "retry_approved", "reconciliation_required", "escalated", "closed"] as const;
export const AUTONOMOUS_RECOVERY_REVIEW_DECISIONS = ["approve_retry", "approve_reconciliation", "escalate", "close"] as const;
export const AUTONOMOUS_RECOVERY_HANDOFF_MAX_ITEMS = 4096;
export const AUTONOMOUS_RECOVERY_HANDOFF_MAX_SNAPSHOT_BYTES = 10_000_000;

export const AUTONOMOUS_RECOVERY_ACTIONS = [
  "complete",
  "retry_provider",
  "refresh_provider_health",
  "collect_credential",
  "approve_provider_call",
  "review_route",
  "review_domain_policy",
  "review_response_quality",
  "review_tool_authorization",
  "reconcile_external_effect",
  "retry_after_review",
  "stop_and_escalate",
] as const;

export type AutonomousRecoveryAction = typeof AUTONOMOUS_RECOVERY_ACTIONS[number];
export type AutonomousRecoveryStatus = "completed" | "retryable" | "held" | "reconciliation_required" | "blocked";
export type AutonomousRecoveryHandoffStatus = typeof AUTONOMOUS_RECOVERY_HANDOFF_STATUSES[number];
export type AutonomousRecoveryReviewDecision = typeof AUTONOMOUS_RECOVERY_REVIEW_DECISIONS[number];

export interface AutonomousRecoveryObservation {
  domain: AutonomousDomainName;
  capability: string;
  status: string;
  failure_class?: string | null;
  failure_code?: string | null;
  retryable?: boolean;
  retry_count?: number;
  max_retries?: number;
  approval_required?: boolean;
  reconciliation_required?: boolean;
  provider_configured?: boolean;
  credential_ready?: boolean;
  route_reviewed?: boolean;
  policy_admitted?: boolean;
  response_quality_passed?: boolean | null;
  tool_authorization_ready?: boolean;
}

export interface AutonomousRecoveryPlan extends JsonObject {
  schema: typeof AUTONOMOUS_RECOVERY_PLAN_SCHEMA;
  domain: AutonomousDomainName;
  capability: string;
  observed_status: string;
  status: AutonomousRecoveryStatus;
  next_action: AutonomousRecoveryAction;
  actions: AutonomousRecoveryAction[];
  retryable: boolean;
  retry_count: number;
  max_retries: number;
  reason_codes: string[];
  domain_guardrails: string[];
  authority: typeof AUTONOMOUS_RECOVERY_AUTHORITY;
  retention: typeof AUTONOMOUS_RECOVERY_RETENTION;
  secret_material: "never_returned";
  plan_digest: string;
}

/** The only identity a recovery queue accepts from the caller: a digest of a private run id. */
export interface AutonomousRecoveryHandoffSubmission {
  plan: AutonomousRecoveryPlan | unknown;
  run_id_digest: string;
  attempt: number;
}

/** Metadata-only queue record. It never carries the transient observation or its values. */
export interface AutonomousRecoveryHandoff extends JsonObject {
  schema: typeof AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA;
  handoff_id: string;
  run_id_digest: string;
  attempt: number;
  plan_digest: string;
  domain: AutonomousDomainName;
  capability: string;
  plan_status: AutonomousRecoveryStatus;
  recommended_action: AutonomousRecoveryAction;
  actions: AutonomousRecoveryAction[];
  retry_count: number;
  max_retries: number;
  status: AutonomousRecoveryHandoffStatus;
  selected_action: AutonomousRecoveryAction | null;
  revision: number;
  last_decision: AutonomousRecoveryReviewDecision | null;
  reviewer_digest: string | null;
  transition_digest: string;
  authority: typeof AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY;
  retention: typeof AUTONOMOUS_RECOVERY_HANDOFF_RETENTION;
  secret_material: "never_returned";
  handoff_digest: string;
}

export interface AutonomousRecoveryHandoffSubmissionResult extends JsonObject {
  schema: typeof AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA;
  status: "accepted" | "duplicate";
  handoff: AutonomousRecoveryHandoff;
  retained_count: number;
  retention: typeof AUTONOMOUS_RECOVERY_HANDOFF_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousRecoveryHandoffReview {
  handoff_id: string;
  decision: AutonomousRecoveryReviewDecision;
  expected_revision: number;
  reviewer_digest: string;
}

export interface AutonomousRecoveryHandoffReviewResult extends JsonObject {
  schema: typeof AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA;
  status: "reviewed";
  decision: AutonomousRecoveryReviewDecision;
  handoff: AutonomousRecoveryHandoff;
  retention: typeof AUTONOMOUS_RECOVERY_HANDOFF_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousRecoveryHandoffSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA;
  entries: AutonomousRecoveryHandoff[];
  generation: number;
  previous_snapshot_digest: string | null;
  retention: typeof AUTONOMOUS_RECOVERY_HANDOFF_RETENTION;
  secret_material: "never_returned";
  snapshot_digest: string;
}

export interface AutonomousRecoveryHandoffPersistence {
  read(): Promise<unknown | null> | unknown | null;
  write(snapshot: unknown): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: unknown): Promise<boolean> | boolean;
}

export interface AutonomousRecoveryHandoffTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousRecoveryHandoffTransactionalTextStore extends AutonomousRecoveryHandoffTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

const OBSERVATION_KEYS = new Set([
  "domain", "capability", "status", "failure_class", "failure_code", "retryable", "retry_count",
  "max_retries", "approval_required", "reconciliation_required", "provider_configured", "credential_ready",
  "route_reviewed", "policy_admitted", "response_quality_passed", "tool_authorization_ready",
]);

const PLAN_KEYS = new Set([
  "schema", "domain", "capability", "observed_status", "status", "next_action", "actions", "retryable",
  "retry_count", "max_retries", "reason_codes", "domain_guardrails", "authority", "retention",
  "secret_material", "plan_digest",
]);

const SECRET_KEYS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "headers", "messages", "password",
  "prompt", "request", "response", "secret", "task", "token", "privatekey", "rawpayload", "arguments", "output",
]);

const DOMAIN_GUARDRAILS: Record<AutonomousDomainName, readonly string[]> = {
  coding: ["report_verification_that_actually_ran", "preserve_rollback_and_diff_review"],
  browser: ["recheck_source_identity_and_freshness", "do_not_treat_page_access_as_truth"],
  data: ["recheck_schema_and_provenance", "report_missingness_before_interpretation"],
  science: ["separate_hypothesis_from_observation", "preserve_uncertainty_and_reproduction"],
  biomedical: ["require_qualified_human_review_for_high_impact_claims", "do_not_diagnose_or_prescribe"],
  neuroscience: ["preserve_specimen_and_coordinate_scope", "escalate_interpretive_uncertainty"],
  operations: ["require_operator_approval_before_effects", "preserve_stop_conditions_and_rollback"],
  enterprise: ["recheck_owner_and_policy_scope", "keep_external_effects_separately_authorized"],
  multi_agent: ["retain_one_accountable_coordinator", "reconcile_specialist_dissent_before_synthesis"],
  multimodal: ["identify_uninspected_modalities", "do_not_infer_absent_observations"],
  cross_domain: ["reconcile_domain_scopes_before_synthesis", "keep_claims_attached_to_specialists"],
  evaluation: ["keep_evaluator_independent_of_subject", "preserve_holdout_and_replay_evidence"],
};

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || value.length > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value.trim();
}

function boundedIdentifier(name: string, value: unknown, maximum = 256): string {
  const normalized = boundedText(name, value, maximum);
  if (!/^[A-Za-z0-9_.:-]+$/.test(normalized)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return normalized;
}

function boundedBoolean(name: string, value: unknown, fallback: boolean): boolean {
  if (value === undefined || value === null) return fallback;
  if (typeof value !== "boolean") throw new ArgumentError(`${name} must be boolean`);
  return value;
}

function boundedCount(name: string, value: unknown, fallback: number, maximum = 64): number {
  const normalized = value === undefined || value === null ? fallback : value;
  if (!Number.isSafeInteger(normalized) || (normalized as number) < 0 || (normalized as number) > maximum) throw new ArgumentError(`${name} must be an integer within [0, ${maximum}]`);
  return normalized as number;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function exactKeys(name: string, value: Record<string, unknown>, expected: readonly string[]): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function boundedActions(name: string, values: readonly unknown[]): AutonomousRecoveryAction[] {
  if (!Array.isArray(values) || values.length < 1 || values.length > AUTONOMOUS_RECOVERY_MAX_ACTIONS) throw new ArgumentError(`${name} must contain 1..${AUTONOMOUS_RECOVERY_MAX_ACTIONS} actions`);
  const normalized = values.map((value) => {
    if (typeof value !== "string" || !(AUTONOMOUS_RECOVERY_ACTIONS as readonly string[]).includes(value)) throw new ArgumentError(`${name} contains an unsupported action`);
    return value as AutonomousRecoveryAction;
  });
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} contains duplicate actions`);
  return normalized;
}

function boundedStringList(name: string, values: readonly unknown[], maximum: number): string[] {
  if (!Array.isArray(values) || values.length > maximum) throw new ArgumentError(`${name} exceeds its bounded list size`);
  const normalized = values.map((value) => boundedIdentifier(`${name} item`, value, 256));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} contains duplicate items`);
  return normalized;
}

function assertNoSecretShape(value: unknown, depth = 0): void {
  if (depth > 8) throw new ArgumentError("autonomous recovery metadata is too deeply nested");
  if (Array.isArray(value)) {
    for (const item of value) assertNoSecretShape(item, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
    if (SECRET_KEYS.has(normalized)) throw new ArgumentError("autonomous recovery metadata contains transient or secret-shaped fields");
    assertNoSecretShape(child, depth + 1);
  }
}

function normalizedObservation(input: AutonomousRecoveryObservation): Required<AutonomousRecoveryObservation> {
  if (!isObject(input)) throw new ArgumentError("autonomous recovery observation must be an object");
  if (Object.keys(input).some((key) => !OBSERVATION_KEYS.has(key))) throw new ArgumentError("autonomous recovery observation contains unsupported fields");
  assertNoSecretShape(input);
  const domain = input.domain;
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError("autonomous recovery observation domain is unsupported");
  const capability = boundedIdentifier("autonomous recovery capability", input.capability, AUTONOMOUS_RECOVERY_MAX_CAPABILITY_BYTES);
  const status = boundedIdentifier("autonomous recovery status", input.status, 128);
  const failureClass = input.failure_class === undefined || input.failure_class === null ? null : boundedIdentifier("autonomous recovery failure_class", input.failure_class, 128);
  const failureCode = input.failure_code === undefined || input.failure_code === null ? null : boundedIdentifier("autonomous recovery failure_code", input.failure_code, 128);
  const retryCount = boundedCount("autonomous recovery retry_count", input.retry_count, 0);
  const maxRetries = boundedCount("autonomous recovery max_retries", input.max_retries, 2);
  if (retryCount > maxRetries) throw new ArgumentError("autonomous recovery retry_count exceeds max_retries");
  const responseQualityPassed = input.response_quality_passed;
  if (responseQualityPassed !== undefined && responseQualityPassed !== null && typeof responseQualityPassed !== "boolean") {
    throw new ArgumentError("autonomous recovery response_quality_passed must be boolean or null");
  }
  return {
    domain,
    capability,
    status,
    failure_class: failureClass,
    failure_code: failureCode,
    retryable: boundedBoolean("autonomous recovery retryable", input.retryable, false),
    retry_count: retryCount,
    max_retries: maxRetries,
    approval_required: boundedBoolean("autonomous recovery approval_required", input.approval_required, false),
    reconciliation_required: boundedBoolean("autonomous recovery reconciliation_required", input.reconciliation_required, false),
    provider_configured: boundedBoolean("autonomous recovery provider_configured", input.provider_configured, true),
    credential_ready: boundedBoolean("autonomous recovery credential_ready", input.credential_ready, true),
    route_reviewed: boundedBoolean("autonomous recovery route_reviewed", input.route_reviewed, true),
    policy_admitted: boundedBoolean("autonomous recovery policy_admitted", input.policy_admitted, true),
    response_quality_passed: responseQualityPassed === undefined ? null : responseQualityPassed,
    tool_authorization_ready: boundedBoolean("autonomous recovery tool_authorization_ready", input.tool_authorization_ready, true),
  };
}

function unique(values: readonly string[]): string[] {
  return [...new Set(values)];
}

/** Build a deterministic recovery handoff from value-only execution metadata. */
export function planAutonomousRecovery(input: AutonomousRecoveryObservation): AutonomousRecoveryPlan {
  const observation = normalizedObservation(input);
  const failure = `${observation.failure_class ?? ""} ${observation.failure_code ?? ""}`.toLowerCase();
  let status: AutonomousRecoveryStatus;
  let nextAction: AutonomousRecoveryAction;
  let actions: AutonomousRecoveryAction[];
  const reasons: string[] = [];

  if (["completed", "children_completed"].includes(observation.status)) {
    status = "completed";
    nextAction = "complete";
    actions = ["complete"];
    reasons.push("run_completed");
  } else if (observation.reconciliation_required || observation.status === "reconciliation_required" || /reconcil/.test(failure)) {
    status = "reconciliation_required";
    nextAction = "reconcile_external_effect";
    actions = ["reconcile_external_effect", "stop_and_escalate"];
    reasons.push("external_state_is_uncertain");
  } else if (observation.approval_required || observation.status === "approval_required" || /approval|approve/.test(failure)) {
    status = "held";
    nextAction = "approve_provider_call";
    actions = ["approve_provider_call", "review_tool_authorization", "stop_and_escalate"];
    reasons.push("explicit_approval_is_missing");
  } else if (!observation.policy_admitted || observation.status === "policy_blocked" || observation.status === "policy_review_required" || /policy/.test(failure)) {
    status = "held";
    nextAction = "review_domain_policy";
    actions = ["review_domain_policy", "stop_and_escalate"];
    reasons.push("domain_policy_is_not_admitted");
  } else if (!observation.route_reviewed || observation.status === "route_review_required" || observation.status === "abstained") {
    status = "held";
    nextAction = "review_route";
    actions = ["review_route", "stop_and_escalate"];
    reasons.push("route_requires_review");
  } else if (!observation.provider_configured || /configuration|provider_missing|not_configured/.test(failure)) {
    status = "blocked";
    nextAction = "stop_and_escalate";
    actions = ["stop_and_escalate"];
    reasons.push("provider_configuration_is_missing");
  } else if (!observation.credential_ready || /credential|authentication|unauthorized|forbidden/.test(failure)) {
    status = "blocked";
    nextAction = "collect_credential";
    actions = ["collect_credential", "retry_provider", "stop_and_escalate"];
    reasons.push("caller_credential_is_not_ready");
  } else if (observation.response_quality_passed === false || observation.status === "response_review_required" || /quality|response_review/.test(failure)) {
    status = "held";
    nextAction = "review_response_quality";
    actions = ["review_response_quality", "retry_after_review", "stop_and_escalate"];
    reasons.push("response_quality_requires_explicit_review");
  } else if (!observation.tool_authorization_ready || /tool.*author|authorization_required/.test(failure)) {
    status = "held";
    nextAction = "review_tool_authorization";
    actions = ["review_tool_authorization", "retry_after_review", "stop_and_escalate"];
    reasons.push("tool_authorization_is_not_ready");
  } else if (observation.retryable && observation.retry_count >= observation.max_retries) {
    status = "blocked";
    nextAction = "stop_and_escalate";
    actions = ["stop_and_escalate"];
    reasons.push("retry_budget_exhausted");
  } else if (observation.retryable && observation.retry_count < observation.max_retries) {
    status = "retryable";
    nextAction = "retry_provider";
    actions = ["retry_provider", "refresh_provider_health", "stop_and_escalate"];
    reasons.push("bounded_retry_budget_remains");
  } else if (["timeout", "transport", "http_5xx", "circuit_open", "provider_error"].some((code) => failure.includes(code))) {
    status = "blocked";
    nextAction = "refresh_provider_health";
    actions = ["refresh_provider_health", "stop_and_escalate"];
    reasons.push("provider_failure_is_not_retryable_in_this_context");
  } else if (observation.status === "turn_limit_reached" || observation.status === "child_failed" || observation.status === "cross_domain_partial") {
    status = "held";
    nextAction = "retry_after_review";
    actions = ["retry_after_review", "review_route", "stop_and_escalate"];
    reasons.push("bounded_execution_did_not_reach_a_complete_result");
  } else {
    status = "blocked";
    nextAction = "stop_and_escalate";
    actions = ["stop_and_escalate"];
    reasons.push("unclassified_failure_requires_review");
  }

  const planBody = {
    schema: AUTONOMOUS_RECOVERY_PLAN_SCHEMA,
    domain: observation.domain,
    capability: observation.capability,
    observed_status: observation.status,
    status,
    next_action: nextAction,
    actions: boundedActions("autonomous recovery actions", actions),
    retryable: status === "retryable" || actions.includes("retry_provider") || actions.includes("retry_after_review"),
    retry_count: observation.retry_count,
    max_retries: observation.max_retries,
    reason_codes: unique(reasons),
    domain_guardrails: [...DOMAIN_GUARDRAILS[observation.domain]],
    authority: AUTONOMOUS_RECOVERY_AUTHORITY,
    retention: AUTONOMOUS_RECOVERY_RETENTION,
    secret_material: "never_returned" as const,
  };
  return { ...planBody, plan_digest: digestJsonSync(planBody) };
}

/** Validate a recovery plan before persisting it or handing it to a queue/UI. */
export function validateAutonomousRecoveryPlan(value: unknown): AutonomousRecoveryPlan {
  if (!isObject(value)) throw new ArgumentError("autonomous recovery plan must be an object");
  if (Object.keys(value).some((key) => !PLAN_KEYS.has(key))) throw new ArgumentError("autonomous recovery plan contains unsupported fields");
  assertNoSecretShape(value);
  const domain = value.domain as AutonomousDomainName;
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError("autonomous recovery plan domain is unsupported");
  const capability = boundedIdentifier("autonomous recovery plan capability", value.capability, AUTONOMOUS_RECOVERY_MAX_CAPABILITY_BYTES);
  const observedStatus = boundedIdentifier("autonomous recovery plan observed_status", value.observed_status, 128);
  const status = boundedIdentifier("autonomous recovery plan status", value.status, 64) as AutonomousRecoveryStatus;
  if (!["completed", "retryable", "held", "reconciliation_required", "blocked"].includes(status)) throw new ArgumentError("autonomous recovery plan status is invalid");
  if (typeof value.next_action !== "string" || !(AUTONOMOUS_RECOVERY_ACTIONS as readonly string[]).includes(value.next_action)) throw new ArgumentError("autonomous recovery plan next_action is invalid");
  const nextAction = value.next_action as AutonomousRecoveryAction;
  const actions = boundedActions("autonomous recovery plan actions", value.actions as unknown[]);
  if (actions[0] !== nextAction) throw new ArgumentError("autonomous recovery plan next_action must be the first action");
  const retryable = boundedBoolean("autonomous recovery plan retryable", value.retryable, false);
  const retryCount = boundedCount("autonomous recovery plan retry_count", value.retry_count, 0);
  const maxRetries = boundedCount("autonomous recovery plan max_retries", value.max_retries, 2);
  if (retryCount > maxRetries) throw new ArgumentError("autonomous recovery plan retry_count exceeds max_retries");
  const reasonCodes = boundedStringList("autonomous recovery plan reason_codes", value.reason_codes as unknown[], AUTONOMOUS_RECOVERY_MAX_REASON_CODES);
  const guardrails = boundedStringList("autonomous recovery plan domain_guardrails", value.domain_guardrails as unknown[], 8);
  if (guardrails.some((guardrail) => !DOMAIN_GUARDRAILS[domain].includes(guardrail))) {
    throw new ArgumentError("autonomous recovery plan domain_guardrails do not match the domain contract");
  }
  if (value.schema !== AUTONOMOUS_RECOVERY_PLAN_SCHEMA || value.authority !== AUTONOMOUS_RECOVERY_AUTHORITY || value.retention !== AUTONOMOUS_RECOVERY_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("autonomous recovery plan retention markers are invalid");
  const body = { schema: AUTONOMOUS_RECOVERY_PLAN_SCHEMA, domain, capability, observed_status: observedStatus, status, next_action: nextAction, actions, retryable, retry_count: retryCount, max_retries: maxRetries, reason_codes: reasonCodes, domain_guardrails: guardrails, authority: AUTONOMOUS_RECOVERY_AUTHORITY, retention: AUTONOMOUS_RECOVERY_RETENTION, secret_material: "never_returned" as const };
  if (value.plan_digest !== digestJsonSync(body)) throw new ArgumentError("autonomous recovery plan digest does not match metadata");
  return { ...body, plan_digest: value.plan_digest as string };
}

function handoffIdentityDigest(runIdDigest: string, attempt: number, planDigest: string): string {
  return digestJsonSync({ schema: AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA, run_id_digest: runIdDigest, attempt, plan_digest: planDigest });
}

function handoffBody(value: Omit<AutonomousRecoveryHandoff, "handoff_digest">): Omit<AutonomousRecoveryHandoff, "handoff_digest"> {
  const { handoff_digest: _ignored, ...body } = value as AutonomousRecoveryHandoff;
  return body;
}

function transitionDigest(handoffId: string, previousHandoffDigest: string | null, decision: AutonomousRecoveryReviewDecision | null, reviewerDigest: string | null, status: AutonomousRecoveryHandoffStatus, revision: number): string {
  return digestJsonSync({ handoff_id: handoffId, previous_handoff_digest: previousHandoffDigest, decision, reviewer_digest: reviewerDigest, status, revision });
}

function validateHandoff(value: unknown): AutonomousRecoveryHandoff {
  if (!isObject(value)) throw new ArgumentError("autonomous recovery handoff must be an object");
  exactKeys("autonomous recovery handoff", value, [
    "schema", "handoff_id", "run_id_digest", "attempt", "plan_digest", "domain", "capability", "plan_status",
    "recommended_action", "actions", "retry_count", "max_retries", "status", "selected_action", "revision",
    "last_decision", "reviewer_digest", "transition_digest", "authority", "retention", "secret_material", "handoff_digest",
  ]);
  assertNoSecretShape(value);
  if (value.schema !== AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA || value.authority !== AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY || value.retention !== AUTONOMOUS_RECOVERY_HANDOFF_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("autonomous recovery handoff markers are invalid");
  const runIdDigest = boundedDigest("autonomous recovery handoff run_id_digest", value.run_id_digest)!;
  const attempt = boundedCount("autonomous recovery handoff attempt", value.attempt, 0);
  const planDigest = boundedDigest("autonomous recovery handoff plan_digest", value.plan_digest)!;
  const handoffId = boundedDigest("autonomous recovery handoff handoff_id", value.handoff_id)!;
  if (handoffId !== handoffIdentityDigest(runIdDigest, attempt, planDigest)) throw new ArgumentError("autonomous recovery handoff identity does not match its digests");
  const domain = value.domain as AutonomousDomainName;
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError("autonomous recovery handoff domain is unsupported");
  const capability = boundedIdentifier("autonomous recovery handoff capability", value.capability, AUTONOMOUS_RECOVERY_MAX_CAPABILITY_BYTES);
  const planStatus = boundedIdentifier("autonomous recovery handoff plan_status", value.plan_status, 64) as AutonomousRecoveryStatus;
  if (!["completed", "retryable", "held", "reconciliation_required", "blocked"].includes(planStatus)) throw new ArgumentError("autonomous recovery handoff plan_status is invalid");
  if (typeof value.recommended_action !== "string" || !(AUTONOMOUS_RECOVERY_ACTIONS as readonly string[]).includes(value.recommended_action)) throw new ArgumentError("autonomous recovery handoff recommended_action is invalid");
  const recommendedAction = value.recommended_action as AutonomousRecoveryAction;
  const actions = boundedActions("autonomous recovery handoff actions", value.actions as unknown[]);
  if (actions[0] !== recommendedAction) throw new ArgumentError("autonomous recovery handoff recommended_action must be the first action");
  const retryCount = boundedCount("autonomous recovery handoff retry_count", value.retry_count, 0);
  const maxRetries = boundedCount("autonomous recovery handoff max_retries", value.max_retries, 2);
  if (retryCount > maxRetries) throw new ArgumentError("autonomous recovery handoff retry_count exceeds max_retries");
  if (typeof value.status !== "string" || !(AUTONOMOUS_RECOVERY_HANDOFF_STATUSES as readonly string[]).includes(value.status)) throw new ArgumentError("autonomous recovery handoff status is invalid");
  const status = value.status as AutonomousRecoveryHandoffStatus;
  const selectedAction = value.selected_action === null ? null : (typeof value.selected_action === "string" && (AUTONOMOUS_RECOVERY_ACTIONS as readonly string[]).includes(value.selected_action) ? value.selected_action as AutonomousRecoveryAction : (() => { throw new ArgumentError("autonomous recovery handoff selected_action is invalid"); })());
  if (selectedAction !== null && !actions.includes(selectedAction)) throw new ArgumentError("autonomous recovery handoff selected_action is not available in actions");
  const revision = boundedCount("autonomous recovery handoff revision", value.revision, 1, 2_147_483_647);
  if (revision < 1) throw new ArgumentError("autonomous recovery handoff revision must be positive");
  const lastDecision = value.last_decision === null ? null : (typeof value.last_decision === "string" && (AUTONOMOUS_RECOVERY_REVIEW_DECISIONS as readonly string[]).includes(value.last_decision) ? value.last_decision as AutonomousRecoveryReviewDecision : (() => { throw new ArgumentError("autonomous recovery handoff last_decision is invalid"); })());
  const reviewerDigest = boundedDigest("autonomous recovery handoff reviewer_digest", value.reviewer_digest, true);
  if (status === "queued" && (lastDecision !== null || reviewerDigest !== null || selectedAction !== null)) throw new ArgumentError("queued recovery handoff contains a review decision");
  if (status === "retry_approved" && (lastDecision !== "approve_retry" || reviewerDigest === null || selectedAction === null || !["retry_provider", "retry_after_review"].includes(selectedAction))) throw new ArgumentError("retry-approved recovery handoff is inconsistent");
  if (status === "reconciliation_required" && (lastDecision !== "approve_reconciliation" || reviewerDigest === null || selectedAction !== "reconcile_external_effect")) throw new ArgumentError("reconciliation recovery handoff is inconsistent");
  if (status === "escalated" && (lastDecision !== "escalate" || reviewerDigest === null || selectedAction !== "stop_and_escalate")) throw new ArgumentError("escalated recovery handoff is inconsistent");
  if (status === "closed" && lastDecision !== null && (lastDecision !== "close" || reviewerDigest === null)) throw new ArgumentError("closed recovery handoff decision is inconsistent");
  const transition = boundedDigest("autonomous recovery handoff transition_digest", value.transition_digest)!;
  const handoffDigest = boundedDigest("autonomous recovery handoff handoff_digest", value.handoff_digest)!;
  const body = { schema: AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA, handoff_id: handoffId, run_id_digest: runIdDigest, attempt, plan_digest: planDigest, domain, capability, plan_status: planStatus, recommended_action: recommendedAction, actions, retry_count: retryCount, max_retries: maxRetries, status, selected_action: selectedAction, revision, last_decision: lastDecision, reviewer_digest: reviewerDigest, transition_digest: transition, authority: AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY, retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" as const };
  if (handoffDigest !== digestJsonSync(handoffBody(body))) throw new ArgumentError("autonomous recovery handoff digest does not match metadata");
  return { ...body, handoff_digest: handoffDigest };
}

function validateHandoffSnapshot(value: unknown): AutonomousRecoveryHandoffSnapshot {
  if (!isObject(value)) throw new ArgumentError("autonomous recovery handoff snapshot must be an object");
  exactKeys("autonomous recovery handoff snapshot", value, ["schema", "entries", "generation", "previous_snapshot_digest", "retention", "secret_material", "snapshot_digest"]);
  assertNoSecretShape(value);
  if (value.schema !== AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA || value.retention !== AUTONOMOUS_RECOVERY_HANDOFF_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("autonomous recovery handoff snapshot markers are invalid");
  if (!Array.isArray(value.entries) || value.entries.length > AUTONOMOUS_RECOVERY_HANDOFF_MAX_ITEMS) throw new ArgumentError("autonomous recovery handoff snapshot entries are outside their bound");
  const entries = value.entries.map(validateHandoff);
  if (new Set(entries.map((entry) => entry.handoff_id)).size !== entries.length || [...entries].sort((left, right) => left.handoff_id.localeCompare(right.handoff_id)).some((entry, index) => entry.handoff_id !== entries[index]!.handoff_id)) throw new ArgumentError("autonomous recovery handoff snapshot entries are not unique and ordered");
  const generation = boundedCount("autonomous recovery handoff snapshot generation", value.generation, 1, 2_147_483_647);
  if (generation < 1) throw new ArgumentError("autonomous recovery handoff snapshot generation must be positive");
  const previous = boundedDigest("autonomous recovery handoff snapshot previous_snapshot_digest", value.previous_snapshot_digest, true);
  if ((generation === 1) !== (previous === null)) throw new ArgumentError("autonomous recovery handoff snapshot generation and predecessor are inconsistent");
  const snapshotDigest = boundedDigest("autonomous recovery handoff snapshot snapshot_digest", value.snapshot_digest)!;
  const body = { schema: AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA, entries, generation, previous_snapshot_digest: previous, retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" as const };
  if (snapshotDigest !== digestJsonSync(body)) throw new ArgumentError("autonomous recovery handoff snapshot digest does not match metadata");
  const normalized = { ...body, snapshot_digest: snapshotDigest };
  if (new TextEncoder().encode(canonicalJson(normalized)).byteLength > AUTONOMOUS_RECOVERY_HANDOFF_MAX_SNAPSHOT_BYTES) throw new ArgumentError("autonomous recovery handoff snapshot exceeds its byte bound");
  return normalized;
}

function handoffSubmission(plan: AutonomousRecoveryPlan, runIdDigest: string, attempt: number): AutonomousRecoveryHandoff {
  const handoffId = handoffIdentityDigest(runIdDigest, attempt, plan.plan_digest);
  const completed = plan.status === "completed";
  const status: AutonomousRecoveryHandoffStatus = completed ? "closed" : "queued";
  const selectedAction: AutonomousRecoveryAction | null = completed ? "complete" : null;
  const transition = transitionDigest(handoffId, null, null, null, status, 1);
  const body = { schema: AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA, handoff_id: handoffId, run_id_digest: runIdDigest, attempt, plan_digest: plan.plan_digest, domain: plan.domain, capability: plan.capability, plan_status: plan.status, recommended_action: plan.next_action, actions: plan.actions, retry_count: plan.retry_count, max_retries: plan.max_retries, status, selected_action: selectedAction, revision: 1, last_decision: null, reviewer_digest: null, transition_digest: transition, authority: AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY, retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" as const };
  return { ...body, handoff_digest: digestJsonSync(handoffBody(body)) };
}

/** Bounded metadata-only handoff queue for explicit human/deployment review. */
export class AutonomousRecoveryHandoffLedger {
  private entriesValue = new Map<string, AutonomousRecoveryHandoff>();
  private generation = 0;
  private previousSnapshotDigest: string | null = null;
  private cachedSnapshot: AutonomousRecoveryHandoffSnapshot | null = null;
  private cachedSignature: string | null = null;

  get(handoffId: string): AutonomousRecoveryHandoff | null {
    const id = boundedDigest("autonomous recovery handoff id", handoffId);
    const value = this.entriesValue.get(id!);
    return value === undefined ? null : structuredClone(value);
  }

  entries(options: { status?: AutonomousRecoveryHandoffStatus; domain?: AutonomousDomainName; limit?: number } = {}): AutonomousRecoveryHandoff[] {
    const limit = boundedCount("autonomous recovery handoff list limit", options.limit, AUTONOMOUS_RECOVERY_HANDOFF_MAX_ITEMS, AUTONOMOUS_RECOVERY_HANDOFF_MAX_ITEMS);
    if (limit < 1) throw new ArgumentError("autonomous recovery handoff list limit must be positive");
    if (options.status !== undefined && !(AUTONOMOUS_RECOVERY_HANDOFF_STATUSES as readonly string[]).includes(options.status)) throw new ArgumentError("autonomous recovery handoff list status is invalid");
    if (options.domain !== undefined && !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(options.domain)) throw new ArgumentError("autonomous recovery handoff list domain is invalid");
    return [...this.entriesValue.values()].filter((entry) => (options.status === undefined || entry.status === options.status) && (options.domain === undefined || entry.domain === options.domain)).sort((left, right) => left.handoff_id.localeCompare(right.handoff_id)).slice(0, limit).map((entry) => structuredClone(entry));
  }

  submit(input: AutonomousRecoveryHandoffSubmission): AutonomousRecoveryHandoffSubmissionResult {
    if (!isObject(input)) throw new ArgumentError("autonomous recovery handoff submission must be an object");
    const plan = validateAutonomousRecoveryPlan(input.plan);
    const runIdDigest = boundedDigest("autonomous recovery handoff submission run_id_digest", input.run_id_digest)!;
    const attempt = boundedCount("autonomous recovery handoff submission attempt", input.attempt, 64);
    const handoff = handoffSubmission(plan, runIdDigest, attempt);
    const existing = this.entriesValue.get(handoff.handoff_id);
    if (existing !== undefined) {
      if (existing.handoff_digest !== handoff.handoff_digest) throw new ArgumentError("autonomous recovery handoff identity conflicts with retained metadata");
      return { schema: AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA, status: "duplicate", handoff: structuredClone(existing), retained_count: this.entriesValue.size, retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" };
    }
    if (this.entriesValue.size >= AUTONOMOUS_RECOVERY_HANDOFF_MAX_ITEMS) throw new ArgumentError("autonomous recovery handoff ledger is at capacity");
    this.entriesValue.set(handoff.handoff_id, handoff);
    this.invalidate();
    return { schema: AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA, status: "accepted", handoff: structuredClone(handoff), retained_count: this.entriesValue.size, retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" };
  }

  review(input: AutonomousRecoveryHandoffReview): AutonomousRecoveryHandoffReviewResult {
    if (!isObject(input)) throw new ArgumentError("autonomous recovery handoff review must be an object");
    const handoffId = boundedDigest("autonomous recovery handoff review handoff_id", input.handoff_id)!;
    if (typeof input.decision !== "string" || !(AUTONOMOUS_RECOVERY_REVIEW_DECISIONS as readonly string[]).includes(input.decision)) throw new ArgumentError("autonomous recovery handoff review decision is invalid");
    const decision = input.decision as AutonomousRecoveryReviewDecision;
    const expectedRevision = boundedCount("autonomous recovery handoff review expected_revision", input.expected_revision, 2_147_483_647, 2_147_483_647);
    const reviewerDigest = boundedDigest("autonomous recovery handoff review reviewer_digest", input.reviewer_digest)!;
    const current = this.entriesValue.get(handoffId);
    if (current === undefined) throw new ArgumentError("autonomous recovery handoff is not retained");
    if (current.revision !== expectedRevision) throw new ArgumentError("autonomous recovery handoff review revision is stale");
    if (current.status !== "queued") throw new ArgumentError("autonomous recovery handoff is already reviewed");
    if (decision === "approve_retry" && (current.recommended_action === "collect_credential" || (!current.actions.includes("retry_provider") && !current.actions.includes("retry_after_review")))) throw new ArgumentError("autonomous recovery handoff does not authorize a retry review");
    if (decision === "approve_reconciliation" && !current.actions.includes("reconcile_external_effect")) throw new ArgumentError("autonomous recovery handoff does not require reconciliation");
    const status: AutonomousRecoveryHandoffStatus = decision === "approve_retry" ? "retry_approved" : decision === "approve_reconciliation" ? "reconciliation_required" : decision === "escalate" ? "escalated" : "closed";
    const selectedAction = decision === "approve_retry" ? (current.actions.includes("retry_provider") ? "retry_provider" : "retry_after_review") : decision === "approve_reconciliation" ? "reconcile_external_effect" : decision === "escalate" ? "stop_and_escalate" : null;
    const transition = transitionDigest(current.handoff_id, current.handoff_digest, decision, reviewerDigest, status, current.revision + 1);
    const body = { ...current, status, selected_action: selectedAction, revision: current.revision + 1, last_decision: decision, reviewer_digest: reviewerDigest, transition_digest: transition };
    const next = validateHandoff({ ...body, handoff_digest: digestJsonSync(handoffBody(body)) });
    this.entriesValue.set(next.handoff_id, next);
    this.invalidate();
    return { schema: AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA, status: "reviewed", decision, handoff: structuredClone(next), retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" };
  }

  snapshot(): AutonomousRecoveryHandoffSnapshot {
    const entries = this.entries();
    const signature = entries.map((entry) => entry.handoff_digest).join(":");
    if (this.cachedSnapshot !== null && this.cachedSignature === signature) return structuredClone(this.cachedSnapshot);
    const body = { schema: AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA, entries, generation: this.generation + 1, previous_snapshot_digest: this.previousSnapshotDigest, retention: AUTONOMOUS_RECOVERY_HANDOFF_RETENTION, secret_material: "never_returned" as const };
    const snapshot = validateHandoffSnapshot({ ...body, snapshot_digest: digestJsonSync(body) });
    this.generation = snapshot.generation; this.previousSnapshotDigest = snapshot.snapshot_digest; this.cachedSnapshot = structuredClone(snapshot); this.cachedSignature = signature;
    return structuredClone(snapshot);
  }

  restore(raw: unknown): void {
    const snapshot = validateHandoffSnapshot(raw);
    this.entriesValue = new Map(snapshot.entries.map((entry) => [entry.handoff_id, entry]));
    this.generation = snapshot.generation; this.previousSnapshotDigest = snapshot.snapshot_digest; this.cachedSnapshot = structuredClone(snapshot); this.cachedSignature = snapshot.entries.map((entry) => entry.handoff_digest).join(":");
  }

  private invalidate(): void { this.cachedSnapshot = null; this.cachedSignature = null; }
}

export class JsonAutonomousRecoveryHandoffPersistence {
  constructor(readonly store: AutonomousRecoveryHandoffTextStore, readonly maxBytes = AUTONOMOUS_RECOVERY_HANDOFF_MAX_SNAPSHOT_BYTES) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("autonomous recovery handoff JSON persistence requires a text store");
    boundedCount("autonomous recovery handoff persistence maxBytes", maxBytes, AUTONOMOUS_RECOVERY_HANDOFF_MAX_SNAPSHOT_BYTES, AUTONOMOUS_RECOVERY_HANDOFF_MAX_SNAPSHOT_BYTES);
  }
  async read(): Promise<AutonomousRecoveryHandoffSnapshot | null> { const value = await this.store.read(); if (value === null) return null; if (typeof value !== "string" || new TextEncoder().encode(value).byteLength > this.maxBytes) throw new ArgumentError("autonomous recovery handoff JSON exceeds its byte bound"); let parsed: unknown; try { parsed = JSON.parse(value); } catch { throw new ArgumentError("autonomous recovery handoff JSON is invalid"); } if (canonicalJson(parsed) !== value) throw new ArgumentError("autonomous recovery handoff JSON is not canonical"); return validateHandoffSnapshot(parsed); }
  async write(snapshot: unknown): Promise<void> { const normalized = validateHandoffSnapshot(snapshot); const encoded = canonicalJson(normalized); if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) throw new ArgumentError("autonomous recovery handoff JSON exceeds its byte bound"); await this.store.write(encoded); }
}

export class TransactionalJsonAutonomousRecoveryHandoffPersistence extends JsonAutonomousRecoveryHandoffPersistence {
  declare readonly store: AutonomousRecoveryHandoffTransactionalTextStore;
  constructor(store: AutonomousRecoveryHandoffTransactionalTextStore, maxBytes = AUTONOMOUS_RECOVERY_HANDOFF_MAX_SNAPSHOT_BYTES) { super(store, maxBytes); this.store = store; if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional autonomous recovery handoff persistence requires writeIfUnchanged"); }
  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: unknown): Promise<boolean> { if (expectedSnapshotDigest !== null) boundedDigest("autonomous recovery handoff expectedSnapshotDigest", expectedSnapshotDigest); return this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validateHandoffSnapshot(snapshot))); }
}

export class AutonomousRecoveryHandoffPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  constructor(readonly ledger: AutonomousRecoveryHandoffLedger, readonly persistence: AutonomousRecoveryHandoffPersistence) {
    if (!(ledger instanceof AutonomousRecoveryHandoffLedger)) throw new ArgumentError("autonomous recovery handoff coordinator requires a handoff ledger");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous recovery handoff persistence is malformed");
  }
  async restore(): Promise<AutonomousRecoveryHandoffSnapshot | null> { const raw = await this.persistence.read(); if (raw === null) { this.expectedSnapshotDigest = null; return null; } const snapshot = validateHandoffSnapshot(raw); this.ledger.restore(snapshot); this.expectedSnapshotDigest = snapshot.snapshot_digest; return snapshot; }
  async flush(): Promise<AutonomousRecoveryHandoffSnapshot> { const snapshot = this.ledger.snapshot(); if (typeof this.persistence.writeIfUnchanged === "function") { if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("autonomous recovery handoff persistence compare-and-swap conflict"); } else await this.persistence.write(snapshot); this.expectedSnapshotDigest = snapshot.snapshot_digest; return snapshot; }
}

export function validateAutonomousRecoveryHandoff(value: unknown): AutonomousRecoveryHandoff { return validateHandoff(value); }
export function validateAutonomousRecoveryHandoffSnapshot(value: unknown): AutonomousRecoveryHandoffSnapshot { return validateHandoffSnapshot(value); }
