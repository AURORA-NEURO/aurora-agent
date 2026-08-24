import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import type { AutonomousEvidenceAcquisitionContext } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Digest-addressed normalizer identity and value-free claim projection contracts. */
export const AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA = "bioprism-typescript-autonomous-evidence-normalizer/0.1" as const;
export const AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-evidence-normalizer-registry/0.1" as const;
export const AUTONOMOUS_EVIDENCE_CLAIM_PROJECTION_SCHEMA = "bioprism-typescript-autonomous-evidence-claim-projection/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS = 256;
export const MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_LIMITATIONS = 16;
export const MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_TEXT_BYTES = 512;
export const MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_VALUE_BYTES = 64_000_000;
export const MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES = 64_000;
export const MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_BYTES = 256_000;

const RETENTION = "metadata_only;normalizer_callbacks_and_raw_values_caller_owned" as const;
const SPEC_RETENTION = "metadata_only;normalizer_callback_not_serialized" as const;
const SECRET_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);
const CREDENTIAL_SHAPE = /(?:^|\b)(?:gsk_|sk-proj-|sk-[A-Za-z0-9]{16,})(?:$|\b)/i;

export type AutonomousEvidenceNormalizer = (
  value: JsonValue,
  context: AutonomousEvidenceAcquisitionContext,
) => JsonValue | Promise<JsonValue>;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function text(name: string, value: unknown, maximum = MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_TEXT_BYTES): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value.trim();
}

function identifier(name: string, value: unknown, maximum = 256): string {
  const result = text(name, value, maximum);
  if (!/^[A-Za-z0-9_.:+\-/ ]+$/.test(result)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return result;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function secretKey(key: string): boolean {
  const normalized = [...key.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
  return SECRET_MARKERS.has(normalized)
    || normalized.startsWith("gsk")
    || normalized.startsWith("skproj")
    || normalized.includes("token")
    || normalized.includes("secret")
    || normalized.includes("credential")
    || normalized.includes("authorization");
}

function assertSafeJson(value: unknown, name: string, depth = 0): asserts value is JsonValue {
  if (depth > 32) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null || typeof value === "boolean" || typeof value === "number") {
    if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
    return;
  }
  if (typeof value === "string") {
    if (CREDENTIAL_SHAPE.test(value)) throw new ArgumentError(`${name} contains credential-shaped material`);
    return;
  }
  if (Array.isArray(value)) {
    if (value.length > 16_384) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((child, index) => assertSafeJson(child, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value as Record<string, unknown>).length > 16_384) throw new ArgumentError(`${name} contains too many fields`);
    for (const [key, child] of Object.entries(value)) {
      if (!key.trim() || key.includes("\u0000") || secretKey(key)) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertSafeJson(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  throw new ArgumentError(`${name} is not JSON-safe`);
}

function canonicalValue(value: unknown, name: string, maximum = MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_VALUE_BYTES): { value: JsonValue; bytes: number } {
  assertSafeJson(value, name);
  let encoded: string;
  try {
    encoded = canonicalJson(value);
  } catch (error) {
    throw new ArgumentError(`${name} is not canonical JSON`, { cause: error });
  }
  const size = bytes(encoded);
  if (size > maximum) throw new ArgumentError(`${name} exceeds its byte bound`);
  return { value: JSON.parse(encoded) as JsonValue, bytes: size };
}

function boundedList(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} must contain between 1 and ${maximum} entries`);
  const result = value.map((item, index) => text(`${name}[${index}]`, item, 2_048));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicates`);
  return [...result].sort();
}

function observationKind(value: JsonValue): "null" | "scalar" | "object" | "array" {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  if (isObject(value)) return "object";
  return "scalar";
}

function shapeDigest(value: JsonValue, kind: ReturnType<typeof observationKind>): string {
  if (kind === "object" && isObject(value)) return digestJsonSync({ kind, keys: Object.keys(value).sort() });
  if (kind === "array" && Array.isArray(value)) {
    const sample = value.slice(0, 64);
    return digestJsonSync({
      kind,
      item_kinds: sample.map((item) => observationKind(item)),
      item_shapes: sample.filter((item): item is JsonObject => isObject(item)).map((item) => Object.keys(item).sort()),
    });
  }
  return digestJsonSync({ kind });
}

function operation(context: AutonomousEvidenceAcquisitionContext): string {
  const metadata = context.request.metadata;
  const candidate = isObject(metadata) ? metadata.operation : undefined;
  return candidate === undefined ? "unspecified" : identifier("autonomous evidence normalizer operation", candidate);
}

export interface AutonomousEvidenceNormalizerSpecJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA;
  domain: AutonomousDomainName;
  normalizer_id: string;
  version: string;
  purpose: string;
  limitations: string[];
  execution: "normalizer_identity_only;callback_not_invoked";
  retention: typeof SPEC_RETENTION;
  secret_material: "never_returned";
  spec_digest: string;
}

export interface AutonomousEvidenceNormalizerRegistryJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_SCHEMA;
  normalizers: AutonomousEvidenceNormalizerSpecJSON[];
  execution: "registry_projection_only;callbacks_not_invoked";
  retention: typeof RETENTION;
  secret_material: "never_returned";
  registry_digest: string;
}

export interface AutonomousEvidenceClaimProjectionJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_CLAIM_PROJECTION_SCHEMA;
  domain: AutonomousDomainName;
  normalizer_id: string;
  normalizer_version: string;
  operation: string;
  observation_kind: "null" | "scalar" | "object" | "array";
  item_count: number;
  value_bytes: number;
  value_digest: string;
  shape_digest: string;
  claim_posture: "projection_only;truth_and_evaluation_caller_owned";
  limitations: string[];
  claim_digest: string;
}

