"""Deterministic, omission-aware quality control for Worldgen P07 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.quality-control-receipt+json"
_HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class QualityObservation:
    observation_id:str; metric:str; value_milli:int|None; threshold_milli:int|None; state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class QualityControlRequest:
    batch_id:str; consumer:str; observation_order:tuple[str,...]; required_metric_order:tuple[str,...]; observations:tuple[QualityObservation,...]; min_pass_fraction_milli:int; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class QualityControlReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("batch_id") and v.get("consumer") and v.get("observation_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","quality_digest")) and a.get("content_hash")==v.get("quality_digest")): raise ResearchContractError("quality identity, observations, locality, digests, or effects are incomplete")
        for key in ("observation_order","passed_order","failed_order","unknown_order","unmeasured_order","contradicted_order","stale_order","blocked_order","omitted_order","required_metric_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("quality vectors are not canonical")
        ids=set(v["observation_order"]); parts=set(v.get("passed_order",()))|set(v.get("failed_order",()))|set(v.get("unknown_order",()))|set(v.get("unmeasured_order",()))|set(v.get("contradicted_order",()))|set(v.get("stale_order",()))|set(v.get("blocked_order",()))|set(v.get("omitted_order",()))
        if len(ids)!=len(v["observation_order"]) or parts!=ids: raise ResearchContractError("quality observation states do not partition")
        verdict_ids={row["observation_id"] for row in v.get("verdicts",())}; state_ids=set(v.get("passed_order",()))|set(v.get("failed_order",()))|set(v.get("unknown_order",()))|set(v.get("unmeasured_order",()))|set(v.get("contradicted_order",()))|set(v.get("stale_order",()))
        if verdict_ids!=state_ids: raise ResearchContractError("quality verdicts do not match observation states")
        if any(e!="block:unsafe-release" and not e.startswith("assess:worldgen-quality:") for e in v["effect_receipts"]): raise ResearchContractError("quality effect is outside assessment gate")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["imaging core scientist","benchmark curator","research program lead","preclinical neuroscientist"],"behavior":f"assess omission-aware research quality for {scale}","value":"turns typed research observations into witness-bearing quality verdicts without treating unknown or unmeasured evidence as pass","input_schema":input_schema,"output_schema":"QualityVerdict1@1","effects":["assess:worldgen-quality","block:unsafe-release"],"permissions":["assess:local-research-quality"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def assess(request:QualityControlRequest,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->QualityControlReceipt:
    if not(request.batch_id.strip() and request.consumer.strip() and request.observation_order and request.required_metric_order and tuple(request.observation_order)==tuple(sorted(set(request.observation_order))) and tuple(request.required_metric_order)==tuple(sorted(set(request.required_metric_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and 0<=request.min_pass_fraction_milli<=1000 and _HEX.fullmatch(request.replay_identity)): raise ResearchContractError("quality identity, ordering, locality, threshold, boundary, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("quality federation approval is required")
    ids=set(request.observation_order); by_id={}
    for o in request.observations:
        if o.observation_id not in ids or o.boundary!=PRECLINICAL_BOUNDARY or not o.raw_data_local or o.replay_identity!=request.replay_identity or not all(_HEX.fullmatch(getattr(o,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")): raise ResearchContractError("quality observation identity, provenance, locality, replay, or boundary is invalid")
        if o.observation_id in by_id: raise ResearchContractError("duplicate quality observation")
        by_id[o.observation_id]=o
    required=set(request.required_metric_order); passed=set(); failed=set(); unknown=set(); unmeasured=set(); contradicted=set(); stale=set(); blocked=set(); omitted=set(); omissions=set(); uncertainty=set(); negative=set(); verdicts=[]
    for oid in sorted(ids):
        o=by_id.get(oid)
        if o is None: omitted.add(oid); omissions.add(f"observation:{oid}:missing")
        elif not request.policy_allow or not request.protected_closure: blocked.add(oid); omissions.add(f"observation:{oid}:policy-or-closure-blocked")
        elif o.negative_result: unknown.add(oid); negative.add(f"observation:{oid}:negative-result-retained"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"unknown","artifact_digest":o.artifact_digest})
        elif o.state=="stale": stale.add(oid); uncertainty.add(f"observation:{oid}:stale"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"stale","artifact_digest":o.artifact_digest})
        elif o.state=="unmeasured": unmeasured.add(oid); uncertainty.add(f"observation:{oid}:unmeasured"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"unmeasured","artifact_digest":o.artifact_digest})
        elif o.state=="contradicted": contradicted.add(oid); uncertainty.add(f"observation:{oid}:contradicted"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"contradicted","artifact_digest":o.artifact_digest})
        elif o.state=="unknown": unknown.add(oid); uncertainty.add(f"observation:{oid}:unknown"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"unknown","artifact_digest":o.artifact_digest})
        elif not o.metric.strip() or o.metric not in required: unknown.add(oid); uncertainty.add(f"observation:{oid}:metric-not-required"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"unknown","artifact_digest":o.artifact_digest})
        elif o.value_milli is None or o.threshold_milli is None: unmeasured.add(oid); uncertainty.add(f"observation:{oid}:value-or-threshold-unmeasured"); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"unmeasured","artifact_digest":o.artifact_digest})
        else:
            ok=o.value_milli>=o.threshold_milli; (passed if ok else failed).add(oid); verdicts.append({"observation_id":oid,"metric":o.metric,"value_milli":o.value_milli,"state":"pass" if ok else "fail","artifact_digest":o.artifact_digest})
    fraction=(len(passed)*1000)//max(1,len(ids)); authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not passed else "qualified" if len(passed)==len(ids) and fraction>=request.min_pass_fraction_milli and not omissions and not uncertainty and not negative else "partial"; effects=["block:unsafe-release"] if disposition=="blocked" else [f"assess:worldgen-quality:{request.batch_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"batch_id":request.batch_id,"consumer":request.consumer,"scale":scale,"disposition":disposition,"observation_order":sorted(ids),"passed_order":sorted(passed),"failed_order":sorted(failed),"unknown_order":sorted(unknown),"unmeasured_order":sorted(unmeasured),"contradicted_order":sorted(contradicted),"stale_order":sorted(stale),"blocked_order":sorted(blocked),"omitted_order":sorted(omitted),"required_metric_order":sorted(required),"verdicts":verdicts,"pass_fraction_milli":fraction,"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    d=research_artifact_digest(payload); payload["quality_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-quality-verdict:{request.batch_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}; receipt=QualityControlReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","QualityObservation","QualityControlRequest","QualityControlReceipt","manifest","assess"]
