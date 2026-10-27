"""Application-owned, account-bound workflow credential sessions.

This repository-defined module has no available source blueprint id. It reuses the in-memory
CredentialStore without registering a service as an LLM provider. It performs no network calls,
OAuth, identity authentication, secret persistence, or private-response retention policy.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
import re
import secrets
import threading
import time
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import urlsplit

from .authoring import canonical_json
from .llm_runtime import CredentialHandle, CredentialStore


WORKFLOW_CONNECTION_SCHEMA = "aurora-workflow-connection/0.1"
_IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$")
_HEADER = re.compile(r"^[A-Za-z0-9-]{1,128}$")
_FORBIDDEN_HEADERS = {"host", "connection", "content-length", "transfer-encoding", "accept"}


class WorkflowConnectionError(ValueError):
    """A session cannot establish the exact application-approved connection boundary."""


def _identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or not _IDENTIFIER.fullmatch(value):
        raise WorkflowConnectionError(f"{label} must be a bounded non-secret identifier")
    return value


@dataclass(frozen=True)
class WorkflowConnectionContext:
    """Non-secret application subject identifiers; these are not authenticated by this SDK."""

    tenant_id: str
    actor_id: str
    session_id: str

    def __post_init__(self) -> None:
        for name in ("tenant_id", "actor_id", "session_id"):
            _identifier(getattr(self, name), name)

    def to_dict(self) -> dict[str, str]:
        return {name: getattr(self, name) for name in ("tenant_id", "actor_id", "session_id")}


@dataclass(frozen=True)
class WorkflowConnectionResource:
    """An operation and exact permitted scalar argument values, never a URL prefix grant.

    Import requires every path parameter to be constrained here. Empty constraints admit only
    an explicitly selected static route. Optional constrained arguments must be supplied.
    """

    operation_id: str
    argument_values: Mapping[str, Sequence[Any]]
    _arguments_json: str = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        _identifier(self.operation_id, "resource operation_id")
        if not isinstance(self.argument_values, Mapping) or len(self.argument_values) > 64:
            raise WorkflowConnectionError("resource argument_values must be a bounded object")
        safe = {}
        for name, values in self.argument_values.items():
            _identifier(name, "resource argument name")
            if isinstance(values, (str, bytes)) or not isinstance(values, Sequence) or not 1 <= len(values) <= 128:
                raise WorkflowConnectionError("resource values must be a bounded nonempty sequence")
            checked = []
            for value in values:
                if (type(value) not in {str, int, float, bool}
                        or isinstance(value, str) and (not value or len(value.encode("utf-8")) > 512)
                        or isinstance(value, float) and not math.isfinite(value)):
                    raise WorkflowConnectionError("resource values must be bounded finite JSON scalars")
                if len(canonical_json(value).encode("utf-8")) > 512:
                    raise WorkflowConnectionError("resource scalar exceeds its byte bound")
                checked.append(value)
            if len({canonical_json(value) for value in checked}) != len(checked):
                raise WorkflowConnectionError("resource values must be unique with strict JSON types")
            safe[name] = checked
        encoded = canonical_json(safe)
        if len(encoded.encode("utf-8")) > 32_000:
            raise WorkflowConnectionError("resource constraints exceed their byte bound")
        object.__setattr__(self, "argument_values", json.loads(encoded))
        object.__setattr__(self, "_arguments_json", encoded)

    def to_dict(self) -> dict[str, Any]:
        return {"operation_id": self.operation_id, "argument_values": json.loads(self._arguments_json)}


class WorkflowConnectionSession:
    """One immutable service/account/subject/resource scope and revocable credential handle.

    Account observation is application attestation, not provider verification. Authorizing this
    session does not authorize an individual workflow dispatch. A new account, permission set,
    credential handle or application subject requires a new session and therefore a new binding.
    """

    __slots__ = ("_service_id", "_context", "_expected_account", "_store", "_credential",
                 "_handle_identity", "_server", "_resources", "_auth_policy", "_header_name",
                 "_expires_at", "_clock", "_nonce", "_lock", "_closed", "_revoked",
                 "_account_state", "_authorization_state", "_failure", "_configuration")

    def __init__(self, *, service_id: str, context: WorkflowConnectionContext,
                 expected_account_id: str, credential_store: CredentialStore,
                 credential: CredentialHandle, server_url: str,
                 resources: Sequence[WorkflowConnectionResource], ttl_seconds: float = 900,
                 clock: Callable[[], float] = time.time, auth_policy: str = "bearer_header",
                 header_name: str = "Authorization") -> None:
        self._service_id = _identifier(service_id, "service_id")
        if not isinstance(context, WorkflowConnectionContext):
            raise WorkflowConnectionError("context must be WorkflowConnectionContext")
        self._expected_account = _identifier(expected_account_id, "expected_account_id")
        if not isinstance(credential_store, CredentialStore) or not isinstance(credential, CredentialHandle):
            raise WorkflowConnectionError("connection requires an existing CredentialStore handle")
        if credential.provider != service_id:
            raise WorkflowConnectionError("credential service does not match the fixed connection service")
        try:
            credential_store.metadata(credential)
        except Exception:
            raise WorkflowConnectionError("connection credential is unavailable") from None
        try:
            parsed = urlsplit(server_url) if isinstance(server_url, str) else None
            if parsed is not None:
                parsed.port
        except ValueError:
            raise WorkflowConnectionError("connection server must be a valid fixed HTTPS base URL") from None
        if (parsed is None or parsed.scheme != "https" or not parsed.hostname or parsed.username
                or parsed.password or parsed.query or parsed.fragment or "{" in server_url
                or "}" in server_url or len(server_url) > 2048):
            raise WorkflowConnectionError("connection server must be a fixed HTTPS base URL")
        if (isinstance(resources, (str, bytes)) or not isinstance(resources, Sequence)
                or not 1 <= len(resources) <= 128
                or any(not isinstance(item, WorkflowConnectionResource) for item in resources)):
            raise WorkflowConnectionError("resources must be explicit bounded typed operation scopes")
        if len({item.operation_id for item in resources}) != len(resources):
            raise WorkflowConnectionError("each connection operation requires one unambiguous resource scope")
        if (type(auth_policy) is not str or auth_policy not in {"bearer_header", "api_key_header"}
                or not isinstance(header_name, str) or not _HEADER.fullmatch(header_name)
                or header_name.lower() in _FORBIDDEN_HEADERS
                or auth_policy == "bearer_header" and header_name.lower() != "authorization"):
            raise WorkflowConnectionError("connection authentication policy or header is unsupported")
        if (isinstance(ttl_seconds, bool) or not isinstance(ttl_seconds, (int, float))
                or not math.isfinite(ttl_seconds) or not 0 < ttl_seconds <= 86_400
                or not callable(clock)):
            raise WorkflowConnectionError("connection requires a finite lifetime of at most one day")
        self._clock = clock
        self._expires_at = self._now() + float(ttl_seconds)
        if not math.isfinite(self._expires_at):
            raise WorkflowConnectionError("connection expiry must be finite")
        self._context = context
        self._store = credential_store
        self._credential = credential
        self._handle_identity = (credential.provider, credential.credential_id, credential._store)
        self._server = server_url.rstrip("/")
        self._resources = tuple(sorted(resources, key=lambda item: item.operation_id))
        self._auth_policy = auth_policy
        self._header_name = header_name
        self._nonce = secrets.token_hex(32)
        self._lock = threading.RLock()
        self._closed = self._revoked = False
        self._account_state = "unknown"
        self._authorization_state = "not_authorized"
        self._failure = None
        self._configuration = self._fixed_configuration()

    def _fixed_configuration(self) -> tuple[Any, ...]:
        # Private equality snapshots avoid publishing low-entropy account/credential hashes.
        # This catches accidental application mutation; Python process owners remain trusted.
        return (self._service_id, self._context, self._expected_account, self._store,
                self._credential, self._handle_identity, self._server,
                canonical_json([item.to_dict() for item in self._resources]),
                self._auth_policy, self._header_name, self._expires_at, self._clock, self._nonce)

    def _now(self) -> float:
        try:
            now = self._clock()
        except Exception:
            raise WorkflowConnectionError("connection clock failed") from None
        if type(now) not in {int, float} or not math.isfinite(now):
            raise WorkflowConnectionError("connection clock must return a finite number")
        return float(now)

    def _assert_local_state(self, now: float | None = None) -> None:
        """Check local invariants without invoking an application or credential-store callback."""
        try:
            unchanged = self._configuration == self._fixed_configuration()
        except Exception:
            unchanged = False
        if not unchanged:
            raise WorkflowConnectionError("connection configuration changed; create a new session")
        if self._closed or self._revoked or now is not None and now >= self._expires_at:
            raise WorkflowConnectionError("connection is closed, revoked, or expired")
        if self._handle_identity != (self._credential.provider, self._credential.credential_id, self._credential._store):
            raise WorkflowConnectionError("connection credential handle identity changed")

    def _lifecycle(self) -> None:
        self._assert_local_state()
        now = self._now()
        self._assert_local_state(now)
        try:
            self._store.metadata(self._credential)
        except Exception:
            raise WorkflowConnectionError("connection credential is unavailable") from None
        self._assert_local_state(now)

    def observe_account(self, account_id: str, *, approve_identity_observation: bool = False) -> None:
        """Record an application-owned identity observation; no network authentication occurs."""
        account_id = _identifier(account_id, "observed account_id")
        if approve_identity_observation is not True:
            raise WorkflowConnectionError("explicit application identity attestation is required")
        with self._lock:
            self._lifecycle()
            if self._failure is not None:
                raise WorkflowConnectionError("connection identity failure is latched; create a new session")
            if account_id != self._expected_account:
                self._account_state = "failed"
                self._failure = "account_mismatch"
                raise WorkflowConnectionError("observed account differs from the expected account")
            self._account_state = "application_attested"

    def authorize(self, *, approve_connection_use: bool = False) -> None:
        if approve_connection_use is not True:
            raise WorkflowConnectionError("explicit connection authorization is required")
        with self._lock:
            self._lifecycle()
            if self._account_state != "application_attested" or self._failure is not None:
                raise WorkflowConnectionError("connection account identity has not been established by the application")
            self._authorization_state = "authorized"

    def revoke(self) -> None:
        """Revoke this connection without revoking another session sharing its credential handle."""
        with self._lock:
            self._revoked = True

    def close(self) -> None:
        with self._lock:
            self._closed = True

    def check_available(self) -> None:
        """Check lifecycle, handle status and fixed authorization without resolving secret bytes."""
        with self._lock:
            self._lifecycle()
            if self._failure is not None or self._account_state != "application_attested" or self._authorization_state != "authorized":
                raise WorkflowConnectionError("connection lacks an authorized application-attested account")

    def binding_metadata(self) -> dict[str, Any]:
        """Stable non-secret authority binding; the nonce is independent of all credentials."""
        self.check_available()
        return {"schema": WORKFLOW_CONNECTION_SCHEMA, "connection_id": self._nonce,
                "service_id": self._service_id, "context": self._context.to_dict(),
                "server_url": self._server, "auth_policy": self._auth_policy,
                "header_name": self._header_name, "expires_at": self._expires_at,
                "resources": [item.to_dict() for item in self._resources],
                "account_identity": "application_attested;not_provider_verified",
                "account_binding": "immutable_expected_account;opaque_session_identity",
                "response_policy": "application_reviewed_public_metadata_only"}

    def to_metadata(self) -> dict[str, Any]:
        """Independent status dimensions; unknown identity never becomes a successful connection."""
        with self._lock:
            try:
                if self._handle_identity != (self._credential.provider, self._credential.credential_id, self._credential._store):
                    raise WorkflowConnectionError("credential handle changed")
                self._store.metadata(self._credential)
                credential_state = "configured"
            except Exception:
                credential_state = "unavailable"
            try:
                expired = self._now() >= self._expires_at
                lifetime = "expired" if expired else "active"
            except WorkflowConnectionError:
                lifetime = "unknown"
            return {"schema": WORKFLOW_CONNECTION_SCHEMA, "connection_id": self._nonce,
                    "service_id": self._service_id, "context": self._context.to_dict(),
                    "credential_state": credential_state, "account_state": self._account_state,
                    "account_observation": "application_attestation;not_provider_verification",
                    "authorization_state": self._authorization_state,
                    "lifecycle_state": "closed" if self._closed else "revoked" if self._revoked else lifetime,
                    "failure": self._failure, "expires_at": self._expires_at,
                    "secret_material": "never_serialized", "live_verification": "unassessed"}

    def validate_import(self, *, service_id: str, context: WorkflowConnectionContext,
                        server_url: str, operation_id: str,
                        path_parameters: Sequence[str], argument_names: Sequence[str]) -> dict[str, Any]:
        """Bind the already-reviewed route to this session before any executor is registered."""
        self.check_available()
        if not isinstance(context, WorkflowConnectionContext) or context != self._context:
            raise WorkflowConnectionError("connection tenant, actor or application session differs")
        if service_id != self._service_id or server_url != self._server:
            raise WorkflowConnectionError("connection service or fixed destination differs")
        resource = next((item for item in self._resources if item.operation_id == operation_id), None)
        if resource is None:
            raise WorkflowConnectionError("operation is outside the fixed connection permission scope")
        constraints = resource.to_dict()["argument_values"]
        if set(path_parameters) - set(constraints) or set(constraints) - set(argument_names):
            raise WorkflowConnectionError("resource scope must constrain every path parameter and only declared arguments")
        return self.binding_metadata()

    def authorize_arguments(self, *, operation_id: str, arguments: Mapping[str, Any],
                            context: WorkflowConnectionContext, server_url: str) -> None:
        with self._lock:
            self.check_available()
            if context != self._context or server_url != self._server:
                raise WorkflowConnectionError("connection subject or fixed destination differs")
            resource = next((item for item in self._resources if item.operation_id == operation_id), None)
            if resource is None:
                raise WorkflowConnectionError("operation is outside the connection permission scope")
            for name, values in resource.to_dict()["argument_values"].items():
                if name not in arguments or canonical_json(arguments[name]) not in {canonical_json(value) for value in values}:
                    raise WorkflowConnectionError("arguments are outside the fixed connection resource scope")

    def resolve_headers(self, *, operation_id: str, arguments: Mapping[str, Any],
                        context: WorkflowConnectionContext, server_url: str) -> dict[str, str]:
        """Resolve a credential only at an approved adapter's final header construction boundary."""
        with self._lock:
            self.authorize_arguments(operation_id=operation_id, arguments=arguments,
                                     context=context, server_url=server_url)
            try:
                value = self._store._resolve(self._credential).expose()
                prefix = "Bearer " if self._auth_policy == "bearer_header" else ""
                if (not value or len(prefix + value) > 8192
                        or any(ord(char) < 32 or ord(char) == 127 for char in value)):
                    raise ValueError()
            except Exception:
                raise WorkflowConnectionError("connection credential resolution failed; secret withheld") from None
            # Foreign clock/store callbacks may revoke, close or change the session reentrantly.
            # The final store availability check also catches revocation during secret resolution.
            # Its callback is followed only by local invariant checks before headers are returned.
            self.check_available()
            return {self._header_name: prefix + value}

    def __repr__(self) -> str:
        return "WorkflowConnectionSession(<opaque application-owned connection>)"


__all__ = ["WORKFLOW_CONNECTION_SCHEMA", "WorkflowConnectionError", "WorkflowConnectionContext",
           "WorkflowConnectionResource", "WorkflowConnectionSession"]
