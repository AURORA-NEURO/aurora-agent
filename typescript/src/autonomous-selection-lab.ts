import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, AutonomousOnlineLearner } from "./autonomous.js";
import {
  autonomousSelectionConfidence,
  rankAutonomousModels,
  type AutonomousModelSelector,
  type AutonomousSelectionDecision,
  type AutonomousSelectionRequest,
  type ProviderHealth,
} from "./llm.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Stable schema for provider-free selection-policy replay cases. */
export const AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA = "bioprism-typescript-autonomous-selection-lab-case/0.1" as const;
/** Stable schema for provider-free selection-policy replay reports. */
export const AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA = "bioprism-typescript-autonomous-selection-lab-report/0.1" as const;

export const MAX_AUTONOMOUS_SELECTION_LAB_CASES = 4_096;
export const MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES = 128;
export const MAX_AUTONOMOUS_SELECTION_LAB_CAPABILITIES = 64;
export const MAX_AUTONOMOUS_SELECTION_LAB_HEALTH_ROWS = 512;
export const MAX_AUTONOMOUS_SELECTION_LAB_TASK_BYTES = 1_000_000;
export const MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES = 2_000_000;

const RETENTION = "metadata_only;tasks_candidates_and_raw_selector_output_not_retained" as const;
const SECRET_MATERIAL = "never_returned" as const;

export type AutonomousSelectionLabStatus =
  | "evaluated"
  | "abstained"
  | "selected_reward_missing"
  | "no_eligible_model"
  | "no_counterfactual_reward";

export interface AutonomousSelectionLabCase extends JsonObject {
  case_id: string;
  domain: (typeof AUTONOMOUS_DOMAIN_NAMES)[number];
  request: AutonomousSelectionRequest;
  /** Counterfactual evaluator rewards supplied by the caller; values are never returned. */
  rewards: Record<string, number | null>;
}

export interface AutonomousSelectionLabCaseResult extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA;
  case_id: string;
  domain: (typeof AUTONOMOUS_DOMAIN_NAMES)[number];
  task_digest: string;
  request_digest: string;
  selected_model_id: string | null;
  oracle_model_id: string | null;
  selected_reward: number | null;
  oracle_reward: number | null;
  regret: number | null;
  selection_confidence: number | null;
  eligible_candidate_count: number;
  counterfactual_candidate_count: number;
  status: AutonomousSelectionLabStatus;
  selection_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousSelectionLabDomainReport extends JsonObject {
  domain: (typeof AUTONOMOUS_DOMAIN_NAMES)[number];
  case_count: number;
  evaluated_count: number;
  abstained_count: number;
  selected_reward_missing_count: number;
  no_eligible_model_count: number;
  no_counterfactual_reward_count: number;
  oracle_agreement_count: number;
  total_regret: number;
  mean_selected_reward: number | null;
  mean_oracle_reward: number | null;
  mean_regret: number | null;
  evaluated_coverage: number;
}

export interface AutonomousSelectionLabReport extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA;
  status: "completed" | "insufficient_coverage";
  selector_label: string;
  require_all_domains: boolean;
  case_count: number;
  evaluated_case_count: number;
  abstained_case_count: number;
  selected_reward_missing_count: number;
  no_eligible_model_count: number;
  no_counterfactual_reward_count: number;
  oracle_agreement_count: number;
  oracle_agreement_rate: number | null;
  total_regret: number;
  mean_regret: number | null;
  missing_domains: (typeof AUTONOMOUS_DOMAIN_NAMES)[number][];
  domains: AutonomousSelectionLabDomainReport[];
  cases: AutonomousSelectionLabCaseResult[];
  report_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousSelectionLabOptions {
  selector?: AutonomousModelSelector;
  learner?: AutonomousOnlineLearner;
  selectorLabel?: string;
  requireAllDomains?: boolean;
}

type DomainName = (typeof AUTONOMOUS_DOMAIN_NAMES)[number];

function fail(message: string): never {
  throw new ArgumentError(`autonomous selection lab ${message}`);
}

function boundedText(name: string, value: unknown, maximumBytes: number): string {
  if (typeof value !== "string" || value.trim().length === 0) fail(`${name} must be a non-empty string`);
  if (new TextEncoder().encode(value).byteLength > maximumBytes) fail(`${name} exceeds ${maximumBytes} bytes`);
  return value;
}

