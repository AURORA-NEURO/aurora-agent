/** Deterministic value-only model learning and the remote contextual selector adapter. */
import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type { ApiClient } from "./client.js";
import {
  type AutonomousModelCandidate,
  type AutonomousModelRanking,
  type AutonomousSelectionWeights,
  normalizeAutonomousSelectionWeights,
  normalizeAutonomousModelObservations,
  type AutonomousModelSelector,
  type AutonomousSelectionDecision,
  type AutonomousSelectionRequest,
  rankAutonomousModels,
  autonomousSelectionConfidence,
} from "./llm.js";
import { digestBytesSync, digestCanonicalJsonTextSync } from "./tooling.js";
import type {
  BrainBanditArm,
  BrainBanditContext,
  BrainBanditContextState,
  BrainBanditPolicy,
  BrainBanditState,
  BrainBanditUpdate,
  BrainContextualModelSelectionResult,
  BrainModelDescriptor,
  BrainModelSelectionArgs,
  BrainProviderHealth,
} from "./types.js";

const AUTONOMOUS_BANDIT_MAX_ARMS = 512;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function validateOnlineSelectionConstraints(request: AutonomousSelectionRequest): void {
  const constraints: Array<[string, unknown, number]> = [
    ["max_cost_per_million_tokens", request.max_cost_per_million_tokens, 1_000_000_000],
    ["max_latency_ms", request.max_latency_ms, 10 * 60_000],
    ["min_quality", request.min_quality, 1],
    ["min_selection_confidence", request.min_selection_confidence, 1],
  ];
  for (const [name, value, maximum] of constraints) {
    if (value === undefined || value === null) continue;
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > maximum) throw new ArgumentError(`online learner ${name} is outside its bounds`);
  }
  if (request.require_json !== undefined && typeof request.require_json !== "boolean") throw new ArgumentError("online learner require_json must be boolean");
}

function normalizeLearningContext(context: Partial<BrainBanditContext>): BrainBanditContext {
  if (!isObject(context)) throw new ArgumentError("online learner context must be an object");
  return {
    domain: boundedText("online learner context domain", context.domain, 256),
    capability: boundedText("online learner context capability", context.capability, 256),
    risk_class: boundedText("online learner context risk_class", context.risk_class, 256),
    task_family: context.task_family === undefined || context.task_family === null ? null : boundedText("online learner context task_family", context.task_family, 256),
  };
}

function assertLearningContextDigest(contextDigest: string, context: BrainBanditContext): void {
  // Keep field order aligned with Rust serde and Python's normalized mapping. The
  // explicit null is part of the shared identity when task_family is absent.
  const expected = digestCanonicalJsonTextSync(JSON.stringify(context));
  if (contextDigest !== expected) throw new ArgumentError("online learner context_digest does not match its context identity");
}

function deterministicBanditDraw(seed: number, generation: number, label: string): number {
  const labelBytes = new TextEncoder().encode(label);
  const payload = new Uint8Array(16 + labelBytes.length);
  const view = new DataView(payload.buffer);
  view.setBigUint64(0, BigInt(seed), false);
  view.setBigUint64(8, BigInt(Math.max(0, Math.floor(generation))), false);
  payload.set(labelBytes, 16);
  const firstWord = BigInt(`0x${digestBytesSync(payload).slice(0, 16)}`);
  return Number(firstWord) / Number(0xffff_ffff_ffff_ffffn);
}

function deterministicBanditDrawWithCounter(seed: number, generation: number, label: string, counter: number): number {
  const labelBytes = new TextEncoder().encode(label);
  const payload = new Uint8Array(16 + labelBytes.length + 8);
  const view = new DataView(payload.buffer);
  view.setBigUint64(0, BigInt(seed), false);
  view.setBigUint64(8, BigInt(Math.max(0, Math.floor(generation))), false);
  payload.set(labelBytes, 16);
  view.setBigUint64(16 + labelBytes.length, BigInt(Math.max(0, Math.floor(counter))), false);
  const firstWord = BigInt(`0x${digestBytesSync(payload).slice(0, 16)}`);
  return (Number(firstWord) + 0.5) / (Number(0xffff_ffff_ffff_ffffn) + 1);
}

/**
 * Evaluator-equivalent observations used to warm-start an arm from its static utility.
 * Keeping this explicit and immutable prevents a cold-start decision from depending on hidden
 * process state, while still letting real evaluator feedback take over after a small sample.
 */
const AUTONOMOUS_LEARNER_PRIOR_PSEUDO_PULLS = 4;

function standardNormalFromUniforms(first: number, second: number): number {
  return Math.sqrt(-2 * Math.log(first)) * Math.cos(2 * Math.PI * second);
}

