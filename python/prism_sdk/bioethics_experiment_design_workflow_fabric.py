"""Parity implementation for ``AFA-bioethics-P09-F13``."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
FEATURE_ID="AFA-bioethics-P09-F13"; CONTRACT_VERSION="bioethics-local-single-study-experiment-design-workflow-fabric/1.0"; INPUT_SCHEMA="ExperimentObjective1@1"; OUTPUT_SCHEMA="ExecutableExperimentDesign4@1"; CONTENT_TYPE="application/vnd.aurora.bioethics-executable-experiment-design-4+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def experiment_design_workflow_fabric_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"bioethics","consumers":["research data steward","workflow operator","protocol scientist"],"behavior":"compile typed preclinical experiment objectives into deterministic resumable workflow schedules with evidence and policy gates","value":"gives research teams a replayable design schedule while preventing unsupported, cyclic, or unauthorized experimental work from being released","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute:local-computation","write:local-artifact"],"permissions":["execute:approved-workflows"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _topological(steps:list[Mapping[str,Any]])->list[str]:
    ids={s.get("step_id") for s in steps}
    if len(ids)!=len(steps) or any(not isinstance(i,str) or not i.strip() for i in ids):raise ResearchContractError("step ids must be unique and non-empty")
    indegree={i:0 for i in ids}; edges={i:set() for i in ids}
    for s in steps:
        deps=s.get("depends_on",[])
        if not _ordered(deps) or not s.get("declared_effect") or not s.get("duration_budget") or any(d not in ids for d in deps):raise ResearchContractError(f"invalid dependencies or budget for step {s['step_id']}")
        for d in deps:edges[d].add(s["step_id"]); indegree[s["step_id"]]+=1
    ready=sorted(i for i,n in indegree.items() if n==0); out=[]
    while ready:
        i=ready.pop(0);out.append(i)
        for child in sorted(edges[i]):indegree[child]-=1; ready.append(child) if indegree[child]==0 else None
        ready.sort()
    if len(out)!=len(ids):raise ResearchContractError("workflow dependency cycle detected")
    return out
def validate_experiment_design_workflow(output:Mapping[str,Any])->None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("step_order") or not output.get("effect_receipts"):raise ResearchContractError("design identity, locality, steps, disposition, or effects are incomplete")
    fields=("step_order","ready_order","blocked_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
    if any(not _ordered(output.get(k,[])) for k in fields):raise ResearchContractError("design ordering is not canonical")
    ids=set(output["step_order"]);parts=sum((output.get(k,[]) for k in ("ready_order","blocked_order")),[])
    if len(ids)!=len(output["step_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("step states do not partition")
    if not _digest(output.get("replay_identity")) or not _digest(output.get("design_digest")) or a.get("content_hash")!=output.get("design_digest") or any(not _digest(v) for v in a.get("provenance_digests",[])):raise ResearchContractError("design digest is inconsistent")
    if any(v!="block:unsafe-release" and not v.startswith("schedule:research-work:") for v in output["effect_receipts"]):raise ResearchContractError("design effect is outside workflow gate")
    if output["disposition"]=="qualified" and output["effect_receipts"]!=[f"schedule:research-work:{output['request_id']}"]:raise ResearchContractError("qualified design effect is invalid")
    if output["disposition"]!="qualified" and output["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified design must block")
def compile_experiment_design_workflow(r:Mapping[str,Any])->dict[str,Any]:
    if r.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(r.get(k),str) or not r[k].strip() for k in ("request_id","consumer","purpose","target_scope","semantic_profile")) or not r.get("minimum_power_milli") or not _digest(r.get("replay_identity")) or not r.get("budget") or r.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("workflow identity, bounds, replay, budget, or boundary is invalid")
    o=r.get("objective",{});
    if not o.get("objective_id") or o.get("target_scope")!=r["target_scope"] or o.get("semantic_profile")!=r["semantic_profile"] or not _digest(o.get("artifact_digest")) or not _digest(o.get("provenance_digest")) or o.get("replay_identity")!=r["replay_identity"] or o.get("power_milli",0)<r["minimum_power_milli"] or not o.get("steps") or not _ordered(o.get("omission_order",[])):raise ResearchContractError("objective scope, evidence, power, replay, or artifact closure is invalid")
    _execution_order=_topological(o["steps"]); order=sorted(s["step_id"] for s in o["steps"]);ready:set[str]=set();blocked:set[str]=set();omissions=set(o.get("omission_order",[]));uncertainty:set[str]=set();negative={f"{o['objective_id']}:negative-result"} if o.get("negative_result") else set(); total=sum(s.get("duration_budget",0) for s in o["steps"])
    for s in o["steps"]:
        if s.get("required") and total>r["budget"]:blocked.add(s["step_id"]);omissions.add(f"{s['step_id']}:budget-exhausted")
        elif o.get("evidence_state") not in {"proven","supported"}:blocked.add(s["step_id"]);uncertainty.add(f"{s['step_id']}:evidence-state")
        else:ready.add(s["step_id"])
    for ok,label in ((r.get("policy_allowed"),"workflow:policy-denied"),(r.get("protected_closure"),"workflow:protected-closure-incomplete"),(r.get("signed_approval"),"workflow:signed-approval-missing"),(r.get("raw_data_local"),"workflow:raw-data-not-local"),(r.get("aggregate_only"),"workflow:aggregate-only-required")):
        if not ok:omissions.add(label)
    global_block=not all(r.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","raw_data_local","aggregate_only"));disposition="blocked" if global_block or blocked else "partial" if not ready else "qualified"
    if global_block:blocked.update(order);ready.clear()
    if disposition!="qualified":omissions.add("workflow:design-closure-not-ready")
    payload={"step_order":order,"ready_order":sorted(ready),"blocked_order":sorted(blocked),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"replay_identity":r["replay_identity"]};digest=_hash(payload)
    out={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"consumer":r["consumer"],"purpose":r["purpose"],"target_scope":r["target_scope"],"semantic_profile":r["semantic_profile"],"disposition":disposition,**payload,"design_digest":digest,"artifact":{"artifact_id":f"bioethics-experiment-design:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[] if disposition=="qualified" else ["design-not-scheduled"],"provenance_digests":[o["provenance_digest"]],"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"schedule:research-work:{r['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};validate_experiment_design_workflow(out);return out
def compile_experiment_design_workflow_json(value:Mapping[str,Any])->dict[str,Any]:return compile_experiment_design_workflow(value)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","experiment_design_workflow_fabric_manifest","compile_experiment_design_workflow","compile_experiment_design_workflow_json","validate_experiment_design_workflow"]
