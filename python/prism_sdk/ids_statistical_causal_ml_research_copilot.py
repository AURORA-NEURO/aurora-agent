"""Python parity for ``AFA-ids-P13-F09`` statistical/causal/ML research copilot."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
)

FEATURE_ID = "AFA-ids-P13-F09"
CONTRACT_VERSION = "ids-local-single-study-statistical-causal-ml-research-copilot/1.0"
INPUT_SCHEMA = "AnalysisCopilotRequest7@1"
OUTPUT_SCHEMA = "QualifiedAnalysisResult10@1"
CONTENT_TYPE = "application/vnd.aurora.qualified-analysis-result-10+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def _nonempty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


@dataclass(frozen=True)
class QualifiedAnalysisResult10:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value
        if (
            value.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or value.get("contract_version") != CONTRACT_VERSION
            or value.get("feature_id") != FEATURE_ID
            or value.get("boundary") != PRECLINICAL_BOUNDARY
            or value.get("raw_data_local") is not True
            or value.get("aggregate_only") is not True
            or not all(_nonempty(value.get(key)) for key in ("request_id", "study_id", "requester", "purpose", "semantic_profile", "model_portfolio_version"))
            or int(value.get("checkpoint", 0)) <= 0
            or not value.get("candidate_order")
            or not value.get("effect_receipts")
            or value.get("disposition") not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("analysis identity, checkpoint, locality, candidates, or effects are incomplete")
        ordered_fields = (
            "candidate_order", "selected_order", "fallback_order", "unresolved_order", "blocked_order",
            "missing_study_order", "underpowered_order", "high_missingness_order", "non_robust_order",
            "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts",
        )
        if any(not _ordered(value.get(key, [])) for key in ordered_fields):
            raise ResearchContractError("analysis ordering is not canonical")
        candidates = set(value["candidate_order"])
        if len(candidates) != len(value["candidate_order"]):
            raise ResearchContractError("analysis candidate ids are not unique")
        parts = [
            *value.get("selected_order", []), *value.get("fallback_order", []),
            *value.get("unresolved_order", []), *value.get("blocked_order", []),
        ]
        if set(parts) != candidates or len(set(parts)) != len(parts):
            raise ResearchContractError("analysis candidate states do not partition")
        if len(value.get("selected_order", [])) + len(value.get("fallback_order", [])) != len(value.get("score_order", [])) or len(value.get("score_order", [])) != len(value.get("sample_size_order", [])):
            raise ResearchContractError("analysis score and sample-size cardinality is inconsistent")
        artifact = value.get("artifact", {})
        digests = [value.get("replay_identity"), value.get("analysis_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if (
            not all(_digest(item) for item in digests)
            or artifact.get("content_type") != CONTENT_TYPE
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("content_hash") != value.get("analysis_digest")
        ):
            raise ResearchContractError("analysis artifact metadata or digest is inconsistent")
        if any(not item.startswith(("exchange:permitted-summaries:", "manage:local-capability:")) and item != "block:unsafe-release" for item in value["effect_receipts"]):
            raise ResearchContractError("effect is outside the governed analysis gate")


def statistical_causal_ml_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["computational biologist", "biostatistician", "downstream AURORA crate maintainer", "research workbench operator"],
        "behavior": "compiles typed local preclinical analysis candidates into a deterministic, replayable qualified-analysis plan",
        "value": "selects a reproducible method portfolio while exposing power, missingness, robustness, uncertainty, provenance, and policy gates before model execution",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["manage:local-capability", "exchange:permitted-summaries"],
        "permissions": ["read:local-analysis-manifests"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _score(candidate: Mapping[str, Any]) -> int:
    return (
        int(candidate["robustness_milli"]) * 4
        + (1000 - int(candidate["uncertainty_milli"])) * 2
        + max(0, 1000 - int(candidate["missingness_milli"]))
        + min(abs(int(candidate["effect_milli"])), 1000)
        + min(int(candidate["sample_size"]), 1000)
    )


def _validate_request(request: Mapping[str, Any]) -> None:
    if (
        not all(_nonempty(request.get(key)) for key in ("request_id", "study_id", "requester", "purpose", "semantic_profile", "model_portfolio_version"))
        or not request.get("candidates")
        or len(request["candidates"]) > 4096
        or int(request.get("checkpoint", 0)) <= 0
        or int(request.get("max_budget_units", 0)) <= 0
        or int(request.get("minimum_candidate_quorum", 0)) <= 0
        or int(request["minimum_candidate_quorum"]) > len(request["candidates"])
        or not _digest(request.get("replay_identity"))
        or not 0 <= int(request.get("maximum_missingness_milli", -1)) <= 1000
        or not 0 <= int(request.get("minimum_robustness_milli", -1)) <= 1000
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
    ):
        raise ResearchContractError("request identity, bounds, candidates, thresholds, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for candidate in request["candidates"]:
        if (
            not all(_nonempty(candidate.get(key)) for key in ("candidate_id", "study_id", "estimand", "method_family", "input_schema", "output_schema"))
            or candidate["candidate_id"] in ids
            or not candidate.get("feature_ids")
            or candidate["feature_ids"] != sorted(set(candidate["feature_ids"]))
            or any(not _nonempty(item) for item in candidate["feature_ids"])
            or not all(_digest(candidate.get(key)) for key in ("artifact_digest", "provenance_digest", "replay_identity"))
            or int(candidate.get("estimated_units", 0)) <= 0
            or int(candidate.get("sample_size", 0)) <= 0
            or not 0 <= int(candidate.get("missingness_milli", -1)) <= 1000
            or not 0 <= int(candidate.get("uncertainty_milli", -1)) <= 1000
            or not 0 <= int(candidate.get("robustness_milli", -1)) <= 1000
        ):
            raise ResearchContractError("candidate identity, feature contract, bounds, or digest is invalid")
        ids.add(candidate["candidate_id"])


def compile_statistical_causal_ml(request: Mapping[str, Any]) -> QualifiedAnalysisResult10:
    _validate_request(request)
    candidates = sorted((dict(item) for item in request["candidates"]), key=lambda item: item["candidate_id"])
    candidate_order = [item["candidate_id"] for item in candidates]
    by_id = {item["candidate_id"]: item for item in candidates}
    selected: set[str] = set()
    fallback: set[str] = set()
    unresolved: set[str] = set()
    blocked: set[str] = set()
    missing_study: set[str] = set()
    underpowered: set[str] = set()
    high_missingness: set[str] = set()
    non_robust: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    eligible: list[tuple[int, str]] = []
    total_units = 0
    for candidate in candidates:
        cid = candidate["candidate_id"]
        total_units += int(candidate["estimated_units"])
        if candidate.get("negative_result"):
            negative.add(f"{cid}:negative-result")
        if candidate["study_id"] != request["study_id"]:
            missing_study.add(cid)
            blocked.add(cid)
            continue
        if candidate.get("local_only") is not True or candidate.get("protected_closure") is not True:
            blocked.add(cid)
            if candidate.get("local_only") is not True:
                omissions.add(f"{cid}:raw-data-not-local")
            if candidate.get("protected_closure") is not True:
                uncertainty.add(f"{cid}:protected-closure-incomplete")
            continue
        if candidate.get("evidence_state") == "contradicted":
            blocked.add(cid)
            negative.add(f"{cid}:contradicted")
            continue
        if (
            candidate.get("replay_identity") != request["replay_identity"]
            or candidate.get("deterministic") is not True
            or candidate.get("permitted") is not True
            or candidate.get("signed") is not True
        ):
            unresolved.add(cid)
            omissions.add(f"{cid}:replay-or-authorization")
            continue
        if candidate.get("evidence_state") not in {"proven", "supported"}:
            unresolved.add(cid)
            uncertainty.add(f"{cid}:evidence-state")
            continue
        disqualified = False
        if int(candidate["sample_size"]) < int(request["minimum_sample_size"]):
            underpowered.add(cid)
            unresolved.add(cid)
            disqualified = True
        if int(candidate["missingness_milli"]) > int(request["maximum_missingness_milli"]):
            high_missingness.add(cid)
            unresolved.add(cid)
            disqualified = True
        if int(candidate["robustness_milli"]) < int(request["minimum_robustness_milli"]):
            non_robust.add(cid)
            unresolved.add(cid)
            disqualified = True
        if disqualified:
            uncertainty.add(f"{cid}:acceptance-threshold")
        else:
            eligible.append((_score(candidate), cid))
    eligible.sort(key=lambda pair: (-pair[0], pair[1]))
    if eligible:
        selected.add(eligible[0][1])
        fallback.update(item[1] for item in eligible[1:])
    if total_units > int(request["max_budget_units"]):
        omissions.add(f"request:budget-exceeded:{total_units}")
    if len(eligible) < int(request["minimum_candidate_quorum"]):
        uncertainty.add("candidate:minimum-quorum-unmet")
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if request.get("policy_allow") is not True:
        negative.add("request:policy-denied")
    if request.get("protected_closure") is not True:
        uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True:
        uncertainty.add("request:signed-approval-missing")
    if global_block:
        blocked.update(candidate_order)
        selected.clear()
        fallback.clear()
        unresolved.clear()
        omissions.add("request:analysis-not-authorized")
    if global_block or (not selected and blocked):
        disposition = "blocked"
    elif not selected or total_units > int(request["max_budget_units"]) or len(eligible) < int(request["minimum_candidate_quorum"]):
        disposition = "unresolved"
    else:
        disposition = "qualified"
    if disposition != "qualified":
        omissions.add("request:analysis-not-release-ready")
    selected_order = sorted(selected)
    fallback_order = sorted(fallback)
    unresolved_order = sorted(unresolved)
    blocked_order = sorted(blocked)
    score_map = {cid: score for score, cid in eligible}
    plan_ids = [*selected_order, *fallback_order]
    score_order = [score_map.get(cid, 0) for cid in plan_ids]
    sample_size_order = [int(by_id[cid]["sample_size"]) for cid in plan_ids]
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request["request_id"],
        "study_id": request["study_id"],
        "requester": request["requester"],
        "purpose": request["purpose"],
        "semantic_profile": request["semantic_profile"],
        "model_portfolio_version": request["model_portfolio_version"],
        "checkpoint": int(request["checkpoint"]),
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "fallback_order": fallback_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_study_order": sorted(missing_study),
        "underpowered_order": sorted(underpowered),
        "high_missingness_order": sorted(high_missingness),
        "non_robust_order": sorted(non_robust),
        "omission_order": sorted(omissions),
        "uncertainty_order": sorted(uncertainty),
        "negative_evidence_order": sorted(negative),
        "score_order": score_order,
        "sample_size_order": sample_size_order,
        "total_units": total_units,
        "replay_identity": request["replay_identity"],
        "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    result = {
        **payload,
        "analysis_digest": digest,
        "artifact": {
            "artifact_id": f"qualified-analysis-result-10:{request['request_id']}",
            "content_type": CONTENT_TYPE,
            "content_hash": digest,
            "semantic_loss": sorted(omissions),
            "provenance_digests": sorted({item["provenance_digest"] for item in candidates}),
            "boundary": PRECLINICAL_BOUNDARY,
        },
        "effect_receipts": sorted(
            [f"exchange:permitted-summaries:{request['request_id']}", f"manage:local-capability:{request['request_id']}"]
            if disposition == "qualified" else ["block:unsafe-release"]
        ),
        "raw_data_local": True,
        "aggregate_only": True,
    }
    receipt = QualifiedAnalysisResult10(result)
    receipt.validate()
    return receipt


def idsStatisticalCausalMlDigest(receipt: QualifiedAnalysisResult10) -> str:
    receipt.validate()
    return _hash(receipt.to_dict())


__all__ = [
    "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE",
    "QualifiedAnalysisResult10", "statistical_causal_ml_manifest", "compile_statistical_causal_ml",
    "idsStatisticalCausalMlDigest",
]
