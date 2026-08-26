import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import type { AutonomousProviderInvocationReceipt } from "./llm.js";
import { canonicalJson, digestCanonicalJsonText, digestJson } from "./tooling.js";
import type { BrainBanditContext, JsonObject } from "./types.js";

/** Stable schema for the provider-receipt-to-model-learning boundary. */
export const AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-provider-evaluation/0.1" as const;
export const AUTONOMOUS_PROVIDER_LEARNING_SCHEMA = "bioprism-typescript-autonomous-provider-learning/0.1" as const;
export const MAX_AUTONOMOUS_PROVIDER_EVALUATION_EVIDENCE_BYTES = 256_000;
export const MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS = 256;

const PROVIDER_STATUSES = new Set<AutonomousProviderInvocationReceipt["status"]>(["completed", "provider_refused"]);
const PROVIDER_OUTCOMES = new Set<AutonomousProviderInvocationReceipt["outcome"]>(["success", "failure"]);
const SAFE_IDENTIFIER = /^[A-Za-z0-9_.-]+$/;
const SHA256 = /^[0-9a-f]{64}$/;
const FORBIDDEN_FIELDS = new Set([
  "apikey", "authorization", "bearer", "credential", "password", "secret",
  "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response",
  "rawpayload", "arguments", "output", "task", "messages", "headers", "body",
]);

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || value.length === 0 || value.includes("\u0000") || bytes(value) > maximum) {
    throw new ArgumentError(`${name} is outside its bounded text contract`);
  }
  return value;
}

function boundedIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!SAFE_IDENTIFIER.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function boundedDigest(name: string, value: unknown, nullable = false): string | null {
  if (value === null || value === undefined) {
    if (nullable) return null;
    throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  }
  if (typeof value !== "string" || !SHA256.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedNonnegative(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > maximum) throw new ArgumentError(`${name} must be finite and within its bound`);
  return value;
}

function boundedInteger(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > maximum) throw new ArgumentError(`${name} must be a non-negative safe integer within its bound`);
  return value;
}

