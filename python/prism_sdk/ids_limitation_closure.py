"""Python parity for ``AFA-ids-P26-F24`` federated limitation closure."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P26-F24"
CONTRACT_VERSION = "ids-federated-limitation-closure-interoperability-gateway/1.0"
INPUT_SCHEMA = "IdsLimitationCase8@1"
OUTPUT_SCHEMA = "IdsClosureReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.ids-closure-receipt-9+json"
MAX_CASES = 16_384
MAX_PEERS = 16_384


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class IdsClosureReceipt9:
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
            or not all(isinstance(value.get(key), str) and value[key].strip() for key in ("request_id", "semantic_profile"))
            or not value.get("required_scope_order")
            or not value.get("case_order")
            or not value.get("peer_order")
            or not value.get("effect_order")
            or not value.get("effect_receipts")
            or value.get("disposition") not in {"closed", "partial", "unknown", "blocked"}
            or int(value.get("qualified_peer_count", -1)) > len(value.get("peer_order", []))
        ):
            raise ResearchContractError("closure identity, scopes, cases, peers, effects, locality, or disposition is incomplete")
        for key in (
            "required_scope_order", "case_order", "resolved_order", "unresolved_order", "blocked_order",
            "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order",
            "negative_evidence_order", "effect_order", "effect_receipts",
        ):
            if not _ordered(value.get(key, [])):
                raise ResearchContractError("closure ordering is not canonical")
        if value.get("evidence_order", []) != sorted(set(value.get("evidence_order", []))):
            raise ResearchContractError("closure evidence ordering is not canonical")
        case_ids = set(value["case_order"])
        case_parts = value["resolved_order"] + value["unresolved_order"] + value["blocked_order"]
        if len(case_ids) != len(value["case_order"]) or len(case_parts) != len(case_ids) or set(case_parts) != case_ids:
            raise ResearchContractError("limitation states do not partition")
        peer_ids = set(value["peer_order"])
        peer_parts = value["qualified_peer_order"] + value["missing_peer_order"]
        if len(peer_ids) != len(value["peer_order"]) or len(peer_parts) != len(peer_ids) or set(peer_parts) != peer_ids:
            raise ResearchContractError("peer states do not partition")
        if (
            int(value.get("minimum_peer_quorum", 0)) == 0
            or not _digest(value.get("replay_identity"))
            or not _digest(value.get("closure_digest"))
            or artifact.get("content_hash") != value.get("closure_digest")
            or artifact.get("content_type") != CONTENT_TYPE
            or any(not _digest(digest) for digest in artifact.get("provenance_digests", []))
        ):
            raise ResearchContractError("closure digest, quorum, or artifact metadata is inconsistent")
        if any(
            not effect.startswith(("exchange:permitted-limitation-digests:", "manage:local-capability:"))
            and effect != "block:unsafe-release"
            for effect in value["effect_receipts"]
        ):
            raise ResearchContractError("effect is outside the governed limitation-closure gate")

    def digest(self) -> str:
        self.validate()
        return _hash(self.to_dict())


def limitation_closure_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["formal methods researcher", "federation operator", "release auditor", "context compiler engineer"],
        "behavior": "close typed limitation cases across policy-separated institutions without hiding unresolved or negative evidence",
        "value": "prevents measured, open, contradictory, or peer-incomplete limitations from being represented as closed",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-limitation-digests", "manage:local-capability"],
        "permissions": ["read:local-limitation-attestations", "request:limitation-closure"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(request: Mapping[str, Any]) -> None:
    if (
        not all(isinstance(request.get(key), str) and request[key].strip() for key in ("request_id", "semantic_profile"))
        or not request.get("required_scopes")
        or not request.get("cases")
        or len(request["cases"]) > MAX_CASES
        or not request.get("peers")
        or len(request["peers"]) > MAX_PEERS
        or int(request.get("minimum_peer_quorum", 0)) < 2
        or int(request.get("minimum_peer_quorum", 0)) > MAX_PEERS
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
    ):
        raise ResearchContractError("request identity, scope, case/peer bound, quorum, replay, locality, or boundary is invalid")
    scopes = request["required_scopes"]
    if len(set(scopes)) != len(scopes) or any(not isinstance(scope, str) or not scope.strip() for scope in scopes):
        raise ResearchContractError("required scopes are not unique and non-empty")
    case_ids: set[str] = set()
    for case in request["cases"]:
        if (
            not isinstance(case, Mapping)
            or not isinstance(case.get("case_id"), str)
            or not case["case_id"].strip()
            or case["case_id"] in case_ids
            or not isinstance(case.get("limitation"), str)
            or not case["limitation"].strip()
            or not isinstance(case.get("scope"), str)
            or not case["scope"].strip()
            or case.get("status") not in {"open", "measured", "resolved", "blocked", "unknown", "contradicted"}
            or any(not _digest(digest) for digest in case.get("evidence_digests", []))
            or not case.get("closure_criteria")
            or any(not isinstance(criterion, str) or not criterion.strip() for criterion in case["closure_criteria"])
            or not isinstance(case.get("mitigation"), str)
            or not case["mitigation"].strip()
            or not _digest(case.get("replay_identity"))
            or case.get("local") is not True
            or case.get("aggregate_only") is not True
        ):
            raise ResearchContractError(f"limitation case {case.get('case_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        case_ids.add(case["case_id"])
    peer_ids: set[str] = set()
    for peer in request["peers"]:
        if (
            not isinstance(peer, Mapping)
            or not isinstance(peer.get("peer_id"), str)
            or not peer["peer_id"].strip()
            or peer["peer_id"] in peer_ids
            or not isinstance(peer.get("semantic_profile"), str)
            or not peer["semantic_profile"].strip()
            or not _digest(peer.get("closure_digest"))
            or peer.get("evidence_state") not in {"qualified", "unknown", "contradicted"}
            or not _digest(peer.get("replay_identity"))
            or peer.get("local") is not True
            or peer.get("aggregate_only") is not True
        ):
            raise ResearchContractError(f"closure peer {peer.get('peer_id', '')} is invalid, duplicated, or non-local")
        peer_ids.add(peer["peer_id"])


def close_ids_limitations(request: Mapping[str, Any]) -> IdsClosureReceipt9:
    _validate_request(request)
    cases = sorted((dict(case) for case in request["cases"]), key=lambda case: case["case_id"])
    peers = sorted((dict(peer) for peer in request["peers"]), key=lambda peer: peer["peer_id"])
    case_order = [case["case_id"] for case in cases]
    peer_order = [peer["peer_id"] for peer in peers]
    required_scope_order = sorted(set(request["required_scopes"]))
    required_scopes = set(request["required_scopes"])
    resolved: set[str] = set()
    unresolved: set[str] = set()
    blocked: set[str] = set()
    evidence: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    for case in cases:
        evidence.update(case.get("evidence_digests", []))
        if case.get("negative_result") is not None:
            negative.add(f"{case['case_id']}:{case['negative_result']}")
        if case["scope"] not in required_scopes:
            unresolved.add(case["case_id"])
            omissions.add(f"{case['case_id']}:scope-not-requested")
            continue
        status = case["status"]
        if status == "resolved" and case.get("evidence_digests") and case.get("closure_criteria"):
            if case["replay_identity"] == request["replay_identity"]:
                resolved.add(case["case_id"])
            else:
                unresolved.add(case["case_id"])
                uncertainty.add(f"{case['case_id']}:replay-identity")
        elif status == "resolved":
            unresolved.add(case["case_id"])
            omissions.add(f"{case['case_id']}:resolved-without-evidence-or-criteria")
        elif status == "measured":
            unresolved.add(case["case_id"])
            uncertainty.add(f"{case['case_id']}:measured-but-not-closed")
        elif status == "open":
            unresolved.add(case["case_id"])
            omissions.add(f"{case['case_id']}:limitation-open")
        elif status == "unknown":
            unresolved.add(case["case_id"])
            uncertainty.add(f"{case['case_id']}:limitation-unknown")
        elif status == "blocked":
            blocked.add(case["case_id"])
            omissions.add(f"{case['case_id']}:limitation-blocked")
        else:
            blocked.add(case["case_id"])
            negative.add(f"{case['case_id']}:contradicted")
    qualified_peers: set[str] = set()
    missing_peers: set[str] = set()
    for peer in peers:
        peer_id = peer["peer_id"]
        if peer["semantic_profile"] != request["semantic_profile"]:
            missing_peers.add(peer_id)
            omissions.add(f"{peer_id}:semantic-profile")
        elif peer["replay_identity"] != request["replay_identity"]:
            missing_peers.add(peer_id)
            uncertainty.add(f"{peer_id}:replay-identity")
        elif peer["evidence_state"] == "qualified":
            qualified_peers.add(peer_id)
        elif peer["evidence_state"] == "unknown":
            missing_peers.add(peer_id)
            uncertainty.add(f"{peer_id}:peer-evidence-unknown")
        else:
            missing_peers.add(peer_id)
            negative.add(f"{peer_id}:peer-evidence-contradicted")
    qualified_peer_count = len(qualified_peers)
    minimum_peer_quorum = int(request["minimum_peer_quorum"])
    if qualified_peer_count < minimum_peer_quorum:
        uncertainty.add(f"peer-quorum:{qualified_peer_count}/{minimum_peer_quorum}")
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "federation_approved", "signed_approval", "raw_data_local", "aggregate_only"))
    if global_block:
        blocked.update(case_order)
        resolved.clear()
        unresolved.clear()
        omissions.add("request:governance-or-locality-denied")
    if global_block:
        disposition = "blocked"
    elif not resolved and not unresolved:
        disposition = "blocked"
    elif not resolved:
        disposition = "unknown"
    elif unresolved or blocked or qualified_peer_count < minimum_peer_quorum:
        disposition = "partial"
    else:
        disposition = "closed"
    if blocked and not resolved:
        disposition = "blocked"
    if disposition != "closed":
        omissions.add("request:limitation-closure-not-complete")
    resolved_order = sorted(resolved)
    unresolved_order = sorted(unresolved)
    blocked_order = sorted(blocked)
    qualified_peer_order = sorted(qualified_peers)
    missing_peer_order = sorted(missing_peers)
    omission_order = sorted(omissions)
    uncertainty_order = sorted(uncertainty)
    negative_evidence_order = sorted(negative)
    effect_order = ["exchange:permitted-limitation-digests", "manage:local-capability"] if disposition == "closed" else (["block:unsafe-release", "exchange:permitted-limitation-digests"] if disposition == "partial" else ["block:unsafe-release"])
    effect_order.sort()
    effect_receipts = sorted(effect if effect == "block:unsafe-release" else f"{effect}:{request['request_id']}" for effect in effect_order)
    reasons = [f"{len(case_order)} limitation cases and {len(peer_order)} peer attestations evaluated with explicit closure states"]
    if qualified_peer_count < minimum_peer_quorum:
        reasons.append("peer quorum is below the requested closure threshold")
    if unresolved_order:
        reasons.append("unresolved limitations remain visible and cannot be promoted to closed")
    if blocked_order:
        reasons.append("blocked or contradicted limitations remain visible")
    reasons.sort()
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request["request_id"],
        "semantic_profile": request["semantic_profile"],
        "required_scope_order": required_scope_order,
        "disposition": disposition,
        "case_order": case_order,
        "resolved_order": resolved_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "minimum_peer_quorum": minimum_peer_quorum,
        "qualified_peer_count": qualified_peer_count,
        "evidence_order": sorted(evidence),
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "reasons": reasons,
        "effect_order": effect_order,
        "replay_identity": request["replay_identity"],
        "raw_data_local": True,
        "aggregate_only": True,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    closure_digest = _hash(payload)
    value = dict(payload)
    value["closure_digest"] = closure_digest
    value["artifact"] = {
        "artifact_id": f"ids-closure-receipt-9:{request['request_id']}",
        "content_type": CONTENT_TYPE,
        "content_hash": closure_digest,
        "semantic_loss": omission_order,
        "provenance_digests": [],
        "boundary": PRECLINICAL_BOUNDARY,
    }
    value["effect_receipts"] = effect_receipts
    receipt = IdsClosureReceipt9(value)
    receipt.validate()
    return receipt


__all__ = [
    "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE",
    "IdsClosureReceipt9", "limitation_closure_manifest", "close_ids_limitations",
]
