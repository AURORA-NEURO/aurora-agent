"""Python parity for ``AFA-api-P03-F27`` context-compilation assurance."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
)

FEATURE_ID = "AFA-api-P03-F27"
CONTRACT_VERSION = "api-prospective-high-throughput-context-compilation-assurance/1.0"
INPUT_SCHEMA = "ContextCompilationRequest6@1"
OUTPUT_SCHEMA = "ContextAssuranceReceipt7@1"
CONTENT_TYPE = "application/vnd.aurora.context-assurance-receipt-7+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ContextAssuranceReceipt7:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        artifact = v.get("artifact", {})
        if (
            v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or v.get("contract_version") != CONTRACT_VERSION
            or v.get("feature_id") != FEATURE_ID
            or v.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or v.get("raw_data_local") is not True
            or v.get("aggregate_only") is not True
            or int(v.get("checkpoint", 0)) <= 0
            or not all(
                str(v.get(k, "")).strip()
                for k in (
                    "request_id",
                    "federation_id",
                    "query_id",
                    "requester",
                    "purpose",
                    "scope",
                    "semantic_profile",
                )
            )
            or v.get("disposition") not in {"qualified", "unresolved", "blocked"}
            or not v.get("fact_order")
            or not v.get("peer_order")
            or not v.get("effect_receipts")
        ):
            raise ResearchContractError("context identity, checkpoint, locality, facts, peers, or effects are incomplete")
        fields = (
            "fact_order", "selected_fact_order", "omitted_fact_order", "unresolved_fact_order",
            "blocked_fact_order", "missing_fact_order", "peer_order", "qualified_peer_order",
            "missing_peer_order", "omission_order", "uncertainty_order", "negative_evidence_order",
            "effect_receipts",
        )
        if any(not _ordered(v.get(key, [])) for key in fields):
            raise ResearchContractError("context ordering is not canonical")
        universe = set(v["fact_order"])
        parts = (
            set(v["selected_fact_order"])
            | set(v["omitted_fact_order"])
            | set(v["unresolved_fact_order"])
            | set(v["blocked_fact_order"])
            | set(v["missing_fact_order"])
        )
        if len(universe) != len(v["fact_order"]) or universe != parts:
            raise ResearchContractError("context facts do not partition")
        peers = set(v["peer_order"])
        peer_parts = set(v["qualified_peer_order"]) | set(v["missing_peer_order"])
        if len(peers) != len(v["peer_order"]) or peers != peer_parts:
            raise ResearchContractError("context peers do not partition")
        if (
            artifact.get("content_type") != CONTENT_TYPE
            or artifact.get("content_hash") != v.get("context_digest")
            or not all(
                _digest(item)
                for item in (
                    v.get("replay_identity"),
                    v.get("context_digest"),
                    artifact.get("content_hash"),
                    *artifact.get("provenance_digests", []),
                )
            )
        ):
            raise ResearchContractError("context artifact digest is invalid")


def context_compilation_assurance_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "api",
        "consumers": ["context researcher", "API client", "federation steward"],
        "behavior": "compiles scoped typed facts into a deterministic context readiness receipt with omission certificate",
        "value": "prevents incomplete or unsupported context from being presented as decision-sufficient",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["retain:context-assurance", "exchange:aggregate-context-summary"],
        "permissions": ["retain:context-receipts", "exchange:aggregate-context"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def assure_context_compilation(q: Mapping[str, Any]) -> ContextAssuranceReceipt7:
    required_keys = ("request_id", "federation_id", "query_id", "requester", "purpose", "scope", "semantic_profile")
    if (
        not all(str(q.get(key, "")).strip() for key in required_keys)
        or not q.get("required_fact_order")
        or not q.get("facts")
        or not q.get("peers")
        or int(q.get("checkpoint", 0)) <= 0
        or int(q.get("minimum_peer_quorum", 0)) <= 0
        or q.get("boundary") != PRECLINICAL_BOUNDARY
        or q.get("raw_data_local") is not True
        or q.get("aggregate_only") is not True
        or not _digest(q.get("replay_identity"))
    ):
        raise ResearchContractError("context identity, bounds, facts, peers, replay, locality, or boundary is invalid")
    facts = sorted((dict(item) for item in q["facts"]), key=lambda item: (-int(item.get("influence_milli", 0)), str(item.get("fact_id", ""))))
    fact_ids = [str(item.get("fact_id", "")) for item in facts]
    if (
        len(set(fact_ids)) != len(fact_ids)
        or any(
            not item.get("fact_id")
            or not str(item.get("scope", "")).strip()
            or not str(item.get("semantic_profile", "")).strip()
            or not _digest(item.get("source_digest"))
            or not _digest(item.get("provenance_digest"))
            or not _digest(item.get("replay_identity"))
            or item.get("replay_identity") != q["replay_identity"]
            for item in facts
        )
    ):
        raise ResearchContractError("fact identity, scope, profile, digests, or replay is invalid")
    required = set(str(item) for item in q["required_fact_order"])
    fact_order = sorted(set(fact_ids) | required)
    selected: set[str] = set()
    omitted: set[str] = set()
    unresolved: set[str] = set()
    blocked: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    for item in facts:
        fact_id = item["fact_id"]
        if item.get("negative_result"):
            negative.add(f"{fact_id}:negative-result")
        state = item.get("evidence_state")
        if state == "contradicted":
            blocked.add(fact_id)
            negative.add(f"{fact_id}:contradicted")
        elif state in {"unknown", "speculative"}:
            unresolved.add(fact_id)
            uncertainty.add(f"{fact_id}:evidence-state")
        elif state in {"proven", "supported"} and item.get("scope") == q["scope"] and item.get("semantic_profile") == q["semantic_profile"] and item.get("local_data") is True and item.get("policy_allowed") is True:
            selected.add(fact_id)
        elif state in {"proven", "supported"}:
            omitted.add(fact_id)
            uncertainty.add(f"{fact_id}:scope-profile-locality")
        else:
            unresolved.add(fact_id)
            uncertainty.add(f"{fact_id}:evidence-state")
    missing = required - set(fact_ids)
    omissions = {f"fact:{item}:missing" for item in missing} | {f"fact:{item}:omitted" for item in omitted}
    peers = sorted((dict(item) for item in q["peers"]), key=lambda item: str(item.get("peer_id", "")))
    peer_order = [str(item.get("peer_id", "")) for item in peers]
    qualified_peers = {
        item["peer_id"] for item in peers
        if item.get("semantic_profile") == q["semantic_profile"]
        and int(item.get("checkpoint", 0)) == int(q["checkpoint"])
        and item.get("signed") is True and item.get("aggregate_only") is True
        and item.get("raw_data_local") is True
        and item.get("evidence_state") in {"proven", "supported"}
    }
    missing_peers = set(peer_order) - qualified_peers
    uncertainty |= {f"peer:{item}:not-qualified" for item in missing_peers}
    global_block = not all(q.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if q.get("policy_allow") is not True:
        negative.add("request:policy-denied")
    if q.get("protected_closure") is not True:
        uncertainty.add("request:protected-closure-incomplete")
    if q.get("signed_approval") is not True:
        uncertainty.add("request:signed-approval-missing")
    if q.get("federation_approved") is not True:
        uncertainty.add("request:federation-approval-missing")
    disposition = "blocked" if global_block or blocked else "unresolved" if not selected or missing or unresolved or len(qualified_peers) < int(q["minimum_peer_quorum"]) else "qualified"
    if disposition != "qualified":
        omissions.add("request:context-not-release-ready")
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": q["request_id"], "federation_id": q["federation_id"], "query_id": q["query_id"], "requester": q["requester"],
        "purpose": q["purpose"], "scope": q["scope"], "semantic_profile": q["semantic_profile"], "checkpoint": int(q["checkpoint"]),
        "disposition": disposition, "fact_order": fact_order, "selected_fact_order": sorted(selected), "omitted_fact_order": sorted(omitted),
        "unresolved_fact_order": sorted(unresolved), "blocked_fact_order": sorted(blocked), "missing_fact_order": sorted(missing),
        "peer_order": peer_order, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers),
        "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative),
        "replay_identity": q["replay_identity"], "boundary": PRECLINICAL_BOUNDARY,
    }
    digest = _hash(payload)
    result = {
        **payload,
        "context_digest": digest,
        "artifact": {
            "artifact_id": f"context-assurance-receipt-7:{q['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest,
            "semantic_loss": [], "provenance_digests": sorted({item["provenance_digest"] for item in facts}), "boundary": PRECLINICAL_BOUNDARY,
        },
        "effect_receipts": [f"retain:context-assurance:{q['request_id']}", f"exchange:aggregate-context-summary:{q['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"],
        "raw_data_local": True, "aggregate_only": True,
    }
    receipt = ContextAssuranceReceipt7(result)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "ContextAssuranceReceipt7", "context_compilation_assurance_manifest", "assure_context_compilation"]
