"""Release assurance gates for Worldgen retrieval-synthesis products (P02 F25–F28)."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
from .worldgen_retrieval_support import infer
from .worldgen_retrieval_copilot_support import RetrievalCopilotRequest
CONTENT_TYPE="application/vnd.aurora.worldgen.retrieval-assurance-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class RetrievalAssuranceRequest:
    query:Any; minimum_selected:int; require_signed_approval:bool; signed_approval:bool; require_federation:bool; federation_approved:bool; boundary:str
@dataclass(frozen=True)
class RetrievalAssuranceReceipt:
    value:dict[str,Any]
    def validate(self,*,feature_id:str,contract_version:str)->None:
        v=self.value;a=v.get("artifact",{});ids=tuple(v.get("candidate_order",()));parts=tuple(v.get("selected_order",()))+tuple(v.get("unresolved_order",()))+tuple(v.get("blocked_order",()))
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=contract_version or v.get("feature_id")!=feature_id or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("request_id","").strip() or not ids or not v.get("gate_order") or not v.get("effect_receipts") or not all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","synthesis_digest","assurance_digest")): raise ResearchContractError("retrieval assurance identity, gates, locality, digests, or effects are incomplete")
        for k in ("candidate_order","selected_order","unresolved_order","blocked_order","gate_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            vals=tuple(v.get(k,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("retrieval assurance ordering is not canonical")
        if set(parts)!=set(ids) or len(parts)!=len(ids) or len(set(ids))!=len(ids): raise ResearchContractError("retrieval assurance states do not partition candidates")
        if any(e not in {"retain:qualified-retrieval-assurance","block:unsafe-release"} for e in v["effect_receipts"]): raise ResearchContractError("retrieval assurance effect is outside release gate")
    def digest(self,*,feature_id:str,contract_version:str)->str:self.validate(feature_id=feature_id,contract_version=contract_version);return _digest(self.value)
def _digest(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","benchmark curator","preclinical neuroscientist"],"behavior":f"assure retrieval-synthesis release for {scale} under evidence and provenance gates","value":"prevents incomplete or unsafe retrieval synthesis from being released as qualified research evidence","input_schema":input_schema,"output_schema":"QualifiedEvidenceSet7@1","effects":["retain:qualified-retrieval-assurance","block:unsafe-release"],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def assure(request:RetrievalAssuranceRequest,*,feature_id:str,contract_version:str,require_approval:bool,require_federation:bool)->RetrievalAssuranceReceipt:
    if request.boundary!=PRECLINICAL_BOUNDARY or request.query.boundary!=PRECLINICAL_BOUNDARY or not request.query.raw_data_local or not request.query.aggregate_only or request.minimum_selected<=0:raise ResearchContractError("retrieval assurance boundary, locality, aggregate-only, or minimum selection is invalid")
    syn=infer(request.query,feature_id=feature_id,contract_version=contract_version);gates=["evidence-state-qualified","replay-identity-matched","provenance-digests-present","raw-data-local"];omissions=[];uncertainty=[]
    if len(syn.value["selected_order"])<request.minimum_selected:gates.append("minimum-selected-failed");omissions.append("assurance:minimum-selected-not-met")
    if require_approval and not request.signed_approval:gates.append("signed-approval-missing");omissions.append("assurance:signed-approval-missing")
    if require_federation and not request.federation_approved:gates.append("federation-approval-missing");omissions.append("assurance:federation-approval-missing")
    if syn.value["disposition"]!="qualified":gates.append("synthesis-not-qualified");uncertainty.append("underlying synthesis remains partial, unknown, or blocked")
    gates=sorted(set(gates));omissions=sorted(set(omissions)|set(syn.value["omission_order"]));uncertainty=sorted(set(uncertainty)|set(syn.value["uncertainty_order"]));negative=sorted(set(syn.value["negative_evidence_order"]));safe=syn.value["disposition"]=="qualified" and len(syn.value["selected_order"])>=request.minimum_selected and (not require_approval or request.signed_approval) and (not require_federation or request.federation_approved);effect="retain:qualified-retrieval-assurance" if safe else "block:unsafe-release";payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.query.request_id,"disposition":"qualified" if safe else "blocked","candidate_order":syn.value["candidate_order"],"selected_order":syn.value["selected_order"],"unresolved_order":syn.value["unresolved_order"],"blocked_order":syn.value["blocked_order"],"gate_order":gates,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"replay_identity":request.query.replay_identity,"synthesis_digest":syn.value["synthesis_digest"],"effect_receipts":[effect],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};payload["assurance_digest"]=_digest(payload);payload["artifact"]={"artifact_id":f"retrieval-assurance:{request.query.request_id}","content_type":CONTENT_TYPE,"content_hash":_digest(payload),"boundary":PRECLINICAL_BOUNDARY};receipt=RetrievalAssuranceReceipt(payload);receipt.validate(feature_id=feature_id,contract_version=contract_version);return receipt
__all__=["RetrievalAssuranceRequest","RetrievalAssuranceReceipt","assure","manifest"]
