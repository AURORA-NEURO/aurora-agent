// Package controlplane is the Go parity slice for Aurora's bounded operational
// control-plane concepts. It ports the small, deterministic primitives shared by
// integrations/agent-fabric (Rust) and integrations/scale-5m (Python): typed
// nonzero identifiers, deterministic shard placement, epoch leases, bounded
// queues with explicit backpressure, a settlement-closed idempotency window,
// adapter descriptor lookup with refusal states, JSON telemetry snapshots, and
// synthetic manifest metadata streaming.
//
// The Rust integration remains the semantic authority; vectors/parity.json is
// generated from executed upstream code and is what makes this port checkable.
//
// # What is deliberately not implemented
//
// A missing capability stated here is a limitation; one implied to exist would
// be a lie.
//
//   - No scheduler core. Routing under capability constraints, retry policies,
//     quotas, cancellation generations, receipt ledgers, and execution drivers
//     stay in the Rust fabric. This package ports the concepts they are built
//     from, not the state machine that composes them.
//   - No distribution and no durability. Shards are logical partitions in one
//     process, leases are authoritative only inside one LeaseTable value, and
//     nothing survives process exit. A second process sharing nothing is not a
//     cluster.
//   - No concurrency inside the primitives. Every type here expects a single
//     goroutine, mirroring the single-threaded Rust core whose determinism its
//     tests assert. Callers that need sharing must add synchronization
//     themselves; adding a mutex would hide that choice.
//   - No live platform connectivity, no MCP/HTTP/A2A/ACP adapters, no OS
//     processes. Adapter descriptors describe shapes; RequireLive still refuses
//     everything except supported/partial local surfaces. Logical agent count
//     is never a thread or process count.
//   - No manifest file walking. Records come from SyntheticRecords or caller
//     streams; reading a real tree stays Python-owned until a Go port needs it.
package controlplane
