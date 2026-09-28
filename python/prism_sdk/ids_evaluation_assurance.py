"""Python parity for ``AFA-ids-P23-F25`` evaluation assurance."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ids-P23-F25"; CONTRACT_VERSION="ids-local-evaluation-observability-assurance-harness/1.0"; INPUT_SCHEMA="CapabilityRun7@1"; OUTPUT_SCHEMA="EvaluationCard9@1"; CONTENT_TYPE="application/vnd.aurora.evaluation-card-9+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class EvaluationCard9:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(v.get(k," ").strip() for k in ("request_id","study_id","purpose","semantic_profile","benchmark_id")) or not v.get("metric_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("evaluation identity, locality, metrics, disposition, or effects are incomplete")
        fields=("metric_order","passed_order","failed_order","unknown_order","unmeasured_order","contradicted_order","omitted_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("evaluation ordering is not canonical")
        ids=set(v["metric_order"]);parts=v["passed_order"]+v["failed_order"]+v["unknown_order"]+v["unmeasured_order"]+v["contradicted_order"]+v["omitted_order"]
        if len(ids)!=len(v["metric_order"]) or len(parts)!=len(ids) or set(parts)!=ids or len(v.get("baseline_delta_milli",[]))!=len(ids):raise ResearchContractError("metric states or baseline deltas do not partition")
        if not all(_digest(d) for d in [v.get("replay_identity"),v.get("card_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]) or a.get("content_hash")!=v.get("card_digest") or a.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("evaluation digest or artifact metadata is inconsistent")
        if any(not e.startswith(("measure:evaluation-card:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("evaluation effect is outside governed gate")
def evaluation_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["evaluation engineer","research lead","release auditor","observability operator"],"behavior":"verify typed local metric summaries against a benchmark and emit an omission-aware evaluation card","value":"prevents missing, contradictory, unmeasured, or below-baseline evidence from being reported as a passing capability","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["measure:evaluation-card","manage:local-capability"],"permissions":["read:local-metric-summaries","request:evaluation-assurance"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","study_id","purpose","semantic_profile","benchmark_id")) or not r.get("required_metrics") or not r.get("observations") or len(r["observations"])>16384 or int(r.get("minimum_pass_fraction_milli",0))>1000 or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("evaluation identity, benchmark, metrics, bound, threshold, replay, or locality is invalid")
    if len(set(r["required_metrics"]))!=len(r["required_metrics"]):raise ResearchContractError("required metrics are not unique")
    ids:set[str]=set()
    for o in r["observations"]:
        if not all(isinstance(o.get(k),str) and o[k].strip() for k in ("observation_id","metric_id","benchmark_id")) or not _digest(o.get("provenance_digest")) or not _digest(o.get("replay_identity")) or o["observation_id"] in ids:raise ResearchContractError("observation identity, benchmark, digest, or uniqueness is invalid")
        ids.add(o["observation_id"])
def assure_evaluation(r:Mapping[str,Any])->EvaluationCard9:
    _validate_request(r);obs=sorted((dict(o) for o in r["observations"]),key=lambda o:(o["metric_id"],o["observation_id"]));metrics=sorted(set(r["required_metrics"]));passed:set[str]=set();failed:set[str]=set();unknown:set[str]=set();unmeasured:set[str]=set();contradicted:set[str]=set();omitted:set[str]=set();negative:set[str]=set();loss:set[str]=set();deltas:list[int]=[];prov:set[str]=set()
    for metric in metrics:
        rows=[o for o in obs if o["metric_id"]==metric]
        if not rows:omitted.add(metric);loss.add(f"{metric}:missing-observation");deltas.append(0);continue
        o=rows[0];prov.add(o["provenance_digest"]);deltas.append(int(o.get("value_milli",0))-int(o.get("baseline_milli",0)))
        if o["benchmark_id"]!=r["benchmark_id"]:omitted.add(metric);loss.add(f"{metric}:benchmark-mismatch")
        elif o["replay_identity"]!=r["replay_identity"]:unknown.add(metric);loss.add(f"{metric}:replay-identity")
        elif o.get("evidence_state")=="contradicted":contradicted.add(metric);negative.add(f"{metric}:contradicted")
        elif o.get("evidence_state")=="unknown":unknown.add(metric);loss.add(f"{metric}:unknown")
        elif o.get("evidence_state")=="unmeasured":unmeasured.add(metric);loss.add(f"{metric}:unmeasured")
        elif int(o.get("value_milli",0))>=int(o.get("threshold_milli",0)):passed.add(metric)
        else:failed.add(metric);negative.add(f"{metric}:below-threshold")
    fraction=(len(passed)*1000)//len(metrics) if metrics else 0
    if fraction<int(r["minimum_pass_fraction_milli"]):loss.add(f"request:pass-fraction:{fraction}")
    global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only"))
    if global_block:passed.clear();failed.clear();unknown.clear();unmeasured.clear();contradicted.clear();omitted.update(metrics);loss.add("request:governance-or-locality-denied")
    po,fo,uo,umo,co,oo=(sorted(x) for x in (passed,failed,unknown,unmeasured,contradicted,omitted));disp="blocked" if global_block or (not po and not fo and not uo and not umo and not co and not oo) else "unresolved" if fo or uo or umo or co or oo or fraction<int(r["minimum_pass_fraction_milli"]) else "qualified";loss.add("request:evaluation-not-closed") if disp!="qualified" else None
    p={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"study_id":r["study_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"benchmark_id":r["benchmark_id"],"disposition":disp,"metric_order":metrics,"passed_order":po,"failed_order":fo,"unknown_order":uo,"unmeasured_order":umo,"contradicted_order":co,"omitted_order":oo,"negative_evidence_order":sorted(negative),"baseline_delta_milli":deltas,"pass_fraction_milli":fraction,"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(p);p["card_digest"]=d;p["artifact"]={"artifact_id":f"evaluation-card-9:{r['study_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(loss),"provenance_digests":sorted(prov),"boundary":PRECLINICAL_BOUNDARY};p["effect_receipts"]=sorted([f"measure:evaluation-card:{r['request_id']}",f"manage:local-capability:{r['request_id']}"] if disp=="qualified" else ["block:unsafe-release"]);out=EvaluationCard9(p);out.validate();return out
def idsEvaluationAssuranceDigest(card:EvaluationCard9)->str:card.validate();return _hash(card.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","EvaluationCard9","evaluation_assurance_manifest","assure_evaluation","idsEvaluationAssuranceDigest"]
