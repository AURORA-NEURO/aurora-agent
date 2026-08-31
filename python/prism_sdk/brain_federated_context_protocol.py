"""Federated continual context protocol parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    FEDERATED_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEDERATED_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

PROTOCOL_VERSION = "aurora-research-context-federated/1.0"
ROUTE = "/v1/research/context/federated/compile"
METHOD = "POST"
RESPONSE_SCHEMA = "FederatedContextProtocolResponse1@1"


@dataclass(frozen=True)
class FederatedContextProtocolPeer:
    institution_id: str
    endpoint: str
    semantic_profile: str
    context_digest: str
    section_digest: str
    replay_identity: str
    state: str = "supported"
    signed_approval: bool = True
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class FederatedContextProtocolReceipt:
    request_id: str
    federation_id: str
    purpose: str
    scope: str
    goal: str
    semantic_profile: str
    protocol_version: str
    method: str
    route: str
    content_type: str
    idempotency_key: str
    response_schema: str
    status_code: int
    disposition: str
    institution_order: tuple[str, ...]
    endpoint_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    minimum_quorum: int
    quorum: int
    checkpoint_seq: int
    envelope_digest: str
    context_digest: str
    section_digest: str
    request_digest: str
    response_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID
    contract_version: str = FEDERATED_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID or self.contract_version != FEDERATED_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION):
            raise ResearchContractError("federated context protocol schema, feature, or version mismatch")
        if (self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.scope.strip() or not self.goal.strip() or not self.semantic_profile.strip() or len(self.institution_order) < 2 or not self.candidate_order or self.minimum_quorum < 1 or self.quorum != len(self.admitted_order) or self.quorum > len(self.candidate_order) or not self.effect_receipts):
            raise ResearchContractError("federated context protocol identity, quorum, locality, or effects are incomplete")
        for values in (self.institution_order, self.endpoint_order, self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated context protocol ordering is not canonical")
        classified = set(self.admitted_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != set(self.candidate_order):
            raise ResearchContractError("federated context protocol states do not partition candidates")
        if self.status_code not in {200, 202, 206, 403, 422} or self.disposition not in {"ready", "partial", "unknown", "blocked"}:
            raise ResearchContractError("federated context protocol status or disposition is invalid")
        for value in (self.envelope_digest, self.context_digest, self.section_digest, self.request_digest, self.response_digest, self.replay_identity, *self.aggregate_order, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated context protocol digest is invalid")
        if any(not effect.startswith("protocol:federated-context-response:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated context protocol effect is outside the governed response gate")
        if not self.raw_data_local and (self.disposition != "blocked" or "protocol:raw-data-locality-failed" not in self.omissions):
            raise ResearchContractError("raw-data locality failure must remain blocked and explicit")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "purpose": self.purpose, "scope": self.scope, "goal": self.goal, "semantic_profile": self.semantic_profile, "protocol_version": self.protocol_version, "method": self.method, "route": self.route, "content_type": self.content_type, "idempotency_key": self.idempotency_key, "response_schema": self.response_schema, "status_code": self.status_code, "disposition": self.disposition, "institution_order": list(self.institution_order), "endpoint_order": list(self.endpoint_order), "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "aggregate_order": list(self.aggregate_order), "minimum_quorum": self.minimum_quorum, "quorum": self.quorum, "checkpoint_seq": self.checkpoint_seq, "envelope_digest": self.envelope_digest, "context_digest": self.context_digest, "section_digest": self.section_digest, "request_digest": self.request_digest, "response_digest": self.response_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "aggregate_only": self.aggregate_only, "boundary": self.boundary})


def serve_federated_context_protocol(*, request_id: str, federation_id: str, purpose: str, scope: str, goal: str, semantic_profile: str, peers: Sequence[FederatedContextProtocolPeer], minimum_quorum: int, idempotency_key: str, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, signer_valid: bool = True, raw_data_local: bool = True, aggregate_only: bool = True, protocol_version: str = PROTOCOL_VERSION, method: str = METHOD, route: str = ROUTE, content_type: str = "application/json", response_schema: str = RESPONSE_SCHEMA) -> FederatedContextProtocolReceipt:
    if (not request_id.strip() or not federation_id.strip() or not purpose.strip() or not scope.strip() or not goal.strip() or not semantic_profile.strip() or len(peers) < 2 or minimum_quorum < 1 or minimum_quorum > len(peers) or not idempotency_key.strip() or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or (protocol_version, method, route, content_type, response_schema) != (PROTOCOL_VERSION, METHOD, ROUTE, "application/json", RESPONSE_SCHEMA)):
        raise ResearchContractError("federated context protocol identity, route, quorum, replay, or version is invalid")
    ordered = tuple(sorted(peers, key=lambda peer: peer.institution_id)); candidate = tuple(peer.institution_id for peer in ordered)
    if any(not value.strip() for value in candidate) or len(set(candidate)) != len(candidate):
        raise ResearchContractError("federated institution identifiers must be unique and non-empty")
    admitted: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); aggregate: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); open_gate = policy_allow and protected_closure and signer_valid and raw_data_local and aggregate_only
    for peer in ordered:
        if not open_gate or not peer.signed_approval or not peer.raw_data_local or not peer.aggregate_only or peer.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(peer.institution_id); omissions.add(f"institution:{peer.institution_id}:policy-approval-locality-blocked")
        elif peer.semantic_profile != semantic_profile:
            blocked.add(peer.institution_id); negative.add(f"institution:{peer.institution_id}:semantic-profile-mismatch")
        elif peer.replay_identity != replay_identity:
            unknown.add(peer.institution_id); uncertainty.add(f"institution:{peer.institution_id}:replay-mismatch")
        elif peer.state in {"unknown", "speculative"}:
            unknown.add(peer.institution_id); uncertainty.add(f"institution:{peer.institution_id}:evidence-uncertain")
        elif peer.state == "contradicted":
            blocked.add(peer.institution_id); negative.add(f"institution:{peer.institution_id}:contradicted")
        else:
            admitted.add(peer.institution_id); aggregate.add(peer.context_digest)
    disposition = "blocked" if not open_gate else "ready" if len(admitted) == len(candidate) and len(admitted) >= minimum_quorum else "unknown" if unknown and len(admitted) < minimum_quorum else "partial" if len(admitted) >= minimum_quorum else "blocked"; status = 403 if not open_gate else 200 if disposition == "ready" else 206 if disposition == "partial" else 202 if disposition == "unknown" else 422
    if not policy_allow: omissions.add("protocol:policy-denied")
    if not protected_closure: omissions.add("protocol:protected-closure-incomplete")
    if not signer_valid: omissions.add("protocol:signer-invalid")
    if not raw_data_local: omissions.add("protocol:raw-data-locality-failed")
    if not aggregate_only: omissions.add("protocol:aggregate-only-required")
    institution_order = candidate; endpoint_order = tuple(peer.endpoint for peer in ordered); aggregate_order = tuple(sorted(aggregate)); quorum = len(admitted); checkpoint_seq = len(ordered); envelope_digest = research_artifact_digest({"federation_id": federation_id, "purpose": purpose, "semantic_profile": semantic_profile, "candidate_order": list(candidate), "aggregate_order": list(aggregate_order), "replay_identity": replay_identity, "aggregate_only": aggregate_only}); context_digest = research_artifact_digest({"scope": scope, "envelope_digest": envelope_digest, "quorum": quorum}); section_digest = research_artifact_digest({"goal": goal, "context_digest": context_digest, "admitted_order": sorted(admitted)}); request_digest = research_artifact_digest({"request_id": request_id, "federation_id": federation_id, "purpose": purpose, "scope": scope, "candidate_order": list(candidate), "replay_identity": replay_identity}); response_digest = research_artifact_digest({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request_id, "status_code": status, "disposition": disposition, "envelope_digest": envelope_digest, "replay_identity": replay_identity}); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.federated-context-protocol-response+json"}
    receipt = FederatedContextProtocolReceipt(request_id=request_id, federation_id=federation_id, purpose=purpose, scope=scope, goal=goal, semantic_profile=semantic_profile, protocol_version=PROTOCOL_VERSION, method=METHOD, route=ROUTE, content_type="application/json", idempotency_key=idempotency_key, response_schema=RESPONSE_SCHEMA, status_code=status, disposition=disposition, institution_order=institution_order, endpoint_order=endpoint_order, candidate_order=candidate, admitted_order=tuple(sorted(admitted)), blocked_order=tuple(sorted(blocked)), unknown_order=tuple(sorted(unknown)), aggregate_order=aggregate_order, minimum_quorum=minimum_quorum, quorum=quorum, checkpoint_seq=checkpoint_seq, envelope_digest=envelope_digest, context_digest=context_digest, section_digest=section_digest, request_digest=request_digest, response_digest=response_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"protocol:federated-context-response:{idempotency_key}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local, aggregate_only=aggregate_only)
    receipt.validate(); return receipt
