"""Deterministic, secret-safe serialization for generated host documents.

JSON output follows the canonical conventions of integrations/hermes (sorted
keys, compact separators, placeholders byte-for-byte visible). TOML output is
emitted by a hand-rolled writer because the package has no runtime
dependencies; it only has to express the narrow document shape the renderer
produces, and every string is escaped through ``json.dumps`` so control
characters can never break out of a TOML basic string.
"""

from __future__ import annotations

import json
import os
import re
from typing import Any, Mapping

from .envref import assert_no_resolved_secrets, is_secret_bearing_name
from .errors import ConfigError

# Canonical key order inside one server entry; anything outside this list is a
# renderer bug and is still emitted deterministically after these.
_ENTRY_KEY_ORDER: tuple[str, ...] = ("type", "command", "args", "env", "url", "headers")

_BARE_KEY = re.compile(r"^[A-Za-z0-9_-]+$")


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _toml_key(key: str) -> str:
    return key if _BARE_KEY.match(key) else _toml_string(key)


def _toml_value(value: Any) -> str:
    if isinstance(value, str):
        return _toml_string(value)
    if isinstance(value, bool) or not isinstance(value, (list, tuple, Mapping)):
        raise ConfigError(f"TOML writer only handles strings, arrays of strings and inline tables: {value!r}")
    if isinstance(value, Mapping):
        if not value:
            return "{}"
        items = ", ".join(f"{_toml_key(str(k))} = {_toml_value(v)}" for k, v in sorted(value.items(), key=lambda pair: str(pair[0])))
        return "{ " + items + " }"
    if not all(isinstance(item, str) for item in value):
        raise ConfigError("TOML arrays must contain only strings")
    return "[" + ", ".join(_toml_string(item) for item in value) + "]"


def dump_toml(document: Mapping[str, Any]) -> str:
    """Render a two-level document as deterministic TOML with LF newlines."""
    if not isinstance(document, Mapping) or not document:
        raise ConfigError("TOML document must be a non-empty mapping")
    lines: list[str] = []
    for section in sorted(document):
        entries = document[section]
        if not isinstance(entries, Mapping) or not entries:
            raise ConfigError(f"TOML section {section!r} must be a non-empty mapping")
        for name in sorted(entries):
            entry = entries[name]
            if not isinstance(entry, Mapping):
                raise ConfigError(f"TOML entry {section}.{name} must be a mapping")
            lines.append(f"[{_toml_key(str(section))}.{_toml_key(str(name))}]")
            keys = [key for key in _ENTRY_KEY_ORDER if key in entry] + sorted(set(entry) - set(_ENTRY_KEY_ORDER))
            for key in keys:
                lines.append(f"{_toml_key(key)} = {_toml_value(entry[key])}")
            lines.append("")
    text = "\n".join(lines)
    assert_no_resolved_secrets(text, os.environ)
    return text


def dump_json(document: Mapping[str, Any]) -> str:
    """Canonical JSON: sorted keys, compact separators, no resolved secrets."""
    text = json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    assert_no_resolved_secrets(text, os.environ)
    return text


def redacted_mapping(value: Any, key: str | None = None) -> Any:
    """Return a diagnostic copy that never repeats a literal under a secret-like key."""
    if isinstance(value, Mapping):
        return {str(k): redacted_mapping(v, str(k)) for k, v in value.items()}
    if isinstance(value, list):
        return [redacted_mapping(item, key) for item in value]
    if key is not None and is_secret_bearing_name(key):
        return "<redacted>"
    return value
