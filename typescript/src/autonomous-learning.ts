import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type { ApiClient } from "./client.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  builtinAutonomousDomainProfiles,
  type AutonomousAgent,
  type AutonomousDomainName,
  type AutonomousRunResult,
} from "./autonomous.js";
import type { AutonomousWorkflowExecutionResult } from "./workflow-execution.js";
import { digestJson } from "./tooling.js";
import type {
  BrainBanditState,
  BrainEvaluatorAssessment,
  BrainLearningEvidence,
  BrainOutcomeRecordResult,
  BrainRunIdentity,
  JsonObject,
  RestToolResponse,
} from "./types.js";

export const AUTONOMOUS_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-workflow-evaluation/0.1" as const;
export const AUTONOMOUS_LEARNING_EPISODE_SCHEMA = "bioprism-typescript-autonomous-learning-episode/0.1" as const;
export const AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA = "bioprism-typescript-autonomous-learning-trajectory/0.1" as const;
export const AUTONOMOUS_LEARNING_MAX_STAGES = 64;
export const AUTONOMOUS_LEARNING_MAX_TRAJECTORY_STEPS = 32;

type Digest = string;

const PRIVATE_RETENTION = "value_only;task_prompt_response_credentials_and_evidence_not_retained" as const;

export interface AutonomousDomainEvaluatorProfile extends JsonObject {
  schema: "bioprism-typescript-autonomous-domain-evaluator/0.1";
  domain: AutonomousDomainName;
  evaluator_id: string;
  evaluator_version: string;
  required_signals: string[];
  signal_weights: Record<string, number>;
  pass_threshold: number;
  execution: "caller_declared_signal_scoring_only";
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousStageSignalEvidence extends JsonObject {
  stage_id: string;
  signals: Record<string, number>;
  evidence_digest?: Digest | null;
}

export interface AutonomousWorkflowEvaluationInput extends JsonObject {
  stages: AutonomousStageSignalEvidence[];
  evidence_digest?: Digest | null;
}

export interface AutonomousWorkflowEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATION_SCHEMA;
  evaluator_id: string;
  evaluator_version: string;
  domain: AutonomousDomainName;
  task_digest: Digest;
  workflow_digest: Digest;
  plan_digest: Digest;
  execution_status: AutonomousWorkflowExecutionResult["status"];
  stage_scores: Record<string, number>;
  signal_scores: Record<string, number>;
  missing_signals: string[];
  rejected_signals: string[];
  required_signals: string[];
  pass_threshold: number;
  reward: number;
  passed: boolean;
  status: "passed" | "failed" | "incomplete";
  evidence_digest: Digest;
  evaluation_digest: Digest;
  evaluator_authority: "caller_declared_signal_scoring_only";
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousLearningEpisode extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_EPISODE_SCHEMA;
  episode_id: string;
  run: BrainRunIdentity;
  domain: AutonomousDomainName;
  capability: string;
  workflow_id: string;
  workflow_digest: Digest;
  status: "pending" | "settled";
  settlement: AutonomousLearningSettlementMetadata | null;
  episode_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningSettlementMetadata extends JsonObject {
  evaluation_digest: Digest | null;
  reward: number;
  credited_reward: number;
  next_generation: number;
  settlement_digest: Digest;
  settled_at: number;
}

export interface AutonomousLearningEpisodeStore {
  load(episodeId: string): Promise<AutonomousLearningEpisode | null> | AutonomousLearningEpisode | null;
  save(episode: AutonomousLearningEpisode): Promise<void> | void;
  markSettled(episodeId: string, settlement: AutonomousLearningSettlementMetadata): Promise<AutonomousLearningEpisode> | AutonomousLearningEpisode;
  pending(limit?: number): Promise<AutonomousLearningEpisode[]> | AutonomousLearningEpisode[];
}

export interface AutonomousLearningTrajectoryStep extends JsonObject {
  index: number;
  episode_id: string;
  arm_id: string;
  run_digest: Digest;
  raw_reward: number | null;
  credited_reward: number | null;
}

export interface AutonomousLearningTrajectory extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA;
  trajectory_id: string;
  discount: number;
  steps: AutonomousLearningTrajectoryStep[];
  status: "pending" | "settled";
  trajectory_digest: Digest;
  settlement_digest: Digest | null;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningTrajectoryStore {
  load(trajectoryId: string): Promise<AutonomousLearningTrajectory | null> | AutonomousLearningTrajectory | null;
  save(trajectory: AutonomousLearningTrajectory): Promise<void> | void;
  markSettled(trajectoryId: string, settlementDigest: Digest): Promise<AutonomousLearningTrajectory> | AutonomousLearningTrajectory;
}

export interface AutonomousEvaluatorRewardInput extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: Digest | null;
  failure_class?: string | null;
  evidence_digest?: Digest | null;
}

