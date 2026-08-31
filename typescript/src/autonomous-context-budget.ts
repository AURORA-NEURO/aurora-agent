import { ProviderRuntimeError } from "./errors.js";
import { digestJson } from "./tooling.js";
import type { ProviderMessage, ProviderRequest } from "./llm.js";

/** Stable schema for the transient context-window admission contract. */
export const AUTONOMOUS_CONTEXT_BUDGET_SCHEMA = "bioprism-autonomous-context-budget/0.1" as const;

/** Hard bounds shared by all callers, independent of a provider's advertised capacity. */
export const MAX_AUTONOMOUS_CONTEXT_INPUT_TOKENS = 1_000_000;
export const MAX_AUTONOMOUS_CONTEXT_MESSAGES = 1_024;
export const MAX_AUTONOMOUS_CONTEXT_RECENT_MESSAGES = 128;

/**
 * Lossy context handling is deliberately explicit. The SDK never asks a model to summarize its
 * own history while trying to recover from a context overflow: that would require another model
 * call, could lose safety instructions, and would make retries non-deterministic. Instead, old
 * conversation units are removed atomically while system/developer instructions and the recent
 * tail remain protected.
 */
export interface AutonomousContextBudgetOptions {
  /** Maximum estimated input tokens allowed after compaction. */
  maxInputTokens: number;
  /** Number of trailing messages protected in addition to the latest user message. */
  preserveRecentMessages?: number;
  /** Maximum number of messages after compaction. */
  maxMessages?: number;
}

export interface AutonomousContextBudgetPlan {
  schema: typeof AUTONOMOUS_CONTEXT_BUDGET_SCHEMA;
  status: "unchanged" | "compacted";
  strategy: "drop_oldest_atomic_turns";
  original_input_tokens: number;
  final_input_tokens: number;
  max_input_tokens: number;
  messages_before: number;
  messages_after: number;
  dropped_message_count: number;
  dropped_message_indexes: number[];
  protected_message_count: number;
  protected_instruction_count: number;
  tool_turns_dropped: number;
  message_shape_digest: string;
  plan_digest: string;
  retention: "transient_request_compaction_metadata_only";
  content_retention: "provider_content_not_retained_in_plan";
}

export interface AutonomousContextBudgetResult {
  request: ProviderRequest;
  plan: AutonomousContextBudgetPlan;
}

function utf8Bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function jsonBytes(value: unknown): number {
  const encoded = JSON.stringify(value);
  if (encoded === undefined) throw new ProviderRuntimeError("context budget input is not JSON-safe", { code: "invalid_request" });
  return utf8Bytes(encoded);
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new ProviderRuntimeError(`${name} must be an integer within [${minimum}, ${maximum}]`, { code: "invalid_request" });
  }
  return value;
}

export function normalizeAutonomousContextBudget(options: AutonomousContextBudgetOptions): Required<AutonomousContextBudgetOptions> {
  if (!options || typeof options !== "object") throw new ProviderRuntimeError("context budget options must be an object", { code: "invalid_request" });
  return {
    maxInputTokens: boundedInteger("context budget maxInputTokens", options.maxInputTokens, 1, MAX_AUTONOMOUS_CONTEXT_INPUT_TOKENS),
    preserveRecentMessages: boundedInteger("context budget preserveRecentMessages", options.preserveRecentMessages ?? 8, 0, MAX_AUTONOMOUS_CONTEXT_RECENT_MESSAGES),
    maxMessages: boundedInteger("context budget maxMessages", options.maxMessages ?? MAX_AUTONOMOUS_CONTEXT_MESSAGES, 1, MAX_AUTONOMOUS_CONTEXT_MESSAGES),
  };
}

function messageTokenEstimate(message: ProviderMessage): number {
  const metadata = {
    role: message.role,
    name: message.name ?? null,
    toolCallId: message.toolCallId ?? null,
    toolCalls: message.toolCalls ?? null,
    isError: message.isError ?? null,
  };
  // Four bytes/token is intentionally conservative and matches the provider runtime's existing
  // admission estimate. A fixed envelope accounts for role/turn framing at the wire boundary.
  return Math.max(1, Math.ceil((utf8Bytes(message.role) + 32 + (typeof message.content === "string" ? utf8Bytes(message.content) : jsonBytes(message.content)) + jsonBytes(metadata)) / 4));
}

function messageShape(message: ProviderMessage, tokens: number, protectedMessage: boolean): Record<string, unknown> {
  return {
    role: message.role,
    content_kind: typeof message.content === "string" ? "text" : "parts",
    content_bytes: typeof message.content === "string" ? utf8Bytes(message.content) : jsonBytes(message.content),
    tool_call_count: message.toolCalls?.length ?? 0,
    has_tool_call_id: message.toolCallId !== undefined,
    tokens,
    protected: protectedMessage,
  };
}

interface ContextUnit {
  indexes: number[];
  tokens: number;
  protected: boolean;
  toolTurn: boolean;
}