function deterministicGammaSample(shapeInput: number, seed: number, generation: number, label: string): number {
  const shape = Math.max(1e-9, shapeInput);
  if (shape < 1) {
    const shifted = deterministicGammaSample(shape + 1, seed, generation, label);
    const uniform = deterministicBanditDrawWithCounter(seed, generation, label, 255);
    return shifted * uniform ** (1 / shape);
  }
  const d = shape - 1 / 3;
  const c = 1 / Math.sqrt(9 * d);
  for (let attempt = 0; attempt < 32; attempt += 1) {
    const first = deterministicBanditDrawWithCounter(seed, generation, label, attempt * 3);
    const second = deterministicBanditDrawWithCounter(seed, generation, label, attempt * 3 + 1);
    const z = standardNormalFromUniforms(first, second);
    const transformed = 1 + c * z;
    if (transformed <= 0) continue;
    const v = transformed ** 3;
    const acceptance = deterministicBanditDrawWithCounter(seed, generation, label, attempt * 3 + 2);
    if (acceptance < 1 - 0.0331 * z ** 4 || Math.log(acceptance) < 0.5 * z * z + d * (1 - v + Math.log(v))) return d * v;
  }
  return shape;
}

function deterministicBetaSample(alpha: number, beta: number, seed: number, generation: number, label: string): number {
  const left = deterministicGammaSample(alpha, seed, generation, `${label}/alpha`);
  const right = deterministicGammaSample(beta, seed, generation, `${label}/beta`);
  const total = left + right;
  return Number.isFinite(total) && total > 0 ? Math.min(1, Math.max(0, left / total)) : alpha / (alpha + beta);
}

function thompsonPosterior(
  arm: BrainBanditArm | undefined,
  policy: BrainBanditPolicy,
  seed: number,
  generation: number,
  armId: string,
  priorReward?: number,
): { alpha: number; beta: number; sample: number; sampledReward: number } {
  const minimum = policy.min_reward ?? -1;
  const maximum = policy.max_reward ?? 1;
  const span = maximum - minimum;
  const pulls = arm?.pulls ?? 0;
  const rewardSum = arm?.reward_sum ?? 0;
  const failures = arm?.failures ?? 0;
  const priorMass = priorReward === undefined ? 0 : AUTONOMOUS_LEARNER_PRIOR_PSEUDO_PULLS;
  const normalizedPriorSuccessMass = priorReward === undefined
    ? 0
    : priorMass * Math.min(1, Math.max(0, (priorReward - minimum) / span));
  const observedSuccessMass = pulls === 0 ? 0 : Math.min(pulls, Math.max(0, (rewardSum - minimum * pulls) / span));
  const normalizedSuccessMass = normalizedPriorSuccessMass + observedSuccessMass;
  const normalizedFailureMass = Math.max(0, priorMass - normalizedPriorSuccessMass + pulls - observedSuccessMass + (policy.failure_penalty ?? 0.25) * failures);
  const alpha = 1 + normalizedSuccessMass;
  const beta = 1 + normalizedFailureMass;
  const sample = deterministicBetaSample(alpha, beta, seed, generation, armId);
  return { alpha, beta, sample, sampledReward: minimum + sample * span };
}

function autonomousLearnerPriorReward(staticBaseScore: number, weights: AutonomousSelectionWeights, policy: BrainBanditPolicy): number {
  // Selection weights are coefficients rather than probabilities. Normalize by their static
  // magnitude before mapping utility into the configured evaluator-reward range; otherwise a
  // caller using weights above 1 could permanently drown out online feedback.
  const staticWeightScale = Math.max(1, weights.quality + weights.reliability + weights.cost + weights.latency);
  const normalizedUtility = Math.max(-1, Math.min(1, staticBaseScore / staticWeightScale));
  const minimum = policy.min_reward ?? -1;
  const maximum = policy.max_reward ?? 1;
  return minimum + ((normalizedUtility + 1) / 2) * (maximum - minimum);
}

function autonomousLearnerAdaptedMean(priorReward: number, meanReward: number, pulls: number): number {
  return (priorReward * AUTONOMOUS_LEARNER_PRIOR_PSEUDO_PULLS + meanReward * pulls)
    / (AUTONOMOUS_LEARNER_PRIOR_PSEUDO_PULLS + pulls);
}

function learnerContext(request: AutonomousSelectionRequest): { context_digest: string; context: BrainBanditContext } | null {
  if (request.context_digest === undefined || request.context_digest === null) return null;
  if (typeof request.context_digest !== "string" || !/^[0-9a-f]{64}$/.test(request.context_digest)) throw new ArgumentError("online learner context_digest must be a lowercase SHA-256 digest");
  const context = normalizeLearningContext(request);
  assertLearningContextDigest(request.context_digest, context);
  return { context_digest: request.context_digest, context };
}

