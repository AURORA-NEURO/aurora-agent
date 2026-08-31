"""Resumable computational-execution workflow fabric for Worldgen P12."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_computational_execution_copilot_support import ExecutionCopilotRequest, ExecutionCopilotReceipt, run as run_copilot
CONTENT_TYPE="application/vnd.aurora.worldgen.execution-workflow-receipt+json"
@dataclass(frozen=True)
class ExecutionWorkflowRequest:
    workflow_id:str; copilot:ExecutionCopilotRequest; stage_order:tuple[str,...]; completed_stage_order:tuple[str,...]; checkpoint_seq:int; budget_units:int; compensation_enabled:bool; replay_identity:str
@dataclass(frozen=True)
class ExecutionWorkflowReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("workflow_id") and v.get("stage_order") and v.get("checkpoint_seq",0)>0 and v.get("budget_units",0)>0 and a.get("content_hash")==v.get("workflow_digest")): raise ResearchContractError("execution workflow receipt is invalid")
        stages=set(v["stage_order"]); parts=set(v.get("completed_stage_order",()))|set(v.get("pending_stage_order",()));
        if stages!=parts: raise ResearchContractError("execution workflow stages do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["bioinformatician","workflow operator"],"behavior":f"orchestrate resumable execution for {scale}","value":"makes execution qualification restartable, budgeted, compensating, and replayable","input_schema":input_schema,"output_schema":"ExecutionWorkflowReceipt1@1","effects":["schedule:worldgen-execution-workflow","block:unsafe-release"],"permissions":["schedule:bounded-execution-workflow"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def schedule(request:ExecutionWorkflowRequest,*,feature_id:str,contract_version:str,scale:str,require_approval:bool=False,require_federation:bool=False)->ExecutionWorkflowReceipt:
    if not(request.workflow_id.strip() and request.stage_order and request.stage_order==tuple(sorted(set(request.stage_order))) and request.checkpoint_seq>0 and request.budget_units>0 and request.compensation_enabled and len(request.replay_identity)==64): raise ResearchContractError("execution workflow request is invalid")
    completed=set(request.completed_stage_order)
    if not completed<=set(request.stage_order): raise ResearchContractError("execution workflow completed stage is undeclared")
    copilot=run_copilot(request.copilot,feature_id=feature_id,contract_version=contract_version,scale=scale,require_approval=require_approval,require_federation=require_federation); pending=[x for x in request.stage_order if x not in completed]; compensation=[f"workflow:{request.workflow_id}:retain-partial-execution-verdict"] if copilot.value["disposition"]=="blocked" else []; disposition="blocked" if compensation else "qualified" if not pending else "partial"; effects=[f"schedule:worldgen-execution-workflow:{request.workflow_id}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"workflow_id":request.workflow_id,"disposition":disposition,"stage_order":list(request.stage_order),"completed_stage_order":sorted(completed),"pending_stage_order":pending,"compensation_order":compensation,"checkpoint_seq":request.checkpoint_seq,"budget_units":request.budget_units,"consumed_units":min(len(request.copilot.action_order),request.budget_units),"replay_identity":request.replay_identity,"copilot":copilot.value,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=research_artifact_digest(payload); payload["workflow_digest"]=digest; payload["artifact"]={"content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}; out=ExecutionWorkflowReceipt(payload); out.validate(); return out
__all__=["CONTENT_TYPE","ExecutionWorkflowRequest","ExecutionWorkflowReceipt","manifest","schedule"]
