"""Python parity surface for ``AFA-fiber-P08-F08``."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-fiber-P08-F08"; CONTRACT_VERSION="fiber-federated-continual-mechanism-exploration-contract-model/1.0"; INPUT_SCHEMA="MechanismQuestion4@1"; OUTPUT_SCHEMA="MechanismPortfolio2@1"; CONTENT_TYPE="application/vnd.aurora.mechanism-contract-model-2+json"
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _canonical(values:Sequence[str])->bool:return tuple(values)==tuple(sorted(set(values)))
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class FiberMechanismPortfolioContract:
    question_id:str; federation_id:str; semantic_profile:str; disposition:str; candidate_order:tuple[str,...]; selected_order:tuple[str,...]; unknown_order:tuple[str,...]; denied_order:tuple[str,...]; missing_candidate_order:tuple[str,...]; omission_order:tuple[str,...]; uncertainty_order:tuple[str,...]; negative_evidence_order:tuple[str,...]; replay_identity:str; contract_digest:str; artifact:dict[str,Any]; effect_receipts:tuple[str,...]=(); raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version:str=CONTRACT_VERSION; feature_id:str=FEATURE_ID
    def to_dict(self)->dict[str,Any]:return {"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"question_id":self.question_id,"federation_id":self.federation_id,"semantic_profile":self.semantic_profile,"disposition":self.disposition,"candidate_order":list(self.candidate_order),"selected_order":list(self.selected_order),"unknown_order":list(self.unknown_order),"denied_order":list(self.denied_order),"missing_candidate_order":list(self.missing_candidate_order),"omission_order":list(self.omission_order),"uncertainty_order":list(self.uncertainty_order),"negative_evidence_order":list(self.negative_evidence_order),"replay_identity":self.replay_identity,"contract_digest":self.contract_digest,"artifact":self.artifact,"effect_receipts":list(self.effect_receipts),"raw_data_local":self.raw_data_local,"aggregate_only":self.aggregate_only,"boundary":self.boundary}
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not all(v.strip() for v in (self.question_id,self.federation_id,self.semantic_profile)) or not self.candidate_order or self.disposition not in {"compatible","partial","unknown","blocked"}:raise ResearchContractError("mechanism contract identity, locality, or candidates are incomplete")
        for values in (self.candidate_order,self.selected_order,self.unknown_order,self.denied_order,self.missing_candidate_order,self.omission_order,self.uncertainty_order,self.negative_evidence_order,self.effect_receipts):
            if not _canonical(values):raise ResearchContractError("mechanism contract ordering is not canonical")
        parts=[*self.selected_order,*self.unknown_order,*self.denied_order]
        if set(parts)!=set(self.candidate_order) or len(parts)!=len(set(parts)):raise ResearchContractError("mechanism contract states do not partition candidates")
        if self.effect_receipts:raise ResearchContractError("mechanism contract model cannot claim an external effect")
        if not all(_digest(v) for v in (self.replay_identity,self.contract_digest,self.artifact.get("content_hash"))):raise ResearchContractError("mechanism contract digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("mechanism contract artifact type is invalid")

def mechanism_contract_model_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"fiber","consumers":["context compiler engineer","federation operator"],"behavior":"canonicalizes typed federated mechanism candidates and emits compatible, partial, unknown, or blocked contract receipts without discovering or executing mechanisms","value":"prevents semantic drift and silent evidence loss when policy-separated institutions exchange mechanism metadata","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def model_mechanism_contract(*,question_id:str,federation_id:str,semantic_profile:str,required_candidate_order:Sequence[str],replay_identity:str,candidates:Sequence[Mapping[str,Any]],policy_allow:bool,protected_closure:bool,signed_approval:bool,federation_approved:bool,raw_data_local:bool,aggregate_only:bool,adversarial_events:Sequence[str]=(),boundary:str=PRECLINICAL_BOUNDARY)->FiberMechanismPortfolioContract:
    if not all(v.strip() for v in (question_id,federation_id,semantic_profile)) or not required_candidate_order or not _canonical(required_candidate_order) or not candidates or not _digest(replay_identity) or not _canonical(adversarial_events) or not raw_data_local or not aggregate_only or boundary!=PRECLINICAL_BOUNDARY:raise ResearchContractError("mechanism question identity, closure, replay, locality, or boundary is invalid")
    rows=[dict(x) for x in candidates];seen:set[str]=set()
    for row in rows:
        cid=str(row.get("candidate_id",""))
        if not cid.strip() or cid in seen or not str(row.get("mechanism_id","")).strip() or not row.get("study_order") or not _canonical(row["study_order"]) or not row.get("modality_order") or not _canonical(row["modality_order"]) or not str(row.get("semantic_profile","")).strip() or not all(_digest(row.get(k)) for k in ("artifact_digest","evidence_digest","provenance_digest")) or not _canonical(row.get("omissions",())) or not _canonical(row.get("uncertainty",())):raise ResearchContractError(f"candidate {cid} is malformed or duplicated")
        seen.add(cid)
    rows.sort(key=lambda x:str(x["candidate_id"])); order=tuple(str(x["candidate_id"]) for x in rows); required=set(required_candidate_order); selected:set[str]=set();unknown:set[str]=set();denied:set[str]=set();missing=required-set(order);omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set()
    for row in rows:
        cid=str(row["candidate_id"]); state=str(row.get("evidence_state","")); omissions.update(f"{cid}:{v}" for v in row.get("omissions",())); uncertainty.update(f"{cid}:{v}" for v in row.get("uncertainty",())); negative.update({f"{cid}:negative-result"} if row.get("negative_result") else set())
        if state=="contradicted" or not row.get("local_only") or not row.get("permitted"):denied.add(cid)
        elif str(row["semantic_profile"])!=semantic_profile or row.get("omissions") or row.get("uncertainty") or state not in {"proven","supported"}:unknown.add(cid)
        else:selected.add(cid)
    for cid in missing:omissions.add(f"{cid}:required-candidate-missing")
    negative.update(f"adversarial:{event}" for event in adversarial_events); global_block=not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or bool(adversarial_events)
    if not policy_allow:negative.add("request:policy-denied")
    if not protected_closure:uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval:uncertainty.add("request:signed-approval-missing")
    if not federation_approved:uncertainty.add("request:federation-approval-missing")
    if global_block:denied.update(order);selected.clear();unknown.clear();missing.clear();omissions.add("request:contract-release-blocked")
    disposition="blocked" if global_block else "compatible" if required.issubset(selected) and not unknown and not denied else "unknown" if not selected or missing else "partial"; so,uo,do,mo=tuple(sorted(selected)),tuple(sorted(unknown)),tuple(sorted(denied)),tuple(sorted(missing));oo,uu,nn=tuple(sorted(omissions)),tuple(sorted(uncertainty)),tuple(sorted(negative));payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"question_id":question_id,"federation_id":federation_id,"semantic_profile":semantic_profile,"disposition":disposition,"candidate_order":list(order),"selected_order":list(so),"unknown_order":list(uo),"denied_order":list(do),"missing_candidate_order":list(mo),"omission_order":list(oo),"uncertainty_order":list(uu),"negative_evidence_order":list(nn),"replay_identity":replay_identity,"effect_receipts":[],"raw_data_local":raw_data_local,"aggregate_only":aggregate_only,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"mechanism-contract:{question_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY};result=FiberMechanismPortfolioContract(question_id,federation_id,semantic_profile,disposition,order,so,uo,do,mo,oo,uu,nn,replay_identity,digest,artifact);result.validate();return result
def fiberMechanismContractDigest(result:FiberMechanismPortfolioContract)->str:result.validate();return _hash(result.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","FiberMechanismPortfolioContract","mechanism_contract_model_manifest","model_mechanism_contract","fiberMechanismContractDigest"]
