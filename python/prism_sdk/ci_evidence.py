"""Typed CI execution-evidence requests and structural reports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError

CI_EXECUTION_EVIDENCE_SCHEMA = "bioprism-devplat-ci-execution-evidence/0.1"


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or not value:
        raise ArgumentError(f"{name} must be a non-empty mapping")
    return dict(value)


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if len(text) != 64 or any(character not in "0123456789abcdef" for character in text.lower()):
        raise ArgumentError(f"{name} must be a 64-character hexadecimal digest")
    return text


@dataclass(frozen=True)
class CiExecutionEvidenceRequest:
    ci: Mapping[str, Any]
    evidence: Mapping[str, Any]

    def __post_init__(self) -> None:
        _mapping("ci", self.ci)
        _mapping("evidence", self.evidence)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"ci": dict(self.ci), "evidence": dict(self.evidence)}


@dataclass(frozen=True)
class CiEvidenceFindingReport:
    raw: dict[str, Any]
    code: str
    severity: str
    subject: str
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiEvidenceFindingReport":
        raw = _route_mapping("CI evidence finding", value)
        return cls(
            raw=raw,
            code=_route_text("CI evidence finding code", raw.get("code")),
            severity=_route_text("CI evidence finding severity", raw.get("severity")),
            subject=_route_text("CI evidence finding subject", raw.get("subject")),
            detail=_route_text("CI evidence finding detail", raw.get("detail")),
        )


@dataclass(frozen=True)
class CiExecutionEvidenceReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    plan_digest: str
    evidence_digest: str
    run_id: str
    provider: str
    source: str
    conclusion: str
    structurally_valid: bool
    complete: bool
    release_candidate: bool
    execution: str
    verification: str
    expected_check_count: int
    observed_check_count: int
    passed_check_count: int
    failed_check_count: int
    skipped_check_count: int
    unknown_check_count: int
    required_missing: tuple[str, ...]
    required_failed: tuple[str, ...]
    optional_nonpassing: tuple[str, ...]
    findings: tuple[CiEvidenceFindingReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiExecutionEvidenceReport":
        raw = _tool_payload(value, "ci_execution_evidence_audit")
        if raw.get("ok") is not True:
            raise ArgumentError("CI execution evidence report is not successful")
        audit = _route_mapping("CI execution evidence audit", raw.get("audit"))
        findings_raw = audit.get("findings", [])
        if not isinstance(findings_raw, list):
            raise ArgumentError("CI evidence findings must be an array")
        return cls(
            raw=raw,
            schema=_route_text("CI evidence schema", raw.get("schema")),
            workflow=_route_text("CI evidence workflow", audit.get("workflow")),
            plan_digest=_digest("CI evidence plan_digest", audit.get("plan_digest")),
            evidence_digest=_digest("CI evidence evidence_digest", audit.get("evidence_digest")),
            run_id=_route_text("CI evidence run_id", audit.get("run_id")),
            provider=_route_text("CI evidence provider", audit.get("provider")),
            source=_route_text("CI evidence source", audit.get("source")),
            conclusion=_route_text("CI evidence conclusion", audit.get("conclusion")),
            structurally_valid=audit.get("structurally_valid") is True,
            complete=audit.get("complete") is True,
            release_candidate=raw.get("ci_evidence_ready") is True,
            execution=_route_text("CI evidence execution", audit.get("execution")),
            verification=_route_text("CI evidence verification", audit.get("verification")),
            expected_check_count=_route_count("CI evidence expected_check_count", audit.get("expected_check_count")),
            observed_check_count=_route_count("CI evidence observed_check_count", audit.get("observed_check_count")),
            passed_check_count=_route_count("CI evidence passed_check_count", audit.get("passed_check_count")),
            failed_check_count=_route_count("CI evidence failed_check_count", audit.get("failed_check_count")),
            skipped_check_count=_route_count("CI evidence skipped_check_count", audit.get("skipped_check_count")),
            unknown_check_count=_route_count("CI evidence unknown_check_count", audit.get("unknown_check_count")),
            required_missing=_route_strings("CI evidence required_missing", audit.get("required_missing", [])),
            required_failed=_route_strings("CI evidence required_failed", audit.get("required_failed", [])),
            optional_nonpassing=_route_strings("CI evidence optional_nonpassing", audit.get("optional_nonpassing", [])),
            findings=tuple(CiEvidenceFindingReport.from_wire(item) for item in findings_raw),
        )

    @property
    def blocking_findings(self) -> tuple[CiEvidenceFindingReport, ...]:
        return tuple(finding for finding in self.findings if finding.severity == "blocking")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def ci_execution_evidence_report(value: Mapping[str, Any]) -> CiExecutionEvidenceReport:
    """Parse a direct MCP result or HTTP REST tool envelope."""

    return CiExecutionEvidenceReport.from_wire(value)


__all__ = [
    "CI_EXECUTION_EVIDENCE_SCHEMA",
    "CiExecutionEvidenceRequest",
    "CiEvidenceFindingReport",
    "CiExecutionEvidenceReport",
    "ci_execution_evidence_report",
]
