import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import type {
  AutonomousAgent,
  AutonomousDomainName,
  AutonomousRunOptions,
  AutonomousRunResult,
  AutonomousTaskBlueprint,
  AutonomousWorkflowStage,
} from "./autonomous.js";
import { digestJson } from "./tooling.js";

export const AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA = "bioprism-typescript-autonomous-workflow-execution/0.1" as const;
export const AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-workflow-checkpoint/0.1" as const;
export const AUTONOMOUS_WORKFLOW_EVENT_SCHEMA = "bioprism-typescript-autonomous-workflow-event/0.1" as const;
export const AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL = 32;
export const AUTONOMOUS_WORKFLOW_MAX_EVENTS = 256;

export type AutonomousWorkflowCheckpointStatus = "running" | "paused" | "completed" | "failed";
export type AutonomousWorkflowExecutionStatus = "completed" | "paused" | "approval_required" | "failed" | "route_review_required";
export type AutonomousWorkflowEventType = "started" | "stage_completed" | "checkpointed" | "approval_required" | "stage_failed" | "completed";

export interface AutonomousWorkflowStageOutcome {
  stage_id: string;
  status: "completed" | "approval_required" | "failed";
  run_status: string;
  selection_digest: string | null;
  response_digest: string | null;
  output_bytes: number;
  error_class: string | null;
}

/** Metadata-only restart checkpoint. Task text, prompts, credentials, and provider responses are never persisted here. */
export interface AutonomousWorkflowCheckpoint {
  schema: typeof AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA;
  job_id: string;
  task_digest: string;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  plan_digest: string;
  completed_stage_ids: string[];
  next_stage_id: string | null;
  stage_outcomes: AutonomousWorkflowStageOutcome[];
  generation: number;
  status: AutonomousWorkflowCheckpointStatus;
  previous_checkpoint_digest: string | null;
  checkpoint_digest: string;
  retention: "metadata_only;task_prompt_response_and_credentials_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowEvent {
  schema: typeof AUTONOMOUS_WORKFLOW_EVENT_SCHEMA;
  sequence: number;
  job_id: string;
  event_type: AutonomousWorkflowEventType;
  stage_id: string | null;
  checkpoint_digest: string;
  previous_event_digest: string | null;
  event_digest: string;
  retention: "metadata_only;provider_payloads_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowCheckpointStore {
  load(jobId: string): Promise<AutonomousWorkflowCheckpoint | null> | AutonomousWorkflowCheckpoint | null;
  save(checkpoint: AutonomousWorkflowCheckpoint): Promise<void> | void;
  appendEvent(event: AutonomousWorkflowEvent): Promise<void> | void;
  events(jobId: string, after?: number, limit?: number): Promise<AutonomousWorkflowEvent[]> | AutonomousWorkflowEvent[];
}

export interface AutonomousWorkflowStageResult {
  stage: AutonomousWorkflowStage;
  run: AutonomousRunResult | null;
  output_digest: string | null;
  output_bytes: number;
}

export interface AutonomousWorkflowExecutionResult {
  schema: typeof AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA;
  status: AutonomousWorkflowExecutionStatus;
  job_id: string | null;
  blueprint: AutonomousTaskBlueprint | null;
  checkpoint: AutonomousWorkflowCheckpoint | null;
  events: AutonomousWorkflowEvent[];
  stage_results: AutonomousWorkflowStageResult[];
  completed_stage_count: number;
  total_stage_count: number;
  recovery: "caller_rehydrates_task_and_credentials";
  retention: "provider_responses_local;checkpoint_metadata_only";
}

export interface AutonomousWorkflowExecuteOptions extends AutonomousRunOptions {
  jobId?: string;
  maxStages?: number;
}

/** A bounded process-local store useful for tests and small workers; production callers can replace it with SQLite/Redis/etc. */
export class InMemoryAutonomousWorkflowCheckpointStore implements AutonomousWorkflowCheckpointStore {
  private readonly checkpoints = new Map<string, AutonomousWorkflowCheckpoint>();
  private readonly eventRows = new Map<string, AutonomousWorkflowEvent[]>();

  load(jobId: string): AutonomousWorkflowCheckpoint | null {
    const checkpoint = this.checkpoints.get(jobId);
    return checkpoint ? structuredClone(checkpoint) : null;
  }

  save(checkpoint: AutonomousWorkflowCheckpoint): void {
    this.checkpoints.set(checkpoint.job_id, structuredClone(checkpoint));
  }

  appendEvent(event: AutonomousWorkflowEvent): void {
    const rows = this.eventRows.get(event.job_id) ?? [];
    if (rows.length && event.sequence !== rows[rows.length - 1]!.sequence + 1) throw new ArgumentError("workflow event sequence must be contiguous");
    if (!rows.length && event.sequence !== 1) throw new ArgumentError("workflow event sequence must start at one");
    rows.push(structuredClone(event));
    if (rows.length > AUTONOMOUS_WORKFLOW_MAX_EVENTS) rows.splice(0, rows.length - AUTONOMOUS_WORKFLOW_MAX_EVENTS);
    this.eventRows.set(event.job_id, rows);
  }

