"""Read-only, omission-aware context workbench for Worldgen P03 F17-F20."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_context_copilot_support import ContextCopilotRequest, run as run_copilot

CONTENT_TYPE = "application/vnd.aurora.worldgen.context-workbench-receipt+json"
_HEX = re.compile(r"^[0-9a-f]{64}$")
VIEWS = ("view:context", "view:evidence", "view:omissions", "view:negative", "view:provenance")
PANELS = ("panel:qualified", "panel:partial", "panel:unknown", "panel:blocked", "panel:negative")

@dataclass(frozen=True)
class ContextWorkbenchRequest:
    copilot: ContextCopilotRequest
    workspace_id: str
    scope: str
    requested_view_order: tuple[str, ...]
    requested_panel_order: tuple[str, ...]
    budget_units: int
    replay_identity: str
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ContextWorkbenchReceipt:
    value: dict[str, Any]

    def validate(self, *, feature_id: str, contract_version: str) -> None:
        value, artifact = self.value, self.value.get("artifact", {})
        required = set(value.get("required_fact_order", ()))
        parts = tuple(value.get("resolved_fact_order", ())) + tuple(value.get("unknown_fact_order", ())) + tuple(value.get("blocked_fact_order", ()))
        valid = (
            value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION
            and value.get("contract_version") == contract_version
            and value.get("feature_id") == feature_id
            and value.get("boundary") == PRECLINICAL_BOUNDARY
            and artifact.get("boundary") == PRECLINICAL_BOUNDARY
            and artifact.get("content_type") == CONTENT_TYPE
            and value.get("raw_data_local") is True
            and value.get("aggregate_only") is True
            and value.get("request_id", "").strip()
            and value.get("workspace_id", "").strip()
            and value.get("scope", "").strip()
            and tuple(value.get("view_order", ())) == VIEWS
            and tuple(value.get("panel_order", ())) == PANELS
            and required
            and len(parts) == len(required)
            and set(parts) == required
            and value.get("effect_receipts")
            and all(_HEX.fullmatch(value.get(key, "")) for key in ("replay_identity", "copilot_digest", "workbench_digest"))
            and artifact.get("content_hash") == value.get("workbench_digest")
        )
        if not valid:
            raise ResearchContractError("worldgen context workbench identity, panels, locality, digests, or effects are incomplete")
        for key in ("required_fact_order", "resolved_fact_order", "unknown_fact_order", "blocked_fact_order", "denied_action_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts"):
            values = tuple(value.get(key, ()))
            if values != tuple(sorted(set(values))):
                raise ResearchContractError("worldgen context workbench ordering is not canonical")
        if any(effect != "block:unsafe-release" and not effect.startswith("view:context-workbench:") for effect in value["effect_receipts"]):
            raise ResearchContractError("worldgen context workbench effect is outside read-only gate")

    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id, contract_version=contract_version)
        return _digest(self.value)

def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["preclinical researcher", "research program lead", "imaging core scientist", "downstream context consumer"], "behavior": f"render deterministic omission-aware context workbench panels for {scale}", "value": "makes qualified, partial, unknown, negative, and blocked context state inspectable without hidden effects", "input_schema": input_schema, "output_schema": "ContextWorkbenchReceipt1@1", "effects": ["view:context-workbench", "block:unsafe-release"], "permissions": ["read:local-context-artifacts"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY}

def render(request: ContextWorkbenchRequest, *, feature_id: str, contract_version: str, require_approval: bool, require_federation: bool) -> ContextWorkbenchReceipt:
    if (not request.workspace_id.strip() or not request.scope.strip() or request.budget_units <= 0 or request.boundary != PRECLINICAL_BOUNDARY or not request.copilot.dry_run or not request.copilot.context_request.raw_data_local or not request.copilot.context_request.aggregate_only or tuple(request.requested_view_order) != VIEWS or tuple(request.requested_panel_order) != PANELS or not _HEX.fullmatch(request.replay_identity) or request.replay_identity != request.copilot.context_request.replay_identity):
        raise ResearchContractError("worldgen context workbench identity, read-only, budget, locality, views, panels, or replay is invalid")
    copilot = run_copilot(request.copilot, feature_id=feature_id, contract_version=contract_version, require_approval=require_approval, require_federation=require_federation)
    required = sorted(request.copilot.context_request.required_fact_order)
    resolved = required if copilot.value["context_disposition"] == "qualified" and not copilot.value["denied_action_order"] else []
    blocked = required if copilot.value["disposition"] == "blocked" else []
    unknown = [] if blocked or resolved else required
    omissions = sorted(set(copilot.value["omissions"] + ["workbench:read-only-local-view"]))
    effects = ["block:unsafe-release"] if copilot.value["disposition"] == "blocked" else [f"view:context-workbench:{request.workspace_id}"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request.copilot.context_request.request_id, "workspace_id": request.workspace_id, "scope": request.scope, "disposition": copilot.value["disposition"], "view_order": list(VIEWS), "panel_order": list(PANELS), "required_fact_order": required, "resolved_fact_order": resolved, "unknown_fact_order": unknown, "blocked_fact_order": blocked, "denied_action_order": sorted(copilot.value["denied_action_order"]), "replay_identity": request.replay_identity, "copilot_digest": _digest(copilot.value), "omissions": omissions, "uncertainty": sorted(set(copilot.value["uncertainty"])), "negative_evidence": sorted(set(copilot.value["negative_evidence"])), "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    workbench_digest = _digest(payload)
    payload["workbench_digest"] = workbench_digest
    payload["artifact"] = {"artifact_id": f"worldgen-context-workbench:{request.workspace_id}", "content_type": CONTENT_TYPE, "content_hash": workbench_digest, "boundary": PRECLINICAL_BOUNDARY, "views": list(VIEWS), "panels": list(PANELS)}
    receipt = ContextWorkbenchReceipt(payload)
    receipt.validate(feature_id=feature_id, contract_version=contract_version)
    return receipt

__all__ = ["CONTENT_TYPE", "VIEWS", "PANELS", "ContextWorkbenchRequest", "ContextWorkbenchReceipt", "manifest", "render"]
