import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  autonomousConnectorMissionExecutor,
  type AutonomousConnectorMissionExecutorOptions,
  type AutonomousMissionConnectorAdapterOptions,
} from "./autonomous-connector-adapters.js";
import {
  AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL,
  type AutonomousMissionExecuteOptions,
  type AutonomousMissionExecutionResult,
} from "./mission-execution.js";
import { digestJsonSync, ToolCatalogue } from "./tooling.js";
import type {
  AgentMissionArgs,
  AgentMissionStep,
  AutonomousOrderedStepPlanRefinementResult,
  JsonObject,
  JsonValue,
} from "./types.js";
import type {
  AutonomousAgent,
  AutonomousOrderedStepPlanRequest,
  AutonomousOrderedStepPlanStep,
  AutonomousProviderPlanningOptions,
} from "./autonomous.js";
import type { AutonomousLaunchAdmissionReport } from "./autonomous-launch-admission.js";

/** Stable schema for the all-domain connector mission composition boundary. */
export const AUTONOMOUS_CONNECTOR_MISSION_SCHEMA = "bioprism-typescript-autonomous-connector-mission/0.1" as const;
/** Stable schema for a provider-ordered mission proposal before caller acceptance. */
export const AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA = "bioprism-typescript-autonomous-connector-planned-mission/0.1" as const;
export const AUTONOMOUS_CONNECTOR_MISSION_MAX_STEPS = AUTONOMOUS_MISSION_MAX_STEPS_PER_CALL;

export type AutonomousConnectorMissionPlanningStatus =
  | "planning_approval_required"
  | "planning_acceptance_required"
  | "planning_review_required"
  | "planning_provider_invalid"
  | "planning_provider_failed"
  | "planning_provider_disagreement"
  | "planning_plan_refused"
  | "planning_policy_review_required"
  | "planning_policy_blocked"
  | string;

export interface AutonomousConnectorMissionRunOptions extends Omit<AutonomousConnectorMissionExecutorOptions, "catalogue" | "executeStep"> {
  catalogue: ToolCatalogue;
  /** Local executor controls. Provider approval remains explicit. */
  execute?: AutonomousMissionExecuteOptions;
}

/** Agent-facing options allow the attached catalogue to be used by default. */
export interface AutonomousConnectorMissionAgentRunOptions extends Omit<AutonomousConnectorMissionRunOptions, "catalogue"> {
  catalogue?: ToolCatalogue;
}

export interface AutonomousConnectorMissionProviderPlanningOptions {
  execution: AutonomousConnectorMissionAgentRunOptions;
  providerPlanning?: AutonomousProviderPlanningOptions;
  /** A previously reviewed value-only refinement. Supplying it is a replay, not a new plan call. */
  acceptedPlanRefinement?: AutonomousOrderedStepPlanRefinementResult;
  /** Explicitly promote the exact refinement to execution. */
  acceptPlan?: boolean;
}

