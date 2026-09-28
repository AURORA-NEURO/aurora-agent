//! Restart-safe state adapters used by the HTTP router.
//!
//! The route handlers operate on these bounded local checkpoints while preserving their
//! explicit process-local recovery guarantees.

use super::*;

pub(super) struct MissionPersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) jobs: Arc<Mutex<BTreeMap<String, Arc<MissionJob>>>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct MissionQueuePersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) authority: Arc<SharedExecutionAuthority>,
    pub(super) startup_recoveries: Vec<FactoryRecovery>,
    pub(super) admission_policy: QueueAdmissionPolicy,
}

pub(super) struct EventPersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) events: Arc<Mutex<EventLog>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct EvidencePersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) registry: Arc<Mutex<EvidenceBundleRegistry>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct ReconciliationPersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct ArtifactPersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) registry: Arc<Mutex<ArtifactRegistry>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct WorkflowExecutionEvidencePersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct WorkbenchPersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) registry: Arc<Mutex<WorkbenchReportRegistry>>,
    pub(super) lock: Mutex<()>,
}

pub(super) struct CiProviderEvidencePersistence {
    pub(super) path: Option<PathBuf>,
    pub(super) registry: Arc<Mutex<CiProviderEvidenceRegistry>>,
    pub(super) lock: Mutex<()>,
}

impl MissionQueuePersistence {
    pub(super) fn new(
        path: Option<PathBuf>,
        admission_policy: QueueAdmissionPolicy,
    ) -> Result<Self, String> {
        admission_policy
            .validate()
            .map_err(|error| format!("invalid mission queue admission policy: {error}"))?;
        let authority = SharedExecutionAuthority::open(path.clone())
            .map_err(|error| format!("mission execution authority could not be opened: {error}"))?;
        let startup_recoveries = if path.is_some() {
            let now = current_timestamp()?;
            authority
                .mutate(
                    AuthorityMutation::new(
                        ExecutionOperation::LeaseRecovered,
                        format!("startup-recovery-sweep:{}", now.as_nanos_utc()),
                        None,
                        Some(MISSION_QUEUE_WORKER_ID.into()),
                        None,
                        now,
                        json!({ "source": "api_startup" }),
                    ),
                    |queue| Ok(queue.recover_expired(now)),
                )
                .map_err(|error| format!("mission queue startup recovery failed: {error}"))?
        } else {
            Vec::new()
        };
        authority.flush().map_err(|error| {
            format!("mission queue authority checkpoint failed during startup: {error}")
        })?;
        Ok(Self {
            path,
            authority,
            startup_recoveries,
            admission_policy,
        })
    }

    pub(super) fn persist(&self) -> Result<usize, String> {
        self.authority
            .flush()
            .map_err(|error| format!("mission queue authority checkpoint failed: {error}"))
    }

    /// Apply a queue transition through the shared authority. The queue image and journal row are
    /// committed together, and a second process reloads the latest image while holding the file
    /// lock instead of mutating a stale in-memory copy.
    pub(super) fn mutate<T, F>(
        &self,
        mutation: AuthorityMutation,
        transition: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&mut JobStore) -> Result<T, String>,
    {
        self.authority
            .mutate(mutation.clone(), |queue| {
                transition(queue).map_err(|error| {
                    bioprism_factory::FactoryError::InvalidAuthoritySnapshot { reason: error }
                })
            })
            .map_err(|error| {
                format!(
                    "mission queue {:?} was refused: {error}",
                    mutation.operation
                )
            })
    }

