/**
 * Evaluator-gated consolidation for autonomous episodic memory.
 *
 * Episodic recall is not permission to generalize. This boundary aggregates only explicit
 * evaluator-backed observations, keeps portable lessons separate from domain-local lessons,
 * marks competing variants as conflicts, and persists lesson/evidence/episode digests rather than
 * prompts, tasks, provider output, credentials, or tool arguments. A caller-owned resolver may
 * materialize a stable lesson into a transient prompt after recall has passed the scope gate.
 */

import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";

export const AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA = "bioprism-typescript-autonomous-memory-consolidation/0.1" as const;
export const AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA = "bioprism-typescript-autonomous-memory-consolidation-lesson/0.1" as const;
export const AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-memory-consolidation-snapshot/0.1" as const;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS = 16_384;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS = 4_096;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS = 32;
export const MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES = 8_000_000;

const RETENTION = "metadata_only_lesson_evidence_and_episode_digests_no_text_or_payloads" as const;
const SECRET_MATERIAL = "never_returned" as const;
const ID = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$/;
const DOMAINS = [...AUTONOMOUS_DOMAIN_NAMES] as AutonomousDomainName[];

export class AutonomousMemoryConsolidationError extends ArgumentError {}

function fail(message: string): never {
  throw new AutonomousMemoryConsolidationError(`memory consolidation ${message}`);
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !ID.test(value) || new TextEncoder().encode(value).byteLength > 256) fail(`${name} is not a bounded identifier`);
  return value;
}

function digest(name: string, value: unknown, optional = false): string | null {
  if (optional && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function domain(value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !(DOMAINS as readonly string[]).includes(value)) fail("domain is not a supported built-in autonomous domain");
  return value as AutonomousDomainName;
}

function numberBound(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function integerBound(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its integer bounds`);
  return value as number;
}

function booleanValue(name: string, value: unknown): boolean {
  if (typeof value !== "boolean") fail(`${name} must be boolean`);
  return value;
}

function stringTuple(name: string, value: unknown, maximum = 64): string[] {
  if (!Array.isArray(value) || value.length > maximum) fail(`${name} must be a bounded array`);
  const result = value.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(result).size !== result.length) fail(`${name} contains duplicates`);
  return [...result].sort();
}

function wilsonLower(successes: number, observations: number): number {
  if (observations <= 0) return 0;
  const z = 1.959963984540054;
  const rate = successes / observations;
  const denominator = 1 + z * z / observations;
  const center = rate + z * z / (2 * observations);
  const spread = z * Math.sqrt((rate * (1 - rate) + z * z / (4 * observations)) / observations);
  return Math.max(0, Math.min(1, (center - spread) / denominator));
}

function normalizedReward(reward: number): number {
  return (reward + 1) / 2;
}

export interface AutonomousMemoryConsolidationObservation {
  episode_id: string;
  lesson_id: string;
  concept_id: string;
  variant_id: string;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  evidence_digest: string;
  lesson_digest: string;
  decision_digest: string | null;
  observed_at: number;
  transferable: boolean;
}

export interface AutonomousMemoryConsolidatedLesson {
  schema: typeof AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA;
  concept_id: string;
  lesson_id: string;
  variant_id: string;
  scope: "domain" | "cross_domain";
  domains: AutonomousDomainName[];
  capabilities: string[];
  risk_classes: string[];
  lesson_digest: string;
  observation_count: number;
  passed_count: number;
  failed_count: number;
  reward_mean: number;
  support_lower_bound: number;
  confidence: number;
  first_observed_at: number;
  last_observed_at: number;
  transferable: boolean;
  status: "candidate" | "stable" | "conflicted" | "stale";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousMemoryConsolidationPolicy {
  min_observations: number;
  min_support_lower_bound: number;
  conflict_dominance: number;
  max_age_seconds: number;
  max_lessons: number;
}

export interface AutonomousMemoryConsolidationDomainProjection {
  domain: AutonomousDomainName;
  observation_count: number;
  lesson_count: number;
  stable_count: number;
  conflicted_count: number;
  portable_count: number;
}

export interface AutonomousMemoryConsolidationReport {
  schema: typeof AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA;
  generation: number;
  previous_report_digest: string | null;
  policy: AutonomousMemoryConsolidationPolicy;
  observation_count: number;
  deduplicated_observation_count: number;
  lessons: AutonomousMemoryConsolidatedLesson[];
  conflicts: Array<{ concept_id: string; scope: "domain" | "cross_domain"; domain: AutonomousDomainName | null; variant_ids: string[]; status: "conflicted" }>;
  domains: AutonomousMemoryConsolidationDomainProjection[];
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  report_digest: string;
}

export interface AutonomousMemoryConsolidationSnapshot {
  schema: typeof AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA;
  generation: number;
  previous_snapshot_digest: string | null;
  report: AutonomousMemoryConsolidationReport;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  snapshot_digest: string;
}

export interface AutonomousMemoryConsolidationTextStore {
  read(): string | null;
  write(value: string): void;
}

export interface AutonomousMemoryConsolidationTransactionalTextStore extends AutonomousMemoryConsolidationTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): boolean;
}

export interface AutonomousMemoryConsolidationPromptReference {
  lesson_id: string;
  concept_id: string;
  lesson_digest: string;
  text: string;
  status: "stable";
  confidence: number;
  source: "evaluator_gated_memory_consolidation";
}

function normalizeObservation(value: AutonomousMemoryConsolidationObservation): AutonomousMemoryConsolidationObservation {
  if (!isObject(value)) fail("observation must be an object");
  const normalized = {
    episode_id: identifier("observation episode_id", value.episode_id),
    lesson_id: identifier("observation lesson_id", value.lesson_id),
    concept_id: identifier("observation concept_id", value.concept_id),
    variant_id: identifier("observation variant_id", value.variant_id),
    domain: domain(value.domain),
    capability: identifier("observation capability", value.capability),
    risk_class: identifier("observation risk_class", value.risk_class),
    evaluator_id: identifier("observation evaluator_id", value.evaluator_id),
    evaluator_version: identifier("observation evaluator_version", value.evaluator_version),
    reward: numberBound("observation reward", value.reward, -1, 1),
    passed: booleanValue("observation passed", value.passed),
    evidence_digest: digest("observation evidence_digest", value.evidence_digest)!,
    lesson_digest: digest("observation lesson_digest", value.lesson_digest)!,
    decision_digest: digest("observation decision_digest", value.decision_digest ?? null, true),
    observed_at: numberBound("observation observed_at", value.observed_at ?? 0, 0, 9_223_372_036_854_775),
    transferable: booleanValue("observation transferable", value.transferable ?? false),
  };
  if (Object.keys(value).some((key) => !Object.keys(normalized).includes(key))) fail("observation contains unsupported fields");
  return normalized;
}

function lessonProjection(row: Omit<AutonomousMemoryConsolidatedLesson, "schema" | "retention" | "secret_material">): AutonomousMemoryConsolidatedLesson {
  return {
    schema: AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA,
    ...row,
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  };
}

function validateLesson(value: unknown): AutonomousMemoryConsolidatedLesson {
  if (!isObject(value) || value.schema !== AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA || value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("report lesson is malformed");
  const row = lessonProjection({
    concept_id: identifier("lesson concept_id", value.concept_id), lesson_id: identifier("lesson lesson_id", value.lesson_id), variant_id: identifier("lesson variant_id", value.variant_id),
    scope: value.scope === "domain" || value.scope === "cross_domain" ? value.scope : fail("lesson scope is unsupported"), domains: (Array.isArray(value.domains) ? value.domains.map(domain) : fail("lesson domains are malformed")),
    capabilities: stringTuple("lesson capabilities", value.capabilities), risk_classes: stringTuple("lesson risk_classes", value.risk_classes), lesson_digest: digest("lesson lesson_digest", value.lesson_digest)!,
    observation_count: integerBound("lesson observation_count", value.observation_count, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS), passed_count: integerBound("lesson passed_count", value.passed_count, 0, value.observation_count as number), failed_count: integerBound("lesson failed_count", value.failed_count, 0, value.observation_count as number),
    reward_mean: numberBound("lesson reward_mean", value.reward_mean, 0, 1), support_lower_bound: numberBound("lesson support_lower_bound", value.support_lower_bound, 0, 1), confidence: numberBound("lesson confidence", value.confidence, 0, 1),
    first_observed_at: numberBound("lesson first_observed_at", value.first_observed_at, 0, 9_223_372_036_854_775), last_observed_at: numberBound("lesson last_observed_at", value.last_observed_at, value.first_observed_at as number, 9_223_372_036_854_775), transferable: booleanValue("lesson transferable", value.transferable),
    status: ["candidate", "stable", "conflicted", "stale"].includes(value.status as string) ? value.status as "candidate" | "stable" | "conflicted" | "stale" : fail("lesson status is unsupported"),
  });
  if (row.passed_count + row.failed_count > row.observation_count) fail("lesson passed and failed counts exceed observations");
  return row;
}

function validateReport(value: unknown): AutonomousMemoryConsolidationReport {
  if (!isObject(value) || value.schema !== AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA || value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("report is malformed");
  const generation = integerBound("report generation", value.generation, 1, 2_147_483_647);
  const observationCount = integerBound("report observation_count", value.observation_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS);
  const deduplicatedObservationCount = integerBound("report deduplicated_observation_count", value.deduplicated_observation_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS);
  if (deduplicatedObservationCount > observationCount) fail("report deduplicated count exceeds observation count");
  if (!isObject(value.policy)) fail("report policy is malformed");
  const policy: AutonomousMemoryConsolidationPolicy = { min_observations: integerBound("report policy min_observations", value.policy.min_observations, 1, 1_024), min_support_lower_bound: numberBound("report policy min_support_lower_bound", value.policy.min_support_lower_bound, 0, 1), conflict_dominance: numberBound("report policy conflict_dominance", value.policy.conflict_dominance, 0.5, 1), max_age_seconds: numberBound("report policy max_age_seconds", value.policy.max_age_seconds, 1, 31_536_000), max_lessons: integerBound("report policy max_lessons", value.policy.max_lessons, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS) };
  if (!Array.isArray(value.lessons) || value.lessons.length > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS) fail("report lessons are malformed");
  const lessons = value.lessons.map(validateLesson);
  if (!Array.isArray(value.domains) || value.domains.length !== DOMAINS.length) fail("report domain coverage is malformed");
  const domains = value.domains.map((raw, index) => {
    if (!isObject(raw) || raw.domain !== DOMAINS[index]) fail("report domain coverage must contain every domain in canonical order");
    return { domain: domain(raw.domain), observation_count: integerBound(`report domain ${raw.domain} observation_count`, raw.observation_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS), lesson_count: integerBound(`report domain ${raw.domain} lesson_count`, raw.lesson_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS), stable_count: integerBound(`report domain ${raw.domain} stable_count`, raw.stable_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS), conflicted_count: integerBound(`report domain ${raw.domain} conflicted_count`, raw.conflicted_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS), portable_count: integerBound(`report domain ${raw.domain} portable_count`, raw.portable_count, 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS) };
  });
  if (!Array.isArray(value.conflicts)) fail("report conflicts are malformed");
  const conflicts = value.conflicts.map((raw) => {
    if (!isObject(raw) || (raw.scope !== "domain" && raw.scope !== "cross_domain") || raw.status !== "conflicted" || !Array.isArray(raw.variant_ids)) fail("report conflict row is malformed");
    if (Object.keys(raw).some((key) => !["concept_id", "scope", "domain", "variant_ids", "status"].includes(key))) fail("report conflict row contains unsupported fields");
    const conflictDomain = raw.domain === null ? null : domain(raw.domain);
    if ((raw.scope === "domain" && conflictDomain === null) || (raw.scope === "cross_domain" && conflictDomain !== null)) fail("report conflict domain scope is malformed");
    return { concept_id: identifier("conflict concept_id", raw.concept_id), scope: raw.scope as "domain" | "cross_domain", domain: conflictDomain, variant_ids: stringTuple("conflict variant_ids" as string, raw.variant_ids), status: "conflicted" as const };
  });
  const reportDigest = digest("report report_digest", value.report_digest)!;
  const body = { schema: value.schema, generation, previous_report_digest: digest("report previous_report_digest", value.previous_report_digest, true), policy, observation_count: observationCount, deduplicated_observation_count: deduplicatedObservationCount, lessons, conflicts, domains, retention: RETENTION, secret_material: SECRET_MATERIAL };
  if (digestJsonSync(body) !== reportDigest) fail("report digest does not match its canonical projection");
  return { ...body, report_digest: reportDigest };
}

export function validateAutonomousMemoryConsolidationReport(value: unknown): AutonomousMemoryConsolidationReport {
  return validateReport(value);
}

export function validateAutonomousMemoryConsolidationSnapshot(value: unknown): AutonomousMemoryConsolidationSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA || value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("snapshot is malformed");
  const generation = integerBound("snapshot generation", value.generation, 1, 2_147_483_647);
  const report = validateReport(value.report);
  const previous = digest("snapshot previous_snapshot_digest", value.previous_snapshot_digest, true);
  const snapshotDigest = digest("snapshot snapshot_digest", value.snapshot_digest)!;
  const body = { schema: value.schema, generation, previous_snapshot_digest: previous, report, retention: RETENTION, secret_material: SECRET_MATERIAL };
  if (digestJsonSync(body) !== snapshotDigest) fail("snapshot digest does not match its canonical projection");
  if (new TextEncoder().encode(canonicalJson(value)).byteLength > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
  return { ...body, snapshot_digest: snapshotDigest };
}

export class AutonomousMemoryConsolidator {
  private readonly policy: AutonomousMemoryConsolidationPolicy;
  private readonly clock: () => number;
  private generationValue = 0;
  private previousSnapshotDigest: string | null = null;
  private reportValue: AutonomousMemoryConsolidationReport | null = null;

  constructor(options: { minObservations?: number; minSupportLowerBound?: number; conflictDominance?: number; maxAgeSeconds?: number; maxLessons?: number; clock?: () => number } = {}) {
    this.policy = { min_observations: integerBound("minObservations", options.minObservations ?? 3, 1, 1_024), min_support_lower_bound: numberBound("minSupportLowerBound", options.minSupportLowerBound ?? 0.6, 0, 1), conflict_dominance: numberBound("conflictDominance", options.conflictDominance ?? 0.75, 0.5, 1), max_age_seconds: numberBound("maxAgeSeconds", options.maxAgeSeconds ?? 30 * 24 * 60 * 60, 1, 31_536_000), max_lessons: integerBound("maxLessons", options.maxLessons ?? MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS) };
    this.clock = options.clock ?? (() => Date.now() / 1000);
    if (typeof this.clock !== "function") fail("clock must be callable");
  }

  get report(): AutonomousMemoryConsolidationReport | null {
    return this.reportValue === null ? null : structuredClone(this.reportValue);
  }

  consolidate(input: readonly AutonomousMemoryConsolidationObservation[], options: { generation?: number } = {}): AutonomousMemoryConsolidationReport {
    if (!Array.isArray(input) || input.length > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS) fail("observations exceed their bound");
    const now = numberBound("consolidation clock", this.clock(), 0, 9_223_372_036_854_775);
    const normalized = input.map(normalizeObservation);
    const identities = new Map<string, AutonomousMemoryConsolidationObservation>();
    for (const row of normalized) {
      const identity = [row.episode_id, row.lesson_id, row.evaluator_id, row.evaluator_version].join("\u0000");
      const prior = identities.get(identity);
      if (prior && canonicalJson(prior) !== canonicalJson(row)) fail(`observation ${row.episode_id} is contradictory for the same evaluator identity`);
      identities.set(identity, row);
    }
    const groups = new Map<string, AutonomousMemoryConsolidationObservation[]>();
    for (const row of identities.values()) {
      const scope = row.transferable ? "cross_domain" : row.domain;
      const key = [row.concept_id, row.lesson_id, row.variant_id, row.lesson_digest, scope].join("\u0000");
      groups.set(key, [...(groups.get(key) ?? []), row]);
    }
    let provisional: Omit<AutonomousMemoryConsolidatedLesson, "schema" | "retention" | "secret_material">[] = [];
    for (const [key, rows] of [...groups.entries()].sort(([left], [right]) => left.localeCompare(right))) {
      const [conceptId, lessonId, variantId, lessonDigest, scopeKey] = key.split("\u0000");
      const observationCount = rows.length;
      const passedCount = rows.filter((row) => row.passed).length;
      const failedCount = observationCount - passedCount;
      const rewardMean = rows.reduce((sum, row) => sum + normalizedReward(row.reward), 0) / observationCount;
      const supportLowerBound = wilsonLower(passedCount, observationCount);
      const confidence = Math.min(1, observationCount / this.policy.min_observations) * (0.5 + 0.5 * (passedCount / observationCount));
      const age = Math.max(0, now - Math.max(...rows.map((row) => row.observed_at)));
      const status: "candidate" | "stable" | "stale" = age > this.policy.max_age_seconds ? "stale" : observationCount >= this.policy.min_observations && supportLowerBound >= this.policy.min_support_lower_bound ? "stable" : "candidate";
      provisional.push({ concept_id: conceptId!, lesson_id: lessonId!, variant_id: variantId!, scope: scopeKey === "cross_domain" ? "cross_domain" : "domain", domains: [...new Set(rows.map((row) => row.domain))].sort() as AutonomousDomainName[], capabilities: [...new Set(rows.map((row) => row.capability))].sort(), risk_classes: [...new Set(rows.map((row) => row.risk_class))].sort(), lesson_digest: lessonDigest!, observation_count: observationCount, passed_count: passedCount, failed_count: failedCount, reward_mean: rewardMean, support_lower_bound: supportLowerBound, confidence, first_observed_at: Math.min(...rows.map((row) => row.observed_at)), last_observed_at: Math.max(...rows.map((row) => row.observed_at)), transferable: scopeKey === "cross_domain", status });
    }
    if (provisional.length > this.policy.max_lessons) provisional = provisional.sort((left, right) => right.confidence - left.confidence || right.reward_mean - left.reward_mean || left.concept_id.localeCompare(right.concept_id) || left.variant_id.localeCompare(right.variant_id)).slice(0, this.policy.max_lessons);
    const conflicts: AutonomousMemoryConsolidationReport["conflicts"] = [];
    const rewritten: typeof provisional = [];
    const byConcept = new Map<string, typeof provisional>();
    for (const row of provisional) {
      const scopeDomain = row.scope === "domain" ? row.domains[0] ?? "" : "cross_domain";
      const key = `${row.concept_id}\u0000${row.scope}\u0000${scopeDomain}`;
      byConcept.set(key, [...(byConcept.get(key) ?? []), row]);
    }
    for (const [key, variants] of [...byConcept.entries()].sort(([left], [right]) => left.localeCompare(right))) {
      if (variants.length < 2) { rewritten.push(...variants); continue; }
      const supportMass = variants.reduce((sum, row) => sum + Math.max(0, row.reward_mean) * row.observation_count, 0);
      const leader = [...variants].sort((left, right) => right.reward_mean - left.reward_mean || right.observation_count - left.observation_count || left.variant_id.localeCompare(right.variant_id))[0]!;
      const leaderMass = Math.max(0, leader.reward_mean) * leader.observation_count;
      if (supportMass <= 0 || leaderMass / supportMass < this.policy.conflict_dominance) {
        const [conceptId, scope, scopeDomain] = key.split("\u0000");
        conflicts.push({ concept_id: conceptId!, scope: scope as "domain" | "cross_domain", domain: scope === "domain" ? domain(scopeDomain) : null, variant_ids: variants.map((row) => row.variant_id).sort(), status: "conflicted" });
        rewritten.push(...variants.map((row) => ({ ...row, status: "conflicted" as const })));
      } else rewritten.push(...variants);
    }
    const lessons = rewritten.sort((left, right) => left.concept_id.localeCompare(right.concept_id) || left.scope.localeCompare(right.scope) || left.variant_id.localeCompare(right.variant_id) || left.lesson_id.localeCompare(right.lesson_id)).map(lessonProjection);
    const domains = DOMAINS.map((item) => {
      const selected = rewritten.filter((row) => row.domains.includes(item));
      return { domain: item, observation_count: selected.reduce((sum, row) => sum + row.observation_count, 0), lesson_count: selected.length, stable_count: selected.filter((row) => row.status === "stable").length, conflicted_count: selected.filter((row) => row.status === "conflicted").length, portable_count: selected.filter((row) => row.scope === "cross_domain").length };
    });
    const generation = options.generation ?? this.generationValue + 1;
    integerBound("generation", generation, 1, 2_147_483_647);
    if (generation <= this.generationValue) fail("generation must advance monotonically");
    const body = { schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA, generation, previous_report_digest: this.reportValue?.report_digest ?? null, policy: this.policy, observation_count: normalized.length, deduplicated_observation_count: identities.size, lessons, conflicts, domains, retention: RETENTION, secret_material: SECRET_MATERIAL } satisfies Omit<AutonomousMemoryConsolidationReport, "report_digest">;
    const report = { ...body, report_digest: digestJsonSync(body) } satisfies AutonomousMemoryConsolidationReport;
    if (new TextEncoder().encode(canonicalJson(report)).byteLength > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES) fail("report exceeds its byte bound");
    this.generationValue = generation;
    this.reportValue = validateReport(report);
    return structuredClone(this.reportValue);
  }

  recall(options: { domain: AutonomousDomainName; capability?: string; includeUnstable?: boolean; limit?: number }): AutonomousMemoryConsolidatedLesson[] {
    const selectedDomain = domain(options.domain);
    const capability = options.capability === undefined ? undefined : identifier("recall capability", options.capability);
    const limit = integerBound("recall limit", options.limit ?? 8, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS);
    const rows = (this.reportValue?.lessons ?? []).filter((row) => row.domains.includes(selectedDomain) && (capability === undefined || row.capabilities.includes(capability)) && (options.includeUnstable === true || row.status === "stable"));
    rows.sort((left, right) => Number(left.status !== "stable") - Number(right.status !== "stable") || right.confidence - left.confidence || right.reward_mean - left.reward_mean || left.concept_id.localeCompare(right.concept_id) || left.variant_id.localeCompare(right.variant_id));
    return structuredClone(rows.slice(0, limit));
  }

  promptReferences(options: { domain: AutonomousDomainName; capability?: string; lessonResolver: (lessonDigest: string) => string | null; limit?: number }): AutonomousMemoryConsolidationPromptReference[] {
    if (typeof options.lessonResolver !== "function") fail("lessonResolver must be callable");
    return this.recall(options).flatMap((row) => {
      const text = options.lessonResolver(row.lesson_digest);
      if (text === null) return [];
      if (typeof text !== "string" || !text.trim() || new TextEncoder().encode(text).byteLength > 4_096 || /\u0000/.test(text)) fail("lessonResolver returned malformed lesson text");
      return [{ lesson_id: row.lesson_id, concept_id: row.concept_id, lesson_digest: row.lesson_digest, text, status: "stable" as const, confidence: row.confidence, source: "evaluator_gated_memory_consolidation" as const }];
    });
  }

  snapshot(): AutonomousMemoryConsolidationSnapshot {
    const report = this.reportValue ?? this.consolidate([]);
    const body = { schema: AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA, generation: this.generationValue, previous_snapshot_digest: this.previousSnapshotDigest, report, retention: RETENTION, secret_material: SECRET_MATERIAL } satisfies Omit<AutonomousMemoryConsolidationSnapshot, "snapshot_digest">;
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) } satisfies AutonomousMemoryConsolidationSnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES) fail("snapshot exceeds its byte bound");
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    return structuredClone(snapshot);
  }

  restore(snapshot: AutonomousMemoryConsolidationSnapshot): AutonomousMemoryConsolidationReport {
    const validated = validateAutonomousMemoryConsolidationSnapshot(snapshot);
    if (canonicalJson(validated.report.policy) !== canonicalJson(this.policy)) fail("restored policy conflicts with the configured consolidator");
    this.generationValue = validated.generation;
    this.previousSnapshotDigest = validated.snapshot_digest;
    this.reportValue = validated.report;
    return structuredClone(validated.report);
  }
}

export class JsonAutonomousMemoryConsolidationPersistence {
  readonly textStore: AutonomousMemoryConsolidationTextStore;
  readonly maxBytes: number;
  constructor(textStore: AutonomousMemoryConsolidationTextStore, maxBytes = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") fail("JSON text store is malformed");
    this.textStore = textStore;
    this.maxBytes = integerBound("JSON maxBytes", maxBytes, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES);
  }
  read(): AutonomousMemoryConsolidationSnapshot | null {
    const encoded = this.textStore.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("JSON snapshot exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { fail("JSON snapshot is invalid"); }
    if (canonicalJson(parsed) !== encoded) fail("JSON snapshot is not canonical");
    return validateAutonomousMemoryConsolidationSnapshot(parsed);
  }
  write(snapshot: AutonomousMemoryConsolidationSnapshot): void {
    const validated = validateAutonomousMemoryConsolidationSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) fail("JSON snapshot exceeds its byte bound");
    this.textStore.write(encoded);
  }
}

export class TransactionalJsonAutonomousMemoryConsolidationPersistence extends JsonAutonomousMemoryConsolidationPersistence {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousMemoryConsolidationSnapshot): boolean {
    digest("expectedSnapshotDigest", expectedSnapshotDigest, true);
    if (typeof (this.textStore as Partial<AutonomousMemoryConsolidationTransactionalTextStore>).writeIfUnchanged !== "function") fail("transactional JSON text store lacks compare-and-swap");
    const encoded = canonicalJson(validateAutonomousMemoryConsolidationSnapshot(snapshot));
    return Boolean((this.textStore as AutonomousMemoryConsolidationTransactionalTextStore).writeIfUnchanged(expectedSnapshotDigest, encoded));
  }
}

export class AutonomousMemoryConsolidationPersistenceCoordinator {
  readonly consolidator: AutonomousMemoryConsolidator;
  readonly persistence: JsonAutonomousMemoryConsolidationPersistence;
  private expectedSnapshotDigest: string | null = null;
  constructor(consolidator: AutonomousMemoryConsolidator, persistence: JsonAutonomousMemoryConsolidationPersistence) {
    if (!(consolidator instanceof AutonomousMemoryConsolidator) || !persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") fail("persistence coordinator inputs are malformed");
    this.consolidator = consolidator;
    this.persistence = persistence;
  }
  restore(): AutonomousMemoryConsolidationSnapshot | null {
    const snapshot = this.persistence.read();
    if (snapshot === null) return null;
    this.consolidator.restore(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
  flush(): AutonomousMemoryConsolidationSnapshot {
    const snapshot = this.consolidator.snapshot();
    if (this.persistence instanceof TransactionalJsonAutonomousMemoryConsolidationPersistence && !this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) fail("persistence compare-and-swap conflict");
    if (!(this.persistence instanceof TransactionalJsonAutonomousMemoryConsolidationPersistence)) this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return snapshot;
  }
}
