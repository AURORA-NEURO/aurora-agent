import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  builtinAutonomousDomainProfiles,
  type AutonomousDomainName,
  type AutonomousDomainProfile,
  type AutonomousWorkflowStage,
} from "./autonomous.js";
import { buildAutonomousEvidencePlan, type AutonomousEvidencePlanJSON } from "./autonomous-evidence.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Digest-bound static/runtime contract audit for every reviewed autonomous domain. */
export const AUTONOMOUS_DOMAIN_AUDIT_SCHEMA = "bioprism-typescript-autonomous-domain-audit/0.1" as const;
export const AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA = "bioprism-typescript-autonomous-domain-audit-row/0.1" as const;
export const MAX_AUTONOMOUS_DOMAIN_AUDIT_BYTES = 512_000;
export const MAX_AUTONOMOUS_DOMAIN_AUDIT_ISSUES = 256;

export type AutonomousDomainAuditIssueSeverity = "blocking" | "warning";
export type AutonomousDomainAuditContractStatus = "valid" | "invalid";
export type AutonomousDomainAuditRuntimeStatus = "unassessed" | "ready_for_review" | "partial" | "blocked";

export interface AutonomousDomainAuditIssue extends JsonObject {
  code: string;
  severity: AutonomousDomainAuditIssueSeverity;
  message: string;
  next_action: string;
}

export interface AutonomousDomainAuditToolSurface extends JsonObject {
  assessed: boolean;
  declared_tool_count: number;
  available_tool_count: number | null;
  missing_tool_names: string[];
  read_only_tool_count: number;
  approval_required_tool_count: number;
  exact_stage_capability_gaps: string[];
}

export interface AutonomousDomainAuditEvidenceSurface extends JsonObject {
  assessed: boolean;
  plan_digest: string;
  requirement_count: number;
  covered_requirement_count: number | null;
  missing_requirement_count: number | null;
  coverage_ratio: number | null;
  coverage_status: AutonomousEvidencePlanJSON["coverage_status"] | "unassessed";
  next_stage_ids: string[];
}

export interface AutonomousDomainAuditRow extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA;
  domain: AutonomousDomainName;
  profile_digest: string;
  workflow_id: string;
  workflow_digest: string;
  stage_ids: string[];
  stage_count: number;
  required_model_capabilities: string[];
  declared_capability_count: number;
  evaluator_domain: AutonomousDomainProfile["evaluator_domain"];
  workflow_evaluator_signals: string[];
  contract_status: AutonomousDomainAuditContractStatus;
  runtime_status: AutonomousDomainAuditRuntimeStatus;
  tool_surface: AutonomousDomainAuditToolSurface;
  evidence_surface: AutonomousDomainAuditEvidenceSurface;
  issues: AutonomousDomainAuditIssue[];
  next_actions: string[];
  retention: "metadata_only;profile_payloads_and_runtime_values_not_retained";
  execution: "audit_only;no_provider_source_tool_queue_or_credential_dispatch";
  secret_material: "never_returned";
  row_digest: string;
}

export interface AutonomousDomainAuditSummary extends JsonObject {
  domain_count: number;
  valid_domain_count: number;
  invalid_domain_count: number;
  runtime_ready_domain_count: number;
  runtime_partial_domain_count: number;
  runtime_blocked_domain_count: number;
  runtime_unassessed_domain_count: number;
  declared_tool_count: number;
  missing_tool_count: number;
  evidence_requirement_count: number;
  evidence_covered_requirement_count: number | null;
  static_contract_status: "valid" | "invalid";
  runtime_status: AutonomousDomainAuditRuntimeStatus;
}

export interface AutonomousDomainAuditReport extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_AUDIT_SCHEMA;
  rows: AutonomousDomainAuditRow[];
  summary: AutonomousDomainAuditSummary;
  next_actions: string[];
  retention: "metadata_only;profile_payloads_and_runtime_values_not_retained";
  execution: "audit_only;no_provider_source_tool_queue_or_credential_dispatch";
  credential_posture: "caller_owned_opaque_handles_only;no_credentials_consumed";
  secret_material: "never_returned";
  report_digest: string;
}

