"""Typed projections for the section-13 red-team and incident-evidence workflow.

The Rust endpoint is a bounded contract replay, not a scanner or incident commander.  It keeps
regression protection, disclosure lifecycle, trust-boundary analysis, containment claims,
hash-linked audit evidence, and attestations separate because each has a different authority and
failure mode.  This module preserves that shape across direct MCP and HTTP envelopes instead of
returning one green/red security boolean.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


REDTEAM_MAX_ITEMS = 1_000
REDTEAM_MAX_FINDINGS = 256
REDTEAM_MAX_VULNERABILITIES = 256
REDTEAM_MAX_DELIVERIES = 256
REDTEAM_MAX_INCIDENTS = 256
REDTEAM_MAX_AUDIT_RECORDS = 512
REDTEAM_MAX_ATTESTATIONS = 256
REDTEAM_MAX_INPUT_BYTES = 20_000_000

VULNERABILITY_CLASSES = frozenset(
    {
        "code_vulnerability",
        "sandbox_bypass",
        "evaluator_bypass",
        "privacy_leakage",
        "benchmark_exploit",
        "hidden_test_exposure",
        "provenance_flaw",
        "malicious_artifact",
        "dependency_compromise",
        "misleading_security_claim",
    }
)
FINDING_STATUSES = frozenset({"reported", "reproduced", "confirmed", "not_reproduced", "duplicate"})
SAFETY_SEVERITIES = frozenset({"low", "medium", "high", "critical"})
DISCLOSURE_STAGES = frozenset({"reported", "triaged", "fixed", "disclosed", "withdrawn", "duplicate"})
BOUNDARY_SCOPES = frozenset({"within_trial", "across_trials"})
TRUST_ZONES = frozenset(
    {
        "user_client",
        "public_api",
        "control_plane",
        "catalog",
        "artifact_service",
        "build_service",
        "agent_sandbox",
        "evaluator_sandbox",
        "trusted_review",
        "private_worker",
        "model_provider",
        "public_registry_mirror",
    }
)
CHANNELS = frozenset(
    {
        "sealed_output_bundle",
        "typed_claim",
        "read_only_input",
        "hidden_oracle_mount",
        "artifact_fetch",
        "control_plane_api",
        "provider_api",
        "human_review",
        "publication",
    }
)
ARTIFACT_KINDS = frozenset(
    {
        "agent_output",
        "hidden_oracle_asset",
        "grader_claim",
        "pack_manifest",
        "credential",
        "trace",
        "published_result",
    }
)
INCIDENT_CLASSES = frozenset(
    {
        "confidentiality_leak",
        "unauthorized_effect",
        "sandbox_escape",
        "cross_tenant_exposure",
        "malicious_pack",
        "compromised_key",
        "result_integrity_failure",
        "benchmark_exploit",
        "hidden_holdout_leak",
        "evaluator_tampering",
        "artifact_substitution",
        "dependency_vulnerability",
        "privacy_breach",
        "service_compromise",
        "widespread_result_invalidity",
    }
)
CONTAINMENT_ACTIONS = frozenset(
    {
        "stop_execution_pool",
        "revoke_leases",
        "revoke_credentials",
        "quarantine_artifacts",
        "freeze_publication",
        "preserve_logs",
        "rotate_keys",
        "notify_federation_peers",
    }
)
AUDIT_EVENTS = frozenset(
    {
        "authentication",
        "privilege_change",
        "policy_change",
        "hidden_oracle_access",
        "sensitive_artifact_access",
        "publication",
        "result_acceptance",
        "reviewer_decision",
        "key_lifecycle",
        "security_quarantine",
        "deletion",
        "federation_import",
    }
)
ATTESTATION_CLAIMS = frozenset(
    {
        "digests_compared",
        "built_from_manifest",
        "bundle_closure_verified",
        "independently_reproduced",
        "source_reviewed",
        "tests_passed",
    }
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _sequence(name: str, value: Any, *, maximum: int | None = None) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    values = tuple(value)
    if maximum is not None and len(values) > maximum:
        raise ArgumentError(f"{name} cannot contain more than {maximum} items")
    return values


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _enum(name: str, value: Any, choices: frozenset[str]) -> str:
    text = _route_text(name, value)
    if text not in choices:
        raise ArgumentError(f"unknown {name} {text!r}")
    return text


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, an MCP structured result, or a REST tool envelope."""

    raw = _route_mapping("security red-team response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("workflow") == "section_13_redteam_incident_evidence"
        return candidate.get("ok") is False and isinstance(candidate.get("refusal"), str)

    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, list):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"security red-team response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)
        content = container.get("content")
        if isinstance(content, list):
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"security red-team response text is not JSON: {error}") from error
                if isinstance(decoded, Mapping):
                    candidates.append(decoded)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a section-13 red-team projection")


