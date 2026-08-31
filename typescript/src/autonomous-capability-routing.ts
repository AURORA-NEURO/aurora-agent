import { ArgumentError, isObject } from "./errors.js";
import { digestJsonSync } from "./tooling.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type { JsonObject } from "./types.js";

/**
 * Provider-free capability routing for automatic task intake.
 *
 * Domain routing answers "which discipline should own this task?".  This second, narrower
 * projection answers "which reviewed capability inside that discipline should shape planning,
 * model selection, and tool coverage?".  It is intentionally lexical and abstaining: it never
 * sends task text to a provider, never discovers tools, and never grants execution authority.
 */
export const AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA = "bioprism-autonomous-capability-route/0.1" as const;
export const AUTONOMOUS_CAPABILITY_ROUTE_SOURCE = "deterministic_capability_vocabulary" as const;
export const AUTONOMOUS_CAPABILITY_ROUTE_REASONS = [
  "selected",
  "explicit_capability",
  "no_matching_capability",
  "insufficient_confidence",
  "insufficient_margin",
] as const;
export const MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES = 32;
export const MAX_AUTONOMOUS_CAPABILITY_ROUTE_MATCHED_TERMS = 16;

export type AutonomousCapabilityRouteReason = typeof AUTONOMOUS_CAPABILITY_ROUTE_REASONS[number];

export interface AutonomousCapabilityRouteCandidate extends JsonObject {
  domain: AutonomousDomainName;
  capability: string;
  score: number;
  matched_terms: string[];
}

export interface AutonomousCapabilityRoute extends JsonObject {
  schema: typeof AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA;
  task_digest: string;
  domain: AutonomousDomainName;
  candidates: AutonomousCapabilityRouteCandidate[];
  selected_capability: string | null;
  confidence: number;
  abstained: boolean;
  reason: AutonomousCapabilityRouteReason;
  source: typeof AUTONOMOUS_CAPABILITY_ROUTE_SOURCE;
  route_digest: string;
  retention: "task_text_transient_only; capability_scores_and_digests_only";
  authorization: "classification_only; no_provider_tool_or_effect_authority";
  secret_material: "never_returned";
}

interface CapabilityVocabularyRow {
  capability: string;
  terms: readonly string[];
}

