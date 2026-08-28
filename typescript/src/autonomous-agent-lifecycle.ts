import { ArgumentError } from "./errors.js";
import { digestJson } from "./tooling.js";
import type { AutonomousAgent } from "./autonomous.js";
import type { AutonomousCapabilityActivationSnapshotStore } from "./autonomous-activation.js";
import type { AutonomousModelInventoryPersistence } from "./autonomous-model-inventory.js";
import type { AutonomousSelectionLifecycleStore } from "./autonomous-selection-lifecycle.js";
import type { AutonomousCapabilityJournalPersistenceCoordinator } from "./autonomous-capability-persistence.js";
import type { AutonomousExecutionPersistenceCoordinator } from "./autonomous-execution.js";
import type { AutonomousDecisionCyclePersistenceCoordinator } from "./autonomous-decision-persistence.js";

/**
 * Strict startup/shutdown composition for the autonomous brain. Each component keeps its own
 * persistence/CAS boundary; this coordinator only orders those boundaries and reports their
 * metadata. It never retains tasks, prompts, provider payloads, credentials, evidence, tool
 * arguments, or raw exception messages, and it never claims cross-store atomicity.
 */
export const AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA = "bioprism-typescript-autonomous-agent-persistence-lifecycle/0.1" as const;
export const AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS = ["model_inventory", "runtime_health", "health", "activation", "selection_promotion", "evaluator_calibration", "memory", "learning", "prompt_learning", "capability_journal", "decision_cycle", "execution"] as const;
/** Optional components are appended to lifecycle reports only when configured on the agent. */
export const AUTONOMOUS_AGENT_LIFECYCLE_OPTIONAL_COMPONENTS = ["tool_selection"] as const;
export const AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER = AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS;
export const AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER = [...AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS].reverse() as unknown as typeof AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS;

export type AutonomousAgentPersistenceLifecycleComponent = typeof AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS[number] | typeof AUTONOMOUS_AGENT_LIFECYCLE_OPTIONAL_COMPONENTS[number];
export type AutonomousAgentPersistenceLifecycleOperation = "restore" | "flush";
export type AutonomousAgentPersistenceLifecycleStatus = "completed" | "partial" | "failed" | "empty" | "unconfigured";
export type AutonomousAgentPersistenceLifecycleComponentStatus = "restored" | "flushed" | "empty" | "unconfigured" | "not_attempted" | "failed";

export interface AutonomousAgentPersistenceComponentResult {
  schema: typeof AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA;
  component_id: AutonomousAgentPersistenceLifecycleComponent;
  operation: AutonomousAgentPersistenceLifecycleOperation;
  status: AutonomousAgentPersistenceLifecycleComponentStatus;
  snapshot_schema: string | null;
  snapshot_digest: string | null;
  state_digest: string | null;
  generation: number | null;
  error_class: string | null;
  component_digest: string;
  retention: "component_metadata_only;cross_store_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousAgentPersistenceLifecycleReport {
  schema: typeof AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA;
  operation: AutonomousAgentPersistenceLifecycleOperation;
  status: AutonomousAgentPersistenceLifecycleStatus;
  ordered_component_ids: AutonomousAgentPersistenceLifecycleComponent[];
  completed_component_ids: AutonomousAgentPersistenceLifecycleComponent[];
  unconfigured_component_ids: AutonomousAgentPersistenceLifecycleComponent[];
  failed_component_id: AutonomousAgentPersistenceLifecycleComponent | null;
  components: AutonomousAgentPersistenceComponentResult[];
  next_action: string;
  atomicity: "per_component_cas_only;cross_store_atomicity_caller_owned";
  lifecycle_digest: string;
  retention: "component_metadata_only;cross_store_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousAgentPersistenceLifecycleOptions {
  modelInventoryPersistence?: AutonomousModelInventoryPersistence;
  activationStore?: AutonomousCapabilityActivationSnapshotStore;
  selectionPromotionStore?: AutonomousSelectionLifecycleStore;
  capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
  decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
  executionPersistence?: AutonomousExecutionPersistenceCoordinator;
  requireAll?: boolean;
  continueOnError?: boolean;
}

