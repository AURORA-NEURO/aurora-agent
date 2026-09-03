"""Multimodal context-compilation protocol parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

PROTOCOL_VERSION = "aurora-research-context-multimodal/1.0"
ROUTE = "/v1/research/context/multimodal/compile"
METHOD = "POST"
RESPONSE_SCHEMA = "MultimodalContextProtocolResponse1@1"


@dataclass(frozen=True)
class MultimodalContextProtocolCell:
    study_id: str
    modality: str
    context_digest: str
    section_digest: str
    replay_identity: str
    state: str = "supported"
    comparable: bool = True
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class MultimodalContextProtocolReceipt:
    request_id: str
    query_id: str
    scope: str
    protocol_version: str
    method: str
    route: str
    content_type: str
    idempotency_key: str
    response_schema: str
    status_code: int
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    cell_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    incompatible_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    context_digest: str
    section_digest: str
    comparability_digest: str
    request_digest: str
    response_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID
    contract_version: str = MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID or self.contract_version != MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION):
            raise ResearchContractError("multimodal context protocol schema, feature, or version mismatch")
        if (self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.query_id.strip() or not self.scope.strip() or self.protocol_version != PROTOCOL_VERSION or self.method != METHOD or self.route != ROUTE or self.content_type != "application/json" or not self.idempotency_key.strip() or self.response_schema != RESPONSE_SCHEMA or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.cell_order or not self.effect_receipts or self.status_code not in {200, 202, 206, 403, 422} or self.disposition not in {"ready", "partial", "unknown", "blocked"}):
            raise ResearchContractError("multimodal protocol identity, route, coverage, idempotency, locality, or effects are incomplete")
        if any(value not in self.cell_order for value in (*self.qualified_order, *self.missing_order, *self.incompatible_order, *self.unknown_order)):
            raise ResearchContractError("multimodal protocol state is not covered by cells")
        for values in (self.study_order, self.modality_order, self.cell_order, self.qualified_order, self.missing_order, self.incompatible_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal protocol ordering is not canonical")
        for value in (self.context_digest, self.section_digest, self.comparability_digest, self.request_digest, self.response_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal protocol digest is invalid")
        if any(not effect.startswith("protocol:local-multimodal-context-response:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal protocol effect is outside local response gate")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "query_id": self.query_id, "scope": self.scope, "protocol_version": self.protocol_version, "method": self.method, "route": self.route, "content_type": self.content_type, "idempotency_key": self.idempotency_key, "response_schema": self.response_schema, "status_code": self.status_code, "disposition": self.disposition, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "cell_order": list(self.cell_order), "qualified_order": list(self.qualified_order), "missing_order": list(self.missing_order), "incompatible_order": list(self.incompatible_order), "unknown_order": list(self.unknown_order), "context_digest": self.context_digest, "section_digest": self.section_digest, "comparability_digest": self.comparability_digest, "request_digest": self.request_digest, "response_digest": self.response_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def serve_multimodal_context_protocol(*, request_id: str, query_id: str, scope: str, goal: str, study_ids: Sequence[str], required_modalities: Sequence[str], cells: Sequence[MultimodalContextProtocolCell], idempotency_key: str, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, protocol_version: str = PROTOCOL_VERSION, method: str = METHOD, route: str = ROUTE, content_type: str = "application/json", response_schema: str = RESPONSE_SCHEMA) -> MultimodalContextProtocolReceipt:
    if (not request_id.strip() or not query_id.strip() or not scope.strip() or not goal.strip() or len(study_ids) < 2 or len(required_modalities) < 2 or not idempotency_key.strip() or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or protocol_version != PROTOCOL_VERSION or method != METHOD or route != ROUTE or content_type != "application/json" or response_schema != RESPONSE_SCHEMA):
        raise ResearchContractError("multimodal context protocol identity, route, idempotency, coverage, replay, or version is invalid")
    studies = tuple(sorted(set(study_ids))); modalities = tuple(sorted(set(required_modalities)))
    if len(studies) != len(study_ids) or len(modalities) != len(required_modalities) or any(not value.strip() for value in (*studies, *modalities)):
        raise ResearchContractError("study and modality identifiers must be unique and non-empty")
    expected = {f"{study}|{modality}" for study in studies for modality in modalities}; cell_map: dict[str, MultimodalContextProtocolCell] = {}
    for cell in cells:
        key = f"{cell.study_id}|{cell.modality}"
        if key in cell_map: raise ResearchContractError("multimodal protocol cells must be unique")
        cell_map[key] = cell
    qualified: set[str] = set(); missing: set[str] = set(); incompatible: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for key in expected:
        cell = cell_map.get(key)
        if cell is None: missing.add(key); omissions.add(f"cell:{key}:missing")
        elif not policy_allow or not protected_closure or not raw_data_local or not cell.raw_data_local or cell.boundary != PRECLINICAL_BOUNDARY: incompatible.add(key); omissions.add(f"cell:{key}:policy-locality-blocked")
        elif not cell.comparable: incompatible.add(key); negative.add(f"cell:{key}:incomparable")
        elif cell.replay_identity != replay_identity: unknown.add(key); uncertainty.add(f"cell:{key}:replay-mismatch")
        elif cell.state in {"proven", "supported"}: qualified.add(key)
        elif cell.state in {"speculative", "unknown"}: unknown.add(key); uncertainty.add(f"cell:{key}:evidence-uncertain")
        else: incompatible.add(key); negative.add(f"cell:{key}:contradicted")
    gates_open = policy_allow and protected_closure and raw_data_local; disposition = "blocked" if not gates_open else ("ready" if len(qualified) == len(expected) else ("unknown" if unknown else "partial")); status_code = 403 if not gates_open else 200 if disposition == "ready" else 206 if disposition == "partial" else 202
    if not policy_allow: omissions.add("protocol:policy-denied")
    if not protected_closure: omissions.add("protocol:protected-closure-incomplete")
    if not raw_data_local: omissions.add("protocol:raw-data-locality-failed")
    cell_order = tuple(sorted(expected)); context_digest = research_artifact_digest({"study_order": list(studies), "modality_order": list(modalities), "qualified_order": sorted(qualified), "replay_identity": replay_identity}); comparability_digest = research_artifact_digest({"study_order": list(studies), "modality_order": list(modalities), "cell_order": list(cell_order), "qualified_order": sorted(qualified), "replay_identity": replay_identity}); section_digest = research_artifact_digest({"scope": scope, "context_digest": context_digest, "comparability_digest": comparability_digest, "qualified_order": sorted(qualified)}); request_digest = research_artifact_digest({"request_id": request_id, "query_id": query_id, "scope": scope, "study_order": list(studies), "modality_order": list(modalities), "replay_identity": replay_identity}); response_digest = research_artifact_digest({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request_id, "status_code": status_code, "disposition": disposition, "comparability_digest": comparability_digest, "replay_identity": replay_identity}); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.multimodal-context-protocol-response+json"}
    receipt = MultimodalContextProtocolReceipt(request_id=request_id, query_id=query_id, scope=scope, protocol_version=PROTOCOL_VERSION, method=METHOD, route=ROUTE, content_type="application/json", idempotency_key=idempotency_key, response_schema=RESPONSE_SCHEMA, status_code=status_code, disposition=disposition, study_order=studies, modality_order=modalities, cell_order=cell_order, qualified_order=tuple(sorted(qualified)), missing_order=tuple(sorted(missing)), incompatible_order=tuple(sorted(incompatible)), unknown_order=tuple(sorted(unknown)), context_digest=context_digest, section_digest=section_digest, comparability_digest=comparability_digest, request_digest=request_digest, response_digest=response_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"protocol:local-multimodal-context-response:{idempotency_key}",) if gates_open else ("block:unsafe-release",), artifact=artifact)
    receipt.validate(); return receipt
