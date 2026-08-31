"""Deterministic Python parity for Worldgen P21 performance/reliability qualification."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-worldgen-P21-F01"
CONTRACT_VERSION = "worldgen-local-performance-reliability/1.0"
INPUT_SCHEMA = "CapabilityWorkload4@1"
OUTPUT_SCHEMA = "ReliableCapabilityResult6@1"
CONTENT_TYPE = "application/vnd.aurora.worldgen.performance-reliability-receipt-1+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def manifest(*, feature_id: str, contract_version: str, scale: str) -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "worldgen",
        "consumers": ["reliability engineer", "research workflow orchestrator", "platform operator"],
        "behavior": f"evaluate bounded workload reliability and interoperability attestations for {scale}",
        "value": "makes retries, latency, duplicates, failures, and missing completion explicit before exchange",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-artifacts", "block:unsafe-release"],
        "permissions": ["read:local-workload-attestations"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(request: Mapping[str, Any]) -> None:
    required = ("request_id", "scope")
    if (
        request.get("schema_version") != INPUT_SCHEMA
        or any(not isinstance(request.get(k), str) or not request[k].strip() for k in required)
        or not request.get("workloads")
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or not isinstance(request.get("latency_slo_ms"), int)
        or request["latency_slo_ms"] <= 0
        or not _ordered(request.get("adversarial_events", []))
    ):
        raise ResearchContractError("reliability request identity, SLO, digest, locality, or boundary is invalid")
    seen: set[str] = set()
    for workload in request["workloads"]:
        if (
            not isinstance(workload.get("workload_id"), str)
            or not workload["workload_id"].strip()
            or workload["workload_id"] in seen
            or not isinstance(workload.get("capability_id"), str)
            or not workload["capability_id"].strip()
            or not isinstance(workload.get("endpoint_id"), str)
            or not workload["endpoint_id"].strip()
            or not isinstance(workload.get("scope"), str)
            or not workload["scope"].strip()
            or not _digest(workload.get("provenance_digest"))
            or not _digest(workload.get("replay_identity"))
            or not _ordered(workload.get("omission_order", []))
        ):
            raise ResearchContractError("workload identity, digest, ordering, or uniqueness is invalid")
        seen.add(workload["workload_id"])


def validate_result(output: Mapping[str, Any], *, allow_feature_variants: bool = False) -> None:
    artifact = output.get("artifact", {})
    if (
        output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or (not allow_feature_variants and output.get("contract_version") != CONTRACT_VERSION)
        or (not allow_feature_variants and output.get("feature_id") != FEATURE_ID)
        or output.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("content_type") != CONTENT_TYPE
        or output.get("raw_data_local") is not True
        or output.get("aggregate_only") is not True
        or output.get("disposition") not in {"qualified", "unresolved", "blocked"}
        or not output.get("workload_order")
        or not _digest(output.get("replay_identity"))
        or not _digest(output.get("result_digest"))
        or artifact.get("content_hash") != output.get("result_digest")
    ):
        raise ResearchContractError("reliability receipt identity, locality, digest, or disposition is incomplete")
    vectors = ("workload_order", "dependable_order", "degraded_order", "blocked_order", "missing_order", "retry_order", "timeout_order", "duplicate_event_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
    if any(not _ordered(output.get(key, [])) for key in vectors):
        raise ResearchContractError("reliability receipt vectors are not canonical")
    workload_ids = set(output["workload_order"])
    represented = output.get("dependable_order", []) + output.get("degraded_order", []) + output.get("blocked_order", [])
    if len(workload_ids) != len(output["workload_order"]) or len(represented) != len(workload_ids) or set(represented) != workload_ids:
        raise ResearchContractError("workload states do not partition")
    if any(not _digest(value) for value in artifact.get("provenance_digests", [])):
        raise ResearchContractError("reliability provenance digest is invalid")


def assess(request: Mapping[str, Any], *, feature_id: str = FEATURE_ID, contract_version: str = CONTRACT_VERSION) -> dict[str, Any]:
    _validate_request(request)
    rows = sorted((dict(workload) for workload in request["workloads"]), key=lambda workload: workload["workload_id"])
    order = [workload["workload_id"] for workload in rows]
    dependable: set[str] = set()
    degraded: set[str] = set()
    blocked: set[str] = set()
    missing: set[str] = set()
    retries: set[str] = set()
    timeouts: set[str] = set()
    duplicates: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    provenance: set[str] = set()
    for workload in rows:
        workload_id = workload["workload_id"]
        provenance.add(workload["provenance_digest"])
        omissions.update(f"{workload_id}:{item}" for item in workload.get("omission_order", []))
        if workload.get("negative_result") or workload.get("evidence_state") == "negative":
            negative.add(f"{workload_id}:negative-result")
        if not workload.get("endpoint_approved") or not workload.get("local") or not workload.get("aggregate_only"):
            blocked.add(workload_id)
            omissions.add(f"{workload_id}:endpoint-approval-or-locality")
        elif workload.get("replay_identity") != request["replay_identity"]:
            degraded.add(workload_id)
            uncertainty.add(f"{workload_id}:replay-mismatch")
        else:
            if workload.get("retry_count", 0) > 0:
                retries.add(workload_id)
            if workload.get("duplicate_events", 0) > 0:
                duplicates.add(workload_id)
            if workload.get("p95_latency_ms", 0) > request["latency_slo_ms"]:
                timeouts.add(workload_id)
            if workload.get("completed_tasks", 0) < workload.get("expected_tasks", 0):
                missing.add(workload_id)
            if workload.get("retry_count", 0) > request.get("max_retries", 0) or workload.get("duplicate_events", 0) > 0 or workload.get("p95_latency_ms", 0) > request["latency_slo_ms"] or workload.get("completed_tasks", 0) < workload.get("expected_tasks", 0) or workload.get("evidence_state") not in {"proven", "supported"}:
                degraded.add(workload_id)
                uncertainty.add(f"{workload_id}:reliability-threshold-or-evidence")
            else:
                dependable.add(workload_id)
    global_block = not all(request.get(key) is True for key in ("policy_allowed", "protected_closure", "signed_approval", "network_available", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block:
        blocked.update(order)
        dependable.clear()
        degraded.clear()
        omissions.add("request:security-or-adversarial-gate-blocked")
    uncertainty.update(f"adversarial:{event}" for event in request.get("adversarial_events", []))
    dependable_order = sorted(dependable)
    degraded_order = sorted(degraded)
    blocked_order = sorted(blocked)
    disposition = "blocked" if global_block or (not dependable_order and not degraded_order) else "unresolved" if degraded_order or blocked_order else "qualified"
    if disposition != "qualified":
        omissions.add("request:reliability-closure-not-ready")
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request["request_id"],
        "scope": request["scope"],
        "disposition": disposition,
        "workload_order": order,
        "dependable_order": dependable_order,
        "degraded_order": degraded_order,
        "blocked_order": blocked_order,
        "missing_order": sorted(missing),
        "retry_order": sorted(retries),
        "timeout_order": sorted(timeouts),
        "duplicate_event_order": sorted(duplicates),
        "omission_order": sorted(omissions),
        "uncertainty_order": sorted(uncertainty),
        "negative_evidence_order": sorted(negative),
        "replay_identity": request["replay_identity"],
        "raw_data_local": True,
        "aggregate_only": True,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    result_digest = _hash(payload)
    payload["result_digest"] = result_digest
    payload["artifact"] = {"artifact_id": f"reliable-capability-result-6:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": result_digest, "semantic_loss": sorted(omissions), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    payload["effect_receipts"] = [f"exchange:permitted-artifacts:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    validate_result(payload, allow_feature_variants=True)
    return payload


CapabilityWorkloadRequest4 = dict[str, Any]
ReliableCapabilityResult6 = dict[str, Any]
WorkloadEvidenceState = str
PerformanceReliabilityError = ResearchContractError

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "CapabilityWorkloadRequest4", "ReliableCapabilityResult6", "WorkloadEvidenceState", "PerformanceReliabilityError", "manifest", "assess", "validate_result"]