export interface AutonomousLearningSettlement extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_EPISODE_SCHEMA;
  episode: AutonomousLearningEpisode;
  assessment: BrainEvaluatorAssessment;
  next_state: BrainBanditState;
  learning_evidence: BrainLearningEvidence | null;
  remote: boolean;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousTrajectorySettlement extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA;
  trajectory: AutonomousLearningTrajectory;
  settlements: AutonomousLearningSettlement[];
  return_to_go: Record<string, number>;
  retention: typeof PRIVATE_RETENTION;
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedReward(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return value;
}

function boundedThreshold(name: string, value: unknown): number {
  const threshold = boundedReward(name, value);
  if (threshold <= 0 || threshold > 1) throw new ArgumentError(`${name} must be within (0, 1]`);
  return threshold;
}

function boundedGeneration(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new ProviderRuntimeError("brain learning response returned an invalid generation");
  return value as number;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function assertStageEvidence(value: unknown): asserts value is AutonomousStageSignalEvidence {
  if (!isObject(value) || typeof value.stage_id !== "string" || !value.stage_id.trim() || !isObject(value.signals)) throw new ArgumentError("stage evaluator evidence must contain stage_id and signals");
  boundedIdentifier("stage evaluator stage_id", value.stage_id);
  const signalEntries = Object.entries(value.signals);
  if (signalEntries.length > 64) throw new ArgumentError("stage evaluator evidence contains too many signals");
  for (const [signal, score] of signalEntries) {
    boundedIdentifier("stage evaluator signal", signal);
    boundedReward(`stage evaluator signal ${signal}`, score);
  }
  if (value.evidence_digest !== undefined) boundedDigest("stage evaluator evidence_digest", value.evidence_digest, true);
}

function assertRewardInput(value: AutonomousEvaluatorRewardInput): void {
  if (!isObject(value)) throw new ArgumentError("evaluator reward must be an object");
  boundedIdentifier("evaluator_id", value.evaluator_id);
  boundedIdentifier("evaluator_version", value.evaluator_version);
  boundedReward("evaluator reward", value.reward);
  if (typeof value.passed !== "boolean") throw new ArgumentError("evaluator passed must be boolean");
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new ArgumentError("evaluator failed must be boolean");
  if (value.feedback_digest !== undefined) boundedDigest("evaluator feedback_digest", value.feedback_digest, true);
  if (value.evidence_digest !== undefined) boundedDigest("evaluator evidence_digest", value.evidence_digest, true);
  if (value.failure_class !== undefined && value.failure_class !== null) boundedIdentifier("evaluator failure_class", value.failure_class);
}

function requiredSignalsFor(execution: AutonomousWorkflowExecutionResult): string[] {
  const blueprint = execution.blueprint;
  if (!blueprint) throw new ArgumentError("workflow evaluation requires an executed blueprint");
  return [...new Set(blueprint.workflow.stages.flatMap((stage) => stage.evaluator_signals.map((signal) => `${stage.id}/${signal}`)))].sort();
}

/** Return one reviewed, caller-declared evaluator profile for every built-in domain. */
export async function builtinAutonomousDomainEvaluatorProfiles(): Promise<AutonomousDomainEvaluatorProfile[]> {
  const profiles = await builtinAutonomousDomainProfiles();
  return profiles.map((profile) => {
    const requiredSignals = [...new Set(profile.workflow.stages.flatMap((stage) => stage.evaluator_signals))].sort();
    const signalWeights = Object.fromEntries(requiredSignals.map((signal) => [signal, 1]));
    return {
      schema: "bioprism-typescript-autonomous-domain-evaluator/0.1",
      domain: profile.domain,
      evaluator_id: `typescript-${profile.domain}-workflow-evaluator`,
      evaluator_version: "0.1",
      required_signals: requiredSignals,
      signal_weights: signalWeights,
      pass_threshold: 0.8,
      execution: "caller_declared_signal_scoring_only",
      retention: PRIVATE_RETENTION,
    };
  });
}

/** Score only explicit evaluator signals; provider completion alone never creates reward. */
export class AutonomousWorkflowEvaluator {
  readonly evaluatorVersion: string;
  readonly passThreshold: number;
  readonly signalWeights: Readonly<Record<string, number>>;
  readonly evaluatorId?: string;

  constructor(options: { evaluatorId?: string; evaluatorVersion?: string; passThreshold?: number; signalWeights?: Readonly<Record<string, number>> } = {}) {
    this.evaluatorId = options.evaluatorId === undefined ? undefined : boundedIdentifier("evaluatorId", options.evaluatorId);
    this.evaluatorVersion = boundedIdentifier("evaluatorVersion", options.evaluatorVersion ?? "0.1");
    this.passThreshold = boundedThreshold("passThreshold", options.passThreshold ?? 0.8);
    this.signalWeights = { ...(options.signalWeights ?? {}) };
    for (const [signal, weight] of Object.entries(this.signalWeights)) {
      boundedIdentifier("evaluator signal", signal);
      if (typeof weight !== "number" || !Number.isFinite(weight) || weight <= 0) throw new ArgumentError(`evaluator signal weight ${signal} must be positive`);
    }
  }

  async evaluate(execution: AutonomousWorkflowExecutionResult, input: AutonomousWorkflowEvaluationInput): Promise<AutonomousWorkflowEvaluation> {
    if (!isObject(input) || !Array.isArray(input.stages)) throw new ArgumentError("workflow evaluator input must contain stages");
    if (input.stages.length > AUTONOMOUS_LEARNING_MAX_STAGES) throw new ArgumentError("workflow evaluator input contains too many stages");
    for (const evidence of input.stages) assertStageEvidence(evidence);
    if (input.evidence_digest !== undefined) boundedDigest("workflow evaluator evidence_digest", input.evidence_digest, true);
    const blueprint = execution.blueprint;
    if (!blueprint) throw new ArgumentError("workflow evaluation requires a blueprint");
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(blueprint.domain_profile.domain)) throw new ArgumentError("workflow evaluation blueprint has an unsupported domain");
    const stageById = new Map(blueprint.workflow.stages.map((stage) => [stage.id, stage]));
    const evidenceById = new Map<string, AutonomousStageSignalEvidence>();
    for (const evidence of input.stages) {
      if (!stageById.has(evidence.stage_id)) throw new ArgumentError(`workflow evaluator received an unknown stage ${evidence.stage_id}`);
      if (evidenceById.has(evidence.stage_id)) throw new ArgumentError(`workflow evaluator received duplicate stage ${evidence.stage_id}`);
      evidenceById.set(evidence.stage_id, evidence);
    }
    const stageScores: Record<string, number> = {};
    const signalScores: Record<string, number> = {};
    const missingSignals: string[] = [];
    const rejectedSignals: string[] = [];
    const allSignalScores: Array<{ score: number; weight: number }> = [];
    for (const stage of blueprint.workflow.stages) {
      const evidence = evidenceById.get(stage.id);
      const declared = [...new Set(stage.evaluator_signals)];
      let total = 0;
      let weightTotal = 0;
      for (const [signal, score] of Object.entries(evidence?.signals ?? {})) {
        if (!declared.includes(signal)) rejectedSignals.push(`${stage.id}/${signal}`);
        else signalScores[`${stage.id}/${signal}`] = score;
      }
      for (const signal of declared) {
        const key = `${stage.id}/${signal}`;
        const weight = this.signalWeights[signal] ?? 1;
        const score = evidence?.signals[signal];
        weightTotal += weight;
        if (score === undefined) missingSignals.push(key);
        else total += score * weight;
        const resolved = score ?? 0;
        allSignalScores.push({ score: resolved, weight });
      }
      stageScores[stage.id] = weightTotal === 0 ? 0 : Number((total / weightTotal).toFixed(12));
    }
    const totalWeight = allSignalScores.reduce((sum, row) => sum + row.weight, 0);
    const reward = totalWeight === 0 ? 0 : Number((allSignalScores.reduce((sum, row) => sum + row.score * row.weight, 0) / totalWeight).toFixed(12));
    const passed = execution.status === "completed" && missingSignals.length === 0 && rejectedSignals.length === 0 && Object.values(signalScores).every((score) => score >= this.passThreshold);
    const status: AutonomousWorkflowEvaluation["status"] = passed ? "passed" : execution.status === "completed" && missingSignals.length === 0 ? "failed" : "incomplete";
    const evidenceDescriptor = {
      task_digest: blueprint.task_digest,
      workflow_digest: blueprint.workflow.workflow_digest,
      plan_digest: blueprint.plan.plan_digest,
      stages: input.stages.map((stage) => ({ stage_id: stage.stage_id, signals: Object.fromEntries(Object.entries(stage.signals).sort(([left], [right]) => left.localeCompare(right))), evidence_digest: stage.evidence_digest ?? null })),
      evidence_digest: input.evidence_digest ?? null,
    };
    const evidenceDigest = input.evidence_digest ?? await digestJson(evidenceDescriptor);
    const descriptor = {
      schema: AUTONOMOUS_EVALUATION_SCHEMA,
      evaluator_id: this.evaluatorId ?? `typescript-${blueprint.domain_profile.domain}-workflow-evaluator`,
      evaluator_version: this.evaluatorVersion,
      domain: blueprint.domain_profile.domain,
      task_digest: blueprint.task_digest,
      workflow_digest: blueprint.workflow.workflow_digest,
      plan_digest: blueprint.plan.plan_digest,
      execution_status: execution.status,
      stage_scores: stageScores,
      signal_scores: signalScores,
      missing_signals: [...new Set(missingSignals)].sort(),
      rejected_signals: [...new Set(rejectedSignals)].sort(),
      required_signals: requiredSignalsFor(execution),
      pass_threshold: this.passThreshold,
      reward,
      passed,
      status,
      evidence_digest: evidenceDigest,
      evaluator_authority: "caller_declared_signal_scoring_only" as const,
      retention: PRIVATE_RETENTION,
    };
    return { ...descriptor, evaluation_digest: await digestJson(descriptor) };
  }
}