const VOCABULARY: Readonly<Record<AutonomousDomainName, readonly CapabilityVocabularyRow[]>> = {
  coding: [
    { capability: "review", terms: ["review", "audit", "inspect", "release", "pull request", "pr"] },
    { capability: "debugging", terms: ["debug", "bug", "failure", "failing", "error", "stack trace"] },
    { capability: "implementation", terms: ["implement", "build", "code", "feature", "develop", "write code"] },
    { capability: "testing", terms: ["test", "tests", "testing", "ci", "regression", "coverage"] },
  ],
  browser: [
    { capability: "web_research", terms: ["research", "search", "look up", "find sources", "web", "citation"] },
    { capability: "navigation", terms: ["navigate", "browse", "open page", "click", "workspace", "route"] },
    { capability: "source_comparison", terms: ["compare sources", "cross-source", "contrast", "fact check", "verify sources"] },
  ],
  data: [
    { capability: "data_analysis", terms: ["analyze data", "analysis", "aggregate", "statistics", "query", "visualize"] },
    { capability: "schema_validation", terms: ["schema", "validate schema", "columns", "types", "data contract"] },
    { capability: "lineage", terms: ["lineage", "provenance", "transform", "trace data", "data flow"] },
    { capability: "quality_control", terms: ["quality", "missingness", "outlier", "duplicate", "quality control", "clean data"] },
  ],
  science: [
    { capability: "literature", terms: ["literature", "paper", "publication", "study", "references"] },
    { capability: "hypothesis", terms: ["hypothesis", "hypotheses", "mechanism", "causal question"] },
    { capability: "experiment", terms: ["experiment", "experimental design", "protocol", "assay"] },
    { capability: "statistics", terms: ["statistics", "statistical", "p value", "confidence interval", "regression"] },
    { capability: "reproducibility", terms: ["reproduce", "reproduction", "replicate", "replication", "reproducibility"] },
  ],
  biomedical: [
    { capability: "biomedical_review", terms: ["biomedical", "clinical evidence", "medical literature", "biomarker"] },
    { capability: "provenance", terms: ["provenance", "reference", "population", "endpoint"] },
    { capability: "safety_boundary", terms: ["safety", "risk", "ethics", "dual use", "medical boundary"] },
    { capability: "human_review", terms: ["human review", "clinician", "clinical review", "subject", "informed consent"] },
  ],
  neuroscience: [
    { capability: "neuroscience_analysis", terms: ["neuroscience", "neural", "brain", "neural data"] },
    { capability: "signal_interpretation", terms: ["signal", "neural signal", "spike", "eeg", "fmri", "interpret"] },
    { capability: "study_design", terms: ["study design", "experiment design", "cohort", "trial"] },
    { capability: "reproducibility", terms: ["reproduce", "replicate", "benchmark", "trace"] },
  ],
  operations: [
    { capability: "observability", terms: ["observe", "monitor", "telemetry", "metrics", "logs", "status"] },
    { capability: "incident_response", terms: ["incident", "outage", "alert", "on call", "triage"] },
    { capability: "risk_review", terms: ["risk", "readiness", "review change", "approval"] },
    { capability: "rollback", terms: ["rollback", "roll back", "restore", "revert", "undo"] },
    { capability: "approval", terms: ["approve", "authorization", "authorize", "change request"] },
    { capability: "runbook", terms: ["runbook", "playbook", "procedure", "plan"] },
  ],
  enterprise: [
    { capability: "workflow", terms: ["workflow", "process", "business process", "coordinate"] },
    { capability: "governance", terms: ["governance", "owner", "ownership", "policy"] },
    { capability: "compliance", terms: ["compliance", "audit", "control", "regulation"] },
    { capability: "analytics", terms: ["analytics", "dashboard", "kpi", "report"] },
    { capability: "coordination", terms: ["coordinate", "stakeholder", "handoff", "meeting"] },
  ],
  multi_agent: [
    { capability: "delegation", terms: ["delegate", "delegation", "assign", "subtask"] },
    { capability: "coordination", terms: ["coordinate", "orchestrate", "multi agent", "agents"] },
    { capability: "consensus", terms: ["consensus", "vote", "agreement", "dissent"] },
    { capability: "conflict_resolution", terms: ["conflict", "disagreement", "resolve conflict"] },
    { capability: "handoff", terms: ["handoff", "handover", "transfer", "agent result"] },
  ],
  multimodal: [
    { capability: "image", terms: ["image", "photo", "visual", "vision"] },
    { capability: "audio", terms: ["audio", "sound", "speech", "recording"] },
    { capability: "video", terms: ["video", "frame", "temporal"] },
    { capability: "document", terms: ["document", "pdf", "text extraction", "ocr"] },
    { capability: "cross_modal_alignment", terms: ["align modalities", "cross modal", "multimodal", "fusion", "synchronize"] },
  ],
  cross_domain: [
    { capability: "routing", terms: ["route", "routing", "which domain", "assign domain"] },
    { capability: "synthesis", terms: ["synthesize", "combine", "integrate", "summary"] },
    { capability: "evidence_alignment", terms: ["evidence", "align evidence", "provenance", "compare findings"] },
    { capability: "workflow_composition", terms: ["workflow", "compose workflow", "pipeline", "dependency"] },
  ],
  evaluation: [
    { capability: "benchmarking", terms: ["benchmark", "benchmarking", "compare models", "performance"] },
    { capability: "rubric", terms: ["rubric", "criteria", "score", "grading"] },
    { capability: "replay", terms: ["replay", "re-run", "reproduce trace", "deterministic"] },
    { capability: "failure_analysis", terms: ["failure", "error analysis", "regression", "root cause"] },
    { capability: "reproducibility", terms: ["reproducibility", "replicate", "exact replay", "repeat"] },
  ],
};

function boundedText(name: string, value: unknown, maximum = 32_000): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\0") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[A-Za-z0-9_.:-]{1,256}$/.test(value)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return value;
}

function boundedDigest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedUnit(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return value;
}

function normalize(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim().replace(/\s+/g, " ");
}

