"""Safe HTTP-tool composition for the autonomous domain runtime.

The HTTP client owns transport and the caller owns credentials.  This module only composes an
already-configured :class:`~prism_sdk.http_client.ApiClient` with an explicitly reviewed
``ToolCatalogue`` and a domain tool executor.  It deliberately does not discover tools,
accept credentials, or interpret a 2xx response as a successful domain operation.

The returned callable is intended for ``AutonomousDomainToolRuntime``.  That runtime remains the
policy boundary for domain admission, approval, secret-shaped argument rejection, JSON bounds,
and metadata-only receipts.  This adapter converts transport/protocol/refusal failures into one
bounded typed error so raw headers, responses, arguments, and credential material do not cross
the autonomous receipt boundary.
"""

from __future__ import annotations

from typing import Any, Callable, Mapping

from .domain_tools import AutonomousDomainTool
from .errors import ArgumentError, ApiError, ToolRefusal, TransportError
from .http_client import ApiClient
from .tooling import ToolCatalogue


AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA = "bioprism-python-autonomous-api-tool-adapter/0.1"
AUTONOMOUS_API_TOOL_FAILURES = (
    "schema_refused",
    "remote_refusal",
    "invalid_response",
    "transport_failed",
)


class AutonomousApiToolError(TransportError):
    """Bounded failure from the HTTP-tool bridge.

    ``reason`` is an allow-listed category rather than server text.  The raw exception/response
    remains available only in the caller's transport scope, never in this error message.
    """

    def __init__(self, tool: str, reason: str) -> None:
        if not isinstance(tool, str) or not tool.strip():
            tool = "unknown"
        if reason not in AUTONOMOUS_API_TOOL_FAILURES:
            reason = "transport_failed"
        self.tool = tool
        self.reason = reason
        super().__init__(f"autonomous API tool {tool!r} failed: {reason}")


def _failure(tool: AutonomousDomainTool, reason: str) -> AutonomousApiToolError:
    return AutonomousApiToolError(tool.name, reason)


def _tool_payload(tool: AutonomousDomainTool, response: Any) -> Any:
    """Extract only the successful MCP result from a bounded REST envelope."""

    if not isinstance(response, Mapping):
        raise _failure(tool, "invalid_response")
    if response.get("ok") is not True:
        raise _failure(tool, "remote_refusal")
    mcp = response.get("mcp")
    if not isinstance(mcp, Mapping):
        raise _failure(tool, "invalid_response")
    if mcp.get("error") is not None:
        raise _failure(tool, "remote_refusal")
    result = mcp.get("result")
    if not isinstance(result, Mapping):
        raise _failure(tool, "invalid_response")
    if result.get("isError") is True:
        raise _failure(tool, "remote_refusal")
    if "structuredContent" in result:
        return result["structuredContent"]
    if "content" in result:
        return result["content"]
    return dict(result)


def create_autonomous_api_tool_executor(
    client: ApiClient,
    *,
    catalogue: ToolCatalogue,
) -> Callable[[AutonomousDomainTool, Mapping[str, Any]], Any]:
    """Build an exact-catalogue HTTP executor for domain tools.

    ``catalogue`` is mandatory so a live execution cannot silently trigger a fresh discovery
    request or execute an unreviewed name.  The caller configures ``client`` (including any
    bearer token) outside this function; credential material is never accepted or copied here.
    """

    if not isinstance(client, ApiClient):
        raise ArgumentError("autonomous API tool adapter requires an ApiClient")
    if not isinstance(catalogue, ToolCatalogue):
        raise ArgumentError("autonomous API tool adapter requires a ToolCatalogue")

    def execute(tool: AutonomousDomainTool, arguments: Mapping[str, Any]) -> Any:
        if not isinstance(tool, AutonomousDomainTool):
            raise ArgumentError("autonomous API tool adapter received an invalid domain tool")
        try:
            response = client.tool_checked(tool.name, arguments, catalogue=catalogue)
            return _tool_payload(tool, response)
        except AutonomousApiToolError:
            raise
        except (ArgumentError, ToolRefusal):
            raise _failure(tool, "schema_refused")
        except (ApiError, TransportError):
            raise _failure(tool, "transport_failed")
        except Exception:
            # The adapter is a safety boundary: unknown client/library failures are bounded just
            # like known transport failures, without echoing potentially sensitive exception text.
            raise _failure(tool, "transport_failed") from None

    return execute


__all__ = [
    "AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA",
    "AUTONOMOUS_API_TOOL_FAILURES",
    "AutonomousApiToolError",
    "create_autonomous_api_tool_executor",
]