export class InMemoryAutonomousLearningEpisodeStore implements AutonomousLearningEpisodeStore {
  private readonly episodes = new Map<string, AutonomousLearningEpisode>();

  load(episodeId: string): AutonomousLearningEpisode | null {
    return clone(this.episodes.get(boundedIdentifier("episodeId", episodeId)) ?? null);
  }

  save(episode: AutonomousLearningEpisode): void {
    boundedIdentifier("episode_id", episode.episode_id);
    if (episode.status !== "pending" || episode.settlement !== null) throw new ArgumentError("only pending unsettled learning episodes can be saved");
    const prior = this.episodes.get(episode.episode_id);
    if (prior) {
      if (prior.episode_digest !== episode.episode_digest) throw new ArgumentError(`learning episode ${episode.episode_id} conflicts with an existing identity`);
      if (prior.status === "settled") throw new ArgumentError(`learning episode ${episode.episode_id} is already settled`);
    }
    if (this.episodes.size >= 4096 && !prior) throw new ArgumentError("learning episode store is full");
    this.episodes.set(episode.episode_id, clone(episode));
  }

  markSettled(episodeId: string, settlement: AutonomousLearningSettlementMetadata): AutonomousLearningEpisode {
    const id = boundedIdentifier("episodeId", episodeId);
    const prior = this.episodes.get(id);
    if (!prior) throw new ArgumentError(`learning episode ${id} was not found`);
    if (prior.status === "settled") throw new ArgumentError(`learning episode ${id} has already been settled`);
    const next = { ...prior, status: "settled" as const, settlement: clone(settlement) };
    this.episodes.set(id, clone(next));
    return clone(next);
  }

