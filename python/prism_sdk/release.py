"""Typed projections for the noncompensatory release-audit composition.

The Rust aggregator deliberately keeps three different facts visible:

* a delegated check can be evaluated and pass or fail;
* a delegated check can refuse to run, which is an invocation failure;
* an advisory observation can be useful evidence without becoming a release gate.

This module preserves those distinctions at the SDK boundary and rechecks the
aggregator's conjunction rather than trusting a single top-level boolean.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


RELEASE_AUDIT_MAX_CHECKS = 32
RELEASE_AUDIT_MAX_INPUT_BYTES = 20_000_000
RELEASE_CHECK_KINDS = (
    "registry_gate",
    "bundle_verify",
    "conformance_run",
    "research_ci_check",
    "quality_gate_run",
    "ops_acceptance",
    "pack_health_assess",
    "repository_impact",
    "developer_platform_status",
)
RELEASE_ADVISORY_ONLY_KINDS = frozenset({"repository_impact", "developer_platform_status"})
BUNDLE_VERIFY_MAX_INPUT_BYTES = 20_000_000


def _optional_bool(name: str, value: Any) -> bool | None:
    if value is None:
        return None
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _bool(name: str, value: Any) -> bool:
    parsed = _optional_bool(name, value)
    if parsed is None:
        raise ArgumentError(f"{name} is required")
    return parsed


def _optional_text(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _route_text(name, value)


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    if value is None:
        return None
    return _route_mapping(name, value)


@dataclass(frozen=True)
class BundleVerifyArgs:
    """Bounded request for keyless or Ed25519 result-bundle verification."""

    bundle: Mapping[str, Any] | None = None
    document: str | None = None
    publicly_attested_bundle: Mapping[str, Any] | None = None
    verification_key: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        sources = sum(value is not None for value in (self.bundle, self.document, self.publicly_attested_bundle))
        if sources != 1:
            raise ArgumentError("bundle verification requires exactly one of bundle, document, or publicly_attested_bundle")
        if self.bundle is not None and not isinstance(self.bundle, Mapping):
            raise ArgumentError("bundle must be an object")
        if self.publicly_attested_bundle is not None and not isinstance(self.publicly_attested_bundle, Mapping):
            raise ArgumentError("publicly_attested_bundle must be an object")
        if self.document is not None and (not isinstance(self.document, str) or not self.document.strip()):
            raise ArgumentError("document must be a non-empty path")
        if self.publicly_attested_bundle is not None and self.verification_key is None:
            raise ArgumentError("verification_key is required for publicly_attested_bundle")
        if self.verification_key is not None:
            key = _route_mapping("verification_key", self.verification_key)
            identity = _route_text("verification_key.key_identity", key.get("key_identity"))
            public_key = _route_text("verification_key.public_key", key.get("public_key"))
            if not public_key.startswith("ed25519:") or len(public_key) != len("ed25519:") + 64 or any(char not in "0123456789abcdef" for char in public_key[len("ed25519:") :]):
                raise ArgumentError("verification_key.public_key must be ed25519:<64 lowercase hex characters>")
            validity = _route_mapping("verification_key.validity", key.get("validity"))
            for field_name in ("not_before", "not_after"):
                value = validity.get(field_name)
                if value is not None and (isinstance(value, bool) or not isinstance(value, int) or value < 0):
                    raise ArgumentError(f"verification_key.validity.{field_name} must be a non-negative integer")
            if not identity:
                raise ArgumentError("verification_key.key_identity must be non-empty")
        encoded = json.dumps(self.to_mcp_arguments(), separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        if len(encoded) > BUNDLE_VERIFY_MAX_INPUT_BYTES:
            raise ArgumentError(f"bundle verification input exceeds the {BUNDLE_VERIFY_MAX_INPUT_BYTES}-byte safety bound")

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        if self.bundle is not None:
            result["bundle"] = dict(self.bundle)
        if self.document is not None:
            result["document"] = self.document
        if self.publicly_attested_bundle is not None:
            result["publicly_attested_bundle"] = dict(self.publicly_attested_bundle)
        if self.verification_key is not None:
            result["verification_key"] = dict(self.verification_key)
        return result


@dataclass(frozen=True)
class BundleVerifyReport:
    """Typed success/refusal projection for result-bundle verification."""

    raw: dict[str, Any]
    ok: bool
    verification_mode: str | None
    manifest_digest: str | None
    authentication: dict[str, Any] | None
    refusal: str | None
    fail_closed: bool | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BundleVerifyReport":
        raw = _bundle_payload(value)
        ok = _bool("bundle verification ok", raw.get("ok"))
        mode = _optional_text("bundle verification mode", raw.get("verification_mode"))
        digest = _optional_text("bundle verification manifest_digest", raw.get("manifest_digest"))
        authentication = _optional_mapping("bundle verification authentication", raw.get("authentication"))
        refusal = _optional_text("bundle verification refusal", raw.get("refusal"))
        fail_closed = _optional_bool("bundle verification fail_closed", raw.get("fail_closed"))
        if ok:
            if refusal is not None or fail_closed is not None:
                raise ArgumentError("successful bundle verification cannot contain refusal fields")
            if digest is None:
                raise ArgumentError("successful bundle verification must contain manifest_digest")
            if mode is not None and mode != "ed25519_public_key":
                raise ArgumentError("bundle verification mode is unknown")
        else:
            if refusal is None or fail_closed is not True:
                raise ArgumentError("failed bundle verification must preserve a fail-closed refusal")
        return cls(
            raw=raw,
            ok=ok,
            verification_mode=mode,
            manifest_digest=digest,
            authentication=authentication,
            refusal=refusal,
            fail_closed=fail_closed,
            guarantees=_route_strings("bundle verification guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("bundle verification limitations", raw.get("limitations", [])),
        )

    @property
    def is_public_key_verified(self) -> bool:
        return self.ok and self.verification_mode == "ed25519_public_key"


@dataclass(frozen=True)
class ReleaseAuditCheckRequest:
    """One exact delegated MCP request in a release-audit composition.

    ``required=None`` intentionally omits the field so the Rust aggregator can
    apply its kind-specific default.  This matters for advisory-only checks,
    whose default is non-required and whose gate is never promotable.
    """

    kind: str
    arguments: Mapping[str, Any] = field(default_factory=dict)
    required: bool | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.kind, str) or not self.kind:
            raise ArgumentError("release check kind must be a non-empty string")
        if self.kind not in RELEASE_CHECK_KINDS:
            allowed = ", ".join(RELEASE_CHECK_KINDS)
            raise ArgumentError(f"unknown release check kind {self.kind!r}; choose {allowed}")
        if not isinstance(self.arguments, Mapping):
            raise ArgumentError("release check arguments must be an object")
        if self.required is not None and not isinstance(self.required, bool):
            raise ArgumentError("release check required must be a boolean when supplied")
        if self.kind in RELEASE_ADVISORY_ONLY_KINDS and self.required is True:
            raise ArgumentError(f"{self.kind} is advisory-only and cannot be marked required")

    def to_mcp_arguments(self) -> dict[str, Any]:
        request: dict[str, Any] = {"kind": self.kind, "arguments": dict(self.arguments)}
        if self.required is not None:
            request["required"] = self.required
        return request


@dataclass(frozen=True)
class ReleaseAuditArgs:
    """Bounded request envelope for :meth:`Workspace.release_audit`."""

    checks: Sequence[ReleaseAuditCheckRequest | Mapping[str, Any]]
    include_details: bool = False

    def __post_init__(self) -> None:
        if isinstance(self.checks, (str, bytes)) or not isinstance(self.checks, Sequence):
            raise ArgumentError("checks must be an array of release check requests")
        if not 1 <= len(self.checks) <= RELEASE_AUDIT_MAX_CHECKS:
            raise ArgumentError(
                f"checks must contain between 1 and {RELEASE_AUDIT_MAX_CHECKS} release check requests"
            )
        normalized: list[ReleaseAuditCheckRequest] = []
        for index, check in enumerate(self.checks):
            if isinstance(check, ReleaseAuditCheckRequest):
                normalized.append(check)
                continue
            if not isinstance(check, Mapping):
                raise ArgumentError(f"checks[{index}] must be a ReleaseAuditCheckRequest or object")
            kind = check.get("kind")
            arguments = check.get("arguments", {})
            required = check.get("required")
            normalized.append(ReleaseAuditCheckRequest(kind, arguments, required))
        if not isinstance(self.include_details, bool):
            raise ArgumentError("include_details must be a boolean")
        object.__setattr__(self, "checks", tuple(normalized))

        encoded = json.dumps(self.to_mcp_arguments(), separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        if len(encoded) > RELEASE_AUDIT_MAX_INPUT_BYTES:
            raise ArgumentError(
                f"release-audit input exceeds the {RELEASE_AUDIT_MAX_INPUT_BYTES}-byte safety bound"
            )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "checks": [check.to_mcp_arguments() for check in self.checks],
            "include_details": self.include_details,
        }


@dataclass(frozen=True)
class ReleaseAuditCheckReport:
    """The bounded projection of one delegated check."""

    raw: dict[str, Any]
    index: int
    kind: str
    required: bool
    advisory: bool
    evaluated: bool
    gate: bool | None
    passed: bool
    result_digest: str | None
    result_ok: bool | None
    refusal: str | None
    fail_closed: bool | None
    result: dict[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ReleaseAuditCheckReport":
        raw = _route_mapping("release audit check", value)
        index = _route_count("release audit check index", raw.get("index"))
        kind = _route_text("release audit check kind", raw.get("kind"))
        if kind not in RELEASE_CHECK_KINDS:
            raise ArgumentError(f"unknown release audit check kind: {kind!r}")
        required = _bool("release audit check required", raw.get("required"))
        advisory = _bool("release audit check advisory", raw.get("advisory"))
        if advisory != (not required):
            raise ArgumentError("release audit check advisory flag does not reconcile with required")
        if kind in RELEASE_ADVISORY_ONLY_KINDS and required:
            raise ArgumentError(f"{kind} cannot be represented as a required release check")
        evaluated = _bool("release audit check evaluated", raw.get("evaluated"))
        gate = _optional_bool("release audit check gate", raw.get("gate"))
        passed = _bool("release audit check passed", raw.get("passed"))
        result_digest = _optional_text("release audit result_digest", raw.get("result_digest"))
        result_ok = _optional_bool("release audit result_ok", raw.get("result_ok"))
        refusal = _optional_text("release audit refusal", raw.get("refusal"))
        fail_closed = _optional_bool("release audit fail_closed", raw.get("fail_closed"))
        result = _optional_mapping("release audit result", raw.get("result"))

        if evaluated:
            if refusal is not None:
                raise ArgumentError("evaluated release check cannot contain a refusal")
            if result_digest is None:
                raise ArgumentError("evaluated release check must contain a result digest")
            if passed != (gate is True):
                raise ArgumentError("release audit passed flag does not reconcile with its gate")
        else:
            if gate is not None or passed:
                raise ArgumentError("unevaluated release check must have null gate and passed=false")
            if refusal is None:
                raise ArgumentError("unevaluated release check must contain a refusal")
            if fail_closed is not True:
                raise ArgumentError("unevaluated release check must be fail-closed")
            if result_digest is not None or result_ok is not None or result is not None:
                raise ArgumentError("unevaluated release check cannot contain a delegated result")
        if kind in RELEASE_ADVISORY_ONLY_KINDS and gate is not None:
            raise ArgumentError(f"advisory-only release check {kind} must have a null gate")
        return cls(raw, index, kind, required, advisory, evaluated, gate, passed, result_digest, result_ok, refusal, fail_closed, result)


@dataclass(frozen=True)
class ReleaseAuditBlockerReport:
    """A required-check blocker retained by the aggregator."""

    raw: dict[str, Any]
    index: int
    kind: str
    reason: str
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ReleaseAuditBlockerReport":
        raw = _route_mapping("release audit blocker", value)
        kind = _route_text("release audit blocker kind", raw.get("kind"))
        if kind not in RELEASE_CHECK_KINDS:
            raise ArgumentError(f"unknown release audit blocker kind: {kind!r}")
        return cls(
            raw=raw,
            index=_route_count("release audit blocker index", raw.get("index")),
            kind=kind,
            reason=_route_text("release audit blocker reason", raw.get("reason")),
            fail_closed=_bool("release audit blocker fail_closed", raw.get("fail_closed")),
        )


@dataclass(frozen=True)
class ReleaseAuditReport:
    """Validated release-audit evidence and derived gate helpers."""

    raw: dict[str, Any]
    ok: bool
    release_ready: bool
    required_check_count: int
    check_count: int
    invocation_failures: int
    blocking_count: int
    blockers: tuple[ReleaseAuditBlockerReport, ...]
    checks: tuple[ReleaseAuditCheckReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ReleaseAuditReport":
        raw = _payload(value)
        ok = _bool("release audit ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("release audit report is not successful")
        raw_checks = raw.get("checks")
        if not isinstance(raw_checks, Sequence) or isinstance(raw_checks, (str, bytes)):
            raise ArgumentError("release audit checks must be an array")
        if not 1 <= len(raw_checks) <= RELEASE_AUDIT_MAX_CHECKS:
            raise ArgumentError("release audit checks must contain between 1 and 32 rows")
        checks = tuple(ReleaseAuditCheckReport.from_wire(item) for item in raw_checks)
        if tuple(check.index for check in checks) != tuple(range(len(checks))):
            raise ArgumentError("release audit check indexes must be contiguous and ordered")

        raw_blockers = raw.get("blockers")
        if not isinstance(raw_blockers, Sequence) or isinstance(raw_blockers, (str, bytes)):
            raise ArgumentError("release audit blockers must be an array")
        blockers = tuple(ReleaseAuditBlockerReport.from_wire(item) for item in raw_blockers)
        by_index = {check.index: check for check in checks}
        for blocker in blockers:
            check = by_index.get(blocker.index)
            if check is None or check.kind != blocker.kind or not check.required or check.passed:
                raise ArgumentError("release audit blocker does not reference a failed required check")

        required_count = _route_count("release audit required_check_count", raw.get("required_check_count"))
        check_count = _route_count("release audit check_count", raw.get("check_count"))
        invocation_failures = _route_count("release audit invocation_failures", raw.get("invocation_failures"))
        blocking_count = _route_count("release audit blocking_count", raw.get("blocking_count"))
        if check_count != len(checks):
            raise ArgumentError("release audit check_count does not reconcile with checks")
        if required_count != sum(check.required for check in checks):
            raise ArgumentError("release audit required_check_count does not reconcile with checks")
        if blocking_count != len(blockers):
            raise ArgumentError("release audit blocking_count does not reconcile with blockers")
        if invocation_failures > check_count:
            raise ArgumentError("release audit invocation_failures exceeds check_count")

        release_ready = _bool("release audit release_ready", raw.get("release_ready"))
        expected_ready = (
            invocation_failures == 0
            and required_count > 0
            and not blockers
            and all(check.passed for check in checks if check.required)
        )
        if release_ready != expected_ready:
            raise ArgumentError("release audit release_ready does not reconcile with its required gates")
        return cls(
            raw=raw,
            ok=ok,
            release_ready=release_ready,
            required_check_count=required_count,
            check_count=check_count,
            invocation_failures=invocation_failures,
            blocking_count=blocking_count,
            blockers=blockers,
            checks=checks,
            guarantees=_route_strings("release audit guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("release audit limitations", raw.get("limitations", [])),
        )

    @property
    def required_checks(self) -> tuple[ReleaseAuditCheckReport, ...]:
        return tuple(check for check in self.checks if check.required)

    @property
    def advisory_checks(self) -> tuple[ReleaseAuditCheckReport, ...]:
        return tuple(check for check in self.checks if check.advisory)

    @property
    def failed_checks(self) -> tuple[ReleaseAuditCheckReport, ...]:
        return tuple(check for check in self.checks if not check.passed)

    @property
    def refused_checks(self) -> tuple[ReleaseAuditCheckReport, ...]:
        return tuple(check for check in self.checks if not check.evaluated)

    @property
    def details_included(self) -> bool:
        return any(check.result is not None for check in self.checks)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("release audit response", value)
    if "checks" in raw and "release_ready" in raw:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping) and "checks" in structured and "release_ready" in structured:
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"release audit response text is not JSON: {error}") from error
                    decoded_mapping = _route_mapping("decoded release audit response", decoded)
                    if "checks" in decoded_mapping and "release_ready" in decoded_mapping:
                        return decoded_mapping
    raise ArgumentError("response does not contain a release-audit projection")


def _bundle_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bundle verification response", value)
    if "ok" in raw and ("manifest_digest" in raw or "refusal" in raw):
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping) and "ok" in structured:
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"bundle verification response text is not JSON: {error}") from error
                    decoded_mapping = _route_mapping("decoded bundle verification response", decoded)
                    if "ok" in decoded_mapping:
                        return decoded_mapping
    raise ArgumentError("response does not contain a bundle-verification projection")


def release_audit_report(value: Mapping[str, Any]) -> ReleaseAuditReport:
    """Parse direct MCP or HTTP release-audit output."""

    return ReleaseAuditReport.from_wire(value)


def bundle_verify_report(value: Mapping[str, Any]) -> BundleVerifyReport:
    """Parse direct MCP or HTTP result-bundle verification output."""

    return BundleVerifyReport.from_wire(value)


__all__ = [
    "RELEASE_ADVISORY_ONLY_KINDS",
    "RELEASE_AUDIT_MAX_CHECKS",
    "RELEASE_AUDIT_MAX_INPUT_BYTES",
    "RELEASE_CHECK_KINDS",
    "BUNDLE_VERIFY_MAX_INPUT_BYTES",
    "BundleVerifyArgs",
    "BundleVerifyReport",
    "bundle_verify_report",
    "ReleaseAuditArgs",
    "ReleaseAuditBlockerReport",
    "ReleaseAuditCheckReport",
    "ReleaseAuditCheckRequest",
    "ReleaseAuditReport",
    "release_audit_report",
]
