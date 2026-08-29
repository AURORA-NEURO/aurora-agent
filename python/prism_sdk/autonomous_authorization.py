"""Tenant-scoped, metadata-only authorization for autonomous execution.

The autonomous runtime deliberately does not mint identities, validate bearer tokens, or
pretend that a digest is proof of authority.  It does need one common contract, however, so a
deployment can bind the same caller-issued grant to planning, provider invocation, evidence,
connectors, tools, learning, memory, and effects across every built-in domain.

This module is that contract.  A grant is an externally issued, digest-bound scope.  Requests
contain only bounded identity and operation metadata; task text, prompts, credentials, provider
payloads, tool arguments, and results are rejected.  The in-memory ledger supplies deterministic
scope checks, expiry, revocation, bounded use accounting, request replay idempotency, an
append-only hash chain, and canonical CAS-friendly snapshots.  Encryption, identity issuance,
token verification, distributed locking, and the final external authorization decision remain
deployment-owned.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
import re
from threading import RLock
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_AUTHORIZATION_SCHEMA = "bioprism-python-autonomous-authorization/0.1"
AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA = "bioprism-python-autonomous-authorization-grant/0.1"
AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA = "bioprism-python-autonomous-authorization-request/0.1"
AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA = "bioprism-python-autonomous-authorization-decision/0.1"
AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA = "bioprism-python-autonomous-authorization-event/0.1"
AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-authorization-snapshot/0.1"
AUTONOMOUS_AUTHORIZATION_RETENTION = (
    "metadata_only;tenant_actor_session_scope_and_digests;no_tasks_prompts_credentials_or_payloads"
)
AUTONOMOUS_AUTHORIZATION_AUTHORITY = (
    "caller_issued_grant_contract;identity_and_token_verification_remain_deployment_owned"
)
AUTONOMOUS_AUTHORIZATION_EXECUTION = (
    "scope_check_only;does_not_mint_identity_or_authorize_unlisted_external_effects"
)
AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL = "never_returned"
AUTONOMOUS_AUTHORIZATION_OPERATIONS = (
    "plan",
    "provider_invocation",
    "evidence_acquisition",
    "connector_dispatch",
    "tool_execution",
    "effect_dispatch",
    "evaluation",
    "learning",
    "memory_retrieval",
    "memory_write",
    "trace_write",
    "analytics_write",
)
AUTONOMOUS_AUTHORIZATION_GRANT_STATUSES = ("active", "revoked", "expired", "exhausted")
AUTONOMOUS_AUTHORIZATION_DECISION_STATUSES = (
    "allowed",
    "already_allowed",
    "not_found",
    "revoked",
    "expired",
    "exhausted",
    "tenant_mismatch",
    "actor_mismatch",
    "session_mismatch",
    "authorization_mismatch",
    "domain_denied",
    "operation_denied",
    "capability_denied",
    "risk_denied",
)
AUTONOMOUS_AUTHORIZATION_EVENT_TYPES = (
    "grant_issued",
    "grant_revoked",
    "request_allowed",
    "request_replayed",
)
MAX_AUTONOMOUS_AUTHORIZATION_GRANTS = 4_096
MAX_AUTONOMOUS_AUTHORIZATION_EVENTS = 32_768
MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT = 4_096
MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS = 31 * 86_400_000
MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES = 8_000_000
MAX_AUTONOMOUS_AUTHORIZATION_IDENTIFIER_BYTES = 256
MAX_AUTONOMOUS_AUTHORIZATION_SCOPE_ITEMS = 128

_IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:+/-]{0,255}$")
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_STATUSES = frozenset(AUTONOMOUS_AUTHORIZATION_GRANT_STATUSES)
_DECISION_STATUSES = frozenset(AUTONOMOUS_AUTHORIZATION_DECISION_STATUSES)
_EVENT_TYPES = frozenset(AUTONOMOUS_AUTHORIZATION_EVENT_TYPES)
_OPERATIONS = frozenset(AUTONOMOUS_AUTHORIZATION_OPERATIONS)


def _fail(message: str) -> None:
    raise ArgumentError(f"autonomous authorization {message}")


def _text(name: str, value: Any, maximum: int = 2_048) -> str:
    if not isinstance(value, str) or not value or "\x00" in value or len(value.encode("utf-8")) > maximum:
        _fail(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, MAX_AUTONOMOUS_AUTHORIZATION_IDENTIFIER_BYTES)
    if not _IDENTIFIER.fullmatch(result):
        _fail(f"{name} is not a safe identifier")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    result = _text(name, value, 64)
    if not _DIGEST.fullmatch(result):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return result


def _integer(name: str, value: Any, minimum: int = 0, maximum: int = 2_147_483_647) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bound")
    return value


def _timestamp(name: str, value: Any) -> int:
    return _integer(name, value, 0, 253_402_300_799_999)


def _scope(name: str, value: Any, *, allowed: set[str] | frozenset[str] | None = None, required: bool = False) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        _fail(f"{name} must be a bounded sequence")
    if len(value) > MAX_AUTONOMOUS_AUTHORIZATION_SCOPE_ITEMS or (required and not value):
        _fail(f"{name} is empty or exceeds its bound")
    normalized = tuple(_identifier(f"{name} entry", item) for item in value)
    if len(set(normalized)) != len(normalized):
        _fail(f"{name} must not contain duplicates")
    if allowed is not None and any(item not in allowed for item in normalized):
        _fail(f"{name} contains an unsupported value")
    return tuple(sorted(normalized))


def _domains(name: str, value: Any, *, required: bool = True) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        _fail(f"{name} must be a domain sequence")
    if not value and required:
        _fail(f"{name} must not be empty")
    if len(value) > MAX_AUTONOMOUS_AUTHORIZATION_SCOPE_ITEMS or (required and not value):
        _fail(f"{name} is empty or exceeds its bound")
    normalized = tuple(_identifier(f"{name} entry", item) for item in value)
    if len(set(normalized)) != len(normalized) or any(item not in AUTONOMOUS_DOMAIN_NAMES for item in normalized):
        _fail(f"{name} contains an unsupported or duplicate domain")
    canonical = tuple(domain for domain in AUTONOMOUS_DOMAIN_NAMES if domain in normalized)
    if normalized != canonical:
        _fail(f"{name} must use canonical built-in domain order")
    return canonical


def _clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False))
    except (TypeError, ValueError) as error:
        raise ArgumentError("autonomous authorization value is not canonical JSON") from error


def _safe_metadata(value: Any, depth: int = 0) -> None:
    if depth > 12:
        _fail("metadata nesting exceeds its bound")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if value != value or value in (float("inf"), -float("inf")):
            _fail("metadata contains a non-finite number")
        return
    if isinstance(value, Mapping):
        if len(value) > 512:
            _fail("metadata mapping exceeds its bound")
        forbidden = {"task", "prompt", "response", "credential", "credentials", "token", "secret", "password", "body", "headers", "messages", "arguments", "payload", "result"}
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("metadata keys must be strings")
            if key.lower().replace("_", "").replace("-", "") in forbidden:
                _fail("metadata contains transient or secret-shaped material")
            _safe_metadata(child, depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            _fail("metadata sequence exceeds its bound")
        for child in value:
            _safe_metadata(child, depth + 1)
        return
    _fail("metadata contains an unsupported value")


def authorization_context_digest(*, tenant_id: str, actor_id: str, session_id: str, authorization_digest: str) -> str:
    """Return the canonical identity binding used by requests and decisions."""

    return content_digest(
        {
            "schema": AUTONOMOUS_AUTHORIZATION_SCHEMA,
            "tenant_id": _identifier("tenant_id", tenant_id),
            "actor_id": _identifier("actor_id", actor_id),
            "session_id": _identifier("session_id", session_id),
            "authorization_digest": _digest("authorization_digest", authorization_digest),
        }
    )


def _grant_core(grant: "AutonomousAuthorizationGrant") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA,
        "grant_id": grant.grant_id,
        "tenant_id": grant.tenant_id,
        "actor_id": grant.actor_id,
        "session_id": grant.session_id,
        "authorization_digest": grant.authorization_digest,
        "allowed_domains": list(grant.allowed_domains),
        "allowed_operations": list(grant.allowed_operations),
        "allowed_capabilities": list(grant.allowed_capabilities),
        "allowed_risk_classes": list(grant.allowed_risk_classes),
        "issued_at": grant.issued_at,
        "expires_at": grant.expires_at,
        "max_uses": grant.max_uses,
    }


@dataclass(frozen=True, slots=True)
class AutonomousAuthorizationGrant:
    grant_id: str
    tenant_id: str
    actor_id: str
    session_id: str
    authorization_digest: str
    allowed_domains: tuple[str, ...]
    allowed_operations: tuple[str, ...]
    allowed_capabilities: tuple[str, ...]
    allowed_risk_classes: tuple[str, ...]
    issued_at: int
    expires_at: int
    max_uses: int | None
    used_count: int
    used_request_digests: tuple[str, ...]
    status: str
    revoked_at: int | None
    revocation_reason_digest: str | None
    grant_digest: str

    def __post_init__(self) -> None:
        _identifier("grant_id", self.grant_id)
        _identifier("grant tenant_id", self.tenant_id)
        _identifier("grant actor_id", self.actor_id)
        _identifier("grant session_id", self.session_id)
        _digest("grant authorization_digest", self.authorization_digest)
        object.__setattr__(self, "allowed_domains", _domains("grant allowed_domains", self.allowed_domains))
        object.__setattr__(self, "allowed_operations", _scope("grant allowed_operations", self.allowed_operations, allowed=_OPERATIONS, required=True))
        object.__setattr__(self, "allowed_capabilities", _scope("grant allowed_capabilities", self.allowed_capabilities))
        object.__setattr__(self, "allowed_risk_classes", _scope("grant allowed_risk_classes", self.allowed_risk_classes))
        issued = _timestamp("grant issued_at", self.issued_at)
        expires = _timestamp("grant expires_at", self.expires_at)
        if expires < issued or expires - issued > MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS:
            _fail("grant lifetime exceeds its bound")
        object.__setattr__(self, "issued_at", issued)
        object.__setattr__(self, "expires_at", expires)
        if self.max_uses is not None:
            _integer("grant max_uses", self.max_uses, 1, MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT)
        count = _integer("grant used_count", self.used_count, 0, MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT)
        digests = tuple(_digest("grant used request digest", item) for item in self.used_request_digests)
        if len(digests) != count or len(set(digests)) != len(digests):
            _fail("grant use accounting is inconsistent")
        if self.max_uses is not None and count > self.max_uses:
            _fail("grant use count exceeds max_uses")
        object.__setattr__(self, "used_count", count)
        object.__setattr__(self, "used_request_digests", digests)
        if self.status not in _STATUSES:
            _fail("grant status is invalid")
        revoked_at = None if self.revoked_at is None else _timestamp("grant revoked_at", self.revoked_at)
        reason = _digest("grant revocation_reason_digest", self.revocation_reason_digest, allow_none=True)
        if self.status == "revoked" and revoked_at is None:
            _fail("revoked grant requires revoked_at")
        if self.status != "revoked" and revoked_at is not None:
            _fail("non-revoked grant cannot retain revoked_at")
        object.__setattr__(self, "revoked_at", revoked_at)
        object.__setattr__(self, "revocation_reason_digest", reason)
        supplied = _digest("grant grant_digest", self.grant_digest)
        if supplied != content_digest(_grant_core(self)):
            _fail("grant_digest does not match grant scope")

    @classmethod
    def issue(
        cls,
        *,
        grant_id: str,
        tenant_id: str,
        actor_id: str,
        session_id: str,
        authorization_digest: str,
        allowed_domains: Sequence[str],
        allowed_operations: Sequence[str],
        allowed_capabilities: Sequence[str] = (),
        allowed_risk_classes: Sequence[str] = (),
        issued_at: int,
        expires_at: int,
        max_uses: int | None = 1,
    ) -> "AutonomousAuthorizationGrant":
        normalized_domains = _domains("grant allowed_domains", tuple(allowed_domains))
        normalized_operations = _scope("grant allowed_operations", tuple(allowed_operations), allowed=_OPERATIONS, required=True)
        normalized_capabilities = _scope("grant allowed_capabilities", tuple(allowed_capabilities))
        normalized_risks = _scope("grant allowed_risk_classes", tuple(allowed_risk_classes))
        core = {
            "schema": AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA,
            "grant_id": _identifier("grant_id", grant_id),
            "tenant_id": _identifier("grant tenant_id", tenant_id),
            "actor_id": _identifier("grant actor_id", actor_id),
            "session_id": _identifier("grant session_id", session_id),
            "authorization_digest": _digest("grant authorization_digest", authorization_digest),
            "allowed_domains": list(normalized_domains),
            "allowed_operations": list(normalized_operations),
            "allowed_capabilities": list(normalized_capabilities),
            "allowed_risk_classes": list(normalized_risks),
            "issued_at": _timestamp("grant issued_at", issued_at),
            "expires_at": _timestamp("grant expires_at", expires_at),
            "max_uses": max_uses,
        }
        if core["expires_at"] < core["issued_at"] or core["expires_at"] - core["issued_at"] > MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS:
            _fail("grant lifetime exceeds its bound")
        return cls(
            grant_id=core["grant_id"], tenant_id=core["tenant_id"], actor_id=core["actor_id"], session_id=core["session_id"],
            authorization_digest=core["authorization_digest"], allowed_domains=normalized_domains, allowed_operations=normalized_operations,
            allowed_capabilities=normalized_capabilities, allowed_risk_classes=normalized_risks, issued_at=core["issued_at"],
            expires_at=core["expires_at"], max_uses=max_uses, used_count=0, used_request_digests=(), status="active",
            revoked_at=None, revocation_reason_digest=None, grant_digest=content_digest(core),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA,
            "grant_id": self.grant_id,
            "tenant_id": self.tenant_id,
            "actor_id": self.actor_id,
            "session_id": self.session_id,
            "authorization_digest": self.authorization_digest,
            "allowed_domains": list(self.allowed_domains),
            "allowed_operations": list(self.allowed_operations),
            "allowed_capabilities": list(self.allowed_capabilities),
            "allowed_risk_classes": list(self.allowed_risk_classes),
            "issued_at": self.issued_at,
            "expires_at": self.expires_at,
            "max_uses": self.max_uses,
            "used_count": self.used_count,
            "used_request_digests": list(self.used_request_digests),
            "status": self.status,
            "revoked_at": self.revoked_at,
            "revocation_reason_digest": self.revocation_reason_digest,
            "grant_digest": self.grant_digest,
            "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
            "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
            "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
            "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousAuthorizationGrant":
        if not isinstance(value, Mapping):
            _fail("grant must be a mapping")
        expected = {
            "schema", "grant_id", "tenant_id", "actor_id", "session_id", "authorization_digest", "allowed_domains",
            "allowed_operations", "allowed_capabilities", "allowed_risk_classes", "issued_at", "expires_at", "max_uses",
            "used_count", "used_request_digests", "status", "revoked_at", "revocation_reason_digest", "grant_digest",
            "retention", "authority", "execution", "secret_material",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA:
            _fail("grant contains unsupported or missing fields")
        if value.get("retention") != AUTONOMOUS_AUTHORIZATION_RETENTION or value.get("authority") != AUTONOMOUS_AUTHORIZATION_AUTHORITY or value.get("execution") != AUTONOMOUS_AUTHORIZATION_EXECUTION or value.get("secret_material") != AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL:
            _fail("grant markers are invalid")
        return cls(
            grant_id=value["grant_id"], tenant_id=value["tenant_id"], actor_id=value["actor_id"], session_id=value["session_id"],
            authorization_digest=value["authorization_digest"], allowed_domains=tuple(value["allowed_domains"]),
            allowed_operations=tuple(value["allowed_operations"]), allowed_capabilities=tuple(value["allowed_capabilities"]),
            allowed_risk_classes=tuple(value["allowed_risk_classes"]), issued_at=value["issued_at"], expires_at=value["expires_at"],
            max_uses=value["max_uses"], used_count=value["used_count"], used_request_digests=tuple(value["used_request_digests"]),
            status=value["status"], revoked_at=value["revoked_at"], revocation_reason_digest=value["revocation_reason_digest"],
            grant_digest=value["grant_digest"],
        )


def _request_core(request: "AutonomousAuthorizationRequest") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA,
        "request_id": request.request_id,
        "grant_id": request.grant_id,
        "tenant_id": request.tenant_id,
        "actor_id": request.actor_id,
        "session_id": request.session_id,
        "authorization_digest": request.authorization_digest,
        "domains": list(request.domains),
        "operation": request.operation,
        "capability": request.capability,
        "risk_class": request.risk_class,
        "resource_digest": request.resource_digest,
        "issued_at": request.issued_at,
    }


@dataclass(frozen=True, slots=True)
class AutonomousAuthorizationRequest:
    request_id: str
    grant_id: str
    tenant_id: str
    actor_id: str
    session_id: str
    authorization_digest: str
    domains: tuple[str, ...]
    operation: str
    capability: str | None
    risk_class: str | None
    resource_digest: str | None
    issued_at: int
    request_digest: str

    def __post_init__(self) -> None:
        _identifier("request_id", self.request_id)
        _identifier("request grant_id", self.grant_id)
        _identifier("request tenant_id", self.tenant_id)
        _identifier("request actor_id", self.actor_id)
        _identifier("request session_id", self.session_id)
        _digest("request authorization_digest", self.authorization_digest)
        object.__setattr__(self, "domains", _domains("request domains", self.domains))
        operation = _identifier("request operation", self.operation)
        if operation not in _OPERATIONS:
            _fail("request operation is unsupported")
        object.__setattr__(self, "operation", operation)
        capability = None if self.capability is None else _identifier("request capability", self.capability)
        risk_class = None if self.risk_class is None else _identifier("request risk_class", self.risk_class)
        object.__setattr__(self, "capability", capability)
        object.__setattr__(self, "risk_class", risk_class)
        object.__setattr__(self, "resource_digest", _digest("request resource_digest", self.resource_digest, allow_none=True))
        object.__setattr__(self, "issued_at", _timestamp("request issued_at", self.issued_at))
        supplied = _digest("request request_digest", self.request_digest)
        if supplied != content_digest(_request_core(self)):
            _fail("request_digest does not match request metadata")

    @classmethod
    def create(
        cls,
        *,
        request_id: str,
        grant_id: str,
        tenant_id: str,
        actor_id: str,
        session_id: str,
        authorization_digest: str,
        domains: Sequence[str],
        operation: str,
        capability: str | None = None,
        risk_class: str | None = None,
        resource_digest: str | None = None,
        issued_at: int,
    ) -> "AutonomousAuthorizationRequest":
        normalized_request = {
            "schema": AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA,
            "request_id": _identifier("request_id", request_id),
            "grant_id": _identifier("request grant_id", grant_id),
            "tenant_id": _identifier("request tenant_id", tenant_id),
            "actor_id": _identifier("request actor_id", actor_id),
            "session_id": _identifier("request session_id", session_id),
            "authorization_digest": _digest("request authorization_digest", authorization_digest),
            "domains": list(_domains("request domains", tuple(domains))),
            "operation": _identifier("request operation", operation),
            "capability": None if capability is None else _identifier("request capability", capability),
            "risk_class": None if risk_class is None else _identifier("request risk_class", risk_class),
            "resource_digest": _digest("request resource_digest", resource_digest, allow_none=True),
            "issued_at": _timestamp("request issued_at", issued_at),
        }
        if normalized_request["operation"] not in _OPERATIONS:
            _fail("request operation is unsupported")
        return cls(
            request_id=normalized_request["request_id"], grant_id=normalized_request["grant_id"], tenant_id=normalized_request["tenant_id"],
            actor_id=normalized_request["actor_id"], session_id=normalized_request["session_id"], authorization_digest=normalized_request["authorization_digest"],
            domains=tuple(normalized_request["domains"]), operation=normalized_request["operation"], capability=normalized_request["capability"],
            risk_class=normalized_request["risk_class"], resource_digest=normalized_request["resource_digest"], issued_at=normalized_request["issued_at"],
            request_digest=content_digest(normalized_request),
        )

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousAuthorizationRequest":
        if not isinstance(value, Mapping):
            _fail("request must be a mapping")
        expected = {
            "schema", "request_id", "grant_id", "tenant_id", "actor_id", "session_id", "authorization_digest", "domains",
            "operation", "capability", "risk_class", "resource_digest", "issued_at", "request_digest", "retention", "authority",
            "execution", "secret_material",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA:
            _fail("request contains unsupported or missing fields")
        if value.get("retention") != AUTONOMOUS_AUTHORIZATION_RETENTION or value.get("authority") != AUTONOMOUS_AUTHORIZATION_AUTHORITY or value.get("execution") != AUTONOMOUS_AUTHORIZATION_EXECUTION or value.get("secret_material") != AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL:
            _fail("request markers are invalid")
        return cls(
            request_id=value["request_id"], grant_id=value["grant_id"], tenant_id=value["tenant_id"], actor_id=value["actor_id"],
            session_id=value["session_id"], authorization_digest=value["authorization_digest"], domains=tuple(value["domains"]),
            operation=value["operation"], capability=value["capability"], risk_class=value["risk_class"], resource_digest=value["resource_digest"],
            issued_at=value["issued_at"], request_digest=value["request_digest"],
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            **_request_core(self),
            "request_digest": self.request_digest,
            "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
            "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
            "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
            "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
        }

    @property
    def context_digest(self) -> str:
        return authorization_context_digest(
            tenant_id=self.tenant_id, actor_id=self.actor_id, session_id=self.session_id, authorization_digest=self.authorization_digest
        )


def _decision_core(decision: "AutonomousAuthorizationDecision") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA,
        "status": decision.status,
        "grant_id": decision.grant_id,
        "request_digest": decision.request_digest,
        "grant_digest": decision.grant_digest,
        "context_digest": decision.context_digest,
        "checked_at": decision.checked_at,
        "reason": decision.reason,
        "remaining_uses": decision.remaining_uses,
    }


@dataclass(frozen=True, slots=True)
class AutonomousAuthorizationDecision:
    status: str
    grant_id: str
    request_digest: str
    grant_digest: str | None
    context_digest: str
    checked_at: int
    reason: str
    remaining_uses: int | None
    decision_digest: str

    def __post_init__(self) -> None:
        if self.status not in _DECISION_STATUSES:
            _fail("decision status is invalid")
        _identifier("decision grant_id", self.grant_id)
        _digest("decision request_digest", self.request_digest)
        _digest("decision grant_digest", self.grant_digest, allow_none=True)
        _digest("decision context_digest", self.context_digest)
        _timestamp("decision checked_at", self.checked_at)
        _identifier("decision reason", self.reason)
        if self.remaining_uses is not None:
            _integer("decision remaining_uses", self.remaining_uses, 0, MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT)
        supplied = _digest("decision decision_digest", self.decision_digest)
        if supplied != content_digest(_decision_core(self)):
            _fail("decision_digest does not match decision metadata")

    @classmethod
    def create(cls, *, status: str, request: AutonomousAuthorizationRequest, grant: AutonomousAuthorizationGrant | None, checked_at: int, reason: str, remaining_uses: int | None) -> "AutonomousAuthorizationDecision":
        normalized_reason = _identifier("decision reason", reason)
        core = {
            "schema": AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA,
            "status": status,
            "grant_id": request.grant_id,
            "request_digest": request.request_digest,
            "grant_digest": None if grant is None else grant.grant_digest,
            "context_digest": request.context_digest,
            "checked_at": _timestamp("decision checked_at", checked_at),
            "reason": normalized_reason,
            "remaining_uses": remaining_uses,
        }
        return cls(status, request.grant_id, request.request_digest, core["grant_digest"], request.context_digest, core["checked_at"], normalized_reason, remaining_uses, content_digest(core))

    def to_dict(self) -> dict[str, Any]:
        return {
            **_decision_core(self),
            "decision_digest": self.decision_digest,
            "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
            "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
            "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
            "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousAuthorizationEvent:
    sequence: int
    event_type: str
    grant_id: str
    request_digest: str | None
    occurred_at: int
    reason: str
    previous_event_digest: str | None
    event_digest: str

    def __post_init__(self) -> None:
        _integer("event sequence", self.sequence, 1, MAX_AUTONOMOUS_AUTHORIZATION_EVENTS)
        if self.event_type not in _EVENT_TYPES:
            _fail("event type is invalid")
        _identifier("event grant_id", self.grant_id)
        _digest("event request_digest", self.request_digest, allow_none=True)
        _timestamp("event occurred_at", self.occurred_at)
        _identifier("event reason", self.reason)
        _digest("event previous_event_digest", self.previous_event_digest, allow_none=True)
        _digest("event event_digest", self.event_digest)
        body = {
            "schema": AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA,
            "sequence": self.sequence,
            "event_type": self.event_type,
            "grant_id": self.grant_id,
            "request_digest": self.request_digest,
            "occurred_at": self.occurred_at,
            "reason": self.reason,
            "previous_event_digest": self.previous_event_digest,
        }
        if content_digest(body) != self.event_digest:
            _fail("event_digest does not match event metadata")

    @classmethod
    def create(cls, *, sequence: int, event_type: str, grant_id: str, request_digest: str | None, occurred_at: int, reason: str, previous_event_digest: str | None) -> "AutonomousAuthorizationEvent":
        body = {
            "schema": AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA,
            "sequence": sequence,
            "event_type": event_type,
            "grant_id": grant_id,
            "request_digest": request_digest,
            "occurred_at": occurred_at,
            "reason": reason,
            "previous_event_digest": previous_event_digest,
        }
        return cls(sequence, event_type, grant_id, request_digest, occurred_at, reason, previous_event_digest, content_digest(body))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA,
            "sequence": self.sequence,
            "event_type": self.event_type,
            "grant_id": self.grant_id,
            "request_digest": self.request_digest,
            "occurred_at": self.occurred_at,
            "reason": self.reason,
            "previous_event_digest": self.previous_event_digest,
            "event_digest": self.event_digest,
            "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
            "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
            "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
            "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousAuthorizationEvent":
        if not isinstance(value, Mapping):
            _fail("event must be a mapping")
        expected = {"schema", "sequence", "event_type", "grant_id", "request_digest", "occurred_at", "reason", "previous_event_digest", "event_digest", "retention", "authority", "execution", "secret_material"}
        if set(value) != expected or value.get("schema") != AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA:
            _fail("event contains unsupported or missing fields")
        if value.get("retention") != AUTONOMOUS_AUTHORIZATION_RETENTION or value.get("authority") != AUTONOMOUS_AUTHORIZATION_AUTHORITY or value.get("execution") != AUTONOMOUS_AUTHORIZATION_EXECUTION or value.get("secret_material") != AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL:
            _fail("event markers are invalid")
        return cls(value["sequence"], value["event_type"], value["grant_id"], value["request_digest"], value["occurred_at"], value["reason"], value["previous_event_digest"], value["event_digest"])


def _validate_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("snapshot must be a mapping")
    expected = {"schema", "generation", "previous_snapshot_digest", "grants", "events", "retention", "authority", "execution", "secret_material", "snapshot_digest"}
    if set(value) != expected or value.get("schema") != AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA:
        _fail("snapshot contains unsupported or missing fields")
    if value.get("retention") != AUTONOMOUS_AUTHORIZATION_RETENTION or value.get("authority") != AUTONOMOUS_AUTHORIZATION_AUTHORITY or value.get("execution") != AUTONOMOUS_AUTHORIZATION_EXECUTION or value.get("secret_material") != AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL:
        _fail("snapshot markers are invalid")
    _safe_metadata(value)
    generation = _integer("snapshot generation", value["generation"], 0, 2_147_483_647)
    previous = _digest("snapshot previous_snapshot_digest", value["previous_snapshot_digest"], allow_none=True)
    raw_grants = value["grants"]
    raw_events = value["events"]
    if not isinstance(raw_grants, list) or len(raw_grants) > MAX_AUTONOMOUS_AUTHORIZATION_GRANTS:
        _fail("snapshot grants exceed their bound")
    if not isinstance(raw_events, list) or len(raw_events) > MAX_AUTONOMOUS_AUTHORIZATION_EVENTS:
        _fail("snapshot events exceed their bound")
    grants = [AutonomousAuthorizationGrant.from_dict(item) for item in raw_grants]
    if len({grant.grant_id for grant in grants}) != len(grants):
        _fail("snapshot contains duplicate grant ids")
    grants.sort(key=lambda grant: grant.grant_id)
    events = [AutonomousAuthorizationEvent.from_dict(item) for item in raw_events]
    grant_ids = {grant.grant_id for grant in grants}
    issued_grant_ids: set[str] = set()
    for expected_sequence, event in enumerate(events, 1):
        if event.sequence != expected_sequence:
            _fail("snapshot event sequence is not contiguous")
        previous_event = None if expected_sequence == 1 else events[expected_sequence - 2].event_digest
        if event.previous_event_digest != previous_event:
            _fail("snapshot event hash chain is broken")
        if event.grant_id not in grant_ids:
            _fail("snapshot event references an unknown grant")
        grant = next(grant for grant in grants if grant.grant_id == event.grant_id)
        if event.event_type == "grant_issued":
            if event.request_digest is not None or event.grant_id in issued_grant_ids:
                _fail("snapshot grant issuance history is inconsistent")
            issued_grant_ids.add(event.grant_id)
        elif event.grant_id not in issued_grant_ids:
            _fail("snapshot event precedes grant issuance")
        elif event.event_type == "grant_revoked":
            if event.request_digest is not None or grant.status != "revoked":
                _fail("snapshot grant revocation history is inconsistent")
        elif event.event_type in {"request_allowed", "request_replayed"}:
            if event.request_digest is None or event.request_digest not in grant.used_request_digests:
                _fail("snapshot request history is inconsistent with grant use accounting")
    if issued_grant_ids != grant_ids:
        _fail("snapshot is missing grant issuance history")
    body = {
        "schema": AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA,
        "generation": generation,
        "previous_snapshot_digest": previous,
        "grants": [grant.to_dict() for grant in grants],
        "events": [event.to_dict() for event in events],
        "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
        "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
        "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
        "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
    }
    supplied = _digest("snapshot snapshot_digest", value["snapshot_digest"])
    expected_digest = content_digest(body)
    if supplied != expected_digest:
        _fail("snapshot_digest does not match snapshot contents")
    result = _clone({**body, "snapshot_digest": expected_digest})
    if len(canonical_json(result).encode("utf-8")) > MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return result


def seal_autonomous_authorization_snapshot(*, generation: int, grants: Sequence[AutonomousAuthorizationGrant | Mapping[str, Any]], events: Sequence[AutonomousAuthorizationEvent | Mapping[str, Any]], previous_snapshot_digest: str | None = None) -> dict[str, Any]:
    generation = _integer("snapshot generation", generation, 0, 2_147_483_647)
    normalized_grants = [item if isinstance(item, AutonomousAuthorizationGrant) else AutonomousAuthorizationGrant.from_dict(item) for item in grants]
    normalized_events = [item if isinstance(item, AutonomousAuthorizationEvent) else AutonomousAuthorizationEvent.from_dict(item) for item in events]
    if len(normalized_grants) > MAX_AUTONOMOUS_AUTHORIZATION_GRANTS or len(normalized_events) > MAX_AUTONOMOUS_AUTHORIZATION_EVENTS:
        _fail("snapshot exceeds its record bound")
    body = {
        "schema": AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA,
        "generation": generation,
        "previous_snapshot_digest": _digest("snapshot previous_snapshot_digest", previous_snapshot_digest, allow_none=True),
        "grants": [item.to_dict() for item in sorted(normalized_grants, key=lambda item: item.grant_id)],
        "events": [item.to_dict() for item in normalized_events],
        "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
        "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
        "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
        "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
    }
    result = _clone({**body, "snapshot_digest": content_digest(body)})
    return _validate_snapshot(result)


class AutonomousAuthorizationLedger:
    """A bounded, single-process authorization ledger with CAS-ready snapshots."""

    def __init__(self, *, max_grants: int = MAX_AUTONOMOUS_AUTHORIZATION_GRANTS, max_events: int = MAX_AUTONOMOUS_AUTHORIZATION_EVENTS) -> None:
        self.max_grants = _integer("max_grants", max_grants, 1, MAX_AUTONOMOUS_AUTHORIZATION_GRANTS)
        self.max_events = _integer("max_events", max_events, 1, MAX_AUTONOMOUS_AUTHORIZATION_EVENTS)
        self._grants: dict[str, AutonomousAuthorizationGrant] = {}
        self._events: list[AutonomousAuthorizationEvent] = []
        self._generation = 0
        self._previous_snapshot_digest: str | None = None
        self._lock = RLock()

    def _append_event(self, *, event_type: str, grant_id: str, request_digest: str | None, occurred_at: int, reason: str) -> None:
        if len(self._events) >= self.max_events:
            _fail("event capacity is exhausted")
        previous = None if not self._events else self._events[-1].event_digest
        self._events.append(AutonomousAuthorizationEvent.create(sequence=len(self._events) + 1, event_type=event_type, grant_id=grant_id, request_digest=request_digest, occurred_at=occurred_at, reason=_identifier("event reason", reason), previous_event_digest=previous))

    def issue(self, *, grant_id: str, tenant_id: str, actor_id: str, session_id: str, authorization_digest: str, allowed_domains: Sequence[str], allowed_operations: Sequence[str], allowed_capabilities: Sequence[str] = (), allowed_risk_classes: Sequence[str] = (), issued_at: int, expires_at: int, max_uses: int | None = 1) -> AutonomousAuthorizationGrant:
        with self._lock:
            normalized_id = _identifier("grant_id", grant_id)
            if normalized_id in self._grants:
                _fail("grant_id already exists")
            if len(self._grants) >= self.max_grants:
                _fail("grant capacity is exhausted")
            grant = AutonomousAuthorizationGrant.issue(grant_id=normalized_id, tenant_id=tenant_id, actor_id=actor_id, session_id=session_id, authorization_digest=authorization_digest, allowed_domains=allowed_domains, allowed_operations=allowed_operations, allowed_capabilities=allowed_capabilities, allowed_risk_classes=allowed_risk_classes, issued_at=issued_at, expires_at=expires_at, max_uses=max_uses)
            self._grants[normalized_id] = grant
            self._append_event(event_type="grant_issued", grant_id=normalized_id, request_digest=None, occurred_at=issued_at, reason="issued")
            return grant

    def revoke(self, grant_id: str, *, revoked_at: int, reason: str = "revoked") -> AutonomousAuthorizationGrant:
        with self._lock:
            normalized_id = _identifier("grant_id", grant_id)
            current = self._grants.get(normalized_id)
            if current is None:
                _fail("cannot revoke an unknown grant")
            if current.status == "revoked":
                return current
            reason_id = _identifier("revocation reason", reason)
            updated = replace(current, status="revoked", revoked_at=_timestamp("revoked_at", revoked_at), revocation_reason_digest=content_digest(reason_id))
            self._grants[normalized_id] = updated
            self._append_event(event_type="grant_revoked", grant_id=normalized_id, request_digest=None, occurred_at=revoked_at, reason=reason_id)
            return updated

    def _status(self, grant: AutonomousAuthorizationGrant, now: int) -> str:
        if grant.status == "revoked":
            return "revoked"
        if now >= grant.expires_at:
            return "expired"
        if grant.max_uses is not None and grant.used_count >= grant.max_uses:
            return "exhausted"
        return "active"

    def authorize(self, request: AutonomousAuthorizationRequest | Mapping[str, Any], *, now: int) -> AutonomousAuthorizationDecision:
        normalized = request if isinstance(request, AutonomousAuthorizationRequest) else AutonomousAuthorizationRequest.from_dict(request)
        checked_at = _timestamp("authorization checked_at", now)
        with self._lock:
            grant = self._grants.get(normalized.grant_id)
            if grant is None:
                return AutonomousAuthorizationDecision.create(status="not_found", request=normalized, grant=None, checked_at=checked_at, reason="grant_not_found", remaining_uses=None)
            status = self._status(grant, checked_at)
            if status != grant.status and status in {"expired", "exhausted"}:
                grant = replace(grant, status=status)
                self._grants[grant.grant_id] = grant
            remaining = None if grant.max_uses is None else max(0, grant.max_uses - grant.used_count)
            if status != "active":
                return AutonomousAuthorizationDecision.create(status=status, request=normalized, grant=grant, checked_at=checked_at, reason=f"grant_{status}", remaining_uses=remaining)
            if normalized.request_digest in grant.used_request_digests:
                self._append_event(event_type="request_replayed", grant_id=grant.grant_id, request_digest=normalized.request_digest, occurred_at=checked_at, reason="request_replay")
                return AutonomousAuthorizationDecision.create(status="already_allowed", request=normalized, grant=grant, checked_at=checked_at, reason="request_replay", remaining_uses=remaining)
            checks = (
                (normalized.tenant_id == grant.tenant_id, "tenant_mismatch"),
                (normalized.actor_id == grant.actor_id, "actor_mismatch"),
                (normalized.session_id == grant.session_id, "session_mismatch"),
                (normalized.authorization_digest == grant.authorization_digest, "authorization_mismatch"),
                (set(normalized.domains).issubset(set(grant.allowed_domains)), "domain_denied"),
                (normalized.operation in grant.allowed_operations, "operation_denied"),
                (not grant.allowed_capabilities or normalized.capability in grant.allowed_capabilities, "capability_denied"),
                (not grant.allowed_risk_classes or normalized.risk_class in grant.allowed_risk_classes, "risk_denied"),
            )
            for passed, failure in checks:
                if not passed:
                    return AutonomousAuthorizationDecision.create(status=failure, request=normalized, grant=grant, checked_at=checked_at, reason=failure, remaining_uses=remaining)
            if len(grant.used_request_digests) >= MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT:
                _fail("grant request replay window is exhausted")
            next_count = grant.used_count + 1
            next_status = "exhausted" if grant.max_uses is not None and next_count >= grant.max_uses else "active"
            grant = replace(grant, used_count=next_count, used_request_digests=(*grant.used_request_digests, normalized.request_digest), status=next_status)
            self._grants[grant.grant_id] = grant
            self._append_event(event_type="request_allowed", grant_id=grant.grant_id, request_digest=normalized.request_digest, occurred_at=checked_at, reason="allowed")
            return AutonomousAuthorizationDecision.create(status="allowed", request=normalized, grant=grant, checked_at=checked_at, reason="allowed", remaining_uses=None if grant.max_uses is None else max(0, grant.max_uses - grant.used_count))

    def get(self, grant_id: str) -> AutonomousAuthorizationGrant | None:
        with self._lock:
            grant = self._grants.get(_identifier("grant_id", grant_id))
            return None if grant is None else replace(grant)

    def grants(self) -> list[AutonomousAuthorizationGrant]:
        with self._lock:
            return [self._grants[key] for key in sorted(self._grants)]

    def events(self) -> list[AutonomousAuthorizationEvent]:
        with self._lock:
            return list(self._events)

    def snapshot(self, *, generation: int | None = None, previous_snapshot_digest: str | None = None) -> dict[str, Any]:
        with self._lock:
            return seal_autonomous_authorization_snapshot(
                generation=self._generation if generation is None else generation,
                grants=self.grants(),
                events=self.events(),
                previous_snapshot_digest=self._previous_snapshot_digest if previous_snapshot_digest is None else previous_snapshot_digest,
            )

    def restore(self, snapshot: Mapping[str, Any]) -> dict[str, Any]:
        normalized = _validate_snapshot(snapshot)
        grants = [AutonomousAuthorizationGrant.from_dict(item) for item in normalized["grants"]]
        events = [AutonomousAuthorizationEvent.from_dict(item) for item in normalized["events"]]
        if len(grants) > self.max_grants or len(events) > self.max_events:
            _fail("snapshot exceeds ledger capacity")
        with self._lock:
            self._grants = {grant.grant_id: grant for grant in grants}
            self._events = events
            self._generation = normalized["generation"]
            self._previous_snapshot_digest = normalized["snapshot_digest"]
        return _clone(normalized)

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.snapshot()
            grants = list(self._grants.values())
            return {
                "verified": True,
                "grant_count": len(grants),
                "event_count": len(self._events),
                "active_grant_count": sum(grant.status == "active" for grant in grants),
                "revoked_grant_count": sum(grant.status == "revoked" for grant in grants),
                "expired_grant_count": sum(grant.status == "expired" for grant in grants),
                "exhausted_grant_count": sum(grant.status == "exhausted" for grant in grants),
                "domain_coverage": {domain: sum(domain in grant.allowed_domains for grant in grants) for domain in AUTONOMOUS_DOMAIN_NAMES},
                "snapshot_digest": snapshot["snapshot_digest"],
                "retention": AUTONOMOUS_AUTHORIZATION_RETENTION,
                "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
                "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
                "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
            }


@dataclass(frozen=True, slots=True)
class AutonomousAuthorizedOperation:
    """Transient result of an operation admitted by :class:`AutonomousAuthorizationGate`.

    ``result`` is deliberately not serializable through this module.  It belongs to the caller
    and may contain a provider response, source value, or effect receipt; only ``decision`` is a
    durable-safe projection.
    """

    decision: AutonomousAuthorizationDecision
    result: Any

    def to_dict(self) -> dict[str, Any]:
        return {
            "decision": self.decision.to_dict(),
            "result_present": True,
            "result_retained": False,
            "retention": "transient_caller_result_only",
            "authority": AUTONOMOUS_AUTHORIZATION_AUTHORITY,
            "execution": AUTONOMOUS_AUTHORIZATION_EXECUTION,
            "secret_material": AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL,
        }


class AutonomousAuthorizationGate:
    """Apply a ledger decision immediately before a caller-owned operation."""

    def __init__(self, ledger: AutonomousAuthorizationLedger) -> None:
        if not isinstance(ledger, AutonomousAuthorizationLedger):
            _fail("gate requires an AutonomousAuthorizationLedger")
        self.ledger = ledger

    def require(self, request: AutonomousAuthorizationRequest | Mapping[str, Any], *, now: int) -> AutonomousAuthorizationDecision:
        decision = self.ledger.authorize(request, now=now)
        if decision.status not in {"allowed", "already_allowed"}:
            _fail(f"operation authorization was refused: {decision.status}")
        return decision

    def execute(
        self,
        request: AutonomousAuthorizationRequest | Mapping[str, Any],
        *,
        now: int,
        operation: Callable[[], Any],
    ) -> AutonomousAuthorizedOperation:
        if not callable(operation):
            _fail("authorized operation must be callable")
        decision = self.require(request, now=now)
        return AutonomousAuthorizedOperation(decision=decision, result=operation())


class AutonomousAuthorizationSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousAuthorizationSnapshotTextStore(AutonomousAuthorizationSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousAuthorizationSnapshotPersistence:
    def __init__(self, store: AutonomousAuthorizationSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            _fail("JSON persistence requires a text store")
        self.store = store
        self.max_bytes = _integer("persistence max_bytes", max_bytes, 1, MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("stored JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("autonomous authorization stored JSON is invalid") from error
        normalized = _validate_snapshot(raw)
        if canonical_json(normalized) != encoded:
            _fail("stored JSON is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _validate_snapshot(snapshot)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("snapshot exceeds configured byte capacity")
        self.store.write(encoded)


class TransactionalJsonAutonomousAuthorizationSnapshotPersistence(JsonAutonomousAuthorizationSnapshotPersistence):
    def __init__(self, store: TransactionalAutonomousAuthorizationSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("transactional JSON persistence requires write_if_unchanged")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected_snapshot_digest", expected_snapshot_digest, allow_none=True)
        normalized = _validate_snapshot(snapshot)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(normalized)))


class AutonomousAuthorizationPersistenceCoordinator:
    """Restore-before-read and CAS-fenced flush for an authorization ledger."""

    def __init__(self, ledger: AutonomousAuthorizationLedger, persistence: JsonAutonomousAuthorizationSnapshotPersistence) -> None:
        if not isinstance(ledger, AutonomousAuthorizationLedger):
            _fail("coordinator requires an AutonomousAuthorizationLedger")
        if not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("coordinator persistence is malformed")
        self.ledger = ledger
        self.persistence = persistence
        self.expected_snapshot_digest: str | None = None
        self.expected_generation = 0

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            self.expected_snapshot_digest = None
            self.expected_generation = 0
            return None
        normalized = self.ledger.restore(snapshot)
        self.expected_snapshot_digest = normalized["snapshot_digest"]
        self.expected_generation = normalized["generation"]
        return normalized

    def flush(self) -> dict[str, Any]:
        snapshot = self.ledger.snapshot(generation=self.expected_generation + 1, previous_snapshot_digest=self.expected_snapshot_digest)
        writer = getattr(self.persistence, "write_if_unchanged", None)
        if callable(writer):
            if not writer(self.expected_snapshot_digest, snapshot):
                _fail("persistence compare-and-set conflict")
        else:
            self.persistence.write(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        self.expected_generation = snapshot["generation"]
        return snapshot


def validate_autonomous_authorization_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    return _validate_snapshot(value)


__all__ = [
    "AUTONOMOUS_AUTHORIZATION_SCHEMA",
    "AUTONOMOUS_AUTHORIZATION_GRANT_SCHEMA",
    "AUTONOMOUS_AUTHORIZATION_REQUEST_SCHEMA",
    "AUTONOMOUS_AUTHORIZATION_DECISION_SCHEMA",
    "AUTONOMOUS_AUTHORIZATION_EVENT_SCHEMA",
    "AUTONOMOUS_AUTHORIZATION_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_AUTHORIZATION_RETENTION",
    "AUTONOMOUS_AUTHORIZATION_AUTHORITY",
    "AUTONOMOUS_AUTHORIZATION_EXECUTION",
    "AUTONOMOUS_AUTHORIZATION_SECRET_MATERIAL",
    "AUTONOMOUS_AUTHORIZATION_OPERATIONS",
    "AUTONOMOUS_AUTHORIZATION_GRANT_STATUSES",
    "AUTONOMOUS_AUTHORIZATION_DECISION_STATUSES",
    "AUTONOMOUS_AUTHORIZATION_EVENT_TYPES",
    "MAX_AUTONOMOUS_AUTHORIZATION_GRANTS",
    "MAX_AUTONOMOUS_AUTHORIZATION_EVENTS",
    "MAX_AUTONOMOUS_AUTHORIZATION_REQUEST_DIGESTS_PER_GRANT",
    "MAX_AUTONOMOUS_AUTHORIZATION_TTL_MS",
    "MAX_AUTONOMOUS_AUTHORIZATION_SNAPSHOT_BYTES",
    "authorization_context_digest",
    "AutonomousAuthorizationGrant",
    "AutonomousAuthorizationRequest",
    "AutonomousAuthorizationDecision",
    "AutonomousAuthorizationEvent",
    "AutonomousAuthorizationLedger",
    "AutonomousAuthorizedOperation",
    "AutonomousAuthorizationGate",
    "AutonomousAuthorizationSnapshotTextStore",
    "TransactionalAutonomousAuthorizationSnapshotTextStore",
    "JsonAutonomousAuthorizationSnapshotPersistence",
    "TransactionalJsonAutonomousAuthorizationSnapshotPersistence",
    "AutonomousAuthorizationPersistenceCoordinator",
    "seal_autonomous_authorization_snapshot",
    "validate_autonomous_authorization_snapshot",
]
