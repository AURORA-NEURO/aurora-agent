"""Python parity for Ops P32 checkpointed execution-run integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"; CONTENT_TYPE="application/vnd.aurora.ops.run-integrity-card-1+json"
ExecutionEvent4=dict[str,Any];RunIntegrityRequest4=dict[str,Any];RunIntegrityCard7=dict[str,Any];RunIntegrityArtifact4=dict[str,Any];RunIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"ops","consumers":["execution controller","workflow fabric","provenance ledger","operator workbench"],"behavior":f"qualify checkpointed execution runs at {scale} ({mode})","value":"prevents duplicate, over-budget, unreplayable, or unauthorized effects from being represented as complete","input_schema":"RunIntegrityRequest4@1","output_schema":"RunIntegrityCard7@1","effects":["emit:run-card","retain:retry-compensation","block:unsafe-run"],"permissions":["read:local-run-log"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or(feature_id is not None and o.get("feature_id")!=feature_id)or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad:raise ResearchContractError("run identity, locality, artifact, digest, or boundary is incomplete")
 for k in("event_order","committed_order","retry_order","compensation_order","duplicate_order","unresolved_order","omitted_order","effect_order","checkpoint_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("run vectors are not canonical")
 ids=set(o["event_order"]);states=set(o["committed_order"])|set(o["unresolved_order"])|set(o["omitted_order"])|set(o["duplicate_order"])
 if len(o["event_order"])!=len(ids)or states!=ids:raise ResearchContractError("event states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("events") or not request.get("required_event_order") or not _ordered(request["required_event_order"]) or not _ordered(request.get("required_effect_order",[])) or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("action_budget",0)<=0:raise ResearchContractError("run identity, ordering, digest, locality, boundary, or budget is invalid")
 rows=sorted(request["events"],key=lambda e:e.get("event_id",""));seen=set();committed=set();retry=set();compensation=set();duplicate=set();unresolved=set();omitted=set();effects=set();checkpoints=set();evidence=set()
 for e in rows:
  eid=e.get("event_id","")
  if not eid.strip() or eid in seen or not e.get("kind","").strip() or not e.get("status","").strip() or not e.get("effect","").strip() or not _digest(e.get("artifact_digest")) or not _digest(e.get("replay_identity")) or e.get("local") is not True or e.get("aggregate_only") is not True:raise ResearchContractError("event identity, digest, effect, or locality is invalid")
  seen.add(eid);effects.add(f"{eid}:{e['effect']}");checkpoints.add(f"{eid}:{e.get('checkpoint',0)}");retry.add(f"{eid}:retry-of:{e['retry_of']}" ) if e.get("retry_of") else None;compensation.add(eid) if e.get("kind")=="compensation" else None;evidence.add(e["artifact_digest"])
  if e["replay_identity"]!=request["replay_identity"] or e["status"]=="unknown":unresolved.add(eid)
  elif e.get("sequence",0)>request.get("max_sequence",0) or e.get("checkpoint",0)>request.get("checkpoint",0) or e["status"]!="committed":duplicate.add(eid)
  elif eid not in request["required_event_order"]:omitted.add(eid)
  else:committed.add(eid)
 missing=[x for x in request["required_event_order"] if x not in seen];global_block=not all(request.get(k) is True for k in("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only"))or bool(request.get("adversarial_events"))or request.get("action_count",0)>request["action_budget"]or len(rows)>request["action_budget"]
 if global_block:omitted.update(seen);committed.clear();duplicate.clear();unresolved.clear()
 disposition="blocked" if global_block else "unknown" if missing or unresolved else "partial" if duplicate or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"event_order":sorted(seen),"committed_order":sorted(committed),"retry_order":sorted(retry),"compensation_order":sorted(compensation),"duplicate_order":sorted(duplicate),"unresolved_order":sorted(unresolved),"omitted_order":sorted(omitted),"effect_order":sorted(effects),"checkpoint_order":sorted(checkpoints),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["effect_receipts"]=[f"approve:run:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-run"];payload["artifact"]={"artifact_id":f"ops-run:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY};validate(payload,feature_id=feature_id);return payload
__all__=["BOUNDARY","CONTENT_TYPE","ExecutionEvent4","RunIntegrityRequest4","RunIntegrityCard7","RunIntegrityArtifact4","RunIntegrityError","manifest","qualify","validate"]
