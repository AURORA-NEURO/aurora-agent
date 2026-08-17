"""Typed requests and reports for external CI provider payload normalization."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError

CI_PROVIDER_NORMALIZATION_SCHEMA = "bioprism-devplat-ci-provider-normalization/0.1"


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or not value:
        raise ArgumentError(f"{name} must be a non-empty mapping")
    return dict(value)


@dataclass(frozen=True)
class CiProviderNormalizationRequest:
    ci: Mapping[str, Any]
    provider: str
    payload: Mapping[str, Any]
    source: str | None = None

    def __post_init__(self) -> None:
        _mapping("ci", self.ci)
        provider = _route_text("provider", self.provider).lower()
        if provider not in {"github_actions", "gitlab_ci", "generic"}:
            raise ArgumentError("provider must be github_actions, gitlab_ci, or generic")
        _mapping("payload", self.payload)
        if self.source is not None and self.source not in {"caller_attested", "provider_observed"}:
            raise ArgumentError("source must be caller_attested or provider_observed")

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "ci": dict(self.ci),
            "provider": self.provider,
            "payload": dict(self.payload),
        }
        if self.source is not None:
            result["source"] = self.source
        return result


@dataclass(frozen=True)
class CiProviderNormalizationReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    provider: str
    source: str
    payload_digest: str
    run_id: str
    conclusion: str
    check_count: int
    derived_result_digest_count: int
    warnings: tuple[str, ...]
    evidence: dict[str, Any]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiProviderNormalizationReport":
        raw = _tool_payload(value, "ci_provider_normalize")
        if raw.get("ok") is not True:
            raise ArgumentError("CI provider normalization report is not successful")
        normalization = _route_mapping("CI provider normalization", raw.get("normalization"))
        evidence = _route_mapping("CI provider normalized evidence", raw.get("evidence"))
        return cls(
            raw=raw,
            schema=_route_text("CI provider normalization schema", raw.get("schema")),
            workflow=_route_text("CI provider normalization workflow", raw.get("workflow")),
            provider=_route_text("CI provider", normalization.get("provider")),
            source=_route_text("CI provider source", normalization.get("source")),
            payload_digest=_route_text("CI provider payload digest", normalization.get("payload_digest")),
            run_id=_route_text("CI provider run_id", normalization.get("run_id")),
            conclusion=_route_text("CI provider conclusion", normalization.get("conclusion")),
            check_count=_route_count("CI provider check_count", normalization.get("check_count")),
            derived_result_digest_count=_route_count(
                "CI provider derived_result_digest_count",
                normalization.get("derived_result_digest_count"),
            ),
            warnings=_route_strings("CI provider warnings", normalization.get("warnings", [])),
            evidence=evidence,
            guarantees=_route_strings("CI provider guarantees", normalization.get("guarantees", [])),
            limitations=_route_strings("CI provider limitations", normalization.get("limitations", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def ci_provider_normalization_report(value: Mapping[str, Any]) -> CiProviderNormalizationReport:
    """Parse a direct MCP result or HTTP REST tool envelope."""

    return CiProviderNormalizationReport.from_wire(value)


__all__ = [
    "CI_PROVIDER_NORMALIZATION_SCHEMA",
    "CiProviderNormalizationRequest",
    "CiProviderNormalizationReport",
    "ci_provider_normalization_report",
]
