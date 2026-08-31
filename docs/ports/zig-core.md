# Zig core port gate

`ports/zig-core` is the first language port in the Aurora optimization matrix. Its scope is a
small hot path that can eventually serve edge/WASM builds: typed IDs, capability normalization,
SHA-256 binding, canonical JSON field ordering, hex encoding, and bounded queues.

It does not claim to port the scheduler, leases, MCP/HTTP adapters, provider SDKs, or distributed
coordination. Those remain Rust-owned until a separate compatibility design and failure model are
approved.

The parity gate is:

1. Pin one Zig release (the source targets 0.13+ APIs).
2. Run `zig build test` and `zig build run` from `ports/zig-core`.
3. Compare the SHA-256, capability, and canonical-object outputs in `vectors/parity.json` with
   `integrations/agent-fabric` tests.
4. Benchmark the same payload sizes with release optimization enabled and record allocations and
   throughput. Do not infer production scheduler performance from this microbenchmark.

The current environment has no Zig executable, so the port is intentionally marked uncompiled.
