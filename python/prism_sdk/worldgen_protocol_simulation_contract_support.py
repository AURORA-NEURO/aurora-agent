"""Versioned protocol simulation contract negotiation for Worldgen P08 F05-F08."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.protocol_simulation-contract-receipt+json"
@dataclass(frozen=True)
class ProtocolContractRequest:
    request_id:str; consumer:str; producer:str; namespace:str; semantic_profile:str; negotiated_version:str; field_order:tuple[str,...]; retained_field_order:tuple[str,...]; missing_field_order:tuple[str,...]; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class ProtocolContractReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("request_id") and v.get("consumer") and v.get("producer") and v.get("field_order") and v.get("effect_receipts")==["none:protocol_simulation-contract-validation"] and all(isinstance(v.get(k),str) and len(v[k])==64 and all(c in "0123456789abcdef" for c in v[k]) for k in ("replay_identity","contract_digest")) and a.get("content_hash")==v.get("contract_digest")): raise ResearchContractError("protocol simulation contract identity, fields, locality, digests, or effects are incomplete")
        for key in ("field_order","retained_field_order","missing_field_order","omitted_field_order","semantic_loss_order","effect_receipts"):
            vals=tuple(v.get(key,()))
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("protocol simulation contract vectors are not canonical")
        fields=set(v["field_order"]); parts=set(v.get("retained_field_order",()))|set(v.get("missing_field_order",()))|set(v.get("omitted_field_order",()))
        if len(fields)!=len(v["field_order"]) or parts!=fields: raise ResearchContractError("protocol simulation contract fields do not partition")
    def digest(self)->str: self.validate(); return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["protocol_simulation steward","preclinical researcher","downstream protocol_simulation consumer"],"behavior":f"negotiate a versioned typed protocol simulation contract for {scale}","value":"makes protocol_simulation schema compatibility, semantic loss, locality, and permissions explicit before reuse","input_schema":input_schema,"output_schema":"ProtocolContractReceipt1@1","effects":["none:protocol_simulation-contract-validation","block:unsafe-release"],"permissions":["negotiate:protocol_simulation-contract"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def negotiate(request:ProtocolContractRequest,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->ProtocolContractReceipt:
    if not(request.request_id.strip() and request.consumer.strip() and request.producer.strip() and request.namespace.strip() and request.semantic_profile.strip() and request.negotiated_version.strip() and request.field_order and tuple(request.field_order)==tuple(sorted(set(request.field_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and len(request.replay_identity)==64 and all(c in "0123456789abcdef" for c in request.replay_identity) and (not require_federation or request.policy_allow)): raise ResearchContractError("protocol simulation contract request is invalid")
    fields=set(request.field_order); retained=fields&set(request.retained_field_order); missing=fields-retained; omitted=fields&set(request.missing_field_order); loss=missing|omitted; compatible=not missing and not omitted and request.protected_closure
    disposition="blocked" if not request.policy_allow or not request.protected_closure else "compatible" if compatible else "unknown" if not retained else "partial"; compatibility="compatible" if compatible else "unknown" if not retained else "additive_migration"
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"consumer":request.consumer,"producer":request.producer,"namespace":request.namespace,"semantic_profile":request.semantic_profile,"negotiated_version":request.negotiated_version,"compatibility":compatibility,"disposition":disposition,"field_order":sorted(fields),"retained_field_order":sorted(retained),"missing_field_order":sorted(missing),"omitted_field_order":sorted(omitted),"semantic_loss_order":sorted(loss),"replay_identity":request.replay_identity,"effect_receipts":["none:protocol_simulation-contract-validation"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    d=research_artifact_digest(payload); payload["contract_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-protocol_simulation-contract:{request.request_id}","content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}; receipt=ProtocolContractReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ProtocolContractRequest","ProtocolContractReceipt","manifest","negotiate"]


