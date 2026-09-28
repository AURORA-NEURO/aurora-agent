import { ArgumentError, isObject } from "./errors.js";
import { AutonomousProtectedRehydrationAdapter } from "./autonomous-protected-rehydration.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type {
  AutonomousBrainAutoCycleResult,
  AutonomousBrainAutoExecution,
  AutonomousBrainAutoReplanCycleResult,
  AutonomousBrainExecution,
} from "./autonomous-brain-facade.js";
import type { AutonomousBrainBatchMode } from "./autonomous-brain-batch-support.js";

export interface AutonomousBrainBatchRehydrationContext {
  job_id: string;
  index: number;
  mode: AutonomousBrainBatchMode;
  request_digest: string;
  task_digest: string;
  expected_result_digest: string;
}

/**
 * Adapt the protected receipt boundary to restart-safe brain batches.
 *
 * Checkpoints contain only digests. The receipt resolver sees the same bounded identity fields
 * that the batch engine verifies, while the adapter owns tenant/authorization/replay fencing.
 * A decoder is available for callers whose protected store returns a canonical JSON projection
 * that must be rebuilt into a richer in-memory execution object.
 */
export class AutonomousBrainBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainExecution | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
    valueDecoder?: (value: unknown) => AutonomousBrainExecution | unknown;
    domain?: AutonomousDomainName;
    purpose?: string;
    valueKind?: string;
    oneTime?: boolean;
    digestScheme?: string;
  }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain batch protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain batch protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain batch protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain batch protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainExecution> {
    if (!context || context.mode !== "brain") throw new ArgumentError("autonomous brain batch protected rehydrator requires a direct brain checkpoint context");
    let receipt: unknown;
    try {
      receipt = await this.receiptResolver(context);
    } catch (error) {
      throw new ArgumentError(`autonomous brain batch protected receipt lookup failed for item ${context.index}`, { cause: error });
    }
    if (!isObject(receipt)) throw new ArgumentError("autonomous brain batch protected receiptResolver must return an object");
    for (const [key, expected] of [
      ["job_id", context.job_id],
      ["index", context.index],
      ["mode", context.mode],
      ["request_digest", context.request_digest],
      ["task_digest", context.task_digest],
      ["expected_result_digest", context.expected_result_digest],
    ] as const) {
      if (receipt[key] !== expected) throw new ArgumentError(`autonomous brain batch protected receipt ${key} does not match item ${context.index}`);
    }
    try {
      const value = this.adapter.resolveReceipt(receipt, {
        domain: this.domain,
        purpose: this.purpose,
        valueKind: this.valueKind,
        oneTime: this.oneTime,
        digestScheme: this.digestScheme,
      });
      const decoded = this.valueDecoder === undefined ? value : await this.valueDecoder(value);
      if (!isObject(decoded)) throw new ArgumentError(`autonomous brain batch protected result for item ${context.index} is not an execution object`);
      return decoded as unknown as AutonomousBrainExecution;
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError(`autonomous brain batch protected result resolution failed for item ${context.index}`, { cause: error });
    }
  }
}

/** Protected-receipt adapter for automatic batches; it never widens a direct checkpoint into automatic execution. */
export class AutonomousBrainAutoBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainAutoExecution | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
    valueDecoder?: (value: unknown) => AutonomousBrainAutoExecution | unknown;
    domain?: AutonomousDomainName;
    purpose?: string;
    valueKind?: string;
    oneTime?: boolean;
    digestScheme?: string;
  }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain automatic batch protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain automatic batch protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain automatic batch protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain automatic batch protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_automatic_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_automatic_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainAutoExecution> {
    if (!context || context.mode !== "automatic") throw new ArgumentError("autonomous brain automatic batch protected rehydrator requires an automatic checkpoint context");
    let receipt: unknown;
    try {
      receipt = await this.receiptResolver(context);
    } catch (error) {
      throw new ArgumentError(`autonomous brain automatic batch protected receipt lookup failed for item ${context.index}`, { cause: error });
    }
    if (!isObject(receipt)) throw new ArgumentError("autonomous brain automatic batch protected receiptResolver must return an object");
    for (const [key, expected] of [
      ["job_id", context.job_id],
      ["index", context.index],
      ["mode", context.mode],
      ["request_digest", context.request_digest],
      ["task_digest", context.task_digest],
      ["expected_result_digest", context.expected_result_digest],
    ] as const) {
      if (receipt[key] !== expected) throw new ArgumentError(`autonomous brain automatic batch protected receipt ${key} does not match item ${context.index}`);
    }
    try {
      const value = this.adapter.resolveReceipt(receipt, {
        domain: this.domain,
        purpose: this.purpose,
        valueKind: this.valueKind,
        oneTime: this.oneTime,
        digestScheme: this.digestScheme,
      });
      const decoded = this.valueDecoder === undefined ? value : await this.valueDecoder(value);
      if (!isObject(decoded)) throw new ArgumentError(`autonomous brain automatic batch protected result for item ${context.index} is not an execution object`);
      return decoded as unknown as AutonomousBrainAutoExecution;
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError(`autonomous brain automatic batch protected result resolution failed for item ${context.index}`, { cause: error });
    }
  }
}

