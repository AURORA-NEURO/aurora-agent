import tempfile
import unittest
from pathlib import Path

from aurora_scale import (
    AdapterState,
    BoundedQueue,
    FileRecord,
    LeaseError,
    LeaseTable,
    ManifestCheckpoint,
    Telemetry,
    UnsupportedAdapterError,
    assign_shard,
    chunks_from_records,
    decode_checkpoint,
    default_registry,
    encode_checkpoint,
    incremental_changes,
    iter_file_records,
    normalize_relative,
    stream_manifest,
    summarize,
    synthetic_manifest_benchmark,
    synthetic_records,
)


class ManifestScaleTests(unittest.TestCase):
    def test_five_million_line_equivalent_is_logical_and_bounded(self):
        chunks = chunks_from_records(synthetic_records(file_count=1, lines_per_file=5_000_000), chunk_records=1)
        summary = summarize("synthetic", chunks)
        self.assertEqual(summary.total_lines, 5_000_000)
        self.assertEqual(summary.total_files, 1)
        self.assertEqual(summary.chunk_count, 1)

    def test_streaming_files_normalize_and_hash_without_loading_tree(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            (base / "b.txt").write_text("two\n", encoding="utf-8")
            (base / "a.txt").write_text("one\nlast", encoding="utf-8")
            chunks = tuple(stream_manifest(base, chunk_records=1))
            self.assertEqual([chunk.records[0].path for chunk in chunks], ["a.txt", "b.txt"])
            self.assertEqual(sum(chunk.lines for chunk in chunks), 3)
        self.assertEqual(normalize_relative("a\\b.txt"), "a/b.txt")
        with self.assertRaises(ValueError):
            normalize_relative("../escape")

    def test_incremental_changes_are_sorted_and_digest_bound(self):
        old = [FileRecord("a", 1, 1, "a"), FileRecord("b", 1, 1, "b")]
        new = [FileRecord("a", 1, 1, "changed"), FileRecord("c", 1, 1, "c")]
        changes = incremental_changes(old, new)
        self.assertEqual(changes.added, ("c",))
        self.assertEqual(changes.removed, ("b",))
        self.assertEqual(changes.changed, ("a",))
        self.assertEqual(len(changes.digest), 64)

    def test_checkpoint_round_trip_is_canonical(self):
        checkpoint = ManifestCheckpoint("repo", 4, "ab" * 32, "cd" * 32)
        text = encode_checkpoint(checkpoint)
        self.assertEqual(text, encode_checkpoint(decode_checkpoint(text)))


class RegistryAndFleetTests(unittest.TestCase):
    def test_registry_scales_with_compact_descriptor_only_platforms(self):
        registry = default_registry(generated_platforms=1024)
        self.assertGreaterEqual(len(registry), 1000)
        self.assertEqual(registry.get("aurora", "mcp-stdio").state, AdapterState.SUPPORTED)
        with self.assertRaises(UnsupportedAdapterError):
            registry.require_live("generic", "a2a")

    def test_shards_leases_queue_and_telemetry_are_bounded(self):
        self.assertEqual(assign_shard("same", 8), assign_shard("same", 8))
        leases = LeaseTable()
        lease = leases.grant("task", "worker", 0, 3)
        with self.assertRaises(LeaseError):
            leases.grant("task", "other", 0, 3)
        self.assertEqual(leases.expire(3)[0].epoch, lease.epoch)
        queue = BoundedQueue(1)
        self.assertTrue(queue.push("x"))
        self.assertFalse(queue.push("y"))
        telemetry = Telemetry()
        telemetry.dispatch()
        telemetry.complete()
        self.assertEqual(telemetry.snapshot()["in_flight"], 0)

    def test_synthetic_benchmark_reports_model_not_fake_files(self):
        result = synthetic_manifest_benchmark(file_count=2048, lines_per_file=100, chunk_records=256)
        self.assertEqual(result["files"], 2048)
        self.assertEqual(result["chunks"], 8)
        self.assertIn("no files created", result["model"])


if __name__ == "__main__":
    unittest.main()
