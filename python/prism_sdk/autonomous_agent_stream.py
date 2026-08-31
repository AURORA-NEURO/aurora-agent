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

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import hashlib
import json
from queue import Empty, Full, Queue
from threading import Event, Lock, Thread
from typing import Any, Callable, Iterator, Mapping, Sequence

from .autonomous_stream import (
    AutonomousStreamArm,
    AutonomousStreamCompletion,
    AutonomousStreamHandle,
    AutonomousStreamRuntime,
)
from .authoring import content_digest
from .llm_runtime import CredentialError, ProviderError, ProviderRequest, ProviderStreamEvent


AUTONOMOUS_AGENT_STREAM_SCHEMA = "bioprism-python-autonomous-agent-stream/0.1"
AUTONOMOUS_AGENT_STREAM_COMPLETION_SCHEMA = "bioprism-python-autonomous-agent-stream-completion/0.1"
MAX_AUTONOMOUS_AGENT_STREAM_TEXT_BYTES = 48_000
MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILDREN = 8
MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_QUEUED_EVENTS = 256
MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILD_OUTPUT_BYTES = 32_000


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
    selection_digest: str | None = None
    error_code: str | None = None

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
        if self.selection_digest is not None:
            _bounded_digest(self.selection_digest, "autonomous agent stream selection_digest")
        if self.error_code is not None and (
            not isinstance(self.error_code, str) or not self.error_code.strip() or len(self.error_code) > 256
        ):
            raise ProviderError("autonomous agent stream error_code is outside its bound")

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
            "selection_digest": self.selection_digest,
            "error_code": self.error_code,
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
    stage_records: tuple[Mapping[str, Any], ...] = ()
    error_code: str | None = None
    error_class: str | None = None
    effect_ids: tuple[str, ...] = ()
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
            "children_completed",
            "children_partial",
            "child_failed",
            "child_incomplete",
            "response_review_required",
            "reconciliation_required",
            "provider_failed",
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
        if not isinstance(self.effect_ids, tuple) or len(self.effect_ids) > 32:
            raise ProviderError("autonomous agent stream effect_ids must be a bounded tuple")
        for effect_id in self.effect_ids:
            _bounded_digest(effect_id, "autonomous agent stream effect_id")

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
            "stage_records": [dict(item) for item in self.stage_records],
            "error_code": self.error_code,
            "error_class": self.error_class,
            "effect_ids": list(self.effect_ids),
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
        "_iterator",
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
        self._iterator: Iterator[AutonomousAgentStreamEvent] | None = None
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
        self._iterator = self._iterate()
        return self._iterator

    def _finish(self, status: str, error: BaseException | None = None) -> None:
        if self._completion is not None:
            return
        inner_completion = None if self._inner is None else self._inner.completion
        provider_invocations: tuple[Mapping[str, Any], ...] = ()
        provider_failover: Mapping[str, Any] | None = None
        inner_completions: tuple[Mapping[str, Any], ...] = ()
        effect_ids: tuple[str, ...] = ()
        if isinstance(inner_completion, AutonomousStreamCompletion):
            inner_dict = inner_completion.to_dict()
            inner_completions = (inner_dict,)
            provider_invocations = tuple(dict(item) for item in inner_completion.provider_invocations)
            provider_failover = (
                None if inner_completion.provider_failover is None else dict(inner_completion.provider_failover)
            )
            effect_ids = tuple(inner_completion.effect_ids)
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
            effect_ids=effect_ids,
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

    def close(self) -> None:
        """Close an unconsumed or partially consumed stream without replaying it."""

        if self._inner is None:
            self._finish(self._completion.status if self._completion is not None else "abandoned")
            return
        iterator = self._iterator
        if iterator is not None and self._started:
            close = getattr(iterator, "close", None)
            if callable(close):
                close()
        if self._completion is None:
            self._finish("abandoned")


class _AutonomousCrossDomainStreamAborted(Exception):
    """Internal cancellation signal used to unwind provider iterators promptly."""


