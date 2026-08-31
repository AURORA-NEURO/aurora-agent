import { ArgumentError, isObject } from "./errors.js";
import { digestJson } from "./tooling.js";
import type {
  AutonomousExecutionPlan,
  AutonomousModelCandidate,
  AutonomousModelRanking,
  AutonomousSelectionDecision,
} from "./llm.js";
import type { JsonObject } from "./types.js";

/** Provider-free continuation artifacts for deterministic model failover and worker resume. */
export const AUTONOMOUS_MODEL_CONTINUATION_SCHEMA = "bioprism-autonomous-model-continuation/0.1" as const;
export const AUTONOMOUS_MODEL_CONTINUATION_STATE_SCHEMA = "bioprism-autonomous-model-continuation-state/0.1" as const;
export const MAX_AUTONOMOUS_MODEL_CONTINUATION_FAILOVERS = 8;
export const MAX_AUTONOMOUS_MODEL_CONTINUATION_STEPS = MAX_AUTONOMOUS_MODEL_CONTINUATION_FAILOVERS + 1;

export type AutonomousContinuationFailureScope = "model" | "provider";
export type AutonomousContinuationStateStatus = "ready" | "completed" | "exhausted";

export interface AutonomousModelContinuationStep extends JsonObject {
  order: number;
  provider: string;
  model: string;
  model_id: string;
  candidate_digest: string;
  ranking_index: number;
  failure_policy: {
    timeout_with_closed_circuit: "exclude_model";
    retryable_provider_error: "exclude_provider";
  };
}

export interface AutonomousModelContinuationPlan extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_CONTINUATION_SCHEMA;
  selection_digest: string;
  strategy: "fixed_selection_snapshot";
  max_failovers: number;
  steps: AutonomousModelContinuationStep[];
  omitted_eligible_candidates: number;
  plan_digest: string;
  retention: "selection_metadata_only_no_task_prompt_provider_payloads";
  secret_material: "never_returned";
}

export interface AutonomousModelContinuationAttempt extends JsonObject {
  order: number;
  provider: string;
  model: string;
  outcome: "failure" | "success";
  failure_scope: AutonomousContinuationFailureScope | null;
  failure_code: string | null;
  status_code: number | null;
}

export interface AutonomousModelContinuationState extends JsonObject {
  schema: typeof AUTONOMOUS_MODEL_CONTINUATION_STATE_SCHEMA;
  plan_digest: string;
  next_step_index: number | null;
  failovers_used: number;
  excluded_providers: string[];
  excluded_models: string[];
  attempts: AutonomousModelContinuationAttempt[];
  status: AutonomousContinuationStateStatus;
  state_digest: string;
  retention: "selection_metadata_only_no_task_prompt_provider_payloads";
  secret_material: "never_returned";
}

interface ContinuationStateBody {
  schema: typeof AUTONOMOUS_MODEL_CONTINUATION_STATE_SCHEMA;
  plan_digest: string;
  next_step_index: number | null;
  failovers_used: number;
  excluded_providers: string[];
  excluded_models: string[];
  attempts: AutonomousModelContinuationAttempt[];
  status: AutonomousContinuationStateStatus;
  retention: "selection_metadata_only_no_task_prompt_provider_payloads";
  secret_material: "never_returned";
}

function boundedDigest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || /[\u0000-\u001f]/.test(value)) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function boundedFailureCode(value: string | null | undefined): string | null {
  if (value === null || value === undefined) return null;
  return boundedIdentifier("continuation failure code", value, 128);
}

function boundedStatusCode(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 599) throw new ArgumentError("continuation status code is invalid");
  return value as number;
}

function hasExactKeys(value: JsonObject, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  return actual.length === sortedExpected.length && actual.every((key, index) => key === sortedExpected[index]);
}

function modelId(provider: string, model: string): string {
  return `${provider}/${model}`;
}

