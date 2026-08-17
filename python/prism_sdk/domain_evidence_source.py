"""Typed models for bounded external evidence source planning and execution."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .artifacts import _digest, _mapping, _text
from .domain_reports import _bounded_text_list
from .errors import ArgumentError

DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA = "bioprism-devplat-domain-evidence-source-plan/0.1"
DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW = "domain_evidence_source_plan"
DOMAIN_EVIDENCE_SOURCE_CONNECTOR_KINDS = (
    "literature",
    "clinical_trial",
    "fhir",
    "object_store",
    "file",
    "provider_api",
    "generic_http",
)
DOMAIN_EVIDENCE_SOURCE_LOCATOR_KINDS = ("uri", "path", "opaque")
DOMAIN_EVIDENCE_SOURCE_RETRIEVAL_MODES = ("reference_only", "metadata_only", "content")
DOMAIN_EVIDENCE_SOURCE_NETWORK_MODES = ("disabled", "caller_managed", "enabled")
DOMAIN_EVIDENCE_SOURCE_CACHE_MODES = ("no_cache", "content_addressed")
DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA = "bioprism-devplat-domain-evidence-source-execution/0.1"
DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW = "domain_evidence_source_execute"
DOMAIN_EVIDENCE_SOURCE_EXECUTION_OUTCOMES = ("observed", "partial", "refused", "error", "unknown")


@dataclass(frozen=True)
class DomainEvidenceSourcePlanRequest:
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    connector_kind: str
    locator_kind: str
    locator: str
    retrieval_mode: str
    source_tool: str | None = None
    expected_content_digest: str | None = None
    parent_digests: tuple[str, ...] = ()
    retrieval_policy: Mapping[str, Any] | None = None
    does_not_claim: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("domain evidence source group_id", self.group_id)
        _bounded_text_list("domain evidence source domains", self.domains, required=True)
        _text("domain evidence source subject_id", self.subject_id)
        if self.source_tool is not None:
            _text("domain evidence source source_tool", self.source_tool)
        if self.connector_kind not in DOMAIN_EVIDENCE_SOURCE_CONNECTOR_KINDS:
            raise ArgumentError("domain evidence source connector_kind is invalid")
        if self.locator_kind not in DOMAIN_EVIDENCE_SOURCE_LOCATOR_KINDS:
            raise ArgumentError("domain evidence source locator_kind is invalid")
        _text("domain evidence source locator", self.locator)
        if "\r" in self.locator or "\n" in self.locator:
            raise ArgumentError("domain evidence source locator must not contain line breaks")
        if self.locator_kind == "uri" and "://" in self.locator:
            authority = self.locator.split("://", 1)[1]
            authority = authority.split("/", 1)[0].split("?", 1)[0].split("#", 1)[0]
            if "@" in authority:
                raise ArgumentError("domain evidence source locator must not contain embedded credentials")
        if self.retrieval_mode not in DOMAIN_EVIDENCE_SOURCE_RETRIEVAL_MODES:
            raise ArgumentError("domain evidence source retrieval_mode is invalid")
        if self.expected_content_digest is not None:
            _digest("domain evidence source expected content digest", self.expected_content_digest)
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain evidence source parent_digests must contain at most 128 values")
        for digest in self.parent_digests:
            _digest("domain evidence source parent digest", digest)
        if self.retrieval_policy is not None:
            if not isinstance(self.retrieval_policy, Mapping):
                raise ArgumentError("domain evidence source retrieval_policy must be an object")
            network = self.retrieval_policy.get("network", "caller_managed")
            if network not in DOMAIN_EVIDENCE_SOURCE_NETWORK_MODES:
                raise ArgumentError("domain evidence source retrieval_policy.network is invalid")
            max_bytes = self.retrieval_policy.get("max_bytes", 2 * 1024 * 1024)
            if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= 64 * 1024 * 1024:
                raise ArgumentError("domain evidence source retrieval_policy.max_bytes is invalid")
            cache = self.retrieval_policy.get("cache", "content_addressed")
            if cache not in DOMAIN_EVIDENCE_SOURCE_CACHE_MODES:
                raise ArgumentError("domain evidence source retrieval_policy.cache is invalid")
            timeout_ms = self.retrieval_policy.get("timeout_ms", 5_000)
            if isinstance(timeout_ms, bool) or not isinstance(timeout_ms, int) or not 1 <= timeout_ms <= 30_000:
                raise ArgumentError("domain evidence source retrieval_policy.timeout_ms is invalid")
            allowed_hosts = self.retrieval_policy.get("allowed_hosts", [])
            if not isinstance(allowed_hosts, (list, tuple)) or len(allowed_hosts) > 32 or any(not isinstance(host, str) or not host.strip() or any(char in host for char in "\\r\\n/?#@:") for host in allowed_hosts):
                raise ArgumentError("domain evidence source retrieval_policy.allowed_hosts is invalid")
            if network == "enabled" and not allowed_hosts:
                raise ArgumentError("domain evidence source retrieval_policy.allowed_hosts is required when network is enabled")
        _bounded_text_list("domain evidence source does_not_claim", self.does_not_claim, required=True)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "connector_kind": self.connector_kind,
            "locator_kind": self.locator_kind,
            "locator": self.locator,
            "retrieval_mode": self.retrieval_mode,
            "parent_digests": list(self.parent_digests),
            "does_not_claim": list(self.does_not_claim),
        }
        if self.source_tool is not None:
            result["source_tool"] = self.source_tool
        if self.expected_content_digest is not None:
            result["expected_content_digest"] = self.expected_content_digest
        if self.retrieval_policy is not None:
            result["retrieval_policy"] = dict(self.retrieval_policy)
        return result


@dataclass(frozen=True)
class DomainEvidenceSourcePlanReport:
    raw: dict[str, Any]
    plan_digest: str
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    connector_kind: str
    locator_kind: str
    retrieval_mode: str
    retrieval_status: str
    plan: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceSourcePlanReport":
        raw = dict(value)
        if raw.get("workflow") != DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW:
            raise ArgumentError("domain evidence source plan workflow is invalid")
        if raw.get("schema") != DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA:
            raise ArgumentError("domain evidence source plan schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain evidence source plan must not claim readiness")
        if raw.get("execution") != "not_started":
            raise ArgumentError("domain evidence source plan execution must be not_started")
        if raw.get("retrieval_status") != "not_started":
            raise ArgumentError("domain evidence source plan retrieval must be not_started")
        domains = _bounded_text_list("domain evidence source domains", raw.get("domains"), required=True)
        artifact_registry = _mapping("domain evidence source artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain evidence source plan artifact is not indexed")
        return cls(
            raw=raw,
            plan_digest=_digest("domain evidence source plan digest", raw.get("plan_digest")),
            group_id=_text("domain evidence source group_id", raw.get("group_id")),
            domains=domains,
            subject_id=_text("domain evidence source subject_id", raw.get("subject_id")),
            connector_kind=_text("domain evidence source connector_kind", raw.get("connector_kind")),
            locator_kind=_text("domain evidence source locator_kind", raw.get("locator_kind")),
            retrieval_mode=_text("domain evidence source retrieval_mode", raw.get("retrieval_mode")),
            retrieval_status=raw["retrieval_status"],
            plan=_mapping("domain evidence source plan", raw.get("plan")),
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain evidence source catalogue digest", raw.get("catalogue_digest")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainEvidenceSourceExecutionRequest:
    """Execute a retained source plan and automatically retain its bounded intake envelope."""

    source_plan_digest: str
    source_tool: str | None = None
    request: Any = None
    claim_posture: Mapping[str, Any] | None = None
    parent_digests: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _digest("domain evidence source execution plan digest", self.source_plan_digest)
        if self.source_tool is not None:
            _text("domain evidence source execution source_tool", self.source_tool)
        if self.claim_posture is not None and not isinstance(self.claim_posture, Mapping):
            raise ArgumentError("domain evidence source execution claim_posture must be an object")
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain evidence source execution parent_digests must contain at most 128 values")
        for digest in self.parent_digests:
            _digest("domain evidence source execution parent digest", digest)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "source_plan_digest": self.source_plan_digest,
            "parent_digests": list(self.parent_digests),
        }
        if self.source_tool is not None:
            result["source_tool"] = self.source_tool
        if self.request is not None:
            result["request"] = self.request
        if self.claim_posture is not None:
            result["claim_posture"] = dict(self.claim_posture)
        return result


@dataclass(frozen=True)
class DomainEvidenceSourceExecutionReport:
    raw: dict[str, Any]
    source_plan_digest: str
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    outcome: str
    retrieval_status: str
    raw_content_digest: str | None
    response_digest: str
    byte_length: int | None
    execution_result: Mapping[str, Any]
    intake: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceSourceExecutionReport":
        raw = dict(value)
        if raw.get("workflow") != DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW:
            raise ArgumentError("domain evidence source execution workflow is invalid")
        if raw.get("schema") != DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA:
            raise ArgumentError("domain evidence source execution schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain evidence source execution must not claim readiness")
        outcome = _text("domain evidence source execution outcome", raw.get("outcome"))
        if outcome not in DOMAIN_EVIDENCE_SOURCE_EXECUTION_OUTCOMES:
            raise ArgumentError("domain evidence source execution outcome is invalid")
        intake = _mapping("domain evidence source execution intake", raw.get("intake"))
        artifact_registry = _mapping(
            "domain evidence source execution artifact registry",
            raw.get("artifact_registry"),
        )
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain evidence source execution intake is not indexed")
        raw_content_digest = raw.get("raw_content_digest")
        if raw_content_digest is not None:
            raw_content_digest = _digest(
                "domain evidence source execution raw content digest", raw_content_digest
            )
        return cls(
            raw=raw,
            source_plan_digest=_digest("domain evidence source execution plan digest", raw.get("source_plan_digest")),
            group_id=_text("domain evidence source execution group_id", raw.get("group_id")),
            domains=_bounded_text_list("domain evidence source execution domains", raw.get("domains"), required=True),
            subject_id=_text("domain evidence source execution subject_id", raw.get("subject_id")),
            source_tool=_text("domain evidence source execution source_tool", raw.get("source_tool")),
            outcome=outcome,
            retrieval_status=_text("domain evidence source execution retrieval status", raw.get("retrieval_status")),
            raw_content_digest=raw_content_digest,
            response_digest=_digest("domain evidence source execution response digest", raw.get("response_digest")),
            byte_length=raw.get("byte_length") if isinstance(raw.get("byte_length"), int) else None,
            execution_result=_mapping("domain evidence source execution result", raw.get("execution_result")),
            intake=intake,
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain evidence source execution catalogue digest", raw.get("catalogue_digest")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)
