//! Shared execution authority for queue state and lifecycle transitions.
//!
//! A [`JobStore`] checkpoint is a recovery image. It is not enough for two API processes to
//! coordinate: each process could load the same image, make a different decision, and overwrite
//! the other's work. This module adds the smallest durable authority boundary available without
//! an external database: an atomic envelope containing the queue snapshot and a hash-chained
//! execution transition journal, protected by an OS-atomic lock directory.
//!
//! The file authority is deliberately scoped. It coordinates cooperating processes that share a
//! filesystem; it does not pretend to provide consensus across hosts, protect a compromised
//! filesystem, or prove that an external provider effect completed. A deployment needing those
//! guarantees should implement the same transaction shape behind a real database backend.

use crate::error::FactoryError;
use crate::job::Job;
use crate::lease::Lease;
use crate::snapshot::JobStoreSnapshot;
use crate::store::JobStore;
use bioprism_ids::ContentHash;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Schema version for the queue-plus-transition authority envelope.
pub const EXECUTION_AUTHORITY_SCHEMA_VERSION: u64 = 1;
/// The envelope remains bounded even though it contains both a queue image and transition rows.
pub const MAX_EXECUTION_AUTHORITY_BYTES: usize = 64 * 1024 * 1024;
/// A journal must be compacted or rotated before it can become an unbounded source of memory.
pub const MAX_EXECUTION_AUTHORITY_EVENTS: usize = 100_000;
const MAX_AUTHORITY_FIELD_BYTES: usize = 512;
const MAX_AUTHORITY_DETAILS_BYTES: usize = 128 * 1024;
const AUTHORITY_LOCK_FILE: &str = "owner.json";
const AUTHORITY_LOCK_RETRIES: usize = 200;
const AUTHORITY_LOCK_RETRY_DELAY: Duration = Duration::from_millis(5);

static NEXT_AUTHORITY_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_AUTHORITY_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Material lifecycle operation recorded in the shared execution journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOperation {
    EnqueueAndLease,
    Heartbeat,
    Staged,
    Committed,
    Failed,
    LeaseRecovered,
    Cancelled,
    Compensated,
    QuarantineReleased,
    LockReleased,
}

/// The caller-supplied part of one authority transaction.
///
/// `idempotency_key` is separate from the queue's work idempotency key. It identifies the
/// lifecycle transition itself, allowing a retry to converge on one journal row instead of
/// silently creating a second history entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityMutation {
    pub operation: ExecutionOperation,
    pub idempotency_key: String,
    pub job_id: Option<String>,
    pub worker_id: Option<String>,
    pub attempt: Option<u32>,
    pub at: Timestamp,
    pub details: Value,
}

impl AuthorityMutation {
    pub fn new(
        operation: ExecutionOperation,
        idempotency_key: impl Into<String>,
        job_id: Option<String>,
        worker_id: Option<String>,
        attempt: Option<u32>,
        at: Timestamp,
        details: Value,
    ) -> Self {
        Self {
            operation,
            idempotency_key: idempotency_key.into(),
            job_id,
            worker_id,
            attempt,
            at,
            details,
        }
    }

    fn validate(&self) -> Result<(), FactoryError> {
        validate_text(&self.idempotency_key, "authority idempotency key")?;
        if let Some(job_id) = &self.job_id {
            validate_text(job_id, "authority job id")?;
        }
        if let Some(worker_id) = &self.worker_id {
            validate_text(worker_id, "authority worker id")?;
        }
        if self.attempt == Some(0) {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: "authority transition attempt cannot be zero".into(),
            });
        }
        let detail_bytes = serde_json::to_vec(&self.details).map_err(|error| {
            FactoryError::InvalidAuthoritySnapshot {
                reason: format!("authority transition details are not serializable: {error}"),
            }
        })?;
        if detail_bytes.len() > MAX_AUTHORITY_DETAILS_BYTES {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "authority transition details are {} bytes above the {}-byte bound",
                    detail_bytes.len(),
                    MAX_AUTHORITY_DETAILS_BYTES
                ),
            });
        }
        Ok(())
    }

    fn request_digest(&self) -> Result<String, FactoryError> {
        self.validate()?;
        digest_value(&json!({
            "operation": self.operation,
            "idempotency_key": self.idempotency_key,
            "job_id": self.job_id,
            "worker_id": self.worker_id,
            "attempt": self.attempt,
            "details": self.details,
        }))
    }
}

