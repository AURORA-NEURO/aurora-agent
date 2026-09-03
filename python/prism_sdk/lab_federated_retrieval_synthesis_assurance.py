"""Python parity surface for ``AFA-lab-P02-F28``."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-lab-P02-F28"; CONTRACT_VERSION="lab-federated-continual-retrieval-synthesis-assurance-harness/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery4@1"; OUTPUT_SCHEMA="EvidenceSynthesis7@1"; CONTENT_TYPE="application/vnd.aurora.evidence-synthesis-7+json"
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _canonical(values:Sequence[str])->bool:return tuple(values)==tuple(sorted(set(values)))
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class EvidenceSynthesis:
    query_id:str; federation_id:str; semantic_profile:str; disposition:str; evidence_order:tuple[str,...]; selected_order:tuple[str,...]; unresolved_order:tuple[str,...]; blocked_order:tuple[str,...]; missing_evidence_order:tuple[str,...]; stale_order:tuple[str,...]; missing_scope_order:tuple[str,...]; peer_order:tuple[str,...]; qualified_peer_order:tuple[str,...]; missing_peer_order:tuple[str,...]; omission_order:tuple[str,...]; uncertainty_order:tuple[str,...]; contradiction_order:tuple[str,...]; negative_evidence_order:tuple[str,...]; replay_identity:str; synthesis_digest:str; artifact:dict[str,Any]; effect_receipts:tuple[str,...]; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version:str=CONTRACT_VERSION; feature_id:str=FEATURE_ID
    def to_dict(self)->dict[str,Any]:return {"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"query_id":self.query_id,"federation_id":self.federation_id,"semantic_profile":self.semantic_profile,"disposition":self.disposition,"evidence_order":list(self.evidence_order),"selected_order":list(self.selected_order),"unresolved_order":list(self.unresolved_order),"blocked_order":list(self.blocked_order),"missing_evidence_order":list(self.missing_evidence_order),"stale_order":list(self.stale_order),"missing_scope_order":list(self.missing_scope_order),"peer_order":list(self.peer_order),"qualified_peer_order":list(self.qualified_peer_order),"missing_peer_order":list(self.missing_peer_order),"omission_order":list(self.omission_order),"uncertainty_order":list(self.uncertainty_order),"contradiction_order":list(self.contradiction_order),"negative_evidence_order":list(self.negative_evidence_order),"replay_identity":self.replay_identity,"synthesis_digest":self.synthesis_digest,"artifact":self.artifact,"effect_receipts":list(self.effect_receipts),"raw_data_local":self.raw_data_local,"aggregate_only":self.aggregate_only,"boundary":self.boundary}
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not all(v.strip() for v in (self.query_id,self.federation_id,self.semantic_profile)) or not self.evidence_order or not self.peer_order or not self.effect_receipts or self.disposition not in {"qualified","unresolved","blocked"}:raise ResearchContractError("synthesis identity, locality, evidence, peers, or effects are incomplete")
        for values in (self.evidence_order,self.selected_order,self.unresolved_order,self.blocked_order,self.missing_evidence_order,self.stale_order,self.missing_scope_order,self.peer_order,self.qualified_peer_order,self.missing_peer_order,self.omission_order,self.uncertainty_order,self.contradiction_order,self.negative_evidence_order,self.effect_receipts):
            if not _canonical(values):raise ResearchContractError("synthesis ordering is not canonical")
        parts=[*self.selected_order,*self.unresolved_order,*self.blocked_order]
        if set(parts)!=set(self.evidence_order) or len(parts)!=len(set(parts)):raise ResearchContractError("synthesis evidence states do not partition candidates")
        peers=[*self.qualified_peer_order,*self.missing_peer_order]
        if set(peers)!=set(self.peer_order) or len(peers)!=len(set(peers)):raise ResearchContractError("synthesis peer states do not partition peers")
        if not all(_digest(v) for v in (self.replay_identity,self.synthesis_digest,self.artifact.get("content_hash"))):raise ResearchContractError("synthesis digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("synthesis artifact type is invalid")
        if self.disposition=="qualified" and self.effect_receipts!=(f"verify:evidence-synthesis:{self.query_id}",):raise ResearchContractError("qualified synthesis effect is invalid")
        if self.disposition!="qualified" and self.effect_receipts!=("block:unsafe-release",):raise ResearchContractError("non-qualified synthesis must block release")

def federated_retrieval_synthesis_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"lab","consumers":["bioinformatician","research data steward","consortium operator"],"behavior":"verifies local retrieval attestations and aggregate-only peer synthesis summaries under explicit evidence, freshness, scope, provenance, replay, and policy gates without performing retrieval","value":"prevents stale, incomparable, contradictory, or unauthorized evidence from silently becoming a federated synthesis","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["read_local_data","execute_local_computation","write_local_artifact","federation_export"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def assure_federated_retrieval_synthesis(*,query_id:str,federation_id:str,semantic_profile:str,required_evidence_order:Sequence[str],required_scope_order:Sequence[str],minimum_freshness_epoch:int,candidates:Sequence[Mapping[str,Any]],peers:Sequence[Mapping[str,Any]],minimum_peer_quorum:int,replay_identity:str,policy_allow:bool,protected_closure:bool,signed_approval:bool,federation_approved:bool,raw_data_local:bool,aggregate_only:bool,adversarial_events:Sequence[str]=(),boundary:str=PRECLINICAL_BOUNDARY)->EvidenceSynthesis:
    if not all(v.strip() for v in (query_id,federation_id,semantic_profile)) or not required_evidence_order or not _canonical(required_evidence_order) or not required_scope_order or not _canonical(required_scope_order) or minimum_freshness_epoch<=0 or not _digest(replay_identity) or not candidates or not peers or minimum_peer_quorum<=0 or not _canonical(adversarial_events) or not raw_data_local or not aggregate_only or boundary!=PRECLINICAL_BOUNDARY:raise ResearchContractError("retrieval query identity, ordering, quorum, locality, replay, or boundary is invalid")
    rows=[dict(x) for x in candidates];seen:set[str]=set()
    for row in rows:
        eid=str(row.get("evidence_id",""));
        if not eid.strip() or eid in seen or not str(row.get("study_id","")).strip() or not str(row.get("source_id","")).strip() or not str(row.get("scope","")).strip() or not str(row.get("semantic_profile","")).strip() or int(row.get("freshness_epoch",0))<=0 or not all(_digest(row.get(k)) for k in ("content_digest","provenance_digest","replay_identity")) or not _canonical(row.get("omissions",())) or not _canonical(row.get("uncertainty",())):raise ResearchContractError(f"candidate {eid} is malformed or duplicated")
        seen.add(eid)
    peer_rows=[dict(x) for x in peers];peer_seen:set[str]=set()
    for row in peer_rows:
        pid=str(row.get("institution_id",""));
        if not pid.strip() or pid in peer_seen or not _digest(row.get("evidence_digest")) or not _digest(row.get("replay_identity")) or not str(row.get("semantic_profile","")).strip():raise ResearchContractError(f"peer {pid} is malformed or duplicated")
        peer_seen.add(pid)
    rows.sort(key=lambda x:str(x["evidence_id"])); order=tuple(str(x["evidence_id"]) for x in rows); required=set(required_evidence_order); scopes=set(required_scope_order); selected:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); stale:set[str]=set(); missing_scope:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); contradiction:set[str]=set(); negative:set[str]=set()
    covered_scopes={str(row["scope"]) for row in rows if str(row["scope"]) in scopes}
    missing_scope.update(scopes-covered_scopes)
    for row in rows:
        eid=str(row["evidence_id"]); omissions.update(f"{eid}:{v}" for v in row.get("omissions",())); uncertainty.update(f"{eid}:{v}" for v in row.get("uncertainty",()));
        if row.get("negative_result"):negative.add(f"{eid}:negative-result")
        if int(row["freshness_epoch"])<minimum_freshness_epoch:stale.add(eid);unresolved.add(eid)
        elif str(row.get("evidence_state",""))=="contradicted":contradiction.add(eid);blocked.add(eid)
        elif not row.get("local_only") or not row.get("permitted"):blocked.add(eid)
        elif str(row["semantic_profile"])!=semantic_profile or str(row.get("replay_identity"))!=replay_identity or int(row.get("relevance_milli",0))<600 or row.get("omissions") or row.get("uncertainty") or str(row.get("evidence_state","")) not in {"proven","supported"}:unresolved.add(eid)
        elif str(row["scope"]) not in scopes:missing_scope.add(str(row["scope"]));unresolved.add(eid)
        else:selected.add(eid)
    missing=required-set(order);omissions.update(f"{eid}:required-evidence-missing" for eid in missing);omissions.update(f"required-scope-missing:{scope}" for scope in missing_scope)
    peer_order=tuple(sorted(str(row["institution_id"]) for row in peer_rows));qualified_peer:set[str]=set();missing_peer:set[str]=set()
    for row in peer_rows:
        pid=str(row["institution_id"])
        if row.get("signed") and row.get("permitted") and row.get("aggregate_only") and str(row["semantic_profile"])==semantic_profile and str(row["replay_identity"])==replay_identity:qualified_peer.add(pid)
        else:missing_peer.add(pid)
    if len(qualified_peer)<minimum_peer_quorum:uncertainty.add("request:peer-quorum-incomplete")
    if not policy_allow:negative.add("request:policy-denied")
    if not protected_closure:uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval:uncertainty.add("request:signed-approval-missing")
    if not federation_approved:uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{event}" for event in adversarial_events);global_block=not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or bool(adversarial_events)
    if global_block:blocked.update(order);selected.clear();unresolved.clear();omissions.add("request:synthesis-release-gate-blocked")
    disposition="blocked" if global_block else "qualified" if required.issubset(selected) and not missing_scope and len(qualified_peer)>=minimum_peer_quorum and not unresolved and not blocked else "unresolved";so,uo,bo=tuple(sorted(selected)),tuple(sorted(unresolved)),tuple(sorted(blocked));mo,sto,ms=tuple(sorted(missing)),tuple(sorted(stale)),tuple(sorted(missing_scope));qpo,mpo=tuple(sorted(qualified_peer)),tuple(sorted(missing_peer));oo,uu,co,ne=tuple(sorted(omissions)),tuple(sorted(uncertainty)),tuple(sorted(contradiction)),tuple(sorted(negative));effects=(f"verify:evidence-synthesis:{query_id}",) if disposition=="qualified" else ("block:unsafe-release",);payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"query_id":query_id,"federation_id":federation_id,"semantic_profile":semantic_profile,"disposition":disposition,"evidence_order":list(order),"selected_order":list(so),"unresolved_order":list(uo),"blocked_order":list(bo),"missing_evidence_order":list(mo),"stale_order":list(sto),"missing_scope_order":list(ms),"peer_order":list(peer_order),"qualified_peer_order":list(qpo),"missing_peer_order":list(mpo),"omission_order":list(oo),"uncertainty_order":list(uu),"contradiction_order":list(co),"negative_evidence_order":list(ne),"replay_identity":replay_identity,"effect_receipts":list(effects),"raw_data_local":raw_data_local,"aggregate_only":aggregate_only,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"evidence-synthesis:{query_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY};result=EvidenceSynthesis(query_id,federation_id,semantic_profile,disposition,order,so,uo,bo,mo,sto,ms,peer_order,qpo,mpo,oo,uu,co,ne,replay_identity,digest,artifact,effects);result.validate();return result
def labFederatedRetrievalSynthesisDigest(result:EvidenceSynthesis)->str:result.validate();return _hash(result.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","EvidenceSynthesis","federated_retrieval_synthesis_manifest","assure_federated_retrieval_synthesis","labFederatedRetrievalSynthesisDigest"]
