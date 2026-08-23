"""Typed cross-domain acquisition and adapter-conformance catalogue reports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError


DOMAIN_ACQUISITION_SCHEMA = "bioprism-devplat-domain-acquisition/0.1"
DOMAIN_ACQUISITION_WORKFLOW = "domain_acquisition_catalogue"
MAX_DOMAIN_ACQUISITION_GROUPS = 64
MAX_DOMAIN_ACQUISITION_DOMAINS = 512


def _filter(name: str, value: str | None) -> None:
    if value is None:
        return
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string when supplied")
    if len(value.encode("utf-8")) > 512:
        raise ArgumentError(f"{name} exceeds the 512-byte safety bound")
    if any(character in value for character in "\x00\r\n"):
        raise ArgumentError(f"{name} contains a control character")


@dataclass(frozen=True)
class DomainAcquisitionQuery:
    """Bounded group/domain filters for the server-side catalogue."""

    group_id: str | None = None
    domain: str | None = None
    include_adapters: bool = False
    max_groups: int = MAX_DOMAIN_ACQUISITION_GROUPS
    max_domains: int = MAX_DOMAIN_ACQUISITION_DOMAINS

    def __post_init__(self) -> None:
        _filter("group_id", self.group_id)
        _filter("domain", self.domain)
        if not isinstance(self.include_adapters, bool):
            raise ArgumentError("include_adapters must be a boolean")
        if isinstance(self.max_groups, bool) or not isinstance(self.max_groups, int) or not 1 <= self.max_groups <= MAX_DOMAIN_ACQUISITION_GROUPS:
            raise ArgumentError(f"max_groups must be between 1 and {MAX_DOMAIN_ACQUISITION_GROUPS}")
        if isinstance(self.max_domains, bool) or not isinstance(self.max_domains, int) or not 1 <= self.max_domains <= MAX_DOMAIN_ACQUISITION_DOMAINS:
            raise ArgumentError(f"max_domains must be between 1 and {MAX_DOMAIN_ACQUISITION_DOMAINS}")

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "include_adapters": self.include_adapters,
            "max_groups": self.max_groups,
            "max_domains": self.max_domains,
        }
        if self.group_id is not None:
            result["group_id"] = self.group_id
        if self.domain is not None:
            result["domain"] = self.domain
        return result


@dataclass(frozen=True)
class DomainAcquisitionRouteReport:
    """One domain route with transport and interpretation kept separate."""

    raw: dict[str, Any]
    group_id: str
    domain: str
    declared_tool_count: int
    transport: Mapping[str, Any]
    interpretation: Mapping[str, Any]
    adapter_ids: tuple[str, ...]
    adapters: tuple[Mapping[str, Any], ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainAcquisitionRouteReport":
        raw = _route_mapping("domain acquisition route", value)
        transport = _route_mapping("domain acquisition transport", raw.get("transport"))
        interpretation = _route_mapping("domain acquisition interpretation", raw.get("interpretation"))
        adapter_ids = _route_strings("domain acquisition adapter_ids", interpretation.get("adapter_ids", []))
        raw_adapters = raw.get("adapters") or []
        if not isinstance(raw_adapters, list):
            raise ArgumentError("domain acquisition adapters must be an array when present")
        adapters = tuple(_route_mapping("domain acquisition adapter", item) for item in raw_adapters)
        declared_count = raw.get("declared_tool_count")
        if isinstance(declared_count, bool) or not isinstance(declared_count, int) or declared_count < 0:
            raise ArgumentError("domain acquisition declared_tool_count must be a non-negative integer")
        return cls(
            raw=raw,
            group_id=_route_text("domain acquisition route group_id", raw.get("group_id")),
            domain=_route_text("domain acquisition route domain", raw.get("domain")),
            declared_tool_count=declared_count,
            transport=transport,
            interpretation=interpretation,
            adapter_ids=adapter_ids,
            adapters=adapters,
            limitations=_route_strings("domain acquisition route limitations", raw.get("limitations", [])),
        )

    @property
    def transport_status(self) -> str:
        return _route_text("domain acquisition transport status", self.transport.get("status"))

    @property
    def caller_managed_tools(self) -> tuple[str, ...]:
        return _route_strings(
            "domain acquisition caller-managed tools",
            self.transport.get("caller_managed_tools", []),
        )

    @property
    def interpretation_status(self) -> str:
        return _route_text("domain acquisition interpretation status", self.interpretation.get("status"))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainAcquisitionReport:
    """Digest-bound response for the cross-domain acquisition catalogue tool."""

    raw: dict[str, Any]
    catalogue: Mapping[str, Any]
    schema: str
    workflow: str
    execution: str
    complete: bool
    truncated: bool
    digest: str
    routes: tuple[DomainAcquisitionRouteReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainAcquisitionReport":
        raw = _tool_payload(value, DOMAIN_ACQUISITION_WORKFLOW)
        if raw.get("ok") is False:
            raise ArgumentError("domain acquisition catalogue report is not successful")
        if raw.get("workflow") != DOMAIN_ACQUISITION_WORKFLOW:
            raise ArgumentError("domain acquisition catalogue workflow is invalid")
        schema = _route_text("domain acquisition schema", raw.get("schema"))
        if schema != DOMAIN_ACQUISITION_SCHEMA:
            raise ArgumentError(f"unexpected domain acquisition schema: {schema!r}")
        execution = _route_text("domain acquisition execution", raw.get("execution"))
        if execution != "not_started":
            raise ArgumentError("domain acquisition execution must remain not_started")
        catalogue = _route_mapping("domain acquisition catalogue", raw.get("catalogue"))
        if _route_text("domain acquisition catalogue schema", catalogue.get("schema")) != DOMAIN_ACQUISITION_SCHEMA:
            raise ArgumentError("domain acquisition nested catalogue schema is invalid")
        routes_value = catalogue.get("routes", [])
        if not isinstance(routes_value, list):
            raise ArgumentError("domain acquisition catalogue routes must be an array")
        routes = tuple(DomainAcquisitionRouteReport.from_wire(route) for route in routes_value)
        complete = catalogue.get("complete")
        truncated = catalogue.get("truncated")
        if not isinstance(complete, bool) or not isinstance(truncated, bool):
            raise ArgumentError("domain acquisition completeness flags must be booleans")
        digest = _route_text("domain acquisition digest", catalogue.get("digest"))
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
            raise ArgumentError("domain acquisition digest must be lowercase SHA-256")
        return cls(
            raw=raw,
            catalogue=catalogue,
            schema=schema,
            workflow=DOMAIN_ACQUISITION_WORKFLOW,
            execution=execution,
            complete=complete,
            truncated=truncated,
            digest=digest,
            routes=routes,
            guarantees=_route_strings("domain acquisition guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("domain acquisition limitations", raw.get("does_not_claim", [])),
        )

    @property
    def selected_domain_count(self) -> int:
        value = self.catalogue.get("selected_domain_count")
        if isinstance(value, bool) or not isinstance(value, int):
            raise ArgumentError("domain acquisition selected_domain_count must be an integer")
        return value

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_acquisition_report(value: Mapping[str, Any]) -> DomainAcquisitionReport:
    """Parse a direct tool result or an HTTP REST-tool envelope."""

    return DomainAcquisitionReport.from_wire(value)


__all__ = [
    "DOMAIN_ACQUISITION_SCHEMA",
    "DOMAIN_ACQUISITION_WORKFLOW",
    "MAX_DOMAIN_ACQUISITION_DOMAINS",
    "MAX_DOMAIN_ACQUISITION_GROUPS",
    "DomainAcquisitionQuery",
    "DomainAcquisitionReport",
    "DomainAcquisitionRouteReport",
    "domain_acquisition_report",
]