function validateContextState(state: BrainBanditContextState): void {
  if (!isObject(state) || typeof state.context_digest !== "string" || !/^[0-9a-f]{64}$/.test(state.context_digest) || !isObject(state.context) || !Array.isArray(state.arms)) throw new ArgumentError("online learner contextual state is malformed");
  if (state.generation !== undefined && (!Number.isSafeInteger(state.generation) || state.generation < 0)) throw new ArgumentError("online learner contextual generation is malformed");
  if (state.observed !== undefined && typeof state.observed !== "boolean") throw new ArgumentError("online learner contextual observed flag is malformed");
  learnerContext({ ...state.context, context_digest: state.context_digest, task: "context", required_capabilities: [], estimated_input_tokens: 1, requested_output_tokens: 1, candidates: [], provider_health: {}, model_health: {} });
}

/** Caller-owned bounded online model adaptation. No hidden server state is used. */
export class AutonomousOnlineLearner {
  private stateValue: BrainBanditState;
  private readonly policy: BrainBanditPolicy;

  constructor(options: { state?: BrainBanditState; policy?: BrainBanditPolicy } = {}) {
    this.policy = { strategy: "ucb1", exploration: 0.5, epsilon: 0.1, min_reward: -1, max_reward: 1, failure_penalty: 0.25, seed: 0, ...(options.state?.policy ?? {}), ...(options.policy ?? {}) };
    if (this.policy.strategy !== "ucb1" && this.policy.strategy !== "epsilon_greedy" && this.policy.strategy !== "thompson_sampling") throw new ArgumentError("online learner strategy must be ucb1, epsilon_greedy, or thompson_sampling");
    for (const [name, value, minimum, maximum] of [
      ["exploration", this.policy.exploration, 0, 100],
      ["epsilon", this.policy.epsilon, 0, 1],
      ["min_reward", this.policy.min_reward, -100, 100],
      ["max_reward", this.policy.max_reward, -100, 100],
      ["failure_penalty", this.policy.failure_penalty, 0, 100],
    ] as const) {
      if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`online learner policy ${name} is outside its bounds`);
    }
    if ((this.policy.min_reward ?? 0) >= (this.policy.max_reward ?? 0)) throw new ArgumentError("online learner policy min_reward must be below max_reward");
    if (typeof this.policy.seed !== "number" || !Number.isSafeInteger(this.policy.seed) || this.policy.seed < 0) throw new ArgumentError("online learner policy seed must be a non-negative safe integer");
    const restoredState = options.state ? cloneBanditState(options.state) : { schema: "bioprism-brain-bandit-state/0.1", generation: 0, policy: this.policy, arms: [] };
    this.stateValue = { ...restoredState, policy: this.policy };
    this.assertState();
  }

  snapshot(): BrainBanditState {
    return cloneBanditState(this.stateValue);
  }

  /**
   * Adopt a value-only projection produced by the remote control plane.
   *
   * Remote settlement may normalize first-run arms, contextual rows, replay receipts, or
   * generation numbers. Replaying the request locally is not equivalent to adopting that
   * projection: a server can legitimately reject, deduplicate, or enrich the transition. Keep
   * the local policy as the runtime's configured policy when older transports omit it, then
   * validate the complete state before making it observable to selection.
   */
  restore(state: BrainBanditState): BrainBanditState {
    const restoredState = cloneBanditState(state);
    if (restoredState.policy !== undefined) {
      for (const field of ["strategy", "exploration", "epsilon", "min_reward", "max_reward", "failure_penalty", "seed"] as const) {
        if (restoredState.policy[field] !== undefined && restoredState.policy[field] !== this.policy[field]) throw new ArgumentError(`online learner remote policy ${field} conflicts with the local policy`);
      }
    }
    this.stateValue = { ...restoredState, policy: this.policy };
    this.assertState();
    return this.snapshot();
  }

  /**
   * Select the best eligible model using static utility plus caller-owned evaluator evidence.
   * Static utility is treated as a bounded four-pull prior, while contextual, global, and
   * request-supplied observations progressively adapt the decision; deterministic ties are by
   * arm id.
   */
  select(request: AutonomousSelectionRequest): AutonomousSelectionDecision {
    validateOnlineSelectionConstraints(request);
    const canonicalRanking = rankAutonomousModels(request);
    // The canonical ranker intentionally mixes caller-supplied observations into its score.
    // Keep that ranking for hard gates and audit context, but derive the learner prior from a
    // score with no observations so the same evaluator evidence is not counted twice.
    const selectionWeights = normalizeAutonomousSelectionWeights(request.weights);
    const staticRanking = rankAutonomousModels({ ...request, weights: selectionWeights, observations: [] });
    const staticRankingByArm = new Map(staticRanking.map((row) => [`${row.provider}/${row.model}`, row]));
    const suppliedObservations = normalizeAutonomousModelObservations(request.observations);
    const suppliedObservationByArm = new Map(suppliedObservations.map((observation) => [observation.arm_id, observation]));
    const context = learnerContext(request);
    const contextualState = context ? this.stateValue.contextual_states?.find((state) => state.context_digest === context.context_digest) : undefined;
    const observationFor = (armId: string): { arm: BrainBanditArm | undefined; source: "contextual" | "global" | "request" | "prior" } => {
      const contextualArm = contextualState?.arms.find((arm) => arm.arm_id === armId);
      if (contextualArm) return { arm: contextualArm, source: "contextual" };
      const globalArm = this.stateValue.arms.find((arm) => arm.arm_id === armId);
      if (globalArm) return { arm: globalArm, source: context ? "global" : "global" };
      const suppliedArm = suppliedObservationByArm.get(armId);
      if (suppliedArm) return { arm: suppliedArm, source: "request" };
      return { arm: undefined, source: "prior" };
    };
    const eligible = canonicalRanking.filter((row) => row.eligible && !observationFor(`${row.provider}/${row.model}`).arm?.disabled);
    const totalPulls = Math.max(1, eligible.reduce((sum, row) => sum + (observationFor(`${row.provider}/${row.model}`).arm?.pulls ?? 0), 0));
    const scoredEligible = eligible.map((row) => {
      const candidate = request.candidates.find((item) => item.provider === row.provider && item.model === row.model)!;
      const armId = `${candidate.provider}/${candidate.model}`;
      const observation = observationFor(armId);
      const arm = observation.arm;
      const pulls = arm?.pulls ?? 0;
      const mean = pulls ? (arm?.reward_sum ?? 0) / pulls : 0;
      const staticRow = staticRankingByArm.get(armId);
      const staticBaseScore = staticRow?.base_score ?? 0;
      const priorReward = autonomousLearnerPriorReward(staticBaseScore, selectionWeights, this.policy);
      const adaptedMean = autonomousLearnerAdaptedMean(priorReward, mean, pulls);
      const failureRate = pulls ? (arm?.failures ?? 0) / pulls : 0;
      const posterior = this.policy.strategy === "thompson_sampling" ? thompsonPosterior(arm, this.policy, this.policy.seed ?? 0, this.stateValue.generation ?? 0, armId, priorReward) : null;
      const bonus = posterior
        ? posterior.sampledReward - adaptedMean
        : this.policy.strategy === "ucb1"
          ? (pulls ? Math.sqrt(Math.log(totalPulls + 1) / pulls) * (this.policy.exploration ?? 0.5) : (this.policy.exploration ?? 0.5))
          : 0;
      const score = (posterior ? posterior.sampledReward : adaptedMean + bonus) - (this.policy.failure_penalty ?? 0.25) * failureRate;
      return { candidate, armId, pulls, source: observation.source, score, mean, adaptedMean, priorReward, staticBaseScore, bonus, failureRate, posterior };
    }).sort((left, right) => right.score - left.score || left.armId.localeCompare(right.armId));
    const explorationDraw = this.policy.strategy === "epsilon_greedy" ? deterministicBanditDraw(this.policy.seed ?? 0, this.stateValue.generation ?? 0, "epsilon") : null;
    const explorationTaken = explorationDraw !== null && explorationDraw < (this.policy.epsilon ?? 0.1);
    const selected = explorationTaken
      ? scoredEligible[Math.min(Math.floor(deterministicBanditDraw(this.policy.seed ?? 0, this.stateValue.generation ?? 0, "epsilon-arm") * scoredEligible.length), Math.max(0, scoredEligible.length - 1))]
      : scoredEligible[0];
    const disabledRanking = canonicalRanking
      .filter((row) => row.eligible && observationFor(`${row.provider}/${row.model}`).arm?.disabled)
      .map((row) => ({ ...row, eligible: false, reasons: [...row.reasons, "bandit arm is disabled"] }));
    const ranking = [
      ...scoredEligible.map((row) => ({ provider: row.candidate.provider, model: row.candidate.model, score: Number(row.score.toFixed(12)), eligible: true, reasons: [`arm_id=${row.armId}`, `pulls=${row.pulls}`, `mean_reward=${row.mean.toFixed(6)}`, `adapted_mean=${row.adaptedMean.toFixed(6)}`, `static_prior_reward=${row.priorReward.toFixed(6)}`, `static_base_score=${row.staticBaseScore.toFixed(6)}`, `failure_rate=${row.failureRate.toFixed(6)}`, `exploration_bonus=${row.bonus.toFixed(6)}`, ...(row.posterior ? [`posterior_alpha=${row.posterior.alpha.toFixed(6)}`, `posterior_beta=${row.posterior.beta.toFixed(6)}`, `posterior_sample=${row.posterior.sample.toFixed(6)}`] : []), `history=${row.source}`, ...(context ? [`context_digest=${context.context_digest}`] : [])], base_score: Number((row.adaptedMean - (this.policy.failure_penalty ?? 0.25) * row.failureRate).toFixed(12)), exploration_bonus: Number(row.bonus.toFixed(12)), observed_pulls: row.pulls })),
      ...disabledRanking,
      ...canonicalRanking.filter((row) => !row.eligible),
    ];
    const selectionConfidence = autonomousSelectionConfidence(
      scoredEligible.map((row) => ({ provider: row.candidate.provider, model: row.candidate.model, score: Number(row.score.toFixed(12)), eligible: true, reasons: [] })),
    );
    if (!selected) {
      const reasons = ranking.flatMap((row) => row.reasons).join("; ");
      return { selected_model: null, strategy: "caller_selector", ranking, abstention_reason: `online learner found no eligible candidate${reasons ? `: ${reasons}` : ""}`, selection_confidence: selectionConfidence, min_selection_confidence: request.min_selection_confidence ?? null, exploration_draw: explorationDraw, exploration_taken: false };
    }
    const minimumConfidence = request.min_selection_confidence ?? null;
    if (minimumConfidence !== null && selectionConfidence < minimumConfidence) {
      return { selected_model: null, strategy: "caller_selector", ranking, abstention_reason: `selection confidence ${selectionConfidence.toFixed(6)} is below caller floor ${minimumConfidence.toFixed(6)}`, selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence, exploration_draw: explorationDraw, exploration_taken: false };
    }
    return { selected_model: { provider: selected.candidate.provider, model: selected.candidate.model }, strategy: "caller_selector", ranking, abstention_reason: null, selection_confidence: selectionConfidence, min_selection_confidence: minimumConfidence, exploration_draw: explorationDraw, exploration_taken: explorationTaken || this.policy.strategy === "thompson_sampling" };
  }

  /** Apply an explicit evaluator reward. Provider success alone is not treated as task quality. */
  update(update: BrainBanditUpdate): BrainBanditState {
    const minimumReward = this.policy.min_reward ?? -1;
    const maximumReward = this.policy.max_reward ?? 1;
    if (!isObject(update) || typeof update.arm_id !== "string" || !update.arm_id.trim() || typeof update.reward !== "number" || !Number.isFinite(update.reward) || update.reward < minimumReward || update.reward > maximumReward) throw new ArgumentError(`online learner update requires an arm_id and reward within [${minimumReward}, ${maximumReward}]`);
    const contextDigest = update.context_digest ?? null;
    if (contextDigest !== null && (typeof contextDigest !== "string" || !/^[0-9a-f]{64}$/.test(contextDigest))) throw new ArgumentError("online learner context_digest must be a lowercase SHA-256 digest");
    if (contextDigest !== null && (!update.context || !isObject(update.context))) throw new ArgumentError("contextual learner updates require their bounded context identity");
    if (contextDigest === null && update.context !== undefined) throw new ArgumentError("online learner context requires a context_digest");
    const context = contextDigest === null
      ? null
      : learnerContext({ ...(update.context as BrainBanditContext), context_digest: contextDigest, task: "context", required_capabilities: [], estimated_input_tokens: 1, requested_output_tokens: 1, candidates: [], provider_health: {}, model_health: {} })!.context;
    const creditedOutcomes = [...(this.stateValue.credited_outcomes ?? [])];
    if (update.outcome_digest !== undefined && update.outcome_digest !== null) {
      if (typeof update.outcome_digest !== "string" || !/^[0-9a-f]{64}$/.test(update.outcome_digest)) throw new ArgumentError("online learner outcome_digest must be a lowercase SHA-256 digest");
      const prior = creditedOutcomes.find((receipt) => receipt.outcome_digest === update.outcome_digest);
      if (prior) {
        if (prior.arm_id !== update.arm_id || prior.reward !== update.reward || Boolean(prior.failed) !== Boolean(update.failed) || (prior.contract_digest ?? null) !== (update.contract_digest ?? null) || (prior.context_digest ?? null) !== contextDigest) throw new ArgumentError("online learner replayed outcome has contradictory evaluator evidence");
        return this.snapshot();
      }
      if (creditedOutcomes.length >= 4096) throw new ArgumentError("online learner credited outcome ledger is full");
      if (update.contract_digest !== undefined && update.contract_digest !== null && (typeof update.contract_digest !== "string" || !/^[0-9a-f]{64}$/.test(update.contract_digest))) throw new ArgumentError("online learner contract_digest must be a lowercase SHA-256 digest");
      creditedOutcomes.push({ outcome_digest: update.outcome_digest, arm_id: update.arm_id, reward: update.reward, failed: update.failed ?? false, contract_digest: update.contract_digest ?? null, ...(contextDigest === null ? {} : { context_digest: contextDigest }) });
    }
    const arms = this.stateValue.arms.map((arm) => ({ ...arm }));
    const contextualStates = (this.stateValue.contextual_states ?? []).map((state) => ({ ...state, context: { ...state.context }, arms: state.arms.map((arm) => ({ ...arm })) }));
    const targetArms = contextDigest === null
      ? arms
      : (contextualStates.find((state) => state.context_digest === contextDigest)?.arms ?? (() => {
        const contextState: BrainBanditContextState = { context_digest: contextDigest, context: { ...context! }, generation: 0, arms: [], observed: false };
        contextualStates.push(contextState);
        return contextState.arms;
      })());
    const existing = targetArms.find((arm) => arm.arm_id === update.arm_id);
    if (existing?.disabled) throw new ArgumentError("online learner cannot update a disabled arm");
    if (existing) {
      existing.pulls = (existing.pulls ?? 0) + 1;
      existing.reward_sum = (existing.reward_sum ?? 0) + update.reward;
      if (update.failed) existing.failures = (existing.failures ?? 0) + 1;
    } else {
      targetArms.push({ arm_id: update.arm_id, pulls: 1, reward_sum: update.reward, failures: update.failed ? 1 : 0 });
    }
    if (contextDigest !== null) {
      const contextual = contextualStates.find((state) => state.context_digest === contextDigest)!;
      contextual.generation = (contextual.generation ?? 0) + 1;
      contextual.observed = true;
      contextual.arms = targetArms.sort((left, right) => left.arm_id.localeCompare(right.arm_id));
    }
    this.stateValue = { ...this.stateValue, generation: (this.stateValue.generation ?? 0) + 1, policy: this.policy, arms: arms.sort((left, right) => left.arm_id.localeCompare(right.arm_id)), credited_outcomes: creditedOutcomes, ...(contextualStates.length ? { contextual_states: contextualStates.sort((left, right) => left.context_digest.localeCompare(right.context_digest)) } : {}) };
    this.assertState();
    return this.snapshot();
  }

  private assertState(): void {
    if (!isObject(this.stateValue) || !Array.isArray(this.stateValue.arms) || this.stateValue.arms.length > AUTONOMOUS_BANDIT_MAX_ARMS) throw new ArgumentError("online learner state is malformed");
    if (!Number.isSafeInteger(this.stateValue.generation) || (this.stateValue.generation ?? 0) < 0) throw new ArgumentError("online learner state generation is malformed");
    const creditedOutcomes = this.stateValue.credited_outcomes ?? [];
    if (!Array.isArray(creditedOutcomes) || creditedOutcomes.length > 4096 || creditedOutcomes.some((receipt) => !isObject(receipt) || typeof receipt.outcome_digest !== "string" || !/^[0-9a-f]{64}$/.test(receipt.outcome_digest) || typeof receipt.arm_id !== "string" || !receipt.arm_id.trim() || typeof receipt.reward !== "number" || !Number.isFinite(receipt.reward) || receipt.reward < (this.policy.min_reward ?? -1) || receipt.reward > (this.policy.max_reward ?? 1) || (receipt.failed !== undefined && typeof receipt.failed !== "boolean") || (receipt.contract_digest !== undefined && receipt.contract_digest !== null && (typeof receipt.contract_digest !== "string" || !/^[0-9a-f]{64}$/.test(receipt.contract_digest))) || (receipt.context_digest !== undefined && receipt.context_digest !== null && (typeof receipt.context_digest !== "string" || !/^[0-9a-f]{64}$/.test(receipt.context_digest)))) || new Set(creditedOutcomes.map((receipt) => receipt.outcome_digest)).size !== creditedOutcomes.length) throw new ArgumentError("online learner credited outcome ledger is malformed");
    const validateArms = (arms: BrainBanditArm[]): void => {
      if (!Array.isArray(arms) || arms.length > AUTONOMOUS_BANDIT_MAX_ARMS) throw new ArgumentError("online learner arm collection is malformed");
      const armIds = new Set<string>();
      for (const arm of arms) {
        const pulls = arm?.pulls ?? 0;
        const rewardSum = arm?.reward_sum ?? 0;
        const failures = arm?.failures ?? 0;
        if (!isObject(arm) || typeof arm.arm_id !== "string" || !arm.arm_id.trim() || !Number.isSafeInteger(pulls) || pulls < 0 || typeof rewardSum !== "number" || !Number.isFinite(rewardSum) || rewardSum < pulls * (this.policy.min_reward ?? -1) || rewardSum > pulls * (this.policy.max_reward ?? 1) || !Number.isSafeInteger(failures) || failures < 0 || failures > pulls || (arm.disabled !== undefined && typeof arm.disabled !== "boolean")) throw new ArgumentError("online learner arm is malformed");
        if (armIds.has(arm.arm_id)) throw new ArgumentError(`online learner arm ${arm.arm_id} is duplicated`);
        armIds.add(arm.arm_id);
      }
    };
    validateArms(this.stateValue.arms);
    const contextualStates = this.stateValue.contextual_states ?? [];
    if (!Array.isArray(contextualStates) || contextualStates.length > 64 || contextualStates.some((state) => !isObject(state) || typeof state.context_digest !== "string") || new Set(contextualStates.map((state) => state.context_digest)).size !== contextualStates.length) throw new ArgumentError("online learner contextual states are malformed");
    for (const state of contextualStates) {
      validateContextState(state);
      validateArms(state.arms);
    }
  }
}

