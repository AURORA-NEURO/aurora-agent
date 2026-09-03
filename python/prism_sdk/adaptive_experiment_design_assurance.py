"""Python parity for ``AFA-adaptive-P09-F27`` experiment-design assurance."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-adaptive-P09-F27"
CONTRACT_VERSION = "adaptive-prospective-high-throughput-experiment-design-assurance/1.0"
INPUT_SCHEMA = "ExperimentDesignRequest7@1"
OUTPUT_SCHEMA = "ExperimentDesignAssuranceReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.experiment-design-assurance-receipt-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ExperimentDesignAssuranceReceipt9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        artifact = v.get("artifact", {})
        if (
            v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or v.get("contract_version") != CONTRACT_VERSION
            or v.get("feature_id") != FEATURE_ID
            or v.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("content_type") != CONTENT_TYPE
            or v.get("raw_data_local") is not True
            or v.get("aggregate_only") is not True
            or int(v.get("checkpoint", 0)) <= 0
            or v.get("disposition") not in {"qualified", "unresolved", "blocked"}
            or not all(str(v.get(k, "")).strip() for k in ("request_id", "federation_id", "researcher", "purpose", "semantic_profile"))
            or not v.get("candidate_order")
            or not v.get("ranked_order")
            or not v.get("peer_order")
            or not v.get("effect_receipts")
        ):
            raise ResearchContractError("design identity, checkpoint, locality, candidates, peers, or effects are incomplete")
        fields = (
            "candidate_order", "ranked_order", "selected_order", "alternative_order", "unresolved_order", "blocked_order",
            "missing_candidate_order", "missing_study_order", "missing_modality_order", "peer_order", "qualified_peer_order",
            "missing_peer_order", "power_witness_order", "variance_witness_order", "attrition_witness_order",
            "replication_witness_order", "omission_order", "uncertainty_order", "contradiction_order", "negative_evidence_order",
            "effect_receipts",
        )
        if any(not _ordered(v.get(key, [])) for key in fields):
            raise ResearchContractError("experiment-design ordering is not canonical")
        universe = set(v["candidate_order"])
        parts = set(v["selected_order"]) | set(v["alternative_order"]) | set(v["unresolved_order"]) | set(v["blocked_order"]) | set(v["missing_candidate_order"])
        if len(universe) != len(v["candidate_order"]) or universe != parts:
            raise ResearchContractError("design candidates do not partition")
        ranked = set(v["ranked_order"])
        if len(ranked) != len(v["ranked_order"]) or not ranked.issubset(universe):
            raise ResearchContractError("design ranking is not a candidate subset")
        peers = set(v["peer_order"])
        if len(peers) != len(v["peer_order"]) or peers != set(v["qualified_peer_order"]) | set(v["missing_peer_order"]):
            raise ResearchContractError("design peers do not partition")
        if not all(_digest(item) for item in (v.get("replay_identity"), v.get("design_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", []))) or artifact.get("content_hash") != v.get("design_digest"):
            raise ResearchContractError("design artifact digest is invalid")


def experiment_design_assurance_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "adaptive",
        "consumers": ["computational biologist", "power-design reviewer", "federation steward"],
        "behavior": "qualifies prospective power-aware experiment designs and peer attestations under explicit threshold and governance gates",
        "value": "prevents underpowered, high-variance, or non-reproducible design candidates from silently entering a preclinical workflow",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["retain:experiment-design-assurance", "exchange:aggregate-design-summary"],
        "permissions": ["retain:design-evidence", "exchange:aggregate-design"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def assure_experiment_design(q: Mapping[str, Any]) -> ExperimentDesignAssuranceReceipt9:
    required_keys = ("request_id", "federation_id", "researcher", "purpose", "semantic_profile")
    if (
        not all(str(q.get(key, "")).strip() for key in required_keys)
        or not q.get("required_study_order")
        or not q.get("required_modality_order")
        or not q.get("candidates")
        or not q.get("peers")
        or int(q.get("checkpoint", 0)) <= 0
        or int(q.get("minimum_peer_quorum", 0)) <= 0
        or any(int(q.get(key, 0)) > 10000 for key in ("minimum_power_milli", "maximum_variance_milli", "maximum_attrition_milli", "minimum_replication_milli"))
        or q.get("boundary") != PRECLINICAL_BOUNDARY
        or q.get("raw_data_local") is not True
        or q.get("aggregate_only") is not True
        or not _digest(q.get("replay_identity"))
    ):
        raise ResearchContractError("design identity, bounds, candidates, peers, replay, locality, or boundary is invalid")
    candidates = sorted((dict(item) for item in q["candidates"]), key=lambda item: (-int(item.get("power_milli", 0)), int(item.get("variance_milli", 0)), int(item.get("attrition_milli", 0)), -int(item.get("replication_milli", 0)), str(item.get("candidate_id", ""))))
    candidate_ids = [str(item.get("candidate_id", "")) for item in candidates]
    if len(set(candidate_ids)) != len(candidate_ids) or any(not item.get("candidate_id") or not item.get("design_id") or not item.get("study_id") or not item.get("modality") or not item.get("semantic_profile") or any(int(item.get(key, 0)) > 10000 for key in ("power_milli", "variance_milli", "attrition_milli", "replication_milli")) or not all(_digest(item.get(key)) for key in ("artifact_digest", "provenance_digest", "replay_identity")) or item.get("replay_identity") != q["replay_identity"] for item in candidates):
        raise ResearchContractError("candidate identity, metrics, digests, or replay is invalid")
    studies = {item["study_id"] for item in candidates}; modalities = {item["modality"] for item in candidates}
    missing_studies = set(q["required_study_order"]) - studies; missing_modalities = set(q["required_modality_order"]) - modalities
    missing_candidates = missing_studies | missing_modalities
    candidate_order = sorted(set(candidate_ids) | missing_candidates)
    ranked_order = sorted(set(candidate_ids))
    selected: set[str] = set(); alternatives: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); power_witness: set[str] = set(); variance_witness: set[str] = set(); attrition_witness: set[str] = set(); replication_witness: set[str] = set(); uncertainty: set[str] = set(); contradiction: set[str] = set(); negative: set[str] = set(); selected_design_id: str | None = None
    for item in candidates:
        candidate_id = item["candidate_id"]
        if item.get("negative_result"): negative.add(f"{candidate_id}:negative-result")
        state = item.get("evidence_state")
        if state == "contradicted": blocked.add(candidate_id); contradiction.add(f"{candidate_id}:contradicted"); continue
        if state in {"unknown", "speculative"}: unresolved.add(candidate_id); uncertainty.add(f"{candidate_id}:evidence-state"); continue
        power_ok = int(item["power_milli"]) >= int(q["minimum_power_milli"]); variance_ok = int(item["variance_milli"]) <= int(q["maximum_variance_milli"]); attrition_ok = int(item["attrition_milli"]) <= int(q["maximum_attrition_milli"]); replication_ok = int(item["replication_milli"]) >= int(q["minimum_replication_milli"])
        if not power_ok: power_witness.add(f"{candidate_id}:power-below-threshold")
        if not variance_ok: variance_witness.add(f"{candidate_id}:variance-above-threshold")
        if not attrition_ok: attrition_witness.add(f"{candidate_id}:attrition-above-threshold")
        if not replication_ok: replication_witness.add(f"{candidate_id}:replication-below-threshold")
        if state in {"proven", "supported"} and power_ok and variance_ok and attrition_ok and replication_ok and item.get("independent_source") is True and item.get("local_data") is True and item.get("policy_allowed") is True and item.get("semantic_profile") == q["semantic_profile"]:
            if not selected: selected.add(candidate_id); selected_design_id = item["design_id"]
            else: alternatives.add(candidate_id)
        else:
            unresolved.add(candidate_id)
            for condition, marker in ((not item.get("independent_source"), "independence-missing"), (not item.get("local_data"), "locality-missing"), (not item.get("policy_allowed"), "policy-not-allowed"), (item.get("semantic_profile") != q["semantic_profile"], "semantic-profile-mismatch")):
                if condition: uncertainty.add(f"{candidate_id}:{marker}")
    peer_rows = sorted((dict(item) for item in q["peers"]), key=lambda item: str(item.get("peer_id", "")))
    peer_order = [str(item.get("peer_id", "")) for item in peer_rows]
    qualified_peers = {item["peer_id"] for item in peer_rows if selected_design_id == item.get("design_id") and item.get("semantic_profile") == q["semantic_profile"] and int(item.get("checkpoint", 0)) == int(q["checkpoint"]) and int(item.get("power_milli", 0)) >= int(q["minimum_power_milli"]) and item.get("signed") is True and item.get("aggregate_only") is True and item.get("raw_data_local") is True and item.get("evidence_state") in {"proven", "supported"}}
    missing_peers = set(peer_order) - qualified_peers; uncertainty |= {f"peer:{item}:not-qualified" for item in missing_peers}
    omissions = {f"study:{item}:missing" for item in missing_studies} | {f"modality:{item}:missing" for item in missing_modalities}
    global_block = not all(q.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if q.get("policy_allow") is not True: negative.add("request:policy-denied")
    if q.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if q.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if q.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    disposition = "blocked" if global_block or blocked else "unresolved" if not selected or missing_studies or missing_modalities or unresolved or len(qualified_peers) < int(q["minimum_peer_quorum"]) else "qualified"
    if disposition != "qualified": omissions.add("request:design-not-release-ready")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": q["request_id"], "federation_id": q["federation_id"], "researcher": q["researcher"], "purpose": q["purpose"], "semantic_profile": q["semantic_profile"], "checkpoint": int(q["checkpoint"]), "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "selected_order": sorted(selected), "alternative_order": sorted(alternatives), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "missing_candidate_order": sorted(missing_candidates), "missing_study_order": sorted(missing_studies), "missing_modality_order": sorted(missing_modalities), "peer_order": peer_order, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers), "power_witness_order": sorted(power_witness), "variance_witness_order": sorted(variance_witness), "attrition_witness_order": sorted(attrition_witness), "replication_witness_order": sorted(replication_witness), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "contradiction_order": sorted(contradiction), "negative_evidence_order": sorted(negative), "replay_identity": q["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    result = {**payload, "design_digest": digest, "artifact": {"artifact_id": f"experiment-design-assurance-receipt-9:{q['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance_digests": sorted({item["provenance_digest"] for item in candidates}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": [f"retain:experiment-design-assurance:{q['request_id']}", f"exchange:aggregate-design-summary:{q['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"], "raw_data_local": True, "aggregate_only": True}
    receipt = ExperimentDesignAssuranceReceipt9(result); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "ExperimentDesignAssuranceReceipt9", "experiment_design_assurance_manifest", "assure_experiment_design"]
