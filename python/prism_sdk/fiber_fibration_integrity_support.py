"""Deterministic Python parity for Fiber P32 fibration-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.fiber.fibration-integrity-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"fiber","consumers":["query compiler","protected-closure verifier","decision-section compiler","release auditor"],"behavior":f"certify evidence fibration and protected closure at {scale} ({mode})","value":"prevents scope leakage and unsupported factor joins while making deferred evidence explicit","input_schema":"FibrationIntegrityRequest4@1","output_schema":"FibrationIntegrityCard7@1","effects":["emit:fibration-card","retain:semantic-loss","block:unsafe-compilation"],"permissions":["read:local-fiber-regions"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("region_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("fibration identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("region_order","accepted_order","rejected_order","unknown_order","omitted_order","factor_order","scope_order","epoch_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("fibration vectors are not canonical")
 states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["region_order"])!=len(set(o["region_order"])) or states!=set(o["region_order"]):raise ResearchContractError("region states do not partition")
def certify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("regions") or not request.get("required_region_order") or not request.get("required_factor_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_region_order"]) or not _ordered(request["required_factor_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("fibration identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["regions"],key=lambda r:r.get("region_id",""));order=[];accepted=set();rejected=set();unknown=set();omitted=set();factors=set();scopes=set();epochs=set();negative=set();digests=set()
 for r in rows:
  rid=r.get("region_id","")
  if rid in order or not isinstance(rid,str) or not rid.strip() or not isinstance(r.get("section_id"),str) or not r["section_id"].strip() or not r.get("factor_order") or not _ordered(r["factor_order"]) or not _digest(r.get("evidence_digest")) or not isinstance(r.get("scope_id"),str) or not r["scope_id"].strip() or r.get("local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("region identity, factor ordering, evidence, or locality is invalid")
  order.append(rid);factors.update(r["factor_order"]);scopes.add(r["scope_id"]);epochs.add(f"{r['scope_id']}:{r.get('policy_epoch',0)}");digests.add(r["evidence_digest"])
  if r.get("negative_result") is True:negative.add(f"{rid}:negative-result")
  if r.get("policy_epoch",0)==0:unknown.add(rid)
  elif rid not in request["required_region_order"] or not set(request["required_factor_order"])<=set(r["factor_order"]):rejected.add(rid)
  elif r["evidence_digest"]==request["replay_identity"]:omitted.add(rid)
  else:accepted.add(rid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);accepted.clear();rejected.clear();unknown.clear()
 missing=not set(request["required_region_order"])<=set(order) or not set(request["required_factor_order"])<=factors;disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"region_order":order,"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"factor_order":sorted(factors),"scope_order":sorted(scopes),"epoch_order":sorted(epochs),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["artifact"]={"artifact_id":f"fiber-fibration:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"region_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:fibration-card:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-compilation"];validate(payload,feature_id=feature_id);return payload
FiberRegion4=dict[str,Any];FibrationIntegrityRequest4=dict[str,Any];FibrationIntegrityCard7=dict[str,Any];FibrationIntegrityArtifact4=dict[str,Any];FibrationIntegrityError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","FiberRegion4","FibrationIntegrityRequest4","FibrationIntegrityCard7","FibrationIntegrityArtifact4","FibrationIntegrityError","manifest","certify","validate"]
