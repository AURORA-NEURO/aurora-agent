//! The job store.
//!
//! Blueprint 40.30's execution path — enqueue, lease, heartbeat, stage, commit, recover — with the
//! four non-negotiable invariants enforced by the store rather than by worker discipline:
//!
//! 1. one active lease per job attempt;
//! 2. lease expiry does not imply safe retry for non-idempotent effects;
//! 3. outputs commit atomically;
//! 4. cancellation and compensation are explicit.
//!
//! The live [`JobStore`] is single-process. [`JobStore::checkpoint_to_path`] provides a bounded
//! recovery image, while [`crate::authority::SharedExecutionAuthority`] adds a local shared-file
//! lock and hash-chained transition journal for cooperating processes. A multi-host deployment
//! still needs the event ledger of 40.09 behind a transactional backend, cross-host fencing and
//! consensus; neither local checkpoint is a substitute for those.

use crate::admission::QueueAdmissionPolicy;
use crate::error::FactoryError;
use crate::job::{Idempotency, Job, JobState, ResourceClass};
use crate::lease::{Lease, WorkerCapability};
use crate::snapshot::{
    CompensationRecord, IdempotencyIndexEntry, JobStoreSnapshot, OutputRecord,
    JOB_STORE_SNAPSHOT_SCHEMA_VERSION, MAX_JOB_STORE_SNAPSHOT_ID_BYTES,
    MAX_JOB_STORE_SNAPSHOT_JOBS, MAX_JOB_STORE_SNAPSHOT_VALUE_BYTES,
    MAX_JOB_STORE_SNAPSHOT_WORKER_ID_BYTES,
};
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// What happened to an attempt whose lease ran out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Recovery {
    /// Idempotent work: requeued for another attempt.
    Requeued { job_id: String, attempt: u32 },
    /// Non-idempotent work: held for a human. The effect may or may not have landed, and the store
    /// cannot tell which from a missed heartbeat.
    Quarantined { job_id: String, reason: String },
    /// Compensable work: compensation must run before the job is eligible again.
    AwaitingCompensation { job_id: String },
    /// Out of attempts.
    DeadLettered { job_id: String, attempts: u32 },
}

#[derive(Debug, Clone, Default)]
pub struct JobStore {
    jobs: BTreeMap<String, Job>,
    leases: BTreeMap<String, Lease>,
    /// Staged but uncommitted outputs, keyed by job id. Never visible through `result`.
    staged: BTreeMap<String, Value>,
    committed: BTreeMap<String, Value>,
    /// Deduplication index over idempotency keys.
    by_key: BTreeMap<String, String>,
    compensated: BTreeMap<String, bool>,
}

