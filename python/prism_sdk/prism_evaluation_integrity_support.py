"""Deterministic Python parity for PRISM P32 evaluation-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.prism.evaluation-integrity-card-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"prism","consumers":["matched-fork evaluator","baseline auditor","result-bundle publisher","release auditor"],"behavior":f"qualify matched decision-cell evaluation at {scale} ({mode})","value":"prevents incomplete or unbaselined evaluation arms from becoming scientific release claims","input_schema":"EvaluationIntegrityRequest4@1","output_schema":"EvaluationIntegrityCard7@1","effects":["emit:evaluation-card","retain:uncertainty","block:unsafe-claim"],"permissions":["read:local-evaluation-arms"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("arm_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest") or a.get("boundary")!=BOUNDARY
 if bad:raise ResearchContractError("evaluation identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("arm_order","accepted_order","rejected_order","unknown_order","omitted_order","metric_order","cell_order","baseline_order","negative_evidence_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("evaluation vectors are not canonical")
 states=set(o["accepted_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["arm_order"])!=len(set(o["arm_order"])) or states!=set(o["arm_order"]):raise ResearchContractError("arm states do not partition")
def evaluate(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("purpose"),str) or not request["purpose"].strip() or not request.get("arms") or not request.get("required_arm_order") or not request.get("required_metric_order") or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_arm_order"]) or not _ordered(request["required_metric_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("evaluation identity, requirements, digest, ordering, locality, or boundary is invalid")
 rows=sorted(request["arms"],key=lambda a:a.get("arm_id",""));order=[];accepted=set();rejected=set();unknown=set();omitted=set();metrics=set();cells=set();baselines=set();negative=set();digests=set()
 for a in rows:
  aid=a.get("arm_id","")
  if aid in order or not isinstance(aid,str) or not aid.strip() or not isinstance(a.get("cell_id"),str) or not a["cell_id"].strip() or not isinstance(a.get("baseline_id"),str) or not a["baseline_id"].strip() or not isinstance(a.get("metric"),str) or not a["metric"].strip() or not _digest(a.get("evidence_digest")) or a.get("local") is not True or a.get("aggregate_only") is not True:raise ResearchContractError("arm identity, baseline, metric, evidence, or locality is invalid")
  order.append(aid);metrics.add(a["metric"]);cells.add(a["cell_id"]);baselines.add(a["baseline_id"]);digests.add(a["evidence_digest"])
  if a.get("negative_result") is True:negative.add(f"{aid}:negative-result")
  if a.get("complete") is not True:unknown.add(aid)
  elif aid not in request["required_arm_order"] or a["metric"] not in request["required_metric_order"]:rejected.add(aid)
  elif a["evidence_digest"]==request["replay_identity"]:omitted.add(aid)
  else:accepted.add(aid)
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count",0)>request.get("action_budget",0)
 if global_block:omitted.update(order);accepted.clear();rejected.clear();unknown.clear()
 missing=not set(request["required_arm_order"])<=set(order) or not set(request["required_metric_order"])<=metrics;disposition="blocked" if global_block else "unknown" if missing else "partial" if rejected or unknown or omitted else "qualified"
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"arm_order":order,"accepted_order":sorted(accepted),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"metric_order":sorted(metrics),"cell_order":sorted(cells),"baseline_order":sorted(baselines),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["artifact"]={"artifact_id":f"prism-evaluation:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(payload["omitted_order"]),"arm_digests":sorted(digests),"boundary":BOUNDARY};payload["effect_receipts"]=[f"emit:evaluation-card:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-claim"];validate(payload,feature_id=feature_id);return payload
EvaluationArm4=dict[str,Any];EvaluationIntegrityRequest4=dict[str,Any];EvaluationIntegrityCard7=dict[str,Any];EvaluationIntegrityArtifact4=dict[str,Any];EvaluationIntegrityError=ResearchContractError
__all__=["BOUNDARY","CONTENT_TYPE","EvaluationArm4","EvaluationIntegrityRequest4","EvaluationIntegrityCard7","EvaluationIntegrityArtifact4","EvaluationIntegrityError","manifest","evaluate","validate"]
