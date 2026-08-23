import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import type { AutonomousEvidenceAcquirer, AutonomousEvidenceAcquisitionContext } from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterManifest,
} from "./autonomous-evidence-adapters.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only evidence adapter selection schemas. */
export const AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-selection/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA = "bioprism-typescript-autonomous-evidence-adapter-selection-row/0.1" as const;
export const AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES = ["lexicographic_adapter_id", "weighted_evidence"] as const;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_DOMAINS = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES = 256;
export const MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SIGNAL_BYTES = 64_000;

export type AutonomousEvidenceAdapterSelectionStrategy = typeof AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES[number];

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} must be between ${minimum} and ${maximum}`);
  return value;
}

function domains(value: readonly AutonomousDomainName[]): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_DOMAINS) throw new ArgumentError("evidence adapter selection domains are outside their bound");
  const result = value.map((domain, index) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError(`evidence adapter selection domain ${index} is unsupported`);
    return domain;
  });
  if (new Set(result).size !== result.length) throw new ArgumentError("evidence adapter selection domains contain duplicates");
  return result;
}

function capability(value: string | null | undefined): string | null {
  return value === undefined || value === null ? null : identifier("evidence adapter selection capability", value);
}

export interface AutonomousEvidenceAdapterSelectionSignal extends JsonObject {
  adapter_id: string;
  eligible: boolean;
  health: number;
  success_rate: number;
  evaluator_reward: number;
  latency_ms: number | null;
  cost_units: number | null;
  score: number;
}

export interface AutonomousEvidenceAdapterSelectionRowJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA;
  domain: AutonomousDomainName;
  status: "selected" | "missing";
  adapter_id: string | null;
  manifest_digest: string | null;
  candidate_ids: string[];
  candidate_manifest_digests: string[];
  candidate_scores: number[];
  candidate_eligible: boolean[];
  reason: string;
  retention: "metadata_only_manifest_and_health_evidence";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterSelectionPlanJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA;
  domains: AutonomousDomainName[];
  capability: string | null;
  registry_digest: string;
  rows: AutonomousEvidenceAdapterSelectionRowJSON[];
  strategy: AutonomousEvidenceAdapterSelectionStrategy;
  signal_digest: string | null;
  complete: boolean;
  plan_digest: string;
  execution: "planning_only;selection_does_not_authorize_source_dispatch";
  retention: "metadata_only_manifest_and_health_evidence";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceAdapterSelectionOptions {
  capability?: string | null;
  strategy?: AutonomousEvidenceAdapterSelectionStrategy;
  selectionSignals?: Readonly<Record<string, JsonObject>>;
  minScore?: number;
  minMargin?: number;
}

function normalizeSignal(adapterId: string, raw: JsonObject | undefined): AutonomousEvidenceAdapterSelectionSignal {
  if (raw !== undefined && (!isObject(raw) || Object.keys(raw).some((key) => !["eligible", "health", "success_rate", "evaluator_reward", "latency_ms", "cost_units"].includes(key)))) throw new ArgumentError(`evidence adapter selection signal for ${adapterId} contains unsupported fields`);
  const missing = raw === undefined;
  const eligible = missing ? false : raw.eligible === undefined ? true : raw.eligible;
  if (typeof eligible !== "boolean") throw new ArgumentError(`evidence adapter selection signal for ${adapterId} eligible must be boolean`);
  const health = finite(`evidence adapter selection signal ${adapterId} health`, missing ? 0 : raw.health ?? 0, 0, 1);
  const successRate = finite(`evidence adapter selection signal ${adapterId} success_rate`, missing ? 0 : raw.success_rate ?? health, 0, 1);
  const evaluatorReward = finite(`evidence adapter selection signal ${adapterId} evaluator_reward`, missing ? 0 : raw.evaluator_reward ?? 0, -1, 1);
  const latency = missing || raw.latency_ms === undefined || raw.latency_ms === null ? null : finite(`evidence adapter selection signal ${adapterId} latency_ms`, raw.latency_ms, 0, 86_400_000);
  const cost = missing || raw.cost_units === undefined || raw.cost_units === null ? null : finite(`evidence adapter selection signal ${adapterId} cost_units`, raw.cost_units, 0, 1_000_000);
  const latencyScore = latency === null ? 0.5 : 1 / (1 + latency / 1_000);
  const costScore = cost === null ? 0.5 : 1 / (1 + cost / 100);
  return {
    adapter_id: adapterId,
    eligible,
    health,
    success_rate: successRate,
    evaluator_reward: evaluatorReward,
    latency_ms: latency,
    cost_units: cost,
    score: Number((0.35 * health + 0.25 * successRate + 0.25 * ((evaluatorReward + 1) / 2) + 0.10 * latencyScore + 0.05 * costScore).toFixed(12)),
  };
}

function candidateManifests(registry: AutonomousEvidenceAdapterRegistry, domain: AutonomousDomainName, selectedCapability: string | null): AutonomousEvidenceAdapterManifest[] {
  return registry.manifests().filter((manifest) => manifest.domains.includes(domain) && (selectedCapability === null || manifest.capabilities.includes(selectedCapability))).slice(0, MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES);
}

export class AutonomousEvidenceAdapterSelectionRow {
  readonly domain: AutonomousDomainName;
  readonly status: "selected" | "missing";
  readonly adapter_id: string | null;
  readonly manifest_digest: string | null;
  readonly candidate_ids: string[];
  readonly candidate_manifest_digests: string[];
  readonly candidate_scores: number[];
  readonly candidate_eligible: boolean[];
  readonly reason: string;

  constructor(input: {
    domain: AutonomousDomainName;
    status: "selected" | "missing";
    adapter_id: string | null;
    manifest_digest: string | null;
    candidate_ids: readonly string[];
    candidate_manifest_digests: readonly string[];
    candidate_scores: readonly number[];
    candidate_eligible: readonly boolean[];
    reason: string;
  }) {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(input.domain)) throw new ArgumentError("evidence adapter selection row domain is unsupported");
    if (input.status !== "selected" && input.status !== "missing") throw new ArgumentError("evidence adapter selection row status is invalid");
    if (!Array.isArray(input.candidate_ids) || input.candidate_ids.length > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES) throw new ArgumentError("evidence adapter selection candidates exceed their bound");
    const ids = input.candidate_ids.map((id) => identifier("evidence adapter selection candidate id", id));
    const digests = input.candidate_manifest_digests.map((value) => digest("evidence adapter selection candidate manifest digest", value));
    const scores = input.candidate_scores.map((value) => finite("evidence adapter selection candidate score", value, 0, 1));
    const eligible = [...input.candidate_eligible];
    if (new Set(ids).size !== ids.length || ids.length !== digests.length || ids.length !== scores.length || ids.length !== eligible.length || eligible.some((value) => typeof value !== "boolean")) throw new ArgumentError("evidence adapter selection candidate metadata is misaligned");
    if (input.status === "selected") {
      if (input.adapter_id === null || input.manifest_digest === null) throw new ArgumentError("selected evidence adapter row requires adapter and manifest identities");
      const index = ids.indexOf(input.adapter_id);
      if (index < 0 || !eligible[index] || digests[index] !== input.manifest_digest) throw new ArgumentError("selected evidence adapter row does not match an eligible candidate");
    } else if (input.adapter_id !== null || input.manifest_digest !== null) throw new ArgumentError("missing evidence adapter row cannot select an adapter");
    this.domain = input.domain;
    this.status = input.status;
    this.adapter_id = input.adapter_id === null ? null : identifier("evidence adapter selection adapter_id", input.adapter_id);
    this.manifest_digest = input.manifest_digest === null ? null : digest("evidence adapter selection manifest_digest", input.manifest_digest);
    this.candidate_ids = ids;
    this.candidate_manifest_digests = digests;
    this.candidate_scores = scores;
    this.candidate_eligible = eligible;
    this.reason = identifier("evidence adapter selection reason", input.reason);
  }

  toJSON(): AutonomousEvidenceAdapterSelectionRowJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA,
      domain: this.domain,
      status: this.status,
      adapter_id: this.adapter_id,
      manifest_digest: this.manifest_digest,
      candidate_ids: [...this.candidate_ids],
      candidate_manifest_digests: [...this.candidate_manifest_digests],
      candidate_scores: [...this.candidate_scores],
      candidate_eligible: [...this.candidate_eligible],
      reason: this.reason,
      retention: "metadata_only_manifest_and_health_evidence",
      secret_material: "never_returned",
    };
  }
}

export class AutonomousEvidenceAdapterSelectionPlan {
  readonly domains: AutonomousDomainName[];
  readonly capability: string | null;
  readonly registry_digest: string;
  readonly rows: AutonomousEvidenceAdapterSelectionRow[];
  readonly strategy: AutonomousEvidenceAdapterSelectionStrategy;
  readonly signal_digest: string | null;

  constructor(input: { domains: readonly AutonomousDomainName[]; capability: string | null; registry_digest: string; rows: readonly AutonomousEvidenceAdapterSelectionRow[]; strategy?: AutonomousEvidenceAdapterSelectionStrategy; signal_digest?: string | null }) {
    const requested = domains(input.domains);
    const selectedCapability = capability(input.capability);
    const strategy = input.strategy ?? "lexicographic_adapter_id";
    if (!AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES.includes(strategy)) throw new ArgumentError("evidence adapter selection strategy is invalid");
    if (input.rows.length !== requested.length || input.rows.some((row, index) => !(row instanceof AutonomousEvidenceAdapterSelectionRow) || row.domain !== requested[index])) throw new ArgumentError("evidence adapter selection rows must align with domains");
    this.domains = requested;
    this.capability = selectedCapability;
    this.registry_digest = digest("evidence adapter selection registry_digest", input.registry_digest);
    this.rows = [...input.rows];
    this.strategy = strategy;
    this.signal_digest = input.signal_digest === undefined || input.signal_digest === null ? null : digest("evidence adapter selection signal_digest", input.signal_digest);
  }

  get complete(): boolean { return this.rows.every((row) => row.status === "selected"); }

  private payload(): JsonObject {
    return { schema: AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA, domains: [...this.domains], capability: this.capability, registry_digest: this.registry_digest, rows: this.rows.map((row) => row.toJSON()), strategy: this.strategy, signal_digest: this.signal_digest };
  }

  get plan_digest(): string { return digestJsonSync(this.payload()); }

  toJSON(): AutonomousEvidenceAdapterSelectionPlanJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA,
      domains: [...this.domains],
      capability: this.capability,
      registry_digest: this.registry_digest,
      rows: this.rows.map((row) => row.toJSON()),
      strategy: this.strategy,
      signal_digest: this.signal_digest,
      complete: this.complete,
      plan_digest: this.plan_digest,
      execution: "planning_only;selection_does_not_authorize_source_dispatch",
      retention: "metadata_only_manifest_and_health_evidence",
      secret_material: "never_returned",
    };
  }

  verify(registry: AutonomousEvidenceAdapterRegistry): this {
    if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence adapter selection verification requires a typed registry");
    if (this.registry_digest !== registry.toJSON().registry_digest) throw new ArgumentError("evidence adapter selection registry is stale or tampered");
    for (const row of this.rows) {
      const candidates = candidateManifests(registry, row.domain, this.capability);
      if (candidates.map((candidate) => candidate.adapter_id).join("\u0000") !== row.candidate_ids.join("\u0000") || candidates.map((candidate) => candidate.manifest_digest).join("\u0000") !== row.candidate_manifest_digests.join("\u0000")) throw new ArgumentError("evidence adapter selection candidate set changed");
    }
    return this;
  }

  static fromJSON(value: unknown): AutonomousEvidenceAdapterSelectionPlan {
    if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SCHEMA || !Array.isArray(value.domains) || !Array.isArray(value.rows)) throw new ArgumentError("evidence adapter selection plan is malformed");
    if (value.execution !== "planning_only;selection_does_not_authorize_source_dispatch" || value.retention !== "metadata_only_manifest_and_health_evidence" || value.secret_material !== "never_returned") throw new ArgumentError("evidence adapter selection plan retention is invalid");
    if (value.complete !== value.rows.every((row) => isObject(row) && row.status === "selected")) throw new ArgumentError("evidence adapter selection plan completeness is invalid");
    const rows = value.rows.map((raw) => {
      if (!isObject(raw)) throw new ArgumentError("evidence adapter selection row is malformed");
      if (raw.schema !== AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_ROW_SCHEMA || raw.retention !== "metadata_only_manifest_and_health_evidence" || raw.secret_material !== "never_returned") throw new ArgumentError("evidence adapter selection row retention is invalid");
      return new AutonomousEvidenceAdapterSelectionRow({ domain: raw.domain as AutonomousDomainName, status: raw.status as "selected" | "missing", adapter_id: (raw.adapter_id as string | null) ?? null, manifest_digest: (raw.manifest_digest as string | null) ?? null, candidate_ids: raw.candidate_ids as string[], candidate_manifest_digests: raw.candidate_manifest_digests as string[], candidate_scores: raw.candidate_scores as number[], candidate_eligible: raw.candidate_eligible as boolean[], reason: raw.reason as string });
    });
    const plan = new AutonomousEvidenceAdapterSelectionPlan({ domains: value.domains as AutonomousDomainName[], capability: (value.capability as string | null) ?? null, registry_digest: value.registry_digest as string, rows, strategy: value.strategy as AutonomousEvidenceAdapterSelectionStrategy, signal_digest: (value.signal_digest as string | null) ?? null });
    if (value.plan_digest !== plan.plan_digest) throw new ArgumentError("evidence adapter selection plan digest is invalid");
    return plan;
  }
}

/** Deterministic, signal-driven selector; selection remains separate from source authorization. */
export class AutonomousEvidenceAdapterSelector {
  constructor(readonly registry: AutonomousEvidenceAdapterRegistry) {
    if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence adapter selector requires a typed registry");
  }

  selectForDomains(requestedDomains: readonly AutonomousDomainName[], options: AutonomousEvidenceAdapterSelectionOptions = {}): AutonomousEvidenceAdapterSelectionPlan {
    const requested = domains(requestedDomains);
    const selectedCapability = capability(options.capability);
    const strategy = options.strategy ?? "lexicographic_adapter_id";
    if (!AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_STRATEGIES.includes(strategy)) throw new ArgumentError("evidence adapter selector strategy is invalid");
    if (strategy === "lexicographic_adapter_id" && options.selectionSignals !== undefined) throw new ArgumentError("lexicographic evidence adapter selection cannot consume signals");
    if (strategy === "weighted_evidence" && options.selectionSignals === undefined) throw new ArgumentError("weighted evidence adapter selection requires explicit signals");
    const minScore = finite("evidence adapter selection minScore", options.minScore ?? 0, 0, 1);
    const minMargin = finite("evidence adapter selection minMargin", options.minMargin ?? 0, 0, 1);
    const signals = new Map<string, AutonomousEvidenceAdapterSelectionSignal>();
    if (options.selectionSignals !== undefined) {
      if (!isObject(options.selectionSignals)) throw new ArgumentError("evidence adapter selection signals are malformed");
      if (Object.keys(options.selectionSignals).length > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_CANDIDATES) throw new ArgumentError("evidence adapter selection signals exceed their bound");
      for (const [adapterId, raw] of Object.entries(options.selectionSignals)) {
        const id = identifier("evidence adapter selection signal adapter_id", adapterId);
        if (!this.registry.manifests().some((manifest) => manifest.adapter_id === id)) throw new ArgumentError(`evidence adapter selection signal names an unknown adapter: ${id}`);
        const safe = JSON.stringify(raw);
        if (typeof safe !== "string" || new TextEncoder().encode(safe).byteLength > MAX_AUTONOMOUS_EVIDENCE_ADAPTER_SELECTION_SIGNAL_BYTES) throw new ArgumentError("evidence adapter selection signal exceeds its bound");
        signals.set(id, normalizeSignal(id, raw));
      }
    }
    const signalDigest = strategy === "weighted_evidence" ? digestJsonSync([...signals.values()].sort((left, right) => left.adapter_id.localeCompare(right.adapter_id))) : null;
    const rows = requested.map((domain) => {
      const candidates = candidateManifests(this.registry, domain, selectedCapability);
      const descriptors = candidates.map((candidate) => strategy === "weighted_evidence" ? signals.get(candidate.adapter_id) ?? normalizeSignal(candidate.adapter_id, undefined) : normalizeSignal(candidate.adapter_id, { eligible: true }));
      const eligible = descriptors.map((descriptor) => strategy === "weighted_evidence" ? descriptor.eligible : true);
      const scores = descriptors.map((descriptor) => strategy === "weighted_evidence" ? descriptor.score : 0);
      const eligibleIndexes = eligible.map((value, index) => value ? index : -1).filter((index) => index >= 0);
      const ranked = [...eligibleIndexes].sort((left, right) => scores[right]! - scores[left]! || candidates[left]!.adapter_id.localeCompare(candidates[right]!.adapter_id));
      const topIndex = ranked[0];
      const secondScore = ranked.length > 1 ? scores[ranked[1]!]! : 0;
      const topScore = topIndex === undefined ? 0 : scores[topIndex]!;
      const margin = topIndex === undefined ? 0 : topScore - secondScore;
      const reason = topIndex === undefined
        ? candidates.length ? "no_eligible_adapter" : "no_matching_adapter"
        : topScore < minScore ? "selection_below_min_score"
          : margin < minMargin ? "insufficient_selection_margin"
            : strategy;
      const selected = reason === strategy && topIndex !== undefined ? candidates[topIndex] : undefined;
      return new AutonomousEvidenceAdapterSelectionRow({ domain, status: selected ? "selected" : "missing", adapter_id: selected?.adapter_id ?? null, manifest_digest: selected?.manifest_digest ?? null, candidate_ids: candidates.map((candidate) => candidate.adapter_id), candidate_manifest_digests: candidates.map((candidate) => candidate.manifest_digest), candidate_scores: scores, candidate_eligible: eligible, reason });
    });
    return new AutonomousEvidenceAdapterSelectionPlan({ domains: requested, capability: selectedCapability, registry_digest: this.registry.toJSON().registry_digest, rows, strategy, signal_digest: signalDigest });
  }

  selectAdaptiveForDomains(domainsToSelect: readonly AutonomousDomainName[], selectionSignals: Readonly<Record<string, JsonObject>>, options: { capability?: string | null; minScore?: number; minMargin?: number } = {}): AutonomousEvidenceAdapterSelectionPlan {
    return this.selectForDomains(domainsToSelect, { ...options, strategy: "weighted_evidence", selectionSignals });
  }

  createAcquirerFromSelection(plan: AutonomousEvidenceAdapterSelectionPlan | unknown): AutonomousEvidenceAcquirer {
    const typedPlan = plan instanceof AutonomousEvidenceAdapterSelectionPlan ? plan : AutonomousEvidenceAdapterSelectionPlan.fromJSON(plan);
    typedPlan.verify(this.registry);
    const adapterIds: Partial<Record<AutonomousDomainName, string>> = {};
    for (const row of typedPlan.rows) {
      if (row.status !== "selected" || row.adapter_id === null) throw new ArgumentError(`evidence adapter selection is incomplete for ${row.domain}`);
      adapterIds[row.domain] = row.adapter_id;
    }
    const acquirer = this.registry.createAcquirer({ adapterIdForDomain: adapterIds });
    return {
      acquire: (context: AutonomousEvidenceAcquisitionContext) => acquirer.acquire(context),
    };
  }
}
