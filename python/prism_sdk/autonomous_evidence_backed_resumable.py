"""Restart-safe execution for reviewed evidence-backed autonomous runs.

The one-shot evidence bridge deliberately keeps source values and provider results transient.
This module adds the process boundary around that bridge: a bounded, digest-verified checkpoint,
caller-owned evidence journal rehydration, explicit provider resume, and an optional compare-and-
swap persistence adapter.  It never serializes task text, requests, evidence bodies, prompts,
credentials, or provider responses.

The provider boundary is treated as a quarantine point. ``provider_pending`` is a safe,
pre-approval state. A compare-and-swap acknowledged ``provider_in_flight`` checkpoint is written
immediately before dispatch; restoring that state can only reconcile a caller-owned result and
never dispatch again. An observed provider outcome that cannot be proved terminal remains
``provider_reconciliation_required``.
"""

from __future__ import annotations

import copy
from dataclasses import asdict, dataclass, field, is_dataclass
import http.client as _stdlib_http_client
import inspect
import json
import socket as _stdlib_socket
import ssl as _stdlib_ssl
from threading import Lock
import types
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence_runtime import (
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
)
from . import autonomous_evidence_brain as _autonomous_evidence_brain_module
from .autonomous_evidence_brain import (
    AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
    AutonomousEvidenceBackedPreflight,
    AutonomousEvidenceBackedRunResult,
    _bounded_domains,
    _bounded_task,
    _bounded_requests,
    _execution_metadata,
    _provider_execution_snapshot,
    run_autonomous_evidence_backed,
)
from .errors import ArgumentError
from .autonomous_effects import AutonomousEffectBoundary
from .autonomy import (
    AutonomousAgent,
    AutonomousTaskOrchestrator,
)
from . import brain as _brain_module
from .brain import AutonomousBrain, BrainRunError
from . import llm_runtime as _llm_runtime_module
from .llm_runtime import (
    CompositeProviderInvocationObserver,
    CredentialHandle,
    CredentialStore,
    InMemoryProvider,
    LLMRuntime,
    MAX_CREDENTIAL_BYTES,
    ProviderConfig,
    ProviderInvocationMetadata,
    ProviderRequest,
    ProviderTool,
    SecretValue,
)

_CREDENTIAL_ENTRY = vars(_llm_runtime_module)["_CredentialEntry"]


# Freeze the trusted virtual-dispatch identities when this module is imported.  Looking methods
# up on their owner during validation would let a later class monkeypatch redefine both the
# observed and expected value at once.
_AUTONOMOUS_AGENT_METHODS = {
    name: getattr(AutonomousAgent, name)
    for name in (
        "evidence_plan",
        "run_with_reviewed_evidence",
        "run",
        "run_auto",
        "run_cross_domain",
    )
}
_AUTONOMOUS_BRAIN_METHODS = {
    "run": AutonomousBrain.run,
    "run_adaptive": AutonomousBrain.run_adaptive,
    "run_cross_domain": AutonomousBrain.run_cross_domain,
}
_AUTONOMOUS_ORCHESTRATOR_METHODS = {
    "run": AutonomousTaskOrchestrator.run,
    "run_cross_domain": AutonomousTaskOrchestrator.run_cross_domain,
}
_LLM_RUNTIME_METHODS = {
    name: getattr(LLMRuntime, name)
    for name in (
        "invoke",
        "_authorize_provider",
        "_invocation_metadata",
        "_notify_invocation_before",
        "_prepare_invocation_dispatch",
        "_notify_invocation_before_transport",
        "_notify_invocation_after",
        "_body",
        "_provider_headers",
        "_post",
        "_post_with_retries",
        "_post_once",
    )
}
_AUTONOMOUS_EFFECT_BOUNDARY_METHODS = {
    "execute": AutonomousEffectBoundary.execute,
    "execute_stream": AutonomousEffectBoundary.execute_stream,
}
_PROVIDER_DISPATCH_FENCE_OBSERVER = vars(
    _autonomous_evidence_brain_module
)["_ProviderDispatchFenceObserver"]
_FROZEN_EVIDENCE_RUNNER = run_autonomous_evidence_backed
_FROZEN_PROVIDER_EXECUTION_SNAPSHOT = _provider_execution_snapshot
_FROZEN_EXECUTION_METADATA = _execution_metadata
_FROZEN_PROVIDER_REQUEST_SNAPSHOT = vars(
    _autonomous_evidence_brain_module
)["_provider_request_snapshot"]
_FROZEN_PROVIDER_OBSERVER_COMPOSER = vars(_brain_module)[
    "_compose_provider_observers"
]


@dataclass(frozen=True, slots=True)
class _FrozenBehaviorValue:
    """Identity plus a detached shape for mutable function-state values."""

    kind: str
    reference: Any
    children: tuple[Any, ...] = ()


@dataclass(frozen=True, slots=True)
class _FrozenFunctionBehavior:
    function: types.FunctionType
    code: types.CodeType
    defaults: _FrozenBehaviorValue
    keyword_defaults: _FrozenBehaviorValue
    annotations: _FrozenBehaviorValue
    attributes: _FrozenBehaviorValue
    closure: tuple[tuple[Any, _FrozenBehaviorValue], ...] | None


def _freeze_behavior_value(value: Any, *, depth: int = 0) -> _FrozenBehaviorValue:
    """Snapshot exact built-in state without invoking user-defined equality or copying hooks."""

    if depth > 32:
        return _FrozenBehaviorValue("identity", value)
    if value is None or type(value) in {bool, int, str, bytes}:
        return _FrozenBehaviorValue("scalar", value)
    if type(value) is float:
        return _FrozenBehaviorValue("float", value.hex())
    if type(value) in {tuple, list}:
        return _FrozenBehaviorValue(
            "tuple" if type(value) is tuple else "list",
            value,
            tuple(
                _freeze_behavior_value(item, depth=depth + 1)
                for item in value
            ),
        )
    if type(value) is dict:
        return _FrozenBehaviorValue(
            "dict",
            value,
            tuple(
                (
                    _freeze_behavior_value(key, depth=depth + 1),
                    _freeze_behavior_value(item, depth=depth + 1),
                )
                for key, item in dict.items(value)
            ),
        )
    if type(value) in {set, frozenset}:
        return _FrozenBehaviorValue(
            "set" if type(value) is set else "frozenset",
            value,
            tuple(
                _freeze_behavior_value(item, depth=depth + 1)
                for item in value
            ),
        )
    return _FrozenBehaviorValue("identity", value)


def _behavior_value_matches(value: Any, frozen: _FrozenBehaviorValue) -> bool:
    if frozen.kind == "scalar":
        return type(value) is type(frozen.reference) and value == frozen.reference
    if frozen.kind == "float":
        return type(value) is float and value.hex() == frozen.reference
    if frozen.kind == "identity":
        return value is frozen.reference
    if value is not frozen.reference:
        return False
    if frozen.kind in {"tuple", "list"}:
        return len(value) == len(frozen.children) and all(
            _behavior_value_matches(item, expected)
            for item, expected in zip(value, frozen.children)
        )
    if frozen.kind == "dict":
        if len(value) != len(frozen.children):
            return False
        return all(
            _behavior_value_matches(key, expected_key)
            and _behavior_value_matches(item, expected_item)
            for (key, item), (expected_key, expected_item) in zip(
                dict.items(value),
                frozen.children,
            )
        )
    if frozen.kind in {"set", "frozenset"}:
        current = tuple(value)
        return len(current) == len(frozen.children) and all(
            _behavior_value_matches(item, expected)
            for item, expected in zip(current, frozen.children)
        )
    return False


def _freeze_function_behavior(value: types.FunctionType) -> _FrozenFunctionBehavior:
    closure = object.__getattribute__(value, "__closure__")
    frozen_closure = None
    if closure is not None:
        cells: list[tuple[Any, _FrozenBehaviorValue]] = []
        for cell in closure:
            try:
                contents = cell.cell_contents
            except ValueError:
                contents = cell
            cells.append((cell, _freeze_behavior_value(contents)))
        frozen_closure = tuple(cells)
    return _FrozenFunctionBehavior(
        function=value,
        code=object.__getattribute__(value, "__code__"),
        defaults=_freeze_behavior_value(
            object.__getattribute__(value, "__defaults__")
        ),
        keyword_defaults=_freeze_behavior_value(
            object.__getattribute__(value, "__kwdefaults__")
        ),
        annotations=_freeze_behavior_value(
            object.__getattribute__(value, "__annotations__")
        ),
        attributes=_freeze_behavior_value(object.__getattribute__(value, "__dict__")),
        closure=frozen_closure,
    )


def _function_behavior_matches(
    value: Any,
    frozen: _FrozenFunctionBehavior,
) -> bool:
    if type(value) is not types.FunctionType or value is not frozen.function:
        return False
    if object.__getattribute__(value, "__code__") is not frozen.code:
        return False
    if not _behavior_value_matches(
        object.__getattribute__(value, "__defaults__"),
        frozen.defaults,
    ):
        return False
    if not _behavior_value_matches(
        object.__getattribute__(value, "__kwdefaults__"),
        frozen.keyword_defaults,
    ):
        return False
    if not _behavior_value_matches(
        object.__getattribute__(value, "__annotations__"),
        frozen.annotations,
    ) or not _behavior_value_matches(
        object.__getattribute__(value, "__dict__"),
        frozen.attributes,
    ):
        return False
    closure = object.__getattribute__(value, "__closure__")
    if frozen.closure is None:
        return closure is None
    if closure is None or len(closure) != len(frozen.closure):
        return False
    for cell, (expected_cell, expected_contents) in zip(
        closure,
        frozen.closure,
    ):
        if cell is not expected_cell:
            return False
        try:
            contents = cell.cell_contents
        except ValueError:
            contents = cell
        if not _behavior_value_matches(contents, expected_contents):
            return False
    return True


def _freeze_referenced_function_globals(
    value: types.FunctionType,
) -> tuple[
    dict[str, Any],
    tuple[
        tuple[str, _FrozenBehaviorValue, _FrozenFunctionBehavior | None],
        ...,
    ],
]:
    globals_state = object.__getattribute__(value, "__globals__")
    referenced = []
    for name in sorted(set(object.__getattribute__(value, "__code__").co_names)):
        if name not in globals_state:
            continue
        candidate = dict.__getitem__(globals_state, name)
        referenced.append(
            (
                name,
                _freeze_behavior_value(candidate),
                (
                    _freeze_function_behavior(candidate)
                    if type(candidate) is types.FunctionType
                    else None
                ),
            )
        )
    return globals_state, tuple(referenced)


def _referenced_function_globals_match(
    value: Any,
    frozen: tuple[
        dict[str, Any],
        tuple[
            tuple[str, _FrozenBehaviorValue, _FrozenFunctionBehavior | None],
            ...,
        ],
    ],
) -> bool:
    if type(value) is not types.FunctionType:
        return False
    expected_globals, expected_bindings = frozen
    current_globals = object.__getattribute__(value, "__globals__")
    if current_globals is not expected_globals:
        return False
    for name, expected_value, expected_behavior in expected_bindings:
        if name not in current_globals:
            return False
        current = dict.__getitem__(current_globals, name)
        if not _behavior_value_matches(current, expected_value):
            return False
        if expected_behavior is not None and not _function_behavior_matches(
            current,
            expected_behavior,
        ):
            return False
    return True


def _descriptor_behavior_snapshot(descriptor: Any) -> Any:
    if type(descriptor) is types.FunctionType:
        return ("function", _freeze_function_behavior(descriptor))
    if isinstance(descriptor, (classmethod, staticmethod)):
        function = descriptor.__func__
        if type(function) is types.FunctionType:
            return ("function", _freeze_function_behavior(function))
    if type(descriptor) is property:
        return (
            "property",
            tuple(
                None
                if function is None
                else _freeze_function_behavior(function)
                for function in (
                    descriptor.fget,
                    descriptor.fset,
                    descriptor.fdel,
                )
            ),
        )
    return None


def _descriptor_behavior_matches(descriptor: Any, frozen: Any) -> bool:
    if frozen is None:
        return True
    kind, state = frozen
    if kind == "function":
        function = (
            descriptor.__func__
            if isinstance(descriptor, (classmethod, staticmethod))
            else descriptor
        )
        return _function_behavior_matches(function, state)
    if kind == "property" and type(descriptor) is property:
        current = (descriptor.fget, descriptor.fset, descriptor.fdel)
        return all(
            function is None
            if expected is None
            else _function_behavior_matches(function, expected)
            for function, expected in zip(current, state)
        )
    return False


def _freeze_callable_surface(
    owner: type[Any],
) -> dict[str, tuple[type[Any], Any, Any]]:
    """Capture every effective callable or data-descriptor on one trusted class.

    Capturing only the class that originally defined a method is insufficient: a later attribute
    added to a higher MRO entry can shadow that definition without modifying the original base.
    The returned owner/descriptor pair therefore records effective lookup, not merely provenance.
    """

    frozen: dict[str, tuple[type[Any], Any, Any]] = {}
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
                frozen[name] = (
                    base,
                    descriptor,
                    _descriptor_behavior_snapshot(descriptor),
                )
    return frozen


def _effective_class_descriptor(
    owner: type[Any],
    name: str,
) -> tuple[type[Any], Any] | None:
    """Resolve a descriptor statically, without invoking caller-controlled lookup hooks."""

    for base in type.__getattribute__(owner, "__mro__"):
        namespace = type.__getattribute__(base, "__dict__")
        if name in namespace:
            return base, namespace[name]
    return None


def _surface_matches(
    owner: type[Any],
    frozen: Mapping[str, tuple[type[Any], Any, Any]],
) -> bool:
    current = _freeze_callable_surface(owner)
    return current.keys() == frozen.keys() and all(
        current[name][0] is expected_owner
        and current[name][1] is expected_descriptor
        and _descriptor_behavior_matches(
            current[name][1],
            expected_behavior,
        )
        for name, (
            expected_owner,
            expected_descriptor,
            expected_behavior,
        ) in frozen.items()
    )


_FROZEN_CORE_CALLABLE_SURFACES = {
    AutonomousAgent: _freeze_callable_surface(AutonomousAgent),
    AutonomousTaskOrchestrator: _freeze_callable_surface(
        AutonomousTaskOrchestrator
    ),
    AutonomousBrain: _freeze_callable_surface(AutonomousBrain),
    LLMRuntime: _freeze_callable_surface(LLMRuntime),
    CompositeProviderInvocationObserver: _freeze_callable_surface(
        CompositeProviderInvocationObserver
    ),
    CredentialStore: _freeze_callable_surface(CredentialStore),
    CredentialHandle: _freeze_callable_surface(CredentialHandle),
    ProviderConfig: _freeze_callable_surface(ProviderConfig),
    ProviderInvocationMetadata: _freeze_callable_surface(
        ProviderInvocationMetadata
    ),
    ProviderRequest: _freeze_callable_surface(ProviderRequest),
    ProviderTool: _freeze_callable_surface(ProviderTool),
    SecretValue: _freeze_callable_surface(SecretValue),
    _CREDENTIAL_ENTRY: _freeze_callable_surface(_CREDENTIAL_ENTRY),
    _stdlib_http_client.HTTPConnection: _freeze_callable_surface(
        _stdlib_http_client.HTTPConnection
    ),
    _stdlib_http_client.HTTPSConnection: _freeze_callable_surface(
        _stdlib_http_client.HTTPSConnection
    ),
    _stdlib_socket.socket: _freeze_callable_surface(_stdlib_socket.socket),
    _stdlib_ssl.SSLContext: _freeze_callable_surface(_stdlib_ssl.SSLContext),
    json.JSONEncoder: _freeze_callable_surface(json.JSONEncoder),
    InMemoryProvider: _freeze_callable_surface(InMemoryProvider),
    _PROVIDER_DISPATCH_FENCE_OBSERVER: _freeze_callable_surface(
        _PROVIDER_DISPATCH_FENCE_OBSERVER
    ),
    AutonomousEffectBoundary: _freeze_callable_surface(
        AutonomousEffectBoundary
    ),
}
_FROZEN_DISPATCH_FUNCTION_BEHAVIORS = {
    "evidence_runner": _freeze_function_behavior(_FROZEN_EVIDENCE_RUNNER),
    "provider_execution_snapshot": _freeze_function_behavior(
        _FROZEN_PROVIDER_EXECUTION_SNAPSHOT
    ),
    "execution_metadata": _freeze_function_behavior(_FROZEN_EXECUTION_METADATA),
    "provider_request_snapshot": _freeze_function_behavior(
        _FROZEN_PROVIDER_REQUEST_SNAPSHOT
    ),
    "provider_observer_composer": _freeze_function_behavior(
        _FROZEN_PROVIDER_OBSERVER_COMPOSER
    ),
}
_FROZEN_EVIDENCE_BRAIN_TRUST_GLOBALS = {
    name: vars(_autonomous_evidence_brain_module).get(name)
    for name in (
        "ProviderRequest",
        "ProviderTool",
        "BrainRunResult",
        "AutonomousAutoResult",
        "AutonomousCrossDomainResult",
        "copy",
        "json",
        "math",
        "hashlib",
        "replace",
        "fields",
        "content_digest",
        "_provider_dataclass_type_is_intact",
        "_provider_execution_projection",
        "_provider_execution_envelope_digest",
        "_assert_provider_execution_snapshot_detached",
        "_provider_execution_snapshot",
        "_execution_metadata",
        "_provider_request_snapshot",
        "_FROZEN_PROVIDER_EXECUTION_TYPES",
        "_FROZEN_PROVIDER_DATACLASS_TYPES",
        "_FROZEN_PROVIDER_DATACLASS_NAMES",
        "_FROZEN_PROVIDER_DATACLASS_FIELDS",
        "_FROZEN_PROVIDER_DATACLASS_SURFACES",
    )
}
_FROZEN_EVIDENCE_BRAIN_TRUST_GLOBAL_STATES = {
    name: _freeze_behavior_value(value)
    for name, value in _FROZEN_EVIDENCE_BRAIN_TRUST_GLOBALS.items()
}
_FROZEN_EVIDENCE_BRAIN_TRUST_FUNCTION_BEHAVIORS = {
    name: _freeze_function_behavior(value)
    for name, value in _FROZEN_EVIDENCE_BRAIN_TRUST_GLOBALS.items()
    if type(value) is types.FunctionType
}
_FROZEN_EVIDENCE_BRAIN_MODULE_DEPENDENCIES = tuple(
    (
        module,
        name,
        vars(module).get(name),
        (
            _freeze_function_behavior(vars(module).get(name))
            if type(vars(module).get(name)) is types.FunctionType
            else None
        ),
    )
    for module, name in (
        (copy, "deepcopy"),
        (json, "dumps"),
        (vars(_autonomous_evidence_brain_module)["math"], "isfinite"),
        (vars(_autonomous_evidence_brain_module)["hashlib"], "sha256"),
    )
)

