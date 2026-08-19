"""High-level autonomous decision loop over the Rust brain and :mod:`llm_runtime`.

This facade is intentionally bounded but real: it selects a model, assembles a bounded prompt,
validates a plan, requires explicit approval for the provider effect, and invokes the model with a
caller-owned credential handle. A structured model decision can then be proposed to the existing
mission executor for server-side preflight and a separate caller approval; the model never grants
itself tools, side effects, or credentials.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import threading
import uuid
from typing import Any, Callable, Mapping, Protocol, Sequence

from .llm_runtime import (
    CredentialHandle,
    LLMRuntime,
    ProviderRequest,
    ProviderResponse,
    ProviderTool,
    ProviderToolCall,
    ProviderToolLoopResult,
    ProviderToolResult,
)
from .errors import ArgumentError
from .mission import MissionPolicy, MissionRequest
from .tooling import ToolCatalogue, ToolSchemaError


class BrainRunError(RuntimeError):
    """The bounded autonomous loop could not reach a provider invocation."""


DEFAULT_MISSION_RESPONSE_SCHEMA: dict[str, Any] = {
    "type": "object",
    "required": ["mission"],
    "properties": {
        "mission": {
            "type": "object",
            "required": ["steps"],
            "properties": {
                "steps": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 128,
                    "items": {
                        "type": "object",
                        "required": [
                            "id",
                            "domain",
                            "capability",
                            "objective",
                            "tool",
                            "arguments",
                        ],
                        "properties": {
                            "id": {"type": "string", "minLength": 1, "maxLength": 256},
                            "domain": {"type": "string", "minLength": 1, "maxLength": 256},
                            "capability": {"type": "string", "minLength": 1, "maxLength": 256},
                            "objective": {"type": "string", "minLength": 1, "maxLength": 4096},
                            "tool": {"type": "string", "minLength": 1, "maxLength": 256},
                            "arguments": {"type": "object"},
                            "depends_on": {"type": "array", "items": {"type": "string"}, "maxItems": 128},
                            "required": {"type": "boolean"},
                            "bindings": {"type": "array", "maxItems": 128},
                        },
                    },
                }
            },
        }
    },
    "additionalProperties": False,
}

MAX_ROUTE_REQUEST_BYTES = 2_000_000
MAX_ROUTE_PROMPT_BYTES = 750_000
MAX_ROUTE_PROMPT_SCHEMAS = 128
MAX_MISSION_AUTHORIZATION_CALLS = 128
MAX_MISSION_AUTHORIZATION_RESULT_BYTES = 750_000
MAX_MISSION_AUTHORIZATION_STEP_OUTPUT_BYTES = 350_000


def _bounded_route_prompt_context(route: Mapping[str, Any]) -> dict[str, Any]:
    """Project route evidence into a bounded, model-readable context packet.

    The capability route is authoritative evidence about the live catalogue, but it can contain
    many schemas. The model needs the candidate contract, not an unbounded registry dump. Schemas
    are admitted in deterministic order until the packet bound is reached; omitted schemas remain
    explicit so a model cannot mistake a truncated route for a complete catalogue.
    """

    recommended = route.get("recommended_tools", [])
    if not isinstance(recommended, list) or any(not isinstance(tool, str) for tool in recommended):
        raise BrainRunError("capability route returned malformed recommended_tools")
    needs = route.get("needs", [])
    if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
        raise BrainRunError("capability route returned malformed needs")
    raw_schemas = route.get("tool_schemas", [])
    if not isinstance(raw_schemas, list) or any(not isinstance(schema, Mapping) for schema in raw_schemas):
        raise BrainRunError("capability route returned malformed tool_schemas")

    compact_needs: list[dict[str, Any]] = []
    for need in needs:
        compact_needs.append(
            {
                "id": need.get("id"),
                "resolution": need.get("resolution"),
                "candidate_groups": need.get("candidate_groups", []),
                "candidate_domains": need.get("candidate_domains", []),
                "candidate_tools": need.get("candidate_tools", []),
            }
        )
    packet: dict[str, Any] = {
        "workflow": "capability_route_context",
        "route_id": route.get("route_id"),
        "catalog_digest": route.get("catalog_digest"),
        "goal": route.get("goal"),
        "needs": compact_needs,
        "recommended_tools": recommended,
        "schema_attachment": route.get("schema_attachment", {}),
        "tool_schemas": [],
        "tool_schemas_omitted": 0,
        "does_not_authorize": [
            "candidate ranking is routing evidence, not permission",
            "the caller mission policy remains the only tool allow-list",
            "tool schemas describe inputs but do not establish domain validity or readiness",
        ],
    }
    for schema in raw_schemas[:MAX_ROUTE_PROMPT_SCHEMAS]:
        candidate = dict(packet)
        candidate["tool_schemas"] = [*packet["tool_schemas"], dict(schema)]
        encoded = json.dumps(candidate, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            break
        packet["tool_schemas"] = candidate["tool_schemas"]
    packet["tool_schemas_omitted"] = len(raw_schemas) - len(packet["tool_schemas"])
    return packet


class BrainLearningLedger:
    """Append-only, value-only persistence for evaluator judgments and bandit state.

    The ledger is deliberately separate from :class:`CredentialStore`: it accepts only the Rust
    learning report and the returned next state, rejects secret-shaped field names, bounds both
    record count and file size, and fsyncs each append. It never stores provider response text.
    """

    _SCHEMA = "bioprism-brain-learning-ledger/0.1"
    _FORBIDDEN_FIELDS = {
        "api_key",
        "apikey",
        "authorization",
        "credential",
        "password",
        "secret",
        "access_token",
        "refresh_token",
    }

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_records: int = 4096,
        max_bytes: int = 32_000_000,
    ) -> None:
        if max_records <= 0 or max_bytes <= 0:
            raise BrainRunError("learning ledger bounds must be positive")
        self.path = Path(path)
        self.max_records = max_records
        self.max_bytes = max_bytes
        self._lock = threading.RLock()

    def append(
        self,
        report: Mapping[str, Any],
        *,
        context_digest: str | None = None,
    ) -> dict[str, Any]:
        if not isinstance(report, Mapping):
            raise BrainRunError("learning ledger report must be an object")
        evidence = report.get("learning_evidence")
        next_state = report.get("next_state")
        if not isinstance(evidence, Mapping) or not isinstance(next_state, Mapping):
            raise BrainRunError("learning ledger report must contain evidence and next_state")
        if context_digest is not None and not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        self._assert_safe(report)
        try:
            encoded_report = json.dumps(
                {"learning_evidence": evidence, "next_state": next_state},
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("learning ledger report must be JSON-safe") from error
        if len(encoded_report) > self.max_bytes:
            raise BrainRunError("learning ledger record exceeds max_bytes")
        record: dict[str, Any] = {
            "learning_evidence": evidence,
            "next_state": next_state,
        }
        if context_digest is not None:
            record["context_digest"] = context_digest
        line = json.dumps(
            {"schema": self._SCHEMA, "record": record},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8") + b"\n"
        with self._lock:
            existing_size = self.path.stat().st_size if self.path.exists() else 0
            if existing_size + len(line) > self.max_bytes:
                raise BrainRunError("learning ledger capacity is exhausted")
            existing_records = self._read_records_locked()
            if len(existing_records) >= self.max_records:
                raise BrainRunError("learning ledger record capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            record_digest = hashlib.sha256(line.rstrip(b"\n")).hexdigest()
            return {
                "schema": self._SCHEMA,
                "record_index": len(existing_records),
                "record_digest": record_digest,
                "evidence_digest": evidence.get("evidence_digest"),
            }

    def records(self) -> list[dict[str, Any]]:
        with self._lock:
            return self._read_records_locked()

    def latest_state(self, context_digest: str | None = None) -> dict[str, Any] | None:
        if context_digest is not None and not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        for row in reversed(self.records()):
            record = row.get("record")
            if not isinstance(record, Mapping):
                continue
            if context_digest is not None and record.get("context_digest") != context_digest:
                continue
            state = record.get("next_state")
            if isinstance(state, Mapping):
                return dict(state)
        return None

    def _read_records_locked(self) -> list[dict[str, Any]]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise BrainRunError("learning ledger exceeds max_bytes")
        rows: list[dict[str, Any]] = []
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_records:
                    raise BrainRunError("learning ledger exceeds max_records")
                try:
                    row = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise BrainRunError("learning ledger contains invalid JSON") from error
                if not isinstance(row, Mapping) or row.get("schema") != self._SCHEMA:
                    raise BrainRunError("learning ledger contains an invalid schema")
                rows.append(dict(row))
        return rows

    @classmethod
    def _assert_safe(cls, value: Any) -> None:
        if isinstance(value, Mapping):
            for key, child in value.items():
                if isinstance(key, str) and key.lower() in cls._FORBIDDEN_FIELDS:
                    raise BrainRunError("learning evidence contains a forbidden secret field")
                cls._assert_safe(child)
        elif isinstance(value, list):
            for child in value:
                cls._assert_safe(child)


class BrainWorkspace(Protocol):
    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]: ...


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(
        character in "0123456789abcdef" for character in value
    )


@dataclass(frozen=True, slots=True)
class BrainRunResult:
    run_id: str
    status: str
    selection: Mapping[str, Any]
    prompt: Mapping[str, Any]
    plan: Mapping[str, Any]
    response: ProviderResponse | None
    outcome_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "run_id": self.run_id,
            "status": self.status,
            "selection": dict(self.selection),
            "prompt": dict(self.prompt),
            "plan": dict(self.plan),
            "response": None if self.response is None else self.response.to_dict(),
            "outcome_digest": self.outcome_digest,
            "credential_posture": "handle_only_not_serialized",
            "execution": "provider_call_only",
            "tool_execution": "not_started",
        }


@dataclass(frozen=True, slots=True)
class BrainToolLoopResult:
    """Brain-level envelope for a provider continuation loop.

    The first decision is still planned and approved through the brain kernel. Subsequent native
    tool turns are represented by the runtime's bounded loop; caller code remains the sole effect
    authority through its authorization callback.
    """

    brain_run: BrainRunResult
    status: str
    provider_loop: ProviderToolLoopResult | None
    route: Mapping[str, Any] | None = None
    authorization_receipts: tuple[Mapping[str, Any], ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "brain_run": self.brain_run.to_dict(),
            "provider_loop": None if self.provider_loop is None else self.provider_loop.to_dict(),
            "route": None if self.route is None else dict(self.route),
            "authorization_receipts": [dict(receipt) for receipt in self.authorization_receipts],
            "authorization": {
                "provider_call": "caller_approved_brain_plan",
                "tool_execution": "caller_callback_only",
            },
        }


@dataclass(frozen=True, slots=True)
class BrainMissionResult:
    """The outcome of proposing and optionally executing one model-authored mission.

    ``preflight`` is always the non-executing server response. ``execution`` is present only
    after the caller explicitly authorizes mission dispatch. The normalized mission carries the
    caller's policy, not a policy selected by the model.
    """

    brain_run: BrainRunResult
    status: str
    mission: Mapping[str, Any] | None
    preflight: Mapping[str, Any] | None
    execution: Mapping[str, Any] | None
    route: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "brain_run": self.brain_run.to_dict(),
            "mission": None if self.mission is None else dict(self.mission),
            "preflight": None if self.preflight is None else dict(self.preflight),
            "execution": None if self.execution is None else dict(self.execution),
            "route": None if self.route is None else dict(self.route),
            "authorization": {
                "provider_call": "recorded_in_brain_run",
                "mission_dispatch": "caller_approved_only",
            },
            "tool_execution": "bounded_agent_mission_executor",
        }


def _mission_tool_identifier(value: Any) -> bool:
    return isinstance(value, str) and bool(value) and all(
        character.isalnum() or character == "_" for character in value
    )


def _json_digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _mission_wire_output(value: Any) -> Any:
    """Extract structured tool output while dropping opaque wire envelopes."""

    if not isinstance(value, Mapping):
        return None
    result = value.get("result")
    if not isinstance(result, Mapping):
        return None
    structured = result.get("structuredContent")
    if structured is not None:
        return structured
    content = result.get("content")
    if isinstance(content, list) and content:
        first = content[0]
        if isinstance(first, Mapping) and isinstance(first.get("text"), str):
            try:
                return json.loads(first["text"])
            except (TypeError, ValueError):
                return None
    return None


def _bounded_mission_report_projection(
    report: Mapping[str, Any],
    *,
    include_outputs: bool,
) -> dict[str, Any]:
    """Project an agent_mission report for continuation without replaying opaque envelopes."""

    if not isinstance(report, Mapping):
        raise BrainRunError("agent_mission returned a non-object report")
    if report.get("workflow") not in (None, "agent_mission"):
        raise BrainRunError("agent_mission returned the wrong workflow")
    projection: dict[str, Any] = {
        "workflow": "agent_mission",
        "ok": report.get("ok", True),
        "execution": report.get("execution", "unknown"),
        "mission_status": report.get("mission_status", "unknown"),
        "dispatch": report.get("dispatch", "unknown"),
        "preflight": report.get("preflight", False),
        "plan_digest": None,
        "succeeded": report.get("succeeded", 0),
        "refused": report.get("refused", 0),
        "blocked": report.get("blocked", 0),
        "cancelled": report.get("cancelled", 0),
        "required_failures": report.get("required_failures", 0),
        "returned_bytes": report.get("returned_bytes", 0),
        "results": [],
        "result_digest": _json_digest(dict(report)),
        "retention": "structured_step_outputs_only",
    }
    plan = report.get("plan")
    if isinstance(plan, Mapping):
        projection["plan_digest"] = plan.get("digest") or plan.get("plan_digest")
        projection["mission_id"] = plan.get("mission_id")
    raw_results = report.get("results", [])
    if isinstance(raw_results, list):
        for raw in raw_results[:MAX_MISSION_AUTHORIZATION_CALLS]:
            if not isinstance(raw, Mapping):
                continue
            row: dict[str, Any] = {
                "id": raw.get("id"),
                "tool": raw.get("tool"),
                "status": raw.get("status"),
                "required": raw.get("required"),
                "arguments_digest": raw.get("arguments_digest"),
                "bytes": raw.get("bytes", 0),
            }
            if raw.get("error") is not None:
                row["error_digest"] = _json_digest({"error": raw.get("error")})
            if include_outputs:
                output = _mission_wire_output(raw.get("wire"))
                if output is not None:
                    encoded_output = json.dumps(
                        output,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                        allow_nan=False,
                    ).encode("utf-8")
                    if len(encoded_output) <= MAX_MISSION_AUTHORIZATION_STEP_OUTPUT_BYTES:
                        row["output"] = output
                    else:
                        row["output_digest"] = hashlib.sha256(encoded_output).hexdigest()
                elif raw.get("wire") is not None:
                    row["output_digest"] = _json_digest(raw.get("wire"))
            projection["results"].append(row)
    BrainLearningLedger._assert_safe(projection)
    encoded = json.dumps(
        projection,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    if len(encoded) > MAX_MISSION_AUTHORIZATION_RESULT_BYTES:
        raise BrainRunError("agent_mission continuation result exceeds the bounded size")
    return projection


@dataclass(frozen=True, slots=True)
class MissionAuthorizationReceipt:
    """One caller-owned tool-intent authorization attempt and its bounded evidence."""

    mission_id: str
    call_ids: tuple[str, ...]
    status: str
    preflight: Mapping[str, Any]
    execution: Mapping[str, Any] | None
    result: Mapping[str, Any]

    def __post_init__(self) -> None:
        if self.status not in {
            "preflight_refused",
            "approval_required",
            "executed",
            "execution_refused",
            "execution_failed",
        }:
            raise BrainRunError("mission authorization receipt has an invalid status")
        if not self.mission_id or not self.call_ids:
            raise BrainRunError("mission authorization receipt is missing identity")

    def to_dict(self) -> dict[str, Any]:
        return {
            "mission_id": self.mission_id,
            "call_ids": list(self.call_ids),
            "status": self.status,
            "preflight": dict(self.preflight),
            "execution": None if self.execution is None else dict(self.execution),
            "result": dict(self.result),
            "authorization": "caller_owned",
        }


class MissionToolAuthorizer:
    """Route, preflight, and optionally dispatch native provider tool intents.

    The object is intentionally a callable so it can be passed directly to
    :meth:`LLMRuntime.invoke_tool_loop` or :meth:`AutonomousBrain.run_tool_loop`. It never treats
    a route recommendation as permission: every call must pass the caller policy, the route
    candidate set, the local route schema when available, and the authoritative ``agent_mission``
    preflight. Dispatch remains disabled unless ``approve_mission_dispatch`` is true.
    """

    def __init__(
        self,
        workspace: BrainWorkspace,
        *,
        task: str,
        mission_policy: MissionPolicy | Mapping[str, Any],
        route: Mapping[str, Any] | None = None,
        approve_mission_dispatch: bool = False,
        mission_id_prefix: str = "brain-tool",
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
    ) -> None:
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("mission authorizer task must be non-empty")
        if not hasattr(workspace, "tool") or not callable(getattr(workspace, "tool")):
            raise BrainRunError("mission authorizer requires a workspace tool boundary")
        if not isinstance(mission_policy, (MissionPolicy, Mapping)):
            raise BrainRunError("mission authorizer policy must be a MissionPolicy or mapping")
        normalized_policy = (
            mission_policy.to_dict()
            if isinstance(mission_policy, MissionPolicy)
            else dict(mission_policy)
        )
        allowed = normalized_policy.get("allowed_tools")
        if not isinstance(allowed, Sequence) or isinstance(allowed, (str, bytes)) or not allowed:
            raise BrainRunError("mission authorizer requires an explicit allowed_tools policy")
        if any(not _mission_tool_identifier(tool) for tool in allowed):
            raise BrainRunError("mission authorizer policy contains an unsafe tool name")
        BrainLearningLedger._assert_safe(normalized_policy)
        if not isinstance(approve_mission_dispatch, bool):
            raise BrainRunError("approve_mission_dispatch must be a boolean")
        if not isinstance(claim_requests, Sequence) or isinstance(claim_requests, (str, bytes)):
            raise BrainRunError("mission authorizer claim_requests must be a sequence")
        if any(not isinstance(value, Mapping) for value in claim_requests):
            raise BrainRunError("mission authorizer claim_requests must contain mappings")
        if evaluator_review is not None and not isinstance(evaluator_review, Mapping):
            raise BrainRunError("mission authorizer evaluator_review must be a mapping")
        if workflow_binding is not None and not isinstance(workflow_binding, Mapping):
            raise BrainRunError("mission authorizer workflow_binding must be a mapping")
        if operations_gate_acceptance is not None and not isinstance(operations_gate_acceptance, Mapping):
            raise BrainRunError("mission authorizer operations_gate_acceptance must be a mapping")
        BrainLearningLedger._assert_safe(
            {
                "claim_requests": list(claim_requests),
                "evaluator_review": evaluator_review,
                "workflow_binding": workflow_binding,
                "operations_gate_acceptance": operations_gate_acceptance,
            }
        )
        self.workspace = workspace
        self.task = task
        self.policy = normalized_policy
        self.policy["execute"] = False
        self.route = None if route is None else dict(route)
        self.approve_mission_dispatch = approve_mission_dispatch
        self.mission_id_prefix = mission_id_prefix
        self.claim_requests = tuple(dict(value) for value in claim_requests)
        self.evaluator_review = None if evaluator_review is None else dict(evaluator_review)
        self.workflow_binding = None if workflow_binding is None else dict(workflow_binding)
        self.operations_gate_acceptance = (
            None if operations_gate_acceptance is None else dict(operations_gate_acceptance)
        )
        self._receipts: list[MissionAuthorizationReceipt] = []
        self._invocation = 0
        self._route_recommended: set[str] | None = None
        self._route_candidates: dict[str, tuple[str, ...]] = {}
        self._route_metadata: dict[str, tuple[str, str, str]] = {}
        self._schema_catalogue: ToolCatalogue | None = None
        if self.route is not None:
            self._configure_route(self.route)

    @property
    def receipts(self) -> tuple[MissionAuthorizationReceipt, ...]:
        return tuple(self._receipts)

    def __call__(
        self,
        calls: tuple[ProviderToolCall, ...],
    ) -> tuple[ProviderToolResult, ...]:
        if not isinstance(calls, tuple) or not calls or len(calls) > MAX_MISSION_AUTHORIZATION_CALLS:
            raise BrainRunError("mission authorizer received an invalid tool-call batch")
        if any(not isinstance(call, ProviderToolCall) for call in calls):
            raise BrainRunError("mission authorizer received malformed tool calls")
        self._invocation += 1
        call_ids = tuple(call.call_id for call in calls)
        if len(set(call_ids)) != len(call_ids):
            return self._refuse(calls, "duplicate provider tool call ids")
        validation_error = self._validate_calls(calls)
        if validation_error is not None:
            return self._refuse(calls, validation_error)
        mission_id = self._mission_id(calls)
        steps = []
        for index, call in enumerate(calls):
            domain, capability, objective = self._route_metadata.get(
                call.name,
                ("cross_domain", call.name, f"Execute caller-authorized tool intent {call.name}"),
            )
            steps.append(
                {
                    "id": f"provider-tool-{self._invocation}-{index}",
                    "domain": domain,
                    "capability": capability,
                    "objective": objective,
                    "tool": call.name,
                    "arguments": dict(call.arguments),
                    "required": True,
                    "depends_on": [],
                    "bindings": [],
                }
            )
        request = MissionRequest(
            mission_id=mission_id,
            goal=self.task,
            steps=steps,
            policy=dict(self.policy),
            claim_requests=self.claim_requests,
            evaluator_review=self.evaluator_review,
            workflow_binding=self.workflow_binding,
            operations_gate_acceptance=self.operations_gate_acceptance,
        )
        try:
            preflight_raw = self.workspace.tool("agent_mission", request.to_mcp_arguments())
            preflight = _bounded_mission_report_projection(preflight_raw, include_outputs=False)
        except Exception as error:
            return self._refuse(calls, "mission preflight transport or validation failed", mission_id=mission_id)
        if not self._preflight_ready(preflight_raw):
            self._record_receipt(
                mission_id,
                calls,
                "preflight_refused",
                preflight,
                None,
                preflight,
            )
            return tuple(
                ProviderToolResult(call.call_id, preflight, approved=False, is_error=True)
                for call in calls
            )
        if not self.approve_mission_dispatch:
            self._record_receipt(
                mission_id,
                calls,
                "approval_required",
                preflight,
                None,
                preflight,
            )
            return tuple(
                ProviderToolResult(call.call_id, preflight, approved=False, is_error=True)
                for call in calls
            )
        execute_policy = dict(self.policy)
        execute_policy["execute"] = True
        execute_request = MissionRequest(
            mission_id=mission_id,
            goal=self.task,
            steps=steps,
            policy=execute_policy,
            claim_requests=self.claim_requests,
            evaluator_review=self.evaluator_review,
            workflow_binding=self.workflow_binding,
            operations_gate_acceptance=self.operations_gate_acceptance,
        )
        try:
            execution_raw = self.workspace.tool("agent_mission", execute_request.to_mcp_arguments())
            execution = _bounded_mission_report_projection(execution_raw, include_outputs=True)
        except Exception:
            execution = {
                "workflow": "agent_mission",
                "ok": False,
                "execution": "refused",
                "mission_status": "failed",
                "result_digest": _json_digest({"mission_id": mission_id, "status": "transport_failed"}),
                "retention": "structured_step_outputs_only",
            }
            self._record_receipt(
                mission_id,
                calls,
                "execution_failed",
                preflight,
                execution,
                execution,
            )
            return tuple(
                ProviderToolResult(call.call_id, execution, approved=False, is_error=True)
                for call in calls
            )
        mission_status = execution.get("mission_status")
        status = "executed" if mission_status == "succeeded" else (
            "execution_refused" if mission_status in {"refused", "blocked", "cancelled"} else "execution_failed"
        )
        self._record_receipt(mission_id, calls, status, preflight, execution, execution)
        return tuple(
            ProviderToolResult(
                call.call_id,
                execution,
                approved=True,
                is_error=status != "executed",
            )
            for call in calls
        )

    def _configure_route(self, route: Mapping[str, Any]) -> None:
        if route.get("workflow") != "capability_route":
            raise BrainRunError("mission authorizer route must be a capability_route report")
        if route.get("goal") != self.task:
            raise BrainRunError("mission authorizer route goal must match the task")
        unresolved = route.get("unresolved_needs", [])
        if not isinstance(unresolved, list) or unresolved:
            raise BrainRunError("mission authorizer route contains unresolved needs")
        recommended = route.get("recommended_tools")
        needs = route.get("needs")
        if not isinstance(recommended, list) or any(not isinstance(tool, str) for tool in recommended):
            raise BrainRunError("mission authorizer route has malformed recommended_tools")
        if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
            raise BrainRunError("mission authorizer route has malformed needs")
        self._route_recommended = set(recommended)
        for need in needs:
            need_id = need.get("id")
            candidate_tools = need.get("candidate_tools", [])
            if not isinstance(need_id, str) or not isinstance(candidate_tools, list):
                raise BrainRunError("mission authorizer route need is malformed")
            domains = need.get("candidate_domains", [])
            groups = need.get("candidate_groups", [])
            domain = domains[0] if isinstance(domains, list) and domains and isinstance(domains[0], str) else "cross_domain"
            capability = groups[0] if isinstance(groups, list) and groups and isinstance(groups[0], str) else need_id
            objective = need.get("query") if isinstance(need.get("query"), str) else f"Resolve routed need {need_id}"
            self._route_metadata.update(
                {tool: (domain, capability, objective) for tool in candidate_tools if isinstance(tool, str)}
            )
        raw_schemas = route.get("tool_schemas", [])
        omitted = route.get("tool_schemas_omitted", 0)
        if isinstance(raw_schemas, list) and raw_schemas and omitted == 0:
            try:
                self._schema_catalogue = ToolCatalogue.from_definitions(raw_schemas)
            except (ArgumentError, ToolSchemaError, TypeError, ValueError) as error:
                raise BrainRunError("mission authorizer route schemas are invalid") from error

    def _validate_calls(self, calls: Sequence[ProviderToolCall]) -> str | None:
        allowed = set(self.policy["allowed_tools"])
        for call in calls:
            if not _mission_tool_identifier(call.name):
                return f"tool {call.name!r} is not an executable mission tool identifier"
            if call.name not in allowed:
                return f"tool {call.name!r} is not in the caller mission policy"
            if self._route_recommended is not None and call.name not in self._route_recommended:
                return f"tool {call.name!r} is not recommended by the live route"
            if self._route_recommended is not None and call.name not in self._route_metadata:
                return f"tool {call.name!r} is not attached to a resolved route need"
            if self._schema_catalogue is not None:
                try:
                    report = self._schema_catalogue.validate(call.name, call.arguments)
                except ToolSchemaError:
                    return f"tool {call.name!r} is absent from the retained route schema set"
                if not report.ok:
                    return f"tool {call.name!r} failed route schema preflight"
        return None

    def _mission_id(self, calls: Sequence[ProviderToolCall]) -> str:
        digest = _json_digest([call.to_dict() for call in calls])[:32]
        prefix = self.mission_id_prefix if _mission_tool_identifier(self.mission_id_prefix) else "brain_tool"
        return f"{prefix}-{self._invocation}-{digest}"

    def _preflight_ready(self, report: Mapping[str, Any]) -> bool:
        if not isinstance(report, Mapping) or report.get("ok") is False:
            return False
        if report.get("workflow") not in (None, "agent_mission"):
            return False
        if report.get("dispatch") in {"executed", "started"}:
            return False
        return report.get("mission_status") in {None, "planned", "succeeded"} or report.get("execution") == "planned"

    def _record_receipt(
        self,
        mission_id: str,
        calls: Sequence[ProviderToolCall],
        status: str,
        preflight: Mapping[str, Any],
        execution: Mapping[str, Any] | None,
        result: Mapping[str, Any],
    ) -> None:
        self._receipts.append(
            MissionAuthorizationReceipt(
                mission_id=mission_id,
                call_ids=tuple(call.call_id for call in calls),
                status=status,
                preflight=preflight,
                execution=execution,
                result=result,
            )
        )

    def _refuse(
        self,
        calls: Sequence[ProviderToolCall],
        reason: str,
        *,
        mission_id: str | None = None,
    ) -> tuple[ProviderToolResult, ...]:
        projection = {
            "workflow": "agent_mission",
            "ok": False,
            "execution": "not_started",
            "mission_status": "refused",
            "refusal": reason,
            "result_digest": _json_digest({"reason": reason}),
            "retention": "structured_step_outputs_only",
        }
        self._receipts.append(
            MissionAuthorizationReceipt(
                mission_id=mission_id or f"{self.mission_id_prefix}-refused-{self._invocation}",
                call_ids=tuple(call.call_id for call in calls),
                status="preflight_refused",
                preflight=projection,
                execution=None,
                result=projection,
            )
        )
        return tuple(
            ProviderToolResult(call.call_id, projection, approved=False, is_error=True)
            for call in calls
        )


class AutonomousBrain:
    """Coordinate the value-only Rust kernel with a real caller-approved provider invocation."""

    def __init__(self, workspace: BrainWorkspace, runtime: LLMRuntime) -> None:
        self.workspace = workspace
        self.runtime = runtime

    def run(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 1024,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
    ) -> BrainRunResult:
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if not isinstance(tools, Sequence) or isinstance(tools, (str, bytes)):
            raise BrainRunError("tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in tools):
            raise BrainRunError("tools must contain ProviderTool values")
        resolved_run_id = run_id or f"brain-{uuid.uuid4().hex}"
        if not isinstance(resolved_run_id, str) or not resolved_run_id.strip() or len(resolved_run_id) > 256:
            raise BrainRunError("run_id must be a bounded non-empty string")
        selection_args = dict(model_selection)
        selection_args["task"] = task
        if context is None:
            if contextual_observations:
                raise BrainRunError("contextual_observations require a context mapping")
            selection = self.workspace.tool("brain_model_select", selection_args)
        else:
            if not isinstance(context, Mapping):
                raise BrainRunError("context must be a mapping")
            BrainLearningLedger._assert_safe(context)
            if not isinstance(contextual_observations, Sequence) or isinstance(
                contextual_observations, (str, bytes)
            ):
                raise BrainRunError("contextual_observations must be a sequence")
            if any(not isinstance(observation, Mapping) for observation in contextual_observations):
                raise BrainRunError("contextual_observations must contain mappings")
            BrainLearningLedger._assert_safe(list(contextual_observations))
            contextual_report = self.workspace.tool(
                "brain_model_select_contextual",
                {
                    "context": dict(context),
                    "base": selection_args,
                    "observations": [dict(observation) for observation in contextual_observations],
                },
            )
            nested_selection = contextual_report.get("selection")
            if not isinstance(nested_selection, Mapping):
                raise BrainRunError("contextual model selection did not produce a selection report")
            context_digest = contextual_report.get("context_digest")
            if not _valid_digest(context_digest):
                raise BrainRunError("contextual model selection returned an invalid context digest")
            selection = dict(nested_selection)
            selection["context_digest"] = context_digest
            selection["contextual_selection_status"] = contextual_report.get("selection_status")
        selected = selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("model selection did not produce an eligible model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not provider or not isinstance(model, str) or not model:
            raise BrainRunError("model selection returned malformed provider/model metadata")

        prompt_args = dict(prompt)
        prompt_args["task"] = task
        prompt_report = self.workspace.tool("brain_prompt_assemble", prompt_args)
        messages = prompt_report.get("messages")
        if not isinstance(messages, list) or not messages:
            raise BrainRunError("prompt assembly did not produce messages")

        plan_args = dict(plan)
        plan_args.setdefault("objective", task)
        plan_report = self.workspace.tool("brain_plan", plan_args)
        if not plan_report.get("ok", False):
            return self._result(resolved_run_id, "plan_refused", selection, prompt_report, plan_report, None)
        planned = plan_report.get("plan")
        if not isinstance(planned, Mapping):
            raise BrainRunError("brain plan reported success without a plan")
        if planned.get("requires_approval", False) and not approve_provider_call:
            return self._result(resolved_run_id, "approval_required", selection, prompt_report, plan_report, None)
        if not approve_provider_call and any(
            isinstance(step, Mapping) and step.get("effect") == "provider_call"
            for step in planned.get("steps", [])
        ):
            return self._result(resolved_run_id, "approval_required", selection, prompt_report, plan_report, None)

        handle = credentials.get(provider)
        if handle is None:
            raise BrainRunError(f"no user credential handle was supplied for provider {provider!r}")
        provider_messages = tuple(
            {"role": message["role"], "content": message["content"]}
            for message in messages
            if isinstance(message, Mapping)
            and isinstance(message.get("role"), str)
            and isinstance(message.get("content"), str)
        )
        if len(provider_messages) != len(messages):
            raise BrainRunError("prompt assembly returned malformed provider messages")
        request = ProviderRequest(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            tools=tuple(tools),
            tool_choice=tool_choice,
        )
        response = self.runtime.invoke(provider, request, credential=handle)
        return self._result(resolved_run_id, "completed_provider_call", selection, prompt_report, plan_report, response)

    def run_tool_loop(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        authorize_and_execute: Callable[[tuple[ProviderToolCall, ...]], Sequence[ProviderToolResult]] | None = None,
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2048,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_turns: int = 4,
        max_tool_calls: int = 128,
        stream: bool = False,
        mission_policy: MissionPolicy | Mapping[str, Any] | None = None,
        approve_mission_dispatch: bool = False,
        route_request: Mapping[str, Any] | None = None,
        enforce_route_tools: bool = True,
        require_resolved_route: bool = True,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
    ) -> BrainToolLoopResult:
        """Run the planned provider call and continue only through caller-approved tool results.

        This method is the high-level bridge for applications that want native function calling
        without converting every turn into a mission. The initial model decision still passes
        through ``brain_plan`` and provider approval. The callback is intentionally typed and
        explicit: it may invoke a caller-owned mission/executor, but the brain and provider
        runtime never do so implicitly.
        """

        if authorize_and_execute is not None and not callable(authorize_and_execute):
            raise BrainRunError("authorize_and_execute must be callable")
        if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
            raise BrainRunError("provider_tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in provider_tools):
            raise BrainRunError("provider_tools must contain ProviderTool values")
        if not isinstance(stream, bool):
            raise BrainRunError("stream must be a boolean")
        if not isinstance(enforce_route_tools, bool) or not isinstance(require_resolved_route, bool):
            raise BrainRunError("route enforcement flags must be booleans")
        prompt_request = dict(prompt)
        route: dict[str, Any] | None = None
        raw_route: dict[str, Any] | None = None
        if route_request is not None:
            if not isinstance(route_request, Mapping):
                raise BrainRunError("route_request must be a mapping")
            BrainLearningLedger._assert_safe(route_request)
            route_arguments = dict(route_request)
            supplied_goal = route_arguments.get("goal")
            if supplied_goal is not None and supplied_goal != task:
                raise BrainRunError("route_request.goal must match the tool-loop task")
            route_arguments["goal"] = task
            route_arguments.setdefault("needs", [{"id": "task", "query": task}])
            route_arguments.setdefault("include_tools", True)
            route_arguments.setdefault("max_tools", 128)
            try:
                encoded_route_request = json.dumps(
                    route_arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("route_request must be JSON-safe") from error
            if len(encoded_route_request) > MAX_ROUTE_REQUEST_BYTES:
                raise BrainRunError("route_request exceeds the bounded size")
            route_response = self.workspace.tool("capability_route", route_arguments)
            if not isinstance(route_response, Mapping):
                raise BrainRunError("capability route returned a non-object")
            if route_response.get("ok") is False or route_response.get("workflow") != "capability_route":
                raise BrainRunError("capability route was refused")
            raw_route = dict(route_response)
            BrainLearningLedger._assert_safe(raw_route)
            unresolved = raw_route.get("unresolved_needs", [])
            if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
                raise BrainRunError("capability route returned malformed unresolved_needs")
            if unresolved and require_resolved_route:
                raise BrainRunError("capability route contains unresolved needs: " + ", ".join(unresolved))
            route_context = _bounded_route_prompt_context(raw_route)
            route = dict(route_context)
            route.update(
                {
                    "ok": True,
                    "workflow": "capability_route",
                    "evidence_digest": raw_route.get("evidence_digest"),
                    "unresolved_needs": list(unresolved),
                    "route_coverage": raw_route.get("route_coverage", {}),
                    "execution": raw_route.get("execution", "not_started"),
                }
            )
            existing_context = prompt_request.get("context", [])
            if not isinstance(existing_context, Sequence) or isinstance(existing_context, (str, bytes)):
                raise BrainRunError("prompt.context must be a sequence when routing is enabled")
            context_chunks = [dict(chunk) for chunk in existing_context if isinstance(chunk, Mapping)]
            if len(context_chunks) != len(existing_context):
                raise BrainRunError("prompt.context must contain mappings")
            if any(chunk.get("id") == "capability-route" for chunk in context_chunks):
                raise BrainRunError("prompt.context already contains the reserved capability-route id")
            context_chunks.append(
                {
                    "id": "capability-route",
                    "role": "developer",
                    "content": json.dumps(route_context, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                    "required": True,
                    "priority": 1_000,
                }
            )
            prompt_request["context"] = context_chunks
            if not provider_tools and route_context["tool_schemas"] and not route_context["tool_schemas_omitted"]:
                provider_tools = tuple(
                    ProviderTool.from_mcp_schema(schema) for schema in route_context["tool_schemas"]
                )
            if enforce_route_tools:
                if mission_policy is None:
                    raise BrainRunError("enforce_route_tools requires mission_policy")
                policy_for_route = (
                    mission_policy.to_dict() if isinstance(mission_policy, MissionPolicy) else dict(mission_policy)
                )
                allowed_tools = policy_for_route.get("allowed_tools")
                recommended_tools = route.get("recommended_tools")
                if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                    raise BrainRunError("enforce_route_tools requires an explicit mission policy allowed_tools list")
                if not isinstance(recommended_tools, list) or any(not isinstance(tool, str) for tool in recommended_tools):
                    raise BrainRunError("capability route returned malformed recommended_tools")
                narrowed = [tool for tool in allowed_tools if tool in set(recommended_tools)]
                if not narrowed:
                    raise BrainRunError("route has no overlap with the caller mission policy allowed_tools")
                policy_for_route["allowed_tools"] = narrowed
                mission_policy = policy_for_route
        if authorize_and_execute is None:
            if mission_policy is None:
                raise BrainRunError("provide authorize_and_execute or mission_policy for the built-in mission authorizer")
            if not provider_tools:
                raise BrainRunError("the built-in mission authorizer requires provider_tools")
            authorizer = MissionToolAuthorizer(
                self.workspace,
                task=task,
                mission_policy=mission_policy,
                route=raw_route,
                approve_mission_dispatch=approve_mission_dispatch,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
                operations_gate_acceptance=operations_gate_acceptance,
            )
            authorize_and_execute = authorizer
        else:
            authorizer = None
        first = self.run(
            task=task,
            model_selection=model_selection,
            prompt=prompt_request,
            plan=plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            context=context,
            contextual_observations=contextual_observations,
            tools=provider_tools,
            tool_choice=tool_choice,
        )
        if first.status != "completed_provider_call" or first.response is None:
            return BrainToolLoopResult(brain_run=first, status=first.status, provider_loop=None, route=route)
        selected = first.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("model selection did not produce a continuation model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("continuation model metadata is malformed")
        prompt_messages = first.prompt.get("messages")
        if not isinstance(prompt_messages, list) or not prompt_messages:
            raise BrainRunError("brain prompt did not retain bounded provider messages")
        provider_messages = tuple(
            {"role": message["role"], "content": message["content"]}
            for message in prompt_messages
            if isinstance(message, Mapping)
            and isinstance(message.get("role"), str)
            and isinstance(message.get("content"), str)
        )
        if len(provider_messages) != len(prompt_messages):
            raise BrainRunError("brain prompt returned malformed continuation messages")
        handle = credentials.get(provider)
        if handle is None:
            raise BrainRunError(f"no user credential handle was supplied for provider {provider!r}")
        request = ProviderRequest(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            tools=tuple(provider_tools),
            tool_choice=tool_choice,
        )
        loop = self.runtime.invoke_tool_loop(
            provider,
            request,
            credential=handle,
            authorize_and_execute=authorize_and_execute,
            max_turns=max_turns,
            max_tool_calls=max_tool_calls,
            stream=stream,
            initial_response=first.response,
        )
        status = {
            "completed": "completed_provider_tool_loop",
            "authorization_required": "tool_authorization_required",
            "turn_limit_reached": "tool_turn_limit_reached",
        }[loop.status]
        receipts = () if authorizer is None else tuple(receipt.to_dict() for receipt in authorizer.receipts)
        return BrainToolLoopResult(
            brain_run=first,
            status=status,
            provider_loop=loop,
            route=route,
            authorization_receipts=receipts,
        )

    def record_evaluator_outcome(
        self,
        result: BrainRunResult,
        *,
        bandit_state: Mapping[str, Any],
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        arm_id: str | None = None,
        failed: bool = False,
        feedback_digest: str | None = None,
        failure_class: str | None = None,
        evidence_digest: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> dict[str, Any]:
        """Submit one explicit evaluator judgment and optionally persist its value-only report."""

        selected = result.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("cannot record an outcome without selected model metadata")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("selected model metadata is malformed")
        selection_digest = result.selection.get("decision_digest")
        prompt_digest = result.prompt.get("prompt_digest")
        plan_digest = (result.plan.get("plan") or {}).get("plan_digest") if isinstance(result.plan.get("plan"), Mapping) else None
        for name, value in (("selection_digest", selection_digest), ("prompt_digest", prompt_digest), ("plan_digest", plan_digest)):
            if not isinstance(value, str) or len(value) != 64:
                raise BrainRunError(f"{name} is missing or is not a SHA-256 digest")
        report = self.workspace.tool(
            "brain_outcome_record",
            {
                "run": {
                    "run_id": result.run_id,
                    "selection_digest": selection_digest,
                    "prompt_digest": prompt_digest,
                    "plan_digest": plan_digest,
                    "provider": provider,
                    "model": model,
                    "outcome_digest": result.outcome_digest,
                    "request_id": result.response.request_id if result.response is not None else None,
                },
                "assessment": {
                    "evaluator_id": evaluator_id,
                    "evaluator_version": evaluator_version,
                    "reward": reward,
                    "passed": passed,
                    "failed": failed,
                    "feedback_digest": feedback_digest,
                    "failure_class": failure_class,
                    "evidence_digest": evidence_digest,
                },
                "bandit_state": dict(bandit_state),
                "arm_id": arm_id or f"{provider}/{model}",
            },
        )
        if not isinstance(report, Mapping) or not report.get("ok"):
            raise BrainRunError("brain outcome recording returned a refusal")
        if ledger is not None:
            context_digest = result.selection.get("context_digest")
            ledger.append(
                report,
                context_digest=context_digest if isinstance(context_digest, str) else None,
            )
        return dict(report)

    def run_mission(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2048,
        temperature: float | None = None,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        route_review: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_request: Mapping[str, Any] | None = None,
        enforce_route_tools: bool = False,
        require_resolved_route: bool = True,
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
    ) -> BrainMissionResult:
        """Run a model decision through the existing bounded mission executor.

        The model supplies only step data. The caller supplies the mission policy and therefore
        the tool allow-list, output budgets, parallelism, and side-effect posture. The server
        receives a preview with ``execute=false`` first; dispatch is a separate request after
        ``approve_mission_dispatch=True``. Claims/evaluator bindings are caller-owned metadata and
        are not accepted from the model response.
        """

        if not isinstance(mission_policy, (MissionPolicy, Mapping)):
            raise BrainRunError("mission_policy must be a MissionPolicy or mapping")
        policy = (
            mission_policy.to_dict()
            if isinstance(mission_policy, MissionPolicy)
            else dict(mission_policy)
        )
        if not isinstance(claim_requests, Sequence) or isinstance(claim_requests, (str, bytes)):
            raise BrainRunError("claim_requests must be a sequence")
        if not isinstance(enforce_route_tools, bool) or not isinstance(require_resolved_route, bool):
            raise BrainRunError("route enforcement flags must be booleans")
        if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
            raise BrainRunError("provider_tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in provider_tools):
            raise BrainRunError("provider_tools must contain ProviderTool values")

        route: dict[str, Any] | None = None
        prompt_request = dict(prompt)
        if route_request is not None:
            if not isinstance(route_request, Mapping):
                raise BrainRunError("route_request must be a mapping")
            BrainLearningLedger._assert_safe(route_request)
            route_arguments = dict(route_request)
            supplied_goal = route_arguments.get("goal")
            if supplied_goal is not None and supplied_goal != task:
                raise BrainRunError("route_request.goal must match the mission task")
            route_arguments["goal"] = task
            route_arguments.setdefault(
                "needs",
                [{"id": "task", "query": task}],
            )
            route_arguments.setdefault("include_tools", True)
            route_arguments.setdefault("max_tools", 128)
            try:
                encoded_route_request = json.dumps(
                    route_arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("route_request must be JSON-safe") from error
            if len(encoded_route_request) > MAX_ROUTE_REQUEST_BYTES:
                raise BrainRunError("route_request exceeds the bounded size")
            route_response = self.workspace.tool("capability_route", route_arguments)
            if not isinstance(route_response, Mapping):
                raise BrainRunError("capability route returned a non-object")
            if route_response.get("ok") is False or route_response.get("workflow") != "capability_route":
                raise BrainRunError("capability route was refused")
            raw_route = dict(route_response)
            BrainLearningLedger._assert_safe(raw_route)
            unresolved = raw_route.get("unresolved_needs", [])
            if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
                raise BrainRunError("capability route returned malformed unresolved_needs")
            if unresolved and require_resolved_route:
                raise BrainRunError(
                    "capability route contains unresolved needs: " + ", ".join(unresolved)
                )
            route_context = _bounded_route_prompt_context(raw_route)
            route = dict(route_context)
            route.update(
                {
                    "ok": True,
                    "workflow": "capability_route",
                    "evidence_digest": raw_route.get("evidence_digest"),
                    "unresolved_needs": list(unresolved),
                    "route_coverage": raw_route.get("route_coverage", {}),
                    "execution": raw_route.get("execution", "not_started"),
                }
            )
            existing_context = prompt_request.get("context", [])
            if not isinstance(existing_context, Sequence) or isinstance(existing_context, (str, bytes)):
                raise BrainRunError("prompt.context must be a sequence when routing is enabled")
            context_chunks = [dict(chunk) for chunk in existing_context if isinstance(chunk, Mapping)]
            if len(context_chunks) != len(existing_context):
                raise BrainRunError("prompt.context must contain mappings")
            route_chunk_id = "capability-route"
            if any(chunk.get("id") == route_chunk_id for chunk in context_chunks):
                raise BrainRunError("prompt.context already contains the reserved capability-route id")
            context_chunks.append(
                {
                    "id": route_chunk_id,
                    "role": "developer",
                    "content": json.dumps(
                        route_context,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                    ),
                    "required": True,
                    "priority": 1_000,
                }
            )
            prompt_request["context"] = context_chunks

            if not provider_tools and route_context["tool_schemas"] and not route_context["tool_schemas_omitted"]:
                provider_tools = tuple(
                    ProviderTool.from_mcp_schema(schema)
                    for schema in route_context["tool_schemas"]
                )

            if enforce_route_tools:
                recommended_tools = route.get("recommended_tools")
                if not isinstance(recommended_tools, list) or any(
                    not isinstance(tool, str) for tool in recommended_tools
                ):
                    raise BrainRunError("capability route returned malformed recommended_tools")
                recommended_set = set(recommended_tools)
                allowed_tools = policy.get("allowed_tools")
                if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                    raise BrainRunError(
                        "enforce_route_tools requires an explicit mission policy allowed_tools list"
                    )
                narrowed = [tool for tool in allowed_tools if tool in recommended_set]
                if not narrowed:
                    raise BrainRunError(
                        "route has no overlap with the caller mission policy allowed_tools"
                    )
                policy["allowed_tools"] = narrowed
        policy["execute"] = False
        brain_run = self.run(
            task=task,
            model_selection=model_selection,
            prompt=prompt_request,
            plan=plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=True,
            response_schema=response_schema or DEFAULT_MISSION_RESPONSE_SCHEMA,
            idempotency_key=idempotency_key,
            context=context,
            contextual_observations=contextual_observations,
            tools=provider_tools,
            tool_choice=tool_choice,
        )
        if brain_run.status != "completed_provider_call" or brain_run.response is None:
            return BrainMissionResult(
                brain_run=brain_run,
                status="brain_run_not_completed",
                mission=None,
                preflight=None,
                execution=None,
                route=route,
            )
        if brain_run.response.tool_calls:
            raw_steps = []
            for index, call in enumerate(brain_run.response.tool_calls):
                domain = "cross_domain"
                if route is not None:
                    for need in route.get("needs", []):
                        if not isinstance(need, Mapping):
                            continue
                        candidate_tools = need.get("candidate_tools", [])
                        if call.name in candidate_tools:
                            domains = need.get("candidate_domains", [])
                            if isinstance(domains, list) and domains and isinstance(domains[0], str):
                                domain = domains[0]
                            break
                raw_steps.append(
                    {
                        "id": f"provider-tool-{index}",
                        "domain": domain,
                        "capability": call.name,
                        "objective": f"Execute the caller-authorized provider tool intent {call.name}",
                        "tool": call.name,
                        "arguments": dict(call.arguments),
                        "required": True,
                        "depends_on": [],
                        "bindings": [],
                    }
                )
        else:
            structured = brain_run.response.structured
            if not isinstance(structured, Mapping):
                raise BrainRunError("structured brain response did not contain a JSON object")
            proposed = structured.get("mission")
            if not isinstance(proposed, Mapping):
                raise BrainRunError("structured brain response did not contain a mission object")
            raw_steps = proposed.get("steps")
            if not isinstance(raw_steps, list) or not raw_steps:
                raise BrainRunError("model mission must contain a non-empty steps array")

        mission_id = f"{brain_run.run_id}-mission"
        preview_request = MissionRequest(
            mission_id=mission_id,
            goal=task,
            steps=raw_steps,
            policy=policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
            route_review=route_review,
            operations_gate_acceptance=operations_gate_acceptance,
        )
        preview_arguments = preview_request.to_mcp_arguments()
        preflight = self.workspace.tool("agent_mission", preview_arguments)
        if not isinstance(preflight, Mapping):
            raise BrainRunError("agent mission preflight returned a non-object")
        if preflight.get("workflow") not in (None, "agent_mission"):
            raise BrainRunError("agent mission preflight returned the wrong workflow")
        mission = dict(preview_arguments)
        if not approve_mission_dispatch:
            return BrainMissionResult(
                brain_run=brain_run,
                status="mission_approval_required",
                mission=mission,
                preflight=dict(preflight),
                execution=None,
                route=route,
            )

        execute_policy = dict(policy)
        execute_policy["execute"] = True
        execute_request = MissionRequest(
            mission_id=mission_id,
            goal=task,
            steps=raw_steps,
            policy=execute_policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
            route_review=route_review,
            operations_gate_acceptance=operations_gate_acceptance,
        )
        execution = self.workspace.tool("agent_mission", execute_request.to_mcp_arguments())
        if not isinstance(execution, Mapping):
            raise BrainRunError("agent mission execution returned a non-object")
        return BrainMissionResult(
            brain_run=brain_run,
            status="mission_dispatched",
            mission=mission,
            preflight=dict(preflight),
            execution=dict(execution),
            route=route,
        )

    @staticmethod
    def _result(
        run_id: str,
        status: str,
        selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        response: ProviderResponse | None,
    ) -> BrainRunResult:
        digest_input = {
            "status": status,
            "selection": selection,
            "prompt_digest": prompt.get("prompt_digest"),
            "plan_digest": (plan.get("plan") or {}).get("plan_digest")
            if isinstance(plan.get("plan"), Mapping)
            else None,
            "response": None
            if response is None
            else {
                "provider": response.provider,
                "model": response.model,
                "text": response.text,
                "request_id": response.request_id,
                "usage": dict(response.usage),
            },
        }
        encoded = json.dumps(digest_input, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
        return BrainRunResult(
            run_id=run_id,
            status=status,
            selection=selection,
            prompt=prompt,
            plan=plan,
            response=response,
            outcome_digest=hashlib.sha256(encoded).hexdigest(),
        )
