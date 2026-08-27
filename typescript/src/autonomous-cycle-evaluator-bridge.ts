import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousCrossDomainRunResult,
  type AutonomousDomainName,
  type AutonomousRunResult,
} from "./autonomous.js";
import type {
  AutonomousCrossDomainDecisionCycleEvaluator,
  AutonomousCrossDomainReplanEvaluation,
  AutonomousCrossDomainReplanEvaluator,
  AutonomousDecisionCycleEvaluator,
  AutonomousReplanEvaluation,
  AutonomousReplanEvaluator,
} from "./autonomous-cycle.js";
import {
  AutonomousValueEvaluatorRegistry,
  type AutonomousValueEvaluation,
  type AutonomousValueEvaluationInput,
} from "./autonomous-domain-evaluators.js";
import {
  autonomousEvaluatorCalibrationAdmission,
  validateAutonomousEvaluatorCalibrationReport,
  type AutonomousEvaluatorCalibrationReport,
} from "./autonomous-evaluator-calibration.js";
import {
  validateAutonomousEvidenceSourceReceipt,
  type AutonomousEvidenceSourceAuthority,
  type AutonomousEvidenceSourceReceiptJSON,
} from "./autonomous-evidence-source.js";
import type { AutonomousEvidenceProviderFreshnessMode } from "./autonomous-evidence-provider-contract.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Shared schema for caller-evidence adapters used by ordinary and replan cycles. */
export const AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA = "bioprism-typescript-autonomous-cycle-evaluator-bridge/0.1" as const;

export type AutonomousCycleEvaluatorMode = "single_domain" | "cross_domain";
export type AutonomousCycleEvaluatorRole = "single" | "specialist" | "synthesis";

/**
 * Metadata-only context supplied to the evidence factory. It deliberately excludes task text,
 * prompts, provider responses, credentials, and raw tool/evidence values.
 */
export interface AutonomousCycleEvaluatorEvidenceContext extends JsonObject {
  schema: typeof AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA;
  mode: AutonomousCycleEvaluatorMode;
  domain: AutonomousDomainName;
  role: AutonomousCycleEvaluatorRole;
  route_digest: string;
  run_status: string;
  learning_episode_id: string | null;
  learning_episode_ids: string[];
  selected_domains: AutonomousDomainName[];
  child_count: number;
  completed_child_count: number;
  evaluator_id: string;
  evaluator_version: string;
  required_signals: string[];
  pass_threshold: number;
  /** Accepted source metadata, or null when no source gate was configured. */
  source_receipt_digest: string | null;
  source_id: string | null;
  source_kind: string | null;
  source_authority: AutonomousEvidenceSourceAuthority | null;
  source_freshness: AutonomousEvidenceProviderFreshnessMode | null;
  source_decision: "not_configured" | "accepted";
  /** Calibration metadata, or null when no evaluator calibration gate was configured. */
  evaluator_calibration_digest: string | null;
  evaluator_calibration_decision: "not_configured" | "admit_learning";
  retention: "metadata_only;caller_evidence_factory_owns_values";
  secret_material: "never_returned";
}

export type AutonomousCycleEvaluatorEvidenceFactory = (
  context: AutonomousCycleEvaluatorEvidenceContext,
) => AutonomousValueEvaluationInput | Promise<AutonomousValueEvaluationInput>;

/**
 * Optional source-provenance gate. The callback returns an existing, metadata-only source
 * receipt; it never receives or returns the source value itself.
 */
export type AutonomousCycleEvaluatorSourceReceiptFactory = (
  context: AutonomousCycleEvaluatorEvidenceContext,
) => AutonomousEvidenceSourceReceiptJSON | null | Promise<AutonomousEvidenceSourceReceiptJSON | null>;

/** Optional evaluator calibration/holdout gate for the exact routed domain and evaluator. */
export type AutonomousCycleEvaluatorCalibrationFactory = (
  context: AutonomousCycleEvaluatorEvidenceContext,
) => AutonomousEvaluatorCalibrationReport | null | Promise<AutonomousEvaluatorCalibrationReport | null>;

export interface AutonomousCycleEvaluatorBridgeOptions {
  /** Registry of reviewed value-only domain evaluators; defaults to every built-in profile. */
  registry?: AutonomousValueEvaluatorRegistry;
  /**
   * Caller-owned evidence builder. It must return bounded value-only evidence for the supplied
   * metadata context. The bridge never copies that evidence into a cycle result or checkpoint.
   */
  evidenceFor: AutonomousCycleEvaluatorEvidenceFactory;
  /** When configured, only accepted observed non-caller-declared source receipts admit evaluation. */
  sourceReceiptFor?: AutonomousCycleEvaluatorSourceReceiptFactory;
  /** When configured, only a ready calibration report admits reward settlement for the routed evaluator. */
  evaluatorCalibrationFor?: AutonomousCycleEvaluatorCalibrationFactory;
}