function matches(normalized: string, term: string): boolean {
  const needle = normalize(term);
  return needle.length > 0 && ` ${normalized} `.includes(` ${needle} `);
}

function scoreFor(terms: readonly string[], normalized: string): { score: number; matched: string[] } {
  const matched = terms.filter((term) => matches(normalized, term));
  const points = matched.reduce((sum, term) => sum + (normalize(term).length >= 6 || term.includes(" ") ? 2 : 1), 0);
  return { score: Number(Math.min(1, points / 4).toFixed(12)), matched: [...matched].sort() };
}

function descriptorWithoutDigest(route: Omit<AutonomousCapabilityRoute, "route_digest">): JsonObject {
  return { ...route };
}

function makeRoute(route: Omit<AutonomousCapabilityRoute, "route_digest">): AutonomousCapabilityRoute {
  return { ...route, route_digest: digestJsonSync(descriptorWithoutDigest(route)) } as AutonomousCapabilityRoute;
}

function assertRouteShape(value: unknown): asserts value is AutonomousCapabilityRoute {
  if (!isObject(value)) throw new ArgumentError("autonomous capability route must be an object");
  const allowed = new Set(["schema", "task_digest", "domain", "candidates", "selected_capability", "confidence", "abstained", "reason", "source", "route_digest", "retention", "authorization", "secret_material"]);
  if (Object.keys(value).some((key) => !allowed.has(key))) throw new ArgumentError("autonomous capability route contains unsupported fields");
  if (value.schema !== AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA || value.source !== AUTONOMOUS_CAPABILITY_ROUTE_SOURCE || value.retention !== "task_text_transient_only; capability_scores_and_digests_only" || value.authorization !== "classification_only; no_provider_tool_or_effect_authority" || value.secret_material !== "never_returned") throw new ArgumentError("autonomous capability route markers are invalid");
  boundedDigest("autonomous capability route task_digest", value.task_digest);
  if (typeof value.domain !== "string" || !(value.domain in VOCABULARY)) throw new ArgumentError("autonomous capability route domain is unsupported");
  if (!Array.isArray(value.candidates) || value.candidates.length > MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES) throw new ArgumentError("autonomous capability route candidates exceed their bound");
  const capabilities = new Set(VOCABULARY[value.domain as AutonomousDomainName].map((row) => row.capability));
  const seen = new Set<string>();
  for (const [index, candidate] of value.candidates.entries()) {
    if (!isObject(candidate) || Object.keys(candidate).some((key) => !["domain", "capability", "score", "matched_terms"].includes(key))) throw new ArgumentError(`autonomous capability route candidate ${index} is malformed`);
    if (!Array.isArray(candidate.matched_terms) || candidate.matched_terms.length > MAX_AUTONOMOUS_CAPABILITY_ROUTE_MATCHED_TERMS || candidate.matched_terms.some((term) => typeof term !== "string")) throw new ArgumentError("autonomous capability route matched_terms are invalid");
    if (candidate.domain !== value.domain || typeof candidate.capability !== "string") throw new ArgumentError("autonomous capability route candidate identity is invalid");
    const candidateCapability = candidate.capability;
    const explicit = candidate.matched_terms.includes("caller_explicit_capability");
    if ((!capabilities.has(candidateCapability) && !explicit) || seen.has(candidateCapability)) throw new ArgumentError("autonomous capability route candidate identity is invalid");
    seen.add(candidateCapability);
    boundedUnit(`autonomous capability route candidate ${candidateCapability} score`, candidate.score);
  }
  if (value.selected_capability !== null && (typeof value.selected_capability !== "string" || !seen.has(value.selected_capability))) throw new ArgumentError("autonomous capability route selected capability is not a candidate");
  boundedUnit("autonomous capability route confidence", value.confidence);
  if (typeof value.abstained !== "boolean" || !AUTONOMOUS_CAPABILITY_ROUTE_REASONS.includes(value.reason as AutonomousCapabilityRouteReason)) throw new ArgumentError("autonomous capability route decision is invalid");
  if (value.abstained && value.selected_capability !== null) throw new ArgumentError("abstained capability route cannot select a capability");
  if (!value.abstained && value.selected_capability === null) throw new ArgumentError("selected capability route must select a capability");
  boundedDigest("autonomous capability route route_digest", value.route_digest);
}

