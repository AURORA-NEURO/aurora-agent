"""Reviewed all-domain evidence profiles and source-route composition.

The catalogue is the domain-facing seam between an :class:`AutonomousEvidencePlan` and the
caller-owned providers that can satisfy it.  Profiles describe what a source family means; routes
bind that description to one transient acquirer.  Registration and preparation are deliberately
request-free.  Execution is delegated to the bounded reconciliation runtime and still requires
explicit approval at the last possible boundary.

This module never discovers providers, accepts credentials, or stores acquired values.  Route
metadata is copied into a digest-bound plan while acquirer callables remain process-local and
transient.  A profile replacement cannot silently invalidate an already prepared route.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_reconciliation import (
    AutonomousEvidenceReconciliationPlan,
    AutonomousEvidenceReconciliationResult,
    AutonomousEvidenceReconciliationRoute,
    AutonomousEvidenceSourceReconciler,
)
from .autonomous_evidence_normalizers import (
    AutonomousEvidenceNormalizerRegistry,
    create_builtin_autonomous_evidence_normalizer_registry,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA = "bioprism-python-autonomous-domain-evidence-profile/0.1"
AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_SCHEMA = "bioprism-python-autonomous-domain-evidence-catalogue/0.1"
AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA = "bioprism-python-autonomous-domain-evidence-route/0.1"
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES = 128
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES = 512
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS = 64
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES = 64
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS = 32
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_METADATA_BYTES = 64_000
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_BYTES = 512_000

AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES = (
    "realtime", "bounded_cache", "historical", "caller_declared",
)
AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES = (
    "none", "caller_managed_credential", "caller_signed_request", "delegated_session",
)
AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES = (
    "none", "cursor", "page_number", "link_header", "caller_defined",
)

_RETENTION = "metadata_only;source_values_queries_and_credentials_caller_owned"
_PROFILE_RETENTION = "profile_metadata_only;source_values_caller_owned"
_PROFILE_EXECUTION = "profile_only;source_dispatch_not_started"
_ROUTE_RETENTION = "route_metadata_only;request_and_values_caller_owned"
_ROUTE_EXECUTION = "registered_route_only;source_dispatch_not_started"
_SECRET_MARKERS = frozenset({
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
})
_RESERVED_METADATA_KEY = "__aurora_domain_evidence_source"


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} is outside its identifier contract")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ArgumentError(f"{name} is outside its bound")
    return value


def _bounded_list(name: str, value: Any, maximum: int, *, minimum: int = 1) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not minimum <= len(value) <= maximum:
        raise ArgumentError(f"{name} must contain between {minimum} and {maximum} entries")
    normalized = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{name} contains duplicates")
    return tuple(sorted(normalized))


def _bounded_text_list(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    normalized = tuple(_text(f"{name}[{index}]", item, 2_048) for index, item in enumerate(value))
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{name} contains duplicates")
    return tuple(sorted(normalized))


def _enum(name: str, value: Any, allowed: Sequence[str]) -> str:
    if value not in allowed:
        raise ArgumentError(f"{name} is invalid")
    return value


def _secret_key(key: str) -> bool:
    normalized = "".join(character for character in key.lower() if character.isalnum())
    return normalized in _SECRET_MARKERS or normalized.startswith("gsk") or normalized.startswith("skproj") or any(
        marker in normalized for marker in ("token", "secret", "credential", "authorization")
    )


def _assert_safe_metadata(value: Any, name: str, depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ArgumentError(f"{name} contains a non-finite number")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid field")
            if _secret_key(key) or key == _RESERVED_METADATA_KEY:
                raise ArgumentError(f"{name}.{key} is credential-shaped or reserved metadata")
            _assert_safe_metadata(child, f"{name}.{key}", depth + 1)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_safe_metadata(child, f"{name}[{index}]", depth + 1)
        return
    raise ArgumentError(f"{name} is not JSON-safe")


def _safe_metadata(value: Any, name: str) -> dict[str, Any]:
    if value is None:
        value = {}
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    _assert_safe_metadata(value, name)
    try:
        encoded = canonical_json(value).encode("utf-8")
        normalized = json.loads(encoded.decode("utf-8"))
    except (TypeError, ValueError, UnicodeError) as error:
        raise ArgumentError(f"{name} is not canonical JSON") from error
    if len(encoded) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_METADATA_BYTES:
        raise ArgumentError(f"{name} exceeds its byte bound")
    if not isinstance(normalized, dict):
        raise ArgumentError(f"{name} must canonicalize to an object")
    return normalized


def _subset(name: str, values: Sequence[str], allowed: Sequence[str]) -> None:
    missing = sorted(set(values).difference(allowed))
    if missing:
        raise ArgumentError(f"{name} exceeds its profile contract: {', '.join(missing)}")


def _profile_payload(
    *, profile_id: str, version: str, domain: str, purpose: str, source_kinds: Sequence[str],
    capabilities: Sequence[str], operations: Sequence[str], required_metadata: Sequence[str],
    freshness: str, auth_mode: str, pagination: str, normalizer_id: str, normalizer_version: str,
    default_quorum: int, default_max_concurrency: int, limitations: Sequence[str],
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA,
        "profile_id": profile_id,
        "version": version,
        "domain": domain,
        "purpose": purpose,
        "source_kinds": list(source_kinds),
        "capabilities": list(capabilities),
        "operations": list(operations),
        "required_metadata": list(required_metadata),
        "freshness": freshness,
        "auth_mode": auth_mode,
        "pagination": pagination,
        "normalizer_id": normalizer_id,
        "normalizer_version": normalizer_version,
        "default_quorum": default_quorum,
        "default_max_concurrency": default_max_concurrency,
        "limitations": list(limitations),
        "execution": _PROFILE_EXECUTION,
        "retention": _PROFILE_RETENTION,
        "secret_material": "never_returned",
    }


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceSourceProfile:
    """One versioned source family; declaration never performs source work."""

    profile_id: str
    version: str
    domain: str
    purpose: str
    source_kinds: tuple[str, ...]
    capabilities: tuple[str, ...]
    operations: tuple[str, ...]
    required_metadata: tuple[str, ...] = ()
    freshness: str = "caller_declared"
    auth_mode: str = "caller_managed_credential"
    pagination: str = "none"
    normalizer_id: str = "identity"
    normalizer_version: str = "1"
    default_quorum: int = 1
    default_max_concurrency: int = 4
    limitations: tuple[str, ...] = ()
    profile_digest: str = field(init=False)

    def __post_init__(self) -> None:
        profile_id = _identifier("domain evidence profile_id", self.profile_id)
        version = _identifier("domain evidence profile version", self.version)
        domain = _identifier("domain evidence profile domain", self.domain)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("domain evidence profile domain is unsupported")
        purpose = _text("domain evidence profile purpose", self.purpose, 2_048)
        source_kinds = _bounded_list("domain evidence profile source_kinds", self.source_kinds, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS)
        capabilities = _bounded_list("domain evidence profile capabilities", self.capabilities, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES)
        operations = _bounded_list("domain evidence profile operations", self.operations, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS)
        required_metadata = _bounded_list("domain evidence profile required_metadata", self.required_metadata, 64, minimum=0)
        if any(_secret_key(key) or key == _RESERVED_METADATA_KEY for key in required_metadata):
            raise ArgumentError("domain evidence profile required_metadata is credential-shaped or reserved")
        freshness = _enum("domain evidence profile freshness", self.freshness, AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES)
        auth_mode = _enum("domain evidence profile auth_mode", self.auth_mode, AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES)
        pagination = _enum("domain evidence profile pagination", self.pagination, AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES)
        normalizer_id = _identifier("domain evidence profile normalizer_id", self.normalizer_id)
        normalizer_version = _identifier("domain evidence profile normalizer_version", self.normalizer_version)
        default_quorum = _bounded_integer("domain evidence profile default_quorum", self.default_quorum, 1, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES)
        default_max_concurrency = _bounded_integer("domain evidence profile default_max_concurrency", self.default_max_concurrency, 1, 8)
        limitations = _bounded_text_list("domain evidence profile limitations", self.limitations, 32)
        for name, value in (
            ("profile_id", profile_id), ("version", version), ("domain", domain), ("purpose", purpose),
            ("source_kinds", source_kinds), ("capabilities", capabilities), ("operations", operations),
            ("required_metadata", required_metadata), ("freshness", freshness), ("auth_mode", auth_mode),
            ("pagination", pagination), ("normalizer_id", normalizer_id), ("normalizer_version", normalizer_version),
            ("default_quorum", default_quorum), ("default_max_concurrency", default_max_concurrency), ("limitations", limitations),
        ):
            object.__setattr__(self, name, value)
        object.__setattr__(self, "profile_digest", content_digest(self._descriptor()))

    def _descriptor(self) -> dict[str, Any]:
        return _profile_payload(
            profile_id=self.profile_id, version=self.version, domain=self.domain, purpose=self.purpose,
            source_kinds=self.source_kinds, capabilities=self.capabilities, operations=self.operations,
            required_metadata=self.required_metadata, freshness=self.freshness, auth_mode=self.auth_mode,
            pagination=self.pagination, normalizer_id=self.normalizer_id, normalizer_version=self.normalizer_version,
            default_quorum=self.default_quorum, default_max_concurrency=self.default_max_concurrency,
            limitations=self.limitations,
        )

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "profile_digest": self.profile_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousDomainEvidenceSourceProfile":
        if not isinstance(value, Mapping):
            raise ArgumentError("domain evidence profile must be a mapping")
        expected = set(_profile_payload(
            profile_id="x", version="1", domain="coding", purpose="x", source_kinds=("x",),
            capabilities=("x",), operations=("x",), required_metadata=(), freshness="realtime",
            auth_mode="none", pagination="none", normalizer_id="x", normalizer_version="1",
            default_quorum=1, default_max_concurrency=1, limitations=("x",),
        )) | {"profile_digest"}
        if set(value) != expected or value.get("schema") != AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA:
            raise ArgumentError("domain evidence profile contains unsupported fields")
        if value.get("execution") != _PROFILE_EXECUTION or value.get("retention") != _PROFILE_RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("domain evidence profile retention is invalid")
        profile = cls(
            profile_id=value.get("profile_id"), version=value.get("version"), domain=value.get("domain"),
            purpose=value.get("purpose"), source_kinds=tuple(value.get("source_kinds", ())),
            capabilities=tuple(value.get("capabilities", ())), operations=tuple(value.get("operations", ())),
            required_metadata=tuple(value.get("required_metadata", ())), freshness=value.get("freshness"),
            auth_mode=value.get("auth_mode"), pagination=value.get("pagination"), normalizer_id=value.get("normalizer_id"),
            normalizer_version=value.get("normalizer_version"), default_quorum=value.get("default_quorum"),
            default_max_concurrency=value.get("default_max_concurrency"), limitations=tuple(value.get("limitations", ())),
        )
        if value.get("profile_digest") != profile.profile_digest or canonical_json(value) != canonical_json(profile.to_dict()):
            raise ArgumentError("domain evidence profile digest or canonical form is invalid")
        return profile


def _route_payload(
    *, source_id: str, profile: AutonomousDomainEvidenceSourceProfile, provider: str,
    source_kinds: Sequence[str], capabilities: Sequence[str], operations: Sequence[str],
    source_digest: str | None, request_id: str | None, contract_digest: str | None,
    adapter_id: str | None, adapter_manifest_digest: str | None, metadata_digest: str,
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA,
        "source_id": source_id,
        "profile_id": profile.profile_id,
        "profile_version": profile.version,
        "profile_digest": profile.profile_digest,
        "domain": profile.domain,
        "provider": provider,
        "source_kinds": list(source_kinds),
        "capabilities": list(capabilities),
        "operations": list(operations),
        "source_digest": source_digest,
        "request_id": request_id,
        "contract_digest": contract_digest,
        "adapter_id": adapter_id,
        "adapter_manifest_digest": adapter_manifest_digest,
        "metadata_digest": metadata_digest,
        "execution": _ROUTE_EXECUTION,
        "retention": _ROUTE_RETENTION,
        "secret_material": "never_returned",
    }


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceRoute:
    """A registered route with metadata projection plus a process-local acquirer."""

    json: Mapping[str, Any]
    metadata: Mapping[str, Any]
    acquirer: Any = field(compare=False, repr=False)

    def __post_init__(self) -> None:
        if not isinstance(self.json, Mapping) or not isinstance(self.metadata, Mapping) or not callable(getattr(self.acquirer, "acquire", None)):
            raise ArgumentError("domain evidence route is malformed")
        object.__setattr__(self, "json", dict(self.json))
        object.__setattr__(self, "metadata", _safe_metadata(self.metadata, "domain evidence route metadata"))

    @property
    def source_id(self) -> str:
        return str(self.json["source_id"])

    @property
    def route_digest(self) -> str:
        return str(self.json["route_digest"])

    def to_dict(self) -> dict[str, Any]:
        return dict(self.json)


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceCoverage:
    domain: str
    profile_ids: tuple[str, ...]
    route_count: int
    source_ids: tuple[str, ...]
    capabilities: tuple[str, ...]
    state: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "domain": self.domain, "profile_ids": list(self.profile_ids), "route_count": self.route_count,
            "source_ids": list(self.source_ids), "capabilities": list(self.capabilities),
            "state": self.state, "retention": "metadata_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceCatalogueReconciliation:
    profile: Mapping[str, Any]
    plan: AutonomousEvidenceReconciliationPlan
    routes: tuple[Mapping[str, Any], ...]
    normalizer_registry_digest: str

    def __post_init__(self) -> None:
        if not isinstance(self.profile, Mapping) or not isinstance(self.plan, AutonomousEvidenceReconciliationPlan) or not isinstance(self.routes, Sequence):
            raise ArgumentError("domain evidence prepared reconciliation is malformed")
        _digest("domain evidence prepared normalizer_registry_digest", self.normalizer_registry_digest)
        object.__setattr__(self, "profile", dict(self.profile))
        object.__setattr__(self, "routes", tuple(dict(route) for route in self.routes))


class AutonomousDomainEvidenceSourceCatalogue:
    """All-domain profile and route catalogue with digest-bound reconciliation preparation."""

    def __init__(
        self,
        profiles: Sequence[AutonomousDomainEvidenceSourceProfile] | None = None,
        *,
        require_all_domains: bool = False,
        normalizer_registry: AutonomousEvidenceNormalizerRegistry | None = None,
    ) -> None:
        if profiles is not None and (isinstance(profiles, (str, bytes, bytearray)) or not isinstance(profiles, Sequence)):
            raise ArgumentError("domain evidence catalogue profiles must be a sequence")
        selected = tuple(builtin_autonomous_domain_evidence_source_profiles() if profiles is None else profiles)
        if not 1 <= len(selected) <= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES:
            raise ArgumentError("domain evidence catalogue profiles are outside their bound")
        self._profiles: dict[str, AutonomousDomainEvidenceSourceProfile] = {}
        self._routes: dict[str, AutonomousDomainEvidenceRoute] = {}
        self.normalizer_registry = normalizer_registry or create_builtin_autonomous_evidence_normalizer_registry()
        if not isinstance(self.normalizer_registry, AutonomousEvidenceNormalizerRegistry):
            raise ArgumentError("domain evidence catalogue normalizer_registry is malformed")
        for profile in selected:
            self.register_profile(profile)
        if require_all_domains and any(domain not in {profile.domain for profile in self._profiles.values()} for domain in AUTONOMOUS_DOMAIN_NAMES):
            raise ArgumentError("domain evidence catalogue must cover every autonomous domain")

    def register_profile(self, profile: AutonomousDomainEvidenceSourceProfile, *, replace: bool = False) -> dict[str, Any]:
        if not isinstance(profile, AutonomousDomainEvidenceSourceProfile) or not isinstance(replace, bool):
            raise ArgumentError("domain evidence catalogue profile registration is malformed")
        existing = self._profiles.get(profile.profile_id)
        if existing is not None and not replace:
            raise ArgumentError(f"domain evidence profile is already registered: {profile.profile_id}")
        if existing is None and len(self._profiles) >= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES:
            raise ArgumentError("domain evidence catalogue profile capacity exceeded")
        if existing is not None and existing.domain != profile.domain:
            raise ArgumentError("replacing a domain evidence profile cannot change its domain")
        dependents = tuple(route for route in self._routes.values() if route.json.get("profile_id") == profile.profile_id)
        if dependents and existing is not None and existing.profile_digest != profile.profile_digest:
            raise ArgumentError("cannot replace a domain evidence profile while routes bind its previous digest")
        self._profiles[profile.profile_id] = profile
        self._assert_size()
        return profile.to_dict()

    def unregister_profile(self, profile_id: str) -> bool:
        normalized = _identifier("domain evidence profile_id", profile_id)
        if any(route.json.get("profile_id") == normalized for route in self._routes.values()):
            raise ArgumentError("cannot unregister a domain evidence profile with registered routes")
        return self._profiles.pop(normalized, None) is not None

    def profiles(self) -> tuple[dict[str, Any], ...]:
        return tuple(self._profiles[key].to_dict() for key in sorted(self._profiles))

    def profile(self, profile_id: str) -> AutonomousDomainEvidenceSourceProfile:
        normalized = _identifier("domain evidence profile_id", profile_id)
        profile = self._profiles.get(normalized)
        if profile is None:
            raise ArgumentError(f"domain evidence profile is not registered: {normalized}")
        return profile

    def register_route(
        self,
        *,
        source_id: str,
        profile_id: str,
        provider: str,
        acquirer: Any,
        source_kinds: Sequence[str] | None = None,
        capabilities: Sequence[str] | None = None,
        operations: Sequence[str] | None = None,
        source_digest: str | None = None,
        request_id: str | None = None,
        contract_digest: str | None = None,
        adapter_id: str | None = None,
        adapter_manifest_digest: str | None = None,
        metadata: Mapping[str, Any] | None = None,
        replace: bool = False,
        provider_contract_registry: Any = None,
    ) -> dict[str, Any]:
        if not callable(getattr(acquirer, "acquire", None)):
            raise ArgumentError("domain evidence route acquirer is required")
        if not isinstance(replace, bool):
            raise ArgumentError("domain evidence route replace must be boolean")
        source_id = _identifier("domain evidence source_id", source_id)
        provider = _identifier("domain evidence route provider", provider)
        profile = self.profile(profile_id)
        source_kinds = _bounded_list("domain evidence route source_kinds", profile.source_kinds if source_kinds is None else source_kinds, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS)
        capabilities = _bounded_list("domain evidence route capabilities", profile.capabilities if capabilities is None else capabilities, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES)
        operations = _bounded_list("domain evidence route operations", profile.operations if operations is None else operations, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS)
        _subset("domain evidence route source_kinds", source_kinds, profile.source_kinds)
        _subset("domain evidence route capabilities", capabilities, profile.capabilities)
        _subset("domain evidence route operations", operations, profile.operations)
        safe_metadata = _safe_metadata(metadata, "domain evidence route metadata")
        for required in profile.required_metadata:
            if required not in safe_metadata:
                raise ArgumentError(f"domain evidence route metadata is missing required field: {required}")
        source_digest = _digest("domain evidence route source_digest", source_digest, allow_none=True)
        request_id = None if request_id is None else _identifier("domain evidence route request_id", request_id)
        contract_digest = _digest("domain evidence route contract_digest", contract_digest, allow_none=True)
        adapter_id = None if adapter_id is None else _identifier("domain evidence route adapter_id", adapter_id)
        adapter_manifest_digest = _digest("domain evidence route adapter_manifest_digest", adapter_manifest_digest, allow_none=True)
        if adapter_manifest_digest is not None and adapter_id is None:
            raise ArgumentError("domain evidence route adapter_manifest_digest requires adapter_id")
        if contract_digest is not None and provider_contract_registry is not None:
            self._verify_contract_binding(
                provider_contract_registry, contract_digest, profile, provider, source_kinds, capabilities, operations,
                adapter_id, adapter_manifest_digest,
            )
        descriptor = _route_payload(
            source_id=source_id, profile=profile, provider=provider, source_kinds=source_kinds,
            capabilities=capabilities, operations=operations, source_digest=source_digest, request_id=request_id,
            contract_digest=contract_digest, adapter_id=adapter_id, adapter_manifest_digest=adapter_manifest_digest,
            metadata_digest=content_digest(safe_metadata),
        )
        route = {**descriptor, "route_digest": content_digest(descriptor)}
        existing = self._routes.get(source_id)
        if existing is not None and not replace:
            raise ArgumentError(f"domain evidence source is already registered: {source_id}")
        if existing is None and len(self._routes) >= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES:
            raise ArgumentError("domain evidence catalogue route capacity exceeded")
        self._routes[source_id] = AutonomousDomainEvidenceRoute(route, safe_metadata, acquirer)
        self._assert_size()
        return dict(route)

    def _verify_contract_binding(
        self, registry: Any, contract_digest: str, profile: AutonomousDomainEvidenceSourceProfile, provider: str,
        source_kinds: Sequence[str], capabilities: Sequence[str], operations: Sequence[str],
        adapter_id: str | None, adapter_manifest_digest: str | None,
    ) -> None:
        resolver = getattr(registry, "contracts", None)
        if not callable(resolver) or not callable(getattr(registry, "resolve", None)):
            raise ArgumentError("domain evidence provider contract registry is malformed")
        contract = next((candidate for candidate in resolver() if getattr(candidate, "contract_digest", None) == contract_digest), None)
        if contract is None:
            raise ArgumentError("domain evidence route contract is not registered")
        if contract.provider != provider or profile.domain not in contract.domains:
            raise ArgumentError("domain evidence route contract provider/domain binding changed")
        if (
            getattr(contract, "auth_mode", None) != profile.auth_mode
            or getattr(contract, "freshness", None) != profile.freshness
            or getattr(contract, "pagination", None) != profile.pagination
        ):
            raise ArgumentError("domain evidence route contract freshness, auth, or pagination binding changed")
        if any(required not in contract.required_metadata for required in profile.required_metadata):
            raise ArgumentError("domain evidence route contract is missing profile-required metadata")
        _subset("domain evidence route contract source_kinds", source_kinds, contract.source_kinds)
        _subset("domain evidence route contract capabilities", capabilities, contract.capabilities)
        _subset("domain evidence route contract operations", operations, contract.operations)
        if adapter_id is not None and contract.adapter_id != adapter_id:
            raise ArgumentError("domain evidence route adapter does not match its provider contract")
        if adapter_manifest_digest is not None and contract.adapter_manifest_digest != adapter_manifest_digest:
            raise ArgumentError("domain evidence route adapter manifest does not match its provider contract")

    def unregister_route(self, source_id: str) -> bool:
        return self._routes.pop(_identifier("domain evidence source_id", source_id), None) is not None

    def routes(self, *, domain: str | None = None, profile_id: str | None = None) -> tuple[dict[str, Any], ...]:
        if domain is not None:
            domain = _identifier("domain evidence route domain", domain)
            if domain not in AUTONOMOUS_DOMAIN_NAMES:
                raise ArgumentError("domain evidence route domain is unsupported")
        if profile_id is not None:
            profile_id = _identifier("domain evidence route profile_id", profile_id)
        return tuple(
            dict(route.json) for route in sorted(self._routes.values(), key=lambda item: item.source_id)
            if (domain is None or route.json["domain"] == domain) and (profile_id is None or route.json["profile_id"] == profile_id)
        )

    def route(self, source_id: str) -> dict[str, Any]:
        normalized = _identifier("domain evidence source_id", source_id)
        route = self._routes.get(normalized)
        if route is None:
            raise ArgumentError(f"domain evidence source is not registered: {normalized}")
        return dict(route.json)

    def coverage(self) -> tuple[AutonomousDomainEvidenceCoverage, ...]:
        result: list[AutonomousDomainEvidenceCoverage] = []
        for domain in AUTONOMOUS_DOMAIN_NAMES:
            routes = tuple(route for route in self._routes.values() if route.json["domain"] == domain)
            result.append(AutonomousDomainEvidenceCoverage(
                domain=domain,
                profile_ids=tuple(sorted({str(route.json["profile_id"]) for route in routes})),
                route_count=len(routes),
                source_ids=tuple(sorted(str(route.json["source_id"]) for route in routes)),
                capabilities=tuple(sorted({capability for route in routes for capability in route.json["capabilities"]})),
                state="missing" if not routes else "partial" if len(routes) == 1 else "ready",
            ))
        return tuple(result)

    def prepare(
        self,
        evidence_plan: AutonomousEvidencePlan,
        requirement_id: str,
        *,
        profile_id: str | None = None,
        source_ids: Sequence[str] | None = None,
        quorum: int | None = None,
        max_concurrency: int | None = None,
        require_all: bool = False,
        parent_evidence_digests: Sequence[str] = (),
    ) -> AutonomousDomainEvidenceCatalogueReconciliation:
        if not isinstance(evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("domain evidence preparation requires a typed evidence plan")
        requirement = next((item for item in evidence_plan.requirements if item.requirement_id == requirement_id), None)
        if requirement is None:
            raise ArgumentError(f"domain evidence requirement is not in the plan: {requirement_id}")
        if profile_id is not None:
            profile_id = _identifier("domain evidence preparation profile_id", profile_id)
        eligible = tuple(
            route for route in self._routes.values()
            if route.json["domain"] == requirement.domain
            and (profile_id is None or route.json["profile_id"] == profile_id)
            and all(capability in route.json["capabilities"] for capability in requirement.required_capabilities)
        )
        matching = eligible
        if source_ids is not None:
            if isinstance(source_ids, (str, bytes, bytearray)) or not isinstance(source_ids, Sequence) or not source_ids:
                raise ArgumentError("domain evidence source_ids must be a non-empty sequence")
            requested = set(_bounded_list("domain evidence source_ids", source_ids, MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES, minimum=1))
            if any(source_id not in {route.source_id for route in eligible} for source_id in requested):
                raise ArgumentError("domain evidence source_ids contain an ineligible or unknown route")
            matching = tuple(route for route in eligible if route.source_id in requested)
        if not matching:
            raise ArgumentError(f"no registered source route satisfies evidence requirement {requirement_id}")
        profile_ids = {str(route.json["profile_id"]) for route in matching}
        if len(profile_ids) != 1:
            raise ArgumentError("domain evidence preparation requires one explicit profile when eligible routes span profiles")
        profile = self.profile(next(iter(profile_ids)))
        if profile_id is not None and profile_id != profile.profile_id:
            raise ArgumentError("domain evidence preparation profile does not match eligible routes")
        routes = tuple(self._reconciliation_route(route) for route in sorted(matching, key=lambda item: item.source_id))
        reconciler = AutonomousEvidenceSourceReconciler(evidence_plan)
        plan = reconciler.prepare(
            requirement_id, routes, quorum=profile.default_quorum if quorum is None else quorum,
            max_concurrency=min(profile.default_max_concurrency if max_concurrency is None else max_concurrency, len(routes)),
            require_all=require_all, normalizer_id=profile.normalizer_id, normalizer_version=profile.normalizer_version,
            parent_evidence_digests=parent_evidence_digests,
        )
        return AutonomousDomainEvidenceCatalogueReconciliation(
            profile=profile.to_dict(), plan=plan,
            routes=tuple(dict(route.json) for route in sorted(matching, key=lambda item: item.source_id)),
            normalizer_registry_digest=self.normalizer_registry.registry_digest,
        )

    def execute(
        self,
        evidence_plan: AutonomousEvidencePlan,
        prepared: AutonomousDomainEvidenceCatalogueReconciliation,
        *,
        approve_source_dispatch: bool = False,
        normalizer: Any = None,
        profile_id: str | None = None,
    ) -> AutonomousEvidenceReconciliationResult:
        if not isinstance(evidence_plan, AutonomousEvidencePlan) or not isinstance(prepared, AutonomousDomainEvidenceCatalogueReconciliation):
            raise ArgumentError("domain evidence execution requires typed plan and prepared reconciliation")
        prepared_profile_id = prepared.profile.get("profile_id")
        profile = self.profile(prepared_profile_id)
        if profile_id is not None and _identifier("domain evidence execution profile_id", profile_id) != profile.profile_id:
            raise ArgumentError("domain evidence execution profile does not match prepared reconciliation")
        if prepared.profile.get("profile_digest") != profile.profile_digest or prepared.profile.get("normalizer_id") != profile.normalizer_id or prepared.profile.get("normalizer_version") != profile.normalizer_version:
            raise ArgumentError("domain evidence profile changed after preparation")
        if prepared.normalizer_registry_digest != self.normalizer_registry.registry_digest:
            raise ArgumentError("domain evidence normalizer registry changed after preparation")
        route_entries: list[AutonomousEvidenceReconciliationRoute] = []
        for planned in prepared.routes:
            source_id = _identifier("domain evidence prepared source_id", planned.get("source_id"))
            if planned.get("profile_id") != profile.profile_id or planned.get("profile_digest") != profile.profile_digest or planned.get("domain") != profile.domain:
                raise ArgumentError(f"domain evidence prepared route is not bound to profile {profile.profile_id}")
            current = self._routes.get(source_id)
            if current is None:
                raise ArgumentError(f"domain evidence source route disappeared after preparation: {source_id}")
            if current.route_digest != planned.get("route_digest"):
                raise ArgumentError(f"domain evidence source route changed after preparation: {source_id}")
            route_entries.append(self._reconciliation_route(current))
        if normalizer is None:
            self.normalizer_registry.resolve(profile.domain, profile.normalizer_id, profile.normalizer_version)
            normalizer = lambda value, context: self.normalizer_registry.normalize(
                profile.domain, profile.normalizer_id, profile.normalizer_version, value, context,
            )
        reconciler = AutonomousEvidenceSourceReconciler(evidence_plan)
        return reconciler.execute(
            prepared.plan, route_entries, approve_source_dispatch=approve_source_dispatch, normalizer=normalizer,
            normalizer_id=profile.normalizer_id, normalizer_version=profile.normalizer_version,
        )

    def reconcile(self, evidence_plan: AutonomousEvidencePlan, requirement_id: str, **options: Any) -> AutonomousEvidenceReconciliationResult:
        prepared = self.prepare(evidence_plan, requirement_id, **{key: value for key, value in options.items() if key not in {"approve_source_dispatch", "normalizer"}})
        return self.execute(
            evidence_plan, prepared, approve_source_dispatch=options.get("approve_source_dispatch", False),
            normalizer=options.get("normalizer"), profile_id=options.get("profile_id"),
        )

    @property
    def registry_digest(self) -> str:
        return content_digest(self._descriptor())

    def _descriptor(self) -> dict[str, Any]:
        profiles = list(self.profiles())
        routes = list(self.routes())
        coverage = [row.to_dict() for row in self.coverage()]
        return {
            "schema": AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_SCHEMA, "profiles": profiles, "routes": routes,
            "coverage": coverage, "profile_count": len(profiles), "route_count": len(routes),
            "covered_domain_count": sum(row["state"] != "missing" for row in coverage),
            "normalizer_registry_digest": self.normalizer_registry.registry_digest,
            "normalizer_count": len(self.normalizer_registry.registrations()),
            "execution": "catalogue_and_route_validation_only;source_dispatch_requires_review",
            "retention": _RETENTION, "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        descriptor = self._descriptor()
        if len(canonical_json(descriptor).encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_BYTES:
            raise ArgumentError("domain evidence catalogue exceeds its byte bound")
        return {**descriptor, "registry_digest": self.registry_digest}

    def _reconciliation_route(self, route: AutonomousDomainEvidenceRoute) -> AutonomousEvidenceReconciliationRoute:
        metadata = dict(route.metadata)
        metadata.update({
            "catalogue_profile_id": route.json["profile_id"],
            "catalogue_profile_digest": route.json["profile_digest"],
            "catalogue_route_digest": route.json["route_digest"],
            "catalogue_source_kinds": list(route.json["source_kinds"]),
            "catalogue_capabilities": list(route.json["capabilities"]),
            "catalogue_operations": list(route.json["operations"]),
        })
        return AutonomousEvidenceReconciliationRoute(
            source_id=route.source_id, source_digest=route.json["source_digest"], request_id=route.json["request_id"],
            metadata=metadata, acquirer=route.acquirer,
        )

    def _assert_size(self) -> None:
        self.to_dict()


def builtin_autonomous_domain_evidence_source_profiles() -> tuple[AutonomousDomainEvidenceSourceProfile, ...]:
    definitions = (
        ("builtin.coding.evidence", "coding", "Repository, change, test, and delivery evidence for engineering tasks.", ("repository", "issue_tracker", "ci", "artifact_registry"), ("review", "debugging", "implementation", "testing"), ("repository_snapshot", "change_set", "test_run", "delivery_receipt"), "realtime", "caller_managed_credential", "cursor", "builtin.coding.claim-projection", 1, ("source access and repository truth remain caller-owned", "test evidence is not inferred from a provider response")),
        ("builtin.browser.evidence", "browser", "Fresh web retrieval, citation identity, and independent source comparison.", ("web_search", "web_page", "archive", "feed"), ("web_research", "navigation", "source_comparison"), ("search", "retrieve", "compare", "freshness_check"), "realtime", "caller_managed_credential", "cursor", "builtin.browser.claim-projection", 2, ("retrieval does not establish truth", "robots, access, freshness, and citation authority remain caller-owned")),
        ("builtin.data.evidence", "data", "Dataset schema, lineage, quality, and transformation evidence.", ("dataset", "schema_registry", "lineage_store", "quality_report"), ("schema_validation", "lineage", "quality_control", "data_analysis"), ("schema", "lineage", "quality", "profile", "transformation_check"), "bounded_cache", "caller_managed_credential", "page_number", "builtin.data.claim-projection", 1, ("schema and lineage declarations are not independently verified by this catalogue", "raw rows remain outside the SDK projection")),
        ("builtin.science.evidence", "science", "Literature, measurements, hypotheses, experimental design, and reproducibility evidence.", ("literature", "registry", "measurement", "experiment_log"), ("literature", "hypothesis", "experiment", "statistics", "reproducibility"), ("literature_search", "evidence_map", "measurement", "design", "reproduction"), "historical", "caller_managed_credential", "cursor", "builtin.science.claim-projection", 2, ("citation retrieval is not causal validation", "the evaluator must distinguish hypothesis, correlation, and causal claim")),
        ("builtin.biomedical.evidence", "biomedical", "Biomedical provenance, population applicability, safety boundaries, and human-review evidence.", ("literature", "guideline", "clinical_dataset", "safety_review"), ("biomedical_review", "provenance", "safety_boundary", "human_review"), ("evidence", "population", "provenance", "safety", "escalation"), "bounded_cache", "caller_managed_credential", "cursor", "builtin.biomedical.claim-projection", 2, ("no diagnosis, prescription, triage, or clinical authorization", "individual decisions require qualified human and institutional review")),
        ("builtin.neuroscience.evidence", "neuroscience", "Neural measurement, preprocessing, signal interpretation, model sensitivity, and reproducibility evidence.", ("neuro_dataset", "signal_store", "literature", "study_registry"), ("neuroscience_analysis", "signal_interpretation", "study_design", "reproducibility"), ("measurement", "preprocess", "model", "interpretation", "reproduction"), "historical", "caller_managed_credential", "page_number", "builtin.neuroscience.claim-projection", 1, ("signal transport and preprocessing are not supplied by the catalogue", "biological interpretation remains bounded by measurement and confound evidence")),
        ("builtin.operations.evidence", "operations", "Telemetry, incidents, runbooks, blast radius, rollback, and approval evidence.", ("telemetry", "incident_system", "runbook", "change_system"), ("observability", "incident_response", "risk_review", "rollback", "approval", "runbook"), ("observe", "impact", "rollback", "approval", "runbook"), "realtime", "delegated_session", "cursor", "builtin.operations.claim-projection", 1, ("observations do not authorize an effect", "rollback and external state require a separate effect and reconciliation boundary")),
        ("builtin.enterprise.evidence", "enterprise", "Business workflow, policy, compliance, ownership, and audit evidence.", ("workflow", "policy_registry", "audit_log", "risk_register"), ("workflow", "governance", "compliance", "analytics", "coordination"), ("request", "policy", "options", "decision", "audit"), "bounded_cache", "delegated_session", "page_number", "builtin.enterprise.claim-projection", 1, ("policy text is not authorization", "ownership and approval authority remain external organizational controls")),
        ("builtin.multi-agent.evidence", "multi_agent", "Bounded delegation, specialist handoff, conflict reconciliation, and synthesis evidence.", ("agent_report", "mission_log", "trace", "handoff"), ("delegation", "coordination", "consensus", "conflict_resolution", "handoff"), ("decompose", "delegate", "reconcile", "synthesize", "handoff"), "realtime", "delegated_session", "cursor", "builtin.multi-agent.claim-projection", 2, ("agent agreement is not independent truth", "one accountable authority must own any external effect")),
        ("builtin.multimodal.evidence", "multimodal", "Asset identity, modality transport, cross-modal alignment, and missing-modality evidence.", ("image", "audio", "video", "document", "asset_registry"), ("image", "audio", "video", "document", "cross_modal_alignment"), ("asset", "modality", "transport", "alignment", "comparison"), "caller_declared", "caller_managed_credential", "none", "builtin.multimodal.claim-projection", 1, ("the catalogue does not inspect raw media", "absence of a modality must remain explicit rather than inferred away")),
        ("builtin.cross-domain.evidence", "cross_domain", "Evidence alignment across domain specialists, synthesis inputs, and workflow composition.", ("domain_evidence", "synthesis_input", "lineage", "workflow"), ("routing", "synthesis", "evidence_alignment", "workflow_composition"), ("route", "synthesis", "alignment", "composition"), "caller_declared", "delegated_session", "cursor", "builtin.cross-domain.claim-projection", 2, ("cross-domain synthesis cannot erase specialist evaluator boundaries", "route composition does not grant domain authority")),
        ("builtin.evaluation.evidence", "evaluation", "Benchmark, rubric, oracle, replay, failure, and reproducibility evidence.", ("benchmark", "oracle", "replay", "evaluation_log"), ("benchmarking", "rubric", "replay", "failure_analysis", "reproducibility"), ("benchmark", "rubric", "replay", "failure", "reproduction"), "historical", "caller_managed_credential", "page_number", "builtin.evaluation.claim-projection", 2, ("the system under evaluation cannot author its own pass signal", "oracle independence and holdout integrity remain evaluator-owned")),
    )
    result: list[AutonomousDomainEvidenceSourceProfile] = []
    for profile_id, domain, purpose, source_kinds, capabilities, operations, freshness, auth_mode, pagination, normalizer_id, quorum, limitations in definitions:
        result.append(AutonomousDomainEvidenceSourceProfile(
            profile_id=profile_id, version="1", domain=domain, purpose=purpose, source_kinds=source_kinds,
            capabilities=capabilities, operations=operations, required_metadata=("operation",), freshness=freshness,
            auth_mode=auth_mode, pagination=pagination, normalizer_id=normalizer_id, normalizer_version="1",
            default_quorum=quorum, default_max_concurrency=4, limitations=limitations,
        ))
    return tuple(result)


def create_builtin_autonomous_domain_evidence_source_catalogue() -> AutonomousDomainEvidenceSourceCatalogue:
    return AutonomousDomainEvidenceSourceCatalogue(builtin_autonomous_domain_evidence_source_profiles(), require_all_domains=True)


def domain_evidence_request_identity(context: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(context, Mapping) or not isinstance(context.get("request"), Mapping):
        raise ArgumentError("domain evidence acquisition context is malformed")
    request = context["request"]
    metadata = request.get("metadata", {})
    return {
        "plan_digest": context.get("plan_digest"), "requirement_id": request.get("requirement_id"),
        "source_id": request.get("source_id"), "source_digest": request.get("source_digest"),
        "request_id": request.get("request_id"), "metadata_digest": content_digest(metadata),
        "attempt": context.get("attempt", 1), "secret_material": "never_returned",
    }


__all__ = [
    "AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SCHEMA", "AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_SCHEMA",
    "AUTONOMOUS_DOMAIN_EVIDENCE_ROUTE_SCHEMA", "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILES",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_ROUTES", "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_OPERATIONS",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_CAPABILITIES", "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_PROFILE_SOURCE_KINDS",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_METADATA_BYTES", "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_CATALOGUE_BYTES",
    "AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES", "AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES",
    "AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES", "AutonomousDomainEvidenceSourceProfile",
    "AutonomousDomainEvidenceRoute", "AutonomousDomainEvidenceCoverage",
    "AutonomousDomainEvidenceCatalogueReconciliation", "AutonomousDomainEvidenceSourceCatalogue",
    "builtin_autonomous_domain_evidence_source_profiles", "create_builtin_autonomous_domain_evidence_source_catalogue",
    "domain_evidence_request_identity",
]
