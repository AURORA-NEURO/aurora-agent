import { ArgumentError } from "./errors.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_MEMORY_SCHEMA = "bioprism-typescript-autonomous-episodic-memory/0.1" as const;
export const AUTONOMOUS_MEMORY_EVENT_SCHEMA = "bioprism-typescript-autonomous-episodic-event/0.1" as const;
export const AUTONOMOUS_MEMORY_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-memory-snapshot/0.1" as const;
export const AUTONOMOUS_MEMORY_MAX_EPISODES = 4_096;
export const AUTONOMOUS_MEMORY_MAX_EVENTS = 16_384;
export const AUTONOMOUS_MEMORY_MAX_TAGS = 64;
export const AUTONOMOUS_MEMORY_MAX_QUERY_LIMIT = 128;

const PRIVATE_RETENTION = "value_only_hash_chained;task_prompt_response_tool_payloads_and_credentials_not_retained" as const;

export type AutonomousMemoryEpisodeStatus = "completed" | "failed" | "partial" | "approval_required";

export interface AutonomousMemoryRouteProjection extends JsonObject {
  route_digest: string;
  source: string;
  selected_domains: string[];
  primary_domain: string | null;
  confidence: number;
}

export interface AutonomousMemoryEvaluation extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string | null;
  evaluation_digest: string;
}

