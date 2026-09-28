"""Python parity for ``AFA-ids-P20-F15`` federated workflow planning."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P20-F15"
CONTRACT_VERSION = "ids-federated-continual-workflow-fabric/1.0"
INPUT_SCHEMA = "FederatedWorkflowRequest7@1"
OUTPUT_SCHEMA = "FederatedWorkflowReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.federated-workflow-receipt-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class FederatedWorkflowReceipt9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("artifact", {}).get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("stage_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("workflow identity, locality, stages, peers, disposition, or effects are incomplete")
        fields = ("stage_order", "selected_stage_order", "unresolved_stage_order", "blocked_stage_order", "missing_dependency_order", "cycle_order", "checkpoint_order", "compensation_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("workflow ordering is not canonical")
        ids = set(v["stage_order"]); states = v["selected_stage_order"] + v["unresolved_stage_order"] + v["blocked_stage_order"]; peers = set(v["peer_order"]); peer_states = v["qualified_peer_order"] + v["missing_peer_order"]
        if len(ids) != len(v["stage_order"]) or len(states) != len(ids) or set(states) != ids or len(peers) != len(v["peer_order"]) or len(peer_states) != len(peers) or set(peer_states) != peers:
            raise ResearchContractError("workflow stage or peer states do not partition")
        a = v.get("artifact", {}); digests = [v.get("replay_identity"), v.get("workflow_digest"), a.get("content_hash"), *a.get("provenance_digests", [])]
        if not all(_digest(d) for d in digests) or a.get("content_type") != CONTENT_TYPE or a.get("content_hash") != v.get("workflow_digest"):
            raise ResearchContractError("workflow digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:workflow-summaries:", "manage:local-capability:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("workflow effect is outside governed gate")


def federated_workflow_fabric_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["research workflow compiler", "federation operator", "laboratory integration steward", "replay auditor"], "behavior": "compiles typed stage dependencies and aggregate peer attestations into a locality-safe federated workflow plan", "value": "prevents cycles, missing checkpoints, budget overflow, peer disagreement, and unsafe effects from becoming executable research workflows", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:workflow-summaries", "manage:local-capability"], "permissions": ["read:local-workflow-manifests", "request:federated-workflow-plan"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "workflow_id", "federation_id", "purpose", "semantic_profile")) or not request.get("stages") or len(request["stages"]) > 16384 or not request.get("peers") or len(request["peers"]) > 1024 or int(request.get("checkpoint", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) <= 0 or int(request.get("max_budget_units", 0)) <= 0 or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("workflow identity, stages, peers, bounds, replay, or locality is invalid")
    ids: set[str] = set(); peer_ids: set[str] = set()
    for stage in request["stages"]:
        if not all(isinstance(stage.get(k), str) and stage[k].strip() for k in ("stage_id", "kind", "checkpoint_id", "compensation_action")) or not _digest(stage.get("input_digest")) or stage["stage_id"] in ids:
            raise ResearchContractError("stage identity, digest, compensation, or uniqueness is invalid")
        ids.add(stage["stage_id"])
    for peer in request["peers"]:
        if not all(isinstance(peer.get(k), str) and peer[k].strip() for k in ("peer_id", "workflow_id", "semantic_profile")) or int(peer.get("checkpoint", 0)) <= 0 or not _digest(peer.get("workflow_digest")) or peer["peer_id"] in peer_ids:
            raise ResearchContractError("peer identity, checkpoint, digest, or uniqueness is invalid")
        peer_ids.add(peer["peer_id"])


def compile_federated_workflow(request: Mapping[str, Any]) -> FederatedWorkflowReceipt9:
    _validate_request(request)
    stages = sorted((dict(s) for s in request["stages"]), key=lambda s: s["stage_id"]); ids = [s["stage_id"] for s in stages]; by_id = {s["stage_id"]: s for s in stages}; missing: set[str] = set(); children = {i: [] for i in ids}; indegree = {i: 0 for i in ids}
    for s in stages:
        for dep in s.get("dependency_ids", []):
            if dep in by_id: indegree[s["stage_id"]] += 1; children[dep].append(s["stage_id"])
            else: missing.add(f"{s['stage_id']}:{dep}")
    queue = sorted(i for i, degree in indegree.items() if degree == 0); topo: list[str] = []
    while queue:
        i = queue.pop(0); topo.append(i)
        for child in sorted(children[i]):
            indegree[child] -= 1
            if indegree[child] == 0: queue.append(child); queue.sort()
    cycles = set(ids) - set(topo); selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); omissions: set[str] = set(); checkpoints = {s["checkpoint_id"] for s in stages}; compensations = {s["compensation_action"] for s in stages}; total = sum(int(s.get("estimated_units", 0)) for s in stages)
    for s in stages:
        i = s["stage_id"]
        if i in cycles: blocked.add(i); omissions.add(f"{i}:dependency-cycle")
        elif any(d not in by_id for d in s.get("dependency_ids", [])): unresolved.add(i)
        elif s.get("local") is not True or s.get("aggregate_only") is not True: blocked.add(i); omissions.add(f"{i}:raw-data-locality")
        elif s.get("evidence_state") == "contradicted": blocked.add(i); negative.add(f"{i}:contradicted")
        elif s.get("evidence_state") not in {"proven", "supported"}: unresolved.add(i); uncertainty.add(f"{i}:evidence-state")
        else: selected.add(i)
    peers = sorted((dict(p) for p in request["peers"]), key=lambda p: p["peer_id"]); peer_ids = [p["peer_id"] for p in peers]; qualified = {p["peer_id"] for p in peers if p["workflow_id"] == request["workflow_id"] and p["semantic_profile"] == request["semantic_profile"] and int(p["checkpoint"]) == int(request["checkpoint"]) and p.get("signed") is True and p.get("local") is True and p.get("aggregate_only") is True and p.get("workflow_digest") == request["replay_identity"] and p.get("evidence_state") in {"proven", "supported"}}; missing_peers = set(peer_ids) - qualified
    if len(qualified) < int(request["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    if total > int(request["max_budget_units"]): omissions.add(f"request:budget-exceeded:{total}")
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(ids); selected.clear(); unresolved.clear(); omissions.add("request:governance-or-locality-denied")
    so, uo, bo = sorted(selected), sorted(unresolved), sorted(blocked); qo, mo = sorted(qualified), sorted(missing_peers); disposition = "blocked" if global_block or (not so and not uo) else "unresolved" if uo or bo or len(qo) < int(request["minimum_peer_quorum"]) or total > int(request["max_budget_units"]) else "qualified"
    if disposition != "qualified": omissions.add("request:workflow-plan-not-closed")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "workflow_id": request["workflow_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "stage_order": ids, "selected_stage_order": so, "unresolved_stage_order": uo, "blocked_stage_order": bo, "missing_dependency_order": sorted(missing), "cycle_order": sorted(cycles), "checkpoint_order": sorted(checkpoints), "compensation_order": sorted(compensations), "peer_order": peer_ids, "qualified_peer_order": qo, "missing_peer_order": mo, "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "total_units": total, "budget_remaining": max(0, int(request["max_budget_units"]) - total), "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}; wd = _hash(payload); payload["workflow_digest"] = wd; payload["artifact"] = {"artifact_id": f"federated-workflow-receipt-9:{request['workflow_id']}", "content_type": CONTENT_TYPE, "content_hash": wd, "semantic_loss": sorted(omissions), "provenance_digests": sorted({s["input_digest"] for s in stages}), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = sorted([f"exchange:workflow-summaries:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]); receipt = FederatedWorkflowReceipt9(payload); receipt.validate(); return receipt


def idsFederatedWorkflowDigest(receipt: FederatedWorkflowReceipt9) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "FederatedWorkflowReceipt9", "federated_workflow_fabric_manifest", "compile_federated_workflow", "idsFederatedWorkflowDigest"]
