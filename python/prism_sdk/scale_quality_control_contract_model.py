"""Parity implementation for ``AFA-scale-P07-F05``.

The model is intentionally read-only: it validates a typed ResearchObject3 envelope and emits a
deterministic QualityVerdict2. Raw preclinical payloads remain at the institution.
"""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-scale-P07-F05"
CONTRACT_VERSION = "scale-local-single-study-quality-control-contract-model/1.0"
INPUT_SCHEMA = "ResearchObject3@1"
OUTPUT_SCHEMA = "QualityVerdict2@1"
CONTENT_TYPE = "application/vnd.aurora.scale-quality-verdict-2+json"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(value: list[str]) -> bool:
    return value == sorted(set(value))

def prospective_quality_control_contract_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "scale", "consumers": ["research workflow operator", "quality engineer", "benchmark curator"], "behavior": "validate and canonicalize local single-study preclinical quality envelopes into witness-bearing verdict artifacts", "value": "gives independent producers a stable schema, serializer, and compatibility boundary without granting execution authority", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["read:local-research-artifacts"], "permissions": ["read:local-research-artifacts"], "autonomy_tier": "A0", "boundary": PRECLINICAL_BOUNDARY}

def _validate_request(request: Mapping[str, Any]) -> None:
    if request.get("schema_version") != INPUT_SCHEMA or not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "consumer", "purpose")) or not request.get("required_metric_order") or not _ordered(request["required_metric_order"]) or not _ordered(request.get("required_modality_order", [])) or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("identity, required closure, replay, or boundary is invalid")
    obj = request.get("object", {})
    if not isinstance(obj.get("object_id"), str) or not obj["object_id"].strip() or not isinstance(obj.get("semantic_profile"), str) or not obj["semantic_profile"].strip() or not _ordered(obj.get("modality_order", [])) or not _digest(obj.get("provenance_digest")) or obj.get("replay_identity") != request["replay_identity"]:
        raise ResearchContractError("research-object identity, modality ordering, provenance, or replay is invalid")
    ids: set[str] = set()
    for metric in obj.get("metrics", []):
        if not isinstance(metric.get("metric_id"), str) or not metric["metric_id"].strip() or metric["metric_id"] in ids or not _digest(metric.get("provenance_digest")) or metric.get("replay_identity") != request["replay_identity"] or not _ordered(metric.get("omission_order", [])) or not _ordered(metric.get("uncertainty_order", [])):
            raise ResearchContractError("metric identity, uniqueness, replay, provenance, or ordering is invalid")
        ids.add(metric["metric_id"])

def validate_prospective_quality_control_contract(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {}).get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version") != CONTRACT_VERSION or output.get("feature_id") != FEATURE_ID or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("disposition") not in {"qualified", "unresolved", "blocked"} or not all(isinstance(output.get(k), str) and output[k].strip() for k in ("request_id", "consumer", "object_id")):
        raise ResearchContractError("identity, locality, metric closure, disposition, or artifact metadata is incomplete")
    fields = ("metric_order", "passed_order", "failed_order", "unknown_order", "unmeasured_order", "blocked_order", "missing_order", "modality_order", "missing_modality_order", "omission_order", "uncertainty_order", "negative_evidence_order")
    if any(not _ordered(output.get(k, [])) for k in fields):
        raise ResearchContractError("output ordering is not canonical")
    ids = set(output.get("metric_order", [])); parts = sum((output.get(k, []) for k in ("passed_order", "failed_order", "unknown_order", "unmeasured_order", "blocked_order", "missing_order")), [])
    if len(ids) != len(output.get("metric_order", [])) or len(parts) != len(ids) or set(parts) != ids:
        raise ResearchContractError("metric states do not partition metrics")
    if not _digest(output.get("replay_identity")) or not _digest(output.get("verdict_digest")) or artifact.get("content_hash") != output.get("verdict_digest") or any(not _digest(v) for v in output.get("artifact", {}).get("provenance_order", [])):
        raise ResearchContractError("artifact or replay digest is invalid")

def model_prospective_quality_control_contract(request: Mapping[str, Any]) -> dict[str, Any]:
    _validate_request(request)
    obj = request["object"]; rows = sorted((dict(row) for row in obj.get("metrics", [])), key=lambda row: row["metric_id"])
    metric_ids = {row["metric_id"] for row in rows}; required = set(request["required_metric_order"])
    passed: set[str] = set(); failed: set[str] = set(); unknown: set[str] = set(); unmeasured: set[str] = set(); blocked: set[str] = set(); missing: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for row in rows:
        ident = row["metric_id"]; provenance.add(row["provenance_digest"]); omissions.update(f"{ident}:{v}" for v in row.get("omission_order", [])); uncertainty.update(f"{ident}:{v}" for v in row.get("uncertainty_order", []))
        if row.get("negative_result") or row.get("evidence_state") == "negative": negative.add(f"{ident}:negative-result")
        if not row.get("policy_allowed", False) or not row.get("local", False) or not row.get("aggregate_only", False) or row.get("replay_identity") != request["replay_identity"]: blocked.add(ident); omissions.add(f"{ident}:policy-locality-or-replay")
        elif row.get("evidence_state") == "contradicted": failed.add(ident)
        elif row.get("evidence_state") in {"unknown", "speculative"}: unknown.add(ident); uncertainty.add(f"{ident}:unknown-evidence")
        elif row.get("evidence_state") == "unmeasured" or row.get("value") is None: unmeasured.add(ident); omissions.add(f"{ident}:unmeasured")
        elif row.get("threshold") is not None and float(row.get("value")) < float(row["threshold"]): failed.add(ident)
        elif row.get("evidence_state") not in {"proven", "supported"}: unknown.add(ident)
        else: passed.add(ident)
    for ident in required - metric_ids: metric_ids.add(ident); missing.add(ident); omissions.add(f"missing:{ident}")
    missing_modality = [] if all(v in obj.get("modality_order", []) for v in request.get("required_modality_order", [])) else sorted(request["required_modality_order"])
    if missing_modality: omissions.add("request:required-modality-missing")
    global_block = not all(request.get(k) is True for k in ("policy_allowed", "protected_closure", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(metric_ids); passed.clear(); failed.clear(); unknown.clear(); unmeasured.clear(); missing.clear(); omissions.add("request:quality-contract-gate-blocked")
    disposition = "blocked" if global_block or (blocked and not passed) else "unresolved" if failed or unknown or unmeasured or blocked or missing or missing_modality else "qualified"
    if disposition != "qualified": omissions.add("request:quality-closure-not-ready")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "consumer": request["consumer"], "object_id": obj["object_id"], "semantic_profile": obj["semantic_profile"], "disposition": disposition, "metric_order": sorted(metric_ids), "passed_order": sorted(passed), "failed_order": sorted(failed), "unknown_order": sorted(unknown), "unmeasured_order": sorted(unmeasured), "blocked_order": sorted(blocked), "missing_order": sorted(missing), "modality_order": sorted(obj.get("modality_order", [])), "missing_modality_order": missing_modality, "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); payload["verdict_digest"] = digest; payload["artifact"] = {"artifact": {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"scale-quality-verdict-2:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}, "semantic_loss_order": payload["omission_order"], "provenance_order": sorted(provenance)}
    validate_prospective_quality_control_contract(payload)
    return payload

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "prospective_quality_control_contract_manifest", "model_prospective_quality_control_contract", "validate_prospective_quality_control_contract"]
