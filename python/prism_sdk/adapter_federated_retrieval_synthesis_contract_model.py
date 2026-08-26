"""Python parity surface for AFA-adapter-P02-F08.

The engine is deliberately local and deterministic.  It ranks typed candidate
metadata for policy-separated institutions exchanging permitted research artifacts, never moves raw source payloads, and emits
an auditable partition of selected and omitted evidence.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .research_contracts import (
    ADAPTER_FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_CONTRACT_VERSION,
    ADAPTER_FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = ADAPTER_FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_FEATURE_ID
CONTRACT_VERSION = ADAPTER_FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_CONTRACT_VERSION
INPUT_SCHEMA = "ScopedRetrievalQuery@1"
OUTPUT_SCHEMA = "EvidenceSynthesis1@1"


@dataclass(frozen=True)
class FederatedRetrievalSynthesisCandidate:
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
class FederatedRetrievalSynthesisContractModelReceipt:
    request_id: str
    query_id: str
    contract_id: str
    schema_profile: str
    canonicalization: str
    consumer: str
    algorithm_version: str
    requested_output: str
    federation_id: str
    purpose: str
    peer_order: tuple[str, ...]
    min_peer_quorum: int
    aggregate_only: bool
    envelope_digest: str
    batch_id: str
    checkpoint_seq: int
    capacity: int
    disposition: str
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    omitted_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_order: tuple[str, ...]
    contradictory_order: tuple[str, ...]
    overflow_order: tuple[str, ...]
    replay_identity: str
    synthesis_digest: str
    contract_digest: str
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
            or not self.contract_id.strip()
            or self.schema_profile != INPUT_SCHEMA
            or self.canonicalization != "aurora-json-canonical-v1"
            or not self.consumer.strip()
            or not self.algorithm_version.strip()
            or self.requested_output != OUTPUT_SCHEMA
            or not self.federation_id.strip()
            or not self.purpose.strip()
            or not self.peer_order
            or self.min_peer_quorum <= 0
            or len(self.peer_order) < self.min_peer_quorum
            or not self.aggregate_only
            or not self.batch_id.strip()
            or self.checkpoint_seq <= 0
            or self.capacity <= 0
            or not self.candidate_order
            or not self.effect_receipts
        ):
            raise ResearchContractError("federated retrieval contract model identity, output, locality, candidates, or effects are incomplete")
        for values in (
            self.peer_order, self.candidate_order, self.selected_order, self.omitted_order,
            self.uncertainty_order, self.negative_order,
            self.contradictory_order, self.overflow_order, self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated retrieval contract model ordering is not canonical")
        if set(self.selected_order) | set(self.omitted_order) != set(self.candidate_order):
            raise ResearchContractError("federated retrieval contract model states do not partition candidates")
        if any(item not in self.omitted_order for item in self.overflow_order):
            raise ResearchContractError("federated overflow must be an omitted candidate subset")
        for value in (
            self.replay_identity, self.synthesis_digest, self.contract_digest, self.envelope_digest,
            self.artifact.get("content_hash"),
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated retrieval contract model digest is invalid")
        if any(
            not effect.startswith("compute:federated-retrieval-contract-model:")
            and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("federated retrieval contract model effect is outside local computation gate")


def run_federated_retrieval_synthesis_contract_model(
    *,
    request_id: str,
    query_id: str,
    requester: str,
    intent: str,
    study_ids: Sequence[str],
    required_modalities: Sequence[str],
    comparability_profile: str,
    max_results: int,
    candidates: Sequence[FederatedRetrievalSynthesisCandidate],
    contract_id: str,
    algorithm_version: str,
    schema_profile: str = INPUT_SCHEMA,
    canonicalization: str = "aurora-json-canonical-v1",
    consumer: str = "consortium-administrator",
    requested_output: str = OUTPUT_SCHEMA,
    batch_id: str = "batch-1",
    checkpoint_seq: int = 1,
    capacity: int = 1,
    federation_id: str = "federation-1",
    purpose: str = "retrieval-synthesis",
    peer_order: Sequence[str] = ("peer-a", "peer-b"),
    min_peer_quorum: int = 2,
    aggregate_only: bool = True,
    policy_allow: bool = True,
    budget_units: int = 1,
    replay_identity: str,
    protected_closure_satisfied: bool = True,
    raw_data_local: bool = True,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> FederatedRetrievalSynthesisContractModelReceipt:
    if (
        not request_id.strip() or not query_id.strip() or not requester.strip()
        or not intent.strip() or not study_ids or not required_modalities
        or not comparability_profile.strip() or max_results <= 0
        or not candidates or not contract_id.strip() or not algorithm_version.strip()
        or schema_profile != INPUT_SCHEMA or canonicalization != "aurora-json-canonical-v1"
        or not consumer.strip()
        or requested_output != OUTPUT_SCHEMA or budget_units <= 0
        or boundary != PRECLINICAL_BOUNDARY or not raw_data_local
        or not federation_id.strip() or not purpose.strip()
        or not peer_order or min_peer_quorum <= 0 or len(peer_order) < min_peer_quorum
        or not aggregate_only or capacity > len(candidates)
        or not batch_id.strip() or checkpoint_seq <= 0 or capacity <= 0
        or capacity > len(candidates)
        or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)
    ):
        raise ResearchContractError("engine identity, scoped query, budget, locality, replay, or boundary is invalid")
    if len({item.evidence_id for item in candidates}) != len(candidates):
        raise ResearchContractError("retrieval candidate identities must be unique")
    ordered = tuple(sorted(candidates, key=lambda item: (-item.relevance_score, item.evidence_id)))
    selected: list[FederatedRetrievalSynthesisCandidate] = []
    omitted: list[FederatedRetrievalSynthesisCandidate] = []
    uncertainty: set[str] = set()
    negative: set[str] = set()
    contradictory: set[str] = set()
    allowed_studies = set(study_ids)
    required = set(required_modalities)
    for item in ordered:
        if (
            policy_allow and protected_closure_satisfied and aggregate_only
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
    if not policy_allow or not protected_closure_satisfied or not aggregate_only:
        uncertainty.add("control:policy-or-protected-closure")
    peer_order = tuple(sorted(set(peer_order)))
    federation_blocked = not policy_allow or not aggregate_only or len(peer_order) < min_peer_quorum
    disposition = (
        "blocked" if federation_blocked or not protected_closure_satisfied
        else "unknown" if not selected or omitted or uncertainty
        else "passed"
    )
    candidate_order = tuple(sorted(item.evidence_id for item in ordered))
    selected_order = tuple(sorted(item.evidence_id for item in selected))
    omitted_order = tuple(sorted(item.evidence_id for item in omitted))
    uncertainty_order = tuple(sorted(uncertainty))
    negative_order = tuple(sorted(negative))
    contradictory_order = tuple(sorted(contradictory))
    overflow_order = tuple(sorted(
        item.evidence_id for item in ordered[capacity:]
        if item.evidence_id in omitted_order
    ))
    envelope_digest = research_artifact_digest({
        "federation_id": federation_id, "purpose": purpose,
        "peer_order": list(peer_order), "min_peer_quorum": min_peer_quorum,
        "aggregate_only": aggregate_only, "policy_allow": policy_allow,
        "raw_data_local": raw_data_local,
    })
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
    contract_digest = research_artifact_digest({
        "contract_id": contract_id, "schema_profile": schema_profile,
        "canonicalization": canonicalization, "consumer": consumer,
        "algorithm_version": algorithm_version,
        "requested_output": requested_output, "query_id": query_id,
        "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "capacity": capacity,
        "federation_id": federation_id, "purpose": purpose,
        "peer_order": list(peer_order), "min_peer_quorum": min_peer_quorum,
        "aggregate_only": aggregate_only,
        "replay_identity": replay_identity, "synthesis_digest": synthesis_digest,
        "overflow_order": list(overflow_order),
        "envelope_digest": envelope_digest,
    })
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request_id, "query_id": query_id, "contract_id": contract_id,
        "schema_profile": schema_profile, "canonicalization": canonicalization,
        "consumer": consumer,
        "algorithm_version": algorithm_version, "requested_output": requested_output,
        "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "capacity": capacity,
        "federation_id": federation_id, "purpose": purpose,
        "peer_order": list(peer_order), "min_peer_quorum": min_peer_quorum,
        "aggregate_only": aggregate_only,
        "disposition": disposition, "candidate_order": list(candidate_order),
        "selected_order": list(selected_order), "omitted_order": list(omitted_order),
        "uncertainty_order": list(uncertainty_order), "negative_order": list(negative_order),
        "contradictory_order": list(contradictory_order),
        "overflow_order": list(overflow_order),
        "envelope_digest": envelope_digest,
        "replay_identity": replay_identity, "synthesis_digest": synthesis_digest,
        "contract_digest": contract_digest, "synthesis": synthesis,
        "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    receipt = FederatedRetrievalSynthesisContractModelReceipt(
        request_id=request_id, query_id=query_id, contract_id=contract_id,
        schema_profile=schema_profile, canonicalization=canonicalization, consumer=consumer,
        algorithm_version=algorithm_version, requested_output=requested_output,
        batch_id=batch_id, checkpoint_seq=checkpoint_seq, capacity=capacity,
        federation_id=federation_id, purpose=purpose, peer_order=peer_order,
        min_peer_quorum=min_peer_quorum, aggregate_only=aggregate_only,
        disposition=disposition, candidate_order=candidate_order,
        selected_order=selected_order, omitted_order=omitted_order,
        uncertainty_order=uncertainty_order, negative_order=negative_order,
        contradictory_order=contradictory_order, overflow_order=overflow_order,
        envelope_digest=envelope_digest,
        replay_identity=replay_identity,
        synthesis_digest=synthesis_digest, contract_digest=contract_digest,
        effect_receipts=(f"compute:federated-retrieval-contract-model:{contract_id}",),
        artifact={"content_hash": research_artifact_digest(payload),
                  "media_type": "application/vnd.aurora.federated-retrieval-synthesis-contract-model+json"},
    )
    receipt.validate()
    return receipt
