# Aurora Zig hot-path port

This directory is a parity port of the small, allocation-sensitive primitives used by
`integrations/agent-fabric`: typed nonzero IDs, capability normalization, SHA-256 payload digest,
hex/canonical JSON helpers, and bounded FIFO backpressure. It is not a replacement for the Rust
scheduler, MCP adapter, lease table, or distributed control plane.

The canonical test vectors live in `vectors/parity.json`. The Rust implementation remains the
semantic authority until this port is built with a pinned Zig toolchain and the vectors are checked
in both directions.

Expected command when Zig 0.13+ is installed:

```text
zig build test
zig build run
```

On the current host, `zig` is not installed, so no compilation result is claimed. The source is
written against the stable Zig standard-library APIs used by 0.13+; a later gate must pin the exact
compiler version, run the unit tests, and compare `vectors/parity.json` against the Rust fixtures.
