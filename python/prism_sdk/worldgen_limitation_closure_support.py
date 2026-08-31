"""Deterministic Python parity for Worldgen P26 limitation closure."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.limitation-closure-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["limitations scientist","federation operator","release auditor","context compiler"],"behavior":f"close typed limitations with explicit unresolved states at {scale} ({mode} scale)","value":"prevents open, contradictory, or peer-incomplete limitations from being represented as closed","input_schema":"LimitationClosureRequest4@1","output_schema":"LimitationClosureCard7@1","effects":["exchange:limitation-digests","block:unsafe-release"],"permissions":["read:local-limitation-attestations"],"determinism":"byte_stable","autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("case_order") or not o.get("peer_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("closure identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("case_order","resolved_order","unresolved_order","blocked_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("closure vectors are not canonical")
 if set(o["case_order"])!=set(o.get("resolved_order",[]))|set(o.get("unresolved_order",[]))|set(o.get("blocked_order",[])):raise ResearchContractError("limitation states do not partition")
 if set(o["peer_order"])!=set(o.get("qualified_peer_order",[]))|set(o.get("missing_peer_order",[])):raise ResearchContractError("peer states do not partition")
def close(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("semantic_profile"),str) or not request["semantic_profile"].strip() or not request.get("required_scope_order") or not request.get("cases") or not request.get("peers") or request.get("minimum_peer_quorum",0)<=0 or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_scope_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("closure identity, scopes, cases, peers, quorum, digest, ordering, locality, or boundary is invalid")
 cases=sorted(request["cases"],key=lambda c:c.get("case_id",""));peers=sorted(request["peers"],key=lambda p:p.get("peer_id",""));case_order=[];resolved=set();unresolved=set();blocked=set();omissions=set();uncertainty=set();negative=set()
 for c in cases:
  cid=c.get("case_id","")
  if cid in case_order or not cid.strip() or not c.get("scope","").strip() or not _digest(c.get("replay_identity")) or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError("case identity or locality is invalid")
  case_order.append(cid)
  if c.get("negative_result") is True:negative.add(f"{cid}:negative-result")
  if c["scope"] not in request["required_scope_order"]:unresolved.add(cid);omissions.add(f"{cid}:scope-not-requested")
  elif c["replay_identity"]!=request["replay_identity"]:unresolved.add(cid);uncertainty.add(f"{cid}:replay-identity")
  elif c.get("status")=="resolved" and c.get("evidence_digests"):resolved.add(cid)
  elif c.get("status") in {"blocked","contradicted"}:blocked.add(cid);negative.add(f"{cid}:contradicted")
  elif c.get("status")=="measured":unresolved.add(cid);uncertainty.add(f"{cid}:measured-not-closed")
  else:unresolved.add(cid);omissions.add(f"{cid}:not-closed")
 peer_order=[];qualified=set();missing=set()
 for p in peers:
  pid=p.get("peer_id","")
  if pid in peer_order or not pid.strip() or not p.get("semantic_profile","").strip() or not _digest(p.get("closure_digest")) or not _digest(p.get("replay_identity")) or p.get("local") is not True or p.get("aggregate_only") is not True:raise ResearchContractError("peer identity, digest, or locality is invalid")
  peer_order.append(pid)
  if p["semantic_profile"]==request["semantic_profile"] and p["replay_identity"]==request["replay_identity"] and p.get("evidence_state")=="qualified":qualified.add(pid)
  else:missing.add(pid);uncertainty.add(f"{pid}:peer-not-qualified")
 if len(qualified)<request["minimum_peer_quorum"]:uncertainty.add(f"peer-quorum:{len(qualified)}/{request['minimum_peer_quorum']}")
 blocked_all=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","federation_approved","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events"))
 if blocked_all:blocked.update(case_order);resolved.clear();unresolved.clear()
 disp="blocked" if blocked_all else "unknown" if not resolved else "partial" if unresolved or blocked or len(qualified)<request["minimum_peer_quorum"] else "closed"
 if disp!="closed":omissions.add("request:closure-incomplete")
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"semantic_profile":request["semantic_profile"],"disposition":disp,"case_order":case_order,"resolved_order":sorted(resolved),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"peer_order":peer_order,"qualified_peer_order":sorted(qualified),"missing_peer_order":sorted(missing),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
 d=_hash(payload);payload["closure_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-limitation-closure:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(omissions),"provenance_digests":[],"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"exchange:limitation-digests:{request['request_id']}"] if disp=="closed" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
LimitationCase4=dict[str,Any];ClosurePeer4=dict[str,Any];LimitationClosureRequest4=dict[str,Any];LimitationClosureCard7=dict[str,Any];LimitationClosureError=ResearchContractError
__all__=["CONTENT_TYPE","LimitationCase4","ClosurePeer4","LimitationClosureRequest4","LimitationClosureCard7","LimitationClosureError","manifest","close","validate"]