@dataclass(frozen=True)
class SecurityRedteamSimulateArgs:
    findings: tuple[Mapping[str, Any], ...] = ()
    vulnerabilities: tuple[Mapping[str, Any], ...] = ()
    deliveries: tuple[Mapping[str, Any], ...] = ()
    incidents: tuple[Mapping[str, Any], ...] = ()
    audit_records: tuple[Mapping[str, Any], ...] = ()
    attestations: tuple[Mapping[str, Any], ...] = ()
    boundary_universe: tuple[str, ...] = ()
    include_details: bool = False
    max_items: int = 100

    def __post_init__(self) -> None:
        arrays = (
            ("findings", self.findings, REDTEAM_MAX_FINDINGS),
            ("vulnerabilities", self.vulnerabilities, REDTEAM_MAX_VULNERABILITIES),
            ("deliveries", self.deliveries, REDTEAM_MAX_DELIVERIES),
            ("incidents", self.incidents, REDTEAM_MAX_INCIDENTS),
            ("audit_records", self.audit_records, REDTEAM_MAX_AUDIT_RECORDS),
            ("attestations", self.attestations, REDTEAM_MAX_ATTESTATIONS),
        )
        normalized: dict[str, tuple[Mapping[str, Any], ...]] = {}
        for name, value, maximum in arrays:
            rows = _sequence(f"red-team {name}", value, maximum=maximum)
            if any(not isinstance(item, Mapping) for item in rows):
                raise ArgumentError(f"red-team {name} must contain objects")
            normalized[name] = tuple(dict(item) for item in rows)
        universe = _route_strings("red-team boundary_universe", self.boundary_universe)
        include_details = _bool("red-team include_details", self.include_details)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= REDTEAM_MAX_ITEMS:
            raise ArgumentError(f"red-team max_items must be between 1 and {REDTEAM_MAX_ITEMS}")
        encoded_args: dict[str, Any] = {
            **normalized,
            "boundary_universe": list(universe),
            "include_details": include_details,
            "max_items": self.max_items,
        }
        try:
            encoded = json.dumps(encoded_args, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"red-team arguments are not JSON serializable: {error}") from error
        if len(encoded) > REDTEAM_MAX_INPUT_BYTES:
            raise ArgumentError("red-team input exceeds the 20000000-byte safety bound")
        for name, value in normalized.items():
            object.__setattr__(self, name, value)
        object.__setattr__(self, "boundary_universe", universe)
        object.__setattr__(self, "include_details", include_details)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityRedteamSimulateArgs":
        raw = _route_mapping("security red-team arguments", value)
        def rows(name: str, maximum: int) -> tuple[Mapping[str, Any], ...]:
            return tuple(_route_mapping(f"red-team {name}[{index}]", item) for index, item in enumerate(_sequence(f"red-team {name}", raw.get(name, []), maximum=maximum)))
        return cls(
            rows("findings", REDTEAM_MAX_FINDINGS),
            rows("vulnerabilities", REDTEAM_MAX_VULNERABILITIES),
            rows("deliveries", REDTEAM_MAX_DELIVERIES),
            rows("incidents", REDTEAM_MAX_INCIDENTS),
            rows("audit_records", REDTEAM_MAX_AUDIT_RECORDS),
            rows("attestations", REDTEAM_MAX_ATTESTATIONS),
            tuple(_route_strings("red-team boundary_universe", raw.get("boundary_universe", []))),
            raw.get("include_details", False),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "findings": [dict(row) for row in self.findings],
            "vulnerabilities": [dict(row) for row in self.vulnerabilities],
            "deliveries": [dict(row) for row in self.deliveries],
            "incidents": [dict(row) for row in self.incidents],
            "audit_records": [dict(row) for row in self.audit_records],
            "attestations": [dict(row) for row in self.attestations],
            "boundary_universe": list(self.boundary_universe),
            "include_details": self.include_details,
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class RegressionGateReport:
    raw: dict[str, Any]
    eligible: bool
    cell: dict[str, Any] | None
    public_summary: str | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegressionGateReport":
        raw = _route_mapping("red-team regression gate", value)
        eligible = _bool("red-team regression eligibility", raw.get("eligible"))
        cell_raw = raw.get("cell")
        summary_raw = raw.get("public_summary")
        cell = None if cell_raw is None else _route_mapping("red-team regression cell", cell_raw)
        summary = None if summary_raw is None else _route_text("red-team regression public_summary", summary_raw)
        if eligible and cell is None:
            raise ArgumentError("eligible regression gate requires a cell")
        refusal = _optional_text("red-team regression refusal", raw.get("refusal"))
        fail_closed = None if raw.get("fail_closed") is None else _bool("red-team regression fail_closed", raw.get("fail_closed"))
        if not eligible and (refusal is None or fail_closed is not True):
            raise ArgumentError("ineligible regression gate must retain a fail-closed refusal")
        return cls(raw, eligible, cell, summary, refusal, fail_closed)


@dataclass(frozen=True)
class RedteamFindingReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    finding: dict[str, Any] | None
    finding_id: str | None
    campaign: str | None
    boundary: str | None
    class_name: str | None
    status: str | None
    reproduction: str | None
    regression_gate: RegressionGateReport | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RedteamFindingReport":
        raw = _route_mapping("red-team finding row", value)
        index = _route_count("red-team finding index", raw.get("index"))
        ok = _bool("red-team finding ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, None, None, None, None, None, None, _route_text("red-team finding refusal", raw.get("refusal")), _bool("red-team finding fail_closed", raw.get("fail_closed")))
        finding = _route_mapping("red-team finding", raw.get("finding"))
        finding_id = _route_text("red-team finding.id", finding.get("id"))
        campaign = _route_text("red-team finding.campaign", finding.get("campaign"))
        boundary = _route_text("red-team finding.boundary", finding.get("boundary"))
        class_name = _enum("red-team finding.class", finding.get("class"), VULNERABILITY_CLASSES)
        status = _enum("red-team finding.status", finding.get("status"), FINDING_STATUSES)
        return cls(raw, index, True, finding, finding_id, campaign, boundary, class_name, status, _optional_text("red-team finding.reproduction", finding.get("reproduction")), RegressionGateReport.from_wire(raw.get("regression_gate")), None, None)


@dataclass(frozen=True)
class RegressionCorpusReport:
    raw: dict[str, Any]
    sentinel_count: int
    covered_boundaries: tuple[str, ...]
    unminimised_count: int
    uncovered_boundaries: tuple[str, ...]
    cells: tuple[dict[str, Any], ...]
    omitted_cells: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegressionCorpusReport":
        raw = _route_mapping("red-team regression corpus", value)
        cells = tuple(_route_mapping(f"red-team regression cell[{index}]", item) for index, item in enumerate(_sequence("red-team regression cells", raw.get("cells", []))))
        return cls(raw, _route_count("red-team sentinel_count", raw.get("sentinel_count")), _route_strings("red-team covered_boundaries", raw.get("covered_boundaries", [])), _route_count("red-team unminimised_count", raw.get("unminimised_count")), _route_strings("red-team uncovered_boundaries", raw.get("uncovered_boundaries", [])), cells, _route_count("red-team omitted_cells", raw.get("omitted_cells")))

    @property
    def has_confirmed_finding(self) -> bool:
        return self.sentinel_count > 0


@dataclass(frozen=True)
class VulnerabilityTransitionReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    to: str | None
    epoch: int | None
    stage_after: str | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "VulnerabilityTransitionReport":
        raw = _route_mapping("red-team vulnerability transition", value)
        index = _route_count("red-team transition index", raw.get("index"))
        ok = _bool("red-team transition ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, None, _optional_text("red-team transition stage_after", raw.get("stage_after")), _route_text("red-team transition refusal", raw.get("refusal")), _bool("red-team transition fail_closed", raw.get("fail_closed")))
        to = _enum("red-team transition.to", raw.get("to"), DISCLOSURE_STAGES)
        stage_after = _enum("red-team transition.stage_after", raw.get("stage_after"), DISCLOSURE_STAGES)
        return cls(raw, index, True, to, _route_count("red-team transition epoch", raw.get("epoch")), stage_after, None, None)


@dataclass(frozen=True)
class VulnerabilityReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    vulnerability: dict[str, Any] | None
    vulnerability_id: str | None
    class_name: str | None
    severity: str | None
    stage: str | None
    entered_at: int | None
    embargoed: bool | None
    history: tuple[dict[str, Any], ...]
    transitions: tuple[VulnerabilityTransitionReport, ...]
    transition_count: int | None
    stopped_after_refusal: bool | None
    advisory_present: bool | None
    advisory_missing_fields: tuple[str, ...]
    independent_verification_required: bool | None
    disclosed: bool | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "VulnerabilityReport":
        raw = _route_mapping("red-team vulnerability row", value)
        index = _route_count("red-team vulnerability index", raw.get("index"))
        ok = _bool("red-team vulnerability ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, None, None, None, None, None, None, (), (), None, None, None, (), None, None, _route_text("red-team vulnerability refusal", raw.get("refusal")), _bool("red-team vulnerability fail_closed", raw.get("fail_closed")))
        vulnerability = _route_mapping("red-team vulnerability", raw.get("vulnerability"))
        transitions = tuple(VulnerabilityTransitionReport.from_wire(item) for item in _sequence("red-team vulnerability transitions", raw.get("transitions", [])))
        history = tuple(_route_mapping(f"red-team vulnerability history[{index}]", item) for index, item in enumerate(_sequence("red-team vulnerability history", vulnerability.get("history", []))))
        return cls(raw, index, True, vulnerability, _route_text("red-team vulnerability.id", vulnerability.get("id")), _enum("red-team vulnerability.class", vulnerability.get("class"), VULNERABILITY_CLASSES), _enum("red-team vulnerability.severity", vulnerability.get("severity"), SAFETY_SEVERITIES), _enum("red-team vulnerability.stage", vulnerability.get("stage"), DISCLOSURE_STAGES), _route_count("red-team vulnerability.entered_at", vulnerability.get("entered_at")), _bool("red-team vulnerability.embargoed", vulnerability.get("embargoed")), history, transitions, _route_count("red-team transition_count", raw.get("transition_count")), _bool("red-team stopped_after_refusal", raw.get("stopped_after_refusal")), _bool("red-team advisory_present", raw.get("advisory_present")), _route_strings("red-team advisory_missing_fields", raw.get("advisory_missing_fields", [])), _bool("red-team independent_verification_required", raw.get("independent_verification_required")), _bool("red-team disclosed", raw.get("disclosed")), None, None)


@dataclass(frozen=True)
class DeliveryReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    crossing: dict[str, Any] | None
    honest_label: str | None
    scope: str | None
    requested: dict[str, Any] | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReport":
        raw = _route_mapping("red-team delivery row", value)
        index = _route_count("red-team delivery index", raw.get("index"))
        ok = _bool("red-team delivery ok", raw.get("ok"))
        if not ok:
            requested_raw = raw.get("requested")
            return cls(raw, index, False, None, None, None, None if requested_raw is None else _route_mapping("red-team delivery requested", requested_raw), _route_text("red-team delivery refusal", raw.get("refusal")), _bool("red-team delivery fail_closed", raw.get("fail_closed")))
        scope = _optional_text("red-team delivery scope", raw.get("scope"))
        if scope is not None and scope not in BOUNDARY_SCOPES:
            raise ArgumentError(f"unknown red-team delivery scope {scope!r}")
        return cls(raw, index, True, _route_mapping("red-team delivery crossing", raw.get("crossing")), _route_text("red-team delivery honest_label", raw.get("honest_label")), scope, None, None, None)


@dataclass(frozen=True)
class BoundaryReport:
    raw: dict[str, Any]
    model: str
    within_trial_agent_to_evaluator: tuple[Any, ...]
    within_trial_evaluator_to_agent: tuple[Any, ...]
    all_scope_agent_to_evaluator: tuple[Any, ...]
    feedback_loops: tuple[Any, ...]
    delivery_rows: tuple[DeliveryReport, ...]
    delivery_rows_omitted: int
    allowed_delivery_count: int
    refused_delivery_count: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BoundaryReport":
        raw = _route_mapping("red-team boundary", value)
        return cls(raw, _route_text("red-team boundary model", raw.get("model")), tuple(_sequence("red-team agent-to-evaluator paths", raw.get("within_trial_agent_to_evaluator", []))), tuple(_sequence("red-team evaluator-to-agent paths", raw.get("within_trial_evaluator_to_agent", []))), tuple(_sequence("red-team all-scope paths", raw.get("all_scope_agent_to_evaluator", []))), tuple(_sequence("red-team feedback loops", raw.get("feedback_loops", []))), tuple(DeliveryReport.from_wire(item) for item in _sequence("red-team delivery rows", raw.get("delivery_rows", []))), _route_count("red-team delivery_rows_omitted", raw.get("delivery_rows_omitted")), _route_count("red-team allowed_delivery_count", raw.get("allowed_delivery_count")), _route_count("red-team refused_delivery_count", raw.get("refused_delivery_count")))

    @property
    def within_trial_feedback_is_absent(self) -> bool:
        return len(self.within_trial_evaluator_to_agent) == 0


@dataclass(frozen=True)
class ContainmentRequestReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    request: dict[str, Any] | None
    honest_label: str | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContainmentRequestReport":
        raw = _route_mapping("red-team containment request", value)
        index = _route_count("red-team containment request index", raw.get("index"))
        ok = _bool("red-team containment request ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, None, _route_text("red-team containment request refusal", raw.get("refusal")), _bool("red-team containment request fail_closed", raw.get("fail_closed")))
        request = _route_mapping("red-team containment request.request", raw.get("request"))
        action = _enum("red-team containment request.action", request.get("action"), CONTAINMENT_ACTIONS)
        request["action"] = action
        return cls(raw, index, True, request, _route_text("red-team containment request honest_label", raw.get("honest_label")), None, None)


@dataclass(frozen=True)
class TimelineEntryReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    epoch: int | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TimelineEntryReport":
        raw = _route_mapping("red-team timeline row", value)
        index = _route_count("red-team timeline index", raw.get("index"))
        ok = _bool("red-team timeline ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, _route_text("red-team timeline refusal", raw.get("refusal")), _bool("red-team timeline fail_closed", raw.get("fail_closed")))
        return cls(raw, index, True, _route_count("red-team timeline epoch", raw.get("epoch")), None, None)


@dataclass(frozen=True)
class ContainmentClaimReport:
    raw: dict[str, Any]
    allowed: bool
    report: dict[str, Any] | None
    caveat: str | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContainmentClaimReport":
        raw = _route_mapping("red-team containment claim", value)
        allowed = _bool("red-team containment allowed", raw.get("allowed"))
        report_raw = raw.get("report")
        report = None if report_raw is None else _route_mapping("red-team containment report", report_raw)
        caveat = _optional_text("red-team containment caveat", raw.get("caveat"))
        if allowed:
            if report is None or caveat is None:
                raise ArgumentError("allowed containment must retain report and non-execution caveat")
            return cls(raw, True, report, caveat, None, None)
        return cls(raw, False, None, None, _route_text("red-team containment refusal", raw.get("refusal")), _bool("red-team containment fail_closed", raw.get("fail_closed")))


@dataclass(frozen=True)
class IncidentReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    incident: dict[str, Any] | None
    incident_id: str | None
    class_name: str | None
    opened_at: int | None
    requests: tuple[ContainmentRequestReport, ...]
    timeline: tuple[TimelineEntryReport, ...]
    containment_claim: ContainmentClaimReport | None
    unrequested_actions: tuple[str, ...]
    result_tainting_class: bool | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "IncidentReport":
        raw = _route_mapping("red-team incident row", value)
        index = _route_count("red-team incident index", raw.get("index"))
        ok = _bool("red-team incident ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, None, None, None, (), (), None, (), None, _route_text("red-team incident refusal", raw.get("refusal")), _bool("red-team incident fail_closed", raw.get("fail_closed")))
        incident = _route_mapping("red-team incident", raw.get("incident"))
        requests = tuple(ContainmentRequestReport.from_wire(item) for item in _sequence("red-team incident requests", raw.get("requests", [])))
        timeline = tuple(TimelineEntryReport.from_wire(item) for item in _sequence("red-team incident timeline", raw.get("timeline", [])))
        unrequested = _route_strings("red-team unrequested_actions", raw.get("unrequested_actions", []))
        return cls(raw, index, True, incident, _route_text("red-team incident.id", incident.get("id")), _enum("red-team incident.class", incident.get("class"), INCIDENT_CLASSES), _route_count("red-team incident.opened_at", incident.get("opened_at")), requests, timeline, ContainmentClaimReport.from_wire(raw.get("containment_claim")), unrequested, _bool("red-team result_tainting_class", raw.get("result_tainting_class")), None, None)


@dataclass(frozen=True)
class AuditRowReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    linked: dict[str, Any] | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AuditRowReport":
        raw = _route_mapping("red-team audit row", value)
        index = _route_count("red-team audit index", raw.get("index"))
        ok = _bool("red-team audit ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, _route_text("red-team audit refusal", raw.get("refusal")), _bool("red-team audit fail_closed", raw.get("fail_closed")))
        return cls(raw, index, True, _route_mapping("red-team audit linked", raw.get("linked")), None, None)


@dataclass(frozen=True)
class AuditReport:
    raw: dict[str, Any]
    rows: tuple[AuditRowReport, ...]
    rows_omitted: int
    chain_length: int
    head: str | None
    verified: bool
    verification_refusal: str | None
    assertion_count: int
    public_view_count: int
    records: tuple[dict[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AuditReport":
        raw = _route_mapping("red-team audit", value)
        verified = _bool("red-team audit verified", raw.get("verified"))
        verification_refusal = _optional_text("red-team audit verification_refusal", raw.get("verification_refusal"))
        if verified and verification_refusal is not None:
            raise ArgumentError("verified audit cannot retain a verification refusal")
        return cls(raw, tuple(AuditRowReport.from_wire(item) for item in _sequence("red-team audit rows", raw.get("rows", []))), _route_count("red-team audit rows_omitted", raw.get("rows_omitted")), _route_count("red-team audit chain_length", raw.get("chain_length")), _optional_text("red-team audit head", raw.get("head")), verified, verification_refusal, _route_count("red-team audit assertion_count", raw.get("assertion_count")), _route_count("red-team audit public_view_count", raw.get("public_view_count")), tuple(_route_mapping(f"red-team audit record[{index}]", item) for index, item in enumerate(_sequence("red-team audit records", raw.get("records", [])))))


@dataclass(frozen=True)
class AttestationReport:
    raw: dict[str, Any]
    index: int
    ok: bool
    observed: bool | None
    attestation: dict[str, Any] | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AttestationReport":
        raw = _route_mapping("red-team attestation row", value)
        index = _route_count("red-team attestation index", raw.get("index"))
        ok = _bool("red-team attestation ok", raw.get("ok"))
        if not ok:
            return cls(raw, index, False, None, None, _route_text("red-team attestation refusal", raw.get("refusal")), _bool("red-team attestation fail_closed", raw.get("fail_closed")))
        return cls(raw, index, True, _bool("red-team attestation observed", raw.get("observed")), _route_mapping("red-team attestation", raw.get("attestation")), None, None)


@dataclass(frozen=True)
class SecurityRedteamReport:
    raw: dict[str, Any]
    ok: bool
    workflow: str | None
    input_counts: dict[str, int]
    findings: tuple[RedteamFindingReport, ...]
    findings_omitted: int
    regression_corpus: RegressionCorpusReport | None
    vulnerabilities: tuple[VulnerabilityReport, ...]
    vulnerabilities_omitted: int
    boundary: BoundaryReport | None
    incidents: tuple[IncidentReport, ...]
    incidents_omitted: int
    audit: AuditReport | None
    attestations: tuple[AttestationReport, ...]
    attestations_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SecurityRedteamReport":
        raw = _payload(value)
        ok = _bool("security red-team ok", raw.get("ok"))
        if not ok:
            return cls(raw, False, None, {}, (), 0, None, (), 0, None, (), 0, None, (), 0, (), (), _route_text("security red-team refusal", raw.get("refusal")), _bool("security red-team fail_closed", raw.get("fail_closed")))
        counts = _route_mapping("security red-team input_counts", raw.get("input_counts"))
        input_counts = {key: _route_count(f"security red-team input_counts.{key}", value) for key, value in counts.items()}
        return cls(raw, True, _route_text("security red-team workflow", raw.get("workflow")), input_counts, tuple(RedteamFindingReport.from_wire(item) for item in _sequence("security red-team findings", raw.get("findings", []))), _route_count("security red-team findings_omitted", raw.get("findings_omitted")), RegressionCorpusReport.from_wire(raw.get("regression_corpus")), tuple(VulnerabilityReport.from_wire(item) for item in _sequence("security red-team vulnerabilities", raw.get("vulnerabilities", []))), _route_count("security red-team vulnerabilities_omitted", raw.get("vulnerabilities_omitted")), BoundaryReport.from_wire(raw.get("boundary")), tuple(IncidentReport.from_wire(item) for item in _sequence("security red-team incidents", raw.get("incidents", []))), _route_count("security red-team incidents_omitted", raw.get("incidents_omitted")), AuditReport.from_wire(raw.get("audit")), tuple(AttestationReport.from_wire(item) for item in _sequence("security red-team attestations", raw.get("attestations", []))), _route_count("security red-team attestations_omitted", raw.get("attestations_omitted")), _route_strings("security red-team guarantees", raw.get("guarantees", [])), _route_strings("security red-team limitations", raw.get("limitations", [])), None, None)

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def confirmed_finding_count(self) -> int:
        return sum(1 for row in self.findings if row.ok and row.status == "confirmed")

    @property
    def ineligible_regression_count(self) -> int:
        return sum(1 for row in self.findings if row.ok and row.regression_gate is not None and not row.regression_gate.eligible)

    @property
    def disclosed_vulnerability_count(self) -> int:
        return sum(1 for row in self.vulnerabilities if row.ok and row.disclosed is True)

    @property
    def failed_vulnerability_count(self) -> int:
        return sum(1 for row in self.vulnerabilities if not row.ok or any(not transition.ok for transition in row.transitions))

    @property
    def allowed_delivery_count(self) -> int:
        return 0 if self.boundary is None else self.boundary.allowed_delivery_count

    @property
    def refused_delivery_count(self) -> int:
        return 0 if self.boundary is None else self.boundary.refused_delivery_count

    @property
    def containment_allowed_count(self) -> int:
        return sum(1 for row in self.incidents if row.ok and row.containment_claim is not None and row.containment_claim.allowed)

    @property
    def containment_withheld_count(self) -> int:
        return sum(1 for row in self.incidents if row.ok and row.containment_claim is not None and not row.containment_claim.allowed)

    @property
    def audit_chain_verified(self) -> bool:
        return self.audit is not None and self.audit.verified

    @property
    def observed_attestation_count(self) -> int:
        return sum(1 for row in self.attestations if row.ok and row.observed is True)

    @property
    def asserted_attestation_count(self) -> int:
        return sum(1 for row in self.attestations if row.ok and row.observed is False)

    @property
    def fail_closed_row_count(self) -> int:
        rows: list[Any] = list(self.findings) + list(self.vulnerabilities) + list(self.incidents) + list(self.attestations)
        if self.boundary is not None:
            rows.extend(self.boundary.delivery_rows)
        if self.audit is not None:
            rows.extend(self.audit.rows)
        return sum(1 for row in rows if getattr(row, "fail_closed", None) is True)

    @property
    def execution_claims_absent(self) -> bool:
        return any("does not run fuzzers" in limitation for limitation in self.limitations)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def security_redteam_simulate_report(value: Mapping[str, Any]) -> SecurityRedteamReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return SecurityRedteamReport.from_wire(value)


__all__ = [
    "REDTEAM_MAX_ITEMS",
    "REDTEAM_MAX_FINDINGS",
    "REDTEAM_MAX_VULNERABILITIES",
    "REDTEAM_MAX_DELIVERIES",
    "REDTEAM_MAX_INCIDENTS",
    "REDTEAM_MAX_AUDIT_RECORDS",
    "REDTEAM_MAX_ATTESTATIONS",
    "REDTEAM_MAX_INPUT_BYTES",
    "VULNERABILITY_CLASSES",
    "FINDING_STATUSES",
    "SAFETY_SEVERITIES",
    "DISCLOSURE_STAGES",
    "BOUNDARY_SCOPES",
    "TRUST_ZONES",
    "CHANNELS",
    "ARTIFACT_KINDS",
    "INCIDENT_CLASSES",
    "CONTAINMENT_ACTIONS",
    "AUDIT_EVENTS",
    "ATTESTATION_CLAIMS",
    "SecurityRedteamSimulateArgs",
    "RegressionGateReport",
    "RedteamFindingReport",
    "RegressionCorpusReport",
    "VulnerabilityTransitionReport",
    "VulnerabilityReport",
    "DeliveryReport",
    "BoundaryReport",
    "ContainmentRequestReport",
    "TimelineEntryReport",
    "ContainmentClaimReport",
    "IncidentReport",
    "AuditRowReport",
    "AuditReport",
    "AttestationReport",
    "SecurityRedteamReport",
    "security_redteam_simulate_report",
]
