"""Python parity for Sweep P32 audit-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.sweep.audit-integrity-card-1+json"
AuditSubject4=dict[str,Any];AuditRequest4=dict[str,Any];AuditCard7=dict[str,Any];AuditArtifact4=dict[str,Any];AuditIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"sweep","consumers":["portfolio auditor","release steward","dependency planner","research workbench"],"behavior":f"classify release and dependency drift at {scale} ({mode})","value":"prevents unreviewed source drift, omitted checks, or poisoned artifacts from entering reproducible research releases","input_schema":"AuditRequest4@1","output_schema":"AuditCard7@1","effects":["emit:audit-card","retain:drift-evidence","block:unsafe-release"],"permissions":["read:local-audit-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or(feature_id is not None and o.get("feature_id")!=feature_id)or not o.get("request_id")or not o.get("purpose")or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad:raise ResearchContractError("audit identity, locality, artifact, digest, or boundary is incomplete")
 for k in("subject_order","clean_order","drift_order","unknown_order","omitted_order","source_order","status_order","evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("audit vectors are not canonical")
 ids=set(o["subject_order"]);states=set(o["clean_order"])|set(o["drift_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["subject_order"])!=len(ids)or states!=ids:raise ResearchContractError("subject states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("subjects") or not request.get("required_subject_order") or not _ordered(request["required_subject_order"]) or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("subject_budget",0)<=0:raise ResearchContractError("audit identity, ordering, digest, locality, boundary, or budget is invalid")
 rows=sorted(request["subjects"],key=lambda a:a.get("subject_id",""));seen=set();clean=set();drift=set();unknown=set();omitted=set();sources=set();statuses=set();evidence=set()
 for s in rows:
  sid=s.get("subject_id","")
  if not sid.strip()or sid in seen or not s.get("source_commit","").strip()or not _digest(s.get("observed_digest"))or not _digest(s.get("expected_digest"))or not s.get("status","").strip()or not s.get("evidence_state","").strip()or s.get("local") is not True or s.get("aggregate_only") is not True:raise ResearchContractError("subject identity, commits, digests, status, evidence, or locality is invalid")
  seen.add(sid);sources.add(f"{sid}:{s['source_commit']}");statuses.add(f"{sid}:{s['status']}");evidence.add(s["observed_digest"])
  if s["evidence_state"]=="unknown"or s["observed_digest"]==request["replay_identity"]:unknown.add(sid)
  elif s["observed_digest"]!=s["expected_digest"]or s["status"]=="drift":drift.add(sid)
  elif sid not in request["required_subject_order"]:omitted.add(sid)
  else:clean.add(sid)
 missing=[x for x in request["required_subject_order"] if x not in seen];global_block=not all(request.get(k) is True for k in("policy_allowed","protected_closure","signed_manifest","raw_data_local","aggregate_only"))or bool(request.get("adversarial_events"))or len(rows)>request["subject_budget"]
 if global_block:omitted.update(seen);clean.clear();drift.clear();unknown.clear()
 disposition="blocked" if global_block else "unknown" if missing or unknown else "partial" if drift or omitted else "qualified";payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"subject_order":sorted(seen),"clean_order":sorted(clean),"drift_order":sorted(drift),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"source_order":sorted(sources),"status_order":sorted(statuses),"evidence_order":sorted(evidence),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["effect_receipts"]=[f"approve:audit:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"];payload["artifact"]={"artifact_id":f"sweep-audit:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY};validate(payload,feature_id=feature_id);return payload
__all__=["BOUNDARY","CONTENT_TYPE","AuditSubject4","AuditRequest4","AuditCard7","AuditArtifact4","AuditIntegrityError","manifest","qualify","validate"]
