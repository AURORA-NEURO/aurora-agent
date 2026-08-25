import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import type { ProviderMessage } from "./llm.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Versioned prompt control-plane schemas. Rendered messages are deliberately not schemas. */
export const AUTONOMOUS_PROMPT_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-prompt-registry/0.1" as const;
export const AUTONOMOUS_PROMPT_MANIFEST_SCHEMA = "bioprism-typescript-autonomous-prompt-manifest/0.1" as const;
export const AUTONOMOUS_PROMPT_SELECTION_SCHEMA = "bioprism-typescript-autonomous-prompt-selection/0.1" as const;
export const AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA = "bioprism-typescript-autonomous-prompt-selection-row/0.1" as const;
export const AUTONOMOUS_PROMPT_RENDER_SCHEMA = "bioprism-typescript-autonomous-prompt-render/0.1" as const;
export const AUTONOMOUS_PROMPT_SELECTION_POLICY = "deterministic_specificity_v1" as const;
export const AUTONOMOUS_BUILTIN_PROMPT_SCHEMA = "bioprism-typescript-autonomous-builtin-prompt/0.1" as const;
export const AUTONOMOUS_BUILTIN_PROMPT_VERSION = "1.0.0" as const;
export const MAX_AUTONOMOUS_PROMPT_TEMPLATES = 1_024;
export const MAX_AUTONOMOUS_PROMPT_CAPABILITIES = 64;
export const MAX_AUTONOMOUS_PROMPT_STAGES = 64;
export const MAX_AUTONOMOUS_PROMPT_SELECTIONS = 128;
export const MAX_AUTONOMOUS_PROMPT_MESSAGES = 64;
export const MAX_AUTONOMOUS_PROMPT_BYTES = 1_000_000;

const PROMPT_ROLES = new Set(["system", "developer", "user", "assistant", "tool"]);
const SECRET_FIELD_MARKERS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret"]);

const BUILTIN_PROMPT_INSTRUCTIONS: Readonly<Record<AutonomousDomainName, string>> = {
  coding: "Inspect the repository and constraints first. Prefer small, testable changes, explain assumptions, preserve compatibility, and report exact verification evidence.",
  browser: "Use only approved navigation and retrieval boundaries. Separate observed page facts from inference, preserve source locators transiently, and abstain when the page or authority is ambiguous.",
  data: "State the schema, units, missingness, provenance, and transformation path before interpreting results. Quantify uncertainty and never manufacture values for absent observations.",
  science: "Frame a falsifiable hypothesis, identify controls and estimands, distinguish measurement from interpretation, and surface confounders, replication limits, and alternative explanations.",
  biomedical: "Stay advisory and evidence-bound. Separate mechanistic plausibility from clinical evidence, flag safety and population limitations, and never diagnose, prescribe, enroll, or claim clinical authority.",
  neuroscience: "Specify signal, acquisition, preprocessing, temporal alignment, and confound assumptions. Distinguish neural evidence from proxy measures and preserve uncertainty around localization and causality.",
  operations: "Prefer reversible, observable, least-privilege actions. Establish impact, dependencies, rollback, incident severity, and approval gates before proposing any external effect.",
  enterprise: "Respect ownership, policy, compliance, privacy, and audit boundaries. Make decisions traceable, identify stakeholders and escalation paths, and keep recommendations separate from authorization.",
  multi_agent: "Decompose work into bounded specialist responsibilities with explicit handoffs, shared evidence identity, conflict handling, and synthesis criteria. Never treat delegation or consensus as authority.",
  multimodal: "Declare each modality and its transport limitations, align observations before synthesis, account for missing or incomparable channels, and avoid inferring a modality that was not observed.",
  cross_domain: "Coordinate domain specialists without flattening their rubrics. Preserve per-domain evidence and dissent, gate synthesis on dependency completion, and make omissions and unresolved conflicts explicit.",
  evaluation: "Use a named rubric, independent evidence, held-out or prospective checks where applicable, and explicit unscored/inapplicable states. Report failure modes and avoid turning a score into truth or authority.",
};

