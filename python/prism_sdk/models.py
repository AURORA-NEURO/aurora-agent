"""Small immutable result models shared by the sync and async clients."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping

from .errors import ProtocolError, ToolRefusal

JsonObject = dict[str, Any]


@dataclass(frozen=True)
class ToolResult:
    """The raw MCP result plus helpers for the server's JSON text projection.

    The Rust server returns tool payloads as JSON in a text content block so the result remains
    compatible with ordinary MCP clients. The raw envelope is retained because error flags and
    additional content are evidence, not decoration.
    """

    tool: str
    envelope: JsonObject

    @property
    def is_error(self) -> bool:
        return bool(self.envelope.get("isError", False))

    @property
    def content(self) -> list[Mapping[str, Any]]:
        raw = self.envelope.get("content", [])
        if not isinstance(raw, list):
            raise ProtocolError("MCP tool result content must be an array")
        return [item for item in raw if isinstance(item, Mapping)]

    @property
    def text_blocks(self) -> tuple[str, ...]:
        values: list[str] = []
        for block in self.content:
            if block.get("type") == "text" and isinstance(block.get("text"), str):
                values.append(block["text"])
        return tuple(values)

    def text(self) -> str:
        """Return all text blocks in order, separated by a newline."""

        return "\n".join(self.text_blocks)

    def value(self) -> Any:
        """Decode the JSON text projection, refusing an absent or malformed projection."""

        text = self.text()
        if not text:
            raise ProtocolError(f"tool {self.tool!r} returned no JSON text content")
        try:
            return json.loads(text)
        except json.JSONDecodeError as error:
            raise ProtocolError(
                f"tool {self.tool!r} returned non-JSON text content: {error}"
            ) from error

    def require_object(self) -> JsonObject:
        value = self.value()
        if not isinstance(value, dict):
            raise ProtocolError(f"tool {self.tool!r} payload must be a JSON object")
        return value

    def require_ok(self) -> JsonObject:
        """Return a successful object or raise while retaining the refusal payload."""

        value = self.require_object()
        if self.is_error or value.get("ok") is False:
            raise ToolRefusal(self.tool, value)
        return value

    @classmethod
    def from_response(cls, tool: str, response: Mapping[str, Any]) -> "ToolResult":
        result = response.get("result")
        if not isinstance(result, Mapping):
            raise ProtocolError(f"tools/call response for {tool!r} has no result object")
        envelope = dict(result)
        return cls(tool=tool, envelope=envelope)


@dataclass(frozen=True)
class Session:
    """The server initialization payload retained for routing and diagnostics."""

    protocol_version: str
    server_info: JsonObject
    capabilities: JsonObject
    raw: JsonObject
