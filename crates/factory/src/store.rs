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
//! In-memory and single-process. A durable multi-node store needs the event ledger of 40.09 and a
//! transactional backend; what is here is the lifecycle logic those would wrap, not a substitute.

use crate::error::FactoryError;
use crate::job::{Idempotency, Job, JobState, ResourceClass};
use crate::lease::{Lease, WorkerCapability};
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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

    pub fn job(&self, id: &str) -> Option<&Job> {
        self.jobs.get(id)
    }

    /// Enqueues a job, deduplicating on its idempotency key.
    ///
    /// Returns the id of the existing job when the same work is already present, so a retrying
    /// submitter does not create a second copy of work already in flight.
    pub fn enqueue(&mut self, job: Job) -> Result<String, FactoryError> {
        let key = job.idempotency_key().as_str().to_string();
        if let Some(existing) = self.by_key.get(&key) {
            if self.jobs.get(existing).is_some_and(|j| !j.state.is_terminal()) {
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
            expires_at: Timestamp::from_nanos_utc(
                now.as_nanos_utc() + worker.lease_duration_nanos,
            ),
            last_heartbeat: now,
        };
        self.leases.insert(job.id.clone(), lease.clone());
        Ok(Some(lease))
    }

    pub fn heartbeat(
        &mut self,
        job_id: &str,
        worker_id: &str,
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
        now: Timestamp,
        output: Value,
    ) -> Result<(), FactoryError> {
        self.require_live_lease(job_id, worker_id, now)?;
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
        now: Timestamp,
    ) -> Result<(), FactoryError> {
        self.require_live_lease(job_id, worker_id, now)?;
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
        now: Timestamp,
        reason: impl Into<String>,
    ) -> Result<Recovery, FactoryError> {
        self.require_live_lease(job_id, worker_id, now)?;
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
    pub fn release_quarantine(
        &mut self,
        job_id: &str,
        operator: &str,
    ) -> Result<(), FactoryError> {
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

    fn require_live_lease(
        &self,
        job_id: &str,
        worker_id: &str,
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
        if lease.is_expired(now) {
            return Err(FactoryError::LeaseExpired {
                job_id: job_id.to_string(),
            });
        }
        Ok(())
    }
}
