"""Python parity for ``AFA-ids-P30-F18`` adversarial recovery previews."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P30-F18"
CONTRACT_VERSION = "ids-adversarial-recovery-research-workbench/1.0"
INPUT_SCHEMA = "IdsAdversarialRecoveryRequest8@1"
OUTPUT_SCHEMA = "IdsAdversarialRecoveryReceipt10@1"
CONTENT_TYPE = "application/vnd.aurora.ids-adversarial-recovery-receipt-10+json"
MAX_EVENTS = 16_384


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def _hostile(kind: str) -> bool:
    return kind in {"revoked_key", "poisoned_artifact", "prompt_injection", "compromised_connector", "resource_exhaustion", "unauthorized_data_movement", "instrument_preflight_failure"}


@dataclass(frozen=True)
class IdsAdversarialRecoveryReceipt10:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value; artifact = value.get("artifact", {})
        if (value.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or value.get("contract_version") != CONTRACT_VERSION or value.get("feature_id") != FEATURE_ID or value.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or value.get("raw_data_local") is not True or value.get("aggregate_only") is not True or not all(isinstance(value.get(key), str) and value[key].strip() for key in ("request_id", "workflow_id", "purpose", "semantic_profile")) or not value.get("event_order") or not value.get("effect_order") or not value.get("effect_receipts") or value.get("disposition") not in {"qualified", "unresolved", "blocked"}):
            raise ResearchContractError("recovery identity, events, effects, locality, or disposition is incomplete")
        for key in ("event_order", "recovered_order", "unresolved_order", "blocked_order", "hostile_order", "replay_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_order", "effect_receipts"):
            if not _ordered(value.get(key, [])): raise ResearchContractError("recovery ordering is not canonical")
        if value.get("checkpoint_digest_order", []) != sorted(set(value.get("checkpoint_digest_order", []))): raise ResearchContractError("checkpoint digest ordering is not canonical")
        ids = set(value["event_order"]); parts = value["recovered_order"] + value["unresolved_order"] + value["blocked_order"]
        if len(ids) != len(value["event_order"]) or len(parts) != len(ids) or set(parts) != ids: raise ResearchContractError("recovery states do not partition")
        if (not _digest(value.get("replay_identity")) or not _digest(value.get("recovery_digest")) or artifact.get("content_hash") != value.get("recovery_digest") or artifact.get("content_type") != CONTENT_TYPE or any(not _digest(d) for d in artifact.get("provenance_digests", []))): raise ResearchContractError("recovery digest or artifact metadata is inconsistent")
        if any(not e.startswith(("preview:adversarial-recovery:", "manage:local-capability:")) and e != "block:unsafe-release" for e in value["effect_receipts"]): raise ResearchContractError("effect is outside the governed recovery gate")

    def digest(self) -> str:
        self.validate(); return _hash(self.to_dict())


def adversarial_recovery_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["researcher", "recovery operator", "security engineer"], "behavior": "classify hostile and failed workflow events into deterministic replayable recovery states with counterexamples and omission witnesses", "value": "makes recovery posture auditable before any retry, connector, instrument, or federation effect is considered", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["preview:adversarial-recovery", "manage:local-capability"], "permissions": ["read:local-recovery-summaries", "request:adversarial-recovery-preview"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if (not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "workflow_id", "purpose", "semantic_profile")) or not request.get("events") or len(request["events"]) > MAX_EVENTS or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True): raise ResearchContractError("recovery identity, event bound, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for event in request["events"]:
        if (not isinstance(event, Mapping) or not isinstance(event.get("event_id"), str) or not event["event_id"].strip() or event["event_id"] in ids or not isinstance(event.get("event_kind"), str) or not event["event_kind"].strip() or not _digest(event.get("payload_digest")) or (event.get("checkpoint_digest") is not None and not _digest(event.get("checkpoint_digest"))) or not _digest(event.get("replay_identity")) or event.get("local") is not True or event.get("aggregate_only") is not True or event.get("evidence_state") not in {"proven", "supported", "unknown", "unmeasured", "contradicted"}): raise ResearchContractError(f"recovery event {event.get('event_id', '')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(event["event_id"])


def preview_adversarial_recovery(request: Mapping[str, Any]) -> IdsAdversarialRecoveryReceipt10:
    _validate_request(request); events = sorted((dict(e) for e in request["events"]), key=lambda e: e["event_id"]); event_order = [e["event_id"] for e in events]
    recovered: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); hostile_order: set[str] = set(); replay: set[str] = set(); checkpoints: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for event in events:
        provenance.add(event["payload_digest"])
        if event.get("checkpoint_digest") is not None: replay.add(event["event_id"]); checkpoints.add(event["checkpoint_digest"])
        is_hostile = _hostile(event["event_kind"])
        if is_hostile: hostile_order.add(event["event_id"]); negative.add(f"{event['event_id']}:adversarial-kind-{event['event_kind']}")
        if event["authorized"] is not True: blocked.add(event["event_id"]); omissions.add(f"{event['event_id']}:authorization-denied")
        elif event["recoverable"] is not True: blocked.add(event["event_id"]); omissions.add(f"{event['event_id']}:non-recoverable")
        elif event["replay_identity"] != request["replay_identity"]: unresolved.add(event["event_id"]); uncertainty.add(f"{event['event_id']}:replay-identity")
        elif event.get("evidence_state") == "contradicted" or is_hostile: blocked.add(event["event_id"])
        elif event.get("evidence_state") not in {"proven", "supported"}: unresolved.add(event["event_id"]); uncertainty.add(f"{event['event_id']}:evidence-state")
        else: recovered.add(event["event_id"])
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(event_order); recovered.clear(); unresolved.clear(); omissions.add("request:governance-or-locality-denied")
    recovered_order = sorted(recovered); unresolved_order = sorted(unresolved); blocked_order = sorted(blocked); disposition = "blocked" if global_block or (not recovered_order and not unresolved_order) else ("unresolved" if blocked_order or unresolved_order else "qualified")
    if disposition != "qualified": omissions.add("request:adversarial-recovery-not-closed")
    effect_order = sorted(["manage:local-capability", "preview:adversarial-recovery"] if disposition == "qualified" else ["block:unsafe-release"])
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "workflow_id": request["workflow_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "event_order": event_order, "recovered_order": recovered_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "hostile_order": sorted(hostile_order), "replay_order": sorted(replay), "checkpoint_digest_order": sorted(checkpoints), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "effect_order": effect_order, "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); value = dict(payload); value["recovery_digest"] = digest; value["artifact"] = {"artifact_id": f"ids-adversarial-recovery-receipt-10:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; value["effect_receipts"] = sorted(e if e == "block:unsafe-release" else f"{e}:{request['request_id']}" for e in effect_order); receipt = IdsAdversarialRecoveryReceipt10(value); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "IdsAdversarialRecoveryReceipt10", "adversarial_recovery_manifest", "preview_adversarial_recovery"]
