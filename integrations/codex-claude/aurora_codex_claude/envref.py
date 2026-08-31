"""Environment-variable placeholder recognition and secret-leak scanning.

Values are never resolved here. A placeholder is data that the host may expand
at connect time (documented for Claude-style ``.mcp.json``) or a literal that
reaches the server unchanged (the only behaviour we can assume for Codex-style
``config.toml``); either way nothing a user would call a secret is ever
persisted by generation, validation or serialization.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Mapping

# Full-string references only: ${VAR} and ${VAR:-default}. Embedded references
# (prefix-${HOME}) are tracked separately because they cannot be redacted
# wholesale if they ever leaked.
_PLACEHOLDER = re.compile(r"^\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-[^{}]*)?\}$")
_ANY_REFERENCE = re.compile(r"\$\{([^{}]*)\}")

#: Substrings whose presence in an env/header *name* marks the value as
#: secret-bearing. Used fail-closed: a name that merely looks sensitive is
#: treated as sensitive.
SECRET_NAME_HINTS: tuple[str, ...] = (
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSPHRASE",
    "CREDENTIAL",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "AUTHORIZATION",
    "COOKIE",
    "SESSION",
)


@dataclass(frozen=True)
class ValueClass:
    """How one scalar will behave when a host reads the generated entry."""

    kind: str  # "placeholder" | "literal"
    variable: str | None


def classify_value(value: object) -> ValueClass:
    """Classify one scalar as a whole-value placeholder or a literal."""
    if not isinstance(value, str):
        return ValueClass("literal", None)
    match = _PLACEHOLDER.match(value)
    if match:
        return ValueClass("placeholder", match.group(1))
    return ValueClass("literal", None)


def is_secret_bearing_name(name: str) -> bool:
    """True when a mapping key names something that must never be persisted literally.

    Hyphens normalize to underscores before matching, so ``Api-Key`` cannot
    slip past a hint written as ``API_KEY``; the scan stays fail-closed.
    """
    upper = name.upper().replace("-", "_")
    return any(hint in upper for hint in SECRET_NAME_HINTS)


def referenced_variables(value: object) -> set[str]:
    """Every env-var name referenced anywhere inside a scalar (embedded included)."""
    if not isinstance(value, str):
        return set()
    names: set[str] = set()
    for match in _ANY_REFERENCE.finditer(value):
        inner = match.group(1)
        inner = inner.split(":-", 1)[0]
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", inner):
            names.add(inner)
    return names


def any_placeholder(values: Mapping[str, object]) -> str | None:
    """First field name carrying a placeholder reference, or None.

    Profiles whose host does not document placeholder expansion call this to
    find the offending field before emitting a value that would reach the
    server as the literal text ``${VAR}``.
    """
    for field in sorted(values):
        if referenced_variables(values[field]):
            return field
    return None


@dataclass(frozen=True)
class LeakFinding:
    """One place where a literal value sits under a secret-bearing name."""

    location: str  # dotted path inside the spec, e.g. aurora.env.AURORA_TOKEN
    detail: str


def secret_leak_findings(spec_name: str, mapping: Mapping[str, object], section: str) -> tuple[LeakFinding, ...]:
    """Report every literal value stored under a secret-bearing name.

    Generation refuses such entries outright; serialization of anything else
    still runs this scan so a planted literal cannot pass silently.
    """
    findings: list[LeakFinding] = []
    for name in sorted(mapping):
        value = mapping[name]
        if isinstance(value, Mapping):
            findings.extend(secret_leak_findings(spec_name, value, f"{section}.{name}"))
            continue
        if is_secret_bearing_name(name) and classify_value(value).kind == "literal":
            findings.append(
                LeakFinding(
                    location=f"{spec_name}.{section}.{name}",
                    detail="literal value under a secret-bearing name; use a ${VAR} placeholder",
                )
            )
    return tuple(findings)


def assert_no_resolved_secrets(text: str, environ: Mapping[str, str]) -> None:
    """Fail if the current process environment leaked into rendered output.

    Every dump function calls this with ``os.environ`` as a belt-and-braces
    check: even a bug that resolved a placeholder could not publish the resolved
    value unnoticed. The check covers every variable this process can see,
    not only referenced ones, because the leak path would be unknown by
    construction — that is why it is asserted rather than enumerated.
    """
    lowered = text.lower()
    for name, value in environ.items():
        if len(value) >= 8 and is_secret_bearing_name(name) and value.lower() in lowered:
            raise AssertionError(
                f"rendered output contains the value of secret-bearing environment variable {name}"
            )