/// One immutable, hash-linked execution transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionTransition {
    pub sequence: u64,
    pub operation: ExecutionOperation,
    pub idempotency_key: String,
    pub request_digest: String,
    pub job_id: Option<String>,
    pub worker_id: Option<String>,
    pub attempt: Option<u32>,
    pub at: Timestamp,
    pub details: Value,
    pub details_digest: String,
    pub previous_digest: String,
    pub digest: String,
}

impl ExecutionTransition {
    fn from_mutation(
        sequence: u64,
        previous_digest: String,
        mutation: &AuthorityMutation,
    ) -> Result<Self, FactoryError> {
        let request_digest = mutation.request_digest()?;
        let details_digest = digest_value(&mutation.details)?;
        let mut transition = Self {
            sequence,
            operation: mutation.operation,
            idempotency_key: mutation.idempotency_key.clone(),
            request_digest,
            job_id: mutation.job_id.clone(),
            worker_id: mutation.worker_id.clone(),
            attempt: mutation.attempt,
            at: mutation.at,
            details: mutation.details.clone(),
            details_digest,
            previous_digest,
            digest: String::new(),
        };
        transition.digest = transition.recompute_digest()?;
        Ok(transition)
    }

    pub fn recompute_digest(&self) -> Result<String, FactoryError> {
        digest_value(&json!({
            "sequence": self.sequence,
            "operation": self.operation,
            "idempotency_key": self.idempotency_key,
            "request_digest": self.request_digest,
            "job_id": self.job_id,
            "worker_id": self.worker_id,
            "attempt": self.attempt,
            "at": self.at,
            "details_digest": self.details_digest,
            "previous_digest": self.previous_digest,
        }))
    }

    fn verify(&self, expected_sequence: u64, expected_previous: &str) -> Result<(), FactoryError> {
        if self.sequence != expected_sequence {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "transition sequence {} is out of order; expected {}",
                    self.sequence, expected_sequence
                ),
            });
        }
        if self.previous_digest != expected_previous {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "transition {} points to {}, expected {}",
                    self.sequence, self.previous_digest, expected_previous
                ),
            });
        }
        let details_digest = digest_value(&self.details)?;
        if details_digest != self.details_digest {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!("transition {} details digest does not match", self.sequence),
            });
        }
        validate_text(&self.idempotency_key, "authority idempotency key")?;
        if let Some(job_id) = &self.job_id {
            validate_text(job_id, "authority job id")?;
        }
        if let Some(worker_id) = &self.worker_id {
            validate_text(worker_id, "authority worker id")?;
        }
        if self.attempt == Some(0) {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!("transition {} has attempt zero", self.sequence),
            });
        }
        if self.recompute_digest()? != self.digest {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!("transition {} digest does not match", self.sequence),
            });
        }
        Ok(())
    }
}

/// A durable queue image and its execution history share one atomic replacement boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAuthoritySnapshot {
    pub schema_version: u64,
    pub revision: u64,
    pub authority_epoch: u64,
    pub queue: JobStoreSnapshot,
    pub events: Vec<ExecutionTransition>,
    pub state_digest: String,
}

impl ExecutionAuthoritySnapshot {
    fn body_value(&self) -> Result<Value, FactoryError> {
        serde_json::to_value(json!({
            "schema_version": self.schema_version,
            "revision": self.revision,
            "authority_epoch": self.authority_epoch,
            "queue": self.queue,
            "events": self.events,
        }))
        .map_err(|error| FactoryError::InvalidAuthoritySnapshot {
            reason: format!("authority snapshot is not serializable: {error}"),
        })
    }

    fn computed_digest(&self) -> Result<String, FactoryError> {
        digest_value(&self.body_value()?)
    }

