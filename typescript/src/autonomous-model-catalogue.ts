import { ArgumentError, isObject } from "./errors.js";
import { boundedModelDigest, boundedText, bytes } from "./autonomous-validation.js";
import type { AutonomousModelCataloguePersistence, AutonomousModelCatalogueSnapshot } from "./autonomous-agent-contracts.js";
import type { AutonomousAgent } from "./autonomous.js";
import type { AutonomousModelCandidate } from "./llm.js";
import { digestJson } from "./tooling.js";

export const AUTONOMOUS_MODEL_REFRESH_SCHEMA = "bioprism-typescript-autonomous-model-refresh/0.1" as const;
export const AUTONOMOUS_MODEL_CATALOGUE_REFRESH_SCHEMA = "bioprism-typescript-autonomous-model-catalogue-refresh/0.1" as const;
export const AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-model-catalogue-snapshot/0.1" as const;
export const AUTONOMOUS_MODEL_CATALOGUE_REFRESH_MAX_PROVIDERS = 32;
export const AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS = 128;
export const AUTONOMOUS_MODEL_CATALOGUE_MAX_SNAPSHOT_BYTES = 1_000_000;

function boundedModelMetric(name: string, value: unknown, minimum: number, maximum: number, integer = false): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum || (integer && !Number.isSafeInteger(value))) {
    throw new ArgumentError(`${name} is outside its bounded model contract`);
  }
  return value;
}

/** Normalize provider catalogue metadata without retaining unsupported or secret-shaped fields. */
export function normalizeAutonomousModelCandidate(candidate: AutonomousModelCandidate): AutonomousModelCandidate {
  if (!isObject(candidate)) throw new ArgumentError("autonomous model candidate must be an object");
  const allowedKeys = new Set(["provider", "model", "capabilities", "context_window_tokens", "max_output_tokens", "quality", "latency_ms", "cost_per_million_tokens", "reliability", "requires_credential", "enabled"]);
  if (Object.keys(candidate).some((key) => !allowedKeys.has(key))) throw new ArgumentError("autonomous model candidate contains unsupported or secret-shaped metadata");
  const provider = boundedText("autonomous model provider", candidate.provider, 128);
  const model = boundedText("autonomous model id", candidate.model, 512);
  let capabilities: string[] | undefined;
  if (candidate.capabilities !== undefined) {
    if (!Array.isArray(candidate.capabilities) || candidate.capabilities.length > 128) throw new ArgumentError("autonomous model capabilities are outside their bounds");
    capabilities = candidate.capabilities.map((capability) => boundedText("autonomous model capability", capability, 128));
    if (new Set(capabilities).size !== capabilities.length) throw new ArgumentError("autonomous model capabilities contain duplicates");
  }
  const contextWindow = boundedModelMetric("autonomous model context_window_tokens", candidate.context_window_tokens, 1, 100_000_000, true);
  const maxOutput = boundedModelMetric("autonomous model max_output_tokens", candidate.max_output_tokens, 1, 10_000_000, true);
  const quality = boundedModelMetric("autonomous model quality", candidate.quality, 0, 1);
  const latency = boundedModelMetric("autonomous model latency_ms", candidate.latency_ms, 0, 10 * 60_000);
  const cost = boundedModelMetric("autonomous model cost_per_million_tokens", candidate.cost_per_million_tokens, 0, 1_000_000_000);
  const reliability = boundedModelMetric("autonomous model reliability", candidate.reliability, 0, 1);
  if (candidate.requires_credential !== undefined && typeof candidate.requires_credential !== "boolean") throw new ArgumentError("autonomous model requires_credential must be boolean");
  if (candidate.enabled !== undefined && typeof candidate.enabled !== "boolean") throw new ArgumentError("autonomous model enabled must be boolean");
  return {
    provider,
    model,
    ...(capabilities ? { capabilities } : {}),
    context_window_tokens: contextWindow,
    max_output_tokens: maxOutput,
    quality,
    latency_ms: latency,
    cost_per_million_tokens: cost,
    reliability,
    ...(candidate.requires_credential === undefined ? {} : { requires_credential: candidate.requires_credential }),
    ...(candidate.enabled === undefined ? {} : { enabled: candidate.enabled }),
  };
}

/** Validate and canonicalize a restart snapshot before it can touch the live catalogue. */
export async function validateAutonomousModelCatalogueSnapshot(value: unknown): Promise<AutonomousModelCatalogueSnapshot> {
  if (!isObject(value)) throw new ArgumentError("autonomous model catalogue snapshot must be an object");
  const allowedKeys = new Set(["schema", "models", "catalogue_digest", "snapshot_digest", "retention", "secret_material"]);
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) throw new ArgumentError("autonomous model catalogue snapshot contains unsupported metadata");
  if (value.schema !== AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA || value.retention !== "model_metadata_only_hash_bound" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous model catalogue snapshot markers are invalid");
  if (!Array.isArray(value.models) || value.models.length > AUTONOMOUS_MODEL_CATALOGUE_MAX_MODELS) throw new ArgumentError("autonomous model catalogue snapshot exceeds its model capacity");
  const models = value.models.map((candidate) => normalizeAutonomousModelCandidate(candidate as AutonomousModelCandidate));
  const ids = new Set<string>();
  for (const candidate of models) {
    const id = `${candidate.provider}/${candidate.model}`;
    if (ids.has(id)) throw new ArgumentError(`autonomous model catalogue snapshot contains duplicate model ${id}`);
    ids.add(id);
  }
  const catalogueDigest = boundedModelDigest("autonomous model catalogue snapshot catalogue_digest", value.catalogue_digest);
  if (await digestJson(models) !== catalogueDigest) throw new ArgumentError("autonomous model catalogue snapshot catalogue digest mismatch");
  const descriptor = {
    schema: AUTONOMOUS_MODEL_CATALOGUE_SNAPSHOT_SCHEMA,
    models,
    catalogue_digest: catalogueDigest,
    retention: "model_metadata_only_hash_bound" as const,
    secret_material: "never_returned" as const,
  };
  const snapshotDigest = boundedModelDigest("autonomous model catalogue snapshot snapshot_digest", value.snapshot_digest);
  if (await digestJson(descriptor) !== snapshotDigest) throw new ArgumentError("autonomous model catalogue snapshot digest mismatch");
  const snapshot = { ...descriptor, snapshot_digest: snapshotDigest };
  if (bytes(JSON.stringify(snapshot)) > AUTONOMOUS_MODEL_CATALOGUE_MAX_SNAPSHOT_BYTES) throw new ArgumentError("autonomous model catalogue snapshot exceeds its byte capacity");
  return structuredClone(snapshot);
}

/** Flushes/restores the agent's model catalogue through a caller-owned persistence adapter. */
export class AutonomousModelCataloguePersistenceCoordinator {
  constructor(readonly agent: AutonomousAgent, readonly persistence: AutonomousModelCataloguePersistence) {
    if (!agent || typeof agent.snapshotModels !== "function" || typeof agent.restoreModels !== "function") throw new ArgumentError("model catalogue persistence requires an AutonomousAgent");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("model catalogue persistence adapter is malformed");
  }

  async restore(): Promise<AutonomousModelCatalogueSnapshot | null> {
    return this.agent.restoreModelCatalogue(this.persistence);
  }

  async flush(): Promise<AutonomousModelCatalogueSnapshot> {
    return this.agent.saveModelCatalogue(this.persistence);
  }
}
