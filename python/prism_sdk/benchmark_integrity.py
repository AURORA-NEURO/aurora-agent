"""Typed benchmark portfolio integrity projections.

This surface composes exact/structural deduplication, deterministic holdout assignment,
contamination assessment, panel calibration, and effective-diversity accounting.  The input
contracts are intentionally serialized projections of the Rust compiler's rich types; the report
keeps independent denominators and omission counts instead of collapsing portfolio health into one
score.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_INTEGRITY_AUDIT_SCHEMA = "bioprism-mcp/benchmark-integrity-audit/0.1"
MAX_INTEGRITY_ITEMS = 1_000
MAX_INTEGRITY_RECORDS = 100_000
MAX_INTEGRITY_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _mapping_array(name: str, value: Any, *, limit: int = MAX_INTEGRITY_RECORDS) -> tuple[dict[str, Any], ...]:
    values = _array(name, value)
    if len(values) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return tuple(dict(_route_mapping(f"{name}[{index}]", item)) for index, item in enumerate(values))


def _text_array(name: str, value: Any, *, limit: int = MAX_INTEGRITY_RECORDS) -> tuple[str, ...]:
    values = _array(name, value)
    if len(values) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(values))


def _object_map(name: str, value: Any, *, values_are_arrays: bool = False) -> dict[str, Any]:
    raw = _route_mapping(name, value)
    if len(raw) > MAX_INTEGRITY_RECORDS:
        raise ArgumentError(f"{name} is bounded at {MAX_INTEGRITY_RECORDS} entries")
    result: dict[str, Any] = {}
    for key, item in raw.items():
        if not isinstance(key, str):
            raise ArgumentError(f"{name} keys must be strings")
        if values_are_arrays:
            result[key] = _mapping_array(f"{name}[{key!r}]", item)
        else:
            result[key] = dict(_route_mapping(f"{name}[{key!r}]", item))
    return result


def _index(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("benchmark integrity response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BENCHMARK_INTEGRITY_AUDIT_SCHEMA and isinstance(candidate.get("dedup"), Mapping) and isinstance(candidate.get("effective_diversity"), Mapping)
        return candidate.get("ok") is False and isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"benchmark integrity response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark integrity projection")


@dataclass(frozen=True)
class BenchmarkIntegrityAuditArgs:
    """Bounded portfolio declarations and observed integrity evidence."""

    instances: tuple[Mapping[str, Any], ...]
    panel_runs: tuple[Mapping[str, Any], ...] = ()
    bench_instances: tuple[Mapping[str, Any], ...] = ()
    known_instances: tuple[str, ...] = ()
    safety_vetoes: tuple[str, ...] = ()
    exposure: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    probes: Mapping[str, Sequence[Mapping[str, Any]]] = field(default_factory=dict)
    private_share: int = 20
    rotating_panels: int = 0
    max_items: int = 100

    def __post_init__(self) -> None:
        instances = _mapping_array("benchmark instances", self.instances)
        panel_runs = _mapping_array("benchmark panel_runs", self.panel_runs)
        bench_instances = _mapping_array("benchmark bench_instances", self.bench_instances)
        known_instances = _text_array("benchmark known_instances", self.known_instances)
        safety_vetoes = _text_array("benchmark safety_vetoes", self.safety_vetoes)
        exposure = _object_map("benchmark exposure", self.exposure)
        probes = _object_map("benchmark probes", self.probes, values_are_arrays=True)
        if not isinstance(self.private_share, int) or isinstance(self.private_share, bool) or not 0 <= self.private_share <= 100:
            raise ArgumentError("benchmark private_share must be between 0 and 100")
        if not isinstance(self.rotating_panels, int) or isinstance(self.rotating_panels, bool) or not 0 <= self.rotating_panels <= 1_000:
            raise ArgumentError("benchmark rotating_panels must be between 0 and 1000")
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= MAX_INTEGRITY_ITEMS:
            raise ArgumentError("benchmark integrity max_items must be between 1 and 1000")
        arguments = {
            "instances": [dict(item) for item in instances],
            "panel_runs": [dict(item) for item in panel_runs],
            "bench_instances": [dict(item) for item in bench_instances],
            "known_instances": list(known_instances),
            "safety_vetoes": list(safety_vetoes),
            "exposure": {key: dict(value) for key, value in exposure.items()},
            "probes": {key: [dict(item) for item in value] for key, value in probes.items()},
            "private_share": self.private_share,
            "rotating_panels": self.rotating_panels,
            "max_items": self.max_items,
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark integrity arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_INTEGRITY_INPUT_BYTES:
            raise ArgumentError("benchmark integrity input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "instances", instances)
        object.__setattr__(self, "panel_runs", panel_runs)
        object.__setattr__(self, "bench_instances", bench_instances)
        object.__setattr__(self, "known_instances", known_instances)
        object.__setattr__(self, "safety_vetoes", safety_vetoes)
        object.__setattr__(self, "exposure", exposure)
        object.__setattr__(self, "probes", probes)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkIntegrityAuditArgs":
        raw = _route_mapping("benchmark integrity arguments", value)
        return cls(
            _mapping_array("benchmark instances", raw.get("instances")),
            _mapping_array("benchmark panel_runs", raw.get("panel_runs", [])),
            _mapping_array("benchmark bench_instances", raw.get("bench_instances", [])),
            _text_array("benchmark known_instances", raw.get("known_instances", [])),
            _text_array("benchmark safety_vetoes", raw.get("safety_vetoes", [])),
            _object_map("benchmark exposure", raw.get("exposure", {})),
            _object_map("benchmark probes", raw.get("probes", {}), values_are_arrays=True),
            raw.get("private_share", 20),
            raw.get("rotating_panels", 0),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "instances": [dict(item) for item in self.instances],
            "panel_runs": [dict(item) for item in self.panel_runs],
            "bench_instances": [dict(item) for item in self.bench_instances],
            "known_instances": list(self.known_instances),
            "safety_vetoes": list(self.safety_vetoes),
            "exposure": {key: dict(value) for key, value in self.exposure.items()},
            "probes": {key: [dict(item) for item in value] for key, value in self.probes.items()},
            "private_share": self.private_share,
            "rotating_panels": self.rotating_panels,
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class BenchmarkIntegrityAuditReport:
    """Typed portfolio integrity evidence with separate dedup, exposure, and denominator layers."""

    raw: dict[str, Any]
    ok: bool
    schema: str | None
    instance_digest: str | None
    counts: Mapping[str, int]
    dedup: Mapping[str, Any] | None
    holdout: Mapping[str, Any] | None
    contamination: Mapping[str, Any] | None
    calibration: Mapping[str, Any] | None
    effective_diversity: Mapping[str, Any] | None
    guarantees: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkIntegrityAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark integrity refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, {}, None, None, None, None, None, _route_strings("benchmark integrity refusal guarantees", raw.get("guarantees", [])), _route_text("benchmark integrity refusal stage", raw.get("stage")), _route_text("benchmark integrity refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != BENCHMARK_INTEGRITY_AUDIT_SCHEMA:
            raise ArgumentError("benchmark integrity projection has an invalid schema")
        counts_raw = _route_mapping("benchmark integrity counts", raw.get("counts"))
        counts = {key: _route_count(f"benchmark integrity count {key}", counts_raw.get(key)) for key in ("instances", "panel_runs", "bench_instances", "known_instances", "safety_vetoes")}
        contamination = _route_mapping("benchmark contamination projection", raw.get("contamination"))
        admissible = _route_count("benchmark contamination admissible", contamination.get("admissible"))
        if admissible > counts["instances"]:
            raise ArgumentError("benchmark contamination admissible count exceeds instances")
        holdout = _route_mapping("benchmark holdout projection", raw.get("holdout"))
        holdout_counts = _route_mapping("benchmark holdout counts", holdout.get("counts"))
        if sum(_route_count(f"benchmark holdout {key}", value) for key, value in holdout_counts.items()) != counts["instances"]:
            raise ArgumentError("benchmark holdout counts do not reconcile with instances")
        diversity = _route_mapping("benchmark effective diversity", raw.get("effective_diversity"))
        effective = _route_count("benchmark effective sample size", diversity.get("equivalence_classes"))
        if effective > _route_count("benchmark diversity instances", diversity.get("instances")):
            raise ArgumentError("benchmark effective diversity exceeds instance count")
        return cls(
            raw,
            True,
            BENCHMARK_INTEGRITY_AUDIT_SCHEMA,
            _route_text("benchmark instance_digest", raw.get("instance_digest")),
            counts,
            _route_mapping("benchmark dedup projection", raw.get("dedup")),
            holdout,
            contamination,
            _route_mapping("benchmark calibration projection", raw.get("calibration")),
            diversity,
            _route_strings("benchmark integrity guarantees", raw.get("guarantees", [])),
            None,
            None,
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def admissible_instances(self) -> int:
        return 0 if self.contamination is None else int(self.contamination.get("admissible", 0))

    @property
    def effective_sample_size(self) -> int | None:
        return None if self.effective_diversity is None else int(self.effective_diversity["equivalence_classes"])

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def benchmark_integrity_audit_report(value: Mapping[str, Any]) -> BenchmarkIntegrityAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkIntegrityAuditReport.from_wire(value)


__all__ = [
    "BENCHMARK_INTEGRITY_AUDIT_SCHEMA",
    "MAX_INTEGRITY_ITEMS",
    "MAX_INTEGRITY_RECORDS",
    "MAX_INTEGRITY_INPUT_BYTES",
    "BenchmarkIntegrityAuditArgs",
    "BenchmarkIntegrityAuditReport",
    "benchmark_integrity_audit_report",
]
