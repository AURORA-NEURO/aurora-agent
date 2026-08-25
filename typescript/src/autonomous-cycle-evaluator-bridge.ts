import { ArgumentError } from "./errors.js";
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
  retention: "metadata_only;caller_evidence_factory_owns_values";
  secret_material: "never_returned";
}

export type AutonomousCycleEvaluatorEvidenceFactory = (
  context: AutonomousCycleEvaluatorEvidenceContext,
) => AutonomousValueEvaluationInput | Promise<AutonomousValueEvaluationInput>;

export interface AutonomousCycleEvaluatorBridgeOptions {
  /** Registry of reviewed value-only domain evaluators; defaults to every built-in profile. */
  registry?: AutonomousValueEvaluatorRegistry;
  /**
   * Caller-owned evidence builder. It must return bounded value-only evidence for the supplied
   * metadata context. The bridge never copies that evidence into a cycle result or checkpoint.
   */
  evidenceFor: AutonomousCycleEvaluatorEvidenceFactory;
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
    retention: BRIDGE_RETENTION,
    secret_material: "never_returned",
  };
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

  const evaluateSingle = async (run: AutonomousRunResult): Promise<AutonomousValueEvaluation> => {
    if (!run || !run.route) throw new ArgumentError("cycle evaluator bridge received a malformed single-domain run");
    const domain = singleDomain(run);
    const evaluator = registry.resolveForAutonomousDomain(domain);
    const evidence = await options.evidenceFor(singleContext(run, domain, evaluator));
    return evaluator.assess(evidence);
  };

  const evaluateCross = async (run: AutonomousCrossDomainRunResult): Promise<{ aggregate: AutonomousValueEvaluation; rewards: Record<string, AutonomousEvaluatorRewardInput> }> => {
    if (!run || !run.route || !Array.isArray(run.learning_episode_ids)) throw new ArgumentError("cycle evaluator bridge received a malformed cross-domain run");
    const aggregateEvaluator = registry.resolveForAutonomousDomain("cross_domain");
    const aggregateEvidence = await options.evidenceFor(crossContext(run, {
      domain: "cross_domain",
      role: "synthesis",
      learningEpisodeId: null,
      evaluator: aggregateEvaluator,
    }));
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
        const evidence = await options.evidenceFor(crossContext(run, {
          domain: unit.domain,
          role: unit.role,
          learningEpisodeId: episodeId,
          evaluator,
        }));
        rewards[episodeId] = rewardInput(evaluator.assess(evidence));
      }
    }
    return { aggregate, rewards };
  };

  return {
    schema: AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    registry,
    evaluator_catalogue_digest: catalogueDigest(registry),
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
