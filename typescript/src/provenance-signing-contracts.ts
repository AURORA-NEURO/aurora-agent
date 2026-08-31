/** Public TypeScript contracts for Worldgen P18 publication/research-object provenance. */
import { digestJsonSync } from "./tooling.js";
import { PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION } from "./research-contracts.js";

export const WORLDGEN_PROVENANCE_SIGNING_CONTENT_TYPE = "application/vnd.aurora.worldgen.provenance-signing-result+json" as const;
export const WORLDGEN_PROVENANCE_SIGNING_CONTRACT_CONTENT_TYPE = "application/vnd.aurora.worldgen.provenance-signing-contract-receipt+json" as const;
export const WORLDGEN_PROVENANCE_SIGNING_COPILOT_CONTENT_TYPE = "application/vnd.aurora.worldgen.provenance-signing-copilot-receipt+json" as const;
export const WORLDGEN_PROVENANCE_SIGNING_WORKFLOW_CONTENT_TYPE = "application/vnd.aurora.worldgen.provenance-signing-workflow-receipt+json" as const;

export const WORLDGEN_LOCAL_PROVENANCE_SIGNING_FEATURE_ID = "AFA-worldgen-P18-F01" as const;
export const WORLDGEN_MULTIMODAL_PROVENANCE_SIGNING_FEATURE_ID = "AFA-worldgen-P18-F02" as const;
export const WORLDGEN_THROUGHPUT_PROVENANCE_SIGNING_FEATURE_ID = "AFA-worldgen-P18-F03" as const;
export const WORLDGEN_FEDERATED_PROVENANCE_SIGNING_FEATURE_ID = "AFA-worldgen-P18-F04" as const;
export const WORLDGEN_LOCAL_PROVENANCE_SIGNING_CONTRACT_FEATURE_ID = "AFA-worldgen-P18-F05" as const;
export const WORLDGEN_MULTIMODAL_PROVENANCE_SIGNING_CONTRACT_FEATURE_ID = "AFA-worldgen-P18-F06" as const;
export const WORLDGEN_THROUGHPUT_PROVENANCE_SIGNING_CONTRACT_FEATURE_ID = "AFA-worldgen-P18-F07" as const;
export const WORLDGEN_FEDERATED_PROVENANCE_SIGNING_CONTRACT_FEATURE_ID = "AFA-worldgen-P18-F08" as const;
export const WORLDGEN_LOCAL_PROVENANCE_SIGNING_COPILOT_FEATURE_ID = "AFA-worldgen-P18-F09" as const;
export const WORLDGEN_MULTIMODAL_PROVENANCE_SIGNING_COPILOT_FEATURE_ID = "AFA-worldgen-P18-F10" as const;
export const WORLDGEN_THROUGHPUT_PROVENANCE_SIGNING_COPILOT_FEATURE_ID = "AFA-worldgen-P18-F11" as const;
export const WORLDGEN_FEDERATED_PROVENANCE_SIGNING_COPILOT_FEATURE_ID = "AFA-worldgen-P18-F12" as const;
export const WORLDGEN_LOCAL_PROVENANCE_SIGNING_WORKFLOW_FEATURE_ID = "AFA-worldgen-P18-F13" as const;
export const WORLDGEN_MULTIMODAL_PROVENANCE_SIGNING_WORKFLOW_FEATURE_ID = "AFA-worldgen-P18-F14" as const;
export const WORLDGEN_THROUGHPUT_PROVENANCE_SIGNING_WORKFLOW_FEATURE_ID = "AFA-worldgen-P18-F15" as const;
export const WORLDGEN_FEDERATED_PROVENANCE_SIGNING_WORKFLOW_FEATURE_ID = "AFA-worldgen-P18-F16" as const;

export interface WorldgenProvenanceSigningResult {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; study_id: string; scope: string;
  disposition: "qualified" | "unresolved" | "blocked"; candidate_order: string[]; selected_order: string[]; unresolved_order: string[]; blocked_order: string[]; omitted_order: string[];
  lineage_score_milli: number[]; uncertainty_milli: number[]; derivation_order: string[]; signature_scope_order: string[]; decisions: Record<string, unknown>[];
  replay_identity: string; provenance_digest: string; semantic_loss: Record<string, unknown>[]; omissions: string[]; uncertainty: string[]; contradiction: string[]; negative_evidence: string[];
  signer_id: string; signature_algorithm: string; signature_digest: string;
  artifact: { artifact_id: string; content_type: string; content_hash: string; semantic_loss: Record<string, unknown>[]; provenance: Record<string, unknown>[]; boundary: string };
  effect_receipts: string[]; raw_data_local: true; federation_export: "aggregate-digest-only"; boundary: string;
}
export interface WorldgenProvenanceSigningContractReceipt { schema_version: string; contract_version: string; feature_id: string; request_id: string; consumer: string; producer: string; namespace: string; semantic_profile: string; negotiated_version: string; compatibility: string; disposition: string; field_order: string[]; retained_field_order: string[]; missing_field_order: string[]; omitted_field_order: string[]; semantic_loss_order: string[]; replay_identity: string; contract_digest: string; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }
export interface WorldgenProvenanceSigningCopilotReceipt { schema_version: string; contract_version: string; feature_id: string; request_id: string; disposition: string; action_order: string[]; admitted_action_order: string[]; denied_action_order: string[]; provenance_disposition: string; provenance_digest: string; copilot_digest: string; replay_identity: string; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }
export interface WorldgenProvenanceSigningWorkflowReceipt { schema_version: string; contract_version: string; feature_id: string; workflow_id: string; disposition: string; stage_order: string[]; completed_stage_order: string[]; pending_stage_order: string[]; compensation_order: string[]; checkpoint_seq: number; budget_units: number; consumed_units: number; replay_identity: string; workflow_digest: string; copilot: Record<string, unknown>; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }

function ordered(v: string[]): boolean { return JSON.stringify([...new Set(v)].sort()) === JSON.stringify(v); }
function validateResult(r: WorldgenProvenanceSigningResult, featureId: string): void {
  const ids = new Set(r.candidate_order); const parts = [...r.selected_order, ...r.unresolved_order, ...r.blocked_order];
  if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.feature_id !== featureId || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PROVENANCE_SIGNING_CONTENT_TYPE || r.raw_data_local !== true || r.federation_export !== "aggregate-digest-only" || !r.candidate_order.length || parts.length !== ids.size || new Set(parts).size !== parts.length || parts.some((v) => !ids.has(v)) || r.artifact.content_hash !== r.provenance_digest || r.lineage_score_milli.length !== r.selected_order.length || r.uncertainty_milli.length !== r.selected_order.length || !r.signer_id.trim() || r.signature_algorithm !== "ed25519-content-digest-v1" || !/^[0-9a-f]{64}$/.test(r.signature_digest)) throw new Error("provenance-signing result identity, signature, states, locality, or digest is invalid");
  for (const values of [r.candidate_order, r.selected_order, r.unresolved_order, r.blocked_order, r.omitted_order, r.derivation_order, r.signature_scope_order, r.omissions, r.uncertainty, r.contradiction, r.negative_evidence, r.effect_receipts]) if (!ordered(values)) throw new Error("provenance-signing vectors are not canonical");
}
export function validateWorldgenLocalProvenanceSigningResult(r: WorldgenProvenanceSigningResult): void { validateResult(r, WORLDGEN_LOCAL_PROVENANCE_SIGNING_FEATURE_ID); }
export function validateWorldgenMultimodalProvenanceSigningResult(r: WorldgenProvenanceSigningResult): void { validateResult(r, WORLDGEN_MULTIMODAL_PROVENANCE_SIGNING_FEATURE_ID); }
export function validateWorldgenThroughputProvenanceSigningResult(r: WorldgenProvenanceSigningResult): void { validateResult(r, WORLDGEN_THROUGHPUT_PROVENANCE_SIGNING_FEATURE_ID); }
export function validateWorldgenFederatedProvenanceSigningResult(r: WorldgenProvenanceSigningResult): void { validateResult(r, WORLDGEN_FEDERATED_PROVENANCE_SIGNING_FEATURE_ID); }
export function validateWorldgenProvenanceSigningContractReceipt(r: WorldgenProvenanceSigningContractReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PROVENANCE_SIGNING_CONTRACT_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.contract_digest) throw new Error("provenance-signing contract identity or digest is invalid"); }
export function validateWorldgenProvenanceSigningCopilotReceipt(r: WorldgenProvenanceSigningCopilotReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PROVENANCE_SIGNING_COPILOT_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.copilot_digest) throw new Error("provenance-signing copilot identity or digest is invalid"); }
export function validateWorldgenProvenanceSigningWorkflowReceipt(r: WorldgenProvenanceSigningWorkflowReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PROVENANCE_SIGNING_WORKFLOW_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.workflow_digest) throw new Error("provenance-signing workflow identity or digest is invalid"); }
export function worldgenProvenanceSigningDigest(r: WorldgenProvenanceSigningResult): string { validateWorldgenLocalProvenanceSigningResult(r); return digestJsonSync(r); }
export function worldgenProvenanceSigningContractDigest(r: WorldgenProvenanceSigningContractReceipt): string { validateWorldgenProvenanceSigningContractReceipt(r); return digestJsonSync(r); }
export function worldgenProvenanceSigningCopilotDigest(r: WorldgenProvenanceSigningCopilotReceipt): string { validateWorldgenProvenanceSigningCopilotReceipt(r); return digestJsonSync(r); }
export function worldgenProvenanceSigningWorkflowDigest(r: WorldgenProvenanceSigningWorkflowReceipt): string { validateWorldgenProvenanceSigningWorkflowReceipt(r); return digestJsonSync(r); }