function boundedIdentifier(name: string, value: unknown, maximumBytes: number): string {
  const text = boundedText(name, value, maximumBytes);
  if (/\u0000/.test(text)) fail(`${name} contains a NUL character`);
  return text;
}

function boundedNumber(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} is outside its numeric bounds`);
  return value;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its integer bounds`);
  return value as number;
}

function optionalNumber(name: string, value: unknown, minimum: number, maximum: number): void {
  if (value !== undefined && value !== null) boundedNumber(name, value, minimum, maximum);
}

function optionalBoolean(name: string, value: unknown): void {
  if (value !== undefined && typeof value !== "boolean") fail(`${name} must be boolean`);
}

function domainName(value: unknown): DomainName {
  if (typeof value !== "string" || !(AUTONOMOUS_DOMAIN_NAMES as readonly string[]).includes(value)) fail("domain is not a supported autonomous domain");
  return value as DomainName;
}

function validateCapabilities(name: string, value: unknown): void {
  if (value === undefined) return;
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_SELECTION_LAB_CAPABILITIES) fail(`${name} must contain at most ${MAX_AUTONOMOUS_SELECTION_LAB_CAPABILITIES} capabilities`);
  const seen = new Set<string>();
  for (const capability of value) {
    const normalized = boundedIdentifier(`${name} item`, capability, 128);
    if (seen.has(normalized)) fail(`${name} contains a duplicate capability`);
    seen.add(normalized);
  }
}

function validateHealthRow(name: string, value: unknown): asserts value is ProviderHealth {
  if (!isObject(value)) fail(`${name} must be an object`);
  if (value.provider !== undefined) boundedIdentifier(`${name}.provider`, value.provider, 128);
  if (value.circuit !== undefined && value.circuit !== "closed" && value.circuit !== "open") fail(`${name}.circuit is invalid`);
  for (const field of ["consecutive_failures", "attempts", "successes", "failures", "quality_observations"] as const) {
    if (value[field] !== undefined) boundedInteger(`${name}.${field}`, value[field], 0, 100_000_000);
  }
  for (const field of ["success_rate", "quality_mean"] as const) {
    if (value[field] !== undefined && value[field] !== null) boundedNumber(`${name}.${field}`, value[field], 0, 1);
  }
  for (const field of ["mean_latency_ms", "last_latency_ms"] as const) {
    if (value[field] !== undefined && value[field] !== null) boundedNumber(`${name}.${field}`, value[field], 0, 24 * 60 * 60 * 1000);
  }
  if (value.last_model !== undefined && value.last_model !== null) boundedIdentifier(`${name}.last_model`, value.last_model, 512);
  if (value.last_status_code !== undefined && value.last_status_code !== null) boundedInteger(`${name}.last_status_code`, value.last_status_code, 100, 999);
  if (value.credential_posture !== undefined && value.credential_posture !== "caller_supplied_opaque_handle" && value.credential_posture !== "caller_supplied_in_memory_handle") fail(`${name}.credential_posture is invalid`);
  optionalBoolean(`${name}.credential_required`, value.credential_required);
  optionalBoolean(`${name}.credential_ready`, value.credential_ready);
  if (value.structured_output_mode !== undefined && value.structured_output_mode !== "disabled" && value.structured_output_mode !== "json_object" && value.structured_output_mode !== "json_schema") fail(`${name}.structured_output_mode is invalid`);
}

function validateHealthMap(name: string, value: unknown, maximumRows: number): void {
  if (!isObject(value)) fail(`${name} must be an object`);
  const entries = Object.entries(value);
  if (entries.length > maximumRows) fail(`${name} contains too many rows`);
  for (const [key, row] of entries) {
    boundedIdentifier(`${name} key`, key, 768);
    validateHealthRow(`${name}.${key}`, row);
  }
}

