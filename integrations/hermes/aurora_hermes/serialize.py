"""Deterministic, secret-safe config serialization."""

from __future__ import annotations

import json
import os
from typing import Any, Mapping

from .config import ServerSpec, validate_config
from .envref import assert_no_resolved_secrets, is_secret_bearing_name
from .errors import ConfigError, YamlSupportUnavailable


def normalized_mapping(config: Mapping[str, Any]) -> dict[str, Any]:
    specs = validate_config(config)
    return {"mcp_servers": {spec.name: spec.as_mapping() for spec in specs}}


def dump_json(config: Mapping[str, Any]) -> str:
    text = json.dumps(normalized_mapping(config), ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    assert_no_resolved_secrets(text, os.environ)
    return text


def load_json(text: str) -> dict[str, Any]:
    try:
        value = json.loads(text)
    except json.JSONDecodeError as error:
        raise ConfigError(f"invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise ConfigError("JSON root must be an object")
    return normalized_mapping(value)


def dump_yaml(config: Mapping[str, Any]) -> str:
    try:
        import yaml  # type: ignore[import-not-found]
    except ImportError as error:
        raise YamlSupportUnavailable("install PyYAML to use YAML serialization") from error
    text = yaml.safe_dump(normalized_mapping(config), sort_keys=True, allow_unicode=True)
    assert_no_resolved_secrets(text, os.environ)
    return text


def load_yaml(text: str) -> dict[str, Any]:
    try:
        import yaml  # type: ignore[import-not-found]
    except ImportError as error:
        raise YamlSupportUnavailable("install PyYAML to load YAML") from error
    value = yaml.safe_load(text)
    if not isinstance(value, dict):
        raise ConfigError("YAML root must be an object")
    return normalized_mapping(value)


def redacted_mapping(value: Any, key: str | None = None) -> Any:
    """Return a diagnostic copy that never repeats a literal under a secret-like key."""
    if isinstance(value, Mapping):
        return {str(k): redacted_mapping(v, str(k)) for k, v in value.items()}
    if isinstance(value, list):
        return [redacted_mapping(item, key) for item in value]
    if key is not None and is_secret_bearing_name(key):
        return "<redacted>"
    return value
