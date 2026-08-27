import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN,
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousAgent,
  type AutonomousAutoRunOptions,
  type AutonomousAutoRunNextAction,
  type AutonomousAutoRunResult,
  type AutonomousCrossDomainRunResult,
  type AutonomousDomainName,
  type AutonomousEvidenceExecutionMode,
  type AutonomousEvidenceBackedRunStatus,
  type AutonomousPlanAndRunStatus,
  type AutonomousPromptChunk,
  type AutonomousRunResult,
  routeAutonomousEvidenceScope,
  validateAutonomousRouteOverride,
} from "./autonomous.js";
import type {
  AutonomousDomainEvidenceCatalogueExecuteOptions,
  AutonomousDomainEvidenceCataloguePrepareOptions,
  AutonomousDomainEvidenceCatalogueReconciliation,
  AutonomousDomainEvidenceSourceCatalogue,
} from "./autonomous-domain-evidence-catalogue.js";
import type { AutonomousEvidencePlan } from "./autonomous-evidence.js";
import type { AutonomousEvidenceReconciliationResult } from "./autonomous-evidence-reconciliation.js";
import { digestJson } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Metadata-only identity for the catalogue-to-provider autonomous brain bridge. */
export const AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA = "bioprism-typescript-autonomous-domain-evidence-brain-run/0.1" as const;
export const AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_SCHEMA = "bioprism-typescript-autonomous-domain-evidence-brain-context/0.1" as const;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_REQUIREMENTS = 256;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS = 8;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES = 64_000;
export const MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RESULT_BYTES = 512_000;

const RETENTION = "metadata_only;source_values_prompt_values_and_provider_response_caller_owned" as const;
const SECRET_KEYS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

export type AutonomousDomainEvidenceBrainStatus =
  | "evidence_review_required"
  | "evidence_blocked"
  | "evidence_failed"
  | "evidence_incomplete"
  | AutonomousEvidenceBackedRunStatus;

export interface AutonomousDomainEvidenceBrainPreparation {
  requirement_id: string;
  domain: AutonomousDomainName;
  prepared: AutonomousDomainEvidenceCatalogueReconciliation;
  result: AutonomousEvidenceReconciliationResult | null;
}

export interface AutonomousDomainEvidenceBrainPromptProjection {
  plan: AutonomousEvidencePlan;
  prepared: readonly AutonomousDomainEvidenceBrainPreparation[];
  /** Raw and normalized values are transient and are never included by toJSON(). */
  values: Readonly<Record<string, Readonly<Record<string, JsonValue | null>>>>;
  normalized_values: Readonly<Record<string, Readonly<Record<string, JsonValue | null>>>>;
}

export type AutonomousDomainEvidenceBrainPromptBuilder = (
  projection: AutonomousDomainEvidenceBrainPromptProjection,
) => readonly AutonomousPromptChunk[] | Promise<readonly AutonomousPromptChunk[]>;

export interface AutonomousDomainEvidenceBrainPreflight {
  plan: AutonomousEvidencePlan;
  prepared: readonly AutonomousDomainEvidenceBrainPreparation[];
  prompt_context: readonly AutonomousPromptChunk[];
}

export type AutonomousDomainEvidenceBrainPreflightHook = (
  preflight: AutonomousDomainEvidenceBrainPreflight,
) => void | Promise<void>;

