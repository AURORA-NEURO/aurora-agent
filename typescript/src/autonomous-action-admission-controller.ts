import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousActionAdmission,
  AutonomousActionPlan,
  type AutonomousActionAdmissionJSON,
  type AutonomousActionPlanJSON,
  admitAutonomousActionPlan,
} from "./autonomous-action-plan.js";
import {
  InMemoryAutonomousActionAdmissionLedger,
  validateAutonomousActionAdmissionRecord,
  type AutonomousActionAdmissionRecord,
  type AutonomousActionAdmissionReviewOptions,
} from "./autonomous-action-admission-persistence.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";
import type { AutonomousActionPlanApproval } from "./autonomous-action-plan.js";

/** Bounded operator review projection; it never includes the task or a runtime payload. */
export const AUTONOMOUS_ACTION_REVIEW_QUEUE_SCHEMA = "bioprism-typescript-autonomous-action-review-queue/0.1" as const;
export const AUTONOMOUS_ACTION_REVIEW_ROW_SCHEMA = "bioprism-typescript-autonomous-action-review-row/0.1" as const;
export const AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA = "bioprism-typescript-autonomous-action-dispatch-handoff/0.1" as const;
export const AUTONOMOUS_ACTION_REVIEW_RETENTION = "metadata_only;operator_review_projection_and_digests;task_prompt_provider_connector_credential_and_effect_values_not_retained" as const;
export const AUTONOMOUS_ACTION_REVIEW_AUTHORITY = "caller_operator_projection_only;authorization_is_external_and_not_verified_by_sdk" as const;
export const AUTONOMOUS_ACTION_REVIEW_EXECUTION = "review_control_only;does_not_authorize_provider_source_tool_effect_or_credentials" as const;
export const AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL = "never_returned" as const;
export const AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES = ["credential_scope", "provider_or_source_approval", "tool_and_effect_authority", "evaluator_settlement"] as const;

export interface AutonomousActionReviewRow extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_REVIEW_ROW_SCHEMA;
  action_id: string;
  revision: number;
  status: "pending_review" | "admitted" | "blocked";
  plan_digest: string;
  admission_digest: string;
  route_digest: string;
  selected_domains: AutonomousDomainName[];
  cross_domain: boolean;
  execution_path: string;
  next_action: string;
  next_actions: string[];
  required_approvals: string[];
  approved_approvals: string[];
  missing_approvals: string[];
  review_reasons: string[];
  blocking_reasons: string[];
  reviewer_digest: string | null;
  reason_digest: string | null;
  record_digest: string;
  authority: typeof AUTONOMOUS_ACTION_REVIEW_AUTHORITY;
  retention: typeof AUTONOMOUS_ACTION_REVIEW_RETENTION;
  execution: typeof AUTONOMOUS_ACTION_REVIEW_EXECUTION;
  secret_material: typeof AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL;
}

export interface AutonomousActionReviewQueue extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_REVIEW_QUEUE_SCHEMA;
  rows: AutonomousActionReviewRow[];
  counts: {
    total: number;
    pending_review: number;
    admitted: number;
    blocked: number;
  };
  domain_counts: Record<AutonomousDomainName, number>;
  queue_digest: string;
  authority: typeof AUTONOMOUS_ACTION_REVIEW_AUTHORITY;
  retention: typeof AUTONOMOUS_ACTION_REVIEW_RETENTION;
  execution: typeof AUTONOMOUS_ACTION_REVIEW_EXECUTION;
  secret_material: typeof AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL;
}

export interface AutonomousActionDispatchHandoff extends JsonObject {
  schema: typeof AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA;
  action_id: string;
  record_digest: string;
  plan_digest: string;
  admission_digest: string;
  plan: AutonomousActionPlanJSON;
  admission: AutonomousActionAdmissionJSON;
  selected_domains: AutonomousDomainName[];
  requested_domains: AutonomousDomainName[];
  cross_domain: boolean;
  execution_path: string;
  status: "ready_for_downstream_gates";
  downstream_gates: string[];
  authority: typeof AUTONOMOUS_ACTION_REVIEW_AUTHORITY;
  retention: typeof AUTONOMOUS_ACTION_REVIEW_RETENTION;
  execution: typeof AUTONOMOUS_ACTION_REVIEW_EXECUTION;
  secret_material: typeof AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL;
  handoff_digest: string;
}

