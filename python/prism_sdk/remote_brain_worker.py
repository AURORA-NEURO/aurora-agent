"""Remote, metadata-only execution for the Python autonomous brain.

The local :class:`~prism_sdk.control_plane.BrainWorker` owns a SQLite job store and the full
restart-safe checkpoint implementation.  This module is the deployment boundary for callers
whose durable authority is a remote ``brain_job_*`` control plane instead.  It deliberately does
not emulate a local store: private orchestration kwargs remain in the resolver process while the
remote service receives only bounded lifecycle metadata and digests.
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, replace
import hashlib
import inspect
import json
import threading
from typing import Any, Awaitable, Callable, Mapping, Protocol, Sequence

from .brain import BrainRunError
from .brain_api import AsyncBrainControlClient, BrainControlClient
from .autonomous_action_execution import AutonomousActionAdmission
from .autonomous_action_plan import AutonomousActionPlan
from .autonomous_action_admission_controller import validate_autonomous_action_dispatch_handoff
from .autonomous_protected_rehydration import AutonomousProtectedRehydrationAdapter


AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA = "bioprism-python-autonomous-remote-brain-worker/0.1"
AUTONOMOUS_REMOTE_BRAIN_JOB_SPEC_SCHEMA = "bioprism-python-autonomous-remote-brain-job-spec/0.1"
AUTONOMOUS_REMOTE_BRAIN_PLAN_SCHEMA = "bioprism-python-autonomous-remote-brain-plan/0.1"
AUTONOMOUS_REMOTE_BRAIN_ROUTE_SCHEMA = "bioprism-python-autonomous-remote-brain-route/0.1"
MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_LEASE_MS = 86_400_000
MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_HEARTBEAT_MS = 300_000
MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_BATCH = 64
MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_EVENT_PAGES = 8
MAX_AUTONOMOUS_REMOTE_BRAIN_REQUEST_BYTES = 256_000
MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES = 2_048

REMOTE_BRAIN_MODES = (
    "autonomous",
    "workflow",
    "workflow_learning",
    "workflow_cycle",
    "workflow_trajectory_learning",
    "cross_domain",
    "cross_domain_learning",
    "cross_domain_trajectory_learning",
    "cross_domain_replan",
)
_APPROVAL_STATUSES = {
    "approval_required",
    "mission_approval_required",
    "route_review_required",
    "plan_review_required",
    "connector_blocked",
    "waiting_approval",
}
_PRIVATE_KEYS = {
    "task",
    "prompt",
    "credentials",
    "credential",
    "password",
    "secret",
    "token",
    "response",
    "provider_response",
    "tool_arguments",
    "tool_output",
}
_SIDE_EFFECT_BOUNDARIES = frozenset({"not_started", "preflight", "dispatched", "unknown"})


class RemoteBrainWorkerError(BrainRunError):
    """A bounded remote-worker refusal with a stable, caller-visible failure category."""

    def __init__(self, message: str, *, code: str = "configuration", retryable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.retryable = retryable


@dataclass(frozen=True, slots=True)
class RemoteBrainJobSubmission:
    """A value-only remote job admission projection."""

    status: str
    job: Mapping[str, Any] | None
    spec_digest: str
    mode: str
    plan_digest: str | None = None
    route_digest: str | None = None
    action_plan_digest: str | None = None
    action_admission_digest: str | None = None
    action_handoff_digest: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA,
            "status": self.status,
            "job": None if self.job is None else dict(self.job),
            "spec_digest": self.spec_digest,
            "mode": self.mode,
            "plan_digest": self.plan_digest,
            "route_digest": self.route_digest,
            "action_plan_digest": self.action_plan_digest,
            "action_admission_digest": self.action_admission_digest,
            "action_handoff_digest": self.action_handoff_digest,
            "private_spec": "caller_owned;request_and_execution_kwargs_not_sent_to_control_plane",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class RemoteBrainJobRun:
    """One remote lifecycle result; ``result`` remains transient to the caller."""

    status: str
    job: Mapping[str, Any]
    mode: str | None
    result: Any | None = None
    error_class: str | None = None
    failure_code: str | None = None
    error_retryable: bool | None = None
    result_digest: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-autonomous-remote-brain-job-run/0.1",
            "status": self.status,
            "job": dict(self.job),
            "mode": self.mode,
            "result_metadata": {
                "status": _result_status(self.result) if self.result is not None else None,
                "result_digest": self.result_digest,
            },
            "error_class": self.error_class,
            "failure_code": self.failure_code,
            "error_retryable": self.error_retryable,
            "retention": "remote_job_metadata_only;brain_result_transient_to_caller",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class RemoteBrainJobBatch:
    """Bounded pull-loop projection that never serializes provider values."""

    status: str
    runs: tuple[RemoteBrainJobRun, ...]
    claimed_count: int
    succeeded_count: int
    waiting_count: int
    retry_scheduled_count: int
    reconciliation_count: int
    failed_count: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA,
            "status": self.status,
            "runs": [run.to_dict() for run in self.runs],
            "claimed_count": self.claimed_count,
            "succeeded_count": self.succeeded_count,
            "waiting_count": self.waiting_count,
            "retry_scheduled_count": self.retry_scheduled_count,
            "reconciliation_count": self.reconciliation_count,
            "failed_count": self.failed_count,
            "batch_digest": _digest_json([
                {
                    "job_id": run.job.get("job_id"),
                    "status": run.status,
                    "record_digest": run.job.get("record_digest"),
                    "result_digest": run.result_digest,
                }
                for run in self.runs
            ]),
            "retention": "remote_job_metadata_only;brain_results_transient_to_caller",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class RemoteBrainJobResolution:
    """Resolver response containing private execution kwargs and a public spec identity."""

    spec_digest: str
    mode: str
    request: Mapping[str, Any]
    kwargs: Mapping[str, Any]
    policy_digest: str | None = None
    plan_digest: str | None = None
    route_digest: str | None = None
    action_plan: AutonomousActionPlan | Mapping[str, Any] | None = None
    action_admission: AutonomousActionAdmission | Mapping[str, Any] | None = None
    action_handoff: Mapping[str, Any] | None = None


@dataclass(frozen=True, slots=True)
class RemoteBrainCredentialBinding:
    """One approved-attempt snapshot of opaque provider credential handles.

    The binding is deliberately not serializable as a job projection.  ``credentials`` contains
    runtime-owned handles, never raw key material, and ``close`` revokes the snapshot after the
    runner returns, fails, or is cancelled.
    """

    credentials: Mapping[str, Any]
    _close: Callable[[], Any]

    def close(self) -> Any:
        """Revoke the attempt-scoped handles at the worker boundary."""

        return self._close()


class RemoteBrainCredentialScope(Protocol):
    """Provider-neutral hook for opening credentials after durable approval is released."""

    def open(self, context: Mapping[str, Any]) -> RemoteBrainCredentialBinding | Awaitable[RemoteBrainCredentialBinding]:
        """Return an opaque binding for one approved job attempt."""


class ProvisionedRemoteBrainCredentialScope:
    """Adapt ``AutonomousBrain`` provisioning into a durable-worker credential scope.

    A fresh ``CredentialSession`` is created for each approved attempt.  Deployment sources are
    resolved inside ``open``; no key is accepted by this worker or copied into a remote job.  The
    returned binding owns the session's opaque handles until the worker's ``finally`` block closes
    it.
    """

    def __init__(
        self,
        brain: Any,
        *,
        providers: Sequence[str] | None = None,
        ttl_seconds: float | None = None,
        environ: Mapping[str, str] | None = None,
    ) -> None:
        if brain is None or not callable(getattr(brain, "start_provisioned_credential_session", None)):
            raise RemoteBrainWorkerError(
                "provisioned remote credential scope requires a brain facade with credential provisioning"
            )
        if providers is not None:
            if not isinstance(providers, Sequence) or isinstance(providers, (str, bytes)):
                raise RemoteBrainWorkerError("provisioned remote credential providers must be a sequence")
            normalized_providers = tuple(_validate_identifier("credential provider", provider) for provider in providers)
        else:
            normalized_providers = None
        if ttl_seconds is not None and (
            not isinstance(ttl_seconds, (int, float))
            or isinstance(ttl_seconds, bool)
            or ttl_seconds <= 0
        ):
            raise RemoteBrainWorkerError("provisioned remote credential ttl_seconds must be positive or None")
        if environ is not None and not isinstance(environ, Mapping):
            raise RemoteBrainWorkerError("provisioned remote credential environ must be a mapping")
        self.brain = brain
        self.providers = normalized_providers
        self.ttl_seconds = ttl_seconds
        self.environ = None if environ is None else dict(environ)

    def open(self, context: Mapping[str, Any]) -> RemoteBrainCredentialBinding:
        if not isinstance(context, Mapping):
            raise RemoteBrainWorkerError("remote credential scope context must be a mapping")
        job_id = _validate_identifier("remote credential scope job_id", context.get("job_id"))
        attempt = context.get("attempt")
        if not isinstance(attempt, int) or isinstance(attempt, bool) or attempt < 1:
            raise RemoteBrainWorkerError("remote credential scope attempt must be a positive integer")
        if context.get("approval_released") is not True:
            raise RemoteBrainWorkerError("remote credential scope requires released approval")
        session, _provisioning = self.brain.start_provisioned_credential_session(
            providers=self.providers,
            ttl_seconds=self.ttl_seconds,
            environ=self.environ,
            require_ready=True,
        )
        if session is None or not callable(getattr(session, "handles", None)) or not callable(getattr(session, "close", None)):
            if session is not None and callable(getattr(session, "close", None)):
                session.close()
            raise RemoteBrainWorkerError("credential provisioning returned an invalid session")
        try:
            handles = session.handles()
            if not isinstance(handles, Mapping):
                raise RemoteBrainWorkerError("credential provisioning returned invalid opaque handles")
            # The validated context is intentionally not retained: it is useful only while
            # opening this attempt and must not become durable worker state.
            _ = job_id, attempt
            return RemoteBrainCredentialBinding(credentials=dict(handles), _close=session.close)
        except Exception:
            session.close()
            raise


def _assert_scope_resolution_clean(resolution: RemoteBrainJobResolution) -> None:
    """Reject caller-supplied credential hooks before the scope injects its own handles."""

    seen: set[int] = set()

    def scan(value: Any, depth: int = 0) -> None:
        if depth > 16:
            raise RemoteBrainWorkerError("remote brain credential-scoped kwargs are too deeply nested")
        if isinstance(value, Mapping):
            identity = id(value)
            if identity in seen:
                return
            seen.add(identity)
            for key, child in value.items():
                if isinstance(key, str) and key.replace("_", "").lower() in {"credential", "credentials", "credentialfor"}:
                    raise RemoteBrainWorkerError("remote brain credential scope rejects caller-supplied credentials")
                scan(child, depth + 1)
        elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
            identity = id(value)
            if identity in seen:
                return
            seen.add(identity)
            for child in value:
                scan(child, depth + 1)

    scan(resolution.kwargs)


def _validate_credential_binding(value: Any) -> RemoteBrainCredentialBinding:
    if not isinstance(value, RemoteBrainCredentialBinding):
        raise RemoteBrainWorkerError("remote brain credential scope returned an invalid binding")
    if not isinstance(value.credentials, Mapping):
        raise RemoteBrainWorkerError("remote brain credential binding handles must be a mapping")
    if not callable(value.close):
        raise RemoteBrainWorkerError("remote brain credential binding must be closable")
    return value


def _bind_credential_scope_resolution(
    resolution: RemoteBrainJobResolution,
    binding: RemoteBrainCredentialBinding,
) -> RemoteBrainJobResolution:
    """Attach handles only to the transient invocation kwargs after approval."""

    kwargs = dict(resolution.kwargs)
    kwargs["credentials"] = dict(binding.credentials)
    return replace(resolution, kwargs=kwargs)


async def _open_async_credential_scope(
    scope: RemoteBrainCredentialScope,
    context: Mapping[str, Any],
) -> RemoteBrainCredentialBinding:
    opener = scope.open
    if inspect.iscoroutinefunction(opener):
        opened = await opener(context)
    else:
        opened = await asyncio.to_thread(opener, context)
    if inspect.isawaitable(opened):
        opened = await opened
    return _validate_credential_binding(opened)


async def _close_async_credential_binding(binding: RemoteBrainCredentialBinding) -> None:
    closed = await asyncio.to_thread(binding.close)
    if inspect.isawaitable(closed):
        await closed


RemoteBrainJobResolver = Callable[[Mapping[str, Any]], RemoteBrainJobResolution | Mapping[str, Any]]
AsyncRemoteBrainJobResolver = Callable[[Mapping[str, Any]], Any]


@dataclass(frozen=True, slots=True)
class RemoteBrainProtectedRehydrationContext:
    """Metadata-only identity presented to a protected remote-job receipt resolver."""

    job_id: str
    spec_digest: str
    domain: str
    capability: str
    attempt: int
    approval_released: bool

    def __post_init__(self) -> None:
        _validate_identifier("protected remote brain job_id", self.job_id)
        _validate_digest("protected remote brain spec_digest", self.spec_digest)
        _validate_identifier("protected remote brain domain", self.domain)
        _validate_identifier("protected remote brain capability", self.capability)
        if not isinstance(self.attempt, int) or isinstance(self.attempt, bool) or self.attempt < 1:
            raise RemoteBrainWorkerError("protected remote brain attempt must be a positive integer")
        if not isinstance(self.approval_released, bool):
            raise RemoteBrainWorkerError("protected remote brain approval_released must be boolean")

    def to_dict(self) -> dict[str, Any]:
        return {
            "job_id": self.job_id,
            "spec_digest": self.spec_digest,
            "domain": self.domain,
            "capability": self.capability,
            "attempt": self.attempt,
            "approval_released": self.approval_released,
        }


RemoteBrainProtectedReceiptResolver = Callable[
    [RemoteBrainProtectedRehydrationContext],
    Mapping[str, Any] | Awaitable[Mapping[str, Any]],
]


@dataclass(frozen=True, slots=True)
class RemoteBrainProtectedRehydration:
    """Rehydrate a private job resolution from a caller-owned protected receipt.

    The worker stores neither the receipt nor the returned resolution.  The receipt resolver is
    given only durable metadata and must return a short-lived receipt whose identity exactly
    matches that metadata before the shared protected boundary releases the value.
    """

    adapter: AutonomousProtectedRehydrationAdapter
    receipt_resolver: RemoteBrainProtectedReceiptResolver
    value_decoder: Callable[[Any], Any] | None = None
    domain: str | None = None
    purpose: str = "remote_brain_job_resolution"
    value_kind: str = "remote_brain_job_resolution"
    one_time: bool = False
    digest_scheme: str = "canonical_json"

    def __post_init__(self) -> None:
        if not isinstance(self.adapter, AutonomousProtectedRehydrationAdapter):
            raise RemoteBrainWorkerError("protected remote brain rehydration requires a protected receipt adapter")
        if not callable(self.receipt_resolver):
            raise RemoteBrainWorkerError("protected remote brain receipt_resolver must be callable")
        if self.value_decoder is not None and not callable(self.value_decoder):
            raise RemoteBrainWorkerError("protected remote brain value_decoder must be callable")
        if self.domain is not None:
            _validate_identifier("protected remote brain domain", self.domain)
        _validate_identifier("protected remote brain purpose", self.purpose)
        _validate_identifier("protected remote brain value_kind", self.value_kind)
        if self.digest_scheme not in {"canonical_json", "utf8_sha256"}:
            raise RemoteBrainWorkerError("protected remote brain digest_scheme is unsupported")
        if not isinstance(self.one_time, bool):
            raise RemoteBrainWorkerError("protected remote brain one_time must be boolean")

    @staticmethod
    def _assert_receipt_identity(receipt: Mapping[str, Any], context: RemoteBrainProtectedRehydrationContext) -> None:
        if not isinstance(receipt, Mapping):
            raise RemoteBrainWorkerError("protected remote brain receipt_resolver must return a mapping", code="protocol")
        expected = context.to_dict()
        for key, value in expected.items():
            if receipt.get(key) != value:
                raise RemoteBrainWorkerError(
                    f"protected remote brain receipt {key} does not match the durable job",
                    code="protocol",
                )

    def _resolve_receipt(self, receipt: Mapping[str, Any], context: RemoteBrainProtectedRehydrationContext) -> Any:
        self._assert_receipt_identity(receipt, context)
        if self.domain is not None and self.domain != context.domain:
            raise RemoteBrainWorkerError("protected remote brain configured domain does not match the durable job", code="protocol")
        try:
            value = self.adapter.resolve_receipt(
                receipt,
                domain=self.domain or context.domain,
                purpose=self.purpose,
                value_kind=self.value_kind,
                one_time=self.one_time,
                digest_scheme=self.digest_scheme,
            )
            return self.value_decoder(value) if self.value_decoder is not None else value
        except RemoteBrainWorkerError:
            raise
        except Exception as error:
            raise RemoteBrainWorkerError("protected remote brain receipt could not be resolved", code="rehydration") from error

    def resolve(self, context: RemoteBrainProtectedRehydrationContext) -> Any:
        if not isinstance(context, RemoteBrainProtectedRehydrationContext):
            raise RemoteBrainWorkerError("protected remote brain rehydration context is malformed")
        receipt = self.receipt_resolver(context)
        if inspect.isawaitable(receipt):
            raise RemoteBrainWorkerError("async protected remote brain receipt resolver requires the async worker", code="configuration")
        return self._resolve_receipt(receipt, context)

    async def resolve_async(self, context: RemoteBrainProtectedRehydrationContext) -> Any:
        if not isinstance(context, RemoteBrainProtectedRehydrationContext):
            raise RemoteBrainWorkerError("protected remote brain rehydration context is malformed")
        try:
            receipt = self.receipt_resolver(context)
            if inspect.isawaitable(receipt):
                receipt = await receipt
            return self._resolve_receipt(receipt, context)
        except RemoteBrainWorkerError:
            raise
        except Exception as error:
            raise RemoteBrainWorkerError("protected remote brain receipt could not be resolved", code="rehydration") from error


def _digest_json(value: Any) -> str:
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise RemoteBrainWorkerError("remote brain digest input must be JSON-safe") from error
    return hashlib.sha256(encoded).hexdigest()


def _reviewed_value_digest(value: Any, *, name: str, schema: str) -> str:
    """Digest a caller-owned reviewed value without putting that value on the remote queue."""

    if hasattr(value, "to_dict") and callable(value.to_dict):
        value = value.to_dict()
    elif isinstance(value, Mapping):
        value = dict(value)
    else:
        raise RemoteBrainWorkerError(f"{name} must be a mapping or expose to_dict()")
    if not isinstance(value, Mapping):
        raise RemoteBrainWorkerError(f"{name}.to_dict() must return a mapping")
    safe_value = _bounded_json(name, value, MAX_AUTONOMOUS_REMOTE_BRAIN_REQUEST_BYTES)
    return _digest_json({"schema": schema, name: safe_value})


def autonomous_remote_brain_plan_digest(blueprint: Any) -> str:
    """Return the stable identity of a caller-owned reviewed plan/blueprint."""

    return _reviewed_value_digest(blueprint, name="blueprint", schema=AUTONOMOUS_REMOTE_BRAIN_PLAN_SCHEMA)


def autonomous_remote_brain_route_digest(route: Any) -> str:
    """Return the stable identity of a caller-owned reviewed routing proposal."""

    return _reviewed_value_digest(route, name="route", schema=AUTONOMOUS_REMOTE_BRAIN_ROUTE_SCHEMA)


def autonomous_remote_brain_job_spec_digest(
    *,
    request: Mapping[str, Any],
    mode: str,
    policy_digest: str | None = None,
    plan_digest: str | None = None,
    route_digest: str | None = None,
    action_plan_digest: str | None = None,
    action_admission_digest: str | None = None,
    action_handoff_digest: str | None = None,
) -> str:
    """Bind request/mode/policy and optional reviewed identities without retaining private values.

    The optional fields are omitted, rather than serialized as ``null``, so jobs created by
    older callers retain their exact pre-extension digest.
    """

    _validate_mode(mode)
    if not isinstance(request, Mapping):
        raise RemoteBrainWorkerError("remote brain job request must be a mapping")
    _bounded_json("remote brain job request", request, MAX_AUTONOMOUS_REMOTE_BRAIN_REQUEST_BYTES)
    policy_digest = _validate_optional_digest("policy_digest", policy_digest)
    plan_digest = _validate_optional_digest("plan_digest", plan_digest)
    route_digest = _validate_optional_digest("route_digest", route_digest)
    action_plan_digest = _validate_optional_digest("action_plan_digest", action_plan_digest)
    action_admission_digest = _validate_optional_digest("action_admission_digest", action_admission_digest)
    action_handoff_digest = _validate_optional_digest("action_handoff_digest", action_handoff_digest)
    if action_admission_digest is not None and action_plan_digest is None:
        raise RemoteBrainWorkerError("action_admission_digest requires action_plan_digest")
    if action_handoff_digest is not None and (action_plan_digest is None or action_admission_digest is None):
        raise RemoteBrainWorkerError("action_handoff_digest requires action plan and admission digests")
    payload: dict[str, Any] = {
        "schema": AUTONOMOUS_REMOTE_BRAIN_JOB_SPEC_SCHEMA,
        "mode": mode,
        "request": dict(request),
        "policy_digest": policy_digest,
    }
    if plan_digest is not None:
        payload["plan_digest"] = plan_digest
    if route_digest is not None:
        payload["route_digest"] = route_digest
    if action_plan_digest is not None:
        payload["action_plan_digest"] = action_plan_digest
    if action_admission_digest is not None:
        payload["action_admission_digest"] = action_admission_digest
    if action_handoff_digest is not None:
        payload["action_handoff_digest"] = action_handoff_digest
    return _digest_json(payload)


def autonomous_remote_brain_job_spec_digest_for_handoff(
    *,
    request: Mapping[str, Any],
    mode: str,
    action_handoff: Mapping[str, Any],
    policy_digest: str | None = None,
) -> str:
    """Compute a durable job identity from a validated dispatch handoff."""

    handoff = _action_handoff_value(action_handoff)
    if handoff is None:
        raise RemoteBrainWorkerError("action_handoff must be a metadata mapping", code="protocol")
    return autonomous_remote_brain_job_spec_digest(
        request=request,
        mode=mode,
        policy_digest=policy_digest,
        action_plan_digest=handoff["plan_digest"],
        action_admission_digest=handoff["admission_digest"],
        action_handoff_digest=handoff["handoff_digest"],
    )


def _bounded_json(name: str, value: Any, maximum: int) -> Any:
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise RemoteBrainWorkerError(f"{name} must be JSON-safe") from error
    if len(encoded) > maximum:
        raise RemoteBrainWorkerError(f"{name} exceeds its bounded size")
    return json.loads(encoded.decode("utf-8"))


def _validate_identifier(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise RemoteBrainWorkerError(f"{name} must be a bounded non-empty string")
    return value


def _validate_digest(name: str, value: Any) -> str:
    value = _validate_identifier(name, value, 64)
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise RemoteBrainWorkerError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _validate_optional_digest(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _validate_digest(name, value)


def _validate_mode(value: Any) -> str:
    if value not in REMOTE_BRAIN_MODES:
        raise RemoteBrainWorkerError("remote brain job mode is unsupported")
    return str(value)


def _assert_no_private_fields(value: Any, depth: int = 0) -> None:
    if depth > 8:
        raise RemoteBrainWorkerError("remote brain control-plane projection is too deeply nested", code="protocol")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if isinstance(key, str) and key.lower() in _PRIVATE_KEYS:
                raise RemoteBrainWorkerError("remote brain control-plane projection contains private material", code="protocol")
            _assert_no_private_fields(child, depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for child in value:
            _assert_no_private_fields(child, depth + 1)


def _job_projection(payload: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(payload, Mapping) or not isinstance(payload.get("job"), Mapping):
        raise RemoteBrainWorkerError("remote brain control plane returned a malformed job", code="protocol")
    _assert_no_private_fields(payload)
    job = dict(payload["job"])
    _validate_identifier("remote brain job_id", job.get("job_id"))
    _validate_digest("remote brain spec_digest", job.get("spec_digest"))
    for field in ("domain", "capability", "risk_class", "state"):
        _validate_identifier(f"remote brain job {field}", job.get(field))
    if job.get("side_effect_boundary") not in _SIDE_EFFECT_BOUNDARIES:
        raise RemoteBrainWorkerError("remote brain job side_effect_boundary is malformed", code="protocol")
    attempts = job.get("attempts")
    if not isinstance(attempts, int) or isinstance(attempts, bool) or attempts < 0:
        raise RemoteBrainWorkerError("remote brain job attempts are malformed", code="protocol")
    if job.get("record_digest") is not None:
        _validate_digest("remote brain record_digest", job.get("record_digest"))
    return job


def _result_status(result: Any) -> str:
    value = result.get("status") if isinstance(result, Mapping) else getattr(result, "status", None)
    if not isinstance(value, str) or not value.strip():
        raise RemoteBrainWorkerError("remote brain execution returned no bounded status", code="protocol")
    return value


def _result_digest(result: Any, mode: str, job_id: str) -> str:
    metadata: dict[str, Any] = {"schema": AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA, "job_id": job_id, "mode": mode, "status": _result_status(result)}
    for name in ("outcome_digest", "plan_digest", "workflow_digest", "checkpoint_digest", "run_id", "cycle_id", "trajectory_id", "learning_episode_ids"):
        value = result.get(name) if isinstance(result, Mapping) else getattr(result, name, None)
        if isinstance(value, (str, int, float, bool)) or value is None or isinstance(value, (list, tuple)) and all(isinstance(item, (str, int, float, bool)) for item in value):
            metadata[name] = value
    return _digest_json(metadata)


def _error_projection(error: BaseException) -> tuple[str, str, bool | None]:
    error_class = type(error).__name__
    retryable = getattr(error, "retryable", None)
    if not isinstance(retryable, bool):
        retryable = None
    code = getattr(error, "code", None)
    if not isinstance(code, str) or not code:
        code = "error"
    return error_class if error_class.isidentifier() else "RemoteBrainWorkerError", code, retryable


def _mapping_value(value: Any, key: str) -> Any:
    if isinstance(value, Mapping):
        return value.get(key)
    return getattr(value, key, None)


def _action_plan_value(value: Any) -> AutonomousActionPlan | None:
    if value is None:
        return None
    if isinstance(value, AutonomousActionPlan):
        return value
    if not isinstance(value, Mapping):
        raise RemoteBrainWorkerError("remote brain action_plan must be metadata mapping", code="protocol")
    _assert_no_private_fields(value)
    try:
        return AutonomousActionPlan.from_dict(value)
    except Exception as error:
        raise RemoteBrainWorkerError("remote brain action_plan metadata is invalid", code="protocol") from error


def _action_admission_value(value: Any) -> AutonomousActionAdmission | None:
    if value is None:
        return None
    if isinstance(value, AutonomousActionAdmission):
        return value
    if not isinstance(value, Mapping):
        raise RemoteBrainWorkerError("remote brain action_admission must be metadata mapping", code="protocol")
    _assert_no_private_fields(value)
    try:
        return AutonomousActionAdmission.from_dict(value)
    except Exception as error:
        raise RemoteBrainWorkerError("remote brain action_admission metadata is invalid", code="protocol") from error


def _action_handoff_value(value: Any) -> dict[str, Any] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise RemoteBrainWorkerError("remote brain action_handoff must be metadata mapping", code="protocol")
    _assert_no_private_fields(value)
    try:
        return validate_autonomous_action_dispatch_handoff(value)
    except Exception as error:
        raise RemoteBrainWorkerError("remote brain action_handoff metadata is invalid", code="protocol") from error


class RemoteBrainJobWorker:
    """Pull and execute private Python brain requests through a remote job control plane.

    ``brain`` may be :class:`~prism_sdk.brain.AutonomousBrain` or an equivalent facade exposing
    the documented ``run_autonomous``, ``run_workflow*``, and ``run_cross_domain*`` methods. The
    resolver decides which private kwargs are appropriate for the selected mode; no kwargs are
    sent to :class:`BrainControlClient`.
    """

    _RUNNERS = {
        "autonomous": "run_autonomous",
        "workflow": "run_workflow",
        "workflow_learning": "run_workflow_learning",
        "workflow_cycle": "run_workflow_cycle",
        "workflow_trajectory_learning": "run_workflow_trajectory_learning",
        "cross_domain": "run_cross_domain",
        "cross_domain_learning": "run_cross_domain_learning",
        "cross_domain_trajectory_learning": "run_cross_domain_trajectory_learning",
        "cross_domain_replan": "run_cross_domain_replan_learning",
    }

    def __init__(
        self,
        brain: Any,
        control: BrainControlClient,
        *,
        worker_id: str,
        resolver: RemoteBrainJobResolver | None = None,
        protected_rehydration: RemoteBrainProtectedRehydration | None = None,
        lease_ms: int = 300_000,
        heartbeat_ms: int | None = None,
        retry_preflight_failures: bool = True,
        credential_scope: RemoteBrainCredentialScope | None = None,
    ) -> None:
        if brain is None:
            raise RemoteBrainWorkerError("remote brain worker requires a brain facade")
        if not all(callable(getattr(brain, name, None)) for name in set(self._RUNNERS.values())):
            raise RemoteBrainWorkerError("remote brain facade does not expose every supported execution mode")
        if not isinstance(control, BrainControlClient):
            raise RemoteBrainWorkerError("remote brain worker requires a BrainControlClient")
        if resolver is not None and not callable(resolver):
            raise RemoteBrainWorkerError("remote brain resolver must be callable")
        if protected_rehydration is not None and not isinstance(protected_rehydration, RemoteBrainProtectedRehydration):
            raise RemoteBrainWorkerError("remote brain protected_rehydration is malformed")
        if resolver is None and protected_rehydration is None:
            raise RemoteBrainWorkerError("remote brain worker requires resolver or protected_rehydration")
        self.brain = brain
        self.control = control
        self.resolver = resolver
        self.protected_rehydration = protected_rehydration
        self.worker_id = _validate_identifier("remote brain worker_id", worker_id)
        if not isinstance(lease_ms, int) or isinstance(lease_ms, bool) or not 100 <= lease_ms <= MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_LEASE_MS:
            raise RemoteBrainWorkerError("remote brain lease_ms is outside its bounds")
        self.lease_ms = lease_ms
        effective_heartbeat = min(30_000, max(1, lease_ms // 3)) if heartbeat_ms is None else heartbeat_ms
        if not isinstance(effective_heartbeat, int) or isinstance(effective_heartbeat, bool) or not 1 <= effective_heartbeat <= MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_HEARTBEAT_MS or effective_heartbeat >= lease_ms:
            raise RemoteBrainWorkerError("remote brain heartbeat_ms must be bounded and less than lease_ms")
        self.heartbeat_ms = effective_heartbeat
        if not isinstance(retry_preflight_failures, bool):
            raise RemoteBrainWorkerError("remote brain retry_preflight_failures must be boolean")
        if credential_scope is not None and not callable(getattr(credential_scope, "open", None)):
            raise RemoteBrainWorkerError("remote brain credential_scope must expose open(context)")
        self.retry_preflight_failures = retry_preflight_failures
        self.credential_scope = credential_scope

    def submit(
        self,
        *,
        idempotency_key: str,
        request: Mapping[str, Any],
        mode: str,
        domain: str,
        capability: str,
        risk_class: str,
        policy_digest: str | None = None,
        plan_digest: str | None = None,
        route_digest: str | None = None,
        action_plan_digest: str | None = None,
        action_admission_digest: str | None = None,
        action_handoff_digest: str | None = None,
        priority: int = 0,
        max_attempts: int = 3,
        checkpoint_digest: str | None = None,
    ) -> RemoteBrainJobSubmission:
        idempotency_key = _validate_identifier("remote brain idempotency_key", idempotency_key)
        mode = _validate_mode(mode)
        domain = _validate_identifier("remote brain domain", domain)
        capability = _validate_identifier("remote brain capability", capability)
        risk_class = _validate_identifier("remote brain risk_class", risk_class)
        if not isinstance(priority, int) or isinstance(priority, bool) or not 0 <= priority <= 255:
            raise RemoteBrainWorkerError("remote brain priority is outside its bounds")
        if not isinstance(max_attempts, int) or isinstance(max_attempts, bool) or not 1 <= max_attempts <= 8:
            raise RemoteBrainWorkerError("remote brain max_attempts is outside its bounds")
        policy_digest = _validate_optional_digest("policy_digest", policy_digest)
        plan_digest = _validate_optional_digest("plan_digest", plan_digest)
        route_digest = _validate_optional_digest("route_digest", route_digest)
        action_plan_digest = _validate_optional_digest("action_plan_digest", action_plan_digest)
        action_admission_digest = _validate_optional_digest("action_admission_digest", action_admission_digest)
        action_handoff_digest = _validate_optional_digest("action_handoff_digest", action_handoff_digest)
        checkpoint_digest = _validate_optional_digest("checkpoint_digest", checkpoint_digest)
        spec_digest = autonomous_remote_brain_job_spec_digest(
            request=request,
            mode=mode,
            policy_digest=policy_digest,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=action_plan_digest,
            action_admission_digest=action_admission_digest,
            action_handoff_digest=action_handoff_digest,
        )
        payload = self.control.submit_job({
            "idempotency_key": idempotency_key,
            "spec_digest": spec_digest,
            "domain": domain,
            "capability": capability,
            "risk_class": risk_class,
            "priority": priority,
            "max_attempts": max_attempts,
            **({"checkpoint_digest": checkpoint_digest} if checkpoint_digest is not None else {}),
        })
        return RemoteBrainJobSubmission(
            status="submitted",
            job=_job_projection(payload),
            spec_digest=spec_digest,
            mode=mode,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=action_plan_digest,
            action_admission_digest=action_admission_digest,
            action_handoff_digest=action_handoff_digest,
        )

    def submit_handoff(
        self,
        *,
        idempotency_key: str,
        request: Mapping[str, Any],
        mode: str,
        domain: str,
        capability: str,
        risk_class: str,
        action_handoff: Mapping[str, Any],
        policy_digest: str | None = None,
        plan_digest: str | None = None,
        route_digest: str | None = None,
        priority: int = 0,
        max_attempts: int = 3,
        checkpoint_digest: str | None = None,
    ) -> RemoteBrainJobSubmission:
        """Submit a job whose action identity is derived from one verified handoff."""

        handoff = _action_handoff_value(action_handoff)
        if handoff is None:
            raise RemoteBrainWorkerError("action_handoff must be a metadata mapping", code="protocol")
        if plan_digest is not None and plan_digest != handoff["plan_digest"]:
            raise RemoteBrainWorkerError("plan_digest does not match the verified action handoff", code="protocol")
        return self.submit(
            idempotency_key=idempotency_key,
            request=request,
            mode=mode,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            policy_digest=policy_digest,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=handoff["plan_digest"],
            action_admission_digest=handoff["admission_digest"],
            action_handoff_digest=handoff["handoff_digest"],
            priority=priority,
            max_attempts=max_attempts,
            checkpoint_digest=checkpoint_digest,
        )

    def status(self, job_id: str) -> Mapping[str, Any]:
        return _job_projection(self.control.job_status(_validate_identifier("remote brain job_id", job_id)))

    def approval(self, job_id: str, action: str, *, reason: str | None = None, authorization_digest: str | None = None) -> Mapping[str, Any]:
        if action not in {"request", "approve", "deny"}:
            raise RemoteBrainWorkerError("remote brain approval action is invalid")
        if action in {"approve", "deny"}:
            _validate_digest("authorization_digest", authorization_digest)
        if reason is not None:
            _validate_identifier("approval reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES)
        payload: dict[str, Any] = {"job_id": _validate_identifier("remote brain job_id", job_id), "action": action}
        if reason is not None:
            payload["reason"] = reason
        if authorization_digest is not None:
            payload["authorization_digest"] = authorization_digest
        response = self.control.approval(payload)
        _assert_no_private_fields(response)
        return dict(response)

    def reconcile(self, job_id: str, *, outcome: str, evidence_digest: str, evidence_kind: str = "caller_observation", operator: str = "caller", reason: str = "caller reconciled uncertain external state", effect_absent: bool = False) -> Mapping[str, Any]:
        if outcome not in {"succeeded", "failed", "not_executed", "unknown"}:
            raise RemoteBrainWorkerError("remote brain reconciliation outcome is invalid")
        if outcome == "not_executed" and effect_absent is not True:
            raise RemoteBrainWorkerError("not_executed reconciliation requires effect_absent=True")
        payload = self.control.reconcile_job({
            "job_id": _validate_identifier("remote brain job_id", job_id),
            "outcome": outcome,
            "evidence_digest": _validate_digest("evidence_digest", evidence_digest),
            "evidence_kind": _validate_identifier("evidence_kind", evidence_kind, 128),
            "operator": _validate_identifier("operator", operator),
            "reason": _validate_identifier("reconciliation reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
            "effect_absent": effect_absent,
        })
        _assert_no_private_fields(payload)
        return dict(payload)

    def cancel(self, job_id: str, *, reason: str = "cancelled by caller") -> Mapping[str, Any]:
        payload = self.control.cancel_job({
            "job_id": _validate_identifier("remote brain job_id", job_id),
            "reason": _validate_identifier("cancellation reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
        })
        _assert_no_private_fields(payload)
        return dict(payload)

    def run_once(self, job_id: str | None = None) -> RemoteBrainJobRun | None:
        if job_id is None:
            claimed_payload = self.control.claim_next_job({"worker_id": self.worker_id, "lease_ms": self.lease_ms})
            _assert_no_private_fields(claimed_payload)
            if claimed_payload.get("claimed") is not True or claimed_payload.get("job") is None:
                return None
        else:
            claimed_payload = self.control.claim_job({"job_id": _validate_identifier("remote brain job_id", job_id), "worker_id": self.worker_id, "lease_ms": self.lease_ms})
            _assert_no_private_fields(claimed_payload)
        job = _job_projection(claimed_payload)
        if job.get("lease_owner") != self.worker_id or job.get("state") not in {"leased", "running"}:
            raise RemoteBrainWorkerError("remote brain control plane returned a job without this worker lease", code="protocol")

        stop = threading.Event()
        heartbeat_error: list[BaseException] = []

        def heartbeat() -> None:
            while not stop.wait(self.heartbeat_ms / 1000.0):
                try:
                    renewed = self.control.renew_job({"job_id": job["job_id"], "worker_id": self.worker_id, "lease_ms": self.lease_ms})
                    _job_projection(renewed)
                except Exception as error:  # pragma: no cover - timing dependent; exercised through the boundary check
                    heartbeat_error.append(error)
                    stop.set()

        thread = threading.Thread(target=heartbeat, name=f"aurora-remote-brain-heartbeat-{self.worker_id}", daemon=True)
        thread.start()
        started = False
        resolution: RemoteBrainJobResolution | None = None
        credential_binding: RemoteBrainCredentialBinding | None = None
        try:
            approval_released = self._approval_released(job["job_id"])
            # Re-entry after approval or a retry must preserve the durable monotonic effect
            # boundary.  Replaying a preflight checkpoint as ``not_started`` would be a
            # backwards transition and would be correctly rejected by the control plane.
            self._checkpoint(job["job_id"], "resolving_private_spec", job["side_effect_boundary"], {"job_id": job["job_id"], "spec_digest": job["spec_digest"], "attempt": job["attempts"]})
            resolution = self._resolve(job, approval_released)
            self._validate_resolution(job, resolution)
            if heartbeat_error:
                raise RemoteBrainWorkerError("remote brain lease heartbeat failed before dispatch", code="transport", retryable=True)
            if not approval_released:
                # Request approval while the worker still owns the lease.  The durable Python
                # router records the approval metadata and performs the running -> waiting
                # transition atomically; checkpointing to waiting first would discard the
                # lease needed to attach that request.
                self._checkpoint(job["job_id"], "provider_approval_required", "preflight", {"spec_digest": job["spec_digest"], "mode": resolution.mode})
                parked = self._request_approval(job["job_id"], reason="provider approval is required before dispatch")
                return RemoteBrainJobRun(status="waiting_approval", job=parked, mode=resolution.mode)
            if self.credential_scope is not None:
                _assert_scope_resolution_clean(resolution)
                opened = self.credential_scope.open({
                    "job_id": job["job_id"],
                    "attempt": job["attempts"],
                    "approval_released": True,
                })
                if inspect.isawaitable(opened):
                    raise RemoteBrainWorkerError("sync remote brain credential_scope returned an awaitable")
                credential_binding = _validate_credential_binding(opened)
                resolution = _bind_credential_scope_resolution(resolution, credential_binding)
            kwargs = self._approved_kwargs(resolution)
            self._checkpoint(job["job_id"], "dispatch_started", "unknown", {"spec_digest": job["spec_digest"], "mode": resolution.mode})
            started = True
            if heartbeat_error:
                raise RemoteBrainWorkerError("remote brain lease heartbeat failed after dispatch", code="transport")
            runner_name = self._RUNNERS[resolution.mode]
            result = getattr(self.brain, runner_name)(**kwargs)
            if heartbeat_error:
                raise RemoteBrainWorkerError("remote brain lease heartbeat failed after dispatch", code="transport")
            status = _result_status(result)
            result_digest = _result_digest(result, resolution.mode, job["job_id"])
            if status in _APPROVAL_STATUSES:
                self._checkpoint(job["job_id"], status, "unknown", {"result_digest": result_digest})
                parked = self._request_approval(job["job_id"], reason="brain execution requires caller approval before continuing")
                return RemoteBrainJobRun(status="waiting_approval", job=parked, mode=resolution.mode, result=result, result_digest=result_digest)
            if status == "reconciliation_required":
                self._checkpoint(job["job_id"], status, "unknown", {"result_digest": result_digest})
                failed = self._fail(job["job_id"], "remote brain execution requires caller reconciliation", retryable=False)
                return RemoteBrainJobRun(status="reconciliation_required", job=failed, mode=resolution.mode, result=result, result_digest=result_digest)
            if _is_success_status(status):
                completed = self._complete(job["job_id"], result_digest)
                return RemoteBrainJobRun(status="succeeded", job=completed, mode=resolution.mode, result=result, result_digest=result_digest)
            self._checkpoint(job["job_id"], f"terminal_{status}", "unknown", {"result_digest": result_digest})
            failed = self._fail(job["job_id"], f"remote brain execution ended with {status}", retryable=False)
            return RemoteBrainJobRun(status="reconciliation_required" if failed.get("state") == "reconciliation_required" else "failed", job=failed, mode=resolution.mode, result=result, result_digest=result_digest)
        except Exception as error:
            error_class, failure_code, error_retryable = _error_projection(error)
            try:
                retryable = bool(not started and self.retry_preflight_failures and error_retryable is True)
                self._checkpoint(job["job_id"], "worker_execution_error", "unknown" if started else "preflight", {"error_class": error_class, "failure_code": failure_code})
                failed = self._fail(job["job_id"], "remote brain execution outcome is uncertain; reconciliation required" if started else "remote brain execution failed before dispatch", retryable=retryable)
                status = "reconciliation_required" if failed.get("state") == "reconciliation_required" else "retry_scheduled" if failed.get("state") == "queued" else "failed"
                return RemoteBrainJobRun(status=status, job=failed, mode=None if resolution is None else resolution.mode, error_class=error_class, failure_code=failure_code, error_retryable=error_retryable, result_digest=None)
            except Exception as settlement_error:
                raise RemoteBrainWorkerError("remote brain worker failure could not be settled", code="configuration") from settlement_error
        finally:
            stop.set()
            thread.join(timeout=max(1.0, self.heartbeat_ms / 1000.0))
            if credential_binding is not None:
                credential_binding.close()

    def run(self, *, limit: int = 1) -> RemoteBrainJobBatch:
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_BATCH:
            raise RemoteBrainWorkerError(f"remote brain worker limit must be within [1, {MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_BATCH}]")
        runs: list[RemoteBrainJobRun] = []
        for _ in range(limit):
            result = self.run_once()
            if result is None:
                break
            runs.append(result)
            if result.status in {"waiting_approval", "retry_scheduled", "reconciliation_required"}:
                break
        succeeded = sum(run.status == "succeeded" for run in runs)
        waiting = sum(run.status == "waiting_approval" for run in runs)
        retryable = sum(run.status == "retry_scheduled" for run in runs)
        reconciliation = sum(run.status == "reconciliation_required" for run in runs)
        failed = sum(run.status == "failed" for run in runs)
        status = "empty" if not runs else "failed" if failed and not succeeded and not waiting and not retryable and not reconciliation else "partial" if failed or waiting or retryable or reconciliation else "completed"
        return RemoteBrainJobBatch(status, tuple(runs), len(runs), succeeded, waiting, retryable, reconciliation, failed)

    def _resolve(self, job: Mapping[str, Any], approval_released: bool) -> RemoteBrainJobResolution:
        context = {"job": dict(job), "approval_released": approval_released, "attempt": job["attempts"]}
        if self.resolver is not None:
            raw = self.resolver(context)
        else:
            assert self.protected_rehydration is not None
            raw = self.protected_rehydration.resolve(RemoteBrainProtectedRehydrationContext(
                job_id=job["job_id"],
                spec_digest=job["spec_digest"],
                domain=job["domain"],
                capability=job["capability"],
                attempt=job["attempts"],
                approval_released=approval_released,
            ))
        if isinstance(raw, RemoteBrainJobResolution):
            return raw
        if not isinstance(raw, Mapping):
            raise RemoteBrainWorkerError("remote brain resolver must return a mapping")
        allowed = {"spec_digest", "policy_digest", "plan_digest", "route_digest", "action_plan", "action_admission", "action_handoff", "mode", "request", "kwargs"}
        unknown = sorted(set(raw).difference(allowed))
        if unknown:
            raise RemoteBrainWorkerError("remote brain resolver returned unsupported fields")
        return RemoteBrainJobResolution(
            spec_digest=raw.get("spec_digest"),
            policy_digest=raw.get("policy_digest"),
            mode=raw.get("mode"),
            request=raw.get("request"),
            kwargs=raw.get("kwargs"),
            plan_digest=raw.get("plan_digest"),
            route_digest=raw.get("route_digest"),
            action_plan=raw.get("action_plan"),
            action_admission=raw.get("action_admission"),
            action_handoff=raw.get("action_handoff"),
        )

    @staticmethod
    def _validate_resolution(job: Mapping[str, Any], resolution: RemoteBrainJobResolution) -> None:
        mode = _validate_mode(resolution.mode)
        spec_digest = _validate_digest("resolver spec_digest", resolution.spec_digest)
        policy_digest = _validate_optional_digest("resolver policy_digest", resolution.policy_digest)
        plan_digest = _validate_optional_digest("resolver plan_digest", resolution.plan_digest)
        route_digest = _validate_optional_digest("resolver route_digest", resolution.route_digest)
        action_handoff = _action_handoff_value(resolution.action_handoff)
        handoff_plan = _action_plan_value(None if action_handoff is None else action_handoff["plan"])
        handoff_admission = _action_admission_value(None if action_handoff is None else action_handoff["admission"])
        explicit_action_plan = _action_plan_value(resolution.action_plan)
        explicit_action_admission = _action_admission_value(resolution.action_admission)
        if action_handoff is not None and explicit_action_plan is not None and explicit_action_plan.plan_digest != handoff_plan.plan_digest:
            raise RemoteBrainWorkerError("remote brain action plan does not match the verified handoff", code="protocol")
        if action_handoff is not None and explicit_action_admission is not None and explicit_action_admission.admission_digest != handoff_admission.admission_digest:
            raise RemoteBrainWorkerError("remote brain action admission does not match the verified handoff", code="protocol")
        action_plan = explicit_action_plan or handoff_plan
        action_admission = explicit_action_admission or handoff_admission
        if (action_plan is None) != (action_admission is None):
            raise RemoteBrainWorkerError("remote brain action_plan and action_admission must be supplied together", code="protocol")
        action_plan_digest = action_plan.plan_digest if action_plan is not None else None
        action_admission_digest = action_admission.admission_digest if action_admission is not None else None
        if action_plan is not None and action_admission is not None:
            if action_admission.plan_digest != action_plan.plan_digest:
                raise RemoteBrainWorkerError("remote brain action admission is bound to a different action plan", code="protocol")
            if action_admission.status != "admitted":
                raise RemoteBrainWorkerError("remote brain action admission must be admitted before worker dispatch", code="protocol")
        if action_handoff is not None:
            selected_domains = action_handoff["selected_domains"]
            request_domain = resolution.request.get("domain") if isinstance(resolution.request, Mapping) else None
            if job["domain"] == "cross_domain" and action_handoff["cross_domain"] is not True and "cross_domain" not in selected_domains:
                raise RemoteBrainWorkerError("cross-domain job requires a cross-domain action handoff", code="protocol")
            if job["domain"] != "cross_domain" and job["domain"] not in selected_domains:
                raise RemoteBrainWorkerError("action handoff does not cover the durable job domain", code="protocol")
            if isinstance(request_domain, str) and request_domain != "cross_domain" and request_domain not in selected_domains:
                raise RemoteBrainWorkerError("action handoff does not cover the request domain", code="protocol")
        if spec_digest != job["spec_digest"]:
            raise RemoteBrainWorkerError("remote brain resolver spec_digest does not match the durable job")
        if not isinstance(resolution.request, Mapping) or not isinstance(resolution.kwargs, Mapping):
            raise RemoteBrainWorkerError("remote brain resolver request and kwargs must be mappings")
        expected = autonomous_remote_brain_job_spec_digest(
            request=resolution.request,
            mode=mode,
            policy_digest=policy_digest,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=action_plan_digest,
            action_admission_digest=action_admission_digest,
            action_handoff_digest=None if action_handoff is None else action_handoff["handoff_digest"],
        )
        if expected != job["spec_digest"]:
            raise RemoteBrainWorkerError("remote brain request, mode, policy, reviewed identities, and action handoff do not match the durable job")
        task = resolution.request.get("task")
        if not isinstance(task, str) or not task.strip():
            raise RemoteBrainWorkerError("remote brain resolver request must contain a bounded task")
        if "task" in resolution.kwargs and resolution.kwargs["task"] != task:
            raise RemoteBrainWorkerError("remote brain execution kwargs task does not match the durable request")
        blueprint = resolution.kwargs.get("blueprint")
        blueprint_spec = _mapping_value(blueprint, "spec")
        blueprint_task = _mapping_value(blueprint_spec, "task")
        if blueprint_task is not None and blueprint_task != task:
            raise RemoteBrainWorkerError("remote brain workflow blueprint does not match the durable request")
        if plan_digest is not None and blueprint is not None:
            if autonomous_remote_brain_plan_digest(blueprint) != plan_digest:
                raise RemoteBrainWorkerError("remote brain workflow blueprint does not match the durable plan digest")
        route = resolution.kwargs.get("route")
        if route_digest is not None and route is not None:
            if autonomous_remote_brain_route_digest(route) != route_digest:
                raise RemoteBrainWorkerError("remote brain route does not match the durable route digest")
        request_domain = resolution.request.get("domain")
        if request_domain is not None and request_domain != job["domain"] and job["domain"] != "cross_domain":
            raise RemoteBrainWorkerError("remote brain request domain does not match the durable job")

    @staticmethod
    def _approved_kwargs(resolution: RemoteBrainJobResolution) -> dict[str, Any]:
        kwargs = dict(resolution.kwargs)
        kwargs["approve_provider_call"] = True
        if "approve_mission_dispatch" in kwargs:
            kwargs["approve_mission_dispatch"] = True
        return kwargs

    def _approval_released(self, job_id: str) -> bool:
        after = 0
        for _ in range(MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_EVENT_PAGES):
            page = self.control.job_events({"job_id": job_id, "after": after, "limit": 256})
            _assert_no_private_fields(page)
            events = page.get("events", [])
            if not isinstance(events, Sequence) or isinstance(events, (str, bytes)):
                raise RemoteBrainWorkerError("remote brain event projection is malformed", code="protocol")
            if any(isinstance(event, Mapping) and event.get("event_type") in {"job_approval_granted", "job_approval_released"} for event in events):
                return True
            next_after = page.get("next_after", after)
            if not isinstance(next_after, int) or next_after <= after or not events:
                return False
            after = next_after
        return False

    def _checkpoint(self, job_id: str, phase: str, boundary: str, metadata: Mapping[str, Any], *, waiting_for_approval: bool = False) -> dict[str, Any]:
        digest = _digest_json({"schema": AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA, "job_id": job_id, "phase": phase, "metadata": dict(metadata)})
        payload = self.control.checkpoint_job({"job_id": job_id, "worker_id": self.worker_id, "phase": _validate_identifier("checkpoint phase", phase, 128), "checkpoint_digest": digest, "side_effect_boundary": boundary, "waiting_for_approval": waiting_for_approval})
        return _job_projection(payload)

    def _request_approval(self, job_id: str, *, reason: str) -> dict[str, Any]:
        payload = self.control.approval({
            "job_id": _validate_identifier("remote brain job_id", job_id),
            "action": "request",
            "reason": _validate_identifier("approval reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
        })
        return _job_projection(payload)

    def _complete(self, job_id: str, result_digest: str) -> dict[str, Any]:
        return _job_projection(self.control.complete_job({"job_id": job_id, "worker_id": self.worker_id, "result_digest": _validate_digest("result_digest", result_digest)}))

    def _fail(self, job_id: str, reason: str, *, retryable: bool) -> dict[str, Any]:
        return _job_projection(self.control.fail_job({"job_id": job_id, "worker_id": self.worker_id, "reason": _validate_identifier("failure reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES), "retryable": retryable}))


class AsyncRemoteBrainJobWorker:
    """Async counterpart to :class:`RemoteBrainJobWorker`.

    The control-plane lifecycle stays non-blocking for async HTTP/MCP hosts.  A synchronous
    ``AutonomousBrain`` facade is invoked in a worker thread so provider work cannot block the
    event loop; a native async runner is also accepted when a deployment supplies one.
    """

    _RUNNERS = RemoteBrainJobWorker._RUNNERS

    def __init__(
        self,
        brain: Any,
        control: AsyncBrainControlClient,
        *,
        worker_id: str,
        resolver: AsyncRemoteBrainJobResolver | None = None,
        protected_rehydration: RemoteBrainProtectedRehydration | None = None,
        lease_ms: int = 300_000,
        heartbeat_ms: int | None = None,
        retry_preflight_failures: bool = True,
        credential_scope: RemoteBrainCredentialScope | None = None,
    ) -> None:
        if brain is None:
            raise RemoteBrainWorkerError("async remote brain worker requires a brain facade")
        if not all(callable(getattr(brain, name, None)) for name in set(self._RUNNERS.values())):
            raise RemoteBrainWorkerError("async remote brain facade does not expose every supported execution mode")
        if not isinstance(control, AsyncBrainControlClient):
            raise RemoteBrainWorkerError("async remote brain worker requires an AsyncBrainControlClient")
        if resolver is not None and not callable(resolver):
            raise RemoteBrainWorkerError("async remote brain resolver must be callable")
        if protected_rehydration is not None and not isinstance(protected_rehydration, RemoteBrainProtectedRehydration):
            raise RemoteBrainWorkerError("async remote brain protected_rehydration is malformed")
        if resolver is None and protected_rehydration is None:
            raise RemoteBrainWorkerError("async remote brain worker requires resolver or protected_rehydration")
        self.brain = brain
        self.control = control
        self.resolver = resolver
        self.protected_rehydration = protected_rehydration
        self.worker_id = _validate_identifier("async remote brain worker_id", worker_id)
        if not isinstance(lease_ms, int) or isinstance(lease_ms, bool) or not 100 <= lease_ms <= MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_LEASE_MS:
            raise RemoteBrainWorkerError("async remote brain lease_ms is outside its bounds")
        self.lease_ms = lease_ms
        effective_heartbeat = min(30_000, max(1, lease_ms // 3)) if heartbeat_ms is None else heartbeat_ms
        if not isinstance(effective_heartbeat, int) or isinstance(effective_heartbeat, bool) or not 1 <= effective_heartbeat <= MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_HEARTBEAT_MS or effective_heartbeat >= lease_ms:
            raise RemoteBrainWorkerError("async remote brain heartbeat_ms must be bounded and less than lease_ms")
        self.heartbeat_ms = effective_heartbeat
        if not isinstance(retry_preflight_failures, bool):
            raise RemoteBrainWorkerError("async remote brain retry_preflight_failures must be boolean")
        if credential_scope is not None and not callable(getattr(credential_scope, "open", None)):
            raise RemoteBrainWorkerError("async remote brain credential_scope must expose open(context)")
        self.retry_preflight_failures = retry_preflight_failures
        self.credential_scope = credential_scope

    async def submit(
        self,
        *,
        idempotency_key: str,
        request: Mapping[str, Any],
        mode: str,
        domain: str,
        capability: str,
        risk_class: str,
        policy_digest: str | None = None,
        plan_digest: str | None = None,
        route_digest: str | None = None,
        action_plan_digest: str | None = None,
        action_admission_digest: str | None = None,
        action_handoff_digest: str | None = None,
        priority: int = 0,
        max_attempts: int = 3,
        checkpoint_digest: str | None = None,
    ) -> RemoteBrainJobSubmission:
        idempotency_key = _validate_identifier("async remote brain idempotency_key", idempotency_key)
        mode = _validate_mode(mode)
        domain = _validate_identifier("async remote brain domain", domain)
        capability = _validate_identifier("async remote brain capability", capability)
        risk_class = _validate_identifier("async remote brain risk_class", risk_class)
        if not isinstance(priority, int) or isinstance(priority, bool) or not 0 <= priority <= 255:
            raise RemoteBrainWorkerError("async remote brain priority is outside its bounds")
        if not isinstance(max_attempts, int) or isinstance(max_attempts, bool) or not 1 <= max_attempts <= 8:
            raise RemoteBrainWorkerError("async remote brain max_attempts is outside its bounds")
        policy_digest = _validate_optional_digest("policy_digest", policy_digest)
        plan_digest = _validate_optional_digest("plan_digest", plan_digest)
        route_digest = _validate_optional_digest("route_digest", route_digest)
        action_plan_digest = _validate_optional_digest("action_plan_digest", action_plan_digest)
        action_admission_digest = _validate_optional_digest("action_admission_digest", action_admission_digest)
        action_handoff_digest = _validate_optional_digest("action_handoff_digest", action_handoff_digest)
        checkpoint_digest = _validate_optional_digest("checkpoint_digest", checkpoint_digest)
        spec_digest = autonomous_remote_brain_job_spec_digest(
            request=request,
            mode=mode,
            policy_digest=policy_digest,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=action_plan_digest,
            action_admission_digest=action_admission_digest,
            action_handoff_digest=action_handoff_digest,
        )
        payload = await self.control.submit_job({
            "idempotency_key": idempotency_key,
            "spec_digest": spec_digest,
            "domain": domain,
            "capability": capability,
            "risk_class": risk_class,
            "priority": priority,
            "max_attempts": max_attempts,
            **({"checkpoint_digest": checkpoint_digest} if checkpoint_digest is not None else {}),
        })
        return RemoteBrainJobSubmission(
            status="submitted",
            job=_job_projection(payload),
            spec_digest=spec_digest,
            mode=mode,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=action_plan_digest,
            action_admission_digest=action_admission_digest,
            action_handoff_digest=action_handoff_digest,
        )

    async def submit_handoff(
        self,
        *,
        idempotency_key: str,
        request: Mapping[str, Any],
        mode: str,
        domain: str,
        capability: str,
        risk_class: str,
        action_handoff: Mapping[str, Any],
        policy_digest: str | None = None,
        plan_digest: str | None = None,
        route_digest: str | None = None,
        priority: int = 0,
        max_attempts: int = 3,
        checkpoint_digest: str | None = None,
    ) -> RemoteBrainJobSubmission:
        """Submit a job whose action identity is derived from one verified handoff."""

        handoff = _action_handoff_value(action_handoff)
        if handoff is None:
            raise RemoteBrainWorkerError("action_handoff must be a metadata mapping", code="protocol")
        if plan_digest is not None and plan_digest != handoff["plan_digest"]:
            raise RemoteBrainWorkerError("plan_digest does not match the verified action handoff", code="protocol")
        return await self.submit(
            idempotency_key=idempotency_key,
            request=request,
            mode=mode,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            policy_digest=policy_digest,
            plan_digest=plan_digest,
            route_digest=route_digest,
            action_plan_digest=handoff["plan_digest"],
            action_admission_digest=handoff["admission_digest"],
            action_handoff_digest=handoff["handoff_digest"],
            priority=priority,
            max_attempts=max_attempts,
            checkpoint_digest=checkpoint_digest,
        )

    async def status(self, job_id: str) -> Mapping[str, Any]:
        return _job_projection(await self.control.job_status(_validate_identifier("async remote brain job_id", job_id)))

    async def approval(
        self,
        job_id: str,
        action: str,
        *,
        reason: str | None = None,
        authorization_digest: str | None = None,
    ) -> Mapping[str, Any]:
        if action not in {"request", "approve", "deny"}:
            raise RemoteBrainWorkerError("async remote brain approval action is invalid")
        if action in {"approve", "deny"}:
            _validate_digest("authorization_digest", authorization_digest)
        if reason is not None:
            _validate_identifier("approval reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES)
        payload: dict[str, Any] = {"job_id": _validate_identifier("async remote brain job_id", job_id), "action": action}
        if reason is not None:
            payload["reason"] = reason
        if authorization_digest is not None:
            payload["authorization_digest"] = authorization_digest
        response = await self.control.approval(payload)
        _assert_no_private_fields(response)
        return dict(response)

    async def reconcile(
        self,
        job_id: str,
        *,
        outcome: str,
        evidence_digest: str,
        evidence_kind: str = "caller_observation",
        operator: str = "caller",
        reason: str = "caller reconciled uncertain external state",
        effect_absent: bool = False,
    ) -> Mapping[str, Any]:
        if outcome not in {"succeeded", "failed", "not_executed", "unknown"}:
            raise RemoteBrainWorkerError("async remote brain reconciliation outcome is invalid")
        if outcome == "not_executed" and effect_absent is not True:
            raise RemoteBrainWorkerError("not_executed reconciliation requires effect_absent=True")
        payload = await self.control.reconcile_job({
            "job_id": _validate_identifier("async remote brain job_id", job_id),
            "outcome": outcome,
            "evidence_digest": _validate_digest("evidence_digest", evidence_digest),
            "evidence_kind": _validate_identifier("evidence_kind", evidence_kind, 128),
            "operator": _validate_identifier("operator", operator),
            "reason": _validate_identifier("reconciliation reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
            "effect_absent": effect_absent,
        })
        _assert_no_private_fields(payload)
        return dict(payload)

    async def cancel(self, job_id: str, *, reason: str = "cancelled by caller") -> Mapping[str, Any]:
        payload = await self.control.cancel_job({
            "job_id": _validate_identifier("async remote brain job_id", job_id),
            "reason": _validate_identifier("cancellation reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
        })
        _assert_no_private_fields(payload)
        return dict(payload)

    async def run_once(self, job_id: str | None = None) -> RemoteBrainJobRun | None:
        if job_id is None:
            claimed_payload = await self.control.claim_next_job({"worker_id": self.worker_id, "lease_ms": self.lease_ms})
            _assert_no_private_fields(claimed_payload)
            if claimed_payload.get("claimed") is not True or claimed_payload.get("job") is None:
                return None
        else:
            claimed_payload = await self.control.claim_job({
                "job_id": _validate_identifier("async remote brain job_id", job_id),
                "worker_id": self.worker_id,
                "lease_ms": self.lease_ms,
            })
            _assert_no_private_fields(claimed_payload)
        job = _job_projection(claimed_payload)
        if job.get("lease_owner") != self.worker_id or job.get("state") not in {"leased", "running"}:
            raise RemoteBrainWorkerError("async remote brain control plane returned a job without this worker lease", code="protocol")

        stop = asyncio.Event()
        heartbeat_error: list[BaseException] = []

        async def heartbeat() -> None:
            while not stop.is_set():
                try:
                    await asyncio.wait_for(stop.wait(), timeout=self.heartbeat_ms / 1000.0)
                    continue
                except asyncio.TimeoutError:
                    pass
                if stop.is_set():
                    return
                try:
                    renewed = await self.control.renew_job({"job_id": job["job_id"], "worker_id": self.worker_id, "lease_ms": self.lease_ms})
                    _job_projection(renewed)
                except BaseException as error:  # pragma: no cover - timing dependent
                    heartbeat_error.append(error)
                    stop.set()
                    return

        heartbeat_task = asyncio.create_task(heartbeat(), name=f"aurora-async-remote-brain-heartbeat-{self.worker_id}")
        started = False
        resolution: RemoteBrainJobResolution | None = None
        credential_binding: RemoteBrainCredentialBinding | None = None
        try:
            approval_released = await self._approval_released(job["job_id"])
            await self._checkpoint(
                job["job_id"],
                "resolving_private_spec",
                job["side_effect_boundary"],
                {"job_id": job["job_id"], "spec_digest": job["spec_digest"], "attempt": job["attempts"]},
            )
            resolution = await self._resolve(job, approval_released)
            self._validate_resolution(job, resolution)
            if heartbeat_error:
                raise RemoteBrainWorkerError("async remote brain lease heartbeat failed before dispatch", code="transport", retryable=True)
            if not approval_released:
                await self._checkpoint(
                    job["job_id"],
                    "provider_approval_required",
                    "preflight",
                    {"spec_digest": job["spec_digest"], "mode": resolution.mode},
                )
                parked = await self._request_approval(job["job_id"], reason="provider approval is required before dispatch")
                return RemoteBrainJobRun(status="waiting_approval", job=parked, mode=resolution.mode)
            if self.credential_scope is not None:
                _assert_scope_resolution_clean(resolution)
                credential_binding = await _open_async_credential_scope(
                    self.credential_scope,
                    {
                        "job_id": job["job_id"],
                        "attempt": job["attempts"],
                        "approval_released": True,
                    },
                )
                resolution = _bind_credential_scope_resolution(resolution, credential_binding)
            kwargs = RemoteBrainJobWorker._approved_kwargs(resolution)
            await self._checkpoint(job["job_id"], "dispatch_started", "unknown", {"spec_digest": job["spec_digest"], "mode": resolution.mode})
            started = True
            if heartbeat_error:
                raise RemoteBrainWorkerError("async remote brain lease heartbeat failed after dispatch", code="transport")
            result = await self._invoke_runner(self._RUNNERS[resolution.mode], kwargs)
            if heartbeat_error:
                raise RemoteBrainWorkerError("async remote brain lease heartbeat failed after dispatch", code="transport")
            status = _result_status(result)
            result_digest = _result_digest(result, resolution.mode, job["job_id"])
            if status in _APPROVAL_STATUSES:
                await self._checkpoint(job["job_id"], status, "unknown", {"result_digest": result_digest})
                parked = await self._request_approval(job["job_id"], reason="brain execution requires caller approval before continuing")
                return RemoteBrainJobRun(status="waiting_approval", job=parked, mode=resolution.mode, result=result, result_digest=result_digest)
            if status == "reconciliation_required":
                await self._checkpoint(job["job_id"], status, "unknown", {"result_digest": result_digest})
                failed = await self._fail(job["job_id"], "remote brain execution requires caller reconciliation", retryable=False)
                return RemoteBrainJobRun(status="reconciliation_required", job=failed, mode=resolution.mode, result=result, result_digest=result_digest)
            if _is_success_status(status):
                completed = await self._complete(job["job_id"], result_digest)
                return RemoteBrainJobRun(status="succeeded", job=completed, mode=resolution.mode, result=result, result_digest=result_digest)
            await self._checkpoint(job["job_id"], f"terminal_{status}", "unknown", {"result_digest": result_digest})
            failed = await self._fail(job["job_id"], f"remote brain execution ended with {status}", retryable=False)
            return RemoteBrainJobRun(status="reconciliation_required" if failed.get("state") == "reconciliation_required" else "failed", job=failed, mode=resolution.mode, result=result, result_digest=result_digest)
        except asyncio.CancelledError as error:
            # Cancellation is not evidence that a provider call did not start.  Persist the
            # same conservative boundary before propagating cancellation to the host.
            try:
                await self._settle_error(job, started, resolution, error)
            finally:
                raise
        except Exception as error:
            return await self._settle_error(job, started, resolution, error)
        finally:
            stop.set()
            heartbeat_task.cancel()
            await asyncio.gather(heartbeat_task, return_exceptions=True)
            if credential_binding is not None:
                await _close_async_credential_binding(credential_binding)

    async def run(self, *, limit: int = 1) -> RemoteBrainJobBatch:
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_BATCH:
            raise RemoteBrainWorkerError(f"async remote brain worker limit must be within [1, {MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_BATCH}]")
        runs: list[RemoteBrainJobRun] = []
        for _ in range(limit):
            result = await self.run_once()
            if result is None:
                break
            runs.append(result)
            if result.status in {"waiting_approval", "retry_scheduled", "reconciliation_required"}:
                break
        succeeded = sum(run.status == "succeeded" for run in runs)
        waiting = sum(run.status == "waiting_approval" for run in runs)
        retryable = sum(run.status == "retry_scheduled" for run in runs)
        reconciliation = sum(run.status == "reconciliation_required" for run in runs)
        failed = sum(run.status == "failed" for run in runs)
        status = "empty" if not runs else "failed" if failed and not succeeded and not waiting and not retryable and not reconciliation else "partial" if failed or waiting or retryable or reconciliation else "completed"
        return RemoteBrainJobBatch(status, tuple(runs), len(runs), succeeded, waiting, retryable, reconciliation, failed)

    async def _resolve(self, job: Mapping[str, Any], approval_released: bool) -> RemoteBrainJobResolution:
        context = {"job": dict(job), "approval_released": approval_released, "attempt": job["attempts"]}
        if self.resolver is not None and inspect.iscoroutinefunction(self.resolver):
            raw = await self.resolver(context)
        elif self.resolver is not None:
            raw = await asyncio.to_thread(self.resolver, context)
        else:
            assert self.protected_rehydration is not None
            raw = await self.protected_rehydration.resolve_async(RemoteBrainProtectedRehydrationContext(
                job_id=job["job_id"],
                spec_digest=job["spec_digest"],
                domain=job["domain"],
                capability=job["capability"],
                attempt=job["attempts"],
                approval_released=approval_released,
            ))
        if inspect.isawaitable(raw):
            raw = await raw
        if isinstance(raw, RemoteBrainJobResolution):
            return raw
        if not isinstance(raw, Mapping):
            raise RemoteBrainWorkerError("async remote brain resolver must return a mapping")
        allowed = {"spec_digest", "policy_digest", "plan_digest", "route_digest", "action_plan", "action_admission", "action_handoff", "mode", "request", "kwargs"}
        unknown = sorted(set(raw).difference(allowed))
        if unknown:
            raise RemoteBrainWorkerError("async remote brain resolver returned unsupported fields")
        return RemoteBrainJobResolution(
            spec_digest=raw.get("spec_digest"),
            policy_digest=raw.get("policy_digest"),
            mode=raw.get("mode"),
            request=raw.get("request"),
            kwargs=raw.get("kwargs"),
            plan_digest=raw.get("plan_digest"),
            route_digest=raw.get("route_digest"),
            action_plan=raw.get("action_plan"),
            action_admission=raw.get("action_admission"),
            action_handoff=raw.get("action_handoff"),
        )

    @staticmethod
    def _validate_resolution(job: Mapping[str, Any], resolution: RemoteBrainJobResolution) -> None:
        RemoteBrainJobWorker._validate_resolution(job, resolution)

    async def _invoke_runner(self, runner_name: str, kwargs: Mapping[str, Any]) -> Any:
        runner = getattr(self.brain, runner_name)
        if inspect.iscoroutinefunction(runner):
            result = await runner(**dict(kwargs))
        else:
            result = await asyncio.to_thread(runner, **dict(kwargs))
        if inspect.isawaitable(result):
            return await result
        return result

    async def _approval_released(self, job_id: str) -> bool:
        after = 0
        for _ in range(MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_EVENT_PAGES):
            page = await self.control.job_events({"job_id": job_id, "after": after, "limit": 256})
            _assert_no_private_fields(page)
            events = page.get("events", [])
            if not isinstance(events, Sequence) or isinstance(events, (str, bytes)):
                raise RemoteBrainWorkerError("async remote brain event projection is malformed", code="protocol")
            if any(isinstance(event, Mapping) and event.get("event_type") in {"job_approval_granted", "job_approval_released"} for event in events):
                return True
            next_after = page.get("next_after", after)
            if not isinstance(next_after, int) or next_after <= after or not events:
                return False
            after = next_after
        return False

    async def _checkpoint(self, job_id: str, phase: str, boundary: str, metadata: Mapping[str, Any]) -> dict[str, Any]:
        digest = _digest_json({"schema": AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA, "job_id": job_id, "phase": phase, "metadata": dict(metadata)})
        payload = await self.control.checkpoint_job({
            "job_id": job_id,
            "worker_id": self.worker_id,
            "phase": _validate_identifier("checkpoint phase", phase, 128),
            "checkpoint_digest": digest,
            "side_effect_boundary": boundary,
            "waiting_for_approval": False,
        })
        return _job_projection(payload)

    async def _request_approval(self, job_id: str, *, reason: str) -> dict[str, Any]:
        payload = await self.control.approval({
            "job_id": _validate_identifier("async remote brain job_id", job_id),
            "action": "request",
            "reason": _validate_identifier("approval reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
        })
        return _job_projection(payload)

    async def _complete(self, job_id: str, result_digest: str) -> dict[str, Any]:
        return _job_projection(await self.control.complete_job({
            "job_id": job_id,
            "worker_id": self.worker_id,
            "result_digest": _validate_digest("result_digest", result_digest),
        }))

    async def _fail(self, job_id: str, reason: str, *, retryable: bool) -> dict[str, Any]:
        return _job_projection(await self.control.fail_job({
            "job_id": job_id,
            "worker_id": self.worker_id,
            "reason": _validate_identifier("failure reason", reason, MAX_AUTONOMOUS_REMOTE_BRAIN_REASON_BYTES),
            "retryable": retryable,
        }))

    async def _settle_error(
        self,
        job: Mapping[str, Any],
        started: bool,
        resolution: RemoteBrainJobResolution | None,
        error: BaseException,
    ) -> RemoteBrainJobRun:
        error_class, failure_code, error_retryable = _error_projection(error)
        try:
            boundary = "unknown" if started else job["side_effect_boundary"]
            await self._checkpoint(job["job_id"], "worker_execution_error", boundary, {"error_class": error_class, "failure_code": failure_code})
            retryable = bool(not started and self.retry_preflight_failures and error_retryable is True)
            failed = await self._fail(
                job["job_id"],
                "remote brain execution outcome is uncertain; reconciliation required" if started else "remote brain execution failed before dispatch",
                retryable=retryable,
            )
            status = "reconciliation_required" if failed.get("state") == "reconciliation_required" else "retry_scheduled" if failed.get("state") == "queued" else "failed"
            return RemoteBrainJobRun(
                status=status,
                job=failed,
                mode=None if resolution is None else resolution.mode,
                error_class=error_class,
                failure_code=failure_code,
                error_retryable=error_retryable,
                result_digest=None,
            )
        except Exception as settlement_error:
            raise RemoteBrainWorkerError("async remote brain worker failure could not be settled", code="configuration") from settlement_error


def _is_success_status(status: str) -> bool:
    return status == "succeeded" or status == "completed" or status.startswith("completed_")


__all__ = [
    "AUTONOMOUS_REMOTE_BRAIN_WORKER_SCHEMA",
    "AUTONOMOUS_REMOTE_BRAIN_JOB_SPEC_SCHEMA",
    "AUTONOMOUS_REMOTE_BRAIN_PLAN_SCHEMA",
    "AUTONOMOUS_REMOTE_BRAIN_ROUTE_SCHEMA",
    "MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_LEASE_MS",
    "MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_HEARTBEAT_MS",
    "MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_BATCH",
    "MAX_AUTONOMOUS_REMOTE_BRAIN_WORKER_EVENT_PAGES",
    "REMOTE_BRAIN_MODES",
    "RemoteBrainWorkerError",
    "RemoteBrainJobSubmission",
    "RemoteBrainJobRun",
    "RemoteBrainJobBatch",
    "RemoteBrainJobResolution",
    "RemoteBrainProtectedRehydrationContext",
    "RemoteBrainProtectedReceiptResolver",
    "RemoteBrainProtectedRehydration",
    "RemoteBrainCredentialBinding",
    "RemoteBrainCredentialScope",
    "ProvisionedRemoteBrainCredentialScope",
    "RemoteBrainJobResolver",
    "AsyncRemoteBrainJobResolver",
    "autonomous_remote_brain_job_spec_digest",
    "autonomous_remote_brain_plan_digest",
    "autonomous_remote_brain_route_digest",
    "RemoteBrainJobWorker",
    "AsyncRemoteBrainJobWorker",
]