export interface AutonomousConnectorMissionPlannedRunJSON extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA;
  status: string;
  mission_id: string;
  protected_contract_digest: string;
  plan_refinement: JsonObject;
  execution: JsonObject | null;
  plan_refinement_digest: string;
  retention: "metadata_only;mission_arguments_connector_values_and_provider_material_not_retained";
  authorization: "planning_is_proposal_only;execution_requires_explicit_plan_acceptance_connector_approval_and_domain_admission";
  secret_material: "never_returned";
  result_digest: string;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function boundedText(name: string, value: unknown, maximum = 2_048): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\u0000")) {
    throw new ArgumentError(`${name} must be bounded text`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 512);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a safe identifier`);
  return text;
}

function supportedDomain(name: string, value: unknown): AutonomousDomainName {
  const domain = boundedText(name, value, 128);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`${name} is not an autonomous domain`);
  return domain as AutonomousDomainName;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function optionalDigest(value: unknown): string | null {
  return value === null || value === undefined ? null : typeof value === "string" && /^[0-9a-f]{64}$/.test(value) ? value : null;
}

function safeLabel(value: unknown): string | null {
  return typeof value === "string" && /^[A-Za-z0-9_.:/-]{1,256}$/.test(value) ? value : null;
}

function normalizeMission(value: AgentMissionArgs): AgentMissionArgs {
  if (!isObject(value)) throw new ArgumentError("connector mission must be an AgentMissionArgs object");
  const mission = clone(value as AgentMissionArgs);
  boundedIdentifier("mission_id", mission.mission_id);
  boundedText("goal", mission.goal, 32_000);
  if (!Array.isArray(mission.steps) || mission.steps.length < 1 || mission.steps.length > AUTONOMOUS_CONNECTOR_MISSION_MAX_STEPS) {
    throw new ArgumentError(`connector mission steps must contain between 1 and ${AUTONOMOUS_CONNECTOR_MISSION_MAX_STEPS} steps`);
  }
  const ids = new Set<string>();
  for (const [index, step] of mission.steps.entries()) {
    if (!isObject(step)) throw new ArgumentError(`connector mission steps[${index}] must be an object`);
    boundedIdentifier(`connector mission steps[${index}].id`, step.id);
    if (ids.has(step.id as string)) throw new ArgumentError(`connector mission contains duplicate step id: ${step.id}`);
    ids.add(step.id as string);
    supportedDomain(`connector mission steps[${index}].domain`, step.domain);
    boundedText(`connector mission steps[${index}].capability`, step.capability, 512);
    boundedText(`connector mission steps[${index}].objective`, step.objective, 32_000);
    boundedIdentifier(`connector mission steps[${index}].tool`, step.tool);
    if (step.arguments !== undefined && !isObject(step.arguments)) throw new ArgumentError(`connector mission steps[${index}].arguments must be a JSON object`);
    if (step.depends_on !== undefined) {
      if (!Array.isArray(step.depends_on)) throw new ArgumentError(`connector mission steps[${index}].depends_on must be an array`);
      const dependencies = step.depends_on;
      const dependencyIds = new Set<string>();
      for (const dependency of dependencies) {
        boundedIdentifier(`connector mission steps[${index}].depends_on entry`, dependency);
        if (dependency === step.id) throw new ArgumentError(`connector mission step ${step.id} cannot depend on itself`);
        if (dependencyIds.has(dependency)) throw new ArgumentError(`connector mission step ${step.id} contains a duplicate dependency`);
        dependencyIds.add(dependency);
      }
    }
  }
  for (const step of mission.steps) {
    for (const dependency of step.depends_on ?? []) {
      if (!ids.has(dependency)) throw new ArgumentError(`connector mission step ${step.id} depends on an unknown step: ${dependency}`);
    }
  }
  assertAcyclic(mission.steps);
  return mission;
}

function assertAcyclic(steps: readonly AgentMissionStep[]): void {
  const indegree = new Map(steps.map((step) => [step.id, 0]));
  const children = new Map<string, string[]>();
  for (const step of steps) {
    for (const dependency of step.depends_on ?? []) {
      indegree.set(step.id, (indegree.get(step.id) ?? 0) + 1);
      children.set(dependency, [...(children.get(dependency) ?? []), step.id]);
    }
  }
  const ready = [...indegree.entries()].filter(([, value]) => value === 0).map(([id]) => id);
  let visited = 0;
  while (ready.length) {
    const id = ready.shift() as string;
    visited += 1;
    for (const child of children.get(id) ?? []) {
      const next = (indegree.get(child) ?? 0) - 1;
      indegree.set(child, next);
      if (next === 0) ready.push(child);
    }
  }
  if (visited !== steps.length) throw new ArgumentError("connector mission step graph contains a cycle");
}

function missionSteps(value: AgentMissionArgs | readonly AgentMissionStep[]): AgentMissionStep[] {
  if (Array.isArray(value)) return clone(value as AgentMissionStep[]);
  return normalizeMission(value as AgentMissionArgs).steps;
}

/**
 * Produce the provider-visible step catalogue. Tools, arguments, bindings, policies, claims,
 * and workflow metadata are intentionally excluded from this projection.
 */
export function connectorMissionPlannerSteps(value: AgentMissionArgs | readonly AgentMissionStep[]): AutonomousOrderedStepPlanStep[] {
  const steps = missionSteps(value);
  const missionLike = { mission_id: "planner", goal: "planner", steps } as AgentMissionArgs;
  const normalized = normalizeMission(missionLike).steps;
  return normalized.map((step) => ({
    id: step.id,
    domain: supportedDomain(`step ${step.id}.domain`, step.domain),
    capability: step.capability,
    objective: step.objective,
    ...(step.depends_on === undefined ? {} : { depends_on: [...step.depends_on] }),
    required: step.required ?? true,
  }));
}

/**
 * Digest the full caller-owned mission contract independent of step order. This binds arguments,
 * tools, dependencies, policy, claims, bindings, and route metadata without exposing them to a
 * provider planner or putting them into a durable result projection.
 */
export function connectorMissionProtectedContractDigest(mission: AgentMissionArgs): string {
  const normalized = normalizeMission(mission);
  const descriptor = clone(normalized) as AgentMissionArgs;
  descriptor.steps = [...normalized.steps].sort((left, right) => left.id.localeCompare(right.id));
  return digestJsonSync(descriptor);
}

function basePlanDigest(mission: AgentMissionArgs): string {
  return digestJsonSync({ steps: connectorMissionPlannerSteps(mission) });
}

function taskDigest(mission: AgentMissionArgs): string {
  return digestJsonSync({ task: mission.goal });
}

function validateRefinement(refinement: AutonomousOrderedStepPlanRefinementResult): AutonomousOrderedStepPlanRefinementResult {
  if (!isObject(refinement)) throw new ArgumentError("ordered mission plan refinement must be an object");
  if (refinement.schema !== "bioprism-typescript-autonomous-ordered-step-plan-refinement/0.1") throw new ArgumentError("ordered mission plan refinement schema is unsupported");
  if (refinement.status !== "completed") throw new ArgumentError(`ordered mission plan refinement is not executable: ${refinement.status}`);
  if (refinement.review_required !== false) throw new ArgumentError("ordered mission plan refinement still requires review");
  if (!Array.isArray(refinement.priority_step_ids) || !Array.isArray(refinement.focus_step_ids)) throw new ArgumentError("ordered mission plan refinement step ids are malformed");
  if (!Number.isFinite(refinement.confidence) || refinement.confidence < 0 || refinement.confidence > 1) throw new ArgumentError("ordered mission plan refinement confidence is malformed");
  digest("ordered mission plan task_digest", refinement.task_digest);
  digest("ordered mission plan base_plan_digest", refinement.base_plan_digest);
  digest("ordered mission plan protected_contract_digest", refinement.protected_contract_digest);
  const unique = (name: string, values: string[]): void => {
    if (values.some((value) => typeof value !== "string" || !/^[A-Za-z0-9_.:-]+$/.test(value)) || new Set(values).size !== values.length) throw new ArgumentError(`${name} contains invalid or duplicate step ids`);
  };
  unique("ordered mission plan priority_step_ids", refinement.priority_step_ids);
  unique("ordered mission plan focus_step_ids", refinement.focus_step_ids);
  return clone(refinement);
}

/** Apply a caller-accepted provider ordering while preserving every protected mission field. */
export function applyAutonomousOrderedStepPlan(
  mission: AgentMissionArgs,
  refinement: AutonomousOrderedStepPlanRefinementResult,
  expectedProtectedContractDigest?: string,
): AgentMissionArgs {
  const normalized = normalizeMission(mission);
  const plan = validateRefinement(refinement);
  const expectedTask = taskDigest(normalized);
  const expectedBase = basePlanDigest(normalized);
  const expectedContract = connectorMissionProtectedContractDigest(normalized);
  if (plan.task_digest !== expectedTask) throw new ArgumentError("ordered mission plan task digest does not match the mission");
  if (plan.base_plan_digest !== expectedBase) throw new ArgumentError("ordered mission plan base digest does not match the mission");
  if (plan.protected_contract_digest !== expectedContract) throw new ArgumentError("ordered mission plan protected contract digest does not match the mission");
  if (expectedProtectedContractDigest !== undefined && digest("expected protected contract digest", expectedProtectedContractDigest) !== expectedContract) throw new ArgumentError("expected protected contract digest does not match the mission");
  const ids = normalized.steps.map((step) => step.id);
  if (plan.priority_step_ids.length !== ids.length || new Set(plan.priority_step_ids).size !== ids.length || plan.priority_step_ids.some((id) => !ids.includes(id))) {
    throw new ArgumentError("ordered mission plan must contain each existing step exactly once");
  }
  if (plan.focus_step_ids.some((id) => !ids.includes(id))) throw new ArgumentError("ordered mission plan focus ids must refer to existing steps");
  const positions = new Map(plan.priority_step_ids.map((id, index) => [id, index]));
  for (const step of normalized.steps) {
    for (const dependency of step.depends_on ?? []) {
      if ((positions.get(dependency) ?? -1) > (positions.get(step.id) ?? -1)) throw new ArgumentError(`ordered mission plan moves ${step.id} before dependency ${dependency}`);
    }
  }
  const byId = new Map(normalized.steps.map((step) => [step.id, step]));
  const ordered = plan.priority_step_ids.map((id) => clone(byId.get(id) as AgentMissionStep));
  const promoted = { ...clone(normalized), steps: ordered };
  if (connectorMissionProtectedContractDigest(promoted) !== expectedContract) throw new ArgumentError("ordered mission plan changed the protected mission contract");
  return promoted;
}

/** Execute a normalized mission with the strict connector adapter and durable metadata kernel. */
export async function runAutonomousConnectorMission(
  mission: AgentMissionArgs,
  options: AutonomousConnectorMissionRunOptions,
): Promise<AutonomousMissionExecutionResult> {
  const normalized = normalizeMission(mission);
  if (!options || !(options.catalogue instanceof ToolCatalogue)) throw new ArgumentError("connector mission run requires a ToolCatalogue");
  if (!options.connector || typeof options.connector !== "object") throw new ArgumentError("connector mission run requires connector adapter options");
  const { execute, ...executorOptions } = options;
  const executor = autonomousConnectorMissionExecutor({ ...executorOptions, catalogue: options.catalogue });
  return executor.start(normalized, execute ?? {});
}

function projectPlan(refinement: AutonomousOrderedStepPlanRefinementResult): JsonObject {
  const selectedModel = isObject(refinement.selected_model)
    && safeLabel(refinement.selected_model.provider) !== null
    && safeLabel(refinement.selected_model.model) !== null
    ? { provider: safeLabel(refinement.selected_model.provider) as string, model: safeLabel(refinement.selected_model.model) as string }
    : null;
  const costBudget = isObject(refinement.cost_budget)
    && typeof refinement.cost_budget.max_cost_units === "number"
    && Number.isFinite(refinement.cost_budget.max_cost_units)
    && typeof refinement.cost_budget.consumed_cost_units === "number"
    && Number.isFinite(refinement.cost_budget.consumed_cost_units)
    && typeof refinement.cost_budget.remaining_cost_units === "number"
    && Number.isFinite(refinement.cost_budget.remaining_cost_units)
    ? {
        max_cost_units: refinement.cost_budget.max_cost_units,
        consumed_cost_units: refinement.cost_budget.consumed_cost_units,
        remaining_cost_units: refinement.cost_budget.remaining_cost_units,
      }
    : null;
  const failure = isObject(refinement.failure)
    && (refinement.failure.error_class === "ProviderRuntimeError" || refinement.failure.error_class === "CredentialError")
    && safeLabel(refinement.failure.code) !== null
    && typeof refinement.failure.retryable === "boolean"
    && (refinement.failure.status_code === null || typeof refinement.failure.status_code === "number")
    && typeof refinement.failure.circuit_open === "boolean"
    ? {
        error_class: refinement.failure.error_class,
        code: safeLabel(refinement.failure.code) as string,
        retryable: refinement.failure.retryable,
        status_code: refinement.failure.status_code,
        circuit_open: refinement.failure.circuit_open,
        retention: "metadata_only;provider_error_message_and_payloads_not_retained",
        secret_material: "never_returned",
      }
    : null;
  const value: JsonObject = {
    schema: "bioprism-typescript-autonomous-ordered-step-plan-refinement/0.1",
    status: ["completed", "approval_required", "plan_refused", "provider_invalid", "provider_failed", "provider_disagreement", "policy_review_required", "policy_blocked"].includes(refinement.status) ? refinement.status : "provider_invalid",
    task_digest: refinement.task_digest,
    base_plan_digest: refinement.base_plan_digest,
    protected_contract_digest: refinement.protected_contract_digest,
    priority_step_ids: [...refinement.priority_step_ids],
    focus_step_ids: [...refinement.focus_step_ids],
    review_required: refinement.review_required,
    confidence: refinement.confidence,
    selected_model: selectedModel,
    selection_digest: optionalDigest(refinement.selection_digest),
    planner_prompt_digest: optionalDigest(refinement.planner_prompt_digest),
    planner_plan_digest: optionalDigest(refinement.planner_plan_digest),
    outcome_digest: optionalDigest(refinement.outcome_digest),
    ...(optionalDigest(refinement.planner_context_digest) === null ? {} : { planner_context_digest: optionalDigest(refinement.planner_context_digest) as string }),
    cost_budget: costBudget,
    ...(failure === null ? {} : { failure }),
    retention: "step_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_arguments_or_effects_authorized",
  };
  return value;
}

function projectExecution(result: AutonomousMissionExecutionResult): JsonObject {
  const preflight = result.preflight;
  return {
    schema: result.schema,
    status: result.status,
    mission_id: result.mission_id,
    preflight: {
      schema: preflight.schema,
      mission_id: preflight.mission_id,
      request_digest: preflight.request_digest,
      catalogue_digest: preflight.catalogue_digest,
      execution: preflight.execution,
      execution_mode: preflight.execution_mode,
      max_parallelism: preflight.max_parallelism,
      ok: preflight.ok,
      fully_checked: preflight.fully_checked,
      ordered_steps: [...preflight.ordered_steps],
      waves: preflight.waves.map((wave) => [...wave]),
      issues: [...preflight.issues],
      warnings: [...preflight.warnings],
      steps: preflight.steps.map((step) => ({
        id: step.id,
        tool: step.tool,
        depends_on: [...step.depends_on],
        wave: step.wave,
        status: step.status,
        schema: step.schema === null ? null : { ...step.schema },
        issues: [...step.issues],
        warnings: [...step.warnings],
      })),
      limitations: [...preflight.limitations],
    },
    checkpoint: result.checkpoint === null ? null : clone(result.checkpoint) as unknown as JsonValue,
    route: null,
    semantic_route_status: result.semantic_route_status,
    events: result.events.map((event) => ({
      schema: event.schema,
      sequence: event.sequence,
      mission_id: event.mission_id,
      event_type: event.event_type,
      wave: event.wave,
      step_id: event.step_id,
      tool: event.tool,
      status: event.status,
      arguments_digest: event.arguments_digest,
      output_bytes: event.output_bytes,
      detail: event.detail,
      checkpoint_digest: event.checkpoint_digest,
      previous_event_digest: event.previous_event_digest,
      event_digest: event.event_digest,
      retention: event.retention,
      secret_material: event.secret_material,
    })),
    results: result.results.map((step) => ({
      step_id: step.step.id,
      domain: step.step.domain,
      capability: step.step.capability,
      tool: step.step.tool,
      depends_on: [...(step.step.depends_on ?? [])],
      status: step.status,
      result_digest: step.result_digest,
      output_bytes: step.output_bytes,
      error_class: step.error_class,
      run_status: step.run_status,
      learning_episode_id: step.learning_episode_id,
      decision: step.decision === null ? null : clone(step.decision),
      quality: step.quality === null ? null : clone(step.quality),
      attempt: step.attempt,
    })),
    completed_steps: result.completed_steps,
    total_steps: result.total_steps,
    succeeded_steps: result.succeeded_steps,
    refused_steps: result.refused_steps,
    blocked_steps: result.blocked_steps,
    failed_steps: result.failed_steps,
    cancelled_steps: result.cancelled_steps,
    returned_bytes: result.returned_bytes,
    next_wave: result.next_wave,
    recovery: result.recovery,
    retention: result.retention,
    secret_material: result.secret_material,
  };
}

/**
 * A planning/execution result with a safe JSON projection. The in-memory mission and raw
 * execution remain caller-owned; `toJSON()` is deliberately digest-only for persistence/logging.
 */
export class AutonomousConnectorPlannedMissionRun {
  readonly schema = AUTONOMOUS_CONNECTOR_MISSION_SCHEMA;
  readonly mission_id: string;
  readonly status: string;
  readonly plan_refinement: AutonomousOrderedStepPlanRefinementResult;
  readonly execution: AutonomousMissionExecutionResult | null;
  readonly protected_contract_digest: string;
  private readonly mission: AgentMissionArgs;

  constructor(
    mission: AgentMissionArgs,
    status: string,
    planRefinement: AutonomousOrderedStepPlanRefinementResult,
    protectedContractDigest: string,
    execution: AutonomousMissionExecutionResult | null = null,
  ) {
    this.mission = clone(mission);
    this.mission_id = mission.mission_id;
    this.status = status;
    this.plan_refinement = clone(planRefinement);
    this.protected_contract_digest = digest("protected contract digest", protectedContractDigest);
    this.execution = execution === null ? null : clone(execution);
  }

  toJSON(): AutonomousConnectorMissionPlannedRunJSON {
    const body = {
      schema: AUTONOMOUS_CONNECTOR_PLANNED_MISSION_SCHEMA,
      status: this.status,
      mission_id: this.mission_id,
      protected_contract_digest: this.protected_contract_digest,
      plan_refinement: projectPlan(this.plan_refinement),
      execution: this.execution === null ? null : projectExecution(this.execution),
      plan_refinement_digest: digestJsonSync(projectPlan(this.plan_refinement)),
      retention: "metadata_only;mission_arguments_connector_values_and_provider_material_not_retained" as const,
      authorization: "planning_is_proposal_only;execution_requires_explicit_plan_acceptance_connector_approval_and_domain_admission" as const,
      secret_material: "never_returned" as const,
    };
    return { ...body, result_digest: digestJsonSync(body) };
  }
}

function planningStatus(refinement: AutonomousOrderedStepPlanRefinementResult, accepted: boolean): string {
  if (refinement.status === "approval_required") return "planning_approval_required";
  if (refinement.status === "provider_invalid") return "planning_provider_invalid";
  if (refinement.status === "provider_failed") return "planning_provider_failed";
  if (refinement.status === "provider_disagreement") return "planning_provider_disagreement";
  if (refinement.status === "plan_refused") return "planning_plan_refused";
  if (refinement.status === "policy_review_required") return "planning_policy_review_required";
  if (refinement.status === "policy_blocked") return "planning_policy_blocked";
  if (refinement.status === "completed" && refinement.review_required) return "planning_review_required";
  if (refinement.status === "completed" && !accepted) return "planning_acceptance_required";
  return `planning_${refinement.status}`;
}

function requestedDomains(mission: AgentMissionArgs): AutonomousDomainName[] {
  return [...new Set(mission.steps.map((step) => supportedDomain(`step ${step.id}.domain`, step.domain)))] as AutonomousDomainName[];
}

function requireAgent(agent: AutonomousAgent): void {
  if (!agent || typeof agent.planOrderedStepsWithProvider !== "function") throw new ArgumentError("connector mission provider planning requires an AutonomousAgent");
}

/** Run a connector mission through provider ordering, explicit plan acceptance, and execution. */
export async function runAutonomousConnectorMissionWithProviderPlanning(
  agent: AutonomousAgent,
  mission: AgentMissionArgs,
  options: AutonomousConnectorMissionProviderPlanningOptions,
): Promise<AutonomousConnectorPlannedMissionRun> {
  requireAgent(agent);
  if (!options || !isObject(options.execution)) throw new ArgumentError("connector mission provider planning requires execution options");
  const normalized = normalizeMission(mission);
  const catalogue = options.execution.catalogue;
  if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("connector mission provider planning requires a ToolCatalogue");
  if (options.acceptedPlanRefinement !== undefined && options.providerPlanning !== undefined) throw new ArgumentError("accepted connector mission plan replay cannot invoke provider planning");
  const plannerSteps = connectorMissionPlannerSteps(normalized);
  const protectedContractDigest = connectorMissionProtectedContractDigest(normalized);
  const planRequest: AutonomousOrderedStepPlanRequest = {
    task: normalized.goal,
    steps: plannerSteps,
    domain: requestedDomains(normalized).length === 1 ? requestedDomains(normalized)[0] : "cross_domain",
    capability: "planning",
    basePlanDigest: digestJsonSync({ steps: plannerSteps }),
    protectedContractDigest,
  };
  const refinement = options.acceptedPlanRefinement === undefined
    ? await agent.planOrderedStepsWithProvider(planRequest, options.providerPlanning ?? {})
    : validateRefinement(options.acceptedPlanRefinement);
  const accepted = options.acceptPlan === true;
  const status = planningStatus(refinement, accepted);
  if (refinement.status !== "completed" || refinement.review_required || !accepted) {
    return new AutonomousConnectorPlannedMissionRun(normalized, status, refinement, protectedContractDigest);
  }
  const promoted = applyAutonomousOrderedStepPlan(normalized, refinement, protectedContractDigest);
  const execution = await runAutonomousConnectorMission(promoted, {
    ...options.execution,
    catalogue,
    agent,
  });
  return new AutonomousConnectorPlannedMissionRun(normalized, execution.status, refinement, protectedContractDigest, execution);
}

/** Execute only after a caller-owned launch admission approves every mission domain. */
export async function runAutonomousConnectorMissionWithLaunchAdmission(
  mission: AgentMissionArgs,
  admission: AutonomousLaunchAdmissionReport,
  options: AutonomousConnectorMissionRunOptions,
): Promise<AutonomousMissionExecutionResult> {
  const normalized = normalizeMission(mission);
  const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
  authorizeAutonomousLaunchDomains(admission, requestedDomains(normalized));
  return runAutonomousConnectorMission(normalized, options);
}

/** Provider-planned connector execution with launch admission checked before any planner call. */
export async function runAutonomousConnectorMissionWithProviderPlanningAndLaunchAdmission(
  agent: AutonomousAgent,
  mission: AgentMissionArgs,
  admission: AutonomousLaunchAdmissionReport,
  options: AutonomousConnectorMissionProviderPlanningOptions,
): Promise<AutonomousConnectorPlannedMissionRun> {
  const normalized = normalizeMission(mission);
  const { authorizeAutonomousLaunchDomains } = await import("./autonomous-launch-admission.js");
  authorizeAutonomousLaunchDomains(admission, requestedDomains(normalized));
  return runAutonomousConnectorMissionWithProviderPlanning(agent, normalized, options);
}

/** Exposed for agent-level tests and integrations that need the exact mission normalization. */
export function validateAutonomousConnectorMission(mission: AgentMissionArgs): AgentMissionArgs {
  return normalizeMission(mission);
}

export type { AutonomousMissionConnectorAdapterOptions };
