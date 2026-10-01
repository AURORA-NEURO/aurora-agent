import { ArgumentError, CredentialError, ProviderRuntimeError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import { boundedIdentifier, boundedText, bytes } from "./autonomous-validation.js";
import { AUTONOMOUS_PROMPT_SCHEMA, assembleAutonomousPrompt } from "./autonomous-blueprint.js";
import {
  AutonomousPromptRegistry,
  AutonomousPromptTemplate,
  selectAdaptiveAutonomousPrompts,
  type AutonomousPromptAdaptiveSelectionJSON,
  type AutonomousPromptLearningState,
  type AutonomousPromptLearningStateJSON,
} from "./autonomous-prompt-registry.js";
import type {
  AutonomousDomainProfile,
  AutonomousOrderedStepPlanRequest,
  AutonomousOrderedStepPlanStep,
  AutonomousPromptChunk,
  AutonomousPromptResult,
  AutonomousProviderPlanningOptions,
  AutonomousRouteProposal,
  AutonomousRunOptions,
  AutonomousTaskBlueprint,
  PreparedProviderPlanning,
  RenderedAutonomousRunPrompt,
} from "./autonomous-agent-contracts.js";
import type {
  AutonomousPlanningFailureProjection,
  BrainBanditContext,
  BrainModelSelectionContext,
  JsonObject,
} from "./types.js";
import type {
  AutonomousExecutionPlan,
  AutonomousSelectionDecision,
  ProviderMessage,
  ProviderRequest,
  ProviderResponse,
} from "./llm.js";
import { digestCanonicalJsonText, digestJson } from "./tooling.js";

/** Versioned response contract for bounded ordering refinements of an existing step graph. */
export const AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA = "bioprism-typescript-autonomous-ordered-step-plan-refinement/0.1" as const;

export async function renderAutonomousRunPrompt(
  task: string,
  blueprint: AutonomousTaskBlueprint,
  route: AutonomousRouteProposal | null,
  options: Pick<AutonomousRunOptions, "promptTemplate" | "promptRegistry" | "promptSelection" | "promptStage" | "promptLearningState" | "promptLearningExploration">,
  contextIds: readonly string[] = blueprint.prompt.included_context_ids,
): Promise<RenderedAutonomousRunPrompt | null> {
  const domain = blueprint.domain_profile.domain;
  return renderVersionedAutonomousPrompt(
    {
      task,
      objective: task,
      requirement: {
        domain,
        stage_id: options.promptStage ?? "answer",
        objective: task,
        workflow_id: blueprint.workflow.workflow_id,
        required_capabilities: [...blueprint.required_capabilities],
      },
      route: {
        route_digest: route?.route_digest ?? blueprint.route_digest,
        selected_domains: route ? [...route.selected_domains] : [domain],
        primary_domain: route?.primary_domain ?? domain,
        cross_domain: route?.cross_domain ?? domain === "cross_domain",
      },
      context_ids: [...contextIds],
    },
    options,
  );
}

async function renderVersionedAutonomousPrompt(
  context: Readonly<Record<string, unknown>>,
  options: Pick<AutonomousRunOptions, "promptTemplate" | "promptRegistry" | "promptSelection" | "promptStage" | "promptLearningState" | "promptLearningExploration">,
): Promise<RenderedAutonomousRunPrompt | null> {
  const template = options.promptTemplate;
  const registry = options.promptRegistry;
  const selection = options.promptSelection;
  if (template !== undefined && !(template instanceof AutonomousPromptTemplate)) throw new ArgumentError("autonomous promptTemplate must be an AutonomousPromptTemplate");
  if (registry !== undefined && !(registry instanceof AutonomousPromptRegistry)) throw new ArgumentError("autonomous promptRegistry must be an AutonomousPromptRegistry");
  if (template !== undefined && (registry !== undefined || selection !== undefined)) throw new ArgumentError("autonomous promptTemplate cannot be combined with promptRegistry or promptSelection");
  if (selection !== undefined && registry === undefined) throw new ArgumentError("autonomous promptSelection requires promptRegistry");
  if (options.promptLearningState !== undefined && registry === undefined) throw new ArgumentError("autonomous promptLearningState requires promptRegistry");
  if (options.promptLearningState !== undefined && selection !== undefined) throw new ArgumentError("autonomous promptLearningState cannot be combined with promptSelection");
  if (registry === undefined && template === undefined) return null;
  const requirement = context.requirement;
  if (!requirement || typeof requirement !== "object") throw new ArgumentError("autonomous prompt context requirement is malformed");
  const requirementRecord = requirement as Record<string, unknown>;
  const domainValue = requirementRecord.domain;
  const stage = boundedIdentifier("autonomous promptStage", requirementRecord.stage_id ?? options.promptStage ?? "answer");
  if (typeof domainValue !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(domainValue as AutonomousDomainName)) throw new ArgumentError("autonomous prompt context domain is unsupported");
  const domain = domainValue as AutonomousDomainName;
  const normalizedContext = { ...context, requirement: { ...requirementRecord, domain, stage_id: stage } } as const;
  if (template !== undefined) {
    const rendered = await template.renderTransient(normalizedContext);
    return { messages: rendered.messages, metadata: rendered.metadata, mode: "versioned_template" };
  }
  const request = {
    // Prompt capabilities are a separate reviewed namespace from model capabilities. A domain
    // blueprint may require "reasoning" or "code" even when a prompt manifest intentionally
    // advertises only its rendering concerns, so selection starts with no implicit model labels.
    domain,
    stage,
    requiredCapabilities: [],
  } as const;
  const adaptive = options.promptLearningState === undefined
    ? null
    : selectAdaptiveAutonomousPrompts(registry!, [request], { state: options.promptLearningState, exploration: options.promptLearningExploration });
  const resolvedSelection = selection ?? adaptive?.plan ?? registry!.selectFor([request]);
  const rendered = await registry!.render(resolvedSelection, normalizedContext);
  const metadata = adaptive === null
    ? rendered.metadata
    : {
      ...rendered.metadata,
      adaptive_selection_digest: adaptive.selectionDigest,
      adaptive_arm_id: adaptive.armIds[0] ?? null,
      adaptive_generation: adaptive.generation,
      selection_policy: "ucb1_explicit_evaluator_v1",
      adaptive_selection: adaptive.toJSON(),
    };
  return { messages: rendered.messages, metadata, mode: "registry_selection" };
}

function planningResponseSchema(ids: readonly string[], focusField: "focus_stage_ids" | "focus_child_ids" | "focus_step_ids"): JsonObject {
  const enumValues = [...ids];
  return {
    type: "object",
    additionalProperties: false,
    properties: {
      priority_order: { type: "array", items: { type: "string", enum: enumValues } },
      [focusField]: { type: "array", items: { type: "string", enum: enumValues } },
      review_required: { type: "boolean" },
      confidence: { type: "number", minimum: 0, maximum: 1 },
      abstain: { type: "boolean" },
    },
    required: ["priority_order", focusField, "review_required", "confidence", "abstain"],
  };
}

/**
 * Bind provider planning to the same reviewed prompt controls as ordinary execution.
 *
 * The legacy assembled prompt remains the bounded source of planning-contract context and
 * input-budget accounting. A versioned renderer replaces only the planner framing/task
 * messages; the contract and optional caller context are inserted before the rendered user
 * message. The raw messages never enter a planning result or digest projection.
 */
async function prepareVersionedPlanningMessages(
  plannerTask: string,
  profile: AutonomousDomainProfile,
  prompt: AutonomousPromptResult,
  planningContext: readonly AutonomousPromptChunk[],
  options: AutonomousProviderPlanningOptions,
): Promise<{ messages: readonly ProviderMessage[]; promptDigest: string; adaptiveSelection?: AutonomousPromptAdaptiveSelectionJSON }> {
  const stage = options.promptStage ?? "planning";
  const rendered = await renderVersionedAutonomousPrompt(
    {
      task: plannerTask,
      objective: plannerTask,
      requirement: {
        domain: profile.domain,
        stage_id: stage,
        objective: plannerTask,
        workflow_id: profile.workflow.workflow_id,
        required_capabilities: [...profile.required_model_capabilities],
      },
      route: {
        route_digest: null,
        selected_domains: [profile.domain],
        primary_domain: profile.domain,
        cross_domain: profile.domain === "cross_domain",
      },
      context_ids: planningContext.map((chunk) => chunk.id),
    },
    {
      promptTemplate: options.promptTemplate,
      promptRegistry: options.promptRegistry,
      promptSelection: options.promptSelection,
      promptLearningState: options.promptLearningState,
      promptLearningExploration: options.promptLearningExploration,
    },
  );
  const legacyMessages = prompt.messages.map(({ role, content }) => ({ role, content } satisfies ProviderMessage));
  if (rendered === null) return { messages: legacyMessages, promptDigest: prompt.prompt_digest };

  const supportingMessages = prompt.messages
    .filter((message) => !["domain-system", "domain-developer", "task"].includes(message.source_id))
    .map(({ role, content }) => ({ role, content } satisfies ProviderMessage));
  const messages = [...rendered.messages];
  const lastUserIndex = messages.reduce((index, message, current) => message.role === "user" ? current : index, -1);
  const insertionIndex = lastUserIndex < 0 ? messages.length : lastUserIndex;
  messages.splice(insertionIndex, 0, ...supportingMessages);
  const promptDigest = await digestJson({
    schema: AUTONOMOUS_PROMPT_SCHEMA,
    base_prompt_digest: prompt.prompt_digest,
    rendered_prompt: rendered.metadata,
    message_digest: await digestJson(messages),
  });
  return { messages, promptDigest, adaptiveSelection: rendered.metadata.adaptive_selection };
}

export async function prepareProviderPlanning(
  profile: AutonomousDomainProfile,
  blueprint: AutonomousTaskBlueprint,
  ids: readonly string[],
  focusField: "focus_stage_ids" | "focus_child_ids",
  contract: JsonObject,
  options: AutonomousProviderPlanningOptions,
): Promise<PreparedProviderPlanning> {
  const taskMessage = blueprint.prompt.messages.find((message) => message.source_id === "task");
  if (!taskMessage) throw new ProviderRuntimeError("provider planning blueprint has no bounded task message");
  const plannerTask = boundedText(
    "autonomous provider planning task",
    "Propose a bounded refinement for the reviewed autonomous workflow. Return only the required JSON object. "
      + "Reorder and focus existing identifiers only; preserve every existing dependency. Do not add tools, "
      + "credentials, domains, permissions, effects, factual claims, or completed evidence. Mark review_required "
      + "when a human should inspect the proposal. Original task:\n\n"
      + taskMessage.content,
    32_000,
  );
  const planningContext: AutonomousPromptChunk[] = [
    { id: "planning-contract", content: JSON.stringify(contract), required: true, priority: 100 },
    ...(options.context ?? []),
  ];
  const prompt = await assembleAutonomousPrompt(profile, plannerTask, {
    context: planningContext,
    maxInputTokens: options.maxInputTokens,
    outputContract: `Return JSON with priority_order, ${focusField}, review_required, confidence, and abstain. Use only identifiers from the planning contract.`,
  });
  const plannerMessages = await prepareVersionedPlanningMessages(plannerTask, profile, prompt, planningContext, options);
  const responseSchema = planningResponseSchema(ids, focusField);
  const requiredCapabilities = [...new Set([...blueprint.required_capabilities, "structured_output"])];
  // Provider planning is its own learner context. The execution blueprint's digest is keyed
  // by the execution capability, while this request is selected as a planning decision; using
  // the blueprint digest here makes learner-backed planning reject its own request identity.
  const planningLearnerContext: BrainBanditContext = {
    domain: profile.domain,
    capability: "planning",
    risk_class: profile.risk_class,
    task_family: profile.workflow.workflow_id,
  };
  const learningContextDigest = await digestCanonicalJsonText(JSON.stringify(planningLearnerContext));
  const request: ProviderRequest = {
    model: "selection-delegated",
    messages: plannerMessages.messages,
    maxOutputTokens: options.maxOutputTokens ?? 1_024,
    ...(options.temperature === undefined ? {} : { temperature: options.temperature }),
    requireJson: true,
    responseSchema,
    ...(options.runId === undefined ? {} : { idempotencyKey: boundedIdentifier("planning run id", options.runId) }),
  };
  return {
    prompt,
    promptDigest: plannerMessages.promptDigest,
    adaptiveSelection: plannerMessages.adaptiveSelection,
    plan: {
      task: plannerTask,
      domain: profile.domain,
      capability: "planning",
      riskClass: profile.risk_class,
      taskFamily: profile.workflow.workflow_id,
      learningContextDigest,
      requiredCapabilities,
      maxCostPerMillionTokens: options.maxCostPerMillionTokens,
      maxLatencyMs: options.maxLatencyMs,
      minQuality: options.minQuality,
      minSelectionConfidence: options.minSelectionConfidence,
      selectionWeights: options.selectionWeights,
      selectionObservations: options.selectionObservations,
      candidates: options.candidates ?? [],
      request,
    },
    learningContext: planningLearnerContext,
    learningContextDigest,
  };
}

export function planningModelProjection(selection: AutonomousSelectionDecision): { provider: string; model: string } | null {
  return selection.selected_model === null ? null : { ...selection.selected_model };
}

export async function planningOutcomeDigest(
  execution: { selection: AutonomousSelectionDecision; response: ProviderResponse },
  learningContextDigest: string | null = null,
  promptDigest: string | null = null,
): Promise<string> {
  const responseDigest = await digestJson({
    provider: execution.response.provider,
    model: execution.response.model,
    status_code: execution.response.statusCode,
    request_id: execution.response.requestId,
    usage: execution.response.usage,
    text: execution.response.text,
    structured: execution.response.structured,
  });
  return digestJson({ selection: execution.selection, response_digest: responseDigest, learning_context_digest: learningContextDigest, prompt_digest: promptDigest });
}

/** Project a provider planning failure without retaining its message, payload, or credential. */
export function planningProviderFailureProjection(error: ProviderRuntimeError | CredentialError): AutonomousPlanningFailureProjection {
  return {
    error_class: error instanceof CredentialError ? "CredentialError" : "ProviderRuntimeError",
    code: error instanceof CredentialError ? "credential" : error.code,
    retryable: error instanceof CredentialError ? false : error.retryable,
    status_code: error instanceof CredentialError ? null : error.statusCode ?? null,
    circuit_open: error instanceof CredentialError ? false : error.circuitOpen,
    retention: "metadata_only;provider_error_message_and_payloads_not_retained",
    secret_material: "never_returned",
  };
}

/** Digest only the stable metadata of a provider planning failure. */
export async function planningProviderFailureDigest(error: ProviderRuntimeError | CredentialError): Promise<string> {
  const projection = planningProviderFailureProjection(error);
  return digestJson({
    error_class: projection.error_class,
    code: projection.code,
    retryable: projection.retryable,
    status_code: projection.status_code,
    circuit_open: projection.circuit_open,
    provider: error instanceof ProviderRuntimeError ? error.provider ?? null : null,
  });
}

export function validateOrderedStepPlanningGraph(steps: readonly AutonomousOrderedStepPlanStep[]): string[] {
  if (!Array.isArray(steps) || steps.length === 0 || steps.length > 128) throw new ProviderRuntimeError("ordered-step provider planning steps are outside their bounds");
  const ids = steps.map((step) => {
    if (!isObject(step) || typeof step.id !== "string" || /^[A-Za-z0-9_.:-]{1,256}$/.exec(step.id)?.[0] !== step.id) throw new ProviderRuntimeError("ordered-step provider planning step is malformed");
    if (typeof step.domain !== "string" || /^[A-Za-z0-9_.-]{1,128}$/.exec(step.domain)?.[0] !== step.domain || !AUTONOMOUS_DOMAIN_NAMES.includes(step.domain as AutonomousDomainName)) throw new ProviderRuntimeError(`ordered-step planning step ${step.id} has an unsupported domain`);
    if (typeof step.capability !== "string" || /^[A-Za-z0-9_.-]{1,128}$/.exec(step.capability)?.[0] !== step.capability || bytes(step.capability) > 128) throw new ProviderRuntimeError(`ordered-step planning step ${step.id} capability is outside its bounds`);
    if (typeof step.objective !== "string" || !step.objective.trim() || step.objective.includes("\u0000") || bytes(step.objective) > 16_000) throw new ProviderRuntimeError(`ordered-step planning step ${step.id} objective is outside its bounds`);
    if (step.required !== undefined && typeof step.required !== "boolean") throw new ProviderRuntimeError(`ordered-step planning step ${step.id} required must be a boolean`);
    return step.id;
  });
  if (new Set(ids).size !== ids.length) throw new ProviderRuntimeError("ordered-step provider planning steps are duplicated");
  const known = new Set(ids);
  const indegree = new Map(ids.map((id) => [id, 0]));
  const dependents = new Map(ids.map((id) => [id, [] as string[]]));
  for (const step of steps) {
    const dependencies = step.depends_on === undefined ? [] : step.depends_on;
    if (!Array.isArray(dependencies) || dependencies.some((dependency) => typeof dependency !== "string" || /^[A-Za-z0-9_.:-]{1,256}$/.exec(dependency)?.[0] !== dependency || !known.has(dependency) || dependency === step.id)) throw new ProviderRuntimeError("ordered-step provider planning dependencies are not closed");
    if (new Set(dependencies).size !== dependencies.length) throw new ProviderRuntimeError("ordered-step provider planning dependencies are duplicated");
    indegree.set(step.id, dependencies.length);
    for (const dependency of dependencies) dependents.get(dependency)!.push(step.id);
  }
  const ready = ids.filter((id) => indegree.get(id) === 0);
  let visited = 0;
  for (let cursor = 0; cursor < ready.length; cursor++) {
    const id = ready[cursor]!;
    visited++;
    for (const dependent of dependents.get(id)!) {
      const next = indegree.get(dependent)! - 1;
      indegree.set(dependent, next);
      if (next === 0) ready.push(dependent);
    }
  }
  if (visited !== ids.length) throw new ProviderRuntimeError("ordered-step provider planning dependencies contain a cycle");
  return ids;
}

export async function prepareOrderedStepPlanning(
  request: AutonomousOrderedStepPlanRequest,
  profile: AutonomousDomainProfile,
  ids: readonly string[],
  taskDigest: string,
  basePlanDigest: string,
  options: AutonomousProviderPlanningOptions,
): Promise<PreparedProviderPlanning> {
  const taskText = boundedText("ordered-step provider planning task", request.task, 32_000);
  const plannerTask = boundedText(
    "ordered-step provider planning prompt",
    "Propose a bounded ordering and focus refinement for the reviewed step graph. Return only the required JSON object. "
      + "Use every existing step identifier exactly once in priority_order. Preserve dependency order. Do not add, remove, "
      + "rewrite, authorize, or execute tools, arguments, credentials, permissions, effects, claims, or external writes. "
      + "Mark review_required when a human should inspect the proposal. Original task:\n\n" + taskText,
    32_000,
  );
  const contract: JsonObject = {
    schema: AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA,
    task_digest: taskDigest,
    base_plan_digest: basePlanDigest,
    protected_contract_digest: request.protectedContractDigest ?? null,
    step_catalogue: request.steps.map((step) => ({
      id: step.id,
      domain: step.domain,
      capability: step.capability,
      objective: step.objective,
      depends_on: [...(step.depends_on ?? [])],
      required: step.required ?? true,
    })),
    reconciliation: "priority_order_must_contain_each_existing_step_exactly_once_and_respect_dependencies",
    does_not_authorize: ["tools", "arguments", "credentials", "permissions", "effects", "claims", "external_writes"],
  };
  const planningContext: AutonomousPromptChunk[] = [
    { id: "planning-contract", content: JSON.stringify(contract), required: true, priority: 100 },
    ...(request.context ?? []),
    ...(options.context ?? []),
  ];
  const prompt = await assembleAutonomousPrompt(profile, plannerTask, {
    context: planningContext,
    maxInputTokens: options.maxInputTokens,
    outputContract: "Return JSON with priority_order, focus_step_ids, review_required, confidence, and abstain. Use only identifiers from the planning contract.",
  });
  const responseSchema = planningResponseSchema(ids, "focus_step_ids");
  const selectionContext: BrainModelSelectionContext = {
    domain: profile.domain,
    capability: request.capability ?? "planning",
    risk_class: profile.risk_class,
    task_family: "ordered_step_plan",
  };
  const plannerMessages = await prepareVersionedPlanningMessages(plannerTask, profile, prompt, planningContext, options);
  // Only the stable four-field learner identity is hashed. Descriptive selection metadata
  // cannot be passed through this digest because the local, Rust, and Python learners all
  // normalize the same bounded BrainBanditContext shape before selecting or settling.
  const learningContext: BrainBanditContext = {
    domain: selectionContext.domain,
    capability: selectionContext.capability,
    risk_class: selectionContext.risk_class,
    task_family: selectionContext.task_family ?? null,
  };
  const learningContextDigest = await digestCanonicalJsonText(JSON.stringify(learningContext));
  const requiredCapabilities = [...new Set([...profile.required_model_capabilities, "structured_output"])]
  const plan: AutonomousExecutionPlan = {
    task: plannerTask,
    domain: profile.domain,
    capability: request.capability ?? "planning",
    riskClass: profile.risk_class,
    taskFamily: "ordered_step_plan",
    learningContextDigest,
    requiredCapabilities,
    maxCostPerMillionTokens: options.maxCostPerMillionTokens,
    maxLatencyMs: options.maxLatencyMs,
    minQuality: options.minQuality,
    minSelectionConfidence: options.minSelectionConfidence,
    selectionWeights: options.selectionWeights,
    selectionObservations: options.selectionObservations,
    candidates: options.candidates ?? [],
    request: {
      model: "selection-delegated",
      messages: plannerMessages.messages,
      maxOutputTokens: options.maxOutputTokens ?? 1_024,
      ...(options.temperature === undefined ? {} : { temperature: options.temperature }),
      requireJson: true,
      responseSchema,
      ...(options.runId === undefined ? {} : { idempotencyKey: boundedIdentifier("ordered-step planning run id", options.runId) }),
    },
  };
  return { prompt, promptDigest: plannerMessages.promptDigest, adaptiveSelection: plannerMessages.adaptiveSelection, plan, learningContext, learningContextDigest };
}
