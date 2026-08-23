"""Provider-call accounting for restart-safe autonomous executions.

This module is the narrow bridge between the live LLM runtime and the durable autonomy
controller.  It deliberately receives provider-neutral request metadata rather than prompts and
never writes response text, tool arguments, credentials, or provider wire payloads.  Admission is
authoritative: a call is not sent when the execution policy rejects its estimated cost or count.
Outcome receipts are value-only and can be used alongside the provider health ledger and explicit
brain evaluator without turning transport success into an automatic task reward.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping

from .authoring import content_digest
from .autonomy_persistence import AutonomousExecutionController, AutonomyPolicyError
from .llm_runtime import (
    ProviderError,
    ProviderInvocationMetadata,
    ProviderResponse,
)


AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA = "bioprism-python-autonomous-provider-invocation/0.1"
MAX_PROVIDER_INVOCATION_ATTEMPTS = 8
MAX_PROVIDER_INVOCATION_TURNS = 32


class AutonomousProviderInvocationError(RuntimeError):
    """A provider accounting hook received malformed or inconsistent metadata."""


def _bounded_text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomousProviderInvocationError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise AutonomousProviderInvocationError(f"{name} exceeds its bounded size")
    return value


def _bounded_digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        raise AutonomousProviderInvocationError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_nonnegative(name: str, value: Any) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value)) or value < 0:
        raise AutonomousProviderInvocationError(f"{name} must be finite and non-negative")
    return float(value)


def _usage_count(usage: Mapping[str, Any], *names: str) -> int | None:
    for name in names:
        value = usage.get(name)
        if isinstance(value, int) and not isinstance(value, bool) and value >= 0:
            return value
    return None


@dataclass(frozen=True, slots=True)
class AutonomousProviderInvocationReceipt:
    """Redacted receipt for one admitted provider request."""

    execution_id: str | None
    provider: str
    model: str
    kind: str
    attempt: int
    turn: int
    status: str
    outcome: str
    input_tokens: int
    output_tokens: int
    estimated_cost_units: float
    actual_cost_units: float
    latency_ms: float
    selection_digest: str | None
    outcome_digest: str
    request_id_digest: str | None = None
    failure_class: str | None = None
    status_code: int | None = None

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA,
            "execution_id": self.execution_id,
            "provider": self.provider,
            "model": self.model,
            "kind": self.kind,
            "attempt": self.attempt,
            "turn": self.turn,
            "status": self.status,
            "outcome": self.outcome,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "estimated_cost_units": self.estimated_cost_units,
            "actual_cost_units": self.actual_cost_units,
            "latency_ms": self.latency_ms,
            "selection_digest": self.selection_digest,
            "outcome_digest": self.outcome_digest,
            "request_id_digest": self.request_id_digest,
            "failure_class": self.failure_class,
            "status_code": self.status_code,
            "retention": "metadata_only_no_provider_payloads_or_credentials",
        }
        return result


class AutonomousProviderInvocationSession:
    """Admission and outcome observer bound to one selected model/failover attempt."""

    def __init__(
        self,
        *,
        controller: AutonomousExecutionController,
        provider: str,
        model: str,
        selection_digest: str | None = None,
        cost_per_million_tokens: float = 0.0,
        attempt: int = 0,
        kind: str = "provider_call",
    ) -> None:
        if not isinstance(controller, AutonomousExecutionController):
            raise AutonomousProviderInvocationError("controller must be an AutonomousExecutionController")
        self.controller = controller
        self.provider = _bounded_text("provider", provider)
        self.model = _bounded_text("model", model)
        self.selection_digest = None if selection_digest is None else _bounded_digest("selection_digest", selection_digest)
        self.cost_per_million_tokens = _bounded_nonnegative(
            "cost_per_million_tokens", cost_per_million_tokens
        )
        if not isinstance(attempt, int) or isinstance(attempt, bool) or not 0 <= attempt <= MAX_PROVIDER_INVOCATION_ATTEMPTS:
            raise AutonomousProviderInvocationError("attempt is outside its bound")
        self.attempt = attempt
        self.kind = _bounded_text("invocation kind", kind, maximum=128)
        self._turn = 0
        self._pending: list[tuple[ProviderInvocationMetadata, float, int]] = []
        self._receipts: list[AutonomousProviderInvocationReceipt] = []

    @property
    def receipts(self) -> tuple[AutonomousProviderInvocationReceipt, ...]:
        return tuple(self._receipts)

    def before(self, metadata: ProviderInvocationMetadata) -> None:
        if not isinstance(metadata, ProviderInvocationMetadata):
            raise AutonomousProviderInvocationError("provider invocation metadata is malformed")
        if metadata.provider != self.provider or metadata.model != self.model:
            raise AutonomousProviderInvocationError("provider invocation metadata does not match selected model")
        if self._turn >= MAX_PROVIDER_INVOCATION_TURNS:
            raise AutonomousProviderInvocationError("provider invocation turn limit exceeded")
        if self.attempt > self.controller.policy.max_provider_failovers:
            raise AutonomyPolicyError("max_provider_failovers exceeded")
        estimated_tokens = metadata.input_tokens + metadata.requested_output_tokens
        estimated_cost = estimated_tokens * self.cost_per_million_tokens / 1_000_000.0
        admission = self.controller.admit_provider_call(
            cost_units=estimated_cost,
            provider=self.provider,
            model=self.model,
            invocation_kind=metadata.kind,
            attempt=self.attempt,
            turn=self._turn,
            selection_digest=self.selection_digest,
            estimated_cost_units=estimated_cost,
        )
        del admission
        self._pending.append((metadata, estimated_cost, self._turn))
        self._turn += 1

    def after(
        self,
        metadata: ProviderInvocationMetadata,
        response: ProviderResponse | None,
        error: BaseException | None,
        latency_ms: float,
    ) -> None:
        if not self._pending:
            raise AutonomousProviderInvocationError("provider outcome has no admitted invocation")
        admitted_metadata, estimated_cost, turn = self._pending.pop()
        if admitted_metadata != metadata:
            raise AutonomousProviderInvocationError("provider outcome metadata does not match admission")
        if response is not None and not isinstance(response, ProviderResponse):
            raise AutonomousProviderInvocationError("provider outcome response is malformed")
        latency = _bounded_nonnegative("latency_ms", latency_ms)
        usage = {} if response is None else response.usage
        reported_input_tokens = _usage_count(usage, "input_tokens", "prompt_tokens")
        reported_output_tokens = _usage_count(usage, "output_tokens", "completion_tokens")
        input_tokens = metadata.input_tokens if reported_input_tokens is None else reported_input_tokens
        output_tokens = 0 if reported_output_tokens is None else reported_output_tokens
        actual_cost = (input_tokens + output_tokens) * self.cost_per_million_tokens / 1_000_000.0
        # Direct streaming observers intentionally receive no assembled response.  The transport
        # boundary still establishes success when the iterator closes without an exception.
        success = error is None
        status = "completed" if success else "provider_refused"
        outcome = "success" if success else "failure"
        failure_class: str | None = None
        status_code: int | None = None
        if isinstance(error, ProviderError):
            failure_class = "circuit_open" if error.circuit_open else "provider_error"
            status_code = error.status_code
        elif error is not None:
            failure_class = "provider_error"
        request_id_digest = None
        if response is not None and response.request_id is not None:
            request_id_digest = content_digest(response.request_id)
        outcome_digest = content_digest(
            {
                "provider": self.provider,
                "model": self.model,
                "kind": metadata.kind,
                "attempt": self.attempt,
                "turn": turn,
                "status": status,
                "outcome": outcome,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "status_code": status_code,
                "failure_class": failure_class,
                "request_id_digest": request_id_digest,
            }
        )
        self.controller.record_provider_outcome(
            provider=self.provider,
            model=self.model,
            invocation_kind=metadata.kind,
            attempt=self.attempt,
            turn=turn,
            status=status,
            outcome=outcome,
            latency_ms=latency,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            estimated_cost_units=estimated_cost,
            actual_cost_units=actual_cost,
            selection_digest=self.selection_digest,
            outcome_digest=outcome_digest,
            request_id_digest=request_id_digest,
            failure_class=failure_class,
            status_code=status_code,
        )
        self._receipts.append(
            AutonomousProviderInvocationReceipt(
                execution_id=self.controller.state.execution_id,
                provider=self.provider,
                model=self.model,
                kind=metadata.kind,
                attempt=self.attempt,
                turn=turn,
                status=status,
                outcome=outcome,
                input_tokens=input_tokens,
                output_tokens=output_tokens,
                estimated_cost_units=estimated_cost,
                actual_cost_units=actual_cost,
                latency_ms=latency,
                selection_digest=self.selection_digest,
                outcome_digest=outcome_digest,
                request_id_digest=request_id_digest,
                failure_class=failure_class,
                status_code=status_code,
            )
        )

    def evidence(self) -> list[dict[str, Any]]:
        """Return bounded provider evidence suitable for an explicit evaluator input."""

        return [receipt.to_dict() for receipt in self._receipts]


__all__ = [
    "AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA",
    "AutonomousProviderInvocationError",
    "AutonomousProviderInvocationReceipt",
    "AutonomousProviderInvocationSession",
]
