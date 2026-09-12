import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import type {
  AutonomousDomainResponse,
  AutonomousDomainResponseContract,
  AutonomousDomainStageResponse,
} from "./autonomous-domain-response.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * Reviewed, domain-specific quality policy used by the structured response evaluator.
 *
 * The response schema intentionally stays stable and provider-neutral.  These policies are the
 * domain operating knowledge that makes that stable schema useful: they tell the prompt what a
 * responsible answer must cover and tell the evaluator which omissions should trigger a replan.
 * They are structural controls, not a source of external truth and never authorize effects.
 */
export const AUTONOMOUS_DOMAIN_QUALITY_POLICY_SCHEMA = "bioprism-typescript-autonomous-domain-quality-policy/0.1" as const;
export const AUTONOMOUS_DOMAIN_QUALITY_POLICY_VERSION = "1" as const;
export const AUTONOMOUS_DOMAIN_QUALITY_REPORT_SCHEMA = "bioprism-typescript-autonomous-domain-quality-report/0.1" as const;
export const AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD = 0.8;
export const MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTIONS = 12;
export const MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTION_BYTES = 2_048;

const AUTONOMOUS_DOMAIN_NAMES = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"] as const;

export type AutonomousDomainQualityStageRequirement = Readonly<{
  evidence: boolean;
  findings: boolean;
  uncertainty: boolean;
  open_questions: boolean;
}>;

export interface AutonomousDomainQualityPolicy extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_QUALITY_POLICY_SCHEMA;
  version: typeof AUTONOMOUS_DOMAIN_QUALITY_POLICY_VERSION;
  domain: AutonomousDomainName;
  required_detail_fields: string[];
  critical_detail_fields: string[];
  safety_detail_fields: string[];
  required_top_level_sections: Array<"observations" | "inferences" | "uncertainty" | "evidence_gaps" | "next_actions">;
  stage_requirements: Record<"complete" | "partial" | "blocked" | "not_attempted", AutonomousDomainQualityStageRequirement>;
  prompt_instructions: string[];
  policy_digest: string;
  retention: "policy_metadata_only;does_not_establish_external_truth";
  secret_material: "never_returned";
}

export interface AutonomousDomainQualityReport extends JsonObject {
  schema: typeof AUTONOMOUS_DOMAIN_QUALITY_REPORT_SCHEMA;
  domain: AutonomousDomainName;
  policy_digest: string;
  signals: Record<string, number>;
  weights: Record<string, number>;
  missing_signals: string[];
  recommendations: string[];
  score: number;
  passed: boolean;
  authority: "structural_domain_quality_only;not_external_truth";
  retention: "value_only;response_payload_not_retained";
  secret_material: "never_returned";
  report_digest: string;
}

const STAGE_REQUIREMENTS: Record<"complete" | "partial" | "blocked" | "not_attempted", AutonomousDomainQualityStageRequirement> = {
  complete: { evidence: true, findings: true, uncertainty: false, open_questions: false },
  partial: { evidence: true, findings: true, uncertainty: true, open_questions: true },
  blocked: { evidence: false, findings: false, uncertainty: true, open_questions: true },
  not_attempted: { evidence: false, findings: false, uncertainty: true, open_questions: true },
};

