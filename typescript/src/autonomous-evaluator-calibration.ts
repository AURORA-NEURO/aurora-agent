import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousValueEvaluatorRegistry,
  type AutonomousValueEvaluation,
  type AutonomousValueEvaluationEvidence,
} from "./autonomous-domain-evaluators.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Deterministic evaluator reliability metrics over caller-labeled value-only cases. */
export const AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA = "bioprism-typescript-autonomous-evaluator-calibration/0.1" as const;
export const AUTONOMOUS_EVALUATOR_CALIBRATION_REPLAY_SCHEMA = "bioprism-typescript-autonomous-evaluator-calibration-replay/0.1" as const;
export const AUTONOMOUS_EVALUATOR_CALIBRATION_ADMISSION_SCHEMA = "bioprism-typescript-autonomous-evaluator-calibration-admission/0.1" as const;
export const MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES = 2_048;
export const MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_BINS = 20;
export const MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_DOMAINS = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES = 512_000;

export type AutonomousEvaluatorCalibrationSplit = "calibration" | "holdout";
export type AutonomousEvaluatorCalibrationDomainStatus = "ready" | "insufficient_calibration" | "insufficient_holdout" | "miscalibrated";
export type AutonomousEvaluatorCalibrationStatus = "ready" | "insufficient_coverage" | "insufficient_evidence" | "miscalibrated";

/** A transient caller-labeled case. The evidence and label never enter the report projection. */
export interface AutonomousEvaluatorCalibrationCase {
  case_id: string;
  domain: AutonomousDomainName;
  evidence: JsonObject;
  context?: JsonObject | null;
  /** Caller-owned reference outcome; null means the reference abstained. */
  label: 0 | 1 | null;
  /** Explicit split is preferred; omitted cases use a deterministic hash split. */
  split?: AutonomousEvaluatorCalibrationSplit;
  expected_evaluator_id?: string;
  expected_evaluator_version?: string;
}

export interface AutonomousEvaluatorCalibrationMetrics extends JsonObject {
  total_count: number;
  scored_count: number;
  unscored_count: number;
  coverage: number;
  abstention_rate: number;
  brier_score: number | null;
  expected_calibration_error: number | null;
  maximum_calibration_error: number | null;
  threshold_accuracy: number | null;
  predicted_positive_rate: number | null;
  observed_positive_rate: number | null;
  bins: AutonomousEvaluatorCalibrationBin[];
}

export interface AutonomousEvaluatorCalibrationBin extends JsonObject {
  lower_bound: number;
  upper_bound: number;
  count: number;
  predicted_mean: number | null;
  observed_rate: number | null;
  absolute_gap: number | null;
  population_fraction: number;
}

export interface AutonomousEvaluatorCalibrationDomainReport extends JsonObject {
  domain: AutonomousDomainName;
  evaluator_id: string;
  evaluator_version: string;
  pass_threshold: number;
  case_count: number;
  calibration_case_count: number;
  holdout_case_count: number;
  calibration: AutonomousEvaluatorCalibrationMetrics;
  holdout: AutonomousEvaluatorCalibrationMetrics;
  status: AutonomousEvaluatorCalibrationDomainStatus;
  case_set_digest: string;
  evaluation_digest: string;
}

export interface AutonomousEvaluatorCalibrationGate extends JsonObject {
  required_domains: AutonomousDomainName[];
  missing_domains: AutonomousDomainName[];
  min_calibration_cases_per_domain: number;
  min_holdout_cases_per_domain: number;
  max_expected_calibration_error: number;
  max_brier_score: number;
  require_all_domains: boolean;
  decision: "admit_learning" | "hold_learning";
  reasons: string[];
}

