"""Resumable, budgeted knowledge workflow fabric for Worldgen P04 F13-F16."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_knowledge_copilot_support import KnowledgeCopilotRequest, run as run_copilot

CONTENT_TYPE = "application/vnd.aurora.worldgen.knowledge-workflow-receipt+json"

@dataclass(frozen=True)
class KnowledgeWorkflowRequest:
    workflow_id: str
    copilot: KnowledgeCopilotRequest
    stage_order: tuple[str, ...]
    completed_stage_order: tuple[str, ...]
    checkpoint_seq: int
    budget_units: int
    compensation_enabled: bool
    replay_identity: str

@dataclass(frozen=True)
class KnowledgeWorkflowReceipt:
    value: dict[str, Any]
    def validate(self) -> None:
        v, artifact = self.value, self.value.get("artifact", {})
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("boundary") != PRECLINICAL_BOUNDARY or
            artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or
            not v.get("raw_data_local") or not v.get("aggregate_only") or not v.get("workflow_id") or not v.get("stage_order") or
            v.get("checkpoint_seq", 0) <= 0 or v.get("budget_units", 0) <= 0 or v.get("consumed_units", 0) > v.get("budget_units", 0) or not v.get("effect_receipts")):
            raise ResearchContractError("knowledge workflow identity, stages, budget, locality, or effects are incomplete")
        for key in ("stage_order", "completed_stage_order", "pending_stage_order", "compensation_order", "effect_receipts"):
            vals = tuple(v.get(key, ()))
            if vals != tuple(sorted(set(vals))):
                raise ResearchContractError("knowledge workflow ordering is not canonical")
        stages, completed, pending = set(v["stage_order"]), set(v["completed_stage_order"]), set(v["pending_stage_order"])
        if len(stages) != len(v["stage_order"]) or completed | pending != stages or len(completed) + len(pending) != len(stages):
            raise ResearchContractError("knowledge workflow stages do not partition")
        for key in ("replay_identity", "workflow_digest"):
            value = v.get(key, "")
            if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
                raise ResearchContractError("knowledge workflow digest is invalid")
        if artifact.get("content_hash") != v.get("workflow_digest"):
            raise ResearchContractError("knowledge workflow artifact digest is inconsistent")
        if not isinstance(v.get("copilot"), dict):
            raise ResearchContractError("nested knowledge copilot receipt is invalid")
    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(self.value)

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen",
            "consumers": ["knowledge engineer", "graph compiler", "workflow operator", "downstream graph consumer"],
            "behavior": f"orchestrate resumable typed knowledge representation for {scale}",
            "value": "makes graph construction restartable, budgeted, compensating, and replayable without hidden effects", "input_schema": input_schema,
            "output_schema": "KnowledgeWorkflowReceipt1@1", "effects": ["schedule:worldgen-knowledge-workflow", "block:unsafe-release"],
            "permissions": ["schedule:bounded-knowledge-workflow"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY, "contract_version": contract_version}

def schedule(request: KnowledgeWorkflowRequest, *, feature_id: str, contract_version: str, scale: str, require_approval: bool = False, require_federation: bool = False) -> KnowledgeWorkflowReceipt:
    if (not request.workflow_id.strip() or not request.stage_order or tuple(request.stage_order) != tuple(sorted(set(request.stage_order))) or request.checkpoint_seq <= 0 or
        request.budget_units <= 0 or not request.compensation_enabled or len(request.replay_identity) != 64 or request.copilot.knowledge_request.replay_identity != request.replay_identity):
        raise ResearchContractError("knowledge workflow request is invalid")
    completed = set(request.completed_stage_order)
    if not completed <= set(request.stage_order):
        raise ResearchContractError("knowledge workflow completed stages must be declared")
    copilot = run_copilot(request.copilot, feature_id=feature_id, contract_version=contract_version, scale=scale, require_approval=require_approval, require_federation=require_federation)
    pending = [stage for stage in request.stage_order if stage not in completed]
    consumed = min(len(request.copilot.action_order), request.budget_units)
    compensation = [f"workflow:{request.workflow_id}:retain-partial-knowledge-graph"] if copilot.value["disposition"] == "blocked" or len(request.copilot.action_order) > request.budget_units else []
    disposition = "blocked" if copilot.value["disposition"] == "blocked" or compensation else "qualified" if not pending else "partial"
    effects = [f"schedule:worldgen-knowledge-workflow:{request.workflow_id}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "workflow_id": request.workflow_id,
               "disposition": disposition, "stage_order": list(request.stage_order), "completed_stage_order": sorted(completed), "pending_stage_order": pending,
               "compensation_order": compensation, "checkpoint_seq": request.checkpoint_seq, "budget_units": request.budget_units, "consumed_units": consumed,
               "replay_identity": request.replay_identity, "copilot": copilot.value, "workflow_digest": "", "effect_receipts": effects,
               "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest_payload = dict(payload)
    digest_payload.pop("workflow_digest", None)
    digest = research_artifact_digest(digest_payload)
    payload["workflow_digest"] = digest
    payload["artifact"] = {"artifact_id": f"worldgen-knowledge-workflow:{request.workflow_id}", "content_type": CONTENT_TYPE, "content_hash": digest, "boundary": PRECLINICAL_BOUNDARY}
    receipt = KnowledgeWorkflowReceipt(payload)
    receipt.validate()
    return receipt

__all__ = ["CONTENT_TYPE", "KnowledgeWorkflowRequest", "KnowledgeWorkflowReceipt", "manifest", "schedule"]
