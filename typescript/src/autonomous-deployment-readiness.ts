import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName, type AutonomousReadinessReport } from "./autonomous.js";
import { PROVIDER_SETUP_SCHEMA, type ProviderSetupPlan } from "./provider-setup.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only deployment audit schemas. This module never creates authority or opens a session. */
export const AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA = "bioprism-typescript-autonomous-deployment-readiness/0.1" as const;
export const AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-deployment-readiness-domain/0.1" as const;
export const AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA = "bioprism-typescript-autonomous-deployment-readiness-capability/0.1" as const;
export const MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BYTES = 512_000;
export const MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BLOCKERS = 512;

const EXECUTION = "audit_only;no_provider_source_tool_queue_or_credential_dispatch" as const;
const RETENTION = "metadata_only;digests_capabilities_and_next_actions" as const;
const SECRET_MATERIAL = "never_returned" as const;

export const AUTONOMOUS_DEPLOYMENT_READINESS_STATES = ["ready_for_review", "partial", "blocked"] as const;
export type AutonomousDeploymentReadinessState = typeof AUTONOMOUS_DEPLOYMENT_READINESS_STATES[number];

export const AUTONOMOUS_DEPLOYMENT_BLOCKER_CODES = [
  "model_catalogue",
  "model_capability",
  "provider_registration",
  "credential",
  "tool_catalogue",
  "evidence_adapter",
  "learning",
  "persistence",
  "queue",
  "approval_authority",
  "external_auth",
  "telemetry",
] as const;
export type AutonomousDeploymentBlockerCode = typeof AUTONOMOUS_DEPLOYMENT_BLOCKER_CODES[number];

export const AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES = [
  "persistence",
  "queue",
  "approval_authority",
  "external_auth",
  "telemetry",
] as const;
export type AutonomousDeploymentCapabilityName = typeof AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES[number];

export interface AutonomousDeploymentReadinessPolicy {
  requireCredentials?: boolean;
  requireToolCatalogue?: boolean;
  requireEvidence?: boolean;
  requireLearning?: boolean;
  requirePersistence?: boolean;
  requireQueue?: boolean;
  requireApprovalAuthority?: boolean;
  requireExternalAuth?: boolean;
  requireTelemetry?: boolean;
}

export interface AutonomousDeploymentCapabilityInput extends JsonObject {
  configured: boolean;
  operational: boolean;
  restart_safe: boolean;
  integrity_fenced: boolean;
  caller_owned: boolean;
  next_actions?: string[];
}

export interface AutonomousDeploymentReadinessInput {
  agent: AutonomousReadinessReport;
  provider_plan: ProviderSetupPlan;
  capabilities?: Partial<Record<AutonomousDeploymentCapabilityName, AutonomousDeploymentCapabilityInput>>;
}

