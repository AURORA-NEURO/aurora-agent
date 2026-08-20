import { ArgumentError } from "./errors.js";
import {
  type AutonomousAgent,
  type AutonomousCrossDomainRunOptions,
  type AutonomousCrossDomainRunResult,
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
import type { BrainEvaluatorAssessment } from "./types.js";

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

export interface AutonomousDecisionCycleOptions extends AutonomousRunOptions {
  semanticRouting?: AutonomousDecisionCycleSemanticOptions;
  learning?: AutonomousDecisionCycleLearningOptions;
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
  retention: "provider_response_local; value_only_evaluation_and_learning_projection";
  authorization: "routing_and_provider_invocation_require_separate_explicit_approval";
}

const RETENTION = "provider_response_local; value_only_evaluation_and_learning_projection" as const;
const AUTHORIZATION = "routing_and_provider_invocation_require_separate_explicit_approval" as const;

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
    retention: RETENTION,
    authorization: AUTHORIZATION,
  };
}

function runOptions(options: AutonomousDecisionCycleOptions, route: AutonomousRouteProposal): AutonomousRunOptions {
  return {
    domain: options.domain,
    routeOverride: route,
    capability: options.capability,
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context: options.context,
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
  const run = await agent.run(task, runOptions(options, route));
  const cycleStatus = cycleStatusForRun(run.status);
  if (cycleStatus !== "completed") return { ...reviewResult(cycleStatus, route, semanticRoute), run };

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

  return {
    schema: AUTONOMOUS_DECISION_CYCLE_SCHEMA,
    status: "completed",
    route,
    semantic_route: semanticRoute,
    run,
    learning_episode_id: learningEpisodeId,
    evaluation: settlement?.assessment ?? null,
    settlement,
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
    retention: CROSS_RETENTION,
    authorization: CROSS_AUTHORIZATION,
  };
}

function crossRunOptions(options: AutonomousCrossDomainDecisionCycleOptions, route: AutonomousRouteProposal): AutonomousCrossDomainRunOptions {
  return {
    routeOverride: route,
    capability: options.capability,
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context: options.context,
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

  const run = await agent.runCrossDomain(task, crossRunOptions(options, route));
  let settlement: AutonomousCrossDomainLearningSettlement | null = null;
  if (options.learning?.evaluate && run.learning_episode_ids.length > 0) {
    const rewards = await options.learning.evaluate(run);
    settlement = await options.learning.controller.settleCrossDomain(run, rewards, {
      trajectoryId: options.learning.trajectoryId,
      discount: options.learning.discount,
      remote: options.learning.remote,
    });
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
    retention: CROSS_RETENTION,
    authorization: CROSS_AUTHORIZATION,
  };
}
