"""Resumable, budgeted resource-discovery workflow for Worldgen P05 F13-F16."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_resource_copilot_support import ResourceCopilotRequest, run as run_copilot
CONTENT_TYPE="application/vnd.aurora.worldgen.resource-workflow-receipt+json"
@dataclass(frozen=True)
class ResourceWorkflowRequest:
    workflow_id:str; copilot:ResourceCopilotRequest; stage_order:tuple[str,...]; completed_stage_order:tuple[str,...]; checkpoint_seq:int; budget_units:int; compensation_enabled:bool; replay_identity:str
@dataclass(frozen=True)
class ResourceWorkflowReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("workflow_id") and v.get("stage_order") and v.get("checkpoint_seq",0)>0 and v.get("budget_units",0)>0 and v.get("consumed_units",0)<=v.get("budget_units",0) and v.get("effect_receipts") and all(isinstance(v.get(k),str) and len(v[k])==64 and all(c in "0123456789abcdef" for c in v[k]) for k in ("replay_identity","workflow_digest")) and a.get("content_hash")==v.get("workflow_digest")): raise ResearchContractError("resource workflow identity, stages, budget, locality, digests, or effects are incomplete")
        for key in ("stage_order","completed_stage_order","pending_stage_order","compensation_order","effect_receipts"):
            vals=tuple(v.get(key,()))
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("resource workflow vectors are not canonical")
        stages=set(v["stage_order"]); parts=set(v.get("completed_stage_order",()))|set(v.get("pending_stage_order",()))
        if len(stages)!=len(v["stage_order"]) or parts!=stages: raise ResearchContractError("resource workflow stages do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["resource steward","workflow operator","downstream resource consumer"],"behavior":f"orchestrate resumable resource discovery for {scale}","value":"makes resource qualification restartable, budgeted, compensating, and replayable without hidden effects","input_schema":input_schema,"output_schema":"ResourceWorkflowReceipt1@1","effects":["schedule:worldgen-resource-workflow","block:unsafe-release"],"permissions":["schedule:bounded-resource-workflow"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def schedule(request:ResourceWorkflowRequest,*,feature_id:str,contract_version:str,scale:str,require_approval:bool=False,require_federation:bool=False)->ResourceWorkflowReceipt:
    if not(request.workflow_id.strip() and request.stage_order and tuple(request.stage_order)==tuple(sorted(set(request.stage_order))) and request.checkpoint_seq>0 and request.budget_units>0 and request.compensation_enabled and len(request.replay_identity)==64 and request.copilot.discovery.replay_identity==request.replay_identity): raise ResearchContractError("resource workflow request is invalid")
    completed=set(request.completed_stage_order)
    if not completed<=set(request.stage_order): raise ResearchContractError("resource workflow completed stage is undeclared")
    copilot=run_copilot(request.copilot,feature_id=feature_id,contract_version=contract_version,scale=scale,require_approval=require_approval,require_federation=require_federation)
    pending=[stage for stage in request.stage_order if stage not in completed]; consumed=min(len(request.copilot.action_order),request.budget_units); compensation=[f"workflow:{request.workflow_id}:retain-partial-resource-set"] if copilot.value["disposition"]=="blocked" or len(request.copilot.action_order)>request.budget_units else []; disposition="blocked" if copilot.value["disposition"]=="blocked" or compensation else "qualified" if not pending else "partial"; effects=[f"schedule:worldgen-resource-workflow:{request.workflow_id}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"workflow_id":request.workflow_id,"disposition":disposition,"stage_order":list(request.stage_order),"completed_stage_order":sorted(completed),"pending_stage_order":pending,"compensation_order":compensation,"checkpoint_seq":request.checkpoint_seq,"budget_units":request.budget_units,"consumed_units":consumed,"replay_identity":request.replay_identity,"copilot":copilot.value,"workflow_digest":"","effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    digest_payload=dict(payload); digest_payload.pop("workflow_digest",None); digest=research_artifact_digest(digest_payload); payload["workflow_digest"]=digest; payload["artifact"]={"artifact_id":f"resource-workflow:{request.workflow_id}","content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}; receipt=ResourceWorkflowReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ResourceWorkflowRequest","ResourceWorkflowReceipt","manifest","schedule"]
