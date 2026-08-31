"""Python parity for ``AFA-devplat-P07-F31`` federated quality assurance."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-devplat-P07-F31"
CONTRACT_VERSION = "devplat-prospective-high-throughput-quality-control-federated-control-plane/1.0"
INPUT_SCHEMA = "DevplatQualityBatchRequest5@1"
OUTPUT_SCHEMA = "DevplatQualityControlPlaneReceipt7@1"
CONTENT_TYPE = "application/vnd.aurora.devplat-quality-control-plane-7+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class QualityVerdict7:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value
        artifact = value.get("artifact", {})
        if not (
            value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION
            and value.get("contract_version") == CONTRACT_VERSION
            and value.get("feature_id") == FEATURE_ID
            and value.get("boundary") == PRECLINICAL_BOUNDARY
            and artifact.get("boundary") == PRECLINICAL_BOUNDARY
            and value.get("raw_data_local") is True
            and value.get("aggregate_only") is True
            and all(isinstance(value.get(k), str) and value[k].strip() for k in ("request_id", "requester", "purpose", "semantic_profile"))
            and value.get("observation_order")
            and value.get("site_order")
            and value.get("modality_order")
            and value.get("effect_receipts")
            and value.get("disposition") in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("quality identity, observations, sites, modalities, locality, or effects are incomplete")
        fields = (
            "observation_order", "selected_observation_order", "unresolved_observation_order", "blocked_observation_order",
            "site_order", "selected_site_order", "unresolved_site_order", "blocked_site_order", "missing_site_order",
            "modality_order", "passed_modality_order", "missing_modality_order", "omission_order", "uncertainty_order",
            "negative_evidence_order", "effect_receipts",
        )
        if any(not _ordered(value.get(field, [])) for field in fields):
            raise ResearchContractError("quality verdict ordering is not canonical")
        observations = set(value["observation_order"])
        parts = value["selected_observation_order"] + value["unresolved_observation_order"] + value["blocked_observation_order"]
        if len(observations) != len(value["observation_order"]) or len(parts) != len(observations) or set(parts) != observations:
            raise ResearchContractError("quality observation states do not form a complete partition")
        sites = set(value["site_order"])
        site_parts = value["selected_site_order"] + value["unresolved_site_order"] + value["blocked_site_order"] + value["missing_site_order"]
        if len(sites) != len(value["site_order"]) or len(site_parts) != len(sites) or set(site_parts) != sites:
            raise ResearchContractError("quality site states do not form a complete partition")
        modalities = set(value["modality_order"])
        modality_parts = value["passed_modality_order"] + value["missing_modality_order"]
        if len(modalities) != len(value["modality_order"]) or len(modality_parts) != len(modalities) or set(modality_parts) != modalities:
            raise ResearchContractError("quality modality states do not form a complete partition")
        if not _digest(value.get("replay_identity")) or not _digest(value.get("report_digest")) or not _digest(artifact.get("content_hash")) or artifact.get("content_hash") != value.get("report_digest") or artifact.get("content_type") != CONTENT_TYPE:
            raise ResearchContractError("quality artifact metadata or digest is inconsistent")
        effects = value["effect_receipts"]
        if any(not effect.startswith("verify:quality:") and effect != "block:unsafe-release" for effect in effects):
            raise ResearchContractError("effect is outside the quality assurance gate")
        if value["disposition"] == "qualified" and effects != [f"verify:quality:{value['request_id']}"]:
            raise ResearchContractError("qualified quality effect is invalid")
        if value["disposition"] != "qualified" and effects != ["block:unsafe-release"]:
            raise ResearchContractError("non-qualified quality verdict must block release")

    def digest(self) -> str:
        self.validate()
        return _hash(self.to_dict())


def devplat_quality_control_federated_control_plane_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "devplat",
        "consumers": ["preclinical researcher", "research administrator", "federated quality steward"],
        "behavior": "verifies digest-bound aggregate quality declarations across policy-separated institutions",
        "value": "prevents stale, contradictory, missing, or unmeasured quality evidence from silently entering a research workflow",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["execute_local_computation", "write_local_artifact"],
        "permissions": ["evaluate:capability-runs", "read:local-research-artifacts"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def _validate_request(request: Mapping[str, Any]) -> None:
    if not (
        request.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION
        and all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "requester", "purpose", "semantic_profile"))
        and request.get("required_site_order") and _ordered(request["required_site_order"])
        and request.get("required_modality_order") and _ordered(request["required_modality_order"])
        and isinstance(request.get("minimum_site_count"), int) and request["minimum_site_count"] > 0
        and isinstance(request.get("minimum_pass_fraction_milli"), int) and 0 <= request["minimum_pass_fraction_milli"] <= 1000
        and _digest(request.get("replay_identity"))
        and _ordered(request.get("adversarial_events", []))
        and request.get("boundary") == PRECLINICAL_BOUNDARY
        and request.get("raw_data_local") is True and request.get("aggregate_only") is True
        and request.get("objects") and len(request["objects"]) <= 8192
    ):
        raise ResearchContractError("quality request identity, axes, threshold, replay, locality, boundary, or objects are invalid")
    ids: set[str] = set()
    for obj in request["objects"]:
        if not (
            all(isinstance(obj.get(k), str) and obj[k].strip() for k in ("object_id", "site_id", "study_id", "semantic_profile"))
            and obj.get("modality_order") and _ordered(obj["modality_order"])
            and _ordered(obj.get("passed_modality_order", []))
            and all(item in obj["modality_order"] for item in obj.get("passed_modality_order", []))
            and isinstance(obj.get("pass_fraction_milli"), int) and 0 <= obj["pass_fraction_milli"] <= 1000
            and all(_digest(obj.get(k)) for k in ("replay_identity", "quality_report_digest", "provenance_digest"))
            and _ordered(obj.get("omission_order", [])) and _ordered(obj.get("uncertainty_order", []))
            and obj.get("object_id") not in ids
        ):
            raise ResearchContractError("research object identity, modality closure, digests, or ordering is invalid")
        ids.add(obj["object_id"])


def compile_devplat_quality_control_federated_control_plane(request: Mapping[str, Any]) -> QualityVerdict7:
    _validate_request(request)
    objects = sorted((dict(obj) for obj in request["objects"]), key=lambda obj: obj["object_id"])
    observation_order = [obj["object_id"] for obj in objects]
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set()
    sites = set(request["required_site_order"]); modalities = set(request["required_modality_order"])
    omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); by_site: dict[str, list[dict[str, Any]]] = {}
    for obj in objects:
        sites.add(obj["site_id"]); modalities.update(obj["modality_order"]); by_site.setdefault(obj["site_id"], []).append(obj)
        if obj.get("negative_result"): negative.add(f"{obj['object_id']}:negative-result")
        omissions.update(f"{obj['object_id']}:{reason}" for reason in obj.get("omission_order", []))
        uncertainty.update(f"{obj['object_id']}:{reason}" for reason in obj.get("uncertainty_order", []))
        if not all(obj.get(k) is True for k in ("local_only", "aggregate_only", "permitted", "signed")):
            blocked.add(obj["object_id"]); omissions.add(f"{obj['object_id']}:authorization-or-locality")
        elif obj.get("stale") or obj["semantic_profile"] != request["semantic_profile"] or obj["replay_identity"] != request["replay_identity"] or obj.get("evidence_state") not in {"proven", "supported"}:
            unresolved.add(obj["object_id"])
            if obj.get("stale"): uncertainty.add(f"{obj['object_id']}:stale")
            if obj["semantic_profile"] != request["semantic_profile"]: uncertainty.add(f"{obj['object_id']}:semantic-profile-mismatch")
            if obj["replay_identity"] != request["replay_identity"]: uncertainty.add(f"{obj['object_id']}:replay-mismatch")
            if obj.get("evidence_state") == "unknown": uncertainty.add(f"{obj['object_id']}:unknown-evidence")
            if obj.get("evidence_state") == "unmeasured": uncertainty.add(f"{obj['object_id']}:unmeasured")
            if obj.get("evidence_state") == "contradicted":
                unresolved.discard(obj["object_id"]); blocked.add(obj["object_id"]); negative.add(f"{obj['object_id']}:contradicted")
        elif obj["pass_fraction_milli"] < request["minimum_pass_fraction_milli"]:
            unresolved.add(obj["object_id"]); omissions.add(f"{obj['object_id']}:threshold-failed")
        else:
            selected.add(obj["object_id"])
    required_sites = set(request["required_site_order"]); selected_sites: set[str] = set(); unresolved_sites: set[str] = set(); blocked_sites: set[str] = set(); missing_sites: set[str] = set(); passed_modalities: set[str] = set()
    for site in sorted(sites):
        rows = by_site.get(site, [])
        if not rows:
            if site in required_sites: missing_sites.add(site); omissions.add(f"site:{site}:missing")
            continue
        ids = [row["object_id"] for row in rows]
        if any(item in blocked for item in ids): blocked_sites.add(site)
        elif any(item in unresolved for item in ids): unresolved_sites.add(site)
        else:
            selected_sites.add(site)
            for row in rows: passed_modalities.update(row.get("passed_modality_order", []))
    passed_modalities.intersection_update(modalities)
    missing_modalities = {item for item in request["required_modality_order"] if item not in passed_modalities}
    if missing_modalities: uncertainty.add("modality:required-closure-incomplete")
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_allow") is not True: negative.add("request:federation-denied")
    negative.update(f"adversarial:{event}" for event in request.get("adversarial_events", []))
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_allow", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block:
        blocked.update(observation_order); selected.clear(); unresolved.clear(); selected_sites.clear(); unresolved_sites.clear(); blocked_sites.update(sites); omissions.add("request:quality-release-gate-blocked")
    aggregate = (sum(obj["pass_fraction_milli"] for obj in objects if obj["object_id"] in selected) // len(selected)) if selected else 0
    if global_block or blocked or blocked_sites: disposition = "blocked"
    elif len(selected_sites) < request["minimum_site_count"] or missing_sites or missing_modalities or unresolved or unresolved_sites or aggregate < request["minimum_pass_fraction_milli"]: disposition = "unresolved"
    else: disposition = "qualified"
    if disposition != "qualified": omissions.add("request:quality-verdict-not-release-ready")
    effects = [f"verify:quality:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request["request_id"], "requester": request["requester"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition,
        "observation_order": observation_order, "selected_observation_order": sorted(selected), "unresolved_observation_order": sorted(unresolved), "blocked_observation_order": sorted(blocked),
        "site_order": sorted(sites), "selected_site_order": sorted(selected_sites), "unresolved_site_order": sorted(unresolved_sites), "blocked_site_order": sorted(blocked_sites), "missing_site_order": sorted(missing_sites),
        "modality_order": sorted(modalities), "passed_modality_order": sorted(passed_modalities), "missing_modality_order": sorted(missing_modalities), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative),
        "qualified_site_count": len(selected_sites), "aggregate_pass_fraction_milli": aggregate, "replay_identity": request["replay_identity"], "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    report_digest = _hash(payload)
    value = {**payload, "report_digest": report_digest, "artifact": {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"devplat-quality-control-plane-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": report_digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}}
    receipt = QualityVerdict7(value); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "QualityVerdict7", "devplat_quality_control_federated_control_plane_manifest", "compile_devplat_quality_control_federated_control_plane"]


