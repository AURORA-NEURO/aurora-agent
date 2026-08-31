"""Deterministic Python parity for Worldgen P24 researcher/admin workspaces."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

CONTENT_TYPE = "application/vnd.aurora.worldgen.researcher-admin-experience-receipt-1+json"
def _hash(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(values: list[str]) -> bool: return values == sorted(set(values))
def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["preclinical researcher", "study administrator", "accessibility reviewer", "provenance auditor"], "behavior": f"render a role-aware, omission-aware research workspace at {scale} ({mode} scale)", "value": "makes permitted research state accessible without hiding protected, missing, uncertain, or negative evidence", "input_schema": "WorkspaceRequest4@1", "output_schema": "ResearchWorkspaceCard7@1", "effects": ["view:research-workspace", "manage:local-capability", "block:unsafe-release"], "permissions": ["read:authorized-research-summaries"], "determinism": "byte_stable", "autonomy_tier": "A0", "boundary": PRECLINICAL_BOUNDARY}
def validate(output: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or feature_id is not None and output.get("feature_id") != feature_id or output.get("boundary") != PRECLINICAL_BOUNDARY or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or not output.get("panel_order") or not _digest(output.get("replay_identity")) or not _digest(output.get("workspace_digest")) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("content_hash") != output.get("workspace_digest") or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("semantic_loss", []) != output.get("omitted_panel_order", []):
        raise ResearchContractError("workspace identity, locality, digest, artifact, or boundary is incomplete")
    for key in ("panel_order", "selected_panel_order", "hidden_panel_order", "omitted_panel_order", "unknown_panel_order", "denied_panel_order", "negative_panel_order", "required_panel_order", "action_order", "approved_action_order", "accessibility_order", "audit_order", "effect_receipts"):
        if not _ordered(output.get(key, [])): raise ResearchContractError("workspace vectors are not canonical")
    ids = set(output["panel_order"]); parts = set(output.get("selected_panel_order", [])) | set(output.get("hidden_panel_order", [])) | set(output.get("omitted_panel_order", [])) | set(output.get("unknown_panel_order", [])) | set(output.get("denied_panel_order", []))
    if ids != parts: raise ResearchContractError("panel states do not partition")
def render(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    required = ("request_id", "scope", "study_id", "role")
    if any(not isinstance(request.get(key), str) or not request[key].strip() for key in required) or not request.get("panels") or not request.get("required_panel_order") or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or any(not _ordered(request.get(key, [])) for key in ("required_panel_order", "requested_action_order", "approved_action_order", "adversarial_events")):
        raise ResearchContractError("workspace identity, required panels, ordering, digest, locality, or boundary is invalid")
    rows = sorted(request["panels"], key=lambda row: row.get("panel_id", "")); order: list[str] = []; selected: set[str] = set(); hidden: set[str] = set(); omitted: set[str] = set(); unknown: set[str] = set(); denied: set[str] = set(); negative: set[str] = set(); panel_digests: set[str] = set()
    for panel in rows:
        pid = panel.get("panel_id", "")
        if pid in order or not isinstance(pid, str) or not pid.strip() or not isinstance(panel.get("label"), str) or not panel["label"].strip() or not isinstance(panel.get("role"), str) or not panel["role"].strip() or not _digest(panel.get("content_digest")) or not panel.get("evidence_state"): raise ResearchContractError("panel identity or digest is invalid")
        order.append(pid); panel_digests.add(panel["content_digest"])
        if panel.get("evidence_state") == "negative": negative.add(pid)
        if panel.get("local") is not True or panel.get("aggregate_only") is not True or not str(panel.get("omission_reason", "")).strip(): omitted.add(pid)
        elif panel.get("visible") is not True: hidden.add(pid)
        elif panel.get("role") != request["role"] or panel.get("requires_approval") is True and request.get("signed_approval") is not True: denied.add(pid)
        elif panel.get("evidence_state") in {"proven", "supported"}: selected.add(pid)
        else: unknown.add(pid)
    global_block = not request.get("policy_allowed") or not request.get("protected_closure") or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not request.get("network_available") or bool(request.get("adversarial_events")) or mode == "research copilot" and (request.get("action_budget", 0) <= 0 or request.get("action_count", 0) > request.get("action_budget", 0))
    if global_block: omitted.update(order); selected.clear(); hidden.clear(); unknown.clear(); denied.clear()
    complete = set(request["required_panel_order"]).issubset(selected)
    disposition = "blocked" if global_block else "approval_required" if not request.get("signed_approval") and mode != "inference" else "ready" if complete and not (unknown or omitted or denied) else "partial" if selected else "unknown"
    audit = {f"role:{request['role']}", f"study:{request['study_id']}", f"panels:{len(order)}"};
    if omitted: audit.add("omissions:visible")
    if negative: audit.add("negative-evidence:visible")
    payload: dict[str, Any] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "mode": mode, "scale": scale, "request_id": request["request_id"], "study_id": request["study_id"], "role": request["role"], "disposition": disposition, "panel_order": order, "selected_panel_order": sorted(selected), "hidden_panel_order": sorted(hidden), "omitted_panel_order": sorted(omitted), "unknown_panel_order": sorted(unknown), "denied_panel_order": sorted(denied), "negative_panel_order": sorted(negative), "required_panel_order": request["required_panel_order"], "action_order": request["requested_action_order"], "approved_action_order": request["approved_action_order"], "accessibility_order": ["contrast-checked", "keyboard-navigation", "screen-reader-labels"], "audit_order": sorted(audit), "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); payload["workspace_digest"] = digest; payload["artifact"] = {"artifact_id": f"worldgen-research-workspace:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": sorted(omitted), "panel_digests": sorted(panel_digests), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"view:research-workspace:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "ready" else ["block:unsafe-release"]; validate(payload, feature_id=feature_id); return payload
WorkspacePanel4 = dict[str, Any]; WorkspaceRequest4 = dict[str, Any]; ResearchWorkspaceCard7 = dict[str, Any]; ResearcherAdminExperienceError = ResearchContractError
__all__ = ["CONTENT_TYPE", "WorkspacePanel4", "WorkspaceRequest4", "ResearchWorkspaceCard7", "ResearcherAdminExperienceError", "manifest", "render", "validate"]
