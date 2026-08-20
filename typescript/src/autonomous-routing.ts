import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type { AutonomousAgent } from "./autonomous.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  builtinAutonomousDomainProfiles,
  type AutonomousDomainName,
  type AutonomousRouteCandidate,
  type AutonomousRouteProposal,
} from "./autonomous.js";
import type { CredentialHandle, ProviderInvocationObserver } from "./llm.js";
import type { AutonomousModelCandidate } from "./llm.js";
import { digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA = "bioprism-typescript-autonomous-semantic-route/0.1" as const;

export interface AutonomousSemanticRouteCandidate extends JsonObject {
  domain: AutonomousDomainName;
  score: number;
  capability: string;
  rationale: string;
}

export interface AutonomousSemanticRouteResult extends JsonObject {
  schema: typeof AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA;
  status: "completed" | "approval_required" | "provider_abstained" | "provider_invalid" | "provider_disagreement";
  route: AutonomousRouteProposal;
  deterministic_route: AutonomousRouteProposal;
  semantic_candidates: AutonomousSemanticRouteCandidate[];
  semantic_selected_domains: AutonomousDomainName[];
  semantic_confidence: number;
  selected_model: { provider: string; model: string } | null;
  selection_digest: string | null;
  prompt_digest: string;
  outcome_digest: string | null;
  retention: "route_digests_and_scores_only;task_prompt_and_provider_response_not_retained";
  authorization: "route_review_only;provider_call_requires_explicit_approval";
}

export interface AutonomousSemanticRouteOptions {
  candidates?: readonly AutonomousModelCandidate[];
  credential?: CredentialHandle;
  credentialFor?: (provider: string) => CredentialHandle | undefined;
  hints?: readonly string[];
  approveProviderCall?: boolean;
  minSemanticConfidence?: number;
  maxDomains?: number;
  allowCrossDomain?: boolean;
  maxOutputTokens?: number;
  execution?: AutonomousExecutionController;
  executionAttempt?: number;
  maxProviderFailovers?: number;
  executionLifecycle?: "managed" | "observe_only";
  signal?: AbortSignal;
  observer?: ProviderInvocationObserver;
}

const RETENTION = "route_digests_and_scores_only;task_prompt_and_provider_response_not_retained" as const;
const AUTHORIZATION = "route_review_only;provider_call_requires_explicit_approval" as const;

async function failSemanticExecution(options: AutonomousSemanticRouteOptions): Promise<void> {
  const execution = options.execution;
  if (!execution || options.executionLifecycle === "observe_only") return;
  const state = execution.state;
  if (["completed", "failed", "cancelled", "reconciliation_required"].includes(state.status) || ["completed", "failed"].includes(state.last_event_kind)) return;
  await execution.fail("semantic_routing_failure");
}

function boundedText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedProbability(name: string, value: unknown, allowZero = true): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < (allowZero ? 0 : Number.MIN_VALUE) || value > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return value;
}

function routeReviewResult(deterministic: AutonomousRouteProposal, status: AutonomousSemanticRouteResult["status"], promptDigest: string, selectionDigest: string | null, selectedModel: { provider: string; model: string } | null, semanticCandidates: AutonomousSemanticRouteCandidate[] = [], selectedDomains: AutonomousDomainName[] = [], confidence = 0, outcomeDigest: string | null = null): AutonomousSemanticRouteResult {
  return { schema: AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA, status, route: deterministic, deterministic_route: deterministic, semantic_candidates: semanticCandidates, semantic_selected_domains: selectedDomains, semantic_confidence: confidence, selected_model: selectedModel, selection_digest: selectionDigest, prompt_digest: promptDigest, outcome_digest: outcomeDigest, retention: RETENTION, authorization: AUTHORIZATION };
}

function routeSchema(): JsonObject {
  return {
    type: "object",
    additionalProperties: false,
    properties: {
      selected_domains: {
        type: "array",
        minItems: 0,
        maxItems: 8,
        items: {
          type: "object",
          additionalProperties: false,
          properties: {
            domain: { type: "string", enum: [...AUTONOMOUS_DOMAIN_NAMES] },
            score: { type: "number", minimum: 0, maximum: 1 },
            rationale: { type: "string", maxLength: 512 },
          },
          required: ["domain", "score", "rationale"],
        },
      },
      confidence: { type: "number", minimum: 0, maximum: 1 },
      abstain: { type: "boolean" },
      abstain_reason: { type: ["string", "null"], maxLength: 512 },
    },
    required: ["selected_domains", "confidence", "abstain", "abstain_reason"],
  };
}

