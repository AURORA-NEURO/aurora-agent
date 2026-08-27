"""Python parity adapter for ``AFA-services-P16-F04``.

The adapter ranks federated release attestations, preserves uncertainty and negative findings,
and emits a digest-only recommendation. Signing and publication remain separate authorized steps.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-services-P16-F04"
CONTRACT_VERSION = "services-federated-continual-publication-release-inference/1.0"
INPUT_SCHEMA = "FederatedPublicationReleaseInferenceBatch1@1"
OUTPUT_SCHEMA = "FederatedPublicationReleaseInferenceReceipt1@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def _score(state: str) -> int:
    return {"proven": 40, "supported": 30, "speculative": 10, "unknown": 0, "contradicted": -40}.get(state, -40)


@dataclass(frozen=True)
class FederatedPublicationReleaseInferenceReceipt:
    schema_version: str
    contract_version: str
    feature_id: str
    request_id: str
    federation_id: str
    purpose: str
    semantic_profile: str
    admission: str
    origin_order: tuple[str, ...]
    qualified_origin_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    rank_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    conditional_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    decisions: tuple[Mapping[str, Any], ...]
    checkpoint_seq: int
    checkpoint_digest: str
    inference_digest: str
    replay_identity: str
    semantic_loss: tuple[Mapping[str, Any], ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool
    boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.candidate_order or len(self.decisions) != len(self.candidate_order) or not self.effect_receipts or self.admission not in {"qualified", "degraded", "blocked", "unknown"} or self.checkpoint_seq <= 0:
            raise ResearchContractError("publication inference identity, locality, admission, candidates, checkpoint, or effects are incomplete")
        for values in (self.origin_order, self.qualified_origin_order, self.candidate_order, self.qualified_order, self.conditional_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _ordered(list(values)):
                raise ResearchContractError("publication inference ordering is not canonical")
        if len(self.rank_order) != len(self.candidate_order) or set(self.rank_order) != set(self.candidate_order):
            raise ResearchContractError("publication inference rank order is not a candidate permutation")
        if tuple(str(d.get("release_id", "")) for d in self.decisions) != self.candidate_order:
            raise ResearchContractError("publication inference decisions do not match candidates")
        classified = set(self.qualified_order) | set(self.conditional_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != set(self.candidate_order):
            raise ResearchContractError("publication inference dispositions do not partition candidates")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.inference_digest, self.replay_identity, self.artifact.get("content_hash"))):
            raise ResearchContractError("publication inference digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.publication-release-inference+json":
            raise ResearchContractError("publication inference artifact type is invalid")
        if any(not (effect.startswith("infer:publication-release:") or effect == "local-only:publication-release" or effect == "block:unsafe-release") for effect in self.effect_receipts):
            raise ResearchContractError("publication inference effect is outside recommendation gate")


def operate_federated_publication_release_inference(*, request: Mapping[str, Any]) -> FederatedPublicationReleaseInferenceReceipt:
    required = ("request_id", "federation_id", "purpose", "semantic_profile", "replay_identity")
    if any(not str(request.get(field, "")).strip() for field in required) or int(request.get("required_quorum", 0)) <= 0 or int(request.get("capacity", 0)) <= 0 or int(request.get("active_runs", 0)) > int(request.get("capacity", 0)) or int(request.get("checkpoint_seq", 0)) <= 0 or not request.get("candidates") or not request.get("raw_data_local", False) or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")):
        raise ResearchContractError("publication inference request identity, quorum, capacity, checkpoint, candidates, locality, replay, or boundary is invalid")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("release_id", "")))
    ids = [str(item.get("release_id", "")) for item in candidates]
    if not all(ids) or len(set(ids)) != len(ids):
        raise ResearchContractError("publication inference release identities must be unique and non-empty")
    for candidate in candidates:
        if any(not _digest(candidate.get(field)) for field in ("artifact_digest", "evidence_digest", "provenance_digest", "replay_identity")):
            raise ResearchContractError("publication inference candidate digests are invalid")
    origins = sorted({str(item.get("origin", "")) for item in candidates})
    if len(origins) < int(request["required_quorum"]):
        raise ResearchContractError("publication inference requires the declared origin quorum")
    global_failed = {gate for gate, failed in (("policy-allow", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("capacity", int(request.get("active_runs", 0)) >= int(request["capacity"])), ("network-permission", not request.get("network_permitted", False))) if failed}
    qualified: list[str] = []
    conditional: list[str] = []
    blocked: list[str] = []
    decisions: list[dict[str, Any]] = []
    semantic_loss: list[dict[str, Any]] = []
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    scores: dict[str, int] = {}
    for candidate in candidates:
        failed = set(global_failed)
        pending: set[str] = set()
        if candidate.get("purpose") != request["purpose"]: failed.add("purpose")
        if candidate.get("semantic_profile") != request["semantic_profile"]: failed.add("semantic-profile")
        if candidate.get("replay_identity") != request["replay_identity"]: failed.add("replay-identity")
        for field, gate in (("policy_allow", "candidate-policy"), ("protected_closure", "candidate-protected-closure"), ("signer_valid", "signer"), ("raw_data_local", "candidate-locality"), ("capability_complete", "capability-completeness")):
            if not candidate.get(field, False): failed.add(gate)
        state = str(candidate.get("evidence_state", "unknown"))
        score = _score(state) + min(int(candidate.get("freshness_seq", 0)), 20) - min(int(candidate.get("omission_count", 0)), 20) * 2
        scores[ids[candidates.index(candidate)]] = score
        if state == "contradicted": failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state"); uncertainty.add(f"{candidate['release_id']}:evidence-state")
        if int(candidate.get("omission_count", 0)) > 0: pending.add("omission-closure"); omissions.add(f"{candidate['release_id']}:omissions={candidate['omission_count']}")
        negative.add(f"{candidate['release_id']}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}")
        disposition = "blocked" if failed else "conditional" if pending else "qualified"
        {"blocked": blocked, "conditional": conditional, "qualified": qualified}[disposition].append(str(candidate["release_id"]))
        decisions.append({"release_id": str(candidate["release_id"]), "origin": str(candidate["origin"]), "score": score, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(candidate.get("negative_result"))})
        if failed:
            semantic_loss.append({"field": f"release:{candidate['release_id']}", "reason": "release attestation failed one or more publication inference gates", "severity": "decision_relevant"})
    rank_order = sorted(ids, key=lambda release_id: (-scores[release_id], release_id))
    qualified_origins = sorted({str(candidate["origin"]) for candidate in candidates if candidate["release_id"] in qualified})
    admission = "blocked" if global_failed or blocked else "degraded" if conditional or not request.get("network_permitted", False) else "unknown" if not qualified else "qualified"
    checkpoint_digest = _hash({"federation_id": request["federation_id"], "checkpoint_seq": request["checkpoint_seq"], "candidate_order": ids, "origin_order": origins})
    inference_digest = _hash({"admission": admission, "rank_order": rank_order, "decisions": decisions, "semantic_loss": semantic_loss})
    effects = [f"infer:publication-release:{request['federation_id']}"] if admission == "qualified" else ["infer:publication-release:local", "local-only:publication-release"] if admission == "degraded" and not request.get("network_permitted", False) else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "admission": admission, "candidate_order": ids, "rank_order": rank_order, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "inference_digest": inference_digest, "replay_identity": request["replay_identity"], "semantic_loss": semantic_loss, "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative), "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"{request['request_id']}:publication-release-inference", "content_type": "application/vnd.aurora.publication-release-inference+json", "content_hash": _hash(payload), "semantic_loss": semantic_loss, "provenance": [{"source_id": request["federation_id"], "relation": "inference-over-release-attestations", "digest": inference_digest}], "boundary": PRECLINICAL_BOUNDARY}
    receipt = FederatedPublicationReleaseInferenceReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["purpose"]), str(request["semantic_profile"]), admission, tuple(origins), tuple(qualified_origins), tuple(ids), tuple(rank_order), tuple(sorted(qualified)), tuple(sorted(conditional)), tuple(sorted(blocked)), (), tuple(decisions), int(request["checkpoint_seq"]), checkpoint_digest, inference_digest, str(request["replay_identity"]), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), tuple(sorted(effects)), artifact, True, PRECLINICAL_BOUNDARY)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "FederatedPublicationReleaseInferenceReceipt", "operate_federated_publication_release_inference"]