const BUILTIN_PROMPT_DOMAIN_CAPABILITIES: Readonly<Record<AutonomousDomainName, readonly string[]>> = {
  coding: ["implementation", "debugging", "testing"],
  browser: ["navigation", "web_research", "source_comparison"],
  data: ["data_analysis", "schema_validation", "lineage"],
  science: ["hypothesis", "literature", "experiment"],
  biomedical: ["biomedical_review", "safety_boundary", "provenance"],
  neuroscience: ["neuroscience_analysis", "signal_interpretation", "study_design"],
  operations: ["observability", "incident_response", "rollback"],
  enterprise: ["governance", "compliance", "workflow"],
  multi_agent: ["coordination", "delegation", "conflict_resolution"],
  multimodal: ["cross_modal_alignment", "image", "audio"],
  cross_domain: ["coordination", "synthesis", "evidence_alignment"],
  evaluation: ["rubric", "benchmarking", "failure_analysis"],
};

export type AutonomousPromptContext = Readonly<Record<string, unknown>>;
export type AutonomousPromptRenderer = (context: AutonomousPromptContext) => readonly ProviderMessage[] | Promise<readonly ProviderMessage[]>;

export interface AutonomousPromptManifest extends JsonObject {
  schema: typeof AUTONOMOUS_PROMPT_MANIFEST_SCHEMA;
  prompt_id: string;
  version: string;
  domain: AutonomousDomainName;
  capabilities: string[];
  stages: string[];
  template_digest: string;
  output_contract_digest: string | null;
  max_messages: number;
  max_prompt_bytes: number;
  retention: "renderer_and_rendered_messages_transient;manifest_metadata_only";
  secret_material: "never_returned";
}

export interface AutonomousPromptTemplateOptions {
  promptId: string;
  version: string;
  domain: AutonomousDomainName;
  capabilities: readonly string[];
  stages: readonly string[];
  templateDigest: string;
  render: AutonomousPromptRenderer;
  outputContractDigest?: string;
  maxMessages?: number;
  maxPromptBytes?: number;
}

export interface AutonomousPromptRenderResult extends JsonObject {
  schema: typeof AUTONOMOUS_PROMPT_RENDER_SCHEMA;
  prompt_id: string;
  version: string;
  domain: AutonomousDomainName;
  stage: string;
  manifest_digest: string;
  rendered_prompt_digest: string;
  selection_plan_digest: string | null;
  message_count: number;
  retention: "rendered_messages_transient;digest_only_projection";
  secret_material: "never_returned";
}

export interface AutonomousPromptSelectionRequest {
  domain: AutonomousDomainName;
  stage: string;
  requiredCapabilities: readonly string[];
}

export interface AutonomousPromptSelectionRow extends JsonObject {
  schema: typeof AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA;
  domain: AutonomousDomainName;
  stage: string;
  required_capabilities: string[];
  selected_prompt_id: string;
  selected_version: string;
  selected_manifest_digest: string;
  candidate_prompt_ids: string[];
  selection_reason: "stage_specificity_then_capability_fit_then_lexical_identity";
}

export interface AutonomousPromptSelectionPlanJSON extends JsonObject {
  schema: typeof AUTONOMOUS_PROMPT_SELECTION_SCHEMA;
  registry_digest: string;
  selection_policy: typeof AUTONOMOUS_PROMPT_SELECTION_POLICY;
  rows: AutonomousPromptSelectionRow[];
  plan_digest: string;
  execution: "selection_only;render_and_provider_invocation_remain_transient_caller_boundaries";
  retention: "registry_and_selection_metadata_only";
  secret_material: "never_returned";
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+/\- ]+$/.test(text)) throw new ArgumentError(`${name} contains unsupported identifier characters`);
  return text;
}

