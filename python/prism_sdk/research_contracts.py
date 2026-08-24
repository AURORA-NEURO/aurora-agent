"""Small standard-library mirror of the Rust research-contract boundary.

The Python SDK transports receipts and hashes payloads; it never interprets an unknown receipt as a
positive scientific conclusion or moves protected source bytes out of an institution.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import json
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
