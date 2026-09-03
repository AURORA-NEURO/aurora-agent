"""Digest-only federated context control plane for Worldgen P03 F29-F32."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

CONTENT_TYPE = "application/vnd.aurora.worldgen.context-control-plane-receipt+json"
_HEX = re.compile(r"^[0-9a-f]{64}$")

@dataclass(frozen=True)
class ContextControlAttestation:
    attestation_id: str; site_id: str; context_digest: str; support_milli: int; freshness_milli: int; evidence_state: str; provenance_digest: str; replay_identity: str; permitted: bool = True; raw_data_local: bool = True; aggregate_only: bool = True; boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ContextControlPlaneRequest:
    program_id: str; attestations: tuple[ContextControlAttestation, ...]; minimum_support_milli: int; minimum_freshness_milli: int; minimum_site_quorum: int; requested_action_order: tuple[str, ...]; action_budget: int; signed_approval: bool = False; federation_approved: bool = False; replay_identity: str = ""; boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ContextControlPlaneReceipt:
    value: dict[str, Any]

    def validate(self, *, feature_id: str, contract_version: str) -> None:
        value, artifact = self.value, self.value.get("artifact", {})
        candidates = set(value.get("candidate_order", ()))
        states = set(value.get("admitted_order", ())) | set(value.get("unresolved_order", ())) | set(value.get("blocked_order", ()))
        actions = set(value.get("action_order", ()))
        action_parts = set(value.get("admitted_action_order", ())) | set(value.get("denied_action_order", ()))
        valid = (value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and value.get("contract_version") == contract_version and value.get("feature_id") == feature_id and value.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("content_type") == CONTENT_TYPE and artifact.get("raw_attestations") is False and value.get("raw_data_local") is True and value.get("aggregate_only") is True and value.get("program_id", "").strip() and candidates and states == candidates and actions and action_parts == actions and value.get("effect_receipts") and _HEX.fullmatch(value.get("replay_identity", "")) and _HEX.fullmatch(value.get("control_digest", "")) and artifact.get("content_hash") == value.get("control_digest"))
        if not valid:
            raise ResearchContractError("context control-plane identity, partitions, locality, digests, or effects are incomplete")
        for key in ("candidate_order", "admitted_order", "unresolved_order", "blocked_order", "site_order", "action_order", "admitted_action_order", "denied_action_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts"):
            values = tuple(value.get(key, ()))
            if values != tuple(sorted(set(values))):
                raise ResearchContractError("context control-plane ordering is not canonical")
        if any(effect != "block:unsafe-release" and not effect.startswith("control:worldgen-context:") for effect in value["effect_receipts"]):
            raise ResearchContractError("context control-plane effect is outside governance gate")

    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id, contract_version=contract_version)
        return _digest(self.value)

def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["consortium operator", "research program lead", "federated benchmark steward"], "behavior": f"rank and govern digest-only context attestations for {scale}", "value": "makes federated context admission, quorum, freshness, and action governance deterministic and auditable", "input_schema": input_schema, "output_schema": "FederatedContextControlReceipt1@1", "effects": ["control:worldgen-context", "block:unsafe-release"], "permissions": ["control:aggregate-context-attestations"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY}

def control(request: ContextControlPlaneRequest, *, feature_id: str, contract_version: str, scale: str, require_approval: bool, require_federation: bool) -> ContextControlPlaneReceipt:
    if (not request.program_id.strip() or not request.attestations or not request.requested_action_order or request.action_budget <= 0 or request.minimum_site_quorum <= 0 or request.boundary != PRECLINICAL_BOUNDARY or tuple(request.requested_action_order) != tuple(sorted(set(request.requested_action_order))) or not _HEX.fullmatch(request.replay_identity)):
        raise ResearchContractError("context control-plane program, attestations, actions, budget, boundary, ordering, or replay is invalid")
    if any((not attestation.attestation_id.strip() or not attestation.site_id.strip() or not _HEX.fullmatch(attestation.context_digest) or not _HEX.fullmatch(attestation.provenance_digest) or not _HEX.fullmatch(attestation.replay_identity) or attestation.replay_identity != request.replay_identity or not attestation.raw_data_local or not attestation.aggregate_only or attestation.boundary != PRECLINICAL_BOUNDARY) for attestation in request.attestations):
        raise ResearchContractError("context attestation identity, replay, provenance, locality, or boundary is invalid")
    candidates = sorted(request.attestations, key=lambda attestation: (-attestation.support_milli, -attestation.freshness_milli, attestation.attestation_id))
    candidate_order = sorted(attestation.attestation_id for attestation in candidates)
    admitted, unresolved, blocked = set(), set(), set()
    for attestation in candidates:
        if not attestation.permitted: blocked.add(attestation.attestation_id)
        elif attestation.evidence_state != "supported" or attestation.support_milli < request.minimum_support_milli or attestation.freshness_milli < request.minimum_freshness_milli: unresolved.add(attestation.attestation_id)
        else: admitted.add(attestation.attestation_id)
    sites = sorted({attestation.site_id for attestation in candidates if attestation.attestation_id in admitted})
    approvals_ok = (not require_approval or request.signed_approval) and (not require_federation or request.federation_approved)
    quorum_ok = len(sites) >= request.minimum_site_quorum
    actions_ok = len(request.requested_action_order) <= request.action_budget
    safe = approvals_ok and quorum_ok and actions_ok and bool(admitted) and not unresolved and not blocked
    disposition = "blocked" if not approvals_ok or not quorum_ok else "qualified" if safe else "partial"
    omissions = sorted(set(([] if approvals_ok else ["control:approval-missing"]) + ([] if quorum_ok else ["control:site-quorum-missing"]) + ([] if actions_ok else ["control:action-budget-exceeded"]) + ([] if not unresolved else ["control:unsupported-or-stale-attestation"]) + ([] if not blocked else ["control:permitted-attestation-missing"])))
    admitted_actions = list(request.requested_action_order) if safe else []
    denied_actions = [] if safe else sorted(request.requested_action_order)
    effects = [f"control:worldgen-context:{request.program_id}"] if disposition == "qualified" else ["block:unsafe-release"]
    uncertainty = [] if safe else ["control:qualification-requires-complete-attestations"]
    negative = [] if actions_ok else ["control:action-budget-negative"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "program_id": request.program_id, "scale": scale, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": sorted(admitted), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "site_order": sites, "action_order": list(request.requested_action_order), "admitted_action_order": admitted_actions, "denied_action_order": denied_actions, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effects, "raw_attestations": False, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    control_digest = _digest(payload)
    payload["control_digest"] = control_digest
    payload["artifact"] = {"artifact_id": f"worldgen-context-control:{request.program_id}", "content_type": CONTENT_TYPE, "content_hash": control_digest, "raw_attestations": False, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    receipt = ContextControlPlaneReceipt(payload)
    receipt.validate(feature_id=feature_id, contract_version=contract_version)
    return receipt

__all__ = ["CONTENT_TYPE", "ContextControlAttestation", "ContextControlPlaneRequest", "ContextControlPlaneReceipt", "manifest", "control"]
