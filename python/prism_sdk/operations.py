"""Typed operations and infrastructure projections.

These reports are intentionally descriptive rather than optimistic.  The operations catalogue
records what the in-tree contract says about storage, service boundaries, SLO names, and metric
definitions; the acceptance report keeps a library-observable refutation separate from the much
larger set of criteria that require an engineer, shell, checkout, CI run, or external service.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


OPERATIONS_MAX_ITEMS = 1_000
OPERATIONS_DEFAULT_MAX_ITEMS = 100
OPERATIONS_DURABILITIES = frozenset({"Canonical", "Rebuildable"})
OPERATIONS_MUTABILITIES = frozenset({"immutable", "append_only", "mutable"})
OPERATIONS_DATA_CLASSES = frozenset({"metadata", "artifact", "event", "analytics", "search"})
OPERATIONS_DEPLOYMENT_PLANES = frozenset({
    "control_api", "catalog", "artifact_storage", "scheduler", "execution_pool",
    "analytics", "search", "signing", "observability",
})
OPERATIONS_TENANT_PATTERNS = frozenset({
    "shared_control", "dedicated_installation", "air_gapped_registry", "hybrid_public_metadata",
})
OPS_ACCEPTANCE_VERDICTS = frozenset({"met", "refuted", "unverifiable"})
OPS_ACCEPTANCE_BASES = frozenset({"linked_type", "workspace_manifest", "author", "no_observer"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _route_text(name, value)


def _bounded_max_items(name: str, value: Any) -> int:
    result = _route_count(name, value)
    if not 1 <= result <= OPERATIONS_MAX_ITEMS:
        raise ArgumentError(f"{name} must be between 1 and {OPERATIONS_MAX_ITEMS}")
    return result


def _payload(value: Mapping[str, Any], keys: tuple[str, ...], label: str) -> dict[str, Any]:
    raw = _route_mapping(f"{label} response", value)
    if all(key in raw for key in keys):
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping) and all(key in structured for key in keys):
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"{label} response text is not JSON: {error}") from error
                    decoded_mapping = _route_mapping(f"decoded {label} response", decoded)
                    if all(key in decoded_mapping for key in keys):
                        return decoded_mapping
    raise ArgumentError(f"response does not contain an {label} projection")


@dataclass(frozen=True)
class OperationsCatalogArgs:
    """Bounded operations-catalogue request."""

    include_details: bool = False
    max_items: int = OPERATIONS_DEFAULT_MAX_ITEMS

    def __post_init__(self) -> None:
        if not isinstance(self.include_details, bool):
            raise ArgumentError("include_details must be a boolean")
        _bounded_max_items("max_items", self.max_items)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"include_details": self.include_details, "max_items": self.max_items}


@dataclass(frozen=True)
class OperationsStoreReport:
    raw: dict[str, Any]
    name: str
    technology: str
    durability: str
    mutability: str
    rebuilt_from: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsStoreReport":
        raw = _route_mapping("operations store", value)
        durability = _route_text("operations store durability", raw.get("durability"))
        if durability not in OPERATIONS_DURABILITIES:
            raise ArgumentError(f"unknown operations durability: {durability!r}")
        mutability = _route_text("operations store mutability", raw.get("mutability"))
        if mutability not in OPERATIONS_MUTABILITIES:
            raise ArgumentError(f"unknown operations mutability: {mutability!r}")
        rebuilt_from = _route_strings("operations store rebuilt_from", raw.get("rebuilt_from", []))
        return cls(raw, _route_text("operations store name", raw.get("name")), _route_text("operations store technology", raw.get("technology")), durability, mutability, rebuilt_from)


@dataclass(frozen=True)
class OperationsTopologyClassReport:
    raw: dict[str, Any]
    class_id: str
    name: str
    store: OperationsStoreReport
    promises: dict[str, Any]
    holds_immutable_evidence: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsTopologyClassReport":
        raw = _route_mapping("operations topology class", value)
        class_id = _route_text("operations topology class id", raw.get("class"))
        if class_id not in OPERATIONS_DATA_CLASSES:
            raise ArgumentError(f"unknown operations data class: {class_id!r}")
        name = _route_text("operations topology class name", raw.get("name"))
        if name != class_id:
            raise ArgumentError("operations topology class name does not reconcile with class")
        promises = _route_mapping("operations topology promises", raw.get("promises"))
        durability = _route_text("operations promise durability", promises.get("durability"))
        mutability = _route_text("operations promise mutability", promises.get("mutability"))
        if durability not in OPERATIONS_DURABILITIES or mutability not in OPERATIONS_MUTABILITIES:
            raise ArgumentError("operations topology promises contain an unknown durability or mutability")
        return cls(raw, class_id, name, OperationsStoreReport.from_wire(raw.get("store")), promises, _bool("operations topology holds_immutable_evidence", raw.get("holds_immutable_evidence")))


@dataclass(frozen=True)
class OperationsTopologyReport:
    raw: dict[str, Any]
    deployment: str
    technologies: tuple[str, ...]
    classes: tuple[OperationsTopologyClassReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsTopologyReport":
        raw = _route_mapping("operations topology", value)
        classes_raw = raw.get("classes")
        if not isinstance(classes_raw, Sequence) or isinstance(classes_raw, (str, bytes)):
            raise ArgumentError("operations topology classes must be an array")
        classes = tuple(OperationsTopologyClassReport.from_wire(item) for item in classes_raw)
        if len(classes) != len(OPERATIONS_DATA_CLASSES) or {item.class_id for item in classes} != OPERATIONS_DATA_CLASSES:
            raise ArgumentError("operations topology must contain exactly the five data classes")
        return cls(raw, _route_text("operations topology deployment", raw.get("deployment")), _route_strings("operations topology technologies", raw.get("technologies")), classes)


@dataclass(frozen=True)
class OperationsPromiseParityReport:
    raw: dict[str, Any]
    compared: int
    holds: bool
    differences: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsPromiseParityReport":
        raw = _route_mapping("operations promise parity", value)
        compared = _route_count("operations promise parity compared", raw.get("compared"))
        holds = _bool("operations promise parity holds", raw.get("holds"))
        differences = _route_strings("operations promise parity differences", raw.get("differences", []))
        if holds != (not differences):
            raise ArgumentError("operations promise parity holds does not reconcile with differences")
        return cls(raw, compared, holds, differences)


@dataclass(frozen=True)
class OperationsDataClassReport:
    raw: dict[str, Any]
    class_id: str
    name: str
    holds_immutable_evidence: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDataClassReport":
        raw = _route_mapping("operations data class", value)
        class_id = _route_text("operations data class id", raw.get("class"))
        if class_id not in OPERATIONS_DATA_CLASSES:
            raise ArgumentError(f"unknown operations data class: {class_id!r}")
        return cls(raw, class_id, _route_text("operations data class name", raw.get("name")), _bool("operations data class holds_immutable_evidence", raw.get("holds_immutable_evidence")))


@dataclass(frozen=True)
class OperationsDeploymentPlaneReport:
    raw: dict[str, Any]
    plane: str
    name: str
    control_plane: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsDeploymentPlaneReport":
        raw = _route_mapping("operations deployment plane", value)
        plane = _route_text("operations deployment plane id", raw.get("plane"))
        if plane not in OPERATIONS_DEPLOYMENT_PLANES:
            raise ArgumentError(f"unknown operations deployment plane: {plane!r}")
        return cls(raw, plane, _route_text("operations deployment plane name", raw.get("name")), _bool("operations deployment plane control_plane", raw.get("control_plane")))


@dataclass(frozen=True)
class OperationsTenantPatternReport:
    raw: dict[str, Any]
    pattern: str
    name: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsTenantPatternReport":
        raw = _route_mapping("operations tenant pattern", value)
        pattern = _route_text("operations tenant pattern id", raw.get("pattern"))
        if pattern not in OPERATIONS_TENANT_PATTERNS:
            raise ArgumentError(f"unknown operations tenant pattern: {pattern!r}")
        return cls(raw, pattern, _route_text("operations tenant pattern name", raw.get("name")))


@dataclass(frozen=True)
class OperationsServiceSummaryReport:
    raw: dict[str, Any]
    satisfied: int
    diverges: int
    not_implemented: int
    divergence_count: int
    total: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsServiceSummaryReport":
        raw = _route_mapping("operations service summary", value)
        satisfied = _route_count("operations service summary satisfied", raw.get("satisfied"))
        diverges = _route_count("operations service summary diverges", raw.get("diverges"))
        not_implemented = _route_count("operations service summary not_implemented", raw.get("not_implemented"))
        total = _route_count("operations service summary total", raw.get("total"))
        divergence_count = _route_count("operations service summary divergences", raw.get("divergences"))
        if total != satisfied + diverges + not_implemented:
            raise ArgumentError("operations service summary total does not reconcile")
        return cls(raw, satisfied, diverges, not_implemented, divergence_count, total)


@dataclass(frozen=True)
class OperationsServiceContractReport:
    raw: dict[str, Any]
    module_id: str
    title: str
    contract: str
    crates: tuple[str, ...]
    verdict: str
    divergence_count: int
    divergences: tuple[str, ...]
    omitted_divergences: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsServiceContractReport":
        raw = _route_mapping("operations service contract", value)
        verdict = _route_text("operations service verdict", raw.get("verdict"))
        if verdict not in {"satisfied", "diverges", "not_implemented"}:
            raise ArgumentError(f"unknown operations service verdict: {verdict!r}")
        divergence_count = _route_count("operations service divergence_count", raw.get("divergence_count"))
        divergences = _route_strings("operations service divergences", raw.get("divergences", []))
        omitted = _route_count("operations service omitted_divergences", raw.get("omitted_divergences"))
        if divergence_count != len(divergences) + omitted:
            raise ArgumentError("operations service divergence count does not reconcile")
        if verdict == "satisfied" and divergence_count:
            raise ArgumentError("satisfied operations service cannot contain divergences")
        return cls(raw, _route_text("operations service module_id", raw.get("module_id")), _route_text("operations service title", raw.get("title")), _route_text("operations service contract", raw.get("contract")), _route_strings("operations service crates", raw.get("crates", [])), verdict, divergence_count, divergences, omitted)


@dataclass(frozen=True)
class OperationsServiceContractsReport:
    raw: dict[str, Any]
    summary: OperationsServiceSummaryReport
    entries: tuple[OperationsServiceContractReport, ...]
    entry_count: int
    omitted_entries: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsServiceContractsReport":
        raw = _route_mapping("operations service contracts", value)
        raw_entries = raw.get("entries")
        if not isinstance(raw_entries, Sequence) or isinstance(raw_entries, (str, bytes)):
            raise ArgumentError("operations service entries must be an array")
        entries = tuple(OperationsServiceContractReport.from_wire(item) for item in raw_entries)
        entry_count = _route_count("operations service entry_count", raw.get("entry_count"))
        omitted = _route_count("operations service omitted_entries", raw.get("omitted_entries"))
        if entry_count != len(entries) + omitted:
            raise ArgumentError("operations service entry count does not reconcile")
        summary = OperationsServiceSummaryReport.from_wire(raw.get("summary"))
        if summary.total != entry_count:
            raise ArgumentError("operations service summary total does not match entry_count")
        return cls(raw, summary, entries, entry_count, omitted)


@dataclass(frozen=True)
class OperationsMetricDefinitionReport:
    raw: dict[str, Any]
    metric: str
    blueprint_name: bool
    numerator: str
    denominator: str
    refuses: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsMetricDefinitionReport":
        raw = _route_mapping("operations metric definition", value)
        return cls(raw, _route_text("operations metric", raw.get("metric")), _bool("operations metric blueprint_name", raw.get("blueprint_name")), _route_text("operations metric numerator", raw.get("numerator")), _route_text("operations metric denominator", raw.get("denominator")), _route_text("operations metric refuses", raw.get("refuses")))


@dataclass(frozen=True)
class OperationsUndefinedMetricReport:
    raw: dict[str, Any]
    origin: str
    module_title: str
    metric: str
    denominator: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsUndefinedMetricReport":
        raw = _route_mapping("operations undefined metric", value)
        return cls(raw, _route_text("operations undefined metric origin", raw.get("origin")), _route_text("operations undefined metric module_title", raw.get("module_title")), _route_text("operations undefined metric metric", raw.get("metric")), _optional_text("operations undefined metric denominator", raw.get("denominator")))


@dataclass(frozen=True)
class OperationsMetricsReport:
    raw: dict[str, Any]
    metrics_schema_version: str
    atlasx_schema_version: str
    named_in_scope: int
    named_but_undefined: int
    defined_here: tuple[OperationsMetricDefinitionReport, ...]
    undefined_metrics_returned: tuple[OperationsUndefinedMetricReport, ...]
    omitted_undefined_metrics: int
    undefined_is_not_zero: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsMetricsReport":
        raw = _route_mapping("operations metrics", value)
        defined_raw = raw.get("defined_here")
        undefined_raw = raw.get("undefined_metrics_returned")
        if not isinstance(defined_raw, Sequence) or isinstance(defined_raw, (str, bytes)):
            raise ArgumentError("operations defined_here must be an array")
        if not isinstance(undefined_raw, Sequence) or isinstance(undefined_raw, (str, bytes)):
            raise ArgumentError("operations undefined_metrics_returned must be an array")
        defined = tuple(OperationsMetricDefinitionReport.from_wire(item) for item in defined_raw)
        undefined = tuple(OperationsUndefinedMetricReport.from_wire(item) for item in undefined_raw)
        named_in_scope = _route_count("operations named_in_scope", raw.get("named_in_scope"))
        named_but_undefined = _route_count("operations named_but_undefined", raw.get("named_but_undefined"))
        omitted = _route_count("operations omitted_undefined_metrics", raw.get("omitted_undefined_metrics"))
        if named_but_undefined != len(undefined) + omitted or named_but_undefined > named_in_scope:
            raise ArgumentError("operations metric undefined count does not reconcile")
        return cls(raw, _route_text("operations metrics schema version", raw.get("metrics_schema_version")), _route_text("operations atlasx schema version", raw.get("atlasx_schema_version")), named_in_scope, named_but_undefined, defined, undefined, omitted, _bool("operations undefined_is_not_zero", raw.get("undefined_is_not_zero")))


@dataclass(frozen=True)
class OperationsSdkReport:
    raw: dict[str, Any]
    registration_note: str
    execution_and_isolation_are_not_implied: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsSdkReport":
        raw = _route_mapping("operations sdk", value)
        return cls(raw, _route_text("operations registration_note", raw.get("registration_note")), _bool("operations execution_and_isolation_are_not_implied", raw.get("execution_and_isolation_are_not_implied")))


@dataclass(frozen=True)
class OperationsCatalogReport:
    raw: dict[str, Any]
    ok: bool
    detail_mode: str
    max_items: int
    local: OperationsTopologyReport
    team: OperationsTopologyReport
    promise_parity: OperationsPromiseParityReport
    technology_is_not_promise_parity: bool
    data_classes: tuple[OperationsDataClassReport, ...]
    deployment_planes: tuple[OperationsDeploymentPlaneReport, ...]
    tenant_patterns: tuple[OperationsTenantPatternReport, ...]
    slo_objectives: tuple[str, ...]
    service_contracts: OperationsServiceContractsReport
    metrics: OperationsMetricsReport
    sdk: OperationsSdkReport
    details: dict[str, Any] | None
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationsCatalogReport":
        raw = _payload(value, ("topologies", "service_contracts", "metrics"), "operations catalog")
        if not _bool("operations catalog ok", raw.get("ok")):
            raise ArgumentError("operations catalog report is not successful")
        detail_mode = _route_text("operations detail_mode", raw.get("detail_mode"))
        if detail_mode not in {"summary", "full"}:
            raise ArgumentError(f"unknown operations detail_mode: {detail_mode!r}")
        max_items = _bounded_max_items("operations max_items", raw.get("max_items"))
        topologies = _route_mapping("operations topologies", raw.get("topologies"))
        local = OperationsTopologyReport.from_wire(topologies.get("local"))
        team = OperationsTopologyReport.from_wire(topologies.get("team"))
        if local.deployment != "local" or team.deployment != "team":
            raise ArgumentError("operations topologies must be labelled local and team")
        parity = OperationsPromiseParityReport.from_wire(topologies.get("promise_parity"))
        if parity.compared != len(local.classes) or parity.compared != len(team.classes):
            raise ArgumentError("operations promise parity compared count does not match topologies")
        if not _bool("operations technology_is_not_promise_parity", topologies.get("technology_is_not_promise_parity")):
            raise ArgumentError("operations topology must state that technology is not promise parity")

        def rows(name: str, factory: Any) -> tuple[Any, ...]:
            value = raw.get(name)
            if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
                raise ArgumentError(f"operations {name} must be an array")
            return tuple(factory(item) for item in value)

        data_classes = rows("data_classes", OperationsDataClassReport.from_wire)
        if len(data_classes) != len(OPERATIONS_DATA_CLASSES) or {item.class_id for item in data_classes} != OPERATIONS_DATA_CLASSES:
            raise ArgumentError("operations data_classes do not cover the closed five-class set")
        deployment_planes = rows("deployment_planes", OperationsDeploymentPlaneReport.from_wire)
        if len(deployment_planes) != len(OPERATIONS_DEPLOYMENT_PLANES) or {item.plane for item in deployment_planes} != OPERATIONS_DEPLOYMENT_PLANES:
            raise ArgumentError("operations deployment_planes do not cover the closed nine-plane set")
        tenant_patterns = rows("tenant_patterns", OperationsTenantPatternReport.from_wire)
        if len(tenant_patterns) != len(OPERATIONS_TENANT_PATTERNS) or {item.pattern for item in tenant_patterns} != OPERATIONS_TENANT_PATTERNS:
            raise ArgumentError("operations tenant_patterns do not cover the closed pattern set")
        slo_objectives = _route_strings("operations slo_objectives", raw.get("slo_objectives"))
        details = raw.get("details")
        if detail_mode == "full":
            details = _route_mapping("operations details", details)
            if not isinstance(details.get("service_entries"), Sequence) or isinstance(details.get("service_entries"), (str, bytes)):
                raise ArgumentError("full operations details must contain service_entries")
            if not isinstance(details.get("undefined_metrics"), Sequence) or isinstance(details.get("undefined_metrics"), (str, bytes)):
                raise ArgumentError("full operations details must contain undefined_metrics")
        elif details is not None and not isinstance(details, Mapping):
            raise ArgumentError("operations details must be an object when supplied")
        return cls(raw, True, detail_mode, max_items, local, team, parity, True, tuple(data_classes), tuple(deployment_planes), tuple(tenant_patterns), slo_objectives, OperationsServiceContractsReport.from_wire(raw.get("service_contracts")), OperationsMetricsReport.from_wire(raw.get("metrics")), OperationsSdkReport.from_wire(raw.get("sdk")), dict(details) if isinstance(details, Mapping) else None, _route_strings("operations limitations", raw.get("limitations", [])))

    @property
    def promise_parity_holds(self) -> bool:
        return self.promise_parity.holds

    @property
    def metric_debt_count(self) -> int:
        return self.metrics.named_but_undefined

    @property
    def service_contracts_all_satisfied(self) -> bool:
        return self.service_contracts.summary.satisfied == self.service_contracts.summary.total


@dataclass(frozen=True)
class OpsAcceptanceArgs:
    max_items: int = OPERATIONS_DEFAULT_MAX_ITEMS

    def __post_init__(self) -> None:
        _bounded_max_items("max_items", self.max_items)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"max_items": self.max_items}


@dataclass(frozen=True)
class OpsAcceptanceBasisReport:
    raw: dict[str, Any]
    basis: str
    krate: str | None
    item: str | None
    who: str | None
    because: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OpsAcceptanceBasisReport":
        raw = _route_mapping("operations acceptance basis", value)
        basis = _route_text("operations acceptance basis kind", raw.get("basis"))
        if basis not in OPS_ACCEPTANCE_BASES:
            raise ArgumentError(f"unknown operations acceptance basis: {basis!r}")
        krate = _optional_text("operations acceptance basis krate", raw.get("krate"))
        item = _optional_text("operations acceptance basis item", raw.get("item"))
        who = _optional_text("operations acceptance basis who", raw.get("who"))
        because = _optional_text("operations acceptance basis because", raw.get("because"))
        if basis == "linked_type" and (krate is None or item is None):
            raise ArgumentError("linked_type acceptance basis must name krate and item")
        if basis == "author" and who is None:
            raise ArgumentError("author acceptance basis must name who")
        if basis == "no_observer" and because is None:
            raise ArgumentError("no_observer acceptance basis must name because")
        return cls(raw, basis, krate, item, who, because)


@dataclass(frozen=True)
class OpsAcceptanceFindingReport:
    raw: dict[str, Any]
    criterion: str
    verdict: str
    basis: OpsAcceptanceBasisReport
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OpsAcceptanceFindingReport":
        raw = _route_mapping("operations acceptance finding", value)
        verdict = _route_text("operations acceptance verdict", raw.get("verdict"))
        if verdict not in OPS_ACCEPTANCE_VERDICTS:
            raise ArgumentError(f"unknown operations acceptance verdict: {verdict!r}")
        return cls(raw, _route_text("operations acceptance criterion", raw.get("criterion")), verdict, OpsAcceptanceBasisReport.from_wire(raw.get("basis")), _route_text("operations acceptance detail", raw.get("detail")))


@dataclass(frozen=True)
class OpsAcceptanceSummaryReport:
    raw: dict[str, Any]
    met: int
    refuted: int
    unverifiable: int
    total: int
    is_release_ready: bool
    is_decidable: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OpsAcceptanceSummaryReport":
        raw = _route_mapping("operations acceptance summary", value)
        met = _route_count("operations acceptance met", raw.get("met"))
        refuted = _route_count("operations acceptance refuted", raw.get("refuted"))
        unverifiable = _route_count("operations acceptance unverifiable", raw.get("unverifiable"))
        total = _route_count("operations acceptance total", raw.get("total"))
        is_release_ready = _bool("operations acceptance is_release_ready", raw.get("is_release_ready"))
        is_decidable = _bool("operations acceptance is_decidable", raw.get("is_decidable"))
        if total != met + refuted + unverifiable or is_release_ready != (met == total) or is_decidable != (unverifiable == 0):
            raise ArgumentError("operations acceptance summary does not reconcile")
        return cls(raw, met, refuted, unverifiable, total, is_release_ready, is_decidable)


@dataclass(frozen=True)
class OpsAcceptanceReport:
    raw: dict[str, Any]
    ok: bool
    summary: OpsAcceptanceSummaryReport
    findings: tuple[OpsAcceptanceFindingReport, ...]
    omitted_findings: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OpsAcceptanceReport":
        raw = _payload(value, ("summary", "findings", "omitted_findings"), "operations acceptance")
        ok = _bool("operations acceptance ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("operations acceptance report is not successful")
        raw_findings = raw.get("findings")
        if not isinstance(raw_findings, Sequence) or isinstance(raw_findings, (str, bytes)):
            raise ArgumentError("operations acceptance findings must be an array")
        findings = tuple(OpsAcceptanceFindingReport.from_wire(item) for item in raw_findings)
        criteria = [finding.criterion for finding in findings]
        if len(criteria) != len(set(criteria)):
            raise ArgumentError("operations acceptance criteria must be unique in the returned page")
        omitted = _route_count("operations acceptance omitted_findings", raw.get("omitted_findings"))
        summary = OpsAcceptanceSummaryReport.from_wire(raw.get("summary"))
        if summary.total != len(findings) + omitted:
            raise ArgumentError("operations acceptance finding count does not reconcile")
        return cls(raw, ok, summary, findings, omitted, _route_strings("operations acceptance guarantees", raw.get("guarantees", [])), _route_strings("operations acceptance limitations", raw.get("limitations", [])))

    @property
    def release_ready(self) -> bool:
        return self.summary.is_release_ready

    @property
    def decidable(self) -> bool:
        return self.summary.is_decidable

    @property
    def verdict_counts(self) -> dict[str, int]:
        return {"met": self.summary.met, "refuted": self.summary.refuted, "unverifiable": self.summary.unverifiable}


def operations_catalog_report(value: Mapping[str, Any]) -> OperationsCatalogReport:
    """Parse direct MCP or HTTP operations-catalogue output."""

    return OperationsCatalogReport.from_wire(value)


def ops_acceptance_report(value: Mapping[str, Any]) -> OpsAcceptanceReport:
    """Parse direct MCP or HTTP operations-acceptance output."""

    return OpsAcceptanceReport.from_wire(value)


__all__ = [
    "OPERATIONS_DATA_CLASSES",
    "OPERATIONS_DEFAULT_MAX_ITEMS",
    "OPERATIONS_DEPLOYMENT_PLANES",
    "OPERATIONS_DURABILITIES",
    "OPERATIONS_MAX_ITEMS",
    "OPERATIONS_MUTABILITIES",
    "OPERATIONS_TENANT_PATTERNS",
    "OPS_ACCEPTANCE_BASES",
    "OPS_ACCEPTANCE_VERDICTS",
    "OperationsCatalogArgs",
    "OperationsCatalogReport",
    "OperationsDataClassReport",
    "OperationsDeploymentPlaneReport",
    "OperationsMetricDefinitionReport",
    "OperationsMetricsReport",
    "OperationsPromiseParityReport",
    "OperationsSdkReport",
    "OperationsServiceContractReport",
    "OperationsServiceContractsReport",
    "OperationsServiceSummaryReport",
    "OperationsStoreReport",
    "OperationsTenantPatternReport",
    "OperationsTopologyClassReport",
    "OperationsTopologyReport",
    "OperationsUndefinedMetricReport",
    "OpsAcceptanceArgs",
    "OpsAcceptanceBasisReport",
    "OpsAcceptanceFindingReport",
    "OpsAcceptanceReport",
    "OpsAcceptanceSummaryReport",
    "operations_catalog_report",
    "ops_acceptance_report",
]
