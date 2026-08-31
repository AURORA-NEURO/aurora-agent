"""Python parity for Worldgen P20 security/federation contract negotiation."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
CONTENT_TYPE="application/vnd.aurora.worldgen.security-federation-contract-receipt+json"
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return values==sorted(set(values))
def manifest(*,feature_id:str,contract_version:str,scale:str)->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["security steward","federation operator","developer"],"behavior":f"negotiate signed security/federation fields for {scale}","value":"makes schema compatibility, redaction, key state, and locality explicit before exchange","input_schema":"SecurityFederationContractRequest1@1","output_schema":"SecurityFederationContractReceipt1@1","effects":["none:security-contract-validation","block:unsafe-export"],"permissions":["negotiate:federation-contract"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate_contract(output:Mapping[str,Any])->None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or not output.get("field_order") or not _digest(output.get("replay_identity")) or not _digest(output.get("contract_digest")) or a.get("content_hash")!=output.get("contract_digest"):raise ResearchContractError("security contract identity, locality, or digest is incomplete")
    for key in ("field_order","retained_field_order","missing_field_order","redacted_field_order","security_issue_order","effect_receipts"):
        if not _ordered(output.get(key,[])):raise ResearchContractError("security contract vectors are not canonical")
    fields=set(output["field_order"]);represented=set(output.get("retained_field_order",[]))|set(output.get("missing_field_order",[]))|set(output.get("redacted_field_order",[]))
    if fields!=represented:raise ResearchContractError("security contract fields do not partition")
def negotiate(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str)->dict[str,Any]:
    if not all(isinstance(request.get(k),str) and request[k].strip() for k in ("request_id","consumer","producer")) or not request.get("field_order") or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _digest(request.get("replay_identity")):raise ResearchContractError("security contract request is invalid")
    fields=set(request["field_order"]);retained=set(request.get("retained_field_order",[]))&fields;missing=fields-retained;redacted=set(request.get("missing_field_order",[]))&fields;issues=set()
    if not request.get("policy_allow"):issues.add("policy-denied")
    if not request.get("protected_closure"):issues.add("protected-closure-incomplete")
    if not request.get("federation_authorized"):issues.add("federation-authorization-missing")
    if not request.get("key_active"):issues.add("signing-key-inactive")
    disposition="blocked" if issues else "unresolved" if not retained else "compatible" if not missing and not redacted else "partial"
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"consumer":request["consumer"],"producer":request["producer"],"namespace":request.get("namespace",""),"semantic_profile":request.get("semantic_profile",""),"negotiated_version":request.get("negotiated_version",""),"compatibility":"compatible" if disposition=="compatible" else "redacted-migration","disposition":disposition,"field_order":sorted(fields),"retained_field_order":sorted(retained),"missing_field_order":sorted(missing),"redacted_field_order":sorted(redacted),"security_issue_order":sorted(issues),"replay_identity":request["replay_identity"],"effect_receipts":["none:security-contract-validation"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);payload.update({"contract_digest":digest,"artifact":{"content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}});validate_contract(payload);return payload
__all__=["CONTENT_TYPE","manifest","negotiate","validate_contract"]