const DOMAIN_QUALITY_SEEDS: Readonly<Record<AutonomousDomainName, {
  critical: readonly string[];
  safety: readonly string[];
  instructions: readonly string[];
}>> = {
  coding: {
    critical: ["files_or_components", "tests_and_verification", "residual_risks", "rollback_or_follow_up"],
    safety: ["residual_risks", "rollback_or_follow_up"],
    instructions: [
      "Name the exact files, modules, interfaces, or deployment units that are in scope; distinguish inspected artifacts from proposed edits.",
      "Report verification as executable checks with their observed result, not as a claim that code is correct because it was generated.",
      "Call out compatibility, security, migration, and operational risks, including the smallest safe rollback or follow-up action.",
      "Keep implementation, test evidence, and remaining work separate so a reviewer can reproduce the decision.",
    ],
  },
  browser: {
    critical: ["sources", "citations", "freshness", "retrieval_gaps"],
    safety: ["freshness", "retrieval_gaps"],
    instructions: [
      "Identify each source and its retrieval boundary; do not turn a search result, snippet, or unvisited link into verified evidence.",
      "Attach claims to citations and state publication or retrieval freshness whenever time can change the answer.",
      "Separate source-reported observations from synthesis and disclose inaccessible, conflicting, or missing sources.",
      "Never imply that browsing performed an external action unless a caller-owned effect receipt explicitly proves that action.",
    ],
  },
  data: {
    critical: ["schema_and_units", "lineage", "quality_metrics", "anomalies_and_transformations"],
    safety: ["quality_metrics", "anomalies_and_transformations"],
    instructions: [
      "State grain, schema, units, null semantics, time basis, and population before interpreting a metric or transformation.",
      "Trace important values to their input and transformation lineage; distinguish observed measurements from calculated estimates.",
      "Report quality metrics, missingness, outliers, leakage, and anomalies before presenting a conclusion.",
      "Make transformations reproducible and identify any irreversible or lossy operation that needs caller approval.",
    ],
  },
  science: {
    critical: ["estimand_and_assumptions", "evidence_map", "hypotheses_and_predictions", "design_and_controls", "reproduction_plan"],
    safety: ["estimand_and_assumptions", "design_and_controls", "reproduction_plan"],
    instructions: [
      "Define the estimand, population, assumptions, and decision criterion before describing a result as supporting a hypothesis.",
      "Map evidence to claims and distinguish prior literature, supplied observations, model output, and speculation.",
      "State falsifiable predictions, controls, confounds, and the smallest reproduction or sensitivity check that could change the conclusion.",
      "Do not convert an association, simulation, or proposed experiment into a causal or externally validated finding.",
    ],
  },
  biomedical: {
    critical: ["scope_boundary", "provenance", "population_and_applicability", "neurosurgical_route", "molecular_assay_coverage", "uncertainty", "human_review_and_escalation"],
    safety: ["scope_boundary", "neurosurgical_route", "molecular_assay_coverage", "uncertainty", "human_review_and_escalation"],
    instructions: [
      "State the clinical or biological scope and explicitly separate educational analysis from diagnosis, treatment, or patient-specific advice.",
      "Track provenance, cohort, sample limitations, applicability, and uncertainty for every clinically meaningful claim.",
      "Escalate decisions requiring licensed, ethical, institutional, or patient-specific review; do not silently fill missing clinical context.",
      "Treat generated text, literature summaries, and model outputs as reviewable evidence projections, never as medical authorization.",
    ],
  },
  neuroscience: {
    critical: ["measurement_contract", "preprocessing_and_exclusions", "neurosurgical_route", "molecular_assay_coverage", "confounds", "model_sensitivity", "validation_plan"],
    safety: ["preprocessing_and_exclusions", "neurosurgical_route", "molecular_assay_coverage", "confounds", "model_sensitivity", "validation_plan"],
    instructions: [
      "Define the signal, sampling, cohort, task, units, and measurement validity before interpreting a neural effect.",
      "Make preprocessing, exclusions, artifact handling, leakage controls, and multiple-comparison choices explicit.",
      "Report confounds and model sensitivity, including whether the conclusion survives reasonable preprocessing or specification changes.",
      "Provide a validation and reproduction plan; do not equate decoded, simulated, or correlated activity with mechanism or subjective experience.",
    ],
  },
  operations: {
    critical: ["observed_state", "blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"],
    safety: ["blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"],
    instructions: [
      "Describe the observed state, scope, dependencies, and blast radius before proposing a change or remediation.",
      "Define measurable stop conditions, rollback or recovery steps, and the owner who can approve an effect.",
      "Keep simulation, recommendation, dry-run, and dispatched effect separate; an agent response never proves an operational change occurred.",
      "Surface approval requirements and unknowns before irreversible, customer-facing, security-sensitive, or high-blast-radius work.",
    ],
  },
  enterprise: {
    critical: ["stakeholders_and_owners", "policy_constraints", "options_and_tradeoffs", "decision_and_approver", "audit_plan"],
    safety: ["policy_constraints", "decision_and_approver", "audit_plan"],
    instructions: [
      "Name stakeholders, accountable owners, decision rights, and impacted systems rather than treating an organization as a single actor.",
      "State applicable policy, compliance, contractual, privacy, and segregation-of-duties constraints before ranking options.",
      "Compare alternatives with explicit tradeoffs, reversibility, cost, risk, and evidence; record who must approve the decision.",
      "Define the audit trail and follow-up measurement needed to verify adoption without claiming organizational effect prematurely.",
    ],
  },
  multi_agent: {
    critical: ["subtasks_and_interfaces", "assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"],
    safety: ["assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"],
    instructions: [
      "Decompose work into bounded subtasks with typed inputs, outputs, budgets, dependencies, and a clear completion condition.",
      "Record agent or worker assignments and reconcile outputs by digest, provenance, and contract rather than majority vote alone.",
      "Preserve dissent, conflicts, and missing outputs; do not hide disagreement in a synthesized narrative.",
      "Name the accountable authority for approval and final decisions; delegation does not transfer responsibility to an unreviewed worker.",
    ],
  },
  multimodal: {
    critical: ["available_modalities", "modality_observations", "alignment", "missing_modalities", "blind_spots"],
    safety: ["alignment", "missing_modalities", "blind_spots"],
    instructions: [
      "Inventory available modalities, resolution, timestamps, provenance, and missing inputs before making a cross-modal claim.",
      "Keep modality-specific observations separate and describe how records, coordinates, identities, or time windows were aligned.",
      "Call out blind spots, unobserved modalities, quality differences, and contradictory signals instead of averaging them away.",
      "Do not infer a real-world event from a generated or weakly aligned modality without an explicit validation boundary.",
    ],
  },
  cross_domain: {
    critical: ["domain_attributions", "terminology_and_units", "disagreements", "decision_gate", "open_questions"],
    safety: ["terminology_and_units", "disagreements", "decision_gate", "open_questions"],
    instructions: [
      "Attribute each material claim to its contributing domain and preserve domain-specific assumptions, units, and terminology.",
      "Reconcile disagreements explicitly, including incompatible evidence, time bases, populations, or definitions.",
      "State the decision gate, authority, and evidence required before a cross-domain recommendation can advance.",
      "Keep unresolved questions visible; synthesis is not permission to erase uncertainty or claim a domain has validated another.",
    ],
  },
  evaluation: {
    critical: ["rubric_and_pass_criteria", "cases_and_coverage", "replay_outcomes", "failures_and_regressions", "reproduction_and_next_learning"],
    safety: ["rubric_and_pass_criteria", "failures_and_regressions", "reproduction_and_next_learning"],
    instructions: [
      "Define measurable pass criteria, evaluator authority, and the boundary between structural checks and external correctness.",
      "Report case coverage, representative failures, regressions, flaky behavior, and replay determinism rather than only aggregate reward.",
      "Preserve failure identity and reproduction steps so the next learning or bandit update can be audited.",
      "Never treat a high score from an incomplete test set as proof that the underlying agent, model, or external task is correct.",
    ],
  },
};