  pending(limit = 256): AutonomousLearningEpisode[] {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) throw new ArgumentError("learning episode pending limit is outside its bounds");
    return [...this.episodes.values()].filter((episode) => episode.status === "pending").slice(0, limit).map((episode) => clone(episode));
  }
}

export class InMemoryAutonomousLearningTrajectoryStore implements AutonomousLearningTrajectoryStore {
  private readonly trajectories = new Map<string, AutonomousLearningTrajectory>();

  load(trajectoryId: string): AutonomousLearningTrajectory | null {
    return clone(this.trajectories.get(boundedIdentifier("trajectoryId", trajectoryId)) ?? null);
  }

  save(trajectory: AutonomousLearningTrajectory): void {
    boundedIdentifier("trajectory_id", trajectory.trajectory_id);
    if (trajectory.status !== "pending") throw new ArgumentError("only pending learning trajectories can be saved");
    const prior = this.trajectories.get(trajectory.trajectory_id);
    if (prior && prior.trajectory_digest !== trajectory.trajectory_digest) throw new ArgumentError(`learning trajectory ${trajectory.trajectory_id} conflicts with an existing identity`);
    if (this.trajectories.size >= 1024 && !prior) throw new ArgumentError("learning trajectory store is full");
    this.trajectories.set(trajectory.trajectory_id, clone(trajectory));
  }

