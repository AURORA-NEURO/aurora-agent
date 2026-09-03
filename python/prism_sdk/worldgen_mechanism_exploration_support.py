"""Deterministic competing-mechanism exploration for Worldgen P08 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.mechanism-exploration-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class MechanismCandidate:
    mechanism_id:str; statement:str; support_milli:int; novelty_milli:int; evidence_state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; competing_order:tuple[str,...]=(); permitted:bool=True; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class MechanismQuestion:
    request_id:str; question_id:str; purpose:str; semantic_profile:str; required_mechanism_order:tuple[str,...]; candidate_order:tuple[str,...]; candidates:tuple[MechanismCandidate,...]; replay_identity:str; min_support_milli:int; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class MechanismPortfolio:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("request_id") and v.get("question_id") and v.get("candidate_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","portfolio_digest")) and a.get("content_hash")==v.get("portfolio_digest")): raise ResearchContractError("mechanism identity, candidates, locality, digests, or effects are incomplete")
        for key in ("required_mechanism_order","candidate_order","selected_order","unresolved_order","blocked_order","omitted_order","competing_order","omissions","uncertainty","contradiction","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("mechanism vectors are not canonical")
        ids=set(v["candidate_order"]); parts=set(v.get("selected_order",()))|set(v.get("unresolved_order",()))|set(v.get("blocked_order",()))|set(v.get("omitted_order",()))
        if len(ids)!=len(v["candidate_order"]) or parts!=ids or len(v.get("support_milli_order",()))!=len(ids) or len(v.get("novelty_milli_order",()))!=len(ids): raise ResearchContractError("mechanism states or score vectors do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["imaging core scientist","research program lead","preclinical neuroscientist","bioinformatician"],"behavior":f"rank competing preclinical mechanisms for {scale}","value":"preserves competing explanations, disagreement, uncertainty, provenance, and negative evidence instead of inventing a single mechanism","input_schema":input_schema,"output_schema":"MechanismPortfolio1@1","effects":["explore:worldgen-mechanism","block:unsafe-release"],"permissions":["explore:local-research-mechanism"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def explore(request:MechanismQuestion,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->MechanismPortfolio:
    if not(request.request_id.strip() and request.question_id.strip() and request.purpose.strip() and request.semantic_profile.strip() and request.required_mechanism_order and request.candidate_order and tuple(request.required_mechanism_order)==tuple(sorted(set(request.required_mechanism_order))) and tuple(request.candidate_order)==tuple(sorted(set(request.candidate_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and 0<=request.min_support_milli<=1000 and _HEX.fullmatch(request.replay_identity)): raise ResearchContractError("mechanism identity, ordering, threshold, locality, boundary, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("mechanism federation approval is required")
    ids=set(request.candidate_order); by_id={}
    for c in request.candidates:
        if c.mechanism_id not in ids or c.boundary!=PRECLINICAL_BOUNDARY or not c.raw_data_local or c.replay_identity!=request.replay_identity or not all(_HEX.fullmatch(getattr(c,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")): raise ResearchContractError("mechanism candidate identity, provenance, locality, replay, or boundary is invalid")
        if c.mechanism_id in by_id: raise ResearchContractError("duplicate mechanism candidate")
        by_id[c.mechanism_id]=c
    req=set(request.required_mechanism_order); selected=[]; unresolved=set(); blocked=set(); omitted=set(); omissions=set(); uncertainty=set(); contradiction=set(); negative=set(); competing=set()
    for mid in sorted(ids):
        c=by_id.get(mid)
        if c is None: omitted.add(mid); omissions.add(f"mechanism:{mid}:missing")
        elif not request.policy_allow or not request.protected_closure or not c.permitted: blocked.add(mid); omissions.add(f"mechanism:{mid}:policy-or-permission-blocked")
        elif c.negative_result: unresolved.add(mid); negative.add(f"mechanism:{mid}:negative-result-retained"); competing.update(c.competing_order)
        elif c.evidence_state=="contradicted": unresolved.add(mid); contradiction.add(f"mechanism:{mid}:contradicted"); competing.update(c.competing_order)
        elif c.evidence_state in {"unknown","unmeasured"}: unresolved.add(mid); uncertainty.add(f"mechanism:{mid}:{c.evidence_state}"); competing.update(c.competing_order)
        elif c.support_milli<request.min_support_milli: unresolved.add(mid); uncertainty.add(f"mechanism:{mid}:support-below-threshold"); competing.update(c.competing_order)
        else: selected.append(c); competing.update(c.competing_order)
    selected.sort(key=lambda c:(-c.support_milli,-c.novelty_milli,c.mechanism_id)); selected_ids={c.mechanism_id for c in selected}; authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not selected else "qualified" if selected_ids==ids and not omissions and not unresolved and not blocked else "partial"; effects=["block:unsafe-release"] if disposition=="blocked" else [f"explore:worldgen-mechanism:{request.question_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"question_id":request.question_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"scale":scale,"disposition":disposition,"required_mechanism_order":sorted(req),"candidate_order":sorted(ids),"selected_order":sorted(selected_ids),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"omitted_order":sorted(omitted),"support_milli_order":[by_id[x].support_milli if x in by_id else 0 for x in sorted(ids)],"novelty_milli_order":[by_id[x].novelty_milli if x in by_id else 0 for x in sorted(ids)],"competing_order":sorted(competing),"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"contradiction":sorted(contradiction),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; d=research_artifact_digest(payload); payload["portfolio_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-mechanism-portfolio:{request.question_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}; receipt=MechanismPortfolio(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","MechanismCandidate","MechanismQuestion","MechanismPortfolio","manifest","explore"]
