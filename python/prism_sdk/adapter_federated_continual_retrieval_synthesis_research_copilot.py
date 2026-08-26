"""Python parity surface for AFA-adapter-P02-F12.

The engine is deliberately local and deterministic.  It compares typed
candidate metadata across multiple preclinical modalities and studies, never
moves raw source payloads, and emits an auditable partition of selected and
omitted evidence plus an approval-gated tool intent.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .research_contracts import (
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_CONTRACT_VERSION,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_FEATURE_ID
CONTRACT_VERSION = ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_CONTRACT_VERSION
INPUT_SCHEMA = "ScopedRetrievalQuery@1"
OUTPUT_SCHEMA = "EvidenceSynthesis1@1"


@dataclass(frozen=True)
class FederatedContinualRetrievalSynthesisCandidate:
    evidence_id: str
    study_id: str
    modality: str
    comparability_profile: str
    digest: str | None
    availability: str = "available"
    relevance_score: int = 0
    negative_result: bool = False
    locator: str = "local://evidence"


@dataclass(frozen=True)
class FederatedContinualRetrievalSynthesisResearchCopilotReceipt:
    request_id: str
    query_id: str
    copilot_id: str
    agent_id: str
    recommendation_mode: str
    approval_required: bool
    schema_profile: str
    canonicalization: str
    consumer: str
    algorithm_version: str
    required_modalities: tuple[str, ...]
    tool_id: str
    approval_token: str
    comparability_digest: str
    batch_id: str
    checkpoint_seq: int
    capacity: int
    queue_digest: str
    checkpoint_digest: str
    federation_id: str
    purpose: str
    peer_ids: tuple[str, ...]
    min_peer_quorum: int
    aggregate_only: bool
    endpoint: str
    federation_digest: str
    requested_output: str
    disposition: str
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    omitted_order: tuple[str, ...]
    overflow_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_order: tuple[str, ...]
    contradictory_order: tuple[str, ...]
    replay_identity: str
    synthesis_digest: str
    copilot_digest: str
    effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (
            self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            or self.contract_version != CONTRACT_VERSION
            or self.feature_id != FEATURE_ID
            or self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.request_id.strip()
            or not self.query_id.strip()
            or not self.copilot_id.strip()
            or not self.agent_id.strip()
            or self.recommendation_mode != "evidence-ranked-read-only"
            or not self.approval_required
            or self.schema_profile != INPUT_SCHEMA
            or self.canonicalization != "aurora-json-canonical-v1"
            or not self.consumer.strip()
            or not self.algorithm_version.strip()
            or len(self.required_modalities) < 2
            or tuple(sorted(set(self.required_modalities))) != self.required_modalities
            or not self.tool_id.strip()
            or not self.batch_id.strip()
            or self.checkpoint_seq <= 0
            or self.capacity <= 0
            or not self.federation_id.strip()
            or not self.purpose.strip()
            or not self.peer_ids
            or self.min_peer_quorum <= 0
            or tuple(sorted(set(self.peer_ids))) != self.peer_ids
            or not self.aggregate_only
            or not self.endpoint.strip()
            or self.requested_output != OUTPUT_SCHEMA
            or not self.candidate_order
            or not self.effect_receipts
        ):
            raise ResearchContractError("federated continual retrieval research copilot identity, batch admission, output, locality, candidates, or effects are incomplete")
        for values in (
            self.candidate_order, self.selected_order, self.omitted_order,
            self.overflow_order,
            self.uncertainty_order, self.negative_order,
            self.contradictory_order, self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated continual retrieval research copilot ordering is not canonical")
        if set(self.selected_order) | set(self.omitted_order) != set(self.candidate_order):
            raise ResearchContractError("federated continual retrieval research copilot states do not partition candidates")
        for value in (
            self.replay_identity, self.synthesis_digest, self.copilot_digest,
            self.comparability_digest, self.queue_digest, self.checkpoint_digest,
            self.federation_digest,
            self.artifact.get("content_hash"),
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated continual retrieval research copilot digest is invalid")
        if any(
            not effect.startswith("compute:federated-continual-retrieval-research-copilot:")
            and not effect.startswith("approval-required:")
            and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("federated continual retrieval research copilot effect is outside bounded computation or approval gate")
        if not set(self.overflow_order).issubset(self.omitted_order):
            raise ResearchContractError("throughput overflow must be an omitted candidate subset")


def run_federated_continual_retrieval_synthesis_research_copilot(
    *,
    request_id: str,
    query_id: str,
    requester: str,
    intent: str,
    study_ids: Sequence[str],
    required_modalities: Sequence[str],
    comparability_profile: str,
    max_results: int,
    candidates: Sequence[FederatedContinualRetrievalSynthesisCandidate],
    copilot_id: str,
    algorithm_version: str,
    tool_id: str,
    comparability_digest: str,
    batch_id: str,
    checkpoint_seq: int,
    capacity: int,
    queue_digest: str,
    checkpoint_digest: str,
    federation_id: str,
    purpose: str,
    peer_ids: Sequence[str],
    min_peer_quorum: int,
    aggregate_only: bool,
    endpoint: str,
    federation_digest: str,
    approval_token: str = "",
    agent_id: str = "agent:federated-continual-retrieval-copilot",
    recommendation_mode: str = "evidence-ranked-read-only",
    approval_required: bool = True,
    schema_profile: str = INPUT_SCHEMA,
    canonicalization: str = "aurora-json-canonical-v1",
    consumer: str = "preclinical-researcher",
    requested_output: str = OUTPUT_SCHEMA,
    budget_units: int = 1,
    replay_identity: str,
    policy_allow: bool = True,
    protected_closure_satisfied: bool = True,
    raw_data_local: bool = True,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> FederatedContinualRetrievalSynthesisResearchCopilotReceipt:
    if (
        not request_id.strip() or not query_id.strip() or not requester.strip()
        or not intent.strip() or not study_ids or not required_modalities
        or not comparability_profile.strip() or max_results <= 0
        or not candidates or not copilot_id.strip() or not agent_id.strip()
        or recommendation_mode != "evidence-ranked-read-only"
        or schema_profile != INPUT_SCHEMA
        or canonicalization != "aurora-json-canonical-v1" or not consumer.strip()
        or not algorithm_version.strip()
        or not batch_id.strip() or checkpoint_seq <= 0 or capacity <= 0
        or not re.fullmatch(r"[0-9a-f]{64}", queue_digest)
        or not re.fullmatch(r"[0-9a-f]{64}", checkpoint_digest)
        or not federation_id.strip() or not purpose.strip() or not peer_ids
        or tuple(sorted(set(peer_ids))) != tuple(peer_ids)
        or min_peer_quorum <= 0 or not aggregate_only or not endpoint.strip()
        or not re.fullmatch(r"[0-9a-f]{64}", federation_digest)
        or len(required_modalities) < 2
        or tuple(sorted(set(required_modalities))) != tuple(required_modalities)
        or not tool_id.strip()
        or not re.fullmatch(r"[0-9a-f]{64}", comparability_digest)
        or not approval_required
        or requested_output != OUTPUT_SCHEMA or budget_units <= 0
        or boundary != PRECLINICAL_BOUNDARY or not raw_data_local
        or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)
    ):
        raise ResearchContractError("engine identity, scoped query, budget, locality, replay, or boundary is invalid")
    if len({item.evidence_id for item in candidates}) != len(candidates):
        raise ResearchContractError("retrieval candidate identities must be unique")
    ordered = tuple(sorted(candidates, key=lambda item: (-item.relevance_score, item.evidence_id)))
    selected: list[FederatedContinualRetrievalSynthesisCandidate] = []
    omitted: list[FederatedContinualRetrievalSynthesisCandidate] = []
    uncertainty: set[str] = set()
    negative: set[str] = set()
    contradictory: set[str] = set()
    allowed_studies = set(study_ids)
    required = set(required_modalities)
    for item in ordered:
        if (
            policy_allow and protected_closure_satisfied
            and item.study_id in allowed_studies
            and item.comparability_profile == comparability_profile
            and item.availability == "available"
            and item.digest is not None
            and len(selected) < min(max_results, capacity)
        ):
            selected.append(item)
            if item.negative_result:
                negative.add(item.evidence_id)
        else:
            omitted.append(item)
            if item.availability in {"unknown", "unmeasured", "stale"} or item.digest is None:
                uncertainty.add(f"evidence:{item.evidence_id}:unresolved")
            if item.availability == "contradictory":
                contradictory.add(item.evidence_id)
            if item.negative_result:
                negative.add(item.evidence_id)
    selected_modalities = {item.modality for item in selected}
    for modality in sorted(required - selected_modalities):
        uncertainty.add(f"modality:{modality}:required-coverage-missing")
    if not policy_allow or not protected_closure_satisfied:
        uncertainty.add("control:policy-or-protected-closure")
    disposition = (
        "blocked" if not policy_allow or not protected_closure_satisfied or not approval_token.strip() or len(peer_ids) < min_peer_quorum
        else "unknown" if not selected or omitted or uncertainty
        else "passed"
    )
    candidate_order = tuple(sorted(item.evidence_id for item in ordered))
    selected_order = tuple(sorted(item.evidence_id for item in selected))
    omitted_order = tuple(sorted(item.evidence_id for item in omitted))
    overflow_order = tuple(sorted(item.evidence_id for item in ordered[min(capacity, len(ordered)):]))
    uncertainty_order = tuple(sorted(uncertainty))
    negative_order = tuple(sorted(negative))
    contradictory_order = tuple(sorted(contradictory))
    synthesis = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "synthesis_id": f"evidence-synthesis:{request_id}", "query_id": query_id,
        "intent": intent, "comparability_profile": comparability_profile,
        "selected_evidence_ids": list(selected_order),
        "selected_modalities": sorted(selected_modalities),
        "selected_digests": [item.digest for item in sorted(selected, key=lambda item: item.evidence_id)],
        "evidence_state": "supported" if disposition == "passed" else "unknown",
        "negative_evidence_ids": list(negative_order),
        "contradictory_evidence_ids": list(contradictory_order),
        "omissions": list(omitted_order), "uncertainty": list(uncertainty_order),
        "boundary": PRECLINICAL_BOUNDARY,
    }
    synthesis_digest = research_artifact_digest(synthesis)
    copilot_digest = research_artifact_digest({
        "copilot_id": copilot_id, "agent_id": agent_id,
        "recommendation_mode": recommendation_mode, "approval_required": approval_required,
        "schema_profile": schema_profile,
        "canonicalization": canonicalization, "consumer": consumer,
        "algorithm_version": algorithm_version,
        "required_modalities": list(required_modalities), "tool_id": tool_id,
        "approval_token": approval_token, "comparability_digest": comparability_digest,
        "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "capacity": capacity,
        "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest,
        "federation_id": federation_id, "purpose": purpose,
        "peer_ids": list(peer_ids), "min_peer_quorum": min_peer_quorum,
        "aggregate_only": aggregate_only, "endpoint": endpoint,
        "federation_digest": federation_digest,
        "requested_output": requested_output, "query_id": query_id,
        "replay_identity": replay_identity, "synthesis_digest": synthesis_digest,
    })
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request_id, "query_id": query_id, "copilot_id": copilot_id,
        "agent_id": agent_id, "recommendation_mode": recommendation_mode,
        "approval_required": approval_required,
        "schema_profile": schema_profile, "canonicalization": canonicalization,
        "consumer": consumer,
        "algorithm_version": algorithm_version,
        "required_modalities": list(required_modalities), "tool_id": tool_id,
        "approval_token": approval_token, "comparability_digest": comparability_digest,
        "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "capacity": capacity,
        "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest,
        "federation_id": federation_id, "purpose": purpose,
        "peer_ids": list(peer_ids), "min_peer_quorum": min_peer_quorum,
        "aggregate_only": aggregate_only, "endpoint": endpoint,
        "federation_digest": federation_digest,
        "requested_output": requested_output,
        "disposition": disposition, "candidate_order": list(candidate_order),
        "selected_order": list(selected_order), "omitted_order": list(omitted_order),
        "overflow_order": list(overflow_order),
        "uncertainty_order": list(uncertainty_order), "negative_order": list(negative_order),
        "contradictory_order": list(contradictory_order),
        "replay_identity": replay_identity, "synthesis_digest": synthesis_digest,
        "copilot_digest": copilot_digest, "synthesis": synthesis,
        "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    receipt = FederatedContinualRetrievalSynthesisResearchCopilotReceipt(
        request_id=request_id, query_id=query_id, copilot_id=copilot_id,
        agent_id=agent_id, recommendation_mode=recommendation_mode,
        approval_required=approval_required,
        schema_profile=schema_profile, canonicalization=canonicalization, consumer=consumer,
        algorithm_version=algorithm_version,
        required_modalities=tuple(required_modalities), tool_id=tool_id,
        approval_token=approval_token, comparability_digest=comparability_digest,
        batch_id=batch_id, checkpoint_seq=checkpoint_seq, capacity=capacity,
        queue_digest=queue_digest, checkpoint_digest=checkpoint_digest,
        federation_id=federation_id, purpose=purpose, peer_ids=tuple(peer_ids),
        min_peer_quorum=min_peer_quorum, aggregate_only=aggregate_only,
        endpoint=endpoint, federation_digest=federation_digest,
        requested_output=requested_output,
        disposition=disposition, candidate_order=candidate_order,
        selected_order=selected_order, omitted_order=omitted_order,
        overflow_order=overflow_order,
        uncertainty_order=uncertainty_order, negative_order=negative_order,
        contradictory_order=contradictory_order, replay_identity=replay_identity,
        synthesis_digest=synthesis_digest, copilot_digest=copilot_digest,
        effect_receipts=(
            f"approval-required:{tool_id}" if not approval_token.strip()
            else "block:unsafe-release" if len(peer_ids) < min_peer_quorum
            else f"compute:federated-continual-retrieval-research-copilot:{copilot_id}",
        ),
        artifact={"content_hash": research_artifact_digest(payload),
                  "media_type": "application/vnd.aurora.federated-continual-retrieval-synthesis-research-copilot+json"},
    )
    receipt.validate()
    return receipt
