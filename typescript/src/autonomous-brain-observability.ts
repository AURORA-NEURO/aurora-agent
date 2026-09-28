/** Shared, payload-free trace registry, analytics, and alert controllers for the brain facade. */
import { ArgumentError, isObject } from "./errors.js";
import { type AutonomousRunTraceStore } from "./autonomous-run-trace.js";
import {
  AutonomousRunTraceRegistry,
  AutonomousRunTraceRegistryPersistenceCoordinator,
  JsonAutonomousRunTraceRegistryPersistence,
  publishAutonomousRunTraceRegistrySnapshot,
  type AutonomousRunTraceRegistryEventQuery,
  type AutonomousRunTraceRegistryImportReport,
  type AutonomousRunTraceRegistryIntegrity,
  type AutonomousRunTraceRegistryPage,
  type AutonomousRunTraceRegistryPublication,
  type AutonomousRunTraceRegistryQuery,
  type AutonomousRunTraceRegistryRecord,
  type AutonomousRunTraceRegistrySnapshot,
} from "./autonomous-run-trace-registry.js";
import {
  AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
  analyzeAutonomousRunTrace,
  type AutonomousRunTraceAnalyticsAlert,
  validateAutonomousRunTraceAnalyticsReport,
  type AutonomousRunTraceAnalyticsPolicy,
  type AutonomousRunTraceAnalyticsReport,
} from "./autonomous-run-analytics.js";
import {
  AutonomousRunAnalyticsLedger,
  AutonomousRunAnalyticsLedgerPersistenceCoordinator,
  JsonAutonomousRunAnalyticsLedgerPersistence,
  validateAutonomousRunAnalyticsLedgerSnapshot,
  type AutonomousRunAnalyticsLedgerEntry,
  type AutonomousRunAnalyticsLedgerIngestResult,
  type AutonomousRunAnalyticsLedgerPolicy,
  type AutonomousRunAnalyticsLedgerSummary,
  type AutonomousRunAnalyticsLedgerStatus,
} from "./autonomous-run-analytics-ledger.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";
import type { AutonomousBrainFacade } from "./autonomous-brain-facade.js";
import { isAutonomousBrainFacade } from "./autonomous-brain-facade-brand.js";
import { errorProjection } from "./autonomous-brain-facade-utils.js";
export const AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-brain-trace-registry-controller/0.1" as const;

export const AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-brain-run-analytics-controller/0.1" as const;

export const AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-brain-run-observability-controller/0.1" as const;

export const AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA = "bioprism-typescript-autonomous-brain-run-observability-alert/0.1" as const;

export type AutonomousBrainTraceRegistryControllerStatus =
  | "empty"
  | "restored"
  | "flushed"
  | "published"
  | "compacted"
  | "publication_failed"
  | "persistence_failed";

/** Construction controls for the facade-bound, metadata-only trace registry controller. */
export interface AutonomousBrainTraceRegistryControllerOptions {
  registry: AutonomousRunTraceRegistry;
  persistence: JsonAutonomousRunTraceRegistryPersistence;
}

/** Operator-safe projection of the trace registry controller state. */
export interface AutonomousBrainTraceRegistryControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA;
  status: AutonomousBrainTraceRegistryControllerStatus;
  snapshot_generation: number | null;
  snapshot_digest: string | null;
  runs: number;
  events: number;
  retained_event_count: number;
  policy: AutonomousRunTraceRegistrySnapshot["policy"] | null;
  persisted: boolean;
  retention: AutonomousRunTraceRegistrySnapshot["retention"];
  authority: AutonomousRunTraceRegistrySnapshot["authority"];
  secret_material: AutonomousRunTraceRegistrySnapshot["secret_material"];
}

