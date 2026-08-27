import { ArgumentError, isObject } from "./errors.js";
import { AutonomousCostBudget } from "./llm.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import {
  AUTONOMOUS_ROUTE_SCHEMA,
  type AutonomousAgent,
  acceptedAutonomousPlan,
  acceptedCrossDomainPlan,
  type AutonomousProviderPlanningOptions,
  type AutonomousCrossDomainRunOptions,
  type AutonomousCrossDomainRunResult,
  type AutonomousCrossDomainSubtask,
  type AutonomousAutoBlueprint,
  type AutonomousCrossDomainBlueprint,
  type AutonomousDomainName,
  type AutonomousTaskBlueprint,
  type AutonomousPromptChunk,
  type AutonomousRunOptions,
  type AutonomousRunResult,
  type AutonomousRouteProposal,
  validateAutonomousRouteOverride,
} from "./autonomous.js";
import {
  semanticRouteAutonomousTask,
  type AutonomousSemanticRouteResult,
} from "./autonomous-routing.js";
import type {
  AutonomousCrossDomainLearningSettlement,
  AutonomousEvaluatorRewardInput,
  AutonomousLearningController,
  AutonomousLearningSettlement,
  AutonomousLearningOutboxSettlementOptions,
  AutonomousPlanningQualitySettlement,
} from "./autonomous-learning.js";
import type {
  AutonomousEpisodicMemoryStore,
  AutonomousMemoryEpisode,
  AutonomousMemoryEvaluationInput,
  AutonomousMemoryQuery,
} from "./autonomous-memory.js";
import { taskFacetDigests } from "./autonomous-memory.js";
import { digestJson } from "./tooling.js";
import type { AutonomousCrossDomainPlanRefinementResult, AutonomousPlanRefinementResult, BrainEvaluatorAssessment, JsonObject } from "./types.js";
import type { AutonomousDomainPolicyExecutionMode } from "./autonomous-domain-policy.js";
import {
  AUTONOMOUS_CYCLE_REPLAN_MAX_REPLANS,
  AUTONOMOUS_CYCLE_REPLAN_STATE_SCHEMA,
  type AutonomousCycleReplanEvaluationRehydrator,
  type AutonomousCycleReplanInstructionRehydrator,
  type AutonomousCycleReplanMode,
  type AutonomousCycleReplanRehydrationContext,
  type AutonomousCycleReplanRouteRehydrator,
  type AutonomousCycleReplanRunRehydrator,
  type AutonomousCycleReplanState,
  type AutonomousCycleReplanStateStore,
  sealAutonomousCycleReplanState,
  validateAutonomousCycleReplanState,
} from "./autonomous-cycle-persistence.js";
import {
  AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA,
  type AutonomousDecisionCycleMode,
  type AutonomousDecisionCyclePhase,
  type AutonomousDecisionCycleRehydrationContext,
  type AutonomousDecisionCycleState,
  type AutonomousDecisionCycleStateStore,
  type AutonomousDecisionCycleTaskDecisionPosture,
  sealAutonomousDecisionCycleState,
  validateAutonomousDecisionCycleState,
} from "./autonomous-decision-persistence.js";

export const AUTONOMOUS_DECISION_CYCLE_SCHEMA = "bioprism-typescript-autonomous-decision-cycle/0.1" as const;

export type AutonomousDecisionCycleStatus =
  | "completed"
  | "approval_required"
  | "policy_review_required"
  | "policy_blocked"
  | "reconciliation_required"
  | "turn_limit_reached"
  | "route_review_required"
  | "provider_abstained"
  | "provider_invalid"
  | "provider_disagreement"
  | "plan_review_required";

export interface AutonomousDecisionCycleSemanticOptions {
  /** Semantic routing is opt-in because it sends the private task to a provider. */
  enabled?: boolean;
  approveProviderCall?: boolean;
  minSemanticConfidence?: number;
  maxDomains?: number;
  allowCrossDomain?: boolean;
  maxOutputTokens?: number;
  execution?: AutonomousExecutionController;
  executionAttempt?: number;
  maxProviderFailovers?: number;
  executionLifecycle?: "managed" | "observe_only";
  /** Strict routing admission is provider-free and is evaluated before classifier selection. */
  domainPolicyMode?: AutonomousDomainPolicyExecutionMode;
  domainPolicyEvidenceReady?: boolean;
  domainPolicyEvaluatorConfigured?: boolean;
  /** Routing is a proposal, so plan acceptance is intentionally internal and true. */
  domainPolicyEffectsRequested?: boolean;
  domainPolicyEffectsApproved?: boolean;
}

export type AutonomousDecisionCycleEvaluator = (
  result: AutonomousRunResult,
) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;

/** Explicit evaluator for the provider's accepted planning decision. */
export type AutonomousDecisionCyclePlanningEvaluator = (
  plan: AutonomousPlanRefinementResult | AutonomousCrossDomainPlanRefinementResult,
) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;

export interface AutonomousDecisionCycleLearningOptions {
  controller: AutonomousLearningController;
  episodeId: string;
  evaluate?: AutonomousDecisionCycleEvaluator;
  /** Planning quality is separate from execution quality and is never inferred from transport success. */
  evaluatePlanning?: AutonomousDecisionCyclePlanningEvaluator;
  remote?: boolean;
  outbox?: AutonomousLearningOutboxSettlementOptions;
}

export interface AutonomousDecisionCycleMemoryOptions {
  store: AutonomousEpisodicMemoryStore;
  query?: AutonomousMemoryQuery;
  limit?: number;
  tags?: readonly string[];
  /** Explicit memory ID for a single cycle; cross-domain cycles use it as a prefix. */
  episodeId?: string;
  episodePrefix?: string;
  lesson?: string | null;
  provenance?: Record<string, string>;
}

export interface AutonomousDecisionCycleMemoryProjection extends JsonObject {
  recalled_episode_ids: string[];
  recall_digest: string | null;
  recorded_episode_ids: string[];
  evaluation_recorded_episode_ids: string[];
}

export interface AutonomousDecisionCyclePersistenceOptions {
  /** Stable caller-owned identity for restart-safe ordinary decision cycles. */
  cycleId?: string;
  /** Metadata-only state store; task text, prompts, responses, and credentials stay caller-owned. */
  decisionStateStore?: AutonomousDecisionCycleStateStore;
  /** Permit an explicit retry of an interrupted provider-assisted semantic route after restart. */
  retrySemanticRoutingOnRestart?: boolean;
  /** Rehydrate a route that was already reviewed before the worker stopped. */
  rehydrateRoute?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousRouteProposal | Promise<AutonomousRouteProposal>;
  /** Rehydrate a provider outcome after a persisted execution boundary. */
  rehydrateRun?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousRunResult | AutonomousCrossDomainRunResult | Promise<AutonomousRunResult | AutonomousCrossDomainRunResult>;
  /** Rehydrate evaluator values after evaluation began; it must not return raw evidence. */
  rehydrateEvaluation?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousEvaluatorRewardInput | Record<string, AutonomousEvaluatorRewardInput> | Promise<AutonomousEvaluatorRewardInput | Record<string, AutonomousEvaluatorRewardInput>>;
  /** Rehydrate the explicit planner evaluator value after a durable evaluation boundary. */
  rehydratePlanningEvaluation?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;
  /** Rehydrate a terminal private result from caller-owned result storage. */
  rehydrateResult?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult | Promise<AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult>;
}

export interface AutonomousDecisionCycleOptions extends Omit<AutonomousRunOptions, "learning">, AutonomousDecisionCyclePersistenceOptions {
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  /** Optional provider proposal phase; it never executes unless acceptPlan is true. */
  providerPlanning?: AutonomousProviderPlanningOptions;
  /** Explicitly accept a completed, non-review provider proposal for this cycle. */
  acceptPlan?: boolean;
  learning?: AutonomousDecisionCycleLearningOptions;
  memory?: AutonomousDecisionCycleMemoryOptions;
}

export interface AutonomousDecisionCycleResult {
  schema: typeof AUTONOMOUS_DECISION_CYCLE_SCHEMA;
  status: AutonomousDecisionCycleStatus;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  run: AutonomousRunResult | null;
  plan_refinement: AutonomousPlanRefinementResult | null;
  learning_episode_id: string | null;
  evaluation: BrainEvaluatorAssessment | null;
  settlement: AutonomousLearningSettlement | null;
  planner_evaluation: AutonomousEvaluatorRewardInput | null;
  planner_settlement: AutonomousPlanningQualitySettlement | null;
  memory: AutonomousDecisionCycleMemoryProjection | null;
  retention: "provider_response_local; value_only_evaluation_and_learning_projection";
  authorization: "routing_and_provider_invocation_require_separate_explicit_approval";
}

/**
 * One automatic evaluator-backed cycle which chooses the correct decision-cycle kernel from
 * the reviewed route.  The route is resolved exactly once: deterministic routing is provider
 * free, while semantic routing is an explicitly approved classifier proposal.  The selected
 * route is then handed to the single- or cross-domain cycle as a digest-verified override.
 */
export const AUTONOMOUS_AUTO_DECISION_CYCLE_SCHEMA = "bioprism-typescript-autonomous-auto-decision-cycle/0.1" as const;

export type AutonomousAutoDecisionCycleOptions = Omit<AutonomousCrossDomainDecisionCycleOptions, "learning" | "semanticRouting"> & {
  domain?: AutonomousDomainName;
  /** Provider-free route confidence floor used before the cycle kernel is selected. */
  minConfidence?: number;
  /** Provider-free route separation floor used before the cycle kernel is selected. */
  minMargin?: number;
  /** Maximum number of routed domains admitted into the bounded fan-out. */
  maxDomains?: number;
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  learning?: AutonomousDecisionCycleLearningOptions | AutonomousCrossDomainDecisionCycleLearningOptions;
  subtasks?: readonly AutonomousCrossDomainSubtask[];
};

export type AutonomousAutoDecisionCycleStatus = AutonomousDecisionCycleStatus | AutonomousCrossDomainDecisionCycleStatus;

export type AutonomousAutoDecisionCycleMode = "single_domain" | "cross_domain";

export interface AutonomousAutoDecisionCycleResult {
  schema: typeof AUTONOMOUS_AUTO_DECISION_CYCLE_SCHEMA;
  status: AutonomousAutoDecisionCycleStatus;
  mode: AutonomousAutoDecisionCycleMode | null;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  cycle: AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult | null;
  next_action: "review_route" | "review_plan" | "review_provider_or_effect_approval" | "inspect_result" | "complete";
  retention: "provider_response_local;route_and_cycle_metadata_value_only;execution_result_caller_owned";
  authorization: "routing_planning_provider_effects_and_evaluator_settlement_remain_explicit";
}

/**
 * One automatic evaluator-guided replan cycle. The facade resolves the route once and then
 * selects the single-domain or cross-domain replan kernel without asking the caller to encode
 * the route shape a second time.
 */
export const AUTONOMOUS_AUTO_REPLAN_CYCLE_SCHEMA = "bioprism-typescript-autonomous-auto-replan-cycle/0.1" as const;

export type AutonomousAutoReplanCycleOptions = Omit<AutonomousCrossDomainReplanCycleOptions, "evaluate" | "learning" | "semanticRouting"> & {
  domain?: AutonomousDomainName;
  /** Provider-free route confidence floor used before the replan kernel is selected. */
  minConfidence?: number;
  /** Provider-free route separation floor used before the replan kernel is selected. */
  minMargin?: number;
  /** Maximum number of routed domains admitted into the bounded fan-out. */
  maxDomains?: number;
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  evaluate: AutonomousReplanEvaluator | AutonomousCrossDomainReplanEvaluator;
  learning?: AutonomousReplanLearningOptions | AutonomousCrossDomainReplanLearningOptions;
  subtasks?: readonly AutonomousCrossDomainSubtask[];
};

export type AutonomousAutoReplanCycleStatus = AutonomousReplanCycleStatus | AutonomousCrossDomainReplanCycleStatus;

export type AutonomousAutoReplanCycleMode = "single_domain" | "cross_domain";

export interface AutonomousAutoReplanCycleResult {
  schema: typeof AUTONOMOUS_AUTO_REPLAN_CYCLE_SCHEMA;
  status: AutonomousAutoReplanCycleStatus;
  mode: AutonomousAutoReplanCycleMode | null;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  cycle: AutonomousReplanCycleResult | AutonomousCrossDomainReplanCycleResult | null;
  next_action: "review_route" | "review_plan" | "review_provider_or_effect_approval" | "inspect_result" | "complete";
  retention: "provider_response_local;route_and_replan_metadata_value_only;execution_result_caller_owned";
  authorization: "routing_planning_provider_effects_evaluator_settlement_and_replanning_remain_explicit";
}

const RETENTION = "provider_response_local; value_only_evaluation_and_learning_projection" as const;
const AUTHORIZATION = "routing_and_provider_invocation_require_separate_explicit_approval" as const;

function emptyMemoryProjection(): AutonomousDecisionCycleMemoryProjection {
  return { recalled_episode_ids: [], recall_digest: null, recorded_episode_ids: [], evaluation_recorded_episode_ids: [] };
}

interface RecalledMemory {
  episodes: AutonomousMemoryEpisode[];
  projection: AutonomousDecisionCycleMemoryProjection;
  promptChunk: AutonomousPromptChunk | null;
}

async function recallMemory(memory: AutonomousDecisionCycleMemoryOptions | undefined, route: AutonomousRouteProposal, task: string, ranking: AutonomousMemoryQuery["ranking"] = "planning"): Promise<RecalledMemory> {
  if (!memory) return { episodes: [], projection: emptyMemoryProjection(), promptChunk: null };
  if (!memory.store || typeof memory.store.retrieve !== "function" || typeof memory.store.recordEpisode !== "function") throw new ArgumentError("autonomous cycle memory store is malformed");
  const query: AutonomousMemoryQuery = { ...(memory.query ?? {}) };
  if (query.domain === undefined && !route.cross_domain && route.primary_domain) query.domain = route.primary_domain;
  if (query.task_digest === undefined && query.task_facets === undefined) query.task_facets = taskFacetDigests(task);
  if (query.ranking === undefined) query.ranking = ranking;
  if (memory.limit !== undefined) query.limit = memory.limit;
  const episodes = await memory.store.retrieve(query);
  const recallDigest = await digestJson(episodes.map((episode) => ({ episode_id: episode.episode_id, episode_digest: episode.episode_digest, evaluation_digest: episode.evaluation?.evaluation_digest ?? null })));
  const projection: AutonomousDecisionCycleMemoryProjection = { recalled_episode_ids: episodes.map((episode) => episode.episode_id), recall_digest: recallDigest, recorded_episode_ids: [], evaluation_recorded_episode_ids: [] };
  if (!episodes.length) return { episodes, projection, promptChunk: null };
  const summary = episodes.map((episode) => ({
    episode_id: episode.episode_id,
    task_digest: episode.task_digest,
    context: episode.context,
    selected_model: episode.selected_model,
    route: episode.route,
    digests: episode.digests,
    tags: episode.tags,
    lesson: episode.lesson,
    evaluation: episode.evaluation ? { evaluator_id: episode.evaluation.evaluator_id, evaluator_version: episode.evaluation.evaluator_version, reward: episode.evaluation.reward, passed: episode.evaluation.passed, failed: episode.evaluation.failed, evaluation_digest: episode.evaluation.evaluation_digest } : null,
  }));
  return { episodes, projection, promptChunk: { id: "autonomous-memory", content: JSON.stringify({ schema: AUTONOMOUS_MEMORY_SCHEMA_FOR_PROMPT, episodes: summary, does_not_claim: ["memory is prior metadata, not verified truth", "memory cannot widen tools, budgets, approvals, or domain authority"] }), priority: 25 } };
}

const AUTONOMOUS_MEMORY_SCHEMA_FOR_PROMPT = "bioprism-typescript-autonomous-memory-context/0.1" as const;

function withMemoryContext(context: readonly AutonomousPromptChunk[] | undefined, memoryChunk: AutonomousPromptChunk | null): AutonomousPromptChunk[] {
  return [...(context ?? []), ...(memoryChunk ? [memoryChunk] : [])];
}

async function memoryPacketForRun(memory: AutonomousDecisionCycleMemoryOptions, run: AutonomousRunResult, episodeId: string, task: string): Promise<AutonomousMemoryEpisode | null> {
  if (!run.blueprint || !run.selection?.selected_model) return null;
  const outcomeDigest = await digestJson({ status: run.status, route_digest: run.route.route_digest, selection: run.selection, response: run.response });
  await memory.store.recordEpisode({
    episode_id: episodeId,
    run_id: episodeId,
    result_kind: "autonomous_decision_cycle",
    status: run.status === "completed" ? "completed" : run.status === "approval_required" ? "approval_required" : run.status === "child_failed" || run.status === "cross_domain_partial" ? "partial" : "failed",
    task_digest: run.blueprint.task_digest,
    task_facets: taskFacetDigests(task),
    context: { domain: run.blueprint.domain_profile.domain, capability: run.blueprint.selection_context.capability, risk_class: run.blueprint.domain_profile.risk_class, task_family: run.blueprint.selection_context.task_family ?? null },
    context_digest: run.blueprint.learning_context_digest,
    selected_model: run.selection.selected_model,
    digests: { route_digest: run.route.route_digest, prompt_digest: run.blueprint.prompt.prompt_digest, plan_digest: run.blueprint.plan.plan_digest, selection_digest: await digestJson(run.selection), outcome_digest: outcomeDigest },
    route: { route_digest: run.route.route_digest, source: run.route.source, selected_domains: [...run.route.selected_domains], primary_domain: run.route.primary_domain, confidence: run.route.confidence },
    tags: memory.tags ?? [],
    lesson: memory.lesson ?? null,
    provenance: { ...(memory.provenance ?? {}), source: "typescript-autonomous-decision-cycle" },
  });
  return memory.store.get(episodeId);
}

async function recordMemoryEvaluation(memory: AutonomousDecisionCycleMemoryOptions, episodeId: string, assessment: BrainEvaluatorAssessment): Promise<void> {
  const input: AutonomousMemoryEvaluationInput = { evaluator_id: assessment.evaluator_id, evaluator_version: assessment.evaluator_version, reward: assessment.reward, passed: assessment.passed, failed: assessment.failed, feedback_digest: assessment.feedback_digest, failure_class: assessment.failure_class, evidence_digest: assessment.evidence_digest };
  await memory.store.recordEvaluation(episodeId, input);
}

function cycleStatusForRun(status: AutonomousRunResult["status"]): AutonomousDecisionCycleStatus {
  if (status === "completed") return "completed";
  if (status === "approval_required") return "approval_required";
  if (status === "reconciliation_required") return "reconciliation_required";
  if (status === "turn_limit_reached") return "turn_limit_reached";
  return "route_review_required";
}

function executionFailureReason(error: unknown): string {
  const name = error instanceof Error ? error.name : "unknown_error";
  return /^[A-Za-z0-9_.:-]{1,128}$/.test(name) ? name : "unknown_error";
}

async function failExecutionIfActive(execution: AutonomousExecutionController | undefined, error: unknown): Promise<void> {
  if (!execution) return;
  const status = execution.state.status;
  if (["completed", "failed", "cancelled", "reconciliation_required"].includes(status) || ["completed", "failed"].includes(execution.state.last_event_kind)) return;
  await execution.fail(executionFailureReason(error));
}

function reviewResult(
  status: AutonomousDecisionCycleStatus,
  route: AutonomousRouteProposal,
  semanticRoute: AutonomousSemanticRouteResult | null,
  planRefinement: AutonomousPlanRefinementResult | null = null,
): AutonomousDecisionCycleResult {
  return {
    schema: AUTONOMOUS_DECISION_CYCLE_SCHEMA,
    status,
    route,
    semantic_route: semanticRoute,
    run: null,
    plan_refinement: planRefinement,
    learning_episode_id: null,
    evaluation: null,
    settlement: null,
    planner_evaluation: null,
    planner_settlement: null,
    memory: null,
    retention: RETENTION,
    authorization: AUTHORIZATION,
  };
}

