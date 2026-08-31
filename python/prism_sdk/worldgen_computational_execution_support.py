"""Deterministic Python parity for Worldgen P12 computational execution."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-worldgen-P12-F01"; CONTRACT_VERSION="worldgen-local-computational-execution/1.0"; INPUT_SCHEMA="ResearchWorkflowSpec3@1"; OUTPUT_SCHEMA="ExecutionRun7@1"; CONTENT_TYPE="application/vnd.aurora.worldgen.execution-run-receipt+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class ExecutionRun7:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or v.get("federation_export")!="aggregate-digest-only" or v.get("raw_data_local") is not True or not v.get("plan_order") or len(v.get("decisions",[]))!=len(v["plan_order"]) or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("execution identity, locality, plan, decisions, or effects are incomplete")
        fields=("plan_order","topological_order","completed_order","unresolved_order","blocked_order","cycle_order","missing_dependency_order","compensation_order","omissions","uncertainty","negative_evidence","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("execution ordering is not canonical")
        ids=set(v["plan_order"]);parts=v["completed_order"]+v["unresolved_order"]+v["blocked_order"]
        if len(ids)!=len(v["plan_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("execution states do not partition plan")
        if not all(_digest(v.get(k)) for k in ("checkpoint_digest","replay_identity","run_digest")) or a.get("content_hash")!=v.get("run_digest"):raise ResearchContractError("execution digest or artifact metadata is invalid")
        if any(e!="block:unsafe-release" and not e.startswith("verify:execution-plan:") for e in v["effect_receipts"]):raise ResearchContractError("execution effect is outside verification gate")
def manifest(*,feature_id:str,contract_version:str,scale:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","computational execution operator","release evidence reviewer"],"behavior":f"verify a bounded preclinical research execution graph for {scale} and emit replayable run evidence without dispatching code","value":"prevents cycles, missing dependencies, policy violations, budget overruns, and unreplayable plans from entering computation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["block:unsafe-release"],"permissions":["evaluate:capability-runs"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if r.get("schema_version")!=INPUT_SCHEMA or not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","run_id","workflow_id","scope")) or not r.get("nodes") or int(r.get("budget_units",0))<=0 or int(r.get("budget_units",0))>int(r.get("max_budget_units",0)) or not _digest(r.get("replay_identity")) or r.get("raw_data_local") is not True or r.get("federated_summary_only") is not True or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("adversarial_events")!=sorted(set(r.get("adversarial_events",[]))):raise ResearchContractError("execution identity, bounds, locality, replay, or boundary is invalid")
    ids=set()
    for n in r["nodes"]:
        if not isinstance(n.get("node_id"),str) or not n["node_id"].strip() or n["node_id"] in ids or not _ordered(n.get("dependency_order",[])) or not _ordered(n.get("omissions",[])) or not _ordered(n.get("uncertainty",[])) or not _digest(n.get("replay_identity")):raise ResearchContractError("node identifiers, dependencies, evidence, or digests are invalid")
        ids.add(n["node_id"])
def assure_computational_execution(request:Mapping[str,Any],*,feature_id:str=FEATURE_ID,contract_version:str=CONTRACT_VERSION)->ExecutionRun7:
    _validate_request(request);nodes=sorted((dict(n) for n in request["nodes"]),key=lambda n:n["node_id"]);plan=[n["node_id"] for n in nodes];known=set(plan);indegree={n["node_id"]:sum(d in known for d in n.get("dependency_order",[])) for n in nodes};children={}
    for n in nodes:
        for d in n.get("dependency_order",[]):
            if d in known:children.setdefault(d,[]).append(n["node_id"])
    queue=sorted(i for i in plan if indegree[i]==0);topo=[]
    while queue:
        i=queue.pop(0);topo.append(i)
        for child in sorted(children.get(i,[])):indegree[child]-=1;queue.append(child) if indegree[child]==0 else None
        queue.sort()
    cycle=sorted(set(plan)-set(topo));completed=[];unresolved=[];blocked=[];missing=set();omissions=set();uncertainty=set();negative=set();decisions=[];loss=[];spent=0;global_block=not all(request.get(k) is True for k in ("policy_allow","protected_closure","raw_data_local","federated_summary_only")) or bool(request.get("adversarial_events"))
    for n in nodes:
        i=n["node_id"];deps=n.get("dependency_order",[]);missing.update(f"{i}:{d}" for d in deps if d not in known);omissions.update(f"{i}:{x}" for x in n.get("omissions",[]));uncertainty.update(f"{i}:{x}" for x in n.get("uncertainty",[]));negative.add(f"{i}:execution-not-started") if n.get("evidence_state")=="negative" else None
        failed=global_block or not n.get("policy_allowed",False) or not n.get("protected_closure",False) or not n.get("artifact_digest") or not n.get("provenance_digest") or i in cycle or n.get("replay_identity")!=request["replay_identity"];pending=not failed and (n.get("evidence_state") not in {"proven","supported"} or bool(n.get("omissions")) or bool(n.get("uncertainty")) or any(d not in known or d not in completed for d in deps))
        if int(n.get("estimated_cost",0))>int(request["budget_units"])-spent:pending=True;omissions.add(f"{i}:budget-ceiling")
        else:spent+=int(n.get("estimated_cost",0))
        state="blocked" if failed else "unresolved" if pending else "completed"; (blocked if state=="blocked" else unresolved if state=="unresolved" else completed).append(i); decisions.append({"node_id":i,"effect_kind":n.get("effect_kind",""),"disposition":state});loss.append({"field":f"node:{i}","reason":"execution gate failed","severity":"decision_relevant"}) if failed else None
    if global_block:completed=[];unresolved=[];blocked=plan[:];omissions.add("request:policy-or-locality-blocked")
    completed,unresolved,blocked=sorted(completed),sorted(unresolved),sorted(blocked);disp="blocked" if global_block or blocked else "unresolved" if unresolved else "qualified";omissions.add("request:execution-closure-not-ready") if disp!="qualified" else None
    checkpoint=_hash({"run_id":request["run_id"],"plan_order":plan,"topological_order":topo,"replay_identity":request["replay_identity"]});payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"run_id":request["run_id"],"workflow_id":request["workflow_id"],"scope":request["scope"],"disposition":disp,"plan_order":plan,"topological_order":topo,"completed_order":completed,"unresolved_order":unresolved,"blocked_order":blocked,"cycle_order":cycle,"missing_dependency_order":sorted(missing),"compensation_order":[],"decisions":decisions,"checkpoint_digest":checkpoint,"replay_identity":request["replay_identity"],"semantic_loss":loss,"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"negative_evidence":sorted(negative),"raw_data_local":True,"federation_export":"aggregate-digest-only","boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);payload["run_digest"]=digest;payload["artifact"]={"artifact_id":f"execution-run-7:{request['run_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":loss,"provenance":[{"source_id":request["run_id"],"relation":"computational-execution-assurance","digest":digest}],"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"verify:execution-plan:{request['run_id']}"] if disp=="qualified" else ["block:unsafe-release"];out=ExecutionRun7(payload);out.validate();return out
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ExecutionRun7","manifest","assure_computational_execution"]