export interface AutonomousDomainAuditOptions {
  /** Override the reviewed profile set for contract validation; defaults to all built-ins. */
  profiles?: readonly AutonomousDomainProfile[];
  /** When supplied, assess exact live tool names without invoking or authorizing them. */
  availableToolNames?: readonly string[];
  /** When supplied, assess caller-owned evidence identifiers against every stage output. */
  availableEvidence?: readonly string[];
  /** Optional completed stage IDs used to project the next executable evidence stages. */
  completedStages?: Readonly<Record<string, readonly string[]>>;
}

const RETENTION = "metadata_only;profile_payloads_and_runtime_values_not_retained" as const;
const EXECUTION = "audit_only;no_provider_source_tool_queue_or_credential_dispatch" as const;

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedDigest(name: string, value: unknown): string {
  const digest = boundedText(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(digest)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return digest;
}

function boundedList(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must be a bounded array`);
  const values = value.map((item) => boundedText(`${name} entry`, item, 512));
  if (new Set(values).size !== values.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return values;
}

function issue(code: string, severity: AutonomousDomainAuditIssueSeverity, message: string, nextAction: string): AutonomousDomainAuditIssue {
  return { code: boundedText("domain audit issue code", code, 128), severity, message: boundedText("domain audit issue message", message, 2_048), next_action: boundedText("domain audit issue next action", nextAction, 1_024) };
}

function uniqueSorted(values: readonly string[]): string[] {
  return [...new Set(values)].sort();
}

function stageMap(stages: readonly AutonomousWorkflowStage[]): Map<string, AutonomousWorkflowStage> {
  return new Map(stages.map((stage) => [stage.id, stage]));
}

function hasDependencyCycle(stages: readonly AutonomousWorkflowStage[]): boolean {
  const known = stageMap(stages);
  const indegree = new Map<string, number>(stages.map((stage) => [stage.id, 0]));
  const outgoing = new Map<string, string[]>(stages.map((stage) => [stage.id, []]));
  for (const stage of stages) {
    for (const dependency of stage.depends_on) {
      if (!known.has(dependency)) continue;
      indegree.set(stage.id, (indegree.get(stage.id) ?? 0) + 1);
      outgoing.get(dependency)?.push(stage.id);
    }
  }
  const queue = stages.filter((stage) => indegree.get(stage.id) === 0).map((stage) => stage.id);
  let visited = 0;
  while (queue.length) {
    const current = queue.shift() as string;
    visited += 1;
    for (const child of outgoing.get(current) ?? []) {
      const remaining = (indegree.get(child) ?? 0) - 1;
      indegree.set(child, remaining);
      if (remaining === 0) queue.push(child);
    }
  }
  return visited !== stages.length;
}

function normalizeToolNames(value: readonly string[] | undefined): string[] | undefined {
  if (value === undefined) return undefined;
  if (!Array.isArray(value) || value.length > 4_096) throw new ArgumentError("domain audit availableToolNames is outside its bounds");
  return uniqueSorted(value.map((name) => boundedText("domain audit available tool name", name, 512)));
}

function normalizeEvidence(value: readonly string[] | undefined): string[] | undefined {
  if (value === undefined) return undefined;
  if (!Array.isArray(value) || value.length > 4_096) throw new ArgumentError("domain audit availableEvidence is outside its bounds");
  return uniqueSorted(value.map((item) => boundedText("domain audit available evidence", item, 512)));
}

function validateProfiles(profiles: readonly AutonomousDomainProfile[]): AutonomousDomainProfile[] {
  if (!Array.isArray(profiles) || profiles.length < 1 || profiles.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("domain audit profiles must contain 1..12 profiles");
  const domains = profiles.map((profile) => {
    if (!isObject(profile) || !AUTONOMOUS_DOMAIN_NAMES.includes(profile.domain as AutonomousDomainName)) throw new ArgumentError("domain audit profile domain is unsupported");
    return profile.domain as AutonomousDomainName;
  });
  if (new Set(domains).size !== domains.length) throw new ArgumentError("domain audit profiles must use unique domains");
  return [...profiles].sort((left, right) => left.domain.localeCompare(right.domain));
}

function auditWorkflow(profile: AutonomousDomainProfile, rows: AutonomousDomainAuditIssue[]): void {
  const workflow = profile.workflow;
  if (workflow.domain !== profile.domain) rows.push(issue("workflow_domain_mismatch", "blocking", "workflow domain does not match its profile domain", "rebuild the reviewed domain profile from one workflow contract"));
  if (!workflow.workflow_id || !workflow.workflow_digest) rows.push(issue("workflow_identity_missing", "blocking", "workflow identity is incomplete", "rebuild the workflow and recompute its digest"));
  if (!Array.isArray(workflow.stages) || workflow.stages.length < 1 || workflow.stages.length > 64) {
    rows.push(issue("workflow_stage_count", "blocking", "workflow must contain between one and sixty-four stages", "define a bounded reviewed workflow stage graph"));
    return;
  }
  const stageIds = workflow.stages.map((stage) => stage.id);
  if (stageIds.some((id) => typeof id !== "string" || id.trim().length === 0) || new Set(stageIds).size !== stageIds.length) rows.push(issue("workflow_stage_identity", "blocking", "workflow stage identifiers must be non-empty and unique", "give every stage a stable unique identifier"));
  const known = new Set(stageIds);
  for (const stage of workflow.stages) {
    if (!Array.isArray(stage.required_capabilities) || stage.required_capabilities.length === 0) rows.push(issue("stage_capability_contract", "blocking", `workflow stage ${stage.id} has no required capability contract`, "bind every stage to at least one reviewed capability"));
    if (!Array.isArray(stage.evidence_outputs) || stage.evidence_outputs.length === 0) rows.push(issue("stage_evidence_contract", "blocking", `workflow stage ${stage.id} has no evidence output contract`, "declare evidence outputs for every stage"));
    if (!Array.isArray(stage.evaluator_signals) || stage.evaluator_signals.length === 0) rows.push(issue("stage_evaluator_contract", "blocking", `workflow stage ${stage.id} has no evaluator signal contract`, "declare at least one evaluator signal for every stage"));
    if (!Array.isArray(stage.depends_on) || stage.depends_on.some((dependency) => dependency === stage.id || !known.has(dependency))) rows.push(issue("workflow_dependency_closure", "blocking", `workflow stage ${stage.id} depends on an unknown or self-referential stage`, "close every dependency against the reviewed workflow graph"));
    if (stage.read_only === false && stage.approval_required !== true) rows.push(issue("effect_approval_contract", "blocking", `workflow stage ${stage.id} permits effects without an approval gate`, "require explicit approval before any non-read-only stage"));
  }
  if (hasDependencyCycle(workflow.stages)) rows.push(issue("workflow_dependency_cycle", "blocking", "workflow stage dependencies contain a cycle", "replace the cycle with a directed acyclic execution graph"));
  if (!Array.isArray(workflow.route_intents) || workflow.route_intents.length === 0) rows.push(issue("workflow_route_intents", "blocking", "workflow does not declare routing intents", "declare the task families this workflow can handle"));
  if (!Array.isArray(workflow.evaluator_signals) || workflow.evaluator_signals.length === 0) rows.push(issue("workflow_evaluator_signals", "blocking", "workflow does not declare evaluator signals", "declare the workflow-level completion signals"));
  if (typeof workflow.completion_contract !== "string" || workflow.completion_contract.trim().length === 0) rows.push(issue("workflow_completion_contract", "blocking", "workflow does not declare a completion contract", "define what evidence is required before the workflow can claim completion"));
}

function auditProfileMetadata(profile: AutonomousDomainProfile, rows: AutonomousDomainAuditIssue[]): void {
  if (profile.schema !== "bioprism-typescript-autonomous-agent/0.1") rows.push(issue("profile_schema", "blocking", "profile schema is not the reviewed autonomy schema", "rebuild the profile through the reviewed built-in profile factory"));
  if (!Array.isArray(profile.required_model_capabilities) || profile.required_model_capabilities.length === 0) rows.push(issue("model_capability_contract", "blocking", "domain has no required model capabilities", "declare the model capabilities needed to serve this domain"));
  if (Array.isArray(profile.required_model_capabilities) && new Set(profile.required_model_capabilities).size !== profile.required_model_capabilities.length) rows.push(issue("model_capability_duplicates", "blocking", "domain model capabilities contain duplicates", "deduplicate required model capabilities"));
  if (!Array.isArray(profile.capabilities) || profile.capabilities.length === 0) rows.push(issue("domain_capability_catalogue", "blocking", "domain has no capability catalogue", "declare the domain capabilities used by routing and planning"));
  if (Array.isArray(profile.capabilities) && !profile.capabilities.includes(profile.default_capability)) rows.push(issue("default_capability_unlisted", "blocking", "default capability is not present in the domain capability catalogue", "add the default capability or select a declared one"));
  if (!Array.isArray(profile.guardrails) || profile.guardrails.length < 1) rows.push(issue("guardrail_contract", "blocking", "domain has no guardrails", "declare domain-specific safety and epistemic guardrails"));
  if (typeof profile.system_instructions !== "string" || profile.system_instructions.trim().length === 0) rows.push(issue("system_instruction_contract", "blocking", "domain has no system instruction contract", "define bounded domain instructions for provider prompting"));
  if (!profile.tool_profile || profile.tool_profile.domain !== profile.domain || !Array.isArray(profile.tool_profile.bindings)) rows.push(issue("tool_profile_contract", "blocking", "domain tool profile is missing or belongs to another domain", "rebuild the domain tool profile with explicit domain bindings"));
}

function auditTools(profile: AutonomousDomainProfile, rows: AutonomousDomainAuditIssue[], availableTools: string[] | undefined): AutonomousDomainAuditToolSurface {
  const bindings = profile.tool_profile?.bindings ?? [];
  const names = bindings.map((binding) => binding.name);
  if (new Set(names).size !== names.length) rows.push(issue("tool_binding_duplicates", "blocking", "domain tool profile contains duplicate binding names", "give every domain tool binding a unique stable name"));
  for (const binding of bindings) {
    if (!binding.domains.includes(profile.domain)) rows.push(issue("tool_binding_domain", "blocking", `tool ${binding.name} is not bound to its profile domain`, "attach each tool binding to the domain that can review it"));
    if (binding.read_only && binding.approval_required) rows.push(issue("tool_approval_inconsistency", "blocking", `read-only tool ${binding.name} is marked approval-required`, "make the binding metadata agree on whether it can cause effects"));
    if (!binding.read_only && !binding.approval_required) rows.push(issue("tool_effect_without_approval", "blocking", `effectful tool ${binding.name} has no approval requirement`, "require approval for every non-read-only tool binding"));
  }
  const exactStageGaps = uniqueSorted(profile.workflow.stages.flatMap((stage) => stage.required_capabilities.filter((capability) => !bindings.some((binding) => binding.capability === capability))));
  if (exactStageGaps.length) rows.push(issue("stage_exact_tool_gap", "warning", `some stage capabilities have no exact tool binding: ${exactStageGaps.join(", ")}`, "attach a reviewed tool adapter or explicitly accept provider-only stage execution; aliases may still cover some capabilities"));
  const missing = availableTools === undefined ? [] : names.filter((name) => !availableTools.includes(name));
  const readOnlyCount = bindings.filter((binding) => binding.read_only).length;
  const approvalCount = bindings.filter((binding) => binding.approval_required).length;
  return {
    assessed: availableTools !== undefined,
    declared_tool_count: bindings.length,
    available_tool_count: availableTools === undefined ? null : names.filter((name) => availableTools.includes(name)).length,
    missing_tool_names: missing,
    read_only_tool_count: readOnlyCount,
    approval_required_tool_count: approvalCount,
    exact_stage_capability_gaps: exactStageGaps,
  };
}

async function auditEvidence(profile: AutonomousDomainProfile, options: AutonomousDomainAuditOptions): Promise<AutonomousDomainAuditEvidenceSurface> {
  const plan = await buildAutonomousEvidencePlan([profile.workflow], {
    ...(options.availableEvidence === undefined ? {} : { availableEvidence: options.availableEvidence }),
    ...(options.completedStages === undefined ? {} : { completedStages: options.completedStages }),
  });
  const json = plan.toJSON();
  return {
    assessed: options.availableEvidence !== undefined,
    plan_digest: plan.plan_digest,
    requirement_count: plan.requirements.length,
    covered_requirement_count: options.availableEvidence === undefined ? null : plan.covered_requirement_ids.length,
    missing_requirement_count: options.availableEvidence === undefined ? null : plan.missing_requirement_ids.length,
    coverage_ratio: options.availableEvidence === undefined ? null : plan.coverage_ratio,
    coverage_status: options.availableEvidence === undefined ? "unassessed" : json.coverage_status,
    next_stage_ids: [...plan.next_stage_ids],
  };
}

function runtimeStatus(contractStatus: AutonomousDomainAuditContractStatus, tools: AutonomousDomainAuditToolSurface, evidence: AutonomousDomainAuditEvidenceSurface): AutonomousDomainAuditRuntimeStatus {
  if (contractStatus === "invalid") return "blocked";
  if (tools.assessed && tools.missing_tool_names.length > 0) return "partial";
  if (evidence.assessed && evidence.coverage_status !== "complete") return "partial";
  if (!tools.assessed && !evidence.assessed) return "unassessed";
  return "ready_for_review";
}

/**
 * Audit the reviewed contract and optional caller-owned runtime surface for each domain.
 * This is intentionally provider-free: it makes gaps visible before routing, model
 * invocation, evidence acquisition, or tool authorization begins.
 */
export async function auditAutonomousDomainContracts(options: AutonomousDomainAuditOptions = {}): Promise<AutonomousDomainAuditReport> {
  if (!options || typeof options !== "object" || Array.isArray(options)) throw new ArgumentError("domain audit options are malformed");
  const profiles = validateProfiles(options.profiles ?? await builtinAutonomousDomainProfiles());
  const availableTools = normalizeToolNames(options.availableToolNames);
  const availableEvidence = normalizeEvidence(options.availableEvidence);
  if (options.completedStages !== undefined && (!isObject(options.completedStages) || Object.keys(options.completedStages).length > AUTONOMOUS_DOMAIN_NAMES.length)) throw new ArgumentError("domain audit completedStages is malformed");
  const rows: AutonomousDomainAuditRow[] = [];
  for (const profile of profiles) {
    const issues: AutonomousDomainAuditIssue[] = [];
    auditProfileMetadata(profile, issues);
    auditWorkflow(profile, issues);
    const toolSurface = auditTools(profile, issues, availableTools);
    const evidenceSurface = await auditEvidence(profile, { ...options, availableEvidence, availableToolNames: availableTools });
    if (issues.length > MAX_AUTONOMOUS_DOMAIN_AUDIT_ISSUES) throw new ArgumentError(`domain audit ${profile.domain} produced too many issues`);
    const contractStatus: AutonomousDomainAuditContractStatus = issues.some((row) => row.severity === "blocking") ? "invalid" : "valid";
    const runtime = runtimeStatus(contractStatus, toolSurface, evidenceSurface);
    const nextActions = uniqueSorted(issues.map((row) => row.next_action));
    if (runtime === "partial") nextActions.push("resolve the missing live tool or evidence coverage before dispatch");
    if (runtime === "unassessed") nextActions.push("provide caller-owned live tool and evidence inventories for runtime coverage assessment");
    const descriptor = {
      schema: AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA,
      domain: profile.domain,
      profile_digest: await digestJson(profile),
      workflow_id: profile.workflow.workflow_id,
      workflow_digest: profile.workflow.workflow_digest,
      stage_ids: profile.workflow.stages.map((stage) => stage.id),
      stage_count: profile.workflow.stages.length,
      required_model_capabilities: [...profile.required_model_capabilities],
      declared_capability_count: profile.capabilities.length,
      evaluator_domain: profile.evaluator_domain,
      workflow_evaluator_signals: [...profile.workflow.evaluator_signals],
      contract_status: contractStatus,
      runtime_status: runtime,
      tool_surface: toolSurface,
      evidence_surface: evidenceSurface,
      issues: [...issues],
      next_actions: uniqueSorted(nextActions),
      retention: RETENTION,
      execution: EXECUTION,
      secret_material: "never_returned" as const,
    };
    const row: AutonomousDomainAuditRow = { ...descriptor, row_digest: await digestJson(descriptor) };
    rows.push(row);
  }
  const validCount = rows.filter((row) => row.contract_status === "valid").length;
  const runtimeReady = rows.filter((row) => row.runtime_status === "ready_for_review").length;
  const runtimePartial = rows.filter((row) => row.runtime_status === "partial").length;
  const runtimeBlocked = rows.filter((row) => row.runtime_status === "blocked").length;
  const runtimeUnassessed = rows.filter((row) => row.runtime_status === "unassessed").length;
  const assessedEvidence = rows.filter((row) => row.evidence_surface.assessed);
  const evidenceCovered = assessedEvidence.length === 0 ? null : assessedEvidence.reduce((sum, row) => sum + (row.evidence_surface.covered_requirement_count ?? 0), 0);
  const staticStatus = validCount === rows.length ? "valid" as const : "invalid" as const;
  const runtimeStatusValue: AutonomousDomainAuditRuntimeStatus = runtimeBlocked > 0 ? "blocked" : runtimePartial > 0 ? "partial" : runtimeUnassessed === rows.length ? "unassessed" : runtimeReady === rows.length ? "ready_for_review" : "partial";
  const summary: AutonomousDomainAuditSummary = {
    domain_count: rows.length,
    valid_domain_count: validCount,
    invalid_domain_count: rows.length - validCount,
    runtime_ready_domain_count: runtimeReady,
    runtime_partial_domain_count: runtimePartial,
    runtime_blocked_domain_count: runtimeBlocked,
    runtime_unassessed_domain_count: runtimeUnassessed,
    declared_tool_count: rows.reduce((sum, row) => sum + row.tool_surface.declared_tool_count, 0),
    missing_tool_count: rows.reduce((sum, row) => sum + row.tool_surface.missing_tool_names.length, 0),
    evidence_requirement_count: rows.reduce((sum, row) => sum + row.evidence_surface.requirement_count, 0),
    evidence_covered_requirement_count: evidenceCovered,
    static_contract_status: staticStatus,
    runtime_status: runtimeStatusValue,
  };
  const nextActions = uniqueSorted([
    ...rows.flatMap((row) => row.next_actions),
    ...(staticStatus === "invalid" ? ["repair blocking domain contract issues before enabling autonomous dispatch"] : []),
    ...(runtimeStatusValue === "unassessed" ? ["supply caller-owned tool and evidence inventories to complete the runtime audit"] : []),
  ]);
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_AUDIT_SCHEMA,
    rows,
    summary,
    next_actions: nextActions,
    retention: RETENTION,
    execution: EXECUTION,
    credential_posture: "caller_owned_opaque_handles_only;no_credentials_consumed" as const,
    secret_material: "never_returned" as const,
  };
  const report: AutonomousDomainAuditReport = { ...descriptor, report_digest: await digestJson(descriptor) };
  if (new TextEncoder().encode(JSON.stringify(report)).byteLength > MAX_AUTONOMOUS_DOMAIN_AUDIT_BYTES) throw new ArgumentError("domain audit report exceeds its bounded size");
  return structuredClone(report);
}

/** Validate a report produced by auditAutonomousDomainContracts before accepting a handoff. */
export async function validateAutonomousDomainAuditReport(value: unknown): Promise<AutonomousDomainAuditReport> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_DOMAIN_AUDIT_SCHEMA || value.retention !== RETENTION || value.execution !== EXECUTION || value.credential_posture !== "caller_owned_opaque_handles_only;no_credentials_consumed" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous domain audit report is malformed");
  if (!Array.isArray(value.rows) || value.rows.length < 1 || value.rows.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("autonomous domain audit rows are outside their bounds");
  const domains: AutonomousDomainName[] = [];
  for (const raw of value.rows) {
    if (!isObject(raw) || !AUTONOMOUS_DOMAIN_NAMES.includes(raw.domain as AutonomousDomainName)) throw new ArgumentError("autonomous domain audit row domain is invalid");
    const rowDigest = boundedDigest("autonomous domain audit row digest", raw.row_digest);
    const { row_digest: _rowDigest, ...descriptor } = raw;
    if (await digestJson(descriptor) !== rowDigest) throw new ArgumentError("autonomous domain audit row digest does not match its metadata");
    domains.push(raw.domain as AutonomousDomainName);
  }
  if (new Set(domains).size !== domains.length) throw new ArgumentError("autonomous domain audit rows contain duplicate domains");
  const reportDigest = boundedDigest("autonomous domain audit report digest", value.report_digest);
  const { report_digest: _reportDigest, ...descriptor } = value;
  if (await digestJson(descriptor) !== reportDigest) throw new ArgumentError("autonomous domain audit report digest does not match its metadata");
  return structuredClone(value as unknown as AutonomousDomainAuditReport);
}