function assertSafeMetadata(value: unknown, depth = 0): void {
  if (depth > 32) throw new ArgumentError("provider evaluator evidence is too deeply nested");
  if (Array.isArray(value)) {
    if (value.length > 4096) throw new ArgumentError("provider evaluator evidence contains too many array items");
    for (const child of value) assertSafeMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 4096) throw new ArgumentError("provider evaluator evidence contains too many object keys");
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (FORBIDDEN_FIELDS.has(normalized)) throw new ArgumentError("provider evaluator evidence contains transient or secret-shaped fields");
      assertSafeMetadata(child, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError("provider evaluator evidence contains a non-finite number");
}

function safeObject(name: string, value: unknown): JsonObject {
  if (!isObject(value)) throw new ArgumentError(`${name} must be an object`);
  assertSafeMetadata(value);
  let encoded: string;
  try {
    encoded = canonicalJson(value);
  } catch (error) {
    throw new ArgumentError(`${name} must be canonical JSON`, { cause: error });
  }
  if (bytes(encoded) > MAX_AUTONOMOUS_PROVIDER_EVALUATION_EVIDENCE_BYTES) throw new ArgumentError(`${name} exceeds its bounded size`);
  return JSON.parse(encoded) as JsonObject;
}

function optionalIdentifier(name: string, value: unknown): string | null {
  return value === null || value === undefined ? null : boundedIdentifier(name, value);
}

function optionalDigest(name: string, value: unknown): string | null {
  return boundedDigest(name, value, true);
}

function providerExecutionId(receipt: AutonomousProviderInvocationReceipt): string | null {
  return receipt.execution_id === null || receipt.execution_id === undefined
    ? null
    : boundedIdentifier("provider receipt execution_id", receipt.execution_id);
}

/** Stable identity used for evidence/context maps and replay deduplication. */
export function autonomousProviderReceiptIdentity(receipt: AutonomousProviderInvocationReceipt): string {
  const metadata = providerReceiptMetadata(receipt);
  return `${metadata.execution_id ?? "unjournaled"}:${metadata.provider}/${metadata.model}:${metadata.attempt}:${metadata.turn}:${metadata.outcome_digest}`;
}

interface ProviderReceiptMetadata {
  execution_id: string | null;
  provider: string;
  model: string;
  kind: string;
  attempt: number;
  turn: number;
  status: AutonomousProviderInvocationReceipt["status"];
  outcome: AutonomousProviderInvocationReceipt["outcome"];
  input_tokens: number;
  output_tokens: number;
  estimated_cost_units: number;
  actual_cost_units: number;
  latency_ms: number;
  selection_digest: string;
  outcome_digest: string;
  request_id_digest: string | null;
  failure_class: string | null;
  status_code: number | null;
}

function providerReceiptMetadata(receipt: AutonomousProviderInvocationReceipt): ProviderReceiptMetadata {
  if (!isObject(receipt) || receipt.schema !== "bioprism-typescript-autonomous-provider-invocation/0.1") throw new ArgumentError("provider evaluation requires an autonomous provider invocation receipt");
  if (!PROVIDER_STATUSES.has(receipt.status)) throw new ArgumentError("provider evaluation receipt status is invalid");
  if (!PROVIDER_OUTCOMES.has(receipt.outcome)) throw new ArgumentError("provider evaluation receipt outcome is invalid");
  if ((receipt.status === "completed") !== (receipt.outcome === "success")) throw new ArgumentError("provider evaluation receipt status and outcome disagree");
  const statusCode = receipt.status_code;
  if (statusCode !== null && statusCode !== undefined && (!Number.isSafeInteger(statusCode) || statusCode < 100 || statusCode > 599)) throw new ArgumentError("provider evaluation receipt status_code is invalid");
  return {
    execution_id: providerExecutionId(receipt),
    provider: boundedIdentifier("provider receipt provider", receipt.provider),
    model: boundedIdentifier("provider receipt model", receipt.model),
    kind: boundedIdentifier("provider receipt kind", receipt.kind),
    attempt: boundedInteger("provider receipt attempt", receipt.attempt, 64),
    turn: boundedInteger("provider receipt turn", receipt.turn, 256),
    status: receipt.status,
    outcome: receipt.outcome,
    input_tokens: boundedInteger("provider receipt input_tokens", receipt.input_tokens, 1_000_000_000),
    output_tokens: boundedInteger("provider receipt output_tokens", receipt.output_tokens, 1_000_000_000),
    estimated_cost_units: boundedNonnegative("provider receipt estimated_cost_units", receipt.estimated_cost_units, 1_000_000_000),
    actual_cost_units: boundedNonnegative("provider receipt actual_cost_units", receipt.actual_cost_units, 1_000_000_000),
    latency_ms: boundedNonnegative("provider receipt latency_ms", receipt.latency_ms, 86_400_000),
    selection_digest: boundedDigest("provider receipt selection_digest", receipt.selection_digest)!,
    outcome_digest: boundedDigest("provider receipt outcome_digest", receipt.outcome_digest)!,
    request_id_digest: optionalDigest("provider receipt request_id_digest", receipt.request_id_digest),
    failure_class: optionalIdentifier("provider receipt failure_class", receipt.failure_class),
    status_code: statusCode === null || statusCode === undefined ? null : statusCode,
  };
}

export interface AutonomousProviderOutcomeContext extends JsonObject {
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  task_family?: string | null;
  contract_digest?: string | null;
  context_digest?: string | null;
}

interface NormalizedProviderContext {
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  task_family: string | null;
  contract_digest: string | null;
  context_digest: string | null;
  context: BrainBanditContext;
}

async function normalizeProviderContext(value: AutonomousProviderOutcomeContext | undefined): Promise<NormalizedProviderContext> {
  if (value === undefined) {
    const context: BrainBanditContext = { domain: "cross_domain", capability: "provider_invocation", risk_class: "provider_call", task_family: null };
    return { domain: "cross_domain", capability: context.capability, risk_class: context.risk_class, task_family: null, contract_digest: null, context_digest: null, context };
  }
  if (!isObject(value)) throw new ArgumentError("provider evaluation context must be an object");
  const keys = Object.keys(value);
  const allowed = new Set(["domain", "capability", "risk_class", "task_family", "contract_digest", "context_digest"]);
  if (keys.some((key) => !allowed.has(key))) throw new ArgumentError("provider evaluation context contains unsupported fields");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value.domain as AutonomousDomainName)) throw new ArgumentError("provider evaluation context domain is unsupported");
  const capability = boundedIdentifier("provider evaluation context capability", value.capability);
  const riskClass = boundedIdentifier("provider evaluation context risk_class", value.risk_class);
  const taskFamily = optionalIdentifier("provider evaluation context task_family", value.task_family);
  const contractDigest = optionalDigest("provider evaluation context contract_digest", value.contract_digest);
  const context: BrainBanditContext = { domain: value.domain as AutonomousDomainName, capability, risk_class: riskClass, task_family: taskFamily };
  const suppliedContextDigest = optionalDigest("provider evaluation context context_digest", value.context_digest);
  // The online learner's contextual identity intentionally preserves the
  // normalized field order for Rust/Python parity. Keep this bridge on the
  // same contract instead of using canonical object-key sorting here.
  const contextDigest = await digestCanonicalJsonText(JSON.stringify(context));
  if (suppliedContextDigest !== null && suppliedContextDigest !== contextDigest) throw new ArgumentError("provider evaluation context_digest does not match its context");
  return { domain: context.domain as AutonomousDomainName, capability, risk_class: riskClass, task_family: taskFamily, contract_digest: contractDigest, context_digest: contextDigest, context };
}

