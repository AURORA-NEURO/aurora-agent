"""The single Aurora server specification both host profiles render from.

One :class:`AuroraServerSpec` is the neutral description of how to reach the
``bioprism-mcp`` launcher (stdio argv) or its remote endpoint (URL). Profile
compatibility decisions happen at render time, so Codex and Claude always draw
from the same spec — and therefore from one capability registry — instead of
drifting into per-host copies.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping
from urllib.parse import urlparse

from .envref import secret_leak_findings
from .errors import ConfigError, UnsafeCommandError, UnsupportedFeatureError
from .profiles import HTTP, STDIO
from .registry import validate_allowlist

#: The repository's MCP launch shape; ``.mcp.json`` at the repo root is the
#: source of truth this default mirrors. On Windows the built artifact gains
#: an ``.exe`` suffix; the adapter never probes the filesystem (offline by
#: contract), so callers pass the path appropriate for their checkout.
DEFAULT_AURORA_COMMAND = "./target/release/bioprism-mcp"

_SHELL_MARKERS = frozenset(";&|<>\r\n")

_SPEC_KEYS = frozenset({"name", "transport", "type", "command", "args", "env", "url", "headers", "allowlist"})

# Provider-specific settings that appear in real-world host configs but whose
# key names or semantics this adapter cannot prove compatible across profiles;
# each is refused by name rather than silently dropped.
_PROVIDER_SPECIFIC_KEYS: dict[str, str] = {
    "oauth": "host-managed OAuth metadata; acquire credentials outside generated config",
    "auth": "host-managed auth metadata; acquire credentials outside generated config",
    "bearer_token_env_var": "Codex-specific remote auth key; provide credentials host-side",
    "http_headers": "Codex-specific remote header key with version-dependent spelling",
    "headersHelper": "Claude-specific header helper executable; runs arbitrary commands host-side",
    "startup_timeout_ms": "per-version host timeout key; not part of the neutral contract",
    "startup_timeout_sec": "per-version host timeout key; not part of the neutral contract",
    "tool_timeout_ms": "per-version host timeout key; not part of the neutral contract",
    "tool_timeout_sec": "per-version host timeout key; not part of the neutral contract",
    "timeout": "host-specific request timeout; not part of the neutral contract",
    "cwd": "host-specific working directory override; not part of the neutral contract",
    "enabled_tools": "host-specific tool filter; use the registry-backed allowlist instead",
    "disabled_tools": "host-specific tool filter; use the registry-backed allowlist instead",
}

_TYPE_ALIASES = {"stdio": STDIO, "http": HTTP, "streamable-http": HTTP}


@dataclass(frozen=True)
class AuroraServerSpec:
    """Validated, immutable description of one Aurora MCP endpoint."""

    name: str
    kind: str  # "stdio" | "http"
    command: str | None = None
    args: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = field(default=())
    url: str | None = None
    headers: tuple[tuple[str, str], ...] = ()
    allowlist: tuple[str, ...] | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.name, str) or not self.name.strip():
            raise ConfigError("server name must be a non-empty string")
        if self.kind not in (STDIO, HTTP):
            raise ConfigError(f"server {self.name!r} transport must be 'stdio' or 'http'")
        if self.kind == STDIO:
            self._validate_stdio()
        else:
            self._validate_http()
        if self.allowlist is not None:
            object.__setattr__(self, "allowlist", validate_allowlist(self.allowlist))

    def _validate_stdio(self) -> None:
        if not isinstance(self.command, str) or not self.command.strip():
            raise ConfigError(f"server {self.name!r}.command must be a non-empty string for stdio")
        if any(marker in self.command for marker in _SHELL_MARKERS):
            raise UnsafeCommandError(
                f"server {self.name!r}.command contains shell syntax; hosts launch argv without a shell"
            )
        for index, arg in enumerate(self.args):
            if not isinstance(arg, str):
                raise ConfigError(f"server {self.name!r}.args[{index}] must be a string")
        _reject_leaks(self.name, dict(self.env), "env")
        if self.url is not None:
            raise ConfigError(f"server {self.name!r} cannot define both command and url")

    def _validate_http(self) -> None:
        if not isinstance(self.url, str) or not self.url.strip():
            raise ConfigError(f"server {self.name!r}.url must be a non-empty string for http")
        parsed = urlparse(self.url)
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            raise ConfigError(f"server {self.name!r}.url must be an absolute http or https URL")
        if self.command is not None:
            raise ConfigError(f"server {self.name!r} cannot define both command and url")
        _reject_leaks(self.name, dict(self.headers), "headers")


def _reject_leaks(name: str, mapping: Mapping[str, object], section: str) -> None:
    findings = secret_leak_findings(name, mapping, section)
    if findings:
        raise ConfigError(
            f"secret literal rejected at {findings[0].location}; "
            "carry it as a ${VAR} placeholder where the profile supports expansion"
        )


def _string_pairs(value: object, path: str) -> tuple[tuple[str, str], ...]:
    if not isinstance(value, Mapping):
        raise ConfigError(f"{path} must be a mapping of strings")
    pairs: list[tuple[str, str]] = []
    for key, item in sorted(value.items(), key=lambda pair: str(pair[0])):
        if not isinstance(key, str) or not isinstance(item, str):
            raise ConfigError(f"{path} must contain only string keys and values")
        pairs.append((key, item))
    return tuple(pairs)


def _string_tuple(value: object, path: str) -> tuple[str, ...]:
    if not isinstance(value, (list, tuple)) or not all(isinstance(item, str) for item in value):
        raise ConfigError(f"{path} must be an array of strings")
    return tuple(value)


def aurora_stdio_spec(
    name: str = "aurora",
    root: str = ".",
    *,
    command: str = DEFAULT_AURORA_COMMAND,
    env: Mapping[str, str] | None = None,
    allowlist: tuple[str, ...] | list[str] | None = None,
) -> AuroraServerSpec:
    """Build the repository's documented stdio launch shape: ``<command> --root <dir>``."""
    return AuroraServerSpec(
        name=name,
        kind=STDIO,
        command=command,
        args=("--root", root),
        env=_string_pairs(env or {}, f"server {name!r}.env"),
        allowlist=None if allowlist is None else tuple(allowlist),
    )


