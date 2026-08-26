import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  autonomousDomainToolBindingSupportsStage,
  builtinAutonomousDomainProfiles,
  type AutonomousDomainProfile,
  type AutonomousWorkflowStage,
} from "./autonomous.js";
import { autonomousDomainPolicy, type AutonomousDomainPolicy } from "./autonomous-domain-policy.js";
import { autonomousDomainTaskLens, type AutonomousDomainTaskLens } from "./autonomous-task-lens.js";
import { buildAutonomousDomainResponseContract, type AutonomousDomainResponseContract } from "./autonomous-domain-response.js";
import { builtinAutonomousPromptRegistry, type AutonomousPromptRegistry } from "./autonomous-prompt-registry.js";
import { builtinAutonomousValueEvaluatorProfiles, type AutonomousValueEvaluatorProfile } from "./autonomous-domain-evaluators.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * A bounded composition of every reviewed contract needed to prepare one domain run.
 *
 * This is deliberately a metadata-only operating contract. It does not contain a task,
 * rendered prompt, provider response, credential, tool argument, evidence value, or authority
 * to execute an effect. Applications can use it as a pre-dispatch consistency check and as a
 * stable handoff between UI, planning, and runtime components.
 */
export const AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA = "bioprism-typescript-autonomous-domain-operating-kit/0.1" as const;
export const AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA = "bioprism-typescript-autonomous-domain-operating-kit-stage/0.1" as const;
export const AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION = "0.1" as const;
export const MAX_AUTONOMOUS_DOMAIN_OPERATING_KITS = 12;
export const MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGES = 16;
export const MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES = 128;
export const MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_TOOLS = 128;

export interface AutonomousDomainOperatingKitStage extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA;
  stage_id: string;
  objective: string;
  required_capabilities: string[];
  evidence_outputs: string[];
  evaluator_signals: string[];
  approval_required: boolean;
  read_only: boolean;
  prompt_candidate_ids: string[];
  selected_prompt_id: string | null;
  selected_prompt_manifest_digest: string | null;
  selected_prompt_version: string | null;
  tool_names: string[];
  stage_digest: string;
}

export type AutonomousDomainOperatingKitStatus = "complete" | "partial" | "blocked";

export interface AutonomousDomainOperatingKitCoverage extends JsonObject {
  profile: boolean;
  workflow: boolean;
  policy: boolean;
  task_lens: boolean;
  response_contract: boolean;
  prompt_templates: boolean;
  evaluator: boolean;
  stage_contracts: boolean;
  tool_bindings: boolean;
}

export interface AutonomousDomainOperatingKitCapability extends JsonObject {
  capability: string;
  stage_ids: string[];
  tool_names: string[];
  evaluator_signals: string[];
  evidence_outputs: string[];
}

export interface AutonomousDomainOperatingKit extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA;
  version: typeof AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION;
  domain: AutonomousDomainName;
  profile_digest: string;
  workflow_id: string;
  workflow_digest: string;
  domain_policy_digest: string;
  task_lens_digest: string;
  response_contract_digest: string;
  prompt_registry_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  evaluator_profile_digest: string;
  stages: AutonomousDomainOperatingKitStage[];
  capability_graph: AutonomousDomainOperatingKitCapability[];
  coverage: AutonomousDomainOperatingKitCoverage;
  status: AutonomousDomainOperatingKitStatus;
  next_actions: string[];
  execution: "metadata_only; no_provider_source_tool_evaluator_or_effect_dispatch";
  retention: "operating_contract_metadata_only;task_prompt_response_values_not_retained";
  credential_posture: "caller_owned_opaque_handles_only;no_credentials_consumed";
  secret_material: "never_returned";
  kit_digest: string;
}

type BuiltinOperatingInputs = {
  profile: AutonomousDomainProfile;
  policy: AutonomousDomainPolicy;
  lens: AutonomousDomainTaskLens;
  responseContract: AutonomousDomainResponseContract;
  promptRegistry: AutonomousPromptRegistry;
  evaluator: AutonomousValueEvaluatorProfile;
};

function isObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function digest(value: unknown, name: string): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedIdentifier(value: unknown, name: string): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function uniqueStrings(value: unknown, name: string, maximum = MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES): string[] {
  if (!Array.isArray(value) || value.length > maximum || value.some((item) => typeof item !== "string" || !item.trim() || item.length > 256)) throw new ArgumentError(`${name} is outside its bounded list contract`);
  const result = value as string[];
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return [...result];
}

