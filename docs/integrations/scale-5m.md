# 5M+ LOC scale layer

`integrations/scale-5m` is a bounded local indexing and coordination layer for repositories whose
combined source volume can exceed five million lines. The target is streaming processing and
bounded state; the implementation must not manufacture millions of files or retain all source
bytes in memory.

## Processing model

- `iter_file_records` walks a tree in normalized lexical path order, skips symlinks, reads each
  file through a 1 MiB buffer, and emits path, byte count, line count, and SHA-256 digest.
- `chunks_from_records` turns records into deterministic bounded chunks. Chunk digests and the
  summary root digest are stable across runs with the same bytes and paths.
- `synthetic_records` and `synthetic_manifest_benchmark` model a 5M-line repository without
  creating files. Their digest is explicitly a logical synthetic digest, not a claim about file
  contents or machine throughput.
- Checkpoints are canonical JSON with a next chunk and prior digest. A durable deployment still
  needs an external journal and atomic persistence policy.
- Incremental comparison is path/digest based and reports added, removed, changed, unchanged, and
  a digest-bound change summary.

## 1000+ platform descriptors

The adapter registry stores compact `(platform, protocol, state, capabilities, notes)` descriptors.
The default registry includes 1024 generated descriptor-only platform entries plus named MCP,
REST, GraphQL, webhook, CLI, archive, A2A, and ACP protocol descriptors. Generated entries are
not live connectors: they prove registry scale and deterministic lookup, not platform support.

`supported` and `partial` are reserved for the small local Aurora MCP surfaces. `descriptor-only`
means a future adapter contract can be registered without pretending connectivity. A2A and ACP are
refusal states here, matching the protocol honesty boundary in the agent-fabric integration.

## Fleet bounds

Shard assignment is deterministic; leases carry monotone epochs and expire; queues reject rather
than grow; telemetry exposes submitted, dispatched, completed, in-flight, backpressure, expiry,
and peak-in-flight counters. These are logical workers. The module does not spawn thousands of OS
processes, provide distributed durability, provide authentication, or connect to any platform.

## Verification

From `integrations/scale-5m`:

```text
python -m unittest discover -s tests -t . -v
python -m compileall -q aurora_scale tests
```

The suite includes a 5,000,000-line-equivalent synthetic manifest, incremental changes,
checkpoint round trips, a 1,000+ descriptor registry, lease/queue/telemetry bounds, and a
no-files-created benchmark assertion.
