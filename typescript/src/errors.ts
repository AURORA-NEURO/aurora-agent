import type { ApiErrorBody, JsonValue, MissionJob } from "./types.js";

/** Base class for errors raised before a remote tool is allowed to run. */
export class PrismSdkError extends Error {
  override readonly name: string = "PrismSdkError";
}

/** The caller supplied an invalid, unsafe, or unbounded argument. */
export class ArgumentError extends PrismSdkError {
  override readonly name: string = "ArgumentError";
}

/** The fetch implementation could not complete a bounded transport operation. */
export class TransportError extends PrismSdkError {
  override readonly name: string = "TransportError";
  override readonly cause?: unknown;

  constructor(message: string, cause?: unknown) {
    super(message);
    this.cause = cause;
  }
}

/** The gateway returned an HTTP error with a structured JSON payload. */
export class ApiError extends TransportError {
  override readonly name = "ApiError";
  readonly status: number;
  readonly payload: ApiErrorBody | JsonValue;
  readonly requestId?: string;

  constructor(status: number, payload: ApiErrorBody | JsonValue, requestId?: string) {
    const detail = isObject(payload) && isObject(payload.error)
      ? `${payload.error.code ?? "api_error"}: ${payload.error.message ?? "request failed"}`
      : `HTTP ${status}`;
    super(`Prism HTTP API returned ${status}: ${detail}`);
    this.status = status;
    this.payload = payload;
    this.requestId = requestId ?? (isObject(payload) && typeof payload.request_id === "string" ? payload.request_id : undefined);
  }
}

/** The server returned a body that was not a bounded JSON object. */
export class ProtocolError extends PrismSdkError {
  override readonly name = "ProtocolError";
}

/** A response exceeded the client-side byte ceiling before it could be parsed. */
export class ResponseTooLargeError extends TransportError {
  override readonly name = "ResponseTooLargeError";
  readonly maxResponseBytes: number;

  constructor(maxResponseBytes: number) {
    super(`HTTP API response exceeded maxResponseBytes (${maxResponseBytes})`);
    this.maxResponseBytes = maxResponseBytes;
  }
}

/** Bounded mission polling expired while retaining the last authoritative status snapshot. */
export class MissionWaitTimeoutError extends PrismSdkError {
  override readonly name = "MissionWaitTimeoutError";
  readonly missionId: string;
  readonly timeoutMs: number;
  readonly lastJob: MissionJob;

  constructor(missionId: string, timeoutMs: number, lastJob: MissionJob) {
    super(`timed out waiting for mission ${missionId} after ${timeoutMs}ms; last status is ${lastJob.status}`);
    this.missionId = missionId;
    this.timeoutMs = timeoutMs;
    this.lastJob = lastJob;
  }
}

/** A remote tool ran and returned a structured refusal or MCP error. */
export class ToolRefusalError extends PrismSdkError {
  override readonly name = "ToolRefusalError";
  readonly tool: string;
  readonly response: unknown;

  constructor(tool: string, response: unknown) {
    super(`${tool}: remote tool returned a structured refusal or protocol error`);
    this.tool = tool;
    this.response = response;
  }
}

/** A provider credential was missing, expired, revoked, or used with the wrong provider. */
export class CredentialError extends PrismSdkError {
  override readonly name = "CredentialError";
}

/** A caller-owned autonomous cost ceiling refused another provider attempt. */
export class AutonomousCostBudgetError extends ArgumentError {
  override readonly name = "AutonomousCostBudgetError";
  readonly maxCostUnits: number;
  readonly consumedCostUnits: number;
  readonly requestedCostUnits: number;

  constructor(message: string, options: { maxCostUnits: number; consumedCostUnits: number; requestedCostUnits: number }) {
    super(message);
    this.maxCostUnits = options.maxCostUnits;
    this.consumedCostUnits = options.consumedCostUnits;
    this.requestedCostUnits = options.requestedCostUnits;
  }
}

/** Stable, redacted categories for provider failures; raw provider bodies are never embedded. */
export type ProviderErrorCode =
  | "provider_error"
  | "configuration"
  | "invalid_request"
  | "credential"
  | "aborted"
  | "timeout"
  | "circuit_open"
  | "http_4xx"
  | "http_5xx"
  | "transport"
  | "response_too_large"
  | "protocol"
  | "invalid_response";

export type ProviderFailureClass =
  | "provider_error"
  | "credential_error"
  | "aborted"
  | "timeout"
  | "circuit_open"
  | "http_4xx"
  | "http_5xx"
  | "response_too_large"
  | "protocol_error";

/** A provider invocation failed at the bounded transport or protocol boundary. */
export class ProviderRuntimeError extends PrismSdkError {
  override readonly name = "ProviderRuntimeError";
  readonly retryable: boolean;
  readonly statusCode?: number;
  readonly circuitOpen: boolean;
  readonly code: ProviderErrorCode;
  readonly provider?: string;
  readonly operation?: string;
  readonly requestId?: string;
  readonly retryAfterMs?: number;
  readonly attempt?: number;

  constructor(
    message: string,
    options: {
      retryable?: boolean;
      statusCode?: number;
      circuitOpen?: boolean;
      code?: ProviderErrorCode;
      provider?: string;
      operation?: string;
      requestId?: string;
      retryAfterMs?: number;
      attempt?: number;
    } = {},
  ) {
    super(message);
    this.retryable = options.retryable ?? false;
    this.statusCode = options.statusCode;
    this.circuitOpen = options.circuitOpen ?? false;
    this.code = options.code ?? (this.circuitOpen ? "circuit_open" : "provider_error");
    this.provider = options.provider;
    this.operation = options.operation;
    this.requestId = options.requestId;
    this.retryAfterMs = options.retryAfterMs;
    this.attempt = options.attempt;
  }

  /** Add non-secret execution context without changing the stable failure category. */
  withContext(context: { provider?: string; operation?: string; requestId?: string; attempt?: number }): ProviderRuntimeError {
    return new ProviderRuntimeError(this.message, {
      retryable: this.retryable,
      statusCode: this.statusCode,
      circuitOpen: this.circuitOpen,
      code: this.code,
      provider: context.provider ?? this.provider,
      operation: context.operation ?? this.operation,
      requestId: context.requestId ?? this.requestId,
      retryAfterMs: this.retryAfterMs,
      attempt: context.attempt ?? this.attempt,
    });
  }
}

export function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
