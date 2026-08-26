"""Python parity surface for AFA-adapter-P02-F26."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .adapter_multimodal_retrieval_synthesis_research_workbench import (
    render_multimodal_retrieval_synthesis_research_workbench,
)
from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = "AFA-adapter-P02-F26"
CONTRACT_VERSION = "adapter-multimodal-retrieval-synthesis-assurance-harness/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery2@1"
OUTPUT_SCHEMA = "EvidenceSynthesis5@1"


def _canonical(values: Sequence[str]) -> bool:
    return tuple(sorted(set(values))) == tuple(values)


@dataclass(frozen=True)
class MultimodalRetrievalSynthesisAssuranceHarnessReceipt:
    request_id: str
    baseline_id: str
    scope: str
    required_modalities: tuple[str, ...]
    comparability_digest: str
    verdict: str
    check_order: tuple[str, ...]
    passed_checks: tuple[str, ...]
    counterexamples: tuple[str, ...]
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    omitted_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_order: tuple[str, ...]
    contradictory_order: tuple[str, ...]
    replay_identity: str
    workbench_digest: str
    assurance_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.request_id.strip()
            or not self.baseline_id.strip()
            or not self.scope.strip()
            or len(self.required_modalities) < 2
            or not _canonical(self.required_modalities)
            or not self.check_order
            or not self.candidate_order
            or not self.effect_receipts
        ):
            raise ResearchContractError("multimodal assurance identity, modality closure, checks, candidates, locality, or effects are incomplete")
        for values in (
            self.required_modalities,
            self.check_order,
            self.passed_checks,
            self.counterexamples,
            self.candidate_order,
            self.selected_order,
            self.omitted_order,
            self.uncertainty_order,
            self.negative_order,
            self.contradictory_order,
            self.omissions,
            self.uncertainty,
            self.effect_receipts,
        ):
            if not _canonical(values):
                raise ResearchContractError("multimodal assurance ordering is not canonical")
        if any(value not in self.candidate_order for value in (*self.selected_order, *self.omitted_order)):
            raise ResearchContractError("assurance evidence state is not covered by candidates")
        for value in (
            self.comparability_digest,
            self.replay_identity,
            self.workbench_digest,
            self.assurance_digest,
            self.artifact.get("content_hash"),
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal assurance digest is invalid")
        if any(
            not effect.startswith("assure:multimodal-retrieval-synthesis:")
            and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("multimodal assurance effect is outside release gate")


def assure_multimodal_retrieval_synthesis(
    *,
    workbench_kwargs: Mapping[str, Any],
    baseline_id: str,
    expected_scope: str,
    expected_modalities: Sequence[str],
    expected_comparability_digest: str,
    policy_allow: bool,
    protected_closure: bool,
    provenance_complete: bool,
    evidence_complete: bool,
    replay_identity: str,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> MultimodalRetrievalSynthesisAssuranceHarnessReceipt:
    modalities = tuple(expected_modalities)
    if (
        not baseline_id.strip()
        or not expected_scope.strip()
        or len(modalities) < 2
        or not _canonical(modalities)
        or not re.fullmatch(r"[0-9a-f]{64}", expected_comparability_digest)
        or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)
        or boundary != PRECLINICAL_BOUNDARY
    ):
        raise ResearchContractError("multimodal assurance identity, modality/comparability contract, replay, or boundary is invalid")
    workbench = render_multimodal_retrieval_synthesis_research_workbench(**dict(workbench_kwargs))
    if (
        workbench.scope != expected_scope
        or tuple(workbench.required_modalities) != modalities
        or workbench.comparability_digest != expected_comparability_digest
        or workbench.replay_identity != replay_identity
    ):
        raise ResearchContractError("multimodal workbench does not match assurance scope, modality, comparability, or replay contract")
    checks = tuple(sorted((
        "comparability digest matches the declared modality profile",
        "evidence states preserve selected, omitted, uncertain, negative, and contradictory items",
        "modality closure covers at least two canonical modalities",
        "provenance and replay identities are content-addressed",
        "raw multimodal observations remain institution-local",
        "typed workbench receipt validates",
        "requested scope matches the workbench scope",
    )))
    passed: set[str] = set()
    counterexamples: set[str] = set()
    omissions = set(workbench.omissions)
    uncertainty = set(workbench.uncertainty)
    for ok, success, failure, uncertain in (
        (policy_allow, "policy authorization", "policy authorization denied", False),
        (protected_closure, "protected closure", "protected closure incomplete", True),
        (provenance_complete, "provenance completeness", "provenance completeness failed", False),
        (evidence_complete, "evidence completeness", "evidence completeness is unknown", True),
    ):
        if ok:
            passed.add(success)
        else:
            counterexamples.add(failure)
            (uncertainty if uncertain else omissions).add(failure)
    if workbench.disposition == "passed" and not counterexamples:
        passed.add("workbench disposition qualified")
    else:
        counterexamples.add("workbench did not establish a qualified multimodal disposition")
    verdict = "blocked" if not policy_allow or not protected_closure else ("passed" if not counterexamples else "unknown")
    effect = f"assure:multimodal-retrieval-synthesis:{baseline_id}" if verdict == "passed" else "block:unsafe-release"
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": workbench.request_id,
        "baseline_id": baseline_id,
        "scope": expected_scope,
        "required_modalities": list(modalities),
        "comparability_digest": expected_comparability_digest,
        "verdict": verdict,
        "check_order": list(checks),
        "passed_checks": sorted(passed),
        "counterexamples": sorted(counterexamples),
        "candidate_order": list(workbench.candidate_order),
        "selected_order": list(workbench.selected_order),
        "omitted_order": list(workbench.omitted_order),
        "uncertainty_order": list(workbench.uncertainty_order),
        "negative_order": list(workbench.negative_order),
        "contradictory_order": list(workbench.contradictory_order),
        "replay_identity": replay_identity,
        "workbench_digest": workbench.workbench_digest,
        "omissions": sorted(omissions),
        "uncertainty": sorted(uncertainty),
        "raw_data_local": True,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    assurance_digest = research_artifact_digest(payload)
    receipt = MultimodalRetrievalSynthesisAssuranceHarnessReceipt(
        request_id=workbench.request_id,
        baseline_id=baseline_id,
        scope=expected_scope,
        required_modalities=modalities,
        comparability_digest=expected_comparability_digest,
        verdict=verdict,
        check_order=checks,
        passed_checks=tuple(sorted(passed)),
        counterexamples=tuple(sorted(counterexamples)),
        candidate_order=tuple(workbench.candidate_order),
        selected_order=tuple(workbench.selected_order),
        omitted_order=tuple(workbench.omitted_order),
        uncertainty_order=tuple(workbench.uncertainty_order),
        negative_order=tuple(workbench.negative_order),
        contradictory_order=tuple(workbench.contradictory_order),
        replay_identity=replay_identity,
        workbench_digest=workbench.workbench_digest,
        assurance_digest=assurance_digest,
        omissions=tuple(sorted(omissions)),
        uncertainty=tuple(sorted(uncertainty)),
        effect_receipts=(effect,),
        artifact={
            "content_hash": research_artifact_digest({**payload, "assurance_digest": assurance_digest}),
            "media_type": "application/vnd.aurora.multimodal-retrieval-synthesis-assurance+json",
        },
    )
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "MultimodalRetrievalSynthesisAssuranceHarnessReceipt", "assure_multimodal_retrieval_synthesis"]