  markSettled(trajectoryId: string, settlementDigest: Digest): AutonomousLearningTrajectory {
    const id = boundedIdentifier("trajectoryId", trajectoryId);
    boundedDigest("trajectory settlement_digest", settlementDigest);
    const prior = this.trajectories.get(id);
    if (!prior) throw new ArgumentError(`learning trajectory ${id} was not found`);
    if (prior.status === "settled") throw new ArgumentError(`learning trajectory ${id} has already been settled`);
    const next = { ...prior, status: "settled" as const, settlement_digest: settlementDigest };
    this.trajectories.set(id, clone(next));
    return clone(next);
  }
}

function projectOutcome(response: RestToolResponse<BrainOutcomeRecordResult>): BrainOutcomeRecordResult {
  if (!response.ok || response.mcp.error || response.mcp.result?.isError) throw new ProviderRuntimeError("brain outcome record returned a refusal");
  const projected = response.mcp.result?.structuredContent;
  if (!projected || typeof projected !== "object") throw new ProviderRuntimeError("brain outcome record returned no structured projection");
  return projected as BrainOutcomeRecordResult;
}

/** Explicit evaluator, delayed-credit, and value-only bandit handoff for every domain. */
export class AutonomousLearningController {
  readonly agent: AutonomousAgent;
  readonly episodes: AutonomousLearningEpisodeStore;
  readonly trajectories: AutonomousLearningTrajectoryStore;
  readonly evaluator: AutonomousWorkflowEvaluator;
  readonly apiClient?: ApiClient;

  constructor(agent: AutonomousAgent, options: { episodes?: AutonomousLearningEpisodeStore; trajectories?: AutonomousLearningTrajectoryStore; evaluator?: AutonomousWorkflowEvaluator; apiClient?: ApiClient } = {}) {
    if (!agent || typeof agent.recordEvaluatorReward !== "function") throw new ArgumentError("learning controller requires an AutonomousAgent");
    this.agent = agent;
    this.episodes = options.episodes ?? new InMemoryAutonomousLearningEpisodeStore();
    this.trajectories = options.trajectories ?? new InMemoryAutonomousLearningTrajectoryStore();
    this.evaluator = options.evaluator ?? new AutonomousWorkflowEvaluator();
    this.apiClient = options.apiClient;
  }

  async evaluateWorkflow(execution: AutonomousWorkflowExecutionResult, input: AutonomousWorkflowEvaluationInput): Promise<AutonomousWorkflowEvaluation> {
    return this.evaluator.evaluate(execution, input);
  }

