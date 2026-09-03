"""Versioned context-contract compilation for Worldgen P03 F05-F08."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.context-contract-receipt+json";_HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ContextContractRequest:
    request_id:str; source_version:str; target_version:str; offered_field_order:tuple[str,...]; required_field_order:tuple[str,...]; semantic_loss_budget:int; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ContextContractReceipt:
    value:dict[str,Any]
    def validate(self,*,feature_id:str,contract_version:str)->None:
        v,a=self.value,self.value.get("artifact",{});ident=v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version")==contract_version and v.get("feature_id")==feature_id and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and isinstance(v.get("request_id"),str) and v["request_id"].strip() and isinstance(v.get("negotiated_version"),str) and v["negotiated_version"].strip() and v.get("field_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","migration_digest"))
        if not ident:raise ResearchContractError("context contract identity, locality, fields, digests, or effects are incomplete")
        for k in ("field_order","retained_field_order","missing_field_order","omitted_field_order","semantic_loss_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("context contract ordering is not canonical")
        fields=set(v["field_order"]);states=v["retained_field_order"]+v["missing_field_order"]+v["omitted_field_order"]
        if len(fields)!=len(v["field_order"]) or len(states)!=len(fields) or set(states)!=fields or len(set(states))!=len(states):raise ResearchContractError("context contract fields do not partition")
        if any(e not in {"none:context-contract-validation","block:unsafe-release"} for e in v["effect_receipts"]):raise ResearchContractError("context contract effect is outside validation gate")
    def digest(self,*,feature_id:str,contract_version:str)->str:self.validate(feature_id=feature_id,contract_version=contract_version);return _digest(self.value)
def _digest(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _ordered(v:list[str]|tuple[str,...])->bool:return list(v)==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","schema steward","downstream context consumer"],"behavior":f"compile a version-pinned context contract for {scale}","value":"makes migration, semantic loss, and field coverage explicit before context reuse","input_schema":input_schema,"output_schema":"CompiledResearchContext6@1","effects":["none:context-contract-validation","block:unsafe-release"],"permissions":["validate:research-contract"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def compile(request:ContextContractRequest,*,feature_id:str,contract_version:str,require_federation:bool)->ContextContractReceipt:
    if request.boundary!=PRECLINICAL_BOUNDARY or not request.raw_data_local or not request.aggregate_only or not all(isinstance(x,str) and x.strip() for x in (request.request_id,request.source_version,request.target_version)) or not request.offered_field_order or not _ordered(request.offered_field_order) or not _ordered(request.required_field_order) or len(set(request.offered_field_order))!=len(request.offered_field_order) or request.semantic_loss_budget<=0 or not _HEX.fullmatch(request.replay_identity):raise ResearchContractError("context contract identity, field coverage, budget, replay, locality, or boundary is invalid")
    offered=set(request.offered_field_order);required=set(request.required_field_order);missing=required-offered;retained=set(offered);omitted=set();loss=set();disp="accepted"
    if request.source_version!=request.target_version:
        if request.source_version=="0.9.0" and request.target_version=="1.0.0":disp="migrated";loss.add("legacy-fields")
        else:disp="incompatible";retained.clear();loss.add("version-incompatibility")
    if missing:disp="unknown"
    if len(loss)>request.semantic_loss_budget:raise ResearchContractError("semantic-loss budget exceeded")
    fields=offered|required
    if disp in {"incompatible"} or not request.policy_allow or not request.protected_closure or not request.raw_data_local or (require_federation and not request.federation_approved):disp="blocked";retained.clear();omitted.update(fields)
    effects=["block:unsafe-release"] if disp=="blocked" else ["none:context-contract-validation"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"disposition":disp,"negotiated_version":request.target_version,"field_order":sorted(fields),"retained_field_order":sorted(retained),"missing_field_order":sorted(missing),"omitted_field_order":sorted(omitted),"semantic_loss_order":sorted(loss),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_digest(payload);payload["migration_digest"]=d;payload["artifact"]={"artifact_id":f"context-contract:{request.request_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY};out=ContextContractReceipt(payload);out.validate(feature_id=feature_id,contract_version=contract_version);return out
__all__=["CONTENT_TYPE","ContextContractRequest","ContextContractReceipt","manifest","compile"]
