"""Resumable, budgeted experiment-design workflow for Worldgen P08 F13-F16."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_experiment_design_copilot_support import ExperimentDesignCopilotRequest, run as run_copilot
CONTENT_TYPE="application/vnd.aurora.worldgen.experiment_design-workflow-receipt+json"
@dataclass(frozen=True)
class ExperimentDesignWorkflowRequest:
    workflow_id:str; copilot:ExperimentDesignCopilotRequest; stage_order:tuple[str,...]; completed_stage_order:tuple[str,...]; checkpoint_seq:int; budget_units:int; compensation_enabled:bool; replay_identity:str
@dataclass(frozen=True)
class ExperimentDesignWorkflowReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("workflow_id") and v.get("stage_order") and v.get("checkpoint_seq",0)>0 and v.get("budget_units",0)>0 and v.get("consumed_units",0)<=v.get("budget_units",0) and v.get("effect_receipts") and all(isinstance(v.get(k),str) and len(v[k])==64 and all(c in "0123456789abcdef" for c in v[k]) for k in ("replay_identity","workflow_digest")) and a.get("content_hash")==v.get("workflow_digest")): raise ResearchContractError("experiment design workflow identity, stages, budget, locality, digests, or effects are incomplete")
        for key in ("stage_order","completed_stage_order","pending_stage_order","compensation_order","effect_receipts"):
            vals=tuple(v.get(key,()))
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("experiment design workflow vectors are not canonical")
        stages=set(v["stage_order"]); parts=set(v.get("completed_stage_order",()))|set(v.get("pending_stage_order",()))
        if len(stages)!=len(v["stage_order"]) or parts!=stages: raise ResearchContractError("experiment design workflow stages do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["experiment_design steward","workflow operator","downstream experiment_design consumer"],"behavior":f"orchestrate resumable experiment_design experiment_design for {scale}","value":"makes experiment_design qualification restartable, budgeted, compensating, and replayable without hidden effects","input_schema":input_schema,"output_schema":"ExperimentDesignWorkflowReceipt1@1","effects":["schedule:worldgen-experiment_design-workflow","block:unsafe-release"],"permissions":["schedule:bounded-experiment_design-workflow"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def schedule(request:ExperimentDesignWorkflowRequest,*,feature_id:str,contract_version:str,scale:str,require_approval:bool=False,require_federation:bool=False)->ExperimentDesignWorkflowReceipt:
    if not(request.workflow_id.strip() and request.stage_order and tuple(request.stage_order)==tuple(sorted(set(request.stage_order))) and request.checkpoint_seq>0 and request.budget_units>0 and request.compensation_enabled and len(request.replay_identity)==64 and request.copilot.design_request.replay_identity==request.replay_identity): raise ResearchContractError("experiment design workflow request is invalid")
    completed=set(request.completed_stage_order)
    if not completed<=set(request.stage_order): raise ResearchContractError("experiment design workflow completed stage is undeclared")
    copilot=run_copilot(request.copilot,feature_id=feature_id,contract_version=contract_version,scale=scale,require_approval=require_approval,require_federation=require_federation)
    pending=[stage for stage in request.stage_order if stage not in completed]; consumed=min(len(request.copilot.action_order),request.budget_units); compensation=[f"workflow:{request.workflow_id}:retain-partial-experiment_design-set"] if copilot.value["disposition"]=="blocked" or len(request.copilot.action_order)>request.budget_units else []; disposition="blocked" if copilot.value["disposition"]=="blocked" or compensation else "qualified" if not pending else "partial"; effects=[f"schedule:worldgen-experiment_design-workflow:{request.workflow_id}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"workflow_id":request.workflow_id,"disposition":disposition,"stage_order":list(request.stage_order),"completed_stage_order":sorted(completed),"pending_stage_order":pending,"compensation_order":compensation,"checkpoint_seq":request.checkpoint_seq,"budget_units":request.budget_units,"consumed_units":consumed,"replay_identity":request.replay_identity,"copilot":copilot.value,"workflow_digest":"","effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    digest_payload=dict(payload); digest_payload.pop("workflow_digest",None); digest=research_artifact_digest(digest_payload); payload["workflow_digest"]=digest; payload["artifact"]={"artifact_id":f"experiment_design-workflow:{request.workflow_id}","content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}; receipt=ExperimentDesignWorkflowReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ExperimentDesignWorkflowRequest","ExperimentDesignWorkflowReceipt","manifest","schedule"]




