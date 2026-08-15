"""Synchronous MCP client for the shipped Prism stdio server."""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass
import os
import queue
import subprocess
import threading
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
from .errors import (
    ArgumentError,
    LifecycleError,
    ProcessExited,
    ProtocolError,
    RemoteError,
    ResponseTimeout,
    TransportError,
)
from .models import JsonObject, Session, ToolResult


@dataclass(frozen=True)
class ClientConfig:
    """Immutable process and framing settings used by :class:`Client`."""

    command: tuple[str, ...]
    cwd: str | None = None
    timeout: float = 30.0
    max_frame_bytes: int = DEFAULT_MAX_FRAME_BYTES

    def __post_init__(self) -> None:
        if not self.command or any(not isinstance(part, str) or not part for part in self.command):
            raise ArgumentError("command must contain at least one non-empty string")
        if self.timeout <= 0:
            raise ArgumentError("timeout must be positive")
        if self.max_frame_bytes <= 0:
            raise ArgumentError("max_frame_bytes must be positive")
        if self.cwd is not None and not isinstance(self.cwd, str):
            raise ArgumentError("cwd must be a string path or None")


class _LineReader:
    def __init__(self, stream: Any, max_frame_bytes: int, stderr: bool = False) -> None:
        self._stream = stream
        self._max_frame_bytes = max_frame_bytes
        self._stderr = stderr
        self._queue: queue.Queue[bytes | BaseException | None] = queue.Queue()
        self._recent_stderr: deque[str] = deque(maxlen=64)
        self._thread = threading.Thread(target=self._run, daemon=True)

    def start(self) -> None:
        self._thread.start()

    def _run(self) -> None:
        try:
            while True:
                line = self._stream.readline()
                if not line:
                    self._queue.put(None)
                    return
                if self._stderr:
                    self._recent_stderr.append(line.decode("utf-8", errors="replace"))
                    continue
                if len(line) > self._max_frame_bytes:
                    self._queue.put(
                        ProtocolError(
                            f"peer frame is {len(line)} bytes, over the "
                            f"{self._max_frame_bytes}-byte bound"
                        )
                    )
                    return
                self._queue.put(line)
        except BaseException as error:  # pragma: no cover - defensive transport boundary
            self._queue.put(error)

    def next(self, timeout: float) -> bytes | BaseException | None:
        return self._queue.get(timeout=timeout)

    def stderr(self) -> str:
        return "".join(self._recent_stderr)


