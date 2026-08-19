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

from dataclasses import dataclass, field
import getpass
import http.client
import json
import os
import secrets
import threading
import time
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import urlsplit


MAX_MESSAGES = 512
MAX_MESSAGE_CHARS = 2_000_000
MAX_RESPONSE_BYTES = 20_000_000
MAX_PROVIDER_TOOLS = 128
MAX_TOOL_NAME_BYTES = 256
MAX_TOOL_ARGUMENT_BYTES = 1_000_000
SUPPORTED_PROTOCOLS = {
    "openai_responses",
    "openai_chat_completions",
    "anthropic_messages",
}


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
            if not isinstance(message, Mapping) or not isinstance(message.get("role"), str):
                raise ProviderError("each message must contain a string role")
            content = message.get("content")
            if not isinstance(content, str) or len(content) > MAX_MESSAGE_CHARS:
                raise ProviderError("each message content must be a bounded string")


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
    ) -> None:
        self.credentials = credentials or CredentialStore()
        self._providers: dict[str, ProviderConfig] = {}
        self._circuits: dict[str, _CircuitState] = {}
        self._clock = clock
        self._sleeper = sleeper

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
        return self._post(config, body, headers, request)

    @staticmethod
    def _body(config: ProviderConfig, request: ProviderRequest) -> dict[str, Any]:
        messages = [dict(message) for message in request.messages]
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
                if message.get("role") == "system"
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

    def _require_provider(self, provider: str) -> None:
        CredentialStore._validate_provider(provider)
        if not any(row.get("provider") == provider for row in self.runtime.provider_metadata()):
            raise CredentialError(f"provider {provider!r} is not registered with the runtime")


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