function cycleCostBudget(options: Pick<AutonomousRunOptions, "maxTotalCostUnits" | "costBudget">): AutonomousCostBudget | undefined {
  if (options.costBudget !== undefined && !(options.costBudget instanceof AutonomousCostBudget)) throw new ArgumentError("costBudget must be an AutonomousCostBudget");
  if (options.costBudget !== undefined && options.maxTotalCostUnits !== undefined) throw new ArgumentError("costBudget and maxTotalCostUnits cannot both be supplied");
  return options.costBudget ?? (options.maxTotalCostUnits === undefined ? undefined : new AutonomousCostBudget(options.maxTotalCostUnits));
}

function cyclePlanningBudget(
  options: Pick<AutonomousRunOptions, "maxTotalCostUnits" | "costBudget"> & { providerPlanning?: AutonomousProviderPlanningOptions },
): AutonomousCostBudget | undefined {
  const runBudget = cycleCostBudget(options);
  const planning = options.providerPlanning;
  if (!planning) return runBudget;
  if (planning.costBudget !== undefined && !(planning.costBudget instanceof AutonomousCostBudget)) throw new ArgumentError("providerPlanning.costBudget must be an AutonomousCostBudget");
  if (planning.costBudget !== undefined && planning.maxTotalCostUnits !== undefined) throw new ArgumentError("providerPlanning.costBudget and providerPlanning.maxTotalCostUnits cannot both be supplied");
  if (runBudget && planning.costBudget && runBudget !== planning.costBudget) throw new ArgumentError("decision cycle planning and execution must share the same AutonomousCostBudget instance");
  if (runBudget && planning.maxTotalCostUnits !== undefined) throw new ArgumentError("decision cycle planning cannot add a second maxTotalCostUnits when execution already has a budget");
  return runBudget ?? planning.costBudget ?? (planning.maxTotalCostUnits === undefined ? undefined : new AutonomousCostBudget(planning.maxTotalCostUnits));
}

interface DecisionPersistenceRuntime {
  readonly store: AutonomousDecisionCycleStateStore;
  readonly cycleId: string;
  readonly taskDigest: string;
  readonly mode: AutonomousDecisionCycleMode;
  readonly learningEnabled: boolean;
  readonly evaluationEnabled: boolean;
  readonly trajectoryId: string | null;
  readonly restored: boolean;
  state: AutonomousDecisionCycleState;
}

type DecisionTaskMetadata = Pick<AutonomousDecisionCycleState, "task_intent_digest" | "task_decision_digest" | "task_decision_posture">;

const emptyDecisionTaskMetadata = (): DecisionTaskMetadata => ({
  task_intent_digest: null,
  task_decision_digest: null,
  task_decision_posture: null,
});

async function decisionTaskMetadataForBlueprintValue(
  blueprint: AutonomousTaskBlueprint | AutonomousCrossDomainBlueprint | null | undefined,
): Promise<DecisionTaskMetadata> {
  if (!blueprint) return emptyDecisionTaskMetadata();
  if ("task_decision" in blueprint) {
    return {
      task_intent_digest: blueprint.task_intent.intent_digest,
      task_decision_digest: blueprint.task_decision.decision_digest,
      task_decision_posture: blueprint.task_decision.posture,
    };
  }
  const entries = [
    ...blueprint.child_blueprints.map((child, index) => ({ id: blueprint.child_ids[index] ?? `child-${index + 1}`, blueprint: child })),
    { id: "synthesis", blueprint: blueprint.synthesis_blueprint },
  ].map(({ id, blueprint: child }) => ({
    id,
    task_intent_digest: child.task_intent.intent_digest,
    task_decision_digest: child.task_decision.decision_digest,
    task_decision_posture: child.task_decision.posture,
  }));
  const taskDecisionPosture: AutonomousDecisionCycleTaskDecisionPosture = entries.some((entry) => entry.task_decision_posture === "blocked")
    ? "blocked"
    : entries.some((entry) => entry.task_decision_posture === "review_required")
      ? "review_required"
      : "admitted";
  return {
    task_intent_digest: await digestJson({ cross_domain_task_intent_digests: entries.map((entry) => entry.task_intent_digest) }),
    task_decision_digest: await digestJson({ cross_domain_task_decisions: entries }),
    task_decision_posture: taskDecisionPosture,
  };
}

async function decisionTaskMetadataForAutoBlueprint(blueprint: AutonomousAutoBlueprint): Promise<DecisionTaskMetadata> {
  return decisionTaskMetadataForBlueprintValue(blueprint.blueprint ?? blueprint.cross_domain_blueprint ?? null);
}

async function decisionTaskMetadataForRun(run: AutonomousRunResult | AutonomousCrossDomainRunResult): Promise<DecisionTaskMetadata> {
  return decisionTaskMetadataForBlueprintValue(run.blueprint);
}

function boundedDecisionCycleIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

async function openDecisionPersistence(
  options: AutonomousDecisionCyclePersistenceOptions,
  task: string,
  mode: AutonomousDecisionCycleMode,
  learningEnabled: boolean,
  evaluationEnabled: boolean,
  trajectoryId: string | null,
): Promise<DecisionPersistenceRuntime | null> {
  if (!options.decisionStateStore) {
    if (options.cycleId !== undefined) throw new ArgumentError("cycleId requires decisionStateStore");
    return null;
  }
  if (options.cycleId === undefined) throw new ArgumentError("cycleId is required when decisionStateStore is configured");
  const cycleId = boundedDecisionCycleIdentifier("decision cycleId", options.cycleId);
  const taskDigest = await digestJson({ task });
  const legacyTaskDigest = await digestJson(task);
  const loadedRaw = await options.decisionStateStore.load(cycleId);
  const loaded = loadedRaw ? await validateAutonomousDecisionCycleState(loadedRaw) : null;
  if (loaded) {
    if (loaded.cycle_id !== cycleId || (loaded.task_digest !== taskDigest && loaded.task_digest !== legacyTaskDigest) || loaded.mode !== mode || loaded.learning_enabled !== learningEnabled || loaded.evaluation_enabled !== evaluationEnabled || loaded.trajectory_id !== trajectoryId) throw new ArgumentError("persisted decision-cycle state does not match the requested contract");
    return { store: options.decisionStateStore, cycleId, taskDigest, mode, learningEnabled, evaluationEnabled, trajectoryId, restored: true, state: loaded };
  }
  const state = await sealAutonomousDecisionCycleState({
    schema: AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA,
    cycle_id: cycleId,
    task_digest: taskDigest,
    task_intent_digest: null,
    task_decision_digest: null,
    task_decision_posture: null,
    mode,
    learning_enabled: learningEnabled,
    evaluation_enabled: evaluationEnabled,
    trajectory_id: trajectoryId,
    phase: "route_pending",
    route_digest: null,
    plan_refinement_digest: null,
    selection_digest: null,
    outcome_digest: null,
    evaluation_digest: null,
    learning_episode_ids: [],
    settlement_digests: [],
    terminal_status: null,
    generation: 1,
    previous_state_digest: null,
    retention: "metadata_only_hash_chained_no_private_payloads",
    secret_material: "never_returned",
  });
  await options.decisionStateStore.save(state);
  return { store: options.decisionStateStore, cycleId, taskDigest, mode, learningEnabled, evaluationEnabled, trajectoryId, restored: false, state };
}

async function commitDecisionPersistence(
  runtime: DecisionPersistenceRuntime | null,
  changes: Partial<Omit<AutonomousDecisionCycleState, "state_digest" | "generation" | "previous_state_digest">>,
): Promise<void> {
  if (!runtime) return;
  const { state_digest: priorDigest, generation: priorGeneration, previous_state_digest: _priorPrevious, ...descriptor } = runtime.state;
  const next = await sealAutonomousDecisionCycleState({
    ...descriptor,
    ...changes,
    generation: priorGeneration + 1,
    previous_state_digest: priorDigest,
  });
  await runtime.store.save(next);
  runtime.state = next;
}

function decisionRehydrationContext(runtime: DecisionPersistenceRuntime): AutonomousDecisionCycleRehydrationContext {
  const state = runtime.state;
  return {
    cycle_id: runtime.cycleId,
    task_digest: runtime.taskDigest,
    task_intent_digest: state.task_intent_digest,
    task_decision_digest: state.task_decision_digest,
    task_decision_posture: state.task_decision_posture,
    mode: runtime.mode,
    learning_enabled: runtime.learningEnabled,
    evaluation_enabled: runtime.evaluationEnabled,
    phase: state.phase,
    route_digest: state.route_digest,
    plan_refinement_digest: state.plan_refinement_digest,
    selection_digest: state.selection_digest,
    outcome_digest: state.outcome_digest,
    evaluation_digest: state.evaluation_digest,
    learning_episode_ids: [...state.learning_episode_ids],
    trajectory_id: state.trajectory_id,
    settlement_digests: [...state.settlement_digests],
    terminal_status: state.terminal_status,
    generation: state.generation,
    state_digest: state.state_digest,
  };
}

async function rehydrateDecisionRoute(
  runtime: DecisionPersistenceRuntime,
  callback: AutonomousDecisionCyclePersistenceOptions["rehydrateRoute"],
): Promise<AutonomousRouteProposal> {
  if (!callback) throw new ArgumentError("restart resume requires rehydrateRoute for the persisted decision route");
  const route = await callback(decisionRehydrationContext(runtime));
  if (!route || route.schema !== AUTONOMOUS_ROUTE_SCHEMA || route.task_digest !== runtime.taskDigest || (runtime.state.route_digest !== null && route.route_digest !== runtime.state.route_digest)) throw new ArgumentError("rehydrated decision route does not match the persisted route identity");
  return route;
}

async function rehydrateDecisionRun(
  runtime: DecisionPersistenceRuntime,
  callback: AutonomousDecisionCyclePersistenceOptions["rehydrateRun"],
): Promise<AutonomousRunResult | AutonomousCrossDomainRunResult> {
  if (!callback || (runtime.state.phase !== "execution_pending" && !runtime.state.outcome_digest)) throw new ArgumentError("restart resume requires rehydrateRun for the persisted provider outcome");
  const run = await callback(decisionRehydrationContext(runtime));
  if (!run || typeof run !== "object" || !run.route || typeof run.route.route_digest !== "string" || run.route.route_digest !== runtime.state.route_digest) throw new ArgumentError("rehydrated decision run does not match the persisted route digest");
  if (runtime.mode === "single_domain" && Array.isArray((run as AutonomousCrossDomainRunResult).child_runs)) throw new ArgumentError("single-domain decision cycle cannot rehydrate a cross-domain run");
  if (runtime.mode === "cross_domain" && !Array.isArray((run as AutonomousCrossDomainRunResult).child_runs)) throw new ArgumentError("cross-domain decision cycle requires a fan-out run during rehydration");
  if (runtime.state.task_decision_digest !== null) {
    const observedTaskDecision = await decisionTaskMetadataForRun(run);
    if (observedTaskDecision.task_intent_digest !== runtime.state.task_intent_digest || observedTaskDecision.task_decision_digest !== runtime.state.task_decision_digest || observedTaskDecision.task_decision_posture !== runtime.state.task_decision_posture) throw new ArgumentError("rehydrated decision run does not match the persisted task decision identity");
  }
  const outcome = runtime.mode === "single_domain"
    ? (await replanRunDigests(run as AutonomousRunResult)).outcome
    : await crossDomainReplanOutcomeDigest(run as AutonomousCrossDomainRunResult);
  if (runtime.state.outcome_digest !== null && outcome !== runtime.state.outcome_digest) throw new ArgumentError("rehydrated decision run does not match the persisted outcome digest");
  return run;
}

async function rehydrateDecisionResult(
  runtime: DecisionPersistenceRuntime,
  callback: AutonomousDecisionCyclePersistenceOptions["rehydrateResult"],
): Promise<AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult> {
  if (!callback || !runtime.state.terminal_status) throw new ArgumentError("restart resume requires rehydrateResult for the terminal private result");
  const result = await callback(decisionRehydrationContext(runtime));
  if (!result || typeof result !== "object" || !result.route || typeof result.route.route_digest !== "string" || result.route.route_digest !== runtime.state.route_digest || result.status !== runtime.state.terminal_status) throw new ArgumentError("rehydrated terminal decision result does not match persisted identity");
  const expectedSchema = runtime.mode === "single_domain" ? AUTONOMOUS_DECISION_CYCLE_SCHEMA : AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA;
  if (result.schema !== expectedSchema) throw new ArgumentError("rehydrated terminal decision result has the wrong cycle schema");
  if (runtime.state.task_decision_digest !== null && result.run) {
    const observedTaskDecision = await decisionTaskMetadataForRun(result.run);
    if (observedTaskDecision.task_intent_digest !== runtime.state.task_intent_digest || observedTaskDecision.task_decision_digest !== runtime.state.task_decision_digest || observedTaskDecision.task_decision_posture !== runtime.state.task_decision_posture) throw new ArgumentError("rehydrated terminal result does not match the persisted task decision identity");
  }
  if (result.run) {
    if (runtime.mode === "single_domain" && Array.isArray((result.run as AutonomousCrossDomainRunResult).child_runs)) throw new ArgumentError("single-domain terminal result cannot contain a cross-domain run");
    if (runtime.mode === "cross_domain" && !Array.isArray((result.run as AutonomousCrossDomainRunResult).child_runs)) throw new ArgumentError("cross-domain terminal result requires a fan-out run");
    const outcome = runtime.mode === "single_domain"
      ? (await replanRunDigests(result.run as AutonomousRunResult)).outcome
      : await crossDomainReplanOutcomeDigest(result.run as AutonomousCrossDomainRunResult);
    if (outcome !== runtime.state.outcome_digest) throw new ArgumentError("rehydrated terminal result does not match persisted outcome digest");
  } else if (runtime.state.outcome_digest !== await digestJson({ status: result.status, route_digest: result.route.route_digest })) {
    throw new ArgumentError("rehydrated terminal route result does not match persisted outcome digest");
  }
  return result;
}

async function decisionEvaluationDigest(input: AutonomousEvaluatorRewardInput): Promise<string> {
  return digestJson({
    evaluator_id: input.evaluator_id,
    evaluator_version: input.evaluator_version,
    reward: input.reward,
    passed: input.passed,
    failed: input.failed ?? !input.passed,
    feedback_digest: input.feedback_digest ?? null,
    failure_class: input.failure_class ?? null,
    evidence_digest: input.evidence_digest ?? null,
  });
}

async function decisionCrossEvaluationDigest(input: Record<string, AutonomousEvaluatorRewardInput>): Promise<string> {
  return digestJson(Object.entries(input).sort(([left], [right]) => left.localeCompare(right)).map(([episodeId, reward]) => ({
    episode_id: episodeId,
    evaluator_id: reward.evaluator_id,
    evaluator_version: reward.evaluator_version,
    reward: reward.reward,
    passed: reward.passed,
    failed: reward.failed ?? !reward.passed,
    feedback_digest: reward.feedback_digest ?? null,
    failure_class: reward.failure_class ?? null,
    evidence_digest: reward.evidence_digest ?? null,
  })));
}

/** Preserve the historical execution-only digest while binding planner and execution evidence together when both exist. */
async function decisionCycleEvaluationDigest(
  execution: AutonomousEvaluatorRewardInput | null,
  planning: AutonomousEvaluatorRewardInput | null,
): Promise<string | null> {
  if (!execution && !planning) return null;
  const executionDigest = execution ? await decisionEvaluationDigest(execution) : null;
  if (!planning) return executionDigest;
  return digestJson({ execution_digest: executionDigest, planning_digest: await decisionEvaluationDigest(planning) });
}

function projectCycleReward(input: AutonomousEvaluatorRewardInput): AutonomousEvaluatorRewardInput {
  return {
    evaluator_id: input.evaluator_id,
    evaluator_version: input.evaluator_version,
    reward: input.reward,
    passed: input.passed,
    failed: input.failed ?? !input.passed,
    feedback_digest: input.feedback_digest ?? null,
    failure_class: input.failure_class ?? null,
    evidence_digest: input.evidence_digest ?? null,
  };
}

interface CyclePersistenceOptions {
  cycleId?: string;
  stateStore?: AutonomousCycleReplanStateStore;
}

interface CyclePersistenceRuntime {
  readonly store: AutonomousCycleReplanStateStore;
  readonly cycleId: string;
  readonly taskDigest: string;
  readonly mode: AutonomousCycleReplanMode;
  state: AutonomousCycleReplanState;
}

async function openCyclePersistence(
  options: CyclePersistenceOptions,
  task: string,
  mode: AutonomousCycleReplanMode,
  maxReplans: number,
): Promise<CyclePersistenceRuntime | null> {
  if (!options.stateStore) {
    if (options.cycleId !== undefined) throw new ArgumentError("cycleId requires stateStore");
    return null;
  }
  if (options.cycleId === undefined) throw new ArgumentError("cycleId is required when stateStore is configured");
  const cycleId = boundedReplanIdentifier("cycleId", options.cycleId);
  const taskDigest = await digestJson(task);
  const loaded = await options.stateStore.load(cycleId);
  if (loaded) {
    const state = await validateAutonomousCycleReplanState(loaded);
    if (state.cycle_id !== cycleId || state.task_digest !== taskDigest || state.mode !== mode || state.max_replans !== maxReplans) throw new ArgumentError("persisted autonomous cycle state does not match the requested cycle contract");
    return { store: options.stateStore, cycleId, taskDigest, mode, state };
  }
  const state = await sealAutonomousCycleReplanState({
    schema: AUTONOMOUS_CYCLE_REPLAN_STATE_SCHEMA,
    cycle_id: cycleId,
    task_digest: taskDigest,
    mode,
    max_replans: maxReplans,
    attempt: 1,
    phase: "execution_pending",
    route_digest: null,
    plan_refinement_digest: null,
    outcome_digest: null,
    evaluation_digest: null,
    replan_instruction_digest: null,
    terminal_status: null,
    attempts: [],
    evaluations: [],
    learning_episode_ids: [],
    response_learning_episode_ids: [],
    settlement_digests: [],
    trajectory_ids: [],
    context_digests: [],
    generation: 1,
    previous_state_digest: null,
    retention: "metadata_only_hash_chained_no_private_payloads",
    secret_material: "never_returned",
  });
  await options.stateStore.save(state);
  return { store: options.stateStore, cycleId, taskDigest, mode, state };
}

async function commitCyclePersistence(
  runtime: CyclePersistenceRuntime | null,
  changes: Partial<Omit<AutonomousCycleReplanState, "state_digest" | "generation" | "previous_state_digest">>,
): Promise<void> {
  if (!runtime) return;
  const { state_digest: priorDigest, generation: priorGeneration, previous_state_digest: _priorPrevious, ...descriptor } = runtime.state;
  const next = await sealAutonomousCycleReplanState({
    ...descriptor,
    ...changes,
    generation: priorGeneration + 1,
    previous_state_digest: priorDigest,
  });
  await runtime.store.save(next);
  runtime.state = next;
}

function cycleAttemptState(
  attempt: number,
  status: string,
  runStatus: string | null,
  routeDigest: string | null,
  selectionDigest: string | null,
  outcomeDigest: string | null,
  evaluationDigest: string | null,
  learningEpisodeIds: readonly string[],
  trajectoryId: string | null,
  planRefinementDigest: string | null = null,
): AutonomousCycleReplanState["attempts"][number] {
  return { attempt, status, run_status: runStatus, route_digest: routeDigest, plan_refinement_digest: planRefinementDigest, selection_digest: selectionDigest, outcome_digest: outcomeDigest, evaluation_digest: evaluationDigest, learning_episode_ids: [...learningEpisodeIds], trajectory_id: trajectoryId };
}

