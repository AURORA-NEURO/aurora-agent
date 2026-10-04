"""Python parity for ``AFA-ids-P29-F15`` scale-frontier previews."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P29-F15"
CONTRACT_VERSION = "ids-prospective-high-throughput-scale-frontier-workflow-fabric/1.0"
INPUT_SCHEMA = "IdsScaleWorkload8@1"
OUTPUT_SCHEMA = "IdsCapacityReport9@1"
CONTENT_TYPE = "application/vnd.aurora.ids-capacity-report-9+json"
MAX_CELLS = 16_384


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class IdsCapacityReport9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value
        artifact = value.get("artifact", {})
        if (
            value.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or value.get("contract_version") != CONTRACT_VERSION
            or value.get("feature_id") != FEATURE_ID
            or value.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or value.get("raw_data_local") is not True
            or value.get("aggregate_only") is not True
            or not all(isinstance(value.get(key), str) and value[key].strip() for key in ("request_id", "purpose", "semantic_profile"))
            or not value.get("workload_order")
            or not value.get("effect_order")
            or not value.get("effect_receipts")
            or value.get("disposition") not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("capacity identity, workload cells, effects, locality, or disposition is incomplete")
        for key in (
            "workload_order", "selected_order", "unresolved_order", "blocked_order", "capacity_exceeded_order",
            "budget_exhausted_order", "unknown_order", "negative_evidence_order", "omission_order",
            "uncertainty_order", "effect_order", "effect_receipts",
        ):
            if not _ordered(value.get(key, [])):
                raise ResearchContractError("capacity ordering is not canonical")
        ids = set(value["workload_order"])
        parts = value["selected_order"] + value["unresolved_order"] + value["blocked_order"]
        if len(ids) != len(value["workload_order"]) or len(parts) != len(ids) or set(parts) != ids:
            raise ResearchContractError("capacity states do not partition")
        if (
            len(value.get("latency_milli_order", [])) != len(value["workload_order"])
            or len(value.get("cost_milli_order", [])) != len(value["workload_order"])
            or not _digest(value.get("replay_identity"))
            or not _digest(value.get("capacity_digest"))
            or artifact.get("content_hash") != value.get("capacity_digest")
            or artifact.get("content_type") != CONTENT_TYPE
            or any(not _digest(digest) for digest in artifact.get("provenance_digests", []))
        ):
            raise ResearchContractError("capacity digest or artifact metadata is inconsistent")
        if any(
            not effect.startswith(("preview:ids-capacity-frontier:", "manage:local-capability:"))
            and effect != "block:unsafe-release"
            for effect in value["effect_receipts"]
        ):
            raise ResearchContractError("effect is outside the governed scale-frontier gate")

    def digest(self) -> str:
        self.validate()
        return _hash(self.to_dict())


def scale_frontier_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["formal methods researcher", "capacity operator", "workflow engineer"],
        "behavior": "preview typed high-throughput capacity cells with deterministic concurrency, latency, cost, evidence, policy, replay, and locality gates",
        "value": "makes scale limits and budget exhaustion visible before any work is scheduled",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["preview:ids-capacity-frontier", "manage:local-capability"],
        "permissions": ["read:local-capacity-summaries", "request:scale-frontier-preview"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(request: Mapping[str, Any]) -> None:
    if (
        not all(isinstance(request.get(key), str) and request[key].strip() for key in ("workload_id", "capability_id", "purpose", "semantic_profile"))
        or not isinstance(request.get("maximum_concurrency"), int) or request["maximum_concurrency"] <= 0
        or not request.get("cells")
        or len(request["cells"]) > MAX_CELLS
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
    ):
        raise ResearchContractError("workload identity, concurrency, cells, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for cell in request["cells"]:
        if (
            not isinstance(cell, Mapping)
            or not isinstance(cell.get("cell_id"), str)
            or not cell["cell_id"].strip()
            or cell["cell_id"] in ids
            or cell.get("workload_id") != request["workload_id"]
            or not isinstance(cell.get("concurrency"), int) or cell["concurrency"] <= 0
            or not isinstance(cell.get("batch_size"), int) or cell["batch_size"] <= 0
            or not isinstance(cell.get("capacity_limit"), int) or cell["capacity_limit"] <= 0
            or not isinstance(cell.get("expected_latency_milli"), int) or cell["expected_latency_milli"] < 0
            or not isinstance(cell.get("expected_cost_milli"), int) or cell["expected_cost_milli"] < 0
            or not _digest(cell.get("provenance_digest")) or not _digest(cell.get("replay_identity"))
            or cell.get("local") is not True or cell.get("aggregate_only") is not True
            or cell.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"}
        ):
            raise ResearchContractError(f"capacity cell {cell.get('cell_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(cell["cell_id"])


def preview_ids_scale_frontier(request: Mapping[str, Any]) -> IdsCapacityReport9:
    _validate_request(request)
    cells = sorted((dict(cell) for cell in request["cells"]), key=lambda cell: cell["cell_id"])
    workload_order = [cell["cell_id"] for cell in cells]
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set()
    exceeded: set[str] = set(); exhausted: set[str] = set(); unknown: set[str] = set(); negative: set[str] = set()
    omissions: set[str] = set(); uncertainty: set[str] = set(); provenance: set[str] = set()
    latency: dict[str, int] = {}; costs: dict[str, int] = {}
    for cell in cells:
        latency[cell["cell_id"]] = cell["expected_latency_milli"]; costs[cell["cell_id"]] = cell["expected_cost_milli"]
        provenance.add(cell["provenance_digest"])
        if cell["concurrency"] > request["maximum_concurrency"] or cell["concurrency"] > cell["capacity_limit"]:
            unresolved.add(cell["cell_id"]); exceeded.add(cell["cell_id"]); omissions.add(f"{cell['cell_id']}:capacity-exceeded")
        elif cell["expected_cost_milli"] > request["budget_milli"]:
            unresolved.add(cell["cell_id"]); exhausted.add(cell["cell_id"]); omissions.add(f"{cell['cell_id']}:budget-exhausted")
        elif cell["replay_identity"] != request["replay_identity"]:
            unresolved.add(cell["cell_id"]); uncertainty.add(f"{cell['cell_id']}:replay-identity")
        elif cell["evidence_state"] == "contradicted":
            blocked.add(cell["cell_id"]); negative.add(f"{cell['cell_id']}:contradicted")
        elif cell["evidence_state"] not in {"proven", "supported"}:
            unresolved.add(cell["cell_id"]); unknown.add(cell["cell_id"]); uncertainty.add(f"{cell['cell_id']}:evidence-state")
        else:
            selected.add(cell["cell_id"])
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if global_block:
        blocked.update(workload_order); selected.clear(); unresolved.clear(); omissions.add("request:governance-or-locality-denied")
    selected_order = sorted(selected); unresolved_order = sorted(unresolved); blocked_order = sorted(blocked)
    disposition = "blocked" if global_block or (not selected_order and not unresolved_order) else ("unresolved" if blocked_order or unresolved_order else "qualified")
    if disposition != "qualified": omissions.add("request:scale-frontier-not-closed")
    effect_order = sorted(["manage:local-capability", "preview:ids-capacity-frontier"] if disposition == "qualified" else ["block:unsafe-release"])
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request["workload_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"],
        "disposition": disposition, "workload_order": workload_order, "selected_order": selected_order,
        "unresolved_order": unresolved_order, "blocked_order": blocked_order, "capacity_exceeded_order": sorted(exceeded),
        "budget_exhausted_order": sorted(exhausted), "unknown_order": sorted(unknown), "negative_evidence_order": sorted(negative),
        "latency_milli_order": [latency[cell_id] for cell_id in workload_order], "cost_milli_order": [costs[cell_id] for cell_id in workload_order],
        "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "effect_order": effect_order,
        "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    value = dict(payload)
    value["capacity_digest"] = digest
    value["artifact"] = {"artifact_id": f"ids-capacity-report-9:{request['workload_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    value["effect_receipts"] = sorted(effect if effect == "block:unsafe-release" else f"{effect}:{request['workload_id']}" for effect in effect_order)
    receipt = IdsCapacityReport9(value); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "IdsCapacityReport9", "scale_frontier_manifest", "preview_ids_scale_frontier"]