function cloneBanditState(state: BrainBanditState): BrainBanditState {
  if (!isObject(state) || !Array.isArray(state.arms)) throw new ArgumentError("bandit state must contain arms");
  if (state.generation !== undefined && (!Number.isSafeInteger(state.generation) || state.generation < 0)) throw new ArgumentError("bandit state generation must be a non-negative safe integer");
  if (state.credited_outcomes !== undefined && !Array.isArray(state.credited_outcomes)) throw new ArgumentError("bandit credited_outcomes must be an array");
  if (state.contextual_states !== undefined && !Array.isArray(state.contextual_states)) throw new ArgumentError("bandit contextual_states must be an array");
  const contextualStates = state.contextual_states ?? [];
  if (contextualStates.some((contextState) => !isObject(contextState) || !isObject(contextState.context) || !Array.isArray(contextState.arms))) throw new ArgumentError("bandit contextual state must contain context and arms");
  return { schema: typeof state.schema === "string" ? state.schema : "bioprism-brain-bandit-state/0.1", generation: state.generation ?? 0, policy: state.policy ? { ...state.policy } : undefined, arms: state.arms.map((arm) => ({ ...arm })), credited_outcomes: (state.credited_outcomes ?? []).map((receipt) => ({ ...receipt })), ...(contextualStates.length ? { contextual_states: contextualStates.map((contextState) => ({ ...contextState, context: { ...contextState.context }, arms: contextState.arms.map((arm) => ({ ...arm })) })) } : {}) };
}

