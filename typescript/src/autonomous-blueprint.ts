/** Domain-aware prompt assembly and reviewed task blueprint compilation. */

import { ArgumentError, isObject } from "./errors.js";
import { boundedText, bytes, assertSafeTransientValue } from "./autonomous-validation.js";
import { digestCanonicalJsonText, digestJson } from "./tooling.js";
import { buildAutonomousEvidencePlan, type AutonomousEvidencePlan } from "./autonomous-evidence.js";
import { buildAutonomousDomainResponseContract } from "./autonomous-domain-response.js";
import { autonomousDomainPolicy } from "./autonomous-domain-policy.js";
import {
  autonomousDomainTaskLens,
  autonomousTaskLensPromptContract,
} from "./autonomous-task-lens.js";
import { autonomousTaskIntentPromptContract, inferAutonomousTaskIntent } from "./autonomous-task-intent.js";
import { autonomousTaskDecisionPromptContract, inferAutonomousTaskDecision } from "./autonomous-task-decision.js";
import { routeAutonomousCapability, type AutonomousCapabilityRoute } from "./autonomous-capability-routing.js";
import { buildDomainPack } from "./autonomous-domain-catalog.js";
import { compileAutonomousPlan } from "./autonomous-plan-compiler.js";
import { compileAutonomousWorkflowStageExecutionPlan } from "./autonomous-workflow-stage-plan.js";
import type { BrainModelSelectionContext } from "./types.js";
import type {
  AutonomousDomainProfile,
  AutonomousPromptChunk,
  AutonomousPromptMessage,
  AutonomousPromptResult,
  AutonomousTaskBlueprint,
} from "./autonomous-agent-contracts.js";

export const AUTONOMOUS_PROMPT_SCHEMA = "bioprism-python-autonomous-prompt/0.1" as const;

