"""Restart-safe transport adapter for the autonomous-brain control plane.

``BrainJobStore`` is the durable state machine.  This module is the small application-owned
bridge that lets an HTTP, MCP, or in-process host expose that state machine through the same
``brain_job_*`` tool names as the Rust projection.  It intentionally accepts only the value-only
wire contract: callers retain task/prompt/provider material and rehydrate it from their private
resolver after a restart.

The adapter is fail-closed for mutations.  A host must provide an authorization callback; a
caller-supplied digest is evidence metadata, not authentication.  No API key, prompt, response,
checkpoint body, failure text, or result metadata is returned by this adapter.
"""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Mapping
from copy import deepcopy
from typing import Any

from .control_plane import (
    BrainControlPlane,
    BrainRunError,
)
from .jobs import (
    JOB_EVENT_SCHEMA,
    JOB_SCHEMA,
    BrainJobError,
    BrainJobEvent,
    BrainJobRecord,
    BrainJobStore,
)
from .memory import _canonical, _digest, _valid_digest


DURABLE_BRAIN_TRANSPORT_SCHEMA = "bioprism-brain-durable-transport/0.1"
CONTROL_SCHEMA = "bioprism-brain-control-plane/0.1"
MAX_DURABLE_TRANSPORT_PAGE = 256
MAX_LEASE_MS = 86_400_000
MIN_LEASE_MS = 100


class DurableBrainTransportError(RuntimeError):
    """A durable transport request could not be admitted or applied."""


class DurableBrainAuthorizationError(DurableBrainTransportError):
    """The application-owned authorization boundary refused a mutation."""


AuthorizationCallback = Callable[[str, Mapping[str, Any]], bool]


def _text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise DurableBrainTransportError(f"{name} must be a non-empty NUL-free string")
    if len(value.encode("utf-8")) > maximum:
        raise DurableBrainTransportError(f"{name} exceeds its bounded size")
    return value


def _digest_argument(name: str, value: Any) -> str:
    value = _text(name, value, 64)
    if not _valid_digest(value):
        raise DurableBrainTransportError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _uint(name: str, value: Any, minimum: int, maximum: int, default: int) -> int:
    if value is None:
        value = default
    if not isinstance(value, int) or isinstance(value, bool) or not minimum <= value <= maximum:
        raise DurableBrainTransportError(f"{name} must be an integer within [{minimum}, {maximum}]")
    return value


def _bool(name: str, value: Any, default: bool = False) -> bool:
    if value is None:
        return default
    if not isinstance(value, bool):
        raise DurableBrainTransportError(f"{name} must be a boolean")
    return value


def _sha256_text(value: str) -> str:
    return _digest(value)


def _safe_error_digest(error: Exception) -> str:
    # The error digest supports diagnostics without echoing a provider key or caller payload.
    return _sha256_text(str(error)[:2_048])


def _arguments(tool: str, arguments: Mapping[str, Any] | None, allowed: set[str]) -> dict[str, Any]:
    if arguments is None:
        return {}
    if not isinstance(arguments, Mapping) or any(not isinstance(key, str) for key in arguments):
        raise DurableBrainTransportError(f"{tool} arguments must be an object with string keys")
    unknown = sorted(set(arguments).difference(allowed))
    if unknown:
        raise DurableBrainTransportError(f"{tool} contains unsupported fields: {', '.join(unknown)}")
    return dict(arguments)


def _valid_optional_digest(value: Any) -> str | None:
    return value if isinstance(value, str) and _valid_digest(value) else None