/** Publication result with persistence separated from in-memory registry ingestion. */
export interface AutonomousBrainTraceRegistryPublicationRun extends JsonObject {
  controller: AutonomousBrainTraceRegistryControllerProjection;
  publication: AutonomousRunTraceRegistryPublication;
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Imported trace snapshot with an explicit persistence outcome. */
export interface AutonomousBrainTraceRegistryImportRun extends JsonObject {
  controller: AutonomousBrainTraceRegistryControllerProjection;
  report: AutonomousRunTraceRegistryImportReport;
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Retention-compaction result with an explicit persistence outcome. */
export interface AutonomousBrainTraceRegistryCompactRun extends JsonObject {
  controller: AutonomousBrainTraceRegistryControllerProjection;
  evicted_run_ids: string[];
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

export type AutonomousBrainRunAnalyticsControllerStatus =
  | "empty"
  | "restored"
  | "flushed"
  | "ingested"
  | "persistence_failed";

/** Construction controls for the facade-bound, metadata-only longitudinal analytics controller. */
export interface AutonomousBrainRunAnalyticsControllerOptions {
  ledger: AutonomousRunAnalyticsLedger;
  persistence: JsonAutonomousRunAnalyticsLedgerPersistence;
}

/** Operator-safe projection of the longitudinal analytics controller state. */
export interface AutonomousBrainRunAnalyticsControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA;
  status: AutonomousBrainRunAnalyticsControllerStatus;
  snapshot_generation: number;
  snapshot_digest: string;
  summary: AutonomousRunAnalyticsLedgerSummary;
  policy: AutonomousRunAnalyticsLedgerPolicy;
  persisted: boolean;
  retention: AutonomousRunAnalyticsLedgerSummary["retention"];
  authority: AutonomousRunAnalyticsLedgerSummary["authority"];
  secret_material: AutonomousRunAnalyticsLedgerSummary["secret_material"];
}

/** One report ingestion with explicit in-memory and persistence outcomes. */
export interface AutonomousBrainRunAnalyticsIngestRun extends JsonObject {
  controller: AutonomousBrainRunAnalyticsControllerProjection;
  ingest: AutonomousRunAnalyticsLedgerIngestResult;
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Trace analysis followed by ledger ingestion, preserving the verified report boundary. */
export interface AutonomousBrainRunAnalyticsAnalysisRun extends JsonObject {
  controller: AutonomousBrainRunAnalyticsControllerProjection;
  report: AutonomousRunTraceAnalyticsReport;
  ingest: AutonomousRunAnalyticsLedgerIngestResult;
  persisted: boolean;
  persistence_error: { error_class: string; failure_code: string } | null;
}

/** Digest and count projection returned after revalidating the complete ledger snapshot. */
export interface AutonomousBrainRunAnalyticsIntegrity extends JsonObject {
  verified: true;
  snapshot_generation: number;
  snapshot_digest: string;
  summary_digest: string;
  report_count: number;
  retention: AutonomousRunAnalyticsLedgerSummary["retention"];
  authority: AutonomousRunAnalyticsLedgerSummary["authority"];
  secret_material: AutonomousRunAnalyticsLedgerSummary["secret_material"];
}

export type AutonomousBrainRunObservabilityControllerStatus =
  | "empty"
  | "restored"
  | "flushed"
  | "published_and_analyzed"
  | "source_snapshot_failed"
  | "trace_publication_failed"
  | "analytics_failed"
  | "alert_delivery_failed"
  | "persistence_partial";

/** Existing facade-bound controllers composed by the application lifecycle supervisor. */
export interface AutonomousBrainRunObservabilityControllerOptions {
  traceRegistry: AutonomousBrainTraceRegistryController;
  runAnalytics: AutonomousBrainRunAnalyticsController;
  alertSink?: AutonomousBrainRunObservabilityAlertSink;
}

/** Caller-owned metadata sink; it must use `alert_id` as its downstream idempotency key. */
export interface AutonomousBrainRunObservabilityAlertSink {
  publish(events: readonly AutonomousBrainRunObservabilityAlert[]): Promise<void> | void;
}

/** Redacted alert envelope derived only from the verified analytics report. */
export interface AutonomousBrainRunObservabilityAlert extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA;
  alert_id: string;
  source_snapshot_digest: string;
  report_digest: string;
  code: AutonomousRunTraceAnalyticsAlert["code"];
  severity: AutonomousRunTraceAnalyticsAlert["severity"];
  scope: AutonomousRunTraceAnalyticsAlert["scope"];
  identity: AutonomousRunTraceAnalyticsAlert["identity"];
  detail: AutonomousRunTraceAnalyticsAlert["detail"];
  observed_value: number | null;
  threshold: number | null;
  retention: typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousBrainRunObservabilityAlertDelivery extends JsonObject {
  status: "not_configured" | "not_needed" | "delivered" | "failed";
  attempted: number;
  delivered: number;
  error: { error_class: string; failure_code: string } | null;
}

/** Operator-safe projection of both metadata-only stores and their shared lifecycle state. */
export interface AutonomousBrainRunObservabilityControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA;
  status: AutonomousBrainRunObservabilityControllerStatus;
  ready: boolean;
  persisted: boolean;
  trace_registry: AutonomousBrainTraceRegistryControllerProjection | null;
  run_analytics: AutonomousBrainRunAnalyticsControllerProjection | null;
  last_run_id: string | null;
  last_source_snapshot_digest: string | null;
}

export interface AutonomousBrainRunObservabilityRestoreRun extends JsonObject {
  controller: AutonomousBrainRunObservabilityControllerProjection;
}

export interface AutonomousBrainRunObservabilityFlushRun extends JsonObject {
  controller: AutonomousBrainRunObservabilityControllerProjection;
  persisted: boolean;
  persistence_errors: Array<{ scope: "trace_registry" | "run_analytics"; error_class: string; failure_code: string }>;
}

export interface AutonomousBrainRunObservabilityError extends JsonObject {
  scope: "source_snapshot" | "trace_publication" | "trace_persistence" | "analytics" | "analytics_persistence" | "alert_delivery";
  error_class: string;
  failure_code: string;
}

/** One shared-snapshot run with independent trace and analytics outcomes. */
export interface AutonomousBrainRunObservabilityRun extends JsonObject {
  controller: AutonomousBrainRunObservabilityControllerProjection;
  run_id: string;
  source_snapshot_digest: string | null;
  trace_registry: AutonomousBrainTraceRegistryPublicationRun | null;
  run_analytics: AutonomousBrainRunAnalyticsAnalysisRun | null;
  alert_delivery: AutonomousBrainRunObservabilityAlertDelivery;
  errors: AutonomousBrainRunObservabilityError[];
}

/**
 * Own the application lifecycle around the metadata-only trace registry projection.
 *
 * A trace journal remains the source of lifecycle events; this controller only indexes validated
 * summaries and bounded event metadata. Restore is mandatory before reads or publication, all
 * mutations are serialized, and a persistence failure is returned separately from a successful
 * in-memory publication so operators never mistake an observability failure for an execution
 * failure or trigger an unsafe provider retry.
 */
export class AutonomousBrainTraceRegistryController {
  private readonly persistenceCoordinator: AutonomousRunTraceRegistryPersistenceCoordinator;
  private restored = false;
  private busy = false;
  private persisted = false;

