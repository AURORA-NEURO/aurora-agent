"""Python parity for P21 approval-bounded reliability research copilot."""
from __future__ import annotations

from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_performance_reliability_support import _digest, _hash, assess

CONTENT_TYPE = "application/vnd.aurora.worldgen.performance-reliability-copilot-receipt+json"


def manifest(*, feature_id: str, contract_version: str, scale: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["reliability steward", "research program lead", "platform operator"], "behavior": f"run approval-bounded workload reliability qualification for {scale}", "value": "turns reliability evidence into a replayable, budgeted release decision without hidden effects", "input_schema": "PerformanceReliabilityCopilotRequest1@1", "output_schema": "PerformanceReliabilityCopilotReceipt1@1", "effects": ["invoke:bounded-reliability-tool", "block:unsafe-release"], "permissions": ["invoke:declared-reliability-tool"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def validate_copilot(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or not _digest(output.get("copilot_digest")) or artifact.get("content_hash") != output.get("copilot_digest") or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True:
        raise ResearchContractError("reliability copilot identity, locality, or digest is incomplete")


def run(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, require_approval: bool = True) -> dict[str, Any]:
    if request.get("boundary") != PRECLINICAL_BOUNDARY or not request.get("action_order") or request["action_order"] != sorted(set(request["action_order"])) or not request.get("action_budget") or not request.get("reliability_request", {}).get("raw_data_local") or not request.get("reliability_request", {}).get("aggregate_only"):
        raise ResearchContractError("reliability copilot request is invalid")
    reliability = assess(request["reliability_request"], feature_id=feature_id, contract_version=contract_version)
    omissions = list(reliability.get("omission_order", []))
    if require_approval and request.get("signed_approval") is not True:
        omissions.append("copilot:approval-missing")
    if request.get("dry_run"):
        omissions.append("copilot:dry-run-no-effect")
    if len(request["action_order"]) > request["action_budget"]:
        omissions.append("copilot:action-budget-exceeded")
    omissions = sorted(set(omissions))
    safe = reliability["disposition"] == "qualified" and (not require_approval or request.get("signed_approval") is True) and len(request["action_order"]) <= request["action_budget"] and not request.get("dry_run")
    disposition = "qualified" if safe else "blocked"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["reliability_request"]["request_id"], "disposition": disposition, "action_order": request["action_order"], "qualified_action_order": request["action_order"] if safe else [], "denied_action_order": [] if safe else request["action_order"], "reliability_disposition": reliability["disposition"], "reliability_digest": reliability["result_digest"], "replay_identity": reliability["replay_identity"], "omissions": omissions, "uncertainty_order": reliability.get("uncertainty_order", []), "negative_evidence_order": reliability.get("negative_evidence_order", []), "effect_receipts": [f"invoke:reliable-capability:{reliability['result_digest']}"] if safe else ["block:unsafe-release"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    payload["copilot_digest"] = digest
    payload["artifact"] = {"content_type": CONTENT_TYPE, "content_hash": digest, "boundary": PRECLINICAL_BOUNDARY}
    validate_copilot(payload)
    return payload


PerformanceReliabilityCopilotRequest = dict[str, Any]
PerformanceReliabilityCopilotReceipt = dict[str, Any]
PerformanceReliabilityCopilotError = ResearchContractError
__all__ = ["CONTENT_TYPE", "PerformanceReliabilityCopilotRequest", "PerformanceReliabilityCopilotReceipt", "PerformanceReliabilityCopilotError", "manifest", "run", "validate_copilot"]
