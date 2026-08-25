"""Caller-owned protected-value rehydration with tenant and replay fencing.

Autonomous workers frequently need to resume with a value that the SDK must not own: a
provider credential, a private connector payload, a delegated session, or an opaque
institutional record.  This module gives those workflows one narrow boundary:

* durable state contains only an opaque reference, bounded labels, and SHA-256 digests;
* tenant, actor, session, authorization, and domain scope are bound into one context digest;
* the caller supplies the resolver and authorization authority;
* returned values are transient and are never included in ``to_dict`` or snapshots;
* expiry, bounded attempts, one-time consumption, and digest verification fence replay and
  accidental cross-tenant substitution.

The SDK intentionally does not implement a vault, identity provider, or authorization system.
The resolver is the integration point for one supplied by the application or deployment.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import re
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES


AUTONOMOUS_PROTECTED_REHYDRATION_SCHEMA = "bioprism-python-autonomous-protected-rehydration/0.1"
AUTONOMOUS_PROTECTED_REHYDRATION_CONTEXT_SCHEMA = "bioprism-python-autonomous-protected-rehydration-context/0.1"
AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA = "bioprism-python-autonomous-protected-rehydration-reference/0.1"
AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-protected-rehydration-snapshot/0.1"
AUTONOMOUS_PROTECTED_REHYDRATION_ADAPTER_SCHEMA = "bioprism-python-autonomous-protected-rehydration-adapter/0.1"
MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES = 4_096
MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS = 8
MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES = 1_000_000
MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS = 31 * 86_400
AUTONOMOUS_PROTECTED_REHYDRATION_DIGEST_SCHEMES = ("canonical_json", "utf8_sha256")

_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)
_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$")
_STATUSES = ("available", "consumed", "expired", "quarantined")
_RETENTION = "metadata_only_opaque_references_and_digests_no_protected_values"
_SECRET_MATERIAL = "never_returned"
_AUTHORITY = "caller_owned_resolver_and_authorizer_required"


class AutonomousProtectedRehydrationError(ValueError):
    """Raised when a protected rehydration request, replay, or snapshot is unsafe."""


class AutonomousProtectedRehydrationTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class AutonomousProtectedRehydrationTransactionalTextStore(AutonomousProtectedRehydrationTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


def _fail(message: str) -> None:
    raise AutonomousProtectedRehydrationError(f"protected rehydration {message}")


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > 256 or _ID_RE.fullmatch(value) is None:
        _fail(f"{name} is not a bounded identifier")
    return value


def _digest(name: str, value: Any, *, optional: bool = False) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _number(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        _fail(f"{name} is outside its numeric bounds")
    return float(value)


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _boolean(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        _fail(f"{name} must be boolean")
    return value


def _domains(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or not value:
        _fail(f"{name} must contain at least one domain")
    values = tuple(value)
    if any(item not in _DOMAINS for item in values) or len(set(values)) != len(values):
        _fail(f"{name} contains an unsupported or duplicate domain")
    return tuple(domain for domain in _DOMAINS if domain in values)


def protected_value_digest(value: Any) -> str:
    """Return the digest callers must bind before handing a protected value to a resolver.

    The value is inspected only in the current process.  This helper does not retain it and
    the returned digest is the only representation accepted by a durable reference.
    """

    try:
        return content_digest(value)
    except (TypeError, ValueError, OverflowError) as error:
        raise AutonomousProtectedRehydrationError("protected value must be canonical JSON") from error


def _digest_scheme(value: Any) -> str:
    if value not in AUTONOMOUS_PROTECTED_REHYDRATION_DIGEST_SCHEMES:
        _fail("digest scheme is unsupported")
    return value


def _digest_for_scheme(value: Any, scheme: str) -> str:
    if scheme == "canonical_json":
        return protected_value_digest(value)
    if scheme == "utf8_sha256":
        if not isinstance(value, str):
            _fail("utf8_sha256 protected values must be strings")
        return hashlib.sha256(value.encode("utf-8")).hexdigest()
    _fail("digest scheme is unsupported")


@dataclass(frozen=True, slots=True)
class AutonomousProtectedRehydrationContext:
    tenant_id: str
    actor_id: str
    session_id: str
    authorization_digest: str
    allowed_domains: tuple[str, ...] = _DOMAINS

    def __post_init__(self) -> None:
        _identifier("context tenant_id", self.tenant_id)
        _identifier("context actor_id", self.actor_id)
        _identifier("context session_id", self.session_id)
        _digest("context authorization_digest", self.authorization_digest)
        if self.allowed_domains != _domains("context allowed_domains", self.allowed_domains):
            _fail("context domains are not in canonical built-in order")

    def immutable_projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROTECTED_REHYDRATION_CONTEXT_SCHEMA,
            "tenant_id": self.tenant_id,
            "actor_id": self.actor_id,
            "session_id": self.session_id,
            "authorization_digest": self.authorization_digest,
            "allowed_domains": list(self.allowed_domains),
        }

    @property
    def context_digest(self) -> str:
        return content_digest(self.immutable_projection())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self.immutable_projection(),
            "context_digest": self.context_digest,
            "retention": _RETENTION,
            "authority": _AUTHORITY,
            "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousProtectedRehydrationReference:
    reference_id: str
    domain: str
    purpose: str
    value_digest: str
    value_kind: str
    issued_at: float
    expires_at: float
    one_time: bool
    status: str
    attempts: int
    context_digest: str
    reference_digest: str
    last_error_class: str | None = None

    def __post_init__(self) -> None:
        _identifier("reference_id", self.reference_id)
        if self.domain not in _DOMAINS:
            _fail("reference domain is unsupported")
        _identifier("reference purpose", self.purpose)
        _digest("reference value_digest", self.value_digest)
        _identifier("reference value_kind", self.value_kind)
        issued = _number("reference issued_at", self.issued_at, 0.0, 9_223_372_036_854_775.0)
        expires = _number("reference expires_at", self.expires_at, 0.0, 9_223_372_036_854_775.0)
        if expires < issued or expires - issued > MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS:
            _fail("reference expiry is outside its bounded lifetime")
        _boolean("reference one_time", self.one_time)
        if self.status not in _STATUSES:
            _fail("reference status is unsupported")
        _integer("reference attempts", self.attempts, 0, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS)
        _digest("reference context_digest", self.context_digest)
        _digest("reference reference_digest", self.reference_digest)
        if self.last_error_class is not None:
            _identifier("reference last_error_class", self.last_error_class)
        if self.status == "consumed" and not self.one_time:
            _fail("non-one-time reference cannot be consumed")

    def immutable_projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA,
            "reference_id": self.reference_id,
            "domain": self.domain,
            "purpose": self.purpose,
            "value_digest": self.value_digest,
            "value_kind": self.value_kind,
            "issued_at": int(self.issued_at) if self.issued_at.is_integer() else self.issued_at,
            "expires_at": int(self.expires_at) if self.expires_at.is_integer() else self.expires_at,
            "one_time": self.one_time,
            "context_digest": self.context_digest,
        }

    def public_projection(self) -> dict[str, Any]:
        return {
            **self.immutable_projection(),
            "status": self.status,
            "attempts": self.attempts,
            "last_error_class": self.last_error_class,
            "reference_digest": self.reference_digest,
            "retention": _RETENTION,
            "authority": _AUTHORITY,
            "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousProtectedRehydrationResult:
    reference: AutonomousProtectedRehydrationReference
    value: Any
    resolution_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "reference": self.reference.public_projection(),
            "resolution_digest": self.resolution_digest,
            "value_present": True,
            "value_retained": False,
            "retention": "transient_caller_value_only",
            "authority": _AUTHORITY,
            "secret_material": _SECRET_MATERIAL,
        }


class AutonomousProtectedRehydrationAdapter:
    """Adapt metadata-only receipts to a context-bound protected rehydration boundary.

    Existing evidence and connector receipts use different identity fields. This adapter derives
    one bounded reference identity from their non-secret identity metadata, binds the receipt's
    value/payload digest, and delegates resolution to the shared boundary. The receipt itself is
    never retained, and the callback supplied to the boundary receives only the reference
    projection and active context.
    """

    def __init__(self, boundary: "AutonomousProtectedRehydrationBoundary") -> None:
        if not isinstance(boundary, AutonomousProtectedRehydrationBoundary):
            _fail("receipt adapter requires an AutonomousProtectedRehydrationBoundary")
        self.boundary = boundary

    @staticmethod
    def _metadata(receipt: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(receipt, Mapping):
            _fail("receipt must be a metadata mapping")
        allowed = (
            "receipt_digest", "request_digest", "request_id", "dispatch_id", "work_id", "value_digest", "payload_digest",
            "domain", "source_id", "connector_id", "plan_digest", "workflow_digest", "stage_id", "attempt",
            "goal_id", "goal_digest", "task_digest", "schedule_digest", "claim_digest", "revision", "execution_binding_digest",
            "job_id", "index", "mode", "expected_result_digest", "spec_digest", "capability", "approval_released",
            "effect_id", "execution_id", "tool", "call_id", "risk_class", "arguments_digest", "idempotency_key_digest",
            "dispatch_attempt", "provider", "operation",
        )
        return {key: receipt[key] for key in allowed if key in receipt and receipt[key] is not None}

    def _binding(self, receipt: Mapping[str, Any], purpose: str, digest_scheme: str) -> tuple[str, str]:
        metadata = self._metadata(receipt)
        value_digest = metadata.get("value_digest") or metadata.get("payload_digest")
        if not isinstance(value_digest, str):
            _fail("receipt has no protected value or payload digest")
        _digest("receipt protected value digest", value_digest)
        purpose = _identifier("receipt purpose", purpose)
        binding = content_digest({"schema": AUTONOMOUS_PROTECTED_REHYDRATION_ADAPTER_SCHEMA, "purpose": purpose, "digest_scheme": digest_scheme, "receipt": metadata})
        return f"rehydrate-{binding[:48]}", value_digest

    def resolve_receipt(
        self,
        receipt: Mapping[str, Any],
        *,
        domain: str | None = None,
        purpose: str = "protected_receipt_value",
        value_kind: str = "opaque",
        one_time: bool = False,
        now: float | None = None,
        digest_scheme: str = "canonical_json",
    ) -> Any:
        metadata = self._metadata(receipt)
        resolved_domain = metadata.get("domain") if domain is None else domain
        if resolved_domain not in self.boundary.context.allowed_domains:
            _fail("receipt domain is outside the active context scope")
        digest_scheme = _digest_scheme(digest_scheme)
        reference_id, value_digest = self._binding(metadata, purpose, digest_scheme)
        self.boundary.issue(
            reference_id,
            domain=resolved_domain,
            purpose=purpose,
            value_digest=value_digest,
            value_kind=value_kind,
            one_time=one_time,
        )
        return self.boundary.resolve(reference_id, now=now, value_digestor=lambda value: _digest_for_scheme(value, digest_scheme)).value

    def resolver(
        self,
        *,
        domain: str | None = None,
        purpose: str = "protected_receipt_value",
        value_kind: str = "opaque",
        one_time: bool = False,
        digest_scheme: str = "canonical_json",
    ) -> Callable[[Mapping[str, Any]], Any]:
        return lambda receipt: self.resolve_receipt(receipt, domain=domain, purpose=purpose, value_kind=value_kind, one_time=one_time, digest_scheme=digest_scheme)


Resolver = Callable[[AutonomousProtectedRehydrationReference, AutonomousProtectedRehydrationContext], Any]
Authorizer = Callable[[AutonomousProtectedRehydrationReference, AutonomousProtectedRehydrationContext], bool]


def _reference_from_snapshot(value: Mapping[str, Any]) -> AutonomousProtectedRehydrationReference:
    expected = {
        "schema", "reference_id", "domain", "purpose", "value_digest", "value_kind", "issued_at", "expires_at",
        "one_time", "status", "attempts", "context_digest", "reference_digest", "last_error_class", "retention", "authority", "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != expected or value.get("schema") != AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA or value.get("retention") != _RETENTION or value.get("authority") != _AUTHORITY or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("snapshot reference is malformed")
    reference = AutonomousProtectedRehydrationReference(
        reference_id=_identifier("snapshot reference_id", value.get("reference_id")),
        domain=value.get("domain"), purpose=_identifier("snapshot purpose", value.get("purpose")),
        value_digest=_digest("snapshot value_digest", value.get("value_digest")) or "",
        value_kind=_identifier("snapshot value_kind", value.get("value_kind")),
        issued_at=_number("snapshot issued_at", value.get("issued_at"), 0.0, 9_223_372_036_854_775.0),
        expires_at=_number("snapshot expires_at", value.get("expires_at"), 0.0, 9_223_372_036_854_775.0),
        one_time=_boolean("snapshot one_time", value.get("one_time")), status=value.get("status"),
        attempts=_integer("snapshot attempts", value.get("attempts"), 0, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS),
        context_digest=_digest("snapshot context_digest", value.get("context_digest")) or "",
        reference_digest=_digest("snapshot reference_digest", value.get("reference_digest")) or "",
        last_error_class=None if value.get("last_error_class") is None else _identifier("snapshot last_error_class", value.get("last_error_class")),
    )
    if content_digest(reference.immutable_projection()) != reference.reference_digest:
        _fail("snapshot reference digest does not match its immutable projection")
    return reference


def _coverage(references: Sequence[AutonomousProtectedRehydrationReference]) -> list[dict[str, Any]]:
    return [
        {
            "domain": domain,
            "reference_count": sum(1 for reference in references if reference.domain == domain),
            "available_count": sum(1 for reference in references if reference.domain == domain and reference.status == "available"),
            "consumed_count": sum(1 for reference in references if reference.domain == domain and reference.status == "consumed"),
            "expired_count": sum(1 for reference in references if reference.domain == domain and reference.status == "expired"),
            "quarantined_count": sum(1 for reference in references if reference.domain == domain and reference.status == "quarantined"),
        }
        for domain in _DOMAINS
    ]


def validate_autonomous_protected_rehydration_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    expected = {"schema", "generation", "previous_snapshot_digest", "context_digest", "policy", "references", "coverage", "retention", "authority", "secret_material", "snapshot_digest"}
    if not isinstance(value, Mapping) or set(value) != expected or value.get("schema") != AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA or value.get("retention") != _RETENTION or value.get("authority") != _AUTHORITY or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("snapshot is malformed")
    _integer("snapshot generation", value.get("generation"), 1, 2_147_483_647)
    _digest("snapshot previous_snapshot_digest", value.get("previous_snapshot_digest"), optional=True)
    context_digest = _digest("snapshot context_digest", value.get("context_digest"))
    policy = value.get("policy")
    if not isinstance(policy, Mapping) or set(policy) != {"max_references", "max_attempts", "max_ttl_seconds"}:
        _fail("snapshot policy is malformed")
    max_references = _integer("snapshot policy max_references", policy.get("max_references"), 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES)
    _integer("snapshot policy max_attempts", policy.get("max_attempts"), 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS)
    _number("snapshot policy max_ttl_seconds", policy.get("max_ttl_seconds"), 1.0, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS)
    raw_references = value.get("references")
    if not isinstance(raw_references, Sequence) or isinstance(raw_references, (str, bytes, bytearray)) or len(raw_references) > max_references:
        _fail("snapshot references are malformed")
    references = [_reference_from_snapshot(item) for item in raw_references]
    if len({reference.reference_id for reference in references}) != len(references) or any(reference.context_digest != context_digest for reference in references):
        _fail("snapshot references are not bound to its context")
    coverage = value.get("coverage")
    if not isinstance(coverage, Sequence) or isinstance(coverage, (str, bytes, bytearray)) or list(coverage) != _coverage(references):
        _fail("snapshot coverage does not match references")
    _digest("snapshot snapshot_digest", value.get("snapshot_digest"))
    descriptor = {
        "schema": value["schema"], "generation": value["generation"], "previous_snapshot_digest": value["previous_snapshot_digest"],
        "context_digest": context_digest, "policy": dict(policy), "references": [reference.public_projection() for reference in sorted(references, key=lambda item: item.reference_id)],
        "coverage": list(coverage), "retention": value["retention"], "authority": value["authority"], "secret_material": value["secret_material"],
    }
    if content_digest(descriptor) != value["snapshot_digest"]:
        _fail("snapshot digest does not match its canonical projection")
    return json.loads(canonical_json(value))


class AutonomousProtectedRehydrationBoundary:
    """Bounded, digest-verified bridge from durable references to caller-owned values."""

    def __init__(
        self,
        context: AutonomousProtectedRehydrationContext,
        resolver: Resolver,
        *,
        authorizer: Authorizer | None = None,
        max_references: int = MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES,
        max_attempts: int = 3,
        max_ttl_seconds: float = 3_600.0,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(context, AutonomousProtectedRehydrationContext):
            _fail("context is malformed")
        if not callable(resolver):
            _fail("resolver is required")
        if authorizer is not None and not callable(authorizer):
            _fail("authorizer is malformed")
        if not callable(clock):
            _fail("clock is malformed")
        self.context = context
        self.resolver = resolver
        self.authorizer = authorizer
        self.max_references = _integer("max_references", max_references, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES)
        self.max_attempts = _integer("max_attempts", max_attempts, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS)
        self.max_ttl_seconds = _number("max_ttl_seconds", max_ttl_seconds, 1.0, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS)
        self.clock = clock
        self._references: dict[str, AutonomousProtectedRehydrationReference] = {}
        self._generation = 0
        self._previous_snapshot_digest: str | None = None

    @property
    def policy(self) -> dict[str, Any]:
        return {"max_references": self.max_references, "max_attempts": self.max_attempts, "max_ttl_seconds": int(self.max_ttl_seconds) if self.max_ttl_seconds.is_integer() else self.max_ttl_seconds}

    def issue(
        self,
        reference_id: str,
        *,
        domain: str,
        purpose: str,
        value_digest: str,
        value_kind: str = "opaque",
        issued_at: float | None = None,
        expires_at: float | None = None,
        one_time: bool = True,
    ) -> dict[str, Any]:
        reference_id = _identifier("reference_id", reference_id)
        if domain not in self.context.allowed_domains:
            _fail("reference domain is outside the context scope")
        purpose = _identifier("purpose", purpose)
        value_digest = _digest("value_digest", value_digest) or ""
        value_kind = _identifier("value_kind", value_kind)
        issued = _number("issued_at", self.clock() if issued_at is None else issued_at, 0.0, 9_223_372_036_854_775.0)
        expires = _number("expires_at", issued + self.max_ttl_seconds if expires_at is None else expires_at, 0.0, 9_223_372_036_854_775.0)
        if expires < issued or expires - issued > self.max_ttl_seconds:
            _fail("expiry exceeds the configured lifetime")
        one_time = _boolean("one_time", one_time)
        candidate = AutonomousProtectedRehydrationReference(
            reference_id, domain, purpose, value_digest, value_kind, issued, expires, one_time, "available", 0, self.context.context_digest, "0" * 64, None,
        )
        reference = AutonomousProtectedRehydrationReference(
            **{**{field: getattr(candidate, field) for field in candidate.__dataclass_fields__}, "reference_digest": content_digest(candidate.immutable_projection())}
        )
        existing = self._references.get(reference_id)
        if existing is not None:
            if existing.reference_digest != reference.reference_digest:
                _fail("reference identifier already exists with a different immutable payload")
            return existing.public_projection()
        if len(self._references) >= self.max_references:
            _fail("reference registry is full")
        self._references[reference_id] = reference
        return reference.public_projection()

    def issue_for_value(self, reference_id: str, value: Any, **kwargs: Any) -> dict[str, Any]:
        """Bind a transient value's digest; the value is never retained by this boundary."""

        return self.issue(reference_id, value_digest=protected_value_digest(value), **kwargs)

    def get(self, reference_id: str) -> dict[str, Any] | None:
        reference_id = _identifier("reference_id", reference_id)
        reference = self._references.get(reference_id)
        return None if reference is None else reference.public_projection()

    def list_references(self, *, limit: int = 128) -> list[dict[str, Any]]:
        limit = _integer("list limit", limit, 1, self.max_references)
        return [reference.public_projection() for reference in sorted(self._references.values(), key=lambda item: (item.status != "available", item.expires_at, item.reference_id))[:limit]]

    def _replace(self, reference: AutonomousProtectedRehydrationReference, **changes: Any) -> AutonomousProtectedRehydrationReference:
        values = {field: getattr(reference, field) for field in reference.__dataclass_fields__}
        values.update(changes)
        return AutonomousProtectedRehydrationReference(**values)

    def _failure(self, reference: AutonomousProtectedRehydrationReference, error_class: str) -> None:
        attempts = min(self.max_attempts, reference.attempts + 1)
        status = "quarantined" if attempts >= self.max_attempts else reference.status
        self._references[reference.reference_id] = self._replace(reference, attempts=attempts, status=status, last_error_class=_identifier("error_class", error_class))

    def resolve(
        self,
        reference_id: str,
        *,
        now: float | None = None,
        value_digestor: Callable[[Any], str] | None = None,
    ) -> AutonomousProtectedRehydrationResult:
        current = _number("resolve now", self.clock() if now is None else now, 0.0, 9_223_372_036_854_775.0)
        reference_id = _identifier("reference_id", reference_id)
        reference = self._references.get(reference_id)
        if reference is None:
            _fail("reference does not exist")
        if reference.context_digest != self.context.context_digest:
            _fail("reference context does not match the active tenant and authorization")
        if reference.status == "consumed":
            _fail("one-time reference has already been consumed")
        if reference.status == "quarantined":
            _fail("reference is quarantined")
        if current >= reference.expires_at:
            self._references[reference_id] = self._replace(reference, status="expired", last_error_class="reference_expired")
            _fail("reference has expired")
        if self.authorizer is not None:
            try:
                allowed = self.authorizer(reference, self.context)
            except Exception as error:
                self._failure(reference, "authorization_check_failure")
                raise AutonomousProtectedRehydrationError("protected rehydration authorization check failed") from error
            if allowed is not True:
                self._failure(reference, "authorization_denied")
                _fail("caller authorization was denied")
        try:
            value = self.resolver(reference, self.context)
            if value_digestor is not None and not callable(value_digestor):
                _fail("value digestor is malformed")
            observed_digest = protected_value_digest(value) if value_digestor is None else value_digestor(value)
        except Exception as error:
            self._failure(reference, "resolver_failure")
            raise AutonomousProtectedRehydrationError("protected value resolver failed") from error
        if observed_digest != reference.value_digest:
            self._failure(reference, "value_digest_mismatch")
            _fail("resolver returned a value with a different digest")
        updated = self._replace(reference, status="consumed" if reference.one_time else "available", attempts=reference.attempts + 1, last_error_class=None)
        self._references[reference_id] = updated
        resolution_digest = content_digest({"schema": AUTONOMOUS_PROTECTED_REHYDRATION_SCHEMA, "reference_digest": reference.reference_digest, "context_digest": self.context.context_digest, "attempt": updated.attempts})
        return AutonomousProtectedRehydrationResult(updated, value, resolution_digest)

    def quarantine(self, reference_id: str, error_class: str = "caller_quarantined") -> dict[str, Any]:
        reference = self._references.get(_identifier("reference_id", reference_id))
        if reference is None:
            _fail("reference does not exist")
        self._references[reference.reference_id] = self._replace(reference, status="quarantined", last_error_class=_identifier("error_class", error_class))
        return self._references[reference.reference_id].public_projection()

    def snapshot(self) -> dict[str, Any]:
        self._generation += 1
        descriptor = {
            "schema": AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA,
            "generation": self._generation,
            "previous_snapshot_digest": self._previous_snapshot_digest,
            "context_digest": self.context.context_digest,
            "policy": self.policy,
            "references": [reference.public_projection() for reference in sorted(self._references.values(), key=lambda item: item.reference_id)],
            "coverage": _coverage(list(self._references.values())),
            "retention": _RETENTION,
            "authority": _AUTHORITY,
            "secret_material": _SECRET_MATERIAL,
        }
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        self._previous_snapshot_digest = snapshot["snapshot_digest"]
        return json.loads(canonical_json(snapshot))

    def restore(self, snapshot: Mapping[str, Any]) -> dict[str, Any]:
        validated = validate_autonomous_protected_rehydration_snapshot(snapshot)
        if validated["context_digest"] != self.context.context_digest:
            _fail("restored snapshot belongs to a different tenant, actor, session, or authorization")
        if validated["policy"] != self.policy:
            _fail("restored policy conflicts with the configured boundary")
        references = [_reference_from_snapshot(item) for item in validated["references"]]
        self._references = {reference.reference_id: reference for reference in references}
        self._generation = validated["generation"]
        self._previous_snapshot_digest = validated["snapshot_digest"]
        return json.loads(canonical_json(validated))


