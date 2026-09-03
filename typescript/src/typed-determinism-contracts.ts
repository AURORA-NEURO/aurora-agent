/** Public TypeScript contracts for Worldgen P17 publication/research-object determinism. */
import { digestJsonSync } from "./tooling.js";
import { PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION } from "./research-contracts.js";

export const WORLDGEN_TYPED_DETERMINISM_CONTENT_TYPE = "application/vnd.aurora.worldgen.typed-determinism-result+json" as const;
export const WORLDGEN_TYPED_DETERMINISM_CONTRACT_CONTENT_TYPE = "application/vnd.aurora.worldgen.typed-determinism-contract-receipt+json" as const;
export const WORLDGEN_TYPED_DETERMINISM_COPILOT_CONTENT_TYPE = "application/vnd.aurora.worldgen.typed-determinism-copilot-receipt+json" as const;
export const WORLDGEN_TYPED_DETERMINISM_WORKFLOW_CONTENT_TYPE = "application/vnd.aurora.worldgen.typed-determinism-workflow-receipt+json" as const;

export const WORLDGEN_LOCAL_TYPED_DETERMINISM_FEATURE_ID = "AFA-worldgen-P17-F01" as const;
export const WORLDGEN_MULTIMODAL_TYPED_DETERMINISM_FEATURE_ID = "AFA-worldgen-P17-F02" as const;
export const WORLDGEN_THROUGHPUT_TYPED_DETERMINISM_FEATURE_ID = "AFA-worldgen-P17-F03" as const;
export const WORLDGEN_FEDERATED_TYPED_DETERMINISM_FEATURE_ID = "AFA-worldgen-P17-F04" as const;
export const WORLDGEN_LOCAL_TYPED_DETERMINISM_CONTRACT_FEATURE_ID = "AFA-worldgen-P17-F05" as const;
export const WORLDGEN_MULTIMODAL_TYPED_DETERMINISM_CONTRACT_FEATURE_ID = "AFA-worldgen-P17-F06" as const;
export const WORLDGEN_THROUGHPUT_TYPED_DETERMINISM_CONTRACT_FEATURE_ID = "AFA-worldgen-P17-F07" as const;
export const WORLDGEN_FEDERATED_TYPED_DETERMINISM_CONTRACT_FEATURE_ID = "AFA-worldgen-P17-F08" as const;
export const WORLDGEN_LOCAL_TYPED_DETERMINISM_COPILOT_FEATURE_ID = "AFA-worldgen-P17-F09" as const;
export const WORLDGEN_MULTIMODAL_TYPED_DETERMINISM_COPILOT_FEATURE_ID = "AFA-worldgen-P17-F10" as const;
export const WORLDGEN_THROUGHPUT_TYPED_DETERMINISM_COPILOT_FEATURE_ID = "AFA-worldgen-P17-F11" as const;
export const WORLDGEN_FEDERATED_TYPED_DETERMINISM_COPILOT_FEATURE_ID = "AFA-worldgen-P17-F12" as const;
export const WORLDGEN_LOCAL_TYPED_DETERMINISM_WORKFLOW_FEATURE_ID = "AFA-worldgen-P17-F13" as const;
export const WORLDGEN_MULTIMODAL_TYPED_DETERMINISM_WORKFLOW_FEATURE_ID = "AFA-worldgen-P17-F14" as const;
export const WORLDGEN_THROUGHPUT_TYPED_DETERMINISM_WORKFLOW_FEATURE_ID = "AFA-worldgen-P17-F15" as const;
export const WORLDGEN_FEDERATED_TYPED_DETERMINISM_WORKFLOW_FEATURE_ID = "AFA-worldgen-P17-F16" as const;

export interface WorldgenTypedDeterminismResult {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; study_id: string; scope: string;
  disposition: "qualified" | "unresolved" | "blocked"; candidate_order: string[]; selected_order: string[]; unresolved_order: string[]; blocked_order: string[]; omitted_order: string[];
  parity_score_milli: number[]; uncertainty_milli: number[]; canonical_field_order: string[]; canonical_effect_order: string[]; decisions: Record<string, unknown>[];
  replay_identity: string; canonical_digest: string; semantic_loss: Record<string, unknown>[]; omissions: string[]; uncertainty: string[]; contradiction: string[]; negative_evidence: string[];
  artifact: { artifact_id: string; content_type: string; content_hash: string; semantic_loss: Record<string, unknown>[]; provenance: Record<string, unknown>[]; boundary: string };
  effect_receipts: string[]; raw_data_local: true; federation_export: "aggregate-digest-only"; boundary: string;
}
export interface WorldgenTypedDeterminismContractReceipt { schema_version: string; contract_version: string; feature_id: string; request_id: string; consumer: string; producer: string; namespace: string; semantic_profile: string; negotiated_version: string; compatibility: string; disposition: string; field_order: string[]; retained_field_order: string[]; missing_field_order: string[]; omitted_field_order: string[]; semantic_loss_order: string[]; replay_identity: string; contract_digest: string; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }
export interface WorldgenTypedDeterminismCopilotReceipt { schema_version: string; contract_version: string; feature_id: string; request_id: string; disposition: string; action_order: string[]; admitted_action_order: string[]; denied_action_order: string[]; determinism_disposition: string; canonical_digest: string; copilot_digest: string; replay_identity: string; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }
export interface WorldgenTypedDeterminismWorkflowReceipt { schema_version: string; contract_version: string; feature_id: string; workflow_id: string; disposition: string; stage_order: string[]; completed_stage_order: string[]; pending_stage_order: string[]; compensation_order: string[]; checkpoint_seq: number; budget_units: number; consumed_units: number; replay_identity: string; workflow_digest: string; copilot: Record<string, unknown>; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }

