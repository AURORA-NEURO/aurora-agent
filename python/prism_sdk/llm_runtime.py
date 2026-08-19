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

from dataclasses import dataclass
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
SUPPORTED_PROTOCOLS = {
    "openai_responses",
    "openai_chat_completions",
    "anthropic_messages",
}


class CredentialError(ValueError):
    """A credential was missing, invalid, expired, revoked, or used with the wrong provider."""


class ProviderError(RuntimeError):
    """A provider call failed without retaining or exposing the credential."""


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

    def to_metadata(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "credential_id": self.credential_id,
            "credential_present": True,
            "secret_persistence": "in_memory_only",
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

    def register(self, provider: str, secret: str, *, ttl_seconds: float | None = None) -> CredentialHandle:
        self._validate_provider(provider)
        if not isinstance(secret, str) or not secret.strip():
            raise CredentialError("credential value must be a non-empty string")
        if ttl_seconds is not None and (not isinstance(ttl_seconds, (int, float)) or ttl_seconds <= 0):
            raise CredentialError("ttl_seconds must be positive or None")
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
        return self.register(provider, value, ttl_seconds=ttl_seconds)

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
        return self.register(provider, value, ttl_seconds=ttl_seconds)

    def revoke(self, handle: CredentialHandle) -> None:
        self._assert_handle(handle)
        with self._lock:
            self._entries.pop(handle.credential_id, None)

    def clear(self) -> None:
        with self._lock:
            self._entries.clear()

    def metadata(self, handle: CredentialHandle) -> dict[str, Any]:
        self._resolve(handle)
        return handle.to_metadata()

    def _assert_handle(self, handle: CredentialHandle) -> None:
        if not isinstance(handle, CredentialHandle) or handle._store is not self:
            raise CredentialError("credential handle belongs to a different store")

    def _resolve(self, handle: CredentialHandle) -> SecretValue:
        self._assert_handle(handle)
        with self._lock:
            self._purge_expired_locked()
            entry = self._entries.get(handle.credential_id)
            if entry is None:
                raise CredentialError("credential handle is unknown, revoked, or expired")
            if entry.provider != handle.provider:
                raise CredentialError("credential handle provider mismatch")
            return entry.secret

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
        }


@dataclass(frozen=True, slots=True)
class ProviderRequest:
    model: str
    messages: tuple[Mapping[str, Any], ...]
    max_output_tokens: int = 1024
    temperature: float | None = None

    def __post_init__(self) -> None:
        if not self.model or len(self.messages) > MAX_MESSAGES:
            raise ProviderError("model and messages are required within their bounds")
        if self.max_output_tokens <= 0:
            raise ProviderError("max_output_tokens must be positive")
        if self.temperature is not None and not 0 <= self.temperature <= 2:
            raise ProviderError("temperature must be within [0, 2]")
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

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "model": self.model,
            "text": self.text,
            "status_code": self.status_code,
            "request_id": self.request_id,
            "usage": dict(self.usage),
            "raw": dict(self.raw),
            "credential_posture": "not_in_response",
        }


class LLMRuntime:
    """Invoke configured providers while resolving secrets only at the header boundary."""

    def __init__(self, credentials: CredentialStore | None = None) -> None:
        self.credentials = credentials or CredentialStore()
        self._providers: dict[str, ProviderConfig] = {}

    def register_provider(self, config: ProviderConfig) -> None:
        self._providers[config.provider] = config

    def provider_metadata(self) -> list[dict[str, Any]]:
        return [self._providers[name].to_metadata() for name in sorted(self._providers)]

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
        return self._post(config, body, headers, request.model)

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
        return body

    def _post(
        self,
        config: ProviderConfig,
        body: Mapping[str, Any],
        headers: Mapping[str, str],
        model: str,
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
            raise ProviderError("provider transport failed; credential material was discarded") from error
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
            raise ProviderError(f"provider returned HTTP status {status}")
        text = _extract_text(config.protocol, decoded)
        usage = decoded.get("usage")
        return ProviderResponse(
            provider=config.provider,
            model=str(decoded.get("model") or model),
            text=text,
            status_code=status,
            request_id=_header(response_headers, "x-request-id") or _string_or_none(decoded.get("id")),
            usage=dict(usage) if isinstance(usage, Mapping) else {},
            raw=dict(decoded),
        )


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


def openai_provider(
    *,
    base_url: str = "https://api.openai.com",
    path: str | None = None,
    timeout_seconds: float = 60.0,
    allow_insecure_http: bool = False,
) -> ProviderConfig:
    """Create a metadata-only OpenAI Responses provider configuration."""

    return ProviderConfig(
        provider="openai",
        base_url=base_url,
        protocol="openai_responses",
        path=path,
        timeout_seconds=timeout_seconds,
        allow_insecure_http=allow_insecure_http,
    )


def anthropic_provider(
    *,
    base_url: str = "https://api.anthropic.com",
    timeout_seconds: float = 60.0,
    allow_insecure_http: bool = False,
) -> ProviderConfig:
    """Create a metadata-only Anthropic Messages provider configuration."""

    return ProviderConfig(
        provider="anthropic",
        base_url=base_url,
        protocol="anthropic_messages",
        timeout_seconds=timeout_seconds,
        allow_insecure_http=allow_insecure_http,
    )


def openai_compatible_provider(
    provider: str,
    base_url: str,
    *,
    timeout_seconds: float = 60.0,
    allow_insecure_http: bool = False,
) -> ProviderConfig:
    """Configure a provider exposing the OpenAI Chat Completions wire shape."""

    return ProviderConfig(
        provider=provider,
        base_url=base_url,
        protocol="openai_chat_completions",
        timeout_seconds=timeout_seconds,
        allow_insecure_http=allow_insecure_http,
    )
