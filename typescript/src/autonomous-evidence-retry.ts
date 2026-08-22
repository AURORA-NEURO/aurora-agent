import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type { AutonomousEvidenceAcquirer, AutonomousEvidenceAcquisitionContext } from "./autonomous-evidence-runtime.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Bounded, caller-controlled retry for transient evidence acquisition failures. */
export const AUTONOMOUS_EVIDENCE_RETRY_POLICY_SCHEMA = "bioprism-typescript-autonomous-evidence-retry-policy/0.1" as const;
export const AUTONOMOUS_EVIDENCE_RETRY_ATTEMPT_SCHEMA = "bioprism-typescript-autonomous-evidence-retry-attempt/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_RETRY_ATTEMPTS = 8;
export const MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS = 60_000;
export const AUTONOMOUS_EVIDENCE_DEFAULT_RETRYABLE_FAILURE_CLASSES = ["timeout", "rate_limited", "transport_error", "http_5xx"] as const;

const IDENTIFIER = /^[A-Za-z0-9_.:+\-/ ]+$/;

export interface AutonomousEvidenceRetryPolicyJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RETRY_POLICY_SCHEMA;
  max_attempts: number;
  base_delay_ms: number;
  max_delay_ms: number;
  retryable_failure_classes: string[];
  execution: "caller_controlled_bounded_retry;no_authorization";
  retention: "metadata_only_policy;no_errors_or_values";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceRetryAttempt extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RETRY_ATTEMPT_SCHEMA;
  domain: AutonomousDomainName;
  attempt: number;
  status: "succeeded" | "retrying" | "failed" | "exhausted";
  failure_class: string | null;
  retryable: boolean;
  delay_ms: number;
  latency_ms: number;
  retention: "metadata_only;error_class_only";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceRetryPolicyOptions {
  maxAttempts?: number;
  baseDelayMs?: number;
  maxDelayMs?: number;
  retryableFailureClasses?: readonly string[];
}

export interface AutonomousEvidenceRetryClassification {
  failure_class: string;
  retryable: boolean;
}

export type AutonomousEvidenceRetryClassifier = (error: unknown) => AutonomousEvidenceRetryClassification;
export type AutonomousEvidenceRetryObserver = (attempt: AutonomousEvidenceRetryAttempt) => void | Promise<void>;

export interface AutonomousEvidenceRetryAcquirerOptions extends AutonomousEvidenceRetryPolicyOptions {
  policy?: AutonomousEvidenceRetryPolicy;
  classify?: AutonomousEvidenceRetryClassifier;
  observe?: AutonomousEvidenceRetryObserver;
  clock?: () => number;
  sleep?: (delayMs: number) => Promise<void> | void;
}

/** Typed source failure that can safely cross the acquisition retry boundary. */
export class AutonomousEvidenceAcquisitionError extends Error {
  override readonly name = "AutonomousEvidenceAcquisitionError";
  readonly failure_class: string;
  readonly retryable: boolean;

  constructor(failureClass: string, retryable: boolean, message = "autonomous evidence acquisition failed") {
    super(message);
    if (typeof failureClass !== "string" || !failureClass.trim() || failureClass.length > 128 || !IDENTIFIER.test(failureClass)) throw new ArgumentError("evidence acquisition failure class is outside its bounds");
    if (typeof retryable !== "boolean") throw new ArgumentError("evidence acquisition retryable flag must be boolean");
    this.failure_class = failureClass.trim();
    this.retryable = retryable;
  }
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value as number;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value;
}

function failureClass(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 128 || !IDENTIFIER.test(value)) throw new ArgumentError("evidence retry failure class is outside its bounds");
  return value.trim();
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function defaultClassification(error: unknown): AutonomousEvidenceRetryClassification {
  if (error instanceof AutonomousEvidenceAcquisitionError) return { failure_class: error.failure_class, retryable: error.retryable };
  if (isObject(error) && typeof error.failure_class === "string" && typeof error.retryable === "boolean") return { failure_class: failureClass(error.failure_class), retryable: error.retryable };
  if (isObject(error) && error.code === "timeout") return { failure_class: "timeout", retryable: true };
  if (isObject(error) && error.code === "http_5xx") return { failure_class: "http_5xx", retryable: true };
  if (isObject(error) && error.code === "transport") return { failure_class: "transport_error", retryable: true };
  if (error instanceof Error && error.name.toLowerCase().includes("timeout")) return { failure_class: "timeout", retryable: true };
  return { failure_class: "unknown", retryable: false };
}

export class AutonomousEvidenceRetryPolicy {
  readonly max_attempts: number;
  readonly base_delay_ms: number;
  readonly max_delay_ms: number;
  readonly retryable_failure_classes: readonly string[];

