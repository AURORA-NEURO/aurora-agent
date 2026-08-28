"""High-level autonomous-agent streaming envelopes.

The low-level :mod:`prism_sdk.autonomous_stream` module already knows how to invoke one
caller-ranked provider/model ladder.  This module adds the application boundary around it:
the agent first compiles and audits a normal domain blueprint without provider approval, then
hands the exact selected arm and transient provider request to the stream runtime only after
the caller opts in.  Completion objects retain metadata and digests, never prompt text,
response text, tool arguments, credential handles, or provider payloads.

This separation is intentional.  A stream is a live delivery mechanism, not a second model
selector, evaluator, memory writer, or authorization system.  The normal autonomous planner
therefore remains the only source of route, prompt, plan, and model-selection decisions.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from typing import Any, Iterator, Mapping, Sequence

from .autonomous_stream import (
    AutonomousStreamArm,
    AutonomousStreamCompletion,
    AutonomousStreamHandle,
    AutonomousStreamRuntime,
)
from .authoring import content_digest
from .llm_runtime import ProviderError, ProviderRequest, ProviderStreamEvent


AUTONOMOUS_AGENT_STREAM_SCHEMA = "bioprism-python-autonomous-agent-stream/0.1"
AUTONOMOUS_AGENT_STREAM_COMPLETION_SCHEMA = "bioprism-python-autonomous-agent-stream-completion/0.1"
MAX_AUTONOMOUS_AGENT_STREAM_TEXT_BYTES = 48_000


def _digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _bounded_digest(value: Any, label: str) -> str:
    digest = value if isinstance(value, str) else _digest(value)
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        raise ProviderError(f"{label} must be a lowercase SHA-256 digest")
    return digest


@dataclass(frozen=True, slots=True)
class AutonomousAgentStreamEvent:
    """One transient high-level event.

    ``event`` is present only for provider events and intentionally remains transient.  The
    event object is useful to a UI or caller-owned transcript, but it is never copied into a
    completion receipt or a persistence projection by this SDK.
    """

    kind: str
    stage: str
    event: ProviderStreamEvent | None = None
    child_id: str | None = None
    phase: str | None = None
    domain: str | None = None
    status: str | None = None
    event_count: int | None = None
    text_delta_bytes: int | None = None

    def __post_init__(self) -> None:
        if self.kind not in {"provider", "lifecycle"}:
            raise ProviderError("autonomous agent stream event kind is invalid")
        if self.stage not in {"route", "direct", "child", "synthesis"}:
            raise ProviderError("autonomous agent stream event stage is invalid")
        if self.kind == "provider" and not isinstance(self.event, ProviderStreamEvent):
            raise ProviderError("provider stream events require a ProviderStreamEvent")
        if self.kind == "lifecycle" and self.event is not None:
            raise ProviderError("lifecycle stream events cannot contain provider payloads")
        if self.child_id is not None and (
            not isinstance(self.child_id, str) or not self.child_id.strip() or len(self.child_id) > 512
        ):
            raise ProviderError("autonomous agent stream child_id is outside its bound")
        if self.event_count is not None and (
            isinstance(self.event_count, bool) or not isinstance(self.event_count, int) or self.event_count < 0
        ):
            raise ProviderError("autonomous agent stream event_count must be non-negative")
        if self.text_delta_bytes is not None and (
            isinstance(self.text_delta_bytes, bool)
            or not isinstance(self.text_delta_bytes, int)
            or self.text_delta_bytes < 0
        ):
            raise ProviderError("autonomous agent stream text_delta_bytes must be non-negative")

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": AUTONOMOUS_AGENT_STREAM_SCHEMA,
            "kind": self.kind,
            "stage": self.stage,
            "child_id": self.child_id,
            "phase": self.phase,
            "domain": self.domain,
            "status": self.status,
            "event_count": self.event_count,
            "text_delta_bytes": self.text_delta_bytes,
        }
        if self.event is not None:
            result["event"] = self.event.to_dict()
        return result


@dataclass(frozen=True, slots=True)
class AutonomousAgentStreamCompletion:
    """Metadata-only terminal receipt for a high-level stream."""

    status: str
    task_digest: str
    blueprint_digest: str | None
    event_count: int
    text_delta_bytes: int
    stage_count: int
    provider_invocations: tuple[Mapping[str, Any], ...] = ()
    provider_failover: Mapping[str, Any] | None = None
    inner_completions: tuple[Mapping[str, Any], ...] = ()
    error_code: str | None = None
    error_class: str | None = None
    schema: str = AUTONOMOUS_AGENT_STREAM_COMPLETION_SCHEMA
    retention: str = "metadata_only_no_stream_payloads_or_credentials"
    secret_material: str = "never_returned"

    def __post_init__(self) -> None:
        if self.status not in {
            "approval_required",
            "route_review_required",
            "plan_refused",
            "selection_refused",
            "completed",
            "failed",
            "abandoned",
        }:
            raise ProviderError("autonomous agent stream completion status is invalid")
        _bounded_digest(self.task_digest, "autonomous agent stream task_digest")
        if self.blueprint_digest is not None:
            _bounded_digest(self.blueprint_digest, "autonomous agent stream blueprint_digest")
        for label, value in (
            ("event_count", self.event_count),
            ("text_delta_bytes", self.text_delta_bytes),
            ("stage_count", self.stage_count),
        ):
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise ProviderError(f"autonomous agent stream {label} must be non-negative")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "task_digest": self.task_digest,
            "blueprint_digest": self.blueprint_digest,
            "event_count": self.event_count,
            "text_delta_bytes": self.text_delta_bytes,
            "stage_count": self.stage_count,
            "provider_invocations": [dict(item) for item in self.provider_invocations],
            "provider_failover": None if self.provider_failover is None else dict(self.provider_failover),
            "inner_completions": [dict(item) for item in self.inner_completions],
            "error_code": self.error_code,
            "error_class": self.error_class,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


class AutonomousAgentStreamHandle:
    """Single-consumer high-level stream handle.

    The handle is deliberately synchronous because the Python provider contract is an iterator.
    Provider invocation starts only when ``events`` is consumed, which lets a caller inspect the
    selected metadata and still abandon a stream before any network work occurs.
    """

    __slots__ = (
        "selection",
        "route",
        "blueprint",
        "_inner",
        "_stage",
        "_task_digest",
        "_blueprint_digest",
        "_completion",
        "_started",
        "_event_count",
        "_text_delta_bytes",
    )

    def __init__(
        self,
        *,
        selection: Mapping[str, Any],
        route: Mapping[str, Any] | None,
        blueprint: Mapping[str, Any] | None,
        task_digest: str,
        blueprint_digest: str | None,
        inner: AutonomousStreamHandle | None,
        initial_status: str | None = None,
        stage: str = "direct",
    ) -> None:
        if not isinstance(selection, Mapping):
            raise ProviderError("autonomous agent stream selection must be a mapping")
        if route is not None and not isinstance(route, Mapping):
            raise ProviderError("autonomous agent stream route must be a mapping or None")
        if blueprint is not None and not isinstance(blueprint, Mapping):
            raise ProviderError("autonomous agent stream blueprint must be a mapping or None")
        if stage not in {"direct", "child", "synthesis"}:
            raise ProviderError("autonomous agent stream stage is invalid")
        self.selection = dict(selection)
        self.route = None if route is None else dict(route)
        self.blueprint = None if blueprint is None else dict(blueprint)
        self._inner = inner
        self._stage = stage
        self._task_digest = _bounded_digest(task_digest, "autonomous agent stream task_digest")
        self._blueprint_digest = (
            None if blueprint_digest is None else _bounded_digest(blueprint_digest, "autonomous agent stream blueprint_digest")
        )
        self._completion: AutonomousAgentStreamCompletion | None = None
        self._started = False
        self._event_count = 0
        self._text_delta_bytes = 0
        if initial_status is not None:
            self._finish(initial_status)

    @property
    def completion(self) -> AutonomousAgentStreamCompletion | None:
        return self._completion

    @property
    def events(self) -> Iterator[AutonomousAgentStreamEvent]:
        """Consume transient high-level events once."""

        if self._started:
            raise ProviderError("autonomous agent stream handles are single-consumer")
        self._started = True
        return self._iterate()

    def _finish(self, status: str, error: BaseException | None = None) -> None:
        if self._completion is not None:
            return
        inner_completion = None if self._inner is None else self._inner.completion
        provider_invocations: tuple[Mapping[str, Any], ...] = ()
        provider_failover: Mapping[str, Any] | None = None
        inner_completions: tuple[Mapping[str, Any], ...] = ()
        if isinstance(inner_completion, AutonomousStreamCompletion):
            inner_dict = inner_completion.to_dict()
            inner_completions = (inner_dict,)
            provider_invocations = tuple(dict(item) for item in inner_completion.provider_invocations)
            provider_failover = (
                None if inner_completion.provider_failover is None else dict(inner_completion.provider_failover)
            )
        error_code = getattr(error, "code", None) if error is not None else None
        if not isinstance(error_code, str):
            error_code = None
        self._completion = AutonomousAgentStreamCompletion(
            status=status,
            task_digest=self._task_digest,
            blueprint_digest=self._blueprint_digest,
            event_count=self._event_count,
            text_delta_bytes=self._text_delta_bytes,
            stage_count=1 if self._inner is not None else 0,
            provider_invocations=provider_invocations,
            provider_failover=provider_failover,
            inner_completions=inner_completions,
            error_code=error_code or (
                inner_completion.error_code if isinstance(inner_completion, AutonomousStreamCompletion) else None
            ),
            error_class=(type(error).__name__ if error is not None else (
                inner_completion.error_class if isinstance(inner_completion, AutonomousStreamCompletion) else None
            )),
        )

    def _finish_from_inner(self) -> None:
        inner_completion = None if self._inner is None else self._inner.completion
        if not isinstance(inner_completion, AutonomousStreamCompletion):
            self._finish("abandoned")
            return
        status = {
            "completed": "completed",
            "abandoned": "abandoned",
            "failed": "failed",
        }.get(inner_completion.status, "failed")
        self._finish(status)

    def _iterate(self) -> Iterator[AutonomousAgentStreamEvent]:
        if self._inner is None:
            self._finish(self._completion.status if self._completion is not None else "abandoned")
            return
        iterator = self._inner.events
        try:
            for event in iterator:
                delta_bytes = len(event.text_delta.encode("utf-8"))
                if self._text_delta_bytes + delta_bytes > MAX_AUTONOMOUS_AGENT_STREAM_TEXT_BYTES:
                    raise ProviderError(
                        "autonomous agent stream output exceeds its bounded transient buffer",
                        code="invalid_response",
                    )
                self._event_count += 1
                self._text_delta_bytes += delta_bytes
                yield AutonomousAgentStreamEvent(kind="provider", stage=self._stage, event=event)
            self._finish_from_inner()
        except GeneratorExit:
            close = getattr(iterator, "close", None)
            if callable(close):
                close()
            self._finish("abandoned")
            raise
        except BaseException as error:
            close = getattr(iterator, "close", None)
            if callable(close):
                close()
            self._finish("failed", error)
            raise
        finally:
            if self._completion is None:
                self._finish("abandoned")


def build_autonomous_agent_stream_request(
    *,
    model: str,
    messages: Sequence[Mapping[str, Any]],
    max_output_tokens: int,
    temperature: float | None,
    require_json: bool,
    response_schema: Mapping[str, Any] | None,
    idempotency_key: str | None,
    tools: Sequence[Any],
    tool_choice: str | None,
) -> ProviderRequest:
    """Build the transient provider request after high-level preflight.

    Keeping this constructor public makes adapters and tests able to verify request assembly
    without reaching into the autonomous agent implementation.  It does not serialize the
    resulting messages; callers retain the request only for the duration of the stream.
    """

    return ProviderRequest(
        model=model,
        messages=tuple(dict(message) for message in messages),
        max_output_tokens=max_output_tokens,
        temperature=temperature,
        require_json=require_json,
        response_schema=None if response_schema is None else dict(response_schema),
        idempotency_key=idempotency_key,
        tools=tuple(tools),
        tool_choice=tool_choice,
    )


__all__ = [
    "AUTONOMOUS_AGENT_STREAM_SCHEMA",
    "AUTONOMOUS_AGENT_STREAM_COMPLETION_SCHEMA",
    "MAX_AUTONOMOUS_AGENT_STREAM_TEXT_BYTES",
    "AutonomousAgentStreamEvent",
    "AutonomousAgentStreamCompletion",
    "AutonomousAgentStreamHandle",
    "build_autonomous_agent_stream_request",
]
