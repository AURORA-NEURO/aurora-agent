"""Python parity for ``AFA-onco-P18-F16`` provenance/signing workflow."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-onco-P18-F16"
CONTRACT_VERSION = "onco-federated-continual-provenance-signing-workflow-fabric/1.0"
INPUT_SCHEMA = "OncoProvenanceObject6@1"
OUTPUT_SCHEMA = "SignedProvenanceWorkflow9@1"
CONTENT_TYPE = "application/vnd.aurora.onco-signed-provenance-workflow-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class SignedProvenanceWorkflow9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value; artifact = value.get("artifact", {})
        if not (value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and value.get("contract_version") == CONTRACT_VERSION and value.get("feature_id") == FEATURE_ID and value.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("boundary") == PRECLINICAL_BOUNDARY and value.get("raw_data_local") is True and value.get("aggregate_only") is True and all(isinstance(value.get(k), str) and value[k].strip() for k in ("request_id", "research_program", "purpose", "semantic_profile")) and value.get("artifact_order") and value.get("site_order") and value.get("signer_order") and value.get("effect_receipts") and value.get("disposition") in {"qualified", "unresolved", "blocked"}):
            raise ResearchContractError("provenance identity, artifacts, sites, signers, locality, or effects are incomplete")
        fields = ("artifact_order", "selected_artifact_order", "unresolved_artifact_order", "blocked_artifact_order", "missing_artifact_order", "site_order", "selected_site_order", "unresolved_site_order", "blocked_site_order", "missing_site_order", "signer_order", "selected_signer_order", "missing_signer_order", "revoked_signer_order", "provenance_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(value.get(field, [])) for field in fields): raise ResearchContractError("provenance workflow ordering is not canonical")
        artifacts = set(value["artifact_order"]); artifact_parts = value["selected_artifact_order"] + value["unresolved_artifact_order"] + value["blocked_artifact_order"] + value["missing_artifact_order"]
        if len(artifacts) != len(value["artifact_order"]) or len(artifact_parts) != len(artifacts) or set(artifact_parts) != artifacts: raise ResearchContractError("artifact states do not form a complete partition")
        sites = set(value["site_order"]); site_parts = value["selected_site_order"] + value["unresolved_site_order"] + value["blocked_site_order"] + value["missing_site_order"]
        if len(sites) != len(value["site_order"]) or len(site_parts) != len(sites) or set(site_parts) != sites: raise ResearchContractError("site states do not form a complete partition")
        signers = set(value["signer_order"]); signer_parts = value["selected_signer_order"] + value["missing_signer_order"] + value["revoked_signer_order"]
        if len(signers) != len(value["signer_order"]) or len(signer_parts) != len(signers) or set(signer_parts) != signers: raise ResearchContractError("signer states do not form a complete partition")
        if not all(_digest(value.get(k)) for k in ("replay_identity", "workflow_digest")) or not _digest(artifact.get("content_hash")) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("content_hash") != value.get("workflow_digest") or not isinstance(value.get("signature_coverage_milli"), int) or not 0 <= value["signature_coverage_milli"] <= 1000: raise ResearchContractError("provenance artifact metadata, coverage, or digest is inconsistent")
        effects = value["effect_receipts"]
        if any(not effect.startswith("exchange:signed-provenance:") and effect != "block:unsafe-release" for effect in effects): raise ResearchContractError("effect is outside the provenance signing gate")
        if value["disposition"] == "qualified" and effects != [f"exchange:signed-provenance:{value['request_id']}"]: raise ResearchContractError("qualified provenance effect is invalid")
        if value["disposition"] != "qualified" and effects != ["block:unsafe-release"]: raise ResearchContractError("non-qualified provenance workflow must block release")

    def digest(self) -> str:
        self.validate(); return _hash(self.to_dict())


def federated_provenance_signing_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "onco", "consumers": ["research program lead", "provenance steward", "preclinical data engineer"], "behavior": "compiles federated continual signed provenance declarations into an omission-aware workflow receipt", "value": "lets research programs exchange verifiable aggregate provenance without moving raw OncoWorld artifacts or hiding signer and lineage failures", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["execute_local_computation", "write_local_artifact"], "permissions": ["read:local-research-artifacts", "exchange:permitted-provenance"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not (request.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "research_program", "purpose", "semantic_profile")) and request.get("required_site_order") and _ordered(request["required_site_order"]) and request.get("required_artifact_order") and _ordered(request["required_artifact_order"]) and isinstance(request.get("minimum_signer_count"), int) and request["minimum_signer_count"] > 0 and _digest(request.get("replay_identity")) and _ordered(request.get("adversarial_events", [])) and request.get("boundary") == PRECLINICAL_BOUNDARY and request.get("raw_data_local") is True and request.get("aggregate_only") is True and request.get("objects")):
        raise ResearchContractError("provenance request identity, closure, signer floor, replay, locality, boundary, or objects are invalid")
    ids: set[str] = set()
    for obj in request["objects"]:
        if not (all(isinstance(obj.get(k), str) and obj[k].strip() for k in ("object_id", "site_id", "study_id", "semantic_profile", "purpose", "signer_id")) and obj.get("lineage_order") and _ordered(obj["lineage_order"]) and all(_digest(obj.get(k)) for k in ("artifact_digest", "provenance_digest", "replay_identity", "signature_digest")) and _ordered(obj.get("omission_order", [])) and _ordered(obj.get("uncertainty_order", [])) and obj.get("object_id") not in ids):
            raise ResearchContractError("provenance object identity, lineage, signer, digests, or ordering is invalid")
        ids.add(obj["object_id"])


def compile_federated_provenance_signing(request: Mapping[str, Any]) -> SignedProvenanceWorkflow9:
    _validate_request(request); objects = sorted((dict(obj) for obj in request["objects"]), key=lambda obj: obj["object_id"])
    artifact_order = sorted(set(request["required_artifact_order"]) | {obj["object_id"] for obj in objects}); selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); sites = set(request["required_site_order"]); signers: set[str] = set(); selected_signers: set[str] = set(); missing_signers: set[str] = set(); revoked_signers: set[str] = set(); provenance: set[str] = set(); by_site: dict[str, list[dict[str, Any]]] = {}
    for obj in objects:
        sites.add(obj["site_id"]); signers.add(obj["signer_id"]); by_site.setdefault(obj["site_id"], []).append(obj); provenance.update(f"{obj['object_id']}:{item}" for item in obj["lineage_order"]); omissions.update(f"{obj['object_id']}:{item}" for item in obj.get("omission_order", [])); uncertainty.update(f"{obj['object_id']}:{item}" for item in obj.get("uncertainty_order", []))
        if obj.get("negative_result"): negative.add(f"{obj['object_id']}:negative-result")
        if obj.get("revoked"):
            blocked.add(obj["object_id"]); revoked_signers.add(obj["signer_id"]); negative.add(f"{obj['object_id']}:signer-revoked")
        elif not all(obj.get(k) is True for k in ("permitted", "local_only", "aggregate_only", "signature_verified")):
            blocked.add(obj["object_id"]); missing_signers.add(obj["signer_id"]); omissions.add(f"{obj['object_id']}:signature-or-locality")
        elif obj.get("stale") or obj["semantic_profile"] != request["semantic_profile"] or obj["purpose"] != request["purpose"] or obj["replay_identity"] != request["replay_identity"] or obj.get("evidence_state") not in {"proven", "supported"}:
            unresolved.add(obj["object_id"]); missing_signers.add(obj["signer_id"])
            if obj.get("stale"): uncertainty.add(f"{obj['object_id']}:stale")
            if obj["semantic_profile"] != request["semantic_profile"]: uncertainty.add(f"{obj['object_id']}:semantic-profile-mismatch")
            if obj["purpose"] != request["purpose"]: uncertainty.add(f"{obj['object_id']}:purpose-mismatch")
            if obj["replay_identity"] != request["replay_identity"]: uncertainty.add(f"{obj['object_id']}:replay-mismatch")
            if obj.get("evidence_state") == "unknown": uncertainty.add(f"{obj['object_id']}:unknown-evidence")
            if obj.get("evidence_state") == "speculative": uncertainty.add(f"{obj['object_id']}:speculative-evidence")
            if obj.get("evidence_state") == "contradicted": unresolved.discard(obj["object_id"]); blocked.add(obj["object_id"]); negative.add(f"{obj['object_id']}:contradicted")
        else:
            selected.add(obj["object_id"]); selected_signers.add(obj["signer_id"])
    required_sites = set(request["required_site_order"]); selected_sites: set[str] = set(); unresolved_sites: set[str] = set(); blocked_sites: set[str] = set(); missing_sites: set[str] = set()
    for site in sorted(sites):
        rows = by_site.get(site, [])
        if not rows:
            if site in required_sites: missing_sites.add(site); omissions.add(f"site:{site}:missing")
        else:
            ids = [row["object_id"] for row in rows]
            if any(item in blocked for item in ids): blocked_sites.add(site)
            elif any(item in unresolved for item in ids): unresolved_sites.add(site)
            else: selected_sites.add(site)
    observed = {obj["object_id"] for obj in objects}; missing_artifacts = {item for item in request["required_artifact_order"] if item not in observed}; omissions.update(f"artifact:{item}:missing-or-unqualified" for item in missing_artifacts)
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_allow") is not True: negative.add("request:federation-denied")
    negative.update(f"adversarial:{event}" for event in request.get("adversarial_events", []))
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_allow", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block:
        blocked.update(obj["object_id"] for obj in objects); selected.clear(); unresolved.clear(); selected_sites.clear(); unresolved_sites.clear(); blocked_sites.update(sites); omissions.add("request:provenance-release-gate-blocked")
    signature_coverage = (len(selected) * 1000) // len(objects) if objects else 0
    if global_block or blocked or blocked_sites: disposition = "blocked"
    elif len(selected) < len(request["required_artifact_order"]) or missing_artifacts or len(selected_signers) < request["minimum_signer_count"] or missing_sites or unresolved or unresolved_sites: disposition = "unresolved"
    else: disposition = "qualified"
    if disposition != "qualified": omissions.add("request:provenance-workflow-not-release-ready")
    effects = [f"exchange:signed-provenance:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "research_program": request["research_program"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "artifact_order": artifact_order, "selected_artifact_order": sorted(selected), "unresolved_artifact_order": sorted(unresolved), "blocked_artifact_order": sorted(blocked), "missing_artifact_order": sorted(missing_artifacts), "site_order": sorted(sites), "selected_site_order": sorted(selected_sites), "unresolved_site_order": sorted(unresolved_sites), "blocked_site_order": sorted(blocked_sites), "missing_site_order": sorted(missing_sites), "signer_order": sorted(signers), "selected_signer_order": sorted(selected_signers), "missing_signer_order": sorted(missing_signers), "revoked_signer_order": sorted(revoked_signers), "provenance_order": sorted(provenance), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "signature_coverage_milli": signature_coverage, "replay_identity": request["replay_identity"], "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    workflow_digest = _hash(payload); value = {**payload, "workflow_digest": workflow_digest, "artifact": {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"onco-signed-provenance-workflow-9:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": workflow_digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}}; receipt = SignedProvenanceWorkflow9(value); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "SignedProvenanceWorkflow9", "federated_provenance_signing_manifest", "compile_federated_provenance_signing"]
