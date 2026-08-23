import { ArgumentError } from "./errors.js";
import {
  AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA,
  validateAutonomousEvaluatorCalibrationReport,
  type AutonomousEvaluatorCalibrationReport,
  type AutonomousEvaluatorCalibrationStatus,
} from "./autonomous-evaluator-calibration.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Restart-safe metadata registry for validated evaluator calibration reports. */
export const AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_SCHEMA = "bioprism-typescript-autonomous-evaluator-calibration-store/0.1" as const;
export const AUTONOMOUS_EVALUATOR_CALIBRATION_IMPORT_SCHEMA = "bioprism-typescript-autonomous-evaluator-calibration-import/0.1" as const;
export const MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORED_REPORTS = 128;
export const MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_BYTES = 2_000_000;

const RETENTION = "metadata_only;calibration_reports_contain_no_cases_labels_prompts_responses_or_credentials" as const;
const SECRET_MATERIAL = "never_returned" as const;

export interface AutonomousEvaluatorCalibrationStoreSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_SCHEMA;
  generation: number;
  reports: AutonomousEvaluatorCalibrationReport[];
  snapshot_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousEvaluatorCalibrationStore {
  read(): Promise<AutonomousEvaluatorCalibrationStoreSnapshot | null> | AutonomousEvaluatorCalibrationStoreSnapshot | null;
  write(snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): Promise<void> | void;
}

export interface AutonomousEvaluatorCalibrationTransactionalStore extends AutonomousEvaluatorCalibrationStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousEvaluatorCalibrationImport extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATOR_CALIBRATION_IMPORT_SCHEMA;
  report_digest: string;
  created: boolean;
  registry_generation: number;
  registry_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  import_digest: string;
}

