"""Python mirror of the bioevalx federated release gateway receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION,
    BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BioevalxFederationGatewayReceipt:
    """Cross-language validator for permitted-artifact federation exchange."""

    request_id: str
    workflow_id: str
    federation_id: str
    endpoint: str
    protocol: str
    disposition: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    release_order: tuple[str, ...]
    artifact_order: tuple[str, ...]
    evidence_order: tuple[str, ...]
    provenance_order: tuple[str, ...]
    replay_order: tuple[str, ...]
    benchmark_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    benchmark_digest: str | None
    effect_receipts: tuple[str, ...]
    objects: tuple[Mapping[str, Any], ...]
    federation_artifact: Mapping[str, Any]
    feature_id: str = BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID
    contract_version: str = BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID or self.contract_version != BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION:
            raise ResearchContractError("bioevalx federation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.federation_id.strip() or not self.endpoint.strip() or not self.protocol.strip() or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("federation identity, endpoint, protocol, locality, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("federation disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("federation candidate state is not covered by candidate order")
        for values in (self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.release_order, self.artifact_order, self.evidence_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federation ordering is invalid")
        for values in (self.provenance_order, self.replay_order, self.benchmark_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federation digest ordering is invalid")
        digests = (*self.provenance_order, *self.replay_order, *self.benchmark_order, self.replay_identity, self.federation_artifact.get("content_hash"))
        if self.benchmark_digest is not None:
            digests += (self.benchmark_digest,)
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
            raise ResearchContractError("federation digest is invalid")
        for obj in self.objects:
            if obj.get("raw_data_local") is not True or obj.get("boundary") != PRECLINICAL_BOUNDARY or obj.get("endpoint") != self.endpoint or obj.get("protocol") != self.protocol or not obj.get("artifact_ids") or not obj.get("evidence_receipt_ids"):
                raise ResearchContractError("federation object is incomplete or inconsistent")
        if self.admitted_order and any(not effect.startswith("exchange:permitted-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted releases require permitted-artifact exchange")
        if not self.admitted_order and self.effect_receipts != ("block:federation-release",):
            raise ResearchContractError("empty federation result must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "federation_id": self.federation_id,
            "endpoint": self.endpoint,
            "protocol": self.protocol,
            "disposition": self.disposition,
            "candidate_order": list(self.candidate_order),
            "admitted_order": list(self.admitted_order),
            "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order),
            "release_order": list(self.release_order),
            "artifact_order": list(self.artifact_order),
            "evidence_order": list(self.evidence_order),
            "provenance_order": list(self.provenance_order),
            "replay_order": list(self.replay_order),
            "benchmark_order": list(self.benchmark_order),
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "replay_identity": self.replay_identity,
            "benchmark_digest": self.benchmark_digest,
            "effect_receipts": list(self.effect_receipts),
            "objects": [dict(obj) for obj in self.objects],
            "federation_artifact": dict(self.federation_artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })
