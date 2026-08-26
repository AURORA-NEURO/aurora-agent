"""Python parity surface for AFA-influence-P14-F24.

The Rust crate remains the scientific kernel.  This module mirrors its transport contract and
deterministic release gates so Python workbenches can validate and replay gateway receipts without
silently upgrading unknown influence or exporting raw factor tables.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-influence-P14-F24"
CONTRACT_VERSION = "influence-federated-continual-interpretation-gateway/1.0"
TARGET_CONTRACT_VERSION = "1.0.0"
INPUT_SCHEMA = "EvidenceBackedResult4@1"
OUTPUT_SCHEMA = "InteractiveInterpretation6@1"


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _canonical(values: Sequence[str]) -> bool:
    return tuple(sorted(set(values))) == tuple(values)


def _bound(table: Sequence[float] | None, perturbation: str, tolerance: float | None) -> tuple[str, float | None, str | None, tuple[str, ...]]:
    methods = ("abstract_interpretation", "chain_contraction", "dynamic_range", "exact_removal")
    if perturbation == "multiplicative_range":
        if tolerance is None or not 0 <= tolerance < 1:
            return "unknown", None, None, methods
        value = min(1.0, float(2.0 * tolerance / (2.0 + tolerance)))
        return "bounded", value, "dynamic_range", methods
    if not table:
        return "unknown", None, None, methods
    if any((not isinstance(v, (int, float)) or not float(v) >= 0 or not float(v) < float("inf")) for v in table):
        return "unknown", None, None, methods
    lo, hi = min(table), max(table)
    if hi == 0:
        return "unknown", None, None, methods
    return "bounded", min(1.0, (hi - lo) / (hi + lo)), "exact_removal", methods


@dataclass(frozen=True)
class InteractiveInterpretationReceipt:
    result_id: str
    consumer: str
    scope: str
    institution_id: str
    federation_id: str
    epoch: int
    negotiated_version: str
    disposition: str
    verdict: str
    peer_order: tuple[str, ...]
    accepted_peer_order: tuple[str, ...]
    capability_order: tuple[str, ...]
    claim_order: tuple[str, ...]
    interpretation_order: tuple[Mapping[str, Any], ...]
    influence_order: tuple[Mapping[str, Any], ...]
    covered_modalities: tuple[str, ...]
    omitted_modalities: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    checks: tuple[str, ...]
    passed_checks: tuple[str, ...]
    counterexamples: tuple[str, ...]
    replay_identity: str
    federation_digest: str
    interpretation_digest: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.result_id.strip() or not self.consumer.strip() or not self.scope.strip() or not self.institution_id.strip() or not self.federation_id.strip() or self.negotiated_version != TARGET_CONTRACT_VERSION or self.verdict not in {"qualified", "conditional", "unknown", "blocked"} or not self.peer_order or not self.claim_order or not self.interpretation_order or not self.checks or not self.effect_receipts:
            raise ResearchContractError("federated interpretation identity, locality, peers, claims, checks, verdict, or effects are incomplete")
        for values in (self.peer_order, self.accepted_peer_order, self.capability_order, self.claim_order, self.covered_modalities, self.omitted_modalities, self.omissions, self.uncertainty, self.negative_evidence, self.checks, self.passed_checks, self.counterexamples, self.effect_receipts):
            if not _canonical(values):
                raise ResearchContractError("federated interpretation ordering is not canonical")
        if any(peer not in self.peer_order for peer in self.accepted_peer_order):
            raise ResearchContractError("accepted peer is absent from peer order")
        if tuple(sorted(view.get("claim_id", "") for view in self.interpretation_order)) != tuple(view.get("claim_id", "") for view in self.interpretation_order):
            raise ResearchContractError("interpretation claims are not canonical")
        for value in (self.replay_identity, self.federation_digest, self.interpretation_digest, self.artifact.get("content_hash")):
            if not _digest(value):
                raise ResearchContractError("federated interpretation digest is invalid")
        if self.verdict == "qualified" and self.disposition not in {"accepted", "migrated"}:
            raise ResearchContractError("only accepted or migrated integrations can be qualified")
        if self.artifact.get("content_type") != "application/vnd.aurora.interactive-interpretation+json":
            raise ResearchContractError("interactive interpretation artifact content type is invalid")

    def digest(self) -> str:
        self.validate()
        return _hash(asdict(self))


def _peer_state(request: Mapping[str, Any]) -> tuple[list[str], list[str], list[str], str, list[dict[str, Any]], list[str], list[str]]:
    peers = sorted(request.get("peer_capabilities", []), key=lambda peer: str(peer.get("endpoint_id", "")))
    peer_order = [str(peer.get("endpoint_id", "")) for peer in peers]
    accepted, capabilities, losses, omissions, uncertainty = [], set(), [], [], []
    migrated = False
    for peer in peers:
        endpoint = str(peer.get("endpoint_id", "")); version = str(peer.get("contract_version", ""))
        version_migrated = version == "0.9.0" and TARGET_CONTRACT_VERSION in peer.get("supported_contract_versions", [])
        if version not in {TARGET_CONTRACT_VERSION, "0.9.0"} or (version == "0.9.0" and not version_migrated):
            omissions.append(f"{endpoint}:incompatible-contract"); continue
        if not peer.get("permitted_export", False):
            omissions.append(f"{endpoint}:export-denied"); continue
        if not peer.get("healthy", False) or not peer.get("signed_capability", False):
            uncertainty.append(f"{endpoint}:peer-health-or-signature-unresolved"); continue
        if not all(cap in peer.get("capabilities", []) for cap in request["required_capabilities"]):
            omissions.append(f"{endpoint}:required-capability-missing"); continue
        accepted.append(endpoint); capabilities.update(str(cap) for cap in peer.get("capabilities", []))
        if version_migrated:
            migrated = True
            losses.append({"field": "legacy_peer_fields", "reason": "compatible migration cannot infer fields absent from the pinned target contract", "severity": "unknown"})
    disposition = "migrated" if migrated and len(accepted) >= request["quorum"] else "accepted" if len(accepted) >= request["quorum"] else "unknown"
    if len(accepted) < request["quorum"]:
        uncertainty.append(f"peer quorum not met: {len(accepted)} of {request['quorum']} required")
    if not request.get("policy_allow", False):
        omissions.append("federation policy denied artifact exchange"); disposition = "blocked"
    elif not request.get("protected_closure", False) or not request.get("signed_approval", False):
        uncertainty.append("protected closure or signed A2 approval is incomplete"); disposition = "approval_required"
    return peer_order, sorted(accepted), sorted(capabilities), disposition, sorted(losses, key=lambda row: row["field"]), sorted(set(omissions)), sorted(set(uncertainty))


def run_federated_continual_interpretation(*, request: Mapping[str, Any]) -> InteractiveInterpretationReceipt:
    required = ("result_id", "consumer", "scope", "institution_id", "federation_id", "target_contract_version", "required_capabilities", "quorum", "peer_capabilities", "evidence_digests", "required_modalities", "variables", "factors", "free_variables", "claims", "perturbation_class", "replay_identity")
    if any(not str(request.get(key, "")).strip() for key in required[:6]) or request.get("target_contract_version") != TARGET_CONTRACT_VERSION or int(request.get("epoch", 0)) <= 0 or int(request.get("quorum", 0)) <= 0 or int(request.get("quorum", 0)) > len(request.get("peer_capabilities", [])) or not request.get("raw_data_local", False) or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")) or request.get("perturbation_class") not in {"removal", "multiplicative_range"}:
        raise ResearchContractError("federated interpretation request identity, contract, quorum, locality, replay, or perturbation is invalid")
    if not _canonical(request["required_capabilities"]) or not _canonical(request["required_modalities"]) or not _canonical(request["free_variables"]):
        raise ResearchContractError("required capabilities, modalities, and free variables must be canonical")
    factor_rows = sorted(request["factors"], key=lambda factor: str(factor.get("factor_id", "")))
    evidence = set(request["evidence_digests"])
    omissions: list[str] = []; uncertainty: list[str] = []; negative: list[str] = []; influence: list[dict[str, Any]] = []
    all_bounded = True
    for factor in factor_rows:
        fid = str(factor.get("factor_id", "")); state = str(factor.get("evidence_state", "unknown")); estimate, bound, method, methods = _bound(factor.get("table"), request["perturbation_class"], request.get("relative_tolerance"))
        if estimate == "unknown":
            all_bounded = False; uncertainty.append(f"{fid}:influence-unknown")
        if state not in {"proven", "supported"}:
            all_bounded = False; uncertainty.append(f"{fid}:evidence-state-not-supported")
        if factor.get("negative_result"): negative.append(f"{fid}:negative-result")
        influence.append({"factor_id": fid, "modality": factor.get("modality", ""), "evidence_state": state, "estimate": estimate, "bound": bound, "selected_method": method, "attempted": [{"method": item} for item in methods], "evidence_digest": factor.get("evidence_digest"), "provenance_digest": factor.get("provenance_digest"), "negative_result": bool(factor.get("negative_result", False))})
    covered = sorted(set(str(claim.get("modality", "")) for claim in request["claims"]))
    omitted_modalities = sorted(set(request["required_modalities"]) - set(covered))
    if omitted_modalities:
        all_bounded = False; omissions.extend(f"required modality unavailable: {modality}" for modality in omitted_modalities)
    peer_order, accepted_peer_order, capability_order, disposition, losses, peer_omissions, peer_uncertainty = _peer_state(request)
    omissions.extend(peer_omissions); uncertainty.extend(peer_uncertainty)
    if disposition in {"accepted", "migrated"} and (not all_bounded or omitted_modalities): disposition = "unknown"
    verdict = "qualified" if disposition in {"accepted", "migrated"} else "conditional" if disposition == "approval_required" else "blocked" if disposition in {"blocked", "incompatible"} else "unknown"
    interpretation = tuple({"claim_id": str(claim.get("claim_id", "")), "modality": str(claim.get("modality", "")), "statement": str(claim.get("statement", "")), "bound_factor_order": [row["factor_id"] for row in influence], "uncertainty": str(claim.get("uncertainty", "")), "negative_evidence": sorted(set(str(value) for value in claim.get("negative_evidence", [])))} for claim in sorted(request["claims"], key=lambda claim: str(claim.get("claim_id", ""))))
    checks = tuple(sorted(("canonical peer and capability negotiation", "digest-only federation and continual replay identity", "evidence, provenance, uncertainty, and negative-result retention", "pinned contract version and migration loss", "protected closure, policy, and signed A2 approval", "typed local region and sound influence methods")))
    passed = checks if verdict == "qualified" else ()
    counter = tuple(sorted(set((["peer quorum not met"] if len(accepted_peer_order) < int(request["quorum"]) else []) + (["one or more required influence or evidence gates are unresolved"] if not all_bounded else []))))
    effects = ("exchange:permitted-artifact-digests-only", "interpret:qualified") if verdict == "qualified" else ("approval-required:protected-closure-or-signed-authority",) if verdict == "conditional" else ("blocked:policy-or-boundary",) if verdict == "blocked" else ("partial:retain-unknown-and-omissions",)
    peer_payload = {"federation_id": request["federation_id"], "epoch": request["epoch"], "target_contract_version": request["target_contract_version"], "peer_order": peer_order, "accepted_peer_order": accepted_peer_order, "capability_order": capability_order}
    federation_digest = _hash(peer_payload); omissions = tuple(sorted(set(omissions))); uncertainty = tuple(sorted(set(uncertainty))); negative = tuple(sorted(set(negative))); losses = tuple(sorted(losses, key=lambda row: row["field"]))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "result_id": request["result_id"], "consumer": request["consumer"], "scope": request["scope"], "institution_id": request["institution_id"], "federation_id": request["federation_id"], "epoch": request["epoch"], "negotiated_version": TARGET_CONTRACT_VERSION, "disposition": disposition, "verdict": verdict, "peer_order": peer_order, "accepted_peer_order": accepted_peer_order, "capability_order": capability_order, "claim_order": [view["claim_id"] for view in interpretation], "interpretation_order": list(interpretation), "influence_order": influence, "covered_modalities": covered, "omitted_modalities": omitted_modalities, "omissions": list(omissions), "uncertainty": list(uncertainty), "negative_evidence": list(negative), "semantic_loss": list(losses), "checks": list(checks), "passed_checks": list(passed), "counterexamples": list(counter), "replay_identity": request["replay_identity"], "federation_digest": federation_digest, "effect_receipts": list(effects), "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    interpretation_digest = _hash(payload)
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"interactive-interpretation:{request['federation_id']}:{request['epoch']}", "content_type": "application/vnd.aurora.interactive-interpretation+json", "content_hash": _hash(payload), "semantic_loss": list(losses), "provenance": [], "boundary": PRECLINICAL_BOUNDARY}
    receipt = InteractiveInterpretationReceipt(result_id=request["result_id"], consumer=request["consumer"], scope=request["scope"], institution_id=request["institution_id"], federation_id=request["federation_id"], epoch=int(request["epoch"]), negotiated_version=TARGET_CONTRACT_VERSION, disposition=disposition, verdict=verdict, peer_order=tuple(peer_order), accepted_peer_order=tuple(accepted_peer_order), capability_order=tuple(capability_order), claim_order=tuple(view["claim_id"] for view in interpretation), interpretation_order=interpretation, influence_order=tuple(influence), covered_modalities=tuple(covered), omitted_modalities=tuple(omitted_modalities), omissions=omissions, uncertainty=uncertainty, negative_evidence=negative, semantic_loss=losses, checks=checks, passed_checks=passed, counterexamples=counter, replay_identity=request["replay_identity"], federation_digest=federation_digest, interpretation_digest=interpretation_digest, effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "InteractiveInterpretationReceipt", "run_federated_continual_interpretation"]
