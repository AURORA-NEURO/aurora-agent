import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import { digestJsonSync } from "./tooling.js";
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

function boundedCount(name: string, value: unknown, fallback: number): number {
  const normalized = value === undefined || value === null ? fallback : value;
  if (!Number.isSafeInteger(normalized) || (normalized as number) < 0 || (normalized as number) > 64) throw new ArgumentError(`${name} must be an integer within [0, 64]`);
  return normalized as number;
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
