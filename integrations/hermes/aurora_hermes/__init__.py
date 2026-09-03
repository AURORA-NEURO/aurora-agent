"""Aurora's dependency-light Hermes MCP configuration adapter."""

from .config import ServerSpec, validate_config, validate_server
from .diagnostics import Readiness, diagnose, diagnose_all
from .errors import (
    ConfigError,
    HermesIntegrationError,
    ProbeError,
    UnsafeCommandError,
    UnsupportedFeatureError,
    YamlSupportUnavailable,
)
from .serialize import dump_json, dump_yaml, load_json, load_yaml, normalized_mapping, redacted_mapping

__all__ = [
    "ConfigError",
    "HermesIntegrationError",
    "ProbeError",
    "Readiness",
    "ServerSpec",
    "UnsafeCommandError",
    "UnsupportedFeatureError",
    "YamlSupportUnavailable",
    "diagnose",
    "diagnose_all",
    "dump_json",
    "dump_yaml",
    "load_json",
    "load_yaml",
    "normalized_mapping",
    "redacted_mapping",
    "validate_config",
    "validate_server",
]
