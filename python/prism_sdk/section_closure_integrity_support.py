"""Deterministic Python parity for Section P32 closure-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE="application/vnd.aurora.section.closure-integrity-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"section","consumers":["decision-section compiler","context certificate verifier","research workbench","release auditor"],"behavior":f"compile omission-aware decision-section closure at {scale} ({mode})","value":"makes every accepted, rejected, unknown, and omitted claim auditable without silently presenting incomplete evidence","input_schema":"ClosureIntegrityRequest4@1","output_schema":"ClosureIntegrityCard7@1","effects":["emit:closure-card","retain:omission-certificate","block:unsafe-release"],"permissions":["read:local-section-claims"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("claim_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("section identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("claim_order","accepted_order","rejected_order","unknown_order","omitted_order","section_order","confidence_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("closure vectors are not canonical")
 states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["claim_order"])!=len(set(o["claim_order"])) or states!=set(o["claim_order"]):raise ResearchContractError("claim states do not partition")
def compile_closure(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("claims") or not request.get("required_claim_order") or not request.get("required_section_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_claim_order"]) or not _ordered(request["required_section_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("closure identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["claims"],key=lambda c:c.get("claim_id",""));order=[];accepted=set();rejected=set();unknown=set();omitted=set();sections=set();confidence=set();negative=set();digests=set()
 for c in rows:
  cid=c.get("claim_id","")
  if cid in order or not isinstance(cid,str) or not cid.strip() or not isinstance(c.get("section_id"),str) or not c["section_id"].strip() or not isinstance(c.get("statement"),str) or not c["statement"].strip() or not _digest(c.get("evidence_digest")) or not isinstance(c.get("confidence_basis"),str) or not c["confidence_basis"].strip() or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError("claim identity, evidence, confidence, or locality is invalid")
  order.append(cid);sections.add(c["section_id"]);confidence.add(c["confidence_basis"]);digests.add(c["evidence_digest"])
  if c.get("negative_result") is True:negative.add(f"{cid}:negative-result")
  if c.get("unresolved") is True:unknown.add(cid)
  elif cid not in request["required_claim_order"] or c["section_id"] not in request["required_section_order"]:rejected.add(cid)
  elif c["evidence_digest"]==request["replay_identity"]:omitted.add(cid)
  else:accepted.add(cid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);accepted.clear();rejected.clear();unknown.clear()
 missing=not set(request["required_claim_order"])<=set(order) or not set(request["required_section_order"])<=sections;disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"claim_order":order,"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"section_order":sorted(sections),"confidence_order":sorted(confidence),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["artifact"]={"artifact_id":f"section-closure:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"claim_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:closure-card:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
SectionClaim4=dict[str,Any];ClosureIntegrityRequest4=dict[str,Any];ClosureIntegrityCard7=dict[str,Any];ClosureIntegrityArtifact4=dict[str,Any];ClosureIntegrityError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","SectionClaim4","ClosureIntegrityRequest4","ClosureIntegrityCard7","ClosureIntegrityArtifact4","ClosureIntegrityError","manifest","compile_closure","validate"]