class JsonAutonomousProtectedRehydrationPersistence:
    def __init__(self, text_store: AutonomousProtectedRehydrationTextStore, *, max_bytes: int = MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(text_store, "read", None)) or not callable(getattr(text_store, "write", None)):
            _fail("JSON text store is malformed")
        self.text_store = text_store
        self.max_bytes = _integer("JSON max_bytes", max_bytes, 1, MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.text_store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        try:
            parsed = json.loads(encoded)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise AutonomousProtectedRehydrationError("protected rehydration JSON is invalid") from error
        if canonical_json(parsed) != encoded:
            _fail("JSON snapshot is not canonical")
        return validate_autonomous_protected_rehydration_snapshot(parsed)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        encoded = canonical_json(validate_autonomous_protected_rehydration_snapshot(snapshot))
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        self.text_store.write(encoded)


class TransactionalJsonAutonomousProtectedRehydrationPersistence(JsonAutonomousProtectedRehydrationPersistence):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected_snapshot_digest", expected_snapshot_digest, optional=True)
        if not callable(getattr(self.text_store, "write_if_unchanged", None)):
            _fail("transactional JSON text store lacks compare-and-swap")
        encoded = canonical_json(validate_autonomous_protected_rehydration_snapshot(snapshot))
        return bool(self.text_store.write_if_unchanged(expected_snapshot_digest, encoded))


class AutonomousProtectedRehydrationPersistenceCoordinator:
    def __init__(self, boundary: AutonomousProtectedRehydrationBoundary, persistence: JsonAutonomousProtectedRehydrationPersistence) -> None:
        if not isinstance(boundary, AutonomousProtectedRehydrationBoundary) or not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("persistence coordinator inputs are malformed")
        self.boundary = boundary
        self.persistence = persistence
        self.expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            return None
        self.boundary.restore(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.boundary.snapshot()
        if isinstance(self.persistence, TransactionalJsonAutonomousProtectedRehydrationPersistence):
            if not self.persistence.write_if_unchanged(self.expected_snapshot_digest, snapshot):
                _fail("persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


__all__ = [
    "AUTONOMOUS_PROTECTED_REHYDRATION_SCHEMA", "AUTONOMOUS_PROTECTED_REHYDRATION_CONTEXT_SCHEMA", "AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCE_SCHEMA", "AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_SCHEMA", "AUTONOMOUS_PROTECTED_REHYDRATION_ADAPTER_SCHEMA",
    "MAX_AUTONOMOUS_PROTECTED_REHYDRATION_REFERENCES", "MAX_AUTONOMOUS_PROTECTED_REHYDRATION_ATTEMPTS", "MAX_AUTONOMOUS_PROTECTED_REHYDRATION_SNAPSHOT_BYTES", "MAX_AUTONOMOUS_PROTECTED_REHYDRATION_TTL_SECONDS",
    "AutonomousProtectedRehydrationError", "AutonomousProtectedRehydrationTextStore", "AutonomousProtectedRehydrationTransactionalTextStore", "AutonomousProtectedRehydrationContext", "AutonomousProtectedRehydrationReference", "AutonomousProtectedRehydrationResult", "AutonomousProtectedRehydrationAdapter", "AutonomousProtectedRehydrationBoundary", "JsonAutonomousProtectedRehydrationPersistence", "TransactionalJsonAutonomousProtectedRehydrationPersistence", "AutonomousProtectedRehydrationPersistenceCoordinator", "AUTONOMOUS_PROTECTED_REHYDRATION_DIGEST_SCHEMES", "protected_value_digest", "validate_autonomous_protected_rehydration_snapshot",
]