impl JobStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Create a deterministic, content-addressed checkpoint of the complete lifecycle state.
    ///
    /// A checkpoint is intentionally separate from the event ledger: it is a compact recovery
    /// image, while the ledger is what a multi-node deployment would use for audit and conflict
    /// resolution. Restoring one validates every cross-index before exposing the store.
    pub fn snapshot(&self) -> Result<JobStoreSnapshot, FactoryError> {
        let mut snapshot = JobStoreSnapshot {
            schema_version: JOB_STORE_SNAPSHOT_SCHEMA_VERSION,
            jobs: self.jobs.values().cloned().collect(),
            leases: self.leases.values().cloned().collect(),
            staged: self
                .staged
                .iter()
                .map(|(job_id, value)| OutputRecord {
                    job_id: job_id.clone(),
                    value: value.clone(),
                })
                .collect(),
            committed: self
                .committed
                .iter()
                .map(|(job_id, value)| OutputRecord {
                    job_id: job_id.clone(),
                    value: value.clone(),
                })
                .collect(),
            idempotency_index: self
                .by_key
                .iter()
                .map(|(key, job_id)| IdempotencyIndexEntry {
                    key: key.clone(),
                    job_id: job_id.clone(),
                })
                .collect(),
            compensation: self
                .compensated
                .iter()
                .map(|(job_id, completed)| CompensationRecord {
                    job_id: job_id.clone(),
                    completed: *completed,
                })
                .collect(),
            state_digest: String::new(),
        };
        snapshot.state_digest = snapshot.computed_digest()?;
        Ok(snapshot)
    }

    /// Restore a store from a validated checkpoint.
    ///
    /// The digest is checked before structural validation. A caller must not turn malformed or
    /// partially written state into an empty queue: every failure is returned explicitly.
    pub fn from_snapshot(snapshot: JobStoreSnapshot) -> Result<Self, FactoryError> {
        snapshot.verify_digest()?;
        if snapshot.schema_version != JOB_STORE_SNAPSHOT_SCHEMA_VERSION {
            return Err(FactoryError::InvalidSnapshot {
                reason: format!(
                    "unsupported schema version {}; expected {}",
                    snapshot.schema_version, JOB_STORE_SNAPSHOT_SCHEMA_VERSION
                ),
            });
        }
        if snapshot.jobs.len() > MAX_JOB_STORE_SNAPSHOT_JOBS {
            return Err(FactoryError::InvalidSnapshot {
                reason: format!(
                    "contains {} jobs above the {}-job bound",
                    snapshot.jobs.len(),
                    MAX_JOB_STORE_SNAPSHOT_JOBS
                ),
            });
        }

        let mut jobs = BTreeMap::new();
        for job in snapshot.jobs {
            validate_identifier(&job.id, MAX_JOB_STORE_SNAPSHOT_ID_BYTES, "job id")?;
            if jobs.insert(job.id.clone(), job).is_some() {
                return Err(FactoryError::InvalidSnapshot {
                    reason: "contains duplicate job ids".into(),
                });
            }
        }

        let mut leases = BTreeMap::new();
        for lease in snapshot.leases {
            validate_identifier(
                &lease.job_id,
                MAX_JOB_STORE_SNAPSHOT_ID_BYTES,
                "lease job id",
            )?;
            validate_identifier(
                &lease.worker_id,
                MAX_JOB_STORE_SNAPSHOT_WORKER_ID_BYTES,
                "lease worker id",
            )?;
            if lease.attempt == 0 {
                return Err(FactoryError::InvalidSnapshot {
                    reason: format!("lease for {} has attempt zero", lease.job_id),
                });
            }
            if leases.insert(lease.job_id.clone(), lease).is_some() {
                return Err(FactoryError::InvalidSnapshot {
                    reason: "contains duplicate active leases".into(),
                });
            }
        }

        let mut staged = BTreeMap::new();
        for output in snapshot.staged {
            validate_output(&output, &jobs, &leases, JobState::Staged)?;
            if staged.insert(output.job_id.clone(), output.value).is_some() {
                return Err(FactoryError::InvalidSnapshot {
                    reason: "contains duplicate staged output records".into(),
                });
            }
        }

        let mut committed = BTreeMap::new();
        for output in snapshot.committed {
            validate_output(&output, &jobs, &BTreeMap::new(), JobState::Succeeded)?;
            if committed
                .insert(output.job_id.clone(), output.value)
                .is_some()
            {
                return Err(FactoryError::InvalidSnapshot {
                    reason: "contains duplicate committed output records".into(),
                });
            }
        }

        for job in jobs.values() {
            let has_lease = leases.contains_key(&job.id);
            let has_staged = staged.contains_key(&job.id);
            let has_committed = committed.contains_key(&job.id);
            match job.state {
                JobState::Queued
                | JobState::Failed
                | JobState::Quarantined
                | JobState::DeadLettered
                | JobState::Cancelled
                    if has_lease || has_staged || has_committed =>
                {
                    return Err(FactoryError::InvalidSnapshot {
                        reason: format!(
                            "job {} in {:?} has incompatible lease or output state",
                            job.id, job.state
                        ),
                    });
                }
                JobState::Leased if !has_lease || has_staged || has_committed => {
                    return Err(FactoryError::InvalidSnapshot {
                        reason: format!(
                            "leased job {} must have exactly one lease and no staged or committed output",
                            job.id
                        ),
                    });
                }
                JobState::Staged if !has_lease || !has_staged || has_committed => {
                    return Err(FactoryError::InvalidSnapshot {
                        reason: format!(
                            "staged job {} must have one lease, staged output, and no committed output",
                            job.id
                        ),
                    });
                }
                JobState::Succeeded if has_lease || has_staged || !has_committed => {
                    return Err(FactoryError::InvalidSnapshot {
                        reason: format!(
                            "succeeded job {} must have committed output and no active lease",
                            job.id
                        ),
                    });
                }
                JobState::Leased | JobState::Staged | JobState::Succeeded => {}
                JobState::Queued
                | JobState::Failed
                | JobState::Quarantined
                | JobState::DeadLettered
                | JobState::Cancelled => {}
            }
            if let Some(lease) = leases.get(&job.id) {
                if lease.attempt != job.attempts {
                    return Err(FactoryError::InvalidSnapshot {
                        reason: format!(
                            "lease for {} is attempt {}, but job records attempt {}",
                            job.id, lease.attempt, job.attempts
                        ),
                    });
                }
            }
        }

        let mut by_key = BTreeMap::new();
        for entry in snapshot.idempotency_index {
            validate_identifier(
                &entry.key,
                MAX_JOB_STORE_SNAPSHOT_ID_BYTES,
                "idempotency key",
            )?;
            let job = jobs
                .get(&entry.job_id)
                .ok_or_else(|| FactoryError::InvalidSnapshot {
                    reason: format!(
                        "idempotency index entry {} references unknown job {}",
                        entry.key, entry.job_id
                    ),
                })?;
            if job.idempotency_key().as_str() != entry.key {
                return Err(FactoryError::InvalidSnapshot {
                    reason: format!(
                        "idempotency index entry for {} does not match the job specification",
                        entry.job_id
                    ),
                });
            }
            if by_key.insert(entry.key, entry.job_id).is_some() {
                return Err(FactoryError::InvalidSnapshot {
                    reason: "contains duplicate idempotency index keys".into(),
                });
            }
        }

        let mut compensated = BTreeMap::new();
        for entry in snapshot.compensation {
            let job = jobs
                .get(&entry.job_id)
                .ok_or_else(|| FactoryError::InvalidSnapshot {
                    reason: format!(
                        "compensation record references unknown job {}",
                        entry.job_id
                    ),
                })?;
            if job.idempotency != Idempotency::Compensable {
                return Err(FactoryError::InvalidSnapshot {
                    reason: format!(
                        "compensation record references non-compensable job {}",
                        entry.job_id
                    ),
                });
            }
            if !entry.completed && job.state != JobState::Quarantined {
                return Err(FactoryError::InvalidSnapshot {
                    reason: format!(
                        "incomplete compensation record for {} requires quarantined state",
                        entry.job_id
                    ),
                });
            }
            if compensated
                .insert(entry.job_id.clone(), entry.completed)
                .is_some()
            {
                return Err(FactoryError::InvalidSnapshot {
                    reason: "contains duplicate compensation records".into(),
                });
            }
        }

        Ok(JobStore {
            jobs,
            leases,
            staged,
            committed,
            by_key,
            compensated,
        })
    }

    /// Load a checkpoint, treating a missing file as a new empty store.
    pub fn load_from_path(path: &Path) -> Result<Self, FactoryError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::new()),
            Err(error) => return Err(snapshot_io("read", path, error)),
        };
        let snapshot = JobStoreSnapshot::from_json_bytes(&bytes)?;
        Self::from_snapshot(snapshot)
    }

    /// Atomically write a bounded checkpoint. The target is replaced only after the complete JSON
    /// document has been written and validated in memory.
    pub fn checkpoint_to_path(&self, path: &Path) -> Result<usize, FactoryError> {
        let snapshot = self.snapshot()?;
        let bytes = snapshot.to_json_bytes()?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| snapshot_io("create directory", path, error))?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| FactoryError::SnapshotIo {
                operation: "name temporary file".into(),
                path: path.display().to_string(),
                reason: "path must name a file".into(),
            })?
            .to_string_lossy();
        let sequence = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let temporary =
            path.with_file_name(format!(".{filename}.tmp-{}-{sequence}", std::process::id()));
        if let Err(error) = std::fs::write(&temporary, &bytes) {
            let _ = std::fs::remove_file(&temporary);
            return Err(snapshot_io("write temporary file", &temporary, error));
        }
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                if let Err(second_error) = std::fs::rename(&temporary, path) {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(FactoryError::SnapshotIo {
                        operation: "install snapshot".into(),
                        path: path.display().to_string(),
                        reason: format!("{first_error}; retry: {second_error}"),
                    });
                }
            }
            #[cfg(not(windows))]
            {
                let _ = std::fs::remove_file(&temporary);
                return Err(snapshot_io("install snapshot", path, first_error));
            }
        }
        Ok(bytes.len())
    }

    /// Recover expired work and persist the resulting branch in one explicit operation.
    pub fn recover_expired_at_path(
        path: &Path,
        now: Timestamp,
    ) -> Result<Vec<Recovery>, FactoryError> {
        let mut store = Self::load_from_path(path)?;
        let recoveries = store.recover_expired(now);
        store.checkpoint_to_path(path)?;
        Ok(recoveries)
    }

    pub fn job(&self, id: &str) -> Option<&Job> {
        self.jobs.get(id)
    }

    /// Return the live lease without exposing the store's mutable ownership boundary.
    ///
    /// Shared authority callers use this when an idempotent enqueue is retried after the first
    /// process accepted the job. Returning the existing lease is safe because the lease still
    /// carries the same attempt fence; minting a second lease would be the unsafe behavior.
    pub fn active_lease(&self, job_id: &str) -> Option<&Lease> {
        self.leases.get(job_id)
    }

    /// Enqueues a job, deduplicating on its idempotency key.
    ///
    /// Returns the id of the existing job when the same work is already present, so a retrying
    /// submitter does not create a second copy of work already in flight.
    pub fn enqueue(&mut self, job: Job) -> Result<String, FactoryError> {
        let key = job.idempotency_key().as_str().to_string();
        if let Some(existing) = self.by_key.get(&key) {
            if self
                .jobs
                .get(existing)
                .is_some_and(|j| !j.state.is_terminal())
            {
                return Ok(existing.clone());
            }
        }
        if self.jobs.contains_key(&job.id) {
            return Err(FactoryError::DuplicateJobId {
                job_id: job.id.clone(),
            });
        }
        let id = job.id.clone();
        self.by_key.insert(key, id.clone());
        self.jobs.insert(id.clone(), job);
        Ok(id)
    }

    /// Enqueue only when the deployment's explicit backpressure and class fair-share policy has
    /// capacity. Existing active idempotent work is deduplicated before admission, so a retry of
    /// accepted work does not consume another slot.
    pub fn enqueue_with_policy(
        &mut self,
        job: Job,
        policy: &QueueAdmissionPolicy,
    ) -> Result<String, FactoryError> {
        let key = job.idempotency_key().as_str().to_string();
        if let Some(existing) = self.by_key.get(&key) {
            if self
                .jobs
                .get(existing)
                .is_some_and(|j| !j.state.is_terminal())
            {
                return Ok(existing.clone());
            }
        }
        policy.check_enqueue(self, &job)?;
        self.enqueue(job)
    }

    /// Lease through the policy-controlled path. The class is checked before any mutation, so a
    /// full active-lease budget is an explicit backpressure refusal rather than an accidental
    /// second worker race.
    pub fn lease_with_policy(
        &mut self,
        worker: &WorkerCapability,
        now: Timestamp,
        policy: &QueueAdmissionPolicy,
    ) -> Result<Option<Lease>, FactoryError> {
        policy.validate()?;
        let compatible_classes = self
            .jobs
            .values()
            .filter(|job| job.state.is_claimable() && worker.can_run(job.resource_class))
            .map(|job| job.resource_class)
            .collect::<Vec<_>>();
        if compatible_classes.is_empty() {
            return Ok(None);
        }
        if self.active_lease_count() >= policy.max_active_leases {
            return Err(FactoryError::AdmissionLimit {
                dimension: "active_leases".into(),
                limit: policy.max_active_leases,
                observed: self.active_lease_count(),
            });
        }
        let active_by_class = self.active_lease_counts_by_class();
        let mut candidates: Vec<&mut Job> = self
            .jobs
            .values_mut()
            .filter(|job| {
                if !job.state.is_claimable() || !worker.can_run(job.resource_class) {
                    return false;
                }
                match policy.max_active_leases_by_class.get(&job.resource_class) {
                    Some(limit) => {
                        active_by_class
                            .get(&job.resource_class)
                            .copied()
                            .unwrap_or(0)
                            < *limit
                    }
                    None => true,
                }
            })
            .collect();
        if candidates.is_empty() {
            policy.check_lease(self, compatible_classes[0])?;
            return Ok(None);
        }
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        let job = candidates
            .into_iter()
            .next()
            .expect("candidate list is non-empty");
        job.attempts += 1;
        job.state = JobState::Leased;
        let lease = Lease {
            job_id: job.id.clone(),
            worker_id: worker.worker_id.clone(),
            attempt: job.attempts,
            granted_at: now,
            expires_at: Timestamp::from_nanos_utc(now.as_nanos_utc() + worker.lease_duration_nanos),
            last_heartbeat: now,
        };
        self.leases.insert(job.id.clone(), lease.clone());
        Ok(Some(lease))
    }

    /// Grants a lease to a compatible worker, highest priority first.
    ///
    /// Enforces invariant 1: a job already leased is not claimable, so a second worker cannot take
    /// the same attempt.
    pub fn lease(
        &mut self,
        worker: &WorkerCapability,
        now: Timestamp,
    ) -> Result<Option<Lease>, FactoryError> {
        let mut candidates: Vec<&mut Job> = self
            .jobs
            .values_mut()
            .filter(|job| job.state.is_claimable() && worker.can_run(job.resource_class))
            .collect();
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));

        let Some(job) = candidates.into_iter().next() else {
            return Ok(None);
        };

        job.attempts += 1;
        job.state = JobState::Leased;
        let lease = Lease {
            job_id: job.id.clone(),
            worker_id: worker.worker_id.clone(),
            attempt: job.attempts,
            granted_at: now,
            expires_at: Timestamp::from_nanos_utc(now.as_nanos_utc() + worker.lease_duration_nanos),
            last_heartbeat: now,
        };
        self.leases.insert(job.id.clone(), lease.clone());
        Ok(Some(lease))
    }

    pub fn heartbeat(
        &mut self,
        job_id: &str,
        worker_id: &str,
        attempt: u32,
        now: Timestamp,
        duration_nanos: i128,
    ) -> Result<(), FactoryError> {
        let lease = self
            .leases
            .get_mut(job_id)
            .ok_or_else(|| FactoryError::NoActiveLease {
                job_id: job_id.to_string(),
            })?;
        if lease.worker_id != worker_id {
            return Err(FactoryError::LeaseHeldByAnother {
                job_id: job_id.to_string(),
                holder: lease.worker_id.clone(),
            });
        }
        if lease.attempt != attempt {
            return Err(FactoryError::StaleLease {
                job_id: job_id.to_string(),
                expected_attempt: attempt,
                active_attempt: lease.attempt,
            });
        }
        if !lease.heartbeat(now, duration_nanos) {
            return Err(FactoryError::LeaseExpired {
                job_id: job_id.to_string(),
            });
        }
        Ok(())
    }

    /// Stages an output. Not visible to readers until committed (invariant 3).
    pub fn stage(
        &mut self,
        job_id: &str,
        worker_id: &str,
        attempt: u32,
        now: Timestamp,
        output: Value,
    ) -> Result<(), FactoryError> {
        self.require_live_lease(job_id, worker_id, attempt, now)?;
        self.staged.insert(job_id.to_string(), output);
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.state = JobState::Staged;
        }
        Ok(())
    }

    /// Commits staged output atomically and releases the lease.
    ///
    /// Refuses if nothing was staged: a success with no output is a worker bug, and recording it
    /// would leave a job marked succeeded with nothing to show.
    pub fn commit(
        &mut self,
        job_id: &str,
        worker_id: &str,
        attempt: u32,
        now: Timestamp,
    ) -> Result<(), FactoryError> {
        self.require_live_lease(job_id, worker_id, attempt, now)?;
        let output = self
            .staged
            .remove(job_id)
            .ok_or_else(|| FactoryError::NothingStaged {
                job_id: job_id.to_string(),
            })?;
        self.committed.insert(job_id.to_string(), output);
        self.leases.remove(job_id);
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.state = JobState::Succeeded;
        }
        Ok(())
    }

    /// Records a typed failure the worker observed and reported.
    ///
    /// Distinct from lease expiry: a reported failure is *unambiguous* — the worker was alive and
    /// says the work did not land — so even a non-idempotent job may be retried.
    pub fn fail(
        &mut self,
        job_id: &str,
        worker_id: &str,
        attempt: u32,
        now: Timestamp,
        reason: impl Into<String>,
    ) -> Result<Recovery, FactoryError> {
        self.require_live_lease(job_id, worker_id, attempt, now)?;
        self.staged.remove(job_id);
        self.leases.remove(job_id);

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| FactoryError::UnknownJob {
                job_id: job_id.to_string(),
            })?;
        job.reason = Some(reason.into());

        if job.attempts_remaining() == 0 {
            job.state = JobState::DeadLettered;
            return Ok(Recovery::DeadLettered {
                job_id: job.id.clone(),
                attempts: job.attempts,
            });
        }
        job.state = JobState::Queued;
        Ok(Recovery::Requeued {
            job_id: job.id.clone(),
            attempt: job.attempts,
        })
    }

    /// Sweeps expired leases and applies the idempotency-aware recovery policy.
    ///
    /// This is invariant 2. A missed heartbeat is *ambiguous*: the worker may have completed the
    /// effect and died before committing. Only idempotent work may be retried on that evidence.
    pub fn recover_expired(&mut self, now: Timestamp) -> Vec<Recovery> {
        let expired: Vec<String> = self
            .leases
            .values()
            .filter(|lease| lease.is_expired(now))
            .map(|lease| lease.job_id.clone())
            .collect();

        let mut recoveries = Vec::new();
        for job_id in expired {
            self.leases.remove(&job_id);
            self.staged.remove(&job_id);

            let Some(job) = self.jobs.get_mut(&job_id) else {
                continue;
            };

            if job.attempts_remaining() == 0 {
                job.state = JobState::DeadLettered;
                job.reason = Some("lease expired and no attempts remain".into());
                recoveries.push(Recovery::DeadLettered {
                    job_id,
                    attempts: job.attempts,
                });
                continue;
            }

            match job.idempotency {
                Idempotency::Idempotent => {
                    job.state = JobState::Queued;
                    job.reason = Some("lease expired; idempotent work requeued".into());
                    recoveries.push(Recovery::Requeued {
                        job_id,
                        attempt: job.attempts,
                    });
                }
                Idempotency::NonIdempotent => {
                    job.state = JobState::Quarantined;
                    job.reason = Some(
                        "lease expired on non-idempotent work; the effect may or may not have \
                         landed, and a missed heartbeat cannot distinguish the two"
                            .into(),
                    );
                    recoveries.push(Recovery::Quarantined {
                        job_id,
                        reason: job.reason.clone().unwrap_or_default(),
                    });
                }
                Idempotency::Compensable => {
                    job.state = JobState::Quarantined;
                    job.reason = Some("lease expired; compensation required before retry".into());
                    self.compensated.insert(job_id.clone(), false);
                    recoveries.push(Recovery::AwaitingCompensation { job_id });
                }
            }
        }
        recoveries
    }

    /// Records that a compensable job's first attempt was undone, making it eligible again.
    pub fn compensate(&mut self, job_id: &str) -> Result<(), FactoryError> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| FactoryError::UnknownJob {
                job_id: job_id.to_string(),
            })?;
        if job.idempotency != Idempotency::Compensable {
            return Err(FactoryError::NotCompensable {
                job_id: job_id.to_string(),
            });
        }
        if job.state != JobState::Quarantined {
            return Err(FactoryError::NotAwaitingCompensation {
                job_id: job_id.to_string(),
            });
        }
        self.compensated.insert(job_id.to_string(), true);
        job.state = JobState::Queued;
        job.reason = Some("compensated; eligible for retry".into());
        Ok(())
    }

    /// Releases a quarantined non-idempotent job after a human decided it is safe.
    pub fn release_quarantine(&mut self, job_id: &str, operator: &str) -> Result<(), FactoryError> {
        if operator.trim().is_empty() {
            return Err(FactoryError::UnattributedRelease {
                job_id: job_id.to_string(),
            });
        }
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| FactoryError::UnknownJob {
                job_id: job_id.to_string(),
            })?;
        if job.state != JobState::Quarantined {
            return Err(FactoryError::NotQuarantined {
                job_id: job_id.to_string(),
            });
        }
        job.state = JobState::Queued;
        job.reason = Some(format!("quarantine released by {operator}"));
        Ok(())
    }

    /// Cancels a job explicitly (invariant 4), dropping any staged output.
    pub fn cancel(&mut self, job_id: &str, reason: impl Into<String>) -> Result<(), FactoryError> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| FactoryError::UnknownJob {
                job_id: job_id.to_string(),
            })?;
        if job.state.is_terminal() {
            return Err(FactoryError::AlreadyTerminal {
                job_id: job_id.to_string(),
                state: format!("{:?}", job.state),
            });
        }
        job.state = JobState::Cancelled;
        job.reason = Some(reason.into());
        self.leases.remove(job_id);
        self.staged.remove(job_id);
        Ok(())
    }

    /// Committed output only. Staged work is invisible here by construction.
    pub fn result(&self, job_id: &str) -> Option<&Value> {
        self.committed.get(job_id)
    }

    pub fn quarantined(&self) -> Vec<&Job> {
        self.jobs
            .values()
            .filter(|job| job.state == JobState::Quarantined)
            .collect()
    }

    pub fn dead_lettered(&self) -> Vec<&Job> {
        self.jobs
            .values()
            .filter(|job| job.state == JobState::DeadLettered)
            .collect()
    }

    pub fn counts_by_class(&self) -> BTreeMap<ResourceClass, usize> {
        let mut counts = BTreeMap::new();
        for job in self.jobs.values() {
            *counts.entry(job.resource_class).or_insert(0) += 1;
        }
        counts
    }

    pub fn active_lease_count(&self) -> usize {
        self.leases.len()
    }

    pub fn active_lease_counts_by_class(&self) -> BTreeMap<ResourceClass, usize> {
        let mut counts = BTreeMap::new();
        for lease in self.leases.values() {
            if let Some(job) = self.jobs.get(&lease.job_id) {
                *counts.entry(job.resource_class).or_insert(0) += 1;
            }
        }
        counts
    }

    fn require_live_lease(
        &self,
        job_id: &str,
        worker_id: &str,
        attempt: u32,
        now: Timestamp,
    ) -> Result<(), FactoryError> {
        let lease = self
            .leases
            .get(job_id)
            .ok_or_else(|| FactoryError::NoActiveLease {
                job_id: job_id.to_string(),
            })?;
        if lease.worker_id != worker_id {
            return Err(FactoryError::LeaseHeldByAnother {
                job_id: job_id.to_string(),
                holder: lease.worker_id.clone(),
            });
        }
        if lease.attempt != attempt {
            return Err(FactoryError::StaleLease {
                job_id: job_id.to_string(),
                expected_attempt: attempt,
                active_attempt: lease.attempt,
            });
        }
        if lease.is_expired(now) {
            return Err(FactoryError::LeaseExpired {
                job_id: job_id.to_string(),
            });
        }
        Ok(())
    }
}

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

