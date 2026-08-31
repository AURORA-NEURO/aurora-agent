"""Deterministic Python parity for Worldgen P31 federated commons."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.federated-commons-envelope-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["consortium steward","federation operator","research program owner","release auditor"],"behavior":f"admit aggregate federated research exchange at {scale} ({mode})","value":"shares signed capability and evidence aggregates across institutions while preserving locality, purpose limitation, and revocation state","input_schema":"FederatedCommonsRequest4@1","output_schema":"FederatedCommonsCard7@1","effects":["emit:federation-envelope","deny:raw-data-export","block:unsafe-release"],"permissions":["read:local-peer-manifests"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or o.get("localization")!="origin-local-aggregate-only" or not o.get("peer_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("federation_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("federation_digest") or a.get("localization")!="origin-local-aggregate-only" or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("federation identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("peer_order","admitted_order","denied_order","unknown_order","omitted_order","revoked_order","negative_evidence_order","capability_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("federation vectors are not canonical")
 states=set(o["admitted_order"])|set(o["denied_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["peer_order"])!=len(set(o["peer_order"])) or states!=set(o["peer_order"]):raise ResearchContractError("peer states do not partition")
def admit(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("peers") or not request.get("required_peer_order") or not request.get("required_capability_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_peer_order"]) or not _ordered(request["required_capability_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("federation identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["peers"],key=lambda p:p.get("peer_id",""));order=[];admitted=set();denied=set();unknown=set();omitted=set();revoked=set();negative=set();capabilities=set();peer_digests=set()
 for p in rows:
  pid=p.get("peer_id","")
  if pid in order or not isinstance(pid,str) or not pid.strip() or not p.get("capabilities") or not _ordered(p["capabilities"]) or not _digest(p.get("capability_digest")) or not _digest(p.get("evidence_digest")) or not isinstance(p.get("semantic_profile"),str) or not p["semantic_profile"].strip() or p.get("local") is not True or p.get("aggregate_only") is not True:raise ResearchContractError("peer identity, capability, digest, or locality is invalid")
  order.append(pid);capabilities.update(p["capabilities"]);peer_digests.add(p["evidence_digest"])
  if p.get("negative_result") is True:negative.add(f"{pid}:negative-result")
  if p.get("policy_epoch",0)==0 or p.get("active") is not True:revoked.add(pid)
  elif p.get("authorized") is not True:denied.add(pid)
  elif p.get("semantic_profile")!="preclinical-research-v1":unknown.add(pid)
  elif p.get("capability_digest")!=request["replay_identity"]:omitted.add(pid)
  else:admitted.add(pid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 missing=not set(request["required_capability_order"])<=capabilities
 if global_block:omitted.update(order);admitted.clear();denied.clear();unknown.clear();revoked.clear()
 disposition="blocked" if global_block else "unknown" if missing or not set(request["required_peer_order"])<=set(order) else "partial" if denied or unknown or omitted or revoked else "admitted"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"peer_order":order,"admitted_order":sorted(admitted),"denied_order":sorted(denied),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"revoked_order":sorted(revoked),"negative_evidence_order":sorted(negative),"capability_order":sorted(capabilities),"localization":"origin-local-aggregate-only","replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["federation_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-federated-commons:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"peer_digests":sorted(peer_digests),"purpose":request["purpose"],"localization":"origin-local-aggregate-only","boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:federation-envelope:{request['request_id']}"] if disposition=="admitted" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
FederationPeer4=dict[str,Any];FederatedCommonsRequest4=dict[str,Any];FederatedCommonsCard7=dict[str,Any];FederatedCommonsArtifact4=dict[str,Any];FederatedCommonsError=ResearchContractError
__all__=["CONTENT_TYPE","FederationPeer4","FederatedCommonsRequest4","FederatedCommonsCard7","FederatedCommonsArtifact4","FederatedCommonsError","manifest","admit","validate"]
