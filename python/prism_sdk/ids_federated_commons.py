"""Python parity for ``AFA-ids-P31-F15`` federated-commons previews."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P31-F15"
CONTRACT_VERSION = "ids-federated-commons-workflow-fabric/1.0"
INPUT_SCHEMA = "IdsFederatedCommonsRequest8@1"
OUTPUT_SCHEMA = "IdsFederatedCommonsReceipt10@1"
CONTENT_TYPE = "application/vnd.aurora.ids-federated-commons-receipt-10+json"
MAX_PEERS = 16_384


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class IdsFederatedCommonsReceipt10:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]: return dict(self.value)

    def validate(self) -> None:
        value = self.value; artifact = value.get("artifact", {})
        if (value.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or value.get("contract_version") != CONTRACT_VERSION or value.get("feature_id") != FEATURE_ID or value.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or value.get("raw_data_local") is not True or value.get("aggregate_only") is not True or not all(isinstance(value.get(k), str) and value[k].strip() for k in ("request_id", "purpose", "required_capability", "semantic_profile")) or not value.get("peer_order") or not value.get("effect_order") or not value.get("effect_receipts") or not isinstance(value.get("minimum_peer_quorum"), int) or value["minimum_peer_quorum"] <= 0 or value.get("disposition") not in {"qualified", "unresolved", "blocked"}): raise ResearchContractError("commons identity, peers, quorum, effects, locality, or disposition is incomplete")
        for key in ("peer_order", "qualified_peer_order", "unresolved_peer_order", "blocked_peer_order", "missing_peer_order", "contradiction_order", "evidence_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_order", "effect_receipts"):
            if not _ordered(value.get(key, [])): raise ResearchContractError("commons ordering is not canonical")
        ids = set(value["peer_order"]); parts = value["qualified_peer_order"] + value["unresolved_peer_order"] + value["blocked_peer_order"]
        if len(ids) != len(value["peer_order"]) or len(parts) != len(ids) or set(parts) != ids or value.get("qualified_peer_count") != len(value["qualified_peer_order"]) or value["qualified_peer_count"] > len(value["peer_order"]): raise ResearchContractError("commons states or quorum do not partition")
        if (not _digest(value.get("replay_identity")) or not _digest(value.get("commons_digest")) or artifact.get("content_hash") != value.get("commons_digest") or artifact.get("content_type") != CONTENT_TYPE or any(not _digest(d) for d in artifact.get("provenance_digests", []))): raise ResearchContractError("commons digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:federated-commons-digests:", "manage:local-capability:")) and e != "block:unsafe-release" for e in value["effect_receipts"]): raise ResearchContractError("effect is outside the governed commons gate")

    def digest(self) -> str: self.validate(); return _hash(self.to_dict())


def federated_commons_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["consortium researcher", "federation operator", "governance administrator"], "behavior": "negotiate digest-only peer participation with deterministic capability, semantic-profile, evidence, replay, quorum, policy, signature, and locality gates", "value": "makes cross-institution research commons membership auditable without moving raw data or accepting incomplete quorum", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:federated-commons-digests", "manage:local-capability"], "permissions": ["read:local-peer-summaries", "request:federated-commons-preview"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if (not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "purpose", "required_capability", "semantic_profile")) or not request.get("peers") or len(request["peers"]) > MAX_PEERS or not isinstance(request.get("minimum_peer_quorum"), int) or request["minimum_peer_quorum"] <= 0 or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True): raise ResearchContractError("commons identity, peer bound, quorum, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for peer in request["peers"]:
        if (not isinstance(peer, Mapping) or not isinstance(peer.get("peer_id"), str) or not peer["peer_id"].strip() or peer["peer_id"] in ids or not isinstance(peer.get("institution_id"), str) or not peer["institution_id"].strip() or not isinstance(peer.get("capability_id"), str) or not peer["capability_id"].strip() or not isinstance(peer.get("semantic_profile"), str) or not peer["semantic_profile"].strip() or not _digest(peer.get("artifact_digest")) or not _digest(peer.get("replay_identity")) or peer.get("local") is not True or peer.get("aggregate_only") is not True or peer.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"}): raise ResearchContractError(f"peer {peer.get('peer_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(peer["peer_id"])


def preview_federated_commons(request: Mapping[str, Any]) -> IdsFederatedCommonsReceipt10:
    _validate_request(request); peers = sorted((dict(p) for p in request["peers"]), key=lambda p: p["peer_id"]); peer_order = [p["peer_id"] for p in peers]
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); missing: set[str] = set(); contradiction: set[str] = set(); evidence: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for peer in peers:
        provenance.add(peer["artifact_digest"])
        if peer["capability_id"] != request["required_capability"]: unresolved.add(peer["peer_id"]); missing.add(f"{peer['peer_id']}:capability")
        elif peer["semantic_profile"] != request["semantic_profile"]: unresolved.add(peer["peer_id"]); uncertainty.add(f"{peer['peer_id']}:semantic-profile")
        elif peer["replay_identity"] != request["replay_identity"]: unresolved.add(peer["peer_id"]); uncertainty.add(f"{peer['peer_id']}:replay-identity")
        elif peer["policy_allow"] is not True or peer["federation_allow"] is not True or peer["signed"] is not True: blocked.add(peer["peer_id"]); omissions.add(f"{peer['peer_id']}:peer-governance")
        elif peer["evidence_state"] == "contradicted": blocked.add(peer["peer_id"]); contradiction.add(peer["peer_id"]); negative.add(f"{peer['peer_id']}:contradicted")
        elif peer["evidence_state"] not in {"proven", "supported"}: unresolved.add(peer["peer_id"]); evidence.add(peer["peer_id"]); uncertainty.add(f"{peer['peer_id']}:evidence-state")
        else: qualified.add(peer["peer_id"])
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(peer_order); qualified.clear(); unresolved.clear(); omissions.add("request:governance-or-locality-denied")
    qualified_order = sorted(qualified); unresolved_order = sorted(unresolved); blocked_order = sorted(blocked); quorum_met = len(qualified_order) >= request["minimum_peer_quorum"]; disposition = "blocked" if global_block or (not quorum_met and not qualified_order) else ("unresolved" if not quorum_met or unresolved_order or blocked_order else "qualified")
    if not quorum_met: omissions.add("request:peer-quorum-not-met")
    if disposition != "qualified": omissions.add("request:federated-commons-not-closed")
    effect_order = sorted(["exchange:federated-commons-digests", "manage:local-capability"] if disposition == "qualified" else ["block:unsafe-release"])
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "purpose": request["purpose"], "required_capability": request["required_capability"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "peer_order": peer_order, "qualified_peer_order": qualified_order, "unresolved_peer_order": unresolved_order, "blocked_peer_order": blocked_order, "missing_peer_order": sorted(missing), "contradiction_order": sorted(contradiction), "evidence_order": sorted(evidence), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "minimum_peer_quorum": request["minimum_peer_quorum"], "qualified_peer_count": len(qualified_order), "effect_order": effect_order, "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); value = dict(payload); value["commons_digest"] = digest; value["artifact"] = {"artifact_id": f"ids-federated-commons-receipt-10:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; value["effect_receipts"] = sorted(e if e == "block:unsafe-release" else f"{e}:{request['request_id']}" for e in effect_order); receipt = IdsFederatedCommonsReceipt10(value); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "IdsFederatedCommonsReceipt10", "federated_commons_manifest", "preview_federated_commons"]