type OperatingKitStageDescriptor = {
  schema: typeof AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA;
  stage_id: string;
  objective: string;
  required_capabilities: string[];
  evidence_outputs: string[];
  evaluator_signals: string[];
  approval_required: boolean;
  read_only: boolean;
  prompt_candidate_ids: string[];
  selected_prompt_id: string | null;
  selected_prompt_manifest_digest: string | null;
  selected_prompt_version: string | null;
  tool_names: string[];
};

function stageDescriptor(stage: OperatingKitStageDescriptor): JsonObject {
  return {
    schema: AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA,
    stage_id: stage.stage_id,
    objective: stage.objective,
    required_capabilities: [...stage.required_capabilities],
    evidence_outputs: [...stage.evidence_outputs],
    evaluator_signals: [...stage.evaluator_signals],
    approval_required: stage.approval_required,
    read_only: stage.read_only,
    prompt_candidate_ids: [...stage.prompt_candidate_ids],
    selected_prompt_id: stage.selected_prompt_id,
    selected_prompt_manifest_digest: stage.selected_prompt_manifest_digest,
    selected_prompt_version: stage.selected_prompt_version,
    tool_names: [...stage.tool_names],
  };
}

function stageDigest(stage: OperatingKitStageDescriptor): string {
  return digestJsonSync(stageDescriptor(stage));
}

type OperatingKitDescriptor = {
  schema: typeof AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA;
  version: typeof AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION;
  domain: AutonomousDomainName;
  profile_digest: string;
  workflow_id: string;
  workflow_digest: string;
  domain_policy_digest: string;
  task_lens_digest: string;
  response_contract_digest: string;
  prompt_registry_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  evaluator_profile_digest: string;
  stages: AutonomousDomainOperatingKitStage[];
  capability_graph: AutonomousDomainOperatingKitCapability[];
  coverage: AutonomousDomainOperatingKitCoverage;
  status: AutonomousDomainOperatingKitStatus;
  next_actions: string[];
  execution: "metadata_only; no_provider_source_tool_evaluator_or_effect_dispatch";
  retention: "operating_contract_metadata_only;task_prompt_response_values_not_retained";
  credential_posture: "caller_owned_opaque_handles_only;no_credentials_consumed";
  secret_material: "never_returned";
};

function kitDescriptor(kit: OperatingKitDescriptor): JsonObject {
  return {
    schema: AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA,
    version: AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION,
    domain: kit.domain,
    profile_digest: kit.profile_digest,
    workflow_id: kit.workflow_id,
    workflow_digest: kit.workflow_digest,
    domain_policy_digest: kit.domain_policy_digest,
    task_lens_digest: kit.task_lens_digest,
    response_contract_digest: kit.response_contract_digest,
    prompt_registry_digest: kit.prompt_registry_digest,
    evaluator_id: kit.evaluator_id,
    evaluator_version: kit.evaluator_version,
    evaluator_profile_digest: kit.evaluator_profile_digest,
    stages: kit.stages.map((stage) => ({ ...stage, required_capabilities: [...stage.required_capabilities], evidence_outputs: [...stage.evidence_outputs], evaluator_signals: [...stage.evaluator_signals], prompt_candidate_ids: [...stage.prompt_candidate_ids], tool_names: [...stage.tool_names] })),
    capability_graph: kit.capability_graph.map((row) => ({ ...row, stage_ids: [...row.stage_ids], tool_names: [...row.tool_names], evaluator_signals: [...row.evaluator_signals], evidence_outputs: [...row.evidence_outputs] })),
    coverage: { ...kit.coverage },
    status: kit.status,
    next_actions: [...kit.next_actions],
    execution: "metadata_only; no_provider_source_tool_evaluator_or_effect_dispatch",
    retention: "operating_contract_metadata_only;task_prompt_response_values_not_retained",
    credential_posture: "caller_owned_opaque_handles_only;no_credentials_consumed",
    secret_material: "never_returned",
  };
}

