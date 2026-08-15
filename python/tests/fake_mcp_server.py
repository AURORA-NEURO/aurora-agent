"""Tiny stdio peer used by the SDK tests; it is not a production server."""

from __future__ import annotations

import json
import sys


def send(value: dict) -> None:
    sys.stdout.write(json.dumps(value, separators=(",", ":")) + "\n")
    sys.stdout.flush()


for raw in sys.stdin:
    request = json.loads(raw)
    if "id" not in request:
        continue
    request_id = request["id"]
    method = request.get("method")
    if method == "initialize":
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {"tools": {"listChanged": False}},
                    "serverInfo": {"name": "fake-prism", "version": "test"},
                },
            }
        )
    elif method == "tools/list":
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {"tools": [{"name": "echo", "inputSchema": {"type": "object"}}]},
            }
        )
    elif method == "tools/call":
        params = request.get("params", {})
        name = params.get("name")
        if name == "remote_error":
            send(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32001, "message": "remote refusal", "data": {"retry": False}},
                }
            )
            continue
        if name == "refuse":
            payload = {"ok": False, "refusal": "fixture refuses", "fail_closed": True}
        else:
            payload = {"ok": True, "echo": params.get("arguments", {})}
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "content": [{"type": "text", "text": json.dumps(payload)}],
                    "isError": payload.get("ok") is False,
                },
            }
        )
    elif method == "resources/read":
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {"contents": [{"uri": request["params"]["uri"], "text": "{}"}]},
            }
        )
    else:
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": -32601, "message": f"unknown method: {method}"},
            }
        )
