"""Provider-neutral HTTP source presets for the reviewed domain evidence catalogue.

Presets are stable metadata, not provider clients.  They give an embedding a safe default contract
for each autonomous domain while leaving endpoint construction, authentication, response parsing,
and credential lifetime in caller code.  The matrix helper preflights all entries and registers
routes only; it never invokes an endpoint resolver or opens a network connection.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_domain_evidence_catalogue import (
    AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES,
    AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES,
    AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES,
    AutonomousDomainEvidenceSourceCatalogue,
    AutonomousDomainEvidenceSourceProfile,
    builtin_autonomous_domain_evidence_source_profiles,
)
from .autonomous_evidence_retry import AutonomousEvidenceAcquisitionError
from .autonomous_http_connector import (
    AutonomousHttpConnectorPolicy,
    create_autonomous_http_connector_executor,
    create_autonomous_http_paginated_connector_executor,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA = "bioprism-python-autonomous-domain-http-source-preset/0.1"
AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_REGISTRATION_SCHEMA = "bioprism-python-autonomous-domain-http-source-preset-registration/0.1"
AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_SCHEMA = "bioprism-python-autonomous-domain-http-source-matrix/0.1"
MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESETS = 64
MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_ENTRIES = 128
MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_BYTES = 16_000

_PRESET_EXECUTION = "preset_metadata_only;caller_transport_and_source_interpretation_required"
_PRESET_RETENTION = "preset_metadata_only;credentials_requests_and_source_values_caller_owned"
_REGISTRATION_EXECUTION = "registered_only;HTTP_dispatch_requires_catalogue_approval"
_REGISTRATION_RETENTION = "route_and_manifest_metadata_only;requests_headers_responses_and_credentials_caller_owned"


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} is outside its identifier contract")
    return result


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _selected_list(name: str, supplied: Sequence[str] | None, allowed: Sequence[str]) -> tuple[str, ...]:
    selected = tuple(allowed) if supplied is None else tuple(supplied)
    if isinstance(supplied, (str, bytes, bytearray)) or not selected:
        raise ArgumentError(f"{name} must be a non-empty string sequence")
    normalized = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(selected))
    if len(set(normalized)) != len(normalized) or any(item not in allowed for item in normalized):
        raise ArgumentError(f"{name} must be a unique subset of its preset contract")
    return normalized


def _preset_descriptor(
    *, preset_id: str, version: str, profile_id: str, profile_digest: str, domain: str,
    provider_protocol: str, default_provider: str, default_adapter_id: str, default_contract_id: str,
    source_kinds: Sequence[str], capabilities: Sequence[str], operations: Sequence[str],
    required_metadata: Sequence[str], freshness: str, auth_mode: str, pagination: str,
    normalizer_id: str, normalizer_version: str, limitations: Sequence[str],
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA, "preset_id": preset_id, "version": version,
        "profile_id": profile_id, "profile_digest": profile_digest, "domain": domain,
        "provider_protocol": provider_protocol, "default_provider": default_provider,
        "default_adapter_id": default_adapter_id, "default_contract_id": default_contract_id,
        "source_kinds": list(source_kinds), "capabilities": list(capabilities), "operations": list(operations),
        "required_metadata": list(required_metadata), "freshness": freshness, "auth_mode": auth_mode,
        "pagination": pagination, "normalizer_id": normalizer_id, "normalizer_version": normalizer_version,
        "limitations": list(limitations), "execution": _PRESET_EXECUTION, "retention": _PRESET_RETENTION,
        "secret_material": "never_returned",
    }


@dataclass(frozen=True, slots=True)
class AutonomousDomainHttpSourcePreset:
    """Digest-bound metadata for a caller-managed HTTP JSON route."""

    preset_id: str
    version: str
    profile_id: str
    profile_digest: str
    domain: str
    provider_protocol: str
    default_provider: str
    default_adapter_id: str
    default_contract_id: str
    source_kinds: tuple[str, ...]
    capabilities: tuple[str, ...]
    operations: tuple[str, ...]
    required_metadata: tuple[str, ...]
    freshness: str
    auth_mode: str
    pagination: str
    normalizer_id: str
    normalizer_version: str
    limitations: tuple[str, ...]
    preset_digest: str = field(init=False)

    def __post_init__(self) -> None:
        values = {
            "preset_id": _identifier("domain HTTP source preset_id", self.preset_id),
            "version": _identifier("domain HTTP source preset version", self.version),
            "profile_id": _identifier("domain HTTP source profile_id", self.profile_id),
            "profile_digest": _digest("domain HTTP source profile_digest", self.profile_digest),
            "domain": _identifier("domain HTTP source domain", self.domain),
            "provider_protocol": _identifier("domain HTTP source provider_protocol", self.provider_protocol),
            "default_provider": _identifier("domain HTTP source default_provider", self.default_provider),
            "default_adapter_id": _identifier("domain HTTP source default_adapter_id", self.default_adapter_id),
            "default_contract_id": _identifier("domain HTTP source default_contract_id", self.default_contract_id),
            "normalizer_id": _identifier("domain HTTP source normalizer_id", self.normalizer_id),
            "normalizer_version": _identifier("domain HTTP source normalizer_version", self.normalizer_version),
        }
        if values["domain"] not in AUTONOMOUS_DOMAIN_NAMES or self.provider_protocol != "http_json":
            raise ArgumentError("domain HTTP source preset domain or protocol is invalid")
        values["source_kinds"] = _selected_list("domain HTTP source preset source_kinds", self.source_kinds, self.source_kinds)
        values["capabilities"] = _selected_list("domain HTTP source preset capabilities", self.capabilities, self.capabilities)
        values["operations"] = _selected_list("domain HTTP source preset operations", self.operations, self.operations)
        values["required_metadata"] = tuple(_identifier(f"domain HTTP source required_metadata[{index}]", item) for index, item in enumerate(self.required_metadata))
        if len(set(values["required_metadata"])) != len(values["required_metadata"]):
            raise ArgumentError("domain HTTP source preset required_metadata contains duplicates")
        if self.freshness not in AUTONOMOUS_DOMAIN_EVIDENCE_FRESHNESS_MODES or self.auth_mode not in AUTONOMOUS_DOMAIN_EVIDENCE_AUTH_MODES or self.pagination not in AUTONOMOUS_DOMAIN_EVIDENCE_PAGINATION_MODES:
            raise ArgumentError("domain HTTP source preset freshness, auth_mode, or pagination is invalid")
        values["limitations"] = tuple(_text(f"domain HTTP source preset limitations[{index}]", item, 2_048) for index, item in enumerate(self.limitations))
        for name, value in values.items():
            object.__setattr__(self, name, value)
        object.__setattr__(self, "preset_digest", content_digest(self._descriptor()))

    def _descriptor(self) -> dict[str, Any]:
        return _preset_descriptor(
            preset_id=self.preset_id, version=self.version, profile_id=self.profile_id, profile_digest=self.profile_digest,
            domain=self.domain, provider_protocol=self.provider_protocol, default_provider=self.default_provider,
            default_adapter_id=self.default_adapter_id, default_contract_id=self.default_contract_id,
            source_kinds=self.source_kinds, capabilities=self.capabilities, operations=self.operations,
            required_metadata=self.required_metadata, freshness=self.freshness, auth_mode=self.auth_mode,
            pagination=self.pagination, normalizer_id=self.normalizer_id, normalizer_version=self.normalizer_version,
            limitations=self.limitations,
        )

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "preset_digest": self.preset_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousDomainHttpSourcePreset":
        if not isinstance(value, Mapping):
            raise ArgumentError("domain HTTP source preset must be a mapping")
        expected = set(_preset_descriptor(
            preset_id="x", version="1", profile_id="x", profile_digest="0" * 64, domain="coding",
            provider_protocol="http_json", default_provider="x", default_adapter_id="x", default_contract_id="x",
            source_kinds=("x",), capabilities=("x",), operations=("x",), required_metadata=(), freshness="realtime",
            auth_mode="none", pagination="none", normalizer_id="x", normalizer_version="1", limitations=("x",),
        )) | {"preset_digest"}
        if set(value) != expected or value.get("schema") != AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA:
            raise ArgumentError("domain HTTP source preset contains unsupported fields")
        if value.get("execution") != _PRESET_EXECUTION or value.get("retention") != _PRESET_RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("domain HTTP source preset retention is invalid")
        preset = cls(
            preset_id=value.get("preset_id"), version=value.get("version"), profile_id=value.get("profile_id"),
            profile_digest=value.get("profile_digest"), domain=value.get("domain"), provider_protocol=value.get("provider_protocol"),
            default_provider=value.get("default_provider"), default_adapter_id=value.get("default_adapter_id"),
            default_contract_id=value.get("default_contract_id"), source_kinds=tuple(value.get("source_kinds", ())),
            capabilities=tuple(value.get("capabilities", ())), operations=tuple(value.get("operations", ())),
            required_metadata=tuple(value.get("required_metadata", ())), freshness=value.get("freshness"),
            auth_mode=value.get("auth_mode"), pagination=value.get("pagination"), normalizer_id=value.get("normalizer_id"),
            normalizer_version=value.get("normalizer_version"), limitations=tuple(value.get("limitations", ())),
        )
        if value.get("preset_digest") != preset.preset_digest or canonical_json(value) != canonical_json(preset.to_dict()):
            raise ArgumentError("domain HTTP source preset digest or canonical form is invalid")
        return preset


def _preset_from_profile(profile: AutonomousDomainEvidenceSourceProfile) -> AutonomousDomainHttpSourcePreset:
    return AutonomousDomainHttpSourcePreset(
        preset_id=f"builtin.http.{profile.domain}", version=profile.version, profile_id=profile.profile_id,
        profile_digest=profile.profile_digest, domain=profile.domain, provider_protocol="http_json",
        default_provider=f"caller-http-{profile.domain}", default_adapter_id=f"builtin.http.{profile.domain}.adapter",
        default_contract_id=f"builtin.http.{profile.domain}.contract", source_kinds=profile.source_kinds,
        capabilities=profile.capabilities, operations=profile.operations, required_metadata=profile.required_metadata,
        freshness=profile.freshness, auth_mode=profile.auth_mode, pagination=profile.pagination,
        normalizer_id=profile.normalizer_id, normalizer_version=profile.normalizer_version, limitations=profile.limitations,
    )


def builtin_autonomous_domain_http_source_presets() -> tuple[AutonomousDomainHttpSourcePreset, ...]:
    profiles = builtin_autonomous_domain_evidence_source_profiles()
    if len(profiles) > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESETS:
        raise ArgumentError("built-in domain HTTP source preset capacity exceeded")
    presets = tuple(_preset_from_profile(profile) for profile in profiles)
    if any(len(canonical_json(preset.to_dict()).encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_BYTES for preset in presets):
        raise ArgumentError("built-in domain HTTP source preset exceeds its metadata bound")
    return presets


def _resolve_preset(catalogue: AutonomousDomainEvidenceSourceCatalogue, value: AutonomousDomainHttpSourcePreset | str) -> AutonomousDomainHttpSourcePreset:
    if not isinstance(catalogue, AutonomousDomainEvidenceSourceCatalogue):
        raise ArgumentError("domain HTTP source preset requires a typed evidence catalogue")
    if isinstance(value, str):
        preset = next((candidate for candidate in builtin_autonomous_domain_http_source_presets() if candidate.preset_id == value), None)
    else:
        preset = AutonomousDomainHttpSourcePreset.from_dict(value) if isinstance(value, Mapping) else value
    if not isinstance(preset, AutonomousDomainHttpSourcePreset):
        raise ArgumentError(f"unknown domain HTTP source preset: {value}")
    profile = catalogue.profile(preset.profile_id)
    if profile.domain != preset.domain or profile.profile_digest != preset.profile_digest:
        raise ArgumentError("domain HTTP source preset is stale or bound to a different catalogue profile")
    return preset


def _contract_for_digest(registry: Any, contract_digest: str | None) -> Any:
    if contract_digest is None:
        return None
    contracts = getattr(registry, "contracts", None)
    if not callable(contracts):
        raise ArgumentError("domain HTTP provider contract registry is malformed")
    return next((contract for contract in contracts() if getattr(contract, "contract_digest", None) == contract_digest), None)


class AutonomousDomainHttpSourceAcquirer:
    """Transient adapter from the bounded HTTP connector executor to evidence acquisition."""

    def __init__(self, executor: Callable[[Any, Mapping[str, Any]], Any], manifest: Any) -> None:
        if not callable(executor):
            raise ArgumentError("domain HTTP source executor must be callable")
        self._executor = executor
        self._manifest = manifest

    def acquire(self, context: Mapping[str, Any]) -> Any:
        if not isinstance(context, Mapping) or not isinstance(context.get("request"), Mapping):
            raise ArgumentError("domain HTTP source acquisition context is malformed")
        observation = self._executor(self._manifest, context["request"])
        status = getattr(observation, "status", None)
        if status == "observed":
            return getattr(observation, "value", None)
        failure_class = getattr(observation, "failure_class", None) or "http_dispatch_failed"
        retryable = failure_class in {"timeout", "transport_error", "rate_limited", "http_5xx", "page_transport"}
        raise AutonomousEvidenceAcquisitionError(failure_class, retryable)


def create_autonomous_domain_http_source_acquirer(
    endpoint_resolver: Callable[[Any, Mapping[str, Any]], Any], *,
    policy: AutonomousHttpConnectorPolicy | None = None,
    header_resolver: Callable[[Any, Mapping[str, Any]], Mapping[str, str]] | None = None,
    opener: Callable[[Any, float], Any] | None = None,
    paginated: bool = False,
    page_parser: Callable[[Any, int], Any] | None = None,
    max_pages: int = 8,
    max_items: int = 512,
    manifest: Any = None,
) -> AutonomousDomainHttpSourceAcquirer:
    if not callable(endpoint_resolver):
        raise ArgumentError("domain HTTP source endpoint_resolver must be callable")
    executor = create_autonomous_http_paginated_connector_executor(
        endpoint_resolver, policy=policy, header_resolver=header_resolver, opener=opener,
        page_parser=page_parser, max_pages=max_pages, max_items=max_items,
    ) if paginated else create_autonomous_http_connector_executor(
        endpoint_resolver, policy=policy, header_resolver=header_resolver, opener=opener,
    )
    return AutonomousDomainHttpSourceAcquirer(executor, {} if manifest is None else manifest)


def register_autonomous_domain_http_source_preset(
    *,
    catalogue: AutonomousDomainEvidenceSourceCatalogue,
    preset: AutonomousDomainHttpSourcePreset | Mapping[str, Any] | str,
    source_id: str,
    acquirer: Any,
    provider: str | None = None,
    adapter_id: str | None = None,
    adapter_manifest_digest: str | None = None,
    contract_digest: str | None = None,
    source_kinds: Sequence[str] | None = None,
    capabilities: Sequence[str] | None = None,
    operations: Sequence[str] | None = None,
    source_digest: str | None = None,
    request_id: str | None = None,
    metadata: Mapping[str, Any] | None = None,
    replace: bool = False,
    provider_contract_registry: Any = None,
) -> dict[str, Any]:
    resolved = _resolve_preset(catalogue, preset)
    contract = _contract_for_digest(provider_contract_registry, contract_digest) if provider_contract_registry is not None else None
    if contract_digest is not None and contract is None:
        raise ArgumentError("domain HTTP source contract is not registered")
    resolved_provider = provider or (None if contract is None else contract.provider) or resolved.default_provider
    resolved_adapter_id = adapter_id or (None if contract is None else contract.adapter_id) or resolved.default_adapter_id
    resolved_adapter_manifest_digest = adapter_manifest_digest or (None if contract is None else contract.adapter_manifest_digest)
    selected_operations = _selected_list("domain HTTP source operations", operations, resolved.operations)
    selected_metadata = dict(metadata or {})
    if "operation" not in selected_metadata:
        selected_metadata["operation"] = selected_operations[0]
    if not isinstance(selected_metadata["operation"], str) or selected_metadata["operation"] not in selected_operations:
        raise ArgumentError("domain HTTP source metadata.operation must be one of the selected preset operations")
    route = catalogue.register_route(
        source_id=source_id, profile_id=resolved.profile_id, provider=resolved_provider,
        acquirer=acquirer, source_kinds=_selected_list("domain HTTP source source_kinds", source_kinds, resolved.source_kinds),
        capabilities=_selected_list("domain HTTP source capabilities", capabilities, resolved.capabilities),
        operations=selected_operations, source_digest=source_digest, request_id=request_id,
        contract_digest=contract_digest, adapter_id=resolved_adapter_id,
        adapter_manifest_digest=resolved_adapter_manifest_digest, metadata=selected_metadata, replace=replace,
        provider_contract_registry=provider_contract_registry,
    )
    return {
        "schema": AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_REGISTRATION_SCHEMA,
        "preset_id": resolved.preset_id, "preset_digest": resolved.preset_digest, "route": route,
        "execution": _REGISTRATION_EXECUTION, "retention": _REGISTRATION_RETENTION, "secret_material": "never_returned",
    }


def register_autonomous_domain_http_source_matrix(
    *,
    catalogue: AutonomousDomainEvidenceSourceCatalogue,
    entries: Sequence[Mapping[str, Any]],
    replace: bool = False,
    require_all_domains: bool = True,
    provider_contract_registry: Any = None,
) -> dict[str, Any]:
    if not isinstance(catalogue, AutonomousDomainEvidenceSourceCatalogue) or isinstance(entries, (str, bytes, bytearray)) or not isinstance(entries, Sequence) or not 1 <= len(entries) <= MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_ENTRIES:
        raise ArgumentError("domain HTTP source matrix inputs are outside their bound")
    if not isinstance(replace, bool) or not isinstance(require_all_domains, bool):
        raise ArgumentError("domain HTTP source matrix flags must be boolean")
    prepared: list[tuple[Mapping[str, Any], AutonomousDomainHttpSourcePreset]] = []
    source_ids: set[str] = set()
    domains: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, Mapping):
            raise ArgumentError(f"domain HTTP source matrix entry {index} is malformed")
        if "source_id" not in entry or "preset" not in entry or not callable(getattr(entry.get("acquirer"), "acquire", None)):
            raise ArgumentError(f"domain HTTP source matrix entry {index} requires source_id, preset, and acquirer")
        source_id = _identifier(f"domain HTTP source matrix entry {index}.source_id", entry["source_id"])
        if source_id in source_ids:
            raise ArgumentError("domain HTTP source matrix contains duplicate source IDs")
        source_ids.add(source_id)
        preset = _resolve_preset(catalogue, entry["preset"])
        domains.add(preset.domain)
        prepared.append((entry, preset))
    if require_all_domains and any(domain not in domains for domain in AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("domain HTTP source matrix must cover every autonomous domain")
    registrations = []
    for entry, preset in prepared:
        kwargs = dict(entry)
        kwargs.pop("preset", None)
        kwargs.pop("source_id", None)
        kwargs["catalogue"] = catalogue
        kwargs["preset"] = preset
        kwargs["source_id"] = entry["source_id"]
        kwargs["replace"] = replace
        kwargs["provider_contract_registry"] = provider_contract_registry
        registrations.append(register_autonomous_domain_http_source_preset(**kwargs))
    return {
        "schema": AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_SCHEMA, "preset_count": len(registrations),
        "registrations": registrations, "coverage": [row.to_dict() for row in catalogue.coverage()],
        "provider_contract_registry_digest": None if provider_contract_registry is None else getattr(provider_contract_registry, "registry_digest", None),
        "execution": _REGISTRATION_EXECUTION, "retention": _REGISTRATION_RETENTION, "secret_material": "never_returned",
    }


__all__ = [
    "AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_SCHEMA", "AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_REGISTRATION_SCHEMA",
    "AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_SCHEMA", "MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESETS",
    "MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_MATRIX_ENTRIES", "MAX_AUTONOMOUS_DOMAIN_HTTP_SOURCE_PRESET_BYTES",
    "AutonomousDomainHttpSourcePreset", "AutonomousDomainHttpSourceAcquirer",
    "builtin_autonomous_domain_http_source_presets", "create_autonomous_domain_http_source_acquirer",
    "register_autonomous_domain_http_source_preset", "register_autonomous_domain_http_source_matrix",
]
