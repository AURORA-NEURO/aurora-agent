"""Python parity for ``AFA-oncoworlds-P05-F28``.

The adapter is a deterministic, read-only interoperability decision for resource
discovery.  It never fetches a peer or exports raw data; unknown, stale,
contradicted, and policy-denied resources remain explicit in the receipt.
"""
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

FEATURE_ID = "AFA-oncoworlds-P05-F28"
CONTRACT_VERSION = "oncoworlds-federated-continual-resource-discovery-assurance-harness/1.0"
INPUT_SCHEMA = "OncoworldsResourceNeed4@1"
OUTPUT_SCHEMA = "OncoworldsQualifiedResourceSet7@1"
CONTENT_TYPE = "application/vnd.aurora.qualified-resource-set-7+json"
MAX_ENDPOINTS = 4096
MAX_RESULTS = 256


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


@dataclass(frozen=True)
class OncoworldsQualifiedResourceSet7:
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
            or not all(str(value.get(key, "")).strip() for key in ("request_id", "federation_id", "requester", "purpose", "semantic_profile", "negotiated_protocol_version"))
            or not value.get("endpoint_order")
            or not value.get("peer_order")
            or not value.get("effect_receipts")
        ):
            raise ResearchContractError("oncoworlds resource interoperability identity, locality, protocol, peers, or effects are incomplete")
        fields = ("endpoint_order", "qualified_order", "unresolved_order", "blocked_order", "missing_capability_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omissions", "uncertainty", "negative_evidence", "adversarial_event_order", "migration_notes", "effect_receipts")
        if any(not _canonical(value.get(field, ())) for field in fields):
            raise ResearchContractError("oncoworlds resource interoperability ordering is not canonical")
        endpoint_order = set(value["endpoint_order"])
        classified = set(value["qualified_order"]) | set(value["unresolved_order"]) | set(value["blocked_order"])
        if endpoint_order != classified or len(endpoint_order) != len(value["endpoint_order"]):
            raise ResearchContractError("oncoworlds resource endpoint dispositions do not partition candidates")
        peer_order = set(value["peer_order"])
        qualified_peers = set(value["qualified_peer_order"])
        missing_peers = set(value["missing_peer_order"])
        if peer_order != qualified_peers | missing_peers or len(qualified_peers) + len(missing_peers) != len(peer_order):
            raise ResearchContractError("oncoworlds resource peer dispositions do not partition peers")
        resources = value.get("resources", ())
        if len(resources) != len(value["qualified_order"]) or any(row.get("resource_id") != resource_id for row, resource_id in zip(resources, value["qualified_order"])):
            raise ResearchContractError("oncoworlds resource qualified rows do not match qualified order")
        artifact = value.get("artifact", {})
        if artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_hash") != value.get("selection_digest"):
            raise ResearchContractError("oncoworlds resource artifact metadata or digest is inconsistent")
        digests = [value.get("replay_identity"), value.get("selection_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", ())]
        if not all(_digest(item) for item in digests):
            raise ResearchContractError("oncoworlds resource digest is invalid")
        if any(not effect.startswith("verify:resource-registry:") and effect != "block:unsafe-release" for effect in value["effect_receipts"]):
            raise ResearchContractError("oncoworlds resource effect is outside the interoperability gate")


