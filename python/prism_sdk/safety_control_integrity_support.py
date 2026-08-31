"""Python parity for Safety P32 risk-tier and interlock integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"; CONTENT_TYPE="application/vnd.aurora.safety.control-integrity-card-1+json"
SafetyControl4=dict[str,Any]; SafetyIntegrityRequest4=dict[str,Any]; SafetyIntegrityCard7=dict[str,Any]; SafetyIntegrityArtifact4=dict[str,Any]; SafetyIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"safety","consumers":["instrument gateway","autonomy broker","red-team auditor","research workbench"],"behavior":f"qualify risk-tier interlock evidence at {scale} ({mode})","value":"prevents unverified physical or autonomous effects from being represented as safe","input_schema":"SafetyIntegrityRequest4@1","output_schema":"SafetyIntegrityCard7@1","effects":["emit:preflight-card","retain:threat-witness","block:unsafe-effect"],"permissions":["read:local-safety-evidence"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{}); bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and o.get("feature_id")!=feature_id) or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad: raise ResearchContractError("safety identity, locality, artifact, digest, or boundary is incomplete")
 for k in ("control_order","accepted_order","rejected_order","unknown_order","omitted_order","threat_order","interlock_order","signer_order","effect_receipts"):
  if not _ordered(o.get(k,[])): raise ResearchContractError("safety vectors are not canonical")
 ids=set(o["control_order"]); states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["control_order"])!=len(ids) or states!=ids: raise ResearchContractError("control states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("controls") or not request.get("required_control_order") or not _ordered(request["required_control_order"]) or not _ordered(request.get("required_interlock_order",[])) or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("control_budget",0)<=0: raise ResearchContractError("safety identity, ordering, digest, locality, boundary, or budget is invalid")
 rows=sorted(request["controls"],key=lambda c:c.get("control_id","")); seen=set(); accepted=set(); rejected=set(); unknown=set(); omitted=set(); threats=set(); interlocks=set(); signers=set(); evidence=set()
 for c in rows:
  cid=c.get("control_id","")
  if not cid.strip() or cid in seen or not c.get("threat_class","").strip() or not c.get("mitigation_status","").strip() or not c.get("signer","").strip() or not _digest(c.get("evidence_digest")) or c.get("local") is not True or c.get("aggregate_only") is not True: raise ResearchContractError("control identity, mitigation, signer, evidence, or locality is invalid")
  seen.add(cid); threats.add(f"{cid}:{c['threat_class']}"); interlocks.update(f"{cid}:{x}" for x in c.get("interlock_order",[])); signers.add(f"{cid}:{c['signer']}"); evidence.add(c["evidence_digest"])
  if c["evidence_digest"]==request["replay_identity"] or c["mitigation_status"]=="unknown": unknown.add(cid)
  elif c["mitigation_status"]!="enforced" or c.get("interlock_order")!=request.get("required_interlock_order") or c["signer"]=="unverified": rejected.add(cid)
  elif cid not in request["required_control_order"]: omitted.add(cid)
  else: accepted.add(cid)
 missing=[i for i in request["required_control_order"] if i not in seen]
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","raw_data_local","aggregate_only")) or not request.get("signed_preflight") or (request.get("physical_effect") and not request.get("human_authorized")) or bool(request.get("adversarial_events")) or len(rows)>request["control_budget"] or (not request.get("dry_run") and request.get("autonomy_tier")=="A0")
 if global_block: omitted.update(seen); accepted.clear(); rejected.clear(); unknown.clear()
 complete=not missing; disposition="blocked" if global_block else "unknown" if not complete or unknown else "partial" if rejected or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"control_order":sorted(seen),"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"threat_order":sorted(threats),"interlock_order":sorted(interlocks),"signer_order":sorted(signers),"autonomy_tier":request["autonomy_tier"],"physical_effect":request["physical_effect"],"dry_run":request["dry_run"],"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY}; d=_hash(payload); payload["closure_digest"]=d; payload["effect_receipts"]=[f"approve:safety-preflight:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-effect"]; payload["artifact"]={"artifact_id":f"safety-control:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY}; validate(payload,feature_id=feature_id); return payload
__all__=["BOUNDARY","CONTENT_TYPE","SafetyControl4","SafetyIntegrityRequest4","SafetyIntegrityCard7","SafetyIntegrityArtifact4","SafetyIntegrityError","manifest","qualify","validate"]