/**
 * Preserve the control-plane's value-only model ranking at the provider boundary.
 *
 * The MCP response is remote input even when it comes from the project's own Rust server. Keep
 * the selected identity bound to the exact local candidate catalogue and reject malformed or
 * unknown rows before the runtime can expose them as an execution decision. Optional score
 * components are retained because they are useful for audits, but they never authorize a model.
 */
function contextualSelectionRanking(value: unknown, candidates: readonly AutonomousModelCandidate[]): AutonomousModelRanking[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value) || value.length > 128) throw new ProviderRuntimeError("contextual brain selector returned a malformed ranking");
  const candidateById = new Map(candidates.map((candidate) => [`${candidate.provider}/${candidate.model}`, candidate]));
  const seen = new Set<string>();
  return value.map((raw, index) => {
    if (!isObject(raw) || typeof raw.model_id !== "string" || !raw.model_id.trim() || typeof raw.score !== "number" || !Number.isFinite(raw.score) || typeof raw.eligible !== "boolean" || !Array.isArray(raw.reasons) || raw.reasons.length > 64 || raw.reasons.some((reason) => typeof reason !== "string" || !reason.trim())) {
      throw new ProviderRuntimeError(`contextual brain selector returned a malformed ranking row ${index}`);
    }
    const modelId = raw.model_id;
    const candidate = candidateById.get(modelId);
    if (!candidate) throw new ProviderRuntimeError(`contextual brain selector returned an unknown model ${modelId}`);
    if (seen.has(modelId)) throw new ProviderRuntimeError(`contextual brain selector returned duplicate model ${modelId}`);
    seen.add(modelId);
    const optionalMetric = (name: string): number | undefined => {
      const metric = raw[name];
      if (metric === undefined) return undefined;
      if (typeof metric !== "number" || !Number.isFinite(metric)) throw new ProviderRuntimeError(`contextual brain selector returned an invalid ${name}`);
      return metric;
    };
    const observedPullsRaw = raw.observed_pulls;
    const observedPulls = observedPullsRaw === undefined || observedPullsRaw === null ? undefined : observedPullsRaw;
    if (observedPulls !== undefined && (typeof observedPulls !== "number" || !Number.isSafeInteger(observedPulls) || observedPulls < 0)) throw new ProviderRuntimeError("contextual brain selector returned invalid observed_pulls");
    return {
      provider: candidate.provider,
      model: candidate.model,
      score: raw.score,
      eligible: raw.eligible,
      reasons: [...raw.reasons],
      ...(raw.base_score === undefined ? {} : { base_score: optionalMetric("base_score") }),
      ...(raw.exploration_bonus === undefined ? {} : { exploration_bonus: optionalMetric("exploration_bonus") }),
      ...(observedPulls === undefined ? {} : { observed_pulls: observedPulls }),
    };
  });
}

