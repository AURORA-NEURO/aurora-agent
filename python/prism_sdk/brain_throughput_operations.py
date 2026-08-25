"""Python parity contract for the throughput operations control plane."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

THROUGHPUT_OPERATIONS_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P01-F31"
THROUGHPUT_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION = "brain-throughput-operations-control-plane/1.0"

@dataclass(frozen=True)
class BrainThroughputOperationsReceipt:
    operation_id:str; actor_id:str; request_id:str; batch_id:str; partition:str; disposition:str; candidate_order:tuple[str,...]; admitted_order:tuple[str,...]; blocked_order:tuple[str,...]; unknown_order:tuple[str,...]; checkpoint_seq:int; attempts:int; recovered:bool; queue_digest:str; evidence_digest:str; operations_digest:str; replay_identity:str; omissions:tuple[str,...]; uncertainty:tuple[str,...]; negative_evidence:tuple[str,...]; effect_receipts:tuple[str,...]; artifact:Mapping[str,Any]; feature_id:str=THROUGHPUT_OPERATIONS_CONTROL_PLANE_FEATURE_ID; contract_version:str=THROUGHPUT_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id!=THROUGHPUT_OPERATIONS_CONTROL_PLANE_FEATURE_ID or self.contract_version!=THROUGHPUT_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION: raise ResearchContractError("throughput operations schema mismatch")
        if self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.operation_id.strip() or not self.actor_id.strip() or not self.request_id.strip() or not self.batch_id.strip() or not self.partition.strip() or self.disposition not in ("completed","degraded","unresolved","denied") or not self.candidate_order or self.attempts<1 or not self.effect_receipts: raise ResearchContractError("throughput operations identity incomplete")
        if any(v not in self.candidate_order for v in (*self.admitted_order,*self.blocked_order,*self.unknown_order)): raise ResearchContractError("throughput operations state is not covered")
        for values in (self.candidate_order,self.admitted_order,self.blocked_order,self.unknown_order,self.omissions,self.uncertainty,self.negative_evidence,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("throughput operations ordering invalid")
        for value in (self.queue_digest,self.evidence_digest,self.operations_digest,self.replay_identity,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("throughput operations digest invalid")
        if any(not effect.startswith("ops:throughput:") and effect!="block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("throughput operations effect invalid")
    def digest(self)->str:
        self.validate(); return research_artifact_digest({"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"operation_id":self.operation_id,"actor_id":self.actor_id,"request_id":self.request_id,"batch_id":self.batch_id,"partition":self.partition,"disposition":self.disposition,"candidate_order":list(self.candidate_order),"admitted_order":list(self.admitted_order),"blocked_order":list(self.blocked_order),"unknown_order":list(self.unknown_order),"checkpoint_seq":self.checkpoint_seq,"attempts":self.attempts,"recovered":self.recovered,"queue_digest":self.queue_digest,"evidence_digest":self.evidence_digest,"operations_digest":self.operations_digest,"replay_identity":self.replay_identity,"omissions":list(self.omissions),"uncertainty":list(self.uncertainty),"negative_evidence":list(self.negative_evidence),"effect_receipts":list(self.effect_receipts),"artifact":dict(self.artifact),"raw_data_local":self.raw_data_local,"boundary":self.boundary})