export class AutonomousEvidenceNormalizerSpec {
  readonly domain: AutonomousDomainName;
  readonly normalizer_id: string;
  readonly version: string;
  readonly purpose: string;
  readonly limitations: string[];
  readonly spec_digest: string;

  constructor(input: {
    domain: AutonomousDomainName;
    normalizerId: string;
    version: string;
    purpose: string;
    limitations: readonly string[];
  }) {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(input.domain)) throw new ArgumentError("evidence normalizer domain is unsupported");
    this.domain = input.domain;
    this.normalizer_id = identifier("evidence normalizer normalizerId", input.normalizerId);
    this.version = identifier("evidence normalizer version", input.version);
    this.purpose = text("evidence normalizer purpose", input.purpose, 2_048);
    this.limitations = boundedList("evidence normalizer limitations", input.limitations, MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_LIMITATIONS);
    this.spec_digest = digestJsonSync(this.descriptor());
  }

  toJSON(): AutonomousEvidenceNormalizerSpecJSON {
    return { ...this.descriptor(), spec_digest: this.spec_digest } as AutonomousEvidenceNormalizerSpecJSON;
  }

  static fromJSON(value: unknown): AutonomousEvidenceNormalizerSpec {
    if (!isObject(value)) throw new ArgumentError("evidence normalizer spec must be a JSON object");
    const expected = ["domain", "execution", "limitations", "normalizer_id", "purpose", "retention", "schema", "secret_material", "spec_digest", "version"];
    if (Object.keys(value).sort().join("\u0000") !== expected.join("\u0000") || value.schema !== AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA) throw new ArgumentError("evidence normalizer spec contains unsupported fields");
    if (value.execution !== "normalizer_identity_only;callback_not_invoked" || value.retention !== SPEC_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("evidence normalizer spec retention is invalid");
    if (!Array.isArray(value.limitations)) throw new ArgumentError("evidence normalizer spec limitations are malformed");
    const spec = new AutonomousEvidenceNormalizerSpec({
      domain: value.domain as AutonomousDomainName,
      normalizerId: value.normalizer_id as string,
      version: value.version as string,
      purpose: value.purpose as string,
      limitations: value.limitations as string[],
    });
    if (value.spec_digest !== spec.spec_digest || canonicalJson(value) !== canonicalJson(spec.toJSON())) throw new ArgumentError("evidence normalizer spec digest or canonical form is invalid");
    return spec;
  }

  private descriptor(): Omit<AutonomousEvidenceNormalizerSpecJSON, "spec_digest"> {
    return {
      schema: AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA,
      domain: this.domain,
      normalizer_id: this.normalizer_id,
      version: this.version,
      purpose: this.purpose,
      limitations: [...this.limitations],
      execution: "normalizer_identity_only;callback_not_invoked",
      retention: SPEC_RETENTION,
      secret_material: "never_returned",
    };
  }
}

