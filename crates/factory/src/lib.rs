//! Job, worker, lease and recovery lifecycle.
//!
//! Implements blueprint 40.30 and the scheduling half of section 35. Long-running work — builds,
//! ingests, sandboxed runs, evaluations, mutations, indexing — executes under observable leases
//! with idempotency-aware recovery.
//!
//! ## The invariant that shapes everything
//!
//! 40.30's second non-negotiable is easy to read past and expensive to get wrong:
//!
//! > Lease expiry does not imply safe retry for non-idempotent effects.
//!
//! A missed heartbeat is *ambiguous*. The worker may have crashed before starting the work, or it
//! may have completed the external effect and died before committing. Nothing in the queue can tell
//! those apart. So recovery branches on [`Idempotency`]: idempotent work requeues, compensable work
//! waits for its compensation to run, and non-idempotent work is **quarantined for a person** —
//! whose release must be attributed.
//!
//! A reported failure is different, and treated differently: the worker was alive and says the work
//! did not land, so even non-idempotent jobs requeue. Conflating the two cases is what makes queues
//! double-charge.
//!
//! ## Not implemented
//!
//! In-memory, single-process, and time is passed in rather than read from a clock so the lifecycle
//! is deterministically testable. A durable multi-node store needs the append-only event ledger of
//! 40.09 and a transactional backend; this crate is the lifecycle logic those would wrap, not a
//! substitute for them. There is no backpressure model, no fair-share scheduling across tenants,
//! and no distributed lease fencing — 35's million-scale concerns beyond enqueue and recovery are
//! absent rather than stubbed.

pub mod error;
pub mod job;
pub mod lease;
pub mod store;

pub use error::FactoryError;
pub use job::{Idempotency, Job, JobState, ResourceClass};
pub use lease::{Lease, WorkerCapability};
pub use store::{JobStore, Recovery};