fn validate_identifier(value: &str, max_bytes: usize, label: &str) -> Result<(), FactoryError> {
    if value.is_empty() || value.len() > max_bytes || value.bytes().any(|byte| byte < 0x20) {
        return Err(FactoryError::InvalidSnapshot {
            reason: format!("{label} is empty, too long, or contains control bytes"),
        });
    }
    Ok(())
}

fn validate_output(
    output: &OutputRecord,
    jobs: &BTreeMap<String, Job>,
    leases: &BTreeMap<String, Lease>,
    required_state: JobState,
) -> Result<(), FactoryError> {
    validate_identifier(
        &output.job_id,
        MAX_JOB_STORE_SNAPSHOT_ID_BYTES,
        "output job id",
    )?;
    let job = jobs
        .get(&output.job_id)
        .ok_or_else(|| FactoryError::InvalidSnapshot {
            reason: format!("output references unknown job {}", output.job_id),
        })?;
    if job.state != required_state {
        return Err(FactoryError::InvalidSnapshot {
            reason: format!(
                "output for {} requires {:?} state, found {:?}",
                output.job_id, required_state, job.state
            ),
        });
    }
    if required_state == JobState::Staged && !leases.contains_key(&output.job_id) {
        return Err(FactoryError::InvalidSnapshot {
            reason: format!("staged output for {} has no active lease", output.job_id),
        });
    }
    let encoded =
        serde_json::to_vec(&output.value).map_err(|error| FactoryError::InvalidSnapshot {
            reason: format!("output for {} is not serializable: {error}", output.job_id),
        })?;
    if encoded.len() > MAX_JOB_STORE_SNAPSHOT_VALUE_BYTES {
        return Err(FactoryError::InvalidSnapshot {
            reason: format!(
                "output for {} is {} bytes, above the {}-byte bound",
                output.job_id,
                encoded.len(),
                MAX_JOB_STORE_SNAPSHOT_VALUE_BYTES
            ),
        });
    }
    Ok(())
}

fn snapshot_io(operation: &str, path: &Path, error: impl std::fmt::Display) -> FactoryError {
    FactoryError::SnapshotIo {
        operation: operation.into(),
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}