export async function buildTaskBlueprint(
  profile: AutonomousDomainProfile,
  task: string,
  options: {
    taskDigest?: string;
    routeDigest?: string;
    capability?: string;
    riskClass?: string;
    capabilityRoute?: AutonomousCapabilityRoute;
    context?: readonly AutonomousPromptChunk[];
    constraints?: readonly string[];
    desiredOutputs?: readonly string[];
    maxInputTokens?: number;
    activeToolNames?: readonly string[];
    selectedToolNames?: readonly string[];
    structuredDomainResponse?: boolean;
  } = {},
): Promise<AutonomousTaskBlueprint> {
  const taskText = boundedText("autonomous task blueprint objective", task, 32_000);
  const taskDigest = options.taskDigest ?? await digestJson({ task: taskText });
  if (typeof options.routeDigest !== "string" || !/^[0-9a-f]{64}$/.test(options.routeDigest)) throw new ArgumentError("autonomous task blueprint routeDigest must be a lowercase SHA-256 digest");
  const activeToolNames = [...new Set(options.activeToolNames ?? [])];
  const selectedToolNames = [...new Set(options.selectedToolNames ?? activeToolNames)];
  const domainPolicy = autonomousDomainPolicy(profile.domain);
  const taskLens = autonomousDomainTaskLens(profile.domain);
  const capabilityRoute = options.capabilityRoute ?? routeAutonomousCapability(taskText, profile.domain, options.capability === undefined ? {} : { explicitCapability: options.capability });
  const effectiveCapability = options.capability ?? capabilityRoute.selected_capability ?? profile.default_capability;
  const taskIntent = inferAutonomousTaskIntent({
    task: taskText,
    taskDigest,
    domain: profile.domain,
    capability: effectiveCapability,
    riskClass: options.riskClass ?? profile.risk_class,
    workflowId: profile.workflow.workflow_id,
    lens: taskLens,
    constraints: options.constraints,
    desiredOutputs: options.desiredOutputs,
  });
  const taskDecision = inferAutonomousTaskDecision({
    intent: taskIntent,
    lens: taskLens,
    policy: domainPolicy,
    requiredModelCapabilities: profile.required_model_capabilities,
  });
  const pack = await buildDomainPack(profile);
  const evidencePlan = await buildAutonomousEvidencePlan([profile.workflow]);
  const responseContract = options.structuredDomainResponse === true
    ? await buildAutonomousDomainResponseContract(profile)
    : null;
  const prompt = await assembleAutonomousPrompt(profile, taskText, {
    context: options.context,
    maxInputTokens: options.maxInputTokens ?? domainPolicy.max_input_tokens,
    stageIds: profile.workflow.stages.map((stage) => stage.id),
    evidencePlan,
    outputContract: responseContract?.prompt_contract,
  });
  const plan = await compileAutonomousPlan(profile, taskText, {
    taskDigest,
    capability: effectiveCapability,
    activeToolNames,
    selectedToolNames,
    selectedToolOrder: selectedToolNames,
    ...(responseContract ? { responseContractDigest: responseContract.contract_digest } : {}),
  });
  const selectionContext: BrainModelSelectionContext = {
    domain: profile.domain,
    capability: effectiveCapability,
    risk_class: taskIntent.risk_class,
    task_family: profile.workflow.workflow_id,
    task_lens_id: taskLens.lens_id,
    task_lens_digest: taskLens.lens_digest,
    task_lens_model_capability_hints: [...taskLens.model_capability_hints],
    task_lens_evaluator_signals: [...taskLens.evaluator_signals],
    task_lens_planning_dimensions: [...taskLens.planning_dimensions],
    task_intent_id: taskIntent.intent_id,
    task_intent_digest: taskIntent.intent_digest,
    task_intent_action_mode: taskIntent.action_mode,
    task_intent_requested_effect: taskIntent.requested_effect,
    task_intent_evidence_mode: taskIntent.evidence_mode,
    task_intent_ambiguity_flags: [...taskIntent.ambiguity_flags],
    task_decision_id: taskDecision.decision_id,
    task_decision_digest: taskDecision.decision_digest,
    task_decision_posture: taskDecision.posture,
    task_decision_recommended_path: taskDecision.recommended_path,
    task_decision_approval_requirements: [...taskDecision.approval_requirements],
    task_decision_review_reasons: [...taskDecision.review_reasons],
    capability_route_digest: capabilityRoute.route_digest,
    capability_route_reason: capabilityRoute.reason,
    capability_route_confidence: capabilityRoute.confidence,
  };
  // Match the Rust/Python context identity byte-for-byte: field order is part of this
  // cross-language value contract, while task text and provider payloads stay outside it.
  // Descriptive task-lens fields are deliberately excluded from the learner
  // identity. They guide planning and auditing, while the shared Rust/Python
  // contextual bandit remains keyed only by the stable four-field identity.
  const learningContext = {
    domain: selectionContext.domain,
    capability: selectionContext.capability,
    risk_class: selectionContext.risk_class,
    task_family: selectionContext.task_family ?? null,
  };
  const learningContextDigest = await digestCanonicalJsonText(JSON.stringify(learningContext));
  const baseBlueprint = {
    schema: "bioprism-python-autonomous-task/0.1",
    task_digest: taskDigest,
    route_digest: options.routeDigest,
    domain_profile: profile,
    domain_pack: pack,
    workflow: profile.workflow,
    evidence_plan: evidencePlan.toJSON(),
    selection_context: selectionContext,
    learning_context_digest: learningContextDigest,
    required_capabilities: profile.required_model_capabilities,
    domain_policy: domainPolicy,
    task_lens: taskLens,
    task_intent: taskIntent,
    task_decision: taskDecision,
    capability_route: capabilityRoute,
    prompt,
    plan,
    ...(responseContract ? { response_contract: responseContract } : {}),
    execution: "not_started",
    credential_posture: "caller_supplied_opaque_handle_not_returned",
  } as Omit<AutonomousTaskBlueprint, "stage_execution_plans">;
  const stageExecutionPlans = await Promise.all(profile.workflow.stages.map((stage) => compileAutonomousWorkflowStageExecutionPlan(baseBlueprint as AutonomousTaskBlueprint, stage, {
    activeToolNames,
    selectedToolNames,
  })));
  return { ...baseBlueprint, stage_execution_plans: stageExecutionPlans } as AutonomousTaskBlueprint;
}

