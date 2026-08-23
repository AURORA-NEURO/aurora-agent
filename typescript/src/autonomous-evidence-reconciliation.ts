import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousEvidencePlan,
  type AutonomousEvidenceRequirement,
} from "./autonomous-evidence.js";
import {
  AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
  type AutonomousEvidenceAcquirer,
  type AutonomousEvidenceAcquisitionContext,
} from "./autonomous-evidence-runtime.js";
import { classifyAutonomousEvidenceAcquisitionError } from "./autonomous-evidence-retry.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Reviewed fan-out/fan-in source adjudication without retaining source values. */
export const AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA = "bioprism-typescript-autonomous-evidence-reconciliation-plan/0.1" as const;
export const AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA = "bioprism-typescript-autonomous-evidence-reconciliation-source/0.1" as const;
export const AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA = "bioprism-typescript-autonomous-evidence-reconciliation-result/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES = 16;
export const MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY = 8;
export const MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_METADATA_BYTES = 64_000;
export const MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_VALUE_BYTES = 64_000_000;
export const MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES = 512_000;

const RETENTION = "metadata_only;source_values_and_normalized_values_caller_owned" as const;
const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
  "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

export type AutonomousEvidenceReconciliationStatus = "consensus" | "consensus_with_dissent" | "disagreement" | "insufficient_evidence" | "failed";
export type AutonomousEvidenceReconciliationSourceStatus = "observed" | "failed";

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function text(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value.trim();
}

