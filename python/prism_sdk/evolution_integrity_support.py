"""Python parity for Mutation P32 evolution-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.mutation.evolution-integrity-card-1+json"
MutationCandidate4=dict[str,Any];EvolutionRequest4=dict[str,Any];EvolutionCard7=dict[str,Any];EvolutionArtifact4=dict[str,Any];EvolutionIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"mutation","consumers":["metamorphic planner","lineage explorer","analysis runtime","research workbench"],"behavior":f"qualify lineage-aware mutation candidates at {scale} ({mode})","value":"prevents orphaned, untyped, unproven, or unsafe mutation candidates from driving preclinical research workflows","input_schema":"EvolutionRequest4@1","output_schema":"EvolutionCard7@1","effects":["emit:evolution-card","retain:lineage-evidence","block:unsafe-evolution"],"permissions":["read:local-mutation-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or(feature_id is not None and o.get("feature_id")!=feature_id)or not o.get("request_id")or not o.get("purpose")or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad:raise ResearchContractError("evolution identity, locality, artifact, digest, or boundary is incomplete")
 for k in("mutation_order","accepted_order","rejected_order","unknown_order","omitted_order","parent_order","effect_order","lineage_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("evolution vectors are not canonical")
 ids=set(o["mutation_order"]);states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["mutation_order"])!=len(ids)or states!=ids:raise ResearchContractError("mutation states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("candidates") or not request.get("required_mutation_order") or not _ordered(request["required_mutation_order"]) or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("mutation_budget",0)<=0:raise ResearchContractError("evolution identity, ordering, digest, locality, boundary, or budget is invalid")
 rows=sorted(request["candidates"],key=lambda a:a.get("mutation_id",""));seen=set();accepted=set();rejected=set();unknown=set();omitted=set();parents=set();effects=set();lineage=set();evidence=set()
 for c in rows:
  mid=c.get("mutation_id","")
  if not mid.strip()or mid in seen or not c.get("parent_id","").strip()or c["parent_id"]==mid or not _digest(c.get("digest"))or not c.get("effect_class","").strip()or not c.get("evidence_state","").strip()or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError("mutation identity, parent, digest, effect, evidence, or locality is invalid")
  seen.add(mid);parents.add(f"{mid}:{c['parent_id']}");effects.add(f"{mid}:{c['effect_class']}");lineage.add(f"{mid}:{c['parent_id']}");evidence.add(c["digest"])
  if c["evidence_state"]=="unknown"or c["digest"]==request["replay_identity"]:unknown.add(mid)
  elif c["effect_class"] in {"unsafe","unknown"}:rejected.add(mid)
  elif mid not in request["required_mutation_order"]:omitted.add(mid)
  else:accepted.add(mid)
 missing=[x for x in request["required_mutation_order"] if x not in seen];parent_missing=any(c.get("parent_id")!="root" and c.get("parent_id") not in seen for c in rows);global_block=not all(request.get(k) is True for k in("policy_allowed","protected_closure","signed_manifest","raw_data_local","aggregate_only"))or bool(request.get("adversarial_events"))or len(rows)>request["mutation_budget"]
 if global_block:omitted.update(seen);accepted.clear();rejected.clear();unknown.clear()
 disposition="blocked" if global_block else "unknown" if missing or parent_missing or unknown else "partial" if rejected or omitted else "qualified";payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"mutation_order":sorted(seen),"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"parent_order":sorted(parents),"effect_order":sorted(effects),"lineage_order":sorted(lineage),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["effect_receipts"]=[f"approve:evolution:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-evolution"];payload["artifact"]={"artifact_id":f"mutation-evolution:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY};validate(payload,feature_id=feature_id);return payload
__all__=["BOUNDARY","CONTENT_TYPE","MutationCandidate4","EvolutionRequest4","EvolutionCard7","EvolutionArtifact4","EvolutionIntegrityError","manifest","qualify","validate"]