/** Assemble the bounded domain prompt locally, retaining exact inclusion/omission evidence. */
export async function assembleAutonomousPrompt(
  profile: AutonomousDomainProfile,
  task: string,
  options: { context?: readonly AutonomousPromptChunk[]; outputContract?: string; maxInputTokens?: number; stageIds?: readonly string[]; evidencePlan?: AutonomousEvidencePlan } = {},
): Promise<AutonomousPromptResult> {
  const taskText = boundedText("autonomous prompt task", task, 32_000);
  const maxInputTokens = options.maxInputTokens ?? 8_192;
  if (!Number.isSafeInteger(maxInputTokens) || maxInputTokens < 128 || maxInputTokens > 1_000_000) throw new ArgumentError("autonomous prompt maxInputTokens is outside its bounds");
  const context = options.context ?? [];
  if (!Array.isArray(context) || context.length > 128) throw new ArgumentError("autonomous prompt context must contain at most 128 chunks");
  for (const chunk of context) {
    if (!isObject(chunk) || typeof chunk.id !== "string" || !chunk.id.trim() || typeof chunk.content !== "string" || bytes(chunk.content) > 64_000) throw new ArgumentError("autonomous prompt context chunk is malformed");
    if (chunk.required !== undefined && typeof chunk.required !== "boolean") throw new ArgumentError("autonomous prompt required must be boolean");
    if (chunk.priority !== undefined && (typeof chunk.priority !== "number" || !Number.isFinite(chunk.priority))) throw new ArgumentError("autonomous prompt priority must be finite");
    assertSafeTransientValue(chunk);
  }
  const outputContract = options.outputContract ?? "Return a structured answer with observations, inferences, uncertainty, evidence gaps, and next actions. Do not claim unobserved effects.";
  boundedText("autonomous prompt output contract", outputContract, 16_000);
  const stageIds = options.stageIds ?? profile.workflow.stages.map((stage) => stage.id);
  const evidencePlan = options.evidencePlan ?? await buildAutonomousEvidencePlan([profile.workflow]);
  const taskLens = autonomousDomainTaskLens(profile.domain);
  const taskDigest = await digestJson({ task: taskText });
  const taskIntent = inferAutonomousTaskIntent({
    task: taskText,
    taskDigest,
    domain: profile.domain,
    capability: profile.default_capability,
    riskClass: profile.risk_class,
    workflowId: profile.workflow.workflow_id,
    lens: taskLens,
  });
  const taskDecision = inferAutonomousTaskDecision({
    intent: taskIntent,
    lens: taskLens,
    policy: autonomousDomainPolicy(profile.domain),
    requiredModelCapabilities: profile.required_model_capabilities,
  });
  const system = `${profile.system_instructions}\n\nGuardrails:\n${profile.guardrails.map((guardrail) => `- ${guardrail}`).join("\n")}`;
  const intentPrompt = maxInputTokens < 1_024 ? "" : `\nTask intent: ${JSON.stringify(autonomousTaskIntentPromptContract(taskIntent, maxInputTokens < 2_048))}`;
  const decisionPrompt = maxInputTokens < 1_024 ? "" : `\nTask decision: ${JSON.stringify(autonomousTaskDecisionPromptContract(taskDecision, maxInputTokens < 2_048))}`;
  const developer = `Domain: ${profile.domain}\nRisk class: ${profile.risk_class}\nCapability: ${profile.default_capability}\nWorkflow: ${profile.workflow.workflow_id}\nStages: ${stageIds.join(", ")}\nTask lens: ${JSON.stringify(autonomousTaskLensPromptContract(taskLens, maxInputTokens < 2_048))}${intentPrompt}${decisionPrompt}\n\n${outputContract}`;
  const requiredMessages: AutonomousPromptMessage[] = [
    { role: "system", content: system, source_id: "domain-system" },
    { role: "developer", content: developer, source_id: "domain-developer" },
    { role: "user", content: taskText, source_id: "task" },
  ];
  const estimate = (messages: readonly { content: string }[]) => Math.max(1, Math.ceil(messages.reduce((sum, message) => sum + bytes(message.content), 0) / 4));
  if (estimate(requiredMessages) > maxInputTokens) throw new ArgumentError("autonomous prompt required content exceeds maxInputTokens");
  const sorted = [...context].sort((left, right) => Number(right.required ?? false) - Number(left.required ?? false) || (right.priority ?? 0) - (left.priority ?? 0) || left.id.localeCompare(right.id));
  // Small caller budgets still receive a digest-bound evidence contract. The
  // full requirement catalogue remains available from the blueprint/facade;
  // the prompt uses a compact projection when the caller explicitly budgets a
  // very small context window.
  const evidencePrompt = maxInputTokens < 2_048 ? evidencePlan.toPromptJSON() : evidencePlan.toJSON();
  sorted.push({ id: "autonomy-evidence-plan", content: JSON.stringify(evidencePrompt), required: true, priority: 988 });
  const included: AutonomousPromptChunk[] = [];
  const omitted: string[] = [];
  const messages = [...requiredMessages];
  for (const chunk of sorted) {
    const candidate = [...messages, { role: "user" as const, content: `Context ${chunk.id}:\n${chunk.content}`, source_id: chunk.id }];
    if (estimate(candidate) <= maxInputTokens) {
      included.push(chunk);
      messages.push(candidate[candidate.length - 1] as AutonomousPromptMessage);
    } else if (chunk.required) {
      throw new ArgumentError(`required autonomous prompt context ${chunk.id} exceeds maxInputTokens`);
    } else {
      omitted.push(chunk.id);
    }
  }
  const promptDescriptor = { schema: AUTONOMOUS_PROMPT_SCHEMA, messages, included_context_ids: included.map((chunk) => chunk.id), omitted_context_ids: omitted, estimated_input_tokens: estimate(messages), complete: omitted.length === 0, warnings: omitted.length ? ["optional context was omitted to preserve the input budget"] : [] };
  return { ...promptDescriptor, prompt_digest: await digestJson(promptDescriptor) };
}