function identifier(name: string, value: unknown): string {
  const result = text(name, value, 256);
  if (!/^[A-Za-z0-9_.:+\-/ ]+$/.test(result)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return result;
}

function digest(name: string, value: unknown, required = true): string | null {
  if (value === undefined || value === null) {
    if (required) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
    return null;
  }
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value as number;
}

function marker(value: string): string {
  return [...value.toLowerCase()].filter((character) => /[a-z0-9]/.test(character)).join("");
}

function assertSafeMetadata(value: unknown, name: string, depth = 0): void {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((child, index) => assertSafeMetadata(child, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = marker(key);
      if (SECRET_FIELD_MARKERS.has(normalized) || normalized.includes("token") || normalized.includes("secret") || normalized.includes("credential")) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertSafeMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function safeMetadata(value: JsonObject | undefined, name: string): JsonObject {
  const result = value === undefined ? {} : value;
  if (!isObject(result)) throw new ArgumentError(`${name} must be a JSON object`);
  assertSafeMetadata(result, name);
  if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_METADATA_BYTES) throw new ArgumentError(`${name} exceeds its metadata byte bound`);
  return structuredClone(result) as JsonObject;
}

function safeJson(value: JsonValue, name: string): { value: JsonValue; bytes: number } {
  assertSafeMetadata(value, name);
  const encoded = canonicalJson(value);
  const size = bytes(encoded);
  if (size > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_VALUE_BYTES) throw new ArgumentError(`${name} exceeds its value byte bound`);
  return { value, bytes: size };
}

function list(name: string, value: readonly string[], maximum: number): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its bound`);
  const normalized = value.map((item, index) => identifier(`${name}[${index}]`, item));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} contains duplicates`);
  return [...normalized].sort();
}

function requirementFor(plan: AutonomousEvidencePlan, requirementId: string): AutonomousEvidenceRequirement {
  const normalized = identifier("evidence reconciliation requirement_id", requirementId);
  const requirement = plan.requirements.find((candidate) => candidate.requirement_id === normalized);
  if (!requirement) throw new ArgumentError(`evidence reconciliation requirement is not in the plan: ${normalized}`);
  return requirement;
}

export interface AutonomousEvidenceReconciliationRouteDescriptor extends JsonObject {
  source_id: string;
  source_digest: string | null;
  request_id: string | null;
  metadata: JsonObject;
}

export interface AutonomousEvidenceReconciliationRoute {
  source_id: string;
  source_digest: string | null;
  request_id: string | null;
  metadata: JsonObject;
  acquirer: AutonomousEvidenceAcquirer;
}

export interface AutonomousEvidenceReconciliationRouteJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA;
  source_id: string;
  source_digest: string | null;
  request_id: string | null;
  metadata_digest: string;
  execution: "planned_route_only;source_dispatch_not_started";
  retention: "metadata_only;request_metadata_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceReconciliationPlanJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA;
  evidence_plan_digest: string;
  requirement_id: string;
  domain: string;
  workflow_id: string;
  stage_id: string;
  route_count: number;
  routes: AutonomousEvidenceReconciliationRouteJSON[];
  quorum: number;
  max_concurrency: number;
  require_all: boolean;
  normalizer_id: string;
  normalizer_version: string;
  parent_evidence_digests: string[];
  plan_digest: string;
  approval_required: true;
  execution: "planning_only;source_dispatch_not_started";
  retention: "metadata_only;route_metadata_and_digests_only";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceReconciliationSourceJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA;
  source_id: string;
  source_digest: string | null;
  request_id: string | null;
  request_digest: string;
  metadata_digest: string;
  status: AutonomousEvidenceReconciliationSourceStatus;
  value_digest: string | null;
  value_bytes: number;
  normalized_digest: string | null;
  failure_class: string | null;
  retryable: boolean;
  limitations: string[];
  retention: "metadata_only;source_values_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceReconciliationResultJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA;
  evidence_plan_digest: string;
  requirement_id: string;
  domain: string;
  reconciliation_plan_digest: string;
  status: AutonomousEvidenceReconciliationStatus;
  route_count: number;
  observed_count: number;
  failed_count: number;
  unique_normalized_count: number;
  quorum: number;
  consensus_normalized_digest: string | null;
  disagreement_digest: string | null;
  source_results: AutonomousEvidenceReconciliationSourceJSON[];
  result_digest: string;
  retention: typeof RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousEvidenceReconciliationPrepareOptions {
  quorum?: number;
  maxConcurrency?: number;
  requireAll?: boolean;
  normalizerId?: string;
  normalizerVersion?: string;
  parentEvidenceDigests?: readonly string[];
}

export interface AutonomousEvidenceReconciliationExecuteOptions {
  approveSourceDispatch?: boolean;
  normalizer?: (value: JsonValue, context: AutonomousEvidenceAcquisitionContext) => JsonValue | Promise<JsonValue>;
  normalizerId?: string;
  normalizerVersion?: string;
}

interface RoutePayload {
  schema: typeof AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA;
  source_id: string;
  source_digest: string | null;
  request_id: string | null;
  metadata_digest: string;
  execution: "planned_route_only;source_dispatch_not_started";
  retention: "metadata_only;request_metadata_caller_owned";
  secret_material: "never_returned";
}

interface PlanPayload {
  schema: typeof AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA;
  evidence_plan_digest: string;
  requirement_id: string;
  domain: string;
  workflow_id: string;
  stage_id: string;
  route_count: number;
  routes: RoutePayload[];
  quorum: number;
  max_concurrency: number;
  require_all: boolean;
  normalizer_id: string;
  normalizer_version: string;
  parent_evidence_digests: string[];
  approval_required: true;
  execution: "planning_only;source_dispatch_not_started";
  retention: "metadata_only;route_metadata_and_digests_only";
  secret_material: "never_returned";
}

function routePayload(input: {
  sourceId: string;
  sourceDigest: string | null;
  requestId: string | null;
  metadataDigest: string;
}): RoutePayload {
  return {
    schema: AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA,
    source_id: input.sourceId,
    source_digest: input.sourceDigest,
    request_id: input.requestId,
    metadata_digest: input.metadataDigest,
    execution: "planned_route_only;source_dispatch_not_started",
    retention: "metadata_only;request_metadata_caller_owned",
    secret_material: "never_returned",
  };
}

function requestDigest(planDigest: string, requirementId: string, route: AutonomousEvidenceReconciliationRouteDescriptor): string {
  return digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    plan_digest: planDigest,
    requirement_id: requirementId,
    source_id: route.source_id,
    source_digest: route.source_digest,
    request_id: route.request_id,
    metadata: route.metadata,
  });
}

