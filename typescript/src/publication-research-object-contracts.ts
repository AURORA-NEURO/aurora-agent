/** Public TypeScript contracts for Worldgen P16 publication/research-object release. */
import { digestJsonSync } from "./tooling.js";
import { PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION } from "./research-contracts.js";

export const WORLDGEN_PUBLICATION_RESEARCH_OBJECT_CONTENT_TYPE = "application/vnd.aurora.worldgen.publication-research-object-result+json" as const;
export const WORLDGEN_PUBLICATION_RESEARCH_OBJECT_CONTRACT_CONTENT_TYPE = "application/vnd.aurora.worldgen.publication-research-object-contract-receipt+json" as const;
export const WORLDGEN_PUBLICATION_RESEARCH_OBJECT_COPILOT_CONTENT_TYPE = "application/vnd.aurora.worldgen.publication-research-object-copilot-receipt+json" as const;
export const WORLDGEN_PUBLICATION_RESEARCH_OBJECT_WORKFLOW_CONTENT_TYPE = "application/vnd.aurora.worldgen.publication-research-object-workflow-receipt+json" as const;

export const WORLDGEN_LOCAL_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID = "AFA-worldgen-P16-F01" as const;
export const WORLDGEN_MULTIMODAL_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID = "AFA-worldgen-P16-F02" as const;
export const WORLDGEN_THROUGHPUT_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID = "AFA-worldgen-P16-F03" as const;
export const WORLDGEN_FEDERATED_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID = "AFA-worldgen-P16-F04" as const;
export const WORLDGEN_LOCAL_PUBLICATION_RESEARCH_OBJECT_CONTRACT_FEATURE_ID = "AFA-worldgen-P16-F05" as const;
export const WORLDGEN_MULTIMODAL_PUBLICATION_RESEARCH_OBJECT_CONTRACT_FEATURE_ID = "AFA-worldgen-P16-F06" as const;
export const WORLDGEN_THROUGHPUT_PUBLICATION_RESEARCH_OBJECT_CONTRACT_FEATURE_ID = "AFA-worldgen-P16-F07" as const;
export const WORLDGEN_FEDERATED_PUBLICATION_RESEARCH_OBJECT_CONTRACT_FEATURE_ID = "AFA-worldgen-P16-F08" as const;
export const WORLDGEN_LOCAL_PUBLICATION_RESEARCH_OBJECT_COPILOT_FEATURE_ID = "AFA-worldgen-P16-F09" as const;
export const WORLDGEN_MULTIMODAL_PUBLICATION_RESEARCH_OBJECT_COPILOT_FEATURE_ID = "AFA-worldgen-P16-F10" as const;
export const WORLDGEN_THROUGHPUT_PUBLICATION_RESEARCH_OBJECT_COPILOT_FEATURE_ID = "AFA-worldgen-P16-F11" as const;
export const WORLDGEN_FEDERATED_PUBLICATION_RESEARCH_OBJECT_COPILOT_FEATURE_ID = "AFA-worldgen-P16-F12" as const;
export const WORLDGEN_LOCAL_PUBLICATION_RESEARCH_OBJECT_WORKFLOW_FEATURE_ID = "AFA-worldgen-P16-F13" as const;
export const WORLDGEN_MULTIMODAL_PUBLICATION_RESEARCH_OBJECT_WORKFLOW_FEATURE_ID = "AFA-worldgen-P16-F14" as const;
export const WORLDGEN_THROUGHPUT_PUBLICATION_RESEARCH_OBJECT_WORKFLOW_FEATURE_ID = "AFA-worldgen-P16-F15" as const;
export const WORLDGEN_FEDERATED_PUBLICATION_RESEARCH_OBJECT_WORKFLOW_FEATURE_ID = "AFA-worldgen-P16-F16" as const;

export interface WorldgenPublicationResearchObjectResult {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; study_id: string; scope: string;
  disposition: "qualified" | "unresolved" | "blocked"; candidate_order: string[]; selected_order: string[]; unresolved_order: string[]; blocked_order: string[]; omitted_order: string[];
  validation_score_milli: number[]; uncertainty_milli: number[]; release_item_order: string[]; manifest_item_order: string[]; decisions: Record<string, unknown>[];
  replay_identity: string; release_digest: string; semantic_loss: Record<string, unknown>[]; omissions: string[]; uncertainty: string[]; contradiction: string[]; negative_evidence: string[];
  artifact: { artifact_id: string; content_type: string; content_hash: string; semantic_loss: Record<string, unknown>[]; provenance: Record<string, unknown>[]; boundary: string };
  effect_receipts: string[]; raw_data_local: true; federation_export: "aggregate-digest-only"; boundary: string;
}
export interface WorldgenPublicationResearchObjectContractReceipt { schema_version: string; contract_version: string; feature_id: string; request_id: string; consumer: string; producer: string; namespace: string; semantic_profile: string; negotiated_version: string; compatibility: string; disposition: string; field_order: string[]; retained_field_order: string[]; missing_field_order: string[]; omitted_field_order: string[]; semantic_loss_order: string[]; replay_identity: string; contract_digest: string; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }
export interface WorldgenPublicationResearchObjectCopilotReceipt { schema_version: string; contract_version: string; feature_id: string; request_id: string; disposition: string; action_order: string[]; admitted_action_order: string[]; denied_action_order: string[]; release_disposition: string; release_digest: string; copilot_digest: string; replay_identity: string; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }
export interface WorldgenPublicationResearchObjectWorkflowReceipt { schema_version: string; contract_version: string; feature_id: string; workflow_id: string; disposition: string; stage_order: string[]; completed_stage_order: string[]; pending_stage_order: string[]; compensation_order: string[]; checkpoint_seq: number; budget_units: number; consumed_units: number; replay_identity: string; workflow_digest: string; copilot: Record<string, unknown>; effect_receipts: string[]; artifact: { content_type: string; content_hash: string; boundary: string }; raw_data_local: true; aggregate_only: true; boundary: string }