function boundedInstruction(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || new TextEncoder().encode(value).byteLength > MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTION_BYTES) throw new ArgumentError("domain quality instruction is malformed");
  return value;
}

function buildPolicy(domain: AutonomousDomainName): AutonomousDomainQualityPolicy {
  const fields = {
    coding: ["files_or_components", "tests_and_verification", "residual_risks", "rollback_or_follow_up"],
    browser: ["sources", "citations", "freshness", "retrieval_gaps"],
    data: ["schema_and_units", "lineage", "quality_metrics", "anomalies_and_transformations"],
    science: ["estimand_and_assumptions", "evidence_map", "hypotheses_and_predictions", "design_and_controls", "reproduction_plan"],
    biomedical: ["scope_boundary", "provenance", "population_and_applicability", "neurosurgical_route", "molecular_assay_coverage", "uncertainty", "human_review_and_escalation"],
    neuroscience: ["measurement_contract", "preprocessing_and_exclusions", "neurosurgical_route", "molecular_assay_coverage", "confounds", "model_sensitivity", "validation_plan"],
    operations: ["observed_state", "blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"],
    enterprise: ["stakeholders_and_owners", "policy_constraints", "options_and_tradeoffs", "decision_and_approver", "audit_plan"],
    multi_agent: ["subtasks_and_interfaces", "assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"],
    multimodal: ["available_modalities", "modality_observations", "alignment", "missing_modalities", "blind_spots"],
    cross_domain: ["domain_attributions", "terminology_and_units", "disagreements", "decision_gate", "open_questions"],
    evaluation: ["rubric_and_pass_criteria", "cases_and_coverage", "replay_outcomes", "failures_and_regressions", "reproduction_and_next_learning"],
  } as const;
  const seed = DOMAIN_QUALITY_SEEDS[domain];
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_QUALITY_POLICY_SCHEMA,
    version: AUTONOMOUS_DOMAIN_QUALITY_POLICY_VERSION,
    domain,
    required_detail_fields: [...fields[domain]],
    critical_detail_fields: [...seed.critical],
    safety_detail_fields: [...seed.safety],
    required_top_level_sections: ["observations", "inferences", "uncertainty", "evidence_gaps", "next_actions"] as Array<"observations" | "inferences" | "uncertainty" | "evidence_gaps" | "next_actions">,
    stage_requirements: STAGE_REQUIREMENTS,
    prompt_instructions: [...seed.instructions],
    retention: "policy_metadata_only;does_not_establish_external_truth" as const,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, policy_digest: digestJsonSync(descriptor) };
}

