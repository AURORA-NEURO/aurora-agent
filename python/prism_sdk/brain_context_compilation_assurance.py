"""Context-compilation verification and safety assurance parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ContextAssuranceCandidate:
    context_id: str
    section_digest: str
    evidence_digest: str | None
    provenance_digest: str | None
    replay_identity: str
    state: str = "supported"
    policy_allow: bool = True
    protected_closure: bool = True
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class ContextCompilationAssuranceReceipt:
    request_id: str
    study_id: str
    scope: str
    verdict: str
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    verification_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID
    contract_version: str = CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID or self.contract_version != CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("context assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or not self.scope.strip() or not self.candidate_order or not self.witness_order or not self.effect_receipts or self.verdict not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("context assurance identity, witnesses, locality, or effects are incomplete")
        for values in (self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.witness_order, self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("context assurance ordering is not canonical")
        classified = set(self.qualified_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != set(self.candidate_order):
            raise ResearchContractError("context assurance outcomes do not partition candidates")
        for value in (self.verification_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("context assurance digest is invalid")
        if any(not e.startswith("assurance:local-context-compilation:") and e != "block:unsafe-release" for e in self.effect_receipts):
            raise ResearchContractError("context assurance effect is outside the local release gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "study_id": self.study_id, "scope": self.scope, "verdict": self.verdict, "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "verification_digest": self.verification_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def assure_context_compilation(*, request_id: str, study_id: str, scope: str, candidates: Sequence[ContextAssuranceCandidate], replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> ContextCompilationAssuranceReceipt:
    if not request_id.strip() or not study_id.strip() or not scope.strip() or not candidates or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("context assurance request identity, candidates, or replay is invalid")
    ordered = tuple(sorted(candidates, key=lambda c: c.context_id)); candidate_order = tuple(c.context_id for c in ordered)
    if any(not value.strip() for value in candidate_order) or len(set(candidate_order)) != len(candidate_order):
        raise ResearchContractError("context identifiers must be unique and non-empty")
    qualified: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); witnesses = {"gate:typed-context-contract", "gate:protected-closure", "gate:provenance", "gate:replay-identity", "gate:locality", "gate:effect-allow-list"}; counterexamples: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); global_open = policy_allow and protected_closure and raw_data_local
    for candidate in ordered:
        if not global_open or not candidate.policy_allow or not candidate.protected_closure or not candidate.raw_data_local or candidate.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(candidate.context_id); counterexamples.add(f"counterexample:{candidate.context_id}:policy-protected-closure-locality")
        elif candidate.replay_identity != replay_identity:
            unknown.add(candidate.context_id); uncertainty.add(f"context:{candidate.context_id}:replay-mismatch")
        elif candidate.evidence_digest is None or candidate.provenance_digest is None:
            unknown.add(candidate.context_id); omissions.add(f"context:{candidate.context_id}:evidence-or-provenance-missing")
        elif candidate.state in {"unknown", "speculative"}:
            unknown.add(candidate.context_id); uncertainty.add(f"context:{candidate.context_id}:evidence-uncertain")
        elif candidate.state == "contradicted":
            blocked.add(candidate.context_id); negative.add(f"context:{candidate.context_id}:contradicted")
        else:
            qualified.add(candidate.context_id)
    if not policy_allow: counterexamples.add("counterexample:policy-denied"); omissions.add("assurance:policy-denied")
    if not protected_closure: counterexamples.add("counterexample:protected-closure-incomplete"); omissions.add("assurance:protected-closure-incomplete")
    if not raw_data_local: counterexamples.add("counterexample:raw-data-locality-failed"); omissions.add("assurance:raw-data-locality-failed")
    if unknown: witnesses.add("gate:unresolved-context-retained")
    verdict = "blocked" if not global_open or blocked else "unresolved" if unknown else "qualified"
    verification_digest = research_artifact_digest({"feature_id": CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID, "request_id": request_id, "candidate_order": list(candidate_order), "qualified_order": sorted(qualified), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "witness_order": sorted(witnesses), "counterexample_order": sorted(counterexamples), "verdict": verdict, "replay_identity": replay_identity})
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "verification_digest": verification_digest}), "media_type": "application/vnd.aurora.context-compilation-assurance+json"}
    receipt = ContextCompilationAssuranceReceipt(request_id=request_id, study_id=study_id, scope=scope, verdict=verdict, candidate_order=candidate_order, qualified_order=tuple(sorted(qualified)), blocked_order=tuple(sorted(blocked)), unknown_order=tuple(sorted(unknown)), witness_order=tuple(sorted(witnesses)), counterexample_order=tuple(sorted(counterexamples)), verification_digest=verification_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"assurance:local-context-compilation:{request_id}",) if verdict == "qualified" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
