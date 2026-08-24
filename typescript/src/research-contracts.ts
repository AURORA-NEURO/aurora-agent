/** Shared v1 research contracts for TypeScript adapters and workbench clients.
 *
 * This module intentionally validates transport and safety metadata only. Scientific conclusions
 * remain evidence-receipt values produced by the Rust kernel; a client cannot upgrade `unknown`
 * or bypass a protected omission by editing a JSON object.
 */
import { digestJsonSync } from "./tooling.js";

export const RESEARCH_CONTRACT_SCHEMA_VERSION = "aurora-research-contract/1.0" as const;
export const PRECLINICAL_BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions" as const;
export const RESEARCH_FEATURE_ID = "AFA-bioir-P02-F01" as const;
export const RELEASE_REVIEW_FEATURE_ID = "AFA-evalengine-P13-F01" as const;
export const RESEARCH_INGESTION_FEATURE_ID = "AFA-adapter-P06-F01" as const;
export const EXPERIMENT_DESIGN_FEATURE_ID = "AFA-lab-P09-F01" as const;

export type PolicyDecision = "allow" | "deny" | "redact" | "local_only" | "approval_required" | "unresolved";
export type EvidenceState = "proven" | "supported" | "speculative" | "contradicted" | "unknown";

export interface PolicyReceipt {
  schema_version: string;
  receipt_id: string;
  decision: PolicyDecision;
  reasons: string[];
  evaluated_artifacts: string[];
  authority_reference?: string | null;
  boundary: string;
}

export interface EvidenceOmission {
  item: string;
  reason: string;
  could_change_decision: "no_known_impact" | "potentially_material" | "unknown";
}

export interface EvidenceReceipt {
  schema_version: string;
  receipt_id: string;
  intent: string;
  sources: readonly { source_id: string; source_type: string; locator: string; digest?: string | null; availability: string }[];
  derivation: string[];
  uncertainty: readonly { kind: string; statement: string }[];
  omissions: readonly EvidenceOmission[];
  competing_explanations: readonly unknown[];
  negative_evidence: readonly unknown[];
  conclusion_state: EvidenceState;
  boundary: string;
}

export interface ReleaseReview {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  card_digest: string;
  verdict: "pass" | "conditional" | "blocked" | "not_evaluated";
  reasons: string[];
  replications: readonly Record<string, unknown>[];
  checks: readonly Record<string, unknown>[];
  provenance_complete: boolean;
  boundary: string;
}

export function validateReleaseReview(review: ReleaseReview): void {
  if (review.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (review.feature_id !== RELEASE_REVIEW_FEATURE_ID || !review.capability_id.trim()) throw new Error("release review feature or capability is missing");
  if (review.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!/^[0-9a-f]{64}$/.test(review.card_digest)) throw new Error("release review card digest is not a canonical sha256");
  if (!review.reasons.length) throw new Error("release review requires reasons");
  if (review.verdict === "pass" && !review.provenance_complete) throw new Error("a passing release review requires complete provenance");
}

export function releaseReviewDigest(review: ReleaseReview): string {
  validateReleaseReview(review);
  return digestJsonSync(review);
}

export interface ResearchIngestionBundle {
  schema_version: string;
  feature_id: string;
  source_id: string;
  adapter: string;
  adapter_version: string;
  source_digest: string;
  ingestion_digest: string;
  artifact: Record<string, unknown>;
  conformance: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateResearchIngestionBundle(bundle: ResearchIngestionBundle): void {
  if (bundle.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (bundle.feature_id !== RESEARCH_INGESTION_FEATURE_ID || !bundle.source_id.trim()) throw new Error("research ingestion feature or source is missing");
  if (bundle.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  for (const digest of [bundle.source_digest, bundle.ingestion_digest, bundle.artifact.content_hash]) {
    if (typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)) throw new Error("research ingestion digest is not a canonical sha256");
  }
  if (!bundle.raw_data_local) throw new Error("raw research data must remain local");
  if (bundle.conformance.verified !== true) throw new Error("research ingestion is not conformance verified");
}

export function researchIngestionBundleDigest(bundle: ResearchIngestionBundle): string {
  validateResearchIngestionBundle(bundle);
  return digestJsonSync(bundle);
}

export interface ExperimentDesignPlan {
  payload: Record<string, unknown> & { allocations: readonly { arm_id: string; units: number }[]; total_units: number };
  artifact: Record<string, unknown>;
}

export function validateExperimentDesignPlan(plan: ExperimentDesignPlan): void {
  if (plan.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (plan.payload.feature_id !== EXPERIMENT_DESIGN_FEATURE_ID) throw new Error("experiment design feature mismatch");
  if (plan.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!plan.payload.allocations.length || plan.payload.allocations.reduce((sum, allocation) => sum + allocation.units, 0) !== plan.payload.total_units) throw new Error("experiment design allocation total is inconsistent");
  if (typeof plan.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(plan.artifact.content_hash)) throw new Error("experiment design artifact digest is invalid");
}

export function experimentDesignPlanDigest(plan: ExperimentDesignPlan): string {
  validateExperimentDesignPlan(plan);
  return digestJsonSync(plan);
}

export function validatePolicyReceipt(receipt: PolicyReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (!receipt.receipt_id.trim() || receipt.reasons.length === 0) throw new Error("policy receipt needs an id and reason");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if ((receipt.decision === "approval_required" || receipt.decision === "unresolved") && receipt.authority_reference) throw new Error("authority is premature for unresolved policy");
  if (receipt.decision === "allow" && receipt.reasons.some((reason) => reason === "unresolved")) throw new Error("unresolved policy cannot allow");
}

export function validateEvidenceReceipt(receipt: EvidenceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (!receipt.receipt_id.trim() || !receipt.intent.trim() || receipt.derivation.length === 0) throw new Error("evidence receipt is incomplete");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.sources.length === 0 && (receipt.conclusion_state !== "unknown" || receipt.omissions.length === 0 || receipt.uncertainty.length === 0)) throw new Error("empty evidence must be explicit unknown");
  if (receipt.conclusion_state === "proven" && receipt.omissions.some((omission) => omission.could_change_decision !== "no_known_impact")) throw new Error("protected omission blocks proven conclusion");
}

/** Hashes the same JSON payload that the Rust `TypedResearchArtifact` seals. */
export function researchArtifactDigest(payload: unknown): string {
  return digestJsonSync(payload);
}
