import test from "node:test";
import assert from "node:assert/strict";
import {
  AdapterRegistry,
  BoundedQueue,
  Telemetry,
  activeDigestBackend,
  canonicalJsonString,
  chunksFromRecords,
  defaultRegistry,
  formatAgentId,
  formatTaskId,
  homeShard,
  mix64,
  newAgentId,
  newTaskId,
  openIncrementalSha256,
  peakRecordState,
  preferenceOrder,
  sha256Hex,
  summarize,
  syntheticRecords,
} from "../dist/index.js";

test("zero raw ids are rejected and branded displays name their kind", () => {
  assert.throws(() => newAgentId(0), { name: "ZeroIdError" });
  assert.throws(() => newTaskId(Number.MAX_SAFE_INTEGER + 1), { name: "IdRangeError" });
  assert.equal(formatAgentId(newAgentId(3)), "agent-3");
  assert.equal(formatTaskId(newTaskId(3)), "task-3");
});

test("SplitMix64 and rendezvous placement match the Rust parity points", () => {
  assert.equal(mix64(0n).toString(16), "e220a8397b1dcdaf");
  assert.equal(homeShard(16n, 12345n), 10n);
  assert.deepEqual(preferenceOrder(7n, 42n), [5n, 3n, 6n, 0n, 1n, 2n, 4n]);
});

test("canonical JSON is sorted, compact, and rejects unsupported values", () => {
  assert.equal(canonicalJsonString({ b: "x", a: 1 }), '{"a":1,"b":"x"}');
  assert.equal(canonicalJsonString({ text: "é\n" }), '{"text":"é\\n"}');
  assert.throws(() => canonicalJsonString({ value: undefined }), { name: "CanonicalJSONError" });
});

test("WebCrypto or the bounded Node fallback produces the known SHA-256", async () => {
  assert.ok(["webcrypto", "node-crypto"].includes(await activeDigestBackend()));
  assert.equal(
    await sha256Hex("abc"),
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
  );
  const incremental = await openIncrementalSha256();
  incremental.update("a");
  incremental.update("bc");
  assert.equal(await sha256Hex("abc"), incremental.digestHex());
});

test("descriptor registry scales past one thousand without claiming live support", () => {
  const registry = defaultRegistry(1024);
  assert.equal(registry.size, 1033);
  assert.equal(registry.countByState("descriptor-only"), 1029);
  assert.throws(() => registry.requireLive("platform-0000", "rest"), { name: "UnsupportedAdapterError" });
  assert.throws(() => registry.get("missing", "rest"), { name: "UnknownAdapterError" });
  assert.ok(registry.snapshot()[0].platform <= registry.snapshot().at(-1).platform);
  assert.ok(registry instanceof AdapterRegistry);
});

test("bounded queue reports backpressure and preserves FIFO", () => {
  const queue = new BoundedQueue(2);
  assert.deepEqual(queue.push("a"), { ok: true });
  assert.deepEqual(queue.push("b"), { ok: true });
  assert.deepEqual(queue.push("c"), { ok: false, reason: "backpressure", capacity: 2 });
  assert.deepEqual(queue.pop(), { ok: true, item: "a" });
  assert.deepEqual(queue.pop(), { ok: true, item: "b" });
  assert.deepEqual(queue.pop(), { ok: false, reason: "empty" });
  assert.equal(queue.highWater, 2);
});

test("telemetry stays nonnegative and serializes in stable key order", () => {
  const telemetry = new Telemetry();
  telemetry.dispatch();
  telemetry.complete();
  telemetry.complete();
  telemetry.rejectBackpressure();
  telemetry.leaseExpired();
  assert.equal(
    telemetry.snapshotJSON(),
    '{"completed":2,"dispatched":1,"in_flight":0,"lease_expiries":1,"peak_in_flight":1,"rejected_backpressure":1,"submitted":1}',
  );
});

test("streaming manifest reproduces the CPython/Rust scale vector and bounds state", async () => {
  const records = syntheticRecords({ fileCount: 2048, linesPerFile: 100, bytesPerLine: 80 });
  const chunks = chunksFromRecords(records, 256);
  const summary = await summarize("synthetic", chunks);
  assert.deepEqual(summary, {
    root: "synthetic",
    total_files: 2048,
    total_bytes: 16384000,
    total_lines: 204800,
    chunk_count: 8,
    root_digest: "881c41fae8f9a88a70422de893de63067158cda909f98cf793a8af20d8d353b5",
  });
  assert.equal(peakRecordState(50000, 1024), 1024);
});
