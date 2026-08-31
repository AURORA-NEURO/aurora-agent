"""Python parity for ``AFA-bioethics-P03-F26`` multimodal context assurance."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-bioethics-P03-F26"; CONTRACT_VERSION="bioethics-multimodal-context-compilation-assurance-harness/1.0"; INPUT_SCHEMA="DecisionQuery2@1"; OUTPUT_SCHEMA="CertifiedDecisionSection7@1"; CONTENT_TYPE="application/vnd.aurora.bioethics-certified-decision-section-7+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class CertifiedDecisionSection7:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{}); required=("request_id","consumer","purpose","target_scope","semantic_profile")
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("disposition") not in {"qualified","partial","blocked"} or not v.get("fact_order") or not v.get("effect_receipts") or not all(str(v.get(k,"")).strip() for k in required):raise ResearchContractError("context identity, locality, facts, or effects are incomplete")
        for k in ("fact_order","selected_order","unresolved_order","blocked_order","missing_study_order","missing_modality_order","omission_order","uncertainty_order","negative_evidence_order","ethical_gate_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("context ordering is not canonical")
        ids=set(v["fact_order"]);parts=[*v["selected_order"],*v["unresolved_order"],*v["blocked_order"]]
        if len(ids)!=len(v["fact_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("context fact states do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","context_digest",a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("context_digest") or not all(_digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("context digest is invalid")
        if v["disposition"]=="qualified" and v["effect_receipts"]!=[f"observe:context:{v['request_id']}"]:raise ResearchContractError("qualified context effect is invalid")
        if v["disposition"]!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified context must block")
def multimodal_context_compilation_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"bioethics","consumers":["consortium operator","context compiler","ethics steward"],"behavior":"assure multimodal preclinical context compilation with ethical, evidence, scope, provenance, replay, omission, and locality gates","value":"prevents incomplete or ethically unreviewed multi-study context from being presented as a certified decision section","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute:local-computation","write:local-artifact"],"permissions":["evaluate:capability-runs"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def assure_multimodal_context_compilation(request:Mapping[str,Any])->CertifiedDecisionSection7:
    if request.get("schema_version")!=INPUT_SCHEMA or not all(str(request.get(k,"")).strip() for k in ("request_id","consumer","purpose","target_scope","semantic_profile")) or not request.get("required_study_order") or not request.get("required_modality_order") or not request.get("facts") or not _digest(request.get("replay_identity")) or request.get("aggregate_only") is not True or request.get("raw_data_local") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("context query identity, requirements, replay, locality, or boundary is invalid")
    facts=[dict(x) for x in request["facts"]];ids=sorted({str(x.get("fact_id","")) for x in facts});
    if len(ids)!=len(facts) or any(not x.strip() for x in ids):raise ResearchContractError("fact ids must be unique and non-empty")
    missing_s=sorted(set(request["required_study_order"])-{str(x.get("study_id")) for x in facts});missing_m=sorted(set(request["required_modality_order"])-{str(x.get("modality")) for x in facts});sel=set();unres=set();blocked=set();om=set();unc=set();ethical=set();neg=set()
    for x in facts:
        fid=str(x["fact_id"]);neg.add(fid) if x.get("negative_result") else None;hard=not x.get("permitted") or not x.get("local_only") or not x.get("privacy_reviewed") or not x.get("dual_use_reviewed") or not x.get("representation_reviewed") or x.get("scope")!=request["target_scope"] or x.get("semantic_profile")!=request["semantic_profile"] or not _digest(x.get("source_digest")) or not _digest(x.get("provenance_digest")) or x.get("replay_identity")!=request["replay_identity"]
        if not x.get("privacy_reviewed"):ethical.add(f"{fid}:privacy-review-missing")
        if not x.get("dual_use_reviewed"):ethical.add(f"{fid}:dual-use-review-missing")
        if not x.get("representation_reviewed"):ethical.add(f"{fid}:representation-review-missing")
        if hard:blocked.add(fid);om.add(f"{fid}:ethical-or-integrity-gate")
        elif str(x.get("evidence_state")) in {"contradicted","unknown"}:unres.add(fid);unc.add(f"{fid}:evidence-state")
        else:sel.add(fid)
        om.update(f"{fid}:{o}" for o in x.get("omission_order",[]))
    om.update(f"study-missing:{s}" for s in missing_s);om.update(f"modality-missing:{m}" for m in missing_m)
    for k,label in (("policy_allow","workflow:policy-denied"),("protected_closure","workflow:protected-closure-incomplete"),("institutional_authorized","workflow:institutional-authorization-missing")):
        if request.get(k) is not True:ethical.add(label)
    global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","institutional_authorized"));disp="blocked" if global_block or blocked else "partial" if missing_s or missing_m or unres else "qualified";
    if global_block:blocked.update(ids);sel.clear();unres.clear()
    checkpoint=_hash({"request_id":request["request_id"],"target_scope":request["target_scope"],"semantic_profile":request["semantic_profile"],"replay_identity":request["replay_identity"]});payload={"fact_order":ids,"selected_order":sorted(sel),"unresolved_order":sorted(unres),"blocked_order":sorted(blocked),"missing_study_order":missing_s,"missing_modality_order":missing_m,"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"ethical_gate_order":sorted(ethical),"replay_identity":request["replay_identity"],"checkpoint":checkpoint};digest=_hash(payload);payload.update({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"consumer":request["consumer"],"purpose":request["purpose"],"target_scope":request["target_scope"],"semantic_profile":request["semantic_profile"],"disposition":disp,"context_digest":digest,"artifact":{"artifact_id":f"bioethics-context:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[] if disp=="qualified" else ["context-closure-incomplete"],"provenance_digests":sorted({str(x.get("provenance_digest")) for x in facts}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"observe:context:{request['request_id']}"] if disp=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY});r=CertifiedDecisionSection7(payload);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","CertifiedDecisionSection7","multimodal_context_compilation_assurance_manifest","assure_multimodal_context_compilation"]
