"""Python parity for the IDS local single-study evidence inference engine."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ids-P01-F01"; CONTRACT_VERSION="ids-local-single-study-evidence-surveillance-inference-engine/1.0"; INPUT_SCHEMA="EvidenceFeed1@1"; OUTPUT_SCHEMA="QualifiedEvidenceSet1@1"; CONTENT_TYPE="application/vnd.aurora.ids-qualified-evidence-set-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class QualifiedEvidenceSet1:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value; a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or not all(str(v.get(k,"")).strip() for k in ("request_id","study_id","intent","scope","semantic_profile")) or not v.get("candidate_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unknown","blocked"}:raise ResearchContractError("IDS evidence identity, locality, candidate closure, or effects are incomplete")
        for k in ("candidate_order","qualified_order","blocked_order","unknown_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("IDS evidence ordering is not canonical")
        ids=set(v["candidate_order"]); parts=[*v["qualified_order"],*v["blocked_order"],*v["unknown_order"]]
        if len(ids)!=len(v["candidate_order"]) or len(parts)!=len(ids) or len(set(parts))!=len(parts) or any(x not in ids for x in parts):raise ResearchContractError("IDS evidence states do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","evidence_digest",a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("evidence_digest"):raise ResearchContractError("IDS evidence digest or artifact metadata is invalid")
        if any(e!="block:unsafe-release" and not e.startswith("read:local-research-artifacts:") for e in v["effect_receipts"]):raise ResearchContractError("IDS evidence effect is outside local-read gate")
        if v["disposition"]=="qualified" and v["effect_receipts"]!=[f"read:local-research-artifacts:{v['study_id']}"]:raise ResearchContractError("qualified IDS read effect is invalid")
        if v["disposition"]!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified IDS evidence must block")
def local_evidence_surveillance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumer":"computational biologist","behavior":"compute deterministic evidence alerts for one institution-local preclinical study from typed EvidenceFeed1 observations","value":"preserves qualified, unknown, unmeasured, contradicted, omitted, and negative evidence in a replayable researcher artifact","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["read:local-research-artifacts"],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY}
def infer_local_evidence_surveillance(feed:Mapping[str,Any])->QualifiedEvidenceSet1:
    if not all(str(feed.get(k,"")).strip() for k in ("request_id","study_id","intent","scope","semantic_profile")) or not feed.get("observations") or int(feed.get("max_items",0))<=0 or not _digest(feed.get("replay_identity")) or feed.get("raw_data_local") is not True or feed.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("IDS evidence feed identity, locality, replay, bounds, or boundary are invalid")
    rows=sorted((dict(x) for x in feed["observations"]),key=lambda x:(-int(x.get("relevance_milli",0)),str(x.get("evidence_id","")))); candidates=sorted({str(x.get("evidence_id","")) for x in rows}); q=set();b=set();u=set();om=set();unc=set();neg=set()
    for i,x in enumerate(rows):
        eid=str(x.get("evidence_id","")); neg.add(f"{eid}:negative-result") if x.get("negative_result") else None; hard=not feed.get("policy_allow",False) or not feed.get("protected_closure",False) or x.get("authorized") is not True or x.get("local_only") is not True or x.get("study_id")!=feed["study_id"] or x.get("scope")!=feed["scope"] or not _digest(x.get("content_digest")) or not _digest(x.get("provenance_digest")); soft=x.get("replay_identity")!=feed["replay_identity"] or x.get("fresh") is not True or int(x.get("relevance_milli",0))<int(feed.get("minimum_relevance_milli",0)) or x.get("evidence_state") not in {"proven","supported"}
        if hard or x.get("evidence_state")=="contradicted":b.add(eid)
        elif soft or i>=int(feed["max_items"]):u.add(eid); unc.add(f"{eid}:unresolved"); om.add(f"{eid}:capacity") if i>=int(feed["max_items"]) else None
        else:q.add(eid)
    if not feed.get("policy_allow",False):om.add("workflow:policy-denied")
    if not feed.get("protected_closure",False):om.add("workflow:protected-closure-incomplete")
    disposition="blocked" if b else "unknown" if u or not q or neg else "qualified"; om.add("workflow:closure-incomplete") if disposition!="qualified" else None
    evidence_digest=_hash({"candidate_order":candidates,"qualified_order":sorted(q),"blocked_order":sorted(b),"unknown_order":sorted(u),"replay_identity":feed["replay_identity"]}); payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":feed["request_id"],"study_id":feed["study_id"],"intent":feed["intent"],"scope":feed["scope"],"semantic_profile":feed["semantic_profile"],"disposition":disposition,"candidate_order":candidates,"qualified_order":sorted(q),"blocked_order":sorted(b),"unknown_order":sorted(u),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"replay_identity":feed["replay_identity"],"evidence_digest":evidence_digest,"artifact":{"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"ids-qualified-evidence:{feed['study_id']}","content_type":CONTENT_TYPE,"content_hash":evidence_digest,"semantic_loss":[],"provenance_digests":sorted({str(x.get('provenance_digest')) for x in rows}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"read:local-research-artifacts:{feed['study_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"boundary":PRECLINICAL_BOUNDARY}; result=QualifiedEvidenceSet1(payload); result.validate(); return result
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","QualifiedEvidenceSet1","local_evidence_surveillance_manifest","infer_local_evidence_surveillance"]
