"""Provider-neutral live streams for already-admitted autonomous model arms.

The Rust brain and :class:`~prism_sdk.brain.AutonomousBrain` remain the authority for task
selection and effect approval.  This module owns the small gap between that decision and a UI
or worker consuming provider deltas: it binds the selected arm to a bounded fallback ladder,
applies context compaction once, and records only metadata after the stream ends.

The stream itself is transient.  Completion receipts never contain prompt messages, text
deltas, tool arguments, credentials, or provider payloads.  A fallback is allowed only when a
provider fails before the first event is observed; replaying after a partial event could produce
two assistant answers or duplicate a caller-visible tool intent.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import hashlib
import json
import time
from typing import Any, Callable, Iterable, Iterator, Mapping, Sequence

from .autonomous_context_budget import (
    AutonomousContextBudgetOptions,
    compact_autonomous_provider_request,
)
from .llm_runtime import (
    CredentialHandle,
    LLMRuntime,
    ProviderError,
    ProviderInvocationObserver,
    ProviderRequest,
    ProviderStreamEvent,
)


AUTONOMOUS_STREAM_COMPLETION_SCHEMA = "bioprism-python-autonomous-stream-completion/0.1"
AUTONOMOUS_STREAM_CONTINUATION_SCHEMA = "bioprism-python-autonomous-stream-continuation/0.1"
MAX_AUTONOMOUS_STREAM_FAILOVERS = 8
MAX_AUTONOMOUS_STREAM_STEPS = MAX_AUTONOMOUS_STREAM_FAILOVERS + 1
_IDENTIFIER_LIMIT = 512


def _digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _identifier(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value.encode("utf-8")) > _IDENTIFIER_LIMIT
        or any(ord(character) < 32 for character in value)
    ):
        raise ProviderError(f"{label} is outside its bounded identifier contract")
    return value


def _non_negative_metric(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value < 0:
        raise ProviderError(f"{label} must be a non-negative number")
    return float(value)


@dataclass(frozen=True, slots=True)
class AutonomousStreamArm:
    """One caller-ranked provider/model arm in a stream continuation ladder.

    ``credential`` is an in-memory capability and is intentionally not serializable.  For
    multi-provider ladders, prefer ``credential_for`` on :meth:`AutonomousStreamRuntime.open` so
    the caller can resolve each handle at the moment its arm is attempted.
    """

    provider: str
    model: str
    credential: CredentialHandle | None = None
    cost_per_million_tokens: float = 0.0

    def __post_init__(self) -> None:
        _identifier(self.provider, "stream arm provider")
        _identifier(self.model, "stream arm model")
        _non_negative_metric(self.cost_per_million_tokens, "stream arm cost")
        if self.credential is not None and not isinstance(self.credential, CredentialHandle):
            raise ProviderError("stream arm credential must be a CredentialHandle")

    @property
    def arm_id(self) -> str:
        return f"{self.provider}/{self.model}"

    def to_metadata(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "model": self.model,
            "cost_per_million_tokens": self.cost_per_million_tokens,
            "credential_posture": "caller_supplied_handle_not_serialized",
        }


@dataclass(frozen=True, slots=True)
class AutonomousStreamCompletion:
    """Metadata-only terminal state for one autonomous stream."""

    status: str
    event_count: int
    text_delta_bytes: int
    done_seen: bool
    provider_invocations: tuple[Mapping[str, Any], ...]
    provider_failover: Mapping[str, Any] | None
    error_code: str | None
    error_class: str | None
    effect_ids: tuple[str, ...] = ()
    schema: str = AUTONOMOUS_STREAM_COMPLETION_SCHEMA
    retention: str = "metadata_only_no_stream_payloads_or_credentials"
    secret_material: str = "never_returned"

    def __post_init__(self) -> None:
        if not isinstance(self.effect_ids, tuple) or len(self.effect_ids) > 32:
            raise ProviderError("autonomous stream effect_ids must be a bounded tuple")
        for effect_id in self.effect_ids:
            if not isinstance(effect_id, str) or len(effect_id) != 64 or any(
                character not in "0123456789abcdef" for character in effect_id
            ):
                raise ProviderError("autonomous stream effect_ids must be lowercase SHA-256 digests")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "event_count": self.event_count,
            "text_delta_bytes": self.text_delta_bytes,
            "done_seen": self.done_seen,
            "provider_invocations": [dict(item) for item in self.provider_invocations],
            "provider_failover": None if self.provider_failover is None else dict(self.provider_failover),
            "error_code": self.error_code,
            "error_class": self.error_class,
            "effect_ids": list(self.effect_ids),
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


class AutonomousStreamHandle:
    """Single-consumer stream handle returned by :class:`AutonomousStreamRuntime`."""

    __slots__ = (
        "selection",
        "continuation_plan",
        "context_budget",
        "_runtime",
        "_request",
        "_arms",
        "_credential",
        "_credential_for",
        "_max_provider_failovers",
        "_observer",
        "_invocation_kind",
        "_effect_boundary",
        "_effect_execution",
        "_provider_quota",
        "_estimated_input_tokens",
        "_started",
        "_completion",
        "_event_count",
        "_text_delta_bytes",
        "_done_seen",
        "_attempts",
        "_effect_ids",
    )

    def __init__(
        self,
        runtime: LLMRuntime,
        request: ProviderRequest,
        arms: tuple[AutonomousStreamArm, ...],
        *,
        selection: Mapping[str, Any],
        continuation_plan: Mapping[str, Any],
        context_budget: Mapping[str, Any] | None,
        credential: CredentialHandle | None,
        credential_for: Callable[[str], CredentialHandle | None] | None,
        max_provider_failovers: int,
        observer: ProviderInvocationObserver | None,
        invocation_kind: str,
        effect_boundary: Any | None,
        effect_execution: Any | None,
        provider_quota: Any | None,
    ) -> None:
        self.selection = dict(selection)
        self.continuation_plan = dict(continuation_plan)
        self.context_budget = None if context_budget is None else dict(context_budget)
        self._runtime = runtime
        self._request = request
        self._arms = arms
        self._credential = credential
        self._credential_for = credential_for
        self._max_provider_failovers = max_provider_failovers
        self._observer = observer
        self._invocation_kind = invocation_kind
        self._effect_boundary = effect_boundary
        self._effect_execution = effect_execution
        self._provider_quota = provider_quota
        self._estimated_input_tokens = max(
            1,
            sum(len(str(message.get("content", "")).encode("utf-8")) for message in request.messages) // 4,
        )
        self._started = False
        self._completion: AutonomousStreamCompletion | None = None
        self._event_count = 0
        self._text_delta_bytes = 0
        self._done_seen = False
        self._attempts: list[dict[str, Any]] = []
        self._effect_ids: list[str] = []

    @property
    def completion(self) -> AutonomousStreamCompletion | None:
        """Return terminal metadata, or ``None`` while the stream is still live."""

        return self._completion

    @property
    def events(self) -> Iterator[ProviderStreamEvent]:
        """Open the transient event iterator once.

        Calling this property twice is refused so two consumers cannot race the same provider
        stream or accidentally make completion accounting non-deterministic.
        """

        if self._started:
            raise ProviderError("autonomous stream handles are single-consumer")
        self._started = True
        return self._iterate()

    def _finish(self, status: str, error: BaseException | None = None) -> None:
        if self._completion is not None:
            return
        error_code = getattr(error, "code", None) if error is not None else None
        if not isinstance(error_code, str):
            error_code = None
        error_class = type(error).__name__ if error is not None else None
        invocation_rows = tuple(dict(row) for row in self._attempts)
        failover = None
        if len(invocation_rows) > 1:
            failover_attempts = [
                {
                    "attempt": row["attempt"],
                    "provider": row["provider"],
                    "model": row["model"],
                    "outcome": row["outcome"],
                    "error_code": row["error_code"],
                    "event_count": row["event_count"],
                }
                for row in invocation_rows
            ]
            failover = {
                "schema": "bioprism-python-autonomous-provider-failover/0.1",
                "strategy": "pre_event_stream_failover_only",
                "attempts": failover_attempts,
                "fallback_count": len(invocation_rows) - 1,
                "failover_digest": _digest(failover_attempts),
                "continuation_plan_digest": self.continuation_plan["plan_digest"],
                "retention": "metadata_only",
                "secret_material": "never_returned",
            }
        self._completion = AutonomousStreamCompletion(
            status=status,
            event_count=self._event_count,
            text_delta_bytes=self._text_delta_bytes,
            done_seen=self._done_seen,
            provider_invocations=invocation_rows,
            provider_failover=failover,
            error_code=error_code,
            error_class=error_class,
            effect_ids=tuple(self._effect_ids),
        )

    def _iterate(self) -> Iterator[ProviderStreamEvent]:
        try:
            for index, arm in enumerate(self._arms):
                if index > self._max_provider_failovers:
                    break
                attempt = {
                    "provider": arm.provider,
                    "model": arm.model,
                    "attempt": index,
                    "status": "running",
                    "outcome": "failure",
                    "event_count": 0,
                    "text_delta_bytes": 0,
                    "done_seen": False,
                    "error_code": None,
                    "effect_id": None,
                    "request_id_present": False,
                    "estimated_input_tokens": self._estimated_input_tokens,
                    "estimated_output_tokens": self._request.max_output_tokens,
                    "estimated_cost_units": (
                        (self._estimated_input_tokens + self._request.max_output_tokens) / 1_000_000
                    )
                    * arm.cost_per_million_tokens,
                }
                self._attempts.append(attempt)
                credential = arm.credential
                if credential is None:
                    credential = self._credential if index == 0 else (
                        self._credential_for(arm.provider) if self._credential_for is not None else None
                    )
                started = time.perf_counter()
                def observe_effect(effect_id: str) -> None:
                    if effect_id not in self._effect_ids:
                        self._effect_ids.append(effect_id)
                    attempt["effect_id"] = effect_id
                try:
                    for event in self._runtime.invoke_stream(
                        arm.provider,
                        replace(self._request, model=arm.model),
                        credential=credential,
                        invocation_observer=self._observer,
                        invocation_kind=self._invocation_kind,
                        effect_boundary=self._effect_boundary,
                        effect_execution=self._effect_execution,
                        effect_id_observer=observe_effect,
                        provider_quota=self._provider_quota,
                        estimated_cost_units=attempt["estimated_cost_units"],
                    ):
                        attempt["event_count"] += 1
                        attempt["text_delta_bytes"] += len(event.text_delta.encode("utf-8"))
                        attempt["done_seen"] = bool(attempt["done_seen"] or event.done)
                        attempt["request_id_present"] = bool(attempt["request_id_present"] or event.request_id)
                        self._event_count += 1
                        self._text_delta_bytes += len(event.text_delta.encode("utf-8"))
                        self._done_seen = bool(self._done_seen or event.done)
                        yield event
                    if not attempt["done_seen"]:
                        raise ProviderError(
                            "autonomous provider stream ended without a done event",
                            retryable=attempt["event_count"] == 0,
                            code="invalid_response",
                        )
                    attempt["status"] = "completed"
                    attempt["outcome"] = "success"
                    attempt["latency_ms"] = max(0.0, (time.perf_counter() - started) * 1000.0)
                    self._finish("completed")
                    return
                except GeneratorExit:
                    attempt["status"] = "abandoned"
                    attempt["latency_ms"] = max(0.0, (time.perf_counter() - started) * 1000.0)
                    raise
                except BaseException as error:
                    attempt["status"] = "failed"
                    attempt["error_code"] = getattr(error, "code", None)
                    attempt["latency_ms"] = max(0.0, (time.perf_counter() - started) * 1000.0)
                    attempt["outcome_digest"] = _digest({
                        "provider": arm.provider,
                        "model": arm.model,
                        "attempt": index,
                        "event_count": attempt["event_count"],
                        "error_code": attempt["error_code"],
                    })
                    if (
                        attempt["event_count"] == 0
                        and isinstance(error, ProviderError)
                        and error.retryable
                        and index < self._max_provider_failovers
                    ):
                        continue
                    self._finish("failed", error)
                    raise
            error = ProviderError("autonomous stream continuation ladder was exhausted")
            self._finish("failed", error)
            raise error
        finally:
            if self._completion is None:
                self._finish("abandoned")


class AutonomousStreamRuntime:
    """Open streams for a caller-ranked autonomous provider/model ladder."""

    def __init__(self, runtime: LLMRuntime) -> None:
        if not isinstance(runtime, LLMRuntime):
            raise ProviderError("AutonomousStreamRuntime requires an LLMRuntime")
        self.runtime = runtime

    @staticmethod
    def _normalize_arm(value: AutonomousStreamArm | Mapping[str, Any] | Sequence[str]) -> AutonomousStreamArm:
        if isinstance(value, AutonomousStreamArm):
            return value
        if isinstance(value, Mapping):
            return AutonomousStreamArm(
                provider=_identifier(value.get("provider"), "stream arm provider"),
                model=_identifier(value.get("model"), "stream arm model"),
                cost_per_million_tokens=_non_negative_metric(
                    value.get("cost_per_million_tokens", 0.0), "stream arm cost"
                ),
            )
        if isinstance(value, Sequence) and not isinstance(value, (str, bytes)) and len(value) == 2:
            return AutonomousStreamArm(
                provider=_identifier(value[0], "stream arm provider"),
                model=_identifier(value[1], "stream arm model"),
            )
        raise ProviderError("stream arm must be AutonomousStreamArm, mapping, or (provider, model)")

    def open(
        self,
        request: ProviderRequest,
        *,
        provider: str,
        model: str,
        fallbacks: Sequence[AutonomousStreamArm | Mapping[str, Any] | Sequence[str]] = (),
        credential: CredentialHandle | None = None,
        credential_for: Callable[[str], CredentialHandle | None] | None = None,
        max_provider_failovers: int = 0,
        context_budget: AutonomousContextBudgetOptions | Mapping[str, Any] | None = None,
        observer: ProviderInvocationObserver | None = None,
        invocation_kind: str = "autonomous_selected_model_stream",
        effect_boundary: Any | None = None,
        effect_execution: Any | None = None,
        provider_quota: Any | None = None,
        selection: Mapping[str, Any] | None = None,
    ) -> AutonomousStreamHandle:
        if not isinstance(request, ProviderRequest):
            raise ProviderError("autonomous stream request must be a ProviderRequest")
        if (
            not isinstance(max_provider_failovers, int)
            or isinstance(max_provider_failovers, bool)
            or not 0 <= max_provider_failovers <= MAX_AUTONOMOUS_STREAM_FAILOVERS
        ):
            raise ProviderError(
                f"max_provider_failovers must be within [0, {MAX_AUTONOMOUS_STREAM_FAILOVERS}]"
            )
        if credential_for is not None and not callable(credential_for):
            raise ProviderError("credential_for must be callable or None")
        first = AutonomousStreamArm(
            provider=_identifier(provider, "stream provider"),
            model=_identifier(model, "stream model"),
            credential=credential,
        )
        arms = [first]
        seen = {first.arm_id}
        for raw in fallbacks:
            arm = self._normalize_arm(raw)
            if arm.arm_id in seen:
                raise ProviderError(f"stream continuation contains duplicate arm {arm.arm_id}")
            if len(arms) >= MAX_AUTONOMOUS_STREAM_STEPS:
                raise ProviderError("stream continuation exceeds its bounded arm count")
            seen.add(arm.arm_id)
            arms.append(arm)

        compacted_request = request
        context_projection: Mapping[str, Any] | None = None
        if context_budget is not None:
            compacted = compact_autonomous_provider_request(request, context_budget)
            compacted_request = compacted.request
            context_projection = compacted.plan.to_dict()
        normalized_selection = {
            "selected_model": {"provider": first.provider, "model": first.model},
            "strategy": (
                _identifier(selection.get("strategy"), "stream selection strategy")
                if isinstance(selection, Mapping) and isinstance(selection.get("strategy"), str)
                else "caller_ranked_arm_order"
            ),
            "selection_digest": _digest({"provider": first.provider, "model": first.model}),
            "credential_posture": "caller_supplied_handle_not_serialized",
        }
        steps = [
            {
                "order": index,
                "provider": arm.provider,
                "model": arm.model,
                "model_id": arm.arm_id,
                "arm_digest": _digest(arm.to_metadata()),
            }
            for index, arm in enumerate(arms)
        ]
        continuation_body = {
            "schema": AUTONOMOUS_STREAM_CONTINUATION_SCHEMA,
            "strategy": "caller_ranked_fixed_arm_order",
            "max_failovers": max_provider_failovers,
            "steps": steps,
            "retention": "selection_metadata_only_no_task_prompt_provider_payloads",
            "secret_material": "never_returned",
        }
        continuation_plan = {
            **continuation_body,
            "plan_digest": _digest(continuation_body),
        }
        return AutonomousStreamHandle(
            self.runtime,
            compacted_request,
            tuple(arms),
            selection=normalized_selection,
            continuation_plan=continuation_plan,
            context_budget=context_projection,
            credential=credential,
            credential_for=credential_for,
            max_provider_failovers=max_provider_failovers,
            observer=observer,
            invocation_kind=invocation_kind,
            effect_boundary=effect_boundary,
            effect_execution=effect_execution,
            provider_quota=provider_quota,
        )


__all__ = [
    "AUTONOMOUS_STREAM_COMPLETION_SCHEMA",
    "AUTONOMOUS_STREAM_CONTINUATION_SCHEMA",
    "MAX_AUTONOMOUS_STREAM_FAILOVERS",
    "MAX_AUTONOMOUS_STREAM_STEPS",
    "AutonomousStreamArm",
    "AutonomousStreamCompletion",
    "AutonomousStreamHandle",
    "AutonomousStreamRuntime",
]
