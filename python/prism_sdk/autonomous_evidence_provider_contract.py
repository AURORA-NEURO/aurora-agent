"""Digest-bound provider/source contracts for autonomous evidence adapters.

This module does not implement provider clients.  It makes the assumptions around a caller-owned
adapter explicit and executable: protocol, operations, domains, capabilities, source kinds, auth
posture, freshness, pagination, and required request metadata are all bound to the exact adapter
manifest selected for a run.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapter_orchestration import (
    AutonomousLLMEvidenceAdapterManifest,
    AutonomousLLMEvidenceAdapterRegistry,
    _digest,
    _domains,
    _identifier,
    _json_bytes,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA = "bioprism-python-autonomous-evidence-provider-contract/0.1"
AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_REGISTRY_SCHEMA = "bioprism-python-autonomous-evidence-provider-contract-registry/0.1"
MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACTS = 256
MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_OPERATIONS = 32
MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS = 32
MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_BYTES = 512_000

AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS = frozenset({
    "http_json", "graphql", "openai_responses", "openai_chat_completions",
    "anthropic_messages", "caller_defined",
})
AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES = frozenset({
    "none", "caller_managed_credential", "caller_signed_request", "delegated_session",
})
AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES = frozenset({
    "realtime", "bounded_cache", "historical", "caller_declared",
})
AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES = frozenset({
    "none", "cursor", "page_number", "link_header", "caller_defined",
})

_RETENTION = "manifest_and_contract_metadata_only;credentials_and_raw_source_values_caller_owned"
_SECRET_MARKERS = frozenset({
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "token", "privatekey", "refreshtoken",
})


def _bounded_list(name: str, value: Any, maximum: int, *, sort: bool = True) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    result = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate entries")
    return tuple(sorted(result)) if sort else result


def _metadata_key(name: str, value: Any) -> str:
    key = _identifier(name, value)
    normalized = "".join(character for character in key.lower() if character.isalnum())
    if normalized in _SECRET_MARKERS or any(marker in normalized for marker in ("token", "secret", "credential", "authorization")):
        raise ArgumentError(f"{name} cannot be credential-shaped")
    return key


def _metadata_keys(name: str, value: Any) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS:
        raise ArgumentError(f"{name} must contain at most {MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS} entries")
    result = tuple(_metadata_key(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate entries")
    return tuple(sorted(result))


def _subset(name: str, values: Sequence[str], allowed: Sequence[str]) -> None:
    missing = sorted(set(values).difference(allowed))
    if missing:
        raise ArgumentError(f"{name} exceeds the bound adapter contract: {', '.join(missing)}")


def _manifest_for(registry: AutonomousLLMEvidenceAdapterRegistry, adapter_id: str) -> AutonomousLLMEvidenceAdapterManifest:
    matches = tuple(manifest for manifest in registry.manifests() if manifest.adapter_id == adapter_id)
    if len(matches) != 1:
        raise ArgumentError(f"provider evidence contract references unknown or ambiguous adapter: {adapter_id}")
    return matches[0]


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceProviderContract:
    contract_id: str
    version: str
    provider: str
    protocol: str
    operations: tuple[str, ...]
    domains: tuple[str, ...]
    capabilities: tuple[str, ...]
    source_kinds: tuple[str, ...]
    auth_mode: str
    freshness: str
    pagination: str
    required_metadata: tuple[str, ...]
    operation_metadata_key: str | None
    adapter_id: str
    adapter_manifest_digest: str
    adapter_registry_digest: str
    contract_digest: str

    @classmethod
    def bind(
        cls,
        registry: AutonomousLLMEvidenceAdapterRegistry,
        *,
        contract_id: str,
        version: str,
        provider: str,
        protocol: str,
        operations: Sequence[str],
        domains: Sequence[str],
        capabilities: Sequence[str],
        source_kinds: Sequence[str],
        auth_mode: str,
        freshness: str,
        pagination: str,
        adapter_id: str,
        required_metadata: Sequence[str] = (),
        operation_metadata_key: str | None = None,
    ) -> "AutonomousEvidenceProviderContract":
        if not isinstance(registry, AutonomousLLMEvidenceAdapterRegistry):
            raise ArgumentError("provider evidence contract requires a typed adapter registry")
        normalized_adapter_id = _identifier("provider evidence contract adapter_id", adapter_id)
        manifest = _manifest_for(registry, normalized_adapter_id)
        normalized_domains = _domains(domains)
        normalized_operations = _bounded_list("provider evidence contract operations", operations, MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_OPERATIONS)
        normalized_capabilities = _bounded_list("provider evidence contract capabilities", capabilities, 64)
        normalized_source_kinds = _bounded_list("provider evidence contract source_kinds", source_kinds, 32)
        normalized_metadata = _metadata_keys("provider evidence contract required_metadata", required_metadata)
        normalized_operation_key = None if operation_metadata_key is None else _metadata_key("provider evidence contract operation_metadata_key", operation_metadata_key)
        if normalized_operation_key is not None and normalized_operation_key not in normalized_metadata:
            raise ArgumentError("provider evidence contract operation_metadata_key must be required metadata")
        if provider != manifest.provider:
            raise ArgumentError("provider evidence contract provider does not match the adapter manifest")
        _subset("provider evidence contract domains", normalized_domains, (manifest.domain,))
        _subset("provider evidence contract capabilities", normalized_capabilities, manifest.capabilities)
        _subset("provider evidence contract source_kinds", normalized_source_kinds, manifest.source_kinds)
        if protocol not in AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS:
            raise ArgumentError("provider evidence contract protocol is invalid")
        if auth_mode not in AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES:
            raise ArgumentError("provider evidence contract auth_mode is invalid")
        if freshness not in AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES:
            raise ArgumentError("provider evidence contract freshness is invalid")
        if pagination not in AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES:
            raise ArgumentError("provider evidence contract pagination is invalid")
        descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA,
            "contract_id": _identifier("provider evidence contract contract_id", contract_id),
            "version": _identifier("provider evidence contract version", version),
            "provider": manifest.provider,
            "protocol": protocol,
            "operations": list(normalized_operations),
            "domains": list(normalized_domains),
            "capabilities": list(normalized_capabilities),
            "source_kinds": list(normalized_source_kinds),
            "auth_mode": auth_mode,
            "freshness": freshness,
            "pagination": pagination,
            "required_metadata": list(normalized_metadata),
            "operation_metadata_key": normalized_operation_key,
            "adapter_id": manifest.adapter_id,
            "adapter_manifest_digest": manifest.manifest_digest,
            "adapter_registry_digest": registry.registry_digest,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }
        return cls(
            contract_id=descriptor["contract_id"],
            version=descriptor["version"],
            provider=descriptor["provider"],
            protocol=descriptor["protocol"],
            operations=normalized_operations,
            domains=normalized_domains,
            capabilities=normalized_capabilities,
            source_kinds=normalized_source_kinds,
            auth_mode=descriptor["auth_mode"],
            freshness=descriptor["freshness"],
            pagination=descriptor["pagination"],
            required_metadata=normalized_metadata,
            operation_metadata_key=normalized_operation_key,
            adapter_id=manifest.adapter_id,
            adapter_manifest_digest=manifest.manifest_digest,
            adapter_registry_digest=registry.registry_digest,
            contract_digest=content_digest(descriptor),
        )

    def __post_init__(self) -> None:
        _identifier("provider evidence contract contract_id", self.contract_id)
        _identifier("provider evidence contract version", self.version)
        _identifier("provider evidence contract provider", self.provider)
        if self.protocol not in AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS or self.auth_mode not in AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES or self.freshness not in AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES or self.pagination not in AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES:
            raise ArgumentError("provider evidence contract protocol, auth, freshness, or pagination is invalid")
        _bounded_list("provider evidence contract operations", self.operations, MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_OPERATIONS)
        _domains(self.domains)
        _bounded_list("provider evidence contract capabilities", self.capabilities, 64)
        _bounded_list("provider evidence contract source_kinds", self.source_kinds, 32)
        _metadata_keys("provider evidence contract required_metadata", self.required_metadata)
        if self.operation_metadata_key is not None:
            key = _metadata_key("provider evidence contract operation_metadata_key", self.operation_metadata_key)
            if key not in self.required_metadata:
                raise ArgumentError("provider evidence contract operation_metadata_key must be required metadata")
        _identifier("provider evidence contract adapter_id", self.adapter_id)
        _digest("provider evidence contract adapter_manifest_digest", self.adapter_manifest_digest)
        _digest("provider evidence contract adapter_registry_digest", self.adapter_registry_digest)
        _digest("provider evidence contract contract_digest", self.contract_digest)
        if content_digest(self._descriptor()) != self.contract_digest:
            raise ArgumentError("provider evidence contract digest is invalid")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA,
            "contract_id": self.contract_id,
            "version": self.version,
            "provider": self.provider,
            "protocol": self.protocol,
            "operations": list(self.operations),
            "domains": list(self.domains),
            "capabilities": list(self.capabilities),
            "source_kinds": list(self.source_kinds),
            "auth_mode": self.auth_mode,
            "freshness": self.freshness,
            "pagination": self.pagination,
            "required_metadata": list(self.required_metadata),
            "operation_metadata_key": self.operation_metadata_key,
            "adapter_id": self.adapter_id,
            "adapter_manifest_digest": self.adapter_manifest_digest,
            "adapter_registry_digest": self.adapter_registry_digest,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "contract_digest": self.contract_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceProviderContract":
        if not isinstance(value, Mapping):
            raise ArgumentError("provider evidence contract must be a mapping")
        allowed = {
            "schema", "contract_id", "version", "provider", "protocol", "operations", "domains",
            "capabilities", "source_kinds", "auth_mode", "freshness", "pagination", "required_metadata",
            "operation_metadata_key", "adapter_id", "adapter_manifest_digest", "adapter_registry_digest",
            "contract_digest", "retention", "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("provider evidence contract contains unsupported or transient fields")
        contract = cls(
            contract_id=value.get("contract_id"), version=value.get("version"), provider=value.get("provider"),
            protocol=value.get("protocol"), operations=tuple(value.get("operations", ())), domains=tuple(value.get("domains", ())),
            capabilities=tuple(value.get("capabilities", ())), source_kinds=tuple(value.get("source_kinds", ())),
            auth_mode=value.get("auth_mode"), freshness=value.get("freshness"), pagination=value.get("pagination"),
            required_metadata=tuple(value.get("required_metadata", ())), operation_metadata_key=value.get("operation_metadata_key"),
            adapter_id=value.get("adapter_id"), adapter_manifest_digest=value.get("adapter_manifest_digest"),
            adapter_registry_digest=value.get("adapter_registry_digest"), contract_digest=value.get("contract_digest"),
        )
        if canonical_json(value) != canonical_json(contract.to_dict()):
            raise ArgumentError("provider evidence contract is not canonical")
        return contract


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceProviderContractCoverage:
    domain: str
    contract_ids: tuple[str, ...]
    providers: tuple[str, ...]
    protocols: tuple[str, ...]
    state: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "domain": self.domain,
            "contract_ids": list(self.contract_ids),
            "providers": list(self.providers),
            "protocols": list(self.protocols),
            "state": self.state,
        }


class AutonomousEvidenceProviderContractRegistry:
    """Process-local contract registry bound to one adapter registry snapshot."""

    def __init__(self, adapter_registry: AutonomousLLMEvidenceAdapterRegistry) -> None:
        if not isinstance(adapter_registry, AutonomousLLMEvidenceAdapterRegistry):
            raise ArgumentError("provider evidence contract registry requires a typed adapter registry")
        self.adapter_registry = adapter_registry
        self._entries: dict[str, AutonomousEvidenceProviderContract] = {}

    def register(self, contract: AutonomousEvidenceProviderContract, *, replace: bool = False) -> dict[str, Any]:
        if not isinstance(contract, AutonomousEvidenceProviderContract):
            raise ArgumentError("provider evidence contract registry accepts a typed contract")
        if not isinstance(replace, bool):
            raise ArgumentError("provider evidence contract registry replace must be boolean")
        if contract.adapter_registry_digest != self.adapter_registry.registry_digest:
            raise ArgumentError("provider evidence contract adapter registry is stale")
        existing = self._entries.get(contract.contract_id)
        if existing is not None and not replace:
            raise ArgumentError(f"provider evidence contract is already registered: {contract.contract_id}")
        if existing is None and len(self._entries) >= MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACTS:
            raise ArgumentError("provider evidence contract registry is full")
        conflicting = next(
            (
                candidate for candidate in self._entries.values()
                if candidate.contract_id != contract.contract_id
                and candidate.adapter_id == contract.adapter_id
                and set(candidate.domains).intersection(contract.domains)
            ),
            None,
        )
        if conflicting is not None:
            raise ArgumentError(f"provider evidence contract overlaps adapter/domain binding: {conflicting.contract_id}")
        self._entries[contract.contract_id] = contract
        self.to_dict()
        return contract.to_dict()

    def register_for_adapter(self, **kwargs: Any) -> dict[str, Any]:
        contract = AutonomousEvidenceProviderContract.bind(self.adapter_registry, **kwargs)
        return self.register(contract)

    def unregister(self, contract_id: str) -> bool:
        return self._entries.pop(_identifier("provider evidence contract contract_id", contract_id), None) is not None

    def contracts(self) -> tuple[AutonomousEvidenceProviderContract, ...]:
        return tuple(self._entries[key] for key in sorted(self._entries))

    def resolve(self, contract_id: str) -> AutonomousEvidenceProviderContract:
        normalized = _identifier("provider evidence contract contract_id", contract_id)
        contract = self._entries.get(normalized)
        if contract is None:
            raise ArgumentError(f"provider evidence contract is not registered: {normalized}")
        return contract

    def coverage(self) -> tuple[AutonomousEvidenceProviderContractCoverage, ...]:
        result = []
        for domain in AUTONOMOUS_DOMAIN_NAMES:
            matches = tuple(contract for contract in self._entries.values() if domain in contract.domains)
            result.append(
                AutonomousEvidenceProviderContractCoverage(
                    domain=domain,
                    contract_ids=tuple(sorted(contract.contract_id for contract in matches)),
                    providers=tuple(sorted({contract.provider for contract in matches})),
                    protocols=tuple(sorted({contract.protocol for contract in matches})),
                    state="complete" if matches else "missing",
                )
            )
        return tuple(result)

    def verify(self) -> "AutonomousEvidenceProviderContractRegistry":
        current = self.adapter_registry.registry_digest
        for contract in self._entries.values():
            manifest = _manifest_for(self.adapter_registry, contract.adapter_id)
            if manifest.manifest_digest != contract.adapter_manifest_digest:
                if contract.adapter_registry_digest != current:
                    raise ArgumentError("provider evidence contract adapter registry is stale or tampered")
                raise ArgumentError(f"provider evidence contract adapter binding changed: {contract.contract_id}")
            if contract.provider != manifest.provider:
                raise ArgumentError(f"provider evidence contract provider binding changed: {contract.contract_id}")
            _subset("provider evidence contract domains", contract.domains, (manifest.domain,))
            _subset("provider evidence contract capabilities", contract.capabilities, manifest.capabilities)
            _subset("provider evidence contract source_kinds", contract.source_kinds, manifest.source_kinds)
        return self

    def contract_for_adapter(self, adapter_id: str, domain: str) -> AutonomousEvidenceProviderContract:
        normalized_id = _identifier("provider evidence contract adapter_id", adapter_id)
        normalized_domains = _domains((domain,))
        self.verify()
        matches = tuple(contract for contract in self._entries.values() if contract.adapter_id == normalized_id and normalized_domains[0] in contract.domains)
        if len(matches) != 1:
            raise ArgumentError(
                f"{'no' if not matches else 'ambiguous'} provider evidence contract binding for {normalized_id}/{domain}"
            )
        return matches[0]

    def create_acquirer_for_adapter(self, adapter_id: str, domain: str) -> Any:
        normalized_id = _identifier("provider evidence contract adapter_id", adapter_id)
        contract = self.contract_for_adapter(normalized_id, domain)
        adapter = self.adapter_registry.resolve(domain, normalized_id)
        registry = self

        class ContractAcquirer:
            def acquire(self, context: Mapping[str, Any]) -> Any:
                registry.verify()
                live = registry.contract_for_adapter(normalized_id, domain)
                if live.contract_digest != contract.contract_digest:
                    raise ArgumentError("provider evidence contract changed after acquirer creation")
                requirement = context.get("requirement")
                requirement_domain = getattr(requirement, "domain", requirement.get("domain") if isinstance(requirement, Mapping) else None)
                if requirement_domain != domain:
                    raise ArgumentError("provider evidence contract acquirer received a different domain")
                required_capabilities = getattr(requirement, "required_capabilities", requirement.get("required_capabilities", ()) if isinstance(requirement, Mapping) else ())
                _subset("provider evidence request required capabilities", tuple(required_capabilities), live.capabilities)
                request = context.get("request")
                metadata = request.get("metadata", {}) if isinstance(request, Mapping) else {}
                if not isinstance(metadata, Mapping):
                    raise ArgumentError("provider evidence request metadata must be a mapping")
                for key in live.required_metadata:
                    if metadata.get(key) is None:
                        raise ArgumentError(f"provider evidence request is missing required metadata: {key}")
                if live.operation_metadata_key is not None:
                    operation = metadata.get(live.operation_metadata_key)
                    if not isinstance(operation, str) or operation not in live.operations:
                        raise ArgumentError(f"provider evidence request operation is not declared by {live.contract_id}")
                return adapter.acquire(context)

        return ContractAcquirer()

    @property
    def registry_digest(self) -> str:
        return content_digest(self._descriptor())

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_REGISTRY_SCHEMA,
            "adapter_registry_digest": self.adapter_registry.registry_digest,
            "contracts": [contract.to_dict() for contract in self.contracts()],
            "coverage": [row.to_dict() for row in self.coverage()],
            "execution": "registry_projection_only;contract_validation_no_source_dispatch",
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        descriptor = self._descriptor()
        _json_bytes(descriptor, "provider evidence contract registry", MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_BYTES)
        return {**descriptor, "registry_digest": content_digest(descriptor)}


def create_autonomous_evidence_provider_contract_registry(
    adapter_registry: AutonomousLLMEvidenceAdapterRegistry,
) -> AutonomousEvidenceProviderContractRegistry:
    return AutonomousEvidenceProviderContractRegistry(adapter_registry)


__all__ = [
    "AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_REGISTRY_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACTS",
    "MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_OPERATIONS",
    "MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_METADATA_KEYS",
    "MAX_AUTONOMOUS_EVIDENCE_PROVIDER_CONTRACT_BYTES",
    "AUTONOMOUS_EVIDENCE_PROVIDER_PROTOCOLS",
    "AUTONOMOUS_EVIDENCE_PROVIDER_AUTH_MODES",
    "AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES",
    "AUTONOMOUS_EVIDENCE_PROVIDER_PAGINATION_MODES",
    "AutonomousEvidenceProviderContract",
    "AutonomousEvidenceProviderContractCoverage",
    "AutonomousEvidenceProviderContractRegistry",
    "create_autonomous_evidence_provider_contract_registry",
]
