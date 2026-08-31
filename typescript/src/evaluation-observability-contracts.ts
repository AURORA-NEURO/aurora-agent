/** Public TypeScript contracts for Worldgen P23 evaluation/observability. */
import { digestJsonSync } from "./tooling.js";
import { PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION } from "./research-contracts.js";

export const WORLDGEN_EVALUATION_OBSERVABILITY_CONTENT_TYPE = "application/vnd.aurora.worldgen.evaluation-observability-receipt-1+json" as const;
export const WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-worldgen-P23-F01" as const;
export const WORLDGEN_MULTIMODAL_EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-worldgen-P23-F02" as const;
export const WORLDGEN_THROUGHPUT_EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-worldgen-P23-F03" as const;
export const WORLDGEN_FEDERATED_EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-worldgen-P23-F04" as const;
export const WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_CONTRACT_FEATURE_ID = "AFA-worldgen-P23-F05" as const;
export const WORLDGEN_MULTIMODAL_EVALUATION_OBSERVABILITY_CONTRACT_FEATURE_ID = "AFA-worldgen-P23-F06" as const;
export const WORLDGEN_THROUGHPUT_EVALUATION_OBSERVABILITY_CONTRACT_FEATURE_ID = "AFA-worldgen-P23-F07" as const;
export const WORLDGEN_FEDERATED_EVALUATION_OBSERVABILITY_CONTRACT_FEATURE_ID = "AFA-worldgen-P23-F08" as const;
export const WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_COPILOT_FEATURE_ID = "AFA-worldgen-P23-F09" as const;
export const WORLDGEN_MULTIMODAL_EVALUATION_OBSERVABILITY_COPILOT_FEATURE_ID = "AFA-worldgen-P23-F10" as const;
export const WORLDGEN_THROUGHPUT_EVALUATION_OBSERVABILITY_COPILOT_FEATURE_ID = "AFA-worldgen-P23-F11" as const;
export const WORLDGEN_FEDERATED_EVALUATION_OBSERVABILITY_COPILOT_FEATURE_ID = "AFA-worldgen-P23-F12" as const;
export const WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P23-F13" as const;
export const WORLDGEN_MULTIMODAL_EVALUATION_OBSERVABILITY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P23-F14" as const;
export const WORLDGEN_THROUGHPUT_EVALUATION_OBSERVABILITY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P23-F15" as const;
export const WORLDGEN_FEDERATED_EVALUATION_OBSERVABILITY_WORKFLOW_FEATURE_ID = "AFA-worldgen-P23-F16" as const;

export interface WorldgenEvaluationObservabilityCard {
 schema_version:string; contract_version:string; feature_id:string; mode:string; scale:string; request_id:string; benchmark_id:string; disposition:string;
 observation_order:string[]; passed_order:string[]; failed_order:string[]; unknown_order:string[]; unmeasured_order:string[]; contradicted_order:string[]; omitted_order:string[];
 baseline_delta_order:string[]; uncertainty_order:string[]; negative_evidence_order:string[]; site_order:string[]; metric_order:string[];
 replay_identity:string; evaluation_digest:string;
 artifact:{artifact_id:string;content_type:string;content_hash:string;semantic_loss:string[];provenance_digests:string[];boundary:string};
 effect_receipts:string[]; raw_data_local:true; aggregate_only:true; boundary:string;
}
const ordered=(v:string[])=>JSON.stringify([...new Set(v)].sort())===JSON.stringify(v);
const digest=(v:unknown)=>typeof v==="string"&&/^[0-9a-f]{64}$/.test(v);
function validate(r:WorldgenEvaluationObservabilityCard,id:string):void{
 if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==id||r.boundary!==PRECLINICAL_BOUNDARY||r.raw_data_local!==true||r.aggregate_only!==true||!r.observation_order.length||!digest(r.replay_identity)||!digest(r.evaluation_digest)||r.artifact.boundary!==PRECLINICAL_BOUNDARY||r.artifact.content_type!==WORLDGEN_EVALUATION_OBSERVABILITY_CONTENT_TYPE||r.artifact.content_hash!==r.evaluation_digest||JSON.stringify(r.artifact.semantic_loss)!==JSON.stringify(r.omitted_order)) throw new Error("evaluation/observability identity, locality, or digest is invalid");
 for(const values of [r.observation_order,r.passed_order,r.failed_order,r.unknown_order,r.unmeasured_order,r.contradicted_order,r.omitted_order,r.baseline_delta_order,r.uncertainty_order,r.negative_evidence_order,r.site_order,r.metric_order,r.effect_receipts]) if(!ordered(values)) throw new Error("evaluation vectors are not canonical");
 const ids=new Set(r.observation_order), parts=new Set([...r.passed_order,...r.failed_order,...r.unknown_order,...r.unmeasured_order,...r.contradicted_order,...r.omitted_order]); if(ids.size!==parts.size||[...ids].some(x=>!parts.has(x))) throw new Error("evaluation states do not partition");
}
export const validateWorldgenLocalEvaluationObservability=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_FEATURE_ID);
export const validateWorldgenMultimodalEvaluationObservability=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_MULTIMODAL_EVALUATION_OBSERVABILITY_FEATURE_ID);
export const validateWorldgenThroughputEvaluationObservability=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_THROUGHPUT_EVALUATION_OBSERVABILITY_FEATURE_ID);
export const validateWorldgenFederatedEvaluationObservability=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_FEDERATED_EVALUATION_OBSERVABILITY_FEATURE_ID);
export const validateWorldgenEvaluationObservabilityContract=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_CONTRACT_FEATURE_ID);
export const validateWorldgenEvaluationObservabilityCopilot=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_COPILOT_FEATURE_ID);
export const validateWorldgenEvaluationObservabilityWorkflow=(r:WorldgenEvaluationObservabilityCard)=>validate(r,WORLDGEN_LOCAL_EVALUATION_OBSERVABILITY_WORKFLOW_FEATURE_ID);
export const worldgenEvaluationObservabilityDigest=(r:WorldgenEvaluationObservabilityCard)=>{validateWorldgenLocalEvaluationObservability(r);return digestJsonSync(r)};
export const worldgenEvaluationObservabilityContractDigest=(r:WorldgenEvaluationObservabilityCard)=>{validateWorldgenEvaluationObservabilityContract(r);return digestJsonSync(r)};
export const worldgenEvaluationObservabilityCopilotDigest=(r:WorldgenEvaluationObservabilityCard)=>{validateWorldgenEvaluationObservabilityCopilot(r);return digestJsonSync(r)};
export const worldgenEvaluationObservabilityWorkflowDigest=(r:WorldgenEvaluationObservabilityCard)=>{validateWorldgenEvaluationObservabilityWorkflow(r);return digestJsonSync(r)};
