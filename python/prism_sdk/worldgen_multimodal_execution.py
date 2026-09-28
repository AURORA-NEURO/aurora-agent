"""Python parity for ``AFA-worldgen-P12-F26`` execution assurance."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-worldgen-P12-F26"; CONTRACT_VERSION="worldgen-multimodal-computational-execution-assurance/1.0"; INPUT_SCHEMA="ResearchWorkflowSpec2@1"; OUTPUT_SCHEMA="ExecutionRun7@1"; CONTENT_TYPE="application/vnd.aurora.worldgen-execution-run-7+json"; MAX_NODES=4096
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return values==sorted(set(values))

@dataclass(frozen=True)
class WorldgenExecutionRun7:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k),str) and v[k].strip() for k in ("request_id","workflow_id","purpose","semantic_profile")) or not v.get("required_study_order") or not v.get("required_modality_order") or not v.get("node_order") or not v.get("effect_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("execution identity, requirements, nodes, effects, locality, or disposition is incomplete")
        keys=("required_study_order","required_modality_order","node_order","selected_node_order","unresolved_node_order","blocked_node_order","cycle_order","missing_dependency_order","missing_study_order","missing_modality_order","budget_exceeded_order","evidence_order","omission_order","uncertainty_order","negative_evidence_order","effect_order","effect_receipts")
        for k in keys:
            if not _ordered(v.get(k,[])):raise ResearchContractError("execution ordering is not canonical")
        ids=set(v["node_order"]);parts=v["selected_node_order"]+v["unresolved_node_order"]+v["blocked_node_order"]
        if len(ids)!=len(v["node_order"]) or len(parts)!=len(ids) or set(parts)!=ids or len(set(v["planned_node_order"]))!=len(v["planned_node_order"]) or any(i not in ids for i in v["planned_node_order"]):raise ResearchContractError("execution states or plan do not partition")
        if not all(_digest(v.get(k)) for k in ("checkpoint_digest","replay_identity","execution_digest")) or a.get("content_hash")!=v.get("execution_digest") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(d) for d in a.get("provenance_digests",[])):raise ResearchContractError("execution digest or artifact metadata is invalid")
        if any(not e.startswith(("exchange:execution-run-digests:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("effect is outside governed execution gate")
    def digest(self)->str:self.validate();return _hash(self.to_dict())

def multimodal_execution_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"worldgen","consumers":["research program lead","computational execution operator","benchmark curator"],"behavior":"verify multimodal multi-study execution graphs with deterministic dependency, evidence, budget, replay, provenance, policy, federation, and locality gates before executor dispatch","value":"prevents incomparable, under-evidenced, over-budget, or unauthorized research runs from entering computation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:execution-run-digests","manage:local-capability","block:unsafe-release"],"permissions":["evaluate:capability-runs"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","workflow_id","purpose","semantic_profile")) or not r.get("required_study_order") or not r.get("required_modality_order") or not r.get("nodes") or len(r["nodes"])>MAX_NODES or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("execution identity, requirements, node bound, replay, locality, or boundary is invalid")
    for k in ("required_study_order","required_modality_order"):
        if len(set(r[k]))!=len(r[k]) or any(not isinstance(x,str) or not x.strip() for x in r[k]):raise ResearchContractError("required studies and modalities must be unique and non-empty")
    ids:set[str]=set()
    for n in r["nodes"]:
        if not isinstance(n,Mapping) or not isinstance(n.get("node_id"),str) or not n["node_id"].strip() or n["node_id"] in ids or not isinstance(n.get("study_id"),str) or not n["study_id"].strip() or not isinstance(n.get("modality"),str) or not n["modality"].strip() or not _digest(n.get("artifact_digest")) or not _digest(n.get("provenance_digest")) or not n.get("local") is True or not n.get("aggregate_only") is True or not isinstance(n.get("depends_on"),list) or len(set(n["depends_on"]))!=len(n["depends_on"]) or n["node_id"] in n["depends_on"]:raise ResearchContractError(f"node {n.get('node_id','')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(n["node_id"])

def assure_worldgen_multimodal_execution(r:Mapping[str,Any])->WorldgenExecutionRun7:
    _validate_request(r); nodes={n["node_id"]:dict(n) for n in r["nodes"]}; node_order=sorted(nodes); indegree={i:0 for i in node_order}; dependents={}; missing:set[str]=set(); missing_nodes:set[str]=set()
    for n in nodes.values():
        for dep in n["depends_on"]:
            if dep not in nodes: missing.add(f"{n['node_id']}:{dep}");missing_nodes.add(n["node_id"])
            else: indegree[n["node_id"]]+=1;dependents.setdefault(dep,set()).add(n["node_id"])
    ready=sorted(i for i,d in indegree.items() if d==0); planned=[]
    while ready:
        i=ready.pop(0);planned.append(i)
        for child in sorted(dependents.get(i,set())):
            indegree[child]-=1
            if indegree[child]==0:ready.append(child);ready.sort()
    planned_set=set(planned);cycle=sorted(set(node_order)-planned_set);studies={n["study_id"] for n in nodes.values()};modalities={n["modality"] for n in nodes.values()};provenance={n["provenance_digest"] for n in nodes.values()}
    missing_study=sorted(set(r["required_study_order"])-studies);missing_modality=sorted(set(r["required_modality_order"])-modalities);selected:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();evidence:set[str]=set();omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set()
    for i in planned:
        n=nodes[i]
        if i in missing_nodes:unresolved.add(i);uncertainty.add(f"{i}:missing-dependency")
        elif any(d in blocked for d in n["depends_on"]):blocked.add(i);negative.add(f"{i}:blocked-dependency")
        elif any(d in unresolved for d in n["depends_on"]):unresolved.add(i);uncertainty.add(f"{i}:unresolved-dependency")
        elif n["evidence_state"]=="contradicted":blocked.add(i);negative.add(f"{i}:contradicted")
        elif n["evidence_state"] in {"unknown","unmeasured"}:unresolved.add(i);evidence.add(i);uncertainty.add(f"{i}:evidence-state")
        else:selected.add(i)
    for i in cycle:blocked.add(i);negative.add(f"{i}:cycle")
    for item in missing:uncertainty.add(f"{item}:missing-dependency")
    for study in missing_study:omissions.add(f"study:{study}:missing");negative.add(f"study:{study}:no-node")
    for modality in missing_modality:omissions.add(f"modality:{modality}:missing");negative.add(f"modality:{modality}:no-node")
    planned_units=sum(nodes[i]["estimated_units"] for i in selected);budget_exceeded=sorted(selected) if planned_units>r["budget_units"] else []
    for i in budget_exceeded:selected.remove(i);unresolved.add(i);uncertainty.add(f"{i}:budget");negative.add(f"{i}:budget-exceeded")
    if budget_exceeded:omissions.add("request:budget-exceeded")
    consumed_units=sum(nodes[i]["estimated_units"] for i in selected);global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"))
    if global_block:blocked.update(node_order);selected.clear();unresolved.clear();omissions.add("request:governance-or-locality-denied")
    so=sorted(selected);uo=sorted(unresolved);bo=sorted(blocked);disp="blocked" if global_block or bo else "unresolved" if uo or missing_study or missing_modality else "qualified"
    if disp!="qualified":omissions.add("request:multimodal-execution-not-closed")
    effects=["exchange:execution-run-digests","manage:local-capability"] if disp=="qualified" else ["block:unsafe-release"]
    checkpoint=_hash({"workflow_id":r["workflow_id"],"planned_node_order":planned,"replay_identity":r["replay_identity"],"budget_units":r["budget_units"]})
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"workflow_id":r["workflow_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"disposition":disp,"required_study_order":r["required_study_order"],"required_modality_order":r["required_modality_order"],"node_order":node_order,"planned_node_order":planned,"selected_node_order":so,"unresolved_node_order":uo,"blocked_node_order":bo,"cycle_order":cycle,"missing_dependency_order":sorted(missing),"missing_study_order":missing_study,"missing_modality_order":missing_modality,"budget_exceeded_order":budget_exceeded,"evidence_order":sorted(evidence),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"consumed_units":consumed_units,"budget_units":r["budget_units"],"checkpoint_digest":checkpoint,"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    digest=_hash(payload);value={**payload,"execution_digest":digest,"artifact":{"artifact_id":f"worldgen-execution-run-7:{r['workflow_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":payload["omission_order"],"provenance_digests":sorted(provenance),"boundary":PRECLINICAL_BOUNDARY},"effect_order":effects,"effect_receipts":sorted(e if e=="block:unsafe-release" else f"{e}:{r['request_id']}" for e in effects)}
    receipt=WorldgenExecutionRun7(value);receipt.validate();return receipt
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","WorldgenExecutionRun7","multimodal_execution_assurance_manifest","assure_worldgen_multimodal_execution"]