  events(jobId: string, after = 0, limit = AUTONOMOUS_WORKFLOW_MAX_EVENTS): AutonomousWorkflowEvent[] {
    if (!Number.isSafeInteger(after) || after < 0) throw new ArgumentError("workflow event after must be a non-negative integer");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > AUTONOMOUS_WORKFLOW_MAX_EVENTS) throw new ArgumentError("workflow event limit is outside its bounds");
    return (this.eventRows.get(jobId) ?? []).filter((event) => event.sequence > after).slice(0, limit).map((event) => structuredClone(event));
  }
}

function boundedJobId(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError("workflow jobId must be a bounded identifier");
  return value;
}

function boundedStageCount(value: unknown): number {
  if (value === undefined) return AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL;
  if (!Number.isSafeInteger(value) || (value as number) < 1 || (value as number) > AUTONOMOUS_WORKFLOW_MAX_STAGES_PER_CALL) throw new ArgumentError("workflow maxStages must be between 1 and 32");
  return value as number;
}

function responseText(run: AutonomousRunResult | null): string {
  if (!run?.response) return "";
  if (run.response.text) return run.response.text;
  return run.response.structured === null || run.response.structured === undefined ? "" : JSON.stringify(run.response.structured);
}

function runOptions(options: AutonomousWorkflowExecuteOptions, stage: AutonomousWorkflowStage, domain: AutonomousDomainName, context: AutonomousRunOptions["context"]): AutonomousRunOptions {
  return {
    domain,
    capability: stage.required_capabilities[0],
    candidates: options.candidates,
    credential: options.credential,
    credentialFor: options.credentialFor,
    context,
    hints: [],
    maxInputTokens: options.maxInputTokens,
    maxOutputTokens: options.maxOutputTokens,
    temperature: options.temperature,
    tools: options.tools,
    authorizeAndExecute: options.authorizeAndExecute,
    approveProviderCall: options.approveProviderCall,
    approveEffects: options.approveEffects,
    signal: options.signal,
    observer: options.observer,
  };
}

export class AutonomousWorkflowExecutor {
  readonly agent: AutonomousAgent;
  readonly store: AutonomousWorkflowCheckpointStore;

  constructor(agent: AutonomousAgent, store: AutonomousWorkflowCheckpointStore) {
    if (!agent || typeof agent.blueprint !== "function" || typeof agent.run !== "function") throw new ArgumentError("workflow executor requires an AutonomousAgent");
    if (!store || typeof store.load !== "function" || typeof store.save !== "function" || typeof store.appendEvent !== "function" || typeof store.events !== "function") throw new ArgumentError("workflow executor requires a checkpoint store");
    this.agent = agent;
    this.store = store;
  }