export interface AutonomousEvaluatorCalibrationReport extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA;
  status: AutonomousEvaluatorCalibrationStatus;
  target_domains: AutonomousDomainName[];
  evaluator_catalogue_digest: string;
  case_set_digest: string;
  seed: string;
  bins: number;
  holdout_fraction: number;
  split_policy: "explicit_split_or_seeded_sha256";
  calibration: AutonomousEvaluatorCalibrationMetrics;
  holdout: AutonomousEvaluatorCalibrationMetrics;
  domains: AutonomousEvaluatorCalibrationDomainReport[];
  gate: AutonomousEvaluatorCalibrationGate;
  report_digest: string;
  execution: "metadata_only;no_provider_or_learning_side_effects";
  retention: "metadata_only;case_evidence_labels_and_evaluator_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvaluatorCalibrationRunOptions {
  cases: readonly AutonomousEvaluatorCalibrationCase[];
  domains?: readonly AutonomousDomainName[];
  bins?: number;
  holdoutFraction?: number;
  seed?: string;
  minCalibrationCasesPerDomain?: number;
  minHoldoutCasesPerDomain?: number;
  maxExpectedCalibrationError?: number;
  maxBrierScore?: number;
  requireAllDomains?: boolean;
}

export interface AutonomousEvaluatorCalibrationReplayResult extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATOR_CALIBRATION_REPLAY_SCHEMA;
  source_report_digest: string;
  replay_report_digest: string;
  evaluator_catalogue_match: boolean;
  case_set_match: boolean;
  matches: boolean;
  execution: "metadata_only;no_provider_or_learning_side_effects";
  retention: "metadata_only;case_evidence_labels_and_evaluator_payloads_caller_owned";
  secret_material: "never_returned";
  replay_digest: string;
}

export interface AutonomousEvaluatorCalibrationAdmission extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATOR_CALIBRATION_ADMISSION_SCHEMA;
  domain: AutonomousDomainName;
  evaluator_id: string | null;
  evaluator_version: string | null;
  report_digest: string;
  decision: "admit_learning" | "hold_learning";
  reasons: string[];
  execution: "gate_only;does_not_assign_reward_or_invoke_provider";
  retention: "metadata_only;calibration_values_and_cases_caller_owned";
  secret_material: "never_returned";
  admission_digest: string;
}

interface ScoredObservation {
  score: number;
  label: 0 | 1;
  threshold: number;
}

interface NormalizedCase {
  case_id: string;
  domain: AutonomousDomainName;
  evidence: AutonomousValueEvaluationEvidence;
  context: JsonObject;
  label: 0 | 1 | null;
  split: AutonomousEvaluatorCalibrationSplit;
  case_digest: string;
}

const RETENTION = "metadata_only;case_evidence_labels_and_evaluator_payloads_caller_owned" as const;
const SECRET_MATERIAL = "never_returned" as const;

