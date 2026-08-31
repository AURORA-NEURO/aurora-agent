# TypeScript edge parity slice

This package is a strict, runtime-neutral TypeScript 5 slice for edge and
Node-style embedding. It provides deterministic identifiers, hashing, canonical
JSON telemetry, descriptor-only adapter inventory, bounded queues, streaming
manifest metadata, and MCP/REST/webhook type shapes.

It does not provide live network clients, authentication/OAuth, provider SDKs,
process spawning, persistence, distributed leases, or a scheduler. Generated
platform descriptors are intentionally not support claims.

Run `npm install`, `npm run build`, and `node --test test/` when npm dependencies
are available. The repository's current host may use Node's native TypeScript
runtime or an external TypeScript 5 compiler; the compiler gate must be reported
separately from runtime tests.