export interface AutonomousActionOperatorReviewOptions extends Omit<AutonomousActionAdmissionReviewOptions, "reviewerDigest"> {
  authorizationDigest: string;
}

export interface AutonomousActionOperatorSubmitOptions {
  approvals?: Partial<Record<AutonomousActionPlanApproval, boolean>>;
  reviewed?: boolean;
  authorizationDigest?: string | null;
  reason?: string | null;
}

function fail(message: string): never {
  throw new ArgumentError(`autonomous action review controller ${message}`);
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function domainList(name: string, value: readonly string[]): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > AUTONOMOUS_DOMAIN_NAMES.length) fail(`${name} must contain one to twelve domains`);
  const result = value.map((domain) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) fail(`${name} contains an unsupported domain`);
    return domain as AutonomousDomainName;
  });
  if (new Set(result).size !== result.length) fail(`${name} must contain unique domains`);
  return result;
}

const DISPATCH_HANDOFF_KEYS = [
  "schema", "action_id", "record_digest", "plan_digest", "admission_digest", "plan", "admission",
  "selected_domains", "requested_domains", "cross_domain", "execution_path", "status", "downstream_gates",
  "authority", "retention", "execution", "secret_material", "handoff_digest",
] as const;

function exactHandoffKeys(value: Record<string, unknown>): void {
  const allowed = new Set<string>(DISPATCH_HANDOFF_KEYS);
  if (Object.keys(value).some((key) => !allowed.has(key)) || DISPATCH_HANDOFF_KEYS.some((key) => !(key in value))) fail("handoff has unsupported or missing fields");
}

function actionId(value: unknown): string {
  if (typeof value !== "string" || value.length < 1 || value.length > 256 || !/^[A-Za-z0-9_.:+/-]+$/.test(value)) fail("handoff action_id is invalid");
  return value;
}

/**
 * Verify a handoff after it has crossed a persistence or process boundary.
 *
 * This proves only metadata continuity. It deliberately does not prove reviewer authorization,
 * credential availability, provider readiness, source truth, evaluator quality, or effect safety;
 * those remain independent downstream gates.
 */
export function validateAutonomousActionDispatchHandoff(value: unknown): AutonomousActionDispatchHandoff {
  if (!isObject(value)) fail("handoff must be an object");
  exactHandoffKeys(value);
  if (value.schema !== AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA || value.authority !== AUTONOMOUS_ACTION_REVIEW_AUTHORITY || value.retention !== AUTONOMOUS_ACTION_REVIEW_RETENTION || value.execution !== AUTONOMOUS_ACTION_REVIEW_EXECUTION || value.secret_material !== AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL) fail("handoff markers are invalid");
  const plan = AutonomousActionPlan.fromJSON(value.plan as AutonomousActionPlanJSON);
  const admission = AutonomousActionAdmission.fromJSON(value.admission as AutonomousActionAdmissionJSON);
  const recordDigest = digest("handoff record_digest", value.record_digest) as string;
  const planDigest = digest("handoff plan_digest", value.plan_digest) as string;
  const admissionDigest = digest("handoff admission_digest", value.admission_digest) as string;
  const suppliedHandoffDigest = digest("handoff handoff_digest", value.handoff_digest) as string;
  if (planDigest !== plan.plan_digest || admissionDigest !== admission.admission_digest || plan.plan_digest !== admission.plan_digest) fail("handoff plan or admission digest is inconsistent");
  if (admission.status !== "admitted") fail("handoff admission is not admitted");
  const selected = domainList("handoff selected_domains", value.selected_domains as readonly string[]);
  const requested = domainList("handoff requested_domains", value.requested_domains as readonly string[]);
  if (JSON.stringify(selected) !== JSON.stringify(plan.selected_domains) || JSON.stringify(selected) !== JSON.stringify(admission.selected_domains)) fail("handoff selected domains are inconsistent");
  if (requested.some((domain) => !selected.includes(domain))) fail("handoff requested domains exceed the selected domains");
  if (typeof value.cross_domain !== "boolean" || value.cross_domain !== plan.cross_domain) fail("handoff cross-domain posture is inconsistent");
  if (typeof value.execution_path !== "string" || value.execution_path !== admission.execution_path) fail("handoff execution path is inconsistent");
  if (value.status !== "ready_for_downstream_gates") fail("handoff status is invalid");
  if (!Array.isArray(value.downstream_gates) || JSON.stringify(value.downstream_gates) !== JSON.stringify(AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES)) fail("handoff downstream gates are invalid");
  const body = {
    schema: AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA,
    action_id: actionId(value.action_id),
    record_digest: recordDigest,
    plan_digest: planDigest,
    admission_digest: admissionDigest,
    plan: plan.toJSON(),
    admission: admission.toJSON(),
    selected_domains: selected,
    requested_domains: requested,
    cross_domain: value.cross_domain,
    execution_path: value.execution_path,
    status: "ready_for_downstream_gates" as const,
    downstream_gates: [...AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES],
    authority: AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
    retention: AUTONOMOUS_ACTION_REVIEW_RETENTION,
    execution: AUTONOMOUS_ACTION_REVIEW_EXECUTION,
    secret_material: AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
  };
  const expected = digestJsonSync(body);
  if (suppliedHandoffDigest !== expected) fail("handoff digest does not match metadata");
  return { ...body, handoff_digest: expected };
}

