"""Parity adapter for ``AFA-cli-P10-F28`` protocol-simulation assurance."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-cli-P10-F28"
CONTRACT_VERSION = "cli-federated-continual-protocol-simulation-assurance/1.0"
INPUT_SCHEMA = "ProtocolDraft4@1"
OUTPUT_SCHEMA = "ProtocolSimulationReport7@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


@dataclass(frozen=True)
class ProtocolSimulationAssuranceReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; protocol_schema: str; disposition: str
    required_step_order: tuple[str, ...]; observed_step_order: tuple[str, ...]; missing_step_order: tuple[str, ...]; violation_order: tuple[str, ...]; adversarial_event_order: tuple[str, ...]
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; protocol_digest: str; replay_identity: str; provenance_digest: str | None; verdict_digest: str; effect_receipts: tuple[str, ...]; artifact: Mapping[str, Any]; raw_data_local: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id, self.protocol_schema) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, INPUT_SCHEMA) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.required_step_order or not self.effect_receipts:
            raise ResearchContractError("protocol assurance identity, schema, locality, steps, or effects are incomplete")
        for values in (self.required_step_order, self.observed_step_order, self.missing_step_order, self.violation_order, self.adversarial_event_order, self.omissions, self.uncertainty, self.negative_evidence):
            if tuple(values) != tuple(sorted(set(values))): raise ResearchContractError("protocol assurance ordering is not canonical")
        required = set(self.required_step_order)
        if len(required) != len(self.required_step_order) or any(step not in required for step in self.observed_step_order + self.missing_step_order): raise ResearchContractError("protocol step orders are not declared subsets")
        if not all(_digest(value) for value in (self.protocol_digest, self.replay_identity, self.verdict_digest) if value is not None) or (self.provenance_digest is not None and not _digest(self.provenance_digest)): raise ResearchContractError("protocol assurance digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.protocol-simulation-report+json": raise ResearchContractError("protocol assurance artifact type is invalid")
        if any(not effect.startswith("verify:protocol-simulation:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("protocol assurance effect is outside the release gate")


def verify_protocol_simulation(*, request: Mapping[str, Any]) -> ProtocolSimulationAssuranceReceipt:
    if any(not str(request.get(key, "")).strip() for key in ("request_id", "federation_id", "purpose")) or request.get("protocol_schema") != INPUT_SCHEMA or not request.get("required_steps") or int(request.get("budget_units", 0)) <= 0 or int(request.get("max_budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True: raise ResearchContractError("protocol assurance request identity, schema, budget, or locality is invalid")
    required = sorted(str(step) for step in request["required_steps"])
    if any(not step.strip() for step in required) or len(set(required)) != len(required): raise ResearchContractError("required steps must be unique and non-empty")
    observed = sorted(set(str(step) for step in request.get("observed_steps", [])))
    if any(step not in required for step in observed): raise ResearchContractError("observed steps must be declared required steps")
    missing = sorted(set(required) - set(observed)); violations = set()
    for name, failed in (("policy", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("signed-approval", not request.get("signed_approval", False)), ("federation-approval", not request.get("federation_approved", False)), ("raw-data-locality", request.get("raw_data_local") is not True), ("budget", int(request["budget_units"]) > int(request["max_budget_units"])), ("provenance", not _digest(request.get("provenance_digest")))):
        if failed: violations.add(name)
    adversarial = set(str(event) for event in request.get("adversarial_events", [])); violations.update(f"adversarial:{event}" for event in adversarial)
    uncertainty = set(); negative = set(); omissions = set()
    if missing: omissions.add("missing-steps:" + ",".join(missing))
    state = str(request.get("evidence_state", "unknown"))
    if state in {"unknown", "speculative"}: uncertainty.add("evidence-state-not-qualified")
    elif state == "contradicted": violations.add("contradicted-evidence"); negative.add("contradicted-evidence")
    if adversarial: negative.add("adversarial-event-present")
    disposition = "blocked" if violations else "unresolved" if missing or uncertainty else "qualified"
    violation_order = tuple(sorted(violations)); adversarial_order = tuple(sorted(adversarial)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "protocol_schema": request["protocol_schema"], "disposition": disposition, "required_step_order": required, "observed_step_order": observed, "missing_step_order": missing, "violation_order": violation_order, "adversarial_event_order": adversarial_order, "protocol_digest": request["protocol_digest"], "replay_identity": request["replay_identity"], "provenance_digest": request.get("provenance_digest")}
    verdict_digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"protocol-simulation-assurance:{request['request_id']}", "content_type": "application/vnd.aurora.protocol-simulation-report+json", "content_hash": _hash(payload), "semantic_loss": [{"field": f"gate:{gate}", "reason": "protocol evidence cannot be promoted through a failed safety gate", "severity": "decision_relevant"} for gate in violation_order], "provenance": [{"source_id": request["request_id"], "relation": "protocol-simulation-assurance", "digest": verdict_digest}], "boundary": PRECLINICAL_BOUNDARY}
    effects = (f"verify:protocol-simulation:{request['request_id']}",) if disposition == "qualified" else ("block:unsafe-release",)
    receipt = ProtocolSimulationAssuranceReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["purpose"]), INPUT_SCHEMA, disposition, tuple(required), tuple(observed), tuple(missing), violation_order, adversarial_order, omissions_order, uncertainty_order, negative_order, str(request["protocol_digest"]), str(request["replay_identity"]), request.get("provenance_digest"), verdict_digest, effects, artifact, True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "ProtocolSimulationAssuranceReceipt", "verify_protocol_simulation"]