/** Safe evaluator input projected from one provider receipt. */
export interface AutonomousProviderOutcomeEvaluationInput extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA;
  receipt_identity: string;
  execution_id: string | null;
  provider: string;
  model: string;
  kind: string;
  attempt: number;
  turn: number;
  status: AutonomousProviderInvocationReceipt["status"];
  outcome: AutonomousProviderInvocationReceipt["outcome"];
  input_tokens: number;
  output_tokens: number;
  estimated_cost_units: number;
  actual_cost_units: number;
  latency_ms: number;
  selection_digest: string;
  outcome_digest: string;
  request_id_digest: string | null;
  failure_class: string | null;
  status_code: number | null;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  task_family: string | null;
  contract_digest: string | null;
  context_digest: string | null;
  context: BrainBanditContext;
  evidence_digest: string;
  evidence: JsonObject;
  retention: "digests_and_safe_evidence_only_no_provider_payloads_or_credentials";
}

/** Build a provider-receipt projection without exposing messages, responses, credentials, or payloads. */
export async function autonomousProviderOutcomeEvaluationInput(
  receipt: AutonomousProviderInvocationReceipt,
  options: { context?: AutonomousProviderOutcomeContext; evidence?: JsonObject } = {},
): Promise<AutonomousProviderOutcomeEvaluationInput> {
  const metadata = providerReceiptMetadata(receipt);
  const context = await normalizeProviderContext(options.context);
  const evidence = safeObject("provider evaluator evidence", options.evidence ?? {});
  const identity = `${metadata.execution_id ?? "unjournaled"}:${metadata.provider}/${metadata.model}:${metadata.attempt}:${metadata.turn}:${metadata.outcome_digest}`;
  const evidenceDigest = await digestJson(evidence);
  return {
    schema: AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA,
    receipt_identity: identity,
    ...metadata,
    domain: context.domain,
    capability: context.capability,
    risk_class: context.risk_class,
    task_family: context.task_family,
    contract_digest: context.contract_digest,
    context_digest: context.context_digest,
    context: context.context,
    evidence_digest: evidenceDigest,
    evidence,
    retention: "digests_and_safe_evidence_only_no_provider_payloads_or_credentials",
  };
}

export interface AutonomousProviderEvaluatorAssessment extends JsonObject {
  evaluator_id?: string;
  evaluator_version?: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: string | null;
  failure_class?: string | null;
  evidence_digest?: string | null;
}

export interface AutonomousProviderEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA;
  receipt_identity: string;
  execution_id: string | null;
  domain: AutonomousDomainName;
  capability: string;
  risk_class: string;
  contract_digest: string | null;
  context_digest: string | null;
  provider: string;
  model: string;
  arm_id: string;
  status: AutonomousProviderInvocationReceipt["status"];
  outcome: AutonomousProviderInvocationReceipt["outcome"];
  attempt: number;
  turn: number;
  evidence_digest: string;
  decision_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  model_outcome_digest: string;
  idempotent_replay: boolean;
}

export interface AutonomousProviderOutcomeEvaluatorOptions {
  evaluator_id: string;
  evaluator_version: string;
  evaluate: (input: AutonomousProviderOutcomeEvaluationInput) => AutonomousProviderEvaluatorAssessment | Promise<AutonomousProviderEvaluatorAssessment>;
}

export interface AutonomousProviderLearningUpdate {
  failed: boolean;
  outcomeDigest: string;
  contractDigest: string | null;
  contextDigest: string | null;
  context?: BrainBanditContext;
}

