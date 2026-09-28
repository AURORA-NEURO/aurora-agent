/** Dependency-closed workflow plan compilation. */

import { ArgumentError } from "./errors.js";
import { autonomousDomainTaskLens } from "./autonomous-task-lens.js";
import { autonomousDomainPolicy } from "./autonomous-domain-policy.js";
import { inferAutonomousTaskDecision } from "./autonomous-task-decision.js";
import { inferAutonomousTaskIntent } from "./autonomous-task-intent.js";
import { autonomousDomainToolBindingSupportsStage as bindingSupportsStage } from "./autonomous-tool-selection.js";
import { digestJson } from "./tooling.js";
import { boundedModelDigest, boundedText } from "./autonomous-validation.js";
import type { AutonomousDomainProfile, AutonomousPlan } from "./autonomous-agent-contracts.js";

export const AUTONOMOUS_PLAN_SCHEMA = "bioprism-python-autonomous-plan/0.1" as const;

/** Compile a dependency-closed plan from the reviewed workflow and live exact tool names. */
export async function compileAutonomousPlan(
  profile: AutonomousDomainProfile,
  task: string,
  options: { taskDigest?: string; capability?: string; activeToolNames?: readonly string[]; selectedToolNames?: readonly string[]; selectedToolOrder?: readonly string[]; responseContractDigest?: string; maxParallelism?: number } = {},
): Promise<AutonomousPlan> {
  const taskText = boundedText("autonomous plan objective", task, 32_000);
  const taskDigest = options.taskDigest ?? await digestJson({ task: taskText });
  const intentTaskDigest = await digestJson({ task: taskText });
  const taskLens = autonomousDomainTaskLens(profile.domain);
  const taskPolicy = autonomousDomainPolicy(profile.domain);
  const effectiveCapability = options.capability ?? profile.default_capability;
  const taskIntent = inferAutonomousTaskIntent({
    task: taskText,
    taskDigest: intentTaskDigest,
    domain: profile.domain,
    capability: effectiveCapability,
    riskClass: profile.risk_class,
    workflowId: profile.workflow.workflow_id,
    lens: taskLens,
  });
  const taskDecision = inferAutonomousTaskDecision({
    intent: taskIntent,
    lens: taskLens,
    policy: taskPolicy,
    requiredModelCapabilities: profile.required_model_capabilities,
  });
  const active = new Set(options.activeToolNames ?? []);
  const selected = new Set(options.selectedToolNames ?? []);
  const selectedOrder = new Map((options.selectedToolOrder ?? options.selectedToolNames ?? []).map((name, index) => [name, index]));
  const bindings = profile.tool_profile.bindings;
  const stages = profile.workflow.stages;
  const steps = stages.map((stage, index) => {
    const candidates = bindings.filter((candidate) => bindingSupportsStage(profile, stage, candidate));
    const binding = [...candidates]
      .filter((candidate) => selected.has(candidate.name))
      .sort((left, right) => (selectedOrder.get(left.name) ?? Number.MAX_SAFE_INTEGER) - (selectedOrder.get(right.name) ?? Number.MAX_SAFE_INTEGER) || left.name.localeCompare(right.name))[0]
      ?? [...candidates]
        .filter((candidate) => active.has(candidate.name))
        .sort((left, right) => left.name.localeCompare(right.name))[0];
    const effect = binding ? binding.risk_class === "read_only" ? "read_only" as const : "external_write" as const : "provider_call" as const;
    return {
      id: stage.id,
      objective: stage.objective,
      tool: binding?.name ?? "provider.invoke",
      arguments: { domain: profile.domain, capability: effectiveCapability, stage_id: stage.id, task_digest: taskDigest, task_lens_id: taskLens.lens_id, task_lens_digest: taskLens.lens_digest, task_intent_id: taskIntent.intent_id, task_intent_digest: taskIntent.intent_digest, task_intent_action_mode: taskIntent.action_mode, task_intent_requested_effect: taskIntent.requested_effect, task_intent_evidence_mode: taskIntent.evidence_mode, task_intent_ambiguity_flags: [...taskIntent.ambiguity_flags], task_decision_id: taskDecision.decision_id, task_decision_digest: taskDecision.decision_digest, task_decision_posture: taskDecision.posture, task_decision_recommended_path: taskDecision.recommended_path, task_decision_approval_requirements: [...taskDecision.approval_requirements], task_decision_review_reasons: [...taskDecision.review_reasons] },
      depends_on: [...stage.depends_on],
      effect,
      estimated_cost: index + 1,
    };
  });
  const maxParallelism = options.maxParallelism ?? 4;
  if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > 8) throw new ArgumentError("autonomous plan maxParallelism must be a safe integer in [1, 8]");
  const waveById = new Map<string, number>();
  const criticalCostById = new Map<string, number>();
  const dependencyWaves: string[][] = [];
  for (const step of steps) {
    const dependencyWave = step.depends_on.reduce((maximum, dependency) => {
      const dependencyWaveIndex = waveById.get(dependency);
      if (dependencyWaveIndex === undefined) throw new ArgumentError(`autonomous plan dependency is not closed or ordered: ${step.id} -> ${dependency}`);
      return Math.max(maximum, dependencyWaveIndex + 1);
    }, 0);
    const dependencyCost = step.depends_on.reduce((maximum, dependency) => Math.max(maximum, criticalCostById.get(dependency) ?? 0), 0);
    waveById.set(step.id, dependencyWave);
    criticalCostById.set(step.id, dependencyCost + step.estimated_cost);
    while (dependencyWaves.length <= dependencyWave) dependencyWaves.push([]);
    dependencyWaves[dependencyWave]!.push(step.id);
  }
  const executionWaves = dependencyWaves.flatMap((wave) => {
    const chunks: string[][] = [];
    for (let index = 0; index < wave.length; index += maxParallelism) chunks.push(wave.slice(index, index + maxParallelism));
    return chunks;
  });
  const criticalPathCost = Math.max(0, ...steps.map((step) => criticalCostById.get(step.id) ?? 0));
  const descriptor = {
    schema: AUTONOMOUS_PLAN_SCHEMA,
    objective: taskText,
    workflow_id: profile.workflow.workflow_id,
    workflow_digest: profile.workflow.workflow_digest,
    ordered_step_ids: stages.map((stage) => stage.id),
    steps,
    execution_waves: executionWaves,
    critical_path_cost: criticalPathCost,
    max_parallelism: maxParallelism,
    estimated_parallel_rounds: executionWaves.length,
    peak_parallelism: Math.max(0, ...executionWaves.map((wave) => wave.length)),
    allowed_tools: ["provider.invoke", ...[...active].sort()],
    estimated_cost: steps.reduce((sum, step) => sum + step.estimated_cost, 0),
    requires_approval: true,
    execution: "not_started" as const,
    ...(options.responseContractDigest === undefined ? {} : { response_contract_digest: boundedModelDigest("autonomous plan response contract digest", options.responseContractDigest) }),
    domain_policy_digest: taskPolicy.policy_digest,
    task_lens_digest: taskLens.lens_digest,
    task_intent_digest: taskIntent.intent_digest,
    task_decision_digest: taskDecision.decision_digest,
    does_not_claim: ["the plan has not executed any provider or tool", "tool registration is not authorization", "a provider response is not external-effect evidence"],
  };
  return { ...descriptor, plan_digest: await digestJson(descriptor) };
}