function upsertCycleAttempt(attempts: AutonomousCycleReplanState["attempts"], next: AutonomousCycleReplanState["attempts"][number]): AutonomousCycleReplanState["attempts"] {
  const remaining = attempts.filter((item) => item.attempt !== next.attempt);
  return [...remaining, next].sort((left, right) => left.attempt - right.attempt);
}

function upsertResultAttempt<T extends { attempt: number }>(attempts: T[], next: T): void {
  const index = attempts.findIndex((item) => item.attempt === next.attempt);
  if (index < 0) attempts.push(next);
  else attempts[index] = next;
  attempts.sort((left, right) => left.attempt - right.attempt);
}

function replanContextDigest(context: AutonomousPromptChunk): Promise<string> {
  return digestJson({ id: context.id, content: context.content, priority: context.priority ?? null, required: context.required ?? false });
}

function rehydrationContext(runtime: CyclePersistenceRuntime): {
  cycle_id: string;
  task_digest: string;
  mode: AutonomousCycleReplanMode;
  attempt: number;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  replan_instruction_digest: string | null;
} {
  const state = runtime.state;
  return { cycle_id: runtime.cycleId, task_digest: runtime.taskDigest, mode: runtime.mode, attempt: state.attempt, route_digest: state.route_digest, plan_refinement_digest: state.plan_refinement_digest, outcome_digest: state.outcome_digest, evaluation_digest: state.evaluation_digest, replan_instruction_digest: state.replan_instruction_digest };
}

async function rehydrateCycleRoute(
  runtime: CyclePersistenceRuntime,
  rehydrateRoute: AutonomousCycleReplanRouteRehydrator<AutonomousRouteProposal> | undefined,
): Promise<AutonomousRouteProposal> {
  if (!rehydrateRoute) throw new ArgumentError("restart resume requires rehydrateRoute for the persisted reviewed route");
  const route = await rehydrateRoute(rehydrationContext(runtime));
  if (!route || typeof route.route_digest !== "string" || route.route_digest !== runtime.state.route_digest) throw new ArgumentError("rehydrated autonomous route does not match the persisted route digest");
  return route;
}

async function rehydrateCycleInstruction(
  runtime: CyclePersistenceRuntime,
  rehydrateInstruction: AutonomousCycleReplanInstructionRehydrator | undefined,
): Promise<string> {
  if (!rehydrateInstruction || !runtime.state.replan_instruction_digest) throw new ArgumentError("restart resume requires rehydrateReplanInstruction for the transient evaluator handoff");
  const instruction = await rehydrateInstruction(rehydrationContext(runtime));
  if (typeof instruction !== "string" || await digestJson(instruction.trim()) !== runtime.state.replan_instruction_digest) throw new ArgumentError("rehydrated evaluator instruction does not match its persisted digest");
  return instruction.trim();
}

function persistedSingleAttempts(state: AutonomousCycleReplanState): AutonomousReplanAttempt[] {
  return state.attempts.map((attempt) => ({ attempt: attempt.attempt, status: attempt.status as AutonomousDecisionCycleStatus, run_status: attempt.run_status as AutonomousRunResult["status"] | null, route_digest: attempt.route_digest, plan_refinement_digest: attempt.plan_refinement_digest, selection_digest: attempt.selection_digest, outcome_digest: attempt.outcome_digest, evaluation_digest: attempt.evaluation_digest, evaluation: state.evaluations[attempt.attempt - 1] as unknown as AutonomousReplanEvaluationProjection ?? null, learning_episode_id: attempt.learning_episode_ids[0] ?? null }));
}

function persistedCrossAttempts(state: AutonomousCycleReplanState): AutonomousCrossDomainReplanAttempt[] {
  return state.attempts.map((attempt) => ({ attempt: attempt.attempt, status: attempt.status as AutonomousCrossDomainDecisionCycleStatus, run_status: attempt.run_status as AutonomousCrossDomainRunResult["status"] | null, route_digest: attempt.route_digest, plan_refinement_digest: attempt.plan_refinement_digest, outcome_digest: attempt.outcome_digest, evaluation_digest: attempt.evaluation_digest, evaluation: state.evaluations[attempt.attempt - 1] as unknown as AutonomousCrossDomainReplanEvaluationProjection ?? null, learning_episode_ids: [...attempt.learning_episode_ids], trajectory_id: attempt.trajectory_id }));
}

function rehydratedSingleCycle(route: AutonomousRouteProposal, run: AutonomousRunResult): AutonomousDecisionCycleResult {
  return {
    schema: AUTONOMOUS_DECISION_CYCLE_SCHEMA,
    status: cycleStatusForRun(run.status),
    route,
    semantic_route: null,
    run,
    plan_refinement: null,
    learning_episode_id: null,
    evaluation: null,
    settlement: null,
    planner_evaluation: null,
    planner_settlement: null,
    memory: null,
    retention: RETENTION,
    authorization: AUTHORIZATION,
  };
}

function rehydratedCrossDomainCycle(route: AutonomousRouteProposal, run: AutonomousCrossDomainRunResult): AutonomousCrossDomainDecisionCycleResult {
  return {
    schema: AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA,
    status: run.status,
    route,
    semantic_route: null,
    run,
    plan_refinement: null,
    learning_episode_ids: [...run.learning_episode_ids],
    response_learning_episode_ids: [...(run.response_learning_episode_ids ?? [])],
    evaluation: null,
    settlement: null,
    response_settlements: [],
    planner_evaluation: null,
    planner_settlement: null,
    memory: null,
    retention: CROSS_RETENTION,
    authorization: CROSS_AUTHORIZATION,
  };
}

function runOptions(options: AutonomousDecisionCycleOptions, route: AutonomousRouteProposal, memoryChunk: AutonomousPromptChunk | null, costBudget?: AutonomousCostBudget): AutonomousRunOptions {
  return {
    domain: options.domain,
    routeOverride: route,
    capability: options.capability,
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context: withMemoryContext(options.context, memoryChunk),
    memoryRecall: options.memoryRecall,
    hints: options.hints,
    allowCrossDomain: options.allowCrossDomain,
    maxInputTokens: options.maxInputTokens,
    maxOutputTokens: options.maxOutputTokens,
    maxCostPerMillionTokens: options.maxCostPerMillionTokens,
    maxLatencyMs: options.maxLatencyMs,
    minQuality: options.minQuality,
    maxTotalCostUnits: costBudget ? undefined : options.maxTotalCostUnits,
    costBudget,
    requireJson: options.requireJson,
    responseSchema: options.responseSchema,
    structuredDomainResponse: options.structuredDomainResponse,
    temperature: options.temperature,
    tools: options.tools,
    authorizeAndExecute: options.authorizeAndExecute,
    toolReadOnly: options.toolReadOnly,
    approveProviderCall: options.approveProviderCall,
    approveEffects: options.approveEffects,
    execution: options.execution,
    executionAttempt: options.executionAttempt,
    maxProviderFailovers: options.maxProviderFailovers,
    executionLifecycle: options.executionLifecycle,
    signal: options.signal,
    observer: options.observer,
    acceptedSingleDomainPlanRefinement: options.acceptedSingleDomainPlanRefinement,
  };
}

/**
 * Execute one bounded autonomous decision cycle for a single reviewed domain.
 *
 * The cycle is deliberately explicit: optional semantic routing, deterministic route handoff,
 * prompt/plan/model/provider execution, and caller-owned evaluator settlement are separate
 * phases. A provider response never becomes reward without the supplied evaluator callback.
 */