  constructor(
    readonly brain: AutonomousBrainFacade,
    readonly registry: AutonomousRunTraceRegistry,
    readonly persistence: JsonAutonomousRunTraceRegistryPersistence,
  ) {
    if (!(isAutonomousBrainFacade(brain))) throw new ArgumentError("autonomous brain trace registry controller requires an AutonomousBrainFacade");
    if (!(registry instanceof AutonomousRunTraceRegistry)) throw new ArgumentError("autonomous brain trace registry controller requires an AutonomousRunTraceRegistry");
    if (!(persistence instanceof JsonAutonomousRunTraceRegistryPersistence)) throw new ArgumentError("autonomous brain trace registry controller requires JSON registry persistence");
    this.persistenceCoordinator = new AutonomousRunTraceRegistryPersistenceCoordinator(registry, persistence);
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("autonomous brain trace registry controller must restore before use");
  }

  private requireIdle(): void {
    if (this.busy) throw new ArgumentError("autonomous brain trace registry controller already has an operation in progress");
  }

  private projection(status: AutonomousBrainTraceRegistryControllerStatus): AutonomousBrainTraceRegistryControllerProjection {
    const snapshot = this.registry.snapshot();
    return {
      schema: AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA,
      status,
      snapshot_generation: snapshot.snapshot_generation,
      snapshot_digest: snapshot.snapshot_digest,
      runs: snapshot.record_count,
      events: snapshot.event_count,
      retained_event_count: snapshot.retained_event_count,
      policy: structuredClone(snapshot.policy),
      persisted: this.persisted,
      retention: snapshot.retention,
      authority: snapshot.authority,
      secret_material: snapshot.secret_material,
    };
  }

  /** Restore and validate the last registry snapshot before any operator read or mutation. */
  async restore(): Promise<AutonomousBrainTraceRegistryControllerProjection> {
    this.requireIdle();
    const snapshot = await this.persistenceCoordinator.restore();
    this.restored = true;
    this.persisted = snapshot !== null;
    return this.projection(snapshot === null ? "empty" : "restored");
  }

