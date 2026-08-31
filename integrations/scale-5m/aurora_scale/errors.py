"""Typed failures for the scale layer."""


class ScaleError(Exception):
    """Base class for scale-layer failures."""


class ManifestError(ScaleError, ValueError):
    """A path, file, digest, or manifest chunk violates its contract."""


class CheckpointError(ScaleError, ValueError):
    """A resume checkpoint is malformed or does not match the manifest stream."""


class RegistryError(ScaleError, ValueError):
    """An adapter descriptor is invalid or collides with an existing key."""


class UnsupportedAdapterError(ScaleError, ValueError):
    """A descriptor is intentionally refusal-only and cannot be used as live support."""


class LeaseError(ScaleError, ValueError):
    """A lease is stale, held, or unknown."""
