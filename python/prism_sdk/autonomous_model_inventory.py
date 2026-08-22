"""Bounded provider-model inventory synchronization for the autonomous brain.

Provider model discovery is intentionally separate from model selection.  A provider can report
that a model exists and supports a transport feature, but it cannot author trustworthy quality,
cost, latency, or domain-suitability priors.  This module closes the application integration gap
without collapsing those facts together:

* discovery projects provider responses into :class:`ProviderModelDescriptor` values immediately;
* every discovered model still requires an explicit caller-owned routing prior;
* each provider is reconciled independently, so a failed provider cannot retire a healthy
  provider's catalogue;
* stale arms are retired only after a successful authoritative inventory response;
* domain coverage is calculated from the reviewed domain requirements, but never treated as
  runtime readiness or semantic quality; and
* optional persistence stores only bounded metadata and a digest-bound snapshot.

Credentials, raw inventory rows, authorization headers, prompts, and provider response bodies
never enter the snapshot.  The coordinator is useful with remote providers and with the explicit
credentialless in-memory transport used by offline tests and local model bridges.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import tempfile
import threading
import time
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_bytes, content_digest
from .llm_runtime import (
    CredentialError,
    CredentialHandle,
    CredentialSession,
    LLMRuntime,
    ModelCatalogue,
    ProviderError,
    ProviderModelDescriptor,
)


AUTONOMOUS_MODEL_INVENTORY_SCHEMA = "bioprism-python-autonomous-model-inventory/0.1"
AUTONOMOUS_MODEL_INVENTORY_PROVIDER_SCHEMA = "bioprism-python-autonomous-model-inventory-provider/0.1"
AUTONOMOUS_MODEL_INVENTORY_COVERAGE_SCHEMA = "bioprism-python-autonomous-model-inventory-coverage/0.1"
AUTONOMOUS_MODEL_INVENTORY_STORE_SCHEMA = "bioprism-python-autonomous-model-inventory-store/0.1"
AUTONOMOUS_MODEL_INVENTORY_STATUSES = ("completed", "partial", "failed")
AUTONOMOUS_MODEL_INVENTORY_PROVIDER_STATUSES = (
    "refreshed",
    "credential_required",
    "provider_failed",
    "not_configured",
)
MAX_AUTONOMOUS_MODEL_INVENTORY_PROVIDERS = 128
MAX_AUTONOMOUS_MODEL_INVENTORY_MODELS_PER_PROVIDER = 512
MAX_AUTONOMOUS_MODEL_INVENTORY_DOMAINS = 64
MAX_AUTONOMOUS_MODEL_INVENTORY_CAPABILITIES = 64
MAX_AUTONOMOUS_MODEL_INVENTORY_IDS = 512
MAX_AUTONOMOUS_MODEL_INVENTORY_SNAPSHOT_BYTES = 8_000_000
MAX_AUTONOMOUS_MODEL_INVENTORY_ERROR_BYTES = 256
MAX_AUTONOMOUS_MODEL_INVENTORY_IDENTIFIER_BYTES = 512


class AutonomousModelInventoryError(RuntimeError):
    """Raised when an inventory request or metadata snapshot violates its contract."""


def _text(name: str, value: Any, *, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomousModelInventoryError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise AutonomousModelInventoryError(f"{name} exceeds its bounded size")
    return value


def _identifier(name: str, value: Any) -> str:
    resolved = _text(name, value, maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_IDENTIFIER_BYTES)
    if "/" in resolved or "\\" in resolved or any(ord(char) < 32 for char in resolved):
        raise AutonomousModelInventoryError(f"{name} is not a safe identifier")
    return resolved


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise AutonomousModelInventoryError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _finite_time(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise AutonomousModelInventoryError(f"{name} must be finite")
    return float(value)


def _bounded_count(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise AutonomousModelInventoryError(f"{name} is outside its bound")
    return value


def _string_tuple(name: str, value: Any, *, maximum: int, identifiers: bool = False) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise AutonomousModelInventoryError(f"{name} must be a sequence")
    if len(value) > maximum:
        raise AutonomousModelInventoryError(f"{name} exceeds its bound")
    result: list[str] = []
    for item in value:
        normalized = (
            _identifier(f"{name} item", item)
            if identifiers
            else _text(f"{name} item", item, maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_IDENTIFIER_BYTES)
        )
        if normalized in result:
            raise AutonomousModelInventoryError(f"{name} contains duplicate values")
        result.append(normalized)
    return tuple(result)


def _safe_json(value: Any, *, depth: int = 0) -> None:
    if depth > 16:
        raise AutonomousModelInventoryError("inventory metadata nesting exceeds its bound")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise AutonomousModelInventoryError("inventory metadata contains a non-finite number")
        return
    if isinstance(value, Mapping):
        if len(value) > 256:
            raise AutonomousModelInventoryError("inventory metadata object exceeds its bound")
        for key, item in value.items():
            if not isinstance(key, str) or not key.strip():
                raise AutonomousModelInventoryError("inventory metadata keys must be non-empty strings")
            normalized = key.lower().replace("-", "_")
            if any(token in normalized for token in ("secret", "credential", "authorization", "api_key", "access_token", "refresh_token", "bearer")):
                raise AutonomousModelInventoryError("inventory metadata contains secret-shaped fields")
            _safe_json(item, depth=depth + 1)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        if len(value) > 512:
            raise AutonomousModelInventoryError("inventory metadata array exceeds its bound")
        for item in value:
            _safe_json(item, depth=depth + 1)
        return
    raise AutonomousModelInventoryError("inventory metadata must be JSON-safe")


def _safe_error_code(error: BaseException) -> int | None:
    status_code = getattr(error, "status_code", None)
    if isinstance(status_code, int) and not isinstance(status_code, bool) and 100 <= status_code <= 999:
        return status_code
    return None


def _failure_class(error: BaseException) -> str:
    if isinstance(error, CredentialError):
        return "credential"
    if isinstance(error, ProviderError):
        return "provider"
    return "inventory"


@dataclass(frozen=True, slots=True)
class AutonomousModelInventoryProviderResult:
    """Redacted outcome for one provider refresh."""

    provider: str
    status: str
    model_count: int
    model_ids: tuple[str, ...]
    descriptor_digest: str | None
    registered_model_ids: tuple[str, ...] = ()
    replaced_model_ids: tuple[str, ...] = ()
    removed_model_ids: tuple[str, ...] = ()
    requires_credential: bool = False
    runtime_status: Mapping[str, Any] | None = None
    error_class: str | None = None
    error_code: int | None = None

    def __post_init__(self) -> None:
        provider = _identifier("inventory provider", self.provider)
        if self.status not in AUTONOMOUS_MODEL_INVENTORY_PROVIDER_STATUSES:
            raise AutonomousModelInventoryError("inventory provider status is unsupported")
        count = _bounded_count("inventory provider model_count", self.model_count, MAX_AUTONOMOUS_MODEL_INVENTORY_MODELS_PER_PROVIDER)
        model_ids = _string_tuple("inventory provider model_ids", self.model_ids, maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_MODELS_PER_PROVIDER)
        if count != len(model_ids):
            raise AutonomousModelInventoryError("inventory provider model_count does not match model_ids")
        for name, value in (
            ("descriptor_digest", self.descriptor_digest),
        ):
            if value is not None:
                _digest(f"inventory provider {name}", value)
        for name, value in (
            ("registered_model_ids", self.registered_model_ids),
            ("replaced_model_ids", self.replaced_model_ids),
            ("removed_model_ids", self.removed_model_ids),
        ):
            _string_tuple(f"inventory provider {name}", value, maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_IDS, identifiers=False)
        if not isinstance(self.requires_credential, bool):
            raise AutonomousModelInventoryError("inventory provider requires_credential must be boolean")
        if self.runtime_status is not None:
            if not isinstance(self.runtime_status, Mapping):
                raise AutonomousModelInventoryError("inventory provider runtime_status must be an object")
            _safe_json(self.runtime_status)
        if self.error_class is not None:
            _identifier("inventory provider error_class", self.error_class)
        if self.error_code is not None and (
            isinstance(self.error_code, bool) or not isinstance(self.error_code, int) or not 100 <= self.error_code <= 999
        ):
            raise AutonomousModelInventoryError("inventory provider error_code is invalid")
        if self.status == "refreshed" and self.descriptor_digest is None:
            raise AutonomousModelInventoryError("refreshed provider must have a descriptor digest")
        if self.status == "refreshed" and self.error_class is not None:
            raise AutonomousModelInventoryError("refreshed provider cannot have an error")
        if self.status != "refreshed" and self.model_count != 0:
            raise AutonomousModelInventoryError("failed provider cannot report models")
        object.__setattr__(self, "provider", provider)
        object.__setattr__(self, "model_ids", model_ids)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MODEL_INVENTORY_PROVIDER_SCHEMA,
            "provider": self.provider,
            "status": self.status,
            "model_count": self.model_count,
            "model_ids": list(self.model_ids),
            "descriptor_digest": self.descriptor_digest,
            "registered_model_ids": list(self.registered_model_ids),
            "replaced_model_ids": list(self.replaced_model_ids),
            "removed_model_ids": list(self.removed_model_ids),
            "requires_credential": self.requires_credential,
            "runtime_status": None if self.runtime_status is None else dict(self.runtime_status),
            "error_class": self.error_class,
            "error_code": self.error_code,
            "retention": "metadata_only_no_credentials_or_provider_payloads",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousModelInventoryProviderResult":
        if not isinstance(value, Mapping):
            raise AutonomousModelInventoryError("inventory provider result must be an object")
        allowed = {
            "schema", "provider", "status", "model_count", "model_ids", "descriptor_digest",
            "registered_model_ids", "replaced_model_ids", "removed_model_ids", "requires_credential",
            "runtime_status", "error_class", "error_code", "retention", "secret_material",
        }
        if set(value).difference(allowed):
            raise AutonomousModelInventoryError("inventory provider result contains unsupported fields")
        if value.get("schema") != AUTONOMOUS_MODEL_INVENTORY_PROVIDER_SCHEMA:
            raise AutonomousModelInventoryError("inventory provider result schema is invalid")
        if value.get("retention") != "metadata_only_no_credentials_or_provider_payloads" or value.get("secret_material") != "never_returned":
            raise AutonomousModelInventoryError("inventory provider result markers are invalid")
        return cls(
            provider=value.get("provider"),
            status=value.get("status"),
            model_count=value.get("model_count"),
            model_ids=tuple(value.get("model_ids", ())),
            descriptor_digest=value.get("descriptor_digest"),
            registered_model_ids=tuple(value.get("registered_model_ids", ())),
            replaced_model_ids=tuple(value.get("replaced_model_ids", ())),
            removed_model_ids=tuple(value.get("removed_model_ids", ())),
            requires_credential=value.get("requires_credential", False),
            runtime_status=value.get("runtime_status"),
            error_class=value.get("error_class"),
            error_code=value.get("error_code"),
        )


@dataclass(frozen=True, slots=True)
class AutonomousModelInventoryCoverage:
    """Static catalogue coverage for one reviewed autonomous domain."""

    domain: str
    required_capabilities: tuple[str, ...]
    compatible_arm_ids: tuple[str, ...]
    candidate_count: int
    compatible_count: int

    def __post_init__(self) -> None:
        domain = _identifier("inventory coverage domain", self.domain)
        required = _string_tuple(
            "inventory coverage required_capabilities",
            self.required_capabilities,
            maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_CAPABILITIES,
        )
        arms = _string_tuple(
            "inventory coverage compatible_arm_ids",
            self.compatible_arm_ids,
            maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_IDS,
        )
        candidate_count = _bounded_count("inventory coverage candidate_count", self.candidate_count, MAX_AUTONOMOUS_MODEL_INVENTORY_IDS)
        compatible_count = _bounded_count("inventory coverage compatible_count", self.compatible_count, candidate_count)
        if compatible_count != len(arms):
            raise AutonomousModelInventoryError("inventory coverage compatible_count does not match compatible_arm_ids")
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "required_capabilities", required)
        object.__setattr__(self, "compatible_arm_ids", arms)

    @property
    def coverage(self) -> float:
        return self.compatible_count / self.candidate_count if self.candidate_count else 0.0

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MODEL_INVENTORY_COVERAGE_SCHEMA,
            "domain": self.domain,
            "required_capabilities": list(self.required_capabilities),
            "compatible_arm_ids": list(self.compatible_arm_ids),
            "candidate_count": self.candidate_count,
            "compatible_count": self.compatible_count,
            "coverage": self.coverage,
            "evidence_posture": "static_caller_declared_capabilities_only",
            "runtime_gates": "not_projected; credentials_health_cost_latency_and_circuit_are_live_gates",
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousModelInventoryCoverage":
        if not isinstance(value, Mapping):
            raise AutonomousModelInventoryError("inventory coverage must be an object")
        allowed = {
            "schema", "domain", "required_capabilities", "compatible_arm_ids", "candidate_count",
            "compatible_count", "coverage", "evidence_posture", "runtime_gates",
        }
        if set(value).difference(allowed):
            raise AutonomousModelInventoryError("inventory coverage contains unsupported fields")
        if value.get("schema") != AUTONOMOUS_MODEL_INVENTORY_COVERAGE_SCHEMA:
            raise AutonomousModelInventoryError("inventory coverage schema is invalid")
        result = cls(
            domain=value.get("domain"),
            required_capabilities=tuple(value.get("required_capabilities", ())),
            compatible_arm_ids=tuple(value.get("compatible_arm_ids", ())),
            candidate_count=value.get("candidate_count"),
            compatible_count=value.get("compatible_count"),
        )
        supplied_coverage = value.get("coverage")
        if isinstance(supplied_coverage, bool) or not isinstance(supplied_coverage, (int, float)) or not math.isclose(float(supplied_coverage), result.coverage, rel_tol=0.0, abs_tol=1e-12):
            raise AutonomousModelInventoryError("inventory coverage value does not match its counts")
        if value.get("evidence_posture") != "static_caller_declared_capabilities_only" or value.get("runtime_gates") != "not_projected; credentials_health_cost_latency_and_circuit_are_live_gates":
            raise AutonomousModelInventoryError("inventory coverage markers are invalid")
        return result


@dataclass(frozen=True, slots=True)
class AutonomousModelInventorySnapshot:
    """Digest-bound, restart-safe metadata snapshot of one inventory refresh."""

    refresh_id: str
    started_at: float
    completed_at: float
    status: str
    providers: tuple[AutonomousModelInventoryProviderResult, ...]
    coverage: tuple[AutonomousModelInventoryCoverage, ...]
    catalogue_digest: str
    snapshot_digest: str | None = None

    def __post_init__(self) -> None:
        refresh_id = _identifier("inventory refresh_id", self.refresh_id)
        started_at = _finite_time("inventory started_at", self.started_at)
        completed_at = _finite_time("inventory completed_at", self.completed_at)
        if completed_at < started_at:
            raise AutonomousModelInventoryError("inventory completed_at cannot precede started_at")
        if self.status not in AUTONOMOUS_MODEL_INVENTORY_STATUSES:
            raise AutonomousModelInventoryError("inventory status is unsupported")
        if not isinstance(self.providers, Sequence) or isinstance(self.providers, (str, bytes)) or not self.providers:
            raise AutonomousModelInventoryError("inventory snapshot must contain providers")
        if len(self.providers) > MAX_AUTONOMOUS_MODEL_INVENTORY_PROVIDERS or any(not isinstance(item, AutonomousModelInventoryProviderResult) for item in self.providers):
            raise AutonomousModelInventoryError("inventory provider rows are invalid")
        if len({item.provider for item in self.providers}) != len(self.providers):
            raise AutonomousModelInventoryError("inventory providers must be unique")
        if not isinstance(self.coverage, Sequence) or isinstance(self.coverage, (str, bytes)) or len(self.coverage) > MAX_AUTONOMOUS_MODEL_INVENTORY_DOMAINS or any(not isinstance(item, AutonomousModelInventoryCoverage) for item in self.coverage):
            raise AutonomousModelInventoryError("inventory coverage rows are invalid")
        if len({item.domain for item in self.coverage}) != len(self.coverage):
            raise AutonomousModelInventoryError("inventory coverage domains must be unique")
        catalogue_digest = _digest("inventory catalogue_digest", self.catalogue_digest)
        if self.snapshot_digest is not None:
            _digest("inventory snapshot_digest", self.snapshot_digest)
        object.__setattr__(self, "refresh_id", refresh_id)
        object.__setattr__(self, "started_at", started_at)
        object.__setattr__(self, "completed_at", completed_at)
        object.__setattr__(self, "catalogue_digest", catalogue_digest)
        if self.snapshot_digest is not None and self.snapshot_digest != content_digest(self._payload()):
            raise AutonomousModelInventoryError("inventory snapshot_digest does not match its contents")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MODEL_INVENTORY_SCHEMA,
            "refresh_id": self.refresh_id,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "status": self.status,
            "providers": [item.to_dict() for item in self.providers],
            "coverage": [item.to_dict() for item in self.coverage],
            "catalogue_digest": self.catalogue_digest,
        }

    @property
    def digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        result = self._payload()
        result.update(
            {
                "snapshot_digest": self.digest,
                "retention": "metadata_only_no_credentials_or_provider_payloads",
                "secret_material": "never_returned",
            }
        )
        return result

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousModelInventorySnapshot":
        if not isinstance(value, Mapping):
            raise AutonomousModelInventoryError("inventory snapshot must be an object")
        allowed = {
            "schema", "refresh_id", "started_at", "completed_at", "status", "providers", "coverage",
            "catalogue_digest", "snapshot_digest", "retention", "secret_material",
        }
        if set(value).difference(allowed):
            raise AutonomousModelInventoryError("inventory snapshot contains unsupported fields")
        if value.get("schema") != AUTONOMOUS_MODEL_INVENTORY_SCHEMA:
            raise AutonomousModelInventoryError("inventory snapshot schema is invalid")
        if value.get("retention") != "metadata_only_no_credentials_or_provider_payloads" or value.get("secret_material") != "never_returned":
            raise AutonomousModelInventoryError("inventory snapshot markers are invalid")
        providers = value.get("providers")
        coverage = value.get("coverage")
        if not isinstance(providers, Sequence) or isinstance(providers, (str, bytes)) or not isinstance(coverage, Sequence) or isinstance(coverage, (str, bytes)):
            raise AutonomousModelInventoryError("inventory snapshot provider/coverage rows are malformed")
        result = cls(
            refresh_id=value.get("refresh_id"),
            started_at=value.get("started_at"),
            completed_at=value.get("completed_at"),
            status=value.get("status"),
            providers=tuple(AutonomousModelInventoryProviderResult.from_mapping(item) for item in providers),
            coverage=tuple(AutonomousModelInventoryCoverage.from_mapping(item) for item in coverage),
            catalogue_digest=value.get("catalogue_digest"),
            snapshot_digest=value.get("snapshot_digest"),
        )
        if result.snapshot_digest != result.digest:
            raise AutonomousModelInventoryError("inventory snapshot digest is invalid")
        return result


class AutonomousModelInventoryStore:
    """Atomic JSON persistence for redacted inventory snapshots."""

    def __init__(self, path: str | os.PathLike[str], *, max_bytes: int = MAX_AUTONOMOUS_MODEL_INVENTORY_SNAPSHOT_BYTES) -> None:
        if not isinstance(path, (str, os.PathLike)) or not str(path):
            raise AutonomousModelInventoryError("inventory store path must be non-empty")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 0 < max_bytes <= MAX_AUTONOMOUS_MODEL_INVENTORY_SNAPSHOT_BYTES:
            raise AutonomousModelInventoryError("inventory store max_bytes is outside its bound")
        self.path = Path(path)
        self.max_bytes = max_bytes
        self._lock = threading.RLock()

    def save(self, snapshot: AutonomousModelInventorySnapshot) -> dict[str, Any]:
        if not isinstance(snapshot, AutonomousModelInventorySnapshot):
            raise AutonomousModelInventoryError("inventory store accepts only snapshots")
        envelope = {
            "schema": AUTONOMOUS_MODEL_INVENTORY_STORE_SCHEMA,
            "snapshot": snapshot.to_dict(),
            "snapshot_digest": snapshot.digest,
            "retention": "metadata_only_no_credentials_or_provider_payloads",
            "secret_material": "never_returned",
        }
        encoded = canonical_bytes(envelope)
        if len(encoded) > self.max_bytes:
            raise AutonomousModelInventoryError("inventory snapshot exceeds its storage bound")
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
        return {"schema": AUTONOMOUS_MODEL_INVENTORY_STORE_SCHEMA, "snapshot_digest": snapshot.digest, "bytes": len(encoded)}

    def load(self) -> AutonomousModelInventorySnapshot | None:
        with self._lock:
            if not self.path.exists():
                return None
            if self.path.stat().st_size > self.max_bytes:
                raise AutonomousModelInventoryError("inventory store exceeds its bound")
            try:
                envelope = json.loads(self.path.read_text(encoding="utf-8"))
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise AutonomousModelInventoryError("inventory store contains invalid JSON") from error
        if not isinstance(envelope, Mapping) or envelope.get("schema") != AUTONOMOUS_MODEL_INVENTORY_STORE_SCHEMA:
            raise AutonomousModelInventoryError("inventory store schema is invalid")
        if envelope.get("retention") != "metadata_only_no_credentials_or_provider_payloads" or envelope.get("secret_material") != "never_returned":
            raise AutonomousModelInventoryError("inventory store markers are invalid")
        snapshot = AutonomousModelInventorySnapshot.from_mapping(envelope.get("snapshot"))
        if envelope.get("snapshot_digest") != snapshot.digest:
            raise AutonomousModelInventoryError("inventory store snapshot digest is invalid")
        return snapshot


class AutonomousModelInventoryCoordinator:
    """Refresh live provider inventory and produce domain coverage without selecting a model.

    Callers can provide a static ``priors`` mapping when the model ids are already known, or a
    ``prior_factory`` that turns each typed descriptor into explicit caller-owned routing priors
    after one live discovery pass. The factory receives no raw provider response or credential.
    """

    def __init__(self, runtime: LLMRuntime, catalogue: ModelCatalogue, *, clock: Callable[[], float] = time.time) -> None:
        if not isinstance(runtime, LLMRuntime):
            raise AutonomousModelInventoryError("inventory runtime must be an LLMRuntime")
        if not isinstance(catalogue, ModelCatalogue):
            raise AutonomousModelInventoryError("inventory catalogue must be a ModelCatalogue")
        if not callable(clock):
            raise AutonomousModelInventoryError("inventory clock must be callable")
        self.runtime = runtime
        self.catalogue = catalogue
        self._clock = clock

    def refresh(
        self,
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession | None = None,
        providers: Sequence[str] | None = None,
        priors: Mapping[str, Mapping[str, Any]] | None = None,
        prior_factory: Callable[[ProviderModelDescriptor], Mapping[str, Any]] | None = None,
        domain_requirements: Mapping[str, Sequence[str]] = {},
        limit: int = MAX_AUTONOMOUS_MODEL_INVENTORY_MODELS_PER_PROVIDER,
        snapshot_store: AutonomousModelInventoryStore | None = None,
        refresh_id: str | None = None,
        raise_on_error: bool = False,
    ) -> AutonomousModelInventorySnapshot:
        if providers is None:
            names = tuple(item["provider"] for item in self.runtime.provider_metadata() if isinstance(item.get("provider"), str))
        else:
            names = _string_tuple("inventory providers", providers, maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_PROVIDERS, identifiers=True)
        if not names:
            raise AutonomousModelInventoryError("inventory refresh requires at least one provider")
        if len(set(names)) != len(names):
            raise AutonomousModelInventoryError("inventory providers must be unique")
        if (priors is None) == (prior_factory is None):
            raise AutonomousModelInventoryError("inventory requires exactly one prior source")
        if priors is not None:
            if not isinstance(priors, Mapping):
                raise AutonomousModelInventoryError("inventory priors must be an object")
            _safe_json(priors)
        if prior_factory is not None and not callable(prior_factory):
            raise AutonomousModelInventoryError("inventory prior_factory must be callable")
        normalized_requirements = self._normalize_requirements(domain_requirements)
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_AUTONOMOUS_MODEL_INVENTORY_MODELS_PER_PROVIDER:
            raise AutonomousModelInventoryError("inventory limit is outside its bound")
        if not isinstance(raise_on_error, bool):
            raise AutonomousModelInventoryError("inventory raise_on_error must be boolean")
        if snapshot_store is not None and not isinstance(snapshot_store, AutonomousModelInventoryStore):
            raise AutonomousModelInventoryError("inventory snapshot_store is invalid")
        refresh_nonce = content_digest({"providers": names, "at": self._clock()})[:32]
        resolved_refresh_id = _identifier("inventory refresh_id", refresh_id or f"inventory-{refresh_nonce}")
        started_at = _finite_time("inventory start clock", self._clock())
        credential_map = self._credentials(credentials)
        results: list[AutonomousModelInventoryProviderResult] = []
        for provider in names:
            requires_credential = False
            try:
                requires_credential = self.runtime.provider_requires_credential(provider)
                descriptors = self.runtime.discover_models(
                    provider,
                    credential=credential_map.get(provider),
                    limit=limit,
                )
                descriptor_digest = content_digest([descriptor.to_dict() for descriptor in descriptors])
                if prior_factory is not None:
                    resolved_priors_by_arm: dict[str, Mapping[str, Any]] = {}
                    for descriptor in descriptors:
                        prior = prior_factory(descriptor)
                        if not isinstance(prior, Mapping):
                            raise AutonomousModelInventoryError("inventory prior_factory must return objects")
                        _safe_json(prior)
                        resolved_priors_by_arm[descriptor.arm_id] = dict(prior)
                    resolved_priors: Mapping[str, Mapping[str, Any]] = resolved_priors_by_arm
                else:
                    resolved_priors = priors or {}
                reconciliation = self.catalogue.reconcile_discovered(
                    descriptors,
                    priors=resolved_priors,
                    providers=(provider,),
                )
                results.append(
                    AutonomousModelInventoryProviderResult(
                        provider=provider,
                        status="refreshed",
                        model_count=len(descriptors),
                        model_ids=tuple(descriptor.model for descriptor in descriptors),
                        descriptor_digest=descriptor_digest,
                        registered_model_ids=tuple(reconciliation["registered_model_ids"]),
                        replaced_model_ids=tuple(reconciliation["replaced_model_ids"]),
                        removed_model_ids=tuple(reconciliation["removed_model_ids"]),
                        requires_credential=requires_credential,
                        runtime_status=self._runtime_status(provider),
                    )
                )
            except CredentialError as error:
                if raise_on_error:
                    raise AutonomousModelInventoryError("provider inventory requires a credential") from error
                results.append(self._failed(provider, "credential_required", requires_credential, error))
            except ProviderError as error:
                if raise_on_error:
                    raise AutonomousModelInventoryError("provider inventory refresh failed") from error
                results.append(self._failed(provider, "provider_failed", requires_credential, error))
            except Exception as error:
                if raise_on_error:
                    raise AutonomousModelInventoryError("provider inventory refresh failed") from error
                results.append(self._failed(provider, "provider_failed", requires_credential, error))
        completed_at = _finite_time("inventory completion clock", self._clock())
        refreshed_count = sum(1 for result in results if result.status == "refreshed")
        status = "completed" if refreshed_count == len(results) else ("failed" if refreshed_count == 0 else "partial")
        coverage = tuple(
            self._coverage(domain, required)
            for domain, required in sorted(normalized_requirements.items())
        )
        snapshot = AutonomousModelInventorySnapshot(
            refresh_id=resolved_refresh_id,
            started_at=started_at,
            completed_at=max(completed_at, started_at),
            status=status,
            providers=tuple(results),
            coverage=coverage,
            catalogue_digest=content_digest(self.catalogue.to_dict()),
        )
        if snapshot_store is not None:
            snapshot_store.save(snapshot)
        return snapshot

    @staticmethod
    def _credentials(credentials: Mapping[str, CredentialHandle] | CredentialSession | None) -> dict[str, CredentialHandle]:
        if credentials is None:
            return {}
        if isinstance(credentials, CredentialSession):
            return credentials.handles()
        if not isinstance(credentials, Mapping):
            raise AutonomousModelInventoryError("inventory credentials must be a mapping or CredentialSession")
        result: dict[str, CredentialHandle] = {}
        for provider, handle in credentials.items():
            if not isinstance(provider, str) or not isinstance(handle, CredentialHandle):
                raise AutonomousModelInventoryError("inventory credentials must contain opaque credential handles")
            result[provider] = handle
        return result

    @staticmethod
    def _normalize_requirements(requirements: Mapping[str, Sequence[str]]) -> dict[str, tuple[str, ...]]:
        if not isinstance(requirements, Mapping):
            raise AutonomousModelInventoryError("inventory domain_requirements must be an object")
        if len(requirements) > MAX_AUTONOMOUS_MODEL_INVENTORY_DOMAINS:
            raise AutonomousModelInventoryError("inventory domain requirements exceed their bound")
        result: dict[str, tuple[str, ...]] = {}
        for domain, values in requirements.items():
            normalized_domain = _identifier("inventory requirement domain", domain)
            normalized = _string_tuple(
                f"inventory requirements for {normalized_domain}",
                values,
                maximum=MAX_AUTONOMOUS_MODEL_INVENTORY_CAPABILITIES,
            )
            if not normalized:
                raise AutonomousModelInventoryError("inventory domain requirements cannot be empty")
            result[normalized_domain] = normalized
        return result

    def _coverage(self, domain: str, required: tuple[str, ...]) -> AutonomousModelInventoryCoverage:
        report = self.catalogue.compatibility_report(required)
        compatible = tuple(
            row["arm_id"] for row in report["candidates"] if row.get("compatible") is True
        )
        return AutonomousModelInventoryCoverage(
            domain=domain,
            required_capabilities=required,
            compatible_arm_ids=compatible,
            candidate_count=report["candidate_count"],
            compatible_count=len(compatible),
        )

    def _runtime_status(self, provider: str) -> Mapping[str, Any] | None:
        try:
            raw = self.runtime.provider_status(provider)
            allowed = (
                "provider", "configured", "circuit", "consecutive_failures", "opened_until",
                "max_attempts", "attempts", "successes", "failures", "success_rate",
                "mean_latency_ms", "last_latency_ms", "last_model", "last_outcome",
                "last_status", "last_status_code", "last_circuit", "observed_at",
                "total_input_tokens", "total_output_tokens",
            )
            return {key: raw[key] for key in allowed if key in raw}
        except Exception:
            return None

    def _failed(
        self,
        provider: str,
        status: str,
        requires_credential: bool,
        error: BaseException,
    ) -> AutonomousModelInventoryProviderResult:
        return AutonomousModelInventoryProviderResult(
            provider=provider,
            status=status,
            model_count=0,
            model_ids=(),
            descriptor_digest=None,
            requires_credential=requires_credential,
            runtime_status=self._runtime_status(provider),
            error_class=_failure_class(error),
            error_code=_safe_error_code(error),
        )


__all__ = [
    "AUTONOMOUS_MODEL_INVENTORY_SCHEMA",
    "AUTONOMOUS_MODEL_INVENTORY_PROVIDER_SCHEMA",
    "AUTONOMOUS_MODEL_INVENTORY_COVERAGE_SCHEMA",
    "AUTONOMOUS_MODEL_INVENTORY_STORE_SCHEMA",
    "AUTONOMOUS_MODEL_INVENTORY_STATUSES",
    "AUTONOMOUS_MODEL_INVENTORY_PROVIDER_STATUSES",
    "MAX_AUTONOMOUS_MODEL_INVENTORY_PROVIDERS",
    "MAX_AUTONOMOUS_MODEL_INVENTORY_MODELS_PER_PROVIDER",
    "MAX_AUTONOMOUS_MODEL_INVENTORY_DOMAINS",
    "MAX_AUTONOMOUS_MODEL_INVENTORY_CAPABILITIES",
    "MAX_AUTONOMOUS_MODEL_INVENTORY_IDS",
    "MAX_AUTONOMOUS_MODEL_INVENTORY_SNAPSHOT_BYTES",
    "AutonomousModelInventoryError",
    "AutonomousModelInventoryProviderResult",
    "AutonomousModelInventoryCoverage",
    "AutonomousModelInventorySnapshot",
    "AutonomousModelInventoryStore",
    "AutonomousModelInventoryCoordinator",
]
