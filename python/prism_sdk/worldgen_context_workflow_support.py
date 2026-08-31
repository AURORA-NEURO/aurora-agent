"""Resumable context-compilation workflow fabric for Worldgen P03 F13-F16."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
from .worldgen_context_copilot_support import ContextCopilotRequest,ContextCopilotReceipt,run as run_copilot
CONTENT_TYPE="application/vnd.aurora.worldgen.context-workflow-receipt+json";_HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ContextWorkflowRequest:
    workflow_id:str;copilot:ContextCopilotRequest;stage_order:tuple[str,...];completed_stage_order:tuple[str,...];checkpoint_seq:int;budget_units:int;compensation_enabled:bool;replay_identity:str
@dataclass(frozen=True)
class ContextWorkflowReceipt:
    value:dict[str,Any]
    def validate(self,*,feature_id:str,contract_version:str)->None:
        v=self.value;a=v.get("artifact",{});syn=v.get("copilot",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=contract_version or v.get("feature_id")!=feature_id or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("workflow_id","").strip() or not v.get("stage_order") or len(v.get("stage_order",[]))!=len(v.get("completed_stage_order",[]))+len(v.get("pending_stage_order",[])) or v.get("budget_units",0)<=0 or v.get("consumed_units",-1)<0 or v.get("consumed_units",0)>v.get("budget_units",0) or v.get("checkpoint_seq",0)<=0 or not v.get("effect_receipts") or not _HEX.fullmatch(v.get("replay_identity","")) or not _HEX.fullmatch(v.get("workflow_digest","")) or a.get("content_hash")!=v.get("workflow_digest"): raise ResearchContractError("worldgen context workflow identity, stages, budget, locality, or effects are incomplete")
        for key in ("stage_order","completed_stage_order","pending_stage_order","compensation_order","effect_receipts"):
            vals=list(v.get(key,[]));
            if vals!=sorted(set(vals)): raise ResearchContractError("worldgen context workflow ordering is not canonical")
        stages=set(v["stage_order"]);parts=list(v.get("completed_stage_order",[]))+list(v.get("pending_stage_order",[]))
        if len(stages)!=len(v["stage_order"]) or len(parts)!=len(stages) or len(set(parts))!=len(parts) or set(parts)!=stages: raise ResearchContractError("worldgen context workflow stages do not partition")
        if not syn: raise ResearchContractError("nested context copilot receipt is missing")
        ContextCopilotReceipt(syn).validate(feature_id=feature_id,contract_version=contract_version)
        if any(e!="block:unsafe-release" and not e.startswith("schedule:worldgen-context-workflow:") for e in v["effect_receipts"]): raise ResearchContractError("worldgen context workflow effect is outside scheduling gate")
    def digest(self,*,feature_id:str,contract_version:str)->str: self.validate(feature_id=feature_id,contract_version=contract_version);return _digest(self.value)
def _digest(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","context compiler","workflow operator","downstream crate consumer"],"behavior":f"orchestrate a resumable context-compilation workflow for {scale} with checkpoints and compensation","value":"makes typed context assembly restartable, budgeted, and replayable without hidden effects","input_schema":input_schema,"output_schema":"CompiledResearchContext6@1","effects":["schedule:worldgen-context-workflow","block:unsafe-release"],"permissions":["schedule:bounded-context-workflow"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def schedule(request:ContextWorkflowRequest,*,feature_id:str,contract_version:str,require_approval:bool,require_federation:bool)->ContextWorkflowReceipt:
    if not request.workflow_id.strip() or not request.stage_order or request.budget_units<=0 or request.checkpoint_seq<=0 or not request.compensation_enabled or not _HEX.fullmatch(request.replay_identity) or request.copilot.context_request.replay_identity!=request.replay_identity: raise ResearchContractError("worldgen context workflow identity, stages, checkpoint, budget, compensation, or replay is invalid")
    stages=sorted(set(request.stage_order))
    if len(stages)!=len(request.stage_order) or any(x not in stages for x in request.completed_stage_order): raise ResearchContractError("worldgen context workflow stages must be unique and declared")
    copilot=run_copilot(request.copilot,feature_id=feature_id,contract_version=contract_version,require_approval=require_approval,require_federation=require_federation);completed=set(request.completed_stage_order);pending=[x for x in stages if x not in completed];consumed=min(len(request.copilot.action_order),request.budget_units);compensation=[f"workflow:{request.workflow_id}:retain-partial-context"] if copilot.value["disposition"]=="blocked" or len(request.copilot.action_order)>request.budget_units else [];disposition="blocked" if copilot.value["disposition"]=="blocked" or compensation else "qualified" if not pending else "partial";effects=[f"schedule:worldgen-context-workflow:{request.workflow_id}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"workflow_id":request.workflow_id,"disposition":disposition,"stage_order":stages,"completed_stage_order":sorted(completed),"pending_stage_order":pending,"compensation_order":compensation,"checkpoint_seq":request.checkpoint_seq,"budget_units":request.budget_units,"consumed_units":consumed,"replay_identity":request.replay_identity,"copilot":copilot.value,"effect_receipts":sorted(effects),"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};digest=_digest(payload);payload["workflow_digest"]=digest;payload["artifact"]={"artifact_id":f"worldgen-context-workflow:{request.workflow_id}","content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY};receipt=ContextWorkflowReceipt(payload);receipt.validate(feature_id=feature_id,contract_version=contract_version);return receipt
__all__=["CONTENT_TYPE","ContextWorkflowRequest","ContextWorkflowReceipt","schedule","manifest"]
