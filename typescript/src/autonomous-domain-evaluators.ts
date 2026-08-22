import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { digestJsonSync } from "./tooling.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import type { JsonObject } from "./types.js";

/** Shared value-only evaluator schema used by the Python and TypeScript adapters. */
export const AUTONOMOUS_VALUE_EVALUATOR_SCHEMA = "bioprism-brain-domain-evaluator/0.1" as const;
export const AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS = 64;
export const AUTONOMOUS_VALUE_EVALUATOR_MAX_REFERENCES = 64;
export const AUTONOMOUS_VALUE_EVALUATOR_MAX_LIMITATIONS = 32;
export const AUTONOMOUS_VALUE_EVALUATOR_MAX_TEXT_BYTES = 256;

const SAFE_SIGNAL = /^[A-Za-z][A-Za-z0-9_.-]{0,127}$/;
const SHA256 = /^[0-9a-f]{64}$/;
const SECRET_KEYS = new Set([
  "accesskey",
  "accesstoken",
  "apikey",
  "authorization",
  "bearer",
  "credential",
  "credentials",
  "password",
  "privatekey",
  "refreshtoken",
  "secret",
  "secretkey",
  "token",
]);

const PRIVATE_RETENTION = "value_only;task_prompt_response_credentials_and_evidence_not_retained" as const;

export interface AutonomousValueEvaluatorProfile extends JsonObject {
  schema: typeof AUTONOMOUS_VALUE_EVALUATOR_SCHEMA;
  domain: AutonomousDomainName | string;
  evaluator_id: string;
  evaluator_version: string;
  required_signals: string[];
  signal_weights: Record<string, number>;
  pass_threshold: number;
  accepted_evidence_domains: string[];
  execution: "caller_declared_signal_scoring_only";
}

export interface AutonomousValueEvaluationEvidence extends JsonObject {
  schema: typeof AUTONOMOUS_VALUE_EVALUATOR_SCHEMA;
  domain: string;
  capability: string;
  risk_class: string;
  signals: Record<string, number>;
  references: string[];
  limitations: string[];
  stage_plan_digest: string | null;
  capability_contract_digests: string[];
  selected_tool_names: string[];
  retention: "value_only_digests_and_signal_scores";
}

export interface AutonomousValueEvaluationInput extends JsonObject {
  evidence?: JsonObject | null;
  context?: JsonObject | null;
}

export interface AutonomousValueEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_VALUE_EVALUATOR_SCHEMA;
  evaluator_id: string;
  evaluator_version: string;
  domain: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  failure_class: string | null;
  feedback_digest: string | null;
  evidence_digest: string | null;
  replan_requested: boolean;
  replan_instruction: string | null;
  missing_signals: string[];
  below_threshold_signals: string[];
  evaluator_authority: "caller_declared_signal_scoring_only";
  retention: typeof PRIVATE_RETENTION | "value_only;composite_identity_and_domain_routing_only";
  secret_material: "never_returned";
  evaluation_digest: string;
}

function boundedText(name: string, value: unknown, maximum = AUTONOMOUS_VALUE_EVALUATOR_MAX_TEXT_BYTES): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000")) throw new ArgumentError(`${name} must be a non-empty string`);
  if (new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} exceeds its bounded size`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  return boundedText(name, value);
}

function boundedDigest(name: string, value: unknown, nullable = false): string | null {
  if (value === null && nullable) return null;
  if (typeof value !== "string" || !SHA256.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedSequence(name: string, value: unknown, maximum: number): unknown[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must be a bounded sequence of at most ${maximum} entries`);
  return value;
}