function routeDescriptor(route: AutonomousEvidenceReconciliationRoute): AutonomousEvidenceReconciliationRouteDescriptor {
  if (!route || typeof route !== "object" || typeof route.acquirer?.acquire !== "function") throw new ArgumentError("evidence reconciliation route is malformed");
  const sourceId = identifier("evidence reconciliation source_id", route.source_id);
  const sourceDigest = digest("evidence reconciliation source_digest", route.source_digest, false);
  const requestId = route.request_id === undefined || route.request_id === null ? null : identifier("evidence reconciliation request_id", route.request_id);
  const metadata = safeMetadata(route.metadata, "evidence reconciliation request metadata");
  return { source_id: sourceId, source_digest: sourceDigest, request_id: requestId, metadata };
}

function parentDigests(value: readonly string[] | undefined): string[] {
  if (!Array.isArray(value) || value.length > 64) throw new ArgumentError("evidence reconciliation parent evidence digests are outside their bound");
  return value.map((item, index) => digest(`evidence reconciliation parent_evidence_digests[${index}]`, item)!);
}

function routeJSON(route: RoutePayload): AutonomousEvidenceReconciliationRouteJSON {
  return { ...route };
}

function planPayloadDigest(payload: PlanPayload): string {
  return digestJsonSync(payload);
}

/** Digest-bound, request-free multi-source adjudication plan. */
export class AutonomousEvidenceReconciliationPlan {
  readonly evidence_plan_digest: string;
  readonly requirement_id: string;
  readonly domain: string;
  readonly workflow_id: string;
  readonly stage_id: string;
  readonly routes: AutonomousEvidenceReconciliationRouteJSON[];
  readonly quorum: number;
  readonly max_concurrency: number;
  readonly require_all: boolean;
  readonly normalizer_id: string;
  readonly normalizer_version: string;
  readonly parent_evidence_digests: string[];
  readonly plan_digest: string;

