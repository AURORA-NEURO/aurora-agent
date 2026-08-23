import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type {
  AutonomousAgent,
  AutonomousDomainName,
  AutonomousReadinessDomain,
  AutonomousReadinessReport,
  AutonomousReadinessState,
} from "./autonomous.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous.js";
import type { AutonomousEvaluatorCalibrationReport } from "./autonomous-evaluator-calibration.js";
import type { AutonomousEvidenceAdapterHealthStore } from "./autonomous-evidence-adapter-health.js";
import type { AutonomousEvidenceAdapterRegistry } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceReadinessAuditOptions } from "./autonomous-evidence-readiness.js";
import type { AutonomousModelCandidate } from "./llm.js";
import type { AutonomousWorkflowPortfolioRunOptions } from "./autonomous-workflow-portfolio-execution.js";
import {
  planAutonomousWorkflowPortfolio,
  validateAutonomousWorkflowPortfolioPlan,
  verifyAutonomousWorkflowPortfolio,
  type AutonomousWorkflowPortfolioItemRequest,
  type AutonomousWorkflowPortfolioItemStatus,
  type AutonomousWorkflowPortfolioPlan,
  type AutonomousWorkflowPortfolioPlanOptions,
} from "./autonomous-workflow-portfolio.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Schema for the provider-free admission decision over a reviewed workflow portfolio. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-admission/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ITEMS = 64;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS = 32;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES = 1_000_000;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-admission-controller/0.1" as const;

export type AutonomousWorkflowPortfolioAdmissionStatus = "ready_for_approval" | "partial" | "blocked";
export type AutonomousWorkflowPortfolioAdmissionItemStatus = "eligible" | "blocked" | "dependency_blocked" | "route_review_required";

export interface AutonomousWorkflowPortfolioAdmissionOptions {
  /** Reuse a caller-reviewed metadata-only plan. When omitted, a new provider-free plan is compiled. */
  plan?: AutonomousWorkflowPortfolioPlan;
  planOptions?: AutonomousWorkflowPortfolioPlanOptions;
  /** Recompile and compare a supplied plan before admission; defaults to true. */
  verifyPlan?: boolean;
  /** Candidate catalogue override used for this projection; no candidate is persisted in the result. */
  candidates?: readonly AutonomousModelCandidate[];
  /** Shared run constraints. Only model/readiness-affecting fields are projected. */
  run?: AutonomousWorkflowPortfolioRunOptions;
  calibrationReport?: AutonomousEvaluatorCalibrationReport;
  requireCalibratedLearning?: boolean;
  evidenceReadiness?: {
    registry: AutonomousEvidenceAdapterRegistry;
    healthStore?: AutonomousEvidenceAdapterHealthStore;
    options?: AutonomousEvidenceReadinessAuditOptions;
  };
  /** Treat missing live domain tools as an admission blocker instead of an actionable warning. */
  requireAvailableTools?: boolean;
}

export interface AutonomousWorkflowPortfolioAdmissionCounts extends JsonObject {
  item_count: number;
  eligible_count: number;
  blocked_count: number;
  dependency_blocked_count: number;
  route_review_required_count: number;
  missing_model_count: number;
  missing_provider_count: number;
  credential_required_count: number;
  calibration_hold_count: number;
  evidence_hold_count: number;
  tool_gap_count: number;
}

export interface AutonomousWorkflowPortfolioAdmissionItem extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA;
  item_id: string;
  domain: AutonomousDomainName;
  depends_on: string[];
  dependency_statuses: Record<string, AutonomousWorkflowPortfolioAdmissionItemStatus>;
  plan_status: AutonomousWorkflowPortfolioItemStatus;
  status: AutonomousWorkflowPortfolioAdmissionItemStatus;
  readiness_state: AutonomousReadinessState | "not_evaluated";
  workflow_digest: string | null;
  plan_digest: string | null;
  request_digest: string;
  required_model_capabilities: string[];
  compatible_model_count: number;
  eligible_model_count: number;
  eligible_model_ids: string[];
  missing_tools: string[];
  blockers: string[];
  next_actions: string[];
  approval: "caller_approval_required_before_provider_dispatch";
  selection: "runtime_reselects_from_the_admitted_catalogue_after_approval";
  retention: "metadata_only_task_and_provider_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioAdmissionPolicy extends JsonObject {
  require_all_domains: boolean;
  allow_partial: boolean;
  verify_plan: boolean;
  require_available_tools: boolean;
  require_calibrated_learning: boolean;
  input_tokens: number;
  output_tokens: number;
}

