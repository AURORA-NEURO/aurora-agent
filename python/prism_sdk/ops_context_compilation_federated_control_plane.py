"""Prospective high-throughput context compilation control plane (``AFA-ops-P03-F31``)."""
from __future__ import annotations
from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ops-P03-F31"; CONTRACT_VERSION="ops-prospective-context-compilation-federated-control-plane/1.0"; INPUT_SCHEMA="DecisionQuery3@1"; OUTPUT_SCHEMA="CertifiedDecisionSection8@1"; CONTENT_TYPE="application/vnd.aurora.ops-certified-decision-section-8+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))

@dataclass(frozen=True)
class CertifiedDecisionSection:
    schema_version:str; contract_version:str; feature_id:str; request_id:str; federation_id:str; query_id:str; purpose:str; semantic_profile:str; disposition:str
    context_order:tuple[str,...]; selected_context_order:tuple[str,...]; unresolved_context_order:tuple[str,...]; blocked_context_order:tuple[str,...]; missing_context_order:tuple[str,...]; peer_order:tuple[str,...]; qualified_peer_order:tuple[str,...]; missing_peer_order:tuple[str,...]
    queue_depth:int; active_runs:int; capacity_exceeded_order:tuple[str,...]; omission_order:tuple[str,...]; uncertainty_order:tuple[str,...]; negative_evidence_order:tuple[str,...]; replay_identity:str; section_digest:str; artifact:Mapping[str,Any]; effect_receipts:tuple[str,...]; raw_data_local:bool; boundary:str
    def to_dict(self)->dict[str,Any]:
        v=asdict(self)
        for k,x in v.items():
            if isinstance(x,tuple):v[k]=list(x)
        return v
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.raw_data_local is not True or not self.request_id.strip() or not self.federation_id.strip() or not self.query_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.context_order or not self.peer_order or not self.effect_receipts: raise ResearchContractError("control identity, locality, contexts, peers, or effects are incomplete")
        for values in (self.context_order,self.selected_context_order,self.unresolved_context_order,self.blocked_context_order,self.missing_context_order,self.peer_order,self.qualified_peer_order,self.missing_peer_order,self.capacity_exceeded_order,self.omission_order,self.uncertainty_order,self.negative_evidence_order,self.effect_receipts):
            if not _ordered(list(values)):raise ResearchContractError("control ordering is not canonical")
        ids=set(self.context_order); parts=list(self.selected_context_order)+list(self.unresolved_context_order)+list(self.blocked_context_order)
        if set(parts)!=ids or len(parts)!=len(ids):raise ResearchContractError("context states do not partition")
        peers=set(self.peer_order); peer_parts=list(self.qualified_peer_order)+list(self.missing_peer_order)
        if set(peer_parts)!=peers or len(peer_parts)!=len(peers):raise ResearchContractError("peer states do not partition")
        if not all(_digest(v) for v in (self.replay_identity,self.section_digest,self.artifact.get("content_hash"))):raise ResearchContractError("control digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("control artifact metadata is invalid")
        expected=[f"exchange:permitted-summaries:{self.request_id}",f"manage:local-capability:{self.request_id}"] if self.disposition=="qualified" else ["block:unsafe-release"]
        if list(self.effect_receipts)!=expected:raise ResearchContractError("control effect receipts are invalid")

def operate_context_compilation(*,request:Mapping[str,Any])->CertifiedDecisionSection:
    if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or any(not str(request.get(k,"")).strip() for k in ("request_id","federation_id","query_id","purpose","semantic_profile")) or not request.get("required_context_order") or not _ordered([str(v) for v in request["required_context_order"]]) or not request.get("contexts") or not request.get("peers") or int(request.get("minimum_peer_quorum",0))<=0 or int(request.get("capacity",0))<=0 or int(request.get("max_queue_depth",0))<=0 or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY or not _ordered([str(v) for v in request.get("adversarial_events",[])]):raise ResearchContractError("control identity, closure, capacity, replay, locality, or boundary is invalid")
    contexts=sorted(request["contexts"],key=lambda x:str(x.get("context_id",""))); context_order=[str(x.get("context_id","")) for x in contexts]
    if not all(context_order) or len(set(context_order))!=len(context_order):raise ResearchContractError("context identities must be unique and non-empty")
    required={str(v) for v in request["required_context_order"]}; missing=required-set(context_order); selected:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set()
    for c in contexts:
        cid=str(c["context_id"]); state=str(c.get("state","unknown"));
        if c.get("negative_result") is True:negative.add(f"{cid}:negative-result")
        if cid not in required:omissions.add(f"{cid}:not-required")
        if str(c.get("semantic_profile",""))!=str(request["semantic_profile"]):uncertainty.add(f"{cid}:semantic-profile-mismatch");unresolved.add(cid)
        elif state=="contradicted" or c.get("local_only") is not True or c.get("permitted") is not True:blocked.add(cid)
        elif state in {"unknown","speculative"}:unresolved.add(cid)
        elif cid in required:selected.add(cid)
        else:unresolved.add(cid)
    peers=sorted(request["peers"],key=lambda x:str(x.get("peer_id",""))); peer_order=[str(x.get("peer_id","")) for x in peers]
    if not all(peer_order) or len(set(peer_order))!=len(peer_order):raise ResearchContractError("peer identities must be unique and non-empty")
    qualified:set[str]=set(); missing_peers:set[str]=set(); contradictory=False
    for p in peers:
        pid=str(p["peer_id"]); valid=str(p.get("semantic_profile",""))==str(request["semantic_profile"]) and p.get("signed") is True and p.get("aggregate_only") is True and p.get("raw_data_local") is True and _digest(p.get("summary_digest")) and int(p.get("queue_depth",0))<=int(request["max_queue_depth"]) and str(p.get("state","unknown")) in {"proven","supported"}
        if p.get("state")=="contradicted":contradictory=True
        if valid:qualified.add(pid)
        else:missing_peers.add(pid);uncertainty.add(f"peer:{pid}:not-qualified")
    capacity=set()
    if int(request["active_runs"])>int(request["capacity"]):capacity.add("active-runs")
    if int(request["queue_depth"])>int(request["max_queue_depth"]):capacity.add("queue-depth")
    if capacity:uncertainty.add("request:capacity-envelope-exceeded")
    if len(qualified)<int(request["minimum_peer_quorum"]):uncertainty.add("peer:minimum-quorum-unmet")
    if request.get("policy_allow") is not True:negative.add("request:policy-denied")
    if request.get("protected_closure") is not True:uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True:uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True:uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{v}" for v in request.get("adversarial_events",[]))
    global_block=request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("signed_approval") is not True or request.get("federation_approved") is not True or request.get("raw_data_local") is not True or bool(request.get("adversarial_events")) or bool(capacity) or contradictory
    if global_block:blocked.update(context_order);selected.clear();unresolved.clear();missing.clear();omissions.add("request:control-gate-blocked")
    disposition="blocked" if global_block or blocked else "unresolved" if missing or unresolved or len(qualified)<int(request["minimum_peer_quorum"]) else "qualified"; selected_order,unresolved_order,blocked_order,missing_order=sorted(selected),sorted(unresolved),sorted(blocked),sorted(missing); qualified_order,missing_peer_order=sorted(qualified),sorted(missing_peers); capacity_order,omission_order,uncertainty_order,negative_order=sorted(capacity),sorted(omissions),sorted(uncertainty),sorted(negative); effects=[f"exchange:permitted-summaries:{request['request_id']}",f"manage:local-capability:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":str(request["request_id"]),"federation_id":str(request["federation_id"]),"query_id":str(request["query_id"]),"purpose":str(request["purpose"]),"semantic_profile":str(request["semantic_profile"]),"disposition":disposition,"context_order":context_order,"selected_context_order":selected_order,"unresolved_context_order":unresolved_order,"blocked_context_order":blocked_order,"missing_context_order":missing_order,"peer_order":peer_order,"qualified_peer_order":qualified_order,"missing_peer_order":missing_peer_order,"queue_depth":int(request["queue_depth"]),"active_runs":int(request["active_runs"]),"capacity_exceeded_order":capacity_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_order,"replay_identity":str(request["replay_identity"]),"effect_receipts":effects,"raw_data_local":True,"boundary":PRECLINICAL_BOUNDARY}; digest=_hash(payload); artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"ops-certified-decision-section:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY}; receipt=CertifiedDecisionSection(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID,str(request["request_id"]),str(request["federation_id"]),str(request["query_id"]),str(request["purpose"]),str(request["semantic_profile"]),disposition,tuple(context_order),tuple(selected_order),tuple(unresolved_order),tuple(blocked_order),tuple(missing_order),tuple(peer_order),tuple(qualified_order),tuple(missing_peer_order),int(request["queue_depth"]),int(request["active_runs"]),tuple(capacity_order),tuple(omission_order),tuple(uncertainty_order),tuple(negative_order),str(request["replay_identity"]),digest,artifact,tuple(effects),True,PRECLINICAL_BOUNDARY);receipt.validate();return receipt

__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CertifiedDecisionSection","operate_context_compilation"]