function candidateDigestInput(candidate: AutonomousModelCandidate): JsonObject {
  return {
    provider: candidate.provider,
    model: candidate.model,
    capabilities: [...(candidate.capabilities ?? [])].sort(),
    context_window_tokens: candidate.context_window_tokens,
    max_output_tokens: candidate.max_output_tokens,
    quality: candidate.quality,
    latency_ms: candidate.latency_ms,
    cost_per_million_tokens: candidate.cost_per_million_tokens,
    reliability: candidate.reliability,
    requires_credential: candidate.requires_credential ?? null,
    enabled: candidate.enabled ?? true,
  };
}

function orderedEligibleRankings(selection: AutonomousSelectionDecision): AutonomousModelRanking[] {
  if (!isObject(selection) || !Array.isArray(selection.ranking)) throw new ArgumentError("autonomous selection ranking is malformed");
  return selection.ranking.filter((row): row is AutonomousModelRanking => isObject(row) && row.eligible === true);
}

/**
 * Compile the first selection into an exact, bounded fallback ladder.
 *
 * This is intentionally separate from provider invocation. The artifact is safe to persist,
 * review, and hand to another worker; it contains no task text, prompt, credential, or response.
 */
export async function compileAutonomousModelContinuationPlan(
  plan: AutonomousExecutionPlan,
  selection: AutonomousSelectionDecision,
  options: { maxFailovers?: number } = {},
): Promise<AutonomousModelContinuationPlan> {
  const maxFailovers = options.maxFailovers ?? 0;
  if (!Number.isSafeInteger(maxFailovers) || maxFailovers < 0 || maxFailovers > MAX_AUTONOMOUS_MODEL_CONTINUATION_FAILOVERS) throw new ArgumentError(`maxFailovers must be within [0, ${MAX_AUTONOMOUS_MODEL_CONTINUATION_FAILOVERS}]`);
  if (!isObject(plan) || !Array.isArray(plan.candidates) || plan.candidates.length === 0) throw new ArgumentError("autonomous continuation requires model candidates");
  if (!isObject(selection) || !isObject(selection.selected_model)) throw new ArgumentError("autonomous continuation requires a selected model");
  const selectedProvider = boundedIdentifier("selected provider", selection.selected_model.provider, 128);
  const selectedModel = boundedIdentifier("selected model", selection.selected_model.model, 512);
  const selectedId = modelId(selectedProvider, selectedModel);
  const candidates = new Map<string, AutonomousModelCandidate>();
  for (const candidate of plan.candidates) {
    if (!isObject(candidate)) throw new ArgumentError("autonomous continuation candidate must be an object");
    const normalized = candidate as unknown as AutonomousModelCandidate;
    const provider = boundedIdentifier("continuation candidate provider", normalized.provider, 128);
    const model = boundedIdentifier("continuation candidate model", normalized.model, 512);
    const id = modelId(provider, model);
    if (candidates.has(id)) throw new ArgumentError(`autonomous continuation contains duplicate model ${id}`);
    candidates.set(id, normalized);
  }
  const selectedCandidate = candidates.get(selectedId);
  if (!selectedCandidate) throw new ArgumentError("selected model is absent from the continuation candidates");
  const eligible = orderedEligibleRankings(selection);
  const byId = new Map<string, { row: AutonomousModelRanking; rankingIndex: number }>();
  selection.ranking.forEach((row, rankingIndex) => {
    if (!isObject(row) || row.eligible !== true) return;
    if (typeof row.provider !== "string" || typeof row.model !== "string") return;
    const id = modelId(row.provider, row.model);
    if (!byId.has(id)) byId.set(id, { row, rankingIndex });
  });
  if (!byId.has(selectedId)) throw new ArgumentError("selected model is not eligible in the selection ranking");
  const orderedIds = [selectedId, ...eligible.map((row) => modelId(row.provider, row.model))];
  const seen = new Set<string>();
  const steps: AutonomousModelContinuationStep[] = [];
  for (const id of orderedIds) {
    if (seen.has(id)) continue;
    seen.add(id);
    const candidate = candidates.get(id);
    const ranked = byId.get(id);
    // Retain the whole bounded ladder: a provider-scoped outage may skip several sibling arms
    // while consuming only one failover transition.
    if (!candidate || !ranked || steps.length >= MAX_AUTONOMOUS_MODEL_CONTINUATION_STEPS) continue;
    const [provider, ...modelParts] = id.split("/");
    const model = modelParts.join("/");
    if (!provider || !model) continue;
    steps.push({
      order: steps.length,
      provider,
      model,
      model_id: id,
      candidate_digest: await digestJson(candidateDigestInput(candidate)),
      ranking_index: ranked.rankingIndex,
      failure_policy: { timeout_with_closed_circuit: "exclude_model", retryable_provider_error: "exclude_provider" },
    });
  }
  if (steps.length === 0 || steps[0]?.model_id !== selectedId) throw new ArgumentError("autonomous continuation could not place the selected model first");
  const selectionDigest = await digestJson(selection);
  const body = {
    schema: AUTONOMOUS_MODEL_CONTINUATION_SCHEMA,
    selection_digest: selectionDigest,
    strategy: "fixed_selection_snapshot" as const,
    max_failovers: maxFailovers,
    steps,
    omitted_eligible_candidates: Math.max(0, eligible.length - steps.length),
    retention: "selection_metadata_only_no_task_prompt_provider_payloads" as const,
    secret_material: "never_returned" as const,
  };
  const continuation = { ...body, plan_digest: await digestJson(body) };
  if (selectedCandidate.provider !== selectedProvider || selectedCandidate.model !== selectedModel) throw new ArgumentError("selected model candidate identity is inconsistent");
  return continuation;
}