class AutonomousCrossDomainStreamHandle:
    """Lazy, bounded fan-out/fan-in stream for an approved cross-domain blueprint.

    Child providers run in a bounded worker pool only after the caller consumes ``events``.
    Provider text is delivered transiently and retained only in bounded local buffers long
    enough to form the synthesis request.  The completion receipt stores stage metadata and
    child completion receipts, never child or synthesis text.
    """

    __slots__ = (
        "selection",
        "route",
        "blueprint",
        "_task_digest",
        "_blueprint_digest",
        "_child_specs",
        "_synthesis_spec",
        "_open_stream",
        "_model_candidates",
        "_credentials",
        "_base_options",
        "_synthesize",
        "_allow_partial",
        "_max_parallelism",
        "_queue",
        "_stop",
        "_started",
        "_worker",
        "_completion",
        "_completion_lock",
        "_event_count",
        "_text_delta_bytes",
        "_stage_records",
        "_inner_completions",
        "_provider_invocations",
    )

    def __init__(
        self,
        *,
        selection: Mapping[str, Any],
        route: Mapping[str, Any] | None,
        blueprint: Mapping[str, Any],
        task_digest: str,
        blueprint_digest: str,
        child_specs: Sequence[Mapping[str, Any]],
        synthesis_spec: Mapping[str, Any],
        open_stream: Callable[..., AutonomousAgentStreamHandle],
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, Any],
        base_options: Mapping[str, Any],
        synthesize: bool,
        allow_partial: bool,
        max_parallelism: int,
        initial_status: str | None = None,
    ) -> None:
        if not isinstance(selection, Mapping):
            raise ProviderError("cross-domain stream selection must be a mapping")
        if route is not None and not isinstance(route, Mapping):
            raise ProviderError("cross-domain stream route must be a mapping or None")
        if not isinstance(blueprint, Mapping):
            raise ProviderError("cross-domain stream blueprint must be a mapping")
        if not isinstance(child_specs, Sequence) or isinstance(child_specs, (str, bytes)):
            raise ProviderError("cross-domain stream child specifications must be a sequence")
        if not child_specs and initial_status is None:
            raise ProviderError("cross-domain stream requires child specifications")
        if len(child_specs) > MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILDREN:
            raise ProviderError("cross-domain stream exceeds its bounded child count")
        if not isinstance(synthesis_spec, Mapping):
            raise ProviderError("cross-domain stream synthesis specification must be a mapping")
        if not callable(open_stream):
            raise ProviderError("cross-domain stream open_stream must be callable")
        if not isinstance(synthesize, bool) or not isinstance(allow_partial, bool):
            raise ProviderError("cross-domain stream synthesis flags must be booleans")
        if (
            isinstance(max_parallelism, bool)
            or not isinstance(max_parallelism, int)
            or not 1 <= max_parallelism <= MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILDREN
        ):
            raise ProviderError("cross-domain stream max_parallelism is outside its bound")
        self.selection = dict(selection)
        self.route = None if route is None else dict(route)
        self.blueprint = dict(blueprint)
        self._task_digest = _bounded_digest(task_digest, "cross-domain stream task_digest")
        self._blueprint_digest = _bounded_digest(blueprint_digest, "cross-domain stream blueprint_digest")
        self._child_specs = tuple(dict(spec) for spec in child_specs)
        self._synthesis_spec = dict(synthesis_spec)
        self._open_stream = open_stream
        self._model_candidates = tuple(dict(candidate) for candidate in model_candidates)
        self._credentials = credentials
        self._base_options = dict(base_options)
        self._synthesize = synthesize
        self._allow_partial = allow_partial
        self._max_parallelism = max_parallelism
        self._queue: Queue[object] = Queue(maxsize=MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_QUEUED_EVENTS)
        self._stop = Event()
        self._started = False
        self._worker: Thread | None = None
        self._completion: AutonomousAgentStreamCompletion | None = None
        self._completion_lock = Lock()
        self._event_count = 0
        self._text_delta_bytes = 0
        self._stage_records: list[dict[str, Any]] = []
        self._inner_completions: list[Mapping[str, Any]] = []
        self._provider_invocations: list[Mapping[str, Any]] = []
        if initial_status is not None:
            self._finish(initial_status)

    @property
    def completion(self) -> AutonomousAgentStreamCompletion | None:
        return self._completion

    @property
    def events(self) -> Iterator[AutonomousAgentStreamEvent]:
        if self._started:
            raise ProviderError("autonomous agent stream handles are single-consumer")
        self._started = True
        if self._completion is not None:
            return iter(())
        if self._completion is None:
            self._worker = Thread(
                target=self._run,
                name="aurora-cross-domain-stream",
                daemon=True,
            )
            self._worker.start()
        return self._iterate()

    def close(self) -> None:
        """Abort all child iterators and release the bounded fan-in worker."""

        self._stop.set()
        if self._completion is None and self._started:
            self._finish("abandoned")
        self._wake_consumer()

    def _wake_consumer(self) -> None:
        try:
            self._queue.put_nowait(None)
        except Full:
            # A full queue is expected under backpressure.  Never evict an event merely to
            # install a sentinel; the consumer polls completion after draining the queue.
            pass

    def _push(self, event: AutonomousAgentStreamEvent) -> None:
        while not self._stop.is_set():
            try:
                self._queue.put(event, timeout=0.1)
                return
            except Full:
                continue
        raise _AutonomousCrossDomainStreamAborted()

    def _record_provider_event(
        self,
        *,
        stage: str,
        event: ProviderStreamEvent,
        child_id: str | None,
    ) -> None:
        delta_bytes = len(event.text_delta.encode("utf-8"))
        with self._completion_lock:
            self._event_count += 1
            self._text_delta_bytes += delta_bytes
        self._push(
            AutonomousAgentStreamEvent(
                kind="provider",
                stage=stage,
                child_id=child_id,
                event=event,
            )
        )

    def _record_completion(self, handle: AutonomousAgentStreamHandle, *, stage: str, child_id: str | None, domain: str) -> AutonomousAgentStreamCompletion:
        completion = handle.completion
        if completion is None:
            completion = AutonomousAgentStreamCompletion(
                status="abandoned",
                task_digest=self._task_digest,
                blueprint_digest=None,
                event_count=0,
                text_delta_bytes=0,
                stage_count=0,
                effect_ids=(),
            )
        completion_dict = completion.to_dict()
        with self._completion_lock:
            self._inner_completions.append(completion_dict)
            self._provider_invocations.extend(completion.provider_invocations)
            self._stage_records.append({
                "stage": stage,
                "child_id": child_id,
                "domain": domain,
                "status": completion.status,
                "event_count": completion.event_count,
                "text_delta_bytes": completion.text_delta_bytes,
                "completion_digest": _digest(completion_dict),
                "selection_digest": _digest(handle.selection),
                "retention": "metadata_only",
            })
        return completion

    def _run_child(self, index: int, outputs: list[dict[str, Any] | None]) -> None:
        spec = self._child_specs[index]
        child_id = spec["id"]
        domain = spec["domain"]
        task = spec["task"]
        handle: AutonomousAgentStreamHandle | None = None
        output_parts: list[str] = []
        output_bytes = 0
        try:
            handle = self._open_stream(
                task=task,
                domain=domain,
                credentials=self._credentials,
                model_candidates=self._model_candidates,
                **self._stream_options(spec, stage="child", child_id=child_id),
            )
            self._push(AutonomousAgentStreamEvent(
                kind="lifecycle",
                stage="child",
                phase="child_started",
                child_id=child_id,
                domain=domain,
                selection_digest=_digest(handle.selection),
            ))
            for item in handle.events:
                if item.event is None:
                    continue
                delta = item.event.text_delta
                encoded = delta.encode("utf-8")
                remaining = MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILD_OUTPUT_BYTES - output_bytes
                if remaining > 0:
                    bounded = encoded[:remaining].decode("utf-8", errors="ignore")
                    if bounded:
                        output_parts.append(bounded)
                        output_bytes += len(bounded.encode("utf-8"))
                self._record_provider_event(stage="child", event=item.event, child_id=child_id)
            completion = self._record_completion(handle, stage="child", child_id=child_id, domain=domain)
            status = completion.status
            outputs[index] = {
                "id": child_id,
                "domain": domain,
                "status": status,
                "output": "".join(output_parts).strip() or "[child returned no textual output]",
            }
            self._push(AutonomousAgentStreamEvent(
                kind="lifecycle",
                stage="child",
                phase="child_completed",
                child_id=child_id,
                domain=domain,
                status=status,
                event_count=completion.event_count,
                text_delta_bytes=completion.text_delta_bytes,
                error_code=completion.error_code,
            ))
        except _AutonomousCrossDomainStreamAborted:
            if handle is not None:
                handle.close()
            raise
        except (ProviderError, CredentialError) as error:
            if handle is not None and handle.completion is not None:
                completion = self._record_completion(handle, stage="child", child_id=child_id, domain=domain)
                status = completion.status
            else:
                status = "provider_failed"
            outputs[index] = {
                "id": child_id,
                "domain": domain,
                "status": status,
                "output": "[child stream failed]",
            }
            self._push(AutonomousAgentStreamEvent(
                kind="lifecycle",
                stage="child",
                phase="child_completed",
                child_id=child_id,
                domain=domain,
                status=status,
                error_code=getattr(error, "code", None),
            ))

    def _stream_options(self, spec: Mapping[str, Any], *, stage: str, child_id: str | None) -> dict[str, Any]:
        options = dict(self._base_options)
        options.update({
            "capability": spec.get("capability"),
            "risk_class": spec.get("risk_class"),
            "constraints": tuple(spec.get("constraints", ())),
            "desired_outputs": tuple(spec.get("desired_outputs", ())),
            "context": dict(spec.get("context", {})),
            "max_steps": spec.get("max_steps", 8),
            "require_json": spec.get("require_json", False),
            "structured_domain_response": spec.get("structured_domain_response", False),
            "response_schema": None if spec.get("structured_domain_response", False) else spec.get("response_schema"),
            "execution_mode": "provider",
            "required_model_capabilities": tuple(spec.get("required_model_capabilities", ())),
            "approve_provider_call": True,
            "semantic_routing": False,
            "auto_route": False,
        })
        if child_id is not None:
            base_idempotency = options.get("idempotency_key")
            options["idempotency_key"] = None if base_idempotency is None else f"{base_idempotency}:{child_id}"
            base_run_id = options.get("run_id")
            options["run_id"] = None if base_run_id is None else f"{base_run_id}:{child_id}"
        options.pop("subtasks", None)
        options.pop("synthesize", None)
        options.pop("allow_partial", None)
        options.pop("max_parallelism", None)
        options.pop("child_execution_mode", None)
        options.pop("synthesis_execution_mode", None)
        options.pop("execution_controller", None)
        options.pop("semantic_routing", None)
        options.pop("planning_mode", None)
        options.pop("hints", None)
        options.pop("min_confidence", None)
        options.pop("min_margin", None)
        options.pop("max_domains", None)
        options.pop("allow_cross_domain", None)
        options.pop("semantic_weight", None)
        options.pop("route_override", None)
        options.pop("record_memory", None)
        options.pop("learning", None)
        options.pop("evaluator", None)
        options.pop("evidence", None)
        return options

    def _run(self) -> None:
        outputs: list[dict[str, Any] | None] = [None] * len(self._child_specs)
        try:
            with ThreadPoolExecutor(
                max_workers=min(self._max_parallelism, len(self._child_specs)),
                thread_name_prefix="aurora-cross-domain-child",
            ) as pool:
                futures = [pool.submit(self._run_child, index, outputs) for index in range(len(self._child_specs))]
                for future in futures:
                    future.result()
            completed = [row for row in outputs if row is not None and row["status"] == "completed"]
            failed = [row for row in outputs if row is not None and row["status"] != "completed"]
            if failed and (not self._allow_partial or not completed):
                self._finish(
                    "child_failed"
                    if any(row["status"] in {"failed", "provider_failed"} for row in failed)
                    else "child_incomplete"
                )
                return
            if not self._synthesize:
                self._finish("children_completed" if not failed else "children_partial")
                return
            synthesis_output = [dict(row) for row in completed]
            synthesis_spec = self._synthesis_spec
            synthesis_context = dict(synthesis_spec.get("context", {}))
            synthesis_context["cross_domain_child_outputs"] = synthesis_output
            synthesis_context["cross_domain_stream_parent_digest"] = self._task_digest
            handle = self._open_stream(
                task=synthesis_spec["task"],
                domain=synthesis_spec["domain"],
                credentials=self._credentials,
                model_candidates=self._model_candidates,
                **self._stream_options({**synthesis_spec, "context": synthesis_context}, stage="synthesis", child_id=None),
            )
            self._push(AutonomousAgentStreamEvent(
                kind="lifecycle",
                stage="synthesis",
                phase="synthesis_started",
                domain=synthesis_spec["domain"],
                selection_digest=_digest(handle.selection),
            ))
            for item in handle.events:
                if item.event is not None:
                    self._record_provider_event(stage="synthesis", event=item.event, child_id=None)
            completion = self._record_completion(handle, stage="synthesis", child_id=None, domain=synthesis_spec["domain"])
            self._push(AutonomousAgentStreamEvent(
                kind="lifecycle",
                stage="synthesis",
                phase="synthesis_completed",
                domain=synthesis_spec["domain"],
                status=completion.status,
                event_count=completion.event_count,
                text_delta_bytes=completion.text_delta_bytes,
                error_code=completion.error_code,
            ))
            self._finish("completed" if completion.status == "completed" else "failed")
        except _AutonomousCrossDomainStreamAborted:
            self._finish("abandoned")
        except (ProviderError, CredentialError) as error:
            self._finish("failed", error)
        except BaseException as error:
            self._finish("failed", error)

    def _finish(self, status: str, error: BaseException | None = None) -> None:
        if error is not None and not isinstance(error, _AutonomousCrossDomainStreamAborted):
            # Preserve the distinction between an expected metadata-only failed completion and
            # a caller-visible programming/provider error.  Queue the transient exception before
            # publishing completion so a polling consumer cannot observe a terminal receipt and
            # silently miss the error.
            while not self._stop.is_set():
                try:
                    self._queue.put(error, timeout=0.1)
                    break
                except Full:
                    continue
        with self._completion_lock:
            if self._completion is not None:
                return
            child_order = {
                str(spec.get("id")): index
                for index, spec in enumerate(self._child_specs)
            }
            ordered_stage_records = sorted(
                self._stage_records,
                key=lambda record: (
                    0 if record.get("stage") == "child" else 1,
                    child_order.get(str(record.get("child_id")), len(child_order)),
                ),
            )
            completion_buckets: dict[str, list[Mapping[str, Any]]] = {}
            for completion in self._inner_completions:
                completion_buckets.setdefault(_digest(completion), []).append(completion)
            ordered_inner_completions: list[Mapping[str, Any]] = []
            for record in ordered_stage_records:
                digest = record.get("completion_digest")
                bucket = completion_buckets.get(digest)
                if bucket:
                    ordered_inner_completions.append(bucket.pop(0))
            for bucket in completion_buckets.values():
                ordered_inner_completions.extend(bucket)
            ordered_provider_invocations = tuple(
                dict(invocation)
                for completion in ordered_inner_completions
                for invocation in completion.get("provider_invocations", ())
                if isinstance(invocation, Mapping)
            )
            effect_id_values: list[str] = []
            for completion in ordered_inner_completions:
                for effect_id in completion.get("effect_ids", ()):
                    if isinstance(effect_id, str) and effect_id not in effect_id_values:
                        effect_id_values.append(effect_id)
            ordered_effect_ids = tuple(effect_id_values)
            safe_status = status if status in {
                "approval_required",
                "route_review_required",
                "plan_refused",
                "selection_refused",
                "response_review_required",
                "reconciliation_required",
                "provider_failed",
                "children_completed",
                "children_partial",
                "child_failed",
                "child_incomplete",
                "completed",
                "failed",
                "abandoned",
            } else "failed"
            self._completion = AutonomousAgentStreamCompletion(
                status=safe_status,
                task_digest=self._task_digest,
                blueprint_digest=self._blueprint_digest,
                event_count=self._event_count,
                text_delta_bytes=self._text_delta_bytes,
                stage_count=len(self._stage_records),
                provider_invocations=ordered_provider_invocations,
                provider_failover=next(
                    (
                        dict(item.get("provider_failover"))
                        for item in reversed(ordered_inner_completions)
                        if item.get("provider_failover") is not None
                    ),
                    None,
                ),
                inner_completions=tuple(dict(item) for item in ordered_inner_completions),
                stage_records=tuple(dict(item) for item in ordered_stage_records),
                error_code=(getattr(error, "code", None) if error is not None else None),
                error_class=(type(error).__name__ if error is not None else None),
                effect_ids=ordered_effect_ids,
            )
        self._stop.set()
        self._wake_consumer()

    def _iterate(self) -> Iterator[AutonomousAgentStreamEvent]:
        try:
            while True:
                try:
                    item = self._queue.get(timeout=0.1)
                except Empty:
                    if self._completion is not None:
                        return
                    continue
                if item is None:
                    if self._completion is not None:
                        return
                    continue
                if isinstance(item, BaseException):
                    raise item
                if not isinstance(item, AutonomousAgentStreamEvent):
                    raise ProviderError("cross-domain stream queue contained an invalid event")
                yield item
        finally:
            if self._completion is None:
                self.close()


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
