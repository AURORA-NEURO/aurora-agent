"""Python parity adapter for ``AFA-runtime-P03-F07``.

This is a deterministic, local-only compiler from ``DecisionQuery3`` bindings to an omission-
aware ``CertifiedDecisionSection2``.  It never fills gaps with model output or silently exports
protected research data.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-runtime-P03-F07"
CONTRACT_VERSION = "runtime-prospective-context-compilation-contract-model/1.0"
INPUT_SCHEMA = "DecisionQuery3@1"
OUTPUT_SCHEMA = "CertifiedDecisionSection2@1"
ARTIFACT_TYPE = "application/vnd.aurora.certified-decision-section+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class ContextContractReceipt:
    schema_version: str
    contract_version: str
    feature_id: str
    query_id: str
    compatibility: str
    migration_order: tuple[str, ...]
    section: Mapping[str, Any]
    checks: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    raw_data_local: bool
    boundary: str

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.compatibility != "compatible"
            or not self.query_id.strip()
            or not self.checks
            or not self.effect_receipts
            or self.raw_data_local is not True
            or self.boundary != PRECLINICAL_BOUNDARY
            or not _canonical(list(self.migration_order))
            or not _canonical(list(self.checks))
            or any(effect not in {"retain:context-contract", "block:unsafe-release"} for effect in self.effect_receipts)
        ):
            raise ResearchContractError("context contract identity, compatibility, locality, checks, or effects are incomplete")
        if self.section.get("schema_version") != OUTPUT_SCHEMA or self.section.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("certified section schema or boundary is invalid")
        for name in ("selected_fact_order", "omitted_fact_order", "unresolved_fact_order", "required_fact_order", "omission_certificate", "uncertainty", "negative_evidence"):
            if not _canonical(list(self.section.get(name, []))):
                raise ResearchContractError("certified section ordering is not canonical")
        required = set(self.section["required_fact_order"])
        partition = list(self.section["selected_fact_order"]) + list(self.section["omitted_fact_order"]) + list(self.section["unresolved_fact_order"])
        if set(partition) != required or len(partition) != len(set(partition)):
            raise ResearchContractError("certified section facts do not partition the query")
        artifact = self.section.get("artifact", {})
        if artifact.get("content_type") != ARTIFACT_TYPE or not _digest(artifact.get("content_hash")):
            raise ResearchContractError("certified section artifact type or digest is invalid")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple):
                value[key] = list(item)
        return value


def compile_context_contract(*, request: Mapping[str, Any]) -> ContextContractReceipt:
    required = ("query_id", "scope", "target")
    if (
        any(not str(request.get(field, "")).strip() for field in required)
        or request.get("schema_version") != INPUT_SCHEMA
        or not request.get("facts")
        or not request.get("required_fact_order")
        or int(request.get("budget_units", 0)) <= 0
        or int(request.get("max_budget_units", 0)) <= 0
        or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0))
        or request.get("raw_data_local") is not True
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or not _digest(request.get("replay_identity"))
        or not _digest(request.get("semantic_digest"))
    ):
        raise ResearchContractError("context query identity, bounds, locality, replay, or boundary is invalid")
    required_order = [str(item) for item in request["required_fact_order"]]
    if not _canonical(required_order) or any(not item.strip() for item in required_order):
        raise ResearchContractError("required facts are not canonical")
    facts = sorted(request["facts"], key=lambda item: str(item.get("fact_id", "")))
    ids = [str(item.get("fact_id", "")) for item in facts]
    if not all(ids) or len(set(ids)) != len(ids) or any(not _digest(item.get("source_digest")) or not _digest(item.get("provenance_digest")) for item in facts):
        raise ResearchContractError("fact identifiers, source digests, or provenance digests are invalid")
    if not set(required_order).issubset(ids):
        raise ResearchContractError("required fact is absent from bindings")
    by_id = {str(item["fact_id"]): item for item in facts}
    ranked = sorted(required_order, key=lambda item: (-int(by_id[item].get("influence_milli", 0)), item))
    selected: list[str] = []; omitted: set[str] = set(); unresolved: set[str] = set(); contradicted: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); spent = 0
    for fact_id in ranked:
        fact = by_id[fact_id]; state = str(fact.get("evidence_state", "unknown"))
        if state == "contradicted":
            omitted.add(fact_id); contradicted.add(fact_id); negative.add(f"{fact_id}:contradicted"); continue
        if state in {"unknown", "speculative"}:
            unresolved.add(fact_id); unknown.add(fact_id); uncertainty.add(f"{fact_id}:evidence-state"); continue
        blocked = False
        if request.get("policy_allow") is not True:
            unresolved.add(fact_id); omissions.add(f"{fact_id}:policy-denied"); blocked = True
        if request.get("protected_closure") is not True:
            unresolved.add(fact_id); omissions.add(f"{fact_id}:protected-closure-incomplete"); blocked = True
        for item in fact.get("omissions", []):
            unresolved.add(fact_id); omissions.add(f"{fact_id}:{item}"); blocked = True
        for item in fact.get("uncertainty", []):
            unresolved.add(fact_id); uncertainty.add(f"{fact_id}:{item}"); blocked = True
        if blocked:
            continue
        cost = len(fact_id) + 1
        if cost > int(request["budget_units"]) - spent:
            unresolved.add(fact_id); omissions.add(f"{fact_id}:budget-ceiling"); continue
        spent += cost; selected.append(fact_id); negative.add(f"{fact_id}:negative-result-not-observed")
    for fact_id in required_order:
        if fact_id not in selected and fact_id not in omitted and fact_id not in unresolved:
            omitted.add(fact_id); omissions.add(f"{fact_id}:not-admitted")
    selected = sorted(selected); omitted_order = sorted(omitted); unresolved_order = sorted(unresolved)
    checks = tuple(sorted(("artifact-closure", "budget-bound", "canonical-order", "evidence-state", "policy-boundary", "protected-closure", "provenance-closure", "replay-identity", "schema-compatibility")))
    disposition = "blocked" if request.get("policy_allow") is not True or request.get("protected_closure") is not True else "unresolved" if omitted_order or unresolved_order else "qualified"
    payload = {"schema_version": OUTPUT_SCHEMA, "section_id": f"section:{request['query_id']}", "query_id": request["query_id"], "scope": request["scope"], "target": request["target"], "selected_fact_order": selected, "omitted_fact_order": omitted_order, "unresolved_fact_order": unresolved_order, "required_fact_order": required_order, "semantic_digest": request["semantic_digest"], "replay_identity": request["replay_identity"], "disposition": disposition}
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"certified-decision-section:{request['query_id']}", "content_type": ARTIFACT_TYPE, "content_hash": _hash(payload), "semantic_loss": [], "provenance": [{"source_id": request["query_id"], "relation": "context-contract-compilation", "digest": _hash(payload)}], "boundary": PRECLINICAL_BOUNDARY}
    section = {"schema_version": OUTPUT_SCHEMA, "section_id": f"section:{request['query_id']}", "query_id": request["query_id"], "scope": request["scope"], "target": request["target"], "selected_fact_order": selected, "omitted_fact_order": omitted_order, "unresolved_fact_order": unresolved_order, "contradicted_fact_order": sorted(contradicted), "unknown_fact_order": sorted(unknown), "required_fact_order": required_order, "omission_certificate": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative), "semantic_digest": request["semantic_digest"], "replay_identity": request["replay_identity"], "section_digest": _hash(payload), "semantic_loss": [], "artifact": artifact, "boundary": PRECLINICAL_BOUNDARY}
    receipt = ContextContractReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["query_id"]), "compatible", tuple(), section, checks, ("retain:context-contract",) if disposition == "qualified" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY)
    receipt.validate()
    return receipt


__all__ = ["ContextContractReceipt", "compile_context_contract", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA"]