function ordered(v: string[]): boolean { return JSON.stringify([...new Set(v)].sort()) === JSON.stringify(v); }
function validateResult(r: WorldgenTypedDeterminismResult, featureId: string): void {
  const ids = new Set(r.candidate_order); const parts = [...r.selected_order, ...r.unresolved_order, ...r.blocked_order];
  if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.feature_id !== featureId || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_TYPED_DETERMINISM_CONTENT_TYPE || r.raw_data_local !== true || r.federation_export !== "aggregate-digest-only" || !r.candidate_order.length || parts.length !== ids.size || new Set(parts).size !== parts.length || parts.some((v) => !ids.has(v)) || r.artifact.content_hash !== r.canonical_digest || r.parity_score_milli.length !== r.selected_order.length || r.uncertainty_milli.length !== r.selected_order.length) throw new Error("typed-determinism result identity, states, locality, or digest is invalid");
  for (const values of [r.candidate_order, r.selected_order, r.unresolved_order, r.blocked_order, r.omitted_order, r.canonical_field_order, r.canonical_effect_order, r.omissions, r.uncertainty, r.contradiction, r.negative_evidence, r.effect_receipts]) if (!ordered(values)) throw new Error("typed-determinism vectors are not canonical");
}
export function validateWorldgenLocalTypedDeterminismResult(r: WorldgenTypedDeterminismResult): void { validateResult(r, WORLDGEN_LOCAL_TYPED_DETERMINISM_FEATURE_ID); }
export function validateWorldgenMultimodalTypedDeterminismResult(r: WorldgenTypedDeterminismResult): void { validateResult(r, WORLDGEN_MULTIMODAL_TYPED_DETERMINISM_FEATURE_ID); }
export function validateWorldgenThroughputTypedDeterminismResult(r: WorldgenTypedDeterminismResult): void { validateResult(r, WORLDGEN_THROUGHPUT_TYPED_DETERMINISM_FEATURE_ID); }
export function validateWorldgenFederatedTypedDeterminismResult(r: WorldgenTypedDeterminismResult): void { validateResult(r, WORLDGEN_FEDERATED_TYPED_DETERMINISM_FEATURE_ID); }
export function validateWorldgenTypedDeterminismContractReceipt(r: WorldgenTypedDeterminismContractReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_TYPED_DETERMINISM_CONTRACT_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.contract_digest) throw new Error("typed-determinism contract identity or digest is invalid"); }
export function validateWorldgenTypedDeterminismCopilotReceipt(r: WorldgenTypedDeterminismCopilotReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_TYPED_DETERMINISM_COPILOT_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.copilot_digest) throw new Error("typed-determinism copilot identity or digest is invalid"); }
export function validateWorldgenTypedDeterminismWorkflowReceipt(r: WorldgenTypedDeterminismWorkflowReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_TYPED_DETERMINISM_WORKFLOW_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.workflow_digest) throw new Error("typed-determinism workflow identity or digest is invalid"); }
export function worldgenTypedDeterminismDigest(r: WorldgenTypedDeterminismResult): string { validateWorldgenLocalTypedDeterminismResult(r); return digestJsonSync(r); }
export function worldgenTypedDeterminismContractDigest(r: WorldgenTypedDeterminismContractReceipt): string { validateWorldgenTypedDeterminismContractReceipt(r); return digestJsonSync(r); }
export function worldgenTypedDeterminismCopilotDigest(r: WorldgenTypedDeterminismCopilotReceipt): string { validateWorldgenTypedDeterminismCopilotReceipt(r); return digestJsonSync(r); }
export function worldgenTypedDeterminismWorkflowDigest(r: WorldgenTypedDeterminismWorkflowReceipt): string { validateWorldgenTypedDeterminismWorkflowReceipt(r); return digestJsonSync(r); }