export interface AutonomousCycleEvaluatorBridge {
  schema: typeof AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA;
  registry: AutonomousValueEvaluatorRegistry;
  /** Digest of the reviewed evaluator catalogue and bridge retention/authority policy. */
  evaluator_catalogue_digest: string;
  policy_digest: string;
  /** Callback for `AutonomousDecisionCycleOptions.learning.evaluate`. */
  evaluate: AutonomousDecisionCycleEvaluator;
  /** Callback for `AutonomousReplanCycleOptions.evaluate`. */
  evaluateReplan: AutonomousReplanEvaluator;
  /** Callback for `AutonomousCrossDomainDecisionCycleOptions.learning.evaluate`. */
  evaluateCrossDomain: AutonomousCrossDomainDecisionCycleEvaluator;
  /** Callback for `AutonomousCrossDomainReplanCycleOptions.evaluate`. */
  evaluateCrossDomainReplan: AutonomousCrossDomainReplanEvaluator;
}

const BRIDGE_RETENTION = "metadata_only;caller_evidence_factory_owns_values" as const;

function assertCompleteRegistry(registry: AutonomousValueEvaluatorRegistry): void {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) registry.resolveForAutonomousDomain(domain);
}

function catalogueDigest(registry: AutonomousValueEvaluatorRegistry): string {
  return digestJsonSync({
    schema: AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    catalogue: registry.catalogue(),
    authority: "caller_declared_signal_scoring_only",
    retention: BRIDGE_RETENTION,
  });
}

function policyDigest(registry: AutonomousValueEvaluatorRegistry): string {
  return digestJsonSync({
    schema: AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    evaluator_catalogue_digest: catalogueDigest(registry),
    modes: ["single_domain", "cross_domain"],
    roles: ["single", "specialist", "synthesis"],
    reward_source: "caller_declared_value_only_evidence",
    provider_success_is_not_reward: true,
    source_receipt_admission: "optional;accepted_observed_non_caller_declared_source_digest_required",
    evaluator_calibration_admission: "optional;ready_exact_evaluator_identity_required",
    retention: BRIDGE_RETENTION,
  });
}