function boundedIdentifier(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || value.includes("\u0000") || !/^[A-Za-z0-9_.:+/-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function boundedDigest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function finiteUnit(name: string, value: unknown, fallback?: number): number {
  const resolved = value === undefined ? fallback : value;
  if (typeof resolved !== "number" || !Number.isFinite(resolved) || resolved < 0 || resolved > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return resolved;
}

function positiveInteger(name: string, value: unknown, minimum: number, maximum: number, fallback?: number): number {
  const resolved = value === undefined ? fallback : value;
  if (!Number.isSafeInteger(resolved) || (resolved as number) < minimum || (resolved as number) > maximum) throw new ArgumentError(`${name} must be an integer within ${minimum}..${maximum}`);
  return resolved as number;
}

function assertSecretFree(value: unknown, depth = 0): void {
  if (depth > 12) throw new ArgumentError("evaluator calibration input exceeds its nesting bound");
  if (Array.isArray(value)) {
    for (const item of value) assertSecretFree(item, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[^a-z]/g, "");
    if (["apikey", "authorization", "bearer", "credential", "password", "privatekey", "refreshtoken", "secret", "token"].includes(normalized)) throw new ArgumentError("evaluator calibration input contains forbidden secret-shaped fields");
    assertSecretFree(child, depth + 1);
  }
}

function splitForCase(caseId: string, domain: AutonomousDomainName, seed: string, holdoutFraction: number): AutonomousEvaluatorCalibrationSplit {
  const digest = digestJsonSync({ schema: AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA, case_id: caseId, domain, seed });
  const bucket = Number.parseInt(digest.slice(0, 8), 16) / 0xffffffff;
  return bucket < holdoutFraction ? "holdout" : "calibration";
}

function normalizeDomains(value: readonly AutonomousDomainName[] | undefined): AutonomousDomainName[] {
  const domains = value === undefined ? [...AUTONOMOUS_DOMAIN_NAMES] : [...value];
  if (!Array.isArray(domains) || domains.length < 1 || domains.length > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_DOMAINS || new Set(domains).size !== domains.length || domains.some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain))) throw new ArgumentError("evaluator calibration domains are outside their unique built-in bound");
  return domains;
}

function normalizeCase(value: AutonomousEvaluatorCalibrationCase, registry: AutonomousValueEvaluatorRegistry, seed: string, holdoutFraction: number): NormalizedCase {
  if (!isObject(value)) throw new ArgumentError("evaluator calibration case must be an object");
  const caseId = boundedIdentifier("evaluator calibration case_id", value.case_id);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value.domain)) throw new ArgumentError("evaluator calibration case domain is unsupported");
  if (!isObject(value.evidence)) throw new ArgumentError("evaluator calibration evidence must be an object");
  assertSecretFree(value.evidence);
  if (value.context !== undefined && value.context !== null && !isObject(value.context)) throw new ArgumentError("evaluator calibration context must be an object or null");
  assertSecretFree(value.context);
  if (value.label !== 0 && value.label !== 1 && value.label !== null) throw new ArgumentError("evaluator calibration label must be 0, 1, or null");
  const adapter = registry.resolveForAutonomousDomain(value.domain);
  const evidence = adapter.normalizeEvidence(value.evidence);
  const context = value.context === null || value.context === undefined ? { domain: value.domain } : structuredClone(value.context);
  const split = value.split ?? splitForCase(caseId, value.domain, seed, holdoutFraction);
  if (split !== "calibration" && split !== "holdout") throw new ArgumentError("evaluator calibration split is invalid");
  if (value.expected_evaluator_id !== undefined && boundedIdentifier("evaluator calibration expected evaluator_id", value.expected_evaluator_id) !== adapter.evaluatorId) throw new ArgumentError(`evaluator identity mismatch for ${value.domain}`);
  if (value.expected_evaluator_version !== undefined && boundedIdentifier("evaluator calibration expected evaluator_version", value.expected_evaluator_version) !== adapter.evaluatorVersion) throw new ArgumentError(`evaluator version mismatch for ${value.domain}`);
  const evidenceDigest = digestJsonSync(evidence);
  const caseDigest = digestJsonSync({ case_id: caseId, domain: value.domain, evidence_digest: evidenceDigest, context_digest: digestJsonSync(context), label: value.label, split });
  return { case_id: caseId, domain: value.domain, evidence, context, label: value.label, split, case_digest: caseDigest };
}

function metricFor(observations: readonly ScoredObservation[], totalCount: number, bins: number, passThreshold: number): AutonomousEvaluatorCalibrationMetrics {
  const scoredCount = observations.length;
  const unscoredCount = Math.max(0, totalCount - scoredCount);
  const buckets = Array.from({ length: bins }, (_, index) => ({ lower_bound: Number((index / bins).toFixed(12)), upper_bound: Number(((index + 1) / bins).toFixed(12)), rows: [] as ScoredObservation[] }));
  for (const observation of observations) buckets[Math.min(bins - 1, Math.floor(observation.score * bins))]!.rows.push(observation);
  const binRows: AutonomousEvaluatorCalibrationBin[] = buckets.map((bucket) => {
    const count = bucket.rows.length;
    const predictedMean = count ? bucket.rows.reduce((sum, row) => sum + row.score, 0) / count : null;
    const observedRate = count ? bucket.rows.reduce((sum, row) => sum + row.label, 0) / count : null;
    return { lower_bound: bucket.lower_bound, upper_bound: bucket.upper_bound, count, predicted_mean: predictedMean === null ? null : Number(predictedMean.toFixed(12)), observed_rate: observedRate === null ? null : Number(observedRate.toFixed(12)), absolute_gap: predictedMean === null || observedRate === null ? null : Number(Math.abs(predictedMean - observedRate).toFixed(12)), population_fraction: scoredCount ? Number((count / scoredCount).toFixed(12)) : 0 };
  });
  const brier = scoredCount ? observations.reduce((sum, row) => sum + ((row.score - row.label) ** 2), 0) / scoredCount : null;
  const ece = scoredCount ? binRows.reduce((sum, row) => sum + (row.population_fraction * (row.absolute_gap ?? 0)), 0) : null;
  const mce = scoredCount ? Math.max(...binRows.map((row) => row.absolute_gap ?? 0)) : null;
  const thresholdAccuracy = scoredCount ? observations.filter((row) => (row.score >= row.threshold ? 1 : 0) === row.label).length / scoredCount : null;
  const predictedPositiveRate = scoredCount ? observations.filter((row) => row.score >= row.threshold).length / scoredCount : null;
  const observedPositiveRate = scoredCount ? observations.reduce((sum, row) => sum + row.label, 0) / scoredCount : null;
  return {
    total_count: totalCount,
    scored_count: scoredCount,
    unscored_count: unscoredCount,
    coverage: totalCount ? Number((scoredCount / totalCount).toFixed(12)) : 0,
    abstention_rate: totalCount ? Number((unscoredCount / totalCount).toFixed(12)) : 0,
    brier_score: brier === null ? null : Number(brier.toFixed(12)),
    expected_calibration_error: ece === null ? null : Number(ece.toFixed(12)),
    maximum_calibration_error: mce === null ? null : Number(mce.toFixed(12)),
    threshold_accuracy: thresholdAccuracy === null ? null : Number(thresholdAccuracy.toFixed(12)),
    predicted_positive_rate: predictedPositiveRate === null ? null : Number(predictedPositiveRate.toFixed(12)),
    observed_positive_rate: observedPositiveRate === null ? null : Number(observedPositiveRate.toFixed(12)),
    bins: binRows,
  };
}

