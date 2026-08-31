"""Bounded retrieval-synthesis operations control plane for Worldgen P02 F29-F32."""
from __future__ import annotations
from dataclasses import dataclass, replace
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_retrieval_support import RetrievalQuery, infer

CONTENT_TYPE = "application/vnd.aurora.worldgen.retrieval-operations-receipt+json"
_HEX = re.compile(r"^[0-9a-f]{64}$")

@dataclass(frozen=True)
class RetrievalOperationsRequest:
    query: RetrievalQuery
    capacity_units: int
    requested_event_order: tuple[str, ...]
    completed_event_order: tuple[str, ...] = ()
    checkpoint_seq: int = 0
    policy_allow: bool = True
    federation_approved: bool = False
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class RetrievalOperationsReceipt:
    value: dict[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self, *, feature_id: str, contract_version: str) -> None:
        v, a = self.value, self.value.get("artifact", {})
        identity = (v.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version") == contract_version and v.get("feature_id") == feature_id and v.get("boundary") == PRECLINICAL_BOUNDARY and a.get("boundary") == PRECLINICAL_BOUNDARY and a.get("content_type") == CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and isinstance(v.get("request_id"), str) and v["request_id"].strip() and v.get("event_order") and v.get("effect_receipts") and isinstance(v.get("capacity_units"), int) and v["capacity_units"] > 0 and isinstance(v.get("used_units"), int) and 0 <= v["used_units"] <= v["capacity_units"] and all(_HEX.fullmatch(v.get(k, "")) for k in ("replay_identity", "synthesis_digest", "operations_digest")))
        if not identity: raise ResearchContractError("retrieval operations identity, locality, capacity, digests, or effects are incomplete")
        keys = ("event_order", "completed_event_order", "retryable_event_order", "dropped_event_order", "candidate_order", "selected_order", "unresolved_order", "blocked_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in keys): raise ResearchContractError("retrieval operations ordering is not canonical")
        events, states = set(v["event_order"]), v["completed_event_order"] + v["retryable_event_order"] + v["dropped_event_order"]
        if len(events) != len(v["event_order"]) or len(states) != len(events) or set(states) != events: raise ResearchContractError("operation states do not partition requested events")
        candidates, retrieval_states = set(v["candidate_order"]), v["selected_order"] + v["unresolved_order"] + v["blocked_order"]
        if len(candidates) != len(v["candidate_order"]) or len(retrieval_states) != len(candidates) or set(retrieval_states) != candidates: raise ResearchContractError("retrieval states do not partition candidates")
        if a.get("content_hash") != v.get("operations_digest"): raise ResearchContractError("retrieval operations artifact digest is inconsistent")
        if any(e != "block:unsafe-release" and not e.startswith("operate:retrieval-operations:") for e in v["effect_receipts"]): raise ResearchContractError("retrieval operations effect is outside operations gate")
    def digest(self, *, feature_id: str, contract_version: str) -> str: self.validate(feature_id=feature_id, contract_version=contract_version); return _digest(self.value)

def _digest(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _ordered(values: list[str] | tuple[str, ...]) -> bool: return list(values) == sorted(set(values))
def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["research program lead", "benchmark curator", "preclinical neuroscientist", "operations steward"], "behavior": f"operate bounded retrieval synthesis for {scale} with capacity, checkpoint, retry, and policy accounting", "value": "turns retrieval synthesis into a replayable institution-local operations product without hiding omissions or negative evidence", "input_schema": input_schema, "output_schema": "EvidenceSynthesis6@1", "effects": ["operate:retrieval-operations", "block:unsafe-release"], "permissions": ["operate:institution-node", "read:local-research-artifacts"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY}

def operate(request: RetrievalOperationsRequest, *, feature_id: str, contract_version: str, require_federation: bool) -> RetrievalOperationsReceipt:
    q = request.query
    if request.boundary != PRECLINICAL_BOUNDARY or q.boundary != PRECLINICAL_BOUNDARY or not q.raw_data_local or not q.aggregate_only or not isinstance(request.capacity_units, int) or request.capacity_units <= 0 or not request.requested_event_order or not _ordered(request.requested_event_order) or not _ordered(request.completed_event_order) or any(e not in request.requested_event_order for e in request.completed_event_order):
        raise ResearchContractError("retrieval operations boundary, capacity, event order, locality, or checkpoint is invalid")
    expected = sorted({c.candidate_id for c in q.candidates})
    if expected != list(request.requested_event_order): raise ResearchContractError("requested events must exactly cover canonical candidate ids")
    bounded = replace(q, max_budget_units=min(q.max_budget_units, request.capacity_units))
    synthesis = infer(bounded, feature_id=feature_id, contract_version=contract_version).value
    candidate_set, selected, unresolved, blocked = set(synthesis["candidate_order"]), set(synthesis["selected_order"]), set(synthesis["unresolved_order"]), set(synthesis["blocked_order"])
    completed, retryable, dropped = set(selected), set(unresolved), set(blocked)
    omissions, uncertainty, negative = list(synthesis["omission_order"]), list(synthesis["uncertainty_order"]), list(synthesis["negative_evidence_order"])
    if not request.policy_allow: omissions.append("request:policy-denied")
    if require_federation and not request.federation_approved: omissions.append("request:federation-approval-missing")
    safe_authority = request.policy_allow and q.protected_closure and (not require_federation or request.federation_approved)
    disposition = "blocked" if not safe_authority else "qualified" if len(completed) == len(candidate_set) and not omissions and not uncertainty and not negative else "partial"
    if disposition == "blocked": completed, retryable, dropped = set(), set(candidate_set), set()
    effect = ["block:unsafe-release"] if disposition == "blocked" else [f"operate:retrieval-operations:{q.request_id}"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": q.request_id, "disposition": disposition, "event_order": list(request.requested_event_order), "completed_event_order": sorted(completed), "retryable_event_order": sorted(retryable), "dropped_event_order": sorted(dropped), "candidate_order": synthesis["candidate_order"], "selected_order": synthesis["selected_order"], "unresolved_order": synthesis["unresolved_order"], "blocked_order": synthesis["blocked_order"], "capacity_units": request.capacity_units, "used_units": min(synthesis["total_units"], request.capacity_units), "checkpoint_seq": request.checkpoint_seq, "replay_identity": q.replay_identity, "synthesis_digest": synthesis["synthesis_digest"], "omissions": sorted(set(omissions)), "uncertainty": sorted(set(uncertainty)), "negative_evidence": sorted(set(negative)), "effect_receipts": effect, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    operations_digest = _digest(payload)
    payload["operations_digest"] = operations_digest
    payload["artifact"] = {"artifact_id": f"retrieval-operations:{q.request_id}", "content_type": CONTENT_TYPE, "content_hash": operations_digest, "boundary": PRECLINICAL_BOUNDARY}
    receipt = RetrievalOperationsReceipt(payload); receipt.validate(feature_id=feature_id, contract_version=contract_version); return receipt

__all__ = ["CONTENT_TYPE", "RetrievalOperationsRequest", "RetrievalOperationsReceipt", "manifest", "operate"]
