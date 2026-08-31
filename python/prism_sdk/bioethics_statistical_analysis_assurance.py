"""Python parity for ``AFA-bioethics-P13-F27`` analysis assurance."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-bioethics-P13-F27"; CONTRACT_VERSION="bioethics-prospective-statistical-causal-ml-analysis-assurance-harness/1.0"; INPUT_SCHEMA="AnalysisQuestion3@1"; OUTPUT_SCHEMA="QualifiedAnalysisResult7@1"; CONTENT_TYPE="application/vnd.aurora.bioethics-qualified-analysis-result-7+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class QualifiedAnalysisResult7:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("disposition") not in {"qualified","partial","blocked"} or not v.get("candidate_order") or not v.get("effect_receipts") or not all(str(v.get(k,"")).strip() for k in ("request_id","consumer","purpose","target_scope","semantic_profile","required_estimand")):raise ResearchContractError("analysis identity, locality, candidates, or effects are incomplete")
        for k in ("candidate_order","qualified_order","unresolved_order","blocked_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("analysis ordering is not canonical")
        ids=set(v["candidate_order"]);parts=[*v["qualified_order"],*v["unresolved_order"],*v["blocked_order"]]
        if len(ids)!=len(v["candidate_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("analysis candidates do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","analysis_digest",a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("analysis_digest") or not all(_digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("analysis digest is invalid")
        if v["disposition"]=="qualified" and v["effect_receipts"]!=[f"observe:analysis:{v['request_id']}"]:raise ResearchContractError("qualified analysis effect is invalid")
        if v["disposition"]!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified analysis must block")
def statistical_analysis_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"bioethics","consumers":["institutional safety reviewer","analysis portfolio steward","prospective workflow operator"],"behavior":"verify prospective high-throughput statistical, causal, and ML analysis declarations with ethical, evidence, provenance, replay, quality, and policy gates","value":"prevents unsupported or ethically unreviewed analytical results from being mistaken for qualified research findings","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute:local-computation","write:local-artifact"],"permissions":["evaluate:capability-runs"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def assure_statistical_analysis(request:Mapping[str,Any])->QualifiedAnalysisResult7:
    if request.get("schema_version")!=INPUT_SCHEMA or not all(str(request.get(k,"")).strip() for k in ("request_id","consumer","purpose","target_scope","semantic_profile","required_estimand")) or int(request.get("minimum_quality_milli",0))<=0 or not request.get("candidates") or not _digest(request.get("replay_identity")) or request.get("aggregate_only") is not True or request.get("raw_data_local") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("analysis query identity, bounds, replay, locality, or boundary is invalid")
    candidates=sorted({str(x.get("model_id","")) for x in request["candidates"]});
    if len(candidates)!=len(request["candidates"]) or any(not x.strip() for x in candidates):raise ResearchContractError("model ids must be unique and non-empty")
    q=set();u=set();b=set();om=set();unc=set();neg=set()
    for x in request["candidates"]:
        mid=str(x["model_id"]);neg.add(mid) if x.get("negative_result") else None;om.update(f"{mid}:{o}" for o in x.get("omission_order",[]));hard=x.get("permitted") is not True or x.get("local_only") is not True or x.get("privacy_reviewed") is not True or x.get("dual_use_reviewed") is not True or x.get("scope")!=request["target_scope"] or x.get("semantic_profile")!=request["semantic_profile"] or x.get("estimand")!=request["required_estimand"] or int(x.get("quality_milli",0))<int(request["minimum_quality_milli"]) or not _digest(x.get("artifact_digest")) or not _digest(x.get("provenance_digest")) or x.get("replay_identity")!=request["replay_identity"]
        if hard:b.add(mid);om.add(f"{mid}:analysis-integrity-or-ethics")
        elif str(x.get("evidence_state")) in {"contradicted","unknown"}:u.add(mid);unc.add(f"{mid}:evidence-state")
        else:q.add(mid)
    for k,label in (("policy_allow","workflow:policy-denied"),("protected_closure","workflow:protected-closure-incomplete"),("institutional_authorized","workflow:institutional-authorization-missing")):
        if request.get(k) is not True:om.add(label)
    global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","institutional_authorized"));disp="blocked" if global_block or b else "partial" if u else "qualified";
    if global_block:b.update(candidates);q.clear();u.clear()
    checkpoint=_hash({"request_id":request["request_id"],"target_scope":request["target_scope"],"semantic_profile":request["semantic_profile"],"required_estimand":request["required_estimand"],"replay_identity":request["replay_identity"]});payload={"candidate_order":candidates,"qualified_order":sorted(q),"unresolved_order":sorted(u),"blocked_order":sorted(b),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"checkpoint":checkpoint,"replay_identity":request["replay_identity"]};digest=_hash(payload);payload.update({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"consumer":request["consumer"],"purpose":request["purpose"],"target_scope":request["target_scope"],"semantic_profile":request["semantic_profile"],"required_estimand":request["required_estimand"],"disposition":disp,"analysis_digest":digest,"artifact":{"artifact_id":f"bioethics-analysis:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[] if disp=="qualified" else ["analysis-not-executed"],"provenance_digests":sorted({str(x.get("provenance_digest")) for x in request["candidates"]}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"observe:analysis:{request['request_id']}"] if disp=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY});r=QualifiedAnalysisResult7(payload);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","QualifiedAnalysisResult7","statistical_analysis_assurance_manifest","assure_statistical_analysis"]
