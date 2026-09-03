"""Deterministic Python parity for Worldgen P29 scale frontier."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.scale-frontier-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["capacity planner","performance steward","research operator","release auditor"],"behavior":f"evaluate throughput, latency, and cost scale-frontier candidates at {scale} ({mode} scale)","value":"turns scaling claims into auditable SLO, economics, and uncertainty decisions","input_schema":"ScaleFrontierRequest4@1","output_schema":"ScaleFrontierCard7@1","effects":["emit:scale-frontier-card","block:unsafe-release"],"permissions":["read:local-benchmark-summaries"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("candidate_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("frontier_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("frontier_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("scale identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("candidate_order","selected_order","rejected_order","unknown_order","omitted_order","uncertainty_order","negative_evidence_order","scale_order","slo_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("scale vectors are not canonical")
 if set(o["candidate_order"])!=set(o.get("selected_order",[]))|set(o.get("rejected_order",[]))|set(o.get("unknown_order",[]))|set(o.get("omitted_order",[])):raise ResearchContractError("scale states do not partition")
def evaluate(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("candidates") or not request.get("required_scale_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_scale_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("scale identity, candidates, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["candidates"],key=lambda c:c.get("candidate_id",""));order=[];selected=set();rejected=set();unknown=set();omitted=set();uncertainty=set();negative=set();scales=set();benchmarks=set()
 for c in rows:
  cid=c.get("candidate_id","")
  if cid in order or not cid.strip() or not c.get("scale","").strip() or not _digest(c.get("benchmark_digest")) or not _digest(c.get("replay_identity")) or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError("candidate identity, digest, or locality is invalid")
  order.append(cid);scales.add(c["scale"]);benchmarks.add(c["benchmark_digest"])
  if c.get("negative_result") is True:negative.add(f"{cid}:negative-result")
  if c["replay_identity"]!=request["replay_identity"]:omitted.add(cid)
  elif c.get("evidence_state") in {"unknown","unmeasured"}:unknown.add(cid)
  elif c.get("evidence_state") in {"proven","supported"} and c.get("throughput_milli",0)>=request.get("min_throughput_milli",0) and c.get("latency_milli",0)<=request.get("max_latency_milli",0) and c.get("cost_milli",0)<=request.get("max_cost_milli",0):selected.add(cid)
  elif c.get("evidence_state") in {"proven","supported"}:rejected.add(cid);uncertainty.add(f"{cid}:slo-or-cost")
  else:rejected.add(cid)
 blocked=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","network_available","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events"))
 if blocked:omitted.update(order);selected.clear();rejected.clear();unknown.clear()
 disp="blocked" if blocked else "unresolved" if not set(request["required_scale_order"])<=scales or rejected or unknown or omitted else "qualified";payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"disposition":disp,"candidate_order":order,"selected_order":sorted(selected),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"scale_order":sorted(scales),"slo_order":[f"throughput>= {request.get('min_throughput_milli',0)}",f"latency<= {request.get('max_latency_milli',0)}",f"cost<= {request.get('max_cost_milli',0)}"],"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["frontier_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-scale-frontier:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(uncertainty),"benchmark_digests":sorted(benchmarks),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:scale-frontier-card:{request['request_id']}"] if disp=="qualified" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
ScaleCandidate4=dict[str,Any];ScaleFrontierRequest4=dict[str,Any];ScaleFrontierCard7=dict[str,Any];ScaleFrontierError=ResearchContractError
__all__=["CONTENT_TYPE","ScaleCandidate4","ScaleFrontierRequest4","ScaleFrontierCard7","ScaleFrontierError","manifest","evaluate","validate"]
