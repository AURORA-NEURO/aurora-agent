"""Python parity for ``AFA-routing-P12-F12``.

The route is a deterministic, evidence-conditioned choice among typed local execution plans.
It emits a receipt only; execution and raw-data federation remain outside this capability.
"""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-routing-P12-F12"
CONTRACT_VERSION = "routing-federated-continual-computational-execution-copilot/1.0"
INPUT_SCHEMA = "ExecutionPlanSet6@1"
OUTPUT_SCHEMA = "ExecutionRoutingReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.routing-execution-routing-receipt-9+json"
MAX_PLANS = 256
MAX_PEERS = 128


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ExecutionRoutingReceipt9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        artifact = v.get("artifact", {})
        identity = (
            v.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION
            and v.get("contract_version") == CONTRACT_VERSION
            and v.get("feature_id") == FEATURE_ID
            and v.get("boundary") == PRECLINICAL_BOUNDARY
            and artifact.get("boundary") == PRECLINICAL_BOUNDARY
            and v.get("raw_data_local") is True
            and v.get("aggregate_only") is True
            and all(isinstance(v.get(k), str) and v[k].strip() for k in ("request_id", "task_id", "purpose", "semantic_profile", "replay_identity", "disposition"))
            and v.get("disposition") in {"qualified", "unresolved", "blocked"}
        )
        if not identity or not v.get("candidate_order") or not v.get("ranked_order") or not v.get("effect_order") or not v.get("effect_receipts"):
            raise ResearchContractError("execution identity, candidates, effects, locality, or disposition is incomplete")
        ordered_keys = ("required_study_order", "required_modality_order", "candidate_order", "selected_order", "unresolved_order", "blocked_order", "missing_study_order", "missing_modality_order", "qualified_peer_order", "missing_peer_order", "evidence_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in ordered_keys):
            raise ResearchContractError("execution ordering is not canonical")
        candidates = v["candidate_order"]
        ranked = v["ranked_order"]
        parts = v["selected_order"] + v["unresolved_order"] + v["blocked_order"]
        if len(set(candidates)) != len(candidates) or len(ranked) != len(candidates) or set(ranked) != set(candidates) or len(parts) != len(candidates) or set(parts) != set(candidates) or len(v.get("score_milli_order", [])) != len(candidates) or set(v.get("qualified_peer_order", [])) & set(v.get("missing_peer_order", [])):
            raise ResearchContractError("execution states, ranking, peers, or scores do not partition")
        if len(v["selected_order"]) > 1 or not _digest(v.get("routing_digest")) or artifact.get("content_hash") != v.get("routing_digest") or artifact.get("content_type") != CONTENT_TYPE or any(not _digest(d) for d in artifact.get("provenance_digests", [])):
            raise ResearchContractError("execution digest or artifact metadata is invalid")
        if any(not e.startswith(("manage:local-capability:", "route:execution-plan:")) and e != "block:execution-effects" for e in v["effect_receipts"]):
            raise ResearchContractError("effect is outside governed execution gate")

    def digest(self) -> str:
        self.validate()
        return _hash(self.to_dict())


def federated_execution_copilot_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "routing",
        "consumers": ["research program lead", "execution planner", "federation steward"],
        "behavior": "rank typed institution-local execution plans using evidence, risk, replay identity, and federated peer quorum, then emit an approval-aware routing receipt without dispatching effects",
        "value": "turn federated computational execution into an auditable, reproducible choice while keeping raw experimental data local and unresolved evidence explicit",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["read_local_data", "write_local_artifact"],
        "permissions": ["route:local-execution-plan", "read:federated-aggregate"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(r: Mapping[str, Any]) -> None:
    if not all(isinstance(r.get(k), str) and r[k].strip() for k in ("request_id", "task_id", "purpose", "semantic_profile", "replay_identity")) or not r.get("candidates") or len(r["candidates"]) > MAX_PLANS or len(r.get("peers", [])) > MAX_PEERS or not isinstance(r.get("minimum_peer_quorum"), int) or r["minimum_peer_quorum"] < 1 or r["minimum_peer_quorum"] > len(r.get("peers", [])) or r.get("boundary") != PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True or not _ordered(r.get("required_study_order", [])) or not _ordered(r.get("required_modality_order", [])):
        raise ResearchContractError("execution identity, requirements, bounds, quorum, replay, locality, or boundary is invalid")
    candidate_ids: set[str] = set()
    for c in r["candidates"]:
        if not isinstance(c, Mapping) or not isinstance(c.get("plan_id"), str) or not c["plan_id"].strip() or c["plan_id"] in candidate_ids or not isinstance(c.get("study_id"), str) or not c["study_id"].strip() or not isinstance(c.get("modality"), str) or not c["modality"].strip() or c.get("semantic_profile") != r["semantic_profile"] or c.get("replay_identity") != r["replay_identity"] or not isinstance(c.get("expected_discovery_milli"), int) or not 0 <= c["expected_discovery_milli"] <= 1000 or not isinstance(c.get("risk_milli"), int) or not 0 <= c["risk_milli"] <= 1000 or c.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"} or not _digest(c.get("workflow_digest")) or not _digest(c.get("artifact_digest")) or not _digest(c.get("provenance_digest")) or c.get("local") is not True or c.get("aggregate_only") is not True:
            raise ResearchContractError(f"candidate {c.get('plan_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        candidate_ids.add(c["plan_id"])
    peer_ids: set[str] = set()
    for p in r.get("peers", []):
        if not isinstance(p, Mapping) or not isinstance(p.get("peer_id"), str) or not p["peer_id"].strip() or p["peer_id"] in peer_ids or not isinstance(p.get("plan_id"), str) or not p["plan_id"].strip() or p.get("semantic_profile") is None or not isinstance(p.get("semantic_profile"), str) or not p["semantic_profile"].strip() or not isinstance(p.get("utility_milli"), int) or not 0 <= p["utility_milli"] <= 1000 or p.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"} or not isinstance(p.get("authorized"), bool) or not _digest(p.get("artifact_digest")) or not _digest(p.get("provenance_digest")) or not isinstance(p.get("replay_identity"), str) or not p["replay_identity"].strip() or p.get("local") is not True or p.get("aggregate_only") is not True:
            raise ResearchContractError(f"peer {p.get('peer_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        peer_ids.add(p["peer_id"])


def route_federated_execution(r: Mapping[str, Any]) -> ExecutionRoutingReceipt9:
    _validate_request(r)
    candidates = sorted((dict(c) for c in r["candidates"]), key=lambda c: c["plan_id"])
    peers = sorted((dict(p) for p in r.get("peers", [])), key=lambda p: p["peer_id"])
    qualified = [p for p in peers if p["semantic_profile"] == r["semantic_profile"] and p["replay_identity"] == r["replay_identity"] and p["authorized"] is True and p["evidence_state"] in {"proven", "supported"}]
    qualified_ids = [p["peer_id"] for p in qualified]
    missing_peer = sorted(set(p["peer_id"] for p in peers) - set(qualified_ids))
    candidate_ids = [c["plan_id"] for c in candidates]
    scores = []
    for c in candidates:
        values = [p["utility_milli"] for p in qualified if p["plan_id"] == c["plan_id"]]
        scores.append(c["expected_discovery_milli"] - c["risk_milli"] // 2 + (sum(values) // len(values) // 10 if values else 0))
    ranked_indices = sorted(range(len(candidates)), key=lambda i: (-scores[i], candidate_ids[i]))
    ranked = [candidate_ids[i] for i in ranked_indices]
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); evidence: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    global_block = not all(r.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block:
        blocked.update(candidate_ids); omissions.add("policy, protected-closure, approval, federation, or locality gate denied routing")
    else:
        for c in candidates:
            i = c["plan_id"]
            if c["evidence_state"] == "contradicted": blocked.add(i); negative.add(f"contradicted execution evidence: {i}")
            elif c["evidence_state"] in {"unknown", "unmeasured"}: unresolved.add(i); evidence.add(f"unresolved execution evidence: {i}"); uncertainty.add(f"plan evidence is not closed: {i}")
        eligible = [i for i in ranked_indices if candidates[i]["evidence_state"] in {"proven", "supported"}]
        if len(qualified) < r["minimum_peer_quorum"]:
            unresolved.update(candidate_ids[i] for i in eligible); omissions.add(f"peer quorum unresolved: required {r['minimum_peer_quorum']}, qualified {len(qualified)}"); uncertainty.add("federated utility is unresolved below the minimum peer quorum")
        elif eligible:
            margin = scores[eligible[0]] - scores[eligible[1]] if len(eligible) > 1 else 2**31 - 1
            if margin >= r["utility_margin_milli"]:
                selected.add(candidate_ids[eligible[0]]); unresolved.update(candidate_ids[i] for i in eligible[1:])
            else:
                unresolved.update(candidate_ids[i] for i in eligible); uncertainty.add(f"utility margin unresolved: observed {margin}, required {r['utility_margin_milli']}")
    missing_study = sorted(set(r.get("required_study_order", [])) - {c["study_id"] for c in candidates})
    missing_modality = sorted(set(r.get("required_modality_order", [])) - {c["modality"] for c in candidates})
    if missing_study or missing_modality:
        omissions.update(f"required study unresolved: {x}" for x in missing_study); omissions.update(f"required modality unresolved: {x}" for x in missing_modality); uncertainty.add("required study or modality coverage is incomplete"); unresolved.update(selected); selected.clear()
    so, uo, bo = sorted(selected), sorted(unresolved), sorted(blocked)
    disp = "blocked" if global_block or (bo and len(bo) == len(candidate_ids)) else "unresolved" if missing_study or missing_modality or uo or len(so) != 1 else "qualified"
    effects = ["manage:local-capability", "route:execution-plan"] if disp == "qualified" else ["block:execution-effects"]
    effect_receipts = [f"{e}:{r['request_id']}" for e in effects]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": r["request_id"], "task_id": r["task_id"], "purpose": r["purpose"], "semantic_profile": r["semantic_profile"], "disposition": disp, "required_study_order": r["required_study_order"], "required_modality_order": r["required_modality_order"], "candidate_order": candidate_ids, "ranked_order": ranked, "selected_order": so, "unresolved_order": uo, "blocked_order": bo, "missing_study_order": missing_study, "missing_modality_order": missing_modality, "qualified_peer_order": qualified_ids, "missing_peer_order": missing_peer, "score_milli_order": scores, "evidence_order": sorted(evidence), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "effect_order": effects, "replay_identity": r["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    value = {**payload, "routing_digest": digest, "artifact": {"artifact_id": f"routing-execution-copilot:{r['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": sorted({c["provenance_digest"] for c in candidates} | {p["provenance_digest"] for p in peers}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": sorted(effect_receipts)}
    receipt = ExecutionRoutingReceipt9(value); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "ExecutionRoutingReceipt9", "federated_execution_copilot_manifest", "route_federated_execution"]
