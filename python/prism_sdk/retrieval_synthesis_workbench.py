"""Python parity adapter for ``AFA-choreography-P02-F19``."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-choreography-P02-F19"
CONTRACT_VERSION = "choreography-prospective-retrieval-synthesis-workbench/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery3@1"
OUTPUT_SCHEMA = "EvidenceSynthesis5@1"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))

@dataclass(frozen=True)
class EvidenceSynthesis:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; batch_id: str; scope: str; query: str; disposition: str; candidate_order: tuple[str, ...]; rank_order: tuple[str, ...]; visible_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]; stale_order: tuple[str, ...]; contradiction_order: tuple[str, ...]; required_source_order: tuple[str, ...]; observed_source_order: tuple[str, ...]; missing_source_order: tuple[str, ...]; views: tuple[str, ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; replay_identity: str; synthesis_digest: str; semantic_loss: tuple[Mapping[str, Any], ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or not all(str(value).strip() for value in (self.request_id, self.batch_id, self.scope, self.query)) or not self.candidate_order or len(self.rank_order) != len(self.candidate_order) or not self.views or not self.effect_receipts or self.raw_data_local is not True or self.boundary != PRECLINICAL_BOUNDARY: raise ResearchContractError("workbench identity, candidates, views, locality, or effects are incomplete")
        for values in (self.candidate_order, self.visible_order, self.unresolved_order, self.blocked_order, self.stale_order, self.contradiction_order, self.required_source_order, self.observed_source_order, self.missing_source_order, self.views, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("workbench ordering is not canonical")
        if set(self.rank_order) != set(self.candidate_order): raise ResearchContractError("rank order is not a candidate permutation")
        covered = list(self.visible_order) + list(self.unresolved_order) + list(self.blocked_order)
        if set(covered) != set(self.candidate_order) or len(covered) != len(set(covered)): raise ResearchContractError("workbench dispositions do not partition candidates")
        if any(effect not in {"view:authorized-research-state", "block:unsafe-release"} for effect in self.effect_receipts): raise ResearchContractError("workbench effect is outside read-only view boundary")
        if self.artifact.get("content_type") != "application/vnd.aurora.evidence-synthesis+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("workbench artifact type or digest is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value

def render_retrieval_workbench(*, request: Mapping[str, Any]) -> EvidenceSynthesis:
    if request.get("schema_version") != INPUT_SCHEMA or not all(str(request.get(field, "")).strip() for field in ("request_id", "batch_id", "scope", "query")) or not request.get("candidates") or not request.get("required_source_order") or int(request.get("min_independent_sources", 0)) <= 0 or int(request.get("max_visible", 0)) <= 0 or int(request.get("max_freshness_days", 0)) <= 0 or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")): raise ResearchContractError("workbench query identity, bounds, locality, replay, or boundary is invalid")
    required = [str(value) for value in request["required_source_order"]]
    if not _canonical(required): raise ResearchContractError("required source order is not canonical")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("candidate_id", ""))); ids = [str(item.get("candidate_id", "")) for item in candidates]
    if not all(ids) or len(set(ids)) != len(ids) or any(not str(item.get("source_id", "")).strip() or not str(item.get("title", "")).strip() or not _digest(item.get("content_digest")) or not _digest(item.get("provenance_digest")) for item in candidates): raise ResearchContractError("candidate identity, source, title, content, or provenance is invalid")
    scores: dict[str, int] = {}; stale: set[str] = set(); contradictions: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for candidate in candidates:
        cid = str(candidate["candidate_id"]); state = str(candidate.get("evidence_state", "unknown")); fresh = int(candidate.get("freshness_days", 0));
        if fresh > int(request["max_freshness_days"]): stale.add(cid); omissions.add(f"{cid}:stale")
        if state == "contradicted": contradictions.add(cid)
        if state in {"unknown", "speculative"}: uncertainty.add(f"{cid}:evidence-state")
        omissions.update(f"{cid}:{item}" for item in candidate.get("omissions", [])); uncertainty.update(f"{cid}:{item}" for item in candidate.get("uncertainty", [])); negative.add(f"{cid}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}")
        scores[cid] = int(candidate.get("relevance_milli", 0)) + (20000 if state == "proven" else 10000 if state == "supported" else 0) - fresh * 10
    rank = sorted(ids, key=lambda cid: (-scores[cid], cid)); required_set = set(required); observed = {str(item["source_id"]) for item in candidates}; missing_sources = sorted(required_set - observed); omissions.update(f"missing-source:{source}" for source in missing_sources); global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True
    if request.get("policy_allow") is not True: omissions.add("query:policy-denied")
    if request.get("protected_closure") is not True: omissions.add("query:protected-closure-incomplete")
    by_id = {str(item["candidate_id"]): item for item in candidates}; visible: list[str] = []; unresolved: list[str] = []; blocked: list[str] = []; spent = 0
    for cid in rank:
        item = by_id[cid]; state = str(item.get("evidence_state", "unknown")); hard = global_block or state == "contradicted"; conditional = int(item.get("freshness_days", 0)) > int(request["max_freshness_days"]) or state in {"unknown", "speculative"} or bool(item.get("omissions")) or bool(item.get("uncertainty"))
        if hard: blocked.append(cid)
        elif conditional or len(visible) >= int(request["max_visible"]): unresolved.append(cid); omissions.add(f"{cid}:view-capacity") if len(visible) >= int(request["max_visible"]) else None
        elif len(str(item["title"])) + 1 > int(request["budget_units"]) - spent: unresolved.append(cid); omissions.add(f"{cid}:budget-ceiling")
        else: visible.append(cid); spent += len(str(item["title"])) + 1
    visible.sort(); unresolved.sort(); blocked.sort(); quorum = len({str(by_id[cid]["source_id"]) for cid in visible});
    if quorum < int(request["min_independent_sources"]): omissions.add(f"source-quorum:{quorum}/{request['min_independent_sources']}")
    disposition = "blocked" if global_block else "unresolved" if blocked or unresolved or quorum < int(request["min_independent_sources"]) else "qualified"; payload = {"schema_version": OUTPUT_SCHEMA, "request_id": request["request_id"], "batch_id": request["batch_id"], "candidate_order": ids, "rank_order": rank, "visible_order": visible, "unresolved_order": unresolved, "blocked_order": blocked, "replay_identity": request["replay_identity"], "disposition": disposition}; digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"evidence-synthesis:{request['request_id']}", "content_type": "application/vnd.aurora.evidence-synthesis+json", "content_hash": digest, "semantic_loss": [], "provenance": [{"source_id": request["batch_id"], "relation": "retrieval-synthesis-workbench", "digest": digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = EvidenceSynthesis(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["batch_id"]), str(request["scope"]), str(request["query"]), disposition, tuple(ids), tuple(rank), tuple(visible), tuple(unresolved), tuple(blocked), tuple(sorted(stale)), tuple(sorted(contradictions)), tuple(required), tuple(sorted(observed)), tuple(missing_sources), ("candidate-table", "omission-audit", "source-lineage"), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), str(request["replay_identity"]), digest, tuple(), artifact, ("view:authorized-research-state",) if disposition == "qualified" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt

__all__ = ["EvidenceSynthesis", "render_retrieval_workbench", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA"]
