import { ArgumentError } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type { AutonomousDomainTaskLens } from "./autonomous-task-lens.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_TASK_INTENT_SCHEMA = "bioprism-autonomous-task-intent/0.1" as const;
export const AUTONOMOUS_TASK_INTENT_VERSION = "0.1" as const;
export const AUTONOMOUS_TASK_INTENT_DOMAINS: readonly AutonomousDomainName[] = [
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise",
  "multi_agent", "multimodal", "cross_domain", "evaluation",
];
export const AUTONOMOUS_TASK_INTENT_ACTION_MODES = ["observe", "investigate", "analyze", "create", "modify", "compare", "plan", "coordinate", "evaluate", "synthesize"] as const;
export const AUTONOMOUS_TASK_INTENT_EFFECTS = ["none", "local_change", "external_effect"] as const;
export const AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES = [
  "repository_and_test_evidence", "source_and_provenance_evidence", "schema_and_lineage_evidence", "method_and_reproduction_evidence",
  "grounding_and_safety_evidence", "modality_and_measurement_evidence", "telemetry_and_postcondition_evidence", "policy_and_audit_evidence",
  "handoff_and_dissent_evidence", "modality_and_transport_evidence", "specialist_and_synthesis_evidence", "holdout_and_replay_evidence",
] as const;
export const MAX_AUTONOMOUS_TASK_INTENT_ITEMS = 8;
const MAX_TASK_INTENT_TEXT_BYTES = 512;
const MAX_TASK_INTENT_TASK_BYTES = 32_000;

const ACTION_CUES: Record<typeof AUTONOMOUS_TASK_INTENT_ACTION_MODES[number], readonly string[]> = {
  observe: ["observe", "monitor", "inspect", "status", "check", "inventory"],
  investigate: ["research", "find", "discover", "look up", "investigate", "review", "understand", "explain"],
  analyze: ["analyze", "analyse", "assess", "measure", "quantify", "validate", "diagnose", "profile"],
  create: ["create", "draft", "write", "generate", "design", "build", "implement", "develop"],
  modify: ["fix", "debug", "refactor", "update", "change", "migrate", "patch", "remove", "delete"],
  compare: ["compare", "contrast", "benchmark", "rank", "choose", "select", "versus", "vs"],
  plan: ["plan", "schedule", "roadmap", "strategy", "rollout", "prepare", "runbook"],
  coordinate: ["delegate", "coordinate", "assign", "handoff", "orchestrate", "manage", "approve"],
  evaluate: ["evaluate", "test", "verify", "audit", "score", "grade", "replay", "red team"],
  synthesize: ["synthesize", "synthesise", "combine", "integrate", "summarize", "summarise", "reconcile", "merge"],
};
const DEFAULT_ACTIONS: Record<AutonomousDomainName, typeof AUTONOMOUS_TASK_INTENT_ACTION_MODES[number]> = {
  coding: "modify", browser: "investigate", data: "analyze", science: "investigate", biomedical: "investigate", neuroscience: "analyze",
  operations: "observe", enterprise: "plan", multi_agent: "coordinate", multimodal: "analyze", cross_domain: "synthesize", evaluation: "evaluate",
};
const EVIDENCE_MODES: Record<AutonomousDomainName, typeof AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES[number]> = {
  coding: "repository_and_test_evidence", browser: "source_and_provenance_evidence", data: "schema_and_lineage_evidence", science: "method_and_reproduction_evidence",
  biomedical: "grounding_and_safety_evidence", neuroscience: "modality_and_measurement_evidence", operations: "telemetry_and_postcondition_evidence", enterprise: "policy_and_audit_evidence",
  multi_agent: "handoff_and_dissent_evidence", multimodal: "modality_and_transport_evidence", cross_domain: "specialist_and_synthesis_evidence", evaluation: "holdout_and_replay_evidence",
};
const EXTERNAL_EFFECT_CUES = ["deploy", "production", "publish", "send", "email", "purchase", "delete data", "restart service", "grant access", "execute command", "run command", "change live", "modify database", "provision", "roll back production"];
const LOCAL_CHANGE_CUES = ["write code", "write a file", "implement", "patch", "refactor", "fix", "create a file", "update the repository", "change the code", "edit the document"];
const CREDENTIAL_CUES = ["api key", "apikey", "token", "password", "secret", "credential", "private key"];
const UNCERTAINTY_CUES = ["maybe", "possibly", "not sure", "unclear", "guess", "try to"];