function digest(name: string, value: unknown, optional = false): string | null {
  if (optional && value === undefined) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function items(name: string, value: unknown, maximum: number, allowWildcard = false, allowEmpty = false): string[] {
  if (!Array.isArray(value) || (!allowEmpty && value.length < 1) || value.length > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  const result = value.map((item) => item === "*" && allowWildcard ? "*" : identifier(`${name} entry`, item));
  if (!allowWildcard && result.includes("*")) throw new ArgumentError(`${name} does not allow wildcard entries`);
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return result;
}

function positiveInteger(name: string, value: unknown, fallback: number, maximum: number): number {
  const selected = value === undefined ? fallback : value;
  if (!Number.isSafeInteger(selected) || (selected as number) < 1 || (selected as number) > maximum) throw new ArgumentError(`${name} must be an integer between 1 and ${maximum}`);
  return selected as number;
}

function safeJson(value: unknown, name: string, depth = 0): JsonValue {
  if (depth > 32) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (Array.isArray(value)) return value.map((item, index) => safeJson(item, `${name}[${index}]`, depth + 1));
  if (value && typeof value === "object") {
    const output: JsonObject = {};
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (!key.trim() || key.includes("\u0000") || child === undefined) throw new ArgumentError(`${name} contains an invalid object field`);
      if (SECRET_FIELD_MARKERS.has(normalized) || normalized.includes("token") || normalized.includes("secret") || normalized.includes("credential")) throw new ArgumentError(`${name} contains credential-shaped fields`);
      output[key] = safeJson(child, `${name}.${key}`, depth + 1);
    }
    return output;
  }
  throw new ArgumentError(`${name} must be JSON-safe`);
}

function contextDomainStage(context: AutonomousPromptContext): { domain: AutonomousDomainName; stage: string } {
  const requirement = context.requirement;
  const source = requirement && typeof requirement === "object" ? requirement as Record<string, unknown> : context;
  const domain = identifier("prompt context domain", source.domain) as AutonomousDomainName;
  const stage = identifier("prompt context stage_id", source.stage_id);
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError(`prompt context domain is unsupported: ${domain}`);
  return { domain, stage };
}

function manifestFor(options: AutonomousPromptTemplateOptions): AutonomousPromptManifest {
  const promptId = identifier("prompt manifest promptId", options.promptId);
  const version = boundedText("prompt manifest version", options.version, 128);
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(options.domain)) throw new ArgumentError("prompt manifest domain is unsupported");
  const capabilities = items("prompt manifest capabilities", options.capabilities, MAX_AUTONOMOUS_PROMPT_CAPABILITIES);
  const stages = items("prompt manifest stages", options.stages, MAX_AUTONOMOUS_PROMPT_STAGES, true);
  const templateDigest = digest("prompt manifest templateDigest", options.templateDigest);
  const outputContractDigest = digest("prompt manifest outputContractDigest", options.outputContractDigest, true);
  return {
    schema: AUTONOMOUS_PROMPT_MANIFEST_SCHEMA,
    prompt_id: promptId,
    version,
    domain: options.domain,
    capabilities,
    stages,
    template_digest: templateDigest!,
    output_contract_digest: outputContractDigest,
    max_messages: positiveInteger("prompt manifest maxMessages", options.maxMessages, MAX_AUTONOMOUS_PROMPT_MESSAGES, MAX_AUTONOMOUS_PROMPT_MESSAGES),
    max_prompt_bytes: positiveInteger("prompt manifest maxPromptBytes", options.maxPromptBytes, MAX_AUTONOMOUS_PROMPT_BYTES, MAX_AUTONOMOUS_PROMPT_BYTES),
    retention: "renderer_and_rendered_messages_transient;manifest_metadata_only",
    secret_material: "never_returned",
  };
}

/** A caller-owned renderer with a stable, auditable prompt identity. */
export class AutonomousPromptTemplate {
  readonly promptId: string;
  readonly version: string;
  readonly domain: AutonomousDomainName;
  readonly capabilities: readonly string[];
  readonly stages: readonly string[];
  readonly templateDigest: string;
  readonly outputContractDigest: string | undefined;
  readonly maxMessages: number;
  readonly maxPromptBytes: number;
  private readonly renderer: AutonomousPromptRenderer;

  constructor(options: AutonomousPromptTemplateOptions) {
    if (!options || typeof options !== "object" || typeof options.render !== "function") throw new ArgumentError("prompt template options are malformed");
    const manifest = manifestFor(options);
    this.promptId = manifest.prompt_id;
    this.version = manifest.version;
    this.domain = manifest.domain;
    this.capabilities = Object.freeze([...manifest.capabilities]);
    this.stages = Object.freeze([...manifest.stages]);
    this.templateDigest = manifest.template_digest;
    this.outputContractDigest = manifest.output_contract_digest ?? undefined;
    this.maxMessages = manifest.max_messages;
    this.maxPromptBytes = manifest.max_prompt_bytes;
    this.renderer = options.render;
    Object.freeze(this);
  }

