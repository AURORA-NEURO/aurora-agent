//! Value-only cross-domain execution receipts.
//!
//! This module owns the bounded progress projection used by the one-shot autonomous runner.
//! It is separate from the durable cross-domain execution controller: the receipt contains only
//! child statuses, identifiers, digests, and the next safe action. Provider payloads remain with
//! the caller and are never added to this restart/UI projection.

import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";
import type {
  AutonomousCrossDomainChildRun,
  AutonomousCrossDomainExecutionNextAction,
  AutonomousCrossDomainExecutionReceipt,
  AutonomousCrossDomainExecutionReceiptFields,
  AutonomousCrossDomainRunResult,
  AutonomousCrossDomainRunStatus,
} from "./autonomous-agent-contracts.js";

export const AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN = 8;
export const AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-result/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-execution-receipt/0.1" as const;

function receiptText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) {
    throw new ArgumentError(name + " is outside its bounded text contract");
  }
  return value;
}

function receiptIdentifier(name: string, value: unknown): string {
  const text = receiptText(name, value, 256);
  if (!/^[A-Za-z0-9_.-]+$/.test(text)) throw new ArgumentError(name + " must be a bounded identifier");
  return text;
}

function crossDomainExecutionReceiptDigestPayload(value: AutonomousCrossDomainExecutionReceiptFields): JsonObject {
  return {
    schema: value.schema,
    status: value.status,
    children: value.execution_child_ids.map((id) => ({
      id,
      domain: value.child_domains[id] ?? null,
      status: value.child_statuses[id] ?? null,
      result_digest: value.child_result_digests[id] ?? null,
    })),
    completed_child_ids: value.completed_child_ids,
    incomplete_child_ids: value.incomplete_child_ids,
    synthesis_status: value.synthesis_status,
    synthesis_result_digest: value.synthesis_result_digest,
    completed_units: value.completed_units,
    total_units: value.total_units,
    progress: value.progress,
    next_action: value.next_action,
    safe_to_synthesize: value.safe_to_synthesize,
    reconciliation_required: value.reconciliation_required,
  };
}

function boundedDigest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

const AUTONOMOUS_CROSS_DOMAIN_CHILD_STATUSES = new Set<string>([
  "not_started",
  "completed",
  "route_review_required",
  "approval_required",
  "policy_review_required",
  "policy_blocked",
  "reconciliation_required",
  "turn_limit_reached",
  "abstained",
  "cross_domain_partial",
  "child_failed",
]);

const AUTONOMOUS_CROSS_DOMAIN_RECEIPT_STATUSES = new Set<AutonomousCrossDomainRunStatus>([
  "completed",
  "children_completed",
  "children_partial",
  "approval_required",
  "policy_review_required",
  "policy_blocked",
  "reconciliation_required",
  "turn_limit_reached",
  "child_failed",
  "route_review_required",
  "response_review_required",
]);

const AUTONOMOUS_CROSS_DOMAIN_RECEIPT_ACTIONS = new Set<AutonomousCrossDomainExecutionNextAction>([
  "review_route",
  "approve_child",
  "reconcile_child",
  "retry_child",
  "synthesize",
  "approve_synthesis",
  "reconcile_synthesis",
  "inspect_synthesis_failure",
  "inspect_partial_synthesis",
  "review_response_gate",
  "complete",
]);

