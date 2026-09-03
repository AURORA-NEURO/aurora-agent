"""Parity implementation for ``AFA-onco-P12-F05``."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
FEATURE_ID="AFA-onco-P12-F05"; CONTRACT_VERSION="onco-local-single-study-computational-execution-contract-model/1.0"; INPUT_SCHEMA="ResearchWorkflowSpec1@1"; OUTPUT_SCHEMA="ExecutionRun2@1"; CONTENT_TYPE="application/vnd.aurora.onco-execution-run-2+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def computational_execution_contract_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"onco","consumers":["benchmark curator","workflow schema steward","replay auditor"],"behavior":"validate and canonicalize local preclinical research execution graphs into replayable run contracts without dispatching work","value":"gives benchmark curators a stable typed graph and replay identity while keeping execution authority outside the domain model","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY}
def _acyclic(nodes:list[Mapping[str,Any]])->bool:
    ids={n.get("node_id") for n in nodes}; indegree={i:0 for i in ids}; edges={i:set() for i in ids}
    for n in nodes:
        for d in n.get("depends_on",[]):edges[d].add(n["node_id"]);indegree[n["node_id"]]+=1
    ready=sorted(i for i,n in indegree.items() if n==0);seen=0
    while ready:
        i=ready.pop(0);seen+=1
        for c in sorted(edges[i]):indegree[c]-=1;ready.append(c) if indegree[c]==0 else None
        ready.sort()
    return seen==len(ids)
def _validate_request(r:Mapping[str,Any])->None:
    if r.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(r.get(k),str) or not r[k].strip() for k in ("request_id","consumer","purpose","scope")) or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or not r.get("nodes"):raise ResearchContractError("workflow identity, replay, boundary, or node closure is invalid")
    ids:set[str]=set()
    for n in r["nodes"]:
        if not isinstance(n.get("node_id"),str) or not n["node_id"].strip() or n["node_id"] in ids or not _ordered(n.get("depends_on",[])) or any(d not in {x.get("node_id") for x in r["nodes"]} for d in n.get("depends_on",[])) or not _digest(n.get("artifact_digest")) or not _digest(n.get("provenance_digest")) or n.get("replay_identity")!=r["replay_identity"]:raise ResearchContractError("node identity, dependency, digest, or replay is invalid")
        ids.add(n["node_id"])
def validate_computational_execution_contract(output:Mapping[str,Any])->None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("node_order"):raise ResearchContractError("execution identity, locality, disposition, or node closure is incomplete")
    if any(not _ordered(output.get(k,[])) for k in ("node_order","valid_order","invalid_order","omission_order","uncertainty_order","negative_evidence_order")):raise ResearchContractError("execution ordering is not canonical")
    ids=set(output["node_order"]);parts=output.get("valid_order",[])+output.get("invalid_order",[])
    if len(ids)!=len(output["node_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("execution node states do not partition")
    if not _digest(output.get("replay_identity")) or not _digest(output.get("run_digest")) or a.get("content_hash")!=output.get("run_digest") or any(not _digest(v) for v in a.get("provenance_digests",[])):raise ResearchContractError("execution digest is inconsistent")
def model_computational_execution_contract(r:Mapping[str,Any])->dict[str,Any]:
    _validate_request(r); nodes=sorted((dict(n) for n in r["nodes"]),key=lambda n:n["node_id"]); order=[n["node_id"] for n in nodes];valid:set[str]=set();invalid:set[str]=set();omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set()
    if not _acyclic(nodes):invalid.update(order);omissions.add("workflow:dependency-cycle")
    for n in nodes:
        if not n.get("local_only") or not n.get("deterministic") or n.get("replay_identity")!=r["replay_identity"]:invalid.add(n["node_id"]);omissions.add(f"{n['node_id']}:local-determinism-or-replay")
        else:valid.add(n["node_id"])
        if n.get("required") and not n.get("deterministic"):negative.add("workflow:nondeterministic-required-node")
    for ok,label in ((r.get("policy_allowed"),"workflow:policy-denied"),(r.get("protected_closure"),"workflow:protected-closure-incomplete"),(r.get("raw_data_local"),"workflow:raw-data-not-local"),(r.get("aggregate_only"),"workflow:aggregate-only-required")):
        if not ok:omissions.add(label)
    global_block=not all(r.get(k) is True for k in ("policy_allowed","protected_closure","raw_data_local","aggregate_only"));disposition="blocked" if global_block or invalid else "partial" if not valid else "qualified"
    if global_block:invalid.update(order);valid.clear()
    payload={"node_order":order,"valid_order":sorted(valid),"invalid_order":sorted(invalid),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"replay_identity":r["replay_identity"]};digest=_hash(payload)
    out={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"consumer":r["consumer"],"purpose":r["purpose"],"scope":r["scope"],"disposition":disposition,**payload,"run_digest":digest,"artifact":{"artifact_id":f"onco-execution-run:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[] if disposition=="qualified" else ["execution-not-dispatched"],"provenance_digests":sorted({n["provenance_digest"] for n in nodes}),"boundary":PRECLINICAL_BOUNDARY},"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};validate_computational_execution_contract(out);return out
def model_computational_execution_contract_json(value:Mapping[str,Any])->dict[str,Any]:return model_computational_execution_contract(value)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","computational_execution_contract_manifest","model_computational_execution_contract","model_computational_execution_contract_json","validate_computational_execution_contract"]
