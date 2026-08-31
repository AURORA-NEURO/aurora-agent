"""Deterministic Python parity for Worldgen P28 semantic parity."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.semantic-parity-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["parity steward","SDK maintainer","federation operator","release auditor"],"behavior":f"compare semantic artifacts with explicit mismatch and loss evidence at {scale} ({mode} scale)","value":"prevents cross-language or cross-site drift from being mistaken for reproducible research state","input_schema":"SemanticParityRequest4@1","output_schema":"SemanticParityCard7@1","effects":["emit:parity-card","block:unsafe-release"],"permissions":["read:local-artifact-summaries"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("artifact_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("parity_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("parity_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("parity identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("language_order","artifact_order","matched_order","mismatched_order","omitted_order","unknown_order","negative_evidence_order","field_order","semantic_loss_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("parity vectors are not canonical")
 if set(o["artifact_order"])!=set(o.get("matched_order",[]))|set(o.get("mismatched_order",[]))|set(o.get("omitted_order",[]))|set(o.get("unknown_order",[])):raise ResearchContractError("parity states do not partition")
def compare(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("artifacts") or not request.get("required_language_order") or not request.get("required_field_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_language_order"]) or not _ordered(request["required_field_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("parity identity, required languages/fields, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["artifacts"],key=lambda a:a.get("artifact_id",""));order=[];langs=set();fields=set();matched=set();mismatched=set();omitted=set();unknown=set();negative=set();loss=set();provenance=set();baseline=rows[0].get("canonical_digest") if rows else None
 for a in rows:
  aid=a.get("artifact_id","")
  if aid in order or not aid.strip() or not a.get("language","").strip() or not a.get("semantic_profile","").strip() or not _digest(a.get("canonical_digest")) or not _digest(a.get("schema_digest")) or not _digest(a.get("provenance_digest")) or not _digest(a.get("replay_identity")) or not _ordered(a.get("field_order",[])) or a.get("local") is not True or a.get("aggregate_only") is not True:raise ResearchContractError("artifact identity, ordering, digest, or locality is invalid")
  order.append(aid);langs.add(a["language"]);fields.update(a.get("field_order",[]));provenance.add(a["provenance_digest"])
  if a.get("negative_result") is True:negative.add(f"{aid}:negative-result")
  if a["replay_identity"]!=request["replay_identity"] or a["semantic_profile"]!="aurora-canonical":omitted.add(aid);loss.add(f"{aid}:profile-or-replay")
  elif a.get("evidence_state") in {"unknown","unmeasured"}:unknown.add(aid)
  elif a["canonical_digest"]==baseline:matched.add(aid)
  else:mismatched.add(aid);loss.add(f"{aid}:canonical-digest-mismatch")
 blocked=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","network_available","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events"))
 if blocked:omitted.update(order);matched.clear();mismatched.clear();unknown.clear()
 langs_ok=set(request["required_language_order"])<=langs;fields_ok=set(request["required_field_order"])<=fields;disp="blocked" if blocked else "unresolved" if not langs_ok or not fields_ok or mismatched or omitted or unknown else "parity"
 if not langs_ok:loss.add("required-language-missing")
 if not fields_ok:loss.add("required-field-missing")
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"disposition":disp,"language_order":sorted(langs),"artifact_order":order,"matched_order":sorted(matched),"mismatched_order":sorted(mismatched),"omitted_order":sorted(omitted),"unknown_order":sorted(unknown),"negative_evidence_order":sorted(negative),"field_order":sorted(fields),"semantic_loss_order":sorted(loss),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["parity_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-semantic-parity:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(loss),"provenance_digests":sorted(provenance),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:parity-card:{request['request_id']}"] if disp=="parity" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
ParityArtifact4=dict[str,Any];SemanticParityRequest4=dict[str,Any];SemanticParityCard7=dict[str,Any];SemanticParityError=ResearchContractError
__all__=["CONTENT_TYPE","ParityArtifact4","SemanticParityRequest4","SemanticParityCard7","SemanticParityError","manifest","compare","validate"]
