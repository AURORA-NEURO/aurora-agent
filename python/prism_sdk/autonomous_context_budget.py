"""Deterministic, provider-neutral context-window budgeting for autonomous runs.

The compactor is intentionally loss-aware and does not call an LLM to summarize history. It
protects instructions and the recent conversation, removes only old atomic turns, and returns a
metadata-only plan. Provider requests and their content remain caller-owned and transient.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
import math
from typing import Any, Mapping, Sequence

from .authoring import content_digest


AUTONOMOUS_CONTEXT_BUDGET_SCHEMA = "bioprism-autonomous-context-budget/0.1"
MAX_AUTONOMOUS_CONTEXT_INPUT_TOKENS = 1_000_000
MAX_AUTONOMOUS_CONTEXT_MESSAGES = 1_024
MAX_AUTONOMOUS_CONTEXT_RECENT_MESSAGES = 128


class AutonomousContextBudgetError(ValueError):
    """Protected provider context cannot fit within the caller's declared budget."""

    code = "invalid_request"


@dataclass(frozen=True, slots=True)
class AutonomousContextBudgetOptions:
    """Explicit, caller-owned limits for lossy context compaction."""

    max_input_tokens: int
    preserve_recent_messages: int = 8
    max_messages: int = MAX_AUTONOMOUS_CONTEXT_MESSAGES

    def __post_init__(self) -> None:
        _integer("context budget max_input_tokens", self.max_input_tokens, 1, MAX_AUTONOMOUS_CONTEXT_INPUT_TOKENS)
        _integer("context budget preserve_recent_messages", self.preserve_recent_messages, 0, MAX_AUTONOMOUS_CONTEXT_RECENT_MESSAGES)
        _integer("context budget max_messages", self.max_messages, 1, MAX_AUTONOMOUS_CONTEXT_MESSAGES)

    def to_dict(self) -> dict[str, int]:
        return {
            "max_input_tokens": self.max_input_tokens,
            "preserve_recent_messages": self.preserve_recent_messages,
            "max_messages": self.max_messages,
        }


@dataclass(frozen=True, slots=True)
class AutonomousContextBudgetPlan:
    schema: str
    status: str
    strategy: str
    original_input_tokens: int
    final_input_tokens: int
    max_input_tokens: int
    messages_before: int
    messages_after: int
    dropped_message_count: int
    dropped_message_indexes: tuple[int, ...]
    protected_message_count: int
    protected_instruction_count: int
    tool_turns_dropped: int
    message_shape_digest: str
    plan_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "strategy": self.strategy,
            "original_input_tokens": self.original_input_tokens,
            "final_input_tokens": self.final_input_tokens,
            "max_input_tokens": self.max_input_tokens,
            "messages_before": self.messages_before,
            "messages_after": self.messages_after,
            "dropped_message_count": self.dropped_message_count,
            "dropped_message_indexes": list(self.dropped_message_indexes),
            "protected_message_count": self.protected_message_count,
            "protected_instruction_count": self.protected_instruction_count,
            "tool_turns_dropped": self.tool_turns_dropped,
            "message_shape_digest": self.message_shape_digest,
            "plan_digest": self.plan_digest,
            "retention": "transient_request_compaction_metadata_only",
            "content_retention": "provider_content_not_retained_in_plan",
        }


@dataclass(frozen=True, slots=True)
class AutonomousContextBudgetResult:
    request: Any
    plan: AutonomousContextBudgetPlan


@dataclass(frozen=True, slots=True)
class _ContextUnit:
    indexes: tuple[int, ...]
    tokens: int
    protected: bool
    tool_turn: bool


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise AutonomousContextBudgetError(f"{name} must be an integer within [{minimum}, {maximum}]")
    return value


