"""Aurora's dependency-light, provider-neutral Codex/Claude MCP adapter.

One :class:`AuroraServerSpec` and one capability registry feed both host
profiles; rendering refuses provider-specific settings that cannot be proven
compatible. This package emits configuration only: it performs no network
connectivity, no OAuth, no provider SDK execution and never resolves secrets.
"""

from .envref import (
    LeakFinding,
    SECRET_NAME_HINTS,
    ValueClass,
    assert_no_resolved_secrets,
    classify_value,
    is_secret_bearing_name,
    referenced_variables,
    secret_leak_findings,
)
from .errors import (
    CodexClaudeIntegrationError,
    ConfigError,
    UnsafeCommandError,
    UnknownCapabilityError,
    UnsupportedFeatureError,
)
from .profiles import CLAUDE, CODEX, PROFILES, HTTP, STDIO, ProviderProfile, resolve_profile
from .registry import AURORA_CAPABILITIES, REGISTRY_SNAPSHOT_SOURCE, validate_allowlist
from .render import permission_entries, render_config, render_entry
from .serialize import dump_json, dump_toml, redacted_mapping
from .spec import (
    DEFAULT_AURORA_COMMAND,
    AuroraServerSpec,
    aurora_http_spec,
    aurora_stdio_spec,
    spec_from_mapping,
)

__all__ = [
    "AURORA_CAPABILITIES",
    "AuroraServerSpec",
    "CLAUDE",
    "CODEX",
    "CodexClaudeIntegrationError",
    "ConfigError",
    "DEFAULT_AURORA_COMMAND",
    "HTTP",
    "LeakFinding",
    "PROFILES",
    "ProviderProfile",
    "REGISTRY_SNAPSHOT_SOURCE",
    "SECRET_NAME_HINTS",
    "STDIO",
    "UnsafeCommandError",
    "UnknownCapabilityError",
    "UnsupportedFeatureError",
    "ValueClass",
    "assert_no_resolved_secrets",
    "aurora_http_spec",
    "aurora_stdio_spec",
    "classify_value",
    "dump_json",
    "dump_toml",
    "is_secret_bearing_name",
    "permission_entries",
    "redacted_mapping",
    "referenced_variables",
    "render_config",
    "render_entry",
    "resolve_profile",
    "secret_leak_findings",
    "spec_from_mapping",
    "validate_allowlist",
]