  /** Flush the verified registry through its caller-owned JSON/CAS persistence adapter. */
  async flush(): Promise<AutonomousBrainTraceRegistryControllerProjection> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      await this.persistenceCoordinator.flush();
      this.persisted = true;
      return this.projection("flushed");
    } finally {
      this.busy = false;
    }
  }

  /** Publish one trace journal into the registry and persist it without coupling to execution. */
  async publish(traceStore: AutonomousRunTraceStore, runId: string): Promise<AutonomousBrainTraceRegistryPublicationRun> {
    this.requireRestored();
    this.requireIdle();
    if (!traceStore || typeof traceStore.snapshot !== "function") throw new ArgumentError("autonomous brain trace registry publication requires a trace store");
    this.busy = true;
    try {
      const publication = await publishAutonomousRunTraceRegistrySnapshot(this.registry, traceStore, runId);
      if (publication.status === "failed") {
        return {
          controller: this.projection("publication_failed"),
          publication,
          persisted: this.persisted,
          persistence_error: null,
        };
      }
      this.persisted = false;
      try {
        await this.persistenceCoordinator.flush();
        this.persisted = true;
        return {
          controller: this.projection("published"),
          publication,
          persisted: true,
          persistence_error: null,
        };
      } catch (error) {
        return {
          controller: this.projection("persistence_failed"),
          publication,
          persisted: false,
          persistence_error: errorProjection(error),
        };
      }
    } finally {
      this.busy = false;
    }
  }

  /**
   * Publish an already-captured snapshot without reading the source journal a second time.
   * This is used by coordinated application lifecycles to keep registry and analytics digests
   * bound to one source snapshot.
   */
  async publishSnapshot(snapshot: unknown, runId: string): Promise<AutonomousBrainTraceRegistryPublicationRun> {
    return this.publish({ snapshot: () => snapshot } as AutonomousRunTraceStore, runId);
  }

  /** Import a validated trace snapshot, then persist the resulting retention projection. */
  async importSnapshot(raw: unknown): Promise<AutonomousBrainTraceRegistryImportRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      const report = this.registry.importSnapshot(raw);
      this.persisted = false;
      try {
        await this.persistenceCoordinator.flush();
        this.persisted = true;
        return { controller: this.projection("published"), report, persisted: true, persistence_error: null };
      } catch (error) {
        return { controller: this.projection("persistence_failed"), report, persisted: false, persistence_error: errorProjection(error) };
      }
    } finally {
      this.busy = false;
    }
  }

  /** Compact eligible terminal records and persist the new bounded retention projection. */
  async compact(): Promise<AutonomousBrainTraceRegistryCompactRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      const compacted = this.registry.compact();
      this.persisted = false;
      try {
        await this.persistenceCoordinator.flush();
        this.persisted = true;
        return { controller: this.projection("compacted"), evicted_run_ids: compacted.evicted_run_ids, persisted: true, persistence_error: null };
      } catch (error) {
        return { controller: this.projection("persistence_failed"), evicted_run_ids: compacted.evicted_run_ids, persisted: false, persistence_error: errorProjection(error) };
      }
    } finally {
      this.busy = false;
    }
  }

  /** Return one cloned metadata record after restore and idle checks. */
  get(runId: string): AutonomousRunTraceRegistryRecord | null {
    this.requireRestored();
    this.requireIdle();
    return this.registry.get(runId);
  }

  /** Query bounded run summaries and retained event metadata. */
  query(query: AutonomousRunTraceRegistryQuery = {}): AutonomousRunTraceRegistryPage {
    this.requireRestored();
    this.requireIdle();
    return this.registry.query(query);
  }

  /** Query retained lifecycle events without exposing source trace payloads. */
  events(query: AutonomousRunTraceRegistryEventQuery = {}): ReturnType<AutonomousRunTraceRegistry["events"]> {
    this.requireRestored();
    this.requireIdle();
    return this.registry.events(query);
  }

  /** Return the canonical registry snapshot for caller-owned export or inspection. */
  snapshot(): AutonomousRunTraceRegistrySnapshot {
    this.requireRestored();
    this.requireIdle();
    return this.registry.snapshot();
  }

  /** Revalidate every record, digest, count, retention, and snapshot lineage invariant. */
  verifyIntegrity(): AutonomousRunTraceRegistryIntegrity {
    this.requireRestored();
    this.requireIdle();
    return this.registry.verifyIntegrity();
  }
}

