"""Typed projections for deterministic synthetic world/query generation.

The Rust generator is the authority for ``WorldSpec`` semantics and document construction.  The
SDK only validates the bounded request envelope and projects the evidence that matters at the
transport boundary: both generated documents were parsed, structural validation diagnostics were
returned, exact world/query digests were bound, and optional documents were either included or
explicitly withheld.  A generation refusal never becomes an empty world or a synthetic zero.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


WORLD_GENERATION_MAX_INPUT_BYTES = 20_000_000
WORLD_GENERATION_MAX_SUBJECTS = 1_000
WORLD_GENERATION_MAX_DISTRACTORS = 10_000
WORLD_GENERATION_MAX_RELAY_DEPTH = 64
WORLD_GENERATION_STAGES = frozenset({"generated_world_parse", "generated_query_parse", "generated_world_validation"})
WORLD_GENERATION_SEVERITIES = frozenset({"warning", "error"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("world generation response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return isinstance(candidate.get("counts"), Mapping) and isinstance(candidate.get("validation"), Mapping)
        return candidate.get("ok") is False and isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

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
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"world generation response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a world-generation projection")


@dataclass(frozen=True)
class WorldGenerateArgs:
    spec: Mapping[str, Any]
    include_world: bool = False
    include_query: bool = False

    def __post_init__(self) -> None:
        spec = _route_mapping("world generation spec", self.spec)
        include_world = _bool("world generation include_world", self.include_world)
        include_query = _bool("world generation include_query", self.include_query)
        arguments = {"spec": spec, "include_world": include_world, "include_query": include_query}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"world generation arguments are not JSON serializable: {error}") from error
        if len(encoded) > WORLD_GENERATION_MAX_INPUT_BYTES:
            raise ArgumentError("world generation input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "spec", spec)
        object.__setattr__(self, "include_world", include_world)
        object.__setattr__(self, "include_query", include_query)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldGenerateArgs":
        raw = _route_mapping("world generation arguments", value)
        return cls(_route_mapping("world generation spec", raw.get("spec")), raw.get("include_world", False), raw.get("include_query", False))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"spec": dict(self.spec), "include_world": self.include_world, "include_query": self.include_query}


@dataclass(frozen=True)
class WorldDiagnosticReport:
    raw: dict[str, Any]
    severity: str
    code: str
    subject: str
    message: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldDiagnosticReport":
        raw = _route_mapping("world generation diagnostic", value)
        severity = _route_text("world generation diagnostic severity", raw.get("severity"))
        if severity not in WORLD_GENERATION_SEVERITIES:
            raise ArgumentError(f"unknown world generation diagnostic severity {severity!r}")
        return cls(raw, severity, _route_text("world generation diagnostic code", raw.get("code")), _route_text("world generation diagnostic subject", raw.get("subject")), _route_text("world generation diagnostic message", raw.get("message")))


@dataclass(frozen=True)
class WorldValidationReport:
    raw: dict[str, Any]
    errors: int
    warnings: int
    diagnostics: tuple[WorldDiagnosticReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldValidationReport":
        raw = _route_mapping("world generation validation", value)
        diagnostics = tuple(WorldDiagnosticReport.from_wire(item) for item in _sequence("world generation diagnostics", raw.get("diagnostics", [])))
        errors = _route_count("world generation validation errors", raw.get("errors"))
        warnings = _route_count("world generation validation warnings", raw.get("warnings"))
        if errors + warnings != len(diagnostics):
            raise ArgumentError("world generation validation counts do not match diagnostics")
        if errors != sum(item.severity == "error" for item in diagnostics):
            raise ArgumentError("world generation error count does not match diagnostic severities")
        return cls(raw, errors, warnings, diagnostics)

    @property
    def clean(self) -> bool:
        return self.errors == 0 and self.warnings == 0


@dataclass(frozen=True)
class WorldGenerationCountsReport:
    raw: dict[str, Any]
    facts: int
    factors: int
    events: int
    subjects: int
    distractors: int
    relay_depth: int
    generated_query_targets: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldGenerationCountsReport":
        raw = _route_mapping("world generation counts", value)
        return cls(raw, _route_count("world generation facts", raw.get("facts")), _route_count("world generation factors", raw.get("factors")), _route_count("world generation events", raw.get("events")), _route_count("world generation subjects", raw.get("subjects")), _route_count("world generation distractors", raw.get("distractors")), _route_count("world generation relay_depth", raw.get("relay_depth")), _route_count("world generation generated_query_targets", raw.get("generated_query_targets")))


@dataclass(frozen=True)
class WorldGenerateReport:
    raw: dict[str, Any]
    ok: bool
    world_id: str | None
    query_id: str | None
    world_digest: str | None
    query_digest: str | None
    counts: WorldGenerationCountsReport | None
    validation: WorldValidationReport | None
    world: dict[str, Any] | None
    query: dict[str, Any] | None
    guarantees: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool | None
    diagnostics: tuple[WorldDiagnosticReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorldGenerateReport":
        raw = _payload(value)
        ok = _bool("world generation ok", raw.get("ok"))
        guarantees = _route_strings("world generation guarantees", raw.get("guarantees", []))
        if not ok:
            stage = _route_text("world generation refusal stage", raw.get("stage"))
            if stage not in WORLD_GENERATION_STAGES:
                raise ArgumentError(f"unknown world generation refusal stage {stage!r}")
            diagnostics = tuple(WorldDiagnosticReport.from_wire(item) for item in _sequence("world generation refusal diagnostics", raw.get("diagnostics", [])))
            return cls(raw, False, None, None, _optional_text("world generation world_digest", raw.get("world_digest")), _optional_text("world generation query_digest", raw.get("query_digest")), None, None, None, None, guarantees, stage, _route_text("world generation refusal", raw.get("refusal")), _bool("world generation fail_closed", raw.get("fail_closed")), diagnostics)
        counts = WorldGenerationCountsReport.from_wire(raw.get("counts"))
        validation = WorldValidationReport.from_wire(raw.get("validation"))
        world_raw = raw.get("world")
        query_raw = raw.get("query")
        world = None if world_raw is None else _route_mapping("world generation world", world_raw)
        query = None if query_raw is None else _route_mapping("world generation query", query_raw)
        if validation.errors != 0:
            raise ArgumentError("successful world generation cannot retain validation errors")
        return cls(raw, True, _route_text("world generation world_id", raw.get("world_id")), _route_text("world generation query_id", raw.get("query_id")), _route_text("world generation world_digest", raw.get("world_digest")), _route_text("world generation query_digest", raw.get("query_digest")), counts, validation, world, query, guarantees, None, None, None, ())

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def documents_included(self) -> bool:
        return self.world is not None and self.query is not None

    @property
    def generation_is_deterministic(self) -> bool:
        return any("pure deterministic function" in guarantee for guarantee in self.guarantees)

    @property
    def digests_bind_exact_documents(self) -> bool:
        return any("digests bind the exact generated JSON documents" in guarantee for guarantee in self.guarantees)

    @property
    def side_effect_free(self) -> bool:
        return any("no file, network, model, clinical, or publication side effect" in guarantee for guarantee in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def world_generate_report(value: Mapping[str, Any]) -> WorldGenerateReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return WorldGenerateReport.from_wire(value)


__all__ = [
    "WORLD_GENERATION_MAX_INPUT_BYTES",
    "WORLD_GENERATION_MAX_SUBJECTS",
    "WORLD_GENERATION_MAX_DISTRACTORS",
    "WORLD_GENERATION_MAX_RELAY_DEPTH",
    "WORLD_GENERATION_STAGES",
    "WORLD_GENERATION_SEVERITIES",
    "WorldGenerateArgs",
    "WorldDiagnosticReport",
    "WorldValidationReport",
    "WorldGenerationCountsReport",
    "WorldGenerateReport",
    "world_generate_report",
]