/** Ask an approved local provider for semantic routing while retaining deterministic fail-closed boundaries. */
export async function semanticRouteAutonomousTask(agent: AutonomousAgent, task: string, options: AutonomousSemanticRouteOptions = {}): Promise<AutonomousSemanticRouteResult> {
  if (!agent || typeof agent.route !== "function" || !agent.runtime) throw new ArgumentError("semantic routing requires an AutonomousAgent");
  const taskText = boundedText("semantic route task", task, 32_000);
  const deterministic = await agent.route(taskText, { hints: options.hints, allowCrossDomain: options.allowCrossDomain ?? true, maxDomains: options.maxDomains ?? 8 });
  const taskDigest = deterministic.task_digest;
  const profiles = await builtinAutonomousDomainProfiles();
  const profileByDomain = new Map(profiles.map((profile) => [profile.domain, profile]));
  const catalogue = profiles.map((profile) => ({ domain: profile.domain, capability: profile.default_capability, risk_class: profile.risk_class, capabilities: profile.capabilities, description: profile.tool_profile.description }));
  const promptDigest = await digestJson({ schema: AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA, task_digest: taskDigest, catalogue: catalogue.map((entry) => ({ domain: entry.domain, capability: entry.capability, risk_class: entry.risk_class, capabilities: entry.capabilities })) });
  if (options.approveProviderCall !== true) return routeReviewResult(deterministic, "approval_required", promptDigest, null, null);
  const candidates: AutonomousModelCandidate[] = options.candidates ? [...options.candidates] : agent.models();
  if (!candidates.length) throw new ProviderRuntimeError("semantic routing requires at least one model candidate");
  const catalogText = catalogue.map((entry) => `${entry.domain}: capability=${entry.capability}; risk=${entry.risk_class}; capabilities=${entry.capabilities.join(", ")}; ${entry.description}`).join("\n");
  const request = {
    model: "semantic-routing-selection-placeholder",
    messages: [
      { role: "system" as const, content: "You are a bounded autonomous task router. You classify work into the reviewed domain catalogue; you do not execute tools, make external changes, diagnose, or claim evidence. Use only catalogue domain names. Abstain when the task is underspecified. Return JSON only.", },
      { role: "developer" as const, content: `Reviewed domain catalogue:\n${catalogText}\n\nReturn selected_domains with at most ${options.maxDomains ?? 8} entries, a score in [0,1], a short rationale, confidence in [0,1], and abstain/abstain_reason.`, },
      { role: "user" as const, content: taskText },
    ],
    maxOutputTokens: options.maxOutputTokens ?? 512,
    requireJson: true,
    responseSchema: routeSchema(),
  };
  let execution: Awaited<ReturnType<AutonomousAgent["runtime"]["invoke"]>>;
  try {
    execution = await agent.runtime.invoke({ task: taskText, domain: "cross_domain", capability: "routing", riskClass: "route_review", requiredCapabilities: ["reasoning"], candidates, request }, { credential: options.credential, credentialFor: options.credentialFor, signal: options.signal, observer: options.observer, execution: options.execution, executionAttempt: options.executionAttempt, maxProviderFailovers: options.maxProviderFailovers });
  } catch (error) {
    if (error instanceof ProviderRuntimeError && error.code === "invalid_response") return routeReviewResult(deterministic, "provider_invalid", promptDigest, null, null);
    await failSemanticExecution(options);
    throw error;
  }
  const outcomeDigest = await digestJson({ status: "semantic_route", selection: execution.selection, response: execution.response });
  const selectionDigest = await digestJson(execution.selection);
  const selectedModel = execution.selection.selected_model;
  let payload: unknown = execution.response.structured;
  if (payload === null && execution.response.text.trim()) {
    try { payload = JSON.parse(execution.response.text); } catch { return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], 0, outcomeDigest); }
  }
  if (!isObject(payload) || !Array.isArray(payload.selected_domains)) return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], 0, outcomeDigest);
  let confidence: number;
  try { confidence = boundedProbability("semantic route confidence", payload.confidence); } catch { return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], 0, outcomeDigest); }
  const semanticCandidates: AutonomousSemanticRouteCandidate[] = [];
  const seen = new Set<AutonomousDomainName>();
  for (const row of payload.selected_domains.slice(0, 8)) {
    if (!isObject(row) || typeof row.domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(row.domain as AutonomousDomainName) || seen.has(row.domain as AutonomousDomainName) || typeof row.rationale !== "string") return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], confidence, outcomeDigest);
    let score: number;
    try { score = boundedProbability("semantic route score", row.score); } catch { return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], confidence, outcomeDigest); }
    try { boundedText("semantic route rationale", row.rationale, 512); } catch { return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], confidence, outcomeDigest); }
    const domain = row.domain as AutonomousDomainName;
    const profile = profileByDomain.get(domain);
    if (!profile) return routeReviewResult(deterministic, "provider_invalid", promptDigest, selectionDigest, selectedModel, [], [], confidence, outcomeDigest);
    seen.add(domain);
    semanticCandidates.push({ domain, score, capability: profile.default_capability, rationale: row.rationale });
  }
  semanticCandidates.sort((left, right) => right.score - left.score || left.domain.localeCompare(right.domain));
  const minConfidence = options.minSemanticConfidence ?? 0.35;
  boundedProbability("minSemanticConfidence", minConfidence, false);
  const maxDomains = options.maxDomains ?? 8;
  if (!Number.isSafeInteger(maxDomains) || maxDomains < 1 || maxDomains > 8) throw new ArgumentError("semantic route maxDomains must be between 1 and 8");
  const selectedDomains = semanticCandidates.filter((candidate) => candidate.score >= minConfidence).slice(0, maxDomains).map((candidate) => candidate.domain);
  const abstained = payload.abstain === true || selectedDomains.length === 0;
  if (abstained) return routeReviewResult(deterministic, "provider_abstained", promptDigest, selectionDigest, selectedModel, semanticCandidates, [], confidence, outcomeDigest);
  const deterministicDomains = new Set(deterministic.selected_domains);
  const agrees = deterministic.abstained
    ? true
    : deterministic.cross_domain
      ? selectedDomains.some((domain) => deterministicDomains.has(domain))
      : selectedDomains.length === 1 && selectedDomains[0] === deterministic.primary_domain;
  if (!agrees) return routeReviewResult(deterministic, "provider_disagreement", promptDigest, selectionDigest, selectedModel, semanticCandidates, selectedDomains, confidence, outcomeDigest);
  const routeCandidates: AutonomousRouteCandidate[] = semanticCandidates.filter((candidate) => selectedDomains.includes(candidate.domain)).map((candidate) => {
    const profile = profileByDomain.get(candidate.domain)!;
    return { domain: candidate.domain, score: candidate.score, matched_terms: ["provider_semantic_candidate"], capability: profile.default_capability, risk_class: profile.risk_class, workflow_id: profile.workflow.workflow_id, evidence: "provider_semantic_candidate" as const };
  });
  const routeDescriptor = {
    schema: deterministic.schema,
    task_digest: deterministic.task_digest,
    candidates: routeCandidates,
    selected_domains: selectedDomains,
    primary_domain: selectedDomains[0] ?? null,
    confidence,
    abstained: false,
    reason: selectedDomains.length > 1 ? "cross_domain" as const : "routed" as const,
    cross_domain: selectedDomains.length > 1,
    source: "provider_semantic_hybrid" as const,
    retention: "route_scores_and_digests_only; task_text_is_not_retained_in_route" as const,
    does_not_claim: ["provider semantic output is a routing hypothesis, not evidence", "routing does not authorize tools, provider calls, or external effects"],
  };
  const route = { ...routeDescriptor, route_digest: await digestJson(routeDescriptor) };
  return { schema: AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA, status: "completed", route, deterministic_route: deterministic, semantic_candidates: routeCandidates.map((candidate) => semanticCandidates.find((row) => row.domain === candidate.domain)!), semantic_selected_domains: selectedDomains, semantic_confidence: confidence, selected_model: selectedModel, selection_digest: selectionDigest, prompt_digest: promptDigest, outcome_digest: outcomeDigest, retention: RETENTION, authorization: AUTHORIZATION };
}