export interface AutonomousProviderLearningUpdater {
  (armId: string, reward: number, update: AutonomousProviderLearningUpdate): unknown | Promise<unknown>;
}

export interface AutonomousProviderLearningEvaluation extends AutonomousProviderEvaluation {
  learning_update: "applied" | "not_configured";
}

export interface AutonomousProviderLearningReport extends JsonObject {
  schema: typeof AUTONOMOUS_PROVIDER_LEARNING_SCHEMA;
  status: "completed" | "no_receipts";
  receipts: number;
  evaluations: AutonomousProviderLearningEvaluation[];
  by_domain: Record<string, number>;
  by_status: Record<string, number>;
  by_model: Record<string, number>;
  next_learning_state: JsonObject | null;
  next_learning_state_digest: string | null;
  learning_digest: string;
  retention: "metadata_and_digests_only";
  secret_material: "never_returned";
}

interface NormalizedProviderAssessment {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed: boolean;
  feedback_digest: string | null;
  failure_class: string | null;
  evidence_digest: string;
}

function normalizeAssessment(raw: AutonomousProviderEvaluatorAssessment, evaluatorId: string, evaluatorVersion: string, evidenceDigest: string): NormalizedProviderAssessment {
  if (!isObject(raw)) throw new ArgumentError("provider evaluator callback must return an object");
  const allowed = new Set(["evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest"]);
  if (Object.keys(raw).some((key) => !allowed.has(key))) throw new ArgumentError("provider evaluator decision contains unsupported fields");
  if (raw.evaluator_id !== undefined && raw.evaluator_id !== evaluatorId) throw new ArgumentError("provider evaluator decision identity does not match the evaluator");
  if (raw.evaluator_version !== undefined && raw.evaluator_version !== evaluatorVersion) throw new ArgumentError("provider evaluator decision identity does not match the evaluator");
  if (typeof raw.reward !== "number" || !Number.isFinite(raw.reward) || raw.reward < -1 || raw.reward > 1) throw new ArgumentError("provider evaluator reward must be finite and within [-1, 1]");
  if (typeof raw.passed !== "boolean") throw new ArgumentError("provider evaluator passed must be boolean");
  const failed = raw.failed === undefined ? !raw.passed : raw.failed;
  if (typeof failed !== "boolean") throw new ArgumentError("provider evaluator failed must be boolean");
  if (raw.passed && failed) throw new ArgumentError("provider evaluator cannot be both passed and failed");
  const feedbackDigest = optionalDigest("provider evaluator feedback_digest", raw.feedback_digest);
  const failureClass = optionalIdentifier("provider evaluator failure_class", raw.failure_class);
  const returnedEvidenceDigest = optionalDigest("provider evaluator evidence_digest", raw.evidence_digest);
  if (returnedEvidenceDigest !== null && returnedEvidenceDigest !== evidenceDigest) throw new ArgumentError("provider evaluator evidence_digest does not match the input");
  return { evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, reward: raw.reward, passed: raw.passed, failed, feedback_digest: feedbackDigest, failure_class: failureClass, evidence_digest: evidenceDigest };
}

function priorLearningCredit(state: JsonObject | null, outcomeDigest: string): boolean {
  if (!state || !Array.isArray(state.credited_outcomes)) return false;
  return state.credited_outcomes.some((credit) => isObject(credit) && credit.outcome_digest === outcomeDigest);
}

/**
 * Independent evaluator adapter for provider/model receipts. Provider transport success is never
 * converted into reward; only the caller-owned evaluator can create a model-learning update.
 */
export class AutonomousProviderOutcomeEvaluator {
  readonly evaluatorId: string;
  readonly evaluatorVersion: string;
  private readonly callback: AutonomousProviderOutcomeEvaluatorOptions["evaluate"];

  constructor(options: AutonomousProviderOutcomeEvaluatorOptions) {
    if (!options || typeof options !== "object" || typeof options.evaluate !== "function") throw new ArgumentError("provider evaluator requires an evaluate callback");
    this.evaluatorId = boundedIdentifier("provider evaluator_id", options.evaluator_id);
    this.evaluatorVersion = boundedIdentifier("provider evaluator_version", options.evaluator_version);
    this.callback = options.evaluate;
  }