export interface AutonomousWorkflowPortfolioAdmission extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA;
  status: AutonomousWorkflowPortfolioAdmissionStatus;
  plan: AutonomousWorkflowPortfolioPlan;
  policy: AutonomousWorkflowPortfolioAdmissionPolicy;
  readiness_digest: string;
  items: AutonomousWorkflowPortfolioAdmissionItem[];
  dependency_graph: AutonomousWorkflowPortfolioPlan["dependency_graph"];
  waves: string[][];
  counts: AutonomousWorkflowPortfolioAdmissionCounts;
  next_actions: string[];
  execution: "admission_only;no_provider_tool_connector_or_effect_dispatch";
  authorization: "admission_does_not_authorize_provider_tools_connectors_or_effects";
  retention: "metadata_only_task_and_provider_values_not_retained";
  secret_material: "never_returned";
  admission_digest: string;
}

/** Caller-owned durable persistence for a validated, metadata-only admission image. */
export interface AutonomousWorkflowPortfolioAdmissionPersistence {
  read(): Promise<AutonomousWorkflowPortfolioAdmission | null> | AutonomousWorkflowPortfolioAdmission | null;
  write(admission: AutonomousWorkflowPortfolioAdmission): Promise<void> | void;
}

export interface AutonomousWorkflowPortfolioAdmissionTransactionalPersistence extends AutonomousWorkflowPortfolioAdmissionPersistence {
  writeIfUnchanged(expectedAdmissionDigest: string | null, admission: AutonomousWorkflowPortfolioAdmission): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioAdmissionTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousWorkflowPortfolioAdmissionTransactionalTextStore extends AutonomousWorkflowPortfolioAdmissionTextStore {
  writeIfUnchanged(expectedAdmissionDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousWorkflowPortfolioAdmissionControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_CONTROLLER_SCHEMA;
  status: "empty" | "restored" | "admitted";
  plan_digest: string | null;
  admission_digest: string | null;
  persisted: true;
  retention: "metadata_only_admission_and_plan_digests;tasks_prompts_credentials_and_provider_values_never_persisted";
  secret_material: "never_returned";
}

const HEX_DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9_.:+-]+$/;
const EXECUTION = "admission_only;no_provider_tool_connector_or_effect_dispatch" as const;
const AUTHORIZATION = "admission_does_not_authorize_provider_tools_connectors_or_effects" as const;
const RETENTION = "metadata_only_task_and_provider_values_not_retained" as const;
const APPROVAL = "caller_approval_required_before_provider_dispatch" as const;
const SELECTION = "runtime_reselects_from_the_admitted_catalogue_after_approval" as const;

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.length < 1 || value.length > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!IDENTIFIER.test(text)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return text;
}