async function resolveAutomaticCycleBatchProtectedValue(
  label: string,
  expectedMode: "automatic_cycle" | "automatic_replan",
  options: {
    adapter: AutonomousProtectedRehydrationAdapter;
    receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
    valueDecoder?: (value: unknown) => unknown;
    domain?: AutonomousDomainName;
    purpose: string;
    valueKind: string;
    oneTime: boolean;
    digestScheme: string;
  },
  context: AutonomousBrainBatchRehydrationContext,
): Promise<unknown> {
  if (!context || context.mode !== expectedMode) throw new ArgumentError(`autonomous brain ${label} protected rehydrator requires a ${expectedMode} checkpoint context`);
  let receipt: unknown;
  try {
    receipt = await options.receiptResolver(context);
  } catch (error) {
    throw new ArgumentError(`autonomous brain ${label} protected receipt lookup failed for item ${context.index}`, { cause: error });
  }
  if (!isObject(receipt)) throw new ArgumentError(`autonomous brain ${label} protected receiptResolver must return an object`);
  for (const [key, expected] of [
    ["job_id", context.job_id],
    ["index", context.index],
    ["mode", context.mode],
    ["request_digest", context.request_digest],
    ["task_digest", context.task_digest],
    ["expected_result_digest", context.expected_result_digest],
  ] as const) {
    if (receipt[key] !== expected) throw new ArgumentError(`autonomous brain ${label} protected receipt ${key} does not match item ${context.index}`);
  }
  try {
    const value = options.adapter.resolveReceipt(receipt, { domain: options.domain, purpose: options.purpose, valueKind: options.valueKind, oneTime: options.oneTime, digestScheme: options.digestScheme });
    const decoded = options.valueDecoder === undefined ? value : await options.valueDecoder(value);
    if (!isObject(decoded)) throw new ArgumentError(`autonomous brain ${label} protected result for item ${context.index} is not an execution object`);
    return decoded;
  } catch (error) {
    if (error instanceof ArgumentError) throw error;
    throw new ArgumentError(`autonomous brain ${label} protected result resolution failed for item ${context.index}`, { cause: error });
  }
}

/** Protected-receipt adapter for automatic evaluator-cycle batch results. */
export class AutonomousBrainAutoCycleBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainAutoCycleResult | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: { adapter: AutonomousProtectedRehydrationAdapter; receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>; valueDecoder?: (value: unknown) => AutonomousBrainAutoCycleResult | unknown; domain?: AutonomousDomainName; purpose?: string; valueKind?: string; oneTime?: boolean; digestScheme?: string }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain automatic cycle protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain automatic cycle protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain automatic cycle protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain automatic cycle protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_automatic_cycle_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_automatic_cycle_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainAutoCycleResult> {
    return await resolveAutomaticCycleBatchProtectedValue("automatic cycle", "automatic_cycle", this, context) as AutonomousBrainAutoCycleResult;
  }
}

/** Protected-receipt adapter for automatic evaluator/replan batch results. */
export class AutonomousBrainAutoReplanBatchProtectedRehydrator {
  readonly adapter: AutonomousProtectedRehydrationAdapter;
  readonly receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>;
  readonly valueDecoder?: (value: unknown) => AutonomousBrainAutoReplanCycleResult | unknown;
  readonly domain?: AutonomousDomainName;
  readonly purpose: string;
  readonly valueKind: string;
  readonly oneTime: boolean;
  readonly digestScheme: string;

  constructor(options: { adapter: AutonomousProtectedRehydrationAdapter; receiptResolver: (context: AutonomousBrainBatchRehydrationContext) => unknown | Promise<unknown>; valueDecoder?: (value: unknown) => AutonomousBrainAutoReplanCycleResult | unknown; domain?: AutonomousDomainName; purpose?: string; valueKind?: string; oneTime?: boolean; digestScheme?: string }) {
    if (!(options?.adapter instanceof AutonomousProtectedRehydrationAdapter)) throw new ArgumentError("autonomous brain automatic replan protected rehydrator requires a protected rehydration adapter");
    if (typeof options.receiptResolver !== "function") throw new ArgumentError("autonomous brain automatic replan protected rehydrator receiptResolver must be callable");
    if (options.valueDecoder !== undefined && typeof options.valueDecoder !== "function") throw new ArgumentError("autonomous brain automatic replan protected rehydrator valueDecoder must be callable");
    if (options.oneTime !== undefined && typeof options.oneTime !== "boolean") throw new ArgumentError("autonomous brain automatic replan protected rehydrator oneTime must be boolean");
    this.adapter = options.adapter;
    this.receiptResolver = options.receiptResolver;
    this.valueDecoder = options.valueDecoder;
    this.domain = options.domain;
    this.purpose = options.purpose ?? "autonomous_automatic_replan_batch_result";
    this.valueKind = options.valueKind ?? "autonomous_automatic_replan_batch_result";
    this.oneTime = options.oneTime ?? false;
    this.digestScheme = options.digestScheme ?? "canonical_json";
  }

  async resolve(context: AutonomousBrainBatchRehydrationContext): Promise<AutonomousBrainAutoReplanCycleResult> {
    return await resolveAutomaticCycleBatchProtectedValue("automatic replan", "automatic_replan", this, context) as AutonomousBrainAutoReplanCycleResult;
  }
}
