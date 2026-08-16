"""Typed security/privacy governance manifests and fail-closed report projections."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


SECURITY_PRIVACY_MANIFEST_SCHEMA = "bioprism-security-privacy/0.1"
SECURITY_PRIVACY_AUDIT_SCHEMA = "bioprism-security-privacy-audit/0.1"
SECURITY_PRIVACY_MAX_INPUT_BYTES = 20_000_000
SECURITY_PRIVACY_MAX_ASSETS = 4_096
SECURITY_PRIVACY_MAX_FLOWS = 8_192
SECURITY_PRIVACY_MAX_IDENTITIES = 8_192
SECURITY_PRIVACY_MAX_THREATS = 8_192
SECURITY_PRIVACY_MAX_REVIEWS = 4_096
SECURITY_PRIVACY_MAX_LIST_ITEMS = 16_384
SECURITY_PRIVACY_MAX_TEXT_BYTES = 4_096

SecurityPrivacyClassification = Literal["public", "internal", "confidential", "restricted", "regulated"]
SecurityPrivacyFlowDecision = Literal["allow", "deny", "conditional"]
SecurityPrivacyThreatSeverity = Literal["low", "medium", "high", "critical"]
SecurityPrivacyThreatStatus = Literal["mitigated", "accepted", "unmitigated", "unanalysed"]
SecurityPrivacyReviewKind = Literal["privacy_impact", "security_assessment", "red_team", "access_review"]
SecurityPrivacyReviewStatus = Literal["draft", "in_review", "complete", "expired"]
SecurityPrivacyIssueSeverity = Literal["warning", "blocking"]


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bounded(name: str, value: Any, limit: int) -> tuple[Any, ...]:
    values = _sequence(name, value)
    if len(values) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return values


def _text(name: str, value: Any, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    result = _route_text(name, value)
    if required and not result.strip():
        raise ArgumentError(f"{name} must not be empty")
    if len(result.encode("utf-8")) > SECURITY_PRIVACY_MAX_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {SECURITY_PRIVACY_MAX_TEXT_BYTES} UTF-8 bytes")
    return result


def _strings(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(_bounded(name, value, SECURITY_PRIVACY_MAX_LIST_ITEMS)))  # type: ignore[misc]


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _enum(name: str, value: Any, allowed: frozenset[str]) -> str:
    result = _text(name, value)
    assert result is not None
    if result not in allowed:
        raise ArgumentError(f"{name} must be one of {sorted(allowed)}")
    return result


def _digest(name: str, value: Any) -> str:
    result = _text(name, value)
    assert result is not None
    if len(result) != 64 or any(character not in "0123456789abcdefABCDEF" for character in result):
        raise ArgumentError(f"{name} must be 64 hexadecimal characters")
    return result


def _json_size(name: str, value: Any) -> None:
    try:
        encoded = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable: {error}") from error
    if len(encoded) > SECURITY_PRIVACY_MAX_INPUT_BYTES:
        raise ArgumentError(f"{name} exceeds the {SECURITY_PRIVACY_MAX_INPUT_BYTES}-byte safety bound")


@dataclass(frozen=True)
class SecurityPrivacySystemArgs:
    id: str
    version: str
    owner: str

    def __post_init__(self) -> None:
        for name in ("id", "version", "owner"):
            _text(f"system.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacySystemArgs":
        raw = _mapping("security/privacy system", value)
        return cls(_text("system.id", raw.get("id")), _text("system.version", raw.get("version")), _text("system.owner", raw.get("owner")))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version, "owner": self.owner}


@dataclass(frozen=True)
class SecurityPrivacyAssetArgs:
    id: str
    name: str
    classification: SecurityPrivacyClassification
    owner: str
    purpose: str
    retention_days: int | None = None
    residency: str = ""
    deletion_process: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "name", "owner", "purpose", "residency"):
            _text(f"asset.{name}", getattr(self, name))
        object.__setattr__(self, "classification", _enum("asset.classification", self.classification, frozenset({"public", "internal", "confidential", "restricted", "regulated"})))
        if self.retention_days is not None and (not isinstance(self.retention_days, int) or isinstance(self.retention_days, bool) or self.retention_days < 0):
            raise ArgumentError("asset.retention_days must be a non-negative integer")
        object.__setattr__(self, "deletion_process", _text("asset.deletion_process", self.deletion_process, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyAssetArgs":
        raw = _mapping("security/privacy asset", value)
        return cls(_text("asset.id", raw.get("id")), _text("asset.name", raw.get("name")), _enum("asset.classification", raw.get("classification"), frozenset({"public", "internal", "confidential", "restricted", "regulated"})), _text("asset.owner", raw.get("owner")), _text("asset.purpose", raw.get("purpose")), raw.get("retention_days"), _text("asset.residency", raw.get("residency")), raw.get("deletion_process"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "name": self.name, "classification": self.classification, "owner": self.owner, "purpose": self.purpose, "residency": self.residency}
        if self.retention_days is not None:
            result["retention_days"] = self.retention_days
        if self.deletion_process is not None:
            result["deletion_process"] = self.deletion_process
        return result


@dataclass(frozen=True)
class SecurityPrivacyFlowArgs:
    id: str
    asset: str
    source: str
    destination: str
    purpose: str
    decision: SecurityPrivacyFlowDecision
    legal_basis: str | None = None
    authorization_evidence: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "asset", "source", "destination", "purpose"):
            _text(f"flow.{name}", getattr(self, name))
        object.__setattr__(self, "decision", _enum("flow.decision", self.decision, frozenset({"allow", "deny", "conditional"})))
        object.__setattr__(self, "legal_basis", _text("flow.legal_basis", self.legal_basis, required=False))
        object.__setattr__(self, "authorization_evidence", None if self.authorization_evidence is None else _digest("flow.authorization_evidence", self.authorization_evidence))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyFlowArgs":
        raw = _mapping("security/privacy flow", value)
        return cls(_text("flow.id", raw.get("id")), _text("flow.asset", raw.get("asset")), _text("flow.source", raw.get("source")), _text("flow.destination", raw.get("destination")), _text("flow.purpose", raw.get("purpose")), _enum("flow.decision", raw.get("decision"), frozenset({"allow", "deny", "conditional"})), raw.get("legal_basis"), raw.get("authorization_evidence"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "asset": self.asset, "source": self.source, "destination": self.destination, "purpose": self.purpose, "decision": self.decision}
        if self.legal_basis is not None:
            result["legal_basis"] = self.legal_basis
        if self.authorization_evidence is not None:
            result["authorization_evidence"] = self.authorization_evidence
        return result


@dataclass(frozen=True)
class SecurityPrivacyIdentityArgs:
    id: str
    principal: str
    role: str
    authentication: str
    mfa: bool = False
    least_privilege: bool = False
    assets: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name in ("id", "principal", "role", "authentication"):
            _text(f"identity.{name}", getattr(self, name))
        object.__setattr__(self, "mfa", _bool("identity.mfa", self.mfa))
        object.__setattr__(self, "least_privilege", _bool("identity.least_privilege", self.least_privilege))
        object.__setattr__(self, "assets", _strings("identity.assets", self.assets))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyIdentityArgs":
        raw = _mapping("security/privacy identity", value)
        return cls(_text("identity.id", raw.get("id")), _text("identity.principal", raw.get("principal")), _text("identity.role", raw.get("role")), _text("identity.authentication", raw.get("authentication")), raw.get("mfa", False), raw.get("least_privilege", False), _strings("identity.assets", raw.get("assets", [])))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "principal": self.principal, "role": self.role, "authentication": self.authentication, "mfa": self.mfa, "least_privilege": self.least_privilege, "assets": list(self.assets)}


@dataclass(frozen=True)
class SecurityPrivacyThreatArgs:
    id: str
    category: str
    severity: SecurityPrivacyThreatSeverity
    status: SecurityPrivacyThreatStatus
    control: str | None = None
    evidence_digest: str | None = None
    rationale: str | None = None

    def __post_init__(self) -> None:
        _text("threat.id", self.id)
        _text("threat.category", self.category)
        object.__setattr__(self, "severity", _enum("threat.severity", self.severity, frozenset({"low", "medium", "high", "critical"})))
        object.__setattr__(self, "status", _enum("threat.status", self.status, frozenset({"mitigated", "accepted", "unmitigated", "unanalysed"})))
        object.__setattr__(self, "control", _text("threat.control", self.control, required=False))
        object.__setattr__(self, "evidence_digest", None if self.evidence_digest is None else _digest("threat.evidence_digest", self.evidence_digest))
        object.__setattr__(self, "rationale", _text("threat.rationale", self.rationale, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyThreatArgs":
        raw = _mapping("security/privacy threat", value)
        return cls(_text("threat.id", raw.get("id")), _text("threat.category", raw.get("category")), _enum("threat.severity", raw.get("severity"), frozenset({"low", "medium", "high", "critical"})), _enum("threat.status", raw.get("status"), frozenset({"mitigated", "accepted", "unmitigated", "unanalysed"})), raw.get("control"), raw.get("evidence_digest"), raw.get("rationale"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "category": self.category, "severity": self.severity, "status": self.status}
        for name, value in (("control", self.control), ("evidence_digest", self.evidence_digest), ("rationale", self.rationale)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityPrivacyReviewArgs:
    id: str
    kind: SecurityPrivacyReviewKind
    scope: str
    reviewer: str
    status: SecurityPrivacyReviewStatus
    evidence_digest: str | None = None
    expires_at: str | None = None
    findings: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("review.id", self.id)
        object.__setattr__(self, "kind", _enum("review.kind", self.kind, frozenset({"privacy_impact", "security_assessment", "red_team", "access_review"})))
        _text("review.scope", self.scope)
        _text("review.reviewer", self.reviewer)
        object.__setattr__(self, "status", _enum("review.status", self.status, frozenset({"draft", "in_review", "complete", "expired"})))
        object.__setattr__(self, "evidence_digest", None if self.evidence_digest is None else _digest("review.evidence_digest", self.evidence_digest))
        object.__setattr__(self, "expires_at", _text("review.expires_at", self.expires_at, required=False))
        object.__setattr__(self, "findings", _strings("review.findings", self.findings))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyReviewArgs":
        raw = _mapping("security/privacy review", value)
        return cls(_text("review.id", raw.get("id")), _enum("review.kind", raw.get("kind"), frozenset({"privacy_impact", "security_assessment", "red_team", "access_review"})), _text("review.scope", raw.get("scope")), _text("review.reviewer", raw.get("reviewer")), _enum("review.status", raw.get("status"), frozenset({"draft", "in_review", "complete", "expired"})), raw.get("evidence_digest"), raw.get("expires_at"), _strings("review.findings", raw.get("findings", [])))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "kind": self.kind, "scope": self.scope, "reviewer": self.reviewer, "status": self.status, "findings": list(self.findings)}
        if self.evidence_digest is not None:
            result["evidence_digest"] = self.evidence_digest
        if self.expires_at is not None:
            result["expires_at"] = self.expires_at
        return result


@dataclass(frozen=True)
class SecurityPrivacyControlsArgs:
    access_control: bool = False
    encryption_at_rest: bool = False
    encryption_in_transit: bool = False
    key_rotation: bool = False
    audit_logging: bool = False
    vulnerability_management: bool = False
    backup_restore: bool = False
    incident_response: bool = False
    vendor_review: bool = False
    data_subject_rights: bool = False

    def __post_init__(self) -> None:
        for name in ("access_control", "encryption_at_rest", "encryption_in_transit", "key_rotation", "audit_logging", "vulnerability_management", "backup_restore", "incident_response", "vendor_review", "data_subject_rights"):
            _bool(f"controls.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SecurityPrivacyControlsArgs":
        raw = {} if value is None else _mapping("security/privacy controls", value)
        return cls(*(raw.get(name, False) for name in ("access_control", "encryption_at_rest", "encryption_in_transit", "key_rotation", "audit_logging", "vulnerability_management", "backup_restore", "incident_response", "vendor_review", "data_subject_rights")))

    def to_wire(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in ("access_control", "encryption_at_rest", "encryption_in_transit", "key_rotation", "audit_logging", "vulnerability_management", "backup_restore", "incident_response", "vendor_review", "data_subject_rights")}


@dataclass(frozen=True)
class SecurityPrivacyPoliciesArgs:
    require_asset_purpose: bool = True
    require_retention: bool = True
    require_flow_authorization: bool = True
    require_identity_hardening: bool = True
    require_threat_treatment: bool = True
    require_reviews: bool = True
    require_controls: bool = True
    require_mfa_for_sensitive: bool = True

    def __post_init__(self) -> None:
        for name in ("require_asset_purpose", "require_retention", "require_flow_authorization", "require_identity_hardening", "require_threat_treatment", "require_reviews", "require_controls", "require_mfa_for_sensitive"):
            _bool(f"policies.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SecurityPrivacyPoliciesArgs":
        raw = {} if value is None else _mapping("security/privacy policies", value)
        return cls(*(raw.get(name, True) for name in ("require_asset_purpose", "require_retention", "require_flow_authorization", "require_identity_hardening", "require_threat_treatment", "require_reviews", "require_controls", "require_mfa_for_sensitive")))

    def to_wire(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in ("require_asset_purpose", "require_retention", "require_flow_authorization", "require_identity_hardening", "require_threat_treatment", "require_reviews", "require_controls", "require_mfa_for_sensitive")}


@dataclass(frozen=True, init=False)
class SecurityPrivacyManifestArgs:
    schema: str
    system: SecurityPrivacySystemArgs
    assets: tuple[SecurityPrivacyAssetArgs, ...]
    flows: tuple[SecurityPrivacyFlowArgs, ...]
    identities: tuple[SecurityPrivacyIdentityArgs, ...]
    threats: tuple[SecurityPrivacyThreatArgs, ...]
    reviews: tuple[SecurityPrivacyReviewArgs, ...]
    controls: SecurityPrivacyControlsArgs
    policies: SecurityPrivacyPoliciesArgs

    def __init__(self, system: SecurityPrivacySystemArgs | Mapping[str, Any], assets: Sequence[SecurityPrivacyAssetArgs | Mapping[str, Any]] = (), flows: Sequence[SecurityPrivacyFlowArgs | Mapping[str, Any]] = (), identities: Sequence[SecurityPrivacyIdentityArgs | Mapping[str, Any]] = (), threats: Sequence[SecurityPrivacyThreatArgs | Mapping[str, Any]] = (), reviews: Sequence[SecurityPrivacyReviewArgs | Mapping[str, Any]] = (), controls: SecurityPrivacyControlsArgs | Mapping[str, Any] | None = None, policies: SecurityPrivacyPoliciesArgs | Mapping[str, Any] | None = None, schema: str = SECURITY_PRIVACY_MANIFEST_SCHEMA) -> None:
        normalized_schema = _text("security/privacy schema", schema)
        normalized_system = system if isinstance(system, SecurityPrivacySystemArgs) else SecurityPrivacySystemArgs.from_wire(system)
        values = [("assets", assets, SECURITY_PRIVACY_MAX_ASSETS, SecurityPrivacyAssetArgs), ("flows", flows, SECURITY_PRIVACY_MAX_FLOWS, SecurityPrivacyFlowArgs), ("identities", identities, SECURITY_PRIVACY_MAX_IDENTITIES, SecurityPrivacyIdentityArgs), ("threats", threats, SECURITY_PRIVACY_MAX_THREATS, SecurityPrivacyThreatArgs), ("reviews", reviews, SECURITY_PRIVACY_MAX_REVIEWS, SecurityPrivacyReviewArgs)]
        normalized: dict[str, tuple[Any, ...]] = {}
        for name, raw_values, limit, cls in values:
            bounded = _bounded(f"security/privacy {name}", raw_values, limit)
            normalized[name] = tuple(item if isinstance(item, cls) else cls.from_wire(item) for item in bounded)
        normalized_controls = controls if isinstance(controls, SecurityPrivacyControlsArgs) else SecurityPrivacyControlsArgs.from_wire(controls)
        normalized_policies = policies if isinstance(policies, SecurityPrivacyPoliciesArgs) else SecurityPrivacyPoliciesArgs.from_wire(policies)
        wire = {"schema": normalized_schema, "system": normalized_system.to_wire(), **{name: [item.to_wire() for item in items] for name, items in normalized.items()}, "controls": normalized_controls.to_wire(), "policies": normalized_policies.to_wire()}
        _json_size("security/privacy manifest", wire)
        for name, value in (("schema", normalized_schema), ("system", normalized_system), *normalized.items(), ("controls", normalized_controls), ("policies", normalized_policies)):
            object.__setattr__(self, name, value)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyManifestArgs":
        raw = _mapping("security/privacy manifest", value)
        return cls(raw.get("system"), _bounded("security/privacy assets", raw.get("assets", []), SECURITY_PRIVACY_MAX_ASSETS), _bounded("security/privacy flows", raw.get("flows", []), SECURITY_PRIVACY_MAX_FLOWS), _bounded("security/privacy identities", raw.get("identities", []), SECURITY_PRIVACY_MAX_IDENTITIES), _bounded("security/privacy threats", raw.get("threats", []), SECURITY_PRIVACY_MAX_THREATS), _bounded("security/privacy reviews", raw.get("reviews", []), SECURITY_PRIVACY_MAX_REVIEWS), raw.get("controls"), raw.get("policies"), raw.get("schema", SECURITY_PRIVACY_MANIFEST_SCHEMA))

    def to_wire(self) -> dict[str, Any]:
        return {"schema": self.schema, "system": self.system.to_wire(), "assets": [item.to_wire() for item in self.assets], "flows": [item.to_wire() for item in self.flows], "identities": [item.to_wire() for item in self.identities], "threats": [item.to_wire() for item in self.threats], "reviews": [item.to_wire() for item in self.reviews], "controls": self.controls.to_wire(), "policies": self.policies.to_wire()}

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("security/privacy response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            add(nested.get("structuredContent"))
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"security/privacy response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        add(container.get("structuredContent"))

    for key in ("mcp", "result", "structuredContent"):
        add(raw.get(key))
    for candidate in candidates:
        if candidate.get("schema") == SECURITY_PRIVACY_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a security/privacy audit projection")


@dataclass(frozen=True)
class SecurityPrivacyIssueReport:
    code: str
    severity: SecurityPrivacyIssueSeverity
    subject: str
    detail: str
    remediation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyIssueReport":
        raw = _mapping("security/privacy issue", value)
        return cls(_text("security/privacy issue code", raw.get("code")), _enum("security/privacy issue severity", raw.get("severity"), frozenset({"warning", "blocking"})), _text("security/privacy issue subject", raw.get("subject")), _text("security/privacy issue detail", raw.get("detail")), _text("security/privacy issue remediation", raw.get("remediation")))  # type: ignore[arg-type]


def _report_bool(name: str, raw: Mapping[str, Any]) -> bool:
    return _bool(name, raw.get(name.rsplit(" ", 1)[-1]))


@dataclass(frozen=True)
class SecurityPrivacyAssetAuditReport:
    asset_id: str
    purpose_valid: bool
    retention_valid: bool
    residency_valid: bool
    deletion_valid: bool
    sensitive: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyAssetAuditReport":
        raw = _mapping("security/privacy asset audit", value)
        return cls(_text("asset audit asset_id", raw.get("asset_id")), *(_bool(f"asset audit {name}", raw.get(name)) for name in ("purpose_valid", "retention_valid", "residency_valid", "deletion_valid", "sensitive", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityPrivacyFlowAuditReport:
    flow_id: str
    asset_valid: bool
    purpose_valid: bool
    legal_basis_present: bool
    authorization_present: bool
    allowed: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyFlowAuditReport":
        raw = _mapping("security/privacy flow audit", value)
        return cls(_text("flow audit flow_id", raw.get("flow_id")), *(_bool(f"flow audit {name}", raw.get(name)) for name in ("asset_valid", "purpose_valid", "legal_basis_present", "authorization_present", "allowed", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityPrivacyIdentityAuditReport:
    identity_id: str
    assets_valid: bool
    authentication_valid: bool
    mfa: bool
    least_privilege: bool
    sensitive_access: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyIdentityAuditReport":
        raw = _mapping("security/privacy identity audit", value)
        return cls(_text("identity audit identity_id", raw.get("identity_id")), *(_bool(f"identity audit {name}", raw.get(name)) for name in ("assets_valid", "authentication_valid", "mfa", "least_privilege", "sensitive_access", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityPrivacyThreatAuditReport:
    threat_id: str
    high_or_worse: bool
    treated: bool
    control_present: bool
    evidence_valid: bool
    rationale_present: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyThreatAuditReport":
        raw = _mapping("security/privacy threat audit", value)
        return cls(_text("threat audit threat_id", raw.get("threat_id")), *(_bool(f"threat audit {name}", raw.get(name)) for name in ("high_or_worse", "treated", "control_present", "evidence_valid", "rationale_present", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityPrivacyReviewAuditReport:
    review_id: str
    reviewer_independent: bool
    evidence_valid: bool
    current: bool
    complete: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyReviewAuditReport":
        raw = _mapping("security/privacy review audit", value)
        return cls(_text("review audit review_id", raw.get("review_id")), *(_bool(f"review audit {name}", raw.get(name)) for name in ("reviewer_independent", "evidence_valid", "current", "complete", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityPrivacyControlAuditReport:
    control: str
    enabled: bool
    required: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyControlAuditReport":
        raw = _mapping("security/privacy control audit", value)
        return cls(_text("control audit control", raw.get("control")), _bool("control audit enabled", raw.get("enabled")), _bool("control audit required", raw.get("required")), _bool("control audit ready", raw.get("ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityPrivacyAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    valid: bool | None
    security_privacy_ready_value: bool | None
    counts: Mapping[str, Any] | None
    asset_audits: tuple[SecurityPrivacyAssetAuditReport, ...]
    flow_audits: tuple[SecurityPrivacyFlowAuditReport, ...]
    identity_audits: tuple[SecurityPrivacyIdentityAuditReport, ...]
    threat_audits: tuple[SecurityPrivacyThreatAuditReport, ...]
    review_audits: tuple[SecurityPrivacyReviewAuditReport, ...]
    control_audits: tuple[SecurityPrivacyControlAuditReport, ...]
    issues: tuple[SecurityPrivacyIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityPrivacyAuditReport":
        raw = _payload(value)
        if raw.get("ok") is not True:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("security/privacy refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), raw.get("manifest_digest"), False, False, None, (), (), (), (), (), (), (), _route_strings("security/privacy refusal guarantees", raw.get("guarantees", [])), _route_strings("security/privacy refusal limitations", raw.get("limitations", [])), raw.get("refusal") or raw.get("error"), True)
        if raw.get("schema") != SECURITY_PRIVACY_AUDIT_SCHEMA:
            raise ArgumentError("security/privacy projection has an invalid schema")
        audit = _mapping("security/privacy audit", raw.get("audit"))
        return cls(raw, True, SECURITY_PRIVACY_AUDIT_SCHEMA, _text("security/privacy workflow", raw.get("workflow")), _text("security/privacy manifest_digest", raw.get("manifest_digest"), required=False), _bool("security/privacy valid", audit.get("valid")), _bool("security_privacy_ready", raw.get("security_privacy_ready")), _mapping("security/privacy counts", audit.get("counts")), tuple(SecurityPrivacyAssetAuditReport.from_wire(item) for item in _bounded("security/privacy asset audits", audit.get("asset_audits", []), SECURITY_PRIVACY_MAX_ASSETS)), tuple(SecurityPrivacyFlowAuditReport.from_wire(item) for item in _bounded("security/privacy flow audits", audit.get("flow_audits", []), SECURITY_PRIVACY_MAX_FLOWS)), tuple(SecurityPrivacyIdentityAuditReport.from_wire(item) for item in _bounded("security/privacy identity audits", audit.get("identity_audits", []), SECURITY_PRIVACY_MAX_IDENTITIES)), tuple(SecurityPrivacyThreatAuditReport.from_wire(item) for item in _bounded("security/privacy threat audits", audit.get("threat_audits", []), SECURITY_PRIVACY_MAX_THREATS)), tuple(SecurityPrivacyReviewAuditReport.from_wire(item) for item in _bounded("security/privacy review audits", audit.get("review_audits", []), SECURITY_PRIVACY_MAX_REVIEWS)), tuple(SecurityPrivacyControlAuditReport.from_wire(item) for item in _bounded("security/privacy control audits", audit.get("control_audits", []), SECURITY_PRIVACY_MAX_LIST_ITEMS)), tuple(SecurityPrivacyIssueReport.from_wire(item) for item in _bounded("security/privacy issues", audit.get("issues", []), SECURITY_PRIVACY_MAX_LIST_ITEMS)), _route_strings("security/privacy guarantees", raw.get("guarantees", audit.get("guarantees", []))), _route_strings("security/privacy limitations", raw.get("limitations", audit.get("limitations", []))), None, False)

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.security_privacy_ready_value is True

    @property
    def security_privacy_ready(self) -> bool:
        return self.security_privacy_ready_value is True

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def blocking_issues(self) -> tuple[SecurityPrivacyIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[SecurityPrivacyIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def security_privacy_audit_report(value: Mapping[str, Any]) -> SecurityPrivacyAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return SecurityPrivacyAuditReport.from_wire(value)


__all__ = [
    "SECURITY_PRIVACY_MANIFEST_SCHEMA", "SECURITY_PRIVACY_AUDIT_SCHEMA", "SECURITY_PRIVACY_MAX_INPUT_BYTES", "SECURITY_PRIVACY_MAX_ASSETS", "SECURITY_PRIVACY_MAX_FLOWS", "SECURITY_PRIVACY_MAX_IDENTITIES", "SECURITY_PRIVACY_MAX_THREATS", "SECURITY_PRIVACY_MAX_REVIEWS",
    "SecurityPrivacySystemArgs", "SecurityPrivacyAssetArgs", "SecurityPrivacyFlowArgs", "SecurityPrivacyIdentityArgs", "SecurityPrivacyThreatArgs", "SecurityPrivacyReviewArgs", "SecurityPrivacyControlsArgs", "SecurityPrivacyPoliciesArgs", "SecurityPrivacyManifestArgs", "SecurityPrivacyIssueReport", "SecurityPrivacyAssetAuditReport", "SecurityPrivacyFlowAuditReport", "SecurityPrivacyIdentityAuditReport", "SecurityPrivacyThreatAuditReport", "SecurityPrivacyReviewAuditReport", "SecurityPrivacyControlAuditReport", "SecurityPrivacyAuditReport", "security_privacy_audit_report",
]
