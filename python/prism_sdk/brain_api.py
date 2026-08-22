"""Typed clients for the transport-facing autonomous-brain control plane.

The control-plane client is intentionally narrower than prism_sdk.llm_runtime. Provider keys are
collected by ProviderOnboarding and remain behind an in-memory CredentialHandle; this module
accepts only value-free job admission, leases, checkpoints, settlement, reconciliation, health,
approval, and replay metadata.

BrainControlClient.from_http and BrainControlClient.from_mcp use the existing transports, so
applications do not need a second network protocol or a secret-bearing API. The asynchronous
facade mirrors the same contract for ApiClient and AsyncClient.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import re
from typing import Any, Awaitable, Callable, Mapping, Sequence


CONTROL_SCHEMA = "bioprism-brain-control-plane/0.1"
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER = re.compile(r"^[A-Za-z][A-Za-z0-9_.-]{0,127}$")
_MAX_TEXT_BYTES = 256
_MAX_REASON_BYTES = 2_048
_MAX_SIGNALS = 64


class BrainControlError(ValueError):
    """A typed control-plane request or response was refused."""


class BrainControlRefusal(BrainControlError):
    """The remote brain control-plane tool returned an explicit refusal."""

    def __init__(self, tool: str, payload: Mapping[str, Any]) -> None:
        self.tool = tool
        self.payload = dict(payload)
        super().__init__(f"{tool} refused the request: {payload.get('error', 'unspecified refusal')}")


def _text(name: str, value: Any, maximum: int = _MAX_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainControlError(f"{name} must be a non-empty NUL-free string")
    if len(value.encode("utf-8")) > maximum:
        raise BrainControlError(f"{name} exceeds its {maximum}-byte bound")
    return value


def _digest(name: str, value: Any) -> str:
    value = _text(name, value, 64)
    if not _DIGEST.fullmatch(value):
        raise BrainControlError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _canonical_digest(value: Any) -> str:
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainControlError("control-plane value must be JSON-safe") from error
    return hashlib.sha256(encoded).hexdigest()


def _bounded_uint(name: str, value: Any, minimum: int, maximum: int, default: int) -> int:
    if value is None:
        value = default
    if not isinstance(value, int) or isinstance(value, bool) or not minimum <= value <= maximum:
        raise BrainControlError(f"{name} must be an integer within [{minimum}, {maximum}]")
    return value


def _signal_map(signals: Mapping[str, Any]) -> dict[str, float]:
    if not isinstance(signals, Mapping) or not signals or len(signals) > _MAX_SIGNALS:
        raise BrainControlError(f"signals must contain 1..{_MAX_SIGNALS} entries")
    normalized: dict[str, float] = {}
    for name, value in signals.items():
        if not isinstance(name, str) or not _IDENTIFIER.fullmatch(name):
            raise BrainControlError("signal names must be safe bounded identifiers")
        if isinstance(value, bool):
            number = 1.0 if value else 0.0
        elif isinstance(value, (int, float)) and not isinstance(value, bool):
            number = float(value)
        else:
            raise BrainControlError("signal values must be booleans or numbers")
        if not math.isfinite(number) or not 0.0 <= number <= 1.0:
            raise BrainControlError("signal values must be finite and within [0, 1]")
        normalized[name] = number
    return normalized


@dataclass(frozen=True, slots=True)
class BrainJobSubmission:
    """A rehydratable job identity; no task, prompt, provider response, or key is retained."""

    idempotency_key: str
    spec_digest: str
    domain: str
    capability: str
    risk_class: str
    job_id: str | None = None
    priority: int = 0
    max_attempts: int = 3
    checkpoint_digest: str | None = None

    def __post_init__(self) -> None:
        _text("idempotency_key", self.idempotency_key)
        _digest("spec_digest", self.spec_digest)
        _text("domain", self.domain)
        _text("capability", self.capability)
        _text("risk_class", self.risk_class)
        if self.job_id is not None:
            _text("job_id", self.job_id)
        _bounded_uint("priority", self.priority, 0, 255, 0)
        _bounded_uint("max_attempts", self.max_attempts, 1, 8, 3)
        if self.checkpoint_digest is not None:
            _digest("checkpoint_digest", self.checkpoint_digest)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "idempotency_key": self.idempotency_key,
            "spec_digest": self.spec_digest,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "priority": self.priority,
            "max_attempts": self.max_attempts,
        }
        if self.job_id is not None:
            result["job_id"] = self.job_id
        if self.checkpoint_digest is not None:
            result["checkpoint_digest"] = self.checkpoint_digest
        return result


@dataclass(frozen=True, slots=True)
class BrainApprovalCommand:
    """A caller-authenticated approval decision represented by an external proof digest."""

    job_id: str
    action: str
    reason: str | None = None
    authorization_digest: str | None = None

    def __post_init__(self) -> None:
        _text("job_id", self.job_id)
        if self.action not in {"request", "approve", "deny"}:
            raise BrainControlError("action must be request, approve, or deny")
        if self.reason is not None:
            _text("reason", self.reason, _MAX_REASON_BYTES)
        if self.action in {"approve", "deny"}:
            _digest("authorization_digest", self.authorization_digest)
        elif self.authorization_digest is not None:
            _digest("authorization_digest", self.authorization_digest)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"job_id": self.job_id, "action": self.action}
        if self.reason is not None:
            result["reason"] = self.reason
        if self.authorization_digest is not None:
            result["authorization_digest"] = self.authorization_digest
        return result


@dataclass(frozen=True, slots=True)
class BrainJobClaimCommand:
    """Claim a metadata-only job for a bounded worker lease."""

    job_id: str
    worker_id: str
    lease_ms: int = 60_000

    def __post_init__(self) -> None:
        _text("job_id", self.job_id)
        _text("worker_id", self.worker_id)
        _bounded_uint("lease_ms", self.lease_ms, 100, 86_400_000, 60_000)

    def to_arguments(self) -> dict[str, Any]:
        return {"job_id": self.job_id, "worker_id": self.worker_id, "lease_ms": self.lease_ms}


@dataclass(frozen=True, slots=True)
class BrainJobRenewCommand(BrainJobClaimCommand):
    """Renew an active metadata-only job lease for its current worker."""


@dataclass(frozen=True, slots=True)
class BrainJobCheckpointCommand:
    """Persist a phase/checkpoint digest without sending checkpoint payloads."""

    job_id: str
    worker_id: str
    phase: str
    checkpoint_digest: str
    side_effect_boundary: str = "not_started"
    waiting_for_approval: bool = False

    def __post_init__(self) -> None:
        _text("job_id", self.job_id)
        _text("worker_id", self.worker_id)
        _text("phase", self.phase, 128)
        _digest("checkpoint_digest", self.checkpoint_digest)
        if self.side_effect_boundary not in {"not_started", "preflight", "dispatched", "unknown"}:
            raise BrainControlError("side_effect_boundary must be not_started, preflight, dispatched, or unknown")
        if not isinstance(self.waiting_for_approval, bool):
            raise BrainControlError("waiting_for_approval must be a boolean")

    def to_arguments(self) -> dict[str, Any]:
        return {
            "job_id": self.job_id,
            "worker_id": self.worker_id,
            "phase": self.phase,
            "checkpoint_digest": self.checkpoint_digest,
            "side_effect_boundary": self.side_effect_boundary,
            "waiting_for_approval": self.waiting_for_approval,
        }


@dataclass(frozen=True, slots=True)
class BrainJobCompleteCommand:
    """Complete an owned job with a digest for caller-owned result metadata."""

    job_id: str
    worker_id: str
    result_digest: str

    def __post_init__(self) -> None:
        _text("job_id", self.job_id)
        _text("worker_id", self.worker_id)
        _digest("result_digest", self.result_digest)

    def to_arguments(self) -> dict[str, Any]:
        return {"job_id": self.job_id, "worker_id": self.worker_id, "result_digest": self.result_digest}


@dataclass(frozen=True, slots=True)
class BrainJobFailCommand:
    """Record a bounded failure; the server decides retry versus reconciliation."""

    job_id: str
    worker_id: str
    reason: str
    retryable: bool = False

    def __post_init__(self) -> None:
        _text("job_id", self.job_id)
        _text("worker_id", self.worker_id)
        _text("reason", self.reason, _MAX_REASON_BYTES)
        if not isinstance(self.retryable, bool):
            raise BrainControlError("retryable must be a boolean")

    def to_arguments(self) -> dict[str, Any]:
        return {"job_id": self.job_id, "worker_id": self.worker_id, "reason": self.reason, "retryable": self.retryable}


@dataclass(frozen=True, slots=True)
class BrainJobReconcileCommand:
    """Resolve an uncertain external effect using evidence and operator metadata digests."""

    job_id: str
    outcome: str
    evidence_digest: str
    evidence_kind: str = "caller_observation"
    operator: str = "caller"
    reason: str = "caller reconciled uncertain external state"
    effect_absent: bool = False

    def __post_init__(self) -> None:
        _text("job_id", self.job_id)
        if self.outcome not in {"succeeded", "failed", "not_executed", "unknown"}:
            raise BrainControlError("outcome must be succeeded, failed, not_executed, or unknown")
        _digest("evidence_digest", self.evidence_digest)
        _text("evidence_kind", self.evidence_kind, 128)
        _text("operator", self.operator)
        _text("reason", self.reason, _MAX_REASON_BYTES)
        if not isinstance(self.effect_absent, bool):
            raise BrainControlError("effect_absent must be a boolean")
        if self.outcome == "not_executed" and not self.effect_absent:
            raise BrainControlError("not_executed reconciliation requires effect_absent=True")

    def to_arguments(self) -> dict[str, Any]:
        return {
            "job_id": self.job_id,
            "outcome": self.outcome,
            "evidence_digest": self.evidence_digest,
            "evidence_kind": self.evidence_kind,
            "operator": self.operator,
            "reason": self.reason,
            "effect_absent": self.effect_absent,
        }


@dataclass(frozen=True, slots=True)
class BrainHealthObservation:
    """One metadata-only provider/model observation for adaptive selection."""

    provider: str
    model: str
    status: str
    latency_ms: int = 0
    quality: float | None = None
    tokens: int = 0
    registered: bool = True
    credential_ready: bool = False
    eligible: bool | None = None

    def __post_init__(self) -> None:
        _text("provider", self.provider)
        _text("model", self.model)
        if self.status not in {"success", "failure", "timeout", "rate_limited", "circuit_open", "unknown"}:
            raise BrainControlError("unsupported provider health status")
        _bounded_uint("latency_ms", self.latency_ms, 0, 600_000, 0)
        _bounded_uint("tokens", self.tokens, 0, 1_000_000_000, 0)
        if self.quality is not None and (
            not isinstance(self.quality, (int, float))
            or isinstance(self.quality, bool)
            or not math.isfinite(float(self.quality))
            or not 0.0 <= float(self.quality) <= 1.0
        ):
            raise BrainControlError("quality must be finite and within [0, 1]")
        if not isinstance(self.registered, bool) or not isinstance(self.credential_ready, bool):
            raise BrainControlError("registered and credential_ready must be booleans")
        if self.eligible is not None and not isinstance(self.eligible, bool):
            raise BrainControlError("eligible must be a boolean or None")

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "operation": "record",
            "provider": self.provider,
            "model": self.model,
            "status": self.status,
            "latency_ms": self.latency_ms,
            "tokens": self.tokens,
            "registered": self.registered,
            "credential_ready": self.credential_ready,
            "eligible": (
                self.registered and self.credential_ready
                if self.eligible is None
                else self.eligible
            ),
        }
        if self.quality is not None:
            result["quality"] = float(self.quality)
        return result


@dataclass(frozen=True, slots=True)
class BrainReplayRequest:
    """Digest-bound normalized evidence for an offline domain evaluator."""

    case_id: str
    domain: str
    capability: str
    risk_class: str
    signals: Mapping[str, Any]
    evidence_digest: str | None = None
    references: Sequence[str] = ()
    limitations: Sequence[str] = ()
    required_signals: Sequence[str] | None = None
    signal_weights: Mapping[str, float] | None = None
    pass_threshold: float | None = None

    def __post_init__(self) -> None:
        _text("case_id", self.case_id)
        _text("domain", self.domain)
        _text("capability", self.capability)
        _text("risk_class", self.risk_class)
        normalized = _signal_map(self.signals)
        references = tuple(_digest("reference", value) for value in self.references)
        if len(references) > 64:
            raise BrainControlError("references exceed the 64-item bound")
        limitations = tuple(
            _text("limitation", value, _MAX_REASON_BYTES) for value in self.limitations
        )
        if len(limitations) > 32:
            raise BrainControlError("limitations exceed the 32-item bound")
        if self.required_signals is not None:
            if not self.required_signals or len(self.required_signals) > _MAX_SIGNALS:
                raise BrainControlError("required_signals must contain 1..64 entries")
            required = tuple(
                _text("required_signal", value, 128) for value in self.required_signals
            )
            if len(set(required)) != len(required):
                raise BrainControlError("required_signals must be unique")
        else:
            required = None
        weights: dict[str, float] | None = None
        if self.signal_weights is not None:
            if not self.signal_weights or len(self.signal_weights) > _MAX_SIGNALS:
                raise BrainControlError("signal_weights must contain 1..64 entries")
            weights = {}
            for name, value in self.signal_weights.items():
                if not isinstance(name, str) or not _IDENTIFIER.fullmatch(name):
                    raise BrainControlError("signal weight names must be safe identifiers")
                if (
                    not isinstance(value, (int, float))
                    or isinstance(value, bool)
                    or not math.isfinite(float(value))
                    or float(value) <= 0
                ):
                    raise BrainControlError("signal weights must be finite positive numbers")
                weights[name] = float(value)
        if self.pass_threshold is not None and (
            not isinstance(self.pass_threshold, (int, float))
            or isinstance(self.pass_threshold, bool)
            or not math.isfinite(float(self.pass_threshold))
            or not 0.0 <= float(self.pass_threshold) <= 1.0
        ):
            raise BrainControlError("pass_threshold must be finite and within [0, 1]")
        evidence = {
            "schema": "bioprism-brain-domain-evaluator/0.1",
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "signals": normalized,
            "references": list(references),
            "limitations": list(limitations),
            "retention": "value_only_digests_and_signal_scores",
        }
        computed = _canonical_digest(evidence)
        if self.evidence_digest is not None and _digest("evidence_digest", self.evidence_digest) != computed:
            raise BrainControlError("evidence_digest does not match normalized replay evidence")
        object.__setattr__(self, "signals", normalized)
        object.__setattr__(self, "references", references)
        object.__setattr__(self, "limitations", limitations)
        object.__setattr__(self, "required_signals", required)
        object.__setattr__(self, "signal_weights", weights)
        object.__setattr__(self, "evidence_digest", computed)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "case_id": self.case_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "evidence_digest": self.evidence_digest,
            "signals": dict(self.signals),
            "references": list(self.references),
            "limitations": list(self.limitations),
        }
        if self.required_signals is not None:
            result["required_signals"] = list(self.required_signals)
        if self.signal_weights is not None:
            result["signal_weights"] = dict(self.signal_weights)
        if self.pass_threshold is not None:
            result["pass_threshold"] = float(self.pass_threshold)
        return result


@dataclass(frozen=True, slots=True)
class BrainEventPageRequest:
    """A bounded cursor request for the metadata-only journal."""

    job_id: str | None = None
    after: int = 0
    limit: int = 100

    def __post_init__(self) -> None:
        if self.job_id is not None:
            _text("job_id", self.job_id)
        _bounded_uint("after", self.after, 0, 2**63 - 1, 0)
        _bounded_uint("limit", self.limit, 1, 256, 100)

    def to_arguments(self) -> dict[str, Any]:
        result = {"after": self.after, "limit": self.limit}
        if self.job_id is not None:
            result["job_id"] = self.job_id
        return result


def _as_submission(value: BrainJobSubmission | Mapping[str, Any]) -> BrainJobSubmission:
    return value if isinstance(value, BrainJobSubmission) else BrainJobSubmission(**dict(value))


def _as_approval(value: BrainApprovalCommand | Mapping[str, Any]) -> BrainApprovalCommand:
    return value if isinstance(value, BrainApprovalCommand) else BrainApprovalCommand(**dict(value))


def _as_claim(value: BrainJobClaimCommand | Mapping[str, Any]) -> BrainJobClaimCommand:
    return value if isinstance(value, BrainJobClaimCommand) else BrainJobClaimCommand(**dict(value))


def _as_renew(value: BrainJobRenewCommand | Mapping[str, Any]) -> BrainJobRenewCommand:
    return value if isinstance(value, BrainJobRenewCommand) else BrainJobRenewCommand(**dict(value))


def _as_checkpoint(value: BrainJobCheckpointCommand | Mapping[str, Any]) -> BrainJobCheckpointCommand:
    return value if isinstance(value, BrainJobCheckpointCommand) else BrainJobCheckpointCommand(**dict(value))


def _as_complete(value: BrainJobCompleteCommand | Mapping[str, Any]) -> BrainJobCompleteCommand:
    return value if isinstance(value, BrainJobCompleteCommand) else BrainJobCompleteCommand(**dict(value))


def _as_fail(value: BrainJobFailCommand | Mapping[str, Any]) -> BrainJobFailCommand:
    return value if isinstance(value, BrainJobFailCommand) else BrainJobFailCommand(**dict(value))


def _as_reconcile(value: BrainJobReconcileCommand | Mapping[str, Any]) -> BrainJobReconcileCommand:
    return value if isinstance(value, BrainJobReconcileCommand) else BrainJobReconcileCommand(**dict(value))


def _as_health(value: BrainHealthObservation | Mapping[str, Any]) -> BrainHealthObservation:
    return value if isinstance(value, BrainHealthObservation) else BrainHealthObservation(**dict(value))


def _as_replay(value: BrainReplayRequest | Mapping[str, Any]) -> BrainReplayRequest:
    return value if isinstance(value, BrainReplayRequest) else BrainReplayRequest(**dict(value))


def _as_events(value: BrainEventPageRequest | Mapping[str, Any] | None) -> BrainEventPageRequest:
    if value is None:
        return BrainEventPageRequest()
    return value if isinstance(value, BrainEventPageRequest) else BrainEventPageRequest(**dict(value))


class BrainControlClient:
    """Typed synchronous facade over an existing HTTP or MCP call_tool transport."""

    def __init__(self, call_tool: Callable[[str, Mapping[str, Any]], Mapping[str, Any]]) -> None:
        if not callable(call_tool):
            raise BrainControlError("call_tool transport must be callable")
        self._call_tool = call_tool

    @classmethod
    def from_http(cls, client: Any) -> "BrainControlClient":
        if not hasattr(client, "call_tool"):
            raise BrainControlError("HTTP client must expose call_tool")
        return cls(client.call_tool)

    @classmethod
    def from_mcp(cls, client: Any) -> "BrainControlClient":
        if not hasattr(client, "call_tool"):
            raise BrainControlError("MCP client must expose call_tool")

        def invoke(name: str, arguments: Mapping[str, Any]) -> Mapping[str, Any]:
            result = client.call_tool(name, arguments)
            if not hasattr(result, "require_ok"):
                raise BrainControlError("MCP transport returned an unrecognised tool result")
            return result.require_ok()

        return cls(invoke)

    @classmethod
    def from_durable(cls, adapter: Any) -> "BrainControlClient":
        """Bind to an application-owned durable brain transport adapter."""

        if not hasattr(adapter, "call_tool"):
            raise BrainControlError("durable adapter must expose call_tool")
        return cls(adapter.call_tool)

    def _invoke(self, name: str, arguments: Mapping[str, Any]) -> dict[str, Any]:
        payload = self._call_tool(name, dict(arguments))
        if not isinstance(payload, Mapping):
            raise BrainControlError(f"{name} returned a non-object payload")
        payload = dict(payload)
        if payload.get("ok") is False:
            raise BrainControlRefusal(name, payload)
        return payload

    def submit_job(self, request: BrainJobSubmission | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_submit", _as_submission(request).to_arguments())

    def job_status(self, job_id: str) -> dict[str, Any]:
        return self._invoke("brain_job_status", {"job_id": _text("job_id", job_id)})

    def job_events(
        self,
        request: BrainEventPageRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._invoke("brain_job_events", _as_events(request).to_arguments())

    def approval(self, request: BrainApprovalCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_approval", _as_approval(request).to_arguments())

    def claim_job(self, request: BrainJobClaimCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_claim", _as_claim(request).to_arguments())

    def renew_job(self, request: BrainJobRenewCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_renew", _as_renew(request).to_arguments())

    def checkpoint_job(self, request: BrainJobCheckpointCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_checkpoint", _as_checkpoint(request).to_arguments())

    def complete_job(self, request: BrainJobCompleteCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_complete", _as_complete(request).to_arguments())

    def fail_job(self, request: BrainJobFailCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_fail", _as_fail(request).to_arguments())

    def reconcile_job(self, request: BrainJobReconcileCommand | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_job_reconcile", _as_reconcile(request).to_arguments())

    def record_health(
        self,
        observation: BrainHealthObservation | Mapping[str, Any],
    ) -> dict[str, Any]:
        return self._invoke("brain_model_health", _as_health(observation).to_arguments())

    def health_snapshot(self, provider: str | None = None) -> dict[str, Any]:
        arguments: dict[str, Any] = {"operation": "snapshot"}
        if provider is not None:
            arguments["provider"] = _text("provider", provider)
        return self._invoke("brain_model_health", arguments)

    def replay(self, request: BrainReplayRequest | Mapping[str, Any]) -> dict[str, Any]:
        return self._invoke("brain_replay_evaluate", _as_replay(request).to_arguments())


class AsyncBrainControlClient:
    """Async counterpart for an HTTP or stdio transport with an awaitable call_tool."""

    def __init__(
        self,
        call_tool: Callable[[str, Mapping[str, Any]], Awaitable[Mapping[str, Any]]],
    ) -> None:
        if not callable(call_tool):
            raise BrainControlError("async call_tool transport must be callable")
        self._call_tool = call_tool

    @classmethod
    def from_http(cls, client: Any) -> "AsyncBrainControlClient":
        if not hasattr(client, "call_tool"):
            raise BrainControlError("async HTTP client must expose call_tool")
        return cls(client.call_tool)

    @classmethod
    def from_mcp(cls, client: Any) -> "AsyncBrainControlClient":
        if not hasattr(client, "call_tool"):
            raise BrainControlError("async MCP client must expose call_tool")

        async def invoke(name: str, arguments: Mapping[str, Any]) -> Mapping[str, Any]:
            result = await client.call_tool(name, arguments)
            if not hasattr(result, "require_ok"):
                raise BrainControlError("async MCP transport returned an unrecognised tool result")
            return result.require_ok()

        return cls(invoke)

    @classmethod
    def from_durable(cls, adapter: Any) -> "AsyncBrainControlClient":
        """Bind to an async application-owned durable brain transport adapter."""

        if not hasattr(adapter, "call_tool"):
            raise BrainControlError("async durable adapter must expose call_tool")
        return cls(adapter.call_tool)

    async def _invoke(self, name: str, arguments: Mapping[str, Any]) -> dict[str, Any]:
        payload = await self._call_tool(name, dict(arguments))
        if not isinstance(payload, Mapping):
            raise BrainControlError(f"{name} returned a non-object payload")
        payload = dict(payload)
        if payload.get("ok") is False:
            raise BrainControlRefusal(name, payload)
        return payload

    async def submit_job(self, request: BrainJobSubmission | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_submit", _as_submission(request).to_arguments())

    async def job_status(self, job_id: str) -> dict[str, Any]:
        return await self._invoke("brain_job_status", {"job_id": _text("job_id", job_id)})

    async def job_events(
        self,
        request: BrainEventPageRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        return await self._invoke("brain_job_events", _as_events(request).to_arguments())

    async def approval(self, request: BrainApprovalCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_approval", _as_approval(request).to_arguments())

    async def claim_job(self, request: BrainJobClaimCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_claim", _as_claim(request).to_arguments())

    async def renew_job(self, request: BrainJobRenewCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_renew", _as_renew(request).to_arguments())

    async def checkpoint_job(self, request: BrainJobCheckpointCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_checkpoint", _as_checkpoint(request).to_arguments())

    async def complete_job(self, request: BrainJobCompleteCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_complete", _as_complete(request).to_arguments())

    async def fail_job(self, request: BrainJobFailCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_fail", _as_fail(request).to_arguments())

    async def reconcile_job(self, request: BrainJobReconcileCommand | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_job_reconcile", _as_reconcile(request).to_arguments())

    async def record_health(
        self,
        observation: BrainHealthObservation | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await self._invoke("brain_model_health", _as_health(observation).to_arguments())

    async def health_snapshot(self, provider: str | None = None) -> dict[str, Any]:
        arguments: dict[str, Any] = {"operation": "snapshot"}
        if provider is not None:
            arguments["provider"] = _text("provider", provider)
        return await self._invoke("brain_model_health", arguments)

    async def replay(self, request: BrainReplayRequest | Mapping[str, Any]) -> dict[str, Any]:
        return await self._invoke("brain_replay_evaluate", _as_replay(request).to_arguments())


__all__ = [
    "CONTROL_SCHEMA",
    "AsyncBrainControlClient",
    "BrainApprovalCommand",
    "BrainControlClient",
    "BrainControlError",
    "BrainControlRefusal",
    "BrainJobClaimCommand",
    "BrainJobCheckpointCommand",
    "BrainJobCompleteCommand",
    "BrainJobFailCommand",
    "BrainJobReconcileCommand",
    "BrainJobRenewCommand",
    "BrainEventPageRequest",
    "BrainHealthObservation",
    "BrainJobSubmission",
    "BrainReplayRequest",
]