function text(name: string, value: unknown, maximum = MAX_TASK_INTENT_TEXT_BYTES): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside the task-intent text bound`);
  return value;
}

function inputItems(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > 64) throw new ArgumentError(`${name} exceeds the task-intent item bound`);
  const result = value.map((item) => text(`${name} item`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate items`);
  return result;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function normalize(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim().replace(/\s+/g, " ");
}

function contains(normalized: string, phrase: string): boolean {
  const needle = normalize(phrase);
  return needle.length > 0 && ` ${normalized} `.includes(` ${needle} `);
}

function unique(values: readonly string[]): string[] {
  return [...new Set(values)];
}

export interface AutonomousTaskIntent extends JsonObject {
  schema: typeof AUTONOMOUS_TASK_INTENT_SCHEMA;
  intent_version: typeof AUTONOMOUS_TASK_INTENT_VERSION;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  workflow_id: string;
  task_digest: string;
  lens_digest: string;
  intent_id: string;
  action_mode: typeof AUTONOMOUS_TASK_INTENT_ACTION_MODES[number];
  alternative_action_modes: string[];
  requested_effect: typeof AUTONOMOUS_TASK_INTENT_EFFECTS[number];
  evidence_mode: typeof AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES[number];
  ambiguity_flags: string[];
  planning_signals: string[];
  success_signals: string[];
  risk_signals: string[];
  requested_output_count: number;
  desired_outputs_digest: string | null;
  constraints_digest: string | null;
  intent_digest: string;
  retention: "value_only_intent_metadata;task_text_not_retained";
  authorization: "classification_only;no_provider_tool_or_effect_authority";
  secret_material: "never_returned";
}

type IntentDescriptor = Omit<AutonomousTaskIntent, "intent_digest" | "retention" | "authorization" | "secret_material">;

function descriptorDigest(descriptor: IntentDescriptor): string {
  return digestJsonSync(descriptor);
}

export function autonomousTaskIntentPromptContract(intent: AutonomousTaskIntent, compact = false): JsonObject {
  if (!intent || intent.schema !== AUTONOMOUS_TASK_INTENT_SCHEMA) throw new ArgumentError("task intent prompt contract requires a valid intent");
  if (compact) {
    return {
      schema: AUTONOMOUS_TASK_INTENT_SCHEMA,
      intent_digest: intent.intent_digest,
      action_mode: intent.action_mode,
      requested_effect: intent.requested_effect,
    };
  }
  const result: JsonObject = {
    schema: AUTONOMOUS_TASK_INTENT_SCHEMA,
    intent_id: intent.intent_id,
    intent_digest: intent.intent_digest,
    task_digest: intent.task_digest,
    lens_digest: intent.lens_digest,
    action_mode: intent.action_mode,
    requested_effect: intent.requested_effect,
    evidence_mode: intent.evidence_mode,
    ambiguity_flags: [...intent.ambiguity_flags],
    authority: "classification_only;no_provider_tool_or_effect_authority",
  };
  if (!compact) {
    Object.assign(result, {
      alternative_action_modes: [...intent.alternative_action_modes],
      planning_signals: [...intent.planning_signals],
      success_signals: [...intent.success_signals],
      risk_signals: [...intent.risk_signals],
      requested_output_count: intent.requested_output_count,
      desired_outputs_digest: intent.desired_outputs_digest,
      constraints_digest: intent.constraints_digest,
    });
  }
  result.secret_material = "never_returned";
  return result;
}

