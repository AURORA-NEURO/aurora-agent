import { ArgumentError } from "./errors.js";
import { isAutonomousBrainFacade } from "./autonomous-brain-facade-brand.js";
import { validateBrainBatchCheckpoint, AUTONOMOUS_BRAIN_BATCH_CONTROLLER_SCHEMA } from "./autonomous-brain-batch-support.js";
import {
  AutonomousBrainBatchProtectedRehydrator,
  AutonomousBrainAutoBatchProtectedRehydrator,
  AutonomousBrainAutoCycleBatchProtectedRehydrator,
  AutonomousBrainAutoReplanBatchProtectedRehydrator,
} from "./autonomous-brain-batch-rehydration.js";
import type {
  AutonomousBrainFacade,
  AutonomousBrainRequest,
  AutonomousBrainBatchCheckpointJSON,
  AutonomousBrainBatchCheckpointStore,
  AutonomousBrainBatchControllerProjection,
  AutonomousBrainBatchControllerRun,
  AutonomousBrainBatchControllerRunOptions,
  AutonomousBrainBatchControllerStatus,
  AutonomousBrainAutoBatchControllerRun,
  AutonomousBrainAutoBatchControllerRunOptions,
  AutonomousBrainAutoBatchResumableOptions,
  AutonomousBrainAutoCycleBatchControllerRun,
  AutonomousBrainAutoCycleBatchControllerRunOptions,
  AutonomousBrainAutoCycleBatchControllerTraceRun,
  AutonomousBrainAutoCycleBatchControllerTraceRunOptions,
  AutonomousBrainAutoCycleBatchResumableOptions,
  AutonomousBrainAutoCycleBatchResumableTraceOptions,
  AutonomousBrainAutoReplanBatchControllerRun,
  AutonomousBrainAutoReplanBatchControllerRunOptions,
  AutonomousBrainAutoReplanBatchControllerTraceRun,
  AutonomousBrainAutoReplanBatchControllerTraceRunOptions,
  AutonomousBrainAutoReplanBatchResumableOptions,
  AutonomousBrainAutoReplanBatchResumableTraceOptions,
  AutonomousBrainResumableBatchOptions,
} from "./autonomous-brain-facade.js";

export interface AutonomousBrainBatchJobControllerOptions {
  protectedRehydration?: AutonomousBrainBatchProtectedRehydrator;
  automaticProtectedRehydration?: AutonomousBrainAutoBatchProtectedRehydrator;
  automaticCycleProtectedRehydration?: AutonomousBrainAutoCycleBatchProtectedRehydrator;
  automaticReplanProtectedRehydration?: AutonomousBrainAutoReplanBatchProtectedRehydrator;
}

/**
 * Own the process lifecycle around the verified resumable brain batch engine.
 *
 * The facade deliberately accepts a checkpoint sink so infrastructure can choose a database,
 * object store, or journal. This controller is the safer application boundary: startup restore
 * is explicit, only one run may mutate a checkpoint at a time, every checkpoint is validated
 * before it reaches the store, and task text, prompts, provider values, connector observations,
 * and credentials remain transient by construction.
 */
export class AutonomousBrainBatchJobController {
  private checkpoint: AutonomousBrainBatchCheckpointJSON | null = null;
  private restored = false;
  private running = false;

