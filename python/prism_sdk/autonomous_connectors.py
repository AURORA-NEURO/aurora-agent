"""Caller-owned connector registration and evidence dispatch for the autonomous brain.

The Rust gateway already defines provider-connector manifests and handoff evidence.  This module
supplies the missing application runtime around those contracts: a connector registry, exact
domain/capability routing, approval admission, and a transient execution value paired with a
metadata-only receipt.  The executor is always supplied by the embedding application, so this
layer never discovers a provider, accepts a raw key, or performs network I/O by itself.

An application may close over a short-lived ``CredentialHandle``/session in its executor.  The
runtime receives only the typed manifest and transient request, and the journal-compatible receipt
contains digests and identities—not the request, response, headers, or credential material.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .domain_evidence_provider_handoff import DomainEvidenceProviderConnectorManifest
from .domain_tools import (
    AUTONOMOUS_DOMAIN_NAMES,
    _identifier,
    _json_safe,
    _reject_secret_fields,
    _sequence,
)
from .errors import ArgumentError


AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA = "bioprism-python-autonomous-connector-registry/0.1"
AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA = "bioprism-python-autonomous-connector-dispatch/0.1"
AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA = "bioprism-python-autonomous-connector-receipt/0.1"
AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES = ("observed", "partial", "refused", "error", "unknown")
MAX_AUTONOMOUS_CONNECTORS = 256
MAX_AUTONOMOUS_CONNECTOR_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES = 2_000_000
MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES = 2_000_000
MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS = 128


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _manifest_digest(manifest: DomainEvidenceProviderConnectorManifest) -> str:
    return content_digest(manifest.to_dict())


@dataclass(frozen=True, slots=True)
class AutonomousConnectorRegistration:
    """Redacted registration metadata plus a caller-owned transient executor."""

    manifest: DomainEvidenceProviderConnectorManifest
    executor: Callable[[DomainEvidenceProviderConnectorManifest, Mapping[str, Any]], Any]
    approval_required: bool = True

    def __post_init__(self) -> None:
        if not isinstance(self.manifest, DomainEvidenceProviderConnectorManifest):
            raise ArgumentError("autonomous connector registration requires a typed manifest")
        if not callable(self.executor):
            raise ArgumentError("autonomous connector registration executor must be callable")
        if not isinstance(self.approval_required, bool):
            raise ArgumentError("autonomous connector approval_required must be a boolean")
        domains = tuple(self.manifest.domains)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector manifest contains an unsupported domain")

    @property
    def connector_id(self) -> str:
        return self.manifest.connector_id

    @property
    def manifest_digest(self) -> str:
        return _manifest_digest(self.manifest)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
            "manifest": self.manifest.to_dict(),
            "manifest_digest": self.manifest_digest,
            "approval_required": self.approval_required,
            "execution": "caller_owned_executor;metadata_only_registration",
            "secret_material": "never_returned",
        }


class AutonomousConnectorRegistry:
    """Exact connector catalogue; registration never authorizes or dispatches a connector."""

    def __init__(self, registrations: Sequence[AutonomousConnectorRegistration] = ()) -> None:
        if not isinstance(registrations, Sequence) or isinstance(registrations, (str, bytes)):
            raise ArgumentError("autonomous connector registrations must be a sequence")
        self._connectors: dict[str, AutonomousConnectorRegistration] = {}
        for registration in registrations:
            self.register(registration, replace=False)

    def register(
        self,
        registration: AutonomousConnectorRegistration,
        *,
        replace: bool = False,
    ) -> AutonomousConnectorRegistration:
        if not isinstance(registration, AutonomousConnectorRegistration):
            raise ArgumentError("autonomous connector registration is invalid")
        if not isinstance(replace, bool):
            raise ArgumentError("autonomous connector replace must be a boolean")
        connector_id = _identifier("autonomous connector id", registration.connector_id)
        if connector_id in self._connectors and not replace:
            raise ArgumentError("autonomous connector is already registered")
        if connector_id not in self._connectors and len(self._connectors) >= MAX_AUTONOMOUS_CONNECTORS:
            raise ArgumentError("autonomous connector registry capacity is exhausted")
        self._connectors[connector_id] = registration
        return registration

    def resolve(self, connector_id: str) -> AutonomousConnectorRegistration:
        connector_id = _identifier("autonomous connector id", connector_id)
        registration = self._connectors.get(connector_id)
        if registration is None:
            raise ArgumentError("autonomous connector is not registered")
        return registration

    def registrations(self) -> tuple[AutonomousConnectorRegistration, ...]:
        return tuple(self._connectors[name] for name in sorted(self._connectors))

    def plan_for_domains(
        self,
        domains: Sequence[str],
        *,
        capability: str | None = None,
    ) -> dict[str, Any]:
        requested = _sequence("autonomous connector plan domains", domains, maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in requested):
            raise ArgumentError("autonomous connector plan contains an unsupported domain")
        if capability is not None:
            capability = _identifier("autonomous connector plan capability", capability)
        coverage: dict[str, dict[str, Any]] = {}
        for domain in requested:
            candidates = [
                registration
                for registration in self.registrations()
                if domain in registration.manifest.domains
                and (capability is None or capability in registration.manifest.capabilities)
            ]
            coverage[domain] = {
                "status": "selected" if candidates else "missing",
                "connector_ids": [item.connector_id for item in candidates],
                "manifest_digests": [item.manifest_digest for item in candidates],
                "capability": capability,
            }
        payload = {
            "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
            "domains": list(requested),
            "capability": capability,
            "coverage": coverage,
            "registry_digest": self.digest,
            "execution": "planning_only;no_dispatch;no_authorization",
            "secret_material": "never_returned",
        }
        payload["plan_digest"] = content_digest(payload)
        return payload

    @property
    def digest(self) -> str:
        return content_digest([registration.to_dict() for registration in self.registrations()])

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
            "digest": self.digest,
            "connectors": [registration.to_dict() for registration in self.registrations()],
            "connector_count": len(self._connectors),
            "execution": "metadata_only;registration_is_not_authorization",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorDispatchRequest:
    """Transient connector input with a digest-only public projection."""

    dispatch_id: str
    execution_id: str
    call_id: str
    connector_id: str
    domains: tuple[str, ...]
    capability: str
    request: Mapping[str, Any]
    parent_digests: tuple[str, ...] = ()
    attempt_id: str | None = None
    approved: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("dispatch_id", self.dispatch_id),
            ("execution_id", self.execution_id),
            ("call_id", self.call_id),
            ("connector_id", self.connector_id),
            ("capability", self.capability),
        ):
            _identifier(f"autonomous connector dispatch {name}", value)
        domains = _sequence(
            "autonomous connector dispatch domains",
            self.domains,
            maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS,
        )
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector dispatch contains an unsupported domain")
        if not isinstance(self.request, Mapping):
            raise ArgumentError("autonomous connector dispatch request must be an object")
        safe_request = _json_safe(
            "autonomous connector dispatch request",
            dict(self.request),
            maximum=MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES,
        )
        _reject_secret_fields(safe_request)
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "request", safe_request)
        if len(self.parent_digests) > MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS:
            raise ArgumentError("autonomous connector dispatch parent_digests exceeds its bound")
        for digest in self.parent_digests:
            _digest("autonomous connector dispatch parent digest", digest)
        if self.attempt_id is not None:
            _identifier("autonomous connector dispatch attempt_id", self.attempt_id)
        if not isinstance(self.approved, bool):
            raise ArgumentError("autonomous connector dispatch approved must be a boolean")

    @property
    def request_digest(self) -> str:
        return content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
                "dispatch_id": self.dispatch_id,
                "execution_id": self.execution_id,
                "call_id": self.call_id,
                "connector_id": self.connector_id,
                "domains": list(self.domains),
                "capability": self.capability,
                "request": dict(self.request),
                "parent_digests": list(self.parent_digests),
                "attempt_id": self.attempt_id,
            }
        )

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
            "dispatch_id": self.dispatch_id,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "connector_id": self.connector_id,
            "domains": list(self.domains),
            "capability": self.capability,
            "request_digest": self.request_digest,
            "parent_digests": list(self.parent_digests),
            "attempt_id": self.attempt_id,
            "approved": self.approved,
            "retention": "metadata_only_request_not_returned",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorObservation:
    """Caller-owned transient result classification; ``value`` is never retained in receipts."""

    value: Any = None
    status: str = "observed"
    failure_class: str | None = None

    def __post_init__(self) -> None:
        if self.status not in AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES:
            raise ArgumentError("autonomous connector observation status is invalid")
        if self.failure_class is not None:
            _identifier("autonomous connector observation failure_class", self.failure_class)
        safe_value = _json_safe(
            "autonomous connector observation value",
            self.value,
            maximum=MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES,
        )
        _reject_secret_fields(safe_value)
        object.__setattr__(self, "value", safe_value)


@dataclass(frozen=True, slots=True)
class AutonomousConnectorDispatchReceipt:
    """Metadata-only outcome for one connector attempt."""

    dispatch_id: str
    execution_id: str
    call_id: str
    connector_id: str
    connector_version: str
    provider: str
    connector_kind: str
    manifest_digest: str
    domains: tuple[str, ...]
    capability: str
    status: str
    request_digest: str
    payload_digest: str | None = None
    parent_digests: tuple[str, ...] = ()
    attempt_id: str | None = None
    failure_class: str | None = None

    def __post_init__(self) -> None:
        for name, value in (
            ("dispatch_id", self.dispatch_id),
            ("execution_id", self.execution_id),
            ("call_id", self.call_id),
            ("connector_id", self.connector_id),
            ("connector_version", self.connector_version),
            ("provider", self.provider),
            ("connector_kind", self.connector_kind),
            ("capability", self.capability),
        ):
            _identifier(f"autonomous connector receipt {name}", value)
        _digest("autonomous connector receipt manifest_digest", self.manifest_digest)
        _digest("autonomous connector receipt request_digest", self.request_digest)
        _digest("autonomous connector receipt payload_digest", self.payload_digest, allow_none=True)
        if self.status not in AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES:
            raise ArgumentError("autonomous connector receipt status is invalid")
        domains = _sequence("autonomous connector receipt domains", self.domains, maximum=MAX_AUTONOMOUS_CONNECTOR_DOMAINS)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains):
            raise ArgumentError("autonomous connector receipt contains an unsupported domain")
        object.__setattr__(self, "domains", domains)
        for digest in self.parent_digests:
            _digest("autonomous connector receipt parent digest", digest)
        if len(self.parent_digests) > MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS:
            raise ArgumentError("autonomous connector receipt parent_digests exceeds its bound")
        if self.attempt_id is not None:
            _identifier("autonomous connector receipt attempt_id", self.attempt_id)
        if self.failure_class is not None:
            _identifier("autonomous connector receipt failure_class", self.failure_class)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA,
            "dispatch_id": self.dispatch_id,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "connector_id": self.connector_id,
            "connector_version": self.connector_version,
            "provider": self.provider,
            "connector_kind": self.connector_kind,
            "manifest_digest": self.manifest_digest,
            "domains": list(self.domains),
            "capability": self.capability,
            "status": self.status,
            "request_digest": self.request_digest,
            "payload_digest": self.payload_digest,
            "parent_digests": list(self.parent_digests),
            "attempt_id": self.attempt_id,
            "failure_class": self.failure_class,
            "retention": "metadata_only_no_request_or_payload",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorDispatchResult:
    """Transient caller value paired with a durable-safe receipt."""

    receipt: AutonomousConnectorDispatchReceipt
    value: Any = None

    def __post_init__(self) -> None:
        if not isinstance(self.receipt, AutonomousConnectorDispatchReceipt):
            raise ArgumentError("autonomous connector dispatch result receipt is invalid")
        safe_value = _json_safe(
            "autonomous connector dispatch result value",
            self.value,
            maximum=MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES,
        )
        _reject_secret_fields(safe_value)
        object.__setattr__(self, "value", safe_value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
            "receipt": self.receipt.to_dict(),
            "value_present": self.value is not None,
            "retention": "receipt_metadata_only;value_transient",
            "secret_material": "never_returned",
        }


class AutonomousConnectorRuntime:
    """Approval-aware dispatcher for caller-owned external evidence connectors."""

    def __init__(
        self,
        registry: AutonomousConnectorRegistry,
        *,
        receipt_sink: Callable[[AutonomousConnectorDispatchReceipt], Any] | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousConnectorRegistry):
            raise ArgumentError("autonomous connector runtime requires an AutonomousConnectorRegistry")
        if receipt_sink is not None and not callable(receipt_sink):
            raise ArgumentError("autonomous connector runtime receipt sink must be callable")
        self.registry = registry
        self.receipt_sink = receipt_sink

    def dispatch(self, request: AutonomousConnectorDispatchRequest) -> AutonomousConnectorDispatchResult:
        if not isinstance(request, AutonomousConnectorDispatchRequest):
            raise ArgumentError("autonomous connector dispatch requires a typed request")
        registration = self.registry.resolve(request.connector_id)
        manifest = registration.manifest
        request_digest = request.request_digest
        missing_domains = sorted(set(request.domains) - set(manifest.domains))
        if missing_domains:
            return self._finish(
                request,
                registration,
                status="refused",
                failure_class="domain_scope",
                request_digest=request_digest,
            )
        if request.capability not in manifest.capabilities:
            return self._finish(
                request,
                registration,
                status="refused",
                failure_class="capability_scope",
                request_digest=request_digest,
            )
        if registration.approval_required and not request.approved:
            return self._finish(
                request,
                registration,
                status="refused",
                failure_class="approval_required",
                request_digest=request_digest,
            )
        try:
            raw = registration.executor(manifest, request.request)
            observation = raw if isinstance(raw, AutonomousConnectorObservation) else AutonomousConnectorObservation(raw)
        except Exception:
            return self._finish(
                request,
                registration,
                status="error",
                failure_class="executor_error",
                request_digest=request_digest,
            )
        payload_digest = None if observation.value is None else content_digest(observation.value)
        return self._finish(
            request,
            registration,
            status=observation.status,
            failure_class=observation.failure_class,
            request_digest=request_digest,
            payload_digest=payload_digest,
            value=observation.value,
        )

    def _finish(
        self,
        request: AutonomousConnectorDispatchRequest,
        registration: AutonomousConnectorRegistration,
        *,
        status: str,
        failure_class: str | None,
        request_digest: str,
        payload_digest: str | None = None,
        value: Any = None,
    ) -> AutonomousConnectorDispatchResult:
        manifest = registration.manifest
        receipt = AutonomousConnectorDispatchReceipt(
            dispatch_id=request.dispatch_id,
            execution_id=request.execution_id,
            call_id=request.call_id,
            connector_id=manifest.connector_id,
            connector_version=manifest.version,
            provider=manifest.provider,
            connector_kind=manifest.connector_kind,
            manifest_digest=registration.manifest_digest,
            domains=request.domains,
            capability=request.capability,
            status=status,
            request_digest=request_digest,
            payload_digest=payload_digest,
            parent_digests=request.parent_digests,
            attempt_id=request.attempt_id,
            failure_class=failure_class,
        )
        if self.receipt_sink is not None:
            try:
                self.receipt_sink(receipt)
            except Exception as error:
                raise ArgumentError("autonomous connector receipt sink failed") from error
        return AutonomousConnectorDispatchResult(receipt, value)


__all__ = [
    "AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA",
    "AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA",
    "AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA",
    "AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES",
    "MAX_AUTONOMOUS_CONNECTORS",
    "MAX_AUTONOMOUS_CONNECTOR_DOMAINS",
    "MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS",
    "AutonomousConnectorRegistration",
    "AutonomousConnectorRegistry",
    "AutonomousConnectorDispatchRequest",
    "AutonomousConnectorObservation",
    "AutonomousConnectorDispatchReceipt",
    "AutonomousConnectorDispatchResult",
    "AutonomousConnectorRuntime",
]