/**
 * Own the application lifecycle around the metadata-only longitudinal analytics ledger.
 *
 * Analysis remains a pure operation over a caller-provided verified trace snapshot. Only the
 * resulting digest-bound report enters the ledger. Reads are fail-closed until restore, all
 * mutations are serialized, and a persistence failure is separated from a successful in-memory
 * ingestion so retry logic cannot accidentally re-run a provider task.
 */
export class AutonomousBrainRunAnalyticsController {
  private readonly persistenceCoordinator: AutonomousRunAnalyticsLedgerPersistenceCoordinator;
  private restored = false;
  private busy = false;
  private persisted = false;

  constructor(
    readonly brain: AutonomousBrainFacade,
    readonly ledger: AutonomousRunAnalyticsLedger,
    readonly persistence: JsonAutonomousRunAnalyticsLedgerPersistence,
  ) {
    if (!(isAutonomousBrainFacade(brain))) throw new ArgumentError("autonomous brain run analytics controller requires an AutonomousBrainFacade");
    if (!(ledger instanceof AutonomousRunAnalyticsLedger)) throw new ArgumentError("autonomous brain run analytics controller requires an AutonomousRunAnalyticsLedger");
    if (!(persistence instanceof JsonAutonomousRunAnalyticsLedgerPersistence)) throw new ArgumentError("autonomous brain run analytics controller requires JSON analytics ledger persistence");
    this.persistenceCoordinator = new AutonomousRunAnalyticsLedgerPersistenceCoordinator(ledger, persistence);
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("autonomous brain run analytics controller must restore before use");
  }

  private requireIdle(): void {
    if (this.busy) throw new ArgumentError("autonomous brain run analytics controller already has an operation in progress");
  }

  private projection(status: AutonomousBrainRunAnalyticsControllerStatus): AutonomousBrainRunAnalyticsControllerProjection {
    const snapshot = this.ledger.snapshot();
    const summary = this.ledger.summary();
    return {
      schema: AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA,
      status,
      snapshot_generation: snapshot.generation as number,
      snapshot_digest: snapshot.snapshot_digest as string,
      summary,
      policy: structuredClone(this.ledger.policy),
      persisted: this.persisted,
      retention: summary.retention,
      authority: summary.authority,
      secret_material: summary.secret_material,
    };
  }

  /** Restore and validate the last analytics snapshot before any operator read or mutation. */
  async restore(): Promise<AutonomousBrainRunAnalyticsControllerProjection> {
    this.requireIdle();
    this.busy = true;
    try {
      const snapshot = await this.persistenceCoordinator.restore();
      this.restored = true;
      this.persisted = snapshot !== null;
      return this.projection(snapshot === null ? "empty" : "restored");
    } finally {
      this.busy = false;
    }
  }

