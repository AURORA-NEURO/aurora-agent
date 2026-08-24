"""Bounded retry and failure classification for autonomous evidence acquisition.

Retry is deliberately a separate policy boundary from candidate failover. A retry can replay one
exact, digest-bound source route for a typed transient failure; failover may then move to the next
reviewed adapter only when the same classification is permitted by the retry policy. Neither
attempt records nor exceptions retain prompts, provider payloads, credentials, or raw messages.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import math
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .errors import ArgumentError, SdkError, TransportError
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .llm_runtime import CredentialError, ProviderError


AUTONOMOUS_EVIDENCE_RETRY_POLICY_SCHEMA = "bioprism-python-autonomous-evidence-retry-policy/0.1"
AUTONOMOUS_EVIDENCE_RETRY_ATTEMPT_SCHEMA = "bioprism-python-autonomous-evidence-retry-attempt/0.1"
MAX_AUTONOMOUS_EVIDENCE_RETRY_ATTEMPTS = 8
MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS = 60_000
MAX_AUTONOMOUS_EVIDENCE_RETRY_FAILURE_CLASSES = 32
MAX_AUTONOMOUS_EVIDENCE_RETRY_CLASS_BYTES = 128
AUTONOMOUS_EVIDENCE_DEFAULT_RETRYABLE_FAILURE_CLASSES = (
    "circuit_open",
    "provider_retryable",
    "timeout",
    "transport_error",
    "http_5xx",
    "rate_limited",
)

_RETENTION = "metadata_only_policy_and_attempts;errors_and_values_never_persisted"
_STATUSES = frozenset({"succeeded", "retrying", "failed", "exhausted"})


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_EVIDENCE_RETRY_CLASS_BYTES) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return result


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        raise ArgumentError(f"{name} is outside its bound")
    return value


def _finite(name: str, value: Any, minimum: float = 0.0, maximum: float = float("1.7976931348623157e308")) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or float(value) < minimum or float(value) > maximum:
        raise ArgumentError(f"{name} is outside its bound")
    return float(value)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRetryClassification:
    failure_class: str
    retryable: bool

    def __post_init__(self) -> None:
        _identifier("evidence retry failure_class", self.failure_class)
        if not isinstance(self.retryable, bool):
            raise ArgumentError("evidence retry retryable must be boolean")

    def to_dict(self) -> dict[str, Any]:
        return {"failure_class": self.failure_class, "retryable": self.retryable}


class AutonomousEvidenceAcquisitionError(SdkError):
    """Typed transient/refusal error for caller-owned evidence adapters."""

    def __init__(self, failure_class: str, retryable: bool, message: str = "autonomous evidence acquisition failed") -> None:
        self.failure_class = _identifier("evidence acquisition failure_class", failure_class)
        if not isinstance(retryable, bool):
            raise ArgumentError("evidence acquisition retryable must be boolean")
        self.retryable = retryable
        # The message is intentionally generic; callers must not place transport detail here if an
        # exception crosses an application boundary.
        super().__init__(_text("evidence acquisition message", message, 256))


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRetryPolicy:
    max_attempts: int = 3
    base_delay_ms: int = 100
    max_delay_ms: int = 5_000
    retryable_failure_classes: tuple[str, ...] = field(default_factory=lambda: AUTONOMOUS_EVIDENCE_DEFAULT_RETRYABLE_FAILURE_CLASSES)

    def __post_init__(self) -> None:
        _integer("evidence retry max_attempts", self.max_attempts, 1, MAX_AUTONOMOUS_EVIDENCE_RETRY_ATTEMPTS)
        _integer("evidence retry base_delay_ms", self.base_delay_ms, 0, MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS)
        _integer("evidence retry max_delay_ms", self.max_delay_ms, 0, MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS)
        if self.max_delay_ms < self.base_delay_ms:
            raise ArgumentError("evidence retry max_delay_ms must be at least base_delay_ms")
        classes = self.retryable_failure_classes
        if isinstance(classes, (str, bytes, bytearray)) or not isinstance(classes, Sequence) or not 1 <= len(classes) <= MAX_AUTONOMOUS_EVIDENCE_RETRY_FAILURE_CLASSES:
            raise ArgumentError("evidence retry failure classes are outside their bound")
        normalized = tuple(sorted(_identifier(f"evidence retry failure classes[{index}]", value) for index, value in enumerate(classes)))
        if len(set(normalized)) != len(normalized):
            raise ArgumentError("evidence retry failure classes contain duplicates")
        object.__setattr__(self, "retryable_failure_classes", normalized)

    def delay_for_attempt(self, attempt: int) -> int:
        _integer("evidence retry attempt", attempt, 1, self.max_attempts)
        return min(self.max_delay_ms, self.base_delay_ms * (2 ** max(0, attempt - 1)))

    def permits(self, classification: AutonomousEvidenceRetryClassification) -> bool:
        if not isinstance(classification, AutonomousEvidenceRetryClassification):
            raise ArgumentError("evidence retry classification is malformed")
        return classification.retryable and classification.failure_class in self.retryable_failure_classes

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RETRY_POLICY_SCHEMA,
            "max_attempts": self.max_attempts,
            "base_delay_ms": self.base_delay_ms,
            "max_delay_ms": self.max_delay_ms,
            "retryable_failure_classes": list(self.retryable_failure_classes),
            "execution": "caller_controlled_bounded_retry;no_authorization",
            "retention": "metadata_only_policy;no_errors_or_values",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRetryAttempt:
    domain: str
    attempt: int
    status: str
    failure_class: str | None
    retryable: bool
    delay_ms: int
    latency_ms: float

    def __post_init__(self) -> None:
        _identifier("evidence retry attempt domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("evidence retry attempt domain is unsupported")
        _integer("evidence retry attempt attempt", self.attempt, 1, MAX_AUTONOMOUS_EVIDENCE_RETRY_ATTEMPTS)
        if self.status not in _STATUSES:
            raise ArgumentError("evidence retry attempt status is invalid")
        if self.failure_class is not None:
            _identifier("evidence retry attempt failure_class", self.failure_class)
        if not isinstance(self.retryable, bool):
            raise ArgumentError("evidence retry attempt retryable must be boolean")
        _integer("evidence retry attempt delay_ms", self.delay_ms, 0, MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS)
        _finite("evidence retry attempt latency_ms", self.latency_ms)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RETRY_ATTEMPT_SCHEMA,
            "domain": self.domain,
            "attempt": self.attempt,
            "status": self.status,
            "failure_class": self.failure_class,
            "retryable": self.retryable,
            "delay_ms": self.delay_ms,
            "latency_ms": self.latency_ms,
            "retention": "metadata_only;error_class_only",
            "secret_material": "never_returned",
        }


def _classification_from(value: Any) -> AutonomousEvidenceRetryClassification:
    if isinstance(value, AutonomousEvidenceRetryClassification):
        return value
    if isinstance(value, Mapping):
        return AutonomousEvidenceRetryClassification(
            failure_class=value.get("failure_class", value.get("failureClass")),
            retryable=value.get("retryable"),
        )
    raise ArgumentError("evidence retry classifier returned malformed metadata")


def classify_autonomous_evidence_acquisition_error(error: BaseException) -> AutonomousEvidenceRetryClassification:
    if isinstance(error, AutonomousEvidenceAcquisitionError):
        return AutonomousEvidenceRetryClassification(error.failure_class, error.retryable)
    if isinstance(error, CredentialError):
        return AutonomousEvidenceRetryClassification("credential_error", False)
    if isinstance(error, ArgumentError):
        return AutonomousEvidenceRetryClassification("invalid_request", False)
    if isinstance(error, ProviderError):
        if error.circuit_open:
            return AutonomousEvidenceRetryClassification("circuit_open", True)
        return AutonomousEvidenceRetryClassification("provider_retryable" if error.retryable else "provider_error", bool(error.retryable))
    if isinstance(error, TimeoutError):
        return AutonomousEvidenceRetryClassification("timeout", True)
    if isinstance(error, TransportError):
        return AutonomousEvidenceRetryClassification("transport_error", True)
    return AutonomousEvidenceRetryClassification("unknown", False)


class AutonomousEvidenceRetryAcquirer(Protocol):
    def acquire(self, context: Mapping[str, Any]) -> Any: ...


def create_autonomous_evidence_retrying_acquirer(
    acquirer: Any,
    *,
    policy: AutonomousEvidenceRetryPolicy | None = None,
    classify: Callable[[BaseException], AutonomousEvidenceRetryClassification | Mapping[str, Any]] | None = None,
    observe: Callable[[AutonomousEvidenceRetryAttempt], Any] | None = None,
    clock: Callable[[], float] | None = None,
    sleep: Callable[[int], Any] | None = None,
) -> Any:
    """Wrap one reviewed acquirer with bounded retry and metadata-only attempt observations."""

    if not callable(getattr(acquirer, "acquire", None)):
        raise ArgumentError("evidence retry acquirer is malformed")
    typed_policy = policy or AutonomousEvidenceRetryPolicy()
    if not isinstance(typed_policy, AutonomousEvidenceRetryPolicy):
        raise ArgumentError("evidence retry policy is malformed")
    if classify is not None and not callable(classify):
        raise ArgumentError("evidence retry classifier is malformed")
    if observe is not None and not callable(observe):
        raise ArgumentError("evidence retry observer is malformed")
    clock_fn = clock or (lambda: time.monotonic() * 1000.0)
    sleep_fn = sleep or (lambda delay_ms: time.sleep(delay_ms / 1000.0))
    if not callable(clock_fn) or not callable(sleep_fn):
        raise ArgumentError("evidence retry clock or sleep callback is malformed")

    class RetryingAcquirer:
        def acquire(self, context: Mapping[str, Any]) -> Any:
            if not isinstance(context, Mapping) or not isinstance(context.get("request"), Mapping) or context.get("requirement") is None:
                raise ArgumentError("evidence retry acquisition context is malformed")
            requirement = context["requirement"]
            domain = getattr(requirement, "domain", requirement.get("domain") if isinstance(requirement, Mapping) else None)
            _identifier("evidence retry context domain", domain)
            for attempt in range(1, typed_policy.max_attempts + 1):
                started = _finite("evidence retry clock", clock_fn())
                try:
                    value = acquirer.acquire({**context, "attempt": attempt})
                    finished = _finite("evidence retry clock", clock_fn())
                    if observe is not None:
                        observe(AutonomousEvidenceRetryAttempt(domain, attempt, "succeeded", None, False, 0, max(0.0, finished - started)))
                    return value
                except Exception as error:
                    classification = _classification_from(classify(error) if classify is not None else classify_autonomous_evidence_acquisition_error(error))
                    should_retry = typed_policy.permits(classification) and attempt < typed_policy.max_attempts
                    delay_ms = typed_policy.delay_for_attempt(attempt) if should_retry else 0
                    finished = _finite("evidence retry clock", clock_fn())
                    if observe is not None:
                        observe(AutonomousEvidenceRetryAttempt(
                            domain,
                            attempt,
                            "retrying" if should_retry else "exhausted" if attempt >= typed_policy.max_attempts else "failed",
                            classification.failure_class,
                            classification.retryable,
                            delay_ms,
                            max(0.0, finished - started),
                        ))
                    if not should_retry:
                        raise
                    sleep_fn(delay_ms)
            raise ArgumentError("evidence retry loop exhausted unexpectedly")

    return RetryingAcquirer()


__all__ = [
    "AUTONOMOUS_EVIDENCE_RETRY_POLICY_SCHEMA",
    "AUTONOMOUS_EVIDENCE_RETRY_ATTEMPT_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_RETRY_ATTEMPTS",
    "MAX_AUTONOMOUS_EVIDENCE_RETRY_DELAY_MS",
    "MAX_AUTONOMOUS_EVIDENCE_RETRY_FAILURE_CLASSES",
    "AUTONOMOUS_EVIDENCE_DEFAULT_RETRYABLE_FAILURE_CLASSES",
    "AutonomousEvidenceRetryClassification",
    "AutonomousEvidenceAcquisitionError",
    "AutonomousEvidenceRetryPolicy",
    "AutonomousEvidenceRetryAttempt",
    "classify_autonomous_evidence_acquisition_error",
    "AutonomousEvidenceRetryAcquirer",
    "create_autonomous_evidence_retrying_acquirer",
]
