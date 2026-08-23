import { CredentialError, ProviderRuntimeError } from "./errors.js";
import {
  LLMRuntime,
  type ProviderProtocol,
  type ProviderRequest,
} from "./llm.js";
import { digestJson } from "./tooling.js";
import {
  providerConfig,
  providerPreset,
  SUPPORTED_PROVIDER_NAMES,
  type SupportedProviderName,
} from "./provider-setup.js";
import type { JsonObject } from "./types.js";

/** Public schema for deterministic, credential-free provider protocol validation. */
export const PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA = "bioprism-typescript-provider-protocol-conformance/0.1" as const;
export const PROVIDER_PROTOCOL_CONFORMANCE_MODE = "keyless_fixture_only" as const;
export const MAX_PROVIDER_CONFORMANCE_PROVIDERS = SUPPORTED_PROVIDER_NAMES.length;
export const MAX_PROVIDER_CONFORMANCE_CHECKS = MAX_PROVIDER_CONFORMANCE_PROVIDERS * 8;

const CONFORMANCE_BASE_URL = "https://aurora-provider-conformance.invalid";
const CONFORMANCE_MODEL = "aurora-conformance-model";
const CONFORMANCE_CREDENTIAL = "offline-fixture-token";
const CONFORMANCE_REQUEST_ID = "offline-fixture-request";

export type ProviderConformanceCheckName =
  | "registration"
  | "credential_guard"
  | "request_wire_shape"
  | "credential_header"
  | "response_normalization"
  | "model_discovery"
  | "stream_normalization"
  | "secret_redaction";

export interface ProviderConformanceCheck extends JsonObject {
  provider: SupportedProviderName;
  protocol: ProviderProtocol;
  check: ProviderConformanceCheckName;
  status: "passed" | "failed";
  code: string;
  metadata_only: true;
}

export interface ProviderConformanceProviderResult extends JsonObject {
  provider: SupportedProviderName;
  protocol: ProviderProtocol;
  status: "passed" | "failed";
  check_count: number;
  passed_check_count: number;
  failed_check_count: number;
  fixture_call_count: number;
  metadata_only: true;
}

export interface ProviderProtocolConformanceReport extends JsonObject {
  schema: typeof PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA;
  mode: typeof PROVIDER_PROTOCOL_CONFORMANCE_MODE;
  status: "passed" | "failed";
  provider_count: number;
  passed_provider_count: number;
  failed_provider_count: number;
  check_count: number;
  passed_check_count: number;
  failed_check_count: number;
  providers: ProviderConformanceProviderResult[];
  checks: ProviderConformanceCheck[];
  transport: "intercepted_fetch_never_networked";
  retention: "metadata_only;request_response_and_credentials_not_retained";
  secret_material: "never_returned";
  report_digest: string;
}

export interface ProviderProtocolConformanceOptions {
  /** Defaults to every built-in provider preset. No network call is made. */
  providers?: readonly string[];
  /** Defaults to a fixed synthetic model name and never enters the report. */
  model?: string;
}

interface CapturedFixtureCall {
  method: string;
  pathname: string;
  headers: Headers;
  body: JsonObject | null;
}

function jsonResponse(payload: JsonObject, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

function sseFrame(event: string | null, payload: JsonObject): string {
  return `${event ? `event: ${event}\n` : ""}data: ${JSON.stringify(payload)}\n\n`;
}

function streamResponse(protocol: ProviderProtocol, model: string): Response {
  let body = "";
  if (protocol === "openai_responses") {
    body += sseFrame("response.output_text.delta", { delta: "fixture-stream" });
    body += sseFrame("response.completed", {
      response: {
        id: CONFORMANCE_REQUEST_ID,
        model,
        usage: { input_tokens: 3, output_tokens: 2, total_tokens: 5 },
      },
    });
  } else if (protocol === "anthropic_messages") {
    body += sseFrame("message_start", { message: { id: CONFORMANCE_REQUEST_ID, model, usage: { input_tokens: 3 } } });
    body += sseFrame("content_block_delta", { index: 0, delta: { type: "text_delta", text: "fixture-stream" } });
    body += sseFrame("message_delta", { usage: { output_tokens: 2 } });
    body += sseFrame("message_stop", {});
  } else {
    body += sseFrame(null, { choices: [{ delta: { content: "fixture-stream" } }] });
    body += sseFrame(null, { choices: [{ delta: {}, finish_reason: "stop" }], usage: { prompt_tokens: 3, completion_tokens: 2, total_tokens: 5 } });
  }
  return new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } });
}