function boundedDigest(name: string, value: unknown): string {
  const digest = boundedText(name, value, 64);
  if (!HEX_DIGEST.test(digest)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return digest;
}

function boundedCount(name: string, value: unknown, maximum = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ITEMS): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function boundedPositive(name: string, value: unknown, maximum = 10_000_000): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1 || value > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function sortedUniqueStrings(name: string, values: unknown, maximum = 128): string[] {
  if (!Array.isArray(values) || values.length > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  const result = values.map((value) => boundedText(`${name} entry`, value, 768));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicates`);
  return [...result].sort();
}

function modelId(model: Pick<AutonomousModelCandidate, "provider" | "model">): string {
  return `${model.provider}/${model.model}`;
}

function finiteConstraint(name: string, value: unknown, minimum = 0): number | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function candidateMatchesPolicy(candidate: AutonomousModelCandidate, run: AutonomousWorkflowPortfolioRunOptions | undefined): boolean {
  if (candidate.enabled === false) return false;
  const maxCost = finiteConstraint("workflow portfolio admission maxCostPerMillionTokens", run?.maxCostPerMillionTokens);
  const maxLatency = finiteConstraint("workflow portfolio admission maxLatencyMs", run?.maxLatencyMs);
  const minQuality = finiteConstraint("workflow portfolio admission minQuality", run?.minQuality);
  return (maxCost === undefined || candidate.cost_per_million_tokens <= maxCost)
    && (maxLatency === undefined || candidate.latency_ms <= maxLatency)
    && (minQuality === undefined || candidate.quality >= minQuality);
}

function readinessObject(value: unknown): Record<string, unknown> | null {
  return isObject(value) ? value : null;
}

function readinessBlockers(row: AutonomousReadinessDomain, options: AutonomousWorkflowPortfolioAdmissionOptions): { blockers: string[]; actions: string[]; calibration: boolean; evidence: boolean; tools: boolean } {
  const blockers: string[] = [];
  const actions = [...row.next_actions];
  const calibration = options.requireCalibratedLearning === true && readinessObject(row.calibration_admission)?.decision !== "admit_learning";
  const evidence = row.evidence_readiness !== undefined && readinessObject(row.evidence_readiness)?.status !== "ready";
  const tools = options.requireAvailableTools === true && row.missing_tools.length > 0;
  if (row.state !== "ready_for_caller_approval") blockers.push(`readiness:${row.state}`);
  if (calibration) blockers.push("calibration:hold");
  if (evidence) blockers.push("evidence:not_ready");
  if (tools) blockers.push("tools:missing");
  if (options.run?.minSelectionConfidence !== undefined) actions.push("runtime_selection_confidence_is_checked_again_at_dispatch");
  return { blockers: [...new Set(blockers)].sort(), actions: [...new Set(actions)].sort(), calibration, evidence, tools };
}

function admissionStatusFor(plan: AutonomousWorkflowPortfolioPlan, items: readonly AutonomousWorkflowPortfolioAdmissionItem[]): AutonomousWorkflowPortfolioAdmissionStatus {
  const eligible = items.filter((item) => item.status === "eligible").length;
  if (plan.status === "blocked" || eligible === 0) return "blocked";
  if (plan.status === "partial") return "partial";
  if (eligible !== items.length && plan.policy.allow_partial === false) return "blocked";
  return eligible === items.length ? "ready_for_approval" : "partial";
}

function itemCounts(items: readonly AutonomousWorkflowPortfolioAdmissionItem[]): AutonomousWorkflowPortfolioAdmissionCounts {
  return {
    item_count: items.length,
    eligible_count: items.filter((item) => item.status === "eligible").length,
    blocked_count: items.filter((item) => item.status === "blocked").length,
    dependency_blocked_count: items.filter((item) => item.status === "dependency_blocked").length,
    route_review_required_count: items.filter((item) => item.status === "route_review_required").length,
    missing_model_count: items.filter((item) => item.blockers.includes("readiness:model_catalogue_required") || item.blockers.includes("readiness:model_capability_gap")).length,
    missing_provider_count: items.filter((item) => item.blockers.includes("readiness:provider_registration_required")).length,
    credential_required_count: items.filter((item) => item.blockers.includes("readiness:credential_required")).length,
    calibration_hold_count: items.filter((item) => item.blockers.includes("calibration:hold")).length,
    evidence_hold_count: items.filter((item) => item.blockers.includes("evidence:not_ready")).length,
    tool_gap_count: items.filter((item) => item.blockers.includes("tools:missing")).length,
  };
}

function nextActionsFor(status: AutonomousWorkflowPortfolioAdmissionStatus, items: readonly AutonomousWorkflowPortfolioAdmissionItem[], plan: AutonomousWorkflowPortfolioPlan): string[] {
  const actions = new Set(items.flatMap((item) => item.next_actions));
  if (plan.status === "partial") actions.add("resolve_missing_required_domain_coverage_before_full_portfolio_execution");
  if (status === "ready_for_approval") actions.add("review_admission_digest_then_approve_provider_calls_per_item");
  if (status === "partial") actions.add("resolve_blocked_items_or_explicitly_accept_partial_portfolio_execution");
  if (status === "blocked") actions.add("resolve_portfolio_admission_blockers_before_dispatch");
  return [...actions].sort().slice(0, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS);
}

function policyFor(options: AutonomousWorkflowPortfolioAdmissionOptions, plan: AutonomousWorkflowPortfolioPlan): AutonomousWorkflowPortfolioAdmissionPolicy {
  const inputTokens = options.run?.maxInputTokens ?? 4_096;
  const outputTokens = options.run?.maxOutputTokens ?? 1_024;
  boundedPositive("workflow portfolio admission input tokens", inputTokens);
  boundedPositive("workflow portfolio admission output tokens", outputTokens);
  if (options.verifyPlan !== undefined && typeof options.verifyPlan !== "boolean") throw new ArgumentError("workflow portfolio admission verifyPlan must be boolean");
  if (options.requireAvailableTools !== undefined && typeof options.requireAvailableTools !== "boolean") throw new ArgumentError("workflow portfolio admission requireAvailableTools must be boolean");
  if (options.requireCalibratedLearning !== undefined && typeof options.requireCalibratedLearning !== "boolean") throw new ArgumentError("workflow portfolio admission requireCalibratedLearning must be boolean");
  return {
    require_all_domains: plan.policy.require_all_domains,
    allow_partial: plan.policy.allow_partial,
    verify_plan: options.verifyPlan !== false,
    require_available_tools: options.requireAvailableTools === true,
    require_calibrated_learning: options.requireCalibratedLearning === true,
    input_tokens: inputTokens,
    output_tokens: outputTokens,
  };
}

function planReadinessOptions(options: AutonomousWorkflowPortfolioAdmissionOptions, policy: AutonomousWorkflowPortfolioAdmissionPolicy): Parameters<AutonomousAgent["readiness"]>[0] {
  const candidates = options.candidates ?? options.run?.candidates;
  return {
    ...(candidates === undefined ? {} : { candidates }),
    estimatedInputTokens: policy.input_tokens,
    requestedOutputTokens: policy.output_tokens,
    ...(options.calibrationReport === undefined ? {} : { calibrationReport: options.calibrationReport }),
    ...(options.requireCalibratedLearning === undefined ? {} : { requireCalibratedLearning: options.requireCalibratedLearning }),
    ...(options.evidenceReadiness === undefined ? {} : { evidenceReadiness: options.evidenceReadiness }),
  };
}

function modelsByDomain(report: AutonomousReadinessReport, candidates: readonly AutonomousModelCandidate[], run: AutonomousWorkflowPortfolioRunOptions | undefined): Map<AutonomousDomainName, string[]> {
  const allowed = new Set(candidates.filter((candidate) => candidateMatchesPolicy(candidate, run)).map(modelId));
  const result = new Map<AutonomousDomainName, string[]>();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    result.set(domain, report.models.filter((row) => row.eligible_domains.includes(domain) && allowed.has(`${row.provider}/${row.model}`)).map((row) => `${row.provider}/${row.model}`).sort());
  }
  return result;
}

function domainRows(report: AutonomousReadinessReport): Map<AutonomousDomainName, AutonomousReadinessDomain> {
  return new Map(report.domains.map((row) => [row.domain, row]));
}

function modelCandidates(agent: AutonomousAgent, options: AutonomousWorkflowPortfolioAdmissionOptions): AutonomousModelCandidate[] {
  const values = options.candidates ?? options.run?.candidates ?? agent.models();
  if (!Array.isArray(values) || values.length > 128) throw new ArgumentError("workflow portfolio admission candidates are outside their bound");
  const ids = new Set<string>();
  for (const candidate of values) {
    if (!candidate || typeof candidate !== "object" || typeof candidate.provider !== "string" || typeof candidate.model !== "string") throw new ArgumentError("workflow portfolio admission candidate is malformed");
    const id = modelId(candidate);
    if (ids.has(id)) throw new ArgumentError(`workflow portfolio admission candidate ${id} is duplicated`);
    ids.add(id);
  }
  return [...values];
}

function buildItem(
  planItem: AutonomousWorkflowPortfolioPlan["items"][number],
  row: AutonomousReadinessDomain | undefined,
  eligibleModelIds: readonly string[],
  dependencyStatuses: Record<string, AutonomousWorkflowPortfolioAdmissionItemStatus>,
  options: AutonomousWorkflowPortfolioAdmissionOptions,
): AutonomousWorkflowPortfolioAdmissionItem {
  const base = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
    item_id: planItem.item_id,
    domain: planItem.domain,
    depends_on: [...planItem.depends_on],
    dependency_statuses: dependencyStatuses,
    plan_status: planItem.status,
    readiness_state: row?.state ?? "not_evaluated" as const,
    workflow_digest: planItem.workflow_digest,
    plan_digest: planItem.plan_digest,
    request_digest: planItem.request_digest,
    required_model_capabilities: [...(row?.required_model_capabilities ?? planItem.required_capabilities)].sort(),
    compatible_model_count: row?.compatible_model_count ?? 0,
    eligible_model_count: eligibleModelIds.length,
    eligible_model_ids: [...eligibleModelIds],
    missing_tools: row?.missing_tools ? [...row.missing_tools] : [],
    approval: APPROVAL,
    selection: SELECTION,
    retention: RETENTION,
    secret_material: "never_returned" as const,
  };
  if (planItem.status === "route_review_required") return { ...base, status: "route_review_required", blockers: ["plan:route_review_required"], next_actions: ["review_route_before_model_admission"] };
  if (planItem.status !== "ready") return { ...base, status: "blocked", blockers: [`plan:${planItem.error_class ?? "not_ready"}`], next_actions: ["repair_portfolio_plan_before_admission"] };
  if (!row) return { ...base, status: "blocked", blockers: ["readiness:domain_missing"], next_actions: ["recompute_all_domain_readiness"] };
  const projected = readinessBlockers(row, options);
  const blockers = [...projected.blockers];
  if (eligibleModelIds.length === 0 && row.state === "ready_for_caller_approval") blockers.push("selection:no_model_matches_run_constraints");
  const nextActions = [...projected.actions];
  if (eligibleModelIds.length === 0 && row.state === "ready_for_caller_approval") nextActions.push("relax_run_constraints_or_register_another_model_arm");
  const status: AutonomousWorkflowPortfolioAdmissionItemStatus = blockers.length ? "blocked" : "eligible";
  return { ...base, status, blockers: [...new Set(blockers)].sort(), next_actions: [...new Set(nextActions)].sort() };
}

function applyDependencyClosure(items: AutonomousWorkflowPortfolioAdmissionItem[], plan: AutonomousWorkflowPortfolioPlan): AutonomousWorkflowPortfolioAdmissionItem[] {
  const byId = new Map(items.map((item) => [item.item_id, item]));
  for (const id of plan.dependency_graph.topological_order) {
    const item = byId.get(id);
    if (!item || item.status === "route_review_required") continue;
    const dependencyStatuses = Object.fromEntries(item.depends_on.map((dependency) => [dependency, byId.get(dependency)?.status ?? "blocked"])) as Record<string, AutonomousWorkflowPortfolioAdmissionItemStatus>;
    const dependencyBlocked = Object.values(dependencyStatuses).some((status) => status !== "eligible");
    if (dependencyBlocked) byId.set(id, { ...item, status: "dependency_blocked", blockers: [...new Set([...item.blockers, "dependency:not_eligible"])].sort(), next_actions: [...new Set([...item.next_actions, "resolve_predecessor_admission_before_dispatch"])].sort(), dependency_statuses: dependencyStatuses });
    else if (item.depends_on.length > 0) byId.set(id, { ...item, dependency_statuses: dependencyStatuses });
  }
  return items.map((item) => byId.get(item.item_id)!).sort((left, right) => left.item_id.localeCompare(right.item_id));
}

/** Validate a persisted admission projection before it is displayed or used for review. */
export async function validateAutonomousWorkflowPortfolioAdmission(value: unknown): Promise<AutonomousWorkflowPortfolioAdmission> {
  if (!isObject(value)) throw new ArgumentError("workflow portfolio admission must be an object");
  const allowed = new Set(["schema", "status", "plan", "policy", "readiness_digest", "items", "dependency_graph", "waves", "counts", "next_actions", "execution", "authorization", "retention", "secret_material", "admission_digest"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new ArgumentError("workflow portfolio admission contains unsupported metadata");
  if (value.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA || value.execution !== EXECUTION || value.authorization !== AUTHORIZATION || value.retention !== RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("workflow portfolio admission markers are invalid");
  if (value.status !== "ready_for_approval" && value.status !== "partial" && value.status !== "blocked") throw new ArgumentError("workflow portfolio admission status is invalid");
  const plan = await validateAutonomousWorkflowPortfolioPlan(value.plan);
  const readinessDigest = boundedDigest("workflow portfolio admission readiness_digest", value.readiness_digest);
  const policyValue = value.policy;
  if (!isObject(policyValue) || typeof policyValue.require_all_domains !== "boolean" || typeof policyValue.allow_partial !== "boolean" || typeof policyValue.verify_plan !== "boolean" || typeof policyValue.require_available_tools !== "boolean" || typeof policyValue.require_calibrated_learning !== "boolean") throw new ArgumentError("workflow portfolio admission policy is malformed");
  if (policyValue.require_all_domains !== plan.policy.require_all_domains || policyValue.allow_partial !== plan.policy.allow_partial) throw new ArgumentError("workflow portfolio admission policy does not match its plan");
  const policy: AutonomousWorkflowPortfolioAdmissionPolicy = { require_all_domains: policyValue.require_all_domains, allow_partial: policyValue.allow_partial, verify_plan: policyValue.verify_plan, require_available_tools: policyValue.require_available_tools, require_calibrated_learning: policyValue.require_calibrated_learning, input_tokens: boundedPositive("workflow portfolio admission policy input_tokens", policyValue.input_tokens), output_tokens: boundedPositive("workflow portfolio admission policy output_tokens", policyValue.output_tokens) };
  if (!Array.isArray(value.items) || value.items.length < 1 || value.items.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ITEMS) throw new ArgumentError("workflow portfolio admission items are outside their bound");
  const planById = new Map(plan.items.map((item) => [item.item_id, item]));
  const seen = new Set<string>();
  const items: AutonomousWorkflowPortfolioAdmissionItem[] = [];
  for (const raw of value.items) {
    if (!isObject(raw)) throw new ArgumentError("workflow portfolio admission item is malformed");
    if (raw.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA) throw new ArgumentError("workflow portfolio admission item schema is invalid");
    const id = boundedIdentifier("workflow portfolio admission item_id", raw.item_id);
    if (seen.has(id) || !planById.has(id)) throw new ArgumentError("workflow portfolio admission item ids do not match the plan");
    seen.add(id);
    const planItem = planById.get(id)!;
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(raw.domain as AutonomousDomainName) || raw.domain !== planItem.domain) throw new ArgumentError("workflow portfolio admission item domain is invalid");
    if (!Array.isArray(raw.depends_on) || JSON.stringify([...raw.depends_on].sort()) !== JSON.stringify([...planItem.depends_on].sort())) throw new ArgumentError("workflow portfolio admission dependencies do not match the plan");
    const dependsOn = sortedUniqueStrings("workflow portfolio admission depends_on", raw.depends_on, 16);
    if (!isObject(raw.dependency_statuses)) throw new ArgumentError("workflow portfolio admission dependency statuses are malformed");
    if (Object.keys(raw.dependency_statuses).sort().join("|") !== dependsOn.sort().join("|")) throw new ArgumentError("workflow portfolio admission dependency status keys do not match dependencies");
    const statusValues = new Set(["eligible", "blocked", "dependency_blocked", "route_review_required"]);
    for (const status of Object.values(raw.dependency_statuses)) if (typeof status !== "string" || !statusValues.has(status)) throw new ArgumentError("workflow portfolio admission dependency status is invalid");
    if (raw.plan_status !== planItem.status || !statusValues.has(raw.status as string)) throw new ArgumentError("workflow portfolio admission item status is invalid");
    const readinessState = raw.readiness_state;
    if (typeof readinessState !== "string" || (readinessState !== "not_evaluated" && !["ready_for_caller_approval", "model_catalogue_required", "provider_registration_required", "credential_required", "model_capability_gap", "partial"].includes(readinessState))) throw new ArgumentError("workflow portfolio admission readiness state is invalid");
    if (raw.workflow_digest !== planItem.workflow_digest || raw.plan_digest !== planItem.plan_digest || boundedDigest("workflow portfolio admission request_digest", raw.request_digest) !== planItem.request_digest) throw new ArgumentError("workflow portfolio admission item digest does not match the plan");
    const required = sortedUniqueStrings("workflow portfolio admission required_model_capabilities", raw.required_model_capabilities);
    const eligibleIds = sortedUniqueStrings("workflow portfolio admission eligible_model_ids", raw.eligible_model_ids, 128);
    if (boundedCount("workflow portfolio admission compatible_model_count", raw.compatible_model_count, 128) !== raw.compatible_model_count || boundedCount("workflow portfolio admission eligible_model_count", raw.eligible_model_count, 128) !== eligibleIds.length) throw new ArgumentError("workflow portfolio admission model counts are invalid");
    const missingTools = sortedUniqueStrings("workflow portfolio admission missing_tools", raw.missing_tools, 128);
    const blockers = sortedUniqueStrings("workflow portfolio admission blockers", raw.blockers, 32);
    const nextActions = sortedUniqueStrings("workflow portfolio admission next_actions", raw.next_actions, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS);
    if (raw.approval !== APPROVAL || raw.selection !== SELECTION || raw.retention !== RETENTION || raw.secret_material !== "never_returned") throw new ArgumentError("workflow portfolio admission item markers are invalid");
    items.push({ schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA, item_id: id, domain: raw.domain as AutonomousDomainName, depends_on: dependsOn, dependency_statuses: Object.fromEntries(Object.entries(raw.dependency_statuses).map(([key, status]) => [key, status as AutonomousWorkflowPortfolioAdmissionItemStatus])), plan_status: raw.plan_status as AutonomousWorkflowPortfolioItemStatus, status: raw.status as AutonomousWorkflowPortfolioAdmissionItemStatus, readiness_state: readinessState as AutonomousWorkflowPortfolioAdmissionItem["readiness_state"], workflow_digest: raw.workflow_digest as string | null, plan_digest: raw.plan_digest as string | null, request_digest: raw.request_digest as string, required_model_capabilities: required, compatible_model_count: raw.compatible_model_count as number, eligible_model_count: raw.eligible_model_count as number, eligible_model_ids: eligibleIds, missing_tools: missingTools, blockers, next_actions: nextActions, approval: APPROVAL, selection: SELECTION, retention: RETENTION, secret_material: "never_returned" });
  }
  if (seen.size !== plan.items.length) throw new ArgumentError("workflow portfolio admission does not cover every plan item");
  if (JSON.stringify(value.dependency_graph) !== JSON.stringify(plan.dependency_graph) || JSON.stringify(value.waves) !== JSON.stringify(plan.dependency_graph.waves)) throw new ArgumentError("workflow portfolio admission dependency projection is inconsistent");
  if (!Array.isArray(value.next_actions)) throw new ArgumentError("workflow portfolio admission next actions are malformed");
  const nextActions = sortedUniqueStrings("workflow portfolio admission next_actions", value.next_actions, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS);
  const counts = itemCounts(items);
  if (!isObject(value.counts) || JSON.stringify(value.counts) !== JSON.stringify(counts)) throw new ArgumentError("workflow portfolio admission counts are inconsistent");
  const status = admissionStatusFor(plan, items);
  if (value.status !== status) throw new ArgumentError("workflow portfolio admission status is inconsistent");
  if (JSON.stringify(nextActions) !== JSON.stringify(nextActionsFor(status, items, plan))) throw new ArgumentError("workflow portfolio admission next actions are inconsistent");
  const descriptor = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA, status, plan, policy, readiness_digest: readinessDigest, items, dependency_graph: plan.dependency_graph, waves: plan.dependency_graph.waves, counts, next_actions: nextActions, execution: EXECUTION, authorization: AUTHORIZATION, retention: RETENTION, secret_material: "never_returned" as const };
  const admissionDigest = boundedDigest("workflow portfolio admission admission_digest", value.admission_digest);
  if (await digestJson(descriptor) !== admissionDigest) throw new ArgumentError("workflow portfolio admission digest is invalid");
  if (new TextEncoder().encode(JSON.stringify({ ...descriptor, admission_digest: admissionDigest })).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES) throw new ArgumentError("workflow portfolio admission exceeds its byte bound");
  return { ...descriptor, admission_digest: admissionDigest };
}

/**
 * Project the complete portfolio admission posture without provider, tool, connector,
 * credential, evaluator, or learning dispatch. Runtime model selection is intentionally
 * deferred until the caller reviews this digest and explicitly approves each invocation.
 */
export async function admitAutonomousWorkflowPortfolio(
  agent: AutonomousAgent,
  requests: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioAdmissionOptions = {},
): Promise<AutonomousWorkflowPortfolioAdmission> {
  if (!agent || typeof agent.models !== "function" || typeof agent.readiness !== "function" || typeof agent.blueprint !== "function") throw new ArgumentError("workflow portfolio admission requires an AutonomousAgent");
  const plan = options.plan === undefined
    ? await planAutonomousWorkflowPortfolio(agent, requests, options.planOptions ?? {})
    : await validateAutonomousWorkflowPortfolioPlan(options.plan);
  const policy = policyFor(options, plan);
  if (policy.verify_plan && options.plan !== undefined) {
    const verification = await verifyAutonomousWorkflowPortfolio(agent, plan, requests, options.planOptions ?? {});
    if (verification.status !== "verified") throw new ProviderRuntimeError("workflow portfolio admission plan verification failed; re-review is required");
  }
  const candidates = modelCandidates(agent, options);
  const report = await agent.readiness(planReadinessOptions(options, policy));
  const rows = domainRows(report);
  const eligibleByDomain = modelsByDomain(report, candidates, options.run);
  const initial = plan.items.map((planItem) => {
    const row = rows.get(planItem.domain);
    const dependencyStatuses = Object.fromEntries(planItem.depends_on.map((dependency) => [dependency, "blocked" as const]));
    return buildItem(planItem, row, eligibleByDomain.get(planItem.domain) ?? [], dependencyStatuses, options);
  });
  const admissionItems = applyDependencyClosure(initial, plan);
  const status = admissionStatusFor(plan, admissionItems);
  const counts = itemCounts(admissionItems);
  const nextActions = nextActionsFor(status, admissionItems, plan);
  const descriptor = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
    status,
    plan,
    policy,
    readiness_digest: report.readiness_digest,
    items: admissionItems,
    dependency_graph: plan.dependency_graph,
    waves: plan.dependency_graph.waves,
    counts,
    next_actions: nextActions,
    execution: EXECUTION,
    authorization: AUTHORIZATION,
    retention: RETENTION,
    secret_material: "never_returned" as const,
  };
  const admission = { ...descriptor, admission_digest: await digestJson(descriptor) };
  if (new TextEncoder().encode(JSON.stringify(admission)).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES) throw new ArgumentError("workflow portfolio admission exceeds its byte bound");
  return structuredClone(admission);
}

/** In-memory reference persistence with the same conditional-write semantics as durable stores. */
export class InMemoryAutonomousWorkflowPortfolioAdmissionPersistence implements AutonomousWorkflowPortfolioAdmissionTransactionalPersistence {
  private admission: AutonomousWorkflowPortfolioAdmission | null = null;

  read(): AutonomousWorkflowPortfolioAdmission | null {
    return this.admission === null ? null : structuredClone(this.admission);
  }

  async write(admission: AutonomousWorkflowPortfolioAdmission): Promise<void> {
    this.admission = await validateAutonomousWorkflowPortfolioAdmission(admission);
  }

  async writeIfUnchanged(expectedAdmissionDigest: string | null, admission: AutonomousWorkflowPortfolioAdmission): Promise<boolean> {
    if ((this.admission?.admission_digest ?? null) !== expectedAdmissionDigest) return false;
    await this.write(admission);
    return true;
  }
}

/** Strict JSON adapter for Node, browser, IndexedDB, and application-owned object storage. */
export class JsonAutonomousWorkflowPortfolioAdmissionPersistence implements AutonomousWorkflowPortfolioAdmissionPersistence {
  constructor(protected readonly store: AutonomousWorkflowPortfolioAdmissionTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("workflow portfolio admission JSON store is malformed");
  }

  async read(): Promise<AutonomousWorkflowPortfolioAdmission | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES) throw new ArgumentError("workflow portfolio admission JSON exceeds its bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("workflow portfolio admission JSON is invalid"); }
    return validateAutonomousWorkflowPortfolioAdmission(parsed);
  }

  async write(admission: AutonomousWorkflowPortfolioAdmission): Promise<void> {
    const validated = await validateAutonomousWorkflowPortfolioAdmission(admission);
    const encoded = JSON.stringify(validated);
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES) throw new ArgumentError("workflow portfolio admission JSON exceeds its bound");
    await this.store.write(encoded);
  }
}

/** JSON adapter that refuses stale writers instead of pretending ordinary writes are atomic. */
export class TransactionalJsonAutonomousWorkflowPortfolioAdmissionPersistence extends JsonAutonomousWorkflowPortfolioAdmissionPersistence implements AutonomousWorkflowPortfolioAdmissionTransactionalPersistence {
  private readonly transactionalStore: AutonomousWorkflowPortfolioAdmissionTransactionalTextStore;

  constructor(store: AutonomousWorkflowPortfolioAdmissionTransactionalTextStore) {
    super(store);
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional workflow portfolio admission store requires writeIfUnchanged");
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedAdmissionDigest: string | null, admission: AutonomousWorkflowPortfolioAdmission): Promise<boolean> {
    const validated = await validateAutonomousWorkflowPortfolioAdmission(admission);
    const encoded = JSON.stringify(validated);
    const committed = await this.transactionalStore.writeIfUnchanged(expectedAdmissionDigest, encoded);
    if (typeof committed !== "boolean") throw new ArgumentError("transactional workflow portfolio admission store returned a non-boolean result");
    return committed;
  }
}

/** Browser-compatible text storage adapter; the caller owns quota, encryption, and lifecycle. */
export class WebStorageAutonomousWorkflowPortfolioAdmissionTextStore implements AutonomousWorkflowPortfolioAdmissionTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("workflow portfolio admission Web Storage adapter is malformed");
    boundedIdentifier("workflow portfolio admission Web Storage key", key);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

/**
 * Serialized admission coordinator for process restarts and remote handoffs. It persists only
 * the redacted admission image, validates every restore, and uses conditional writes when the
 * backing adapter supports them. The caller still supplies transient requests and credentials to
 * the later resumable execution call.
 */
export class AutonomousWorkflowPortfolioAdmissionController {
  private currentAdmission: AutonomousWorkflowPortfolioAdmission | null = null;
  private expectedAdmissionDigest: string | null = null;
  private controllerStatus: "empty" | "restored" | "admitted" = "empty";
  private mutation: Promise<void> = Promise.resolve();

  constructor(readonly agent: AutonomousAgent, readonly persistence: AutonomousWorkflowPortfolioAdmissionPersistence) {
    if (!agent || typeof agent.admitWorkflowPortfolio !== "function") throw new ArgumentError("workflow portfolio admission controller requires an AutonomousAgent");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("workflow portfolio admission controller persistence is malformed");
  }

  async restore(): Promise<AutonomousWorkflowPortfolioAdmissionControllerProjection> {
    return this.serial(async () => {
      const stored = await this.persistence.read();
      this.currentAdmission = stored === null ? null : await validateAutonomousWorkflowPortfolioAdmission(stored);
      this.expectedAdmissionDigest = this.currentAdmission?.admission_digest ?? null;
      this.controllerStatus = this.currentAdmission === null ? "empty" : "restored";
      return this.projection();
    });
  }

  async admit(requests: readonly AutonomousWorkflowPortfolioItemRequest[], options: AutonomousWorkflowPortfolioAdmissionOptions = {}): Promise<AutonomousWorkflowPortfolioAdmission> {
    return this.serial(async () => {
      const admission = await this.agent.admitWorkflowPortfolio(requests, options);
      const validated = await validateAutonomousWorkflowPortfolioAdmission(admission);
      if (this.persistence && "writeIfUnchanged" in this.persistence && typeof this.persistence.writeIfUnchanged === "function") {
        const committed = await this.persistence.writeIfUnchanged(this.expectedAdmissionDigest, validated);
        if (!committed) throw new ArgumentError("workflow portfolio admission is stale; another coordinator committed after restore");
      } else {
        await this.persistence.write(validated);
      }
      this.currentAdmission = structuredClone(validated);
      this.expectedAdmissionDigest = validated.admission_digest;
      this.controllerStatus = "admitted";
      return structuredClone(validated);
    });
  }

  admission(): AutonomousWorkflowPortfolioAdmission | null {
    return this.currentAdmission === null ? null : structuredClone(this.currentAdmission);
  }

  projection(): AutonomousWorkflowPortfolioAdmissionControllerProjection {
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_CONTROLLER_SCHEMA,
      status: this.controllerStatus,
      plan_digest: this.currentAdmission?.plan.portfolio_digest ?? null,
      admission_digest: this.currentAdmission?.admission_digest ?? null,
      persisted: true,
      retention: "metadata_only_admission_and_plan_digests;tasks_prompts_credentials_and_provider_values_never_persisted",
      secret_material: "never_returned",
    };
  }

  private async serial<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.mutation.then(operation, operation);
    this.mutation = next.then(() => undefined, () => undefined);
    return next;
  }
}