# The HTTP constructor is itself part of the transport boundary.  Keeping only the runtime
# method identity would still allow caller code to replace ``http.client.HTTPSConnection``
# between evidence preparation and the wire call.  Freeze both the module graph and the two
# concrete factories when this module establishes its trust root.
_FROZEN_LLM_RUNTIME_HTTP_PACKAGE = vars(_llm_runtime_module).get("http")
_FROZEN_HTTP_CLIENT_MODULE = _stdlib_http_client
_FROZEN_HTTP_CONNECTION = _stdlib_http_client.HTTPConnection
_FROZEN_HTTPS_CONNECTION = _stdlib_http_client.HTTPSConnection
_FROZEN_HTTP_SOCKET_MODULE = vars(_stdlib_http_client).get("socket")
_FROZEN_SOCKET_MODULE = _stdlib_socket
_FROZEN_SOCKET_CREATE_CONNECTION = vars(_stdlib_socket).get(
    "create_connection"
)
_FROZEN_SOCKET_GETADDRINFO = vars(_stdlib_socket).get("getaddrinfo")
_FROZEN_SOCKET_CLASS = vars(_stdlib_socket).get("socket")
_FROZEN_LOW_LEVEL_SOCKET_MODULE = vars(_stdlib_socket).get("_socket")
_FROZEN_LOW_LEVEL_GETADDRINFO = (
    None
    if _FROZEN_LOW_LEVEL_SOCKET_MODULE is None
    else vars(_FROZEN_LOW_LEVEL_SOCKET_MODULE).get("getaddrinfo")
)
_FROZEN_HTTP_CREATE_HTTPS_CONTEXT = vars(_stdlib_http_client).get(
    "_create_https_context"
)
_FROZEN_SSL_MODULE = _stdlib_ssl
_FROZEN_SSL_CONTEXT_CLASS = vars(_stdlib_ssl).get("SSLContext")
_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT = vars(_stdlib_ssl).get(
    "_create_default_https_context"
)
_FROZEN_JSON_MODULE = json
_FROZEN_JSON_DUMPS = json.dumps
_FROZEN_JSON_ENCODER = json.JSONEncoder
_FROZEN_JSON_ENCODER_MODULE = json.encoder
_FROZEN_JSON_ENCODER_GLOBALS = {
    name: vars(json.encoder).get(name)
    for name in (
        "encode_basestring",
        "encode_basestring_ascii",
        "c_make_encoder",
        "_make_iterencode",
    )
}
_FROZEN_LLM_RUNTIME_DISPATCH_GLOBALS = {
    name: vars(_llm_runtime_module).get(name)
    for name in (
        "ProviderConfig",
        "ProviderInvocationMetadata",
        "ProviderRequest",
        "ProviderTool",
        "SecretValue",
        "_CredentialEntry",
        "_provider_observer_metadata_projection",
        "content_digest",
        "replace",
        "urlsplit",
        "_normalize_provider_path",
        "_bounded_json_bytes",
        "_canonical_provider_content_part",
        "_normalize_provider_content",
        "_provider_content_text",
        "_image_data_url",
        "_wire_provider_content",
        "_wire_messages",
    )
}
_FROZEN_LLM_RUNTIME_DISPATCH_FUNCTION_BEHAVIORS = {
    name: _freeze_function_behavior(value)
    for name, value in _FROZEN_LLM_RUNTIME_DISPATCH_GLOBALS.items()
    if type(value) is types.FunctionType
}
_FROZEN_JSON_DUMPS_BEHAVIOR = _freeze_function_behavior(_FROZEN_JSON_DUMPS)
_FROZEN_JSON_ENCODER_GLOBAL_BEHAVIORS = {
    name: _freeze_function_behavior(value)
    for name, value in _FROZEN_JSON_ENCODER_GLOBALS.items()
    if type(value) is types.FunctionType
}
_FROZEN_SOCKET_CREATE_CONNECTION_BEHAVIOR = (
    _freeze_function_behavior(_FROZEN_SOCKET_CREATE_CONNECTION)
    if type(_FROZEN_SOCKET_CREATE_CONNECTION) is types.FunctionType
    else None
)
_FROZEN_SOCKET_CREATE_CONNECTION_GLOBALS = (
    _freeze_referenced_function_globals(_FROZEN_SOCKET_CREATE_CONNECTION)
    if type(_FROZEN_SOCKET_CREATE_CONNECTION) is types.FunctionType
    else None
)
_FROZEN_SOCKET_GETADDRINFO_BEHAVIOR = (
    _freeze_function_behavior(_FROZEN_SOCKET_GETADDRINFO)
    if type(_FROZEN_SOCKET_GETADDRINFO) is types.FunctionType
    else None
)
_FROZEN_SOCKET_GETADDRINFO_GLOBALS = (
    _freeze_referenced_function_globals(_FROZEN_SOCKET_GETADDRINFO)
    if type(_FROZEN_SOCKET_GETADDRINFO) is types.FunctionType
    else None
)
_FROZEN_HTTP_CREATE_HTTPS_CONTEXT_BEHAVIOR = (
    _freeze_function_behavior(_FROZEN_HTTP_CREATE_HTTPS_CONTEXT)
    if type(_FROZEN_HTTP_CREATE_HTTPS_CONTEXT) is types.FunctionType
    else None
)
_FROZEN_HTTP_CREATE_HTTPS_CONTEXT_GLOBALS = (
    _freeze_referenced_function_globals(_FROZEN_HTTP_CREATE_HTTPS_CONTEXT)
    if type(_FROZEN_HTTP_CREATE_HTTPS_CONTEXT) is types.FunctionType
    else None
)
_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT_BEHAVIOR = (
    _freeze_function_behavior(_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT)
    if type(_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT) is types.FunctionType
    else None
)
_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT_GLOBALS = (
    _freeze_referenced_function_globals(_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT)
    if type(_FROZEN_SSL_DEFAULT_HTTPS_CONTEXT) is types.FunctionType
    else None
)
_FROZEN_PROVIDER_CONFIG_FIELDS = tuple(
    vars(ProviderConfig).get("__dataclass_fields__", {})
)
_PROVIDER_CONFIG_TEXT_FIELDS = frozenset(
    {
        "provider",
        "base_url",
        "protocol",
        "path",
        "api_key_header",
        "models_path",
        "structured_output_mode",
    }
)
_PROVIDER_CONFIG_BOOLEAN_FIELDS = frozenset(
    {"requires_credential", "allow_insecure_http"}
)
_PROVIDER_CONFIG_INTEGER_FIELDS = frozenset(
    {
        "max_response_bytes",
        "max_attempts",
        "circuit_breaker_failure_threshold",
    }
)
_PROVIDER_CONFIG_NUMBER_FIELDS = frozenset(
    {
        "timeout_seconds",
        "retry_backoff_seconds",
        "circuit_breaker_reset_seconds",
    }
)


def _frozen_callable_surface_is_intact(instance: Any, owner: type[Any]) -> bool:
    frozen = _FROZEN_CORE_CALLABLE_SURFACES[owner]
    if not _surface_matches(owner, frozen):
        return False
    try:
        instance_values = object.__getattribute__(instance, "__dict__")
    except AttributeError:
        instance_values = None
    if instance_values is not None and type(instance_values) is not dict:
        return False
    return instance_values is None or not any(name in instance_values for name in frozen)


AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA = "bioprism-python-autonomous-evidence-backed-checkpoint/0.4"
AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA = "bioprism-python-autonomous-evidence-backed-resumable-result/0.1"
AUTONOMOUS_EVIDENCE_BACKED_CONTROLLER_SCHEMA = "bioprism-python-autonomous-evidence-backed-controller/0.1"
AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA = (
    "bioprism-python-autonomous-evidence-backed-provider-dispatch-receipt/0.1"
)
MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES = 64_000
MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_GENERATION = 2_147_483_647
MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES = 1_024
AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES = (
    "evidence_review_required",
    "evidence_incomplete",
    "evidence_failed",
    "evidence_reconciliation_required",
    "provider_pending",
    "provider_in_flight",
    "provider_reconciliation_required",
    "completed",
)
AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_STATUSES = (
    *AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES,
)
_RETENTION = "metadata_only;task_requests_evidence_and_provider_payloads_caller_owned"
_RESULT_RETENTION = "metadata_only;raw_evidence_and_provider_payloads_caller_owned"
_CONTROLLER_RETENTION = "metadata_only_task_request_evidence_and_provider_payloads_caller_owned"
_SECRET_MATERIAL = "never_returned"
_RESUMABLE_POLICY_IDENTITY_ROLES = (
    "acquirer",
    "projector",
    "evaluator",
    "value_rehydrator",
    "prompt_builder",
    "provider_policy",
)
_EVIDENCE_PROVIDER_PROJECTION_SCHEMA = (
    "bioprism-python-autonomous-evidence-provider-projection/0.1"
)
_PROVIDER_OPERATION_SCHEMA = (
    "bioprism-python-autonomous-evidence-backed-provider-operation/0.1"
)
_PROVIDER_IDEMPOTENCY_SCHEMA = (
    "bioprism-python-autonomous-evidence-backed-provider-idempotency/0.1"
)
_PROVIDER_IDEMPOTENCY_KEY_DIGEST_SCHEMA = (
    "bioprism-python-autonomous-evidence-backed-provider-idempotency-key/0.1"
)
_PROVIDER_DISPATCH_RETENTION = (
    "metadata_only;exact_idempotency_key_private_to_dispatch_ledger"
)
_PROVIDER_DISPATCH_PRIVATE_RETENTION = (
    "caller_owned_private_provider_dispatch_receipt"
)
_PROVIDER_DISPATCH_PRIVATE_MATERIAL = (
    "caller_owned_private_idempotency_key_present"
)
_PROVIDER_COMPLETION_STATUSES = frozenset(
    {"completed", "completed_provider_call", "children_completed", "succeeded"}
)
_PROVIDER_CHECKPOINT_STATUSES = frozenset(
    {
        "completed",
        "approval_required",
        "route_review_required",
        "planning_review_required",
        "policy_review_required",
        "response_review_required",
        "synthesis_response_review_required",
        "policy_blocked",
        "plan_refused",
        "abstained",
        "provider_abstained",
        "provider_invalid",
        "provider_failed",
        "provider_disagreement",
        "reconciliation_required",
        "children_partial",
        "child_failed",
        "child_incomplete",
        "failed",
    }
)


def _identifier(name: str, value: Any) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value.encode("utf-8")) > 256
        or "\x00" in value
        or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-" for character in value)
    ):
        raise ArgumentError(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _identifier(name, value)


def _json_bytes(value: Any, name: str) -> int:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return len(encoded)


def _policy_value(value: Any, *, depth: int = 0) -> Any:
    """Project run policy objects without retaining their transient values."""

    if depth > 12:
        raise ArgumentError("evidence-backed run policy is too deeply nested")
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, Mapping):
        return {str(key): _policy_value(child, depth=depth + 1) for key, child in sorted(value.items(), key=lambda item: str(item[0]))}
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 256:
            raise ArgumentError("evidence-backed run policy contains too many entries")
        return [_policy_value(child, depth=depth + 1) for child in value]
    serializer = getattr(value, "to_dict", None)
    if callable(serializer):
        return _policy_value(serializer(), depth=depth + 1)
    if callable(value):
        return {"callable_type": value.__class__.__name__}
    return {"object_type": value.__class__.__name__}


def _provider_policy_value(
    value: Any,
    *,
    name: str,
    depth: int = 0,
    trust_opaque: bool = False,
) -> Any:
    """Project provider-shaping policy without silently collapsing opaque state."""

    if depth > 12:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, Mapping):
        if any(not isinstance(key, str) for key in value):
            raise ArgumentError(f"{name} must use string mapping keys")
        return {
            key: _provider_policy_value(
                child,
                name=f"{name}.{key}",
                depth=depth + 1,
                trust_opaque=trust_opaque,
            )
            for key, child in sorted(value.items())
        }
    if isinstance(value, Sequence) and not isinstance(
        value, (str, bytes, bytearray)
    ):
        if len(value) > 256:
            raise ArgumentError(f"{name} contains too many entries")
        return [
            _provider_policy_value(
                child,
                name=f"{name}[{index}]",
                depth=depth + 1,
                trust_opaque=trust_opaque,
            )
            for index, child in enumerate(value)
        ]
    serializer = getattr(value, "to_dict", None)
    if callable(serializer):
        try:
            serialized = serializer()
        except Exception as error:
            raise ArgumentError(
                f"{name} could not be projected into stable provider policy"
            ) from error
        return _provider_policy_value(
            serialized,
            name=name,
            depth=depth + 1,
            trust_opaque=trust_opaque,
        )
    if is_dataclass(value) and not isinstance(value, type):
        try:
            serialized = asdict(value)
        except Exception as error:
            raise ArgumentError(
                f"{name} could not be projected into stable provider policy"
            ) from error
        return _provider_policy_value(
            serialized,
            name=name,
            depth=depth + 1,
            trust_opaque=trust_opaque,
        )
    kind = "callable" if callable(value) else "opaque"
    if trust_opaque:
        return {
            "present": True,
            "kind": kind,
            "identity": "bound_by_resumable_provider_policy_config_digest",
        }
    raise ArgumentError(
        f"resumable provider fencing: {name} contains an unsupported {kind} provider-shaping value"
    )


def _provider_input_snapshot(
    name: str,
    value: Any,
    *,
    trust_opaque: bool = False,
    depth: int = 0,
) -> Any:
    if depth > 12:
        raise ArgumentError(f"{name} is too deeply nested to snapshot")
    if value is None or isinstance(value, (str, int, float, bool, bytes)):
        return value
    if isinstance(value, Mapping):
        return {
            key: _provider_input_snapshot(
                f"{name}.{key}",
                child,
                trust_opaque=trust_opaque,
                depth=depth + 1,
            )
            for key, child in value.items()
        }
    if isinstance(value, tuple):
        return tuple(
            _provider_input_snapshot(
                f"{name}[{index}]",
                child,
                trust_opaque=trust_opaque,
                depth=depth + 1,
            )
            for index, child in enumerate(value)
        )
    if isinstance(value, list):
        return [
            _provider_input_snapshot(
                f"{name}[{index}]",
                child,
                trust_opaque=trust_opaque,
                depth=depth + 1,
            )
            for index, child in enumerate(value)
        ]
    try:
        return copy.deepcopy(value)
    except Exception as error:
        if trust_opaque:
            return value
        raise ArgumentError(
            f"{name} could not be snapshotted before provider fencing"
        ) from error


def _exact_bound_method(instance: Any, name: str, expected: Any) -> bool:
    try:
        instance_values = object.__getattribute__(instance, "__dict__")
    except AttributeError:
        instance_values = None
    if instance_values is not None and (
        type(instance_values) is not dict or name in instance_values
    ):
        return False
    resolved = _effective_class_descriptor(type(instance), name)
    if resolved is None:
        return False
    descriptor = resolved[1]
    target = (
        descriptor.__func__
        if isinstance(descriptor, (classmethod, staticmethod))
        else descriptor
    )
    return target is expected


def _exact_instance_dict(instance: Any) -> dict[str, Any] | None:
    """Read ordinary instance state without invoking an overridden ``__getattribute__``."""

    try:
        values = object.__getattribute__(instance, "__dict__")
    except AttributeError:
        return None
    return values if type(values) is dict else None


def _assert_resumable_core_runtime(
    agent: Any,
    *,
    run_options: Mapping[str, Any] | None,
) -> None:
    """Reject virtual dispatch that could bypass the final provider fence."""

    # Check the outer type before touching attributes.  A duck-typed object may implement
    # hostile descriptors, and resumable validation must not execute caller code before it has
    # established the core implementation boundary.
    if type(agent) is not AutonomousAgent:
        raise ArgumentError(
            "resumable provider fencing requires the exact core AutonomousAgent type"
        )
    # These helpers are outside the class MRO but still sit directly on the dispatch fence:
    # evidence composition creates the private observer and brain composition carries it beside
    # the execution-policy observer. Freeze their module bindings and private class surface too.
    if (
        vars(_autonomous_evidence_brain_module).get(
            "run_autonomous_evidence_backed"
        )
        is not _FROZEN_EVIDENCE_RUNNER
        or vars(_autonomous_evidence_brain_module).get(
            "_provider_execution_snapshot"
        )
        is not _FROZEN_PROVIDER_EXECUTION_SNAPSHOT
        or vars(_autonomous_evidence_brain_module).get("_execution_metadata")
        is not _FROZEN_EXECUTION_METADATA
        or vars(_autonomous_evidence_brain_module).get(
            "_provider_request_snapshot"
        )
        is not _FROZEN_PROVIDER_REQUEST_SNAPSHOT
        or vars(_autonomous_evidence_brain_module).get(
            "_ProviderDispatchFenceObserver"
        )
        is not _PROVIDER_DISPATCH_FENCE_OBSERVER
        or vars(_autonomous_evidence_brain_module).get(
            "CompositeProviderInvocationObserver"
        )
        is not CompositeProviderInvocationObserver
        or vars(_brain_module).get("_compose_provider_observers")
        is not _FROZEN_PROVIDER_OBSERVER_COMPOSER
        or vars(_brain_module).get("CompositeProviderInvocationObserver")
        is not CompositeProviderInvocationObserver
        or vars(_llm_runtime_module).get(
            "CompositeProviderInvocationObserver"
        )
        is not CompositeProviderInvocationObserver
        or any(
            not _behavior_value_matches(
                vars(_autonomous_evidence_brain_module).get(name),
                expected,
            )
            for name, expected in _FROZEN_EVIDENCE_BRAIN_TRUST_GLOBAL_STATES.items()
        )
        or any(
            not _function_behavior_matches(
                vars(_autonomous_evidence_brain_module).get(name),
                expected,
            )
            for name, expected in _FROZEN_EVIDENCE_BRAIN_TRUST_FUNCTION_BEHAVIORS.items()
        )
        or any(
            vars(module).get(name) is not expected
            or (
                expected_behavior is not None
                and not _function_behavior_matches(
                    vars(module).get(name),
                    expected_behavior,
                )
            )
            for (
                module,
                name,
                expected,
                expected_behavior,
            ) in _FROZEN_EVIDENCE_BRAIN_MODULE_DEPENDENCIES
        )
        or not _function_behavior_matches(
            _FROZEN_EVIDENCE_RUNNER,
            _FROZEN_DISPATCH_FUNCTION_BEHAVIORS["evidence_runner"],
        )
        or not _function_behavior_matches(
            _FROZEN_PROVIDER_EXECUTION_SNAPSHOT,
            _FROZEN_DISPATCH_FUNCTION_BEHAVIORS[
                "provider_execution_snapshot"
            ],
        )
        or not _function_behavior_matches(
            _FROZEN_EXECUTION_METADATA,
            _FROZEN_DISPATCH_FUNCTION_BEHAVIORS["execution_metadata"],
        )
        or not _function_behavior_matches(
            _FROZEN_PROVIDER_REQUEST_SNAPSHOT,
            _FROZEN_DISPATCH_FUNCTION_BEHAVIORS[
                "provider_request_snapshot"
            ],
        )
        or not _function_behavior_matches(
            _FROZEN_PROVIDER_OBSERVER_COMPOSER,
            _FROZEN_DISPATCH_FUNCTION_BEHAVIORS[
                "provider_observer_composer"
            ],
        )
        or not _surface_matches(
            _PROVIDER_DISPATCH_FENCE_OBSERVER,
            _FROZEN_CORE_CALLABLE_SURFACES[
                _PROVIDER_DISPATCH_FENCE_OBSERVER
            ],
        )
    ):
        raise ArgumentError(
            "resumable provider fencing rejects modified dispatch-fence composition"
        )
    # Validate effective class lookup before reading any instance edge.  In particular, a newly
    # added ``__getattribute__`` or data descriptor must never get an opportunity to execute while
    # the fence is deciding whether the graph is trusted.
    if not _frozen_callable_surface_is_intact(
        agent, AutonomousAgent
    ) or any(
        not _exact_bound_method(agent, name, expected)
        for name, expected in _AUTONOMOUS_AGENT_METHODS.items()
    ):
        raise ArgumentError(
            "resumable provider fencing rejects overridden AutonomousAgent execution methods"
        )
    agent_values = _exact_instance_dict(agent)
    if agent_values is None:
        raise ArgumentError(
            "resumable provider fencing requires ordinary core agent instance state"
        )
    runtime = agent_values.get("runtime")
    brain = agent_values.get("brain")
    orchestrator = agent_values.get("orchestrator")
    if (
        type(runtime) is not LLMRuntime
        or type(brain) is not AutonomousBrain
        or type(orchestrator) is not AutonomousTaskOrchestrator
    ):
        raise ArgumentError(
            "resumable provider fencing requires exact core agent, orchestrator, brain, and runtime types"
        )
    if (
        not _frozen_callable_surface_is_intact(runtime, LLMRuntime)
        or not _surface_matches(
            CompositeProviderInvocationObserver,
            _FROZEN_CORE_CALLABLE_SURFACES[
                CompositeProviderInvocationObserver
            ],
        )
        or not _frozen_callable_surface_is_intact(brain, AutonomousBrain)
        or not _frozen_callable_surface_is_intact(
            orchestrator,
            AutonomousTaskOrchestrator,
        )
        or any(
            not _exact_bound_method(runtime, name, expected)
            for name, expected in _LLM_RUNTIME_METHODS.items()
        )
        or any(
            not _exact_bound_method(brain, name, expected)
            for name, expected in _AUTONOMOUS_BRAIN_METHODS.items()
        )
        or any(
            not _exact_bound_method(orchestrator, name, expected)
            for name, expected in _AUTONOMOUS_ORCHESTRATOR_METHODS.items()
        )
    ):
        raise ArgumentError(
            "resumable provider fencing rejects overridden provider dispatch-chain methods"
        )
    runtime_values = _exact_instance_dict(runtime)
    brain_values = _exact_instance_dict(brain)
    orchestrator_values = _exact_instance_dict(orchestrator)
    if (
        runtime_values is None
        or brain_values is None
        or orchestrator_values is None
        or brain_values.get("runtime") is not runtime
        or orchestrator_values.get("brain") is not brain
    ):
        raise ArgumentError(
            "resumable provider fencing requires an intact agent-to-runtime dispatch graph"
        )
    explicit_boundary = (
        None if run_options is None else run_options.get("effect_boundary")
    )
    for boundary in (
        agent_values.get("effect_boundary"),
        runtime_values.get("_effect_boundary"),
        explicit_boundary,
    ):
        if boundary is not None and (
            type(boundary) is not AutonomousEffectBoundary
            or not _frozen_callable_surface_is_intact(
                boundary,
                AutonomousEffectBoundary,
            )
            or not _exact_bound_method(
                boundary,
                "execute",
                _AUTONOMOUS_EFFECT_BOUNDARY_METHODS["execute"],
            )
            or not _exact_bound_method(
                boundary,
                "execute_stream",
                _AUTONOMOUS_EFFECT_BOUNDARY_METHODS["execute_stream"],
            )
        ):
            raise ArgumentError(
                "resumable provider fencing accepts only an unmodified exact built-in AutonomousEffectBoundary"
            )