export interface AutonomousEvaluatorCalibrationQueryOptions {
  domain?: AutonomousDomainName;
  status?: AutonomousEvaluatorCalibrationStatus;
  decision?: "admit_learning" | "hold_learning";
  limit?: number;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer within ${minimum}..${maximum}`);
  return value as number;
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+/-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function snapshotDescriptor(snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): Omit<AutonomousEvaluatorCalibrationStoreSnapshot, "snapshot_digest"> {
  const { snapshot_digest: _snapshotDigest, ...descriptor } = snapshot;
  return descriptor;
}

function registryDigest(reports: readonly AutonomousEvaluatorCalibrationReport[]): string {
  return digestJsonSync(reports.map((report) => report.report_digest).sort());
}

function snapshotFrom(reports: readonly AutonomousEvaluatorCalibrationReport[], generation: number): AutonomousEvaluatorCalibrationStoreSnapshot {
  const descriptor = {
    schema: AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_SCHEMA,
    generation,
    reports: [...reports].sort((left, right) => left.report_digest.localeCompare(right.report_digest)).map((report) => structuredClone(report)),
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  } satisfies Omit<AutonomousEvaluatorCalibrationStoreSnapshot, "snapshot_digest">;
  return { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
}

function validateSnapshot(value: unknown): AutonomousEvaluatorCalibrationStoreSnapshot {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ArgumentError("evaluator calibration store snapshot is malformed");
  const snapshot = value as AutonomousEvaluatorCalibrationStoreSnapshot;
  if (snapshot.schema !== AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_SCHEMA) throw new ArgumentError("evaluator calibration store snapshot schema is invalid");
  integer("evaluator calibration store generation", snapshot.generation, 0, Number.MAX_SAFE_INTEGER);
  if (!Array.isArray(snapshot.reports) || snapshot.reports.length > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORED_REPORTS) throw new ArgumentError("evaluator calibration store report count is outside its bound");
  const reports = snapshot.reports.map((report) => validateAutonomousEvaluatorCalibrationReport(report));
  if (new Set(reports.map((report) => report.report_digest)).size !== reports.length) throw new ArgumentError("evaluator calibration store report digests must be unique");
  digest("evaluator calibration store snapshot_digest", snapshot.snapshot_digest);
  if (snapshot.retention !== RETENTION || snapshot.secret_material !== SECRET_MATERIAL) throw new ArgumentError("evaluator calibration store retention markers are invalid");
  if (digestJsonSync(snapshotDescriptor({ ...snapshot, reports })) !== snapshot.snapshot_digest) throw new ArgumentError("evaluator calibration store snapshot digest does not match its content");
  const encoded = JSON.stringify({ ...snapshot, reports });
  if (!encoded || bytes(encoded) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_BYTES) throw new ArgumentError("evaluator calibration store snapshot exceeds its bound");
  return structuredClone({ ...snapshot, reports });
}

function importProjection(report: AutonomousEvaluatorCalibrationReport, created: boolean, generation: number, reportDigests: readonly string[]): AutonomousEvaluatorCalibrationImport {
  const descriptor = {
    schema: AUTONOMOUS_EVALUATOR_CALIBRATION_IMPORT_SCHEMA,
    report_digest: report.report_digest,
    created,
    registry_generation: generation,
    registry_digest: digestJsonSync([...reportDigests].sort()),
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  } satisfies Omit<AutonomousEvaluatorCalibrationImport, "import_digest">;
  return { ...descriptor, import_digest: digestJsonSync(descriptor) };
}

/** In-memory reference store with the same snapshot and CAS semantics as durable adapters. */
export class InMemoryAutonomousEvaluatorCalibrationStore implements AutonomousEvaluatorCalibrationTransactionalStore {
  private snapshot: AutonomousEvaluatorCalibrationStoreSnapshot | null = null;

  read(): AutonomousEvaluatorCalibrationStoreSnapshot | null {
    return this.snapshot === null ? null : structuredClone(this.snapshot);
  }

  write(snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): void {
    this.snapshot = validateSnapshot(snapshot);
  }

  writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): boolean {
    if ((this.snapshot?.snapshot_digest ?? null) !== expectedSnapshotDigest) return false;
    this.write(snapshot);
    return true;
  }
}

/** Strict JSON adapter for browser, Node, and embedded persistence layers. */
export class JsonAutonomousEvaluatorCalibrationStore implements AutonomousEvaluatorCalibrationStore {
  constructor(protected readonly store: { read(): Promise<string | null> | string | null; write(value: string): Promise<void> | void }) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("evaluator calibration JSON store is malformed");
  }

  async read(): Promise<AutonomousEvaluatorCalibrationStoreSnapshot | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_BYTES) throw new ArgumentError("evaluator calibration JSON store text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("evaluator calibration JSON store text is invalid JSON");
    }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("evaluator calibration JSON store text is not canonical");
    return validateSnapshot(parsed);
  }

  async write(snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): Promise<void> {
    const validated = validateSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    if (bytes(encoded) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORE_BYTES) throw new ArgumentError("evaluator calibration JSON store text exceeds its bound");
    await this.store.write(encoded);
  }
}

/** JSON adapter that exposes compare-and-swap instead of pretending ordinary writes are atomic. */
export class TransactionalJsonAutonomousEvaluatorCalibrationStore extends JsonAutonomousEvaluatorCalibrationStore implements AutonomousEvaluatorCalibrationTransactionalStore {
  private readonly transactionalStore: { writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean };

  constructor(store: { read(): Promise<string | null> | string | null; write(value: string): Promise<void> | void; writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean }) {
    super(store);
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional evaluator calibration store requires writeIfUnchanged");
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): Promise<boolean> {
    const validated = validateSnapshot(snapshot);
    const encoded = canonicalJson(validated);
    const committed = await this.transactionalStore.writeIfUnchanged(expectedSnapshotDigest, encoded);
    if (typeof committed !== "boolean") throw new ArgumentError("transactional evaluator calibration store returned a non-boolean result");
    return committed;
  }
}

/** Deterministic registry that carries validated evaluator reports through restarts. */
export class AutonomousEvaluatorCalibrationRegistry {
  private readonly reports = new Map<string, AutonomousEvaluatorCalibrationReport>();
  private generationValue = 0;

  constructor(reports: readonly AutonomousEvaluatorCalibrationReport[] = []) {
    if (!Array.isArray(reports) || reports.length > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORED_REPORTS) throw new ArgumentError("evaluator calibration registry reports exceed their bound");
    for (const report of reports) this.import(report);
    this.generationValue = this.reports.size;
  }

  get generation(): number {
    return this.generationValue;
  }

  get size(): number {
    return this.reports.size;
  }

  import(report: AutonomousEvaluatorCalibrationReport): AutonomousEvaluatorCalibrationImport {
    const validated = validateAutonomousEvaluatorCalibrationReport(report);
    const digestValue = validated.report_digest;
    const existing = this.reports.get(digestValue);
    if (existing) {
      if (JSON.stringify(existing) !== JSON.stringify(validated)) throw new ArgumentError("evaluator calibration report digest collision detected");
      return importProjection(existing, false, this.generationValue, [...this.reports.keys()]);
    }
    if (this.reports.size >= MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORED_REPORTS) throw new ArgumentError("evaluator calibration registry is full");
    this.reports.set(digestValue, structuredClone(validated));
    this.generationValue += 1;
    return importProjection(validated, true, this.generationValue, [...this.reports.keys()]);
  }

  get(reportDigest: string): AutonomousEvaluatorCalibrationReport | null {
    const digestValue = digest("evaluator calibration report digest", reportDigest);
    const report = this.reports.get(digestValue);
    return report ? structuredClone(report) : null;
  }

  query(options: AutonomousEvaluatorCalibrationQueryOptions = {}): AutonomousEvaluatorCalibrationReport[] {
    if (options.domain !== undefined && !AUTONOMOUS_DOMAIN_NAMES.includes(options.domain)) throw new ArgumentError("evaluator calibration query domain is unsupported");
    if (options.status !== undefined && !["ready", "insufficient_coverage", "insufficient_evidence", "miscalibrated"].includes(options.status)) throw new ArgumentError("evaluator calibration query status is invalid");
    if (options.decision !== undefined && options.decision !== "admit_learning" && options.decision !== "hold_learning") throw new ArgumentError("evaluator calibration query decision is invalid");
    const limit = integer("evaluator calibration query limit", options.limit ?? MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORED_REPORTS, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_STORED_REPORTS);
    return [...this.reports.values()]
      .filter((report) => options.domain === undefined || report.target_domains.includes(options.domain) || report.domains.some((row) => row.domain === options.domain))
      .filter((report) => options.status === undefined || report.status === options.status)
      .filter((report) => options.decision === undefined || report.gate.decision === options.decision)
      .sort((left, right) => right.report_digest.localeCompare(left.report_digest))
      .slice(0, limit)
      .map((report) => structuredClone(report));
  }

  snapshot(): AutonomousEvaluatorCalibrationStoreSnapshot {
    return snapshotFrom([...this.reports.values()], this.generationValue);
  }

  restore(snapshot: AutonomousEvaluatorCalibrationStoreSnapshot): void {
    const validated = validateSnapshot(snapshot);
    const next = new Map<string, AutonomousEvaluatorCalibrationReport>();
    for (const report of validated.reports) next.set(report.report_digest, structuredClone(report));
    this.reports.clear();
    for (const [reportDigest, report] of next) this.reports.set(reportDigest, report);
    this.generationValue = validated.generation;
  }

  async flush(store: AutonomousEvaluatorCalibrationStore): Promise<AutonomousEvaluatorCalibrationStoreSnapshot> {
    if (!store || typeof store.write !== "function") throw new ArgumentError("evaluator calibration persistence adapter is malformed");
    const snapshot = this.snapshot();
    await store.write(snapshot);
    return snapshot;
  }

  async restoreFrom(store: AutonomousEvaluatorCalibrationStore): Promise<AutonomousEvaluatorCalibrationStoreSnapshot | null> {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("evaluator calibration persistence adapter is malformed");
    const snapshot = await store.read();
    if (snapshot === null) return null;
    this.restore(snapshot);
    return this.snapshot();
  }
}
