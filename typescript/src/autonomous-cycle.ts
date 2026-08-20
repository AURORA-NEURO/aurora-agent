import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import {
  type AutonomousAgent,
  type AutonomousCrossDomainRunOptions,
  type AutonomousCrossDomainRunResult,
  type AutonomousPromptChunk,
  type AutonomousRunOptions,
  type AutonomousRunResult,
  type AutonomousRouteProposal,
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
} from "./autonomous-learning.js";
import type {
  AutonomousEpisodicMemoryStore,
  AutonomousMemoryEpisode,
  AutonomousMemoryEvaluationInput,
  AutonomousMemoryQuery,
} from "./autonomous-memory.js";
import { digestJson } from "./tooling.js";
import type { BrainEvaluatorAssessment, JsonObject } from "./types.js";

export const AUTONOMOUS_DECISION_CYCLE_SCHEMA = "bioprism-typescript-autonomous-decision-cycle/0.1" as const;

export type AutonomousDecisionCycleStatus =
  | "completed"
  | "approval_required"
  | "route_review_required"
  | "provider_abstained"
  | "provider_invalid"
  | "provider_disagreement";

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
}

export type AutonomousDecisionCycleEvaluator = (
  result: AutonomousRunResult,
) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;

export interface AutonomousDecisionCycleLearningOptions {
  controller: AutonomousLearningController;
  episodeId: string;
  evaluate?: AutonomousDecisionCycleEvaluator;
  remote?: boolean;
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

export interface AutonomousDecisionCycleOptions extends AutonomousRunOptions {
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  learning?: AutonomousDecisionCycleLearningOptions;
  memory?: AutonomousDecisionCycleMemoryOptions;
}

export interface AutonomousDecisionCycleResult {
  schema: typeof AUTONOMOUS_DECISION_CYCLE_SCHEMA;
  status: AutonomousDecisionCycleStatus;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  run: AutonomousRunResult | null;
  learning_episode_id: string | null;
  evaluation: BrainEvaluatorAssessment | null;
  settlement: AutonomousLearningSettlement | null;
  memory: AutonomousDecisionCycleMemoryProjection | null;
  retention: "provider_response_local; value_only_evaluation_and_learning_projection";
  authorization: "routing_and_provider_invocation_require_separate_explicit_approval";
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

async function recallMemory(memory: AutonomousDecisionCycleMemoryOptions | undefined, route: AutonomousRouteProposal): Promise<RecalledMemory> {
  if (!memory) return { episodes: [], projection: emptyMemoryProjection(), promptChunk: null };
  if (!memory.store || typeof memory.store.retrieve !== "function" || typeof memory.store.recordEpisode !== "function") throw new ArgumentError("autonomous cycle memory store is malformed");
  const query: AutonomousMemoryQuery = { ...(memory.query ?? {}) };
  if (query.domain === undefined && !route.cross_domain && route.primary_domain) query.domain = route.primary_domain;
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

async function memoryPacketForRun(memory: AutonomousDecisionCycleMemoryOptions, run: AutonomousRunResult, episodeId: string): Promise<AutonomousMemoryEpisode | null> {
  if (!run.blueprint || !run.selection?.selected_model) return null;
  const outcomeDigest = await digestJson({ status: run.status, route_digest: run.route.route_digest, selection: run.selection, response: run.response });
  await memory.store.recordEpisode({
    episode_id: episodeId,
    run_id: episodeId,
    result_kind: "autonomous_decision_cycle",
    status: run.status === "completed" ? "completed" : run.status === "approval_required" ? "approval_required" : run.status === "child_failed" || run.status === "cross_domain_partial" ? "partial" : "failed",
    task_digest: run.blueprint.task_digest,
    context: { domain: run.blueprint.domain_profile.domain, capability: run.blueprint.selection_context.capability, risk_class: run.blueprint.domain_profile.risk_class },
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
): AutonomousDecisionCycleResult {
  return {
    schema: AUTONOMOUS_DECISION_CYCLE_SCHEMA,
    status,
    route,
    semantic_route: semanticRoute,
    run: null,
    learning_episode_id: null,
    evaluation: null,
    settlement: null,
    memory: null,
    retention: RETENTION,
    authorization: AUTHORIZATION,
  };
}

function runOptions(options: AutonomousDecisionCycleOptions, route: AutonomousRouteProposal, memoryChunk: AutonomousPromptChunk | null): AutonomousRunOptions {
  return {
    domain: options.domain,
    routeOverride: route,
    capability: options.capability,
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context: withMemoryContext(options.context, memoryChunk),
    hints: options.hints,
    allowCrossDomain: options.allowCrossDomain,
    maxInputTokens: options.maxInputTokens,
    maxOutputTokens: options.maxOutputTokens,
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

  let route: AutonomousRouteProposal;
  let semanticRoute: AutonomousSemanticRouteResult | null = null;
  if (options.semanticRouting?.enabled) {
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
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers,
      signal: options.signal,
      observer: options.observer,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: semanticRoute.status, reason: `semantic_route_${semanticRoute.status}` });
      return reviewResult(semanticRoute.status === "approval_required" ? "approval_required" : semanticRoute.status, route, semanticRoute);
    }
  } else if (options.routeOverride) {
    route = options.routeOverride;
  } else {
    route = await agent.route(task, { domain: options.domain, hints: options.hints, allowCrossDomain: options.allowCrossDomain });
  }

  if (route.abstained || !route.primary_domain || route.cross_domain || route.selected_domains.length !== 1) {
    if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: "route_review_required", reason: "single_domain_route_review_required" });
    return reviewResult("route_review_required", route, semanticRoute);
  }
  const recalledMemory = await recallMemory(options.memory, route);
  let run: AutonomousRunResult;
  try {
    run = await agent.run(task, runOptions(options, route, recalledMemory.promptChunk));
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
  const cycleStatus = cycleStatusForRun(run.status);
  if (cycleStatus !== "completed") {
    if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: cycleStatus, reason: `run_${cycleStatus}` });
    return { ...reviewResult(cycleStatus, route, semanticRoute), run, memory: recalledMemory.projection };
  }