export interface AutonomousAgentPersistenceLifecycleRunOptions {
  strict?: boolean;
  continueOnError?: boolean;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function boundedDigest(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError("lifecycle snapshot digest is invalid");
  return value;
}

function errorClass(error: unknown): string {
  const name = error instanceof Error && typeof error.name === "string" ? error.name : "UnknownError";
  return /^[A-Za-z0-9_.-]{1,128}$/.test(name) ? name : "UnknownError";
}

function projection(value: unknown): { schema: string | null; snapshotDigest: string | null; stateDigest: string | null; generation: number | null } {
  if (!isObject(value)) return { schema: null, snapshotDigest: null, stateDigest: null, generation: null };
  const schema = typeof value.schema === "string" ? value.schema : null;
  let snapshotDigest: string | null = null;
  for (const key of ["snapshot_digest", "inventory_digest", "report_digest", "state_digest", "memory_digest", "prompt_learning_digest", "digest"]) {
    if (value[key] !== undefined && value[key] !== null) { snapshotDigest = boundedDigest(value[key]); break; }
  }
  const stateDigest = boundedDigest(value.state_digest);
  const generationValue = value.generation ?? value.snapshot_generation;
  const generation = Number.isSafeInteger(generationValue) && (generationValue as number) >= 0 ? generationValue as number : null;
  return { schema, snapshotDigest, stateDigest, generation };
}

async function componentResult(
  componentId: AutonomousAgentPersistenceLifecycleComponent,
  operation: AutonomousAgentPersistenceLifecycleOperation,
  status: AutonomousAgentPersistenceLifecycleComponentStatus,
  value?: unknown,
  failure?: unknown,
): Promise<AutonomousAgentPersistenceComponentResult> {
  const row = projection(value);
  const payload = {
    schema: AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA,
    component_id: componentId,
    operation,
    status,
    snapshot_schema: row.schema,
    snapshot_digest: row.snapshotDigest,
    state_digest: row.stateDigest,
    generation: row.generation,
    error_class: failure === undefined ? null : errorClass(failure),
  };
  return { ...payload, component_digest: await digestJson(payload), retention: "component_metadata_only;cross_store_payloads_caller_owned", secret_material: "never_returned" };
}

export class AutonomousAgentPersistenceLifecycleError extends ArgumentError {
  readonly operation: AutonomousAgentPersistenceLifecycleOperation;
  readonly report: AutonomousAgentPersistenceLifecycleReport;
  constructor(operation: AutonomousAgentPersistenceLifecycleOperation, report: AutonomousAgentPersistenceLifecycleReport) {
    super(`autonomous agent persistence ${operation} did not complete`);
    this.operation = operation;
    this.report = report;
  }
}

/** Orders the configured agent coordinators and exposes a strict metadata-only lifecycle report. */
export class AutonomousAgentPersistenceLifecycleCoordinator {
  readonly modelInventoryPersistence?: AutonomousModelInventoryPersistence;
  readonly activationStore?: AutonomousCapabilityActivationSnapshotStore;
  readonly selectionPromotionStore?: AutonomousSelectionLifecycleStore;
  readonly capabilityJournalPersistence?: AutonomousCapabilityJournalPersistenceCoordinator;
  readonly decisionCyclePersistence?: AutonomousDecisionCyclePersistenceCoordinator;
  readonly executionPersistence?: AutonomousExecutionPersistenceCoordinator;
  readonly requireAll: boolean;
  readonly continueOnError: boolean;
  private readonly agent: AutonomousAgent;
  private lastReport: AutonomousAgentPersistenceLifecycleReport | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(agent: AutonomousAgent, options: AutonomousAgentPersistenceLifecycleOptions = {}) {
    if (!agent || typeof agent.restoreModelInventory !== "function" || typeof agent.flushModelInventory !== "function") throw new ArgumentError("agent persistence lifecycle requires an AutonomousAgent");
    if (options.modelInventoryPersistence !== undefined && (typeof options.modelInventoryPersistence.read !== "function" || typeof options.modelInventoryPersistence.write !== "function")) throw new ArgumentError("agent lifecycle model inventory persistence is malformed");
    if (options.activationStore !== undefined && (typeof options.activationStore.load !== "function" || typeof options.activationStore.save !== "function")) throw new ArgumentError("agent lifecycle activation store is malformed");
    if (options.selectionPromotionStore !== undefined && (typeof options.selectionPromotionStore.load !== "function" || typeof options.selectionPromotionStore.save !== "function")) throw new ArgumentError("agent lifecycle selection promotion store is malformed");
    if (options.selectionPromotionStore !== undefined && !(agent as unknown as { selectionPromotion?: unknown }).selectionPromotion) throw new ArgumentError("agent lifecycle selection promotion store requires a configured lifecycle");
    if (options.capabilityJournalPersistence !== undefined && (typeof options.capabilityJournalPersistence.restore !== "function" || typeof options.capabilityJournalPersistence.flush !== "function")) throw new ArgumentError("agent lifecycle capability journal persistence is malformed");
    if (options.capabilityJournalPersistence !== undefined && (agent as unknown as { capabilityJournalPersistence?: unknown }).capabilityJournalPersistence !== options.capabilityJournalPersistence) throw new ArgumentError("agent lifecycle capability journal persistence must be bound to the agent");
    if (options.decisionCyclePersistence !== undefined && (typeof options.decisionCyclePersistence.restore !== "function" || typeof options.decisionCyclePersistence.flush !== "function")) throw new ArgumentError("agent lifecycle decision-cycle persistence is malformed");
    if (options.decisionCyclePersistence !== undefined && (agent as unknown as { decisionCyclePersistence?: unknown }).decisionCyclePersistence !== options.decisionCyclePersistence) throw new ArgumentError("agent lifecycle decision-cycle persistence must be bound to the agent");
    if (options.executionPersistence !== undefined && (typeof options.executionPersistence.restore !== "function" || typeof options.executionPersistence.flush !== "function")) throw new ArgumentError("agent lifecycle execution persistence is malformed");
    if (options.executionPersistence !== undefined && (agent as unknown as { executionPersistence?: unknown }).executionPersistence !== options.executionPersistence) throw new ArgumentError("agent lifecycle execution persistence must be bound to the agent");
    if (options.requireAll !== undefined && typeof options.requireAll !== "boolean") throw new ArgumentError("agent lifecycle requireAll must be boolean");
    if (options.continueOnError !== undefined && typeof options.continueOnError !== "boolean") throw new ArgumentError("agent lifecycle continueOnError must be boolean");
    this.agent = agent;
    this.modelInventoryPersistence = options.modelInventoryPersistence;
    this.activationStore = options.activationStore;
    this.selectionPromotionStore = options.selectionPromotionStore;
    this.capabilityJournalPersistence = options.capabilityJournalPersistence;
    this.decisionCyclePersistence = options.decisionCyclePersistence;
    this.executionPersistence = options.executionPersistence;
    this.requireAll = options.requireAll ?? false;
    this.continueOnError = options.continueOnError ?? false;
  }