/** Return the reviewed capability labels available to the given domain. */
export function autonomousCapabilityVocabulary(domain: AutonomousDomainName): readonly string[] {
  if (!(domain in VOCABULARY)) throw new ArgumentError(`unsupported autonomous capability domain: ${domain}`);
  return VOCABULARY[domain].map((row) => row.capability);
}

/** Build one digest-bound capability proposal without contacting any external system. */
export function routeAutonomousCapability(
  task: string,
  domain: AutonomousDomainName,
  options: { explicitCapability?: string; minConfidence?: number; minMargin?: number } = {},
): AutonomousCapabilityRoute {
  const taskText = boundedText("autonomous capability route task", task);
  if (!(domain in VOCABULARY)) throw new ArgumentError(`unsupported autonomous capability domain: ${domain}`);
  const taskDigest = digestJsonSync({ task: taskText });
  if (options.minConfidence !== undefined) boundedUnit("autonomous capability route minConfidence", options.minConfidence);
  if (options.minMargin !== undefined) boundedUnit("autonomous capability route minMargin", options.minMargin);
  if (options.explicitCapability !== undefined) {
    const capability = boundedIdentifier("autonomous capability route explicit capability", options.explicitCapability);
    return makeRoute({ schema: AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA, task_digest: taskDigest, domain, candidates: [{ domain, capability, score: 1, matched_terms: ["caller_explicit_capability"] }], selected_capability: capability, confidence: 1, abstained: false, reason: "explicit_capability", source: AUTONOMOUS_CAPABILITY_ROUTE_SOURCE, retention: "task_text_transient_only; capability_scores_and_digests_only", authorization: "classification_only; no_provider_tool_or_effect_authority", secret_material: "never_returned" });
  }
  const normalized = normalize(taskText);
  const candidates = VOCABULARY[domain].map((row) => {
    const scored = scoreFor(row.terms, normalized);
    return { domain, capability: row.capability, score: scored.score, matched_terms: scored.matched };
  }).filter((candidate) => candidate.score > 0).sort((left, right) => right.score - left.score || left.capability.localeCompare(right.capability)).slice(0, MAX_AUTONOMOUS_CAPABILITY_ROUTE_CANDIDATES);
  const base = { schema: AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA as typeof AUTONOMOUS_CAPABILITY_ROUTE_SCHEMA, task_digest: taskDigest, domain, candidates, selected_capability: null, confidence: candidates[0]?.score ?? 0, abstained: true, reason: "no_matching_capability" as AutonomousCapabilityRouteReason, source: AUTONOMOUS_CAPABILITY_ROUTE_SOURCE, retention: "task_text_transient_only; capability_scores_and_digests_only" as const, authorization: "classification_only; no_provider_tool_or_effect_authority" as const, secret_material: "never_returned" as const };
  if (!candidates.length) return makeRoute(base);
  const top = candidates[0]!;
  const second = candidates[1];
  const minConfidence = options.minConfidence ?? 0.25;
  const minMargin = options.minMargin ?? 0.10;
  if (top.score < minConfidence) return makeRoute({ ...base, reason: "insufficient_confidence" });
  if (second && top.score - second.score < minMargin) return makeRoute({ ...base, reason: "insufficient_margin" });
  return makeRoute({ ...base, selected_capability: top.capability, confidence: top.score, abstained: false, reason: "selected" });
}

/** Verify a capability handoff against the exact task digest before it shapes planning. */
export function validateAutonomousCapabilityRoute(task: string, value: unknown): AutonomousCapabilityRoute {
  assertRouteShape(value);
  const taskDigest = digestJsonSync({ task: boundedText("autonomous capability route task", task) });
  if (value.task_digest !== taskDigest) throw new ArgumentError("autonomous capability route does not match the task digest");
  const { route_digest: _routeDigest, ...descriptor } = value;
  if (digestJsonSync(descriptor) !== value.route_digest) throw new ArgumentError("autonomous capability route digest does not match its metadata");
  return structuredClone(value);
}