    pub fn digest(&self) -> Result<String, FactoryError> {
        self.computed_digest()
    }

    pub fn verify(&self) -> Result<(), FactoryError> {
        if self.schema_version != EXECUTION_AUTHORITY_SCHEMA_VERSION {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "unsupported authority schema {}; expected {}",
                    self.schema_version, EXECUTION_AUTHORITY_SCHEMA_VERSION
                ),
            });
        }
        if self.authority_epoch == 0 {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: "authority epoch cannot be zero".into(),
            });
        }
        if self.events.len() > MAX_EXECUTION_AUTHORITY_EVENTS {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "contains {} transitions above the {}-transition bound",
                    self.events.len(),
                    MAX_EXECUTION_AUTHORITY_EVENTS
                ),
            });
        }
        if self.revision != self.events.len() as u64 {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "revision {} does not equal event count {}",
                    self.revision,
                    self.events.len()
                ),
            });
        }
        JobStore::from_snapshot(self.queue.clone())?;
        let mut previous = String::new();
        for (sequence, event) in self.events.iter().enumerate() {
            event.verify(sequence as u64, &previous)?;
            previous = event.digest.clone();
        }
        let actual = self.computed_digest()?;
        if self.state_digest != actual {
            return Err(FactoryError::AuthorityDigestMismatch {
                expected: self.state_digest.clone(),
                actual,
            });
        }
        Ok(())
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, FactoryError> {
        if bytes.len() > MAX_EXECUTION_AUTHORITY_BYTES {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "authority snapshot is {} bytes above the {}-byte bound",
                    bytes.len(),
                    MAX_EXECUTION_AUTHORITY_BYTES
                ),
            });
        }
        match serde_json::from_slice::<Self>(bytes) {
            Ok(snapshot) => {
                snapshot.verify()?;
                Ok(snapshot)
            }
            Err(authority_error) => {
                // The previous release wrote a bare JobStoreSnapshot. Treating that exact,
                // digest-verified shape as a migration input avoids turning a safe upgrade into
                // an empty queue while still rejecting arbitrary malformed JSON.
                let legacy = serde_json::from_slice::<JobStoreSnapshot>(bytes).map_err(|error| {
                    FactoryError::InvalidAuthoritySnapshot {
                        reason: format!(
                            "invalid authority JSON ({authority_error}); legacy queue migration also failed: {error}"
                        ),
                    }
                })?;
                legacy.verify_digest()?;
                Self::from_queue(legacy, 1, Vec::new())
            }
        }
    }

    pub fn load_from_path(path: &Path) -> Result<Self, FactoryError> {
        let bytes = fs::read(path).map_err(|error| FactoryError::AuthorityIo {
            operation: "read authority snapshot".into(),
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        Self::from_json_bytes(&bytes)
    }

    fn from_queue(
        queue: JobStoreSnapshot,
        authority_epoch: u64,
        events: Vec<ExecutionTransition>,
    ) -> Result<Self, FactoryError> {
        let mut snapshot = Self {
            schema_version: EXECUTION_AUTHORITY_SCHEMA_VERSION,
            revision: events.len() as u64,
            authority_epoch,
            queue,
            events,
            state_digest: String::new(),
        };
        snapshot.state_digest = snapshot.computed_digest()?;
        snapshot.verify()?;
        Ok(snapshot)
    }

    fn to_json_bytes(&self) -> Result<Vec<u8>, FactoryError> {
        self.verify()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            FactoryError::InvalidAuthoritySnapshot {
                reason: format!("authority snapshot serialization failed: {error}"),
            }
        })?;
        if bytes.len() > MAX_EXECUTION_AUTHORITY_BYTES {
            return Err(FactoryError::InvalidAuthoritySnapshot {
                reason: format!(
                    "authority snapshot is {} bytes above the {}-byte bound",
                    bytes.len(),
                    MAX_EXECUTION_AUTHORITY_BYTES
                ),
            });
        }
        Ok(bytes)
    }
}

