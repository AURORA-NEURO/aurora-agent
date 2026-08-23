import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousAgent, type AutonomousAutoBlueprint, type AutonomousDomainName, type AutonomousPromptChunk } from "./autonomous.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Digest-bound, non-executing composition of multiple reviewed domain workflows. */
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio/0.1" as const;
export const AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-verification/0.1" as const;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS = 64;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES = 16;
export const MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HINTS = 32;

export type AutonomousWorkflowPortfolioItemStatus = "ready" | "blocked" | "failed" | "route_review_required";
export type AutonomousWorkflowPortfolioStatus = "ready" | "partial" | "blocked";

/** Caller-owned input. Tasks, context, and hints are transient and never appear in a plan. */
export interface AutonomousWorkflowPortfolioItemRequest {
  id?: string;
  task: string;
  domain: AutonomousDomainName;
  capability?: string;
  dependsOn?: readonly string[];
  hints?: readonly string[];
  context?: readonly AutonomousPromptChunk[];
}

export interface AutonomousWorkflowPortfolioPlanOptions {
  /** Require one ready item for every built-in autonomous domain. */
  requireAllDomains?: boolean;
  /** Keep planning independent items when one item fails; defaults to true. */
  allowPartial?: boolean;
}

export interface AutonomousWorkflowPortfolioPolicy extends JsonObject {
  require_all_domains: boolean;
  allow_partial: boolean;
}

export interface AutonomousWorkflowPortfolioItem extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA;
  item_id: string;
  domain: AutonomousDomainName;
  capability: string | null;
  depends_on: string[];
  task_digest: string;
  request_digest: string;
  route_digest: string | null;
  workflow_id: string | null;
  workflow_digest: string | null;
  plan_digest: string | null;
  evidence_plan_digest: string | null;
  stage_ids: string[];
  required_capabilities: string[];
  status: AutonomousWorkflowPortfolioItemStatus;
  error_class: string | null;
  retention: "metadata_only_task_and_blueprint_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioCoverage extends JsonObject {
  requested_domains: AutonomousDomainName[];
  ready_domains: AutonomousDomainName[];
  missing_domains: AutonomousDomainName[];
  duplicate_domain_items: AutonomousDomainName[];
  requested_item_count: number;
  ready_item_count: number;
  blocked_item_count: number;
  failed_item_count: number;
  complete: boolean;
}

export interface AutonomousWorkflowPortfolioDependencyGraph extends JsonObject {
  topological_order: string[];
  waves: string[][];
  cycle_item_ids: string[];
  edge_count: number;
}

export interface AutonomousWorkflowPortfolioPlan extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA;
  status: AutonomousWorkflowPortfolioStatus;
  policy: AutonomousWorkflowPortfolioPolicy;
  items: AutonomousWorkflowPortfolioItem[];
  coverage: AutonomousWorkflowPortfolioCoverage;
  dependency_graph: AutonomousWorkflowPortfolioDependencyGraph;
  portfolio_digest: string;
  execution: "not_started;planning_and_verification_only";
  authorization: "portfolio_selection_does_not_authorize_provider_tools_or_effects";
  retention: "metadata_only_task_and_blueprint_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousWorkflowPortfolioMismatch extends JsonObject {
  item_id: string;
  codes: string[];
}

export interface AutonomousWorkflowPortfolioVerification extends JsonObject {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA;
  status: "verified" | "mismatch" | "incomplete";
  expected_portfolio_digest: string;
  observed_portfolio_digest: string | null;
  mismatches: AutonomousWorkflowPortfolioMismatch[];
  expected_item_count: number;
  observed_item_count: number;
  replayed_item_count: number;
  execution: "planning_only;no_provider_or_tool_calls";
  retention: "metadata_only_task_and_blueprint_values_not_retained";
  secret_material: "never_returned";
  verification_digest: string;
}

interface NormalizedPortfolioRequest {
  id: string;
  task: string;
  domain: AutonomousDomainName;
  capability: string | undefined;
  dependsOn: string[];
  hints: string[];
  context: readonly AutonomousPromptChunk[];
}

const identifierPattern = /^[A-Za-z0-9_.:-]+$/;

function boundedIdentifier(label: string, value: unknown, maximum = 128): string {
  if (typeof value !== "string" || !value || value.length > maximum || !identifierPattern.test(value)) throw new ArgumentError(`${label} is outside its identifier contract`);
  return value;
}

