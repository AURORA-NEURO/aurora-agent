"""Freshness, authority, and metadata-only provenance admission for evidence sources."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_provider_contract import (
    AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES,
    AutonomousEvidenceProviderContract,
    AutonomousEvidenceProviderContractRegistry,
)
from .autonomous_evidence_runtime import AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .autonomous_evidence_adapter_orchestration import (
    _digest,
    _finite,
    _identifier,
    _integer,
    _json_bytes,
    _optional_digest,
)


AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA = "bioprism-python-autonomous-evidence-source/0.1"
AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA = "bioprism-python-autonomous-evidence-source-ledger-entry/0.1"
AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA = "bioprism-python-autonomous-evidence-source-ledger/0.1"
AUTONOMOUS_EVIDENCE_SOURCE_POLICY_SCHEMA = "bioprism-python-autonomous-evidence-source-policy/0.1"
MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES = 512
MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS = 32
MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS = 4_096
MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES = 64_000_000
MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES = 1_000_000
MAX_AUTONOMOUS_EVIDENCE_SOURCE_AGE_MS = 31_536_000_000
MAX_AUTONOMOUS_EVIDENCE_SOURCE_FUTURE_SKEW_MS = 86_400_000
DEFAULT_AUTONOMOUS_REALTIME_SOURCE_AGE_MS = 300_000

AUTONOMOUS_EVIDENCE_SOURCE_AUTHORITIES = frozenset({"caller_declared", "provider_observed", "human_verified", "derived"})
AUTONOMOUS_EVIDENCE_SOURCE_STATUSES = frozenset({"observed", "partial", "unavailable", "refused", "stale"})
AUTONOMOUS_EVIDENCE_SOURCE_DECISIONS = frozenset({"accepted", "partial", "stale", "unverified", "refused"})
_RETENTION = "metadata_only;raw_source_values_and_locators_caller_owned"
_SECRET_MARKERS = frozenset({
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
})


def _bounded_text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _source_identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _bounded_text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return result


def _bounded_list(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} entries")
    result = tuple(_bounded_text(f"{name}[{index}]", item, 512) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate entries")
    return tuple(sorted(result))


def _safe_field_marker(value: str) -> str:
    return "".join(character for character in value.lower() if character.isalnum())


def _assert_safe_source_value(value: Any, name: str, depth: int = 0) -> None:
    if depth > 32:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if value != value or value in {float("inf"), float("-inf")}:
            raise ArgumentError(f"{name} contains a non-finite number")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid object field")
            normalized = _safe_field_marker(key)
            if normalized in _SECRET_MARKERS or any(marker in normalized for marker in ("token", "secret", "credential", "authorization")):
                raise ArgumentError(f"{name}.{key} is credential-shaped source metadata")
            _assert_safe_source_value(child, f"{name}.{key}", depth + 1)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 16_384:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_safe_source_value(child, f"{name}[{index}]", depth + 1)
        return
    raise ArgumentError(f"{name} is not JSON-safe")


def _source_value_bytes(value: Any) -> int:
    _assert_safe_source_value(value, "source value")
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError("source value is not canonical JSON") from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES:
        raise ArgumentError("source value exceeds its bounded byte limit")
    return len(encoded)


def _source_digest(name: str, value: Any, *, required: bool = False) -> str | None:
    if value is None:
        if required:
            raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
        return None
    return _digest(name, value)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceSourceDescriptor:
    source_id: str
    source_digest: str | None
    authority: str
    status: str
    observed_at_ms: int
    expires_at_ms: int | None
    citation_digest: str | None
    limitations: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _source_identifier("source descriptor source_id", self.source_id, MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES)
        _source_digest("source descriptor source_digest", self.source_digest)
        if self.authority not in AUTONOMOUS_EVIDENCE_SOURCE_AUTHORITIES or self.status not in AUTONOMOUS_EVIDENCE_SOURCE_STATUSES:
            raise ArgumentError("source descriptor authority or status is invalid")
        _integer("source descriptor observed_at_ms", self.observed_at_ms, 0, 9_000_000_000_000_000)
        if self.expires_at_ms is not None:
            _integer("source descriptor expires_at_ms", self.expires_at_ms, 0, 9_000_000_000_000_000)
            if self.expires_at_ms < self.observed_at_ms:
                raise ArgumentError("source descriptor expiry precedes observation")
        _source_digest("source descriptor citation_digest", self.citation_digest)
        _bounded_list("source descriptor limitations", self.limitations, MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS)

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "authority": self.authority,
            "status": self.status,
            "observed_at_ms": self.observed_at_ms,
            "expires_at_ms": self.expires_at_ms,
            "citation_digest": self.citation_digest,
            "limitations": list(self.limitations),
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceSourcePolicyDecision:
    decision: str
    usable: bool
    reasons: tuple[str, ...]

    def __post_init__(self) -> None:
        if self.decision not in AUTONOMOUS_EVIDENCE_SOURCE_DECISIONS or not isinstance(self.usable, bool):
            raise ArgumentError("source policy decision is invalid")
        _bounded_list("source policy decision reasons", self.reasons, 32)

    def to_dict(self) -> dict[str, Any]:
        return {"decision": self.decision, "usable": self.usable, "reasons": list(self.reasons)}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceSourcePolicy:
    max_age_ms: int | None = None
    max_future_skew_ms: int = 60_000
    allow_partial: bool = False
    allow_unverified: bool = False
    require_source_digest: bool = True
    clock: Callable[[], float] = field(default=time.time, repr=False, compare=False)

    def __post_init__(self) -> None:
        if self.max_age_ms is not None:
            _integer("source policy max_age_ms", self.max_age_ms, 0, MAX_AUTONOMOUS_EVIDENCE_SOURCE_AGE_MS)
        _integer("source policy max_future_skew_ms", self.max_future_skew_ms, 0, MAX_AUTONOMOUS_EVIDENCE_SOURCE_FUTURE_SKEW_MS)
        for name, value in (("allow_partial", self.allow_partial), ("allow_unverified", self.allow_unverified), ("require_source_digest", self.require_source_digest)):
            if not isinstance(value, bool):
                raise ArgumentError(f"source policy {name} must be boolean")
        if not callable(self.clock):
            raise ArgumentError("source policy clock must be callable")

    def now(self) -> int:
        return _integer("source policy clock value", int(self.clock()), 0, 9_000_000_000_000_000)

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_POLICY_SCHEMA,
            "max_age_ms": self.max_age_ms,
            "max_future_skew_ms": self.max_future_skew_ms,
            "allow_partial": self.allow_partial,
            "allow_unverified": self.allow_unverified,
            "require_source_digest": self.require_source_digest,
            "execution": "freshness_and_authority_gate;no_source_dispatch",
            "retention": "metadata_only_policy",
            "secret_material": "never_returned",
        }

    @property
    def policy_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "policy_digest": self.policy_digest}

    def evaluate(
        self,
        contract: AutonomousEvidenceProviderContract,
        descriptor: AutonomousEvidenceSourceDescriptor,
        *,
        now_ms: int | None = None,
    ) -> AutonomousEvidenceSourcePolicyDecision:
        if not isinstance(contract, AutonomousEvidenceProviderContract) or not isinstance(descriptor, AutonomousEvidenceSourceDescriptor):
            raise ArgumentError("source policy requires typed contract and descriptor")
        timestamp = self.now() if now_ms is None else _integer("source policy evaluation now_ms", now_ms, 0, 9_000_000_000_000_000)
        reasons: list[str] = []
        decision = "accepted"
        priority = {"accepted": 0, "partial": 1, "unverified": 2, "stale": 3, "refused": 4}

        def apply(candidate: str) -> None:
            nonlocal decision
            if priority[candidate] > priority[decision]:
                decision = candidate

        if descriptor.observed_at_ms > timestamp + self.max_future_skew_ms:
            return AutonomousEvidenceSourcePolicyDecision("refused", False, ("observed_at_is_in_the_future",))
        if descriptor.status in {"unavailable", "refused"}:
            return AutonomousEvidenceSourcePolicyDecision("refused", False, (f"source_status_{descriptor.status}",))
        if descriptor.status == "stale":
            apply("stale")
        if descriptor.status == "partial":
            reasons.append("source_status_partial")
            apply("partial" if self.allow_partial else "refused")
        if self.require_source_digest and descriptor.source_digest is None:
            reasons.append("source_digest_missing")
            apply("unverified")
        if descriptor.authority == "caller_declared":
            reasons.append("authority_caller_declared")
            apply("unverified")
        age_limit = self.max_age_ms
        if age_limit is None and contract.freshness == "realtime":
            age_limit = DEFAULT_AUTONOMOUS_REALTIME_SOURCE_AGE_MS
        if age_limit is not None and timestamp >= descriptor.observed_at_ms and timestamp - descriptor.observed_at_ms > age_limit:
            reasons.append("source_observation_exceeds_max_age")
            apply("stale")
        if contract.freshness == "bounded_cache" and (descriptor.expires_at_ms is None or timestamp > descriptor.expires_at_ms):
            reasons.append("bounded_cache_expiry_missing" if descriptor.expires_at_ms is None else "bounded_cache_expired")
            apply("stale")
        if contract.freshness == "caller_declared" and descriptor.authority != "caller_declared":
            reasons.append("caller_declared_contract_requires_explicit_authority")
            apply("unverified")
        unique_reasons = tuple(sorted(set(reasons)))
        if decision == "unverified" and self.allow_unverified:
            return AutonomousEvidenceSourcePolicyDecision(decision, True, unique_reasons)
        usable = decision == "accepted" or decision == "partial" and self.allow_partial
        return AutonomousEvidenceSourcePolicyDecision(decision, usable, unique_reasons)


def normalize_autonomous_evidence_source_descriptor(
    value: Mapping[str, Any],
    *,
    default_source_id: str | None = None,
) -> AutonomousEvidenceSourceDescriptor:
    if not isinstance(value, Mapping):
        raise ArgumentError("source descriptor must be a mapping")
    source_id = value.get("source_id", value.get("sourceId", default_source_id))
    observed = value.get("observed_at_ms", value.get("observedAtMs"))
    expires = value.get("expires_at_ms", value.get("expiresAtMs"))
    source_digest = value.get("source_digest", value.get("sourceDigest"))
    citation = value.get("citation_digest", value.get("citationDigest"))
    limitations = value.get("limitations", ())
    if isinstance(limitations, (str, bytes, bytearray)) or not isinstance(limitations, Sequence):
        raise ArgumentError("source descriptor limitations must be a sequence")
    return AutonomousEvidenceSourceDescriptor(
        source_id=_source_identifier("source descriptor source_id", source_id, MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES),
        source_digest=_source_digest("source descriptor source_digest", source_digest),
        authority=value.get("authority"),
        status=value.get("status"),
        observed_at_ms=observed,
        expires_at_ms=expires,
        citation_digest=_source_digest("source descriptor citation_digest", citation),
        limitations=tuple(_bounded_text(f"source descriptor limitations[{index}]", item, 512) for index, item in enumerate(limitations)),
    )


def _source_request_digest(context: Mapping[str, Any]) -> str:
    request = context.get("request")
    requirement = context.get("requirement")
    if not isinstance(request, Mapping):
        raise ArgumentError("source acquirer context request is malformed")
    requirement_id = getattr(requirement, "requirement_id", requirement.get("requirement_id") if isinstance(requirement, Mapping) else None)
    return content_digest({
        "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
        "plan_digest": context.get("plan_digest"),
        "requirement_id": requirement_id,
        "source_id": request.get("source_id"),
        "source_digest": request.get("source_digest"),
        "request_id": request.get("request_id"),
        "metadata": request.get("metadata", {}),
    })


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceSourceReceipt:
    request_digest: str
    plan_digest: str
    requirement_id: str
    domain: str
    source_id: str
    source_digest: str | None
    value_digest: str
    value_bytes: int
    provider: str
    protocol: str
    adapter_id: str
    contract_digest: str
    policy_digest: str
    source_kind: str
    freshness: str
    authority: str
    status: str
    observed_at_ms: int
    expires_at_ms: int | None
    citation_digest: str | None
    decision: str
    decision_reasons: tuple[str, ...]
    limitations: tuple[str, ...]
    receipt_digest: str

    def __post_init__(self) -> None:
        for name, value in (("request_digest", self.request_digest), ("plan_digest", self.plan_digest), ("value_digest", self.value_digest), ("contract_digest", self.contract_digest), ("policy_digest", self.policy_digest), ("receipt_digest", self.receipt_digest)):
            _digest(f"source receipt {name}", value)
        for name, value in (("requirement_id", self.requirement_id), ("domain", self.domain), ("provider", self.provider), ("protocol", self.protocol), ("adapter_id", self.adapter_id), ("source_kind", self.source_kind)):
            _source_identifier(f"source receipt {name}", value)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES or self.freshness not in AUTONOMOUS_EVIDENCE_PROVIDER_FRESHNESS_MODES or self.authority not in AUTONOMOUS_EVIDENCE_SOURCE_AUTHORITIES or self.status not in AUTONOMOUS_EVIDENCE_SOURCE_STATUSES or self.decision not in AUTONOMOUS_EVIDENCE_SOURCE_DECISIONS:
            raise ArgumentError("source receipt domain, freshness, authority, status, or decision is invalid")
        _source_identifier("source receipt source_id", self.source_id, MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES)
        _source_digest("source receipt source_digest", self.source_digest)
        _integer("source receipt value_bytes", self.value_bytes, 0, MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES)
        _integer("source receipt observed_at_ms", self.observed_at_ms, 0, 9_000_000_000_000_000)
        if self.expires_at_ms is not None:
            _integer("source receipt expires_at_ms", self.expires_at_ms, 0, 9_000_000_000_000_000)
        _source_digest("source receipt citation_digest", self.citation_digest)
        _bounded_list("source receipt decision_reasons", self.decision_reasons, 32)
        _bounded_list("source receipt limitations", self.limitations, MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS)
        if content_digest(self._descriptor()) != self.receipt_digest:
            raise ArgumentError("source receipt digest is invalid")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA,
            "request_digest": self.request_digest,
            "plan_digest": self.plan_digest,
            "requirement_id": self.requirement_id,
            "domain": self.domain,
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "value_digest": self.value_digest,
            "value_bytes": self.value_bytes,
            "provider": self.provider,
            "protocol": self.protocol,
            "adapter_id": self.adapter_id,
            "contract_digest": self.contract_digest,
            "policy_digest": self.policy_digest,
            "source_kind": self.source_kind,
            "freshness": self.freshness,
            "authority": self.authority,
            "status": self.status,
            "observed_at_ms": self.observed_at_ms,
            "expires_at_ms": self.expires_at_ms,
            "citation_digest": self.citation_digest,
            "decision": self.decision,
            "decision_reasons": list(self.decision_reasons),
            "limitations": list(self.limitations),
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "receipt_digest": self.receipt_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceSourceReceipt":
        if not isinstance(value, Mapping):
            raise ArgumentError("source receipt must be a mapping")
        allowed = set(cls.__dataclass_fields__) | {"schema", "retention", "secret_material"}  # type: ignore[attr-defined]
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA or value.get("retention") != _RETENTION or value.get("secret_material") != "never_returned":
            raise ArgumentError("source receipt contains unsupported fields")
        receipt = cls(
            request_digest=value.get("request_digest"), plan_digest=value.get("plan_digest"), requirement_id=value.get("requirement_id"), domain=value.get("domain"), source_id=value.get("source_id"), source_digest=value.get("source_digest"), value_digest=value.get("value_digest"), value_bytes=value.get("value_bytes"), provider=value.get("provider"), protocol=value.get("protocol"), adapter_id=value.get("adapter_id"), contract_digest=value.get("contract_digest"), policy_digest=value.get("policy_digest"), source_kind=value.get("source_kind"), freshness=value.get("freshness"), authority=value.get("authority"), status=value.get("status"), observed_at_ms=value.get("observed_at_ms"), expires_at_ms=value.get("expires_at_ms"), citation_digest=value.get("citation_digest"), decision=value.get("decision"), decision_reasons=tuple(value.get("decision_reasons", ())), limitations=tuple(value.get("limitations", ())), receipt_digest=value.get("receipt_digest"),
        )
        if canonical_json(value) != canonical_json(receipt.to_dict()):
            raise ArgumentError("source receipt is not canonical")
        return receipt


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceSourceLedgerEntry:
    sequence: int
    previous_entry_digest: str | None
    receipt: AutonomousEvidenceSourceReceipt
    entry_digest: str

    def __post_init__(self) -> None:
        _integer("source ledger entry sequence", self.sequence, 1, MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS)
        _optional_digest("source ledger entry previous_entry_digest", self.previous_entry_digest)
        if not isinstance(self.receipt, AutonomousEvidenceSourceReceipt):
            raise ArgumentError("source ledger entry receipt is malformed")
        _digest("source ledger entry entry_digest", self.entry_digest)
        if content_digest(self._descriptor()) != self.entry_digest:
            raise ArgumentError("source ledger entry digest is invalid")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA,
            "sequence": self.sequence,
            "previous_entry_digest": self.previous_entry_digest,
            "receipt": self.receipt.to_dict(),
            "retention": "metadata_only;raw_source_values_excluded",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "entry_digest": self.entry_digest}


class AutonomousEvidenceSourceLedger:
    """Hash-chained metadata ledger keyed by request digest."""

    def __init__(self, persistence: Any | None = None) -> None:
        if persistence is not None and not all(callable(getattr(persistence, name, None)) for name in ("append", "records")):
            raise ArgumentError("source ledger persistence is malformed")
        self.persistence = persistence
        self._entries: dict[str, AutonomousEvidenceSourceLedgerEntry] = {}

    def records(self) -> tuple[AutonomousEvidenceSourceLedgerEntry, ...]:
        return tuple(sorted(self._entries.values(), key=lambda entry: entry.sequence))

    def get(self, request_digest: str) -> AutonomousEvidenceSourceLedgerEntry | None:
        return self._entries.get(_digest("source ledger request_digest", request_digest))

    def append(self, receipt: AutonomousEvidenceSourceReceipt) -> AutonomousEvidenceSourceLedgerEntry:
        if not isinstance(receipt, AutonomousEvidenceSourceReceipt):
            raise ArgumentError("source ledger append requires a typed receipt")
        existing = self._entries.get(receipt.request_digest)
        if existing is not None:
            if existing.receipt.receipt_digest != receipt.receipt_digest:
                raise ArgumentError("source ledger request already has a conflicting receipt")
            return existing
        if len(self._entries) >= MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS:
            raise ArgumentError("source ledger is full")
        previous = self.records()[-1] if self.records() else None
        descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA,
            "sequence": 1 if previous is None else previous.sequence + 1,
            "previous_entry_digest": None if previous is None else previous.entry_digest,
            "receipt": receipt.to_dict(),
            "retention": "metadata_only;raw_source_values_excluded",
            "secret_material": "never_returned",
        }
        entry = AutonomousEvidenceSourceLedgerEntry(
            sequence=descriptor["sequence"], previous_entry_digest=descriptor["previous_entry_digest"], receipt=receipt,
            entry_digest=content_digest(descriptor),
        )
        persisted = entry.to_dict() if self.persistence is None else self.persistence.append(entry.to_dict())
        validated = _entry_from_dict(persisted)
        if validated.entry_digest != entry.entry_digest or validated.sequence != entry.sequence or validated.previous_entry_digest != entry.previous_entry_digest:
            raise ArgumentError("source ledger persistence changed the appended entry")
        self._entries[receipt.request_digest] = validated
        return validated

    def _snapshot_descriptor(self) -> dict[str, Any]:
        entries = [entry.to_dict() for entry in self.records()]
        return {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA,
            "entries": entries,
            "head_digest": None if not entries else entries[-1]["entry_digest"],
            "execution": "metadata_only_source_observation_ledger",
            "retention": "metadata_only;raw_source_values_excluded",
            "secret_material": "never_returned",
        }

    def snapshot(self) -> dict[str, Any]:
        descriptor = self._snapshot_descriptor()
        _json_bytes(descriptor, "source ledger snapshot", MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES)
        return {**descriptor, "ledger_digest": content_digest(descriptor)}

    def restore(self, snapshot: Mapping[str, Any] | None = None) -> dict[str, Any]:
        raw_entries = [] if snapshot is None else snapshot.get("entries")
        if snapshot is not None:
            _validate_snapshot(snapshot)
            raw_entries = snapshot["entries"]
        if not isinstance(raw_entries, Sequence) or isinstance(raw_entries, (str, bytes)) or len(raw_entries) > MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS:
            raise ArgumentError("source ledger entries are outside their bound")
        entries = tuple(_entry_from_dict(value) for value in raw_entries)
        _validate_chain(entries)
        restored: dict[str, AutonomousEvidenceSourceLedgerEntry] = {}
        for entry in entries:
            if entry.receipt.request_digest in restored:
                raise ArgumentError("source ledger contains duplicate request digests")
            restored[entry.receipt.request_digest] = entry
        self._entries = restored
        return {"restored": len(entries), "head_digest": None if not entries else entries[-1].entry_digest}

    def verify_integrity(self) -> dict[str, Any]:
        snapshot = self.snapshot()
        return {"verified": True, "entries": len(self._entries), "head_digest": snapshot["head_digest"], "ledger_digest": snapshot["ledger_digest"]}


def _entry_from_dict(value: Mapping[str, Any]) -> AutonomousEvidenceSourceLedgerEntry:
    if not isinstance(value, Mapping):
        raise ArgumentError("source ledger entry must be a mapping")
    allowed = {"schema", "sequence", "previous_entry_digest", "receipt", "entry_digest", "retention", "secret_material"}
    if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA or value.get("retention") != "metadata_only;raw_source_values_excluded" or value.get("secret_material") != "never_returned":
        raise ArgumentError("source ledger entry contains unsupported fields")
    entry = AutonomousEvidenceSourceLedgerEntry(
        sequence=value.get("sequence"),
        previous_entry_digest=value.get("previous_entry_digest"),
        receipt=AutonomousEvidenceSourceReceipt.from_dict(value.get("receipt")),
        entry_digest=value.get("entry_digest"),
    )
    if canonical_json(value) != canonical_json(entry.to_dict()):
        raise ArgumentError("source ledger entry is not canonical")
    return entry


def _validate_chain(entries: Sequence[AutonomousEvidenceSourceLedgerEntry]) -> None:
    previous = None
    for index, entry in enumerate(entries, start=1):
        if entry.sequence != index or entry.previous_entry_digest != previous:
            raise ArgumentError("source ledger hash chain is not contiguous")
        previous = entry.entry_digest


def _validate_snapshot(value: Mapping[str, Any]) -> None:
    if not isinstance(value, Mapping):
        raise ArgumentError("source ledger snapshot must be a mapping")
    allowed = {"schema", "entries", "head_digest", "execution", "retention", "secret_material", "ledger_digest"}
    if set(value) != allowed or value.get("schema") != AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA or value.get("execution") != "metadata_only_source_observation_ledger" or value.get("retention") != "metadata_only;raw_source_values_excluded" or value.get("secret_material") != "never_returned":
        raise ArgumentError("source ledger snapshot contains unsupported fields")
    entries = tuple(_entry_from_dict(item) for item in value.get("entries", ()))
    _validate_chain(entries)
    descriptor = {
        "schema": AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA,
        "entries": [entry.to_dict() for entry in entries],
        "head_digest": None if not entries else entries[-1].entry_digest,
        "execution": "metadata_only_source_observation_ledger",
        "retention": "metadata_only;raw_source_values_excluded",
        "secret_material": "never_returned",
    }
    if value.get("head_digest") != descriptor["head_digest"] or value.get("ledger_digest") != content_digest(descriptor):
        raise ArgumentError("source ledger snapshot digest or head is invalid")
    _json_bytes(value, "source ledger snapshot", MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES)


class AutonomousEvidenceSourceLedgerTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousEvidenceSourceLedgerTextStore(AutonomousEvidenceSourceLedgerTextStore, Protocol):
    def write_if_unchanged(self, expected_ledger_digest: str | None, value: str) -> bool: ...


class JsonAutonomousEvidenceSourceLedgerPersistence:
    def __init__(self, store: AutonomousEvidenceSourceLedgerTextStore, *, max_bytes: int = MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("source ledger JSON persistence requires a text store")
        self.store = store
        self.max_bytes = _integer("source ledger persistence max_bytes", max_bytes, 1, MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("source ledger persistence text exceeds its bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("source ledger persistence text is invalid JSON") from error
        if not isinstance(value, Mapping) or canonical_json(value) != encoded:
            raise ArgumentError("source ledger persistence text is not canonical")
        _validate_snapshot(value)
        return dict(value)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        _validate_snapshot(snapshot)
        encoded = canonical_json(snapshot)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("source ledger persistence snapshot exceeds its bound")
        self.store.write(encoded)

    def records(self) -> tuple[dict[str, Any], ...]:
        snapshot = self.read()
        return () if snapshot is None else tuple(snapshot["entries"])

    def append(self, entry: Mapping[str, Any]) -> dict[str, Any]:
        validated = _entry_from_dict(entry)
        current = self.read()
        entries = [] if current is None else list(current["entries"])
        existing = next((item for item in entries if item["sequence"] == validated.sequence), None)
        if existing is not None:
            if existing["entry_digest"] != validated.entry_digest:
                raise ArgumentError("source ledger persistence has a conflicting sequence")
            return dict(existing)
        if validated.sequence != len(entries) + 1 or validated.previous_entry_digest != (None if current is None else current["head_digest"]):
            raise ArgumentError("source ledger persistence append is stale or out of order")
        next_snapshot = {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA,
            "entries": [*entries, validated.to_dict()],
            "head_digest": validated.entry_digest,
            "execution": "metadata_only_source_observation_ledger",
            "retention": "metadata_only;raw_source_values_excluded",
            "secret_material": "never_returned",
        }
        next_snapshot["ledger_digest"] = content_digest({key: value for key, value in next_snapshot.items() if key != "ledger_digest"})
        self.write(next_snapshot)
        return validated.to_dict()


class TransactionalJsonAutonomousEvidenceSourceLedgerPersistence(JsonAutonomousEvidenceSourceLedgerPersistence):
    def write_if_unchanged(self, expected_ledger_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_ledger_digest is not None:
            _digest("source ledger expected ledger_digest", expected_ledger_digest)
        _validate_snapshot(snapshot)
        encoded = canonical_json(snapshot)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("source ledger persistence snapshot exceeds its bound")
        writer = getattr(self.store, "write_if_unchanged", None)
        if not callable(writer):
            raise ArgumentError("source ledger store does not support compare-and-swap")
        result = writer(expected_ledger_digest, encoded)
        if not isinstance(result, bool):
            raise ArgumentError("source ledger compare-and-swap returned a non-boolean")
        return result

    def append(self, entry: Mapping[str, Any]) -> dict[str, Any]:
        validated = _entry_from_dict(entry)
        current = self.read()
        entries = [] if current is None else list(current["entries"])
        existing = next((item for item in entries if item["sequence"] == validated.sequence), None)
        if existing is not None:
            if existing["entry_digest"] != validated.entry_digest:
                raise ArgumentError("source ledger persistence has a conflicting sequence")
            return dict(existing)
        if validated.sequence != len(entries) + 1 or validated.previous_entry_digest != (None if current is None else current["head_digest"]):
            raise ArgumentError("source ledger persistence append is stale or out of order")
        descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA,
            "entries": [*entries, validated.to_dict()],
            "head_digest": validated.entry_digest,
            "execution": "metadata_only_source_observation_ledger",
            "retention": "metadata_only;raw_source_values_excluded",
            "secret_material": "never_returned",
        }
        snapshot = {**descriptor, "ledger_digest": content_digest(descriptor)}
        if not self.write_if_unchanged(None if current is None else current["ledger_digest"], snapshot):
            raise ArgumentError("source ledger persistence rejected a stale writer")
        return validated.to_dict()


class AutonomousEvidenceSourceLedgerPersistenceCoordinator:
    def __init__(self, ledger: AutonomousEvidenceSourceLedger, persistence: Any) -> None:
        if not isinstance(ledger, AutonomousEvidenceSourceLedger) or not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("source ledger persistence coordinator is malformed")
        self.ledger = ledger
        self.persistence = persistence
        self._expected_ledger_digest: str | None = None

    def restore(self) -> dict[str, Any]:
        snapshot = self.persistence.read()
        if snapshot is None:
            self._expected_ledger_digest = None
            self.ledger.restore(None)
        else:
            self.ledger.restore(snapshot)
            self._expected_ledger_digest = snapshot["ledger_digest"]
        return self.ledger.verify_integrity()

    def flush(self) -> dict[str, Any]:
        snapshot = self.ledger.snapshot()
        writer = getattr(self.persistence, "write_if_unchanged", None)
        if callable(writer):
            if not writer(self._expected_ledger_digest, snapshot):
                raise ArgumentError("source ledger persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_ledger_digest = snapshot["ledger_digest"]
        return snapshot


class AutonomousEvidenceSourceAdmissionError(ArgumentError):
    """Non-retryable source truth/admission refusal with a stable decision class."""

    def __init__(self, decision: str, reasons: Sequence[str]) -> None:
        self.decision = decision
        self.reasons = tuple(reasons)
        super().__init__(f"source admission decision: {decision}")


class AutonomousEvidenceSourceAcquirer:
    """Guard a caller-owned acquirer with contract and provenance admission."""

    def __init__(
        self,
        base: Any,
        *,
        contract_registry: AutonomousEvidenceProviderContractRegistry,
        adapter_id: str,
        domain: str,
        source_kind: str,
        policy: AutonomousEvidenceSourcePolicy,
        ledger: AutonomousEvidenceSourceLedger | None = None,
        describe_source: Callable[[Mapping[str, Any]], Mapping[str, Any]] | None = None,
    ) -> None:
        if not callable(getattr(base, "acquire", None)):
            raise ArgumentError("source acquirer base must expose acquire")
        if not isinstance(contract_registry, AutonomousEvidenceProviderContractRegistry):
            raise ArgumentError("source acquirer requires a typed contract registry")
        if not isinstance(policy, AutonomousEvidenceSourcePolicy):
            raise ArgumentError("source acquirer policy is malformed")
        if ledger is not None and not isinstance(ledger, AutonomousEvidenceSourceLedger):
            raise ArgumentError("source acquirer ledger is malformed")
        if not callable(describe_source):
            raise ArgumentError("source acquirer requires an explicit describe_source callback")
        self.base = base
        self.contract_registry = contract_registry
        self.adapter_id = _identifier("source acquirer adapter_id", adapter_id)
        self.domain = _source_identifier("source acquirer domain", domain)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("source acquirer domain is unsupported")
        self.source_kind = _source_identifier("source acquirer source_kind", source_kind)
        self.policy = policy
        self.ledger = ledger
        self.describe_source = describe_source
        contract = contract_registry.contract_for_adapter(self.adapter_id, self.domain)
        if self.source_kind not in contract.source_kinds:
            raise ArgumentError(f"source kind is not declared by provider contract: {contract.contract_id}")

    def acquire(self, context: Mapping[str, Any]) -> Any:
        if not isinstance(context, Mapping):
            raise ArgumentError("source acquirer context must be a mapping")
        requirement = context.get("requirement")
        requirement_domain = getattr(requirement, "domain", requirement.get("domain") if isinstance(requirement, Mapping) else None)
        if requirement_domain != self.domain:
            raise ArgumentError("source acquirer received a different domain")
        self.contract_registry.verify()
        contract = self.contract_registry.contract_for_adapter(self.adapter_id, self.domain)
        value = self.base.acquire(context)
        value_digest = content_digest(value)
        value_bytes = _source_value_bytes(value)
        now_ms = self.policy.now()
        described = self.describe_source({
            "context": context,
            "value_digest": value_digest,
            "value_bytes": value_bytes,
            "contract_digest": contract.contract_digest,
            "provider": contract.provider,
            "protocol": contract.protocol,
            "source_kind": self.source_kind,
            "now_ms": now_ms,
        })
        if not isinstance(described, Mapping):
            raise ArgumentError("source descriptor callback must return a mapping")
        request = context.get("request")
        request_source_id = request.get("source_id") if isinstance(request, Mapping) else None
        descriptor = normalize_autonomous_evidence_source_descriptor(described, default_source_id=request_source_id)
        if descriptor.source_id != request_source_id:
            raise ArgumentError("source descriptor source_id does not match the acquisition request")
        requested_source_digest = request.get("source_digest") if isinstance(request, Mapping) else None
        if requested_source_digest is not None and descriptor.source_digest != requested_source_digest:
            raise ArgumentError("source descriptor source_digest does not match the acquisition request")
        decision = self.policy.evaluate(contract, descriptor, now_ms=now_ms)
        receipt_descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA,
            "request_digest": _source_request_digest(context),
            "plan_digest": _digest("source receipt plan_digest", context.get("plan_digest")),
            "requirement_id": _source_identifier("source receipt requirement_id", getattr(requirement, "requirement_id", requirement.get("requirement_id") if isinstance(requirement, Mapping) else None)),
            "domain": self.domain,
            "source_id": descriptor.source_id,
            "source_digest": descriptor.source_digest,
            "value_digest": value_digest,
            "value_bytes": value_bytes,
            "provider": contract.provider,
            "protocol": contract.protocol,
            "adapter_id": self.adapter_id,
            "contract_digest": contract.contract_digest,
            "policy_digest": self.policy.policy_digest,
            "source_kind": self.source_kind,
            "freshness": contract.freshness,
            "authority": descriptor.authority,
            "status": descriptor.status,
            "observed_at_ms": descriptor.observed_at_ms,
            "expires_at_ms": descriptor.expires_at_ms,
            "citation_digest": descriptor.citation_digest,
            "decision": decision.decision,
            "decision_reasons": list(decision.reasons),
            "limitations": list(descriptor.limitations),
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }
        receipt = AutonomousEvidenceSourceReceipt(
            request_digest=receipt_descriptor["request_digest"],
            plan_digest=receipt_descriptor["plan_digest"],
            requirement_id=receipt_descriptor["requirement_id"],
            domain=receipt_descriptor["domain"],
            source_id=receipt_descriptor["source_id"],
            source_digest=receipt_descriptor["source_digest"],
            value_digest=receipt_descriptor["value_digest"],
            value_bytes=receipt_descriptor["value_bytes"],
            provider=receipt_descriptor["provider"],
            protocol=receipt_descriptor["protocol"],
            adapter_id=receipt_descriptor["adapter_id"],
            contract_digest=receipt_descriptor["contract_digest"],
            policy_digest=receipt_descriptor["policy_digest"],
            source_kind=receipt_descriptor["source_kind"],
            freshness=receipt_descriptor["freshness"],
            authority=receipt_descriptor["authority"],
            status=receipt_descriptor["status"],
            observed_at_ms=receipt_descriptor["observed_at_ms"],
            expires_at_ms=receipt_descriptor["expires_at_ms"],
            citation_digest=receipt_descriptor["citation_digest"],
            decision=receipt_descriptor["decision"],
            decision_reasons=tuple(receipt_descriptor["decision_reasons"]),
            limitations=tuple(receipt_descriptor["limitations"]),
            receipt_digest=content_digest(receipt_descriptor),
        )
        if self.ledger is not None:
            self.ledger.append(receipt)
        if not decision.usable:
            raise AutonomousEvidenceSourceAdmissionError(decision.decision, decision.reasons)
        return value

    def to_dict(self) -> dict[str, Any]:
        contract = self.contract_registry.contract_for_adapter(self.adapter_id, self.domain)
        return {
            "schema": AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA,
            "adapter_id": self.adapter_id,
            "domain": self.domain,
            "source_kind": self.source_kind,
            "contract_digest": contract.contract_digest,
            "policy_digest": self.policy.policy_digest,
            "ledger_enabled": self.ledger is not None,
            "execution": "contract_and_source_admission_only;raw_value_transient",
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }


def create_autonomous_evidence_source_acquirer(
    contract_registry: AutonomousEvidenceProviderContractRegistry,
    *,
    adapter_id: str,
    domain: str,
    source_kind: str | None = None,
    policy: AutonomousEvidenceSourcePolicy | None = None,
    ledger: AutonomousEvidenceSourceLedger | None = None,
    describe_source: Callable[[Mapping[str, Any]], Mapping[str, Any]],
) -> AutonomousEvidenceSourceAcquirer:
    if not isinstance(contract_registry, AutonomousEvidenceProviderContractRegistry):
        raise ArgumentError("source acquirer requires a typed contract registry")
    contract = contract_registry.contract_for_adapter(adapter_id, domain)
    selected_source_kind = source_kind
    if selected_source_kind is None:
        if len(contract.source_kinds) != 1:
            raise ArgumentError("source acquirer requires source_kind for a multi-kind contract")
        selected_source_kind = contract.source_kinds[0]
    base = contract_registry.create_acquirer_for_adapter(adapter_id, domain)
    return AutonomousEvidenceSourceAcquirer(
        base,
        contract_registry=contract_registry,
        adapter_id=adapter_id,
        domain=domain,
        source_kind=selected_source_kind,
        policy=policy or AutonomousEvidenceSourcePolicy(),
        ledger=ledger,
        describe_source=describe_source,
    )


def create_autonomous_evidence_source_guard(
    base: Any,
    *,
    contract: AutonomousEvidenceProviderContract,
    contract_registry: AutonomousEvidenceProviderContractRegistry,
    adapter_id: str,
    domain: str,
    source_kind: str,
    policy: AutonomousEvidenceSourcePolicy,
    ledger: AutonomousEvidenceSourceLedger | None = None,
    describe_source: Callable[[Mapping[str, Any]], Mapping[str, Any]],
) -> AutonomousEvidenceSourceAcquirer:
    if not isinstance(contract, AutonomousEvidenceProviderContract):
        raise ArgumentError("source guard requires a typed provider contract")
    live = contract_registry.contract_for_adapter(adapter_id, domain)
    if live.contract_digest != contract.contract_digest:
        raise ArgumentError("source guard contract is stale")
    return AutonomousEvidenceSourceAcquirer(
        base,
        contract_registry=contract_registry,
        adapter_id=adapter_id,
        domain=domain,
        source_kind=source_kind,
        policy=policy,
        ledger=ledger,
        describe_source=describe_source,
    )


__all__ = [
    "AUTONOMOUS_EVIDENCE_SOURCE_SCHEMA",
    "AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_ENTRY_SCHEMA",
    "AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_SCHEMA",
    "AUTONOMOUS_EVIDENCE_SOURCE_POLICY_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_ID_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_LIMITATIONS",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_RECORDS",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_VALUE_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_LEDGER_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_AGE_MS",
    "MAX_AUTONOMOUS_EVIDENCE_SOURCE_FUTURE_SKEW_MS",
    "DEFAULT_AUTONOMOUS_REALTIME_SOURCE_AGE_MS",
    "AUTONOMOUS_EVIDENCE_SOURCE_AUTHORITIES",
    "AUTONOMOUS_EVIDENCE_SOURCE_STATUSES",
    "AUTONOMOUS_EVIDENCE_SOURCE_DECISIONS",
    "AutonomousEvidenceSourceDescriptor",
    "AutonomousEvidenceSourcePolicyDecision",
    "AutonomousEvidenceSourcePolicy",
    "normalize_autonomous_evidence_source_descriptor",
    "AutonomousEvidenceSourceReceipt",
    "AutonomousEvidenceSourceLedgerEntry",
    "AutonomousEvidenceSourceLedger",
    "AutonomousEvidenceSourceLedgerTextStore",
    "TransactionalAutonomousEvidenceSourceLedgerTextStore",
    "JsonAutonomousEvidenceSourceLedgerPersistence",
    "TransactionalJsonAutonomousEvidenceSourceLedgerPersistence",
    "AutonomousEvidenceSourceLedgerPersistenceCoordinator",
    "AutonomousEvidenceSourceAdmissionError",
    "AutonomousEvidenceSourceAcquirer",
    "create_autonomous_evidence_source_acquirer",
    "create_autonomous_evidence_source_guard",
]
