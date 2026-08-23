"""Typed caller-managed provider evidence normalization models."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .adapter_execution_evidence import AdapterExecutionEvidenceRequest
from .artifacts import _digest, _mapping, _text
from .authoring import content_digest
from .capability import _route_count, _route_strings, _route_text, _tool_payload
from .domain_reports import DOMAIN_REPORT_CLAIM_STATUSES, _bounded_text_list
from .errors import ArgumentError

DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA = "bioprism-devplat-domain-evidence-provider-normalization/0.1"
DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW = "domain_evidence_provider_normalize"
DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA = "bioprism-devplat-domain-evidence-provider-replay/0.1"
DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW = "domain_evidence_provider_replay_verify"
DOMAIN_EVIDENCE_PROVIDER_REPLAY_STATUSES = ("matched", "mismatch")
DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_SCHEMA = "bioprism-devplat-domain-evidence-provider-record-index/0.1"
MAX_DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_ITEMS = 2048
DOMAIN_EVIDENCE_PROVIDER_CONNECTOR_KINDS = ("literature", "clinical_trial", "fhir", "object_store", "provider_api")
DOMAIN_EVIDENCE_PROVIDER_OUTCOMES = ("observed", "partial", "refused", "error", "unknown")
DOMAIN_EVIDENCE_PROVIDER_SHAPE_AUDIT_SCHEMA = "bioprism-devplat-domain-evidence-provider-shape-audit/0.1"
DOMAIN_EVIDENCE_PROVIDER_SHAPE_STATUSES = ("structured", "partial", "unclassified", "refused")
_MISSING = object()


def _json_value(name: str, value: Any) -> None:
    try:
        json.dumps(value, separators=(",", ":"), ensure_ascii=False)
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable") from error


@dataclass(frozen=True)
class DomainEvidenceProviderShapeCoverage:
    """Field-presence counts without retaining any identifier or payload value."""

    candidate_fields: tuple[str, ...]
    present_record_count: int
    missing_record_count: int

    @classmethod
    def from_wire(cls, name: str, value: Any) -> "DomainEvidenceProviderShapeCoverage":
        raw = _mapping(name, value)
        return cls(
            candidate_fields=_route_strings(f"{name}.candidate_fields", raw.get("candidate_fields")),
            present_record_count=_route_count(
                f"{name}.present_record_count", raw.get("present_record_count")
            ),
            missing_record_count=_route_count(
                f"{name}.missing_record_count", raw.get("missing_record_count")
            ),
        )


@dataclass(frozen=True)
class DomainEvidenceProviderShapeAudit:
    """Connector-specific container audit; it does not interpret domain values."""

    schema: str
    status: str
    connector_kind: str
    root_kind: str
    recognized_container: str | None
    record_count: int
    valid_record_count: int
    invalid_record_count: int
    identifier_coverage: DomainEvidenceProviderShapeCoverage
    content_digest_coverage: DomainEvidenceProviderShapeCoverage | None
    missing_fields: tuple[str, ...]
    warnings: tuple[str, ...]
    limitations: tuple[str, ...]
    shape_digest: str

    @classmethod
    def from_wire(cls, value: Any) -> "DomainEvidenceProviderShapeAudit":
        raw = _mapping("domain evidence provider shape audit", value)
        schema = _route_text("domain evidence provider shape audit schema", raw.get("schema"))
        if schema != DOMAIN_EVIDENCE_PROVIDER_SHAPE_AUDIT_SCHEMA:
            raise ArgumentError("domain evidence provider shape audit schema is unsupported")
        status = _route_text("domain evidence provider shape audit status", raw.get("status"))
        if status not in DOMAIN_EVIDENCE_PROVIDER_SHAPE_STATUSES:
            raise ArgumentError("domain evidence provider shape audit status is invalid")
        recognized_container = raw.get("recognized_container")
        if recognized_container is not None:
            recognized_container = _route_text(
                "domain evidence provider recognized container", recognized_container
            )
        digest_coverage = raw.get("content_digest_coverage")
        return cls(
            schema=schema,
            status=status,
            connector_kind=_route_text("domain evidence provider shape audit connector", raw.get("connector_kind")),
            root_kind=_route_text("domain evidence provider shape audit root kind", raw.get("root_kind")),
            recognized_container=recognized_container,
            record_count=_route_count("domain evidence provider shape audit record count", raw.get("record_count")),
            valid_record_count=_route_count(
                "domain evidence provider shape audit valid record count", raw.get("valid_record_count")
            ),
            invalid_record_count=_route_count(
                "domain evidence provider shape audit invalid record count", raw.get("invalid_record_count")
            ),
            identifier_coverage=DomainEvidenceProviderShapeCoverage.from_wire(
                "domain evidence provider identifier coverage", raw.get("identifier_coverage")
            ),
            content_digest_coverage=(
                None
                if digest_coverage is None
                else DomainEvidenceProviderShapeCoverage.from_wire(
                    "domain evidence provider content digest coverage", digest_coverage
                )
            ),
            missing_fields=_route_strings(
                "domain evidence provider shape audit missing fields", raw.get("missing_fields", [])
            ),
            warnings=_route_strings(
                "domain evidence provider shape audit warnings", raw.get("warnings", [])
            ),
            limitations=_route_strings(
                "domain evidence provider shape audit limitations", raw.get("limitations", [])
            ),
            shape_digest=_digest("domain evidence provider shape digest", raw.get("shape_digest")),
        )


@dataclass(frozen=True)
class DomainEvidenceProviderRecordIndex:
    """Bounded canonical row identities with explicit omission accounting."""

    schema: str
    connector_kind: str
    recognized_container: str | None
    record_count: int
    indexed_record_count: int
    omitted_record_count: int
    row_digests: tuple[str, ...]
    index_digest: str
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Any) -> "DomainEvidenceProviderRecordIndex":
        raw = _mapping("domain evidence provider record index", value)
        schema = _route_text("domain evidence provider record index schema", raw.get("schema"))
        if schema != DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_SCHEMA:
            raise ArgumentError("domain evidence provider record index schema is unsupported")
        recognized_container = raw.get("recognized_container")
        if recognized_container is not None:
            recognized_container = _route_text(
                "domain evidence provider record index container", recognized_container
            )
        row_digests = raw.get("row_digests")
        if not isinstance(row_digests, Sequence) or isinstance(row_digests, (str, bytes)):
            raise ArgumentError("domain evidence provider record index row_digests must be an array")
        if len(row_digests) > MAX_DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_ITEMS:
            raise ArgumentError("domain evidence provider record index exceeds its item bound")
        return cls(
            schema=schema,
            connector_kind=_route_text("domain evidence provider record index connector", raw.get("connector_kind")),
            recognized_container=recognized_container,
            record_count=_route_count("domain evidence provider record count", raw.get("record_count")),
            indexed_record_count=_route_count(
                "domain evidence provider indexed record count", raw.get("indexed_record_count")
            ),
            omitted_record_count=_route_count(
                "domain evidence provider omitted record count", raw.get("omitted_record_count")
            ),
            row_digests=tuple(
                _digest("domain evidence provider row digest", digest) for digest in row_digests
            ),
            index_digest=_digest("domain evidence provider index digest", raw.get("index_digest")),
            limitations=_route_strings(
                "domain evidence provider record index limitations", raw.get("limitations", [])
            ),
        )


@dataclass(frozen=True)
class DomainEvidenceProviderNormalizationRequest:
    """Caller-owned provider payload with explicit domain and connector scope."""

    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    connector_kind: str
    provider: str
    payload: Any
    request: Any = _MISSING
    outcome: str = "unknown"
    claim_posture: Mapping[str, Any] | None = None
    parent_digests: tuple[str, ...] = ()
    source_plan_digest: str | None = None

    def __post_init__(self) -> None:
        _text("domain evidence provider group_id", self.group_id)
        _bounded_text_list("domain evidence provider domains", self.domains, required=True)
        _text("domain evidence provider subject_id", self.subject_id)
        _text("domain evidence provider source_tool", self.source_tool)
        _text("domain evidence provider", self.provider)
        if self.connector_kind not in DOMAIN_EVIDENCE_PROVIDER_CONNECTOR_KINDS:
            raise ArgumentError("domain evidence provider connector_kind is invalid")
        if self.outcome not in DOMAIN_EVIDENCE_PROVIDER_OUTCOMES:
            raise ArgumentError("domain evidence provider outcome is invalid")
        if self.claim_posture is not None:
            if not isinstance(self.claim_posture, Mapping):
                raise ArgumentError("domain evidence provider claim_posture must be an object")
            if self.claim_posture.get("status") not in DOMAIN_REPORT_CLAIM_STATUSES:
                raise ArgumentError("domain evidence provider claim_posture.status is invalid")
            _bounded_text_list(
                "domain evidence provider claim_posture.does_not_claim",
                self.claim_posture.get("does_not_claim"),
                required=True,
            )
        if not isinstance(self.payload, (Mapping, Sequence)) or isinstance(self.payload, (str, bytes)):
            raise ArgumentError("domain evidence provider payload must be an object or array")
        _json_value("domain evidence provider payload", self.payload)
        if self.request is not _MISSING:
            _json_value("domain evidence provider request", self.request)
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain evidence provider parent_digests must contain at most 128 values")
        for parent in self.parent_digests:
            _digest("domain evidence provider parent digest", parent)
        if self.source_plan_digest is not None:
            _digest("domain evidence provider source plan digest", self.source_plan_digest)

    @property
    def request_supplied(self) -> bool:
        return self.request is not _MISSING

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "source_tool": self.source_tool,
            "connector_kind": self.connector_kind,
            "provider": self.provider,
            "payload": self.payload,
            "outcome": self.outcome,
            "parent_digests": list(self.parent_digests),
        }
        if self.request is not _MISSING:
            result["request"] = self.request
        if self.claim_posture is not None:
            result["claim_posture"] = dict(self.claim_posture)
        if self.source_plan_digest is not None:
            result["source_plan_digest"] = self.source_plan_digest
        return result


@dataclass(frozen=True)
class DomainEvidenceProviderReplayRequest:
    """Re-submit one provider envelope against retained canonical identities."""

    observation: DomainEvidenceProviderNormalizationRequest
    expected_payload_digest: str
    expected_request_digest: str | None
    expected_shape_digest: str
    expected_normalization_digest: str
    expected_intake_digest: str

    def __post_init__(self) -> None:
        for name, value in (
            ("expected provider payload digest", self.expected_payload_digest),
            ("expected provider shape digest", self.expected_shape_digest),
            ("expected provider normalization digest", self.expected_normalization_digest),
            ("expected provider intake digest", self.expected_intake_digest),
        ):
            _digest(name, value)
        if self.expected_request_digest is not None:
            _digest("expected provider request digest", self.expected_request_digest)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            **self.observation.to_mcp_arguments(),
            "expected_payload_digest": self.expected_payload_digest,
            "expected_request_digest": self.expected_request_digest,
            "expected_shape_digest": self.expected_shape_digest,
            "expected_normalization_digest": self.expected_normalization_digest,
            "expected_intake_digest": self.expected_intake_digest,
        }


@dataclass(frozen=True)
class DomainEvidenceProviderNormalizationReport:
    raw: dict[str, Any]
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    connector_kind: str
    provider: str
    outcome: str
    payload_digest: str
    request_digest: str | None
    response: Mapping[str, Any]
    shape_audit: DomainEvidenceProviderShapeAudit
    record_index: DomainEvidenceProviderRecordIndex
    intake: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderNormalizationReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW)
        if raw.get("ok") is not True:
            raise ArgumentError("domain evidence provider normalization report is not successful")
        normalization = _mapping("domain evidence provider normalization", raw.get("normalization"))
        artifact_registry = _mapping("domain evidence provider artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain evidence provider intake artifact is not indexed")
        request_digest = raw.get("request_digest")
        return cls(
            raw=raw,
            group_id=_route_text("domain evidence provider group_id", raw.get("group_id")),
            domains=_bounded_text_list("domain evidence provider domains", raw.get("domains"), required=True),
            subject_id=_route_text("domain evidence provider subject_id", raw.get("subject_id")),
            source_tool=_route_text("domain evidence provider source_tool", raw.get("source_tool")),
            connector_kind=_route_text("domain evidence provider connector_kind", raw.get("connector_kind")),
            provider=_route_text("domain evidence provider", raw.get("provider")),
            outcome=_route_text("domain evidence provider outcome", raw.get("outcome")),
            payload_digest=_digest("domain evidence provider payload digest", normalization.get("payload_digest")),
            request_digest=(
                None
                if request_digest is None
                else _digest("domain evidence provider request digest", request_digest)
            ),
            response=_mapping("domain evidence provider response", raw.get("response")),
            shape_audit=DomainEvidenceProviderShapeAudit.from_wire(raw.get("shape_audit")),
            record_index=DomainEvidenceProviderRecordIndex.from_wire(raw.get("record_index")),
            intake=_mapping("domain evidence provider intake", raw.get("intake")),
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain evidence provider catalogue digest", raw.get("catalogue_digest")),
            guarantees=_route_strings("domain evidence provider guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("domain evidence provider limitations", raw.get("does_not_claim", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    @property
    def normalization_digest(self) -> str:
        """Digest the public normalization object in the same canonical form as Rust."""

        return content_digest(self.raw["normalization"])

    @property
    def intake_digest(self) -> str:
        return _digest("domain evidence provider intake digest", self.intake.get("intake_digest"))

    def to_adapter_execution_evidence_request(
        self,
        adapter_id: str,
        adapter_version: str,
        source_id: str,
        *,
        parent_digests: Sequence[str] = (),
        attempt_id: str | None = None,
    ) -> AdapterExecutionEvidenceRequest:
        """Project structural provider normalization into shared caller-owned evidence.

        Provider normalization is not adapter execution and does not establish provider
        authenticity. The caller must therefore declare the adapter identity and source identity;
        this bridge only maps the retained provider envelope, carries its known digest lineage, and
        preserves observed/partial/refused/error outcomes.
        """

        execution_status = {
            "observed": "succeeded",
            "partial": "partial",
            "refused": "refused",
            "error": "failed",
            "unknown": "unknown",
        }[self.outcome]
        conformance_status = {
            "structured": "verified",
            "partial": "partial",
            "refused": "refused",
            "unclassified": "unknown",
        }[self.shape_audit.status]
        output_digest = self.normalization_digest if execution_status in {"succeeded", "partial"} else None
        error_code = None
        if execution_status in {"refused", "failed"}:
            error_code = f"provider_{self.outcome}"
        lineage: list[str] = []
        candidates: list[str | None] = [
            self.request_digest,
            self.shape_audit.shape_digest,
            self.record_index.index_digest,
            self.catalogue_digest,
        ]
        raw_source_plan_digest = self.raw.get("source_plan_digest")
        if isinstance(raw_source_plan_digest, str):
            candidates.append(raw_source_plan_digest)
        raw_intake_digest = self.intake.get("intake_digest")
        if isinstance(raw_intake_digest, str):
            candidates.append(raw_intake_digest)
        candidates.extend(parent_digests)
        for digest in candidates:
            if digest is None:
                continue
            normalized = _digest("domain evidence provider parent digest", digest)
            if normalized not in lineage:
                lineage.append(normalized)
        return AdapterExecutionEvidenceRequest(
            group_id=self.group_id,
            domains=self.domains,
            subject_id=self.subject_id,
            adapter_id=adapter_id,
            adapter_version=adapter_version,
            source_id=source_id,
            input_digest=self.payload_digest,
            output_digest=output_digest,
            execution_status=execution_status,
            conformance_status=conformance_status,
            semantic_loss_status="unknown",
            item_count=self.record_index.record_count,
            error_code=error_code,
            parent_digests=tuple(lineage),
            attempt_id=attempt_id,
        )


def domain_evidence_provider_normalization_report(
    value: Mapping[str, Any],
) -> DomainEvidenceProviderNormalizationReport:
    return DomainEvidenceProviderNormalizationReport.from_wire(value)


@dataclass(frozen=True)
class DomainEvidenceProviderReplayVerificationReport:
    """Value-free replay comparison and its indexed artifact registration."""

    raw: dict[str, Any]
    replay_status: str
    matched: bool
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    connector_kind: str
    provider: str
    expected_payload_digest: str
    observed_payload_digest: str
    expected_request_digest: str | None
    observed_request_digest: str | None
    expected_shape_digest: str
    observed_shape_digest: str
    expected_normalization_digest: str
    observed_normalization_digest: str
    expected_intake_digest: str
    observed_intake_digest: str
    matches: Mapping[str, Any]
    differences: tuple[str, ...]
    shape_audit: DomainEvidenceProviderShapeAudit
    record_index: DomainEvidenceProviderRecordIndex
    replay_digest: str
    artifact_registry: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderReplayVerificationReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW)
        if raw.get("ok") is not True:
            raise ArgumentError("domain evidence provider replay report is not successful")
        replay = _mapping("domain evidence provider replay", raw.get("replay"))
        replay_status = _route_text("domain evidence provider replay status", replay.get("replay_status"))
        if replay_status not in DOMAIN_EVIDENCE_PROVIDER_REPLAY_STATUSES:
            raise ArgumentError("domain evidence provider replay status is invalid")
        matched = replay.get("matched")
        if not isinstance(matched, bool) or matched != (replay_status == "matched"):
            raise ArgumentError("domain evidence provider replay matched/status fields disagree")
        return cls(
            raw=raw,
            replay_status=replay_status,
            matched=matched,
            group_id=_route_text("domain evidence provider replay group_id", replay.get("group_id")),
            domains=_bounded_text_list("domain evidence provider replay domains", replay.get("domains"), required=True),
            subject_id=_route_text("domain evidence provider replay subject_id", replay.get("subject_id")),
            source_tool=_route_text("domain evidence provider replay source_tool", replay.get("source_tool")),
            connector_kind=_route_text("domain evidence provider replay connector_kind", replay.get("connector_kind")),
            provider=_route_text("domain evidence provider replay provider", replay.get("provider")),
            expected_payload_digest=_digest("expected provider payload digest", replay.get("expected_payload_digest")),
            observed_payload_digest=_digest("observed provider payload digest", replay.get("observed_payload_digest")),
            expected_request_digest=(
                None
                if replay.get("expected_request_digest") is None
                else _digest("expected provider request digest", replay.get("expected_request_digest"))
            ),
            observed_request_digest=(
                None
                if replay.get("observed_request_digest") is None
                else _digest("observed provider request digest", replay.get("observed_request_digest"))
            ),
            expected_shape_digest=_digest("expected provider shape digest", replay.get("expected_shape_digest")),
            observed_shape_digest=_digest("observed provider shape digest", replay.get("observed_shape_digest")),
            expected_normalization_digest=_digest(
                "expected provider normalization digest", replay.get("expected_normalization_digest")
            ),
            observed_normalization_digest=_digest(
                "observed provider normalization digest", replay.get("observed_normalization_digest")
            ),
            expected_intake_digest=_digest("expected provider intake digest", replay.get("expected_intake_digest")),
            observed_intake_digest=_digest("observed provider intake digest", replay.get("observed_intake_digest")),
            matches=_mapping("domain evidence provider replay matches", replay.get("matches")),
            differences=_route_strings("domain evidence provider replay differences", replay.get("differences", [])),
            shape_audit=DomainEvidenceProviderShapeAudit.from_wire(replay.get("shape_audit")),
            record_index=DomainEvidenceProviderRecordIndex.from_wire(replay.get("record_index")),
            replay_digest=_digest("domain evidence provider replay digest", replay.get("replay_digest")),
            artifact_registry=_mapping("domain evidence provider replay artifact registry", raw.get("artifact_registry")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_evidence_provider_replay_verification_report(
    value: Mapping[str, Any],
) -> DomainEvidenceProviderReplayVerificationReport:
    return DomainEvidenceProviderReplayVerificationReport.from_wire(value)


__all__ = [
    "DOMAIN_EVIDENCE_PROVIDER_CONNECTOR_KINDS",
    "DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW",
    "DOMAIN_EVIDENCE_PROVIDER_OUTCOMES",
    "DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_REPLAY_STATUSES",
    "DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW",
    "DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_SCHEMA",
    "MAX_DOMAIN_EVIDENCE_PROVIDER_RECORD_INDEX_ITEMS",
    "DomainEvidenceProviderRecordIndex",
    "DOMAIN_EVIDENCE_PROVIDER_SHAPE_AUDIT_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_SHAPE_STATUSES",
    "DomainEvidenceProviderShapeAudit",
    "DomainEvidenceProviderShapeCoverage",
    "DomainEvidenceProviderNormalizationRequest",
    "DomainEvidenceProviderNormalizationReport",
    "DomainEvidenceProviderReplayRequest",
    "DomainEvidenceProviderReplayVerificationReport",
    "domain_evidence_provider_replay_verification_report",
    "domain_evidence_provider_normalization_report",
]