/// Lock metadata exposed to operators. A lock is intentionally not silently discarded when a
/// process dies; releasing one is an auditable operator action because the old process may have
/// been paused rather than dead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityLockInfo {
    pub owner_id: String,
    pub acquired_unix_nanos: i128,
}

/// A bounded, inspectable authority projection for API and operator surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionAuthorityStatus {
    pub configured: bool,
    pub path: Option<String>,
    pub lock_path: Option<String>,
    pub lock_present: bool,
    pub lock: Option<AuthorityLockInfo>,
    pub schema_version: u64,
    pub revision: u64,
    pub authority_epoch: u64,
    pub event_count: usize,
    pub event_head_digest: String,
    pub authority_digest: String,
    pub queue_state_digest: String,
    pub integrity_verified: bool,
    pub transaction_model: String,
    pub execution_scope: String,
    pub operator_recovery: String,
}

/// Result of explicitly releasing an orphaned lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityLockRelease {
    pub operator: String,
    pub reason: String,
    pub previous_owner: AuthorityLockInfo,
    pub recorded_revision: u64,
}

#[derive(Debug, Clone)]
struct AuthorityState {
    queue: JobStore,
    events: Vec<ExecutionTransition>,
    authority_epoch: u64,
}

impl AuthorityState {
    fn empty() -> Self {
        Self {
            queue: JobStore::new(),
            events: Vec::new(),
            authority_epoch: 1,
        }
    }

    fn from_snapshot(snapshot: ExecutionAuthoritySnapshot) -> Result<Self, FactoryError> {
        snapshot.verify()?;
        Ok(Self {
            queue: JobStore::from_snapshot(snapshot.queue)?,
            events: snapshot.events,
            authority_epoch: snapshot.authority_epoch,
        })
    }

    fn snapshot(&self) -> Result<ExecutionAuthoritySnapshot, FactoryError> {
        let queue = self.queue.snapshot()?;
        let mut snapshot = ExecutionAuthoritySnapshot {
            schema_version: EXECUTION_AUTHORITY_SCHEMA_VERSION,
            revision: self.events.len() as u64,
            authority_epoch: self.authority_epoch,
            queue,
            events: self.events.clone(),
            state_digest: String::new(),
        };
        snapshot.state_digest = snapshot.computed_digest()?;
        snapshot.verify()?;
        Ok(snapshot)
    }
}

/// A filesystem-backed shared authority. Without a path it remains a useful single-process
/// authority with the same transition semantics, which keeps tests and embedded callers simple.
pub struct SharedExecutionAuthority {
    path: Option<PathBuf>,
    lock_path: Option<PathBuf>,
    owner_id: String,
    state: Mutex<AuthorityState>,
    process_lock: Mutex<()>,
}

impl std::fmt::Debug for SharedExecutionAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedExecutionAuthority")
            .field("path", &self.path)
            .field("lock_path", &self.lock_path)
            .field("owner_id", &self.owner_id)
            .finish_non_exhaustive()
    }
}

impl SharedExecutionAuthority {
    pub fn open(path: Option<PathBuf>) -> Result<Arc<Self>, FactoryError> {
        let lock_path = path.as_deref().map(authority_lock_path);
        let state = match path.as_deref() {
            Some(path) if path.exists() => {
                AuthorityState::from_snapshot(ExecutionAuthoritySnapshot::load_from_path(path)?)?
            }
            _ => AuthorityState::empty(),
        };
        let id = NEXT_AUTHORITY_ID.fetch_add(1, Ordering::Relaxed);
        let now = unix_nanos();
        Ok(Arc::new(Self {
            path,
            lock_path,
            owner_id: format!("pid-{}-authority-{id}-{now}", std::process::id()),
            state: Mutex::new(state),
            process_lock: Mutex::new(()),
        }))
    }

