"""Python parity for ``AFA-dataops-P18-F15`` prospective provenance assurance."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-dataops-P18-F15"
CONTRACT_VERSION = "dataops-prospective-high-throughput-provenance-signing-workflow-fabric/1.0"
INPUT_SCHEMA = "ArtifactAndDerivation3@1"
OUTPUT_SCHEMA = "SignedProvenanceEnvelope7@1"
CONTENT_TYPE = "application/vnd.aurora.dataops-provenance-signing-workflow+json"


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _hash(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class SignedProvenanceEnvelope7:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if not (v.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version") == CONTRACT_VERSION and v.get("feature_id") == FEATURE_ID and v.get("boundary") == PRECLINICAL_BOUNDARY and v.get("artifact", {}).get("boundary") == PRECLINICAL_BOUNDARY and v.get("artifact", {}).get("content_type") == CONTENT_TYPE and v.get("signature_mode") == "detached-digest-attestation" and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("artifact_order") and v.get("effect_receipts") and v.get("disposition") in {"qualified", "unresolved", "blocked"}):
            raise ResearchContractError("provenance envelope identity, signature, locality, or effects are incomplete")
        fields = ("artifact_order", "verified_order", "unresolved_order", "blocked_order", "missing_parent_order", "cycle_order", "invalid_signature_order", "root_mismatch_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("provenance ordering is not canonical")
        ids = set(v["artifact_order"]); states = v["verified_order"] + v["unresolved_order"] + v["blocked_order"]
        if len(ids) != len(v["artifact_order"]) or len(states) != len(ids) or set(states) != ids:
            raise ResearchContractError("provenance artifact states do not partition")
        artifact = v["artifact"]; digests = [v.get("root_digest"), v.get("replay_identity"), v.get("receipt_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if not all(_hash(d) for d in digests) or artifact.get("content_hash") != v.get("receipt_digest"):
            raise ResearchContractError("provenance digest or artifact metadata is inconsistent")
        if any(e != "block:unsafe-release" and not e.startswith("verify:provenance-envelope:") for e in v["effect_receipts"]):
            raise ResearchContractError("provenance effect is outside governed gate")


def prospective_provenance_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "dataops", "consumers": ["research-object steward", "provenance auditor", "release operator"], "behavior": "verify prospective high-throughput artifact derivation lineage and detached signatures", "value": "prevents root drift, missing parents, cycles, invalid signatures, and incomplete evidence from entering release", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["verify:provenance-envelope", "block:unsafe-release"], "permissions": ["evaluate:capability-runs"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if request.get("schema_version") != INPUT_SCHEMA or not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "workflow_id", "batch_id", "semantic_profile")) or not isinstance(request.get("capacity"), int) or request["capacity"] <= 0 or not isinstance(request.get("active_jobs"), int) or request["active_jobs"] > request["capacity"] or not request.get("artifacts") or not _hash(request.get("expected_root")) or not _hash(request.get("replay_identity")) or request.get("adversarial_events") != sorted(set(request.get("adversarial_events", []))) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("provenance request identity, lineage, digest, locality, or boundary is invalid")
    ids: set[str] = set()
    for item in request["artifacts"]:
        if not all(isinstance(item.get(k), str) and item[k].strip() for k in ("artifact_id", "derivation_id", "actor", "semantic_profile")) or not _hash(item.get("content_digest")) or not _hash(item.get("provenance_digest")) or not _hash(item.get("replay_identity")) or item.get("parent_ids") != sorted(set(item.get("parent_ids", []))) or item.get("omission_order") != sorted(set(item.get("omission_order", []))) or item.get("artifact_id") in ids:
            raise ResearchContractError("provenance artifact identity, parent ordering, digests, or uniqueness is invalid")
        ids.add(item["artifact_id"])


def assure_prospective_provenance(request: Mapping[str, Any]) -> SignedProvenanceEnvelope7:
    _validate_request(request)
    artifacts = sorted((dict(item) for item in request["artifacts"]), key=lambda item: item["artifact_id"]); ids = [item["artifact_id"] for item in artifacts]; known = set(ids)
    parents = {item["artifact_id"]: item.get("parent_ids", []) for item in artifacts}; visiting: set[str] = set(); done: set[str] = set(); cycle: set[str] = set()
    def visit(node: str) -> None:
        if node in done: return
        if node in visiting: cycle.add(node); return
        visiting.add(node)
        for parent in parents.get(node, []):
            if parent in known: visit(parent)
        visiting.remove(node); done.add(node)
    for node in ids: visit(node)
    verified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); missing: set[str] = set(); invalid_signature: set[str] = set(); root_mismatch: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for item in artifacts:
        aid = item["artifact_id"]; provenance.add(item["provenance_digest"]); omissions.update(f"{aid}:{value}" for value in item.get("omission_order", []))
        if item.get("negative_result") or item.get("evidence_state") == "negative": negative.add(f"{aid}:negative-result")
        if any(parent not in known for parent in item.get("parent_ids", [])): missing.add(aid); omissions.add(f"{aid}:missing-parent")
        if aid in cycle: blocked.add(aid); omissions.add(f"{aid}:cycle")
        elif item.get("signature_valid") is not True: invalid_signature.add(aid); blocked.add(aid); omissions.add(f"{aid}:invalid-signature")
        elif item.get("semantic_profile") != request["semantic_profile"] or item.get("local") is not True: blocked.add(aid); omissions.add(f"{aid}:semantic-profile-or-locality-mismatch")
        elif item.get("replay_identity") != request["replay_identity"] or item.get("protected_closure") is not True: unresolved.add(aid); uncertainty.add(f"{aid}:replay-or-protected-closure-unresolved")
        elif item.get("evidence_state") not in {"proven", "supported"}: unresolved.add(aid); uncertainty.add(f"{aid}:evidence-not-supported")
        elif aid not in missing: verified.add(aid)
    root_payload = [[item["artifact_id"], item["content_digest"]] for item in artifacts]; calculated_root = _digest(root_payload)
    if calculated_root != request["expected_root"]: root_mismatch.update(ids); omissions.add("request:root-mismatch")
    global_block = not all(request.get(k) is True for k in ("policy_allowed", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block: blocked.update(ids); verified.clear(); unresolved.clear(); omissions.add("request:governance-or-adversarial-gate-blocked")
    uncertainty.update(f"adversarial:{event}" for event in request.get("adversarial_events", []))
    if root_mismatch: blocked.update(root_mismatch); verified.clear(); unresolved.clear()
    vo, uo, bo = sorted(verified), sorted(unresolved), sorted(blocked); disposition = "blocked" if global_block or (not vo and not uo) else "unresolved" if missing or cycle or invalid_signature or root_mismatch or bo or uo else "qualified"
    if disposition != "qualified": omissions.add("request:provenance-closure-not-ready")
    payload: dict[str, Any] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "workflow_id": request["workflow_id"], "batch_id": request["batch_id"], "capacity": request["capacity"], "active_jobs": request["active_jobs"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "artifact_order": ids, "verified_order": vo, "unresolved_order": uo, "blocked_order": bo, "missing_parent_order": sorted(missing), "cycle_order": sorted(cycle), "invalid_signature_order": sorted(invalid_signature), "root_mismatch_order": sorted(root_mismatch), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "root_digest": calculated_root, "replay_identity": request["replay_identity"], "signature_mode": "detached-digest-attestation", "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    rd = _digest(payload); payload["receipt_digest"] = rd; payload["artifact"] = {"artifact_id": f"dataops-signed-provenance-workflow-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": rd, "semantic_loss": sorted(omissions), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"verify:provenance-envelope:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    result = SignedProvenanceEnvelope7(payload); result.validate(); return result


def idsProspectiveProvenanceDigest(output: SignedProvenanceEnvelope7) -> str:
    output.validate(); return _digest(output.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "SignedProvenanceEnvelope7", "prospective_provenance_assurance_manifest", "assure_prospective_provenance", "idsProspectiveProvenanceDigest"]



def dataops_provenance_signing_workflow_fabric_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "dataops", "consumers": ["research-object steward", "provenance auditor", "release operator"], "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["verify:provenance-envelope", "block:unsafe-release"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

__all__.append("dataops_provenance_signing_workflow_fabric_manifest")
