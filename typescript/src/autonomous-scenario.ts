import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  type AutonomousDomainName,
  type AutonomousModelSelectionPreview,
  type AutonomousPromptChunk,
} from "./autonomous.js";
import {
  AutonomousValueEvaluatorRegistry,
  type AutonomousValueEvaluation,
  type AutonomousValueEvaluationInput,
} from "./autonomous-domain-evaluators.js";
import {
  assertAutonomousEvaluatorCalibrationReady,
  validateAutonomousEvaluatorCalibrationReport,
} from "./autonomous-evaluator-calibration.js";
import type { AutonomousEvaluatorCalibrationReport } from "./autonomous-evaluator-calibration.js";
import { digestJsonSync } from "./tooling.js";
import type { AutonomousModelCandidate } from "./llm.js";
import type { JsonObject } from "./types.js";

/** Shared metadata-only scenario matrix schema for offline autonomy validation. */
export const AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA = "bioprism-autonomous-offline-scenario/0.1" as const;
export const AUTONOMOUS_OFFLINE_SCENARIO_REPLAY_SCHEMA = "bioprism-autonomous-offline-scenario-replay/0.1" as const;
export const MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES = 750_000;

const SHA256 = /^[0-9a-f]{64}$/;

export interface AutonomousOfflineScenarioCase {
  id?: string;
  task: string;
  domain: AutonomousDomainName;
  capability?: string;
  context?: readonly AutonomousPromptChunk[];
  candidates?: readonly AutonomousModelCandidate[];
  /** Optional bounded value-only evidence packet; it is never copied into a report. */
  evidence?: JsonObject;
}

export interface AutonomousOfflineScenarioExecutionMetadata extends JsonObject {
  status: string;
  selected_model: { provider: string; model: string };
  provider_request_id: string | null;
  response_metadata: {
    provider: string;
    model: string;
    request_id: string | null;
    structured: boolean;
    tool_call_count: number;
  } | null;
}

export interface AutonomousOfflineScenarioEvidenceContext {
  case: AutonomousOfflineScenarioCase;
  preview: AutonomousModelSelectionPreview;
  execution: AutonomousOfflineScenarioExecutionMetadata;
}

export type AutonomousOfflineScenarioEvidenceFactory = (
  context: AutonomousOfflineScenarioEvidenceContext,
) => AutonomousValueEvaluationInput | Promise<AutonomousValueEvaluationInput>;

export interface AutonomousOfflineScenarioRunOptions {
  cases: readonly AutonomousOfflineScenarioCase[];
  evidenceFor?: AutonomousOfflineScenarioEvidenceFactory;
  evaluatorRegistry?: AutonomousValueEvaluatorRegistry;
  /** Require a previously validated holdout calibration report before settling learning. */
  calibrationReport?: AutonomousEvaluatorCalibrationReport;
  requireCalibratedLearning?: boolean;
}

export interface AutonomousOfflineScenarioAllDomainsOptions {
  /** One task per built-in domain. Missing entries use a deterministic bounded local task. */
  tasks?: Partial<Record<AutonomousDomainName, string>>;
  taskForDomain?: (domain: AutonomousDomainName) => string | Promise<string>;
  evidenceFor: AutonomousOfflineScenarioEvidenceFactory;
  capabilityForDomain?: (domain: AutonomousDomainName) => string | undefined;
  contextForDomain?: (domain: AutonomousDomainName) => readonly AutonomousPromptChunk[] | undefined;
  candidatesForDomain?: (domain: AutonomousDomainName) => readonly AutonomousModelCandidate[] | undefined;
  evaluatorRegistry?: AutonomousValueEvaluatorRegistry;
  calibrationReport?: AutonomousEvaluatorCalibrationReport;
  requireCalibratedLearning?: boolean;
}

