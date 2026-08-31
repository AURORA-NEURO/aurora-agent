"""Deterministic Python parity for IDs P32 identity continuity."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.ids.identity-continuity-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"ids","consumers":["provenance ledger","identity steward","federation gateway","release auditor"],"behavior":f"qualify namespace-safe content identity continuity at {scale} ({mode})","value":"prevents lineage collisions and silent semantic drift while exposing deterministic identity receipts","input_schema":"IdentityContinuityRequest4@1","output_schema":"IdentityContinuityCard7@1","effects":["emit:identity-card","retain:semantic-loss","block:unsafe-release"],"permissions":["read:local-identity-assertions"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("assertion_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("continuity_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("continuity_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("assertion_order","accepted_order","rejected_order","unknown_order","omitted_order","namespace_order","issuer_order","epoch_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("identity vectors are not canonical")
 states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["assertion_order"])!=len(set(o["assertion_order"])) or states!=set(o["assertion_order"]):raise ResearchContractError("assertion states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("assertions") or not request.get("required_subject_order") or not request.get("namespace_allow_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_subject_order"]) or not _ordered(request["namespace_allow_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["assertions"],key=lambda a:a.get("assertion_id",""));order=[];accepted=set();rejected=set();unknown=set();omitted=set();namespaces=set();issuers=set();epochs=set();negative=set();digests=set()
 for a in rows:
  aid=a.get("assertion_id","")
  if aid in order or not isinstance(aid,str) or not aid.strip() or not isinstance(a.get("subject_id"),str) or not a["subject_id"].strip() or not isinstance(a.get("namespace"),str) or not a["namespace"].strip() or not _digest(a.get("content_digest")) or a.get("parent_digest") is not None and not _digest(a.get("parent_digest")) or not isinstance(a.get("issuer_id"),str) or not a["issuer_id"].strip() or a.get("local") is not True or a.get("aggregate_only") is not True:raise ResearchContractError("assertion identity, digest, issuer, or locality is invalid")
  order.append(aid);namespaces.add(a["namespace"]);issuers.add(a["issuer_id"]);epochs.add(f"{a['issuer_id']}:{a.get('epoch',0)}");digests.add(a["content_digest"])
  if a.get("negative_result") is True:negative.add(f"{aid}:negative-result")
  if a.get("parent_digest")==request["replay_identity"]:omitted.add(aid)
  elif a["namespace"] not in request["namespace_allow_order"]:rejected.add(aid)
  elif a.get("epoch",0)==0:unknown.add(aid)
  else:accepted.add(aid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);accepted.clear();rejected.clear();unknown.clear()
 missing=not all(any(a.get("subject_id")==s for a in rows) for s in request["required_subject_order"]);disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"disposition":disposition,"assertion_order":order,"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"namespace_order":sorted(namespaces),"issuer_order":sorted(issuers),"epoch_order":sorted(epochs),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["continuity_digest"]=d;payload["artifact"]={"artifact_id":f"ids-identity-continuity:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"assertion_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:identity-card:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
IdentityAssertion4=dict[str,Any];IdentityContinuityRequest4=dict[str,Any];IdentityContinuityCard7=dict[str,Any];IdentityContinuityArtifact4=dict[str,Any];IdentityContinuityError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","IdentityAssertion4","IdentityContinuityRequest4","IdentityContinuityCard7","IdentityContinuityArtifact4","IdentityContinuityError","manifest","qualify","validate"]
