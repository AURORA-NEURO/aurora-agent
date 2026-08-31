"""Python parity surface for ``AFA-oraclex-P21-F24``.

The gateway evaluates metadata-only invocation summaries.  It never moves raw experimental
data and preserves retries, omissions, uncertainty, negative results, and adversarial events.
"""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-oraclex-P21-F24"
CONTRACT_VERSION = "oraclex-federated-continual-performance-reliability-interoperability-gateway/1.0"
INPUT_SCHEMA = "CapabilityWorkload4@1"
OUTPUT_SCHEMA = "ReliableCapabilityResult6@1"
CONTENT_TYPE = "application/vnd.aurora.reliable-capability-result-6+json"


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def performance_reliability_interoperability_gateway_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "oraclex",
        "consumers": ["research program lead", "federation reliability steward", "institution operations operator"],
        "behavior": "negotiate signed capability reliability summaries into deterministic federated envelopes with explicit retry, timeout, duplicate-event, migration, and failure evidence",
        "value": "lets research programs depend on measurable, replayable capability health without moving raw workloads or hiding degraded operation",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["execute:local-computation", "write:local-artifact", "exchange:permitted-artifacts"],
        "permissions": ["connect:approved-endpoints", "exchange:permitted-artifacts"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def validate_performance_reliability_result(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    required = ("request_id", "scope", "federation_id", "institution_id", "capability_id")
    if (
        output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or output.get("contract_version") != CONTRACT_VERSION
        or output.get("feature_id") != FEATURE_ID
        or output.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("content_type") != CONTENT_TYPE
        or output.get("raw_data_local") is not True
        or output.get("aggregate_only") is not True
        or any(not isinstance(output.get(key), str) or not output[key].strip() for key in required)
        or output.get("disposition") not in {"qualified", "unresolved", "blocked"}
        or not output.get("invocation_order")
        or not output.get("endpoint_order")
        or not output.get("effect_receipts")
        or not output.get("budget_used_units", 0) > 0
    ):
        raise ResearchContractError("reliability identity, locality, endpoint, budget, or effects are incomplete")
    keys = ("invocation_order", "dependable_order", "degraded_order", "blocked_order", "missing_order", "endpoint_order", "retry_order", "timeout_order", "duplicate_event_order", "migration_order", "omission_order", "uncertainty_order", "negative_evidence_order", "adversarial_event_order", "effect_receipts")
    if any(not _ordered(output.get(key, [])) for key in keys):
        raise ResearchContractError("reliability ordering is not canonical")
    identifiers = set(output["invocation_order"])
    states = output["dependable_order"] + output["degraded_order"] + output["blocked_order"] + output["missing_order"]
    if len(identifiers) != len(output["invocation_order"]) or len(states) != len(set(states)) or set(states) != identifiers:
        raise ResearchContractError("invocation states do not partition")
    if not _valid_digest(output.get("replay_identity")) or not _valid_digest(output.get("result_digest")) or artifact.get("content_hash") != output.get("result_digest"):
        raise ResearchContractError("reliability digest or artifact hash is inconsistent")
    if any(effect != "block:unsafe-release" and not effect.startswith("exchange:permitted-artifacts:") for effect in output["effect_receipts"]):
        raise ResearchContractError("reliability effect is outside exchange gate")


def negotiate_performance_reliability(workload: Mapping[str, Any]) -> dict[str, Any]:
    if (
        workload.get("schema_version") != INPUT_SCHEMA
        or any(not isinstance(workload.get(key), str) or not workload[key].strip() for key in ("request_id", "scope", "federation_id", "institution_id", "capability_id"))
        or not workload.get("required_invocation_order")
        or not _ordered(workload["required_invocation_order"])
        or not _ordered(workload.get("adversarial_event_order", []))
        or not _valid_digest(workload.get("replay_identity"))
        or not workload.get("budget_units", 0) > 0
        or workload.get("raw_data_local") is not True
        or workload.get("aggregate_only") is not True
        or workload.get("boundary") != PRECLINICAL_BOUNDARY
        or not workload.get("invocations")
    ):
        raise ResearchContractError("workload identity, required closure, replay, budget, locality, or boundary is invalid")
    rows = sorted((dict(row) for row in workload["invocations"]), key=lambda row: row.get("invocation_id", ""))
    ids = [row.get("invocation_id", "") for row in rows]
    if not all(ids) or len(ids) != len(set(ids)):
        raise ResearchContractError("invocation identifiers must be unique and non-empty")
    dependable: set[str] = set(); degraded: set[str] = set(); blocked: set[str] = set(); missing: set[str] = set(); endpoints: set[str] = set(); retry: set[str] = set(); timeout: set[str] = set(); duplicate: set[str] = set(); omission: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    budget_used = 0
    for row in rows:
        iid = row["invocation_id"]; endpoints.add(row.get("endpoint", "")); budget_used += int(row.get("expected_tasks", 0) or 0)
        omission.update(f"{iid}:{item}" for item in row.get("omission_order", []))
        if row.get("negative_result") or row.get("evidence_state") in {"negative", "contradicted"}: negative.add(f"{iid}:negative-result")
        if row.get("retry_count", 0) > 0: retry.add(iid)
        if row.get("p95_latency_ms", 0) > row.get("latency_slo_ms", 0): timeout.add(iid)
        if row.get("duplicate_events", 0) > 0: duplicate.add(iid)
        if not row.get("signed") or not row.get("permitted") or row.get("raw_data_local") is not True or row.get("aggregate_only") is not True:
            blocked.add(iid); omission.add(f"{iid}:signature-permission-or-locality")
        elif row.get("replay_identity") != workload["replay_identity"]:
            degraded.add(iid); uncertainty.add(f"{iid}:replay-mismatch")
        elif int(row.get("expected_tasks", 0) or 0) == 0 or int(row.get("completed_tasks", 0) or 0) < int(row.get("expected_tasks", 0) or 0) or int(row.get("retry_count", 0) or 0) > int(row.get("max_retries", 0) or 0) or int(row.get("duplicate_events", 0) or 0) > 0 or int(row.get("p95_latency_ms", 0) or 0) > int(row.get("latency_slo_ms", 0) or 0) or row.get("evidence_state") not in {"proven", "supported"} or not _valid_digest(row.get("artifact_digest")) or not _valid_digest(row.get("provenance_digest")):
            degraded.add(iid); uncertainty.add(f"{iid}:reliability-threshold-or-evidence")
        else: dependable.add(iid)
    for required in workload["required_invocation_order"]:
        if required not in ids: missing.add(required); omission.add(f"request:missing-invocation:{required}")
    global_block = not all(workload.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "network_available", "raw_data_local", "aggregate_only")) or bool(workload.get("adversarial_event_order")) or budget_used > int(workload["budget_units"])
    if global_block:
        blocked.update(ids); dependable.clear(); degraded.clear(); omission.add("request:security-policy-protected-closure-or-network-blocked" if budget_used <= int(workload["budget_units"]) else "request:budget-exhausted")
    uncertainty.update(f"adversarial:{event}" for event in workload.get("adversarial_event_order", []))
    dependable_order = sorted(dependable); degraded_order = sorted(degraded); blocked_order = sorted(blocked); missing_order = sorted(missing); invocation_order = ids; endpoint_order = sorted(endpoints); retry_order = sorted(retry); timeout_order = sorted(timeout); duplicate_order = sorted(duplicate); omission_order = sorted(omission); uncertainty_order = sorted(uncertainty); negative_order = sorted(negative)
    disposition = "blocked" if global_block or (not dependable_order and not degraded_order) else "unresolved" if degraded_order or blocked_order or missing_order else "qualified"
    payload: dict[str, Any] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": workload["request_id"], "scope": workload["scope"], "federation_id": workload["federation_id"], "institution_id": workload["institution_id"], "capability_id": workload["capability_id"], "disposition": disposition, "invocation_order": invocation_order, "dependable_order": dependable_order, "degraded_order": degraded_order, "blocked_order": blocked_order, "missing_order": missing_order, "endpoint_order": endpoint_order, "retry_order": retry_order, "timeout_order": timeout_order, "duplicate_event_order": duplicate_order, "migration_order": [], "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_order, "adversarial_event_order": sorted(workload.get("adversarial_event_order", [])), "budget_used_units": budget_used, "replay_identity": workload["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    result_digest = _digest(payload); payload["result_digest"] = result_digest; payload["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"reliable-capability-result-6:{workload['request_id']}", "content_type": CONTENT_TYPE, "content_hash": result_digest, "semantic_loss": [{"field": item, "reason": "reliability gate or migration boundary", "severity": "unknown"} for item in omission_order], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"exchange:permitted-artifacts:{workload['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    validate_performance_reliability_result(payload)
    return payload


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "performance_reliability_interoperability_gateway_manifest", "negotiate_performance_reliability", "validate_performance_reliability_result"]