async function sealState(body: ContinuationStateBody): Promise<AutonomousModelContinuationState> {
  const state = { ...body, state_digest: await digestJson(body) };
  return state;
}

function stateBody(state: AutonomousModelContinuationState): ContinuationStateBody {
  const { state_digest: _stateDigest, ...body } = state;
  return body;
}

async function assertPlan(plan: AutonomousModelContinuationPlan): Promise<void> {
  if (!isObject(plan) || !hasExactKeys(plan, ["schema", "selection_digest", "strategy", "max_failovers", "steps", "omitted_eligible_candidates", "retention", "secret_material", "plan_digest"]) || plan.schema !== AUTONOMOUS_MODEL_CONTINUATION_SCHEMA || plan.strategy !== "fixed_selection_snapshot" || !Array.isArray(plan.steps) || plan.steps.length === 0 || plan.steps.length > MAX_AUTONOMOUS_MODEL_CONTINUATION_STEPS) throw new ArgumentError("autonomous continuation plan is malformed");
  boundedDigest("continuation selection digest", plan.selection_digest);
  if (!Number.isSafeInteger(plan.max_failovers) || plan.max_failovers < 0 || plan.max_failovers > MAX_AUTONOMOUS_MODEL_CONTINUATION_FAILOVERS) throw new ArgumentError("autonomous continuation plan failover budget is malformed");
  if (!Number.isSafeInteger(plan.omitted_eligible_candidates) || plan.omitted_eligible_candidates < 0) throw new ArgumentError("autonomous continuation omitted candidate count is malformed");
  if (plan.retention !== "selection_metadata_only_no_task_prompt_provider_payloads") throw new ArgumentError("autonomous continuation plan retention contract is malformed");
  if (plan.secret_material !== "never_returned") throw new ArgumentError("autonomous continuation plan secret-material contract is malformed");
  const seenModelIds = new Set<string>();
  for (const [index, step] of plan.steps.entries()) {
    if (!isObject(step) || !hasExactKeys(step, ["order", "provider", "model", "model_id", "candidate_digest", "ranking_index", "failure_policy"]) || step.order !== index) throw new ArgumentError("autonomous continuation plan step is malformed");
    const provider = boundedIdentifier("continuation step provider", step.provider, 128);
    const model = boundedIdentifier("continuation step model", step.model, 512);
    const id = modelId(provider, model);
    if (step.model_id !== id || seenModelIds.has(id)) throw new ArgumentError("autonomous continuation plan step identity is malformed");
    seenModelIds.add(id);
    boundedDigest("continuation candidate digest", step.candidate_digest);
    if (!Number.isSafeInteger(step.ranking_index) || step.ranking_index < 0) throw new ArgumentError("autonomous continuation plan ranking index is malformed");
    if (!isObject(step.failure_policy) || !hasExactKeys(step.failure_policy, ["timeout_with_closed_circuit", "retryable_provider_error"]) || step.failure_policy.timeout_with_closed_circuit !== "exclude_model" || step.failure_policy.retryable_provider_error !== "exclude_provider") throw new ArgumentError("autonomous continuation plan failure policy is malformed");
  }
  boundedDigest("continuation plan digest", plan.plan_digest);
  const { plan_digest: _planDigest, ...body } = plan;
  if (await digestJson(body) !== plan.plan_digest) throw new ArgumentError("autonomous continuation plan digest mismatch");
}