  get manifest(): AutonomousPromptManifest {
    return {
      schema: AUTONOMOUS_PROMPT_MANIFEST_SCHEMA,
      prompt_id: this.promptId,
      version: this.version,
      domain: this.domain,
      capabilities: [...this.capabilities],
      stages: [...this.stages],
      template_digest: this.templateDigest,
      output_contract_digest: this.outputContractDigest ?? null,
      max_messages: this.maxMessages,
      max_prompt_bytes: this.maxPromptBytes,
      retention: "renderer_and_rendered_messages_transient;manifest_metadata_only",
      secret_material: "never_returned",
    };
  }

  get manifestDigest(): string {
    return digestJsonSync(this.manifest);
  }

  async renderTransient(context: AutonomousPromptContext, selectionPlanDigest: string | null = null): Promise<{ messages: readonly ProviderMessage[]; metadata: AutonomousPromptRenderResult }> {
    if (!context || typeof context !== "object") throw new ArgumentError("prompt render context must be an object");
    const { domain, stage } = contextDomainStage(context);
    if (domain !== this.domain) throw new ArgumentError("prompt template domain does not match render context");
    if (!this.stages.includes(stage) && !this.stages.includes("*")) throw new ArgumentError("prompt template does not cover render context stage");
    let messages: readonly ProviderMessage[];
    try {
      messages = await this.renderer(context);
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError("prompt template renderer failed");
    }
    if (!Array.isArray(messages) || messages.length < 1 || messages.length > this.maxMessages) throw new ArgumentError("prompt renderer returned an unsupported message count");
    const normalized = messages.map((message, index) => {
      if (!message || typeof message !== "object" || !PROMPT_ROLES.has(message.role)) throw new ArgumentError(`prompt message ${index} has an unsupported role`);
      if (!("content" in message)) throw new ArgumentError(`prompt message ${index} is missing content`);
      return safeJson(message, `prompt message ${index}`) as JsonObject;
    }) as unknown as readonly ProviderMessage[];
    const encoded = JSON.stringify(normalized);
    if (new TextEncoder().encode(encoded).byteLength > this.maxPromptBytes) throw new ArgumentError("rendered prompt exceeds its bounded size");
    const renderedPromptDigest = digestJsonSync(normalized);
    return {
      messages: normalized,
      metadata: {
        schema: AUTONOMOUS_PROMPT_RENDER_SCHEMA,
        prompt_id: this.promptId,
        version: this.version,
        domain,
        stage,
        manifest_digest: this.manifestDigest,
        rendered_prompt_digest: renderedPromptDigest,
        selection_plan_digest: selectionPlanDigest,
        message_count: normalized.length,
        retention: "rendered_messages_transient;digest_only_projection",
        secret_material: "never_returned",
      },
    };
  }
}

/** Immutable selection plan that binds every request to the current prompt registry. */
export class AutonomousPromptSelectionPlan {
  readonly registryDigest: string;
  readonly rows: readonly AutonomousPromptSelectionRow[];
  readonly selectionPolicy = AUTONOMOUS_PROMPT_SELECTION_POLICY;

