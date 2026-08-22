"""Deterministic MCP workspace used by the keyless autonomous brain integration test.

This fixture deliberately implements the smallest real stdio boundary needed by the CLI:
brain selection/prompt/plan calls, live tool discovery, and read-only workspace tools. It
does not accept credentials, open a network socket, or persist model/provider values.
"""

from __future__ import annotations

import hashlib
import json
import sys


def send(value: dict[str, object]) -> None:
    sys.stdout.write(json.dumps(value, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def result(request_id: object, value: object) -> None:
    send({"jsonrpc": "2.0", "id": request_id, "result": value})


def tool_result(request_id: object, value: dict[str, object]) -> None:
    result(
        request_id,
        {
            "content": [{"type": "text", "text": json.dumps(value, separators=(",", ":"))}],
            "isError": value.get("ok") is False,
        },
    )


for raw in sys.stdin:
    request = json.loads(raw)
    if "id" not in request:
        continue
    request_id = request["id"]
    method = request.get("method")
    params = request.get("params", {})
    if method == "initialize":
        result(
            request_id,
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": "aurora-brain-fixture", "version": "1"},
            },
        )
    elif method == "tools/list":
        result(
            request_id,
            {
                "tools": [
                    {
                        "name": "workspace_read",
                        "description": "Read one bounded fixture workspace path.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"path": {"type": "string"}},
                            "required": ["path"],
                            "additionalProperties": False,
                        },
                    },
                    {
                        "name": "repository_catalog",
                        "description": "Read bounded repository metadata for the coding domain.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"scope": {"type": "string"}},
                            "required": ["scope"],
                            "additionalProperties": False,
                        },
                    },
                    {
                        "name": "repository_update",
                        "description": "Apply a bounded repository mutation for explicit policy tests.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string"},
                                "content": {"type": "string"},
                            },
                            "required": ["path", "content"],
                            "additionalProperties": False,
                        },
                    },
                ]
            },
        )
    elif method == "tools/call":
        name = params.get("name") if isinstance(params, dict) else None
        arguments = params.get("arguments", {}) if isinstance(params, dict) else {}
        if name in {"brain_model_select", "brain_model_select_contextual"}:
            if name == "brain_model_select_contextual":
                context = arguments.get("context", {}) if isinstance(arguments, dict) else {}
                identity = {
                    field: context.get(field) if isinstance(context, dict) else None
                    for field in ("domain", "capability", "risk_class", "task_family")
                }
                context_digest = hashlib.sha256(
                    json.dumps(identity, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
                ).hexdigest()
                tool_result(
                    request_id,
                    {
                        "context_digest": context_digest,
                        "selection_status": "selected",
                        "selection": {
                            "selected_model": {"provider": "local", "model": "local-model"},
                            "decision_digest": "d" * 64,
                        },
                    },
                )
                continue
            tool_result(
                request_id,
                {
                    "ok": True,
                    "selected_model": {"provider": "local", "model": "local-model"},
                    "decision_digest": "d" * 64,
                },
            )
        elif name == "brain_prompt_assemble":
            task = arguments.get("task", "") if isinstance(arguments, dict) else ""
            tool_result(
                request_id,
                {
                    "ok": True,
                    "messages": [
                        {"role": "system", "content": "Use only bounded fixture evidence."},
                        {"role": "user", "content": str(task)},
                    ],
                    "prompt_digest": "p" * 64,
                },
            )
        elif name == "brain_plan":
            tool_result(
                request_id,
                {
                    "ok": True,
                    "plan": {
                        "requires_approval": True,
                        "steps": [{"effect": "provider_call"}],
                        "plan_digest": "b" * 64,
                    },
                },
            )
        elif name == "workspace_read":
            path = arguments.get("path", "") if isinstance(arguments, dict) else ""
            tool_result(
                request_id,
                {
                    "ok": True,
                    "path": path,
                    "content": "fixture workspace evidence",
                    "evidence": ["workspace_read_completed"],
                },
            )
        elif name == "repository_catalog":
            scope = arguments.get("scope", "") if isinstance(arguments, dict) else ""
            tool_result(
                request_id,
                {
                    "ok": True,
                    "scope": scope,
                    "repository": "fixture-repository",
                    "evidence": ["repository_catalog_completed"],
                },
            )
        elif name == "repository_update":
            path = arguments.get("path", "") if isinstance(arguments, dict) else ""
            tool_result(
                request_id,
                {
                    "ok": True,
                    "path": path,
                    "status": "repository_update_completed",
                    "evidence": ["repository_update_completed"],
                },
            )
        else:
            send(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32601, "message": "unknown fixture tool"},
                }
            )
    else:
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": -32601, "message": "unknown fixture method"},
            }
        )