export class AutonomousEvidenceNormalizerRegistration {
  readonly spec: AutonomousEvidenceNormalizerSpec;
  readonly normalizer: AutonomousEvidenceNormalizer;

  constructor(spec: AutonomousEvidenceNormalizerSpec, normalizer: AutonomousEvidenceNormalizer) {
    if (!(spec instanceof AutonomousEvidenceNormalizerSpec) || typeof normalizer !== "function") throw new ArgumentError("evidence normalizer registration is malformed");
    this.spec = spec;
    this.normalizer = normalizer;
  }

  toJSON(): AutonomousEvidenceNormalizerSpecJSON {
    return this.spec.toJSON();
  }
}

/** Built-in digest/shape projection that never returns the observed value. */
export class AutonomousEvidenceClaimProjector {
  constructor(readonly spec: AutonomousEvidenceNormalizerSpec) {
    if (!(spec instanceof AutonomousEvidenceNormalizerSpec)) throw new ArgumentError("claim projector requires a typed normalizer spec");
  }

  normalize(value: JsonValue, context: AutonomousEvidenceAcquisitionContext): AutonomousEvidenceClaimProjectionJSON {
    if (!isObject(context)) throw new ArgumentError("evidence claim projection context must be a JSON object");
    const canonical = canonicalValue(value, "evidence claim projection value");
    const kind = observationKind(canonical.value);
    const itemCount: number = isObject(canonical.value)
      ? Object.keys(canonical.value).length
      : Array.isArray(canonical.value) ? canonical.value.length : 1;
    if (itemCount > 16_384) throw new ArgumentError("evidence claim projection item count exceeds its bound");
    const descriptor = {
      schema: AUTONOMOUS_EVIDENCE_CLAIM_PROJECTION_SCHEMA,
      domain: this.spec.domain,
      normalizer_id: this.spec.normalizer_id,
      normalizer_version: this.spec.version,
      operation: operation(context),
      observation_kind: kind,
      item_count: itemCount,
      value_bytes: canonical.bytes,
      value_digest: digestJsonSync(canonical.value),
      shape_digest: shapeDigest(canonical.value, kind),
      claim_posture: "projection_only;truth_and_evaluation_caller_owned" as const,
      limitations: [...this.spec.limitations],
    };
    const result = { ...descriptor, claim_digest: digestJsonSync(descriptor) } as AutonomousEvidenceClaimProjectionJSON;
    if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES) throw new ArgumentError("evidence claim projection exceeds its byte bound");
    return result;
  }
}

function identity(value: JsonValue, _context: AutonomousEvidenceAcquisitionContext): JsonValue {
  return canonicalValue(value, "evidence identity normalizer value").value;
}

export class AutonomousEvidenceNormalizerRegistry {
  private readonly entries = new Map<string, AutonomousEvidenceNormalizerRegistration>();

  constructor(registrations: readonly AutonomousEvidenceNormalizerRegistration[] = []) {
    if (!Array.isArray(registrations) || registrations.length > MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS) throw new ArgumentError("evidence normalizer registrations are outside their bound");
    registrations.forEach((registration) => this.register(registration));
  }

  register(registration: AutonomousEvidenceNormalizerRegistration, options: { replace?: boolean } = {}): AutonomousEvidenceNormalizerSpecJSON {
    if (!(registration instanceof AutonomousEvidenceNormalizerRegistration)) throw new ArgumentError("evidence normalizer registration is malformed");
    const key = this.key(registration.spec.domain, registration.spec.normalizer_id, registration.spec.version);
    const existing = this.entries.get(key);
    if (existing && options.replace !== true) throw new ArgumentError(`evidence normalizer is already registered: ${key}`);
    if (existing && existing.spec.spec_digest === registration.spec.spec_digest && existing.normalizer !== registration.normalizer) throw new ArgumentError("evidence normalizer callback changed without a new versioned spec");
    if (!existing && this.entries.size >= MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS) throw new ArgumentError("evidence normalizer registry is full");
    this.entries.set(key, registration);
    try {
      this.toJSON();
    } catch (error) {
      if (existing) this.entries.set(key, existing); else this.entries.delete(key);
      throw error;
    }
    return registration.toJSON();
  }

  unregister(domain: AutonomousDomainName, normalizerId: string, version: string): boolean {
    return this.entries.delete(this.key(domain, normalizerId, version));
  }

