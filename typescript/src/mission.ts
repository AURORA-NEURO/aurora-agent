import { ArgumentError, isObject } from "./errors.js";
import { digestJson, ToolCatalogue, ToolSchemaError } from "./tooling.js";
import type {
  AgentMissionArgs,
  AgentMissionBinding,
  AgentMissionPolicy,
  AgentMissionStep,
  JsonObject,
  MissionPreflightResult,
  MissionStepPreflight,
  ToolValidationReport,
} from "./types.js";

export const MISSION_PREFLIGHT_SCHEMA = "bioprism-typescript-mission-preflight/0.1";
export const MAX_MISSION_STEPS = 128;
export const MAX_ALLOWED_TOOLS = 512;
export const MAX_STEP_OUTPUT_BYTES = 20_000_000;
export const MAX_TOTAL_OUTPUT_BYTES = 20_000_000;

export class MissionPreflightError extends ArgumentError {
  override readonly name: string = "MissionPreflightError";
  readonly result?: MissionPreflightResult;

  constructor(message: string, result?: MissionPreflightResult) {
    super(message);
    this.result = result;
  }
}

interface NormalStep {
  id: string;
  tool: string;
  dependsOn: string[];
  bindings: AgentMissionBinding[];
  arguments: JsonObject;
  issues: string[];
  warnings: string[];
}

interface NormalPolicy {
  execute: boolean;
  stopOnError: boolean;
  allowSideEffects: boolean;
  allowedTools: string[];
  maxSteps: number;
  maxStepOutputBytes: number;
  maxTotalOutputBytes: number;
}

/**
 * Review a cross-domain mission without issuing any tool call.
 *
 * This is intentionally an orchestration and transport check. The Rust `agent_mission` tool
 * remains the authority for execution, refusal propagation, output accounting, and domain claims.
 */