function aggregateMetrics(rows: readonly { split: AutonomousEvaluatorCalibrationSplit; observation: ScoredObservation | null; total: boolean }[], bins: number, passThreshold: number): AutonomousEvaluatorCalibrationMetrics {
  const observations = rows.flatMap((row) => row.observation === null ? [] : [row.observation]);
  return metricFor(observations, rows.length, bins, passThreshold);
}

function caseSetDigest(cases: readonly NormalizedCase[]): string {
  return digestJsonSync(cases.map((item) => ({ case_id: item.case_id, domain: item.domain, case_digest: item.case_digest, label: item.label, split: item.split })).sort((left, right) => `${left.domain}:${left.case_id}`.localeCompare(`${right.domain}:${right.case_id}`)));
}

function reportWithoutDigest(report: AutonomousEvaluatorCalibrationReport): Omit<AutonomousEvaluatorCalibrationReport, "report_digest"> {
  const { report_digest: _reportDigest, ...descriptor } = report;
  return descriptor;
}

function assertReportDigest(report: AutonomousEvaluatorCalibrationReport): void {
  if (!isObject(report) || report.schema !== AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA) throw new ArgumentError("evaluator calibration report schema is invalid");
  if (typeof report.report_digest !== "string" || !/^[0-9a-f]{64}$/.test(report.report_digest)) throw new ArgumentError("evaluator calibration report digest is invalid");
  if (digestJsonSync(reportWithoutDigest(report)) !== report.report_digest) throw new ArgumentError("evaluator calibration report digest does not match its metadata");
  const encoded = JSON.stringify(report);
  if (!encoded || new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES) throw new ArgumentError("evaluator calibration report exceeds its bound");
}

function statusForDomain(calibration: AutonomousEvaluatorCalibrationMetrics, holdout: AutonomousEvaluatorCalibrationMetrics, minCalibration: number, minHoldout: number, maxEce: number, maxBrier: number): AutonomousEvaluatorCalibrationDomainStatus {
  if (calibration.scored_count < minCalibration) return "insufficient_calibration";
  if (holdout.scored_count < minHoldout) return "insufficient_holdout";
  if (holdout.expected_calibration_error === null || holdout.brier_score === null || holdout.expected_calibration_error > maxEce || holdout.brier_score > maxBrier) return "miscalibrated";
  return "ready";
}

