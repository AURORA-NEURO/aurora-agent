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
import type {
  AutonomousAgent,
  AutonomousDomainName,
  AutonomousRunOptions,
  AutonomousRunResult,
  AutonomousModelRefreshSpec,
} from "./autonomous.js";
import type {
  AutonomousBrainAdaptiveCycleExecution,
  AutonomousBrainAdaptiveCycleOptions,
  AutonomousBrainAutoExecution,
  AutonomousBrainAutoExecuteOptions,
  AutonomousBrainCycleExecution,
  AutonomousBrainCycleOptions,
  AutonomousBrainExecuteOptions,
  AutonomousBrainExecution,
  AutonomousBrainFacade,
  AutonomousBrainRequest,
} from "./autonomous-brain-facade.js";
import type {
  AutonomousModelInventoryRefreshOptions,
  AutonomousModelInventorySnapshot,
} from "./autonomous-model-inventory.js";
import type { AutonomousCredentialScope } from "./autonomous-credential-scope.js";
import {
  authorizeAutonomousLaunchDomains,
  type AutonomousLaunchAdmissionReport,
} from "./autonomous-launch-admission.js";

/** Redacted provider catalog and setup-flow contract for embedding applications. */
export const PROVIDER_SETUP_SCHEMA = "bioprism-typescript-provider-setup/0.1" as const;
export const PROVIDER_CATALOG_SCHEMA = "bioprism-typescript-provider-catalog/0.1" as const;
export const AUTONOMOUS_PROVISIONED_RUN_SCHEMA = "bioprism-typescript-autonomous-provisioned-run/0.1" as const;
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

/** A caller-transient execution plus the safe provisioning/inventory projection. */
export interface AutonomousProvisionedRun<T> {
  schema: typeof AUTONOMOUS_PROVISIONED_RUN_SCHEMA;
  status: string;
  /** Provider output remains available only to the initiating application. */
  result: T;
  provisioning: CredentialProvisioningResult;
  inventory: AutonomousModelInventorySnapshot | null;
  /** Metadata-only projection safe for logs, events, and durable state. */
  toJSON(): JsonObject;
}

/** Request-scoped deployment provisioning controls; raw credentials are intentionally absent. */
export interface AutonomousProvisioningControls {
  credentialProviders?: readonly string[];
  credentialTtlMs?: number;
  environment?: Record<string, string | undefined>;
  requireReady?: boolean;
  refreshInventory?: boolean;
  inventorySpecs?: readonly AutonomousModelRefreshSpec[];
  inventoryOptions?: Omit<AutonomousModelInventoryRefreshOptions, "credentialFor" | "credentialSession">;
}

export type AutonomousProvisionedExecutionOptions = Omit<AutonomousRunOptions, "credential" | "credentialFor"> & AutonomousProvisioningControls;

export type AutonomousExplicitProvisionedExecutionOptions = Omit<AutonomousProvisionedExecutionOptions, "domain"> & {
  domain: AutonomousDomainName;
};

export type AutonomousAutomaticProvisionedExecutionOptions = Omit<AutonomousProvisionedExecutionOptions, "domain">;

type WithoutCredentialFields<T> = T extends unknown
  ? Omit<T, "credential" | "credentialFor" | "providerPlanning">
    & (T extends { providerPlanning?: infer P }
      ? { providerPlanning?: WithoutCredentialFields<NonNullable<P>> }
      : {})
  : never;
type AutonomousBrainRunOptions = NonNullable<AutonomousBrainExecuteOptions["run"]>;
type AutonomousBrainCyclePolicy = NonNullable<AutonomousBrainCycleOptions["cycle"]>;
type AutonomousBrainAdaptivePolicy = NonNullable<AutonomousBrainAdaptiveCycleOptions["adaptive"]>;

/** Brain-facade execution controls with credential handles owned by this setup boundary. */
export type AutonomousProvisionedBrainExecuteOptions = Omit<AutonomousBrainExecuteOptions, "run"> & {
  run?: WithoutCredentialFields<AutonomousBrainRunOptions>;
} & AutonomousProvisioningControls;

/** Automatic route/planning/execution controls with credential fields owned by this setup boundary. */
export type AutonomousProvisionedBrainAutoExecuteOptions = WithoutCredentialFields<AutonomousBrainAutoExecuteOptions> & AutonomousProvisioningControls;

/** Brain-facade closed-loop controls with credential handles owned by this setup boundary. */
export type AutonomousProvisionedBrainCycleOptions = Omit<AutonomousBrainCycleOptions, "cycle"> & {
  cycle?: WithoutCredentialFields<AutonomousBrainCyclePolicy>;
} & AutonomousProvisioningControls;

