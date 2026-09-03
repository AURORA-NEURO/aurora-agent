"""High-level reviewed-evidence composition for the Python autonomous agent.

The lower-level evidence runtime intentionally does not know how a model should be invoked.
This module closes that gap without collapsing authorization boundaries.  It binds a reviewed
evidence plan to caller-owned acquisition/evaluation adapters, projects the accepted evidence
into a transient provider context, and then delegates model selection and invocation to the
ordinary :class:`~prism_sdk.autonomy.AutonomousAgent` paths.

Three decisions remain independently visible:

* source dispatch requires ``approve_source_dispatch``;
* evidence must be accepted unless ``allow_incomplete_evidence`` is explicitly enabled; and
* provider invocation requires ``approve_provider_call`` and the existing agent gates.

The result is intentionally metadata-only when serialized.  ``evidence.values``, prompt
context, and the provider result remain available to the initiating caller but are never copied
into the durable projection or journal by this composition layer.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, fields, is_dataclass, replace
import hashlib
import inspect
import json
import math
import sys
from threading import Lock
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_runtime import (
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
)
from .llm_runtime import (
    CompositeProviderInvocationObserver,
    ProviderInvocationMetadata,
    ProviderRequest,
    ProviderResponse,
    ProviderTool,
)
from .errors import ArgumentError
from .brain import BrainRunError, BrainRunResult
from .autonomy import AutonomousAutoResult, AutonomousCrossDomainResult


AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA = "bioprism-python-autonomous-evidence-backed-run/0.1"
AUTONOMOUS_EVIDENCE_BACKED_RUN_STATUSES = (
    "evidence_review_required",
    "evidence_incomplete",
    "evidence_failed",
    "evidence_reconciliation_required",
    "provider_review_required",
    "provider_failed",
    "completed",
)
MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES = 2_000_000
MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_EXECUTION_BYTES = 2_000_000
MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_INTEGER_BITS = 4_096
MAX_AUTONOMOUS_EVIDENCE_BACKED_DOMAINS = 16
MAX_AUTONOMOUS_EVIDENCE_BACKED_CROSS_DOMAIN_SUBTASKS = 8
_PROVIDER_COMPLETION_STATUSES = frozenset(
    {"completed", "completed_provider_call", "children_completed", "succeeded"}
)
_PROVIDER_EXECUTION_ENVELOPE_SCHEMA = (
    "bioprism-python-autonomous-evidence-provider-execution-envelope/0.1"
)
_PROVIDER_REQUEST_IDEMPOTENCY_SCHEMA = (
    "bioprism-python-autonomous-evidence-provider-request-idempotency/0.1"
)


def _freeze_provider_dataclass_surface(
    owner: type[Any],
) -> dict[str, tuple[type[Any], Any]]:
    frozen: dict[str, tuple[type[Any], Any]] = {}
    seen: set[str] = set()
    for base in type.__getattribute__(owner, "__mro__"):
        namespace = type.__getattribute__(base, "__dict__")
        for name, descriptor in namespace.items():
            if name in seen:
                continue
            seen.add(name)
            target = (
                descriptor.__func__
                if isinstance(descriptor, (classmethod, staticmethod))
                else descriptor
            )
            if (
                callable(target) or inspect.isdatadescriptor(descriptor)
            ):
                frozen[name] = (base, descriptor)
    return frozen


def _freeze_provider_dataclass_types() -> dict[str, type[Any]]:
    frozen: dict[str, type[Any]] = {}
    for module_name, module in tuple(sys.modules.items()):
        if not module_name.startswith("prism_sdk.") or module is None:
            continue
        for exported_name, candidate in vars(module).items():
            if not isinstance(candidate, type) or not is_dataclass(candidate):
                continue
            if (
                type.__getattribute__(candidate, "__module__") != module_name
                or type.__getattribute__(candidate, "__qualname__") != exported_name
            ):
                continue
            frozen[f"{module_name}.{exported_name}"] = candidate
    return frozen


_FROZEN_PROVIDER_EXECUTION_TYPES = frozenset(
    {BrainRunResult, AutonomousAutoResult, AutonomousCrossDomainResult}
)
_FROZEN_PROVIDER_DATACLASS_TYPES = _freeze_provider_dataclass_types()
_FROZEN_PROVIDER_DATACLASS_NAMES = {
    value_type: name for name, value_type in _FROZEN_PROVIDER_DATACLASS_TYPES.items()
}
_FROZEN_PROVIDER_DATACLASS_FIELDS = {
    value_type: tuple(item.name for item in fields(value_type))
    for value_type in _FROZEN_PROVIDER_DATACLASS_NAMES
}
_FROZEN_PROVIDER_DATACLASS_SURFACES = {
    value_type: _freeze_provider_dataclass_surface(value_type)
    for value_type in _FROZEN_PROVIDER_DATACLASS_NAMES
}


def _provider_dataclass_type_is_intact(value_type: type[Any]) -> bool:
    type_name = _FROZEN_PROVIDER_DATACLASS_NAMES.get(value_type)
    if type_name is None:
        return False
    module_name, _, exported_name = type_name.rpartition(".")
    module = sys.modules.get(module_name)
    if module is None or vars(module).get(exported_name) is not value_type:
        return False
    frozen = _FROZEN_PROVIDER_DATACLASS_SURFACES[value_type]
    current = _freeze_provider_dataclass_surface(value_type)
    return current.keys() == frozen.keys() and all(
        current[name][0] is expected_owner
        and current[name][1] is expected_descriptor
        for name, (expected_owner, expected_descriptor) in frozen.items()
    )


def _provider_request_snapshot(
    provider: Any,
    request: Any,
) -> tuple[ProviderRequest, str]:
    """Detach and digest the exact request that may cross the private wire fence."""

    if (
        type(provider) is not str
        or type(request) is not ProviderRequest
        or not _provider_dataclass_type_is_intact(ProviderRequest)
    ):
        raise BrainRunError(
            "resumable provider dispatch requires an exact unmodified ProviderRequest"
        )
    nodes = [0]

    def detach(value: Any, *, depth: int = 0) -> Any:
        nodes[0] += 1
        if depth > 64 or nodes[0] > 100_000:
            raise BrainRunError(
                "resumable provider dispatch request exceeds its structural bound"
            )
        if value is None or type(value) is bool:
            return value
        if type(value) is int:
            if value.bit_length() > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_INTEGER_BITS:
                raise BrainRunError(
                    "resumable provider dispatch request integer is outside its bound"
                )
            return value
        if type(value) is float:
            if not math.isfinite(value):
                raise BrainRunError(
                    "resumable provider dispatch request contains a non-finite number"
                )
            return value
        if type(value) is str:
            if len(value.encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES:
                raise BrainRunError(
                    "resumable provider dispatch request string is outside its bound"
                )
            return value
        if type(value) is dict:
            if any(type(key) is not str for key in dict.keys(value)):
                raise BrainRunError(
                    "resumable provider dispatch request mappings require exact string keys"
                )
            try:
                return {
                    key: detach(child, depth=depth + 1)
                    for key, child in dict.items(value)
                }
            except RuntimeError as error:
                raise BrainRunError(
                    "resumable provider dispatch request changed while being snapshotted"
                ) from error
        if type(value) is list:
            return [detach(child, depth=depth + 1) for child in value]
        if type(value) is tuple:
            return tuple(detach(child, depth=depth + 1) for child in value)
        raise BrainRunError(
            "resumable provider dispatch request requires an exact detached built-in value graph"
        )

    messages = object.__getattribute__(request, "messages")
    tools = object.__getattribute__(request, "tools")
    response_schema = object.__getattribute__(request, "response_schema")
    if type(messages) is not tuple or type(tools) is not tuple:
        raise BrainRunError(
            "resumable provider dispatch request collections must be exact tuples"
        )
    if any(type(message) is not dict for message in messages) or (
        response_schema is not None and type(response_schema) is not dict
    ):
        raise BrainRunError(
            "resumable provider dispatch request requires exact built-in mappings"
        )
    if any(
        type(tool) is not ProviderTool
        or not _provider_dataclass_type_is_intact(ProviderTool)
        for tool in tools
    ):
        raise BrainRunError(
            "resumable provider dispatch tools are not exact trusted values"
        )
    detached_tools = tuple(
        ProviderTool(
            name=object.__getattribute__(tool, "name"),
            description=object.__getattribute__(tool, "description"),
            parameters=detach(
                object.__getattribute__(tool, "parameters"),
            ),
        )
        for tool in tools
    )
    snapshot = ProviderRequest(
        model=object.__getattribute__(request, "model"),
        messages=tuple(detach(message) for message in messages),
        max_output_tokens=object.__getattribute__(request, "max_output_tokens"),
        temperature=object.__getattribute__(request, "temperature"),
        require_json=object.__getattribute__(request, "require_json"),
        response_schema=(
            None if response_schema is None else detach(response_schema)
        ),
        idempotency_key=object.__getattribute__(request, "idempotency_key"),
        tools=detached_tools,
        tool_choice=object.__getattribute__(request, "tool_choice"),
    )
    request_projection = {
        "schema": _PROVIDER_REQUEST_IDEMPOTENCY_SCHEMA,
        "provider": provider,
        "model": object.__getattribute__(snapshot, "model"),
        "messages": [
            dict(message)
            for message in object.__getattribute__(snapshot, "messages")
        ],
        "max_output_tokens": object.__getattribute__(
            snapshot,
            "max_output_tokens",
        ),
        "temperature": object.__getattribute__(snapshot, "temperature"),
        "require_json": object.__getattribute__(snapshot, "require_json"),
        "response_schema": object.__getattribute__(snapshot, "response_schema"),
        "tools": [
            {
                "name": object.__getattribute__(tool, "name"),
                "description": object.__getattribute__(tool, "description"),
                "parameters": object.__getattribute__(tool, "parameters"),
            }
            for tool in detached_tools
        ],
        "tool_choice": object.__getattribute__(snapshot, "tool_choice"),
    }
    try:
        encoded = json.dumps(
            request_projection,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError(
            "resumable provider dispatch request is not canonical JSON"
        ) from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_EXECUTION_BYTES:
        raise BrainRunError(
            "resumable provider dispatch request exceeds its serialized byte bound"
        )
    request_digest = content_digest(request_projection)
    return snapshot, request_digest


class _ProviderDispatchFenceObserver:
    """Concurrency-safe per-attempt CAS fence and exact request-key scope."""

    __slots__ = (
        "_before_dispatch",
        "_dispatches",
        "_failure",
        "_lock",
        "_provider_idempotency_key",
    )

    def __init__(
        self,
        provider_idempotency_key: str,
        before_dispatch: Callable[[Mapping[str, Any]], None],
    ) -> None:
        self._provider_idempotency_key = provider_idempotency_key
        self._before_dispatch = before_dispatch
        self._dispatches: dict[str, dict[str, str]] = {}
        self._failure: BaseException | None = None
        self._lock = Lock()

    def before(self, _metadata: ProviderInvocationMetadata) -> None:
        return None

    def after(
        self,
        _metadata: ProviderInvocationMetadata,
        _response: ProviderResponse | None,
        _error: BaseException | None,
        _latency_ms: float,
    ) -> None:
        return None

    def prepare_dispatch(
        self,
        provider: str,
        request: ProviderRequest,
    ) -> ProviderRequest:
        request_snapshot, request_digest = _provider_request_snapshot(
            provider,
            request,
        )
        incoming_scope = request_snapshot.idempotency_key
        if incoming_scope is None:
            raise BrainRunError(
                "resumable provider dispatch lost its operation idempotency scope"
            )
        scoped_key = content_digest(
            {
                "schema": _PROVIDER_REQUEST_IDEMPOTENCY_SCHEMA,
                "provider_operation_idempotency_key": self._provider_idempotency_key,
                "incoming_idempotency_scope": incoming_scope,
                "provider": provider,
                "model": request_snapshot.model,
                "request_digest": request_digest,
            }
        )
        attestation = {
            "provider": provider,
            "model": request_snapshot.model,
            "request_digest": request_digest,
            "provider_idempotency_key": scoped_key,
        }
        with self._lock:
            if self._failure is not None:
                raise self._failure
            existing = self._dispatches.get(scoped_key)
            if existing is not None and existing != attestation:
                raise BrainRunError(
                    "resumable provider dispatch key collides with different request metadata"
                )
            self._dispatches[scoped_key] = attestation
        return replace(request_snapshot, idempotency_key=scoped_key)

    def before_transport(
        self,
        metadata: ProviderInvocationMetadata,
        dispatch_context: Mapping[str, Any],
    ) -> ProviderRequest:
        with self._lock:
            if self._failure is not None:
                raise self._failure
            if not isinstance(dispatch_context, Mapping) or set(
                dispatch_context
            ) != {
                "provider_idempotency_key",
                "provider_invocation_metadata",
                "provider_config",
                "provider_config_snapshot",
                "provider_transport",
                "provider_http_connection_factory",
                "provider_request",
                "provider_secret",
                "dispatch_scope_digest",
                "transport_attempt",
            }:
                raise BrainRunError(
                    "resumable provider dispatch received malformed transport context"
                )
            provider_idempotency_key = dispatch_context.get(
                "provider_idempotency_key"
            )
            if not isinstance(provider_idempotency_key, str):
                raise BrainRunError(
                    "resumable provider dispatch reached transport without an exact key"
                )
            attestation = self._dispatches.get(provider_idempotency_key)
            if attestation is None:
                raise BrainRunError(
                    "resumable provider dispatch reached transport without a bound request"
                )
            metadata_snapshot = dispatch_context.get(
                "provider_invocation_metadata"
            )
            if (
                type(metadata) is not ProviderInvocationMetadata
                or type(metadata_snapshot) is not tuple
                or len(metadata_snapshot) != 6
                or type(metadata_snapshot[0]) is not str
                or type(metadata_snapshot[1]) is not str
                or type(metadata_snapshot[2]) is not str
                or not metadata_snapshot[2]
                or len(metadata_snapshot[2].encode("utf-8")) > 128
                or any(
                    type(value) is not int or value < 0
                    for value in metadata_snapshot[3:]
                )
                or attestation["provider"] != metadata_snapshot[0]
                or attestation["model"] != metadata_snapshot[1]
            ):
                raise BrainRunError(
                    "resumable provider dispatch metadata changed after request fencing"
                )
            request_snapshot, current_request_digest = _provider_request_snapshot(
                metadata.provider,
                dispatch_context.get("provider_request"),
            )
            if (
                object.__getattribute__(request_snapshot, "idempotency_key")
                != provider_idempotency_key
                or current_request_digest != attestation["request_digest"]
            ):
                raise BrainRunError(
                    "resumable provider dispatch request changed after request fencing"
                )
            dispatch_scope_digest = dispatch_context.get(
                "dispatch_scope_digest"
            )
            transport_attempt = dispatch_context.get("transport_attempt")
            if (
                not isinstance(dispatch_scope_digest, str)
                or len(dispatch_scope_digest) != 64
                or any(
                    character not in "0123456789abcdef"
                    for character in dispatch_scope_digest
                )
                or isinstance(transport_attempt, bool)
                or not isinstance(transport_attempt, int)
                or not 1 <= transport_attempt <= 1_024
            ):
                raise BrainRunError(
                    "resumable provider dispatch transport context is outside its bounds"
                )
            try:
                self._before_dispatch(
                    {
                        **attestation,
                        # This exact object is private, process-local attestation material.
                        # The resumable layer validates its identity and deliberately omits it
                        # from the durable receipt.
                        "provider_config": dispatch_context.get("provider_config"),
                        "provider_config_snapshot": dispatch_context.get(
                            "provider_config_snapshot"
                        ),
                        "provider_transport": dispatch_context.get(
                            "provider_transport"
                        ),
                        "provider_http_connection_factory": dispatch_context.get(
                            "provider_http_connection_factory"
                        ),
                        "provider_request": request_snapshot,
                        "provider_secret": dispatch_context.get(
                            "provider_secret"
                        ),
                        "invocation_kind": metadata_snapshot[2],
                        "dispatch_scope_digest": dispatch_scope_digest,
                        "transport_attempt": transport_attempt,
                    }
                )
            except BaseException as error:
                self._failure = error
                raise
            # Return a second exact detached graph. The CAS callback never receives this value,
            # and the runtime uses it for the concrete transport immediately after this method.
            transport_request, transport_request_digest = _provider_request_snapshot(
                metadata.provider,
                request_snapshot,
            )
            if (
                transport_request_digest != attestation["request_digest"]
                or object.__getattribute__(
                    transport_request,
                    "idempotency_key",
                )
                != provider_idempotency_key
            ):
                raise BrainRunError(
                    "resumable provider dispatch request changed during durable fencing"
                )
            return transport_request


def _bounded_task(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 32_000:
        raise ArgumentError("evidence-backed task must be bounded non-empty text")
    return value.strip()


def _bounded_domains(value: Any, default: Sequence[str]) -> tuple[str, ...]:
    selected = default if value is None else value
    if not isinstance(selected, Sequence) or isinstance(selected, (str, bytes, bytearray)):
        raise ArgumentError("evidence-backed domains must be a sequence")
    if not 1 <= len(selected) <= MAX_AUTONOMOUS_EVIDENCE_BACKED_DOMAINS:
        raise ArgumentError("evidence-backed domains must contain 1..16 entries")
    result: list[str] = []
    for index, domain in enumerate(selected):
        if not isinstance(domain, str) or not domain.strip() or len(domain.encode("utf-8")) > 256:
            raise ArgumentError(f"evidence-backed domain {index} is malformed")
        normalized = domain.strip()
        if normalized in result:
            raise ArgumentError("evidence-backed domains must not contain duplicates")
        result.append(normalized)
    return tuple(result)


def _bounded_requests(value: Any) -> tuple[Mapping[str, Any], ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or not value:
        raise ArgumentError("evidence-backed requests must contain at least one mapping")
    if len(value) > 128:
        raise ArgumentError("evidence-backed requests exceed the 128-request bound")
    if any(not isinstance(item, Mapping) for item in value):
        raise ArgumentError("evidence-backed requests must contain mappings")
    return tuple(dict(item) for item in value)


def _json_safe_context(value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("evidence-backed prompt builder must return a mapping")
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    except (TypeError, ValueError) as error:
        raise ArgumentError("evidence-backed prompt context must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES:
        raise ArgumentError("evidence-backed prompt context exceeds its byte bound")
    return dict(value)


def _result_status(evidence_status: str) -> str:
    if evidence_status == "reconciliation_required":
        return "evidence_reconciliation_required"
    if evidence_status == "failed":
        return "evidence_failed"
    return "evidence_incomplete"


def _provider_execution_projection(
    value: Any,
    *,
    depth: int = 0,
    budget: list[int] | None = None,
    active: set[int] | None = None,
) -> Any:
    """Build a strict, type-preserving JSON graph without calling user serializers."""

    if depth > 64:
        raise BrainRunError("provider execution envelope is too deeply nested")
    if budget is None:
        budget = [100_000]
    budget[0] -= 1
    if budget[0] < 0:
        raise BrainRunError("provider execution envelope exceeds its node bound")
    if value is None or type(value) is bool:
        return value
    if type(value) is str:
        if len(value.encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_EXECUTION_BYTES:
            raise BrainRunError(
                "provider execution envelope contains a string outside its byte bound"
            )
        return value
    if type(value) is int:
        if value.bit_length() > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_INTEGER_BITS:
            raise BrainRunError(
                "provider execution envelope contains an integer outside its scalar bound"
            )
        return value
    if type(value) is float:
        if not math.isfinite(value):
            raise BrainRunError("provider execution envelope contains a non-finite float")
        return value

    if active is None:
        active = set()
    value_type = type(value)
    trusted_dataclass = value_type in _FROZEN_PROVIDER_DATACLASS_NAMES
    container = value_type in {dict, list, tuple} or trusted_dataclass
    identity = id(value)
    if container:
        if identity in active:
            raise BrainRunError("provider execution envelope contains a cycle")
        active.add(identity)
    try:
        if type(value) is dict:
            if any(type(key) is not str for key in value):
                raise BrainRunError(
                    "provider execution envelope mappings require string keys"
                )
            if any(
                len(key.encode("utf-8"))
                > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_EXECUTION_BYTES
                for key in value
            ):
                raise BrainRunError(
                    "provider execution envelope contains a mapping key outside its byte bound"
                )
            return {
                "kind": "dict",
                "items": {
                    key: _provider_execution_projection(
                        child,
                        depth=depth + 1,
                        budget=budget,
                        active=active,
                    )
                    for key, child in value.items()
                },
            }
        if type(value) in {list, tuple}:
            return {
                "kind": "tuple" if type(value) is tuple else "list",
                "items": [
                    _provider_execution_projection(
                        child,
                        depth=depth + 1,
                        budget=budget,
                        active=active,
                    )
                    for child in value
                ],
            }
        if trusted_dataclass:
            if not _provider_dataclass_type_is_intact(value_type):
                raise BrainRunError(
                    "provider execution envelope contains a modified or rebound SDK dataclass"
                )
            return {
                "kind": "sdk_dataclass",
                "type": _FROZEN_PROVIDER_DATACLASS_NAMES[value_type],
                "fields": {
                    field_name: _provider_execution_projection(
                        object.__getattribute__(value, field_name),
                        depth=depth + 1,
                        budget=budget,
                        active=active,
                    )
                    for field_name in _FROZEN_PROVIDER_DATACLASS_FIELDS[value_type]
                },
            }
        raise BrainRunError(
            "provider execution envelope contains an unsupported or opaque value"
        )
    finally:
        if container:
            active.remove(identity)


def _provider_execution_envelope_digest(execution: Any) -> str:
    """Digest the complete caller-owned result graph without persisting its payload."""

    if type(execution) not in _FROZEN_PROVIDER_EXECUTION_TYPES:
        raise BrainRunError(
            "provider execution envelope requires an exact built-in provider result type"
        )
    execution_value_type = type(execution)
    execution_type = (
        f"{type.__getattribute__(execution_value_type, '__module__')}."
        f"{type.__getattribute__(execution_value_type, '__qualname__')}"
    )
    projection = _provider_execution_projection(execution)
    envelope = {
        "schema": _PROVIDER_EXECUTION_ENVELOPE_SCHEMA,
        "execution_type": execution_type,
        "execution": projection,
    }
    try:
        encoded = json.dumps(
            envelope,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except Exception as error:
        raise BrainRunError(
            "provider execution envelope is not canonical JSON"
        ) from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_EXECUTION_BYTES:
        raise BrainRunError(
            "provider execution envelope exceeds its bounded serialized size"
        )
    # Hash only after the complete canonical envelope has passed its byte bound.  This avoids a
    # second serialization and never retains the caller-owned payload beyond this stack frame.
    return hashlib.sha256(encoded).hexdigest()


def _assert_provider_execution_snapshot_detached(
    original: Any,
    snapshot: Any,
    *,
    active: set[tuple[int, int]] | None = None,
) -> None:
    """Prove that every mutable/result container was detached from provider-owned state."""

    if type(original) is not type(snapshot):
        raise BrainRunError("provider execution snapshot changed a nested value type")
    value_type = type(original)
    if original is None or value_type in {str, bool, int, float}:
        return
    if active is None:
        active = set()
    pair = (id(original), id(snapshot))
    if pair in active:
        return
    active.add(pair)
    try:
        if value_type is dict:
            if original is snapshot:
                raise BrainRunError(
                    "provider execution snapshot retained mutable provider state"
                )
            if tuple(original) != tuple(snapshot):
                raise BrainRunError("provider execution snapshot changed mapping keys")
            for key in original:
                _assert_provider_execution_snapshot_detached(
                    original[key], snapshot[key], active=active
                )
            return
        if value_type in {list, tuple}:
            if value_type is list and original is snapshot:
                raise BrainRunError(
                    "provider execution snapshot retained mutable provider state"
                )
            if len(original) != len(snapshot):
                raise BrainRunError("provider execution snapshot changed sequence length")
            for left, right in zip(original, snapshot):
                _assert_provider_execution_snapshot_detached(
                    left, right, active=active
                )
            return
        if value_type in _FROZEN_PROVIDER_DATACLASS_NAMES:
            if original is snapshot:
                raise BrainRunError(
                    "provider execution snapshot retained provider result state"
                )
            for field_name in _FROZEN_PROVIDER_DATACLASS_FIELDS[value_type]:
                _assert_provider_execution_snapshot_detached(
                    object.__getattribute__(original, field_name),
                    object.__getattribute__(snapshot, field_name),
                    active=active,
                )
            return
        raise BrainRunError(
            "provider execution snapshot contains an unsupported or opaque value"
        )
    finally:
        active.remove(pair)


def _provider_execution_snapshot(execution: Any) -> Any:
    """Freeze the exact built-in result graph used for digesting and caller return."""

    # Validation is deliberately performed before and after deepcopy.  Frozen SDK dataclasses
    # may contain mutable caller/provider mappings, so retaining the original graph would permit
    # a digest/return time-of-check/time-of-use split.
    original_digest = _provider_execution_envelope_digest(execution)
    try:
        snapshot = copy.deepcopy(execution)
    except Exception as error:
        raise BrainRunError(
            "provider execution envelope could not be snapshotted"
        ) from error
    if type(snapshot) is not type(execution):
        raise BrainRunError("provider execution snapshot changed its result type")
    try:
        _assert_provider_execution_snapshot_detached(execution, snapshot)
    except BrainRunError:
        raise
    except Exception as error:
        raise BrainRunError(
            "provider execution changed while snapshot detachment was verified"
        ) from error
    snapshot_digest = _provider_execution_envelope_digest(snapshot)
    if (
        _provider_execution_envelope_digest(execution) != original_digest
        or snapshot_digest != original_digest
    ):
        raise BrainRunError("provider execution changed while it was being snapshotted")
    return snapshot


def _execution_metadata(agent: Any, execution: Any) -> tuple[str | None, str | None, str | None]:
    """Return status, route digest, and a complete payload-bound execution digest."""

    if execution is None:
        return None, None, None
    status = getattr(execution, "execution_status", getattr(execution, "status", None))
    normalized_status = status if isinstance(status, str) else None
    route_digest: str | None = None
    route = getattr(execution, "route", None)
    candidate_route_digest = getattr(route, "route_digest", None)
    if isinstance(candidate_route_digest, str) and len(candidate_route_digest) == 64:
        route_digest = candidate_route_digest
    try:
        metadata = agent._trace_execution_metadata(execution)
    except Exception:
        metadata = {
            "status": normalized_status,
            "result_type": execution.__class__.__name__,
        }
    if isinstance(metadata, Mapping):
        candidate = metadata.get("route_digest")
        if isinstance(candidate, str) and len(candidate) == 64:
            route_digest = candidate
    execution_digest = _provider_execution_envelope_digest(execution)
    return normalized_status, route_digest, execution_digest


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedPreflight:
    """Transient, caller-owned state immediately before the provider boundary."""

    task_digest: str
    execution_plan_digest: str
    evidence_plan: AutonomousEvidencePlan
    evidence: AutonomousEvidenceRuntimeResult
    prompt_context: Mapping[str, Any]


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedRunResult:
    """Transient execution envelope with a strict metadata-only projection."""

    status: str
    task_digest: str
    execution_plan_digest: str
    evidence_plan: AutonomousEvidencePlan
    evidence: AutonomousEvidenceRuntimeResult | None
    prompt_context: Mapping[str, Any]
    execution: Any | None
    route_digest: str | None
    execution_status: str | None
    execution_digest: str | None
    result_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor: dict[str, Any] = {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
            "status": self.status,
            "task_digest": self.task_digest,
            "execution_plan_digest": self.execution_plan_digest,
            "evidence_plan_digest": self.evidence_plan.plan_digest,
            "evidence_result_digest": None if self.evidence is None else self.evidence.result_digest,
            "execution_status": self.execution_status,
            "execution_digest": self.execution_digest,
            "route_digest": self.route_digest,
            "retention": "metadata_only;raw_evidence_prompt_values_and_provider_response_caller_owned",
            "secret_material": "never_returned",
        }
        descriptor["result_digest"] = self.result_digest
        return descriptor


def _build_result(
    *,
    status: str,
    task_digest: str,
    execution_plan_digest: str,
    evidence_plan: AutonomousEvidencePlan,
    evidence: AutonomousEvidenceRuntimeResult | None,
    prompt_context: Mapping[str, Any],
    execution: Any | None,
    route_digest: str | None,
    execution_status: str | None,
    execution_digest: str | None,
) -> AutonomousEvidenceBackedRunResult:
    descriptor = {
        "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
        "status": status,
        "task_digest": task_digest,
        "evidence_plan_digest": evidence_plan.plan_digest,
        "evidence_result_digest": None if evidence is None else evidence.result_digest,
        "execution_status": execution_status,
        "execution_digest": execution_digest,
        "route_digest": route_digest,
        "retention": "metadata_only;raw_evidence_prompt_values_and_provider_response_caller_owned",
        "secret_material": "never_returned",
    }
    return AutonomousEvidenceBackedRunResult(
        status=status,
        task_digest=task_digest,
        execution_plan_digest=execution_plan_digest,
        evidence_plan=evidence_plan,
        evidence=evidence,
        prompt_context=dict(prompt_context),
        execution=execution,
        route_digest=route_digest,
        execution_status=execution_status,
        execution_digest=execution_digest,
        result_digest=content_digest(descriptor),
    )


def run_autonomous_evidence_backed(
    agent: Any,
    *,
    task: str,
    requests: Sequence[Mapping[str, Any]],
    acquirer: Any,
    credentials: Any,
    domains: Sequence[str] | None = None,
    model_candidates: Sequence[Any] | None = None,
    projector: Any | None = None,
    evaluator: Any | None = None,
    rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None,
    parent_evidence_digests: Sequence[str] = (),
    stop_on_failure: bool = False,
    reevaluate_pending: bool = False,
    available_evidence: Sequence[str] = (),
    completed_stages: Mapping[str, Sequence[str]] | None = None,
    journal: AutonomousEvidenceRuntimeJournal | None = None,
    approve_source_dispatch: bool = False,
    allow_incomplete_evidence: bool = False,
    approve_provider_call: bool = False,
    provider_run_override: Any | None = None,
    provider_probe_only: bool = False,
    before_provider_run: Callable[
        [AutonomousEvidenceBackedPreflight], Mapping[str, Any] | None
    ]
    | None = None,
    before_provider_dispatch: Callable[[Mapping[str, Any]], None] | None = None,
    prompt_builder: Callable[[AutonomousEvidenceRuntimeResult], Mapping[str, Any]] | None = None,
    run_mode: str = "auto",
    run_options: Mapping[str, Any] | None = None,
) -> AutonomousEvidenceBackedRunResult:
    """Acquire reviewed evidence, then invoke the existing autonomous execution path.

    ``run_options`` is intentionally a mapping of ordinary agent options rather than a second
    authorization surface.  Task, domain, credentials, model candidates, and the three explicit
    approval controls are reserved and cannot be smuggled through it.  Opting into incomplete
    evidence may run the provider, but the outer result remains ``evidence_incomplete``.
    """

    if not hasattr(agent, "evidence_plan") or not callable(agent.evidence_plan):
        raise BrainRunError("evidence-backed execution requires an AutonomousAgent")
    if before_provider_dispatch is not None and not callable(
        before_provider_dispatch
    ):
        raise ArgumentError(
            "evidence-backed before_provider_dispatch must be callable or None"
        )
    task_text = _bounded_task(task)
    from .autonomy import AUTONOMOUS_DOMAINS

    selected_domains = _bounded_domains(domains, AUTONOMOUS_DOMAINS)
    if run_mode not in {"auto", "domain", "cross_domain"}:
        raise ArgumentError("evidence-backed run_mode must be auto, domain, or cross_domain")
    if run_mode == "domain" and len(selected_domains) != 1:
        raise ArgumentError("domain evidence-backed execution requires exactly one domain")
    if run_mode == "cross_domain" and not 2 <= len(selected_domains) <= MAX_AUTONOMOUS_EVIDENCE_BACKED_CROSS_DOMAIN_SUBTASKS:
        raise ArgumentError("cross-domain evidence-backed execution requires 2..8 domains")
    if not isinstance(approve_source_dispatch, bool) or not isinstance(allow_incomplete_evidence, bool) or not isinstance(approve_provider_call, bool) or not isinstance(provider_probe_only, bool):
        raise ArgumentError("evidence-backed approval controls must be booleans")
    if not callable(acquirer):
        # Protocol-style objects are supported by the runtime, so only reject an object that has
        # neither the callable form nor the documented acquire method.
        if not callable(getattr(acquirer, "acquire", None)):
            raise ArgumentError("evidence-backed acquirer must be callable or implement acquire")
    if not isinstance(run_options, Mapping) and run_options is not None:
        raise ArgumentError("evidence-backed run_options must be a mapping")
    source_requests = _bounded_requests(requests)
    options = {} if run_options is None else dict(run_options)
    reserved = {
        "task", "domain", "subtasks", "credentials", "model_candidates", "execution_id",
        "approve_provider_call", "approve_source_dispatch", "provider_idempotency_key",
    }
    forbidden = sorted(reserved.intersection(options))
    if forbidden:
        raise ArgumentError("evidence-backed run_options cannot override: " + ", ".join(forbidden))

    plan = agent.evidence_plan(
        selected_domains,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
    )
    task_digest = content_digest({"task": task_text})
    execution_plan_digest = content_digest(
        {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
            "task_digest": task_digest,
            "evidence_plan_digest": plan.plan_digest,
            "domains": list(selected_domains),
            "run_mode": run_mode,
        }
    )
    empty_context: dict[str, Any] = {}
    if not approve_source_dispatch:
        return _build_result(
            status="evidence_review_required",
            task_digest=task_digest,
            execution_plan_digest=execution_plan_digest,
            evidence_plan=plan,
            evidence=None,
            prompt_context=empty_context,
            execution=None,
            route_digest=None,
            execution_status=None,
            execution_digest=None,
        )

    runtime = AutonomousEvidenceRuntime(plan, journal=journal)
    runtime.rehydrate()
    evidence = runtime.execute(
        source_requests,
        acquirer=acquirer,
        projector=projector,
        evaluator=evaluator,
        rehydrate_value=rehydrate_value,
        parent_evidence_digests=parent_evidence_digests,
        stop_on_failure=stop_on_failure,
        reevaluate_pending=reevaluate_pending,
    )
    evidence_incomplete = evidence.status != "completed"
    if evidence_incomplete and not allow_incomplete_evidence:
        return _build_result(
            status=_result_status(evidence.status),
            task_digest=task_digest,
            execution_plan_digest=execution_plan_digest,
            evidence_plan=plan,
            evidence=evidence,
            prompt_context=empty_context,
            execution=None,
            route_digest=None,
            execution_status=None,
            execution_digest=None,
        )

    if prompt_builder is None:
        prompt_context: Mapping[str, Any] = {
            "evidence_backed": {
                "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
                "plan_digest": plan.plan_digest,
                "result_digest": evidence.result_digest,
                "status": evidence.status,
                "completed_requirement_ids": list(evidence.completed_requirement_ids),
                "pending_evaluation_requirement_ids": list(evidence.pending_evaluation_requirement_ids),
                "missing_requirement_ids": list(evidence.missing_requirement_ids),
                "retention": "metadata_only;raw_values_caller_owned",
            }
        }
    else:
        try:
            prompt_context = _json_safe_context(prompt_builder(evidence))
        except ArgumentError:
            raise
        except Exception as error:
            raise BrainRunError("evidence-backed prompt builder failed") from error

    if provider_probe_only:
        return _build_result(
            status="provider_review_required",
            task_digest=task_digest,
            execution_plan_digest=execution_plan_digest,
            evidence_plan=plan,
            evidence=evidence,
            prompt_context=prompt_context,
            execution=None,
            route_digest=None,
            execution_status=None,
            execution_digest=None,
        )

    options["approve_provider_call"] = approve_provider_call
    existing_context = options.get("context")
    if existing_context is not None and not isinstance(existing_context, Mapping):
        raise ArgumentError("evidence-backed run_options.context must be a mapping")
    merged_context = dict(existing_context or {})
    conflicting_context = sorted(set(merged_context).intersection(prompt_context))
    if conflicting_context:
        raise ArgumentError(
            "evidence-backed prompt context cannot override caller context: "
            + ", ".join(conflicting_context)
        )
    merged_context.update(prompt_context)
    options["context"] = merged_context

    if provider_run_override is not None:
        if approve_provider_call is not True:
            raise ArgumentError("evidence-backed provider_run_override requires provider approval")
        execution = provider_run_override
    else:
        provider_idempotency_key: str | None = None
        if before_provider_run is not None:
            if not callable(before_provider_run):
                raise ArgumentError("evidence-backed before_provider_run must be callable or None")
            provider_options = before_provider_run(
                AutonomousEvidenceBackedPreflight(
                    task_digest=task_digest,
                    execution_plan_digest=execution_plan_digest,
                    evidence_plan=plan,
                    evidence=evidence,
                    prompt_context=dict(prompt_context),
                )
            )
            if provider_options is not None:
                if not isinstance(provider_options, Mapping) or set(
                    provider_options
                ) != {"provider_idempotency_key"}:
                    raise ArgumentError(
                        "evidence-backed before_provider_run may return only provider_idempotency_key"
                    )
                idempotency_key = provider_options.get("provider_idempotency_key")
                if (
                    not isinstance(idempotency_key, str)
                    or not idempotency_key.strip()
                    or "\x00" in idempotency_key
                    or len(idempotency_key.encode("utf-8")) > 256
                ):
                    raise ArgumentError(
                        "evidence-backed provider idempotency_key is malformed"
                    )
                if "idempotency_key" in options:
                    raise ArgumentError(
                        "evidence-backed provider hook cannot replace caller idempotency_key"
                    )
                provider_idempotency_key = idempotency_key
                options["idempotency_key"] = idempotency_key
        if before_provider_dispatch is not None:
            if provider_idempotency_key is None:
                raise ArgumentError(
                    "evidence-backed before_provider_dispatch requires a provider_idempotency_key"
                )
            fence_observer = _ProviderDispatchFenceObserver(
                provider_idempotency_key,
                before_provider_dispatch,
            )
            existing_observer = options.get("invocation_observer")
            options["invocation_observer"] = CompositeProviderInvocationObserver._with_provider_dispatch_fence(
                () if existing_observer is None else (existing_observer,),
                fence_observer,
            )
        if run_mode == "domain":
            execution = agent.run(
                task=task_text,
                domain=selected_domains[0],
                credentials=credentials,
                model_candidates=model_candidates,
                **options,
            )
        elif run_mode == "cross_domain":
            subtasks = tuple(
                {
                    "id": f"evidence-{domain}",
                    "domain": domain,
                    "task": task_text,
                }
                for domain in selected_domains
            )
            execution = agent.run_cross_domain(
                task=task_text,
                subtasks=subtasks,
                credentials=credentials,
                model_candidates=model_candidates,
                **options,
            )
        else:
            execution = agent.run_auto(
                task=task_text,
                credentials=credentials,
                model_candidates=model_candidates,
                **options,
            )
    execution = _provider_execution_snapshot(execution)
    execution_status, route_digest, execution_digest = _execution_metadata(agent, execution)
    final_status = (
        "evidence_incomplete"
        if evidence_incomplete
        else "completed"
        if execution_status in _PROVIDER_COMPLETION_STATUSES
        else "provider_review_required"
        if isinstance(execution_status, str)
        and (execution_status == "approval_required" or execution_status.endswith("review_required"))
        else "provider_failed"
    )
    return _build_result(
        status=final_status,
        task_digest=task_digest,
        execution_plan_digest=execution_plan_digest,
        evidence_plan=plan,
        evidence=evidence,
        prompt_context=prompt_context,
        execution=execution,
        route_digest=route_digest,
        execution_status=execution_status,
        execution_digest=execution_digest,
    )


__all__ = [
    "AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_RUN_STATUSES",
    "MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES",
    "AutonomousEvidenceBackedPreflight",
    "AutonomousEvidenceBackedRunResult",
    "run_autonomous_evidence_backed",
]
