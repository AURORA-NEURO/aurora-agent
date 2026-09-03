"""Typed provider profiles for Codex-style and Claude-style MCP hosts.

A profile states, as data, exactly what a host's configuration surface can
carry *as far as this adapter could verify*. Anything outside the declared
surface is refused by :mod:`aurora_codex_claude.render` rather than dropped,
so a generated file never contains a key whose semantics we could not prove.

The declarations are conservative on purpose:

- Claude-style hosts document ``${VAR}`` / ``${VAR:-default}`` expansion in
  ``command``, ``args``, ``env``, ``url`` and ``headers`` of ``.mcp.json``
  entries, and accept ``{"type": "http", "url": ..., "headers": ...}``.
- Codex-style hosts document TOML ``[mcp_servers.<name>]`` tables with
  ``command``, ``args`` and ``env``. Placeholder expansion inside those values
  is not documented behaviour we can rely on, so this profile refuses
  placeholders instead of emitting values that might reach the server as the
  literal text ``${VAR}``. Remote entries are emitted URL-only: header keys
  for remote servers have used different names across versions, so no header
  can be proven compatible.
"""

from __future__ import annotations

from dataclasses import dataclass

STDIO = "stdio"
HTTP = "http"


@dataclass(frozen=True)
class ProviderProfile:
    """What one host family's configuration surface can carry."""

    profile_id: str
    display_name: str
    config_format: str  # "json" | "toml"
    servers_key: str  # mapping name holding server entries
    transports: frozenset[str]
    expands_placeholders: bool
    http_entry_keys: frozenset[str]  # keys permitted on an HTTP entry
    tool_permission_pattern: str | None  # host surface for per-tool allowlists, if proven
    notes: str


CLAUDE = ProviderProfile(
    profile_id="claude",
    display_name="Claude Code (.mcp.json)",
    config_format="json",
    servers_key="mcpServers",
    transports=frozenset({STDIO, HTTP}),
    expands_placeholders=True,
    http_entry_keys=frozenset({"type", "url", "headers"}),
    tool_permission_pattern="mcp__<server>__<tool>",
    notes=(
        "stdio entries are {command, args, env}; HTTP entries carry "
        '{"type": "http", url, headers}. ${VAR} and ${VAR:-default} placeholders '
        "are documented to expand in command/args/env/url/headers and are kept "
        "literal here; unset variables stay literal host-side."
    ),
)

CODEX = ProviderProfile(
    profile_id="codex",
    display_name="Codex CLI (config.toml)",
    config_format="toml",
    servers_key="mcp_servers",
    transports=frozenset({STDIO, HTTP}),
    expands_placeholders=False,
    # Remote support is URL-only: bearer/header key names have varied across
    # versions, so a header cannot be proven compatible.
    http_entry_keys=frozenset({"url"}),
    tool_permission_pattern=None,
    notes=(
        "stdio entries are [mcp_servers.<name>] with command/args/env; HTTP "
        "entries carry url only. Values pass through literally; provide secrets "
        "in the server process environment rather than via placeholder text."
    ),
)

PROFILES: dict[str, ProviderProfile] = {"codex": CODEX, "claude": CLAUDE}


def resolve_profile(profile: ProviderProfile | str) -> ProviderProfile:
    """Accept a profile instance or its id; anything else is a loud failure."""
    if isinstance(profile, ProviderProfile):
        return profile
    if isinstance(profile, str):
        try:
            return PROFILES[profile]
        except KeyError:
            known = ", ".join(sorted(PROFILES))
            raise ValueError(f"unknown provider profile {profile!r}; known profiles: {known}") from None
    raise TypeError(f"expected ProviderProfile or profile id, got {type(profile).__name__}")
