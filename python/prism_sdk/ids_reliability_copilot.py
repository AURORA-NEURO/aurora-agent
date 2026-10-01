"""Python parity for ``AFA-ids-P21-F12`` reliability preflight."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P21-F12"
CONTRACT_VERSION = "ids-dry-run-replay-bounded-reliability-copilot/1.0"
INPUT_SCHEMA = "CapabilityWorkload7@1"
OUTPUT_SCHEMA = "ReliableCapabilityResult9@1"
CONTENT_TYPE = "application/vnd.aurora.reliable-capability-result-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ReliableCapabilityResult9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("artifact", {}).get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("request_id", "").strip() or not v.get("workload_id", "").strip() or not v.get("purpose", "").strip() or not v.get("semantic_profile", "").strip() or not v.get("unit_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("reliability identity, locality, units, disposition, or effects are incomplete")
        fields = ("unit_order", "dry_run_order", "ready_order", "retry_order", "unresolved_order", "blocked_order", "duplicate_order", "replay_mismatch_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(key, [])) for key in fields):
            raise ResearchContractError("reliability ordering is not canonical")
        ids = set(v["unit_order"])
        parts = v["dry_run_order"] + v["ready_order"] + v["retry_order"] + v["unresolved_order"] + v["blocked_order"] + v["duplicate_order"] + v["replay_mismatch_order"]
        if len(ids) != len(v["unit_order"]) or len(parts) != len(ids) or set(parts) != ids:
            raise ResearchContractError("reliability unit states do not partition")
        artifact = v.get("artifact", {})
        digests = [v.get("replay_identity"), v.get("result_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if not all(_digest(digest) for digest in digests) or artifact.get("content_hash") != v.get("result_digest") or artifact.get("content_type") != CONTENT_TYPE:
            raise ResearchContractError("reliability digest or artifact metadata is inconsistent")
        if any(not effect.startswith(("observe:reliability-plan:", "manage:local-capability:")) and effect != "block:unsafe-release" for effect in v["effect_receipts"]):
            raise ResearchContractError("reliability effect is outside governed gate")


def reliability_copilot_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["capability operator", "agent SDK", "replay auditor", "federation administrator"], "behavior": "preflight idempotent capability workloads with dry-run, replay, retry, evidence, and locality gates", "value": "prevents duplicate, unreplayable, over-budget, under-evidenced, or unauthorized capability effects from being treated as reliable", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["observe:reliability-plan", "manage:local-capability"], "permissions": ["read:local-capability-manifests", "request:reliability-preflight"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(key), str) and request[key].strip() for key in ("request_id", "workload_id", "purpose", "semantic_profile")) or not request.get("units") or len(request["units"]) > 16384 or int(request.get("max_budget_units", 0)) <= 0 or int(request.get("max_retries", 0)) > 16 or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("reliability identity, unit bound, retry bound, budget, replay, or locality is invalid")
    ids: set[str] = set()
    for unit in request["units"]:
        if not all(isinstance(unit.get(key), str) and unit[key].strip() for key in ("unit_id", "capability_id", "idempotency_key")) or not _digest(unit.get("input_digest")) or not _digest(unit.get("replay_identity")) or int(unit.get("estimated_units", 0)) <= 0 or int(unit.get("retry_budget", 0)) > 16 or unit["unit_id"] in ids:
            raise ResearchContractError("unit identity, capability, digest, budget, retry, or uniqueness is invalid")
        ids.add(unit["unit_id"])


def preflight_reliability(request: Mapping[str, Any]) -> ReliableCapabilityResult9:
    _validate_request(request)
    units = sorted((dict(unit) for unit in request["units"]), key=lambda unit: unit["unit_id"])
    unit_order = [unit["unit_id"] for unit in units]
    dry_run: set[str] = set(); ready: set[str] = set(); retry: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); duplicate: set[str] = set(); mismatch: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); idempotency: dict[str, str] = {}; total = 0; retry_count = 0
    for unit in units:
        total += int(unit["estimated_units"]); ident = unit["unit_id"]
        if unit["idempotency_key"] in idempotency:
            duplicate.add(ident)
        else:
            idempotency[unit["idempotency_key"]] = ident
            if unit.get("local") is not True or unit.get("aggregate_only") is not True:
                blocked.add(ident); omissions.add(f"{ident}:raw-data-locality")
            elif unit["replay_identity"] != request["replay_identity"]:
                mismatch.add(ident); omissions.add(f"{ident}:replay-identity")
            elif unit.get("evidence_state") == "contradicted":
                blocked.add(ident); negative.add(f"{ident}:contradicted")
            elif unit.get("evidence_state") not in {"proven", "supported"}:
                unresolved.add(ident); uncertainty.add(f"{ident}:evidence-state")
            elif int(unit.get("retry_budget", 0)) > int(request["max_retries"]):
                retry.add(ident); retry_count += int(unit["retry_budget"]); omissions.add(f"{ident}:retry-budget")
            elif request.get("dry_run") is True:
                dry_run.add(ident)
            else:
                ready.add(ident)
    if total > int(request["max_budget_units"]):
        omissions.add(f"request:budget-exceeded:{total}")
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if global_block:
        blocked.update(unit_order); dry_run.clear(); ready.clear(); retry.clear(); unresolved.clear(); duplicate.clear(); mismatch.clear(); omissions.add("request:governance-or-locality-denied")
    dry, read, retr, unres, block, dup, mism = (sorted(values) for values in (dry_run, ready, retry, unresolved, blocked, duplicate, mismatch))
    disposition = "blocked" if global_block or (not dry and not read and not unres) else "unresolved" if unres or block or retr or dup or mism or total > int(request["max_budget_units"]) else "qualified"
    if disposition != "qualified":
        omissions.add("request:reliability-plan-not-closed")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "workload_id": request["workload_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "unit_order": unit_order, "dry_run_order": dry, "ready_order": read, "retry_order": retr, "unresolved_order": unres, "blocked_order": block, "duplicate_order": dup, "replay_mismatch_order": mism, "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "retry_count": retry_count, "total_units": total, "budget_remaining": max(0, int(request["max_budget_units"]) - total), "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); payload["result_digest"] = digest; payload["artifact"] = {"artifact_id": f"reliable-capability-result-9:{request['workload_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": sorted(omissions), "provenance_digests": sorted({unit["input_digest"] for unit in units}), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = sorted([f"observe:reliability-plan:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"])
    result = ReliableCapabilityResult9(payload); result.validate(); return result


def idsReliabilityCopilotDigest(result: ReliableCapabilityResult9) -> str:
    result.validate(); return _hash(result.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "ReliableCapabilityResult9", "reliability_copilot_manifest", "preflight_reliability", "idsReliabilityCopilotDigest"]
