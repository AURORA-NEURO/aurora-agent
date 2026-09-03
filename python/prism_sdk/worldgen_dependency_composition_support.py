"""Deterministic Python parity for Worldgen P27 dependency composition."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.dependency-composition-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["workflow compiler","dependency steward","research operator","release auditor"],"behavior":f"compose typed capability dependencies with cycle and semantic-loss receipts at {scale} ({mode} scale)","value":"prevents undeclared, cyclic, or semantically lossy capability plans from executing","input_schema":"DependencyCompositionRequest4@1","output_schema":"DependencyCompositionCard7@1","effects":["emit:composition-plan","block:unsafe-release"],"permissions":["read:local-capability-graph"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate(o:Mapping[str,Any],*,feature_id:str|None=None)->None:
 a=o.get("artifact",{});bad=o.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and o.get("feature_id")!=feature_id or o.get("boundary")!=PRECLINICAL_BOUNDARY or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True or not o.get("node_order") or not _digest(o.get("replay_identity")) or not _digest(o.get("composition_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=o.get("composition_digest") or a.get("boundary")!=PRECLINICAL_BOUNDARY
 if bad:raise ResearchContractError("composition identity, locality, digest, artifact, or boundary is incomplete")
 for k in ("node_order","edge_order","root_order","resolved_order","blocked_order","omitted_order","cycle_order","uncertainty_order","semantic_loss_order","effect_receipts"):
  if not _ordered(o.get(k,[])):raise ResearchContractError("composition vectors are not canonical")
 if set(o["node_order"])!=set(o.get("resolved_order",[]))|set(o.get("blocked_order",[]))|set(o.get("omitted_order",[])):raise ResearchContractError("dependency states do not partition")
def compose(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
 if not isinstance(request.get("request_id"),str) or not request["request_id"].strip() or not isinstance(request.get("scope"),str) or not request["scope"].strip() or not request.get("nodes") or not request.get("required_root_order") or request.get("max_depth",0)<=0 or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_root_order"]) or not _ordered(request.get("adversarial_events",[])):raise ResearchContractError("composition identity, roots, depth, digest, ordering, locality, or boundary is invalid")
 nodes=sorted(request["nodes"],key=lambda n:n.get("node_id",""));ids=[];digests=set()
 for n in nodes:
  nid=n.get("node_id","")
  if nid in ids or not nid.strip() or not n.get("capability_id","").strip() or not n.get("version","").strip() or not n.get("consumer","").strip() or not _digest(n.get("input_digest")) or not _digest(n.get("output_digest")) or n.get("local") is not True or n.get("aggregate_only") is not True:raise ResearchContractError("node identity, consumer, digest, or locality is invalid")
  ids.append(nid);digests.add(n["output_digest"])
 edges=sorted(request.get("edges",[]),key=lambda e:(e.get("from",""),e.get("to","")));adj={i:set() for i in ids};indeg={i:0 for i in ids};edge_order=[];loss=set()
 for e in edges:
  if e.get("from") not in adj or e.get("to") not in adj or e["from"]==e["to"]:raise ResearchContractError("edge endpoint or self-cycle is invalid")
  edge_order.append(f"{e['from']}>{e['to']}");adj[e["from"]].add(e["to"]);indeg[e["to"]]+=1
  if e.get("semantic_loss"):loss.add(f"{e['from']}>{e['to']}:{e['semantic_loss']}")
 ready=sorted(i for i,d in indeg.items() if d==0);topo=[]
 while ready:
  n=ready.pop(0);topo.append(n)
  for child in sorted(adj[n]):indeg[child]-=1;ready.append(child) if indeg[child]==0 else None
  ready.sort()
 cycle=sorted(set(ids)-set(topo));resolved=set(topo);blocked=set(cycle);omitted=set();uncertainty=set()
 if cycle:uncertainty.add("graph:cycle-detected")
 global_block=not all(request.get(k) is True for k in ("policy_allowed","protected_closure","signed_approval","network_available","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events"))
 if global_block:omitted.update(ids);resolved.clear();blocked.clear()
 roots=set(request["required_root_order"]);disp="blocked" if global_block or cycle else "composed" if roots<=resolved and not loss else "unresolved";uncertainty.add("composition:release-gate-incomplete") if disp!="composed" else None
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request["request_id"],"disposition":disp,"node_order":ids,"edge_order":edge_order,"root_order":request["required_root_order"],"resolved_order":sorted(resolved),"blocked_order":sorted(blocked),"omitted_order":sorted(omitted),"cycle_order":cycle,"uncertainty_order":sorted(uncertainty),"semantic_loss_order":sorted(loss),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);payload["composition_digest"]=d;payload["artifact"]={"artifact_id":f"worldgen-dependency-composition:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(loss),"node_digests":sorted(digests),"boundary":PRECLINICAL_BOUNDARY};payload["effect_receipts"]=[f"emit:composition-plan:{request['request_id']}"] if disp=="composed" else ["block:unsafe-release"];validate(payload,feature_id=feature_id);return payload
DependencyNode4=dict[str,Any];DependencyEdge4=dict[str,Any];DependencyCompositionRequest4=dict[str,Any];DependencyCompositionCard7=dict[str,Any];DependencyCompositionError=ResearchContractError
__all__=["CONTENT_TYPE","DependencyNode4","DependencyEdge4","DependencyCompositionRequest4","DependencyCompositionCard7","DependencyCompositionError","manifest","compose","validate"]
