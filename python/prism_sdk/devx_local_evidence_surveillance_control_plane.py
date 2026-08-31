"""Python parity for ``AFA-devx-P01-F29`` local evidence control."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-devx-P01-F29";CONTRACT_VERSION="devx-local-single-study-evidence-surveillance-federated-control-plane/1.0";INPUT_SCHEMA="DevxEvidenceFeed5@1";OUTPUT_SCHEMA="DevxEvidenceControlReceipt8@1";CONTENT_TYPE="application/vnd.aurora.devx-evidence-control-receipt-8+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class DevxEvidenceControlReceipt8:
 value:Mapping[str,Any]
 def to_dict(self)->dict[str,Any]:return dict(self.value)
 def validate(self)->None:
  v=self.value
  if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("artifact",{}).get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or any(not str(v.get(k,"")).strip() for k in ("request_id","study_id","scope","requester","purpose","semantic_profile")) or not v.get("candidate_order") or not v.get("effect_receipts"):raise ResearchContractError("DevX evidence identity, locality, candidates, or effects are incomplete")
  keys=("candidate_order","qualified_order","unresolved_order","blocked_order","missing_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
  if any(not _ordered(v.get(k,[])) for k in keys):raise ResearchContractError("DevX evidence ordering is not canonical")
  ids=set(v["candidate_order"]);parts=v.get("qualified_order",[])+v.get("unresolved_order",[])+v.get("blocked_order",[])+v.get("missing_order",[])
  if len(v["candidate_order"])!=len(ids) or set(parts)!=ids or len(parts)!=len(set(parts)):raise ResearchContractError("DevX evidence states do not partition")
  a=v.get("artifact",{})
  if not all(_digest(x) for x in (v.get("replay_identity"),v.get("control_digest"),a.get("content_hash"))) or v.get("control_digest")!=a.get("content_hash") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("DevX evidence digest is invalid")
  if any(e!="block:unsafe-release" and not e.startswith("read:local-research-artifacts:") for e in v["effect_receipts"]):raise ResearchContractError("DevX effect is outside local read gate")
  if v.get("disposition")=="qualified" and v["effect_receipts"]!=[f"read:local-research-artifacts:{v['study_id']}"]:raise ResearchContractError("qualified DevX read effect is invalid")
  if v.get("disposition")!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified DevX evidence must block")
 def digest(self)->str:self.validate();return _hash(self.value)
def control_devx_evidence_surveillance(*,feed:Mapping[str,Any])->DevxEvidenceControlReceipt8:
 if feed.get("schema_version")!=INPUT_SCHEMA or any(not str(feed.get(k,"")).strip() for k in ("request_id","study_id","scope","requester","purpose","semantic_profile")) or not feed.get("required_evidence_order") or not feed.get("observations") or not _digest(feed.get("replay_identity")) or feed.get("raw_data_local") is not True or feed.get("aggregate_only") is not True or feed.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("DevX evidence request identity, closure, replay, locality, or boundary is invalid")
 rows=sorted(feed["observations"],key=lambda x:(-int(x.get("relevance_milli",0)),str(x.get("evidence_id",""))));ids=sorted(set(feed["required_evidence_order"])|{str(x.get("evidence_id")) for x in rows});q,u,b,m,o,unc,n=set(),set(),set(),set(),set(),set(),set()
 for x in rows:
  eid=str(x.get("evidence_id"));o.update(f"{eid}:{z}" for z in x.get("omission_order",[]));n.update({f"{eid}:negative-result"} if x.get("negative_result") else set());hard=any(x.get(k) is not True for k in ("permitted","local_only","aggregate_only","signed")) or x.get("semantic_profile")!=feed["semantic_profile"] or feed.get("policy_allow") is not True or feed.get("protected_closure") is not True;soft=str(x.get("replay_identity"))!=str(feed["replay_identity"]) or int(x.get("relevance_milli",0))<int(feed.get("minimum_relevance_milli",1)) or int(x.get("freshness_milli",0))<int(feed.get("minimum_freshness_milli",1)) or str(x.get("evidence_state","")).lower() not in {"proven","supported"};(b if hard else u if soft else q).add(eid)
 for eid in feed["required_evidence_order"]:
  if eid not in {str(x.get("evidence_id")) for x in rows}:m.add(eid);o.add(f"evidence:{eid}:missing")
 unc.update(f"adversarial:{z}" for z in feed.get("adversarial_events",[]));glob=any(feed.get(k) is not True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(feed.get("adversarial_events"));
 if glob:b.update(str(x.get("evidence_id")) for x in rows);q.clear();u.clear();o.add("request:surveillance-gate-blocked")
 disposition="blocked" if glob else ("partial" if u or b or m else "qualified");o.add("request:surveillance-not-release-ready") if disposition!="qualified" else None
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":str(feed["request_id"]),"study_id":str(feed["study_id"]),"scope":str(feed["scope"]),"requester":str(feed["requester"]),"purpose":str(feed["purpose"]),"semantic_profile":str(feed["semantic_profile"]),"disposition":disposition,"candidate_order":ids,"qualified_order":sorted(q),"unresolved_order":sorted(u),"blocked_order":sorted(b),"missing_order":sorted(m),"omission_order":sorted(o),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(n),"effect_receipts":[f"read:local-research-artifacts:{feed['study_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["replay_identity"]=str(feed["replay_identity"]);payload["control_digest"]=d;payload["artifact"]={"artifact_id":f"devx-evidence-control:{feed['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(o),"provenance_digests":sorted(str(x.get("provenance_digest")) for x in rows),"boundary":PRECLINICAL_BOUNDARY};r=DevxEvidenceControlReceipt8(payload);r.validate();return r
def devx_evidence_surveillance_control_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"devx","consumers":["research developer","single-study operator","evidence pipeline steward"],"input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"autonomy_tier":"A1","effects":["read:local-research-artifacts","block:unsafe-release"],"boundary":PRECLINICAL_BOUNDARY}
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","DevxEvidenceControlReceipt8","control_devx_evidence_surveillance","devx_evidence_surveillance_control_manifest"]