function validateDomains(domains: readonly AutonomousDomainName[] | undefined): readonly AutonomousDomainName[] {
  const resolved = domains ?? AUTONOMOUS_DOMAIN_NAMES;
  if (!Array.isArray(resolved) || resolved.length < 1 || resolved.length > MAX_AUTONOMOUS_DOMAIN_OPERATING_KITS) throw new ArgumentError("operating-kit domains are outside their bounds");
  if (new Set(resolved).size !== resolved.length || resolved.some((domain) => !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain))) throw new ArgumentError("operating-kit domains contain an unsupported or duplicate domain");
  return resolved;
}

async function inputsFor(domain: AutonomousDomainName): Promise<BuiltinOperatingInputs> {
  const profiles = await builtinAutonomousDomainProfiles();
  const profile = profiles.find((candidate) => candidate.domain === domain);
  if (!profile) throw new ArgumentError(`operating kit has no profile for ${domain}`);
  const evaluator = builtinAutonomousValueEvaluatorProfiles().find((candidate) => candidate.domain === domain);
  if (!evaluator) throw new ArgumentError(`operating kit has no evaluator for ${domain}`);
  return {
    profile,
    policy: autonomousDomainPolicy(domain),
    lens: autonomousDomainTaskLens(domain),
    responseContract: await buildAutonomousDomainResponseContract(profile),
    promptRegistry: builtinAutonomousPromptRegistry([domain]),
    evaluator,
  };
}

function buildStage(profile: AutonomousDomainProfile, stage: AutonomousWorkflowStage, promptRegistry: AutonomousPromptRegistry): AutonomousDomainOperatingKitStage {
  const candidates = promptRegistry.candidates(profile.domain, stage.id, []);
  const selected = candidates[0];
  const toolNames = profile.tool_profile.bindings
    .filter((binding) => autonomousDomainToolBindingSupportsStage(profile, stage, binding))
    .map((binding) => binding.name)
    .sort();
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA,
    stage_id: stage.id,
    objective: stage.objective,
    required_capabilities: [...stage.required_capabilities],
    evidence_outputs: [...stage.evidence_outputs],
    evaluator_signals: [...stage.evaluator_signals],
    approval_required: stage.approval_required,
    read_only: stage.read_only,
    prompt_candidate_ids: candidates.map((candidate) => candidate.promptId),
    selected_prompt_id: selected?.promptId ?? null,
    selected_prompt_manifest_digest: selected?.manifestDigest ?? null,
    selected_prompt_version: selected?.version ?? null,
    tool_names: toolNames,
  } satisfies Omit<AutonomousDomainOperatingKitStage, "stage_digest">;
  return { ...descriptor, stage_digest: stageDigest(descriptor) };
}

function stageCapabilityGraph(profile: AutonomousDomainProfile, stages: readonly AutonomousDomainOperatingKitStage[]): AutonomousDomainOperatingKitCapability[] {
  const capabilities = [...new Set([...profile.capabilities, ...stages.flatMap((stage) => stage.required_capabilities)])].sort();
  return capabilities.map((capability) => {
    const matching = stages.filter((stage) => stage.required_capabilities.includes(capability));
    return {
      capability,
      stage_ids: matching.map((stage) => stage.stage_id),
      tool_names: [...new Set(matching.flatMap((stage) => stage.tool_names))].sort(),
      evaluator_signals: [...new Set(matching.flatMap((stage) => stage.evaluator_signals))].sort(),
      evidence_outputs: [...new Set(matching.flatMap((stage) => stage.evidence_outputs))].sort(),
    };
  });
}

