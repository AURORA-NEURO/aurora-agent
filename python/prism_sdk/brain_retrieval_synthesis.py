"""Python parity contract for local retrieval and synthesis."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F01"
RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-retrieval-synthesis/1.0"

@dataclass(frozen=True)
class BrainEvidenceSynthesis:
    request_id:str; study_id:str; scope:str; disposition:str; candidate_order:tuple[str,...]; ranked_order:tuple[str,...]; qualified_order:tuple[str,...]; blocked_order:tuple[str,...]; unknown_order:tuple[str,...]; support_order:tuple[int,...]; source_order:tuple[str,...]; modality_order:tuple[str,...]; semantic_order:tuple[str,...]; artifact_order:tuple[str,...]; provenance_order:tuple[str,...]; omissions:tuple[str,...]; uncertainty:tuple[str,...]; negative_evidence:tuple[str,...]; replay_identity:str; synthesis_digest:str; effect_receipts:tuple[str,...]; artifact:Mapping[str,Any]; feature_id:str=RETRIEVAL_SYNTHESIS_FEATURE_ID; contract_version:str=RETRIEVAL_SYNTHESIS_CONTRACT_VERSION; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id!=RETRIEVAL_SYNTHESIS_FEATURE_ID or self.contract_version!=RETRIEVAL_SYNTHESIS_CONTRACT_VERSION: raise ResearchContractError("retrieval synthesis schema mismatch")
        if self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or not self.scope.strip() or self.disposition not in ("qualified","partial","unknown","blocked") or not self.candidate_order or not self.ranked_order or len(self.ranked_order)!=len(self.support_order) or not self.effect_receipts: raise ResearchContractError("retrieval synthesis identity incomplete")
        if any(v not in self.candidate_order for v in (*self.ranked_order,*self.qualified_order,*self.blocked_order,*self.unknown_order)): raise ResearchContractError("retrieval synthesis state is not covered")
        for values in (self.candidate_order,self.qualified_order,self.blocked_order,self.unknown_order,self.source_order,self.modality_order,self.omissions,self.uncertainty,self.negative_evidence,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("retrieval synthesis ordering invalid")
        for value in (self.replay_identity,self.synthesis_digest,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("retrieval synthesis digest invalid")
        if any(not effect.startswith("read:local-research-artifacts:") and effect!="block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("retrieval synthesis effect invalid")
    def digest(self)->str:
        self.validate(); return research_artifact_digest({"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"study_id":self.study_id,"scope":self.scope,"disposition":self.disposition,"candidate_order":list(self.candidate_order),"ranked_order":list(self.ranked_order),"qualified_order":list(self.qualified_order),"blocked_order":list(self.blocked_order),"unknown_order":list(self.unknown_order),"support_order":list(self.support_order),"source_order":list(self.source_order),"modality_order":list(self.modality_order),"semantic_order":list(self.semantic_order),"artifact_order":list(self.artifact_order),"provenance_order":list(self.provenance_order),"omissions":list(self.omissions),"uncertainty":list(self.uncertainty),"negative_evidence":list(self.negative_evidence),"replay_identity":self.replay_identity,"synthesis_digest":self.synthesis_digest,"effect_receipts":list(self.effect_receipts),"artifact":dict(self.artifact),"raw_data_local":self.raw_data_local,"boundary":self.boundary})
