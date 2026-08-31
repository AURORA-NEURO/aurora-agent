"""Omission-aware typed context compilation for Worldgen P03 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.research-context-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ContextFact:
    fact_id:str; statement:str; support_milli:int; state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; negative_result:bool=False; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ContextCompilationRequest:
    request_id:str; objective:str; scope:str; required_fact_order:tuple[str,...]; minimum_support_milli:int; facts:tuple[ContextFact,...]; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ContextCompilationReceipt:
    value:dict[str,Any]
    def validate(self,*,feature_id:str,contract_version:str)->None:
        v,a=self.value,self.value.get("artifact",{}); ident=v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version")==contract_version and v.get("feature_id")==feature_id and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and all(isinstance(v.get(k),str) and v[k].strip() for k in ("request_id","objective","scope")) and v.get("required_fact_order") and v.get("effect_receipts") and v.get("disposition") in {"qualified","partial","unknown","blocked"} and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","context_digest"))
        if not ident: raise ResearchContractError("context identity, boundary, locality, digests, or effects are incomplete")
        keys=("required_fact_order","resolved_fact_order","missing_fact_order","blocked_fact_order","unknown_fact_order","omissions","uncertainty","negative_evidence","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in keys): raise ResearchContractError("context ordering is not canonical")
        required=set(v["required_fact_order"]); parts=v["resolved_fact_order"]+v["missing_fact_order"]+v["blocked_fact_order"]+v["unknown_fact_order"]
        if len(required)!=len(v["required_fact_order"]) or len(parts)!=len(required) or set(parts)!=required or len(set(parts))!=len(parts): raise ResearchContractError("context states do not partition required facts")
        if any(e!="block:unsafe-release" and not e.startswith("compile:worldgen-research-context:") for e in v["effect_receipts"]): raise ResearchContractError("context effect is outside compilation gate")
    def digest(self,*,feature_id:str,contract_version:str)->str:self.validate(feature_id=feature_id,contract_version=contract_version);return _digest(self.value)
def _digest(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _ordered(v:list[str]|tuple[str,...])->bool:return list(v)==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","preclinical neuroscientist","downstream crate context consumer"],"behavior":f"compile omission-aware typed research context for {scale}","value":"turns retrieval evidence into reusable context without inventing missing or protected facts","input_schema":input_schema,"output_schema":"CompiledResearchContext6@1","effects":["compile:worldgen-research-context","block:unsafe-release"],"permissions":["compile:local-research-context"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def compile(request:ContextCompilationRequest,*,feature_id:str,contract_version:str,require_federation:bool)->ContextCompilationReceipt:
    if request.boundary!=PRECLINICAL_BOUNDARY or not request.raw_data_local or not request.aggregate_only or not all(isinstance(x,str) and x.strip() for x in (request.request_id,request.objective,request.scope)) or not request.required_fact_order or not _ordered(request.required_fact_order) or len(set(request.required_fact_order))!=len(request.required_fact_order) or not _HEX.fullmatch(request.replay_identity): raise ResearchContractError("context identity, required facts, replay, locality, or boundary is invalid")
    required=set(request.required_fact_order); facts={f.fact_id:f for f in request.facts}; resolved=set();missing=set();blocked=set();unknown=set();omissions=set();uncertainty=set();negative=set()
    for fid in required:
        f=facts.get(fid)
        if f is None: missing.add(fid);omissions.add(f"fact:{fid}:missing")
        elif f.negative_result: blocked.add(fid);negative.add(f"fact:{fid}:negative-result-retained")
        elif not request.policy_allow or not request.protected_closure or not f.raw_data_local or f.boundary!=PRECLINICAL_BOUNDARY: blocked.add(fid);omissions.add(f"fact:{fid}:policy-or-locality-blocked")
        elif f.replay_identity!=request.replay_identity: unknown.add(fid);uncertainty.add(f"fact:{fid}:replay-mismatch")
        elif f.state=="supported" and f.support_milli>=request.minimum_support_milli: resolved.add(fid)
        elif f.state in {"unknown","speculative"}: unknown.add(fid);uncertainty.add(f"fact:{fid}:state-unknown")
        else: blocked.add(fid);omissions.add(f"fact:{fid}:unsupported-or-below-threshold")
    if require_federation and not request.federation_approved: omissions.add("request:federation-approval-missing")
    authority=request.policy_allow and request.protected_closure and request.raw_data_local and (not require_federation or request.federation_approved)
    disp="blocked" if not authority else "unknown" if not resolved else "qualified" if len(resolved)==len(required) and not omissions and not uncertainty and not negative else "partial"; effects=["block:unsafe-release"] if disp=="blocked" else [f"compile:worldgen-research-context:{request.request_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"objective":request.objective,"scope":request.scope,"disposition":disp,"required_fact_order":sorted(required),"resolved_fact_order":sorted(resolved),"missing_fact_order":sorted(missing),"blocked_fact_order":sorted(blocked),"unknown_fact_order":sorted(unknown),"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=_digest(payload);payload["context_digest"]=digest;payload["artifact"]={"artifact_id":f"worldgen-research-context:{request.request_id}","content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY};out=ContextCompilationReceipt(payload);out.validate(feature_id=feature_id,contract_version=contract_version);return out
__all__=["CONTENT_TYPE","ContextFact","ContextCompilationRequest","ContextCompilationReceipt","manifest","compile"]
