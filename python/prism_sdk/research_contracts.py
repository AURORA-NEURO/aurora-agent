"""Small standard-library mirror of the Rust research-contract boundary.

The Python SDK transports receipts and hashes payloads; it never interprets an unknown receipt as a
positive scientific conclusion or moves protected source bytes out of an institution.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import json
import math
import re
from typing import Any, Mapping, Sequence

RESEARCH_CONTRACT_SCHEMA_VERSION = "aurora-research-contract/1.0"
PRECLINICAL_BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
RESEARCH_FEATURE_ID = "AFA-bioir-P02-F01"
RELEASE_REVIEW_FEATURE_ID = "AFA-evalengine-P13-F01"
RESEARCH_INGESTION_FEATURE_ID = "AFA-adapter-P06-F01"
EXPERIMENT_DESIGN_FEATURE_ID = "AFA-lab-P09-F01"
PROTOCOL_SIMULATION_FEATURE_ID = "AFA-lab-P10-F01"
REPLICATION_FEATURE_ID = "AFA-evalengine-P15-F01"
QUALITY_CONTROL_FEATURE_ID = "AFA-adapter-P07-F01"
RESEARCH_CONTEXT_FEATURE_ID = "AFA-fiber-P03-F01"
REPLAY_AUDIT_FEATURE_ID = "AFA-runtime-P23-F01"
WORKFLOW_EXECUTION_FEATURE_ID = "AFA-runtime-P12-F10"
EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-evalengine-P23-F01"
RESEARCH_RELEASE_FEATURE_ID = "AFA-services-P16-F02"
INSTRUMENT_PREFLIGHT_FEATURE_ID = "AFA-lab-P11-F01"
MULTIMODAL_HARMONIZATION_FEATURE_ID = "AFA-adapter-P06-F02"
ANALYSIS_QUALIFICATION_FEATURE_ID = "AFA-evalengine-P13-F01"
PROTOCOL_MATRIX_FEATURE_ID = "AFA-lab-P10-F02"
MULTIMODAL_REPLICATION_FEATURE_ID = "AFA-evalengine-P15-F02"
QUALITY_DRIFT_FEATURE_ID = "AFA-adapter-P07-F02"
DESIGN_FRONTIER_FEATURE_ID = "AFA-lab-P09-F02"
AUTONOMY_BATCH_FEATURE_ID = "AFA-policy-P19-F02"
WORKFLOW_BATCH_FEATURE_ID = "AFA-runtime-P12-F11"
RESEARCH_RELEASE_BATCH_FEATURE_ID = "AFA-services-P16-F03"
FEDERATED_EVALUATION_FEATURE_ID = "AFA-evalengine-P23-F02"
RESOURCE_WORKBENCH_FEATURE_ID = "AFA-fiber-P05-F20"
RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID = "AFA-mcp-P05-F08"
RESOURCE_DISCOVERY_CONTRACT_VERSION = "aurora-mcp-resource-discovery/2.0"
GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID = "AFA-governance-P16-F08"
GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION = "signed-research-object/2.0"
RELEASE_HARNESS_FEATURE_ID = "AFA-obligation-P16-F27"
RELEASE_HARNESS_CONTRACT_VERSION = "release-assurance-harness/1.0"
PROTOCOL_ASSURANCE_FEATURE_ID = "AFA-policy-P10-F27"
PROTOCOL_ASSURANCE_CONTRACT_VERSION = "protocol-assurance-harness/1.0"
FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID = "AFA-routing-P06-F28"
FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION = "federated-multimodal-assurance/1.0"
FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID = "AFA-store-P04-F24"
FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION = "federated-knowledge-gateway/1.0"
FEDERATED_LENS_ASSURANCE_FEATURE_ID = "AFA-lens-P04-F28"
FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION = "federated-lens-assurance/1.0"
SEMANTIC_PARITY_FEATURE_ID = "AFA-lab-P28-F12"
SEMANTIC_PARITY_CONTRACT_VERSION = "lab-semantic-parity/1.0"
FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-fiber-P02-F28"
FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "federated-retrieval-assurance/1.0"
FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID = "AFA-atlashub-P02-F12"
FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION = "federated-continual-retrieval-copilot/1.0"
CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-devplat-P03-F28"
CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "federated-context-compilation-assurance/1.0"
KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID = "AFA-ops-P04-F28"
KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION = "federated-knowledge-representation-assurance/1.0"
RESOURCE_CONTROL_PLANE_FEATURE_ID = "AFA-weave-P05-F32"
RESOURCE_CONTROL_PLANE_CONTRACT_VERSION = "federated-resource-control-plane/1.0"
WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID = "AFA-weavelang-P16-F27"
WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION = "weavelang-release-assurance/1.0"
MECHANISM_CONTROL_PLANE_FEATURE_ID = "AFA-adapter-P08-F31"
MECHANISM_CONTROL_PLANE_CONTRACT_VERSION = "federated-mechanism-control-plane/1.0"
MECHANISM_GATEWAY_FEATURE_ID = "AFA-fiber-P08-F24"
MECHANISM_GATEWAY_CONTRACT_VERSION = "federated-mechanism-interoperability-gateway/1.0"
EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-adapter-P01-F09"
EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "evidence-surveillance-copilot/1.0"
RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-adapter-P02-F06"
RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "multimodal-retrieval-synthesis/1.0"
ADAPTER_CONTEXT_COMPILATION_FEATURE_ID = "AFA-adapter-P03-F27"
ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION = "prospective-context-compilation-assurance/1.0"
KNOWLEDGE_WORKFLOW_FEATURE_ID = "AFA-adapter-P04-F14"
KNOWLEDGE_WORKFLOW_CONTRACT_VERSION = "multimodal-knowledge-workflow-fabric/1.0"
RESOURCE_WORKBENCH_FEATURE_ID = "AFA-adapter-P05-F18"
RESOURCE_WORKBENCH_CONTRACT_VERSION = "multimodal-resource-workbench/1.0"
INGESTION_GATEWAY_FEATURE_ID = "AFA-adapter-P06-F23"
INGESTION_GATEWAY_CONTRACT_VERSION = "1.0"
QUALITY_ENVELOPE_FEATURE_ID = "AFA-adapter-P07-F06"
QUALITY_ENVELOPE_CONTRACT_VERSION = "multi-study-quality-envelope/1.0"
EXPERIMENT_DESIGN_CONTROL_FEATURE_ID = "AFA-adapter-P09-F30"
EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION = "federated-experiment-design-control-plane/1.0"
PROTOCOL_SIMULATION_FEATURE_ID = "AFA-adapter-P10-F03"
PROTOCOL_SIMULATION_CONTRACT_VERSION = "prospective-protocol-simulation/1.0"
INSTRUMENT_MESH_FEATURE_ID = "AFA-adapter-P11-F04"
INSTRUMENT_MESH_CONTRACT_VERSION = "federated-laboratory-integration/1.0"
EXECUTION_CONTROL_FEATURE_ID = "AFA-adapter-P12-F31"
EXECUTION_CONTROL_CONTRACT_VERSION = "computational-execution-control-plane/1.0"


class ResearchContractError(ValueError):
    """A receipt would violate the shared safety or evidence boundary."""


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def research_artifact_digest(payload: Any) -> str:
    return hashlib.sha256(canonical_json(payload)).hexdigest()


@dataclass(frozen=True)
class PolicyReceipt:
    receipt_id: str
    decision: str
    reasons: tuple[str, ...]
    evaluated_artifacts: tuple[str, ...] = ()
    authority_reference: str | None = None
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if not self.receipt_id.strip() or not self.reasons:
            raise ResearchContractError("policy receipt needs an id and reason")
        if self.decision in {"approval_required", "unresolved"} and self.authority_reference:
            raise ResearchContractError("authority is premature for unresolved policy")
        if self.decision == "allow" and "unresolved" in self.reasons:
            raise ResearchContractError("unresolved policy cannot allow")


@dataclass(frozen=True)
class EvidenceReceipt:
    receipt_id: str
    intent: str
    sources: tuple[Mapping[str, Any], ...]
    derivation: tuple[str, ...]
    uncertainty: tuple[Mapping[str, str], ...]
    omissions: tuple[Mapping[str, str], ...]
    conclusion_state: str
    competing_explanations: tuple[Mapping[str, Any], ...] = ()
    negative_evidence: tuple[Mapping[str, Any], ...] = ()
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if not self.receipt_id.strip() or not self.intent.strip() or not self.derivation:
            raise ResearchContractError("evidence receipt is incomplete")
        if not self.sources and (self.conclusion_state != "unknown" or not self.omissions or not self.uncertainty):
            raise ResearchContractError("empty evidence must be explicit unknown")
        if self.conclusion_state == "proven" and any(item.get("could_change_decision") != "no_known_impact" for item in self.omissions):
            raise ResearchContractError("protected omission blocks proven conclusion")


@dataclass(frozen=True)
class ReleaseReview:
    """Transport-level mirror of the Rust fail-closed production review."""

    capability_id: str
    card_digest: str
    verdict: str
    reasons: tuple[str, ...]
    replications: tuple[Mapping[str, Any], ...] = ()
    checks: tuple[Mapping[str, Any], ...] = ()
    provenance_complete: bool = False
    feature_id: str = RELEASE_REVIEW_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if self.feature_id != RELEASE_REVIEW_FEATURE_ID or not self.capability_id.strip():
            raise ResearchContractError("release review feature or capability is missing")
        if len(self.card_digest) != 64 or any(char not in "0123456789abcdef" for char in self.card_digest):
            raise ResearchContractError("release review card digest is not a canonical sha256")
        if self.verdict not in {"pass", "conditional", "blocked", "not_evaluated"} or not self.reasons:
            raise ResearchContractError("release review verdict and reasons are required")
        if self.verdict == "pass" and not self.provenance_complete:
            raise ResearchContractError("a passing release review requires complete provenance")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "capability_id": self.capability_id,
            "card_digest": self.card_digest,
            "verdict": self.verdict,
            "reasons": list(self.reasons),
            "replications": list(self.replications),
            "checks": list(self.checks),
            "provenance_complete": self.provenance_complete,
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ResearchIngestionBundle:
    """Portable QC certificate; raw source bytes stay institution-local."""

    source_id: str
    adapter: str
    adapter_version: str
    source_digest: str
    ingestion_digest: str
    artifact: Mapping[str, Any]
    conformance: Mapping[str, Any]
    raw_data_local: bool = True
    feature_id: str = RESEARCH_INGESTION_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if self.feature_id != RESEARCH_INGESTION_FEATURE_ID or not self.source_id.strip():
            raise ResearchContractError("research ingestion feature or source is missing")
        for digest in (self.source_digest, self.ingestion_digest, self.artifact.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("research ingestion digest is not a canonical sha256")
        if not self.raw_data_local:
            raise ResearchContractError("raw research data must remain local")
        if not self.conformance.get("verified", False):
            raise ResearchContractError("research ingestion is not conformance verified")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "source_id": self.source_id,
            "adapter": self.adapter,
            "adapter_version": self.adapter_version,
            "source_digest": self.source_digest,
            "ingestion_digest": self.ingestion_digest,
            "artifact": dict(self.artifact),
            "conformance": dict(self.conformance),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ExperimentDesignPlan:
    """Cross-language transport validator for a deterministic design plan."""

    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != EXPERIMENT_DESIGN_FEATURE_ID:
            raise ResearchContractError("experiment design feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        allocations = self.payload.get("allocations")
        total = self.payload.get("total_units")
        if not isinstance(allocations, list) or not allocations or not isinstance(total, int):
            raise ResearchContractError("experiment design allocations are incomplete")
        if sum(int(item.get("units", 0)) for item in allocations) != total:
            raise ResearchContractError("experiment design allocation total is inconsistent")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("experiment design artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class ProtocolSimulationReport:
    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != PROTOCOL_SIMULATION_FEATURE_ID:
            raise ResearchContractError("protocol simulation feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        results = self.payload.get("results")
        if not isinstance(results, list) or not results:
            raise ResearchContractError("protocol simulation results are incomplete")
        allowed = {"passed", "failed_closed", "requires_approval"}
        if any(item.get("status") not in allowed for item in results):
            raise ResearchContractError("protocol simulation status is unknown")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("protocol simulation artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class ReplicationReport:
    """Transport validator for a replication disposition and negative-result ledger."""

    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != REPLICATION_FEATURE_ID:
            raise ResearchContractError("replication feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        summary = self.payload.get("summary")
        if not isinstance(summary, Mapping) or int(summary.get("total_observations", 0)) <= 0:
            raise ResearchContractError("replication summary is incomplete")
        if summary.get("disposition") not in {
            "replicated",
            "partially_replicated",
            "contradicted",
            "null_result",
            "insufficient_evidence",
        }:
            raise ResearchContractError("replication disposition is unknown")
        if not isinstance(summary.get("reasons"), list) or not summary["reasons"]:
            raise ResearchContractError("replication reasons are required")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("replication artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class MultimodalReplicationReport:
    """Transport validator for comparability-gated multimodal replication evidence."""

    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != MULTIMODAL_REPLICATION_FEATURE_ID:
            raise ResearchContractError("multimodal replication feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        if not str(self.payload.get("capability_id", "")).strip() or not str(self.payload.get("claim", "")).strip():
            raise ResearchContractError("multimodal replication identity is incomplete")
        required = self.payload.get("required_modalities")
        studies = self.payload.get("studies")
        summary = self.payload.get("summary")
        if not isinstance(required, list) or not required or not isinstance(studies, list) or not studies or not isinstance(summary, Mapping):
            raise ResearchContractError("multimodal replication evidence set is incomplete")
        if summary.get("disposition") not in {"replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence"}:
            raise ResearchContractError("multimodal replication disposition is unknown")
        if int(summary.get("total_observations", 0)) != len(studies) or not isinstance(summary.get("reasons"), list) or not summary["reasons"]:
            raise ResearchContractError("multimodal replication summary is inconsistent")
        for study in studies:
            if not isinstance(study, Mapping) or not str(study.get("study_id", "")).strip() or not isinstance(study.get("reasons"), list):
                raise ResearchContractError("multimodal study comparability record is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("multimodal replication artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class QualityControlReceipt:
    """Transport validator for modality quality gates and explicit unknown outcomes."""

    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != QUALITY_CONTROL_FEATURE_ID:
            raise ResearchContractError("quality-control feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        summary = self.payload.get("summary")
        if not isinstance(summary, Mapping) or not isinstance(summary.get("reasons"), list) or not summary["reasons"]:
            raise ResearchContractError("quality-control summary is incomplete")
        if summary.get("disposition") not in {"pass", "pass_with_warnings", "blocked", "unknown"}:
            raise ResearchContractError("quality-control disposition is unknown")
        if self.payload.get("raw_data_local") is False:
            raise ResearchContractError("raw research data must remain local")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("quality-control artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class QualityDriftReceipt:
    """Transport validator for baseline-relative continual-ingestion QC drift."""

    dataset_id: str
    modality: str
    request_digest: str
    summary: Mapping[str, Any]
    metrics: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool
    feature_id: str = QUALITY_DRIFT_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != QUALITY_DRIFT_FEATURE_ID:
            raise ResearchContractError("quality drift schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.dataset_id.strip() or not self.modality.strip():
            raise ResearchContractError("quality drift identity, locality, or boundary is invalid")
        if self.summary.get("disposition") not in {"stable", "drifted", "unknown", "blocked"}:
            raise ResearchContractError("quality drift disposition is unknown")
        if not self.metrics or not isinstance(self.summary.get("reasons"), list) or not self.summary["reasons"]:
            raise ResearchContractError("quality drift metrics and reasons are incomplete")
        if len(self.metrics) != int(self.summary.get("stable", 0)) + int(self.summary.get("drifted", 0)) + int(self.summary.get("unknown", 0)):
            raise ResearchContractError("quality drift metric counts are inconsistent")
        if not isinstance(self.request_digest, str) or len(self.request_digest) != 64 or any(char not in "0123456789abcdef" for char in self.request_digest):
            raise ResearchContractError("quality drift request digest is invalid")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("quality drift artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "dataset_id": self.dataset_id,
            "modality": self.modality,
            "request_digest": self.request_digest,
            "summary": dict(self.summary),
            "metrics": [dict(metric) for metric in self.metrics],
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class DesignFrontierReceipt:
    """Transport validator for scenario-replayed power-aware experiment designs."""

    study_id: str
    feasible_scenarios: int
    blocked_scenarios: int
    scenarios: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = DESIGN_FRONTIER_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != DESIGN_FRONTIER_FEATURE_ID:
            raise ResearchContractError("design frontier schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.study_id.strip() or not self.scenarios:
            raise ResearchContractError("design frontier identity or boundary is invalid")
        if self.feasible_scenarios < 0 or self.blocked_scenarios < 0 or self.feasible_scenarios + self.blocked_scenarios != len(self.scenarios):
            raise ResearchContractError("design frontier scenario counts are inconsistent")
        if any(not scenario.get("scenario_id") or scenario.get("disposition") not in {"feasible", "blocked"} or not scenario.get("reasons") for scenario in self.scenarios):
            raise ResearchContractError("design frontier scenario record is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("design frontier artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "study_id": self.study_id,
            "feasible_scenarios": self.feasible_scenarios,
            "blocked_scenarios": self.blocked_scenarios,
            "scenarios": [dict(scenario) for scenario in self.scenarios],
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class BatchAdmissionReceipt:
    """Transport validator for one-grant multi-action autonomy admission."""

    actor: str
    total_actions: int
    allowed_actions: int
    approval_actions: int
    denied_actions: int
    actions: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = AUTONOMY_BATCH_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != AUTONOMY_BATCH_FEATURE_ID:
            raise ResearchContractError("autonomy batch schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.actor.strip() or self.total_actions <= 0 or self.total_actions != len(self.actions):
            raise ResearchContractError("autonomy batch identity or boundary is invalid")
        if self.allowed_actions < 0 or self.approval_actions < 0 or self.denied_actions < 0 or self.allowed_actions + self.approval_actions + self.denied_actions != self.total_actions:
            raise ResearchContractError("autonomy batch counts are inconsistent")
        if any(not action.get("action_id") or action.get("decision") not in {"allowed", "approval_required", "denied"} or not action.get("reasons") for action in self.actions):
            raise ResearchContractError("autonomy batch action record is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("autonomy batch artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "actor": self.actor,
            "total_actions": self.total_actions,
            "allowed_actions": self.allowed_actions,
            "approval_actions": self.approval_actions,
            "denied_actions": self.denied_actions,
            "actions": [dict(action) for action in self.actions],
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class WorkflowBatchReceipt:
    """Transport validator for high-throughput workflow execution ledgers."""

    total_workflows: int
    succeeded_workflows: int
    dry_run_workflows: int
    blocked_workflows: int
    entries: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = WORKFLOW_BATCH_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != WORKFLOW_BATCH_FEATURE_ID:
            raise ResearchContractError("workflow batch schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or self.total_workflows <= 0 or self.total_workflows != len(self.entries):
            raise ResearchContractError("workflow batch identity or boundary is invalid")
        if self.succeeded_workflows < 0 or self.dry_run_workflows < 0 or self.blocked_workflows < 0 or self.succeeded_workflows + self.dry_run_workflows + self.blocked_workflows != self.total_workflows:
            raise ResearchContractError("workflow batch counts are inconsistent")
        if any(not entry.get("workflow_id") or entry.get("disposition") not in {"succeeded", "dry_run", "blocked"} or not entry.get("reasons") for entry in self.entries):
            raise ResearchContractError("workflow batch entry is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("workflow batch artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "total_workflows": self.total_workflows,
            "succeeded_workflows": self.succeeded_workflows,
            "dry_run_workflows": self.dry_run_workflows,
            "blocked_workflows": self.blocked_workflows,
            "entries": [dict(entry) for entry in self.entries],
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ResearchReleaseBatchReceipt:
    """Transport validator for high-throughput signed research-release publication."""

    total_releases: int
    published_releases: int
    blocked_releases: int
    entries: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = RESEARCH_RELEASE_BATCH_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RESEARCH_RELEASE_BATCH_FEATURE_ID:
            raise ResearchContractError("research-release batch schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or self.total_releases <= 0 or self.total_releases != len(self.entries):
            raise ResearchContractError("research-release batch identity or boundary is invalid")
        if self.published_releases < 0 or self.blocked_releases < 0 or self.published_releases + self.blocked_releases != self.total_releases:
            raise ResearchContractError("research-release batch counts are inconsistent")
        if any(not entry.get("release_id") or entry.get("disposition") not in {"published", "blocked"} or not entry.get("reasons") or (entry.get("disposition") == "published" and not entry.get("release_digest")) for entry in self.entries):
            raise ResearchContractError("research-release batch entry is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("research-release batch artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "total_releases": self.total_releases,
            "published_releases": self.published_releases,
            "blocked_releases": self.blocked_releases,
            "entries": [dict(entry) for entry in self.entries],
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class FederatedEvaluationReceipt:
    """Transport validator for omission-aware multi-site EvaluationCard consensus."""

    capability_id: str
    benchmark_world: str
    minimum_sites: int
    total_sites: int
    agreeing_sites: int
    contradictory_sites: int
    blocked_sites: int
    disposition: str
    entries: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_EVALUATION_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_EVALUATION_FEATURE_ID:
            raise ResearchContractError("federated evaluation schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.capability_id.strip() or not self.benchmark_world.strip() or self.minimum_sites <= 0 or self.total_sites <= 0 or self.total_sites != len(self.entries):
            raise ResearchContractError("federated evaluation identity or boundary is invalid")
        if self.agreeing_sites < 0 or self.contradictory_sites < 0 or self.blocked_sites < 0 or self.agreeing_sites + self.contradictory_sites + self.blocked_sites != self.total_sites:
            raise ResearchContractError("federated evaluation counts are inconsistent")
        if self.disposition not in {"consensus", "partial", "contradicted", "blocked"}:
            raise ResearchContractError("federated evaluation disposition is unknown")
        if any(not entry.get("site_id") or entry.get("disposition") not in {"accepted", "contradictory", "blocked"} or not entry.get("reasons") or (entry.get("disposition") == "accepted" and not entry.get("card_digest")) for entry in self.entries):
            raise ResearchContractError("federated evaluation site entry is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("federated evaluation artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "capability_id": self.capability_id,
            "benchmark_world": self.benchmark_world,
            "minimum_sites": self.minimum_sites,
            "total_sites": self.total_sites,
            "agreeing_sites": self.agreeing_sites,
            "contradictory_sites": self.contradictory_sites,
            "blocked_sites": self.blocked_sites,
            "disposition": self.disposition,
            "entries": [dict(entry) for entry in self.entries],
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class QualifiedResourceSet:
    """Transport validator for deterministic, omission-aware resource discovery."""

    need_id: str
    requester: str
    disposition: str
    considered_candidates: int
    qualified_count: int
    resources: tuple[Mapping[str, Any], ...]
    omissions: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RESOURCE_WORKBENCH_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RESOURCE_WORKBENCH_FEATURE_ID:
            raise ResearchContractError("resource workbench schema or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.need_id.strip() or not self.requester.strip():
            raise ResearchContractError("resource workbench identity or boundary is invalid")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("resource discovery disposition is unknown")
        if self.considered_candidates <= 0 or self.qualified_count < 0 or self.qualified_count != len(self.resources) or not self.reasons:
            raise ResearchContractError("resource discovery counts or reasons are incomplete")
        if any(not item.get("resource_id") or not item.get("origin") or not item.get("rank") or not item.get("reasons") for item in self.resources):
            raise ResearchContractError("qualified resource entry is incomplete")
        if any(not item.get("resource_id") or not item.get("reason") for item in self.omissions):
            raise ResearchContractError("resource omission entry is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("resource workbench artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "need_id": self.need_id,
            "requester": self.requester,
            "disposition": self.disposition,
            "considered_candidates": self.considered_candidates,
            "qualified_count": self.qualified_count,
            "resources": [dict(item) for item in self.resources],
            "omissions": [dict(item) for item in self.omissions],
            "reasons": list(self.reasons),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ResourceDiscoveryContractReceipt:
    """Compatibility envelope for the MCP resource-discovery contract."""

    request_id: str
    requested_by: str
    compatibility_profile: str
    result: Mapping[str, Any]
    migration_notes: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID
    contract_version: str = RESOURCE_DISCOVERY_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID or self.contract_version != RESOURCE_DISCOVERY_CONTRACT_VERSION:
            raise ResearchContractError("resource discovery contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.requested_by.strip() or not self.compatibility_profile.strip() or len(self.compatibility_profile.encode("utf-8")) > 256 or not self.migration_notes:
            raise ResearchContractError("resource discovery contract identity, compatibility, migration, or boundary is invalid")
        if self.result.get("feature_id") != RESOURCE_WORKBENCH_FEATURE_ID or self.result.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("resource discovery contract result is not the qualified-resource contract")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("resource discovery contract artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "requested_by": self.requested_by,
            "compatibility_profile": self.compatibility_profile,
            "result": dict(self.result),
            "migration_notes": list(self.migration_notes),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class SignedResearchObjectReceipt:
    """Transport validator for governance-owned signed research-object metadata."""

    run_id: str
    release_id: str
    origin: str
    purpose: str
    artifact_ids: tuple[str, ...]
    evidence_receipt_ids: tuple[str, ...]
    release_digest: str
    signer_public_key_hex: str
    signer_signature_hex: str
    migration_notes: tuple[str, ...]
    omissions: tuple[str, ...]
    raw_data_local: bool
    artifact: Mapping[str, Any]
    feature_id: str = GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID
    contract_version: str = GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID or self.contract_version != GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION:
            raise ResearchContractError("governance research-release schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not all(value.strip() for value in (self.run_id, self.release_id, self.origin, self.purpose)):
            raise ResearchContractError("signed research object identity or locality is invalid")
        if not self.artifact_ids or len(set(self.artifact_ids)) != len(self.artifact_ids) or not self.evidence_receipt_ids or len(set(self.evidence_receipt_ids)) != len(self.evidence_receipt_ids) or not self.migration_notes:
            raise ResearchContractError("signed research object provenance or migration is incomplete")
        if not isinstance(self.release_digest, str) or len(self.release_digest) != 64 or any(char not in "0123456789abcdef" for char in self.release_digest):
            raise ResearchContractError("signed research object release digest is invalid")
        if len(self.signer_public_key_hex) != 64 or len(self.signer_signature_hex) != 128 or any(char not in "0123456789abcdef" for char in self.signer_public_key_hex + self.signer_signature_hex):
            raise ResearchContractError("signed research object signature material is invalid")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("signed research object artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "run_id": self.run_id,
            "release_id": self.release_id,
            "origin": self.origin,
            "purpose": self.purpose,
            "artifact_ids": list(self.artifact_ids),
            "evidence_receipt_ids": list(self.evidence_receipt_ids),
            "release_digest": self.release_digest,
            "signer_public_key_hex": self.signer_public_key_hex,
            "signer_signature_hex": self.signer_signature_hex,
            "migration_notes": list(self.migration_notes),
            "omissions": list(self.omissions),
            "raw_data_local": self.raw_data_local,
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ReleaseHarnessReceipt:
    """Transport validator for omission-aware signed-object admission checks."""

    request_id: str
    object_digest: str
    disposition: str
    checks: tuple[Mapping[str, Any], ...]
    omissions: tuple[str, ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RELEASE_HARNESS_FEATURE_ID
    contract_version: str = RELEASE_HARNESS_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RELEASE_HARNESS_FEATURE_ID or self.contract_version != RELEASE_HARNESS_CONTRACT_VERSION:
            raise ResearchContractError("release harness schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or self.disposition not in {"passed", "blocked", "unknown"} or not self.checks or not self.reasons:
            raise ResearchContractError("release harness identity, disposition, checks, or boundary is invalid")
        if not isinstance(self.object_digest, str) or len(self.object_digest) != 64 or any(char not in "0123456789abcdef" for char in self.object_digest):
            raise ResearchContractError("release harness object digest is invalid")
        if any(not check.get("check_id") or check.get("disposition") not in {"passed", "blocked", "unknown"} or not check.get("reason") for check in self.checks):
            raise ResearchContractError("release harness check is incomplete")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("release harness artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "object_digest": self.object_digest,
            "disposition": self.disposition,
            "checks": [dict(check) for check in self.checks],
            "omissions": list(self.omissions),
            "reasons": list(self.reasons),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ProtocolAssuranceReceipt:
    """Transport validator for policy-gated protocol simulation admission."""

    request_id: str
    protocol_id: str
    disposition: str
    total_cells: int
    passed_cells: int
    blocked_cells: int
    unknown_cells: int
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    simulation_digest: str
    artifact: Mapping[str, Any]
    feature_id: str = PROTOCOL_ASSURANCE_FEATURE_ID
    contract_version: str = PROTOCOL_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != PROTOCOL_ASSURANCE_FEATURE_ID or self.contract_version != PROTOCOL_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("protocol assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.protocol_id.strip():
            raise ResearchContractError("protocol assurance identity or boundary is invalid")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("protocol assurance disposition or checks are incomplete")
        counts = (self.total_cells, self.passed_cells, self.blocked_cells, self.unknown_cells)
        if self.total_cells <= 0 or any(value < 0 for value in counts) or self.total_cells != self.passed_cells + self.blocked_cells + self.unknown_cells:
            raise ResearchContractError("protocol assurance cell counts do not partition")
        for digest in (self.simulation_digest, self.artifact.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("protocol assurance digest is not a canonical sha256")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "protocol_id": self.protocol_id,
            "disposition": self.disposition,
            "total_cells": self.total_cells,
            "passed_cells": self.passed_cells,
            "blocked_cells": self.blocked_cells,
            "unknown_cells": self.unknown_cells,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "simulation_digest": self.simulation_digest,
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class FederatedMultimodalAssuranceReceipt:
    """Transport validator for locality-preserving federated multimodal admission."""

    request_id: str
    federation_id: str
    benchmark_id: str
    institution_ids: tuple[str, ...]
    disposition: str
    harmonized_digest: str
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool = True
    feature_id: str = FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID
    contract_version: str = FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID or self.contract_version != FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("federated multimodal assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.benchmark_id.strip():
            raise ResearchContractError("federated multimodal assurance identity or locality is invalid")
        if len(self.institution_ids) < 2 or any(not institution.strip() for institution in self.institution_ids) or len(set(self.institution_ids)) != len(self.institution_ids):
            raise ResearchContractError("federated multimodal institution set is incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("federated multimodal disposition or checks are incomplete")
        for digest in (self.harmonized_digest, self.artifact.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("federated multimodal digest is not a canonical sha256")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "benchmark_id": self.benchmark_id,
            "institution_ids": list(self.institution_ids),
            "disposition": self.disposition,
            "harmonized_digest": self.harmonized_digest,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class FederatedKnowledgeGatewayReceipt:
    """Transport validator for manifest-only federated knowledge-store admission."""

    request_id: str
    federation_id: str
    interoperability_profile: str
    institution_ids: tuple[str, ...]
    disposition: str
    manifest_digest: str
    permitted_tags: tuple[str, ...]
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool = True
    feature_id: str = FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID
    contract_version: str = FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID or self.contract_version != FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION:
            raise ResearchContractError("federated knowledge gateway schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.interoperability_profile.strip():
            raise ResearchContractError("federated knowledge gateway identity or locality is invalid")
        if len(self.institution_ids) < 2 or any(not institution.strip() for institution in self.institution_ids) or len(set(self.institution_ids)) != len(self.institution_ids):
            raise ResearchContractError("federated knowledge institution set is incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("federated knowledge disposition or checks are incomplete")
        for digest in (self.manifest_digest, self.artifact.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("federated knowledge digest is not a canonical sha256")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "interoperability_profile": self.interoperability_profile,
            "institution_ids": list(self.institution_ids),
            "disposition": self.disposition,
            "manifest_digest": self.manifest_digest,
            "permitted_tags": list(self.permitted_tags),
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class FederatedLensAssuranceReceipt:
    """Transport validator for omission-aware federated lens-report admission."""

    request_id: str
    federation_id: str
    institution_ids: tuple[str, ...]
    required_lens_ids: tuple[str, ...]
    report_digests: tuple[str, ...]
    absent_lens_ids: tuple[str, ...]
    disposition: str
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_LENS_ASSURANCE_FEATURE_ID
    contract_version: str = FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_LENS_ASSURANCE_FEATURE_ID or self.contract_version != FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("federated lens assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip():
            raise ResearchContractError("federated lens assurance identity or boundary is invalid")
        if len(self.institution_ids) < 2 or any(not institution.strip() for institution in self.institution_ids) or tuple(sorted(set(self.institution_ids))) != self.institution_ids:
            raise ResearchContractError("federated lens institution ordering is invalid")
        if not self.required_lens_ids or self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("federated lens required set, disposition, or checks are incomplete")
        for digest in self.report_digests:
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("federated lens report digest is not a canonical sha256")
        artifact_digest = self.artifact.get("content_hash")
        if not isinstance(artifact_digest, str) or len(artifact_digest) != 64 or any(char not in "0123456789abcdef" for char in artifact_digest):
            raise ResearchContractError("federated lens artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "institution_ids": list(self.institution_ids),
            "required_lens_ids": list(self.required_lens_ids),
            "report_digests": list(self.report_digests),
            "absent_lens_ids": list(self.absent_lens_ids),
            "disposition": self.disposition,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class LabSemanticParityReceipt:
    """Transport validator for federated protocol semantic-parity admission."""

    request_id: str
    federation_id: str
    protocol_id: str
    benchmark_id: str
    institution_ids: tuple[str, ...]
    disposition: str
    semantic_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = SEMANTIC_PARITY_FEATURE_ID
    contract_version: str = SEMANTIC_PARITY_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != SEMANTIC_PARITY_FEATURE_ID or self.contract_version != SEMANTIC_PARITY_CONTRACT_VERSION:
            raise ResearchContractError("lab semantic parity schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.protocol_id.strip() or not self.benchmark_id.strip():
            raise ResearchContractError("lab semantic parity identity or boundary is invalid")
        if len(self.institution_ids) < 2 or tuple(sorted(set(self.institution_ids))) != self.institution_ids or any(not institution.strip() for institution in self.institution_ids):
            raise ResearchContractError("lab semantic parity institution ordering is invalid")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("lab semantic parity disposition or checks is incomplete")
        if self.semantic_digest is not None and (len(self.semantic_digest) != 64 or any(char not in "0123456789abcdef" for char in self.semantic_digest)):
            raise ResearchContractError("lab semantic parity semantic digest is invalid")
        artifact_digest = self.artifact.get("content_hash")
        if not isinstance(artifact_digest, str) or len(artifact_digest) != 64 or any(char not in "0123456789abcdef" for char in artifact_digest):
            raise ResearchContractError("lab semantic parity artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "protocol_id": self.protocol_id,
            "benchmark_id": self.benchmark_id,
            "institution_ids": list(self.institution_ids),
            "disposition": self.disposition,
            "semantic_digest": self.semantic_digest,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class FederatedRetrievalAssuranceReceipt:
    """Transport validator for omission-aware federated retrieval admission."""

    request_id: str
    federation_id: str
    query_id: str
    returned_source_ids: tuple[str, ...]
    disposition: str
    evidence_receipt_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID
    contract_version: str = FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID or self.contract_version != FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("federated retrieval assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.query_id.strip() or not self.checks:
            raise ResearchContractError("federated retrieval identity, boundary, or checks are incomplete")
        if tuple(sorted(set(self.returned_source_ids))) != self.returned_source_ids:
            raise ResearchContractError("federated retrieval source ordering is invalid")
        if self.disposition not in {"passed", "blocked", "unknown"}:
            raise ResearchContractError("federated retrieval disposition is unknown")
        if self.evidence_receipt_digest is not None and (len(self.evidence_receipt_digest) != 64 or any(char not in "0123456789abcdef" for char in self.evidence_receipt_digest)):
            raise ResearchContractError("federated retrieval evidence digest is invalid")
        artifact_digest = self.artifact.get("content_hash")
        if not isinstance(artifact_digest, str) or len(artifact_digest) != 64 or any(char not in "0123456789abcdef" for char in artifact_digest):
            raise ResearchContractError("federated retrieval artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "query_id": self.query_id,
            "returned_source_ids": list(self.returned_source_ids),
            "disposition": self.disposition,
            "evidence_receipt_digest": self.evidence_receipt_digest,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ResearchContextReceipt:
    """Transport validator for omission-certified Decision Section compilation."""

    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != RESEARCH_CONTEXT_FEATURE_ID:
            raise ResearchContractError("research-context feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        if self.payload.get("protected_closure_satisfied") is not True:
            raise ResearchContractError("protected closure is not satisfied")
        if not isinstance(self.payload.get("supports_sufficiency_claim"), bool):
            raise ResearchContractError("sufficiency state is missing")
        if not isinstance(self.payload.get("unresolved_obligations"), int) or self.payload["unresolved_obligations"] < 0:
            raise ResearchContractError("unresolved-obligation count is invalid")
        for key in ("section_digest", "certificate_digest"):
            digest = self.payload.get(key)
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError(f"{key} is not a canonical sha256")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("research-context artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class ReplayAuditReceipt:
    """Transport validator for fail-closed semantic replay comparison."""

    payload: Mapping[str, Any]
    artifact: Mapping[str, Any]

    def validate(self) -> None:
        if self.payload.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.payload.get("feature_id") != REPLAY_AUDIT_FEATURE_ID:
            raise ResearchContractError("replay-audit feature mismatch")
        if self.payload.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        if self.payload.get("status") not in {"equivalent", "diverged", "invalid"}:
            raise ResearchContractError("replay-audit status is unknown")
        if not isinstance(self.payload.get("reasons"), list) or not self.payload["reasons"]:
            raise ResearchContractError("replay-audit reasons are required")
        for key in ("baseline_digest", "candidate_digest"):
            digest = self.payload.get(key)
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError(f"{key} is not a canonical sha256")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ResearchContractError("replay-audit artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"payload": dict(self.payload), "artifact": dict(self.artifact)})


@dataclass(frozen=True)
class WorkflowExecutionReceipt:
    """Transport validator for deterministic typed workflow execution receipts."""

    workflow_id: str
    mode: str
    status: str
    ordered_nodes: tuple[str, ...]
    completed_nodes: tuple[str, ...]
    run: Mapping[str, Any]
    run_digest: str
    remaining_budget: Mapping[str, float]
    artifact: Mapping[str, Any]
    reasons: tuple[str, ...]
    feature_id: str = WORKFLOW_EXECUTION_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != WORKFLOW_EXECUTION_FEATURE_ID:
            raise ResearchContractError("workflow-execution feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        if not self.workflow_id.strip() or self.mode not in {"dry_run", "execute"}:
            raise ResearchContractError("workflow execution identity or mode is invalid")
        if self.status not in {"dry_run", "succeeded"} or not self.reasons:
            raise ResearchContractError("workflow execution status and reasons are required")
        if not self.ordered_nodes or any(not node.strip() for node in self.ordered_nodes):
            raise ResearchContractError("workflow execution order is incomplete")
        if any(node not in self.ordered_nodes for node in self.completed_nodes):
            raise ResearchContractError("completed workflow node is outside the ordered plan")
        if self.run.get("workflow_id") != self.workflow_id:
            raise ResearchContractError("workflow run identity does not match receipt")
        expected_run_status = "planned" if self.status == "dry_run" else "succeeded"
        if self.run.get("status") != expected_run_status:
            raise ResearchContractError("workflow run status does not match receipt status")
        if not isinstance(self.remaining_budget, Mapping) or any(
            not isinstance(value, (int, float)) or value < 0 for value in self.remaining_budget.values()
        ):
            raise ResearchContractError("workflow remaining budget is invalid")
        if not isinstance(self.run_digest, str) or len(self.run_digest) != 64 or any(
            char not in "0123456789abcdef" for char in self.run_digest
        ):
            raise ResearchContractError("workflow run digest is not a canonical sha256")
        digest = self.artifact.get("content_hash")
        if not isinstance(digest, str) or len(digest) != 64 or any(
            char not in "0123456789abcdef" for char in digest
        ):
            raise ResearchContractError("workflow execution artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "workflow_id": self.workflow_id,
                "mode": self.mode,
                "status": self.status,
                "ordered_nodes": list(self.ordered_nodes),
                "completed_nodes": list(self.completed_nodes),
                "run": dict(self.run),
                "run_digest": self.run_digest,
                "remaining_budget": dict(self.remaining_budget),
                "artifact": dict(self.artifact),
                "reasons": list(self.reasons),
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class EvaluationCardReceipt:
    """Transport validator for cost-normalized, uncertainty-bearing EvaluationCards."""

    card: Mapping[str, Any]
    card_digest: str
    observations_digest: str
    baseline_counts: Mapping[str, int]
    omissions: tuple[str, ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = EVALUATION_OBSERVABILITY_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != EVALUATION_OBSERVABILITY_FEATURE_ID:
            raise ResearchContractError("evaluation-observability feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        if not self.card.get("capability_id") or self.card.get("benchmark_world") is None:
            raise ResearchContractError("evaluation card identity is incomplete")
        if self.card.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("evaluation card schema mismatch")
        if self.card.get("release_verdict") not in {"pass", "conditional", "blocked", "not_evaluated"}:
            raise ResearchContractError("evaluation card release verdict is unknown")
        if not self.card.get("baselines") or not self.card.get("metrics") or not self.card.get("uncertainty"):
            raise ResearchContractError("evaluation card evidence fields are incomplete")
        if not self.reasons or not self.baseline_counts:
            raise ResearchContractError("evaluation receipt needs baseline counts and reasons")
        if self.card.get("release_verdict") == "pass" and self.omissions:
            raise ResearchContractError("a passing evaluation card cannot hide baseline omissions")
        for digest in (self.card_digest, self.observations_digest, self.artifact.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("evaluation receipt digest is not a canonical sha256")
        if any(not isinstance(value, int) or value < 0 for value in self.baseline_counts.values()):
            raise ResearchContractError("evaluation baseline count is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "card": dict(self.card),
                "card_digest": self.card_digest,
                "observations_digest": self.observations_digest,
                "baseline_counts": dict(self.baseline_counts),
                "omissions": list(self.omissions),
                "reasons": list(self.reasons),
                "artifact": dict(self.artifact),
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class ResearchReleaseReceipt:
    """Transport validator for signed, local-first research-object publication."""

    release_id: str
    research_object: Mapping[str, Any]
    release_digest: str
    omissions: tuple[str, ...]
    reasons: tuple[str, ...]
    feature_id: str = RESEARCH_RELEASE_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != RESEARCH_RELEASE_FEATURE_ID:
            raise ResearchContractError("research-release feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.release_id.strip():
            raise ResearchContractError("research release identity or boundary is invalid")
        if self.research_object.get("release_id") != self.release_id:
            raise ResearchContractError("research object release identity does not match receipt")
        artifact_ids = self.research_object.get("artifact_ids")
        evidence_ids = self.research_object.get("evidence_receipt_ids")
        if not isinstance(artifact_ids, list) or not artifact_ids or len(set(artifact_ids)) != len(artifact_ids):
            raise ResearchContractError("research object artifact ids are incomplete or duplicated")
        if not isinstance(evidence_ids, list) or not evidence_ids or len(set(evidence_ids)) != len(evidence_ids):
            raise ResearchContractError("research object evidence ids are incomplete or duplicated")
        federation = self.research_object.get("federation")
        envelope = federation.get("envelope") if isinstance(federation, Mapping) else None
        if not isinstance(envelope, Mapping) or envelope.get("raw_data_local") is not True:
            raise ResearchContractError("research release must keep raw data local")
        if not envelope.get("signature") or not envelope.get("localization_statement"):
            raise ResearchContractError("research release signature and localization are required")
        export = envelope.get("export")
        if not isinstance(export, Mapping) or not export.get("provenance"):
            raise ResearchContractError("research release provenance is incomplete")
        if not self.reasons:
            raise ResearchContractError("research release reasons are required")
        for digest in (self.release_digest, export.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("research release digest is not a canonical sha256")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "release_id": self.release_id,
                "research_object": dict(self.research_object),
                "release_digest": self.release_digest,
                "omissions": list(self.omissions),
                "reasons": list(self.reasons),
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class InstrumentPreflightReceipt:
    """Transport validator for approval-gated, no-hardware instrument preflight."""

    run_id: str
    study_id: str
    decision: str
    ordered_actions: tuple[str, ...]
    action_digests: Mapping[str, str]
    remaining_budget: Mapping[str, float]
    omissions: tuple[str, ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = INSTRUMENT_PREFLIGHT_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != INSTRUMENT_PREFLIGHT_FEATURE_ID:
            raise ResearchContractError("instrument-preflight feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.run_id.strip() or not self.study_id.strip():
            raise ResearchContractError("instrument preflight identity or boundary is invalid")
        if self.decision not in {"ready", "blocked", "requires_approval", "emergency_stop"}:
            raise ResearchContractError("instrument preflight decision is unknown")
        if not self.ordered_actions or not self.action_digests or not self.reasons:
            raise ResearchContractError("instrument preflight evidence is incomplete")
        if len(set(self.ordered_actions)) != len(self.ordered_actions) or any(action not in self.action_digests for action in self.ordered_actions):
            raise ResearchContractError("instrument action ordering or digest coverage is invalid")
        for digest in self.action_digests.values():
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("instrument action digest is not a canonical sha256")
        if any(not isinstance(value, (int, float)) or not float(value) == float(value) or value < 0 for value in self.remaining_budget.values()):
            raise ResearchContractError("instrument remaining budget is invalid")
        if self.artifact.get("content_hash") is None or not isinstance(self.artifact.get("content_hash"), str) or len(self.artifact["content_hash"]) != 64:
            raise ResearchContractError("instrument preflight artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "run_id": self.run_id,
                "study_id": self.study_id,
                "decision": self.decision,
                "ordered_actions": list(self.ordered_actions),
                "action_digests": dict(self.action_digests),
                "remaining_budget": dict(self.remaining_budget),
                "omissions": list(self.omissions),
                "reasons": list(self.reasons),
                "artifact": dict(self.artifact),
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class HarmonizedResearchObject:
    """Transport validator for manifest-level multimodal harmonization."""

    study_id: str
    reference_schema: str
    decision: str
    modality_order: tuple[str, ...]
    alignment: Mapping[str, Sequence[str]]
    omitted_modalities: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool
    feature_id: str = MULTIMODAL_HARMONIZATION_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != MULTIMODAL_HARMONIZATION_FEATURE_ID:
            raise ResearchContractError("multimodal harmonization feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local:
            raise ResearchContractError("multimodal raw data must remain local")
        if not self.study_id.strip() or not self.reference_schema.strip():
            raise ResearchContractError("multimodal research object identity is incomplete")
        if self.decision not in {"comparable", "partial", "blocked"}:
            raise ResearchContractError("multimodal decision is unknown")
        if not self.modality_order or not self.alignment or not self.reasons:
            raise ResearchContractError("multimodal alignment and reasons are incomplete")
        if any(modality not in self.alignment for modality in self.modality_order):
            raise ResearchContractError("multimodal alignment omits a modality projection")
        if not isinstance(self.artifact.get("content_hash"), str) or len(self.artifact["content_hash"]) != 64:
            raise ResearchContractError("multimodal artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "study_id": self.study_id,
                "reference_schema": self.reference_schema,
                "decision": self.decision,
                "modality_order": list(self.modality_order),
                "alignment": {key: list(value) for key, value in self.alignment.items()},
                "omitted_modalities": list(self.omitted_modalities),
                "semantic_loss": list(self.semantic_loss),
                "reasons": list(self.reasons),
                "artifact": dict(self.artifact),
                "raw_data_local": self.raw_data_local,
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class QualifiedAnalysisResult:
    """Transport validator for omission-aware declared-analysis qualification."""

    question_id: str
    estimand: str
    verdict: str
    selected_candidate: str | None
    candidate_order: tuple[str, ...]
    uncertainty: tuple[str, ...]
    omissions: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool
    feature_id: str = ANALYSIS_QUALIFICATION_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != ANALYSIS_QUALIFICATION_FEATURE_ID:
            raise ResearchContractError("analysis qualification feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local:
            raise ResearchContractError("qualified analysis must retain raw data locally")
        if not self.question_id.strip() or not self.estimand.strip():
            raise ResearchContractError("qualified analysis identity is incomplete")
        if self.verdict not in {"qualified", "conditional", "blocked"}:
            raise ResearchContractError("qualified analysis verdict is unknown")
        if not self.candidate_order or not self.reasons or not self.uncertainty:
            raise ResearchContractError("qualified analysis evidence is incomplete")
        if self.verdict == "qualified" and self.selected_candidate is None:
            raise ResearchContractError("qualified analysis needs a selected candidate")
        if not isinstance(self.artifact.get("content_hash"), str) or len(self.artifact["content_hash"]) != 64:
            raise ResearchContractError("qualified analysis artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "question_id": self.question_id,
                "estimand": self.estimand,
                "verdict": self.verdict,
                "selected_candidate": self.selected_candidate,
                "candidate_order": list(self.candidate_order),
                "uncertainty": list(self.uncertainty),
                "omissions": list(self.omissions),
                "negative_evidence": list(self.negative_evidence),
                "reasons": list(self.reasons),
                "artifact": dict(self.artifact),
                "raw_data_local": self.raw_data_local,
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class ProtocolMatrixReceipt:
    """Transport validator for bounded protocol robustness matrices."""

    protocol_id: str
    total_cells: int
    passed_cells: int
    failed_closed_cells: int
    approval_cells: int
    cells: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = PROTOCOL_MATRIX_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != PROTOCOL_MATRIX_FEATURE_ID:
            raise ResearchContractError("protocol matrix feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.protocol_id.strip():
            raise ResearchContractError("protocol matrix identity or boundary is invalid")
        if self.total_cells <= 0 or self.total_cells != len(self.cells):
            raise ResearchContractError("protocol matrix cell count is invalid")
        if any(value < 0 for value in (self.passed_cells, self.failed_closed_cells, self.approval_cells)):
            raise ResearchContractError("protocol matrix status count is invalid")
        if self.passed_cells + self.failed_closed_cells + self.approval_cells != self.total_cells:
            raise ResearchContractError("protocol matrix status counts do not partition cells")
        if not self.cells or any(not cell.get("cell_id") or not cell.get("reasons") for cell in self.cells):
            raise ResearchContractError("protocol matrix cells need ids and reasons")
        if any(cell.get("status") not in {"passed", "failed_closed", "requires_approval"} for cell in self.cells):
            raise ResearchContractError("protocol matrix cell status is unknown")
        for digest in (self.artifact.get("content_hash"),):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("protocol matrix artifact digest is not a canonical sha256")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "protocol_id": self.protocol_id,
                "total_cells": self.total_cells,
                "passed_cells": self.passed_cells,
                "failed_closed_cells": self.failed_closed_cells,
                "approval_cells": self.approval_cells,
                "cells": [dict(cell) for cell in self.cells],
                "artifact": dict(self.artifact),
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class RetrievalSourceUpdate:
    source_id: str
    version: str
    digest: str
    evidence_state: str
    stale: bool


@dataclass(frozen=True)
class FederatedContinualRetrievalReceipt:
    """Transport validator for continual federated evidence refreshes."""

    request_id: str
    federation_id: str
    query_id: str
    selected_source_ids: tuple[str, ...]
    stale_source_ids: tuple[str, ...]
    disposition: str
    prior_synthesis_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID
    contract_version: str = FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID:
            raise ResearchContractError("federated continual retrieval feature mismatch")
        if self.contract_version != FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION:
            raise ResearchContractError("federated continual retrieval contract mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research boundary mismatch")
        if not self.request_id.strip() or not self.federation_id.strip() or not self.query_id.strip():
            raise ResearchContractError("continual retrieval identity is incomplete")
        if not self.selected_source_ids or len(set(self.selected_source_ids)) != len(self.selected_source_ids):
            raise ResearchContractError("continual retrieval source identities are not unique")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("continual retrieval disposition and checks are required")
        if self.prior_synthesis_digest is not None and (
            len(self.prior_synthesis_digest) != 64
            or any(char not in "0123456789abcdef" for char in self.prior_synthesis_digest)
        ):
            raise ResearchContractError("continual retrieval prior digest is not canonical")
        if not isinstance(self.artifact.get("content_hash"), str) or len(self.artifact["content_hash"]) != 64:
            raise ResearchContractError("continual retrieval artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "feature_id": self.feature_id,
                "contract_version": self.contract_version,
                "request_id": self.request_id,
                "federation_id": self.federation_id,
                "query_id": self.query_id,
                "selected_source_ids": list(self.selected_source_ids),
                "stale_source_ids": list(self.stale_source_ids),
                "disposition": self.disposition,
                "prior_synthesis_digest": self.prior_synthesis_digest,
                "checks": list(self.checks),
                "omissions": list(self.omissions),
                "artifact": dict(self.artifact),
                "boundary": self.boundary,
            }
        )


@dataclass(frozen=True)
class ContextCompilationAssuranceReceipt:
    """Transport validator for omission-aware federated context certification."""

    request_id: str
    federation_id: str
    query_id: str
    resolved_context_ids: tuple[str, ...]
    disposition: str
    evidence_receipt_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID
    contract_version: str = CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID or self.contract_version != CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("context compilation assurance feature or contract mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.query_id.strip():
            raise ResearchContractError("context compilation assurance identity or boundary is invalid")
        if not self.resolved_context_ids or len(set(self.resolved_context_ids)) != len(self.resolved_context_ids):
            raise ResearchContractError("context compilation resolved identities are not unique")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("context compilation disposition and checks are required")
        if self.evidence_receipt_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.evidence_receipt_digest):
            raise ResearchContractError("context compilation evidence digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("context compilation artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "query_id": self.query_id,
            "resolved_context_ids": list(self.resolved_context_ids),
            "disposition": self.disposition,
            "evidence_receipt_digest": self.evidence_receipt_digest,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class KnowledgeRepresentationAssuranceReceipt:
    """Transport validator for omission-aware federated knowledge projections."""

    request_id: str
    federation_id: str
    query_id: str
    resolved_fact_ids: tuple[str, ...]
    disposition: str
    evidence_receipt_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID
    contract_version: str = KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION:
            raise ResearchContractError("unsupported research contract schema")
        if self.feature_id != KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID or self.contract_version != KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("knowledge representation assurance feature or contract mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.query_id.strip():
            raise ResearchContractError("knowledge representation assurance identity or boundary is invalid")
        if not self.resolved_fact_ids or len(set(self.resolved_fact_ids)) != len(self.resolved_fact_ids):
            raise ResearchContractError("knowledge representation fact identities are not unique")
        if self.disposition not in {"passed", "blocked", "unknown"} or not self.checks:
            raise ResearchContractError("knowledge representation disposition and checks are required")
        if self.evidence_receipt_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.evidence_receipt_digest):
            raise ResearchContractError("knowledge representation evidence digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("knowledge representation artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "contract_version": self.contract_version,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "query_id": self.query_id,
            "resolved_fact_ids": list(self.resolved_fact_ids),
            "disposition": self.disposition,
            "evidence_receipt_digest": self.evidence_receipt_digest,
            "checks": list(self.checks),
            "omissions": list(self.omissions),
            "artifact": dict(self.artifact),
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ResourceControlPlaneReceipt:
    request_id: str
    federation_id: str
    institution_ids: tuple[str, ...]
    qualified_resource_ids: tuple[str, ...]
    disposition: str
    qualification_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RESOURCE_CONTROL_PLANE_FEATURE_ID
    contract_version: str = RESOURCE_CONTROL_PLANE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RESOURCE_CONTROL_PLANE_FEATURE_ID or self.contract_version != RESOURCE_CONTROL_PLANE_CONTRACT_VERSION: raise ResearchContractError("resource control-plane schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or len(self.institution_ids) < 2: raise ResearchContractError("resource control-plane identity or boundary is invalid")
        if len(set(self.institution_ids)) != len(self.institution_ids) or list(self.institution_ids) != sorted(self.institution_ids): raise ResearchContractError("resource control-plane institution ordering is invalid")
        if not self.qualified_resource_ids or len(set(self.qualified_resource_ids)) != len(self.qualified_resource_ids) or self.disposition not in {"passed", "blocked", "unknown"} or not self.checks: raise ResearchContractError("resource control-plane qualification or checks are incomplete")
        if self.qualification_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.qualification_digest): raise ResearchContractError("resource control-plane digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]): raise ResearchContractError("resource control-plane artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "federation_id": self.federation_id, "institution_ids": list(self.institution_ids), "qualified_resource_ids": list(self.qualified_resource_ids), "disposition": self.disposition, "qualification_digest": self.qualification_digest, "checks": list(self.checks), "omissions": list(self.omissions), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class WeaveLangReleaseAssuranceReceipt:
    request_id: str
    run_id: str
    release_id: str
    disposition: str
    artifact_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID
    contract_version: str = WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID or self.contract_version != WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION: raise ResearchContractError("WeaveLang release assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.run_id.strip() or not self.release_id.strip() or not self.checks: raise ResearchContractError("WeaveLang release assurance identity or checks are incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"}: raise ResearchContractError("WeaveLang release assurance disposition is unknown")
        if self.artifact_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.artifact_digest): raise ResearchContractError("WeaveLang release artifact digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]): raise ResearchContractError("WeaveLang release receipt digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "run_id": self.run_id, "release_id": self.release_id, "disposition": self.disposition, "artifact_digest": self.artifact_digest, "checks": list(self.checks), "omissions": list(self.omissions), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class MechanismControlPlaneReceipt:
    request_id: str
    federation_id: str
    question_id: str
    admitted_candidate_ids: tuple[str, ...]
    disposition: str
    evidence_receipt_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MECHANISM_CONTROL_PLANE_FEATURE_ID
    contract_version: str = MECHANISM_CONTROL_PLANE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MECHANISM_CONTROL_PLANE_FEATURE_ID or self.contract_version != MECHANISM_CONTROL_PLANE_CONTRACT_VERSION: raise ResearchContractError("mechanism control-plane schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.question_id.strip() or not self.checks: raise ResearchContractError("mechanism control-plane identity or checks are incomplete")
        if not self.admitted_candidate_ids or len(set(self.admitted_candidate_ids)) != len(self.admitted_candidate_ids): raise ResearchContractError("mechanism candidate identities are not unique")
        if self.disposition not in {"passed", "blocked", "unknown"}: raise ResearchContractError("mechanism control-plane disposition is unknown")
        if self.evidence_receipt_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.evidence_receipt_digest): raise ResearchContractError("mechanism evidence digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]): raise ResearchContractError("mechanism receipt digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "federation_id": self.federation_id, "question_id": self.question_id, "admitted_candidate_ids": list(self.admitted_candidate_ids), "disposition": self.disposition, "evidence_receipt_digest": self.evidence_receipt_digest, "checks": list(self.checks), "omissions": list(self.omissions), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class MechanismGatewayReceipt:
    request_id: str
    federation_id: str
    source_profile: str
    target_profile: str
    projected_candidate_ids: tuple[str, ...]
    interoperability_profile: str
    disposition: str
    projection_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MECHANISM_GATEWAY_FEATURE_ID
    contract_version: str = MECHANISM_GATEWAY_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MECHANISM_GATEWAY_FEATURE_ID or self.contract_version != MECHANISM_GATEWAY_CONTRACT_VERSION: raise ResearchContractError("mechanism gateway schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.federation_id.strip() or not self.source_profile.strip() or not self.target_profile.strip() or not self.interoperability_profile.strip() or not self.checks: raise ResearchContractError("mechanism gateway identity or checks are incomplete")
        if not self.projected_candidate_ids or len(set(self.projected_candidate_ids)) != len(self.projected_candidate_ids): raise ResearchContractError("mechanism gateway candidate identities are not unique")
        if self.disposition not in {"passed", "blocked", "unknown"}: raise ResearchContractError("mechanism gateway disposition is unknown")
        if self.projection_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.projection_digest): raise ResearchContractError("mechanism gateway projection digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]): raise ResearchContractError("mechanism gateway receipt digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "federation_id": self.federation_id, "source_profile": self.source_profile, "target_profile": self.target_profile, "projected_candidate_ids": list(self.projected_candidate_ids), "interoperability_profile": self.interoperability_profile, "disposition": self.disposition, "projection_digest": self.projection_digest, "checks": list(self.checks), "omissions": list(self.omissions), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class EvidenceSurveillanceReceipt:
    request_id: str
    study_id: str
    intent: str
    selected_source_ids: tuple[str, ...]
    disposition: str
    qualified_set: Mapping[str, Any]
    effect_receipts: tuple[Mapping[str, Any], ...]
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = EVIDENCE_SURVEILLANCE_FEATURE_ID
    contract_version: str = EVIDENCE_SURVEILLANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != EVIDENCE_SURVEILLANCE_FEATURE_ID or self.contract_version != EVIDENCE_SURVEILLANCE_CONTRACT_VERSION:
            raise ResearchContractError("evidence surveillance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.study_id.strip() or not self.intent.strip() or not self.checks or not self.effect_receipts:
            raise ResearchContractError("evidence surveillance identity, checks, or effect receipts are incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"}:
            raise ResearchContractError("evidence surveillance disposition is unknown")
        if tuple(self.qualified_set.get("selected_source_ids", ())) != self.selected_source_ids or self.qualified_set.get("study_id") != self.study_id or self.qualified_set.get("intent") != self.intent:
            raise ResearchContractError("qualified evidence set is not linked to its receipt")
        if self.qualified_set.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or self.qualified_set.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("qualified evidence set schema or boundary mismatch")
        if self.qualified_set.get("ordering_rule") != "relevance_score descending, source_id ascending":
            raise ResearchContractError("qualified evidence ordering rule is not canonical")
        if len(set(self.selected_source_ids)) != len(self.selected_source_ids):
            raise ResearchContractError("qualified evidence source identities are not unique")
        if self.qualified_set.get("evidence_state") == "proven" and (self.omissions or self.uncertainty):
            raise ResearchContractError("proven evidence cannot contain unresolved omissions")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("evidence surveillance artifact digest is invalid")
        for effect in self.effect_receipts:
            if effect.get("effect") != "read_local_data" or not isinstance(effect.get("authorized"), bool) or not isinstance(effect.get("reason"), str) or not re.fullmatch(r"[0-9a-f]{64}", str(effect.get("receipt_digest", ""))):
                raise ResearchContractError("evidence surveillance effect receipt is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "study_id": self.study_id, "intent": self.intent, "selected_source_ids": list(self.selected_source_ids), "disposition": self.disposition, "qualified_set": dict(self.qualified_set), "effect_receipts": [dict(item) for item in self.effect_receipts], "checks": list(self.checks), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class RetrievalSynthesisReceipt:
    request_id: str
    query_id: str
    disposition: str
    synthesis: Mapping[str, Any]
    effect_receipts: tuple[Mapping[str, Any], ...]
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RETRIEVAL_SYNTHESIS_FEATURE_ID
    contract_version: str = RETRIEVAL_SYNTHESIS_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RETRIEVAL_SYNTHESIS_FEATURE_ID or self.contract_version != RETRIEVAL_SYNTHESIS_CONTRACT_VERSION:
            raise ResearchContractError("retrieval synthesis schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.query_id.strip() or not self.checks or not self.effect_receipts:
            raise ResearchContractError("retrieval synthesis identity, checks, or effects are incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"}:
            raise ResearchContractError("retrieval synthesis disposition is unknown")
        if self.synthesis.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or self.synthesis.get("query_id") != self.query_id or self.synthesis.get("boundary") != PRECLINICAL_BOUNDARY or not str(self.synthesis.get("comparability_profile", "")).strip():
            raise ResearchContractError("retrieval synthesis linkage or boundary is invalid")
        if tuple(self.synthesis.get("omissions", ())) != self.omissions or tuple(self.synthesis.get("uncertainty", ())) != self.uncertainty:
            raise ResearchContractError("retrieval synthesis omission linkage is invalid")
        selected = self.synthesis.get("selected_evidence_ids", ())
        if len(set(selected)) != len(selected) or len(selected) != len(self.synthesis.get("selected_digests", ())) or len(selected) != len(self.synthesis.get("selected_modalities", ())):
            raise ResearchContractError("retrieval synthesis selected evidence alignment is invalid")
        if self.synthesis.get("evidence_state") == "proven" and (self.omissions or self.uncertainty):
            raise ResearchContractError("proven synthesis cannot contain unresolved omissions")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("retrieval synthesis artifact digest is invalid")
        for effect in self.effect_receipts:
            if effect.get("effect") != "read_local_data" or not isinstance(effect.get("authorized"), bool) or not isinstance(effect.get("reason"), str) or not re.fullmatch(r"[0-9a-f]{64}", str(effect.get("receipt_digest", ""))):
                raise ResearchContractError("retrieval synthesis effect receipt is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "query_id": self.query_id, "disposition": self.disposition, "synthesis": dict(self.synthesis), "effect_receipts": [dict(item) for item in self.effect_receipts], "checks": list(self.checks), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class AdapterContextCompilationReceipt:
    request_id: str
    query_id: str
    resolved_fact_ids: tuple[str, ...]
    disposition: str
    evidence_receipt_digest: str | None
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_CONTEXT_COMPILATION_FEATURE_ID
    contract_version: str = ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != ADAPTER_CONTEXT_COMPILATION_FEATURE_ID or self.contract_version != ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION:
            raise ResearchContractError("adapter context compilation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.query_id.strip() or not self.checks:
            raise ResearchContractError("adapter context compilation identity or checks are incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"}:
            raise ResearchContractError("adapter context compilation disposition is unknown")
        if not self.resolved_fact_ids or len(set(self.resolved_fact_ids)) != len(self.resolved_fact_ids):
            raise ResearchContractError("resolved decision fact identities are invalid")
        if self.evidence_receipt_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", self.evidence_receipt_digest):
            raise ResearchContractError("adapter context evidence digest is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("adapter context artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "query_id": self.query_id, "resolved_fact_ids": list(self.resolved_fact_ids), "disposition": self.disposition, "evidence_receipt_digest": self.evidence_receipt_digest, "checks": list(self.checks), "omissions": list(self.omissions), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class KnowledgeWorkflowReceipt:
    request_id: str
    workflow_id: str
    disposition: str
    world: Mapping[str, Any]
    checks: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = KNOWLEDGE_WORKFLOW_FEATURE_ID
    contract_version: str = KNOWLEDGE_WORKFLOW_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != KNOWLEDGE_WORKFLOW_FEATURE_ID or self.contract_version != KNOWLEDGE_WORKFLOW_CONTRACT_VERSION:
            raise ResearchContractError("knowledge workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.workflow_id.strip() or not self.checks:
            raise ResearchContractError("knowledge workflow identity or checks are incomplete")
        if self.disposition not in {"passed", "blocked", "unknown"}:
            raise ResearchContractError("knowledge workflow disposition is unknown")
        if self.world.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or self.world.get("workflow_id") != self.workflow_id or self.world.get("boundary") != PRECLINICAL_BOUNDARY or not self.world.get("study_ids") or not self.world.get("stages"):
            raise ResearchContractError("typed knowledge world linkage is invalid")
        if tuple(self.world.get("omissions", ())) != self.omissions or tuple(self.world.get("uncertainty", ())) != self.uncertainty:
            raise ResearchContractError("knowledge workflow omission linkage is invalid")
        claims = self.world.get("resolved_claim_ids", ())
        if len(set(claims)) != len(claims):
            raise ResearchContractError("typed knowledge claim identities are not unique")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("knowledge workflow artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "workflow_id": self.workflow_id, "disposition": self.disposition, "world": dict(self.world), "checks": list(self.checks), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class ResourceWorkbenchReceipt:
    request_id: str
    need_id: str
    disposition: str
    qualified_resources: tuple[Mapping[str, Any], ...]
    omissions: tuple[Mapping[str, Any], ...]
    checks: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RESOURCE_WORKBENCH_FEATURE_ID
    contract_version: str = RESOURCE_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RESOURCE_WORKBENCH_FEATURE_ID or self.contract_version != RESOURCE_WORKBENCH_CONTRACT_VERSION:
            raise ResearchContractError("resource workbench schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.need_id.strip() or not self.checks:
            raise ResearchContractError("resource workbench identity or checks are incomplete")
        if self.disposition not in {"qualified", "partial", "blocked", "unknown"}:
            raise ResearchContractError("resource workbench disposition is unknown")
        for index, item in enumerate(self.qualified_resources, start=1):
            if item.get("rank") != index or not str(item.get("resource_id", "")).strip() or not str(item.get("origin", "")).strip() or not item.get("reasons"):
                raise ResearchContractError("qualified resource ranking or reasons are invalid")
            if not re.fullmatch(r"[0-9a-f]{64}", str(item.get("artifact_digest", ""))):
                raise ResearchContractError("qualified resource digest is invalid")
        for item in self.omissions:
            if not str(item.get("resource_id", "")).strip() or not str(item.get("reason", "")).strip():
                raise ResearchContractError("resource omission is incomplete")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("resource workbench artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "need_id": self.need_id, "disposition": self.disposition, "qualified_resources": [dict(item) for item in self.qualified_resources], "omissions": [dict(item) for item in self.omissions], "checks": list(self.checks), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class IngestionGatewayReceipt:
    request_id: str
    study_id: str
    disposition: str
    harmonized: Mapping[str, Any]
    admitted_bundles: tuple[str, ...]
    omitted_bundles: tuple[str, ...]
    effect_receipts: tuple[Mapping[str, Any], ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    contract_version: str = INGESTION_GATEWAY_CONTRACT_VERSION
    feature_id: str = INGESTION_GATEWAY_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != INGESTION_GATEWAY_FEATURE_ID or self.contract_version != INGESTION_GATEWAY_CONTRACT_VERSION:
            raise ResearchContractError("ingestion gateway schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.study_id.strip() or not self.reasons:
            raise ResearchContractError("ingestion gateway identity, boundary, or reasons are incomplete")
        if self.disposition not in {"admitted", "partial", "blocked"}:
            raise ResearchContractError("ingestion gateway disposition is unknown")
        if self.harmonized.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or self.harmonized.get("study_id") != self.study_id or self.harmonized.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("harmonized research object linkage is invalid")
        if len(set(self.admitted_bundles)) != len(self.admitted_bundles) or len(set(self.omitted_bundles)) != len(self.omitted_bundles):
            raise ResearchContractError("ingestion gateway bundle identities are not unique")
        if self.disposition == "blocked" and self.effect_receipts:
            raise ResearchContractError("blocked gateway receipts cannot contain effects")
        if len(self.effect_receipts) != len(self.admitted_bundles):
            raise ResearchContractError("each admitted bundle needs one effect receipt")
        for effect in self.effect_receipts:
            if effect.get("action") != "admit-local-harmonization" or effect.get("authorized") is not True or effect.get("bundle_id") not in self.admitted_bundles or not re.fullmatch(r"[0-9a-f]{64}", str(effect.get("source_digest", ""))):
                raise ResearchContractError("ingestion gateway effect receipt is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("ingestion gateway artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "study_id": self.study_id, "disposition": self.disposition, "harmonized": dict(self.harmonized), "admitted_bundles": list(self.admitted_bundles), "omitted_bundles": list(self.omitted_bundles), "effect_receipts": [dict(item) for item in self.effect_receipts], "semantic_loss": [dict(item) for item in self.semantic_loss], "reasons": list(self.reasons), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class QualityEnvelopeReceipt:
    envelope_id: str
    reference_schema: str
    comparability_profile: str
    disposition: str
    study_order: tuple[str, ...]
    modality_coverage: Mapping[str, int]
    verdicts: tuple[Mapping[str, Any], ...]
    omitted_modalities: tuple[str, ...]
    comparability_conflicts: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    contract_version: str = QUALITY_ENVELOPE_CONTRACT_VERSION
    feature_id: str = QUALITY_ENVELOPE_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != QUALITY_ENVELOPE_FEATURE_ID or self.contract_version != QUALITY_ENVELOPE_CONTRACT_VERSION:
            raise ResearchContractError("quality envelope schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.envelope_id.strip() or not self.reference_schema.strip() or not self.comparability_profile.strip() or not self.reasons:
            raise ResearchContractError("quality envelope identity, boundary, profile, or reasons are incomplete")
        if self.disposition not in {"qualified", "partial", "blocked", "unknown"}:
            raise ResearchContractError("quality envelope disposition is unknown")
        if not self.study_order or tuple(sorted(set(self.study_order))) != self.study_order or len(self.verdicts) != len(self.study_order):
            raise ResearchContractError("quality envelope study ordering is invalid")
        for study_id, verdict in zip(self.study_order, self.verdicts):
            if verdict.get("study_id") != study_id or not str(verdict.get("modality", "")).strip() or verdict.get("quality_disposition") not in {"pass", "pass_with_warnings", "blocked", "unknown"} or not isinstance(verdict.get("comparable"), bool) or not verdict.get("reasons"):
                raise ResearchContractError("quality envelope study verdict linkage is invalid")
        if any(not str(modality).strip() or not isinstance(count, int) or count < 0 for modality, count in self.modality_coverage.items()):
            raise ResearchContractError("quality envelope modality coverage is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("quality envelope artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "envelope_id": self.envelope_id, "reference_schema": self.reference_schema, "comparability_profile": self.comparability_profile, "disposition": self.disposition, "study_order": list(self.study_order), "modality_coverage": dict(self.modality_coverage), "verdicts": [dict(item) for item in self.verdicts], "omitted_modalities": list(self.omitted_modalities), "comparability_conflicts": list(self.comparability_conflicts), "semantic_loss": [dict(item) for item in self.semantic_loss], "reasons": list(self.reasons), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class ExperimentDesignReceipt:
    request_id: str
    objective_id: str
    disposition: str
    site_order: tuple[str, ...]
    assignments: tuple[Mapping[str, Any], ...]
    modality_coverage: Mapping[str, int]
    omitted_modalities: tuple[str, ...]
    comparability_conflicts: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = EXPERIMENT_DESIGN_CONTROL_FEATURE_ID
    contract_version: str = EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != EXPERIMENT_DESIGN_CONTROL_FEATURE_ID or self.contract_version != EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION:
            raise ResearchContractError("experiment design schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.objective_id.strip() or not self.reasons:
            raise ResearchContractError("experiment design identity, boundary, or reasons are incomplete")
        if self.disposition not in {"admitted", "partial", "blocked"}:
            raise ResearchContractError("experiment design disposition is unknown")
        if not self.site_order or tuple(sorted(set(self.site_order))) != self.site_order:
            raise ResearchContractError("experiment design site ordering is invalid")
        if self.disposition == "blocked" and self.assignments:
            raise ResearchContractError("blocked experiment design cannot contain assignments")
        for assignment in self.assignments:
            if not str(assignment.get("site_id", "")).strip() or not str(assignment.get("modality", "")).strip() or not str(assignment.get("instrument_profile", "")).strip() or assignment.get("authorized") is not True or not isinstance(assignment.get("budget"), (int, float)) or not math.isfinite(float(assignment["budget"])):
                raise ResearchContractError("experiment design assignment is invalid")
        if any(not str(key).strip() or not isinstance(value, int) or value < 0 for key, value in self.modality_coverage.items()):
            raise ResearchContractError("experiment design modality coverage is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("experiment design artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "objective_id": self.objective_id, "disposition": self.disposition, "site_order": list(self.site_order), "assignments": [dict(item) for item in self.assignments], "modality_coverage": dict(self.modality_coverage), "omitted_modalities": list(self.omitted_modalities), "comparability_conflicts": list(self.comparability_conflicts), "semantic_loss": [dict(item) for item in self.semantic_loss], "reasons": list(self.reasons), "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class ProtocolSimulationReceipt:
    protocol_id: str
    design_digest: str
    results: tuple[Mapping[str, Any], ...]
    passed: int
    failed_closed: int
    approval_required: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = PROTOCOL_SIMULATION_FEATURE_ID
    contract_version: str = PROTOCOL_SIMULATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != PROTOCOL_SIMULATION_FEATURE_ID or self.contract_version != PROTOCOL_SIMULATION_CONTRACT_VERSION:
            raise ResearchContractError("protocol simulation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.protocol_id.strip() or not re.fullmatch(r"[0-9a-f]{64}", self.design_digest) or not self.results:
            raise ResearchContractError("protocol simulation identity, digest, boundary, or results are incomplete")
        if self.passed + self.failed_closed + self.approval_required != len(self.results):
            raise ResearchContractError("protocol simulation state counts do not match results")
        ids = [str(result.get("scenario_id", "")) for result in self.results]
        if any(not item.strip() for item in ids) or ids != sorted(set(ids)):
            raise ResearchContractError("protocol simulation scenarios are not canonically ordered")
        for result in self.results:
            if result.get("state") not in {"passed", "failed_closed", "approval_required"} or not isinstance(result.get("reasons"), list) or not result["reasons"]:
                raise ResearchContractError("protocol simulation scenario result is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("protocol simulation artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "protocol_id": self.protocol_id, "design_digest": self.design_digest, "results": [dict(item) for item in self.results], "passed": self.passed, "failed_closed": self.failed_closed, "approval_required": self.approval_required, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "semantic_loss": [dict(item) for item in self.semantic_loss], "artifact": dict(self.artifact), "boundary": self.boundary})


@dataclass(frozen=True)
class InstrumentMeshReceipt:
    request_id: str
    federation_id: str
    action_id: str
    decision: str
    candidate_order: tuple[str, ...]
    selected_instrument_id: str | None
    selected_site_id: str | None
    selected_protocol_profile: str | None
    satisfied_capabilities: tuple[str, ...]
    missing_capabilities: tuple[str, ...]
    missing_interlocks: tuple[str, ...]
    effect: Mapping[str, Any] | None
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = INSTRUMENT_MESH_FEATURE_ID
    contract_version: str = INSTRUMENT_MESH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != INSTRUMENT_MESH_FEATURE_ID or self.contract_version != INSTRUMENT_MESH_CONTRACT_VERSION:
            raise ResearchContractError("instrument mesh schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.action_id.strip() or not self.reasons:
            raise ResearchContractError("instrument mesh identity, locality, boundary, or reasons are incomplete")
        if self.decision not in {"admitted", "approval_required", "blocked", "unknown"}:
            raise ResearchContractError("instrument mesh decision is unknown")
        if tuple(sorted(set(self.candidate_order))) != self.candidate_order:
            raise ResearchContractError("instrument mesh candidate order is not canonical")
        if any(not str(item).strip() for item in self.missing_capabilities + self.missing_interlocks):
            raise ResearchContractError("instrument mesh missing capability or interlock is empty")
        if self.decision == "admitted":
            if not self.selected_instrument_id or not self.selected_site_id or not self.effect:
                raise ResearchContractError("admitted instrument mesh receipt needs selection and effect receipt")
            if self.effect.get("authorized") is not True or self.effect.get("executed") is not False or self.effect.get("raw_data_local") is not True:
                raise ResearchContractError("instrument mesh effect must be authorized, not executed, and local")
        elif self.effect is not None:
            raise ResearchContractError("non-admitted instrument mesh receipt cannot contain an effect")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("instrument mesh artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "feature_id": self.feature_id, "contract_version": self.contract_version, "request_id": self.request_id, "federation_id": self.federation_id, "action_id": self.action_id, "decision": self.decision, "candidate_order": list(self.candidate_order), "selected_instrument_id": self.selected_instrument_id, "selected_site_id": self.selected_site_id, "selected_protocol_profile": self.selected_protocol_profile, "satisfied_capabilities": list(self.satisfied_capabilities), "missing_capabilities": list(self.missing_capabilities), "missing_interlocks": list(self.missing_interlocks), "effect": dict(self.effect) if self.effect else None, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "semantic_loss": [dict(item) for item in self.semantic_loss], "reasons": list(self.reasons), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


@dataclass(frozen=True)
class ComputationalExecutionReceipt:
    request_id: str
    workflow_id: str
    run_id: str
    decision: str
    ordered_nodes: tuple[str, ...]
    admitted_nodes: tuple[str, ...]
    run: Mapping[str, Any]
    run_digest: str
    authorized_effects: tuple[Mapping[str, Any], ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    semantic_loss: tuple[Mapping[str, Any], ...]
    reasons: tuple[str, ...]
    artifact: Mapping[str, Any]
    effects_executed: bool = False
    raw_data_local: bool = True
    feature_id: str = EXECUTION_CONTROL_FEATURE_ID
    contract_version: str = EXECUTION_CONTROL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != EXECUTION_CONTROL_FEATURE_ID or self.contract_version != EXECUTION_CONTROL_CONTRACT_VERSION:
            raise ResearchContractError("computational execution schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or self.effects_executed or not self.request_id.strip() or not self.workflow_id.strip() or not self.run_id.strip() or not self.ordered_nodes or not self.reasons:
            raise ResearchContractError("computational execution identity, locality, non-execution, graph, or reasons are incomplete")
        if self.decision not in {"dry_run", "admitted", "approval_required", "blocked"}:
            raise ResearchContractError("computational execution decision is unknown")
        if len(set(self.ordered_nodes)) != len(self.ordered_nodes) or len(set(self.admitted_nodes)) != len(self.admitted_nodes) or any(node not in self.ordered_nodes for node in self.admitted_nodes):
            raise ResearchContractError("computational execution node identities are invalid")
        if self.run.get("workflow_id") != self.workflow_id or self.run.get("status") != "planned":
            raise ResearchContractError("execution run linkage or planned status is invalid")
        if not re.fullmatch(r"[0-9a-f]{64}", self.run_digest):
            raise ResearchContractError("computational execution run digest is invalid")
        if self.decision == "admitted" and len(self.authorized_effects) != len(self.admitted_nodes):
            raise ResearchContractError("every admitted node needs an authorized effect")
        if self.decision != "admitted" and self.authorized_effects:
            raise ResearchContractError("non-admitted execution cannot contain effects")
        for effect in self.authorized_effects:
            if effect.get("effect") != "execute_local_computation" or effect.get("authorized") is not True or effect.get("executed") is not False or not re.fullmatch(r"[0-9a-f]{64}", str(effect.get("payload_digest", ""))):
                raise ResearchContractError("computational execution effect receipt is invalid")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("computational execution artifact digest is invalid")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "workflow_id": self.workflow_id, "run_id": self.run_id, "decision": self.decision, "ordered_nodes": list(self.ordered_nodes), "admitted_nodes": list(self.admitted_nodes), "run": dict(self.run), "run_digest": self.run_digest, "authorized_effects": [dict(item) for item in self.authorized_effects], "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "semantic_loss": [dict(item) for item in self.semantic_loss], "reasons": list(self.reasons), "artifact": dict(self.artifact), "effects_executed": self.effects_executed, "raw_data_local": self.raw_data_local, "boundary": self.boundary})