export async function runAutonomousDecisionCycle(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousDecisionCycleOptions = {},
): Promise<AutonomousDecisionCycleResult> {
  if (!agent || typeof agent.run !== "function" || typeof agent.route !== "function") throw new ArgumentError("decision cycle requires an AutonomousAgent");
  if (options.semanticRouting?.enabled && options.domain !== undefined) throw new ArgumentError("semantic decision routing cannot replace an explicit caller domain");
  const costBudget = cyclePlanningBudget(options);
  const persistence = await openDecisionPersistence(options, task, "single_domain", options.learning !== undefined, Boolean(options.learning?.evaluate || options.learning?.evaluatePlanning), null);
  if (persistence?.state.phase === "terminal") {
    return await rehydrateDecisionResult(persistence, options.rehydrateResult) as AutonomousDecisionCycleResult;
  }
  const persistedRoute = persistence?.restored === true && persistence.state.route_digest !== null;
  const persistedPhase = persistence?.restored === true && (persistence.state.phase !== "route_pending" || persistedRoute) ? persistence.state.phase : null;

  let route: AutonomousRouteProposal;
  let semanticRoute: AutonomousSemanticRouteResult | null = null;
  let rehydratedRun: AutonomousRunResult | null = null;
  if (persistedPhase) {
    route = await rehydrateDecisionRoute(persistence!, options.rehydrateRoute);
    if (persistedPhase === "execution_pending" || persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending") {
      rehydratedRun = await rehydrateDecisionRun(persistence!, options.rehydrateRun) as AutonomousRunResult;
    }
  } else if (persistence?.restored === true && options.rehydrateRoute) {
    route = await rehydrateDecisionRoute(persistence, options.rehydrateRoute);
  } else if (persistence?.restored === true && options.semanticRouting?.enabled && options.retrySemanticRoutingOnRestart !== true) {
    throw new ArgumentError("restart resume of provider-assisted semantic routing requires rehydrateRoute or retrySemanticRoutingOnRestart: true");
  } else if (options.semanticRouting?.enabled) {
    semanticRoute = await semanticRouteAutonomousTask(agent, task, {
      candidates: options.candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      hints: options.hints,
      approveProviderCall: options.semanticRouting.approveProviderCall,
      minSemanticConfidence: options.semanticRouting.minSemanticConfidence,
      maxDomains: options.semanticRouting.maxDomains,
      allowCrossDomain: options.semanticRouting.allowCrossDomain,
      maxOutputTokens: options.semanticRouting.maxOutputTokens,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      maxTotalCostUnits: undefined,
      costBudget,
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers,
      executionLifecycle: options.executionLifecycle,
      domainPolicyMode: options.semanticRouting.domainPolicyMode ?? options.domainPolicyMode,
      domainPolicyEvidenceReady: options.semanticRouting.domainPolicyEvidenceReady ?? options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: options.semanticRouting.domainPolicyEvaluatorConfigured ?? options.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: options.semanticRouting.domainPolicyEffectsRequested ?? options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: options.semanticRouting.domainPolicyEffectsApproved ?? options.domainPolicyEffectsApproved,
      signal: options.signal,
      observer: options.observer,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: semanticRoute.status, reason: `semantic_route_${semanticRoute.status}` });
      const reviewed = reviewResult(semanticRoute.status === "approval_required" ? "approval_required" : semanticRoute.status, route, semanticRoute);
      await commitDecisionPersistence(persistence, { phase: "terminal", route_digest: route.route_digest, selection_digest: null, outcome_digest: await digestJson({ status: reviewed.status, route_digest: route.route_digest }), evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: reviewed.status });
      return reviewed;
    }
  } else if (options.routeOverride) {
    route = options.routeOverride;
  } else {
    route = await agent.route(task, { domain: options.domain, hints: options.hints, allowCrossDomain: options.allowCrossDomain });
  }

  const decisionBlueprint = await agent.blueprint(task, {
    domain: route.primary_domain ?? undefined,
    routeOverride: route,
    capability: options.capability,
    context: options.context,
    maxInputTokens: options.maxInputTokens,
    tools: options.tools?.map((tool) => tool.name),
    hints: options.hints,
    structuredDomainResponse: options.structuredDomainResponse,
  });
  const taskDecisionMetadata = await decisionTaskMetadataForAutoBlueprint(decisionBlueprint);
  if (persistence && persistence.state.route_digest === null) await commitDecisionPersistence(persistence, { phase: "route_pending", route_digest: route.route_digest, ...taskDecisionMetadata, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: null });

  if (route.abstained || !route.primary_domain || route.cross_domain || route.selected_domains.length !== 1) {
    if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: "route_review_required", reason: "single_domain_route_review_required" });
    const reviewed = reviewResult("route_review_required", route, semanticRoute);
    await commitDecisionPersistence(persistence, { phase: "terminal", route_digest: route.route_digest, selection_digest: null, outcome_digest: await digestJson({ status: reviewed.status, route_digest: route.route_digest }), evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: reviewed.status });
    return reviewed;
  }
  const recalledMemory = await recallMemory(options.memory, route, task, options.memoryRecall);
  let planRefinement: AutonomousPlanRefinementResult | null = null;
  const persistedPlanRefinementDigest = persistence?.state.plan_refinement_digest ?? null;
  if (options.providerPlanning || options.acceptedSingleDomainPlanRefinement !== undefined || persistedPlanRefinementDigest !== null) {
    const blueprintEnvelope = await agent.blueprint(task, {
      domain: route.primary_domain,
      routeOverride: route,
      capability: options.capability,
      context: withMemoryContext(options.context, recalledMemory.promptChunk),
      hints: options.hints,
      maxInputTokens: options.maxInputTokens,
      tools: options.tools?.map((tool) => tool.name),
    });
    if (!blueprintEnvelope.blueprint || blueprintEnvelope.cross_domain_blueprint) throw new ArgumentError("single-domain decision planning requires a single-domain blueprint");
    const accepted = await acceptedAutonomousPlan(blueprintEnvelope.blueprint, options.acceptedSingleDomainPlanRefinement);
    if (persistedPlanRefinementDigest !== null && !accepted) throw new ArgumentError("restart resume requires the caller to rehydrate the accepted single-domain plan refinement");
    if (accepted && persistedPlanRefinementDigest !== null && accepted.refinement_digest !== persistedPlanRefinementDigest) throw new ArgumentError("accepted single-domain plan does not match the persisted planning digest");
    planRefinement = options.acceptedSingleDomainPlanRefinement ?? null;
    if (!accepted && options.providerPlanning) {
      await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: null, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: null });
      const proposal = await agent.planWithProvider(blueprintEnvelope.blueprint, {
        ...options.providerPlanning,
        context: withMemoryContext(options.providerPlanning.context, recalledMemory.promptChunk),
        costBudget,
        maxTotalCostUnits: undefined,
        execution: options.providerPlanning.execution ?? options.execution,
        executionAttempt: options.providerPlanning.executionAttempt ?? options.executionAttempt,
        signal: options.providerPlanning.signal ?? options.signal,
      });
      if (proposal.status !== "completed") {
        const status: AutonomousDecisionCycleStatus = proposal.status === "approval_required" ? "approval_required" : proposal.status === "provider_invalid" ? "provider_invalid" : "provider_disagreement";
        const reviewed = reviewResult(status, route, semanticRoute, proposal);
        await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: null, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: null });
        return reviewed;
      }
      const proposalDigest = await digestJson(proposal);
      if (proposal.review_required || options.acceptPlan !== true) {
        if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: "plan_review_required", reason: "provider_plan_review_required" });
        const reviewed = reviewResult("plan_review_required", route, semanticRoute, proposal);
        await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: proposalDigest, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: null });
        return reviewed;
      }
      planRefinement = proposal;
      await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: proposalDigest, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: null });
    }
  }
  let run: AutonomousRunResult;
  try {
    if (rehydratedRun) {
      run = rehydratedRun;
    } else {
      await commitDecisionPersistence(persistence, { phase: "execution_pending", route_digest: route.route_digest, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: null });
      run = await agent.run(task, { ...runOptions(options, route, recalledMemory.promptChunk, costBudget), acceptedSingleDomainPlanRefinement: planRefinement ?? undefined });
    }
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
  const cycleStatus = cycleStatusForRun(run.status);
  const runDigests = await replanRunDigests(run);
  if (cycleStatus !== "completed") {
    if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: cycleStatus, reason: `run_${cycleStatus}` });
    await commitDecisionPersistence(persistence, { phase: "terminal", route_digest: route.route_digest, selection_digest: runDigests.selection, outcome_digest: runDigests.outcome, evaluation_digest: null, learning_episode_ids: [], settlement_digests: [], terminal_status: cycleStatus });
    return { ...reviewResult(cycleStatus, route, semanticRoute, planRefinement), run, memory: recalledMemory.projection };
  }

  try {
    let learningEpisodeId: string | null = null;
    let settlement: AutonomousLearningSettlement | null = null;
    let plannerEvaluation: AutonomousEvaluatorRewardInput | null = null;
    let plannerSettlement: AutonomousPlanningQualitySettlement | null = null;
    let executionEvaluation: AutonomousEvaluatorRewardInput | null = null;
    if (persistence && (persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending")) {
      if (persistence.state.outcome_digest !== runDigests.outcome || persistence.state.selection_digest !== runDigests.selection) throw new ArgumentError("rehydrated decision run does not match the persisted selection or outcome digest");
    }
    const executionEvaluationEnabled = Boolean(options.learning?.evaluate);
    const planningEvaluationEnabled = Boolean(options.learning?.evaluatePlanning && planRefinement?.status === "completed");
    const cycleEvaluationEnabled = executionEvaluationEnabled || planningEvaluationEnabled;
    if (options.learning && cycleEvaluationEnabled) {
      const controller = options.learning.controller;
      if (!controller) throw new ArgumentError("decision cycle learning controller is malformed");
      if (executionEvaluationEnabled) {
        if (typeof controller.prepareRun !== "function" || typeof controller.settleRun !== "function") throw new ArgumentError("decision cycle execution learning controller is malformed");
        learningEpisodeId = persistence?.state.learning_episode_ids[0] ?? null;
        if (!learningEpisodeId) {
          const episode = await controller.prepareRun(run, { episodeId: options.learning.episodeId });
          learningEpisodeId = episode.episode_id;
        }
      }
      const resumedEvaluation = persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending";
      const persistedEvaluationDigest = persistence?.state.evaluation_digest ?? null;
      if (persistence && !resumedEvaluation) await commitDecisionPersistence(persistence, { phase: "evaluation_pending", route_digest: route.route_digest, selection_digest: runDigests.selection, outcome_digest: runDigests.outcome, evaluation_digest: null, learning_episode_ids: learningEpisodeId ? [learningEpisodeId] : [], settlement_digests: [], terminal_status: null });
      if (executionEvaluationEnabled) {
        const reward = resumedEvaluation
          ? await (options.rehydrateEvaluation ? options.rehydrateEvaluation(decisionRehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydrateEvaluation for the persisted evaluator boundary"))) as AutonomousEvaluatorRewardInput
          : await options.learning.evaluate!(run);
        executionEvaluation = projectCycleReward(reward);
      }
      if (planningEvaluationEnabled) {
        const reward = resumedEvaluation
          ? await (options.rehydratePlanningEvaluation ? options.rehydratePlanningEvaluation(decisionRehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydratePlanningEvaluation for the persisted planner evaluator boundary")))
          : await options.learning.evaluatePlanning!(planRefinement!);
        plannerEvaluation = projectCycleReward(reward);
      }
      const evaluationDigest = await decisionCycleEvaluationDigest(executionEvaluation, plannerEvaluation);
      if (persistedPhase === "settlement_pending" && persistedEvaluationDigest !== evaluationDigest) throw new ArgumentError("rehydrated decision evaluation does not match the persisted evaluation digest");
      await commitDecisionPersistence(persistence, { phase: "settlement_pending", route_digest: route.route_digest, selection_digest: runDigests.selection, outcome_digest: runDigests.outcome, evaluation_digest: evaluationDigest, learning_episode_ids: learningEpisodeId ? [learningEpisodeId] : [], settlement_digests: [], terminal_status: null });
      if (executionEvaluation && learningEpisodeId) {
        settlement = await controller.settleRun(learningEpisodeId, executionEvaluation, { remote: options.learning.remote, outbox: options.learning.outbox, ...(persistence ? { idempotencyKey: `decision:${persistence.cycleId}:${learningEpisodeId}` } : {}) });
      }
      if (plannerEvaluation && planRefinement) {
        plannerSettlement = await controller.settlePlanningQuality(planRefinement, {
          domain: route.primary_domain ?? "cross_domain",
          capability: options.capability,
          riskClass: run.blueprint?.domain_profile.risk_class ?? "planning_review",
          taskFamily: run.blueprint?.selection_context.task_family ?? null,
          evaluator: plannerEvaluation,
          remote: options.learning.remote,
        });
      }
    }

    const memoryProjection = recalledMemory.projection;
    if (options.memory) {
      const memoryEpisodeId = options.memory.episodeId ?? `memory:${learningEpisodeId ?? `${run.blueprint!.task_digest}:${run.blueprint!.prompt.prompt_digest}`}`;
      const memoryEpisode = await memoryPacketForRun(options.memory, run, memoryEpisodeId, task);
      if (memoryEpisode) {
        memoryProjection.recorded_episode_ids.push(memoryEpisode.episode_id);
        if (settlement) {
          await recordMemoryEvaluation(options.memory, memoryEpisode.episode_id, settlement.assessment);
          memoryProjection.evaluation_recorded_episode_ids.push(memoryEpisode.episode_id);
        }
      }
    }

    const settlementDigests = [
      settlement?.episode.settlement?.settlement_digest ?? null,
      plannerSettlement?.status === "settled" ? await digestJson(plannerSettlement) : null,
    ].filter((digest): digest is string => digest !== null);
    await commitDecisionPersistence(persistence, {
      phase: "terminal",
      route_digest: route.route_digest,
      selection_digest: runDigests.selection,
      outcome_digest: runDigests.outcome,
      evaluation_digest: await decisionCycleEvaluationDigest(executionEvaluation, plannerEvaluation),
      learning_episode_ids: learningEpisodeId ? [learningEpisodeId] : [],
      settlement_digests: settlementDigests,
      terminal_status: "completed",
    });
    if (options.executionLifecycle !== "observe_only") await options.execution?.complete("completed");

    return {
      schema: AUTONOMOUS_DECISION_CYCLE_SCHEMA,
      status: "completed",
      route,
      semantic_route: semanticRoute,
      run,
      plan_refinement: planRefinement,
      learning_episode_id: learningEpisodeId,
      evaluation: settlement?.assessment ?? null,
      settlement,
      planner_evaluation: plannerEvaluation,
      planner_settlement: plannerSettlement,
      memory: memoryProjection,
      retention: RETENTION,
      authorization: AUTHORIZATION,
    };
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
}

export const AUTONOMOUS_REPLAN_CYCLE_SCHEMA = "bioprism-typescript-autonomous-replan-cycle/0.1" as const;
export const AUTONOMOUS_REPLAN_CONTEXT_SCHEMA = "bioprism-typescript-autonomous-replan-context/0.1" as const;
export const AUTONOMOUS_REPLAN_MAX_REPLANS = AUTONOMOUS_CYCLE_REPLAN_MAX_REPLANS;

export type AutonomousReplanCycleStatus =
  | AutonomousDecisionCycleStatus
  | "completed_without_replan"
  | "replan_limit_reached";

export interface AutonomousReplanEvaluation extends AutonomousEvaluatorRewardInput {
  replan_requested: boolean;
  replan_instruction?: string | null;
}

export type AutonomousReplanEvaluator = (
  result: AutonomousRunResult,
) => AutonomousReplanEvaluation | Promise<AutonomousReplanEvaluation>;

export interface AutonomousReplanLearningOptions {
  controller: AutonomousLearningController;
  /** Prefix must be unique for the caller's logical cycle when learning is enabled. */
  episodePrefix?: string;
  remote?: boolean;
  outbox?: AutonomousLearningOutboxSettlementOptions;
}

export interface AutonomousReplanPlanningEvaluationProjection extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string | null;
}

export interface AutonomousReplanEvaluationProjection extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string | null;
  replan_requested: boolean;
  replan_instruction_digest: string | null;
}

export interface AutonomousReplanAttempt extends JsonObject {
  attempt: number;
  status: AutonomousDecisionCycleStatus;
  run_status: AutonomousRunResult["status"] | null;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  selection_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evaluation: AutonomousReplanEvaluationProjection | null;
  learning_episode_id: string | null;
}

export interface AutonomousReplanCycleOptions extends Omit<AutonomousDecisionCycleOptions, "learning" | "memory" | "cycleId" | "decisionStateStore" | "rehydrateRoute" | "rehydrateRun" | "rehydrateEvaluation" | "rehydratePlanningEvaluation" | "rehydrateResult"> {
  evaluate: AutonomousReplanEvaluator;
  /** Explicit evaluator for each accepted provider planning proposal. */
  evaluatePlanning?: AutonomousDecisionCyclePlanningEvaluator;
  /** Additional evaluator-requested attempts. The SDK caps this at three. */
  maxReplans?: number;
  learning?: AutonomousReplanLearningOptions;
  /** Stable caller-owned identity used to resume this logical cycle after a process restart. */
  cycleId?: string;
  /** Optional metadata-only state store. Private task/run/evaluator material remains caller-owned. */
  stateStore?: AutonomousCycleReplanStateStore;
  /** Rehydrate a private run when a restart happened after provider execution. */
  rehydrateRun?: AutonomousCycleReplanRunRehydrator<AutonomousRunResult>;
  /** Rehydrate a private route when resuming a persisted replan handoff. */
  rehydrateRoute?: AutonomousCycleReplanRouteRehydrator<AutonomousRouteProposal>;
  /** Rehydrate the private evaluator packet when settlement was interrupted. */
  rehydrateEvaluation?: AutonomousCycleReplanEvaluationRehydrator;
  /** Rehydrate planner quality values after a settlement boundary. */
  rehydratePlanningEvaluation?: (context: AutonomousCycleReplanRehydrationContext) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;
  /** Rehydrate transient evaluator guidance from caller-owned storage. */
  rehydrateReplanInstruction?: AutonomousCycleReplanInstructionRehydrator;
}

export interface AutonomousReplanCycleResult {
  schema: typeof AUTONOMOUS_REPLAN_CYCLE_SCHEMA;
  status: AutonomousReplanCycleStatus;
  final: AutonomousDecisionCycleResult | null;
  attempts: AutonomousReplanAttempt[];
  replan_count: number;
  evaluations: AutonomousReplanEvaluationProjection[];
  planner_evaluations: AutonomousReplanPlanningEvaluationProjection[];
  learning_episode_ids: string[];
  settlements: AutonomousLearningSettlement[];
  planner_settlements: AutonomousPlanningQualitySettlement[];
  retention: "provider_response_local; replan_instructions_transient; value_only_evaluation_and_learning_projection";
  authorization: "routing_and_provider_invocation_require_separate_explicit_approval";
}

const REPLAN_RETENTION = "provider_response_local; replan_instructions_transient; value_only_evaluation_and_learning_projection" as const;

function boundedReplanCount(value: unknown): number {
  const count = value ?? 1;
  if (!Number.isSafeInteger(count) || (count as number) < 0 || (count as number) > AUTONOMOUS_REPLAN_MAX_REPLANS) throw new ArgumentError(`maxReplans must be an integer within [0, ${AUTONOMOUS_REPLAN_MAX_REPLANS}]`);
  return count as number;
}

function boundedReplanIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function boundedReplanDigest(name: string, value: unknown, allowNull = false): string | null {
  if (value === undefined && allowNull) return null;
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedReplanReward(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError("replan evaluator reward must be within [0, 1]");
  return value;
}

function boundedReplanInstruction(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string" || !value.trim() || value.length > 8_000) throw new ArgumentError("replan instruction must be a non-empty string of at most 8,000 characters");
  if (/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/.test(value)) throw new ArgumentError("replan instruction contains control characters");
  if (/(?:api[-_ ]?key|authorization|bearer|password|secret|private[-_ ]?key|credential|refresh[-_ ]?token|gsk_|\bsk-[A-Za-z0-9])/i.test(value)) throw new ArgumentError("replan instruction appears to contain credential material");
  return value.trim();
}

function normalizeReplanEvaluation(value: unknown): AutonomousReplanEvaluation {
  if (!isObject(value)) throw new ArgumentError("replan evaluator must return an object");
  const evaluatorId = boundedReplanIdentifier("replan evaluator_id", value.evaluator_id);
  const evaluatorVersion = boundedReplanIdentifier("replan evaluator_version", value.evaluator_version);
  const reward = boundedReplanReward(value.reward);
  if (typeof value.passed !== "boolean") throw new ArgumentError("replan evaluator passed must be boolean");
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new ArgumentError("replan evaluator failed must be boolean");
  if (typeof value.replan_requested !== "boolean") throw new ArgumentError("replan evaluator replan_requested must be boolean");
  const instruction = boundedReplanInstruction(value.replan_instruction);
  if (value.replan_requested && !instruction) throw new ArgumentError("replan evaluator must provide a bounded instruction when replan_requested is true");
  if (!value.replan_requested && instruction) throw new ArgumentError("replan evaluator supplied an instruction without requesting a replan");
  const feedbackDigest = boundedReplanDigest("replan evaluator feedback_digest", value.feedback_digest, true);
  const evidenceDigest = boundedReplanDigest("replan evaluator evidence_digest", value.evidence_digest, true);
  let failureClass: string | null = null;
  if (value.failure_class !== undefined && value.failure_class !== null) failureClass = boundedReplanIdentifier("replan evaluator failure_class", value.failure_class);
  return {
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward,
    passed: value.passed,
    failed: value.failed,
    feedback_digest: feedbackDigest,
    failure_class: failureClass,
    evidence_digest: evidenceDigest,
    replan_requested: value.replan_requested,
    replan_instruction: instruction,
  };
}

async function replanEvaluationProjection(value: AutonomousReplanEvaluation): Promise<AutonomousReplanEvaluationProjection> {
  const instructionDigest = value.replan_instruction ? await digestJson(value.replan_instruction) : null;
  return {
    evaluator_id: value.evaluator_id,
    evaluator_version: value.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed ?? !value.passed,
    feedback_digest: value.feedback_digest ?? null,
    failure_class: value.failure_class ?? null,
    evidence_digest: value.evidence_digest ?? null,
    replan_requested: value.replan_requested,
    replan_instruction_digest: instructionDigest,
  };
}

async function replanRunDigests(run: AutonomousRunResult | null): Promise<{ selection: string | null; outcome: string | null }> {
  if (!run) return { selection: null, outcome: null };
  const selection = run.selection ? await digestJson(run.selection) : null;
  const outcome = await digestJson({ status: run.status, route_digest: run.route.route_digest, selection: run.selection, response: run.response });
  return { selection, outcome };
}

async function cyclePlanRefinementDigest(cycle: AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult): Promise<string | null> {
  if (cycle.plan_refinement) return digestJson(cycle.plan_refinement);
  return cycle.run?.plan_refinement_digest ?? null;
}

async function replanContextChunk(
  attempt: number,
  routeDigest: string,
  selectionDigest: string | null,
  outcomeDigest: string | null,
  evaluation: AutonomousReplanEvaluation,
): Promise<AutonomousPromptChunk> {
  const instruction = evaluation.replan_instruction;
  if (!instruction) throw new ArgumentError("replan context requires an instruction");
  const content = JSON.stringify({
    schema: AUTONOMOUS_REPLAN_CONTEXT_SCHEMA,
    attempt,
    prior: { route_digest: routeDigest, selection_digest: selectionDigest, outcome_digest: outcomeDigest },
    evaluator: {
      evaluator_id: evaluation.evaluator_id,
      evaluator_version: evaluation.evaluator_version,
      reward: evaluation.reward,
      passed: evaluation.passed,
      failed: evaluation.failed ?? !evaluation.passed,
      feedback_digest: evaluation.feedback_digest ?? null,
      failure_class: evaluation.failure_class ?? null,
      evidence_digest: evaluation.evidence_digest ?? null,
    },
    instruction,
    guardrails: [
      "This is bounded evaluator feedback, not a new authorization.",
      "Preserve the reviewed domain, model capability requirements, tool allow-list, budgets, and approval gates.",
      "Do not treat the prior provider response as verified truth or claim an external effect occurred.",
    ],
  });
  return { id: `autonomous-replan-${attempt}`, content, required: true, priority: 95 };
}

function replanResult(
  status: AutonomousReplanCycleStatus,
  final: AutonomousDecisionCycleResult | null,
  attempts: AutonomousReplanAttempt[],
  evaluations: AutonomousReplanEvaluationProjection[],
  plannerEvaluations: AutonomousReplanPlanningEvaluationProjection[],
  learningEpisodeIds: string[],
  settlements: AutonomousLearningSettlement[],
  plannerSettlements: AutonomousPlanningQualitySettlement[],
): AutonomousReplanCycleResult {
  return {
    schema: AUTONOMOUS_REPLAN_CYCLE_SCHEMA,
    status,
    final,
    attempts,
    replan_count: Math.max(0, attempts.length - 1),
    evaluations,
    planner_evaluations: plannerEvaluations,
    learning_episode_ids: learningEpisodeIds,
    settlements,
    planner_settlements: plannerSettlements,
    retention: REPLAN_RETENTION,
    authorization: AUTHORIZATION,
  };
}

/**
 * Execute a bounded evaluator-guided single-domain loop. Each completed attempt is evaluated
 * explicitly; a replan can only add a transient, credential-screened context chunk and reuse the
 * reviewed route. Provider responses never become reward, and every attempt settles independently
 * so a later failure cannot erase earlier value-only learning evidence.
 */
export async function runAutonomousReplanCycle(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousReplanCycleOptions,
): Promise<AutonomousReplanCycleResult> {
  if (!options || typeof options.evaluate !== "function") throw new ArgumentError("replan cycle requires an evaluator callback");
  if (!agent || typeof agent.run !== "function" || typeof agent.route !== "function") throw new ArgumentError("replan cycle requires an AutonomousAgent");
  const maxReplans = boundedReplanCount(options.maxReplans);
  const episodePrefix = options.learning ? boundedReplanIdentifier("replan episodePrefix", options.learning.episodePrefix ?? "autonomous-replan") : null;
  if (options.evaluatePlanning && !options.learning?.controller) throw new ArgumentError("replan planner evaluation requires a learning controller");
  if (options.learning && (typeof options.learning.controller.prepareRun !== "function" || typeof options.learning.controller.settleRun !== "function")) throw new ArgumentError("replan learning controller is malformed");
  if (options.evaluatePlanning && options.learning?.controller && typeof options.learning.controller.settlePlanningQuality !== "function") throw new ArgumentError("replan planner learning controller is malformed");

  const persistence = await openCyclePersistence(options, task, "single_domain", maxReplans);
  if (persistence?.state.phase === "terminal") {
    return replanResult(persistence.state.terminal_status as AutonomousReplanCycleStatus, null, persistedSingleAttempts(persistence.state), persistence.state.evaluations as unknown as AutonomousReplanEvaluationProjection[], [], [...persistence.state.learning_episode_ids], [], []);
  }

  const attempts: AutonomousReplanAttempt[] = [];
  const evaluations: AutonomousReplanEvaluationProjection[] = [];
  const plannerEvaluations: AutonomousReplanPlanningEvaluationProjection[] = [];
  const learningEpisodeIds: string[] = [];
  const settlements: AutonomousLearningSettlement[] = [];
  const plannerSettlements: AutonomousPlanningQualitySettlement[] = [];
  let context = [...(options.context ?? [])];
  let routeOverride = options.routeOverride;
  let final: AutonomousDecisionCycleResult | null = null;

  let startAttempt = 0;
  if (persistence) {
    if (persistence.state.phase === "replan_handoff") {
      if (persistence.state.attempt >= maxReplans + 1) throw new ArgumentError("persisted autonomous cycle replan handoff exceeds its attempt limit");
      routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
      const instruction = await rehydrateCycleInstruction(persistence, options.rehydrateReplanInstruction);
      const projection = persistence.state.evaluations[persistence.state.evaluations.length - 1] as unknown as AutonomousReplanEvaluationProjection;
      if (!projection || !projection.replan_requested) throw new ArgumentError("persisted autonomous cycle handoff is missing a replan evaluation");
      const priorEvaluation: AutonomousReplanEvaluation = { ...projection, replan_instruction: instruction };
      const nextContext = await replanContextChunk(persistence.state.attempt + 1, persistence.state.route_digest!, persistence.state.attempts[persistence.state.attempt - 1]?.selection_digest ?? null, persistence.state.outcome_digest, priorEvaluation);
      context = [...context, nextContext];
      startAttempt = persistence.state.attempt;
      attempts.push(...persistedSingleAttempts(persistence.state));
      evaluations.push(...persistence.state.evaluations as unknown as AutonomousReplanEvaluationProjection[]);
      learningEpisodeIds.push(...persistence.state.learning_episode_ids);
    } else {
      startAttempt = persistence.state.attempt - 1;
      if (persistence.state.phase === "execution_pending" && persistence.state.route_digest) routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
      if (persistence.state.phase === "evaluation_pending" || persistence.state.phase === "settlement_pending") routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
      attempts.push(...persistedSingleAttempts(persistence.state));
      evaluations.push(...persistence.state.evaluations as unknown as AutonomousReplanEvaluationProjection[]);
      learningEpisodeIds.push(...persistence.state.learning_episode_ids);
    }
  }

  for (let attempt = startAttempt; attempt <= maxReplans; attempt += 1) {
    let cycle: AutonomousDecisionCycleResult;
    const persistedPhase = persistence?.state.attempt === attempt + 1 ? persistence.state.phase : null;
    try {
      if (persistence && (persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending")) {
        if (!options.rehydrateRun) throw new ArgumentError("restart resume requires rehydrateRun for the persisted autonomous provider outcome");
        const run = await options.rehydrateRun(rehydrationContext(persistence));
        const digests = await replanRunDigests(run);
        if (digests.outcome !== persistence.state.outcome_digest || run.route.route_digest !== persistence.state.route_digest) throw new ArgumentError("rehydrated autonomous run does not match the persisted outcome or route digest");
        cycle = rehydratedSingleCycle(routeOverride!, run);
      } else if (persistence && persistedPhase === "execution_pending" && persistence.state.route_digest && options.rehydrateRun) {
        if (!routeOverride) routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
        const run = await options.rehydrateRun(rehydrationContext(persistence));
        const digests = await replanRunDigests(run);
        if (digests.outcome !== persistence.state.outcome_digest || run.route.route_digest !== persistence.state.route_digest) throw new ArgumentError("rehydrated autonomous run does not match the persisted pending outcome");
        cycle = rehydratedSingleCycle(routeOverride, run);
      } else {
        if (persistence) await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "execution_pending", route_digest: routeOverride?.route_digest ?? null, plan_refinement_digest: persistence.state.plan_refinement_digest, outcome_digest: null, evaluation_digest: null, replan_instruction_digest: null, terminal_status: null });
        cycle = await runAutonomousDecisionCycle(agent, task, {
          ...options,
          semanticRouting: attempt === 0 ? options.semanticRouting : undefined,
          routeOverride,
          context,
          executionAttempt: attempt + 1,
          executionLifecycle: "observe_only",
          learning: undefined,
          memory: undefined,
          cycleId: undefined,
          decisionStateStore: undefined,
          rehydrateRoute: undefined,
          rehydrateRun: undefined,
          rehydrateEvaluation: undefined,
          rehydratePlanningEvaluation: undefined,
          rehydrateResult: undefined,
        });
      }
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    final = cycle;
    let digests: { selection: string | null; outcome: string | null };
    const planRefinementDigest = await cyclePlanRefinementDigest(cycle);
    try {
      digests = await replanRunDigests(cycle.run);
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    if (persistence && cycle.status === "completed" && cycle.run && persistedPhase !== "evaluation_pending" && persistedPhase !== "settlement_pending") {
      await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "evaluation_pending", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: digests.outcome, evaluation_digest: null, replan_instruction_digest: null, terminal_status: null, attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, cycle.run.status, cycle.route.route_digest, digests.selection, digests.outcome, null, [], null, planRefinementDigest)) });
    }
    if (cycle.status === "plan_review_required") {
      const attemptRecord = { attempt: attempt + 1, status: cycle.status, run_status: null, route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, selection_digest: null, outcome_digest: null, evaluation_digest: null, evaluation: null, learning_episode_id: null } satisfies AutonomousReplanAttempt;
      upsertResultAttempt(attempts, attemptRecord);
      if (persistence) await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "execution_pending", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: null, evaluation_digest: null, replan_instruction_digest: null, terminal_status: null, attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, null, cycle.route.route_digest, null, null, null, [], null, planRefinementDigest)) });
      return replanResult("plan_review_required", final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements);
    }
    if (cycle.status !== "completed" || !cycle.run) {
      try {
        await options.execution?.checkpoint({ status: cycle.status, reason: `replan_cycle_${cycle.status}` });
      } catch (error) {
        await failExecutionIfActive(options.execution, error);
        throw error;
      }
      const attemptRecord = { attempt: attempt + 1, status: cycle.status, run_status: cycle.run?.status ?? null, route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, selection_digest: digests.selection, outcome_digest: digests.outcome, evaluation_digest: null, evaluation: null, learning_episode_id: null } satisfies AutonomousReplanAttempt;
      upsertResultAttempt(attempts, attemptRecord);
      if (persistence) await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "terminal", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: digests.outcome, evaluation_digest: null, replan_instruction_digest: null, terminal_status: cycle.status, attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, cycle.run?.status ?? null, cycle.route.route_digest, digests.selection, digests.outcome, null, [], null, planRefinementDigest)) });
      return replanResult(cycle.status, final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements);
    }

    let evaluation: AutonomousReplanEvaluation;
    let projection: AutonomousReplanEvaluationProjection;
    let plannerEvaluation: AutonomousEvaluatorRewardInput | null = null;
    let plannerSettlement: AutonomousPlanningQualitySettlement | null = null;
    const plannerPlan = cycle.plan_refinement ?? options.acceptedSingleDomainPlanRefinement ?? null;
    const plannerEligible = Boolean(options.evaluatePlanning && plannerPlan?.status === "completed");
    let evaluationDigest: string;
    const resumedSettlement = persistedPhase === "settlement_pending";
    try {
      if (resumedSettlement) {
        if (!options.rehydrateEvaluation) throw new ArgumentError("restart resume requires rehydrateEvaluation after a settlement interruption");
        evaluation = normalizeReplanEvaluation(await options.rehydrateEvaluation(rehydrationContext(persistence!)));
      } else {
        evaluation = normalizeReplanEvaluation(await options.evaluate(cycle.run));
      }
      projection = await replanEvaluationProjection(evaluation);
      const executionEvaluationDigest = await digestJson(projection);
      if (plannerEligible) {
        const reward = resumedSettlement
          ? await (options.rehydratePlanningEvaluation ? options.rehydratePlanningEvaluation(rehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydratePlanningEvaluation after a planner settlement interruption")))
          : await options.evaluatePlanning!(plannerPlan!);
        plannerEvaluation = projectCycleReward(reward);
        plannerEvaluations.push({
          evaluator_id: plannerEvaluation.evaluator_id,
          evaluator_version: plannerEvaluation.evaluator_version,
          reward: plannerEvaluation.reward,
          passed: plannerEvaluation.passed,
          failed: plannerEvaluation.failed ?? !plannerEvaluation.passed,
          feedback_digest: plannerEvaluation.feedback_digest ?? null,
          failure_class: plannerEvaluation.failure_class ?? null,
          evidence_digest: plannerEvaluation.evidence_digest ?? null,
        });
        evaluationDigest = await digestJson({ execution_evaluation_digest: executionEvaluationDigest, planning_evaluation_digest: await decisionEvaluationDigest(plannerEvaluation) });
      } else {
        evaluationDigest = executionEvaluationDigest;
      }
      if (resumedSettlement && evaluationDigest !== persistence?.state.evaluation_digest) throw new ArgumentError("rehydrated evaluator packet does not match the persisted evaluation digest");
      if (persistence && !resumedSettlement) {
        const persistedAttempt = cycleAttemptState(attempt + 1, cycle.status, cycle.run.status, cycle.route.route_digest, digests.selection, digests.outcome, evaluationDigest, [], null, planRefinementDigest);
        await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "settlement_pending", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: digests.outcome, evaluation_digest: evaluationDigest, replan_instruction_digest: projection.replan_instruction_digest, evaluations: [...persistence.state.evaluations, projection], attempts: upsertCycleAttempt(persistence.state.attempts, persistedAttempt) });
      }
      if (!resumedSettlement) await options.execution?.recordEvaluation({ evaluatorId: evaluation.evaluator_id, evaluatorVersion: evaluation.evaluator_version, reward: evaluation.reward, passed: evaluation.passed, evaluationDigest: executionEvaluationDigest, failureClass: evaluation.failure_class });
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    let learningEpisodeId: string | null = null;
    let settlement: AutonomousLearningSettlement | null = null;
    try {
      if (options.learning) {
        learningEpisodeId = `${episodePrefix}:${cycle.run.blueprint!.task_digest}:attempt-${attempt + 1}`;
        const episode = await options.learning.controller.prepareRun(cycle.run, { episodeId: learningEpisodeId, runId: learningEpisodeId, stageId: `replan-${attempt + 1}` });
        settlement = await options.learning.controller.settleRun(episode.episode_id, evaluation, { remote: options.learning.remote, outbox: options.learning.outbox });
        learningEpisodeIds.push(episode.episode_id);
        settlements.push(settlement);
      }
      if (plannerEvaluation && plannerPlan) {
        plannerSettlement = await options.learning!.controller.settlePlanningQuality(plannerPlan, {
          domain: cycle.route.primary_domain ?? "cross_domain",
          capability: options.capability,
          riskClass: cycle.run.blueprint?.domain_profile.risk_class ?? "planning_review",
          taskFamily: cycle.run.blueprint?.selection_context.task_family ?? null,
          evaluator: plannerEvaluation,
          remote: options.learning?.remote,
        });
        plannerSettlements.push(plannerSettlement);
      }
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    if (resumedSettlement && evaluations.length > 0) evaluations[evaluations.length - 1] = projection;
    else evaluations.push(projection);
    upsertResultAttempt(attempts, { attempt: attempt + 1, status: cycle.status, run_status: cycle.run.status, route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, selection_digest: digests.selection, outcome_digest: digests.outcome, evaluation_digest: evaluationDigest, evaluation: projection, learning_episode_id: learningEpisodeId });
    final = { ...cycle, planner_evaluation: plannerEvaluation, planner_settlement: plannerSettlement };

    const shouldReplan = evaluation.replan_requested && attempt < maxReplans;
    if (persistence) {
      const settlementDigest = settlement ? await digestJson(settlement) : null;
      const stateLearningEpisodeIds = learningEpisodeId && !persistence.state.learning_episode_ids.includes(learningEpisodeId)
        ? [...persistence.state.learning_episode_ids, learningEpisodeId]
        : [...persistence.state.learning_episode_ids];
      const stateSettlementDigests = settlementDigest && !persistence.state.settlement_digests.includes(settlementDigest)
        ? [...persistence.state.settlement_digests, settlementDigest]
        : [...persistence.state.settlement_digests];
      const plannerSettlementDigest = plannerSettlement?.status === "settled" ? await digestJson(plannerSettlement) : null;
      if (plannerSettlementDigest && !stateSettlementDigests.includes(plannerSettlementDigest)) stateSettlementDigests.push(plannerSettlementDigest);
      await commitCyclePersistence(persistence, {
        attempt: attempt + 1,
        phase: shouldReplan ? "replan_handoff" : "terminal",
        route_digest: cycle.route.route_digest,
        plan_refinement_digest: planRefinementDigest,
        outcome_digest: digests.outcome,
        evaluation_digest: evaluationDigest,
        replan_instruction_digest: shouldReplan ? projection.replan_instruction_digest : null,
        terminal_status: shouldReplan ? null : (evaluation.replan_requested ? "replan_limit_reached" : (evaluation.passed ? "completed" : "completed_without_replan")),
        attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, cycle.run.status, cycle.route.route_digest, digests.selection, digests.outcome, evaluationDigest, learningEpisodeId ? [learningEpisodeId] : [], null, planRefinementDigest)),
        learning_episode_ids: stateLearningEpisodeIds,
        settlement_digests: stateSettlementDigests,
      });
    }

    if (!evaluation.replan_requested) {
      try {
        await options.execution?.complete(evaluation.passed ? "completed" : "completed_without_replan");
      } catch (error) {
        await failExecutionIfActive(options.execution, error);
        throw error;
      }
      return replanResult(evaluation.passed ? "completed" : "completed_without_replan", final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements);
    }
    if (attempt >= maxReplans) {
      try {
        await options.execution?.complete("replan_limit_reached");
      } catch (error) {
        await failExecutionIfActive(options.execution, error);
        throw error;
      }
      return replanResult("replan_limit_reached", final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements);
    }

    let nextContext: AutonomousPromptChunk;
    try {
      nextContext = await replanContextChunk(attempt + 2, cycle.route.route_digest, digests.selection, digests.outcome, evaluation);
      await options.execution?.replan({ instructionDigest: projection.replan_instruction_digest, attempt: attempt + 2, reason: "evaluator_requested" });
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    context = [...context, nextContext];
    routeOverride = cycle.route;
    if (persistence) await commitCyclePersistence(persistence, { context_digests: [...persistence.state.context_digests, await replanContextDigest(nextContext)] });
  }

  throw new ArgumentError("replan cycle exited without a terminal result");
}