function overallStatus(domains: readonly AutonomousEvaluatorCalibrationDomainReport[], requiredDomains: readonly AutonomousDomainName[], missingDomains: readonly AutonomousDomainName[], requireAllDomains: boolean): AutonomousEvaluatorCalibrationStatus {
  if (requireAllDomains && missingDomains.length) return "insufficient_coverage";
  if (domains.some((row) => row.status === "insufficient_calibration" || row.status === "insufficient_holdout")) return "insufficient_evidence";
  if (domains.some((row) => row.status === "miscalibrated")) return "miscalibrated";
  return requiredDomains.length && domains.length ? "ready" : "insufficient_coverage";
}

/** Provider-free, caller-labeled evaluator calibration and holdout harness. */
export class AutonomousEvaluatorCalibrationHarness {
  constructor(readonly registry: AutonomousValueEvaluatorRegistry) {
    if (!(registry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("evaluator calibration harness requires an AutonomousValueEvaluatorRegistry");
  }

  run(options: AutonomousEvaluatorCalibrationRunOptions): AutonomousEvaluatorCalibrationReport {
    if (!options || !Array.isArray(options.cases) || options.cases.length < 1 || options.cases.length > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES) throw new ArgumentError("evaluator calibration cases must contain 1..2048 entries");
    const domains = normalizeDomains(options.domains);
    const bins = positiveInteger("evaluator calibration bins", options.bins, 2, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_BINS, 10);
    const holdoutFraction = finiteUnit("evaluator calibration holdoutFraction", options.holdoutFraction, 0.2);
    if (holdoutFraction <= 0 || holdoutFraction >= 1) throw new ArgumentError("evaluator calibration holdoutFraction must be within (0, 1)");
    const seed = boundedText("evaluator calibration seed", options.seed ?? "default", 256);
    const minCalibration = positiveInteger("evaluator calibration minCalibrationCasesPerDomain", options.minCalibrationCasesPerDomain, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES, 2);
    const minHoldout = positiveInteger("evaluator calibration minHoldoutCasesPerDomain", options.minHoldoutCasesPerDomain, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES, 2);
    const maxEce = finiteUnit("evaluator calibration maxExpectedCalibrationError", options.maxExpectedCalibrationError, 0.1);
    const maxBrier = finiteUnit("evaluator calibration maxBrierScore", options.maxBrierScore, 0.2);
    const requireAllDomains = options.requireAllDomains ?? true;
    if (typeof requireAllDomains !== "boolean") throw new ArgumentError("evaluator calibration requireAllDomains must be boolean");
    const normalized = options.cases.map((value) => normalizeCase(value, this.registry, seed, holdoutFraction));
    if (new Set(normalized.map((item) => item.case_id)).size !== normalized.length) throw new ArgumentError("evaluator calibration case_id values must be unique");
    const targetSet = new Set(domains);
    const selected = normalized.filter((item) => targetSet.has(item.domain));
    const missingDomains = domains.filter((domain) => !selected.some((item) => item.domain === domain));
    const rows: AutonomousEvaluatorCalibrationDomainReport[] = [];
    const allCalibrationRows: { split: AutonomousEvaluatorCalibrationSplit; observation: ScoredObservation | null; total: boolean }[] = [];
    const allHoldoutRows: { split: AutonomousEvaluatorCalibrationSplit; observation: ScoredObservation | null; total: boolean }[] = [];
    for (const domain of domains) {
      const adapter = this.registry.resolveForAutonomousDomain(domain);
      const domainCases = selected.filter((item) => item.domain === domain);
      const calibrationObservations: ScoredObservation[] = [];
      const holdoutObservations: ScoredObservation[] = [];
      const allDomainRows: { split: AutonomousEvaluatorCalibrationSplit; observation: ScoredObservation | null; total: boolean }[] = [];
      for (const item of domainCases) {
        let evaluation: AutonomousValueEvaluation;
        try {
          evaluation = adapter.assess({ evidence: item.evidence, context: item.context });
        } catch {
          evaluation = {
            schema: "bioprism-brain-domain-evaluator/0.1",
            evaluator_id: adapter.evaluatorId,
            evaluator_version: adapter.evaluatorVersion,
            domain,
            reward: 0,
            passed: false,
            failed: true,
            failure_class: "evaluator_refused",
            feedback_digest: null,
            evidence_digest: null,
            replan_requested: true,
            replan_instruction: "caller evaluator refused the calibration case",
            missing_signals: [],
            below_threshold_signals: [],
            evaluator_authority: "caller_declared_signal_scoring_only",
            retention: "value_only;task_prompt_response_credentials_and_evidence_not_retained",
            secret_material: "never_returned",
            evaluation_digest: digestJsonSync({ domain, case_digest: item.case_digest, status: "evaluator_refused" }),
          } as AutonomousValueEvaluation;
        }
        const scored = item.label === null || evaluation.evidence_digest === null ? null : { score: finiteUnit("evaluator calibration reward", evaluation.reward), label: item.label, threshold: adapter.profile.pass_threshold } satisfies ScoredObservation;
        const row = { split: item.split, observation: scored, total: true };
        allDomainRows.push(row);
        if (item.split === "calibration") calibrationObservations.push(...(scored === null ? [] : [scored]));
        else holdoutObservations.push(...(scored === null ? [] : [scored]));
      }
      allCalibrationRows.push(...allDomainRows.filter((row) => row.split === "calibration"));
      allHoldoutRows.push(...allDomainRows.filter((row) => row.split === "holdout"));
      const calibration = metricFor(calibrationObservations, domainCases.filter((item) => item.split === "calibration").length, bins, adapter.profile.pass_threshold);
      const holdout = metricFor(holdoutObservations, domainCases.filter((item) => item.split === "holdout").length, bins, adapter.profile.pass_threshold);
      const status = statusForDomain(calibration, holdout, minCalibration, minHoldout, maxEce, maxBrier);
      const descriptor = { domain, evaluator_id: adapter.evaluatorId, evaluator_version: adapter.evaluatorVersion, pass_threshold: adapter.profile.pass_threshold, case_count: domainCases.length, calibration_case_count: domainCases.filter((item) => item.split === "calibration").length, holdout_case_count: domainCases.filter((item) => item.split === "holdout").length, calibration, holdout, status, case_set_digest: digestJsonSync(domainCases.map((item) => item.case_digest).sort()) };
      rows.push({ ...descriptor, evaluation_digest: digestJsonSync(descriptor) });
    }
    const calibration = aggregateMetrics(allCalibrationRows, bins, 0.5);
    const holdout = aggregateMetrics(allHoldoutRows, bins, 0.5);
    const evaluatorCatalogueDigest = digestJsonSync(this.registry.catalogue());
    const selectedNormalized = normalized.filter((item) => targetSet.has(item.domain));
    const status = overallStatus(rows, domains, missingDomains, requireAllDomains);
    const reasons = [
      ...(missingDomains.length ? [`missing evaluator calibration domains: ${missingDomains.join(", ")}`] : []),
      ...rows.filter((row) => row.status !== "ready").map((row) => `${row.domain}:${row.status}`),
    ];
    const gate: AutonomousEvaluatorCalibrationGate = {
      required_domains: [...domains],
      missing_domains: [...missingDomains],
      min_calibration_cases_per_domain: minCalibration,
      min_holdout_cases_per_domain: minHoldout,
      max_expected_calibration_error: maxEce,
      max_brier_score: maxBrier,
      require_all_domains: requireAllDomains,
      decision: status === "ready" ? "admit_learning" : "hold_learning",
      reasons,
    };
    const descriptor: Omit<AutonomousEvaluatorCalibrationReport, "report_digest"> = {
      schema: AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA,
      status,
      target_domains: [...domains],
      evaluator_catalogue_digest: evaluatorCatalogueDigest,
      case_set_digest: caseSetDigest(selectedNormalized),
      seed,
      bins,
      holdout_fraction: holdoutFraction,
      split_policy: "explicit_split_or_seeded_sha256",
      calibration,
      holdout,
      domains: rows,
      gate,
      execution: "metadata_only;no_provider_or_learning_side_effects",
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
    const report = { ...descriptor, report_digest: digestJsonSync(descriptor) };
    const encoded = JSON.stringify(report);
    if (new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES) throw new ProviderRuntimeError("evaluator calibration report exceeds its bounded size");
    return structuredClone(report) as AutonomousEvaluatorCalibrationReport;
  }

  replay(report: AutonomousEvaluatorCalibrationReport, options: { cases: readonly AutonomousEvaluatorCalibrationCase[] }): AutonomousEvaluatorCalibrationReplayResult {
    assertReportDigest(report);
    const replayed = this.run({ cases: options.cases, domains: report.target_domains, seed: report.seed, bins: report.bins, holdoutFraction: report.holdout_fraction, minCalibrationCasesPerDomain: report.gate.min_calibration_cases_per_domain, minHoldoutCasesPerDomain: report.gate.min_holdout_cases_per_domain, maxExpectedCalibrationError: report.gate.max_expected_calibration_error, maxBrierScore: report.gate.max_brier_score, requireAllDomains: report.gate.require_all_domains });
    const evaluatorCatalogueMatch = report.evaluator_catalogue_digest === replayed.evaluator_catalogue_digest;
    const caseSetMatch = report.case_set_digest === replayed.case_set_digest;
    const matches = report.report_digest === replayed.report_digest;
    const descriptor = { schema: AUTONOMOUS_EVALUATOR_CALIBRATION_REPLAY_SCHEMA, source_report_digest: report.report_digest, replay_report_digest: replayed.report_digest, evaluator_catalogue_match: evaluatorCatalogueMatch, case_set_match: caseSetMatch, matches, execution: "metadata_only;no_provider_or_learning_side_effects" as const, retention: RETENTION, secret_material: SECRET_MATERIAL } satisfies Omit<AutonomousEvaluatorCalibrationReplayResult, "replay_digest">;
    return { ...descriptor, replay_digest: digestJsonSync(descriptor) };
  }
}

/** Return an explicit learning-admission decision for one calibrated domain. */
export function autonomousEvaluatorCalibrationAdmission(report: AutonomousEvaluatorCalibrationReport, domain: AutonomousDomainName): AutonomousEvaluatorCalibrationAdmission {
  assertReportDigest(report);
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("evaluator calibration admission domain is unsupported");
  const row = report.domains.find((candidate) => candidate.domain === domain);
  const reasons = row && row.status === "ready" && report.gate.decision === "admit_learning" ? [] : [
    ...(report.status !== "ready" ? [`calibration_report:${report.status}`] : []),
    ...(row ? (row.status === "ready" ? [] : [`domain:${row.status}`]) : ["domain_missing"]),
  ];
  const descriptor = { schema: AUTONOMOUS_EVALUATOR_CALIBRATION_ADMISSION_SCHEMA, domain, evaluator_id: row?.evaluator_id ?? null, evaluator_version: row?.evaluator_version ?? null, report_digest: report.report_digest, decision: reasons.length ? "hold_learning" as const : "admit_learning" as const, reasons, execution: "gate_only;does_not_assign_reward_or_invoke_provider" as const, retention: "metadata_only;calibration_values_and_cases_caller_owned" as const, secret_material: SECRET_MATERIAL } satisfies Omit<AutonomousEvaluatorCalibrationAdmission, "admission_digest">;
  return { ...descriptor, admission_digest: digestJsonSync(descriptor) };
}

/** Throw before reward settlement when calibration has not admitted a domain. */
export function assertAutonomousEvaluatorCalibrationReady(report: AutonomousEvaluatorCalibrationReport, domain: AutonomousDomainName): AutonomousEvaluatorCalibrationAdmission {
  const admission = autonomousEvaluatorCalibrationAdmission(report, domain);
  if (admission.decision !== "admit_learning") throw new ProviderRuntimeError(`evaluator calibration holds learning for ${domain}: ${admission.reasons.join(", ")}`);
  return admission;
}

export function validateAutonomousEvaluatorCalibrationReport(report: AutonomousEvaluatorCalibrationReport): AutonomousEvaluatorCalibrationReport {
  assertReportDigest(report);
  if (!Array.isArray(report.target_domains) || !report.target_domains.length || report.target_domains.some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain))) throw new ArgumentError("evaluator calibration report target domains are invalid");
  if (!Array.isArray(report.domains) || report.domains.some((row) => !report.target_domains.includes(row.domain))) throw new ArgumentError("evaluator calibration report domain rows are invalid");
  if (!["ready", "insufficient_coverage", "insufficient_evidence", "miscalibrated"].includes(report.status)) throw new ArgumentError("evaluator calibration report status is invalid");
  return structuredClone(report);
}
