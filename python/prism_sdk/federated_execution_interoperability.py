"""Federated continual execution-artifact interoperability for ``AFA-fiber-P12-F24``."""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-fiber-P12-F24"
CONTRACT_VERSION = "fiber-federated-continual-computational-execution-interoperability-gateway/1.0"
INPUT_SCHEMA = "ExecutionRun8@1"
OUTPUT_SCHEMA = "FederationEnvelope8@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class FederatedExecutionInteroperabilityEnvelope:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; source_institution: str; target_institution: str; purpose: str; workflow_schema: str; semantic_profile: str; protocol_version: str; disposition: str
    required_capability_order: tuple[str, ...]; offered_capability_order: tuple[str, ...]; missing_capability_order: tuple[str, ...]; violation_order: tuple[str, ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]
    artifact_digest: str; replay_identity: str; envelope_digest: str; effect_receipts: tuple[str, ...]; artifact: Mapping[str, Any]; raw_data_local: bool; aggregate_only: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id, self.workflow_schema) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, INPUT_SCHEMA) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or self.aggregate_only is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.federation_id, self.source_institution, self.target_institution, self.purpose, self.semantic_profile, self.protocol_version)) or not self.required_capability_order or not self.effect_receipts:
            raise ResearchContractError("federated execution envelope identity, schema, locality, aggregate boundary, capabilities, or effects are incomplete")
        if self.source_institution == self.target_institution:
            raise ResearchContractError("federated execution source and target institutions must differ")
        for values in (self.required_capability_order, self.offered_capability_order, self.missing_capability_order, self.violation_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values):
                raise ResearchContractError("federated execution envelope ordering is not canonical")
        required = set(self.required_capability_order); offered = set(self.offered_capability_order); missing = set(self.missing_capability_order)
        if len(required) != len(self.required_capability_order) or len(offered) != len(self.offered_capability_order) or missing != required - offered:
            raise ResearchContractError("federated execution capabilities do not close")
        for digest in (self.artifact_digest, self.replay_identity, self.envelope_digest, self.artifact.get("content_hash")):
            if not _digest(digest):
                raise ResearchContractError("federated execution digest is invalid")
        if any(not effect.startswith("exchange:execution-envelope:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated execution effect is outside the digest-only gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.execution-federation-envelope+json":
            raise ResearchContractError("federated execution artifact type is invalid")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def assure_federated_execution(*, request: Mapping[str, Any]) -> FederatedExecutionInteroperabilityEnvelope:
    required = ("request_id", "federation_id", "source_institution", "target_institution", "purpose", "semantic_profile", "protocol_version")
    if any(not str(request.get(field, "")).strip() for field in required) or request["source_institution"] == request["target_institution"] or request.get("workflow_schema") != INPUT_SCHEMA or not request.get("required_capability_order") or not _canonical([str(item) for item in request["required_capability_order"]]) or not _canonical([str(item) for item in request.get("offered_capability_order", [])]) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")):
        raise ResearchContractError("federated execution request identity, schema, capability orders, locality, budget, replay, or boundary is invalid")
    required_set = {str(item) for item in request["required_capability_order"]}; offered = [str(item) for item in request.get("offered_capability_order", [])]; offered_set = set(offered)
    if len(required_set) != len(request["required_capability_order"]) or len(offered_set) != len(offered):
        raise ResearchContractError("federated execution capability orders must be unique")
    artifact = request.get("artifact", {}); artifact_required = ("artifact_id", "content_hash", "provenance_digest", "replay_identity", "semantic_profile", "schema_version", "effect_scope")
    if any(not str(artifact.get(field, "")).strip() for field in artifact_required if field not in ("provenance_digest",)) or not _digest(artifact.get("content_hash")) or not _digest(artifact.get("provenance_digest")) or not _digest(artifact.get("replay_identity")) or artifact.get("replay_identity") != request["replay_identity"] or artifact.get("semantic_profile") != request["semantic_profile"] or artifact.get("schema_version") != "ExecutionArtifact7@1" or artifact.get("effect_scope") != "permitted-artifact" or artifact.get("raw_data_local") is not True or artifact.get("permitted") is not True:
        raise ResearchContractError("federated execution artifact identity, provenance, replay, permission, or locality is invalid")
    missing = sorted(required_set - offered_set); omissions = {str(item) for item in artifact.get("omissions", [])}; uncertainty = {str(item) for item in artifact.get("uncertainty", [])}; negative_label = "negative-result" if artifact.get("negative_result") else "negative-result-not-observed"; negative = {f"artifact:{negative_label}"}; violations: set[str] = set()
    if missing: omissions.add(f"missing-capabilities:{','.join(missing)}"); uncertainty.add("capability-closure-incomplete")
    for name, failed, omission in (("policy", request.get("policy_allow") is not True, "workflow:policy-denied"), ("protected-closure", request.get("protected_closure") is not True, "workflow:protected-closure-incomplete"), ("signed-approval", request.get("signed_approval") is not True, "workflow:signed-approval-missing"), ("federation-approval", request.get("federation_approved") is not True, "workflow:federation-approval-missing")):
        if failed: violations.add(name); omissions.add(omission)
    if artifact.get("evidence_state") == "contradicted": violations.add("contradicted-evidence"); negative.add("artifact:contradicted-evidence")
    if artifact.get("evidence_state") in {"unknown", "speculative"}: uncertainty.add("artifact:evidence-state-not-qualified")
    for event in request.get("adversarial_events", []): violations.add(f"adversarial:{event}"); omissions.add(f"workflow:adversarial:{event}")
    disposition = "blocked" if violations else "unresolved" if missing or uncertainty else "qualified"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "source_institution": request["source_institution"], "target_institution": request["target_institution"], "purpose": request["purpose"], "workflow_schema": request["workflow_schema"], "semantic_profile": request["semantic_profile"], "protocol_version": request["protocol_version"], "required_capability_order": sorted(required_set), "offered_capability_order": offered, "missing_capability_order": missing, "artifact_digest": artifact["content_hash"], "replay_identity": request["replay_identity"], "disposition": disposition, "boundary": PRECLINICAL_BOUNDARY}
    envelope_digest = _hash(payload); typed_artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"fiber-execution-envelope:{request['request_id']}", "content_type": "application/vnd.aurora.execution-federation-envelope+json", "content_hash": envelope_digest, "semantic_loss": [], "provenance": [{"source_id": request["source_institution"], "relation": "federated-execution-interoperability", "digest": artifact["content_hash"]}], "boundary": PRECLINICAL_BOUNDARY}
    report = FederatedExecutionInteroperabilityEnvelope(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["source_institution"]), str(request["target_institution"]), str(request["purpose"]), str(request["workflow_schema"]), str(request["semantic_profile"]), str(request["protocol_version"]), disposition, tuple(sorted(required_set)), tuple(offered), tuple(missing), tuple(sorted(violations)), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), str(artifact["content_hash"]), str(request["replay_identity"]), envelope_digest, (f"exchange:execution-envelope:{request['request_id']}",) if disposition == "qualified" else ("block:unsafe-release",), typed_artifact, True, True, PRECLINICAL_BOUNDARY); report.validate(); return report


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "FederatedExecutionInteroperabilityEnvelope", "assure_federated_execution"]