  registrations(): AutonomousEvidenceNormalizerRegistration[] {
    return [...this.entries.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([, registration]) => registration);
  }

  resolve(domain: AutonomousDomainName, normalizerId: string, version: string): AutonomousEvidenceNormalizerRegistration {
    const registration = this.entries.get(this.key(domain, normalizerId, version));
    if (!registration) throw new ArgumentError(`evidence normalizer is not registered: ${domain}/${normalizerId}/${version}`);
    return registration;
  }

  async normalize(domain: AutonomousDomainName, normalizerId: string, version: string, value: JsonValue, context: AutonomousEvidenceAcquisitionContext): Promise<JsonValue> {
    const registration = this.resolve(domain, normalizerId, version);
    const result = await registration.normalizer(value, context);
    return canonicalValue(result, "evidence normalizer output", MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES).value;
  }

  get registryDigest(): string {
    return digestJsonSync(this.descriptor());
  }

  toJSON(): AutonomousEvidenceNormalizerRegistryJSON {
    const descriptor = this.descriptor();
    if (bytes(canonicalJson(descriptor)) > MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_BYTES) throw new ArgumentError("evidence normalizer registry exceeds its byte bound");
    return { ...descriptor, registry_digest: this.registryDigest } as AutonomousEvidenceNormalizerRegistryJSON;
  }

  private descriptor(): Omit<AutonomousEvidenceNormalizerRegistryJSON, "registry_digest"> {
    return {
      schema: AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_SCHEMA,
      normalizers: this.registrations().map((registration) => registration.toJSON()),
      execution: "registry_projection_only;callbacks_not_invoked",
      retention: RETENTION,
      secret_material: "never_returned",
    };
  }

  private key(domain: AutonomousDomainName, normalizerId: string, version: string): string {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("evidence normalizer domain is unsupported");
    return `${domain}\u0000${identifier("evidence normalizer normalizerId", normalizerId)}\u0000${identifier("evidence normalizer version", version)}`;
  }
}

const BUILTIN_NORMALIZER_IDS: Record<AutonomousDomainName, string> = {
  coding: "builtin.coding.claim-projection",
  browser: "builtin.browser.claim-projection",
  data: "builtin.data.claim-projection",
  science: "builtin.science.claim-projection",
  biomedical: "builtin.biomedical.claim-projection",
  neuroscience: "builtin.neuroscience.claim-projection",
  operations: "builtin.operations.claim-projection",
  enterprise: "builtin.enterprise.claim-projection",
  multi_agent: "builtin.multi-agent.claim-projection",
  multimodal: "builtin.multimodal.claim-projection",
  cross_domain: "builtin.cross-domain.claim-projection",
  evaluation: "builtin.evaluation.claim-projection",
};

export function createBuiltinAutonomousEvidenceNormalizerRegistry(): AutonomousEvidenceNormalizerRegistry {
  const registrations: AutonomousEvidenceNormalizerRegistration[] = [];
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const identitySpec = new AutonomousEvidenceNormalizerSpec({
      domain,
      normalizerId: "identity",
      version: "1",
      purpose: "Preserve an exact caller-selected JSON observation for transient reconciliation.",
      limitations: ["exact value equality is required", "the caller owns truth and evaluation"],
    });
    registrations.push(new AutonomousEvidenceNormalizerRegistration(identitySpec, identity));
    const projectionSpec = new AutonomousEvidenceNormalizerSpec({
      domain,
      normalizerId: BUILTIN_NORMALIZER_IDS[domain],
      version: "1",
      purpose: `Project bounded ${domain} evidence into digest and response-shape metadata.`,
      limitations: ["raw source values are not returned by the projection", "shape and digest are not truth or evaluator verdicts"],
    });
    const projector = new AutonomousEvidenceClaimProjector(projectionSpec);
    registrations.push(new AutonomousEvidenceNormalizerRegistration(projectionSpec, projector.normalize.bind(projector)));
  }
  return new AutonomousEvidenceNormalizerRegistry(registrations);
}

export function builtinAutonomousEvidenceNormalizerSpecs(): AutonomousEvidenceNormalizerSpec[] {
  return createBuiltinAutonomousEvidenceNormalizerRegistry().registrations().map((registration) => registration.spec);
}
