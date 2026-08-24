"""Digest-bound, value-free normalizers for autonomous evidence routes.

Evidence acquisition and truth evaluation stay caller-owned.  This module supplies the missing
middle contract: a deterministic projection can make heterogeneous provider responses comparable
without retaining their values.  The built-in projection records only an operation, bounded shape
classification, byte count, and a digest of the transient observation.  It deliberately omits
source identity so equal observations from independent routes can reach reconciliation quorum.

Normalizers are registered by domain, identifier, and version.  The registry digest is part of the
catalogue identity and is checked again before execution, so changing a normalizer cannot silently
reinterpret an approved reconciliation plan.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA = "bioprism-python-autonomous-evidence-normalizer/0.1"
AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_SCHEMA = "bioprism-python-autonomous-evidence-normalizer-registry/0.1"
AUTONOMOUS_EVIDENCE_CLAIM_PROJECTION_SCHEMA = "bioprism-python-autonomous-evidence-claim-projection/0.1"
MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS = 256
MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_LIMITATIONS = 16
MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_TEXT_BYTES = 512
MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_VALUE_BYTES = 64_000_000
MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES = 64_000
MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_BYTES = 256_000

_RETENTION = "metadata_only;normalizer_callbacks_and_raw_values_caller_owned"
_SPEC_RETENTION = "metadata_only;normalizer_callback_not_serialized"
_SECRET_MARKERS = frozenset({
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
})
_OBSERVATION_KINDS = frozenset({"null", "scalar", "object", "array"})


def _text(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_TEXT_BYTES) -> str:
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


def _bounded_list(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    result = tuple(_text(f"{name}[{index}]", item, 2_048) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicates")
    return tuple(sorted(result))


def _secret_key(key: str) -> bool:
    normalized = "".join(character for character in key.lower() if character.isalnum())
    return normalized in _SECRET_MARKERS or normalized.startswith("gsk") or normalized.startswith("skproj") or any(
        marker in normalized for marker in ("token", "secret", "credential", "authorization")
    )


def _assert_safe_json(value: Any, name: str, depth: int = 0) -> None:
    if depth > 32:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ArgumentError(f"{name} contains a non-finite number")
        return
    if isinstance(value, Mapping):
        if len(value) > 16_384:
            raise ArgumentError(f"{name} contains too many fields")
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid field")
            if _secret_key(key):
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _assert_safe_json(child, f"{name}.{key}", depth + 1)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 16_384:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_safe_json(child, f"{name}[{index}]", depth + 1)
        return
    raise ArgumentError(f"{name} is not JSON-safe")


def _canonical_value(value: Any, name: str, maximum: int = MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_VALUE_BYTES) -> tuple[Any, int]:
    _assert_safe_json(value, name)
    try:
        encoded = canonical_json(value).encode("utf-8")
        cloned = json.loads(encoded.decode("utf-8"))
    except (TypeError, ValueError, UnicodeError) as error:
        raise ArgumentError(f"{name} is not canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its byte bound")
    return cloned, len(encoded)


def _observation_kind(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, Mapping):
        return "object"
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return "array"
    return "scalar"


def _shape_digest(value: Any, kind: str) -> str:
    if kind == "object":
        keys = sorted(str(key) for key in value)
        return content_digest({"kind": kind, "keys": keys})
    if kind == "array":
        item_kinds = tuple(_observation_kind(item) for item in value[:64])
        item_shapes = tuple(
            sorted(str(key) for key in item)
            for item in value[:64]
            if isinstance(item, Mapping)
        )
        return content_digest({"kind": kind, "item_kinds": list(item_kinds), "item_shapes": [list(shape) for shape in item_shapes]})
    return content_digest({"kind": kind})


def _operation(context: Mapping[str, Any]) -> str:
    request = context.get("request")
    metadata = request.get("metadata") if isinstance(request, Mapping) else None
    operation = metadata.get("operation") if isinstance(metadata, Mapping) else None
    if operation is None:
        return "unspecified"
    return _identifier("autonomous evidence normalizer operation", operation)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceNormalizerSpec:
    """Serializable identity and limitations for one normalizer callback."""

    domain: str
    normalizer_id: str
    version: str
    purpose: str
    limitations: tuple[str, ...]
    spec_digest: str = field(init=False)

    def __post_init__(self) -> None:
        domain = _identifier("evidence normalizer domain", self.domain)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("evidence normalizer domain is unsupported")
        normalizer_id = _identifier("evidence normalizer normalizer_id", self.normalizer_id)
        version = _identifier("evidence normalizer version", self.version)
        purpose = _text("evidence normalizer purpose", self.purpose, 2_048)
        limitations = _bounded_list("evidence normalizer limitations", self.limitations, MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_LIMITATIONS)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "normalizer_id", normalizer_id)
        object.__setattr__(self, "version", version)
        object.__setattr__(self, "purpose", purpose)
        object.__setattr__(self, "limitations", limitations)
        object.__setattr__(self, "spec_digest", content_digest(self._descriptor()))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA,
            "domain": self.domain,
            "normalizer_id": self.normalizer_id,
            "version": self.version,
            "purpose": self.purpose,
            "limitations": list(self.limitations),
            "execution": "normalizer_identity_only;callback_not_invoked",
            "retention": _SPEC_RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "spec_digest": self.spec_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceNormalizerSpec":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence normalizer spec must be a mapping")
        expected = {
            "schema", "domain", "normalizer_id", "version", "purpose", "limitations", "execution",
            "retention", "secret_material", "spec_digest",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA:
            raise ArgumentError("evidence normalizer spec contains unsupported fields")
        if value.get("execution") != "normalizer_identity_only;callback_not_invoked" or value.get("retention") != _SPEC_RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("evidence normalizer spec retention is invalid")
        limitations = value.get("limitations")
        if not isinstance(limitations, Sequence) or isinstance(limitations, (str, bytes, bytearray)):
            raise ArgumentError("evidence normalizer spec limitations are malformed")
        spec = cls(
            domain=value.get("domain"), normalizer_id=value.get("normalizer_id"), version=value.get("version"),
            purpose=value.get("purpose"), limitations=tuple(limitations),
        )
        if value.get("spec_digest") != spec.spec_digest or canonical_json(value) != canonical_json(spec.to_dict()):
            raise ArgumentError("evidence normalizer spec digest or canonical form is invalid")
        return spec


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceNormalizerRegistration:
    """Metadata-only registration paired with an in-process normalizer callback."""

    spec: AutonomousEvidenceNormalizerSpec
    normalizer: Callable[[Any, Mapping[str, Any]], Any] = field(compare=False, repr=False)

    def __post_init__(self) -> None:
        if not isinstance(self.spec, AutonomousEvidenceNormalizerSpec) or not callable(self.normalizer):
            raise ArgumentError("evidence normalizer registration is malformed")

    def to_dict(self) -> dict[str, Any]:
        return self.spec.to_dict()


class AutonomousEvidenceClaimProjector:
    """Built-in normalizer that compares observations by digest and bounded shape only."""

    def __init__(self, spec: AutonomousEvidenceNormalizerSpec) -> None:
        if not isinstance(spec, AutonomousEvidenceNormalizerSpec):
            raise ArgumentError("claim projector requires a typed normalizer spec")
        self.spec = spec

    def __call__(self, value: Any, context: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(context, Mapping):
            raise ArgumentError("evidence claim projection context must be a mapping")
        canonical, value_bytes = _canonical_value(value, "evidence claim projection value")
        kind = _observation_kind(canonical)
        item_count = len(canonical) if isinstance(canonical, (Mapping, list)) else 1
        if item_count > 16_384:
            raise ArgumentError("evidence claim projection item count exceeds its bound")
        descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_CLAIM_PROJECTION_SCHEMA,
            "domain": self.spec.domain,
            "normalizer_id": self.spec.normalizer_id,
            "normalizer_version": self.spec.version,
            "operation": _operation(context),
            "observation_kind": kind,
            "item_count": item_count,
            "value_bytes": value_bytes,
            "value_digest": content_digest(canonical),
            "shape_digest": _shape_digest(canonical, kind),
            "claim_posture": "projection_only;truth_and_evaluation_caller_owned",
            "limitations": list(self.spec.limitations),
        }
        descriptor["claim_digest"] = content_digest(descriptor)
        if len(canonical_json(descriptor).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES:
            raise ArgumentError("evidence claim projection exceeds its byte bound")
        return descriptor


def _identity(value: Any, _context: Mapping[str, Any]) -> Any:
    canonical, _ = _canonical_value(value, "evidence identity normalizer value")
    return canonical


class AutonomousEvidenceNormalizerRegistry:
    """Digest-addressed registry for caller-owned and built-in normalizer callbacks."""

    def __init__(self, registrations: Sequence[AutonomousEvidenceNormalizerRegistration] = ()) -> None:
        if isinstance(registrations, (str, bytes, bytearray)) or not isinstance(registrations, Sequence):
            raise ArgumentError("evidence normalizer registrations must be a sequence")
        if len(registrations) > MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS:
            raise ArgumentError("evidence normalizer registry is over capacity")
        self._entries: dict[tuple[str, str, str], AutonomousEvidenceNormalizerRegistration] = {}
        for registration in registrations:
            self.register(registration)

    def register(self, registration: AutonomousEvidenceNormalizerRegistration, *, replace: bool = False) -> dict[str, Any]:
        if not isinstance(registration, AutonomousEvidenceNormalizerRegistration) or not isinstance(replace, bool):
            raise ArgumentError("evidence normalizer registration is malformed")
        spec = registration.spec
        key = (spec.domain, spec.normalizer_id, spec.version)
        if key in self._entries and not replace:
            raise ArgumentError(f"evidence normalizer is already registered: {'/'.join(key)}")
        existing = self._entries.get(key)
        if existing is not None and existing.spec.spec_digest == spec.spec_digest and existing.normalizer is not registration.normalizer:
            raise ArgumentError("evidence normalizer callback changed without a new versioned spec")
        if key not in self._entries and len(self._entries) >= MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS:
            raise ArgumentError("evidence normalizer registry is full")
        self._entries[key] = registration
        try:
            self.to_dict()
        except Exception:
            if existing is None:
                self._entries.pop(key, None)
            else:
                self._entries[key] = existing
            raise
        return registration.to_dict()

    def unregister(self, domain: str, normalizer_id: str, version: str) -> bool:
        key = (
            _identifier("evidence normalizer domain", domain),
            _identifier("evidence normalizer normalizer_id", normalizer_id),
            _identifier("evidence normalizer version", version),
        )
        return self._entries.pop(key, None) is not None

    def registrations(self) -> tuple[AutonomousEvidenceNormalizerRegistration, ...]:
        return tuple(self._entries[key] for key in sorted(self._entries))

    def resolve(self, domain: str, normalizer_id: str, version: str) -> AutonomousEvidenceNormalizerRegistration:
        key = (
            _identifier("evidence normalizer domain", domain),
            _identifier("evidence normalizer normalizer_id", normalizer_id),
            _identifier("evidence normalizer version", version),
        )
        registration = self._entries.get(key)
        if registration is None:
            raise ArgumentError(f"evidence normalizer is not registered: {'/'.join(key)}")
        return registration

    def normalize(self, domain: str, normalizer_id: str, version: str, value: Any, context: Mapping[str, Any]) -> Any:
        registration = self.resolve(domain, normalizer_id, version)
        result = registration.normalizer(value, context)
        normalized, _ = _canonical_value(result, "evidence normalizer output", MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES)
        return normalized

    @property
    def registry_digest(self) -> str:
        return content_digest(self._descriptor())

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_SCHEMA,
            "normalizers": [registration.to_dict() for registration in self.registrations()],
            "execution": "registry_projection_only;callbacks_not_invoked",
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        descriptor = self._descriptor()
        if len(canonical_json(descriptor).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_BYTES:
            raise ArgumentError("evidence normalizer registry exceeds its byte bound")
        return {**descriptor, "registry_digest": self.registry_digest}


_BUILTIN_NORMALIZER_IDS = {
    "coding": "builtin.coding.claim-projection",
    "browser": "builtin.browser.claim-projection",
    "data": "builtin.data.claim-projection",
    "science": "builtin.science.claim-projection",
    "biomedical": "builtin.biomedical.claim-projection",
    "neuroscience": "builtin.neuroscience.claim-projection",
    "operations": "builtin.operations.claim-projection",
    "enterprise": "builtin.enterprise.claim-projection",
    "multi_agent": "builtin.multi-agent.claim-projection",
    "multimodal": "builtin.multimodal.claim-projection",
    "cross_domain": "builtin.cross-domain.claim-projection",
    "evaluation": "builtin.evaluation.claim-projection",
}


def create_builtin_autonomous_evidence_normalizer_registry() -> AutonomousEvidenceNormalizerRegistry:
    registrations: list[AutonomousEvidenceNormalizerRegistration] = []
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        identity_spec = AutonomousEvidenceNormalizerSpec(
            domain=domain, normalizer_id="identity", version="1",
            purpose="Preserve an exact caller-selected JSON observation for transient reconciliation.",
            limitations=("exact value equality is required", "the caller owns truth and evaluation"),
        )
        registrations.append(AutonomousEvidenceNormalizerRegistration(identity_spec, _identity))
        projection_spec = AutonomousEvidenceNormalizerSpec(
            domain=domain, normalizer_id=_BUILTIN_NORMALIZER_IDS[domain], version="1",
            purpose=f"Project bounded {domain} evidence into digest and response-shape metadata.",
            limitations=("raw source values are not returned by the projection", "shape and digest are not truth or evaluator verdicts"),
        )
        registrations.append(AutonomousEvidenceNormalizerRegistration(projection_spec, AutonomousEvidenceClaimProjector(projection_spec)))
    return AutonomousEvidenceNormalizerRegistry(registrations)


def builtin_autonomous_evidence_normalizer_specs() -> tuple[AutonomousEvidenceNormalizerSpec, ...]:
    return tuple(registration.spec for registration in create_builtin_autonomous_evidence_normalizer_registry().registrations())


__all__ = [
    "AUTONOMOUS_EVIDENCE_NORMALIZER_SCHEMA", "AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_SCHEMA",
    "AUTONOMOUS_EVIDENCE_CLAIM_PROJECTION_SCHEMA", "MAX_AUTONOMOUS_EVIDENCE_NORMALIZERS",
    "MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_LIMITATIONS", "MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_VALUE_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_OUTPUT_BYTES", "MAX_AUTONOMOUS_EVIDENCE_NORMALIZER_REGISTRY_BYTES",
    "AutonomousEvidenceNormalizerSpec", "AutonomousEvidenceNormalizerRegistration",
    "AutonomousEvidenceClaimProjector", "AutonomousEvidenceNormalizerRegistry",
    "create_builtin_autonomous_evidence_normalizer_registry", "builtin_autonomous_evidence_normalizer_specs",
]