  getLastReport(): AutonomousAgentPersistenceLifecycleReport | null { return this.lastReport === null ? null : structuredClone(this.lastReport); }

  private configured(componentId: AutonomousAgentPersistenceLifecycleComponent): boolean {
    if (componentId === "tool_selection") return Boolean((this.agent as unknown as { toolSelectionPersistence?: unknown }).toolSelectionPersistence);
    if (componentId === "model_inventory") return this.modelInventoryPersistence !== undefined;
    if (componentId === "activation") return this.activationStore !== undefined;
    if (componentId === "selection_promotion") return this.selectionPromotionStore !== undefined && Boolean((this.agent as unknown as { selectionPromotion?: unknown }).selectionPromotion);
    if (componentId === "capability_journal") return Boolean(this.capabilityJournalPersistence ?? (this.agent as unknown as { capabilityJournalPersistence?: unknown }).capabilityJournalPersistence);
    if (componentId === "decision_cycle") return Boolean(this.decisionCyclePersistence ?? (this.agent as unknown as { decisionCyclePersistence?: unknown }).decisionCyclePersistence);
    if (componentId === "execution") return Boolean(this.executionPersistence ?? (this.agent as unknown as { executionPersistence?: unknown }).executionPersistence);
    if (componentId === "health") {
      const typed = this.agent as unknown as { modelHealthPersistence?: unknown; healthPersistence?: unknown };
      // `healthPersistence` is retained for lightweight test doubles and older embeddings;
      // the concrete agent uses the more precise model-health name.
      return Boolean(typed.modelHealthPersistence ?? typed.healthPersistence);
    }
    const names = {
      runtime_health: "runtimeHealthPersistence",
      evaluator_calibration: "evaluatorCalibrationPersistence",
      memory: "memoryPersistence",
      learning: "learnerPersistence",
      prompt_learning: "promptLearningCoordinator",
      tool_selection: "toolSelectionPersistence",
    } as const;
    const propertyName = names[componentId as keyof typeof names];
    return propertyName !== undefined && Boolean((this.agent as unknown as Record<string, unknown>)[propertyName]);
  }

