"""Durable, redacted capability activation for the autonomous agent.

Provider keys and credential handles are deliberately absent from this module.  It stores only
the metadata needed to resume an embedding application's onboarding UI: provider readiness,
live-catalogue/profile digests, exact approved tool names, per-domain coverage, and a bounded
activation status.  The state is not an execution journal and never grants authority by itself;
the provider runtime, domain-tool runtime, mission policy, and caller approval callbacks remain
the final authorization boundaries.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
import math
import os
from pathlib import Path
import tempfile
import threading
import time
from typing import Any, Callable, Mapping, Sequence
import uuid

from .authoring import canonical_bytes, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES, DOMAIN_TOOL_BINDING_PLAN_SCHEMA
from .errors import ArgumentError


AUTONOMOUS_ACTIVATION_SCHEMA = "bioprism-python-autonomous-capability-activation/0.1"
AUTONOMOUS_ACTIVATION_STORE_SCHEMA = "bioprism-python-autonomous-capability-activation-store/0.1"
AUTONOMOUS_ACTIVATION_STATUSES = (
    "created",
    "provider_pending",
    "catalogue_pending",
    "review_required",
    "partially_activated",
    "ready",
    "stale",
    "revoked",
)
_ALLOWED_ACTIVATION_TRANSITIONS = {
    "created": {"created", "provider_pending", "catalogue_pending", "review_required", "revoked"},
    "provider_pending": {
        "provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked",
    },
    "catalogue_pending": {
        "provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked",
    },
    "review_required": {
        "provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked",
    },
    "partially_activated": {
        "provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked",
    },
    "ready": {
        "provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked",
    },
    "stale": {
        "provider_pending", "catalogue_pending", "review_required", "partially_activated", "ready", "stale", "revoked",
    },
    "revoked": {"revoked"},
}
MAX_ACTIVATION_PROVIDERS = 64
MAX_ACTIVATION_TOOLS = 512
MAX_ACTIVATION_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_ACTIVATION_STATE_BYTES = 512_000
MAX_ACTIVATION_STORE_BYTES = 1_000_000
MAX_ACTIVATION_ERROR_BYTES = 2_000
_SAFE_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-"
)
_SECRET_FIELDS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "password",
        "secret",
        "accesstoken",
        "refreshtoken",
        "token",
        "privatekey",
        "prompt",
        "response",
        "rawpayload",
        "arguments",
        "output",
        "task",
        "messages",
    }
)


class AutonomousActivationError(ArgumentError):
    """A durable capability activation transition or snapshot was refused."""


def _text(name: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomousActivationError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise AutonomousActivationError(f"{name} exceeds its bounded size")
    return value


def _identifier(name: str, value: Any, *, maximum: int = 512) -> str:
    result = _text(name, value, maximum=maximum)
    if any(character not in _SAFE_IDENTIFIER_CHARS for character in result):
        raise AutonomousActivationError(f"{name} must be a bounded identifier")
    return result


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise AutonomousActivationError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(
        character in "0123456789abcdef" for character in value
    )


def _assert_safe(value: Any, *, depth: int = 0) -> None:
    if depth > 24:
        raise AutonomousActivationError("activation metadata is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                raise AutonomousActivationError("activation metadata keys must be strings")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _SECRET_FIELDS:
                raise AutonomousActivationError("activation metadata contains transient or secret-shaped fields")
            _assert_safe(child, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _assert_safe(child, depth=depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise AutonomousActivationError("activation metadata contains a non-finite number")


def _safe_mapping(name: str, value: Mapping[str, Any], *, maximum: int = 32_000) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise AutonomousActivationError(f"{name} must be a mapping")
    _assert_safe(value)
    try:
        encoded = canonical_bytes(value)
    except (ArgumentError, TypeError, ValueError) as error:
        raise AutonomousActivationError(f"{name} must be canonical JSON") from error
    if len(encoded) > maximum:
        raise AutonomousActivationError(f"{name} exceeds its bounded size")
    return json.loads(encoded.decode("utf-8"))


def _string_tuple(name: str, value: Any, *, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise AutonomousActivationError(f"{name} must be a sequence")
    if len(value) > maximum:
        raise AutonomousActivationError(f"{name} exceeds its bounded size")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        item_text = _identifier(f"{name} entry", item)
        if item_text in seen:
            raise AutonomousActivationError(f"{name} contains a duplicate entry: {item_text}")
        seen.add(item_text)
        result.append(item_text)
    return tuple(sorted(result))


def _finite_time(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or float(value) < 0:
        raise AutonomousActivationError(f"{name} must be a finite non-negative timestamp")
    return float(value)


def _provider_projection(value: Mapping[str, Any]) -> dict[str, Any]:
    """Project onboarding status without copying the nested credential metadata."""

    if not isinstance(value, Mapping):
        raise AutonomousActivationError("provider status must be a mapping")
    provider = _identifier("activation provider", value.get("provider"))
    requires = value.get("requires_credential")
    if requires is not None and not isinstance(requires, bool):
        raise AutonomousActivationError("provider requires_credential must be a boolean or None")
    credential = value.get("credential")
    if credential is not None and not isinstance(credential, Mapping):
        raise AutonomousActivationError("provider credential status must be a mapping or None")
    configured = bool(credential.get("configured", False)) if isinstance(credential, Mapping) else bool(value.get("credential_ready", False))
    count = credential.get("credential_count", 0) if isinstance(credential, Mapping) else value.get("credential_count", 0)
    if not isinstance(count, int) or isinstance(count, bool) or count < 0 or count > MAX_ACTIVATION_PROVIDERS:
        raise AutonomousActivationError("provider credential_count is outside its bound")
    ready = value.get("ready", value.get("credential_ready", False))
    if not isinstance(ready, bool):
        raise AutonomousActivationError("provider readiness must be a boolean")
    next_action = value.get("next_action", "ready" if ready else "collect_user_credential")
    next_action = _identifier("provider next_action", next_action)
    return {
        "provider": provider,
        "provider_registered": bool(value.get("provider_registered", False)),
        "requires_credential": requires,
        "credential_configured": configured,
        "credential_count": count,
        "ready": ready,
        "next_action": next_action,
        "secret_persistence": "in_memory_only",
    }


def _coverage_projection(domain: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise AutonomousActivationError(f"coverage for {domain!r} must be a mapping")
    required = value.get("required_tool_count", 0)
    available = value.get("available_tool_count", 0)
    proposed = value.get("proposed_tool_count", 0)
    for name, count in (("required_tool_count", required), ("available_tool_count", available), ("proposed_tool_count", proposed)):
        if not isinstance(count, int) or isinstance(count, bool) or not 0 <= count <= MAX_ACTIVATION_TOOLS:
            raise AutonomousActivationError(f"coverage {name} is outside its bound")
    missing_tools = _string_tuple(f"coverage {domain} missing_tools", value.get("missing_tools", ()), maximum=MAX_ACTIVATION_TOOLS)
    missing_capabilities = _string_tuple(
        f"coverage {domain} missing_capabilities", value.get("missing_capabilities", ()), maximum=MAX_ACTIVATION_TOOLS
    )
    if proposed == 0:
        status = "unavailable"
    elif proposed < required:
        status = "partial"
    else:
        status = "available"
    ratios: dict[str, float] = {}
    for name in ("coverage_ratio", "approved_coverage_ratio"):
        ratio = value.get(name, 0.0)
        if isinstance(ratio, bool) or not isinstance(ratio, (int, float)) or not math.isfinite(float(ratio)) or not 0 <= float(ratio) <= 1:
            raise AutonomousActivationError(f"coverage {name} must be within [0, 1]")
        ratios[name] = float(ratio)
    return {
        "domain": _identifier("activation domain", domain),
        "required_tool_count": required,
        "available_tool_count": available,
        "proposed_tool_count": proposed,
        "missing_tools": list(missing_tools),
        "missing_capabilities": list(missing_capabilities),
        "coverage_ratio": ratios["coverage_ratio"],
        "approved_coverage_ratio": ratios["approved_coverage_ratio"],
        "status": status,
    }


def _binding_plan_digest(plan: Mapping[str, Any]) -> str:
    """Digest only the policy-bearing portion of a plan, excluding presentation metadata."""

    if not isinstance(plan, Mapping):
        raise AutonomousActivationError("binding plan must be a mapping")
    proposed = plan.get("proposed_bindings", {})
    review_bindings = plan.get("review_required_bindings", {})
    if not isinstance(proposed, Mapping) or not isinstance(review_bindings, Mapping):
        raise AutonomousActivationError("binding plan binding rows are malformed")

    def row_projection(name: Any, row: Any) -> dict[str, Any]:
        if not isinstance(name, str) or not isinstance(row, Mapping):
            raise AutonomousActivationError("binding plan contains a malformed binding row")
        return {
            "name": row.get("name", name),
            "domains": row.get("domains", ()),
            "capability": row.get("capability"),
            "risk_class": row.get("risk_class"),
            "read_only": row.get("read_only"),
            "approval_required": row.get("approval_required"),
            "live_schema_digest": row.get("live_schema_digest"),
            "catalogue_digest": row.get("catalogue_digest"),
        }

    projection = {
        "schema": plan.get("schema"),
        "catalogue_digest": plan.get("catalogue_digest"),
        "profile_digest": plan.get("profile_digest"),
        "domains": plan.get("domains", ()),
        "available_curated_tools": plan.get("available_curated_tools", ()),
        "missing_curated_tools": plan.get("missing_curated_tools", ()),
        "review_required_tools": plan.get("review_required_tools", ()),
        "unclassified_tools": plan.get("unclassified_tools", ()),
        "coverage": plan.get("coverage", {}),
        "proposed_bindings": {
            name: row_projection(name, row) for name, row in sorted(proposed.items())
        },
        "review_required_bindings": {
            name: row_projection(name, row) for name, row in sorted(review_bindings.items())
        },
    }
    return content_digest(_safe_mapping("binding plan digest payload", projection, maximum=MAX_ACTIVATION_STATE_BYTES))


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityActivationState:
    """Redacted, restart-safe snapshot of provider and domain capability activation."""

    activation_id: str
    status: str = "created"
    revision: int = 0
    created_at: float = 0.0
    updated_at: float = 0.0
    catalogue_digest: str | None = None
    plan_digest: str | None = None
    profile_digest: str | None = None
    approved_tools: tuple[str, ...] = ()
    pending_review_tools: tuple[str, ...] = ()
    unclassified_tools: tuple[str, ...] = ()
    provider_statuses: tuple[Mapping[str, Any], ...] = ()
    domain_statuses: tuple[Mapping[str, Any], ...] = ()
    registered_tool_count: int = 0
    last_error: str | None = None

    def __post_init__(self) -> None:
        _identifier("activation_id", self.activation_id, maximum=256)
        if self.status not in AUTONOMOUS_ACTIVATION_STATUSES:
            raise AutonomousActivationError("activation status is unsupported")
        if not isinstance(self.revision, int) or isinstance(self.revision, bool) or not 0 <= self.revision <= 1_000_000:
            raise AutonomousActivationError("activation revision is outside its bound")
        _finite_time("activation created_at", self.created_at)
        _finite_time("activation updated_at", self.updated_at)
        if self.updated_at < self.created_at:
            raise AutonomousActivationError("activation updated_at cannot precede created_at")
        for name, value in (("catalogue_digest", self.catalogue_digest), ("plan_digest", self.plan_digest), ("profile_digest", self.profile_digest)):
            if value is not None:
                _digest(f"activation {name}", value)
        for name, value in (
            ("approved_tools", self.approved_tools),
            ("pending_review_tools", self.pending_review_tools),
            ("unclassified_tools", self.unclassified_tools),
        ):
            _string_tuple(f"activation {name}", value, maximum=MAX_ACTIVATION_TOOLS)
        if not isinstance(self.provider_statuses, Sequence) or isinstance(self.provider_statuses, (str, bytes)) or len(self.provider_statuses) > MAX_ACTIVATION_PROVIDERS:
            raise AutonomousActivationError("activation provider statuses exceed their bound")
        if not isinstance(self.domain_statuses, Sequence) or isinstance(self.domain_statuses, (str, bytes)) or len(self.domain_statuses) > MAX_ACTIVATION_DOMAINS:
            raise AutonomousActivationError("activation domain statuses exceed their bound")
        for row in (*self.provider_statuses, *self.domain_statuses):
            _safe_mapping("activation status row", row)
        if not isinstance(self.registered_tool_count, int) or isinstance(self.registered_tool_count, bool) or not 0 <= self.registered_tool_count <= MAX_ACTIVATION_TOOLS:
            raise AutonomousActivationError("activation registered_tool_count is outside its bound")
        if self.last_error is not None:
            _text("activation last_error", self.last_error, maximum=MAX_ACTIVATION_ERROR_BYTES)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_ACTIVATION_SCHEMA,
            "activation_id": self.activation_id,
            "status": self.status,
            "revision": self.revision,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "catalogue_digest": self.catalogue_digest,
            "plan_digest": self.plan_digest,
            "profile_digest": self.profile_digest,
            "approved_tools": list(self.approved_tools),
            "pending_review_tools": list(self.pending_review_tools),
            "unclassified_tools": list(self.unclassified_tools),
            "provider_statuses": [dict(row) for row in self.provider_statuses],
            "domain_statuses": [dict(row) for row in self.domain_statuses],
            "registered_tool_count": self.registered_tool_count,
            "last_error": self.last_error,
        }

    @property
    def state_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        result = self._payload()
        result.update(
            {
                "state_digest": self.state_digest,
                "retention": "metadata_only_no_keys_handles_prompts_tasks_or_payloads",
                "authorization": "status_only; does_not_grant_provider_or_tool_authority",
                "secret_material": "never_returned",
            }
        )
        return result

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousCapabilityActivationState":
        if not isinstance(value, Mapping):
            raise AutonomousActivationError("activation state must be a mapping")
        if value.get("schema") not in (None, AUTONOMOUS_ACTIVATION_SCHEMA):
            raise AutonomousActivationError("activation state schema is unsupported")
        allowed = set(cls(activation_id="placeholder")._payload()) | {"schema", "state_digest", "retention", "authorization", "secret_material"}
        unknown = set(value).difference(allowed)
        if unknown:
            raise AutonomousActivationError("activation state contains unsupported fields")
        state = cls(
            activation_id=value.get("activation_id"),
            status=value.get("status", "created"),
            revision=value.get("revision", 0),
            created_at=value.get("created_at", 0.0),
            updated_at=value.get("updated_at", 0.0),
            catalogue_digest=value.get("catalogue_digest"),
            plan_digest=value.get("plan_digest"),
            profile_digest=value.get("profile_digest"),
            approved_tools=tuple(value.get("approved_tools", ())),
            pending_review_tools=tuple(value.get("pending_review_tools", ())),
            unclassified_tools=tuple(value.get("unclassified_tools", ())),
            provider_statuses=tuple(value.get("provider_statuses", ())),
            domain_statuses=tuple(value.get("domain_statuses", ())),
            registered_tool_count=value.get("registered_tool_count", 0),
            last_error=value.get("last_error"),
        )
        supplied_digest = value.get("state_digest")
        if supplied_digest is not None and supplied_digest != state.state_digest:
            raise AutonomousActivationError("activation state digest does not match its contents")
        return state


class AutonomousCapabilityActivation:
    """Thread-safe state machine for the redacted onboarding/activation lifecycle."""

    def __init__(
        self,
        activation_id: str | None = None,
        *,
        state: AutonomousCapabilityActivationState | Mapping[str, Any] | None = None,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not callable(clock):
            raise AutonomousActivationError("activation clock must be callable")
        self._clock = clock
        self._lock = threading.RLock()
        if state is not None:
            self._state = state if isinstance(state, AutonomousCapabilityActivationState) else AutonomousCapabilityActivationState.from_mapping(state)
            return
        now = _finite_time("activation clock", clock())
        self._state = AutonomousCapabilityActivationState(
            activation_id=activation_id or f"activation-{uuid.uuid4().hex}",
            created_at=now,
            updated_at=now,
        )

    @property
    def state(self) -> AutonomousCapabilityActivationState:
        with self._lock:
            return self._state

    def to_dict(self) -> dict[str, Any]:
        return self.state.to_dict()

    def record_provider_statuses(self, statuses: Sequence[Mapping[str, Any]]) -> AutonomousCapabilityActivationState:
        if not isinstance(statuses, Sequence) or isinstance(statuses, (str, bytes)):
            raise AutonomousActivationError("provider statuses must be a sequence")
        if len(statuses) > MAX_ACTIVATION_PROVIDERS:
            raise AutonomousActivationError("provider statuses exceed their bound")
        projected = tuple(sorted((_provider_projection(row) for row in statuses), key=lambda row: row["provider"]))
        if len({row["provider"] for row in projected}) != len(projected):
            raise AutonomousActivationError("provider statuses contain duplicate providers")
        with self._lock:
            self._ensure_not_revoked()
            self._commit(provider_statuses=projected, last_error=None)
            return self._state

    def record_binding_plan(self, plan: Mapping[str, Any]) -> AutonomousCapabilityActivationState:
        if not isinstance(plan, Mapping) or plan.get("schema") != DOMAIN_TOOL_BINDING_PLAN_SCHEMA:
            raise AutonomousActivationError("activation requires a valid domain tool binding plan")
        catalogue_digest = _digest("binding plan catalogue_digest", plan.get("catalogue_digest"))
        profile_digest = _digest("binding plan profile_digest", plan.get("profile_digest"))
        plan_digest = _binding_plan_digest(plan)
        raw_domains = plan.get("domains")
        if not isinstance(raw_domains, Sequence) or isinstance(raw_domains, (str, bytes)):
            raise AutonomousActivationError("binding plan domains are missing")
        domains = _string_tuple("binding plan domains", raw_domains, maximum=MAX_ACTIVATION_DOMAINS)
        unknown_domains = set(domains).difference(AUTONOMOUS_DOMAIN_NAMES)
        if unknown_domains:
            raise AutonomousActivationError("binding plan contains unknown domains")
        raw_coverage = plan.get("coverage")
        if not isinstance(raw_coverage, Mapping):
            raise AutonomousActivationError("binding plan coverage is missing")
        coverage = tuple(_coverage_projection(domain, raw_coverage.get(domain, {})) for domain in domains)
        review = _string_tuple("binding plan review_required_tools", plan.get("review_required_tools", ()), maximum=MAX_ACTIVATION_TOOLS)
        unclassified = _string_tuple("binding plan unclassified_tools", plan.get("unclassified_tools", ()), maximum=MAX_ACTIVATION_TOOLS)
        changed = self.state.catalogue_digest is not None and self.state.catalogue_digest != catalogue_digest
        with self._lock:
            self._ensure_not_revoked()
            invalidated_approved = changed and bool(self._state.approved_tools)
            self._commit(
                status="stale" if invalidated_approved else None,
                catalogue_digest=catalogue_digest,
                plan_digest=plan_digest,
                profile_digest=profile_digest,
                approved_tools=() if changed else self._state.approved_tools,
                pending_review_tools=tuple(sorted(set(review).union(unclassified))),
                unclassified_tools=unclassified,
                domain_statuses=coverage,
                registered_tool_count=0 if changed else self._state.registered_tool_count,
                last_error=None,
            )
            if not invalidated_approved:
                self._commit(status=self._derived_status(preserve_stale=False))
            return self._state

    def record_registered_tools(self, count: int) -> AutonomousCapabilityActivationState:
        if not isinstance(count, int) or isinstance(count, bool) or not 0 <= count <= MAX_ACTIVATION_TOOLS:
            raise AutonomousActivationError("registered tool count is outside its bound")
        with self._lock:
            self._ensure_not_revoked()
            self._commit(registered_tool_count=count, status=self._derived_status())
            return self._state

    def approve_bindings(
        self,
        plan: Mapping[str, Any],
        approved_tools: Sequence[str],
        *,
        registered_tool_count: int | None = None,
    ) -> AutonomousCapabilityActivationState:
        if not isinstance(approved_tools, Sequence) or isinstance(approved_tools, (str, bytes)) or not approved_tools:
            raise AutonomousActivationError("approved_tools must be a non-empty sequence")
        approved = _string_tuple("approved_tools", approved_tools, maximum=MAX_ACTIVATION_TOOLS)
        if not isinstance(plan, Mapping):
            raise AutonomousActivationError("approval requires a binding plan")
        with self._lock:
            self._ensure_not_revoked()
            expected_plan_digest = _binding_plan_digest(plan)
            if self._state.plan_digest != expected_plan_digest:
                raise AutonomousActivationError("approved binding plan does not match the recorded plan")
            proposed = plan.get("proposed_bindings")
            if not isinstance(proposed, Mapping) or not set(approved).issubset(proposed):
                raise AutonomousActivationError("approved tools must be present in proposed_bindings")
            count = self._state.registered_tool_count if registered_tool_count is None else registered_tool_count
            if not isinstance(count, int) or isinstance(count, bool) or not 0 <= count <= MAX_ACTIVATION_TOOLS:
                raise AutonomousActivationError("registered tool count is outside its bound")
            self._commit(approved_tools=approved, registered_tool_count=count, last_error=None)
            self._commit(status=self._derived_status(preserve_stale=False))
            return self._state

    def revoke(self, *, reason: str = "activation_revoked") -> AutonomousCapabilityActivationState:
        reason = _text("activation revocation reason", reason, maximum=MAX_ACTIVATION_ERROR_BYTES)
        with self._lock:
            if self._state.status == "revoked":
                return self._state
            self._commit(status="revoked", approved_tools=(), last_error=reason)
            return self._state

    def _derived_status(self, *, preserve_stale: bool = True) -> str:
        state = self._state
        if state.status == "revoked" or (preserve_stale and state.status == "stale"):
            return state.status
        if not state.provider_statuses or not any(bool(row.get("ready")) for row in state.provider_statuses):
            return "provider_pending"
        if state.plan_digest is None:
            return "catalogue_pending"
        if not state.approved_tools:
            return "review_required"
        if state.pending_review_tools:
            return "partially_activated"
        return "ready"

    def _commit(self, **changes: Any) -> None:
        now = _finite_time("activation clock", self._clock())
        changes = {
            key: value
            for key, value in changes.items()
            if not (key == "status" and value is None)
        }
        if all(getattr(self._state, key) == value for key, value in changes.items()):
            return
        next_status = changes.get("status", self._state.status)
        if next_status not in _ALLOWED_ACTIVATION_TRANSITIONS[self._state.status]:
            raise AutonomousActivationError(
                f"activation transition {self._state.status!r} -> {next_status!r} is not allowed"
            )
        changes["revision"] = self._state.revision + 1
        changes["updated_at"] = max(now, self._state.updated_at)
        self._state = replace(self._state, **changes)

    def _ensure_not_revoked(self) -> None:
        if self._state.status == "revoked":
            raise AutonomousActivationError("activation is revoked")


class AutonomousCapabilityActivationStore:
    """Atomic JSON snapshot store for redacted activation state."""

    def __init__(self, path: str | os.PathLike[str], *, max_bytes: int = MAX_ACTIVATION_STORE_BYTES) -> None:
        if not isinstance(path, (str, os.PathLike)) or not str(path):
            raise AutonomousActivationError("activation store path must be non-empty")
        if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes <= 0 or max_bytes > MAX_ACTIVATION_STORE_BYTES:
            raise AutonomousActivationError("activation store max_bytes is outside its bound")
        self.path = Path(path)
        self.max_bytes = max_bytes
        self._lock = threading.RLock()

    def save(self, value: AutonomousCapabilityActivation | AutonomousCapabilityActivationState | Mapping[str, Any]) -> dict[str, Any]:
        if isinstance(value, AutonomousCapabilityActivation):
            state = value.state
        elif isinstance(value, AutonomousCapabilityActivationState):
            state = value
        else:
            state = AutonomousCapabilityActivationState.from_mapping(value)
        envelope = {
            "schema": AUTONOMOUS_ACTIVATION_STORE_SCHEMA,
            "state": state.to_dict(),
            "state_digest": state.state_digest,
        }
        encoded = canonical_bytes(envelope)
        if len(encoded) > self.max_bytes:
            raise AutonomousActivationError("activation store snapshot exceeds its bound")
        with self._lock:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            descriptor, temporary = tempfile.mkstemp(prefix=f".{self.path.name}.", dir=str(self.path.parent))
            try:
                with os.fdopen(descriptor, "wb") as handle:
                    handle.write(encoded)
                    handle.flush()
                    os.fsync(handle.fileno())
                os.replace(temporary, self.path)
            except Exception:
                try:
                    os.unlink(temporary)
                except FileNotFoundError:
                    pass
                raise
        return {"schema": AUTONOMOUS_ACTIVATION_STORE_SCHEMA, "state_digest": state.state_digest, "bytes": len(encoded)}

    def save_if_unchanged(
        self,
        value: AutonomousCapabilityActivation | AutonomousCapabilityActivationState | Mapping[str, Any],
        expected_state_digest: str | None,
    ) -> bool:
        """Atomically persist activation only when the expected state is still current.

        ``None`` means create-if-absent. This is the safe handoff for separate approval and
        revocation workers that share one activation file.
        """

        if expected_state_digest is not None and not _valid_digest(expected_state_digest):
            raise AutonomousActivationError("activation expected_state_digest is invalid")
        with self._lock:
            current = self.load()
            observed = None if current is None else current.state_digest
            if observed != expected_state_digest:
                return False
            self.save(value)
            return True

    def load(self) -> AutonomousCapabilityActivationState | None:
        with self._lock:
            if not self.path.exists():
                return None
            if self.path.stat().st_size > self.max_bytes:
                raise AutonomousActivationError("activation store exceeds its bound")
            try:
                encoded = self.path.read_bytes()
                envelope = json.loads(encoded.decode("utf-8"))
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise AutonomousActivationError("activation store contains invalid JSON") from error
        if canonical_bytes(envelope) != encoded:
            raise AutonomousActivationError("activation store JSON is not canonical")
        if not isinstance(envelope, Mapping) or envelope.get("schema") != AUTONOMOUS_ACTIVATION_STORE_SCHEMA:
            raise AutonomousActivationError("activation store schema is invalid")
        state = AutonomousCapabilityActivationState.from_mapping(envelope.get("state"))
        if envelope.get("state_digest") != state.state_digest:
            raise AutonomousActivationError("activation store state digest is invalid")
        return state


__all__ = [
    "AUTONOMOUS_ACTIVATION_SCHEMA",
    "AUTONOMOUS_ACTIVATION_STATUSES",
    "AUTONOMOUS_ACTIVATION_STORE_SCHEMA",
    "MAX_ACTIVATION_DOMAINS",
    "MAX_ACTIVATION_PROVIDERS",
    "MAX_ACTIVATION_STATE_BYTES",
    "MAX_ACTIVATION_STORE_BYTES",
    "MAX_ACTIVATION_TOOLS",
    "AutonomousActivationError",
    "AutonomousCapabilityActivation",
    "AutonomousCapabilityActivationState",
    "AutonomousCapabilityActivationStore",
]
