import { CredentialError } from "./errors.js";
import {
  CredentialHandle,
  CredentialProvisioner,
  CredentialProvisioningResult,
  CredentialSession,
  LLMRuntime,
  ProviderConfig,
  ProviderCredentialInstructions,
  ProviderFactoryOptions,
  ProviderOnboarding,
  AutonomousModelCandidate,
  AutonomousModelCandidateDefaults,
  ProviderModelDiscovery,
  anthropicProvider,
  deepseekProvider,
  groqProvider,
  mistralProvider,
  openaiProvider,
  openrouterProvider,
  providerModelsToCandidates,
  xaiProvider,
} from "./llm.js";
import type { JsonObject } from "./types.js";
import type { AutonomousAgent } from "./autonomous.js";
import type {
  AutonomousModelInventoryRefreshOptions,
  AutonomousModelInventorySnapshot,
} from "./autonomous-model-inventory.js";
import type { AutonomousModelRefreshSpec } from "./autonomous.js";

/** Redacted provider catalog and setup-flow contract for embedding applications. */
export const PROVIDER_SETUP_SCHEMA = "bioprism-typescript-provider-setup/0.1" as const;
export const PROVIDER_CATALOG_SCHEMA = "bioprism-typescript-provider-catalog/0.1" as const;
export const PROVIDER_SETUP_INPUT_METHODS = [
  "protected_ui",
  "no_echo_prompt",
  "environment_variable",
  "external_secret_resolver",
] as const;

export const SUPPORTED_PROVIDER_NAMES = [
  "openai",
  "anthropic",
  "deepseek",
  "groq",
  "mistral",
  "openrouter",
  "xai",
] as const;

export type SupportedProviderName = typeof SUPPORTED_PROVIDER_NAMES[number];

export interface ProviderPreset extends JsonObject {
  schema: typeof PROVIDER_CATALOG_SCHEMA;
  provider: SupportedProviderName;
  display_name: string;
  protocol: ProviderConfig["protocol"];
  default_base_url: string;
  default_path: string;
  default_models_path: string;
  default_structured_output_mode: NonNullable<ProviderConfig["structuredOutputMode"]>;
  environment_variable: string;
  requires_credential: true;
  input_methods: string[];
  secret_material: "never_returned";
}

export interface ProviderSetupStatus extends JsonObject {
  schema: typeof PROVIDER_SETUP_SCHEMA;
  provider: SupportedProviderName;
  display_name: string;
  protocol: ProviderConfig["protocol"];
  environment_variable: string;
  provider_registered: boolean;
  requires_credential: boolean | null;
  ready: boolean;
  next_action: string;
  input_methods: string[];
  secret_persistence: "in_memory_only";
  secret_material: "never_returned";
}

export interface ProviderSetupPlan extends JsonObject {
  schema: typeof PROVIDER_SETUP_SCHEMA;
  provider_catalog_schema: typeof PROVIDER_CATALOG_SCHEMA;
  providers: ProviderSetupStatus[];
  provider_count: number;
  ready: boolean;
  next_action: string;
  provisioning: JsonObject;
  process: string[];
  credential_posture: "caller_input_only; opaque_handles_at_runtime";
  secret_material: "never_returned";
}

interface ProviderPresetRecord {
  readonly provider: SupportedProviderName;
  readonly displayName: string;
  readonly protocol: ProviderConfig["protocol"];
  readonly baseUrl: string;
  readonly path: string;
  readonly modelsPath: string;
  readonly structuredOutputMode: NonNullable<ProviderConfig["structuredOutputMode"]>;
  readonly environmentVariable: string;
}

const PRESET_RECORDS: Readonly<Record<SupportedProviderName, ProviderPresetRecord>> = {
  openai: {
    provider: "openai",
    displayName: "OpenAI",
    protocol: "openai_responses",
    baseUrl: "https://api.openai.com",
    path: "/v1/responses",
    modelsPath: "/v1/models",
    structuredOutputMode: "json_schema",
    environmentVariable: "OPENAI_API_KEY",
  },
  anthropic: {
    provider: "anthropic",
    displayName: "Anthropic",
    protocol: "anthropic_messages",
    baseUrl: "https://api.anthropic.com",
    path: "/v1/messages",
    modelsPath: "/v1/models",
    structuredOutputMode: "disabled",
    environmentVariable: "ANTHROPIC_API_KEY",
  },
  deepseek: {
    provider: "deepseek",
    displayName: "DeepSeek",
    protocol: "openai_chat_completions",
    baseUrl: "https://api.deepseek.com",
    path: "/chat/completions",
    modelsPath: "/models",
    structuredOutputMode: "json_object",
    environmentVariable: "DEEPSEEK_API_KEY",
  },
  groq: {
    provider: "groq",
    displayName: "Groq",
    protocol: "openai_chat_completions",
    baseUrl: "https://api.groq.com/openai/v1",
    path: "/chat/completions",
    modelsPath: "/models",
    structuredOutputMode: "json_object",
    environmentVariable: "GROQ_API_KEY",
  },
  mistral: {
    provider: "mistral",
    displayName: "Mistral",
    protocol: "openai_chat_completions",
    baseUrl: "https://api.mistral.ai",
    path: "/v1/chat/completions",
    modelsPath: "/v1/models",
    structuredOutputMode: "json_object",
    environmentVariable: "MISTRAL_API_KEY",
  },
  openrouter: {
    provider: "openrouter",
    displayName: "OpenRouter",
    protocol: "openai_chat_completions",
    baseUrl: "https://openrouter.ai/api/v1",
    path: "/chat/completions",
    modelsPath: "/models",
    structuredOutputMode: "json_object",
    environmentVariable: "OPENROUTER_API_KEY",
  },
  xai: {
    provider: "xai",
    displayName: "xAI",
    protocol: "openai_chat_completions",
    baseUrl: "https://api.x.ai",
    path: "/v1/chat/completions",
    modelsPath: "/v1/models",
    structuredOutputMode: "json_object",
    environmentVariable: "XAI_API_KEY",
  },
};

