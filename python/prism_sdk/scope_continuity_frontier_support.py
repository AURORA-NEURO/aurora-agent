"""Deterministic Python parity for Scope P32 continuity frontier."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.scope.continuity-frontier-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"scope","consumers":["context compiler","provenance ledger","scope steward","release auditor"],"behavior":f"qualify scope continuity and closure at {scale} ({mode})","value":"prevents silent scope widening while preserving typed dimensions, evidence lineage, and semantic loss","input_schema":"ScopeContinuityRequest4@1","output_schema":"ScopeContinuityCard7@1","effects":["emit:scope-continuity-card","retain:scope-loss","block:unsafe-release"],"permissions":["read:local-scope-assertions"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("assertion_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("continuity_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("continuity_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("scope identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("assertion_order","accepted_order","rejected_order","unknown_order","omitted_order","scope_order","dimension_order","epoch_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("scope vectors are not canonical")
 states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["assertion_order"])!=len(set(o["assertion_order"])) or states!=set(o["assertion_order"]):raise ResearchContractError("assertion states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("assertions") or not request.get("required_scope_order") or not request.get("required_dimension_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_scope_order"]) or not _ordered(request["required_dimension_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("scope identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["assertions"],key=lambda a:a.get("assertion_id",""));order=[];accepted=set();rejected=set();unknown=set();omitted=set();scopes=set();dimensions=set();epochs=set();negative=set();digests=set()
 for a in rows:
  aid=a.get("assertion_id","")
  if aid in order or not isinstance(aid,str) or not aid.strip() or not isinstance(a.get("scope_id"),str) or not a["scope_id"].strip() or not isinstance(a.get("dimension"),str) or not a["dimension"].strip() or not isinstance(a.get("value"),str) or not a["value"].strip() or not _digest(a.get("evidence_digest")) or a.get("local") is not True or a.get("aggregate_only") is not True:raise ResearchContractError("assertion identity, digest, dimension, or locality is invalid")
  order.append(aid);scopes.add(a["scope_id"]);dimensions.add(a["dimension"]);epochs.add(f"{a['scope_id']}:{a.get('policy_epoch',0)}");digests.add(a["evidence_digest"])
  if a.get("negative_result") is True:negative.add(f"{aid}:negative-result")
  if a.get("policy_epoch",0)==0:unknown.add(aid)
  elif a["scope_id"] not in request["required_scope_order"] or a["dimension"] not in request["required_dimension_order"]:rejected.add(aid)
  elif a["evidence_digest"]==request["replay_identity"]:omitted.add(aid)
  else:accepted.add(aid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);accepted.clear();rejected.clear();unknown.clear()
 missing=not set(request["required_scope_order"])<=scopes or not set(request["required_dimension_order"])<=dimensions;disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"assertion_order":order,"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"scope_order":sorted(scopes),"dimension_order":sorted(dimensions),"epoch_order":sorted(epochs),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["continuity_digest"]=d;payload["artifact"]={"artifact_id":f"scope-continuity:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"assertion_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:scope-continuity-card:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
ScopeAssertion4=dict[str,Any];ScopeContinuityRequest4=dict[str,Any];ScopeContinuityCard7=dict[str,Any];ScopeContinuityArtifact4=dict[str,Any];ScopeContinuityError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","ScopeAssertion4","ScopeContinuityRequest4","ScopeContinuityCard7","ScopeContinuityArtifact4","ScopeContinuityError","manifest","qualify","validate"]
