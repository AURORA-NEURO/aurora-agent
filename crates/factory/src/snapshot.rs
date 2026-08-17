//! Durable, content-addressed checkpoints for [`crate::JobStore`].
//!
//! A checkpoint is a recovery boundary, not an event ledger. It contains enough state to resume
//! the lifecycle deterministically, but it cannot prove that an external effect did or did not
//! happen before a worker disappeared. The lease/idempotency policy remains authoritative after a
//! restore. The digest detects torn or tampered JSON; it is not a signature or an authorization
//! token.

use crate::error::FactoryError;
use crate::job::Job;
use crate::lease::Lease;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JOB_STORE_SNAPSHOT_SCHEMA_VERSION: u64 = 1;
pub const MAX_JOB_STORE_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_JOB_STORE_SNAPSHOT_JOBS: usize = 100_000;
pub const MAX_JOB_STORE_SNAPSHOT_VALUE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_JOB_STORE_SNAPSHOT_ID_BYTES: usize = 512;
pub const MAX_JOB_STORE_SNAPSHOT_WORKER_ID_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputRecord {
    pub job_id: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyIndexEntry {
    pub key: String,
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationRecord {
    pub job_id: String,
    pub completed: bool,
}

/// A self-contained checkpoint of the factory lifecycle state.
///
/// Vectors are used at the wire boundary rather than exposing implementation maps. They are
/// emitted in deterministic key order and checked for duplicates during restore, which makes the
/// digest stable across Rust versions and prevents ambiguous duplicate-key documents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobStoreSnapshot {
    pub schema_version: u64,
    pub jobs: Vec<Job>,
    pub leases: Vec<Lease>,
    pub staged: Vec<OutputRecord>,
    pub committed: Vec<OutputRecord>,
    pub idempotency_index: Vec<IdempotencyIndexEntry>,
    pub compensation: Vec<CompensationRecord>,
    pub state_digest: String,
}

#[derive(Serialize)]
struct SnapshotBody<'a> {
    schema_version: u64,
    jobs: &'a [Job],
    leases: &'a [Lease],
    staged: &'a [OutputRecord],
    committed: &'a [OutputRecord],
    idempotency_index: &'a [IdempotencyIndexEntry],
    compensation: &'a [CompensationRecord],
}

impl JobStoreSnapshot {
    pub(crate) fn body_value(&self) -> Result<Value, FactoryError> {
        serde_json::to_value(SnapshotBody {
            schema_version: self.schema_version,
            jobs: &self.jobs,
            leases: &self.leases,
            staged: &self.staged,
            committed: &self.committed,
            idempotency_index: &self.idempotency_index,
            compensation: &self.compensation,
        })
        .map_err(|error| FactoryError::SnapshotSerialization {
            reason: error.to_string(),
        })
    }

    pub(crate) fn computed_digest(&self) -> Result<String, FactoryError> {
        let body = self.body_value()?;
        Ok(ContentHash::of_value(&body)
            .map_err(|error| FactoryError::SnapshotSerialization {
                reason: error.to_string(),
            })?
            .to_string())
    }

    pub fn verify_digest(&self) -> Result<(), FactoryError> {
        let actual = self.computed_digest()?;
        if self.state_digest != actual {
            return Err(FactoryError::SnapshotDigestMismatch {
                expected: self.state_digest.clone(),
                actual,
            });
        }
        Ok(())
    }

    /// Return the digest that would authenticate the current body fields.
    ///
    /// This is useful to migration tooling that intentionally constructs a new snapshot version;
    /// loading still requires the caller to provide the resulting digest and passes it through
    /// the same verification path.
    pub fn digest(&self) -> Result<String, FactoryError> {
        self.computed_digest()
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, FactoryError> {
        self.verify_digest()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            FactoryError::SnapshotSerialization {
                reason: error.to_string(),
            }
        })?;
        if bytes.len() > MAX_JOB_STORE_SNAPSHOT_BYTES {
            return Err(FactoryError::SnapshotTooLarge {
                bytes: bytes.len(),
                max_bytes: MAX_JOB_STORE_SNAPSHOT_BYTES,
            });
        }
        Ok(bytes)
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, FactoryError> {
        if bytes.len() > MAX_JOB_STORE_SNAPSHOT_BYTES {
            return Err(FactoryError::SnapshotTooLarge {
                bytes: bytes.len(),
                max_bytes: MAX_JOB_STORE_SNAPSHOT_BYTES,
            });
        }
        let snapshot: Self =
            serde_json::from_slice(bytes).map_err(|error| FactoryError::InvalidSnapshot {
                reason: format!("invalid JSON: {error}"),
            })?;
        snapshot.verify_digest()?;
        Ok(snapshot)
    }
}