/** Build the complete provider-free operating contract for one built-in domain. */
export async function buildAutonomousDomainOperatingKit(domain: AutonomousDomainName): Promise<AutonomousDomainOperatingKit> {
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError(`unsupported autonomous operating-kit domain: ${domain}`);
  const { profile, policy, lens, responseContract, promptRegistry, evaluator } = await inputsFor(domain);
  const stages = profile.workflow.stages.map((stage) => buildStage(profile, stage, promptRegistry));
  if (stages.length < 1 || stages.length > MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGES) throw new ArgumentError(`operating kit ${domain} has an invalid stage count`);
  const coverage: AutonomousDomainOperatingKitCoverage = {
    profile: profile.schema === "bioprism-typescript-autonomous-agent/0.1",
    workflow: profile.workflow.workflow_id.length > 0 && digest(profile.workflow.workflow_digest, `${domain} workflow_digest`).length === 64,
    policy: digest(policy.policy_digest, `${domain} domain_policy_digest`).length === 64,
    task_lens: digest(lens.lens_digest, `${domain} task_lens_digest`).length === 64,
    response_contract: digest(responseContract.contract_digest, `${domain} response_contract_digest`).length === 64,
    prompt_templates: stages.every((stage) => stage.prompt_candidate_ids.length > 0 && stage.selected_prompt_id !== null && stage.selected_prompt_manifest_digest !== null),
    evaluator: evaluator.domain === domain && evaluator.evaluator_id.length > 0,
    stage_contracts: stages.every((stage) => stage.required_capabilities.length > 0 && stage.evidence_outputs.length > 0 && stage.evaluator_signals.length > 0 && stage.stage_digest.length === 64),
    tool_bindings: stages.every((stage) => stage.tool_names.length > 0),
  };
  const failed = Object.entries(coverage).filter(([, value]) => !value).map(([key]) => key);
  const status: AutonomousDomainOperatingKitStatus = !coverage.profile || !coverage.workflow || !coverage.policy || !coverage.response_contract || !coverage.evaluator ? "blocked" : failed.length ? "partial" : "complete";
  const nextActions = status === "complete" ? ["resolve caller-owned provider and credential handles", "run the ordinary route, selection, evidence, approval, and evaluator gates"] : failed.map((name) => `repair_missing_${name}`);
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA,
    version: AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION,
    domain,
    profile_digest: digestJsonSync(profile),
    workflow_id: profile.workflow.workflow_id,
    workflow_digest: profile.workflow.workflow_digest,
    domain_policy_digest: policy.policy_digest,
    task_lens_digest: lens.lens_digest,
    response_contract_digest: responseContract.contract_digest,
    prompt_registry_digest: promptRegistry.registryDigest,
    evaluator_id: evaluator.evaluator_id,
    evaluator_version: evaluator.evaluator_version,
    evaluator_profile_digest: digestJsonSync(evaluator),
    stages,
    capability_graph: stageCapabilityGraph(profile, stages),
    coverage,
    status,
    next_actions: nextActions,
    execution: "metadata_only; no_provider_source_tool_evaluator_or_effect_dispatch" as const,
    retention: "operating_contract_metadata_only;task_prompt_response_values_not_retained" as const,
    credential_posture: "caller_owned_opaque_handles_only;no_credentials_consumed" as const,
    secret_material: "never_returned" as const,
  } satisfies Omit<AutonomousDomainOperatingKit, "kit_digest">;
  return Object.freeze({ ...descriptor, kit_digest: digestJsonSync(kitDescriptor(descriptor as OperatingKitDescriptor)) }) as AutonomousDomainOperatingKit;
}

/** Build one deterministic, canonical collection covering all requested built-in domains. */
export async function buildAutonomousDomainOperatingKits(domains?: readonly AutonomousDomainName[]): Promise<readonly AutonomousDomainOperatingKit[]> {
  return Promise.all(validateDomains(domains).map((domain) => buildAutonomousDomainOperatingKit(domain)));
}

/** Resolve the singular operating-kit API using the same naming style as other registries. */
export async function autonomousDomainOperatingKit(domain: AutonomousDomainName): Promise<AutonomousDomainOperatingKit> {
  return buildAutonomousDomainOperatingKit(domain);
}

