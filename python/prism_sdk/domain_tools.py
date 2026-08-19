"""Domain-aware tool contracts for the autonomous agent façade.

The provider runtime only knows how to transport a function schema and continue a conversation
after a caller-approved result.  This module supplies the application-level composition that was
previously left to each embedding application:

* tools are registered with explicit domains, capabilities, and risk posture;
* the exact input schema is reused for local preflight and provider wire generation;
* read-only tools may be automatically executed by an embedding application;
* every effectful or approval-required tool still needs a separate caller approval callback; and
* receipts retain digests and statuses, never tool arguments, outputs, credentials, or provider
  payloads.

This is deliberately an execution adapter, not a new source of scientific or operational truth.
The registered executor remains caller-owned and the authoritative MCP tool may still refuse the
request.  A model intent never becomes permission merely because it appears in a tool schema.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError
from .llm_runtime import ProviderTool, ProviderToolCall, ProviderToolResult
from .autonomy_persistence import (
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomyPolicyError,
)
from .tooling import ToolCatalogue, ToolDefinition, ToolSchemaError


DOMAIN_TOOL_SCHEMA = "bioprism-python-autonomous-domain-tool/0.1"
DOMAIN_TOOL_REGISTRY_SCHEMA = "bioprism-python-autonomous-domain-tool-registry/0.1"
DOMAIN_TOOL_RISK_CLASSES = (
    "read_only",
    "reversible_effect",
    "external_effect",
    "high_impact_effect",
)
DOMAIN_TOOL_EXECUTION_STATUSES = (
    "executed",
    "approval_required",
    "policy_refused",
    "unknown_tool",
    "schema_refused",
    "execution_failed",
)
MAX_DOMAIN_TOOLS = 512
MAX_DOMAIN_TOOL_DOMAINS = 32
MAX_DOMAIN_TOOL_DESCRIPTION_BYTES = 16_000
MAX_DOMAIN_TOOL_RESULT_BYTES = 1_000_000
MAX_DOMAIN_TOOL_CALLS = 128
_SAFE_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-"
)
_SECRET_FIELD_MARKERS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "password",
        "refreshtoken",
        "secret",
        "accesstoken",
        "token",
        "privatekey",
        "secretkey",
        "clientsecret",
    }
)


def _text(name: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return value


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, maximum=256)
    if any(character not in _SAFE_IDENTIFIER_CHARS for character in result):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return result


def _sequence(name: str, value: Any, *, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be a sequence")
    if not value or len(value) > maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        item_text = _identifier(f"{name} entry", item)
        if item_text in seen:
            raise ArgumentError(f"{name} contains a duplicate entry: {item_text}")
        seen.add(item_text)
        result.append(item_text)
    return tuple(result)


def _json_safe(name: str, value: Any, *, maximum: int) -> Any:
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return json.loads(encoded)


def _reject_secret_fields(value: Any, *, depth: int = 0) -> None:
    if depth > 32:
        raise ArgumentError("domain tool value is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if isinstance(key, str):
                normalized = "".join(character for character in key.lower() if character.isalnum())
                if normalized in _SECRET_FIELD_MARKERS:
                    raise ArgumentError("domain tool values cannot contain credential-shaped fields")
            _reject_secret_fields(child, depth=depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        for child in value:
            _reject_secret_fields(child, depth=depth + 1)


@dataclass(frozen=True, slots=True)
class AutonomousDomainTool:
    """One provider-visible tool contract with explicit domain and effect posture."""

    name: str
    domains: tuple[str, ...]
    capability: str
    description: str
    parameters: Mapping[str, Any]
    risk_class: str = "read_only"
    read_only: bool = True
    approval_required: bool = False

    def __post_init__(self) -> None:
        name = _identifier("domain tool name", self.name)
        domains = _sequence("domain tool domains", self.domains, maximum=MAX_DOMAIN_TOOL_DOMAINS)
        capability = _identifier("domain tool capability", self.capability)
        description = _text(
            "domain tool description",
            self.description,
            maximum=MAX_DOMAIN_TOOL_DESCRIPTION_BYTES,
        )
        if self.risk_class not in DOMAIN_TOOL_RISK_CLASSES:
            raise ArgumentError(
                "domain tool risk_class must be one of: " + ", ".join(DOMAIN_TOOL_RISK_CLASSES)
            )
        if not isinstance(self.read_only, bool) or not isinstance(self.approval_required, bool):
            raise ArgumentError("domain tool safety flags must be booleans")
        if self.read_only and self.risk_class != "read_only":
            raise ArgumentError("read-only tools must use risk_class=read_only")
        if not self.read_only and not self.approval_required:
            raise ArgumentError("effectful domain tools must require approval")
        if not isinstance(self.parameters, Mapping):
            raise ArgumentError("domain tool parameters must be a JSON object")
        parameters = _json_safe(
            "domain tool parameters",
            dict(self.parameters),
            maximum=256_000,
        )
        if not isinstance(parameters, dict):
            raise ArgumentError("domain tool parameters must be a JSON object")
        _reject_secret_fields(parameters)
        object.__setattr__(self, "name", name)
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "capability", capability)
        object.__setattr__(self, "description", description)
        object.__setattr__(self, "parameters", parameters)

    @classmethod
    def from_mcp_definition(
        cls,
        definition: Mapping[str, Any] | ToolDefinition,
        *,
        domains: Sequence[str],
        capability: str,
        risk_class: str = "read_only",
        read_only: bool = True,
        approval_required: bool = False,
    ) -> "AutonomousDomainTool":
        source = definition if isinstance(definition, ToolDefinition) else ToolDefinition.from_mapping(definition)
        return cls(
            name=source.name,
            domains=tuple(domains),
            capability=capability,
            description=source.description or f"Invoke the bounded {source.name} tool.",
            parameters=source.input_schema,
            risk_class=risk_class,
            read_only=read_only,
            approval_required=approval_required,
        )

    @property
    def schema_digest(self) -> str:
        return content_digest(dict(self.parameters))

    def to_provider_tool(self) -> ProviderTool:
        posture = "read-only" if self.read_only else f"{self.risk_class}; caller approval required"
        return ProviderTool(
            name=self.name,
            description=f"[{posture}] {self.description}",
            parameters=dict(self.parameters),
        )

    def to_tool_definition(self) -> ToolDefinition:
        return ToolDefinition(
            name=self.name,
            description=self.description,
            input_schema=dict(self.parameters),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_SCHEMA,
            "name": self.name,
            "domains": list(self.domains),
            "capability": self.capability,
            "description": self.description,
            "schema_digest": self.schema_digest,
            "risk_class": self.risk_class,
            "read_only": self.read_only,
            "approval_required": self.approval_required,
            "execution": "caller_owned_executor",
        }


class AutonomousDomainToolRegistry:
    """Bounded, duplicate-free registry of tools available to the autonomous brain."""

    def __init__(self, tools: Sequence[AutonomousDomainTool] = ()) -> None:
        if not isinstance(tools, Sequence) or isinstance(tools, (str, bytes)):
            raise ArgumentError("domain tools must be a sequence")
        if len(tools) > MAX_DOMAIN_TOOLS:
            raise ArgumentError(f"domain tools may contain at most {MAX_DOMAIN_TOOLS} entries")
        self._tools: dict[str, AutonomousDomainTool] = {}
        for tool in tools:
            self.register(tool)

    def register(self, tool: AutonomousDomainTool, *, replace_existing: bool = False) -> AutonomousDomainTool:
        if not isinstance(tool, AutonomousDomainTool):
            raise ArgumentError("domain tool registry entries must be AutonomousDomainTool values")
        if tool.name in self._tools and not replace_existing:
            raise ArgumentError(f"domain tool is already registered: {tool.name}")
        if len(self._tools) >= MAX_DOMAIN_TOOLS and tool.name not in self._tools:
            raise ArgumentError(f"domain tools may contain at most {MAX_DOMAIN_TOOLS} entries")
        self._tools[tool.name] = tool
        return tool

    def register_mcp_definition(
        self,
        definition: Mapping[str, Any] | ToolDefinition,
        *,
        domains: Sequence[str],
        capability: str,
        risk_class: str = "read_only",
        read_only: bool = True,
        approval_required: bool = False,
        replace_existing: bool = False,
    ) -> AutonomousDomainTool:
        return self.register(
            AutonomousDomainTool.from_mcp_definition(
                definition,
                domains=domains,
                capability=capability,
                risk_class=risk_class,
                read_only=read_only,
                approval_required=approval_required,
            ),
            replace_existing=replace_existing,
        )

    def resolve(self, name: str) -> AutonomousDomainTool:
        _identifier("domain tool name", name)
        tool = self._tools.get(name)
        if tool is None:
            raise ToolSchemaError(f"domain tool {name!r} is not registered")
        return tool

    def tools_for(
        self,
        domains: Sequence[str] | None = None,
        *,
        capabilities: Sequence[str] = (),
        include_shared: bool = True,
    ) -> tuple[AutonomousDomainTool, ...]:
        if domains is not None:
            domain_set = set(_sequence("tool selection domains", domains, maximum=MAX_DOMAIN_TOOL_DOMAINS))
        else:
            domain_set = None
        capability_set = set(_sequence("tool selection capabilities", capabilities, maximum=MAX_DOMAIN_TOOLS)) if capabilities else set()
        selected: list[AutonomousDomainTool] = []
        for tool in self._tools.values():
            if domain_set is not None:
                matches = bool(domain_set.intersection(tool.domains))
                if include_shared and "cross_domain" in tool.domains:
                    matches = True
                if not matches:
                    continue
            if capability_set and tool.capability not in capability_set:
                continue
            selected.append(tool)
        return tuple(sorted(selected, key=lambda item: item.name))

    def provider_tools(
        self,
        domains: Sequence[str] | None = None,
        *,
        capabilities: Sequence[str] = (),
    ) -> tuple[ProviderTool, ...]:
        return tuple(tool.to_provider_tool() for tool in self.tools_for(domains, capabilities=capabilities))

    def catalogue(self, domains: Sequence[str] | None = None) -> list[dict[str, Any]]:
        return [tool.to_dict() for tool in self.tools_for(domains)]

    @property
    def digest(self) -> str:
        return content_digest([tool.to_dict() for tool in self.tools_for()])

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_REGISTRY_SCHEMA,
            "digest": self.digest,
            "tool_count": len(self._tools),
            "tools": self.catalogue(),
            "execution": "metadata_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainToolReceipt:
    """Metadata-only receipt for one tool intent handled by the domain runtime."""

    call_id: str
    tool: str
    status: str
    schema_digest: str | None = None
    arguments_digest: str | None = None
    output_digest: str | None = None
    execution_id: str | None = None
    domain: str | None = None
    capability: str | None = None
    risk_class: str | None = None

    def __post_init__(self) -> None:
        _text("tool receipt call_id", self.call_id, maximum=256)
        _identifier("tool receipt tool", self.tool)
        if self.status not in DOMAIN_TOOL_EXECUTION_STATUSES:
            raise ArgumentError("tool receipt status is invalid")
        for name, value in (("schema_digest", self.schema_digest), ("arguments_digest", self.arguments_digest), ("output_digest", self.output_digest)):
            if value is not None and (not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value)):
                raise ArgumentError(f"{name} must be a lowercase SHA-256 digest or None")
        for name, value in (("execution_id", self.execution_id), ("domain", self.domain), ("capability", self.capability), ("risk_class", self.risk_class)):
            if value is not None:
                _identifier(f"tool receipt {name}", value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_SCHEMA,
            "call_id": self.call_id,
            "tool": self.tool,
            "status": self.status,
            "schema_digest": self.schema_digest,
            "arguments_digest": self.arguments_digest,
            "output_digest": self.output_digest,
            "execution_id": self.execution_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "retention": "metadata_only_no_arguments_or_outputs",
        }


class AutonomousDomainToolRuntime:
    """Approval-aware adapter from provider intents to a caller-owned tool executor."""

    def __init__(
        self,
        registry: AutonomousDomainToolRegistry,
        *,
        executor: Callable[[AutonomousDomainTool, Mapping[str, Any]], Any],
        approve: Callable[[AutonomousDomainTool, ProviderToolCall], bool] | None = None,
        auto_execute_read_only: bool = True,
        controller: AutonomousExecutionController | None = None,
        _receipt_store: list[AutonomousDomainToolReceipt] | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousDomainToolRegistry):
            raise ArgumentError("domain tool runtime requires an AutonomousDomainToolRegistry")
        if not callable(executor):
            raise ArgumentError("domain tool runtime executor must be callable")
        if approve is not None and not callable(approve):
            raise ArgumentError("domain tool runtime approval callback must be callable")
        if not isinstance(auto_execute_read_only, bool):
            raise ArgumentError("auto_execute_read_only must be a boolean")
        if controller is not None and not isinstance(controller, AutonomousExecutionController):
            raise ArgumentError("domain tool runtime controller must be an AutonomousExecutionController or None")
        if _receipt_store is not None and not isinstance(_receipt_store, list):
            raise ArgumentError("domain tool runtime receipt store must be a list or None")
        self.registry = registry
        self.executor = executor
        self.approve = approve
        self.auto_execute_read_only = auto_execute_read_only
        self.controller = controller
        self._receipts = _receipt_store if _receipt_store is not None else []

    def session(
        self,
        *,
        execution_id: str,
        domain: str,
        capability: str,
        risk_class: str,
        policy: AutonomousExecutionPolicy | Mapping[str, Any] | None = None,
        journal: AutonomousExecutionJournal | None = None,
        resume: bool = False,
    ) -> "AutonomousDomainToolRuntime":
        """Create an isolated run controller while preserving the application receipt stream."""

        controller = AutonomousExecutionController(
            execution_id=execution_id,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            policy=policy,
            journal=journal,
            resume=resume,
        )
        return AutonomousDomainToolRuntime(
            self.registry,
            executor=self.executor,
            approve=self.approve,
            auto_execute_read_only=self.auto_execute_read_only,
            controller=controller,
            _receipt_store=self._receipts,
        )

    @property
    def receipts(self) -> tuple[AutonomousDomainToolReceipt, ...]:
        return tuple(self._receipts)

    def _result(
        self,
        call: ProviderToolCall,
        *,
        status: str,
        content: Mapping[str, Any],
        approved: bool,
        is_error: bool = True,
        receipt: AutonomousDomainToolReceipt,
    ) -> ProviderToolResult:
        self._receipts.append(receipt)
        return ProviderToolResult(call.call_id, dict(content), approved=approved, is_error=is_error)

    def __call__(self, calls: tuple[ProviderToolCall, ...]) -> tuple[ProviderToolResult, ...]:
        if not isinstance(calls, tuple) or not calls or len(calls) > MAX_DOMAIN_TOOL_CALLS:
            raise ArgumentError("domain tool runtime received an invalid call batch")
        if any(not isinstance(call, ProviderToolCall) for call in calls):
            raise ArgumentError("domain tool runtime received malformed provider calls")
        if len({call.call_id for call in calls}) != len(calls):
            return tuple(
                self._result(
                    call,
                    status="schema_refused",
                    content={"status": "refused", "reason": "duplicate_call_ids", "authorization": "approval_required"},
                    approved=False,
                    receipt=AutonomousDomainToolReceipt(call.call_id, call.name, "schema_refused"),
                )
                for call in calls
            )

        prepared: list[tuple[ProviderToolCall, AutonomousDomainTool, dict[str, Any], str]] = []
        refusals: list[tuple[ProviderToolCall, str, AutonomousDomainToolReceipt]] = []
        for call in calls:
            arguments_digest = content_digest(dict(call.arguments))
            try:
                _reject_secret_fields(dict(call.arguments))
                tool = self.registry.resolve(call.name)
                plan = ToolCatalogue.from_definitions([tool.to_tool_definition()]).plan(call.name, call.arguments)
            except (ArgumentError, ToolSchemaError):
                refusals.append(
                    (
                        call,
                        "schema_refused",
                        AutonomousDomainToolReceipt(
                            call.call_id,
                            call.name,
                            "schema_refused",
                            arguments_digest=arguments_digest,
                        ),
                    )
                )
                continue
            if self.controller is not None:
                try:
                    self.controller.admit_tool_call(
                        tool=tool.name,
                        call_id=call.call_id,
                        read_only=tool.read_only,
                        approval_required=tool.approval_required,
                    )
                except AutonomyPolicyError:
                    refusals.append(
                        (
                            call,
                            "policy_refused",
                            AutonomousDomainToolReceipt(
                                call.call_id,
                                tool.name,
                                "policy_refused",
                                schema_digest=plan.schema_digest,
                                arguments_digest=arguments_digest,
                                execution_id=self.controller.state.execution_id,
                                domain=self.controller.state.domain,
                                capability=tool.capability,
                                risk_class=tool.risk_class,
                            ),
                        )
                    )
                    continue
            authorized = tool.read_only and self.auto_execute_read_only
            if not authorized and self.approve is not None:
                try:
                    authorized = bool(self.approve(tool, call))
                except Exception:
                    authorized = False
            if not authorized:
                refusals.append(
                    (
                        call,
                        "approval_required",
                        AutonomousDomainToolReceipt(
                            call.call_id,
                            tool.name,
                            "approval_required",
                            schema_digest=plan.schema_digest,
                            arguments_digest=arguments_digest,
                            execution_id=None if self.controller is None else self.controller.state.execution_id,
                            domain=None if self.controller is None else self.controller.state.domain,
                            capability=tool.capability,
                            risk_class=tool.risk_class,
                        ),
                    )
                )
                continue
            prepared.append((call, tool, plan.arguments, arguments_digest))

        if refusals:
            results: list[ProviderToolResult] = []
            for call in calls:
                matching = next((item for item in refusals if item[0].call_id == call.call_id), None)
                if matching is not None:
                    _call, status, receipt = matching
                    results.append(
                        self._result(
                            call,
                            status=status,
                            content={"status": "refused", "tool": call.name, "reason": status, "authorization": "approval_required"},
                            approved=False,
                            receipt=receipt,
                        )
                    )
                    if self.controller is not None and status == "approval_required":
                        tool = self.registry.resolve(call.name)
                        self.controller.record_tool_outcome(
                            tool=tool.name,
                            call_id=call.call_id,
                            status="approval_required",
                            reason="caller_approval_missing",
                        )
                else:
                    tool = next(item[1] for item in prepared if item[0].call_id == call.call_id)
                    results.append(
                        self._result(
                            call,
                            status="approval_required",
                            content={"status": "refused", "tool": tool.name, "reason": "batch_contains_unapproved_call", "authorization": "approval_required"},
                            approved=False,
                            receipt=AutonomousDomainToolReceipt(
                                call.call_id,
                                tool.name,
                                "approval_required",
                                schema_digest=tool.schema_digest,
                                arguments_digest=content_digest(dict(call.arguments)),
                                execution_id=None if self.controller is None else self.controller.state.execution_id,
                                domain=None if self.controller is None else self.controller.state.domain,
                                capability=tool.capability,
                                risk_class=tool.risk_class,
                            ),
                        )
                    )
                    if self.controller is not None:
                        self.controller.record_tool_outcome(
                            tool=tool.name,
                            call_id=call.call_id,
                            status="approval_required",
                            reason="batch_contains_unapproved_call",
                        )
            return tuple(results)

        results: list[ProviderToolResult] = []
        for call, tool, arguments, arguments_digest in prepared:
            try:
                output = self.executor(tool, arguments)
                _json_safe("domain tool result", output, maximum=MAX_DOMAIN_TOOL_RESULT_BYTES)
                _reject_secret_fields(output)
                results.append(
                    self._result(
                        call,
                        status="executed",
                        content=output if isinstance(output, Mapping) else {"result": output},
                        approved=True,
                        is_error=False,
                        receipt=AutonomousDomainToolReceipt(
                            call.call_id,
                            tool.name,
                            "executed",
                            schema_digest=tool.schema_digest,
                            arguments_digest=arguments_digest,
                            output_digest=content_digest(output if isinstance(output, Mapping) else {"result": output}),
                            execution_id=None if self.controller is None else self.controller.state.execution_id,
                            domain=None if self.controller is None else self.controller.state.domain,
                            capability=tool.capability,
                            risk_class=tool.risk_class,
                        ),
                    )
                )
                if self.controller is not None:
                    self.controller.record_tool_outcome(
                        tool=tool.name,
                        call_id=call.call_id,
                        status="executed",
                        outcome_digest=content_digest(output if isinstance(output, Mapping) else {"result": output}),
                    )
            except Exception:
                results.append(
                    self._result(
                        call,
                        status="execution_failed",
                        content={"status": "execution_failed", "tool": tool.name, "authorization": "caller_approved"},
                        approved=True,
                        receipt=AutonomousDomainToolReceipt(
                            call.call_id,
                            tool.name,
                            "execution_failed",
                            schema_digest=tool.schema_digest,
                            arguments_digest=arguments_digest,
                            execution_id=None if self.controller is None else self.controller.state.execution_id,
                            domain=None if self.controller is None else self.controller.state.domain,
                            capability=tool.capability,
                            risk_class=tool.risk_class,
                        ),
                    )
                )
                if self.controller is not None:
                    self.controller.record_tool_outcome(
                        tool=tool.name,
                        call_id=call.call_id,
                        status="execution_failed",
                        reason="executor_error",
                    )
        return tuple(results)


__all__ = [
    "DOMAIN_TOOL_EXECUTION_STATUSES",
    "DOMAIN_TOOL_REGISTRY_SCHEMA",
    "DOMAIN_TOOL_RISK_CLASSES",
    "DOMAIN_TOOL_SCHEMA",
    "MAX_DOMAIN_TOOL_CALLS",
    "MAX_DOMAIN_TOOLS",
    "AutonomousDomainTool",
    "AutonomousDomainToolReceipt",
    "AutonomousDomainToolRegistry",
    "AutonomousDomainToolRuntime",
]
