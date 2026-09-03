"""Error taxonomy for the Codex/Claude integration.

Configuration, safety and compatibility failures stay distinct so a caller can
fix a malformed spec without mistaking a refused provider-specific feature for
a typo, mirroring the error discipline of integrations/hermes.
"""

from __future__ import annotations


class CodexClaudeIntegrationError(Exception):
    """Base class for every failure raised by this package."""


class ConfigError(CodexClaudeIntegrationError, ValueError):
    """A server specification violates the documented adapter contract."""


class UnknownCapabilityError(ConfigError):
    """An allowlist names a tool absent from the Aurora capability registry.

    The registry is a static snapshot of what ``bioprism-mcp`` advertises; an
    unknown name is almost always a typo, and silently dropping it would shrink
    the allowlist without telling anyone.
    """


class UnsafeCommandError(CodexClaudeIntegrationError, ValueError):
    """A launcher contract that could permit shell injection was requested.

    Neither host profile runs the server through a shell; building a shell
    string here would be the one path to argument injection, so rejection is
    the terminal state.
    """


class UnsupportedFeatureError(CodexClaudeIntegrationError, ValueError):
    """A request names a provider-specific setting this adapter cannot prove compatible.

    Refusal is deliberate and named: the feature may exist in some host version,
    but its key or semantics could not be verified for the profile in question,
    and emitting it anyway would trade a loud error for silent misconfiguration.
    """
