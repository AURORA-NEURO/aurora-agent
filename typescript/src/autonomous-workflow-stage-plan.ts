/** Digest-bound workflow stage capability contracts and restart validation. */

import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import { digestJson } from "./tooling.js";
import { boundedDigest, boundedIdentifier, boundedText, bytes } from "./autonomous-validation.js";
import {
  WORKFLOW_CAPABILITY_ALIASES,
  autonomousDomainToolBindingSupportsStage as bindingSupportsStage,
} from "./autonomous-tool-selection.js";
import type {
  AutonomousCapabilityContract,
  AutonomousDomainPack,
  AutonomousDomainProfile,
  AutonomousTaskBlueprint,
  AutonomousWorkflowStage,
  AutonomousWorkflowStageExecutionPlan,
} from "./autonomous-agent-contracts.js";

export const AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA = "bioprism-python-autonomous-workflow-stage-plan/0.1" as const;
export const AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA = "bioprism-python-autonomous-capability-contract/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_STAGE_PLAN_BYTES = 64_000;

/** Build the reviewed capability/evidence graph used by stage execution packets. */
async function buildAutonomousCapabilityContracts(
  profile: AutonomousDomainProfile,
  pack: AutonomousDomainPack,
): Promise<AutonomousCapabilityContract[]> {
  const orderedCapabilities = [...new Set([
    ...profile.capabilities,
    ...profile.workflow.stages.flatMap((stage) => stage.required_capabilities),
  ])];
  const contracts = await Promise.all(orderedCapabilities.map(async (capability) => {
    const stages = profile.workflow.stages.filter((stage) => stage.required_capabilities.includes(capability));
    const aliases = WORKFLOW_CAPABILITY_ALIASES[profile.domain][capability] ?? [];
    const evidenceOutputs = [...new Set(stages.flatMap((stage) => stage.evidence_outputs))];
    const evaluatorSignals = [...new Set([
      ...stages.flatMap((stage) => stage.evaluator_signals),
      ...profile.workflow.evaluator_signals,
    ])];
    const descriptor = {
      schema: AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA,
      domain: profile.domain,
      capability,
      stage_ids: stages.map((stage) => stage.id),
      tool_capabilities: [...new Set([capability, ...aliases])],
      required_model_capabilities: [...pack.model_capabilities],
      evidence_outputs: evidenceOutputs.length ? evidenceOutputs : [`${capability}_result`],
      evaluator_signals: evaluatorSignals,
      read_only: stages.length === 0 || stages.every((stage) => stage.read_only),
      approval_required: stages.some((stage) => stage.approval_required),
      review_triggers: [...pack.review_triggers],
      fallback_policy: stages.length ? "provider_only_or_blocked" as const : "provider_only" as const,
    };
    return {
      ...descriptor,
      contract_digest: await digestJson(descriptor),
      adapter_posture: "exact_capability_aliases_only" as const,
      credential_posture: "caller_supplied_opaque_handles" as const,
      authority_posture: "metadata_only; no_provider_or_effect_authority" as const,
    };
  }));
  if (contracts.length > 128) throw new ArgumentError("autonomous capability contract catalogue exceeds its bound");
  return contracts;
}

/**
 * Compile the exact stage packet used to narrow live tools and bind stage evidence. The
 * packet deliberately contains no task text, arguments, provider output, credentials, or
 * effect authorization, and its schema is shared with the Python façade for replay parity.
 */
