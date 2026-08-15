"""Dependency-free Python SDK for the AURORA/Prism MCP server.

The package intentionally depends only on Python's standard library. It is an integration layer
above the Rust kernel: it transports exact MCP arguments and returns the server's evidence-bearing
JSON without recreating domain semantics or silently converting refusals into ordinary values.
"""

from .async_client import AsyncClient
from .authoring import (
    AcceptanceResult,
    AuthoringError,
    DecisionCell,
    DecisionCellBuilder,
    InputRef,
    MutationPlan,
    MutationSpec,
    PackArtifact,
    PackBuilder,
    ValidationIssue,
    ValidationReport,
    canonical_bytes,
    canonical_json,
    content_digest,
    validate_pack,
)
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
    "AcceptanceResult",
    "AsyncClient",
    "AsyncApiClient",
    "AsyncWorkspace",
    "AuthoringError",
    "DecisionCell",
    "DecisionCellBuilder",
    "Client",
    "ClientConfig",
    "LifecycleError",
    "InputRef",
    "MutationPlan",
    "MutationSpec",
    "ProcessExited",
    "PackArtifact",
    "PackBuilder",
    "ProtocolError",
    "RemoteError",
    "ResponseTimeout",
    "SdkError",
    "Session",
    "ToolRefusal",
    "ToolResult",
    "TransportError",
    "ValidationIssue",
    "ValidationReport",
    "Workspace",
    "canonical_bytes",
    "canonical_json",
    "content_digest",
    "validate_pack",
    "__version__",
]