const POLICY_CACHE = new Map<AutonomousDomainName, AutonomousDomainQualityPolicy>();

/** Return the reviewed quality policy for one built-in domain. */
export function autonomousDomainQualityPolicy(domain: AutonomousDomainName): AutonomousDomainQualityPolicy {
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError(`unsupported autonomous domain quality policy: ${domain}`);
  const existing = POLICY_CACHE.get(domain);
  if (existing) return structuredClone(existing);
  const policy = buildPolicy(domain);
  POLICY_CACHE.set(domain, policy);
  return structuredClone(policy);
}

/** Return all policies in canonical domain order for registry audits and parity tests. */
export function builtinAutonomousDomainQualityPolicies(): AutonomousDomainQualityPolicy[] {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain) => autonomousDomainQualityPolicy(domain));
}

/** Verify a policy's shape and digest before a caller uses it as an evaluator input. */
export function validateAutonomousDomainQualityPolicy(value: unknown): AutonomousDomainQualityPolicy {
  if (!isObject(value)) throw new ArgumentError("domain quality policy must be an object");
  const domain = value.domain;
  if (typeof domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("domain quality policy domain is invalid");
  const current = autonomousDomainQualityPolicy(domain as AutonomousDomainName);
  const { policy_digest: suppliedDigest, ...descriptor } = value;
  if (suppliedDigest !== current.policy_digest || digestJsonSync(descriptor) !== suppliedDigest) throw new ArgumentError("domain quality policy is stale or tampered");
  return current;
}

function fraction(total: number, satisfied: number): number {
  return total <= 0 ? 0 : Number(Math.max(0, Math.min(1, satisfied / total)).toFixed(12));
}

function hasValues(value: unknown): boolean {
  return Array.isArray(value) && value.length > 0;
}

function stageQuality(stage: AutonomousDomainStageResponse, policy: AutonomousDomainQualityPolicy): number {
  const requirements = policy.stage_requirements[stage.status];
  const checks = [
    !requirements.evidence || hasValues(stage.evidence),
    !requirements.findings || hasValues(stage.findings),
    !requirements.uncertainty || hasValues(stage.uncertainty),
    !requirements.open_questions || hasValues(stage.open_questions),
  ];
  return fraction(checks.length, checks.filter(Boolean).length);
}

/**
 * Evaluate domain-specific quality without retaining the response body.  The report is designed
 * to be merged into the general response reward so model/prompt/bandit learning receives useful
 * domain feedback while remaining explicitly non-authoritative about the outside world.
 */
export function evaluateAutonomousDomainResponseQuality(
  response: AutonomousDomainResponse,
  contract: AutonomousDomainResponseContract,
  suppliedPolicy?: AutonomousDomainQualityPolicy,
): AutonomousDomainQualityReport {
  if (!response || !contract || response.domain !== contract.domain) throw new ArgumentError("domain quality evaluation identity is malformed");
  const policy = suppliedPolicy ? validateAutonomousDomainQualityPolicy(suppliedPolicy) : autonomousDomainQualityPolicy(response.domain);
  const statuses = response.stages.map((stage) => stage.status);
  const allComplete = statuses.length > 0 && statuses.every((status) => status === "complete");
  const incomplete = statuses.some((status) => status !== "complete");
  const hasBlocked = statuses.includes("blocked");
  const disclosures = hasValues(response.uncertainty) || hasValues(response.evidence_gaps) || hasValues(response.next_actions);
  const statusCoherent = response.status === "complete"
    ? allComplete
    : response.status === "partial"
      ? incomplete && disclosures
      : response.status === "blocked"
        ? hasBlocked && hasValues(response.next_actions)
        : disclosures && incomplete;
  const stageCoverage = response.stages.map((stage) => stageQuality(stage, policy));
  const criticalCoverage = policy.critical_detail_fields.map((field) => hasValues(response.domain_details[field]));
  const safetyCoverage = policy.safety_detail_fields.map((field) => hasValues(response.domain_details[field]));
  const signals: Record<string, number> = {
    quality_status_coherence: statusCoherent ? 1 : 0,
    quality_stage_contract_coverage: fraction(stageCoverage.length, stageCoverage.reduce((sum, score) => sum + score, 0)),
    quality_critical_detail_coverage: fraction(criticalCoverage.length, criticalCoverage.filter(Boolean).length),
    quality_safety_control_coverage: fraction(safetyCoverage.length, safetyCoverage.filter(Boolean).length),
    quality_reasoning_trace: hasValues(response.observations) && hasValues(response.inferences) ? 1 : 0,
    quality_actionability: hasValues(response.next_actions) ? 1 : 0,
    quality_evidence_boundary: hasValues(response.uncertainty) && hasValues(response.evidence_gaps) ? 1 : 0,
  };
  const weights: Record<string, number> = {
    quality_status_coherence: 2.5,
    quality_stage_contract_coverage: 2.5,
    quality_critical_detail_coverage: 2,
    quality_safety_control_coverage: 2,
    quality_reasoning_trace: 1.5,
    quality_actionability: 1.5,
    quality_evidence_boundary: 1.5,
  };
  const totalWeight = Object.values(weights).reduce((sum, weight) => sum + weight, 0);
  const score = Number((Object.entries(weights).reduce((sum, [signal, weight]) => sum + (signals[signal] ?? 0) * weight, 0) / totalWeight).toFixed(12));
  const missingSignals = Object.entries(signals).filter(([, scoreValue]) => scoreValue < 1).map(([signal]) => signal);
  const recommendations = missingSignals.map((signal) => {
    const map: Record<string, string> = {
      quality_status_coherence: "align the top-level status with every stage status and disclose why incomplete work remains",
      quality_stage_contract_coverage: "complete each stage's evidence/findings or explicitly record uncertainty and open questions",
      quality_critical_detail_coverage: `populate every critical ${response.domain} decision field: ${policy.critical_detail_fields.join(", ")}`,
      quality_safety_control_coverage: `address ${response.domain} safety controls: ${policy.safety_detail_fields.join(", ")}`,
      quality_reasoning_trace: "separate observed facts from bounded inferences",
      quality_actionability: "provide caller-reviewable next actions",
      quality_evidence_boundary: "state evidence gaps and uncertainty explicitly",
    };
    return map[signal] ?? `repair ${signal}`;
  });
  const descriptor = {
    schema: AUTONOMOUS_DOMAIN_QUALITY_REPORT_SCHEMA,
    domain: response.domain,
    policy_digest: policy.policy_digest,
    signals,
    weights,
    missing_signals: missingSignals,
    recommendations,
    score,
    // A high aggregate score cannot hide one missing safety or stage-control signal.  The
    // quality report is a readiness gate, so every domain control must be satisfied as well.
    passed: score >= AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD && missingSignals.length === 0,
    authority: "structural_domain_quality_only;not_external_truth" as const,
    retention: "value_only;response_payload_not_retained" as const,
    secret_material: "never_returned" as const,
  };
  return { ...descriptor, report_digest: digestJsonSync(descriptor) };
}

/** Render the reviewed quality policy into a bounded provider prompt fragment. */
export function autonomousDomainQualityPrompt(policy: AutonomousDomainQualityPolicy): string {
  const normalized = validateAutonomousDomainQualityPolicy(policy);
  return [
    `Apply quality policy ${normalized.policy_digest} for ${normalized.domain}.`,
    ...normalized.prompt_instructions,
    `Required top-level sections: ${normalized.required_top_level_sections.join(", ")}.`,
    "A quality pass is a structural readiness signal only; it is not external validation or permission to create an effect.",
  ].join(" ");
}

export function assertAutonomousDomainQualityPolicyCoverage(): true {
  const policies = builtinAutonomousDomainQualityPolicies();
  if (policies.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("domain quality policy registry is incomplete");
  for (const policy of policies) {
    const fields = new Set(policy.required_detail_fields);
    if (policy.critical_detail_fields.some((field) => !fields.has(field)) || policy.safety_detail_fields.some((field) => !fields.has(field))) throw new ArgumentError(`domain quality policy ${policy.domain} references an unknown detail field`);
    if (policy.prompt_instructions.length === 0 || policy.prompt_instructions.length > MAX_AUTONOMOUS_DOMAIN_QUALITY_INSTRUCTIONS) throw new ArgumentError(`domain quality policy ${policy.domain} has invalid prompt guidance`);
  }
  return true;
}
