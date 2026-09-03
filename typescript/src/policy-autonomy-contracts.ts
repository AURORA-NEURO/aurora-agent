/** Public TypeScript contracts for Worldgen P19 policy/autonomy admission. */
import { digestJsonSync } from "./tooling.js";
import { PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION } from "./research-contracts.js";

export const WORLDGEN_POLICY_AUTONOMY_CONTENT_TYPE = "application/vnd.aurora.worldgen.policy-autonomy-receipt-1+json" as const;
export const WORLDGEN_POLICY_AUTONOMY_CONTRACT_CONTENT_TYPE = "application/vnd.aurora.worldgen.policy-autonomy-contract-receipt+json" as const;
export const WORLDGEN_POLICY_AUTONOMY_COPILOT_CONTENT_TYPE = "application/vnd.aurora.worldgen.policy-autonomy-signing-copilot-receipt+json" as const;
export const WORLDGEN_POLICY_AUTONOMY_WORKFLOW_CONTENT_TYPE = "application/vnd.aurora.worldgen.policy-autonomy-signing-workflow-receipt+json" as const;

export const WORLDGEN_LOCAL_POLICY_AUTONOMY_FEATURE_ID = "AFA-worldgen-P19-F01" as const;
export const WORLDGEN_MULTIMODAL_POLICY_AUTONOMY_FEATURE_ID = "AFA-worldgen-P19-F02" as const;
export const WORLDGEN_THROUGHPUT_POLICY_AUTONOMY_FEATURE_ID = "AFA-worldgen-P19-F03" as const;
export const WORLDGEN_FEDERATED_POLICY_AUTONOMY_FEATURE_ID = "AFA-worldgen-P19-F04" as const;
export const WORLDGEN_LOCAL_POLICY_AUTONOMY_CONTRACT_FEATURE_ID = "AFA-worldgen-P19-F05" as const;
export const WORLDGEN_MULTIMODAL_POLICY_AUTONOMY_CONTRACT_FEATURE_ID = "AFA-worldgen-P19-F06" as const;
export const WORLDGEN_THROUGHPUT_POLICY_AUTONOMY_CONTRACT_FEATURE_ID = "AFA-worldgen-P19-F07" as const;
export const WORLDGEN_FEDERATED_POLICY_AUTONOMY_CONTRACT_FEATURE_ID = "AFA-worldgen-P19-F08" as const;
export const WORLDGEN_LOCAL_POLICY_AUTONOMY_COPILOT_FEATURE_ID = "AFA-worldgen-P19-F09" as const;
export const WORLDGEN_MULTIMODAL_POLICY_AUTONOMY_COPILOT_FEATURE_ID = "AFA-worldgen-P19-F10" as const;
export const WORLDGEN_THROUGHPUT_POLICY_AUTONOMY_COPILOT_FEATURE_ID = "AFA-worldgen-P19-F11" as const;
export const WORLDGEN_FEDERATED_POLICY_AUTONOMY_COPILOT_FEATURE_ID = "AFA-worldgen-P19-F12" as const;
export const WORLDGEN_LOCAL_POLICY_AUTONOMY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P19-F13" as const;
export const WORLDGEN_MULTIMODAL_POLICY_AUTONOMY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P19-F14" as const;
export const WORLDGEN_THROUGHPUT_POLICY_AUTONOMY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P19-F15" as const;
export const WORLDGEN_FEDERATED_POLICY_AUTONOMY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P19-F16" as const;

export interface WorldgenPolicyAutonomyResult {
  schema_version:string; contract_version:string; feature_id:string; request_id:string; consumer:string; purpose:string; required_scope:string; policy_epoch:string;
  disposition:"qualified"|"partial"|"blocked"; action_order:string[]; allowed_order:string[]; approval_required_order:string[]; local_only_order:string[]; denied_order:string[]; unresolved_order:string[]; omission_order:string[]; uncertainty_order:string[]; negative_evidence_order:string[];
  replay_identity:string; receipt_digest:string; artifact:{artifact_id:string;content_type:string;content_hash:string;semantic_loss:string[];provenance_digests:string[];boundary:string}; raw_data_local:true; boundary:string;
}
export interface WorldgenPolicyAutonomyContractReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; consumer:string; producer:string; namespace:string; semantic_profile:string; negotiated_version:string; compatibility:string; disposition:string; field_order:string[]; retained_field_order:string[]; missing_field_order:string[]; omitted_field_order:string[]; semantic_loss_order:string[]; replay_identity:string; contract_digest:string; effect_receipts:string[]; artifact:{content_type:string;content_hash:string;boundary:string}; raw_data_local:true; aggregate_only:true; boundary:string }
export interface WorldgenPolicyAutonomyCopilotReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; disposition:string; action_order:string[]; admitted_action_order:string[]; denied_action_order:string[]; policy_disposition:string; policy_digest:string; copilot_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:{content_type:string;content_hash:string;boundary:string}; raw_data_local:true; aggregate_only:true; boundary:string }
export interface WorldgenPolicyAutonomyWorkflowReceipt { schema_version:string; contract_version:string; feature_id:string; workflow_id:string; disposition:string; stage_order:string[]; completed_stage_order:string[]; pending_stage_order:string[]; compensation_order:string[]; checkpoint_seq:number; budget_units:number; consumed_units:number; replay_identity:string; workflow_digest:string; copilot:Record<string,unknown>; effect_receipts:string[]; artifact:{content_type:string;content_hash:string;boundary:string}; raw_data_local:true; aggregate_only:true; boundary:string }

