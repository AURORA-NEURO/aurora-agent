"""Deterministic protocol-state simulation for Worldgen P10 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.protocol_simulation-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ProtocolStep:
    step_id:str; action:str; resource_need_milli:int; risk_milli:int; duration_milli:int; compensatable:bool; evidence_state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; permitted:bool=True; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ProtocolDraft:
    request_id:str; protocol_id:str; purpose:str; semantic_profile:str; required_step_order:tuple[str,...]; step_order:tuple[str,...]; steps:tuple[ProtocolStep,...]; replay_identity:str; max_resource_milli:int; max_risk_milli:int; max_duration_milli:int; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ProtocolSimulationReport:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("request_id") and v.get("protocol_id") and v.get("step_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","simulation_digest")) and a.get("content_hash")==v.get("simulation_digest")): raise ResearchContractError("protocol identity, steps, locality, digests, or effects are incomplete")
        for key in ("required_step_order","step_order","admitted_step_order","unresolved_step_order","blocked_step_order","omitted_step_order","failure_order","compensation_order","omissions","uncertainty","contradiction","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("protocol vectors are not canonical")
        ids=set(v["step_order"]); parts=set(v.get("admitted_step_order",()))|set(v.get("unresolved_step_order",()))|set(v.get("blocked_step_order",()))|set(v.get("omitted_step_order",()))
        if len(ids)!=len(v["step_order"]) or parts!=ids or any(len(v.get(k,()))!=len(ids) for k in ("resource_need_milli_order","risk_milli_order","duration_milli_order")): raise ResearchContractError("protocol states or metric vectors do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research program lead","preclinical neuroscientist","bioinformatician","imaging core scientist"],"behavior":f"simulate preclinical protocol state machines for {scale}","value":"preflights resources, risks, failure branches, and compensations without executing protocols or touching instruments","input_schema":input_schema,"output_schema":"ProtocolSimulationReport1@1","effects":["simulate:worldgen-protocol","block:unsafe-release"],"permissions":["simulate:local-preclinical-protocol"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def simulate(request:ProtocolDraft,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->ProtocolSimulationReport:
    if not(request.request_id.strip() and request.protocol_id.strip() and request.purpose.strip() and request.semantic_profile.strip() and request.required_step_order and request.step_order and tuple(request.required_step_order)==tuple(sorted(set(request.required_step_order))) and tuple(request.step_order)==tuple(sorted(set(request.step_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and all(0<=getattr(request,k)<=1000 for k in ("max_resource_milli","max_risk_milli","max_duration_milli")) and _HEX.fullmatch(request.replay_identity)): raise ResearchContractError("protocol identity, ordering, thresholds, locality, boundary, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("protocol federation approval is required")
    ids=set(request.step_order); by_id={}
    for st in request.steps:
        if st.step_id not in ids or not st.action.strip() or st.boundary!=PRECLINICAL_BOUNDARY or not st.raw_data_local or st.replay_identity!=request.replay_identity or any(not 0<=getattr(st,k)<=1000 for k in ("resource_need_milli","risk_milli","duration_milli")) or not all(_HEX.fullmatch(getattr(st,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")): raise ResearchContractError("protocol step identity, metrics, provenance, locality, replay, or boundary is invalid")
        if st.step_id in by_id: raise ResearchContractError("duplicate protocol step")
        by_id[st.step_id]=st
    admitted=[]; unresolved=set(); blocked=set(); omitted=set(); failures=set(); compensation=set(); omissions=set(); uncertainty=set(); contradiction=set(); negative=set()
    for sid in sorted(ids):
        st=by_id.get(sid)
        if st is None: omitted.add(sid); omissions.add(f"step:{sid}:missing")
        elif not request.policy_allow or not request.protected_closure or not st.permitted: blocked.add(sid); failures.add(f"step:{sid}:authorization-denied"); omissions.add(f"step:{sid}:policy-or-permission-blocked")
        elif st.negative_result: unresolved.add(sid); negative.add(f"step:{sid}:negative-result-retained"); compensation.add(f"step:{sid}:compensation-available") if st.compensatable else None
        elif st.evidence_state=="contradicted": unresolved.add(sid); contradiction.add(f"step:{sid}:contradicted"); failures.add(f"step:{sid}:contradiction")
        elif st.evidence_state in {"unknown","unmeasured"}: unresolved.add(sid); uncertainty.add(f"step:{sid}:{st.evidence_state}")
        elif st.resource_need_milli>request.max_resource_milli or st.risk_milli>request.max_risk_milli or st.duration_milli>request.max_duration_milli: unresolved.add(sid); uncertainty.add(f"step:{sid}:resource-risk-duration-threshold"); compensation.add(f"step:{sid}:compensation-available") if st.compensatable else None
        elif st.evidence_state not in {"supported","proven"}: unresolved.add(sid); uncertainty.add(f"step:{sid}:evidence-not-qualified")
        else: admitted.append(st)
    admitted.sort(key=lambda s:s.step_id); admitted_ids={s.step_id for s in admitted}; authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not admitted else "qualified" if admitted_ids==ids and not omissions and not unresolved and not blocked else "partial"; effects=["block:unsafe-release"] if disposition=="blocked" else [f"simulate:worldgen-protocol:{request.protocol_id}"]; ordered=sorted(ids)
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"protocol_id":request.protocol_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"scale":scale,"disposition":disposition,"required_step_order":sorted(request.required_step_order),"step_order":ordered,"admitted_step_order":sorted(admitted_ids),"unresolved_step_order":sorted(unresolved),"blocked_step_order":sorted(blocked),"omitted_step_order":sorted(omitted),"resource_need_milli_order":[by_id[x].resource_need_milli if x in by_id else 0 for x in ordered],"risk_milli_order":[by_id[x].risk_milli if x in by_id else 0 for x in ordered],"duration_milli_order":[by_id[x].duration_milli if x in by_id else 0 for x in ordered],"failure_order":sorted(failures),"compensation_order":sorted(compensation),"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"contradiction":sorted(contradiction),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; d=research_artifact_digest(payload); payload["simulation_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-protocol_simulation:{request.protocol_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}; receipt=ProtocolSimulationReport(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ProtocolStep","ProtocolDraft","ProtocolSimulationReport","manifest","simulate"]