  private orderedComponents(operation: AutonomousAgentPersistenceLifecycleOperation): AutonomousAgentPersistenceLifecycleComponent[] {
    const base = [...(operation === "restore" ? AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER : AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER)] as AutonomousAgentPersistenceLifecycleComponent[];
    if (!this.configured("tool_selection")) return base;
    const insertionIndex = operation === "restore" ? 9 : 4;
    base.splice(insertionIndex, 0, "tool_selection");
    return base;
  }

  private async invoke(componentId: AutonomousAgentPersistenceLifecycleComponent, operation: AutonomousAgentPersistenceLifecycleOperation): Promise<unknown> {
    if (componentId === "model_inventory") return operation === "restore" ? this.agent.restoreModelInventory(this.modelInventoryPersistence!) : this.agent.flushModelInventory(this.modelInventoryPersistence!);
    if (componentId === "activation") return operation === "restore" ? this.agent.restoreActivation(this.activationStore!) : this.agent.saveActivation(this.activationStore!);
    if (componentId === "selection_promotion") return operation === "restore" ? this.agent.restoreSelectionPromotion(this.selectionPromotionStore!) : this.agent.saveSelectionPromotion(this.selectionPromotionStore!);
    if (componentId === "capability_journal") return operation === "restore" ? this.agent.restoreCapabilityJournalPersistence() : this.agent.flushCapabilityJournalPersistence();
    if (componentId === "decision_cycle") return operation === "restore" ? this.agent.restoreDecisionCyclePersistence() : this.agent.flushDecisionCyclePersistence();
    if (componentId === "execution") return operation === "restore" ? this.agent.restoreExecutionPersistence() : this.agent.flushExecutionPersistence();
    const names: Record<AutonomousAgentPersistenceLifecycleComponent, [string, string]> = {
      model_inventory: ["restoreModelInventory", "flushModelInventory"],
      runtime_health: ["restoreRuntimeHealth", "flushRuntimeHealth"],
      health: ["restoreHealth", "flushHealth"],
      activation: ["restoreActivation", "saveActivation"],
      selection_promotion: ["restoreSelectionPromotion", "saveSelectionPromotion"],
      evaluator_calibration: ["restoreEvaluatorCalibration", "flushEvaluatorCalibration"],
      memory: ["restoreMemory", "flushMemory"],
      learning: ["restoreOnlineLearning", "flushOnlineLearning"],
      prompt_learning: ["restorePromptLearning", "flushPromptLearning"],
      tool_selection: ["restoreToolSelection", "flushToolSelection"],
      capability_journal: ["restoreCapabilityJournalPersistence", "flushCapabilityJournalPersistence"],
      decision_cycle: ["restoreDecisionCyclePersistence", "flushDecisionCyclePersistence"],
      execution: ["restoreExecutionPersistence", "flushExecutionPersistence"],
    };
    const methodNames = names[componentId];
    if (methodNames === undefined) throw new ArgumentError(`agent does not expose ${operation} for ${componentId}`);
    const methodName = methodNames[operation === "restore" ? 0 : 1];
    if (methodName === undefined) throw new ArgumentError(`agent does not expose ${operation} for ${componentId}`);
    const method = (this.agent as unknown as Record<string, unknown>)[methodName];
    if (typeof method !== "function") throw new ArgumentError(`agent does not expose ${operation} for ${componentId}`);
    return (method as () => Promise<unknown>).call(this.agent);
  }

