"""Python parity surface for ``AFA-bioevalx-P08-F28``.

The adapter performs the same deterministic, digest-only release gating as the Rust
implementation.  It never receives raw experimental payloads and preserves unresolved,
contradictory, omitted, and negative evidence in the returned report.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-bioevalx-P08-F28"
CONTRACT_VERSION = "bioevalx-federated-continual-mechanism-exploration-assurance-harness/1.0"
INPUT_SCHEMA = "MechanismPortfolio5@1"
OUTPUT_SCHEMA = "MechanismAssuranceReport8@1"
CONTENT_TYPE = "application/vnd.aurora.bioevalx-mechanism-assurance-report+json"
CHECKPOINT_ORDER = (
    "admit-typed-portfolio",
    "check-evidence-and-baseline",
    "check-provenance-and-replay",
    "check-policy-and-federation",
    "retain-omission-and-negative-receipt",
)


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


@dataclass(frozen=True)
class BioevalxMechanismAssuranceReport:
    request_id: str
    federation_id: str
    purpose: str
    semantic_profile: str
    disposition: str
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    missing_candidate_order: tuple[str, ...]
    missing_study_order: tuple[str, ...]
    missing_modality_order: tuple[str, ...]
    omission_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    competing_explanation_order: tuple[str, ...]
    negative_evidence_order: tuple[str, ...]
    adversarial_event_order: tuple[str, ...]
    checkpoint_order: tuple[str, ...]
    replay_identity: str
    portfolio_digest: str
    artifact: dict[str, Any]
    effect_receipts: tuple[str, ...]
    raw_data_local: bool = True
    aggregate_only: bool = True
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID
    boundary: str = PRECLINICAL_BOUNDARY

    def to_dict(self) -> dict[str, Any]:
        result = {
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "disposition": self.disposition,
            "candidate_order": list(self.candidate_order),
            "ranked_order": list(self.ranked_order),
            "qualified_order": list(self.qualified_order),
            "unresolved_order": list(self.unresolved_order),
            "blocked_order": list(self.blocked_order),
            "missing_candidate_order": list(self.missing_candidate_order),
            "missing_study_order": list(self.missing_study_order),
            "missing_modality_order": list(self.missing_modality_order),
            "omission_order": list(self.omission_order),
            "uncertainty_order": list(self.uncertainty_order),
            "competing_explanation_order": list(self.competing_explanation_order),
            "negative_evidence_order": list(self.negative_evidence_order),
            "adversarial_event_order": list(self.adversarial_event_order),
            "checkpoint_order": list(self.checkpoint_order),
            "replay_identity": self.replay_identity,
            "portfolio_digest": self.portfolio_digest,
            "artifact": self.artifact,
            "effect_receipts": list(self.effect_receipts),
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
            "boundary": self.boundary,
        }
        return result

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY
            or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.aggregate_only
            or not self.request_id.strip()
            or not self.federation_id.strip()
            or not self.purpose.strip()
            or not self.semantic_profile.strip()
            or self.disposition not in {"qualified", "unresolved", "blocked"}
            or not self.candidate_order
            or len(self.ranked_order) != len(self.candidate_order)
            or not self.effect_receipts
            or self.checkpoint_order != CHECKPOINT_ORDER
        ):
            raise ResearchContractError("mechanism report identity, locality, partition, checkpoints, disposition, or effects are incomplete")
        for values in (
            self.candidate_order,
            self.qualified_order,
            self.unresolved_order,
            self.blocked_order,
            self.missing_candidate_order,
            self.missing_study_order,
            self.missing_modality_order,
            self.omission_order,
            self.uncertainty_order,
            self.competing_explanation_order,
            self.negative_evidence_order,
            self.adversarial_event_order,
            self.effect_receipts,
        ):
            if not _canonical(values):
                raise ResearchContractError("mechanism report ordering is not canonical")
        candidate_set = set(self.candidate_order)
        partitions = list(self.qualified_order + self.unresolved_order + self.blocked_order)
        if (
            any(item not in candidate_set for item in partitions)
            or len(partitions) != len(candidate_set)
            or len(set(partitions)) != len(partitions)
            or any(item in candidate_set for item in self.missing_candidate_order)
            or set(self.ranked_order) != candidate_set
        ):
            raise ResearchContractError("mechanism candidate states do not partition observed candidates")
        for value in (self.replay_identity, self.portfolio_digest, self.artifact.get("content_hash")):
            if not _digest(value):
                raise ResearchContractError("mechanism report digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE:
            raise ResearchContractError("mechanism report artifact content type is invalid")
        if self.disposition == "qualified":
            if len(self.effect_receipts) != 1 or not self.effect_receipts[0].startswith("verify:bioevalx-mechanism-assurance:"):
                raise ResearchContractError("qualified mechanism report effect is invalid")
        elif self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified mechanism report must block release")


def assure_mechanism_portfolio(
    *,
    request_id: str,
    federation_id: str,
    purpose: str,
    semantic_profile: str,
    required_candidate_order: Sequence[str],
    required_study_order: Sequence[str],
    required_modality_order: Sequence[str],
    candidates: Sequence[Mapping[str, Any]],
    replay_identity: str,
    policy_allow: bool,
    protected_closure: bool,
    signed_approval: bool,
    federation_approved: bool,
    raw_data_local: bool,
    aggregate_only: bool,
    budget: int,
    max_budget: int,
    adversarial_events: Sequence[str] = (),
    boundary: str = PRECLINICAL_BOUNDARY,
) -> BioevalxMechanismAssuranceReport:
    if (
        not request_id.strip()
        or not federation_id.strip()
        or not purpose.strip()
        or not semantic_profile.strip()
        or not candidates
        or not required_candidate_order
        or not required_study_order
        or not required_modality_order
        or not _canonical(required_candidate_order)
        or not _canonical(required_study_order)
        or not _canonical(required_modality_order)
        or not _canonical(adversarial_events)
        or not _digest(replay_identity)
        or budget <= 0
        or max_budget <= 0
        or boundary != PRECLINICAL_BOUNDARY
        or not raw_data_local
        or not aggregate_only
    ):
        raise ResearchContractError("mechanism portfolio identity, requirements, digest, budget, locality, or boundary is invalid")
    rows = [dict(candidate) for candidate in candidates]
    seen: set[str] = set()
    for candidate in rows:
        candidate_id = str(candidate.get("candidate_id", ""))
        if (
            not candidate_id.strip()
            or not str(candidate.get("mechanism_label", "")).strip()
            or candidate_id in seen
            or not candidate.get("study_order")
            or not candidate.get("modality_order")
            or not _canonical(candidate["study_order"])
            or not _canonical(candidate["modality_order"])
            or not 0 <= int(candidate.get("support_score_milli", -1)) <= 1000
            or not 0 <= int(candidate.get("novelty_score_milli", -1)) <= 1000
            or not _digest(candidate.get("artifact_digest"))
            or (candidate.get("provenance_digest") is not None and not _digest(candidate.get("provenance_digest")))
            or not _digest(candidate.get("replay_identity"))
            or not str(candidate.get("semantic_profile", "")).strip()
            or (candidate.get("baseline_digest") is not None and not _digest(candidate.get("baseline_digest")))
            or not _canonical(candidate.get("omissions", ()))
            or not _canonical(candidate.get("uncertainty", ()))
        ):
            raise ResearchContractError(f"candidate {candidate_id} is malformed or duplicated")
        seen.add(candidate_id)
    rows.sort(key=lambda item: (-int(item["support_score_milli"]), -int(item["novelty_score_milli"]), str(item["candidate_id"])))
    ranked_order = tuple(str(item["candidate_id"]) for item in rows)
    candidate_order = tuple(sorted(ranked_order))
    candidate_map = {str(item["candidate_id"]): item for item in rows}
    missing_candidate_order = tuple(sorted(set(required_candidate_order) - set(candidate_map)))
    required_studies, required_modalities = set(required_study_order), set(required_modality_order)
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set()
    omissions: set[str] = set(); uncertainty: set[str] = set(); competing: set[str] = set(); negative: set[str] = set()
    for candidate in rows:
        cid = str(candidate["candidate_id"])
        state = str(candidate.get("evidence_state", ""))
        if candidate.get("negative_result"):
            negative.add(f"{cid}:negative-result")
        omissions.update(f"{cid}:{item}" for item in candidate.get("omissions", ()))
        uncertainty.update(f"{cid}:{item}" for item in candidate.get("uncertainty", ()))
        if state == "contradicted":
            blocked.add(cid); competing.add(f"{cid}:contradicted-evidence"); continue
        if state in {"unknown", "speculative"}:
            unresolved.add(cid); uncertainty.add(f"{cid}:evidence-unresolved"); continue
        studies, modalities = set(candidate["study_order"]), set(candidate["modality_order"])
        complete = (
            bool(candidate.get("local_data"))
            and bool(candidate.get("permitted"))
            and candidate.get("provenance_digest") is not None
            and candidate.get("baseline_digest") is not None
            and candidate.get("replay_identity") == replay_identity
            and candidate.get("semantic_profile") == semantic_profile
            and required_studies <= studies
            and required_modalities <= modalities
            and not candidate.get("omissions")
            and not candidate.get("uncertainty")
            and int(candidate["support_score_milli"]) >= 600
        )
        if complete and state in {"proven", "supported"}:
            qualified.add(cid)
        else:
            unresolved.add(cid)
            if candidate.get("provenance_digest") is None or candidate.get("baseline_digest") is None:
                omissions.add(f"{cid}:typed-provenance-or-baseline-missing")
            if not required_studies <= studies:
                omissions.add(f"{cid}:required-study-coverage-incomplete")
            if not required_modalities <= modalities:
                omissions.add(f"{cid}:required-modality-coverage-incomplete")
            if int(candidate["support_score_milli"]) < 600:
                uncertainty.add(f"{cid}:support-threshold-not-met")
            if not candidate.get("local_data") or not candidate.get("permitted"):
                blocked.add(cid); unresolved.discard(cid); omissions.add(f"{cid}:locality-or-permission-denied")
    missing_study_order = tuple(sorted(study for study in required_study_order if not any(study in item.get("study_order", ()) for item in rows)))
    missing_modality_order = tuple(sorted(modality for modality in required_modality_order if not any(modality in item.get("modality_order", ()) for item in rows)))
    omissions.update(f"{item}:required-candidate-missing" for item in missing_candidate_order)
    omissions.update(f"required-study-missing:{item}" for item in missing_study_order)
    omissions.update(f"required-modality-missing:{item}" for item in missing_modality_order)
    negative.update(f"adversarial:{item}" for item in adversarial_events)
    global_block = (not policy_allow or not protected_closure or not signed_approval or not federation_approved or not aggregate_only or not raw_data_local or bool(adversarial_events) or budget > max_budget)
    disposition = "blocked" if global_block else ("qualified" if not missing_candidate_order and not missing_study_order and not missing_modality_order and qualified and not unresolved and not blocked else "unresolved")
    if not policy_allow: uncertainty.add("request:policy-denied")
    if not protected_closure: uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval or not federation_approved: uncertainty.add("request:institutional-approval-incomplete")
    if budget > max_budget: omissions.add("request:budget-ceiling-exceeded")
    qualified_order, unresolved_order, blocked_order = tuple(sorted(qualified)), tuple(sorted(unresolved)), tuple(sorted(blocked))
    omission_order, uncertainty_order = tuple(sorted(omissions)), tuple(sorted(uncertainty))
    competing_order, negative_order = tuple(sorted(competing)), tuple(sorted(negative))
    adversarial_order = tuple(adversarial_events)
    effects = (f"verify:bioevalx-mechanism-assurance:{request_id}",) if disposition == "qualified" else ("block:unsafe-release",)
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request_id, "federation_id": federation_id, "purpose": purpose, "semantic_profile": semantic_profile,
        "disposition": disposition, "candidate_order": list(candidate_order), "ranked_order": list(ranked_order),
        "qualified_order": list(qualified_order), "unresolved_order": list(unresolved_order), "blocked_order": list(blocked_order),
        "missing_candidate_order": list(missing_candidate_order), "missing_study_order": list(missing_study_order), "missing_modality_order": list(missing_modality_order),
        "omission_order": list(omission_order), "uncertainty_order": list(uncertainty_order), "competing_explanation_order": list(competing_order),
        "negative_evidence_order": list(negative_order), "adversarial_event_order": list(adversarial_order), "checkpoint_order": list(CHECKPOINT_ORDER),
        "replay_identity": replay_identity, "effect_receipts": list(effects), "raw_data_local": raw_data_local, "aggregate_only": aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    portfolio_digest = _hash(payload)
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"bioevalx-mechanism-assurance:{request_id}", "content_type": CONTENT_TYPE, "content_hash": portfolio_digest, "semantic_loss": [], "provenance": [{"source_id": f"federation:{federation_id}", "relation": "derived-from-local-aggregate-manifest", "digest": replay_identity}], "boundary": PRECLINICAL_BOUNDARY}
    report = BioevalxMechanismAssuranceReport(request_id=request_id, federation_id=federation_id, purpose=purpose, semantic_profile=semantic_profile, disposition=disposition, candidate_order=candidate_order, ranked_order=ranked_order, qualified_order=qualified_order, unresolved_order=unresolved_order, blocked_order=blocked_order, missing_candidate_order=missing_candidate_order, missing_study_order=missing_study_order, missing_modality_order=missing_modality_order, omission_order=omission_order, uncertainty_order=uncertainty_order, competing_explanation_order=competing_order, negative_evidence_order=negative_order, adversarial_event_order=adversarial_order, checkpoint_order=CHECKPOINT_ORDER, replay_identity=replay_identity, portfolio_digest=portfolio_digest, artifact=artifact, effect_receipts=effects)
    report.validate()
    return report


def bioevalx_mechanism_assurance_digest(report: BioevalxMechanismAssuranceReport) -> str:
    report.validate()
    return _hash(report.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "BioevalxMechanismAssuranceReport", "assure_mechanism_portfolio", "bioevalx_mechanism_assurance_digest"]