  constructor(input: {
    evidencePlan: AutonomousEvidencePlan;
    requirement: AutonomousEvidenceRequirement;
    routes: readonly AutonomousEvidenceReconciliationRoute[] | readonly AutonomousEvidenceReconciliationRouteJSON[];
    quorum: number;
    maxConcurrency: number;
    requireAll: boolean;
    normalizerId: string;
    normalizerVersion: string;
    parentEvidenceDigests: readonly string[];
  }) {
    if (!(input.evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence reconciliation plan requires a typed evidence plan");
    if (!input.requirement || input.requirement.domain === undefined) throw new ArgumentError("evidence reconciliation requirement is malformed");
    const requirement = requirementFor(input.evidencePlan, input.requirement.requirement_id);
    if (!Array.isArray(input.routes) || input.routes.length < 1 || input.routes.length > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES) throw new ArgumentError("evidence reconciliation routes are outside their bound");
    const routes = input.routes.map((route) => "acquirer" in route ? routePayloadFromRoute(route as AutonomousEvidenceReconciliationRoute) : validateRouteJSON(route as AutonomousEvidenceReconciliationRouteJSON));
    if (new Set(routes.map((route) => route.source_id)).size !== routes.length) throw new ArgumentError("evidence reconciliation source IDs must be unique");
    routes.sort((left, right) => left.source_id.localeCompare(right.source_id));
    this.evidence_plan_digest = digest("evidence reconciliation evidence plan digest", input.evidencePlan.plan_digest)!;
    this.requirement_id = requirement.requirement_id;
    this.domain = requirement.domain;
    this.workflow_id = requirement.workflow_id;
    this.stage_id = requirement.stage_id;
    this.routes = routes.map(routeJSON);
    this.quorum = integer("evidence reconciliation quorum", input.quorum, 1, routes.length);
    this.max_concurrency = integer("evidence reconciliation maxConcurrency", input.maxConcurrency, 1, Math.min(MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY, routes.length));
    if (typeof input.requireAll !== "boolean") throw new ArgumentError("evidence reconciliation requireAll must be boolean");
    this.require_all = input.requireAll;
    this.normalizer_id = identifier("evidence reconciliation normalizerId", input.normalizerId);
    this.normalizer_version = identifier("evidence reconciliation normalizerVersion", input.normalizerVersion);
    this.parent_evidence_digests = parentDigests(input.parentEvidenceDigests);
    const payload = this.planPayload();
    this.plan_digest = planPayloadDigest(payload);
  }

  verify(evidencePlan: AutonomousEvidencePlan): this {
    if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence reconciliation verification requires a typed evidence plan");
    if (evidencePlan.plan_digest !== this.evidence_plan_digest) throw new ArgumentError("evidence reconciliation evidence plan changed after planning");
    if (this.plan_digest !== planPayloadDigest(this.planPayload())) throw new ArgumentError("evidence reconciliation plan digest is invalid");
    return this;
  }

  toJSON(): AutonomousEvidenceReconciliationPlanJSON {
    const projection = { ...this.planPayload(), plan_digest: this.plan_digest } as AutonomousEvidenceReconciliationPlanJSON;
    if (bytes(canonicalJson(projection)) > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES) throw new ArgumentError("evidence reconciliation plan exceeds its byte bound");
    return projection;
  }

  private planPayload(): PlanPayload {
    return {
      schema: AUTONOMOUS_EVIDENCE_RECONCILIATION_PLAN_SCHEMA,
      evidence_plan_digest: this.evidence_plan_digest,
      requirement_id: this.requirement_id,
      domain: this.domain,
      workflow_id: this.workflow_id,
      stage_id: this.stage_id,
      route_count: this.routes.length,
      routes: this.routes.map((route) => ({ ...route })),
      quorum: this.quorum,
      max_concurrency: this.max_concurrency,
      require_all: this.require_all,
      normalizer_id: this.normalizer_id,
      normalizer_version: this.normalizer_version,
      parent_evidence_digests: [...this.parent_evidence_digests],
      approval_required: true,
      execution: "planning_only;source_dispatch_not_started",
      retention: "metadata_only;route_metadata_and_digests_only",
      secret_material: "never_returned",
    };
  }
}

function routePayloadFromRoute(route: AutonomousEvidenceReconciliationRoute): RoutePayload {
  const descriptor = routeDescriptor(route);
  return routePayload({
    sourceId: descriptor.source_id,
    sourceDigest: descriptor.source_digest,
    requestId: descriptor.request_id,
    metadataDigest: digestJsonSync(descriptor.metadata),
  });
}

function validateRouteJSON(route: AutonomousEvidenceReconciliationRouteJSON): RoutePayload {
  if (!isObject(route) || route.schema !== AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA) throw new ArgumentError("evidence reconciliation route projection is malformed");
  const sourceId = identifier("evidence reconciliation route source_id", route.source_id);
  const sourceDigest = digest("evidence reconciliation route source_digest", route.source_digest, false);
  const requestId = route.request_id === undefined || route.request_id === null ? null : identifier("evidence reconciliation route request_id", route.request_id);
  const metadataDigest = digest("evidence reconciliation route metadata_digest", route.metadata_digest)!;
  if (route.execution !== "planned_route_only;source_dispatch_not_started" || route.retention !== "metadata_only;request_metadata_caller_owned" || route.secret_material !== "never_returned") throw new ArgumentError("evidence reconciliation route retention is invalid");
  return routePayload({ sourceId, sourceDigest, requestId, metadataDigest });
}

interface SourceExecution {
  route: AutonomousEvidenceReconciliationRouteDescriptor;
  request_digest: string;
  status: AutonomousEvidenceReconciliationSourceStatus;
  value: JsonValue | null;
  normalized: JsonValue | null;
  result: AutonomousEvidenceReconciliationSourceJSON;
}

function sourceResultDescriptor(result: Omit<AutonomousEvidenceReconciliationSourceJSON, "schema" | "retention" | "secret_material">): AutonomousEvidenceReconciliationSourceJSON {
  return {
    schema: AUTONOMOUS_EVIDENCE_RECONCILIATION_SOURCE_SCHEMA,
    ...result,
    retention: "metadata_only;source_values_caller_owned",
    secret_material: "never_returned",
  } as AutonomousEvidenceReconciliationSourceJSON;
}

function concurrency(max: number, requested: number): number {
  return Math.min(max, Math.max(1, requested));
}

/**
 * High-level reviewed source adjudicator. It performs bounded fan-out, normalizes transient
 * values under an explicit caller-named contract, and returns consensus/dissent metadata while
 * leaving source truth and evaluator authority outside the transport boundary.
 */
export class AutonomousEvidenceSourceReconciler {
  constructor(readonly evidencePlan: AutonomousEvidencePlan) {
    if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence reconciler requires a typed evidence plan");
  }

  prepare(requirementId: string, routes: readonly AutonomousEvidenceReconciliationRoute[], options: AutonomousEvidenceReconciliationPrepareOptions = {}): AutonomousEvidenceReconciliationPlan {
    const requirement = requirementFor(this.evidencePlan, requirementId);
    if (!Array.isArray(routes) || routes.length < 1 || routes.length > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_ROUTES) throw new ArgumentError("evidence reconciliation routes are outside their bound");
    const quorum = options.quorum ?? (routes.length === 1 ? 1 : 2);
    const maxConcurrency = options.maxConcurrency ?? Math.min(routes.length, MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_CONCURRENCY);
    const normalizerId = options.normalizerId ?? "identity";
    const normalizerVersion = options.normalizerVersion ?? "1";
    if (normalizerId === "identity" && normalizerVersion !== "1") throw new ArgumentError("identity normalizer version must be 1");
    return new AutonomousEvidenceReconciliationPlan({
      evidencePlan: this.evidencePlan,
      requirement,
      routes,
      quorum,
      maxConcurrency,
      requireAll: options.requireAll ?? false,
      normalizerId,
      normalizerVersion,
      parentEvidenceDigests: options.parentEvidenceDigests ?? [],
    });
  }

  async execute(plan: AutonomousEvidenceReconciliationPlan, routes: readonly AutonomousEvidenceReconciliationRoute[], options: AutonomousEvidenceReconciliationExecuteOptions = {}): Promise<AutonomousEvidenceReconciliationResult> {
    if (!(plan instanceof AutonomousEvidenceReconciliationPlan)) throw new ArgumentError("evidence reconciliation execute requires a typed reconciliation plan");
    plan.verify(this.evidencePlan);
    if (options.approveSourceDispatch !== true) throw new ArgumentError("evidence reconciliation source dispatch requires explicit approval");
    if (!Array.isArray(routes) || routes.length !== plan.routes.length) throw new ArgumentError("evidence reconciliation execution routes do not match its plan");
    const descriptors = routes.map(routeDescriptor).sort((left, right) => left.source_id.localeCompare(right.source_id));
    const planned = [...plan.routes].sort((left, right) => left.source_id.localeCompare(right.source_id));
    for (let index = 0; index < planned.length; index += 1) {
      const route = descriptors[index]!;
      const expected = planned[index]!;
      if (route.source_id !== expected.source_id || route.source_digest !== expected.source_digest || route.request_id !== expected.request_id || digestJsonSync(route.metadata) !== expected.metadata_digest) throw new ArgumentError("evidence reconciliation execution route changed after planning");
    }
    const normalizerId = options.normalizerId ?? "identity";
    const normalizerVersion = options.normalizerVersion ?? "1";
    if (normalizerId !== plan.normalizer_id || normalizerVersion !== plan.normalizer_version) throw new ArgumentError("evidence reconciliation normalizer contract changed after planning");
    if (plan.normalizer_id !== "identity" && typeof options.normalizer !== "function") throw new ArgumentError("evidence reconciliation requires the planned normalizer callback");
    if (options.normalizer !== undefined && typeof options.normalizer !== "function") throw new ArgumentError("evidence reconciliation normalizer is malformed");
    const routeById = new Map(routes.map((route) => {
      const descriptor = routeDescriptor(route);
      return [descriptor.source_id, route] as const;
    }));
    const outputs: Array<SourceExecution | null> = Array.from({ length: descriptors.length }, () => null);
    let cursor = 0;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = cursor++;
        if (index >= descriptors.length) return;
        const route = descriptors[index]!;
        outputs[index] = await this.executeOne(plan, route, routeById.get(route.source_id)!, options.normalizer);
      }
    };
    await Promise.all(Array.from({ length: concurrency(descriptors.length, plan.max_concurrency) }, () => worker()));
    const executions = outputs.map((output) => output!).sort((left, right) => left.route.source_id.localeCompare(right.route.source_id));
    const observed = executions.filter((execution) => execution.status === "observed" && execution.normalized !== null);
    const failed = executions.filter((execution) => execution.status === "failed");
    const groups = new Map<string, { count: number; value: JsonValue; sources: string[] }>();
    for (const execution of observed) {
      const digestValue = execution.result.normalized_digest!;
      const group = groups.get(digestValue);
      if (group) group.sources.push(execution.route.source_id);
      else groups.set(digestValue, { count: 1, value: execution.normalized!, sources: [execution.route.source_id] });
      if (group) group.count += 1;
    }
    const ranked = [...groups.entries()].sort((left, right) => right[1].count - left[1].count || left[0].localeCompare(right[0]));
    const winner = ranked[0] ?? null;
    const status: AutonomousEvidenceReconciliationStatus = observed.length === 0
      ? "failed"
      : plan.require_all && failed.length > 0
        ? "insufficient_evidence"
        : observed.length < plan.quorum
          ? "insufficient_evidence"
          : !winner || winner[1].count < plan.quorum
            ? groups.size > 1 ? "disagreement" : "insufficient_evidence"
            : groups.size > 1 ? "consensus_with_dissent" : "consensus";
    const groupProjection = ranked.map(([normalizedDigest, group]) => ({ normalized_digest: normalizedDigest, count: group.count, source_ids: [...group.sources].sort() }));
    const descriptor = {
      schema: AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA,
      evidence_plan_digest: this.evidencePlan.plan_digest,
      requirement_id: plan.requirement_id,
      domain: plan.domain,
      reconciliation_plan_digest: plan.plan_digest,
      status,
      route_count: executions.length,
      observed_count: observed.length,
      failed_count: failed.length,
      unique_normalized_count: groups.size,
      quorum: plan.quorum,
      consensus_normalized_digest: winner && winner[1].count >= plan.quorum ? winner[0] : null,
      disagreement_digest: groups.size > 1 ? digestJsonSync(groupProjection) : null,
      source_results: executions.map((execution) => execution.result),
      retention: RETENTION,
      secret_material: "never_returned" as const,
    };
    const resultJson = { ...descriptor, result_digest: digestJsonSync(descriptor) } as AutonomousEvidenceReconciliationResultJSON;
    if (bytes(canonicalJson(resultJson)) > MAX_AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_BYTES) throw new ArgumentError("evidence reconciliation result exceeds its byte bound");
    const values: Record<string, JsonValue | null> = {};
    const normalizedValues: Record<string, JsonValue | null> = {};
    for (const execution of executions) {
      values[execution.route.source_id] = execution.value;
      normalizedValues[execution.route.source_id] = execution.normalized;
    }
    return new AutonomousEvidenceReconciliationResult(resultJson, values, normalizedValues);
  }

