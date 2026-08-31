"""Python parity contract for the local evidence protocol adapter."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
EVIDENCE_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P01-F21"
EVIDENCE_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-evidence-protocol-adapter/1.0"
@dataclass(frozen=True)
class BrainEvidenceProtocolReceipt:
    request_id:str; protocol_version:str; method:str; route:str; content_type:str; idempotency_key:str; response_schema:str; status_code:int; disposition:str; candidate_order:tuple[str,...]; qualified_order:tuple[str,...]; blocked_order:tuple[str,...]; unknown_order:tuple[str,...]; evidence_digest:str; request_digest:str; response_digest:str; replay_identity:str; omissions:tuple[str,...]; uncertainty:tuple[str,...]; negative_evidence:tuple[str,...]; effect_receipts:tuple[str,...]; artifact:Mapping[str,Any]; feature_id:str=EVIDENCE_PROTOCOL_ADAPTER_FEATURE_ID; contract_version:str=EVIDENCE_PROTOCOL_ADAPTER_CONTRACT_VERSION; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id!=EVIDENCE_PROTOCOL_ADAPTER_FEATURE_ID or self.contract_version!=EVIDENCE_PROTOCOL_ADAPTER_CONTRACT_VERSION: raise ResearchContractError("protocol schema, feature, or version mismatch")
        if self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or self.protocol_version!="aurora-research/1.0" or self.method!="POST" or self.route!="/v1/research/evidence/surveil" or self.content_type!="application/json" or not self.idempotency_key.strip() or self.response_schema!="EvidenceProtocolResponse1@1" or not self.candidate_order or not self.effect_receipts: raise ResearchContractError("protocol identity, route, idempotency, evidence, locality, or effects are incomplete")
        if any(v not in self.candidate_order for v in (*self.qualified_order,*self.blocked_order,*self.unknown_order)): raise ResearchContractError("protocol state is not covered by candidates")
        for values in (self.candidate_order,self.qualified_order,self.blocked_order,self.unknown_order,self.omissions,self.uncertainty,self.negative_evidence,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("protocol ordering is invalid")
        if self.status_code not in (200,202,206,403,422): raise ResearchContractError("protocol status code is invalid")
        for value in (self.evidence_digest,self.request_digest,self.response_digest,self.replay_identity,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("protocol digest is invalid")
        if any(not e.startswith("protocol:local-response:") and e!="block:unsafe-release" for e in self.effect_receipts): raise ResearchContractError("protocol effect is invalid")
    def digest(self)->str:
        self.validate(); return research_artifact_digest({"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"protocol_version":self.protocol_version,"method":self.method,"route":self.route,"content_type":self.content_type,"idempotency_key":self.idempotency_key,"response_schema":self.response_schema,"status_code":self.status_code,"disposition":self.disposition,"candidate_order":list(self.candidate_order),"qualified_order":list(self.qualified_order),"blocked_order":list(self.blocked_order),"unknown_order":list(self.unknown_order),"evidence_digest":self.evidence_digest,"request_digest":self.request_digest,"response_digest":self.response_digest,"replay_identity":self.replay_identity,"omissions":list(self.omissions),"uncertainty":list(self.uncertainty),"negative_evidence":list(self.negative_evidence),"effect_receipts":list(self.effect_receipts),"artifact":dict(self.artifact),"raw_data_local":self.raw_data_local,"boundary":self.boundary})