function presetRecord(provider: string): ProviderPresetRecord {
  if (!SUPPORTED_PROVIDER_NAMES.includes(provider as SupportedProviderName)) {
    throw new CredentialError(`unsupported provider preset: ${provider}`);
  }
  return PRESET_RECORDS[provider as SupportedProviderName];
}

function presetFromRecord(record: ProviderPresetRecord): ProviderPreset {
  return {
    schema: PROVIDER_CATALOG_SCHEMA,
    provider: record.provider,
    display_name: record.displayName,
    protocol: record.protocol,
    default_base_url: record.baseUrl,
    default_path: record.path,
    default_models_path: record.modelsPath,
    default_structured_output_mode: record.structuredOutputMode,
    environment_variable: record.environmentVariable,
    requires_credential: true,
    input_methods: [...PROVIDER_SETUP_INPUT_METHODS],
    secret_material: "never_returned",
  };
}

/** Return every built-in provider without exposing credentials or live readiness. */
export function providerPresets(): ProviderPreset[] {
  return SUPPORTED_PROVIDER_NAMES.map((provider) => presetFromRecord(PRESET_RECORDS[provider]));
}

/** Return one redacted provider preset suitable for rendering in a setup UI. */
export function providerPreset(provider: string): ProviderPreset {
  return presetFromRecord(presetRecord(provider));
}

/** Build a transport configuration from the same presets used by the setup UI. */
export function providerConfig(provider: string, options: ProviderFactoryOptions = {}): ProviderConfig {
  const record = presetRecord(provider);
  const configured = {
    ...options,
    baseUrl: options.baseUrl ?? record.baseUrl,
    path: options.path ?? record.path,
    modelsPath: options.modelsPath ?? record.modelsPath,
    structuredOutputMode: options.structuredOutputMode ?? record.structuredOutputMode,
  } satisfies ProviderFactoryOptions;
  switch (record.provider) {
    case "openai": return openaiProvider(configured);
    case "anthropic": return anthropicProvider(configured);
    case "deepseek": return deepseekProvider(configured);
    case "groq": return groqProvider(configured);
    case "mistral": return mistralProvider(configured);
    case "openrouter": return openrouterProvider(configured);
    case "xai": return xaiProvider(configured);
  }
}

/**
 * Application-facing provider setup process.
 *
 * This class intentionally does not implement a UI and never persists a key. A browser, desktop
 * app, or operator service renders `instructions()`/`statuses()`, collects the value through its
 * own protected password input, and passes it to `collectUserCredential()` for immediate
 * conversion into a process-local opaque handle. The session is then the only credential object
 * an autonomous selection or invocation call should receive.
 */
export class ProviderSetup {
  readonly runtime: LLMRuntime;
  readonly onboarding: ProviderOnboarding;
  readonly provisioner: CredentialProvisioner;

  constructor(runtime: LLMRuntime, options: { maxSources?: number } = {}) {
    if (!(runtime instanceof LLMRuntime)) throw new CredentialError("ProviderSetup requires an LLMRuntime");
    this.runtime = runtime;
    this.onboarding = runtime.onboarding;
    this.provisioner = new CredentialProvisioner(this.onboarding, options);
  }

  /** Register one known provider's transport metadata; no credential is read. */
  registerProvider(provider: string, options: ProviderFactoryOptions = {}): ProviderPreset {
    const preset = providerPreset(provider);
    this.onboarding.registerProvider(providerConfig(provider, options));
    return preset;
  }

  /** Register all built-in provider transports so the UI can offer a complete catalog. */
  registerProviders(providers: readonly string[] = SUPPORTED_PROVIDER_NAMES, options: ProviderFactoryOptions = {}): ProviderPreset[] {
    if (!Array.isArray(providers) || providers.length === 0) throw new CredentialError("provider setup requires at least one provider");
    return providers.map((provider) => this.registerProvider(provider, options));
  }

  catalog(): ProviderPreset[] {
    return providerPresets();
  }