  /** Flush the verified ledger through its caller-owned JSON/CAS persistence adapter. */
  async flush(): Promise<AutonomousBrainRunAnalyticsControllerProjection> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      await this.persistenceCoordinator.flush();
      this.persisted = true;
      return this.projection("flushed");
    } finally {
      this.busy = false;
    }
  }

  private async ingestReport(raw: unknown, ingestedAt?: number): Promise<AutonomousBrainRunAnalyticsIngestRun> {
    const report = validateAutonomousRunTraceAnalyticsReport(raw);
    const ingest = ingestedAt === undefined ? this.ledger.ingest(report) : this.ledger.ingest(report, { ingestedAt });
    if (ingest.status !== "accepted") {
      return { controller: this.projection("ingested"), ingest, persisted: this.persisted, persistence_error: null };
    }
    this.persisted = false;
    try {
      await this.persistenceCoordinator.flush();
      this.persisted = true;
      return { controller: this.projection("ingested"), ingest, persisted: true, persistence_error: null };
    } catch (error) {
      return { controller: this.projection("persistence_failed"), ingest, persisted: false, persistence_error: errorProjection(error) };
    }
  }

  /** Ingest one already-verified analytics report and persist accepted state. */
  async ingest(raw: unknown, options: { ingestedAt?: number } = {}): Promise<AutonomousBrainRunAnalyticsIngestRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      if (!isObject(options) || Array.isArray(options)) throw new ArgumentError("autonomous brain run analytics ingestion options are malformed");
      return await this.ingestReport(raw, options.ingestedAt as number | undefined);
    } finally {
      this.busy = false;
    }
  }

  /** Analyze a verified trace snapshot, retain only its safe report, and persist it atomically. */
  async analyzeAndIngest(
    snapshot: unknown,
    options: { policy?: Partial<AutonomousRunTraceAnalyticsPolicy>; ingestedAt?: number } = {},
  ): Promise<AutonomousBrainRunAnalyticsAnalysisRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    try {
      if (!isObject(options) || Array.isArray(options)) throw new ArgumentError("autonomous brain run analytics analysis options are malformed");
      const { ingestedAt, ...analysisOptions } = options;
      const report = analyzeAutonomousRunTrace(snapshot, analysisOptions);
      const outcome = await this.ingestReport(report, ingestedAt);
      return { ...outcome, report };
    } finally {
      this.busy = false;
    }
  }

  /** Return the current longitudinal summary after restore and idle checks. */
  summary(): AutonomousRunAnalyticsLedgerSummary {
    this.requireRestored();
    this.requireIdle();
    return this.ledger.summary();
  }

  /** Return bounded retained reports, newest first, without exposing source traces. */
  history(options: { limit?: number; status?: AutonomousRunAnalyticsLedgerStatus } = {}): AutonomousRunAnalyticsLedgerEntry[] {
    this.requireRestored();
    this.requireIdle();
    return this.ledger.history(options);
  }

  /** Return the canonical ledger snapshot for caller-owned export or inspection. */
  snapshot(): Record<string, unknown> {
    this.requireRestored();
    this.requireIdle();
    return this.ledger.snapshot();
  }

  /** Revalidate every retained report, entry digest, count, retention marker, and lineage field. */
  verifyIntegrity(): AutonomousBrainRunAnalyticsIntegrity {
    this.requireRestored();
    this.requireIdle();
    const snapshot = validateAutonomousRunAnalyticsLedgerSnapshot(this.ledger.snapshot());
    const summary = this.ledger.summary();
    return {
      verified: true,
      snapshot_generation: snapshot.generation as number,
      snapshot_digest: snapshot.snapshot_digest as string,
      summary_digest: summary.summary_digest,
      report_count: summary.report_count,
      retention: summary.retention,
      authority: summary.authority,
      secret_material: summary.secret_material,
    };
  }
}

/**
 * Coordinate the application lifecycle for trace indexing and longitudinal run analytics.
 *
 * The source journal is read exactly once per coordinated publication. Both consumers receive
 * that same verified snapshot, so an append racing with a second read cannot produce a registry
 * record whose digest differs from the analytics report. The two persistence adapters remain
 * independent: a failure in one is surfaced as a bounded partial outcome and never causes the
 * completed provider task to be replayed.
 */
export class AutonomousBrainRunObservabilityController {
  private busy = false;
  private restored = false;
  private persisted = false;
  private lastRunId: string | null = null;
  private lastSourceSnapshotDigest: string | null = null;
  private lastTraceProjection: AutonomousBrainTraceRegistryControllerProjection | null = null;
  private lastAnalyticsProjection: AutonomousBrainRunAnalyticsControllerProjection | null = null;

  constructor(
    readonly brain: AutonomousBrainFacade,
    readonly traceRegistry: AutonomousBrainTraceRegistryController,
    readonly runAnalytics: AutonomousBrainRunAnalyticsController,
    readonly alertSink?: AutonomousBrainRunObservabilityAlertSink,
  ) {
    if (!(isAutonomousBrainFacade(brain))) throw new ArgumentError("autonomous brain run observability controller requires an AutonomousBrainFacade");
    if (!(traceRegistry instanceof AutonomousBrainTraceRegistryController) || !(runAnalytics instanceof AutonomousBrainRunAnalyticsController)) throw new ArgumentError("autonomous brain run observability controller requires facade-bound metadata controllers");
    if (traceRegistry.brain !== brain || runAnalytics.brain !== brain) throw new ArgumentError("autonomous brain run observability controllers must belong to the same facade");
    if (alertSink !== undefined && (!isObject(alertSink) || typeof alertSink.publish !== "function")) throw new ArgumentError("autonomous brain run observability alert sink is malformed");
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("autonomous brain run observability controller must restore before use");
  }

  private requireIdle(): void {
    if (this.busy) throw new ArgumentError("autonomous brain run observability controller already has an operation in progress");
  }

