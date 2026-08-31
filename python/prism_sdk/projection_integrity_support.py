"""Python parity for Graph P32 projection-integrity cards."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
BOUNDARY="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";CONTENT_TYPE="application/vnd.aurora.graph.projection-integrity-card-1+json"
GraphNode4=dict[str,Any];GraphEdge4=dict[str,Any];ProjectionRequest4=dict[str,Any];ProjectionCard7=dict[str,Any];ProjectionArtifact4=dict[str,Any];ProjectionIntegrityError=ResearchContractError
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"graph","consumers":["knowledge compiler","mechanism explorer","analysis runtime","research workbench"],"behavior":f"qualify typed graph projections at {scale} ({mode})","value":"prevents orphaned, cyclically unsafe, semantically untyped, or unprovenanced graph projections from driving research workflows","input_schema":"ProjectionRequest4@1","output_schema":"ProjectionCard7@1","effects":["emit:projection-card","retain:lineage-loss","block:unsafe-projection"],"permissions":["read:local-graph-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or(feature_id is not None and o.get("feature_id")!=feature_id)or not o.get("request_id")or not o.get("purpose")or o.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not _digest(o.get("replay_identity")) or not _digest(o.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("closure_digest")
 if bad:raise ResearchContractError("projection identity, locality, artifact, digest, or boundary is incomplete")
 for k in("node_order","edge_order","selected_order","rejected_order","unknown_order","omitted_order","relation_order","kind_order","orphan_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("projection vectors are not canonical")
 ids=set(o["node_order"]);states=set(o["selected_order"])|set(o["rejected_order"])|set(o["unknown_order"])|set(o["omitted_order"])
 if len(o["node_order"])!=len(ids)or states!=ids:raise ResearchContractError("node states do not partition")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("nodes") or not request.get("required_node_order") or not _ordered(request["required_node_order"]) or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events",[])) or request.get("node_budget",0)<=0:raise ResearchContractError("projection identity, ordering, digest, locality, boundary, or budget is invalid")
 rows=sorted(request["nodes"],key=lambda a:a.get("node_id",""));seen=set();selected=set();rejected=set();unknown=set();omitted=set();kinds=set();orphans=set();evidence=set()
 for n in rows:
  nid=n.get("node_id","")
  if not nid.strip()or nid in seen or not n.get("kind","").strip()or not _digest(n.get("digest"))or not n.get("evidence_state","").strip()or n.get("local") is not True or n.get("aggregate_only") is not True:raise ResearchContractError("node identity, kind, digest, evidence, or locality is invalid")
  seen.add(nid);kinds.add(f"{nid}:{n['kind']}");evidence.add(n["digest"])
  if n["evidence_state"]=="unknown"or n["digest"]==request["replay_identity"]:unknown.add(nid)
  elif nid not in request["required_node_order"]:omitted.add(nid)
  else:selected.add(nid)
 edges=set();relations=set()
 for e in request.get("edges",[]):
  if not e.get("edge_id","").strip()or e.get("edge_id") in edges or not e.get("source","").strip()or not e.get("target","").strip()or e["source"]==e["target"]or e["source"] not in seen or e["target"] not in seen or not e.get("relation","").strip()or not _digest(e.get("digest")):raise ResearchContractError("edge identity, endpoint, self-loop, relation, or digest is invalid")
  edges.add(e["edge_id"]);relations.add(f"{e['edge_id']}:{e['relation']}")
 for nid in seen:
  if not any(e.get("source")==nid or e.get("target")==nid for e in request.get("edges",[])):orphans.add(nid)
 missing=[x for x in request["required_node_order"] if x not in seen];global_block=not all(request.get(k) is True for k in("policy_allowed","protected_closure","signed_manifest","raw_data_local","aggregate_only"))or bool(request.get("adversarial_events"))or len(rows)>request["node_budget"]
 if global_block:omitted.update(seen);selected.clear();rejected.clear();unknown.clear()
 disposition="blocked" if global_block else "unknown" if missing or unknown else "partial" if orphans or omitted else "qualified";payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"node_order":sorted(seen),"edge_order":sorted(edges),"selected_order":sorted(selected),"rejected_order":sorted(rejected),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"relation_order":sorted(relations),"kind_order":sorted(kinds),"orphan_order":sorted(orphans),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY};d=_hash(payload);payload["closure_digest"]=d;payload["effect_receipts"]=[f"approve:projection:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-projection"];payload["artifact"]={"artifact_id":f"graph-projection:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omitted_order"],"evidence_digests":sorted(evidence),"boundary":BOUNDARY};validate(payload,feature_id=feature_id);return payload
__all__=["BOUNDARY","CONTENT_TYPE","GraphNode4","GraphEdge4","ProjectionRequest4","ProjectionCard7","ProjectionArtifact4","ProjectionIntegrityError","manifest","qualify","validate"]
