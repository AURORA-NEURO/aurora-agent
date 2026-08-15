"""Errors raised by the dependency-free Prism MCP SDK.

The SDK deliberately keeps transport, protocol, lifecycle, and remote-tool failures distinct.
Callers can therefore retry a broken process without retrying a refused scientific or safety
decision, and can surface the server's structured refusal without parsing exception strings.
"""

from __future__ import annotations

from typing import Any


class SdkError(Exception):
    """Base class for all SDK failures."""


class ArgumentError(SdkError, ValueError):
    """The caller supplied an invalid or unbounded SDK argument."""


class LifecycleError(SdkError):
    """An operation was attempted in the wrong client lifecycle state."""


class TransportError(SdkError):
    """The child process or its stdio transport could not be used."""


class ApiError(TransportError):
    """The HTTP API returned a bounded error response."""

    def __init__(self, status: int, payload: Any) -> None:
        self.status = status
        self.payload = payload
        message = payload.get("error", payload) if isinstance(payload, dict) else payload
        super().__init__(f"Prism HTTP API returned {status}: {message}")


class ProcessExited(TransportError):
    """The MCP child process exited before answering a request."""

    def __init__(self, returncode: int | None, stderr: str = "") -> None:
        self.returncode = returncode
        self.stderr = stderr
        detail = f"MCP process exited with code {returncode}"
        if stderr:
            detail += f": {stderr.strip()[-1000:]}"
        super().__init__(detail)


class ResponseTimeout(TransportError):
    """The MCP child process did not answer within the configured bound."""

    def __init__(self, method: str, timeout: float) -> None:
        self.method = method
        self.timeout = timeout
        super().__init__(f"timed out waiting for {method!r} after {timeout:g}s")


class MissionWaitTimeout(SdkError, TimeoutError):
    """A bounded mission wait expired before the job reached a terminal state."""

    def __init__(self, mission_id: str, timeout: float, last_job: Any) -> None:
        self.mission_id = mission_id
        self.timeout = timeout
        self.last_job = last_job
        status = getattr(last_job, "status", "unknown")
        super().__init__(
            f"timed out waiting for mission {mission_id!r} after {timeout:g}s; last status is {status!r}"
        )


class ProtocolError(SdkError):
    """The peer emitted malformed or semantically invalid JSON-RPC."""


class RemoteError(SdkError):
    """The peer returned a JSON-RPC error response."""

    def __init__(
        self,
        code: int,
        message: str,
        data: Any = None,
    ) -> None:
        self.code = code
        self.message = message
        self.data = data
        suffix = f"; data={data!r}" if data is not None else ""
        super().__init__(f"MCP error {code}: {message}{suffix}")


class ToolRefusal(SdkError):
    """A tool returned a structured refusal instead of a successful payload."""

    def __init__(self, tool: str, payload: Any) -> None:
        self.tool = tool
        self.payload = payload
        reason = "tool returned a non-success payload"
        if isinstance(payload, dict):
            reason = str(payload.get("refusal") or payload.get("error") or reason)
        super().__init__(f"{tool}: {reason}")