class Client:
    """A lifecycle-safe synchronous client for an MCP server over newline-delimited stdio.

    ``Client`` never invokes a shell. The command is an argv sequence, paths remain the server's
    responsibility, and every request is bounded by a frame size and response timeout. Use
    ``with Client([...]) as client`` to make process cleanup automatic.
    """

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
        resolved_cwd = str(Path(cwd)) if cwd is not None else None
        self.config = ClientConfig(
            tuple(command),
            cwd=resolved_cwd,
            timeout=timeout,
            max_frame_bytes=max_frame_bytes,
        )
        self._env = dict(env) if env is not None else None
        self._client_name = client_name
        self._client_version = client_version
        self._process: subprocess.Popen[bytes] | None = None
        self._stdout: _LineReader | None = None
        self._stderr: _LineReader | None = None
        self._request_id = 0
        self._initialized = False
        self._session: Session | None = None
        self._lock = threading.RLock()

    @property
    def session(self) -> Session | None:
        return self._session

    @property
    def running(self) -> bool:
        return self._process is not None and self._process.poll() is None

    def start(self) -> "Client":
        with self._lock:
            if self._process is not None:
                raise LifecycleError("client has already been started")
            env = None
            if self._env is not None:
                env = os.environ.copy()
                env.update(self._env)
            try:
                self._process = subprocess.Popen(
                    list(self.config.command),
                    cwd=self.config.cwd,
                    env=env,
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    bufsize=0,
                    shell=False,
                )
            except OSError as error:
                self._process = None
                raise TransportError(f"could not start MCP command: {error}") from error
            assert self._process.stdout is not None
            assert self._process.stderr is not None
            self._stdout = _LineReader(self._process.stdout, self.config.max_frame_bytes)
            self._stderr = _LineReader(self._process.stderr, self.config.max_frame_bytes, stderr=True)
            self._stdout.start()
            self._stderr.start()
            return self

    def connect(self, params: Mapping[str, Any] | None = None) -> Session:
        if self._process is None:
            self.start()
        return self.initialize(params)

    def initialize(self, params: Mapping[str, Any] | None = None) -> Session:
        if self._process is None:
            raise LifecycleError("start or connect must be called before initialize")
        if self._initialized:
            assert self._session is not None
            return self._session
        supplied = object_argument(params or {}, "initialize params")
        defaults: JsonObject = {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": self._client_name, "version": self._client_version},
        }
        defaults.update(supplied)
        response = self._request("initialize", defaults)
        result = response.get("result")
        if not isinstance(result, Mapping):
            raise ProtocolError("initialize response has no result object")
        protocol_version = result.get("protocolVersion")
        if not isinstance(protocol_version, str) or not protocol_version:
            raise ProtocolError("initialize response has no protocolVersion")
        capabilities = result.get("capabilities", {})
        server_info = result.get("serverInfo", {})
        if not isinstance(capabilities, Mapping) or not isinstance(server_info, Mapping):
            raise ProtocolError("initialize response has invalid capabilities or serverInfo")
        self.notify("notifications/initialized", {})
        self._session = Session(protocol_version, dict(server_info), dict(capabilities), dict(result))
        self._initialized = True
        return self._session

    def notify(self, method: str, params: Mapping[str, Any] | None = None) -> None:
        self._write(frame(notification_object(method, params), self.config.max_frame_bytes))

    def request(self, method: str, params: Mapping[str, Any] | None = None) -> JsonObject:
        return self._request(method, object_argument(params or {}, "params"))

    def list_tools(self) -> list[JsonObject]:
        self._require_initialized()
        response = self._request("tools/list", {})
        result = response.get("result")
        if not isinstance(result, Mapping) or not isinstance(result.get("tools"), list):
            raise ProtocolError("tools/list response has no tools array")
        return [dict(tool) for tool in result["tools"] if isinstance(tool, Mapping)]

    def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
        self._require_initialized()
        if not isinstance(name, str) or not name:
            raise ArgumentError("tool name must be a non-empty string")
        args = object_argument(arguments or {}, "tool arguments")
        response = self._request("tools/call", {"name": name, "arguments": args})
        return ToolResult.from_response(name, response)

    def read_resource(self, uri: str) -> JsonObject:
        self._require_initialized()
        if not isinstance(uri, str) or not uri:
            raise ArgumentError("resource uri must be a non-empty string")
        response = self._request("resources/read", {"uri": uri})
        result = response.get("result")
        if not isinstance(result, Mapping):
            raise ProtocolError("resources/read response has no result object")
        return dict(result)

    def close(self) -> None:
        with self._lock:
            process, self._process = self._process, None
            stdout_reader, self._stdout = self._stdout, None
            stderr_reader, self._stderr = self._stderr, None
            self._initialized = False
            self._session = None
            if process is None:
                return
            try:
                if process.stdin is not None:
                    process.stdin.close()
            except OSError:
                pass
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=min(self.config.timeout, 2.0))
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=2.0)
            for stream in (process.stdout, process.stderr):
                if stream is not None:
                    stream.close()
            for reader in (stdout_reader, stderr_reader):
                if reader is not None:
                    reader._thread.join(timeout=1.0)

    def stderr(self) -> str:
        return self._stderr.stderr() if self._stderr is not None else ""

    def _require_initialized(self) -> None:
        if self._process is None:
            raise LifecycleError("client is not started")
        if not self._initialized:
            raise LifecycleError("client is not initialized")

    def _write(self, data: bytes) -> None:
        process = self._process
        if process is None or process.stdin is None:
            raise LifecycleError("client is not started")
        if process.poll() is not None:
            raise ProcessExited(process.returncode, self.stderr())
        try:
            process.stdin.write(data)
            process.stdin.flush()
        except OSError as error:
            raise TransportError(f"could not write to MCP process: {error}") from error

    def _request(self, method: str, params: Mapping[str, Any]) -> JsonObject:
        if self._process is None:
            raise LifecycleError("client is not started")
        with self._lock:
            self._request_id += 1
            request_id = self._request_id
            self._write(frame(request_object(request_id, method, params), self.config.max_frame_bytes))
            reader = self._stdout
            if reader is None:
                raise LifecycleError("client stdout reader is unavailable")
            deadline = time.monotonic() + self.config.timeout
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ResponseTimeout(method, self.config.timeout)
                try:
                    item = reader.next(remaining)
                except queue.Empty as error:
                    raise ResponseTimeout(method, self.config.timeout) from error
                if item is None:
                    process = self._process
                    raise ProcessExited(
                        process.poll() if process is not None else None,
                        self.stderr(),
                    )
                if isinstance(item, BaseException):
                    raise item
                response = parse_response(item, self.config.max_frame_bytes)
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

    def __enter__(self) -> "Client":
        try:
            self.connect()
        except BaseException:
            self.close()
            raise
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        self.close()