function validateCandidate(candidate: unknown, index: number, seen: Set<string>): string {
  if (!isObject(candidate)) fail(`candidate ${index} must be an object`);
  const provider = boundedIdentifier(`candidate ${index}.provider`, candidate.provider, 128);
  const model = boundedIdentifier(`candidate ${index}.model`, candidate.model, 512);
  validateCapabilities(`candidate ${index}.capabilities`, candidate.capabilities);
  boundedInteger(`candidate ${index}.context_window_tokens`, candidate.context_window_tokens, 1, 1_000_000_000);
  boundedInteger(`candidate ${index}.max_output_tokens`, candidate.max_output_tokens, 1, 1_000_000_000);
  boundedNumber(`candidate ${index}.quality`, candidate.quality, 0, 1);
  boundedNumber(`candidate ${index}.latency_ms`, candidate.latency_ms, 0, 24 * 60 * 60 * 1000);
  boundedNumber(`candidate ${index}.cost_per_million_tokens`, candidate.cost_per_million_tokens, 0, 1_000_000_000);
  boundedNumber(`candidate ${index}.reliability`, candidate.reliability, 0, 1);
  optionalBoolean(`candidate ${index}.requires_credential`, candidate.requires_credential);
  optionalBoolean(`candidate ${index}.enabled`, candidate.enabled);
  const armId = `${provider}/${model}`;
  if (seen.has(armId)) fail(`candidate arm ${armId} is duplicated`);
  seen.add(armId);
  return armId;
}

function validateSelectionRequest(request: unknown, expectedDomain: DomainName): AutonomousSelectionRequest {
  if (!isObject(request)) fail("case request must be an object");
  const domain = domainName(request.domain);
  if (domain !== expectedDomain) fail(`case request domain must equal ${expectedDomain}`);
  boundedText("request.task", request.task, MAX_AUTONOMOUS_SELECTION_LAB_TASK_BYTES);
  boundedIdentifier("request.capability", request.capability, 256);
  boundedIdentifier("request.risk_class", request.risk_class, 128);
  if (request.task_family !== undefined && request.task_family !== null) boundedIdentifier("request.task_family", request.task_family, 256);
  if (request.context_digest !== undefined && request.context_digest !== null && (typeof request.context_digest !== "string" || !/^[0-9a-f]{64}$/.test(request.context_digest))) fail("request.context_digest must be a lowercase SHA-256 digest");
  validateCapabilities("request.required_capabilities", request.required_capabilities);
  if (!Array.isArray(request.required_capabilities)) fail("request.required_capabilities must be an array");
  boundedInteger("request.estimated_input_tokens", request.estimated_input_tokens, 0, 1_000_000_000);
  boundedInteger("request.requested_output_tokens", request.requested_output_tokens, 0, 1_000_000_000);
  optionalNumber("request.max_cost_per_million_tokens", request.max_cost_per_million_tokens, 0, 1_000_000_000);
  optionalNumber("request.max_latency_ms", request.max_latency_ms, 0, 24 * 60 * 60 * 1000);
  optionalNumber("request.min_quality", request.min_quality, 0, 1);
  optionalNumber("request.min_selection_confidence", request.min_selection_confidence, 0, 1);
  optionalBoolean("request.require_json", request.require_json);
  if (!Array.isArray(request.candidates) || request.candidates.length === 0 || request.candidates.length > MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES) fail(`request.candidates must contain 1-${MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES} candidates`);
  const candidateIds = new Set<string>();
  request.candidates.forEach((candidate, index) => validateCandidate(candidate, index, candidateIds));
  validateHealthMap("request.provider_health", request.provider_health, MAX_AUTONOMOUS_SELECTION_LAB_HEALTH_ROWS);
  validateHealthMap("request.model_health", request.model_health, MAX_AUTONOMOUS_SELECTION_LAB_HEALTH_ROWS);
  return request as AutonomousSelectionRequest;
}

function validateLabCase(value: unknown, index: number, seenCaseIds: Set<string>): AutonomousSelectionLabCase {
  if (!isObject(value)) fail(`case ${index} must be an object`);
  const caseId = boundedIdentifier(`case ${index}.case_id`, value.case_id, 256);
  if (seenCaseIds.has(caseId)) fail(`case_id ${caseId} is duplicated`);
  seenCaseIds.add(caseId);
  const domain = domainName(value.domain);
  const request = validateSelectionRequest(value.request, domain);
  if (!isObject(value.rewards)) fail(`case ${index}.rewards must be an object`);
  const candidateIds = new Set(request.candidates.map((candidate) => `${candidate.provider}/${candidate.model}`));
  for (const [armId, reward] of Object.entries(value.rewards)) {
    if (!candidateIds.has(armId)) fail(`case ${index}.rewards contains an unknown arm`);
    if (reward !== null) boundedNumber(`case ${index}.rewards.${armId}`, reward, -1, 1);
  }
  return value as AutonomousSelectionLabCase;
}