export interface AutonomousDomainEvidenceBrainRunOptions {
  catalogue: AutonomousDomainEvidenceSourceCatalogue;
  domains?: readonly AutonomousDomainName[];
  availableEvidence?: readonly string[];
  completedStages?: Readonly<Record<string, readonly string[]>>;
  prepare?: AutonomousDomainEvidenceCataloguePrepareOptions;
  prepareForRequirement?: (
    requirement: AutonomousEvidencePlan["requirements"][number],
  ) => AutonomousDomainEvidenceCataloguePrepareOptions;
  execute?: Omit<AutonomousDomainEvidenceCatalogueExecuteOptions, "normalizer">;
  /** Bounded source fan-out across independent evidence requirements. */
  maxParallelRequirements?: number;
  /** Select the provider handoff shape; defaults to the historical single-domain handoff. */
  runMode?: AutonomousEvidenceExecutionMode;
  run?: AutonomousAutoRunOptions;
  /** Additional bounded controls for cross-domain fan-out and synthesis. */
  crossDomain?: Pick<import("./autonomous.js").AutonomousCrossDomainRunOptions, "subtasks" | "allowPartial" | "synthesize" | "maxParallelChildren" | "responseAlignments" | "requireResponseAlignment" | "minimumResponseReward" | "minimumResponseAlignmentConfidence" | "responseContradictionConfidenceThreshold">;
  promptBuilder?: AutonomousDomainEvidenceBrainPromptBuilder;
  beforeProviderRun?: AutonomousDomainEvidenceBrainPreflightHook;
  providerRunOverride?: AutonomousRunResult;
  automaticRunOverride?: AutonomousAutoRunResult;
  crossDomainRunOverride?: AutonomousCrossDomainRunResult;
  allowIncompleteEvidence?: boolean;
}

export interface AutonomousDomainEvidenceBrainRunProjection extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA;
  status: AutonomousDomainEvidenceBrainStatus;
  run_mode: AutonomousEvidenceExecutionMode;
  task_digest: string;
  evidence_plan_digest: string;
  catalogue_digest: string;
  normalizer_registry_digest: string;
  prepared: JsonObject[];
  reconciliations: Array<JsonObject | null>;
  prompt_context_digest: string | null;
  run_status: string | null;
  cross_domain_run_status: string | null;
  automatic_status: AutonomousPlanAndRunStatus | null;
  automatic_route_digest: string | null;
  automatic_next_action: AutonomousAutoRunNextAction | null;
  selection_digest: string | null;
  response_digest: string | null;
  retention: typeof RETENTION;
  secret_material: "never_returned";
  result_digest: string;
}

export interface AutonomousDomainEvidenceBrainRunResult {
  schema: typeof AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA;
  status: AutonomousDomainEvidenceBrainStatus;
  run_mode: AutonomousEvidenceExecutionMode;
  task_digest: string;
  plan: AutonomousEvidencePlan;
  prepared: readonly AutonomousDomainEvidenceBrainPreparation[];
  prompt_context: readonly AutonomousPromptChunk[];
  run: AutonomousRunResult | null;
  cross_domain_run: AutonomousCrossDomainRunResult | null;
  automatic: AutonomousAutoRunResult | null;
  toJSON(): AutonomousDomainEvidenceBrainRunProjection;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value.trim();
}