def normalize_autonomous_context_budget(
    value: AutonomousContextBudgetOptions | Mapping[str, Any],
) -> AutonomousContextBudgetOptions:
    if isinstance(value, AutonomousContextBudgetOptions):
        return value
    if not isinstance(value, Mapping):
        raise AutonomousContextBudgetError("context budget options must be a mapping")
    unsupported = sorted(set(value).difference({"max_input_tokens", "preserve_recent_messages", "max_messages"}))
    if unsupported:
        raise AutonomousContextBudgetError("context budget options contain unsupported fields: " + ", ".join(unsupported))
    return AutonomousContextBudgetOptions(
        max_input_tokens=value.get("max_input_tokens"),
        preserve_recent_messages=value.get("preserve_recent_messages", 8),
        max_messages=value.get("max_messages", MAX_AUTONOMOUS_CONTEXT_MESSAGES),
    )


def _json_bytes(value: Any) -> int:
    try:
        encoded = json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
    except (TypeError, ValueError) as error:
        raise AutonomousContextBudgetError("context budget input is not JSON-safe") from error
    return len(encoded.encode("utf-8"))


def _message_tokens(message: Mapping[str, Any]) -> int:
    content = message.get("content")
    content_size = len(content.encode("utf-8")) if isinstance(content, str) else _json_bytes(content)
    metadata = {
        "role": message.get("role"),
        "name": message.get("name"),
        "tool_call_id": message.get("tool_call_id"),
        "tool_calls": message.get("tool_calls"),
        "is_error": message.get("is_error"),
    }
    role_size = len(str(message.get("role", "")).encode("utf-8"))
    return max(1, math.ceil((role_size + 32 + content_size + _json_bytes(metadata)) / 4))


def _tool_call_ids(message: Mapping[str, Any]) -> set[str]:
    calls = message.get("tool_calls", ())
    if not isinstance(calls, Sequence) or isinstance(calls, (str, bytes)):
        return set()
    return {
        str(call.get("id"))
        for call in calls
        if isinstance(call, Mapping) and isinstance(call.get("id"), str)
    }


def _units(messages: Sequence[Mapping[str, Any]], preserve_recent_messages: int) -> tuple[list[_ContextUnit], list[int], set[int]]:
    tokens = [_message_tokens(message) for message in messages]
    latest_user = max((index for index, message in enumerate(messages) if message.get("role") == "user"), default=-1)
    protected_indexes = {
        index
        for index, message in enumerate(messages)
        if message.get("role") in {"system", "developer"}
        or index >= max(0, len(messages) - preserve_recent_messages)
        or index == latest_user
    }
    result: list[_ContextUnit] = []
    index = 0
    while index < len(messages):
        message = messages[index]
        call_ids = _tool_call_ids(message)
        indexes = [index]
        end = index + 1
        if message.get("role") == "assistant" and call_ids:
            while end < len(messages) and messages[end].get("role") == "tool" and messages[end].get("tool_call_id") in call_ids:
                indexes.append(end)
                end += 1
        result.append(
            _ContextUnit(
                indexes=tuple(indexes),
                tokens=sum(tokens[item] for item in indexes),
                protected=any(item in protected_indexes for item in indexes),
                tool_turn=len(indexes) > 1 or message.get("role") == "tool" or bool(call_ids),
            )
        )
        index = end
    # A freshly appended tool continuation is semantically required to interpret the next model
    # turn. Keep the newest tool unit protected even when the caller intentionally sets a zero
    # recent-message tail; older tool units remain removable, but always as atomic units.
    for unit_index in range(len(result) - 1, -1, -1):
        unit = result[unit_index]
        if not unit.tool_turn or unit.indexes[-1] != len(messages) - 1:
            continue
        result[unit_index] = replace(unit, protected=True)
        protected_indexes.update(unit.indexes)
        break
    return result, tokens, protected_indexes


