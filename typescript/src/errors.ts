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

export function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
