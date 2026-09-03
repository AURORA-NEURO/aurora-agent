"""Bounded context-compilation research copilot for Worldgen P03 F09-F12."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
from .worldgen_context_compilation_support import ContextCompilationRequest,compile as compile_context
CONTENT_TYPE="application/vnd.aurora.worldgen.context-copilot-receipt+json";_HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ContextCopilotRequest:
    context_request:ContextCompilationRequest; action_order:tuple[str,...]; action_budget:int; dry_run:bool=False; signed_approval:bool=False; federation_approved:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ContextCopilotReceipt:
    value:dict[str,Any]
    def validate(self,*,feature_id:str,contract_version:str)->None:
        v,a=self.value,self.value.get("artifact",{});ident=v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version")==contract_version and v.get("feature_id")==feature_id and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and isinstance(v.get("request_id"),str) and v["request_id"].strip() and v.get("action_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("context_digest","copilot_digest","replay_identity"))
        if not ident:raise ResearchContractError("context copilot identity, locality, actions, digests, or effects are incomplete")
        for k in ("action_order","admitted_action_order","denied_action_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            if list(v.get(k,[]))!=sorted(set(v.get(k,[]))):raise ResearchContractError("context copilot ordering is not canonical")
        ids=set(v["action_order"]);parts=v["admitted_action_order"]+v["denied_action_order"]
        if len(ids)!=len(v["action_order"]) or len(parts)!=len(ids) or set(parts)!=ids or len(set(parts))!=len(parts):raise ResearchContractError("context copilot actions do not partition")
        if any(e!="block:unsafe-release" and not e.startswith("invoke:bounded-context-tool:") for e in v["effect_receipts"]):raise ResearchContractError("context copilot effect is outside bounded-tool gate")
    def digest(self,*,feature_id:str,contract_version:str)->str:self.validate(feature_id=feature_id,contract_version=contract_version);return _digest(self.value)
def _digest(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _ordered(v:list[str]|tuple[str,...])->bool:return list(v)==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","preclinical neuroscientist","context compiler"],"behavior":f"run bounded context-compilation copilot actions for {scale}","value":"turns typed context compilation into an approval-bounded agent product without hiding omissions","input_schema":input_schema,"output_schema":"CompiledResearchContext6@1","effects":["invoke:bounded-context-tool","block:unsafe-release"],"permissions":["invoke:declared-context-tool"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def run(request:ContextCopilotRequest,*,feature_id:str,contract_version:str,require_approval:bool,require_federation:bool)->ContextCopilotReceipt:
    if request.boundary!=PRECLINICAL_BOUNDARY or request.context_request.boundary!=PRECLINICAL_BOUNDARY or not request.context_request.raw_data_local or not request.context_request.aggregate_only or not request.action_order or not _ordered(request.action_order) or len(set(request.action_order))!=len(request.action_order) or request.action_budget<=0:raise ResearchContractError("context copilot boundary, actions, budget, locality, or boundary is invalid")
    context=compile_context(request.context_request,feature_id=feature_id,contract_version=contract_version,require_federation=require_federation).value;omissions=list(context["omissions"]);uncertainty=list(context["uncertainty"]);negative=list(context["negative_evidence"]);approvals=(not require_approval or request.signed_approval) and (not require_federation or request.federation_approved)
    if not approvals:omissions.append("copilot:approval-missing")
    if request.dry_run:omissions.append("copilot:dry-run-no-effect")
    if len(request.action_order)>request.action_budget:omissions.append("copilot:action-budget-exceeded")
    safe=context["disposition"]!="blocked" and approvals and len(request.action_order)<=request.action_budget;admitted=sorted(request.action_order) if safe else [];denied=[] if safe else sorted(request.action_order);disp="qualified" if safe else "blocked" if context["disposition"]=="blocked" or not approvals else "partial";effects=["block:unsafe-release"] if disp=="blocked" else [f"invoke:bounded-context-tool:{request.context_request.request_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.context_request.request_id,"disposition":disp,"action_order":list(request.action_order),"admitted_action_order":admitted,"denied_action_order":denied,"context_disposition":context["disposition"],"context_digest":context["context_digest"],"replay_identity":context["replay_identity"],"omissions":sorted(set(omissions)),"uncertainty":sorted(set(uncertainty)),"negative_evidence":sorted(set(negative)),"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_digest(payload);payload["copilot_digest"]=d;payload["artifact"]={"artifact_id":f"context-copilot:{request.context_request.request_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY};out=ContextCopilotReceipt(payload);out.validate(feature_id=feature_id,contract_version=contract_version);return out
__all__=["CONTENT_TYPE","ContextCopilotRequest","ContextCopilotReceipt","manifest","run"]
