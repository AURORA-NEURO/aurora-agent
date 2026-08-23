import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import {
  LLMRuntime,
  type CredentialHandle,
  type ProviderInvocationObserver,
  type ProviderMessage,
  type ProviderRequest,
  type ProviderResponse,
} from "./llm.js";
import type {
  AutonomousEvidenceAcquisitionContext,
  AutonomousEvidenceProjector,
} from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterRegistrationInput as EvidenceAdapterRegistrationInput,
} from "./autonomous-evidence-adapters.js";
import { AutonomousEvidenceAcquisitionError } from "./autonomous-evidence-retry.js";
import { digestJson } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Provider-backed evidence adapter bridge over the existing provider-neutral LLM runtime. */
export const AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SCHEMA = "bioprism-typescript-autonomous-llm-evidence-adapter/0.1" as const;
export const MAX_AUTONOMOUS_LLM_EVIDENCE_PROMPT_MESSAGES = 64;
export const MAX_AUTONOMOUS_LLM_EVIDENCE_OUTPUT_TOKENS = 32_000;
export const MAX_AUTONOMOUS_LLM_EVIDENCE_MODEL_BYTES = 256;

const SECRET_FIELD_MARKERS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret"]);

type JsonParser = (response: ProviderResponse, context: AutonomousEvidenceAcquisitionContext) => JsonValue | Promise<JsonValue>;
type ModelResolver = (context: AutonomousEvidenceAcquisitionContext) => string | Promise<string>;
type CredentialResolver = (provider: string, context: AutonomousEvidenceAcquisitionContext) => CredentialHandle | undefined;
type PromptResolver = (context: AutonomousEvidenceAcquisitionContext) => readonly ProviderMessage[] | Promise<readonly ProviderMessage[]>;

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function positiveInteger(name: string, value: unknown, fallback: number, maximum: number): number {
  const selected = value === undefined ? fallback : value;
  if (!Number.isSafeInteger(selected) || (selected as number) < 1 || (selected as number) > maximum) throw new ArgumentError(`${name} must be an integer between 1 and ${maximum}`);
  return selected as number;
}

function safeModel(name: string, value: unknown): string {
  return boundedText(name, value, MAX_AUTONOMOUS_LLM_EVIDENCE_MODEL_BYTES);
}

function jsonValue(value: unknown, name: string, depth = 0): JsonValue {
  if (depth > 32) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (Array.isArray(value)) return value.map((item) => jsonValue(item, name, depth + 1));
  if (value && typeof value === "object") {
    const output: JsonObject = {};
    for (const [key, child] of Object.entries(value)) {
      if (!key.trim() || key.includes("\u0000") || child === undefined) throw new ArgumentError(`${name} contains an invalid object field`);
      const normalizedKey = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (SECRET_FIELD_MARKERS.has(normalizedKey) || normalizedKey.includes("token") || normalizedKey.includes("secret") || normalizedKey.includes("credential")) throw new ArgumentError(`${name} contains credential-shaped response fields`);
      output[key] = jsonValue(child, `${name}.${key}`, depth + 1);
    }
    return output;
  }
  throw new ArgumentError(`${name} must be JSON-safe`);
}

function providerFailure(error: unknown): AutonomousEvidenceAcquisitionError {
  if (error instanceof AutonomousEvidenceAcquisitionError) return error;
  if (error instanceof ProviderRuntimeError) {
    const failureClass = error.code === "transport"
      ? "transport_error"
      : error.code === "http_5xx"
        ? "http_5xx"
        : error.code === "timeout"
          ? "timeout"
          : error.code === "http_4xx"
            ? "http_4xx"
            : error.code === "credential"
              ? "credential_error"
              : error.code === "circuit_open"
                ? "circuit_open"
                : error.code === "invalid_response"
                  ? "invalid_response"
                  : error.code === "invalid_request"
                    ? "invalid_request"
                    : "provider_error";
    return new AutonomousEvidenceAcquisitionError(failureClass, error.retryable === true);
  }
  return new AutonomousEvidenceAcquisitionError("provider_error", false);
}

function defaultParser(response: ProviderResponse): JsonValue {
  if (response.structured !== null) return jsonValue(response.structured, "LLM evidence structured response");
  return response.text;
}

function requestIdentity(context: AutonomousEvidenceAcquisitionContext): Promise<string> {
  return digestJson({
    schema: AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SCHEMA,
    plan_digest: context.plan_digest,
    requirement_id: context.requirement.requirement_id,
    source_id: context.request.source_id,
    request_id: context.request.request_id ?? null,
  });
}

export interface AutonomousLLMEvidenceAdapterOptions {
  adapterId: string;
  version: string;
  domain: AutonomousDomainName;
  provider: string;
  runtime: LLMRuntime;
  model?: string;
  modelForContext?: ModelResolver;
  capabilities: readonly string[];
  sourceKinds?: readonly string[];
  credential?: CredentialHandle;
  credentialFor?: CredentialResolver;
  promptForContext: PromptResolver;
  parseResponse?: JsonParser;
  project?: AutonomousEvidenceProjector["project"];
  maxOutputTokens?: number;
  temperature?: number;
  requireJson?: boolean;
  responseSchema?: JsonObject;
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
  invocationKind?: string;
}

