/** Public TypeScript contracts for Factory P32 lease/fencing integrity cards. */
import {digestJsonSync} from "./tooling.js";

export const FACTORY_LEASE_FENCING_INTEGRITY_CONTENT_TYPE="application/vnd.aurora.factory.lease-fencing-integrity-card-1+json" as const;
export const FACTORY_LEASE_FENCING_INTEGRITY_BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions" as const;

export const FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_FEATURE_ID="AFA-factory-P32-F01" as const;
export const FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_FEATURE_ID="AFA-factory-P32-F02" as const;
export const FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_FEATURE_ID="AFA-factory-P32-F03" as const;
export const FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_FEATURE_ID="AFA-factory-P32-F04" as const;
export const FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID="AFA-factory-P32-F05" as const;
export const FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID="AFA-factory-P32-F06" as const;
export const FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID="AFA-factory-P32-F07" as const;
export const FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID="AFA-factory-P32-F08" as const;
export const FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID="AFA-factory-P32-F09" as const;
export const FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID="AFA-factory-P32-F10" as const;
export const FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID="AFA-factory-P32-F11" as const;
export const FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID="AFA-factory-P32-F12" as const;
export const FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID="AFA-factory-P32-F13" as const;
export const FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID="AFA-factory-P32-F14" as const;
export const FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID="AFA-factory-P32-F15" as const;
export const FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID="AFA-factory-P32-F16" as const;

export interface FactoryLeaseFencingIntegrityCard {
  schema_version:string; contract_version:string; feature_id:string; request_id:string; purpose:string;
  disposition:"qualified"|"partial"|"unknown"|"blocked";
  lease_order:string[]; admitted_order:string[]; rejected_order:string[]; unknown_order:string[]; omitted_order:string[];
  fencing_order:string[]; worker_order:string[]; job_order:string[]; effect_order:string[];
  replay_identity:string; closure_digest:string; admitted_lease_count:number; total_lease_count:number;
  raw_data_local:true; aggregate_only:true; boundary:string; effect_receipts:string[];
  artifact:{artifact_id:string;content_type:string;content_hash:string;semantic_loss:string[];fence_tokens:string[];boundary:string};
}

const ordered=(v:string[])=>JSON.stringify([...new Set(v)].sort())===JSON.stringify(v);
const digest=(v:unknown):v is string=>typeof v==="string"&&/^[0-9a-f]{64}$/.test(v);
function validate(c:FactoryLeaseFencingIntegrityCard,id:string){
  if(c.schema_version!=="aurora-research-contract/1.0"||c.feature_id!==id||!c.request_id||!c.purpose||c.boundary!==FACTORY_LEASE_FENCING_INTEGRITY_BOUNDARY||c.raw_data_local!==true||c.aggregate_only!==true||!digest(c.replay_identity)||!digest(c.closure_digest)||c.artifact.content_type!==FACTORY_LEASE_FENCING_INTEGRITY_CONTENT_TYPE||c.artifact.content_hash!==c.closure_digest||c.artifact.boundary!==FACTORY_LEASE_FENCING_INTEGRITY_BOUNDARY||c.admitted_lease_count>c.total_lease_count)throw new Error("lease identity, locality, artifact, digest, boundary, or count is incomplete");
  for(const v of [c.lease_order,c.admitted_order,c.rejected_order,c.unknown_order,c.omitted_order,c.fencing_order,c.worker_order,c.job_order,c.effect_order,c.effect_receipts])if(!ordered(v))throw new Error("lease vectors are not canonical");
  const ids=new Set(c.lease_order),states=new Set([...c.admitted_order,...c.rejected_order,...c.unknown_order,...c.omitted_order]);
  if(ids.size!==states.size||[...ids].some(x=>!states.has(x)))throw new Error("lease states do not partition leases");
  if(c.admitted_lease_count!==c.admitted_order.length)throw new Error("admitted lease count does not match admitted order");
}

export const validateFactoryLocalLeaseFencingIntegrity=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_FEATURE_ID);
export const validateFactoryMultimodalLeaseFencingIntegrity=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_FEATURE_ID);
export const validateFactoryThroughputLeaseFencingIntegrity=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_FEATURE_ID);
export const validateFactoryFederatedContinualLeaseFencingIntegrity=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_FEATURE_ID);
export const validateFactoryLocalLeaseFencingIntegrityContract=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID);
export const validateFactoryMultimodalLeaseFencingIntegrityContract=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID);
export const validateFactoryThroughputLeaseFencingIntegrityContract=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID);
export const validateFactoryFederatedContinualLeaseFencingIntegrityContract=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_CONTRACT_FEATURE_ID);
export const validateFactoryLocalLeaseFencingIntegrityCopilot=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID);
export const validateFactoryMultimodalLeaseFencingIntegrityCopilot=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID);
export const validateFactoryThroughputLeaseFencingIntegrityCopilot=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID);
export const validateFactoryFederatedContinualLeaseFencingIntegrityCopilot=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_COPILOT_FEATURE_ID);
export const validateFactoryLocalLeaseFencingIntegrityWorkflow=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_LOCAL_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID);
export const validateFactoryMultimodalLeaseFencingIntegrityWorkflow=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_MULTIMODAL_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID);
export const validateFactoryThroughputLeaseFencingIntegrityWorkflow=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_THROUGHPUT_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID);
export const validateFactoryFederatedContinualLeaseFencingIntegrityWorkflow=(c:FactoryLeaseFencingIntegrityCard)=>validate(c,FACTORY_FEDERATED_CONTINUAL_LEASE_FENCING_INTEGRITY_WORKFLOW_FEATURE_ID);

export const factoryLeaseFencingIntegrityDigest=(c:FactoryLeaseFencingIntegrityCard)=>{validateFactoryLocalLeaseFencingIntegrity(c);return digestJsonSync(c)};
export const factoryLeaseFencingIntegrityContractDigest=(c:FactoryLeaseFencingIntegrityCard)=>{validateFactoryLocalLeaseFencingIntegrityContract(c);return digestJsonSync(c)};
export const factoryLeaseFencingIntegrityCopilotDigest=(c:FactoryLeaseFencingIntegrityCard)=>{validateFactoryLocalLeaseFencingIntegrityCopilot(c);return digestJsonSync(c)};
export const factoryLeaseFencingIntegrityWorkflowDigest=(c:FactoryLeaseFencingIntegrityCard)=>{validateFactoryLocalLeaseFencingIntegrityWorkflow(c);return digestJsonSync(c)};
