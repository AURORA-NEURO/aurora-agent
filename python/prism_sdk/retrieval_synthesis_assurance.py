"""Python parity adapter for ``AFA-cli-P02-F27`` retrieval assurance.

The adapter verifies a caller-supplied retrieval summary.  It never performs network retrieval or
promotes unknown evidence into a scientific conclusion.
"""
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

FEATURE_ID = "AFA-cli-P02-F27"
CONTRACT_VERSION = "cli-prospective-retrieval-synthesis-assurance/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery3@1"
OUTPUT_SCHEMA = "EvidenceSynthesis7@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class RetrievalSynthesisReceipt:
    schema_version: str
    contract_version: str
    feature_id: str
    request_id: str
    corpus_id: str
    scope: str
    query: str
    disposition: str
    candidate_order: tuple[str, ...]
    rank_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    required_source_order: tuple[str, ...]
    observed_source_order: tuple[str, ...]
    missing_source_order: tuple[str, ...]
    stale_order: tuple[str, ...]
    contradiction_order: tuple[str, ...]
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    evidence_digest: str
    artifact: Mapping[str, Any]
    effect_receipts: tuple[str, ...]
    raw_data_local: bool
    boundary: str

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY
            or self.raw_data_local is not True
            or not all(
                isinstance(value, str) and value.strip()
                for value in (self.request_id, self.corpus_id, self.scope, self.query)
            )
            or not self.candidate_order
            or len(self.rank_order) != len(self.candidate_order)
            or not self.effect_receipts
            or not self.checks
        ):
            raise ResearchContractError("retrieval assurance identity, locality, candidates, checks, or effects are incomplete")
        for values in (
            self.candidate_order,
            self.selected_order,
            self.unresolved_order,
            self.blocked_order,
            self.required_source_order,
            self.observed_source_order,
            self.missing_source_order,
            self.stale_order,
            self.contradiction_order,
            self.checks,
            self.omissions,
            self.uncertainty,
            self.negative_evidence,
            self.effect_receipts,
        ):
            if not _canonical(list(values)):
                raise ResearchContractError("retrieval orders and evidence annotations are not canonical")
        if set(self.rank_order) != set(self.candidate_order):
            raise ResearchContractError("retrieval rank order is not a candidate permutation")
        partition = set(self.selected_order) | set(self.unresolved_order) | set(self.blocked_order)
        if partition != set(self.candidate_order) or sum(
            len(values) for values in (self.selected_order, self.unresolved_order, self.blocked_order)
        ) != len(partition):
            raise ResearchContractError("retrieval dispositions do not partition candidates")
        if any(
            not effect.startswith("verify:retrieval-synthesis:") and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("retrieval effect is outside the release gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.evidence-synthesis+json" or not _digest(self.artifact.get("content_hash")):
            raise ResearchContractError("retrieval artifact type or digest is invalid")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple):
                value[key] = list(item)
        return value


def assure_retrieval_synthesis(*, request: Mapping[str, Any]) -> RetrievalSynthesisReceipt:
    required = ("request_id", "corpus_id", "scope", "query")
    if (
        any(not str(request.get(field, "")).strip() for field in required)
        or request.get("query_schema") != INPUT_SCHEMA
        or not request.get("candidates")
        or not request.get("required_source_ids")
        or int(request.get("min_independent_sources", 0)) <= 0
        or int(request.get("max_selected", 0)) <= 0
        or int(request.get("budget_units", 0)) <= 0
        or int(request.get("max_budget_units", 0)) <= 0
        or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0))
        or int(request.get("max_freshness_days", 0)) <= 0
        or request.get("raw_data_local") is not True
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or not _digest(request.get("replay_identity"))
    ):
        raise ResearchContractError("retrieval assurance identity, bounds, locality, replay, or boundary is invalid")
    required_sources = [str(item) for item in request["required_source_ids"]]
    if not _canonical(required_sources) or any(not item.strip() for item in required_sources):
        raise ResearchContractError("required sources are not canonical")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("candidate_id", "")))
    ids = [str(item.get("candidate_id", "")) for item in candidates]
    if not all(ids) or len(set(ids)) != len(ids) or any(
        not str(item.get(field, "")).strip() for item in candidates for field in ("source_id", "title")
    ):
        raise ResearchContractError("candidate identifiers, sources, or titles are invalid")
    required_set = set(required_sources)
    observed_sources = sorted({str(item["source_id"]) for item in candidates})
    missing_sources = sorted(required_set - set(observed_sources))
    global_failed = {
        gate
        for gate, failed in (
            ("policy", not request.get("policy_allow", False)),
            ("protected-closure", not request.get("protected_closure", False)),
            ("raw-data-locality", request.get("raw_data_local") is not True),
            ("adversarial-input", bool(request.get("adversarial_events"))),
        )
        if failed
    }
    scores: dict[str, int] = {}
    selected: list[str] = []
    unresolved: list[str] = []
    blocked: list[str] = []
    stale: set[str] = set()
    contradictions: set[str] = set()
    omissions: set[str] = {f"missing-source:{source}" for source in missing_sources}
    uncertainty: set[str] = set()
    negative: set[str] = set()
    decisions: list[dict[str, Any]] = []
    for candidate in candidates:
        cid = str(candidate["candidate_id"])
        failed = set(global_failed)
        conditional: set[str] = set()
        if str(candidate["source_id"]) not in required_set:
            failed.add("source-not-authorized")
            omissions.add(f"{cid}:source-not-authorized")
        if not candidate.get("content_digest"):
            conditional.add("content-digest-missing")
            omissions.add(f"{cid}:content-digest-missing")
        if not candidate.get("provenance_digest"):
            conditional.add("provenance-missing")
            omissions.add(f"{cid}:provenance-missing")
        if int(candidate.get("freshness_days", 0)) > int(request["max_freshness_days"]):
            conditional.add("stale-evidence")
            stale.add(cid)
            omissions.add(f"{cid}:stale")
        for value in candidate.get("omissions", []):
            conditional.add("candidate-omissions")
            omissions.add(f"{cid}:{value}")
        for value in candidate.get("uncertainty", []):
            conditional.add("candidate-uncertainty")
            uncertainty.add(f"{cid}:{value}")
        state = str(candidate.get("evidence_state", "unknown"))
        if state == "contradicted":
            failed.add("contradicted-evidence")
            contradictions.add(cid)
            negative.add(f"{cid}:contradicted")
        elif state in {"unknown", "speculative"}:
            conditional.add("evidence-state-not-qualified")
            uncertainty.add(f"{cid}:evidence-state")
        negative.add(f"{cid}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}")
        score = int(candidate.get("relevance_milli", 0)) + (20_000 if state == "proven" else 10_000 if state == "supported" else 0) - int(candidate.get("freshness_days", 0)) * 10 - len(conditional) * 500
        scores[cid] = score
        disposition = "blocked" if failed else "unresolved" if conditional else "eligible"
        if disposition == "blocked":
            blocked.append(cid)
        elif disposition == "unresolved":
            unresolved.append(cid)
        decisions.append({"candidate_id": cid, "source_id": str(candidate["source_id"]), "score_milli": score, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(conditional), "negative_result": bool(candidate.get("negative_result"))})
    rank = sorted(ids, key=lambda cid: (-scores[cid], cid))
    spent = 0
    by_id = {str(item["candidate_id"]): item for item in candidates}
    for cid in rank:
        if cid in blocked or cid in unresolved:
            continue
        if len(selected) >= int(request["max_selected"]):
            unresolved.append(cid)
            omissions.add(f"{cid}:selection-capacity")
            continue
        cost = len(str(by_id[cid]["title"])) + 1
        if cost > int(request["budget_units"]) - spent:
            unresolved.append(cid)
            omissions.add(f"{cid}:budget-ceiling")
        else:
            spent += cost
            selected.append(cid)
    selected = sorted(set(selected))
    unresolved = sorted(set(unresolved))
    blocked = sorted(set(blocked))
    source_quorum = len({str(by_id[cid]["source_id"]) for cid in selected})
    if source_quorum < int(request["min_independent_sources"]):
        omissions.add(f"independent-source-quorum:{source_quorum}/{request['min_independent_sources']}")
    disposition = "blocked" if global_failed or blocked else "unresolved" if unresolved or missing_sources or source_quorum < int(request["min_independent_sources"]) else "qualified"
    checks = tuple(sorted({"schema-version", "candidate-identity", "content-addressed-evidence", "provenance-closure", "freshness-window", "negative-evidence-retention", "policy-boundary", "replay-identity", "source-quorum"}))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "corpus_id": request["corpus_id"], "scope": request["scope"], "query": request["query"], "candidate_order": ids, "rank_order": rank, "selected_order": selected, "unresolved_order": unresolved, "blocked_order": blocked, "missing_source_order": missing_sources, "decisions": decisions, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    evidence_digest = _hash(payload)
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"evidence-synthesis:{request['request_id']}", "content_type": "application/vnd.aurora.evidence-synthesis+json", "content_hash": _hash(payload), "semantic_loss": [{"field": f"candidate:{item}", "reason": "candidate failed a retrieval release gate", "severity": "decision_relevant"} for item in blocked], "provenance": [{"source_id": str(request["corpus_id"]), "relation": "retrieval-synthesis-assurance", "digest": evidence_digest}], "boundary": PRECLINICAL_BOUNDARY}
    receipt = RetrievalSynthesisReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["corpus_id"]), str(request["scope"]), str(request["query"]), disposition, tuple(ids), tuple(rank), tuple(selected), tuple(unresolved), tuple(blocked), tuple(required_sources), tuple(observed_sources), tuple(missing_sources), tuple(sorted(stale)), tuple(sorted(contradictions)), checks, tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), str(request["replay_identity"]), evidence_digest, artifact, (f"verify:retrieval-synthesis:{request['request_id']}",) if disposition == "qualified" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "RetrievalSynthesisReceipt", "assure_retrieval_synthesis"]