  constructor(registryDigest: string, rows: readonly AutonomousPromptSelectionRow[]) {
    const verifiedDigest = digest("prompt selection plan registryDigest", registryDigest);
    if (!Array.isArray(rows) || rows.length < 1 || rows.length > MAX_AUTONOMOUS_PROMPT_SELECTIONS) throw new ArgumentError("prompt selection plan rows are outside their bounds");
    const normalized = rows.map((row, index) => {
      if (!row || typeof row !== "object") throw new ArgumentError(`prompt selection row ${index} is malformed`);
      if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(row.domain)) throw new ArgumentError(`prompt selection row ${index} domain is unsupported`);
      const required = items(`prompt selection row ${index} requiredCapabilities`, row.required_capabilities, MAX_AUTONOMOUS_PROMPT_CAPABILITIES, false, true);
      const candidates = items(`prompt selection row ${index} candidatePromptIds`, row.candidate_prompt_ids, MAX_AUTONOMOUS_PROMPT_TEMPLATES);
      const selectedManifestDigest = digest(`prompt selection row ${index} selectedManifestDigest`, row.selected_manifest_digest);
      return Object.freeze({
        schema: AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA,
        domain: row.domain,
        stage: identifier(`prompt selection row ${index} stage`, row.stage),
        required_capabilities: required,
        selected_prompt_id: identifier(`prompt selection row ${index} selectedPromptId`, row.selected_prompt_id),
        selected_version: boundedText(`prompt selection row ${index} selectedVersion`, row.selected_version, 128),
        selected_manifest_digest: selectedManifestDigest!,
        candidate_prompt_ids: candidates,
        selection_reason: "stage_specificity_then_capability_fit_then_lexical_identity" as const,
      });
    });
    const keys = normalized.map((row) => `${row.domain}\u0000${row.stage}\u0000${row.required_capabilities.join("\u0000")}`);
    if (new Set(keys).size !== keys.length) throw new ArgumentError("prompt selection plan rows contain duplicates");
    this.registryDigest = verifiedDigest!;
    this.rows = Object.freeze(normalized);
    Object.freeze(this);
  }

  private descriptor(): JsonObject {
    return { schema: AUTONOMOUS_PROMPT_SELECTION_SCHEMA, registry_digest: this.registryDigest, selection_policy: this.selectionPolicy, rows: this.rows as unknown as JsonValue };
  }

  get planDigest(): string {
    return digestJsonSync(this.descriptor());
  }

  toJSON(): AutonomousPromptSelectionPlanJSON {
    return {
      schema: AUTONOMOUS_PROMPT_SELECTION_SCHEMA,
      registry_digest: this.registryDigest,
      selection_policy: this.selectionPolicy,
      rows: this.rows.map((row) => ({ ...row, required_capabilities: [...row.required_capabilities], candidate_prompt_ids: [...row.candidate_prompt_ids] })),
      plan_digest: this.planDigest,
      execution: "selection_only;render_and_provider_invocation_remain_transient_caller_boundaries",
      retention: "registry_and_selection_metadata_only",
      secret_material: "never_returned",
    };
  }

  static fromJSON(value: JsonObject): AutonomousPromptSelectionPlan {
    if (!value || typeof value !== "object" || !Array.isArray(value.rows)) throw new ArgumentError("prompt selection plan JSON is malformed");
    const plan = new AutonomousPromptSelectionPlan(value.registry_digest as string, value.rows as unknown as AutonomousPromptSelectionRow[]);
    if (value.plan_digest !== undefined && value.plan_digest !== plan.planDigest) throw new ArgumentError("prompt selection plan digest does not match its contents");
    return plan;
  }
}

/** Registry, deterministic selector, stale-plan verifier, and transient renderer. */
export class AutonomousPromptRegistry {
  private readonly templates = new Map<string, AutonomousPromptTemplate>();

  constructor(templates: readonly AutonomousPromptTemplate[] = []) {
    templates.forEach((template) => this.register(template));
  }

  register(template: AutonomousPromptTemplate, options: { replace?: boolean } = {}): AutonomousPromptManifest {
    if (!(template instanceof AutonomousPromptTemplate)) throw new ArgumentError("prompt registry requires an AutonomousPromptTemplate");
    const replace = options.replace ?? false;
    if (typeof replace !== "boolean") throw new ArgumentError("prompt registry replace must be a boolean");
    if (this.templates.has(template.promptId) && !replace) throw new ArgumentError(`prompt registry already contains prompt: ${template.promptId}`);
    if (!this.templates.has(template.promptId) && this.templates.size >= MAX_AUTONOMOUS_PROMPT_TEMPLATES) throw new ArgumentError("prompt registry exceeds its template bound");
    this.templates.set(template.promptId, template);
    return template.manifest;
  }

