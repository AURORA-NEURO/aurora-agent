"""Parity surface for ``AFA-obligation-P20-F24``.

This SDK helper performs the same deterministic, metadata-only admission as the Rust gateway.
It never opens a network connection or serializes raw experimental data.
"""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-obligation-P20-F24"
CONTRACT_VERSION = "obligation-security-federation-interoperability-gateway/1.0"
INPUT_SCHEMA = "FederationRequest4@1"
OUTPUT_SCHEMA = "FederationEnvelope6@1"
CONTENT_TYPE = "application/vnd.aurora.federation-envelope-6+json"


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def security_federation_interoperability_gateway_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "obligation", "consumers": ["context compiler engineer", "federation security steward", "institution operator"], "behavior": "negotiate versioned policy-bounded federation capabilities into deterministic aggregate-only envelopes while preserving omissions, uncertainty, semantic loss, and negative evidence", "value": "enables interoperable institutional research exchange without unauthorized data egress or silent semantic drift", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["execute:local-computation", "write:local-artifact", "exchange:permitted-artifacts"], "permissions": ["connect:approved-endpoints", "exchange:permitted-artifacts"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def validate_security_federation_envelope(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version") != CONTRACT_VERSION or output.get("feature_id") != FEATURE_ID or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified", "unresolved", "blocked"} or not output.get("capability_order") or not output.get("provider_order") or not output.get("reasons") or not output.get("effect_receipts"):
        raise ResearchContractError("gateway identity, typed closure, locality, or effects are incomplete")
    keys = ("capability_order", "selected_capability_order", "unresolved_capability_order", "denied_capability_order", "missing_capability_order", "provider_order", "selected_provider_order", "missing_provider_order", "protocol_order", "migration_order", "semantic_loss_order", "omission_order", "uncertainty_order", "negative_evidence_order", "adversarial_event_order", "reasons", "effect_receipts")
    if any(not _ordered(output.get(key, [])) for key in keys):
        raise ResearchContractError("gateway ordering is not canonical")
    identifiers = set(output["capability_order"]); states = output["selected_capability_order"] + output["unresolved_capability_order"] + output["denied_capability_order"] + output["missing_capability_order"]
    if len(identifiers) != len(output["capability_order"]) or set(states) != identifiers or len(states) != len(set(states)):
        raise ResearchContractError("capability states do not partition")
    providers = set(output["provider_order"]); provider_states = output["selected_provider_order"] + output["missing_provider_order"]
    if len(providers) != len(output["provider_order"]) or set(provider_states) != providers or len(provider_states) != len(set(provider_states)):
        raise ResearchContractError("provider states do not partition")
    if not _valid_digest(output.get("replay_identity")) or not _valid_digest(output.get("envelope_digest")) or artifact.get("content_hash") != output.get("envelope_digest"):
        raise ResearchContractError("gateway digest or artifact hash is inconsistent")
    if any(effect != "block:unsafe-release" and not effect.startswith("exchange:permitted-artifacts:") for effect in output["effect_receipts"]):
        raise ResearchContractError("gateway effect is outside exchange gate")


def negotiate_security_federation(request: Mapping[str, Any], capabilities: list[Mapping[str, Any]]) -> dict[str, Any]:
    if request.get("schema_version") != INPUT_SCHEMA or not all(isinstance(request.get(key), str) and request[key].strip() for key in ("request_id", "federation_id", "consumer", "purpose", "scope", "semantic_profile")) or not request.get("required_capability_order") or not _ordered(request["required_capability_order"]) or not _ordered(request.get("required_provider_order", [])) or not _ordered(request.get("adversarial_event_order", [])) or not _valid_digest(request.get("replay_identity")) or request.get("budget_units", 0) <= 0 or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not capabilities:
        raise ResearchContractError("request identity, required closure, digest, budget, locality, or boundary is invalid")
    rows = sorted((dict(row) for row in capabilities), key=lambda row: row.get("capability_id", ""))
    ids = [row.get("capability_id", "") for row in rows]
    if not all(ids) or len(ids) != len(set(ids)):
        raise ResearchContractError("capability identifiers must be unique and non-empty")
    selected: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); missing: set[str] = set(); providers: set[str] = set(); selected_providers: set[str] = set(); protocols: set[str] = set(); migration: set[str] = set(); semantic_loss: set[str] = set(); omission: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for row in rows:
        cid = row["capability_id"]; provider = row.get("provider_id", ""); providers.add(provider); protocols.add(f"{row.get('protocol', '')}@{row.get('schema_version', '')}")
        semantic_loss.update(f"{cid}:{item}" for item in row.get("semantic_loss_order", [])); omission.update(f"{cid}:{item}" for item in row.get("omission_order", [])); uncertainty.update(f"{cid}:{item}" for item in row.get("uncertainty_order", []))
        if row.get("negative_result") or row.get("evidence_state") == "negative": negative.add(f"{cid}:negative-result")
        if row.get("revoked") or not row.get("permitted") or not row.get("signed") or row.get("raw_data_local") is not True or row.get("aggregate_only") is not True or row.get("purpose") != request["purpose"]:
            denied.add(cid); omission.add(f"{cid}:permission-or-locality-denied")
        elif row.get("semantic_profile") != request["semantic_profile"]:
            unresolved.add(cid); migration.add(f"{cid}:semantic-profile:{row.get('semantic_profile', '')}->{request['semantic_profile']}"); uncertainty.add(f"{cid}:semantic-profile-mismatch")
        elif row.get("replay_identity") != request["replay_identity"]:
            unresolved.add(cid); uncertainty.add(f"{cid}:replay-mismatch")
        elif row.get("evidence_state") not in {"proven", "supported"}:
            unresolved.add(cid); uncertainty.add(f"{cid}:evidence-not-supported")
        else:
            selected.add(cid); selected_providers.add(provider)
    for required in request["required_capability_order"]:
        if required not in ids: missing.add(required); omission.add(f"request:missing-capability:{required}")
    for required in request.get("required_provider_order", []):
        if required not in selected_providers: omission.add(f"request:missing-provider:{required}")
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "network_available", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_event_order"))
    if global_block:
        denied.update(ids); selected.clear(); unresolved.clear(); omission.add("request:security-policy-protected-closure-or-network-blocked")
    uncertainty.update(f"adversarial:{event}" for event in request.get("adversarial_event_order", []))
    selected_order = sorted(selected); unresolved_order = sorted(unresolved); denied_order = sorted(denied); missing_order = sorted(missing); provider_order = sorted(providers | set(request.get("required_provider_order", []))); selected_provider_order = sorted(selected_providers); missing_provider_order = sorted(set(request.get("required_provider_order", [])) - set(selected_provider_order))
    if global_block or (not selected_order and not unresolved_order): omission.add("request:federation-closure-not-ready")
    disposition = "blocked" if global_block or (not selected_order and not unresolved_order) else "unresolved" if missing_order or missing_provider_order or denied_order or unresolved_order else "qualified"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "consumer": request["consumer"], "purpose": request["purpose"], "scope": request["scope"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "capability_order": ids, "selected_capability_order": selected_order, "unresolved_capability_order": unresolved_order, "denied_capability_order": denied_order, "missing_capability_order": missing_order, "provider_order": provider_order, "selected_provider_order": selected_provider_order, "missing_provider_order": missing_provider_order, "protocol_order": sorted(protocols), "migration_order": sorted(migration), "semantic_loss_order": sorted(semantic_loss), "omission_order": sorted(omission), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "adversarial_event_order": sorted(request.get("adversarial_event_order", [])), "reasons": ["all-required-capabilities-qualified"] if disposition == "qualified" else [f"disposition:{disposition}", "partial-and-negative-evidence-retained"], "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    envelope_digest = _digest(payload); payload["envelope_digest"] = envelope_digest; payload["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"federation-envelope-6:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": envelope_digest, "semantic_loss": [{"field": item, "reason": "peer-declared semantic loss or migration boundary", "severity": "unknown"} for item in sorted(semantic_loss)], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"exchange:permitted-artifacts:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    validate_security_federation_envelope(payload)
    return payload


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "security_federation_interoperability_gateway_manifest", "negotiate_security_federation", "validate_security_federation_envelope"]
