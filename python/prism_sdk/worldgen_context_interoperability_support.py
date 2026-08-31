"""Aggregate-only context interoperability gateway for Worldgen P03 F21-F24."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_context_compilation_support import ContextCompilationRequest, compile as compile_context

CONTENT_TYPE = "application/vnd.aurora.worldgen.context-interoperability-receipt+json"
_HEX = re.compile(r"^[0-9a-f]{64}$")

@dataclass(frozen=True)
class ContextInteroperabilityRequest:
    context_request: ContextCompilationRequest
    partner_id: str
    semantic_profile: str
    expected_contract_version: str
    requested_export_order: tuple[str, ...]
    permitted_export_order: tuple[str, ...]
    replay_identity: str
    signed_approval: bool = False
    federation_approved: bool = False
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ContextInteroperabilityReceipt:
    value: dict[str, Any]

    def validate(self, *, feature_id: str, contract_version: str) -> None:
        value, artifact = self.value, self.value.get("artifact", {})
        requested = set(value.get("requested_export_order", ()))
        parts = set(value.get("exported_order", ())) | set(value.get("denied_export_order", ()))
        valid = (value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and value.get("contract_version") == contract_version and value.get("feature_id") == feature_id and value.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("content_type") == CONTENT_TYPE and artifact.get("raw_facts") is False and value.get("raw_data_local") is True and value.get("aggregate_only") is True and requested and parts == requested and value.get("effect_receipts") and all(_HEX.fullmatch(value.get(key, "")) for key in ("context_digest", "replay_identity", "envelope_digest")) and artifact.get("content_hash") == value.get("envelope_digest"))
        if not valid:
            raise ResearchContractError("context gateway identity, export contract, locality, digests, or effects are incomplete")
        for key in ("requested_export_order", "permitted_export_order", "exported_order", "denied_export_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts"):
            values = tuple(value.get(key, ()))
            if values != tuple(sorted(set(values))):
                raise ResearchContractError("context gateway ordering is not canonical")
        if any(effect != "block:unsafe-release" and not effect.startswith("export:worldgen-context:") for effect in value["effect_receipts"]):
            raise ResearchContractError("context gateway effect is outside aggregate export gate")

    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id, contract_version=contract_version)
        return _digest(self.value)

def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["schema steward", "federation operator", "downstream context consumer"], "behavior": f"negotiate a typed aggregate-only context exchange for {scale}", "value": "prevents semantic or policy-incompatible context artifacts from crossing a research boundary", "input_schema": input_schema, "output_schema": "FederationEnvelopeContext1@1", "effects": ["export:worldgen-context", "block:unsafe-release"], "permissions": ["export:aggregate-context-metadata"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY}

def negotiate(request: ContextInteroperabilityRequest, *, feature_id: str, contract_version: str, scale: str, require_approval: bool, require_federation: bool) -> ContextInteroperabilityReceipt:
    if (not request.partner_id.strip() or not request.semantic_profile.strip() or not request.expected_contract_version.strip() or request.boundary != PRECLINICAL_BOUNDARY or request.context_request.boundary != PRECLINICAL_BOUNDARY or not request.context_request.raw_data_local or not request.context_request.aggregate_only or not _HEX.fullmatch(request.replay_identity) or request.replay_identity != request.context_request.replay_identity or not request.requested_export_order or tuple(request.requested_export_order) != tuple(sorted(set(request.requested_export_order))) or not request.permitted_export_order or tuple(request.permitted_export_order) != tuple(sorted(set(request.permitted_export_order)))):
        raise ResearchContractError("context gateway identity, semantic profile, export order, locality, boundary, or replay is invalid")
    context = compile_context(request.context_request, feature_id=feature_id, contract_version=contract_version, require_federation=require_federation).value
    approvals = (not require_approval or request.signed_approval) and (not require_federation or request.federation_approved)
    contract_ok = request.expected_contract_version == contract_version
    permitted = sorted(set(request.requested_export_order) & set(request.permitted_export_order))
    denied = sorted(set(request.requested_export_order) - set(request.permitted_export_order))
    safe = context["disposition"] == "qualified" and approvals and contract_ok and not denied
    disposition = "blocked" if not approvals or not contract_ok or context["disposition"] == "blocked" else "qualified" if safe else "partial"
    omissions = list(context["omissions"])
    omissions += ([] if approvals else ["gateway:approval-missing"]) + ([] if contract_ok else ["gateway:contract-version-mismatch"]) + ([] if not denied else ["gateway:requested-field-not-permitted"])
    omissions = sorted(set(omissions))
    effects = [f"export:worldgen-context:{request.partner_id}"] if disposition == "qualified" else ["block:unsafe-release"]
    exported = permitted if safe else []
    denied_out = denied if safe else sorted(set(request.requested_export_order))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request.context_request.request_id, "partner_id": request.partner_id, "semantic_profile": request.semantic_profile, "disposition": disposition, "requested_export_order": list(request.requested_export_order), "permitted_export_order": list(request.permitted_export_order), "exported_order": exported, "denied_export_order": denied_out, "context_disposition": context["disposition"], "context_digest": context["context_digest"], "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": sorted(context["uncertainty"]), "negative_evidence": sorted(context["negative_evidence"]), "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    envelope_digest = _digest(payload)
    payload["envelope_digest"] = envelope_digest
    payload["artifact"] = {"artifact_id": f"worldgen-context-envelope:{request.partner_id}", "content_type": CONTENT_TYPE, "content_hash": envelope_digest, "raw_facts": False, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    receipt = ContextInteroperabilityReceipt(payload)
    receipt.validate(feature_id=feature_id, contract_version=contract_version)
    return receipt

__all__ = ["CONTENT_TYPE", "ContextInteroperabilityRequest", "ContextInteroperabilityReceipt", "manifest", "negotiate"]
