"""Resumable retrieval-synthesis workflow fabric for Worldgen P02 F13–F16."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_retrieval_copilot_support import RetrievalCopilotRequest, RetrievalCopilotReceipt, run as run_copilot
CONTENT_TYPE="application/vnd.aurora.worldgen.retrieval-workflow-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class RetrievalWorkflowRequest:
    workflow_id:str; copilot:RetrievalCopilotRequest; stage_order:tuple[str,...]; completed_stage_order:tuple[str,...]; checkpoint_seq:int; budget_units:int; compensation_enabled:bool; replay_identity:str
@dataclass(frozen=True)
class RetrievalWorkflowReceipt:
    value:dict[str,Any]
    def validate(self, *, feature_id:str, contract_version:str)->None:
        v=self.value; a=v.get("artifact",{}); syn=v.get("copilot",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=contract_version or v.get("feature_id")!=feature_id or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("workflow_id","").strip() or not v.get("stage_order") or len(v.get("stage_order",[]))!=len(v.get("completed_stage_order",[]))+len(v.get("pending_stage_order",[])) or not v.get("effect_receipts") or not _HEX.fullmatch(v.get("replay_identity","")) or not _HEX.fullmatch(v.get("workflow_digest","")) or a.get("content_hash")!=v.get("workflow_digest"): raise ResearchContractError("worldgen retrieval workflow identity, stages, locality, or effects are incomplete")
        for key in ("stage_order","completed_stage_order","pending_stage_order","compensation_order","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("worldgen retrieval workflow ordering is not canonical")
        stages=set(v["stage_order"]); parts=list(v.get("completed_stage_order",()))+list(v.get("pending_stage_order",()))
        if len(stages)!=len(v["stage_order"]) or len(parts)!=len(stages) or len(set(parts))!=len(parts) or set(parts)!=stages: raise ResearchContractError("worldgen retrieval workflow stages do not partition")
        if not syn: raise ResearchContractError("nested retrieval copilot receipt is missing")
    def digest(self, *,feature_id:str,contract_version:str)->str: self.validate(feature_id=feature_id,contract_version=contract_version); return _digest(self.value)
def _digest(value:Any)->str: return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","bioinformatician","imaging core scientist","benchmark curator"],"behavior":f"orchestrate a resumable retrieval-synthesis workflow for {scale} with checkpoints and compensation","value":"makes retrieval plans restartable, budgeted, and replayable without hidden effects","input_schema":input_schema,"output_schema":"EvidenceSynthesis4@1","effects":["schedule:local-retrieval-workflow","block:unsafe-release"],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def schedule(request:RetrievalWorkflowRequest, *, feature_id:str, contract_version:str, require_approval:bool, require_federation:bool)->RetrievalWorkflowReceipt:
    if not request.workflow_id.strip() or not request.stage_order or request.budget_units<=0 or request.checkpoint_seq<=0 or not request.compensation_enabled or not _HEX.fullmatch(request.replay_identity) or request.copilot.query.replay_identity!=request.replay_identity: raise ResearchContractError("worldgen retrieval workflow identity, stages, checkpoint, budget, compensation, or replay is invalid")
    stages=sorted(set(request.stage_order));
    if len(stages)!=len(request.stage_order) or any(x not in stages for x in request.completed_stage_order): raise ResearchContractError("worldgen retrieval workflow stages must be unique and declared")
    copilot=run_copilot(request.copilot,feature_id=feature_id,contract_version=contract_version,require_approval=require_approval,require_federation=require_federation); completed=set(request.completed_stage_order); pending=[x for x in stages if x not in completed]; consumed=min(sum(c.estimated_units for c in request.copilot.query.candidates),request.budget_units); compensation=[f"workflow:{request.workflow_id}:retain-partial-artifact"] if copilot.value["disposition"]=="blocked" else []; disposition="blocked" if copilot.value["disposition"]=="blocked" or compensation else "qualified" if not pending else "partial"; effects=[f"schedule:local-retrieval-workflow:{request.workflow_id}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"workflow_id":request.workflow_id,"disposition":disposition,"stage_order":stages,"completed_stage_order":sorted(completed),"pending_stage_order":pending,"compensation_order":compensation,"checkpoint_seq":request.checkpoint_seq,"budget_units":request.budget_units,"consumed_units":consumed,"replay_identity":request.replay_identity,"copilot":copilot.value,"effect_receipts":sorted(effects),"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=_digest(payload); payload["workflow_digest"]=digest; payload["artifact"]={"artifact_id":f"retrieval-workflow:{request.workflow_id}","content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}; receipt=RetrievalWorkflowReceipt(payload); receipt.validate(feature_id=feature_id,contract_version=contract_version); return receipt
__all__=["RetrievalWorkflowRequest","RetrievalWorkflowReceipt","schedule","manifest"]