  private projection(
    status: AutonomousBrainRunObservabilityControllerStatus,
    traceRegistry: AutonomousBrainTraceRegistryControllerProjection | null = this.lastTraceProjection,
    runAnalytics: AutonomousBrainRunAnalyticsControllerProjection | null = this.lastAnalyticsProjection,
  ): AutonomousBrainRunObservabilityControllerProjection {
    return {
      schema: AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA,
      status,
      ready: this.restored,
      persisted: this.persisted,
      trace_registry: traceRegistry === null ? null : structuredClone(traceRegistry),
      run_analytics: runAnalytics === null ? null : structuredClone(runAnalytics),
      last_run_id: this.lastRunId,
      last_source_snapshot_digest: this.lastSourceSnapshotDigest,
    };
  }

  private async deliverAlerts(report: AutonomousRunTraceAnalyticsReport | null): Promise<AutonomousBrainRunObservabilityAlertDelivery> {
    if (this.alertSink === undefined) return { status: "not_configured", attempted: 0, delivered: 0, error: null };
    if (report === null || report.alerts.length === 0) return { status: "not_needed", attempted: 0, delivered: 0, error: null };
    const events: AutonomousBrainRunObservabilityAlert[] = report.alerts.map((alert) => {
      const identity = { source_snapshot_digest: report.source_snapshot_digest, report_digest: report.report_digest, code: alert.code, severity: alert.severity, scope: alert.scope, identity: alert.identity };
      return {
        schema: AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA,
        alert_id: digestJsonSync(identity),
        source_snapshot_digest: report.source_snapshot_digest,
        report_digest: report.report_digest,
        code: alert.code,
        severity: alert.severity,
        scope: alert.scope,
        identity: alert.identity,
        detail: alert.detail,
        observed_value: alert.observed_value,
        threshold: alert.threshold,
        retention: AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
        secret_material: "never_returned",
      };
    });
    try {
      await this.alertSink.publish(events);
      return { status: "delivered", attempted: events.length, delivered: events.length, error: null };
    } catch (error) {
      return { status: "failed", attempted: events.length, delivered: 0, error: errorProjection(error) };
    }
  }

  /** Restore both metadata projections before exposing the coordinated lifecycle. */
  async restore(): Promise<AutonomousBrainRunObservabilityRestoreRun> {
    this.requireIdle();
    this.busy = true;
    try {
      this.restored = false;
      const traceRegistry = await this.traceRegistry.restore();
      const runAnalytics = await this.runAnalytics.restore();
      this.lastTraceProjection = traceRegistry;
      this.lastAnalyticsProjection = runAnalytics;
      this.persisted = traceRegistry.persisted && runAnalytics.persisted;
      this.restored = true;
      const status = traceRegistry.status === "empty" && runAnalytics.status === "empty" ? "empty" : "restored";
      return { controller: this.projection(status, traceRegistry, runAnalytics) };
    } finally {
      this.busy = false;
    }
  }

  /** Flush each store independently and report exactly which persistence boundary failed. */
  async flush(): Promise<AutonomousBrainRunObservabilityFlushRun> {
    this.requireRestored();
    this.requireIdle();
    this.busy = true;
    const persistenceErrors: AutonomousBrainRunObservabilityFlushRun["persistence_errors"] = [];
    try {
      let traceRegistry = this.lastTraceProjection;
      let runAnalytics = this.lastAnalyticsProjection;
      try {
        traceRegistry = await this.traceRegistry.flush();
        this.lastTraceProjection = traceRegistry;
      } catch (error) {
        const failure = errorProjection(error);
        persistenceErrors.push({ scope: "trace_registry", ...failure });
      }
      try {
        runAnalytics = await this.runAnalytics.flush();
        this.lastAnalyticsProjection = runAnalytics;
      } catch (error) {
        const failure = errorProjection(error);
        persistenceErrors.push({ scope: "run_analytics", ...failure });
      }
      this.persisted = persistenceErrors.length === 0;
      return {
        controller: this.projection(persistenceErrors.length === 0 ? "flushed" : "persistence_partial", traceRegistry, runAnalytics),
        persisted: this.persisted,
        persistence_errors: persistenceErrors,
      };
    } finally {
      this.busy = false;
    }
  }