  async assess(input: AutonomousProviderOutcomeEvaluationInput): Promise<AutonomousProviderEvaluation> {
    if (!isObject(input) || input.schema !== AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA) throw new ArgumentError("provider evaluator input schema is invalid");
    // Rebuild the receipt independently because the public input is intentionally not a receipt.
    // This revalidates every receipt field before the callback sees the projection.
    const receipt: AutonomousProviderInvocationReceipt = {
      schema: "bioprism-typescript-autonomous-provider-invocation/0.1",
      execution_id: input.execution_id,
      provider: input.provider,
      model: input.model,
      kind: input.kind,
      attempt: input.attempt,
      turn: input.turn,
      status: input.status,
      outcome: input.outcome,
      input_tokens: input.input_tokens,
      output_tokens: input.output_tokens,
      estimated_cost_units: input.estimated_cost_units,
      actual_cost_units: input.actual_cost_units,
      latency_ms: input.latency_ms,
      selection_digest: input.selection_digest,
      outcome_digest: input.outcome_digest,
      request_id_digest: input.request_id_digest,
      failure_class: input.failure_class as AutonomousProviderInvocationReceipt["failure_class"],
      status_code: input.status_code,
      retention: "metadata_only_no_provider_payloads_or_credentials",
      secret_material: "never_returned",
    };
    const rebuilt = await autonomousProviderOutcomeEvaluationInput(receipt, {
      context: input.context_digest === null ? undefined : {
        domain: input.domain,
        capability: input.capability,
        risk_class: input.risk_class,
        task_family: input.task_family,
        contract_digest: input.contract_digest,
        context_digest: input.context_digest,
      },
      evidence: input.evidence,
    });
    if (rebuilt.receipt_identity !== input.receipt_identity || rebuilt.evidence_digest !== input.evidence_digest || rebuilt.context_digest !== input.context_digest) throw new ArgumentError("provider evaluator input identity is invalid");
    let raw: AutonomousProviderEvaluatorAssessment;
    try {
      raw = await this.callback(structuredClone(rebuilt));
    } catch (error) {
      throw new ArgumentError("provider evaluator callback failed", { cause: error });
    }
    const decision = normalizeAssessment(raw, this.evaluatorId, this.evaluatorVersion, input.evidence_digest);
    const base = {
      schema: AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA,
      receipt_identity: input.receipt_identity,
      execution_id: input.execution_id,
      domain: input.domain,
      capability: input.capability,
      risk_class: input.risk_class,
      contract_digest: input.contract_digest,
      context_digest: input.context_digest,
      provider: input.provider,
      model: input.model,
      arm_id: `${input.provider}/${input.model}`,
      status: input.status,
      outcome: input.outcome,
      attempt: input.attempt,
      turn: input.turn,
      evidence_digest: input.evidence_digest,
      evaluator_id: decision.evaluator_id,
      evaluator_version: decision.evaluator_version,
      reward: decision.reward,
      passed: decision.passed,
      failed: decision.failed,
      feedback_digest: decision.feedback_digest,
      failure_class: decision.failure_class,
      retention: "value_only" as const,
    };
    const decisionDigest = await digestJson(base);
    const { evidence: _evidence, ...metadata } = rebuilt;
    const modelOutcomeDigest = await digestJson({
      schema: "bioprism-autonomous-provider-model-outcome/0.1",
      receipt_identity: input.receipt_identity,
      input_digest: await digestJson(metadata),
      decision_digest: decisionDigest,
    });
    return { ...base, decision_digest: decisionDigest, model_outcome_digest: modelOutcomeDigest, idempotent_replay: false };
  }

