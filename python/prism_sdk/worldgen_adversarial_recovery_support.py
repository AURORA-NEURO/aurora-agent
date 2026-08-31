"""Deterministic Python parity for Worldgen P30 adversarial recovery."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError

CONTENT_TYPE="application/vnd.aurora.worldgen.adversarial-recovery-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["security steward","incident operator","workflow owner","release auditor"],"behavior":f"contain and recover from adversarial research events at {scale} ({mode})","value":"makes prompt injection, poisoned artifacts, compromised connectors, revoked keys, crashes, and duplicate events recoverable without hiding unsafe state","input_schema":"AdversarialRecoveryRequest4@1","output_schema":"AdversarialRecoveryCard7@1","effects":["quarantine:unsafe-events","emit:recovery-card","block:unsafe-release"],"permissions":["read:local-security-events"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("event_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("recovery_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("recovery_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("recovery identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("event_order","recovered_order","quarantined_order","blocked_order","unknown_order","omitted_order","checkpoint_order","compensation_order","security_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("recovery vectors are not canonical")
 states=set(o["recovered_order"])|set(o["quarantined_order"])|set(o["blocked_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["event_order"])!=len(set(o["event_order"])) or states!=set(o["event_order"]):raise ResearchContractError("recovery states do not partition")
def recover(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("events") or not request.get("required_event_kind_order") or not request.get("checkpoint_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_event_kind_order"]) or not _ordered(request.get("checkpoint_order",[])) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("recovery identity, event kinds, checkpoints, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["events"],key=lambda e:e.get("event_id",""));order=[];recovered=set();quarantined=set();blocked=set();unknown=set();omitted=set();negative=set();digests=set();security=set()
 for e in rows:
  eid=e.get("event_id","")
  if eid in order or not isinstance(eid,str) or not eid.strip() or not isinstance(e.get("event_kind"),str) or not e["event_kind"].strip() or not isinstance(e.get("severity"),str) or not e["severity"].strip() or not isinstance(e.get("source_id"),str) or not e["source_id"].strip() or not _digest(e.get("event_digest")) or not _digest(e.get("replay_identity")) or e.get("local") is not True or e.get("aggregate_only") is not True:raise ResearchContractError("event identity, digest, or locality is invalid")
  order.append(eid);digests.add(e["event_digest"]);security.add(f"{e['event_kind']}:{e['severity']}")
  if e.get("negative_result") is True:negative.add(f"{eid}:negative-result")
  if e["replay_identity"]!=request["replay_identity"]:omitted.add(eid)
  elif e["event_kind"] in {"revoked-key","compromised-connector"} or e["severity"]=="critical":quarantined.add(eid)
  elif e["event_kind"] in {"crash","duplicate"}: (recovered if e.get("recoverable") is True else blocked).add(eid)
  elif e.get("recoverable") is True:recovered.add(eid)
  else:unknown.add(eid)
 kinds={e["event_kind"] for e in rows};required=set(request["required_event_kind_order"])
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","network_available","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or (mode=="research copilot" and (request.get("action_budget",0)==0 or request.get("action_count",0)>request.get("action_budget",0)))
 if global_block:omitted.update(order);recovered.clear();quarantined.clear();blocked.clear();unknown.clear()
 disposition="blocked" if global_block else "unknown" if not required<=kinds else "partial" if quarantined or blocked or unknown or omitted else "recovered"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"disposition":disposition,"event_order":order,"recovered_order":sorted(recovered),"quarantined_order":sorted(quarantined),"blocked_order":sorted(blocked),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"checkpoint_order":request["checkpoint_order"],"compensation_order":["honest-partial-replay"],"security_order":sorted(security),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
 d=_hash(payload);payload["recovery_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-adversarial-recovery:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"event_digests":sorted(digests),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:recovery-card:{request['request_id']}"] if disposition=="recovered" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
RecoveryEvent4=dict[str,Any];AdversarialRecoveryRequest4=dict[str,Any];AdversarialRecoveryCard7=dict[str,Any];RecoveryArtifact4=dict[str,Any];AdversarialRecoveryError=ResearchContractError
__all__=["CONTENT_TYPE","RecoveryEvent4","AdversarialRecoveryRequest4","AdversarialRecoveryCard7","RecoveryArtifact4","AdversarialRecoveryError","manifest","recover","validate"]