@dataclass(frozen=True, slots=True)
class _ProviderRegistrationFence:
    provider: str
    config: ProviderConfig
    scalar_values: tuple[tuple[str, type[Any], Any], ...]
    transport: InMemoryProvider | None
    transport_provider: str | None
    handler: Any | None
    stream_handler: Any | None
    discovery_handler: Any | None


@dataclass(frozen=True, slots=True)
class _CredentialRegistrationFence:
    credential_id: str
    entry: Any
    provider: str
    secret: SecretValue
    secret_digest: str
    expires_at_type: type[Any] | None
    expires_at: float | int | None
    source: str


@dataclass(frozen=True, slots=True)
class _ProviderTransportGraphFence:
    runtime: LLMRuntime
    credentials: CredentialStore
    credential_clock: Any
    credential_entries: dict[str, Any]
    credential_lock: Any
    credential_maximum: int
    credential_registrations: tuple[_CredentialRegistrationFence, ...]
    providers: dict[str, ProviderConfig]
    circuits: dict[str, Any]
    clock: Any
    sleeper: Any
    provider_quota: Any
    registrations: tuple[_ProviderRegistrationFence, ...]


def _assert_frozen_http_transport_bindings() -> None:
    """Reject late replacement of any dependency used after the private dispatch fence."""

    http_package = vars(_llm_runtime_module).get("http")
    if (
        http_package is not _FROZEN_LLM_RUNTIME_HTTP_PACKAGE
        or vars(http_package).get("client") is not _FROZEN_HTTP_CLIENT_MODULE
        or vars(_FROZEN_HTTP_CLIENT_MODULE).get("HTTPConnection")
        is not _FROZEN_HTTP_CONNECTION
        or vars(_FROZEN_HTTP_CLIENT_MODULE).get("HTTPSConnection")
        is not _FROZEN_HTTPS_CONNECTION
        or vars(_FROZEN_HTTP_CLIENT_MODULE).get("socket")
        is not _FROZEN_HTTP_SOCKET_MODULE
        or _FROZEN_HTTP_SOCKET_MODULE is not _FROZEN_SOCKET_MODULE
        or vars(_FROZEN_SOCKET_MODULE).get("create_connection")
        is not _FROZEN_SOCKET_CREATE_CONNECTION
        or vars(_FROZEN_SOCKET_MODULE).get("getaddrinfo")
        is not _FROZEN_SOCKET_GETADDRINFO
        or vars(_FROZEN_SOCKET_MODULE).get("socket") is not _FROZEN_SOCKET_CLASS
        or vars(_FROZEN_SOCKET_MODULE).get("_socket")
        is not _FROZEN_LOW_LEVEL_SOCKET_MODULE
        or (
            _FROZEN_LOW_LEVEL_SOCKET_MODULE is not None
            and vars(_FROZEN_LOW_LEVEL_SOCKET_MODULE).get("getaddrinfo")
            is not _FROZEN_LOW_LEVEL_GETADDRINFO
        )
        or vars(_FROZEN_HTTP_CLIENT_MODULE).get("_create_https_context")
        is not _FROZEN_HTTP_CREATE_HTTPS_CONTEXT
        or vars(_stdlib_ssl).get("SSLContext") is not _FROZEN_SSL_CONTEXT_CLASS
        or vars(_stdlib_ssl).get("_create_default_https_context")
        is not _FROZEN_SSL_DEFAULT_HTTPS_CONTEXT
        or vars(_llm_runtime_module).get("json") is not _FROZEN_JSON_MODULE
        or vars(_FROZEN_JSON_MODULE).get("dumps") is not _FROZEN_JSON_DUMPS
        or vars(_FROZEN_JSON_MODULE).get("JSONEncoder")
        is not _FROZEN_JSON_ENCODER
        or vars(_FROZEN_JSON_MODULE).get("encoder")
        is not _FROZEN_JSON_ENCODER_MODULE
        or any(
            vars(_FROZEN_JSON_ENCODER_MODULE).get(name) is not expected
            for name, expected in _FROZEN_JSON_ENCODER_GLOBALS.items()
        )
        or not _function_behavior_matches(
            vars(_FROZEN_JSON_MODULE).get("dumps"),
            _FROZEN_JSON_DUMPS_BEHAVIOR,
        )
        or (
            _FROZEN_SOCKET_CREATE_CONNECTION_BEHAVIOR is not None
            and not _function_behavior_matches(
                vars(_FROZEN_SOCKET_MODULE).get("create_connection"),
                _FROZEN_SOCKET_CREATE_CONNECTION_BEHAVIOR,
            )
        )
        or (
            _FROZEN_SOCKET_CREATE_CONNECTION_GLOBALS is not None
            and not _referenced_function_globals_match(
                _FROZEN_SOCKET_CREATE_CONNECTION,
                _FROZEN_SOCKET_CREATE_CONNECTION_GLOBALS,
            )
        )
        or (
            _FROZEN_SOCKET_GETADDRINFO_BEHAVIOR is not None
            and not _function_behavior_matches(
                vars(_FROZEN_SOCKET_MODULE).get("getaddrinfo"),
                _FROZEN_SOCKET_GETADDRINFO_BEHAVIOR,
            )
        )
        or (
            _FROZEN_SOCKET_GETADDRINFO_GLOBALS is not None
            and not _referenced_function_globals_match(
                _FROZEN_SOCKET_GETADDRINFO,
                _FROZEN_SOCKET_GETADDRINFO_GLOBALS,
            )
        )
        or (
            _FROZEN_HTTP_CREATE_HTTPS_CONTEXT_BEHAVIOR is not None
            and not _function_behavior_matches(
                vars(_FROZEN_HTTP_CLIENT_MODULE).get(
                    "_create_https_context"
                ),
                _FROZEN_HTTP_CREATE_HTTPS_CONTEXT_BEHAVIOR,
            )
        )
        or (
            _FROZEN_HTTP_CREATE_HTTPS_CONTEXT_GLOBALS is not None
            and not _referenced_function_globals_match(
                _FROZEN_HTTP_CREATE_HTTPS_CONTEXT,
                _FROZEN_HTTP_CREATE_HTTPS_CONTEXT_GLOBALS,
            )
        )
        or (
            _FROZEN_SSL_DEFAULT_HTTPS_CONTEXT_BEHAVIOR is not None
            and not _function_behavior_matches(
                vars(_FROZEN_SSL_MODULE).get(
                    "_create_default_https_context"
                ),
                _FROZEN_SSL_DEFAULT_HTTPS_CONTEXT_BEHAVIOR,
            )
        )
        or (
            _FROZEN_SSL_DEFAULT_HTTPS_CONTEXT_GLOBALS is not None
            and not _referenced_function_globals_match(
                _FROZEN_SSL_DEFAULT_HTTPS_CONTEXT,
                _FROZEN_SSL_DEFAULT_HTTPS_CONTEXT_GLOBALS,
            )
        )
        or any(
            not _function_behavior_matches(
                vars(_FROZEN_JSON_ENCODER_MODULE).get(name),
                expected,
            )
            for name, expected in _FROZEN_JSON_ENCODER_GLOBAL_BEHAVIORS.items()
        )
        or any(
            vars(_llm_runtime_module).get(name) is not expected
            for name, expected in _FROZEN_LLM_RUNTIME_DISPATCH_GLOBALS.items()
        )
        or any(
            not _function_behavior_matches(
                vars(_llm_runtime_module).get(name),
                expected,
            )
            for name, expected in _FROZEN_LLM_RUNTIME_DISPATCH_FUNCTION_BEHAVIORS.items()
        )
        or any(
            not _surface_matches(
                value_type,
                _FROZEN_CORE_CALLABLE_SURFACES[value_type],
            )
            for value_type in (
                ProviderInvocationMetadata,
                ProviderRequest,
                ProviderTool,
                SecretValue,
                _CREDENTIAL_ENTRY,
                _FROZEN_HTTP_CONNECTION,
                _FROZEN_HTTPS_CONNECTION,
                _FROZEN_SOCKET_CLASS,
                _FROZEN_SSL_CONTEXT_CLASS,
                _FROZEN_JSON_ENCODER,
            )
        )
    ):
        raise ArgumentError(
            "resumable provider HTTP transport factories or dispatch dependencies changed after the SDK trust root was established"
        )


def _provider_config_scalar_values(
    config: ProviderConfig,
) -> tuple[tuple[str, type[Any], Any], ...]:
    values: list[tuple[str, type[Any], Any]] = []
    for name in _FROZEN_PROVIDER_CONFIG_FIELDS:
        if name == "transport":
            continue
        value = object.__getattribute__(config, name)
        if name in _PROVIDER_CONFIG_TEXT_FIELDS:
            if value is not None and type(value) is not str:
                raise ArgumentError(
                    "resumable provider fencing requires normalized provider configuration text"
                )
        elif name in _PROVIDER_CONFIG_BOOLEAN_FIELDS:
            if type(value) is not bool:
                raise ArgumentError(
                    "resumable provider fencing requires normalized provider configuration flags"
                )
        elif name in _PROVIDER_CONFIG_INTEGER_FIELDS:
            if type(value) is not int:
                raise ArgumentError(
                    "resumable provider fencing requires normalized provider configuration bounds"
                )
        elif name in _PROVIDER_CONFIG_NUMBER_FIELDS:
            if type(value) not in {int, float}:
                raise ArgumentError(
                    "resumable provider fencing requires normalized provider configuration numbers"
                )
        else:
            raise ArgumentError(
                "resumable provider fencing encountered an unrecognized provider configuration field"
            )
        values.append((name, type(value), value))
    return tuple(values)


def _provider_config_snapshot_matches(
    value: Any,
    expected: tuple[tuple[str, type[Any], Any], ...],
) -> bool:
    """Compare private runtime attestation without invoking attacker-defined equality."""

    if type(value) is not tuple or len(value) != len(expected):
        return False
    for actual, reference in zip(value, expected):
        if type(actual) is not tuple or len(actual) != 3:
            return False
        actual_name, actual_type, actual_value = actual
        expected_name, expected_type, expected_value = reference
        if (
            type(actual_name) is not str
            or actual_name != expected_name
            or actual_type is not expected_type
            or type(actual_value) is not expected_type
            or actual_value != expected_value
        ):
            return False
    return True


def _capture_provider_registration(
    provider: Any,
    config: Any,
) -> _ProviderRegistrationFence:
    if type(provider) is not str or type(config) is not ProviderConfig:
        raise ArgumentError(
            "resumable provider fencing requires exact immutable ProviderConfig registrations"
        )
    if (
        vars(_llm_runtime_module).get("ProviderConfig") is not ProviderConfig
        or not _surface_matches(
            ProviderConfig,
            _FROZEN_CORE_CALLABLE_SURFACES[ProviderConfig],
        )
    ):
        raise ArgumentError(
            "resumable provider fencing rejects a modified ProviderConfig type"
        )
    scalar_values = _provider_config_scalar_values(config)
    configured_provider = object.__getattribute__(config, "provider")
    if configured_provider != provider:
        raise ArgumentError(
            "resumable provider registry key does not match its configuration"
        )
    transport = object.__getattribute__(config, "transport")
    if transport is None:
        return _ProviderRegistrationFence(
            provider,
            config,
            scalar_values,
            None,
            None,
            None,
            None,
            None,
        )
    if (
        type(transport) is not InMemoryProvider
        or vars(_llm_runtime_module).get("InMemoryProvider") is not InMemoryProvider
        or not _frozen_callable_surface_is_intact(
            transport,
            InMemoryProvider,
        )
    ):
        raise ArgumentError(
            "resumable provider fencing accepts only the exact built-in in-memory transport or stdlib HTTP"
        )
    transport_values = _exact_instance_dict(transport)
    if transport_values is None:
        raise ArgumentError(
            "resumable provider fencing requires ordinary in-memory transport state"
        )
    transport_provider = transport_values.get("provider")
    handler = transport_values.get("_handler")
    stream_handler = transport_values.get("_stream_handler")
    discovery_handler = transport_values.get("_model_discovery_handler")
    if (
        type(transport_provider) is not str
        or transport_provider != provider
        or not callable(handler)
        or (stream_handler is not None and not callable(stream_handler))
        or (discovery_handler is not None and not callable(discovery_handler))
    ):
        raise ArgumentError(
            "resumable provider fencing found malformed in-memory transport handlers"
        )
    return _ProviderRegistrationFence(
        provider,
        config,
        scalar_values,
        transport,
        transport_provider,
        handler,
        stream_handler,
        discovery_handler,
    )


def _capture_credential_registration(
    credential_id: Any,
    entry: Any,
) -> _CredentialRegistrationFence:
    if (
        type(credential_id) is not str
        or not credential_id
        or len(credential_id.encode("utf-8")) > 512
        or type(entry) is not _CREDENTIAL_ENTRY
        or not _frozen_callable_surface_is_intact(entry, _CREDENTIAL_ENTRY)
    ):
        raise ArgumentError(
            "resumable provider fencing found malformed credential state"
        )
    provider = object.__getattribute__(entry, "provider")
    secret = object.__getattribute__(entry, "secret")
    expires_at = object.__getattribute__(entry, "expires_at")
    source = object.__getattribute__(entry, "source")
    if (
        type(provider) is not str
        or not provider
        or len(provider.encode("utf-8")) > 256
        or type(secret) is not SecretValue
        or not _frozen_callable_surface_is_intact(secret, SecretValue)
        or (
            expires_at is not None
            and (
                type(expires_at) not in {int, float}
                or expires_at != expires_at
                or expires_at in {float("inf"), float("-inf")}
            )
        )
        or type(source) is not str
        or not source
        or len(source.encode("utf-8")) > 128
    ):
        raise ArgumentError(
            "resumable provider fencing found malformed credential state"
        )
    secret_value = object.__getattribute__(secret, "_value")
    if (
        type(secret_value) is not str
        or not secret_value
        or len(secret_value.encode("utf-8")) > MAX_CREDENTIAL_BYTES
    ):
        raise ArgumentError(
            "resumable provider fencing found malformed credential secret state"
        )
    return _CredentialRegistrationFence(
        credential_id=credential_id,
        entry=entry,
        provider=provider,
        secret=secret,
        secret_digest=content_digest({"secret_value": secret_value}),
        expires_at_type=None if expires_at is None else type(expires_at),
        expires_at=expires_at,
        source=source,
    )


def _capture_provider_transport_graph(
    agent: AutonomousAgent,
    *,
    run_options: Mapping[str, Any] | None,
) -> _ProviderTransportGraphFence:
    """Capture every mutable edge that selects the concrete provider transport."""

    _assert_resumable_core_runtime(agent, run_options=run_options)
    _assert_frozen_http_transport_bindings()
    agent_values = _exact_instance_dict(agent)
    assert agent_values is not None
    runtime = agent_values["runtime"]
    runtime_values = _exact_instance_dict(runtime)
    assert runtime_values is not None
    credentials = runtime_values.get("credentials")
    providers = runtime_values.get("_providers")
    circuits = runtime_values.get("_circuits")
    clock = runtime_values.get("_clock")
    sleeper = runtime_values.get("_sleeper")
    provider_quota = runtime_values.get("_provider_quota")
    if (
        type(credentials) is not CredentialStore
        or not _frozen_callable_surface_is_intact(credentials, CredentialStore)
        or type(providers) is not dict
        or type(circuits) is not dict
        or not callable(clock)
        or not callable(sleeper)
    ):
        raise ArgumentError(
            "resumable provider fencing requires a concrete built-in provider transport graph"
        )
    credential_values = _exact_instance_dict(credentials)
    if (
        credential_values is None
        or type(credential_values.get("_entries")) is not dict
        or not callable(credential_values.get("_clock"))
        or type(credential_values.get("_max_credentials")) is not int
        or credential_values.get("_max_credentials") <= 0
        or credential_values.get("_lock") is None
    ):
        raise ArgumentError(
            "resumable provider fencing requires ordinary built-in credential state"
        )
    try:
        entries = tuple(dict.items(providers))
    except RuntimeError as error:
        raise ArgumentError(
            "resumable provider registry changed while it was being snapshotted"
        ) from error
    registrations = tuple(
        _capture_provider_registration(provider, config)
        for provider, config in entries
    )
    credential_entries = credential_values["_entries"]
    try:
        credential_items = tuple(dict.items(credential_entries))
    except RuntimeError as error:
        raise ArgumentError(
            "resumable credential store changed while it was being snapshotted"
        ) from error
    credential_registrations = tuple(
        _capture_credential_registration(credential_id, entry)
        for credential_id, entry in credential_items
    )
    return _ProviderTransportGraphFence(
        runtime=runtime,
        credentials=credentials,
        credential_clock=credential_values["_clock"],
        credential_entries=credential_entries,
        credential_lock=credential_values["_lock"],
        credential_maximum=credential_values["_max_credentials"],
        credential_registrations=credential_registrations,
        providers=providers,
        circuits=circuits,
        clock=clock,
        sleeper=sleeper,
        provider_quota=provider_quota,
        registrations=registrations,
    )