  try {
    let learningEpisodeId: string | null = null;
    let settlement: AutonomousLearningSettlement | null = null;
    if (options.learning) {
      const controller = options.learning.controller;
      if (!controller || typeof controller.prepareRun !== "function" || typeof controller.settleRun !== "function") throw new ArgumentError("decision cycle learning controller is malformed");
      const episode = await controller.prepareRun(run, { episodeId: options.learning.episodeId });
      learningEpisodeId = episode.episode_id;
      if (options.learning.evaluate) {
        const reward = await options.learning.evaluate(run);
        settlement = await controller.settleRun(episode.episode_id, reward, { remote: options.learning.remote });
      }
    }

    const memoryProjection = recalledMemory.projection;
    if (options.memory) {
      const memoryEpisodeId = options.memory.episodeId ?? `memory:${learningEpisodeId ?? `${run.blueprint!.task_digest}:${run.blueprint!.prompt.prompt_digest}`}`;
      const memoryEpisode = await memoryPacketForRun(options.memory, run, memoryEpisodeId);
      if (memoryEpisode) {
        memoryProjection.recorded_episode_ids.push(memoryEpisode.episode_id);
        if (settlement) {
          await recordMemoryEvaluation(options.memory, memoryEpisode.episode_id, settlement.assessment);
          memoryProjection.evaluation_recorded_episode_ids.push(memoryEpisode.episode_id);
        }
      }
    }

    if (options.executionLifecycle !== "observe_only") await options.execution?.complete("completed");

    return {
      schema: AUTONOMOUS_DECISION_CYCLE_SCHEMA,
      status: "completed",
      route,
      semantic_route: semanticRoute,
      run,
      learning_episode_id: learningEpisodeId,
      evaluation: settlement?.assessment ?? null,
      settlement,
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
export const AUTONOMOUS_REPLAN_MAX_REPLANS = 3;

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
  selection_digest: string | null;
  outcome_digest: string | null;
  evaluation_digest: string | null;
  evaluation: AutonomousReplanEvaluationProjection | null;
  learning_episode_id: string | null;
}

export interface AutonomousReplanCycleOptions extends Omit<AutonomousDecisionCycleOptions, "learning" | "memory"> {
  evaluate: AutonomousReplanEvaluator;
  /** Additional evaluator-requested attempts. The SDK caps this at three. */
  maxReplans?: number;
  learning?: AutonomousReplanLearningOptions;
}

export interface AutonomousReplanCycleResult {
  schema: typeof AUTONOMOUS_REPLAN_CYCLE_SCHEMA;
  status: AutonomousReplanCycleStatus;
  final: AutonomousDecisionCycleResult | null;
  attempts: AutonomousReplanAttempt[];
  replan_count: number;
  evaluations: AutonomousReplanEvaluationProjection[];
  learning_episode_ids: string[];
  settlements: AutonomousLearningSettlement[];
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
  learningEpisodeIds: string[],
  settlements: AutonomousLearningSettlement[],
): AutonomousReplanCycleResult {
  return {
    schema: AUTONOMOUS_REPLAN_CYCLE_SCHEMA,
    status,
    final,
    attempts,
    replan_count: Math.max(0, attempts.length - 1),
    evaluations,
    learning_episode_ids: learningEpisodeIds,
    settlements,
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
  if (options.learning && (!options.learning.controller || typeof options.learning.controller.prepareRun !== "function" || typeof options.learning.controller.settleRun !== "function")) throw new ArgumentError("replan learning controller is malformed");

  const attempts: AutonomousReplanAttempt[] = [];
  const evaluations: AutonomousReplanEvaluationProjection[] = [];
  const learningEpisodeIds: string[] = [];
  const settlements: AutonomousLearningSettlement[] = [];
  let context = [...(options.context ?? [])];
  let routeOverride = options.routeOverride;
  let final: AutonomousDecisionCycleResult | null = null;

  for (let attempt = 0; attempt <= maxReplans; attempt += 1) {
    let cycle: AutonomousDecisionCycleResult;
    try {
      cycle = await runAutonomousDecisionCycle(agent, task, {
        ...options,
        semanticRouting: attempt === 0 ? options.semanticRouting : undefined,
        routeOverride,
        context,
        executionAttempt: attempt + 1,
        executionLifecycle: "observe_only",
        learning: undefined,
        memory: undefined,
      });
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    final = cycle;
    const digests = await replanRunDigests(cycle.run);
    if (cycle.status !== "completed" || !cycle.run) {
      await options.execution?.checkpoint({ status: cycle.status, reason: `replan_cycle_${cycle.status}` });
      attempts.push({ attempt: attempt + 1, status: cycle.status, run_status: cycle.run?.status ?? null, route_digest: cycle.route.route_digest, selection_digest: digests.selection, outcome_digest: digests.outcome, evaluation_digest: null, evaluation: null, learning_episode_id: null });
      return replanResult(cycle.status, final, attempts, evaluations, learningEpisodeIds, settlements);
    }

    let evaluation: AutonomousReplanEvaluation;
    let projection: AutonomousReplanEvaluationProjection;
    let evaluationDigest: string;
    try {
      evaluation = normalizeReplanEvaluation(await options.evaluate(cycle.run));
      projection = await replanEvaluationProjection(evaluation);
      evaluationDigest = await digestJson(projection);
      await options.execution?.recordEvaluation({ evaluatorId: evaluation.evaluator_id, evaluatorVersion: evaluation.evaluator_version, reward: evaluation.reward, passed: evaluation.passed, evaluationDigest, failureClass: evaluation.failure_class });
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    let learningEpisodeId: string | null = null;
    try {
      if (options.learning) {
        learningEpisodeId = `${episodePrefix}:${cycle.run.blueprint!.task_digest}:attempt-${attempt + 1}`;
        const episode = await options.learning.controller.prepareRun(cycle.run, { episodeId: learningEpisodeId, runId: learningEpisodeId, stageId: `replan-${attempt + 1}` });
        const settlement = await options.learning.controller.settleRun(episode.episode_id, evaluation, { remote: options.learning.remote });
        learningEpisodeIds.push(episode.episode_id);
        settlements.push(settlement);
      }
    } catch (error) {
      await failExecutionIfActive(options.execution, error);
      throw error;
    }
    evaluations.push(projection);
    attempts.push({ attempt: attempt + 1, status: cycle.status, run_status: cycle.run.status, route_digest: cycle.route.route_digest, selection_digest: digests.selection, outcome_digest: digests.outcome, evaluation_digest: evaluationDigest, evaluation: projection, learning_episode_id: learningEpisodeId });

    if (!evaluation.replan_requested) {
      await options.execution?.complete(evaluation.passed ? "completed" : "completed_without_replan");
      return replanResult(evaluation.passed ? "completed" : "completed_without_replan", final, attempts, evaluations, learningEpisodeIds, settlements);
    }
    if (attempt >= maxReplans) {
      await options.execution?.complete("replan_limit_reached");
      return replanResult("replan_limit_reached", final, attempts, evaluations, learningEpisodeIds, settlements);
    }

    const nextContext = await replanContextChunk(attempt + 2, cycle.route.route_digest, digests.selection, digests.outcome, evaluation);
    await options.execution?.replan({ instructionDigest: projection.replan_instruction_digest, attempt: attempt + 2, reason: "evaluator_requested" });
    context = [...context, nextContext];
    routeOverride = cycle.route;
  }

  throw new ArgumentError("replan cycle exited without a terminal result");
}

export const AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA = "bioprism-typescript-autonomous-cross-domain-decision-cycle/0.1" as const;

export type AutonomousCrossDomainDecisionCycleStatus =
  | AutonomousDecisionCycleStatus
  | "children_completed"
  | "children_partial"
  | "child_failed";

export type AutonomousCrossDomainDecisionCycleEvaluator = (
  result: AutonomousCrossDomainRunResult,
) => Record<string, AutonomousEvaluatorRewardInput> | Promise<Record<string, AutonomousEvaluatorRewardInput>>;

export interface AutonomousCrossDomainDecisionCycleLearningOptions {
  controller: AutonomousLearningController;
  trajectoryId: string;
  discount?: number;
  evaluate?: AutonomousCrossDomainDecisionCycleEvaluator;
  remote?: boolean;
}

export interface AutonomousCrossDomainDecisionCycleOptions extends Omit<AutonomousCrossDomainRunOptions, "learning"> {
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  learning?: AutonomousCrossDomainDecisionCycleLearningOptions;
  memory?: AutonomousDecisionCycleMemoryOptions;
}

export interface AutonomousCrossDomainDecisionCycleResult {
  schema: typeof AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA;
  status: AutonomousCrossDomainDecisionCycleStatus;
  route: AutonomousRouteProposal;
  semantic_route: AutonomousSemanticRouteResult | null;
  run: AutonomousCrossDomainRunResult | null;
  learning_episode_ids: string[];
  evaluation: Record<string, BrainEvaluatorAssessment> | null;
  settlement: AutonomousCrossDomainLearningSettlement | null;
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
): AutonomousCrossDomainDecisionCycleResult {
  return {
    schema: AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA,
    status,
    route,
    semantic_route: semanticRoute,
    run: null,
    learning_episode_ids: [],
    evaluation: null,
    settlement: null,
    memory: null,
    retention: CROSS_RETENTION,
    authorization: CROSS_AUTHORIZATION,
  };
}

function crossRunOptions(options: AutonomousCrossDomainDecisionCycleOptions, route: AutonomousRouteProposal, memoryChunk: AutonomousPromptChunk | null): AutonomousCrossDomainRunOptions {
  return {
    routeOverride: route,
    capability: options.capability,
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context: withMemoryContext(options.context, memoryChunk),
    hints: options.hints,
    allowCrossDomain: options.allowCrossDomain,
    maxInputTokens: options.maxInputTokens,
    maxOutputTokens: options.maxOutputTokens,
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
    learning: options.learning?.controller,
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
  if (options.learning && (!options.learning.controller || typeof options.learning.controller.prepareCrossDomainTrajectory !== "function" || typeof options.learning.controller.settleCrossDomain !== "function")) throw new ArgumentError("cross-domain decision cycle learning controller is malformed");

  let route: AutonomousRouteProposal;
  let semanticRoute: AutonomousSemanticRouteResult | null = null;
  if (options.semanticRouting?.enabled) {
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
      execution: options.execution,
      executionAttempt: options.executionAttempt,
      maxProviderFailovers: options.semanticRouting.maxProviderFailovers,
      signal: options.signal,
      observer: options.observer,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: semanticRoute.status, reason: `semantic_route_${semanticRoute.status}` });
      return crossReviewResult(semanticRoute.status === "approval_required" ? "approval_required" : semanticRoute.status, route, semanticRoute);
    }
  } else if (options.routeOverride) {
    route = options.routeOverride;
  } else {
    route = await agent.route(task, { hints: options.hints, allowCrossDomain: options.allowCrossDomain ?? true });
  }
  if (route.abstained || !route.cross_domain || route.selected_domains.length < 2) {
    if (options.executionLifecycle !== "observe_only") await options.execution?.checkpoint({ status: "route_review_required", reason: "cross_domain_route_review_required" });
    return crossReviewResult("route_review_required", route, semanticRoute);
  }

  const recalledMemory = await recallMemory(options.memory, route);
  let run: AutonomousCrossDomainRunResult;
  try {
    run = await agent.runCrossDomain(task, crossRunOptions(options, route, recalledMemory.promptChunk));
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
  try {
    let settlement: AutonomousCrossDomainLearningSettlement | null = null;
    if (options.learning?.evaluate && run.learning_episode_ids.length > 0) {
      const rewards = await options.learning.evaluate(run);
      settlement = await options.learning.controller.settleCrossDomain(run, rewards, {
        trajectoryId: options.learning.trajectoryId,
        discount: options.learning.discount,
        remote: options.learning.remote,
      });
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
        const memoryEpisode = await memoryPacketForRun(options.memory, childRun, memoryEpisodeId);
        if (!memoryEpisode) continue;
        memoryProjection.recorded_episode_ids.push(memoryEpisode.episode_id);
        const settlementItem = settlement?.trajectory.settlements.find((item) => item.episode.episode_id === learningEpisodeId);
        if (settlementItem) {
          await recordMemoryEvaluation(options.memory, memoryEpisode.episode_id, settlementItem.assessment);
          memoryProjection.evaluation_recorded_episode_ids.push(memoryEpisode.episode_id);
        }
      }
    }
    if (options.executionLifecycle !== "observe_only") {
      if (run.status === "completed" || run.status === "children_completed" || run.status === "children_partial") await options.execution?.complete(run.status);
      else await options.execution?.checkpoint({ status: run.status, reason: `cross_domain_${run.status}` });
    }
    return {
      schema: AUTONOMOUS_CROSS_DOMAIN_DECISION_CYCLE_SCHEMA,
      status: run.status,
      route,
      semantic_route: semanticRoute,
      run,
      learning_episode_ids: [...run.learning_episode_ids],
      evaluation: settlement ? projectedEvaluations(settlement) : null,
      settlement,
      memory: memoryProjection,
      retention: CROSS_RETENTION,
      authorization: CROSS_AUTHORIZATION,
    };
  } catch (error) {
    if (options.executionLifecycle !== "observe_only") await failExecutionIfActive(options.execution, error);
    throw error;
  }
}
