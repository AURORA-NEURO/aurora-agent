"""Python parity for the backends federated retrieval/synthesis workflow fabric."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-backends-P02-F16";CONTRACT_VERSION="backends-federated-continual-retrieval-synthesis-workflow-fabric/1.0";INPUT_SCHEMA="FederatedRetrievalSynthesisRequest6@1";OUTPUT_SCHEMA="FederatedRetrievalSynthesisRun8@1";CONTENT_TYPE="application/vnd.aurora.backends-federated-retrieval-synthesis-run-8+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class FederatedRetrievalSynthesisRun8:
 value:Mapping[str,Any]
 def to_dict(self)->dict[str,Any]:return dict(self.value)
 def validate(self)->None:
  v=self.value
  if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("artifact",{}).get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or any(not str(v.get(k,"")).strip() for k in ("request_id","workflow_id","federation_id","requester","purpose","semantic_profile")) or not v.get("candidate_order") or not v.get("peer_order") or not v.get("stage_order") or not v.get("effect_receipts"):raise ResearchContractError("federated retrieval identity, axes, locality, stages, or effects are incomplete")
  keys=("stage_order","candidate_order","selected_candidate_order","unresolved_candidate_order","blocked_candidate_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
  if any(not _ordered(v.get(k,[])) for k in keys):raise ResearchContractError("federated retrieval ordering is not canonical")
  ids=set(v["candidate_order"]);parts=v.get("selected_candidate_order",[])+v.get("unresolved_candidate_order",[])+v.get("blocked_candidate_order",[])
  if len(v["candidate_order"])!=len(ids) or set(parts)!=ids or len(parts)!=len(set(parts)):raise ResearchContractError("candidate states do not partition")
  peers=set(v["peer_order"]);pp=v.get("qualified_peer_order",[])+v.get("missing_peer_order",[])
  if len(v["peer_order"])!=len(peers) or set(pp)!=peers or len(pp)!=len(set(pp)):raise ResearchContractError("peer states do not partition")
  a=v.get("artifact",{})
  if not all(_digest(x) for x in (v.get("replay_identity"),v.get("workflow_digest"),a.get("content_hash"))) or v.get("workflow_digest")!=a.get("content_hash") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("federated retrieval digest is invalid")
  if any(e!="block:unsafe-release" and not e.startswith("coordinate:federated-retrieval:") for e in v["effect_receipts"]):raise ResearchContractError("federated retrieval effect is outside governed gate")
  if v.get("disposition")=="qualified" and v["effect_receipts"]!=[f"coordinate:federated-retrieval:{v['request_id']}"]:raise ResearchContractError("qualified effect is invalid")
  if v.get("disposition")!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified run must block")
 def digest(self)->str:self.validate();return _hash(self.value)
def run_federated_retrieval_synthesis(*,request:Mapping[str,Any])->FederatedRetrievalSynthesisRun8:
 if request.get("schema_version")!=INPUT_SCHEMA or any(not str(request.get(k,"")).strip() for k in ("request_id","workflow_id","federation_id","requester","purpose","semantic_profile")) or not request.get("required_candidate_order") or not request.get("required_peer_order") or not request.get("peers") or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("federated retrieval request identity, closure, replay, locality, or boundary is invalid")
 rows=sorted(request["peers"],key=lambda p:str(p.get("peer_id","")));ids=sorted(set(request["required_candidate_order"])|{str(c) for p in rows for c in p.get("candidate_order",[])});peers=sorted(set(request["required_peer_order"])|{str(p.get("peer_id","")) for p in rows});q,m,o,u,n=set(),set(),set(),set(),set()
 for p in rows:
  pid=str(p.get("peer_id",""));o.update(f"{pid}:{x}" for x in p.get("omission_order",[]));n.update({f"{pid}:negative-result"} if p.get("negative_result") else set());ok=str(p.get("semantic_profile"))==str(request["semantic_profile"]) and str(p.get("replay_identity"))==str(request["replay_identity"]) and all(p.get(k) is True for k in ("signed","permitted","local_only","aggregate_only","policy_allow","protected_closure")) and str(p.get("evidence_state","")).lower() in {"proven","supported"};(q if ok else m).add(pid);u.update({f"{pid}:peer-closure"} if not ok else set())
 for pid in request["required_peer_order"]:
  if pid not in {str(p.get("peer_id")) for p in rows}:m.add(pid);o.add(f"peer:{pid}:missing")
 selected={str(c) for p in rows if str(p.get("peer_id")) in q for c in p.get("candidate_order",[])};unresolved=set(ids)-selected
 for cid in request["required_candidate_order"]:
  if cid not in selected:o.add(f"candidate:{cid}:missing")
 global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","signed_approval","federation_authorized","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events"));
 if global_block:selected.clear();o.add("workflow:federation-gate-blocked")
 disposition="blocked" if global_block else ("partial" if m or unresolved or not selected else "qualified");o.add("workflow:not-release-ready") if disposition!="qualified" else None
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":str(request["request_id"]),"workflow_id":str(request["workflow_id"]),"federation_id":str(request["federation_id"]),"requester":str(request["requester"]),"purpose":str(request["purpose"]),"semantic_profile":str(request["semantic_profile"]),"disposition":disposition,"stage_order":["stage:candidate-merge","stage:checkpoint","stage:peer-qualify","stage:seal-envelope"],"candidate_order":ids,"selected_candidate_order":sorted(selected),"unresolved_candidate_order":sorted(unresolved),"blocked_candidate_order":ids if global_block else [],"peer_order":peers,"qualified_peer_order":sorted(q),"missing_peer_order":sorted(m),"omission_order":sorted(o),"uncertainty_order":sorted(u),"negative_evidence_order":sorted(n),"effect_receipts":[f"coordinate:federated-retrieval:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["replay_identity"]=str(request["replay_identity"]);payload["workflow_digest"]=d;payload["artifact"]={"artifact_id":f"backends-federated-retrieval:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(o),"provenance_digests":sorted(str(p.get("provenance_digest")) for p in rows),"boundary":PRECLINICAL_BOUNDARY};r=FederatedRetrievalSynthesisRun8(payload);r.validate();return r
def federated_retrieval_synthesis_workflow_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"backends","consumers":["federated workflow operator","retrieval synthesis steward","backend planner"],"input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"autonomy_tier":"A1","effects":["coordinate:federated-retrieval","block:unsafe-release"],"boundary":PRECLINICAL_BOUNDARY}
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","FederatedRetrievalSynthesisRun8","run_federated_retrieval_synthesis","federated_retrieval_synthesis_workflow_manifest"]
