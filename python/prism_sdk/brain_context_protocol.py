"""Local Decision-Section context protocol parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

PROTOCOL_VERSION = "aurora-research-context/1.0"
ROUTE = "/v1/research/context/compile"
METHOD = "POST"
RESPONSE_SCHEMA = "ContextProtocolResponse1@1"


@dataclass(frozen=True)
class ContextProtocolCandidate:
    context_id: str
    context_digest: str
    section_digest: str
    replay_identity: str
    state: str = "supported"
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class ContextProtocolReceipt:
    request_id: str
    query_id: str
    study_id: str
    scope: str
    protocol_version: str
    method: str
    route: str
    content_type: str
    idempotency_key: str
    response_schema: str
    status_code: int
    disposition: str
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
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
    feature_id: str = CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID
    contract_version: str = CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID
                or self.contract_version != CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION):
            raise ResearchContractError("context protocol schema, feature, or version mismatch")
        if (self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip()
                or not self.query_id.strip() or not self.study_id.strip() or not self.scope.strip()
                or self.protocol_version != PROTOCOL_VERSION or self.method != METHOD or self.route != ROUTE
                or self.content_type != "application/json" or not self.idempotency_key.strip()
                or self.response_schema != RESPONSE_SCHEMA or not self.candidate_order or not self.effect_receipts
                or self.status_code not in {200, 202, 206, 403, 422}
                or self.disposition not in {"ready", "partial", "unknown", "blocked"}):
            raise ResearchContractError("context protocol identity, route, idempotency, candidates, locality, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("context protocol state is not covered by candidates")
        for values in (self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("context protocol ordering is invalid")
        for value in (self.context_digest, self.section_digest, self.request_digest, self.response_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("context protocol digest is invalid")
        if any(not effect.startswith("protocol:local-context-response:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("context protocol effect is outside local response gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "query_id": self.query_id, "study_id": self.study_id, "scope": self.scope, "protocol_version": self.protocol_version, "method": self.method, "route": self.route, "content_type": self.content_type, "idempotency_key": self.idempotency_key, "response_schema": self.response_schema, "status_code": self.status_code, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "context_digest": self.context_digest, "section_digest": self.section_digest, "request_digest": self.request_digest, "response_digest": self.response_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def serve_context_protocol(*, request_id: str, query_id: str, study_id: str, scope: str, goal: str,
                           required_context_ids: Sequence[str], candidates: Sequence[ContextProtocolCandidate],
                           idempotency_key: str, replay_identity: str, policy_allow: bool = True,
                           protected_closure: bool = True, raw_data_local: bool = True,
                           protocol_version: str = PROTOCOL_VERSION, method: str = METHOD,
                           route: str = ROUTE, content_type: str = "application/json",
                           response_schema: str = RESPONSE_SCHEMA) -> ContextProtocolReceipt:
    if (not request_id.strip() or not query_id.strip() or not study_id.strip() or not scope.strip() or not goal.strip()
            or not idempotency_key.strip() or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)
            or protocol_version != PROTOCOL_VERSION or method != METHOD or route != ROUTE
            or content_type != "application/json" or response_schema != RESPONSE_SCHEMA):
        raise ResearchContractError("context protocol version, route, idempotency, response schema, replay, or identity is invalid")
    required = tuple(sorted(set(required_context_ids)))
    if not required or len(required) != len(required_context_ids) or any(not value.strip() for value in required):
        raise ResearchContractError("required context identifiers must be unique and non-empty")
    candidate_map: dict[str, ContextProtocolCandidate] = {}
    for candidate in candidates:
        if candidate.context_id in candidate_map:
            raise ResearchContractError("context protocol candidates must be unique")
        candidate_map[candidate.context_id] = candidate
    qualified: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for context_id in required:
        candidate = candidate_map.get(context_id)
        if candidate is None:
            unknown.add(context_id); omissions.add(f"context:{context_id}:missing"); continue
        if not policy_allow or not protected_closure or not raw_data_local or not candidate.raw_data_local or candidate.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(context_id); omissions.add(f"context:{context_id}:policy-locality-blocked")
        elif candidate.replay_identity != replay_identity:
            unknown.add(context_id); uncertainty.add(f"context:{context_id}:replay-mismatch")
        elif candidate.state in {"proven", "supported"}:
            qualified.add(context_id)
        elif candidate.state in {"speculative", "unknown"}:
            unknown.add(context_id); uncertainty.add(f"context:{context_id}:evidence-uncertain")
        else:
            blocked.add(context_id); negative.add(f"context:{context_id}:contradicted")
    gates_open = policy_allow and protected_closure and raw_data_local
    disposition = "blocked" if not gates_open else ("ready" if len(qualified) == len(required) else ("unknown" if unknown else "partial"))
    status_code = 403 if not gates_open else (200 if disposition == "ready" else 206 if disposition == "partial" else 202)
    if not policy_allow: omissions.add("protocol:policy-denied")
    if not protected_closure: omissions.add("protocol:protected-closure-incomplete")
    if not raw_data_local: omissions.add("protocol:raw-data-locality-failed")
    context_digest = research_artifact_digest({"required_order": list(required), "qualified_order": sorted(qualified), "replay_identity": replay_identity})
    section_digest = research_artifact_digest({"study_id": study_id, "scope": scope, "context_digest": context_digest, "qualified_order": sorted(qualified)})
    request_digest = research_artifact_digest({"request_id": request_id, "query_id": query_id, "study_id": study_id, "scope": scope, "goal": goal, "required_context_order": list(required), "replay_identity": replay_identity})
    response_digest = research_artifact_digest({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request_id, "status_code": status_code, "disposition": disposition, "context_digest": context_digest, "section_digest": section_digest, "replay_identity": replay_identity})
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.context-protocol-response+json"}
    receipt = ContextProtocolReceipt(request_id=request_id, query_id=query_id, study_id=study_id, scope=scope, protocol_version=PROTOCOL_VERSION, method=METHOD, route=ROUTE, content_type="application/json", idempotency_key=idempotency_key, response_schema=RESPONSE_SCHEMA, status_code=status_code, disposition=disposition, candidate_order=required, qualified_order=tuple(sorted(qualified)), blocked_order=tuple(sorted(blocked)), unknown_order=tuple(sorted(unknown)), context_digest=context_digest, section_digest=section_digest, request_digest=request_digest, response_digest=response_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"protocol:local-context-response:{idempotency_key}",) if gates_open else ("block:unsafe-release",), artifact=artifact)
    receipt.validate(); return receipt