class DurableBrainControlPlaneAdapter:
    """Expose a :class:`BrainJobStore` through the Rust-compatible brain job tool contract.

    The adapter is intentionally synchronous because it delegates SQLite transactions to the
    store.  Async HTTP/MCP hosts can call it from their application executor, while Python code
    can pass ``adapter.call_tool`` directly to ``BrainControlClient``.
    """

    TOOL_NAMES = tuple(
        (
            "brain_job_submit",
            "brain_job_status",
            "brain_job_events",
            "brain_job_approval",
            "brain_job_claim",
            "brain_job_renew",
            "brain_job_checkpoint",
            "brain_job_complete",
            "brain_job_fail",
            "brain_job_reconcile",
        )
    )

    @classmethod
    def tool_definitions(cls) -> tuple[dict[str, Any], ...]:
        """Return bounded schemas a host can register with HTTP or MCP discovery."""

        digest = {"type": "string", "pattern": "^[0-9a-f]{64}$"}
        identifier = {"type": "string", "minLength": 1, "maxLength": 256}
        definitions = (
            (
                "brain_job_submit",
                "Admit a restart-safe value-only autonomous-brain job identity.",
                {"job_id": identifier, "idempotency_key": identifier, "spec_digest": digest, "domain": identifier, "capability": identifier, "risk_class": identifier, "priority": {"type": "integer", "minimum": 0, "maximum": 255}, "max_attempts": {"type": "integer", "minimum": 1, "maximum": 8}, "checkpoint_digest": digest},
                ("idempotency_key", "spec_digest", "domain", "capability", "risk_class"),
            ),
            ("brain_job_status", "Read one durable metadata-only job projection.", {"job_id": identifier}, ("job_id",)),
            ("brain_job_events", "Read a bounded cursor page of hash-chained metadata-only events.", {"job_id": identifier, "after": {"type": "integer", "minimum": 0}, "limit": {"type": "integer", "minimum": 1, "maximum": MAX_DURABLE_TRANSPORT_PAGE}}, ()),
            ("brain_job_approval", "Request or decide a caller-owned approval boundary.", {"job_id": identifier, "action": {"type": "string", "enum": ["request", "approve", "deny"]}, "reason": {"type": "string", "maxLength": 2048}, "authorization_digest": digest}, ("job_id", "action")),
            ("brain_job_claim", "Acquire a bounded worker lease.", {"job_id": identifier, "worker_id": identifier, "lease_ms": {"type": "integer", "minimum": MIN_LEASE_MS, "maximum": MAX_LEASE_MS}}, ("job_id", "worker_id")),
            ("brain_job_renew", "Extend an owned worker lease.", {"job_id": identifier, "worker_id": identifier, "lease_ms": {"type": "integer", "minimum": MIN_LEASE_MS, "maximum": MAX_LEASE_MS}}, ("job_id", "worker_id")),
            ("brain_job_checkpoint", "Persist a phase and monotonic external-effect boundary digest.", {"job_id": identifier, "worker_id": identifier, "phase": {"type": "string", "maxLength": 128}, "checkpoint_digest": digest, "side_effect_boundary": {"type": "string", "enum": ["not_started", "preflight", "dispatched", "unknown"]}, "waiting_for_approval": {"type": "boolean"}}, ("job_id", "worker_id", "phase", "checkpoint_digest")),
            ("brain_job_complete", "Settle an owned lease with a caller-owned result digest.", {"job_id": identifier, "worker_id": identifier, "result_digest": digest}, ("job_id", "worker_id", "result_digest")),
            ("brain_job_fail", "Record a bounded failure without retaining the failure text.", {"job_id": identifier, "worker_id": identifier, "reason": {"type": "string", "maxLength": 2048}, "retryable": {"type": "boolean"}}, ("job_id", "worker_id", "reason")),
            ("brain_job_reconcile", "Resolve an uncertain external effect with explicit caller evidence.", {"job_id": identifier, "outcome": {"type": "string", "enum": ["succeeded", "failed", "not_executed", "unknown"]}, "evidence_digest": digest, "evidence_kind": {"type": "string", "maxLength": 128}, "operator": identifier, "reason": {"type": "string", "maxLength": 2048}, "effect_absent": {"type": "boolean"}}, ("job_id", "outcome", "evidence_digest")),
        )
        return tuple(
            deepcopy(
                {
                    "name": name,
                    "description": description,
                    "inputSchema": {
                        "type": "object",
                        "additionalProperties": False,
                        "properties": properties,
                        "required": list(required),
                    },
                }
            )
            for name, description, properties, required in definitions
        )

    def __init__(
        self,
        store: BrainJobStore,
        *,
        authorizer: AuthorizationCallback | None = None,
        principal: str = "durable-control-plane",
    ) -> None:
        if not isinstance(store, BrainJobStore):
            raise DurableBrainTransportError("durable transport requires a BrainJobStore")
        if authorizer is not None and not callable(authorizer):
            raise DurableBrainTransportError("authorizer must be callable")
        self.store = store
        self.control = BrainControlPlane(store)
        self.authorizer = authorizer
        self.principal = _text("principal", principal)

    def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Handle one value-only control-plane tool request.

        Refusals are returned as ``ok: false`` envelopes so the existing typed clients preserve
        their normal ``BrainControlRefusal`` behavior across HTTP, MCP, and this durable bridge.
        The envelope contains only an error code and a diagnostic digest.
        """

        if not isinstance(name, str) or name not in self.TOOL_NAMES:
            return self._refusal(str(name), "unknown_tool", None)
        try:
            result = getattr(self, f"_{name}")(arguments)
            if not isinstance(result, dict):
                raise DurableBrainTransportError("durable tool handler returned a non-object")
            return result
        except DurableBrainAuthorizationError as error:
            return self._refusal(name, "authorization_required", error)
        except (DurableBrainTransportError, BrainJobError, BrainRunError) as error:
            return self._refusal(name, "operation_refused", error)
        except (TypeError, ValueError, KeyError) as error:
            return self._refusal(name, "malformed_request", error)

    def _authorize(self, operation: str, arguments: Mapping[str, Any], *, required: bool = True) -> None:
        if not required:
            return
        if self.authorizer is None:
            raise DurableBrainAuthorizationError(
                "mutating durable brain operations require an application-owned authorizer"
            )
        metadata: dict[str, Any] = {"operation": operation, "principal": self.principal}
        if "job_id" in arguments:
            metadata["job_id"] = arguments["job_id"]
        if "worker_id" in arguments:
            metadata["worker_id_digest"] = _sha256_text(str(arguments["worker_id"]))
        if "authorization_digest" in arguments:
            metadata["authorization_digest"] = arguments["authorization_digest"]
        if "reason" in arguments:
            metadata["reason_digest"] = _sha256_text(str(arguments["reason"]))
        try:
            allowed = self.authorizer(operation, metadata)
        except Exception as error:
            raise DurableBrainAuthorizationError("application authorizer failed") from error
        if allowed is not True:
            raise DurableBrainAuthorizationError("application authorizer refused the operation")

    def _base(self) -> dict[str, Any]:
        return {
            "schema": CONTROL_SCHEMA,
            "ok": True,
            "retention": "metadata_only_hash_chained",
            "durability": {
                "scope": "python_sqlite",
                "restart": "durable_brain_job_store",
                "secrets": "never_retained_by_transport_projection",
                "authorization": "application_owned_callback; caller_digest_is_not_authentication",
            },
        }

    def _refusal(self, tool: str, code: str, error: Exception | None) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": CONTROL_SCHEMA,
            "ok": False,
            "error": code,
            "retention": "metadata_only_hash_chained",
        }
        if error is not None:
            result["error_digest"] = _safe_error_digest(error)
        return result

    def _brain_job_submit(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments(
            "brain_job_submit",
            arguments,
            {
                "job_id",
                "idempotency_key",
                "spec_digest",
                "domain",
                "capability",
                "risk_class",
                "priority",
                "max_attempts",
                "checkpoint_digest",
            },
        )
        self._authorize("brain_job_submit", args)
        packet: dict[str, Any] = {
            "job_id": args.get("job_id"),
            "idempotency_key": _text("idempotency_key", args.get("idempotency_key")),
            "spec_digest": _digest_argument("spec_digest", args.get("spec_digest")),
            "domain": _text("domain", args.get("domain")),
            "capability": _text("capability", args.get("capability")),
            "risk_class": _text("risk_class", args.get("risk_class")),
            "priority": _uint("priority", args.get("priority"), 0, 255, 0),
            "max_attempts": _uint("max_attempts", args.get("max_attempts"), 1, 8, 3),
            "checkpoint": {},
        }
        if packet["job_id"] is None:
            packet.pop("job_id")
        checkpoint_digest = args.get("checkpoint_digest")
        if checkpoint_digest is not None:
            packet["checkpoint"] = {"checkpoint_digest": _digest_argument("checkpoint_digest", checkpoint_digest)}
        record, receipt = self.store.submit(packet)
        result = self._base()
        result.update(
            {
                "created": not receipt.idempotent,
                "idempotent": receipt.idempotent,
                "job": self._job_projection(record),
                "event": self._event_by_sequence(receipt.sequence, record.job_id),
            }
        )
        return result

    def _brain_job_status(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments("brain_job_status", arguments, {"job_id"})
        job_id = _text("job_id", args.get("job_id"))
        record = self.store.get(job_id)
        if record is None:
            raise BrainJobError("unknown brain job")
        result = self._base()
        result.update({"job": self._job_projection(record), "head_digest": self.store.head_digest()})
        return result

    def _brain_job_events(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments("brain_job_events", arguments, {"job_id", "after", "limit"})
        job_id = None if args.get("job_id") is None else _text("job_id", args["job_id"])
        if job_id is not None and self.store.get(job_id) is None:
            raise BrainJobError("unknown brain job")
        after = _uint("after", args.get("after"), 0, 2**63 - 1, 0)
        limit = _uint("limit", args.get("limit"), 1, MAX_DURABLE_TRANSPORT_PAGE, 100)
        rows = self.store.events(after_sequence=after, job_id=job_id, limit=min(limit, MAX_DURABLE_TRANSPORT_PAGE))
        result = self._base()
        result.update(
            {
                "events": [self._event_projection(event) for event in rows],
                "after": after,
                "next_after": after if not rows else rows[-1].sequence,
                "head_digest": self.store.head_digest(),
                "chain": "sha256_prev_digest",
            }
        )
        return result

    def _brain_job_approval(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments(
            "brain_job_approval", arguments, {"job_id", "action", "reason", "authorization_digest"}
        )
        self._authorize("brain_job_approval", args)
        job_id = _text("job_id", args.get("job_id"))
        action = _text("action", args.get("action"), 32)
        if action not in {"request", "approve", "deny"}:
            raise DurableBrainTransportError("action must be request, approve, or deny")
        reason = args.get("reason")
        if reason is not None:
            reason = _text("reason", reason, 2_048)
        authorization_digest = args.get("authorization_digest")
        if action in {"approve", "deny"}:
            authorization_digest = _digest_argument("authorization_digest", authorization_digest)
        elif authorization_digest is not None:
            authorization_digest = _digest_argument("authorization_digest", authorization_digest)
        before = self.store.get(job_id)
        if before is None:
            raise BrainJobError("unknown brain job")
        safe_reason = "caller approval decision"
        if action == "request":
            request_digest = _sha256_text(
                _canonical({"job_id": job_id, "reason_digest": None if reason is None else _sha256_text(reason)})
            )
            self.control.approvals.request(
                job_id,
                self.principal,
                approval_scope="caller approval boundary",
                request_digest=request_digest,
            )
        elif action == "approve":
            self.control.approvals.approve(job_id, approver=self.principal, reason=safe_reason)
        else:
            self.control.approvals.deny(job_id, approver=self.principal, reason=safe_reason)
        after = self.store.get(job_id)
        if after is None:
            raise BrainJobError("job disappeared after approval transition")
        result = self._base()
        result.update(
            {
                "operation": action,
                "idempotent": action == "request" and before.state == "waiting_approval",
                "job": self._job_projection(after),
                "event": None
                if action == "request" and before.state == "waiting_approval"
                else self._event_for_record(after),
                "authorization": {
                    "posture": "application_authorizer_callback",
                    "verified_by_server": False,
                    "authorized_by_adapter": True,
                    "authorization_digest": authorization_digest,
                    "execution": "not_started",
                },
            }
        )
        return result

    def _brain_job_claim(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments("brain_job_claim", arguments, {"job_id", "worker_id", "lease_ms"})
        self._authorize("brain_job_claim", args)
        job_id = _text("job_id", args.get("job_id"))
        worker_id = _text("worker_id", args.get("worker_id"))
        lease_ms = _uint("lease_ms", args.get("lease_ms"), MIN_LEASE_MS, MAX_LEASE_MS, 60_000)
        before = self.store.get(job_id)
        if before is None:
            raise BrainJobError("unknown brain job")
        record = self.store.claim(job_id, worker_id, lease_seconds=lease_ms / 1_000.0)
        idempotent = before.terminal or (
            before.state in {"leased", "running"} and before.lease_owner == worker_id
        )
        return self._transition_response("claim", record, idempotent=idempotent, include_event=not idempotent)

    def _brain_job_renew(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments("brain_job_renew", arguments, {"job_id", "worker_id", "lease_ms"})
        self._authorize("brain_job_renew", args)
        job_id = _text("job_id", args.get("job_id"))
        worker_id = _text("worker_id", args.get("worker_id"))
        lease_ms = _uint("lease_ms", args.get("lease_ms"), MIN_LEASE_MS, MAX_LEASE_MS, 60_000)
        record = self.store.renew(job_id, worker_id, lease_seconds=lease_ms / 1_000.0)
        return self._transition_response("renew", record)

    def _brain_job_checkpoint(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments(
            "brain_job_checkpoint",
            arguments,
            {"job_id", "worker_id", "phase", "checkpoint_digest", "side_effect_boundary", "waiting_for_approval"},
        )
        self._authorize("brain_job_checkpoint", args)
        record = self.store.checkpoint(
            _text("job_id", args.get("job_id")),
            _text("worker_id", args.get("worker_id")),
            phase=_text("phase", args.get("phase"), 128),
            checkpoint={"checkpoint_digest": _digest_argument("checkpoint_digest", args.get("checkpoint_digest"))},
            side_effect_boundary=_text("side_effect_boundary", args.get("side_effect_boundary", "not_started"), 32),
            waiting_for_approval=_bool("waiting_for_approval", args.get("waiting_for_approval")),
        )
        return self._transition_response("checkpoint", record)

    def _brain_job_complete(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments("brain_job_complete", arguments, {"job_id", "worker_id", "result_digest"})
        self._authorize("brain_job_complete", args)
        record = self.store.complete(
            _text("job_id", args.get("job_id")),
            _text("worker_id", args.get("worker_id")),
            result_metadata={"result_digest": _digest_argument("result_digest", args.get("result_digest"))},
        )
        return self._transition_response("complete", record)

    def _brain_job_fail(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments("brain_job_fail", arguments, {"job_id", "worker_id", "reason", "retryable"})
        self._authorize("brain_job_fail", args)
        reason = _text("reason", args.get("reason"), 2_048)
        record = self.store.fail(
            _text("job_id", args.get("job_id")),
            _text("worker_id", args.get("worker_id")),
            reason="caller reported a bounded failure",
            reason_digest=_sha256_text(reason),
            retryable=_bool("retryable", args.get("retryable")),
        )
        return self._transition_response("fail", record)

    def _brain_job_reconcile(self, arguments: Mapping[str, Any] | None) -> dict[str, Any]:
        args = _arguments(
            "brain_job_reconcile",
            arguments,
            {"job_id", "outcome", "evidence_digest", "evidence_kind", "operator", "reason", "effect_absent"},
        )
        self._authorize("brain_job_reconcile", args)
        job_id = _text("job_id", args.get("job_id"))
        outcome = _text("outcome", args.get("outcome"), 32)
        if outcome not in {"succeeded", "failed", "not_executed", "unknown"}:
            raise DurableBrainTransportError("outcome must be succeeded, failed, not_executed, or unknown")
        evidence_digest = _digest_argument("evidence_digest", args.get("evidence_digest"))
        evidence_kind = _text("evidence_kind", args.get("evidence_kind", "caller_observation"), 128)
        operator = _text("operator", args.get("operator", "caller"))
        reason = _text("reason", args.get("reason", "caller reconciled uncertain external state"), 2_048)
        effect_absent = _bool("effect_absent", args.get("effect_absent"))
        if outcome == "not_executed" and not effect_absent:
            raise DurableBrainTransportError("not_executed reconciliation requires effect_absent=True")
        before = self.store.get(job_id)
        receipt = self.control.reconciliations.resolve(
            job_id,
            outcome=outcome,
            evidence_digest=evidence_digest,
            evidence_kind=evidence_kind,
            operator=operator,
            reason="caller reconciliation decision",
            metadata={"effect_absent": effect_absent, "reason_digest": _sha256_text(reason)},
        )
        record = self.store.get(job_id)
        if record is None:
            raise BrainJobError("job disappeared after reconciliation")
        result = self._transition_response(
            "reconcile",
            record,
            idempotent=receipt.idempotent,
            include_event=not receipt.idempotent,
        )
        result["reconciliation"] = {
            "outcome": receipt.outcome,
            "evidence_digest": receipt.evidence_digest,
            "evidence_kind": receipt.evidence_kind,
            "operator_digest": _sha256_text(receipt.operator),
            "reason_digest": _sha256_text(reason),
            "decision_digest": receipt.decision_digest,
            "state": receipt.state,
            "effect_absent": effect_absent,
        }
        if before is None:
            raise BrainJobError("unknown brain job")
        return result

    def _transition_response(
        self,
        operation: str,
        record: BrainJobRecord,
        *,
        idempotent: bool = False,
        include_event: bool = True,
    ) -> dict[str, Any]:
        result = self._base()
        result.update(
            {
                "operation": operation,
                "idempotent": idempotent,
                "job": self._job_projection(record),
                "event": self._event_for_record(record) if include_event else None,
            }
        )
        return result

    def _job_projection(self, record: BrainJobRecord) -> dict[str, Any]:
        checkpoint = record.checkpoint
        checkpoint_digest = _valid_optional_digest(checkpoint.get("checkpoint_digest"))
        if checkpoint_digest is None and checkpoint:
            checkpoint_digest = _sha256_text(_canonical(self._checkpoint_projection(checkpoint)))
        result_digest: str | None = None
        result_metadata = checkpoint.get("result_metadata")
        if isinstance(result_metadata, Mapping):
            result_digest = _valid_optional_digest(result_metadata.get("result_digest"))
        reconciliation = checkpoint.get("reconciliation")
        reconciliation_outcome = None
        reconciliation_digest = None
        if isinstance(reconciliation, Mapping):
            reconciliation_outcome = reconciliation.get("outcome") if isinstance(reconciliation.get("outcome"), str) else None
            reconciliation_digest = _valid_optional_digest(reconciliation.get("decision_digest"))
        reason_digest = _valid_optional_digest(checkpoint.get("reason_digest"))
        if reason_digest is None and isinstance(reconciliation, Mapping):
            reconciliation_metadata = reconciliation.get("metadata")
            if isinstance(reconciliation_metadata, Mapping):
                reason_digest = _valid_optional_digest(reconciliation_metadata.get("reason_digest"))
        if reason_digest is None and record.reason is not None:
            reason_digest = _sha256_text(record.reason)
        first_event = self.store.events(after_sequence=0, job_id=record.job_id, limit=1)
        created_sequence = first_event[0].sequence if first_event else record.record_sequence
        return {
            "schema": JOB_SCHEMA,
            "job_id": record.job_id,
            "idempotency_key_digest": _sha256_text(record.idempotency_key),
            "spec_digest": record.spec_digest,
            "domain": record.domain,
            "capability": record.capability,
            "risk_class": record.risk_class,
            "priority": record.priority,
            "max_attempts": record.max_attempts,
            "state": record.state,
            "attempts": record.attempts,
            "lease_owner": record.lease_owner,
            "lease_expires_ns": record.lease_expires_ns,
            "checkpoint_digest": checkpoint_digest,
            "side_effect_boundary": record.side_effect_boundary,
            "recovered_after_restart": record.recovered_after_restart,
            "reason_digest": reason_digest,
            "result_digest": result_digest,
            "reconciliation_outcome": reconciliation_outcome,
            "reconciliation_digest": reconciliation_digest,
            "created_sequence": created_sequence,
            "updated_sequence": record.record_sequence,
            "record_digest": record.record_digest,
            "spec": "not_returned; caller resolver owns rehydration",
            "retention": "metadata_only_hash_chained",
        }

    def _checkpoint_projection(self, checkpoint: Mapping[str, Any]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        phase = checkpoint.get("phase")
        if isinstance(phase, str):
            result["phase"] = phase
        digest = _valid_optional_digest(checkpoint.get("checkpoint_digest"))
        if digest is not None:
            result["checkpoint_digest"] = digest
        result_metadata = checkpoint.get("result_metadata")
        if isinstance(result_metadata, Mapping):
            result["result_digest"] = _valid_optional_digest(result_metadata.get("result_digest"))
        reconciliation = checkpoint.get("reconciliation")
        if isinstance(reconciliation, Mapping):
            result["reconciliation_digest"] = _valid_optional_digest(reconciliation.get("decision_digest"))
            result["reconciliation_outcome"] = reconciliation.get("outcome")
        if not result:
            result["metadata_digest"] = _sha256_text(_canonical(checkpoint))
        return result

    def _event_for_record(self, record: BrainJobRecord) -> dict[str, Any] | None:
        return self._event_by_sequence(record.record_sequence, record.job_id)

    def _event_by_sequence(self, sequence: int, job_id: str) -> dict[str, Any] | None:
        if not isinstance(sequence, int) or sequence <= 0:
            return None
        rows = self.store.events(after_sequence=sequence - 1, job_id=job_id, limit=1)
        return None if not rows or rows[0].sequence != sequence else self._event_projection(rows[0])

    def _event_projection(self, event: BrainJobEvent) -> dict[str, Any]:
        raw_payload = event.payload
        raw_details = raw_payload.get("details") if isinstance(raw_payload, Mapping) else None
        details = raw_details if isinstance(raw_details, Mapping) else {}
        projected_details: dict[str, Any] = {}
        scalar_fields = {
            "state",
            "previous_state",
            "attempts",
            "max_attempts",
            "priority",
            "lease_expires_ns",
            "side_effect_boundary",
            "spec_digest",
            "domain",
            "capability",
            "risk_class",
            "execution",
            "retryable",
        }
        for key in scalar_fields:
            value = details.get(key)
            if isinstance(value, (str, int, float, bool)) and not isinstance(value, float) or value is None:
                if key in details:
                    projected_details[key] = value
        for key in ("worker_id", "previous_owner", "lease_owner", "reason", "operator"):
            if key in details and details[key] is not None:
                projected_details[f"{key}_digest"] = _sha256_text(str(details[key]))
        for key in ("reason_digest", "authorization_digest", "result_digest", "checkpoint_digest", "evidence_digest", "decision_digest"):
            digest = _valid_optional_digest(details.get(key))
            if digest is not None:
                projected_details[key] = digest
        checkpoint = details.get("checkpoint")
        if isinstance(checkpoint, Mapping):
            projected_details["checkpoint_digest"] = _sha256_text(_canonical(self._checkpoint_projection(checkpoint)))
        reconciliation = details.get("reconciliation")
        if isinstance(reconciliation, Mapping):
            projected_reconciliation: dict[str, Any] = {}
            for key in ("outcome", "evidence_kind", "effect_absent"):
                if key in reconciliation and isinstance(reconciliation[key], (str, bool)):
                    projected_reconciliation[key] = reconciliation[key]
            for key in ("evidence_digest", "decision_digest"):
                digest = _valid_optional_digest(reconciliation.get(key))
                if digest is not None:
                    projected_reconciliation[key] = digest
            for key in ("operator", "reason"):
                if key in reconciliation and reconciliation[key] is not None:
                    projected_reconciliation[f"{key}_digest"] = _sha256_text(str(reconciliation[key]))
            if projected_reconciliation:
                projected_details["reconciliation"] = projected_reconciliation
        projected_payload = {
            "schema": JOB_EVENT_SCHEMA,
            "event": raw_payload.get("event", event.event_type),
            "job_id": event.job_id,
            "details": projected_details,
        }
        return {
            "schema": JOB_EVENT_SCHEMA,
            "sequence": event.sequence,
            "event_type": event.event_type,
            "job_id": event.job_id,
            "payload": projected_payload,
            "payload_digest": _sha256_text(_canonical(raw_payload)),
            "previous_digest": event.previous_digest,
            "event_digest": event.event_digest,
            "head_digest": event.event_digest,
            "created_ns": event.created_ns,
            "retention": "metadata_only_hash_chained; payload_projection",
        }


class AsyncDurableBrainControlPlaneAdapter:
    """Async transport façade for the same application-owned SQLite adapter.

    SQLite transactions remain synchronous and serialized by ``BrainJobStore``.  ``to_thread``
    keeps an async HTTP/MCP host's event loop responsive without creating a second state machine.
    """

    def __init__(self, adapter: DurableBrainControlPlaneAdapter) -> None:
        if not isinstance(adapter, DurableBrainControlPlaneAdapter):
            raise DurableBrainTransportError("async durable transport requires a durable adapter")
        self.adapter = adapter

    async def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.adapter.call_tool, name, arguments)


__all__ = [
    "AsyncDurableBrainControlPlaneAdapter",
    "AuthorizationCallback",
    "CONTROL_SCHEMA",
    "DURABLE_BRAIN_TRANSPORT_SCHEMA",
    "DurableBrainAuthorizationError",
    "DurableBrainControlPlaneAdapter",
    "DurableBrainTransportError",
]
