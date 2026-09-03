/** Public TypeScript contracts for Epistemic P32 evidence-closure cards. */
import {digestJsonSync} from "./tooling.js";

export const EPISTEMIC_EVIDENCE_CLOSURE_CONTENT_TYPE = "application/vnd.aurora.epistemic.evidence-closure-card-1+json" as const;
export const EPISTEMIC_EVIDENCE_CLOSURE_BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions" as const;

export const EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_FEATURE_ID = "AFA-epistemic-P32-F01" as const;
export const EPISTEMIC_MULTIMODAL_EVIDENCE_CLOSURE_FEATURE_ID = "AFA-epistemic-P32-F02" as const;
export const EPISTEMIC_THROUGHPUT_EVIDENCE_CLOSURE_FEATURE_ID = "AFA-epistemic-P32-F03" as const;
export const EPISTEMIC_FEDERATED_EVIDENCE_CLOSURE_FEATURE_ID = "AFA-epistemic-P32-F04" as const;
export const EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_CONTRACT_FEATURE_ID = "AFA-epistemic-P32-F05" as const;
export const EPISTEMIC_MULTIMODAL_EVIDENCE_CLOSURE_CONTRACT_FEATURE_ID = "AFA-epistemic-P32-F06" as const;
export const EPISTEMIC_THROUGHPUT_EVIDENCE_CLOSURE_CONTRACT_FEATURE_ID = "AFA-epistemic-P32-F07" as const;
export const EPISTEMIC_FEDERATED_EVIDENCE_CLOSURE_CONTRACT_FEATURE_ID = "AFA-epistemic-P32-F08" as const;
export const EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_COPILOT_FEATURE_ID = "AFA-epistemic-P32-F09" as const;
export const EPISTEMIC_MULTIMODAL_EVIDENCE_CLOSURE_COPILOT_FEATURE_ID = "AFA-epistemic-P32-F10" as const;
export const EPISTEMIC_THROUGHPUT_EVIDENCE_CLOSURE_COPILOT_FEATURE_ID = "AFA-epistemic-P32-F11" as const;
export const EPISTEMIC_FEDERATED_EVIDENCE_CLOSURE_COPILOT_FEATURE_ID = "AFA-epistemic-P32-F12" as const;
export const EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_WORKFLOW_FEATURE_ID = "AFA-epistemic-P32-F13" as const;
export const EPISTEMIC_MULTIMODAL_EVIDENCE_CLOSURE_WORKFLOW_FEATURE_ID = "AFA-epistemic-P32-F14" as const;
export const EPISTEMIC_THROUGHPUT_EVIDENCE_CLOSURE_WORKFLOW_FEATURE_ID = "AFA-epistemic-P32-F15" as const;
export const EPISTEMIC_FEDERATED_EVIDENCE_CLOSURE_WORKFLOW_FEATURE_ID = "AFA-epistemic-P32-F16" as const;

export interface EpistemicEvidenceClosureCard {
  schema_version: string; contract_version: string; feature_id: string; mode: string; scale: string;
  request_id: string; purpose: string; disposition: "qualified" | "partial" | "unknown" | "blocked";
  assertion_order: string[]; supported_order: string[]; contradicted_order: string[]; unknown_order: string[];
  omitted_order: string[]; source_order: string[]; uncertainty_order: string[]; competing_order: string[];
  negative_evidence_order: string[]; replay_identity: string; closure_digest: string;
  artifact: {artifact_id: string; content_type: string; content_hash: string; semantic_loss: string[]; assertion_digests: string[]; boundary: string};
  effect_receipts: string[]; raw_data_local: true; aggregate_only: true; boundary: string;
}

const ordered = (value: string[]): boolean => JSON.stringify([...new Set(value)].sort()) === JSON.stringify(value);
const digest = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{64}$/.test(value);

function validate(card: EpistemicEvidenceClosureCard, featureId: string): void {
  if (card.schema_version !== "1.0.0" || card.feature_id !== featureId || card.boundary !== EPISTEMIC_EVIDENCE_CLOSURE_BOUNDARY ||
      card.raw_data_local !== true || card.aggregate_only !== true || !card.assertion_order.length || !digest(card.replay_identity) ||
      !digest(card.closure_digest) || card.artifact.content_type !== EPISTEMIC_EVIDENCE_CLOSURE_CONTENT_TYPE ||
      card.artifact.content_hash !== card.closure_digest || card.artifact.boundary !== EPISTEMIC_EVIDENCE_CLOSURE_BOUNDARY) {
    throw new Error("evidence identity, locality, digest, artifact, or boundary is incomplete");
  }
  for (const vector of [card.assertion_order, card.supported_order, card.contradicted_order, card.unknown_order, card.omitted_order,
    card.source_order, card.uncertainty_order, card.competing_order, card.negative_evidence_order, card.effect_receipts]) {
    if (!ordered(vector)) throw new Error("evidence vectors are not canonical");
  }
  const ids = new Set(card.assertion_order);
  const states = new Set([...card.supported_order, ...card.contradicted_order, ...card.unknown_order, ...card.omitted_order]);
  if (ids.size !== states.size || [...ids].some((id) => !states.has(id))) throw new Error("assertion states do not partition");
}

export const validateEpistemicLocalEvidenceClosure = (card: EpistemicEvidenceClosureCard): void => validate(card, EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_FEATURE_ID);
export const validateEpistemicMultimodalEvidenceClosure = (card: EpistemicEvidenceClosureCard): void => validate(card, EPISTEMIC_MULTIMODAL_EVIDENCE_CLOSURE_FEATURE_ID);
export const validateEpistemicThroughputEvidenceClosure = (card: EpistemicEvidenceClosureCard): void => validate(card, EPISTEMIC_THROUGHPUT_EVIDENCE_CLOSURE_FEATURE_ID);
export const validateEpistemicFederatedEvidenceClosure = (card: EpistemicEvidenceClosureCard): void => validate(card, EPISTEMIC_FEDERATED_EVIDENCE_CLOSURE_FEATURE_ID);
export const epistemicEvidenceClosureDigest = (card: EpistemicEvidenceClosureCard): string => { validateEpistemicLocalEvidenceClosure(card); return digestJsonSync(card); };
export const epistemicEvidenceClosureContractDigest = (card: EpistemicEvidenceClosureCard): string => { validate(card, EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_CONTRACT_FEATURE_ID); return digestJsonSync(card); };
export const epistemicEvidenceClosureCopilotDigest = (card: EpistemicEvidenceClosureCard): string => { validate(card, EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_COPILOT_FEATURE_ID); return digestJsonSync(card); };
export const epistemicEvidenceClosureWorkflowDigest = (card: EpistemicEvidenceClosureCard): string => { validate(card, EPISTEMIC_LOCAL_EVIDENCE_CLOSURE_WORKFLOW_FEATURE_ID); return digestJsonSync(card); };
