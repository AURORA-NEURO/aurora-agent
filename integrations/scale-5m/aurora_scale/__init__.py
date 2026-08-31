"""Bounded, deterministic scale/index primitives for Aurora."""

from .benchmark import synthetic_manifest_benchmark
from .checkpoint import ManifestCheckpoint, decode_checkpoint, encode_checkpoint, read_checkpoint, write_checkpoint
from .errors import CheckpointError, LeaseError, ManifestError, RegistryError, ScaleError, UnsupportedAdapterError
from .fleet import BoundedQueue, Lease, LeaseTable, Telemetry, assign_shard
from .incremental import ChangeSet, incremental_changes
from .manifest import FileRecord, ManifestChunk, ManifestSummary, chunks_from_records, iter_file_records, normalize_relative, stream_manifest, summarize, synthetic_records
from .registry import AdapterDescriptor, AdapterRegistry, AdapterState, default_registry

__all__ = [
    "AdapterDescriptor", "AdapterRegistry", "AdapterState", "BoundedQueue", "ChangeSet", "CheckpointError",
    "FileRecord", "Lease", "LeaseError", "LeaseTable", "ManifestCheckpoint", "ManifestChunk", "ManifestError",
    "ManifestSummary", "RegistryError", "ScaleError", "Telemetry", "UnsupportedAdapterError", "assign_shard",
    "chunks_from_records", "decode_checkpoint", "default_registry", "encode_checkpoint", "incremental_changes",
    "iter_file_records", "normalize_relative", "read_checkpoint", "stream_manifest", "summarize",
    "synthetic_manifest_benchmark", "synthetic_records", "write_checkpoint",
]
