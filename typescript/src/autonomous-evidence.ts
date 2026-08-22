import { ArgumentError } from "./errors.js";
import type { AutonomousDomainName, AutonomousWorkflow } from "./autonomous.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_EVIDENCE_PLAN_SCHEMA = "bioprism-typescript-autonomous-evidence-plan/0.1" as const;
export const AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA = "bioprism-typescript-autonomous-evidence-requirement/0.1" as const;
export const AUTONOMOUS_EVIDENCE_COVERAGE_STATUSES = ["not_evaluated", "missing", "partial", "complete"] as const;
export const MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS = 16;
export const MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS = 512;
export const MAX_AUTONOMOUS_EVIDENCE_PLAN_BYTES = 256_000;

export type AutonomousEvidenceCoverageStatus = typeof AUTONOMOUS_EVIDENCE_COVERAGE_STATUSES[number];

function identifier(value: unknown, name: string): string {
  if (typeof value !== "string" || !value.trim() || new TextEncoder().encode(value).byteLength > 256 || !/^[A-Za-z0-9_.:+\- /]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value.trim();
}

function text(value: unknown, name: string): string {
  if (typeof value !== "string" || !value.trim() || new TextEncoder().encode(value).byteLength > 2_048 || value.includes("\0")) throw new ArgumentError(`${name} must be bounded non-empty text`);
  return value.trim();
}

function list(value: unknown, name: string, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must be a bounded array`);
  const result = value.map((item) => identifier(item, `${name} entry`));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return result;
}

function digest(value: unknown, name: string): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

export interface AutonomousEvidenceRequirement extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA;
  requirement_id: string;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  stage_id: string;
  label: string;
  objective: string;
  required_capabilities: string[];
  evaluator_signals: string[];
  depends_on: string[];
}

export interface AutonomousEvidencePlanJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_PLAN_SCHEMA;
  domains: AutonomousDomainName[];
  workflow_ids: string[];
  workflow_digests: string[];
  requirements: AutonomousEvidenceRequirement[];
  available_evidence: string[];
  covered_requirement_ids: string[];
  missing_requirement_ids: string[];
  next_stage_ids: string[];
  coverage_status: AutonomousEvidenceCoverageStatus;
  plan_digest: string;
  coverage_ratio: number;
  retention: "evidence_contract_and_digests_only;raw_payloads_caller_owned";
  execution: "planning_only;no_source_or_provider_dispatch";
  does_not_claim: string[];
  secret_material: "never_returned";
}

export interface AutonomousEvidencePlanOptions {
  availableEvidence?: readonly string[];
  completedStages?: Readonly<Record<string, readonly string[]>>;
}

function payload(input: {
  domains: AutonomousDomainName[];
  workflowIds: string[];
  workflowDigests: string[];
  requirements: AutonomousEvidenceRequirement[];
  availableEvidence: string[];
  covered: string[];
  missing: string[];
  nextStages: string[];
  coverageStatus: AutonomousEvidenceCoverageStatus;
}): Record<string, unknown> {
  return {
    schema: AUTONOMOUS_EVIDENCE_PLAN_SCHEMA,
    domains: input.domains,
    workflow_ids: input.workflowIds,
    workflow_digests: input.workflowDigests,
    requirements: input.requirements,
    available_evidence: input.availableEvidence,
    covered_requirement_ids: input.covered,
    missing_requirement_ids: input.missing,
    next_stage_ids: input.nextStages,
    coverage_status: input.coverageStatus,
  };
}

/** A deterministic evidence contract; it never retrieves or validates source truth. */
export class AutonomousEvidencePlan {
  readonly schema = AUTONOMOUS_EVIDENCE_PLAN_SCHEMA;
  readonly domains: AutonomousDomainName[];
  readonly workflow_ids: string[];
  readonly workflow_digests: string[];
  readonly requirements: AutonomousEvidenceRequirement[];
  readonly available_evidence: string[];
  readonly covered_requirement_ids: string[];
  readonly missing_requirement_ids: string[];
  readonly next_stage_ids: string[];
  readonly coverage_status: AutonomousEvidenceCoverageStatus;
  readonly plan_digest: string;

  constructor(input: {
    domains: readonly AutonomousDomainName[];
    workflow_ids: readonly string[];
    workflow_digests: readonly string[];
    requirements: readonly AutonomousEvidenceRequirement[];
    available_evidence?: readonly string[];
    covered_requirement_ids?: readonly string[];
    missing_requirement_ids?: readonly string[];
    next_stage_ids?: readonly string[];
    coverage_status?: AutonomousEvidenceCoverageStatus;
    plan_digest?: string;
  }) {
    if (!Array.isArray(input.domains) || input.domains.length < 1 || input.domains.length > MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS) throw new ArgumentError("evidence plan domains are outside their bound");
    const domains = input.domains.map((domain) => identifier(domain, "evidence plan domain") as AutonomousDomainName);
    if (new Set(domains).size !== domains.length) throw new ArgumentError("evidence plan domains must be unique");
    const workflowIds = list(input.workflow_ids, "evidence plan workflow_ids", MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS);
    const workflowDigests = input.workflow_digests.map((value, index) => digest(value, `evidence plan workflow_digests[${index}]`));
    if (workflowIds.length !== domains.length || workflowDigests.length !== domains.length) throw new ArgumentError("evidence plan workflow metadata must align");
    if (!Array.isArray(input.requirements) || input.requirements.length > MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS) throw new ArgumentError("evidence plan requirements are outside their bound");
    const requirements = input.requirements.map((value, index) => {
      if (!value || value.schema !== AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA) throw new ArgumentError(`evidence requirement ${index} is malformed`);
      return {
        schema: AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA,
        requirement_id: identifier(value.requirement_id, `evidence requirement ${index}.requirement_id`),
        domain: identifier(value.domain, `evidence requirement ${index}.domain`) as AutonomousDomainName,
        workflow_id: identifier(value.workflow_id, `evidence requirement ${index}.workflow_id`),
        workflow_digest: digest(value.workflow_digest, `evidence requirement ${index}.workflow_digest`),
        stage_id: identifier(value.stage_id, `evidence requirement ${index}.stage_id`),
        label: identifier(value.label, `evidence requirement ${index}.label`),
        objective: text(value.objective, `evidence requirement ${index}.objective`),
        required_capabilities: list(value.required_capabilities, `evidence requirement ${index}.required_capabilities`, 64),
        evaluator_signals: list(value.evaluator_signals, `evidence requirement ${index}.evaluator_signals`, 64),
        depends_on: list(value.depends_on, `evidence requirement ${index}.depends_on`, 64),
      };
    });
    const ids = requirements.map((item) => item.requirement_id);
    if (new Set(ids).size !== ids.length) throw new ArgumentError("evidence plan requirement IDs must be unique");
    const available = list(input.available_evidence ?? [], "evidence plan available_evidence", MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS);
    const covered = list(input.covered_requirement_ids ?? [], "evidence plan covered_requirement_ids", MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS);
    const missing = list(input.missing_requirement_ids ?? [], "evidence plan missing_requirement_ids", MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS);
    const nextStages = list(input.next_stage_ids ?? [], "evidence plan next_stage_ids", MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS);
    const idSet = new Set(ids);
    if (covered.some((id) => !idSet.has(id)) || missing.some((id) => !idSet.has(id)) || covered.some((id) => missing.includes(id)) || new Set([...covered, ...missing]).size !== ids.length) throw new ArgumentError("evidence plan coverage IDs must partition requirements");
    const coverageStatus = input.coverage_status ?? "not_evaluated";
    if (!AUTONOMOUS_EVIDENCE_COVERAGE_STATUSES.includes(coverageStatus)) throw new ArgumentError("evidence plan coverage_status is invalid");
    const base = payload({ domains, workflowIds, workflowDigests, requirements, availableEvidence: available, covered, missing, nextStages, coverageStatus });
    const encoded = JSON.stringify(base);
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_EVIDENCE_PLAN_BYTES) throw new ArgumentError("evidence plan exceeds its bounded size");
    this.domains = domains;
    this.workflow_ids = workflowIds;
    this.workflow_digests = workflowDigests;
    this.requirements = requirements;
    this.available_evidence = available;
    this.covered_requirement_ids = covered;
    this.missing_requirement_ids = missing;
    this.next_stage_ids = nextStages;
    this.coverage_status = coverageStatus;
    this.plan_digest = input.plan_digest ?? "";
  }

  get coverage_ratio(): number { return this.requirements.length === 0 ? 1 : this.covered_requirement_ids.length / this.requirements.length; }

  async finalize(): Promise<AutonomousEvidencePlan> {
    const digestValue = await digestJson(payload({ domains: this.domains, workflowIds: this.workflow_ids, workflowDigests: this.workflow_digests, requirements: this.requirements, availableEvidence: this.available_evidence, covered: this.covered_requirement_ids, missing: this.missing_requirement_ids, nextStages: this.next_stage_ids, coverageStatus: this.coverage_status }));
    return this.plan_digest === digestValue ? this : new AutonomousEvidencePlan({ domains: this.domains, workflow_ids: this.workflow_ids, workflow_digests: this.workflow_digests, requirements: this.requirements, available_evidence: this.available_evidence, covered_requirement_ids: this.covered_requirement_ids, missing_requirement_ids: this.missing_requirement_ids, next_stage_ids: this.next_stage_ids, coverage_status: this.coverage_status, plan_digest: digestValue });
  }

  toJSON(): AutonomousEvidencePlanJSON {
    return {
      ...payload({ domains: [...this.domains], workflowIds: [...this.workflow_ids], workflowDigests: [...this.workflow_digests], requirements: this.requirements.map((item) => ({ ...item, required_capabilities: [...item.required_capabilities], evaluator_signals: [...item.evaluator_signals], depends_on: [...item.depends_on] })), availableEvidence: [...this.available_evidence], covered: [...this.covered_requirement_ids], missing: [...this.missing_requirement_ids], nextStages: [...this.next_stage_ids], coverageStatus: this.coverage_status }),
      plan_digest: this.plan_digest,
      coverage_ratio: this.coverage_ratio,
      retention: "evidence_contract_and_digests_only;raw_payloads_caller_owned",
      execution: "planning_only;no_source_or_provider_dispatch",
      does_not_claim: ["evidence was acquired", "a source is truthful or current", "a connector, tool, provider, or credential is available", "coverage proves task completion"],
      secret_material: "never_returned",
    } as AutonomousEvidencePlanJSON;
  }

  /**
   * A budget-friendly prompt projection that keeps the plan identity and
   * execution boundaries while leaving the full requirement catalogue on the
   * blueprint/facade. This is intentionally not a replacement for toJSON().
   */
  toPromptJSON(): JsonObject {
    return {
      schema: AUTONOMOUS_EVIDENCE_PLAN_SCHEMA,
      projection: "prompt_summary",
      plan_digest: this.plan_digest,
      domains: [...this.domains],
      workflow_ids: [...this.workflow_ids],
      coverage_status: this.coverage_status,
      coverage_ratio: this.coverage_ratio,
      requirement_count: this.requirements.length,
      covered_requirement_count: this.covered_requirement_ids.length,
      missing_requirement_count: this.missing_requirement_ids.length,
      available_evidence_count: this.available_evidence.length,
      next_stage_ids: [...this.next_stage_ids],
      retention: "evidence_contract_and_digests_only;raw_payloads_caller_owned",
      execution: "planning_only;no_source_or_provider_dispatch",
      does_not_claim: ["evidence was acquired", "a source is truthful or current", "coverage proves task completion"],
      secret_material: "never_returned",
    };
  }
}

/** Compile reviewed workflow stages into a deterministic evidence plan. */
export async function buildAutonomousEvidencePlan(workflows: readonly AutonomousWorkflow[], options: AutonomousEvidencePlanOptions = {}): Promise<AutonomousEvidencePlan> {
  if (!Array.isArray(workflows) || workflows.length < 1 || workflows.length > MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS) throw new ArgumentError("autonomous evidence workflows must contain 1..16 workflows");
  const domains = workflows.map((workflow) => identifier(workflow.domain, "autonomous evidence workflow domain") as AutonomousDomainName);
  if (new Set(domains).size !== domains.length) throw new ArgumentError("autonomous evidence workflows must use unique domains");
  const workflowIds = workflows.map((workflow) => identifier(workflow.workflow_id, "autonomous evidence workflow_id"));
  const workflowDigests = workflows.map((workflow, index) => digest(workflow.workflow_digest, `autonomous evidence workflow ${index} digest`));
  const requirements: AutonomousEvidenceRequirement[] = [];
  for (const [workflowIndex, workflow] of workflows.entries()) {
    if (!Array.isArray(workflow.stages) || workflow.stages.length === 0) throw new ArgumentError(`autonomous evidence workflow ${workflowIndex} has no stages`);
    const stageIds = new Set<string>();
    for (const stage of workflow.stages) {
      const stageId = identifier(stage.id, "autonomous evidence stage id");
      if (stageIds.has(stageId)) throw new ArgumentError("autonomous evidence stage IDs must be unique");
      stageIds.add(stageId);
      const capabilities = list(stage.required_capabilities, "autonomous evidence stage capabilities", 64);
      const signals = list(stage.evaluator_signals, "autonomous evidence stage evaluator signals", 64);
      const dependencies = list(stage.depends_on, "autonomous evidence stage dependencies", 64);
      const objective = text(stage.objective, "autonomous evidence stage objective");
      if (!capabilities.length) throw new ArgumentError("autonomous evidence stage must require a capability");
      for (const label of list(stage.evidence_outputs, "autonomous evidence stage outputs", 64)) requirements.push({ schema: AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA, requirement_id: `${workflow.domain}:${stageId}:${label}`, domain: workflow.domain, workflow_id: workflow.workflow_id, workflow_digest: workflow.workflow_digest, stage_id: stageId, label, objective, required_capabilities: capabilities, evaluator_signals: signals, depends_on: dependencies });
    }
  }
  if (requirements.length > MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS) throw new ArgumentError("autonomous evidence requirements exceed their bound");
  const available = list(options.availableEvidence ?? [], "autonomous evidence availableEvidence", MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS);
  const byLabel = new Map<string, string[]>();
  for (const item of requirements) byLabel.set(item.label, [...(byLabel.get(item.label) ?? []), item.requirement_id]);
  const covered = requirements.filter((item) => available.includes(item.requirement_id) || (available.includes(item.label) && (byLabel.get(item.label)?.length ?? 0) === 1)).map((item) => item.requirement_id);
  const missing = requirements.map((item) => item.requirement_id).filter((id) => !covered.includes(id));
  const completed = options.completedStages ?? {};
  const nextStages: string[] = [];
  for (const workflow of workflows) {
    const done = new Set(list(completed[workflow.domain] ?? [], `completed stages for ${workflow.domain}`, 64));
    for (const stage of workflow.stages) if (!done.has(stage.id) && stage.depends_on.every((dependency: string) => done.has(dependency))) nextStages.push(`${workflow.domain}:${stage.id}`);
  }
  const coverageStatus: AutonomousEvidenceCoverageStatus = available.length === 0 ? "not_evaluated" : missing.length === 0 ? "complete" : covered.length ? "partial" : "missing";
  return new AutonomousEvidencePlan({ domains, workflow_ids: workflowIds, workflow_digests: workflowDigests, requirements, available_evidence: available, covered_requirement_ids: covered, missing_requirement_ids: missing, next_stage_ids: nextStages, coverage_status: coverageStatus }).finalize();
}
