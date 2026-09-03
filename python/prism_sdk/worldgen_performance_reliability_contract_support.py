"""Python parity for P21 workload reliability contract negotiation."""
from __future__ import annotations

from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_performance_reliability_support import _digest, _hash, _ordered

CONTENT_TYPE = "application/vnd.aurora.worldgen.performance-reliability-contract-receipt+json"


def manifest(*, feature_id: str, contract_version: str, scale: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["reliability steward", "workflow orchestrator", "developer"], "behavior": f"negotiate typed workload reliability fields for {scale}", "value": "makes retention, migration, degradation, and failure budgets explicit before execution", "input_schema": "PerformanceReliabilityContractRequest1@1", "output_schema": "PerformanceReliabilityContractReceipt1@1", "effects": ["none:reliability-contract-validation", "block:unsafe-release"], "permissions": ["negotiate:reliability-contract"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def validate_contract(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or not output.get("field_order") or not _digest(output.get("replay_identity")) or not _digest(output.get("contract_digest")) or artifact.get("content_hash") != output.get("contract_digest"):
        raise ResearchContractError("reliability contract identity, locality, or digest is incomplete")
    for key in ("field_order", "retained_field_order", "missing_field_order", "degraded_field_order", "reliability_issue_order", "effect_receipts"):
        if not _ordered(output.get(key, [])):
            raise ResearchContractError("reliability contract vectors are not canonical")
    fields = set(output["field_order"])
    represented = set(output.get("retained_field_order", [])) | set(output.get("missing_field_order", [])) | set(output.get("degraded_field_order", []))
    if fields != represented:
        raise ResearchContractError("reliability contract fields do not partition")


def negotiate(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str) -> dict[str, Any]:
    if not all(isinstance(request.get(key), str) and request[key].strip() for key in ("request_id", "consumer", "producer")) or not request.get("field_order") or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _digest(request.get("replay_identity")):
        raise ResearchContractError("reliability contract request is invalid")
    fields = set(request["field_order"])
    retained = set(request.get("retained_field_order", [])) & fields
    missing = fields - retained
    degraded = set(request.get("degraded_field_order", [])) & fields
    issues: set[str] = set()
    if not request.get("policy_allow"):
        issues.add("policy-denied")
    if not request.get("protected_closure"):
        issues.add("protected-closure-incomplete")
    if not request.get("replay_compatible"):
        issues.add("replay-incompatible")
    if request.get("budget_exceeded"):
        issues.add("reliability-budget-exceeded")
    disposition = "blocked" if issues else "unresolved" if not retained else "compatible" if not missing and not degraded else "partial"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["request_id"], "consumer": request["consumer"], "producer": request["producer"], "namespace": request.get("namespace", ""), "semantic_profile": request.get("semantic_profile", ""), "negotiated_version": request.get("negotiated_version", ""), "compatibility": "compatible" if disposition == "compatible" else "degraded-migration", "disposition": disposition, "field_order": sorted(fields), "retained_field_order": sorted(retained), "missing_field_order": sorted(missing), "degraded_field_order": sorted(degraded), "reliability_issue_order": sorted(issues), "replay_identity": request["replay_identity"], "effect_receipts": ["none:reliability-contract-validation"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    payload.update({"contract_digest": digest, "artifact": {"content_type": CONTENT_TYPE, "content_hash": digest, "boundary": PRECLINICAL_BOUNDARY}})
    validate_contract(payload)
    return payload


PerformanceReliabilityContractRequest = dict[str, Any]
PerformanceReliabilityContractReceipt = dict[str, Any]
PerformanceReliabilityContractError = ResearchContractError
__all__ = ["CONTENT_TYPE", "PerformanceReliabilityContractRequest", "PerformanceReliabilityContractReceipt", "PerformanceReliabilityContractError", "manifest", "negotiate", "validate_contract"]