def _assert_provider_transport_graph(
    agent: AutonomousAgent,
    expected: _ProviderTransportGraphFence,
    *,
    run_options: Mapping[str, Any] | None,
) -> None:
    """Revalidate the transport graph without invoking caller-controlled agent lookup."""

    _assert_resumable_core_runtime(agent, run_options=run_options)
    _assert_frozen_http_transport_bindings()
    agent_values = _exact_instance_dict(agent)
    assert agent_values is not None
    runtime = agent_values["runtime"]
    runtime_values = _exact_instance_dict(runtime)
    assert runtime_values is not None
    credentials = runtime_values.get("credentials")
    credential_values = (
        None if credentials is None else _exact_instance_dict(credentials)
    )
    if (
        runtime is not expected.runtime
        or credentials is not expected.credentials
        or type(credentials) is not CredentialStore
        or not _frozen_callable_surface_is_intact(
            credentials,
            CredentialStore,
        )
        or vars(_llm_runtime_module).get("CredentialHandle")
        is not CredentialHandle
        or not _surface_matches(
            CredentialHandle,
            _FROZEN_CORE_CALLABLE_SURFACES[CredentialHandle],
        )
        or credential_values is None
        or credential_values.get("_clock") is not expected.credential_clock
        or credential_values.get("_entries") is not expected.credential_entries
        or credential_values.get("_lock") is not expected.credential_lock
        or credential_values.get("_max_credentials")
        != expected.credential_maximum
        or runtime_values.get("_providers") is not expected.providers
        or runtime_values.get("_circuits") is not expected.circuits
        or runtime_values.get("_clock") is not expected.clock
        or runtime_values.get("_sleeper") is not expected.sleeper
        or runtime_values.get("_provider_quota") is not expected.provider_quota
    ):
        raise ArgumentError(
            "resumable provider transport graph changed after its policy snapshot"
        )
    try:
        credential_items = tuple(dict.items(expected.credential_entries))
    except RuntimeError as error:
        raise ArgumentError(
            "resumable credential store changed while it was being checked"
        ) from error
    if len(credential_items) != len(expected.credential_registrations):
        raise ArgumentError(
            "resumable credential store changed after its policy snapshot"
        )
    for (credential_id, entry), registration in zip(
        credential_items,
        expected.credential_registrations,
    ):
        if credential_id != registration.credential_id or entry is not registration.entry:
            raise ArgumentError(
                "resumable credential store changed after its policy snapshot"
            )
        current_credential = _capture_credential_registration(
            credential_id,
            entry,
        )
        if (
            current_credential.provider != registration.provider
            or current_credential.secret is not registration.secret
            or current_credential.secret_digest != registration.secret_digest
            or current_credential.expires_at_type
            is not registration.expires_at_type
            or current_credential.expires_at != registration.expires_at
            or current_credential.source != registration.source
        ):
            raise ArgumentError(
                "resumable credential store changed after its policy snapshot"
            )
    try:
        entries = tuple(dict.items(expected.providers))
    except RuntimeError as error:
        raise ArgumentError(
            "resumable provider registry changed while it was being checked"
        ) from error
    if len(entries) != len(expected.registrations):
        raise ArgumentError(
            "resumable provider registry changed after its policy snapshot"
        )
    for (provider, config), registration in zip(
        entries,
        expected.registrations,
    ):
        if (
            type(provider) is not str
            or type(config) is not ProviderConfig
            or provider != registration.provider
            or config is not registration.config
        ):
            raise ArgumentError(
                "resumable provider registry changed after its policy snapshot"
            )
        current = _capture_provider_registration(provider, config)
        if (
            current.scalar_values != registration.scalar_values
            or current.transport is not registration.transport
            or current.transport_provider != registration.transport_provider
            or current.handler is not registration.handler
            or current.stream_handler is not registration.stream_handler
            or current.discovery_handler is not registration.discovery_handler
        ):
            raise ArgumentError(
                "resumable provider transport graph changed after its policy snapshot"
            )


def _snapshot_provider_credentials(
    value: Any,
    credential_store: CredentialStore,
) -> dict[str, CredentialHandle]:
    """Detach the provider-to-handle map without copying or serializing any secret."""

    if type(value) is not dict:
        raise ArgumentError(
            "resumable provider fencing requires credentials to be a plain provider-handle dict"
        )
    if (
        vars(_llm_runtime_module).get("CredentialHandle") is not CredentialHandle
        or not _surface_matches(
            CredentialHandle,
            _FROZEN_CORE_CALLABLE_SURFACES[CredentialHandle],
        )
    ):
        raise ArgumentError(
            "resumable provider fencing rejects a modified CredentialHandle type"
        )
    snapshot: dict[str, CredentialHandle] = {}
    try:
        entries = tuple(dict.items(value))
    except RuntimeError as error:
        raise ArgumentError(
            "resumable credential mapping changed while it was being snapshotted"
        ) from error
    for provider, handle in entries:
        if type(provider) is not str or type(handle) is not CredentialHandle:
            raise ArgumentError(
                "resumable credentials must map provider names to exact opaque handles"
            )
        handle_provider = object.__getattribute__(handle, "provider")
        credential_id = object.__getattribute__(handle, "credential_id")
        handle_store = object.__getattribute__(handle, "_store")
        if (
            type(handle_provider) is not str
            or handle_provider != provider
            or type(credential_id) is not str
            or handle_store is not credential_store
        ):
            raise ArgumentError(
                "resumable credential handle does not belong to the validated runtime store"
            )
        snapshot[provider] = CredentialHandle(
            handle_provider,
            credential_id,
            credential_store,
        )
    return snapshot


def _explicit_component_identity(name: str, value: Any) -> dict[str, str | None]:
    if not isinstance(value, Mapping) or set(value) - {"id", "version", "config_digest"}:
        raise ArgumentError(
            f"evidence-backed resumable policy identity {name} must contain only id, version, and optional config_digest"
        )
    if "id" not in value or "version" not in value:
        raise ArgumentError(
            f"evidence-backed resumable policy identity {name} requires id and version"
        )
    return {
        "id": _identifier(f"evidence-backed resumable policy identity {name} id", value.get("id")),
        "version": _identifier(
            f"evidence-backed resumable policy identity {name} version", value.get("version")
        ),
        "config_digest": _digest(
            f"evidence-backed resumable policy identity {name} config_digest",
            value.get("config_digest"),
            allow_none=True,
        ),
    }


def _normalize_explicit_policy_identity(value: Any) -> dict[str, dict[str, str | None]]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ArgumentError("evidence-backed resumable_policy_identity must be a mapping")
    unknown = sorted(
        str(key)
        for key in value
        if not isinstance(key, str) or key not in _RESUMABLE_POLICY_IDENTITY_ROLES
    )
    if unknown:
        raise ArgumentError(
            "evidence-backed resumable_policy_identity contains unsupported roles: "
            + ", ".join(unknown)
        )
    return {
        role: _explicit_component_identity(role, value[role])
        for role in _RESUMABLE_POLICY_IDENTITY_ROLES
        if role in value
    }


def _code_constant(value: Any, *, depth: int = 0) -> Any:
    """Return a bounded immutable code/default/closure projection.

    Mutable closure state is deliberately not guessed. A caller that intentionally uses it must
    provide an explicit role identity and bind that state through ``config_digest``.
    """

    if depth > 12:
        raise ArgumentError("evidence-backed callable identity is too deeply nested")
    if value is None or isinstance(value, (str, int, bool)):
        return value
    if isinstance(value, float):
        if value != value or value in {float("inf"), float("-inf")}:
            return {"float": repr(value)}
        return value
    if isinstance(value, bytes):
        return {"bytes_digest": content_digest({"hex": value.hex()})}
    if isinstance(value, complex):
        return {"complex": [repr(value.real), repr(value.imag)]}
    if value is Ellipsis:
        return {"constant": "ellipsis"}
    if isinstance(value, tuple):
        return [_code_constant(child, depth=depth + 1) for child in value]
    if isinstance(value, frozenset):
        children = [_code_constant(child, depth=depth + 1) for child in value]
        return sorted(children, key=canonical_json)
    if isinstance(value, types.CodeType):
        return _code_descriptor(value, depth=depth + 1)
    raise ArgumentError(
        "evidence-backed callable captures mutable or opaque state; provide an explicit "
        "resumable_policy_identity entry with a config_digest"
    )


def _code_descriptor(code: types.CodeType, *, depth: int = 0) -> dict[str, Any]:
    if depth > 12:
        raise ArgumentError("evidence-backed callable code is too deeply nested")
    return {
        "bytecode": code.co_code.hex(),
        "constants": [_code_constant(value, depth=depth + 1) for value in code.co_consts],
        "names": list(code.co_names),
        "varnames": list(code.co_varnames),
        "freevars": list(code.co_freevars),
        "cellvars": list(code.co_cellvars),
        "argcount": code.co_argcount,
        "posonlyargcount": code.co_posonlyargcount,
        "kwonlyargcount": code.co_kwonlyargcount,
        "flags": code.co_flags,
        "exceptiontable": getattr(code, "co_exceptiontable", b"").hex(),
    }


def _function_identity(
    value: types.FunctionType,
    *,
    explicit_config_bound: bool,
) -> dict[str, str | None]:
    closure: Any
    if value.__closure__ is None:
        closure = None
    elif explicit_config_bound:
        closure = "caller_config_digest"
    else:
        closure = [
            _code_constant(cell.cell_contents)
            for cell in value.__closure__
        ]
    defaults = (
        "caller_config_digest"
        if explicit_config_bound and value.__defaults__
        else _code_constant(value.__defaults__)
    )
    kwdefaults = (
        "caller_config_digest"
        if explicit_config_bound and value.__kwdefaults__
        else _code_constant(
            None
            if value.__kwdefaults__ is None
            else tuple(sorted(value.__kwdefaults__.items()))
        )
    )
    implementation = {
        "module": value.__module__,
        "qualname": value.__qualname__,
        "code": _code_descriptor(value.__code__),
        "defaults": defaults,
        "kwdefaults": kwdefaults,
        "closure": closure,
    }
    location_digest = content_digest(
        {"module": value.__module__, "qualname": value.__qualname__}
    )
    return {
        "id": f"python-function-{location_digest[:24]}",
        "version": f"sha256-{content_digest(implementation)}",
        "config_digest": None,
    }


def _declared_component_identity(role: str, value: Any) -> dict[str, str | None] | None:
    id_names = [f"{role}_id", "resumable_id"]
    version_names = [f"{role}_version", "resumable_version"]
    if role in {"acquirer", "projector"}:
        id_names.append("adapter_id")
        version_names.extend(("adapter_version", "version"))
    elif role == "evaluator":
        id_names.append("evaluator_id")
        version_names.append("evaluator_version")
    elif role == "prompt_builder":
        id_names.append("builder_id")
        version_names.append("builder_version")

    def first(names: Sequence[str]) -> Any:
        for attribute in names:
            candidate = getattr(value, attribute, None)
            if candidate is not None:
                return candidate
        return None

    identity = first(id_names)
    version = first(version_names)
    if identity is None and version is None:
        return None
    if identity is None or version is None:
        raise ArgumentError(
            f"evidence-backed {role} declares an incomplete stable identity; both id and version are required"
        )
    config_digest = first(
        (
            f"{role}_config_digest",
            "manifest_digest",
            "registry_digest",
            "config_digest",
        )
    )
    return {
        "id": _identifier(f"evidence-backed {role} id", identity),
        "version": _identifier(f"evidence-backed {role} version", version),
        "config_digest": _digest(
            f"evidence-backed {role} config digest", config_digest, allow_none=True
        ),
    }


def _component_policy_identity(
    role: str,
    value: Any,
    explicit: dict[str, str | None] | None,
) -> dict[str, Any] | None:
    if role == "provider_policy":
        if explicit is None or explicit.get("config_digest") is None:
            raise ArgumentError(
                "resumable evidence-backed execution requires provider_policy identity "
                "with config_digest for caller-owned and agent-owned provider state"
            )
        return {
            "declared": None,
            "implementation": None,
            "caller": explicit,
        }
    if value is None:
        if role == "value_rehydrator" and explicit is not None:
            # A value rehydrator is commonly unavailable before the first process restart.
            # Supplying the same explicit identity on both runs reserves and binds that recovery
            # policy without pretending the absent callback can be introspected.
            return {"declared": None, "implementation": None, "caller": explicit}
        if explicit is not None:
            raise ArgumentError(
                f"evidence-backed resumable policy identity declares absent component {role}"
            )
        return None
    if role == "value_rehydrator" and explicit is not None:
        return {"declared": None, "implementation": None, "caller": explicit}
    bound_method = isinstance(value, types.MethodType)
    identity_source = value.__self__ if bound_method else value
    declared = _declared_component_identity(role, identity_source)
    implementation = None
    function = value.__func__ if bound_method else value
    if isinstance(function, types.FunctionType):
        implementation = _function_identity(
            function,
            explicit_config_bound=explicit is not None
            and explicit.get("config_digest") is not None,
        )
    if (
        (declared is None and implementation is None and explicit is None)
        or (bound_method and declared is None and explicit is None)
    ):
        raise ArgumentError(
            f"evidence-backed resumable {role} has no deterministic stable identity; provide "
            f"resumable_policy_identity.{role} with id, version, and a config_digest when state affects behavior"
        )
    return {
        "declared": declared,
        "implementation": implementation,
        "caller": explicit,
    }


def _resumable_policy_identity(
    *,
    acquirer: Any,
    projector: Any,
    evaluator: Any,
    value_rehydrator: Any,
    prompt_builder: Any,
    explicit: Any,
) -> dict[str, Any]:
    supplied = _normalize_explicit_policy_identity(explicit)
    values = {
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": evaluator,
        "value_rehydrator": value_rehydrator,
        "prompt_builder": prompt_builder,
        "provider_policy": None,
    }
    resolved = {
        role: _component_policy_identity(role, values[role], supplied.get(role))
        for role in _RESUMABLE_POLICY_IDENTITY_ROLES
    }
    _json_bytes(resolved, "evidence-backed resumable policy identity")
    return resolved


def _request_digest(requests: Sequence[Mapping[str, Any]]) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
            "requests": [_policy_value(request) for request in requests],
        }
    )


def _model_policy_value(candidates: Sequence[Any] | None) -> Any:
    if candidates is None:
        return None
    return [
        _provider_policy_value(
            candidate,
            name=f"evidence-backed model_candidates[{index}]",
        )
        for index, candidate in enumerate(candidates)
    ]


def _run_policy_digest(
    *,
    domains: Sequence[str],
    model_candidates: Sequence[Any] | None,
    run_mode: str,
    run_options: Mapping[str, Any] | None,
    approve_source_dispatch: bool,
    allow_incomplete_evidence: bool,
    prompt_builder: Any,
    component_identity: Mapping[str, Any],
    available_evidence: Sequence[str],
    completed_stages: Mapping[str, Sequence[str]] | None,
    parent_evidence_digests: Sequence[str],
    stop_on_failure: bool,
    reevaluate_pending: bool,
) -> str:
    payload = {
        "schema": AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
        "domains": list(domains),
        "model_candidates": _model_policy_value(model_candidates),
        "run_mode": run_mode,
        "run_options": _provider_policy_value(
            {} if run_options is None else run_options,
            name="evidence-backed run_options",
            trust_opaque=True,
        ),
        # Approval is fresh transition authority, not source/provider-shaping policy. A held
        # evidence-review checkpoint must remain resumable after the caller grants it later.
        "approve_source_dispatch": "managed_transition_control",
        "allow_incomplete_evidence": allow_incomplete_evidence,
        "prompt_builder_configured": prompt_builder is not None,
        "component_identity": _policy_value(component_identity),
        "available_evidence": list(available_evidence),
        "completed_stages": _policy_value({} if completed_stages is None else completed_stages),
        "parent_evidence_digests": list(parent_evidence_digests),
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
    }
    _json_bytes(payload, "evidence-backed run policy")
    return content_digest(payload)


def _execution_plan_digest(task_digest: str, evidence_plan_digest: str, domains: Sequence[str], run_mode: str) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
            "task_digest": task_digest,
            "evidence_plan_digest": evidence_plan_digest,
            "domains": list(domains),
            "run_mode": run_mode,
        }
    )


def _provider_result_was_observed(status: str | None) -> bool:
    return status is not None and status not in {"approval_required", "route_review_required", "abstained"} and not status.endswith("review_required")


def _provider_execution_completed(result: AutonomousEvidenceBackedRunResult) -> bool:
    status = result.execution_status
    return isinstance(status, str) and status in _PROVIDER_COMPLETION_STATUSES


def _checkpoint_provider_status(status: str | None) -> str | None:
    if status is None:
        return None
    if status in _PROVIDER_COMPLETION_STATUSES:
        return "completed"
    return status


def _checkpoint_generation(value: Any) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or not 1 <= value <= MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_GENERATION
    ):
        raise ArgumentError("evidence-backed checkpoint generation is outside its bound")
    return value


def _provider_operation_digest(
    *,
    job_id: str,
    task_digest: str,
    request_digest: str,
    run_policy_digest: str,
    evidence_plan_digest: str,
    execution_plan_digest: str,
    evidence_result_digest: str,
    prompt_projection_digest: str | None,
) -> str:
    """Derive the stable provider idempotency identity from provider-bound inputs."""

    return content_digest(
        {
            "schema": _PROVIDER_OPERATION_SCHEMA,
            "job_id": job_id,
            "task_digest": task_digest,
            "request_digest": request_digest,
            "run_policy_digest": run_policy_digest,
            "evidence_plan_digest": evidence_plan_digest,
            "execution_plan_digest": execution_plan_digest,
            "evidence_result_digest": evidence_result_digest,
            "prompt_projection_digest": prompt_projection_digest,
        }
    )


def _provider_idempotency_key(provider_operation_digest: str) -> str:
    return content_digest(
        {
            "schema": _PROVIDER_IDEMPOTENCY_SCHEMA,
            "provider_operation_digest": provider_operation_digest,
        }
    )


def _bounded_provider_text(name: str, value: Any) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or "\x00" in value
        or len(value.encode("utf-8")) > 512
    ):
        raise ArgumentError(f"{name} must be bounded non-empty text")
    return value.strip()


