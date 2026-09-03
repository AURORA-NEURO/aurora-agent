"""Python parity surface for ``AFA-oraclex-P16-F07``.

The Rust contract is authoritative.  This adapter mirrors the admission gates and receipt shape so
workbenches can preflight a batch without uploading raw preclinical payloads or turning unknown
evidence into a release claim.
"""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-oraclex-P16-F07"
CONTRACT_VERSION = "oraclex-prospective-publication-release/1.0"
INPUT_SCHEMA = "PublicationReleaseBatch1@1"
OUTPUT_SCHEMA = "PublicationReleaseReceipt1@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(sorted(set(values))) == tuple(values)


@dataclass(frozen=True)
class PublicationReleaseReceipt:
    request_id: str
    consumer: str
    batch_id: str
    release_channel: str
    verdict: str
    candidate_order: tuple[str, ...]
    accepted_order: tuple[str, ...]
    conditional_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    decisions: tuple[Mapping[str, Any], ...]
    gate_order: tuple[str, ...]
    passed_gates: tuple[str, ...]
    failed_gates: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    replay_identity: str
    prior_release_digest: str | None
    release_digest: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool = True
    network_permitted: bool = True
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.consumer.strip() or not self.batch_id.strip() or not self.release_channel.strip() or self.verdict not in {"released", "conditional", "blocked", "unknown"} or not self.candidate_order or not self.effect_receipts or not _digest(self.replay_identity) or not _digest(self.release_digest):
            raise ResearchContractError("publication release identity, verdict, locality, candidates, effects, or digest is incomplete")
        for values in (self.candidate_order, self.accepted_order, self.conditional_order, self.blocked_order, self.unknown_order, self.gate_order, self.passed_gates, self.failed_gates, self.omissions, self.uncertainty, self.negative_evidence):
            if not _canonical(values):
                raise ResearchContractError("publication release ordering is not canonical")
        if len(self.decisions) != len(self.candidate_order) or tuple(str(item.get("artifact_id", "")) for item in self.decisions) != self.candidate_order:
            raise ResearchContractError("publication release decisions do not match candidate order")
        classified = set(self.accepted_order) | set(self.conditional_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified - set(self.candidate_order):
            raise ResearchContractError("publication release disposition references an unknown candidate")
        for value in (self.replay_identity, self.release_digest, self.artifact.get("content_hash")):
            if not _digest(value):
                raise ResearchContractError("publication release digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.publication-release+json":
            raise ResearchContractError("publication release artifact content type is invalid")
        if any(effect not in {"release:publication-research-object", "release:conditional-review", "block:unsafe-release"} for effect in self.effect_receipts):
            raise ResearchContractError("publication release effect is outside the release gate")
        if self.verdict == "released" and self.effect_receipts != ("release:publication-research-object",):
            raise ResearchContractError("released receipt must contain only the release effect")


def _candidate_complete(candidate: Mapping[str, Any]) -> bool:
    fields = ("artifact_id", "title", "version", "schema_version", "baseline_id", "license")
    digests = ("artifact_digest", "provenance_digest", "workflow_digest", "evidence_digest", "evaluation_digest")
    return all(str(candidate.get(field, "")).strip() for field in fields) and bool(candidate.get("raw_data_local")) and bool(candidate.get("source_digests")) and all(_digest(candidate.get(value)) for value in digests) and all(_digest(item) for item in candidate.get("source_digests", []))


def compile_publication_release(*, request: Mapping[str, Any]) -> PublicationReleaseReceipt:
    required = ("request_id", "consumer", "batch_id", "release_channel", "capacity", "candidates", "required_standards", "replay_identity")
    if any(not str(request.get(field, "")).strip() for field in required[:4]) or int(request.get("capacity", 0)) <= 0 or int(request.get("active_jobs", 0)) > int(request.get("capacity", 0)) or not request.get("candidates") or len(request.get("candidates", [])) > int(request.get("capacity", 0)) or not request.get("required_standards") or not request.get("raw_data_local", False) or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")):
        raise ResearchContractError("publication release request identity, capacity, standards, locality, replay, or boundary is invalid")
    if request.get("signed_approval") and not str(request.get("approval_token", "")).strip():
        raise ResearchContractError("signed approval requires an approval token")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("artifact_id", "")))
    if any(not _candidate_complete(candidate) for candidate in candidates) or any(candidates[index]["artifact_id"] == candidates[index - 1]["artifact_id"] for index in range(1, len(candidates))):
        raise ResearchContractError("publication candidates must have unique ids and complete digest envelopes")
    candidate_order: list[str] = []
    accepted: list[str] = []
    conditional: list[str] = []
    blocked: list[str] = []
    uncertainty: set[str] = set()
    omissions: set[str] = set()
    negative: set[str] = set()
    global_failed: set[str] = set()
    decisions: list[dict[str, Any]] = []
    required_standards = set(request["required_standards"])
    for candidate in candidates:
        artifact_id = str(candidate["artifact_id"])
        candidate_order.append(artifact_id)
        failed: set[str] = set()
        pending: set[str] = set()
        for gate, condition in (("policy-allow", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("signed-approval", not request.get("signed_approval", False)), ("reproducibility-bundle", not request.get("reproducibility_bundle", False)), ("negative-result-disclosure", not request.get("all_negative_results_reported", False)), ("federation-permission", not request.get("network_permitted", False))):
            if condition:
                failed.add(gate)
        standards = set(candidate.get("standards", []))
        if not standards & required_standards:
            failed.add("standards-coverage")
        elif not required_standards <= standards:
            pending.add("partial-standards-coverage")
        if not candidate.get("replication_sites"):
            pending.add("replication-site")
        state = str(candidate.get("evidence_state", "unknown"))
        if state == "contradicted":
            failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}:
            pending.add("evidence-state")
            uncertainty.add(f"{artifact_id}:evidence-state")
        findings = [str(value) for value in candidate.get("negative_findings", [])]
        if findings:
            negative.update(f"{artifact_id}:{finding}" for finding in findings)
        else:
            omissions.add(f"{artifact_id}:negative-findings-not-observed")
        global_failed.update(failed)
        disposition = "blocked" if failed else "conditional" if pending else "accepted"
        {"blocked": blocked, "conditional": conditional, "accepted": accepted}[disposition].append(artifact_id)
        decisions.append({"artifact_id": artifact_id, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_findings": findings, "source_digests": list(candidate["source_digests"])})
    if int(request.get("active_jobs", 0)) >= int(request["capacity"]):
        global_failed.add("capacity")
    migration = request.get("migration_from")
    semantic_loss = ({"field": "contract_version", "reason": "release receipt crossed a version boundary; replay the pinned source contract", "severity": "bounded"},) if migration else ()
    if migration and migration != CONTRACT_VERSION:
        uncertainty.add(f"migration:{migration}")
    verdict = "blocked" if global_failed or blocked else "conditional" if conditional else "released" if accepted else "unknown"
    gate_order = tuple(sorted({"capacity", "evaluation-baseline", "federation-permission", "negative-result-disclosure", "policy-allow", "provenance", "protected-closure", "reproducibility-bundle", "signed-approval", "standards-coverage", "typed-digests", "workflow-replay"}))
    passed = tuple(sorted({"evaluation-baseline", "provenance", "raw-locality", "typed-digests", "workflow-replay"} - global_failed))
    failed = tuple(sorted(global_failed))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "consumer": request["consumer"], "batch_id": request["batch_id"], "release_channel": request["release_channel"], "verdict": verdict, "candidate_order": candidate_order, "accepted_order": accepted, "conditional_order": conditional, "blocked_order": blocked, "unknown_order": [], "decisions": decisions, "gate_order": list(gate_order), "passed_gates": list(passed), "failed_gates": list(failed), "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative), "semantic_loss": list(semantic_loss), "replay_identity": request["replay_identity"], "prior_release_digest": request.get("prior_release_digest")}
    release_digest = _hash(payload)
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"{request['request_id']}:publication-release", "content_type": "application/vnd.aurora.publication-release+json", "content_hash": release_digest, "semantic_loss": list(semantic_loss), "provenance": [{"source_id": request["batch_id"], "relation": "compiled-from-release-batch", "digest": release_digest}], "boundary": PRECLINICAL_BOUNDARY}
    effects = ("release:publication-research-object",) if verdict == "released" else ("release:conditional-review", "block:unsafe-release") if verdict == "conditional" else ("block:unsafe-release",)
    receipt = PublicationReleaseReceipt(request_id=request["request_id"], consumer=request["consumer"], batch_id=request["batch_id"], release_channel=request["release_channel"], verdict=verdict, candidate_order=tuple(candidate_order), accepted_order=tuple(accepted), conditional_order=tuple(conditional), blocked_order=tuple(blocked), unknown_order=(), decisions=tuple(decisions), gate_order=gate_order, passed_gates=passed, failed_gates=failed, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), semantic_loss=semantic_loss, replay_identity=request["replay_identity"], prior_release_digest=request.get("prior_release_digest"), release_digest=release_digest, effect_receipts=effects, artifact=artifact, network_permitted=bool(request.get("network_permitted", False)))
    receipt.validate()
    return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "PublicationReleaseReceipt", "compile_publication_release"]