def aurora_http_spec(
    name: str,
    url: str,
    *,
    headers: Mapping[str, str] | None = None,
    allowlist: tuple[str, ...] | list[str] | None = None,
) -> AuroraServerSpec:
    """Build a remote-endpoint specification; configuration only, no connectivity implied."""
    return AuroraServerSpec(
        name=name,
        kind=HTTP,
        url=url,
        headers=_string_pairs(headers or {}, f"server {name!r}.headers"),
        allowlist=None if allowlist is None else tuple(allowlist),
    )


def spec_from_mapping(entry: Mapping[str, Any]) -> AuroraServerSpec:
    """Parse a neutral specification mapping, refusing anything provider-specific."""
    if not isinstance(entry, Mapping):
        raise ConfigError("server specification must be a mapping")
    unknown_specific = sorted(set(entry) & set(_PROVIDER_SPECIFIC_KEYS))
    if unknown_specific:
        key = unknown_specific[0]
        raise UnsupportedFeatureError(
            f"{key!r} is a provider-specific setting this adapter refuses: {_PROVIDER_SPECIFIC_KEYS[key]}"
        )
    unknown = sorted(set(entry) - _SPEC_KEYS - {"type"})
    if unknown:
        raise ConfigError(f"unsupported specification keys: {', '.join(map(str, unknown))}")
    raw_kind = entry.get("transport", entry.get("type"))
    if raw_kind is None:
        raise ConfigError("specification needs transport 'stdio' or 'http'")
    if not isinstance(raw_kind, str) or raw_kind.lower() not in _TYPE_ALIASES:
        raise UnsupportedFeatureError(
            f"transport {raw_kind!r} is not one of 'stdio'/'http'; SSE and WebSocket shapes are refused"
        )
    kind = _TYPE_ALIASES[raw_kind.lower()]
    # Cross-kind keys must be refused, not ignored: a stdio entry carrying url
    # is almost certainly a mistake, and silently dropping it would emit
    # configuration for an endpoint the caller did not describe.
    if kind == STDIO:
        stray = sorted(set(entry) & {"url", "headers"})
        if stray:
            raise ConfigError(f"stdio specification carries http-only keys: {', '.join(stray)}")
    else:
        stray = sorted(set(entry) & {"command", "args", "env"})
        if stray:
            raise ConfigError(f"http specification carries stdio-only keys: {', '.join(stray)}")
    name = entry.get("name")
    common: dict[str, Any] = {}
    if "allowlist" in entry:
        raw_allowlist = entry["allowlist"]
        if raw_allowlist is not None and (
            isinstance(raw_allowlist, str) or not isinstance(raw_allowlist, (list, tuple))
        ):
            raise ConfigError("allowlist must be null or an array of capability names")
        common["allowlist"] = None if raw_allowlist is None else tuple(raw_allowlist)
    if kind == STDIO:
        return AuroraServerSpec(
            name=name,
            kind=STDIO,
            command=entry.get("command"),
            args=_string_tuple(entry.get("args", ()), "args"),
            env=_string_pairs(entry.get("env", {}), "env"),
            **common,
        )
    return AuroraServerSpec(
        name=name,
        kind=HTTP,
        url=entry.get("url"),
        headers=_string_pairs(entry.get("headers", {}), "headers"),
        **common,
    )
