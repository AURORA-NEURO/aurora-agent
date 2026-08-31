"""Python parity for ``AFA-ids-P09-F17`` experiment-design workbench."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
)

FEATURE_ID = "AFA-ids-P09-F17"
CONTRACT_VERSION = "ids-local-single-study-experiment-design-research-workbench/1.0"
INPUT_SCHEMA = "ExperimentDesignRequest4@1"
OUTPUT_SCHEMA = "DesignFrontier8@1"
CONTENT_TYPE = "application/vnd.aurora.design-frontier-8+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


@dataclass(frozen=True)
class DesignFrontier8:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if (
            v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or v.get("contract_version") != CONTRACT_VERSION
            or v.get("feature_id") != FEATURE_ID
            or v.get("boundary") != PRECLINICAL_BOUNDARY
            or v.get("raw_data_local") is not True
            or v.get("aggregate_only") is not True
            or not all(str(v.get(k, "")).strip() for k in ("request_id", "study_id", "requester", "purpose", "semantic_profile"))
            or int(v.get("checkpoint", 0)) <= 0
            or not v.get("candidate_order")
            or not v.get("effect_receipts")
            or v.get("disposition") not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("design identity, checkpoint, locality, candidates, or effects are incomplete")
        fields = (
            "candidate_order", "selected_order", "frontier_order", "unresolved_order", "blocked_order",
            "missing_control_order", "underpowered_order", "omission_order", "uncertainty_order",
            "negative_evidence_order", "effect_receipts",
        )
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("design ordering is not canonical")
        ids = set(v["candidate_order"])
        parts = [*v["selected_order"], *v["frontier_order"], *v["unresolved_order"], *v["blocked_order"]]
        if len(parts) != len(ids) or set(parts) != ids or len(set(parts)) != len(parts):
            raise ResearchContractError("design candidate states do not partition")
        artifact = v.get("artifact", {})
        digests = [v.get("replay_identity"), v.get("frontier_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if (
            not all(_digest(d) for d in digests)
            or len(v["selected_order"]) + len(v["frontier_order"]) != len(v.get("power_scores_milli", []))
            or len(v["selected_order"]) + len(v["frontier_order"]) != len(v.get("sample_sizes", []))
            or artifact.get("content_type") != CONTENT_TYPE
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("content_hash") != v.get("frontier_digest")
        ):
            raise ResearchContractError("design artifact, digest, or score cardinality is invalid")
        if any(not e.startswith("manage:local-capability:") and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("design effect is outside governed gate")


def experiment_design_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["experimental neuroscientist", "biostatistician", "research workbench operator"],
        "behavior": "ranks typed power-aware preclinical design summaries into a deterministic design frontier",
        "value": "makes design controls, power shortfalls, omissions, and uncertainty explicit before any laboratory action",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["manage:local-capability"],
        "permissions": ["read:local-research-artifacts"],
        "autonomy_tier": "A0",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def design_experiment(request: Mapping[str, Any], candidates: Sequence[Mapping[str, Any]]) -> DesignFrontier8:
    if (
        not all(str(request.get(k, "")).strip() for k in ("request_id", "study_id", "requester", "purpose", "semantic_profile"))
        or not request.get("required_controls")
        or not 0 <= int(request.get("minimum_power_milli", -1)) <= 1000
        or int(request.get("checkpoint", 0)) <= 0
        or int(request.get("budget_units", 0)) <= 0
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or not _digest(request.get("replay_identity"))
        or not candidates
    ):
        raise ResearchContractError("design identity, controls, power, checkpoint, budget, replay, locality, candidates, or boundary is invalid")
    rows = sorted(
        (dict(x) for x in candidates),
        key=lambda x: (-int(x.get("power_milli", 0)), -int(x.get("effect_milli", 0)), str(x.get("design_id", ""))),
    )
    ids = sorted(str(x.get("design_id", "")) for x in rows)
    if (
        len(set(ids)) != len(ids)
        or any(
            not x.get("design_id")
            or not x.get("estimand")
            or not x.get("study_id")
            or not x.get("origin")
            or not all(_digest(x.get(k)) for k in ("design_digest", "provenance_digest", "replay_identity"))
            for x in rows
        )
    ):
        raise ResearchContractError("design identity, uniqueness, origin, estimand, or digest is invalid")
    selected: set[str] = set(); frontier: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set()
    missing_controls: set[str] = set(); underpowered: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    rank = {x["design_id"]: (int(x.get("power_milli", 0)), int(x.get("sample_size", 0))) for x in rows}
    for x in rows:
        did = x["design_id"]
        if x.get("negative_result"): negative.add(f"{did}:negative-result")
        omissions.update(f"{did}:{r}" for r in x.get("omission_reasons", []))
        missing = [c for c in request["required_controls"] if c not in x.get("control_ids", [])]
        if missing: missing_controls.add(f"{did}:missing:{len(missing)}")
        if rank[did][0] < int(request["minimum_power_milli"]): underpowered.add(did)
        reasons = []
        if x.get("study_id") != request["study_id"]: reasons.append("study-mismatch")
        if x.get("semantic_profile") != request["semantic_profile"]: reasons.append("semantic-profile-mismatch")
        if missing: reasons.append("control-closure-incomplete")
        if rank[did][0] < int(request["minimum_power_milli"]): reasons.append("power-threshold-failed")
        if x.get("replay_identity") != request["replay_identity"]: reasons.append("replay-identity-mismatch")
        if x.get("signed") is not True or x.get("permitted") is not True: reasons.append("authorization-missing")
        if x.get("raw_data_local") is not True or x.get("aggregate_only") is not True: reasons.append("locality-or-aggregate-only-failed")
        if x.get("evidence_state") == "contradicted": blocked.add(did); negative.add(f"{did}:contradicted")
        elif x.get("evidence_state") not in {"proven", "supported"} or reasons: unresolved.add(did); uncertainty.add(f"{did}:unresolved")
        elif not selected: selected.add(did)
        else: frontier.add(did)
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    disposition = "blocked" if global_block or blocked else "unresolved" if not selected else "qualified"
    if global_block: blocked.update(ids); selected.clear(); frontier.clear(); unresolved.clear()
    if disposition != "qualified": omissions.add("request:design-gates-incomplete")
    ordered_ids = sorted(selected | frontier)
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request["request_id"], "study_id": request["study_id"], "requester": request["requester"], "purpose": request["purpose"],
        "semantic_profile": request["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition,
        "candidate_order": ids, "selected_order": sorted(selected), "frontier_order": sorted(frontier), "unresolved_order": sorted(unresolved),
        "blocked_order": sorted(blocked), "missing_control_order": sorted(missing_controls), "underpowered_order": sorted(underpowered),
        "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative),
        "power_scores_milli": [rank[x][0] for x in ordered_ids], "sample_sizes": [rank[x][1] for x in ordered_ids],
        "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    result = {
        **payload,
        "frontier_digest": digest,
        "artifact": {"artifact_id": f"design-frontier-8:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance_digests": sorted({x["provenance_digest"] for x in rows}), "boundary": PRECLINICAL_BOUNDARY},
        "effect_receipts": [f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"],
        "raw_data_local": True, "aggregate_only": True,
    }
    receipt = DesignFrontier8(result); receipt.validate(); return receipt


def idsExperimentDesignDigest(receipt: DesignFrontier8) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "DesignFrontier8", "experiment_design_manifest", "design_experiment", "idsExperimentDesignDigest"]