export interface AutonomousDeploymentCapabilityProjection extends JsonObject {
  schema: typeof AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA;
  name: AutonomousDeploymentCapabilityName;
  required: boolean;
  configured: boolean;
  operational: boolean;
  restart_safe: boolean;
  integrity_fenced: boolean;
  caller_owned: boolean;
  satisfies_requirement: boolean;
  next_actions: string[];
  execution: "projection_only;does_not_initialize_or_test_capability";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousDeploymentBlocker extends JsonObject {
  code: AutonomousDeploymentBlockerCode;
  scope: "global" | "domain";
  domain: AutonomousDomainName | null;
  severity: "blocking" | "warning";
  message: string;
  next_action: string;
}

export interface AutonomousDeploymentReadinessDomain extends JsonObject {
  schema: typeof AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA;
  domain: AutonomousDomainName;
  workflow_id: string;
  workflow_digest: string;
  agent_state: string;
  state: AutonomousDeploymentReadinessState;
  model_gate: {
    compatible_model_count: number;
    eligible_model_count: number;
    required_model_capabilities: string[];
  };
  tool_gate: {
    required_tool_count: number;
    available_tool_count: number;
    missing_tools: string[];
  };
  evidence_gate: {
    requested: boolean;
    status: string;
    report_digest: string | null;
  };
  learning_gate: {
    required: boolean;
    configured: boolean;
    calibration_decision: string | null;
  };
  blockers: AutonomousDeploymentBlocker[];
  warnings: AutonomousDeploymentBlocker[];
  next_actions: string[];
  execution: typeof EXECUTION;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousDeploymentReadinessReport extends JsonObject {
  schema: typeof AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA;
  agent_readiness_digest: string;
  provider_setup_digest: string;
  policy: {
    require_credentials: boolean;
    require_tool_catalogue: boolean;
    require_evidence: boolean;
    require_learning: boolean;
    require_persistence: boolean;
    require_queue: boolean;
    require_approval_authority: boolean;
    require_external_auth: boolean;
    require_telemetry: boolean;
  };
  provider_gate: {
    candidate_provider_count: number;
    ready_provider_count: number;
    unresolved_provider_count: number;
    providers: Array<{
      provider: string;
      registered: boolean;
      ready: boolean;
      next_action: string;
    }>;
  };
  capabilities: AutonomousDeploymentCapabilityProjection[];
  domains: AutonomousDeploymentReadinessDomain[];
  global_blockers: AutonomousDeploymentBlocker[];
  warnings: AutonomousDeploymentBlocker[];
  ready_domain_count: number;
  partial_domain_count: number;
  blocked_domain_count: number;
  state: AutonomousDeploymentReadinessState;
  next_actions: string[];
  readiness_claimed: false;
  execution: typeof EXECUTION;
  authority: "audit_does_not_grant_dispatch_authority";
  credential_posture: "caller_owned_protected_input;opaque_runtime_handles_only";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  readiness_digest: string;
}

interface NormalizedPolicy {
  require_credentials: boolean;
  require_tool_catalogue: boolean;
  require_evidence: boolean;
  require_learning: boolean;
  require_persistence: boolean;
  require_queue: boolean;
  require_approval_authority: boolean;
  require_external_auth: boolean;
  require_telemetry: boolean;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value as number;
}

function strings(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value.map((item, index) => boundedText(`${name}[${index}]`, item));
}

function exactKeys(value: object, allowed: readonly string[], name: string): void {
  const allowedSet = new Set(allowed);
  for (const key of Object.keys(value)) if (!allowedSet.has(key)) throw new ArgumentError(`${name} contains unsupported field ${key}`);
}

function policy(input: AutonomousDeploymentReadinessPolicy = {}): NormalizedPolicy {
  if (!input || typeof input !== "object" || Array.isArray(input)) throw new ArgumentError("deployment readiness policy is malformed");
  const read = (key: keyof AutonomousDeploymentReadinessPolicy, fallback: boolean): boolean => {
    const value = input[key];
    if (value !== undefined && typeof value !== "boolean") throw new ArgumentError(`deployment readiness ${key} must be boolean`);
    return value ?? fallback;
  };
  return {
    require_credentials: read("requireCredentials", true),
    require_tool_catalogue: read("requireToolCatalogue", false),
    require_evidence: read("requireEvidence", false),
    require_learning: read("requireLearning", false),
    require_persistence: read("requirePersistence", true),
    require_queue: read("requireQueue", false),
    require_approval_authority: read("requireApprovalAuthority", true),
    require_external_auth: read("requireExternalAuth", false),
    require_telemetry: read("requireTelemetry", false),
  };
}

function validateAgent(report: unknown): AutonomousReadinessReport {
  if (!isObject(report)) throw new ArgumentError("deployment readiness agent report is malformed");
  exactKeys(report, ["schema", "providers", "models", "domains", "workflows", "domain_packs", "model_capability_coverage", "model_inventory_readiness", "model_health", "learning", "tooling", "evidence", "connectors", "activation", "next_actions", "readiness_state", "execution", "credential_posture", "secret_material", "readiness_digest"], "deployment readiness agent report");
  if (report.schema !== "bioprism-autonomous-agent-readiness/0.1") throw new ArgumentError("deployment readiness agent schema is unsupported");
  digest("deployment readiness agent digest", report.readiness_digest);
  const { readiness_digest: _digest, ...withoutDigest } = report;
  if (digestJsonSync(withoutDigest) !== report.readiness_digest) throw new ArgumentError("deployment readiness agent digest does not match its metadata");
  if (report.secret_material !== "never_returned" || report.execution !== "not_started; no_provider_or_tool_calls") throw new ArgumentError("deployment readiness agent report has an unsafe execution posture");
  if (!isObject(report.model_inventory_readiness) || report.model_inventory_readiness.schema !== "bioprism-typescript-autonomous-model-inventory-readiness/0.1" || report.model_inventory_readiness.execution !== "provider_readiness_projection_only;no_discovery_or_invocation" || report.model_inventory_readiness.secret_material !== "never_returned") throw new ArgumentError("deployment readiness agent model inventory readiness is malformed");
  if (!Array.isArray(report.model_inventory_readiness.domains) || report.model_inventory_readiness.domains.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("deployment readiness agent model inventory readiness must cover all built-in domains");
  if (typeof report.model_inventory_readiness.readiness_digest !== "string" || !/^[0-9a-f]{64}$/.test(report.model_inventory_readiness.readiness_digest)) throw new ArgumentError("deployment readiness agent model inventory readiness digest is malformed");
  const { readiness_digest: _inventoryDigest, execution: _inventoryExecution, selection_posture: _inventorySelectionPosture, retention: _inventoryRetention, secret_material: _inventorySecretMaterial, ...withoutInventoryMarkers } = report.model_inventory_readiness;
  if (digestJsonSync(withoutInventoryMarkers) !== report.model_inventory_readiness.readiness_digest) throw new ArgumentError("deployment readiness agent model inventory readiness digest does not match its metadata");
  if (!Array.isArray(report.domains) || report.domains.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("deployment readiness agent report must cover all built-in domains");
  const seen = new Set<string>();
  for (const [index, row] of report.domains.entries()) {
    if (!isObject(row) || !AUTONOMOUS_DOMAIN_NAMES.includes(row.domain as AutonomousDomainName)) throw new ArgumentError(`deployment readiness domain row ${index} is malformed`);
    if (seen.has(row.domain as string)) throw new ArgumentError(`deployment readiness domain ${row.domain} is duplicated`);
    seen.add(row.domain as string);
    if (!["ready_for_caller_approval", "model_catalogue_required", "provider_registration_required", "credential_required", "model_capability_gap", "partial"].includes(row.state as string)) throw new ArgumentError(`deployment readiness domain ${row.domain} state is invalid`);
    integer(`deployment readiness ${row.domain} compatible model count`, row.compatible_model_count, 0, 1_000_000);
    integer(`deployment readiness ${row.domain} eligible model count`, row.eligible_model_count, 0, row.compatible_model_count as number);
    integer(`deployment readiness ${row.domain} required tool count`, row.required_tool_count, 0, 1_000_000);
    integer(`deployment readiness ${row.domain} available tool count`, row.available_tool_count, 0, row.required_tool_count as number);
    strings(`deployment readiness ${row.domain} missing tools`, row.missing_tools, 1_000);
    strings(`deployment readiness ${row.domain} next actions`, row.next_actions, 64);
  }
  if (seen.size !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("deployment readiness agent report does not cover every built-in domain");
  return report as unknown as AutonomousReadinessReport;
}

function validateProviderPlan(plan: unknown): ProviderSetupPlan {
  if (!isObject(plan)) throw new ArgumentError("deployment readiness provider plan is malformed");
  exactKeys(plan, ["schema", "provider_catalog_schema", "providers", "provider_count", "ready", "next_action", "provisioning", "process", "credential_posture", "secret_material"], "deployment readiness provider plan");
  if (plan.schema !== PROVIDER_SETUP_SCHEMA || plan.secret_material !== "never_returned" || plan.credential_posture !== "caller_input_only; opaque_handles_at_runtime") throw new ArgumentError("deployment readiness provider plan has an unsafe schema or credential posture");
  if (!Array.isArray(plan.providers) || plan.providers.length < 1 || plan.providers.length > 128) throw new ArgumentError("deployment readiness provider plan providers are outside their bound");
  integer("deployment readiness provider count", plan.provider_count, 1, 128);
  if (plan.provider_count !== plan.providers.length || typeof plan.ready !== "boolean") throw new ArgumentError("deployment readiness provider plan counts are inconsistent");
  for (const [index, row] of plan.providers.entries()) {
    if (!isObject(row)) throw new ArgumentError(`deployment readiness provider ${index} is malformed`);
    boundedText(`deployment readiness provider ${index} name`, row.provider, 128);
    if (typeof row.provider_registered !== "boolean" || typeof row.ready !== "boolean") throw new ArgumentError(`deployment readiness provider ${index} flags are malformed`);
    boundedText(`deployment readiness provider ${index} next action`, row.next_action, 256);
  }
  return plan as unknown as ProviderSetupPlan;
}

function normalizeCapability(name: AutonomousDeploymentCapabilityName, input: AutonomousDeploymentCapabilityInput | undefined): AutonomousDeploymentCapabilityInput {
  if (input === undefined) return { configured: false, operational: false, restart_safe: false, integrity_fenced: false, caller_owned: true, next_actions: [`configure ${name} through the deployment owner`] };
  if (!isObject(input)) throw new ArgumentError(`deployment readiness capability ${name} is malformed`);
  exactKeys(input, ["configured", "operational", "restart_safe", "integrity_fenced", "caller_owned", "next_actions"], `deployment readiness capability ${name}`);
  for (const key of ["configured", "operational", "restart_safe", "integrity_fenced", "caller_owned"] as const) if (typeof input[key] !== "boolean") throw new ArgumentError(`deployment readiness capability ${name} ${key} must be boolean`);
  return { configured: input.configured, operational: input.operational, restart_safe: input.restart_safe, integrity_fenced: input.integrity_fenced, caller_owned: input.caller_owned, next_actions: strings(`deployment readiness capability ${name} next actions`, input.next_actions ?? [], 32) };
}

function capabilityRequired(name: AutonomousDeploymentCapabilityName, normalized: NormalizedPolicy): boolean {
  return normalized[`require_${name}` as keyof NormalizedPolicy] as boolean;
}

function capabilitySatisfied(status: AutonomousDeploymentCapabilityInput): boolean {
  return status.configured && status.operational && status.restart_safe && status.integrity_fenced;
}

function sortBlockers(rows: readonly AutonomousDeploymentBlocker[]): AutonomousDeploymentBlocker[] {
  return [...rows].sort((left, right) => `${left.scope}:${left.domain ?? ""}:${left.code}:${left.message}`.localeCompare(`${right.scope}:${right.domain ?? ""}:${right.code}:${right.message}`));
}

function summarizeToolNames(names: readonly string[], maximum = 1_700): string {
  const joined = names.join(", ");
  if (joined.length <= maximum) return joined;
  const suffix = ` … (+${names.length} total; list truncated)`;
  return `${joined.slice(0, Math.max(0, maximum - suffix.length))}${suffix}`;
}

function blocker(code: AutonomousDeploymentBlockerCode, scope: "global" | "domain", domain: AutonomousDomainName | null, message: string, nextAction: string, severity: "blocking" | "warning" = "blocking"): AutonomousDeploymentBlocker {
  return { code, scope, domain, severity, message: boundedText("deployment readiness blocker message", message, 2_048), next_action: boundedText("deployment readiness blocker next action", nextAction, 1_024) };
}

function reportPayload(report: Omit<AutonomousDeploymentReadinessReport, "readiness_digest">): Omit<AutonomousDeploymentReadinessReport, "readiness_digest"> {
  return report;
}

function validateReportShape(report: unknown): AutonomousDeploymentReadinessReport {
  if (!isObject(report)) throw new ArgumentError("deployment readiness report is malformed");
  exactKeys(report, ["schema", "agent_readiness_digest", "provider_setup_digest", "policy", "provider_gate", "capabilities", "domains", "global_blockers", "warnings", "ready_domain_count", "partial_domain_count", "blocked_domain_count", "state", "next_actions", "readiness_claimed", "execution", "authority", "credential_posture", "retention", "secret_material", "readiness_digest"], "deployment readiness report");
  if (report.schema !== AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA || report.readiness_claimed !== false || report.execution !== EXECUTION || report.authority !== "audit_does_not_grant_dispatch_authority" || report.secret_material !== SECRET_MATERIAL) throw new ArgumentError("deployment readiness report has an unsafe execution posture");
  digest("deployment readiness report agent digest", report.agent_readiness_digest);
  digest("deployment readiness report provider digest", report.provider_setup_digest);
  digest("deployment readiness report digest", report.readiness_digest);
  const { readiness_digest: _digest, ...withoutDigest } = report;
  if (digestJsonSync(withoutDigest) !== report.readiness_digest) throw new ArgumentError("deployment readiness report digest does not match its metadata");
  if (!AUTONOMOUS_DEPLOYMENT_READINESS_STATES.includes(report.state as AutonomousDeploymentReadinessState)) throw new ArgumentError("deployment readiness report state is invalid");
  if (!Array.isArray(report.domains) || report.domains.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("deployment readiness report domain count is invalid");
  if (!Array.isArray(report.capabilities) || report.capabilities.length !== AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES.length) throw new ArgumentError("deployment readiness report capability count is invalid");
  strings("deployment readiness report next actions", report.next_actions, 512);
  return report as unknown as AutonomousDeploymentReadinessReport;
}

/**
 * Joins agent readiness, protected provider onboarding, and deployment-owned capability gates.
 * It is intentionally an audit object: callers must still perform explicit review and use their
 * own deployment adapters to initialize persistence, queues, authentication, and telemetry.
 */
export class AutonomousDeploymentReadinessAuditor {
  readonly policy: NormalizedPolicy;

  constructor(options: AutonomousDeploymentReadinessPolicy = {}) {
    this.policy = policy(options);
  }

  audit(input: AutonomousDeploymentReadinessInput): AutonomousDeploymentReadinessReport {
    if (!input || typeof input !== "object" || Array.isArray(input)) throw new ArgumentError("deployment readiness input is malformed");
    const agent = validateAgent(input.agent);
    const providerPlan = validateProviderPlan(input.provider_plan);
    const usedProviders = new Set<string>();
    if (!Array.isArray(agent.models)) throw new ArgumentError("deployment readiness agent models are malformed");
    for (const [index, model] of agent.models.entries()) {
      if (!isObject(model)) throw new ArgumentError(`deployment readiness model ${index} is malformed`);
      usedProviders.add(boundedText(`deployment readiness model ${index} provider`, model.provider, 128));
    }
    const providerRows = [...usedProviders].sort().map((provider) => {
      const status = providerPlan.providers.find((candidate) => candidate.provider === provider);
      return {
        provider,
        registered: status?.provider_registered === true,
        ready: status?.ready === true,
        next_action: status?.next_action ?? "register_provider_transport",
      };
    });
    const globalBlockers: AutonomousDeploymentBlocker[] = [];
    if (!usedProviders.size) globalBlockers.push(blocker("model_catalogue", "global", null, "the agent readiness report contains no model candidates", "register reviewed model candidates with domain capabilities"));
    for (const row of providerRows) {
      if (row.registered === false) globalBlockers.push(blocker("provider_registration", "global", null, `provider transport ${row.provider} is not registered`, `register_provider_transport: ${row.provider}`));
      else if (this.policy.require_credentials && row.ready === false) globalBlockers.push(blocker("credential", "global", null, `provider ${row.provider} is registered but not credential-ready`, `collect_user_credential through a protected onboarding boundary: ${row.provider}`));
    }

    const capabilities: AutonomousDeploymentCapabilityProjection[] = AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES.map((name) => {
      const inputCapability = normalizeCapability(name, input.capabilities?.[name]);
      const required = capabilityRequired(name, this.policy);
      const satisfies = capabilitySatisfied(inputCapability);
      const nextActions = inputCapability.next_actions ?? [];
      if (required && !satisfies) globalBlockers.push(blocker(name, "global", null, `${name} is required but its deployment contract is incomplete`, nextActions[0] ?? `configure ${name} with restart-safe integrity fencing`));
      return {
        schema: AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA,
        name,
        required,
        configured: inputCapability.configured,
        operational: inputCapability.operational,
        restart_safe: inputCapability.restart_safe,
        integrity_fenced: inputCapability.integrity_fenced,
        caller_owned: inputCapability.caller_owned,
        satisfies_requirement: !required || satisfies,
        next_actions: [...nextActions],
        execution: "projection_only;does_not_initialize_or_test_capability",
        retention: RETENTION,
        secret_material: SECRET_MATERIAL,
      };
    });

    const domainRows: AutonomousDeploymentReadinessDomain[] = agent.domains.map((row) => {
      const domain = row.domain as AutonomousDomainName;
      const domainBlockers: AutonomousDeploymentBlocker[] = [];
      const warnings: AutonomousDeploymentBlocker[] = [];
      if (row.state === "model_catalogue_required") domainBlockers.push(blocker("model_catalogue", "domain", domain, "no model catalogue is available for this domain", "register a reviewed candidate model for this domain"));
      if (row.state === "model_capability_gap") domainBlockers.push(blocker("model_capability", "domain", domain, "available models do not declare the required domain capabilities", "register a model with the required capabilities"));
      if (row.state === "provider_registration_required") domainBlockers.push(blocker("provider_registration", "domain", domain, "compatible model providers are not registered", "register the provider transport before invocation"));
      if (row.state === "credential_required") domainBlockers.push(blocker("credential", "domain", domain, "compatible providers have no active caller credential", "collect a short-lived user credential through protected onboarding"));
      if (row.state === "partial") domainBlockers.push(blocker("model_capability", "domain", domain, "domain readiness is partial and requires review before deployment", row.next_actions[0] ?? "resolve the domain readiness next action"));
      if (this.policy.require_tool_catalogue && row.missing_tools.length > 0) domainBlockers.push(blocker("tool_catalogue", "domain", domain, `required domain tools are missing: ${summarizeToolNames(row.missing_tools)}`, "attach and review the live tool catalogue"));
      else if (row.missing_tools.length > 0) warnings.push(blocker("tool_catalogue", "domain", domain, `optional domain tools are not currently attached: ${summarizeToolNames(row.missing_tools)}`, "attach a reviewed tool catalogue for richer execution", "warning"));
      const evidence = isObject(row.evidence_readiness) ? row.evidence_readiness : undefined;
      const evidenceStatus = typeof evidence?.status === "string" ? evidence.status : "not_requested";
      const evidenceDigest = typeof evidence?.report_digest === "string" && /^[0-9a-f]{64}$/.test(evidence.report_digest) ? evidence.report_digest : null;
      if (this.policy.require_evidence && evidenceStatus !== "ready") domainBlockers.push(blocker("evidence_adapter", "domain", domain, `evidence readiness is ${evidenceStatus}`, "register and health-check a source adapter before evidence dispatch"));
      else if (evidenceStatus === "blocked" || evidenceStatus === "missing") warnings.push(blocker("evidence_adapter", "domain", domain, `evidence readiness is ${evidenceStatus}`, "resolve source adapter coverage before evidence-backed work", "warning"));
      const learning = isObject(agent.learning) ? agent.learning : undefined;
      const learningConfigured = learning?.configured === true;
      const calibration = isObject(learning?.calibration) && typeof learning.calibration.decision === "string" ? learning.calibration.decision : null;
      if (this.policy.require_learning && !learningConfigured) domainBlockers.push(blocker("learning", "domain", domain, "online learning is required but no learner is attached", "attach persisted online learning and evaluator settlement"));
      if (this.policy.require_learning && calibration !== null && calibration !== "admit_learning") domainBlockers.push(blocker("learning", "domain", domain, `learning calibration is ${calibration}`, "resolve evaluator calibration before enabling learning"));
      const state: AutonomousDeploymentReadinessState = domainBlockers.length ? "blocked" : row.state === "ready_for_caller_approval" ? "ready_for_review" : "partial";
      const nextActions = new Set<string>(row.next_actions.filter((value): value is string => typeof value === "string"));
      for (const current of domainBlockers) nextActions.add(current.next_action);
      for (const current of warnings) nextActions.add(current.next_action);
      return {
        schema: AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA,
        domain,
        workflow_id: boundedText(`deployment readiness ${domain} workflow id`, row.workflow_id, 256),
        workflow_digest: digest(`deployment readiness ${domain} workflow digest`, row.workflow_digest),
        agent_state: boundedText(`deployment readiness ${domain} agent state`, row.state, 128),
        state,
        model_gate: {
          compatible_model_count: row.compatible_model_count as number,
          eligible_model_count: row.eligible_model_count as number,
          required_model_capabilities: strings(`deployment readiness ${domain} required model capabilities`, row.required_model_capabilities, 256),
        },
        tool_gate: {
          required_tool_count: row.required_tool_count as number,
          available_tool_count: row.available_tool_count as number,
          missing_tools: strings(`deployment readiness ${domain} missing tools`, row.missing_tools, 1_000),
        },
        evidence_gate: { requested: this.policy.require_evidence, status: evidenceStatus, report_digest: evidenceDigest },
        learning_gate: { required: this.policy.require_learning, configured: learningConfigured, calibration_decision: calibration },
        blockers: sortBlockers(domainBlockers),
        warnings: sortBlockers(warnings),
        next_actions: [...nextActions].sort(),
        execution: EXECUTION,
        retention: RETENTION,
        secret_material: SECRET_MATERIAL,
      };
    });
    const sortedGlobalBlockers = sortBlockers(globalBlockers);
    const warnings = sortBlockers(domainRows.flatMap((row) => row.warnings));
    const readyDomainCount = domainRows.filter((row) => row.state === "ready_for_review").length;
    const partialDomainCount = domainRows.filter((row) => row.state === "partial").length;
    const blockedDomainCount = domainRows.filter((row) => row.state === "blocked").length;
    const state: AutonomousDeploymentReadinessState = sortedGlobalBlockers.length || blockedDomainCount === domainRows.length ? "blocked" : readyDomainCount === domainRows.length ? "ready_for_review" : "partial";
    const nextActions = new Set<string>(agent.next_actions.filter((value): value is string => typeof value === "string"));
    for (const current of sortedGlobalBlockers) nextActions.add(current.next_action);
    for (const current of warnings) nextActions.add(current.next_action);
    const body: Omit<AutonomousDeploymentReadinessReport, "readiness_digest"> = reportPayload({
      schema: AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA,
      agent_readiness_digest: agent.readiness_digest,
      provider_setup_digest: digestJsonSync(providerPlan),
      policy: {
        require_credentials: this.policy.require_credentials,
        require_tool_catalogue: this.policy.require_tool_catalogue,
        require_evidence: this.policy.require_evidence,
        require_learning: this.policy.require_learning,
        require_persistence: this.policy.require_persistence,
        require_queue: this.policy.require_queue,
        require_approval_authority: this.policy.require_approval_authority,
        require_external_auth: this.policy.require_external_auth,
        require_telemetry: this.policy.require_telemetry,
      },
      provider_gate: {
        candidate_provider_count: providerRows.length,
        ready_provider_count: providerRows.filter((row) => row.ready).length,
        unresolved_provider_count: providerRows.filter((row) => !row.ready).length,
        providers: providerRows,
      },
      capabilities,
      domains: domainRows,
      global_blockers: sortedGlobalBlockers,
      warnings,
      ready_domain_count: readyDomainCount,
      partial_domain_count: partialDomainCount,
      blocked_domain_count: blockedDomainCount,
      state,
      next_actions: [...nextActions].sort(),
      readiness_claimed: false,
      execution: EXECUTION,
      authority: "audit_does_not_grant_dispatch_authority",
      credential_posture: "caller_owned_protected_input;opaque_runtime_handles_only",
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    });
    const report = { ...(body as unknown as Record<string, unknown>), readiness_digest: digestJsonSync(body) } as unknown as AutonomousDeploymentReadinessReport;
    if (new TextEncoder().encode(JSON.stringify(report)).byteLength > MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BYTES) throw new ArgumentError("deployment readiness report exceeds its bounded size");
    return clone(report);
  }
}

export function validateAutonomousDeploymentReadinessReport(raw: unknown): AutonomousDeploymentReadinessReport {
  return clone(validateReportShape(raw));
}

export function auditAutonomousDeploymentReadiness(input: AutonomousDeploymentReadinessInput, options: AutonomousDeploymentReadinessPolicy = {}): AutonomousDeploymentReadinessReport {
  return new AutonomousDeploymentReadinessAuditor(options).audit(input);
}
