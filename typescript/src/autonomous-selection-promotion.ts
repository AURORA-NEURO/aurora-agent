import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  validateAutonomousSelectionLabReport,
  type AutonomousSelectionLabDomainReport,
  type AutonomousSelectionLabReport,
} from "./autonomous-selection-lab.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Stable schema for the policy used to admit a replayed selection learner. */
export const AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA = "bioprism-typescript-autonomous-selection-promotion-policy/0.1" as const;
/** Stable schema for one domain's selection-promotion projection. */
export const AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-selection-promotion-domain/0.1" as const;
/** Stable schema for a cross-domain selection-promotion decision. */
export const AUTONOMOUS_SELECTION_PROMOTION_SCHEMA = "bioprism-typescript-autonomous-selection-promotion/0.1" as const;

export const MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS = 64;
export const MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES = 512_000;

const EXECUTION = "gate_only;does_not_mutate_learner_or_invoke_provider" as const;
const RETENTION = "metadata_only;selection_metrics_and_digests" as const;
const SECRET_MATERIAL = "never_returned" as const;

export type AutonomousSelectionPromotionDecision = "admit" | "hold";
export type AutonomousSelectionPromotionDomainDecision = "admit" | "hold" | "not_required";

/** Caller-tunable, bounded thresholds for moving replay evidence into active selection. */
export interface AutonomousSelectionPromotionPolicy {
  requireAllDomains?: boolean;
  minCasesPerDomain?: number;
  minEvaluatedCasesPerDomain?: number;
  minEvaluatedCoverage?: number;
  minOracleAgreementRate?: number;
  maxMeanRegret?: number;
  maxAbstentionRate?: number;
  maxSelectedRewardMissingRate?: number;
  maxNoEligibleModelRate?: number;
  maxNoCounterfactualRewardRate?: number;
}

export interface AutonomousSelectionPromotionPolicyProjection extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA;
  require_all_domains: boolean;
  min_cases_per_domain: number;
  min_evaluated_cases_per_domain: number;
  min_evaluated_coverage: number;
  min_oracle_agreement_rate: number;
  max_mean_regret: number;
  max_abstention_rate: number;
  max_selected_reward_missing_rate: number;
  max_no_eligible_model_rate: number;
  max_no_counterfactual_reward_rate: number;
}

export interface AutonomousSelectionPromotionDomainReport extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA;
  domain: AutonomousDomainName;
  case_count: number;
  evaluated_count: number;
  evaluated_coverage: number;
  oracle_agreement_count: number;
  oracle_agreement_rate: number | null;
  mean_regret: number | null;
  abstention_rate: number;
  selected_reward_missing_rate: number;
  no_eligible_model_rate: number;
  no_counterfactual_reward_rate: number;
  decision: AutonomousSelectionPromotionDomainDecision;
  reasons: string[];
}

export interface AutonomousSelectionPromotionReport extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_PROMOTION_SCHEMA;
  source_report_digest: string;
  policy: AutonomousSelectionPromotionPolicyProjection;
  decision: AutonomousSelectionPromotionDecision;
  reasons: string[];
  domains: AutonomousSelectionPromotionDomainReport[];
  execution: typeof EXECUTION;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  promotion_digest: string;
}

type DomainName = (typeof AUTONOMOUS_DOMAIN_NAMES)[number];

