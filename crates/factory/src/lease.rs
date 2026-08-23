//! Leases and heartbeats.
//!
//! Blueprint 40.30, first invariant: **one active lease per job attempt**. A second worker taking
//! the same attempt is how a queue silently double-executes, so the store refuses it rather than
//! detecting it afterwards.
//!
//! Time is passed in explicitly rather than read from a clock. A lease lifecycle that consults the
//! system clock cannot be tested deterministically, and the runtime crate's virtualization seam
//! exists for exactly this reason.

use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCapability {
    pub worker_id: String,
    /// Resource classes this worker can execute. A worker that claims a class it cannot run is a
    /// configuration error the store cannot detect, so capability is declared, not inferred.
    pub classes: Vec<crate::job::ResourceClass>,
    /// How long this worker's leases run before expiring, in nanoseconds.
    pub lease_duration_nanos: i128,
}

impl WorkerCapability {
    pub fn new(worker_id: impl Into<String>, classes: Vec<crate::job::ResourceClass>) -> Self {
        WorkerCapability {
            worker_id: worker_id.into(),
            classes,
            lease_duration_nanos: 30_000_000_000,
        }
    }

    pub fn with_lease_duration_nanos(mut self, nanos: i128) -> Self {
        self.lease_duration_nanos = nanos;
        self
    }

    pub fn can_run(&self, class: crate::job::ResourceClass) -> bool {
        self.classes.contains(&class)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub job_id: String,
    pub worker_id: String,
    /// Which attempt this lease covers. This is also the fencing token: every heartbeat, stage,
    /// failure, and commit must present the same attempt, so a stale worker cannot mutate a later
    /// lease even when its worker identity is reused.
    pub attempt: u32,
    pub granted_at: Timestamp,
    pub expires_at: Timestamp,
    /// Last heartbeat. A worker that stops reporting loses the lease at expiry.
    pub last_heartbeat: Timestamp,
}

impl Lease {
    pub fn is_expired(&self, now: Timestamp) -> bool {
        now >= self.expires_at
    }

    pub fn remaining_nanos(&self, now: Timestamp) -> i128 {
        (self.expires_at.as_nanos_utc() - now.as_nanos_utc()).max(0)
    }

    /// Extends the lease from `now`. Refuses once expired.
    ///
    /// A worker that missed its window must not be able to reclaim by heartbeating late: the store
    /// may already have decided the attempt was ambiguous and acted on it.
    pub fn heartbeat(&mut self, now: Timestamp, duration_nanos: i128) -> bool {
        if self.is_expired(now) {
            return false;
        }
        self.last_heartbeat = now;
        self.expires_at = Timestamp::from_nanos_utc(now.as_nanos_utc() + duration_nanos);
        true
    }
}