/** Validate a persisted continuation plan before a worker accepts it. */
export async function validateAutonomousModelContinuationPlan(plan: AutonomousModelContinuationPlan): Promise<AutonomousModelContinuationPlan> {
  await assertPlan(plan);
  return structuredClone(plan);
}

/** Create the durable cursor for a compiled continuation plan. */
export async function createAutonomousModelContinuationState(plan: AutonomousModelContinuationPlan): Promise<AutonomousModelContinuationState> {
  await assertPlan(plan);
  return sealState({
    schema: AUTONOMOUS_MODEL_CONTINUATION_STATE_SCHEMA,
    plan_digest: plan.plan_digest,
    next_step_index: 0,
    failovers_used: 0,
    excluded_providers: [],
    excluded_models: [],
    attempts: [],
    status: "ready",
    retention: "selection_metadata_only_no_task_prompt_provider_payloads",
    secret_material: "never_returned",
  });
}

async function assertState(plan: AutonomousModelContinuationPlan, state: AutonomousModelContinuationState): Promise<void> {
  await assertPlan(plan);
  if (!isObject(state) || !hasExactKeys(state, ["schema", "plan_digest", "next_step_index", "failovers_used", "excluded_providers", "excluded_models", "attempts", "status", "retention", "secret_material", "state_digest"]) || state.schema !== AUTONOMOUS_MODEL_CONTINUATION_STATE_SCHEMA || state.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous continuation state is not bound to the supplied plan");
  boundedDigest("continuation state digest", state.state_digest);
  if (await digestJson(stateBody(state)) !== state.state_digest) throw new ArgumentError("autonomous continuation state digest mismatch");
  if (!Number.isSafeInteger(state.failovers_used) || state.failovers_used < 0 || state.failovers_used > plan.max_failovers) throw new ArgumentError("autonomous continuation state failover count is outside its bounds");
  if (!Array.isArray(state.attempts) || state.attempts.length > plan.steps.length) throw new ArgumentError("autonomous continuation state attempts are outside their bounds");
  if (!["ready", "completed", "exhausted"].includes(state.status)) throw new ArgumentError("autonomous continuation state status is malformed");
  if (state.next_step_index !== null && (!Number.isSafeInteger(state.next_step_index) || state.next_step_index < 0 || state.next_step_index >= plan.steps.length)) throw new ArgumentError("autonomous continuation next step is malformed");
  if (state.retention !== "selection_metadata_only_no_task_prompt_provider_payloads") throw new ArgumentError("autonomous continuation state retention contract is malformed");
  if (state.secret_material !== "never_returned") throw new ArgumentError("autonomous continuation state secret-material contract is malformed");
  if (!Array.isArray(state.excluded_providers) || !Array.isArray(state.excluded_models)) throw new ArgumentError("autonomous continuation exclusions are malformed");
  const allowedProviders = new Set(plan.steps.map((step) => step.provider));
  const allowedModels = new Set(plan.steps.map((step) => step.model_id));
  const normalizeExclusions = (name: string, values: string[], allowed: Set<string>): Set<string> => {
    if (values.length > plan.steps.length) throw new ArgumentError(`autonomous continuation ${name} exceed their bounds`);
    const normalized = values.map((value) => boundedIdentifier(name, value, 768));
    const unique = new Set(normalized);
    if (unique.size !== normalized.length || normalized.some((value, index) => value !== [...unique].sort()[index])) throw new ArgumentError(`autonomous continuation ${name} must be sorted and unique`);
    if ([...unique].some((value) => !allowed.has(value))) throw new ArgumentError(`autonomous continuation ${name} references an unknown arm`);
    return unique;
  };
  const excludedProviders = normalizeExclusions("excluded providers", state.excluded_providers, allowedProviders);
  const excludedModels = normalizeExclusions("excluded models", state.excluded_models, allowedModels);
  const expectedAttemptKeys = ["order", "provider", "model", "outcome", "failure_scope", "failure_code", "status_code"];
  let previousOrder = -1;
  let failureCount = 0;
  let successCount = 0;
  const expectedExcludedProviders = new Set<string>();
  const expectedExcludedModels = new Set<string>();
  for (const attempt of state.attempts) {
    if (!isObject(attempt) || !hasExactKeys(attempt, expectedAttemptKeys)) throw new ArgumentError("autonomous continuation state attempt is malformed");
    const order = attempt.order;
    if (!Number.isSafeInteger(order) || order < 0 || order >= plan.steps.length || order <= previousOrder) throw new ArgumentError("autonomous continuation state attempt ordering is malformed");
    previousOrder = order;
    const step = plan.steps[order];
    if (!step) throw new ArgumentError("autonomous continuation state attempt references an unknown step");
    if (attempt.provider !== step.provider || attempt.model !== step.model) throw new ArgumentError("autonomous continuation state attempt identity is malformed");
    if (attempt.outcome === "failure") {
      if (attempt.failure_scope !== "model" && attempt.failure_scope !== "provider") throw new ArgumentError("autonomous continuation state failure scope is malformed");
      failureCount += 1;
      if (attempt.failure_scope === "provider") expectedExcludedProviders.add(step.provider);
      else expectedExcludedModels.add(step.model_id);
    } else if (attempt.outcome === "success") {
      successCount += 1;
      if (attempt.failure_scope !== null || attempt.failure_code !== null) throw new ArgumentError("autonomous continuation successful attempt contains failure metadata");
    } else {
      throw new ArgumentError("autonomous continuation state attempt outcome is malformed");
    }
    boundedFailureCode(attempt.failure_code);
    boundedStatusCode(attempt.status_code);
  }
  if (failureCount !== state.failovers_used) throw new ArgumentError("autonomous continuation failover count does not match attempts");
  if (successCount > 1 || (successCount > 0 && state.attempts[state.attempts.length - 1]?.outcome !== "success")) throw new ArgumentError("autonomous continuation state has an invalid terminal attempt");
  if (excludedProviders.size !== expectedExcludedProviders.size || [...excludedProviders].some((value) => !expectedExcludedProviders.has(value)) || excludedModels.size !== expectedExcludedModels.size || [...excludedModels].some((value) => !expectedExcludedModels.has(value))) throw new ArgumentError("autonomous continuation state exclusions do not match attempts");
  if (state.attempts.length > 0 && state.attempts[0]?.order !== 0) throw new ArgumentError("autonomous continuation state must begin with the selected model");
  const expectedNextIndex = plan.steps.findIndex((step, index) => index > previousOrder && !expectedExcludedProviders.has(step.provider) && !expectedExcludedModels.has(step.model_id));
  const expectedNext = expectedNextIndex < 0 ? null : expectedNextIndex;
  if (state.status === "ready") {
    if (successCount > 0 || expectedNext === null || state.next_step_index !== expectedNext) throw new ArgumentError("autonomous continuation ready cursor is inconsistent");
  } else if (state.status === "completed") {
    if (state.next_step_index !== null || successCount !== 1) throw new ArgumentError("autonomous continuation completed cursor is inconsistent");
  } else if (state.next_step_index !== null || expectedNext !== null || state.attempts.length === 0) {
    throw new ArgumentError("autonomous continuation exhausted cursor is inconsistent");
  }
}

/** Validate a persisted continuation cursor and its binding to the exact plan. */
export async function validateAutonomousModelContinuationState(plan: AutonomousModelContinuationPlan, state: AutonomousModelContinuationState): Promise<AutonomousModelContinuationState> {
  await assertState(plan, state);
  return structuredClone(state);
}

/** Advance a cursor after a metadata-only provider failure; the next model is never reselected. */
export async function advanceAutonomousModelContinuationState(
  plan: AutonomousModelContinuationPlan,
  state: AutonomousModelContinuationState,
  event: { provider: string; model: string; failureScope: AutonomousContinuationFailureScope; failureCode?: string | null; statusCode?: number | null },
): Promise<AutonomousModelContinuationState> {
  await assertState(plan, state);
  if (state.status !== "ready" || state.next_step_index === null) throw new ArgumentError("autonomous continuation is not ready for another failure");
  if (state.failovers_used >= plan.max_failovers) throw new ArgumentError("autonomous continuation failover budget is exhausted");
  const current = plan.steps[state.next_step_index];
  if (!current || current.provider !== event.provider || current.model !== event.model) throw new ArgumentError("continuation failure does not match the current step");
  if (event.failureScope !== "model" && event.failureScope !== "provider") throw new ArgumentError("continuation failure scope is malformed");
  boundedStatusCode(event.statusCode);
  const excludedProviders = new Set(state.excluded_providers);
  const excludedModels = new Set(state.excluded_models);
  if (event.failureScope === "provider") excludedProviders.add(current.provider);
  else excludedModels.add(current.model_id);
  const attempts = [...state.attempts, { order: current.order, provider: current.provider, model: current.model, outcome: "failure" as const, failure_scope: event.failureScope, failure_code: boundedFailureCode(event.failureCode), status_code: event.statusCode ?? null }];
  const nextStepIndex = plan.steps.findIndex((step, index) => index > current.order && !excludedProviders.has(step.provider) && !excludedModels.has(step.model_id));
  const failoversUsed = state.failovers_used + 1;
  return sealState({
    schema: AUTONOMOUS_MODEL_CONTINUATION_STATE_SCHEMA,
    plan_digest: plan.plan_digest,
    next_step_index: nextStepIndex < 0 ? null : nextStepIndex,
    failovers_used: failoversUsed,
    excluded_providers: [...excludedProviders].sort(),
    excluded_models: [...excludedModels].sort(),
    attempts,
    status: nextStepIndex < 0 ? "exhausted" : "ready",
    retention: "selection_metadata_only_no_task_prompt_provider_payloads",
    secret_material: "never_returned",
  });
}

/** Seal a successful terminal attempt without retaining its response. */
export async function completeAutonomousModelContinuationState(
  plan: AutonomousModelContinuationPlan,
  state: AutonomousModelContinuationState,
  event: { provider: string; model: string; statusCode?: number | null },
): Promise<AutonomousModelContinuationState> {
  await assertState(plan, state);
  if (state.status !== "ready" || state.next_step_index === null) throw new ArgumentError("autonomous continuation is not ready for completion");
  const current = plan.steps[state.next_step_index];
  if (!current || current.provider !== event.provider || current.model !== event.model) throw new ArgumentError("continuation success does not match the current step");
  boundedStatusCode(event.statusCode);
  return sealState({
    ...stateBody(state),
    next_step_index: null,
    attempts: [...state.attempts, { order: current.order, provider: current.provider, model: current.model, outcome: "success", failure_scope: null, failure_code: null, status_code: event.statusCode ?? null }],
    status: "completed",
  });
}

/** Project a fixed continuation step as a normal selection decision for existing observers. */
export function continuationSelectionDecision(selection: AutonomousSelectionDecision, step: AutonomousModelContinuationStep): AutonomousSelectionDecision {
  return {
    ...selection,
    selected_model: { provider: step.provider, model: step.model },
    abstention_reason: null,
  };
}
