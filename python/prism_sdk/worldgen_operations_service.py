"""Shared deterministic operations/federation service kernel for Worldgen P01 F29–F32."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

@dataclass(frozen=True)
class OperationsEvent:
    event_id: str; evidence_state: str; provenance_digest: str; permitted: bool = True; retryable: bool = False; negative_result: bool = False

@dataclass(frozen=True)
class OperationsRequest:
    request_id: str; operator: str; scope: str; scale: str; input_schema: str; output_schema: str; events: tuple[OperationsEvent, ...]; capacity: int; budget_units: int; requested_units: int; replay_identity: str; policy_allow: bool = True; protected_closure: bool = True; signed_approval: bool = True; federation_approved: bool = True; raw_data_local: bool = True; aggregate_only: bool = True; boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class OperationsReceipt:
    value: dict[str, Any]
    def validate(self, *, feature_id: str, contract_version: str) -> None:
        v=self.value; a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=contract_version or v.get("feature_id")!=feature_id or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("event_order") or not v.get("effect_receipts"):
            raise ResearchContractError("worldgen operations identity, locality, events, or effects are incomplete")
        for key in ("event_order","admitted_order","blocked_order","unknown_order","recovery_order","telemetry_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("worldgen operations ordering is not canonical")
        ids=set(v["event_order"]); parts=set(v.get("admitted_order",()))|set(v.get("blocked_order",()))|set(v.get("unknown_order",()))
        if parts!=ids: raise ResearchContractError("worldgen operations states do not partition events")
        for value in (v.get("replay_identity"),v.get("capability_digest"),v.get("artifact_digest"),a.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("worldgen operations digest is invalid")
        if a.get("content_hash")!=v.get("artifact_digest"): raise ResearchContractError("worldgen operations artifact digest is inconsistent")
    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id,contract_version=contract_version); return _digest(self.value)

def _digest(value: Any) -> str: return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def manifest(*, feature_id: str, contract_version: str, input_schema: str, output_schema: str, scale: str, autonomy_tier: str) -> dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["benchmark curator","research program lead","preclinical neuroscientist","bioinformatician"],"behavior":f"operate a typed {scale} evidence stream with deterministic telemetry, recovery, capacity, policy, and federation receipts","value":"keeps institution-local research operations observable, replayable, and fail-closed","input_schema":input_schema,"output_schema":output_schema,"autonomy_tier":autonomy_tier,"determinism":"byte_stable","boundary":PRECLINICAL_BOUNDARY}
def operate(request: OperationsRequest, *, feature_id: str, contract_version: str) -> OperationsReceipt:
    if not all(isinstance(x,str) and x.strip() for x in (request.request_id,request.operator,request.scope,request.scale,request.input_schema,request.output_schema)) or not request.events or request.capacity<=0 or len(request.events)>request.capacity or request.requested_units<=0 or request.requested_units>request.budget_units or request.boundary!=PRECLINICAL_BOUNDARY or not request.raw_data_local or not request.aggregate_only or not re.fullmatch(r"[0-9a-f]{64}",request.replay_identity): raise ResearchContractError("worldgen operations request identity, bound, budget, replay, locality, or boundary is invalid")
    events=tuple(sorted(request.events,key=lambda e:e.event_id)); ids=[e.event_id for e in events]
    if len(set(ids))!=len(ids) or any(not e.event_id.strip() or not re.fullmatch(r"[0-9a-f]{64}",e.provenance_digest) for e in events): raise ResearchContractError("worldgen operations events require unique ids and provenance")
    base=request.policy_allow and request.protected_closure and request.signed_approval and (request.scale=="local single-study" or request.federation_approved)
    admitted=[]; blocked=[]; unknown=[]; recovery=[]; omissions=[]; uncertainty=[]; negative=[]
    for e in events:
        if base and e.permitted and e.evidence_state in {"proven","supported"}: admitted.append(e.event_id)
        else:
            blocked.append(e.event_id)
            if e.evidence_state in {"unknown","unmeasured","speculative"}: unknown.append(e.event_id); uncertainty.append(f"event:{e.event_id}:evidence-unresolved")
            if e.retryable: recovery.append(f"event:{e.event_id}:retryable-recovery")
            if e.negative_result: negative.append(f"event:{e.event_id}:negative-result-retained")
    if not request.policy_allow: omissions.append("request:policy-denied")
    if not request.protected_closure: uncertainty.append("request:protected-closure-incomplete")
    if not request.signed_approval: omissions.append("request:signed-approval-missing")
    if request.scale!="local single-study" and not request.federation_approved: omissions.append("request:federation-approval-missing")
    if request.requested_units>request.capacity: omissions.append("request:capacity-exceeded")
    disposition="blocked" if not base or request.requested_units>request.capacity else "qualified" if not blocked and not omissions and not uncertainty and not negative else "partial"
    consumed=min(request.requested_units,request.capacity,request.budget_units)
    capability=_digest({"feature_id":feature_id,"contract_version":contract_version,"scale":request.scale,"input_schema":request.input_schema,"output_schema":request.output_schema})
    telemetry=[f"telemetry:operations:events:{len(ids)}",f"telemetry:operations:units:{consumed}"]
    effects=sorted([f"telemetry:operations:{request.request_id}",f"exchange:permitted-summaries:{request.request_id}"]) if disposition=="qualified" else (sorted([f"recover:operations:{request.request_id}","block:unsafe-release"]) if recovery else ["block:unsafe-release"])
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"operator":request.operator,"scope":request.scope,"scale":request.scale,"input_schema":request.input_schema,"output_schema":request.output_schema,"disposition":disposition,"event_order":ids,"admitted_order":sorted(admitted),"blocked_order":sorted(blocked),"unknown_order":sorted(unknown),"recovery_order":sorted(recovery),"telemetry_order":telemetry,"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"consumed_units":consumed,"capacity":request.capacity,"budget_units":request.budget_units,"replay_identity":request.replay_identity,"capability_digest":capability,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    artifact_digest=_digest(payload); payload["artifact_digest"]=artifact_digest; payload["artifact"]={"artifact_id":f"operations:{request.request_id}","content_type":"application/vnd.aurora.worldgen.operations-receipt+json","content_hash":artifact_digest,"boundary":PRECLINICAL_BOUNDARY}
    receipt=OperationsReceipt(payload); receipt.validate(feature_id=feature_id,contract_version=contract_version); return receipt
