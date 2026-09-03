"""Prospective high-throughput context assurance parity contract."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence
from .research_contracts import THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION, THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

@dataclass(frozen=True)
class ThroughputContextAssuranceJob:
    job_id:str; context_digest:str; section_digest:str; evidence_digest:str|None; provenance_digest:str|None; replay_identity:str; state:str="supported"; ready:bool=True; cost_units:int=1; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ThroughputContextAssuranceReceipt:
    request_id:str; batch_id:str; partition:str; verdict:str; candidate_order:tuple[str,...]; qualified_order:tuple[str,...]; blocked_order:tuple[str,...]; unknown_order:tuple[str,...]; checkpoint_seq:int; queue_digest:str; verification_digest:str; replay_identity:str; witness_order:tuple[str,...]; counterexample_order:tuple[str,...]; omissions:tuple[str,...]; uncertainty:tuple[str,...]; negative_evidence:tuple[str,...]; effect_receipts:tuple[str,...]; artifact:Mapping[str,Any]; feature_id:str=THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID; contract_version:str=THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id!=THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID or self.contract_version!=THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION: raise ResearchContractError("throughput assurance schema, feature, or version mismatch")
        if self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.partition.strip() or not self.candidate_order or not self.witness_order or not self.effect_receipts or self.verdict not in {"qualified","unresolved","blocked"}: raise ResearchContractError("throughput assurance identity, queue, witnesses, locality, or effects are incomplete")
        for values in (self.candidate_order,self.qualified_order,self.blocked_order,self.unknown_order,self.witness_order,self.counterexample_order,self.omissions,self.uncertainty,self.negative_evidence,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("throughput assurance ordering is not canonical")
        classified=set(self.qualified_order)|set(self.blocked_order)|set(self.unknown_order)
        if classified!=set(self.candidate_order): raise ResearchContractError("throughput assurance outcomes do not partition candidates")
        for value in (self.queue_digest,self.verification_digest,self.replay_identity,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("throughput assurance digest is invalid")
        if any(not e.startswith("assurance:local-throughput-context:") and e!="block:unsafe-release" for e in self.effect_receipts): raise ResearchContractError("throughput assurance effect is outside the local release gate")
    def digest(self)->str:
        self.validate(); return research_artifact_digest({"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"batch_id":self.batch_id,"partition":self.partition,"verdict":self.verdict,"candidate_order":list(self.candidate_order),"qualified_order":list(self.qualified_order),"blocked_order":list(self.blocked_order),"unknown_order":list(self.unknown_order),"checkpoint_seq":self.checkpoint_seq,"queue_digest":self.queue_digest,"verification_digest":self.verification_digest,"replay_identity":self.replay_identity,"witness_order":list(self.witness_order),"counterexample_order":list(self.counterexample_order),"omissions":list(self.omissions),"uncertainty":list(self.uncertainty),"negative_evidence":list(self.negative_evidence),"effect_receipts":list(self.effect_receipts),"artifact":dict(self.artifact),"raw_data_local":self.raw_data_local,"boundary":self.boundary})
def assure_throughput_context_compilation(*,request_id:str,batch_id:str,partition:str,jobs:Sequence[ThroughputContextAssuranceJob],max_concurrency:int,budget_units:int,replay_identity:str,policy_allow:bool=True,protected_closure:bool=True,raw_data_local:bool=True)->ThroughputContextAssuranceReceipt:
    if not request_id.strip() or not batch_id.strip() or not partition.strip() or not jobs or max_concurrency<=0 or budget_units<=0 or not re.fullmatch(r"[0-9a-f]{64}",replay_identity): raise ResearchContractError("throughput assurance identity, queue, budget, or replay is invalid")
    ordered=tuple(sorted(jobs,key=lambda j:j.job_id)); candidate=tuple(j.job_id for j in ordered)
    if any(not x.strip() for x in candidate) or len(set(candidate))!=len(candidate): raise ResearchContractError("throughput job identifiers must be unique and non-empty")
    qualified:set[str]=set(); blocked:set[str]=set(); unknown:set[str]=set(); witnesses={"gate:typed-throughput-contract","gate:queue-checkpoint","gate:provenance","gate:replay-identity","gate:concurrency-window","gate:budget","gate:locality"}; counter:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set(); open_gate=policy_allow and protected_closure and raw_data_local; consumed=0
    for job in ordered:
        if not open_gate or not job.raw_data_local or job.boundary!=PRECLINICAL_BOUNDARY: blocked.add(job.job_id); counter.add(f"counterexample:{job.job_id}:policy-protected-closure-locality")
        elif job.replay_identity!=replay_identity: unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:replay-mismatch")
        elif not job.ready: unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:not-ready")
        elif job.evidence_digest is None or job.provenance_digest is None: unknown.add(job.job_id); omissions.add(f"job:{job.job_id}:evidence-or-provenance-missing")
        elif job.state in {"unknown","speculative"}: unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:evidence-uncertain")
        elif job.state=="contradicted": blocked.add(job.job_id); negative.add(f"job:{job.job_id}:contradicted")
        elif len(qualified)>=max_concurrency: unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:concurrency-window")
        elif consumed+job.cost_units>budget_units: blocked.add(job.job_id); omissions.add(f"job:{job.job_id}:budget-exhausted")
        else: qualified.add(job.job_id); consumed+=job.cost_units
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("assurance:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("assurance:protected-closure-incomplete")
    if not raw_data_local: counter.add("counterexample:raw-data-locality-failed"); omissions.add("assurance:raw-data-locality-failed")
    if unknown: witnesses.add("gate:unresolved-batch-retained")
    verdict="blocked" if not open_gate or blocked else "unresolved" if unknown else "qualified"; queue=research_artifact_digest({"candidate_order":list(candidate),"qualified_order":sorted(qualified),"blocked_order":sorted(blocked),"unknown_order":sorted(unknown),"max_concurrency":max_concurrency,"budget_units":budget_units,"consumed":consumed,"replay_identity":replay_identity}); verification=research_artifact_digest({"feature_id":THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,"request_id":request_id,"batch_id":batch_id,"queue_digest":queue,"witness_order":sorted(witnesses),"counterexample_order":sorted(counter),"verdict":verdict,"replay_identity":replay_identity}); artifact={"content_hash":research_artifact_digest({"request_id":request_id,"verification_digest":verification}),"media_type":"application/vnd.aurora.throughput-context-compilation-assurance+json"}
    receipt=ThroughputContextAssuranceReceipt(request_id=request_id,batch_id=batch_id,partition=partition,verdict=verdict,candidate_order=candidate,qualified_order=tuple(sorted(qualified)),blocked_order=tuple(sorted(blocked)),unknown_order=tuple(sorted(unknown)),checkpoint_seq=len(ordered),queue_digest=queue,verification_digest=verification,replay_identity=replay_identity,witness_order=tuple(sorted(witnesses)),counterexample_order=tuple(sorted(counter)),omissions=tuple(sorted(omissions)),uncertainty=tuple(sorted(uncertainty)),negative_evidence=tuple(sorted(negative)),effect_receipts=(f"assurance:local-throughput-context:{request_id}",) if verdict=="qualified" else ("block:unsafe-release",),artifact=artifact,raw_data_local=raw_data_local); receipt.validate(); return receipt
