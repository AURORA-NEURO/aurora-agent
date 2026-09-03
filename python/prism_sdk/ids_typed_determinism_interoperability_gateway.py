"""Python parity for ``AFA-ids-P17-F24`` typed determinism negotiation."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P17-F24"
CONTRACT_VERSION = "ids-multimodal-version-negotiated-typed-determinism-interoperability-gateway/1.0"
INPUT_SCHEMA = "TypedDeterminismRequest7@1"
OUTPUT_SCHEMA = "TypedDeterminismReceipt8@1"
CONTENT_TYPE = "application/vnd.aurora.typed-determinism-receipt-8+json"


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _hash(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class TypedDeterminismReceipt8:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        identity = (v.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version") == CONTRACT_VERSION and v.get("feature_id") == FEATURE_ID and v.get("boundary") == PRECLINICAL_BOUNDARY and v.get("raw_data_local") is True and v.get("aggregate_only") is True and int(v.get("checkpoint", 0)) > 0 and all(isinstance(v.get(k), str) and v[k].strip() for k in ("request_id", "capability_id", "semantic_profile", "negotiated_version")) and v.get("endpoint_order") and v.get("effect_receipts") and v.get("disposition") in {"qualified", "unresolved", "blocked"})
        if not identity:
            raise ResearchContractError("typed-determinism identity, checkpoint, locality, endpoints, version, or effects are incomplete")
        fields = ("endpoint_order", "accepted_order", "migrated_order", "approval_required_order", "incompatible_order", "blocked_order", "missing_version_order", "missing_provenance_order", "omission_order", "uncertainty_order", "negative_evidence_order", "canonical_field_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("typed-determinism ordering is not canonical")
        ids = set(v["endpoint_order"]); states = v["accepted_order"] + v["migrated_order"] + v["approval_required_order"] + v["incompatible_order"] + v["blocked_order"]
        if len(ids) != len(v["endpoint_order"]) or len(states) != len(ids) or set(states) != ids:
            raise ResearchContractError("typed-determinism endpoint states do not partition")
        artifact = v.get("artifact", {}); digests = [v.get("canonical_input_digest"), v.get("replay_identity"), v.get("receipt_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if not all(_hash(d) for d in digests) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_hash") != v.get("receipt_digest"):
            raise ResearchContractError("typed-determinism digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:permitted-artifacts:", "manage:local-capability:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("typed-determinism effect is outside governed gate")


def typed_determinism_interoperability_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["interoperability engineer", "research SDK maintainer", "federation steward", "replay auditor"], "behavior": "negotiates typed schema versions and canonical field order across local aggregate-only research endpoints", "value": "prevents schema migration, semantic loss, or endpoint disagreement from becoming byte-level replay claims", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:permitted-artifacts", "manage:local-capability"], "permissions": ["read:local-capability-manifests", "request:version-negotiation"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "capability_id", "required_version", "preferred_version", "semantic_profile")) or not request.get("canonical_field_order") or request["canonical_field_order"] != sorted(set(request["canonical_field_order"])) or not request.get("endpoints") or len(request["endpoints"]) > 1024 or int(request.get("checkpoint", 0)) <= 0 or not _hash(request.get("canonical_input_digest")) or not _hash(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("typed-determinism request identity, versions, fields, bounds, digests, or locality is invalid")
    ids: set[str] = set()
    for endpoint in request["endpoints"]:
        if not all(isinstance(endpoint.get(k), str) and endpoint[k].strip() for k in ("endpoint_id", "origin", "capability_id", "semantic_profile")) or not endpoint.get("offered_versions") or not endpoint.get("canonical_field_order") or endpoint["canonical_field_order"] != sorted(set(endpoint["canonical_field_order"])) or not all(_hash(endpoint.get(k)) for k in ("canonical_input_digest", "provenance_digest", "replay_identity")) or endpoint.get("endpoint_id") in ids:
            raise ResearchContractError("typed-determinism endpoint identity, versions, fields, digests, or uniqueness is invalid")
        ids.add(endpoint["endpoint_id"])


def negotiate_typed_determinism(request: Mapping[str, Any]) -> TypedDeterminismReceipt8:
    _validate_request(request)
    endpoints = sorted((dict(e) for e in request["endpoints"]), key=lambda e: e["endpoint_id"])
    ids = [e["endpoint_id"] for e in endpoints]
    accepted: set[str] = set(); migrated: set[str] = set(); approval: set[str] = set(); incompatible: set[str] = set(); blocked: set[str] = set(); missing_version: set[str] = set(); missing_provenance: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for endpoint in endpoints:
        eid = endpoint["endpoint_id"]; provenance.add(endpoint["provenance_digest"])
        if endpoint.get("evidence_state") == "contradicted": blocked.add(eid); negative.add(f"{eid}:contradicted"); continue
        if endpoint["capability_id"] != request["capability_id"] or endpoint["semantic_profile"] != request["semantic_profile"] or endpoint.get("local") is not True or endpoint.get("aggregate_only") is not True:
            blocked.add(eid); omissions.add(f"{eid}:identity-or-locality-mismatch"); continue
        if endpoint["canonical_input_digest"] != request["canonical_input_digest"] or endpoint["canonical_field_order"] != request["canonical_field_order"]:
            incompatible.add(eid); omissions.add(f"{eid}:canonical-field-or-input-mismatch"); continue
        if not _hash(endpoint.get("provenance_digest")): missing_provenance.add(eid)
        versions = endpoint["offered_versions"]
        if request["required_version"] not in versions and request["preferred_version"] not in versions:
            missing_version.add(eid); incompatible.add(eid); uncertainty.add(f"{eid}:required-and-preferred-versions-unavailable"); continue
        if endpoint.get("replay_identity") != request["replay_identity"] or endpoint.get("signed") is not True:
            approval.add(eid); omissions.add(f"{eid}:replay-or-signature-approval-required"); continue
        if endpoint.get("evidence_state") not in {"proven", "supported"}:
            approval.add(eid); uncertainty.add(f"{eid}:evidence-state-not-supported"); continue
        (accepted if request["required_version"] in versions else migrated).add(eid)
        if request["required_version"] not in versions: omissions.add(f"{eid}:preferred-version-migrated-to-required-contract")
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block:
        blocked.update(ids); accepted.clear(); migrated.clear(); approval.clear(); incompatible.clear(); omissions.add("request:governance-or-locality-denied")
    ao, mo, po, io, bo = (sorted(x) for x in (accepted, migrated, approval, incompatible, blocked))
    disposition = "blocked" if global_block or (not ao and not mo and not po) else "unresolved" if (not ao and not mo) or po or io or bo else "qualified"
    if disposition != "qualified": omissions.add("request:determinism-negotiation-not-closed")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "capability_id": request["capability_id"], "semantic_profile": request["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "endpoint_order": ids, "accepted_order": ao, "migrated_order": mo, "approval_required_order": po, "incompatible_order": io, "blocked_order": bo, "missing_version_order": sorted(missing_version), "missing_provenance_order": sorted(missing_provenance), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "canonical_field_order": list(request["canonical_field_order"]), "canonical_input_digest": request["canonical_input_digest"], "negotiated_version": request["required_version"], "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    rd = _digest(payload); payload["receipt_digest"] = rd; payload["artifact"] = {"artifact_id": f"typed-determinism-receipt-8:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": rd, "semantic_loss": sorted(omissions), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = sorted([f"exchange:permitted-artifacts:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"])
    receipt = TypedDeterminismReceipt8(payload); receipt.validate(); return receipt


def idsTypedDeterminismDigest(receipt: TypedDeterminismReceipt8) -> str:
    receipt.validate(); return _digest(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "TypedDeterminismReceipt8", "typed_determinism_interoperability_manifest", "negotiate_typed_determinism", "idsTypedDeterminismDigest"]