export async function compileAutonomousWorkflowStageExecutionPlan(
  blueprint: AutonomousTaskBlueprint,
  stage: AutonomousWorkflowStage,
  options: {
    activeToolNames?: readonly string[];
    selectedToolNames?: readonly string[];
    withheldToolNames?: readonly string[];
  } = {},
): Promise<AutonomousWorkflowStageExecutionPlan> {
  if (!isObject(blueprint) || blueprint.schema !== "bioprism-python-autonomous-task/0.1") throw new ArgumentError("stage execution plan requires an AutonomousTaskBlueprint");
  if (!isObject(stage)) throw new ArgumentError("stage execution plan requires an AutonomousWorkflowStage");
  const reviewedStage = blueprint.workflow.stages.find((candidate) => candidate.id === stage.id);
  if (!reviewedStage) throw new ArgumentError("stage execution plan stage is outside the prepared workflow");
  if (reviewedStage !== stage && await digestJson(reviewedStage) !== await digestJson(stage)) throw new ArgumentError("stage execution plan stage does not match the prepared workflow");
  const activeToolNames = [...new Set((options.activeToolNames ?? blueprint.plan.allowed_tools.filter((name) => name !== "provider.invoke")).map((name) => boundedIdentifier("stage execution plan active tool", name)))].sort();
  const selectedToolNames = [...new Set((options.selectedToolNames ?? activeToolNames).map((name) => boundedIdentifier("stage execution plan selected tool", name)))];
  const withheldToolNames = [...new Set((options.withheldToolNames ?? blueprint.domain_profile.tool_profile.bindings.map((binding) => binding.name).filter((name) => !activeToolNames.includes(name))).map((name) => boundedIdentifier("stage execution plan withheld tool", name)))].sort();
  if (activeToolNames.length > 512 || selectedToolNames.length > 512 || withheldToolNames.length > 512) throw new ArgumentError("stage execution plan tool names exceed their bound");
  if (selectedToolNames.some((name) => !activeToolNames.includes(name))) throw new ArgumentError("stage execution plan selected tools must be active");
  if (withheldToolNames.some((name) => activeToolNames.includes(name))) throw new ArgumentError("stage execution plan withheld tools must not be active");
  const contracts = await buildAutonomousCapabilityContracts(blueprint.domain_profile, blueprint.domain_pack);
  const stageContracts = contracts.filter((contract) => reviewedStage.required_capabilities.includes(contract.capability));
  if (stageContracts.length !== new Set(reviewedStage.required_capabilities).size) throw new ArgumentError("stage execution plan has an unresolved capability contract");
  const supportedBindings = blueprint.domain_profile.tool_profile.bindings.filter((binding) => bindingSupportsStage(blueprint.domain_profile, reviewedStage, binding));
  const selected = new Set(selectedToolNames);
  const selectedForStage = supportedBindings.filter((binding) => selected.has(binding.name)).map((binding) => binding.name).sort();
  const descriptor = {
    schema: AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA,
    domain: blueprint.domain_profile.domain,
    workflow_id: blueprint.workflow.workflow_id,
    workflow_digest: blueprint.workflow.workflow_digest,
    stage_id: reviewedStage.id,
    stage_objective: reviewedStage.objective,
    required_capabilities: [...reviewedStage.required_capabilities],
    tool_capabilities: [...new Set(stageContracts.flatMap((contract) => contract.tool_capabilities))],
    capability_contracts: stageContracts,
    required_model_capabilities: [...blueprint.required_capabilities],
    evidence_outputs: [...reviewedStage.evidence_outputs],
    evaluator_signals: [...reviewedStage.evaluator_signals],
    active_tool_names: activeToolNames,
    selected_tool_names: selectedForStage,
    withheld_tool_names: withheldToolNames,
    approval_required: reviewedStage.approval_required,
    read_only: reviewedStage.read_only,
    execution_posture: reviewedStage.approval_required
      ? "approval_gated" as const
      : selectedForStage.length
        ? "tool_backed" as const
        : "provider_only_or_blocked" as const,
    source_plan_digest: blueprint.plan.plan_digest,
  };
  return validateAutonomousWorkflowStageExecutionPlan({
    ...descriptor,
    stage_plan_digest: await digestJson(descriptor),
    capability_contract_digests: stageContracts.map((contract) => contract.contract_digest),
    credential_posture: "caller_supplied_opaque_handles; no_keys_or_handles",
    authority_posture: "metadata_only; stage_plan_does_not_grant_authority",
  }, { blueprint, stage: reviewedStage });
}

/**
 * Reconstruct and verify a persisted stage packet before it can re-enter workflow execution.
 *
 * A stage packet is metadata-only, but it still narrows provider-visible tools and capability
 * contracts.  This validator therefore checks the full derived identity rather than accepting a
 * plausible digest string.  It is safe for restart workers: no task text, prompt, provider
 * payload, credential, argument, or effect authority is imported from the packet.
 */