function assertSafeTransient(value: unknown, name: string, depth = 0): void {
  if (depth > 32) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((child, index) => assertSafeTransient(child, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (SECRET_KEYS.has(normalized) || normalized.includes("token") || normalized.includes("secret") || normalized.includes("credential")) {
        throw new ArgumentError(`${name}.${key} is credential-shaped transient data`);
      }
      assertSafeTransient(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function normalizeDomains(value: readonly AutonomousDomainName[] | undefined): AutonomousDomainName[] {
  const domains = value === undefined ? [...AUTONOMOUS_DOMAIN_NAMES] : [...value];
  if (!Array.isArray(domains) || domains.length < 1 || domains.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("domain evidence brain domains are outside their bound");
  if (domains.some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain))) throw new ArgumentError("domain evidence brain domains contain an unsupported domain");
  if (new Set(domains).size !== domains.length) throw new ArgumentError("domain evidence brain domains contain duplicates");
  return domains;
}

function normalizeRunMode(value: unknown): AutonomousEvidenceExecutionMode {
  if (value === undefined) return "domain";
  if (value !== "domain" && value !== "cross_domain" && value !== "auto") throw new ArgumentError("domain evidence brain runMode must be domain, cross_domain, or auto");
  return value;
}

function normalizePromptContext(value: readonly AutonomousPromptChunk[]): AutonomousPromptChunk[] {
  if (!Array.isArray(value) || value.length > 128) throw new ArgumentError("domain evidence brain prompt context is outside its bound");
  const result = value.map((chunk, index) => {
    if (!isObject(chunk) || typeof chunk.id !== "string" || !chunk.id.trim() || typeof chunk.content !== "string" || bytes(chunk.content) > 64_000) {
      throw new ArgumentError(`domain evidence brain prompt chunk ${index} is malformed`);
    }
    if (chunk.required !== undefined && typeof chunk.required !== "boolean") throw new ArgumentError(`domain evidence brain prompt chunk ${index}.required is malformed`);
    if (chunk.priority !== undefined && (typeof chunk.priority !== "number" || !Number.isFinite(chunk.priority))) throw new ArgumentError(`domain evidence brain prompt chunk ${index}.priority is malformed`);
    assertSafeTransient(chunk, `domain evidence brain prompt chunk ${index}`);
    return structuredClone(chunk) as unknown as AutonomousPromptChunk;
  });
  if (new Set(result.map((chunk) => chunk.id)).size !== result.length) throw new ArgumentError("domain evidence brain prompt context contains duplicate chunk IDs");
  if (bytes(JSON.stringify(result)) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES) throw new ArgumentError("domain evidence brain prompt context exceeds its bound");
  return result;
}

function defaultPromptContext(prepared: readonly AutonomousDomainEvidenceBrainPreparation[], plan: AutonomousEvidencePlan): AutonomousPromptChunk[] {
  const content = JSON.stringify({
    schema: AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_SCHEMA,
    evidence_plan_digest: plan.plan_digest,
    reconciliations: prepared.map(({ requirement_id, domain, prepared: item, result }) => ({
      requirement_id,
      domain,
      profile_id: item.profile.profile_id,
      profile_digest: item.profile.profile_digest,
      normalizer_id: item.profile.normalizer_id,
      normalizer_version: item.profile.normalizer_version,
      reconciliation_plan_digest: item.plan.plan_digest,
      result: result?.toJSON() ?? null,
    })),
    retention: RETENTION,
    secret_material: "never_returned",
  });
  if (bytes(content) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES) throw new ArgumentError("default domain evidence brain context exceeds its bound");
  return [{ id: "catalogue-reviewed-evidence", content, required: true, priority: 960 }];
}

function accepted(status: string): boolean {
  return status === "consensus" || status === "consensus_with_dissent";
}

function evidenceStatus(results: readonly AutonomousDomainEvidenceBrainPreparation[]): Exclude<AutonomousDomainEvidenceBrainStatus, AutonomousRunResult["status"]> {
  if (results.length > 0 && results.every((item) => item.result?.toJSON().status === "failed")) return "evidence_failed";
  return "evidence_incomplete";
}

function preparedProjection(item: AutonomousDomainEvidenceBrainPreparation): JsonObject {
  return {
    requirement_id: item.requirement_id,
    domain: item.domain,
    profile: item.prepared.profile,
    plan: item.prepared.plan.toJSON(),
    routes: item.prepared.routes,
    normalizer_registry_digest: item.prepared.normalizer_registry_digest,
    result_digest: item.result?.toJSON().result_digest ?? null,
  };
}

/**
 * Compose the domain source catalogue with the normal autonomous brain lifecycle.
 *
 * Catalogue preparation is request-free. Every requirement is independently route-bound and
 * reconciled under the catalogue's digest-bound normalizer registry. Source dispatch approval,
 * evidence acceptance, and provider approval remain separate. Only the explicit prompt builder
 * can bridge transient values into the provider prompt; the default context contains digests and
 * source/evaluator metadata only.
 */
export async function runAutonomousDomainEvidenceBacked(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousDomainEvidenceBrainRunOptions,
): Promise<AutonomousDomainEvidenceBrainRunResult> {
  if (!agent || typeof agent.evidencePlan !== "function" || typeof agent.run !== "function") throw new ArgumentError("domain evidence brain requires an AutonomousAgent");
  if (!options || typeof options !== "object" || !options.catalogue || typeof options.catalogue.prepare !== "function" || typeof options.catalogue.execute !== "function") throw new ArgumentError("domain evidence brain options require a typed source catalogue");
  const taskText = boundedText("domain evidence brain task", task, 32_000);
  const domains = normalizeDomains(options.domains);
  const runMode = normalizeRunMode(options.runMode);
  const overrideCount = [options.providerRunOverride, options.automaticRunOverride, options.crossDomainRunOverride].filter((value) => value !== undefined).length;
  if (overrideCount > 1) throw new ArgumentError("domain evidence brain accepts only one result override");
  if (options.providerRunOverride !== undefined && runMode !== "domain") throw new ArgumentError("domain evidence brain providerRunOverride is supported only for domain mode");
  if (options.automaticRunOverride !== undefined && runMode !== "auto") throw new ArgumentError("domain evidence brain automaticRunOverride is supported only for auto mode");
  if (options.crossDomainRunOverride !== undefined && runMode !== "cross_domain") throw new ArgumentError("domain evidence brain crossDomainRunOverride is supported only for cross_domain mode");
  if (runMode === "cross_domain" && options.domains === undefined) throw new ArgumentError("cross-domain catalogue execution requires an explicit 2..8 domain scope");
  if (runMode === "cross_domain" && (domains.length < 2 || domains.length > AUTONOMOUS_CROSS_DOMAIN_MAX_CHILDREN || domains.includes("cross_domain"))) throw new ArgumentError("cross-domain catalogue execution requires 2..8 non-synthesis domains");
  const evidenceScopeRoute = options.domains === undefined ? null : await routeAutonomousEvidenceScope(taskText, domains);
  const plan = await agent.evidencePlan(domains, { availableEvidence: options.availableEvidence, completedStages: options.completedStages });
  if (plan.requirements.length < 1 || plan.requirements.length > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_REQUIREMENTS) throw new ArgumentError("domain evidence brain plan requirements are outside their bound");
  const catalogueDigest = options.catalogue.toJSON().registry_digest;
  const normalizerRegistryDigest = options.catalogue.normalizerRegistry.registryDigest;
  const prepared: AutonomousDomainEvidenceBrainPreparation[] = plan.requirements.map((requirement) => ({
    requirement_id: requirement.requirement_id,
    domain: requirement.domain,
    prepared: options.catalogue.prepare(plan, requirement.requirement_id, {
      ...(options.prepare ?? {}),
      ...(options.prepareForRequirement?.(requirement) ?? {}),
    }),
    result: null,
  }));

  const finish = async (
    status: AutonomousDomainEvidenceBrainStatus,
    promptContext: readonly AutonomousPromptChunk[],
    run: AutonomousRunResult | null,
    crossDomainRun: AutonomousCrossDomainRunResult | null = null,
    automatic: AutonomousAutoRunResult | null = null,
  ): Promise<AutonomousDomainEvidenceBrainRunResult> => {
    const reconciliations = prepared.map((item) => item.result?.toJSON() ?? null);
    const metadataRun = run ?? crossDomainRun?.synthesis ?? (automatic?.result?.schema === "bioprism-typescript-autonomous-run/0.1" ? automatic.result : automatic?.result?.synthesis ?? null);
    const descriptor = {
      schema: AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA,
      status,
      run_mode: runMode,
      task_digest: await digestJson({ task: taskText }),
      evidence_plan_digest: plan.plan_digest,
      catalogue_digest: catalogueDigest,
      normalizer_registry_digest: normalizerRegistryDigest,
      prepared: prepared.map(preparedProjection),
      reconciliations,
      prompt_context_digest: promptContext.length ? await digestJson(promptContext) : null,
      run_status: run?.status ?? null,
      cross_domain_run_status: crossDomainRun?.status ?? null,
      automatic_status: automatic?.status ?? null,
      automatic_route_digest: automatic?.route.route_digest ?? null,
      automatic_next_action: automatic?.next_action ?? null,
      selection_digest: metadataRun?.selection ? await digestJson(metadataRun.selection) : null,
      response_digest: metadataRun?.response ? await digestJson(metadataRun.response) : null,
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    const projection = { ...descriptor, result_digest: await digestJson(descriptor) } satisfies Omit<AutonomousDomainEvidenceBrainRunProjection, "result_digest"> & { result_digest: string };
    if (bytes(JSON.stringify(projection)) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RESULT_BYTES) throw new ProviderRuntimeError("domain evidence brain result exceeds its bound");
    return {
      schema: AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA,
      status,
      run_mode: runMode,
      task_digest: descriptor.task_digest,
      plan,
      // Preserve the typed reconciliation/result objects for the caller's transient bridge;
      // cloning class instances would erase their verification and value-access methods.
      prepared: prepared.map((item) => ({ ...item })),
      prompt_context: structuredClone(promptContext),
      run,
      cross_domain_run: crossDomainRun,
      automatic,
      toJSON: () => structuredClone(projection),
    };
  };

  if (options.execute?.approveSourceDispatch !== true) return finish("evidence_review_required", [], null);
  if (options.catalogue.toJSON().registry_digest !== catalogueDigest) throw new ArgumentError("domain evidence catalogue changed after preparation; review is required again");
  const parallel = options.maxParallelRequirements ?? Math.min(MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS, prepared.length);
  if (!Number.isSafeInteger(parallel) || parallel < 1 || parallel > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS) throw new ArgumentError("domain evidence brain maxParallelRequirements is outside its bound");
  let cursor = 0;
  const worker = async (): Promise<void> => {
    while (true) {
      const index = cursor++;
      if (index >= prepared.length) return;
      const item = prepared[index]!;
      item.result = await options.catalogue.execute(plan, item.prepared, options.execute);
    }
  };
  await Promise.all(Array.from({ length: Math.min(parallel, prepared.length) }, () => worker()));
  const complete = prepared.every((item) => item.result !== null && accepted(item.result.toJSON().status));
  if (!complete && options.allowIncompleteEvidence !== true) return finish(evidenceStatus(prepared), [], null);

  const values: Record<string, Readonly<Record<string, JsonValue | null>>> = {};
  const normalizedValues: Record<string, Readonly<Record<string, JsonValue | null>>> = {};
  for (const item of prepared) {
    values[item.requirement_id] = item.result ? item.result.values : {};
    normalizedValues[item.requirement_id] = item.result ? item.result.normalizedValues : {};
  }
  const promptProjection: AutonomousDomainEvidenceBrainPromptProjection = { plan, prepared, values, normalized_values: normalizedValues };
  const promptContext = normalizePromptContext(options.promptBuilder ? await options.promptBuilder(promptProjection) : defaultPromptContext(prepared, plan));
  const runOptions = options.run ?? {};
  const context = normalizePromptContext([...(runOptions.context ?? []), ...promptContext]);
  let run: AutonomousRunResult | null = null;
  let crossDomainRun: AutonomousCrossDomainRunResult | null = null;
  let automatic: AutonomousAutoRunResult | null = null;
  if (options.providerRunOverride !== undefined) {
    if (!isObject(options.providerRunOverride) || options.providerRunOverride.schema !== "bioprism-typescript-autonomous-run/0.1") throw new ArgumentError("domain evidence brain provider run override is malformed");
    if (runOptions.approveProviderCall !== true) throw new ArgumentError("domain evidence brain provider run override requires provider approval");
    await validateAutonomousRouteOverride(taskText, options.providerRunOverride.route);
    run = options.providerRunOverride;
  } else if (options.automaticRunOverride !== undefined) {
    if (!isObject(options.automaticRunOverride) || options.automaticRunOverride.schema !== "bioprism-typescript-autonomous-auto-run/0.1") throw new ArgumentError("domain evidence brain automatic run override is malformed");
    if (runOptions.approveProviderCall !== true) throw new ArgumentError("domain evidence brain automatic run override requires provider approval");
    if (!options.automaticRunOverride.result) throw new ArgumentError("domain evidence brain automatic run override requires a completed result envelope");
    const overrideRoute = await validateAutonomousRouteOverride(taskText, options.automaticRunOverride.route);
    if (evidenceScopeRoute && overrideRoute.route_digest !== evidenceScopeRoute.route_digest) throw new ArgumentError("domain evidence brain automatic run override does not match the reviewed evidence route");
    const nestedRoute = await validateAutonomousRouteOverride(taskText, options.automaticRunOverride.result.route);
    if (nestedRoute.route_digest !== overrideRoute.route_digest) throw new ArgumentError("domain evidence brain automatic run override result route does not match its envelope");
    automatic = options.automaticRunOverride;
    const automaticResult = automatic.result;
    if (automaticResult && automaticResult.schema === "bioprism-typescript-autonomous-run/0.1") run = automaticResult;
    if (automaticResult && automaticResult.schema === "bioprism-typescript-autonomous-cross-domain-result/0.1") crossDomainRun = automaticResult;
  } else if (options.crossDomainRunOverride !== undefined) {
    if (!isObject(options.crossDomainRunOverride) || options.crossDomainRunOverride.schema !== "bioprism-typescript-autonomous-cross-domain-result/0.1") throw new ArgumentError("domain evidence brain cross-domain run override is malformed");
    if (runOptions.approveProviderCall !== true) throw new ArgumentError("domain evidence brain cross-domain run override requires provider approval");
    const overrideRoute = await validateAutonomousRouteOverride(taskText, options.crossDomainRunOverride.route);
    if (!overrideRoute.cross_domain) throw new ArgumentError("domain evidence brain cross-domain run override must carry a cross-domain route");
    if (evidenceScopeRoute && overrideRoute.route_digest !== evidenceScopeRoute.route_digest) throw new ArgumentError("domain evidence brain cross-domain run override does not match the reviewed evidence route");
    crossDomainRun = options.crossDomainRunOverride;
  } else {
    await options.beforeProviderRun?.({ plan, prepared, prompt_context: promptContext });
    if (runMode === "domain") {
      const evidenceDomain = options.domains?.length === 1 ? options.domains[0] : undefined;
      if (evidenceDomain !== undefined && runOptions.domain !== undefined && runOptions.domain !== evidenceDomain) throw new ArgumentError("catalogue evidence scope does not match the requested provider domain");
      run = await agent.run(taskText, {
        ...runOptions,
        context,
        ...(evidenceDomain !== undefined && runOptions.domain === undefined ? { domain: evidenceDomain } : {}),
        domainPolicyEvidenceReady: true,
      });
    } else if (runMode === "cross_domain") {
      if (!evidenceScopeRoute) throw new ProviderRuntimeError("cross-domain catalogue route was not compiled");
      if (runOptions.semanticRouting !== undefined && runOptions.semanticRouting !== false) throw new ArgumentError("cross-domain catalogue execution requires provider-free route binding");
      const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...crossRunOptions } = runOptions;
      crossDomainRun = await agent.runCrossDomain(taskText, {
        ...crossRunOptions,
        ...options.crossDomain,
        context,
        routeOverride: evidenceScopeRoute,
        domainPolicyEvidenceReady: true,
        semanticRouting: false,
      });
    } else {
      if (evidenceScopeRoute) {
        if (runOptions.routeOverride !== undefined) throw new ArgumentError("automatic catalogue execution owns the evidence-scope route override");
        if (runOptions.semanticRouting !== undefined && runOptions.semanticRouting !== false) throw new ArgumentError("automatic catalogue execution cannot combine an exact evidence scope with provider-assisted semantic routing");
        const { domain: _domain, routeOverride: _routeOverride, semanticRouting: _semanticRouting, ...automaticRunOptions } = runOptions;
        automatic = await agent.runAuto(taskText, {
          ...automaticRunOptions,
          context,
          routeOverride: evidenceScopeRoute,
          semanticRouting: false,
          domainPolicyEvidenceReady: true,
        });
      } else {
        automatic = await agent.runAuto(taskText, { ...runOptions, context, domainPolicyEvidenceReady: true });
      }
      if (automatic.result?.schema === "bioprism-typescript-autonomous-run/0.1") run = automatic.result;
      if (automatic.result?.schema === "bioprism-typescript-autonomous-cross-domain-result/0.1") crossDomainRun = automatic.result;
    }
  }
  const status = automatic?.status ?? crossDomainRun?.status ?? run?.status ?? "evidence_failed";
  return finish(status, promptContext, run, crossDomainRun, automatic);
}