/** Adapt the TypeScript runtime to the value-only Rust/Python contextual selector. */
export function contextualSelector(client: ApiClient, options: { requestOptions?: Parameters<ApiClient["brainModelSelectContextual"]>[1]; observations?: (request: AutonomousSelectionRequest) => Array<{ context_digest: string; arm_id: string; pulls?: number; reward_sum?: number; failures?: number; disabled?: boolean }> } = {}): AutonomousModelSelector {
  if (!client || typeof client.brainModelSelectContextual !== "function") throw new ArgumentError("contextual selector requires an ApiClient");
  return async (request) => {
    const models = request.candidates.map((candidate) => ({
      provider: candidate.provider,
      model: candidate.model,
      capabilities: [...(candidate.capabilities ?? [])],
      context_window_tokens: candidate.context_window_tokens,
      max_output_tokens: candidate.max_output_tokens,
      quality: candidate.quality,
      latency_ms: candidate.latency_ms,
      cost_per_million_tokens: candidate.cost_per_million_tokens,
      reliability: candidate.reliability,
      requires_credential: candidate.requires_credential,
      enabled: candidate.enabled,
      model_id: `${candidate.provider}/${candidate.model}`,
    } satisfies BrainModelDescriptor));
    const base: BrainModelSelectionArgs = {
      task: request.task,
      required_capabilities: [...request.required_capabilities],
      input_tokens: request.estimated_input_tokens,
      requested_output_tokens: request.requested_output_tokens,
      max_cost_per_million_tokens: request.max_cost_per_million_tokens ?? null,
      max_latency_ms: request.max_latency_ms ?? null,
      min_quality: request.min_quality ?? null,
      min_selection_confidence: request.min_selection_confidence ?? null,
      models,
      // `selectionObservations` are the caller's global value-only history. They must remain in
      // the base request so the remote contextual selector can use them as a cold-start fallback
      // for domains without exact contextual evidence.
      observations: (request.observations ?? []).map((observation) => ({
        arm_id: observation.arm_id,
        pulls: observation.pulls,
        reward_sum: observation.reward_sum,
        failures: observation.failures,
        ...(observation.disabled === undefined ? {} : { disabled: observation.disabled }),
      })),
      provider_health: Object.fromEntries(Object.entries(request.provider_health).map(([provider, health]) => [provider, { registered: true, circuit: health.circuit, credential_ready: health.credential_ready, eligible: health.eligible, attempts: health.attempts, successes: health.successes, failures: health.failures, success_rate: health.success_rate, mean_latency_ms: health.mean_latency_ms }] as [string, BrainProviderHealth])),
      model_health: Object.fromEntries(Object.entries(request.model_health).map(([arm, health]) => [arm, { attempts: health.attempts, successes: health.successes, failures: health.failures, success_rate: health.success_rate, mean_latency_ms: health.mean_latency_ms, last_latency_ms: health.last_latency_ms, circuit: health.circuit }])),
    };
    const response = await client.brainModelSelectContextual({ context: { domain: request.domain, capability: request.capability, risk_class: request.risk_class, task_family: request.task_family ?? null }, base, observations: options.observations?.(request) }, options.requestOptions);
    if (!response.ok || response.mcp.error || response.mcp.result?.isError) throw new ProviderRuntimeError("contextual brain selector returned a refusal");
    const projected = response.mcp.result?.structuredContent as BrainContextualModelSelectionResult | undefined;
    const selection = projected?.selection;
    if (!selection || !isObject(selection)) throw new ProviderRuntimeError("contextual brain selector returned no selection projection");
    const selectedId = typeof selection.selected_model_id === "string" ? selection.selected_model_id : null;
    const exactMatches = selectedId ? request.candidates.filter((candidate) => `${candidate.provider}/${candidate.model}` === selectedId) : [];
    const modelMatches = selectedId && exactMatches.length === 0 ? request.candidates.filter((candidate) => candidate.model === selectedId) : [];
    const matches = exactMatches.length > 0 ? exactMatches : modelMatches;
    const selected = matches.length === 1 ? matches[0] : null;
    return {
      selected_model: selected ? { provider: selected.provider, model: selected.model } : null,
      strategy: "caller_selector",
      ranking: contextualSelectionRanking(selection.ranking, request.candidates),
      abstention_reason: selected ? null : matches.length > 1 ? "contextual selector returned an ambiguous model id" : selection.selection_status || "contextual selector abstained",
      selection_confidence: typeof selection.selection_confidence === "number" ? selection.selection_confidence : undefined,
      min_selection_confidence: typeof selection.min_selection_confidence === "number" ? selection.min_selection_confidence : request.min_selection_confidence ?? null,
    };
  };
}