function assertAutonomousDomain(name: string, value: unknown): string {
  const domain = boundedText(name, value);
  if (!(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(domain)) throw new ArgumentError(`${name} is not a built-in autonomous domain`);
  return domain;
}

function assertSafeSignal(name: string, value: unknown): string {
  const signal = boundedText(name, value);
  if (!SAFE_SIGNAL.test(signal)) throw new ArgumentError(`${name} is not a safe signal identifier`);
  return signal;
}

function assertSecretFree(value: unknown, depth = 0): void {
  if (depth > 12) throw new ArgumentError("domain evaluator evidence exceeds its nesting bound");
  if (Array.isArray(value)) {
    for (const item of value) assertSecretFree(item, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[^a-z]/g, "");
    if (SECRET_KEYS.has(normalized)) throw new ArgumentError("domain evaluator evidence contains forbidden secret-shaped fields");
    assertSecretFree(child, depth + 1);
  }
}

function assertKnownKeys(value: Record<string, unknown>, allowed: ReadonlySet<string>, label: string): void {
  const unsupported = Object.keys(value).filter((key) => !allowed.has(key));
  if (unsupported.length) throw new ArgumentError(`${label} contains unsupported fields`);
}

function normalizeProfile(value: AutonomousValueEvaluatorProfile): AutonomousValueEvaluatorProfile {
  if (!isObject(value)) throw new ArgumentError("domain evaluator profile must be an object");
  assertKnownKeys(value, new Set(["schema", "domain", "evaluator_id", "evaluator_version", "required_signals", "signal_weights", "pass_threshold", "accepted_evidence_domains", "execution"]), "domain evaluator profile");
  if (value.schema !== AUTONOMOUS_VALUE_EVALUATOR_SCHEMA) throw new ArgumentError("domain evaluator profile schema is invalid");
  const domain = assertAutonomousDomain("domain evaluator domain", value.domain);
  const evaluatorId = boundedIdentifier("domain evaluator evaluator_id", value.evaluator_id);
  const evaluatorVersion = boundedIdentifier("domain evaluator evaluator_version", value.evaluator_version);
  const requiredRaw = boundedSequence("domain evaluator required_signals", value.required_signals, AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS);
  if (!requiredRaw.length) throw new ArgumentError("domain evaluator required_signals must contain at least one entry");
  const requiredSignals = requiredRaw.map((signal) => assertSafeSignal("domain evaluator required signal", signal));
  if (new Set(requiredSignals).size !== requiredSignals.length) throw new ArgumentError("domain evaluator required_signals must be unique");
  if (!isObject(value.signal_weights) || !Object.keys(value.signal_weights).length || Object.keys(value.signal_weights).length > AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS) throw new ArgumentError("domain evaluator signal_weights must contain 1..64 entries");
  const signalWeights: Record<string, number> = {};
  let totalWeight = 0;
  for (const [rawSignal, rawWeight] of Object.entries(value.signal_weights)) {
    const signal = assertSafeSignal("domain evaluator weighted signal", rawSignal);
    if (typeof rawWeight !== "number" || !Number.isFinite(rawWeight) || rawWeight <= 0) throw new ArgumentError("domain evaluator signal weights must be finite positive numbers");
    signalWeights[signal] = rawWeight;
    totalWeight += rawWeight;
  }
  if (!requiredSignals.some((signal) => signal in signalWeights) || !Number.isFinite(totalWeight) || totalWeight <= 0) throw new ArgumentError("domain evaluator must weight at least one required signal");
  if (typeof value.pass_threshold !== "number" || !Number.isFinite(value.pass_threshold) || value.pass_threshold < 0 || value.pass_threshold > 1) throw new ArgumentError("domain evaluator pass_threshold must be within [0, 1]");
  const acceptedRaw = boundedSequence("domain evaluator accepted_evidence_domains", value.accepted_evidence_domains, AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS);
  const accepted = acceptedRaw.map((candidate) => boundedText("domain evaluator accepted evidence domain", candidate));
  if (new Set(accepted).size !== accepted.length || accepted.includes(domain)) throw new ArgumentError("domain evaluator accepted evidence domains must be unique and distinct");
  if (value.execution !== "caller_declared_signal_scoring_only") throw new ArgumentError("domain evaluator execution authority is invalid");
  return {
    schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
    domain,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    required_signals: [...requiredSignals],
    signal_weights: { ...signalWeights },
    pass_threshold: value.pass_threshold,
    accepted_evidence_domains: [...accepted],
    execution: "caller_declared_signal_scoring_only",
  };
}

function normalizeEvidence(value: unknown): AutonomousValueEvaluationEvidence {
  if (!isObject(value)) throw new ArgumentError("domain evaluation evidence must be an object");
  assertSecretFree(value);
  assertKnownKeys(value, new Set([
    "schema", "domain", "capability", "risk_class", "signals", "references", "limitations", "retention",
    "workflow_id", "workflow_digest", "stage_id", "required_signals", "stage_plan_digest",
    "capability_contract_digests", "selected_tool_names",
  ]), "domain evaluation evidence");
  if (value.schema !== undefined && value.schema !== AUTONOMOUS_VALUE_EVALUATOR_SCHEMA) throw new ArgumentError("domain evaluation evidence schema is invalid");
  if (value.retention !== undefined && value.retention !== "value_only_digests_and_signal_scores") throw new ArgumentError("domain evaluation evidence retention is invalid");
  const domain = boundedText("domain evidence domain", value.domain);
  const capability = boundedText("domain evidence capability", value.capability);
  const riskClass = boundedText("domain evidence risk_class", value.risk_class);
  if (!isObject(value.signals) || !Object.keys(value.signals).length || Object.keys(value.signals).length > AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS) throw new ArgumentError("domain evidence signals must contain 1..64 entries");
  const signals: Record<string, number> = {};
  for (const [rawSignal, rawValue] of Object.entries(value.signals)) {
    const signal = assertSafeSignal("domain evidence signal", rawSignal);
    const numeric = typeof rawValue === "boolean" ? (rawValue ? 1 : 0) : rawValue;
    if (typeof numeric !== "number" || !Number.isFinite(numeric) || numeric < 0 || numeric > 1) throw new ArgumentError("domain evidence signal values must be finite numbers within [0, 1]");
    signals[signal] = numeric;
  }
  const references = boundedSequence("domain evidence references", value.references ?? [], AUTONOMOUS_VALUE_EVALUATOR_MAX_REFERENCES).map((reference) => {
    const digest = boundedText("domain evidence reference", reference);
    if (!SHA256.test(digest)) throw new ArgumentError("domain evidence references must be lowercase SHA-256 digests");
    return digest;
  });
  const limitations = boundedSequence("domain evidence limitations", value.limitations ?? [], AUTONOMOUS_VALUE_EVALUATOR_MAX_LIMITATIONS).map((limitation) => boundedText("domain evidence limitation", limitation));
  const stagePlanDigest = value.stage_plan_digest === undefined ? null : boundedDigest("domain evidence stage_plan_digest", value.stage_plan_digest, true);
  const capabilityContractDigests = boundedSequence("domain evidence capability_contract_digests", value.capability_contract_digests ?? [], AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS).map((digest) => boundedDigest("domain evidence capability contract digest", digest) as string);
  const selectedToolNames = boundedSequence("domain evidence selected_tool_names", value.selected_tool_names ?? [], AUTONOMOUS_VALUE_EVALUATOR_MAX_SIGNALS).map((name) => boundedText("domain evidence selected tool name", name));
  return {
    schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
    domain,
    capability,
    risk_class: riskClass,
    signals: Object.fromEntries(Object.entries(signals).sort(([left], [right]) => left.localeCompare(right))),
    references: [...references],
    limitations: [...limitations],
    stage_plan_digest: stagePlanDigest,
    capability_contract_digests: [...capabilityContractDigests],
    selected_tool_names: [...selectedToolNames],
    retention: "value_only_digests_and_signal_scores",
  };
}

function evidenceDigest(evidence: AutonomousValueEvaluationEvidence): string {
  return digestJsonSync(evidence);
}

function failureResult(evaluatorId: string, evaluatorVersion: string, domain: string, failureClass: string, instruction: string): AutonomousValueEvaluation {
  const descriptor = {
    schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
    evaluator_id: evaluatorId,
    evaluator_version: evaluatorVersion,
    domain,
    reward: 0,
    passed: false,
    failed: true,
    failure_class: failureClass,
    feedback_digest: null,
    evidence_digest: null,
    replan_requested: true,
    replan_instruction: boundedText("domain evaluator replan instruction", instruction),
    missing_signals: [],
    below_threshold_signals: [],
    evaluator_authority: "caller_declared_signal_scoring_only" as const,
    retention: PRIVATE_RETENTION,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, evaluation_digest: digestJsonSync(descriptor) };
}

export class AutonomousValueEvaluatorAdapter {
  readonly profile: AutonomousValueEvaluatorProfile;
  readonly evaluatorId: string;
  readonly evaluatorVersion: string;

  constructor(profile: AutonomousValueEvaluatorProfile) {
    this.profile = normalizeProfile(profile);
    this.evaluatorId = this.profile.evaluator_id;
    this.evaluatorVersion = this.profile.evaluator_version;
  }

  normalizeEvidence(value: unknown): AutonomousValueEvaluationEvidence {
    const evidence = normalizeEvidence(value);
    const accepted = new Set([this.profile.domain, ...this.profile.accepted_evidence_domains]);
    if (!accepted.has(evidence.domain)) throw new ArgumentError(`domain evaluator ${this.profile.domain} cannot evaluate ${evidence.domain} evidence`);
    return evidence;
  }

  assessValueOnlyInput(value: unknown): AutonomousValueEvaluation {
    if (!isObject(value)) throw new ArgumentError("domain evaluation input must be an object");
    const rawEvidence = value.evidence;
    if (!isObject(rawEvidence)) return failureResult(this.evaluatorId, this.evaluatorVersion, this.profile.domain, "missing_domain_evidence", `Collect bounded ${this.profile.domain} evaluation signals.`);
    const evidence = this.normalizeEvidence(rawEvidence);
    const missing: string[] = [];
    const belowThreshold: string[] = [];
    for (const signal of this.profile.required_signals) {
      const score = evidence.signals[signal];
      if (score === undefined) missing.push(signal);
      else if (score < this.profile.pass_threshold) belowThreshold.push(signal);
    }
    let weightedTotal = 0;
    let observedWeight = 0;
    for (const [signal, weight] of Object.entries(this.profile.signal_weights)) {
      const score = evidence.signals[signal];
      if (score === undefined) continue;
      weightedTotal += score * weight;
      observedWeight += weight;
    }
    const reward = observedWeight === 0 ? 0 : Number((weightedTotal / observedWeight).toFixed(12));
    const failed = missing.length > 0 || belowThreshold.length > 0 || reward < this.profile.pass_threshold;
    const digest = evidenceDigest(evidence);
    const gaps = [...new Set([...missing, ...belowThreshold])];
    const descriptor = {
      schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
      evaluator_id: this.evaluatorId,
      evaluator_version: this.evaluatorVersion,
      domain: this.profile.domain,
      reward,
      passed: !failed,
      failed,
      failure_class: failed ? "domain_evidence_gate" : null,
      feedback_digest: digest,
      evidence_digest: digest,
      replan_requested: failed,
      replan_instruction: failed ? boundedText("domain evaluator replan instruction", `Address bounded ${this.profile.domain} evaluation gaps: ${gaps.length ? gaps.join(", ") : "the weighted quality threshold"}.`) : null,
      missing_signals: [...missing].sort(),
      below_threshold_signals: [...belowThreshold].sort(),
      evaluator_authority: "caller_declared_signal_scoring_only" as const,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    return { ...descriptor, evaluation_digest: digestJsonSync(descriptor) };
  }

  assess(value: AutonomousValueEvaluationInput): AutonomousValueEvaluation {
    return this.assessValueOnlyInput(value);
  }

  toRewardInput(value: AutonomousValueEvaluationInput): AutonomousEvaluatorRewardInput {
    const evaluation = this.assess(value);
    return {
      evaluator_id: evaluation.evaluator_id,
      evaluator_version: evaluation.evaluator_version,
      reward: evaluation.reward,
      passed: evaluation.passed,
      failed: evaluation.failed,
      feedback_digest: evaluation.feedback_digest,
      evidence_digest: evaluation.evidence_digest,
      failure_class: evaluation.failure_class,
    };
  }

  catalogueEntry(): AutonomousValueEvaluatorProfile {
    return {
      ...this.profile,
      required_signals: [...this.profile.required_signals],
      signal_weights: { ...this.profile.signal_weights },
      accepted_evidence_domains: [...this.profile.accepted_evidence_domains],
    };
  }
}

export class AutonomousCompositeValueEvaluator {
  readonly evaluatorId: string;
  readonly evaluatorVersion: string;
  readonly evaluators: ReadonlyMap<string, AutonomousValueEvaluatorAdapter>;

  constructor(options: { evaluators: ReadonlyMap<string, AutonomousValueEvaluatorAdapter> | Record<string, AutonomousValueEvaluatorAdapter>; evaluatorId?: string; evaluatorVersion?: string }) {
    if (!isObject(options) || !options.evaluators) throw new ArgumentError("composite domain evaluators must be supplied");
    const entries = options.evaluators instanceof Map ? [...options.evaluators.entries()] : Object.entries(options.evaluators);
    if (!entries.length || entries.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("composite domain evaluators must contain 1..12 entries");
    const normalized = new Map<string, AutonomousValueEvaluatorAdapter>();
    for (const [domain, evaluator] of entries) {
      const boundedDomain = assertAutonomousDomain("composite domain evaluator domain", domain);
      if (!(evaluator instanceof AutonomousValueEvaluatorAdapter)) throw new ArgumentError("composite domain evaluator values must be adapters");
      if (normalized.has(boundedDomain)) throw new ArgumentError(`duplicate composite domain evaluator: ${boundedDomain}`);
      normalized.set(boundedDomain, evaluator);
    }
    this.evaluators = normalized;
    this.evaluatorId = boundedIdentifier("composite evaluator_id", options.evaluatorId ?? "composite-domain-quality");
    this.evaluatorVersion = boundedIdentifier("composite evaluator_version", options.evaluatorVersion ?? "1");
  }

  private resolveDomain(value: Record<string, unknown>): string | null {
    const context = value.context;
    const evidence = value.evidence;
    const contextDomain = isObject(context) && typeof context.domain === "string" ? context.domain : null;
    const evidenceDomain = isObject(evidence) && typeof evidence.domain === "string" ? evidence.domain : null;
    return contextDomain ?? evidenceDomain;
  }

  assessValueOnlyInput(value: unknown): AutonomousValueEvaluation {
    if (!isObject(value)) throw new ArgumentError("composite domain evaluation input must be an object");
    const domain = this.resolveDomain(value);
    const evaluator = domain === null ? undefined : this.evaluators.get(domain);
    if (!evaluator) return failureResult(this.evaluatorId, this.evaluatorVersion, domain ?? "unknown", "unmapped_domain_evaluator", "Provide an explicit reviewed evaluator for the routed domain.");
    const nested = evaluator.assessValueOnlyInput(value);
    const descriptor = {
      ...nested,
      evaluator_id: this.evaluatorId,
      evaluator_version: this.evaluatorVersion,
      domain: domain ?? "unknown",
      feedback_digest: nested.feedback_digest,
      evidence_digest: nested.evidence_digest,
      retention: "value_only;composite_identity_and_domain_routing_only" as const,
    };
    const { evaluation_digest: _nestedDigest, ...withoutNestedDigest } = descriptor;
    return { ...withoutNestedDigest, evaluation_digest: digestJsonSync(withoutNestedDigest) };
  }

  assess(value: AutonomousValueEvaluationInput): AutonomousValueEvaluation {
    return this.assessValueOnlyInput(value);
  }

  toRewardInput(value: AutonomousValueEvaluationInput): AutonomousEvaluatorRewardInput {
    const evaluation = this.assess(value);
    return {
      evaluator_id: evaluation.evaluator_id,
      evaluator_version: evaluation.evaluator_version,
      reward: evaluation.reward,
      passed: evaluation.passed,
      failed: evaluation.failed,
      feedback_digest: evaluation.feedback_digest,
      evidence_digest: evaluation.evidence_digest,
      failure_class: evaluation.failure_class,
    };
  }

  catalogueEntry(): JsonObject {
    return {
      schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
      evaluator_id: this.evaluatorId,
      evaluator_version: this.evaluatorVersion,
      domains: [...this.evaluators.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([domain, evaluator]) => ({ domain, evaluator_id: evaluator.evaluatorId, evaluator_version: evaluator.evaluatorVersion })),
      execution: "value_only_domain_routing",
      retention: "evaluator_id_version_and_domain_keys_only",
    };
  }

  static fromRegistry(registry: AutonomousValueEvaluatorRegistry, domains: readonly AutonomousDomainName[] = AUTONOMOUS_DOMAIN_NAMES, options: { evaluatorId?: string; evaluatorVersion?: string } = {}): AutonomousCompositeValueEvaluator {
    if (!(registry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("composite evaluator registry must be an AutonomousValueEvaluatorRegistry");
    if (!Array.isArray(domains) || !domains.length || domains.length > AUTONOMOUS_DOMAIN_NAMES.length || new Set(domains).size !== domains.length) throw new ArgumentError("composite evaluator domains must be a unique non-empty bounded sequence");
    return new AutonomousCompositeValueEvaluator({
      evaluators: new Map(domains.map((domain) => [domain, registry.resolveForAutonomousDomain(domain)])),
      evaluatorId: options.evaluatorId,
      evaluatorVersion: options.evaluatorVersion,
    });
  }
}

export class AutonomousValueEvaluatorRegistry {
  private readonly adapters = new Map<string, AutonomousValueEvaluatorAdapter>();

  constructor(adapters: readonly AutonomousValueEvaluatorAdapter[] = []) {
    for (const adapter of adapters) this.register(adapter);
  }

  register(adapter: AutonomousValueEvaluatorAdapter): void {
    if (!(adapter instanceof AutonomousValueEvaluatorAdapter)) throw new ArgumentError("registry entries must be AutonomousValueEvaluatorAdapter values");
    if (this.adapters.has(adapter.profile.domain)) throw new ArgumentError(`domain evaluator is already registered: ${adapter.profile.domain}`);
    this.adapters.set(adapter.profile.domain, adapter);
  }

  resolve(domain: string): AutonomousValueEvaluatorAdapter {
    const normalized = assertAutonomousDomain("evaluator registry domain", domain);
    const adapter = this.adapters.get(normalized);
    if (!adapter) throw new ArgumentError(`no domain evaluator is registered for ${normalized}`);
    return adapter;
  }

  resolveForAutonomousDomain(domain: AutonomousDomainName): AutonomousValueEvaluatorAdapter {
    return this.resolve(domain);
  }

  resolveForReplay(domain: AutonomousDomainName, identity: { evaluator_id: string; evaluator_version: string }): AutonomousValueEvaluatorAdapter {
    const adapter = this.resolve(domain);
    const evaluatorId = boundedIdentifier("replay evaluator_id", identity.evaluator_id);
    const evaluatorVersion = boundedIdentifier("replay evaluator_version", identity.evaluator_version);
    if (adapter.evaluatorId !== evaluatorId || adapter.evaluatorVersion !== evaluatorVersion) throw new ArgumentError(`replay evaluator identity does not match the registered ${domain} evaluator`);
    return adapter;
  }

  catalogue(): AutonomousValueEvaluatorProfile[] {
    return [...this.adapters.values()].sort((left, right) => left.profile.domain.localeCompare(right.profile.domain)).map((adapter) => adapter.catalogueEntry());
  }

  static withBuiltinProfiles(): AutonomousValueEvaluatorRegistry {
    return new AutonomousValueEvaluatorRegistry(builtinAutonomousValueEvaluatorProfiles().map((profile) => new AutonomousValueEvaluatorAdapter(profile)));
  }
}

function profile(domain: AutonomousDomainName, evaluatorId: string, requiredSignals: string[], signalWeights: Record<string, number>, acceptedEvidenceDomains: string[] = []): AutonomousValueEvaluatorProfile {
  return {
    schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
    domain,
    evaluator_id: evaluatorId,
    evaluator_version: "1",
    required_signals: requiredSignals,
    signal_weights: signalWeights,
    pass_threshold: 1,
    accepted_evidence_domains: acceptedEvidenceDomains,
    execution: "caller_declared_signal_scoring_only",
  };
}

/** Reviewed conservative value-only contracts covering every built-in autonomous domain. */
export function builtinAutonomousValueEvaluatorProfiles(): AutonomousValueEvaluatorProfile[] {
  return [
    profile("coding", "autonomous-coding-quality", ["schema_valid", "tests_passed", "evidence_complete"], { schema_valid: 1, tests_passed: 2, evidence_complete: 1 }, ["engineering"]),
    profile("browser", "autonomous-browser-quality", ["evidence_traceable", "source_comparison", "freshness_reported", "claim_scope_respected"], { evidence_traceable: 2, source_comparison: 1, freshness_reported: 1, claim_scope_respected: 2 }, ["research"]),
    profile("data", "autonomous-data-quality", ["schema_valid", "lineage_complete", "quality_gate_passed"], { schema_valid: 1, lineage_complete: 2, quality_gate_passed: 2 }),
    profile("science", "autonomous-science-quality", ["evidence_traceable", "uncertainty_reported", "claim_scope_respected", "reproducible"], { evidence_traceable: 2, uncertainty_reported: 1, claim_scope_respected: 2, reproducible: 1 }, ["research"]),
    profile("biomedical", "autonomous-biomedical-boundary", ["boundary_compliant", "provenance_complete", "human_review_ready"], { boundary_compliant: 3, provenance_complete: 2, human_review_ready: 2 }),
    profile("neuroscience", "autonomous-neuroscience-quality", ["signal_quality_reported", "preprocessing_traceable", "claim_scope_respected", "reproducible"], { signal_quality_reported: 2, preprocessing_traceable: 2, claim_scope_respected: 2, reproducible: 1 }, ["biomedical"]),
    profile("operations", "autonomous-operations-quality", ["safety_gate_passed", "approval_complete", "rollback_plan_present"], { safety_gate_passed: 3, approval_complete: 2, rollback_plan_present: 2, observability_ready: 1 }),
    profile("enterprise", "autonomous-enterprise-quality", ["ownership_complete", "policy_aligned", "approval_complete", "decision_traceable"], { ownership_complete: 2, policy_aligned: 2, approval_complete: 2, decision_traceable: 1 }, ["operations"]),
    profile("multi_agent", "autonomous-multi-agent-quality", ["contract_complete", "attribution_complete", "conflict_resolved", "approval_complete"], { contract_complete: 2, attribution_complete: 2, conflict_resolved: 2, approval_complete: 1 }, ["engineering"]),
    profile("multimodal", "autonomous-multimodal-quality", ["modality_inventory_complete", "alignment_valid", "uncertainty_reported", "claim_scope_respected"], { modality_inventory_complete: 2, alignment_valid: 2, uncertainty_reported: 1, claim_scope_respected: 2 }, ["research"]),
    profile("cross_domain", "autonomous-cross-domain-quality", ["route_traceable", "evidence_aligned", "attribution_complete", "uncertainty_reported"], { route_traceable: 1, evidence_aligned: 2, attribution_complete: 2, uncertainty_reported: 1 }, ["research"]),
    profile("evaluation", "autonomous-evaluation-quality", ["rubric_frozen", "replay_reproducible", "evaluator_independent", "evidence_complete"], { rubric_frozen: 2, replay_reproducible: 2, evaluator_independent: 2, evidence_complete: 1 }, ["engineering"]),
  ];
}