export const AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA = "bioprism-typescript-autonomous-cross-domain-decision-cycle/0.1" as const;

export type AutonomousCrossDomainDecisionCycleStatus =
  | AutonomousDecisionCycleStatus
  | "children_completed"
  | "children_partial"
  | "child_failed"
  | "reconciliation_required"
  | "response_review_required";

export type AutonomousCrossDomainDecisionCycleEvaluator = (
  result: AutonomousCrossDomainRunResult,
) => Record<string, AutonomousEvaluatorRewardInput> | Promise<Record<string, AutonomousEvaluatorRewardInput>>;

export interface AutonomousCrossDomainDecisionCycleLearningOptions {
  controller: AutonomousLearningController;
  trajectoryId: string;
  discount?: number;
  evaluate?: AutonomousCrossDomainDecisionCycleEvaluator;
  /** Explicit evaluator for the accepted provider fan-out ordering. */
  evaluatePlanning?: AutonomousDecisionCyclePlanningEvaluator;
  remote?: boolean;
  outbox?: AutonomousLearningOutboxSettlementOptions;
}

export interface AutonomousCrossDomainDecisionCycleOptions extends Omit<AutonomousCrossDomainRunOptions, "learning"> {
  cycleId?: string;
  decisionStateStore?: AutonomousDecisionCycleStateStore;
  retrySemanticRoutingOnRestart?: boolean;
  rehydrateRoute?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousRouteProposal | Promise<AutonomousRouteProposal>;
  rehydrateRun?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousRunResult | AutonomousCrossDomainRunResult | Promise<AutonomousRunResult | AutonomousCrossDomainRunResult>;
  rehydrateEvaluation?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousEvaluatorRewardInput | Record<string, AutonomousEvaluatorRewardInput> | Promise<AutonomousEvaluatorRewardInput | Record<string, AutonomousEvaluatorRewardInput>>;
  rehydratePlanningEvaluation?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;
  rehydrateResult?: (context: AutonomousDecisionCycleRehydrationContext) => AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult | Promise<AutonomousDecisionCycleResult | AutonomousCrossDomainDecisionCycleResult>;
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  /** Optional provider proposal phase; it never executes unless acceptPlan is true. */
  providerPlanning?: AutonomousProviderPlanningOptions;
  /** Explicitly accept a completed, non-review provider proposal for this cycle. */
  acceptPlan?: boolean;
  learning?: AutonomousCrossDomainDecisionCycleLearningOptions;
  memory?: AutonomousDecisionCycleMemoryOptions;
}

export interface AutonomousCrossDomainDecisionCycleResult {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA;
  status: AutonomousCrossDomainDecisionCycleStatus;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  run: AutonomousCrossDomainRunResult | null;
  plan_refinement: AutonomousCrossDomainPlanRefinementResult | null;
  learning_episode_ids: string[];
  response_learning_episode_ids: string[];
  evaluation: Record<string, BrainEvaluatorAssessment> | null;
  settlement: AutonomousCrossDomainLearningSettlement | null;
  response_settlements: AutonomousLearningSettlement[];
  planner_evaluation: AutonomousEvaluatorRewardInput | null;
  planner_settlement: AutonomousPlanningQualitySettlement | null;
  memory: AutonomousDecisionCycleMemoryProjection | null;
  retention: "provider_responses_local; value_only_evaluation_and_learning_projection";
  authorization: "semantic_routing_and_fanout_require_separate_explicit_approval";
}

const CROSS_RETENTION = "provider_responses_local; value_only_evaluation_and_learning_projection" as const;
const CROSS_AUTHORIZATION = "semantic_routing_and_fanout_require_separate_explicit_approval" as const;

function crossReviewResult(
  status: AutonomousCrossDomainDecisionCycleStatus,
  route: AutonomousRouteProposal,
  semanticRoute: AutonomousSemanticRouteResult | null,
  planRefinement: AutonomousCrossDomainPlanRefinementResult | null = null,
): AutonomousCrossDomainDecisionCycleResult {
  return {
    schema: AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA,
    status,
    route,
    semantic_route: semanticRoute,
    run: null,
    plan_refinement: planRefinement,
    learning_episode_ids: [],
    response_learning_episode_ids: [],
    evaluation: null,
    settlement: null,
    response_settlements: [],
    planner_evaluation: null,
    planner_settlement: null,
    memory: null,
    retention: CROSS_RETENTION,
    authorization: CROSS_AUTHORIZATION,
  };
}

function autoDecisionCycleNextAction(status: AutonomousAutoDecisionCycleStatus): AutonomousAutoDecisionCycleResult["next_action"] {
  if (status === "route_review_required" || status === "provider_abstained" || status === "policy_review_required" || status === "policy_blocked") return "review_route";
  if (status === "plan_review_required" || status === "provider_invalid" || status === "provider_disagreement") return "review_plan";
  if (status === "approval_required" || status === "reconciliation_required") return "review_provider_or_effect_approval";
  if (status === "completed") return "complete";
  return "inspect_result";
}

/**
 * Resolve one route and enter the matching evaluator-backed decision cycle.
 *
 * The helper exists at the application-facing boundary so callers do not need to guess whether
 * a task became a single-domain run or a fan-out/fan-in run. It deliberately does not hide any
 * authorization: semantic routing, provider planning, provider invocation, effects, and
 * evaluator settlement remain the explicit options of the underlying cycle.
 */
