"""Python parity surface for ``AFA-oncoworlds-P15-F28``."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-oncoworlds-P15-F28"; CONTRACT_VERSION="oncoworlds-federated-continual-replication-negative-results-assurance-harness/1.0"; INPUT_SCHEMA="OncoworldsClaimAndProtocol4@1"; OUTPUT_SCHEMA="OncoworldsReplicationRecord7@1"; CONTENT_TYPE="application/vnd.aurora.replication-record-7+json"
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _canonical(values:Sequence[str])->bool:return tuple(values)==tuple(sorted(set(values)))
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class OncoworldsReplicationRecord:
    run_id:str; federation_id:str; semantic_profile:str; disposition:str; claim_order:tuple[str,...]; admitted_order:tuple[str,...]; unresolved_order:tuple[str,...]; blocked_order:tuple[str,...]; reproduced_order:tuple[str,...]; negative_result_order:tuple[str,...]; missing_claim_order:tuple[str,...]; omission_order:tuple[str,...]; uncertainty_order:tuple[str,...]; negative_evidence_order:tuple[str,...]; replay_identity:str; record_digest:str; artifact:dict[str,Any]; effect_receipts:tuple[str,...]; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version:str=CONTRACT_VERSION; feature_id:str=FEATURE_ID
    def to_dict(self)->dict[str,Any]:return {"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"run_id":self.run_id,"federation_id":self.federation_id,"semantic_profile":self.semantic_profile,"disposition":self.disposition,"claim_order":list(self.claim_order),"admitted_order":list(self.admitted_order),"unresolved_order":list(self.unresolved_order),"blocked_order":list(self.blocked_order),"reproduced_order":list(self.reproduced_order),"negative_result_order":list(self.negative_result_order),"missing_claim_order":list(self.missing_claim_order),"omission_order":list(self.omission_order),"uncertainty_order":list(self.uncertainty_order),"negative_evidence_order":list(self.negative_evidence_order),"replay_identity":self.replay_identity,"record_digest":self.record_digest,"artifact":self.artifact,"effect_receipts":list(self.effect_receipts),"raw_data_local":self.raw_data_local,"aggregate_only":self.aggregate_only,"boundary":self.boundary}
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not all(v.strip() for v in (self.run_id,self.federation_id,self.semantic_profile)) or not self.claim_order or not self.effect_receipts or self.disposition not in {"qualified","unresolved","blocked"}:raise ResearchContractError("replication identity, locality, claims, or effects are incomplete")
        for values in (self.claim_order,self.admitted_order,self.unresolved_order,self.blocked_order,self.reproduced_order,self.negative_result_order,self.missing_claim_order,self.omission_order,self.uncertainty_order,self.negative_evidence_order,self.effect_receipts):
            if not _canonical(values):raise ResearchContractError("replication ordering is not canonical")
        parts=[*self.admitted_order,*self.unresolved_order,*self.blocked_order]
        if set(parts)!=set(self.claim_order) or len(parts)!=len(set(parts)):raise ResearchContractError("replication states do not partition claims")
        if not all(_digest(v) for v in (self.replay_identity,self.record_digest,self.artifact.get("content_hash"))):raise ResearchContractError("replication digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("replication artifact type is invalid")
        if self.disposition=="qualified" and self.effect_receipts!=(f"verify:replication-record:{self.run_id}",):raise ResearchContractError("qualified replication effect is invalid")
        if self.disposition!="qualified" and self.effect_receipts!=("block:unsafe-release",):raise ResearchContractError("non-qualified replication must block release")

def oncoworlds_replication_negative_results_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"oncoworlds","consumers":["research program lead","replication lead","consortium operator"],"behavior":"verifies federated replication claims and retains null or failed outcomes as typed evidence without converting them into positive conclusions","value":"makes reproducibility, contradiction, negative results, omissions, and policy boundaries auditable before research-object release","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute_local_computation","write_local_artifact","federation_export"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def assure_replication(*,run_id:str,federation_id:str,semantic_profile:str,required_claim_order:Sequence[str],protocol_digest:str,replay_identity:str,claims:Sequence[Mapping[str,Any]],policy_allow:bool,protected_closure:bool,signed_approval:bool,federation_approved:bool,raw_data_local:bool,aggregate_only:bool,adversarial_events:Sequence[str]=(),boundary:str=PRECLINICAL_BOUNDARY)->OncoworldsReplicationRecord:
    if not all(v.strip() for v in (run_id,federation_id,semantic_profile)) or not required_claim_order or not _canonical(required_claim_order) or not _digest(protocol_digest) or not _digest(replay_identity) or not claims or not _canonical(adversarial_events) or not raw_data_local or not aggregate_only or boundary!=PRECLINICAL_BOUNDARY:raise ResearchContractError("claim/protocol identity, closure, digests, locality, or boundary is invalid")
    rows=[dict(x) for x in claims];seen:set[str]=set()
    for row in rows:
        cid=str(row.get("claim_id",""))
        if not cid.strip() or cid in seen or not str(row.get("study_id","")).strip() or not str(row.get("independent_site_id","")).strip() or not all(_digest(row.get(k)) for k in ("protocol_digest","result_digest","provenance_digest","replay_identity")) or not str(row.get("semantic_profile","")).strip() or not _canonical(row.get("omissions",())) or not _canonical(row.get("uncertainty",())):raise ResearchContractError(f"claim {cid} is malformed or duplicated")
        seen.add(cid)
    rows.sort(key=lambda x:str(x["claim_id"])); order=tuple(str(x["claim_id"]) for x in rows);required=set(required_claim_order);admitted:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();reproduced:set[str]=set();negative_result:set[str]=set();missing=required-set(order);omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set()
    for row in rows:
        cid=str(row["claim_id"]);outcome=str(row.get("outcome",""));state=str(row.get("evidence_state","")); omissions.update(f"{cid}:{v}" for v in row.get("omissions",())); uncertainty.update(f"{cid}:{v}" for v in row.get("uncertainty",()))
        if outcome in {"null","failed"}:negative_result.add(cid);negative.add(f"{cid}:{outcome}")
        if state=="contradicted" or not row.get("local_only") or not row.get("permitted"):blocked.add(cid)
        elif str(row["semantic_profile"])!=semantic_profile or str(row.get("replay_identity"))!=replay_identity or row.get("omissions") or row.get("uncertainty") or state not in {"proven","supported"}:unresolved.add(cid)
        else:admitted.add(cid);reproduced.update({cid} if outcome=="reproduced" else set())
    for cid in missing:omissions.add(f"{cid}:required-claim-missing")
    if not policy_allow:negative.add("request:policy-denied")
    if not protected_closure:uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval:uncertainty.add("request:signed-approval-missing")
    if not federation_approved:uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{event}" for event in adversarial_events);global_block=not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or bool(adversarial_events)
    if global_block:blocked.update(order);admitted.clear();unresolved.clear();missing.clear();omissions.add("request:replication-release-gate-blocked")
    disposition="blocked" if global_block else "qualified" if required.issubset(admitted) and not unresolved and not blocked else "unresolved";ao,uo,bo,ro,no,mo=tuple(sorted(admitted)),tuple(sorted(unresolved)),tuple(sorted(blocked)),tuple(sorted(reproduced)),tuple(sorted(negative_result)),tuple(sorted(missing));oo,uu,ne=tuple(sorted(omissions)),tuple(sorted(uncertainty)),tuple(sorted(negative));effects=(f"verify:replication-record:{run_id}",) if disposition=="qualified" else ("block:unsafe-release",);payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"run_id":run_id,"federation_id":federation_id,"semantic_profile":semantic_profile,"disposition":disposition,"claim_order":list(order),"admitted_order":list(ao),"unresolved_order":list(uo),"blocked_order":list(bo),"reproduced_order":list(ro),"negative_result_order":list(no),"missing_claim_order":list(mo),"omission_order":list(oo),"uncertainty_order":list(uu),"negative_evidence_order":list(ne),"replay_identity":replay_identity,"effect_receipts":list(effects),"raw_data_local":raw_data_local,"aggregate_only":aggregate_only,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"replication-record:{run_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY};result=OncoworldsReplicationRecord(run_id,federation_id,semantic_profile,disposition,order,ao,uo,bo,ro,no,mo,oo,uu,ne,replay_identity,digest,artifact,effects);result.validate();return result
def oncoworldsReplicationDigest(result:OncoworldsReplicationRecord)->str:result.validate();return _hash(result.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","OncoworldsReplicationRecord","oncoworlds_replication_negative_results_manifest","assure_replication","oncoworldsReplicationDigest"]

