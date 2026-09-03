"""Python parity for AFA-runtime-P04-F28."""
from __future__ import annotations
from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-runtime-P04-F28"
CONTRACT_VERSION = "runtime-federated-continual-knowledge-representation-assurance/1.0"
INPUT_SCHEMA = "ScopedResearchClaims4@1"
OUTPUT_SCHEMA = "TypedKnowledgeWorld7@1"
CONTENT_TYPE = "application/vnd.aurora.typed-knowledge-world-7+json"

def _hash(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(values: Sequence[str]) -> bool: return list(values) == sorted(set(values))

@dataclass(frozen=True)
class TypedKnowledgeWorld7:
    value: Mapping[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self) -> None:
        v=self.value
        if (v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("request_id") or not v.get("world_id") or not v.get("federation_id") or not v.get("requester") or not v.get("purpose") or not v.get("semantic_profile") or int(v.get("checkpoint",0))<=0 or not v.get("claim_order") or not v.get("source_order") or not v.get("peer_order") or v.get("effect_receipts")!=["block:unsafe-release"]): raise ResearchContractError("knowledge identity, closure, locality, or release gate is incomplete")
        keys=("claim_order","selected_claim_order","unresolved_claim_order","blocked_claim_order","missing_claim_order","source_order","selected_source_order","missing_source_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,())) for k in keys): raise ResearchContractError("knowledge ordering is not canonical")
        ids=set(v["claim_order"]); parts=list(v.get("selected_claim_order",()))+list(v.get("unresolved_claim_order",()))+list(v.get("blocked_claim_order",()))+list(v.get("missing_claim_order",()))
        if set(parts)!=ids or len(parts)!=len(ids): raise ResearchContractError("claim outcomes do not partition")
        sources=set(v["source_order"]); source_parts=list(v.get("selected_source_order",()))+list(v.get("missing_source_order",()))
        if set(source_parts)!=sources or len(source_parts)!=len(sources): raise ResearchContractError("source outcomes do not partition")
        peers=set(v["peer_order"]); peer_parts=list(v.get("qualified_peer_order",()))+list(v.get("missing_peer_order",()))
        if set(peer_parts)!=peers or len(peer_parts)!=len(peers): raise ResearchContractError("peer outcomes do not partition")
        if len(v.get("confidence_scores_milli",()))!=len(v.get("selected_claim_order",())) or not all(_digest(x) for x in (v.get("replay_identity"),v.get("world_digest"),v.get("artifact",{}).get("content_hash"))): raise ResearchContractError("knowledge artifact digest is invalid")
        if v.get("artifact",{}).get("content_type")!=CONTENT_TYPE or v["artifact"].get("boundary")!=PRECLINICAL_BOUNDARY or v["artifact"].get("content_hash")!=v.get("world_digest"): raise ResearchContractError("knowledge artifact metadata is invalid")
    def digest(self) -> str: self.validate(); return _hash(self.value)

def assure_knowledge_representation(*, request: Mapping[str, Any]) -> TypedKnowledgeWorld7:
    if request.get("schema_version")!=INPUT_SCHEMA or any(not str(request.get(k,"")).strip() for k in ("request_id","world_id","federation_id","requester","purpose","semantic_profile")) or int(request.get("checkpoint",0))<=0 or not all(_ordered([str(x) for x in request.get(k,())]) and request.get(k) for k in ("required_claim_order","required_source_order","required_peer_order")) or not _digest(request.get("replay_identity")) or int(request.get("budget_units",0))<=0 or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY or not isinstance(request.get("claims"),Sequence) or not request.get("claims") or not isinstance(request.get("peers"),Sequence) or not request.get("peers"): raise ResearchContractError("knowledge identity, closure, replay, budget, locality, or boundary is invalid")
    claims=sorted(request["claims"],key=lambda x:(-int(x.get("confidence_milli",0)),str(x.get("claim_id","")))); ids=[str(x.get("claim_id","")) for x in claims]
    if not all(ids) or len(set(ids))!=len(ids): raise ResearchContractError("claim identities must be unique and non-empty")
    required=set(map(str,request["required_claim_order"])); q=set(); u=set(); b=set(); missing=set(); sources=set(); selected_sources=set(); om=set(); un=set(); neg=set(); prov=set(); scores=[]
    for c in claims:
        cid=str(c["claim_id"]); sid=str(c.get("source_id","")); sources.add(sid); prov.add(str(c.get("provenance_digest",""))); om.update(f"{cid}:{x}" for x in c.get("omission_order",())); un.update(f"{cid}:{x}" for x in c.get("uncertainty_order",()));
        if c.get("negative_result") is True or str(c.get("evidence_state")) in {"negative","contradicted"}: neg.add(f"{cid}:negative-result")
        if not sid or cid not in required: missing.add(cid) if cid not in required else u.add(cid)
        elif not all(c.get(k) is True for k in ("signed","permitted","raw_data_local","aggregate_only")) or str(c.get("semantic_profile"))!=str(request["semantic_profile"]) or str(c.get("replay_identity"))!=str(request["replay_identity"]): b.add(cid)
        elif str(c.get("evidence_state")) in {"proven","supported"} and int(c.get("confidence_milli",0))>=600: q.add(cid); selected_sources.add(sid); scores.append(int(c.get("confidence_milli",0)))
        else: u.add(cid)
    peers=sorted(request["peers"],key=lambda x:str(x.get("peer_id",""))); peer_ids=[str(x.get("peer_id","")) for x in peers]
    if not all(peer_ids) or len(set(peer_ids))!=len(peer_ids): raise ResearchContractError("peer identities must be unique and non-empty")
    qp=set(); mp=set()
    for p in peers:
        pid=str(p["peer_id"]); ok=str(p.get("semantic_profile"))==str(request["semantic_profile"]) and int(p.get("checkpoint",0))==int(request["checkpoint"]) and p.get("signed") is True and p.get("permitted") is True and p.get("raw_data_local") is True and p.get("aggregate_only") is True and str(p.get("replay_identity"))==str(request["replay_identity"]) and str(p.get("evidence_state")) in {"proven","supported"}
        (qp if ok else mp).add(pid); om.update(f"{pid}:{x}" for x in p.get("omission_order",())); un.update(f"{pid}:{x}" for x in p.get("uncertainty_order",()));
        if p.get("negative_result") is True or str(p.get("evidence_state")) in {"negative","contradicted"}: neg.add(f"{pid}:negative-result")
    global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","federation_allow","signed_approval","raw_data_local","aggregate_only")) or bool(request.get("adversarial_event_order"));
    if global_block: b.update(ids); q.clear(); u.clear(); missing.clear(); om.add("request:governance-or-adversarial-blocked")
    un.update(f"adversarial:{x}" for x in request.get("adversarial_event_order",()));
    for rid in sorted(required-set(ids)): missing.add(rid); om.add(f"required:{rid}")
    if not required.issubset(q): om.add("request:required-knowledge-closure-not-met")
    if not required.issubset(qp): om.add("request:peer-quorum-not-met")
    disposition="blocked" if global_block else "unresolved" if u or b or missing or not required.issubset(q) or not required.issubset(qp) else "qualified"; om.add("request:knowledge-closure-not-ready") if disposition!="qualified" else None
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":str(request["request_id"]),"world_id":str(request["world_id"]),"federation_id":str(request["federation_id"]),"requester":str(request["requester"]),"purpose":str(request["purpose"]),"semantic_profile":str(request["semantic_profile"]),"checkpoint":int(request["checkpoint"]),"disposition":disposition,"claim_order":ids,"selected_claim_order":sorted(q),"unresolved_claim_order":sorted(u),"blocked_claim_order":sorted(b),"missing_claim_order":sorted(missing),"source_order":sorted(sources),"selected_source_order":sorted(selected_sources),"missing_source_order":sorted(sources-selected_sources),"peer_order":peer_ids,"qualified_peer_order":sorted(qp),"missing_peer_order":sorted(mp),"omission_order":sorted(om),"uncertainty_order":sorted(un),"negative_evidence_order":sorted(neg),"confidence_scores_milli":scores,"replay_identity":str(request["replay_identity"]),"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; d=_hash(payload); payload["world_digest"]=d; payload["artifact"]={"artifact_id":f"typed-knowledge-world-7:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":payload["omission_order"],"provenance_digests":sorted(prov),"boundary":PRECLINICAL_BOUNDARY}; payload["effect_receipts"]=["block:unsafe-release"]; receipt=TypedKnowledgeWorld7(payload); receipt.validate(); return receipt

def knowledge_representation_assurance_manifest() -> dict[str, Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"obligation","consumers":["computational biologist","federation verifier"],"input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"autonomy_tier":"A1","effects":["block:unsafe-release"],"permissions":["evaluate:capability-runs"],"boundary":PRECLINICAL_BOUNDARY}

__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","TypedKnowledgeWorld7","assure_knowledge_representation","knowledge_representation_assurance_manifest"]
