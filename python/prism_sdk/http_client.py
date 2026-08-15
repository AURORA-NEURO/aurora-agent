"""Standard-library HTTP client for the bounded Prism API gateway.

The HTTP client is deliberately separate from the stdio MCP client.  It supports the gateway's
health/capability routes, REST tool calls, cursor-based events, and signed webhook outbox lifecycle;
it never retries a domain refusal automatically and never treats a 2xx transport response as proof
that a scientific claim was accepted.
"""

from __future__ import annotations

import asyncio
import http.client
import json
import ssl
from typing import Any, Mapping, Sequence
from urllib.parse import urlsplit

from .biological import AdapterPlanRequest
from .capability import CapabilityQuery, CapabilityRouteNeed, CapabilityRouteRequest
from .context_requests import (
    ContextLayer,
    FiberCompileRequest,
    FiberExplainRequest,
    FiberRefineRequest,
    FiberVerifyRequest,
    ProjectionBundleRequest,
)
from .errors import ApiError, ArgumentError, TransportError
from .bioql import BioQlCompileRequest
from .evidence import BioCapabilityEvidenceAuditRequest
from .domain_requests import LabPlanRequest, RoutingDecisionRequest, WorldClaimCheckRequest
from .mission import MissionPreflight, MissionRequest, preflight_mission
from .repository_requests import (
    RepositoryBundleRequest,
    RepositoryCatalogRequest,
    RepositoryImpactRequest,
    RepositoryTraversalPolicy,
    TelemetryProjectRequest,
)
from .tooling import ToolCallPlan, ToolCatalogue


def _capability_query_arguments(
    query: CapabilityQuery | str | None,
    *,
    text: str | None,
    domain: str | None,
    tool: str | None,
    group_id: str | None,
    max_items: int,
    include_tools: bool,
) -> dict[str, Any]:
    if isinstance(query, CapabilityQuery):
        if (
            any(value is not None for value in (text, domain, tool, group_id))
            or max_items != 50
            or include_tools
        ):
            raise ArgumentError("query cannot be combined with individual capability filters")
        return query.to_mcp_arguments()
    if isinstance(query, str):
        if text is not None:
            raise ArgumentError("query cannot be combined with text")
        return CapabilityQuery(query, group_id, domain, tool, max_items, include_tools).to_mcp_arguments()
    if query is not None:
        raise ArgumentError("query must be a CapabilityQuery or string")
    return CapabilityQuery(text, group_id, domain, tool, max_items, include_tools).to_mcp_arguments()


