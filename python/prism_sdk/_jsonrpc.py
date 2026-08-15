"""Internal JSON-RPC framing and argument validation shared by both clients."""

from __future__ import annotations

import json
from typing import Any, Mapping

from .errors import ArgumentError, ProtocolError

DEFAULT_MAX_FRAME_BYTES = 20_000_000
JSON_RPC_VERSION = "2.0"
MCP_PROTOCOL_VERSION = "2025-06-18"


def object_argument(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{label} must be a JSON object")
    return dict(value)


def frame(value: Mapping[str, Any], max_bytes: int = DEFAULT_MAX_FRAME_BYTES) -> bytes:
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"JSON-RPC value is not JSON-safe: {error}") from error
    if len(encoded) > max_bytes:
        raise ArgumentError(
            f"JSON-RPC frame is {len(encoded)} bytes, over the {max_bytes}-byte bound"
        )
    return encoded + b"\n"


def parse_response(raw: bytes, max_bytes: int = DEFAULT_MAX_FRAME_BYTES) -> dict[str, Any]:
    if len(raw) > max_bytes:
        raise ProtocolError(
            f"peer response is {len(raw)} bytes, over the {max_bytes}-byte bound"
        )
    try:
        decoded = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProtocolError(f"peer emitted malformed JSON-RPC: {error}") from error
    if not isinstance(decoded, dict):
        raise ProtocolError("JSON-RPC response must be an object")
    if decoded.get("jsonrpc") != JSON_RPC_VERSION:
        raise ProtocolError("JSON-RPC response has an unsupported jsonrpc version")
    if "id" not in decoded and "method" not in decoded:
        raise ProtocolError("JSON-RPC response has neither id nor method")
    if "error" in decoded and not isinstance(decoded["error"], Mapping):
        raise ProtocolError("JSON-RPC error member must be an object")
    return decoded


def request_object(request_id: int, method: str, params: Mapping[str, Any] | None) -> dict[str, Any]:
    if not method or not isinstance(method, str):
        raise ArgumentError("JSON-RPC method must be a non-empty string")
    return {
        "jsonrpc": JSON_RPC_VERSION,
        "id": request_id,
        "method": method,
        "params": dict(params or {}),
    }


def notification_object(method: str, params: Mapping[str, Any] | None) -> dict[str, Any]:
    if not method or not isinstance(method, str):
        raise ArgumentError("JSON-RPC method must be a non-empty string")
    return {"jsonrpc": JSON_RPC_VERSION, "method": method, "params": dict(params or {})}
