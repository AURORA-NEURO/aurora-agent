"""Python parity surface for ``AFA-oracle-P28-F08``."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-oracle-P28-F08"; CONTRACT_VERSION="oracle-federated-continual-oracle-semantic-parity-contract-model/1.0"; INPUT_SCHEMA="OracleParityContract3@1"; OUTPUT_SCHEMA="OracleSemanticParityReceipt7@1"; CONTENT_TYPE="application/vnd.aurora.oracle-semantic-parity-receipt-7+json"
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _canonical(values:Sequence[str])->bool:return tuple(values)==tuple(sorted(set(values)))
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class OracleSemanticParityReceipt7:
    corpus_id:str; federation_id:str; semantic_profile:str; disposition:str; case_order:tuple[str,...]; passed_order:tuple[str,...]; mismatch_order:tuple[str,...]; unknown_order:tuple[str,...]; blocked_order:tuple[str,...]; missing_case_order:tuple[str,...]; omission_order:tuple[str,...]; uncertainty_order:tuple[str,...]; negative_evidence_order:tuple[str,...]; replay_identity:str; witness_digest:str; artifact:dict[str,Any]; effect_receipts:tuple[str,...]; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version:str=CONTRACT_VERSION; feature_id:str=FEATURE_ID
    def to_dict(self)->dict[str,Any]:return {"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"corpus_id":self.corpus_id,"federation_id":self.federation_id,"semantic_profile":self.semantic_profile,"disposition":self.disposition,"case_order":list(self.case_order),"passed_order":list(self.passed_order),"mismatch_order":list(self.mismatch_order),"unknown_order":list(self.unknown_order),"blocked_order":list(self.blocked_order),"missing_case_order":list(self.missing_case_order),"omission_order":list(self.omission_order),"uncertainty_order":list(self.uncertainty_order),"negative_evidence_order":list(self.negative_evidence_order),"replay_identity":self.replay_identity,"witness_digest":self.witness_digest,"artifact":self.artifact,"effect_receipts":list(self.effect_receipts),"raw_data_local":self.raw_data_local,"aggregate_only":self.aggregate_only,"boundary":self.boundary}
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not all(v.strip() for v in (self.corpus_id,self.federation_id,self.semantic_profile)) or not self.case_order or not self.effect_receipts or self.disposition not in {"qualified","unresolved","blocked"}:raise ResearchContractError("parity identity, locality, cases, or effects are incomplete")
        for values in (self.case_order,self.passed_order,self.mismatch_order,self.unknown_order,self.blocked_order,self.missing_case_order,self.omission_order,self.uncertainty_order,self.negative_evidence_order,self.effect_receipts):
            if not _canonical(values):raise ResearchContractError("parity ordering is not canonical")
        parts=[*self.passed_order,*self.mismatch_order,*self.unknown_order,*self.blocked_order]
        if set(parts)!=set(self.case_order) or len(parts)!=len(set(parts)):raise ResearchContractError("parity states do not partition cases")
        if not all(_digest(v) for v in (self.replay_identity,self.witness_digest,self.artifact.get("content_hash"))):raise ResearchContractError("parity digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("parity artifact type is invalid")
        if self.disposition=="qualified" and self.effect_receipts!=(f"verify:oracle-semantic-parity:{self.corpus_id}",):raise ResearchContractError("qualified parity effect is invalid")
        if self.disposition!="qualified" and self.effect_receipts!=("block:unsafe-release",):raise ResearchContractError("non-qualified parity must block release")

def oracle_semantic_parity_contract_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"oracle","consumers":["context compiler engineer","release governance board","federation operator"],"behavior":"verifies federated continual Oracle parity contracts across Rust, Python, TypeScript, schema, semantic, artifact, and provenance surfaces without executing workflows","value":"prevents cross-language semantic drift from changing an Oracle research contract silently and preserves mismatch evidence for release review","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute_local_computation","write_local_artifact","federation_export"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def model_oracle_semantic_parity_contract(*,corpus_id:str,federation_id:str,semantic_profile:str,required_case_order:Sequence[str],replay_identity:str,cases:Sequence[Mapping[str,Any]],policy_allow:bool,protected_closure:bool,signed_approval:bool,federation_approved:bool,raw_data_local:bool,aggregate_only:bool,adversarial_events:Sequence[str]=(),boundary:str=PRECLINICAL_BOUNDARY)->OracleSemanticParityReceipt7:
    if not all(v.strip() for v in (corpus_id,federation_id,semantic_profile)) or not required_case_order or not _canonical(required_case_order) or not _digest(replay_identity) or not cases or not _canonical(adversarial_events) or not raw_data_local or not aggregate_only or boundary!=PRECLINICAL_BOUNDARY:raise ResearchContractError("parity fixture identity, closure, replay, locality, or boundary is invalid")
    rows=[dict(x) for x in cases];seen:set[str]=set()
    for row in rows:
        cid=str(row.get("case_id","")); keys=("rust_digest","python_digest","typescript_digest","schema_digest","semantic_digest","artifact_digest","provenance_digest","replay_identity")
        if not cid.strip() or cid in seen or not str(row.get("semantic_profile","")).strip() or not all(_digest(row.get(k)) for k in keys) or not _canonical(row.get("omissions",())) or not _canonical(row.get("uncertainty",())):raise ResearchContractError(f"parity case {cid} is malformed or duplicated")
        seen.add(cid)
    rows.sort(key=lambda x:str(x["case_id"]));order=tuple(str(x["case_id"]) for x in rows);required=set(required_case_order);passed:set[str]=set();mismatch:set[str]=set();unknown:set[str]=set();blocked:set[str]=set();missing=required-set(order);omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set()
    for row in rows:
        cid=str(row["case_id"]); values=[row[k] for k in ("rust_digest","python_digest","typescript_digest","schema_digest","semantic_digest","artifact_digest","provenance_digest")]; parity=len(set(values))==1; omissions.update(f"{cid}:{v}" for v in row.get("omissions",())); uncertainty.update(f"{cid}:{v}" for v in row.get("uncertainty",()))
        if str(row.get("evidence_state",""))=="contradicted" or not row.get("local_only") or not row.get("permitted"):blocked.add(cid)
        elif not parity:mismatch.add(cid)
        elif str(row.get("replay_identity"))!=replay_identity or str(row["semantic_profile"])!=semantic_profile or row.get("omissions") or row.get("uncertainty") or str(row.get("evidence_state","")) not in {"proven","supported"}:unknown.add(cid)
        else:passed.add(cid)
    for cid in missing:omissions.add(f"{cid}:required-case-missing")
    if not policy_allow:negative.add("request:policy-denied")
    if not protected_closure:uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval:uncertainty.add("request:signed-approval-missing")
    if not federation_approved:uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{event}" for event in adversarial_events);global_block=not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or bool(adversarial_events)
    if global_block:blocked.update(order);passed.clear();unknown.clear();mismatch.clear();missing.clear();omissions.add("request:parity-release-gate-blocked")
    disposition="blocked" if global_block else "qualified" if required.issubset(passed) and not unknown and not mismatch and not blocked else "unresolved";po,mo,uo,bo,mm=tuple(sorted(passed)),tuple(sorted(mismatch)),tuple(sorted(unknown)),tuple(sorted(blocked)),tuple(sorted(missing));oo,uu,nn=tuple(sorted(omissions)),tuple(sorted(uncertainty)),tuple(sorted(negative));effects=(f"verify:oracle-semantic-parity:{corpus_id}",) if disposition=="qualified" else ("block:unsafe-release",);payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"corpus_id":corpus_id,"federation_id":federation_id,"semantic_profile":semantic_profile,"disposition":disposition,"case_order":list(order),"passed_order":list(po),"mismatch_order":list(mo),"unknown_order":list(uo),"blocked_order":list(bo),"missing_case_order":list(mm),"omission_order":list(oo),"uncertainty_order":list(uu),"negative_evidence_order":list(nn),"replay_identity":replay_identity,"effect_receipts":list(effects),"raw_data_local":raw_data_local,"aggregate_only":aggregate_only,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"oracle-semantic-parity:{corpus_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY};result=OracleSemanticParityReceipt7(corpus_id,federation_id,semantic_profile,disposition,order,po,mo,uo,bo,mm,oo,uu,nn,replay_identity,digest,artifact,effects);result.validate();return result
def oracleSemanticParityReceipt7Digest(result:OracleSemanticParityReceipt7)->str:result.validate();return _hash(result.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","OracleSemanticParityReceipt7","oracle_semantic_parity_contract_manifest","model_oracle_semantic_parity_contract","oracleSemanticParityReceipt7Digest"]

