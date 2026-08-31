"""Approval-bounded computational-execution copilot for Worldgen P12."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_computational_execution_support import assure_computational_execution
CONTENT_TYPE="application/vnd.aurora.worldgen.execution-copilot-receipt+json"
@dataclass(frozen=True)
class ExecutionCopilotRequest:
    execution_request:Mapping[str,Any]; action_order:tuple[str,...]; action_budget:int; dry_run:bool=False; signed_approval:bool=False; federation_approved:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ExecutionCopilotReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("action_order") and a.get("content_hash")==v.get("copilot_digest")): raise ResearchContractError("execution copilot receipt is invalid")
        ids=set(v["action_order"]); parts=set(v.get("admitted_action_order",()))|set(v.get("denied_action_order",()));
        if ids!=parts or v["action_order"]!=sorted(set(v["action_order"])): raise ResearchContractError("execution copilot actions do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["bioinformatician","workflow operator"],"behavior":f"run bounded execution actions for {scale}","value":"turns replayable execution runs into an approval-bounded product","input_schema":input_schema,"output_schema":"ExecutionCopilotReceipt1@1","effects":["invoke:bounded-execution-tool","block:unsafe-release"],"permissions":["invoke:declared-execution-tool"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def run(request:ExecutionCopilotRequest,*,feature_id:str,contract_version:str,scale:str,require_approval:bool=False,require_federation:bool=False)->ExecutionCopilotReceipt:
    if request.boundary!=PRECLINICAL_BOUNDARY or not request.action_order or request.action_order!=tuple(sorted(set(request.action_order))) or request.action_budget<=0: raise ResearchContractError("execution copilot request is invalid")
    d=assure_computational_execution(request.execution_request,feature_id=feature_id,contract_version=contract_version); approved=(not require_approval or request.signed_approval) and (not require_federation or request.federation_approved); omissions=list(d.value.get("omissions",()))
    if not approved: omissions.append("copilot:approval-missing")
    if request.dry_run: omissions.append("copilot:dry-run-no-effect")
    if len(request.action_order)>request.action_budget: omissions.append("copilot:action-budget-exceeded")
    safe=d.value["disposition"]=="qualified" and approved and len(request.action_order)<=request.action_budget; disposition="qualified" if safe else "blocked" if d.value["disposition"]=="blocked" or not approved else "partial"; admitted=list(request.action_order) if safe else []; denied=[] if safe else list(request.action_order); effects=[f"invoke:bounded-execution-tool:{request.execution_request['run_id']}"] if disposition!="blocked" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.execution_request["request_id"],"disposition":disposition,"action_order":sorted(request.action_order),"admitted_action_order":sorted(admitted),"denied_action_order":sorted(denied),"execution_disposition":d.value["disposition"],"run_digest":d.value["run_digest"],"replay_identity":d.value["replay_identity"],"omissions":sorted(set(omissions)),"uncertainty":d.value["uncertainty"],"negative_evidence":d.value["negative_evidence"],"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=research_artifact_digest(payload); payload["copilot_digest"]=digest; payload["artifact"]={"content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}; out=ExecutionCopilotReceipt(payload); out.validate(); return out
__all__=["CONTENT_TYPE","ExecutionCopilotRequest","ExecutionCopilotReceipt","manifest","run"]