function validateDecision(value: unknown, ranking: ReturnType<typeof rankAutonomousModels>, request: AutonomousSelectionRequest): { selectedModelId: string | null; confidence: number | null } {
  if (!isObject(value)) fail("selector returned a non-object decision");
  if (!Object.prototype.hasOwnProperty.call(value, "selected_model")) fail("selector decision omitted selected_model");
  const selected = value.selected_model;
  let selectedModelId: string | null = null;
  if (selected !== null) {
    if (!isObject(selected)) fail("selector selected_model must be null or an object");
    const provider = boundedIdentifier("selector selected_model.provider", selected.provider, 128);
    const model = boundedIdentifier("selector selected_model.model", selected.model, 512);
    selectedModelId = `${provider}/${model}`;
    const candidate = request.candidates.find((item) => `${item.provider}/${item.model}` === selectedModelId);
    if (!candidate) fail("selector selected an unknown model arm");
    const canonical = ranking.find((row) => `${row.provider}/${row.model}` === selectedModelId);
    if (!canonical?.eligible) fail("selector selected an ineligible model arm");
  }
  if (value.strategy !== undefined) boundedIdentifier("selector strategy", value.strategy, 128);
  if (value.abstention_reason !== undefined && value.abstention_reason !== null) boundedText("selector abstention_reason", value.abstention_reason, 4_096);
  const rawConfidence = value.selection_confidence;
  const confidence = rawConfidence === undefined || rawConfidence === null ? null : boundedNumber("selector selection_confidence", rawConfidence, 0, 1);
  return { selectedModelId, confidence };
}

function rounded(value: number): number {
  return Number(value.toFixed(12));
}

function selectionDigest(result: {
  case_id: string;
  selected_model_id: string | null;
  oracle_model_id: string | null;
  selected_reward: number | null;
  oracle_reward: number | null;
  regret: number | null;
  status: AutonomousSelectionLabStatus;
  selection_confidence: number | null;
}): string {
  return digestJsonSync({
    case_id: result.case_id,
    selected_model_id: result.selected_model_id,
    oracle_model_id: result.oracle_model_id,
    selected_reward: result.selected_reward,
    oracle_reward: result.oracle_reward,
    regret: result.regret,
    status: result.status,
    selection_confidence: result.selection_confidence,
  });
}

function projectionDigest(value: AutonomousSelectionLabReport): string {
  const { report_digest: _reportDigest, ...body } = value;
  return digestJsonSync(body);
}

