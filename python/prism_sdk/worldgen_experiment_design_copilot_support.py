"""Approval-bounded experiment-design copilot for Worldgen P08 F09-F12."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_experiment_design_support import ExperimentDesignQuestion, design
CONTENT_TYPE="application/vnd.aurora.worldgen.experiment-design-copilot-receipt+json"
@dataclass(frozen=True)
class ExperimentDesignCopilotRequest:
    design_request:ExperimentDesignQuestion; action_order:tuple[str,...]; action_budget:int; dry_run:bool=False; signed_approval:bool=False; federation_approved:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ExperimentDesignCopilotReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("question_id") and v.get("action_order") and v.get("effect_receipts") and all(isinstance(v.get(k),str) and len(v[k])==64 and all(c in "0123456789abcdef" for c in v[k]) for k in ("portfolio_digest","copilot_digest","replay_identity")) and a.get("content_hash")==v.get("copilot_digest")): raise ResearchContractError("experiment design copilot identity, actions, locality, digests, or effects are incomplete")
        for key in ("action_order","admitted_action_order","denied_action_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()))
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("experiment design copilot vectors are not canonical")
        actions=set(v["action_order"]); parts=set(v.get("admitted_action_order",()))|set(v.get("denied_action_order",()))
        if len(actions)!=len(v["action_order"]) or parts!=actions: raise ResearchContractError("experiment design copilot actions do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["experiment_design steward","preclinical researcher","workflow operator"],"behavior":f"run bounded experiment-design actions for {scale}","value":"turns ranked experiment_design observations into an approval-bounded product without exposing protected experiment_design observations","input_schema":input_schema,"output_schema":"ExperimentDesignCopilotReceipt1@1","effects":["invoke:bounded-experiment_design-tool","block:unsafe-release"],"permissions":["invoke:declared-experiment_design-tool"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def run(request:ExperimentDesignCopilotRequest,*,feature_id:str,contract_version:str,scale:str,require_approval:bool=False,require_federation:bool=False)->ExperimentDesignCopilotReceipt:
    if not(request.boundary==PRECLINICAL_BOUNDARY and request.action_budget>0 and request.action_order and tuple(request.action_order)==tuple(sorted(set(request.action_order))) and request.design_request.raw_data_local and request.design_request.aggregate_only): raise ResearchContractError("experiment design copilot request is invalid")
    d=design(request.design_request,feature_id=feature_id,contract_version=contract_version,scale=scale,require_federation=require_federation); omissions=list(d.value["omissions"]); approved=(not require_approval or request.signed_approval) and (not require_federation or request.federation_approved)
    if not approved: omissions.append("copilot:approval-missing")
    if request.dry_run: omissions.append("copilot:dry-run-no-effect")
    if len(request.action_order)>request.action_budget: omissions.append("copilot:action-budget-exceeded")
    safe=d.value["disposition"]=="qualified" and approved and len(request.action_order)<=request.action_budget; disposition="qualified" if safe else "blocked" if d.value["disposition"]=="blocked" or not approved else "partial"; admitted=list(request.action_order) if safe else []; denied=[] if safe else list(request.action_order); effects=[f"invoke:bounded-experiment_design-tool:{request.design_request.question_id}"] if disposition!="blocked" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"question_id":request.design_request.question_id,"disposition":disposition,"action_order":sorted(request.action_order),"admitted_action_order":sorted(admitted),"denied_action_order":sorted(denied),"experiment_design_disposition":d.value["disposition"],"portfolio_digest":d.value["portfolio_digest"],"copilot_digest":"","replay_identity":d.value["replay_identity"],"omissions":sorted(set(omissions)),"uncertainty":d.value["uncertainty"],"negative_evidence":d.value["negative_evidence"],"effect_receipts":sorted(effects),"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    digest_payload=dict(payload); digest_payload.pop("copilot_digest",None); digest=research_artifact_digest(digest_payload); payload["copilot_digest"]=digest; payload["artifact"]={"artifact_id":f"experiment_design-copilot:{request.design_request.question_id}","content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}; receipt=ExperimentDesignCopilotReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ExperimentDesignCopilotRequest","ExperimentDesignCopilotReceipt","manifest","run"]




