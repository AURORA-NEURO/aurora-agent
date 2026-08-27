"""Python parity surface for ``AFA-oracle-P03-F32``."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-oracle-P03-F32"
CONTRACT_VERSION = "oracle-federated-continual-context-control-plane/1.0"
INPUT_SCHEMA = "DecisionQuery4@1"
OUTPUT_SCHEMA = "CertifiedDecisionSection8@1"
CONTENT_TYPE = "application/vnd.aurora.certified-decision-section+json"
CHECKPOINTS = ("admit-typed-query", "check-scope-and-evidence", "check-provenance-and-replay", "check-policy-and-federation", "retain-omission-and-negative-receipt")


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


@dataclass(frozen=True)
class OracleContextFederationEnvelope:
    request_id: str; workflow_id: str; target_schema: str; purpose: str; semantic_profile: str; disposition: str
    claim_order: tuple[str, ...]; ranked_order: tuple[str, ...]; selected_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]
    missing_claim_order: tuple[str, ...]; missing_scope_order: tuple[str, ...]; omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; contradiction_order: tuple[str, ...]; negative_evidence_order: tuple[str, ...]; adversarial_event_order: tuple[str, ...]; checkpoint_order: tuple[str, ...]
    replay_identity: str; context_digest: str; artifact: dict[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool = True; aggregate_only: bool = True; boundary: str = PRECLINICAL_BOUNDARY
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version: str = CONTRACT_VERSION; feature_id: str = FEATURE_ID

    def to_dict(self) -> dict[str, Any]:
        return {"schema_version":self.schema_version,"contract_version":self.contract_version,"feature_id":self.feature_id,"request_id":self.request_id,"workflow_id":self.workflow_id,"target_schema":self.target_schema,"purpose":self.purpose,"semantic_profile":self.semantic_profile,"disposition":self.disposition,"claim_order":list(self.claim_order),"ranked_order":list(self.ranked_order),"selected_order":list(self.selected_order),"unresolved_order":list(self.unresolved_order),"blocked_order":list(self.blocked_order),"missing_claim_order":list(self.missing_claim_order),"missing_scope_order":list(self.missing_scope_order),"omission_order":list(self.omission_order),"uncertainty_order":list(self.uncertainty_order),"contradiction_order":list(self.contradiction_order),"negative_evidence_order":list(self.negative_evidence_order),"adversarial_event_order":list(self.adversarial_event_order),"checkpoint_order":list(self.checkpoint_order),"replay_identity":self.replay_identity,"context_digest":self.context_digest,"artifact":self.artifact,"effect_receipts":list(self.effect_receipts),"raw_data_local":self.raw_data_local,"aggregate_only":self.aggregate_only,"boundary":self.boundary}

    def validate(self) -> None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or self.artifact.get("boundary")!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not all(v.strip() for v in (self.request_id,self.workflow_id,self.target_schema,self.purpose,self.semantic_profile)) or not self.claim_order or len(self.ranked_order)!=len(self.claim_order) or tuple(self.checkpoint_order)!=CHECKPOINTS or not self.effect_receipts: raise ResearchContractError("federated context identity, locality, checkpoints, ranking, or effects are incomplete")
        for values in (self.claim_order,self.selected_order,self.unresolved_order,self.blocked_order,self.missing_claim_order,self.missing_scope_order,self.omission_order,self.uncertainty_order,self.contradiction_order,self.negative_evidence_order,self.adversarial_event_order,self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("federated context ordering is not canonical")
        ids=set(self.claim_order); parts=self.selected_order+self.unresolved_order+self.blocked_order
        if len(parts)!=len(ids) or any(v not in ids for v in parts) or len(set(parts))!=len(parts) or set(self.ranked_order)!=ids: raise ResearchContractError("federated context states do not partition claims")
        for value in (self.replay_identity,self.context_digest,self.artifact.get("content_hash")):
            if not _digest(value): raise ResearchContractError("federated context digest is invalid")
        if self.artifact.get("content_type")!=CONTENT_TYPE: raise ResearchContractError("federated context artifact type is invalid")
        if self.disposition=="qualified" and self.effect_receipts!=(f"exchange:permitted-summary:{self.request_id}",f"manage:local-capability:{self.request_id}"): raise ResearchContractError("qualified context effects are invalid")
        if self.disposition!="qualified" and self.effect_receipts!=("block:unsafe-release",): raise ResearchContractError("non-qualified federated context must block release")


def operate_context_federation(*, request_id:str, workflow_id:str, target_schema:str, purpose:str, semantic_profile:str, required_claim_order:Sequence[str], required_scope_order:Sequence[str], claims:Sequence[Mapping[str,Any]], replay_identity:str, policy_allow:bool, protected_closure:bool, signed_approval:bool, federation_approved:bool, raw_data_local:bool, aggregate_only:bool, budget:int, max_budget:int, adversarial_events:Sequence[str]=(), boundary:str=PRECLINICAL_BOUNDARY) -> OracleContextFederationEnvelope:
    if not all(v.strip() for v in (request_id,workflow_id,target_schema,purpose,semantic_profile)) or not required_claim_order or not required_scope_order or not claims or not _canonical(required_claim_order) or not _canonical(required_scope_order) or not _canonical(adversarial_events) or not _digest(replay_identity) or budget<=0 or max_budget<=0 or boundary!=PRECLINICAL_BOUNDARY or not raw_data_local or not aggregate_only: raise ResearchContractError("query identity, closure, digest, budget, locality, or boundary is invalid")
    rows=[dict(row) for row in claims]; seen:set[str]=set()
    for row in rows:
        identifier=str(row.get("claim_id",""))
        if not identifier.strip() or identifier in seen or not str(row.get("proposition","")).strip() or not str(row.get("scope","")).strip() or not 0<=int(row.get("influence_milli",-1))<=1000 or not _digest(row.get("replay_identity")) or not str(row.get("semantic_profile","")).strip() or not _canonical(row.get("omissions",())) or not _canonical(row.get("uncertainty",())): raise ResearchContractError(f"claim {identifier} is malformed or duplicated")
        if row.get("evidence_digest") is not None and not _digest(row["evidence_digest"]): raise ResearchContractError(f"claim {identifier} evidence digest is invalid")
        if row.get("provenance_digest") is not None and not _digest(row["provenance_digest"]): raise ResearchContractError(f"claim {identifier} provenance digest is invalid")
        seen.add(identifier)
    rows.sort(key=lambda row:(-int(row["influence_milli"]),str(row["claim_id"]))); ranked=tuple(str(row["claim_id"]) for row in rows); order=tuple(sorted(ranked)); required=set(required_claim_order); missing=tuple(sorted(required-set(order))); scopes=set(required_scope_order); selected:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); contradiction:set[str]=set(); negative:set[str]=set()
    for row in rows:
        identifier=str(row["claim_id"]); omissions.update(f"{identifier}:{x}" for x in row.get("omissions",())); uncertainty.update(f"{identifier}:{x}" for x in row.get("uncertainty",())); negative.update(f"{identifier}:{x}" for x in row.get("negative_evidence",()))
        if row.get("negative_result"): negative.add(f"{identifier}:negative-result")
        state=str(row.get("evidence_state",""))
        if state=="contradicted": blocked.add(identifier); contradiction.add(f"{identifier}:contradicted-evidence"); continue
        if state in {"unknown","speculative"}: unresolved.add(identifier); uncertainty.add(f"{identifier}:evidence-unresolved"); continue
        factors=bool(row.get("evidence_digest")) and bool(row.get("provenance_digest")); complete=bool(str(row.get("proposition","")).strip()) and str(row.get("scope")) in scopes and factors and row.get("semantic_profile")==semantic_profile and row.get("replay_identity")==replay_identity and not row.get("omissions") and not row.get("uncertainty") and bool(row.get("local_data")) and bool(row.get("permitted")) and int(row["influence_milli"])>=700
        if complete and state in {"proven","supported"}: selected.add(identifier)
        else:
            unresolved.add(identifier)
            if not factors: omissions.add(f"{identifier}:evidence-or-provenance-missing")
            if str(row.get("scope")) not in scopes: omissions.add(f"{identifier}:required-scope-missing")
            if int(row["influence_milli"])<700: uncertainty.add(f"{identifier}:influence-threshold-not-met")
            if not row.get("local_data") or not row.get("permitted"): blocked.add(identifier); unresolved.discard(identifier); omissions.add(f"{identifier}:locality-or-permission-denied")
    omissions.update(f"{identifier}:required-claim-missing" for identifier in missing); missing_scope=tuple(sorted(scope for scope in required_scope_order if not any(str(row.get("scope"))==scope for row in rows))); omissions.update(f"required-scope-missing:{scope}" for scope in missing_scope); negative.update(f"adversarial:{event}" for event in adversarial_events)
    if not policy_allow: uncertainty.add("request:policy-denied")
    if not protected_closure: uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval or not federation_approved: uncertainty.add("request:institutional-approval-incomplete")
    if budget>max_budget: omissions.add("request:budget-ceiling-exceeded")
    global_block=not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or budget>max_budget or bool(adversarial_events); disposition="blocked" if global_block else ("qualified" if not missing and not missing_scope and selected and not unresolved and not blocked else "unresolved")
    selected_order=tuple(sorted(selected)); unresolved_order=tuple(sorted(unresolved)); blocked_order=tuple(sorted(blocked)); omission_order=tuple(sorted(omissions)); uncertainty_order=tuple(sorted(uncertainty)); contradiction_order=tuple(sorted(contradiction)); negative_order=tuple(sorted(negative)); checkpoint_order=CHECKPOINTS; effects=(f"exchange:permitted-summary:{request_id}",f"manage:local-capability:{request_id}") if disposition=="qualified" else ("block:unsafe-release",)
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request_id,"workflow_id":workflow_id,"target_schema":target_schema,"purpose":purpose,"semantic_profile":semantic_profile,"disposition":disposition,"claim_order":list(order),"ranked_order":list(ranked),"selected_order":list(selected_order),"unresolved_order":list(unresolved_order),"blocked_order":list(blocked_order),"missing_claim_order":list(missing),"missing_scope_order":list(missing_scope),"omission_order":list(omission_order),"uncertainty_order":list(uncertainty_order),"contradiction_order":list(contradiction_order),"negative_evidence_order":list(negative_order),"adversarial_event_order":list(adversarial_events),"checkpoint_order":list(checkpoint_order),"replay_identity":replay_identity,"effect_receipts":list(effects),"raw_data_local":raw_data_local,"aggregate_only":aggregate_only,"boundary":PRECLINICAL_BOUNDARY}; digest_value=_hash(payload); artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"oracle-certified-context:{request_id}","content_type":CONTENT_TYPE,"content_hash":digest_value,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY}
    result=OracleContextFederationEnvelope(request_id,workflow_id,target_schema,purpose,semantic_profile,disposition,order,ranked,selected_order,unresolved_order,blocked_order,missing,missing_scope,omission_order,uncertainty_order,contradiction_order,negative_order,tuple(adversarial_events),checkpoint_order,replay_identity,digest_value,artifact,effects,raw_data_local,aggregate_only); result.validate(); return result


def oracle_context_federation_digest(result:OracleContextFederationEnvelope)->str: result.validate(); return _hash(result.to_dict())

__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","OracleContextFederationEnvelope","operate_context_federation","oracle_context_federation_digest"]