  async prepareRun(result: AutonomousRunResult, options: { episodeId: string; runId?: string }): Promise<AutonomousLearningEpisode> {
    if (!isObject(options)) throw new ArgumentError("learning episode options must be an object");
    const episodeId = boundedIdentifier("episodeId", options.episodeId);
    if (!result.blueprint || !result.selection?.selected_model) throw new ArgumentError("learning episode requires a provider-completed or selected autonomous run");
    const runId = boundedIdentifier("runId", options.runId ?? episodeId);
    const selectionDigest = await digestJson(result.selection);
    const outcomeDigest = await digestJson({ status: result.status, route_digest: result.route.route_digest, selection: result.selection, response: result.response });
    const run: BrainRunIdentity = {
      run_id: runId,
      selection_digest: selectionDigest,
      prompt_digest: result.blueprint.prompt.prompt_digest,
      plan_digest: result.blueprint.plan.plan_digest,
      provider: result.selection.selected_model.provider,
      model: result.selection.selected_model.model,
      outcome_digest: outcomeDigest,
      request_id: null,
    };
    const descriptor = {
      schema: AUTONOMOUS_LEARNING_EPISODE_SCHEMA,
      episode_id: episodeId,
      run,
      domain: result.blueprint.domain_profile.domain,
      capability: result.blueprint.selection_context.capability,
      workflow_id: result.blueprint.workflow.workflow_id,
      workflow_digest: result.blueprint.workflow.workflow_digest,
      status: "pending" as const,
      settlement: null,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    const episode = { ...descriptor, episode_digest: await digestJson(descriptor) };
    this.episodes.save(episode);
    return clone(episode);
  }

  async settleRun(episodeId: string, input: AutonomousEvaluatorRewardInput, options: { creditedReward?: number; remote?: boolean } = {}): Promise<AutonomousLearningSettlement> {
    const episode = await this.episodes.load(boundedIdentifier("episodeId", episodeId));
    if (!episode) throw new ArgumentError(`learning episode ${episodeId} was not found`);
    if (episode.status === "settled") throw new ArgumentError(`learning episode ${episodeId} has already been settled`);
    assertRewardInput(input);
    const creditedReward = boundedReward("credited reward", options.creditedReward ?? input.reward);
    if (!this.agent.learner) throw new ArgumentError("learning settlement requires an AutonomousOnlineLearner on the agent");
    const assessment: BrainEvaluatorAssessment = {
      evaluator_id: input.evaluator_id,
      evaluator_version: input.evaluator_version,
      reward: creditedReward,
      passed: input.passed,
      failed: input.failed ?? !input.passed,
      feedback_digest: input.feedback_digest ?? null,
      failure_class: input.failure_class ?? null,
      evidence_digest: input.evidence_digest ?? null,
    };
    let nextState: BrainBanditState;
    let learningEvidence: BrainLearningEvidence | null = null;
    let remote = false;
    const armId = `${episode.run.provider}/${episode.run.model}`;
    if (options.remote === true) {
      if (!this.apiClient || typeof this.apiClient.brainOutcomeRecord !== "function") throw new ArgumentError("remote learning settlement requires an ApiClient with brainOutcomeRecord");
      const projected = projectOutcome(await this.apiClient.brainOutcomeRecord({ run: episode.run, assessment, bandit_state: this.agent.learner.snapshot(), arm_id: armId }));
      if (!projected.next_state || !Array.isArray(projected.next_state.arms) || !projected.learning_evidence) throw new ProviderRuntimeError("brain outcome record returned an incomplete learning projection");
      nextState = projected.next_state;
      learningEvidence = projected.learning_evidence;
      this.agent.learner.update({ arm_id: armId, reward: creditedReward, failed: assessment.failed, outcome_digest: episode.run.outcome_digest });
      remote = true;
    } else {
      nextState = await this.agent.recordEvaluatorReward(armId, creditedReward, { failed: assessment.failed, outcomeDigest: episode.run.outcome_digest });
    }
    const settlementBase = { evaluation_digest: input.evidence_digest ?? null, reward: input.reward, credited_reward: creditedReward, next_generation: boundedGeneration(nextState.generation ?? 0), settled_at: Date.now() };
    const settlement: AutonomousLearningSettlementMetadata = { ...settlementBase, settlement_digest: await digestJson(settlementBase) };
    const settledEpisode = await this.episodes.markSettled(episode.episode_id, settlement);
    return { schema: AUTONOMOUS_LEARNING_EPISODE_SCHEMA, episode: settledEpisode, assessment, next_state: clone(nextState), learning_evidence: learningEvidence, remote, retention: PRIVATE_RETENTION };
  }

  async prepareTrajectory(episodeIds: readonly string[], options: { trajectoryId: string; discount?: number }): Promise<AutonomousLearningTrajectory> {
    const trajectoryId = boundedIdentifier("trajectoryId", options.trajectoryId);
    if (!Array.isArray(episodeIds) || episodeIds.length < 1 || episodeIds.length > AUTONOMOUS_LEARNING_MAX_TRAJECTORY_STEPS) throw new ArgumentError("learning trajectory must contain between 1 and 32 episodes");
    const discount = boundedReward("trajectory discount", options.discount ?? 0.9);
    if (discount >= 1) throw new ArgumentError("trajectory discount must be below 1");
    const ids = episodeIds.map((id) => boundedIdentifier("trajectory episodeId", id));
    if (new Set(ids).size !== ids.length) throw new ArgumentError("learning trajectory episode IDs must be unique");
    const episodes = await Promise.all(ids.map((id) => this.episodes.load(id)));
    if (episodes.some((episode) => !episode)) throw new ArgumentError("learning trajectory references a missing episode");
    if (episodes.some((episode) => episode?.status !== "pending")) throw new ArgumentError("learning trajectory can only contain pending episodes");
    const steps = episodes.map((episode, index) => ({ index, episode_id: episode!.episode_id, arm_id: `${episode!.run.provider}/${episode!.run.model}`, run_digest: episode!.run.outcome_digest, raw_reward: null, credited_reward: null }));
    const descriptor = { schema: AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA, trajectory_id: trajectoryId, discount, steps, status: "pending" as const, settlement_digest: null, retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    const trajectory = { ...descriptor, trajectory_digest: await digestJson(descriptor) };
    this.trajectories.save(trajectory);
    return clone(trajectory);
  }

  async settleTrajectory(trajectoryId: string, rewards: Record<string, AutonomousEvaluatorRewardInput>, options: { remote?: boolean } = {}): Promise<AutonomousTrajectorySettlement> {
    const trajectory = await this.trajectories.load(boundedIdentifier("trajectoryId", trajectoryId));
    if (!trajectory) throw new ArgumentError(`learning trajectory ${trajectoryId} was not found`);
    if (trajectory.status === "settled") throw new ArgumentError(`learning trajectory ${trajectoryId} has already been settled`);
    if (!isObject(rewards)) throw new ArgumentError("trajectory rewards must be an object keyed by episode ID");
    const expected = new Set(trajectory.steps.map((step) => step.episode_id));
    const supplied = Object.keys(rewards);
    if (supplied.length !== expected.size || supplied.some((id) => !expected.has(id))) throw new ArgumentError("trajectory rewards must cover exactly every episode");
    for (const step of trajectory.steps) assertRewardInput(rewards[step.episode_id]!);
    const returnToGo: Record<string, number> = {};
    let next = 0;
    for (let index = trajectory.steps.length - 1; index >= 0; index -= 1) {
      const step = trajectory.steps[index]!;
      const raw = rewards[step.episode_id]!.reward;
      boundedReward(`trajectory reward ${step.episode_id}`, raw);
      next = Number(Math.min(1, raw + trajectory.discount * next).toFixed(12));
      returnToGo[step.episode_id] = next;
    }
    const settlements: AutonomousLearningSettlement[] = [];
    for (const step of trajectory.steps) {
      const reward = rewards[step.episode_id]!;
      settlements.push(await this.settleRun(step.episode_id, reward, { creditedReward: returnToGo[step.episode_id], remote: options.remote }));
    }
    const settlementDigest = await digestJson({ trajectory_digest: trajectory.trajectory_digest, return_to_go: returnToGo, settlement_digests: settlements.map((settlement) => settlement.episode.settlement?.settlement_digest ?? null) });
    const settledTrajectory = await this.trajectories.markSettled(trajectory.trajectory_id, settlementDigest);
    return { schema: AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA, trajectory: settledTrajectory, settlements, return_to_go: returnToGo, retention: PRIVATE_RETENTION };
  }
}