  async start(task: string, options: AutonomousWorkflowExecuteOptions = {}): Promise<AutonomousWorkflowExecutionResult> {
    const route = await this.agent.route(task, { domain: options.domain, hints: options.hints });
    if (route.abstained || !route.primary_domain || route.cross_domain) return this.routeReviewResult();
    const blueprintEnvelope = await this.agent.blueprint(task, { domain: route.primary_domain, capability: options.capability, context: options.context, hints: options.hints, maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name) });
    const blueprint = blueprintEnvelope.blueprint;
    if (!blueprint) return this.routeReviewResult();
    const jobId = boundedJobId(options.jobId ?? `workflow-${route.task_digest.slice(0, 24)}`);
    const existing = await this.store.load(jobId);
    if (existing) {
      if (existing.task_digest !== blueprint.task_digest || existing.workflow_digest !== blueprint.workflow.workflow_digest) throw new ArgumentError("workflow job already exists with a different task or workflow");
      return this.drive(task, blueprint, existing, options);
    }
    const initial = await this.makeCheckpoint(jobId, blueprint, [], [], "running", null);
    await this.store.save(initial);
    await this.appendEvent(jobId, "started", null, initial);
    return this.drive(task, blueprint, initial, options);
  }

  async resume(jobId: string, task: string, options: Omit<AutonomousWorkflowExecuteOptions, "jobId"> = {}): Promise<AutonomousWorkflowExecutionResult> {
    const normalizedJobId = boundedJobId(jobId);
    const checkpoint = await this.store.load(normalizedJobId);
    if (!checkpoint) throw new ArgumentError(`workflow job ${normalizedJobId} was not found; caller must rehydrate from its durable store`);
    const route = await this.agent.route(task, { domain: checkpoint.domain, hints: options.hints });
    if (route.abstained || !route.primary_domain || route.primary_domain !== checkpoint.domain) throw new ProviderRuntimeError("workflow rehydration route does not match the checkpoint domain");
    if (route.task_digest !== checkpoint.task_digest) throw new ProviderRuntimeError("workflow rehydration task digest does not match the checkpoint");
    const blueprintEnvelope = await this.agent.blueprint(task, { domain: checkpoint.domain, capability: options.capability, context: options.context, hints: options.hints, maxInputTokens: options.maxInputTokens, tools: options.tools?.map((tool) => tool.name) });
    const blueprint = blueprintEnvelope.blueprint;
    if (!blueprint || blueprint.workflow.workflow_digest !== checkpoint.workflow_digest || blueprint.plan.plan_digest !== checkpoint.plan_digest) throw new ProviderRuntimeError("workflow rehydration blueprint digest does not match the checkpoint");
    return this.drive(task, blueprint, checkpoint, { ...options, jobId: normalizedJobId });
  }

  async events(jobId: string, after = 0, limit = AUTONOMOUS_WORKFLOW_MAX_EVENTS): Promise<AutonomousWorkflowEvent[]> {
    return this.store.events(boundedJobId(jobId), after, limit);
  }

  private routeReviewResult(): AutonomousWorkflowExecutionResult {
    return { schema: AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA, status: "route_review_required", job_id: null, blueprint: null, checkpoint: null, events: [], stage_results: [], completed_stage_count: 0, total_stage_count: 0, recovery: "caller_rehydrates_task_and_credentials", retention: "provider_responses_local;checkpoint_metadata_only" };
  }

  private async makeCheckpoint(jobId: string, blueprint: AutonomousTaskBlueprint, completed: string[], outcomes: AutonomousWorkflowStageOutcome[], status: AutonomousWorkflowCheckpointStatus, previous: AutonomousWorkflowCheckpoint | null): Promise<AutonomousWorkflowCheckpoint> {
    const next = blueprint.workflow.stages.find((stage) => !completed.includes(stage.id))?.id ?? null;
    const descriptor = { schema: AUTONOMOUS_WORKFLOW_CHECKPOINT_SCHEMA, job_id: jobId, task_digest: blueprint.task_digest, domain: blueprint.domain_profile.domain, workflow_id: blueprint.workflow.workflow_id, workflow_digest: blueprint.workflow.workflow_digest, plan_digest: blueprint.plan.plan_digest, completed_stage_ids: completed, next_stage_id: next, stage_outcomes: outcomes, generation: (previous?.generation ?? 0) + 1, status, previous_checkpoint_digest: previous?.checkpoint_digest ?? null, retention: "metadata_only;task_prompt_response_and_credentials_not_retained" as const, secret_material: "never_returned" as const };
    return { ...descriptor, checkpoint_digest: await digestJson(descriptor) };
  }

  private async appendEvent(jobId: string, eventType: AutonomousWorkflowEventType, stageId: string | null, checkpoint: AutonomousWorkflowCheckpoint): Promise<AutonomousWorkflowEvent> {
    const prior = await this.store.events(jobId, 0, AUTONOMOUS_WORKFLOW_MAX_EVENTS);
    const previousEventDigest = prior.at(-1)?.event_digest ?? null;
    const descriptor = { schema: AUTONOMOUS_WORKFLOW_EVENT_SCHEMA, sequence: (prior.at(-1)?.sequence ?? 0) + 1, job_id: jobId, event_type: eventType, stage_id: stageId, checkpoint_digest: checkpoint.checkpoint_digest, previous_event_digest: previousEventDigest, retention: "metadata_only;provider_payloads_not_retained" as const, secret_material: "never_returned" as const };
    const event = { ...descriptor, event_digest: await digestJson(descriptor) };
    await this.store.appendEvent(event);
    return event;
  }

  private async drive(task: string, blueprint: AutonomousTaskBlueprint, initial: AutonomousWorkflowCheckpoint, options: AutonomousWorkflowExecuteOptions): Promise<AutonomousWorkflowExecutionResult> {
    const maxStages = boundedStageCount(options.maxStages);
    let checkpoint = initial;
    const stageResults: AutonomousWorkflowStageResult[] = [];
    const stages = blueprint.workflow.stages;
    let consumed = 0;
    if (checkpoint.status === "completed") return this.result("completed", checkpoint, blueprint, stageResults);
    if (options.approveProviderCall !== true) {
      checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes, "paused", checkpoint);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, "approval_required", checkpoint.next_stage_id, checkpoint);
      return this.result("approval_required", checkpoint, blueprint, stageResults);
    }
    while (checkpoint.next_stage_id && consumed < maxStages) {
      const stage = stages.find((candidate) => candidate.id === checkpoint.next_stage_id);
      if (!stage) throw new ProviderRuntimeError(`workflow checkpoint references unknown stage ${checkpoint.next_stage_id}`);
      if (stage.depends_on.some((dependency) => !checkpoint.completed_stage_ids.includes(dependency))) throw new ProviderRuntimeError(`workflow stage ${stage.id} has incomplete dependencies`);
      consumed += 1;
      const priorOutputs = stageResults.map((entry) => ({ stage_id: entry.stage.id, output_digest: entry.output_digest, output_bytes: entry.output_bytes }));
      const context = [
        ...(options.context ?? []),
        { id: "workflow-checkpoint", content: JSON.stringify({ job_id: checkpoint.job_id, workflow_digest: checkpoint.workflow_digest, completed_stage_ids: checkpoint.completed_stage_ids, stage_outcomes: checkpoint.stage_outcomes, prior_outputs: priorOutputs }), required: true, priority: 100 },
        { id: "workflow-stage-contract", content: JSON.stringify({ stage_id: stage.id, objective: stage.objective, required_capabilities: stage.required_capabilities, evidence_outputs: stage.evidence_outputs, evaluator_signals: stage.evaluator_signals }), required: true, priority: 90 },
      ];
      let run: AutonomousRunResult;
      try {
        run = await this.agent.run(`Execute workflow stage ${stage.id} for task: ${task}`, runOptions(options, stage, blueprint.domain_profile.domain, context));
      } catch (error) {
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "failed", run_status: "exception", selection_digest: null, response_digest: null, output_bytes: 0, error_class: error instanceof Error ? error.constructor.name : "UnknownError" }], "failed", checkpoint);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "stage_failed", stage.id, checkpoint);
        return this.result("failed", checkpoint, blueprint, stageResults);
      }
      const text = responseText(run);
      const outputDigest = text ? await digestJson({ stage_id: stage.id, output: text }) : null;
      const selectionDigest = run.selection ? await digestJson(run.selection) : null;
      const outputBytes = new TextEncoder().encode(text).byteLength;
      stageResults.push({ stage, run, output_digest: outputDigest, output_bytes: outputBytes });
      if (run.status === "approval_required") {
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "approval_required", run_status: run.status, selection_digest: selectionDigest, response_digest: null, output_bytes: 0, error_class: null }], "paused", checkpoint);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "approval_required", stage.id, checkpoint);
        return this.result("approval_required", checkpoint, blueprint, stageResults);
      }
      if (run.status !== "completed") {
        checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "failed", run_status: run.status, selection_digest: selectionDigest, response_digest: outputDigest, output_bytes: outputBytes, error_class: null }], "failed", checkpoint);
        await this.store.save(checkpoint);
        await this.appendEvent(checkpoint.job_id, "stage_failed", stage.id, checkpoint);
        return this.result("failed", checkpoint, blueprint, stageResults);
      }
      const completed = [...checkpoint.completed_stage_ids, stage.id];
      const outcomes = [...checkpoint.stage_outcomes, { stage_id: stage.id, status: "completed" as const, run_status: run.status, selection_digest: selectionDigest, response_digest: outputDigest, output_bytes: outputBytes, error_class: null }];
      const nextStatus: AutonomousWorkflowCheckpointStatus = completed.length === stages.length ? "completed" : "running";
      checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, completed, outcomes, nextStatus, checkpoint);
      await this.store.save(checkpoint);
      await this.appendEvent(checkpoint.job_id, nextStatus === "completed" ? "completed" : "stage_completed", stage.id, checkpoint);
    }
    if (checkpoint.status === "completed") return this.result("completed", checkpoint, blueprint, stageResults);
    checkpoint = await this.makeCheckpoint(checkpoint.job_id, blueprint, checkpoint.completed_stage_ids, checkpoint.stage_outcomes, "paused", checkpoint);
    await this.store.save(checkpoint);
    await this.appendEvent(checkpoint.job_id, "checkpointed", checkpoint.next_stage_id, checkpoint);
    return this.result("paused", checkpoint, blueprint, stageResults);
  }

  private async result(status: AutonomousWorkflowExecutionStatus, checkpoint: AutonomousWorkflowCheckpoint, blueprint: AutonomousTaskBlueprint, stageResults: AutonomousWorkflowStageResult[]): Promise<AutonomousWorkflowExecutionResult> {
    return { schema: AUTONOMOUS_WORKFLOW_EXECUTION_SCHEMA, status, job_id: checkpoint.job_id, blueprint, checkpoint, events: await this.store.events(checkpoint.job_id, 0, AUTONOMOUS_WORKFLOW_MAX_EVENTS), stage_results: stageResults, completed_stage_count: checkpoint.completed_stage_ids.length, total_stage_count: blueprint.workflow.stages.length, recovery: "caller_rehydrates_task_and_credentials", retention: "provider_responses_local;checkpoint_metadata_only" };
  }
}
