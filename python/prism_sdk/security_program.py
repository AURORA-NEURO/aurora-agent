"""Typed security, safety, and red-team program manifests and audit projections."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


SECURITY_PROGRAM_MANIFEST_SCHEMA = "bioprism-security-program/0.1"
SECURITY_PROGRAM_AUDIT_SCHEMA = "bioprism-security-program-audit/0.1"
SECURITY_PROGRAM_MAX_INPUT_BYTES = 20_000_000
SECURITY_PROGRAM_MAX_SCOPES = 4_096
SECURITY_PROGRAM_MAX_CAMPAIGNS = 8_192
SECURITY_PROGRAM_MAX_FINDINGS = 16_384
SECURITY_PROGRAM_MAX_REMEDIATIONS = 16_384
SECURITY_PROGRAM_MAX_INCIDENTS = 8_192
SECURITY_PROGRAM_MAX_DISCLOSURES = 8_192
SECURITY_PROGRAM_MAX_LIST_ITEMS = 32_768
SECURITY_PROGRAM_MAX_TEXT_BYTES = 4_096

SecurityProgramScopeKind = Literal["service", "api", "model", "dataset", "workflow", "research_artifact", "vendor", "organization"]
SecurityProgramCampaignStatus = Literal["planned", "running", "completed", "stopped", "cancelled"]
SecurityProgramFindingSeverity = Literal["informational", "low", "medium", "high", "critical"]
SecurityProgramFindingStatus = Literal["new", "triaged", "accepted", "remediated", "closed", "false_positive", "duplicate"]
SecurityProgramRemediationStatus = Literal["open", "in_progress", "blocked", "complete", "waived"]
SecurityProgramIncidentStatus = Literal["open", "contained", "closed", "accepted"]
SecurityProgramDisclosureStage = Literal["withheld", "internal", "advisory", "public"]
SecurityProgramIssueSeverity = Literal["warning", "blocking"]


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
    if len(result.encode("utf-8")) > SECURITY_PROGRAM_MAX_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {SECURITY_PROGRAM_MAX_TEXT_BYTES} UTF-8 bytes")
    return result


def _strings(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(_bounded(name, value, SECURITY_PROGRAM_MAX_LIST_ITEMS)))  # type: ignore[misc]


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


def _optional_digest(name: str, value: Any) -> str | None:
    return None if value is None else _digest(name, value)


def _bounded_texts(name: str, value: Any) -> tuple[str, ...]:
    values = _strings(name, value)
    for index, item in enumerate(values):
        if "*" in item or ".." in item:
            raise ArgumentError(f"{name}[{index}] must be a bounded non-wildcard value")
    return values


def _json_size(name: str, value: Any) -> None:
    try:
        encoded = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable: {error}") from error
    if len(encoded) > SECURITY_PROGRAM_MAX_INPUT_BYTES:
        raise ArgumentError(f"{name} exceeds the {SECURITY_PROGRAM_MAX_INPUT_BYTES}-byte safety bound")


SCOPE_KINDS = frozenset({"service", "api", "model", "dataset", "workflow", "research_artifact", "vendor", "organization"})
CAMPAIGN_STATUSES = frozenset({"planned", "running", "completed", "stopped", "cancelled"})
FINDING_SEVERITIES = frozenset({"informational", "low", "medium", "high", "critical"})
FINDING_STATUSES = frozenset({"new", "triaged", "accepted", "remediated", "closed", "false_positive", "duplicate"})
REMEDIATION_STATUSES = frozenset({"open", "in_progress", "blocked", "complete", "waived"})
INCIDENT_STATUSES = frozenset({"open", "contained", "closed", "accepted"})
DISCLOSURE_STAGES = frozenset({"withheld", "internal", "advisory", "public"})


@dataclass(frozen=True)
class SecurityProgramSystemArgs:
    id: str
    version: str
    owner: str
    mission: str

    def __post_init__(self) -> None:
        for name in ("id", "version", "owner", "mission"):
            _text(f"system.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramSystemArgs":
        raw = _mapping("security program system", value)
        return cls(_text("system.id", raw.get("id")), _text("system.version", raw.get("version")), _text("system.owner", raw.get("owner")), _text("system.mission", raw.get("mission")))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version, "owner": self.owner, "mission": self.mission}


@dataclass(frozen=True)
class SecurityProgramScopeArgs:
    id: str
    name: str
    kind: SecurityProgramScopeKind
    target: str
    owner: str
    authorization_digest: str | None = None
    allowed_methods: tuple[str, ...] = ()
    forbidden_actions: tuple[str, ...] = ()
    environments: tuple[str, ...] = ()
    data_handling: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "name", "target", "owner"):
            _text(f"scope.{name}", getattr(self, name))
        object.__setattr__(self, "kind", _enum("scope.kind", self.kind, SCOPE_KINDS))
        object.__setattr__(self, "authorization_digest", _optional_digest("scope.authorization_digest", self.authorization_digest))
        object.__setattr__(self, "allowed_methods", _bounded_texts("scope.allowed_methods", self.allowed_methods))
        object.__setattr__(self, "forbidden_actions", _bounded_texts("scope.forbidden_actions", self.forbidden_actions))
        object.__setattr__(self, "environments", _bounded_texts("scope.environments", self.environments))
        object.__setattr__(self, "data_handling", _text("scope.data_handling", self.data_handling, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramScopeArgs":
        raw = _mapping("security program scope", value)
        return cls(_text("scope.id", raw.get("id")), _text("scope.name", raw.get("name")), _enum("scope.kind", raw.get("kind"), SCOPE_KINDS), _text("scope.target", raw.get("target")), _text("scope.owner", raw.get("owner")), raw.get("authorization_digest"), _bounded_texts("scope.allowed_methods", raw.get("allowed_methods", [])), _bounded_texts("scope.forbidden_actions", raw.get("forbidden_actions", [])), _bounded_texts("scope.environments", raw.get("environments", [])), raw.get("data_handling"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "name": self.name, "kind": self.kind, "target": self.target, "owner": self.owner, "allowed_methods": list(self.allowed_methods), "forbidden_actions": list(self.forbidden_actions), "environments": list(self.environments)}
        for name, value in (("authorization_digest", self.authorization_digest), ("data_handling", self.data_handling)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityProgramCampaignArgs:
    id: str
    scope: str
    operator: str
    methodology: str
    hypothesis: str
    status: SecurityProgramCampaignStatus
    independent_reviewer: str | None = None
    started_at: str | None = None
    completed_at: str | None = None
    evidence_digest: str | None = None
    stop_conditions: tuple[str, ...] = ()
    finding_ids: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name in ("id", "scope", "operator", "methodology", "hypothesis"):
            _text(f"campaign.{name}", getattr(self, name))
        object.__setattr__(self, "status", _enum("campaign.status", self.status, CAMPAIGN_STATUSES))
        for name in ("independent_reviewer", "started_at", "completed_at"):
            object.__setattr__(self, name, _text(f"campaign.{name}", getattr(self, name), required=False))
        object.__setattr__(self, "evidence_digest", _optional_digest("campaign.evidence_digest", self.evidence_digest))
        object.__setattr__(self, "stop_conditions", _bounded_texts("campaign.stop_conditions", self.stop_conditions))
        object.__setattr__(self, "finding_ids", _bounded_texts("campaign.finding_ids", self.finding_ids))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramCampaignArgs":
        raw = _mapping("security program campaign", value)
        return cls(_text("campaign.id", raw.get("id")), _text("campaign.scope", raw.get("scope")), _text("campaign.operator", raw.get("operator")), _text("campaign.methodology", raw.get("methodology")), _text("campaign.hypothesis", raw.get("hypothesis")), _enum("campaign.status", raw.get("status"), CAMPAIGN_STATUSES), raw.get("independent_reviewer"), raw.get("started_at"), raw.get("completed_at"), raw.get("evidence_digest"), _bounded_texts("campaign.stop_conditions", raw.get("stop_conditions", [])), _bounded_texts("campaign.finding_ids", raw.get("finding_ids", [])))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "scope": self.scope, "operator": self.operator, "methodology": self.methodology, "hypothesis": self.hypothesis, "status": self.status, "stop_conditions": list(self.stop_conditions), "finding_ids": list(self.finding_ids)}
        for name, value in (("independent_reviewer", self.independent_reviewer), ("started_at", self.started_at), ("completed_at", self.completed_at), ("evidence_digest", self.evidence_digest)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityProgramFindingArgs:
    id: str
    campaign: str
    title: str
    severity: SecurityProgramFindingSeverity
    status: SecurityProgramFindingStatus
    discovered_at: str
    evidence_digest: str | None = None
    reproduction_digest: str | None = None
    regression_digest: str | None = None
    affected_targets: tuple[str, ...] = ()
    remediation_ids: tuple[str, ...] = ()
    incident_id: str | None = None
    public_safe: bool = False
    resolution_note: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "campaign", "title", "discovered_at"):
            _text(f"finding.{name}", getattr(self, name))
        object.__setattr__(self, "severity", _enum("finding.severity", self.severity, FINDING_SEVERITIES))
        object.__setattr__(self, "status", _enum("finding.status", self.status, FINDING_STATUSES))
        object.__setattr__(self, "evidence_digest", _optional_digest("finding.evidence_digest", self.evidence_digest))
        object.__setattr__(self, "reproduction_digest", _optional_digest("finding.reproduction_digest", self.reproduction_digest))
        object.__setattr__(self, "regression_digest", _optional_digest("finding.regression_digest", self.regression_digest))
        object.__setattr__(self, "affected_targets", _bounded_texts("finding.affected_targets", self.affected_targets))
        object.__setattr__(self, "remediation_ids", _bounded_texts("finding.remediation_ids", self.remediation_ids))
        object.__setattr__(self, "incident_id", _text("finding.incident_id", self.incident_id, required=False))
        object.__setattr__(self, "public_safe", _bool("finding.public_safe", self.public_safe))
        object.__setattr__(self, "resolution_note", _text("finding.resolution_note", self.resolution_note, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramFindingArgs":
        raw = _mapping("security program finding", value)
        return cls(_text("finding.id", raw.get("id")), _text("finding.campaign", raw.get("campaign")), _text("finding.title", raw.get("title")), _enum("finding.severity", raw.get("severity"), FINDING_SEVERITIES), _enum("finding.status", raw.get("status"), FINDING_STATUSES), _text("finding.discovered_at", raw.get("discovered_at")), raw.get("evidence_digest"), raw.get("reproduction_digest"), raw.get("regression_digest"), _bounded_texts("finding.affected_targets", raw.get("affected_targets", [])), _bounded_texts("finding.remediation_ids", raw.get("remediation_ids", [])), raw.get("incident_id"), raw.get("public_safe", False), raw.get("resolution_note"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "campaign": self.campaign, "title": self.title, "severity": self.severity, "status": self.status, "discovered_at": self.discovered_at, "affected_targets": list(self.affected_targets), "remediation_ids": list(self.remediation_ids), "public_safe": self.public_safe}
        for name, value in (("evidence_digest", self.evidence_digest), ("reproduction_digest", self.reproduction_digest), ("regression_digest", self.regression_digest), ("incident_id", self.incident_id), ("resolution_note", self.resolution_note)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityProgramRemediationArgs:
    id: str
    finding: str
    owner: str
    action: str
    status: SecurityProgramRemediationStatus
    due_at: str
    verification_digest: str | None = None
    rationale: str | None = None
    approval_digest: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "finding", "owner", "action", "due_at"):
            _text(f"remediation.{name}", getattr(self, name))
        object.__setattr__(self, "status", _enum("remediation.status", self.status, REMEDIATION_STATUSES))
        object.__setattr__(self, "verification_digest", _optional_digest("remediation.verification_digest", self.verification_digest))
        object.__setattr__(self, "rationale", _text("remediation.rationale", self.rationale, required=False))
        object.__setattr__(self, "approval_digest", _optional_digest("remediation.approval_digest", self.approval_digest))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramRemediationArgs":
        raw = _mapping("security program remediation", value)
        return cls(_text("remediation.id", raw.get("id")), _text("remediation.finding", raw.get("finding")), _text("remediation.owner", raw.get("owner")), _text("remediation.action", raw.get("action")), _enum("remediation.status", raw.get("status"), REMEDIATION_STATUSES), _text("remediation.due_at", raw.get("due_at")), raw.get("verification_digest"), raw.get("rationale"), raw.get("approval_digest"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "finding": self.finding, "owner": self.owner, "action": self.action, "status": self.status, "due_at": self.due_at}
        for name, value in (("verification_digest", self.verification_digest), ("rationale", self.rationale), ("approval_digest", self.approval_digest)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityProgramTimelineEventArgs:
    epoch: int
    actor: str
    event: str
    evidence_digest: str | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.epoch, int) or isinstance(self.epoch, bool) or self.epoch < 0:
            raise ArgumentError("timeline.epoch must be a non-negative integer")
        _text("timeline.actor", self.actor)
        _text("timeline.event", self.event)
        object.__setattr__(self, "evidence_digest", _optional_digest("timeline.evidence_digest", self.evidence_digest))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramTimelineEventArgs":
        raw = _mapping("security program timeline event", value)
        return cls(raw.get("epoch"), _text("timeline.actor", raw.get("actor")), _text("timeline.event", raw.get("event")), raw.get("evidence_digest"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"epoch": self.epoch, "actor": self.actor, "event": self.event}
        if self.evidence_digest is not None:
            result["evidence_digest"] = self.evidence_digest
        return result


@dataclass(frozen=True)
class SecurityProgramIncidentArgs:
    id: str
    finding: str
    severity: SecurityProgramFindingSeverity
    owner: str
    status: SecurityProgramIncidentStatus
    opened_at: str
    contained_at: str | None = None
    closed_at: str | None = None
    containment_evidence: str | None = None
    closure_evidence: str | None = None
    notification_required: bool = False
    timeline: tuple[SecurityProgramTimelineEventArgs, ...] = ()

    def __post_init__(self) -> None:
        for name in ("id", "finding", "owner", "opened_at"):
            _text(f"incident.{name}", getattr(self, name))
        object.__setattr__(self, "severity", _enum("incident.severity", self.severity, FINDING_SEVERITIES))
        object.__setattr__(self, "status", _enum("incident.status", self.status, INCIDENT_STATUSES))
        for name in ("contained_at", "closed_at"):
            object.__setattr__(self, name, _text(f"incident.{name}", getattr(self, name), required=False))
        object.__setattr__(self, "containment_evidence", _optional_digest("incident.containment_evidence", self.containment_evidence))
        object.__setattr__(self, "closure_evidence", _optional_digest("incident.closure_evidence", self.closure_evidence))
        object.__setattr__(self, "notification_required", _bool("incident.notification_required", self.notification_required))
        values = _bounded("incident.timeline", self.timeline, SECURITY_PROGRAM_MAX_LIST_ITEMS)
        object.__setattr__(self, "timeline", tuple(item if isinstance(item, SecurityProgramTimelineEventArgs) else SecurityProgramTimelineEventArgs.from_wire(item) for item in values))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramIncidentArgs":
        raw = _mapping("security program incident", value)
        return cls(_text("incident.id", raw.get("id")), _text("incident.finding", raw.get("finding")), _enum("incident.severity", raw.get("severity"), FINDING_SEVERITIES), _text("incident.owner", raw.get("owner")), _enum("incident.status", raw.get("status"), INCIDENT_STATUSES), _text("incident.opened_at", raw.get("opened_at")), raw.get("contained_at"), raw.get("closed_at"), raw.get("containment_evidence"), raw.get("closure_evidence"), raw.get("notification_required", False), _bounded("incident.timeline", raw.get("timeline", []), SECURITY_PROGRAM_MAX_LIST_ITEMS))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "finding": self.finding, "severity": self.severity, "owner": self.owner, "status": self.status, "opened_at": self.opened_at, "notification_required": self.notification_required, "timeline": [item.to_wire() for item in self.timeline]}
        for name, value in (("contained_at", self.contained_at), ("closed_at", self.closed_at), ("containment_evidence", self.containment_evidence), ("closure_evidence", self.closure_evidence)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityProgramDisclosureArgs:
    id: str
    finding: str
    stage: SecurityProgramDisclosureStage
    audience: str
    requested_at: str
    approver: str | None = None
    approval_digest: str | None = None
    advisory_digest: str | None = None
    published_at: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "finding", "audience", "requested_at"):
            _text(f"disclosure.{name}", getattr(self, name))
        object.__setattr__(self, "stage", _enum("disclosure.stage", self.stage, DISCLOSURE_STAGES))
        object.__setattr__(self, "approver", _text("disclosure.approver", self.approver, required=False))
        object.__setattr__(self, "approval_digest", _optional_digest("disclosure.approval_digest", self.approval_digest))
        object.__setattr__(self, "advisory_digest", _optional_digest("disclosure.advisory_digest", self.advisory_digest))
        object.__setattr__(self, "published_at", _text("disclosure.published_at", self.published_at, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramDisclosureArgs":
        raw = _mapping("security program disclosure", value)
        return cls(_text("disclosure.id", raw.get("id")), _text("disclosure.finding", raw.get("finding")), _enum("disclosure.stage", raw.get("stage"), DISCLOSURE_STAGES), _text("disclosure.audience", raw.get("audience")), _text("disclosure.requested_at", raw.get("requested_at")), raw.get("approver"), raw.get("approval_digest"), raw.get("advisory_digest"), raw.get("published_at"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "finding": self.finding, "stage": self.stage, "audience": self.audience, "requested_at": self.requested_at}
        for name, value in (("approver", self.approver), ("approval_digest", self.approval_digest), ("advisory_digest", self.advisory_digest), ("published_at", self.published_at)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class SecurityProgramControlsArgs:
    scope_authorization: bool = False
    operator_separation: bool = False
    independent_review: bool = False
    evidence_retention: bool = False
    remediation_tracking: bool = False
    incident_response: bool = False
    disclosure_review: bool = False
    regression_testing: bool = False

    def __post_init__(self) -> None:
        for name in ("scope_authorization", "operator_separation", "independent_review", "evidence_retention", "remediation_tracking", "incident_response", "disclosure_review", "regression_testing"):
            _bool(f"controls.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SecurityProgramControlsArgs":
        raw = {} if value is None else _mapping("security program controls", value)
        return cls(*(raw.get(name, False) for name in ("scope_authorization", "operator_separation", "independent_review", "evidence_retention", "remediation_tracking", "incident_response", "disclosure_review", "regression_testing")))

    def to_wire(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in ("scope_authorization", "operator_separation", "independent_review", "evidence_retention", "remediation_tracking", "incident_response", "disclosure_review", "regression_testing")}


@dataclass(frozen=True)
class SecurityProgramPoliciesArgs:
    require_scope_authorization: bool = True
    require_independent_review: bool = True
    require_campaign_evidence: bool = True
    require_finding_evidence: bool = True
    require_remediation: bool = True
    require_incident_for_high: bool = True
    require_disclosure_approval: bool = True
    require_regression_for_closed: bool = True
    require_controls: bool = True

    def __post_init__(self) -> None:
        for name in ("require_scope_authorization", "require_independent_review", "require_campaign_evidence", "require_finding_evidence", "require_remediation", "require_incident_for_high", "require_disclosure_approval", "require_regression_for_closed", "require_controls"):
            _bool(f"policies.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SecurityProgramPoliciesArgs":
        raw = {} if value is None else _mapping("security program policies", value)
        return cls(*(raw.get(name, True) for name in ("require_scope_authorization", "require_independent_review", "require_campaign_evidence", "require_finding_evidence", "require_remediation", "require_incident_for_high", "require_disclosure_approval", "require_regression_for_closed", "require_controls")))

    def to_wire(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in ("require_scope_authorization", "require_independent_review", "require_campaign_evidence", "require_finding_evidence", "require_remediation", "require_incident_for_high", "require_disclosure_approval", "require_regression_for_closed", "require_controls")}


@dataclass(frozen=True, init=False)
class SecurityProgramManifestArgs:
    schema: str
    system: SecurityProgramSystemArgs
    scopes: tuple[SecurityProgramScopeArgs, ...]
    campaigns: tuple[SecurityProgramCampaignArgs, ...]
    findings: tuple[SecurityProgramFindingArgs, ...]
    remediations: tuple[SecurityProgramRemediationArgs, ...]
    incidents: tuple[SecurityProgramIncidentArgs, ...]
    disclosures: tuple[SecurityProgramDisclosureArgs, ...]
    controls: SecurityProgramControlsArgs
    policies: SecurityProgramPoliciesArgs

    def __init__(self, system: SecurityProgramSystemArgs | Mapping[str, Any], scopes: Sequence[SecurityProgramScopeArgs | Mapping[str, Any]] = (), campaigns: Sequence[SecurityProgramCampaignArgs | Mapping[str, Any]] = (), findings: Sequence[SecurityProgramFindingArgs | Mapping[str, Any]] = (), remediations: Sequence[SecurityProgramRemediationArgs | Mapping[str, Any]] = (), incidents: Sequence[SecurityProgramIncidentArgs | Mapping[str, Any]] = (), disclosures: Sequence[SecurityProgramDisclosureArgs | Mapping[str, Any]] = (), controls: SecurityProgramControlsArgs | Mapping[str, Any] | None = None, policies: SecurityProgramPoliciesArgs | Mapping[str, Any] | None = None, schema: str = SECURITY_PROGRAM_MANIFEST_SCHEMA) -> None:
        normalized_schema = _text("security program schema", schema)
        normalized_system = system if isinstance(system, SecurityProgramSystemArgs) else SecurityProgramSystemArgs.from_wire(system)
        specs = [("scopes", scopes, SECURITY_PROGRAM_MAX_SCOPES, SecurityProgramScopeArgs), ("campaigns", campaigns, SECURITY_PROGRAM_MAX_CAMPAIGNS, SecurityProgramCampaignArgs), ("findings", findings, SECURITY_PROGRAM_MAX_FINDINGS, SecurityProgramFindingArgs), ("remediations", remediations, SECURITY_PROGRAM_MAX_REMEDIATIONS, SecurityProgramRemediationArgs), ("incidents", incidents, SECURITY_PROGRAM_MAX_INCIDENTS, SecurityProgramIncidentArgs), ("disclosures", disclosures, SECURITY_PROGRAM_MAX_DISCLOSURES, SecurityProgramDisclosureArgs)]
        normalized: dict[str, tuple[Any, ...]] = {}
        for name, raw_values, limit, klass in specs:
            bounded = _bounded(f"security program {name}", raw_values, limit)
            normalized[name] = tuple(item if isinstance(item, klass) else klass.from_wire(item) for item in bounded)
        normalized_controls = controls if isinstance(controls, SecurityProgramControlsArgs) else SecurityProgramControlsArgs.from_wire(controls)
        normalized_policies = policies if isinstance(policies, SecurityProgramPoliciesArgs) else SecurityProgramPoliciesArgs.from_wire(policies)
        wire = {"schema": normalized_schema, "system": normalized_system.to_wire(), **{name: [item.to_wire() for item in items] for name, items in normalized.items()}, "controls": normalized_controls.to_wire(), "policies": normalized_policies.to_wire()}
        _json_size("security program manifest", wire)
        for name, value in (("schema", normalized_schema), ("system", normalized_system), *normalized.items(), ("controls", normalized_controls), ("policies", normalized_policies)):
            object.__setattr__(self, name, value)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramManifestArgs":
        raw = _mapping("security program manifest", value)
        return cls(raw.get("system"), _bounded("security program scopes", raw.get("scopes", []), SECURITY_PROGRAM_MAX_SCOPES), _bounded("security program campaigns", raw.get("campaigns", []), SECURITY_PROGRAM_MAX_CAMPAIGNS), _bounded("security program findings", raw.get("findings", []), SECURITY_PROGRAM_MAX_FINDINGS), _bounded("security program remediations", raw.get("remediations", []), SECURITY_PROGRAM_MAX_REMEDIATIONS), _bounded("security program incidents", raw.get("incidents", []), SECURITY_PROGRAM_MAX_INCIDENTS), _bounded("security program disclosures", raw.get("disclosures", []), SECURITY_PROGRAM_MAX_DISCLOSURES), raw.get("controls"), raw.get("policies"), raw.get("schema", SECURITY_PROGRAM_MANIFEST_SCHEMA))

    def to_wire(self) -> dict[str, Any]:
        return {"schema": self.schema, "system": self.system.to_wire(), "scopes": [item.to_wire() for item in self.scopes], "campaigns": [item.to_wire() for item in self.campaigns], "findings": [item.to_wire() for item in self.findings], "remediations": [item.to_wire() for item in self.remediations], "incidents": [item.to_wire() for item in self.incidents], "disclosures": [item.to_wire() for item in self.disclosures], "controls": self.controls.to_wire(), "policies": self.policies.to_wire()}

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("security program response", value)
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
                            raise ArgumentError(f"security program response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        add(container.get("structuredContent"))

    for key in ("mcp", "result", "structuredContent"):
        add(raw.get(key))
    for candidate in candidates:
        if candidate.get("schema") == SECURITY_PROGRAM_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a security program audit projection")


@dataclass(frozen=True)
class SecurityProgramIssueReport:
    code: str
    severity: SecurityProgramIssueSeverity
    subject: str
    detail: str
    remediation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramIssueReport":
        raw = _mapping("security program issue", value)
        return cls(_text("security program issue code", raw.get("code")), _enum("security program issue severity", raw.get("severity"), frozenset({"warning", "blocking"})), _text("security program issue subject", raw.get("subject")), _text("security program issue detail", raw.get("detail")), _text("security program issue remediation", raw.get("remediation")))  # type: ignore[arg-type]


def _row_mapping(name: str, value: Any) -> dict[str, Any]:
    return _mapping(f"security program {name} row", value)


def _row_bools(name: str, raw: Mapping[str, Any], names: Sequence[str]) -> tuple[bool, ...]:
    return tuple(_bool(f"security program {name}.{field}", raw.get(field)) for field in names)


@dataclass(frozen=True)
class SecurityProgramScopeAuditReport:
    scope_id: str
    authorization_valid: bool
    methods_valid: bool
    guardrails_valid: bool
    environments_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramScopeAuditReport":
        raw = _row_mapping("scope audit", value)
        return cls(_text("scope audit.scope_id", raw.get("scope_id")), *_row_bools("scope audit", raw, ("authorization_valid", "methods_valid", "guardrails_valid", "environments_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramCampaignAuditReport:
    campaign_id: str
    scope_valid: bool
    operator_present: bool
    independent_review_valid: bool
    methodology_valid: bool
    evidence_valid: bool
    complete: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramCampaignAuditReport":
        raw = _row_mapping("campaign audit", value)
        return cls(_text("campaign audit.campaign_id", raw.get("campaign_id")), *_row_bools("campaign audit", raw, ("scope_valid", "operator_present", "independent_review_valid", "methodology_valid", "evidence_valid", "complete", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramFindingAuditReport:
    finding_id: str
    campaign_valid: bool
    evidence_valid: bool
    reproduction_valid: bool
    severity_requires_action: bool
    remediation_valid: bool
    incident_required: bool
    incident_valid: bool
    regression_present: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramFindingAuditReport":
        raw = _row_mapping("finding audit", value)
        return cls(_text("finding audit.finding_id", raw.get("finding_id")), *_row_bools("finding audit", raw, ("campaign_valid", "evidence_valid", "reproduction_valid", "severity_requires_action", "remediation_valid", "incident_required", "incident_valid", "regression_present", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramRemediationAuditReport:
    remediation_id: str
    finding_valid: bool
    owner_valid: bool
    completion_valid: bool
    verification_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramRemediationAuditReport":
        raw = _row_mapping("remediation audit", value)
        return cls(_text("remediation audit.remediation_id", raw.get("remediation_id")), *_row_bools("remediation audit", raw, ("finding_valid", "owner_valid", "completion_valid", "verification_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramIncidentAuditReport:
    incident_id: str
    finding_valid: bool
    timeline_valid: bool
    containment_valid: bool
    closure_valid: bool
    notification_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramIncidentAuditReport":
        raw = _row_mapping("incident audit", value)
        return cls(_text("incident audit.incident_id", raw.get("incident_id")), *_row_bools("incident audit", raw, ("finding_valid", "timeline_valid", "containment_valid", "closure_valid", "notification_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramDisclosureAuditReport:
    disclosure_id: str
    finding_valid: bool
    stage_order_valid: bool
    approval_valid: bool
    advisory_valid: bool
    publication_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramDisclosureAuditReport":
        raw = _row_mapping("disclosure audit", value)
        return cls(_text("disclosure audit.disclosure_id", raw.get("disclosure_id")), *_row_bools("disclosure audit", raw, ("finding_valid", "stage_order_valid", "approval_valid", "advisory_valid", "publication_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramControlAuditReport:
    control: str
    enabled: bool
    required: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramControlAuditReport":
        raw = _row_mapping("control audit", value)
        return cls(_text("control audit.control", raw.get("control")), *_row_bools("control audit", raw, ("enabled", "required", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SecurityProgramAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    valid: bool | None
    security_program_ready_value: bool | None
    counts: Mapping[str, Any] | None
    scope_audits: tuple[SecurityProgramScopeAuditReport, ...]
    campaign_audits: tuple[SecurityProgramCampaignAuditReport, ...]
    finding_audits: tuple[SecurityProgramFindingAuditReport, ...]
    remediation_audits: tuple[SecurityProgramRemediationAuditReport, ...]
    incident_audits: tuple[SecurityProgramIncidentAuditReport, ...]
    disclosure_audits: tuple[SecurityProgramDisclosureAuditReport, ...]
    control_audits: tuple[SecurityProgramControlAuditReport, ...]
    issues: tuple[SecurityProgramIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityProgramAuditReport":
        raw = _payload(value)
        if raw.get("ok") is not True:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("security program refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), raw.get("manifest_digest"), False, False, None, (), (), (), (), (), (), (), (), _route_strings("security program refusal guarantees", raw.get("guarantees", [])), _route_strings("security program refusal limitations", raw.get("limitations", [])), raw.get("refusal") or raw.get("error"), True)
        if raw.get("schema") != SECURITY_PROGRAM_AUDIT_SCHEMA:
            raise ArgumentError("security program projection has an invalid schema")
        audit = _mapping("security program audit", raw.get("audit"))
        return cls(raw, True, SECURITY_PROGRAM_AUDIT_SCHEMA, _text("security program workflow", raw.get("workflow")), _text("security program manifest_digest", raw.get("manifest_digest"), required=False), _bool("security program valid", audit.get("valid")), _bool("security_program_ready", raw.get("security_program_ready")), _mapping("security program counts", audit.get("counts")), tuple(SecurityProgramScopeAuditReport.from_wire(item) for item in _bounded("security program scope audits", audit.get("scope_audits", []), SECURITY_PROGRAM_MAX_SCOPES)), tuple(SecurityProgramCampaignAuditReport.from_wire(item) for item in _bounded("security program campaign audits", audit.get("campaign_audits", []), SECURITY_PROGRAM_MAX_CAMPAIGNS)), tuple(SecurityProgramFindingAuditReport.from_wire(item) for item in _bounded("security program finding audits", audit.get("finding_audits", []), SECURITY_PROGRAM_MAX_FINDINGS)), tuple(SecurityProgramRemediationAuditReport.from_wire(item) for item in _bounded("security program remediation audits", audit.get("remediation_audits", []), SECURITY_PROGRAM_MAX_REMEDIATIONS)), tuple(SecurityProgramIncidentAuditReport.from_wire(item) for item in _bounded("security program incident audits", audit.get("incident_audits", []), SECURITY_PROGRAM_MAX_INCIDENTS)), tuple(SecurityProgramDisclosureAuditReport.from_wire(item) for item in _bounded("security program disclosure audits", audit.get("disclosure_audits", []), SECURITY_PROGRAM_MAX_DISCLOSURES)), tuple(SecurityProgramControlAuditReport.from_wire(item) for item in _bounded("security program control audits", audit.get("control_audits", []), SECURITY_PROGRAM_MAX_LIST_ITEMS)), tuple(SecurityProgramIssueReport.from_wire(item) for item in _bounded("security program issues", audit.get("issues", []), SECURITY_PROGRAM_MAX_LIST_ITEMS)), _route_strings("security program guarantees", raw.get("guarantees", audit.get("guarantees", []))), _route_strings("security program limitations", raw.get("limitations", audit.get("limitations", []))), None, False)

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.security_program_ready_value is True

    @property
    def security_program_ready(self) -> bool:
        return self.security_program_ready_value is True

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def blocking_issues(self) -> tuple[SecurityProgramIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[SecurityProgramIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def security_program_audit_report(value: Mapping[str, Any]) -> SecurityProgramAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return SecurityProgramAuditReport.from_wire(value)


__all__ = [
    "SECURITY_PROGRAM_MANIFEST_SCHEMA", "SECURITY_PROGRAM_AUDIT_SCHEMA", "SECURITY_PROGRAM_MAX_INPUT_BYTES", "SECURITY_PROGRAM_MAX_SCOPES", "SECURITY_PROGRAM_MAX_CAMPAIGNS", "SECURITY_PROGRAM_MAX_FINDINGS", "SECURITY_PROGRAM_MAX_REMEDIATIONS", "SECURITY_PROGRAM_MAX_INCIDENTS", "SECURITY_PROGRAM_MAX_DISCLOSURES",
    "SecurityProgramSystemArgs", "SecurityProgramScopeArgs", "SecurityProgramCampaignArgs", "SecurityProgramFindingArgs", "SecurityProgramRemediationArgs", "SecurityProgramTimelineEventArgs", "SecurityProgramIncidentArgs", "SecurityProgramDisclosureArgs", "SecurityProgramControlsArgs", "SecurityProgramPoliciesArgs", "SecurityProgramManifestArgs", "SecurityProgramIssueReport", "SecurityProgramScopeAuditReport", "SecurityProgramCampaignAuditReport", "SecurityProgramFindingAuditReport", "SecurityProgramRemediationAuditReport", "SecurityProgramIncidentAuditReport", "SecurityProgramDisclosureAuditReport", "SecurityProgramControlAuditReport", "SecurityProgramAuditReport", "security_program_audit_report",
]