  async evaluateReceipts(
    receipts: readonly AutonomousProviderInvocationReceipt[],
    options: {
      contexts?: Readonly<Record<string, AutonomousProviderOutcomeContext>>;
      evidence?: Readonly<Record<string, JsonObject>>;
      learningState?: JsonObject | null;
      learningUpdater?: AutonomousProviderLearningUpdater;
    } = {},
  ): Promise<AutonomousProviderLearningReport> {
    if (!Array.isArray(receipts) || receipts.length > MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS) throw new ArgumentError(`provider receipt batches must contain at most ${MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS} entries`);
    const metadata = receipts.map((receipt) => ({ receipt, value: providerReceiptMetadata(receipt) }));
    const identities = metadata.map(({ receipt }) => autonomousProviderReceiptIdentity(receipt));
    if (new Set(identities).size !== identities.length) throw new ArgumentError("provider receipt batches cannot contain duplicate identities");
    const outcomeDigests = metadata.map(({ value }) => value.outcome_digest);
    const uniqueOutcomeDigests = new Set(outcomeDigests.filter((digest) => outcomeDigests.filter((candidate) => candidate === digest).length === 1));
    if (options.evidence !== undefined && (!isObject(options.evidence) || Object.values(options.evidence).some((packet) => !isObject(packet)))) throw new ArgumentError("provider receipt evidence must map identities to objects");
    if (options.contexts !== undefined && (!isObject(options.contexts) || Object.values(options.contexts).some((context) => !isObject(context)))) throw new ArgumentError("provider receipt contexts must map identities to objects");
    const evidence = options.evidence ?? {};
    const contexts = options.contexts ?? {};
    const validKeys = new Set([...identities, ...uniqueOutcomeDigests]);
    if (Object.keys(evidence).some((key) => !validKeys.has(key))) throw new ArgumentError("provider receipt evidence contains an unknown receipt identity");
    if (Object.keys(contexts).some((key) => !validKeys.has(key))) throw new ArgumentError("provider receipt contexts contains an unknown receipt identity");
    let learningState = options.learningState === undefined || options.learningState === null ? null : safeObject("provider learning state", options.learningState);
    const evaluations: AutonomousProviderLearningEvaluation[] = [];
    const byDomain: Record<string, number> = {};
    const byStatus: Record<string, number> = {};
    const byModel: Record<string, number> = {};
    for (const { receipt, value } of metadata) {
      const identity = autonomousProviderReceiptIdentity(receipt);
      const evidencePacket = evidence[identity] ?? (uniqueOutcomeDigests.has(value.outcome_digest) ? evidence[value.outcome_digest] : undefined) ?? {};
      const context = contexts[identity] ?? (uniqueOutcomeDigests.has(value.outcome_digest) ? contexts[value.outcome_digest] : undefined);
      const input = await autonomousProviderOutcomeEvaluationInput(receipt, { context, evidence: evidencePacket });
      const decision = await this.assess(input);
      const { evidence: _evidence, ...metadataInput } = input;
      const inputDigest = await digestJson(metadataInput);
      const modelOutcomeDigest = await digestJson({ schema: "bioprism-autonomous-provider-model-outcome/0.1", receipt_identity: identity, input_digest: inputDigest, decision_digest: decision.decision_digest });
      const replay = priorLearningCredit(learningState, modelOutcomeDigest);
      let learningUpdate: "applied" | "not_configured" = "not_configured";
      if (options.learningUpdater) {
        let updated: unknown;
        try {
          updated = await options.learningUpdater(decision.arm_id, decision.reward, {
            failed: decision.failed,
            outcomeDigest: modelOutcomeDigest,
            contractDigest: input.contract_digest ?? input.selection_digest,
            contextDigest: input.context_digest,
            ...(input.context_digest === null ? {} : { context: input.context }),
          });
        } catch (error) {
          throw new ArgumentError("provider learning updater failed", { cause: error });
        }
        if (updated !== undefined) learningState = safeObject("next provider learning state", updated);
        learningUpdate = "applied";
      }
      evaluations.push({
        ...decision,
        model_outcome_digest: modelOutcomeDigest,
        idempotent_replay: replay,
        learning_update: learningUpdate,
      });
      byDomain[input.domain] = (byDomain[input.domain] ?? 0) + 1;
      byStatus[input.status] = (byStatus[input.status] ?? 0) + 1;
      byModel[decision.arm_id] = (byModel[decision.arm_id] ?? 0) + 1;
    }
    const digestEvaluations = evaluations.map(({ idempotent_replay: _replay, learning_update: _update, ...evaluation }) => evaluation);
    const learningDigest = await digestJson(digestEvaluations);
    return {
      schema: AUTONOMOUS_PROVIDER_LEARNING_SCHEMA,
      status: evaluations.length ? "completed" : "no_receipts",
      receipts: evaluations.length,
      evaluations,
      by_domain: Object.fromEntries(Object.entries(byDomain).sort(([left], [right]) => left.localeCompare(right))),
      by_status: Object.fromEntries(Object.entries(byStatus).sort(([left], [right]) => left.localeCompare(right))),
      by_model: Object.fromEntries(Object.entries(byModel).sort(([left], [right]) => left.localeCompare(right))),
      next_learning_state: learningState,
      next_learning_state_digest: learningState === null ? null : await digestJson(learningState),
      learning_digest: learningDigest,
      retention: "metadata_and_digests_only",
      secret_material: "never_returned",
    };
  }
}
