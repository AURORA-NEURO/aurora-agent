"""Python parity for ``AFA-ids-P27-F18`` dependency composition."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P27-F18"
CONTRACT_VERSION = "ids-multimodal-dependency-composition-research-workbench/1.0"
INPUT_SCHEMA = "IdsCompositionRequest7@1"
OUTPUT_SCHEMA = "IdsCompositionReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.ids-composition-receipt-9+json"
MAX_CANDIDATES = 16_384


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class IdsCompositionReceipt9:
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
            or not all(isinstance(value.get(key), str) and value[key].strip() for key in ("request_id", "purpose", "semantic_profile", "required_capability"))
            or not value.get("required_modality_order")
            or not value.get("required_study_order")
            or not value.get("candidate_order")
            or not value.get("effect_order")
            or not value.get("effect_receipts")
            or value.get("disposition") not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("composition identity, requirements, candidates, effects, locality, or disposition is incomplete")
        for key in (
            "required_modality_order", "required_study_order", "candidate_order", "selected_order", "unresolved_order",
            "blocked_order", "missing_capability_order", "dependency_order", "modality_order", "study_order",
            "omission_order", "uncertainty_order", "negative_evidence_order", "reasons", "effect_order", "effect_receipts",
        ):
            if not _ordered(value.get(key, [])):
                raise ResearchContractError("composition ordering is not canonical")
        if value.get("artifact_order", []) != sorted(set(value.get("artifact_order", []))):
            raise ResearchContractError("composition artifact ordering is not canonical")
        ids = set(value["candidate_order"])
        parts = value["selected_order"] + value["unresolved_order"] + value["blocked_order"]
        if len(ids) != len(value["candidate_order"]) or len(parts) != len(ids) or set(parts) != ids:
            raise ResearchContractError("candidate states do not partition")
        if (
            not _digest(value.get("replay_identity"))
            or not _digest(value.get("composition_digest"))
            or artifact.get("content_hash") != value.get("composition_digest")
            or artifact.get("content_type") != CONTENT_TYPE
            or any(not _digest(digest) for digest in artifact.get("provenance_digests", []))
        ):
            raise ResearchContractError("composition digest or artifact metadata is inconsistent")
        if any(
            not effect.startswith(("view:ids-composition:", "manage:local-capability:")) and effect != "block:unsafe-release"
            for effect in value["effect_receipts"]
        ):
            raise ResearchContractError("effect is outside the governed composition gate")

    def digest(self) -> str:
        self.validate()
        return _hash(self.to_dict())


def dependency_composition_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["context compiler engineer", "formal methods researcher", "workbench operator"],
        "behavior": "compose typed capability dependencies for comparable imaging and omics studies with deterministic provider ranking",
        "value": "makes missing capabilities, dependency gaps, semantic mismatch, and protected omissions visible before a workflow is admitted",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["view:ids-composition", "manage:local-capability"],
        "permissions": ["read:local-capability-manifests", "request:dependency-composition"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(request: Mapping[str, Any]) -> None:
    if (
        not all(isinstance(request.get(key), str) and request[key].strip() for key in ("request_id", "purpose", "semantic_profile", "required_capability"))
        or not request.get("required_modalities")
        or not request.get("required_studies")
        or not request.get("candidates")
        or len(request["candidates"]) > MAX_CANDIDATES
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
    ):
        raise ResearchContractError("composition identity, requirements, candidate bound, replay, locality, or boundary is invalid")
    for key in ("required_modalities", "required_studies"):
        values = request[key]
        if len(set(values)) != len(values) or any(not isinstance(value, str) or not value.strip() for value in values):
            raise ResearchContractError("required modalities and studies must be unique and non-empty")
    ids: set[str] = set()
    for candidate in request["candidates"]:
        if (
            not isinstance(candidate, Mapping)
            or not isinstance(candidate.get("candidate_id"), str)
            or not candidate["candidate_id"].strip()
            or candidate["candidate_id"] in ids
            or not isinstance(candidate.get("capability_id"), str)
            or not candidate["capability_id"].strip()
            or not isinstance(candidate.get("provider_id"), str)
            or not candidate["provider_id"].strip()
            or not isinstance(candidate.get("semantic_profile"), str)
            or not candidate["semantic_profile"].strip()
            or any(not isinstance(value, str) or not value.strip() for value in candidate.get("modality_order", []))
            or any(not isinstance(value, str) or not value.strip() for value in candidate.get("study_order", []))
            or any(not isinstance(value, str) or not value.strip() for value in candidate.get("requires", []))
            or any(not _digest(digest) for digest in candidate.get("artifact_digests", []))
            or not _digest(candidate.get("provenance_digest"))
            or not _digest(candidate.get("replay_identity"))
            or candidate.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"}
            or candidate.get("local") is not True
            or candidate.get("aggregate_only") is not True
        ):
            raise ResearchContractError(f"candidate {candidate.get('candidate_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(candidate["candidate_id"])


def compose_ids_dependencies(request: Mapping[str, Any]) -> IdsCompositionReceipt9:
    _validate_request(request)
    candidates = sorted((dict(candidate) for candidate in request["candidates"]), key=lambda candidate: candidate["candidate_id"])
    candidate_order = [candidate["candidate_id"] for candidate in candidates]
    by_id = {candidate["candidate_id"]: candidate for candidate in candidates}
    providers: dict[str, list[str]] = {}
    for candidate in candidates:
        providers.setdefault(candidate["capability_id"], []).append(candidate["candidate_id"])
    for values in providers.values():
        values.sort()
    queue = [request["required_capability"]]
    seen: set[str] = set()
    selected: set[str] = set()
    unresolved: set[str] = set()
    blocked: set[str] = set()
    missing: set[str] = set()
    dependencies: set[str] = set()
    modalities: set[str] = set()
    studies: set[str] = set()
    artifacts: set[str] = set()
    provenance: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    while queue:
        capability = queue.pop(0)
        if capability in seen:
            continue
        seen.add(capability)
        provider_ids = providers.get(capability, [])
        if not provider_ids:
            missing.add(capability)
            omissions.add(f"capability:{capability}:no-compatible-provider")
            negative.add(f"capability:{capability}:negative-provider-evidence")
            continue
        if len(provider_ids) > 1:
            uncertainty.add(f"capability:{capability}:multiple-providers-ranked-by-candidate-id")
        provider_id = provider_ids[0]
        candidate = by_id[provider_id]
        if candidate["semantic_profile"] != request["semantic_profile"]:
            unresolved.add(provider_id)
            omissions.add(f"candidate:{provider_id}:semantic-profile")
        elif candidate["replay_identity"] != request["replay_identity"]:
            unresolved.add(provider_id)
            uncertainty.add(f"candidate:{provider_id}:replay-identity")
        elif candidate["evidence_state"] == "contradicted":
            blocked.add(provider_id)
            negative.add(f"candidate:{provider_id}:contradicted")
        elif candidate["evidence_state"] not in {"proven", "supported"}:
            unresolved.add(provider_id)
            uncertainty.add(f"candidate:{provider_id}:evidence-state")
        elif candidate.get("local") is not True or candidate.get("aggregate_only") is not True:
            blocked.add(provider_id)
            omissions.add(f"candidate:{provider_id}:raw-data-locality")
        else:
            modality_gap = set(request["required_modalities"]) - set(candidate.get("modality_order", []))
            study_gap = set(request["required_studies"]) - set(candidate.get("study_order", []))
            for modality in modality_gap:
                omissions.add(f"candidate:{provider_id}:missing-modality:{modality}")
            for study in study_gap:
                omissions.add(f"candidate:{provider_id}:missing-study:{study}")
            if modality_gap or study_gap:
                unresolved.add(provider_id)
            else:
                selected.add(provider_id)
                modalities.update(candidate.get("modality_order", []))
                studies.update(candidate.get("study_order", []))
                artifacts.update(candidate.get("artifact_digests", []))
                provenance.add(candidate["provenance_digest"])
        for dependency in candidate.get("requires", []):
            dependencies.add(f"{provider_id}->{dependency}")
            queue.append(dependency)
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if global_block:
        blocked.update(candidate_order)
        selected.clear()
        unresolved.clear()
        omissions.add("request:governance-or-locality-denied")
    selected_order = sorted(selected)
    unresolved_order = sorted(unresolved)
    blocked_order = sorted(blocked)
    missing_capability_order = sorted(missing)
    disposition = "blocked" if global_block or (not selected_order and not unresolved_order) else ("unresolved" if blocked_order or unresolved_order or missing_capability_order else "qualified")
    if disposition != "qualified":
        omissions.add("request:dependency-composition-not-closed")
    omission_order = sorted(omissions)
    uncertainty_order = sorted(uncertainty)
    negative_evidence_order = sorted(negative)
    modality_order = sorted(modalities)
    study_order = sorted(studies)
    artifact_order = sorted(artifacts)
    effect_order = sorted(["manage:local-capability", "view:ids-composition"] if disposition == "qualified" else ["block:unsafe-release"])
    effect_receipts = sorted(effect if effect == "block:unsafe-release" else f"{effect}:{request['request_id']}" for effect in effect_order)
    reasons = [f"{request['required_capability']} required capability evaluated across {len(candidate_order)} declared candidates"]
    if missing_capability_order:
        reasons.append("missing capabilities remain explicit and cannot be composed")
    if unresolved_order:
        reasons.append("semantic, replay, evidence, modality, or study gaps remain visible")
    if blocked_order:
        reasons.append("blocked candidates remain visible and cannot be admitted")
    reasons.sort()
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request["request_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"],
        "required_capability": request["required_capability"], "required_modality_order": sorted(set(request["required_modalities"])),
        "required_study_order": sorted(set(request["required_studies"])), "disposition": disposition, "candidate_order": candidate_order,
        "selected_order": selected_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order,
        "missing_capability_order": missing_capability_order, "dependency_order": sorted(dependencies), "modality_order": modality_order,
        "study_order": study_order, "artifact_order": artifact_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order, "reasons": reasons, "effect_order": effect_order,
        "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    value = dict(payload)
    value["composition_digest"] = digest
    value["artifact"] = {"artifact_id": f"ids-composition-receipt-9:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": omission_order, "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    value["effect_receipts"] = effect_receipts
    receipt = IdsCompositionReceipt9(value)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "IdsCompositionReceipt9", "dependency_composition_manifest", "compose_ids_dependencies"]