const ordered=(v:string[])=>JSON.stringify([...new Set(v)].sort())===JSON.stringify(v);
const digest=(v:unknown)=>typeof v==="string"&&/^[0-9a-f]{64}$/.test(v);
function validateResult(r:WorldgenPolicyAutonomyResult,id:string):void{
  const ids=new Set(r.action_order), parts=[...r.allowed_order,...r.approval_required_order,...r.local_only_order,...r.denied_order,...r.unresolved_order];
  if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==id||r.boundary!==PRECLINICAL_BOUNDARY||r.artifact.boundary!==PRECLINICAL_BOUNDARY||r.artifact.content_type!==WORLDGEN_POLICY_AUTONOMY_CONTENT_TYPE||r.raw_data_local!==true||!r.action_order.length||parts.length!==ids.size||new Set(parts).size!==parts.length||parts.some(x=>!ids.has(x))||!digest(r.replay_identity)||!digest(r.receipt_digest)||r.artifact.content_hash!==r.receipt_digest) throw new Error("policy-autonomy result identity, states, locality, or digest is invalid");
  for(const values of [r.action_order,r.allowed_order,r.approval_required_order,r.local_only_order,r.denied_order,r.unresolved_order,r.omission_order,r.uncertainty_order,r.negative_evidence_order]) if(!ordered(values)) throw new Error("policy-autonomy vectors are not canonical");
}
export function validateWorldgenLocalPolicyAutonomyResult(r:WorldgenPolicyAutonomyResult):void{validateResult(r,WORLDGEN_LOCAL_POLICY_AUTONOMY_FEATURE_ID)}
export function validateWorldgenMultimodalPolicyAutonomyResult(r:WorldgenPolicyAutonomyResult):void{validateResult(r,WORLDGEN_MULTIMODAL_POLICY_AUTONOMY_FEATURE_ID)}
export function validateWorldgenThroughputPolicyAutonomyResult(r:WorldgenPolicyAutonomyResult):void{validateResult(r,WORLDGEN_THROUGHPUT_POLICY_AUTONOMY_FEATURE_ID)}
export function validateWorldgenFederatedPolicyAutonomyResult(r:WorldgenPolicyAutonomyResult):void{validateResult(r,WORLDGEN_FEDERATED_POLICY_AUTONOMY_FEATURE_ID)}
export function validateWorldgenPolicyAutonomyContractReceipt(r:WorldgenPolicyAutonomyContractReceipt):void{if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.boundary!==PRECLINICAL_BOUNDARY||r.artifact.boundary!==PRECLINICAL_BOUNDARY||r.artifact.content_type!==WORLDGEN_POLICY_AUTONOMY_CONTRACT_CONTENT_TYPE||r.raw_data_local!==true||r.aggregate_only!==true||r.artifact.content_hash!==r.contract_digest)throw new Error("policy-autonomy contract identity or digest is invalid")}
export function validateWorldgenPolicyAutonomyCopilotReceipt(r:WorldgenPolicyAutonomyCopilotReceipt):void{if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.boundary!==PRECLINICAL_BOUNDARY||r.artifact.boundary!==PRECLINICAL_BOUNDARY||r.artifact.content_type!==WORLDGEN_POLICY_AUTONOMY_COPILOT_CONTENT_TYPE||r.raw_data_local!==true||r.aggregate_only!==true||r.artifact.content_hash!==r.copilot_digest)throw new Error("policy-autonomy copilot identity or digest is invalid")}
export function validateWorldgenPolicyAutonomyWorkflowReceipt(r:WorldgenPolicyAutonomyWorkflowReceipt):void{if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.boundary!==PRECLINICAL_BOUNDARY||r.artifact.boundary!==PRECLINICAL_BOUNDARY||r.artifact.content_type!==WORLDGEN_POLICY_AUTONOMY_WORKFLOW_CONTENT_TYPE||r.raw_data_local!==true||r.aggregate_only!==true||r.artifact.content_hash!==r.workflow_digest)throw new Error("policy-autonomy workflow identity or digest is invalid")}
export const worldgenPolicyAutonomyDigest=(r:WorldgenPolicyAutonomyResult)=>{validateWorldgenLocalPolicyAutonomyResult(r);return digestJsonSync(r)};
export const worldgenPolicyAutonomyContractDigest=(r:WorldgenPolicyAutonomyContractReceipt)=>{validateWorldgenPolicyAutonomyContractReceipt(r);return digestJsonSync(r)};
export const worldgenPolicyAutonomyCopilotDigest=(r:WorldgenPolicyAutonomyCopilotReceipt)=>{validateWorldgenPolicyAutonomyCopilotReceipt(r);return digestJsonSync(r)};
export const worldgenPolicyAutonomyWorkflowDigest=(r:WorldgenPolicyAutonomyWorkflowReceipt)=>{validateWorldgenPolicyAutonomyWorkflowReceipt(r);return digestJsonSync(r)};