function normalResponse(protocol: ProviderProtocol, model: string): JsonObject {
  if (protocol === "openai_responses") {
    return { id: CONFORMANCE_REQUEST_ID, model, output_text: '{"ok":true}', usage: { input_tokens: 4, output_tokens: 3, total_tokens: 7 } };
  }
  if (protocol === "anthropic_messages") {
    return { id: CONFORMANCE_REQUEST_ID, model, content: [{ type: "text", text: "fixture-answer" }], usage: { input_tokens: 4, output_tokens: 3 }, stop_reason: "end_turn" };
  }
  return { id: CONFORMANCE_REQUEST_ID, model, choices: [{ message: { role: "assistant", content: '{"ok":true}' }, finish_reason: "stop" }], usage: { prompt_tokens: 4, completion_tokens: 3, total_tokens: 7 } };
}

function discoveryResponse(model: string): JsonObject {
  return {
    data: [{
      id: model,
      active: true,
      owned_by: "offline-fixture",
      context_window_tokens: 32_000,
      max_output_tokens: 4_000,
      capabilities: ["reasoning"],
      supported_parameters: ["tools", "response_format"],
    }],
  };
}

function requestFor(protocol: ProviderProtocol, model: string): ProviderRequest {
  const request: ProviderRequest = {
    model,
    messages: [
      { role: "system", content: "Use the bounded conformance contract." },
      { role: "user", content: "Return the fixture response." },
    ],
    maxOutputTokens: 64,
  };
  if (protocol !== "anthropic_messages") {
    request.requireJson = true;
    request.responseSchema = { type: "object", properties: { ok: { type: "boolean" } }, required: ["ok"], additionalProperties: false };
  }
  return request;
}