export async function preflightMission(
  request: AgentMissionArgs,
  catalogue: ToolCatalogue,
): Promise<MissionPreflightResult> {
  if (!isObject(request)) throw new ArgumentError("request must be a JSON object");
  if (!(catalogue instanceof ToolCatalogue)) throw new ArgumentError("catalogue must be a ToolCatalogue");

  const issues: string[] = [];
  const warnings: string[] = [];
  const missionId = typeof request.mission_id === "string" ? request.mission_id : "";
  const goal = typeof request.goal === "string" ? request.goal : "";
  if (!missionId.trim()) issues.push("mission_id must be a non-empty string");
  if (!goal.trim()) issues.push("goal must be a non-empty string");

  let requestDigest = "";
  try {
    requestDigest = await digestJson(request);
  } catch (error) {
    issues.push(`request cannot be canonically digested: ${String(error)}`);
  }

  const policyIssueStart = issues.length;
  const policy = normalisePolicy(request.policy, issues);
  const rawSteps = Array.isArray(request.steps) ? request.steps : [];
  if (rawSteps.length === 0) issues.push("steps must contain at least one step");
  if (rawSteps.length > MAX_MISSION_STEPS) issues.push(`mission has ${rawSteps.length} steps; maximum is ${MAX_MISSION_STEPS}`);
  if (rawSteps.length > policy.maxSteps) issues.push(`mission has ${rawSteps.length} steps; policy.max_steps is ${policy.maxSteps}`);

  const steps: NormalStep[] = [];
  for (const [index, raw] of rawSteps.entries()) {
    const stepIssues: string[] = [];
    if (!isObject(raw)) {
      issues.push(`step ${index} must be a JSON object`);
      steps.push({ id: `step-${index}`, tool: "", dependsOn: [], bindings: [], arguments: {}, issues: ["step must be a JSON object"], warnings: [] });
      continue;
    }
    const candidate = raw as Record<string, unknown>;
    const id = typeof candidate.id === "string" && candidate.id.trim() ? candidate.id : `step-${index}`;
    if (typeof candidate.id !== "string" || !(candidate.id as string).trim()) stepIssues.push("id must be a non-empty string");
    const tool = typeof candidate.tool === "string" ? candidate.tool : "";
    for (const field of ["domain", "capability", "objective"] as const) {
    if (typeof candidate[field] !== "string" || !(candidate[field] as string).trim()) stepIssues.push(`${field} must be a non-empty string`);
    }
    if (!tool.trim()) stepIssues.push("tool must be a non-empty string");
    if (candidate.required !== undefined && typeof candidate.required !== "boolean") stepIssues.push("required must be a boolean");

    const dependsOn = normaliseStringArray(candidate.depends_on, `${id}.depends_on`, stepIssues);
    const rawArguments = candidate.arguments;
    const stepArguments = rawArguments === undefined ? {} : rawArguments;
    if (!isObject(stepArguments)) stepIssues.push("arguments must be a JSON object");
    const bindings = normaliseBindings(candidate.bindings, `${id}.bindings`, stepIssues);
    steps.push({ id, tool, dependsOn, bindings, arguments: isObject(stepArguments) ? stepArguments as JsonObject : {}, issues: stepIssues, warnings: [] });
  }

  const byId = new Map<string, NormalStep>();
  for (const step of steps) {
    if (byId.has(step.id)) {
      const message = `duplicate mission step id: ${step.id}`;
      issues.push(message);
      step.issues.push(message);
    } else {
      byId.set(step.id, step);
    }
  }

  const dependencies = new Map<string, Set<string>>();
  for (const step of steps) {
    const local = step.issues;
    const seen = new Set<string>();
    const direct = new Set<string>();
    for (const dependency of step.dependsOn) {
      if (dependency === step.id) {
        const message = "step depends on itself";
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      } else if (seen.has(dependency)) {
        const message = `duplicate dependency: ${dependency}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      } else if (!byId.has(dependency)) {
        const message = `unknown dependency: ${dependency}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      } else {
        seen.add(dependency);
        direct.add(dependency);
      }
    }
    dependencies.set(step.id, direct);

    const bindingTargets = new Set<string>();
    for (const binding of step.bindings) {
      if (!validPointer(binding.source_pointer, true)) {
        const message = `invalid binding source pointer: ${binding.source_pointer}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      }
      if (!validPointer(binding.target_pointer, false)) {
        const message = `invalid binding target pointer: ${binding.target_pointer}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      }
      if (!byId.has(binding.from_step)) {
        const message = `binding source is unknown: ${binding.from_step}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      } else if (!direct.has(binding.from_step)) {
        const message = `binding source is not a direct dependency: ${binding.from_step}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      }
      if (bindingTargets.has(binding.target_pointer)) {
        const message = `duplicate binding target: ${binding.target_pointer}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      }
      bindingTargets.add(binding.target_pointer);
      if (!pointerExists(step.arguments, binding.target_pointer)) {
        const message = `binding target does not exist: ${binding.target_pointer}`;
        issues.push(`${step.id}: ${message}`);
        local.push(message);
      }
    }

    if (step.tool === "agent_mission") {
      const message = "agent_mission cannot invoke itself";
      issues.push(`${step.id}: ${message}`);
      local.push(message);
    }
    if (policy.execute && !policy.allowedTools.includes(step.tool)) {
      const message = `tool is not allow-listed: ${step.tool}`;
      issues.push(`${step.id}: ${message}`);
      local.push(message);
    }
    if (policy.execute && !policy.allowSideEffects && containsConfirmation(step.arguments)) {
      const message = "confirmation flag is present while side effects are disabled";
      issues.push(`${step.id}: ${message}`);
      local.push(message);
    }
  }

  if (policy.execute && policy.allowedTools.length === 0) issues.push("execution requires a non-empty explicit allowed_tools list");

  const waves: string[][] = [];
  const remaining = new Map<string, Set<string>>([...dependencies.entries()].map(([id, deps]) => [id, new Set(deps)]));
  while (remaining.size > 0) {
    const ready = [...remaining.entries()].filter(([, deps]) => deps.size === 0).map(([id]) => id).sort();
    if (ready.length === 0) {
      const cycle = [...remaining.keys()].sort();
      const message = `dependency cycle contains: ${cycle.join(", ")}`;
      issues.push(message);
      for (const id of cycle) byId.get(id)?.issues.push(message);
      break;
    }
    waves.push(ready);
    for (const id of ready) remaining.delete(id);
    for (const deps of remaining.values()) ready.forEach((id) => deps.delete(id));
  }
  const waveById = new Map<string, number>();
  waves.forEach((wave, index) => wave.forEach((id) => waveById.set(id, index)));

  const stepResults: MissionStepPreflight[] = [];
  for (const step of steps) {
    let schema: ToolValidationReport | null = null;
    try {
      if (step.tool) {
        schema = catalogue.validate(step.tool, step.arguments);
        if (!schema.ok) {
          for (const issue of schema.issues) {
            const message = `${issue.path}: ${issue.code}: ${issue.message}`;
            step.issues.push(message);
            issues.push(`${step.id}: ${message}`);
          }
        }
        for (const warning of schema.warnings) {
          const message = `${warning.path}: ${warning.code}: ${warning.message}`;
          step.warnings.push(message);
          warnings.push(`${step.id}: ${message}`);
        }
      }
    } catch (error) {
      const message = error instanceof ToolSchemaError ? error.message : String(error);
      step.issues.push(message);
      issues.push(`${step.id}: ${message}`);
    }
    const uniqueIssues = [...new Set(step.issues)];
    const uniqueWarnings = [...new Set(step.warnings)];
    const blocked = uniqueIssues.some((issue) => issue.includes("dependency") || issue.includes("binding") || issue.includes("cycle"));
    stepResults.push({
      id: step.id,
      tool: step.tool,
      depends_on: [...step.dependsOn],
      wave: waveById.get(step.id) ?? null,
      status: uniqueIssues.length > 0 ? blocked ? "blocked" : "invalid" : "ready",
      schema,
      issues: uniqueIssues,
      warnings: uniqueWarnings,
    });
  }

  const uniqueIssues = [...new Set(issues)];
  const uniqueWarnings = [...new Set(warnings)];
  const ok = uniqueIssues.length === 0 && stepResults.every((step) => step.status === "ready");
  const fullyChecked = ok && uniqueWarnings.length === 0 && stepResults.every((step) => step.schema === null || step.schema.fullyChecked);
  const policyValid = issues.length === policyIssueStart;
  return {
    schema: MISSION_PREFLIGHT_SCHEMA,
    mission_id: missionId,
    goal,
    request_digest: requestDigest,
    catalogue_digest: catalogue.digest,
    execution: policy.execute && policyValid ? "authorized" : "planned",
    ok,
    fully_checked: fullyChecked,
    ordered_steps: waves.flat(),
    waves,
    issues: uniqueIssues,
    warnings: uniqueWarnings,
    steps: stepResults,
    limitations: [
      "preflight checks transport shape and mission graph invariants only",
      "the remote MCP server remains authoritative for domain semantics and refusal results",
      "no step is executed by this report",
    ],
  };
}

/** Throw a typed local error when a preflight report is not safe to submit. */
export function assertMissionPreflight(result: MissionPreflightResult): MissionPreflightResult {
  if (!isObject(result) || result.ok !== true) {
    const missionId = isObject(result) && typeof result.mission_id === "string" ? result.mission_id : "<unknown>";
    const topLevel = isObject(result) && Array.isArray(result.issues) ? result.issues.filter((value): value is string => typeof value === "string") : [];
    const stepIssues = isObject(result) && Array.isArray(result.steps)
      ? result.steps.flatMap((step) => isObject(step) && Array.isArray(step.issues)
        ? step.issues.filter((value): value is string => typeof value === "string").map((issue) => `${String(step.id ?? "step")}: ${issue}`)
        : [])
      : [];
    const details = [...topLevel, ...stepIssues].join("; ") || "mission preflight failed";
    throw new MissionPreflightError(`mission ${missionId} failed preflight: ${details}`, result);
  }
  return result;
}

function normalisePolicy(raw: AgentMissionPolicy | undefined, issues: string[]): NormalPolicy {
  if (raw !== undefined && !isObject(raw)) issues.push("policy must be a JSON object");
  const policy = isObject(raw) ? raw : {};
  const execute = booleanValue(policy.execute, false, "policy.execute", issues);
  const stopOnError = booleanValue(policy.stop_on_error, true, "policy.stop_on_error", issues);
  const allowSideEffects = booleanValue(policy.allow_side_effects, false, "policy.allow_side_effects", issues);
  const maxSteps = boundedNumber(policy.max_steps, 64, MAX_MISSION_STEPS, "policy.max_steps", issues);
  const maxStepOutputBytes = boundedNumber(policy.max_step_output_bytes, 2_000_000, MAX_STEP_OUTPUT_BYTES, "policy.max_step_output_bytes", issues);
  const maxTotalOutputBytes = boundedNumber(policy.max_total_output_bytes, 10_000_000, MAX_TOTAL_OUTPUT_BYTES, "policy.max_total_output_bytes", issues);
  if (maxStepOutputBytes > maxTotalOutputBytes) issues.push("policy.max_step_output_bytes cannot exceed policy.max_total_output_bytes");
  const allowed = policy.allowed_tools;
  const allowedTools: string[] = [];
  if (allowed !== undefined && !Array.isArray(allowed)) issues.push("policy.allowed_tools must be an array");
  if (Array.isArray(allowed)) {
    if (allowed.length > MAX_ALLOWED_TOOLS) issues.push(`policy.allowed_tools exceeds ${MAX_ALLOWED_TOOLS} items`);
    for (const tool of allowed) {
      if (typeof tool !== "string" || !tool.trim() || !/^[A-Za-z0-9_]+$/.test(tool)) issues.push(`unsafe allowed tool: ${String(tool)}`);
      else if (allowedTools.includes(tool)) issues.push(`duplicate allowed tool: ${tool}`);
      else if (tool === "agent_mission") issues.push("agent_mission cannot be allow-listed recursively");
      else allowedTools.push(tool);
    }
  }
  return { execute, stopOnError, allowSideEffects, allowedTools, maxSteps, maxStepOutputBytes, maxTotalOutputBytes };
}

function booleanValue(value: unknown, fallback: boolean, name: string, issues: string[]): boolean {
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") {
    issues.push(`${name} must be a boolean`);
    return fallback;
  }
  return value;
}

function boundedNumber(value: unknown, fallback: number, maximum: number, name: string, issues: string[]): number {
  if (value === undefined) return fallback;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1 || value > maximum) {
    issues.push(`${name} must be between 1 and ${maximum}`);
    return fallback;
  }
  return value;
}

function normaliseStringArray(value: unknown, name: string, issues: string[]): string[] {
  if (value === undefined) return [];
  if (!Array.isArray(value)) {
    issues.push(`${name} must be an array`);
    return [];
  }
  const output: string[] = [];
  for (const item of value) {
    if (typeof item !== "string" || !item.trim()) issues.push(`${name} contains a non-empty string violation`);
    else output.push(item);
  }
  return output;
}

function normaliseBindings(value: unknown, name: string, issues: string[]): AgentMissionBinding[] {
  if (value === undefined) return [];
  if (!Array.isArray(value)) {
    issues.push(`${name} must be an array`);
    return [];
  }
  const output: AgentMissionBinding[] = [];
  for (const item of value) {
    if (!isObject(item) || typeof item.from_step !== "string" || typeof item.source_pointer !== "string" || typeof item.target_pointer !== "string") {
      issues.push(`${name} contains an invalid binding`);
    } else {
      output.push(item as AgentMissionBinding);
    }
  }
  return output;
}

function validPointer(pointer: string, allowEmpty: boolean): boolean {
  if (pointer === "") return allowEmpty;
  if (!pointer.startsWith("/") || /[\u0000-\u001f]/.test(pointer)) return false;
  for (let index = 0; index < pointer.length; index += 1) {
    if (pointer[index] === "~" && (pointer[index + 1] !== "0" && pointer[index + 1] !== "1")) return false;
    if (pointer[index] === "~") index += 1;
  }
  return true;
}

function pointerExists(value: unknown, pointer: string): boolean {
  if (!validPointer(pointer, false)) return false;
  let current: unknown = value;
  for (const rawToken of pointer.slice(1).split("/")) {
    const token = rawToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (isObject(current)) {
      if (!(token in current)) return false;
      current = current[token];
    } else if (Array.isArray(current) && /^\d+$/.test(token)) {
      const index = Number(token);
      if (index >= current.length) return false;
      current = current[index];
    } else return false;
  }
  return true;
}

function containsConfirmation(value: unknown): boolean {
  if (isObject(value)) return value.confirm === true || Object.values(value).some((child) => containsConfirmation(child));
  if (Array.isArray(value)) return value.some((child) => containsConfirmation(child));
  return false;
}