function ordered(v: string[]): boolean { return JSON.stringify([...new Set(v)].sort()) === JSON.stringify(v); }
function validateResult(r: WorldgenPublicationResearchObjectResult, featureId: string): void {
  const ids = new Set(r.candidate_order); const parts = [...r.selected_order, ...r.unresolved_order, ...r.blocked_order];
  if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.feature_id !== featureId || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PUBLICATION_RESEARCH_OBJECT_CONTENT_TYPE || r.raw_data_local !== true || r.federation_export !== "aggregate-digest-only" || !r.candidate_order.length || parts.length !== ids.size || new Set(parts).size !== parts.length || parts.some((v) => !ids.has(v)) || r.artifact.content_hash !== r.release_digest || r.validation_score_milli.length !== r.selected_order.length || r.uncertainty_milli.length !== r.selected_order.length) throw new Error("publication-research-object result identity, states, locality, or digest is invalid");
  for (const values of [r.candidate_order, r.selected_order, r.unresolved_order, r.blocked_order, r.omitted_order, r.release_item_order, r.manifest_item_order, r.omissions, r.uncertainty, r.contradiction, r.negative_evidence, r.effect_receipts]) if (!ordered(values)) throw new Error("publication-research-object vectors are not canonical");
}
export function validateWorldgenLocalPublicationResearchObjectResult(r: WorldgenPublicationResearchObjectResult): void { validateResult(r, WORLDGEN_LOCAL_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID); }
export function validateWorldgenMultimodalPublicationResearchObjectResult(r: WorldgenPublicationResearchObjectResult): void { validateResult(r, WORLDGEN_MULTIMODAL_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID); }
export function validateWorldgenThroughputPublicationResearchObjectResult(r: WorldgenPublicationResearchObjectResult): void { validateResult(r, WORLDGEN_THROUGHPUT_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID); }
export function validateWorldgenFederatedPublicationResearchObjectResult(r: WorldgenPublicationResearchObjectResult): void { validateResult(r, WORLDGEN_FEDERATED_PUBLICATION_RESEARCH_OBJECT_FEATURE_ID); }
export function validateWorldgenPublicationResearchObjectContractReceipt(r: WorldgenPublicationResearchObjectContractReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PUBLICATION_RESEARCH_OBJECT_CONTRACT_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.contract_digest) throw new Error("publication-research-object contract identity or digest is invalid"); }
export function validateWorldgenPublicationResearchObjectCopilotReceipt(r: WorldgenPublicationResearchObjectCopilotReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PUBLICATION_RESEARCH_OBJECT_COPILOT_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.copilot_digest) throw new Error("publication-research-object copilot identity or digest is invalid"); }
export function validateWorldgenPublicationResearchObjectWorkflowReceipt(r: WorldgenPublicationResearchObjectWorkflowReceipt): void { if (r.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || r.boundary !== PRECLINICAL_BOUNDARY || r.artifact.boundary !== PRECLINICAL_BOUNDARY || r.artifact.content_type !== WORLDGEN_PUBLICATION_RESEARCH_OBJECT_WORKFLOW_CONTENT_TYPE || r.raw_data_local !== true || r.aggregate_only !== true || r.artifact.content_hash !== r.workflow_digest) throw new Error("publication-research-object workflow identity or digest is invalid"); }
export function worldgenPublicationResearchObjectDigest(r: WorldgenPublicationResearchObjectResult): string { validateWorldgenLocalPublicationResearchObjectResult(r); return digestJsonSync(r); }
export function worldgenPublicationResearchObjectContractDigest(r: WorldgenPublicationResearchObjectContractReceipt): string { validateWorldgenPublicationResearchObjectContractReceipt(r); return digestJsonSync(r); }
export function worldgenPublicationResearchObjectCopilotDigest(r: WorldgenPublicationResearchObjectCopilotReceipt): string { validateWorldgenPublicationResearchObjectCopilotReceipt(r); return digestJsonSync(r); }
export function worldgenPublicationResearchObjectWorkflowDigest(r: WorldgenPublicationResearchObjectWorkflowReceipt): string { validateWorldgenPublicationResearchObjectWorkflowReceipt(r); return digestJsonSync(r); }
