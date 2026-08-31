"""Python parity for ``AFA-ids-P17-F28`` typed-determinism assurance."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P17-F28"
CONTRACT_VERSION = "ids-federated-continual-typed-determinism-assurance-harness/1.0"
INPUT_SCHEMA = "TypedCapabilityInput4@1"
OUTPUT_SCHEMA = "CanonicalCapabilityOutput7@1"
CONTENT_TYPE = "application/vnd.aurora.canonical-capability-output-7+json"


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _hash(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class CanonicalCapabilityOutput7:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if not (v.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version") == CONTRACT_VERSION and v.get("feature_id") == FEATURE_ID and v.get("boundary") == PRECLINICAL_BOUNDARY and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("implementation_order") and v.get("canonical_field_order") and v.get("effect_receipts") and v.get("disposition") in {"qualified", "unresolved", "blocked"}):
            raise ResearchContractError("typed-determinism assurance identity, closure, locality, or effects are incomplete")
        fields = ("implementation_order", "verified_order", "mismatch_order", "unresolved_order", "blocked_order", "omission_order", "uncertainty_order", "negative_evidence_order", "canonical_field_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("typed-determinism assurance ordering is not canonical")
        ids = set(v["implementation_order"]); states = v["verified_order"] + v["mismatch_order"] + v["unresolved_order"] + v["blocked_order"]
        if len(ids) != len(v["implementation_order"]) or len(states) != len(ids) or set(states) != ids:
            raise ResearchContractError("typed-determinism assurance states do not partition")
        artifact = v.get("artifact", {}); digests = [v.get("input_digest"), v.get("canonical_output_digest"), v.get("replay_identity"), v.get("receipt_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if not all(_hash(d) for d in digests) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_hash") != v.get("receipt_digest"):
            raise ResearchContractError("typed-determinism assurance digest or artifact metadata is inconsistent")
        if any(e != "block:unsafe-release" and not e.startswith("verify:canonical-parity:") for e in v["effect_receipts"]):
            raise ResearchContractError("typed-determinism assurance effect is outside governed gate")


def typed_determinism_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["formal methods researcher", "verification engineer", "release auditor"], "behavior": "verify typed canonical capability outputs across federated continual implementations", "value": "turns cross-language parity, omissions, and negative evidence into release-gated receipts", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["verify:canonical-parity", "block:unsafe-release"], "permissions": ["evaluate:capability-runs"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_input(request: Mapping[str, Any]) -> None:
    if request.get("schema_version") != INPUT_SCHEMA or not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "capability_id", "scope", "semantic_profile")) or request.get("canonical_field_order") != sorted(set(request.get("canonical_field_order", []))) or not request.get("canonical_field_order") or not request.get("implementations") or not all(_hash(request.get(k)) for k in ("input_digest", "expected_output_digest", "replay_identity")) or request.get("adversarial_events") != sorted(set(request.get("adversarial_events", []))) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("typed-determinism assurance input identity, fields, digests, locality, or boundary is invalid")
    ids: set[str] = set()
    for item in request["implementations"]:
        if not all(isinstance(item.get(k), str) and item[k].strip() for k in ("implementation_id", "origin", "semantic_profile")) or item.get("canonical_field_order") != sorted(set(item.get("canonical_field_order", []))) or not item.get("canonical_field_order") or not all(_hash(item.get(k)) for k in ("input_digest", "output_digest", "provenance_digest", "replay_identity")) or item.get("omission_order") != sorted(set(item.get("omission_order", []))) or item.get("implementation_id") in ids:
            raise ResearchContractError("typed-determinism assurance implementation identity, ordering, digests, or uniqueness is invalid")
        ids.add(item["implementation_id"])


def assure_typed_determinism(request: Mapping[str, Any]) -> CanonicalCapabilityOutput7:
    _validate_input(request)
    implementations = sorted((dict(item) for item in request["implementations"]), key=lambda item: item["implementation_id"])
    ids = [item["implementation_id"] for item in implementations]
    verified: set[str] = set(); mismatch: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for item in implementations:
        iid = item["implementation_id"]; provenance.add(item["provenance_digest"]); omissions.update(f"{iid}:{value}" for value in item.get("omission_order", []))
        if item.get("negative_result") or item.get("evidence_state") == "negative": negative.add(f"{iid}:negative-result")
        if item.get("semantic_profile") != request["semantic_profile"] or item.get("local") is not True or item.get("aggregate_only") is not True:
            blocked.add(iid); omissions.add(f"{iid}:semantic-profile-or-locality-mismatch")
        elif item.get("canonical_field_order") != request["canonical_field_order"] or item.get("input_digest") != request["input_digest"] or item.get("output_digest") != request["expected_output_digest"]:
            mismatch.add(iid); omissions.add(f"{iid}:canonical-input-or-output-mismatch")
        elif item.get("replay_identity") != request["replay_identity"] or item.get("signed") is not True:
            unresolved.add(iid); uncertainty.add(f"{iid}:replay-or-signature-unresolved")
        elif item.get("evidence_state") not in {"proven", "supported"}:
            unresolved.add(iid); uncertainty.add(f"{iid}:evidence-not-proven")
        else:
            verified.add(iid)
    global_block = not all(request.get(k) is True for k in ("policy_allowed", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block:
        blocked.update(ids); verified.clear(); mismatch.clear(); unresolved.clear(); omissions.add("request:governance-or-adversarial-gate-blocked")
    uncertainty.update(f"adversarial:{event}" for event in request.get("adversarial_events", []))
    vo, mo, uo, bo = sorted(verified), sorted(mismatch), sorted(unresolved), sorted(blocked)
    disposition = "blocked" if global_block or (not vo and not uo) else "unresolved" if mo or uo or bo else "qualified"
    if disposition != "qualified": omissions.add("request:canonical-parity-not-closed")
    payload: dict[str, Any] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "capability_id": request["capability_id"], "scope": request["scope"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "implementation_order": ids, "verified_order": vo, "mismatch_order": mo, "unresolved_order": uo, "blocked_order": bo, "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "canonical_field_order": list(request["canonical_field_order"]), "input_digest": request["input_digest"], "canonical_output_digest": request["expected_output_digest"], "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    rd = _digest(payload); payload["receipt_digest"] = rd; payload["artifact"] = {"artifact_id": f"canonical-capability-output-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": rd, "semantic_loss": sorted(omissions), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"verify:canonical-parity:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    result = CanonicalCapabilityOutput7(payload); result.validate(); return result


def idsTypedDeterminismAssuranceDigest(output: CanonicalCapabilityOutput7) -> str:
    output.validate(); return _digest(output.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "CanonicalCapabilityOutput7", "typed_determinism_assurance_manifest", "assure_typed_determinism", "idsTypedDeterminismAssuranceDigest"]
