"""Asyncio MCP client for the shipped Prism stdio server."""

from __future__ import annotations

import asyncio
import os
import time
from pathlib import Path
from typing import Any, Mapping, Sequence

from ._jsonrpc import (
    DEFAULT_MAX_FRAME_BYTES,
    MCP_PROTOCOL_VERSION,
    frame,
    notification_object,
    object_argument,
    parse_response,
    request_object,
)
from .errors import ArgumentError, LifecycleError, ProcessExited, ProtocolError, RemoteError, ResponseTimeout, TransportError
from .models import JsonObject, Session, ToolResult


class AsyncClient:
    """Asyncio-native counterpart to :class:`prism_sdk.client.Client`."""

    def __init__(
        self,
        command: Sequence[str],
        *,
        cwd: str | os.PathLike[str] | None = None,
        env: Mapping[str, str] | None = None,
        timeout: float = 30.0,
        max_frame_bytes: int = DEFAULT_MAX_FRAME_BYTES,
        client_name: str = "prism-sdk",
        client_version: str = "0.1.0",
    ) -> None:
        if not command or any(not isinstance(part, str) or not part for part in command):
            raise ArgumentError("command must contain at least one non-empty string")
        if timeout <= 0 or max_frame_bytes <= 0:
            raise ArgumentError("timeout and max_frame_bytes must be positive")
        self.command = tuple(command)
        self.cwd = str(Path(cwd)) if cwd is not None else None
        self.env = dict(env) if env is not None else None
        self.timeout = timeout
        self.max_frame_bytes = max_frame_bytes
        self.client_name = client_name
        self.client_version = client_version
        self._process: asyncio.subprocess.Process | None = None
        self._request_id = 0
        self._initialized = False
        self._session: Session | None = None
        self._stderr_task: asyncio.Task[None] | None = None
        self._stderr_chunks: list[str] = []
        self._lock = asyncio.Lock()

    @property
    def session(self) -> Session | None:
        return self._session

    @property
    def running(self) -> bool:
        return self._process is not None and self._process.returncode is None

    async def start(self) -> "AsyncClient":
        if self._process is not None:
            raise LifecycleError("client has already been started")
        env = None
        if self.env is not None:
            env = os.environ.copy()
            env.update(self.env)
        try:
            self._process = await asyncio.create_subprocess_exec(
                *self.command,
                cwd=self.cwd,
                env=env,
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
        except (OSError, ValueError) as error:
            raise TransportError(f"could not start MCP command: {error}") from error
        assert self._process.stderr is not None
        self._stderr_task = asyncio.create_task(self._drain_stderr(self._process.stderr))
        return self

    async def connect(self, params: Mapping[str, Any] | None = None) -> Session:
        if self._process is None:
            await self.start()
        return await self.initialize(params)

    async def initialize(self, params: Mapping[str, Any] | None = None) -> Session:
        if self._process is None:
            raise LifecycleError("start or connect must be called before initialize")
        if self._initialized:
            assert self._session is not None
            return self._session
        supplied = object_argument(params or {}, "initialize params")
        defaults: JsonObject = {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": self.client_name, "version": self.client_version},
        }
        defaults.update(supplied)
        response = await self._request("initialize", defaults)
        result = response.get("result")
        if not isinstance(result, Mapping):
            raise ProtocolError("initialize response has no result object")
        protocol_version = result.get("protocolVersion")
        capabilities = result.get("capabilities", {})
        server_info = result.get("serverInfo", {})
        if not isinstance(protocol_version, str) or not protocol_version:
            raise ProtocolError("initialize response has no protocolVersion")
        if not isinstance(capabilities, Mapping) or not isinstance(server_info, Mapping):
            raise ProtocolError("initialize response has invalid capabilities or serverInfo")
        await self.notify("notifications/initialized", {})
        self._session = Session(protocol_version, dict(server_info), dict(capabilities), dict(result))
        self._initialized = True
        return self._session

    async def notify(self, method: str, params: Mapping[str, Any] | None = None) -> None:
        await self._write(frame(notification_object(method, params), self.max_frame_bytes))

    async def request(self, method: str, params: Mapping[str, Any] | None = None) -> JsonObject:
        return await self._request(method, object_argument(params or {}, "params"))

    async def list_tools(self) -> list[JsonObject]:
        self._require_initialized()
        response = await self._request("tools/list", {})
        result = response.get("result")
        if not isinstance(result, Mapping) or not isinstance(result.get("tools"), list):
            raise ProtocolError("tools/list response has no tools array")
        return [dict(tool) for tool in result["tools"] if isinstance(tool, Mapping)]

    async def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
        self._require_initialized()
        if not isinstance(name, str) or not name:
            raise ArgumentError("tool name must be a non-empty string")
        response = await self._request(
            "tools/call",
            {"name": name, "arguments": object_argument(arguments or {}, "tool arguments")},
        )
        return ToolResult.from_response(name, response)

    async def read_resource(self, uri: str) -> JsonObject:
        self._require_initialized()
        if not isinstance(uri, str) or not uri:
            raise ArgumentError("resource uri must be a non-empty string")
        response = await self._request("resources/read", {"uri": uri})
        result = response.get("result")
        if not isinstance(result, Mapping):
            raise ProtocolError("resources/read response has no result object")
        return dict(result)

    async def close(self) -> None:
        process, self._process = self._process, None
        self._initialized = False
        self._session = None
        if process is None:
            return
        if process.stdin is not None:
            process.stdin.close()
        if process.returncode is None:
            process.terminate()
            try:
                await asyncio.wait_for(process.wait(), timeout=min(self.timeout, 2.0))
            except asyncio.TimeoutError:
                process.kill()
                await process.wait()
        if self._stderr_task is not None:
            await asyncio.gather(self._stderr_task, return_exceptions=True)
            self._stderr_task = None

    def stderr(self) -> str:
        return "".join(self._stderr_chunks)[-64_000:]

    def _require_initialized(self) -> None:
        if self._process is None:
            raise LifecycleError("client is not started")
        if not self._initialized:
            raise LifecycleError("client is not initialized")

    async def _write(self, data: bytes) -> None:
        process = self._process
        if process is None or process.stdin is None:
            raise LifecycleError("client is not started")
        if process.returncode is not None:
            raise ProcessExited(process.returncode, self.stderr())
        try:
            process.stdin.write(data)
            await process.stdin.drain()
        except (BrokenPipeError, ConnectionError) as error:
            raise TransportError(f"could not write to MCP process: {error}") from error

    async def _request(self, method: str, params: Mapping[str, Any]) -> JsonObject:
        process = self._process
        if process is None or process.stdout is None:
            raise LifecycleError("client is not started")
        async with self._lock:
            self._request_id += 1
            request_id = self._request_id
            await self._write(frame(request_object(request_id, method, params), self.max_frame_bytes))
            deadline = time.monotonic() + self.timeout
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ResponseTimeout(method, self.timeout)
                try:
                    raw = await asyncio.wait_for(process.stdout.readline(), timeout=remaining)
                except asyncio.TimeoutError as error:
                    raise ResponseTimeout(method, self.timeout) from error
                if not raw:
                    raise ProcessExited(process.returncode, self.stderr())
                response = parse_response(raw, self.max_frame_bytes)
                if "id" not in response:
                    continue
                if response.get("id") != request_id:
                    raise ProtocolError(
                        f"response id {response.get('id')!r} does not match request {request_id}"
                    )
                error_member = response.get("error")
                if isinstance(error_member, Mapping):
                    code = error_member.get("code")
                    message = error_member.get("message")
                    if not isinstance(code, int) or not isinstance(message, str):
                        raise ProtocolError("JSON-RPC error has invalid code or message")
                    raise RemoteError(code, message, error_member.get("data"))
                if "result" not in response:
                    raise ProtocolError("JSON-RPC response has neither result nor error")
                return response

    async def _drain_stderr(self, stream: asyncio.StreamReader) -> None:
        while True:
            raw = await stream.readline()
            if not raw:
                return
            self._stderr_chunks.append(raw.decode("utf-8", errors="replace"))
            if len(self._stderr_chunks) > 64:
                del self._stderr_chunks[:-64]

    async def __aenter__(self) -> "AsyncClient":
        try:
            await self.connect()
        except BaseException:
            await self.close()
            raise
        return self

    async def __aexit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        await self.close()