    pub(super) fn enqueue_and_lease(
        &self,
        job: FactoryJob,
        now: Timestamp,
    ) -> Result<FactoryLease, String> {
        let mission_id = job.id.clone();
        let work_digest = job.idempotency_key().to_string();
        let mutation = AuthorityMutation::new(
            ExecutionOperation::EnqueueAndLease,
            format!("queue-enqueue-lease:{mission_id}"),
            Some(mission_id.clone()),
            Some(MISSION_QUEUE_WORKER_ID.into()),
            Some(job.attempts.saturating_add(1).max(1)),
            now,
            json!({
                "resource_class": job.resource_class,
                "work_digest": work_digest,
            }),
        );
        self.mutate(mutation, move |queue| {
            let accepted = queue
                .enqueue_with_policy(job, &self.admission_policy)
                .map_err(|error| error.to_string())?;
            if accepted != mission_id {
                return Err(format!(
                    "duplicate work is already represented by {accepted}"
                ));
            }
            if let Some(lease) = queue.active_lease(&mission_id) {
                return Ok(lease.clone());
            }
            let worker =
                WorkerCapability::new(MISSION_QUEUE_WORKER_ID, vec![ResourceClass::Evaluate])
                    .with_lease_duration_nanos(MISSION_QUEUE_LEASE_DURATION_NANOS);
            queue
                .lease_with_policy(&worker, now, &self.admission_policy)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "no compatible mission worker capacity is available".to_string())
        })
    }

    pub(super) fn heartbeat(
        &self,
        mission_id: &str,
        attempt: u32,
        now: Timestamp,
    ) -> Result<(), String> {
        let mission_id = mission_id.to_string();
        let mutation = AuthorityMutation::new(
            ExecutionOperation::Heartbeat,
            format!(
                "queue-heartbeat:{mission_id}:{attempt}:{}",
                now.as_nanos_utc()
            ),
            Some(mission_id.clone()),
            Some(MISSION_QUEUE_WORKER_ID.into()),
            Some(attempt),
            now,
            json!({ "lease_duration_nanos": MISSION_QUEUE_LEASE_DURATION_NANOS }),
        );
        self.mutate(mutation, move |queue| {
            queue
                .heartbeat(
                    &mission_id,
                    MISSION_QUEUE_WORKER_ID,
                    attempt,
                    now,
                    MISSION_QUEUE_LEASE_DURATION_NANOS,
                )
                .map_err(|error| error.to_string())
        })
    }

    pub(super) fn commit_success(
        &self,
        mission_id: &str,
        attempt: u32,
        result: Value,
        now: Timestamp,
    ) -> Result<(), String> {
        let mission_id = mission_id.to_string();
        let result_digest = ContentHash::of_value(&result)
            .map_err(|error| format!("mission result could not be digested: {error}"))?
            .to_string();
        let mutation = AuthorityMutation::new(
            ExecutionOperation::Committed,
            format!("queue-commit:{mission_id}:{attempt}:{result_digest}"),
            Some(mission_id.clone()),
            Some(MISSION_QUEUE_WORKER_ID.into()),
            Some(attempt),
            now,
            json!({ "result_digest": result_digest }),
        );
        self.mutate(mutation, move |queue| {
            if let Some(job) = queue.job(&mission_id) {
                if job.state == bioprism_factory::JobState::Succeeded
                    && queue.result(&mission_id) == Some(&result)
                {
                    return Ok(());
                }
            }
            queue
                .stage(&mission_id, MISSION_QUEUE_WORKER_ID, attempt, now, result)
                .and_then(|_| queue.commit(&mission_id, MISSION_QUEUE_WORKER_ID, attempt, now))
                .map_err(|error| error.to_string())
        })
    }

    pub(super) fn record_failure(
        &self,
        mission_id: &str,
        attempt: u32,
        reason: String,
        now: Timestamp,
    ) -> Result<(), String> {
        let mission_id = mission_id.to_string();
        let mutation = AuthorityMutation::new(
            ExecutionOperation::Failed,
            format!("queue-failure:{mission_id}:{attempt}:{reason}"),
            Some(mission_id.clone()),
            Some(MISSION_QUEUE_WORKER_ID.into()),
            Some(attempt),
            now,
            json!({ "reason": reason }),
        );
        let retry_reason = reason.clone();
        self.mutate(mutation, move |queue| {
            if let Some(job) = queue.job(&mission_id) {
                if queue.active_lease(&mission_id).is_none()
                    && job.reason.as_deref() == Some(retry_reason.as_str())
                    && matches!(
                        job.state,
                        bioprism_factory::JobState::Queued
                            | bioprism_factory::JobState::DeadLettered
                    )
                {
                    return Ok(());
                }
            }
            queue
                .fail(&mission_id, MISSION_QUEUE_WORKER_ID, attempt, now, reason)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
    }

    pub(super) fn cancel(&self, mission_id: &str, reason: &str) -> Result<(), String> {
        let mission_id = mission_id.to_string();
        let reason = reason.to_string();
        let mutation = AuthorityMutation::new(
            ExecutionOperation::Cancelled,
            format!("queue-cancel:{mission_id}:{reason}"),
            Some(mission_id.clone()),
            Some(MISSION_QUEUE_WORKER_ID.into()),
            None,
            current_timestamp().map_err(|error| error.to_string())?,
            json!({ "reason": reason }),
        );
        let retry_reason = reason.clone();
        self.mutate(mutation, move |queue| {
            if let Some(job) = queue.job(&mission_id) {
                if job.state == bioprism_factory::JobState::Cancelled
                    && job.reason.as_deref() == Some(retry_reason.as_str())
                {
                    return Ok(());
                }
            }
            queue
                .cancel(&mission_id, reason)
                .map_err(|error| error.to_string())
        })
    }

    pub(super) fn status(&self) -> Result<Value, String> {
        let path = self.path.as_deref();
        let authority_snapshot = self
            .authority
            .snapshot()
            .map_err(|error| format!("mission authority snapshot failed: {error}"))?;
        let queue = JobStore::from_snapshot(authority_snapshot.queue.clone())
            .map_err(|error| format!("mission queue projection failed: {error}"))?;
        let authority_status = self
            .authority
            .status()
            .map_err(|error| format!("mission authority status failed: {error}"))?;
        let jobs = authority_snapshot
            .queue
            .jobs
            .iter()
            .map(mission_queue_job_json)
            .collect::<Vec<_>>();
        let file_bytes = path
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|m| m.len());
        let authority_json = serde_json::to_value(&authority_status)
            .map_err(|error| format!("mission authority status serialization failed: {error}"))?;
        Ok(json!({
            "ok": true,
            "enabled": path.is_some(),
            "file_present": file_bytes.is_some(),
            "file_bytes": file_bytes,
            "schema_version": EXECUTION_AUTHORITY_SCHEMA_VERSION,
            "queue_schema_version": JOB_STORE_SNAPSHOT_SCHEMA_VERSION,
            "state_digest": authority_snapshot.queue.state_digest,
            "authority_digest": authority_snapshot.state_digest,
            "integrity_verified": authority_status.integrity_verified,
            "max_file_bytes": MAX_EXECUTION_AUTHORITY_BYTES,
            "registry_size": jobs.len(),
            "jobs": jobs,
            "authority": authority_json,
            "admission_policy": {
                "max_jobs": self.admission_policy.max_jobs,
                "max_active_leases": self.admission_policy.max_active_leases,
                "max_jobs_by_class": self.admission_policy.max_jobs_by_class,
                "max_active_leases_by_class": self.admission_policy.max_active_leases_by_class,
                "observed_active_leases": queue.active_lease_count(),
                "observed_active_leases_by_class": queue.active_lease_counts_by_class(),
                "backpressure": "refuse_before_checkpoint_mutation"
            },
            "startup_recoveries": self.startup_recoveries,
            "automatic_resume": false,
            "execution_scope": authority_status.execution_scope,
            "recovery_policy": "expired leases are classified by idempotency at startup; no recovered job is automatically dispatched",
            "does_not_claim": [
                "multi-host consensus or network-partition tolerance",
                "external effect completion",
                "automatic resume of an interrupted mission",
                "provider authentication or tenant isolation"
            ],
            "flush": "/v1/missions/queue/persistence/flush"
        }))
    }

    pub(super) fn projection(&self, mission_id: &str) -> Result<Value, String> {
        Ok(self
            .authority
            .job(mission_id)
            .map_err(|error| format!("mission authority projection failed: {error}"))?
            .as_ref()
            .map(mission_queue_job_json)
            .unwrap_or(Value::Null))
    }

    pub(super) fn release_orphaned_lock(
        &self,
        operator: &str,
        reason: &str,
        at: Timestamp,
    ) -> Result<Value, String> {
        let release = self
            .authority
            .release_orphaned_lock(operator, reason, at)
            .map_err(|error| format!("mission authority lock release refused: {error}"))?;
        serde_json::to_value(release)
            .map_err(|error| format!("mission authority lock receipt failed: {error}"))
    }
}