    /// Apply one queue transition and atomically publish its corresponding journal row.
    pub fn mutate<T, F>(
        &self,
        mutation: AuthorityMutation,
        transition: F,
    ) -> Result<T, FactoryError>
    where
        F: FnOnce(&mut JobStore) -> Result<T, FactoryError>,
    {
        mutation.validate()?;
        let _process_guard = self
            .process_lock
            .lock()
            .map_err(|_| authority_lock_error("acquire process authority lock"))?;
        let _file_guard = self.acquire_file_lock()?;
        let mut state = self.current_state()?;
        let request_digest = mutation.request_digest()?;
        let existing_request_digest = state
            .events
            .iter()
            .find(|event| event.idempotency_key == mutation.idempotency_key)
            .map(|event| event.request_digest.clone());
        if let Some(existing_request_digest) = &existing_request_digest {
            if existing_request_digest != &request_digest {
                return Err(FactoryError::AuthorityIdempotencyConflict {
                    key: mutation.idempotency_key,
                });
            }
        }

        let value = transition(&mut state.queue)?;
        if existing_request_digest.is_none() {
            let previous_digest = state
                .events
                .last()
                .map(|event| event.digest.clone())
                .unwrap_or_default();
            let event = ExecutionTransition::from_mutation(
                state.events.len() as u64,
                previous_digest,
                &mutation,
            )?;
            if state.events.len() >= MAX_EXECUTION_AUTHORITY_EVENTS {
                return Err(FactoryError::InvalidAuthoritySnapshot {
                    reason: format!(
                        "authority transition limit {} has been reached",
                        MAX_EXECUTION_AUTHORITY_EVENTS
                    ),
                });
            }
            state.events.push(event);
        }
        self.persist_state(&state)?;
        self.replace_cached_state(state)?;
        Ok(value)
    }

    pub fn flush(&self) -> Result<usize, FactoryError> {
        let _process_guard = self
            .process_lock
            .lock()
            .map_err(|_| authority_lock_error("acquire process authority lock"))?;
        let _file_guard = self.acquire_file_lock()?;
        let state = self.current_state()?;
        let bytes = self.persist_state(&state)?;
        self.replace_cached_state(state)?;
        Ok(bytes)
    }

    pub fn snapshot(&self) -> Result<ExecutionAuthoritySnapshot, FactoryError> {
        let _process_guard = self
            .process_lock
            .lock()
            .map_err(|_| authority_lock_error("acquire process authority lock"))?;
        let state = self.current_state()?;
        let snapshot = state.snapshot()?;
        self.replace_cached_state(state)?;
        Ok(snapshot)
    }

    pub fn job(&self, job_id: &str) -> Result<Option<Job>, FactoryError> {
        Ok(self
            .snapshot()?
            .queue
            .jobs
            .into_iter()
            .find(|job| job.id == job_id))
    }

    pub fn active_lease(&self, job_id: &str) -> Result<Option<Lease>, FactoryError> {
        Ok(self
            .snapshot()?
            .queue
            .leases
            .into_iter()
            .find(|lease| lease.job_id == job_id))
    }

    pub fn status(&self) -> Result<ExecutionAuthorityStatus, FactoryError> {
        let snapshot = self.snapshot()?;
        let lock = self.read_lock_info();
        Ok(ExecutionAuthorityStatus {
            configured: self.path.is_some(),
            path: self.path.as_ref().map(|path| path.display().to_string()),
            lock_path: self
                .lock_path
                .as_ref()
                .map(|path| path.display().to_string()),
            lock_present: lock.is_some(),
            lock,
            schema_version: snapshot.schema_version,
            revision: snapshot.revision,
            authority_epoch: snapshot.authority_epoch,
            event_count: snapshot.events.len(),
            event_head_digest: snapshot
                .events
                .last()
                .map(|event| event.digest.clone())
                .unwrap_or_default(),
            authority_digest: snapshot.state_digest.clone(),
            queue_state_digest: snapshot.queue.state_digest,
            integrity_verified: true,
            transaction_model: "atomic queue snapshot plus hash-chained transition journal".into(),
            execution_scope: if self.path.is_some() {
                "cooperating processes sharing one local filesystem".into()
            } else {
                "single-process in-memory authority".into()
            },
            operator_recovery:
                "orphaned locks require explicit operator release; no silent takeover".into(),
        })
    }