export interface AutonomousOfflineScenarioCaseReport extends JsonObject {
  id: string;
  domain: AutonomousDomainName;
  status: "completed" | "selection_refused";
  task_digest: string;
  selected_model: { provider: string; model: string } | null;
  selection_digest: string;
  selection_contract_digest: string;
  execution: {
    status: string | null;
    provider_request_id: string | null;
  };
  evaluation: {
    evaluator_id: string;
    evaluator_version: string;
    reward: number;
    passed: boolean;
    failed: boolean;
    failure_class: string | null;
    feedback_digest: string | null;
    evidence_digest: string | null;
    evaluation_digest: string;
  } | null;
  learning: {
    arm_id: string | null;
    outcome_digest: string | null;
    contract_digest: string | null;
    generation: number | null;
  };
  retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousOfflineScenarioReport extends JsonObject {
  schema: typeof AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA;
  status: "completed" | "partial";
  case_count: number;
  completed_count: number;
  refused_count: number;
  domains: AutonomousDomainName[];
  cases: AutonomousOfflineScenarioCaseReport[];
  evaluator_catalogue_digest: string;
  learning_state_digest: string | null;
  learning_generation: number | null;
  report_digest: string;
  execution: "offline_provider_invocation_allowed;external_network_not_required";
  retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousOfflineScenarioReplayResult extends JsonObject {
  schema: typeof AUTONOMOUS_OFFLINE_SCENARIO_REPLAY_SCHEMA;
  source_report_digest: string;
  case_count: number;
  verified_count: number;
  replayed_count: number;
  learner_generation_before: number | null;
  learner_generation_after: number | null;
  idempotent: boolean;
  replay_digest: string;
  execution: "metadata_only;no_provider_or_tool_invocation";
  retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained";
  secret_material: "never_returned";
}

function boundedText(name: string, value: unknown, maximum = 32_000): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000")) throw new ArgumentError(`${name} must be a non-empty string`);
  if (new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} exceeds its bounded size`);
  return value;
}

function boundedId(value: unknown, fallback: string): string {
  if (value === undefined) return fallback;
  return boundedText("offline scenario case id", value, 256);
}

function assertDigest(name: string, value: unknown): asserts value is string {
  if (typeof value !== "string" || !SHA256.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
}

function cloneCase(value: AutonomousOfflineScenarioCase, index: number): AutonomousOfflineScenarioCase {
  if (!isObject(value)) throw new ArgumentError("offline scenario cases must be objects");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value.domain as AutonomousDomainName)) throw new ArgumentError("offline scenario case domain must be a built-in domain");
  const task = boundedText("offline scenario task", value.task);
  const id = boundedId(value.id, `${value.domain}-${index + 1}`);
  return {
    id,
    task,
    domain: value.domain as AutonomousDomainName,
    ...(value.capability === undefined ? {} : { capability: boundedText("offline scenario capability", value.capability, 256) }),
    ...(value.context === undefined ? {} : { context: structuredClone(value.context) }),
    ...(value.candidates === undefined ? {} : { candidates: structuredClone(value.candidates) }),
    ...(value.evidence === undefined ? {} : { evidence: structuredClone(value.evidence) }),
  };
}

function executionMetadata(result: { status: string; response?: unknown }): AutonomousOfflineScenarioExecutionMetadata {
  const response = isObject(result.response) ? result.response : null;
  const provider = response && typeof response.provider === "string" ? response.provider : null;
  const model = response && typeof response.model === "string" ? response.model : null;
  const requestId = response && typeof response.requestId === "string" ? response.requestId : null;
  const structured = response ? response.structured !== undefined && response.structured !== null : false;
  const toolCallCount = response && Array.isArray(response.toolCalls) ? response.toolCalls.length : 0;
  return {
    status: boundedText("offline scenario execution status", result.status, 128),
    selected_model: { provider: provider ?? "unknown", model: model ?? "unknown" },
    provider_request_id: requestId,
    response_metadata: response && provider && model ? { provider, model, request_id: requestId, structured, tool_call_count: toolCallCount } : null,
    secret_material: "never_returned",
  };
}

function safeEvaluation(value: AutonomousValueEvaluation): AutonomousOfflineScenarioCaseReport["evaluation"] {
  return {
    evaluator_id: value.evaluator_id,
    evaluator_version: value.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed,
    failure_class: value.failure_class,
    feedback_digest: value.feedback_digest,
    evidence_digest: value.evidence_digest,
    evaluation_digest: value.evaluation_digest,
  };
}

function reportWithoutDigest(report: AutonomousOfflineScenarioReport): JsonObject {
  const { report_digest: _reportDigest, ...withoutDigest } = report;
  return withoutDigest;
}

function assertReport(value: unknown): AutonomousOfflineScenarioReport {
  if (!isObject(value) || value.schema !== AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA || !Array.isArray(value.cases)) throw new ArgumentError("offline scenario report is malformed");
  if (value.cases.length > MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES) throw new ArgumentError("offline scenario report exceeds its case bound");
  assertDigest("offline scenario report_digest", value.report_digest);
  if (digestJsonSync(reportWithoutDigest(value as AutonomousOfflineScenarioReport)) !== value.report_digest) throw new ArgumentError("offline scenario report digest does not match its metadata");
  const domains = new Set<string>();
  for (const row of value.cases) {
    if (!isObject(row) || !AUTONOMOUS_DOMAIN_NAMES.includes(row.domain as AutonomousDomainName)) throw new ArgumentError("offline scenario report case domain is invalid");
    if (domains.has(row.domain as string)) throw new ArgumentError("offline scenario report must contain at most one case per domain");
    domains.add(row.domain as string);
    assertDigest("offline scenario task_digest", row.task_digest);
    assertDigest("offline scenario selection_digest", row.selection_digest);
    assertDigest("offline scenario selection_contract_digest", row.selection_contract_digest);
    const learning = row.learning;
    if (!isObject(learning)) throw new ArgumentError("offline scenario learning projection is malformed");
    if (learning.outcome_digest !== null) assertDigest("offline scenario outcome_digest", learning.outcome_digest);
    if (learning.contract_digest !== null) assertDigest("offline scenario contract_digest", learning.contract_digest);
    const evaluation = row.evaluation;
    if (evaluation !== null) {
      if (!isObject(evaluation)) throw new ArgumentError("offline scenario evaluation projection is malformed");
      assertDigest("offline scenario evaluation_digest", evaluation.evaluation_digest);
      if (evaluation.evidence_digest !== null) assertDigest("offline scenario evidence_digest", evaluation.evidence_digest);
      if (evaluation.feedback_digest !== null) assertDigest("offline scenario feedback_digest", evaluation.feedback_digest);
    }
  }
  const encoded = JSON.stringify(value);
  if (!encoded || new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES) throw new ArgumentError("offline scenario report exceeds its bounded size");
  return structuredClone(value as AutonomousOfflineScenarioReport);
}

/**
 * Provider-neutral closed-loop harness for local evaluation and replay.
 *
 * It deliberately requires caller-owned value evidence. Transport success is never converted
 * into reward, and the returned report excludes task text, prompts, responses, credentials, and
 * evidence payloads. A credentialless in-memory provider is sufficient for the full matrix.
 */
export class AutonomousOfflineScenarioHarness {
  readonly agent: AutonomousAgent;
  readonly evaluatorRegistry: AutonomousValueEvaluatorRegistry;

  constructor(agent: AutonomousAgent, options: { evaluatorRegistry?: AutonomousValueEvaluatorRegistry } = {}) {
    if (!(agent instanceof AutonomousAgent)) throw new ArgumentError("offline scenario harness requires an AutonomousAgent");
    if (options.evaluatorRegistry !== undefined && !(options.evaluatorRegistry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("offline scenario evaluatorRegistry is malformed");
    this.agent = agent;
    this.evaluatorRegistry = options.evaluatorRegistry ?? AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
    if (!this.agent.learner) throw new ArgumentError("offline scenario harness requires an AutonomousOnlineLearner on the agent");
  }

  async run(options: AutonomousOfflineScenarioRunOptions): Promise<AutonomousOfflineScenarioReport> {
    if (!options || !Array.isArray(options.cases) || options.cases.length < 1 || options.cases.length > MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES) throw new ArgumentError("offline scenario cases must contain 1..12 entries");
    if (new Set(options.cases.map((item) => item.domain)).size !== options.cases.length) throw new ArgumentError("offline scenario cases must contain at most one case per domain");
    if (options.evidenceFor !== undefined && typeof options.evidenceFor !== "function") throw new ArgumentError("offline scenario evidenceFor must be callable");
    const cases = options.cases.map(cloneCase);
    const evaluatorRegistry = options.evaluatorRegistry ?? this.evaluatorRegistry;
    if (!(evaluatorRegistry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("offline scenario evaluator registry is malformed");
    if (options.requireCalibratedLearning !== undefined && typeof options.requireCalibratedLearning !== "boolean") throw new ArgumentError("offline scenario requireCalibratedLearning must be boolean");
    const calibrationReport = options.calibrationReport === undefined ? null : validateAutonomousEvaluatorCalibrationReport(options.calibrationReport);
    if (options.requireCalibratedLearning === true) {
      if (calibrationReport === null) throw new ArgumentError("offline scenario calibrated learning requires calibrationReport");
      for (const scenarioCase of cases) assertAutonomousEvaluatorCalibrationReady(calibrationReport, scenarioCase.domain);
    }
    const rows: AutonomousOfflineScenarioCaseReport[] = [];
    for (const scenarioCase of cases) {
      const preview = await this.agent.modelSelectionPreview(scenarioCase.task, {
        domain: scenarioCase.domain,
        ...(scenarioCase.capability === undefined ? {} : { capability: scenarioCase.capability }),
        ...(scenarioCase.context === undefined ? {} : { context: scenarioCase.context }),
        ...(scenarioCase.candidates === undefined ? {} : { candidates: scenarioCase.candidates }),
      });
      const selectionDigest = digestJsonSync(preview.selection_audit);
      const selectionContractDigest = digestJsonSync(preview.selection_contract);
      if (preview.status !== "selected") {
        rows.push({
          id: scenarioCase.id!,
          domain: scenarioCase.domain,
          status: "selection_refused",
          task_digest: preview.task_digest,
          selected_model: null,
          selection_digest: selectionDigest,
          selection_contract_digest: selectionContractDigest,
          execution: { status: null, provider_request_id: null },
          evaluation: null,
          learning: { arm_id: null, outcome_digest: null, contract_digest: null, generation: this.agent.learner!.snapshot().generation ?? null },
          retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained",
          secret_material: "never_returned",
        });
        continue;
      }
      const result = await this.agent.runApprovedModelSelection(scenarioCase.task, preview, {
        domain: scenarioCase.domain,
        ...(scenarioCase.capability === undefined ? {} : { capability: scenarioCase.capability }),
        ...(scenarioCase.context === undefined ? {} : { context: scenarioCase.context }),
        ...(scenarioCase.candidates === undefined ? {} : { candidates: scenarioCase.candidates }),
      });
      const selected = preview.selection_audit.selected_model;
      if (!selected) throw new ProviderRuntimeError("offline scenario selected model disappeared before invocation");
      const execution = executionMetadata(result);
      const evaluationInput = options.evidenceFor
        ? await options.evidenceFor({ case: scenarioCase, preview, execution })
        : scenarioCase.evidence === undefined
          ? null
          : { evidence: scenarioCase.evidence };
      if (!evaluationInput) throw new ArgumentError(`offline scenario ${scenarioCase.domain} requires caller-owned evaluation evidence`);
      const evaluation = evaluatorRegistry.resolveForAutonomousDomain(scenarioCase.domain).assess(evaluationInput);
      const outcomeDigest = digestJsonSync({ task_digest: preview.task_digest, domain: scenarioCase.domain, selected_model: selected, execution_status: result.status, evaluation_digest: evaluation.evaluation_digest });
      const contractDigest = digestJsonSync(preview.selection_contract);
      const learningState = this.agent.learner!.update({ arm_id: `${selected.provider}/${selected.model}`, reward: evaluation.reward, failed: evaluation.failed, outcome_digest: outcomeDigest, contract_digest: contractDigest });
      rows.push({
        id: scenarioCase.id!,
        domain: scenarioCase.domain,
        status: "completed",
        task_digest: preview.task_digest,
        selected_model: { provider: selected.provider, model: selected.model },
        selection_digest: selectionDigest,
        selection_contract_digest: selectionContractDigest,
        execution: { status: result.status, provider_request_id: execution.provider_request_id },
        evaluation: safeEvaluation(evaluation),
        learning: { arm_id: `${selected.provider}/${selected.model}`, outcome_digest: outcomeDigest, contract_digest: contractDigest, generation: learningState.generation ?? null },
        retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained",
        secret_material: "never_returned",
      });
    }
    const learnerState = this.agent.learner!.snapshot();
    const descriptor: Omit<AutonomousOfflineScenarioReport, "report_digest"> = {
      schema: AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA,
      status: rows.every((row) => row.status === "completed") ? "completed" : "partial",
      case_count: rows.length,
      completed_count: rows.filter((row) => row.status === "completed").length,
      refused_count: rows.filter((row) => row.status === "selection_refused").length,
      domains: rows.map((row) => row.domain),
      cases: rows,
      evaluator_catalogue_digest: digestJsonSync(evaluatorRegistry.catalogue()),
      learning_state_digest: digestJsonSync(learnerState),
      learning_generation: learnerState.generation ?? null,
      execution: "offline_provider_invocation_allowed;external_network_not_required",
      retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained",
      secret_material: "never_returned",
    };
    const report = { ...descriptor, report_digest: digestJsonSync(descriptor) };
    const encoded = JSON.stringify(report);
    if (!encoded || new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES) throw new ProviderRuntimeError("offline scenario report exceeds its bounded size");
    return structuredClone(report) as AutonomousOfflineScenarioReport;
  }

  async runAll(options: AutonomousOfflineScenarioAllDomainsOptions): Promise<AutonomousOfflineScenarioReport> {
    if (!options || typeof options.evidenceFor !== "function") throw new ArgumentError("offline scenario runAll requires evidenceFor");
    const cases = await Promise.all(AUTONOMOUS_DOMAIN_NAMES.map(async (domain) => {
      const task = options.taskForDomain
        ? await options.taskForDomain(domain)
        : options.tasks?.[domain] ?? `perform a bounded offline ${domain} evaluation`;
      return {
        id: domain,
        task,
        domain,
        ...(options.capabilityForDomain?.(domain) === undefined ? {} : { capability: options.capabilityForDomain(domain) }),
        ...(options.contextForDomain?.(domain) === undefined ? {} : { context: options.contextForDomain(domain) }),
        ...(options.candidatesForDomain?.(domain) === undefined ? {} : { candidates: options.candidatesForDomain(domain) }),
      } satisfies AutonomousOfflineScenarioCase;
    }));
    return this.run({ cases, evidenceFor: options.evidenceFor, evaluatorRegistry: options.evaluatorRegistry, calibrationReport: options.calibrationReport, requireCalibratedLearning: options.requireCalibratedLearning });
  }

  /** Verify and idempotently settle a metadata-only report without invoking a provider. */
  replay(report: AutonomousOfflineScenarioReport, options: { evaluatorRegistry?: AutonomousValueEvaluatorRegistry } = {}): AutonomousOfflineScenarioReplayResult {
    const verified = assertReport(report);
    const registry = options.evaluatorRegistry ?? this.evaluatorRegistry;
    if (!(registry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("offline scenario replay evaluator registry is malformed");
    const before = this.agent.learner!.snapshot().generation ?? null;
    let verifiedCount = 0;
    let replayedCount = 0;
    const replayRows: JsonObject[] = [];
    for (const row of verified.cases) {
      if (row.evaluation === null || row.selected_model === null || row.learning.outcome_digest === null || row.learning.contract_digest === null || row.learning.arm_id === null) continue;
      registry.resolveForReplay(row.domain, { evaluator_id: row.evaluation.evaluator_id, evaluator_version: row.evaluation.evaluator_version });
      assertDigest("offline scenario replay outcome_digest", row.learning.outcome_digest);
      assertDigest("offline scenario replay contract_digest", row.learning.contract_digest);
      const beforeRow = this.agent.learner!.snapshot().generation;
      const after = this.agent.learner!.update({ arm_id: row.learning.arm_id, reward: row.evaluation.reward, failed: row.evaluation.failed, outcome_digest: row.learning.outcome_digest, contract_digest: row.learning.contract_digest });
      verifiedCount += 1;
      if (after.generation !== beforeRow) replayedCount += 1;
      replayRows.push({ domain: row.domain, outcome_digest: row.learning.outcome_digest, evaluation_digest: row.evaluation.evaluation_digest, generation: after.generation });
    }
    const after = this.agent.learner!.snapshot().generation ?? null;
    const descriptor = { schema: AUTONOMOUS_OFFLINE_SCENARIO_REPLAY_SCHEMA, source_report_digest: verified.report_digest, case_count: verified.case_count, verified_count: verifiedCount, replayed_count: replayedCount, learner_generation_before: before, learner_generation_after: after, idempotent: after === before, execution: "metadata_only;no_provider_or_tool_invocation" as const, retention: "metadata_only;task_prompt_response_credentials_and_evidence_not_retained" as const, secret_material: "never_returned" as const };
    return { ...descriptor, replay_digest: digestJsonSync({ ...descriptor, rows: replayRows }) };
  }
}
