"""Deterministic, approval-gated instrument preflight for Worldgen P11."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.laboratory-integration-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class InstrumentAction:
    action_id:str; instrument_id:str; operation:str; modality:str; interlock_order:tuple[str,...]; signed_preflight:bool; operator_authorized:bool; effect_tier:int; evidence_state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; permitted:bool=True; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class InstrumentActionRequest:
    request_id:str; study_id:str; purpose:str; semantic_profile:str; required_action_order:tuple[str,...]; action_order:tuple[str,...]; actions:tuple[InstrumentAction,...]; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; emergency_stop:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class InstrumentActionReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("request_id") and v.get("study_id") and v.get("action_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","receipt_digest")) and a.get("content_hash")==v.get("receipt_digest")): raise ResearchContractError("instrument identity, locality, digests, or effects are incomplete")
        for key in ("required_action_order","action_order","admitted_action_order","unresolved_action_order","blocked_action_order","omitted_action_order","interlock_order","failure_order","omissions","uncertainty","contradiction","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("instrument vectors are not canonical")
        ids=set(v["action_order"]); parts=set(v.get("admitted_action_order",()))|set(v.get("unresolved_action_order",()))|set(v.get("blocked_action_order",()))|set(v.get("omitted_action_order",()))
        if len(ids)!=len(v["action_order"]) or parts!=ids: raise ResearchContractError("instrument action states do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","imaging core scientist","robotics operator","research program lead"],"behavior":f"preflight signed instrument actions for {scale}","value":"binds instrument effects to interlocks, authorization, emergency stops, provenance, and local-only raw data","input_schema":input_schema,"output_schema":"InstrumentActionReceipt1@1","effects":["preflight:worldgen-instrument","block:unsafe-release"],"permissions":["preflight:instrument-action"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def integrate(request:InstrumentActionRequest,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->InstrumentActionReceipt:
    if not(request.request_id.strip() and request.study_id.strip() and request.purpose.strip() and request.semantic_profile.strip() and request.action_order and tuple(request.action_order)==tuple(sorted(set(request.action_order))) and request.required_action_order and tuple(request.required_action_order)==tuple(sorted(set(request.required_action_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and _HEX.fullmatch(request.replay_identity)): raise ResearchContractError("instrument identity, ordering, locality, boundary, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("instrument federation approval is required")
    ids=set(request.action_order); by_id={}
    for action in request.actions:
        if action.action_id not in ids or not(action.instrument_id.strip() and action.operation.strip() and action.modality.strip() and action.interlock_order and tuple(action.interlock_order)==tuple(sorted(set(action.interlock_order))) and action.boundary==PRECLINICAL_BOUNDARY and action.raw_data_local and action.replay_identity==request.replay_identity and 0<=action.effect_tier<=4 and all(_HEX.fullmatch(getattr(action,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity"))): raise ResearchContractError("instrument action identity, interlocks, provenance, locality, replay, or boundary is invalid")
        if action.action_id in by_id: raise ResearchContractError("duplicate instrument action")
        by_id[action.action_id]=action
    admitted=[]; unresolved=set(); blocked=set(); omitted=set(); interlocks=set(); failures=set(); omissions=set(); uncertainty=set(); contradiction=set(); negative=set()
    for aid in sorted(ids):
        action=by_id.get(aid)
        if action is None: omitted.add(aid); omissions.add(f"action:{aid}:missing")
        elif request.emergency_stop: blocked.add(aid); failures.add(f"action:{aid}:emergency-stop"); omissions.add(f"action:{aid}:emergency-stop-active")
        elif not request.policy_allow or not request.protected_closure or not action.permitted: blocked.add(aid); failures.add(f"action:{aid}:policy-or-permission-denied"); omissions.add(f"action:{aid}:policy-or-permission-blocked")
        elif action.effect_tier>=3 and (not action.signed_preflight or not action.operator_authorized): blocked.add(aid); failures.add(f"action:{aid}:signed-preflight-or-operator-authorization-required")
        elif action.negative_result: unresolved.add(aid); negative.add(f"action:{aid}:negative-result-retained"); interlocks.update(action.interlock_order)
        elif action.evidence_state=="contradicted": unresolved.add(aid); contradiction.add(f"action:{aid}:contradicted"); interlocks.update(action.interlock_order)
        elif action.evidence_state in {"unknown","unmeasured"}: unresolved.add(aid); uncertainty.add(f"action:{aid}:{action.evidence_state}"); interlocks.update(action.interlock_order)
        else: admitted.append(action); interlocks.update(action.interlock_order)
    admitted_ids={a.action_id for a in admitted}; authority=request.policy_allow and request.protected_closure and not request.emergency_stop and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not admitted else "qualified" if admitted_ids==ids and not unresolved and not blocked and not omissions else "partial"; effects=["block:unsafe-release"] if disposition=="blocked" else [f"preflight:worldgen-instrument:{request.study_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"study_id":request.study_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"scale":scale,"disposition":disposition,"required_action_order":sorted(request.required_action_order),"action_order":sorted(ids),"admitted_action_order":sorted(admitted_ids),"unresolved_action_order":sorted(unresolved),"blocked_action_order":sorted(blocked),"omitted_action_order":sorted(omitted),"interlock_order":sorted(interlocks),"failure_order":sorted(failures),"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"contradiction":sorted(contradiction),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    d=research_artifact_digest(payload); payload["receipt_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-instrument-preflight:{request.study_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}; receipt=InstrumentActionReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","InstrumentAction","InstrumentActionRequest","InstrumentActionReceipt","manifest","integrate"]