def oncoworlds_resource_discovery_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "oncoworlds",
        "consumers": ["computational biologist", "federation steward", "resource registry"],
        "behavior": "qualifies typed local/aggregate-only resource endpoints against peer capability and policy evidence",
        "value": "makes federated resource discovery interoperable, replayable, and fail-closed",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["verify:resource-registry"],
        "permissions": ["evaluate:capability-runs"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def assure_oncoworlds_resources(
    request: Mapping[str, Any],
    endpoints: Sequence[Mapping[str, Any]],
    peers: Sequence[Mapping[str, Any]],
) -> OncoworldsQualifiedResourceSet7:
    required = set(map(str, request.get("required_capabilities", ())))
    if (
        not all(str(request.get(key, "")).strip() for key in ("request_id", "federation_id", "requester", "purpose", "semantic_profile", "required_protocol_version"))
        or not required
        or int(request.get("max_results", 0)) <= 0
        or int(request.get("max_results", 0)) > MAX_RESULTS
        or int(request.get("minimum_peer_quorum", 0)) <= 0
        or request.get("boundary", PRECLINICAL_BOUNDARY) != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or not endpoints
        or not peers
        or len(endpoints) > MAX_ENDPOINTS
        or not _digest(request.get("replay_identity"))
        or not _canonical(request.get("adversarial_event_order", ()))
    ):
        raise ResearchContractError("oncoworlds resource request identity, capabilities, bounds, locality, peers, endpoints, replay, or boundary is invalid")
    endpoint_rows = sorted((dict(row) for row in endpoints), key=lambda row: (-int(row.get("fitness_milli", 0)), str(row.get("resource_id", "")), str(row.get("endpoint_id", ""))))
    endpoint_ids = [str(row.get("resource_id", "")) for row in endpoint_rows]
    if any(not row.get("resource_id") or not row.get("endpoint_id") or not row.get("origin") or not all(_digest(row.get(key)) for key in ("artifact_digest", "provenance_digest", "replay_identity")) for row in endpoint_rows) or len(set(endpoint_ids)) != len(endpoint_ids):
        raise ResearchContractError("oncoworlds resource endpoint identity, uniqueness, or digests are invalid")
    peer_rows = sorted((dict(row) for row in peers), key=lambda row: str(row.get("peer_id", "")))
    peer_ids = [str(row.get("peer_id", "")) for row in peer_rows]
    if any(not row.get("peer_id") or not row.get("origin") or not _digest(row.get("summary_digest")) for row in peer_rows) or len(set(peer_ids)) != len(peer_ids):
        raise ResearchContractError("oncoworlds resource peer identity, uniqueness, or digest is invalid")
    qualified_peers = set()
    missing_peers = set()
    uncertainty = set()
    for peer in peer_rows:
        qualified = (
            peer.get("semantic_profile") == request["semantic_profile"]
            and peer.get("protocol_version") == request["required_protocol_version"]
            and peer.get("signed") is True
            and peer.get("aggregate_only") is True
            and peer.get("raw_data_local") is True
            and peer.get("state") in {"proven", "supported"}
        )
        if qualified:
            qualified_peers.add(peer["peer_id"])
        else:
            missing_peers.add(peer["peer_id"])
            uncertainty.add(f"peer:{peer['peer_id']}:not-qualified")
        if peer.get("state") == "contradicted":
            uncertainty.add(f"peer:{peer['peer_id']}:contradicted")
    allowed_origins = set(map(str, request.get("allowed_origins", ())))
    qualified: list[dict[str, Any]] = []
    unresolved: set[str] = set()
    blocked: set[str] = set()
    missing_capabilities: set[str] = set()
    omissions: set[str] = set()
    negative: set[str] = set()
    migration_notes: set[str] = set()
    for endpoint in endpoint_rows:
        resource_id = endpoint["resource_id"]
        if endpoint.get("negative_result"):
            negative.add(f"{resource_id}:negative-result")
        omissions.update(f"{resource_id}:{reason}" for reason in endpoint.get("omission_reasons", ()))
        missing = sorted(required - set(map(str, endpoint.get("capabilities", ()))))
        reasons: list[str] = []
        if endpoint.get("semantic_profile") != request["semantic_profile"]: reasons.append("semantic-profile-mismatch")
        if allowed_origins and endpoint.get("origin") not in allowed_origins: reasons.append("origin-out-of-scope")
        if missing:
            missing_capabilities.update(f"{resource_id}:{capability}" for capability in missing); reasons.append("required-capability-missing")
        if request["required_protocol_version"] not in endpoint.get("protocol_versions", ()): reasons.append("protocol-version-unavailable")
        if endpoint.get("replay_identity") != request["replay_identity"]: reasons.append("replay-identity-mismatch")
        if endpoint.get("signed") is not True: reasons.append("endpoint-signature-missing")
        if endpoint.get("permitted") is not True: reasons.append("endpoint-policy-denied")
        if endpoint.get("raw_data_local") is not True or endpoint.get("aggregate_only") is not True: reasons.append("endpoint-locality-or-aggregate-only-failed")
        status = endpoint.get("status")
        if status == "stale": reasons.append("stale-endpoint")
        elif status in {"protected", "revoked"}: reasons.append("protected-or-revoked-endpoint")
        elif status == "unavailable": reasons.append("endpoint-unavailable")
        state = endpoint.get("evidence_state")
        if state == "contradicted": reasons.append("contradicted-evidence"); negative.add(f"{resource_id}:contradicted")
        elif state not in {"proven", "supported"}: reasons.append("evidence-state-unresolved"); uncertainty.add(f"{resource_id}:evidence-state")
        if any(reason in {"protected-or-revoked-endpoint", "replay-identity-mismatch", "contradicted-evidence", "endpoint-policy-denied", "endpoint-locality-or-aggregate-only-failed"} for reason in reasons):
            blocked.add(resource_id)
        elif not reasons and len(qualified) < int(request["max_results"]):
            qualified.append({"resource_id": resource_id, "endpoint_id": endpoint["endpoint_id"], "origin": endpoint["origin"], "protocol_version": request["required_protocol_version"], "fitness_milli": int(endpoint.get("fitness_milli", 0)), "compatibility": "native", "migration_notes": []})
        else:
            if len(qualified) >= int(request["max_results"]): omissions.add(f"{resource_id}:result-limit")
            unresolved.add(resource_id)
    if len(qualified_peers) < int(request["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    adversarial_events = sorted(str(event) for event in request.get("adversarial_event_order", ()))
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only")) or bool(adversarial_events)
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{event}" for event in adversarial_events)
    if global_block:
        blocked.update(endpoint_ids); qualified.clear(); unresolved.clear(); omissions.add("request:global-interoperability-gate-blocked")
    disposition = "blocked" if global_block or blocked else "unresolved" if len(qualified_peers) < int(request["minimum_peer_quorum"]) or not qualified else "qualified"
    if disposition != "qualified" and not qualified: omissions.add("request:no-qualified-resource")
    qualified.sort(key=lambda row: row["resource_id"])
    for endpoint in endpoint_rows:
        versions = endpoint.get("protocol_versions", ())
        if request["required_protocol_version"] not in versions and any(str(version).startswith("1.") for version in versions) and str(request["required_protocol_version"]).startswith("1."):
            migration_notes.add(f"{endpoint['resource_id']}:protocol-major-compatible-minor-migration")
    endpoint_order = endpoint_ids
    qualified_order = [row["resource_id"] for row in qualified]
    unresolved_order = sorted(unresolved); blocked_order = sorted(blocked); missing_capability_order = sorted(missing_capabilities)
    peer_order = peer_ids; qualified_peer_order = sorted(qualified_peers); missing_peer_order = sorted(missing_peers)
    omissions_order = sorted(omissions); uncertainty_order = sorted(uncertainty); negative_order = sorted(negative); migration_order = sorted(migration_notes)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "requester": request["requester"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "negotiated_protocol_version": request["required_protocol_version"], "disposition": disposition, "endpoint_order": endpoint_order, "qualified_order": qualified_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "peer_order": peer_order, "qualified_peer_order": qualified_peer_order, "missing_peer_order": missing_peer_order, "omissions": omissions_order, "uncertainty": uncertainty_order, "negative_evidence": negative_order, "adversarial_event_order": adversarial_events, "migration_notes": migration_order, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    selection_digest = _hash(payload)
    provenance = sorted({str(row["provenance_digest"]) for row in endpoint_rows})
    effect_receipts = [f"verify:resource-registry:{request['request_id']}" if disposition == "qualified" else "block:unsafe-release"]
    result = {**payload, "qualified_order": qualified_order, "missing_capability_order": missing_capability_order, "resources": qualified, "effect_receipts": effect_receipts, "selection_digest": selection_digest, "artifact": {"artifact_id": f"qualified-resource-set-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": selection_digest, "semantic_loss": [], "provenance_digests": provenance, "boundary": PRECLINICAL_BOUNDARY}, "raw_data_local": request["raw_data_local"], "aggregate_only": request["aggregate_only"]}
    receipt = OncoworldsQualifiedResourceSet7(result)
    receipt.validate()
    return receipt


def oncoworlds_federated_resource_discovery_assurance_digest(receipt: OncoworldsQualifiedResourceSet7) -> str:
    receipt.validate()
    return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "OncoworldsQualifiedResourceSet7", "oncoworlds_resource_discovery_manifest", "assure_oncoworlds_resources", "oncoworlds_federated_resource_discovery_assurance_digest"]