  private async makeReport(operation: AutonomousAgentPersistenceLifecycleOperation, ordered: readonly AutonomousAgentPersistenceLifecycleComponent[], results: AutonomousAgentPersistenceComponentResult[], failedComponentId: AutonomousAgentPersistenceLifecycleComponent | null): Promise<AutonomousAgentPersistenceLifecycleReport> {
    while (results.length < ordered.length) {
      const componentId = ordered[results.length];
      if (componentId === undefined) break;
      results.push(await componentResult(componentId, operation, "not_attempted"));
    }
    const completed = results.filter((row) => row.status === "restored" || row.status === "flushed" || row.status === "empty").map((row) => row.component_id);
    const unconfigured = results.filter((row) => row.status === "unconfigured").map((row) => row.component_id);
    const failed = failedComponentId !== null || results.some((row) => row.status === "failed");
    const configured = ordered.length - unconfigured.length;
    let status: AutonomousAgentPersistenceLifecycleStatus;
    let nextAction: string;
    if (failed) { status = completed.length === 0 ? "failed" : "partial"; nextAction = "recover_failed_components_before_execution"; }
    else if (configured === 0) { status = "unconfigured"; nextAction = "bind_persistence_coordinators_before_execution"; }
    else if (unconfigured.length > 0) { status = "partial"; nextAction = "bind_unconfigured_persistence_or_accept_partial_lifecycle"; }
    else if (results.every((row) => row.status === "empty")) { status = "empty"; nextAction = "safe_to_begin_execution_with_empty_persistence"; }
    else { status = "completed"; nextAction = operation === "restore" ? "safe_to_begin_execution" : "safe_to_finalize_process"; }
    const payload = { schema: AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA, operation, status, ordered_component_ids: [...ordered], completed_component_ids: completed, unconfigured_component_ids: unconfigured, failed_component_id: failedComponentId, components: results, next_action: nextAction, atomicity: "per_component_cas_only;cross_store_atomicity_caller_owned" as const };
    return { ...payload, lifecycle_digest: await digestJson(payload), retention: "component_metadata_only;cross_store_payloads_caller_owned", secret_material: "never_returned" };
  }

  private async run(operation: AutonomousAgentPersistenceLifecycleOperation, options: AutonomousAgentPersistenceLifecycleRunOptions): Promise<AutonomousAgentPersistenceLifecycleReport> {
    if (options.strict !== undefined && typeof options.strict !== "boolean") throw new ArgumentError("agent lifecycle strict must be boolean");
    if (options.continueOnError !== undefined && typeof options.continueOnError !== "boolean") throw new ArgumentError("agent lifecycle continueOnError must be boolean");
    const strict = options.strict ?? true;
    const continueOnError = options.continueOnError ?? this.continueOnError;
    const ordered = this.orderedComponents(operation);
    const results: AutonomousAgentPersistenceComponentResult[] = [];
    let failedComponentId: AutonomousAgentPersistenceLifecycleComponent | null = null;
    for (const componentId of ordered) {
      if (!this.configured(componentId)) {
        results.push(await componentResult(componentId, operation, "unconfigured"));
        if (this.requireAll && failedComponentId === null) failedComponentId = componentId;
        if (this.requireAll && !continueOnError) break;
        continue;
      }
      try {
        const value = await this.invoke(componentId, operation);
        results.push(await componentResult(componentId, operation, value === null ? "empty" : operation === "restore" ? "restored" : "flushed", value));
      } catch (error) {
        results.push(await componentResult(componentId, operation, "failed", undefined, error));
        if (failedComponentId === null) failedComponentId = componentId;
        if (!continueOnError) break;
      }
    }
    const report = await this.makeReport(operation, ordered, results, failedComponentId);
    this.lastReport = report;
    if (strict && failedComponentId !== null) throw new AutonomousAgentPersistenceLifecycleError(operation, report);
    return structuredClone(report);
  }

  restore(options: AutonomousAgentPersistenceLifecycleRunOptions = {}): Promise<AutonomousAgentPersistenceLifecycleReport> { return this.enqueue(() => this.run("restore", options)); }
  flush(options: AutonomousAgentPersistenceLifecycleRunOptions = {}): Promise<AutonomousAgentPersistenceLifecycleReport> { return this.enqueue(() => this.run("flush", options)); }
  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(operation);
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}