export async function validateAutonomousWorkflowStageExecutionPlan(
  value: unknown,
  options: { blueprint?: AutonomousTaskBlueprint; stage?: AutonomousWorkflowStage } = {},
): Promise<AutonomousWorkflowStageExecutionPlan> {
  if (!isObject(value)) throw new ArgumentError("workflow stage execution plan must be an object");
  const allowed = new Set([
    "schema", "domain", "workflow_id", "workflow_digest", "stage_id", "stage_objective",
    "required_capabilities", "tool_capabilities", "capability_contracts",
    "required_model_capabilities", "evidence_outputs", "evaluator_signals", "active_tool_names",
    "selected_tool_names", "withheld_tool_names", "approval_required", "read_only",
    "execution_posture", "source_plan_digest", "stage_plan_digest", "capability_contract_digests",
    "credential_posture", "authority_posture",
  ]);
  if (Object.keys(value).some((key) => !allowed.has(key)) || Object.keys(value).length !== allowed.size) {
    throw new ArgumentError("workflow stage execution plan has unexpected or missing fields");
  }
  if (value.schema !== AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA) throw new ArgumentError("workflow stage execution plan schema is invalid");
  if (value.credential_posture !== "caller_supplied_opaque_handles; no_keys_or_handles") throw new ArgumentError("workflow stage execution plan credential posture is invalid");
  if (value.authority_posture !== "metadata_only; stage_plan_does_not_grant_authority") throw new ArgumentError("workflow stage execution plan authority posture is invalid");
  if (bytes(JSON.stringify(value) ?? "") > MAX_AUTONOMOUS_WORKFLOW_STAGE_PLAN_BYTES) throw new ArgumentError("workflow stage execution plan exceeds its byte bound");

  const sequence = (name: string, raw: unknown, maximum = 64): string[] => {
    if (!Array.isArray(raw) || raw.length > maximum) throw new ArgumentError(`workflow stage execution plan ${name} must be a bounded array`);
    const result = raw.map((item, index) => boundedIdentifier(`workflow stage execution plan ${name}[${index}]`, item));
    if (new Set(result).size !== result.length) throw new ArgumentError(`workflow stage execution plan ${name} contains duplicates`);
    return result;
  };
  const domain = boundedIdentifier("workflow stage execution plan domain", value.domain) as AutonomousDomainName;
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("workflow stage execution plan domain is unsupported");
  const workflowId = boundedIdentifier("workflow stage execution plan workflow_id", value.workflow_id);
  const workflowDigest = boundedDigest("workflow stage execution plan workflow_digest", value.workflow_digest);
  const stageId = boundedIdentifier("workflow stage execution plan stage_id", value.stage_id);
  const stageObjective = boundedText("workflow stage execution plan stage_objective", value.stage_objective, 2_048);
  const requiredCapabilities = sequence("required_capabilities", value.required_capabilities);
  const toolCapabilities = sequence("tool_capabilities", value.tool_capabilities);
  const requiredModelCapabilities = sequence("required_model_capabilities", value.required_model_capabilities);
  const evidenceOutputs = sequence("evidence_outputs", value.evidence_outputs);
  const evaluatorSignals = sequence("evaluator_signals", value.evaluator_signals);
  const activeToolNames = sequence("active_tool_names", value.active_tool_names, 512);
  const selectedToolNames = sequence("selected_tool_names", value.selected_tool_names, 512);
  const withheldToolNames = sequence("withheld_tool_names", value.withheld_tool_names, 512);
  if (!requiredCapabilities.length || !evidenceOutputs.length || !evaluatorSignals.length) throw new ArgumentError("workflow stage execution plan requires capabilities, evidence outputs, and evaluator signals");
  if (selectedToolNames.some((name) => !activeToolNames.includes(name))) throw new ArgumentError("workflow stage execution plan selected tools must be active");
  if (activeToolNames.some((name) => withheldToolNames.includes(name))) throw new ArgumentError("workflow stage execution plan active and withheld tools overlap");
  if (typeof value.approval_required !== "boolean" || typeof value.read_only !== "boolean") throw new ArgumentError("workflow stage execution plan safety flags are invalid");
  const posture = value.execution_posture;
  if (posture !== "approval_gated" && posture !== "tool_backed" && posture !== "provider_only_or_blocked") throw new ArgumentError("workflow stage execution plan execution posture is invalid");
  const expectedPosture = value.approval_required ? "approval_gated" : selectedToolNames.length ? "tool_backed" : "provider_only_or_blocked";
  if (posture !== expectedPosture) throw new ArgumentError("workflow stage execution plan execution posture is inconsistent");
  const sourcePlanDigest = value.source_plan_digest === null ? null : boundedDigest("workflow stage execution plan source_plan_digest", value.source_plan_digest);
  if (!Array.isArray(value.capability_contracts) || value.capability_contracts.length > 64) throw new ArgumentError("workflow stage execution plan capability contracts are outside their bound");
  const contracts = value.capability_contracts.map((raw, index) => {
    if (!isObject(raw)) throw new ArgumentError(`workflow stage execution plan capability contract ${index} is malformed`);
    const contract = structuredClone(raw);
    const contractDigest = boundedDigest(`workflow stage execution plan capability contract ${index} digest`, contract.contract_digest);
    const descriptor = { ...contract };
    delete descriptor.contract_digest;
    delete descriptor.adapter_posture;
    delete descriptor.credential_posture;
    delete descriptor.authority_posture;
    if (contract.schema !== AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA) throw new ArgumentError("workflow stage execution plan capability contract schema is invalid");
    if (contract.adapter_posture !== "exact_capability_aliases_only" || contract.credential_posture !== "caller_supplied_opaque_handles" || contract.authority_posture !== "metadata_only; no_provider_or_effect_authority") throw new ArgumentError("workflow stage execution plan capability contract markers are invalid");
    return { contract, contractDigest, descriptor };
  });
  for (const { contract, contractDigest, descriptor } of contracts) {
    if (await digestJson(descriptor) !== contractDigest) throw new ArgumentError("workflow stage execution plan capability contract digest is invalid");
  }
  const contractCapabilities = contracts.map(({ contract }) => boundedIdentifier("workflow stage execution plan capability contract capability", contract.capability));
  if (new Set(contractCapabilities).size !== requiredCapabilities.length || contractCapabilities.some((capability) => !requiredCapabilities.includes(capability))) throw new ArgumentError("workflow stage execution plan capability contracts do not match required capabilities");
  if (!Array.isArray(value.capability_contract_digests) || value.capability_contract_digests.length !== contracts.length) throw new ArgumentError("workflow stage execution plan capability contract digests are malformed");
  const capabilityContractDigests = value.capability_contract_digests.map((item, index) => boundedDigest(`workflow stage execution plan capability_contract_digests[${index}]`, item));
  if (JSON.stringify(capabilityContractDigests) !== JSON.stringify(contracts.map(({ contractDigest }) => contractDigest))) throw new ArgumentError("workflow stage execution plan capability contract digests are inconsistent");
  const descriptor = {
    schema: AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA,
    domain,
    workflow_id: workflowId,
    workflow_digest: workflowDigest,
    stage_id: stageId,
    stage_objective: stageObjective,
    required_capabilities: requiredCapabilities,
    tool_capabilities: toolCapabilities,
    capability_contracts: contracts.map(({ contract }) => contract as unknown as AutonomousCapabilityContract),
    required_model_capabilities: requiredModelCapabilities,
    evidence_outputs: evidenceOutputs,
    evaluator_signals: evaluatorSignals,
    active_tool_names: activeToolNames,
    selected_tool_names: selectedToolNames,
    withheld_tool_names: withheldToolNames,
    approval_required: value.approval_required,
    read_only: value.read_only,
    execution_posture: posture as AutonomousWorkflowStageExecutionPlan["execution_posture"],
    source_plan_digest: sourcePlanDigest,
  };
  const stagePlanDigest = boundedDigest("workflow stage execution plan stage_plan_digest", value.stage_plan_digest);
  if (await digestJson(descriptor) !== stagePlanDigest) throw new ArgumentError("workflow stage execution plan digest does not match its contents");

  const validated: AutonomousWorkflowStageExecutionPlan = {
    ...descriptor,
    stage_plan_digest: stagePlanDigest,
    capability_contract_digests: capabilityContractDigests,
    credential_posture: "caller_supplied_opaque_handles; no_keys_or_handles",
    authority_posture: "metadata_only; stage_plan_does_not_grant_authority",
  };
  const reviewedStage = options.stage ?? options.blueprint?.workflow.stages.find((candidate) => candidate.id === stageId);
  if (reviewedStage !== undefined) {
    if (await digestJson(reviewedStage) !== await digestJson({ id: stageId, objective: stageObjective, required_capabilities: requiredCapabilities, depends_on: reviewedStage.depends_on, evidence_outputs: evidenceOutputs, evaluator_signals: evaluatorSignals, read_only: value.read_only, approval_required: value.approval_required })) throw new ArgumentError("workflow stage execution plan stage contract does not match");
  }
  if (options.blueprint !== undefined) {
    const blueprint = options.blueprint;
    if (!isObject(blueprint) || blueprint.schema !== "bioprism-python-autonomous-task/0.1") throw new ArgumentError("workflow stage execution plan blueprint binding is invalid");
    if (domain !== blueprint.domain_profile.domain || workflowId !== blueprint.workflow.workflow_id || workflowDigest !== blueprint.workflow.workflow_digest) throw new ArgumentError("workflow stage execution plan blueprint workflow does not match");
    if (JSON.stringify(requiredModelCapabilities) !== JSON.stringify(blueprint.required_capabilities)) throw new ArgumentError("workflow stage execution plan blueprint model capabilities do not match");
    if (sourcePlanDigest !== null && sourcePlanDigest !== blueprint.plan.plan_digest) throw new ArgumentError("workflow stage execution plan source plan does not match the blueprint");
    const blueprintStage = blueprint.workflow.stages.find((candidate) => candidate.id === stageId);
    if (!blueprintStage) throw new ArgumentError("workflow stage execution plan stage is outside the blueprint workflow");
    if (options.stage !== undefined && await digestJson(options.stage) !== await digestJson(blueprintStage)) throw new ArgumentError("workflow stage execution plan supplied stage does not match the blueprint");
  }
  return structuredClone(validated);
}