def compact_autonomous_provider_request(
    request: Any,
    options: AutonomousContextBudgetOptions | Mapping[str, Any],
) -> AutonomousContextBudgetResult:
    """Remove old atomic turns until the provider request fits the explicit context budget."""

    messages = getattr(request, "messages", None)
    if not isinstance(messages, Sequence) or isinstance(messages, (str, bytes)) or not messages:
        raise AutonomousContextBudgetError("context budget requires a non-empty provider request")
    if len(messages) > MAX_AUTONOMOUS_CONTEXT_MESSAGES:
        raise AutonomousContextBudgetError("provider request exceeds the context budget message ceiling")
    if any(not isinstance(message, Mapping) for message in messages):
        raise AutonomousContextBudgetError("context budget provider messages must be mappings")
    normalized = normalize_autonomous_context_budget(options)
    source = [dict(message) for message in messages]
    units, tokens, protected_indexes = _units(source, normalized.preserve_recent_messages)
    original = max(1, sum(tokens))
    kept = set(range(len(source)))
    final = original
    dropped: list[int] = []
    dropped_tool_turns = 0
    for unit in (item for item in units if not item.protected):
        if final <= normalized.max_input_tokens and len(kept) <= normalized.max_messages:
            break
        for index in unit.indexes:
            kept.discard(index)
            dropped.append(index)
        final -= unit.tokens
        if unit.tool_turn:
            dropped_tool_turns += 1
    dropped.sort()
    if final > normalized.max_input_tokens or len(kept) > normalized.max_messages:
        raise AutonomousContextBudgetError(
            f"context budget cannot fit protected provider messages ({final} estimated tokens, {len(kept)} messages)"
        )
    final_messages = tuple(source[index] for index in range(len(source)) if index in kept)
    shape = [
        {
            "role": message.get("role"),
            "content_kind": "text" if isinstance(message.get("content"), str) else "parts",
            "content_bytes": len(message["content"].encode("utf-8")) if isinstance(message.get("content"), str) else _json_bytes(message.get("content")),
            "tool_call_count": len(message.get("tool_calls", ())) if isinstance(message.get("tool_calls"), Sequence) and not isinstance(message.get("tool_calls"), (str, bytes)) else 0,
            "has_tool_call_id": "tool_call_id" in message,
            "tokens": tokens[index],
            "protected": index in protected_indexes,
        }
        for index, message in enumerate(source)
    ]
    shape_digest = content_digest(shape)
    body = {
        "schema": AUTONOMOUS_CONTEXT_BUDGET_SCHEMA,
        "status": "compacted" if dropped else "unchanged",
        "strategy": "drop_oldest_atomic_turns",
        "original_input_tokens": original,
        "final_input_tokens": final,
        "max_input_tokens": normalized.max_input_tokens,
        "messages_before": len(source),
        "messages_after": len(final_messages),
        "dropped_message_count": len(dropped),
        "dropped_message_indexes": dropped,
        "protected_message_count": sum(1 for index in protected_indexes if index in kept),
        "protected_instruction_count": sum(message.get("role") in {"system", "developer"} for message in source),
        "tool_turns_dropped": dropped_tool_turns,
        "message_shape_digest": shape_digest,
    }
    plan = AutonomousContextBudgetPlan(
        schema=AUTONOMOUS_CONTEXT_BUDGET_SCHEMA,
        status=body["status"],
        strategy=body["strategy"],
        original_input_tokens=original,
        final_input_tokens=final,
        max_input_tokens=normalized.max_input_tokens,
        messages_before=len(source),
        messages_after=len(final_messages),
        dropped_message_count=len(dropped),
        dropped_message_indexes=tuple(dropped),
        protected_message_count=body["protected_message_count"],
        protected_instruction_count=body["protected_instruction_count"],
        tool_turns_dropped=body["tool_turns_dropped"],
        message_shape_digest=shape_digest,
        plan_digest=content_digest(body),
    )
    return AutonomousContextBudgetResult(
        request=request if not dropped else replace(request, messages=final_messages),
        plan=plan,
    )


__all__ = [
    "AUTONOMOUS_CONTEXT_BUDGET_SCHEMA",
    "MAX_AUTONOMOUS_CONTEXT_INPUT_TOKENS",
    "MAX_AUTONOMOUS_CONTEXT_MESSAGES",
    "MAX_AUTONOMOUS_CONTEXT_RECENT_MESSAGES",
    "AutonomousContextBudgetError",
    "AutonomousContextBudgetOptions",
    "AutonomousContextBudgetPlan",
    "AutonomousContextBudgetResult",
    "compact_autonomous_provider_request",
    "normalize_autonomous_context_budget",
]