/** Brain-facade evaluator-guided controls with credential handles owned by this setup boundary. */
export type AutonomousProvisionedBrainAdaptiveCycleOptions = Omit<AutonomousBrainAdaptiveCycleOptions, "adaptive"> & {
  adaptive: WithoutCredentialFields<AutonomousBrainAdaptivePolicy>;
} & AutonomousProvisioningControls;

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
   * Provision, optionally refresh inventory, execute one task, and revoke the session in a
   * finally block. The callback receives no credential object; the runtime resolves the selected
   * provider through a transient opaque-handle lookup at invocation time.
   */
  private async runProvisioned<T, TOptions extends AutonomousProvisioningControls>(
    agent: AutonomousAgent,
    task: string,
    options: TOptions,
    execute: (runOptions: Omit<TOptions, keyof AutonomousProvisioningControls>, session: CredentialSession) => Promise<T>,
  ): Promise<AutonomousProvisionedRun<T>> {
    if (!agent || typeof agent.run !== "function" || typeof agent.refreshModelInventory !== "function") throw new CredentialError("provisioned autonomous execution requires an AutonomousAgent");
    if (!options || typeof options !== "object") throw new CredentialError("provisioned autonomous execution options are malformed");
    const rawOptions = options as Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(rawOptions, "credential") || Object.prototype.hasOwnProperty.call(rawOptions, "credentialFor")) {
      throw new CredentialError("provisioned autonomous execution owns credentials; pass deployment sources instead");
    }
    const refreshInventory = this.validateProvisioningControls(options);

    const {
      credentialProviders,
      credentialTtlMs,
      environment,
      requireReady,
      refreshInventory: _refreshInventory,
      inventorySpecs,
      inventoryOptions,
      ...runOptions
    } = options;
    const session = this.startSession({ ttlMs: credentialTtlMs });
    try {
      const provisioning = await this.provision(session, { providers: credentialProviders, environment });
      if ((requireReady ?? true) && !provisioning.ready) {
        throw new CredentialError(`credential provisioning is incomplete for providers: ${provisioning.required_failures.join(", ")}`);
      }
      let inventory: AutonomousModelInventorySnapshot | null = null;
      if (refreshInventory) {
        inventory = await this.refreshModelInventory(agent, session, inventorySpecs!, inventoryOptions);
        if (inventory.status !== "completed") {
          throw new CredentialError("requested model inventory refresh did not complete; execution was refused");
        }
      }
      const result = await execute(runOptions, session);
      const resultStatus = typeof result === "object" && result !== null
        ? (result as { status?: unknown }).status
        : undefined;
      const statusValue = typeof resultStatus === "string" && resultStatus.length > 0
        ? resultStatus
        : "completed";
      return {
        schema: AUTONOMOUS_PROVISIONED_RUN_SCHEMA,
        status: statusValue,
        result,
        provisioning,
        inventory,
        toJSON(): JsonObject {
          return {
            schema: AUTONOMOUS_PROVISIONED_RUN_SCHEMA,
            status: statusValue,
            result_metadata: {
              status: statusValue,
              retention: "result_transient_caller_owned",
              serialized: false,
            },
            provisioning,
            inventory: inventory === null ? null : {
              status: inventory.status,
              refresh_id: inventory.refresh_id,
              inventory_digest: inventory.inventory_digest,
              readiness: inventory.readiness,
              model_count: inventory.models.length,
              domain_count: inventory.domains.length,
              retention: "inventory_metadata_only",
            },
            credential_posture: "opaque_handles_only; session_closed_after_execution",
            secret_material: "never_returned",
          };
        },
      };
    } finally {
      session.close();
    }
  }

  /** Execute one explicit-domain task through a fresh deployment-managed credential session. */
  async runWithProvisionedCredentials(
    agent: AutonomousAgent,
    task: string,
    options: AutonomousExplicitProvisionedExecutionOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousRunResult>> {
    if (!options || typeof options.domain !== "string") throw new CredentialError("explicit provisioned execution requires a domain");
    return this.runProvisioned(agent, task, options, async (runOptions, session) => agent.run(task, {
      ...runOptions,
      credentialFor: (provider) => {
        const metadata = agent.llm.providerMetadata().find((row) => row.provider === provider);
        return metadata?.requires_credential === false ? undefined : session.handle(provider);
      },
    }));
  }

  /** Execute automatic single- or cross-domain routing through a fresh credential session. */
  async runAutoWithProvisionedCredentials(
    agent: AutonomousAgent,
    task: string,
    options: AutonomousAutomaticProvisionedExecutionOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousRunResult>> {
    const rawOptions = options as unknown as Record<string, unknown>;
    if (Object.prototype.hasOwnProperty.call(rawOptions, "domain") && rawOptions.domain !== undefined) throw new CredentialError("automatic provisioned execution chooses its route; omit domain");
    return this.runProvisioned(agent, task, options, async (runOptions, session) => agent.run(task, {
      ...runOptions,
      credentialFor: (provider) => {
        const metadata = agent.llm.providerMetadata().find((row) => row.provider === provider);
        return metadata?.requires_credential === false ? undefined : session.handle(provider);
      },
    }));
  }

  /**
   * Authorize one explicit domain before opening a credential session or refreshing inventory.
   * Provider/effect approval is still passed through to the normal execution boundary; launch
   * admission only proves that this reviewed domain was approved by the caller's preflight.
   */
  async runWithProvisionedCredentialsWithLaunchAdmission(
    agent: AutonomousAgent,
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousExplicitProvisionedExecutionOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousRunResult>> {
    if (!options || typeof options.domain !== "string") throw new CredentialError("explicit provisioned execution requires a domain");
    authorizeAutonomousLaunchDomains(admission, [options.domain]);
    return this.runWithProvisionedCredentials(agent, task, options);
  }

  /**
   * Compile the same deterministic route that automatic execution will use and authorize it
   * before provisioning. Provider-assisted semantic routing is rejected here because its
   * classifier would be a provider call before the launch admission could be enforced.
   */
  async authorizeAutoLaunchAdmission(
    agent: AutonomousAgent,
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: { hints?: readonly string[]; allowCrossDomain?: boolean; semanticRouting?: AutonomousRunOptions["semanticRouting"] } = {},
  ): Promise<AutonomousLaunchAdmissionReport> {
    if (!agent || typeof agent.route !== "function") throw new CredentialError("automatic launch admission requires an AutonomousAgent");
    const semantic = options.semanticRouting;
    if (semantic === true || (semantic !== undefined && typeof semantic === "object" && semantic !== null)) {
      throw new CredentialError("launch-admitted automatic execution requires provider-free routing; admit semantic routing separately before enabling it");
    }
    const route = await agent.route(task, {
      hints: options.hints,
      allowCrossDomain: options.allowCrossDomain,
    });
    return authorizeAutonomousLaunchDomains(admission, route.selected_domains);
  }

  /** Automatic provider execution behind a provider-free, caller-approved route admission. */
  async runAutoWithProvisionedCredentialsWithLaunchAdmission(
    agent: AutonomousAgent,
    task: string,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousAutomaticProvisionedExecutionOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousRunResult>> {
    const { hints, allowCrossDomain, semanticRouting } = options;
    await this.authorizeAutoLaunchAdmission(agent, task, admission, { hints, allowCrossDomain, semanticRouting });
    return this.runAutoWithProvisionedCredentials(agent, task, options);
  }

  private async authorizeBrainLaunchAdmission(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    ...semanticRouting: readonly unknown[]
  ): Promise<AutonomousLaunchAdmissionReport> {
    for (const semantic of semanticRouting) {
      if (semantic === true) throw new CredentialError("launch-admitted brain execution requires provider-free routing; admit semantic routing separately before enabling it");
      if (typeof semantic === "object" && semantic !== null) {
        const configured = semantic as { enabled?: unknown };
        // Direct-run semantic routing has no `enabled` field, so any object means enabled;
        // cycle-level routing has an explicit false posture that remains provider-free.
        if (!("enabled" in configured) || configured.enabled === true) {
          throw new CredentialError("launch-admitted brain execution requires provider-free routing; admit semantic routing separately before enabling it");
        }
      }
    }
    if (input.domain !== undefined) return authorizeAutonomousLaunchDomains(admission, [input.domain]);
    const route = await brain.agent.route(input.task, {
      hints: input.hints,
      allowCrossDomain: input.allow_cross_domain ?? true,
    });
    return authorizeAutonomousLaunchDomains(admission, route.selected_domains);
  }

  /** Execute the application-facing route/plan/connector/provider boundary with one fresh session. */
  async runBrainWithProvisionedCredentials(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    this.assertBrainFacade(brain, "execute");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, ["run"]);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainExecuteOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      const run = brainOptions.run === undefined ? { credentialFor } : { ...brainOptions.run, credentialFor };
      return brain.execute(input, { ...brainOptions, run } as AutonomousBrainExecuteOptions);
    });
  }

  /**
   * Brain-facade execution with launch admission checked before credential provisioning. The
   * facade checks the same admission again immediately before connector/provider dispatch.
   */
  async runBrainWithProvisionedCredentialsWithLaunchAdmission(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainExecution>> {
    this.assertBrainFacade(brain, "execute");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, ["run"]);
    await this.authorizeBrainLaunchAdmission(brain, input, admission, options.semanticRouting, options.run?.semanticRouting);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainExecuteOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      const run = brainOptions.run === undefined ? { credentialFor } : { ...brainOptions.run, credentialFor };
      return brain.executeWithLaunchAdmission(input, admission, { ...brainOptions, run } as AutonomousBrainExecuteOptions);
    });
  }

  /** Execute the automatic facade route, optional provider planning, and invocation in one session. */
  async runBrainAutoWithProvisionedCredentials(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAutoExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAutoExecution>> {
    this.assertBrainFacade(brain, "executeAuto");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, []);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainAutoExecuteOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      return brain.executeAuto(input, { ...brainOptions, credentialFor } as AutonomousBrainAutoExecuteOptions);
    });
  }

  /** Automatic facade execution with launch admission checked before credential provisioning. */
  async runBrainAutoWithProvisionedCredentialsWithLaunchAdmission(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAutoExecuteOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAutoExecution>> {
    this.assertBrainFacade(brain, "executeAuto");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, []);
    await this.authorizeBrainLaunchAdmission(brain, input, admission, options.semanticRouting);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainAutoExecuteOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      return brain.executeAutoWithLaunchAdmission(input, admission, { ...brainOptions, credentialFor } as AutonomousBrainAutoExecuteOptions);
    });
  }

  /** Execute the application-facing evaluator/learning cycle with one fresh session. */
  async runBrainCycleWithProvisionedCredentials(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainCycleOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainCycleExecution>> {
    this.assertBrainFacade(brain, "executeCycle");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, ["cycle"]);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainCycleOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      const cycle = brainOptions.cycle === undefined ? { credentialFor } : { ...brainOptions.cycle, credentialFor };
      return brain.executeCycle(input, { ...brainOptions, cycle } as AutonomousBrainCycleOptions);
    });
  }

  /** Closed-loop brain execution with pre-provision launch admission and final dispatch recheck. */
  async runBrainCycleWithProvisionedCredentialsWithLaunchAdmission(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainCycleOptions = {},
  ): Promise<AutonomousProvisionedRun<AutonomousBrainCycleExecution>> {
    this.assertBrainFacade(brain, "executeCycle");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, ["cycle"]);
    await this.authorizeBrainLaunchAdmission(brain, input, admission, options.semanticRouting);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainCycleOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      const cycle = brainOptions.cycle === undefined ? { credentialFor } : { ...brainOptions.cycle, credentialFor };
      return brain.executeCycleWithLaunchAdmission(input, admission, { ...brainOptions, cycle } as AutonomousBrainCycleOptions);
    });
  }

  /** Execute the bounded evaluator-guided replan loop with one fresh session. */
  async runBrainAdaptiveCycleWithProvisionedCredentials(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    options: AutonomousProvisionedBrainAdaptiveCycleOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAdaptiveCycleExecution>> {
    this.assertBrainFacade(brain, "executeAdaptiveCycle");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, ["adaptive"]);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainAdaptiveCycleOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      const adaptive = { ...brainOptions.adaptive, credentialFor };
      return brain.executeAdaptiveCycle(input, { ...brainOptions, adaptive } as AutonomousBrainAdaptiveCycleOptions);
    });
  }

  /** Evaluator-guided replanning with pre-provision launch admission and final dispatch recheck. */
  async runBrainAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(
    brain: AutonomousBrainFacade,
    input: AutonomousBrainRequest,
    admission: AutonomousLaunchAdmissionReport,
    options: AutonomousProvisionedBrainAdaptiveCycleOptions,
  ): Promise<AutonomousProvisionedRun<AutonomousBrainAdaptiveCycleExecution>> {
    this.assertBrainFacade(brain, "executeAdaptiveCycle");
    this.assertBrainInput(input);
    this.rejectNestedCredentialFields(options, ["adaptive"]);
    await this.authorizeBrainLaunchAdmission(brain, input, admission, options.semanticRouting);
    return this.runProvisioned(brain.agent, input.task, options, async (runOptions, session) => {
      const brainOptions = runOptions as Omit<AutonomousProvisionedBrainAdaptiveCycleOptions, keyof AutonomousProvisioningControls>;
      const credentialFor = this.credentialResolver(brain.agent, session);
      const adaptive = { ...brainOptions.adaptive, credentialFor };
      return brain.executeAdaptiveCycleWithLaunchAdmission(input, admission, { ...brainOptions, adaptive } as AutonomousBrainAdaptiveCycleOptions);
    });
  }

  /**
   * Create a deployment-owned scope for durable workers. A new session is opened only after the
   * worker releases its approval gate and the returned binding must be closed after dispatch.
   */
  createCredentialScope(
    agent: AutonomousAgent,
    options: AutonomousProvisioningControls = {},
  ): AutonomousCredentialScope {
    if (!agent || typeof agent.run !== "function" || typeof agent.refreshModelInventory !== "function") throw new CredentialError("credential scope requires an AutonomousAgent");
    const refreshInventory = this.validateProvisioningControls(options);
    const { credentialProviders, credentialTtlMs, environment, requireReady, inventorySpecs, inventoryOptions } = options;
    return {
      open: async (context) => {
        if (!context || typeof context.jobId !== "string" || !context.jobId.trim() || !Number.isSafeInteger(context.attempt) || context.attempt < 1 || context.approvalReleased !== true) throw new CredentialError("credential scope can open only for an approved durable attempt");
        const session = this.startSession({ ttlMs: credentialTtlMs });
        try {
          const provisioning = await this.provision(session, { providers: credentialProviders, environment });
          if ((requireReady ?? true) && !provisioning.ready) throw new CredentialError(`credential provisioning is incomplete for providers: ${provisioning.required_failures.join(", ")}`);
          if (refreshInventory) {
            const inventory = await this.refreshModelInventory(agent, session, inventorySpecs!, inventoryOptions);
            if (inventory.status !== "completed") throw new CredentialError("requested model inventory refresh did not complete; execution was refused");
          }
          return {
            credentialFor: this.credentialResolver(agent, session),
            close: () => session.close(),
          };
        } catch (error) {
          session.close();
          throw error;
        }
      },
    };
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

  private assertBrainFacade(brain: AutonomousBrainFacade, operation: "execute" | "executeAuto" | "executeCycle" | "executeAdaptiveCycle"): void {
    if (!brain || !brain.agent || typeof brain[operation] !== "function") throw new CredentialError(`provisioned brain ${operation} requires an AutonomousBrainFacade`);
  }

  private assertBrainInput(input: AutonomousBrainRequest): void {
    if (!input || typeof input !== "object" || typeof input.task !== "string" || !input.task.trim()) throw new CredentialError("provisioned brain execution requires a non-empty task");
  }

  private rejectNestedCredentialFields(options: unknown, sections: readonly string[]): void {
    if (!options || typeof options !== "object" || Array.isArray(options)) throw new CredentialError("provisioned brain options are malformed");
    const raw = options as Record<string, unknown>;
    const containsForbidden = (value: unknown, depth = 0): boolean => {
      if (!value || typeof value !== "object" || Array.isArray(value)) return false;
      const candidate = value as Record<string, unknown>;
      if (Object.prototype.hasOwnProperty.call(candidate, "credential") || Object.prototype.hasOwnProperty.call(candidate, "credentialFor")) return true;
      return depth < 4 && containsForbidden(candidate.providerPlanning, depth + 1);
    };
    if (containsForbidden(raw) || sections.some((section) => containsForbidden(raw[section]))) throw new CredentialError("provisioned brain execution owns credentials; pass deployment sources instead");
  }

  private credentialResolver(agent: AutonomousAgent, session: CredentialSession): (provider: string) => CredentialHandle | undefined {
    return (provider) => {
      const metadata = agent.llm.providerMetadata().find((row) => row.provider === provider);
      return metadata?.requires_credential === false ? undefined : session.handle(provider);
    };
  }

  private validateProvisioningControls(options: AutonomousProvisioningControls): boolean {
    if (!options || typeof options !== "object") throw new CredentialError("credential provisioning controls are malformed");
    if (options.requireReady !== undefined && typeof options.requireReady !== "boolean") throw new CredentialError("credential provisioning requireReady must be a boolean");
    if (options.refreshInventory !== undefined && typeof options.refreshInventory !== "boolean") throw new CredentialError("credential provisioning refreshInventory must be a boolean");
    const refreshInventory = options.refreshInventory ?? false;
    if (!refreshInventory && (options.inventorySpecs !== undefined || options.inventoryOptions !== undefined)) throw new CredentialError("inventory options require refreshInventory=true");
    if (refreshInventory && (!options.inventorySpecs || options.inventorySpecs.length === 0)) throw new CredentialError("refreshInventory=true requires inventorySpecs");
    return refreshInventory;
  }

  private assertSession(session: CredentialSession): void {
    if (!(session instanceof CredentialSession) || session.onboarding !== this.onboarding) {
      throw new CredentialError("credential session belongs to a different provider setup");
    }
  }
}
