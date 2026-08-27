"""Federated continual context-compilation copilot (``AFA-oraclex-P03-F12``)."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
)

FEATURE_ID = "AFA-oraclex-P03-F12"
CONTRACT_VERSION = "oraclex-federated-context-compilation-copilot/1.0"
INPUT_SCHEMA = "DecisionQuery4@1"
OUTPUT_SCHEMA = "CertifiedDecisionSection3@1"
CONTENT_TYPE = "application/vnd.aurora.certified-decision-section-3+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class CertifiedDecisionSection:
    schema_version: str
    contract_version: str
    feature_id: str
    request_id: str
    consumer: str
    federation_id: str
    purpose: str
    semantic_profile: str
    target_schema: str
    disposition: str
    fact_order: tuple[str, ...]
    selected_fact_order: tuple[str, ...]
    unresolved_fact_order: tuple[str, ...]
    blocked_fact_order: tuple[str, ...]
    missing_fact_order: tuple[str, ...]
    peer_order: tuple[str, ...]
    qualified_peer_order: tuple[str, ...]
    missing_peer_order: tuple[str, ...]
    tool_plan_order: tuple[str, ...]
    omission_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_evidence_order: tuple[str, ...]
    replay_identity: str
    section_digest: str
    evidence_digest: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool
    boundary: str

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple):
                value[key] = list(item)
        return value

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY
            or self.raw_data_local is not True
            or not self.request_id.strip()
            or not self.consumer.strip()
            or not self.federation_id.strip()
            or not self.purpose.strip()
            or not self.semantic_profile.strip()
            or not self.target_schema.strip()
            or not self.fact_order
            or not self.peer_order
            or not self.effect_receipts
        ):
            raise ResearchContractError("decision section identity, locality, facts, peers, or effects are incomplete")
        for values in (
            self.fact_order,
            self.selected_fact_order,
            self.unresolved_fact_order,
            self.blocked_fact_order,
            self.missing_fact_order,
            self.peer_order,
            self.qualified_peer_order,
            self.missing_peer_order,
            self.tool_plan_order,
            self.omission_order,
            self.uncertainty_order,
            self.negative_evidence_order,
            self.effect_receipts,
        ):
            if not _ordered(list(values)):
                raise ResearchContractError("decision section ordering is not canonical")
        fact_ids = set(self.fact_order)
        parts = list(self.selected_fact_order) + list(self.unresolved_fact_order) + list(self.blocked_fact_order)
        if set(parts) != fact_ids or len(parts) != len(fact_ids):
            raise ResearchContractError("decision facts do not partition the supplied context")
        peer_ids = set(self.peer_order)
        peer_parts = list(self.qualified_peer_order) + list(self.missing_peer_order)
        if set(peer_parts) != peer_ids or len(peer_parts) != len(peer_ids):
            raise ResearchContractError("decision peers do not partition the supplied federation")
        if not all(_digest(value) for value in (self.replay_identity, self.section_digest, self.evidence_digest, self.artifact.get("content_hash"))):
            raise ResearchContractError("decision section digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("decision section artifact metadata is invalid")
        expected = [f"invoke:declared-tools:{self.request_id}"] if self.disposition == "qualified" else ["block:unsafe-release"]
        if list(self.effect_receipts) != expected:
            raise ResearchContractError("decision section effect receipt is invalid")


def compile_context(*, request: Mapping[str, Any]) -> CertifiedDecisionSection:
    if (
        request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or any(not str(request.get(key, "")).strip() for key in ("request_id", "consumer", "federation_id", "purpose", "semantic_profile", "target_schema"))
        or not request.get("required_fact_order")
        or not _ordered([str(value) for value in request["required_fact_order"]])
        or not request.get("facts")
        or not request.get("peers")
        or any(int(request.get(key, 0)) <= 0 for key in ("max_facts", "max_tools", "tool_budget"))
        or len(request["facts"]) > int(request["max_facts"])
        or not _digest(request.get("replay_identity"))
        or not _ordered([str(value) for value in request.get("adversarial_events", [])])
        or request.get("raw_data_local") is not True
        or request.get("boundary") != PRECLINICAL_BOUNDARY
    ):
        raise ResearchContractError("decision query identity, closure, limits, replay, locality, or boundary is invalid")
    facts = sorted(request["facts"], key=lambda item: str(item.get("fact_id", "")))
    fact_order = [str(fact.get("fact_id", "")) for fact in facts]
    if not all(fact_order) or len(set(fact_order)) != len(fact_order):
        raise ResearchContractError("fact identities must be unique and non-empty")
    required = {str(value) for value in request["required_fact_order"]}
    present = set(fact_order)
    missing = sorted(required - present)
    selected: set[str] = set()
    unresolved: set[str] = set()
    blocked: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    for fact in facts:
        fact_id = str(fact["fact_id"])
        if fact.get("negative_result") is True:
            negative.add(f"{fact_id}:negative-result")
        if fact_id not in required:
            omissions.add(f"{fact_id}:not-required")
        state = str(fact.get("state", "unknown"))
        if state == "contradicted":
            blocked.add(fact_id)
        elif state in {"unknown", "speculative"}:
            unresolved.add(fact_id)
        elif state in {"proven", "supported"}:
            if fact.get("local_only") is not True or fact.get("permitted") is not True:
                blocked.add(fact_id)
            elif fact_id in required:
                selected.add(fact_id)
            else:
                unresolved.add(fact_id)
        else:
            unresolved.add(fact_id)
        if str(fact.get("semantic_profile", "")) != str(request["semantic_profile"]):
            uncertainty.add(f"{fact_id}:semantic-profile-mismatch")
            unresolved.add(fact_id)
            selected.discard(fact_id)
    peer_order: set[str] = set()
    qualified_peers: set[str] = set()
    missing_peers: set[str] = set()
    for peer in request["peers"]:
        peer_id = str(peer.get("peer_id", ""))
        peer_order.add(peer_id)
        valid = (
            str(peer.get("purpose", "")) == str(request["purpose"])
            and str(peer.get("semantic_profile", "")) == str(request["semantic_profile"])
            and peer.get("signed") is True
            and peer.get("aggregate_only") is True
            and peer.get("raw_data_local") is True
            and _digest(peer.get("context_digest"))
            and _ordered([str(value) for value in peer.get("fact_order", [])])
            and str(peer.get("state", "unknown")) in {"proven", "supported"}
        )
        if valid:
            qualified_peers.add(peer_id)
        else:
            missing_peers.add(peer_id)
            uncertainty.add(f"peer:{peer_id}:not-qualified")
    tool_plan = sorted(f"tool:{fact_id}" for fact_id in selected)[: int(request["max_tools"])]
    if len(selected) > int(request["max_tools"]) or len(tool_plan) > int(request["tool_budget"]):
        omissions.add("request:tool-budget-exhausted")
    if request.get("policy_allow") is not True:
        negative.add("request:policy-denied")
    if request.get("protected_closure") is not True:
        uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True:
        uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approval") is not True:
        uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{value}" for value in request.get("adversarial_events", []))
    global_block = (
        request.get("policy_allow") is not True
        or request.get("raw_data_local") is not True
        or request.get("signed_approval") is not True
        or request.get("federation_approval") is not True
        or request.get("protected_closure") is not True
        or bool(request.get("adversarial_events"))
        or len(selected) > int(request["max_tools"])
        or len(tool_plan) > int(request["tool_budget"])
    )
    if global_block:
        blocked.update(fact_order)
        selected.clear()
        unresolved.clear()
        omissions.add("request:context-gate-blocked")
    disposition = "blocked" if global_block or blocked else "unresolved" if missing or unresolved or missing_peers else "qualified"
    selected_order, unresolved_order, blocked_order = sorted(selected), sorted(unresolved), sorted(blocked)
    peer_order_list, qualified_peer_order, missing_peer_order = sorted(peer_order), sorted(qualified_peers), sorted(missing_peers)
    omission_order, uncertainty_order, negative_order = sorted(omissions), sorted(uncertainty), sorted(negative)
    effects = [f"invoke:declared-tools:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    evidence_payload = {
        "fact_order": fact_order,
        "selected_fact_order": selected_order,
        "unresolved_fact_order": unresolved_order,
        "blocked_fact_order": blocked_order,
        "missing_fact_order": missing,
        "peer_order": peer_order_list,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_order,
    }
    evidence_digest = _hash(evidence_payload)
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": str(request["request_id"]),
        "consumer": str(request["consumer"]),
        "federation_id": str(request["federation_id"]),
        "purpose": str(request["purpose"]),
        "semantic_profile": str(request["semantic_profile"]),
        "target_schema": str(request["target_schema"]),
        "disposition": disposition,
        "evidence": evidence_payload,
        "tool_plan_order": tool_plan,
        "replay_identity": str(request["replay_identity"]),
        "evidence_digest": evidence_digest,
        "effect_receipts": effects,
        "raw_data_local": True,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    artifact = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "artifact_id": f"certified-decision-section:{request['request_id']}",
        "content_type": CONTENT_TYPE,
        "content_hash": digest,
        "semantic_loss": [],
        "provenance": [],
        "boundary": PRECLINICAL_BOUNDARY,
    }
    receipt = CertifiedDecisionSection(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        str(request["request_id"]),
        str(request["consumer"]),
        str(request["federation_id"]),
        str(request["purpose"]),
        str(request["semantic_profile"]),
        str(request["target_schema"]),
        disposition,
        tuple(fact_order),
        tuple(selected_order),
        tuple(unresolved_order),
        tuple(blocked_order),
        tuple(missing),
        tuple(peer_order_list),
        tuple(qualified_peer_order),
        tuple(missing_peer_order),
        tuple(tool_plan),
        tuple(omission_order),
        tuple(uncertainty_order),
        tuple(negative_order),
        str(request["replay_identity"]),
        digest,
        evidence_digest,
        tuple(effects),
        artifact,
        True,
        PRECLINICAL_BOUNDARY,
    )
    receipt.validate()
    return receipt


__all__ = [
    "FEATURE_ID",
    "CONTRACT_VERSION",
    "INPUT_SCHEMA",
    "OUTPUT_SCHEMA",
    "CertifiedDecisionSection",
    "compile_context",
]