export async function runAutonomousAutoDecisionCycle(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousAutoDecisionCycleOptions = {},
): Promise<AutonomousAutoDecisionCycleResult> {
  if (!agent || typeof agent.route !== "function" || typeof agent.run !== "function" || typeof agent.runCrossDomain !== "function") throw new ArgumentError("automatic decision cycle requires an AutonomousAgent");
  if (options.routeOverride !== undefined && options.semanticRouting?.enabled) throw new ArgumentError("automatic decision cycle cannot combine routeOverride with semanticRouting");
  const costBudget = cyclePlanningBudget(options);
  let route: AutonomousRouteProposal;
  let semanticRoute: AutonomousSemanticRouteResult | null = null;
  if (options.routeOverride !== undefined) {
    route = await validateAutonomousRouteOverride(task, options.routeOverride);
  } else if (options.semanticRouting?.enabled) {
    semanticRoute = await semanticRouteAutonomousTask(agent, task, {
      candidates: options.candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      hints: options.hints,
      approveProviderCall: options.semanticRouting.approveProviderCall,
      minSemanticConfidence: options.semanticRouting.minSemanticConfidence,
      maxDomains: options.semanticRouting.maxDomains,
      allowCrossDomain: options.semanticRouting.allowCrossDomain ?? options.allowCrossDomain,
      maxOutputTokens: options.semanticRouting.maxOutputTokens,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      costBudget,
      execution: options.semanticRouting.execution ?? options.execution,
      executionAttempt: options.semanticRouting.executionAttempt ?? options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers ?? options.maxProviderFailovers,
      executionLifecycle: options.semanticRouting.executionLifecycle ?? options.executionLifecycle,
      signal: options.signal,
      observer: options.observer,
      domainPolicyMode: options.semanticRouting.domainPolicyMode ?? options.domainPolicyMode,
      domainPolicyEvidenceReady: options.semanticRouting.domainPolicyEvidenceReady ?? options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: options.semanticRouting.domainPolicyEvaluatorConfigured ?? options.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: options.semanticRouting.domainPolicyEffectsRequested ?? options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: options.semanticRouting.domainPolicyEffectsApproved ?? options.domainPolicyEffectsApproved,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      const status = semanticRoute.status as AutonomousAutoDecisionCycleStatus;
      return {
        schema: AUTONOMOUS_AUTO_DECISION_CYCLE_SCHEMA,
        status,
        mode: route.cross_domain && route.selected_domains.length > 1 ? "cross_domain" : route.primary_domain ? "single_domain" : null,
        route,
        semantic_route: semanticRoute,
        cycle: null,
        next_action: autoDecisionCycleNextAction(status),
        retention: "provider_response_local;route_and_cycle_metadata_value_only;execution_result_caller_owned",
        authorization: "routing_planning_provider_effects_and_evaluator_settlement_remain_explicit",
      };
    }
  } else {
    route = await agent.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
  }

  const crossDomain = route.cross_domain && route.selected_domains.length > 1;
  const innerOptions = {
    ...options,
    routeOverride: route,
    semanticRouting: undefined,
    costBudget,
    maxTotalCostUnits: undefined,
  };
  const cycle = crossDomain
    ? await runAutonomousCrossDomainDecisionCycle(agent, task, innerOptions as AutonomousCrossDomainDecisionCycleOptions)
    : await runAutonomousDecisionCycle(agent, task, innerOptions as AutonomousDecisionCycleOptions);
  const cycleWithSemanticRoute = semanticRoute === null ? cycle : { ...cycle, semantic_route: semanticRoute };
  return {
    schema: AUTONOMOUS_AUTO_DECISION_CYCLE_SCHEMA,
    status: cycle.status,
    mode: crossDomain ? "cross_domain" : "single_domain",
    route,
    semantic_route: semanticRoute,
    cycle: cycleWithSemanticRoute,
    next_action: autoDecisionCycleNextAction(cycle.status),
    retention: "provider_response_local;route_and_cycle_metadata_value_only;execution_result_caller_owned",
    authorization: "routing_planning_provider_effects_and_evaluator_settlement_remain_explicit",
  };
}

function autoReplanCycleNextAction(status: AutonomousAutoReplanCycleStatus): AutonomousAutoReplanCycleResult["next_action"] {
  if (status === "route_review_required" || status === "provider_abstained" || status === "policy_review_required" || status === "policy_blocked") return "review_route";
  if (status === "plan_review_required" || status === "provider_invalid" || status === "provider_disagreement") return "review_plan";
  if (status === "approval_required" || status === "reconciliation_required") return "review_provider_or_effect_approval";
  if (status === "completed") return "complete";
  return "inspect_result";
}

/**
 * Resolve one route and enter the matching bounded evaluator-guided replan cycle.
 *
 * The evaluator is intentionally supplied to the selected kernel rather than used to choose a
 * new domain. Replans may refine transient context and prompts, but they can never widen or
 * change the reviewed route. A shared cost budget also covers semantic routing, provider
 * planning, every provider attempt, and any cross-domain synthesis call.
 */
export async function runAutonomousAutoReplanCycle(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousAutoReplanCycleOptions,
): Promise<AutonomousAutoReplanCycleResult> {
  if (!options || typeof options.evaluate !== "function") throw new ArgumentError("automatic replan cycle requires an evaluator callback");
  if (!agent || typeof agent.route !== "function" || typeof agent.run !== "function" || typeof agent.runCrossDomain !== "function") throw new ArgumentError("automatic replan cycle requires an AutonomousAgent");
  if (options.routeOverride !== undefined && options.semanticRouting?.enabled) throw new ArgumentError("automatic replan cycle cannot combine routeOverride with semanticRouting");
  const costBudget = cyclePlanningBudget(options);
  let route: AutonomousRouteProposal;
  let semanticRoute: AutonomousSemanticRouteResult | null = null;
  if (options.routeOverride !== undefined) {
    route = await validateAutonomousRouteOverride(task, options.routeOverride);
  } else if (options.semanticRouting?.enabled) {
    semanticRoute = await semanticRouteAutonomousTask(agent, task, {
      candidates: options.candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      hints: options.hints,
      approveProviderCall: options.semanticRouting.approveProviderCall,
      minSemanticConfidence: options.semanticRouting.minSemanticConfidence,
      maxDomains: options.semanticRouting.maxDomains,
      allowCrossDomain: options.semanticRouting.allowCrossDomain ?? options.allowCrossDomain,
      maxOutputTokens: options.semanticRouting.maxOutputTokens,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      costBudget,
      execution: options.semanticRouting.execution ?? options.execution,
      executionAttempt: options.semanticRouting.executionAttempt ?? options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers ?? options.maxProviderFailovers,
      executionLifecycle: options.semanticRouting.executionLifecycle ?? options.executionLifecycle,
      signal: options.signal,
      observer: options.observer,
      domainPolicyMode: options.semanticRouting.domainPolicyMode ?? options.domainPolicyMode,
      domainPolicyEvidenceReady: options.semanticRouting.domainPolicyEvidenceReady ?? options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: options.semanticRouting.domainPolicyEvaluatorConfigured ?? options.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: options.semanticRouting.domainPolicyEffectsRequested ?? options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: options.semanticRouting.domainPolicyEffectsApproved ?? options.domainPolicyEffectsApproved,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      const status = semanticRoute.status as AutonomousAutoReplanCycleStatus;
      return {
        schema: AUTONOMOUS_AUTO_REPLAN_CYCLE_SCHEMA,
        status,
        mode: route.cross_domain && route.selected_domains.length > 1 ? "cross_domain" : route.primary_domain ? "single_domain" : null,
        route,
        semantic_route: semanticRoute,
        cycle: null,
        next_action: autoReplanCycleNextAction(status),
        retention: "provider_response_local;route_and_replan_metadata_value_only;execution_result_caller_owned",
        authorization: "routing_planning_provider_effects_evaluator_settlement_and_replanning_remain_explicit",
      };
    }
  } else {
    route = await agent.route(task, {
      domain: options.domain,
      hints: options.hints,
      minConfidence: options.minConfidence,
      minMargin: options.minMargin,
      maxDomains: options.maxDomains,
      allowCrossDomain: options.allowCrossDomain,
    });
  }

  const crossDomain = route.cross_domain && route.selected_domains.length > 1;
  const innerOptions = {
    ...options,
    routeOverride: route,
    semanticRouting: undefined,
    costBudget,
    maxTotalCostUnits: undefined,
  };
  const cycle = crossDomain
    ? await runAutonomousCrossDomainReplanCycle(agent, task, innerOptions as AutonomousCrossDomainReplanCycleOptions)
    : await runAutonomousReplanCycle(agent, task, innerOptions as AutonomousReplanCycleOptions);
  const cycleWithSemanticRoute = semanticRoute === null ? cycle : { ...cycle, semantic_route: semanticRoute };
  return {
    schema: AUTONOMOUS_AUTO_REPLAN_CYCLE_SCHEMA,
    status: cycle.status,
    mode: crossDomain ? "cross_domain" : "single_domain",
    route,
    semantic_route: semanticRoute,
    cycle: cycleWithSemanticRoute,
    next_action: autoReplanCycleNextAction(cycle.status),
    retention: "provider_response_local;route_and_replan_metadata_value_only;execution_result_caller_owned",
    authorization: "routing_planning_provider_effects_evaluator_settlement_and_replanning_remain_explicit",
  };
}

function crossRunOptions(options: AutonomousCrossDomainDecisionCycleOptions, route: AutonomousRouteProposal, memoryChunk: AutonomousPromptChunk | null, costBudget?: AutonomousCostBudget): AutonomousCrossDomainRunOptions {
  return {
    routeOverride: route,
    capability: options.capability,
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context: withMemoryContext(options.context, memoryChunk),
    memoryRecall: options.memoryRecall,
    hints: options.hints,
    allowCrossDomain: options.allowCrossDomain,
    maxInputTokens: options.maxInputTokens,
    maxOutputTokens: options.maxOutputTokens,
    maxCostPerMillionTokens: options.maxCostPerMillionTokens,
    maxLatencyMs: options.maxLatencyMs,
    minQuality: options.minQuality,
    maxTotalCostUnits: costBudget ? undefined : options.maxTotalCostUnits,
    costBudget,
    requireJson: options.requireJson,
    responseSchema: options.responseSchema,
    structuredDomainResponse: options.structuredDomainResponse,
    temperature: options.temperature,
    tools: options.tools,
    authorizeAndExecute: options.authorizeAndExecute,
    toolReadOnly: options.toolReadOnly,
    approveProviderCall: options.approveProviderCall,
    approveEffects: options.approveEffects,
    execution: options.execution,
    executionAttempt: options.executionAttempt,
    maxProviderFailovers: options.maxProviderFailovers,
    executionLifecycle: options.executionLifecycle,
    signal: options.signal,
    observer: options.observer,
    subtasks: options.subtasks,
    allowPartial: options.allowPartial,
    synthesize: options.synthesize,
    maxParallelChildren: options.maxParallelChildren,
    learning: options.learning?.evaluate ? options.learning.controller : undefined,
    acceptedCrossDomainPlanRefinement: options.acceptedCrossDomainPlanRefinement,
  };
}

function projectedEvaluations(settlement: AutonomousCrossDomainLearningSettlement): Record<string, BrainEvaluatorAssessment> {
  return Object.fromEntries(settlement.trajectory.settlements.map((item) => [item.episode.episode_id, item.assessment]));
}

/**
 * Execute the bounded fan-out/fan-in decision cycle with optional semantic routing and delayed
 * credit across completed specialists and synthesis. Child and synthesis learning identities are
 * created by the existing cross-domain runner and settled only from an exact evaluator packet.
 */
export async function runAutonomousCrossDomainDecisionCycle(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousCrossDomainDecisionCycleOptions = {},
): Promise<AutonomousCrossDomainDecisionCycleResult> {
  if (!agent || typeof agent.runCrossDomain !== "function" || typeof agent.route !== "function") throw new ArgumentError("cross-domain decision cycle requires an AutonomousAgent");
  if (options.semanticRouting?.enabled && options.domain !== undefined) throw new ArgumentError("semantic decision routing cannot replace an explicit caller domain");
  if (options.learning && !options.learning.controller) throw new ArgumentError("cross-domain decision cycle learning controller is malformed");
  if (options.learning?.evaluate && (typeof options.learning.controller.prepareCrossDomainTrajectory !== "function" || typeof options.learning.controller.settleCrossDomain !== "function")) throw new ArgumentError("cross-domain decision cycle execution learning controller is malformed");
  if (options.learning?.evaluatePlanning && typeof options.learning.controller.settlePlanningQuality !== "function") throw new ArgumentError("cross-domain decision cycle planner learning controller is malformed");
  const costBudget = cyclePlanningBudget(options);
  const persistence = await openDecisionPersistence(options, task, "cross_domain", options.learning !== undefined, Boolean(options.learning?.evaluate || options.learning?.evaluatePlanning), options.learning?.trajectoryId ?? null);
  if (persistence?.state.phase === "terminal") {
    return await rehydrateDecisionResult(persistence, options.rehydrateResult) as AutonomousCrossDomainDecisionCycleResult;
  }
  const persistedRoute = persistence?.restored === true && persistence.state.route_digest !== null;
  const persistedPhase = persistence?.restored === true && (persistence.state.phase !== "route_pending" || persistedRoute) ? persistence.state.phase : null;

  let route: AutonomousRouteProposal;
  let semanticRoute: AutonomousSemanticRouteResult | null = null;
  let rehydratedRun: AutonomousCrossDomainRunResult | null = null;
  if (persistedPhase) {
    route = await rehydrateDecisionRoute(persistence!, options.rehydrateRoute);
    if (persistedPhase === "execution_pending" || persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending") {
      rehydratedRun = await rehydrateDecisionRun(persistence!, options.rehydrateRun) as AutonomousCrossDomainRunResult;
    }
  } else if (persistence?.restored === true && options.rehydrateRoute) {
    route = await rehydrateDecisionRoute(persistence, options.rehydrateRoute);
  } else if (persistence?.restored === true && options.semanticRouting?.enabled && options.retrySemanticRoutingOnRestart !== true) {
    throw new ArgumentError("restart resume of provider-assisted semantic routing requires rehydrateRoute or retrySemanticRoutingOnRestart: true");
  } else if (options.semanticRouting?.enabled) {
    semanticRoute = await semanticRouteAutonomousTask(agent, task, {
      candidates: options.candidates,
      credential: options.credential,
      credentialFor: options.credentialFor,
      hints: options.hints,
      approveProviderCall: options.semanticRouting.approveProviderCall,
      minSemanticConfidence: options.semanticRouting.minSemanticConfidence,
      maxDomains: options.semanticRouting.maxDomains,
      allowCrossDomain: options.semanticRouting.allowCrossDomain ?? true,
      maxOutputTokens: options.semanticRouting.maxOutputTokens,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      maxTotalCostUnits: undefined,
      costBudget,
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers,
      executionLifecycle: options.executionLifecycle,
      domainPolicyMode: options.semanticRouting.domainPolicyMode ?? options.domainPolicyMode,
      domainPolicyEvidenceReady: options.semanticRouting.domainPolicyEvidenceReady ?? options.domainPolicyEvidenceReady,
      domainPolicyEvaluatorConfigured: options.semanticRouting.domainPolicyEvaluatorConfigured ?? options.domainPolicyEvaluatorConfigured,
      domainPolicyEffectsRequested: options.semanticRouting.domainPolicyEffectsRequested ?? options.domainPolicyEffectsRequested,
      domainPolicyEffectsApproved: options.semanticRouting.domainPolicyEffectsApproved ?? options.domainPolicyEffectsApproved,
      signal: options.signal,
      observer: options.observer,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: semanticRoute.status, reason: `semantic_route_${semanticRoute.status}` });
      const reviewed = crossReviewResult(semanticRoute.status === "approval_required" ? "approval_required" : semanticRoute.status, route, semanticRoute);
      await commitDecisionPersistence(persistence, { phase: "terminal", route_digest: route.route_digest, selection_digest: null, outcome_digest: await digestJson({ status: reviewed.status, route_digest: route.route_digest }), evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: reviewed.status });
      return reviewed;
    }
  } else if (options.routeOverride) {
    route = options.routeOverride;
  } else {
    route = await agent.route(task, { hints: options.hints, allowCrossDomain: options.allowCrossDomain ?? true });
  }
  const decisionBlueprint = await agent.blueprint(task, {
    routeOverride: route,
    capability: options.capability,
    context: options.context,
    maxInputTokens: options.maxInputTokens,
    tools: options.tools?.map((tool) => tool.name),
    hints: options.hints,
    subtasks: options.subtasks,
  });
  const taskDecisionMetadata = await decisionTaskMetadataForAutoBlueprint(decisionBlueprint);
  if (persistence && persistence.state.route_digest === null) await commitDecisionPersistence(persistence, { phase: "route_pending", route_digest: route.route_digest, ...taskDecisionMetadata, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
  if (route.abstained || !route.cross_domain || route.selected_domains.length < 2) {
    if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: "route_review_required", reason: "cross_domain_route_review_required" });
    const reviewed = crossReviewResult("route_review_required", route, semanticRoute);
    await commitDecisionPersistence(persistence, { phase: "terminal", route_digest: route.route_digest, selection_digest: null, outcome_digest: await digestJson({ status: reviewed.status, route_digest: route.route_digest }), evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: reviewed.status });
    return reviewed;
  }

  const recalledMemory = await recallMemory(options.memory, route, task, options.memoryRecall);
  let planRefinement: AutonomousCrossDomainPlanRefinementResult | null = null;
  const persistedPlanRefinementDigest = persistence?.state.plan_refinement_digest ?? null;
  if (options.providerPlanning || options.acceptedCrossDomainPlanRefinement !== undefined || persistedPlanRefinementDigest !== null) {
    const blueprintEnvelope = await agent.blueprint(task, {
      routeOverride: route,
      capability: options.capability,
      context: withMemoryContext(options.context, recalledMemory.promptChunk),
      hints: options.hints,
      maxInputTokens: options.maxInputTokens,
      tools: options.tools?.map((tool) => tool.name),
      subtasks: options.subtasks,
    });
    if (!blueprintEnvelope.cross_domain_blueprint || blueprintEnvelope.blueprint === null) throw new ArgumentError("cross-domain decision planning requires a cross-domain blueprint");
    const accepted = await acceptedCrossDomainPlan(blueprintEnvelope.cross_domain_blueprint, options.acceptedCrossDomainPlanRefinement);
    if (persistedPlanRefinementDigest !== null && !accepted) throw new ArgumentError("restart resume requires the caller to rehydrate the accepted cross-domain plan refinement");
    if (accepted && persistedPlanRefinementDigest !== null && accepted.refinement_digest !== persistedPlanRefinementDigest) throw new ArgumentError("accepted cross-domain plan does not match the persisted planning digest");
    planRefinement = options.acceptedCrossDomainPlanRefinement ?? null;
    if (!accepted && options.providerPlanning) {
      await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: null, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
      const proposal = await agent.planCrossDomainWithProvider(blueprintEnvelope.cross_domain_blueprint, {
        ...options.providerPlanning,
        context: withMemoryContext(options.providerPlanning.context, recalledMemory.promptChunk),
        costBudget,
        maxTotalCostUnits: undefined,
        execution: options.providerPlanning.execution ?? options.execution,
        executionAttempt: options.providerPlanning.executionAttempt ?? options.executionAttempt,
        signal: options.providerPlanning.signal ?? options.signal,
      });
      if (proposal.status !== "completed") {
        const status: AutonomousCrossDomainDecisionCycleStatus = proposal.status === "approval_required" ? "approval_required" : proposal.status === "provider_invalid" ? "provider_invalid" : "provider_disagreement";
        const reviewed = crossReviewResult(status, route, semanticRoute, proposal);
        await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: null, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
        return reviewed;
      }
      const proposalDigest = await digestJson(proposal);
      if (proposal.review_required || options.acceptPlan !== true) {
        if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: "plan_review_required", reason: "cross_domain_provider_plan_review_required" });
        const reviewed = crossReviewResult("plan_review_required", route, semanticRoute, proposal);
        await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: proposalDigest, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
        return reviewed;
      }
      planRefinement = proposal;
      await commitDecisionPersistence(persistence, { phase: "planning_pending", route_digest: route.route_digest, plan_refinement_digest: proposalDigest, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
    }
  }
  let run: AutonomousCrossDomainRunResult;
  try {
    if (rehydratedRun) {
      run = rehydratedRun;
    } else {
      await commitDecisionPersistence(persistence, { phase: "execution_pending", route_digest: route.route_digest, selection_digest: null, outcome_digest: null, evaluation_digest: null, learning_episode_ids: [], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
      run = await agent.runCrossDomain(task, { ...crossRunOptions(options, route, recalledMemory.promptChunk, costBudget), acceptedCrossDomainPlanRefinement: planRefinement ?? undefined });
    }
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
  try {
    const outcomeDigest = await crossDomainReplanOutcomeDigest(run);
    if (persistence && (persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending") && persistence.state.outcome_digest !== outcomeDigest) throw new ArgumentError("rehydrated cross-domain run does not match the persisted outcome digest");
    let settlement: AutonomousCrossDomainLearningSettlement | null = null;
    let plannerEvaluation: AutonomousEvaluatorRewardInput | null = null;
    let plannerSettlement: AutonomousPlanningQualitySettlement | null = null;
    let rewards: Record<string, AutonomousEvaluatorRewardInput> = {};
    const executionEvaluationEnabled = Boolean(options.learning?.evaluate && run.learning_episode_ids.length > 0);
    const planningEvaluationEnabled = Boolean(options.learning?.evaluatePlanning && planRefinement?.status === "completed");
    const cycleEvaluationEnabled = executionEvaluationEnabled || planningEvaluationEnabled;
    if (persistence && (persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending") && !cycleEvaluationEnabled) throw new ArgumentError("persisted cross-domain evaluator state has no configured evaluator boundary");
    if (options.learning && cycleEvaluationEnabled) {
      const learning = options.learning;
      const resumedEvaluation = persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending";
      const persistedEvaluationDigest = persistence?.state.evaluation_digest ?? null;
      if (persistence && !resumedEvaluation) await commitDecisionPersistence(persistence, { phase: "evaluation_pending", route_digest: route.route_digest, selection_digest: null, outcome_digest: outcomeDigest, evaluation_digest: null, learning_episode_ids: [...run.learning_episode_ids], trajectory_id: persistence.trajectoryId, settlement_digests: [], terminal_status: null });
      if (executionEvaluationEnabled) {
        const rawRewards = resumedEvaluation
          ? await (options.rehydrateEvaluation ? options.rehydrateEvaluation(decisionRehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydrateEvaluation for the persisted cross-domain evaluator boundary"))) as Record<string, AutonomousEvaluatorRewardInput>
          : await learning.evaluate!(run);
        rewards = Object.fromEntries(Object.entries(rawRewards).map(([episodeId, reward]) => [episodeId, projectCycleReward(reward)]));
      }
      if (planningEvaluationEnabled) {
        const reward = resumedEvaluation
          ? await (options.rehydratePlanningEvaluation ? options.rehydratePlanningEvaluation(decisionRehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydratePlanningEvaluation for the persisted cross-domain planner evaluator boundary")))
          : await learning.evaluatePlanning!(planRefinement!);
        plannerEvaluation = projectCycleReward(reward);
      }
      const executionDigest = executionEvaluationEnabled ? await decisionCrossEvaluationDigest(rewards) : null;
      const evaluationDigest = plannerEvaluation
        ? await digestJson({ execution_digest: executionDigest, planning_digest: await decisionEvaluationDigest(plannerEvaluation) })
        : executionDigest;
      if (persistedPhase === "settlement_pending" && persistedEvaluationDigest !== evaluationDigest) throw new ArgumentError("rehydrated cross-domain evaluation does not match the persisted evaluation digest");
      await commitDecisionPersistence(persistence, { phase: "settlement_pending", route_digest: route.route_digest, selection_digest: null, outcome_digest: outcomeDigest, evaluation_digest: evaluationDigest, learning_episode_ids: [...run.learning_episode_ids], trajectory_id: persistence?.trajectoryId ?? null, settlement_digests: [], terminal_status: null });
      if (executionEvaluationEnabled) {
        settlement = await learning.controller.settleCrossDomain(run, rewards, {
          trajectoryId: learning.trajectoryId,
          discount: learning.discount,
          remote: learning.remote,
          outbox: learning.outbox,
          ...(persistence ? { idempotencyKey: `decision:${persistence.cycleId}:${learning.trajectoryId}` } : {}),
        });
      }
      if (plannerEvaluation && planRefinement) {
        plannerSettlement = await learning.controller.settlePlanningQuality(planRefinement, {
          domain: route.primary_domain ?? "cross_domain",
          capability: options.capability,
          riskClass: "cross_domain_integration",
          taskFamily: "cross_domain_synthesis",
          evaluator: plannerEvaluation,
          remote: learning.remote,
        });
      }
    }
    const memoryProjection = recalledMemory.projection;
    if (options.memory) {
      const completedRuns = [
        ...run.child_runs.filter((child) => child.result.status === "completed").map((child) => child.result),
        ...(run.synthesis?.status === "completed" ? [run.synthesis] : []),
      ];
      for (let index = 0; index < completedRuns.length; index += 1) {
        const childRun = completedRuns[index]!;
        const learningEpisodeId = run.learning_episode_ids[index] ?? null;
        const explicitSingleId = options.memory.episodeId && completedRuns.length === 1 ? options.memory.episodeId : null;
        const prefix = options.memory.episodePrefix ?? options.memory.episodeId ?? "memory:cross";
        const memoryEpisodeId = explicitSingleId ?? `${prefix}:${learningEpisodeId ?? `${childRun.blueprint?.task_digest ?? index}:${childRun.blueprint?.prompt.prompt_digest ?? index}`}`;
        const memoryEpisode = await memoryPacketForRun(options.memory, childRun, memoryEpisodeId, task);
        if (!memoryEpisode) continue;
        memoryProjection.recorded_episode_ids.push(memoryEpisode.episode_id);
        const settlementItem = settlement?.trajectory.settlements.find((item) => item.episode.episode_id === learningEpisodeId);
        if (settlementItem) {
          await recordMemoryEvaluation(options.memory, memoryEpisode.episode_id, settlementItem.assessment);
          memoryProjection.evaluation_recorded_episode_ids.push(memoryEpisode.episode_id);
        }
      }
    }
    const settlementDigests = [
      settlement?.trajectory.settlement_digest ?? (settlement ? await digestJson(settlement.trajectory) : null),
      plannerSettlement?.status === "settled" ? await digestJson(plannerSettlement) : null,
    ].filter((digest): digest is string => digest !== null);
    await commitDecisionPersistence(persistence, {
      phase: "terminal",
      route_digest: route.route_digest,
      selection_digest: null,
      outcome_digest: outcomeDigest,
      evaluation_digest: plannerEvaluation
        ? await digestJson({ execution_digest: executionEvaluationEnabled ? await decisionCrossEvaluationDigest(rewards) : null, planning_digest: await decisionEvaluationDigest(plannerEvaluation) })
        : executionEvaluationEnabled ? await decisionCrossEvaluationDigest(rewards) : null,
      learning_episode_ids: [...run.learning_episode_ids],
      trajectory_id: persistence?.trajectoryId ?? null,
      settlement_digests: settlementDigests,
      terminal_status: run.status,
    });
    if (options.executionLifecycle !== "observe_only") {
      if (run.status === "completed" || run.status === "children_completed" || run.status === "children_partial") await options.execution?.complete(run.status);
      else if (run.status !== "reconciliation_required") await options.execution?.checkpoint({ status: run.status, reason: `cross_domain_${run.status}` });
    }
    return {
      schema: AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA,
      status: run.status,
      route,
      semantic_route: semanticRoute,
      run,
      plan_refinement: planRefinement,
      learning_episode_ids: [...run.learning_episode_ids],
      response_learning_episode_ids: [...(run.response_learning_episode_ids ?? [])],
      evaluation: settlement ? projectedEvaluations(settlement) : null,
      settlement,
      response_settlements: settlement?.response_settlements ?? [],
      planner_evaluation: plannerEvaluation,
      planner_settlement: plannerSettlement,
      memory: memoryProjection,
      retention: CROSS_RETENTION,
      authorization: CROSS_AUTHORIZATION,
    };
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
}

export const AUTONOMOUS_CROSS_DOMAIN_REPLAN_CYCLE_SCHEMA = "bioprism-typescript-autonomous-cross-domain-replan-cycle/0.1" as const;
export const AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_SCHEMA = "bioprism-typescript-autonomous-cross-domain-replan-context/0.1" as const;

export interface AutonomousCrossDomainReplanEvaluation extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: string | null;
  failure_class?: string | null;
  evidence_digest?: string | null;
  rewards: Record<string, AutonomousEvaluatorRewardInput>;
  replan_requested: boolean;
  replan_instruction?: string | null;
}

export type AutonomousCrossDomainReplanEvaluator = (
  result: AutonomousCrossDomainRunResult,
) => AutonomousCrossDomainReplanEvaluation | Promise<AutonomousCrossDomainReplanEvaluation>;

export interface AutonomousCrossDomainReplanLearningOptions {
  controller: AutonomousLearningController;
  /** Prefix must be unique for the caller's logical cross-domain replan cycle. */
  episodePrefix?: string;
  /** Prefix must be unique for the caller's logical cross-domain replan cycle. */
  trajectoryIdPrefix?: string;
  discount?: number;
  remote?: boolean;
  outbox?: AutonomousLearningOutboxSettlementOptions;
}

export interface AutonomousCrossDomainReplanEvaluationProjection extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string | null;
  reward_episode_count: number;
  replan_requested: boolean;
  replan_instruction_digest: string | null;
}

export interface AutonomousCrossDomainReplanAttempt extends JsonObject {
  attempt: number;
  status: AutonomousCrossDomainDecisionCycleStatus;
  run_status: AutonomousCrossDomainRunResult["status"] | null;
  route_digest: string | null;
  plan_refinement_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evaluation: AutonomousCrossDomainReplanEvaluationProjection | null;
  learning_episode_ids: string[];
  trajectory_id: string | null;
}

export type AutonomousCrossDomainReplanCycleStatus =
  | AutonomousCrossDomainDecisionCycleStatus
  | "completed_without_replan"
  | "replan_limit_reached";

export interface AutonomousCrossDomainReplanCycleOptions extends Omit<AutonomousCrossDomainDecisionCycleOptions, "learning" | "cycleId" | "decisionStateStore" | "rehydrateRoute" | "rehydrateRun" | "rehydrateEvaluation" | "rehydratePlanningEvaluation" | "rehydrateResult"> {
  evaluate: AutonomousCrossDomainReplanEvaluator;
  /** Explicit evaluator for each accepted provider fan-out ordering. */
  evaluatePlanning?: AutonomousDecisionCyclePlanningEvaluator;
  /** Additional evaluator-requested fan-out/fan-in attempts. The SDK caps this at three. */
  maxReplans?: number;
  learning?: AutonomousCrossDomainReplanLearningOptions;
  /** Stable caller-owned identity used to resume this logical cycle after a process restart. */
  cycleId?: string;
  /** Optional metadata-only state store. Private task/run/evaluator material remains caller-owned. */
  stateStore?: AutonomousCycleReplanStateStore;
  /** Rehydrate a private cross-domain run after provider execution. */
  rehydrateRun?: AutonomousCycleReplanRunRehydrator<AutonomousCrossDomainRunResult>;
  /** Rehydrate a private route when resuming a persisted replan handoff. */
  rehydrateRoute?: AutonomousCycleReplanRouteRehydrator<AutonomousRouteProposal>;
  /** Rehydrate the private evaluator packet when settlement was interrupted. */
  rehydrateEvaluation?: AutonomousCycleReplanEvaluationRehydrator;
  /** Rehydrate planner quality values after a settlement boundary. */
  rehydratePlanningEvaluation?: (context: AutonomousCycleReplanRehydrationContext) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;
  /** Rehydrate transient evaluator guidance from caller-owned storage. */
  rehydrateReplanInstruction?: AutonomousCycleReplanInstructionRehydrator;
}

export interface AutonomousCrossDomainReplanCycleResult {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_REPLAN_CYCLE_SCHEMA;
  status: AutonomousCrossDomainReplanCycleStatus;
  final: AutonomousCrossDomainDecisionCycleResult | null;
  attempts: AutonomousCrossDomainReplanAttempt[];
  replan_count: number;
  evaluations: AutonomousCrossDomainReplanEvaluationProjection[];
  planner_evaluations: AutonomousReplanPlanningEvaluationProjection[];
  learning_episode_ids: string[];
  response_learning_episode_ids: string[];
  settlements: AutonomousCrossDomainLearningSettlement[];
  response_settlements: AutonomousLearningSettlement[];
  planner_settlements: AutonomousPlanningQualitySettlement[];
  retention: "provider_responses_local; replan_instructions_transient; value_only_evaluation_and_learning_projection";
  authorization: "semantic_routing_and_fanout_require_separate_explicit_approval";
}

const CROSS_REPLAN_RETENTION = "provider_responses_local; replan_instructions_transient; value_only_evaluation_and_learning_projection" as const;

function normalizeCrossDomainReplanReward(value: unknown, episodeId: string): AutonomousEvaluatorRewardInput {
  if (!isObject(value)) throw new ArgumentError(`cross-domain replan reward for ${episodeId} must be an object`);
  const evaluatorId = boundedReplanIdentifier(`cross-domain replan reward ${episodeId} evaluator_id`, value.evaluator_id);
  const evaluatorVersion = boundedReplanIdentifier(`cross-domain replan reward ${episodeId} evaluator_version`, value.evaluator_version);
  const reward = boundedReplanReward(value.reward);
  if (typeof value.passed !== "boolean") throw new ArgumentError(`cross-domain replan reward for ${episodeId} passed must be boolean`);
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new ArgumentError(`cross-domain replan reward for ${episodeId} failed must be boolean`);
  let failureClass: string | null = null;
  if (value.failure_class !== undefined && value.failure_class !== null) failureClass = boundedReplanIdentifier(`cross-domain replan reward ${episodeId} failure_class`, value.failure_class);
  return {
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward,
    passed: value.passed,
    failed: value.failed,
    feedback_digest: boundedReplanDigest(`cross-domain replan reward ${episodeId} feedback_digest`, value.feedback_digest, true),
    failure_class: failureClass,
    evidence_digest: boundedReplanDigest(`cross-domain replan reward ${episodeId} evidence_digest`, value.evidence_digest, true),
  };
}

function normalizeCrossDomainReplanEvaluation(value: unknown, expectedEpisodeIds: readonly string[]): AutonomousCrossDomainReplanEvaluation {
  if (!isObject(value)) throw new ArgumentError("cross-domain replan evaluator must return an object");
  const evaluatorId = boundedReplanIdentifier("cross-domain replan evaluator_id", value.evaluator_id);
  const evaluatorVersion = boundedReplanIdentifier("cross-domain replan evaluator_version", value.evaluator_version);
  const reward = boundedReplanReward(value.reward);
  if (typeof value.passed !== "boolean") throw new ArgumentError("cross-domain replan evaluator passed must be boolean");
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new ArgumentError("cross-domain replan evaluator failed must be boolean");
  if (typeof value.replan_requested !== "boolean") throw new ArgumentError("cross-domain replan evaluator replan_requested must be boolean");
  const instruction = boundedReplanInstruction(value.replan_instruction);
  if (value.replan_requested && !instruction) throw new ArgumentError("cross-domain replan evaluator must provide a bounded instruction when replan_requested is true");
  if (!value.replan_requested && instruction) throw new ArgumentError("cross-domain replan evaluator supplied an instruction without requesting a replan");
  if (!isObject(value.rewards)) throw new ArgumentError("cross-domain replan evaluator rewards must be an object keyed by learning episode ID");
  const rewards: Record<string, AutonomousEvaluatorRewardInput> = {};
  const rewardEntries = Object.entries(value.rewards);
  if (rewardEntries.length > 32) throw new ArgumentError("cross-domain replan evaluator returned too many episode rewards");
  for (const [episodeId, reward] of rewardEntries) {
    boundedReplanIdentifier("cross-domain replan reward episode_id", episodeId);
    rewards[episodeId] = normalizeCrossDomainReplanReward(reward, episodeId);
  }
  const expected = new Set(expectedEpisodeIds);
  const supplied = Object.keys(rewards);
  if (supplied.length !== expected.size || supplied.some((episodeId) => !expected.has(episodeId))) throw new ArgumentError("cross-domain replan evaluator rewards must cover exactly every pending learning episode");
  let failureClass: string | null = null;
  if (value.failure_class !== undefined && value.failure_class !== null) failureClass = boundedReplanIdentifier("cross-domain replan evaluator failure_class", value.failure_class);
  return {
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    reward,
    passed: value.passed,
    failed: value.failed,
    feedback_digest: boundedReplanDigest("cross-domain replan evaluator feedback_digest", value.feedback_digest, true),
    failure_class: failureClass,
    evidence_digest: boundedReplanDigest("cross-domain replan evaluator evidence_digest", value.evidence_digest, true),
    rewards,
    replan_requested: value.replan_requested,
    replan_instruction: instruction,
  };
}

async function crossDomainReplanEvaluationProjection(value: AutonomousCrossDomainReplanEvaluation): Promise<AutonomousCrossDomainReplanEvaluationProjection> {
  return {
    evaluator_id: value.evaluator_id,
    evaluator_version: value.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed ?? !value.passed,
    feedback_digest: value.feedback_digest ?? null,
    failure_class: value.failure_class ?? null,
    evidence_digest: value.evidence_digest ?? null,
    reward_episode_count: Object.keys(value.rewards).length,
    replan_requested: value.replan_requested,
    replan_instruction_digest: value.replan_instruction ? await digestJson(value.replan_instruction) : null,
  };
}

async function crossDomainReplanOutcomeDigest(run: AutonomousCrossDomainRunResult): Promise<string> {
  return digestJson({
    status: run.status,
    route_digest: run.route.route_digest,
    child_runs: run.child_runs.map((child) => ({
      id: child.id,
      domain: child.domain,
      task_digest: child.task_digest,
      status: child.result.status,
      selection: child.result.selection,
      response: child.result.response,
      output_digest: child.output_digest,
    })),
    synthesis: run.synthesis ? {
      status: run.synthesis.status,
      selection: run.synthesis.selection,
      response: run.synthesis.response,
    } : null,
  });
}

async function prepareCrossDomainReplanEpisodes(
  controller: AutonomousLearningController,
  run: AutonomousCrossDomainRunResult,
  episodePrefix: string,
  attempt: number,
): Promise<AutonomousCrossDomainRunResult> {
  const ids: string[] = [];
  const responseIds: string[] = [];
  const parentJobId = boundedReplanIdentifier("cross-domain replan parent job", `${episodePrefix}:${run.route.task_digest}:attempt-${attempt}`);
  for (const child of run.child_runs) {
    if (child.result.status !== "completed") continue;
    const episodeId = boundedReplanIdentifier("cross-domain replan episode", `${parentJobId}:${child.id}`);
    const episode = await controller.prepareRun(child.result, { episodeId, runId: episodeId, stageId: child.id, parentJobId });
    ids.push(episode.episode_id);
    if (child.result.response_evaluation) {
      const responseEpisodeId = boundedReplanIdentifier("cross-domain replan response episode", `${parentJobId}:response:${(await digestJson({ episode_id: episode.episode_id })).slice(0, 64)}`);
      const responseEpisode = await controller.prepareRun(child.result, { episodeId: responseEpisodeId, runId: responseEpisodeId, stageId: child.id, parentJobId });
      responseIds.push(responseEpisode.episode_id);
    }
  }
  if (run.synthesis?.status === "completed") {
    const episodeId = boundedReplanIdentifier("cross-domain replan episode", `${parentJobId}:synthesis`);
    const episode = await controller.prepareRun(run.synthesis, { episodeId, runId: episodeId, stageId: "synthesis", parentJobId });
    ids.push(episode.episode_id);
    if (run.synthesis.response_evaluation) {
      const responseEpisodeId = boundedReplanIdentifier("cross-domain replan response episode", `${parentJobId}:response:${(await digestJson({ episode_id: episode.episode_id })).slice(0, 64)}`);
      const responseEpisode = await controller.prepareRun(run.synthesis, { episodeId: responseEpisodeId, runId: responseEpisodeId, stageId: "synthesis", parentJobId });
      responseIds.push(responseEpisode.episode_id);
    }
  }
  if (!ids.length) throw new ArgumentError("cross-domain replan requires at least one completed learning episode");
  return { ...run, learning_episode_ids: ids, response_learning_episode_ids: responseIds, learning: "online_bandit_feedback_available" };
}

async function crossDomainReplanContextChunk(
  attempt: number,
  routeDigest: string,
  outcomeDigest: string,
  evaluation: AutonomousCrossDomainReplanEvaluation,
): Promise<AutonomousPromptChunk> {
  const instruction = evaluation.replan_instruction;
  if (!instruction) throw new ArgumentError("cross-domain replan context requires an instruction");
  return {
    id: `autonomous-cross-domain-replan-${attempt}`,
    content: JSON.stringify({
      schema: AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_SCHEMA,
      attempt,
      prior: { route_digest: routeDigest, outcome_digest: outcomeDigest },
      evaluator: {
        evaluator_id: evaluation.evaluator_id,
        evaluator_version: evaluation.evaluator_version,
        reward: evaluation.reward,
        passed: evaluation.passed,
        failed: evaluation.failed ?? !evaluation.passed,
        feedback_digest: evaluation.feedback_digest ?? null,
        failure_class: evaluation.failure_class ?? null,
        evidence_digest: evaluation.evidence_digest ?? null,
      },
      instruction,
      guardrails: [
        "This is bounded evaluator feedback, not a new authorization.",
        "Preserve the reviewed domain set, model capability requirements, tool allow-list, budgets, and approval gates.",
        "Do not treat prior specialist or synthesis responses as verified truth or claim an external effect occurred.",
        "Use the same fan-out/fan-in contract and report uncertainty or unresolved domain disagreement explicitly.",
      ],
    }),
    required: true,
    priority: 95,
  };
}

function crossDomainReplanResult(
  status: AutonomousCrossDomainReplanCycleStatus,
  final: AutonomousCrossDomainDecisionCycleResult | null,
  attempts: AutonomousCrossDomainReplanAttempt[],
  evaluations: AutonomousCrossDomainReplanEvaluationProjection[],
  plannerEvaluations: AutonomousReplanPlanningEvaluationProjection[],
  learningEpisodeIds: string[],
  settlements: AutonomousCrossDomainLearningSettlement[],
  plannerSettlements: AutonomousPlanningQualitySettlement[],
  responseLearningEpisodeIds: string[] = settlements.flatMap((settlement) => settlement.response_settlements.map((item) => item.episode.episode_id)),
): AutonomousCrossDomainReplanCycleResult {
  return {
    schema: AUTONOMOUS_CROSS_DOMAIN_REPLAN_CYCLE_SCHEMA,
    status,
    final,
    attempts,
    replan_count: Math.max(0, attempts.length - 1),
    evaluations,
    planner_evaluations: plannerEvaluations,
    learning_episode_ids: learningEpisodeIds,
    response_learning_episode_ids: [...new Set(responseLearningEpisodeIds)],
    settlements,
    response_settlements: settlements.flatMap((settlement) => settlement.response_settlements),
    planner_settlements: plannerSettlements,
    retention: CROSS_REPLAN_RETENTION,
    authorization: CROSS_AUTHORIZATION,
  };
}

/**
 * Execute a bounded evaluator-guided cross-domain loop. Each attempt runs a complete specialist
 * fan-out and optional synthesis under one shared budget. Evaluator feedback is transient and
 * cannot widen the route, tools, approvals, or cost limits; every attempt gets unique pending
 * episodes and an independently settled trajectory so replay and partial failure remain safe.
 */
export async function runAutonomousCrossDomainReplanCycle(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousCrossDomainReplanCycleOptions,
): Promise<AutonomousCrossDomainReplanCycleResult> {
  if (!options || typeof options.evaluate !== "function") throw new ArgumentError("cross-domain replan cycle requires an evaluator callback");
  if (!agent || typeof agent.runCrossDomain !== "function" || typeof agent.route !== "function") throw new ArgumentError("cross-domain replan cycle requires an AutonomousAgent");
  const maxReplans = boundedReplanCount(options.maxReplans);
  const episodePrefix = options.learning ? boundedReplanIdentifier("cross-domain replan episodePrefix", options.learning.episodePrefix ?? "autonomous-cross-domain-replan") : null;
  const trajectoryIdPrefix = options.learning ? boundedReplanIdentifier("cross-domain replan trajectoryIdPrefix", options.learning.trajectoryIdPrefix ?? "autonomous-cross-domain-replan") : null;
  if (options.evaluatePlanning && !options.learning?.controller) throw new ArgumentError("cross-domain replan planner evaluation requires a learning controller");
  if (options.learning && (!options.learning.controller || typeof options.learning.controller.prepareRun !== "function" || typeof options.learning.controller.settleCrossDomain !== "function")) throw new ArgumentError("cross-domain replan learning controller is malformed");
  if (options.evaluatePlanning && options.learning?.controller && typeof options.learning.controller.settlePlanningQuality !== "function") throw new ArgumentError("cross-domain replan planner learning controller is malformed");
  const costBudget = cycleCostBudget(options);
  const persistence = await openCyclePersistence(options, task, "cross_domain", maxReplans);
  if (persistence?.state.phase === "terminal") {
    return crossDomainReplanResult(persistence.state.terminal_status as AutonomousCrossDomainReplanCycleStatus, null, persistedCrossAttempts(persistence.state), persistence.state.evaluations as unknown as AutonomousCrossDomainReplanEvaluationProjection[], [], [...persistence.state.learning_episode_ids], [], [], [...(persistence.state.response_learning_episode_ids ?? [])]);
  }
  const attempts: AutonomousCrossDomainReplanAttempt[] = [];
  const evaluations: AutonomousCrossDomainReplanEvaluationProjection[] = [];
  const plannerEvaluations: AutonomousReplanPlanningEvaluationProjection[] = [];
  const learningEpisodeIds: string[] = [];
  const responseLearningEpisodeIds: string[] = [];
  const settlements: AutonomousCrossDomainLearningSettlement[] = [];
  const plannerSettlements: AutonomousPlanningQualitySettlement[] = [];
  let context = [...(options.context ?? [])];
  let routeOverride = options.routeOverride;
  let final: AutonomousCrossDomainDecisionCycleResult | null = null;

  let startAttempt = 0;
  if (persistence) {
    if (persistence.state.phase === "replan_handoff") {
      if (persistence.state.attempt >= maxReplans + 1) throw new ArgumentError("persisted cross-domain cycle replan handoff exceeds its attempt limit");
      routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
      const instruction = await rehydrateCycleInstruction(persistence, options.rehydrateReplanInstruction);
      const projection = persistence.state.evaluations[persistence.state.evaluations.length - 1] as unknown as AutonomousCrossDomainReplanEvaluationProjection;
      if (!projection || !projection.replan_requested) throw new ArgumentError("persisted cross-domain cycle handoff is missing a replan evaluation");
      const priorEvaluation: AutonomousCrossDomainReplanEvaluation = { ...projection, rewards: {}, replan_instruction: instruction };
      const nextContext = await crossDomainReplanContextChunk(persistence.state.attempt + 1, persistence.state.route_digest!, persistence.state.outcome_digest!, priorEvaluation);
      context = [...context, nextContext];
      startAttempt = persistence.state.attempt;
      attempts.push(...persistedCrossAttempts(persistence.state));
      evaluations.push(...persistence.state.evaluations as unknown as AutonomousCrossDomainReplanEvaluationProjection[]);
      learningEpisodeIds.push(...persistence.state.learning_episode_ids);
      responseLearningEpisodeIds.push(...(persistence.state.response_learning_episode_ids ?? []));
    } else {
      startAttempt = persistence.state.attempt - 1;
      if (persistence.state.phase === "execution_pending" && persistence.state.route_digest) routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
      if (persistence.state.phase === "evaluation_pending" || persistence.state.phase === "settlement_pending") routeOverride = await rehydrateCycleRoute(persistence, options.rehydrateRoute);
      attempts.push(...persistedCrossAttempts(persistence.state));
      evaluations.push(...persistence.state.evaluations as unknown as AutonomousCrossDomainReplanEvaluationProjection[]);
      learningEpisodeIds.push(...persistence.state.learning_episode_ids);
      responseLearningEpisodeIds.push(...(persistence.state.response_learning_episode_ids ?? []));
    }
  }

  for (let attempt = startAttempt; attempt <= maxReplans; attempt += 1) {
    let cycle: AutonomousCrossDomainDecisionCycleResult;
    const persistedPhase = persistence?.state.attempt === attempt + 1 ? persistence.state.phase : null;
    try {
      if (persistence && (persistedPhase === "evaluation_pending" || persistedPhase === "settlement_pending")) {
        if (!options.rehydrateRun) throw new ArgumentError("restart resume requires rehydrateRun for the persisted cross-domain provider outcome");
        const run = await options.rehydrateRun(rehydrationContext(persistence));
        const outcomeDigest = await crossDomainReplanOutcomeDigest(run);
        if (outcomeDigest !== persistence.state.outcome_digest || run.route.route_digest !== persistence.state.route_digest) throw new ArgumentError("rehydrated cross-domain run does not match the persisted outcome or route digest");
        cycle = rehydratedCrossDomainCycle(routeOverride!, run);
      } else {
        if (persistence) await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "execution_pending", route_digest: routeOverride?.route_digest ?? null, plan_refinement_digest: persistence.state.plan_refinement_digest, outcome_digest: null, evaluation_digest: null, replan_instruction_digest: null, terminal_status: null });
        cycle = await runAutonomousCrossDomainDecisionCycle(agent, task, {
          ...options,
          semanticRouting: attempt === 0 ? options.semanticRouting : undefined,
          routeOverride,
          context,
          costBudget,
          maxTotalCostUnits: undefined,
          executionAttempt: attempt + 1,
          executionLifecycle: "observe_only",
          learning: undefined,
          memory: attempt === 0 ? options.memory : undefined,
          cycleId: undefined,
          decisionStateStore: undefined,
          rehydrateRoute: undefined,
          rehydrateRun: undefined,
          rehydrateEvaluation: undefined,
          rehydratePlanningEvaluation: undefined,
          rehydrateResult: undefined,
        });
      }
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    final = cycle;
    const run = cycle.run;
    const planRefinementDigest = await cyclePlanRefinementDigest(cycle);
    let outcomeDigest: string | null = null;
    try {
      outcomeDigest = run ? await crossDomainReplanOutcomeDigest(run) : null;
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    const terminalRun = run && (run.status === "completed" || run.status === "children_completed" || run.status === "children_partial");
    if (persistence && terminalRun && run && persistedPhase !== "evaluation_pending" && persistedPhase !== "settlement_pending") {
      await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "evaluation_pending", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: outcomeDigest, evaluation_digest: null, replan_instruction_digest: null, terminal_status: null, attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, run.status, cycle.route.route_digest, null, outcomeDigest, null, [], null, planRefinementDigest)) });
    }
    if (cycle.status === "plan_review_required") {
      const attemptRecord = { attempt: attempt + 1, status: cycle.status, run_status: null, route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: null, evaluation_digest: null, evaluation: null, learning_episode_ids: [], trajectory_id: null } satisfies AutonomousCrossDomainReplanAttempt;
      upsertResultAttempt(attempts, attemptRecord);
      if (persistence) await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "execution_pending", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: null, evaluation_digest: null, replan_instruction_digest: null, terminal_status: null, attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, null, cycle.route.route_digest, null, null, null, [], null, planRefinementDigest)) });
      return crossDomainReplanResult("plan_review_required", final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements, responseLearningEpisodeIds);
    }
    if (!terminalRun || !run) {
      try {
        await options.execution?.checkpoint({ status: cycle.status, reason: `cross_domain_replan_${cycle.status}` });
      } catch (error) {
        await failExecutionIfActive(options.execution, error);
        throw error;
      }
      const attemptRecord = { attempt: attempt + 1, status: cycle.status, run_status: run?.status ?? null, route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: outcomeDigest, evaluation_digest: null, evaluation: null, learning_episode_ids: [], trajectory_id: null } satisfies AutonomousCrossDomainReplanAttempt;
      upsertResultAttempt(attempts, attemptRecord);
      if (persistence) await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "terminal", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: outcomeDigest, evaluation_digest: null, replan_instruction_digest: null, terminal_status: cycle.status, attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, run?.status ?? null, cycle.route.route_digest, null, outcomeDigest, null, [], null, planRefinementDigest)) });
      return crossDomainReplanResult(cycle.status, final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements, responseLearningEpisodeIds);
    }

    let runForEvaluation = run;
    let pendingEpisodeIds: string[] = [];
    let pendingResponseEpisodeIds: string[] = [];
    let trajectoryId: string | null = null;
    try {
      if (options.learning) {
        runForEvaluation = await prepareCrossDomainReplanEpisodes(options.learning.controller, run, episodePrefix!, attempt + 1);
        pendingEpisodeIds = [...runForEvaluation.learning_episode_ids];
        pendingResponseEpisodeIds = [...(runForEvaluation.response_learning_episode_ids ?? [])];
        trajectoryId = boundedReplanIdentifier("cross-domain replan trajectory", `${trajectoryIdPrefix}:${run.route.task_digest}:attempt-${attempt + 1}`);
      }
      const resumedSettlement = persistedPhase === "settlement_pending";
      const evaluation = resumedSettlement
        ? normalizeCrossDomainReplanEvaluation(await (options.rehydrateEvaluation ? options.rehydrateEvaluation(rehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydrateEvaluation after a cross-domain settlement interruption"))), pendingEpisodeIds)
        : normalizeCrossDomainReplanEvaluation(await options.evaluate(runForEvaluation), pendingEpisodeIds);
      const projection = await crossDomainReplanEvaluationProjection(evaluation);
      const executionEvaluationDigest = await digestJson(projection);
      const plannerPlan = cycle.plan_refinement ?? options.acceptedCrossDomainPlanRefinement ?? null;
      const plannerEligible = Boolean(options.evaluatePlanning && plannerPlan?.status === "completed");
      let plannerEvaluation: AutonomousEvaluatorRewardInput | null = null;
      let plannerSettlement: AutonomousPlanningQualitySettlement | null = null;
      if (plannerEligible) {
        const reward = resumedSettlement
          ? await (options.rehydratePlanningEvaluation ? options.rehydratePlanningEvaluation(rehydrationContext(persistence!)) : Promise.reject(new ArgumentError("restart resume requires rehydratePlanningEvaluation after a cross-domain planner settlement interruption")))
          : await options.evaluatePlanning!(plannerPlan!);
        plannerEvaluation = projectCycleReward(reward);
        plannerEvaluations.push({
          evaluator_id: plannerEvaluation.evaluator_id,
          evaluator_version: plannerEvaluation.evaluator_version,
          reward: plannerEvaluation.reward,
          passed: plannerEvaluation.passed,
          failed: plannerEvaluation.failed ?? !plannerEvaluation.passed,
          feedback_digest: plannerEvaluation.feedback_digest ?? null,
          failure_class: plannerEvaluation.failure_class ?? null,
          evidence_digest: plannerEvaluation.evidence_digest ?? null,
        });
      }
      const evaluationDigest = plannerEvaluation
        ? await digestJson({ execution_evaluation_digest: executionEvaluationDigest, planning_evaluation_digest: await decisionEvaluationDigest(plannerEvaluation) })
        : executionEvaluationDigest;
      if (resumedSettlement && evaluationDigest !== persistence?.state.evaluation_digest) throw new ArgumentError("rehydrated cross-domain evaluator packet does not match the persisted evaluation digest");
      if (persistence && !resumedSettlement) {
        const persistedAttempt = cycleAttemptState(attempt + 1, cycle.status, run.status, cycle.route.route_digest, null, outcomeDigest, evaluationDigest, pendingEpisodeIds, trajectoryId, planRefinementDigest);
        await commitCyclePersistence(persistence, { attempt: attempt + 1, phase: "settlement_pending", route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: outcomeDigest, evaluation_digest: evaluationDigest, replan_instruction_digest: projection.replan_instruction_digest, evaluations: [...persistence.state.evaluations, projection], attempts: upsertCycleAttempt(persistence.state.attempts, persistedAttempt), learning_episode_ids: [...new Set([...persistence.state.learning_episode_ids, ...pendingEpisodeIds])], response_learning_episode_ids: [...new Set([...(persistence.state.response_learning_episode_ids ?? []), ...pendingResponseEpisodeIds])] });
      }
      if (!resumedSettlement) await options.execution?.recordEvaluation({ evaluatorId: evaluation.evaluator_id, evaluatorVersion: evaluation.evaluator_version, reward: evaluation.reward, passed: evaluation.passed, evaluationDigest: executionEvaluationDigest, failureClass: evaluation.failure_class });
      let settlement: AutonomousCrossDomainLearningSettlement | null = null;
      if (options.learning) {
        settlement = await options.learning.controller.settleCrossDomain(runForEvaluation, evaluation.rewards, { trajectoryId: trajectoryId!, discount: options.learning.discount, remote: options.learning.remote, outbox: options.learning.outbox });
        settlements.push(settlement);
        for (const episodeId of pendingEpisodeIds) if (!learningEpisodeIds.includes(episodeId)) learningEpisodeIds.push(episodeId);
        for (const episodeId of pendingResponseEpisodeIds) if (!responseLearningEpisodeIds.includes(episodeId)) responseLearningEpisodeIds.push(episodeId);
        if (attempt === 0 && options.memory && cycle.memory) {
          const settlementItems = settlement.trajectory.settlements;
          for (let index = 0; index < cycle.memory.recorded_episode_ids.length; index += 1) {
            const memoryEpisodeId = cycle.memory.recorded_episode_ids[index];
            const learningEpisodeId = pendingEpisodeIds[index];
            if (!memoryEpisodeId || !learningEpisodeId) continue;
            const settlementItem = settlementItems.find((item) => item.episode.episode_id === learningEpisodeId);
            if (!settlementItem) continue;
            await recordMemoryEvaluation(options.memory, memoryEpisodeId, settlementItem.assessment);
            cycle.memory.evaluation_recorded_episode_ids.push(memoryEpisodeId);
          }
        }
      }
      if (plannerEvaluation && plannerPlan) {
        plannerSettlement = await options.learning!.controller.settlePlanningQuality(plannerPlan, {
          domain: cycle.route.primary_domain ?? "cross_domain",
          capability: options.capability,
          riskClass: "cross_domain_integration",
          taskFamily: "cross_domain_synthesis",
          evaluator: plannerEvaluation,
          remote: options.learning?.remote,
        });
        plannerSettlements.push(plannerSettlement);
      }
      final = { ...cycle, run: runForEvaluation, learning_episode_ids: pendingEpisodeIds, response_learning_episode_ids: pendingResponseEpisodeIds, evaluation: settlement ? projectedEvaluations(settlement) : null, settlement, response_settlements: settlement?.response_settlements ?? [], planner_evaluation: plannerEvaluation, planner_settlement: plannerSettlement };
      if (resumedSettlement && evaluations.length > 0) evaluations[evaluations.length - 1] = projection;
      else evaluations.push(projection);
      upsertResultAttempt(attempts, { attempt: attempt + 1, status: cycle.status, run_status: run.status, route_digest: cycle.route.route_digest, plan_refinement_digest: planRefinementDigest, outcome_digest: outcomeDigest, evaluation_digest: evaluationDigest, evaluation: projection, learning_episode_ids: pendingEpisodeIds, trajectory_id: trajectoryId });

      const shouldReplan = evaluation.replan_requested && attempt < maxReplans;
      if (persistence) {
        const settlementDigest = settlement ? await digestJson(settlement) : null;
        const stateSettlementDigests = settlementDigest && !persistence.state.settlement_digests.includes(settlementDigest)
          ? [...persistence.state.settlement_digests, settlementDigest]
          : [...persistence.state.settlement_digests];
        const plannerSettlementDigest = plannerSettlement?.status === "settled" ? await digestJson(plannerSettlement) : null;
        if (plannerSettlementDigest && !stateSettlementDigests.includes(plannerSettlementDigest)) stateSettlementDigests.push(plannerSettlementDigest);
        await commitCyclePersistence(persistence, {
          attempt: attempt + 1,
          phase: shouldReplan ? "replan_handoff" : "terminal",
          route_digest: cycle.route.route_digest,
          plan_refinement_digest: planRefinementDigest,
          outcome_digest: outcomeDigest,
          evaluation_digest: evaluationDigest,
          replan_instruction_digest: shouldReplan ? projection.replan_instruction_digest : null,
          terminal_status: shouldReplan ? null : (evaluation.replan_requested ? "replan_limit_reached" : (evaluation.passed ? "completed" : "completed_without_replan")),
          attempts: upsertCycleAttempt(persistence.state.attempts, cycleAttemptState(attempt + 1, cycle.status, run.status, cycle.route.route_digest, null, outcomeDigest, evaluationDigest, pendingEpisodeIds, trajectoryId, planRefinementDigest)),
          learning_episode_ids: [...new Set([...persistence.state.learning_episode_ids, ...pendingEpisodeIds])],
          response_learning_episode_ids: [...new Set([...(persistence.state.response_learning_episode_ids ?? []), ...pendingResponseEpisodeIds])],
          settlement_digests: stateSettlementDigests,
          trajectory_ids: trajectoryId && !persistence.state.trajectory_ids.includes(trajectoryId) ? [...persistence.state.trajectory_ids, trajectoryId] : [...persistence.state.trajectory_ids],
        });
      }

      if (!evaluation.replan_requested) {
        await options.execution?.complete(evaluation.passed ? "completed" : "completed_without_replan");
        return crossDomainReplanResult(evaluation.passed ? "completed" : "completed_without_replan", final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements, responseLearningEpisodeIds);
      }
      if (attempt >= maxReplans) {
        await options.execution?.complete("replan_limit_reached");
        return crossDomainReplanResult("replan_limit_reached", final, attempts, evaluations, plannerEvaluations, learningEpisodeIds, settlements, plannerSettlements, responseLearningEpisodeIds);
      }
      const nextContext = await crossDomainReplanContextChunk(attempt + 2, cycle.route.route_digest, outcomeDigest!, evaluation);
      await options.execution?.replan({ instructionDigest: projection.replan_instruction_digest, attempt: attempt + 2, reason: "cross_domain_evaluator_requested" });
      context = [...context, nextContext];
      routeOverride = cycle.route;
      if (persistence) await commitCyclePersistence(persistence, { context_digests: [...persistence.state.context_digests, await replanContextDigest(nextContext)] });
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
  }
  throw new ArgumentError("cross-domain replan cycle exited without a terminal result");
}
