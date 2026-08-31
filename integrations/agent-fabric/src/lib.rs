//! `agent-fabric` — a bounded multi-agent coordination fabric for Aurora.
//!
//! The fabric coordinates thousands of *logical* agents from one process without spawning
//! thousands of OS processes. Logical agents are registry entries placed on logical shards;
//! work is dispatched to them through a small pool of real executors (one inline driver, or a
//! bounded thread pool), so concurrency is a configured bound, not a function of agent count.
//!
//! The design is a pure, deterministic scheduling core plus swappable execution drivers:
//!
//! - [`scheduler::Fabric`] owns routing, shard queues, leases, quotas, retries, idempotency and
//!   receipts. It never blocks and never touches a thread; time is an explicit tick.
//! - [`exec::Driver`] is the only execution seam. [`exec::InlineDriver`] runs handlers
//!   synchronously (deterministic, used by tests and benches); [`exec::ThreadDriver`] runs a
//!   bounded worker pool with a bounded hand-off channel, so backpressure reaches the scheduler
//!   as a first-class signal rather than an unbounded queue.
//! - [`sim::Simulation`] drives the same core under a virtual clock with injected faults, which
//!   is what the scale/failure/fairness test suites assert against.
//!
//! Provenance is enforced twice on purpose: every [`envelope::TaskEnvelope`] carries the SHA-256
//! of its payload ([`digest`]) computed at composition time, and both the worker side
//! ([`exec::run_bound_job`]) and the settlement side re-verify bindings. A mismatch becomes a
//! `Corrupted` terminal outcome — never a silent pass — because a right answer from a payload the
//! fabric cannot attest to is not a pass.
//!
//! Relationship to the workspace: this crate is deliberately **not** a member of the aurora
//! workspace and shares no code with it. It is unrelated to `bioprism-fabric`, which implements
//! blueprint §23 (composition algebra, contracts, interweave); that crate composes agents'
//! *minds*, this one bounds their *dispatch*. The name overlap is recorded in the integration
//! doc (`docs/integrations/agent-fabric.md`) as a known confusion risk.
//!
//! # What is deliberately not implemented
//!
//! A missing capability stated here is a limitation; one implied to exist would be a lie.
//!
//! - **No distribution.** Shards are logical partitions inside one process, not hosts. Leases,
//!   quotas and idempotency windows are authoritative only within this process; a second process
//!   pointing at the same "fabric" would share nothing.
//! - **No durability.** Queues, leases and receipts are in-memory. A restart loses all state;
//!   there is no journal, no replay, no persistence layer.
//! - **MCP adapter covers a subset**: `initialize`, `ping`, `tools/list`, `tools/call`
//!   (`fabric.submit`, `fabric.cancel`) and cancellation notifications over line-delimited stdio
//!   framing. No resources, prompts, sampling, logging, or progress notifications.
//! - **HTTP adapter speaks HTTP/1.1 with Content-Length responses only.** No chunked transfer,
//!   no TLS, no HTTP/2, no keep-alive pooling. A server that replies with chunked encoding fails
//!   loudly with a protocol error rather than being misparsed.
//! - **A2A/ACP: wire-shape compatibility only, no support claim.** [`bridge`] maps envelopes to
//!   and from message shapes whose field names follow A2A-style conventions, and the ACP entry
//!   point returns a typed refusal. There is no executable A2A or ACP adapter in this crate; do
//!   not describe this crate as speaking either protocol.
//! - **No async runtime.** The only preemptive executor is the bounded thread pool.
//! - **Quotas are admission control, not security.** They throttle dispatch rate per agent;
//!   they are not an isolation or authorization boundary.

pub mod bridge;
pub mod cancel;
pub mod capability;
pub mod digest;
pub mod envelope;
pub mod exec;
pub mod http;
pub mod ids;
pub mod json;
pub mod lease;
pub mod mcp_stdio;
pub mod queue;
pub mod quota;
pub mod retry;
pub mod router;
pub mod scheduler;
pub mod shard;
pub mod sim;
pub mod transport;

pub use capability::{Capability, CapabilityError, CapabilitySet};
pub use digest::Digest;
pub use envelope::{Completion, DispatchJob, Outcome, Receipt, TaskEnvelope};
pub use exec::{Driver, Handler, InlineDriver, ThreadDriver};
pub use ids::{AgentId, IdempotencyKey, LeaseEpoch, ShardId, TaskId};
pub use retry::RetryPolicy;
pub use scheduler::{Fabric, FabricConfig, Metrics, Submission};

/// Crate-wide semantic-version guard used by tests to pin behaviour descriptions to a build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
