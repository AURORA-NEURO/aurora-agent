"""Deterministic Python parity for Epistemic P32 evidence-closure cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.epistemic.evidence-closure-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"epistemic","consumers":["evidence compiler","retrieval synthesizer","decision-section compiler","release auditor"],"behavior":f"qualify evidence-backed assertion closure at {scale} ({mode})","value":"keeps uncertainty, contradictions, competing explanations, and negative evidence visible instead of manufacturing confidence","input_schema":"EvidenceClosureRequest4@1","output_schema":"EvidenceClosureCard7@1","effects":["emit:evidence-closure-card","retain:uncertainty","block:unsupported-claim"],"permissions":["read:local-assertions"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("assertion_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("evidence identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("assertion_order","supported_order","contradicted_order","unknown_order","omitted_order","source_order","uncertainty_order","competing_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("evidence vectors are not canonical")
 states=set(o["supported_order"])|set(o["contradicted_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["assertion_order"])!=len(set(o["assertion_order"])) or states!=set(o["assertion_order"]):raise ResearchContractError("assertion states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("assertions") or not request.get("required_assertion_order") or not request.get("required_source_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_assertion_order"]) or not _ordered(request["required_source_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("evidence identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["assertions"],key=lambda a:a.get("assertion_id",""));order=[];supported=set();contradicted=set();unknown=set();omitted=set();sources=set();uncertainty=set();competing=set();negative=set();digests=set()
 for a in rows:
  aid=a.get("assertion_id","")
  if aid in order or not isinstance(aid,str) or not aid.strip() or not isinstance(a.get("source_id"),str) or not a["source_id"].strip() or not isinstance(a.get("statement"),str) or not a["statement"].strip() or not _digest(a.get("evidence_digest")) or a.get("local") is not True or a.get("aggregate_only") is not True:raise ResearchContractError("assertion identity, source, evidence, or locality is invalid")
  order.append(aid);sources.add(a["source_id"]);uncertainty.add(f"{aid}:{a.get('uncertainty_milli',0)}");digests.add(a["evidence_digest"])
  if a.get("competing_explanation") is True:competing.add(aid)
  if a.get("negative_result") is True:negative.add(f"{aid}:negative-result")
  if a.get("policy_epoch",0)==0:unknown.add(aid)
  elif a.get("contradicted") is True or aid not in request["required_assertion_order"] or a["source_id"] not in request["required_source_order"]:contradicted.add(aid)
  elif a["evidence_digest"]==request["replay_identity"]:omitted.add(aid)
  else:supported.add(aid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);supported.clear();contradicted.clear();unknown.clear()
 missing=not set(request["required_assertion_order"])<=set(order) or not set(request["required_source_order"])<=sources;disposition="blocked" if global_block else "unknown" if missing else "partial" if contradicted or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"assertion_order":order,"supported_order":sorted(supported),"contradicted_order":sorted(contradicted),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"source_order":sorted(sources),"uncertainty_order":sorted(uncertainty),"competing_order":sorted(competing),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["artifact"]={"artifact_id":f"epistemic-evidence:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"assertion_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:evidence-closure:{request['request_id']}"] if disposition=="qualified" else ["block:unsupported-claim"];validate(payload,feature_id=feature_id);return payload
EpistemicAssertion4=dict[str,Any];EvidenceClosureRequest4=dict[str,Any];EvidenceClosureCard7=dict[str,Any];EvidenceClosureArtifact4=dict[str,Any];EvidenceClosureError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","EpistemicAssertion4","EvidenceClosureRequest4","EvidenceClosureCard7","EvidenceClosureArtifact4","EvidenceClosureError","manifest","qualify","validate"]
