"""Deterministic, omission-aware resource discovery for Worldgen P05 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.resource-discovery-receipt+json"
_HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ResourceCandidate:
    resource_id:str; resource_type:str; capabilities:tuple[str,...]; fitness_milli:int; availability:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; permitted:bool=True; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ResourceDiscoveryRequest:
    need_id:str; consumer:str; intent:str; required_capability_order:tuple[str,...]; candidate_order:tuple[str,...]; candidates:tuple[ResourceCandidate,...]; max_results:int; minimum_fitness_milli:int; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ResourceDiscoveryReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("need_id") and v.get("consumer") and v.get("required_capability_order") and v.get("candidate_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","discovery_digest")) and a.get("content_hash")==v.get("discovery_digest")): raise ResearchContractError("resource identity, candidates, locality, digests, or effects are incomplete")
        for key in ("required_capability_order","candidate_order","qualified_order","stale_order","protected_order","unavailable_order","unknown_order","omitted_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()))
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("resource vectors are not canonical")
        candidates=set(v["candidate_order"]); parts=set(v.get("qualified_order",()))|set(v.get("stale_order",()))|set(v.get("protected_order",()))|set(v.get("unavailable_order",()))|set(v.get("unknown_order",()))|set(v.get("omitted_order",()))
        if len(candidates)!=len(v["candidate_order"]) or parts!=candidates: raise ResearchContractError("resource candidate states do not partition")
        if {row["resource_id"] for row in v.get("resources",())}!=set(v.get("qualified_order",())): raise ResearchContractError("qualified resources do not match partition")
        if any(e!="block:unsafe-release" and not e.startswith("discover:worldgen-resource:") for e in v["effect_receipts"]): raise ResearchContractError("resource effect is outside discovery gate")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","bioinformatician","imaging core scientist","benchmark curator"],"behavior":f"discover fitness-qualified research resources for {scale}","value":"turns typed resource needs into ranked, auditable, locally governed choices without leaking protected data","input_schema":input_schema,"output_schema":"QualifiedResourceSet1@1","effects":["discover:worldgen-resource","block:unsafe-release"],"permissions":["discover:local-research-resource"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def discover(request:ResourceDiscoveryRequest,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->ResourceDiscoveryReceipt:
    if not(request.need_id.strip() and request.consumer.strip() and request.intent.strip() and request.required_capability_order and request.candidate_order and request.max_results>0 and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and _HEX.fullmatch(request.replay_identity) and tuple(request.required_capability_order)==tuple(sorted(set(request.required_capability_order))) and tuple(request.candidate_order)==tuple(sorted(set(request.candidate_order)))): raise ResearchContractError("resource need, candidates, locality, boundary, ordering, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("resource federation approval is required")
    ids=set(request.candidate_order); by_id={}
    for c in request.candidates:
        if c.resource_id not in ids or c.boundary!=PRECLINICAL_BOUNDARY or not c.raw_data_local or c.replay_identity!=request.replay_identity or not all(_HEX.fullmatch(getattr(c,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")): raise ResearchContractError("resource candidate identity, provenance, replay, locality, or boundary is invalid")
        if c.resource_id in by_id: raise ResearchContractError("duplicate resource candidate")
        by_id[c.resource_id]=c
    required=set(request.required_capability_order); qualified=[]; stale=set(); protected=set(); unavailable=set(); unknown=set(); omitted=set(); omissions=set(); uncertainty=set(); negative=set()
    for rid in sorted(ids):
        c=by_id.get(rid)
        if c is None: omitted.add(rid); omissions.add(f"resource:{rid}:missing")
        elif c.negative_result: unknown.add(rid); negative.add(f"resource:{rid}:negative-result-retained")
        elif not request.policy_allow or not request.protected_closure or not c.permitted: protected.add(rid); omissions.add(f"resource:{rid}:policy-or-permission-blocked")
        elif c.availability=="stale": stale.add(rid); uncertainty.add(f"resource:{rid}:stale")
        elif c.availability!="available": unavailable.add(rid); omissions.add(f"resource:{rid}:unavailable")
        elif c.fitness_milli<request.minimum_fitness_milli or not required.issubset(set(c.capabilities)): unknown.add(rid); uncertainty.add(f"resource:{rid}:fitness-or-capability-below-threshold")
        else: qualified.append(c)
    qualified.sort(key=lambda c:(-c.fitness_milli,c.resource_id)); selected=qualified[:request.max_results]
    for c in qualified[request.max_results:]: omitted.add(c.resource_id); omissions.add(f"resource:{c.resource_id}:max-results")
    selected_ids={c.resource_id for c in selected}; residual=ids-selected_ids-stale-protected-unavailable-unknown; omitted|=residual
    authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved)
    disposition="blocked" if not authority else "unknown" if not selected else "qualified" if len(selected)==len(request.candidate_order) and not omissions and not uncertainty and not negative else "partial"
    resources=[{"resource_id":c.resource_id,"resource_type":c.resource_type,"fitness_milli":c.fitness_milli,"rank":i+1,"capability_order":sorted(required),"artifact_digest":c.artifact_digest} for i,c in enumerate(selected)]
    effects=["block:unsafe-release"] if disposition=="blocked" else [f"discover:worldgen-resource:{request.need_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"need_id":request.need_id,"consumer":request.consumer,"scale":scale,"disposition":disposition,"required_capability_order":sorted(required),"candidate_order":sorted(ids),"qualified_order":sorted(selected_ids),"stale_order":sorted(stale),"protected_order":sorted(protected),"unavailable_order":sorted(unavailable),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"resources":resources,"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY,"raw_candidates":False}
    d=research_artifact_digest(payload); payload["discovery_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-resource-set:{request.need_id}","content_type":CONTENT_TYPE,"content_hash":d,"raw_candidates":False,"boundary":PRECLINICAL_BOUNDARY}
    receipt=ResourceDiscoveryReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ResourceCandidate","ResourceDiscoveryRequest","ResourceDiscoveryReceipt","manifest","discover"]
