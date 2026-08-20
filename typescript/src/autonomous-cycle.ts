import { ArgumentError } from "./errors.js";
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
    approveProviderCall: options.approveProviderCall,
    approveEffects: options.approveEffects,
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
      signal: options.signal,
      observer: options.observer,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") {
      return reviewResult(semanticRoute.status === "approval_required" ? "approval_required" : semanticRoute.status, route, semanticRoute);
    }
  } else if (options.routeOverride) {
    route = options.routeOverride;
  } else {
    route = await agent.route(task, { domain: options.domain, hints: options.hints, allowCrossDomain: options.allowCrossDomain });
  }

  if (route.abstained || !route.primary_domain || route.cross_domain || route.selected_domains.length !== 1) return reviewResult("route_review_required", route, semanticRoute);
  const recalledMemory = await recallMemory(options.memory, route);
  const run = await agent.run(task, runOptions(options, route, recalledMemory.promptChunk));
  const cycleStatus = cycleStatusForRun(run.status);
  if (cycleStatus !== "completed") return { ...reviewResult(cycleStatus, route, semanticRoute), run, memory: recalledMemory.projection };

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
    approveProviderCall: options.approveProviderCall,
    approveEffects: options.approveEffects,
    signal: options.signal,
    observer: options.observer,
    subtasks: options.subtasks,
    allowPartial: options.allowPartial,
    synthesize: options.synthesize,
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
      signal: options.signal,
      observer: options.observer,
    });
    route = semanticRoute.route;
    if (semanticRoute.status !== "completed") return crossReviewResult(semanticRoute.status === "approval_required" ? "approval_required" : semanticRoute.status, route, semanticRoute);
  } else if (options.routeOverride) {
    route = options.routeOverride;
  } else {
    route = await agent.route(task, { hints: options.hints, allowCrossDomain: options.allowCrossDomain ?? true });
  }
  if (route.abstained || !route.cross_domain || route.selected_domains.length < 2) return crossReviewResult("route_review_required", route, semanticRoute);

  const recalledMemory = await recallMemory(options.memory, route);
  const run = await agent.runCrossDomain(task, crossRunOptions(options, route, recalledMemory.promptChunk));
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
}