function boundedText(label: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value || value.length > maximum || /[\u0000]/.test(value)) throw new ArgumentError(`${label} is outside its bounded text contract`);
  return value;
}

function boundedStringList(label: string, values: unknown, maximum: number): string[] {
  if (values === undefined) return [];
  if (!Array.isArray(values) || values.length > maximum) throw new ArgumentError(`${label} must contain at most ${maximum} entries`);
  const normalized = values.map((value) => boundedText(label, value, 2_048));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${label} must not contain duplicates`);
  return normalized;
}

function normalizeRequest(value: AutonomousWorkflowPortfolioItemRequest, index: number, ids: ReadonlySet<string>): NormalizedPortfolioRequest {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ArgumentError(`workflow portfolio item ${index} must be an object`);
  const id = boundedIdentifier(`workflow portfolio item ${index} id`, value.id ?? `item-${index + 1}`);
  if (ids.has(id)) throw new ArgumentError(`workflow portfolio item id is duplicated: ${id}`);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value.domain)) throw new ArgumentError(`workflow portfolio item ${id} domain is unsupported`);
  const task = boundedText(`workflow portfolio item ${id} task`, value.task, 32_000);
  const capability = value.capability === undefined || value.capability === null ? undefined : boundedText(`workflow portfolio item ${id} capability`, value.capability, 256);
  const dependsOn = boundedStringList(`workflow portfolio item ${id} dependsOn`, value.dependsOn, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES);
  const hints = boundedStringList(`workflow portfolio item ${id} hints`, value.hints, MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HINTS);
  if (value.context !== undefined && (!Array.isArray(value.context) || value.context.length > 128)) throw new ArgumentError(`workflow portfolio item ${id} context is outside its bound`);
  return { id, task, domain: value.domain, capability, dependsOn, hints, context: value.context ?? [] };
}

function normalizeRequests(values: readonly AutonomousWorkflowPortfolioItemRequest[]): NormalizedPortfolioRequest[] {
  if (!Array.isArray(values) || values.length < 1 || values.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS) throw new ArgumentError(`workflow portfolio items must contain 1..=${MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS} entries`);
  const ids = new Set<string>();
  const normalized = values.map((value, index) => {
    const item = normalizeRequest(value, index, ids);
    ids.add(item.id);
    return item;
  });
  for (const item of normalized) {
    if (item.dependsOn.includes(item.id)) throw new ArgumentError(`workflow portfolio item ${item.id} cannot depend on itself`);
    if (item.dependsOn.some((dependency) => !ids.has(dependency))) throw new ArgumentError(`workflow portfolio item ${item.id} depends on an unknown item`);
  }
  return normalized;
}

function dependencyGraph(requests: readonly NormalizedPortfolioRequest[]): AutonomousWorkflowPortfolioDependencyGraph {
  const byId = new Map(requests.map((request) => [request.id, request]));
  const indegree = new Map(requests.map((request) => [request.id, request.dependsOn.length]));
  const children = new Map<string, string[]>(requests.map((request) => [request.id, []]));
  for (const request of requests) for (const dependency of request.dependsOn) children.get(dependency)!.push(request.id);
  for (const values of children.values()) values.sort();
  const ready = [...requests].filter((request) => indegree.get(request.id) === 0).map((request) => request.id).sort();
  const topological: string[] = [];
  const waves: string[][] = [];
  while (ready.length) {
    const wave = ready.splice(0, ready.length).sort();
    waves.push([...wave]);
    for (const id of wave) {
      topological.push(id);
      for (const child of children.get(id) ?? []) {
        const next = (indegree.get(child) ?? 0) - 1;
        indegree.set(child, next);
        if (next === 0) ready.push(child);
      }
    }
  }
  const cycleItemIds = requests.map((request) => request.id).filter((id) => !topological.includes(id)).sort();
  return { topological_order: topological, waves, cycle_item_ids: cycleItemIds, edge_count: requests.reduce((count, request) => count + request.dependsOn.length, 0) };
}

async function portfolioRequestDigest(item: NormalizedPortfolioRequest): Promise<string> {
  return digestJson({
    schema: "bioprism-typescript-autonomous-workflow-portfolio-request/0.1",
    item_id: item.id,
    task: item.task,
    domain: item.domain,
    capability: item.capability ?? null,
    depends_on: [...item.dependsOn],
    hints: [...item.hints],
    context: item.context,
  });
}

async function blueprintMetadata(item: NormalizedPortfolioRequest, blueprint: AutonomousAutoBlueprint, requestDigest: string): Promise<AutonomousWorkflowPortfolioItem> {
  const taskDigest = blueprint.route.task_digest;
  const workflow = blueprint.blueprint?.workflow;
  const plan = blueprint.blueprint?.plan;
  const evidencePlan = blueprint.blueprint?.evidence_plan;
  if (!workflow || !plan) {
    return {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
      item_id: item.id,
      domain: item.domain,
      capability: item.capability ?? null,
      depends_on: [...item.dependsOn],
      task_digest: taskDigest,
      request_digest: requestDigest,
      route_digest: blueprint.route.route_digest,
      workflow_id: null,
      workflow_digest: null,
      plan_digest: null,
      evidence_plan_digest: null,
      stage_ids: [],
      required_capabilities: [],
      status: "route_review_required",
      error_class: "route_not_ready",
      retention: "metadata_only_task_and_blueprint_values_not_retained",
      secret_material: "never_returned",
    };
  }
  return {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
    item_id: item.id,
    domain: item.domain,
    capability: item.capability ?? null,
    depends_on: [...item.dependsOn],
    task_digest: taskDigest,
    request_digest: requestDigest,
    route_digest: blueprint.route.route_digest,
    workflow_id: workflow.workflow_id,
    workflow_digest: workflow.workflow_digest,
    plan_digest: plan.plan_digest,
    evidence_plan_digest: evidencePlan ? evidencePlan.plan_digest : null,
    stage_ids: workflow.stages.map((stage) => stage.id),
    required_capabilities: [...new Set(workflow.stages.flatMap((stage) => stage.required_capabilities))].sort(),
    status: "ready",
    error_class: null,
    retention: "metadata_only_task_and_blueprint_values_not_retained",
    secret_material: "never_returned",
  };
}

function coverage(items: readonly AutonomousWorkflowPortfolioItem[], requests: readonly NormalizedPortfolioRequest[], requireAllDomains: boolean): AutonomousWorkflowPortfolioCoverage {
  const requestedDomains = [...new Set(requests.map((request) => request.domain))] as AutonomousDomainName[];
  const readyDomains = [...new Set(items.filter((item) => item.status === "ready").map((item) => item.domain))] as AutonomousDomainName[];
  const counts = new Map<AutonomousDomainName, number>();
  for (const request of requests) counts.set(request.domain, (counts.get(request.domain) ?? 0) + 1);
  const duplicateDomainItems = [...counts.entries()].filter(([, count]) => count > 1).map(([domain]) => domain).sort() as AutonomousDomainName[];
  const missingDomains = (requireAllDomains ? AUTONOMOUS_DOMAIN_NAMES : requestedDomains).filter((domain) => !readyDomains.includes(domain));
  return {
    requested_domains: [...requestedDomains].sort() as AutonomousDomainName[],
    ready_domains: [...readyDomains].sort() as AutonomousDomainName[],
    missing_domains: [...missingDomains].sort() as AutonomousDomainName[],
    duplicate_domain_items: duplicateDomainItems,
    requested_item_count: items.length,
    ready_item_count: items.filter((item) => item.status === "ready").length,
    blocked_item_count: items.filter((item) => item.status === "blocked").length,
    failed_item_count: items.filter((item) => item.status === "failed" || item.status === "route_review_required").length,
    complete: missingDomains.length === 0 && items.every((item) => item.status === "ready"),
  };
}

function planStatus(items: readonly AutonomousWorkflowPortfolioItem[], complete: boolean, allowPartial: boolean): AutonomousWorkflowPortfolioStatus {
  if (complete) return "ready";
  if (allowPartial && items.some((item) => item.status === "ready")) return "partial";
  return "blocked";
}

/** Validate a metadata-only portfolio and its content digest before it is reused after restart. */
export async function validateAutonomousWorkflowPortfolioPlan(value: unknown): Promise<AutonomousWorkflowPortfolioPlan> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ArgumentError("workflow portfolio plan must be an object");
  const plan = value as Record<string, unknown>;
  if (plan.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA || !Array.isArray(plan.items) || !plan.policy || !plan.coverage || !plan.dependency_graph) throw new ArgumentError("workflow portfolio plan is malformed");
  if (plan.status !== "ready" && plan.status !== "partial" && plan.status !== "blocked") throw new ArgumentError("workflow portfolio plan status is invalid");
  const allowedKeys = new Set(["schema", "status", "policy", "items", "coverage", "dependency_graph", "portfolio_digest", "execution", "authorization", "retention", "secret_material"]);
  if (Object.keys(plan).some((key) => !allowedKeys.has(key))) throw new ArgumentError("workflow portfolio plan contains unsupported fields");
  if (typeof (plan.policy as Record<string, unknown>).require_all_domains !== "boolean" || typeof (plan.policy as Record<string, unknown>).allow_partial !== "boolean") throw new ArgumentError("workflow portfolio plan policy is malformed");
  const items = plan.items as AutonomousWorkflowPortfolioItem[];
  if (items.length < 1 || items.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS) throw new ArgumentError("workflow portfolio plan item count is outside its bound");
  const ids = new Set<string>();
  const itemKeys = new Set(["schema", "item_id", "domain", "capability", "depends_on", "task_digest", "request_digest", "route_digest", "workflow_id", "workflow_digest", "plan_digest", "evidence_plan_digest", "stage_ids", "required_capabilities", "status", "error_class", "retention", "secret_material"]);
  for (const item of items) {
    if (!item || item.schema !== AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA) throw new ArgumentError("workflow portfolio plan item is malformed");
    if (Object.keys(item).some((key) => !itemKeys.has(key))) throw new ArgumentError("workflow portfolio plan item contains unsupported fields");
    const id = boundedIdentifier("workflow portfolio plan item_id", item.item_id);
    if (ids.has(id)) throw new ArgumentError("workflow portfolio plan item ids must be unique");
    ids.add(id);
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(item.domain) || !Array.isArray(item.depends_on) || item.depends_on.length > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES) throw new ArgumentError("workflow portfolio plan item domain or dependencies are malformed");
    if (!["ready", "blocked", "failed", "route_review_required"].includes(item.status)) throw new ArgumentError("workflow portfolio plan item status is invalid");
    if (typeof item.task_digest !== "string" || !/^[0-9a-f]{64}$/.test(item.task_digest)) throw new ArgumentError("workflow portfolio plan item task_digest is malformed");
    if (typeof item.request_digest !== "string" || !/^[0-9a-f]{64}$/.test(item.request_digest)) throw new ArgumentError("workflow portfolio plan item request_digest is malformed");
    if (item.capability !== null && typeof item.capability !== "string") throw new ArgumentError("workflow portfolio plan item capability is malformed");
    for (const [label, value] of [["route_digest", item.route_digest], ["workflow_digest", item.workflow_digest], ["plan_digest", item.plan_digest], ["evidence_plan_digest", item.evidence_plan_digest]] as const) {
      if (value !== null && (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))) throw new ArgumentError(`workflow portfolio plan item ${label} is malformed`);
    }
    if (!Array.isArray(item.stage_ids) || item.stage_ids.length > 64 || !Array.isArray(item.required_capabilities) || item.required_capabilities.length > 128 || item.retention !== "metadata_only_task_and_blueprint_values_not_retained" || item.secret_material !== "never_returned") throw new ArgumentError("workflow portfolio plan item metadata is malformed");
    for (const dependency of item.depends_on) boundedIdentifier("workflow portfolio plan dependency", dependency);
  }
  for (const item of items) if (item.depends_on.some((dependency) => !ids.has(dependency))) throw new ArgumentError("workflow portfolio plan references an unknown dependency");
  if (plan.execution !== "not_started;planning_and_verification_only" || plan.authorization !== "portfolio_selection_does_not_authorize_provider_tools_or_effects" || plan.retention !== "metadata_only_task_and_blueprint_values_not_retained" || plan.secret_material !== "never_returned") throw new ArgumentError("workflow portfolio plan authority markers are invalid");
  if (typeof plan.portfolio_digest !== "string" || !/^[0-9a-f]{64}$/.test(plan.portfolio_digest)) throw new ArgumentError("workflow portfolio plan digest is malformed");
  const descriptor = { ...plan };
  delete descriptor.portfolio_digest;
  if (await digestJson(descriptor) !== plan.portfolio_digest) throw new ArgumentError("workflow portfolio plan digest is invalid");
  const graphInput = items.map((item) => ({ id: item.item_id, task: "rehydrated", domain: item.domain, capability: item.capability ?? undefined, dependsOn: [...item.depends_on], hints: [], context: [] }));
  if (canonicalJson(dependencyGraph(graphInput)) !== canonicalJson(plan.dependency_graph)) throw new ArgumentError("workflow portfolio dependency graph is inconsistent");
  const expectedCoverage = coverage(items, graphInput, (plan.policy as Record<string, unknown>).require_all_domains === true);
  if (canonicalJson(expectedCoverage) !== canonicalJson(plan.coverage)) throw new ArgumentError("workflow portfolio coverage is inconsistent");
  return structuredClone(value) as AutonomousWorkflowPortfolioPlan;
}

/** Build a dependency-aware portfolio of all-domain workflow blueprints without provider calls. */
export async function planAutonomousWorkflowPortfolio(
  agent: AutonomousAgent,
  values: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioPlanOptions = {},
): Promise<AutonomousWorkflowPortfolioPlan> {
  if (!agent || typeof agent.blueprint !== "function") throw new ArgumentError("workflow portfolio planning requires an AutonomousAgent");
  if (options.requireAllDomains !== undefined && typeof options.requireAllDomains !== "boolean") throw new ArgumentError("workflow portfolio requireAllDomains must be boolean");
  if (options.allowPartial !== undefined && typeof options.allowPartial !== "boolean") throw new ArgumentError("workflow portfolio allowPartial must be boolean");
  const requireAllDomains = options.requireAllDomains === true;
  const allowPartial = options.allowPartial !== false;
  const requests = normalizeRequests(values);
  const graph = dependencyGraph(requests);
  const itemById = new Map<string, AutonomousWorkflowPortfolioItem>();
  for (const id of graph.topological_order) {
    const request = requests.find((candidate) => candidate.id === id)!;
    const dependencyItems = request.dependsOn.map((dependency) => itemById.get(dependency)!);
    if (dependencyItems.some((item) => item.status !== "ready")) {
      const requestDigest = await portfolioRequestDigest(request);
      itemById.set(id, {
        schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
        item_id: request.id,
        domain: request.domain,
        capability: request.capability ?? null,
        depends_on: [...request.dependsOn],
        task_digest: await digestJson({ task: request.task }),
        request_digest: requestDigest,
        route_digest: null,
        workflow_id: null,
        workflow_digest: null,
        plan_digest: null,
        evidence_plan_digest: null,
        stage_ids: [],
        required_capabilities: [],
        status: "blocked",
        error_class: "dependency_not_ready",
        retention: "metadata_only_task_and_blueprint_values_not_retained",
        secret_material: "never_returned",
      });
      continue;
    }
    try {
      const requestDigest = await portfolioRequestDigest(request);
      const auto = await agent.blueprint(request.task, { domain: request.domain, capability: request.capability, hints: request.hints, context: request.context });
      itemById.set(id, await blueprintMetadata(request, auto, requestDigest));
    } catch (error) {
      const requestDigest = await portfolioRequestDigest(request);
      itemById.set(id, {
        schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
        item_id: request.id,
        domain: request.domain,
        capability: request.capability ?? null,
        depends_on: [...request.dependsOn],
        task_digest: await digestJson({ task: request.task }),
        request_digest: requestDigest,
        route_digest: null,
        workflow_id: null,
        workflow_digest: null,
        plan_digest: null,
        evidence_plan_digest: null,
        stage_ids: [],
        required_capabilities: [],
        status: "failed",
        error_class: error instanceof Error && error.constructor.name ? error.constructor.name : "workflow_portfolio_item_failed",
        retention: "metadata_only_task_and_blueprint_values_not_retained",
        secret_material: "never_returned",
      });
    }
  }
  for (const id of graph.cycle_item_ids) {
    const request = requests.find((candidate) => candidate.id === id)!;
    const requestDigest = await portfolioRequestDigest(request);
    itemById.set(id, {
      schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
      item_id: id,
      domain: request.domain,
      capability: request.capability ?? null,
      depends_on: [...request.dependsOn],
      task_digest: await digestJson({ task: request.task }),
      request_digest: requestDigest,
      route_digest: null,
      workflow_id: null,
      workflow_digest: null,
      plan_digest: null,
      evidence_plan_digest: null,
      stage_ids: [],
      required_capabilities: [],
      status: "blocked",
      error_class: "dependency_cycle",
      retention: "metadata_only_task_and_blueprint_values_not_retained",
      secret_material: "never_returned",
    });
  }
  const items = requests.map((request) => itemById.get(request.id)!).sort((left, right) => left.item_id.localeCompare(right.item_id));
  const planCoverage = coverage(items, requests, requireAllDomains);
  const status = planStatus(items, planCoverage.complete && graph.cycle_item_ids.length === 0, allowPartial);
  const policy: AutonomousWorkflowPortfolioPolicy = { require_all_domains: requireAllDomains, allow_partial: allowPartial };
  const descriptor = {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
    status,
    policy,
    items,
    coverage: planCoverage,
    dependency_graph: graph,
    execution: "not_started;planning_and_verification_only" as const,
    authorization: "portfolio_selection_does_not_authorize_provider_tools_or_effects" as const,
    retention: "metadata_only_task_and_blueprint_values_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, portfolio_digest: await digestJson(descriptor) };
}

/** Re-run the non-executing portfolio compiler and compare every digest-bound identity. */
export async function verifyAutonomousWorkflowPortfolio(
  agent: AutonomousAgent,
  plan: AutonomousWorkflowPortfolioPlan,
  values: readonly AutonomousWorkflowPortfolioItemRequest[],
  options: AutonomousWorkflowPortfolioPlanOptions = {},
): Promise<AutonomousWorkflowPortfolioVerification> {
  const expected = await validateAutonomousWorkflowPortfolioPlan(plan);
  let observed: AutonomousWorkflowPortfolioPlan | null = null;
  const mismatches: AutonomousWorkflowPortfolioMismatch[] = [];
  const replayOptions: AutonomousWorkflowPortfolioPlanOptions = {
    requireAllDomains: options.requireAllDomains ?? expected.policy.require_all_domains,
    allowPartial: options.allowPartial ?? expected.policy.allow_partial,
  };
  try {
    observed = await planAutonomousWorkflowPortfolio(agent, values, replayOptions);
  } catch (error) {
    const verification = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA, status: "incomplete" as const, expected_portfolio_digest: expected.portfolio_digest, observed_portfolio_digest: null, mismatches: [{ item_id: "portfolio", codes: [error instanceof Error && error.constructor.name ? error.constructor.name : "replay_failed"] }], expected_item_count: expected.items.length, observed_item_count: 0, replayed_item_count: 0, execution: "planning_only;no_provider_or_tool_calls" as const, retention: "metadata_only_task_and_blueprint_values_not_retained" as const, secret_material: "never_returned" as const };
    return { ...verification, verification_digest: await digestJson(verification) };
  }
  const observedById = new Map(observed.items.map((item) => [item.item_id, item]));
  for (const expectedItem of expected.items) {
    const actual = observedById.get(expectedItem.item_id);
    if (!actual) { mismatches.push({ item_id: expectedItem.item_id, codes: ["missing_item"] }); continue; }
    const codes = ["domain", "capability", "depends_on", "task_digest", "request_digest", "route_digest", "workflow_id", "workflow_digest", "plan_digest", "evidence_plan_digest", "stage_ids", "required_capabilities", "status", "error_class"].filter((field) => canonicalJson(actual[field]) !== canonicalJson(expectedItem[field]));
    if (codes.length) mismatches.push({ item_id: expectedItem.item_id, codes });
  }
  for (const actual of observed.items) if (!expected.items.some((item) => item.item_id === actual.item_id)) mismatches.push({ item_id: actual.item_id, codes: ["unexpected_item"] });
  if (canonicalJson(observed.coverage) !== canonicalJson(expected.coverage)) mismatches.push({ item_id: "portfolio", codes: ["coverage"] });
  if (canonicalJson(observed.dependency_graph) !== canonicalJson(expected.dependency_graph)) mismatches.push({ item_id: "portfolio", codes: ["dependency_graph"] });
  if (canonicalJson(observed.policy) !== canonicalJson(expected.policy)) mismatches.push({ item_id: "portfolio", codes: ["policy"] });
  const verification = { schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA, status: mismatches.length ? "mismatch" as const : "verified" as const, expected_portfolio_digest: expected.portfolio_digest, observed_portfolio_digest: observed.portfolio_digest, mismatches, expected_item_count: expected.items.length, observed_item_count: observed.items.length, replayed_item_count: observed.items.filter((item) => item.status === "ready").length, execution: "planning_only;no_provider_or_tool_calls" as const, retention: "metadata_only_task_and_blueprint_values_not_retained" as const, secret_material: "never_returned" as const };
  return { ...verification, verification_digest: await digestJson(verification) };
}