function pathFor(baseUrl: string, path: string): string {
  const url = new URL(baseUrl);
  const basePath = url.pathname.replace(/\/+$/, "");
  url.pathname = `${basePath}${path}` || "/";
  return url.pathname;
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function check(
  checks: ProviderConformanceCheck[],
  provider: SupportedProviderName,
  protocol: ProviderProtocol,
  name: ProviderConformanceCheckName,
  passed: boolean,
  code: string,
): void {
  checks.push({ provider, protocol, check: name, status: passed ? "passed" : "failed", code, metadata_only: true });
}

function lastCall(calls: readonly CapturedFixtureCall[], method: string): CapturedFixtureCall | null {
  return [...calls].reverse().find((call) => call.method === method) ?? null;
}

function safeFailureCode(error: unknown): string {
  if (error instanceof CredentialError) return "credential_error";
  if (error instanceof ProviderRuntimeError) return error.code;
  return "fixture_execution_error";
}

async function runProviderConformance(
  provider: SupportedProviderName,
  model: string,
): Promise<{ result: ProviderConformanceProviderResult; checks: ProviderConformanceCheck[] }> {
  const preset = providerPreset(provider);
  const checks: ProviderConformanceCheck[] = [];
  const calls: CapturedFixtureCall[] = [];
  const fixtureFetch = async (url: string | URL | Request, init: RequestInit = {}): Promise<Response> => {
    const method = String(init.method ?? "GET").toUpperCase();
    const headers = new Headers(init.headers);
    let body: JsonObject | null = null;
    if (init.body !== undefined) {
      try {
        const parsed: unknown = JSON.parse(String(init.body));
        if (isObject(parsed)) body = parsed;
      } catch {
        body = null;
      }
    }
    calls.push({ method, pathname: new URL(String(url)).pathname, headers, body });
    if (method === "GET") return jsonResponse(discoveryResponse(model), 200, { "x-request-id": CONFORMANCE_REQUEST_ID });
    if (body?.stream === true) return streamResponse(preset.protocol, model);
    return jsonResponse(normalResponse(preset.protocol, model), 200, { "x-request-id": CONFORMANCE_REQUEST_ID });
  };

  const runtime = new LLMRuntime({ fetch: fixtureFetch });
  try {
    const config = providerConfig(provider, {
      baseUrl: CONFORMANCE_BASE_URL,
      requiresCredential: true,
      maxAttempts: 1,
      timeoutMs: 1_000,
    });
    runtime.registerProvider(config);
    check(checks, provider, preset.protocol, "registration", true, "registered");
    const credential = runtime.credentials.register(provider, CONFORMANCE_CREDENTIAL);

    const request = requestFor(preset.protocol, model);
    let responseText = "";
    const before = calls.length;
    try {
      const response = await runtime.invoke(provider, request, { credential });
      const call = calls[before] ?? null;
      const expectedPath = pathFor(CONFORMANCE_BASE_URL, preset.default_path);
      const body = call?.body;
      const wireShape = preset.protocol === "openai_responses"
        ? Array.isArray(body?.input) && body?.max_output_tokens === 64 && isObject(body?.text)
        : preset.protocol === "anthropic_messages"
          ? Array.isArray(body?.messages) && body?.max_tokens === 64 && typeof body?.system === "string" && body?.response_format === undefined
          : Array.isArray(body?.messages) && body?.max_tokens === 64 && isObject(body?.response_format);
      check(checks, provider, preset.protocol, "request_wire_shape", call?.method === "POST" && call.pathname === expectedPath && wireShape, call?.method === "POST" && call.pathname === expectedPath && wireShape ? "wire_shape_valid" : "wire_shape_invalid");
      const authHeader = preset.protocol === "anthropic_messages" ? "x-api-key" : "authorization";
      const expectedCredential = preset.protocol === "anthropic_messages" ? CONFORMANCE_CREDENTIAL : `Bearer ${CONFORMANCE_CREDENTIAL}`;
      const credentialValid = call?.headers.get(authHeader) === expectedCredential;
      check(checks, provider, preset.protocol, "credential_header", credentialValid, credentialValid ? "credential_header_valid" : "credential_header_invalid");
      responseText = response.text;
      const structuredValid = preset.protocol === "anthropic_messages" ? response.structured === null : isObject(response.structured) && response.structured.ok === true;
      const normalized = response.provider === provider && response.model === model && response.requestId === CONFORMANCE_REQUEST_ID && response.statusCode === 200 && responseText.length > 0 && structuredValid;
      check(checks, provider, preset.protocol, "response_normalization", normalized, normalized ? "response_normalized" : "response_normalization_invalid");
    } catch (error) {
      check(checks, provider, preset.protocol, "request_wire_shape", false, safeFailureCode(error));
      check(checks, provider, preset.protocol, "credential_header", false, safeFailureCode(error));
      check(checks, provider, preset.protocol, "response_normalization", false, safeFailureCode(error));
    }

    try {
      const discovery = await runtime.discoverModels(provider, { credential });
      const discoveryValid = discovery.models.length === 1 && discovery.models[0]?.model === model && discovery.models_path === preset.default_models_path;
      const call = lastCall(calls, "GET");
      const pathValid = call?.pathname === pathFor(CONFORMANCE_BASE_URL, preset.default_models_path);
      const authHeader = preset.protocol === "anthropic_messages" ? "x-api-key" : "authorization";
      const authValid = call?.headers.get(authHeader) === (preset.protocol === "anthropic_messages" ? CONFORMANCE_CREDENTIAL : `Bearer ${CONFORMANCE_CREDENTIAL}`);
      check(checks, provider, preset.protocol, "model_discovery", discoveryValid && pathValid && authValid, discoveryValid && pathValid && authValid ? "model_discovery_valid" : "model_discovery_invalid");
    } catch (error) {
      check(checks, provider, preset.protocol, "model_discovery", false, safeFailureCode(error));
    }

    try {
      const streamRequest = requestFor(preset.protocol, model);
      delete streamRequest.requireJson;
      delete streamRequest.responseSchema;
      const events = [];
      for await (const event of runtime.invokeStream(provider, streamRequest, { credential })) events.push(event);
      const streamText = events.map((event) => event.textDelta).join("");
      const streamValid = events.length > 0 && streamText === "fixture-stream" && events.some((event) => event.done) && events.every((event) => event.provider === provider && event.model === model);
      check(checks, provider, preset.protocol, "stream_normalization", streamValid, streamValid ? "stream_normalized" : "stream_normalization_invalid");
    } catch (error) {
      check(checks, provider, preset.protocol, "stream_normalization", false, safeFailureCode(error));
    }

    const guardBefore = calls.length;
    try {
      await runtime.invoke(provider, request, {});
      check(checks, provider, preset.protocol, "credential_guard", false, "missing_credential_accepted");
    } catch (error) {
      const dispatchSafe = calls.length === guardBefore;
      const guardValid = error instanceof CredentialError && dispatchSafe;
      check(checks, provider, preset.protocol, "credential_guard", guardValid, guardValid ? "missing_credential_refused" : error instanceof CredentialError && !dispatchSafe ? "missing_credential_dispatched" : safeFailureCode(error));
    }
    const serializedChecks = JSON.stringify(checks);
    check(checks, provider, preset.protocol, "secret_redaction", !serializedChecks.includes(CONFORMANCE_CREDENTIAL), serializedChecks.includes(CONFORMANCE_CREDENTIAL) ? "fixture_secret_in_report" : "fixture_secret_redacted");
  } catch (error) {
    check(checks, provider, preset.protocol, "registration", false, safeFailureCode(error));
  }

  const passed = checks.filter((item) => item.status === "passed").length;
  const result: ProviderConformanceProviderResult = {
    provider,
    protocol: preset.protocol,
    status: passed === checks.length && checks.length > 0 ? "passed" : "failed",
    check_count: checks.length,
    passed_check_count: passed,
    failed_check_count: checks.length - passed,
    fixture_call_count: calls.length,
    metadata_only: true,
  };
  return { result, checks };
}

function normalizeProviders(values: readonly string[] | undefined): SupportedProviderName[] {
  const selected = values ?? SUPPORTED_PROVIDER_NAMES;
  if (!Array.isArray(selected) || selected.length < 1 || selected.length > MAX_PROVIDER_CONFORMANCE_PROVIDERS) {
    throw new ProviderRuntimeError(`provider conformance selection must contain 1-${MAX_PROVIDER_CONFORMANCE_PROVIDERS} providers`);
  }
  const result: SupportedProviderName[] = [];
  for (const value of selected) {
    if (!SUPPORTED_PROVIDER_NAMES.includes(value as SupportedProviderName)) throw new ProviderRuntimeError(`provider conformance does not support provider ${String(value)}`);
    const provider = value as SupportedProviderName;
    if (result.includes(provider)) throw new ProviderRuntimeError(`provider conformance selection contains duplicate provider ${provider}`);
    result.push(provider);
  }
  return result;
}

/**
 * Run every selected built-in provider through registration, auth, unary, discovery, and stream
 * boundaries using an intercepted fetch fixture. This function is intentionally unable to make a
 * network request: the only base URL is a reserved invalid host and the fetch implementation is
 * supplied locally. Synthetic credentials are held in the process-local credential store and are
 * never returned in the report.
 */
export async function runProviderProtocolConformance(options: ProviderProtocolConformanceOptions = {}): Promise<ProviderProtocolConformanceReport> {
  const providers = normalizeProviders(options.providers);
  const model = typeof options.model === "string" && options.model.trim().length > 0 ? options.model : CONFORMANCE_MODEL;
  if (model.length > 512 || /[\u0000-\u001f]/.test(model)) throw new ProviderRuntimeError("provider conformance model is outside its bounded contract");
  const rows = await Promise.all(providers.map((provider) => runProviderConformance(provider, model)));
  const providerResults = rows.map((row) => row.result);
  const checks = rows.flatMap((row) => row.checks);
  if (checks.length > MAX_PROVIDER_CONFORMANCE_CHECKS) throw new ProviderRuntimeError("provider conformance report exceeded its bounded check count");
  const passedProviderCount = providerResults.filter((row) => row.status === "passed").length;
  const passedCheckCount = checks.filter((row) => row.status === "passed").length;
  const body = {
    schema: PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA,
    mode: PROVIDER_PROTOCOL_CONFORMANCE_MODE,
    status: passedProviderCount === providerResults.length ? "passed" as const : "failed" as const,
    provider_count: providerResults.length,
    passed_provider_count: passedProviderCount,
    failed_provider_count: providerResults.length - passedProviderCount,
    check_count: checks.length,
    passed_check_count: passedCheckCount,
    failed_check_count: checks.length - passedCheckCount,
    providers: providerResults,
    checks,
    transport: "intercepted_fetch_never_networked" as const,
    retention: "metadata_only;request_response_and_credentials_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  return { ...body, report_digest: await digestJson(body) };
}

/** Fail a deployment gate when one or more provider protocol fixtures do not conform. */
export function assertProviderProtocolConformance(report: ProviderProtocolConformanceReport): void {
  if (!report || report.schema !== PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA || report.mode !== PROVIDER_PROTOCOL_CONFORMANCE_MODE) {
    throw new ProviderRuntimeError("provider conformance report is malformed", { code: "protocol" });
  }
  if (report.status !== "passed") throw new ProviderRuntimeError("provider protocol conformance failed", { code: "protocol" });
}
