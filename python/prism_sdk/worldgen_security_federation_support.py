"""Deterministic Python parity for Worldgen P20 security/federation admission."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
FEATURE_ID="AFA-worldgen-P20-F01"; CONTRACT_VERSION="worldgen-local-security-federation/1.0"; INPUT_SCHEMA="SecurityFederationRequest1@1"; OUTPUT_SCHEMA="FederationEnvelope1@1"; CONTENT_TYPE="application/vnd.aurora.worldgen.security-federation-receipt-1+json"
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return values==sorted(set(values))
def _validate(request:Mapping[str,Any])->None:
    required=("request_id","consumer","purpose","origin","destination","policy_epoch","key_id")
    if request.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(request.get(k),str) or not request[k].strip() for k in required) or not _digest(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or not request.get("actions"):raise ResearchContractError("security request identity, replay, boundary, or action closure is invalid")
    ids:set[str]=set()
    for action in request["actions"]:
        if not isinstance(action.get("action_id"),str) or not action["action_id"].strip() or action["action_id"] in ids or not isinstance(action.get("actor"),str) or not action["actor"].strip() or not isinstance(action.get("source"),str) or not action["source"].strip() or not isinstance(action.get("destination"),str) or not action["destination"].strip() or not _ordered(action.get("effect_order",[])) or not _digest(action.get("artifact_digest")) or not _digest(action.get("provenance_digest")) or action.get("replay_identity")!=request["replay_identity"] or not isinstance(action.get("revocation_epoch"),str) or not action["revocation_epoch"].strip():raise ResearchContractError("action identity, effect ordering, digest, replay, or revocation epoch is invalid")
        ids.add(action["action_id"])
@dataclass(frozen=True)
class SignedFederationEnvelope1:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:validate_security_receipt(self.value,allow_feature_variants=True)
def manifest(*,feature_id:str,contract_version:str,scale:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["consortium security steward","federation operator","research program lead"],"behavior":f"classify signed aggregate federation actions for {scale} with key, authorization, locality, and evidence gates","value":"prevents unauthorized or raw-data export while preserving replayable security evidence","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["emit:federation-receipt","block:unauthorized-export"],"permissions":["read:local-research-artifact-metadata"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate_security_receipt(output:Mapping[str,Any],*,allow_feature_variants:bool=False)->None:
    artifact=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or (not allow_feature_variants and output.get("contract_version")!=CONTRACT_VERSION) or (not allow_feature_variants and output.get("feature_id")!=FEATURE_ID) or output.get("boundary")!=PRECLINICAL_BOUNDARY or artifact.get("boundary")!=PRECLINICAL_BOUNDARY or artifact.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"admitted","local_only","blocked","unresolved"} or not output.get("action_order") or any(not _digest(output.get(k)) for k in ("replay_identity","federation_digest")) or artifact.get("content_hash")!=output.get("federation_digest"):raise ResearchContractError("security receipt identity, locality, digest, or disposition is incomplete")
    fields=("action_order","admitted_order","local_only_order","denied_order","unresolved_order","omission_order","threat_order","revocation_order")
    if any(not _ordered(output.get(k,[])) for k in fields):raise ResearchContractError("security receipt vectors are not canonical")
    ids=set(output["action_order"]);parts=sum((output.get(k,[]) for k in ("admitted_order","local_only_order","denied_order","unresolved_order")),[])
    if len(ids)!=len(output["action_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("security action states do not partition")
    if any(not _digest(value) for value in artifact.get("provenance_digests",[])):raise ResearchContractError("security provenance digest is invalid")
def qualify(request:Mapping[str,Any],*,feature_id:str=FEATURE_ID,contract_version:str=CONTRACT_VERSION)->SignedFederationEnvelope1:
    _validate(request);rows=sorted((dict(a) for a in request["actions"]),key=lambda a:a["action_id"]);order=[a["action_id"] for a in rows];admitted:set[str]=set();local_only:set[str]=set();denied:set[str]=set();unresolved:set[str]=set();omissions:set[str]=set();threats:set[str]=set();revocations:set[str]=set()
    for action in rows:
        action_id=action["action_id"]
        if action.get("revocation_epoch")!=request["policy_epoch"] or not action.get("key_active"):revocations.add(f"{action_id}:key-revoked-or-stale");denied.add(action_id)
        elif not action.get("authorized"):threats.add(f"{action_id}:authorization-missing");denied.add(action_id)
        elif action.get("evidence_state") in {"unknown","speculative","contradicted"}:unresolved.add(action_id);omissions.add(f"{action_id}:evidence-not-closed")
        elif not action.get("export_requested") or action.get("destination")==request["origin"]:local_only.add(action_id)
        elif action.get("source")!=request["origin"] or action.get("destination")!=request["destination"]:threats.add(f"{action_id}:route-outside-declaration");denied.add(action_id)
        else:admitted.add(action_id)
    if not request.get("protected_closure"):omissions.add("request:protected-closure-incomplete")
    if not request.get("raw_data_local"):threats.add("request:raw-data-locality-false")
    if not request.get("aggregate_only"):threats.add("request:aggregate-only-false")
    if request.get("federation_requested") and not request.get("federation_authorized"):threats.add("request:federation-authorization-missing")
    global_block=not all(request.get(k) is True for k in ("protected_closure","raw_data_local","aggregate_only")) or (request.get("federation_requested") and not request.get("federation_authorized"))
    disposition="blocked" if global_block or denied else "unresolved" if unresolved else "admitted" if admitted else "local_only"
    if global_block:denied.update(order);admitted.clear();local_only.clear();unresolved.clear();omissions.add("request:export-closure-not-ready")
    payload={"action_order":order,"admitted_order":sorted(admitted),"local_only_order":sorted(local_only),"denied_order":sorted(denied),"unresolved_order":sorted(unresolved),"omission_order":sorted(omissions),"threat_order":sorted(threats),"revocation_order":sorted(revocations),"replay_identity":request["replay_identity"],"origin":request["origin"],"destination":request["destination"],"policy_epoch":request["policy_epoch"]}
    digest=_hash(payload);output={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"consumer":request["consumer"],"purpose":request["purpose"],"origin":request["origin"],"destination":request["destination"],"policy_epoch":request["policy_epoch"],"disposition":disposition,**payload,"federation_digest":digest,"artifact":{"artifact_id":f"worldgen-security-federation:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"provenance_digests":sorted({a["provenance_digest"] for a in rows}),"semantic_loss":[] if disposition=="admitted" else ["export-not-executed"],"boundary":PRECLINICAL_BOUNDARY},"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};validate_security_receipt(output,allow_feature_variants=True);return SignedFederationEnvelope1(output)
SecurityFederationRequest1=dict[str,Any];SecurityFederationAction1=dict[str,Any];SecurityFederationEvidenceState=str;SecurityFederationError1=ResearchContractError
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","SignedFederationEnvelope1","SecurityFederationRequest1","SecurityFederationAction1","SecurityFederationEvidenceState","SecurityFederationError1","manifest","qualify","validate_security_receipt"]

