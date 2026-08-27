"""Python parity surface for ``AFA-weavelang-P26-F32``."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-weavelang-P26-F32"; CONTRACT_VERSION="weavelang-federated-continual-limitation-closure-control-plane/1.0"; INPUT_SCHEMA="WeavelangLimitationCase4@1"; OUTPUT_SCHEMA="WeavelangClosureReceipt8@1"; CONTENT_TYPE="application/vnd.aurora.weavelang-closure-receipt-8+json"
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _canonical(values:Sequence[str])->bool:return tuple(values)==tuple(sorted(set(values)))
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class WeavelangClosureReceipt:
    request_id:str; federation_id:str; semantic_profile:str; disposition:str; case_order:tuple[str,...]; selected_order:tuple[str,...]; unresolved_order:tuple[str,...]; blocked_order:tuple[str,...]; missing_case_order:tuple[str,...]; peer_order:tuple[str,...]; qualified_peer_order:tuple[str,...]; missing_peer_order:tuple[str,...]; omission_order:tuple[str,...]; uncertainty_order:tuple[str,...]; negative_evidence_order:tuple[str,...]; replay_identity:str; closure_digest:str; artifact:dict[str,Any]; effect_receipts:tuple[str,...]; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version:str=CONTRACT_VERSION; feature_id:str=FEATURE_ID
    def to_dict(self)->dict[str,Any]:return {"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"federation_id":self.federation_id,"semantic_profile":self.semantic_profile,"disposition":self.disposition,"case_order":list(self.case_order),"selected_order":list(self.selected_order),"unresolved_order":list(self.unresolved_order),"blocked_order":list(self.blocked_order),"missing_case_order":list(self.missing_case_order),"peer_order":list(self.peer_order),"qualified_peer_order":list(self.qualified_peer_order),"missing_peer_order":list(self.missing_peer_order),"omission_order":list(self.omission_order),"uncertainty_order":list(self.uncertainty_order),"negative_evidence_order":list(self.negative_evidence_order),"replay_identity":self.replay_identity,"closure_digest":self.closure_digest,"artifact":self.artifact,"effect_receipts":list(self.effect_receipts),"raw_data_local":self.raw_data_local,"aggregate_only":self.aggregate_only,"boundary":self.boundary}
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not all(v.strip() for v in (self.request_id,self.federation_id,self.semantic_profile)) or not self.case_order or not self.peer_order or not self.effect_receipts or self.disposition not in {"qualified","unresolved","blocked"}:raise ResearchContractError("closure identity, locality, cases, peers, or effects are incomplete")
        for values in (self.case_order,self.selected_order,self.unresolved_order,self.blocked_order,self.missing_case_order,self.peer_order,self.qualified_peer_order,self.missing_peer_order,self.omission_order,self.uncertainty_order,self.negative_evidence_order,self.effect_receipts):
            if not _canonical(values):raise ResearchContractError("closure ordering is not canonical")
        parts=[*self.selected_order,*self.unresolved_order,*self.blocked_order]
        if set(parts)!=set(self.case_order) or len(parts)!=len(set(parts)):raise ResearchContractError("closure case states do not partition cases")
        peer_parts=[*self.qualified_peer_order,*self.missing_peer_order]
        if set(peer_parts)!=set(self.peer_order) or len(peer_parts)!=len(set(peer_parts)):raise ResearchContractError("closure peer states do not partition peers")
        if not all(_digest(v) for v in (self.replay_identity,self.closure_digest,self.artifact.get("content_hash"))):raise ResearchContractError("closure digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("closure artifact type is invalid")
        if self.disposition=="qualified" and self.effect_receipts!=(f"exchange:permitted-summaries:{self.request_id}",f"manage:local-capability:{self.request_id}"):raise ResearchContractError("qualified closure effects are invalid")
        if self.disposition!="qualified" and self.effect_receipts!=("block:unsafe-release",):raise ResearchContractError("non-qualified closure must block release")

def weavelang_limitation_closure_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"weavelang","consumers":["research workflow operator","institution node administrator","federation governance board"],"behavior":"operates typed WeaveLang limitation-closure attestations and digest-only peer summaries under explicit A2 authority, budget, policy, provenance, replay, and federation gates without executing WeaveLang programs","value":"prevents unresolved, unauthorized, semantically drifting, or over-budget limitation states from silently becoming an operated capability or federated release","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute_local_computation","write_local_artifact","federation_export"],"permissions":["operate:institution-node"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}

def assure_weavelang_limitation_closure(*,request_id:str,federation_id:str,semantic_profile:str,required_case_order:Sequence[str],cases:Sequence[Mapping[str,Any]],peers:Sequence[Mapping[str,Any]],minimum_peer_quorum:int,replay_identity:str,autonomy_grant:Mapping[str,Any],policy_receipt:Mapping[str,Any],federation_approved:bool,raw_data_local:bool,aggregate_only:bool,adversarial_events:Sequence[str]=(),boundary:str=PRECLINICAL_BOUNDARY)->WeavelangClosureReceipt:
    if not all(v.strip() for v in (request_id,federation_id,semantic_profile)) or not required_case_order or not _canonical(required_case_order) or not cases or not peers or minimum_peer_quorum<=0 or minimum_peer_quorum>len(peers) or not _digest(replay_identity) or not _canonical(adversarial_events) or not raw_data_local or not aggregate_only or boundary!=PRECLINICAL_BOUNDARY:raise ResearchContractError("closure request identity, ordering, quorum, replay, locality, or boundary is invalid")
    if str(autonomy_grant.get("schema_version"))!=RESEARCH_CONTRACT_SCHEMA_VERSION or str(autonomy_grant.get("autonomy_tier"))!="a2" or autonomy_grant.get("revoked") or not str(autonomy_grant.get("approval_reference","")).strip() or not {"manage:local-capability","exchange:permitted-summaries"}.issubset(set(autonomy_grant.get("permitted_actions",()))):raise ResearchContractError("A2 authority grant is incomplete")
    budget=float(dict(autonomy_grant.get("resource_budget",{})).get("research_units",0.0));
    if budget<0 or not budget==budget:raise ResearchContractError("research budget is invalid")
    if str(policy_receipt.get("schema_version"))!=RESEARCH_CONTRACT_SCHEMA_VERSION or str(policy_receipt.get("decision"))!="allow" or not policy_receipt.get("reasons"):raise ResearchContractError("policy receipt is incomplete")
    rows=[dict(x) for x in cases];seen:set[str]=set()
    for row in rows:
        cid=str(row.get("case_id",""));
        if not cid.strip() or cid in seen or not str(row.get("limitation_id","")).strip() or not str(row.get("capability_id","")).strip() or not str(row.get("institution_id","")).strip() or not str(row.get("semantic_profile","")).strip() or not all(_digest(row.get(k)) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")) or not _canonical(row.get("omission_order",())) or not _canonical(row.get("uncertainty_order",())):raise ResearchContractError(f"limitation case {cid} is malformed or duplicated")
        seen.add(cid)
    peer_rows=[dict(x) for x in peers];peer_seen:set[str]=set()
    for row in peer_rows:
        pid=str(row.get("institution_id",""));
        if not pid.strip() or pid in peer_seen or not _digest(row.get("closure_digest")) or not str(row.get("semantic_profile","")).strip() or not _digest(row.get("replay_identity")):raise ResearchContractError(f"peer {pid} is malformed or duplicated")
        peer_seen.add(pid)
    rows.sort(key=lambda x:str(x["case_id"]));order=tuple(str(x["case_id"]) for x in rows);required=set(required_case_order);selected:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set()
    for row in rows:
        cid=str(row["case_id"]); omissions.update(f"{cid}:{v}" for v in row.get("omission_order",())); uncertainty.update(f"{cid}:{v}" for v in row.get("uncertainty_order",()));
        if row.get("negative_result"):negative.add(f"{cid}:negative-result")
        if str(row.get("evidence_state",""))=="contradicted" or not row.get("local_only") or not row.get("permitted") or not row.get("operator_attested"):blocked.add(cid)
        elif int(row.get("resource_units",0))>budget:uncertainty.add(f"{cid}:resource-budget-exceeded");unresolved.add(cid)
        elif str(row["semantic_profile"])!=semantic_profile or str(row.get("replay_identity"))!=replay_identity or row.get("omission_order") or row.get("uncertainty_order") or str(row.get("evidence_state","")) not in {"proven","supported"}:unresolved.add(cid)
        else:selected.add(cid)
    missing=required-set(order);omissions.update(f"{cid}:required-case-missing" for cid in missing)
    peer_order=tuple(sorted(str(x["institution_id"]) for x in peer_rows));qualified_peer:set[str]=set();missing_peer:set[str]=set()
    for row in peer_rows:
        pid=str(row["institution_id"])
        if row.get("signed") and row.get("permitted") and row.get("aggregate_only") and str(row["semantic_profile"])==semantic_profile and str(row["replay_identity"])==replay_identity:qualified_peer.add(pid)
        else:missing_peer.add(pid)
    if len(qualified_peer)<minimum_peer_quorum:uncertainty.add("request:peer-quorum-incomplete")
    if not federation_approved:uncertainty.add("request:federation-approval-missing")
    if autonomy_grant.get("revoked"):negative.add("request:autonomy-grant-revoked")
    negative.update(f"adversarial:{event}" for event in adversarial_events);global_block=str(policy_receipt.get("decision"))!="allow" or bool(autonomy_grant.get("revoked")) or not str(autonomy_grant.get("approval_reference","")).strip() or not federation_approved or not raw_data_local or not aggregate_only or not {"manage:local-capability","exchange:permitted-summaries"}.issubset(set(autonomy_grant.get("permitted_actions",()))) or bool(adversarial_events)
    if global_block:blocked.update(order);selected.clear();unresolved.clear();omissions.add("request:limitation-closure-release-gate-blocked")
    disposition="blocked" if global_block else "qualified" if required.issubset(selected) and not missing and len(qualified_peer)>=minimum_peer_quorum and not unresolved and not blocked else "unresolved";so,uo,bo=tuple(sorted(selected)),tuple(sorted(unresolved)),tuple(sorted(blocked));mo=tuple(sorted(missing));qpo,mpo=tuple(sorted(qualified_peer)),tuple(sorted(missing_peer));oo,uu,ne=tuple(sorted(omissions)),tuple(sorted(uncertainty)),tuple(sorted(negative));effects=(f"exchange:permitted-summaries:{request_id}",f"manage:local-capability:{request_id}") if disposition=="qualified" else ("block:unsafe-release",);payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request_id,"federation_id":federation_id,"semantic_profile":semantic_profile,"disposition":disposition,"case_order":list(order),"selected_order":list(so),"unresolved_order":list(uo),"blocked_order":list(bo),"missing_case_order":list(mo),"peer_order":list(peer_order),"qualified_peer_order":list(qpo),"missing_peer_order":list(mpo),"omission_order":list(oo),"uncertainty_order":list(uu),"negative_evidence_order":list(ne),"replay_identity":replay_identity,"effect_receipts":list(effects),"raw_data_local":raw_data_local,"aggregate_only":aggregate_only,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"weavelang-closure:{request_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY};result=WeavelangClosureReceipt(request_id,federation_id,semantic_profile,disposition,order,so,uo,bo,mo,peer_order,qpo,mpo,oo,uu,ne,replay_identity,digest,artifact,effects);result.validate();return result
def weavelangLimitationClosureDigest(result:WeavelangClosureReceipt)->str:result.validate();return _hash(result.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","WeavelangClosureReceipt","weavelang_limitation_closure_manifest","assure_weavelang_limitation_closure","weavelangLimitationClosureDigest"]
