"""Deterministic Python parity for Worldgen P23 evaluation/observability."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

CONTENT_TYPE = "application/vnd.aurora.worldgen.evaluation-observability-receipt-1+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "worldgen",
        "consumers": ["evaluation scientist", "benchmark steward", "research program lead", "release operator"],
        "behavior": f"evaluate benchmark observations with baseline and uncertainty witnesses at {scale} ({mode} scale)",
        "value": "makes release claims falsifiable, replayable, and honest about null, unknown, and negative outcomes",
        "input_schema": "EvaluationRequest4@1",
        "output_schema": "EvaluationCard8@1",
        "effects": ["emit:evaluation-card", "block:unsafe-release"],
        "permissions": ["read:local-evaluation-observations"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def validate(output: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = output.get("artifact", {})
    if (
        output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or feature_id is not None and output.get("feature_id") != feature_id
        or output.get("boundary") != PRECLINICAL_BOUNDARY
        or output.get("raw_data_local") is not True
        or output.get("aggregate_only") is not True
        or not output.get("observation_order")
        or not _digest(output.get("replay_identity"))
        or not _digest(output.get("evaluation_digest"))
        or artifact.get("content_type") != CONTENT_TYPE
        or artifact.get("content_hash") != output.get("evaluation_digest")
        or artifact.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("semantic_loss", []) != output.get("omitted_order", [])
    ):
        raise ResearchContractError("evaluation card identity, locality, digest, artifact, or boundary is incomplete")
    for key in (
        "observation_order", "passed_order", "failed_order", "unknown_order", "unmeasured_order",
        "contradicted_order", "omitted_order", "baseline_delta_order", "uncertainty_order",
        "negative_evidence_order", "site_order", "metric_order", "effect_receipts",
    ):
        if not _ordered(output.get(key, [])):
            raise ResearchContractError("evaluation vectors are not canonical")
    observations = set(output["observation_order"])
    parts = set(output.get("passed_order", [])) | set(output.get("failed_order", [])) | set(output.get("unknown_order", [])) | set(output.get("unmeasured_order", [])) | set(output.get("contradicted_order", [])) | set(output.get("omitted_order", []))
    if observations != parts:
        raise ResearchContractError("evaluation states do not partition")


def evaluate(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    required = ("request_id", "scope", "benchmark_id")
    if (
        any(not isinstance(request.get(key), str) or not request[key].strip() for key in required)
        or not request.get("observations")
        or not request.get("required_metric_order")
        or not isinstance(request.get("min_pass_fraction_milli"), int)
        or not 0 <= request["min_pass_fraction_milli"] <= 1000
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or not _ordered(request.get("required_metric_order", []))
        or not _ordered(request.get("adversarial_events", []))
    ):
        raise ResearchContractError("evaluation request identity, metrics, digest, ordering, locality, or boundary is invalid")
    rows = sorted(request["observations"], key=lambda row: row.get("observation_id", ""))
    order: list[str] = []
    passed: set[str] = set(); failed: set[str] = set(); unknown: set[str] = set(); unmeasured: set[str] = set(); contradicted: set[str] = set(); omitted: set[str] = set()
    baseline: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); sites: set[str] = set(); metrics: set[str] = set(); provenance: set[str] = set()
    for row in rows:
        observation_id = row.get("observation_id", "")
        metric = row.get("metric", "")
        if observation_id in order or not isinstance(observation_id, str) or not observation_id.strip() or not isinstance(metric, str) or not metric.strip() or not isinstance(row.get("independent_site"), str) or not row["independent_site"].strip() or not _digest(row.get("provenance_digest")) or not _digest(row.get("replay_identity")):
            raise ResearchContractError("observation identity or digest is invalid")
        order.append(observation_id); sites.add(row["independent_site"]); metrics.add(metric); provenance.add(row["provenance_digest"])
        baseline.add(f"{observation_id}:delta={row.get('value_milli', 0) - row.get('baseline_milli', 0)}")
        if row.get("uncertainty_milli", 0) > 0: uncertainty.add(f"{observation_id}:uncertainty={row['uncertainty_milli']}")
        if row.get("negative_result") is True: negative.add(f"{observation_id}:negative-result")
        if row.get("replay_identity") != request["replay_identity"] or row.get("local") is not True or row.get("aggregate_only") is not True:
            omitted.add(observation_id)
        else:
            state = row.get("evidence_state")
            if state in {"proven", "supported"} and row.get("value_milli", 0) >= row.get("baseline_milli", 0): passed.add(observation_id)
            elif state in {"proven", "supported"}: failed.add(observation_id)
            elif state == "unknown": unknown.add(observation_id)
            elif state == "unmeasured": unmeasured.add(observation_id)
            elif state == "contradicted": contradicted.add(observation_id)
            else: unknown.add(observation_id)
    global_block = (not request.get("policy_allowed") or not request.get("protected_closure") or not request.get("signed_approval") or not request.get("network_available") or not request.get("raw_data_local") or not request.get("aggregate_only") or bool(request.get("adversarial_events")) or (mode == "copilot" and (request.get("action_budget", 0) <= 0 or request.get("action_count", 0) > request.get("action_budget", 0))))
    if global_block:
        omitted.update(order); passed.clear(); failed.clear(); unknown.clear(); unmeasured.clear(); contradicted.clear(); uncertainty.add("request:global-evaluation-gate-blocked")
    p, f, u, um, c, om = (sorted(values) for values in (passed, failed, unknown, unmeasured, contradicted, omitted))
    fraction = len(p) * 1000 // max(1, len(order))
    disposition = "blocked" if global_block else "unresolved" if not p or fraction < request["min_pass_fraction_milli"] or any((f, u, um, c, om)) else "qualified"
    payload: dict[str, Any] = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id,
        "mode": mode, "scale": scale, "request_id": request["request_id"], "benchmark_id": request["benchmark_id"], "disposition": disposition,
        "observation_order": order, "passed_order": p, "failed_order": f, "unknown_order": u, "unmeasured_order": um, "contradicted_order": c, "omitted_order": om,
        "baseline_delta_order": sorted(baseline), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "site_order": sorted(sites), "metric_order": sorted(metrics),
        "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    evaluation_digest = _hash(payload); payload["evaluation_digest"] = evaluation_digest
    payload["artifact"] = {"artifact_id": f"worldgen-evaluation-card:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": evaluation_digest, "semantic_loss": om, "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    payload["effect_receipts"] = [f"emit:evaluation-card:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    validate(payload, feature_id=feature_id)
    return payload


EvaluationRequest4 = dict[str, Any]
EvaluationCard8 = dict[str, Any]
EvaluationObservabilityError = ResearchContractError
__all__ = ["CONTENT_TYPE", "EvaluationRequest4", "EvaluationCard8", "EvaluationObservabilityError", "manifest", "evaluate", "validate"]
