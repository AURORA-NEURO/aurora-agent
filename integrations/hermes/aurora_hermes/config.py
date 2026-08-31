"""Validation and immutable normalization for Hermes ``mcp_servers`` entries.

The validator accepts only argv-based stdio launchers and HTTP URL entries. It never resolves
environment placeholders and rejects literal values under secret-bearing names, so a generated
configuration can be persisted without becoming a credential store.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping
from urllib.parse import urlparse

from .envref import is_secret_bearing_name, secret_leak_findings
from .errors import ConfigError, UnsafeCommandError, UnsupportedFeatureError

_SHELL_MARKERS = frozenset(";&|<>\r\n")
_SUPPORTED_KEYS = frozenset({"command", "args", "env", "url", "headers", "transport"})


@dataclass(frozen=True)
class ServerSpec:
    """Validated, immutable server entry; secret placeholders remain literal references."""

    name: str
    kind: str  # ``stdio`` or ``http``
    command: str | None = None
    args: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = ()
    url: str | None = None
    headers: tuple[tuple[str, str], ...] = ()

    def as_mapping(self) -> dict[str, Any]:
        if self.kind == "stdio":
            return {
                "command": self.command,
                "args": list(self.args),
                "env": {key: value for key, value in self.env},
            }
        return {
            "url": self.url,
            "headers": {key: value for key, value in self.headers},
        }


def validate_config(config: Mapping[str, Any]) -> tuple[ServerSpec, ...]:
    if not isinstance(config, Mapping):
        raise ConfigError("Hermes configuration must be a mapping")
    raw_servers = config.get("mcp_servers")
    if raw_servers is None:
        raw_servers = config.get("mcpServers")
    if not isinstance(raw_servers, Mapping) or not raw_servers:
        raise ConfigError("mcp_servers must be a non-empty mapping")
    specs = tuple(validate_server(name, entry) for name, entry in sorted(raw_servers.items(), key=lambda item: str(item[0])))
    return specs


def validate_server(name: object, entry: object) -> ServerSpec:
    if not isinstance(name, str) or not name.strip():
        raise ConfigError("server names must be non-empty strings")
    if not isinstance(entry, Mapping):
        raise ConfigError(f"server {name!r} must be a mapping")
    if "oauth" in entry or "auth" in entry:
        raise UnsupportedFeatureError(f"server {name!r} requests OAuth/auth metadata; use a placeholder header")
    unknown = sorted(set(entry) - _SUPPORTED_KEYS)
    if unknown:
        raise ConfigError(f"server {name!r} has unsupported keys: {', '.join(map(str, unknown))}")
    if "command" in entry and "url" in entry:
        raise ConfigError(f"server {name!r} cannot define both command and url")
    if "url" in entry:
        return _validate_http(name, entry)
    if "command" not in entry:
        raise ConfigError(f"server {name!r} needs command for stdio or url for HTTP")
    return _validate_stdio(name, entry)


def _validate_stdio(name: str, entry: Mapping[str, Any]) -> ServerSpec:
    command = entry["command"]
    if not isinstance(command, str) or not command.strip():
        raise ConfigError(f"server {name!r}.command must be a non-empty string")
    if any(marker in command for marker in _SHELL_MARKERS):
        raise UnsafeCommandError(f"server {name!r}.command contains shell syntax; use argv without a shell")
    args = _string_tuple(entry.get("args", ()), f"server {name!r}.args")
    for index, arg in enumerate(args):
        if is_secret_bearing_name(arg.lstrip("-")):
            if index + 1 >= len(args) or "${" not in args[index + 1]:
                raise ConfigError(f"server {name!r}.args[{index}] appears to carry a literal secret")
    env = _string_pairs(entry.get("env", {}), f"server {name!r}.env")
    _reject_leaks(name, dict(env), "env")
    return ServerSpec(name=name, kind="stdio", command=command, args=args, env=env)


def _validate_http(name: str, entry: Mapping[str, Any]) -> ServerSpec:
    url = entry["url"]
    if not isinstance(url, str) or not url.strip():
        raise ConfigError(f"server {name!r}.url must be a non-empty string")
    parsed = urlparse(url)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise ConfigError(f"server {name!r}.url must be an absolute http or https URL")
    if entry.get("transport") not in (None, "http", "streamable-http"):
        raise UnsupportedFeatureError(f"server {name!r} requests unsupported transport {entry['transport']!r}")
    headers = _string_pairs(entry.get("headers", {}), f"server {name!r}.headers")
    _reject_leaks(name, dict(headers), "headers")
    return ServerSpec(name=name, kind="http", url=url, headers=headers)


def _string_tuple(value: object, path: str) -> tuple[str, ...]:
    if not isinstance(value, (list, tuple)) or not all(isinstance(item, str) for item in value):
        raise ConfigError(f"{path} must be an array of strings")
    return tuple(value)


def _string_pairs(value: object, path: str) -> tuple[tuple[str, str], ...]:
    if not isinstance(value, Mapping):
        raise ConfigError(f"{path} must be a mapping of strings")
    pairs: list[tuple[str, str]] = []
    for key, item in sorted(value.items(), key=lambda pair: str(pair[0])):
        if not isinstance(key, str) or not isinstance(item, str):
            raise ConfigError(f"{path} must contain only string keys and values")
        pairs.append((key, item))
    return tuple(pairs)


def _reject_leaks(name: str, mapping: Mapping[str, object], section: str) -> None:
    findings = secret_leak_findings(name, mapping, section)
    if findings:
        raise ConfigError(f"secret literal rejected at {findings[0].location}; use a ${'{'}VAR{'}'} placeholder")
