# TypeScript edge/runtime parity slice

`ports/typescript-edge` is the TypeScript 5 boundary for browser, worker, and
Node-style embedding of Aurora's scale primitives. It is intentionally separate
from the existing root TypeScript package so its invariants and test gate do not
depend on unrelated changes.

Included surfaces are typed nonzero IDs with safe-integer bounds, BigInt-exact
rendezvous placement, canonical JSON, WebCrypto-first SHA-256 with a narrowly
bounded Node fallback, descriptor-only registry entries beyond 1,000 platforms,
explicit backpressure, stable telemetry, streaming manifest chunks, and
type-only MCP/REST/webhook shapes.

The five-million-line target is logical metadata scale: the manifest generator
retains one bounded chunk plus digest state and creates no files. Platform count
is registry coverage, not live connectivity. This slice does not claim provider
SDKs, auth/OAuth, network transport, process execution, persistence, distributed
leases, or scheduler behavior.

Verification performed in this workspace:

- TypeScript 5.6 compiler `--noEmit` and build passed using the cached compiler
  at verification time.
- The runtime test suite is `node --test test/` after build. The package-local
  npm install gate hit host `ENOSPC`; the implementation has no checked-in
  `node_modules` and this failure is not reported as a passing npm install.
- Rust and Python reference gates remain the authorities for the parity vectors.
