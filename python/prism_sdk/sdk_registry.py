"""Typed SDK plugin-registry admission projections.

The registry validates every manifest before resolving any capability.  A malformed declaration and
a valid-but-conflicting set are different refusal stages, but neither may return a partial registry.
This module keeps that fail-closed admission boundary typed while leaving manifest semantics and
trust policy authoritative in the Rust SDK crate.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


def _object(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("SDK registry response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and "manifests" in candidate

    if matches(raw):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and matches(structured):
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"SDK registry response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded SDK registry response", decoded)
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError("response does not contain an SDK registry projection")


@dataclass(frozen=True)
class SdkRegistryCheckArgs:
    manifests: tuple[Mapping[str, Any], ...]
    policy: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        raw = _array("SDK registry manifests", self.manifests)
        if not 1 <= len(raw) <= 256:
            raise ArgumentError("SDK registry manifests must contain between 1 and 256 entries")
        manifests = tuple(_object(f"SDK registry manifests[{index}]", item) for index, item in enumerate(raw))
        envelope = {"manifests": manifests, "policy": self.policy}
        if len(json.dumps(envelope, separators=(",", ":"), ensure_ascii=False).encode("utf-8")) > 20_000_000:
            raise ArgumentError("SDK registry input must be at most 20000000 bytes")
        object.__setattr__(self, "manifests", manifests)
        object.__setattr__(self, "policy", None if self.policy is None else _object("SDK registry policy", self.policy))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SdkRegistryCheckArgs":
        raw = _object("SDK registry arguments", value)
        manifests = tuple(_object(f"SDK registry manifests[{index}]", item) for index, item in enumerate(_array("SDK registry manifests", raw.get("manifests"))))
        return cls(manifests, raw.get("policy"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"manifests": [dict(manifest) for manifest in self.manifests]}
        if self.policy is not None:
            result["policy"] = dict(self.policy)
        return result


def _manifest_row(name: str, value: Any) -> dict[str, Any]:
    row = _object(name, value)
    _route_count(f"{name}.index", row.get("index"))
    if row.get("valid") is False:
        _bool(f"{name}.valid", row.get("valid"))
        _route_text(f"{name}.refusal", row.get("refusal"))
        return row
    if row.get("id") is not None:
        _route_text(f"{name}.id", row.get("id"))
    _bool(f"{name}.valid", row.get("valid"))
    _optional_text(f"{name}.validation_error", row.get("validation_error"))
    _optional_text(f"{name}.digest", row.get("digest"))
    _optional_text(f"{name}.core_digest", row.get("core_digest"))
    _route_strings(f"{name}.capability_kinds", row.get("capability_kinds", ()))
    if row.get("trust") is not None:
        _object(f"{name}.trust", row.get("trust"))
    return row


def _registration(name: str, value: Any) -> dict[str, Any]:
    row = _object(name, value)
    _route_text(f"{name}.id", row.get("id"))
    _route_text(f"{name}.digest", row.get("digest"))
    _route_text(f"{name}.core_digest", row.get("core_digest"))
    _object(f"{name}.negotiated", row.get("negotiated"))
    _object(f"{name}.trust", row.get("trust"))
    _bool(f"{name}.load_bearing_selectable", row.get("load_bearing_selectable"))
    return row


@dataclass(frozen=True)
class SdkRegistryCheckReport:
    raw: dict[str, Any]
    ok: bool
    stage: str | None
    refusal: str | None
    fail_closed: bool
    manifests: tuple[dict[str, Any], ...]
    registry: dict[str, Any] | None
    manifest_count: int
    conformance_note: str | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SdkRegistryCheckReport":
        raw = _payload(value)
        ok = _bool("SDK registry ok", raw.get("ok"))
        manifests = tuple(_manifest_row(f"SDK registry manifests[{index}]", item) for index, item in enumerate(_array("SDK registry manifests", raw.get("manifests"))))
        if ok:
            _route_count("SDK registry manifest_count", raw.get("manifest_count"))
            registry = _object("SDK registry registry", raw.get("registry"))
            _route_count("SDK registry registration_count", registry.get("registration_count"))
            resolution = _object("SDK registry resolution", registry.get("resolution"))
            for key, plugin in resolution.items():
                _object(f"SDK registry resolution[{key!r}]", plugin)
            registrations = tuple(_registration(f"SDK registry registrations[{index}]", item) for index, item in enumerate(_array("SDK registry registrations", registry.get("registrations"))))
            _object("SDK registry policy", registry.get("policy"))
            registry = {**registry, "resolution": resolution, "registrations": list(registrations)}
            stage = None
            refusal = None
            fail_closed = False
            conformance_note = _route_text("SDK registry conformance_note", raw.get("conformance_note"))
        else:
            stage = _route_text("SDK registry stage", raw.get("stage"))
            if stage not in {"manifest_validation", "registry_registration"}:
                raise ArgumentError("SDK registry refusal stage is not recognized")
            refusal = _optional_text("SDK registry refusal", raw.get("refusal"))
            if stage == "registry_registration" and refusal is None:
                raise ArgumentError("registry registration refusals must include a refusal")
            fail_closed = _bool("SDK registry fail_closed", raw.get("fail_closed"))
            if not fail_closed:
                raise ArgumentError("SDK registry refusals must be fail-closed")
            if raw.get("registry") is not None:
                raise ArgumentError("refused SDK registry checks must not return a partial registry")
            registry = None
            _route_count("SDK registry manifest_count", len(manifests)) if raw.get("manifest_count") is not None else None
            conformance_note = None
        return cls(raw, ok, stage, refusal, fail_closed, manifests, registry, len(manifests) if not ok else int(raw["manifest_count"]), conformance_note, _route_strings("SDK registry guarantees", raw.get("guarantees", ())))

    @property
    def admitted(self) -> bool:
        return self.ok and self.registry is not None

    @property
    def partial_registry_absent(self) -> bool:
        return self.registry is None


def sdk_registry_check_report(value: Mapping[str, Any]) -> SdkRegistryCheckReport:
    return SdkRegistryCheckReport.from_wire(value)

