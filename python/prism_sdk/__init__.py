"""Dependency-free Python SDK for the AURORA/Prism MCP server.

The package intentionally depends only on Python's standard library. It is an integration layer
above the Rust kernel: it transports exact MCP arguments and returns the server's evidence-bearing
JSON without recreating domain semantics or silently converting refusals into ordinary values.
"""

from .async_client import AsyncClient
from .client import Client, ClientConfig
from .errors import (
    ApiError,
    ArgumentError,
    LifecycleError,
    ProcessExited,
    ProtocolError,
    RemoteError,
    ResponseTimeout,
    SdkError,
    ToolRefusal,
    TransportError,
)
from .http_client import ApiClient, AsyncApiClient
from .models import Session, ToolResult
from .workspace import AsyncWorkspace, Workspace

__version__ = "0.1.0"

__all__ = [
    "ArgumentError",
    "ApiClient",
    "ApiError",
    "AsyncClient",
    "AsyncApiClient",
    "AsyncWorkspace",
    "Client",
    "ClientConfig",
    "LifecycleError",
    "ProcessExited",
    "ProtocolError",
    "RemoteError",
    "ResponseTimeout",
    "SdkError",
    "Session",
    "ToolRefusal",
    "ToolResult",
    "TransportError",
    "Workspace",
    "__version__",
]
