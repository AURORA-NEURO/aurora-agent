"""Versioned typed knowledge-contract negotiation for Worldgen P04 F05-F08."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTENT_TYPE = "application/vnd.aurora.worldgen.knowledge-contract-receipt+json"

@dataclass(frozen=True)
class KnowledgeContractRequest:
    request_id: str
    consumer: str
    producer: str
    namespace: str
    semantic_profile: str
    negotiated_version: str
    field_order: tuple[str, ...]
    retained_field_order: tuple[str, ...]
    missing_field_order: tuple[str, ...] = ()
    replay_identity: str = ""
    policy_allow: bool = True
    protected_closure: bool = True
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class KnowledgeContractReceipt:
    value: dict[str, Any]
    def validate(self) -> None:
        v, artifact = self.value, self.value.get("artifact", {})
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or
            v.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or
            artifact.get("content_type") != CONTENT_TYPE or not v.get("raw_data_local") or not v.get("aggregate_only") or
            not v.get("request_id") or not v.get("consumer") or not v.get("producer") or not v.get("namespace") or
            not v.get("semantic_profile") or not v.get("negotiated_version") or not v.get("field_order") or
            v.get("effect_receipts") != ["none:knowledge-contract-validation"]):
            raise ResearchContractError("knowledge contract identity, fields, locality, or effects are incomplete")
        for key in ("field_order", "retained_field_order", "missing_field_order", "omitted_field_order", "semantic_loss_order", "effect_receipts"):
            values = tuple(v.get(key, ()))
            if values != tuple(sorted(set(values))):
                raise ResearchContractError("knowledge contract ordering is not canonical")
        fields, retained, missing, omitted = set(v["field_order"]), set(v.get("retained_field_order", ())), set(v.get("missing_field_order", ())), set(v.get("omitted_field_order", ()))
        if len(fields) != len(v["field_order"]) or retained | missing | omitted != fields or len(retained) + len(missing) + len(omitted) != len(fields):
            raise ResearchContractError("knowledge contract fields do not partition")
        for value in (v.get("replay_identity"), v.get("contract_digest"), artifact.get("content_hash")):
            if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
                raise ResearchContractError("knowledge contract digest is invalid")
        if artifact.get("content_hash") != v.get("contract_digest"):
            raise ResearchContractError("knowledge contract artifact digest is inconsistent")
    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(self.value)

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version,
            "owner_crate": "worldgen", "consumers": ["schema steward", "knowledge engineer", "downstream graph consumer"],
            "behavior": f"negotiate a versioned typed knowledge contract for {scale}",
            "value": "makes schema compatibility and semantic loss explicit before graph reuse", "input_schema": input_schema,
            "output_schema": "KnowledgeContractReceipt1@1", "effects": ["none:knowledge-contract-validation", "block:unsafe-release"],
            "permissions": ["negotiate:knowledge-contract"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier,
            "boundary": PRECLINICAL_BOUNDARY, "contract_version": contract_version}

def negotiate(request: KnowledgeContractRequest, *, feature_id: str, contract_version: str, scale: str, require_federation: bool = False) -> KnowledgeContractReceipt:
    if (not request.request_id.strip() or not request.consumer.strip() or not request.producer.strip() or
        not request.namespace.strip() or not request.semantic_profile.strip() or not request.negotiated_version.strip() or
        not request.field_order or tuple(request.field_order) != tuple(sorted(set(request.field_order))) or
        request.boundary != PRECLINICAL_BOUNDARY or not request.raw_data_local or not request.aggregate_only or
        len(request.replay_identity) != 64 or any(c not in "0123456789abcdef" for c in request.replay_identity) or (require_federation and not request.policy_allow)):
        raise ResearchContractError("knowledge contract request is invalid")
    fields = set(request.field_order)
    retained = fields & set(request.retained_field_order)
    missing = fields - retained
    omitted = fields & set(request.missing_field_order)
    semantic_loss = missing | omitted
    compatible = not missing and not omitted and request.protected_closure
    disposition = "blocked" if not request.policy_allow or not request.protected_closure else "compatible" if compatible else "unknown" if not retained else "partial"
    compatibility = "compatible" if compatible else "unknown" if not retained else "additive_migration"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id,
               "request_id": request.request_id, "consumer": request.consumer, "producer": request.producer, "namespace": request.namespace,
               "semantic_profile": request.semantic_profile, "negotiated_version": request.negotiated_version, "compatibility": compatibility,
               "disposition": disposition, "field_order": sorted(fields), "retained_field_order": sorted(retained), "missing_field_order": sorted(missing),
               "omitted_field_order": sorted(omitted), "semantic_loss_order": sorted(semantic_loss), "replay_identity": request.replay_identity,
               "effect_receipts": ["none:knowledge-contract-validation"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = research_artifact_digest(payload)
    payload["contract_digest"] = digest
    payload["artifact"] = {"artifact_id": f"worldgen-knowledge-contract:{request.request_id}", "content_type": CONTENT_TYPE, "content_hash": digest, "boundary": PRECLINICAL_BOUNDARY}
    receipt = KnowledgeContractReceipt(payload)
    receipt.validate()
    return receipt

__all__ = ["CONTENT_TYPE", "KnowledgeContractRequest", "KnowledgeContractReceipt", "manifest", "negotiate"]
