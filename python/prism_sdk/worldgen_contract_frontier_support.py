"""Deterministic Python parity for Worldgen P25 contract-frontier admission."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.contract-frontier-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["frontier steward","research scientist","schema maintainer","release operator"],"behavior":f"admit versioned contract-frontier candidates at {scale} ({mode} scale)","value":"turns novelty and support claims into auditable, typed, replayable capability admission decisions","input_schema":"ContractFrontierRequest4@1","output_schema":"ContractFrontierCard7@1","effects":["emit:frontier-card","block:unsafe-release"],"permissions":["read:local-capability-candidates"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(output:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=output.get("artifact",{}); bad=output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and output.get("feature_id")!=feature_id or output.get("boundary")!=PRECLINICAL_BOUNDARY or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or not output.get("candidate_order") or not _digest(output.get("replay_identity")) or not _digest(output.get("frontier_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=output.get("frontier_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("frontier identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("candidate_order","selected_order","rejected_order","unknown_order","omitted_order","uncertainty_order","negative_evidence_order","capability_order","effect_receipts"):
  if not _ordered(output.get(k,[])):raise ResearchContractError("frontier vectors are not canonical")
 ids=set(output["candidate_order"]);parts=set(output.get("selected_order",[]))|set(output.get("rejected_order",[]))|set(output.get("unknown_order",[]))|set(output.get("omitted_order",[]))
 if ids!=parts:raise ResearchContractError("candidate states do not partition")
def admit(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if any(not isinstance(request.get(k),str) or not request[k].strip() for k in ("request_id","scope","benchmark_id")) or not request.get("candidates") or not request.get("required_capability_order") or not 0<=request.get("min_novelty_milli",-1)<=1000 or not 0<=request.get("min_support_milli",-1)<=1000 or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_capability_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("frontier identity, thresholds, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["candidates"],key=lambda c:c.get("candidate_id",""));order=[];selected=set();rejected=set();unknown=set();omitted=set();uncertainty=set();negative=set();caps=set();schemas=set();provenance=set()
 for c in rows:
  cid=c.get("candidate_id","")
  if cid in order or not cid.strip() or not c.get("capability_id","").strip() or not c.get("version","").strip() or not 0<=c.get("novelty_score_milli",-1)<=1000 or not 0<=c.get("support_score_milli",-1)<=1000 or not c.get("consumer","").strip() or not _digest(c.get("schema_digest")) or not _digest(c.get("provenance_digest")) or not _digest(c.get("replay_identity")):raise ResearchContractError("candidate identity, score, consumer, or digest is invalid")
  order.append(cid);caps.add(c["capability_id"]);schemas.add(c["schema_digest"]);provenance.add(c["provenance_digest"])
  if c.get("negative_result") is True:negative.add(f"{cid}:negative-result")
  if c.get("replay_identity")!=request["replay_identity"] or c.get("local") is not True or c.get("aggregate_only") is not True:omitted.add(cid)
  elif c.get("evidence_state") in {"unknown","unmeasured"}:unknown.add(cid)
  elif c.get("evidence_state")=="contradicted":rejected.add(cid);uncertainty.add(f"{cid}:contradicted")
  elif c.get("evidence_state") in {"proven","supported"} and c["novelty_score_milli"]>=request["min_novelty_milli"] and c["support_score_milli"]>=request["min_support_milli"]:selected.add(cid)
  elif c.get("evidence_state") in {"proven","supported"}:rejected.add(cid);uncertainty.add(f"{cid}:threshold")
  else:unknown.add(cid)
 blocked=not request.get("policy_allowed") or not request.get("protected_closure") or not request.get("signed_approval") or not request.get("network_available") or bool(request.get("adversarial_events")) or mode=="research copilot" and (request.get("action_budget",0)<=0 or request.get("action_count",0)>request.get("action_budget",0))
 if blocked:omitted.update(order);selected.clear();rejected.clear();unknown.clear()
 chosen={c["capability_id"] for c in rows if c.get("candidate_id") in selected};required=set(request["required_capability_order"]);disp="blocked" if blocked else "approval_required" if not request.get("signed_approval") and mode!="inference" else "qualified" if required<=chosen and selected and not (rejected or unknown or omitted) else "unresolved" if selected else "unknown"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"benchmark_id":request["benchmark_id"],"disposition":disp,"candidate_order":order,"selected_order":sorted(selected),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"capability_order":sorted(caps),"schema_digest_order":sorted(schemas),"provenance_digest_order":sorted(provenance),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
 d=_hash(payload);payload["frontier_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-contract-frontier:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(omitted),"candidate_digests":sorted(schemas),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:frontier-card:{request['request_id']}"] if disp=="qualified" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
FrontierCandidate4=dict[str,Any];ContractFrontierRequest4=dict[str,Any];ContractFrontierCard7=dict[str,Any];ContractFrontierError=ResearchContractError
__all__=["CONTENT_TYPE","FrontierCandidate4","ContractFrontierRequest4","ContractFrontierCard7","ContractFrontierError","manifest","admit","validate"]
