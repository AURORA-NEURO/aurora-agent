"""Python parity for Governance P32 schema-evolution integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE="application/vnd.aurora.governance.evolution-integrity-card-1+json"
EvolutionChange4=dict[str,Any]; EvolutionIntegrityRequest4=dict[str,Any]; EvolutionIntegrityCard7=dict[str,Any]; EvolutionIntegrityArtifact4=dict[str,Any]; EvolutionIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"governance","consumers":["schema registry","migration release gate","artifact verifier","research workbench"],"behavior":f"qualify digest-safe schema evolution at {scale} ({mode})","value":"prevents undeclared compatibility breaks, lossy migrations, and premature deprecation from releasing","input_schema":"EvolutionIntegrityRequest4@1","output_schema":"EvolutionIntegrityCard7@1","effects":["emit:evolution-card","retain:loss-witness","block:unsafe-migration"],"permissions":["read:local-migration-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{}); bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and o.get("feature_id")!=feature_id) or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or not o.get("raw_data_local") is True or not o.get("aggregate_only") is True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad: raise ResearchContractError("identity, locality, artifact, digest, or boundary is incomplete")
 for k in ("change_order","accepted_order","rejected_order","unknown_order","omitted_order","class_order","version_order","loss_order","deprecation_order","effect_receipts"):
  if not _ordered(o.get(k,[])): raise ResearchContractError("evolution vectors are not canonical")
 ids=set(o["change_order"]); states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["change_order"])!=len(ids) or states!=ids: raise ResearchContractError("change states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("changes") or not request.get("required_change_order") or not _ordered(request["required_change_order"]) or not request.get("required_version_bump") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("declared_change_count")!=len(request["changes"]) or request.get("change_budget",0)<=0: raise ResearchContractError("identity, ordering, bump, digest, locality, boundary, or budget is invalid")
 rows=sorted(request["changes"],key=lambda c:c.get("change_id","")); seen=set(); accepted=set(); rejected=set(); unknown=set(); omitted=set(); classes=set(); versions=set(); losses=set(); deprecations=set(); evidence=set()
 for c in rows:
  cid=c.get("change_id","")
  if not cid.strip() or cid in seen or not c.get("field_path","").strip() or not c.get("required_class","").strip() or not c.get("declared_class","").strip() or not c.get("old_version","").strip() or not c.get("new_version","").strip() or not c.get("deprecation_stage","").strip() or not c.get("evidence_state","").strip() or not _digest(c.get("evidence_digest")) or c.get("local") is not True or c.get("aggregate_only") is not True: raise ResearchContractError("change identity, versions, evidence, or locality is invalid")
  seen.add(cid); classes.add(f"{cid}:{c['declared_class']}"); versions.add(f"{cid}:{c['old_version']}->{c['new_version']}"); deprecations.add(f"{cid}:{c['deprecation_stage']}"); evidence.add(c["evidence_digest"])
  if c.get("digest_affecting") and c["declared_class"]!="major": losses.add(f"{cid}:digest-affecting-requires-major")
  if not c.get("roundtrip_preserved") and not c.get("loss_declared"): losses.add(f"{cid}:undeclared-roundtrip-loss")
  if c.get("evidence_state")=="unknown" or c["evidence_digest"]==request["replay_identity"]: unknown.add(cid)
  elif c.get("deprecation_stage")=="retired" or (c.get("digest_affecting") and c["declared_class"]!="major") or (not c.get("roundtrip_preserved") and not c.get("loss_declared")) or c["declared_class"]!=c["required_class"]: rejected.add(cid)
  elif cid not in request["required_change_order"]: omitted.add(cid)
  else: accepted.add(cid)
 missing=[i for i in request["required_change_order"] if i not in seen]; losses.update(f"{i}:required-change-missing" for i in missing)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or len(rows)>request["change_budget"] or request["required_version_bump"]=="unknown"
 if global_block: omitted.update(seen); accepted.clear(); rejected.clear(); unknown.clear()
 complete=all(i in seen for i in request["required_change_order"]); disposition="blocked" if global_block else "unknown" if not complete or unknown else "partial" if rejected or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"change_order":sorted(seen),"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"class_order":sorted(classes),"version_order":sorted(versions),"loss_order":sorted(losses),"deprecation_order":sorted(deprecations),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY}; d=_hash(payload); payload["closure_digest"]=d; payload["effect_receipts"]=[f"approve:evolution:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-migration"]; payload["artifact"]={"artifact_id":f"governance-evolution:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY}; validate(payload,feature_id=feature_id); return payload
__all__=["BOUNDARY","CONTENT_TYPE","EvolutionChange4","EvolutionIntegrityRequest4","EvolutionIntegrityCard7","EvolutionIntegrityArtifact4","EvolutionIntegrityError","manifest","qualify","validate"]