  private async executeOne(plan: AutonomousEvidenceReconciliationPlan, descriptor: AutonomousEvidenceReconciliationRouteDescriptor, route: AutonomousEvidenceReconciliationRoute, normalizer: AutonomousEvidenceReconciliationExecuteOptions["normalizer"]): Promise<SourceExecution> {
    const request: AutonomousEvidenceReconciliationRouteDescriptor = { ...descriptor, metadata: safeMetadata(descriptor.metadata, "evidence reconciliation execution metadata") };
    const requestDigestValue = requestDigest(this.evidencePlan.plan_digest, plan.requirement_id, request);
    const context: AutonomousEvidenceAcquisitionContext = {
      plan_digest: this.evidencePlan.plan_digest,
      requirement: requirementFor(this.evidencePlan, plan.requirement_id),
      request: { requirement_id: plan.requirement_id, source_id: request.source_id, source_digest: request.source_digest, request_id: request.request_id, metadata: request.metadata },
      attempt: 1,
      parent_evidence_digests: [...plan.parent_evidence_digests],
      execution: "caller_owned_adapter;raw_value_transient",
    };
    try {
      const value = await route.acquirer.acquire(context);
      const safe = safeJson(value, "evidence reconciliation acquired value");
      const valueDigest = digestJsonSync(safe.value);
      const normalized = plan.normalizer_id === "identity" ? safe.value : await normalizer!(safe.value, context);
      const normalizedSafe = safeJson(normalized, "evidence reconciliation normalized value");
      const normalizedDigest = digestJsonSync(normalizedSafe.value);
      return {
        route: request,
        request_digest: requestDigestValue,
        status: "observed",
        value: safe.value,
        normalized: normalizedSafe.value,
        result: sourceResultDescriptor({ source_id: request.source_id, source_digest: request.source_digest, request_id: request.request_id, request_digest: requestDigestValue, metadata_digest: digestJsonSync(request.metadata), status: "observed", value_digest: valueDigest, value_bytes: safe.bytes, normalized_digest: normalizedDigest, failure_class: null, retryable: false, limitations: [] }),
      };
    } catch (error) {
      const classification = classifyAutonomousEvidenceAcquisitionError(error);
      const failureClass = typeof classification.failure_class === "string" ? text("evidence reconciliation failure_class", classification.failure_class, 128) : "acquisition_failed";
      return {
        route: request,
        request_digest: requestDigestValue,
        status: "failed",
        value: null,
        normalized: null,
        result: sourceResultDescriptor({ source_id: request.source_id, source_digest: request.source_digest, request_id: request.request_id, request_digest: requestDigestValue, metadata_digest: digestJsonSync(request.metadata), status: "failed", value_digest: null, value_bytes: 0, normalized_digest: null, failure_class: failureClass, retryable: classification.retryable === true, limitations: ["source acquisition or normalization failed"] }),
      };
    }
  }
}

export class AutonomousEvidenceReconciliationResult {
  constructor(
    readonly json: AutonomousEvidenceReconciliationResultJSON,
    readonly values: Readonly<Record<string, JsonValue | null>>,
    readonly normalizedValues: Readonly<Record<string, JsonValue | null>>,
  ) {
    if (!json || json.schema !== AUTONOMOUS_EVIDENCE_RECONCILIATION_RESULT_SCHEMA) throw new ArgumentError("evidence reconciliation result is malformed");
    if (!isObject(values) || !isObject(normalizedValues)) throw new ArgumentError("evidence reconciliation transient values are malformed");
  }

  toJSON(): AutonomousEvidenceReconciliationResultJSON {
    return structuredClone(this.json);
  }
}
