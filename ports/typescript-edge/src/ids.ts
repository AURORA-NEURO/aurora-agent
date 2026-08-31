/**
 * Typed nonzero identifiers for agents, tasks and shards.
 *
 * Every identifier is a distinct branded number on purpose: an `AgentId` must never be
 * assignable to a `TaskId` slot by accident, so there is no structural overlap between the
 * brands and the raw integer is never exposed as a plain number. Zero is never issued — a
 * decoded id of zero from a transport frame is a protocol error, not a handle — so the zero
 * value cannot sneak through a constructor.
 *
 * Scope bound: ids are safe integers (2^53 - 1). The Rust fabric uses u64 counters; an edge
 * runtime that needs more than nine quadrillion logical agents has outgrown this slice, and
 * widening silently would let precision loss look like id reuse.
 */

declare const agentBrand: unique symbol;
declare const taskBrand: unique symbol;
declare const shardBrand: unique symbol;

/** A logical agent registered with a control plane. None implies a thread or OS process. */
export type AgentId = number & { readonly [agentBrand]: "AgentId" };
/** One unit of work admitted to a control plane. */
export type TaskId = number & { readonly [taskBrand]: "TaskId" };
/** One logical placement partition — an in-process queue lane, never a host. */
export type ShardId = number & { readonly [shardBrand]: "ShardId" };

/**
 * Reports a raw value of 0, which no constructor issues. Distinct from range errors because
 * a decoded zero is a protocol-level finding, not a caller arithmetic mistake.
 */
export class ZeroIdError extends RangeError {
  constructor(kind: string) {
    super(`${kind} raw value 0 is never issued`);
    this.name = "ZeroIdError";
  }
}

/** Reports a raw value outside the safe-integer bound this port enforces. */
export class IdRangeError extends RangeError {
  constructor(kind: string, raw: number) {
    super(
      `${kind} raw value ${raw} must be a positive safe integer (1 .. 2^53-1)`,
    );
    this.name = "IdRangeError";
  }
}

function issue<T extends number>(kind: string, brand: (raw: number) => T, raw: number): T {
  if (raw === 0) throw new ZeroIdError(kind);
  if (!Number.isSafeInteger(raw) || raw < 0) throw new IdRangeError(kind, raw);
  return brand(raw);
}

function decode<T extends number>(kind: string, brand: (raw: number) => T, raw: number): T | null {
  if (!Number.isSafeInteger(raw) || raw <= 0) return null;
  return brand(raw);
}

function newAgent(raw: number): AgentId {
  return issue("agent id", (n) => n as AgentId, raw);
}
function newTask(raw: number): TaskId {
  return issue("task id", (n) => n as TaskId, raw);
}
function newShard(raw: number): ShardId {
  return issue("shard id", (n) => n as ShardId, raw);
}

/**
 * Constructs an id from a counter value; throws {@link ZeroIdError} on 0 and
 * {@link IdRangeError} outside the safe-integer bound.
 */
export const newAgentId = newAgent;
export const newTaskId = newTask;
export const newShardId = newShard;

/**
 * Decodes a raw transport value into an id, or null when it is zero / unsafe. This mirrors
 * the Rust `from_raw -> Option` path; callers that need the reason use the constructors,
 * which throw typed errors rather than collapsing both failure kinds into one null.
 */
export const decodeAgentId = (raw: number): AgentId | null => decode("agent id", (n) => n as AgentId, raw);
export const decodeTaskId = (raw: number): TaskId | null => decode("task id", (n) => n as TaskId, raw);
export const decodeShardId = (raw: number): ShardId | null => decode("shard id", (n) => n as ShardId, raw);

/** Exposes the counter value for placement keys and telemetry. */
export const agentIdRaw = (id: AgentId): number => id;
export const taskIdRaw = (id: TaskId): number => id;
export const shardIdRaw = (id: ShardId): number => id;

/** Display names the kind, so `"3"` from two different id types never looks equal in logs. */
export const formatAgentId = (id: AgentId): string => `agent-${id}`;
export const formatTaskId = (id: TaskId): string => `task-${id}`;
export const formatShardId = (id: ShardId): string => `shard-${id}`;
