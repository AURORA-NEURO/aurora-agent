"""Python parity adapter for ``AFA-interweave-P25-F31``.

This is a deterministic preflight for the Interweave frontier control plane.  It admits only
typed, protocol-pinned, aggregate-only federated work; it never executes jobs or moves raw data.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-interweave-P25-F31"
CONTRACT_VERSION = "interweave-prospective-frontier-control-plane/1.0"
INPUT_SCHEMA = "InterweaveControlBatch1@1"
OUTPUT_SCHEMA = "InterweaveControlReceipt1@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class InterweaveControlReceipt:
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
    job_order: tuple[str, ...]
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
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.service_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.batch_id.strip() or not self.peer_order or not self.job_order or self.admission not in {"admitted", "degraded", "approval_required", "blocked", "unknown"} or not self.effect_receipts or self.checkpoint_seq <= 0:
            raise ResearchContractError("interweave control identity, locality, admission, queue, or effects are incomplete")
        for values in (self.peer_order, self.accepted_peer_order, self.incompatible_peer_order, self.job_order, self.admitted_order, self.conditional_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _ordered(list(values)):
                raise ResearchContractError("interweave control ordering is not canonical")
        if len(self.decisions) != len(self.job_order) or tuple(str(item.get("job_id", "")) for item in self.decisions) != self.job_order:
            raise ResearchContractError("interweave control decisions do not match job order")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.queue_digest, self.control_digest, self.replay_identity, self.artifact.get("content_hash"))):
            raise ResearchContractError("interweave control digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.interweave-control+json":
            raise ResearchContractError("interweave control artifact content type is invalid")
        if any(not (effect.startswith("operate:interweave-frontier:") or effect.startswith("approval-required:") or effect == "block:unsafe-release") for effect in self.effect_receipts):
            raise ResearchContractError("interweave control effect is outside admission gate")


def operate_interweave_frontier(*, request: Mapping[str, Any]) -> InterweaveControlReceipt:
    required = ("request_id", "service_id", "federation_id", "purpose", "batch_id", "required_protocol_version", "replay_identity")
    if any(not str(request.get(field, "")).strip() for field in required) or int(request.get("capacity", 0)) <= 0 or int(request.get("active_runs", 0)) > int(request.get("capacity", 0)) or int(request.get("checkpoint_seq", 0)) <= 0 or not request.get("jobs") or not request.get("peers") or request.get("boundary") != PRECLINICAL_BOUNDARY or not request.get("raw_data_local", False) or not _digest(request.get("replay_identity")):
        raise ResearchContractError("interweave control request identity, capacity, checkpoint, peers, jobs, locality, replay, or boundary is invalid")
    if request.get("signed_approval") and not str(request.get("approval_token", "")).strip():
        raise ResearchContractError("signed approval requires an approval token")
    peers = sorted(request["peers"], key=lambda p: str(p.get("peer_id", "")))
    jobs = sorted(request["jobs"], key=lambda j: str(j.get("job_id", "")))
    if any(not str(peer.get("peer_id", "")).strip() for peer in peers) or any(not str(job.get("job_id", "")).strip() for job in jobs) or len({str(peer.get("peer_id")) for peer in peers}) != len(peers) or len({str(job.get("job_id")) for job in jobs}) != len(jobs):
        raise ResearchContractError("interweave peers and jobs must have unique ids")
    required_protocol = str(request["required_protocol_version"])
    required_capabilities = set(request.get("required_capabilities", []))
    accepted_peers: list[str] = []
    incompatible: list[str] = []
    semantic_loss: list[dict[str, Any]] = []
    for peer in peers:
        peer_id = str(peer["peer_id"])
        compatible = peer.get("protocol_version") == required_protocol and bool(peer.get("healthy")) and bool(peer.get("signed_identity")) and bool(peer.get("permitted_export")) and bool(peer.get("raw_data_local")) and required_capabilities.issubset(set(peer.get("capabilities", [])))
        if compatible:
            accepted_peers.append(peer_id)
        else:
            incompatible.append(peer_id)
            semantic_loss.append({"field": f"peer:{peer_id}", "reason": "peer failed protocol, health, identity, capability, export, or locality compatibility", "severity": "bounded"})
    global_failed: set[str] = set()
    if not request.get("policy_allow", False): global_failed.add("policy-allow")
    if not request.get("protected_closure", False): global_failed.add("protected-closure")
    if not request.get("signed_approval", False): global_failed.add("signed-approval")
    if not request.get("network_permitted", False): global_failed.add("network-permission")
    if int(request.get("active_runs", 0)) >= int(request["capacity"]): global_failed.add("capacity")
    if len(accepted_peers) < int(request.get("required_peer_quorum", 0)): global_failed.add("peer-quorum")
    decisions: list[dict[str, Any]] = []
    admitted: list[str] = []
    conditional: list[str] = []
    blocked: list[str] = []
    unknown: list[str] = []
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    for job in jobs:
        job_id = str(job["job_id"])
        failed = set(global_failed)
        pending: set[str] = set()
        if job.get("protocol_version") != required_protocol: failed.add("job-protocol")
        state = str(job.get("evidence_state", "unknown"))
        if state == "contradicted": failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state"); uncertainty.add(f"{job_id}:evidence-state")
        if not job.get("capability_digests"): pending.add("capability-evidence")
        if not job.get("required_dimensions"): pending.add("required-dimensions")
        if job.get("negative_result"): negative.add(f"{job_id}:negative-result")
        else: omissions.add(f"{job_id}:negative-result-not-observed")
        disposition = "blocked" if failed else "conditional" if pending else "admitted"
        {"blocked": blocked, "conditional": conditional, "admitted": admitted}[disposition].append(job_id)
        decisions.append({"job_id": job_id, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(job.get("negative_result"))})
    if global_failed and not accepted_peers:
        unknown = []
    admission = "blocked" if global_failed or blocked else "approval_required" if conditional else "degraded" if incompatible else "admitted"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "service_id": request["service_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "batch_id": request["batch_id"], "admission": admission, "peer_order": [p["peer_id"] for p in peers], "accepted_peer_order": accepted_peers, "incompatible_peer_order": incompatible, "job_order": [j["job_id"] for j in jobs], "admitted_order": admitted, "conditional_order": conditional, "blocked_order": blocked, "unknown_order": unknown, "decisions": decisions, "checkpoint_seq": int(request["checkpoint_seq"]), "replay_identity": request["replay_identity"], "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative)}
    checkpoint_digest = _hash({"batch_id": request["batch_id"], "checkpoint_seq": request["checkpoint_seq"], "job_order": payload["job_order"], "peer_order": payload["peer_order"]})
    queue_digest = _hash({"capacity": request["capacity"], "active_runs": request["active_runs"], "admitted_order": admitted, "conditional_order": conditional, "blocked_order": blocked})
    control_digest = _hash({**payload, "checkpoint_digest": checkpoint_digest, "queue_digest": queue_digest, "semantic_loss": semantic_loss})
    effects = (f"operate:interweave-frontier:{request['service_id']}",) if admission in {"admitted", "degraded"} else ("approval-required:interweave-frontier", "block:unsafe-release") if admission == "approval_required" else ("block:unsafe-release",)
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"{request['request_id']}:interweave-control", "content_type": "application/vnd.aurora.interweave-control+json", "content_hash": control_digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": request["batch_id"], "relation": "compiled-from-interweave-control", "digest": control_digest}], "boundary": PRECLINICAL_BOUNDARY}
    receipt = InterweaveControlReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, request["request_id"], request["service_id"], request["federation_id"], request["purpose"], request["batch_id"], admission, tuple(payload["peer_order"]), tuple(accepted_peers), tuple(incompatible), tuple(payload["job_order"]), tuple(admitted), tuple(conditional), tuple(blocked), tuple(unknown), tuple(decisions), int(request["checkpoint_seq"]), checkpoint_digest, queue_digest, control_digest, request["replay_identity"], tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), effects, artifact, True, PRECLINICAL_BOUNDARY)
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "InterweaveControlReceipt", "operate_interweave_frontier"]