  instructions(provider: string): ProviderSetupStatus {
    const preset = providerPreset(provider);
    const onboarding = this.onboarding.instructions(provider) as ProviderCredentialInstructions;
    return {
      schema: PROVIDER_SETUP_SCHEMA,
      provider: preset.provider,
      display_name: preset.display_name,
      protocol: preset.protocol,
      environment_variable: preset.environment_variable,
      provider_registered: onboarding.provider_registered,
      requires_credential: onboarding.requires_credential,
      ready: onboarding.ready,
      next_action: onboarding.next_action,
      input_methods: [...PROVIDER_SETUP_INPUT_METHODS],
      secret_persistence: "in_memory_only",
      secret_material: "never_returned",
    };
  }

  statuses(providers: readonly string[] = SUPPORTED_PROVIDER_NAMES): ProviderSetupStatus[] {
    return providers.map((provider) => this.instructions(provider));
  }

  /** Start a short-lived collection/execution scope. Close it after the run or on cancellation. */
  startSession(options: { ttlMs?: number; sessionId?: string; clock?: () => number } = {}): CredentialSession {
    return this.onboarding.startSession(options);
  }

  /**
   * Protected UI boundary: callers must collect `value` in their own password/secure input
   * control and must not send it through MCP, prompts, telemetry, or durable state.
   */
  collectUserCredential(session: CredentialSession, provider: string, value: string, options: { ttlMs?: number } = {}): CredentialHandle {
    this.assertSession(session);
    return session.collectUserCredential(provider, value, options);
  }

  async configureFromPrompt(session: CredentialSession, provider: string, options: { prompt?: string; ttlMs?: number; reader?: (prompt: string) => string | Promise<string> } = {}): Promise<CredentialHandle> {
    this.assertSession(session);
    return session.configureFromPrompt(provider, options);
  }

  configureFromEnvironment(session: CredentialSession, provider: string, options: { variable?: string; ttlMs?: number; environment?: Record<string, string | undefined> } = {}): CredentialHandle {
    this.assertSession(session);
    return session.configureFromEnvironment(provider, options);
  }

  async configureFromResolver(session: CredentialSession, provider: string, reference: string, resolver: (reference: string) => string | Promise<string>, options: { ttlMs?: number } = {}): Promise<CredentialHandle> {
    this.assertSession(session);
    return session.configureFromResolver(provider, reference, resolver, options);
  }

  async provision(session: CredentialSession, options: { providers?: readonly string[]; environment?: Record<string, string | undefined> } = {}): Promise<CredentialProvisioningResult> {
    this.assertSession(session);
    return this.provisioner.provision(session, options);
  }

  /**
   * Bridge the protected onboarding session into the agent's model inventory lifecycle.
   * Provider-specific credentials are resolved only at invocation time from the opaque session;
   * this method never accepts, returns, or persists a raw key.
   */
  async refreshModelInventory(
    agent: AutonomousAgent,
    session: CredentialSession,
    specs: readonly AutonomousModelRefreshSpec[],
    options: Omit<AutonomousModelInventoryRefreshOptions, "credentialFor" | "credentialSession"> = {},
  ): Promise<AutonomousModelInventorySnapshot> {
    this.assertSession(session);
    if (!agent || typeof agent.refreshModelInventory !== "function") throw new CredentialError("provider setup model inventory requires an AutonomousAgent");
    return agent.refreshModelInventory(specs, { ...options, credentialSession: session });
  }

  /** Discover live model ids through a short-lived session without returning raw provider data. */
  async discoverModels(session: CredentialSession, provider: string, options: { signal?: AbortSignal } = {}): Promise<ProviderModelDiscovery> {
    this.assertSession(session);
    return this.runtime.discoverModels(provider, { credential: session.handle(provider), signal: options.signal });
  }

  /** Apply explicit caller-owned quality, cost, and reliability priors to discovered rows. */
  modelCandidates(discovery: ProviderModelDiscovery, defaults: AutonomousModelCandidateDefaults): AutonomousModelCandidate[] {
    if (!discovery || typeof discovery !== "object" || !Array.isArray(discovery.models)) throw new CredentialError("provider model discovery is malformed");
    return providerModelsToCandidates(discovery.models, defaults);
  }

  /** Safe setup snapshot for a UI, operator dashboard, or readiness gate. */
  plan(providers: readonly string[] = SUPPORTED_PROVIDER_NAMES): ProviderSetupPlan {
    const statuses = this.statuses(providers);
    const pending = statuses.find((status) => status.next_action !== "ready");
    return {
      schema: PROVIDER_SETUP_SCHEMA,
      provider_catalog_schema: PROVIDER_CATALOG_SCHEMA,
      providers: statuses,
      provider_count: statuses.length,
      ready: pending === undefined,
      next_action: pending?.next_action ?? "ready",
      provisioning: this.provisioner.plan(providers),
      process: [
        "register_provider_transport",
        "collect_key_at_protected_boundary",
        "create_short_lived_session",
        "select_and_invoke_with_opaque_handle",
        "close_session",
      ],
      credential_posture: "caller_input_only; opaque_handles_at_runtime",
      secret_material: "never_returned",
    };
  }

  private assertSession(session: CredentialSession): void {
    if (!(session instanceof CredentialSession) || session.onboarding !== this.onboarding) {
      throw new CredentialError("credential session belongs to a different provider setup");
    }
  }
}
