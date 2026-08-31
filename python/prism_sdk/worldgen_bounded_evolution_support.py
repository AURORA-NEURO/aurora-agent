"""Deterministic Python parity for Worldgen P32 bounded evolution."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.bounded-evolution-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["research platform steward","evaluation operator","change approver","release auditor"],"behavior":f"promote bounded research-system evolution at {scale} ({mode})","value":"turns prospective self-improvement into independently reviewed, replayable, reversible product releases","input_schema":"BoundedEvolutionRequest4@1","output_schema":"BoundedEvolutionCard7@1","effects":["emit:evolution-card","retain:migration-witness","block:unsafe-release"],"permissions":["read:local-evaluation-cards"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("candidate_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("evolution_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("evolution_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("evolution identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("candidate_order","promoted_order","rejected_order","unknown_order","omitted_order","benchmark_order","review_order","uncertainty_order","negative_evidence_order","migration_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("evolution vectors are not canonical")
 states=set(o["promoted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["candidate_order"])!=len(set(o["candidate_order"])) or states!=set(o["candidate_order"]):raise ResearchContractError("candidate states do not partition")
def promote(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("candidates") or not request.get("required_candidate_order") or not request.get("benchmark_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_candidate_order"]) or not _ordered(request["benchmark_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("evolution identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["candidates"],key=lambda c:c.get("candidate_id",""));order=[];promoted=set();rejected=set();unknown=set();omitted=set();uncertainty=set();negative=set();migrations=set();digests=set()
 for c in rows:
  cid=c.get("candidate_id","")
  if cid in order or not isinstance(cid,str) or not cid.strip() or not _ordered(c.get("parent_ids",[])) or not isinstance(c.get("change_kind"),str) or not c["change_kind"].strip() or not _digest(c.get("evidence_digest")) or not _digest(c.get("baseline_digest")) or not _digest(c.get("replay_identity")) or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError("candidate identity, digest, safety, or locality is invalid")
  order.append(cid);digests.add(c["evidence_digest"])
  if c.get("negative_result") is True:negative.add(f"{cid}:negative-result")
  if c["replay_identity"]!=request["replay_identity"]:omitted.add(cid)
  elif c.get("deterministic") is not True or c.get("bounded") is not True or c.get("safety_impact")!="no-new-physical-effect":rejected.add(cid);uncertainty.add(f"{cid}:safety-or-bound")
  elif c["evidence_digest"]!=c["baseline_digest"]:rejected.add(cid);migrations.add(f"{cid}:baseline-migration")
  else:promoted.add(cid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","independent_review","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);promoted.clear();rejected.clear();unknown.clear()
 missing=not set(request["required_candidate_order"])<=set(order);disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "promoted"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"disposition":disposition,"candidate_order":order,"promoted_order":sorted(promoted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"benchmark_order":request["benchmark_order"],"review_order":["independent-review"],"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"migration_order":sorted(migrations),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["evolution_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-bounded-evolution:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"candidate_digests":sorted(digests),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:evolution-card:{request['request_id']}"] if disposition=="promoted" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
EvolutionCandidate4=dict[str,Any];BoundedEvolutionRequest4=dict[str,Any];BoundedEvolutionCard7=dict[str,Any];EvolutionArtifact4=dict[str,Any];BoundedEvolutionError=ResearchContractError
__all__=["CONTENT_TYPE","EvolutionCandidate4","BoundedEvolutionRequest4","BoundedEvolutionCard7","EvolutionArtifact4","BoundedEvolutionError","manifest","promote","validate"]