def _provider_idempotency_key_digest(value: str) -> str:
    return content_digest(
        {
            "schema": _PROVIDER_IDEMPOTENCY_KEY_DIGEST_SCHEMA,
            "provider_idempotency_key": value,
        }
    )


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedProviderDispatchReceipt:
    """Private exact-key receipt atomically committed with one attempt checkpoint."""

    job_id: str
    provider_operation_digest: str
    dispatch_index: int
    previous_provider_dispatch_head_digest: str | None
    provider: str
    model: str
    invocation_kind: str
    dispatch_scope_digest: str
    transport_attempt: int
    request_digest: str
    provider_idempotency_key: str = field(repr=False)

    def __post_init__(self) -> None:
        _identifier("provider dispatch receipt job_id", self.job_id)
        _digest(
            "provider dispatch receipt provider_operation_digest",
            self.provider_operation_digest,
        )
        if (
            isinstance(self.dispatch_index, bool)
            or not isinstance(self.dispatch_index, int)
            or not 1
            <= self.dispatch_index
            <= MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES
        ):
            raise ArgumentError("provider dispatch receipt index is outside its bound")
        _digest(
            "provider dispatch receipt previous head digest",
            self.previous_provider_dispatch_head_digest,
            allow_none=True,
        )
        if (self.dispatch_index == 1) != (
            self.previous_provider_dispatch_head_digest is None
        ):
            raise ArgumentError(
                "provider dispatch receipt index and previous head are inconsistent"
            )
        _bounded_provider_text("provider dispatch receipt provider", self.provider)
        _bounded_provider_text("provider dispatch receipt model", self.model)
        _bounded_provider_text(
            "provider dispatch receipt invocation_kind",
            self.invocation_kind,
        )
        _digest(
            "provider dispatch receipt dispatch_scope_digest",
            self.dispatch_scope_digest,
        )
        if (
            isinstance(self.transport_attempt, bool)
            or not isinstance(self.transport_attempt, int)
            or not 1
            <= self.transport_attempt
            <= MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES
        ):
            raise ArgumentError(
                "provider dispatch receipt transport attempt is outside its bound"
            )
        _digest("provider dispatch receipt request_digest", self.request_digest)
        _digest(
            "provider dispatch receipt provider_idempotency_key",
            self.provider_idempotency_key,
        )

    @property
    def provider_idempotency_key_digest(self) -> str:
        return _provider_idempotency_key_digest(self.provider_idempotency_key)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA,
            "job_id": self.job_id,
            "provider_operation_digest": self.provider_operation_digest,
            "dispatch_index": self.dispatch_index,
            "previous_provider_dispatch_head_digest": (
                self.previous_provider_dispatch_head_digest
            ),
            "provider": self.provider,
            "model": self.model,
            "invocation_kind": self.invocation_kind,
            "dispatch_scope_digest": self.dispatch_scope_digest,
            "transport_attempt": self.transport_attempt,
            "request_digest": self.request_digest,
            "provider_idempotency_key_digest": (
                self.provider_idempotency_key_digest
            ),
        }

    @property
    def receipt_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        """Return the public metadata projection without the exact transport key."""

        value = {
            **self._payload(),
            "receipt_digest": self.receipt_digest,
            "retention": _PROVIDER_DISPATCH_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        _json_bytes(value, "provider dispatch receipt")
        return value

    def to_private_dict(self) -> dict[str, Any]:
        """Return the caller-owned durable ledger form containing the exact key."""

        value = {
            **self.to_dict(),
            "provider_idempotency_key": self.provider_idempotency_key,
            "retention": _PROVIDER_DISPATCH_PRIVATE_RETENTION,
            "secret_material": _PROVIDER_DISPATCH_PRIVATE_MATERIAL,
        }
        _json_bytes(value, "private provider dispatch receipt")
        return value

    @classmethod
    def from_private_dict(
        cls,
        value: Mapping[str, Any],
    ) -> "AutonomousEvidenceBackedProviderDispatchReceipt":
        if not isinstance(value, Mapping):
            raise ArgumentError("private provider dispatch receipt must be a mapping")
        expected = {
            "schema",
            "job_id",
            "provider_operation_digest",
            "dispatch_index",
            "previous_provider_dispatch_head_digest",
            "provider",
            "model",
            "invocation_kind",
            "dispatch_scope_digest",
            "transport_attempt",
            "request_digest",
            "provider_idempotency_key_digest",
            "provider_idempotency_key",
            "receipt_digest",
            "retention",
            "secret_material",
        }
        if (
            set(value) != expected
            or value.get("schema")
            != AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA
        ):
            raise ArgumentError(
                "private provider dispatch receipt contains unsupported or missing fields"
            )
        if (
            value.get("retention") != _PROVIDER_DISPATCH_PRIVATE_RETENTION
            or value.get("secret_material") != _PROVIDER_DISPATCH_PRIVATE_MATERIAL
        ):
            raise ArgumentError("private provider dispatch receipt markers are invalid")
        receipt = cls(
            job_id=value.get("job_id"),
            provider_operation_digest=value.get("provider_operation_digest"),
            dispatch_index=value.get("dispatch_index"),
            previous_provider_dispatch_head_digest=value.get(
                "previous_provider_dispatch_head_digest"
            ),
            provider=value.get("provider"),
            model=value.get("model"),
            invocation_kind=value.get("invocation_kind"),
            dispatch_scope_digest=value.get("dispatch_scope_digest"),
            transport_attempt=value.get("transport_attempt"),
            request_digest=value.get("request_digest"),
            provider_idempotency_key=value.get("provider_idempotency_key"),
        )
        if value.get("provider_idempotency_key_digest") != (
            receipt.provider_idempotency_key_digest
        ):
            raise ArgumentError("private provider dispatch receipt key digest is invalid")
        if value.get("receipt_digest") != receipt.receipt_digest:
            raise ArgumentError("private provider dispatch receipt digest is invalid")
        if canonical_json(value) != canonical_json(receipt.to_private_dict()):
            raise ArgumentError("private provider dispatch receipt is not normalized")
        return receipt


def validate_autonomous_evidence_backed_provider_dispatch_receipt(
    value: Mapping[str, Any] | AutonomousEvidenceBackedProviderDispatchReceipt,
) -> AutonomousEvidenceBackedProviderDispatchReceipt:
    return AutonomousEvidenceBackedProviderDispatchReceipt.from_private_dict(
        value.to_private_dict()
        if isinstance(value, AutonomousEvidenceBackedProviderDispatchReceipt)
        else value
    )


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedCheckpoint:
    """Digest-bound metadata-only restart state for one evidence-backed run."""

    job_id: str
    task_digest: str
    request_digest: str
    run_policy_digest: str
    evidence_plan_digest: str
    execution_plan_digest: str
    evidence_result_digest: str | None
    prompt_projection_digest: str | None
    provider_operation_digest: str | None
    provider_dispatch_count: int
    provider_dispatch_head_digest: str | None
    provider_result_digest: str | None
    provider_status: str | None
    status: str
    generation: int
    previous_checkpoint_digest: str | None

    def __post_init__(self) -> None:
        _identifier("evidence-backed checkpoint job_id", self.job_id)
        for name, value in (
            ("task_digest", self.task_digest),
            ("request_digest", self.request_digest),
            ("run_policy_digest", self.run_policy_digest),
            ("evidence_plan_digest", self.evidence_plan_digest),
            ("execution_plan_digest", self.execution_plan_digest),
            ("evidence_result_digest", self.evidence_result_digest),
            ("prompt_projection_digest", self.prompt_projection_digest),
            ("provider_operation_digest", self.provider_operation_digest),
            ("provider_dispatch_head_digest", self.provider_dispatch_head_digest),
            ("provider_result_digest", self.provider_result_digest),
        ):
            _digest(
                f"evidence-backed checkpoint {name}",
                value,
                allow_none=name
                in {
                    "evidence_result_digest",
                    "prompt_projection_digest",
                    "provider_operation_digest",
                    "provider_dispatch_head_digest",
                    "provider_result_digest",
                },
            )
        provider_status = _optional_text(
            "evidence-backed checkpoint provider_status",
            self.provider_status,
        )
        if (
            provider_status is not None
            and provider_status not in _PROVIDER_CHECKPOINT_STATUSES
        ):
            raise ArgumentError(
                "evidence-backed checkpoint provider_status is invalid"
            )
        generation = _checkpoint_generation(self.generation)
        _digest(
            "evidence-backed checkpoint previous_checkpoint_digest",
            self.previous_checkpoint_digest,
            allow_none=True,
        )
        if (generation == 1) != (self.previous_checkpoint_digest is None):
            raise ArgumentError(
                "evidence-backed checkpoint generation and previous digest are inconsistent"
            )
        if self.status not in AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES:
            raise ArgumentError("evidence-backed checkpoint status is invalid")
        if (
            isinstance(self.provider_dispatch_count, bool)
            or not isinstance(self.provider_dispatch_count, int)
            or not 0
            <= self.provider_dispatch_count
            <= MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES
        ):
            raise ArgumentError(
                "evidence-backed checkpoint provider dispatch count is outside its bound"
            )
        if generation == 1 and self.status in {
            "provider_reconciliation_required",
            "completed",
        }:
            raise ArgumentError(
                "terminal or reconciliation checkpoint requires an attempted predecessor"
            )
        evidence_only = self.status in {
            "evidence_review_required",
            "evidence_incomplete",
            "evidence_failed",
            "evidence_reconciliation_required",
        }
        provider_bound = self.status in {
            "provider_in_flight",
            "provider_reconciliation_required",
            "completed",
        }
        if evidence_only or self.status == "provider_pending":
            if (
                self.provider_dispatch_count != 0
                or self.provider_dispatch_head_digest is not None
            ):
                raise ArgumentError(
                    f"{self.status.replace('_', '-')} checkpoint cannot contain provider dispatch metadata"
                )
        elif (
            self.provider_dispatch_count < 1
            or self.provider_dispatch_head_digest is None
        ):
            raise ArgumentError(
                "provider-bound checkpoint requires a provider dispatch receipt head"
            )
        if (evidence_only or self.status == "provider_pending") and any(
            value is not None
            for value in (
                self.provider_operation_digest,
                self.provider_result_digest,
                self.provider_status,
            )
        ):
            raise ArgumentError(
                f"{self.status.replace('_', '-')} checkpoint cannot contain provider metadata"
            )
        if (provider_bound or self.status == "provider_pending") and (
            self.evidence_result_digest is None
        ):
            raise ArgumentError(
                "provider-ready checkpoint requires an evidence result digest"
            )
        if provider_bound and self.provider_operation_digest is None:
            raise ArgumentError(
                "provider-bound checkpoint requires an operation digest"
            )
        if provider_bound:
            derived_operation_digest = _provider_operation_digest(
                job_id=self.job_id,
                task_digest=self.task_digest,
                request_digest=self.request_digest,
                run_policy_digest=self.run_policy_digest,
                evidence_plan_digest=self.evidence_plan_digest,
                execution_plan_digest=self.execution_plan_digest,
                evidence_result_digest=self.evidence_result_digest,
                prompt_projection_digest=self.prompt_projection_digest,
            )
            if self.provider_operation_digest != derived_operation_digest:
                raise ArgumentError(
                    "evidence-backed checkpoint provider operation digest is invalid"
                )
        if self.status == "provider_in_flight" and (
            self.provider_result_digest is not None or self.provider_status is not None
        ):
            raise ArgumentError(
                f"{self.status.replace('_', '-')} checkpoint cannot contain provider result metadata"
            )
        if self.status == "completed" and (
            self.provider_result_digest is None or self.provider_status != "completed"
        ):
            raise ArgumentError(
                "completed evidence-backed checkpoint requires a completed provider digest"
            )
        if self.status == "provider_reconciliation_required":
            unknown = (
                self.provider_result_digest is None
                and self.provider_status is None
            )
            observed = (
                self.provider_result_digest is not None
                and self.provider_status is not None
            )
            if not (unknown or observed):
                raise ArgumentError(
                    "provider reconciliation checkpoint has inconsistent outcome metadata"
                )

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
            "job_id": self.job_id,
            "task_digest": self.task_digest,
            "request_digest": self.request_digest,
            "run_policy_digest": self.run_policy_digest,
            "evidence_plan_digest": self.evidence_plan_digest,
            "execution_plan_digest": self.execution_plan_digest,
            "evidence_result_digest": self.evidence_result_digest,
            "prompt_projection_digest": self.prompt_projection_digest,
            "provider_operation_digest": self.provider_operation_digest,
            "provider_dispatch_count": self.provider_dispatch_count,
            "provider_dispatch_head_digest": self.provider_dispatch_head_digest,
            "provider_result_digest": self.provider_result_digest,
            "provider_status": self.provider_status,
            "status": self.status,
            "generation": self.generation,
            "previous_checkpoint_digest": self.previous_checkpoint_digest,
        }

    @property
    def checkpoint_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        value = {
            **self._payload(),
            "checkpoint_digest": self.checkpoint_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        _json_bytes(value, "evidence-backed checkpoint")
        return value

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousEvidenceBackedCheckpoint":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence-backed checkpoint must be a mapping")
        expected = {
            "schema", "job_id", "task_digest", "request_digest", "run_policy_digest",
            "evidence_plan_digest", "execution_plan_digest", "evidence_result_digest",
            "prompt_projection_digest", "provider_operation_digest", "provider_result_digest",
            "provider_dispatch_count", "provider_dispatch_head_digest", "provider_status",
            "status", "generation", "previous_checkpoint_digest",
            "checkpoint_digest", "retention", "secret_material",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA:
            raise ArgumentError("evidence-backed checkpoint contains unsupported or missing fields")
        if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("evidence-backed checkpoint retention markers are invalid")
        checkpoint = cls(
            job_id=value.get("job_id"),
            task_digest=value.get("task_digest"),
            request_digest=value.get("request_digest"),
            run_policy_digest=value.get("run_policy_digest"),
            evidence_plan_digest=value.get("evidence_plan_digest"),
            execution_plan_digest=value.get("execution_plan_digest"),
            evidence_result_digest=value.get("evidence_result_digest"),
            prompt_projection_digest=value.get("prompt_projection_digest"),
            provider_operation_digest=value.get("provider_operation_digest"),
            provider_dispatch_count=value.get("provider_dispatch_count"),
            provider_dispatch_head_digest=value.get(
                "provider_dispatch_head_digest"
            ),
            provider_result_digest=value.get("provider_result_digest"),
            provider_status=value.get("provider_status"),
            status=value.get("status"),
            generation=value.get("generation"),
            previous_checkpoint_digest=value.get("previous_checkpoint_digest"),
        )
        supplied = _digest("evidence-backed checkpoint checkpoint_digest", value.get("checkpoint_digest"))
        if supplied != checkpoint.checkpoint_digest:
            raise ArgumentError("evidence-backed checkpoint digest is invalid")
        if canonical_json(value) != canonical_json(checkpoint.to_dict()):
            raise ArgumentError("evidence-backed checkpoint is not normalized")
        return checkpoint


def validate_autonomous_evidence_backed_checkpoint(value: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> AutonomousEvidenceBackedCheckpoint:
    """Validate a checkpoint before journal replay or provider dispatch."""

    return AutonomousEvidenceBackedCheckpoint.from_dict(value.to_dict() if isinstance(value, AutonomousEvidenceBackedCheckpoint) else value)


class AutonomousEvidenceBackedCheckpointStore(Protocol):
    def read(self) -> Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint | None: ...

    def write(self, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> None: ...


class TransactionalAutonomousEvidenceBackedCheckpointStore(
    AutonomousEvidenceBackedCheckpointStore,
    Protocol,
):
    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> bool: ...

    def write_dispatch_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint,
        private_receipt: Mapping[str, Any]
        | AutonomousEvidenceBackedProviderDispatchReceipt,
    ) -> bool: ...


class AutonomousEvidenceBackedCheckpointTextStore(Protocol):
    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousEvidenceBackedCheckpointTextStore(AutonomousEvidenceBackedCheckpointTextStore, Protocol):
    def write_if_unchanged(self, expected_checkpoint_digest: str | None, value: str) -> bool: ...

    def write_dispatch_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint_json: str,
        private_receipt_json: str,
    ) -> bool: ...


class InMemoryAutonomousEvidenceBackedCheckpointStore:
    """Thread-safe reference checkpoint store with compare-and-swap fencing."""

    def __init__(self, initial: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint | None = None) -> None:
        self._store_lock = Lock()
        self._checkpoint = None if initial is None else validate_autonomous_evidence_backed_checkpoint(initial)
        self._provider_dispatch_receipts: dict[
            str, AutonomousEvidenceBackedProviderDispatchReceipt
        ] = {}

    def read(self) -> dict[str, Any] | None:
        with self._store_lock:
            return None if self._checkpoint is None else self._checkpoint.to_dict()

    def write(self, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> None:
        validated = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        with self._store_lock:
            self._checkpoint = validated

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> bool:
        validated = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        with self._store_lock:
            observed = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
            if observed != expected_checkpoint_digest:
                return False
            _assert_ordinary_checkpoint_transition(self._checkpoint, validated)
            self._checkpoint = validated
            return True

    def write_dispatch_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint,
        private_receipt: Mapping[str, Any]
        | AutonomousEvidenceBackedProviderDispatchReceipt,
    ) -> bool:
        validated = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        receipt = validate_autonomous_evidence_backed_provider_dispatch_receipt(
            private_receipt
        )
        with self._store_lock:
            observed = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
            if observed != expected_checkpoint_digest:
                return False
            _assert_provider_dispatch_commit(
                self._checkpoint,
                validated,
                receipt,
            )
            existing = self._provider_dispatch_receipts.get(receipt.receipt_digest)
            if existing is not None and existing != receipt:
                raise BrainRunError(
                    "provider dispatch receipt digest collides with different private data"
                )
            self._provider_dispatch_receipts[receipt.receipt_digest] = receipt
            self._checkpoint = validated
            return True

    def provider_dispatch_receipt(
        self,
        receipt_digest: str,
    ) -> AutonomousEvidenceBackedProviderDispatchReceipt | None:
        """Privileged exact-key lookup for caller-owned reconciliation."""

        _digest("provider dispatch receipt lookup digest", receipt_digest)
        with self._store_lock:
            receipt = self._provider_dispatch_receipts.get(receipt_digest)
            return (
                None
                if receipt is None
                else validate_autonomous_evidence_backed_provider_dispatch_receipt(
                    receipt.to_private_dict()
                )
            )

    def provider_dispatch_receipt_projections(
        self,
        head_digest: str | None = None,
    ) -> tuple[dict[str, Any], ...]:
        """Enumerate ordered public receipt projections without exact transport keys."""

        with self._store_lock:
            checkpoint = self._checkpoint
            selected_head = (
                head_digest
                if head_digest is not None
                else None
                if checkpoint is None
                else checkpoint.provider_dispatch_head_digest
            )
            if selected_head is None:
                return ()
            _digest("provider dispatch receipt head", selected_head)
            rows: list[AutonomousEvidenceBackedProviderDispatchReceipt] = []
            cursor: str | None = selected_head
            while cursor is not None:
                receipt = self._provider_dispatch_receipts.get(cursor)
                if receipt is None:
                    raise BrainRunError(
                        "provider dispatch receipt chain is incomplete"
                    )
                rows.append(receipt)
                cursor = receipt.previous_provider_dispatch_head_digest
                if len(rows) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES:
                    raise BrainRunError(
                        "provider dispatch receipt chain exceeds its bound"
                    )
            rows.reverse()
            if checkpoint is not None and selected_head == (
                checkpoint.provider_dispatch_head_digest
            ) and len(rows) != checkpoint.provider_dispatch_count:
                raise BrainRunError(
                    "provider dispatch receipt chain count does not match checkpoint"
                )
            return tuple(receipt.to_dict() for receipt in rows)

    def provider_dispatch_receipts(
        self,
        head_digest: str | None = None,
    ) -> tuple[dict[str, Any], ...]:
        """Backward-compatible public projection enumeration."""

        return self.provider_dispatch_receipt_projections(head_digest)


class JsonAutonomousEvidenceBackedCheckpointPersistence:
    """Canonical JSON checkpoint persistence for files, browser storage, or service adapters."""

    def __init__(self, store: AutonomousEvidenceBackedCheckpointTextStore, *, max_bytes: int = MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("evidence-backed JSON checkpoint store is malformed")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES:
            raise ArgumentError("evidence-backed JSON checkpoint max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("evidence-backed JSON checkpoint exceeds its byte bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("evidence-backed JSON checkpoint is invalid") from error
        normalized = validate_autonomous_evidence_backed_checkpoint(value).to_dict()
        if encoded != canonical_json(normalized):
            raise ArgumentError("evidence-backed JSON checkpoint is not canonical")
        return normalized

    def write(self, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> None:
        normalized = validate_autonomous_evidence_backed_checkpoint(checkpoint).to_dict()
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("evidence-backed JSON checkpoint exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence(JsonAutonomousEvidenceBackedCheckpointPersistence):
    """Canonical JSON persistence with stale-writer compare-and-swap fencing."""

    def __init__(self, store: TransactionalAutonomousEvidenceBackedCheckpointTextStore, *, max_bytes: int = MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional evidence-backed checkpoint store requires write_if_unchanged")
        if not callable(getattr(store, "write_dispatch_if_unchanged", None)):
            raise ArgumentError(
                "transactional evidence-backed checkpoint store requires write_dispatch_if_unchanged"
            )
        self.store = store

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint) -> bool:
        if expected_checkpoint_digest is not None:
            _digest("evidence-backed expected checkpoint digest", expected_checkpoint_digest)
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        current_value = self.read()
        previous = (
            None
            if current_value is None
            else validate_autonomous_evidence_backed_checkpoint(current_value)
        )
        observed = None if previous is None else previous.checkpoint_digest
        if observed != expected_checkpoint_digest:
            return False
        _assert_ordinary_checkpoint_transition(previous, verified)
        normalized = verified.to_dict()
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("evidence-backed JSON checkpoint exceeds its byte bound")
        result = self.store.write_if_unchanged(expected_checkpoint_digest, encoded)
        if not isinstance(result, bool):
            raise ArgumentError("transactional evidence-backed checkpoint store returned a non-boolean")
        return result

    def write_dispatch_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint,
        private_receipt: Mapping[str, Any]
        | AutonomousEvidenceBackedProviderDispatchReceipt,
    ) -> bool:
        """Atomically append one private attempt receipt and its public checkpoint head."""

        if expected_checkpoint_digest is not None:
            _digest(
                "evidence-backed expected checkpoint digest",
                expected_checkpoint_digest,
            )
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        receipt = validate_autonomous_evidence_backed_provider_dispatch_receipt(
            private_receipt
        )
        current_value = self.read()
        previous = (
            None
            if current_value is None
            else validate_autonomous_evidence_backed_checkpoint(current_value)
        )
        observed = None if previous is None else previous.checkpoint_digest
        if observed != expected_checkpoint_digest:
            return False
        _assert_provider_dispatch_commit(previous, verified, receipt)
        checkpoint_json = canonical_json(verified.to_dict())
        private_receipt_json = canonical_json(receipt.to_private_dict())
        if (
            len(checkpoint_json.encode("utf-8")) > self.max_bytes
            or len(private_receipt_json.encode("utf-8")) > self.max_bytes
        ):
            raise ArgumentError(
                "evidence-backed provider dispatch transaction exceeds its byte bound"
            )
        result = self.store.write_dispatch_if_unchanged(
            expected_checkpoint_digest,
            checkpoint_json,
            private_receipt_json,
        )
        if not isinstance(result, bool):
            raise ArgumentError(
                "transactional evidence-backed provider dispatch store returned a non-boolean"
            )
        return result


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedResumableRun:
    """Transient result plus a metadata-only restart checkpoint."""

    status: str
    job_id: str
    result: AutonomousEvidenceBackedRunResult
    checkpoint: AutonomousEvidenceBackedCheckpoint
    provider_rehydrated: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA,
            "status": self.status,
            "job_id": self.job_id,
            "checkpoint_digest": self.checkpoint.checkpoint_digest,
            "result_status": self.result.status,
            "provider_rehydrated": self.provider_rehydrated,
            "retention": _RESULT_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _checkpoint_status_for_result(result: AutonomousEvidenceBackedRunResult) -> str:
    if result.status == "evidence_review_required":
        return "evidence_review_required"
    if result.evidence is not None and result.evidence.status != "completed":
        return "evidence_incomplete" if result.evidence.status not in {"failed", "reconciliation_required"} else f"evidence_{result.evidence.status}"
    if result.execution_status in _PROVIDER_COMPLETION_STATUSES:
        return "completed"
    if _provider_result_was_observed(result.execution_status):
        return "provider_reconciliation_required"
    return "provider_pending"


def _evidence_projection_digest(evidence: AutonomousEvidenceRuntimeResult) -> str:
    """Bind provider-relevant evidence while excluding replay/process bookkeeping."""

    receipts = []
    for receipt in evidence.receipts:
        receipt_projection = receipt.to_dict()
        for key in ("replay", "duration_ms", "receipt_digest", "assessment_digest"):
            receipt_projection.pop(key, None)
        receipts.append(receipt_projection)
    assessments = []
    for assessment in evidence.assessments:
        assessment_projection = assessment.to_dict()
        for key in ("receipt_digest", "assessment_digest"):
            assessment_projection.pop(key, None)
        assessments.append(assessment_projection)
    return content_digest(
        {
            "schema": _EVIDENCE_PROVIDER_PROJECTION_SCHEMA,
            "status": evidence.status,
            "plan_digest": evidence.plan.plan_digest,
            "receipts": receipts,
            "assessments": assessments,
            "completed_requirement_ids": list(evidence.completed_requirement_ids),
            "pending_evaluation_requirement_ids": list(
                evidence.pending_evaluation_requirement_ids
            ),
            "missing_requirement_ids": list(evidence.missing_requirement_ids),
            "next_stage_ids": list(evidence.next_stage_ids),
            "omitted_request_digests": list(evidence.omitted_request_digests),
            "retention": "metadata_only;raw_values_caller_owned",
            "secret_material": _SECRET_MATERIAL,
        }
    )


def _default_resumable_prompt_context(
    evidence: AutonomousEvidenceRuntimeResult,
) -> dict[str, Any]:
    """Use the semantic evidence digest so a journal replay does not change provider input."""

    return {
        "evidence_backed": {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
            "plan_digest": evidence.plan.plan_digest,
            "result_digest": _evidence_projection_digest(evidence),
            "status": evidence.status,
            "completed_requirement_ids": list(evidence.completed_requirement_ids),
            "pending_evaluation_requirement_ids": list(
                evidence.pending_evaluation_requirement_ids
            ),
            "missing_requirement_ids": list(evidence.missing_requirement_ids),
            "retention": "metadata_only;raw_values_caller_owned",
        }
    }


def _checkpoint_for_result(
    *,
    job_id: str,
    request_digest: str,
    run_policy_digest: str,
    result: AutonomousEvidenceBackedRunResult,
    predecessor: AutonomousEvidenceBackedCheckpoint | None,
    status: str | None = None,
    provider_result_digest_override: str | None = None,
    provider_status_override: str | None = None,
    provider_outcome_unknown: bool = False,
    provider_result_observed: bool = False,
) -> AutonomousEvidenceBackedCheckpoint:
    resolved_status = _checkpoint_status_for_result(result) if status is None else status
    evidence_digest = (
        None if result.evidence is None else _evidence_projection_digest(result.evidence)
    )
    prompt_digest = _prompt_projection_digest(result.prompt_context)
    provider_digest = provider_result_digest_override
    provider_status = provider_status_override
    if resolved_status == "completed":
        provider_digest = result.execution_digest
        provider_status = "completed"
    elif provider_outcome_unknown:
        if resolved_status != "provider_reconciliation_required":
            raise BrainRunError(
                "unknown provider outcome is valid only during reconciliation"
            )
        provider_digest = None
        provider_status = None
    elif resolved_status == "provider_reconciliation_required" and provider_status is None:
        observed_status = _checkpoint_provider_status(result.execution_status)
        if result.execution_digest is not None and (
            provider_result_observed
            or _provider_result_was_observed(observed_status)
        ):
            provider_digest = result.execution_digest
            provider_status = observed_status
        else:
            provider_digest = None
            provider_status = None
    if resolved_status in {
        "provider_pending",
        "provider_in_flight",
        "evidence_review_required",
        "evidence_incomplete",
        "evidence_failed",
        "evidence_reconciliation_required",
    }:
        provider_digest = None
        if resolved_status != "provider_reconciliation_required":
            provider_status = None
    evidence_only = resolved_status.startswith("evidence_")
    if evidence_only:
        prompt_digest = None
    provider_operation_digest = None
    provider_dispatch_count = 0
    provider_dispatch_head_digest = None
    provider_bound = resolved_status in {
        "provider_in_flight",
        "provider_reconciliation_required",
        "completed",
    }
    if provider_bound:
        if evidence_digest is None:
            raise BrainRunError(
                "provider-bound checkpoint requires a completed evidence projection"
            )
        provider_operation_digest = _provider_operation_digest(
            job_id=job_id,
            task_digest=result.task_digest,
            request_digest=request_digest,
            run_policy_digest=run_policy_digest,
            evidence_plan_digest=result.evidence_plan.plan_digest,
            execution_plan_digest=result.execution_plan_digest,
            evidence_result_digest=evidence_digest,
            prompt_projection_digest=prompt_digest,
        )
        if predecessor is not None:
            provider_dispatch_count = predecessor.provider_dispatch_count
            provider_dispatch_head_digest = (
                predecessor.provider_dispatch_head_digest
            )
        if (
            predecessor is not None
            and predecessor.provider_operation_digest is not None
            and predecessor.provider_operation_digest != provider_operation_digest
        ):
            raise BrainRunError(
                "provider operation digest changed across a checkpoint transition"
            )
    return AutonomousEvidenceBackedCheckpoint(
        job_id=job_id,
        task_digest=result.task_digest,
        request_digest=request_digest,
        run_policy_digest=run_policy_digest,
        evidence_plan_digest=result.evidence_plan.plan_digest,
        execution_plan_digest=result.execution_plan_digest,
        evidence_result_digest=evidence_digest,
        prompt_projection_digest=prompt_digest,
        provider_operation_digest=provider_operation_digest,
        provider_dispatch_count=provider_dispatch_count,
        provider_dispatch_head_digest=provider_dispatch_head_digest,
        provider_result_digest=provider_digest,
        provider_status=provider_status,
        status=resolved_status,
        generation=(
            1 if predecessor is None else predecessor.generation + 1
        ),
        previous_checkpoint_digest=(
            None if predecessor is None else predecessor.checkpoint_digest
        ),
    )


def _provider_operation_digest_for_preflight(
    *,
    job_id: str,
    request_digest: str,
    run_policy_digest: str,
    preflight: AutonomousEvidenceBackedPreflight,
) -> str:
    return _provider_operation_digest(
        job_id=job_id,
        task_digest=preflight.task_digest,
        request_digest=request_digest,
        run_policy_digest=run_policy_digest,
        evidence_plan_digest=preflight.evidence_plan.plan_digest,
        execution_plan_digest=preflight.execution_plan_digest,
        evidence_result_digest=_evidence_projection_digest(preflight.evidence),
        prompt_projection_digest=_prompt_projection_digest(
            preflight.prompt_context
        ),
    )


def _checkpoint_for_preflight(
    *,
    job_id: str,
    request_digest: str,
    run_policy_digest: str,
    preflight: AutonomousEvidenceBackedPreflight,
    predecessor: AutonomousEvidenceBackedCheckpoint | None,
    status: str,
    provider_dispatch_count: int,
    provider_dispatch_head_digest: str,
) -> AutonomousEvidenceBackedCheckpoint:
    evidence_digest = _evidence_projection_digest(preflight.evidence)
    prompt_digest = _prompt_projection_digest(preflight.prompt_context)
    operation_digest = _provider_operation_digest_for_preflight(
        job_id=job_id,
        request_digest=request_digest,
        run_policy_digest=run_policy_digest,
        preflight=preflight,
    )
    if (
        predecessor is not None
        and predecessor.provider_operation_digest is not None
        and predecessor.provider_operation_digest != operation_digest
    ):
        raise BrainRunError(
            "provider operation digest changed across a checkpoint transition"
        )
    return AutonomousEvidenceBackedCheckpoint(
        job_id=job_id,
        task_digest=preflight.task_digest,
        request_digest=request_digest,
        run_policy_digest=run_policy_digest,
        evidence_plan_digest=preflight.evidence_plan.plan_digest,
        execution_plan_digest=preflight.execution_plan_digest,
        evidence_result_digest=evidence_digest,
        prompt_projection_digest=prompt_digest,
        provider_operation_digest=operation_digest,
        provider_dispatch_count=provider_dispatch_count,
        provider_dispatch_head_digest=provider_dispatch_head_digest,
        provider_result_digest=None,
        provider_status=None,
        status=status,
        generation=(
            1 if predecessor is None else predecessor.generation + 1
        ),
        previous_checkpoint_digest=(
            None if predecessor is None else predecessor.checkpoint_digest
        ),
    )


def _prompt_projection_digest(prompt_context: Mapping[str, Any]) -> str | None:
    return None if not prompt_context else content_digest(prompt_context)


def _assert_checkpoint_evidence_projection(
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    *,
    evidence: Any | None,
    prompt_context: Mapping[str, Any],
) -> None:
    """Refuse provider reuse or dispatch when reconstructed preflight inputs drifted."""

    evidence_result_digest = (
        None if evidence is None else _evidence_projection_digest(evidence)
    )
    prompt_projection_digest = _prompt_projection_digest(prompt_context)
    if checkpoint.evidence_result_digest != evidence_result_digest:
        raise BrainRunError(
            "resumed evidence result does not match its provider-bound checkpoint digest"
        )
    if checkpoint.prompt_projection_digest != prompt_projection_digest:
        raise BrainRunError(
            "resumed prompt projection does not match its provider-bound checkpoint digest"
        )


def _assert_checkpoint_callback_value_unchanged(
    value: AutonomousEvidenceBackedCheckpoint,
    expected: AutonomousEvidenceBackedCheckpoint,
) -> None:
    try:
        observed = validate_autonomous_evidence_backed_checkpoint(value)
    except Exception as error:
        raise BrainRunError(
            "evidence-backed checkpoint changed during persistence callback"
        ) from error
    if (
        observed.checkpoint_digest != expected.checkpoint_digest
        or observed.to_dict() != expected.to_dict()
    ):
        raise BrainRunError(
            "evidence-backed checkpoint changed during persistence callback"
        )


def _persist_checkpoint(
    sink: Callable[[AutonomousEvidenceBackedCheckpoint], Any],
    checkpoint: AutonomousEvidenceBackedCheckpoint,
) -> AutonomousEvidenceBackedCheckpoint:
    if not callable(sink):
        raise ArgumentError("evidence-backed checkpoint sink must be callable")
    snapshot = validate_autonomous_evidence_backed_checkpoint(checkpoint)
    callback_value = validate_autonomous_evidence_backed_checkpoint(snapshot)
    sink(callback_value)
    _assert_checkpoint_callback_value_unchanged(checkpoint, snapshot)
    _assert_checkpoint_callback_value_unchanged(callback_value, snapshot)
    return snapshot


def _assert_checkpoint_transition(
    previous: AutonomousEvidenceBackedCheckpoint | None,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
) -> None:
    expected_generation = 1 if previous is None else previous.generation + 1
    expected_previous = None if previous is None else previous.checkpoint_digest
    if (
        checkpoint.generation != expected_generation
        or checkpoint.previous_checkpoint_digest != expected_previous
    ):
        raise BrainRunError(
            "evidence-backed checkpoint transition does not extend the exact current head"
        )
    if previous is None:
        if (
            checkpoint.status == "provider_in_flight"
            and checkpoint.provider_dispatch_count != 1
        ):
            raise BrainRunError(
                "initial provider dispatch checkpoint must bind exactly one receipt"
            )
        return
    for key in (
        "job_id",
        "task_digest",
        "request_digest",
        "run_policy_digest",
        "evidence_plan_digest",
        "execution_plan_digest",
    ):
        if getattr(previous, key) != getattr(checkpoint, key):
            raise BrainRunError(
                "evidence-backed checkpoint transition changed its bound operation"
            )
    evidence_successors = {
        "evidence_review_required",
        "evidence_incomplete",
        "evidence_failed",
        "evidence_reconciliation_required",
        "provider_pending",
    }
    allowed = {
        "evidence_review_required": evidence_successors,
        "evidence_incomplete": evidence_successors,
        "evidence_failed": evidence_successors,
        "evidence_reconciliation_required": evidence_successors,
        "provider_pending": {"provider_in_flight"},
        "provider_in_flight": {
            "provider_in_flight",
            "provider_reconciliation_required",
            "completed",
        },
        "provider_reconciliation_required": {
            "provider_reconciliation_required",
            "completed",
        },
        "completed": set(),
    }
    if checkpoint.status not in allowed[previous.status]:
        raise BrainRunError(
            f"invalid evidence-backed checkpoint transition {previous.status} -> {checkpoint.status}"
        )
    if checkpoint.status == "provider_in_flight":
        if (
            checkpoint.provider_dispatch_count
            != previous.provider_dispatch_count + 1
            or checkpoint.provider_dispatch_head_digest
            == previous.provider_dispatch_head_digest
        ):
            raise BrainRunError(
                "provider dispatch checkpoint did not append exactly one receipt"
            )
    elif (
        checkpoint.provider_dispatch_count != previous.provider_dispatch_count
        or checkpoint.provider_dispatch_head_digest
        != previous.provider_dispatch_head_digest
    ):
        raise BrainRunError(
            "non-dispatch checkpoint transition changed its provider receipt head"
        )
    if previous.status in {
        "provider_pending",
        "provider_in_flight",
        "provider_reconciliation_required",
        "completed",
    } and (
        checkpoint.evidence_result_digest != previous.evidence_result_digest
        or checkpoint.prompt_projection_digest != previous.prompt_projection_digest
    ):
        raise BrainRunError(
            "provider-ready checkpoint transition changed its settled evidence or prompt"
        )
    if previous.provider_operation_digest is not None and (
        checkpoint.provider_operation_digest != previous.provider_operation_digest
    ):
        raise BrainRunError(
            "provider-bound checkpoint transition changed its operation digest"
        )
    if (
        previous.status == "provider_reconciliation_required"
        and previous.provider_result_digest is not None
        and (
            checkpoint.provider_result_digest != previous.provider_result_digest
            or checkpoint.provider_status != previous.provider_status
        )
    ):
        raise BrainRunError(
            "observed provider reconciliation transition changed its outcome"
        )


def _assert_ordinary_checkpoint_transition(
    previous: AutonomousEvidenceBackedCheckpoint | None,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
) -> None:
    """Forbid receipt-chain advances outside the atomic dispatch transaction."""

    _assert_checkpoint_transition(previous, checkpoint)
    previous_count = 0 if previous is None else previous.provider_dispatch_count
    previous_head = (
        None if previous is None else previous.provider_dispatch_head_digest
    )
    if (
        checkpoint.provider_dispatch_count != previous_count
        or checkpoint.provider_dispatch_head_digest != previous_head
    ):
        raise BrainRunError(
            "provider dispatch checkpoint requires an atomic private receipt transaction"
        )


def _assert_provider_dispatch_commit(
    previous: AutonomousEvidenceBackedCheckpoint | None,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
) -> None:
    _assert_checkpoint_transition(previous, checkpoint)
    previous_count = 0 if previous is None else previous.provider_dispatch_count
    previous_head = (
        None if previous is None else previous.provider_dispatch_head_digest
    )
    if checkpoint.status != "provider_in_flight":
        raise BrainRunError(
            "provider dispatch commit must produce an in-flight checkpoint"
        )
    if (
        receipt.job_id != checkpoint.job_id
        or receipt.provider_operation_digest
        != checkpoint.provider_operation_digest
        or receipt.dispatch_index != previous_count + 1
        or receipt.previous_provider_dispatch_head_digest != previous_head
        or checkpoint.provider_dispatch_count != receipt.dispatch_index
        or checkpoint.provider_dispatch_head_digest != receipt.receipt_digest
    ):
        raise BrainRunError(
            "provider dispatch receipt does not match its checkpoint transition"
        )


def _compare_and_store_checkpoint(
    compare_and_store: Callable[
        [str | None, AutonomousEvidenceBackedCheckpoint], bool
    ]
    | None,
    previous: AutonomousEvidenceBackedCheckpoint | None,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
) -> AutonomousEvidenceBackedCheckpoint:
    if not callable(compare_and_store):
        raise ArgumentError(
            "provider dispatch requires checkpoint_compare_and_store"
        )
    previous_snapshot = (
        None
        if previous is None
        else validate_autonomous_evidence_backed_checkpoint(previous)
    )
    snapshot = validate_autonomous_evidence_backed_checkpoint(checkpoint)
    callback_value = validate_autonomous_evidence_backed_checkpoint(snapshot)
    _assert_ordinary_checkpoint_transition(previous_snapshot, snapshot)
    expected = (
        None
        if previous_snapshot is None
        else previous_snapshot.checkpoint_digest
    )
    stored = compare_and_store(expected, callback_value)
    if previous is not None and previous_snapshot is not None:
        _assert_checkpoint_callback_value_unchanged(previous, previous_snapshot)
    _assert_checkpoint_callback_value_unchanged(checkpoint, snapshot)
    _assert_checkpoint_callback_value_unchanged(callback_value, snapshot)
    if not isinstance(stored, bool):
        raise ArgumentError(
            "checkpoint_compare_and_store must return a boolean"
        )
    if not stored:
        raise BrainRunError(
            "evidence-backed checkpoint compare-and-swap conflict"
        )
    return snapshot


def _compare_and_store_provider_dispatch(
    compare_and_store: Callable[
        [
            str | None,
            AutonomousEvidenceBackedCheckpoint,
            AutonomousEvidenceBackedProviderDispatchReceipt,
        ],
        bool,
    ]
    | None,
    previous: AutonomousEvidenceBackedCheckpoint | None,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
) -> AutonomousEvidenceBackedCheckpoint:
    if not callable(compare_and_store):
        raise ArgumentError(
            "provider dispatch requires checkpoint_dispatch_compare_and_store"
        )
    previous_snapshot = (
        None
        if previous is None
        else validate_autonomous_evidence_backed_checkpoint(previous)
    )
    checkpoint_snapshot = validate_autonomous_evidence_backed_checkpoint(
        checkpoint
    )
    receipt_snapshot = (
        validate_autonomous_evidence_backed_provider_dispatch_receipt(receipt)
    )
    callback_checkpoint = validate_autonomous_evidence_backed_checkpoint(
        checkpoint_snapshot
    )
    callback_receipt = (
        validate_autonomous_evidence_backed_provider_dispatch_receipt(
            receipt_snapshot
        )
    )
    _assert_provider_dispatch_commit(
        previous_snapshot,
        checkpoint_snapshot,
        receipt_snapshot,
    )
    expected = (
        None
        if previous_snapshot is None
        else previous_snapshot.checkpoint_digest
    )
    stored = compare_and_store(
        expected,
        callback_checkpoint,
        callback_receipt,
    )
    if previous is not None and previous_snapshot is not None:
        _assert_checkpoint_callback_value_unchanged(previous, previous_snapshot)
    _assert_checkpoint_callback_value_unchanged(
        checkpoint,
        checkpoint_snapshot,
    )
    _assert_checkpoint_callback_value_unchanged(
        callback_checkpoint,
        checkpoint_snapshot,
    )
    try:
        observed_receipt = (
            validate_autonomous_evidence_backed_provider_dispatch_receipt(
                callback_receipt
            )
        )
    except Exception as error:
        raise BrainRunError(
            "provider dispatch receipt changed during persistence callback"
        ) from error
    if observed_receipt.to_private_dict() != receipt_snapshot.to_private_dict():
        raise BrainRunError(
            "provider dispatch receipt changed during persistence callback"
        )
    if stored is not True:
        raise BrainRunError(
            "provider dispatch atomic compare-and-swap conflict or lost acknowledgement; reload required"
        )
    # The caller-owned transaction callback received these objects and can retain or mutate them
    # despite their frozen dataclass API. Revalidate the exact public head/private receipt
    # relationship after acknowledgement, before any transport can consume the bound key.
    _assert_provider_dispatch_commit(
        previous_snapshot,
        checkpoint_snapshot,
        receipt_snapshot,
    )
    return checkpoint_snapshot


def _assert_checkpoint_request_binding(
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    *,
    job_id: str,
    task_digest: str,
    request_digest: str,
    run_policy_digest: str,
) -> None:
    if (
        checkpoint.job_id != job_id
        or checkpoint.task_digest != task_digest
        or checkpoint.request_digest != request_digest
        or checkpoint.run_policy_digest != run_policy_digest
    ):
        raise ArgumentError(
            "evidence-backed checkpoint does not match the current task, requests, policy, or job"
        )


def _assert_checkpoint_plan_binding(
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    *,
    evidence_plan_digest: str,
    execution_plan_digest: str,
) -> None:
    if (
        checkpoint.evidence_plan_digest != evidence_plan_digest
        or checkpoint.execution_plan_digest != execution_plan_digest
    ):
        raise ArgumentError(
            "evidence-backed checkpoint does not match the current evidence or execution plan"
        )


def _resumable_result(
    *,
    status: str,
    job_id: str,
    result: AutonomousEvidenceBackedRunResult,
    checkpoint: AutonomousEvidenceBackedCheckpoint,
    provider_rehydrated: bool,
) -> AutonomousEvidenceBackedResumableRun:
    return AutonomousEvidenceBackedResumableRun(status, job_id, result, checkpoint, provider_rehydrated)


def run_autonomous_evidence_backed_resumable(
    agent: Any,
    *,
    task: str,
    job_id: str,
    requests: Sequence[Mapping[str, Any]],
    acquirer: Any,
    credentials: Any,
    checkpoint_sink: Callable[[AutonomousEvidenceBackedCheckpoint], Any],
    checkpoint_compare_and_store: Callable[
        [str | None, AutonomousEvidenceBackedCheckpoint], bool
    ]
    | None = None,
    checkpoint_dispatch_compare_and_store: Callable[
        [
            str | None,
            AutonomousEvidenceBackedCheckpoint,
            AutonomousEvidenceBackedProviderDispatchReceipt,
        ],
        bool,
    ]
    | None = None,
    checkpoint: Mapping[str, Any] | AutonomousEvidenceBackedCheckpoint | None = None,
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
    prompt_builder: Callable[[Any], Mapping[str, Any]] | None = None,
    run_mode: str = "auto",
    run_options: Mapping[str, Any] | None = None,
    resumable_policy_identity: Mapping[str, Any] | None = None,
    resume_provider: bool = False,
    rehydrate_provider_run: Callable[[AutonomousEvidenceBackedCheckpoint, AutonomousEvidenceBackedRunResult], Any | None] | None = None,
) -> AutonomousEvidenceBackedResumableRun:
    """Execute or resume without silently replaying provider work or stale inputs.

    Plain Python functions receive a deterministic implementation fingerprint. Stateful or
    otherwise opaque adapters must expose stable role-specific ``*_id`` and ``*_version``
    attributes, or the caller must supply their ``id``, ``version``, and optional
    ``config_digest`` under ``resumable_policy_identity``. A value-rehydrator identity may be
    supplied before that callback exists, then repeated when the callback becomes available
    after restart. Unknown callable identity fails closed before evidence or provider work.
    ``resumable_policy_identity.provider_policy`` is always required with a non-null
    ``config_digest``; it is the caller's trust root for provider-shaping agent/runtime state and
    credential-account policy that cannot be projected safely into a checkpoint.
    """

    if not isinstance(resume_provider, bool):
        raise ArgumentError("evidence-backed resume_provider must be a boolean")
    if not isinstance(approve_provider_call, bool):
        raise ArgumentError("evidence-backed approve_provider_call must be a boolean")
    if not callable(checkpoint_sink):
        raise ArgumentError("evidence-backed checkpoint sink must be callable")
    if checkpoint_compare_and_store is not None and not callable(
        checkpoint_compare_and_store
    ):
        raise ArgumentError(
            "evidence-backed checkpoint_compare_and_store must be callable or None"
        )
    if checkpoint_dispatch_compare_and_store is not None and not callable(
        checkpoint_dispatch_compare_and_store
    ):
        raise ArgumentError(
            "evidence-backed checkpoint_dispatch_compare_and_store must be callable or None"
        )
    # Decode the restart contract before touching the agent or any provider-shaping option.
    # Incompatible legacy checkpoints therefore fail without invoking caller-owned objects.
    restored = (
        None
        if checkpoint is None
        else validate_autonomous_evidence_backed_checkpoint(checkpoint)
    )
    if run_options is not None and type(run_options) is not dict:
        raise ArgumentError(
            "resumable evidence-backed provider fencing requires run_options to be a plain dict or None"
        )
    if run_options is not None:
        run_options = _provider_input_snapshot(
            "evidence-backed run_options",
            run_options,
            trust_opaque=True,
        )
    if isinstance(run_options, Mapping) and {
        "idempotency_key",
        "provider_idempotency_key",
        "execution_controller",
    }.intersection(run_options):
        raise ArgumentError(
            "resumable evidence-backed execution owns provider dispatch identity and execution control"
        )
    if isinstance(run_options, Mapping) and (
        (
            run_options.get("execution_mode") is not None
            and run_options.get("execution_mode") != "provider"
        )
        or (
            run_options.get("child_execution_mode") is not None
            and run_options.get("child_execution_mode") != "provider"
        )
        or (
            run_options.get("synthesis_execution_mode") is not None
            and run_options.get("synthesis_execution_mode") != "provider"
        )
        or any(
            key in run_options
            for key in ("tool_loop_options", "mission_policy", "mission_options")
        )
    ):
        raise ArgumentError(
            "resumable evidence-backed provider fencing supports only direct provider execution"
        )
    if isinstance(run_options, Mapping) and (
        (
            run_options.get("semantic_routing") is not None
            and run_options.get("semantic_routing") is not False
        )
        or (
            run_options.get("planning_mode") is not None
            and run_options.get("planning_mode") != "deterministic"
        )
        or (
            run_options.get("learning_mode") is not None
            and run_options.get("learning_mode") != "off"
        )
        or any(
            run_options.get(key) is not None
            and run_options.get(key) is not False
            for key in (
                "learn",
                "workflow_execution",
                "workflow_learning",
                "workflow_trajectory_learning",
                "cross_domain_learning",
                "cross_domain_trajectory_learning",
                "cross_domain_replan_learning",
                "resume_decision_cycle",
            )
        )
        or any(
            run_options.get(key) is not None
            for key in ("decision_cycle_id", "decision_cycle_store")
        )
    ):
        raise ArgumentError(
            "resumable evidence-backed provider fencing rejects auxiliary provider, workflow, and learning modes"
        )
    if journal is None:
        raise ArgumentError("resumable evidence-backed execution requires a caller-owned evidence journal")
    normalized_job_id = _identifier("evidence-backed resumable job_id", job_id)
    if rehydrate_provider_run is not None and not callable(rehydrate_provider_run):
        raise ArgumentError(
            "evidence-backed rehydrate_provider_run must be callable or None"
        )
    if (
        not isinstance(model_candidates, Sequence)
        or isinstance(model_candidates, (str, bytes, bytearray))
        or len(model_candidates) != 1
    ):
        raise ArgumentError(
            "resumable evidence-backed provider fencing requires exactly one explicit model candidate"
        )
    if (
        restored is not None
        and (
            restored.status == "provider_in_flight"
            or (
                restored.status == "provider_reconciliation_required"
                and rehydrate_provider_run is not None
            )
        )
        and checkpoint_compare_and_store is None
    ):
        raise ArgumentError(
            "provider checkpoint transition requires checkpoint_compare_and_store"
        )
    provider_dispatch_authority_requested = bool(
        approve_provider_call
        and (
            restored is None
            or (
                restored.status == "provider_pending"
                and resume_provider
            )
        )
    )
    if provider_dispatch_authority_requested and checkpoint_compare_and_store is None:
        raise ArgumentError(
            "provider dispatch requires checkpoint_compare_and_store"
        )
    if (
        provider_dispatch_authority_requested
        and checkpoint_dispatch_compare_and_store is None
    ):
        raise ArgumentError(
            "provider dispatch requires checkpoint_dispatch_compare_and_store"
        )
    model_candidates = tuple(
        _provider_input_snapshot(
            "evidence-backed model_candidates",
            tuple(model_candidates),
        )
    )
    _model_policy_value(model_candidates)
    source_requests = tuple(
        _provider_input_snapshot(
            "evidence-backed requests",
            _bounded_requests(requests),
        )
    )
    from .autonomy import AUTONOMOUS_DOMAINS

    selected_domains = _bounded_domains(domains, AUTONOMOUS_DOMAINS)
    if not isinstance(available_evidence, Sequence) or isinstance(
        available_evidence, (str, bytes, bytearray)
    ):
        raise ArgumentError("evidence-backed available_evidence must be a sequence")
    available_evidence = tuple(
        _provider_input_snapshot(
            "evidence-backed available_evidence",
            tuple(available_evidence),
        )
    )
    if completed_stages is not None and not isinstance(completed_stages, Mapping):
        raise ArgumentError("evidence-backed completed_stages must be a mapping or None")
    completed_stages = (
        None
        if completed_stages is None
        else _provider_input_snapshot(
            "evidence-backed completed_stages",
            dict(completed_stages),
        )
    )
    if not isinstance(parent_evidence_digests, Sequence) or isinstance(
        parent_evidence_digests, (str, bytes, bytearray)
    ):
        raise ArgumentError(
            "evidence-backed parent_evidence_digests must be a sequence"
        )
    parent_evidence_digests = tuple(
        _provider_input_snapshot(
            "evidence-backed parent_evidence_digests",
            tuple(parent_evidence_digests),
        )
    )
    component_identity = _resumable_policy_identity(
        acquirer=acquirer,
        projector=projector,
        evaluator=evaluator,
        value_rehydrator=rehydrate_value,
        prompt_builder=prompt_builder,
        explicit=resumable_policy_identity,
    )
    normalized_task = _bounded_task(task)
    task_digest = content_digest({"task": normalized_task})
    request_digest_value = _request_digest(source_requests)
    run_policy_digest_value = _run_policy_digest(
        domains=selected_domains,
        model_candidates=model_candidates,
        run_mode=run_mode,
        run_options=run_options,
        approve_source_dispatch=approve_source_dispatch,
        allow_incomplete_evidence=allow_incomplete_evidence,
        prompt_builder=prompt_builder,
        component_identity=component_identity,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
        parent_evidence_digests=parent_evidence_digests,
        stop_on_failure=stop_on_failure,
        reevaluate_pending=reevaluate_pending,
    )
    if restored is not None:
        _assert_checkpoint_request_binding(
            restored,
            job_id=normalized_job_id,
            task_digest=task_digest,
            request_digest=request_digest_value,
            run_policy_digest=run_policy_digest_value,
        )
    # Pure request/policy/checkpoint validation deliberately precedes the exact-core check so a
    # malformed legacy checkpoint or unsupported mode cannot cause any agent lookup.  From this
    # point onward, capture the concrete provider graph before source or prompt callbacks run.
    initial_provider_transport_graph = _capture_provider_transport_graph(
        agent,
        run_options=run_options,
    )
    credentials = _snapshot_provider_credentials(
        credentials,
        initial_provider_transport_graph.credentials,
    )
    plan = _AUTONOMOUS_AGENT_METHODS["evidence_plan"](
        agent,
        selected_domains,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
    )
    execution_plan_digest_value = _execution_plan_digest(
        task_digest,
        plan.plan_digest,
        selected_domains,
        run_mode,
    )
    if restored is not None:
        _assert_checkpoint_plan_binding(
            restored,
            evidence_plan_digest=plan.plan_digest,
            execution_plan_digest=execution_plan_digest_value,
        )

    common: dict[str, Any] = {
        "task": normalized_task,
        "requests": source_requests,
        "acquirer": acquirer,
        "credentials": credentials,
        "domains": selected_domains,
        "model_candidates": model_candidates,
        "projector": projector,
        "evaluator": evaluator,
        "rehydrate_value": rehydrate_value,
        "parent_evidence_digests": parent_evidence_digests,
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
        "available_evidence": available_evidence,
        "completed_stages": completed_stages,
        "journal": journal,
        "approve_source_dispatch": approve_source_dispatch,
        "allow_incomplete_evidence": allow_incomplete_evidence,
        "prompt_builder": (
            _default_resumable_prompt_context
            if prompt_builder is None
            else prompt_builder
        ),
        "run_mode": run_mode,
        "run_options": run_options,
    }

    def execute_without_provider(
        *,
        provider_probe_only: bool = False,
    ) -> AutonomousEvidenceBackedRunResult:
        result = _FROZEN_EVIDENCE_RUNNER(
            agent,
            **common,
            approve_provider_call=False,
            provider_probe_only=provider_probe_only,
        )
        # Source acquisition, value rehydration, and prompt construction are caller-extensible
        # even on a probe.  Do not persist their result after they changed the later transport.
        _assert_provider_transport_graph(
            agent,
            initial_provider_transport_graph,
            run_options=run_options,
        )
        return result

    head = restored

    def store_checkpoint(
        checkpoint_value: AutonomousEvidenceBackedCheckpoint,
        *,
        require_compare_and_store: bool,
    ) -> AutonomousEvidenceBackedCheckpoint:
        nonlocal head
        _assert_ordinary_checkpoint_transition(head, checkpoint_value)
        if checkpoint_compare_and_store is not None:
            committed = _compare_and_store_checkpoint(
                checkpoint_compare_and_store,
                head,
                checkpoint_value,
            )
        elif require_compare_and_store:
            raise ArgumentError(
                "provider dispatch requires checkpoint_compare_and_store"
            )
        else:
            committed = _persist_checkpoint(checkpoint_sink, checkpoint_value)
        head = committed
        return committed

    def persist_result(
        result: AutonomousEvidenceBackedRunResult,
        status: str | None = None,
        *,
        require_compare_and_store: bool = False,
        provider_result_digest_override: str | None = None,
        provider_status_override: str | None = None,
        provider_outcome_unknown: bool = False,
        provider_result_observed: bool = False,
    ) -> AutonomousEvidenceBackedResumableRun:
        next_checkpoint = _checkpoint_for_result(
            job_id=normalized_job_id,
            request_digest=request_digest_value,
            run_policy_digest=run_policy_digest_value,
            result=result,
            predecessor=head,
            status=status,
            provider_result_digest_override=provider_result_digest_override,
            provider_status_override=provider_status_override,
            provider_outcome_unknown=provider_outcome_unknown,
            provider_result_observed=provider_result_observed,
        )
        committed_checkpoint = store_checkpoint(
            next_checkpoint,
            require_compare_and_store=require_compare_and_store,
        )
        return _resumable_result(
            status=committed_checkpoint.status,
            job_id=normalized_job_id,
            result=result,
            checkpoint=committed_checkpoint,
            provider_rehydrated=False,
        )

    def probe_provider_bound_checkpoint() -> AutonomousEvidenceBackedRunResult:
        if restored is None:
            raise BrainRunError("provider checkpoint probe requires restored state")
        probe = execute_without_provider(provider_probe_only=True)
        _assert_checkpoint_evidence_projection(
            restored,
            evidence=probe.evidence,
            prompt_context=probe.prompt_context,
        )
        if probe.evidence is None:
            raise BrainRunError(
                "restored provider checkpoint no longer reconstructs its settled evidence"
            )
        return probe

    def rehydrate_provider_result(
        probe: AutonomousEvidenceBackedRunResult,
    ) -> AutonomousEvidenceBackedResumableRun | None:
        if restored is None or rehydrate_provider_run is None:
            return None
        if type(probe) is not AutonomousEvidenceBackedRunResult:
            raise BrainRunError("provider rehydration probe is malformed")

        def probe_integrity(value: AutonomousEvidenceBackedRunResult) -> dict[str, Any]:
            if type(value) is not AutonomousEvidenceBackedRunResult:
                raise BrainRunError("provider rehydration probe is malformed")
            evidence = object.__getattribute__(value, "evidence")
            prompt_context = object.__getattribute__(value, "prompt_context")
            execution = object.__getattribute__(value, "execution")
            return {
                "status": object.__getattribute__(value, "status"),
                "task_digest": object.__getattribute__(value, "task_digest"),
                "execution_plan_digest": object.__getattribute__(
                    value,
                    "execution_plan_digest",
                ),
                "evidence_plan_digest": object.__getattribute__(
                    object.__getattribute__(value, "evidence_plan"),
                    "plan_digest",
                ),
                "evidence_result_digest": (
                    None
                    if evidence is None
                    else _evidence_projection_digest(evidence)
                ),
                "prompt_projection_digest": _prompt_projection_digest(
                    prompt_context
                ),
                "execution_digest": (
                    None
                    if execution is None
                    else _FROZEN_EXECUTION_METADATA(
                        agent,
                        _FROZEN_PROVIDER_EXECUTION_SNAPSHOT(execution),
                    )[2]
                ),
                "route_digest": object.__getattribute__(value, "route_digest"),
                "execution_status": object.__getattribute__(
                    value,
                    "execution_status",
                ),
                "declared_execution_digest": object.__getattribute__(
                    value,
                    "execution_digest",
                ),
                "result_digest": object.__getattribute__(value, "result_digest"),
            }

        try:
            probe_snapshot = copy.deepcopy(probe)
            callback_probe = copy.deepcopy(probe_snapshot)
        except Exception as error:
            raise BrainRunError(
                "provider rehydration probe could not be detached"
            ) from error
        expected_probe_integrity = probe_integrity(probe_snapshot)
        if probe_integrity(probe) != expected_probe_integrity:
            raise BrainRunError(
                "provider rehydration probe changed while being snapshotted"
            )
        checkpoint_snapshot = validate_autonomous_evidence_backed_checkpoint(
            restored
        )
        checkpoint_snapshot_digest = checkpoint_snapshot.checkpoint_digest
        callback_checkpoint = validate_autonomous_evidence_backed_checkpoint(
            checkpoint_snapshot
        )
        recovered = rehydrate_provider_run(callback_checkpoint, callback_probe)
        # The rehydrator is caller code. Re-establish every execution/result helper binding
        # before using it to validate or detach the returned provider-owned graph.
        _assert_provider_transport_graph(
            agent,
            initial_provider_transport_graph,
            run_options=run_options,
        )
        try:
            checkpoint_after_callback = (
                validate_autonomous_evidence_backed_checkpoint(restored)
            )
        except Exception as error:
            raise BrainRunError(
                "provider checkpoint changed during result rehydration"
            ) from error
        if (
            checkpoint_after_callback.checkpoint_digest
            != checkpoint_snapshot_digest
            or checkpoint_after_callback.to_dict() != checkpoint_snapshot.to_dict()
        ):
            raise BrainRunError(
                "provider checkpoint changed during result rehydration"
            )
        if (
            probe_integrity(probe) != expected_probe_integrity
            or probe_integrity(probe_snapshot) != expected_probe_integrity
        ):
            raise BrainRunError(
                "provider rehydration probe changed during result rehydration"
            )
        if recovered is None:
            return _resumable_result(
                status=checkpoint_snapshot.status,
                job_id=normalized_job_id,
                result=probe_snapshot,
                checkpoint=checkpoint_snapshot,
                provider_rehydrated=False,
            )
        recovered = _FROZEN_PROVIDER_EXECUTION_SNAPSHOT(recovered)
        _assert_provider_transport_graph(
            agent,
            initial_provider_transport_graph,
            run_options=run_options,
        )
        recovered_status, _route, recovered_digest = _FROZEN_EXECUTION_METADATA(
            agent,
            recovered,
        )
        normalized_recovered_status = _checkpoint_provider_status(recovered_status)
        if (
            checkpoint_snapshot.provider_result_digest is not None
            and recovered_digest != checkpoint_snapshot.provider_result_digest
        ):
            raise BrainRunError(
                "rehydrated provider result does not match its checkpoint digest"
            )
        if (
            checkpoint_snapshot.provider_status is not None
            and normalized_recovered_status != checkpoint_snapshot.provider_status
        ):
            raise BrainRunError(
                "rehydrated provider result does not match its checkpoint status"
            )
        final = _FROZEN_EVIDENCE_RUNNER(
            agent,
            **common,
            approve_provider_call=True,
            provider_run_override=recovered,
        )
        # The replay reruns caller-owned evidence/prompt adapters even though it does not
        # contact the provider. Keep their second invocation inside the same transport-policy
        # snapshot before accepting the rehydrated outcome into durable state.
        _assert_provider_transport_graph(
            agent,
            initial_provider_transport_graph,
            run_options=run_options,
        )
        _assert_checkpoint_evidence_projection(
            checkpoint_snapshot,
            evidence=final.evidence,
            prompt_context=final.prompt_context,
        )
        final_status_metadata = _checkpoint_provider_status(
            final.execution_status
        )
        if (
            final.execution_digest != recovered_digest
            or final_status_metadata != normalized_recovered_status
        ):
            raise BrainRunError(
                "rehydrated provider result changed during evidence replay"
            )
        if checkpoint_snapshot.status == "completed":
            if not _provider_execution_completed(final):
                raise BrainRunError(
                    "rehydrated completed provider result is not terminally completed"
                )
            return _resumable_result(
                status="completed",
                job_id=normalized_job_id,
                result=final,
                checkpoint=checkpoint_snapshot,
                provider_rehydrated=True,
            )
        final_status = (
            "completed"
            if final.evidence is not None
            and final.evidence.status == "completed"
            and _provider_execution_completed(final)
            else "provider_reconciliation_required"
        )
        result = persist_result(
            final,
            final_status,
            require_compare_and_store=True,
            provider_result_observed=True,
        )
        return _resumable_result(
            status=result.status,
            job_id=normalized_job_id,
            result=final,
            checkpoint=result.checkpoint,
            provider_rehydrated=True,
        )

    # A restored provider boundary is never converted back into dispatch authority. Approval and
    # resume booleans are deliberately ignored here; only caller-owned result rehydration can
    # settle these states.
    if restored is not None and restored.status == "provider_in_flight":
        probe = probe_provider_bound_checkpoint()
        return persist_result(
            probe,
            "provider_reconciliation_required",
            require_compare_and_store=True,
            provider_outcome_unknown=True,
        )

    if restored is not None and restored.status in {
        "completed",
        "provider_reconciliation_required",
    }:
        probe = probe_provider_bound_checkpoint()
        rehydrated = rehydrate_provider_result(probe)
        if rehydrated is not None:
            return rehydrated
        if restored.status == "completed":
            return _resumable_result(
                status="completed",
                job_id=normalized_job_id,
                result=probe,
                checkpoint=restored,
                provider_rehydrated=False,
            )
        if restored.status == "provider_reconciliation_required":
            return _resumable_result(
                status="provider_reconciliation_required",
                job_id=normalized_job_id,
                result=probe,
                checkpoint=restored,
                provider_rehydrated=False,
            )

    provider_dispatch_authorized = bool(
        approve_provider_call
        and (
            restored is None
            or (
                restored.status == "provider_pending"
                and resume_provider
            )
        )
    )
    if (
        restored is not None
        and restored.status == "provider_pending"
        and not provider_dispatch_authorized
    ):
        probe = execute_without_provider()
        _assert_checkpoint_evidence_projection(
            restored,
            evidence=probe.evidence,
            prompt_context=probe.prompt_context,
        )
        return _resumable_result(
            status="provider_pending",
            job_id=normalized_job_id,
            result=probe,
            checkpoint=restored,
            provider_rehydrated=False,
        )

    if provider_dispatch_authorized:
        provider_boundary_crossed = False
        prepared_preflight: AutonomousEvidenceBackedPreflight | None = None
        prepared_operation_digest: str | None = None

        def before_provider(
            preflight: AutonomousEvidenceBackedPreflight,
        ) -> Mapping[str, Any]:
            nonlocal prepared_operation_digest, prepared_preflight
            # Evidence acquisition and prompt construction are caller-extensible.  Recheck the
            # core and effect-boundary identities after those callbacks have completed.
            _assert_provider_transport_graph(
                agent,
                initial_provider_transport_graph,
                run_options=run_options,
            )
            if prepared_preflight is not None:
                raise BrainRunError(
                    "provider preflight was prepared more than once for one operation"
                )
            if restored is not None and restored.status == "provider_pending":
                _assert_checkpoint_evidence_projection(
                    restored,
                    evidence=preflight.evidence,
                    prompt_context=preflight.prompt_context,
                )
            prepared_preflight = preflight
            prepared_operation_digest = _provider_operation_digest_for_preflight(
                job_id=normalized_job_id,
                request_digest=request_digest_value,
                run_policy_digest=run_policy_digest_value,
                preflight=preflight,
            )
            return {
                "provider_idempotency_key": _provider_idempotency_key(
                    prepared_operation_digest
                )
            }

        def commit_before_provider_dispatch(
            attestation: Mapping[str, Any],
        ) -> None:
            nonlocal head, provider_boundary_crossed
            # Route/policy/plan callbacks may run between preflight and the final wire seam.
            # No atomic receipt is written until their opportunity to mutate runtime state has
            # passed and the exact transport request is ready.
            _assert_provider_transport_graph(
                agent,
                initial_provider_transport_graph,
                run_options=run_options,
            )
            if prepared_preflight is None or prepared_operation_digest is None:
                raise BrainRunError(
                    "provider dispatch reached transport without a bound preflight"
                )
            expected_attestation_fields = {
                "provider",
                "model",
                "invocation_kind",
                "dispatch_scope_digest",
                "transport_attempt",
                "request_digest",
                "provider_idempotency_key",
                "provider_config",
                "provider_config_snapshot",
                "provider_transport",
                "provider_http_connection_factory",
                "provider_request",
                "provider_secret",
            }
            if not isinstance(attestation, Mapping) or set(attestation) != (
                expected_attestation_fields
            ):
                raise BrainRunError(
                    "provider dispatch attestation is malformed"
                )
            selected_provider = attestation.get("provider")
            selected_config = attestation.get("provider_config")
            selected_request = attestation.get("provider_request")
            selected_secret = attestation.get("provider_secret")
            registration = next(
                (
                    candidate
                    for candidate in initial_provider_transport_graph.registrations
                    if candidate.provider == selected_provider
                ),
                None,
            )
            if (
                type(selected_provider) is not str
                or type(selected_config) is not ProviderConfig
                or dict.get(
                    initial_provider_transport_graph.providers,
                    selected_provider,
                )
                is not selected_config
                or registration is None
                or registration.config is not selected_config
                or type(selected_request) is not ProviderRequest
                or not _provider_config_snapshot_matches(
                    attestation.get("provider_config_snapshot"),
                    registration.scalar_values,
                )
                or attestation.get("provider_transport")
                is not registration.transport
            ):
                raise BrainRunError(
                    "provider dispatch selected a configuration outside its snapshotted registry"
                )
            requires_credential = next(
                value
                for name, _value_type, value in registration.scalar_values
                if name == "requires_credential"
            )
            selected_credential_registration = next(
                (
                    credential
                    for credential in initial_provider_transport_graph.credential_registrations
                    if credential.provider == selected_provider
                    and credential.secret is selected_secret
                ),
                None,
            )
            if (
                requires_credential
                and selected_credential_registration is None
            ) or (not requires_credential and selected_secret is not None):
                raise BrainRunError(
                    "provider dispatch selected credential state outside its snapshotted store"
                )
            expected_http_factory = None
            if registration.transport is None:
                base_url = next(
                    value
                    for name, _value_type, value in registration.scalar_values
                    if name == "base_url"
                )
                expected_http_factory = (
                    _FROZEN_HTTPS_CONNECTION
                    if base_url.startswith("https://")
                    else _FROZEN_HTTP_CONNECTION
                )
            if attestation.get(
                "provider_http_connection_factory"
            ) is not expected_http_factory:
                raise BrainRunError(
                    "provider dispatch selected a transport outside its snapshotted registry"
                )
            maximum_attempts = next(
                value
                for name, _value_type, value in registration.scalar_values
                if name == "max_attempts"
            )
            if attestation.get("transport_attempt") > maximum_attempts:
                raise BrainRunError(
                    "provider dispatch attempt exceeds its snapshotted provider policy"
                )
            previous_count = (
                0 if head is None else head.provider_dispatch_count
            )
            previous_dispatch_head = (
                None if head is None else head.provider_dispatch_head_digest
            )
            receipt = AutonomousEvidenceBackedProviderDispatchReceipt(
                job_id=normalized_job_id,
                provider_operation_digest=prepared_operation_digest,
                dispatch_index=previous_count + 1,
                previous_provider_dispatch_head_digest=previous_dispatch_head,
                provider=attestation.get("provider"),
                model=attestation.get("model"),
                invocation_kind=attestation.get("invocation_kind"),
                dispatch_scope_digest=attestation.get("dispatch_scope_digest"),
                transport_attempt=attestation.get("transport_attempt"),
                request_digest=attestation.get("request_digest"),
                provider_idempotency_key=attestation.get(
                    "provider_idempotency_key"
                ),
            )
            next_in_flight = _checkpoint_for_preflight(
                job_id=normalized_job_id,
                request_digest=request_digest_value,
                run_policy_digest=run_policy_digest_value,
                preflight=prepared_preflight,
                predecessor=head,
                status="provider_in_flight",
                provider_dispatch_count=receipt.dispatch_index,
                provider_dispatch_head_digest=receipt.receipt_digest,
            )
            committed_in_flight = _compare_and_store_provider_dispatch(
                checkpoint_dispatch_compare_and_store,
                head,
                next_in_flight,
                receipt,
            )
            # The atomic store is caller-owned and can mutate the registry or concrete handler
            # before acknowledging its write.  Recheck its exact true acknowledgement before a
            # socket constructor or in-memory handler is allowed to run.
            _assert_provider_transport_graph(
                agent,
                initial_provider_transport_graph,
                run_options=run_options,
            )
            if (
                selected_credential_registration is not None
                and selected_credential_registration.expires_at is not None
            ):
                current_credential_time = (
                    initial_provider_transport_graph.credential_clock()
                )
                if (
                    type(current_credential_time) not in {int, float}
                    or current_credential_time != current_credential_time
                    or current_credential_time
                    in {float("inf"), float("-inf")}
                    or current_credential_time
                    >= selected_credential_registration.expires_at
                ):
                    raise BrainRunError(
                        "provider credential expired before durable dispatch completed"
                    )
                # The clock is caller-supplied. It must not be the final callback before wire
                # access, so attest the complete graph once more after reading it.
                _assert_provider_transport_graph(
                    agent,
                    initial_provider_transport_graph,
                    run_options=run_options,
                )
            head = committed_in_flight
            provider_boundary_crossed = True

        result = _FROZEN_EVIDENCE_RUNNER(
            agent,
            **common,
            approve_provider_call=True,
            before_provider_run=before_provider,
            before_provider_dispatch=commit_before_provider_dispatch,
        )
        # Outcome observers run after the socket/in-memory handler and remain caller-owned.
        # Refuse to persist their result if they changed the transport graph behind the policy
        # snapshot; any additional retry would already have passed this same fence.
        _assert_provider_transport_graph(
            agent,
            initial_provider_transport_graph,
            run_options=run_options,
        )
        if provider_boundary_crossed:
            return persist_result(
                result,
                "completed"
                if result.evidence is not None
                and result.evidence.status == "completed"
                and _provider_execution_completed(result)
                else "provider_reconciliation_required",
                require_compare_and_store=True,
                provider_result_observed=True,
            )
        if restored is not None and restored.status == "provider_pending":
            _assert_checkpoint_evidence_projection(
                restored,
                evidence=result.evidence,
                prompt_context=result.prompt_context,
            )
            return _resumable_result(
                status="provider_pending",
                job_id=normalized_job_id,
                result=result,
                checkpoint=restored,
                provider_rehydrated=False,
            )
        if prepared_preflight is not None:
            safe_status = _checkpoint_status_for_result(result)
            if safe_status in {
                "completed",
                "provider_reconciliation_required",
            }:
                safe_status = "provider_pending"
            return persist_result(result, safe_status)
    else:
        result = execute_without_provider()
    return persist_result(result)


class AutonomousEvidenceBackedController:
    """Serialize local resumable operations and fence optional shared persistence."""

    def __init__(self, agent: Any, job_id: str, persistence: AutonomousEvidenceBackedCheckpointStore) -> None:
        if not hasattr(agent, "run_with_reviewed_evidence") or not callable(agent.run_with_reviewed_evidence):
            raise BrainRunError("evidence-backed controller requires an AutonomousAgent")
        self.agent = agent
        self.job_id = _identifier("evidence-backed controller job_id", job_id)
        if not all(
            callable(getattr(persistence, name, None))
            for name in ("read", "write")
        ):
            raise ArgumentError(
                "evidence-backed controller persistence requires read and write"
            )
        self.persistence = persistence
        self._checkpoint: AutonomousEvidenceBackedCheckpoint | None = None
        self._expected_checkpoint_digest: str | None = None
        self._status = "empty"
        self._running = False
        self._lock = Lock()

    def _projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_CONTROLLER_SCHEMA,
            "status": self._status,
            "job_id": self.job_id,
            "checkpoint_digest": None if self._checkpoint is None else self._checkpoint.checkpoint_digest,
            "persisted": True,
            "retention": _CONTROLLER_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    def restore(self) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("evidence-backed controller is already running")
            raw = self.persistence.read()
            if raw is None:
                self._checkpoint = None
                self._expected_checkpoint_digest = None
                self._status = "empty"
            else:
                self._checkpoint = validate_autonomous_evidence_backed_checkpoint(raw)
                self._expected_checkpoint_digest = self._checkpoint.checkpoint_digest
                self._status = "restored"
            return self._projection()

    def _persist(self, checkpoint: AutonomousEvidenceBackedCheckpoint) -> None:
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        _assert_ordinary_checkpoint_transition(self._checkpoint, verified)
        if callable(getattr(self.persistence, "write_if_unchanged", None)):
            if not self._compare_and_store(
                self._expected_checkpoint_digest,
                verified,
            ):
                raise BrainRunError(
                    "evidence-backed checkpoint compare-and-swap conflict; reload before continuing"
                )
            return
        self.persistence.write(verified.to_dict())
        self._checkpoint = verified
        self._expected_checkpoint_digest = verified.checkpoint_digest
        self._status = verified.status

    def _invalidate_cached_head(self) -> None:
        with self._lock:
            self._checkpoint = None
            self._expected_checkpoint_digest = None
            self._status = "reload_required"

    def _compare_and_store(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: AutonomousEvidenceBackedCheckpoint,
    ) -> bool:
        if expected_checkpoint_digest != self._expected_checkpoint_digest:
            self._invalidate_cached_head()
            return False
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if not callable(write_if_unchanged):
            raise ArgumentError(
                "provider dispatch requires persistence.write_if_unchanged"
            )
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        _assert_ordinary_checkpoint_transition(self._checkpoint, verified)
        try:
            stored = write_if_unchanged(
                expected_checkpoint_digest,
                verified.to_dict(),
            )
        except BaseException:
            self._invalidate_cached_head()
            raise
        if not isinstance(stored, bool):
            self._invalidate_cached_head()
            raise ArgumentError(
                "evidence-backed controller persistence returned a non-boolean CAS result"
            )
        if not stored:
            self._invalidate_cached_head()
            return False
        self._checkpoint = verified
        self._expected_checkpoint_digest = verified.checkpoint_digest
        self._status = verified.status
        return True

    def _dispatch_compare_and_store(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: AutonomousEvidenceBackedCheckpoint,
        private_receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
    ) -> bool:
        with self._lock:
            if expected_checkpoint_digest != self._expected_checkpoint_digest:
                self._checkpoint = None
                self._expected_checkpoint_digest = None
                self._status = "reload_required"
                return False
        write_dispatch = getattr(
            self.persistence,
            "write_dispatch_if_unchanged",
            None,
        )
        if not callable(write_dispatch):
            raise ArgumentError(
                "provider dispatch requires persistence.write_dispatch_if_unchanged"
            )
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)
        receipt = validate_autonomous_evidence_backed_provider_dispatch_receipt(
            private_receipt
        )
        with self._lock:
            previous = self._checkpoint
        _assert_provider_dispatch_commit(previous, verified, receipt)
        try:
            stored = write_dispatch(
                expected_checkpoint_digest,
                verified.to_dict(),
                receipt.to_private_dict(),
            )
        except BaseException:
            self._invalidate_cached_head()
            raise
        if stored is not True:
            self._invalidate_cached_head()
            return False
        with self._lock:
            self._checkpoint = verified
            self._expected_checkpoint_digest = verified.checkpoint_digest
            self._status = verified.status
        return True

    def flush(self) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("evidence-backed controller is already running")
            if self._checkpoint is not None:
                raw = self.persistence.read()
                if raw is None:
                    raise BrainRunError(
                        "evidence-backed checkpoint disappeared from persistence"
                    )
                observed = validate_autonomous_evidence_backed_checkpoint(raw)
                if observed.checkpoint_digest != self._checkpoint.checkpoint_digest:
                    raise BrainRunError(
                        "evidence-backed checkpoint compare-and-swap conflict; reload before continuing"
                    )
            return self._projection()

    def run(self, *, task: str, **options: Any) -> dict[str, Any]:
        with self._lock:
            if self._running:
                raise BrainRunError("evidence-backed controller is already running")
            if self._checkpoint is None:
                raw = self.persistence.read()
                if raw is not None:
                    self._checkpoint = validate_autonomous_evidence_backed_checkpoint(raw)
                    self._expected_checkpoint_digest = self._checkpoint.checkpoint_digest
            self._running = True
        try:
            if any(
                key in options
                for key in {
                    "job_id",
                    "checkpoint",
                    "checkpoint_sink",
                    "checkpoint_compare_and_store",
                    "checkpoint_dispatch_compare_and_store",
                }
            ):
                raise ArgumentError(
                    "controller owns job_id, checkpoint, checkpoint_sink, and checkpoint_compare_and_store"
                )
            run = run_autonomous_evidence_backed_resumable(
                self.agent,
                task=task,
                job_id=self.job_id,
                checkpoint=None if self._checkpoint is None else self._checkpoint.to_dict(),
                checkpoint_sink=self._persist,
                checkpoint_compare_and_store=(
                    self._compare_and_store
                    if callable(
                        getattr(self.persistence, "write_if_unchanged", None)
                    )
                    else None
                ),
                checkpoint_dispatch_compare_and_store=(
                    self._dispatch_compare_and_store
                    if callable(
                        getattr(
                            self.persistence,
                            "write_dispatch_if_unchanged",
                            None,
                        )
                    )
                    else None
                ),
                **options,
            )
            with self._lock:
                self._checkpoint = run.checkpoint
                self._expected_checkpoint_digest = run.checkpoint.checkpoint_digest
                self._status = run.status
            return {"controller": self._projection(), "run": run}
        except BaseException:
            # A dispatch transaction can commit its in-flight head and then fail the mandatory
            # post-CAS transport revalidation.  Never let the controller retain that attempted
            # head as locally runnable state; the next operation must explicitly reload and
            # reconcile it from persistence.
            with self._lock:
                if (
                    self._checkpoint is not None
                    and self._checkpoint.status == "provider_in_flight"
                ):
                    self._checkpoint = None
                    self._expected_checkpoint_digest = None
                    self._status = "reload_required"
            raise
        finally:
            with self._lock:
                self._running = False


__all__ = [
    "AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_CONTROLLER_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_GENERATION",
    "MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES",
    "AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_STATUSES",
    "AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_STATUSES",
    "AutonomousEvidenceBackedCheckpoint",
    "validate_autonomous_evidence_backed_checkpoint",
    "AutonomousEvidenceBackedProviderDispatchReceipt",
    "validate_autonomous_evidence_backed_provider_dispatch_receipt",
    "AutonomousEvidenceBackedCheckpointStore",
    "TransactionalAutonomousEvidenceBackedCheckpointStore",
    "AutonomousEvidenceBackedCheckpointTextStore",
    "TransactionalAutonomousEvidenceBackedCheckpointTextStore",
    "InMemoryAutonomousEvidenceBackedCheckpointStore",
    "JsonAutonomousEvidenceBackedCheckpointPersistence",
    "TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence",
    "AutonomousEvidenceBackedResumableRun",
    "run_autonomous_evidence_backed_resumable",
    "AutonomousEvidenceBackedController",
]
