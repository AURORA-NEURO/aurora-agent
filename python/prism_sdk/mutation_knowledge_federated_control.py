"""Python parity adapter for ``AFA-mutation-P04-F32``.

Only mutation-derived digests and oracle attestations cross the federation boundary; source worlds
and mutation payloads remain at the institution that owns them.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-mutation-P04-F32"
CONTRACT_VERSION = "mutation-federated-continual-knowledge-representation-control-plane/1.0"
INPUT_SCHEMA = "MutationKnowledgeFederatedBatch1@1"
OUTPUT_SCHEMA = "MutationKnowledgeFederatedReceipt1@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def _score(state: str) -> int:
    return {"proven": 40, "supported": 30, "speculative": 10, "unknown": 0, "contradicted": -40}.get(state, -40)


@dataclass(frozen=True)
class MutationKnowledgeFederatedReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; semantic_profile: str; admission: str
    origin_order: tuple[str, ...]; admitted_origin_order: tuple[str, ...]; mutation_order: tuple[str, ...]; rank_order: tuple[str, ...]; admitted_order: tuple[str, ...]; conditional_order: tuple[str, ...]; blocked_order: tuple[str, ...]; unknown_order: tuple[str, ...]
    decisions: tuple[Mapping[str, Any], ...]; checkpoint_seq: int; checkpoint_digest: str; control_digest: str; replay_identity: str; semantic_loss: tuple[Mapping[str, Any], ...]
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; effect_receipts: tuple[str, ...]; artifact: Mapping[str, Any]; raw_data_local: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.mutation_order or len(self.decisions) != len(self.mutation_order) or not self.effect_receipts or self.admission not in {"admitted", "approval_required", "blocked", "unknown"} or self.checkpoint_seq <= 0:
            raise ResearchContractError("mutation federation identity, locality, admission, candidates, checkpoint, or effects are incomplete")
        for values in (self.origin_order, self.admitted_origin_order, self.mutation_order, self.admitted_order, self.conditional_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _ordered(list(values)): raise ResearchContractError("mutation federation ordering is not canonical")
        if len(self.rank_order) != len(self.mutation_order) or set(self.rank_order) != set(self.mutation_order): raise ResearchContractError("mutation rank order is not a candidate permutation")
        if tuple(str(d.get("mutation_id", "")) for d in self.decisions) != self.mutation_order: raise ResearchContractError("mutation decisions do not match mutation order")
        if set(self.admitted_order) | set(self.conditional_order) | set(self.blocked_order) | set(self.unknown_order) != set(self.mutation_order): raise ResearchContractError("mutation dispositions do not partition candidates")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.control_digest, self.replay_identity, self.artifact.get("content_hash"))): raise ResearchContractError("mutation federation digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.mutation-knowledge-control+json": raise ResearchContractError("mutation federation artifact type is invalid")
        if any(not (effect.startswith("operate:mutation-knowledge:") or effect.startswith("approval-required:") or effect == "block:unsafe-release") for effect in self.effect_receipts): raise ResearchContractError("mutation effect is outside governed gate")


def operate_mutation_knowledge_federated_control(*, request: Mapping[str, Any]) -> MutationKnowledgeFederatedReceipt:
    required = ("request_id", "federation_id", "purpose", "semantic_profile", "replay_identity")
    if any(not str(request.get(field, "")).strip() for field in required) or int(request.get("required_origin_quorum", 0)) <= 0 or int(request.get("capacity", 0)) <= 0 or int(request.get("active_runs", 0)) > int(request.get("capacity", 0)) or int(request.get("checkpoint_seq", 0)) <= 0 or not request.get("candidates") or not request.get("raw_data_local", False) or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")):
        raise ResearchContractError("mutation federation request identity, quorum, capacity, checkpoint, candidates, locality, replay, or boundary is invalid")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("mutation_id", "")))
    ids = [str(item.get("mutation_id", "")) for item in candidates]
    if not all(ids) or len(set(ids)) != len(ids): raise ResearchContractError("mutation identities must be unique and non-empty")
    for candidate in candidates:
        if any(not _digest(candidate.get(field)) for field in ("parent_digest", "instance_digest", "relation_digest", "knowledge_digest", "replay_identity")): raise ResearchContractError("mutation candidate digests are invalid")
    origins = sorted({str(item.get("origin", "")) for item in candidates})
    if len(origins) < int(request["required_origin_quorum"]): raise ResearchContractError("mutation federation requires the declared origin quorum")
    global_failed = {gate for gate, failed in (("policy-allow", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("signed-approval", not request.get("signed_approval", False)), ("network-permission", not request.get("network_permitted", False)), ("origin-quorum", len(origins) < int(request["required_origin_quorum"]))) if failed}
    admitted: list[str] = []; conditional: list[str] = []; blocked: list[str] = []; decisions: list[dict[str, Any]] = []; semantic_loss: list[dict[str, Any]] = []; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); scores: dict[str, int] = {}
    for candidate in candidates:
        failed = set(global_failed); pending: set[str] = set()
        for field, gate in (("semantic_profile", "semantic-profile"), ("replay_identity", "replay-identity")):
            if candidate.get(field) != request[field]: failed.add(gate)
        for field, gate in (("policy_allow", "candidate-policy"), ("protected_closure", "candidate-protected-closure"), ("oracle_verified", "oracle-verification"), ("raw_data_local", "candidate-locality")):
            if not candidate.get(field, False): failed.add(gate)
        state = str(candidate.get("evidence_state", "unknown")); score = _score(state) + (20 if candidate.get("oracle_verified") else -20) + min(int(candidate.get("freshness_seq", 0)), 20) - min(int(candidate.get("omission_count", 0)), 20) * 2; scores[str(candidate["mutation_id"])] = score
        if state == "contradicted": failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state"); uncertainty.add(f"{candidate['mutation_id']}:evidence-state")
        if int(candidate.get("omission_count", 0)) > 0: pending.add("omission-closure"); omissions.add(f"{candidate['mutation_id']}:omissions={candidate['omission_count']}")
        negative.add(f"{candidate['mutation_id']}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}")
        disposition = "blocked" if failed else "conditional" if pending else "admitted"; {"blocked": blocked, "conditional": conditional, "admitted": admitted}[disposition].append(str(candidate["mutation_id"]))
        decisions.append({"mutation_id": str(candidate["mutation_id"]), "origin": str(candidate["origin"]), "score": score, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(candidate.get("negative_result"))})
        if failed: semantic_loss.append({"field": f"mutation:{candidate['mutation_id']}", "reason": "mutation-derived knowledge failed one or more federation gates", "severity": "decision_relevant"})
    rank_order = sorted(ids, key=lambda mutation_id: (-scores[mutation_id], mutation_id)); admitted_origins = sorted({str(c["origin"]) for c in candidates if c["mutation_id"] in admitted}); admission = "blocked" if global_failed or blocked else "approval_required" if conditional else "unknown" if not admitted else "admitted"
    checkpoint_digest = _hash({"federation_id": request["federation_id"], "checkpoint_seq": request["checkpoint_seq"], "mutation_order": ids, "origin_order": origins}); control_digest = _hash({"admission": admission, "rank_order": rank_order, "decisions": decisions, "semantic_loss": semantic_loss})
    effects = [f"operate:mutation-knowledge:{request['federation_id']}"] if admission == "admitted" else ["approval-required:mutation-knowledge", "block:unsafe-release"] if admission == "approval_required" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "admission": admission, "mutation_order": ids, "rank_order": rank_order, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "control_digest": control_digest, "replay_identity": request["replay_identity"], "semantic_loss": semantic_loss, "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative), "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"{request['request_id']}:mutation-knowledge", "content_type": "application/vnd.aurora.mutation-knowledge-control+json", "content_hash": _hash(payload), "semantic_loss": semantic_loss, "provenance": [{"source_id": request["federation_id"], "relation": "mutation-knowledge-federated-control", "digest": control_digest}], "boundary": PRECLINICAL_BOUNDARY}
    receipt = MutationKnowledgeFederatedReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["purpose"]), str(request["semantic_profile"]), admission, tuple(origins), tuple(admitted_origins), tuple(ids), tuple(rank_order), tuple(sorted(admitted)), tuple(sorted(conditional)), tuple(sorted(blocked)), (), tuple(decisions), int(request["checkpoint_seq"]), checkpoint_digest, control_digest, str(request["replay_identity"]), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), tuple(sorted(effects)), artifact, True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "MutationKnowledgeFederatedReceipt", "operate_mutation_knowledge_federated_control"]


