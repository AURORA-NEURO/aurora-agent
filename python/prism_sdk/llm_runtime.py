"""User-supplied LLM credentials and provider invocation.

The MCP/Rust brain deliberately never accepts a provider secret. Applications use this module as
the runtime boundary:

1. collect a key from a UI, a no-echo prompt, an environment variable, or an external secret
   manager;
2. register it in an in-memory :class:`CredentialStore` and receive an opaque handle;
3. invoke a configured provider with the handle; and
4. revoke or discard the store when the session ends.

The handle is metadata-only and cannot be JSON serialized into a brain plan. The store is
intentionally not persistent. Production applications that need persistence should keep only an
external secret-manager reference and recreate a short-lived handle at process startup.

The OpenAI adapter uses the Responses API shape (``POST /v1/responses`` with ``model`` and
``input``) and Bearer authentication. The transport is standard-library-only so the SDK remains
dependency-free. Anthropic Messages and OpenAI-compatible Chat Completions are supported through
the same explicit provider contract.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
import getpass
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import secrets
import threading
import time
from typing import Any, Callable, Iterator, Mapping, Protocol, Sequence
from urllib.parse import urlsplit


MAX_MESSAGES = 512
MAX_MESSAGE_CHARS = 2_000_000
MAX_RESPONSE_BYTES = 20_000_000
MAX_PROVIDER_TOOLS = 128
MAX_TOOL_NAME_BYTES = 256
MAX_TOOL_ARGUMENT_BYTES = 1_000_000
MAX_STREAM_EVENTS = 100_000
MAX_STREAM_EVENT_BYTES = 2_000_000
MAX_STREAM_TEXT_BYTES = MAX_MESSAGE_CHARS
SUPPORTED_PROTOCOLS = {
    "openai_responses",
    "openai_chat_completions",
    "anthropic_messages",
}
PROVIDER_OBSERVATION_SCHEMA = "bioprism-llm-provider-observation/0.1"
MODEL_CATALOGUE_SCHEMA = "bioprism-llm-model-catalogue/0.1"
PROVIDER_HEALTH_LEDGER_SCHEMA = "bioprism-llm-provider-health-ledger/0.1"
MAX_MODEL_CANDIDATES = 512
MAX_MODEL_METADATA_BYTES = 256_000
MAX_PROVIDER_HEALTH_RECORDS = 16_384
MAX_PROVIDER_HEALTH_BYTES = 32_000_000
_MODEL_CANDIDATE_FIELDS = frozenset(
    {
        "provider",
        "model",
        "context_window_tokens",
        "max_output_tokens",
        "quality",
        "latency_ms",
        "cost_per_million_tokens",
        "reliability",
        "capabilities",
        "requires_credential",
        "enabled",
        "metadata",
    }
)
_MODEL_SECRET_METADATA_KEYS = frozenset(
    {
        "api_key",
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "password",
        "secret",
        "token",
    }
)


class CredentialError(ValueError):
    """A credential was missing, invalid, expired, revoked, or used with the wrong provider."""


class ProviderError(RuntimeError):
    """A provider call failed without retaining or exposing the credential."""

    def __init__(
        self,
        message: str,
        *,
        retryable: bool = False,
        status_code: int | None = None,
        circuit_open: bool = False,
    ) -> None:
        super().__init__(message)
        self.retryable = retryable
        self.status_code = status_code
        self.circuit_open = circuit_open


@dataclass(frozen=True, slots=True)
class ProviderInvocationMetadata:
    """Value-only metadata for one provider request boundary.

    Observers receive token estimates and request shape, never the messages, headers, credential
    handle, response text, or provider wire payload.  The estimate is deliberately conservative
    and is useful for admission budgets; authoritative usage, when returned by a provider, is
    supplied separately to :class:`ProviderInvocationObserver`.
    """

    provider: str
    model: str
    kind: str
    input_tokens: int
    requested_output_tokens: int
    tool_count: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "model": self.model,
            "kind": self.kind,
            "input_tokens": self.input_tokens,
            "requested_output_tokens": self.requested_output_tokens,
            "tool_count": self.tool_count,
            "retention": "metadata_only_no_provider_payloads",
        }


class ProviderInvocationObserver(Protocol):
    """Optional per-request admission/outcome hook for autonomous execution accounting."""

    def before(self, metadata: ProviderInvocationMetadata) -> None:
        ...

    def after(
        self,
        metadata: ProviderInvocationMetadata,
        response: "ProviderResponse | None",
        error: BaseException | None,
        latency_ms: float,
    ) -> None:
        ...


class SecretValue:
    """A non-serializable secret wrapper whose display forms are always redacted."""

    __slots__ = ("_value",)

    def __init__(self, value: str) -> None:
        if not isinstance(value, str) or not value:
            raise CredentialError("credential value must be a non-empty string")
        self._value = value

    def expose(self) -> str:
        """Expose the value only at the HTTP-header construction boundary."""

        return self._value

    def __repr__(self) -> str:
        return "SecretValue(<redacted>)"

    def __str__(self) -> str:
        return "<redacted>"


@dataclass(frozen=True, slots=True)
class _CredentialEntry:
    provider: str
    secret: SecretValue
    expires_at: float | None
    source: str


@dataclass(frozen=True, slots=True)
class CredentialStatus:
    """Redacted readiness projection for one user-owned provider credential set."""

    provider: str
    configured: bool
    credential_count: int
    credentials: tuple[Mapping[str, Any], ...]
    secret_persistence: str = "in_memory_only"

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "configured": self.configured,
            "credential_count": self.credential_count,
            "credentials": [dict(item) for item in self.credentials],
            "secret_persistence": self.secret_persistence,
            "secret_material": "never_returned",
        }


class CredentialHandle:
    """Opaque capability for one provider credential.

    The underlying store is held by identity and the secret is never an attribute of this handle.
    ``to_metadata`` is the only supported serialization path and returns no secret material.
    """

    __slots__ = ("provider", "credential_id", "_store")

    def __init__(self, provider: str, credential_id: str, store: "CredentialStore") -> None:
        self.provider = provider
        self.credential_id = credential_id
        self._store = store

    def to_metadata(
        self,
        *,
        source: str = "unknown",
        expires_at: float | None = None,
    ) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "credential_id": self.credential_id,
            "credential_present": True,
            "secret_persistence": "in_memory_only",
            "source": source,
            "expires_at": expires_at,
        }

    def __repr__(self) -> str:
        return f"CredentialHandle(provider={self.provider!r}, credential_id='<redacted>')"

    def __str__(self) -> str:
        return "<credential-handle redacted>"


class CredentialStore:
    """Thread-safe, in-memory user credential store.

    ``register`` is the normal UI integration point. ``prompt`` uses :mod:`getpass`, so a CLI
    user can enter a key without terminal echo. Neither method writes the value to disk, an
    environment variable, a log, a brain plan, or an MCP argument.
    """

    def __init__(self, *, max_credentials: int = 32, clock: Callable[[], float] = time.time) -> None:
        if max_credentials <= 0:
            raise CredentialError("max_credentials must be positive")
        self._max_credentials = max_credentials
        self._clock = clock
        self._entries: dict[str, _CredentialEntry] = {}
        self._lock = threading.RLock()

    _SOURCES = frozenset({"direct", "prompt", "environment", "external_resolver"})

    def register(
        self,
        provider: str,
        secret: str,
        *,
        ttl_seconds: float | None = None,
        source: str = "direct",
    ) -> CredentialHandle:
        self._validate_provider(provider)
        if not isinstance(secret, str) or not secret.strip():
            raise CredentialError("credential value must be a non-empty string")
        if ttl_seconds is not None and (not isinstance(ttl_seconds, (int, float)) or ttl_seconds <= 0):
            raise CredentialError("ttl_seconds must be positive or None")
        if not isinstance(source, str) or source not in self._SOURCES:
            raise CredentialError("credential source is not supported")
        with self._lock:
            self._purge_expired_locked()
            if len(self._entries) >= self._max_credentials:
                raise CredentialError("credential store capacity is exhausted")
            credential_id = secrets.token_urlsafe(24)
            while credential_id in self._entries:
                credential_id = secrets.token_urlsafe(24)
            expires_at = None if ttl_seconds is None else self._clock() + float(ttl_seconds)
            self._entries[credential_id] = _CredentialEntry(
                provider=provider,
                secret=SecretValue(secret),
                expires_at=expires_at,
                source=source,
            )
            return CredentialHandle(provider, credential_id, self)

    def register_environment(
        self,
        provider: str,
        variable: str,
        *,
        ttl_seconds: float | None = None,
        environ: Mapping[str, str] | None = None,
    ) -> CredentialHandle:
        """Load a named environment variable without including its value in any metadata."""

        if not isinstance(variable, str) or not variable or not variable.replace("_", "").isalnum():
            raise CredentialError("environment variable name must be alphanumeric with underscores")
        source = os.environ if environ is None else environ
        value = source.get(variable)
        if value is None:
            raise CredentialError(f"environment variable {variable!r} is not set")
        return self.register(provider, value, ttl_seconds=ttl_seconds, source="environment")

    def prompt(
        self,
        provider: str,
        *,
        prompt: str = "Provider API key: ",
        ttl_seconds: float | None = None,
        reader: Callable[[str], str] | None = None,
    ) -> CredentialHandle:
        """Collect a key without echo by default; ``reader`` makes UI/tests injectable."""

        value = (getpass.getpass(prompt) if reader is None else reader(prompt))
        return self.register(provider, value, ttl_seconds=ttl_seconds, source="prompt")

    def register_resolver(
        self,
        provider: str,
        reference: str,
        resolver: Callable[[str], str],
        *,
        ttl_seconds: float | None = None,
    ) -> CredentialHandle:
        """Resolve one external secret-manager reference for this short-lived process.

        Only the resolver sees the reference and value. The store retains the resulting secret in
        memory for the handle lifetime and deliberately does not retain the reference, so a ledger,
        route, prompt, or provider response cannot accidentally become a secret-manager index.
        """

        self._validate_provider(provider)
        if not isinstance(reference, str) or not reference.strip() or len(reference) > 512:
            raise CredentialError("external credential reference must be a bounded non-empty string")
        if any(ord(character) < 32 for character in reference):
            raise CredentialError("external credential reference contains a control character")
        if not callable(resolver):
            raise CredentialError("external credential resolver must be callable")
        try:
            value = resolver(reference)
        except Exception as error:  # pragma: no cover - defensive boundary for foreign resolvers
            raise CredentialError("external credential resolver failed") from error
        return self.register(
            provider,
            value,
            ttl_seconds=ttl_seconds,
            source="external_resolver",
        )

    def revoke(self, handle: CredentialHandle) -> None:
        self._assert_handle(handle)
        with self._lock:
            self._entries.pop(handle.credential_id, None)

    def clear(self) -> None:
        with self._lock:
            self._entries.clear()

    def metadata(self, handle: CredentialHandle) -> dict[str, Any]:
        entry = self._resolve_entry(handle)
        return handle.to_metadata(source=entry.source, expires_at=entry.expires_at)

    def status(self, provider: str) -> CredentialStatus:
        """Return readiness metadata without returning handles or secret material."""

        self._validate_provider(provider)
        with self._lock:
            self._purge_expired_locked()
            entries = [
                (credential_id, entry)
                for credential_id, entry in self._entries.items()
                if entry.provider == provider
            ]
        credentials = tuple(
            {
                "credential_id": credential_id,
                "source": entry.source,
                "expires_at": entry.expires_at,
                "secret_persistence": "in_memory_only",
            }
            for credential_id, entry in sorted(entries)
        )
        return CredentialStatus(
            provider=provider,
            configured=bool(credentials),
            credential_count=len(credentials),
            credentials=credentials,
        )

    def statuses(self) -> list[CredentialStatus]:
        """Return deterministic redacted status for every provider currently in the store."""

        with self._lock:
            self._purge_expired_locked()
            providers = sorted({entry.provider for entry in self._entries.values()})
        return [self.status(provider) for provider in providers]

    def _assert_handle(self, handle: CredentialHandle) -> None:
        if not isinstance(handle, CredentialHandle) or handle._store is not self:
            raise CredentialError("credential handle belongs to a different store")

    def _resolve(self, handle: CredentialHandle) -> SecretValue:
        return self._resolve_entry(handle).secret

    def _resolve_entry(self, handle: CredentialHandle) -> _CredentialEntry:
        self._assert_handle(handle)
        with self._lock:
            self._purge_expired_locked()
            entry = self._entries.get(handle.credential_id)
            if entry is None:
                raise CredentialError("credential handle is unknown, revoked, or expired")
            if entry.provider != handle.provider:
                raise CredentialError("credential handle provider mismatch")
            return entry

    def _purge_expired_locked(self) -> None:
        now = self._clock()
        expired = [
            identifier
            for identifier, entry in self._entries.items()
            if entry.expires_at is not None and now >= entry.expires_at
        ]
        for identifier in expired:
            self._entries.pop(identifier, None)

    @staticmethod
    def _validate_provider(provider: str) -> None:
        if not isinstance(provider, str) or not provider.strip() or "/" in provider or " " in provider:
            raise CredentialError("provider must be a non-empty path-safe identifier")


@dataclass(frozen=True, slots=True)
class ProviderConfig:
    """Non-secret transport and protocol metadata for one provider."""

    provider: str
    base_url: str
    protocol: str = "openai_responses"
    path: str | None = None
    requires_credential: bool = True
    timeout_seconds: float = 60.0
    max_response_bytes: int = MAX_RESPONSE_BYTES
    allow_insecure_http: bool = False
    api_key_header: str | None = None
    max_attempts: int = 1
    retry_backoff_seconds: float = 0.0
    circuit_breaker_failure_threshold: int = 3
    circuit_breaker_reset_seconds: float = 30.0

    def __post_init__(self) -> None:
        if not self.provider or "/" in self.provider or " " in self.provider:
            raise ProviderError("provider must be a non-empty path-safe identifier")
        if self.protocol not in SUPPORTED_PROTOCOLS:
            raise ProviderError(f"unsupported provider protocol {self.protocol!r}")
        parsed = urlsplit(self.base_url)
        if parsed.scheme not in {"http", "https"} or not parsed.hostname:
            raise ProviderError("base_url must be an absolute http(s) URL")
        if parsed.username or parsed.password:
            raise ProviderError("base_url must not contain embedded credentials")
        if self.timeout_seconds <= 0 or self.max_response_bytes <= 0:
            raise ProviderError("provider timeout and response bound must be positive")
        if not 1 <= self.max_attempts <= 8:
            raise ProviderError("max_attempts must be within [1, 8]")
        if not 0 <= self.retry_backoff_seconds <= 60:
            raise ProviderError("retry_backoff_seconds must be within [0, 60]")
        if not 1 <= self.circuit_breaker_failure_threshold <= 100:
            raise ProviderError("circuit_breaker_failure_threshold must be within [1, 100]")
        if self.circuit_breaker_reset_seconds <= 0:
            raise ProviderError("circuit_breaker_reset_seconds must be positive")
        if parsed.scheme == "http" and not self.allow_insecure_http:
            raise ProviderError("plain HTTP requires allow_insecure_http=True for local/test use")

    @property
    def endpoint(self) -> tuple[str, int | None, str, str]:
        parsed = urlsplit(self.base_url)
        default_port = 443 if parsed.scheme == "https" else 80
        prefix = parsed.path.rstrip("/")
        path = self.path or {
            "openai_responses": "/v1/responses",
            "openai_chat_completions": "/v1/chat/completions",
            "anthropic_messages": "/v1/messages",
        }[self.protocol]
        if not path.startswith("/"):
            path = "/" + path
        return parsed.hostname or "", parsed.port or default_port, prefix + path, parsed.scheme

    def to_metadata(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "base_url": self.base_url,
            "protocol": self.protocol,
            "path": self.endpoint[2],
            "requires_credential": self.requires_credential,
            "credential_transport": "caller_supplied_in_memory_handle",
            "secret_logging": "redacted",
            "max_attempts": self.max_attempts,
            "retry_backoff_seconds": self.retry_backoff_seconds,
            "circuit_breaker_failure_threshold": self.circuit_breaker_failure_threshold,
            "circuit_breaker_reset_seconds": self.circuit_breaker_reset_seconds,
        }


@dataclass(frozen=True, slots=True)
class ModelCandidate:
    """Typed, non-secret metadata for one selectable provider model.

    A candidate is a routing prior, not a claim that the model is available, safe, or suitable
    for a particular decision.  Runtime registration and credential readiness are evaluated by
    :class:`LLMRuntime`/``AutonomousBrain`` at selection time.  The catalogue intentionally
    contains no key, endpoint credential, prompt, or provider response.
    """

    provider: str
    model: str
    context_window_tokens: int
    max_output_tokens: int
    quality: float
    latency_ms: int
    cost_per_million_tokens: int
    reliability: float = 0.5
    capabilities: tuple[str, ...] = ()
    requires_credential: bool = True
    enabled: bool = True
    metadata: Mapping[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if (
            not isinstance(self.provider, str)
            or not self.provider.strip()
            or "/" in self.provider
            or " " in self.provider
        ):
            raise ProviderError("model candidate provider must be a path-safe identifier")
        if not isinstance(self.model, str) or not self.model.strip() or len(self.model.encode("utf-8")) > 512:
            raise ProviderError("model candidate model must be a bounded non-empty string")
        for name, value in (
            ("context_window_tokens", self.context_window_tokens),
            ("max_output_tokens", self.max_output_tokens),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                raise ProviderError(f"model candidate {name} must be a positive integer")
        for name, value in (
            ("latency_ms", self.latency_ms),
            ("cost_per_million_tokens", self.cost_per_million_tokens),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                raise ProviderError(f"model candidate {name} must be a non-negative integer")
        if self.max_output_tokens > self.context_window_tokens:
            raise ProviderError("model candidate max_output_tokens cannot exceed its context window")
        for name, value in (("quality", self.quality), ("reliability", self.reliability)):
            if (
                isinstance(value, bool)
                or not isinstance(value, (int, float))
                or not math.isfinite(float(value))
                or not 0 <= float(value) <= 1
            ):
                raise ProviderError(f"model candidate {name} must be within [0, 1]")
        if not isinstance(self.capabilities, Sequence) or isinstance(self.capabilities, (str, bytes)):
            raise ProviderError("model candidate capabilities must be a string sequence")
        capabilities: list[str] = []
        for capability in self.capabilities:
            if (
                not isinstance(capability, str)
                or not capability.strip()
                or len(capability.encode("utf-8")) > 128
                or any(ord(character) < 32 for character in capability)
            ):
                raise ProviderError("model candidate capabilities must contain bounded strings")
            if capability not in capabilities:
                capabilities.append(capability)
        object.__setattr__(self, "capabilities", tuple(capabilities))
        if not isinstance(self.requires_credential, bool) or not isinstance(self.enabled, bool):
            raise ProviderError("model candidate availability flags must be booleans")
        if not isinstance(self.metadata, Mapping):
            raise ProviderError("model candidate metadata must be an object")
        for key in self.metadata:
            if not isinstance(key, str) or not key.strip() or len(key.encode("utf-8")) > 128:
                raise ProviderError("model candidate metadata keys must be bounded strings")
            normalized_key = key.lower().replace("-", "_")
            if normalized_key in _MODEL_CANDIDATE_FIELDS:
                raise ProviderError(f"model candidate metadata cannot override field {key!r}")
            if normalized_key in _MODEL_SECRET_METADATA_KEYS:
                raise ProviderError("model candidate metadata cannot contain credential fields")
        try:
            encoded = json.dumps(self.metadata, ensure_ascii=False, allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ProviderError("model candidate metadata must be JSON-safe") from error
        if len(encoded) > MAX_MODEL_METADATA_BYTES:
            raise ProviderError("model candidate metadata exceeds its bounded size")
        object.__setattr__(self, "metadata", dict(self.metadata))

    @property
    def arm_id(self) -> str:
        return f"{self.provider}/{self.model}"

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "ModelCandidate":
        if not isinstance(value, Mapping):
            raise ProviderError("model candidate must be an object")
        known = _MODEL_CANDIDATE_FIELDS
        metadata = value.get("metadata", {})
        if not isinstance(metadata, Mapping):
            raise ProviderError("model candidate metadata must be an object")
        extras = {key: item for key, item in value.items() if key not in known}
        merged_metadata = {**dict(metadata), **extras}
        return cls(
            provider=value.get("provider"),
            model=value.get("model"),
            context_window_tokens=value.get("context_window_tokens"),
            max_output_tokens=value.get("max_output_tokens"),
            quality=value.get("quality"),
            latency_ms=value.get("latency_ms"),
            cost_per_million_tokens=value.get("cost_per_million_tokens"),
            reliability=value.get("reliability", 0.5),
            capabilities=tuple(value.get("capabilities", ())),
            requires_credential=value.get("requires_credential", True),
            enabled=value.get("enabled", True),
            metadata=merged_metadata,
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "provider": self.provider,
            "model": self.model,
            "context_window_tokens": self.context_window_tokens,
            "max_output_tokens": self.max_output_tokens,
            "quality": float(self.quality),
            "latency_ms": self.latency_ms,
            "cost_per_million_tokens": self.cost_per_million_tokens,
            "reliability": float(self.reliability),
            "capabilities": list(self.capabilities),
            "requires_credential": self.requires_credential,
            "enabled": self.enabled,
        }
        result.update(dict(self.metadata))
        return result


class ModelCatalogue:
    """Thread-safe caller-owned registry of selectable model metadata.

    Registration is deliberately independent from provider credentials.  An application can
    install its approved model inventory at startup, show it in a UI, then collect a key later.
    ``candidates()`` returns deterministic mappings ready for ``AutonomousBrain`` selection;
    provider registration, circuit state, and credential readiness remain live runtime gates.
    """

    def __init__(self, candidates: Sequence[ModelCandidate | Mapping[str, Any]] = ()) -> None:
        if not isinstance(candidates, Sequence) or isinstance(candidates, (str, bytes)):
            raise ProviderError("model catalogue candidates must be a sequence")
        self._lock = threading.RLock()
        self._candidates: dict[tuple[str, str], ModelCandidate] = {}
        for candidate in candidates:
            self.register(candidate)

    def register(
        self,
        candidate: ModelCandidate | Mapping[str, Any],
        *,
        replace_existing: bool = False,
    ) -> ModelCandidate:
        resolved = candidate if isinstance(candidate, ModelCandidate) else ModelCandidate.from_mapping(candidate)
        if not isinstance(replace_existing, bool):
            raise ProviderError("replace_existing must be a boolean")
        key = (resolved.provider, resolved.model)
        with self._lock:
            if key in self._candidates and not replace_existing:
                raise ProviderError(f"model candidate is already registered: {resolved.arm_id}")
            if len(self._candidates) >= MAX_MODEL_CANDIDATES and key not in self._candidates:
                raise ProviderError("model catalogue capacity is exhausted")
            self._candidates[key] = resolved
        return resolved

    def remove(self, provider: str, model: str) -> ModelCandidate:
        key = (provider, model)
        with self._lock:
            candidate = self._candidates.pop(key, None)
        if candidate is None:
            raise ProviderError(f"model candidate is not registered: {provider}/{model}")
        return candidate

    def get(self, provider: str, model: str) -> ModelCandidate | None:
        with self._lock:
            return self._candidates.get((provider, model))

    def candidates(
        self,
        *,
        providers: Sequence[str] | None = None,
        enabled_only: bool = False,
    ) -> list[dict[str, Any]]:
        if providers is not None and (not isinstance(providers, Sequence) or isinstance(providers, (str, bytes))):
            raise ProviderError("model catalogue providers must be a sequence")
        if providers is not None and any(not isinstance(item, str) for item in providers):
            raise ProviderError("model catalogue providers must contain strings")
        provider_filter = None if providers is None else set(providers)
        if not isinstance(enabled_only, bool):
            raise ProviderError("enabled_only must be a boolean")
        with self._lock:
            values = tuple(self._candidates.values())
        return [
            candidate.to_dict()
            for candidate in sorted(values, key=lambda item: item.arm_id)
            if (provider_filter is None or candidate.provider in provider_filter)
            and (not enabled_only or candidate.enabled)
        ]

    def to_dict(self) -> dict[str, Any]:
        values = self.candidates()
        return {
            "schema": MODEL_CATALOGUE_SCHEMA,
            "candidate_count": len(values),
            "candidates": values,
            "credential_posture": "caller_supplied_opaque_handles",
            "secret_material": "never_returned",
        }

    def __len__(self) -> int:
        with self._lock:
            return len(self._candidates)


class ProviderHealthLedger:
    """Durable, value-only provider observations for restart-safe routing.

    The runtime circuit is intentionally process-local because it owns the live transport. This
    ledger persists only bounded outcome metadata, allowing an embedding application to restore
    a conservative historical provider gate after a restart. It never accepts request messages,
    response text, headers, credential handles, or arbitrary metadata fields.
    """

    _FORBIDDEN_FIELDS = frozenset(
        {
            "api_key",
            "apikey",
            "authorization",
            "bearer",
            "credential",
            "password",
            "secret",
            "access_token",
            "refresh_token",
            "token",
        }
    )
    _FORBIDDEN_NORMALIZED_FIELDS = frozenset(
        "".join(character for character in field if character.isalnum())
        for field in _FORBIDDEN_FIELDS
    )
    _ALLOWED_FIELDS = frozenset(
        {
            "schema",
            "provider",
            "model",
            "status",
            "outcome",
            "latency_ms",
            "observed_at",
            "status_code",
            "failure_class",
            "circuit",
            "consecutive_failures",
            "opened_until",
            "input_tokens",
            "output_tokens",
            "retention",
        }
    )

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_records: int = MAX_PROVIDER_HEALTH_RECORDS,
        max_bytes: int = MAX_PROVIDER_HEALTH_BYTES,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(max_records, int) or isinstance(max_records, bool) or max_records <= 0:
            raise ProviderError("provider health ledger max_records must be positive")
        if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes <= 0:
            raise ProviderError("provider health ledger max_bytes must be positive")
        if not callable(clock):
            raise ProviderError("provider health ledger clock must be callable")
        self.path = Path(path)
        self.max_records = max_records
        self.max_bytes = max_bytes
        self._clock = clock
        self._lock = threading.RLock()

    def record(self, observation: Mapping[str, Any]) -> dict[str, Any]:
        """Append one runtime observation and return a metadata-only receipt.

        This method is suitable as ``LLMRuntime(observation_callback=ledger.record)``. Runtime
        callbacks are best-effort, so a full or temporarily unreadable ledger cannot alter the
        provider invocation that produced the observation.
        """

        normalized = self._normalize_observation(observation)
        line = json.dumps(
            {"schema": PROVIDER_HEALTH_LEDGER_SCHEMA, "observation": normalized},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8") + b"\n"
        with self._lock:
            existing_size = self.path.stat().st_size if self.path.exists() else 0
            if existing_size + len(line) > self.max_bytes:
                raise ProviderError("provider health ledger capacity is exhausted")
            rows = self._read_records_locked()
            if len(rows) >= self.max_records:
                raise ProviderError("provider health ledger record capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            digest = hashlib.sha256(line.rstrip(b"\n")).hexdigest()
            return {
                "schema": PROVIDER_HEALTH_LEDGER_SCHEMA,
                "record_index": len(rows),
                "record_digest": digest,
                "provider": normalized["provider"],
                "outcome": normalized["outcome"],
            }

    def records(self, *, provider: str | None = None, limit: int | None = None) -> list[dict[str, Any]]:
        """Read bounded observations in append order, optionally filtered by provider."""

        if provider is not None:
            self._validate_provider(provider)
        if limit is not None and (
            not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_records
        ):
            raise ProviderError("provider health ledger limit is outside its bounds")
        with self._lock:
            rows = self._read_records_locked()
        observations = [
            dict(row["observation"])
            for row in rows
            if provider is None or row["observation"].get("provider") == provider
        ]
        if limit is not None:
            observations = observations[-limit:]
        return observations

    def health_snapshot(self, *, now: float | None = None) -> dict[str, dict[str, Any]]:
        """Aggregate the latest safe health state for each observed provider.

        An expired historical circuit is projected as closed. This prevents a transient outage
        from permanently disabling a provider while preserving the latest success/failure and
        latency evidence for diagnostics and future routing policies.
        """

        current_time = self._clock() if now is None else now
        if not isinstance(current_time, (int, float)) or isinstance(current_time, bool) or not math.isfinite(float(current_time)):
            raise ProviderError("provider health snapshot time must be finite")
        aggregate: dict[str, dict[str, Any]] = {}
        for observation in self.records():
            provider = observation["provider"]
            state = aggregate.setdefault(
                provider,
                {
                    "attempts": 0,
                    "successes": 0,
                    "failures": 0,
                    "total_input_tokens": 0,
                    "total_output_tokens": 0,
                },
            )
            state["attempts"] += 1
            state["successes"] += int(observation["outcome"] == "success")
            state["failures"] += int(observation["outcome"] == "failure")
            state["total_input_tokens"] += int(observation.get("input_tokens", 0))
            state["total_output_tokens"] += int(observation.get("output_tokens", 0))
            state.update(
                {
                    "last_model": observation["model"],
                    "circuit": observation.get("circuit", "closed"),
                    "consecutive_failures": observation.get("consecutive_failures", 0),
                    "opened_until": observation.get("opened_until"),
                    "last_outcome": observation["outcome"],
                    "last_status": observation["status"],
                    "last_latency_ms": observation["latency_ms"],
                    "observed_at": observation["observed_at"],
                }
            )
            if "status_code" in observation:
                state["last_status_code"] = observation["status_code"]
        for state in aggregate.values():
            attempts = state["attempts"]
            state["success_rate"] = state["successes"] / attempts if attempts else 0.0
            opened_until = state.get("opened_until")
            if state.get("circuit") == "open" and (
                opened_until is None or float(opened_until) > float(current_time)
            ):
                state["circuit"] = "open"
            else:
                state["circuit"] = "closed"
                state["opened_until"] = None
        return {provider: aggregate[provider] for provider in sorted(aggregate)}

    def selection_overrides(self, *, now: float | None = None) -> dict[str, Any]:
        """Return the brain selector's safe historical provider-health overlay."""

        snapshot = self.health_snapshot(now=now)
        return {} if not snapshot else {"provider_health": snapshot}

    def to_dict(self, *, now: float | None = None) -> dict[str, Any]:
        snapshot = self.health_snapshot(now=now)
        return {
            "schema": PROVIDER_HEALTH_LEDGER_SCHEMA,
            "provider_count": len(snapshot),
            "providers": snapshot,
            "record_count": len(self.records()),
            "retention": "value_only_provider_outcomes_no_payloads_or_credentials",
        }

    def _normalize_observation(self, observation: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(observation, Mapping):
            raise ProviderError("provider health observation must be an object")
        self._assert_value_only(observation)
        unknown = [key for key in observation if not isinstance(key, str) or key not in self._ALLOWED_FIELDS]
        if unknown:
            raise ProviderError("provider health observation contains unsupported fields")
        if observation.get("schema") != PROVIDER_OBSERVATION_SCHEMA:
            raise ProviderError("provider health observation schema is invalid")
        provider = observation.get("provider")
        self._validate_provider(provider)
        model = observation.get("model")
        if not isinstance(model, str) or not model.strip() or len(model.encode("utf-8")) > 512:
            raise ProviderError("provider health observation model is invalid")
        status = observation.get("status")
        outcome = observation.get("outcome")
        if status not in {"completed", "provider_refused"} or outcome not in {"success", "failure"}:
            raise ProviderError("provider health observation status or outcome is invalid")
        latency = observation.get("latency_ms")
        if not isinstance(latency, (int, float)) or isinstance(latency, bool) or not math.isfinite(float(latency)) or latency < 0:
            raise ProviderError("provider health observation latency is invalid")
        observed_at = observation.get("observed_at", self._clock())
        if not isinstance(observed_at, (int, float)) or isinstance(observed_at, bool) or not math.isfinite(float(observed_at)):
            raise ProviderError("provider health observation timestamp is invalid")
        result: dict[str, Any] = {
            "schema": PROVIDER_OBSERVATION_SCHEMA,
            "provider": provider,
            "model": model,
            "status": status,
            "outcome": outcome,
            "latency_ms": float(latency),
            "observed_at": float(observed_at),
        }
        for field_name in ("status_code", "consecutive_failures", "input_tokens", "output_tokens"):
            value = observation.get(field_name)
            if value is not None:
                if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                    raise ProviderError(f"provider health observation {field_name} is invalid")
                result[field_name] = value
        failure_class = observation.get("failure_class")
        if failure_class is not None:
            if failure_class not in {"provider_error", "circuit_open"}:
                raise ProviderError("provider health observation failure_class is invalid")
            result["failure_class"] = failure_class
        circuit = observation.get("circuit", "closed")
        if circuit not in {"open", "closed"}:
            raise ProviderError("provider health observation circuit is invalid")
        result["circuit"] = circuit
        opened_until = observation.get("opened_until")
        if opened_until is not None:
            if not isinstance(opened_until, (int, float)) or isinstance(opened_until, bool) or not math.isfinite(float(opened_until)):
                raise ProviderError("provider health observation opened_until is invalid")
            result["opened_until"] = float(opened_until)
        return result

    def _read_records_locked(self) -> list[dict[str, Any]]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise ProviderError("provider health ledger exceeds max_bytes")
        rows: list[dict[str, Any]] = []
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_records:
                    raise ProviderError("provider health ledger exceeds max_records")
                try:
                    row = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise ProviderError("provider health ledger contains invalid JSON") from error
                if not isinstance(row, Mapping) or row.get("schema") != PROVIDER_HEALTH_LEDGER_SCHEMA:
                    raise ProviderError("provider health ledger contains an invalid schema")
                observation = row.get("observation")
                rows.append({"schema": row["schema"], "observation": self._normalize_observation(observation)})
        return rows

    @classmethod
    def _assert_value_only(cls, value: Any, *, depth: int = 0) -> None:
        if depth > 16:
            raise ProviderError("provider health observation is too deeply nested")
        if isinstance(value, Mapping):
            for key, child in value.items():
                normalized = "".join(character for character in key.lower() if character.isalnum()) if isinstance(key, str) else ""
                if normalized in cls._FORBIDDEN_NORMALIZED_FIELDS:
                    raise ProviderError("provider health observation contains forbidden secret fields")
                cls._assert_value_only(child, depth=depth + 1)
        elif isinstance(value, (list, tuple)):
            for child in value:
                cls._assert_value_only(child, depth=depth + 1)
        elif isinstance(value, float) and not math.isfinite(value):
            raise ProviderError("provider health observation contains a non-finite number")

    @staticmethod
    def _validate_provider(provider: Any) -> None:
        if not isinstance(provider, str) or not provider.strip() or "/" in provider or " " in provider:
            raise ProviderError("provider health provider must be a path-safe identifier")


@dataclass(frozen=True, slots=True)
class ProviderTool:
    """Provider-neutral function schema; it describes a tool but never grants execution."""

    name: str
    description: str = ""
    parameters: Mapping[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if (
            not isinstance(self.name, str)
            or not self.name.strip()
            or len(self.name.encode("utf-8")) > MAX_TOOL_NAME_BYTES
            or any(not (character.isalnum() or character in "_-.") for character in self.name)
        ):
            raise ProviderError("provider tool name is not a bounded safe identifier")
        if not isinstance(self.description, str) or len(self.description) > MAX_MESSAGE_CHARS:
            raise ProviderError("provider tool description is not bounded")
        if not isinstance(self.parameters, Mapping):
            raise ProviderError("provider tool parameters must be a JSON object")
        try:
            encoded = json.dumps(self.parameters, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ProviderError("provider tool parameters must be JSON-safe") from error
        if len(encoded.encode("utf-8")) > 256_000:
            raise ProviderError("provider tool parameters exceed the bounded size")

    @classmethod
    def from_mcp_schema(cls, schema: Mapping[str, Any]) -> "ProviderTool":
        if not isinstance(schema, Mapping):
            raise ProviderError("MCP tool schema must be an object")
        parameters = schema.get("inputSchema", schema.get("parameters", {}))
        if not isinstance(parameters, Mapping):
            raise ProviderError("MCP tool schema inputSchema must be an object")
        return cls(
            name=schema.get("name", ""),
            description=schema.get("description", ""),
            parameters=dict(parameters),
        )

    def to_wire(self, protocol: str) -> dict[str, Any]:
        if protocol == "anthropic_messages":
            return {
                "name": self.name,
                "description": self.description,
                "input_schema": dict(self.parameters),
            }
        if protocol == "openai_chat_completions":
            return {
                "type": "function",
                "function": {
                    "name": self.name,
                    "description": self.description,
                    "parameters": dict(self.parameters),
                },
            }
        return {
            "type": "function",
            "name": self.name,
            "description": self.description,
            "parameters": dict(self.parameters),
        }


@dataclass(frozen=True, slots=True)
class ProviderToolCall:
    """A parsed provider intent; callers must validate and authorize it before execution."""

    call_id: str
    name: str
    arguments: Mapping[str, Any]

    def __post_init__(self) -> None:
        if not isinstance(self.call_id, str) or not self.call_id.strip() or len(self.call_id) > 256:
            raise ProviderError("provider tool call id is not bounded")
        if not isinstance(self.name, str) or not self.name.strip() or len(self.name) > MAX_TOOL_NAME_BYTES:
            raise ProviderError("provider tool call name is not bounded")
        if not isinstance(self.arguments, Mapping):
            raise ProviderError("provider tool call arguments must be an object")
        try:
            encoded = json.dumps(self.arguments, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ProviderError("provider tool call arguments must be JSON-safe") from error
        if len(encoded.encode("utf-8")) > MAX_TOOL_ARGUMENT_BYTES:
            raise ProviderError("provider tool call arguments exceed the bounded size")

    def to_dict(self) -> dict[str, Any]:
        return {
            "call_id": self.call_id,
            "name": self.name,
            "arguments": dict(self.arguments),
            "execution": "not_started",
            "authorization": "caller_owned",
        }


@dataclass(frozen=True, slots=True)
class ProviderRequest:
    model: str
    messages: tuple[Mapping[str, Any], ...]
    max_output_tokens: int = 1024
    temperature: float | None = None
    require_json: bool = False
    response_schema: Mapping[str, Any] | None = None
    idempotency_key: str | None = None
    tools: tuple[ProviderTool, ...] = ()
    tool_choice: str | None = None

    def __post_init__(self) -> None:
        if not self.model or len(self.messages) > MAX_MESSAGES:
            raise ProviderError("model and messages are required within their bounds")
        if self.max_output_tokens <= 0:
            raise ProviderError("max_output_tokens must be positive")
        if self.temperature is not None and not 0 <= self.temperature <= 2:
            raise ProviderError("temperature must be within [0, 2]")
        if not isinstance(self.require_json, bool):
            raise ProviderError("require_json must be a boolean")
        if self.response_schema is not None:
            if not isinstance(self.response_schema, Mapping):
                raise ProviderError("response_schema must be a JSON object")
            try:
                encoded_schema = json.dumps(self.response_schema, allow_nan=False)
            except (TypeError, ValueError) as error:
                raise ProviderError("response_schema must be JSON-safe") from error
            if len(encoded_schema.encode("utf-8")) > 256_000:
                raise ProviderError("response_schema exceeds the bounded size")
        if self.idempotency_key is not None and (
            not isinstance(self.idempotency_key, str)
            or not self.idempotency_key.strip()
            or len(self.idempotency_key) > 256
        ):
            raise ProviderError("idempotency_key must be a bounded non-empty string")
        if not isinstance(self.tools, Sequence) or isinstance(self.tools, (str, bytes)):
            raise ProviderError("tools must be a sequence of ProviderTool values")
        if len(self.tools) > MAX_PROVIDER_TOOLS:
            raise ProviderError("tools exceed the bounded provider limit")
        names: set[str] = set()
        for tool in self.tools:
            if not isinstance(tool, ProviderTool):
                raise ProviderError("tools must contain only ProviderTool values")
            if tool.name in names:
                raise ProviderError("provider tool names must be unique")
            names.add(tool.name)
        if self.tool_choice not in {None, "auto", "none", "required"}:
            raise ProviderError("tool_choice must be auto, none, required, or None")
        for message in self.messages:
            _validate_provider_message(message)

    def with_tool_results(
        self,
        tool_calls: Sequence[ProviderToolCall],
        results: Sequence[ProviderToolResult],
    ) -> "ProviderRequest":
        """Append a provider-neutral assistant/tool turn for an explicit continuation.

        The internal ``tool_calls`` and ``tool`` message shapes are translated by ``_body`` for
        Responses, Chat Completions, and Anthropic Messages. The method requires one caller-
        approved result for every call, so a model cannot silently advance after an unapproved
        intent or a missing execution result.
        """

        if not isinstance(tool_calls, Sequence) or isinstance(tool_calls, (str, bytes)):
            raise ProviderError("tool_calls must be a sequence")
        if not isinstance(results, Sequence) or isinstance(results, (str, bytes)):
            raise ProviderError("tool results must be a sequence")
        if any(not isinstance(call, ProviderToolCall) for call in tool_calls):
            raise ProviderError("tool_calls must contain ProviderToolCall values")
        if any(not isinstance(result, ProviderToolResult) for result in results):
            raise ProviderError("tool results must contain ProviderToolResult values")
        if len(tool_calls) != len(results):
            raise ProviderError("every provider tool call requires exactly one result")
        expected_ids = [call.call_id for call in tool_calls]
        result_ids = [result.call_id for result in results]
        if result_ids != expected_ids or any(not result.approved for result in results):
            raise ProviderError("provider tool results require caller approval in call order")
        assistant_tool_calls = tuple(
            {
                "id": call.call_id,
                "name": call.name,
                "arguments": json.dumps(
                    call.arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                ),
            }
            for call in tool_calls
        )
        continuation_messages: list[Mapping[str, Any]] = [
            {
                "role": "assistant",
                "content": "",
                "tool_calls": assistant_tool_calls,
            }
        ]
        continuation_messages.extend(
            {
                "role": "tool",
                "tool_call_id": result.call_id,
                "content": result.serialized_content(),
                "is_error": result.is_error,
            }
            for result in results
        )
        combined = self.messages + tuple(continuation_messages)
        if len(combined) > MAX_MESSAGES:
            raise ProviderError("provider continuation would exceed the message bound")
        return replace(self, messages=combined)


@dataclass(frozen=True, slots=True)
class ProviderResponse:
    provider: str
    model: str
    text: str
    status_code: int
    request_id: str | None
    usage: Mapping[str, Any]
    raw: Mapping[str, Any]
    structured: Any = None
    tool_calls: tuple[ProviderToolCall, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "model": self.model,
            "text": self.text,
            "status_code": self.status_code,
            "request_id": self.request_id,
            "usage": dict(self.usage),
            "raw": dict(self.raw),
            "structured": self.structured,
            "tool_calls": [call.to_dict() for call in self.tool_calls],
            "credential_posture": "not_in_response",
        }


@dataclass(frozen=True, slots=True)
class ProviderStreamEvent:
    """A bounded provider-neutral projection of one SSE event.

    The event deliberately exposes deltas and a finalized :class:`ProviderToolCall`, rather than
    provider payloads. This keeps streaming useful to a UI while preventing raw event bodies from
    becoming an accidental persistence or logging channel.
    """

    provider: str
    model: str
    sequence: int
    event_type: str
    request_id: str | None = None
    text_delta: str = ""
    tool_call_id: str | None = None
    tool_name: str | None = None
    arguments_delta: str = ""
    tool_call: ProviderToolCall | None = None
    usage: Mapping[str, Any] = field(default_factory=dict)
    done: bool = False

    def __post_init__(self) -> None:
        if not isinstance(self.provider, str) or not self.provider:
            raise ProviderError("stream event provider is required")
        if not isinstance(self.model, str) or not self.model:
            raise ProviderError("stream event model is required")
        if not isinstance(self.sequence, int) or self.sequence < 0:
            raise ProviderError("stream event sequence must be non-negative")
        if (
            not isinstance(self.event_type, str)
            or not self.event_type
            or len(self.event_type) > 256
            or any(ord(character) < 32 for character in self.event_type)
        ):
            raise ProviderError("stream event type is not bounded")
        for label, value, limit in (
            ("text delta", self.text_delta, MAX_STREAM_TEXT_BYTES),
            ("arguments delta", self.arguments_delta, MAX_TOOL_ARGUMENT_BYTES),
        ):
            if not isinstance(value, str) or len(value.encode("utf-8")) > limit:
                raise ProviderError(f"stream {label} exceeds the bounded size")
        for label, value in (("request id", self.request_id), ("tool call id", self.tool_call_id), ("tool name", self.tool_name)):
            if value is not None and (
                not isinstance(value, str)
                or not value
                or len(value.encode("utf-8")) > MAX_TOOL_NAME_BYTES
            ):
                raise ProviderError(f"stream {label} is not bounded")
        if self.tool_call is not None and not isinstance(self.tool_call, ProviderToolCall):
            raise ProviderError("stream tool_call must be a ProviderToolCall")
        if not isinstance(self.usage, Mapping):
            raise ProviderError("stream usage must be an object")
        _bounded_json_bytes(self.usage, 256_000, "stream usage")
        if not isinstance(self.done, bool):
            raise ProviderError("stream done must be a boolean")

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "model": self.model,
            "sequence": self.sequence,
            "event_type": self.event_type,
            "request_id": self.request_id,
            "text_delta": self.text_delta,
            "tool_call_id": self.tool_call_id,
            "tool_name": self.tool_name,
            "arguments_delta": self.arguments_delta,
            "tool_call": None if self.tool_call is None else self.tool_call.to_dict(),
            "usage": dict(self.usage),
            "done": self.done,
            "credential_posture": "not_in_event",
        }


@dataclass(frozen=True, slots=True)
class ProviderToolResult:
    """Caller-approved output returned to a provider continuation turn."""

    call_id: str
    content: Any
    approved: bool = False
    is_error: bool = False

    def __post_init__(self) -> None:
        if not isinstance(self.call_id, str) or not self.call_id.strip() or len(self.call_id) > 256:
            raise ProviderError("provider tool result call id is not bounded")
        if not isinstance(self.approved, bool) or not isinstance(self.is_error, bool):
            raise ProviderError("provider tool result flags must be booleans")
        _bounded_json_bytes(self.content, MAX_TOOL_ARGUMENT_BYTES, "provider tool result")

    def serialized_content(self) -> str:
        if isinstance(self.content, str):
            return self.content
        return json.dumps(self.content, ensure_ascii=False, sort_keys=True, separators=(",", ":"))

    def to_dict(self) -> dict[str, Any]:
        return {
            "call_id": self.call_id,
            "content": self.content,
            "approved": self.approved,
            "is_error": self.is_error,
            "authorization": "caller_approved" if self.approved else "caller_approval_required",
        }


@dataclass(frozen=True, slots=True)
class ProviderToolLoopResult:
    """Bounded result of explicit caller-authorized multi-turn tool continuation."""

    status: str
    responses: tuple[ProviderResponse, ...]
    final_response: ProviderResponse | None
    turns: int
    tool_calls: int

    def __post_init__(self) -> None:
        if self.status not in {"completed", "authorization_required", "turn_limit_reached"}:
            raise ProviderError("provider tool loop returned an invalid status")
        if not isinstance(self.responses, tuple) or any(
            not isinstance(response, ProviderResponse) for response in self.responses
        ):
            raise ProviderError("provider tool loop responses are malformed")
        if self.final_response is not None and not isinstance(self.final_response, ProviderResponse):
            raise ProviderError("provider tool loop final response is malformed")
        if not isinstance(self.turns, int) or self.turns < 0:
            raise ProviderError("provider tool loop turns must be non-negative")
        if not isinstance(self.tool_calls, int) or self.tool_calls < 0:
            raise ProviderError("provider tool loop tool_calls must be non-negative")

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "turns": self.turns,
            "tool_calls": self.tool_calls,
            "responses": [response.to_dict() for response in self.responses],
            "final_response": None if self.final_response is None else self.final_response.to_dict(),
            "tool_execution": "caller_authorized_only",
        }


@dataclass(slots=True)
class _CircuitState:
    consecutive_failures: int = 0
    opened_until: float | None = None


class LLMRuntime:
    """Invoke configured providers while resolving secrets only at the header boundary."""

    def __init__(
        self,
        credentials: CredentialStore | None = None,
        *,
        clock: Callable[[], float] = time.time,
        sleeper: Callable[[float], None] = time.sleep,
        observation_callback: Callable[[Mapping[str, Any]], None] | None = None,
    ) -> None:
        self.credentials = credentials or CredentialStore()
        self._providers: dict[str, ProviderConfig] = {}
        self._circuits: dict[str, _CircuitState] = {}
        self._clock = clock
        self._sleeper = sleeper
        self._observation_lock = threading.RLock()
        self._observation_callbacks: list[Callable[[Mapping[str, Any]], None]] = []
        if observation_callback is not None:
            self.add_observation_callback(observation_callback)

    def add_observation_callback(self, callback: Callable[[Mapping[str, Any]], None]) -> None:
        """Register a best-effort value-only provider outcome observer.

        Observers receive provider/model/status/latency/usage metadata only. They never receive
        request messages, response text, headers, credential handles, or raw provider payloads.
        Observer failures are isolated from the provider call so telemetry cannot change runtime
        authorization or retry semantics.
        """

        if not callable(callback):
            raise ProviderError("observation callback must be callable")
        with self._observation_lock:
            if callback not in self._observation_callbacks:
                self._observation_callbacks.append(callback)

    def remove_observation_callback(self, callback: Callable[[Mapping[str, Any]], None]) -> None:
        with self._observation_lock:
            if callback in self._observation_callbacks:
                self._observation_callbacks.remove(callback)

    def _notify_observation(
        self,
        config: ProviderConfig,
        request: ProviderRequest,
        *,
        status: str,
        outcome: str,
        latency_ms: float,
        response: ProviderResponse | None = None,
        error: ProviderError | None = None,
    ) -> None:
        payload: dict[str, Any] = {
            "schema": PROVIDER_OBSERVATION_SCHEMA,
            "provider": config.provider,
            "model": request.model,
            "status": status,
            "outcome": outcome,
            "latency_ms": max(0.0, float(latency_ms)),
            "observed_at": float(self._clock()),
            "retention": "metadata_only_no_provider_payloads",
        }
        circuit_state = self._circuits.get(config.provider)
        if circuit_state is not None:
            now = self._clock()
            circuit_open = circuit_state.opened_until is not None and now < circuit_state.opened_until
            payload["circuit"] = "open" if circuit_open else "closed"
            payload["consecutive_failures"] = circuit_state.consecutive_failures
            if circuit_state.opened_until is not None:
                payload["opened_until"] = circuit_state.opened_until
        if response is not None:
            for key in ("input_tokens", "output_tokens"):
                value = response.usage.get(key)
                if isinstance(value, int) and not isinstance(value, bool) and value >= 0:
                    payload[key] = value
        if error is not None:
            payload["failure_class"] = "circuit_open" if error.circuit_open else "provider_error"
            if error.status_code is not None:
                payload["status_code"] = error.status_code
        with self._observation_lock:
            callbacks = tuple(self._observation_callbacks)
        for callback in callbacks:
            try:
                callback(dict(payload))
            except Exception:
                # Telemetry is deliberately non-authoritative and must not turn a successful
                # provider response into a failed model invocation.
                continue

    def register_provider(self, config: ProviderConfig) -> None:
        self._providers[config.provider] = config
        self._circuits.setdefault(config.provider, _CircuitState())

    def provider_metadata(self) -> list[dict[str, Any]]:
        return [self._providers[name].to_metadata() for name in sorted(self._providers)]

    def provider_status(self, provider: str) -> dict[str, Any]:
        """Return value-only circuit state; no credential or provider response is retained."""

        config = self._providers.get(provider)
        if config is None:
            raise ProviderError(f"provider {provider!r} is not configured")
        state = self._circuits.setdefault(provider, _CircuitState())
        open_now = state.opened_until is not None and self._clock() < state.opened_until
        return {
            "provider": provider,
            "configured": True,
            "circuit": "open" if open_now else "closed",
            "consecutive_failures": state.consecutive_failures,
            "opened_until": state.opened_until,
            "max_attempts": config.max_attempts,
            "credential_posture": "caller_supplied_in_memory_handle",
        }

    def provider_requires_credential(self, provider: str) -> bool:
        """Return whether this registered transport requires a caller-owned credential handle."""

        config = self._providers.get(provider)
        if config is None:
            raise ProviderError(f"provider {provider!r} is not configured")
        return config.requires_credential

    @staticmethod
    def _invocation_metadata(
        provider: str,
        request: ProviderRequest,
        kind: str,
    ) -> ProviderInvocationMetadata:
        if not isinstance(kind, str) or not kind.strip() or len(kind) > 128:
            raise ProviderError("provider invocation kind must be a bounded non-empty string")
        # This is an admission estimate only.  Provider-reported usage is used for the final
        # receipt when available.  Counting bytes rather than retaining content keeps the hook
        # useful for policy without giving telemetry access to prompt data.
        input_bytes = 0
        for message in request.messages:
            content = message.get("content")
            if isinstance(content, str):
                input_bytes += len(content.encode("utf-8"))
            else:
                input_bytes += len(json.dumps(content, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))
        input_tokens = max(1, (input_bytes + 3) // 4)
        return ProviderInvocationMetadata(
            provider=provider,
            model=request.model,
            kind=kind,
            input_tokens=input_tokens,
            requested_output_tokens=request.max_output_tokens,
            tool_count=len(request.tools),
        )

    @staticmethod
    def _notify_invocation_before(
        observer: ProviderInvocationObserver | None,
        metadata: ProviderInvocationMetadata,
    ) -> None:
        if observer is not None:
            observer.before(metadata)

    @staticmethod
    def _notify_invocation_after(
        observer: ProviderInvocationObserver | None,
        metadata: ProviderInvocationMetadata,
        response: ProviderResponse | None,
        error: BaseException | None,
        started: float,
    ) -> None:
        if observer is not None:
            observer.after(metadata, response, error, max(0.0, (time.perf_counter() - started) * 1000.0))

    def reset_provider(self, provider: str) -> None:
        """Explicitly close a circuit after an operator or health check has reviewed it."""

        if provider not in self._providers:
            raise ProviderError(f"provider {provider!r} is not configured")
        self._circuits[provider] = _CircuitState()

    def invoke(
        self,
        provider: str,
        request: ProviderRequest,
        *,
        credential: CredentialHandle | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        invocation_kind: str = "provider_call",
    ) -> ProviderResponse:
        config = self._providers.get(provider)
        if config is None:
            raise ProviderError(f"provider {provider!r} is not configured")
        secret: SecretValue | None = None
        if config.requires_credential:
            if credential is None:
                raise CredentialError(f"provider {provider!r} requires a user credential handle")
            if credential.provider != provider:
                raise CredentialError("credential provider does not match invocation provider")
            secret = self.credentials._resolve(credential)
        body = self._body(config, request)
        headers = {
            "Accept": "application/json",
            "Content-Type": "application/json",
        }
        if secret is not None:
            if config.protocol == "anthropic_messages":
                headers[config.api_key_header or "x-api-key"] = secret.expose()
                headers["anthropic-version"] = "2023-06-01"
            else:
                headers[config.api_key_header or "Authorization"] = "Bearer " + secret.expose()
        if request.idempotency_key is not None:
            headers["Idempotency-Key"] = request.idempotency_key
        metadata = self._invocation_metadata(provider, request, invocation_kind)
        self._notify_invocation_before(invocation_observer, metadata)
        started = time.perf_counter()
        try:
            response = self._post(config, body, headers, request)
        except BaseException as error:
            self._notify_invocation_after(invocation_observer, metadata, None, error, started)
            raise
        self._notify_invocation_after(invocation_observer, metadata, response, None, started)
        return response

    def invoke_stream(
        self,
        provider: str,
        request: ProviderRequest,
        *,
        credential: CredentialHandle | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        invocation_kind: str = "provider_stream",
    ) -> Iterator[ProviderStreamEvent]:
        """Open one bounded SSE provider invocation.

        The returned iterator yields provider-neutral deltas and finalized tool intents. It never
        dispatches a tool. Streaming retries are intentionally not attempted after a connection
        has begun yielding events because replaying a partial assistant turn could duplicate a
        caller-visible intent.
        """

        config = self._providers.get(provider)
        if config is None:
            raise ProviderError(f"provider {provider!r} is not configured")
        secret: SecretValue | None = None
        if config.requires_credential:
            if credential is None:
                raise CredentialError(f"provider {provider!r} requires a user credential handle")
            if credential.provider != provider:
                raise CredentialError("credential provider does not match invocation provider")
            secret = self.credentials._resolve(credential)
        body = dict(self._body(config, request))
        body["stream"] = True
        headers = {
            "Accept": "text/event-stream",
            "Content-Type": "application/json",
        }
        if secret is not None:
            if config.protocol == "anthropic_messages":
                headers[config.api_key_header or "x-api-key"] = secret.expose()
                headers["anthropic-version"] = "2023-06-01"
            else:
                headers[config.api_key_header or "Authorization"] = "Bearer " + secret.expose()
        if request.idempotency_key is not None:
            headers["Idempotency-Key"] = request.idempotency_key
        metadata = self._invocation_metadata(provider, request, invocation_kind)
        stream = self._stream(config, body, headers, request)
        if invocation_observer is None:
            return stream

        def observed_stream() -> Iterator[ProviderStreamEvent]:
            self._notify_invocation_before(invocation_observer, metadata)
            started = time.perf_counter()
            try:
                yield from stream
            except BaseException as error:
                self._notify_invocation_after(invocation_observer, metadata, None, error, started)
                raise
            self._notify_invocation_after(invocation_observer, metadata, None, None, started)

        return observed_stream()

    def collect_stream(
        self,
        provider: str,
        request: ProviderRequest,
        *,
        credential: CredentialHandle | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        invocation_kind: str = "provider_stream",
    ) -> ProviderResponse:
        """Collect a stream into the same bounded response contract as ``invoke``."""

        metadata = self._invocation_metadata(provider, request, invocation_kind)
        self._notify_invocation_before(invocation_observer, metadata)
        started = time.perf_counter()
        text_parts: list[str] = []
        text_bytes = 0
        tool_calls: list[ProviderToolCall] = []
        usage: Mapping[str, Any] = {}
        request_id: str | None = None
        model = request.model
        event_count = 0
        terminal_type: str | None = None
        try:
            for event in self.invoke_stream(provider, request, credential=credential):
                event_count += 1
                if event_count > MAX_STREAM_EVENTS:
                    raise ProviderError("provider stream exceeded max event count")
                if event.text_delta:
                    text_parts.append(event.text_delta)
                    text_bytes += len(event.text_delta.encode("utf-8"))
                    if text_bytes > MAX_STREAM_TEXT_BYTES:
                        raise ProviderError("provider stream text exceeded the bounded size")
                if event.tool_call is not None:
                    tool_calls.append(event.tool_call)
                if event.usage:
                    usage = event.usage
                request_id = event.request_id or request_id
                model = event.model or model
                if event.done:
                    terminal_type = event.event_type
            if not text_parts and not tool_calls:
                raise ProviderError("provider stream contained no assistant text or tool call")
            text = "".join(text_parts)
            structured = None if tool_calls else _validate_structured_response(text, request)
            response = ProviderResponse(
                provider=provider,
                model=model,
                text=text,
                status_code=200,
                request_id=request_id,
                usage=dict(usage),
                raw={
                    "stream": True,
                    "event_count": event_count,
                    "terminal_event": terminal_type,
                },
                structured=structured,
                tool_calls=tuple(tool_calls),
            )
        except BaseException as error:
            self._notify_invocation_after(invocation_observer, metadata, None, error, started)
            raise
        self._notify_invocation_after(invocation_observer, metadata, response, None, started)
        return response

    def invoke_tool_loop(
        self,
        provider: str,
        request: ProviderRequest,
        *,
        credential: CredentialHandle | None = None,
        authorize_and_execute: Callable[[tuple[ProviderToolCall, ...]], Sequence[ProviderToolResult]],
        max_turns: int = 4,
        max_tool_calls: int = MAX_PROVIDER_TOOLS,
        stream: bool = False,
        initial_response: ProviderResponse | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        invocation_kind: str = "tool_loop_turn",
    ) -> ProviderToolLoopResult:
        """Run bounded native tool continuation with a caller-owned authorization callback.

        ``authorize_and_execute`` is the only place where an application may perform effects. It
        must return one explicitly ``approved`` result per model intent. The runtime only carries
        those results back to the provider and stops at either caller refusal or the turn budget.
        """

        if not callable(authorize_and_execute):
            raise ProviderError("authorize_and_execute must be callable")
        if not 1 <= max_turns <= 32:
            raise ProviderError("max_turns must be within [1, 32]")
        if not 1 <= max_tool_calls <= 1024:
            raise ProviderError("max_tool_calls must be within [1, 1024]")
        if initial_response is not None and (
            not isinstance(initial_response, ProviderResponse)
            or initial_response.provider != provider
            or initial_response.model != request.model
        ):
            raise ProviderError("initial response does not match the continuation request")
        current = request
        responses: list[ProviderResponse] = []
        total_tool_calls = 0
        response = initial_response
        for _turn in range(max_turns):
            if response is None:
                response = (
                    self.collect_stream(
                        provider,
                        current,
                        credential=credential,
                        invocation_observer=invocation_observer,
                        invocation_kind=invocation_kind,
                    )
                    if stream
                    else self.invoke(
                        provider,
                        current,
                        credential=credential,
                        invocation_observer=invocation_observer,
                        invocation_kind=invocation_kind,
                    )
                )
            responses.append(response)
            if not response.tool_calls:
                return ProviderToolLoopResult(
                    status="completed",
                    responses=tuple(responses),
                    final_response=response,
                    turns=len(responses),
                    tool_calls=total_tool_calls,
                )
            total_tool_calls += len(response.tool_calls)
            if total_tool_calls > max_tool_calls:
                return ProviderToolLoopResult(
                    status="turn_limit_reached",
                    responses=tuple(responses),
                    final_response=response,
                    turns=len(responses),
                    tool_calls=total_tool_calls,
                )
            if _turn + 1 >= max_turns:
                return ProviderToolLoopResult(
                    status="turn_limit_reached",
                    responses=tuple(responses),
                    final_response=response,
                    turns=len(responses),
                    tool_calls=total_tool_calls,
                )
            returned = authorize_and_execute(response.tool_calls)
            if not isinstance(returned, Sequence) or isinstance(returned, (str, bytes)):
                raise ProviderError("authorization callback must return a sequence of tool results")
            if any(not isinstance(result, ProviderToolResult) for result in returned):
                raise ProviderError("authorization callback returned a malformed tool result")
            if len(returned) != len(response.tool_calls) or any(not result.approved for result in returned):
                return ProviderToolLoopResult(
                    status="authorization_required",
                    responses=tuple(responses),
                    final_response=response,
                    turns=len(responses),
                    tool_calls=total_tool_calls,
                )
            current = current.with_tool_results(response.tool_calls, returned)
            response = None
        return ProviderToolLoopResult(
            status="turn_limit_reached",
            responses=tuple(responses),
            final_response=responses[-1] if responses else None,
            turns=len(responses),
            tool_calls=total_tool_calls,
        )

    def _stream(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        request: ProviderRequest,
    ) -> Iterator[ProviderStreamEvent]:
        started = time.perf_counter()
        try:
            yield from self._stream_with_circuit(config, body, headers, request)
        except ProviderError as error:
            self._notify_observation(
                config,
                request,
                status="provider_refused",
                outcome="failure",
                latency_ms=(time.perf_counter() - started) * 1000.0,
                error=error,
            )
            raise
        else:
            self._notify_observation(
                config,
                request,
                status="completed",
                outcome="success",
                latency_ms=(time.perf_counter() - started) * 1000.0,
            )

    def _stream_with_circuit(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        request: ProviderRequest,
    ) -> Iterator[ProviderStreamEvent]:
        state = self._circuits.setdefault(config.provider, _CircuitState())
        now = self._clock()
        if state.opened_until is not None:
            if now < state.opened_until:
                raise ProviderError(
                    "provider circuit is open; invocation is temporarily refused",
                    circuit_open=True,
                )
            state.opened_until = None
            state.consecutive_failures = 0
        try:
            yield from self._stream_once(config, body, headers, request)
        except ProviderError as error:
            if error.retryable:
                state.consecutive_failures += 1
                if state.consecutive_failures >= config.circuit_breaker_failure_threshold:
                    state.opened_until = self._clock() + config.circuit_breaker_reset_seconds
            raise
        else:
            state.consecutive_failures = 0
            state.opened_until = None

    def _stream_once(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        request: ProviderRequest,
    ) -> Iterator[ProviderStreamEvent]:
        host, port, path, scheme = config.endpoint
        try:
            encoded = json.dumps(body, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ProviderError(f"provider request is not JSON-safe: {error}") from error
        connection: http.client.HTTPConnection | http.client.HTTPSConnection = (
            http.client.HTTPSConnection(host, port, timeout=config.timeout_seconds)
            if scheme == "https"
            else http.client.HTTPConnection(host, port, timeout=config.timeout_seconds)
        )
        try:
            connection.request("POST", path, body=encoded, headers=dict(headers))
            response = connection.getresponse()
            response_headers = {name.lower(): value for name, value in response.getheaders()}
            status = response.status
            if status >= 400:
                raise ProviderError(
                    f"provider returned HTTP status {status}",
                    retryable=status == 408 or status == 429 or status >= 500,
                    status_code=status,
                )
            content_type = response_headers.get("content-type", "").split(";", 1)[0].strip().lower()
            if content_type and content_type != "text/event-stream":
                raise ProviderError("provider stream did not return text/event-stream")
            state: dict[str, Any] = {
                "model": request.model,
                "request_id": None,
                "calls": {},
            }
            sequence = 0
            for event_name, data in _iter_sse_frames(response, config.max_response_bytes):
                if data.strip() == "[DONE]":
                    specs = _finalize_stream_tool_calls(config.protocol, state)
                    specs.append({"event_type": "stream.done", "done": True})
                else:
                    try:
                        payload = json.loads(data)
                    except (UnicodeDecodeError, json.JSONDecodeError) as error:
                        raise ProviderError("provider stream contained invalid JSON") from error
                    if not isinstance(payload, Mapping):
                        raise ProviderError("provider stream event must be a JSON object")
                    specs = _project_stream_payload(config.protocol, event_name, payload, state)
                for spec in specs:
                    if sequence >= MAX_STREAM_EVENTS:
                        raise ProviderError("provider stream exceeded max event count")
                    sequence += 1
                    event = ProviderStreamEvent(
                        provider=config.provider,
                        model=str(state.get("model") or request.model),
                        sequence=sequence,
                        request_id=_string_or_none(state.get("request_id")),
                        **spec,
                    )
                    state["text_bytes"] = state.get("text_bytes", 0) + len(event.text_delta.encode("utf-8"))
                    if state["text_bytes"] > MAX_STREAM_TEXT_BYTES:
                        raise ProviderError("provider stream text exceeded the bounded size")
                    if event.tool_name is not None and event.tool_name not in {tool.name for tool in request.tools}:
                        raise ProviderError("provider returned an unrequested streamed tool call")
                    yield event
        except (OSError, http.client.HTTPException) as error:
            raise ProviderError(
                "provider stream transport failed; credential material was discarded",
                retryable=True,
            ) from error
        finally:
            connection.close()

    @staticmethod
    def _body(config: ProviderConfig, request: ProviderRequest) -> dict[str, Any]:
        messages = _wire_messages(config.protocol, request.messages)
        if config.protocol == "openai_responses":
            body: dict[str, Any] = {
                "model": request.model,
                "input": messages,
                "max_output_tokens": request.max_output_tokens,
            }
        elif config.protocol == "anthropic_messages":
            system = "\n\n".join(
                str(message["content"])
                for message in messages
                if message.get("role") == "system" and isinstance(message.get("content"), str)
            )
            body = {
                "model": request.model,
                "messages": [
                    message for message in messages if message.get("role") != "system"
                ],
                "max_tokens": request.max_output_tokens,
            }
            if system:
                body["system"] = system
        else:
            body = {
                "model": request.model,
                "messages": messages,
                "max_tokens": request.max_output_tokens,
            }
        if request.temperature is not None:
            body["temperature"] = request.temperature
        if request.tools:
            body["tools"] = [tool.to_wire(config.protocol) for tool in request.tools]
            if request.tool_choice is not None:
                body["tool_choice"] = (
                    {"type": request.tool_choice}
                    if config.protocol == "anthropic_messages"
                    else request.tool_choice
                )
        if (
            (request.require_json or request.response_schema is not None)
            and config.protocol in {"openai_responses", "openai_chat_completions"}
            and (not request.tools or request.tool_choice == "none")
        ):
            # OpenAI-compatible protocols accept this hint. Anthropic has a different structured
            # output surface, so it is validated locally without receiving an unsupported field.
            body["response_format"] = {"type": "json_object"}
        return body

    def _post(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        request: ProviderRequest,
    ) -> ProviderResponse:
        started = time.perf_counter()
        try:
            response = self._post_with_retries(config, body, headers, request)
        except ProviderError as error:
            self._notify_observation(
                config,
                request,
                status="provider_refused",
                outcome="failure",
                latency_ms=(time.perf_counter() - started) * 1000.0,
                error=error,
            )
            raise
        self._notify_observation(
            config,
            request,
            status="completed",
            outcome="success",
            latency_ms=(time.perf_counter() - started) * 1000.0,
            response=response,
        )
        return response

    def _post_with_retries(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        request: ProviderRequest,
    ) -> ProviderResponse:
        state = self._circuits.setdefault(config.provider, _CircuitState())
        now = self._clock()
        if state.opened_until is not None:
            if now < state.opened_until:
                raise ProviderError(
                    "provider circuit is open; invocation is temporarily refused",
                    circuit_open=True,
                )
            state.opened_until = None
            state.consecutive_failures = 0

        last_error: ProviderError | None = None
        for attempt in range(config.max_attempts):
            try:
                response = self._post_once(config, body, headers, request)
                state.consecutive_failures = 0
                state.opened_until = None
                return response
            except ProviderError as error:
                last_error = error
                if not error.retryable or attempt + 1 >= config.max_attempts:
                    break
                delay = min(config.retry_backoff_seconds * (2**attempt), 60.0)
                if delay:
                    self._sleeper(delay)
        assert last_error is not None
        if last_error.retryable:
            state.consecutive_failures += 1
            if state.consecutive_failures >= config.circuit_breaker_failure_threshold:
                state.opened_until = self._clock() + config.circuit_breaker_reset_seconds
        raise last_error

    def _post_once(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        request: ProviderRequest,
    ) -> ProviderResponse:
        host, port, path, scheme = config.endpoint
        try:
            encoded = json.dumps(body, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ProviderError(f"provider request is not JSON-safe: {error}") from error
        connection: http.client.HTTPConnection | http.client.HTTPSConnection
        connection = (
            http.client.HTTPSConnection(host, port, timeout=config.timeout_seconds)
            if scheme == "https"
            else http.client.HTTPConnection(host, port, timeout=config.timeout_seconds)
        )
        try:
            connection.request("POST", path, body=encoded, headers=dict(headers))
            response = connection.getresponse()
            raw = response.read(config.max_response_bytes + 1)
            status = response.status
            response_headers = {name.lower(): value for name, value in response.getheaders()}
        except (OSError, http.client.HTTPException) as error:
            # Do not include the exception text: proxies and providers can echo request headers.
            raise ProviderError(
                "provider transport failed; credential material was discarded",
                retryable=True,
            ) from error
        finally:
            connection.close()
        if len(raw) > config.max_response_bytes:
            raise ProviderError("provider response exceeded max_response_bytes")
        try:
            decoded = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ProviderError("provider returned a non-JSON response") from error
        if not isinstance(decoded, Mapping):
            raise ProviderError("provider response must be a JSON object")
        if status >= 400:
            # The body is intentionally not returned: an upstream error may reflect headers or
            # request content, and callers need a stable safe error rather than diagnostics.
            raise ProviderError(
                f"provider returned HTTP status {status}",
                retryable=status == 408 or status == 429 or status >= 500,
                status_code=status,
            )
        tool_calls = _extract_tool_calls(config.protocol, decoded)
        if tool_calls:
            allowed_tool_names = {tool.name for tool in request.tools}
            if not allowed_tool_names or any(call.name not in allowed_tool_names for call in tool_calls):
                raise ProviderError("provider returned an unrequested tool call")
            text = ""
            structured = None
        else:
            text = _extract_text(config.protocol, decoded)
            structured = _validate_structured_response(text, request)
        usage = decoded.get("usage")
        return ProviderResponse(
            provider=config.provider,
            model=str(decoded.get("model") or request.model),
            text=text,
            status_code=status,
            request_id=_header(response_headers, "x-request-id") or _string_or_none(decoded.get("id")),
            usage=dict(usage) if isinstance(usage, Mapping) else {},
            raw=dict(decoded),
            structured=structured,
            tool_calls=tool_calls,
        )


class ProviderOnboarding:
    """Explicit BYOK lifecycle for applications embedding the provider runtime.

    The onboarding object owns no additional secret state. It coordinates provider transport
    registration with one of the supported user-entry paths and returns only an opaque handle or
    redacted readiness metadata. A UI can use ``configure_from_prompt``'s injected reader, a
    server can use ``configure_from_environment``, and a deployment with a secret manager can use
    ``configure_from_resolver`` without placing a credential in MCP arguments or brain state.
    """

    _DEFAULT_ENVIRONMENT_VARIABLES = {
        "openai": "OPENAI_API_KEY",
        "anthropic": "ANTHROPIC_API_KEY",
    }

    def __init__(
        self,
        runtime: LLMRuntime,
        *,
        environment_variables: Mapping[str, str] | None = None,
    ) -> None:
        if not isinstance(runtime, LLMRuntime):
            raise CredentialError("ProviderOnboarding requires an LLMRuntime")
        self.runtime = runtime
        self._environment_variables = dict(self._DEFAULT_ENVIRONMENT_VARIABLES)
        if environment_variables is not None:
            for provider, variable in environment_variables.items():
                CredentialStore._validate_provider(provider)
                if not isinstance(variable, str) or not variable or not variable.replace("_", "").isalnum():
                    raise CredentialError("environment variable name must be alphanumeric with underscores")
                self._environment_variables[provider] = variable

    def register_provider(self, config: ProviderConfig) -> None:
        """Register non-secret provider transport metadata before collecting a key."""

        self.runtime.register_provider(config)

    def register_value(
        self,
        provider: str,
        value: str,
        *,
        ttl_seconds: float | None = None,
    ) -> CredentialHandle:
        self._require_provider(provider)
        return self.runtime.credentials.register(provider, value, ttl_seconds=ttl_seconds)

    def configure_from_prompt(
        self,
        provider: str,
        *,
        prompt: str | None = None,
        ttl_seconds: float | None = None,
        reader: Callable[[str], str] | None = None,
    ) -> CredentialHandle:
        self._require_provider(provider)
        return self.runtime.credentials.prompt(
            provider,
            prompt=prompt or f"{provider} API key: ",
            ttl_seconds=ttl_seconds,
            reader=reader,
        )

    def configure_from_environment(
        self,
        provider: str,
        *,
        variable: str | None = None,
        ttl_seconds: float | None = None,
        environ: Mapping[str, str] | None = None,
    ) -> CredentialHandle:
        self._require_provider(provider)
        selected_variable = variable or self._environment_variables.get(provider)
        if selected_variable is None:
            raise CredentialError(
                f"no default environment variable is registered for provider {provider!r}"
            )
        return self.runtime.credentials.register_environment(
            provider,
            selected_variable,
            ttl_seconds=ttl_seconds,
            environ=environ,
        )

    def configure_from_resolver(
        self,
        provider: str,
        reference: str,
        resolver: Callable[[str], str],
        *,
        ttl_seconds: float | None = None,
    ) -> CredentialHandle:
        self._require_provider(provider)
        return self.runtime.credentials.register_resolver(
            provider,
            reference,
            resolver,
            ttl_seconds=ttl_seconds,
        )

    def revoke(self, handle: CredentialHandle) -> None:
        self.runtime.credentials.revoke(handle)

    def status(self, provider: str) -> dict[str, Any]:
        CredentialStore._validate_provider(provider)
        registered = any(
            row.get("provider") == provider for row in self.runtime.provider_metadata()
        )
        credential = self.runtime.credentials.status(provider)
        return {
            "provider": provider,
            "provider_registered": registered,
            "credential": credential.to_dict(),
            "ready": registered and credential.configured,
            "next_action": "ready" if registered and credential.configured else (
                "register_provider" if not registered else "collect_user_credential"
            ),
            "secret_material": "never_returned",
        }

    def statuses(self) -> list[dict[str, Any]]:
        providers = {
            row.get("provider")
            for row in self.runtime.provider_metadata()
            if isinstance(row.get("provider"), str)
        }
        providers.update(status.provider for status in self.runtime.credentials.statuses())
        return [self.status(provider) for provider in sorted(providers)]

    def start_session(
        self,
        *,
        ttl_seconds: float | None = None,
        session_id: str | None = None,
        clock: Callable[[], float] = time.time,
    ) -> "CredentialSession":
        """Create a short-lived collection session for one UI/request lifecycle."""

        return CredentialSession(
            self,
            ttl_seconds=ttl_seconds,
            session_id=session_id,
            clock=clock,
        )

    def _require_provider(self, provider: str) -> None:
        CredentialStore._validate_provider(provider)
        if not any(row.get("provider") == provider for row in self.runtime.provider_metadata()):
            raise CredentialError(f"provider {provider!r} is not registered with the runtime")


@dataclass(frozen=True, slots=True)
class CredentialSessionStatus:
    """Redacted status for one caller-owned credential collection session."""

    session_id: str
    active: bool
    created_at: float
    expires_at: float | None
    providers: tuple[str, ...]
    secret_persistence: str = "in_memory_only"

    def to_dict(self) -> dict[str, Any]:
        return {
            "session_id": self.session_id,
            "active": self.active,
            "created_at": self.created_at,
            "expires_at": self.expires_at,
            "providers": list(self.providers),
            "secret_persistence": self.secret_persistence,
            "secret_material": "never_returned",
        }


class CredentialSession:
    """Short-lived BYOK session that groups opaque handles for revocation and readiness.

    The session retains only handles, never a key or external secret-manager reference. Closing
    or expiring it revokes every handle it created. Applications may keep the redacted status in
    UI state, but should recreate the session and resolve the secret again after a process restart.
    """

    def __init__(
        self,
        onboarding: ProviderOnboarding,
        *,
        ttl_seconds: float | None = None,
        session_id: str | None = None,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(onboarding, ProviderOnboarding):
            raise CredentialError("CredentialSession requires ProviderOnboarding")
        if ttl_seconds is not None and (
            not isinstance(ttl_seconds, (int, float))
            or isinstance(ttl_seconds, bool)
            or ttl_seconds <= 0
        ):
            raise CredentialError("session ttl_seconds must be positive or None")
        if not callable(clock):
            raise CredentialError("session clock must be callable")
        resolved_id = session_id or secrets.token_urlsafe(18)
        if (
            not isinstance(resolved_id, str)
            or not resolved_id.strip()
            or len(resolved_id) > 256
            or any(ord(character) < 32 for character in resolved_id)
        ):
            raise CredentialError("session_id must be a bounded non-empty string")
        self.onboarding = onboarding
        self.session_id = resolved_id
        self._clock = clock
        self.created_at = float(clock())
        self.expires_at = None if ttl_seconds is None else self.created_at + float(ttl_seconds)
        self._handles: dict[str, CredentialHandle] = {}
        self._closed = False
        self._lock = threading.RLock()

    def register_value(
        self,
        provider: str,
        value: str,
        *,
        ttl_seconds: float | None = None,
    ) -> CredentialHandle:
        handle = self._onboarded_call(
            self.onboarding.register_value,
            provider,
            value,
            ttl_seconds=ttl_seconds,
        )
        return self._attach(handle)

    def configure_from_prompt(
        self,
        provider: str,
        *,
        prompt: str | None = None,
        ttl_seconds: float | None = None,
        reader: Callable[[str], str] | None = None,
    ) -> CredentialHandle:
        handle = self._onboarded_call(
            self.onboarding.configure_from_prompt,
            provider,
            prompt=prompt,
            ttl_seconds=ttl_seconds,
            reader=reader,
        )
        return self._attach(handle)

    def configure_from_environment(
        self,
        provider: str,
        *,
        variable: str | None = None,
        ttl_seconds: float | None = None,
        environ: Mapping[str, str] | None = None,
    ) -> CredentialHandle:
        handle = self._onboarded_call(
            self.onboarding.configure_from_environment,
            provider,
            variable=variable,
            ttl_seconds=ttl_seconds,
            environ=environ,
        )
        return self._attach(handle)

    def configure_from_resolver(
        self,
        provider: str,
        reference: str,
        resolver: Callable[[str], str],
        *,
        ttl_seconds: float | None = None,
    ) -> CredentialHandle:
        handle = self._onboarded_call(
            self.onboarding.configure_from_resolver,
            provider,
            reference,
            resolver,
            ttl_seconds=ttl_seconds,
        )
        return self._attach(handle)

    def handle(self, provider: str) -> CredentialHandle:
        self._assert_active()
        CredentialStore._validate_provider(provider)
        with self._lock:
            handle = self._handles.get(provider)
        if handle is None:
            raise CredentialError(f"provider {provider!r} is not configured in this session")
        self.onboarding.runtime.credentials.metadata(handle)
        return handle

    def status(self) -> CredentialSessionStatus:
        active = self._is_active()
        with self._lock:
            providers = tuple(sorted(self._handles))
        return CredentialSessionStatus(
            session_id=self.session_id,
            active=active,
            created_at=self.created_at,
            expires_at=self.expires_at,
            providers=providers,
        )

    def provider_statuses(self) -> list[dict[str, Any]]:
        self._assert_active()
        with self._lock:
            providers = tuple(sorted(self._handles))
        return [self.onboarding.status(provider) for provider in providers]

    def handles(self) -> dict[str, CredentialHandle]:
        """Return a caller-owned snapshot for one bounded execution call.

        The returned mapping contains opaque handles only.  It is intentionally a snapshot so a
        concurrent revoke or session expiry cannot mutate a mapping already handed to a brain
        invocation; the runtime still revalidates every handle at the provider boundary.
        """

        self._assert_active()
        with self._lock:
            handles = dict(self._handles)
        for handle in handles.values():
            self.onboarding.runtime.credentials.metadata(handle)
        return handles

    def revoke(self, provider: str) -> None:
        self._assert_active()
        CredentialStore._validate_provider(provider)
        with self._lock:
            handle = self._handles.pop(provider, None)
        if handle is not None:
            self.onboarding.revoke(handle)

    def close(self) -> None:
        with self._lock:
            if self._closed:
                return
            handles = tuple(self._handles.values())
            self._handles.clear()
            self._closed = True
        for handle in handles:
            self.onboarding.revoke(handle)

    def __enter__(self) -> "CredentialSession":
        self._assert_active()
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def __repr__(self) -> str:
        return f"CredentialSession(session_id={self.session_id!r}, active={self._is_active()!r})"

    def _attach(self, handle: CredentialHandle) -> CredentialHandle:
        if not isinstance(handle, CredentialHandle):
            raise CredentialError("onboarding did not return a credential handle")
        try:
            self._assert_active()
        except CredentialError:
            self.onboarding.revoke(handle)
            raise
        with self._lock:
            previous = self._handles.get(handle.provider)
            self._handles[handle.provider] = handle
        if previous is not None and previous is not handle:
            self.onboarding.revoke(previous)
        return handle

    def _onboarded_call(self, callback: Callable[..., CredentialHandle], *args: Any, **kwargs: Any) -> CredentialHandle:
        self._assert_active()
        return callback(*args, **kwargs)

    def _is_active(self) -> bool:
        with self._lock:
            if self._closed:
                return False
            if self.expires_at is not None and self._clock() >= self.expires_at:
                self._closed = True
                handles = tuple(self._handles.values())
                self._handles.clear()
            else:
                handles = ()
        for handle in handles:
            self.onboarding.revoke(handle)
        return not self._closed

    def _assert_active(self) -> None:
        if not self._is_active():
            raise CredentialError("credential session is closed or expired")


def _bounded_json_bytes(value: Any, limit: int, label: str) -> int:
    try:
        encoded = json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
    except (TypeError, ValueError) as error:
        raise ProviderError(f"{label} must be JSON-safe") from error
    size = len(encoded.encode("utf-8"))
    if size > limit:
        raise ProviderError(f"{label} exceeds the bounded size")
    return size


def _validate_provider_message(message: Mapping[str, Any]) -> None:
    if not isinstance(message, Mapping) or not isinstance(message.get("role"), str):
        raise ProviderError("each message must contain a string role")
    content = message.get("content")
    if not isinstance(content, (str, Mapping, list, tuple)):
        raise ProviderError("each message content must be a bounded JSON value")
    _bounded_json_bytes(content, MAX_MESSAGE_CHARS, "provider message content")
    tool_call_id = message.get("tool_call_id")
    if tool_call_id is not None and (
        not isinstance(tool_call_id, str) or not tool_call_id.strip() or len(tool_call_id) > 256
    ):
        raise ProviderError("provider message tool_call_id is not bounded")
    tool_calls = message.get("tool_calls")
    if tool_calls is not None:
        if not isinstance(tool_calls, Sequence) or isinstance(tool_calls, (str, bytes)):
            raise ProviderError("provider message tool_calls must be a sequence")
        if len(tool_calls) > MAX_PROVIDER_TOOLS:
            raise ProviderError("provider message tool_calls exceed the bounded limit")
        for call in tool_calls:
            if not isinstance(call, Mapping):
                raise ProviderError("provider message tool call must be an object")
            name = call.get("name")
            call_id = call.get("id")
            arguments = call.get("arguments")
            if not isinstance(name, str) or not name.strip() or len(name) > MAX_TOOL_NAME_BYTES:
                raise ProviderError("provider message tool call name is not bounded")
            if not isinstance(call_id, str) or not call_id.strip() or len(call_id) > 256:
                raise ProviderError("provider message tool call id is not bounded")
            if not isinstance(arguments, str):
                raise ProviderError("provider message tool call arguments must be a string")
            _bounded_json_bytes(arguments, MAX_TOOL_ARGUMENT_BYTES, "provider message tool arguments")


def _wire_messages(
    protocol: str,
    source_messages: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Translate continuation markers into each provider's native conversation shape."""

    messages: list[dict[str, Any]] = []
    for source in source_messages:
        message = dict(source)
        role = message.get("role")
        tool_calls = message.get("tool_calls")
        if protocol == "openai_responses":
            if role == "assistant" and tool_calls:
                content = message.get("content")
                if isinstance(content, str) and content:
                    messages.append({"role": "assistant", "content": content})
                for call in tool_calls:
                    messages.append(
                        {
                            "type": "function_call",
                            "call_id": call["id"],
                            "name": call["name"],
                            "arguments": call["arguments"],
                        }
                    )
                continue
            if role == "tool":
                messages.append(
                    {
                        "type": "function_call_output",
                        "call_id": message["tool_call_id"],
                        "output": message["content"],
                    }
                )
                continue
            messages.append(message)
            continue
        if protocol == "anthropic_messages":
            if role == "assistant" and tool_calls:
                blocks: list[dict[str, Any]] = []
                content = message.get("content")
                if isinstance(content, str) and content:
                    blocks.append({"type": "text", "text": content})
                for call in tool_calls:
                    try:
                        arguments = json.loads(call["arguments"])
                    except (TypeError, ValueError) as error:
                        raise ProviderError("provider continuation tool arguments are invalid") from error
                    if not isinstance(arguments, Mapping):
                        raise ProviderError("provider continuation tool arguments must be an object")
                    blocks.append(
                        {
                            "type": "tool_use",
                            "id": call["id"],
                            "name": call["name"],
                            "input": dict(arguments),
                        }
                    )
                messages.append({"role": "assistant", "content": blocks})
                continue
            if role == "tool":
                messages.append(
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": message["tool_call_id"],
                                "content": message["content"],
                                "is_error": bool(message.get("is_error", False)),
                            }
                        ],
                    }
                )
                continue
            messages.append(message)
            continue
        if role == "assistant" and tool_calls:
            wire_calls = [
                {
                    "id": call["id"],
                    "type": "function",
                    "function": {
                        "name": call["name"],
                        "arguments": call["arguments"],
                    },
                }
                for call in tool_calls
            ]
            message["tool_calls"] = wire_calls
        messages.append(message)
    return messages


def _iter_sse_frames(
    response: http.client.HTTPResponse,
    max_response_bytes: int,
) -> Iterator[tuple[str | None, str]]:
    """Parse SSE framing with total, line, and event data bounds."""

    total_bytes = 0
    event_bytes = 0
    event_name: str | None = None
    data_lines: list[str] = []

    def flush() -> tuple[str | None, str] | None:
        nonlocal event_bytes, event_name, data_lines
        if not data_lines:
            event_name = None
            event_bytes = 0
            return None
        data = "\n".join(data_lines)
        result = (event_name, data)
        event_name = None
        data_lines = []
        event_bytes = 0
        return result

    while True:
        line = response.readline()
        if not line:
            break
        total_bytes += len(line)
        if total_bytes > max_response_bytes:
            raise ProviderError("provider stream exceeded max_response_bytes")
        if len(line) > MAX_STREAM_EVENT_BYTES:
            raise ProviderError("provider stream line exceeded the bounded size")
        stripped = line.rstrip(b"\r\n")
        if not stripped:
            frame = flush()
            if frame is not None:
                yield frame
            continue
        if stripped.startswith(b":"):
            continue
        try:
            decoded = stripped.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ProviderError("provider stream contained invalid UTF-8") from error
        field, separator, value = decoded.partition(":")
        if not separator:
            continue
        if value.startswith(" "):
            value = value[1:]
        if field == "event":
            if len(value) > 256:
                raise ProviderError("provider stream event name exceeded the bounded size")
            event_name = value
        elif field == "data":
            event_bytes += len(value.encode("utf-8"))
            if event_bytes > MAX_STREAM_EVENT_BYTES:
                raise ProviderError("provider stream event exceeded the bounded size")
            data_lines.append(value)
    frame = flush()
    if frame is not None:
        yield frame


def _stream_meta(payload: Mapping[str, Any], state: dict[str, Any]) -> None:
    response = payload.get("response")
    if isinstance(response, Mapping):
        if isinstance(response.get("id"), str):
            state["request_id"] = response["id"]
        if isinstance(response.get("model"), str):
            state["model"] = response["model"]
    for key in ("id", "model"):
        if isinstance(payload.get(key), str):
            state["request_id" if key == "id" else "model"] = payload[key]


def _parse_stream_arguments(value: Any) -> Mapping[str, Any]:
    if isinstance(value, Mapping):
        arguments = value
    elif isinstance(value, str):
        if len(value.encode("utf-8")) > MAX_TOOL_ARGUMENT_BYTES:
            raise ProviderError("provider streamed tool arguments exceed the bounded size")
        try:
            arguments = json.loads(value or "{}")
        except (TypeError, ValueError) as error:
            raise ProviderError("provider streamed tool arguments were not valid JSON") from error
    else:
        raise ProviderError("provider streamed tool arguments were malformed")
    if not isinstance(arguments, Mapping):
        raise ProviderError("provider streamed tool arguments must be an object")
    _bounded_json_bytes(arguments, MAX_TOOL_ARGUMENT_BYTES, "provider streamed tool arguments")
    return dict(arguments)


def _append_stream_fragment(current: Any, fragment: Any, limit: int, label: str) -> str:
    if not isinstance(current, str) or not isinstance(fragment, str):
        raise ProviderError(f"{label} was malformed")
    combined = current + fragment
    if len(combined.encode("utf-8")) > limit:
        raise ProviderError(f"{label} exceeded the bounded size")
    return combined


def _project_stream_payload(
    protocol: str,
    event_name: str | None,
    payload: Mapping[str, Any],
    state: dict[str, Any],
) -> list[dict[str, Any]]:
    event_type = event_name or _string_or_none(payload.get("type")) or "provider.event"
    _stream_meta(payload, state)
    specs: list[dict[str, Any]] = []
    handled = False
    if protocol == "openai_responses":
        response = payload.get("response")
        if isinstance(response, Mapping):
            _stream_meta(response, state)
        if event_type == "response.output_text.delta" and isinstance(payload.get("delta"), str):
            specs.append({"event_type": event_type, "text_delta": payload["delta"]})
            handled = True
        elif event_type == "response.output_item.added":
            item = payload.get("item")
            if isinstance(item, Mapping) and item.get("type") == "function_call":
                key = str(item.get("id") or payload.get("item_id") or len(state["calls"]))
                state["calls"][key] = {
                    "call_id": item.get("call_id") or item.get("id") or key,
                    "name": item.get("name"),
                    "arguments": "",
                }
            handled = True
        elif event_type == "response.function_call_arguments.delta":
            item_id = str(payload.get("item_id") or payload.get("id") or len(state["calls"]))
            call = state["calls"].setdefault(
                item_id,
                {"call_id": payload.get("call_id") or item_id, "name": payload.get("name"), "arguments": ""},
            )
            delta = payload.get("delta", "")
            if not isinstance(delta, str):
                raise ProviderError("provider streamed function arguments delta was malformed")
            call["arguments"] = _append_stream_fragment(
                call.get("arguments", ""),
                delta,
                MAX_TOOL_ARGUMENT_BYTES,
                "provider streamed function arguments",
            )
            specs.append(
                {
                    "event_type": event_type,
                    "tool_call_id": str(call["call_id"]),
                    "tool_name": _string_or_none(call.get("name")),
                    "arguments_delta": delta,
                }
            )
            handled = True
        elif event_type == "response.function_call_arguments.done":
            item_id = str(payload.get("item_id") or payload.get("id") or len(state["calls"]))
            call = state["calls"].setdefault(
                item_id,
                {"call_id": payload.get("call_id") or item_id, "name": payload.get("name"), "arguments": ""},
            )
            if isinstance(payload.get("arguments"), (str, Mapping)):
                call["arguments"] = payload["arguments"]
            if isinstance(payload.get("name"), str):
                call["name"] = payload["name"]
            specs.extend(_finalize_stream_tool_calls(protocol, state, only=item_id))
            handled = True
        elif event_type in {"response.completed", "response.done"}:
            usage = {}
            if isinstance(response, Mapping) and isinstance(response.get("usage"), Mapping):
                usage = dict(response["usage"])
            elif isinstance(payload.get("usage"), Mapping):
                usage = dict(payload["usage"])
            specs.extend(_finalize_stream_tool_calls(protocol, state))
            specs.append({"event_type": event_type, "usage": usage, "done": True})
            handled = True
    elif protocol == "openai_chat_completions":
        choices = payload.get("choices")
        if isinstance(choices, list) and choices and isinstance(choices[0], Mapping):
            choice = choices[0]
            delta = choice.get("delta")
            if not isinstance(delta, Mapping):
                delta = choice.get("message") if isinstance(choice.get("message"), Mapping) else {}
            content = delta.get("content") if isinstance(delta, Mapping) else None
            if isinstance(content, str) and content:
                specs.append({"event_type": event_type, "text_delta": content})
                handled = True
            chunks = delta.get("tool_calls") if isinstance(delta, Mapping) else None
            if isinstance(chunks, list):
                for chunk in chunks:
                    if not isinstance(chunk, Mapping):
                        continue
                    arguments_delta = ""
                    index = str(chunk.get("index", len(state["calls"])))
                    call = state["calls"].setdefault(
                        index,
                        {"call_id": chunk.get("id") or f"tool-call-{index}", "name": None, "arguments": ""},
                    )
                    if isinstance(chunk.get("id"), str):
                        call["call_id"] = chunk["id"]
                    function = chunk.get("function")
                    if isinstance(function, Mapping):
                        if isinstance(function.get("name"), str):
                            call["name"] = function["name"]
                        arguments_delta = function.get("arguments", "")
                        if not isinstance(arguments_delta, str):
                            raise ProviderError("provider streamed chat tool arguments were malformed")
                        call["arguments"] = _append_stream_fragment(
                            call.get("arguments", ""),
                            arguments_delta,
                            MAX_TOOL_ARGUMENT_BYTES,
                            "provider streamed chat tool arguments",
                        )
                    specs.append(
                        {
                            "event_type": event_type,
                            "tool_call_id": str(call["call_id"]),
                            "tool_name": _string_or_none(call.get("name")),
                            "arguments_delta": arguments_delta if isinstance(arguments_delta, str) else "",
                        }
                    )
                    handled = True
            finish_reason = choice.get("finish_reason")
            if finish_reason == "tool_calls":
                specs.extend(_finalize_stream_tool_calls(protocol, state))
                specs.append({"event_type": event_type, "done": True})
                handled = True
            elif finish_reason is not None:
                specs.append({"event_type": event_type, "done": True})
                handled = True
        if isinstance(payload.get("usage"), Mapping):
            specs.append({"event_type": event_type, "usage": dict(payload["usage"])})
            handled = True
    else:
        if event_type == "message_start":
            message = payload.get("message")
            if isinstance(message, Mapping):
                if isinstance(message.get("id"), str):
                    state["request_id"] = message["id"]
                if isinstance(message.get("model"), str):
                    state["model"] = message["model"]
                if isinstance(message.get("usage"), Mapping):
                    specs.append({"event_type": event_type, "usage": dict(message["usage"])})
            handled = True
        elif event_type == "content_block_start":
            block = payload.get("content_block")
            index = str(payload.get("index", len(state["calls"])))
            if isinstance(block, Mapping) and block.get("type") == "tool_use":
                state["calls"][index] = {
                    "call_id": block.get("id") or f"tool-call-{index}",
                    "name": block.get("name"),
                    "arguments": "",
                }
            handled = True
        elif event_type == "content_block_delta":
            index = str(payload.get("index", len(state["calls"])))
            delta = payload.get("delta")
            if isinstance(delta, Mapping) and delta.get("type") == "text_delta":
                text = delta.get("text", "")
                if not isinstance(text, str):
                    raise ProviderError("provider streamed Anthropic text delta was malformed")
                specs.append({"event_type": event_type, "text_delta": text})
            elif isinstance(delta, Mapping) and delta.get("type") == "input_json_delta":
                call = state["calls"].setdefault(
                    index,
                    {"call_id": f"tool-call-{index}", "name": None, "arguments": ""},
                )
                partial = delta.get("partial_json", "")
                if not isinstance(partial, str):
                    raise ProviderError("provider streamed Anthropic tool arguments were malformed")
                call["arguments"] = _append_stream_fragment(
                    call.get("arguments", ""),
                    partial,
                    MAX_TOOL_ARGUMENT_BYTES,
                    "provider streamed Anthropic tool arguments",
                )
                specs.append(
                    {
                        "event_type": event_type,
                        "tool_call_id": str(call["call_id"]),
                        "tool_name": _string_or_none(call.get("name")),
                        "arguments_delta": partial,
                    }
                )
            handled = True
        elif event_type == "message_delta":
            usage = payload.get("usage")
            if isinstance(usage, Mapping):
                specs.append({"event_type": event_type, "usage": dict(usage)})
            handled = True
        elif event_type == "message_stop":
            specs.extend(_finalize_stream_tool_calls(protocol, state))
            specs.append({"event_type": event_type, "done": True})
            handled = True
    if not handled:
        specs.append({"event_type": event_type})
    return specs


def _finalize_stream_tool_calls(
    protocol: str,
    state: dict[str, Any],
    *,
    only: str | None = None,
) -> list[dict[str, Any]]:
    specs: list[dict[str, Any]] = []
    keys = [only] if only is not None else list(state["calls"])
    for key in keys:
        if key is None or key not in state["calls"]:
            continue
        call = state["calls"].pop(key)
        name = call.get("name")
        if not isinstance(name, str) or not name.strip():
            raise ProviderError("provider stream completed a tool call without a name")
        arguments = _parse_stream_arguments(call.get("arguments", "{}"))
        parsed = ProviderToolCall(
            call_id=str(call.get("call_id") or key),
            name=name,
            arguments=arguments,
        )
        specs.append(
            {
                "event_type": "provider.tool_call.done",
                "tool_call_id": parsed.call_id,
                "tool_name": parsed.name,
                "tool_call": parsed,
            }
        )
    return specs


def _header(headers: Mapping[str, str], name: str) -> str | None:
    value = headers.get(name.lower())
    return value if value else None


def _string_or_none(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None


def _extract_text(protocol: str, payload: Mapping[str, Any]) -> str:
    if protocol == "openai_responses":
        direct = payload.get("output_text")
        if isinstance(direct, str):
            return direct
        output = payload.get("output")
        if isinstance(output, list):
            pieces: list[str] = []
            for item in output:
                if not isinstance(item, Mapping):
                    continue
                content = item.get("content")
                if isinstance(content, list):
                    for block in content:
                        if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                            pieces.append(block["text"])
            if pieces:
                return "".join(pieces)
    elif protocol == "anthropic_messages":
        content = payload.get("content")
        if isinstance(content, list):
            pieces = [
                block["text"]
                for block in content
                if isinstance(block, Mapping) and block.get("type") == "text" and isinstance(block.get("text"), str)
            ]
            if pieces:
                return "".join(pieces)
    else:
        choices = payload.get("choices")
        if isinstance(choices, list) and choices and isinstance(choices[0], Mapping):
            message = choices[0].get("message")
            if isinstance(message, Mapping):
                content = message.get("content")
                if isinstance(content, str):
                    return content
                if isinstance(content, list):
                    return "".join(
                        block["text"]
                        for block in content
                        if isinstance(block, Mapping) and isinstance(block.get("text"), str)
                    )
    raise ProviderError("provider response contained no assistant text")


def _extract_tool_calls(
    protocol: str,
    payload: Mapping[str, Any],
) -> tuple[ProviderToolCall, ...]:
    """Parse provider-native function calls without ever dispatching them."""

    candidates: list[Mapping[str, Any]] = []
    if protocol == "openai_responses":
        output = payload.get("output")
        if isinstance(output, list):
            candidates = [
                item
                for item in output
                if isinstance(item, Mapping) and item.get("type") == "function_call"
            ]
    elif protocol == "anthropic_messages":
        content = payload.get("content")
        if isinstance(content, list):
            candidates = [
                item
                for item in content
                if isinstance(item, Mapping) and item.get("type") == "tool_use"
            ]
    else:
        choices = payload.get("choices")
        if isinstance(choices, list) and choices and isinstance(choices[0], Mapping):
            message = choices[0].get("message")
            calls = message.get("tool_calls") if isinstance(message, Mapping) else None
            if isinstance(calls, list):
                candidates = [item for item in calls if isinstance(item, Mapping)]
    if len(candidates) > MAX_PROVIDER_TOOLS:
        raise ProviderError("provider returned too many tool calls")

    parsed: list[ProviderToolCall] = []
    for index, candidate in enumerate(candidates):
        if protocol == "openai_chat_completions":
            function = candidate.get("function")
            if not isinstance(function, Mapping):
                raise ProviderError("provider returned a malformed tool call")
            name = function.get("name")
            arguments = function.get("arguments")
            call_id = candidate.get("id") or f"tool-call-{index}"
        elif protocol == "anthropic_messages":
            name = candidate.get("name")
            arguments = candidate.get("input")
            call_id = candidate.get("id") or f"tool-call-{index}"
        else:
            name = candidate.get("name")
            arguments = candidate.get("arguments")
            call_id = candidate.get("call_id") or candidate.get("id") or f"tool-call-{index}"
        if not isinstance(name, str) or not name.strip():
            raise ProviderError("provider returned a tool call without a name")
        if isinstance(arguments, str):
            if len(arguments.encode("utf-8")) > MAX_TOOL_ARGUMENT_BYTES:
                raise ProviderError("provider tool call arguments exceed the bounded size")
            try:
                arguments = json.loads(arguments)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise ProviderError("provider returned invalid JSON tool arguments") from error
        if not isinstance(arguments, Mapping):
            raise ProviderError("provider tool call arguments must be a JSON object")
        parsed.append(
            ProviderToolCall(
                call_id=str(call_id),
                name=name,
                arguments=dict(arguments),
            )
        )
    return tuple(parsed)


def _validate_structured_response(
    text: str,
    request: ProviderRequest,
) -> Any:
    """Parse and validate a bounded JSON response without echoing response contents in errors."""

    if not request.require_json and request.response_schema is None:
        return None
    try:
        value = json.loads(text)
    except (TypeError, ValueError) as error:
        raise ProviderError("provider response was not valid JSON") from error
    if request.response_schema is not None:
        _validate_json_schema(value, request.response_schema, "$")
    return value


def _validate_json_schema(value: Any, schema: Mapping[str, Any], path: str) -> None:
    """Validate the deliberately small, dependency-free JSON Schema subset used by the brain."""

    schema_type = schema.get("type")
    if schema_type is not None:
        allowed_types = [schema_type] if isinstance(schema_type, str) else schema_type
        if not isinstance(allowed_types, list) or not all(isinstance(item, str) for item in allowed_types):
            raise ProviderError("structured-output schema has an invalid type declaration")
        if not any(_json_type_matches(value, item) for item in allowed_types):
            raise ProviderError("provider response failed structured-output validation")
    if "enum" in schema:
        enum = schema["enum"]
        if not isinstance(enum, list) or value not in enum:
            raise ProviderError("provider response failed structured-output validation")
    if isinstance(value, Mapping):
        required = schema.get("required", [])
        if not isinstance(required, list) or any(not isinstance(item, str) for item in required):
            raise ProviderError("structured-output schema has invalid required fields")
        if any(field not in value for field in required):
            raise ProviderError("provider response failed structured-output validation")
        properties = schema.get("properties", {})
        if properties is not None and not isinstance(properties, Mapping):
            raise ProviderError("structured-output schema has invalid properties")
        if isinstance(properties, Mapping):
            if schema.get("additionalProperties") is False and any(field not in properties for field in value):
                raise ProviderError("provider response failed structured-output validation")
            for field, child_schema in properties.items():
                if field in value:
                    if not isinstance(child_schema, Mapping):
                        raise ProviderError("structured-output schema has invalid child schema")
                    _validate_json_schema(value[field], child_schema, f"{path}.{field}")
    if isinstance(value, list):
        if "minItems" in schema and (not isinstance(schema["minItems"], int) or len(value) < schema["minItems"]):
            raise ProviderError("provider response failed structured-output validation")
        if "maxItems" in schema and (not isinstance(schema["maxItems"], int) or len(value) > schema["maxItems"]):
            raise ProviderError("provider response failed structured-output validation")
        items = schema.get("items")
        if items is not None:
            if not isinstance(items, Mapping):
                raise ProviderError("structured-output schema has invalid array items")
            for index, item in enumerate(value):
                _validate_json_schema(item, items, f"{path}[{index}]")
    if isinstance(value, str):
        if "minLength" in schema and (not isinstance(schema["minLength"], int) or len(value) < schema["minLength"]):
            raise ProviderError("provider response failed structured-output validation")
        if "maxLength" in schema and (not isinstance(schema["maxLength"], int) or len(value) > schema["maxLength"]):
            raise ProviderError("provider response failed structured-output validation")


def _json_type_matches(value: Any, schema_type: str) -> bool:
    return {
        "object": isinstance(value, Mapping),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "number": isinstance(value, (int, float)) and not isinstance(value, bool),
        "integer": isinstance(value, int) and not isinstance(value, bool),
        "boolean": isinstance(value, bool),
        "null": value is None,
    }.get(schema_type, False)


def openai_provider(
    *,
    base_url: str = "https://api.openai.com",
    path: str | None = None,
    timeout_seconds: float = 60.0,
    allow_insecure_http: bool = False,
    max_attempts: int = 1,
    retry_backoff_seconds: float = 0.0,
    circuit_breaker_failure_threshold: int = 3,
    circuit_breaker_reset_seconds: float = 30.0,
) -> ProviderConfig:
    """Create a metadata-only OpenAI Responses provider configuration."""

    return ProviderConfig(
        provider="openai",
        base_url=base_url,
        protocol="openai_responses",
        path=path,
        timeout_seconds=timeout_seconds,
        allow_insecure_http=allow_insecure_http,
        max_attempts=max_attempts,
        retry_backoff_seconds=retry_backoff_seconds,
        circuit_breaker_failure_threshold=circuit_breaker_failure_threshold,
        circuit_breaker_reset_seconds=circuit_breaker_reset_seconds,
    )


def anthropic_provider(
    *,
    base_url: str = "https://api.anthropic.com",
    timeout_seconds: float = 60.0,
    allow_insecure_http: bool = False,
    max_attempts: int = 1,
    retry_backoff_seconds: float = 0.0,
    circuit_breaker_failure_threshold: int = 3,
    circuit_breaker_reset_seconds: float = 30.0,
) -> ProviderConfig:
    """Create a metadata-only Anthropic Messages provider configuration."""

    return ProviderConfig(
        provider="anthropic",
        base_url=base_url,
        protocol="anthropic_messages",
        timeout_seconds=timeout_seconds,
        allow_insecure_http=allow_insecure_http,
        max_attempts=max_attempts,
        retry_backoff_seconds=retry_backoff_seconds,
        circuit_breaker_failure_threshold=circuit_breaker_failure_threshold,
        circuit_breaker_reset_seconds=circuit_breaker_reset_seconds,
    )


def openai_compatible_provider(
    provider: str,
    base_url: str,
    *,
    timeout_seconds: float = 60.0,
    allow_insecure_http: bool = False,
    max_attempts: int = 1,
    retry_backoff_seconds: float = 0.0,
    circuit_breaker_failure_threshold: int = 3,
    circuit_breaker_reset_seconds: float = 30.0,
) -> ProviderConfig:
    """Configure a provider exposing the OpenAI Chat Completions wire shape."""

    return ProviderConfig(
        provider=provider,
        base_url=base_url,
        protocol="openai_chat_completions",
        timeout_seconds=timeout_seconds,
        allow_insecure_http=allow_insecure_http,
        max_attempts=max_attempts,
        retry_backoff_seconds=retry_backoff_seconds,
        circuit_breaker_failure_threshold=circuit_breaker_failure_threshold,
        circuit_breaker_reset_seconds=circuit_breaker_reset_seconds,
    )