function fail(message: string): never {
  throw new ArgumentError(`autonomous selection promotion ${message}`);
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its integer bounds`);
  return value as number;
}

function boundedNumber(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function boundedRate(name: string, value: unknown): number {
  return boundedNumber(name, value, 0, 1);
}

function boundedReasonList(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS) fail(`${name} must contain at most ${MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS} reasons`);
  return value.map((reason, index) => {
    if (typeof reason !== "string" || reason.trim().length === 0 || reason.length > 512 || /\u0000/.test(reason)) fail(`${name}[${index}] is invalid`);
    return reason;
  });
}

function domainName(value: unknown): DomainName {
  if (typeof value !== "string" || !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(value)) fail("domain is not a supported autonomous domain");
  return value as DomainName;
}

function validateOptions(options: AutonomousSelectionPromotionPolicy): AutonomousSelectionPromotionPolicyProjection {
  if (!isObject(options)) fail("options must be an object");
  const requireAllDomains = options.requireAllDomains ?? true;
  if (typeof requireAllDomains !== "boolean") fail("requireAllDomains must be boolean");
  const minCasesPerDomain = options.minCasesPerDomain ?? 1;
  const minEvaluatedCasesPerDomain = options.minEvaluatedCasesPerDomain ?? 1;
  const minEvaluatedCoverage = options.minEvaluatedCoverage ?? 0.5;
  const minOracleAgreementRate = options.minOracleAgreementRate ?? 0.5;
  const maxMeanRegret = options.maxMeanRegret ?? 0.25;
  const maxAbstentionRate = options.maxAbstentionRate ?? 0.25;
  const maxSelectedRewardMissingRate = options.maxSelectedRewardMissingRate ?? 0;
  const maxNoEligibleModelRate = options.maxNoEligibleModelRate ?? 0;
  const maxNoCounterfactualRewardRate = options.maxNoCounterfactualRewardRate ?? 0;
  const normalizedMinCasesPerDomain = boundedInteger("minCasesPerDomain", minCasesPerDomain, 1, 4_096);
  const normalizedMinEvaluatedCasesPerDomain = boundedInteger("minEvaluatedCasesPerDomain", minEvaluatedCasesPerDomain, 1, 4_096);
  const normalizedMinEvaluatedCoverage = boundedRate("minEvaluatedCoverage", minEvaluatedCoverage);
  const normalizedMinOracleAgreementRate = boundedRate("minOracleAgreementRate", minOracleAgreementRate);
  const normalizedMaxMeanRegret = boundedNumber("maxMeanRegret", maxMeanRegret, 0, 2);
  const normalizedMaxAbstentionRate = boundedRate("maxAbstentionRate", maxAbstentionRate);
  const normalizedMaxSelectedRewardMissingRate = boundedRate("maxSelectedRewardMissingRate", maxSelectedRewardMissingRate);
  const normalizedMaxNoEligibleModelRate = boundedRate("maxNoEligibleModelRate", maxNoEligibleModelRate);
  const normalizedMaxNoCounterfactualRewardRate = boundedRate("maxNoCounterfactualRewardRate", maxNoCounterfactualRewardRate);
  return {
    schema: AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA,
    require_all_domains: requireAllDomains,
    min_cases_per_domain: normalizedMinCasesPerDomain,
    min_evaluated_cases_per_domain: normalizedMinEvaluatedCasesPerDomain,
    min_evaluated_coverage: normalizedMinEvaluatedCoverage,
    min_oracle_agreement_rate: normalizedMinOracleAgreementRate,
    max_mean_regret: normalizedMaxMeanRegret,
    max_abstention_rate: normalizedMaxAbstentionRate,
    max_selected_reward_missing_rate: normalizedMaxSelectedRewardMissingRate,
    max_no_eligible_model_rate: normalizedMaxNoEligibleModelRate,
    max_no_counterfactual_reward_rate: normalizedMaxNoCounterfactualRewardRate,
  };
}

function ratio(numerator: number, denominator: number): number {
  return denominator === 0 ? 0 : Number((numerator / denominator).toFixed(12));
}

function optionalRatio(numerator: number, denominator: number): number | null {
  return denominator === 0 ? null : Number((numerator / denominator).toFixed(12));
}

function reason(reasons: string[], value: string): void {
  if (!reasons.includes(value)) reasons.push(value);
}

function evaluateDomain(row: AutonomousSelectionLabDomainReport, policy: AutonomousSelectionPromotionPolicyProjection): AutonomousSelectionPromotionDomainReport {
  const reasons: string[] = [];
  const hasCases = row.case_count > 0;
  const evaluatedCoverage = row.evaluated_coverage;
  const oracleAgreementRate = optionalRatio(row.oracle_agreement_count, row.evaluated_count);
  const abstentionRate = ratio(row.abstained_count, row.case_count);
  const selectedRewardMissingRate = ratio(row.selected_reward_missing_count, row.case_count);
  const noEligibleModelRate = ratio(row.no_eligible_model_count, row.case_count);
  const noCounterfactualRewardRate = ratio(row.no_counterfactual_reward_count, row.case_count);
  if (!hasCases) {
    if (policy.require_all_domains) reason(reasons, "domain has no replay cases");
  } else {
    if (row.case_count < policy.min_cases_per_domain) reason(reasons, `domain has fewer than ${policy.min_cases_per_domain} replay cases`);
    if (row.evaluated_count < policy.min_evaluated_cases_per_domain) reason(reasons, `domain has fewer than ${policy.min_evaluated_cases_per_domain} evaluated cases`);
    if (evaluatedCoverage < policy.min_evaluated_coverage) reason(reasons, `evaluated coverage is below ${policy.min_evaluated_coverage}`);
    if (oracleAgreementRate === null || oracleAgreementRate < policy.min_oracle_agreement_rate) reason(reasons, `oracle agreement is below ${policy.min_oracle_agreement_rate}`);
    if (row.mean_regret === null || row.mean_regret > policy.max_mean_regret) reason(reasons, `mean regret exceeds ${policy.max_mean_regret}`);
    if (abstentionRate > policy.max_abstention_rate) reason(reasons, `abstention rate exceeds ${policy.max_abstention_rate}`);
    if (selectedRewardMissingRate > policy.max_selected_reward_missing_rate) reason(reasons, `selected reward missing rate exceeds ${policy.max_selected_reward_missing_rate}`);
    if (noEligibleModelRate > policy.max_no_eligible_model_rate) reason(reasons, `no eligible model rate exceeds ${policy.max_no_eligible_model_rate}`);
    if (noCounterfactualRewardRate > policy.max_no_counterfactual_reward_rate) reason(reasons, `no counterfactual reward rate exceeds ${policy.max_no_counterfactual_reward_rate}`);
  }
  const decision: AutonomousSelectionPromotionDomainDecision = !hasCases && !policy.require_all_domains
    ? "not_required"
    : reasons.length === 0 ? "admit" : "hold";
  return {
    schema: AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA,
    domain: row.domain,
    case_count: row.case_count,
    evaluated_count: row.evaluated_count,
    evaluated_coverage: evaluatedCoverage,
    oracle_agreement_count: row.oracle_agreement_count,
    oracle_agreement_rate: oracleAgreementRate,
    mean_regret: row.mean_regret,
    abstention_rate: abstentionRate,
    selected_reward_missing_rate: selectedRewardMissingRate,
    no_eligible_model_rate: noEligibleModelRate,
    no_counterfactual_reward_rate: noCounterfactualRewardRate,
    decision,
    reasons,
  };
}

function reportBody(report: AutonomousSelectionPromotionReport): Omit<AutonomousSelectionPromotionReport, "promotion_digest"> {
  const { promotion_digest: _promotionDigest, ...body } = report;
  return body;
}

function validatePolicyProjection(value: unknown): AutonomousSelectionPromotionPolicyProjection {
  if (!isObject(value)) fail("report policy must be an object");
  if (value.schema !== AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA) fail("report policy schema is invalid");
  if (typeof value.require_all_domains !== "boolean") fail("report policy require_all_domains must be boolean");
  boundedInteger("report policy min_cases_per_domain", value.min_cases_per_domain, 1, 4_096);
  boundedInteger("report policy min_evaluated_cases_per_domain", value.min_evaluated_cases_per_domain, 1, 4_096);
  boundedRate("report policy min_evaluated_coverage", value.min_evaluated_coverage);
  boundedRate("report policy min_oracle_agreement_rate", value.min_oracle_agreement_rate);
  boundedNumber("report policy max_mean_regret", value.max_mean_regret, 0, 2);
  boundedRate("report policy max_abstention_rate", value.max_abstention_rate);
  boundedRate("report policy max_selected_reward_missing_rate", value.max_selected_reward_missing_rate);
  boundedRate("report policy max_no_eligible_model_rate", value.max_no_eligible_model_rate);
  boundedRate("report policy max_no_counterfactual_reward_rate", value.max_no_counterfactual_reward_rate);
  return value as AutonomousSelectionPromotionPolicyProjection;
}

function validateDomainReport(value: unknown, index: number): AutonomousSelectionPromotionDomainReport {
  if (!isObject(value)) fail(`report domain ${index} must be an object`);
  if (value.schema !== AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA) fail(`report domain ${index} schema is invalid`);
  const domain = domainName(value.domain);
  const caseCount = boundedInteger(`report domain ${domain}.case_count`, value.case_count, 0, 4_096);
  const evaluatedCount = boundedInteger(`report domain ${domain}.evaluated_count`, value.evaluated_count, 0, 4_096);
  const evaluatedCoverage = boundedRate(`report domain ${domain}.evaluated_coverage`, value.evaluated_coverage);
  const oracleAgreementCount = boundedInteger(`report domain ${domain}.oracle_agreement_count`, value.oracle_agreement_count, 0, 4_096);
  if (evaluatedCount > caseCount || oracleAgreementCount > evaluatedCount) fail(`report domain ${domain} counts are inconsistent`);
  if (evaluatedCoverage !== ratio(evaluatedCount, caseCount)) fail(`report domain ${domain}.evaluated_coverage is inconsistent`);
  if (value.oracle_agreement_rate !== null) boundedRate(`report domain ${domain}.oracle_agreement_rate`, value.oracle_agreement_rate);
  if (value.oracle_agreement_rate !== optionalRatio(oracleAgreementCount, evaluatedCount)) fail(`report domain ${domain}.oracle_agreement_rate is inconsistent`);
  if (value.mean_regret !== null) boundedNumber(`report domain ${domain}.mean_regret`, value.mean_regret, 0, 2);
  boundedRate(`report domain ${domain}.abstention_rate`, value.abstention_rate);
  boundedRate(`report domain ${domain}.selected_reward_missing_rate`, value.selected_reward_missing_rate);
  boundedRate(`report domain ${domain}.no_eligible_model_rate`, value.no_eligible_model_rate);
  boundedRate(`report domain ${domain}.no_counterfactual_reward_rate`, value.no_counterfactual_reward_rate);
  if (value.decision !== "admit" && value.decision !== "hold" && value.decision !== "not_required") fail(`report domain ${domain}.decision is invalid`);
  const reasons = boundedReasonList(`report domain ${domain}.reasons`, value.reasons);
  if (value.decision === "not_required" && (caseCount !== 0 || reasons.length !== 0)) fail(`report domain ${domain}.not_required decision is inconsistent`);
  if (value.decision === "admit" && reasons.length !== 0) fail(`report domain ${domain}.admit decision contains reasons`);
  if (value.decision === "hold" && reasons.length === 0) fail(`report domain ${domain}.hold decision has no reasons`);
  return value as AutonomousSelectionPromotionDomainReport;
}

/** Validate a promotion decision, including its canonical digest and projection bounds. */
export function validateAutonomousSelectionPromotionReport(value: unknown): AutonomousSelectionPromotionReport {
  if (!isObject(value)) fail("report must be an object");
  if (new TextEncoder().encode(canonicalJson(value)).byteLength > MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES) fail(`report exceeds ${MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES} bytes`);
  if (value.schema !== AUTONOMOUS_SELECTION_PROMOTION_SCHEMA) fail("report schema is invalid");
  if (typeof value.source_report_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.source_report_digest)) fail("report source_report_digest is malformed");
  const policy = validatePolicyProjection(value.policy);
  if (value.decision !== "admit" && value.decision !== "hold") fail("report decision is invalid");
  const reasons = boundedReasonList("report reasons", value.reasons);
  if (!Array.isArray(value.domains) || value.domains.length !== AUTONOMOUS_DOMAIN_NAMES.length) fail("report domains are malformed");
  const domains = value.domains.map((row, index) => validateDomainReport(row, index));
  if (domains.map((row) => row.domain).join("\u0000") !== AUTONOMOUS_DOMAIN_NAMES.join("\u0000")) fail("report domains are not in canonical order or contain duplicates");
  if (value.execution !== EXECUTION || value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("report retention posture is invalid");
  if (typeof value.promotion_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.promotion_digest)) fail("report promotion_digest is malformed");
  const expectedDecision: AutonomousSelectionPromotionDecision = reasons.length === 0 && domains.every((row) => row.decision !== "hold") ? "admit" : "hold";
  const allowedGlobalReasons = new Set(["selection replay report is not complete", "selection replay contains no cases"]);
  if (reasons.some((value) => !allowedGlobalReasons.has(value))) fail("report contains an unknown global reason");
  if (domains.every((row) => row.case_count === 0) && !reasons.includes("selection replay contains no cases")) fail("report omits its empty-replay reason");
  for (const row of domains) {
    const expectedReasons: string[] = [];
    if (row.case_count === 0) {
      if (policy.require_all_domains) expectedReasons.push("domain has no replay cases");
    } else {
      if (row.case_count < policy.min_cases_per_domain) expectedReasons.push(`domain has fewer than ${policy.min_cases_per_domain} replay cases`);
      if (row.evaluated_count < policy.min_evaluated_cases_per_domain) expectedReasons.push(`domain has fewer than ${policy.min_evaluated_cases_per_domain} evaluated cases`);
      if (row.evaluated_coverage < policy.min_evaluated_coverage) expectedReasons.push(`evaluated coverage is below ${policy.min_evaluated_coverage}`);
      if (row.oracle_agreement_rate === null || row.oracle_agreement_rate < policy.min_oracle_agreement_rate) expectedReasons.push(`oracle agreement is below ${policy.min_oracle_agreement_rate}`);
      if (row.mean_regret === null || row.mean_regret > policy.max_mean_regret) expectedReasons.push(`mean regret exceeds ${policy.max_mean_regret}`);
      if (row.abstention_rate > policy.max_abstention_rate) expectedReasons.push(`abstention rate exceeds ${policy.max_abstention_rate}`);
      if (row.selected_reward_missing_rate > policy.max_selected_reward_missing_rate) expectedReasons.push(`selected reward missing rate exceeds ${policy.max_selected_reward_missing_rate}`);
      if (row.no_eligible_model_rate > policy.max_no_eligible_model_rate) expectedReasons.push(`no eligible model rate exceeds ${policy.max_no_eligible_model_rate}`);
      if (row.no_counterfactual_reward_rate > policy.max_no_counterfactual_reward_rate) expectedReasons.push(`no counterfactual reward rate exceeds ${policy.max_no_counterfactual_reward_rate}`);
    }
    if (canonicalJson(row.reasons) !== canonicalJson(expectedReasons)) fail(`report domain ${row.domain} reasons do not match its policy metrics`);
    const expectedDomainDecision: AutonomousSelectionPromotionDomainDecision = row.case_count === 0 && !policy.require_all_domains
      ? "not_required"
      : expectedReasons.length === 0 ? "admit" : "hold";
    if (row.decision !== expectedDomainDecision) fail(`report domain ${row.domain} decision does not match its policy metrics`);
  }
  if (value.decision !== expectedDecision) fail("report decision does not match domain and global reasons");
  if (value.decision === "admit" && domains.some((row) => row.decision === "hold")) fail("admitted report contains a held domain");
  if (digestJsonSync(reportBody(value as unknown as AutonomousSelectionPromotionReport)) !== value.promotion_digest) fail("report promotion_digest does not match its canonical projection");
  return structuredClone({ ...value, policy: value.policy, reasons, domains }) as AutonomousSelectionPromotionReport;
}

/**
 * Convert a validated replay report into a deterministic learner-promotion decision.
 *
 * This is deliberately an admission boundary rather than a learner mutation API: it never
 * invokes a provider, reads a credential, assigns a reward, or changes online state.
 */
export function evaluateAutonomousSelectionPromotion(
  report: AutonomousSelectionLabReport,
  options: AutonomousSelectionPromotionPolicy = {},
): AutonomousSelectionPromotionReport {
  const validatedReport = validateAutonomousSelectionLabReport(report);
  const policy = validateOptions(options);
  const reasons: string[] = [];
  if (validatedReport.status !== "completed") reason(reasons, "selection replay report is not complete");
  if (validatedReport.case_count === 0) reason(reasons, "selection replay contains no cases");
  const domains = AUTONOMOUS_DOMAIN_NAMES.map((domain) => evaluateDomain(
    validatedReport.domains.find((row) => row.domain === domain)!,
    policy,
  ));
  const body = {
    schema: AUTONOMOUS_SELECTION_PROMOTION_SCHEMA,
    source_report_digest: validatedReport.report_digest,
    policy,
    decision: reasons.length === 0 && domains.every((row) => row.decision !== "hold") ? "admit" as const : "hold" as const,
    reasons,
    domains,
    execution: EXECUTION,
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  } satisfies Omit<AutonomousSelectionPromotionReport, "promotion_digest">;
  const result = { ...body, promotion_digest: digestJsonSync(body) };
  if (new TextEncoder().encode(canonicalJson(result)).byteLength > MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES) fail(`report exceeds ${MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES} bytes`);
  return validateAutonomousSelectionPromotionReport(result);
}
