"""Environment-variable placeholder recognition and secret-leak scanning.

Hermes resolves ``${VAR}`` and ``${env:VAR}`` from the active profile's secret
scope at connect time and keeps the literal placeholder when a variable is
unset. This adapter therefore treats placeholders as the only sanctioned way to
carry credentials through a config: values are never resolved here, so nothing
a user would call a secret is ever persisted by generation, validation or
serialization.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Mapping

# Hermes accepts ${VAR} and the Cursor-style ${env:VAR}; both resolve identically.
_PLACEHOLDER = re.compile(r"^\$\{(?:env:)?([A-Za-z_][A-Za-z0-9_]*)\}$")
_ANY_REFERENCE = re.compile(r"\$\{([^{}]*)\}")

# Cursor-style context variables documented for mcp_servers entries; case-sensitive.
CONTEXT_VARIABLES: frozenset[str] = frozenset(
    {"userHome", "workspaceFolder", "workspaceFolderBasename", "pathSeparator"}
)

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
    """How a scalar found in a server entry will behave at Hermes resolve time."""

    kind: str  # "placeholder" | "context" | "literal"
    variable: str | None


def classify_value(value: object) -> ValueClass:
    """Classify one scalar exactly as a full-string reference or a literal.

    Only a value that is *entirely* ``${...}`` counts as a reference; embedded
    references (``prefix-${HOME}``) are also interpolated by Hermes but are kept
    distinct here because they cannot be redacted wholesale.
    """
    if not isinstance(value, str):
        return ValueClass("literal", None)
    match = _PLACEHOLDER.match(value)
    if match:
        return ValueClass("context" if match.group(1) in CONTEXT_VARIABLES else "placeholder", match.group(1))
    return ValueClass("literal", None)


def is_secret_bearing_name(name: str) -> bool:
    """True when a mapping key names something that must never be persisted literally."""
    upper = name.upper()
    return any(hint in upper for hint in SECRET_NAME_HINTS)


def referenced_variables(value: object) -> set[str]:
    """Every env-var name referenced anywhere inside a scalar (embedded included)."""
    if not isinstance(value, str):
        return set()
    names: set[str] = set()
    for match in _ANY_REFERENCE.finditer(value):
        inner = match.group(1)
        if inner.startswith("env:"):
            inner = inner[4:]
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", inner):
            names.add(inner)
    return names


@dataclass(frozen=True)
class LeakFinding:
    """One place where a literal value sits under a secret-bearing name."""

    location: str  # dotted path inside the entry, e.g. servers.github.env.GITHUB_TOKEN
    detail: str


def secret_leak_findings(entry_name: str, mapping: Mapping[str, object], section: str) -> tuple[LeakFinding, ...]:
    """Report every literal value stored under a secret-bearing name.

    This is the scanner half of the leakage contract: generation refuses such
    entries and serialization of a policy-permitted entry still reports them, so
    a planted literal cannot pass silently.
    """
    findings: list[LeakFinding] = []
    for name in sorted(mapping):
        value = mapping[name]
        if isinstance(value, Mapping):
            findings.extend(secret_leak_findings(entry_name, value, f"{section}.{name}"))
            continue
        if is_secret_bearing_name(name) and classify_value(value).kind == "literal":
            findings.append(
                LeakFinding(
                    location=f"{entry_name}.{section}.{name}",
                    detail="literal value under a secret-bearing name; use a ${VAR} placeholder",
                )
            )
    return tuple(findings)


def assert_no_resolved_secrets(text: str, environ: Mapping[str, str]) -> None:
    """Fail if the current process environment leaked into rendered output.

    Generation and serialization call this with ``os.environ`` as a belt-and-
    braces check: even a bug that resolved a placeholder could not publish the
    resolved value unnoticed. The check covers every variable this process can
    see, not only referenced ones, because the leak path is unknown by
    construction — that is why it is asserted rather than enumerated.
    """
    lowered = text.lower()
    for name, value in environ.items():
        if (
            len(value) >= 8
            and is_secret_bearing_name(name)
            and value.lower() in lowered
        ):
            raise AssertionError(
                f"rendered output contains the value of secret-bearing environment variable {name}"
            )
