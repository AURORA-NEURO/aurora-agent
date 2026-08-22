import { ArgumentError } from "./errors.js";
import {
  AutonomousConnectorDispatchReceipt,
  AutonomousConnectorDispatchRequest,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  AutonomousConnectorSelectionPlan,
  type AutonomousConnectorDispatchResult,
} from "./autonomous-connectors.js";
import {
  AutonomousConnectorOperationRegistry,
  InMemoryAutonomousConnectorFeedbackLedger,
  type AutonomousConnectorFeedbackEntry,
  type AutonomousConnectorFeedbackInput,
} from "./autonomous-connector-worker.js";
import { IN_MEMORY_PROVIDER_SCHEMA, type AutonomousSelectionDecision, type ProviderResponse } from "./llm.js";
import type {
  AutonomousDomainName,
  AutonomousRunResult,
} from "./autonomous.js";
import type {
  AutonomousWorkflowStageExecutionContext,
  AutonomousWorkflowStageExecutor,
} from "./workflow-execution.js";
import type {
  AutonomousMissionStepExecutionContext,
  AutonomousMissionStepExecutionResult,
  AutonomousMissionExecutorOptions,
  AutonomousMissionStepExecutor,
} from "./mission-execution.js";
import { AutonomousMissionExecutor } from "./mission-execution.js";
import { ToolCatalogue, digestJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

const CONNECTOR_ADAPTER_ID_BYTES = 96;

export interface AutonomousConnectorPayloadRehydrator {
  (receipt: AutonomousConnectorDispatchResult["receipt"]): JsonValue | null | Promise<JsonValue | null>;
}

export interface AutonomousWorkflowConnectorAdapterOptions {
  runtime: AutonomousConnectorRuntime;
  registry?: AutonomousConnectorRegistry;
  /** Optional exact operation vocabulary. When present, every stage is operation-bound. */
  operationRegistry?: AutonomousConnectorOperationRegistry;
  approved?: boolean;
  selectionStrategy?: "lexicographic_connector_id" | "weighted_evidence";
  selectionSignals?: Readonly<Record<string, JsonObject>>;
  /** Explicit evaluator outcomes only; transport success is never treated as reward. */
  feedbackLedger?: InMemoryAutonomousConnectorFeedbackLedger;
  planForStage?: (context: AutonomousWorkflowStageExecutionContext) => AutonomousConnectorSelectionPlan | Promise<AutonomousConnectorSelectionPlan>;
  requestForStage?: (context: AutonomousWorkflowStageExecutionContext) => JsonObject | Promise<JsonObject>;
  rehydratePayload?: AutonomousConnectorPayloadRehydrator;
  onDispatch?: (result: AutonomousConnectorDispatchResult, context: AutonomousWorkflowStageExecutionContext) => void | Promise<void>;
}

export interface AutonomousMissionConnectorAdapterOptions {
  runtime: AutonomousConnectorRuntime;
  registry?: AutonomousConnectorRegistry;
  /** Optional exact operation vocabulary. When present, every mission step is operation-bound. */
  operationRegistry?: AutonomousConnectorOperationRegistry;
  approved?: boolean;
  selectionStrategy?: "lexicographic_connector_id" | "weighted_evidence";
  selectionSignals?: Readonly<Record<string, JsonObject>>;
  /** Explicit evaluator outcomes only; transport success is never treated as reward. */
  feedbackLedger?: InMemoryAutonomousConnectorFeedbackLedger;
  planForStep?: (context: AutonomousMissionStepExecutionContext) => AutonomousConnectorSelectionPlan | Promise<AutonomousConnectorSelectionPlan>;
  requestForStep?: (context: AutonomousMissionStepExecutionContext) => JsonObject | Promise<JsonObject>;
  rehydratePayload?: AutonomousConnectorPayloadRehydrator;
  onDispatch?: (result: AutonomousConnectorDispatchResult, context: AutonomousMissionStepExecutionContext) => void | Promise<void>;
}

function boundedAdapterId(value: string, label: string): string {
  if (!value || value.length > CONNECTOR_ADAPTER_ID_BYTES || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${label} is outside the connector adapter identifier contract`);
  return value;
}

function connectorDomain(value: string, label: string): AutonomousDomainName {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"] as const;
  if (!domains.includes(value as typeof domains[number])) throw new ArgumentError(`${label} is not an autonomous domain`);
  return value as AutonomousDomainName;
}

function connectorPlan(
  plan: AutonomousConnectorSelectionPlan,
  domain: AutonomousDomainName,
  capability: string,
): { plan: AutonomousConnectorSelectionPlan; connectorId: string } {
  if (!(plan instanceof AutonomousConnectorSelectionPlan)) throw new ArgumentError("connector adapter plan selector must return an AutonomousConnectorSelectionPlan");
  if (plan.capability !== capability) throw new ArgumentError("connector adapter selection plan capability does not match the execution contract");
  const row = plan.rows.find((candidate) => candidate.domain === domain);
  if (!row || row.status !== "selected" || !row.connector_id) throw new ArgumentError(`no connector is selected for ${domain}/${capability}`);
  return { plan, connectorId: row.connector_id };
}

function operationFor(
  registry: AutonomousConnectorOperationRegistry | undefined,
  domain: AutonomousDomainName,
  capability: string,
  label: string,
) {
  if (registry === undefined) return null;
  const matches = registry.forDomain(domain).filter((operation) => operation.supports(capability));
  if (matches.length !== 1) throw new ArgumentError(`${label} requires exactly one operation for ${domain}/${capability}`);
  return matches[0] ?? null;
}

function selectionInputs(
  domain: AutonomousDomainName,
  capability: string,
  selectionSignals: Readonly<Record<string, JsonObject>> | undefined,
  feedbackLedger: InMemoryAutonomousConnectorFeedbackLedger | undefined,
  strategy: "lexicographic_connector_id" | "weighted_evidence" | undefined,
): { selectionSignals?: Readonly<Record<string, JsonObject>>; strategy?: "lexicographic_connector_id" | "weighted_evidence" } {
  const feedback = feedbackLedger?.signals({ domain, capability }) ?? {};
  const merged = { ...(selectionSignals ?? {}), ...feedback };
  const hasSignals = Object.keys(merged).length > 0;
  return {
    ...(hasSignals ? { selectionSignals: merged } : {}),
    ...(strategy === undefined && hasSignals ? { strategy: "weighted_evidence" as const } : strategy === undefined ? {} : { strategy }),
  };
}

function attachOperation(request: JsonObject, operation: ReturnType<typeof operationFor>, label: string): JsonObject {
  if (!operation) return request;
  if (request.operation_id !== undefined && request.operation_id !== operation.operation_id) throw new ArgumentError(`${label} request operation_id does not match its domain operation`);
  return { ...request, operation_id: operation.operation_id };
}

async function connectorValue(
  result: AutonomousConnectorDispatchResult,
  rehydrate: AutonomousConnectorPayloadRehydrator | undefined,
): Promise<{ value: JsonValue | null; replayRecoveryRequired: boolean }> {
  if (result.replay !== "replayed" || result.receipt.payload_digest === null || result.value !== null) return { value: result.value, replayRecoveryRequired: false };
  if (!rehydrate) return { value: null, replayRecoveryRequired: true };
  const restored = await rehydrate(result.receipt);
  if (restored === null) return { value: null, replayRecoveryRequired: true };
  try {
    if (digestJsonSync(restored) !== result.receipt.payload_digest) return { value: null, replayRecoveryRequired: true };
  } catch {
    return { value: null, replayRecoveryRequired: true };
  }
  return { value: restored, replayRecoveryRequired: false };
}

function connectorSelection(result: AutonomousConnectorDispatchResult): AutonomousSelectionDecision {
  return {
    selected_model: { provider: result.receipt.provider, model: result.receipt.connector_version },
    strategy: "caller_selector",
    ranking: [{ provider: result.receipt.provider, model: result.receipt.connector_version, score: 1, eligible: true, reasons: ["digest_bound_connector_selection"] }],
    abstention_reason: null,
    selection_confidence: 1,
    min_selection_confidence: null,
  };
}

function connectorResponse(
  result: AutonomousConnectorDispatchResult,
  value: JsonValue | null,
  structured: JsonValue | null,
): ProviderResponse {
  const localValue = value === null ? null : { connector_id: result.receipt.connector_id, payload: value };
  return {
    provider: result.receipt.provider,
    model: result.receipt.connector_version,
    text: JSON.stringify({ receipt: result.receipt.toJSON(), replay: result.replay, value: localValue }),
    statusCode: result.receipt.status === "observed" || result.receipt.status === "partial" ? 200 : 409,
    requestId: result.receipt.dispatch_id,
    usage: {},
    structured,
    toolCalls: [],
    stopReason: "connector_receipt",
    schema: IN_MEMORY_PROVIDER_SCHEMA,
    transport: "caller_owned",
  };
}

function connectorRun(
  context: AutonomousWorkflowStageExecutionContext,
  result: AutonomousConnectorDispatchResult,
  value: JsonValue | null,
  replayRecoveryRequired: boolean,
): AutonomousRunResult {
  const observed = result.receipt.status === "observed";
  const partial = result.receipt.status === "partial";
  const approvalRequired = result.receipt.failure_class === "approval_required";
  const status: AutonomousRunResult["status"] = replayRecoveryRequired
    ? "reconciliation_required"
    : approvalRequired
      ? "approval_required"
      : observed || partial
        ? "completed"
        : "abstained";
  const structured: JsonObject | null = status === "completed"
    ? {
        stage_id: context.stage.id,
        status: observed ? "completed" : "proposed",
        evidence: [`connector:${result.receipt.connector_id}`, `payload:${result.receipt.payload_digest ?? "none"}`],
        uncertainty: [
          ...(partial ? ["connector returned a partial observation"] : []),
          ...(result.replay === "replayed" ? ["connector payload was caller-rehydrated from its digest"] : []),
        ],
        notes: `connector receipt ${result.receipt.request_digest}`,
        next_actions: partial ? ["review partial connector evidence before treating the stage as complete"] : [],
      }
    : null;
  return {
    schema: "bioprism-typescript-autonomous-run/0.1",
    status,
    route: context.route,
    blueprint: context.blueprint,
    plan_refinement_digest: null,
    selection: connectorSelection(result),
    response: connectorResponse(result, value, structured),
    tool_loop: null,
    cross_domain: null,
    learning: "provider_health_feedback_only",
    retention: "provider_response_local; value_only_learning_projection",
  };
}

async function workflowPlan(
  context: AutonomousWorkflowStageExecutionContext,
  options: AutonomousWorkflowConnectorAdapterOptions,
  registry: AutonomousConnectorRegistry,
): Promise<AutonomousConnectorSelectionPlan> {
  if (options.planForStage) return options.planForStage(context);
  const domain = connectorDomain(context.workflow.domain, "workflow connector domain");
  const capability = context.stage.required_capabilities[0];
  if (!capability) throw new ArgumentError(`workflow stage ${context.stage.id} has no connector capability`);
  return registry.selectForDomains([domain], { capability, ...selectionInputs(domain, capability, options.selectionSignals, options.feedbackLedger, options.selectionStrategy) });
}

function defaultWorkflowRequest(context: AutonomousWorkflowStageExecutionContext, operation: ReturnType<typeof operationFor>): JsonObject {
  return {
    stage_id: context.stage.id,
    workflow_id: context.workflow.workflow_id,
    workflow_digest: context.workflow.workflow_digest,
    task_digest: context.task_digest,
    objective: context.stage.objective,
    ...(operation ? { operation_id: operation.operation_id } : {}),
  };
}

/** Bind a connector portfolio to the durable workflow stage executor contract. */
export function autonomousConnectorWorkflowStageExecutor(options: AutonomousWorkflowConnectorAdapterOptions): AutonomousWorkflowStageExecutor {
  if (!options || !(options.runtime instanceof AutonomousConnectorRuntime)) throw new ArgumentError("workflow connector adapter requires an AutonomousConnectorRuntime");
  if (options.operationRegistry !== undefined && !(options.operationRegistry instanceof AutonomousConnectorOperationRegistry)) throw new ArgumentError("workflow connector operationRegistry is invalid");
  if (options.feedbackLedger !== undefined && !(options.feedbackLedger instanceof InMemoryAutonomousConnectorFeedbackLedger)) throw new ArgumentError("workflow connector feedbackLedger is invalid");
  const registry = options.registry ?? options.runtime.registry;
  if (registry !== options.runtime.registry) throw new ArgumentError("workflow connector adapter registry must match its runtime");
  if (options.requestForStage !== undefined && typeof options.requestForStage !== "function") throw new ArgumentError("workflow connector requestForStage must be callable");
  if (options.onDispatch !== undefined && typeof options.onDispatch !== "function") throw new ArgumentError("workflow connector onDispatch must be callable");
  return async (context) => {
    const domain = connectorDomain(context.workflow.domain, "workflow connector domain");
    const capability = context.stage.required_capabilities[0];
    if (!capability) throw new ArgumentError(`workflow stage ${context.stage.id} has no connector capability`);
    const operation = operationFor(options.operationRegistry, domain, capability, "workflow connector");
    const plan = connectorPlan(await workflowPlan(context, options, registry), domain, capability);
    const stageDigest = await digestJson(context.stage);
    const identityDigest = await digestJson({ job_id: context.job_id, stage_id: context.stage.id, stage_attempt: context.stage_attempt, execution_contract_digest: context.execution_contract_digest });
    const request = new AutonomousConnectorDispatchRequest({
      dispatch_id: boundedAdapterId(`workflow-dispatch-${identityDigest.slice(0, 48)}`, "workflow connector dispatch_id"),
      execution_id: boundedAdapterId(`workflow-execution-${identityDigest.slice(0, 48)}`, "workflow connector execution_id"),
      call_id: boundedAdapterId(`workflow-call-${identityDigest.slice(0, 48)}`, "workflow connector call_id"),
      connector_id: plan.connectorId,
      domains: [domain],
      capability,
      request: attachOperation(await (options.requestForStage?.(context) ?? defaultWorkflowRequest(context, operation)), operation, "workflow connector"),
      parent_digests: [context.route.route_digest, context.workflow.workflow_digest, context.execution_contract_digest, stageDigest],
      attempt_id: boundedAdapterId(`a${context.stage_attempt}`, "workflow connector attempt_id"),
      selection_plan_digest: plan.plan.plan_digest,
      approved: options.approved === true,
    });
    if (operation) operation.assertRequest(request);
    const result = await options.runtime.dispatchFromPlan(plan.plan, request);
    await options.onDispatch?.(result, context);
    const resolved = await connectorValue(result, options.rehydratePayload);
    return connectorRun(context, result, resolved.value, resolved.replayRecoveryRequired);
  };
}

async function missionPlan(
  context: AutonomousMissionStepExecutionContext,
  options: AutonomousMissionConnectorAdapterOptions,
  registry: AutonomousConnectorRegistry,
): Promise<AutonomousConnectorSelectionPlan> {
  if (options.planForStep) return options.planForStep(context);
  const domain = connectorDomain(context.step.domain, "mission connector domain");
  return registry.selectForDomains([domain], { capability: context.step.capability, ...selectionInputs(domain, context.step.capability, options.selectionSignals, options.feedbackLedger, options.selectionStrategy) });
}

function defaultMissionRequest(context: AutonomousMissionStepExecutionContext, goalDigest: string, operation: ReturnType<typeof operationFor>): JsonObject {
  // Keep both the structured argument envelope and the flattened recommended fields.
  // Connector contracts can consume stable fields without needing to understand the
  // mission transport, while the envelope remains available for custom adapters.
  return {
    ...context.arguments,
    mission_id: context.mission_id,
    step_id: context.step.id,
    domain: context.step.domain,
    capability: context.step.capability,
    objective: context.step.objective,
    goal_digest: goalDigest,
    arguments: context.arguments,
    ...(operation ? { operation_id: operation.operation_id } : {}),
  };
}

/** Bind a connector portfolio to the mission executor's strict step contract. */
export function autonomousConnectorMissionStepExecutor(options: AutonomousMissionConnectorAdapterOptions): AutonomousMissionStepExecutor {
  if (!options || !(options.runtime instanceof AutonomousConnectorRuntime)) throw new ArgumentError("mission connector adapter requires an AutonomousConnectorRuntime");
  if (options.operationRegistry !== undefined && !(options.operationRegistry instanceof AutonomousConnectorOperationRegistry)) throw new ArgumentError("mission connector operationRegistry is invalid");
  if (options.feedbackLedger !== undefined && !(options.feedbackLedger instanceof InMemoryAutonomousConnectorFeedbackLedger)) throw new ArgumentError("mission connector feedbackLedger is invalid");
  const registry = options.registry ?? options.runtime.registry;
  if (registry !== options.runtime.registry) throw new ArgumentError("mission connector adapter registry must match its runtime");
  if (options.requestForStep !== undefined && typeof options.requestForStep !== "function") throw new ArgumentError("mission connector requestForStep must be callable");
  if (options.onDispatch !== undefined && typeof options.onDispatch !== "function") throw new ArgumentError("mission connector onDispatch must be callable");
  return async (context): Promise<AutonomousMissionStepExecutionResult> => {
    const domain = connectorDomain(context.step.domain, "mission connector domain");
    const argumentDigest = await digestJson(context.arguments);
    const goalDigest = await digestJson({ goal: context.goal });
    const operation = operationFor(options.operationRegistry, domain, context.step.capability, "mission connector");
    const plan = connectorPlan(await missionPlan(context, options, registry), domain, context.step.capability);
    const stepDigest = await digestJson({ id: context.step.id, domain, capability: context.step.capability, objective: context.step.objective, tool: context.step.tool });
    const identityDigest = await digestJson({ mission_id: context.mission_id, step_id: context.step.id, attempt: context.execution_attempt, argument_digest: argumentDigest });
    const requestPayload = attachOperation(await (options.requestForStep?.(context) ?? defaultMissionRequest(context, goalDigest, operation)), operation, "mission connector");
    const request = new AutonomousConnectorDispatchRequest({
      dispatch_id: boundedAdapterId(`mission-dispatch-${identityDigest.slice(0, 48)}`, "mission connector dispatch_id"),
      execution_id: boundedAdapterId(`mission-execution-${identityDigest.slice(0, 48)}`, "mission connector execution_id"),
      call_id: boundedAdapterId(`mission-call-${identityDigest.slice(0, 48)}`, "mission connector call_id"),
      connector_id: plan.connectorId,
      domains: [domain],
      capability: context.step.capability,
      request: requestPayload,
      parent_digests: [stepDigest, argumentDigest, goalDigest, ...(operation ? [operation.operation_digest] : []), ...(Object.keys(context.dependency_outputs).length ? [await digestJson(context.dependency_outputs)] : [])],
      attempt_id: boundedAdapterId(`a${context.execution_attempt}`, "mission connector attempt_id"),
      selection_plan_digest: plan.plan.plan_digest,
      approved: options.approved === true,
    });
    if (operation) operation.assertRequest(request);
    const result = await options.runtime.dispatchFromPlan(plan.plan, request);
    await options.onDispatch?.(result, context);
    const resolved = await connectorValue(result, options.rehydratePayload);
    const decision = {
      selection_digest: plan.plan.plan_digest,
      provider: result.receipt.provider,
      model: result.receipt.connector_version,
      route_digest: null,
      plan_digest: plan.plan.plan_digest,
      prompt_digest: await digestJson({ step_digest: stepDigest, request_digest: request.request_digest }),
    } as const;
    if (resolved.replayRecoveryRequired) return { status: "reconciliation_required", error_class: "ConnectorResultRehydrationRequired", detail: "connector receipt is replayed but its caller-owned payload was not rehydrated", run_status: "reconciliation_required", decision };
    if (result.receipt.failure_class === "approval_required") return { status: "approval_required", error_class: "ConnectorApprovalRequired", detail: "connector dispatch requires explicit approval", run_status: "approval_required", decision };
    if (result.receipt.status === "refused") return { status: "refused", error_class: result.receipt.failure_class ?? "ConnectorRefused", detail: "connector dispatch was refused by its scope or policy gate", run_status: "refused", decision };
    if (result.receipt.status === "error" || result.receipt.status === "unknown") return { status: "failed", error_class: result.receipt.failure_class ?? "ConnectorExecutionFailed", detail: "connector dispatch did not produce an observation", run_status: result.receipt.status, decision };
    return { status: "succeeded", value: resolved.value, run_status: result.receipt.status === "partial" ? "connector_partial" : "connector_observed", decision };
  };
}

/**
 * Compose the connector adapter with the SDK's durable mission executor in one call.
 * The returned executor owns only checkpoint metadata; connector values remain in the
 * caller's result store and replay payloads still require explicit rehydration.
 */
export interface AutonomousConnectorMissionExecutorOptions extends Omit<AutonomousMissionExecutorOptions, "catalogue" | "executeStep"> {
  catalogue: ToolCatalogue;
  connector: AutonomousMissionConnectorAdapterOptions;
}

export function autonomousConnectorMissionExecutor(options: AutonomousConnectorMissionExecutorOptions): AutonomousMissionExecutor {
  if (!options || !(options.catalogue instanceof ToolCatalogue)) throw new ArgumentError("connector mission executor requires a ToolCatalogue");
  if (!options.connector || typeof options.connector !== "object") throw new ArgumentError("connector mission executor options are malformed");
  const { connector, ...executorOptions } = options;
  return new AutonomousMissionExecutor({
    ...executorOptions,
    catalogue: options.catalogue,
    executeStep: autonomousConnectorMissionStepExecutor(connector),
  });
}

/** Record evaluator-owned reward after a connector receipt has been reviewed. */
export function settleAutonomousConnectorEvaluatorFeedback(
  ledger: InMemoryAutonomousConnectorFeedbackLedger,
  receipt: AutonomousConnectorDispatchReceipt,
  feedback: AutonomousConnectorFeedbackInput,
  now?: number,
): AutonomousConnectorFeedbackEntry {
  if (!(ledger instanceof InMemoryAutonomousConnectorFeedbackLedger)) throw new ArgumentError("connector feedback ledger is invalid");
  if (!(receipt instanceof AutonomousConnectorDispatchReceipt)) throw new ArgumentError("connector feedback requires a typed receipt");
  return ledger.record({ feedback, receipt, ...(now === undefined ? {} : { now }) });
}