  constructor(options: AutonomousEvidenceRetryPolicyOptions = {}) {
    this.max_attempts = integer("evidence retry maxAttempts", options.maxAttempts ?? 3, 1, MAX_AUTONOMOUS_EVIDENCE_RETRY_ATTEMPTS);
    this.base_delay_ms = integer("evidence retry baseDelayMs", options.baseDelayMs ?? 100, 0, MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS);
    this.max_delay_ms = integer("evidence retry maxDelayMs", options.maxDelayMs ?? 5_000, 0, MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS);
    if (this.max_delay_ms < this.base_delay_ms) throw new ArgumentError("evidence retry maxDelayMs must be at least baseDelayMs");
    const classes = options.retryableFailureClasses ?? AUTONOMOUS_EVIDENCE_DEFAULT_RETRYABLE_FAILURE_CLASSES;
    if (!Array.isArray(classes) || classes.length < 1 || classes.length > 32) throw new ArgumentError("evidence retry failure classes are outside their bound");
    const normalized = classes.map((value) => failureClass(value));
    if (new Set(normalized).size !== normalized.length) throw new ArgumentError("evidence retry failure classes contain duplicates");
    this.retryable_failure_classes = [...normalized].sort();
  }

  delayForAttempt(attempt: number): number {
    integer("evidence retry attempt", attempt, 1, this.max_attempts);
    return Math.min(this.max_delay_ms, this.base_delay_ms * (2 ** Math.max(0, attempt - 1)));
  }

  permits(classification: AutonomousEvidenceRetryClassification): boolean {
    const normalized = { failure_class: failureClass(classification.failure_class), retryable: classification.retryable };
    return normalized.retryable && this.retryable_failure_classes.includes(normalized.failure_class);
  }

  toJSON(): AutonomousEvidenceRetryPolicyJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_RETRY_POLICY_SCHEMA,
      max_attempts: this.max_attempts,
      base_delay_ms: this.base_delay_ms,
      max_delay_ms: this.max_delay_ms,
      retryable_failure_classes: [...this.retryable_failure_classes],
      execution: "caller_controlled_bounded_retry;no_authorization",
      retention: "metadata_only_policy;no_errors_or_values",
      secret_material: "never_returned",
    };
  }
}

function attemptRecord(context: AutonomousEvidenceAcquisitionContext, attempt: number, status: AutonomousEvidenceRetryAttempt["status"], failure: AutonomousEvidenceRetryClassification | null, delayMs: number, latencyMs: number): AutonomousEvidenceRetryAttempt {
  return {
    schema: AUTONOMOUS_EVIDENCE_RETRY_ATTEMPT_SCHEMA,
    domain: context.requirement.domain,
    attempt,
    status,
    failure_class: failure?.failure_class ?? null,
    retryable: failure?.retryable ?? false,
    delay_ms: delayMs,
    latency_ms: latencyMs,
    retention: "metadata_only;error_class_only",
    secret_material: "never_returned",
  };
}

/** Wrap one reviewed acquirer with bounded, typed retry; it never retries an unclassified refusal. */
export function createAutonomousEvidenceRetryingAcquirer(
  acquirer: AutonomousEvidenceAcquirer,
  options: AutonomousEvidenceRetryAcquirerOptions = {},
): AutonomousEvidenceAcquirer {
  if (!acquirer || typeof acquirer.acquire !== "function") throw new ArgumentError("evidence retry acquirer is malformed");
  const policy = options.policy ?? new AutonomousEvidenceRetryPolicy(options);
  if (!(policy instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("evidence retry policy is malformed");
  if (options.classify !== undefined && typeof options.classify !== "function") throw new ArgumentError("evidence retry classifier is malformed");
  if (options.observe !== undefined && typeof options.observe !== "function") throw new ArgumentError("evidence retry observer is malformed");
  const clock = options.clock ?? (() => Date.now());
  const sleep = options.sleep ?? ((delayMs: number) => new Promise<void>((resolve) => setTimeout(resolve, delayMs)));
  return {
    acquire: async (context: AutonomousEvidenceAcquisitionContext): Promise<JsonValue> => {
      if (!context || !context.requirement || !context.request) throw new ArgumentError("evidence retry acquisition context is malformed");
      for (let attempt = 1; attempt <= policy.max_attempts; attempt += 1) {
        const started = finite("evidence retry clock", clock(), 0, Number.MAX_SAFE_INTEGER);
        try {
          const value = await acquirer.acquire({ ...context, attempt });
          await options.observe?.(attemptRecord(context, attempt, "succeeded", null, 0, Math.max(0, finite("evidence retry clock", clock(), 0, Number.MAX_SAFE_INTEGER) - started)));
          return value;
        } catch (error) {
          const classified = options.classify ? options.classify(error) : defaultClassification(error);
          if (!classified || typeof classified !== "object" || typeof classified.retryable !== "boolean") throw new ArgumentError("evidence retry classifier returned malformed metadata");
          const normalized = { failure_class: failureClass(classified.failure_class), retryable: classified.retryable };
          const shouldRetry = policy.permits(normalized) && attempt < policy.max_attempts;
          const delayMs = shouldRetry ? policy.delayForAttempt(attempt) : 0;
          await options.observe?.(attemptRecord(context, attempt, shouldRetry ? "retrying" : attempt >= policy.max_attempts ? "exhausted" : "failed", normalized, delayMs, Math.max(0, finite("evidence retry clock", clock(), 0, Number.MAX_SAFE_INTEGER) - started)));
          if (!shouldRetry) throw error;
          await sleep(delayMs);
        }
      }
      throw new ArgumentError("evidence retry loop exhausted unexpectedly");
    },
  };
}

export function classifyAutonomousEvidenceAcquisitionError(error: unknown): AutonomousEvidenceRetryClassification {
  return clone(defaultClassification(error));
}