  /**
   * Publish and analyze one immutable source snapshot. The trace store is intentionally read
   * once; the registry controller's snapshot-only path prevents a second, potentially newer
   * journal read from drifting away from the report digest.
   */
  async publishAndAnalyze(
    traceStore: AutonomousRunTraceStore,
    runId: string,
    options: { policy?: Partial<AutonomousRunTraceAnalyticsPolicy>; ingestedAt?: number } = {},
  ): Promise<AutonomousBrainRunObservabilityRun> {
    this.requireRestored();
    this.requireIdle();
    if (!traceStore || typeof traceStore.snapshot !== "function") throw new ArgumentError("autonomous brain run observability publication requires a trace store");
    if (typeof runId !== "string" || !/^[A-Za-z0-9_.:-]{1,256}$/.test(runId)) throw new ArgumentError("autonomous brain run observability run_id must be a bounded identifier");
    this.busy = true;
    const errors: AutonomousBrainRunObservabilityError[] = [];
    let sourceSnapshotDigest: string | null = null;
    let traceOutcome: AutonomousBrainTraceRegistryPublicationRun | null = null;
    let analyticsOutcome: AutonomousBrainRunAnalyticsAnalysisRun | null = null;
    try {
      let sourceSnapshot: unknown;
      try {
        sourceSnapshot = await traceStore.snapshot();
        if (isObject(sourceSnapshot) && typeof sourceSnapshot.snapshot_digest === "string" && /^[0-9a-f]{64}$/.test(sourceSnapshot.snapshot_digest)) sourceSnapshotDigest = sourceSnapshot.snapshot_digest;
      } catch (error) {
        errors.push({ scope: "source_snapshot", ...errorProjection(error) });
        return {
          controller: this.projection("source_snapshot_failed"),
          run_id: runId,
          source_snapshot_digest: sourceSnapshotDigest,
          trace_registry: null,
          run_analytics: null,
          alert_delivery: { status: "not_needed", attempted: 0, delivered: 0, error: null },
          errors,
        };
      }

      traceOutcome = await this.traceRegistry.publishSnapshot(sourceSnapshot, runId);
      this.lastTraceProjection = traceOutcome.controller;
      if (traceOutcome.publication.status === "failed") {
        errors.push({
          scope: "trace_publication",
          error_class: traceOutcome.publication.error_class ?? "AutonomousRunTraceRegistryPublicationError",
          failure_code: traceOutcome.publication.failure_code ?? "trace_registry_publication_failed",
        });
      } else if (traceOutcome.persistence_error !== null) {
        errors.push({ scope: "trace_persistence", ...traceOutcome.persistence_error });
      }

      try {
        analyticsOutcome = await this.runAnalytics.analyzeAndIngest(sourceSnapshot, options);
        this.lastAnalyticsProjection = analyticsOutcome.controller;
        if (analyticsOutcome.persistence_error !== null) errors.push({ scope: "analytics_persistence", ...analyticsOutcome.persistence_error });
      } catch (error) {
        errors.push({ scope: "analytics", ...errorProjection(error) });
      }
      const alertDelivery = await this.deliverAlerts(analyticsOutcome?.report ?? null);
      if (alertDelivery.error !== null) errors.push({ scope: "alert_delivery", ...alertDelivery.error });
      this.lastRunId = runId;
      this.lastSourceSnapshotDigest = sourceSnapshotDigest;
      this.persisted = traceOutcome.persisted && analyticsOutcome?.persisted === true;
      const status: AutonomousBrainRunObservabilityControllerStatus = traceOutcome.publication.status === "failed"
          ? "trace_publication_failed"
          : analyticsOutcome === null
            ? "analytics_failed"
            : alertDelivery.status === "failed"
              ? "alert_delivery_failed"
            : errors.length > 0 && (traceOutcome.persisted === false || analyticsOutcome.persisted === false)
            ? "persistence_partial"
            : errors.length > 0
              ? "analytics_failed"
              : "published_and_analyzed";
      return {
        controller: this.projection(status, traceOutcome.controller, analyticsOutcome?.controller ?? this.lastAnalyticsProjection),
        run_id: runId,
        source_snapshot_digest: sourceSnapshotDigest,
        trace_registry: traceOutcome,
        run_analytics: analyticsOutcome,
        alert_delivery: alertDelivery,
        errors,
      };
    } finally {
      this.busy = false;
    }
  }

  /** Revalidate both metadata stores and return their digest-bound integrity projections. */
  verifyIntegrity(): { verified: true; trace_registry: AutonomousRunTraceRegistryIntegrity; run_analytics: AutonomousBrainRunAnalyticsIntegrity } {
    this.requireRestored();
    this.requireIdle();
    return {
      verified: true,
      trace_registry: this.traceRegistry.verifyIntegrity(),
      run_analytics: this.runAnalytics.verifyIntegrity(),
    };
  }
}
