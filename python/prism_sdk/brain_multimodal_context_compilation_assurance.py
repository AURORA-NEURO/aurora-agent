"""Multimodal multi-study context-compilation assurance parity contract."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence
from .research_contracts import MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION, MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

@dataclass(frozen=True)
class MultimodalContextAssuranceCell:
    study_id: str; modality: str; context_digest: str; section_digest: str; evidence_digest: str|None; provenance_digest: str|None; replay_identity: str; state: str="supported"; comparable: bool=True; raw_data_local: bool=True; boundary: str=PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class MultimodalContextAssuranceReceipt:
    request_id: str; scope: str; verdict: str; study_order: tuple[str,...]; modality_order: tuple[str,...]; candidate_order: tuple[str,...]; qualified_order: tuple[str,...]; blocked_order: tuple[str,...]; unknown_order: tuple[str,...]; missing_order: tuple[str,...]; incomparable_order: tuple[str,...]; witness_order: tuple[str,...]; counterexample_order: tuple[str,...]; verification_digest: str; replay_identity: str; omissions: tuple[str,...]; uncertainty: tuple[str,...]; negative_evidence: tuple[str,...]; effect_receipts: tuple[str,...]; artifact: Mapping[str,Any]; feature_id: str=MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID; contract_version: str=MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION; schema_version: str=RESEARCH_CONTRACT_SCHEMA_VERSION; raw_data_local: bool=True; boundary: str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if self.schema_version!=RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id!=MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID or self.contract_version!=MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION: raise ResearchContractError("multimodal assurance schema, feature, or version mismatch")
        if self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.scope.strip() or len(self.study_order)<2 or len(self.modality_order)<2 or not self.candidate_order or not self.witness_order or not self.effect_receipts or self.verdict not in {"qualified","unresolved","blocked"}: raise ResearchContractError("multimodal assurance identity, closure, witnesses, locality, or effects are incomplete")
        for values in (self.study_order,self.modality_order,self.candidate_order,self.qualified_order,self.blocked_order,self.unknown_order,self.missing_order,self.incomparable_order,self.witness_order,self.counterexample_order,self.omissions,self.uncertainty,self.negative_evidence,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("multimodal assurance ordering is not canonical")
        classified=set(self.qualified_order)|set(self.blocked_order)|set(self.unknown_order)
        if classified!=set(self.candidate_order): raise ResearchContractError("multimodal assurance outcomes do not partition candidates")
        for value in (self.verification_digest,self.replay_identity,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("multimodal assurance digest is invalid")
        if any(not e.startswith("assurance:local-multimodal-context:") and e!="block:unsafe-release" for e in self.effect_receipts): raise ResearchContractError("multimodal assurance effect is outside the local release gate")
    def digest(self)->str:
        self.validate(); return research_artifact_digest({"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"scope":self.scope,"verdict":self.verdict,"study_order":list(self.study_order),"modality_order":list(self.modality_order),"candidate_order":list(self.candidate_order),"qualified_order":list(self.qualified_order),"blocked_order":list(self.blocked_order),"unknown_order":list(self.unknown_order),"missing_order":list(self.missing_order),"incomparable_order":list(self.incomparable_order),"witness_order":list(self.witness_order),"counterexample_order":list(self.counterexample_order),"verification_digest":self.verification_digest,"replay_identity":self.replay_identity,"omissions":list(self.omissions),"uncertainty":list(self.uncertainty),"negative_evidence":list(self.negative_evidence),"effect_receipts":list(self.effect_receipts),"artifact":dict(self.artifact),"raw_data_local":self.raw_data_local,"boundary":self.boundary})

def assure_multimodal_context_compilation(*, request_id:str, scope:str, study_ids:Sequence[str], modalities:Sequence[str], cells:Sequence[MultimodalContextAssuranceCell], replay_identity:str, policy_allow:bool=True, protected_closure:bool=True, raw_data_local:bool=True)->MultimodalContextAssuranceReceipt:
    if not request_id.strip() or not scope.strip() or len(study_ids)<2 or len(modalities)<2 or not re.fullmatch(r"[0-9a-f]{64}",replay_identity): raise ResearchContractError("multimodal assurance identity, closure, or replay is invalid")
    studies=tuple(sorted(set(study_ids))); modes=tuple(sorted(set(modalities)))
    if len(studies)!=len(study_ids) or len(modes)!=len(modalities) or any(not x.strip() for x in (*studies,*modes)): raise ResearchContractError("study and modality identifiers must be unique and non-empty")
    candidate=tuple(f"{s}|{m}" for s in studies for m in modes); cell_map={f"{c.study_id}|{c.modality}":c for c in cells}
    if len(cell_map)!=len(cells): raise ResearchContractError("multimodal assurance cells must be unique")
    qualified:set[str]=set(); blocked:set[str]=set(); unknown:set[str]=set(); missing:set[str]=set(); incomparable:set[str]=set(); witnesses={"gate:typed-multimodal-contract","gate:study-modality-closure","gate:comparability","gate:provenance","gate:replay-identity","gate:locality"}; counter:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set(); open_gate=policy_allow and protected_closure and raw_data_local
    for key in candidate:
        cell=cell_map.get(key)
        if cell is None: unknown.add(key); missing.add(key); omissions.add(f"cell:{key}:missing")
        elif not open_gate or not cell.raw_data_local or cell.boundary!=PRECLINICAL_BOUNDARY: blocked.add(key); counter.add(f"counterexample:{key}:policy-protected-closure-locality")
        elif not cell.comparable: blocked.add(key); incomparable.add(key); negative.add(f"cell:{key}:incomparable")
        elif cell.replay_identity!=replay_identity: unknown.add(key); uncertainty.add(f"cell:{key}:replay-mismatch")
        elif cell.evidence_digest is None or cell.provenance_digest is None: unknown.add(key); omissions.add(f"cell:{key}:evidence-or-provenance-missing")
        elif cell.state in {"unknown","speculative"}: unknown.add(key); uncertainty.add(f"cell:{key}:evidence-uncertain")
        elif cell.state=="contradicted": blocked.add(key); negative.add(f"cell:{key}:contradicted")
        else: qualified.add(key)
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("assurance:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("assurance:protected-closure-incomplete")
    if not raw_data_local: counter.add("counterexample:raw-data-locality-failed"); omissions.add("assurance:raw-data-locality-failed")
    if missing: witnesses.add("gate:missing-modality-retained")
    verdict="blocked" if not open_gate or blocked else "unresolved" if unknown else "qualified"; verification=research_artifact_digest({"feature_id":MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,"request_id":request_id,"candidate_order":list(candidate),"qualified_order":sorted(qualified),"blocked_order":sorted(blocked),"unknown_order":sorted(unknown),"missing_order":sorted(missing),"incomparable_order":sorted(incomparable),"witness_order":sorted(witnesses),"counterexample_order":sorted(counter),"verdict":verdict,"replay_identity":replay_identity}); artifact={"content_hash":research_artifact_digest({"request_id":request_id,"verification_digest":verification}),"media_type":"application/vnd.aurora.multimodal-context-compilation-assurance+json"}
    receipt=MultimodalContextAssuranceReceipt(request_id=request_id,scope=scope,verdict=verdict,study_order=studies,modality_order=modes,candidate_order=candidate,qualified_order=tuple(sorted(qualified)),blocked_order=tuple(sorted(blocked)),unknown_order=tuple(sorted(unknown)),missing_order=tuple(sorted(missing)),incomparable_order=tuple(sorted(incomparable)),witness_order=tuple(sorted(witnesses)),counterexample_order=tuple(sorted(counter)),verification_digest=verification,replay_identity=replay_identity,omissions=tuple(sorted(omissions)),uncertainty=tuple(sorted(uncertainty)),negative_evidence=tuple(sorted(negative)),effect_receipts=(f"assurance:local-multimodal-context:{request_id}",) if verdict=="qualified" else ("block:unsafe-release",),artifact=artifact,raw_data_local=raw_data_local); receipt.validate(); return receipt
