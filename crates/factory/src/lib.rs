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
//! The live controller is single-process and time is passed in rather than read from a clock so
//! the lifecycle is deterministically testable. It now provides a bounded, content-addressed JSON
//! checkpoint with cross-index validation and atomic replacement, plus an explicit startup sweep
//! for expired leases. A durable multi-node deployment still needs the append-only event ledger of
//! 40.09, a transactional backend, cross-node lease fencing, and an authenticated worker
//! identity. The local controller now has explicit queue admission and per-resource-class
//! fair-share limits, but cross-node scheduling and tenant fairness remain absent rather than
//! stubbed.

pub mod error;
pub mod admission;
pub mod job;
pub mod lease;
pub mod snapshot;
pub mod store;

pub use error::FactoryError;
pub use admission::QueueAdmissionPolicy;
pub use job::{Idempotency, Job, JobState, ResourceClass};
pub use lease::{Lease, WorkerCapability};
pub use snapshot::{
    CompensationRecord, IdempotencyIndexEntry, JobStoreSnapshot, OutputRecord,
    JOB_STORE_SNAPSHOT_SCHEMA_VERSION, MAX_JOB_STORE_SNAPSHOT_BYTES,
    MAX_JOB_STORE_SNAPSHOT_ID_BYTES, MAX_JOB_STORE_SNAPSHOT_JOBS,
    MAX_JOB_STORE_SNAPSHOT_VALUE_BYTES, MAX_JOB_STORE_SNAPSHOT_WORKER_ID_BYTES,
};
pub use store::{JobStore, Recovery};