function assertKitShape(value: unknown): asserts value is AutonomousDomainOperatingKit {
  if (!isObject(value)) throw new ArgumentError("autonomous operating kit must be a JSON object");
  const allowed = new Set(["schema", "version", "domain", "profile_digest", "workflow_id", "workflow_digest", "domain_policy_digest", "task_lens_digest", "response_contract_digest", "prompt_registry_digest", "evaluator_id", "evaluator_version", "evaluator_profile_digest", "stages", "capability_graph", "coverage", "status", "next_actions", "execution", "retention", "credential_posture", "secret_material", "kit_digest"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new ArgumentError("autonomous operating kit contains unsupported fields");
  if (value.schema !== AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA || value.version !== AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION) throw new ArgumentError("autonomous operating kit schema or version is unsupported");
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(value.domain as string)) throw new ArgumentError("autonomous operating kit domain is unsupported");
  for (const name of ["profile_digest", "workflow_digest", "domain_policy_digest", "task_lens_digest", "response_contract_digest", "prompt_registry_digest", "evaluator_profile_digest", "kit_digest"]) digest(value[name], `operating kit ${name}`);
  boundedIdentifier(value.workflow_id, "operating kit workflow_id");
  boundedIdentifier(value.evaluator_id, "operating kit evaluator_id");
  if (typeof value.evaluator_version !== "string" || !value.evaluator_version.trim()) throw new ArgumentError("operating kit evaluator_version is malformed");
  if (!Array.isArray(value.stages) || value.stages.length < 1 || value.stages.length > MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGES) throw new ArgumentError("operating kit stages are outside their bounds");
  if (!Array.isArray(value.capability_graph) || value.capability_graph.length < 1 || value.capability_graph.length > MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES) throw new ArgumentError("operating kit capability_graph is outside its bounds");
  if (!isObject(value.coverage) || Object.keys(value.coverage).some((key) => !["profile", "workflow", "policy", "task_lens", "response_contract", "prompt_templates", "evaluator", "stage_contracts", "tool_bindings"].includes(key)) || Object.values(value.coverage).some((item) => typeof item !== "boolean")) throw new ArgumentError("operating kit coverage is malformed");
  if (value.status !== "complete" && value.status !== "partial" && value.status !== "blocked") throw new ArgumentError("operating kit status is unsupported");
  if (!Array.isArray(value.next_actions) || value.next_actions.some((item) => typeof item !== "string")) throw new ArgumentError("operating kit next_actions is malformed");
  if (value.execution !== "metadata_only; no_provider_source_tool_evaluator_or_effect_dispatch" || value.retention !== "operating_contract_metadata_only;task_prompt_response_values_not_retained" || value.credential_posture !== "caller_owned_opaque_handles_only;no_credentials_consumed" || value.secret_material !== "never_returned") throw new ArgumentError("operating kit safety markers are malformed");
  const stageIds = new Set<string>();
  for (const raw of value.stages) {
    if (!isObject(raw)) throw new ArgumentError("operating kit stage is malformed");
    if (raw.schema !== AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA) throw new ArgumentError("operating kit stage schema is unsupported");
    const stage = raw as unknown as AutonomousDomainOperatingKitStage;
    boundedIdentifier(stage.stage_id, "operating kit stage_id");
    if (stageIds.has(stage.stage_id)) throw new ArgumentError("operating kit stage ids must be unique");
    stageIds.add(stage.stage_id);
    if (typeof stage.objective !== "string" || !stage.objective.trim()) throw new ArgumentError("operating kit stage objective is malformed");
    for (const [name, items] of [["required_capabilities", stage.required_capabilities], ["evidence_outputs", stage.evidence_outputs], ["evaluator_signals", stage.evaluator_signals], ["prompt_candidate_ids", stage.prompt_candidate_ids], ["tool_names", stage.tool_names]] as const) uniqueStrings(items, `operating kit stage ${stage.stage_id}.${name}`, MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_TOOLS);
    if (typeof stage.read_only !== "boolean" || typeof stage.approval_required !== "boolean") throw new ArgumentError("operating kit stage safety flags are malformed");
    if (stage.selected_prompt_id !== null) boundedIdentifier(stage.selected_prompt_id, "operating kit selected_prompt_id");
    if (stage.selected_prompt_manifest_digest !== null) digest(stage.selected_prompt_manifest_digest, "operating kit selected_prompt_manifest_digest");
    if (stage.selected_prompt_version !== null && (typeof stage.selected_prompt_version !== "string" || !stage.selected_prompt_version.trim())) throw new ArgumentError("operating kit selected_prompt_version is malformed");
    digest(stage.stage_digest, `operating kit stage ${stage.stage_id}.stage_digest`);
    if (stageDigest(stage as unknown as OperatingKitStageDescriptor) !== stage.stage_digest) throw new ArgumentError(`operating kit stage ${stage.stage_id} digest does not match its contents`);
  }
}

/** Reject stale, tampered, or malformed kits by replaying the current built-in composition. */
export async function validateAutonomousDomainOperatingKit(value: unknown): Promise<AutonomousDomainOperatingKit> {
  assertKitShape(value);
  const current = await buildAutonomousDomainOperatingKit(value.domain);
  if (current.kit_digest !== value.kit_digest) throw new ArgumentError("autonomous operating kit is stale or tampered");
  if (digestJsonSync(kitDescriptor(value as unknown as OperatingKitDescriptor)) !== value.kit_digest) throw new ArgumentError("autonomous operating kit digest does not match its contents");
  return current;
}
