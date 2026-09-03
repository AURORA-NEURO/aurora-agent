"""Python parity for P21 resumable workload reliability workflow fabric."""
from __future__ import annotations

from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_performance_reliability_copilot_support import run
from .worldgen_performance_reliability_support import _digest, _hash

CONTENT_TYPE = "application/vnd.aurora.worldgen.performance-reliability-workflow-receipt+json"


def manifest(*, feature_id: str, contract_version: str, scale: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["reliability steward", "workflow orchestrator", "research program lead"], "behavior": f"orchestrate resumable, budgeted workload reliability qualification for {scale}", "value": "makes reliability release qualification restartable, compensating, and replayable", "input_schema": "PerformanceReliabilityWorkflowRequest1@1", "output_schema": "PerformanceReliabilityWorkflowReceipt1@1", "effects": ["schedule:reliability-workflow", "block:unsafe-release"], "permissions": ["schedule:bounded-reliability-workflow"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def validate_workflow(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or not output.get("stage_order") or not _digest(output.get("replay_identity")) or not _digest(output.get("workflow_digest")) or artifact.get("content_hash") != output.get("workflow_digest") or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True:
        raise ResearchContractError("reliability workflow identity, locality, or digest is incomplete")


def schedule(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, require_approval: bool = True) -> dict[str, Any]:
    if not isinstance(request.get("workflow_id"), str) or not request["workflow_id"].strip() or not request.get("stage_order") or request["stage_order"] != sorted(set(request["stage_order"])) or not request.get("checkpoint_seq") or not request.get("budget_units") or request.get("compensation_enabled") is not True or not _digest(request.get("replay_identity")):
        raise ResearchContractError("reliability workflow request is invalid")
    completed = set(request.get("completed_stage_order", []))
    if not completed.issubset(set(request["stage_order"])):
        raise ResearchContractError("reliability workflow completed stage is undeclared")
    copilot = run(request["copilot"], feature_id=feature_id, contract_version=contract_version, scale=scale, require_approval=require_approval)
    pending = [stage for stage in request["stage_order"] if stage not in completed]
    compensation = [f"workflow:{request['workflow_id']}:retain-reliability-verdict"] if copilot["disposition"] == "blocked" else []
    disposition = "blocked" if copilot["disposition"] == "blocked" else "qualified" if not pending else "partial"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "workflow_id": request["workflow_id"], "disposition": disposition, "stage_order": request["stage_order"], "completed_stage_order": sorted(completed), "pending_stage_order": pending, "compensation_order": compensation, "checkpoint_seq": request["checkpoint_seq"], "budget_units": request["budget_units"], "consumed_units": min(len(request["copilot"]["action_order"]), request["budget_units"]), "replay_identity": request["replay_identity"], "copilot": copilot, "effect_receipts": [f"schedule:reliability-workflow:{request['workflow_id']}"] if disposition == "qualified" else ["block:unsafe-release"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    payload["workflow_digest"] = digest
    payload["artifact"] = {"content_type": CONTENT_TYPE, "content_hash": digest, "boundary": PRECLINICAL_BOUNDARY}
    validate_workflow(payload)
    return payload


PerformanceReliabilityWorkflowRequest = dict[str, Any]
PerformanceReliabilityWorkflowReceipt = dict[str, Any]
PerformanceReliabilityWorkflowError = ResearchContractError
__all__ = ["CONTENT_TYPE", "PerformanceReliabilityWorkflowRequest", "PerformanceReliabilityWorkflowReceipt", "PerformanceReliabilityWorkflowError", "manifest", "schedule", "validate_workflow"]
