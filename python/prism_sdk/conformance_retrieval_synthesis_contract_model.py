"""Parity surface for ``AFA-conformance-P02-F07``.

This module negotiates a bounded retrieval/synthesis contract.  It never retrieves documents,
ships raw experimental data, or turns incomplete evidence into a positive release decision.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-conformance-P02-F07"
CONTRACT_VERSION = "conformance-prospective-high-throughput-retrieval-synthesis-contract-model/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery3@1"
OUTPUT_SCHEMA = "EvidenceSynthesis2@1"
CONTENT_TYPE = "application/vnd.aurora.conformance-evidence-synthesis-2+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: Sequence[str]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class EvidenceSynthesis2:
    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        required = ("request_id", "consumer", "scope", "semantic_profile")
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
                or v.get("contract_version") != CONTRACT_VERSION
                or v.get("feature_id") != FEATURE_ID
                or v.get("boundary") != PRECLINICAL_BOUNDARY
                or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True
                or any(not str(v.get(k, "")).strip() for k in required)
                or not v.get("candidate_order") or not v.get("effect_receipts")
                or v.get("disposition") not in {"compatible", "partial", "unknown", "blocked"}):
            raise ResearchContractError("retrieval identity, locality, closure, or effects are incomplete")
        keys = ("candidate_order", "compatible_order", "unresolved_order", "blocked_order", "omitted_order",
                "negative_evidence_order", "migration_order", "semantic_loss_order", "effect_receipts")
        if any(not _ordered(v.get(k, ())) for k in keys):
            raise ResearchContractError("retrieval contract ordering is not canonical")
        all_ids = list(v["candidate_order"])
        parts = list(v.get("compatible_order", ())) + list(v.get("unresolved_order", ())) + list(v.get("blocked_order", ())) + list(v.get("omitted_order", ()))
        candidate_parts = [x for x in parts if x in set(all_ids)]
        if len(all_ids) != len(set(all_ids)) or set(candidate_parts) != set(all_ids) or len(candidate_parts) != len(set(candidate_parts)):
            raise ResearchContractError("retrieval candidate states do not partition")
        artifact = v.get("artifact", {})
        if (not _digest(v.get("replay_identity")) or not _digest(v.get("contract_digest"))
                or artifact.get("content_hash") != v.get("contract_digest")
                or artifact.get("content_type") != CONTENT_TYPE
                or artifact.get("boundary") != PRECLINICAL_BOUNDARY
                or any(not _digest(x) for x in artifact.get("provenance_digests", ()) )):
            raise ResearchContractError("retrieval artifact metadata or digest is invalid")
        effects = list(v["effect_receipts"])
        if any(x != "block:unsafe-release" and not x.startswith("read:local-research-artifacts:") for x in effects):
            raise ResearchContractError("retrieval effect is outside local-read gate")
        if v["disposition"] == "blocked" and effects != ["block:unsafe-release"]:
            raise ResearchContractError("blocked retrieval contract must block")

    def digest(self) -> str:
        self.validate()
        return _hash(self.value)


def negotiate_retrieval_synthesis_contract(*, request: Mapping[str, Any]) -> EvidenceSynthesis2:
    if (request.get("schema_version") != INPUT_SCHEMA
            or any(not str(request.get(k, "")).strip() for k in ("request_id", "consumer", "scope", "semantic_profile", "input_schema", "output_schema"))
            or not _digest(request.get("replay_identity"))
            or int(request.get("minimum_relevance_milli", 0)) <= 0
            or int(request.get("minimum_freshness_milli", 0)) <= 0
            or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True
            or request.get("boundary") != PRECLINICAL_BOUNDARY or not request.get("candidates")):
        raise ResearchContractError("retrieval identity, bounds, replay, locality, or boundary is invalid")
    compatibility = "compatible" if request["input_schema"] == INPUT_SCHEMA and request["output_schema"] == OUTPUT_SCHEMA else (
        "additive_migration" if str(request["input_schema"]).startswith("ScopedRetrievalQuery") and str(request["output_schema"]).startswith("EvidenceSynthesis") else "breaking")
    rows = sorted(request["candidates"], key=lambda r: (-int(r.get("relevance_milli", 0)), str(r.get("candidate_id", ""))))
    candidate_order = sorted(str(r.get("candidate_id", "")) for r in rows)
    if len(candidate_order) != len(set(candidate_order)):
        raise ResearchContractError("duplicate retrieval candidates are invalid")
    compatible, unresolved, blocked, omitted, negative, migration, loss, provenance = set(), set(), set(), set(), set(), set(), set(), set()
    for r in rows:
        cid = str(r.get("candidate_id", ""))
        provenance.add(str(r.get("provenance_digest", "")))
        omitted.update(f"{cid}:{x}" for x in r.get("omission_reasons", ()))
        if r.get("negative_result") is True:
            negative.add(f"{cid}:negative-result")
        hard = (request.get("policy_allow") is not True or request.get("protected_closure") is not True
                or r.get("permitted") is not True or r.get("local_only") is not True
                or str(r.get("semantic_profile")) != str(request["semantic_profile"])
                or r.get("comparable") is not True or not _digest(r.get("content_digest")) or not _digest(r.get("provenance_digest")))
        soft = (str(r.get("replay_identity")) != str(request["replay_identity"])
                or int(r.get("relevance_milli", 0)) < int(request["minimum_relevance_milli"])
                or int(r.get("freshness_milli", 0)) < int(request["minimum_freshness_milli"]))
        if compatibility != "compatible":
            migration.add(f"{cid}:schema-migration")
        state = str(r.get("evidence_state", "")).lower()
        if hard or state == "contradicted":
            blocked.add(cid)
        elif soft or state not in {"proven", "supported"}:
            unresolved.add(cid)
        else:
            compatible.add(cid)
    if request.get("policy_allow") is not True:
        loss.add("workflow:policy-denied")
    if request.get("protected_closure") is not True:
        loss.add("workflow:protected-closure-incomplete")
    disposition = "blocked" if request.get("policy_allow") is not True or request.get("protected_closure") is not True or blocked else (
        "partial" if compatibility != "compatible" or unresolved or negative else "compatible")
    if disposition != "compatible":
        loss.add("workflow:contract-not-fully-compatible")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": str(request["request_id"]), "consumer": str(request["consumer"]), "scope": str(request["scope"]), "semantic_profile": str(request["semantic_profile"]),
        "input_schema": str(request["input_schema"]), "output_schema": str(request["output_schema"]), "compatibility": compatibility, "disposition": disposition,
        "candidate_order": candidate_order, "compatible_order": sorted(compatible), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked),
        "omitted_order": sorted(omitted), "negative_evidence_order": sorted(negative), "migration_order": sorted(migration), "semantic_loss_order": sorted(loss),
        "replay_identity": str(request["replay_identity"]), "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    contract_digest = _hash(payload)
    payload["contract_digest"] = contract_digest
    payload["artifact"] = {"artifact_id": f"conformance-evidence-synthesis-2:{request['request_id']}", "content_type": CONTENT_TYPE,
                            "content_hash": contract_digest, "semantic_loss": sorted(loss), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    payload["effect_receipts"] = [f"read:local-research-artifacts:{request['request_id']}"] if disposition == "compatible" else ["block:unsafe-release"]
    receipt = EvidenceSynthesis2(payload)
    receipt.validate()
    return receipt


def conformance_retrieval_synthesis_contract_model_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION,
            "owner_crate": "conformance", "consumers": ["institutional safety reviewer", "retrieval schema steward", "evidence synthesis engineer"],
            "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["read:local-research-artifacts", "write:local-artifact"],
            "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "EvidenceSynthesis2",
           "negotiate_retrieval_synthesis_contract", "conformance_retrieval_synthesis_contract_model_manifest"]