function singleDomain(run: AutonomousRunResult): AutonomousDomainName {
  const domain = run.route.primary_domain;
  if (!domain || domain === "cross_domain" || !AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError("cycle evaluator bridge requires a single-domain run with a built-in primary domain");
  return domain;
}

function selectedDomains(run: AutonomousRunResult | AutonomousCrossDomainRunResult): AutonomousDomainName[] {
  const domains = run.route.selected_domains.filter((domain): domain is AutonomousDomainName => AUTONOMOUS_DOMAIN_NAMES.includes(domain));
  if (!domains.length) throw new ArgumentError("cycle evaluator bridge received a run without selected built-in domains");
  return [...new Set(domains)];
}

function singleContext(
  run: AutonomousRunResult,
  domain: AutonomousDomainName,
  evaluator: { evaluatorId: string; evaluatorVersion: string; profile: { required_signals: string[]; pass_threshold: number } },
): AutonomousCycleEvaluatorEvidenceContext {
  return {
    schema: AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    mode: "single_domain",
    domain,
    role: "single",
    route_digest: run.route.route_digest,
    run_status: run.status,
    learning_episode_id: null,
    learning_episode_ids: [],
    selected_domains: [domain],
    child_count: 0,
    completed_child_count: run.status === "completed" ? 1 : 0,
    evaluator_id: evaluator.evaluatorId,
    evaluator_version: evaluator.evaluatorVersion,
    required_signals: [...evaluator.profile.required_signals],
    pass_threshold: evaluator.profile.pass_threshold,
    source_receipt_digest: null,
    source_id: null,
    source_kind: null,
    source_authority: null,
    source_freshness: null,
    source_decision: "not_configured",
    evaluator_calibration_digest: null,
    evaluator_calibration_decision: "not_configured",
    retention: BRIDGE_RETENTION,
    secret_material: "never_returned",
  };
}

function crossContext(
  run: AutonomousCrossDomainRunResult,
  input: {
    domain: AutonomousDomainName;
    role: AutonomousCycleEvaluatorRole;
    learningEpisodeId: string | null;
    evaluator: { evaluatorId: string; evaluatorVersion: string; profile: { required_signals: string[]; pass_threshold: number } };
  },
): AutonomousCycleEvaluatorEvidenceContext {
  return {
    schema: AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    mode: "cross_domain",
    domain: input.domain,
    role: input.role,
    route_digest: run.route.route_digest,
    run_status: run.status,
    learning_episode_id: input.learningEpisodeId,
    learning_episode_ids: [...run.learning_episode_ids],
    selected_domains: selectedDomains(run),
    child_count: run.total_children,
    completed_child_count: run.completed_children + (run.synthesis?.status === "completed" ? 1 : 0),
    evaluator_id: input.evaluator.evaluatorId,
    evaluator_version: input.evaluator.evaluatorVersion,
    required_signals: [...input.evaluator.profile.required_signals],
    pass_threshold: input.evaluator.profile.pass_threshold,
    source_receipt_digest: null,
    source_id: null,
    source_kind: null,
    source_authority: null,
    source_freshness: null,
    source_decision: "not_configured",
    evaluator_calibration_digest: null,
    evaluator_calibration_decision: "not_configured",
    retention: BRIDGE_RETENTION,
    secret_material: "never_returned",
  };
}

async function admittedContext(
  context: AutonomousCycleEvaluatorEvidenceContext,
  options: AutonomousCycleEvaluatorBridgeOptions,
): Promise<AutonomousCycleEvaluatorEvidenceContext> {
  let admitted = context;
  if (options.sourceReceiptFor !== undefined) {
    let raw: AutonomousEvidenceSourceReceiptJSON | null;
    try {
      raw = await options.sourceReceiptFor(admitted);
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError("cycle evaluator bridge source receipt callback failed");
    }
    if (raw === null) throw new ArgumentError("cycle evaluator bridge source receipt is required when sourceReceiptFor is configured");
    const receipt = validateAutonomousEvidenceSourceReceipt(raw);
    if (receipt.domain !== context.domain) throw new ArgumentError("cycle evaluator bridge source receipt domain does not match the routed evaluator");
    if (receipt.decision !== "accepted" || receipt.status !== "observed" || receipt.source_digest === null || receipt.authority === "caller_declared") {
      throw new ArgumentError("cycle evaluator bridge source receipt is not an accepted authoritative observation");
    }
    admitted = {
      ...admitted,
      source_receipt_digest: receipt.receipt_digest,
      source_id: receipt.source_id,
      source_kind: receipt.source_kind,
      source_authority: receipt.authority,
      source_freshness: receipt.freshness,
      source_decision: "accepted",
    };
  }
  if (options.evaluatorCalibrationFor !== undefined) {
    let raw: AutonomousEvaluatorCalibrationReport | null;
    try {
      raw = await options.evaluatorCalibrationFor(admitted);
    } catch (error) {
      if (error instanceof ArgumentError) throw error;
      throw new ArgumentError("cycle evaluator bridge evaluator calibration callback failed");
    }
    if (raw === null) throw new ArgumentError("cycle evaluator bridge evaluator calibration report is required when evaluatorCalibrationFor is configured");
    if (!isObject(raw)) throw new ArgumentError("cycle evaluator bridge evaluator calibration report is malformed");
    const report = validateAutonomousEvaluatorCalibrationReport(raw as AutonomousEvaluatorCalibrationReport);
    const admission = autonomousEvaluatorCalibrationAdmission(report, context.domain);
    if (admission.decision !== "admit_learning" || admission.evaluator_id !== context.evaluator_id || admission.evaluator_version !== context.evaluator_version) {
      throw new ArgumentError(`cycle evaluator bridge evaluator calibration holds ${context.domain} learning`);
    }
    admitted = {
      ...admitted,
      evaluator_calibration_digest: report.report_digest,
      evaluator_calibration_decision: "admit_learning",
    };
  }
  return admitted;
}

function rewardInput(evaluation: AutonomousValueEvaluation): AutonomousEvaluatorRewardInput {
  return {
    evaluator_id: evaluation.evaluator_id,
    evaluator_version: evaluation.evaluator_version,
    reward: evaluation.reward,
    passed: evaluation.passed,
    failed: evaluation.failed,
    failure_class: evaluation.failure_class,
    feedback_digest: evaluation.feedback_digest,
    evidence_digest: evaluation.evidence_digest,
  };
}

function replanEvaluation(evaluation: AutonomousValueEvaluation): AutonomousReplanEvaluation {
  return {
    ...rewardInput(evaluation),
    replan_requested: evaluation.replan_requested,
    replan_instruction: evaluation.replan_instruction,
  };
}

interface CrossUnit {
  domain: AutonomousDomainName;
  role: AutonomousCycleEvaluatorRole;
}

function crossUnits(run: AutonomousCrossDomainRunResult): CrossUnit[] {
  const units: CrossUnit[] = run.child_runs.map((child) => ({ domain: child.domain, role: "specialist" }));
  if (run.synthesis !== null) units.push({ domain: "cross_domain", role: "synthesis" });
  return units;
}

function aggregateCrossEvaluation(
  evaluation: AutonomousValueEvaluation,
  rewards: Record<string, AutonomousEvaluatorRewardInput>,
): AutonomousCrossDomainReplanEvaluation {
  return {
    evaluator_id: evaluation.evaluator_id,
    evaluator_version: evaluation.evaluator_version,
    reward: evaluation.reward,
    passed: evaluation.passed,
    failed: evaluation.failed,
    feedback_digest: evaluation.feedback_digest,
    failure_class: evaluation.failure_class,
    evidence_digest: evaluation.evidence_digest,
    rewards,
    replan_requested: evaluation.replan_requested,
    replan_instruction: evaluation.replan_instruction,
  };
}

/**
 * Connect the reviewed value-only evaluator registry to all cycle shapes.
 *
 * The returned callbacks are intentionally separate for single and cross-domain cycles so an
 * application cannot accidentally apply a single-domain rubric to a fan-out. Cross-domain
 * evaluator calls use the `cross_domain` rubric for the aggregate decision and the exact routed
 * domain rubric for each pending learning episode. Provider results are represented only by
 * statuses, route metadata, and episode identities in the evidence context.
 */
export function createAutonomousCycleEvaluatorBridge(
  options: AutonomousCycleEvaluatorBridgeOptions,
): AutonomousCycleEvaluatorBridge {
  if (!options || typeof options.evidenceFor !== "function") throw new ArgumentError("cycle evaluator bridge requires an evidenceFor callback");
  const registry = options.registry ?? AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  if (!(registry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("cycle evaluator bridge registry is malformed");
  assertCompleteRegistry(registry);
  const evaluatorCatalogueDigest = catalogueDigest(registry);

  const evaluateSingle = async (run: AutonomousRunResult): Promise<AutonomousValueEvaluation> => {
    if (!run || !run.route) throw new ArgumentError("cycle evaluator bridge received a malformed single-domain run");
    const domain = singleDomain(run);
    const evaluator = registry.resolveForAutonomousDomain(domain);
    const context = await admittedContext(singleContext(run, domain, evaluator), options);
    const evidence = await options.evidenceFor(context);
    return evaluator.assess(evidence);
  };

  const evaluateCross = async (run: AutonomousCrossDomainRunResult): Promise<{ aggregate: AutonomousValueEvaluation; rewards: Record<string, AutonomousEvaluatorRewardInput> }> => {
    if (!run || !run.route || !Array.isArray(run.learning_episode_ids)) throw new ArgumentError("cycle evaluator bridge received a malformed cross-domain run");
    const aggregateEvaluator = registry.resolveForAutonomousDomain("cross_domain");
    const aggregateContext = await admittedContext(crossContext(run, {
      domain: "cross_domain",
      role: "synthesis",
      learningEpisodeId: null,
      evaluator: aggregateEvaluator,
    }), options);
    const aggregateEvidence = await options.evidenceFor(aggregateContext);
    const aggregate = aggregateEvaluator.assess(aggregateEvidence);
    const rewards: Record<string, AutonomousEvaluatorRewardInput> = {};
    const episodeIds = [...run.learning_episode_ids];
    if (episodeIds.length) {
      const units = crossUnits(run);
      if (units.length < episodeIds.length) throw new ArgumentError("cycle evaluator bridge cannot map every cross-domain learning episode to a reviewed unit");
      for (let index = 0; index < episodeIds.length; index += 1) {
        const episodeId = episodeIds[index]!;
        const unit = units[index]!;
        const evaluator = registry.resolveForAutonomousDomain(unit.domain);
        const context = await admittedContext(crossContext(run, {
          domain: unit.domain,
          role: unit.role,
          learningEpisodeId: episodeId,
          evaluator,
        }), options);
        const evidence = await options.evidenceFor(context);
        rewards[episodeId] = rewardInput(evaluator.assess(evidence));
      }
    }
    return { aggregate, rewards };
  };

  return {
    schema: AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    registry,
    evaluator_catalogue_digest: evaluatorCatalogueDigest,
    policy_digest: policyDigest(registry),
    evaluate: async (run) => rewardInput(await evaluateSingle(run)),
    evaluateReplan: async (run) => replanEvaluation(await evaluateSingle(run)),
    evaluateCrossDomain: async (run) => (await evaluateCross(run)).rewards,
    evaluateCrossDomainReplan: async (run) => {
      const result = await evaluateCross(run);
      return aggregateCrossEvaluation(result.aggregate, result.rewards);
    },
  };
}