function unitsFor(messages: readonly ProviderMessage[], preserveRecentMessages: number): { units: ContextUnit[]; tokens: number[]; protectedIndexes: Set<number> } {
  const tokens = messages.map(messageTokenEstimate);
  const latestUser = messages.reduce((found, message, index) => message.role === "user" ? index : found, -1);
  const protectedIndexes = new Set<number>();
  messages.forEach((message, index) => {
    if (message.role === "system" || message.role === "developer" || index >= Math.max(0, messages.length - preserveRecentMessages) || index === latestUser) protectedIndexes.add(index);
  });
  const units: ContextUnit[] = [];
  let index = 0;
  while (index < messages.length) {
    const message = messages[index]!;
    const callIds = new Set((message.toolCalls ?? []).map((call) => call.id));
    const indexes = [index];
    let end = index + 1;
    if (message.role === "assistant" && callIds.size > 0) {
      while (end < messages.length && messages[end]!.role === "tool" && callIds.has(messages[end]!.toolCallId ?? "")) {
        indexes.push(end);
        end += 1;
      }
    } else if (message.role === "tool") {
      // A malformed/orphan tool message is still removed as one unit; the normal provider
      // validator remains responsible for rejecting it when it is not compacted away.
      end = index + 1;
    }
    units.push({
      indexes,
      tokens: indexes.reduce((sum, current) => sum + tokens[current]!, 0),
      protected: indexes.some((current) => protectedIndexes.has(current)),
      toolTurn: indexes.length > 1 || message.role === "tool" || callIds.size > 0,
    });
    index = end;
  }
  // A freshly appended tool continuation is semantically required to interpret the next model
  // turn. Keep the newest tool unit protected even when the caller intentionally sets a zero
  // recent-message tail; older tool units remain removable, but always as atomic units.
  for (let unitIndex = units.length - 1; unitIndex >= 0; unitIndex -= 1) {
    const unit = units[unitIndex]!;
    if (!unit.toolTurn || unit.indexes[unit.indexes.length - 1] !== messages.length - 1) continue;
    unit.protected = true;
    for (const messageIndex of unit.indexes) protectedIndexes.add(messageIndex);
    break;
  }
  return { units, tokens, protectedIndexes };
}

/**
 * Compact one provider-neutral request without changing authorization, tool definitions, or
 * caller-owned content. The returned plan contains only shapes, counts, indexes, and digests.
 */
export async function compactAutonomousProviderRequest(
  request: ProviderRequest,
  options: AutonomousContextBudgetOptions,
): Promise<AutonomousContextBudgetResult> {
  if (!request || typeof request !== "object" || !Array.isArray(request.messages) || request.messages.length === 0) {
    throw new ProviderRuntimeError("context budget requires a non-empty provider request", { code: "invalid_request" });
  }
  const normalized = normalizeAutonomousContextBudget(options);
  if (request.messages.length > MAX_AUTONOMOUS_CONTEXT_MESSAGES) throw new ProviderRuntimeError("provider request exceeds the context budget message ceiling", { code: "invalid_request" });
  const { units, tokens, protectedIndexes } = unitsFor(request.messages, normalized.preserveRecentMessages);
  const originalInputTokens = Math.max(1, tokens.reduce((sum, value) => sum + value, 0));
  const kept = new Set<number>(request.messages.map((_message, index) => index));
  let finalInputTokens = originalInputTokens;
  const removable = units.filter((unit) => !unit.protected);
  const dropped: number[] = [];
  let droppedToolTurns = 0;
  const needsCompaction = (): boolean => finalInputTokens > normalized.maxInputTokens || kept.size > normalized.maxMessages;
  for (const unit of removable) {
    if (!needsCompaction()) break;
    for (const index of unit.indexes) {
      if (kept.delete(index)) dropped.push(index);
    }
    finalInputTokens -= unit.tokens;
    if (unit.toolTurn) droppedToolTurns += 1;
  }
  dropped.sort((left, right) => left - right);
  if (needsCompaction()) {
    throw new ProviderRuntimeError(
      `context budget cannot fit protected provider messages (${finalInputTokens} estimated tokens, ${kept.size} messages)`,
      { code: "invalid_request" },
    );
  }
  const finalMessages = request.messages.filter((_message, index) => kept.has(index));
  const protectedMessageCount = [...protectedIndexes].filter((index) => kept.has(index)).length;
  const shape = request.messages.map((message, index) => messageShape(message, tokens[index]!, protectedIndexes.has(index)));
  const messageShapeDigest = await digestJson(shape);
  const planBody = {
    schema: AUTONOMOUS_CONTEXT_BUDGET_SCHEMA,
    status: dropped.length > 0 ? "compacted" : "unchanged",
    strategy: "drop_oldest_atomic_turns",
    original_input_tokens: originalInputTokens,
    final_input_tokens: finalInputTokens,
    max_input_tokens: normalized.maxInputTokens,
    messages_before: request.messages.length,
    messages_after: finalMessages.length,
    dropped_message_count: dropped.length,
    dropped_message_indexes: dropped,
    protected_message_count: protectedMessageCount,
    protected_instruction_count: request.messages.filter((message) => message.role === "system" || message.role === "developer").length,
    tool_turns_dropped: droppedToolTurns,
    message_shape_digest: messageShapeDigest,
    retention: "transient_request_compaction_metadata_only",
    content_retention: "provider_content_not_retained_in_plan",
  } as const;
  const plan: AutonomousContextBudgetPlan = { ...planBody, plan_digest: await digestJson(planBody) };
  return { request: dropped.length > 0 ? { ...request, messages: finalMessages } : request, plan };
}