function receiptStringArray(name: string, value: unknown, maximum = AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must be a bounded string array`);
  return value.map((item, index) => receiptIdentifier(`${name}[${index}]`, item));
}

function receiptStringMap(name: string, value: unknown): Record<string, string> {
  if (!isObject(value) || Object.keys(value).length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN) throw new ArgumentError(`${name} must be a bounded object`);
  const output: Record<string, string> = {};
  for (const [key, item] of Object.entries(value)) {
    const id = receiptIdentifier(`${name} key`, key);
    output[id] = receiptText(`${name}.${id}`, item, 128);
  }
  return output;
}

function receiptNullableDigestMap(name: string, value: unknown): Record<string, string | null> {
  if (!isObject(value) || Object.keys(value).length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN) throw new ArgumentError(`${name} must be a bounded object`);
  const output: Record<string, string | null> = {};
  for (const [key, item] of Object.entries(value)) {
    const id = receiptIdentifier(`${name} key`, key);
    output[id] = item === null ? null : boundedDigest(`${name}.${id}`, item);
  }
  return output;
}

function receiptDomainMap(value: unknown): Record<string, AutonomousDomainName> {
  if (!isObject(value) || Object.keys(value).length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN) throw new ArgumentError("child_domains must be a bounded object");
  const output: Record<string, AutonomousDomainName> = {};
  for (const [key, item] of Object.entries(value)) {
    const id = receiptIdentifier("child_domains key", key);
    if (typeof item !== "string" || !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(item)) throw new ArgumentError(`child_domains.${id} contains an unknown domain`);
    output[id] = item as AutonomousDomainName;
  }
  return output;
}

function receiptMapKeys(value: Record<string, unknown>): string[] {
  return Object.keys(value).sort();
}

/** Build the value-only cross-domain execution/recovery receipt for a result envelope. */
export async function autonomousCrossDomainExecutionReceipt(result: AutonomousCrossDomainRunResult): Promise<AutonomousCrossDomainExecutionReceipt> {
  if (!isObject(result) || result.schema !== AUTONOMOUS_CROSS_DOMAIN_RESULT_SCHEMA) throw new ArgumentError("cross-domain execution receipt requires a valid result envelope");
  if (!AUTONOMOUS_CROSS_DOMAIN_RECEIPT_STATUSES.has(result.status)) throw new ArgumentError("cross-domain result has an unsupported status");
  if (!Array.isArray(result.child_runs)) throw new ArgumentError("cross-domain result child_runs must be an array");

  const blueprintIds = result.blueprint
    ? result.blueprint.child_ids.map((id, index) => receiptIdentifier(`cross-domain blueprint child id ${index + 1}`, id))
    : [];
  const blueprintDomains = new Map<string, AutonomousDomainName>();
  if (result.blueprint) {
    if (blueprintIds.length !== result.blueprint.child_blueprints.length || new Set(blueprintIds).size !== blueprintIds.length) {
      throw new ArgumentError("cross-domain blueprint child ids must be unique and aligned with child blueprints");
    }
    result.blueprint.child_blueprints.forEach((child, index) => {
      const domain = child?.domain_profile?.domain;
      if (typeof domain !== "string" || !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError(`cross-domain blueprint child ${index + 1} has an unknown domain`);
      blueprintDomains.set(blueprintIds[index] as string, domain as AutonomousDomainName);
    });
  }

  const childRunsById = new Map<string, AutonomousCrossDomainChildRun>();
  for (const [index, child] of result.child_runs.entries()) {
    if (!isObject(child)) throw new ArgumentError(`cross-domain child run ${index + 1} must be an object`);
    const id = receiptIdentifier(`cross-domain child run ${index + 1} id`, child.id);
    if (childRunsById.has(id)) throw new ArgumentError(`cross-domain child run id ${id} is duplicated`);
    if (result.blueprint && !blueprintDomains.has(id)) throw new ArgumentError(`cross-domain child run ${id} is not present in its blueprint`);
    if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(child.domain)) throw new ArgumentError(`cross-domain child run ${id} has an unknown domain`);
    const expectedDomain = blueprintDomains.get(id);
    if (expectedDomain && expectedDomain !== child.domain) throw new ArgumentError(`cross-domain child run ${id} domain does not match its blueprint`);
    if (!isObject(child.result) || !AUTONOMOUS_CROSS_DOMAIN_CHILD_STATUSES.has(child.result.status)) throw new ArgumentError(`cross-domain child run ${id} has an unsupported status`);
    if (child.output_digest !== null) boundedDigest(`cross-domain child run ${id} output_digest`, child.output_digest);
    childRunsById.set(id, child);
  }

  const executionChildIds = result.blueprint
    ? [...childRunsById.keys(), ...blueprintIds.filter((id) => !childRunsById.has(id))]
    : result.route.selected_domains.map((domain) => `route-${receiptIdentifier("cross-domain route domain", domain)}`);
  if (executionChildIds.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN || new Set(executionChildIds).size !== executionChildIds.length) throw new ArgumentError("cross-domain execution child ids exceed the bounded unique contract");

  const childDomains: Record<string, AutonomousDomainName> = {};
  const childStatuses: Record<string, string> = {};
  const childResultDigests: Record<string, string | null> = {};
  for (const id of executionChildIds) {
    const child = childRunsById.get(id);
    const routeDomain = id.startsWith("route-") ? id.slice("route-".length) : null;
    const domain = child?.domain ?? blueprintDomains.get(id) ?? (routeDomain && (AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(routeDomain) ? routeDomain as AutonomousDomainName : null);
    if (!domain) throw new ArgumentError(`cross-domain execution child ${id} has no domain projection`);
    childDomains[id] = domain;
    childStatuses[id] = child?.result.status ?? "not_started";
    childResultDigests[id] = child?.output_digest ?? null;
  }

  const completedChildIds = executionChildIds.filter((id) => childStatuses[id] === "completed");
  const incompleteChildIds = executionChildIds.filter((id) => childStatuses[id] !== "completed");
  const synthesisStatus = result.synthesis?.status ?? null;
  let synthesisResultDigest: string | null = null;
  if (result.synthesis) {
    synthesisResultDigest = await digestJson({
      status: result.synthesis.status,
      task_digest: result.synthesis.blueprint?.task_digest ?? null,
      plan_refinement_digest: result.synthesis.plan_refinement_digest,
      selection: result.synthesis.selection,
      response_evaluation: result.synthesis.response_evaluation ?? null,
      tool_loop_status: result.synthesis.tool_loop?.status ?? null,
    });
  }

  const reconciliationRequired = result.status === "reconciliation_required"
    || Object.values(childStatuses).some((status) => status === "reconciliation_required")
    || synthesisStatus === "reconciliation_required";
  // A partial fan-out must finish/retry its missing child before response alignment review. Once
  // all children are present, or synthesis has already been attempted for an explicit partial
  // run, the response gate is the authoritative next action.
  const responseGateRequiresReview = result.response_assessment !== undefined
    && result.response_assessment !== null
    && result.response_assessment.status !== "ready_to_synthesize"
    && result.response_assessment.status !== "completed"
    && (incompleteChildIds.length === 0 || synthesisStatus !== null);
  const nextAction: AutonomousCrossDomainExecutionNextAction = responseGateRequiresReview
    ? "review_response_gate"
    : result.status === "route_review_required" || result.status === "policy_review_required" || result.status === "policy_blocked"
    ? "review_route"
    : synthesisStatus === "completed" && incompleteChildIds.length === 0
      ? "complete"
      : synthesisStatus === "completed" && incompleteChildIds.length > 0
        ? "inspect_partial_synthesis"
        : synthesisStatus === "approval_required"
          ? "approve_synthesis"
          : synthesisStatus === "reconciliation_required"
            ? "reconcile_synthesis"
            : result.synthesis && synthesisStatus !== null
              ? "inspect_synthesis_failure"
              : incompleteChildIds[0] && childStatuses[incompleteChildIds[0]] === "approval_required"
                ? "approve_child"
                : incompleteChildIds[0] && childStatuses[incompleteChildIds[0]] === "reconciliation_required"
                  ? "reconcile_child"
                  : incompleteChildIds.length > 0
                    ? "retry_child"
                    : result.status === "children_completed"
                      ? "complete"
                      : "synthesize";
  const safeToSynthesize = incompleteChildIds.length === 0
    && result.synthesis === null
    && !responseGateRequiresReview
    && result.status !== "route_review_required"
    && result.status !== "approval_required"
    && result.status !== "policy_review_required"
    && result.status !== "policy_blocked";
  const completedUnits = completedChildIds.length + (synthesisStatus === "completed" ? 1 : 0);
  const totalUnits = Math.max(1, executionChildIds.length + (result.synthesis ? 1 : 0));
  const fields: AutonomousCrossDomainExecutionReceiptFields = {
    schema: AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA,
    status: result.status,
    execution_child_ids: executionChildIds,
    child_domains: childDomains,
    child_statuses: childStatuses,
    child_result_digests: childResultDigests,
    completed_child_ids: completedChildIds,
    incomplete_child_ids: incompleteChildIds,
    synthesis_status: synthesisStatus,
    synthesis_result_digest: synthesisResultDigest,
    completed_units: completedUnits,
    total_units: totalUnits,
    progress: completedUnits / totalUnits,
    next_action: nextAction,
    safe_to_synthesize: safeToSynthesize,
    reconciliation_required: reconciliationRequired,
    retention: "status_and_outcome_digests_only; provider_payloads_caller_owned",
    secret_material: "never_returned",
  };
  return { ...fields, receipt_digest: await digestJson(crossDomainExecutionReceiptDigestPayload(fields)) };
}

/** Validate and recompute a cross-domain execution receipt, rejecting tampered projections. */
export async function validateAutonomousCrossDomainExecutionReceipt(value: unknown): Promise<AutonomousCrossDomainExecutionReceipt> {
  if (!isObject(value)) throw new ArgumentError("cross-domain execution receipt must be an object");
  const allowedKeys = new Set([
    "schema", "status", "execution_child_ids", "child_domains", "child_statuses", "child_result_digests",
    "completed_child_ids", "incomplete_child_ids", "synthesis_status", "synthesis_result_digest", "completed_units",
    "total_units", "progress", "next_action", "safe_to_synthesize", "reconciliation_required", "receipt_digest",
    "retention", "secret_material",
  ]);
  if (Object.keys(value).some((key) => !allowedKeys.has(key)) || allowedKeys.size !== Object.keys(value).length) throw new ArgumentError("cross-domain execution receipt contains unexpected or missing fields");
  if (value.schema !== AUTONOMOUS_CROSS_DOMAIN_EXECUTION_RECEIPT_SCHEMA) throw new ArgumentError("cross-domain execution receipt schema is invalid");
  if (typeof value.status !== "string" || !AUTONOMOUS_CROSS_DOMAIN_RECEIPT_STATUSES.has(value.status as AutonomousCrossDomainRunStatus)) throw new ArgumentError("cross-domain execution receipt status is invalid");
  const executionChildIds = receiptStringArray("execution_child_ids", value.execution_child_ids);
  if (new Set(executionChildIds).size !== executionChildIds.length) throw new ArgumentError("execution_child_ids must be unique");
  const completedChildIds = receiptStringArray("completed_child_ids", value.completed_child_ids);
  const incompleteChildIds = receiptStringArray("incomplete_child_ids", value.incomplete_child_ids);
  if (new Set([...completedChildIds, ...incompleteChildIds]).size !== executionChildIds.length || new Set([...completedChildIds, ...incompleteChildIds]).size !== completedChildIds.length + incompleteChildIds.length || [...completedChildIds, ...incompleteChildIds].some((id) => !executionChildIds.includes(id))) throw new ArgumentError("completed and incomplete child ids must partition execution_child_ids");
  const childDomains = receiptDomainMap(value.child_domains);
  const childStatuses = receiptStringMap("child_statuses", value.child_statuses);
  const childResultDigests = receiptNullableDigestMap("child_result_digests", value.child_result_digests);
  const expectedKeys = [...executionChildIds].sort();
  if (JSON.stringify(receiptMapKeys(childDomains)) !== JSON.stringify(expectedKeys) || JSON.stringify(receiptMapKeys(childStatuses)) !== JSON.stringify(expectedKeys) || JSON.stringify(receiptMapKeys(childResultDigests)) !== JSON.stringify(expectedKeys)) throw new ArgumentError("cross-domain receipt child maps must cover exactly execution_child_ids");
  for (const id of executionChildIds) {
    if (!AUTONOMOUS_CROSS_DOMAIN_CHILD_STATUSES.has(childStatuses[id] as string)) throw new ArgumentError(`child_statuses.${id} is invalid`);
    if ((childStatuses[id] === "completed") !== completedChildIds.includes(id)) throw new ArgumentError(`completed_child_ids does not match child_statuses.${id}`);
    if ((childStatuses[id] !== "completed") !== incompleteChildIds.includes(id)) throw new ArgumentError(`incomplete_child_ids does not match child_statuses.${id}`);
  }
  const synthesisStatus = value.synthesis_status === null ? null : receiptText("synthesis_status", value.synthesis_status, 128);
  if (synthesisStatus !== null && !AUTONOMOUS_CROSS_DOMAIN_CHILD_STATUSES.has(synthesisStatus)) throw new ArgumentError("synthesis_status is invalid");
  const synthesisResultDigest = value.synthesis_result_digest === null ? null : boundedDigest("synthesis_result_digest", value.synthesis_result_digest);
  if ((synthesisStatus === null) !== (synthesisResultDigest === null)) throw new ArgumentError("synthesis status and digest must be present together");
  if (typeof value.completed_units !== "number" || !Number.isSafeInteger(value.completed_units) || value.completed_units !== completedChildIds.length + (synthesisStatus === "completed" ? 1 : 0)) throw new ArgumentError("completed_units is inconsistent with the receipt");
  const expectedTotalUnits = Math.max(1, executionChildIds.length + (synthesisStatus === null ? 0 : 1));
  if (typeof value.total_units !== "number" || !Number.isSafeInteger(value.total_units) || value.total_units !== expectedTotalUnits) throw new ArgumentError("total_units is inconsistent with the receipt");
  if (typeof value.progress !== "number" || !Number.isFinite(value.progress) || value.progress < 0 || value.progress > 1 || value.progress !== value.completed_units / value.total_units) throw new ArgumentError("progress is inconsistent with the receipt");
  if (typeof value.next_action !== "string" || !AUTONOMOUS_CROSS_DOMAIN_RECEIPT_ACTIONS.has(value.next_action as AutonomousCrossDomainExecutionNextAction)) throw new ArgumentError("next_action is invalid");
  const safeToSynthesize = incompleteChildIds.length === 0 && synthesisStatus === null && value.next_action !== "review_response_gate" && value.status !== "route_review_required" && value.status !== "approval_required" && value.status !== "policy_review_required" && value.status !== "policy_blocked" && value.status !== "response_review_required";
  if (value.safe_to_synthesize !== safeToSynthesize) throw new ArgumentError("safe_to_synthesize is inconsistent with the receipt");
  const reconciliationRequired = value.status === "reconciliation_required" || Object.values(childStatuses).some((status) => status === "reconciliation_required") || synthesisStatus === "reconciliation_required";
  if (value.reconciliation_required !== reconciliationRequired) throw new ArgumentError("reconciliation_required is inconsistent with the receipt");
  if (value.retention !== "status_and_outcome_digests_only; provider_payloads_caller_owned" || value.secret_material !== "never_returned") throw new ArgumentError("cross-domain execution receipt retention contract is invalid");
  const fields: AutonomousCrossDomainExecutionReceiptFields = {
    schema: value.schema,
    status: value.status as AutonomousCrossDomainRunStatus,
    execution_child_ids: executionChildIds,
    child_domains: childDomains,
    child_statuses: childStatuses,
    child_result_digests: childResultDigests,
    completed_child_ids: completedChildIds,
    incomplete_child_ids: incompleteChildIds,
    synthesis_status: synthesisStatus,
    synthesis_result_digest: synthesisResultDigest,
    completed_units: value.completed_units,
    total_units: value.total_units,
    progress: value.progress,
    next_action: value.next_action as AutonomousCrossDomainExecutionNextAction,
    safe_to_synthesize: value.safe_to_synthesize,
    reconciliation_required: value.reconciliation_required,
    retention: value.retention,
    secret_material: value.secret_material,
  };
  const receiptDigest = boundedDigest("receipt_digest", value.receipt_digest);
  const expectedDigest = await digestJson(crossDomainExecutionReceiptDigestPayload(fields));
  if (receiptDigest !== expectedDigest) throw new ArgumentError("cross-domain execution receipt digest does not match its fields");
  return { ...fields, receipt_digest: receiptDigest };
}
