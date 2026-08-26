"""Prospective high-throughput knowledge-representation parity contract."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence
from .research_contracts import THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION, THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

@dataclass(frozen=True)
class ThroughputKnowledgeJob:
    job_id:str; claims_digest:str; world_digest:str; evidence_digest:str|None; provenance_digest:str|None; replay_identity:str; state:str="supported"; ready:bool=True; retry_count:int=0; telemetry_digest:str|None=None; cost_units:int=1; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ThroughputKnowledgeReceipt:
    request_id:str; batch_id:str; disposition:str; candidate_order:tuple[str,...]; completed_order:tuple[str,...]; degraded_order:tuple[str,...]; unresolved_order:tuple[str,...]; denied_order:tuple[str,...]; exchange_order:tuple[str,...]; checkpoint_seq:int; retry_count:int; consumed_budget_units:int; run_digest:str; telemetry_digest:str; replay_identity:str; witness_order:tuple[str,...]; counterexample_order:tuple[str,...]; omissions:tuple[str,...]; uncertainty:tuple[str,...]; negative_evidence:tuple[str,...]; effect_receipts:tuple[str,...]; artifact:Mapping[str,Any]; feature_id:str=THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID; contract_version:str=THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version!=THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION or self.feature_id!=THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID: raise ResearchContractError("throughput knowledge schema mismatch")
        if self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.candidate_order or self.checkpoint_seq!=len(self.candidate_order) or not self.effect_receipts: raise ResearchContractError("throughput knowledge identity, checkpoint, locality, or effects are incomplete")
        for values in (self.candidate_order,self.completed_order,self.degraded_order,self.unresolved_order,self.denied_order,self.witness_order,self.counterexample_order,self.omissions,self.uncertainty,self.negative_evidence,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("throughput knowledge ordering is not canonical")
        if tuple(sorted(set(self.exchange_order)))!=self.exchange_order: raise ResearchContractError("throughput knowledge exchange ordering is not canonical")
        classified=set(self.completed_order)|set(self.degraded_order)|set(self.unresolved_order)|set(self.denied_order)
        if classified!=set(self.candidate_order): raise ResearchContractError("throughput knowledge states do not partition jobs")
        if len(self.exchange_order)!=len(self.completed_order): raise ResearchContractError("throughput knowledge exchange does not match completed jobs")
        for value in (*self.exchange_order,self.run_digest,self.telemetry_digest,self.replay_identity,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("throughput knowledge digest is invalid")
        if any(not e.startswith("read:local-throughput-knowledge:") and e!="block:unsafe-release" for e in self.effect_receipts): raise ResearchContractError("throughput knowledge effect is outside read-only gate")
    def digest(self)->str:
        self.validate(); return research_artifact_digest({"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"batch_id":self.batch_id,"disposition":self.disposition,"candidate_order":list(self.candidate_order),"completed_order":list(self.completed_order),"degraded_order":list(self.degraded_order),"unresolved_order":list(self.unresolved_order),"denied_order":list(self.denied_order),"exchange_order":list(self.exchange_order),"checkpoint_seq":self.checkpoint_seq,"retry_count":self.retry_count,"consumed_budget_units":self.consumed_budget_units,"run_digest":self.run_digest,"telemetry_digest":self.telemetry_digest,"replay_identity":self.replay_identity,"witness_order":list(self.witness_order),"counterexample_order":list(self.counterexample_order),"omissions":list(self.omissions),"uncertainty":list(self.uncertainty),"negative_evidence":list(self.negative_evidence),"effect_receipts":list(self.effect_receipts),"artifact":dict(self.artifact),"raw_data_local":self.raw_data_local,"boundary":self.boundary})

def infer_throughput_knowledge_representation(*,request_id:str,batch_id:str,jobs:Sequence[ThroughputKnowledgeJob],max_concurrency:int,max_retries:int,budget_units:int,replay_identity:str,policy_allow:bool=True,protected_closure:bool=True,raw_data_local:bool=True)->ThroughputKnowledgeReceipt:
    if not request_id.strip() or not batch_id.strip() or not jobs or max_concurrency<=0 or max_retries<0 or budget_units<=0 or not re.fullmatch(r"[0-9a-f]{64}",replay_identity) or not raw_data_local: raise ResearchContractError("throughput knowledge identity, queue, concurrency, budget, replay, locality, or replay is invalid")
    ordered=tuple(sorted(j.job_id for j in jobs))
    if len(set(ordered))!=len(jobs) or any(not v.strip() for v in ordered): raise ResearchContractError("throughput knowledge job identifiers must be unique and non-empty")
    jmap={j.job_id:j for j in jobs}; completed:set[str]=set(); degraded:set[str]=set(); unresolved:set[str]=set(); denied:set[str]=set(); exchange:list[str]=[]; witnesses={"gate:typed-throughput-knowledge","gate:queue-checkpoint","gate:concurrency-window","gate:bounded-retry","gate:telemetry","gate:evidence-provenance","gate:locality"}; counter:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set(); global_open=policy_allow and protected_closure and raw_data_local; consumed=0; retries=0
    for index,jid in enumerate(ordered):
        j=jmap[jid]; retries+=j.retry_count
        if not global_open or not j.raw_data_local or j.boundary!=PRECLINICAL_BOUNDARY: denied.add(jid); counter.add(f"counterexample:{jid}:policy-closure-locality")
        elif index>=max_concurrency: unresolved.add(jid); uncertainty.add(f"job:{jid}:concurrency-window")
        elif j.retry_count>max_retries: degraded.add(jid); omissions.add(f"job:{jid}:retry-budget-exhausted")
        elif consumed+j.cost_units>budget_units: denied.add(jid); omissions.add(f"job:{jid}:resource-budget-exhausted")
        elif not j.ready: unresolved.add(jid); uncertainty.add(f"job:{jid}:not-ready")
        elif j.replay_identity!=replay_identity: unresolved.add(jid); uncertainty.add(f"job:{jid}:replay-mismatch")
        elif j.telemetry_digest is None: unresolved.add(jid); omissions.add(f"job:{jid}:telemetry-missing")
        elif j.evidence_digest is None or j.provenance_digest is None: unresolved.add(jid); omissions.add(f"job:{jid}:evidence-or-provenance-missing")
        elif j.state in {"unknown","speculative"}: unresolved.add(jid); uncertainty.add(f"job:{jid}:unknown-not-asserted")
        elif j.state=="contradicted": denied.add(jid); negative.add(f"job:{jid}:contradicted")
        else: completed.add(jid); consumed+=j.cost_units; exchange.append(research_artifact_digest({"job_id":j.job_id,"claims_digest":j.claims_digest,"world_digest":j.world_digest,"evidence_digest":j.evidence_digest,"provenance_digest":j.provenance_digest,"telemetry_digest":j.telemetry_digest}))
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("control:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("control:protected-closure-incomplete")
    if unresolved or degraded: witnesses.add("gate:partial-knowledge-retained")
    exchange_order=tuple(sorted(exchange)); disposition="denied" if not global_open or denied else "unresolved" if unresolved else "degraded" if degraded else "completed"; telemetry=research_artifact_digest({"feature_id":THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,"batch_id":batch_id,"candidate_order":list(ordered),"retry_count":retries}); run=research_artifact_digest({"feature_id":THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,"request_id":request_id,"disposition":disposition,"completed_order":sorted(completed),"degraded_order":sorted(degraded),"unresolved_order":sorted(unresolved),"denied_order":sorted(denied),"checkpoint_seq":len(ordered),"consumed_budget_units":consumed,"telemetry_digest":telemetry,"replay_identity":replay_identity}); artifact={"content_hash":research_artifact_digest({"request_id":request_id,"run_digest":run}),"media_type":"application/vnd.aurora.throughput-knowledge-world+json"}; receipt=ThroughputKnowledgeReceipt(request_id=request_id,batch_id=batch_id,disposition=disposition,candidate_order=ordered,completed_order=tuple(sorted(completed)),degraded_order=tuple(sorted(degraded)),unresolved_order=tuple(sorted(unresolved)),denied_order=tuple(sorted(denied)),exchange_order=exchange_order,checkpoint_seq=len(ordered),retry_count=retries,consumed_budget_units=consumed,run_digest=run,telemetry_digest=telemetry,replay_identity=replay_identity,witness_order=tuple(sorted(witnesses)),counterexample_order=tuple(sorted(counter)),omissions=tuple(sorted(omissions)),uncertainty=tuple(sorted(uncertainty)),negative_evidence=tuple(sorted(negative)),effect_receipts=(f"read:local-throughput-knowledge:{request_id}",) if disposition=="completed" else ("block:unsafe-release",),artifact=artifact,raw_data_local=raw_data_local); receipt.validate(); return receipt
