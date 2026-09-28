"""Python parity for ``AFA-ids-P28-F06`` semantic-parity witnesses."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P28-F06"
CONTRACT_VERSION = "ids-multimodal-semantic-parity-contract-model/1.0"
INPUT_SCHEMA = "IdsParityFixture8@1"
OUTPUT_SCHEMA = "IdsParityWitness9@1"
CONTENT_TYPE = "application/vnd.aurora.ids-parity-witness-9+json"
MAX_FIXTURES = 16_384


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class IdsParityWitness9:
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
            or not all(isinstance(value.get(key), str) and value[key].strip() for key in ("request_id", "purpose", "semantic_profile"))
            or not value.get("required_study_order")
            or not value.get("required_modality_order")
            or len(value.get("fixture_order", [])) < 2
            or not value.get("effect_order")
            or not value.get("effect_receipts")
            or value.get("disposition") not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("parity identity, requirements, fixtures, effects, locality, or disposition is incomplete")
        for key in (
            "required_study_order", "required_modality_order", "fixture_order", "qualified_order", "unresolved_order", "blocked_order",
            "missing_study_order", "missing_modality_order", "omission_order", "uncertainty_order", "negative_evidence_order",
            "effect_order", "effect_receipts",
        ):
            if not _ordered(value.get(key, [])):
                raise ResearchContractError("parity ordering is not canonical")
        for key in ("schema_digest_order", "semantic_digest_order", "artifact_order"):
            if value.get(key, []) != sorted(set(value.get(key, []))):
                raise ResearchContractError("parity digest ordering is not canonical")
        ids = set(value["fixture_order"])
        parts = value["qualified_order"] + value["unresolved_order"] + value["blocked_order"]
        if len(ids) != len(value["fixture_order"]) or len(parts) != len(ids) or set(parts) != ids:
            raise ResearchContractError("fixture states do not partition")
        if (
            not _digest(value.get("replay_identity"))
            or not _digest(value.get("parity_digest"))
            or artifact.get("content_hash") != value.get("parity_digest")
            or artifact.get("content_type") != CONTENT_TYPE
            or any(not _digest(digest) for digest in artifact.get("provenance_digests", []))
        ):
            raise ResearchContractError("parity digest or artifact metadata is inconsistent")
        if any(
            not effect.startswith(("exchange:semantic-parity-digests:", "manage:local-capability:")) and effect != "block:unsafe-release"
            for effect in value["effect_receipts"]
        ):
            raise ResearchContractError("effect is outside the governed parity gate")

    def digest(self) -> str:
        self.validate()
        return _hash(self.to_dict())


def semantic_parity_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["formal methods researcher", "context compiler engineer", "compatibility operator"],
        "behavior": "compare schema, semantic, study, modality, artifact, provenance, and replay identities across typed multimodal fixtures",
        "value": "detects semantic drift and incomparable multimodal summaries before a research workflow consumes them",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:semantic-parity-digests", "manage:local-capability"],
        "permissions": ["read:local-parity-fixtures", "request:semantic-parity"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(request: Mapping[str, Any]) -> None:
    if (
        not all(isinstance(request.get(key), str) and request[key].strip() for key in ("request_id", "purpose", "semantic_profile"))
        or not request.get("required_study_order")
        or not request.get("required_modality_order")
        or len(request.get("fixtures", [])) < 2
        or len(request["fixtures"]) > MAX_FIXTURES
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
    ):
        raise ResearchContractError("parity identity, requirements, fixture bound, replay, locality, or boundary is invalid")
    for key in ("required_study_order", "required_modality_order"):
        if len(set(request[key])) != len(request[key]) or any(not isinstance(value, str) or not value.strip() for value in request[key]):
            raise ResearchContractError("required studies and modalities must be unique and non-empty")
    ids: set[str] = set()
    for fixture in request["fixtures"]:
        if (
            not isinstance(fixture, Mapping)
            or not isinstance(fixture.get("fixture_id"), str)
            or not fixture["fixture_id"].strip()
            or fixture["fixture_id"] in ids
            or not isinstance(fixture.get("producer_id"), str)
            or not fixture["producer_id"].strip()
            or not isinstance(fixture.get("study_id"), str)
            or not fixture["study_id"].strip()
            or not _digest(fixture.get("schema_digest"))
            or not _digest(fixture.get("semantic_digest"))
            or not fixture.get("modality_order")
            or any(not isinstance(value, str) or not value.strip() for value in fixture["modality_order"])
            or any(not _digest(digest) for digest in fixture.get("artifact_digests", []))
            or not _digest(fixture.get("provenance_digest"))
            or fixture.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"}
            or not _digest(fixture.get("replay_identity"))
            or fixture.get("local") is not True
            or fixture.get("aggregate_only") is not True
        ):
            raise ResearchContractError(f"fixture {fixture.get('fixture_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(fixture["fixture_id"])


def evaluate_ids_semantic_parity(request: Mapping[str, Any]) -> IdsParityWitness9:
    _validate_request(request)
    fixtures = sorted((dict(fixture) for fixture in request["fixtures"]), key=lambda fixture: fixture["fixture_id"])
    fixture_order = [fixture["fixture_id"] for fixture in fixtures]
    qualified: set[str] = set()
    unresolved: set[str] = set()
    blocked: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    schemas = {fixture["schema_digest"] for fixture in fixtures}
    semantics = {fixture["semantic_digest"] for fixture in fixtures}
    artifacts = {digest for fixture in fixtures for digest in fixture.get("artifact_digests", [])}
    provenance = {fixture["provenance_digest"] for fixture in fixtures}
    modalities = {value for fixture in fixtures for value in fixture["modality_order"]}
    studies = {fixture["study_id"] for fixture in fixtures}
    for fixture in fixtures:
        if fixture["evidence_state"] == "contradicted":
            blocked.add(fixture["fixture_id"])
            negative.add(f"{fixture['fixture_id']}:contradicted")
        elif fixture["evidence_state"] not in {"proven", "supported"}:
            unresolved.add(fixture["fixture_id"])
            uncertainty.add(f"{fixture['fixture_id']}:evidence-state")
        elif fixture["replay_identity"] != request["replay_identity"]:
            unresolved.add(fixture["fixture_id"])
            uncertainty.add(f"{fixture['fixture_id']}:replay-identity")
    required_modalities = set(request["required_modality_order"])
    required_studies = set(request["required_study_order"])
    for modality in sorted(required_modalities - modalities):
        omissions.add(f"modality:{modality}:missing")
        negative.add(f"modality:{modality}:no-parity-evidence")
    for study in sorted(required_studies - studies):
        omissions.add(f"study:{study}:missing")
        negative.add(f"study:{study}:no-parity-evidence")
    parity_match = all(
        left["schema_digest"] == right["schema_digest"]
        and left["semantic_digest"] == right["semantic_digest"]
        and set(left["modality_order"]) == set(right["modality_order"])
        for left, right in zip(fixtures, fixtures[1:])
    )
    if not parity_match:
        uncertainty.add("fixtures:schema-semantic-or-modality-disagreement")
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if not global_block and parity_match and not omissions:
        qualified.update(fixture_order)
    elif not global_block:
        unresolved.update(fixture["fixture_id"] for fixture in fixtures if fixture["fixture_id"] not in blocked and fixture["fixture_id"] not in unresolved)
    if global_block:
        blocked.update(fixture_order)
        qualified.clear()
        unresolved.clear()
        omissions.add("request:governance-or-locality-denied")
    qualified_order = sorted(qualified)
    unresolved_order = sorted(unresolved)
    blocked_order = sorted(blocked)
    disposition = "blocked" if global_block or (not qualified_order and not unresolved_order) else ("unresolved" if blocked_order or unresolved_order or omissions or not parity_match else "qualified")
    if disposition != "qualified":
        omissions.add("request:semantic-parity-not-closed")
    omission_order = sorted(omissions)
    uncertainty_order = sorted(uncertainty)
    negative_evidence_order = sorted(negative)
    effect_order = sorted(["exchange:semantic-parity-digests", "manage:local-capability"] if disposition == "qualified" else ["block:unsafe-release"])
    effect_receipts = sorted(effect if effect == "block:unsafe-release" else f"{effect}:{request['request_id']}" for effect in effect_order)
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request["request_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"],
        "required_study_order": sorted(required_studies), "required_modality_order": sorted(required_modalities), "disposition": disposition,
        "fixture_order": fixture_order, "qualified_order": qualified_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order,
        "missing_study_order": sorted(required_studies - studies), "missing_modality_order": sorted(required_modalities - modalities),
        "schema_digest_order": sorted(schemas), "semantic_digest_order": sorted(semantics), "artifact_order": sorted(artifacts),
        "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_evidence_order,
        "effect_order": effect_order, "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    value = dict(payload)
    value["parity_digest"] = digest
    value["artifact"] = {"artifact_id": f"ids-parity-witness-9:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": omission_order, "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    value["effect_receipts"] = effect_receipts
    receipt = IdsParityWitness9(value)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "IdsParityWitness9", "semantic_parity_manifest", "evaluate_ids_semantic_parity"]
