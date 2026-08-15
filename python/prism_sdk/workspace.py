"""High-level helpers for the most important cross-domain MCP workflows."""

from __future__ import annotations

from typing import Any, Mapping, Sequence

from .async_client import AsyncClient
from .client import Client
from .errors import ArgumentError


def _targets(request_id: str | None, targets: Sequence[str] | None) -> dict[str, Any] | None:
    if request_id is None and targets is None:
        return None
    if not isinstance(request_id, str) or not request_id:
        raise ArgumentError("request_id is required when targets are supplied")
    if not targets:
        raise ArgumentError("targets must contain at least one target")
    return {"id": request_id, "targets": list(targets)}


class Workspace:
    """Typed convenience facade over an initialized synchronous MCP client."""

    def __init__(self, client: Client) -> None:
        self.client = client

    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return self.client.call_tool(name, arguments).require_ok()

    def developer_delivery_audit(
        self,
        *,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        platform: Mapping[str, Any] | None = None,
        repository: Mapping[str, Any] | None = None,
        repository_impact: Mapping[str, Any] | None = None,
        sdk: Mapping[str, Any] | None = None,
        conformance: Mapping[str, Any] | None = None,
        provider: Mapping[str, Any] | None = None,
        governance: Mapping[str, Any] | None = None,
        release: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {}
        for key, value in (
            ("platform", platform),
            ("repository", repository),
            ("repository_impact", repository_impact),
            ("sdk", sdk),
            ("conformance", conformance),
            ("provider", provider),
            ("governance", governance),
            ("release", release),
        ):
            if value is not None:
                arguments[key] = dict(value)
        release_request = _targets(request_id, targets)
        if release_request is not None:
            arguments["release_request"] = release_request
        return self.tool("developer_delivery_audit", arguments)

    def bioatlas_publication_audit(
        self,
        atlas: Mapping[str, Any],
        *,
        weighting: Mapping[str, Any] | None = None,
        evidence_audit: Mapping[str, Any] | None = None,
        card: Mapping[str, Any] | None = None,
        leaderboard: Mapping[str, Any] | None = None,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        max_items: int | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {"atlas": dict(atlas)}
        for key, value in (
            ("weighting", weighting),
            ("evidence_audit", evidence_audit),
            ("card", card),
            ("leaderboard", leaderboard),
        ):
            if value is not None:
                arguments[key] = dict(value)
        release_request = _targets(request_id, targets)
        if release_request is not None:
            arguments["release_request"] = release_request
        if max_items is not None:
            arguments["max_items"] = max_items
        return self.tool("bioatlas_publication_audit", arguments)

    def compile_context(
        self,
        world: Mapping[str, Any],
        query: Mapping[str, Any],
        *,
        policy: str | None = None,
        profile: str | None = None,
        include_views: bool | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {"world": dict(world), "query": dict(query)}
        if policy is not None:
            arguments["policy"] = policy
        if profile is not None:
            arguments["profile"] = profile
        if include_views is not None:
            arguments["include_views"] = include_views
        return self.tool("fiber_compile", arguments)


class AsyncWorkspace:
    """Async convenience facade mirroring :class:`Workspace`."""

    def __init__(self, client: AsyncClient) -> None:
        self.client = client

    async def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return (await self.client.call_tool(name, arguments)).require_ok()

    async def developer_delivery_audit(
        self,
        *,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        platform: Mapping[str, Any] | None = None,
        repository: Mapping[str, Any] | None = None,
        repository_impact: Mapping[str, Any] | None = None,
        sdk: Mapping[str, Any] | None = None,
        conformance: Mapping[str, Any] | None = None,
        provider: Mapping[str, Any] | None = None,
        governance: Mapping[str, Any] | None = None,
        release: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        arguments = _developer_delivery_arguments(
            {
                "request_id": request_id,
                "targets": targets,
                "platform": platform,
                "repository": repository,
                "repository_impact": repository_impact,
                "sdk": sdk,
                "conformance": conformance,
                "provider": provider,
                "governance": governance,
                "release": release,
            }
        )
        return (await self.client.call_tool("developer_delivery_audit", arguments)).require_ok()

    async def bioatlas_publication_audit(self, atlas: Mapping[str, Any], **kwargs: Any) -> dict[str, Any]:
        arguments: dict[str, Any] = {"atlas": dict(atlas)}
        for key in ("weighting", "evidence_audit", "card", "leaderboard"):
            if kwargs.get(key) is not None:
                arguments[key] = dict(kwargs[key])
        release_request = _targets(kwargs.get("request_id"), kwargs.get("targets"))
        if release_request is not None:
            arguments["release_request"] = release_request
        if kwargs.get("max_items") is not None:
            arguments["max_items"] = kwargs["max_items"]
        return (await self.client.call_tool("bioatlas_publication_audit", arguments)).require_ok()

    async def compile_context(
        self,
        world: Mapping[str, Any],
        query: Mapping[str, Any],
        *,
        policy: str | None = None,
        profile: str | None = None,
        include_views: bool | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {"world": dict(world), "query": dict(query)}
        if policy is not None:
            arguments["policy"] = policy
        if profile is not None:
            arguments["profile"] = profile
        if include_views is not None:
            arguments["include_views"] = include_views
        return (await self.client.call_tool("fiber_compile", arguments)).require_ok()


def _developer_delivery_arguments(kwargs: Mapping[str, Any]) -> dict[str, Any]:
    arguments: dict[str, Any] = {}
    for key in (
        "platform",
        "repository",
        "repository_impact",
        "sdk",
        "conformance",
        "provider",
        "governance",
        "release",
    ):
        if kwargs.get(key) is not None:
            arguments[key] = dict(kwargs[key])
    release_request = _targets(kwargs.get("request_id"), kwargs.get("targets"))
    if release_request is not None:
        arguments["release_request"] = release_request
    return arguments