  get manifests(): readonly AutonomousPromptManifest[] {
    return [...this.templates.keys()].sort().map((key) => this.templates.get(key)!.manifest);
  }

  get registryDigest(): string {
    return digestJsonSync({ schema: AUTONOMOUS_PROMPT_REGISTRY_SCHEMA, manifests: this.manifests });
  }

  toJSON(): JsonObject {
    return {
      schema: AUTONOMOUS_PROMPT_REGISTRY_SCHEMA,
      registry_digest: this.registryDigest,
      templates: this.manifests.map((manifest) => ({ ...manifest, capabilities: [...manifest.capabilities], stages: [...manifest.stages] })),
      template_count: this.templates.size,
      retention: "renderer_and_rendered_messages_transient;manifest_metadata_only",
      secret_material: "never_returned",
    };
  }

  templateFor(promptId: string): AutonomousPromptTemplate {
    const normalized = identifier("prompt registry promptId", promptId);
    const template = this.templates.get(normalized);
    if (!template) throw new ArgumentError(`prompt registry has no template: ${normalized}`);
    return template;
  }

  candidates(domain: AutonomousDomainName, stage: string, requiredCapabilities: readonly string[] = []): readonly AutonomousPromptTemplate[] {
    if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError("prompt candidate domain is unsupported");
    const normalizedStage = identifier("prompt candidate stage", stage);
    const required = items("prompt candidate requiredCapabilities", requiredCapabilities, MAX_AUTONOMOUS_PROMPT_CAPABILITIES, false, true);
    return [...this.templates.values()]
      .filter((template) => template.domain === domain && (template.stages.includes(normalizedStage) || template.stages.includes("*")) && required.every((capability) => template.capabilities.includes(capability)))
      .sort((left, right) => {
        const stageRank = (template: AutonomousPromptTemplate) => template.stages.includes(normalizedStage) ? 0 : 1;
        return stageRank(left) - stageRank(right) || (left.capabilities.length - required.length) - (right.capabilities.length - required.length) || left.promptId.localeCompare(right.promptId) || left.version.localeCompare(right.version);
      });
  }

  selectFor(requests: readonly AutonomousPromptSelectionRequest[]): AutonomousPromptSelectionPlan {
    if (!Array.isArray(requests) || requests.length < 1 || requests.length > MAX_AUTONOMOUS_PROMPT_SELECTIONS) throw new ArgumentError("prompt selection requests are outside their bounds");
    const rows = requests.map((request, index) => {
      if (!request || typeof request !== "object") throw new ArgumentError(`prompt selection request ${index} is malformed`);
      const candidates = this.candidates(request.domain, request.stage, request.requiredCapabilities);
      if (candidates.length === 0) throw new ArgumentError(`no prompt template satisfies ${request.domain}/${request.stage}`);
      const selected = candidates[0];
      if (!selected) throw new ArgumentError(`no prompt template satisfies ${request.domain}/${request.stage}`);
      return {
        schema: AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA,
        domain: request.domain,
        stage: identifier(`prompt selection request ${index} stage`, request.stage),
        required_capabilities: items(`prompt selection request ${index} requiredCapabilities`, request.requiredCapabilities, MAX_AUTONOMOUS_PROMPT_CAPABILITIES, false, true),
        selected_prompt_id: selected.promptId,
        selected_version: selected.version,
        selected_manifest_digest: selected.manifestDigest,
        candidate_prompt_ids: candidates.map((candidate) => candidate.promptId),
        selection_reason: "stage_specificity_then_capability_fit_then_lexical_identity" as const,
      } satisfies AutonomousPromptSelectionRow;
    });
    return new AutonomousPromptSelectionPlan(this.registryDigest, rows);
  }

  verifySelection(plan: AutonomousPromptSelectionPlan | AutonomousPromptSelectionPlanJSON): AutonomousPromptSelectionPlan {
    const verified = plan instanceof AutonomousPromptSelectionPlan ? plan : AutonomousPromptSelectionPlan.fromJSON(plan);
    if (verified.registryDigest !== this.registryDigest) throw new ArgumentError("prompt selection plan is stale for the current registry");
    for (const row of verified.rows) {
      const template = this.templateFor(row.selected_prompt_id);
      if (template.domain !== row.domain || template.version !== row.selected_version || template.manifestDigest !== row.selected_manifest_digest) throw new ArgumentError("prompt selection plan selected manifest is stale or tampered");
      if (!this.candidates(row.domain, row.stage, row.required_capabilities).includes(template)) throw new ArgumentError("prompt selection plan selected template no longer satisfies its request");
    }
    return verified;
  }