impl EventPersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "event persistence lock is unavailable".to_string())?;
        let events = self
            .events
            .lock()
            .map_err(|_| "event log is unavailable".to_string())?;
        events.checkpoint_to_path(path)
    }
}

impl EvidencePersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let registry = self
            .registry
            .lock()
            .map_err(|_| "evidence registry is unavailable".to_string())?;
        let document = registry.snapshot().map_err(|error| error.to_string())?;
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "evidence persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, &document)
    }

    pub(super) fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "evidence persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, document)
    }

    pub(super) fn write_snapshot(path: &Path, document: &Value) -> Result<usize, String> {
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|error| format!("evidence state could not be serialized: {error}"))?;
        if bytes.len() > MAX_EVIDENCE_REGISTRY_BYTES {
            return Err(format!(
                "evidence state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_EVIDENCE_REGISTRY_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("evidence state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "evidence_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!("evidence state temporary file could not be written: {error}")
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "evidence state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "evidence state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(bytes.len())
    }
}

impl ReconciliationPersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let registry = self
            .registry
            .lock()
            .map_err(|_| "workflow reconciliation registry is unavailable".to_string())?;
        let document = registry.snapshot().map_err(|error| error.to_string())?;
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "workflow reconciliation persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, &document)
    }

    pub(super) fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "workflow reconciliation persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, document)
    }

    pub(super) fn write_snapshot(path: &Path, document: &Value) -> Result<usize, String> {
        let bytes = serde_json::to_vec_pretty(document).map_err(|error| {
            format!("workflow reconciliation state could not be serialized: {error}")
        })?;
        if bytes.len() > MAX_WORKFLOW_RECONCILIATION_STATE_BYTES {
            return Err(format!(
                "workflow reconciliation state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_WORKFLOW_RECONCILIATION_STATE_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("workflow reconciliation state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "reconciliation_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!("workflow reconciliation state temporary file could not be written: {error}")
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "workflow reconciliation state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "workflow reconciliation state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(bytes.len())
    }
}

impl ArtifactPersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let registry = self
            .registry
            .lock()
            .map_err(|_| "artifact registry is unavailable".to_string())?;
        let document = registry.snapshot().map_err(|error| error.to_string())?;
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "artifact registry persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, &document)
    }

    pub(super) fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "artifact registry persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, document)
    }

    pub(super) fn write_snapshot(path: &Path, document: &Value) -> Result<usize, String> {
        let bytes = serde_json::to_vec_pretty(document)
            .map_err(|error| format!("artifact state could not be serialized: {error}"))?;
        if bytes.len() > MAX_ARTIFACT_REGISTRY_BYTES {
            return Err(format!(
                "artifact state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_ARTIFACT_REGISTRY_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("artifact state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "artifact_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!("artifact state temporary file could not be written: {error}")
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "artifact state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "artifact state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(bytes.len())
    }
}

impl WorkflowExecutionEvidencePersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        if self.path.is_none() {
            return Ok(0);
        }
        let registry = self
            .registry
            .lock()
            .map_err(|_| "workflow execution evidence registry is unavailable".to_string())?;
        let document = registry.snapshot().map_err(|error| error.to_string())?;
        self.persist_snapshot(&document)
    }

    pub(super) fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self.lock.lock().map_err(|_| {
            "workflow execution evidence persistence lock is unavailable".to_string()
        })?;
        let bytes = serde_json::to_vec_pretty(document).map_err(|error| {
            format!("workflow execution evidence state could not be serialized: {error}")
        })?;
        if bytes.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES {
            return Err(format!(
                "workflow execution evidence state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("workflow execution evidence state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "workflow_execution_evidence_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!(
                "workflow execution evidence state temporary file could not be written: {error}"
            )
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "workflow execution evidence state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "workflow execution evidence state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(bytes.len())
    }
}

impl WorkbenchPersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        let Some(_) = self.path.as_deref() else {
            return Ok(0);
        };
        let registry = self
            .registry
            .lock()
            .map_err(|_| "workbench registry is unavailable".to_string())?;
        let document = registry.snapshot().map_err(|error| error.to_string())?;
        self.persist_snapshot(&document)
    }

    pub(super) fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "workbench persistence lock is unavailable".to_string())?;
        let bytes = serde_json::to_vec_pretty(document)
            .map_err(|error| format!("workbench state could not be serialized: {error}"))?;
        if bytes.len() > MAX_WORKBENCH_REGISTRY_STATE_BYTES {
            return Err(format!(
                "workbench state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_WORKBENCH_REGISTRY_STATE_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("workbench state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "workbench_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!("workbench state temporary file could not be written: {error}")
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "workbench state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "workbench state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(bytes.len())
    }
}

impl CiProviderEvidencePersistence {
    pub(super) fn persist(&self) -> Result<usize, String> {
        let Some(_) = self.path.as_deref() else {
            return Ok(0);
        };
        let registry = self
            .registry
            .lock()
            .map_err(|_| "CI provider evidence registry is unavailable".to_string())?;
        let document = registry.snapshot().map_err(|error| error.to_string())?;
        self.persist_snapshot(&document)
    }

    pub(super) fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "CI provider evidence persistence lock is unavailable".to_string())?;
        let bytes = serde_json::to_vec_pretty(document).map_err(|error| {
            format!("CI provider evidence state could not be serialized: {error}")
        })?;
        if bytes.len() > MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES {
            return Err(format!(
                "CI provider evidence state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("CI provider evidence state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "ci_provider_evidence_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!("CI provider evidence state temporary file could not be written: {error}")
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "CI provider evidence state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "CI provider evidence state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(bytes.len())
    }
}

impl MissionPersistence {
    pub(super) fn persist(&self) -> Result<(), String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "mission persistence lock is unavailable".to_string())?;
        let mut missions = {
            let jobs = self
                .jobs
                .lock()
                .map_err(|_| "mission registry is unavailable".to_string())?;
            jobs.iter()
                .map(|(mission_id, job)| {
                    let state = job
                        .state
                        .lock()
                        .map_err(|_| "mission state is unavailable".to_string())?;
                    Ok(durable_mission_state_json(mission_id, &state))
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        trim_mission_snapshot_to_bound(&mut missions)?;
        let mut document = json!({
            "schema_version": MISSION_STATE_SCHEMA_VERSION,
            "missions": missions,
            "guarantees": [
                "terminal reports are restored only when their bounded JSON was retained",
                "queued and running jobs are marked failed after a process restart",
                "event cursors and webhook deliveries remain process-local"
            ]
        });
        let state_digest = mission_checkpoint_digest(&document)?;
        let Some(document_object) = document.as_object_mut() else {
            return Err("mission checkpoint document is not an object".into());
        };
        document_object.insert("state_digest".into(), Value::String(state_digest));
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|error| format!("mission state could not be serialized: {error}"))?;
        if bytes.len() > MAX_MISSION_STATE_FILE_BYTES {
            return Err(format!(
                "mission state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_MISSION_STATE_FILE_BYTES
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("mission state directory could not be created: {error}")
            })?;
        }
        let filename = path
            .file_name()
            .ok_or_else(|| "mission_state_path must name a file".to_string())?
            .to_string_lossy();
        let temporary = path.with_file_name(format!(
            ".{filename}.tmp-{}",
            NEXT_CHECKPOINT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&temporary, &bytes).map_err(|error| {
            format!("mission state temporary file could not be written: {error}")
        })?;
        if let Err(first_error) = std::fs::rename(&temporary, path) {
            #[cfg(windows)]
            {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&temporary, path).map_err(|second_error| {
                    format!(
                        "mission state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                    )
                })?;
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "mission state snapshot could not be installed: {first_error}"
                ));
            }
        }
        Ok(())
    }
}