function normalizedRecord(record: AutonomousActionAdmissionRecord): { record: AutonomousActionAdmissionRecord; plan: AutonomousActionPlan; admission: AutonomousActionAdmission } {
  const normalized = validateAutonomousActionAdmissionRecord(record);
  const plan = AutonomousActionPlan.fromJSON(normalized.plan as AutonomousActionPlanJSON);
  const admission = AutonomousActionAdmission.fromJSON(normalized.admission as AutonomousActionAdmissionJSON);
  return { record: normalized, plan, admission };
}

function rowFor(record: AutonomousActionAdmissionRecord): AutonomousActionReviewRow {
  const { record: normalized, plan, admission } = normalizedRecord(record);
  return {
    schema: AUTONOMOUS_ACTION_REVIEW_ROW_SCHEMA,
    action_id: normalized.action_id,
    revision: normalized.revision,
    status: normalized.status,
    plan_digest: plan.plan_digest,
    admission_digest: admission.admission_digest,
    route_digest: plan.route_digest,
    selected_domains: [...plan.selected_domains],
    cross_domain: plan.cross_domain,
    execution_path: admission.execution_path,
    next_action: admission.next_action,
    next_actions: [...admission.next_actions],
    required_approvals: [...admission.required_approvals],
    approved_approvals: [...admission.approved_approvals],
    missing_approvals: [...admission.missing_approvals],
    review_reasons: [...admission.review_reasons],
    blocking_reasons: [...admission.blocking_reasons],
    reviewer_digest: normalized.reviewer_digest,
    reason_digest: normalized.reason_digest,
    record_digest: normalized.record_digest,
    authority: AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
    retention: AUTONOMOUS_ACTION_REVIEW_RETENTION,
    execution: AUTONOMOUS_ACTION_REVIEW_EXECUTION,
    secret_material: AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
  };
}

function queueFor(rows: AutonomousActionReviewRow[]): AutonomousActionReviewQueue {
  const sorted = [...rows].sort((left, right) => left.action_id.localeCompare(right.action_id));
  const domainCounts = Object.fromEntries(AUTONOMOUS_DOMAIN_NAMES.map((domain) => [domain, 0])) as Record<AutonomousDomainName, number>;
  for (const row of sorted) for (const domain of row.selected_domains) domainCounts[domain] += 1;
  const counts = {
    total: sorted.length,
    pending_review: sorted.filter((row) => row.status === "pending_review").length,
    admitted: sorted.filter((row) => row.status === "admitted").length,
    blocked: sorted.filter((row) => row.status === "blocked").length,
  };
  const body = {
    schema: AUTONOMOUS_ACTION_REVIEW_QUEUE_SCHEMA,
    rows: sorted,
    counts,
    domain_counts: domainCounts,
    authority: AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
    retention: AUTONOMOUS_ACTION_REVIEW_RETENTION,
    execution: AUTONOMOUS_ACTION_REVIEW_EXECUTION,
    secret_material: AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
  } as Omit<AutonomousActionReviewQueue, "queue_digest">;
  return { ...body, queue_digest: digestJsonSync(body) } as AutonomousActionReviewQueue;
}