  async render(plan: AutonomousPromptSelectionPlan | AutonomousPromptSelectionPlanJSON, context: AutonomousPromptContext): Promise<{ messages: readonly ProviderMessage[]; metadata: AutonomousPromptRenderResult }> {
    const verified = this.verifySelection(plan);
    const { domain, stage } = contextDomainStage(context);
    const matching = verified.rows.filter((row) => row.domain === domain && row.stage === stage);
    if (matching.length !== 1) throw new ArgumentError("prompt selection plan has no unique row for render context");
    const selectedRow = matching[0];
    if (!selectedRow) throw new ArgumentError("prompt selection plan has no unique row for render context");
    return this.templateFor(selectedRow.selected_prompt_id).renderTransient(context, verified.planDigest);
  }
}

function builtinPromptSubject(context: AutonomousPromptContext): string {
  const requirement = context.requirement;
  const source = requirement && typeof requirement === "object" ? requirement as Record<string, unknown> : context;
  const candidate = source.objective || source.label || context.task || context.objective || context.label;
  if (typeof candidate !== "string" || !candidate.trim() || candidate.includes("\u0000")) throw new ArgumentError("built-in prompt context requires a bounded objective");
  const subject = boundedText("built-in prompt objective", candidate, 32_000);
  return subject;
}

function createBuiltinPromptTemplate(domain: AutonomousDomainName): AutonomousPromptTemplate {
  const instruction = BUILTIN_PROMPT_INSTRUCTIONS[domain];
  const capabilities = [
    "analysis",
    "llm_evidence",
    "structured_output",
    "safe_reasoning",
    ...BUILTIN_PROMPT_DOMAIN_CAPABILITIES[domain],
    `domain:${domain}`,
  ];
  const templateDigest = digestJsonSync({
    schema: AUTONOMOUS_BUILTIN_PROMPT_SCHEMA,
    version: AUTONOMOUS_BUILTIN_PROMPT_VERSION,
    domain,
    instruction,
    capabilities,
  });
  return new AutonomousPromptTemplate({
    promptId: `builtin.${domain}.specialist`,
    version: AUTONOMOUS_BUILTIN_PROMPT_VERSION,
    domain,
    capabilities,
    stages: ["*"],
    templateDigest,
    render: (context) => {
      const { domain: contextDomain, stage } = contextDomainStage(context);
      const subject = builtinPromptSubject(context);
      return [
        {
          role: "system",
          content: `You are AURORA's ${domain} specialist for the ${stage} stage. ${instruction} Treat provider output as an observation, preserve explicit approval boundaries, and do not invent evidence, credentials, permissions, or external effects.`,
        },
        {
          role: "user",
          content: `Reviewed objective for ${contextDomain}: ${subject}`,
        },
      ];
    },
  });
}

/** Return the reviewed built-in specialist template for each requested autonomous domain. */
export function builtinAutonomousPromptTemplates(domains: readonly AutonomousDomainName[] = AUTONOMOUS_DOMAIN_NAMES): readonly AutonomousPromptTemplate[] {
  if (!Array.isArray(domains) || domains.length < 1 || domains.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("built-in prompt domains are outside their bounds");
  if (new Set(domains).size !== domains.length || domains.some((domain) => !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain))) throw new ArgumentError("built-in prompt domains contain an unsupported or duplicate domain");
  return domains.map((domain) => createBuiltinPromptTemplate(domain));
}

/** Create a complete caller-owned registry of domain-specialist prompt templates. */
export function builtinAutonomousPromptRegistry(domains: readonly AutonomousDomainName[] = AUTONOMOUS_DOMAIN_NAMES): AutonomousPromptRegistry {
  return new AutonomousPromptRegistry(builtinAutonomousPromptTemplates(domains));
}
