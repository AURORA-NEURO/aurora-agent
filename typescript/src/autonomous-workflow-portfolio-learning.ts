import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousValueEvaluatorRegistry,
  type AutonomousValueEvaluationInput,
} from "./autonomous-domain-evaluators.js";
import type { AutonomousEvaluatorRewardInput } from "./autonomous-learning.js";
import type { AutonomousWorkflowPortfolioLearningEvaluationContext } from "./autonomous-workflow-portfolio-execution.js";
import { digestJsonSync } from "./tooling.js";

export const AUTONOMOUS_WORKFLOW_PORTFOLIO_EVALUATOR_BRIDGE_SCHEMA = "bioprism-typescript-autonomous-workflow-portfolio-evaluator-bridge/0.1" as const;

export interface AutonomousWorkflowPortfolioDomainEvidenceContext extends AutonomousWorkflowPortfolioLearningEvaluationContext {
  evaluator_id: string;
  evaluator_version: string;
  required_signals: string[];
  pass_threshold: number;
}

export interface AutonomousWorkflowPortfolioEvaluatorBridgeOptions {
  /** Registry of value-only domain adapters; defaults to all reviewed built-in profiles. */
  registry?: AutonomousValueEvaluatorRegistry;
  /** Caller-owned evidence builder. It must return bounded value-only evidence, not task/output payloads. */
  evidenceFor: (context: AutonomousWorkflowPortfolioDomainEvidenceContext) => AutonomousValueEvaluationInput | Promise<AutonomousValueEvaluationInput>;
}

export interface AutonomousWorkflowPortfolioEvaluatorBridge {
  schema: typeof AUTONOMOUS_WORKFLOW_PORTFOLIO_EVALUATOR_BRIDGE_SCHEMA;
  registry: AutonomousValueEvaluatorRegistry;
  learningPolicyDigest: string;
  evaluateItem: (context: AutonomousWorkflowPortfolioLearningEvaluationContext) => Promise<AutonomousEvaluatorRewardInput>;
}

function assertCompleteRegistry(registry: AutonomousValueEvaluatorRegistry): void {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) registry.resolveForAutonomousDomain(domain);
}

function policyDigest(registry: AutonomousValueEvaluatorRegistry): string {
  return digestJsonSync({
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVALUATOR_BRIDGE_SCHEMA,
    domains: AUTONOMOUS_DOMAIN_NAMES.map((domain) => registry.resolveForAutonomousDomain(domain).catalogueEntry()),
    authority: "caller_declared_signal_scoring_only",
    retention: "evaluator_identity_and_contract_digest_only",
  });
}

/**
 * Build a reusable all-domain evaluator callback for portfolio execution.
 *
 * The bridge owns routing and evaluator identity only. The caller still owns evidence
 * acquisition and must return bounded evidence for the selected domain; no provider response,
 * task text, credential, or raw evidence is copied into the returned reward packet.
 */
export function createAutonomousWorkflowPortfolioEvaluatorBridge(
  options: AutonomousWorkflowPortfolioEvaluatorBridgeOptions,
): AutonomousWorkflowPortfolioEvaluatorBridge {
  if (!options || typeof options.evidenceFor !== "function") throw new ArgumentError("workflow portfolio evaluator bridge requires an evidenceFor callback");
  const registry = options.registry ?? AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  if (!(registry instanceof AutonomousValueEvaluatorRegistry)) throw new ArgumentError("workflow portfolio evaluator bridge registry is malformed");
  assertCompleteRegistry(registry);
  return {
    schema: AUTONOMOUS_WORKFLOW_PORTFOLIO_EVALUATOR_BRIDGE_SCHEMA,
    registry,
    learningPolicyDigest: policyDigest(registry),
    evaluateItem: async (context) => {
      if (!context || !AUTONOMOUS_DOMAIN_NAMES.includes(context.domain as AutonomousDomainName)) throw new ArgumentError("workflow portfolio evaluator bridge received an unsupported item domain");
      const evaluator = registry.resolveForAutonomousDomain(context.domain);
      const profile = evaluator.catalogueEntry();
      const evidenceContext: AutonomousWorkflowPortfolioDomainEvidenceContext = {
        ...context,
        evaluator_id: profile.evaluator_id,
        evaluator_version: profile.evaluator_version,
        required_signals: [...profile.required_signals],
        pass_threshold: profile.pass_threshold,
      };
      const valueInput = await options.evidenceFor(evidenceContext);
      return evaluator.toRewardInput(valueInput);
    },
  };
}