/** Build a scoped evidence adapter that uses LLMRuntime for provider selection, credentials, health, and invocation. */
export function createAutonomousLLMEvidenceAdapterRegistration(
  options: AutonomousLLMEvidenceAdapterOptions,
): Omit<EvidenceAdapterRegistrationInput, "domains"> & { domains: [AutonomousDomainName] } {
  if (!options || typeof options !== "object") throw new ArgumentError("LLM evidence adapter options are malformed");
  if (!(options.runtime instanceof LLMRuntime)) throw new ArgumentError("LLM evidence adapter requires an LLMRuntime");
  if (typeof options.promptForContext !== "function") throw new ArgumentError("LLM evidence adapter promptForContext is required");
  if (options.parseResponse !== undefined && typeof options.parseResponse !== "function") throw new ArgumentError("LLM evidence adapter parseResponse is malformed");
  if (options.project !== undefined && typeof options.project !== "function") throw new ArgumentError("LLM evidence adapter project is malformed");
  if (options.credential !== undefined && options.credentialFor !== undefined) throw new ArgumentError("LLM evidence adapter cannot configure both credential and credentialFor");
  if (options.credentialFor !== undefined && typeof options.credentialFor !== "function") throw new ArgumentError("LLM evidence adapter credentialFor is malformed");
  const adapterId = boundedText("LLM evidence adapter adapterId", options.adapterId, 256);
  const version = boundedText("LLM evidence adapter version", options.version, 128);
  const provider = boundedText("LLM evidence adapter provider", options.provider, 256);
  const outputTokens = positiveInteger("LLM evidence adapter maxOutputTokens", options.maxOutputTokens, 1_024, MAX_AUTONOMOUS_LLM_EVIDENCE_OUTPUT_TOKENS);
  if (options.temperature !== undefined && (typeof options.temperature !== "number" || !Number.isFinite(options.temperature) || options.temperature < 0 || options.temperature > 2)) throw new ArgumentError("LLM evidence adapter temperature must be between 0 and 2");
  if (options.responseSchema !== undefined && options.requireJson !== true) throw new ArgumentError("LLM evidence adapter responseSchema requires requireJson=true");
  const staticModel = options.model === undefined ? undefined : safeModel("LLM evidence adapter model", options.model);
  if (staticModel === undefined && options.modelForContext === undefined) throw new ArgumentError("LLM evidence adapter requires model or modelForContext");
  if (staticModel !== undefined && options.modelForContext !== undefined) throw new ArgumentError("LLM evidence adapter cannot configure both model and modelForContext");
  if (options.modelForContext !== undefined && typeof options.modelForContext !== "function") throw new ArgumentError("LLM evidence adapter modelForContext is malformed");
  return {
    adapterId,
    version,
    domains: [options.domain],
    capabilities: options.capabilities,
    sourceKinds: options.sourceKinds ?? ["llm_structured"],
    project: options.project,
    acquire: async (context) => {
      try {
        const model = safeModel("LLM evidence adapter resolved model", staticModel ?? await options.modelForContext!(context));
        const messages = await options.promptForContext(context);
        if (!Array.isArray(messages) || messages.length < 1 || messages.length > MAX_AUTONOMOUS_LLM_EVIDENCE_PROMPT_MESSAGES) throw new ArgumentError("LLM evidence adapter prompt must contain between 1 and 64 messages");
        const request: ProviderRequest = {
          model,
          messages,
          maxOutputTokens: outputTokens,
          ...(options.temperature === undefined ? {} : { temperature: options.temperature }),
          ...(options.requireJson === undefined ? {} : { requireJson: options.requireJson }),
          ...(options.responseSchema === undefined ? {} : { responseSchema: options.responseSchema }),
          idempotencyKey: await requestIdentity(context),
        };
        const response = await options.runtime.invoke(provider, request, {
          ...(options.credential === undefined ? {} : { credential: options.credential }),
          ...(options.credentialFor === undefined ? {} : { credential: options.credentialFor(provider, context) }),
          signal: options.signal,
          observer: options.observer,
          invocationKind: options.invocationKind ?? "autonomous_evidence_acquisition",
        });
        const parser = options.parseResponse ?? ((value: ProviderResponse) => defaultParser(value));
        return jsonValue(await parser(response, context), "LLM evidence adapter parsed response");
      } catch (error) {
        if (error instanceof ArgumentError) throw error;
        throw providerFailure(error);
      }
    },
  };
}

/** Register one provider-backed evidence adapter while preserving the existing registry contract. */
export function registerAutonomousLLMEvidenceAdapter(
  registry: AutonomousEvidenceAdapterRegistry,
  options: AutonomousLLMEvidenceAdapterOptions,
  registrationOptions: { replace?: boolean } = {},
): JsonObject {
  if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("LLM evidence adapter registration requires a typed adapter registry");
  return registry.register(createAutonomousLLMEvidenceAdapterRegistration(options), registrationOptions);
}
