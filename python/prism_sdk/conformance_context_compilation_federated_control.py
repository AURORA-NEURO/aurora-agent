"""Python parity adapter for ``AFA-conformance-P03-F31``.

It performs deterministic federated conformance admission over digests and metadata only. Private
context compilation and suite execution stay at the institution that owns the payload.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-conformance-P03-F31"
CONTRACT_VERSION = "conformance-prospective-context-compilation-federated-control-plane/1.0"
INPUT_SCHEMA = "ContextCompilationFederatedBatch1@1"
OUTPUT_SCHEMA = "ContextCompilationFederatedReceipt1@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ContextCompilationFederatedControlReceipt:
    schema_version: str
    contract_version: str
    feature_id: str
    request_id: str
    service_id: str
    federation_id: str
    purpose: str
    batch_id: str
    admission: str
    peer_order: tuple[str, ...]
    accepted_peer_order: tuple[str, ...]
    incompatible_peer_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    conditional_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    decisions: tuple[Mapping[str, Any], ...]
    checkpoint_seq: int
    checkpoint_digest: str
    queue_digest: str
    control_digest: str
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
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.service_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.batch_id.strip() or not self.peer_order or not self.candidate_order or not self.effect_receipts or self.admission not in {"admitted", "degraded", "approval_required", "blocked", "unknown"} or self.checkpoint_seq <= 0:
            raise ResearchContractError("conformance context control identity, locality, admission, checkpoint, candidates, or effects are incomplete")
        for values in (self.peer_order, self.accepted_peer_order, self.incompatible_peer_order, self.candidate_order, self.admitted_order, self.conditional_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _ordered(list(values)):
                raise ResearchContractError("conformance context control ordering is not canonical")
        if len(self.decisions) != len(self.candidate_order) or tuple(str(item.get("candidate_id", "")) for item in self.decisions) != self.candidate_order:
            raise ResearchContractError("conformance context control decisions do not match candidates")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.queue_digest, self.control_digest, self.replay_identity, self.artifact.get("content_hash"))):
            raise ResearchContractError("conformance context control digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.conformance-context-control+json":
            raise ResearchContractError("conformance context control artifact type is invalid")
        if any(not (effect.startswith("operate:conformance-context:") or effect.startswith("approval-required:") or effect == "block:unsafe-release") for effect in self.effect_receipts):
            raise ResearchContractError("conformance context control effect is outside admission gate")


def operate_context_compilation_federated_control(*, request: Mapping[str, Any]) -> ContextCompilationFederatedControlReceipt:
    required = ("request_id", "service_id", "federation_id", "purpose", "batch_id", "required_suite_id", "required_protocol_version", "replay_identity")
    if any(not str(request.get(field, "")).strip() for field in required) or int(request.get("capacity", 0)) <= 0 or int(request.get("active_runs", 0)) > int(request.get("capacity", 0)) or int(request.get("checkpoint_seq", 0)) <= 0 or not request.get("candidates") or not request.get("peers") or int(request.get("required_peer_quorum", 0)) <= 0 or not request.get("raw_data_local", False) or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")):
        raise ResearchContractError("conformance context control request identity, capacity, checkpoint, peers, candidates, locality, replay, or boundary is invalid")
    peers = sorted(request["peers"], key=lambda peer: str(peer.get("peer_id", "")))
    candidates = sorted(request["candidates"], key=lambda candidate: str(candidate.get("candidate_id", "")))
    if len({str(peer.get("peer_id")) for peer in peers}) != len(peers) or len({str(candidate.get("candidate_id")) for candidate in candidates}) != len(candidates):
        raise ResearchContractError("conformance peers and candidates require unique ids")
    accepted: list[str] = []
    incompatible: list[str] = []
    semantic_loss: list[dict[str, Any]] = []
    required_caps = set(request.get("required_capabilities", []))
    for peer in peers:
        compatible = peer.get("suite_id") == request["required_suite_id"] and peer.get("protocol_version") == request["required_protocol_version"] and bool(peer.get("healthy")) and bool(peer.get("signed_identity")) and bool(peer.get("permitted_export")) and bool(peer.get("raw_data_local")) and required_caps.issubset(set(peer.get("capabilities", [])))
        (accepted if compatible else incompatible).append(str(peer["peer_id"]))
        if not compatible:
            semantic_loss.append({"field": f"peer:{peer['peer_id']}", "reason": "peer failed suite, protocol, health, identity, capability, export, or locality compatibility", "severity": "bounded"})
    gate_results = (
        ("policy-allow", not request.get("policy_allow", False)),
        ("protected-closure", not request.get("protected_closure", False)),
        ("signed-approval", not request.get("signed_approval", False)),
        ("network-permission", not request.get("network_permitted", False)),
        ("capacity", int(request.get("active_runs", 0)) >= int(request["capacity"])),
        ("peer-quorum", len(accepted) < int(request["required_peer_quorum"])),
    )
    global_failed = {gate for gate, failed in gate_results if failed}
    admitted: list[str] = []
    conditional: list[str] = []
    blocked: list[str] = []
    decisions: list[dict[str, Any]] = []
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    for candidate in candidates:
        failed = set(global_failed)
        pending: set[str] = set()
        if candidate.get("suite_id") != request["required_suite_id"]: failed.add("candidate-suite")
        if candidate.get("protocol_version") != request["required_protocol_version"]: failed.add("candidate-protocol")
        if candidate.get("replay_identity") != request["replay_identity"]: failed.add("candidate-replay")
        if not _digest(candidate.get("section_digest")) or not _digest(candidate.get("context_digest")): failed.add("typed-digests")
        state = str(candidate.get("evidence_state", "unknown"))
        if state == "contradicted": failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state"); uncertainty.add(f"{candidate['candidate_id']}:evidence-state")
        if int(candidate.get("omission_count", 0)) > 0: pending.add("omission-closure"); omissions.add(f"{candidate['candidate_id']}:omissions={candidate['omission_count']}")
        if candidate.get("negative_result"): negative.add(f"{candidate['candidate_id']}:negative-result")
        else: omissions.add(f"{candidate['candidate_id']}:negative-result-not-observed")
        disposition = "blocked" if failed else "conditional" if pending else "admitted"
        {"blocked": blocked, "conditional": conditional, "admitted": admitted}[disposition].append(str(candidate["candidate_id"]))
        decisions.append({"candidate_id": str(candidate["candidate_id"]), "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(candidate.get("negative_result"))})
    admission = "blocked" if global_failed or blocked else "approval_required" if conditional else "degraded" if incompatible else "admitted"
    peer_order = [str(peer["peer_id"]) for peer in peers]
    candidate_order = [str(candidate["candidate_id"]) for candidate in candidates]
    checkpoint_digest = _hash({"batch_id": request["batch_id"], "checkpoint_seq": request["checkpoint_seq"], "candidate_order": candidate_order, "peer_order": peer_order})
    queue_digest = _hash({"capacity": request["capacity"], "active_runs": request["active_runs"], "admitted": admitted, "conditional": conditional, "blocked": blocked})
    control_digest = _hash({"admission": admission, "checkpoint_digest": checkpoint_digest, "queue_digest": queue_digest, "semantic_loss": semantic_loss, "decisions": decisions})
    effects = (f"operate:conformance-context:{request['service_id']}",) if admission in {"admitted", "degraded"} else ("approval-required:conformance-context", "block:unsafe-release") if admission == "approval_required" else ("block:unsafe-release",)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "service_id": request["service_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "batch_id": request["batch_id"], "admission": admission, "peer_order": peer_order, "accepted_peer_order": accepted, "incompatible_peer_order": incompatible, "candidate_order": candidate_order, "admitted_order": admitted, "conditional_order": conditional, "blocked_order": blocked, "unknown_order": [], "decisions": decisions, "checkpoint_seq": request["checkpoint_seq"], "checkpoint_digest": checkpoint_digest, "queue_digest": queue_digest, "control_digest": control_digest, "replay_identity": request["replay_identity"], "semantic_loss": semantic_loss, "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative)}
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"{request['request_id']}:context-compilation-control", "content_type": "application/vnd.aurora.conformance-context-control+json", "content_hash": control_digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": request["batch_id"], "relation": "compiled-from-context-compilation-control", "digest": control_digest}], "boundary": PRECLINICAL_BOUNDARY}
    receipt = ContextCompilationFederatedControlReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["service_id"]), str(request["federation_id"]), str(request["purpose"]), str(request["batch_id"]), admission, tuple(peer_order), tuple(accepted), tuple(incompatible), tuple(candidate_order), tuple(admitted), tuple(conditional), tuple(blocked), (), tuple(decisions), int(request["checkpoint_seq"]), checkpoint_digest, queue_digest, control_digest, str(request["replay_identity"]), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), effects, artifact, True, PRECLINICAL_BOUNDARY)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "ContextCompilationFederatedControlReceipt", "operate_context_compilation_federated_control"]
