"""Deterministic, omission-aware typed knowledge representation for Worldgen P04 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

CONTENT_TYPE="application/vnd.aurora.worldgen.knowledge-representation-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class KnowledgeNode:
    node_id:str; semantic_type:str; label:str; confidence_milli:int; state:str; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; negative_result:bool=False; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class KnowledgeRelation:
    relation_id:str; subject_id:str; predicate:str; object_id:str; evidence_digest:str; provenance_digest:str; replay_identity:str; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class KnowledgeRepresentationRequest:
    request_id:str; namespace:str; required_node_order:tuple[str,...]; required_relation_order:tuple[str,...]; minimum_confidence_milli:int; nodes:tuple[KnowledgeNode,...]; relations:tuple[KnowledgeRelation,...]; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class KnowledgeRepresentationReceipt:
    value:dict[str,Any]
    def validate(self,*,feature_id:str,contract_version:str)->None:
        v,a=self.value,self.value.get("artifact",{}); req=set(v.get("required_node_order",())); np=set(v.get("resolved_node_order",()))|set(v.get("unknown_node_order",()))|set(v.get("blocked_node_order",()))|set(v.get("omitted_node_order",())); rel=set(v.get("relation_order",())); rp=set(v.get("resolved_relation_order",()))|set(v.get("omitted_relation_order",()))
        if not (v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version")==contract_version and v.get("feature_id")==feature_id and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and a.get("raw_nodes") is False and v.get("raw_data_local") is True and v.get("aggregate_only") is True and req and np==req and rel and rp==rel and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","graph_digest")) and a.get("content_hash")==v.get("graph_digest")): raise ResearchContractError("knowledge graph identity, partitions, locality, digests, or effects are incomplete")
        for key in ("required_node_order","resolved_node_order","unknown_node_order","blocked_node_order","omitted_node_order","relation_order","resolved_relation_order","omitted_relation_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            values=tuple(v.get(key,()));
            if values!=tuple(sorted(set(values))): raise ResearchContractError("knowledge representation ordering is not canonical")
        if any(e!="block:unsafe-release" and not e.startswith("represent:worldgen-knowledge:") for e in v["effect_receipts"]): raise ResearchContractError("knowledge representation effect is outside typed graph gate")
    def digest(self,*,feature_id:str,contract_version:str)->str:self.validate(feature_id=feature_id,contract_version=contract_version);return _digest(self.value)
def _digest(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _sorted(values):return sorted(set(values))
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["knowledge engineer","preclinical neuroscientist","research program lead","downstream graph consumer"],"behavior":f"compile a typed omission-aware knowledge graph for {scale}","value":"turns evidence into replayable typed nodes and relations without inventing unsupported facts","input_schema":input_schema,"output_schema":"KnowledgeGraphReceipt1@1","effects":["represent:worldgen-knowledge","block:unsafe-release"],"permissions":["represent:local-research-knowledge"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def represent(request:KnowledgeRepresentationRequest,*,feature_id:str,contract_version:str,scale:str,require_federation:bool)->KnowledgeRepresentationReceipt:
    if not request.request_id.strip() or not request.namespace.strip() or request.boundary!=PRECLINICAL_BOUNDARY or not request.raw_data_local or not request.aggregate_only or not request.required_node_order or tuple(request.required_node_order)!=tuple(sorted(set(request.required_node_order))) or not request.required_relation_order or tuple(request.required_relation_order)!=tuple(sorted(set(request.required_relation_order))) or not _HEX.fullmatch(request.replay_identity): raise ResearchContractError("knowledge graph identity, required nodes, relations, locality, boundary, ordering, or replay is invalid")
    required=set(request.required_node_order); relations_required=set(request.required_relation_order); nodes={node.node_id:node for node in request.nodes}; relations={relation.relation_id:relation for relation in request.relations}
    if len(nodes)!=len(request.nodes) or len(relations)!=len(request.relations) or any(node_id not in required or node.boundary!=PRECLINICAL_BOUNDARY or not node.raw_data_local or node.replay_identity!=request.replay_identity or not all(_HEX.fullmatch(getattr(node,key)) for key in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")) for node_id,node in nodes.items()) or any(relation_id not in relations_required or relation.subject_id not in required or relation.object_id not in required or relation.boundary!=PRECLINICAL_BOUNDARY or not relation.raw_data_local or relation.replay_identity!=request.replay_identity or not all(_HEX.fullmatch(getattr(relation,key)) for key in ("evidence_digest","provenance_digest","replay_identity")) for relation_id,relation in relations.items()): raise ResearchContractError("knowledge node or relation identity, provenance, replay, locality, or boundary is invalid")
    resolved=set(); unknown=set(); blocked=set(); omitted=set(); omissions=set(); uncertainty=set(); negative=set()
    for node_id in required:
        node=nodes.get(node_id)
        if node is None: omitted.add(node_id); omissions.add(f"node:{node_id}:missing")
        elif node.negative_result: blocked.add(node_id); negative.add(f"node:{node_id}:negative-result-retained")
        elif not request.policy_allow or not request.protected_closure or not node.raw_data_local: blocked.add(node_id); omissions.add(f"node:{node_id}:policy-or-locality-blocked")
        elif node.state!="supported" or node.confidence_milli<request.minimum_confidence_milli: unknown.add(node_id); uncertainty.add(f"node:{node_id}:unsupported-or-below-threshold")
        else: resolved.add(node_id)
    resolved_rel=set()
    for relation_id,relation in relations.items():
        if relation_id in relations_required and relation.subject_id in resolved and relation.object_id in resolved: resolved_rel.add(relation_id)
        elif relation_id in relations_required: omissions.add(f"relation:{relation_id}:endpoint-unresolved")
    for relation_id in relations_required-set(relations): omissions.add(f"relation:{relation_id}:missing")
    if require_federation and not request.federation_approved: omissions.add("request:federation-approval-missing")
    authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not resolved else "qualified" if len(resolved)==len(required) and not omissions and not uncertainty and not negative else "partial"; omitted_rel=relations_required-resolved_rel; effects=["block:unsafe-release"] if disposition=="blocked" else [f"represent:worldgen-knowledge:{request.request_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"namespace":request.namespace,"scale":scale,"disposition":disposition,"required_node_order":_sorted(required),"resolved_node_order":_sorted(resolved),"unknown_node_order":_sorted(unknown),"blocked_node_order":_sorted(blocked),"omitted_node_order":_sorted(omitted),"relation_order":_sorted(relations_required),"resolved_relation_order":_sorted(resolved_rel),"omitted_relation_order":_sorted(omitted_rel),"replay_identity":request.replay_identity,"omissions":_sorted(omissions),"uncertainty":_sorted(uncertainty),"negative_evidence":_sorted(negative),"effect_receipts":effects,"raw_nodes":False,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; d=_digest(payload); payload["graph_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-knowledge-graph:{request.request_id}","content_type":CONTENT_TYPE,"content_hash":d,"raw_nodes":False,"boundary":PRECLINICAL_BOUNDARY}; receipt=KnowledgeRepresentationReceipt(payload); receipt.validate(feature_id=feature_id,contract_version=contract_version); return receipt
__all__=["CONTENT_TYPE","KnowledgeNode","KnowledgeRelation","KnowledgeRepresentationRequest","KnowledgeRepresentationReceipt","manifest","represent"]