  constructor(
    readonly brain: AutonomousBrainFacade,
    readonly persistence: AutonomousBrainBatchCheckpointStore,
    readonly options: AutonomousBrainBatchJobControllerOptions = {},
  ) {
    if (!isAutonomousBrainFacade(brain)) throw new ArgumentError("autonomous brain batch controller requires an AutonomousBrainFacade");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous brain batch checkpoint store is malformed");
    if (options.protectedRehydration !== undefined && !(options.protectedRehydration instanceof AutonomousBrainBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller protectedRehydration is malformed");
    if (options.automaticProtectedRehydration !== undefined && !(options.automaticProtectedRehydration instanceof AutonomousBrainAutoBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller automaticProtectedRehydration is malformed");
    if (options.automaticCycleProtectedRehydration !== undefined && !(options.automaticCycleProtectedRehydration instanceof AutonomousBrainAutoCycleBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller automaticCycleProtectedRehydration is malformed");
    if (options.automaticReplanProtectedRehydration !== undefined && !(options.automaticReplanProtectedRehydration instanceof AutonomousBrainAutoReplanBatchProtectedRehydrator)) throw new ArgumentError("autonomous brain batch controller automaticReplanProtectedRehydration is malformed");
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("autonomous brain batch controller must restore before execution");
  }

  private requireIdle(): void {
    if (this.running) throw new ArgumentError("autonomous brain batch controller already has a run in progress");
  }

  private projection(status: AutonomousBrainBatchControllerStatus, totalItems: number | null = null, jobId: string | null = this.checkpoint?.job_id ?? null): AutonomousBrainBatchControllerProjection {
    return {
      schema: AUTONOMOUS_BRAIN_BATCH_CONTROLLER_SCHEMA,
      status,
      job_id: jobId,
      checkpoint_digest: this.checkpoint?.checkpoint_digest ?? null,
      completed_items: this.checkpoint?.completed_indices.length ?? 0,
      total_items: totalItems ?? (this.checkpoint?.request_digests.length ?? null),
      persisted: true,
      retention: "metadata_only_request_and_result_digests;task_prompt_provider_connector_values_never_persisted",
      secret_material: "never_returned",
    };
  }

  /** Restore and verify the last checkpoint before accepting any execution request. */
  async restore(): Promise<AutonomousBrainBatchControllerProjection> {
    this.requireIdle();
    const raw = await this.persistence.read();
    this.checkpoint = raw === null ? null : validateBrainBatchCheckpoint(raw);
    this.restored = true;
    return this.projection(this.checkpoint === null ? "empty" : "restored");
  }

  /** Re-write the last verified checkpoint through the caller-owned store. */
  async flush(): Promise<AutonomousBrainBatchControllerProjection> {
    this.requireRestored();
    this.requireIdle();
    if (this.checkpoint === null) return this.projection("empty");
    const verified = validateBrainBatchCheckpoint(this.checkpoint);
    await this.persistence.write(verified);
    this.checkpoint = verified;
    return this.projection("flushed");
  }

  /** Run a routed/domain/cross-domain batch while the controller owns persistence and restart state. */
  async run(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainBatchControllerRunOptions): Promise<AutonomousBrainBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain batch controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainResumableBatchOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain batch controller owns checkpoint and checkpointSink");
    const rehydrateExecution = options.rehydrateExecution ?? (this.options.protectedRehydration === undefined ? undefined : this.options.protectedRehydration.resolve.bind(this.options.protectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeBatchResumable(inputs, {
        ...options,
        ...(rehydrateExecution === undefined ? {} : { rehydrateExecution }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run a restart-safe automatic batch while sharing the controller's verified checkpoint. */
  async runAutomatic(inputs: readonly AutonomousBrainRequest[], options: AutonomousBrainAutoBatchControllerRunOptions): Promise<AutonomousBrainAutoBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic batch controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoBatchResumableOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic batch controller owns checkpoint and checkpointSink");
    const rehydrateExecution = options.rehydrateExecution ?? (this.options.automaticProtectedRehydration === undefined ? undefined : this.options.automaticProtectedRehydration.resolve.bind(this.options.automaticProtectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeAutoBatchResumable(inputs, {
        ...options,
        ...(rehydrateExecution === undefined ? {} : { rehydrateExecution }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run restart-safe automatic evaluator cycles with controller-owned checkpoint persistence. */
  async runAutomaticCycle(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchControllerRunOptions,
  ): Promise<AutonomousBrainAutoCycleBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic cycle controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoCycleBatchResumableOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic cycle controller owns checkpoint and checkpointSink");
    const rehydrateCycle = options.rehydrateCycle ?? (this.options.automaticCycleProtectedRehydration === undefined ? undefined : this.options.automaticCycleProtectedRehydration.resolve.bind(this.options.automaticCycleProtectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeAutoCycleBatchResumable(inputs, {
        ...options,
        ...(rehydrateCycle === undefined ? {} : { rehydrateCycle }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run restart-safe automatic evaluator/replan cycles with controller-owned persistence. */
  async runAutomaticReplan(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchControllerRunOptions,
  ): Promise<AutonomousBrainAutoReplanBatchControllerRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic replan controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoReplanBatchResumableOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic replan controller owns checkpoint and checkpointSink");
    const rehydrateReplan = options.rehydrateReplan ?? (this.options.automaticReplanProtectedRehydration === undefined ? undefined : this.options.automaticReplanProtectedRehydration.resolve.bind(this.options.automaticReplanProtectedRehydration));
    this.running = true;
    try {
      const batch = await this.brain.executeAutoReplanCycleBatchResumable(inputs, {
        ...options,
        ...(rehydrateReplan === undefined ? {} : { rehydrateReplan }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(batch.status, inputs.length, options.jobId), batch };
    } finally {
      this.running = false;
    }
  }

  /** Run automatic evaluator cycles with controller-owned checkpoints and one redacted trace. */
  async runAutomaticCycleWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoCycleBatchControllerTraceRunOptions,
  ): Promise<AutonomousBrainAutoCycleBatchControllerTraceRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic cycle traced controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoCycleBatchResumableTraceOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic cycle traced controller owns checkpoint and checkpointSink");
    const rehydrateCycle = options.rehydrateCycle ?? (this.options.automaticCycleProtectedRehydration === undefined ? undefined : this.options.automaticCycleProtectedRehydration.resolve.bind(this.options.automaticCycleProtectedRehydration));
    this.running = true;
    try {
      const traced = await this.brain.executeAutoCycleBatchResumableWithTrace(inputs, {
        ...options,
        ...(rehydrateCycle === undefined ? {} : { rehydrateCycle }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(traced.batch.status, inputs.length, options.jobId), traced };
    } finally {
      this.running = false;
    }
  }

  /** Run automatic evaluator/replan cycles with controller-owned checkpoints and one trace. */
  async runAutomaticReplanWithTrace(
    inputs: readonly AutonomousBrainRequest[],
    options: AutonomousBrainAutoReplanBatchControllerTraceRunOptions,
  ): Promise<AutonomousBrainAutoReplanBatchControllerTraceRun> {
    this.requireRestored();
    this.requireIdle();
    if (!options || typeof options !== "object" || typeof options.jobId !== "string") throw new ArgumentError("autonomous brain automatic replan traced controller run requires jobId");
    const runtimeOptions = options as AutonomousBrainAutoReplanBatchResumableTraceOptions & Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpoint") || Object.prototype.hasOwnProperty.call(runtimeOptions, "checkpointSink")) throw new ArgumentError("autonomous brain automatic replan traced controller owns checkpoint and checkpointSink");
    const rehydrateReplan = options.rehydrateReplan ?? (this.options.automaticReplanProtectedRehydration === undefined ? undefined : this.options.automaticReplanProtectedRehydration.resolve.bind(this.options.automaticReplanProtectedRehydration));
    this.running = true;
    try {
      const traced = await this.brain.executeAutoReplanCycleBatchResumableWithTrace(inputs, {
        ...options,
        ...(rehydrateReplan === undefined ? {} : { rehydrateReplan }),
        checkpoint: this.checkpoint ?? undefined,
        checkpointSink: async (checkpoint) => {
          const verified = validateBrainBatchCheckpoint(checkpoint);
          await this.persistence.write(verified);
          this.checkpoint = verified;
        },
      });
      return { controller: this.projection(traced.batch.status, inputs.length, options.jobId), traced };
    } finally {
      this.running = false;
    }
  }
}

/** A small verified store useful for local processes, tests, and wiring examples. */
export class InMemoryAutonomousBrainBatchCheckpointStore implements AutonomousBrainBatchCheckpointStore {
  private checkpoint: AutonomousBrainBatchCheckpointJSON | null = null;

  constructor(initial?: AutonomousBrainBatchCheckpointJSON | null) {
    if (initial !== undefined && initial !== null) this.checkpoint = validateBrainBatchCheckpoint(initial);
  }

  read(): AutonomousBrainBatchCheckpointJSON | null {
    return this.checkpoint === null ? null : structuredClone(this.checkpoint);
  }

  write(checkpoint: AutonomousBrainBatchCheckpointJSON): void {
    this.checkpoint = structuredClone(validateBrainBatchCheckpoint(checkpoint));
  }
}
