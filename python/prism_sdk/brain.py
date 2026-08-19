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
from typing import Any, Mapping, Protocol, Sequence

from .llm_runtime import CredentialHandle, LLMRuntime, ProviderRequest, ProviderResponse, ProviderTool
from .mission import MissionPolicy, MissionRequest


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
