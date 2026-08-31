"""Deterministic Python parity for World P32 causal-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.world.causal-integrity-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"world","consumers":["causal compiler","mechanism explorer","provenance ledger","release auditor"],"behavior":f"qualify causal-world closure at {scale} ({mode})","value":"prevents cyclic, stale, or silently incomplete causal explanations from entering a replayable research object","input_schema":"CausalIntegrityRequest4@1","output_schema":"CausalIntegrityCard7@1","effects":["emit:causal-card","retain:causal-omissions","block:unsafe-release"],"permissions":["read:local-causal-edges"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("edge_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("causal identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("edge_order","accepted_order","rejected_order","unknown_order","omitted_order","node_order","relation_order","epoch_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("causal vectors are not canonical")
 states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["edge_order"])!=len(set(o["edge_order"])) or states!=set(o["edge_order"]):raise ResearchContractError("edge states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("edges") or not request.get("required_node_order") or not request.get("required_relation_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_node_order"]) or not _ordered(request["required_relation_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("causal identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["edges"],key=lambda e:e.get("edge_id",""));order=[];accepted=set();rejected=set();unknown=set();omitted=set();nodes=set();relations=set();epochs=set();negative=set();digests=set()
 for e in rows:
  eid=e.get("edge_id","")
  if eid in order or not isinstance(eid,str) or not eid.strip() or not isinstance(e.get("cause_id"),str) or not e["cause_id"].strip() or not isinstance(e.get("effect_id"),str) or not e["effect_id"].strip() or e["cause_id"]==e["effect_id"] or not isinstance(e.get("relation"),str) or not e["relation"].strip() or not _digest(e.get("evidence_digest")) or e.get("local") is not True or e.get("aggregate_only") is not True:raise ResearchContractError("edge identity, endpoints, evidence, or locality is invalid")
  order.append(eid);nodes.update((e["cause_id"],e["effect_id"]));relations.add(e["relation"]);epochs.add(f"{e['relation']}:{e.get('policy_epoch',0)}");digests.add(e["evidence_digest"])
  if e.get("negative_result") is True:negative.add(f"{eid}:negative-result")
  if e.get("policy_epoch",0)==0:unknown.add(eid)
  elif e["relation"] not in request["required_relation_order"] or e["cause_id"] not in request["required_node_order"] or e["effect_id"] not in request["required_node_order"]:rejected.add(eid)
  elif e["evidence_digest"]==request["replay_identity"]:omitted.add(eid)
  else:accepted.add(eid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);accepted.clear();rejected.clear();unknown.clear()
 missing=not set(request["required_node_order"])<=nodes or not set(request["required_relation_order"])<=relations;disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"edge_order":order,"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"node_order":sorted(nodes),"relation_order":sorted(relations),"epoch_order":sorted(epochs),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["artifact"]={"artifact_id":f"world-causal:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"edge_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:causal-card:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
CausalEdge4=dict[str,Any];CausalIntegrityRequest4=dict[str,Any];CausalIntegrityCard7=dict[str,Any];CausalIntegrityArtifact4=dict[str,Any];CausalIntegrityError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","CausalEdge4","CausalIntegrityRequest4","CausalIntegrityCard7","CausalIntegrityArtifact4","CausalIntegrityError","manifest","qualify","validate"]