export class AutonomousActionAdmissionController {
  readonly ledger: InMemoryAutonomousActionAdmissionLedger;

  constructor(ledger: InMemoryAutonomousActionAdmissionLedger) {
    if (!(ledger instanceof InMemoryAutonomousActionAdmissionLedger)) fail("requires a typed action admission ledger");
    this.ledger = ledger;
  }

  queue(): AutonomousActionReviewQueue {
    return queueFor(this.ledger.list().map(rowFor));
  }

  get(actionId: string): AutonomousActionReviewRow | null {
    const record = this.ledger.get(actionId);
    return record === null ? null : rowFor(record);
  }

  /**
   * Submit one provider-free plan to the durable review queue.
   *
   * A submitter may attach a caller authorization digest only when the plan is already
   * admitted; ordinary submissions remain pending until the separate review operation.
   */
  submit(actionId: string, source: AutonomousActionPlan | AutonomousActionPlanJSON, options: AutonomousActionOperatorSubmitOptions = {}): AutonomousActionReviewRow {
    if (!options || typeof options !== "object") fail("submit options are malformed");
    const plan = source instanceof AutonomousActionPlan ? source : AutonomousActionPlan.fromJSON(source);
    const admission = admitAutonomousActionPlan(plan, { approvals: options.approvals, reviewed: options.reviewed ?? false });
    const authorizationDigest = options.authorizationDigest === undefined || options.authorizationDigest === null
      ? null
      : digest("authorizationDigest", options.authorizationDigest);
    if (admission.status === "admitted" && authorizationDigest === null) fail("an admitted submission requires an authorizationDigest");
    const record = this.ledger.submit(plan, admission, {
      actionId,
      reviewerDigest: authorizationDigest,
      reason: options.reason,
    });
    return rowFor(record);
  }

  review(actionId: string, options: AutonomousActionOperatorReviewOptions): AutonomousActionReviewRow {
    if (!options || typeof options !== "object") fail("review options are malformed");
    const authorizationDigest = digest("authorizationDigest", options.authorizationDigest);
    const { authorizationDigest: _authorizationDigest, ...reviewOptions } = options;
    const record = this.ledger.review(actionId, { ...reviewOptions, reviewerDigest: authorizationDigest as string });
    return rowFor(record);
  }

  dispatchHandoff(actionId: string, requestedDomains?: readonly AutonomousDomainName[]): AutonomousActionDispatchHandoff {
    const record = this.ledger.get(actionId);
    if (record === null) fail("cannot create a handoff for an unknown action");
    const { record: normalized, plan, admission } = normalizedRecord(record);
    if (normalized.status !== "admitted" || admission.status !== "admitted") fail("action admission is not ready for downstream gates");
    const selected = [...plan.selected_domains];
    const requested = domainList("requestedDomains", requestedDomains === undefined ? selected : requestedDomains);
    const missing = requested.filter((domain) => !selected.includes(domain));
    if (missing.length) fail(`requested domains are outside the admitted plan: ${missing.join(", ")}`);
    const body = {
      schema: AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA,
      action_id: normalized.action_id,
      record_digest: normalized.record_digest,
      plan_digest: plan.plan_digest,
      admission_digest: admission.admission_digest,
      plan: plan.toJSON(),
      admission: admission.toJSON(),
      selected_domains: selected,
      requested_domains: requested,
      cross_domain: plan.cross_domain,
      execution_path: admission.execution_path,
      status: "ready_for_downstream_gates" as const,
      downstream_gates: [...AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES],
      authority: AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
      retention: AUTONOMOUS_ACTION_REVIEW_RETENTION,
      execution: AUTONOMOUS_ACTION_REVIEW_EXECUTION,
      secret_material: AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
    };
    return validateAutonomousActionDispatchHandoff({ ...body, handoff_digest: digestJsonSync(body) });
  }
}