export function inferAutonomousTaskIntent(args: {
  task: string;
  taskDigest: string;
  domain: AutonomousDomainName;
  capability: string;
  riskClass: string;
  workflowId: string;
  lens: AutonomousDomainTaskLens;
  constraints?: readonly string[];
  desiredOutputs?: readonly string[];
}): AutonomousTaskIntent {
  const taskText = text("task intent task", args.task, MAX_TASK_INTENT_TASK_BYTES);
  const taskDigest = digest("task intent taskDigest", args.taskDigest);
  if (taskDigest !== digestJsonSync({ task: taskText })) throw new ArgumentError("task intent taskDigest does not match task text");
  if (!AUTONOMOUS_TASK_INTENT_DOMAINS.includes(args.domain) || args.lens.domain !== args.domain) throw new ArgumentError("task intent domain and lens must agree");
  const capability = text("task intent capability", args.capability, 256);
  const riskClass = text("task intent riskClass", args.riskClass, 256);
  const workflowId = text("task intent workflowId", args.workflowId, 256);
  const constraints = inputItems("task intent constraints", args.constraints ?? []);
  const desiredOutputs = inputItems("task intent desiredOutputs", args.desiredOutputs ?? []);
  const normalized = normalize(taskText);
  const scores = Object.fromEntries(AUTONOMOUS_TASK_INTENT_ACTION_MODES.map((mode) => [mode, ACTION_CUES[mode].reduce((sum, cue) => sum + (contains(normalized, cue) ? 1 : 0), 0)])) as Record<typeof AUTONOMOUS_TASK_INTENT_ACTION_MODES[number], number>;
  const ranked = [...AUTONOMOUS_TASK_INTENT_ACTION_MODES].sort((left, right) => scores[right] - scores[left] || AUTONOMOUS_TASK_INTENT_ACTION_MODES.indexOf(left) - AUTONOMOUS_TASK_INTENT_ACTION_MODES.indexOf(right));
  const active = ranked.filter((mode) => scores[mode] > 0);
  const actionMode = active[0] ?? DEFAULT_ACTIONS[args.domain];
  const alternatives = active.slice(1, 5);
  const ambiguity: string[] = [];
  if (!active.length) ambiguity.push("missing_action_signal");
  else if (active.length > 1 && active[0] !== undefined && active[1] !== undefined && scores[active[0]] === scores[active[1]]) ambiguity.push("competing_action_modes");
  if (!desiredOutputs.length) ambiguity.push("no_explicit_output_contract");
  if (UNCERTAINTY_CUES.some((cue) => contains(normalized, cue))) ambiguity.push("uncertainty_language");
  const requestedEffect = EXTERNAL_EFFECT_CUES.some((cue) => contains(normalized, cue)) ? "external_effect" : LOCAL_CHANGE_CUES.some((cue) => contains(normalized, cue)) ? "local_change" : "none";
  if (requestedEffect === "external_effect") ambiguity.push("effect_requires_explicit_approval");
  const riskSignals: string[] = [];
  if (riskClass !== "read_only") riskSignals.push("domain_policy_review");
  if (requestedEffect === "external_effect") riskSignals.push("external_effect_language");
  if (CREDENTIAL_CUES.some((cue) => contains(normalized, cue))) riskSignals.push("credential_or_secret_language");
  const domainRiskSignals: Partial<Record<AutonomousDomainName, string>> = { biomedical: "human_review_boundary", operations: "rollback_required", enterprise: "governance_boundary", multi_agent: "single_effect_authority", cross_domain: "source_domain_ownership", evaluation: "independent_evaluator" };
  const domainRisk = domainRiskSignals[args.domain];
  if (domainRisk) riskSignals.push(domainRisk);
  if (!desiredOutputs.length) riskSignals.push("output_contract_missing");
  const successSignals = ["workflow_completion_contract", ...args.lens.evaluator_signals, ...(desiredOutputs.length ? ["caller_outputs_requested"] : [])];
  const constraintsDigest = constraints.length ? digestJsonSync([...constraints]) : null;
  const desiredOutputsDigest = desiredOutputs.length ? digestJsonSync([...desiredOutputs]) : null;
  const descriptor: IntentDescriptor = {
    schema: AUTONOMOUS_TASK_INTENT_SCHEMA,
    intent_version: AUTONOMOUS_TASK_INTENT_VERSION,
    domain: args.domain,
    capability,
    risk_class: riskClass,
    workflow_id: workflowId,
    task_digest: taskDigest,
    lens_digest: args.lens.lens_digest,
    intent_id: `${args.domain}:${workflowId}:${actionMode}`,
    action_mode: actionMode,
    alternative_action_modes: alternatives,
    requested_effect: requestedEffect,
    evidence_mode: EVIDENCE_MODES[args.domain],
    ambiguity_flags: unique(ambiguity),
    planning_signals: unique([`action:${actionMode}`, ...args.lens.planning_dimensions]),
    success_signals: unique(successSignals),
    risk_signals: unique(riskSignals),
    requested_output_count: desiredOutputs.length,
    desired_outputs_digest: desiredOutputsDigest,
    constraints_digest: constraintsDigest,
  };
  const intentDigest = descriptorDigest(descriptor);
  return Object.freeze({
    ...descriptor,
    intent_digest: intentDigest,
    retention: "value_only_intent_metadata;task_text_not_retained",
    authorization: "classification_only;no_provider_tool_or_effect_authority",
    secret_material: "never_returned",
  }) as AutonomousTaskIntent;
}

export { descriptorDigest as autonomousTaskIntentDigest };