export interface AutonomousMemoryEpisode extends JsonObject {
  schema: typeof AUTONOMOUS_MEMORY_SCHEMA;
  episode_id: string;
  run_id: string;
  result_kind: string;
  status: AutonomousMemoryEpisodeStatus;
  task_digest: string;
  context: { domain: string; capability: string; risk_class: string };
  selected_model: { provider: string; model: string } | null;
  digests: Record<string, string | null>;
  route: AutonomousMemoryRouteProjection | null;
  tags: string[];
  lesson: string | null;
  provenance: Record<string, string>;
  evaluation: AutonomousMemoryEvaluation | null;
  created_at: number;
  updated_at: number;
  episode_digest: string;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousMemoryEpisodeInput {
  episode_id: string;
  run_id: string;
  result_kind: string;
  status: AutonomousMemoryEpisodeStatus;
  task_digest: string;
  context: { domain: string; capability: string; risk_class: string };
  selected_model?: { provider: string; model: string } | null;
  digests: Record<string, string | null>;
  route?: AutonomousMemoryRouteProjection | null;
  tags?: readonly string[];
  lesson?: string | null;
  provenance?: Record<string, string>;
}

export interface AutonomousMemoryEvaluationInput {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: string | null;
  failure_class?: string | null;
  evidence_digest?: string | null;
}

export interface AutonomousMemoryQuery {
  domain?: string;
  capability?: string;
  risk_class?: string;
  task_digest?: string;
  tags?: readonly string[];
  statuses?: readonly AutonomousMemoryEpisodeStatus[];
  includeFailed?: boolean;
  limit?: number;
}

export interface AutonomousMemoryEvent extends JsonObject {
  schema: typeof AUTONOMOUS_MEMORY_EVENT_SCHEMA;
  sequence: number;
  event_type: "episode_recorded" | "evaluation_recorded";
  episode_id: string;
  payload: JsonObject;
  previous_digest: string;
  event_digest: string;
  created_at: number;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousMemoryReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_MEMORY_EVENT_SCHEMA;
  event_type: AutonomousMemoryEvent["event_type"];
  episode_id: string;
  sequence: number;
  event_digest: string;
  head_digest: string;
  idempotent: boolean;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousMemoryStats extends JsonObject {
  episodes: number;
  evaluated: number;
  pending_evaluation: number;
  failed: number;
  head_digest: string;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousMemorySnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_MEMORY_SNAPSHOT_SCHEMA;
  sequence: number;
  head_digest: string;
  episodes: AutonomousMemoryEpisode[];
  events: AutonomousMemoryEvent[];
  snapshot_digest: string;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousMemoryPersistence {
  read(): Promise<AutonomousMemorySnapshot | null> | AutonomousMemorySnapshot | null;
  write(snapshot: AutonomousMemorySnapshot): Promise<void> | void;
}

export interface AutonomousEpisodicMemoryStore {
  recordEpisode(input: AutonomousMemoryEpisodeInput): Promise<AutonomousMemoryReceipt> | AutonomousMemoryReceipt;
  recordEvaluation(episodeId: string, input: AutonomousMemoryEvaluationInput): Promise<AutonomousMemoryReceipt> | AutonomousMemoryReceipt;
  get(episodeId: string): Promise<AutonomousMemoryEpisode | null> | AutonomousMemoryEpisode | null;
  retrieve(query?: AutonomousMemoryQuery): Promise<AutonomousMemoryEpisode[]> | AutonomousMemoryEpisode[];
  stats(): Promise<AutonomousMemoryStats> | AutonomousMemoryStats;
  verifyIntegrity(): Promise<AutonomousMemoryStats> | AutonomousMemoryStats;
  snapshot(): Promise<AutonomousMemorySnapshot> | AutonomousMemorySnapshot;
  restore(snapshot: AutonomousMemorySnapshot): Promise<void> | void;
}

const FORBIDDEN_KEYS = new Set([
  "apikey", "authorization", "bearer", "credential", "password", "secret", "accesstoken", "refreshtoken",
  "prompt", "messages", "response", "content", "raw", "body", "headers", "arguments", "input", "output", "task",
]);
const SECRET_PATTERNS = [
  /(?i:api[_ -]?key|access[_ -]?token|refresh[_ -]?token|password|authorization|secret)\s*[:=]\s*\S+/i,
  /\bbearer\s+[A-Za-z0-9._~+/=-]{16,}/i,
  /\b(?:sk|rk|pk)-[A-Za-z0-9_-]{16,}\b/,
];

function clone<T>(value: T): T { return structuredClone(value); }

function boundedString(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded memory contract`);
  if (SECRET_PATTERNS.some((pattern) => pattern.test(value))) throw new ArgumentError(`${name} resembles secret material`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedString(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedProbability(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return value;
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 8) throw new ArgumentError("autonomous memory metadata is too deeply nested");
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    if (typeof value === "string") boundedString("autonomous memory metadata", value, 16_000);
    return;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ArgumentError("autonomous memory metadata contains a non-finite number");
    return;
  }
  if (Array.isArray(value)) { if (value.length > 128) throw new ArgumentError("autonomous memory metadata sequence is too large"); for (const item of value) safeMetadata(item, depth + 1); return; }
  if (typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length > 64) throw new ArgumentError("autonomous memory metadata mapping is too large");
    for (const [key, child] of entries) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (FORBIDDEN_KEYS.has(normalized)) throw new ArgumentError("autonomous memory metadata contains a forbidden raw-content or secret field");
      boundedString("autonomous memory metadata key", key, 256);
      safeMetadata(child, depth + 1);
    }
    return;
  }
  throw new ArgumentError("autonomous memory metadata is not JSON-safe");
}

function normalizeRoute(route: AutonomousMemoryRouteProjection | null | undefined): AutonomousMemoryRouteProjection | null {
  if (route === null || route === undefined) return null;
  safeMetadata(route);
  if (!Array.isArray(route.selected_domains) || route.selected_domains.length > 16) throw new ArgumentError("memory route selected_domains is outside its bounds");
  return {
    route_digest: boundedDigest("memory route_digest", route.route_digest)!,
    source: boundedIdentifier("memory route source", route.source),
    selected_domains: route.selected_domains.map((domain) => boundedIdentifier("memory route domain", domain)),
    primary_domain: route.primary_domain === null ? null : boundedIdentifier("memory route primary_domain", route.primary_domain),
    confidence: boundedProbability("memory route confidence", route.confidence),
  };
}

function normalizeDigests(value: unknown): Record<string, string | null> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ArgumentError("memory digests must be a mapping");
  const entries = Object.entries(value);
  if (entries.length > 32) throw new ArgumentError("memory digests exceed their bounded count");
  const result: Record<string, string | null> = {};
  for (const [key, digest] of entries) {
    const name = boundedIdentifier("memory digest field", key);
    if (!name.endsWith("_digest")) throw new ArgumentError("memory digest fields must end with _digest");
    result[name] = boundedDigest(name, digest, true);
  }
  return result;
}

interface AutonomousMemoryEvaluationCore {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string | null;
}

function normalizeEvaluation(input: AutonomousMemoryEvaluationInput): AutonomousMemoryEvaluationCore {
  safeMetadata(input);
  return {
    evaluator_id: boundedIdentifier("memory evaluator_id", input.evaluator_id),
    evaluator_version: boundedIdentifier("memory evaluator_version", input.evaluator_version),
    reward: boundedProbability("memory evaluator reward", input.reward),
    passed: input.passed === true,
    failed: input.failed ?? input.passed !== true,
    feedback_digest: boundedDigest("memory feedback_digest", input.feedback_digest ?? null, true),
    failure_class: input.failure_class === undefined || input.failure_class === null ? null : boundedIdentifier("memory failure_class", input.failure_class),
    evidence_digest: boundedDigest("memory evidence_digest", input.evidence_digest ?? null, true),
  };
}

/** Bounded in-memory reference implementation; production callers can persist snapshots externally. */
export class InMemoryAutonomousEpisodicMemory implements AutonomousEpisodicMemoryStore {
  private readonly episodes = new Map<string, AutonomousMemoryEpisode>();
  private readonly events: AutonomousMemoryEvent[] = [];
  private readonly clock: () => number;
  private readonly maxEpisodes: number;
  private readonly maxEvents: number;

  constructor(options: { clock?: () => number; maxEpisodes?: number; maxEvents?: number } = {}) {
    this.clock = options.clock ?? (() => Date.now());
    this.maxEpisodes = options.maxEpisodes ?? AUTONOMOUS_MEMORY_MAX_EPISODES;
    this.maxEvents = options.maxEvents ?? AUTONOMOUS_MEMORY_MAX_EVENTS;
    if (!Number.isSafeInteger(this.maxEpisodes) || this.maxEpisodes < 1 || this.maxEpisodes > AUTONOMOUS_MEMORY_MAX_EPISODES) throw new ArgumentError("memory maxEpisodes is outside its bounds");
    if (!Number.isSafeInteger(this.maxEvents) || this.maxEvents < 1 || this.maxEvents > AUTONOMOUS_MEMORY_MAX_EVENTS) throw new ArgumentError("memory maxEvents is outside its bounds");
  }

  async recordEpisode(input: AutonomousMemoryEpisodeInput): Promise<AutonomousMemoryReceipt> {
    safeMetadata(input);
    const episodeId = boundedIdentifier("memory episode_id", input.episode_id);
    const taskDigest = boundedDigest("memory task_digest", input.task_digest)!;
    const context = input.context;
    if (!context || typeof context !== "object") throw new ArgumentError("memory context is required");
    const normalizedContext = { domain: boundedIdentifier("memory context domain", context.domain), capability: boundedIdentifier("memory context capability", context.capability), risk_class: boundedIdentifier("memory context risk_class", context.risk_class) };
    if (!["completed", "failed", "partial", "approval_required"].includes(input.status)) throw new ArgumentError("memory episode status is unsupported");
    const selectedModel = input.selected_model === null || input.selected_model === undefined ? null : { provider: boundedIdentifier("memory selected provider", input.selected_model.provider), model: boundedIdentifier("memory selected model", input.selected_model.model) };
    const tags = [...new Set((input.tags ?? []).map((tag) => boundedString("memory tag", tag, 128)))].slice(0, AUTONOMOUS_MEMORY_MAX_TAGS);
    const lesson = input.lesson === null || input.lesson === undefined ? null : boundedString("memory lesson", input.lesson, 4_096);
    const provenance = input.provenance ?? {};
    safeMetadata(provenance);
    const normalizedProvenance = Object.fromEntries(Object.entries(provenance).map(([key, value]) => [boundedIdentifier("memory provenance key", key), boundedString("memory provenance value", value, 512)]));
    const route = normalizeRoute(input.route);
    const digests = normalizeDigests(input.digests);
    const core = { schema: AUTONOMOUS_MEMORY_SCHEMA, episode_id: episodeId, run_id: boundedIdentifier("memory run_id", input.run_id), result_kind: boundedIdentifier("memory result_kind", input.result_kind), status: input.status, task_digest: taskDigest, context: normalizedContext, selected_model: selectedModel, digests, route, tags, lesson, provenance: normalizedProvenance, retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    const episodeDigest = await digestJson(core);
    const existing = this.episodes.get(episodeId);
    if (existing) {
      if (existing.episode_digest !== episodeDigest) throw new ArgumentError("memory episode_id already exists with different metadata");
      return this.receipt("episode_recorded", episodeId, existing.episode_digest, true);
    }
    if (this.episodes.size >= this.maxEpisodes) throw new ArgumentError("autonomous memory episode capacity is exhausted");
    const now = this.clock();
    if (!Number.isFinite(now)) throw new ArgumentError("memory clock must return a finite number");
    const episode: AutonomousMemoryEpisode = { ...core, evaluation: null, created_at: now, updated_at: now, episode_digest: episodeDigest };
    const receipt = await this.appendEvent("episode_recorded", episodeId, episode);
    this.episodes.set(episodeId, episode);
    return receipt;
  }

  async recordEvaluation(episodeId: string, input: AutonomousMemoryEvaluationInput): Promise<AutonomousMemoryReceipt> {
    const id = boundedIdentifier("memory episode_id", episodeId);
    const episode = this.episodes.get(id);
    if (!episode) throw new ArgumentError("cannot evaluate an unknown memory episode");
    const normalized = normalizeEvaluation(input);
    const evaluationDigest = await digestJson(normalized);
    if (episode.evaluation?.evaluation_digest === evaluationDigest) return this.receipt("evaluation_recorded", id, evaluationDigest, true);
    const evaluation: AutonomousMemoryEvaluation = {
      evaluator_id: normalized.evaluator_id,
      evaluator_version: normalized.evaluator_version,
      reward: normalized.reward,
      passed: normalized.passed,
      failed: normalized.failed,
      feedback_digest: normalized.feedback_digest,
      failure_class: normalized.failure_class,
      evidence_digest: normalized.evidence_digest,
      evaluation_digest: evaluationDigest,
    };
    const receipt = await this.appendEvent("evaluation_recorded", id, evaluation);
    const updated = { ...episode, evaluation, updated_at: this.clock() };
    this.episodes.set(id, updated);
    return receipt;
  }

  get(episodeId: string): AutonomousMemoryEpisode | null {
    return clone(this.episodes.get(boundedIdentifier("memory episode_id", episodeId)) ?? null);
  }

  retrieve(query: AutonomousMemoryQuery = {}): AutonomousMemoryEpisode[] {
    safeMetadata(query);
    const limit = query.limit ?? 8;
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > AUTONOMOUS_MEMORY_MAX_QUERY_LIMIT) throw new ArgumentError("memory query limit is outside its bounds");
    if (query.includeFailed !== undefined && typeof query.includeFailed !== "boolean") throw new ArgumentError("memory query includeFailed must be boolean");
    const tags = new Set((query.tags ?? []).map((tag) => boundedString("memory query tag", tag, 128)));
    const statuses = new Set(query.statuses ?? []);
    for (const status of statuses) if (!["completed", "failed", "partial", "approval_required"].includes(status)) throw new ArgumentError("memory query contains an unsupported status");
    const taskDigest = query.task_digest === undefined ? undefined : boundedDigest("memory query task_digest", query.task_digest)!;
    const matches = [...this.episodes.values()].filter((episode) => {
      if (query.domain !== undefined && episode.context.domain !== boundedIdentifier("memory query domain", query.domain)) return false;
      if (query.capability !== undefined && episode.context.capability !== boundedIdentifier("memory query capability", query.capability)) return false;
      if (query.risk_class !== undefined && episode.context.risk_class !== boundedIdentifier("memory query risk_class", query.risk_class)) return false;
      if (taskDigest !== undefined && episode.task_digest !== taskDigest) return false;
      if (statuses.size && !statuses.has(episode.status)) return false;
      if (query.includeFailed === false && (episode.status === "failed" || episode.evaluation?.failed === true)) return false;
      if (tags.size && ![...tags].some((tag) => episode.tags.includes(tag))) return false;
      return true;
    }).map((episode) => ({ score: (query.domain ? 20 : 0) + (query.capability ? 20 : 0) + (query.risk_class ? 10 : 0) + (query.task_digest ? 100 : 0) + [...tags].filter((tag) => episode.tags.includes(tag)).length * 5 + (episode.evaluation?.passed ? 2 : 0), episode }));
    matches.sort((left, right) => right.score - left.score || right.episode.updated_at - left.episode.updated_at || left.episode.episode_id.localeCompare(right.episode.episode_id));
    return matches.slice(0, limit).map(({ episode }) => clone(episode));
  }

  stats(): AutonomousMemoryStats {
    const episodes = [...this.episodes.values()];
    return { episodes: episodes.length, evaluated: episodes.filter((episode) => episode.evaluation !== null).length, pending_evaluation: episodes.filter((episode) => episode.evaluation === null).length, failed: episodes.filter((episode) => episode.status === "failed" || episode.evaluation?.failed === true).length, head_digest: this.events.at(-1)?.event_digest ?? "", retention: PRIVATE_RETENTION };
  }

  async verifyIntegrity(): Promise<AutonomousMemoryStats> {
    let previous = "";
    const recordedEpisodes = new Set<string>();
    for (let index = 0; index < this.events.length; index += 1) {
      const event = this.events[index]!;
      if (event.sequence !== index + 1 || event.previous_digest !== previous) throw new ArgumentError(`autonomous memory hash chain breaks at sequence ${event.sequence}`);
      const { event_digest: _eventDigest, ...body } = event;
      if (await digestJson(body) !== event.event_digest) throw new ArgumentError(`autonomous memory event digest mismatch at sequence ${event.sequence}`);
      if (event.event_type === "episode_recorded") {
        if (!event.payload || event.payload.episode_id !== event.episode_id || typeof event.payload.episode_digest !== "string") throw new ArgumentError(`autonomous memory episode event is malformed at sequence ${event.sequence}`);
        const episode = this.episodes.get(event.episode_id);
        if (!episode || episode.episode_digest !== event.payload.episode_digest) throw new ArgumentError(`autonomous memory episode index disagrees at sequence ${event.sequence}`);
        recordedEpisodes.add(event.episode_id);
      } else if (event.event_type === "evaluation_recorded") {
        if (!event.payload || event.payload.evaluation_digest !== this.episodes.get(event.episode_id)?.evaluation?.evaluation_digest) throw new ArgumentError(`autonomous memory evaluation index disagrees at sequence ${event.sequence}`);
      } else {
        throw new ArgumentError(`autonomous memory event type is unsupported at sequence ${event.sequence}`);
      }
      previous = event.event_digest;
    }
    for (const episode of this.episodes.values()) {
      if (!recordedEpisodes.has(episode.episode_id)) throw new ArgumentError(`autonomous memory episode ${episode.episode_id} has no recorded event`);
      const { episode_digest: _episodeDigest, evaluation: _evaluation, created_at: _createdAt, updated_at: _updatedAt, ...core } = episode;
      if (await digestJson(core) !== episode.episode_digest) throw new ArgumentError(`autonomous memory episode digest mismatch for ${episode.episode_id}`);
      if (episode.evaluation) {
        const { evaluation_digest: evaluationDigest, ...evaluation } = episode.evaluation;
        if (await digestJson(evaluation) !== evaluationDigest) throw new ArgumentError(`autonomous memory evaluation digest mismatch for ${episode.episode_id}`);
      }
    }
    return this.stats();
  }

  async snapshot(): Promise<AutonomousMemorySnapshot> {
    const body = { schema: AUTONOMOUS_MEMORY_SNAPSHOT_SCHEMA, sequence: this.events.length, head_digest: this.events.at(-1)?.event_digest ?? "", episodes: [...this.episodes.values()].map(clone), events: this.events.map(clone), retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    return { ...body, snapshot_digest: await digestJson(body) };
  }

  async restore(snapshot: AutonomousMemorySnapshot): Promise<void> {
    if (!snapshot || snapshot.schema !== AUTONOMOUS_MEMORY_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.episodes) || !Array.isArray(snapshot.events)) throw new ArgumentError("autonomous memory snapshot is malformed");
    const { snapshot_digest: supplied, ...body } = snapshot;
    if (await digestJson(body) !== supplied) throw new ArgumentError("autonomous memory snapshot digest mismatch");
    if (snapshot.events.length > this.maxEvents || snapshot.episodes.length > this.maxEpisodes) throw new ArgumentError("autonomous memory snapshot exceeds store capacity");
    const restored = new InMemoryAutonomousEpisodicMemory({ clock: this.clock, maxEpisodes: this.maxEpisodes, maxEvents: this.maxEvents });
    for (const episode of snapshot.episodes) restored.episodes.set(boundedIdentifier("memory episode_id", episode.episode_id), clone(episode));
    restored.events.push(...snapshot.events.map(clone));
    await restored.verifyIntegrity();
    if (restored.events.length !== snapshot.sequence || (restored.events.at(-1)?.event_digest ?? "") !== snapshot.head_digest) throw new ArgumentError("autonomous memory snapshot head is inconsistent");
    this.episodes.clear();
    this.events.splice(0, this.events.length, ...restored.events);
    for (const [id, episode] of restored.episodes) this.episodes.set(id, episode);
  }

  private async appendEvent(type: AutonomousMemoryEvent["event_type"], episodeId: string, payload: JsonObject): Promise<AutonomousMemoryReceipt> {
    if (this.events.length >= this.maxEvents) throw new ArgumentError("autonomous memory event capacity is exhausted");
    const base = { schema: AUTONOMOUS_MEMORY_EVENT_SCHEMA, sequence: this.events.length + 1, event_type: type, episode_id: episodeId, payload, previous_digest: this.events.at(-1)?.event_digest ?? "", created_at: this.clock(), retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    const event: AutonomousMemoryEvent = { ...base, event_digest: await digestJson(base) };
    this.events.push(event);
    return { schema: AUTONOMOUS_MEMORY_EVENT_SCHEMA, event_type: type, episode_id: episodeId, sequence: event.sequence, event_digest: event.event_digest, head_digest: event.event_digest, idempotent: false, retention: PRIVATE_RETENTION };
  }

  private receipt(type: AutonomousMemoryEvent["event_type"], episodeId: string, digest: string, idempotent: boolean): AutonomousMemoryReceipt {
    const event = [...this.events].reverse().find((candidate) => candidate.event_type === type && candidate.episode_id === episodeId);
    return { schema: AUTONOMOUS_MEMORY_EVENT_SCHEMA, event_type: type, episode_id: episodeId, sequence: event?.sequence ?? 0, event_digest: event?.event_digest ?? digest, head_digest: this.events.at(-1)?.event_digest ?? "", idempotent, retention: PRIVATE_RETENTION };
  }
}

/** Connect memory snapshots to SQLite, IndexedDB, Postgres, or another caller-owned adapter. */
export class AutonomousMemoryPersistenceCoordinator {
  constructor(readonly store: AutonomousEpisodicMemoryStore, readonly persistence: AutonomousMemoryPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("memory store is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("memory persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousMemorySnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot) await this.store.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousMemorySnapshot> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}
