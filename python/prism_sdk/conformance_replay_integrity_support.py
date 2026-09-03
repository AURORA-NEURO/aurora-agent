"""Python parity for Conformance P32 canonical replay-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"; CONTENT_TYPE="application/vnd.aurora.conformance.replay-integrity-card-1+json"
ReplayCase4=dict[str,Any]; ReplayIntegrityRequest4=dict[str,Any]; ReplayIntegrityCard7=dict[str,Any]; ReplayIntegrityArtifact4=dict[str,Any]; ReplayIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"conformance","consumers":["release gate","SDK parity harness","workflow replay auditor","standards registry"],"behavior":f"qualify byte-identical cross-language replay at {scale} ({mode})","value":"prevents semantic drift, stale standards, and mismatched replay artifacts from releasing","input_schema":"ReplayIntegrityRequest4@1","output_schema":"ReplayIntegrityCard7@1","effects":["emit:replay-card","retain:parity-witness","block:semantic-drift"],"permissions":["read:local-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and o.get("feature_id")!=feature_id) or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad:raise ResearchContractError("replay identity, locality, artifact, digest, or boundary is incomplete")
 for k in ("case_order","parity_order","mismatch_order","unknown_order","omitted_order","language_order","standards_order","migration_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("replay vectors are not canonical")
 ids=set(o["case_order"]);states=set(o["parity_order"])|set(o["mismatch_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["case_order"])!=len(ids) or states!=ids:raise ResearchContractError("case states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("cases") or not request.get("required_case_order") or not _ordered(request["required_case_order"]) or not _ordered(request.get("required_language_order",[])) or not request.get("standards_epoch","").strip() or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("case_budget",0)<=0:raise ResearchContractError("replay identity, ordering, standards, locality, boundary, or budget is invalid")
 rows=sorted(request["cases"],key=lambda c:c.get("case_id",""));seen=set();parity=set();mismatch=set();unknown=set();omitted=set();languages=set();standards=set();migrations=set();evidence=set()
 for c in rows:
  cid=c.get("case_id","")
  if not cid.strip() or cid in seen or not c.get("language","").strip() or not c.get("runtime_version","").strip() or not c.get("schema_version","").strip() or not _digest(c.get("expected_digest")) or not _digest(c.get("observed_digest")) or not c.get("canonical_bytes","").strip() or not c.get("evidence_state","").strip() or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError("case identity, digest, bytes, evidence, or locality is invalid")
  seen.add(cid);languages.add(f"{cid}:{c['language']}");standards.update(f"{cid}:{x}" for x in c.get("standards",[]));migrations.add(f"{cid}:declared-loss") if c.get("migration_loss_declared") else None;evidence.add(c["expected_digest"])
  if c["evidence_state"]=="unknown" or c["expected_digest"]==request["replay_identity"]:unknown.add(cid)
  elif c["expected_digest"]!=c["observed_digest"] or c["canonical_bytes"]!="canonical":mismatch.add(cid)
  elif cid not in request["required_case_order"]:omitted.add(cid)
  else:parity.add(cid)
 missing=[i for i in request["required_case_order"] if i not in seen];global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or len(rows)>request["case_budget"]
 if global_block:omitted.update(seen);parity.clear();mismatch.clear();unknown.clear()
 disposition="blocked" if global_block else "unknown" if missing or unknown else "partial" if mismatch or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"case_order":sorted(seen),"parity_order":sorted(parity),"mismatch_order":sorted(mismatch),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"language_order":sorted(languages),"standards_order":sorted(standards),"migration_order":sorted(migrations),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["effect_receipts"]=[f"approve:replay:{request['request_id']}"] if disposition=="qualified" else ["block:semantic-drift"];payload["artifact"]={"artifact_id":f"conformance-replay:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY};validate(payload,feature_id=feature_id);return payload
__all__=["BOUNDARY","CONTENT_TYPE","ReplayCase4","ReplayIntegrityRequest4","ReplayIntegrityCard7","ReplayIntegrityArtifact4","ReplayIntegrityError","manifest","qualify","validate"]