    /// Explicitly remove a lock left by a crashed or paused process and record the action.
    pub fn release_orphaned_lock(
        &self,
        operator: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<AuthorityLockRelease, FactoryError> {
        if operator.trim().is_empty() {
            return Err(FactoryError::UnattributedAuthorityAction {
                operation: "release authority lock".into(),
            });
        }
        validate_text(operator, "authority operator")?;
        validate_text(reason, "authority release reason")?;
        let lock_path = self
            .lock_path
            .as_ref()
            .ok_or_else(|| FactoryError::AuthorityIo {
                operation: "release authority lock".into(),
                path: "<memory>".into(),
                reason: "no shared authority path is configured".into(),
            })?
            .clone();
        let _process_guard = self
            .process_lock
            .lock()
            .map_err(|_| authority_lock_error("acquire process authority lock"))?;
        let previous_owner = self
            .read_lock_info()
            .ok_or_else(|| FactoryError::AuthorityIo {
                operation: "release authority lock".into(),
                path: lock_path.display().to_string(),
                reason: "no orphaned authority lock is present".into(),
            })?;
        let quarantine_path = lock_path.with_file_name(format!(
            ".{}.released-{}",
            lock_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("authority-lock"),
            NEXT_AUTHORITY_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&lock_path, &quarantine_path).map_err(|error| FactoryError::AuthorityIo {
            operation: "quarantine authority lock".into(),
            path: lock_path.display().to_string(),
            reason: error.to_string(),
        })?;
        fs::remove_dir_all(&quarantine_path).map_err(|error| FactoryError::AuthorityIo {
            operation: "remove authority lock".into(),
            path: quarantine_path.display().to_string(),
            reason: error.to_string(),
        })?;
        drop(_process_guard);

        let key = format!("authority-lock-release:{}:{}", operator, at.as_nanos_utc());
        self.mutate(
            AuthorityMutation::new(
                ExecutionOperation::LockReleased,
                key,
                None,
                Some(operator.to_string()),
                None,
                at,
                json!({
                    "reason": reason,
                    "previous_owner": previous_owner,
                }),
            ),
            |_| Ok(()),
        )?;
        let recorded_revision = self.status()?.revision;
        Ok(AuthorityLockRelease {
            operator: operator.to_string(),
            reason: reason.to_string(),
            previous_owner,
            recorded_revision,
        })
    }

    fn current_state(&self) -> Result<AuthorityState, FactoryError> {
        match self.path.as_deref() {
            Some(path) if path.exists() => {
                AuthorityState::from_snapshot(ExecutionAuthoritySnapshot::load_from_path(path)?)
            }
            _ => self
                .state
                .lock()
                .map_err(|_| authority_lock_error("read cached authority state"))
                .map(|state| state.clone()),
        }
    }

    fn replace_cached_state(&self, state: AuthorityState) -> Result<(), FactoryError> {
        let mut cached = self
            .state
            .lock()
            .map_err(|_| authority_lock_error("publish cached authority state"))?;
        *cached = state;
        Ok(())
    }

    fn persist_state(&self, state: &AuthorityState) -> Result<usize, FactoryError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let snapshot = state.snapshot()?;
        let bytes = snapshot.to_json_bytes()?;
        atomic_write(path, &bytes)?;
        Ok(bytes.len())
    }

    fn acquire_file_lock(&self) -> Result<Option<FileAuthorityLock>, FactoryError> {
        let Some(lock_path) = self.lock_path.as_deref() else {
            return Ok(None);
        };
        if let Some(parent) = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| FactoryError::AuthorityIo {
                operation: "create authority lock parent".into(),
                path: parent.display().to_string(),
                reason: error.to_string(),
            })?;
        }
        let mut acquired = false;
        for retry in 0..=AUTHORITY_LOCK_RETRIES {
            match fs::create_dir(lock_path) {
                Ok(()) => {
                    acquired = true;
                    break;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists
                        && retry < AUTHORITY_LOCK_RETRIES =>
                {
                    thread::sleep(AUTHORITY_LOCK_RETRY_DELAY);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(FactoryError::AuthorityBusy {
                        path: lock_path.display().to_string(),
                    });
                }
                Err(error) => {
                    return Err(FactoryError::AuthorityIo {
                        operation: "acquire authority lock".into(),
                        path: lock_path.display().to_string(),
                        reason: error.to_string(),
                    });
                }
            }
        }
        if acquired {
            let info = AuthorityLockInfo {
                owner_id: self.owner_id.clone(),
                acquired_unix_nanos: unix_nanos(),
            };
            let owner_path = lock_path.join(AUTHORITY_LOCK_FILE);
            let bytes = serde_json::to_vec(&info).map_err(|error| FactoryError::AuthorityIo {
                operation: "serialize authority lock owner".into(),
                path: owner_path.display().to_string(),
                reason: error.to_string(),
            })?;
            if let Err(error) = fs::write(&owner_path, bytes) {
                let _ = fs::remove_dir_all(lock_path);
                return Err(FactoryError::AuthorityIo {
                    operation: "write authority lock owner".into(),
                    path: owner_path.display().to_string(),
                    reason: error.to_string(),
                });
            }
            Ok(Some(FileAuthorityLock {
                path: lock_path.to_path_buf(),
            }))
        } else {
            Err(FactoryError::AuthorityBusy {
                path: lock_path.display().to_string(),
            })
        }
    }