function validateReportShape(value: unknown): AutonomousSelectionLabReport {
  if (!isObject(value)) fail("report must be an object");
  if (value.schema !== AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA) fail("report schema is invalid");
  if (value.status !== "completed" && value.status !== "insufficient_coverage") fail("report status is invalid");
  boundedIdentifier("report selector_label", value.selector_label, 256);
  if (typeof value.require_all_domains !== "boolean") fail("report require_all_domains must be boolean");
  for (const field of ["case_count", "evaluated_case_count", "abstained_case_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count"] as const) boundedInteger(`report ${field}`, value[field], 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES);
  boundedNumber("report total_regret", value.total_regret, 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES * 2);
  if (value.oracle_agreement_rate !== null) boundedNumber("report oracle_agreement_rate", value.oracle_agreement_rate, 0, 1);
  if (value.mean_regret !== null) boundedNumber("report mean_regret", value.mean_regret, 0, 2);
  if (!Array.isArray(value.missing_domains) || !Array.isArray(value.domains) || value.domains.length !== AUTONOMOUS_DOMAIN_NAMES.length || !Array.isArray(value.cases) || value.cases.length !== value.case_count) fail("report domain or case projections are malformed");
  if (value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail("report retention posture is invalid");
  if (typeof value.report_digest !== "string" || !/^[0-9a-f]{64}$/.test(value.report_digest)) fail("report digest is malformed");
  const domains = value.domains.map((row, index) => validateDomainReport(row, index));
  const cases = value.cases.map((row, index) => validateCaseResult(row, index));
  const report = value as unknown as AutonomousSelectionLabReport;
  const expectedDomainOrder = AUTONOMOUS_DOMAIN_NAMES.join("\u0000");
  if (domains.map((row) => row.domain).join("\u0000") !== expectedDomainOrder) fail("report domains are not in canonical order or contain duplicates");
  const missingDomains = domains.filter((row) => row.case_count === 0).map((row) => row.domain);
  if (canonicalJson(missingDomains) !== canonicalJson(report.missing_domains)) fail("report missing_domains does not match domain coverage");
  const expectedStatus = report.require_all_domains && missingDomains.length > 0 ? "insufficient_coverage" : "completed";
  if (report.status !== expectedStatus) fail("report status does not match its coverage policy");
  const perDomain = new Map<DomainName, {
    case_count: number;
    evaluated_count: number;
    abstained_count: number;
    selected_reward_missing_count: number;
    no_eligible_model_count: number;
    no_counterfactual_reward_count: number;
    oracle_agreement_count: number;
    total_regret: number;
    selected_rewards: number[];
    oracle_rewards: number[];
    regrets: number[];
  }>(AUTONOMOUS_DOMAIN_NAMES.map((domain) => [domain, {
    case_count: 0,
    evaluated_count: 0,
    abstained_count: 0,
    selected_reward_missing_count: 0,
    no_eligible_model_count: 0,
    no_counterfactual_reward_count: 0,
    oracle_agreement_count: 0,
    total_regret: 0,
    selected_rewards: [],
    oracle_rewards: [],
    regrets: [],
  }]));
  const caseIds = new Set<string>();
  for (const result of cases) {
    if (caseIds.has(result.case_id)) fail("report case ids are duplicated");
    caseIds.add(result.case_id);
    const aggregate = perDomain.get(result.domain)!;
    aggregate.case_count += 1;
    if (result.status === "evaluated") {
      if (result.selected_model_id === null || result.oracle_model_id === null || result.selected_reward === null || result.oracle_reward === null || result.regret === null) fail(`report case ${result.case_id} evaluated fields are incomplete`);
      const expectedRegret = rounded(Math.max(0, result.oracle_reward - result.selected_reward));
      if (result.regret !== expectedRegret) fail(`report case ${result.case_id} regret is inconsistent with its rewards`);
      aggregate.evaluated_count += 1;
      aggregate.selected_rewards.push(result.selected_reward);
      aggregate.oracle_rewards.push(result.oracle_reward);
      aggregate.regrets.push(result.regret);
      aggregate.total_regret += result.regret;
      if (result.selected_model_id === result.oracle_model_id) aggregate.oracle_agreement_count += 1;
    } else if (result.status === "abstained") {
      if (result.selected_model_id !== null || result.selected_reward !== null || result.oracle_model_id === null || result.oracle_reward === null || result.regret !== null) fail(`report case ${result.case_id} abstention fields are inconsistent`);
      aggregate.abstained_count += 1;
    } else if (result.status === "selected_reward_missing") {
      if (result.selected_model_id === null || result.selected_reward !== null || result.oracle_model_id === null || result.oracle_reward === null || result.regret !== null) fail(`report case ${result.case_id} missing-reward fields are inconsistent`);
      aggregate.selected_reward_missing_count += 1;
    } else if (result.status === "no_eligible_model") {
      if (result.eligible_candidate_count !== 0 || result.counterfactual_candidate_count !== 0 || result.selected_model_id !== null || result.oracle_model_id !== null || result.selected_reward !== null || result.oracle_reward !== null || result.regret !== null) fail(`report case ${result.case_id} no-eligible fields are inconsistent`);
      aggregate.no_eligible_model_count += 1;
    } else {
      if (result.counterfactual_candidate_count !== 0 || result.oracle_model_id !== null || result.oracle_reward !== null || result.regret !== null) fail(`report case ${result.case_id} no-counterfactual fields are inconsistent`);
      aggregate.no_counterfactual_reward_count += 1;
    }
  }
  const total = { evaluated_count: 0, abstained_count: 0, selected_reward_missing_count: 0, no_eligible_model_count: 0, no_counterfactual_reward_count: 0, oracle_agreement_count: 0, total_regret: 0 };
  for (const row of domains) {
    const aggregate = perDomain.get(row.domain)!;
    const mean = (values: number[]): number | null => values.length === 0 ? null : rounded(values.reduce((sum, value) => sum + value, 0) / values.length);
    for (const field of ["case_count", "evaluated_count", "abstained_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count"] as const) {
      if (row[field] !== aggregate[field]) fail(`report domain ${row.domain}.${field} disagrees with its cases`);
    }
    if (row.total_regret !== rounded(aggregate.total_regret) || row.mean_selected_reward !== mean(aggregate.selected_rewards) || row.mean_oracle_reward !== mean(aggregate.oracle_rewards) || row.mean_regret !== mean(aggregate.regrets) || row.evaluated_coverage !== (aggregate.case_count === 0 ? 0 : rounded(aggregate.evaluated_count / aggregate.case_count))) fail(`report domain ${row.domain} metrics disagree with its cases`);
    total.evaluated_count += aggregate.evaluated_count;
    total.abstained_count += aggregate.abstained_count;
    total.selected_reward_missing_count += aggregate.selected_reward_missing_count;
    total.no_eligible_model_count += aggregate.no_eligible_model_count;
    total.no_counterfactual_reward_count += aggregate.no_counterfactual_reward_count;
    total.oracle_agreement_count += aggregate.oracle_agreement_count;
    total.total_regret += aggregate.total_regret;
  }
  if (report.case_count !== cases.length || report.evaluated_case_count !== total.evaluated_count || report.abstained_case_count !== total.abstained_count || report.selected_reward_missing_count !== total.selected_reward_missing_count || report.no_eligible_model_count !== total.no_eligible_model_count || report.no_counterfactual_reward_count !== total.no_counterfactual_reward_count || report.oracle_agreement_count !== total.oracle_agreement_count || report.total_regret !== rounded(total.total_regret) || report.mean_regret !== (total.evaluated_count === 0 ? null : rounded(total.total_regret / total.evaluated_count)) || report.oracle_agreement_rate !== (total.evaluated_count === 0 ? null : rounded(total.oracle_agreement_count / total.evaluated_count))) fail("report aggregate metrics disagree with its cases");
  if (projectionDigest(report) !== value.report_digest) fail("report digest does not match its canonical projection");
  return structuredClone({ ...report, domains, cases });
}

function validateDomainReport(value: unknown, index: number): AutonomousSelectionLabDomainReport {
  if (!isObject(value)) fail(`report domain ${index} must be an object`);
  const domain = domainName(value.domain);
  for (const field of ["case_count", "evaluated_count", "abstained_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count"] as const) boundedInteger(`report domain ${domain}.${field}`, value[field], 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES);
  boundedNumber(`report domain ${domain}.total_regret`, value.total_regret, 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES * 2);
  if (value.mean_selected_reward !== null) boundedNumber(`report domain ${domain}.mean_selected_reward`, value.mean_selected_reward, -1, 1);
  if (value.mean_oracle_reward !== null) boundedNumber(`report domain ${domain}.mean_oracle_reward`, value.mean_oracle_reward, -1, 1);
  if (value.mean_regret !== null) boundedNumber(`report domain ${domain}.mean_regret`, value.mean_regret, 0, 2);
  boundedNumber(`report domain ${domain}.evaluated_coverage`, value.evaluated_coverage, 0, 1);
  return value as AutonomousSelectionLabDomainReport;
}

function validateCaseResult(value: unknown, index: number): AutonomousSelectionLabCaseResult {
  if (!isObject(value)) fail(`report case ${index} must be an object`);
  if (value.schema !== AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA) fail(`report case ${index} schema is invalid`);
  const caseId = boundedIdentifier(`report case ${index}.case_id`, value.case_id, 256);
  const domain = domainName(value.domain);
  for (const field of ["task_digest", "request_digest", "selection_digest"] as const) {
    if (typeof value[field] !== "string" || !/^[0-9a-f]{64}$/.test(value[field])) fail(`report case ${caseId}.${field} is malformed`);
  }
  const selectedModelId = value.selected_model_id;
  const oracleModelId = value.oracle_model_id;
  if (selectedModelId !== null) boundedIdentifier(`report case ${caseId}.selected_model_id`, selectedModelId, 768);
  if (oracleModelId !== null) boundedIdentifier(`report case ${caseId}.oracle_model_id`, oracleModelId, 768);
  if (value.selected_reward !== null) boundedNumber(`report case ${caseId}.selected_reward`, value.selected_reward, -1, 1);
  if (value.oracle_reward !== null) boundedNumber(`report case ${caseId}.oracle_reward`, value.oracle_reward, -1, 1);
  if (value.regret !== null) boundedNumber(`report case ${caseId}.regret`, value.regret, 0, 2);
  if (value.selection_confidence !== null) boundedNumber(`report case ${caseId}.selection_confidence`, value.selection_confidence, 0, 1);
  boundedInteger(`report case ${caseId}.eligible_candidate_count`, value.eligible_candidate_count, 0, MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES);
  boundedInteger(`report case ${caseId}.counterfactual_candidate_count`, value.counterfactual_candidate_count, 0, MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES);
  if (value.status !== "evaluated" && value.status !== "abstained" && value.status !== "selected_reward_missing" && value.status !== "no_eligible_model" && value.status !== "no_counterfactual_reward") fail(`report case ${caseId}.status is invalid`);
  if (value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) fail(`report case ${caseId} retention posture is invalid`);
  const result = value as unknown as AutonomousSelectionLabCaseResult;
  if (selectionDigest(result) !== value.selection_digest) fail(`report case ${caseId} selection digest is invalid`);
  return result;
}

/**
 * Replay a model-selection policy against caller-owned counterfactual evaluator rewards.
 *
 * This is intentionally provider-free: it exercises health gates, deterministic ranking,
 * contextual/bandit selection, abstention, and regret measurement without invoking a model,
 * reading a key, or retaining task/candidate content in the returned report.
 */
export async function evaluateAutonomousSelectionPolicy(
  cases: readonly AutonomousSelectionLabCase[],
  options: AutonomousSelectionLabOptions = {},
): Promise<AutonomousSelectionLabReport> {
  if (!Array.isArray(cases) || cases.length > MAX_AUTONOMOUS_SELECTION_LAB_CASES) fail(`cases must contain at most ${MAX_AUTONOMOUS_SELECTION_LAB_CASES} items`);
  if (!isObject(options)) fail("options must be an object");
  if (options.selector !== undefined && typeof options.selector !== "function") fail("selector must be callable");
  if (options.learner !== undefined && !(options.learner instanceof AutonomousOnlineLearner)) fail("learner must be an AutonomousOnlineLearner");
  if (options.selector !== undefined && options.learner !== undefined) fail("selector and learner cannot both be supplied");
  const selectorLabel = options.selectorLabel === undefined
    ? options.learner ? "autonomous_online_learner" : options.selector ? "caller_selector" : "deterministic_health_utility"
    : boundedIdentifier("selectorLabel", options.selectorLabel, 256);
  if (options.requireAllDomains !== undefined && typeof options.requireAllDomains !== "boolean") fail("requireAllDomains must be boolean");
  const requireAllDomains = options.requireAllDomains ?? false;
  const seenCaseIds = new Set<string>();
  const normalizedCases = cases.map((value, index) => validateLabCase(value, index, seenCaseIds)).sort((left, right) => left.case_id.localeCompare(right.case_id));
  const results: AutonomousSelectionLabCaseResult[] = [];
  for (const labCase of normalizedCases) {
    const request = labCase.request;
    const ranking = rankAutonomousModels(request);
    const counterfactual = ranking.filter((row) => row.eligible && typeof labCase.rewards[`${row.provider}/${row.model}`] === "number")
      .sort((left, right) => (labCase.rewards[`${right.provider}/${right.model}`] as number) - (labCase.rewards[`${left.provider}/${left.model}`] as number) || `${left.provider}/${left.model}`.localeCompare(`${right.provider}/${right.model}`));
    const decision: AutonomousSelectionDecision = options.learner
      ? options.learner.select(request)
      : options.selector
        ? await options.selector(request)
        : (() => {
          const top = ranking.find((row) => row.eligible);
          return {
            selected_model: top ? { provider: top.provider, model: top.model } : null,
            strategy: "deterministic_health_utility" as const,
            ranking,
            abstention_reason: top ? null : "no eligible model",
            selection_confidence: autonomousSelectionConfidence(ranking),
            min_selection_confidence: request.min_selection_confidence ?? null,
          };
        })();
    const validatedDecision = validateDecision(decision, ranking, request);
    const selectedReward = validatedDecision.selectedModelId !== null && typeof labCase.rewards[validatedDecision.selectedModelId] === "number"
      ? labCase.rewards[validatedDecision.selectedModelId] as number
      : null;
    const oracle = counterfactual[0];
    const oracleModelId = oracle ? `${oracle.provider}/${oracle.model}` : null;
    const oracleReward = oracle ? labCase.rewards[oracleModelId!] as number : null;
    const status: AutonomousSelectionLabStatus = ranking.every((row) => !row.eligible)
      ? "no_eligible_model"
      : counterfactual.length === 0
        ? "no_counterfactual_reward"
        : validatedDecision.selectedModelId === null
          ? "abstained"
          : selectedReward === null
            ? "selected_reward_missing"
            : "evaluated";
    const regret = status === "evaluated" ? rounded(Math.max(0, (oracleReward as number) - (selectedReward as number))) : null;
    const taskDigest = digestJsonSync(request.task);
    const requestDigest = digestJsonSync({ ...request, task: taskDigest });
    const base = {
      schema: AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA,
      case_id: labCase.case_id,
      domain: labCase.domain,
      task_digest: taskDigest,
      request_digest: requestDigest,
      selected_model_id: validatedDecision.selectedModelId,
      oracle_model_id: oracleModelId,
      selected_reward: selectedReward,
      oracle_reward: oracleReward,
      regret,
      selection_confidence: validatedDecision.confidence,
      eligible_candidate_count: ranking.filter((row) => row.eligible).length,
      counterfactual_candidate_count: counterfactual.length,
      status,
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
    const completeResult: AutonomousSelectionLabCaseResult = { ...base, selection_digest: selectionDigest(base) };
    results.push(completeResult);
  }
  const domainReports = AUTONOMOUS_DOMAIN_NAMES.map((domain): AutonomousSelectionLabDomainReport => {
    const domainCases = results.filter((result) => result.domain === domain);
    const evaluated = domainCases.filter((result) => result.status === "evaluated");
    const mean = (values: number[]): number | null => values.length === 0 ? null : rounded(values.reduce((sum, value) => sum + value, 0) / values.length);
    return {
      domain,
      case_count: domainCases.length,
      evaluated_count: evaluated.length,
      abstained_count: domainCases.filter((result) => result.status === "abstained").length,
      selected_reward_missing_count: domainCases.filter((result) => result.status === "selected_reward_missing").length,
      no_eligible_model_count: domainCases.filter((result) => result.status === "no_eligible_model").length,
      no_counterfactual_reward_count: domainCases.filter((result) => result.status === "no_counterfactual_reward").length,
      oracle_agreement_count: evaluated.filter((result) => result.selected_model_id === result.oracle_model_id).length,
      total_regret: rounded(evaluated.reduce((sum, result) => sum + (result.regret ?? 0), 0)),
      mean_selected_reward: mean(evaluated.map((result) => result.selected_reward as number)),
      mean_oracle_reward: mean(evaluated.map((result) => result.oracle_reward as number)),
      mean_regret: mean(evaluated.map((result) => result.regret as number)),
      evaluated_coverage: domainCases.length === 0 ? 0 : rounded(evaluated.length / domainCases.length),
    };
  });
  const missingDomains = domainReports.filter((row) => row.case_count === 0).map((row) => row.domain);
  const evaluated = results.filter((result) => result.status === "evaluated");
  const body: Omit<AutonomousSelectionLabReport, "report_digest"> = {
    schema: AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA,
    status: requireAllDomains && missingDomains.length > 0 ? "insufficient_coverage" : "completed",
    selector_label: selectorLabel,
    require_all_domains: requireAllDomains,
    case_count: results.length,
    evaluated_case_count: evaluated.length,
    abstained_case_count: results.filter((result) => result.status === "abstained").length,
    selected_reward_missing_count: results.filter((result) => result.status === "selected_reward_missing").length,
    no_eligible_model_count: results.filter((result) => result.status === "no_eligible_model").length,
    no_counterfactual_reward_count: results.filter((result) => result.status === "no_counterfactual_reward").length,
    oracle_agreement_count: evaluated.filter((result) => result.selected_model_id === result.oracle_model_id).length,
    oracle_agreement_rate: evaluated.length === 0 ? null : rounded(evaluated.filter((result) => result.selected_model_id === result.oracle_model_id).length / evaluated.length),
    total_regret: rounded(evaluated.reduce((sum, result) => sum + (result.regret ?? 0), 0)),
    mean_regret: evaluated.length === 0 ? null : rounded(evaluated.reduce((sum, result) => sum + (result.regret ?? 0), 0) / evaluated.length),
    missing_domains: missingDomains,
    domains: domainReports,
    cases: results,
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  };
  const report = { ...body, report_digest: digestJsonSync(body) };
  if (new TextEncoder().encode(canonicalJson(report)).byteLength > MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES) fail(`report exceeds ${MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES} bytes`);
  return validateAutonomousSelectionLabReport(report);
}

/** Validate and clone a selection-lab report before a caller persists or forwards it. */
export function validateAutonomousSelectionLabReport(value: unknown): AutonomousSelectionLabReport {
  if (isObject(value)) {
    const encoded = (() => { try { return canonicalJson(value); } catch { return null; } })();
    if (encoded === null || new TextEncoder().encode(encoded).byteLength > MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES) fail(`report exceeds ${MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES} bytes or is not canonical JSON`);
  }
  return validateReportShape(value);
}
