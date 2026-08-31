"""Deterministic power-aware experiment design for Worldgen P09 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTENT_TYPE="application/vnd.aurora.worldgen.experiment-design-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ExperimentDesignCandidate:
    design_id:str; objective:str; sample_size:int; power_milli:int; variance_milli:int; attrition_milli:int; replication_milli:int; evidence_state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; permitted:bool=True; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ExperimentDesignQuestion:
    request_id:str; objective:str; purpose:str; semantic_profile:str; required_design_order:tuple[str,...]; candidate_order:tuple[str,...]; candidates:tuple[ExperimentDesignCandidate,...]; replay_identity:str; min_power_milli:int; max_variance_milli:int; max_attrition_milli:int; min_replication_milli:int; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ExperimentDesignPortfolio:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("request_id") and v.get("objective") and v.get("candidate_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","portfolio_digest")) and a.get("content_hash")==v.get("portfolio_digest")): raise ResearchContractError("design identity, candidates, locality, digests, or effects are incomplete")
        for key in ("required_design_order","candidate_order","selected_order","unresolved_order","blocked_order","omitted_order","omissions","uncertainty","contradiction","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("experiment design vectors are not canonical")
        ids=set(v["candidate_order"]); parts=set(v.get("selected_order",()))|set(v.get("unresolved_order",()))|set(v.get("blocked_order",()))|set(v.get("omitted_order",()))
        if len(ids)!=len(v["candidate_order"]) or parts!=ids or any(len(v.get(k,()))!=len(ids) for k in ("power_milli_order","variance_milli_order","attrition_milli_order","replication_milli_order")): raise ResearchContractError("design states or score vectors do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","preclinical neuroscientist","bioinformatician","imaging core scientist"],"behavior":f"compute power-aware preclinical experiment designs for {scale}","value":"makes power, variance, attrition, replication, evidence, omissions, and authority explicit before a study is proposed","input_schema":input_schema,"output_schema":"ExecutableExperimentDesign1@1","effects":["design:worldgen-experiment","block:unsafe-release"],"permissions":["design:local-preclinical-study"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def design(request:ExperimentDesignQuestion,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->ExperimentDesignPortfolio:
    if not(request.request_id.strip() and request.objective.strip() and request.purpose.strip() and request.semantic_profile.strip() and request.required_design_order and request.candidate_order and tuple(request.required_design_order)==tuple(sorted(set(request.required_design_order))) and tuple(request.candidate_order)==tuple(sorted(set(request.candidate_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and 0<=request.min_power_milli<=1000 and 0<=request.max_variance_milli<=1000 and 0<=request.max_attrition_milli<=1000 and 0<=request.min_replication_milli<=1000 and _HEX.fullmatch(request.replay_identity)): raise ResearchContractError("design identity, ordering, thresholds, locality, boundary, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("experiment design federation approval is required")
    ids=set(request.candidate_order); by_id={}
    for c in request.candidates:
        if c.design_id not in ids or not c.objective.strip() or c.sample_size<=0 or c.boundary!=PRECLINICAL_BOUNDARY or not c.raw_data_local or c.replay_identity!=request.replay_identity or any(c.__dict__[k]<0 or c.__dict__[k]>1000 for k in ("power_milli","variance_milli","attrition_milli","replication_milli")) or not all(_HEX.fullmatch(getattr(c,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")): raise ResearchContractError("design candidate identity, metrics, provenance, locality, replay, or boundary is invalid")
        if c.design_id in by_id: raise ResearchContractError("duplicate experiment design candidate")
        by_id[c.design_id]=c
    selected=[]; unresolved=set(); blocked=set(); omitted=set(); omissions=set(); uncertainty=set(); contradiction=set(); negative=set()
    for did in sorted(ids):
        c=by_id.get(did)
        if c is None: omitted.add(did); omissions.add(f"design:{did}:missing")
        elif not request.policy_allow or not request.protected_closure or not c.permitted: blocked.add(did); omissions.add(f"design:{did}:policy-or-permission-blocked")
        elif c.negative_result: unresolved.add(did); negative.add(f"design:{did}:negative-result-retained")
        elif c.evidence_state=="contradicted": unresolved.add(did); contradiction.add(f"design:{did}:contradicted")
        elif c.evidence_state in {"unknown","unmeasured"}: unresolved.add(did); uncertainty.add(f"design:{did}:{c.evidence_state}")
        elif c.power_milli<request.min_power_milli or c.variance_milli>request.max_variance_milli or c.attrition_milli>request.max_attrition_milli or c.replication_milli<request.min_replication_milli: unresolved.add(did); uncertainty.add(f"design:{did}:threshold-not-met")
        elif c.evidence_state not in {"supported","proven"}: unresolved.add(did); uncertainty.add(f"design:{did}:evidence-not-qualified")
        else: selected.append(c)
    selected.sort(key=lambda c:(-c.power_milli,c.variance_milli,c.attrition_milli,-c.replication_milli,c.design_id)); selected_ids={c.design_id for c in selected}; authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not selected else "qualified" if selected_ids==ids and not omissions and not unresolved and not blocked else "partial"; effects=["block:unsafe-release"] if disposition=="blocked" else [f"design:worldgen-experiment:{request.objective}"]; ordered_ids=sorted(ids)
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"objective":request.objective,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"scale":scale,"disposition":disposition,"required_design_order":sorted(request.required_design_order),"candidate_order":ordered_ids,"selected_order":sorted(selected_ids),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"omitted_order":sorted(omitted),"power_milli_order":[by_id[x].power_milli if x in by_id else 0 for x in ordered_ids],"variance_milli_order":[by_id[x].variance_milli if x in by_id else 0 for x in ordered_ids],"attrition_milli_order":[by_id[x].attrition_milli if x in by_id else 0 for x in ordered_ids],"replication_milli_order":[by_id[x].replication_milli if x in by_id else 0 for x in ordered_ids],"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"contradiction":sorted(contradiction),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; d=research_artifact_digest(payload); payload["portfolio_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-experiment-design:{request.objective}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}; receipt=ExperimentDesignPortfolio(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ExperimentDesignCandidate","ExperimentDesignQuestion","ExperimentDesignPortfolio","manifest","design"]
