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

from .errors import ApiError, ArgumentError, TransportError


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

    async def events(self, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.events, after=after, limit=limit)