class ApiClient:
    """Synchronous, bounded HTTP client for ``bioprism-api``."""

    def __init__(
        self,
        base_url: str,
        *,
        bearer_token: str | None = None,
        timeout: float = 30.0,
        max_response_bytes: int = 20_000_000,
        ssl_context: ssl.SSLContext | None = None,
    ) -> None:
        parsed = urlsplit(base_url.rstrip("/"))
        if parsed.scheme not in {"http", "https"} or not parsed.hostname:
            raise ArgumentError("base_url must be an http(s) URL with a host")
        if parsed.path not in {"", "/"} or parsed.query or parsed.fragment:
            raise ArgumentError("base_url must not include a path, query, or fragment")
        if timeout <= 0 or max_response_bytes <= 0:
            raise ArgumentError("timeout and max_response_bytes must be positive")
        if bearer_token is not None and (len(bearer_token) < 16 or any(ord(c) <= 0x20 for c in bearer_token)):
            raise ArgumentError("bearer_token must contain at least 16 visible characters")
        self.base_url = parsed
        self.bearer_token = bearer_token
        self.timeout = timeout
        self.max_response_bytes = max_response_bytes
        self.ssl_context = ssl_context

    def request(
        self,
        method: str,
        path: str,
        payload: Mapping[str, Any] | None = None,
        *,
        headers: Mapping[str, str] | None = None,
    ) -> dict[str, Any]:
        if method not in {"GET", "POST", "DELETE", "OPTIONS"}:
            raise ArgumentError("method must be GET, POST, DELETE, or OPTIONS")
        if not path.startswith("/") or "\r" in path or "\n" in path:
            raise ArgumentError("path must be an origin-form path")
        body = b""
        request_headers = {"Accept": "application/json"}
        if payload is not None:
            try:
                body = json.dumps(payload, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise ArgumentError(f"payload is not JSON-safe: {error}") from error
            request_headers["Content-Type"] = "application/json"
        if self.bearer_token is not None:
            request_headers["Authorization"] = f"Bearer {self.bearer_token}"
        if headers is not None:
            for name, value in headers.items():
                if not name or "\r" in name or "\n" in name or "\r" in value or "\n" in value:
                    raise ArgumentError("HTTP headers must not contain control-line breaks")
                request_headers[name] = value
        connection: http.client.HTTPConnection | http.client.HTTPSConnection
        try:
            if self.base_url.scheme == "https":
                connection = http.client.HTTPSConnection(
                    self.base_url.hostname,
                    self.base_url.port,
                    timeout=self.timeout,
                    context=self.ssl_context,
                )
            else:
                connection = http.client.HTTPConnection(
                    self.base_url.hostname,
                    self.base_url.port,
                    timeout=self.timeout,
                )
            connection.request(method, path, body=body, headers=request_headers)
            response = connection.getresponse()
            raw = response.read(self.max_response_bytes + 1)
            status = response.status
        except (OSError, http.client.HTTPException) as error:
            raise TransportError(f"HTTP API request failed: {error}") from error
        finally:
            try:
                connection.close()
            except UnboundLocalError:
                pass
        if len(raw) > self.max_response_bytes:
            raise TransportError("HTTP API response exceeded max_response_bytes")
        if not raw:
            parsed: Any = {}
        else:
            try:
                parsed = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise TransportError(f"HTTP API returned invalid JSON: {error}") from error
        if not isinstance(parsed, dict):
            raise TransportError("HTTP API response must be a JSON object")
        if status >= 400:
            raise ApiError(status, parsed)
        return parsed

    def health(self) -> dict[str, Any]:
        return self.request("GET", "/healthz")

    def capabilities(self) -> dict[str, Any]:
        return self.request("GET", "/v1/capabilities")

    def tools(self) -> list[dict[str, Any]]:
        value = self.request("GET", "/v1/tools")
        tools = value.get("tools")
        if not isinstance(tools, list) or any(not isinstance(tool, dict) for tool in tools):
            raise TransportError("HTTP API tools response has no object array")
        return tools

    def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        if not isinstance(name, str) or not name or "/" in name:
            raise ArgumentError("tool name must be a non-empty path-safe string")
        return self.request("POST", f"/v1/tools/{name}", dict(arguments or {}))

    def tool_catalogue(self) -> ToolCatalogue:
        """Snapshot the authoritative live HTTP ``/v1/tools`` catalogue."""

        return ToolCatalogue.from_definitions(self.tools())

    def plan_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> ToolCallPlan:
        """Validate any advertised tool's JSON shape without issuing a POST."""

        snapshot = catalogue if catalogue is not None else self.tool_catalogue()
        if not isinstance(snapshot, ToolCatalogue):
            raise ArgumentError("catalogue must be a ToolCatalogue")
        return snapshot.plan(name, arguments)

    def tool_checked(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> dict[str, Any]:
        """Run any advertised tool after conservative schema preflight."""

        plan = self.plan_tool(name, arguments, catalogue=catalogue)
        return self.call_tool(plan.tool, plan.to_mcp_arguments())

    def mission_preflight(
        self,
        request: MissionRequest,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> MissionPreflight:
        """Review a mission against the live HTTP tool catalogue without issuing a POST."""

        if not isinstance(request, MissionRequest):
            raise ArgumentError("request must be a MissionRequest")
        snapshot = catalogue if catalogue is not None else self.tool_catalogue()
        return preflight_mission(request, snapshot)

    def capability_discover(
        self,
        *,
        query: CapabilityQuery | str | None = None,
        text: str | None = None,
        domain: str | None = None,
        tool: str | None = None,
        group_id: str | None = None,
        max_items: int = 50,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Search the complete cross-domain catalogue through the HTTP gateway."""

        arguments = _capability_query_arguments(
            query,
            text=text,
            domain=domain,
            tool=tool,
            group_id=group_id,
            max_items=max_items,
            include_tools=include_tools,
        )
        return self.call_tool("capability_discover", arguments)

    def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return self.call_tool("capability_audit", {"include_groups": include_groups})

    def capability_route(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return self.call_tool("capability_route", request.to_mcp_arguments())

    def adapter_plan(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Plan native and Python-delegated adapters through the HTTP gateway."""

        request = AdapterPlanRequest(
            source_id,
            source_kind,
            declared_format,
            required_conformance,
            available_dependencies,
        )
        return self.call_tool("adapter_plan", request.to_mcp_arguments())

    def biocapability_evidence_audit(
        self,
        request: BioCapabilityEvidenceAuditRequest,
    ) -> dict[str, Any]:
        """Run the evidence-conditioned capability audit through the HTTP gateway."""

        if not isinstance(request, BioCapabilityEvidenceAuditRequest):
            raise ArgumentError("request must be a BioCapabilityEvidenceAuditRequest")
        return self.call_tool("biocapability_evidence_audit", request.to_mcp_arguments())

    def bioql_compile(
        self,
        query: str | BioQlCompileRequest,
        schema: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Compile BioQL through the HTTP gateway without executing it."""

        if isinstance(query, BioQlCompileRequest):
            if schema is not None:
                raise ArgumentError("schema must be omitted when query is a BioQlCompileRequest")
            request = query
        else:
            if schema is None:
                raise ArgumentError("schema is required when query is a string")
            request = BioQlCompileRequest(query, schema)
        return self.call_tool("bioql_compile", request.to_mcp_arguments())

    def world_claim_check(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Check a serialized claim against world provenance through HTTP."""

        if isinstance(provenance, WorldClaimCheckRequest):
            if claim is not None:
                raise ArgumentError("claim must be omitted when provenance is a WorldClaimCheckRequest")
            request = provenance
        else:
            if claim is None:
                raise ArgumentError("claim is required when provenance is a mapping")
            request = WorldClaimCheckRequest(provenance, claim)
        return self.call_tool("world_claim_check", request.to_mcp_arguments())

    def lab_plan(
        self,
        graph: Mapping[str, Any] | LabPlanRequest,
        actions: Sequence[Mapping[str, Any]] | None = None,
        budget: Mapping[str, Any] | None = None,
        *,
        marginal_value_floor: float = 0.0,
        hypotheses: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Plan bounded evidence acquisition through HTTP without executing actions."""

        if isinstance(graph, LabPlanRequest):
            if actions is not None or budget is not None:
                raise ArgumentError("actions and budget must be omitted when graph is a LabPlanRequest")
            request = graph
        else:
            if actions is None or budget is None:
                raise ArgumentError("actions and budget are required when graph is a mapping")
            request = LabPlanRequest(graph, actions, budget, marginal_value_floor, hypotheses, observations, max_items)
        return self.call_tool("lab_plan", request.to_mcp_arguments())

    def routing_decide(
        self,
        fingerprint: Mapping[str, Any] | RoutingDecisionRequest,
        evidence: Sequence[Mapping[str, Any]] | None = None,
        policy: Mapping[str, Any] | None = None,
        *,
        task_id: str | None = None,
    ) -> dict[str, Any]:
        """Route an unseen task through HTTP without exposing a hidden oracle answer."""

        if isinstance(fingerprint, RoutingDecisionRequest):
            if evidence is not None or policy is not None or task_id is not None:
                raise ArgumentError("other routing arguments must be omitted when fingerprint is a RoutingDecisionRequest")
            request = fingerprint
        else:
            if evidence is None or policy is None:
                raise ArgumentError("evidence and policy are required when fingerprint is a mapping")
            request = RoutingDecisionRequest(fingerprint, evidence, policy, task_id)
        return self.call_tool("routing_decide", request.to_mcp_arguments())

    def repository_catalog(
        self,
        request: RepositoryCatalogRequest | None = None,
        *,
        prefix: str | None = None,
        limit: int = 200,
        include_briefs: bool = False,
        include_findings: bool = False,
    ) -> dict[str, Any]:
        """Discover repository modules through the HTTP gateway."""

        if request is not None:
            if prefix is not None or limit != 200 or include_briefs or include_findings:
                raise ArgumentError("catalog options must be omitted when passing a RepositoryCatalogRequest")
        else:
            request = RepositoryCatalogRequest(prefix, limit, include_briefs, include_findings)
        return self.call_tool("repository_catalog", request.to_mcp_arguments())

    def repository_bundle(
        self,
        route: Mapping[str, Any] | RepositoryBundleRequest,
        *,
        policy: RepositoryTraversalPolicy | str = RepositoryTraversalPolicy.NORMATIVE,
        max_depth: int | None = None,
        denied_labels: Sequence[str] = (),
        follow: Sequence[str] = (),
        include_markdown: bool = False,
        max_markdown_chars: int | None = None,
    ) -> dict[str, Any]:
        """Compile a route-specific repository context through HTTP."""

        if isinstance(route, RepositoryBundleRequest):
            if (
                policy not in (RepositoryTraversalPolicy.NORMATIVE, "normative")
                or max_depth is not None
                or denied_labels
                or follow
                or include_markdown
                or max_markdown_chars is not None
            ):
                raise ArgumentError("bundle options must be omitted when passing a RepositoryBundleRequest")
            request = route
        else:
            request = RepositoryBundleRequest(route, policy, max_depth, denied_labels, follow, include_markdown, max_markdown_chars)
        return self.call_tool("repository_bundle", request.to_mcp_arguments())

    def repository_impact(
        self,
        changed: str | RepositoryImpactRequest,
        *,
        route: Mapping[str, Any] | None = None,
        routes: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compute conservative repository impact through HTTP."""

        if isinstance(changed, RepositoryImpactRequest):
            if route is not None or routes is not None:
                raise ArgumentError("route and routes must be omitted when passing a RepositoryImpactRequest")
            request = changed
        else:
            request = RepositoryImpactRequest(changed, route, routes)
        return self.call_tool("repository_impact", request.to_mcp_arguments())

    def telemetry_project(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Project a redacted telemetry event through HTTP."""

        if isinstance(event, TelemetryProjectRequest):
            if policy is not None or trace is not None or metric is not None or observations is not None:
                raise ArgumentError("telemetry fields must be omitted when passing a TelemetryProjectRequest")
            request = event
        else:
            if policy is None or trace is None:
                raise ArgumentError("policy and trace are required when event is a mapping")
            request = TelemetryProjectRequest(event, policy, trace, metric, observations)
        return self.call_tool("telemetry_project", request.to_mcp_arguments())

    def fiber_compile(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> dict[str, Any]:
        """Compile a typed world/query pair through the HTTP gateway."""

        if isinstance(world, FiberCompileRequest):
            if query is not None or layer not in (ContextLayer.L0, "l0"):
                raise ArgumentError("query and layer must be omitted when passing a FiberCompileRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberCompileRequest(world, query, layer)
        return self.call_tool("fiber_compile", request.to_mcp_arguments())

    def fiber_refine(
        self,
        layer: ContextLayer | str | FiberRefineRequest,
        *,
        handle: Mapping[str, Any] | None = None,
        world: str | None = None,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Refine a compiled context through the HTTP gateway."""

        if isinstance(layer, FiberRefineRequest):
            if handle is not None or world is not None or query is not None:
                raise ArgumentError("source arguments must be omitted when passing a FiberRefineRequest")
            request = layer
        else:
            request = FiberRefineRequest(layer, handle, world, query)
        return self.call_tool("fiber_refine", request.to_mcp_arguments())

    def fiber_explain(
        self,
        world: str | FiberExplainRequest,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Request a compile-plan explanation through the HTTP gateway."""

        if isinstance(world, FiberExplainRequest):
            if query is not None:
                raise ArgumentError("query must be omitted when passing a FiberExplainRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberExplainRequest(world, query)
        return self.call_tool("fiber_explain", request.to_mcp_arguments())

    def fiber_verify(self, certificate: str | FiberVerifyRequest) -> dict[str, Any]:
        """Verify a context certificate through the HTTP gateway."""

        request = certificate if isinstance(certificate, FiberVerifyRequest) else FiberVerifyRequest(certificate)
        return self.call_tool("fiber_verify", request.to_mcp_arguments())

    def projection_bundle(
        self,
        world: str | ProjectionBundleRequest,
        query: str | None = None,
        *,
        include_views: bool = False,
    ) -> dict[str, Any]:
        """Request bounded context projections through the HTTP gateway."""

        if isinstance(world, ProjectionBundleRequest):
            if query is not None or include_views:
                raise ArgumentError("query and include_views must be omitted when passing a ProjectionBundleRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = ProjectionBundleRequest(world=world, query=query, include_views=include_views)
        return self.call_tool("projection_bundle", request.to_mcp_arguments())

    context_compile = fiber_compile
    context_refine = fiber_refine
    context_explain = fiber_explain
    context_verify = fiber_verify

    def events(self, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        if after < 0 or not 1 <= limit <= 1000:
            raise ArgumentError("after must be non-negative and limit must be 1..=1000")
        return self.request("GET", f"/v1/events?after={after}&limit={limit}")

    def subscribe(
        self,
        endpoint: str,
        secret: str,
        *,
        subscription_id: str | None = None,
        events: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {"endpoint": endpoint, "secret": secret}
        if subscription_id is not None:
            payload["id"] = subscription_id
        if events is not None:
            payload["events"] = list(events)
        return self.request("POST", "/v1/webhooks/subscriptions", payload)

    def deliveries(self, subscription_id: str, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        if after < 0 or not 1 <= limit <= 1000:
            raise ArgumentError("after must be non-negative and limit must be 1..=1000")
        return self.request("GET", f"/v1/webhooks/subscriptions/{subscription_id}/deliveries?after={after}&limit={limit}")

    def acknowledge(self, subscription_id: str, delivery_ids: Sequence[int]) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        return self.request("POST", f"/v1/webhooks/subscriptions/{subscription_id}/ack", {"delivery_ids": list(delivery_ids)})

    def retry(self, subscription_id: str, delivery_ids: Sequence[int]) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        return self.request("POST", f"/v1/webhooks/subscriptions/{subscription_id}/retry", {"delivery_ids": list(delivery_ids)})

    def delete_subscription(self, subscription_id: str) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        return self.request("DELETE", f"/v1/webhooks/subscriptions/{subscription_id}")

    @staticmethod
    def _subscription_id(value: str) -> None:
        if not isinstance(value, str) or not value or "/" in value or "\r" in value or "\n" in value:
            raise ArgumentError("subscription_id must be a non-empty path-safe string")


class AsyncApiClient:
    """Async facade over :class:`ApiClient`, using bounded worker threads for stdlib portability."""

    def __init__(self, client: ApiClient) -> None:
        self.client = client

    async def request(self, method: str, path: str, payload: Mapping[str, Any] | None = None, *, headers: Mapping[str, str] | None = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.request, method, path, payload, headers=headers)

    async def health(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.health)

    async def capabilities(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.capabilities)

    async def tools(self) -> list[dict[str, Any]]:
        return await asyncio.to_thread(self.client.tools)

    async def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.call_tool, name, arguments)

    async def tool_catalogue(self) -> ToolCatalogue:
        """Async snapshot of the authoritative live HTTP ``/v1/tools`` catalogue."""

        return ToolCatalogue.from_definitions(await self.tools())

    async def plan_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> ToolCallPlan:
        """Validate any advertised tool's JSON shape without issuing a POST."""

        snapshot = catalogue if catalogue is not None else await self.tool_catalogue()
        if not isinstance(snapshot, ToolCatalogue):
            raise ArgumentError("catalogue must be a ToolCatalogue")
        return snapshot.plan(name, arguments)

    async def tool_checked(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> dict[str, Any]:
        """Run any advertised tool after conservative schema preflight."""

        plan = await self.plan_tool(name, arguments, catalogue=catalogue)
        return await self.call_tool(plan.tool, plan.to_mcp_arguments())

    async def mission_preflight(
        self,
        request: MissionRequest,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> MissionPreflight:
        """Async review of a mission against the live HTTP tool catalogue."""

        if not isinstance(request, MissionRequest):
            raise ArgumentError("request must be a MissionRequest")
        snapshot = catalogue if catalogue is not None else await self.tool_catalogue()
        return preflight_mission(request, snapshot)

    async def capability_discover(
        self,
        *,
        query: CapabilityQuery | str | None = None,
        text: str | None = None,
        domain: str | None = None,
        tool: str | None = None,
        group_id: str | None = None,
        max_items: int = 50,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        arguments = _capability_query_arguments(
            query,
            text=text,
            domain=domain,
            tool=tool,
            group_id=group_id,
            max_items=max_items,
            include_tools=include_tools,
        )
        return await self.call_tool("capability_discover", arguments)

    async def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return await self.call_tool("capability_audit", {"include_groups": include_groups})

    async def capability_route(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return await self.call_tool("capability_route", request.to_mcp_arguments())

    async def adapter_plan(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.adapter_plan`."""

        request = AdapterPlanRequest(
            source_id,
            source_kind,
            declared_format,
            required_conformance,
            available_dependencies,
        )
        return await self.call_tool("adapter_plan", request.to_mcp_arguments())

    async def biocapability_evidence_audit(
        self,
        request: BioCapabilityEvidenceAuditRequest,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.biocapability_evidence_audit`."""

        if not isinstance(request, BioCapabilityEvidenceAuditRequest):
            raise ArgumentError("request must be a BioCapabilityEvidenceAuditRequest")
        return await self.call_tool("biocapability_evidence_audit", request.to_mcp_arguments())

    async def bioql_compile(
        self,
        query: str | BioQlCompileRequest,
        schema: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.bioql_compile`."""

        if isinstance(query, BioQlCompileRequest):
            if schema is not None:
                raise ArgumentError("schema must be omitted when query is a BioQlCompileRequest")
            request = query
        else:
            if schema is None:
                raise ArgumentError("schema is required when query is a string")
            request = BioQlCompileRequest(query, schema)
        return await self.call_tool("bioql_compile", request.to_mcp_arguments())

    async def world_claim_check(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.world_claim_check`."""

        if isinstance(provenance, WorldClaimCheckRequest):
            if claim is not None:
                raise ArgumentError("claim must be omitted when provenance is a WorldClaimCheckRequest")
            request = provenance
        else:
            if claim is None:
                raise ArgumentError("claim is required when provenance is a mapping")
            request = WorldClaimCheckRequest(provenance, claim)
        return await self.call_tool("world_claim_check", request.to_mcp_arguments())

    async def lab_plan(
        self,
        graph: Mapping[str, Any] | LabPlanRequest,
        actions: Sequence[Mapping[str, Any]] | None = None,
        budget: Mapping[str, Any] | None = None,
        *,
        marginal_value_floor: float = 0.0,
        hypotheses: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.lab_plan`."""

        if isinstance(graph, LabPlanRequest):
            if actions is not None or budget is not None:
                raise ArgumentError("actions and budget must be omitted when graph is a LabPlanRequest")
            request = graph
        else:
            if actions is None or budget is None:
                raise ArgumentError("actions and budget are required when graph is a mapping")
            request = LabPlanRequest(graph, actions, budget, marginal_value_floor, hypotheses, observations, max_items)
        return await self.call_tool("lab_plan", request.to_mcp_arguments())

    async def routing_decide(
        self,
        fingerprint: Mapping[str, Any] | RoutingDecisionRequest,
        evidence: Sequence[Mapping[str, Any]] | None = None,
        policy: Mapping[str, Any] | None = None,
        *,
        task_id: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.routing_decide`."""

        if isinstance(fingerprint, RoutingDecisionRequest):
            if evidence is not None or policy is not None or task_id is not None:
                raise ArgumentError("other routing arguments must be omitted when fingerprint is a RoutingDecisionRequest")
            request = fingerprint
        else:
            if evidence is None or policy is None:
                raise ArgumentError("evidence and policy are required when fingerprint is a mapping")
            request = RoutingDecisionRequest(fingerprint, evidence, policy, task_id)
        return await self.call_tool("routing_decide", request.to_mcp_arguments())

    async def repository_catalog(
        self,
        request: RepositoryCatalogRequest | None = None,
        *,
        prefix: str | None = None,
        limit: int = 200,
        include_briefs: bool = False,
        include_findings: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.repository_catalog`."""

        if request is not None:
            if prefix is not None or limit != 200 or include_briefs or include_findings:
                raise ArgumentError("catalog options must be omitted when passing a RepositoryCatalogRequest")
        else:
            request = RepositoryCatalogRequest(prefix, limit, include_briefs, include_findings)
        return await self.call_tool("repository_catalog", request.to_mcp_arguments())

    async def repository_bundle(
        self,
        route: Mapping[str, Any] | RepositoryBundleRequest,
        *,
        policy: RepositoryTraversalPolicy | str = RepositoryTraversalPolicy.NORMATIVE,
        max_depth: int | None = None,
        denied_labels: Sequence[str] = (),
        follow: Sequence[str] = (),
        include_markdown: bool = False,
        max_markdown_chars: int | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.repository_bundle`."""

        if isinstance(route, RepositoryBundleRequest):
            if (
                policy not in (RepositoryTraversalPolicy.NORMATIVE, "normative")
                or max_depth is not None
                or denied_labels
                or follow
                or include_markdown
                or max_markdown_chars is not None
            ):
                raise ArgumentError("bundle options must be omitted when passing a RepositoryBundleRequest")
            request = route
        else:
            request = RepositoryBundleRequest(route, policy, max_depth, denied_labels, follow, include_markdown, max_markdown_chars)
        return await self.call_tool("repository_bundle", request.to_mcp_arguments())

    async def repository_impact(
        self,
        changed: str | RepositoryImpactRequest,
        *,
        route: Mapping[str, Any] | None = None,
        routes: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.repository_impact`."""

        if isinstance(changed, RepositoryImpactRequest):
            if route is not None or routes is not None:
                raise ArgumentError("route and routes must be omitted when passing a RepositoryImpactRequest")
            request = changed
        else:
            request = RepositoryImpactRequest(changed, route, routes)
        return await self.call_tool("repository_impact", request.to_mcp_arguments())

    async def telemetry_project(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.telemetry_project`."""

        if isinstance(event, TelemetryProjectRequest):
            if policy is not None or trace is not None or metric is not None or observations is not None:
                raise ArgumentError("telemetry fields must be omitted when passing a TelemetryProjectRequest")
            request = event
        else:
            if policy is None or trace is None:
                raise ArgumentError("policy and trace are required when event is a mapping")
            request = TelemetryProjectRequest(event, policy, trace, metric, observations)
        return await self.call_tool("telemetry_project", request.to_mcp_arguments())

    async def fiber_compile(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_compile`."""

        if isinstance(world, FiberCompileRequest):
            if query is not None or layer not in (ContextLayer.L0, "l0"):
                raise ArgumentError("query and layer must be omitted when passing a FiberCompileRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberCompileRequest(world, query, layer)
        return await self.call_tool("fiber_compile", request.to_mcp_arguments())

    async def fiber_refine(
        self,
        layer: ContextLayer | str | FiberRefineRequest,
        *,
        handle: Mapping[str, Any] | None = None,
        world: str | None = None,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_refine`."""

        if isinstance(layer, FiberRefineRequest):
            if handle is not None or world is not None or query is not None:
                raise ArgumentError("source arguments must be omitted when passing a FiberRefineRequest")
            request = layer
        else:
            request = FiberRefineRequest(layer, handle, world, query)
        return await self.call_tool("fiber_refine", request.to_mcp_arguments())

    async def fiber_explain(
        self,
        world: str | FiberExplainRequest,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_explain`."""

        if isinstance(world, FiberExplainRequest):
            if query is not None:
                raise ArgumentError("query must be omitted when passing a FiberExplainRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberExplainRequest(world, query)
        return await self.call_tool("fiber_explain", request.to_mcp_arguments())

    async def fiber_verify(self, certificate: str | FiberVerifyRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_verify`."""

        request = certificate if isinstance(certificate, FiberVerifyRequest) else FiberVerifyRequest(certificate)
        return await self.call_tool("fiber_verify", request.to_mcp_arguments())

    async def projection_bundle(
        self,
        world: str | ProjectionBundleRequest,
        query: str | None = None,
        *,
        include_views: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.projection_bundle`."""

        if isinstance(world, ProjectionBundleRequest):
            if query is not None or include_views:
                raise ArgumentError("query and include_views must be omitted when passing a ProjectionBundleRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = ProjectionBundleRequest(world=world, query=query, include_views=include_views)
        return await self.call_tool("projection_bundle", request.to_mcp_arguments())

    context_compile = fiber_compile
    context_refine = fiber_refine
    context_explain = fiber_explain
    context_verify = fiber_verify

    async def events(self, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.events, after=after, limit=limit)
