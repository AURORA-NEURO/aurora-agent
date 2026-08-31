"""Error taxonomy for the Hermes Agent integration.

Transport, configuration, policy and protocol failures stay distinct so a caller
can retry a broken process without retrying a refused safety decision, mirroring
the error discipline of python/prism_sdk/errors.py.
"""

from __future__ import annotations


class HermesIntegrationError(Exception):
    """Base class for every failure raised by this package."""


class ConfigError(HermesIntegrationError, ValueError):
    """A server entry violates the documented ``mcp_servers`` contract."""


class YamlSupportUnavailable(HermesIntegrationError):
    """YAML input was requested but PyYAML is not installed in this interpreter.

    The dependency-free core never imports YAML; only :func:`aurora_hermes.serialize.load_yaml`
    reaches here, so the failure names the missing capability instead of degrading silently.
    """


class UnsafeCommandError(HermesIntegrationError, ValueError):
    """A launcher contract was requested that could permit argument injection or
    non-argv execution. The integration never builds shell strings; rejection is
    the safe terminal state."""


class UnsupportedFeatureError(HermesIntegrationError, ValueError):
    """A configuration requests an auth or transport feature this adapter does not implement."""


class ProbeError(HermesIntegrationError):
    """The optional live readiness probe could not be performed at all.

    A probe that did not run is reported as ``unmeasured`` by the diagnostics
    layer; this error is reserved for callers who asked for a probe that cannot
    even be attempted (for example a remote entry probed with the stdio prober).
    """
