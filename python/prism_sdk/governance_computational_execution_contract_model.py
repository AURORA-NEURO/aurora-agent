"""Python parity for governance computational-execution contract model ``AFA-governance-P12-F08``."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEATURE_ID="AFA-governance-P12-F08"; CONTRACT_VERSION="governance-federated-continual-computational-execution-contract-model/1.0"; INPUT_SCHEMA="ExecutionContractRequest5@1"; OUTPUT_SCHEMA="GovernanceExecutionContract8@1"; CONTENT_TYPE="application/vnd.aurora.governance-execution-contract-8+json"
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))

@dataclass(frozen=True)
class GovernanceExecutionContract8:
 value:Mapping[str,Any]
 def to_dict(self)->dict[str,Any]:return dict(self.value)
 def validate(self)->None:
  v=self.value; a=v.get("artifact",{})
  if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k,"")).strip() for k in ("request_id","federation_id","workflow_id","requester","purpose","semantic_profile","engine_version")) or int(v.get("checkpoint",0))<=0 or not v.get("node_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("execution identity, graph, peers, locality, or effects are incomplete")
  for k in ("node_order","planned_node_order","unresolved_node_order","blocked_node_order","cycle_order","missing_dependency_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts"):
   if not _ordered(v.get(k,[])):raise ResearchContractError("execution ordering is not canonical")
  nodes=set(v["node_order"]); parts=[*v["planned_node_order"],*v["unresolved_node_order"],*v["blocked_node_order"]]; peers=set(v["peer_order"]); pp=[*v["qualified_peer_order"],*v["missing_peer_order"]]
  if len(parts)!=len(nodes) or set(parts)!=nodes or len(set(parts))!=len(parts) or len(pp)!=len(peers) or set(pp)!=peers or len(set(pp))!=len(pp):raise ResearchContractError("node or peer outcomes do not partition")
  if not all(_digest(x) for x in (v.get("replay_identity"),v.get("contract_digest"),a.get("content_hash"),*a.get("provenance_digests",[]))) or a.get("content_hash")!=v.get("contract_digest"):raise ResearchContractError("execution contract digest is invalid")
  if v["disposition"]=="qualified" and v["effect_receipts"]!= ["exchange:aggregate-contract","retain:execution-contract"]:raise ResearchContractError("qualified execution effects are invalid")
  if v["disposition"]!="qualified" and v["effect_receipts"]!= ["block:unsafe-release"]:raise ResearchContractError("non-qualified execution must block release")

def computational_execution_contract_model_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"governance","consumers":["computational biologist","workflow compiler engineer","governance steward"],"behavior":"validate a federated computational workflow contract and emit a deterministic dry-run execution artifact","value":"makes graph closure, evidence, budget, replay, peer, policy, and locality conditions auditable before dispatch","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:aggregate-contract","retain:execution-contract","block:unsafe-release"],"permissions":["read:local-workflow-manifests","evaluate:capability-runs"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def model_computational_execution_contract(request:Mapping[str,Any])->GovernanceExecutionContract8:
 if request.get("schema_version")!=INPUT_SCHEMA or not all(str(request.get(k,"")).strip() for k in ("request_id","federation_id","workflow_id","requester","purpose","semantic_profile","engine_version")) or not request.get("nodes") or not request.get("peers") or int(request.get("checkpoint",0))<=0 or int(request.get("max_budget_units",0))<=0 or int(request.get("minimum_peer_quorum",0))<=0 or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("execution request identity, bounds, peers, replay, locality, or boundary is invalid")
 nodes=sorted((dict(n) for n in request["nodes"]),key=lambda n:str(n.get("node_id",""))); ids=[str(n.get("node_id","")) for n in nodes]; peers=sorted((dict(p) for p in request["peers"]),key=lambda p:str(p.get("peer_id",""))); pids=[str(p.get("peer_id","")) for p in peers]
 if len(set(ids))!=len(ids) or len(set(pids))!=len(pids) or any(not n.get("node_id") or int(n.get("estimated_units",0))<=0 or not all(_digest(n.get(k)) for k in ("artifact_digest","provenance_digest","replay_identity")) for n in nodes):raise ResearchContractError("node identity, budget, or digests are invalid")
 by_id={n["node_id"]:n for n in nodes}; remaining={n["node_id"]:len([d for d in n.get("dependency_order",[]) if d in by_id]) for n in nodes}; ready=sorted(k for k,v in remaining.items() if v==0); plan=[]
 while ready:
  nid=ready.pop(0);plan.append(nid)
  for n in nodes:
   if nid in n.get("dependency_order",[]) and n["node_id"] in remaining:remaining[n["node_id"]]-=1; ready.append(n["node_id"]) if remaining[n["node_id"]]==0 else None
  ready.sort()
 cycles={k for k,v in remaining.items() if v}; missing={f"{n['node_id']}:missing:{d}" for n in nodes for d in n.get("dependency_order",[]) if d not in by_id}; planned:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set(); total=0
 for n in nodes:
  nid=n["node_id"];total+=int(n["estimated_units"]); negative.add(f"{nid}:negative-result") if n.get("negative_result") else None; hard=nid in cycles or str(n.get("replay_identity"))!=str(request["replay_identity"]) or n.get("local_only") is not True or n.get("permitted") is not True or n.get("signed") is not True or n.get("deterministic") is not True or n.get("evidence_state") in {"contradicted","negative"}
  if hard: blocked.add(nid) if nid in cycles or n.get("local_only") is not True or n.get("evidence_state") in {"contradicted","negative"} else unresolved.add(nid)
  elif n.get("evidence_state") not in {"proven","supported"}:unresolved.add(nid);uncertainty.add(f"{nid}:evidence-state")
  elif any(d not in by_id or d in cycles for d in n.get("dependency_order",[])):blocked.add(nid)
  else:planned.add(nid)
 if missing:omissions.add("request:missing-dependency-closure")
 if total>int(request["max_budget_units"]):omissions.add(f"request:budget-exceeded:{total}")
 qp={p["peer_id"] for p in peers if p.get("workflow_id")==request["workflow_id"] and p.get("semantic_profile")==request["semantic_profile"] and int(p.get("checkpoint",0))>=int(request["checkpoint"]) and str(p.get("replay_identity"))==str(request["replay_identity"]) and p.get("signed") is True and p.get("aggregate_only") is True and p.get("raw_data_local") is True and p.get("evidence_state") in {"proven","supported"}};mp=set(pids)-qp
 if len(qp)<int(request["minimum_peer_quorum"]):uncertainty.add("peer:minimum-quorum-unmet")
 uncertainty.update(f"adversarial:{e}" for e in request.get("adversarial_event_order",[])); global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","federation_allowed","raw_data_local","aggregate_only")) or bool(request.get("adversarial_event_order"))
 if global_block:blocked.update(ids);planned.clear();unresolved.clear();omissions.add("request:governance-or-adversarial-blocked")
 disposition="blocked" if global_block else "unresolved" if blocked or unresolved or missing or total>int(request["max_budget_units"]) or len(qp)<int(request["minimum_peer_quorum"]) or not planned else "qualified"; omissions.add("request:execution-contract-not-release-ready") if disposition!="qualified" else None
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"federation_id":request["federation_id"],"workflow_id":request["workflow_id"],"requester":request["requester"],"purpose":request["purpose"],"semantic_profile":request["semantic_profile"],"engine_version":request["engine_version"],"checkpoint":int(request["checkpoint"]),"disposition":disposition,"node_order":ids,"planned_node_order":sorted(planned),"unresolved_node_order":sorted(unresolved),"blocked_node_order":sorted(blocked),"cycle_order":sorted(cycles),"missing_dependency_order":sorted(missing),"peer_order":pids,"qualified_peer_order":sorted(qp),"missing_peer_order":sorted(mp),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"total_units":total,"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=research_artifact_digest(payload); payload["contract_digest"]=digest; payload["artifact"]={"artifact_id":f"governance-execution-contract-8:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":sorted(omissions),"provenance_digests":sorted({n["provenance_digest"] for n in nodes}),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=["exchange:aggregate-contract","retain:execution-contract"] if disposition=="qualified" else ["block:unsafe-release"]; out=GovernanceExecutionContract8(payload);out.validate();return out

__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","GovernanceExecutionContract8","computational_execution_contract_model_manifest","model_computational_execution_contract"]
