"""Profile-aware rendering: one Aurora spec, per-host configuration documents.

Every refusal in this module is the point, not a limitation: where a host's
documented surface cannot be proven to carry a value (placeholders on Codex,
headers on Codex remote entries, per-tool allowlists outside Claude), the
renderer raises :class:`UnsupportedFeatureError` instead of emitting
configuration whose semantics nobody verified.
"""

from __future__ import annotations

from typing import Iterable

from .envref import referenced_variables
from .errors import ConfigError, UnsupportedFeatureError
from .profiles import HTTP, STDIO, ProviderProfile, resolve_profile
from .serialize import dump_json, dump_toml
from .spec import AuroraServerSpec


def _first_placeholder_field(spec: AuroraServerSpec) -> str | None:
    """First field of the spec that carries an env-var reference, if any."""
    scalars: dict[str, object] = {}
    if spec.kind == STDIO:
        scalars["command"] = spec.command or ""
        for index, arg in enumerate(spec.args):
            scalars[f"args[{index}]"] = arg
        for key, value in spec.env:
            scalars[f"env.{key}"] = value
    else:
        scalars["url"] = spec.url or ""
        for key, value in spec.headers:
            scalars[f"headers.{key}"] = value
    for field_name in sorted(scalars):
        if referenced_variables(scalars[field_name]):
            return field_name
    return None


def render_entry(profile: ProviderProfile | str, spec: AuroraServerSpec) -> dict:
    """Render one server entry in the profile's documented shape.

    The entry is a plain mapping; serialization happens separately so tests can
    inspect structure and bytes independently.
    """
    profile = resolve_profile(profile)
    if spec.kind not in profile.transports:
        raise UnsupportedFeatureError(
            f"profile '{profile.profile_id}' does not accept {spec.kind} entries"
        )
    if not profile.expands_placeholders:
        field_with_reference = _first_placeholder_field(spec)
        if field_with_reference is not None:
            raise UnsupportedFeatureError(
                f"profile 'codex' does not document environment-placeholder expansion; "
                f"{spec.name}.{field_with_reference} carries a ${{VAR}} reference. "
                "Provide the variable in the server process environment instead."
            )
    if spec.kind == HTTP and spec.headers:
        permitted = set(profile.http_entry_keys)
        if "headers" not in permitted:
            raise UnsupportedFeatureError(
                f"profile '{profile.profile_id}' accepts only "
                f"{', '.join(sorted(permitted))} on remote entries; headers are refused "
                "because remote header key names vary across host versions"
            )
    if spec.kind == STDIO:
        entry: dict = {"command": spec.command, "args": list(spec.args)}
        # Claude-style project files mirror the repository .mcp.json contract,
        # which always states env; Codex tables omit empty inline tables.
        if spec.env or profile.config_format == "json":
            entry["env"] = {key: value for key, value in spec.env}
        return entry
    if "type" in profile.http_entry_keys:
        return {"type": "http", "url": spec.url, "headers": {key: value for key, value in spec.headers}}
    return {"url": spec.url}


def _render_document(profile: ProviderProfile, specs: tuple[AuroraServerSpec, ...]) -> dict:
    servers: dict[str, dict] = {}
    seen: set[str] = set()
    for spec in specs:
        if spec.name in seen:
            raise ConfigError(f"duplicate server name {spec.name!r}")
        seen.add(spec.name)
        servers[spec.name] = render_entry(profile, spec)
    return {profile.servers_key: servers}


def render_config(profile: ProviderProfile | str, specs: AuroraServerSpec | Iterable[AuroraServerSpec]) -> str:
    """Emit the full configuration document text for a profile.

    Output bytes depend only on the specs: sorted keys, LF newlines, a trailing
    newline for TOML and none for JSON (mirroring integrations/hermes).
    """
    profile = resolve_profile(profile)
    normalized = (specs,) if isinstance(specs, AuroraServerSpec) else tuple(specs)
    if not normalized:
        raise ConfigError("at least one server specification is required")
    document = _render_document(profile, normalized)
    if profile.config_format == "json":
        return dump_json(document)
    if profile.config_format == "toml":
        return dump_toml(document)
    raise UnsupportedFeatureError(f"profile '{profile.profile_id}' names unknown format {profile.config_format!r}")


def permission_entries(profile: ProviderProfile | str, spec: AuroraServerSpec) -> tuple[str, ...]:
    """Host-side per-tool permission strings derived from the registry-backed allowlist.

    Only emitted where the host documents such a surface; elsewhere the
    allowlist stays adapter-side rather than being written into a key the host
    would ignore.
    """
    profile = resolve_profile(profile)
    if profile.tool_permission_pattern is None:
        raise UnsupportedFeatureError(
            f"profile '{profile.profile_id}' has no proven per-server tool allowlist surface; "
            "the validated allowlist stays adapter-side"
        )
    if spec.allowlist is None:
        raise ConfigError(
            "allowlist is None (unrestricted); permission entries require an explicit "
            "registry-backed allowlist"
        )
    return tuple(f"mcp__{spec.name}__{capability}" for capability in spec.allowlist)