    fn read_lock_info(&self) -> Option<AuthorityLockInfo> {
        let path = self.lock_path.as_ref()?.join(AUTHORITY_LOCK_FILE);
        let bytes = fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
}

struct FileAuthorityLock {
    path: PathBuf,
}

impl Drop for FileAuthorityLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn authority_lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("execution-authority");
    path.with_file_name(format!(".{file_name}.authority-lock"))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), FactoryError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| FactoryError::AuthorityIo {
            operation: "create authority snapshot directory".into(),
            path: parent.display().to_string(),
            reason: error.to_string(),
        })?;
    }
    let filename = path
        .file_name()
        .ok_or_else(|| FactoryError::AuthorityIo {
            operation: "name authority temporary file".into(),
            path: path.display().to_string(),
            reason: "authority path must name a file".into(),
        })?
        .to_string_lossy();
    let temporary = path.with_file_name(format!(
        ".{filename}.authority-tmp-{}-{}",
        std::process::id(),
        NEXT_AUTHORITY_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let write_result = (|| -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(FactoryError::AuthorityIo {
            operation: "write authority temporary snapshot".into(),
            path: temporary.display().to_string(),
            reason: error.to_string(),
        });
    }
    if let Err(error) = fs::rename(&temporary, path) {
        #[cfg(windows)]
        {
            let _first_error = error;
            let _ = fs::remove_file(path);
            if let Err(second_error) = fs::rename(&temporary, path) {
                let _ = fs::remove_file(&temporary);
                return Err(FactoryError::AuthorityIo {
                    operation: "install authority snapshot".into(),
                    path: path.display().to_string(),
                    reason: second_error.to_string(),
                });
            }
        }
        #[cfg(not(windows))]
        {
            let _ = fs::remove_file(&temporary);
            return Err(FactoryError::AuthorityIo {
                operation: "install authority snapshot".into(),
                path: path.display().to_string(),
                reason: error.to_string(),
            });
        }
    }
    Ok(())
}

fn digest_value(value: &Value) -> Result<String, FactoryError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| FactoryError::InvalidAuthoritySnapshot {
            reason: format!("authority digest could not be computed: {error}"),
        })
}

fn validate_text(value: &str, label: &str) -> Result<(), FactoryError> {
    if value.is_empty()
        || value.len() > MAX_AUTHORITY_FIELD_BYTES
        || value.bytes().any(|byte| byte < 0x20)
    {
        return Err(FactoryError::InvalidAuthoritySnapshot {
            reason: format!("{label} is empty, too long, or contains control bytes"),
        });
    }
    Ok(())
}

fn unix_nanos() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as i128)
        .unwrap_or_default()
}

fn authority_lock_error(operation: &str) -> FactoryError {
    FactoryError::AuthorityIo {
        operation: operation.into(),
        path: "<process>".into(),
        reason: "authority mutex is poisoned".into(),
    }
}
