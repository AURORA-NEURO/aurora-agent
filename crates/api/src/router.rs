//! HTTP routes over the existing MCP server.
//!
//! The router intentionally delegates domain semantics to `bioprism-mcp::Server`.  The HTTP
//! layer owns transport concerns only: authentication, request bounds, route shape, cursors, and
//! the event/webhook outbox.  A REST call and an MCP `tools/call` therefore reach exactly the same
//! Rust implementation and produce the same evidence-bearing result.

use crate::events::{
    DeliveryRunReport, DeliverySender, EventLog, EventMetrics, EVENT_STATE_SCHEMA_VERSION,
    MAX_EVENT_STATE_FILE_BYTES, MAX_FILTERS,
};
use crate::http::{HttpRequest, HttpResponse};
use bioprism_devplat::{
    build_cross_domain_audit, plan_mission, verify_mission_evidence_bundle, ArtifactRegistry,
    ArtifactRegistryError, CiProviderEvidenceRegistry, DomainWorkflowReconciliationRegistry,
    EvidenceBundleError, EvidenceBundleRegistry, EvidenceRegistryError, MissionEvaluatorCatalogue,
    MissionEvaluatorReplayCompareRequest, MissionEvaluatorReplayRequest, MissionRequest,
    WorkbenchReportRegistry, WorkflowExecutionEvidenceRegistry, MAX_ARTIFACT_REGISTRY_BYTES,
    MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES, MAX_EVIDENCE_REGISTRY_BYTES,
    MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES,
};
use bioprism_factory::{
    AuthorityMutation, ExecutionOperation, Idempotency as FactoryIdempotency, Job as FactoryJob,
    JobStore, Lease as FactoryLease, QueueAdmissionPolicy, Recovery as FactoryRecovery,
    ResourceClass, SharedExecutionAuthority, WorkerCapability, EXECUTION_AUTHORITY_SCHEMA_VERSION,
    JOB_STORE_SNAPSHOT_SCHEMA_VERSION, MAX_EXECUTION_AUTHORITY_BYTES,
};
use bioprism_ids::ContentHash;
use bioprism_mcp::{Request, Response, PROTOCOL_VERSION, SERVER_NAME};
use bioprism_scope::Timestamp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const API_VERSION: &str = "v1";
pub const DEFAULT_MAX_HEADER_BYTES: usize = 32 * 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_EVENT_CAPACITY: usize = 4096;
pub const MAX_MISSION_JOBS: usize = 4096;
pub const MAX_MISSION_LIST_LIMIT: usize = 256;
pub const MAX_MISSION_TRACE_EVENTS: usize = 4096;
pub const MAX_OPERATIONS_SNAPSHOT_LIMIT: usize = 256;
pub const MAX_OPERATIONS_DOMAIN_GROUPS: usize = 64;
pub const MAX_OPERATIONS_DOMAIN_TOOLS: usize = 256;
pub const MISSION_STATE_SCHEMA_VERSION: u64 = 2;
const LEGACY_MISSION_STATE_SCHEMA_VERSION: u64 = 1;
pub const MAX_MISSION_STATE_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PERSISTED_MISSION_RESULT_BYTES: usize = 256 * 1024;
pub const MAX_PERSISTED_MISSION_TRACE_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_PERSISTED_MISSION_PROVENANCE_BYTES: usize = 128 * 1024;
pub const MAX_MISSION_EVIDENCE_BUNDLE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_WORKFLOW_RECONCILIATION_STATE_BYTES: usize =
    bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES;
pub const MAX_WORKBENCH_REGISTRY_STATE_BYTES: usize =
    bioprism_devplat::MAX_WORKBENCH_REGISTRY_BYTES;
pub const MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES: usize =
    MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES;
pub const MISSION_QUEUE_LEASE_DURATION_NANOS: i128 = 24 * 60 * 60 * 1_000_000_000;
const MISSION_QUEUE_WORKER_ID: &str = "bioprism-api-mission-worker";
static NEXT_CHECKPOINT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub event_capacity: usize,
    pub bearer_token: Option<String>,
    /// Optional atomic JSON checkpoint for the bounded asynchronous mission registry.
    ///
    /// Event cursors remain process-local; this path only restores mission status, bounded
    /// trace rows, progress, and size-bounded result metadata after an API restart.
    pub mission_state_path: Option<PathBuf>,
    /// Optional atomic checkpoint for the typed factory lifecycle behind mission execution.
    ///
    /// This is separate from `mission_state_path`: the mission projection answers what the API
    /// observed, while the queue checkpoint answers which lease/idempotency branch is recoverable.
    /// It never enables automatic resumption of an in-process worker.
    pub mission_queue_state_path: Option<PathBuf>,
    /// Maximum total checkpointed queue jobs admitted before backpressure is returned.
    pub mission_queue_max_jobs: usize,
    /// Maximum concurrent leased mission jobs in this API process.
    pub mission_queue_max_active_leases: usize,
    /// Optional atomic JSON checkpoint for the bounded event cursor, subscription metadata, and
    /// signed pending webhook outbox.
    pub event_state_path: Option<PathBuf>,
    /// Optional atomic JSON checkpoint for imported, independently verified evidence bundles.
    pub evidence_state_path: Option<PathBuf>,
    /// Optional atomic JSON checkpoint for imported domain-workflow reconciliation reports.
    pub reconciliation_state_path: Option<PathBuf>,
    /// Optional atomic JSON checkpoint for the bounded cross-domain artifact and lineage index.
    pub artifact_state_path: Option<PathBuf>,
    /// Optional atomic JSON checkpoint for independently validated workflow execution evidence.
    pub workflow_execution_evidence_state_path: Option<PathBuf>,
    /// Optional atomic JSON checkpoint for retained, structurally valid workbench reports.
    pub workbench_state_path: Option<PathBuf>,
    /// Optional atomic JSON checkpoint for retained, re-audited provider-shaped CI evidence.
    pub ci_provider_evidence_state_path: Option<PathBuf>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_header_bytes: DEFAULT_MAX_HEADER_BYTES,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            bearer_token: None,
            mission_state_path: None,
            mission_queue_state_path: None,
            mission_queue_max_jobs: MAX_MISSION_JOBS,
            mission_queue_max_active_leases: 64,
            event_state_path: None,
            evidence_state_path: None,
            reconciliation_state_path: None,
            artifact_state_path: None,
            workflow_execution_evidence_state_path: None,
            workbench_state_path: None,
            ci_provider_evidence_state_path: None,
        }
    }
}

impl ApiConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(1024..=1024 * 1024).contains(&self.max_header_bytes) {
            return Err("max_header_bytes must be between 1024 and 1048576".into());
        }
        if !(1024..=64 * 1024 * 1024).contains(&self.max_body_bytes) {
            return Err("max_body_bytes must be between 1024 and 67108864".into());
        }
        if self.event_capacity == 0 || self.event_capacity > 100_000 {
            return Err("event_capacity must be between 1 and 100000".into());
        }
        if let Some(token) = &self.bearer_token {
            if token.len() < 16 || token.len() > 4096 || token.bytes().any(|byte| byte <= 0x20) {
                return Err("bearer_token must contain 16..=4096 visible bytes".into());
            }
        }
        if self
            .mission_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("mission_state_path must not be empty".into());
        }
        if self
            .mission_queue_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("mission_queue_state_path must not be empty".into());
        }
        if self.mission_queue_max_jobs == 0 || self.mission_queue_max_jobs > MAX_MISSION_JOBS {
            return Err(format!(
                "mission_queue_max_jobs must be between 1 and {MAX_MISSION_JOBS}"
            ));
        }
        if self.mission_queue_max_active_leases == 0
            || self.mission_queue_max_active_leases > self.mission_queue_max_jobs
        {
            return Err(
                "mission_queue_max_active_leases must be between 1 and mission_queue_max_jobs"
                    .into(),
            );
        }
        if self
            .event_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("event_state_path must not be empty".into());
        }
        if self
            .evidence_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("evidence_state_path must not be empty".into());
        }
        if self
            .reconciliation_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("reconciliation_state_path must not be empty".into());
        }
        if self
            .artifact_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("artifact_state_path must not be empty".into());
        }
        if self
            .workflow_execution_evidence_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("workflow_execution_evidence_state_path must not be empty".into());
        }
        if self
            .workbench_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("workbench_state_path must not be empty".into());
        }
        if self
            .ci_provider_evidence_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("ci_provider_evidence_state_path must not be empty".into());
        }
        Ok(())
    }
}

pub struct ApiRouter {
    server: bioprism_mcp::Server,
    mission_executor: Arc<bioprism_mcp::Server>,
    config: ApiConfig,
    events: Arc<Mutex<EventLog>>,
    next_request_id: AtomicU64,
    mission_jobs: Arc<Mutex<BTreeMap<String, Arc<MissionJob>>>>,
    mission_persistence: Arc<MissionPersistence>,
    mission_queue_persistence: Arc<MissionQueuePersistence>,
    event_persistence: Arc<EventPersistence>,
    evidence_registry: Arc<Mutex<EvidenceBundleRegistry>>,
    evidence_persistence: Arc<EvidencePersistence>,
    reconciliation_registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
    reconciliation_persistence: Arc<ReconciliationPersistence>,
    artifact_registry: Arc<Mutex<ArtifactRegistry>>,
    artifact_persistence: Arc<ArtifactPersistence>,
    workflow_execution_evidence_registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
    workflow_execution_evidence_persistence: Arc<WorkflowExecutionEvidencePersistence>,
    workbench_registry: Arc<Mutex<WorkbenchReportRegistry>>,
    workbench_persistence: Arc<WorkbenchPersistence>,
    ci_provider_evidence_registry: Arc<Mutex<CiProviderEvidenceRegistry>>,
    ci_provider_evidence_persistence: Arc<CiProviderEvidencePersistence>,
}

struct MissionJob {
    cancellation: Arc<AtomicBool>,
    state: Arc<Mutex<MissionJobState>>,
}

struct MissionPersistence {
    path: Option<PathBuf>,
    jobs: Arc<Mutex<BTreeMap<String, Arc<MissionJob>>>>,
    lock: Mutex<()>,
}

struct MissionQueuePersistence {
    path: Option<PathBuf>,
    authority: Arc<SharedExecutionAuthority>,
    startup_recoveries: Vec<FactoryRecovery>,
    admission_policy: QueueAdmissionPolicy,
}

struct EventPersistence {
    path: Option<PathBuf>,
    events: Arc<Mutex<EventLog>>,
    lock: Mutex<()>,
}

struct EvidencePersistence {
    path: Option<PathBuf>,
    registry: Arc<Mutex<EvidenceBundleRegistry>>,
    lock: Mutex<()>,
}

struct ReconciliationPersistence {
    path: Option<PathBuf>,
    registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
    lock: Mutex<()>,
}

struct ArtifactPersistence {
    path: Option<PathBuf>,
    registry: Arc<Mutex<ArtifactRegistry>>,
    lock: Mutex<()>,
}

struct WorkflowExecutionEvidencePersistence {
    path: Option<PathBuf>,
    registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
    lock: Mutex<()>,
}

struct WorkbenchPersistence {
    path: Option<PathBuf>,
    registry: Arc<Mutex<WorkbenchReportRegistry>>,
    lock: Mutex<()>,
}

struct CiProviderEvidencePersistence {
    path: Option<PathBuf>,
    registry: Arc<Mutex<CiProviderEvidenceRegistry>>,
    lock: Mutex<()>,
}

impl MissionQueuePersistence {
    fn new(path: Option<PathBuf>, admission_policy: QueueAdmissionPolicy) -> Result<Self, String> {
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

    fn persist(&self) -> Result<usize, String> {
        self.authority
            .flush()
            .map_err(|error| format!("mission queue authority checkpoint failed: {error}"))
    }

    /// Apply a queue transition through the shared authority. The queue image and journal row are
    /// committed together, and a second process reloads the latest image while holding the file
    /// lock instead of mutating a stale in-memory copy.
    fn mutate<T, F>(&self, mutation: AuthorityMutation, transition: F) -> Result<T, String>
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

    fn enqueue_and_lease(&self, job: FactoryJob, now: Timestamp) -> Result<FactoryLease, String> {
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

    fn heartbeat(&self, mission_id: &str, attempt: u32, now: Timestamp) -> Result<(), String> {
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

    fn commit_success(
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

    fn record_failure(
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

    fn cancel(&self, mission_id: &str, reason: &str) -> Result<(), String> {
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

    fn status(&self) -> Result<Value, String> {
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

    fn projection(&self, mission_id: &str) -> Result<Value, String> {
        Ok(self
            .authority
            .job(mission_id)
            .map_err(|error| format!("mission authority projection failed: {error}"))?
            .as_ref()
            .map(mission_queue_job_json)
            .unwrap_or(Value::Null))
    }

    fn release_orphaned_lock(
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
    fn persist(&self) -> Result<usize, String> {
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
    fn persist(&self) -> Result<usize, String> {
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

    fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "evidence persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, document)
    }

    fn write_snapshot(path: &Path, document: &Value) -> Result<usize, String> {
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
    fn persist(&self) -> Result<usize, String> {
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

    fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "workflow reconciliation persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, document)
    }

    fn write_snapshot(path: &Path, document: &Value) -> Result<usize, String> {
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
    fn persist(&self) -> Result<usize, String> {
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

    fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(0);
        };
        let _write_guard = self
            .lock
            .lock()
            .map_err(|_| "artifact registry persistence lock is unavailable".to_string())?;
        Self::write_snapshot(path, document)
    }

    fn write_snapshot(path: &Path, document: &Value) -> Result<usize, String> {
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
    fn persist(&self) -> Result<usize, String> {
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

    fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
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
    fn persist(&self) -> Result<usize, String> {
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

    fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
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
    fn persist(&self) -> Result<usize, String> {
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

    fn persist_snapshot(&self, document: &Value) -> Result<usize, String> {
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
    fn persist(&self) -> Result<(), String> {
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
        document
            .as_object_mut()
            .expect("mission checkpoint document is an object")
            .insert("state_digest".into(), Value::String(state_digest));
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

#[derive(Clone)]
struct MissionJobState {
    total_steps: usize,
    trace: Vec<Value>,
    progress: MissionProgressState,
    status: String,
    cancel_requested: bool,
    cancel_reason: Option<String>,
    result: Option<Value>,
    result_omitted: Option<Value>,
    evaluator_replay_summary: Option<Value>,
    route_review_provenance: Option<Value>,
    error: Option<String>,
    recovered_after_restart: bool,
    execution_provenance: Option<Value>,
}

#[derive(Clone)]
struct MissionProgressState {
    phase: String,
    current_wave: Option<usize>,
    total_steps: usize,
    completed_steps: usize,
    active_steps: usize,
    succeeded: usize,
    refused: usize,
    blocked: usize,
    cancelled: usize,
    required_failures: usize,
    returned_bytes: usize,
    trace_sequence: Option<usize>,
    last_event: Option<String>,
}

impl MissionProgressState {
    fn new(total_steps: usize) -> Self {
        Self {
            phase: "queued".into(),
            current_wave: None,
            total_steps,
            completed_steps: 0,
            active_steps: 0,
            succeeded: 0,
            refused: 0,
            blocked: 0,
            cancelled: 0,
            required_failures: 0,
            returned_bytes: 0,
            trace_sequence: None,
            last_event: None,
        }
    }

    fn observe_trace(&mut self, event: &Value) {
        self.trace_sequence = event
            .get("sequence")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        self.last_event = event
            .get("event")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(wave) = event
            .get("wave")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        {
            self.current_wave = Some(wave);
        }
        match event.get("event").and_then(Value::as_str) {
            Some("mission.started") => self.phase = "running".into(),
            Some("wave.started") => self.phase = "running".into(),
            Some("step.started") => self.active_steps = self.active_steps.saturating_add(1),
            Some("step.completed") => {
                self.active_steps = self.active_steps.saturating_sub(1);
                self.completed_steps = self.completed_steps.saturating_add(1);
                self.succeeded = self.succeeded.saturating_add(1);
            }
            Some("step.refused") => {
                self.active_steps = self.active_steps.saturating_sub(1);
                self.completed_steps = self.completed_steps.saturating_add(1);
                self.refused = self.refused.saturating_add(1);
            }
            Some("step.blocked") => {
                self.completed_steps = self.completed_steps.saturating_add(1);
                self.blocked = self.blocked.saturating_add(1);
            }
            Some("step.cancelled") => {
                self.completed_steps = self.completed_steps.saturating_add(1);
                self.cancelled = self.cancelled.saturating_add(1);
            }
            Some("mission.cancelled") => self.phase = "cancelled".into(),
            Some("mission.completed") => {
                if let Some(status) = event.get("status").and_then(Value::as_str) {
                    self.phase = status.to_string();
                }
            }
            _ => {}
        }
    }

    fn request_cancel(&mut self) {
        if !is_terminal_mission_status(&self.phase) {
            self.phase = "cancellation_requested".into();
        }
    }

    fn reconcile(&mut self, report: &Value) {
        self.phase = report
            .get("mission_status")
            .and_then(Value::as_str)
            .unwrap_or("failed")
            .into();
        self.total_steps = report
            .pointer("/plan/ordered_steps")
            .and_then(Value::as_array)
            .map_or(self.total_steps, Vec::len);
        self.completed_steps = report
            .get("results")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        self.active_steps = 0;
        self.succeeded = progress_count(report, "succeeded");
        self.refused = progress_count(report, "refused");
        self.blocked = progress_count(report, "blocked");
        self.cancelled = progress_count(report, "cancelled");
        self.required_failures = progress_count(report, "required_failures");
        self.returned_bytes = progress_count(report, "returned_bytes");
        if let Some(event) = report
            .get("execution_trace")
            .and_then(Value::as_array)
            .and_then(|events| events.last())
        {
            self.observe_trace(event);
            self.phase = report
                .get("mission_status")
                .and_then(Value::as_str)
                .unwrap_or("failed")
                .into();
        }
    }
}

impl MissionJobState {
    fn record_trace(&mut self, event: Value) {
        self.progress.observe_trace(&event);
        if self.trace.len() >= MAX_MISSION_TRACE_EVENTS {
            self.trace.remove(0);
        }
        self.trace.push(event);
    }
}

impl ApiRouter {
    pub fn new(root: PathBuf, config: ApiConfig) -> Result<Self, String> {
        config.validate()?;
        let mission_queue_policy = QueueAdmissionPolicy::new(
            config.mission_queue_max_jobs,
            config.mission_queue_max_active_leases,
        )
        .with_resource_class_limit(
            ResourceClass::Evaluate,
            config.mission_queue_max_jobs,
            config.mission_queue_max_active_leases,
        );
        let mission_queue_persistence = Arc::new(MissionQueuePersistence::new(
            config.mission_queue_state_path.clone(),
            mission_queue_policy,
        )?);
        let events = Arc::new(Mutex::new(EventLog::from_checkpoint_path(
            config.event_capacity,
            config.event_state_path.as_deref(),
        )?));
        let restored_jobs = load_mission_jobs(config.mission_state_path.as_deref())?;
        let restored_evidence = load_evidence_registry(config.evidence_state_path.as_deref())?;
        let restored_reconciliations =
            load_workflow_reconciliation_registry(config.reconciliation_state_path.as_deref())?;
        let restored_artifacts = load_artifact_registry(config.artifact_state_path.as_deref())?;
        let restored_workflow_execution_evidence = load_workflow_execution_evidence_registry(
            config.workflow_execution_evidence_state_path.as_deref(),
        )?;
        let restored_workbench = load_workbench_registry(config.workbench_state_path.as_deref())?;
        let restored_ci_provider_evidence =
            load_ci_provider_evidence_registry(config.ci_provider_evidence_state_path.as_deref())?;
        let evidence_registry = Arc::new(Mutex::new(restored_evidence));
        let reconciliation_registry = Arc::new(Mutex::new(restored_reconciliations));
        let artifact_registry = Arc::new(Mutex::new(restored_artifacts));
        let workflow_execution_evidence_registry =
            Arc::new(Mutex::new(restored_workflow_execution_evidence));
        let workbench_registry = Arc::new(Mutex::new(restored_workbench));
        let ci_provider_evidence_registry = Arc::new(Mutex::new(restored_ci_provider_evidence));
        let mut server = bioprism_mcp::Server::with_all_registries_and_ci_provider_evidence(
            root,
            Arc::clone(&evidence_registry),
            Arc::clone(&reconciliation_registry),
            Arc::clone(&workflow_execution_evidence_registry),
            Arc::clone(&workbench_registry),
            Arc::clone(&ci_provider_evidence_registry),
            Arc::clone(&artifact_registry),
        );
        let initialize = Request {
            id: Some(json!(0)),
            method: "initialize".into(),
            params: json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "bioprism-api", "version": env!("CARGO_PKG_VERSION") }
            }),
        };
        server
            .handle(&initialize)
            .ok_or_else(|| "API server initialization produced no response".to_string())?;
        let initialized = Request {
            id: None,
            method: "notifications/initialized".into(),
            params: Value::Null,
        };
        server.handle(&initialized);
        let mission_executor = Arc::new(server.clone());
        let mission_jobs = Arc::new(Mutex::new(restored_jobs));
        let mission_persistence = Arc::new(MissionPersistence {
            path: config.mission_state_path.clone(),
            jobs: Arc::clone(&mission_jobs),
            lock: Mutex::new(()),
        });
        let event_persistence = Arc::new(EventPersistence {
            path: config.event_state_path.clone(),
            events: Arc::clone(&events),
            lock: Mutex::new(()),
        });
        let evidence_persistence = Arc::new(EvidencePersistence {
            path: config.evidence_state_path.clone(),
            registry: Arc::clone(&evidence_registry),
            lock: Mutex::new(()),
        });
        let reconciliation_persistence = Arc::new(ReconciliationPersistence {
            path: config.reconciliation_state_path.clone(),
            registry: Arc::clone(&reconciliation_registry),
            lock: Mutex::new(()),
        });
        let artifact_persistence = Arc::new(ArtifactPersistence {
            path: config.artifact_state_path.clone(),
            registry: Arc::clone(&artifact_registry),
            lock: Mutex::new(()),
        });
        let workflow_execution_evidence_persistence =
            Arc::new(WorkflowExecutionEvidencePersistence {
                path: config.workflow_execution_evidence_state_path.clone(),
                registry: Arc::clone(&workflow_execution_evidence_registry),
                lock: Mutex::new(()),
            });
        let workbench_persistence = Arc::new(WorkbenchPersistence {
            path: config.workbench_state_path.clone(),
            registry: Arc::clone(&workbench_registry),
            lock: Mutex::new(()),
        });
        let ci_provider_evidence_persistence = Arc::new(CiProviderEvidencePersistence {
            path: config.ci_provider_evidence_state_path.clone(),
            registry: Arc::clone(&ci_provider_evidence_registry),
            lock: Mutex::new(()),
        });
        let router = Self {
            server,
            mission_executor,
            config,
            events,
            next_request_id: AtomicU64::new(1),
            mission_jobs,
            mission_persistence,
            mission_queue_persistence,
            event_persistence,
            evidence_registry,
            evidence_persistence,
            reconciliation_registry,
            reconciliation_persistence,
            artifact_registry,
            artifact_persistence,
            workflow_execution_evidence_registry,
            workflow_execution_evidence_persistence,
            workbench_registry,
            workbench_persistence,
            ci_provider_evidence_registry,
            ci_provider_evidence_persistence,
        };
        if router.config.mission_state_path.is_some() {
            router.persist_mission_registry()?;
        }
        if router.config.event_state_path.is_some() {
            router.event_persistence.persist().map_err(|error| {
                format!("event state checkpoint failed during startup: {error}")
            })?;
        }
        if router.config.evidence_state_path.is_some() {
            router.evidence_persistence.persist().map_err(|error| {
                format!("evidence state checkpoint failed during startup: {error}")
            })?;
        }
        if router.config.reconciliation_state_path.is_some() {
            router
                .reconciliation_persistence
                .persist()
                .map_err(|error| {
                    format!(
                        "workflow reconciliation state checkpoint failed during startup: {error}"
                    )
                })?;
        }
        if router.config.artifact_state_path.is_some() {
            router.artifact_persistence.persist().map_err(|error| {
                format!("artifact registry state checkpoint failed during startup: {error}")
            })?;
        }
        if router
            .config
            .workflow_execution_evidence_state_path
            .is_some()
        {
            router.persist_workflow_execution_evidence_registry().map_err(|error| {
                    format!(
                        "workflow execution evidence state checkpoint failed during startup: {error}"
                    )
            })?;
        }
        if router.config.workbench_state_path.is_some() {
            router.workbench_persistence.persist().map_err(|error| {
                format!("workbench state checkpoint failed during startup: {error}")
            })?;
        }
        if router.config.ci_provider_evidence_state_path.is_some() {
            router
                .ci_provider_evidence_persistence
                .persist()
                .map_err(|error| {
                    format!("CI provider evidence state checkpoint failed during startup: {error}")
                })?;
        }
        Ok(router)
    }

    fn persist_mission_registry(&self) -> Result<(), String> {
        self.mission_persistence.persist()
    }

    fn persist_mission_queue(&self) -> Result<usize, String> {
        self.mission_queue_persistence.persist()
    }

    fn persist_evidence_registry(&self) -> Result<usize, String> {
        self.evidence_persistence.persist()
    }

    fn persist_reconciliation_registry(&self) -> Result<usize, String> {
        self.reconciliation_persistence.persist()
    }

    fn persist_artifact_registry(&self) -> Result<usize, String> {
        self.artifact_persistence.persist()
    }

    fn persist_workflow_execution_evidence_registry(&self) -> Result<usize, String> {
        self.workflow_execution_evidence_persistence.persist()
    }

    fn persist_workbench_registry(&self) -> Result<usize, String> {
        self.workbench_persistence.persist()
    }

    fn persist_ci_provider_evidence_registry(&self) -> Result<usize, String> {
        self.ci_provider_evidence_persistence.persist()
    }

    pub fn handle(&self, request: HttpRequest) -> HttpResponse {
        let request_id = self.request_id(&request);
        if request.body.len() > self.config.max_body_bytes {
            return self.finish(
                self.error(
                    413,
                    "body_too_large",
                    "request body exceeds the configured bound",
                    &request_id,
                ),
                &request_id,
            );
        }
        let public = request.path() == "/healthz"
            || request.path() == "/readyz"
            || request.path() == "/openapi.json"
            || request.path() == "/v1/openapi.json";
        if request.method == "OPTIONS" {
            return self.finish(
                HttpResponse::empty(204)
                    .with_header("access-control-allow-origin", "*")
                    .with_header("access-control-allow-methods", "GET, POST, DELETE, OPTIONS")
                    .with_header(
                        "access-control-allow-headers",
                        "authorization, content-type, x-request-id",
                    ),
                &request_id,
            );
        }
        if !public && !self.authorized(&request) {
            return self.finish(
                self.error(
                    401,
                    "unauthorized",
                    "a valid bearer token is required",
                    &request_id,
                ),
                &request_id,
            );
        }

        let response = match (request.method.as_str(), request.path()) {
            ("GET", "/healthz") => self.health(false),
            ("GET", "/readyz") => self.health(true),
            ("GET", "/openapi.json") | ("GET", "/v1/openapi.json") => self.openapi(),
            ("GET", "/v1") => self.index(),
            ("GET", "/v1/capabilities") => self.capabilities(),
            ("GET", "/v1/capabilities/dashboard") => {
                self.capability_dashboard(&request, &request_id)
            }
            ("POST", "/v1/capabilities/route") => self.capability_route(&request, &request_id),
            ("POST", "/v1/capabilities/route/review") => {
                self.capability_route_review(&request, &request_id)
            }
            ("POST", "/v1/capabilities/route/plan") => {
                self.capability_route_plan(&request, &request_id)
            }
            ("POST", "/v1/capabilities/route/plan/verify") => {
                self.capability_route_plan_verify(&request, &request_id)
            }
            ("GET", "/v1/recovery") => self.recovery_matrix(),
            ("GET", "/v1/operations/snapshot") => self.operations_snapshot(&request, &request_id),
            ("GET", "/v1/operations/domains") => {
                self.operations_domain_activity(&request, &request_id)
            }
            ("GET", "/v1/operations/gates") => self.operations_domain_gates(&request, &request_id),
            ("GET", "/v1/operations/gate-reviews") => {
                self.operations_gate_reviews(&request, &request_id)
            }
            ("POST", "/v1/operations/gate-reviews") => {
                self.create_operations_gate_review(&request, &request_id)
            }
            ("POST", "/v1/operations/handoff") => self.operations_handoff(&request, &request_id),
            ("GET", "/v1/domain-workflows") => self.domain_workflow_catalogue(&request_id),
            ("POST", "/v1/domain-workflows/reconcile") => {
                self.domain_workflow_reconcile(&request, &request_id)
            }
            ("GET", "/v1/domain-workflows/reconciliations") => {
                self.query_workflow_reconciliations(&request, &request_id)
            }
            ("POST", "/v1/domain-workflows/reconciliations") => {
                self.import_workflow_reconciliation(&request, &request_id)
            }
            ("GET", "/v1/domain-workflows/reconciliations/persistence") => {
                self.reconciliation_persistence_status()
            }
            ("POST", "/v1/domain-workflows/reconciliations/persistence/flush") => {
                self.flush_reconciliation_persistence(&request_id)
            }
            ("GET", path) if path.starts_with("/v1/domain-workflows/reconciliations/") => {
                self.get_workflow_reconciliation(&request, &request_id)
            }
            ("POST", "/v1/domain-workflows/instantiate") => {
                self.domain_workflow_instantiate(&request, &request_id)
            }
            ("POST", "/v1/domain-workflows/portfolio") => {
                self.domain_workflow_portfolio(&request, &request_id)
            }
            ("POST", "/v1/domain-workflows/portfolio/verify") => {
                self.domain_workflow_portfolio_verify(&request, &request_id)
            }
            ("POST", "/v1/developer-workbench/verify") => {
                self.developer_workbench_verify(&request, &request_id)
            }
            ("POST", "/v1/developer-workbench/reports") => {
                self.import_workbench_report(&request, &request_id)
            }
            ("GET", "/v1/developer-workbench/reports") => {
                self.query_workbench_reports(&request, &request_id)
            }
            ("GET", "/v1/developer-workbench/reports/persistence") => {
                self.workbench_persistence_status()
            }
            ("POST", "/v1/developer-workbench/reports/persistence/flush") => {
                self.flush_workbench_persistence(&request_id)
            }
            ("GET", path) if path.starts_with("/v1/developer-workbench/reports/") => {
                self.get_workbench_report(&request, &request_id)
            }
            ("POST", "/v1/ci/provider-evidence") => {
                self.import_ci_provider_evidence(&request, &request_id)
            }
            ("GET", "/v1/ci/provider-evidence") => {
                self.query_ci_provider_evidence(&request, &request_id)
            }
            ("GET", "/v1/ci/provider-evidence/persistence") => {
                self.ci_provider_evidence_persistence_status()
            }
            ("POST", "/v1/ci/provider-evidence/persistence/flush") => {
                self.flush_ci_provider_evidence_persistence(&request_id)
            }
            ("GET", path) if path.starts_with("/v1/ci/provider-evidence/") => {
                self.get_ci_provider_evidence(&request, &request_id)
            }
            ("POST", "/v1/domain-workflows/verify") => {
                self.domain_workflow_verify(&request, &request_id)
            }
            ("POST", "/v1/domain-workflows/scaffold") => {
                self.domain_workflow_scaffold(&request, &request_id)
            }
            ("POST", "/v1/evidence-bundles/verify") => {
                self.verify_evidence_bundle(&request, &request_id)
            }
            ("GET", "/v1/evidence-bundles") => self.query_evidence_bundles(&request, &request_id),
            ("POST", "/v1/evidence-bundles") => self.import_evidence_bundle(&request, &request_id),
            ("GET", "/v1/evidence-bundles/persistence") => self.evidence_persistence_status(),
            ("POST", "/v1/evidence-bundles/persistence/flush") => {
                self.flush_evidence_persistence(&request_id)
            }
            ("GET", path) if path.starts_with("/v1/evidence-bundles/") => {
                self.get_evidence_bundle(&request, &request_id)
            }
            ("GET", "/v1/artifacts/cross-store") => self.cross_store_artifact_audit(&request_id),
            ("GET", "/v1/domain-reports/coverage") => {
                self.domain_report_coverage(&request, &request_id)
            }
            ("POST", "/v1/domain-reports") => self.domain_report_project(&request, &request_id),
            ("POST", "/v1/domain-evidence/harmonize") => {
                self.domain_evidence_harmonize(&request, &request_id)
            }
            ("GET", "/v1/domain-evidence/harmonization/coverage") => {
                self.domain_evidence_harmonization_coverage(&request, &request_id)
            }
            ("GET", "/v1/domain-evidence/lineage") => {
                self.domain_evidence_lineage(&request, &request_id)
            }
            ("POST", "/v1/domain-evidence/intake") => {
                self.domain_evidence_intake(&request, &request_id)
            }
            ("POST", "/v1/domain-evidence/sources") => {
                self.domain_evidence_source_plan(&request, &request_id)
            }
            ("POST", "/v1/domain-evidence/sources/execute") => {
                self.domain_evidence_source_execute(&request, &request_id)
            }
            ("GET", "/v1/domain-evidence/coverage") => {
                self.domain_evidence_coverage(&request, &request_id)
            }
            ("GET", "/v1/artifacts") => self.query_artifacts(&request, &request_id),
            ("POST", "/v1/artifacts") => self.register_artifact(&request, &request_id),
            ("GET", "/v1/domain-decision-readiness") => {
                self.query_domain_decision_readiness(&request, &request_id)
            }
            ("POST", "/v1/control-plane-readiness") => {
                self.control_plane_readiness_audit(&request, &request_id)
            }
            ("POST", "/v1/control-plane-readiness/compare") => {
                self.control_plane_readiness_compare(&request, &request_id)
            }
            ("POST", "/v1/control-plane-readiness/compare-retained") => {
                self.control_plane_readiness_compare_retained(&request, &request_id)
            }
            ("GET", "/v1/control-plane-readiness") => {
                self.query_control_plane_readiness(&request, &request_id)
            }
            ("GET", "/v1/artifacts/persistence") => self.artifact_persistence_status(),
            ("POST", "/v1/artifacts/persistence/flush") => {
                self.flush_artifact_persistence(&request_id)
            }
            ("GET", path) if path.ends_with("/lineage") && path.starts_with("/v1/artifacts/") => {
                self.artifact_lineage(&request, &request_id)
            }
            ("GET", path) if path.starts_with("/v1/artifacts/") => {
                self.get_artifact(&request, &request_id)
            }
            ("GET", "/v1/tools") => self.tools(),
            ("GET", "/v1/metrics") => self.metrics(),
            ("GET", "/v1/events") => self.events(&request),
            ("GET", "/v1/events/stream") => self.event_stream(&request),
            ("GET", path)
                if path.starts_with("/v1/delivery-receipts/") && path.ends_with("/attempts") =>
            {
                self.delivery_receipt_attempts(&request, &request_id)
            }
            ("GET", path) if path.starts_with("/v1/delivery-receipts/") => {
                self.delivery_receipt_events(&request, &request_id)
            }
            ("GET", path) if path.starts_with("/v1/route-reviews/") => {
                self.route_review_evidence(&request, &request_id)
            }
            ("GET", "/v1/events/persistence") => self.event_persistence_status(),
            ("POST", "/v1/events/persistence/flush") => self.flush_event_persistence(&request_id),
            ("GET", "/v1/missions") => self.mission_inventory(&request, &request_id),
            ("GET", "/v1/missions/persistence") => self.mission_persistence_status(),
            ("POST", "/v1/missions/persistence/flush") => {
                self.flush_mission_persistence(&request_id)
            }
            ("GET", "/v1/missions/queue") => self.mission_queue_inventory(&request_id),
            ("GET", "/v1/missions/queue/persistence") => self.mission_queue_persistence_status(),
            ("POST", "/v1/missions/queue/persistence/flush") => {
                self.flush_mission_queue_persistence(&request_id)
            }
            ("POST", "/v1/missions/queue/authority/release-lock") => {
                self.release_mission_queue_lock(&request, &request_id)
            }
            ("POST", "/v1/missions/preflight") => self.preflight_mission(&request, &request_id),
            ("POST", "/v1/missions") => self.submit_mission(&request, &request_id),
            ("GET", path) if path.starts_with("/v1/missions/") && path.ends_with("/provenance") => {
                self.mission_provenance(&request, &request_id)
            }
            ("GET", path)
                if path.starts_with("/v1/missions/") && path.ends_with("/evidence-bundle") =>
            {
                self.mission_evidence_bundle(&request, &request_id)
            }
            ("GET", path)
                if path.starts_with("/v1/missions/")
                    && path.ends_with("/evaluator-replay/compare") =>
            {
                self.mission_evaluator_replay_compare(&request, &request_id)
            }
            ("GET", path)
                if path.starts_with("/v1/missions/") && path.ends_with("/evaluator-replay") =>
            {
                self.mission_evaluator_replay(&request, &request_id)
            }
            ("GET", path) if path.starts_with("/v1/missions/") && path.ends_with("/claims") => {
                self.mission_claims(&request, &request_id)
            }
            ("GET", path) if path.starts_with("/v1/missions/") && path.ends_with("/trace") => {
                self.mission_trace(&request, &request_id)
            }
            ("GET", path) if path.starts_with("/v1/missions/") => {
                self.mission_status(&request, &request_id)
            }
            ("POST", path) if path.starts_with("/v1/missions/") => {
                self.mission_control(&request, &request_id)
            }
            ("DELETE", path) if path.starts_with("/v1/missions/") => {
                self.delete_mission(&request, &request_id)
            }
            ("POST", "/v1/rpc") => self.rpc(&request, &request_id),
            ("POST", path) if path.starts_with("/v1/tools/") => {
                self.rest_tool(&request, &request_id)
            }
            ("GET", "/v1/webhooks/subscriptions") => self.list_subscriptions(),
            ("POST", "/v1/webhooks/subscriptions") => {
                self.create_subscription(&request, &request_id)
            }
            ("DELETE", path) if path.starts_with("/v1/webhooks/subscriptions/") => {
                self.delete_subscription(&request, &request_id)
            }
            ("POST", path) if path.ends_with("/rebind") => {
                self.rebind_subscription(&request, &request_id)
            }
            ("GET", path) if path.ends_with("/deliveries") => {
                self.list_deliveries(&request, &request_id)
            }
            ("GET", path) if path.ends_with("/attempts") => {
                self.list_delivery_attempts(&request, &request_id)
            }
            ("POST", path) if path.ends_with("/ack") => self.ack_deliveries(&request, &request_id),
            ("POST", path) if path.ends_with("/retry") => {
                self.retry_deliveries(&request, &request_id)
            }
            ("POST", path) if path.ends_with("/replay") => {
                self.replay_deliveries(&request, &request_id)
            }
            _ => self.error(404, "not_found", "route does not exist", &request_id),
        };
        self.finish(response, &request_id)
    }

    pub fn event_metrics(&self) -> crate::events::EventMetrics {
        self.events
            .lock()
            .map(|events| events.metrics())
            .unwrap_or_else(|_| unavailable_event_metrics())
    }

    /// Run one bounded webhook delivery cycle using an operator-owned transport.
    pub fn deliver_once<S: DeliverySender>(
        &self,
        sender: &mut S,
        max_batch: usize,
    ) -> Result<DeliveryRunReport, String> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| "event log is unavailable".to_string())?;
        let report = events.deliver_once(sender, max_batch)?;
        drop(events);
        let _ = self.event_persistence.persist();
        Ok(report)
    }

    pub fn limits(&self) -> (usize, usize) {
        (self.config.max_header_bytes, self.config.max_body_bytes)
    }

    fn mission_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.mission_state_path.is_some();
        let file_bytes = self
            .config
            .mission_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let state_digest = checkpoint_digest_from_path(self.config.mission_state_path.as_deref());
        let integrity_verified = checkpoint_integrity_from_path(
            self.config.mission_state_path.as_deref(),
            MISSION_STATE_SCHEMA_VERSION,
        );
        let registry_size = self.mission_jobs.lock().map(|jobs| jobs.len()).unwrap_or(0);
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema_version": MISSION_STATE_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "max_file_bytes": MAX_MISSION_STATE_FILE_BYTES,
                "max_result_bytes": MAX_PERSISTED_MISSION_RESULT_BYTES,
                "max_provenance_bytes": MAX_PERSISTED_MISSION_PROVENANCE_BYTES,
                "registry_size": registry_size,
                "event_log_durable": false,
                "webhook_deliveries_durable": false,
                "recovery_policy": "terminal snapshots restore; queued and running jobs fail explicitly after restart",
                "flush": "/v1/missions/persistence/flush"
            }),
        )
    }

    fn mission_queue_persistence_status(&self) -> HttpResponse {
        match self.mission_queue_persistence.status() {
            Ok(status) => HttpResponse::json(200, &status),
            Err(error) => HttpResponse::json(
                500,
                &json!({
                    "ok": false,
                    "error": "mission_queue_unavailable",
                    "detail": error
                }),
            ),
        }
    }

    fn mission_queue_inventory(&self, request_id: &str) -> HttpResponse {
        match self.mission_queue_persistence.status() {
            Ok(status) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-mission-queue/0.1",
                    "queue": status,
                    "guarantees": [
                        "queue state is projected from the typed factory lifecycle",
                        "job specifications remain checkpointed but are not returned in this inventory",
                        "expired leases are classified explicitly rather than silently dropped",
                        "a queued recovery record is not evidence that a worker has resumed"
                    ],
                    "links": {
                        "persistence": "/v1/missions/queue/persistence",
                        "flush": "/v1/missions/queue/persistence/flush",
                        "release_lock": "/v1/missions/queue/authority/release-lock",
                        "mission_inventory": "/v1/missions"
                    }
                }),
            ),
            Err(error) => self.error(500, "mission_queue_unavailable", &error, request_id),
        }
    }

    fn flush_mission_queue_persistence(&self, request_id: &str) -> HttpResponse {
        match self.persist_mission_queue() {
            Ok(bytes) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "bytes": bytes,
                    "queue": self.mission_queue_persistence.status().unwrap_or_else(|error| json!({"ok": false, "error": error})),
                    "request_id": request_id,
                    "guarantees": [
                        "the checkpoint is content-addressed and atomically replaced",
                        "a successful flush does not claim external effect completion"
                    ]
                }),
            ),
            Err(error) => self.error(
                503,
                "mission_queue_persistence_unavailable",
                &error,
                request_id,
            ),
        }
    }

    fn release_mission_queue_lock(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let operator = match arguments.get("operator").and_then(Value::as_str) {
            Some(operator) if !operator.trim().is_empty() => operator,
            _ => {
                return self.error(
                    422,
                    "invalid_authority_operator",
                    "operator must be a non-empty string",
                    request_id,
                )
            }
        };
        let reason = match arguments.get("reason").and_then(Value::as_str) {
            Some(reason) if !reason.trim().is_empty() => reason,
            _ => {
                return self.error(
                    422,
                    "invalid_authority_release_reason",
                    "reason must be a non-empty string",
                    request_id,
                )
            }
        };
        let at = match current_timestamp() {
            Ok(at) => at,
            Err(error) => {
                return self.error(500, "authority_clock_unavailable", &error, request_id)
            }
        };
        match self
            .mission_queue_persistence
            .release_orphaned_lock(operator, reason, at)
        {
            Ok(receipt) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "receipt": receipt,
                    "request_id": request_id,
                    "warning": "release is an explicit operator override; confirm the previous process cannot still mutate the shared authority"
                }),
            ),
            Err(error) => self.error(
                409,
                "mission_queue_authority_lock_release_refused",
                &error,
                request_id,
            ),
        }
    }

    fn event_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.event_state_path.is_some();
        let file_bytes = self
            .config
            .event_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let metrics = self.event_metrics();
        let state_digest = checkpoint_digest_from_path(self.config.event_state_path.as_deref());
        let integrity_verified = checkpoint_integrity_from_path(
            self.config.event_state_path.as_deref(),
            EVENT_STATE_SCHEMA_VERSION,
        );
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema_version": EVENT_STATE_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "max_file_bytes": MAX_EVENT_STATE_FILE_BYTES,
                "retained_events": metrics.retained_events,
                "next_event_id": metrics.next_event_id,
                "dropped_events": metrics.dropped_events,
                "retained_delivery_attempts": metrics.retained_delivery_attempts,
                "dropped_delivery_attempts": metrics.dropped_delivery_attempts,
                "next_attempt_id": metrics.next_attempt_id,
                "subscriptions_durable": true,
                "webhook_deliveries_durable": true,
                "delivery_attempts_durable": true,
                "delivery_receipt_metadata_durable": true,
                "secrets_persisted": false,
                "recovery_policy": "events, subscription metadata, and signed outbox rows restore; subscriptions pause until explicit in-memory secret rebind",
                "flush": "/v1/events/persistence/flush"
            }),
        )
    }

    fn recovery_matrix(&self) -> HttpResponse {
        let mission_enabled = self.config.mission_state_path.is_some();
        let mission_checkpoint_present = self
            .config
            .mission_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .is_some();
        let event_enabled = self.config.event_state_path.is_some();
        let event_checkpoint_present = self
            .config
            .event_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .is_some();
        let mission_state_digest =
            checkpoint_digest_from_path(self.config.mission_state_path.as_deref());
        let mission_integrity_verified = checkpoint_integrity_from_path(
            self.config.mission_state_path.as_deref(),
            MISSION_STATE_SCHEMA_VERSION,
        );
        let mission_queue_persistence = response_value(self.mission_queue_persistence_status());
        let mission_queue_enabled = mission_queue_persistence
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mission_queue_checkpoint_present = mission_queue_persistence
            .get("file_present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let metrics = self.event_metrics();
        let state_digest = checkpoint_digest_from_path(self.config.event_state_path.as_deref());
        let event_integrity_verified = checkpoint_integrity_from_path(
            self.config.event_state_path.as_deref(),
            EVENT_STATE_SCHEMA_VERSION,
        );
        let evidence_persistence = response_value(self.evidence_persistence_status());
        let evidence_enabled = evidence_persistence
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let evidence_checkpoint_present = evidence_persistence
            .get("file_present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let reconciliation_persistence = response_value(self.reconciliation_persistence_status());
        let reconciliation_enabled = reconciliation_persistence
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let reconciliation_checkpoint_present = reconciliation_persistence
            .get("file_present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let artifact_persistence = response_value(self.artifact_persistence_status());
        let artifact_enabled = artifact_persistence
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let artifact_checkpoint_present = artifact_persistence
            .get("file_present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ci_provider_evidence_persistence =
            response_value(self.ci_provider_evidence_persistence_status());
        let ci_provider_evidence_enabled = ci_provider_evidence_persistence
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ci_provider_evidence_checkpoint_present = ci_provider_evidence_persistence
            .get("file_present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-recovery-matrix/0.1",
                "scope": "single-process-api-instance",
                "automatic_resume": false,
                "automatic_external_delivery": false,
                "boundaries": [
                    {
                        "id": "mission_jobs",
                        "configured": mission_enabled,
                        "checkpoint_present": mission_checkpoint_present,
                        "schema_version": MISSION_STATE_SCHEMA_VERSION,
                        "state_digest": mission_state_digest,
                        "integrity_verified": mission_integrity_verified,
                        "restores": [
                            "terminal mission status, bounded progress, retained trace, size-limited result metadata, and bounded execution provenance"
                        ],
                        "does_not_restore": [
                            "queued or running execution",
                            "in-flight external effects",
                            "effect rollback or distributed scheduling"
                        ],
                        "operator_action": "inspect recovered_after_restart and re-submit interrupted work explicitly"
                    },
                    {
                        "id": "event_rows",
                        "configured": event_enabled,
                        "checkpoint_present": event_checkpoint_present,
                        "schema_version": EVENT_STATE_SCHEMA_VERSION,
                        "state_digest": state_digest,
                        "integrity_verified": event_integrity_verified,
                        "restores": [
                            "retained sequence-addressed event rows",
                            "next cursor and retention-gap accounting"
                        ],
                        "does_not_restore": [
                            "distributed consensus or cross-instance ordering"
                        ],
                        "operator_action": "verify the state_digest and treat retention gaps as explicit evidence"
                    },
                    {
                        "id": "mission_execution_queue",
                        "configured": mission_queue_enabled,
                        "checkpoint_present": mission_queue_checkpoint_present,
                        "schema_version": mission_queue_persistence.get("schema_version").cloned().unwrap_or(Value::Null),
                        "state_digest": mission_queue_persistence.get("state_digest").cloned().unwrap_or(Value::Null),
                        "integrity_verified": mission_queue_persistence.get("integrity_verified").cloned().unwrap_or(Value::Null),
                        "restores": [
                            "typed mission job state, idempotency class, attempt count, lease ownership, staged/committed output boundary, and recovery posture"
                        ],
                        "does_not_restore": [
                            "an in-process worker thread",
                            "external effect completion or rollback",
                            "cross-node lease fencing, tenant/fair-share scheduling, provider authentication, or automatic dispatch"
                        ],
                        "operator_action": "inspect /v1/missions/queue and explicitly resubmit interrupted work after reviewing quarantine or requeue evidence"
                    },
                    {
                        "id": "subscription_metadata",
                        "configured": event_enabled,
                        "checkpoint_present": event_checkpoint_present,
                        "schema_version": EVENT_STATE_SCHEMA_VERSION,
                        "state_digest": state_digest,
                        "integrity_verified": event_integrity_verified,
                        "restores": [
                            "subscription id, endpoint, event filters, and creation sequence"
                        ],
                        "does_not_restore": [
                            "active delivery authorization",
                            "webhook signing secrets"
                        ],
                        "operator_action": "POST /v1/webhooks/subscriptions/{id}/rebind before retry, replay, or delivery"
                    },
                    {
                        "id": "webhook_outbox",
                        "configured": event_enabled,
                        "checkpoint_present": event_checkpoint_present,
                        "schema_version": EVENT_STATE_SCHEMA_VERSION,
                        "state_digest": state_digest,
                        "integrity_verified": event_integrity_verified,
                        "restores": [
                            "pending delivery ids, attempts, signed envelope evidence, and bounded failure metadata"
                        ],
                        "does_not_restore": [
                            "receiver acceptance",
                            "network sends or automatic acknowledgement"
                        ],
                        "operator_action": "rebind secrets, poll the outbox through an egress-controlled worker, then acknowledge accepted ids"
                    },
                    {
                        "id": "delivery_attempts",
                        "configured": event_enabled,
                        "checkpoint_present": event_checkpoint_present,
                        "schema_version": EVENT_STATE_SCHEMA_VERSION,
                        "state_digest": state_digest,
                        "integrity_verified": event_integrity_verified,
                        "restores": [
                            "bounded send, retry, replay, acknowledgement, and secret-rebind provenance rows"
                        ],
                        "does_not_restore": [
                            "receiver state beyond explicit worker acknowledgement",
                            "network transport or external side effects"
                        ],
                        "operator_action": "query /v1/webhooks/subscriptions/{id}/attempts and correlate attempt_id with delivery_id"
                    },
                    {
                        "id": "evidence_bundle_registry",
                        "configured": evidence_enabled,
                        "checkpoint_present": evidence_checkpoint_present,
                        "schema_version": evidence_persistence.get("schema").cloned().unwrap_or(Value::Null),
                        "state_digest": evidence_persistence.get("state_digest").cloned().unwrap_or(Value::Null),
                        "integrity_verified": evidence_persistence.get("integrity_verified").cloned().unwrap_or(Value::Null),
                        "registry_size": evidence_persistence.get("registry_size").cloned().unwrap_or(json!(0)),
                        "restores": [
                            "independently verified, content-addressed mission evidence bundles and deterministic mission/domain index rows"
                        ],
                        "does_not_restore": [
                            "queued or running execution",
                            "external effects, evaluator reruns, provenance beyond the supplied bundle",
                            "scientific validity, clinical safety, release approval, or distributed registry consensus"
                        ],
                        "operator_action": "inspect the state_digest and re-submit interrupted execution explicitly; use evidence bundles as audit artifacts only"
                    },
                    {
                        "id": "workflow_reconciliation_registry",
                        "configured": reconciliation_enabled,
                        "checkpoint_present": reconciliation_checkpoint_present,
                        "schema_version": reconciliation_persistence.get("schema").cloned().unwrap_or(Value::Null),
                        "state_digest": reconciliation_persistence.get("state_digest").cloned().unwrap_or(Value::Null),
                        "integrity_verified": reconciliation_persistence.get("integrity_verified").cloned().unwrap_or(Value::Null),
                        "registry_size": reconciliation_persistence.get("registry_size").cloned().unwrap_or(json!(0)),
                        "restores": [
                            "digest-valid workflow reconciliation reports and deterministic mission/workflow/plan/completion index rows"
                        ],
                        "does_not_restore": [
                            "mission execution, raw outputs omitted from the report, external effects, or evaluator reruns",
                            "scientific, clinical, operational, regulatory, or release truth"
                        ],
                        "operator_action": "inspect the reconciliation_digest and review_required posture; import or reconcile new evidence explicitly after a restart"
                    },
                    {
                        "id": "cross_domain_artifact_registry",
                        "configured": artifact_enabled,
                        "checkpoint_present": artifact_checkpoint_present,
                        "schema_version": artifact_persistence.get("schema").cloned().unwrap_or(Value::Null),
                        "state_digest": artifact_persistence.get("state_digest").cloned().unwrap_or(Value::Null),
                        "integrity_verified": artifact_persistence.get("integrity_verified").cloned().unwrap_or(Value::Null),
                        "registry_size": artifact_persistence.get("registry_size").cloned().unwrap_or(json!(0)),
                        "restores": [
                            "bounded exact-content artifact records, declared parent edges, and explicit verification posture"
                        ],
                        "does_not_restore": [
                            "causal provenance, scientific validity, clinical safety, publication authority, or external effect completion",
                            "execution or missing parent artifacts"
                        ],
                        "operator_action": "inspect /v1/artifacts/{content_digest}/lineage and treat missing parents as unresolved evidence"
                    },
                    {
                        "id": "ci_provider_evidence_registry",
                        "configured": ci_provider_evidence_enabled,
                        "checkpoint_present": ci_provider_evidence_checkpoint_present,
                        "schema_version": ci_provider_evidence_persistence.get("schema").cloned().unwrap_or(Value::Null),
                        "state_digest": ci_provider_evidence_persistence.get("state_digest").cloned().unwrap_or(Value::Null),
                        "integrity_verified": ci_provider_evidence_persistence.get("integrity_verified").cloned().unwrap_or(Value::Null),
                        "registry_size": ci_provider_evidence_persistence.get("registry_size").cloned().unwrap_or(json!(0)),
                        "restores": [
                            "re-audited provider/run/check evidence with deterministic artifact, log, and attestation record-digest joins",
                            "failed and unknown provider runs as explicit non-conformant evidence records"
                        ],
                        "does_not_restore": [
                            "provider authentication, remote artifact/log bytes, signature verification, execution, or release authority"
                        ],
                        "operator_action": "inspect provider_evidence_digest, then correlate artifact_record_digest, log_record_digest, and attestation_record_digest before making a separate release decision"
                    },
                    {
                        "id": "webhook_signing_secrets",
                        "configured": event_enabled,
                        "checkpoint_present": false,
                        "schema_version": Value::Null,
                        "state_digest": Value::Null,
                        "integrity_verified": Value::Null,
                        "restores": [],
                        "does_not_restore": [
                            "all signing secrets; they remain process-local by policy"
                        ],
                        "operator_action": "supply each secret through the explicit in-memory rebind route"
                    },
                    {
                        "id": "external_delivery_effects",
                        "configured": false,
                        "checkpoint_present": false,
                        "schema_version": Value::Null,
                        "state_digest": Value::Null,
                        "integrity_verified": Value::Null,
                        "restores": [],
                        "does_not_restore": [
                            "network transport, TLS termination, receiver state, and external side effects"
                        ],
                        "operator_action": "keep sending and acknowledgement in an operator-owned delivery worker"
                    }
                ],
                "observed": {
                    "mission_checkpoint_present": mission_checkpoint_present,
                    "mission_queue_checkpoint_present": mission_queue_checkpoint_present,
                    "event_checkpoint_present": event_checkpoint_present,
                    "artifact_checkpoint_present": artifact_checkpoint_present,
                    "retained_events": metrics.retained_events,
                    "active_subscriptions": metrics.active_subscriptions,
                    "subscriptions": metrics.subscriptions,
                    "pending_deliveries": metrics.pending_deliveries,
                    "dropped_events": metrics.dropped_events,
                    "dropped_deliveries": metrics.dropped_deliveries,
                    "retained_delivery_attempts": metrics.retained_delivery_attempts,
                    "dropped_delivery_attempts": metrics.dropped_delivery_attempts,
                    "next_attempt_id": metrics.next_attempt_id
                },
                "guarantees": [
                    "restart boundaries are reported separately for missions, events, subscriptions, outbox rows, delivery provenance, secrets, and external effects",
                    "mission execution queue recovery is reported separately from the mission status projection",
                    "absence of a checkpoint is visible and never presented as recovered state",
                    "a successful HTTP response does not claim receiver acceptance or effect completion"
                ],
                "non_claims": [
                    "distributed event storage",
                    "distributed mission scheduling",
                    "automatic job resumption",
                    "secret recovery",
                    "network delivery or receiver acknowledgement"
                ],
                "links": {
                    "mission_persistence": "/v1/missions/persistence",
                    "mission_queue": "/v1/missions/queue",
                    "mission_queue_persistence": "/v1/missions/queue/persistence",
                    "mission_queue_flush": "/v1/missions/queue/persistence/flush",
                    "event_persistence": "/v1/events/persistence",
                    "evidence_bundle_persistence": "/v1/evidence-bundles/persistence",
                    "evidence_bundle_persistence_flush": "/v1/evidence-bundles/persistence/flush",
                    "workflow_reconciliation_persistence": "/v1/domain-workflows/reconciliations/persistence",
                    "workflow_reconciliation_persistence_flush": "/v1/domain-workflows/reconciliations/persistence/flush",
                    "artifact_persistence": "/v1/artifacts/persistence",
                    "artifact_persistence_flush": "/v1/artifacts/persistence/flush",
                    "ci_provider_evidence": "/v1/ci/provider-evidence",
                    "ci_provider_evidence_persistence": "/v1/ci/provider-evidence/persistence",
                    "ci_provider_evidence_persistence_flush": "/v1/ci/provider-evidence/persistence/flush",
                    "event_flush": "/v1/events/persistence/flush",
                    "mission_flush": "/v1/missions/persistence/flush",
                    "delivery_attempts": "/v1/webhooks/subscriptions/{id}/attempts",
                    "delivery_receipt_attempts": "/v1/delivery-receipts/{receipt_id}/attempts"
                }
            }),
        )
    }

    fn operations_snapshot(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "operations snapshot accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=MAX_OPERATIONS_SNAPSHOT_LIMIT).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    &format!("limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"),
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };

        let recent_events = match self.events.lock() {
            Ok(events) => match events.events(after, limit) {
                Ok(page) => serde_json::to_value(page).unwrap_or_else(|_| json!({})),
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            },
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let metrics = self.event_metrics();
        let mission_summary = match self.operations_mission_summary() {
            Ok(summary) => summary,
            Err(code) => {
                return self.error(500, code, "mission registry is unavailable", request_id)
            }
        };
        let mission_persistence = response_value(self.mission_persistence_status());
        let mission_queue_persistence = response_value(self.mission_queue_persistence_status());
        let event_persistence = response_value(self.event_persistence_status());
        let evidence_persistence = response_value(self.evidence_persistence_status());
        let reconciliation_persistence = response_value(self.reconciliation_persistence_status());
        let artifact_persistence = response_value(self.artifact_persistence_status());
        let ci_provider_evidence_persistence =
            response_value(self.ci_provider_evidence_persistence_status());
        let reconciliation_summary = match self.reconciliation_registry.lock() {
            Ok(registry) => registry.operator_summary(),
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        let recovery = response_value(self.recovery_matrix());
        let mut operator_actions = vec![
            "advance the event cursor with recent_events.next_after and inspect gap before claiming continuity".to_string(),
            "use recovery.boundaries and persistence digests before treating restart state as authoritative".to_string(),
            "treat tool completion as local evidence; verify any external delivery or scientific claim independently".to_string(),
        ];
        if metrics.pending_deliveries > 0 {
            operator_actions.push(
                "inspect pending webhook deliveries and correlated delivery attempts through the outbox routes".to_string(),
            );
        }
        if self.config.event_state_path.is_none() {
            operator_actions.push(
                "configure event_state_path if event rows, subscriptions, outbox rows, and attempt provenance must survive restart".to_string(),
            );
        }
        if self.config.mission_state_path.is_none() {
            operator_actions.push(
                "configure mission_state_path if terminal mission snapshots must survive restart"
                    .to_string(),
            );
        }
        if self.config.mission_queue_state_path.is_none() {
            operator_actions.push(
                "configure mission_queue_state_path if mission lease, idempotency, and restart recovery state must survive an API restart".to_string(),
            );
        }
        if self.config.evidence_state_path.is_none() {
            operator_actions.push(
                "configure evidence_state_path if independently verified evidence bundles must survive an API restart"
                    .to_string(),
            );
        }
        if self.config.reconciliation_state_path.is_none() {
            operator_actions.push(
                "configure reconciliation_state_path if digest-valid workflow reconciliation audit records must survive an API restart"
                    .to_string(),
            );
        }
        if self.config.artifact_state_path.is_none() {
            operator_actions.push(
                "configure artifact_state_path if cross-domain artifact records and parent-lineage inspection must survive an API restart".to_string(),
            );
        }
        if self.config.ci_provider_evidence_state_path.is_none() {
            operator_actions.push(
                "configure ci_provider_evidence_state_path if re-audited provider CI evidence and its artifact/log/attestation joins must survive an API restart".to_string(),
            );
        }

        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-operations-snapshot/0.1",
                "service": SERVER_NAME,
                "api_version": API_VERSION,
                "protocol_version": PROTOCOL_VERSION,
                "after": after,
                "limit": limit,
                "recent_events": recent_events,
                "event_metrics": metrics,
                "mission_summary": mission_summary,
                "persistence": {
                    "missions": mission_persistence,
                    "mission_queue": mission_queue_persistence,
                    "events": event_persistence,
                    "evidence_bundles": evidence_persistence,
                    "workflow_reconciliations": reconciliation_persistence,
                    "artifacts": artifact_persistence,
                    "ci_provider_evidence": ci_provider_evidence_persistence
                },
                "reconciliation_summary": reconciliation_summary,
                "recovery": recovery,
                "domain_coverage": operations_domain_coverage(),
                "consistency": {
                    "read_model": "bounded composition of process-local stores",
                    "cross_store_atomic": false,
                    "event_cursor_authoritative": true,
                    "clock_free": true,
                    "underlying_routes_remain_authoritative": true
                },
                "capabilities": {
                    "tool_count": bioprism_mcp::tool_definitions().len(),
                    "resource_count": bioprism_mcp::resource_definitions().len(),
                    "rest_tools": true,
                    "json_rpc": true,
                    "event_cursor": true,
                    "async_missions": true,
                    "mission_inventory": true,
                    "mission_execution_queue": true,
                    "mission_queue_persistence": self.config.mission_queue_state_path.is_some(),
                    "mission_execution_provenance": true,
                    "mission_claim_lineage": true,
                    "mission_evaluator_replay_query": true,
                    "mission_evaluator_replay_compare": true,
                    "mission_evidence_bundle_export": true,
                    "mission_evidence_bundle_verify": true,
                    "mission_evidence_bundle_registry": true,
                    "mission_evidence_bundle_import": true,
                    "mission_evidence_bundle_query": true,
                    "mission_evidence_bundle_persistence": self.config.evidence_state_path.is_some(),
                    "workflow_reconciliation_registry": true,
                    "workflow_reconciliation_persistence": self.config.reconciliation_state_path.is_some(),
                    "ci_provider_evidence_registry": true,
                    "ci_provider_evidence_persistence": self.config.ci_provider_evidence_state_path.is_some(),
                    "operations_snapshot": true,
                    "domain_coverage": true,
                    "operations_domains": true,
                    "operations_gates": true,
                    "operations_gate_reviews": true,
                    "operations_handoff": true,
                    "delivery_attempt_provenance": true,
                    "external_delivery_worker": false
                },
                "operator_actions": operator_actions,
                "guarantees": [
                    "the event page is bounded by the caller-supplied cursor and the server limit",
                    "mission counts come from the process-local authoritative registry without returning terminal reports",
                    "persistence and recovery views retain their existing digest, integrity, and non-claim semantics",
                    "reconciliation_summary counts only stored digest-valid reports and keeps completion, integrity, and evidence posture separate",
                    "the snapshot reports local evidence and capability boundaries; it does not execute tools or external effects"
                ],
                "non_claims": [
                    "scientific validity or clinical safety",
                    "network delivery or receiver acceptance",
                    "automatic mission resumption",
                    "distributed ordering, consensus, or cross-instance state"
                ],
                "links": {
                    "events": "/v1/events",
                    "missions": "/v1/missions",
                    "recovery": "/v1/recovery",
                    "mission_persistence": "/v1/missions/persistence",
                    "event_persistence": "/v1/events/persistence",
                    "mission_provenance": "/v1/missions/{mission_id}/provenance",
                    "mission_claims": "/v1/missions/{mission_id}/claims",
                    "mission_evaluator_replay": "/v1/missions/{mission_id}/evaluator-replay",
                    "mission_evaluator_replay_compare": "/v1/missions/{mission_id}/evaluator-replay/compare",
                    "mission_evidence_bundle": "/v1/missions/{mission_id}/evidence-bundle",
                    "mission_evidence_bundle_verify": "/v1/evidence-bundles/verify",
                    "evidence_bundles": "/v1/evidence-bundles",
                    "evidence_bundle_persistence": "/v1/evidence-bundles/persistence",
                    "evidence_bundle_persistence_flush": "/v1/evidence-bundles/persistence/flush",
                    "workflow_reconciliations": "/v1/domain-workflows/reconciliations",
                    "workflow_reconciliation_persistence": "/v1/domain-workflows/reconciliations/persistence",
                    "workflow_reconciliation_persistence_flush": "/v1/domain-workflows/reconciliations/persistence/flush",
                    "ci_provider_evidence": "/v1/ci/provider-evidence",
                    "ci_provider_evidence_persistence": "/v1/ci/provider-evidence/persistence",
                    "ci_provider_evidence_persistence_flush": "/v1/ci/provider-evidence/persistence/flush",
                    "capabilities": "/v1/capabilities",
                    "delivery_attempts": "/v1/webhooks/subscriptions/{id}/attempts"
                }
            }),
        )
    }

    fn operations_handoff(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        match operations_handoff_value(&arguments) {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_operations_handoff", &error, request_id),
        }
    }

    fn operations_domain_activity(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "operations domain activity accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=MAX_OPERATIONS_SNAPSHOT_LIMIT).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    &format!("limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"),
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let page = match self.events.lock() {
            Ok(events) => match events.events(after, limit) {
                Ok(page) => page,
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            },
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let coverage = operations_domain_coverage();
        let groups = coverage
            .get("groups")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let capability_groups = bioprism_mcp::workspace_capabilities();
        let mut tools_by_group = BTreeMap::<String, BTreeSet<String>>::new();
        if let Some(capability_groups) = capability_groups.as_array() {
            for group in capability_groups {
                let Some(id) = group.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let tools = group
                    .get("mcp_tools")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                tools_by_group.insert(id.to_string(), tools);
            }
        }
        let advertised_tools = bioprism_mcp::tool_definitions()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let tool_name = |event: &crate::events::ApiEvent| -> Option<String> {
            event
                .payload
                .get("tool")
                .and_then(Value::as_str)
                .filter(|name| advertised_tools.contains(*name))
                .map(str::to_owned)
                .or_else(|| {
                    advertised_tools
                        .contains(&event.subject)
                        .then_some(event.subject.clone())
                })
        };
        let tool_events_scanned = page
            .events
            .iter()
            .filter(|event| tool_name(event).is_some())
            .count();
        let mut attributed_event_ids = BTreeSet::new();
        let mut groups_with_gaps = 0usize;
        let mut groups_with_observed_activity = 0usize;
        let mut catalogued_unobserved_tool_count = 0usize;
        let mut domain_rows = Vec::new();
        for group in groups {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let declared_tools = tools_by_group.get(id).cloned().unwrap_or_default();
            let advertised_group_tools = declared_tools
                .iter()
                .filter(|tool| advertised_tools.contains(*tool))
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut observed_tools = BTreeSet::new();
            let mut observed_event_count = 0usize;
            let mut last_event_id = None;
            for event in &page.events {
                let Some(tool) = tool_name(event) else {
                    continue;
                };
                if !declared_tools.contains(&tool) {
                    continue;
                }
                observed_event_count += 1;
                observed_tools.insert(tool);
                last_event_id = Some(event.id);
                attributed_event_ids.insert(event.id);
            }
            let missing_tool_count = group
                .get("missing_tool_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if missing_tool_count > 0 {
                groups_with_gaps += 1;
            }
            if observed_event_count > 0 {
                groups_with_observed_activity += 1;
            }
            let unobserved_tool_count = advertised_group_tools
                .len()
                .saturating_sub(observed_tools.intersection(&advertised_group_tools).count());
            catalogued_unobserved_tool_count += unobserved_tool_count;
            let activity_state = if missing_tool_count > 0 {
                "catalogue_gap"
            } else if observed_event_count > 0 {
                "observed_in_page"
            } else {
                "catalogued_unobserved_in_page"
            };
            let mut row = group;
            row["observed_event_count"] = json!(observed_event_count);
            row["observed_tool_count"] = json!(observed_tools.len());
            row["observed_tools"] = json!(observed_tools.into_iter().collect::<Vec<_>>());
            row["unobserved_advertised_tool_count"] = json!(unobserved_tool_count);
            row["last_event_id"] = json!(last_event_id);
            row["activity_state"] = json!(activity_state);
            row["observation_scope"] = json!("requested_event_page_only");
            domain_rows.push(row);
        }
        let unmatched_tool_events = tool_events_scanned.saturating_sub(attributed_event_ids.len());
        let event_cursor = json!({
            "after": page.after,
            "next_after": page.next_after,
            "oldest": page.oldest,
            "newest": page.newest,
            "gap": page.gap,
            "dropped_events": page.dropped_events,
            "returned_events": page.events.len()
        });
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "workflow": "operations_domain_activity",
                "schema": "bioprism-operations-domain-activity/0.1",
                "event_cursor": event_cursor,
                "groups": domain_rows,
                "summary": {
                    "group_count": coverage.get("group_count").and_then(Value::as_u64).unwrap_or(0),
                    "returned_groups": coverage.get("returned_groups").and_then(Value::as_u64).unwrap_or(0),
                    "tool_events_scanned": tool_events_scanned,
                    "attributed_tool_events": attributed_event_ids.len(),
                    "unattributed_tool_events": unmatched_tool_events,
                    "groups_with_catalogue_gaps": groups_with_gaps,
                    "groups_with_observed_activity": groups_with_observed_activity,
                    "catalogued_unobserved_tool_count": catalogued_unobserved_tool_count
                },
                "observation_policy": {
                    "event_matching": "exact advertised tool name from event payload or subject",
                    "scope": "only the bounded event page requested by the caller",
                    "control_plane_evidence_scope": "completed evaluation, safety, and release tools are pooled across the page and applied to each matched domain group",
                    "cross_group_membership": "one tool event may contribute to multiple groups",
                    "readiness_claimed": false
                },
                "guarantees": [
                    "catalogue coverage and observed local activity remain separate fields",
                    "event cursor gaps and the observation window are explicit",
                    "no tool is invoked by this projection"
                ],
                "non_claims": [
                    "runtime health or successful execution for unobserved tools",
                    "scientific, clinical, safety, or release readiness",
                    "complete historical activity when retention gaps or a bounded page apply"
                ],
                "links": {
                    "operations_snapshot": "/v1/operations/snapshot",
                    "operations_domains": "/v1/operations/domains",
                    "operations_gates": "/v1/operations/gates",
                    "operations_gate_reviews": "/v1/operations/gate-reviews",
                    "operations_handoff": "/v1/operations/handoff",
                    "events": "/v1/events",
                    "capabilities": "/v1/capabilities"
                }
            }),
        )
    }

    fn operations_domain_gates(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "operations domain gates accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=MAX_OPERATIONS_SNAPSHOT_LIMIT).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    &format!("limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"),
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let page = match self.events.lock() {
            Ok(events) => match events.events(after, limit) {
                Ok(page) => page,
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            },
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let coverage = operations_domain_coverage();
        let groups = coverage
            .get("groups")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let reconciliation_registry = match self.reconciliation_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        let artifact_registry = match self.artifact_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        let artifact_registry_generation = artifact_registry.generation();
        let artifact_registry_size = artifact_registry.len();
        let capability_groups = bioprism_mcp::workspace_capabilities();
        let mut tools_by_group = BTreeMap::<String, BTreeSet<String>>::new();
        if let Some(capability_groups) = capability_groups.as_array() {
            for group in capability_groups {
                let Some(id) = group.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let tools = group
                    .get("mcp_tools")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                tools_by_group.insert(id.to_string(), tools);
            }
        }
        let advertised_tools = bioprism_mcp::tool_definitions()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let tool_name = |event: &crate::events::ApiEvent| -> Option<String> {
            event
                .payload
                .get("tool")
                .and_then(Value::as_str)
                .filter(|name| advertised_tools.contains(*name))
                .map(str::to_owned)
                .or_else(|| {
                    advertised_tools
                        .contains(&event.subject)
                        .then_some(event.subject.clone())
                })
        };
        let tool_events = page
            .events
            .iter()
            .filter_map(|event| tool_name(event).map(|tool| (event, tool)))
            .collect::<Vec<_>>();
        let tool_events_scanned = tool_events.len();
        let completed_tool_events = tool_events
            .iter()
            .filter(|(event, _)| event.event_type == "tool.completed")
            .count();
        let refused_tool_events = tool_events
            .iter()
            .filter(|(event, _)| {
                matches!(event.event_type.as_str(), "tool.refused" | "tool.rpc_error")
            })
            .count();
        let mut global_channel_events = BTreeMap::<String, usize>::new();
        let mut global_channel_tools = BTreeMap::<String, BTreeSet<String>>::new();
        let mut evaluator_bindings_by_group = BTreeMap::<String, Vec<Value>>::new();
        for (event, tool) in &tool_events {
            if event.event_type != "tool.completed" {
                continue;
            }
            for channel in operations_evidence_channels(tool) {
                *global_channel_events
                    .entry((*channel).to_string())
                    .or_default() += 1;
                global_channel_tools
                    .entry((*channel).to_string())
                    .or_default()
                    .insert(tool.clone());
            }
            if operations_evidence_channels(tool).contains(&"evaluation") {
                for group_id in operations_evaluator_group_bindings(tool) {
                    evaluator_bindings_by_group
                        .entry((*group_id).to_string())
                        .or_default()
                        .push(json!({
                            "tool": tool,
                            "event_id": event.id,
                            "binding": "catalogue_group_alias"
                        }));
                }
            }
        }
        let mut attributed_event_ids = BTreeSet::new();
        let mut groups_blocked_catalogue = 0usize;
        let mut groups_insufficient_evidence = 0usize;
        let mut groups_review_required = 0usize;
        let mut groups_reconciliation_blocked = 0usize;
        let mut groups_with_artifact_evidence = 0usize;
        let mut artifact_evidence_records = 0usize;
        let mut rows = Vec::new();
        for group in groups {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let group_domains = group
                .get("domains")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let artifact_evidence = artifact_registry.domain_evidence_posture(id, &group_domains);
            let matching_artifact_records = artifact_evidence
                .get("matching_record_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if matching_artifact_records > 0 {
                groups_with_artifact_evidence += 1;
                artifact_evidence_records =
                    artifact_evidence_records.saturating_add(matching_artifact_records);
            }
            let reconciliation_posture = reconciliation_registry.workflow_posture(id);
            let reconciliation_state = reconciliation_posture
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("missing");
            let reconciliation_blocks = matches!(reconciliation_state, "incomplete" | "invalid");
            if reconciliation_blocks {
                groups_reconciliation_blocked += 1;
            }
            let declared_tools = tools_by_group.get(id).cloned().unwrap_or_default();
            let missing_tool_count = group
                .get("missing_tool_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let mut observed_tools = BTreeSet::new();
            let mut completed_tools = BTreeSet::new();
            let mut refused_tools = BTreeSet::new();
            let mut observed_event_count = 0usize;
            let mut completed_event_count = 0usize;
            let mut refused_event_count = 0usize;
            let channel_tools = global_channel_tools.clone();
            let channel_events = global_channel_events.clone();
            let evaluator_bindings = evaluator_bindings_by_group
                .get(id)
                .cloned()
                .unwrap_or_default();
            let evaluator_tools = evaluator_bindings
                .iter()
                .filter_map(|binding| binding.get("tool").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            let mut last_event_id = None;
            for (event, tool) in &tool_events {
                if !declared_tools.contains(tool) {
                    continue;
                }
                observed_event_count += 1;
                observed_tools.insert(tool.clone());
                last_event_id = Some(event.id);
                attributed_event_ids.insert(event.id);
                match event.event_type.as_str() {
                    "tool.completed" => {
                        completed_event_count += 1;
                        completed_tools.insert(tool.clone());
                    }
                    "tool.refused" | "tool.rpc_error" => {
                        refused_event_count += 1;
                        refused_tools.insert(tool.clone());
                    }
                    _ => {}
                }
            }
            let channel_gate = |name: &str| {
                let tools = channel_tools.get(name).cloned().unwrap_or_default();
                let events = channel_events.get(name).copied().unwrap_or(0);
                json!({
                    "state": if events > 0 { "observed" } else { "missing" },
                    "scope": "cross_domain_control_plane_event_page",
                    "event_count": events,
                    "tool_count": tools.len(),
                    "tools": tools.into_iter().collect::<Vec<_>>()
                })
            };
            let catalogue_state = if missing_tool_count == 0 {
                "pass"
            } else {
                "blocked"
            };
            let activity_state = if observed_event_count > 0 {
                "observed"
            } else {
                "missing"
            };
            let transport_state = if completed_event_count > 0 {
                "observed"
            } else if refused_event_count > 0 {
                "refused_or_failed"
            } else {
                "missing"
            };
            let domain_evaluator_state = if evaluator_bindings.is_empty() {
                "missing"
            } else {
                "observed"
            };
            let gate_state = if missing_tool_count > 0 {
                groups_blocked_catalogue += 1;
                "catalogue_blocked"
            } else if observed_event_count == 0
                || completed_event_count == 0
                || channel_events.get("evaluation").copied().unwrap_or(0) == 0
                || evaluator_bindings.is_empty()
                || channel_events.get("safety").copied().unwrap_or(0) == 0
                || channel_events.get("release").copied().unwrap_or(0) == 0
                || reconciliation_blocks
            {
                groups_insufficient_evidence += 1;
                "insufficient_evidence"
            } else {
                groups_review_required += 1;
                "review_required"
            };
            rows.push(json!({
                "id": id,
                "status": group.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                "domains": group.get("domains").cloned().unwrap_or_else(|| json!([])),
                "declared_tool_count": group.get("declared_tool_count").cloned().unwrap_or_else(|| json!(0)),
                "advertised_tool_count": group.get("advertised_tool_count").cloned().unwrap_or_else(|| json!(0)),
                "missing_tool_count": missing_tool_count,
                "missing_tools": group.get("missing_tools").cloned().unwrap_or_else(|| json!([])),
                "gate_state": gate_state,
                "readiness_claimed": false,
                "gates": {
                    "catalogue": { "state": catalogue_state, "missing_tool_count": missing_tool_count },
                    "observed_activity": { "state": activity_state, "event_count": observed_event_count, "tool_count": observed_tools.len(), "tools": observed_tools },
                    "transport_completion": { "state": transport_state, "event_count": completed_event_count, "tool_count": completed_tools.len(), "tools": completed_tools, "refused_event_count": refused_event_count, "refused_tool_count": refused_tools.len(), "refused_tools": refused_tools },
                    "evaluation_evidence": channel_gate("evaluation"),
                    "domain_evaluator_evidence": {
                        "state": domain_evaluator_state,
                        "scope": "completed_evaluator_tool_exact_or_catalogue_group_binding",
                        "event_count": evaluator_bindings.len(),
                        "tool_count": evaluator_tools.len(),
                        "tools": evaluator_tools,
                        "bindings": evaluator_bindings,
                        "readiness_claimed": false
                    },
                    "safety_evidence": channel_gate("safety"),
                    "release_evidence": channel_gate("release"),
                    "reconciliation_evidence": reconciliation_posture,
                    "artifact_evidence": artifact_evidence
                },
                "last_event_id": last_event_id,
                "evidence_scope": "requested_event_page_only",
                "artifact_evidence_scope": "current_digest_verified_artifact_registry_exact_declared_matches"
            }));
        }
        let unmatched_tool_events = tool_events_scanned.saturating_sub(attributed_event_ids.len());
        let mut body = json!({
            "ok": true,
            "workflow": "operations_domain_gates",
            "schema": "bioprism-operations-domain-gates/0.1",
            "event_cursor": {
                "after": page.after,
                "next_after": page.next_after,
                "oldest": page.oldest,
                "newest": page.newest,
                "gap": page.gap,
                "dropped_events": page.dropped_events,
                "returned_events": page.events.len()
            },
            "artifact_evidence_scope": "current_digest_verified_artifact_registry_exact_declared_matches",
            "groups": rows,
            "summary": {
                "group_count": coverage.get("group_count").and_then(Value::as_u64).unwrap_or(0),
                "returned_groups": coverage.get("returned_groups").and_then(Value::as_u64).unwrap_or(0),
                "tool_events_scanned": tool_events_scanned,
                "attributed_tool_events": attributed_event_ids.len(),
                "unattributed_tool_events": unmatched_tool_events,
                "completed_tool_events": completed_tool_events,
                "refused_tool_events": refused_tool_events,
                "evaluation_evidence_events": global_channel_events.get("evaluation").copied().unwrap_or(0),
                "domain_evaluator_evidence_events": evaluator_bindings_by_group.values().map(Vec::len).sum::<usize>(),
                "safety_evidence_events": global_channel_events.get("safety").copied().unwrap_or(0),
                "release_evidence_events": global_channel_events.get("release").copied().unwrap_or(0),
                "groups_blocked_catalogue": groups_blocked_catalogue,
                "groups_insufficient_evidence": groups_insufficient_evidence,
                "groups_review_required": groups_review_required,
                "groups_reconciliation_blocked": groups_reconciliation_blocked,
                "groups_with_artifact_evidence": groups_with_artifact_evidence,
                "artifact_evidence_records": artifact_evidence_records,
                "artifact_registry_generation": artifact_registry_generation,
                "artifact_registry_size": artifact_registry_size,
                "readiness_claimed": false
            },
            "gate_policy": {
                "required_gates": operations_required_gates(),
                "optional_evidence_gates": ["artifact_evidence"],
                "decision_rule": "all gates need observed evidence; domain evaluator evidence must bind a completed evaluator tool to the selected capability group; a complete evidence set still requires human or domain authority review",
                "event_matching": "exact advertised tool name from event payload or subject",
                "scope": "only the bounded event page requested by the caller",
                "artifact_evidence_scope": "exact declared artifact registration domain intersection or explicit artifact.group_id for the selected capability group",
                "artifact_evidence_policy": "advisory; missing artifact evidence is visible and never promoted to readiness or used to pass a required gate",
                "control_plane_evidence_scope": "completed evaluation, safety, and release tools are pooled across the page and applied to each matched domain group",
                "domain_evaluator_binding_scope": "only completed evaluation-channel tools with an exact or catalogue-declared capability-group binding",
                "reconciliation_evidence_scope": "matching workflow_id rows from the bounded digest-valid reconciliation registry",
                "reconciliation_evidence_policy": "missing is explicit and does not pass; incomplete or invalid retained posture blocks review; structurally_ready remains review-required evidence",
                "cross_group_membership": "one tool event may contribute to multiple groups",
                "readiness_claimed": false
            },
            "guarantees": [
                "catalogue, activity, transport, pooled evaluation, domain evaluator, safety, and release evidence remain separate",
                "reconciliation evidence is joined by exact capability-group workflow_id and cannot be inferred from a different domain",
                "domain evaluator evidence is a bounded catalogue binding and does not assert scientific validity or evaluator adequacy",
                "missing evidence is represented as a gate state instead of inferred readiness",
                "artifact evidence is joined from the current digest-verified registry for every returned capability group",
                "no tool is invoked by this projection"
            ],
            "non_claims": [
                "scientific validity, clinical safety, or deployment authorization",
                "successful tool transport proves only a completed local call, not semantic correctness",
                "complete historical evidence when retention gaps or a bounded page apply"
            ],
            "links": {
                "operations_snapshot": "/v1/operations/snapshot",
                "operations_domains": "/v1/operations/domains",
                "operations_gates": "/v1/operations/gates",
                "operations_gate_reviews": "/v1/operations/gate-reviews",
                "operations_handoff": "/v1/operations/handoff",
                "events": "/v1/events",
                "capabilities": "/v1/capabilities"
            }
        });
        let mut digest_body = body.clone();
        if let Some(cursor) = digest_body
            .get_mut("event_cursor")
            .and_then(Value::as_object_mut)
        {
            cursor.insert(
                "oldest".into(),
                tool_events
                    .first()
                    .map(|(event, _)| json!(event.id))
                    .unwrap_or(Value::Null),
            );
            cursor.insert(
                "newest".into(),
                tool_events
                    .last()
                    .map(|(event, _)| json!(event.id))
                    .unwrap_or(Value::Null),
            );
            cursor.insert(
                "next_after".into(),
                tool_events
                    .last()
                    .map(|(event, _)| json!(event.id))
                    .unwrap_or_else(|| json!(page.after)),
            );
            cursor.insert("returned_events".into(), json!(tool_events_scanned));
        }
        let gate_digest = serde_json::to_vec(&digest_body)
            .map(|bytes| hex_digest(&Sha256::digest(&bytes)))
            .unwrap_or_default();
        body["gate_digest"] = json!(gate_digest);
        body["gate_digest_scope"] =
            json!("operations_evidence_and_reconciliation_projection_without_gate_digest");
        HttpResponse::json(200, &body)
    }

    fn operations_gate_snapshot(&self) -> Value {
        let gate_request = HttpRequest {
            method: "GET".into(),
            target: "/v1/operations/gates?after=0&limit=256".into(),
            version: "HTTP/1.1".into(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        response_value(self.operations_domain_gates(&gate_request, "internal-operations-gates"))
    }

    fn operations_gate_reviews(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(key.as_str(), "after" | "limit" | "review_id") {
                return self.error(
                    400,
                    "invalid_query",
                    "operations gate reviews accepts only after, limit, and review_id",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let review_id = query.get("review_id").map(String::as_str);
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let page = match events.operations_gate_reviews(after, limit, review_id) {
            Ok(page) => page,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let reviews = page
            .events
            .iter()
            .filter_map(|event| {
                let payload = event.payload.as_object()?;
                let review_id = payload.get("review_id")?.as_str()?;
                Some(json!({
                    "review_id": review_id,
                    "event_id": event.id,
                    "request_id": event.request_id,
                    "acceptance": payload.get("acceptance").cloned().unwrap_or_else(|| json!({})),
                    "gate_digest": payload.get("gate_digest").cloned().unwrap_or(Value::Null),
                    "group_ids": payload.get("group_ids").cloned().unwrap_or_else(|| json!([])),
                    "evidence": payload.get("evidence").cloned().unwrap_or_else(|| json!([])),
                    "replay": format!("/v1/operations/gate-reviews?review_id={review_id}"),
                    "readiness_claimed": false
                }))
            })
            .collect::<Vec<_>>();
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "workflow": "operations_gate_reviews",
                "schema": "bioprism-operations-gate-reviews/0.1",
                "review_id": review_id,
                "found": !reviews.is_empty(),
                "page": page,
                "reviews": reviews,
                "review_count": reviews.len(),
                "durability": {
                    "event_checkpoint_configured": self.config.event_state_path.is_some(),
                    "durable_across_restart": self.config.event_state_path.is_some(),
                    "persistence_endpoint": "/v1/events/persistence"
                },
                "readiness_claimed": false,
                "guarantees": [
                    "review records are read from the bounded retained event log",
                    "content-addressed review IDs and event cursors make the acceptance replayable",
                    "retention gaps remain visible instead of being presented as complete history",
                    "cross-restart durability is true only when the event checkpoint is configured"
                ],
                "non_claims": [
                    "a retained operator review is not scientific, clinical, regulatory, or deployment approval"
                ]
            }),
        )
    }

    fn create_operations_gate_review(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let object = match self.json_object(request) {
            Ok(object) => object,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let acceptance = Value::Object(object.clone());
        if let Err(error) = validate_operations_gate_acceptance_value(&acceptance, false) {
            return self.error(422, "invalid_operations_gate_review", &error, request_id);
        }
        let snapshot = self.operations_gate_snapshot();
        let group_ids = object
            .get("group_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let rows = operations_gate_review_rows(&snapshot, &group_ids);
        if rows.len() != group_ids.len() {
            return self.error(
                422,
                "operations_gate_review_unresolved_group",
                "every reviewed group_id must exist in the current operations gate catalogue",
                request_id,
            );
        }
        let gate_digest = snapshot.get("gate_digest").cloned().unwrap_or(Value::Null);
        let match_arguments = json!({ "operations_gate_acceptance": acceptance.clone() });
        if !operations_gate_acceptance_matches(&match_arguments, &group_ids, &gate_digest, &rows) {
            return self.error(
                422,
                "operations_gate_review_not_ready",
                "a review requires a current gate digest and every selected group to be review_required with all required accepted gates",
                request_id,
            );
        }
        let canonical = operations_gate_acceptance_canonical(&acceptance)
            .ok_or_else(|| "operations gate review could not be canonicalized".to_string());
        let canonical = match canonical {
            Ok(value) => value,
            Err(error) => return self.error(500, "review_digest_failed", &error, request_id),
        };
        let canonical_bytes = match serde_json::to_vec(&canonical) {
            Ok(bytes) => bytes,
            Err(error) => {
                return self.error(
                    500,
                    "review_digest_failed",
                    &format!("operations gate review could not be digested: {error}"),
                    request_id,
                )
            }
        };
        let review_id = hex_digest(&Sha256::digest(&canonical_bytes));
        let mut recorded_acceptance = object;
        recorded_acceptance.insert("review_id".into(), json!(review_id));
        let payload = json!({
            "workflow": "operations_gate_review",
            "schema": "bioprism-operations-gate-review/0.1",
            "review_id": review_id,
            "acceptance": recorded_acceptance,
            "gate_digest": gate_digest,
            "group_ids": group_ids,
            "evidence": rows,
            "readiness_claimed": false
        });
        let event = {
            let mut events = match self.events.lock() {
                Ok(events) => events,
                Err(_) => {
                    return self.error(
                        500,
                        "event_log_unavailable",
                        "event log is unavailable",
                        request_id,
                    )
                }
            };
            match events.emit(
                "operations.gate_review.accepted",
                "operations_gate_review",
                request_id,
                payload.clone(),
            ) {
                Ok(event) => event,
                Err(error) => return self.error(500, "event_emit_failed", &error, request_id),
            }
        };
        if let Err(error) = self.event_persistence.persist() {
            return self.error(503, "event_persistence_unavailable", &error, request_id);
        }
        HttpResponse::json(
            201,
            &json!({
                "ok": true,
                "workflow": "operations_gate_review",
                "schema": "bioprism-operations-gate-review/0.1",
                "review_id": review_id,
                "event_id": event.id,
                "request_id": request_id,
                "acceptance": payload["acceptance"],
                "gate_digest": payload["gate_digest"],
                "group_ids": payload["group_ids"],
                "evidence": payload["evidence"],
                "replay": format!("/v1/operations/gate-reviews?review_id={review_id}"),
                "durability": {
                    "event_checkpoint_configured": self.config.event_state_path.is_some(),
                    "durable_across_restart": self.config.event_state_path.is_some(),
                    "persistence_endpoint": "/v1/events/persistence"
                },
                "readiness_claimed": false,
                "guarantees": [
                    "the acceptance is appended to the retained event log before the success response",
                    "the review ID is a content hash of the normalized acceptance",
                    "missions must present this retained review ID and matching acceptance before execution",
                    "cross-restart durability is true only when the event checkpoint is configured"
                ],
                "non_claims": [
                    "the review is not scientific, clinical, regulatory, or deployment approval"
                ]
            }),
        )
    }

    fn operations_gate_review_matches(
        &self,
        arguments: &Value,
        gate_digest: &Value,
    ) -> Option<u64> {
        let acceptance = arguments
            .get("operations_gate_acceptance")
            .and_then(Value::as_object)?;
        let review_id = acceptance.get("review_id").and_then(Value::as_str)?;
        let current_fingerprint =
            operations_gate_acceptance_canonical(&Value::Object(acceptance.clone()))?;
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => return None,
        };
        let page = match events.events_for_operations_gate_review(0, 1000, review_id) {
            Ok(page) => page,
            Err(_) => return None,
        };
        page.events.iter().find_map(|event| {
            let matches = event.payload.get("review_id").and_then(Value::as_str) == Some(review_id)
                && event.payload.get("gate_digest") == Some(gate_digest)
                && event
                    .payload
                    .get("acceptance")
                    .and_then(operations_gate_acceptance_canonical)
                    .is_some_and(|fingerprint| fingerprint == current_fingerprint);
            matches.then_some(event.id)
        })
    }

    fn operations_gate_projection(&self, arguments: &Value) -> Value {
        let requirements = mission_domain_group_requirements(arguments);
        let snapshot = self.operations_gate_snapshot();
        let group_ids = requirements
            .get("group_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut rows = Vec::new();
        let mut all_review_required = !group_ids.is_empty();
        for group_id in &group_ids {
            let group_id = group_id.as_str().unwrap_or("unknown");
            let group = snapshot
                .get("groups")
                .and_then(Value::as_array)
                .and_then(|groups| {
                    groups
                        .iter()
                        .find(|group| group.get("id").and_then(Value::as_str) == Some(group_id))
                });
            let Some(group) = group else {
                all_review_required = false;
                rows.push(json!({
                    "group_id": group_id,
                    "gate_state": "unresolved_group",
                    "missing_gates": operations_required_gates(),
                    "readiness_claimed": false
                }));
                continue;
            };
            let gate_state = group
                .get("gate_state")
                .and_then(Value::as_str)
                .unwrap_or("insufficient_evidence");
            let gates = group.get("gates").and_then(Value::as_object);
            let missing_gates = operations_required_gates()
                .iter()
                .filter(|gate| {
                    let expected = if **gate == "catalogue" {
                        "pass"
                    } else {
                        "observed"
                    };
                    gates
                        .and_then(|gates| gates.get(**gate))
                        .and_then(|gate| gate.get("state"))
                        .and_then(Value::as_str)
                        != Some(expected)
                })
                .map(|gate| (*gate).to_string())
                .collect::<Vec<_>>();
            if gate_state != "review_required" || !missing_gates.is_empty() {
                all_review_required = false;
            }
            rows.push(json!({
                "group_id": group_id,
                "gate_state": gate_state,
                "missing_gates": missing_gates,
                "gates": group.get("gates").cloned().unwrap_or_else(|| json!({})),
                "last_event_id": group.get("last_event_id").cloned().unwrap_or(Value::Null),
                "readiness_claimed": false
            }));
        }
        let unresolved_steps = requirements
            .get("unresolved_steps")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let decision = if !unresolved_steps.is_empty()
            || rows
                .iter()
                .any(|row| row["gate_state"] == json!("unresolved_group"))
        {
            "unresolved_domain"
        } else if rows
            .iter()
            .any(|row| row["gate_state"] == json!("catalogue_blocked"))
        {
            "catalogue_blocked"
        } else if !all_review_required {
            "insufficient_evidence"
        } else {
            "review_required"
        };
        let gate_digest = snapshot.get("gate_digest").cloned().unwrap_or(Value::Null);
        let acceptance_matches =
            operations_gate_acceptance_matches(arguments, &group_ids, &gate_digest, &rows);
        let review_event_id = self.operations_gate_review_matches(arguments, &gate_digest);
        let review_present = review_event_id.is_some();
        let acceptance_valid = all_review_required && acceptance_matches && review_present;
        let review_id = arguments
            .pointer("/operations_gate_acceptance/review_id")
            .cloned()
            .unwrap_or(Value::Null);
        json!({
            "schema": "bioprism-operations-preflight-evidence/0.1",
            "gate_digest": gate_digest,
            "gate_digest_scope": "operations_evidence_and_reconciliation_projection_without_gate_digest",
            "group_ids": group_ids,
            "groups": rows,
            "unresolved_steps": unresolved_steps,
            "required_gates": operations_required_gates(),
            "decision": decision,
            "acceptance_required": mission_execution_requested(arguments),
            "acceptance_present": arguments.get("operations_gate_acceptance").is_some(),
            "review_id": review_id,
            "review_event_id": review_event_id,
            "review_present": review_present,
            "acceptance_matches_current_gates": acceptance_matches,
            "acceptance_valid": acceptance_valid,
            "dispatch_prerequisite": if acceptance_valid { "satisfied" } else { "acceptance_required" },
            "gate_endpoint": "/v1/operations/gates?after=0&limit=256",
            "review_endpoint": "/v1/operations/gate-reviews",
            "readiness_claimed": false,
            "guarantees": [
                "mission steps are mapped to every matching workspace capability group by exact tool name",
                "all seven evidence gates remain separate from the mission execution policy",
                "execution acceptance is bound to the current gate digest, selected group set, and retained review event"
            ],
            "non_claims": [
                "scientific validity, clinical safety, or deployment authorization",
                "an operator acceptance is not a domain-authority or regulatory approval",
                "complete historical evidence beyond the bounded retained event page"
            ]
        })
    }

    fn operations_mission_summary(&self) -> Result<Value, &'static str> {
        let jobs = self
            .mission_jobs
            .lock()
            .map_err(|_| "mission_registry_unavailable")?;
        let mut status_counts = BTreeMap::<String, usize>::new();
        let mut recovered_after_restart = 0usize;
        let mut cancel_requested = 0usize;
        for job in jobs.values() {
            let state = job_state(job).map_err(|_| "mission_state_unavailable")?;
            *status_counts.entry(state.status).or_default() += 1;
            if state.recovered_after_restart {
                recovered_after_restart += 1;
            }
            if state.cancel_requested {
                cancel_requested += 1;
            }
        }
        Ok(json!({
            "total": jobs.len(),
            "status_counts": status_counts,
            "recovered_after_restart": recovered_after_restart,
            "cancel_requested": cancel_requested,
            "registry_capacity": MAX_MISSION_JOBS
        }))
    }

    fn flush_event_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.event_state_path.is_none() {
            return self.error(
                409,
                "event_persistence_disabled",
                "configure --event-state before flushing an event snapshot",
                request_id,
            );
        }
        match self.event_persistence.persist() {
            Ok(_) => self.event_persistence_status(),
            Err(error) => self.error(503, "event_persistence_unavailable", &error, request_id),
        }
    }

    fn flush_mission_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.mission_state_path.is_none() {
            return self.error(
                409,
                "mission_persistence_disabled",
                "configure --mission-state before flushing a mission snapshot",
                request_id,
            );
        }
        match self.persist_mission_registry() {
            Ok(()) => self.mission_persistence_status(),
            Err(error) => self.error(503, "mission_persistence_unavailable", &error, request_id),
        }
    }

    fn request_id(&self, request: &HttpRequest) -> String {
        if let Some(value) = request.header("x-request-id") {
            if !value.is_empty() && value.len() <= 256 && value.bytes().all(|byte| byte >= 0x20) {
                return value.to_string();
            }
        }
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let id = format!("http-{id}");
        id
    }

    fn authorized(&self, request: &HttpRequest) -> bool {
        let Some(expected) = self.config.bearer_token.as_deref() else {
            return true;
        };
        let Some(actual) = request.header("authorization") else {
            return false;
        };
        let Some(actual) = actual.strip_prefix("Bearer ") else {
            return false;
        };
        constant_time_equal(actual.as_bytes(), expected.as_bytes())
    }

    fn finish(&self, response: HttpResponse, request_id: &str) -> HttpResponse {
        response
            .with_header("x-request-id", request_id)
            .with_header("cache-control", "no-store")
    }

    fn health(&self, _ready: bool) -> HttpResponse {
        let metrics = self.event_metrics();
        let payload = json!({
            "ok": true,
            "ready": true,
            "service": SERVER_NAME,
            "api_version": API_VERSION,
            "protocol_version": PROTOCOL_VERSION,
            "event_metrics": metrics,
            "guarantees": [
                "HTTP requests are bounded before JSON parsing",
                "domain calls delegate to the same MCP server implementation",
                "event cursors expose retention gaps instead of silently skipping history"
            ],
        });
        HttpResponse::json(200, &payload)
    }

    fn index(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "service": SERVER_NAME,
                "api_version": API_VERSION,
                "links": {
                    "health": "/healthz",
                    "ready": "/readyz",
                    "openapi": "/v1/openapi.json",
                    "capabilities": "/v1/capabilities",
                    "capability_dashboard": "/v1/capabilities/dashboard",
                    "capability_route": "/v1/capabilities/route",
                    "capability_route_review": "/v1/capabilities/route/review",
                    "capability_route_plan": "/v1/capabilities/route/plan",
                    "capability_route_plan_verify": "/v1/capabilities/route/plan/verify",
                    "recovery": "/v1/recovery",
                    "operations_snapshot": "/v1/operations/snapshot",
                    "operations_domains": "/v1/operations/domains",
                    "operations_gates": "/v1/operations/gates",
                    "operations_gate_reviews": "/v1/operations/gate-reviews",
                    "operations_handoff": "/v1/operations/handoff",
                    "domain_workflows": "/v1/domain-workflows",
                    "domain_reports": "/v1/domain-reports",
                    "domain_report_coverage": "/v1/domain-reports/coverage",
                    "domain_evidence_harmonize": "/v1/domain-evidence/harmonize",
                    "domain_evidence_harmonization_coverage": "/v1/domain-evidence/harmonization/coverage",
                    "domain_evidence_lineage": "/v1/domain-evidence/lineage",
                    "domain_evidence_intake": "/v1/domain-evidence/intake",
                    "domain_evidence_source_plan": "/v1/domain-evidence/sources",
                    "domain_evidence_source_execute": "/v1/domain-evidence/sources/execute",
                    "domain_evidence_coverage": "/v1/domain-evidence/coverage",
                    "domain_decision_readiness": "/v1/domain-decision-readiness",
                    "control_plane_readiness": "/v1/control-plane-readiness",
                    "control_plane_readiness_compare": "/v1/control-plane-readiness/compare",
                    "control_plane_readiness_compare_retained": "/v1/control-plane-readiness/compare-retained",
                    "domain_workflow_scaffold": "/v1/domain-workflows/scaffold",
                    "domain_workflow_instantiate": "/v1/domain-workflows/instantiate",
                    "domain_workflow_portfolio": "/v1/domain-workflows/portfolio",
                    "domain_workflow_portfolio_verify": "/v1/domain-workflows/portfolio/verify",
                    "developer_workbench_verify": "/v1/developer-workbench/verify",
                    "developer_workbench_reports": "/v1/developer-workbench/reports",
                    "developer_workbench_report_persistence": "/v1/developer-workbench/reports/persistence",
                    "developer_workbench_report_persistence_flush": "/v1/developer-workbench/reports/persistence/flush",
                    "ci_provider_evidence": "/v1/ci/provider-evidence",
                    "ci_provider_evidence_persistence": "/v1/ci/provider-evidence/persistence",
                    "ci_provider_evidence_persistence_flush": "/v1/ci/provider-evidence/persistence/flush",
                    "domain_workflow_verify": "/v1/domain-workflows/verify",
                    "domain_workflow_reconcile": "/v1/domain-workflows/reconcile",
                    "domain_workflow_reconciliations": "/v1/domain-workflows/reconciliations",
                    "domain_workflow_reconciliation_persistence": "/v1/domain-workflows/reconciliations/persistence",
                    "domain_workflow_reconciliation_persistence_flush": "/v1/domain-workflows/reconciliations/persistence/flush",
                    "tools": "/v1/tools",
                    "missions": "/v1/missions",
                     "mission_provenance": "/v1/missions/{mission_id}/provenance",
                     "mission_claims": "/v1/missions/{mission_id}/claims",
                     "mission_evaluator_replay": "/v1/missions/{mission_id}/evaluator-replay",
                     "mission_evaluator_replay_compare": "/v1/missions/{mission_id}/evaluator-replay/compare",
                     "mission_evidence_bundle": "/v1/missions/{mission_id}/evidence-bundle",
                     "mission_evidence_bundle_verify": "/v1/evidence-bundles/verify",
                     "evidence_bundles": "/v1/evidence-bundles",
                     "evidence_bundle_persistence": "/v1/evidence-bundles/persistence",
                     "evidence_bundle_persistence_flush": "/v1/evidence-bundles/persistence/flush",
                    "artifacts": "/v1/artifacts",
                    "domain_decision_readiness_query": "/v1/domain-decision-readiness",
                    "control_plane_readiness_query": "/v1/control-plane-readiness",
                    "control_plane_readiness_compare_retained": "/v1/control-plane-readiness/compare-retained",
                     "artifact_persistence": "/v1/artifacts/persistence",
                     "artifact_persistence_flush": "/v1/artifacts/persistence/flush",
                    "mission_persistence": "/v1/missions/persistence",
                    "mission_queue": "/v1/missions/queue",
                     "mission_queue_persistence": "/v1/missions/queue/persistence",
                     "mission_queue_persistence_flush": "/v1/missions/queue/persistence/flush",
                     "mission_queue_authority_release_lock": "/v1/missions/queue/authority/release-lock",
                    "mission_preflight": "/v1/missions/preflight",
                    "events": "/v1/events",
                    "delivery_receipt_events": "/v1/delivery-receipts/{receipt_id}/events",
                    "delivery_receipt_attempts": "/v1/delivery-receipts/{receipt_id}/attempts",
                    "route_review_evidence": "/v1/route-reviews/{review_id}/evidence",
                    "event_persistence": "/v1/events/persistence",
                    "webhooks": "/v1/webhooks/subscriptions",
                    "delivery_attempts": "/v1/webhooks/subscriptions/{id}/attempts"
                }
            }),
        )
    }

    fn capabilities(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "api_version": API_VERSION,
                "mcp_protocol_version": PROTOCOL_VERSION,
                "tool_count": bioprism_mcp::tool_definitions().len(),
                "resource_count": bioprism_mcp::resource_definitions().len(),
                "workspace": bioprism_mcp::workspace_capabilities(),
                "transport": {
                    "rest_tools": true,
                    "json_rpc": true,
                    "event_cursor": true,
                    "server_sent_events_snapshot": true,
                    "async_missions": true,
                    "mission_preflight": true,
                    "mission_inventory": true,
                    "mission_execution_provenance": true,
                    "mission_claim_lineage": true,
                    "mission_trace": true,
                    "delivery_receipt_events": true,
                    "delivery_receipt_attempt_provenance": true,
                    "route_review_evidence": true,
                    "mission_evidence_bundle_registry": true,
                    "mission_evidence_bundle_import": true,
                    "mission_evidence_bundle_query": true,
                    "mission_evidence_bundle_persistence": self.config.evidence_state_path.is_some(),
                    "artifact_registry": true,
                    "artifact_registry_lineage": true,
                    "artifact_registry_persistence": self.config.artifact_state_path.is_some(),
                    "ci_provider_evidence_registry": true,
                    "ci_provider_evidence_lineage": true,
                    "ci_provider_evidence_persistence": self.config.ci_provider_evidence_state_path.is_some(),
                    "domain_report_projection": true,
                    "domain_report_coverage": true,
                    "domain_evidence_harmonization": true,
                    "domain_evidence_harmonization_coverage": true,
                    "domain_evidence_lineage": true,
                    "domain_evidence_intake": true,
                    "domain_evidence_source_plan": true,
                    "domain_evidence_source_execute": true,
                    "domain_evidence_coverage": true,
                    "domain_decision_readiness_query": true,
                    "control_plane_readiness_audit": true,
                    "control_plane_readiness_compare": true,
                    "control_plane_readiness_compare_retained": true,
                    "control_plane_readiness_query": true,
                    "capability_dashboard": true,
                    "capability_route": true,
                    "capability_route_review": true,
                    "capability_route_plan": true,
                    "capability_route_plan_verify": true,
                    "recovery_matrix": true,
                    "operations_snapshot": true,
                    "domain_coverage": true,
                    "operations_domains": true,
                    "operations_gates": true,
                    "operations_gate_reviews": true,
                    "operations_handoff": true,
                    "domain_workflow_catalogue": true,
                    "domain_workflow_scaffold": true,
                    "domain_workflow_instantiate": true,
                    "domain_workflow_portfolio": true,
                    "domain_workflow_portfolio_verify": true,
                    "developer_workbench_verify": true,
                    "developer_workbench_report_registry": true,
                    "developer_workbench_report_persistence": self.config.workbench_state_path.is_some(),
                    "domain_workflow_verify": true,
                    "domain_workflow_reconcile": true,
                    "domain_workflow_reconciliation_registry": true,
                    "domain_workflow_reconciliation_persistence": self.config.reconciliation_state_path.is_some(),
                    "max_mission_trace_events": MAX_MISSION_TRACE_EVENTS,
                    "cooperative_mission_cancellation": true,
                    "durable_mission_snapshots": self.config.mission_state_path.is_some(),
                    "durable_mission_queue_snapshots": self.config.mission_queue_state_path.is_some(),
                    "durable_event_snapshots": self.config.event_state_path.is_some(),
                    "signed_webhook_outbox": true,
                    "delivery_failure_inspection": true,
                    "bounded_delivery_replay": true,
                    "delivery_attempt_provenance": true,
                    "restart_aware_webhook_metadata": true,
                    "explicit_secret_rebind": true,
                    "grpc": false,
                    "tls": false,
                    "external_delivery_worker": false
                },
                "limits": {
                    "max_header_bytes": self.config.max_header_bytes,
                    "max_body_bytes": self.config.max_body_bytes,
                    "event_capacity": self.config.event_capacity,
                    "mission_state_file_bytes": MAX_MISSION_STATE_FILE_BYTES,
                    "persisted_mission_result_bytes": MAX_PERSISTED_MISSION_RESULT_BYTES,
                    "persisted_mission_provenance_bytes": MAX_PERSISTED_MISSION_PROVENANCE_BYTES,
                    "event_state_file_bytes": MAX_EVENT_STATE_FILE_BYTES,
                    "evidence_registry_file_bytes": MAX_EVIDENCE_REGISTRY_BYTES,
                    "evidence_registry_max_bundles": bioprism_devplat::MAX_EVIDENCE_REGISTRY_BUNDLES,
                    "evidence_registry_max_query_items": bioprism_devplat::MAX_EVIDENCE_REGISTRY_QUERY_ITEMS,
                    "workflow_reconciliation_file_bytes": MAX_WORKFLOW_RECONCILIATION_STATE_BYTES,
                    "workflow_reconciliation_max_records": bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
                    "workflow_reconciliation_max_query_items": bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS,
                    "artifact_registry_file_bytes": MAX_ARTIFACT_REGISTRY_BYTES,
                    "artifact_registry_max_records": bioprism_devplat::MAX_ARTIFACT_REGISTRY_RECORDS,
                    "artifact_registry_max_query_items": bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS,
                    "ci_provider_evidence_registry_file_bytes": MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES,
                    "ci_provider_evidence_max_records": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_RECORDS,
                    "ci_provider_evidence_max_query_items": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS,
                    "delivery_error_bytes": crate::events::MAX_DELIVERY_ERROR_BYTES,
                    "webhook_filters": MAX_FILTERS
                }
            }),
        )
    }

    fn tools(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "api_version": API_VERSION,
                "tools": bioprism_mcp::tool_definitions(),
                "call_shape": "POST /v1/tools/{name} with a JSON object body"
            }),
        )
    }

    fn metrics(&self) -> HttpResponse {
        HttpResponse::json(200, &json!({ "ok": true, "metrics": self.event_metrics() }))
    }

    fn events(&self, request: &HttpRequest) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), "query"),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let review_id = query.get("review_id").map(String::as_str);
        let receipt_id = query.get("receipt_id").map(String::as_str);
        if review_id.is_some() && receipt_id.is_some() {
            return self.error(
                400,
                "invalid_query",
                "review_id and receipt_id are mutually exclusive event filters",
                "query",
            );
        }
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    "query",
                )
            }
        };
        let page = match (review_id, receipt_id) {
            (Some(review_id), None) => events.events_for_review(after, limit, review_id),
            (None, Some(receipt_id)) => events.events_for_receipt(after, limit, receipt_id),
            (None, None) => events.events(after, limit),
            (Some(_), Some(_)) => unreachable!("mutually exclusive event filters were checked"),
        };
        match page {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(400, "invalid_query", &error, "query"),
        }
    }

    fn event_stream(&self, request: &HttpRequest) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), "query"),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let review_id = query.get("review_id").map(String::as_str);
        let receipt_id = query.get("receipt_id").map(String::as_str);
        if review_id.is_some() && receipt_id.is_some() {
            return self.error(
                400,
                "invalid_query",
                "review_id and receipt_id are mutually exclusive event filters",
                "query",
            );
        }
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    "query",
                )
            }
        };
        let page = match (review_id, receipt_id) {
            (Some(review_id), None) => events.events_for_review(after, limit, review_id),
            (None, Some(receipt_id)) => events.events_for_receipt(after, limit, receipt_id),
            (None, None) => events.events(after, limit),
            (Some(_), Some(_)) => unreachable!("mutually exclusive event filters were checked"),
        };
        match page {
            Ok(page) => {
                HttpResponse::text(200, "text/event-stream; charset=utf-8", events.sse(&page))
                    .with_header("x-next-after", page.next_after.to_string())
            }
            Err(error) => self.error(400, "invalid_query", &error, "query"),
        }
    }

    fn delivery_receipt_events(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(receipt_id) = delivery_receipt_id(&request.path_segments()) else {
            return self.error(
                404,
                "not_found",
                "delivery-receipt event route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.events_for_receipt(after, limit, &receipt_id) {
            Ok(page) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "workflow": "developer_delivery_receipt_events",
                    "receipt_id": receipt_id,
                    "found": !page.events.is_empty(),
                    "page": page,
                    "guarantees": [
                        "evidence is limited to retained developer delivery receipt events with an exact receipt_id match",
                        "after is an exclusive event cursor and next_after is the last returned event id",
                        "retention gaps are reported instead of silently presented as complete history",
                        "an empty result means no matching retained event was found in the requested cursor window"
                    ]
                }),
            ),
            Err(error) => self.error(400, "invalid_query", &error, request_id),
        }
    }

    fn delivery_receipt_attempts(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(receipt_id) = delivery_receipt_attempts_id(&request.path_segments()) else {
            return self.error(
                404,
                "not_found",
                "delivery-receipt attempt route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.delivery_attempts_for_receipt(&receipt_id, after, limit) {
            Ok(page) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "workflow": "developer_delivery_receipt_attempts",
                    "receipt_id": receipt_id,
                    "found": !page.attempts.is_empty(),
                    "page": page,
                    "guarantees": [
                        "provenance is limited to retained attempt rows with an exact receipt_id match",
                        "receiver acceptance is reported only when the operator-owned sender returned success",
                        "retention gaps are reported instead of silently presented as complete history"
                    ]
                }),
            ),
            Err(error) => self.error(400, "invalid_query", &error, request_id),
        }
    }

    fn route_review_evidence(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(review_id) = route_review_id(&request.path_segments()) else {
            return self.error(
                404,
                "not_found",
                "route-review evidence route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.events_for_review(after, limit, &review_id) {
            Ok(page) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "workflow": "capability_route_review_evidence",
                    "review_id": review_id,
                    "found": !page.events.is_empty(),
                    "page": page,
                    "guarantees": [
                        "evidence is limited to retained capability_route_review tool events with an exact review_id match",
                        "after is an exclusive event cursor and next_after is the last returned event id",
                        "retention gaps are reported instead of silently presented as complete history",
                        "an empty result means no matching retained event was found in the requested cursor window"
                    ]
                }),
            ),
            Err(error) => self.error(400, "invalid_query", &error, request_id),
        }
    }

    fn rpc(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let text = match std::str::from_utf8(&request.body) {
            Ok(text) => text,
            Err(_) => return self.error(400, "invalid_json", "body is not UTF-8", request_id),
        };
        let parsed = match Request::parse(text) {
            Ok(request) => request,
            Err(error) => return HttpResponse::json(400, &error.to_json()),
        };
        if parsed.method == "initialize" {
            return HttpResponse::json(
                200,
                &Response::result(
                    parsed.id.clone(),
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": {
                            "tools": { "listChanged": false },
                            "resources": { "subscribe": false, "listChanged": false }
                        },
                        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                        "instructions": "Use the REST routes for ordinary calls, or continue with JSON-RPC tools/list, tools/call, resources/list, and resources/read."
                    }),
                )
                .to_json(),
            );
        }
        if parsed.is_notification() {
            return HttpResponse::empty(204);
        }
        let method = parsed.method.clone();
        let tool = parsed
            .params
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut server = self.server.clone();
        let Some(response) = server.handle(&parsed) else {
            return HttpResponse::empty(204);
        };
        let wire = response.to_json();
        if method == "tools/call" {
            if let Some(tool) = tool {
                self.record_tool_event(request_id, &tool, &wire);
            }
            // A synchronous MCP call may execute a workflow-bound mission directly rather than
            // through the asynchronous mission worker. The server writes the shared registry;
            // checkpoint it before returning the transport response when durability is enabled.
            let _ = self.reconciliation_persistence.persist();
            let _ = self.artifact_persistence.persist();
            let _ = self.workflow_execution_evidence_persistence.persist();
            let _ = self.ci_provider_evidence_persistence.persist();
        }
        HttpResponse::json(response_status(&wire), &wire)
    }

    fn rest_tool(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 3 || segments[0] != "v1" || segments[1] != "tools" {
            return self.error(404, "not_found", "tool route does not exist", request_id);
        }
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let tool = &segments[2];
        let call = Request {
            id: Some(Value::String(request_id.to_string())),
            method: "tools/call".into(),
            params: json!({ "name": tool, "arguments": arguments }),
        };
        let mut server = self.server.clone();
        let Some(response) = server.handle(&call) else {
            return self.error(
                500,
                "dispatch_failed",
                "tool dispatch produced no response",
                request_id,
            );
        };
        let wire = response.to_json();
        self.record_tool_event(request_id, tool, &wire);
        let _ = self.reconciliation_persistence.persist();
        let _ = self.artifact_persistence.persist();
        let _ = self.workflow_execution_evidence_persistence.persist();
        let _ = self.ci_provider_evidence_persistence.persist();
        let transport_ok = wire.get("error").is_none();
        HttpResponse::json(
            if transport_ok {
                200
            } else {
                response_status(&wire)
            },
            &json!({
                "ok": transport_ok,
                "tool": tool,
                "request_id": request_id,
                "mcp": wire,
                "guarantee": "REST and MCP calls share the same in-process tool dispatcher"
            }),
        )
    }

    fn domain_workflow_catalogue(&self, request_id: &str) -> HttpResponse {
        self.domain_workflow_tool(request_id, "domain_workflow_catalogue", json!({}))
    }

    fn domain_workflow_instantiate(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_instantiate",
            Value::Object(arguments),
        )
    }

    fn domain_workflow_portfolio(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_portfolio",
            Value::Object(arguments),
        )
    }

    fn domain_workflow_portfolio_verify(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let mut arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        // REST adds request_id to every response envelope. It is transport metadata, not part of
        // the retained content-addressed portfolio, so accept a directly round-tripped report.
        if let Some(portfolio) = arguments
            .get_mut("portfolio")
            .and_then(Value::as_object_mut)
        {
            portfolio.remove("request_id");
        }
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_portfolio_verify",
            Value::Object(arguments),
        )
    }

    fn developer_workbench_verify(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "developer_workbench_verify",
            Value::Object(arguments),
        )
    }

    fn import_workbench_report(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let report = match arguments.get("report") {
            Some(report) => report,
            None => return self.error(400, "invalid_json", "report is required", request_id),
        };
        let mut registry = match self.workbench_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "workbench_registry_unavailable",
                    "workbench registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let result = match registry.import(report) {
            Ok(result) => result,
            Err(error) => return self.error(422, "invalid_report", &error.to_string(), request_id),
        };
        if self.config.workbench_state_path.is_some() && result["created"] == true {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "workbench_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.workbench_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(503, "workbench_persistence_unavailable", &error, request_id);
            }
        }
        HttpResponse::json(
            if result["created"].as_bool().unwrap_or(false) {
                201
            } else {
                200
            },
            &result,
        )
    }

    fn query_workbench_reports(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "session_digest"
                    | "domain"
                    | "capability"
                    | "state"
                    | "release_ready"
                    | "after"
                    | "limit"
                    | "include_reports"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "workbench query accepts only session_digest, domain, capability, state, release_ready, after, limit, and include_reports",
                    request_id,
                );
            }
        }
        let session_digest = query.get("session_digest").map(String::as_str);
        let domain = query.get("domain").map(String::as_str);
        let capability = query.get("capability").map(String::as_str);
        let state = query.get("state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=bioprism_devplat::MAX_WORKBENCH_QUERY_ITEMS).contains(&value) => {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let release_ready = match query_bool(&query, "release_ready", false) {
            Ok(value) if query.contains_key("release_ready") => Some(value),
            Ok(_) => None,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_reports = match query_bool(&query, "include_reports", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.workbench_registry.lock() {
            Ok(registry) => registry.query(
                session_digest,
                domain,
                capability,
                state,
                release_ready,
                after,
                max_items,
                include_reports,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "workbench_registry_unavailable",
                    "workbench registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn get_workbench_report(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 4
            || segments[0] != "v1"
            || segments[1] != "developer-workbench"
            || segments[2] != "reports"
        {
            return self.error(
                404,
                "not_found",
                "workbench report route does not exist",
                request_id,
            );
        }
        let digest = &segments[3];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "workbench_report_digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let result = match self.workbench_registry.lock() {
            Ok(registry) => registry.get_response(digest),
            Err(_) => {
                return self.error(
                    500,
                    "workbench_registry_unavailable",
                    "workbench registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(_) => self.error(
                404,
                "not_found",
                "workbench report does not exist",
                request_id,
            ),
        }
    }

    fn workbench_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.workbench_state_path.is_some();
        let file_bytes = self
            .config
            .workbench_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.workbench_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .workbench_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = WorkbenchReportRegistry::from_snapshot(&document).is_ok();
                (digest, Value::Bool(valid))
            })
            .unwrap_or((Value::Null, Value::Null));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema": bioprism_devplat::WORKBENCH_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_reports": bioprism_devplat::MAX_WORKBENCH_REPORTS,
                "max_file_bytes": MAX_WORKBENCH_REGISTRY_STATE_BYTES,
                "recovery_policy": "only structurally valid digest-bound reports restore; retained reports never resume execution",
                "flush": "/v1/developer-workbench/reports/persistence/flush"
            }),
        )
    }

    fn flush_workbench_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.workbench_state_path.is_none() {
            return self.error(
                409,
                "workbench_persistence_disabled",
                "configure workbench_state_path before flushing a workbench report snapshot",
                request_id,
            );
        }
        match self.persist_workbench_registry() {
            Ok(_) => self.workbench_persistence_status(),
            Err(error) => self.error(503, "workbench_persistence_unavailable", &error, request_id),
        }
    }

    fn import_ci_provider_evidence(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mut registry = match self.ci_provider_evidence_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "ci_provider_evidence_registry_unavailable",
                    "CI provider evidence registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let result = match registry.import(&Value::Object(arguments)) {
            Ok(result) => result,
            Err(error) => {
                return self.error(
                    422,
                    "invalid_ci_provider_evidence",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        if self.config.ci_provider_evidence_state_path.is_some() && result["created"] == true {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "ci_provider_evidence_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self
                .ci_provider_evidence_persistence
                .persist_snapshot(&snapshot)
            {
                *registry = before;
                return self.error(
                    503,
                    "ci_provider_evidence_persistence_unavailable",
                    &error,
                    request_id,
                );
            }
        }
        HttpResponse::json(
            if result["created"].as_bool().unwrap_or(false) {
                201
            } else {
                200
            },
            &result,
        )
    }

    fn query_ci_provider_evidence(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "provider"
                    | "run_id"
                    | "plan_digest"
                    | "structurally_valid"
                    | "conformance_ready"
                    | "min_local_byte_hash_artifacts"
                    | "min_local_byte_hash_logs"
                    | "min_attestation_subject_digest_bindings"
                    | "after"
                    | "limit"
                    | "max_items"
                    | "include_records"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "CI provider evidence query accepts only provider, run_id, plan_digest, structurally_valid, conformance_ready, minimum digest-binding thresholds, after, limit or max_items, and include_records",
                    request_id,
                );
            }
        }
        let structurally_valid = match query_bool(&query, "structurally_valid", false) {
            Ok(value) if query.contains_key("structurally_valid") => Some(value),
            Ok(_) => None,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let conformance_ready = match query_bool(&query, "conformance_ready", false) {
            Ok(value) if query.contains_key("conformance_ready") => Some(value),
            Ok(_) => None,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let optional_minimum = |name: &str| -> Result<Option<usize>, String> {
            match query_usize(&query, name, 0) {
                Ok(_) if !query.contains_key(name) => Ok(None),
                Ok(value) if value <= 128 => Ok(Some(value)),
                Ok(_) => Err(format!("{name} must be between 0 and 128")),
                Err(error) => Err(error),
            }
        };
        let min_local_byte_hash_artifacts = match optional_minimum("min_local_byte_hash_artifacts")
        {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let min_local_byte_hash_logs = match optional_minimum("min_local_byte_hash_logs") {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let min_attestation_subject_digest_bindings =
            match optional_minimum("min_attestation_subject_digest_bindings") {
                Ok(value) => value,
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            };
        if query.contains_key("limit") && query.contains_key("max_items") {
            return self.error(
                400,
                "invalid_query",
                "CI provider evidence query accepts either limit or max_items, not both",
                request_id,
            );
        }
        let item_limit_key = if query.contains_key("max_items") {
            "max_items"
        } else {
            "limit"
        };
        let include_records = match query_bool(&query, "include_records", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, item_limit_key, 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS)
                    .contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.ci_provider_evidence_registry.lock() {
            Ok(registry) => registry.query(
                query.get("provider").map(String::as_str),
                query.get("run_id").map(String::as_str),
                query.get("plan_digest").map(String::as_str),
                structurally_valid,
                conformance_ready,
                min_local_byte_hash_artifacts,
                min_local_byte_hash_logs,
                min_attestation_subject_digest_bindings,
                query.get("after").map(String::as_str),
                max_items,
                include_records,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "ci_provider_evidence_registry_unavailable",
                    "CI provider evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn get_ci_provider_evidence(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 4
            || segments[0] != "v1"
            || segments[1] != "ci"
            || segments[2] != "provider-evidence"
        {
            return self.error(
                404,
                "not_found",
                "CI provider evidence route does not exist",
                request_id,
            );
        }
        let digest = &segments[3];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "provider_evidence_digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let result = match self.ci_provider_evidence_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "ci_provider_evidence_registry_unavailable",
                    "CI provider evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(_) => self.error(
                404,
                "not_found",
                "CI provider evidence record does not exist",
                request_id,
            ),
        }
    }

    fn ci_provider_evidence_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.ci_provider_evidence_state_path.is_some();
        let file_bytes = self
            .config
            .ci_provider_evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let (registry_size, generation) = self
            .ci_provider_evidence_registry
            .lock()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .ci_provider_evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = CiProviderEvidenceRegistry::from_snapshot(&document).is_ok();
                (digest, Value::Bool(valid))
            })
            .unwrap_or((Value::Null, Value::Null));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema": bioprism_devplat::CI_PROVIDER_EVIDENCE_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_records": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_RECORDS,
                "max_query_items": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS,
                "max_file_bytes": MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES,
                "recovery_policy": "only re-audited provider evidence records restore; failed and unknown provider runs remain explicit and never resume execution",
                "lineage_policy": "artifact, log, and attestation record digests are retained as provider-observed joins; remote bytes and signatures are not verified",
                "flush": "/v1/ci/provider-evidence/persistence/flush"
            }),
        )
    }

    fn flush_ci_provider_evidence_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.ci_provider_evidence_state_path.is_none() {
            return self.error(
                409,
                "ci_provider_evidence_persistence_disabled",
                "configure ci_provider_evidence_state_path before flushing a CI provider evidence snapshot",
                request_id,
            );
        }
        match self.persist_ci_provider_evidence_registry() {
            Ok(_) => self.ci_provider_evidence_persistence_status(),
            Err(error) => self.error(
                503,
                "ci_provider_evidence_persistence_unavailable",
                &error,
                request_id,
            ),
        }
    }

    fn domain_workflow_verify(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_verify",
            Value::Object(arguments),
        )
    }

    fn domain_workflow_scaffold(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_scaffold",
            Value::Object(arguments),
        )
    }

    fn domain_workflow_reconcile(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_reconcile",
            Value::Object(arguments),
        )
    }

    fn domain_report_project(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_report_project",
            Value::Object(arguments),
        )
    }

    fn domain_report_coverage(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_groups = match query_usize(&query, "max_groups", 64) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_report_digests = match query_bool(&query, "include_report_digests", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("operation".into(), json!("coverage"));
        arguments.insert("max_groups".into(), json!(max_groups));
        arguments.insert(
            "include_report_digests".into(),
            json!(include_report_digests),
        );
        if let Some(group_id) = query.get("group_id") {
            arguments.insert("group_id".into(), json!(group_id));
        }
        if let Some(domain) = query.get("domain") {
            arguments.insert("domain".into(), json!(domain));
        }
        if let Some(report_class) = query.get("report_class") {
            arguments.insert("report_class".into(), json!(report_class));
        }
        if let Some(bridge_mode) = query.get("bridge_mode") {
            arguments.insert("bridge_mode".into(), json!(bridge_mode));
        }
        self.domain_workflow_tool(
            request_id,
            "domain_report_project",
            Value::Object(arguments),
        )
    }

    fn domain_evidence_harmonize(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_harmonize",
            Value::Object(arguments),
        )
    }

    fn domain_evidence_harmonization_coverage(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_items = match query_usize(&query, "max_items", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_report_digests = match query_bool(&query, "include_report_digests", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_items".into(), json!(max_items));
        arguments.insert(
            "include_report_digests".into(),
            json!(include_report_digests),
        );
        for name in [
            "subject_id",
            "domain",
            "report_class",
            "bridge_mode",
            "traceability_state",
            "after",
        ] {
            if let Some(value) = query.get(name) {
                arguments.insert(name.into(), json!(value));
            }
        }
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_harmonization_coverage",
            Value::Object(arguments),
        )
    }

    fn domain_evidence_intake(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_intake",
            Value::Object(arguments),
        )
    }

    fn domain_evidence_lineage(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        const ALLOWED: &[&str] = &[
            "content_digest",
            "group_id",
            "domain",
            "subject_id",
            "source_tool",
            "outcome",
            "request_digest",
            "response_digest",
            "intake_digest",
            "source_plan_digest",
            "after",
            "max_items",
            "include_children",
        ];
        if let Some(unknown) = query.keys().find(|key| !ALLOWED.contains(&key.as_str())) {
            return self.error(
                400,
                "invalid_query",
                &format!("unknown domain evidence lineage query parameter {unknown:?}"),
                request_id,
            );
        }
        let max_items = match query_usize(&query, "max_items", 100) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_children = match query_bool(&query, "include_children", true) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_items".into(), json!(max_items));
        arguments.insert("include_children".into(), json!(include_children));
        for name in [
            "content_digest",
            "group_id",
            "domain",
            "subject_id",
            "source_tool",
            "outcome",
            "request_digest",
            "response_digest",
            "intake_digest",
            "source_plan_digest",
            "after",
        ] {
            if let Some(value) = query.get(name) {
                arguments.insert(name.into(), json!(value));
            }
        }
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.domain_evidence_lineage(&Value::Object(arguments)),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(ArtifactRegistryError::NotFound { .. }) => self.error(
                404,
                "not_found",
                "domain evidence intake does not exist",
                request_id,
            ),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn domain_evidence_source_plan(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_source_plan",
            Value::Object(arguments),
        )
    }

    fn domain_evidence_source_execute(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_source_execute",
            Value::Object(arguments),
        )
    }

    fn domain_evidence_coverage(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_groups = match query_usize(&query, "max_groups", 64) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_intake_digests = match query_bool(&query, "include_intake_digests", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_groups".into(), json!(max_groups));
        arguments.insert(
            "include_intake_digests".into(),
            json!(include_intake_digests),
        );
        if let Some(group_id) = query.get("group_id") {
            arguments.insert("group_id".into(), json!(group_id));
        }
        if let Some(domain) = query.get("domain") {
            arguments.insert("domain".into(), json!(domain));
        }
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_coverage",
            Value::Object(arguments),
        )
    }

    fn capability_dashboard(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_groups = match query_usize(&query, "max_groups", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    400,
                    "invalid_query",
                    "max_groups must be between 1 and 512",
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_tools = match query_bool(&query, "include_tools", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_gaps = match query_bool(&query, "include_gaps", true) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_groups".into(), json!(max_groups));
        arguments.insert("include_tools".into(), json!(include_tools));
        arguments.insert("include_gaps".into(), json!(include_gaps));
        for name in ["group_id", "domain", "status"] {
            if let Some(value) = query.get(name) {
                if value.trim().is_empty() {
                    return self.error(
                        400,
                        "invalid_query",
                        &format!("{name} must be non-empty when supplied"),
                        request_id,
                    );
                }
                arguments.insert(name.into(), json!(value));
            }
        }
        self.domain_workflow_tool(request_id, "capability_dashboard", Value::Object(arguments))
    }

    fn capability_route(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(request_id, "capability_route", Value::Object(arguments))
    }

    fn capability_route_review(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "capability_route_review",
            Value::Object(arguments),
        )
    }

    fn capability_route_plan(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "capability_route_plan",
            Value::Object(arguments),
        )
    }

    fn capability_route_plan_verify(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "capability_route_plan_verify",
            Value::Object(arguments),
        )
    }

    fn import_workflow_reconciliation(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let record = match arguments.get("record") {
            Some(record) => record,
            None => return self.error(422, "invalid_record", "record is required", request_id),
        };
        let mut registry = match self.reconciliation_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let report = match registry.import(record) {
            Ok(report) => report,
            Err(error) => return self.error(422, "invalid_record", &error.to_string(), request_id),
        };
        if self.config.reconciliation_state_path.is_some() {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "reconciliation_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.reconciliation_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(
                    503,
                    "reconciliation_persistence_unavailable",
                    &error,
                    request_id,
                );
            }
        }
        let artifact_projection = self.automatic_artifact_projection(
            "workflow_reconciliation",
            record
                .get("mission_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown-mission"),
            Vec::new(),
            strip_artifact_transport_fields(record),
        );
        let mut report = report;
        report["artifact_registry"] = artifact_projection;
        HttpResponse::json(
            if report["created"].as_bool().unwrap_or(false) {
                201
            } else {
                200
            },
            &report,
        )
    }

    fn query_workflow_reconciliations(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "mission_id"
                    | "workflow_id"
                    | "mission_plan_digest"
                    | "completion_status"
                    | "decision_readiness_state"
                    | "decision_readiness_gate_satisfied"
                    | "after"
                    | "limit"
                    | "include_records"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "workflow reconciliation query accepts only mission_id, workflow_id, mission_plan_digest, completion_status, decision_readiness_state, decision_readiness_gate_satisfied, after, limit, and include_records",
                    request_id,
                );
            }
        }
        let mission_id = query.get("mission_id").map(String::as_str);
        let workflow_id = query.get("workflow_id").map(String::as_str);
        let mission_plan_digest = query.get("mission_plan_digest").map(String::as_str);
        let completion_status = query.get("completion_status").map(String::as_str);
        let decision_readiness_state = query.get("decision_readiness_state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS)
                    .contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_records = match query_bool(&query, "include_records", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let decision_readiness_gate_satisfied = match query
            .get("decision_readiness_gate_satisfied")
            .map(|_| query_bool(&query, "decision_readiness_gate_satisfied", false))
        {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => return self.error(422, "invalid_query", &error, request_id),
            None => None,
        };
        let result = match self.reconciliation_registry.lock() {
            Ok(registry) => registry.query(
                mission_id,
                workflow_id,
                mission_plan_digest,
                completion_status,
                decision_readiness_state,
                decision_readiness_gate_satisfied,
                after,
                max_items,
                include_records,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn get_workflow_reconciliation(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 4
            || segments[0] != "v1"
            || segments[1] != "domain-workflows"
            || segments[2] != "reconciliations"
        {
            return self.error(
                404,
                "not_found",
                "workflow reconciliation route does not exist",
                request_id,
            );
        }
        let digest = &segments[3];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "reconciliation_digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let record = match self.reconciliation_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        match record {
            Some(record) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-api/domain-workflow-reconciliation-record/0.1",
                    "workflow": "domain_workflow_reconciliation_get",
                    "reconciliation_digest": digest,
                    "record": record,
                    "execution": "not_started"
                }),
            ),
            None => self.error(
                404,
                "not_found",
                "workflow reconciliation does not exist",
                request_id,
            ),
        }
    }

    fn reconciliation_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.reconciliation_state_path.is_some();
        let file_bytes = self
            .config
            .reconciliation_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.reconciliation_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .reconciliation_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = DomainWorkflowReconciliationRegistry::from_snapshot(&document).is_ok();
                (digest, Value::Bool(valid))
            })
            .unwrap_or((Value::Null, Value::Null));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema": bioprism_devplat::DOMAIN_WORKFLOW_RECONCILIATION_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_reconciliations": bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
                "max_file_bytes": MAX_WORKFLOW_RECONCILIATION_STATE_BYTES,
                "recovery_policy": "only digest-valid reconciliation reports restore; imported audit records never resume execution",
                "flush": "/v1/domain-workflows/reconciliations/persistence/flush"
            }),
        )
    }

    fn flush_reconciliation_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.reconciliation_state_path.is_none() {
            return self.error(
                409,
                "reconciliation_persistence_disabled",
                "configure --reconciliation-state before flushing a workflow reconciliation snapshot",
                request_id,
            );
        }
        match self.persist_reconciliation_registry() {
            Ok(_) => self.reconciliation_persistence_status(),
            Err(error) => self.error(
                503,
                "reconciliation_persistence_unavailable",
                &error,
                request_id,
            ),
        }
    }

    fn domain_workflow_tool(&self, request_id: &str, tool: &str, arguments: Value) -> HttpResponse {
        let call = Request {
            id: Some(Value::String(request_id.to_string())),
            method: "tools/call".into(),
            params: json!({ "name": tool, "arguments": arguments }),
        };
        let mut server = self.server.clone();
        let Some(response) = server.handle(&call) else {
            return self.error(
                500,
                "dispatch_failed",
                "domain workflow call produced no response",
                request_id,
            );
        };
        let wire = response.to_json();
        self.record_tool_event(request_id, tool, &wire);
        let _ = self.reconciliation_persistence.persist();
        let _ = self.artifact_persistence.persist();
        let _ = self.workflow_execution_evidence_persistence.persist();
        let is_error = wire
            .pointer("/result/isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let payload = wire
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or_else(|| {
                json!({
                    "ok": false,
                    "error": "domain workflow dispatcher returned no structured payload"
                })
            });
        if is_error {
            let message = payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("domain workflow request was refused");
            return self.error(422, "invalid_domain_workflow", message, request_id);
        }
        let mut payload = payload;
        payload["request_id"] = json!(request_id);
        HttpResponse::json(200, &payload)
    }

    fn preflight_mission(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let raw_arguments = Value::Object(arguments.clone());
        if let Err(error) = validate_operations_gate_acceptance(&raw_arguments) {
            return self.error(
                422,
                "invalid_operations_gate_acceptance",
                &error,
                request_id,
            );
        }
        let evidence = self.operations_gate_projection(&raw_arguments);
        let mut report = match self
            .mission_executor
            .preflight_agent_mission(&raw_arguments)
        {
            Ok(report) => report,
            Err(error) => return self.error(422, "invalid_mission", &error, request_id),
        };
        report["request_id"] = json!(request_id);
        report["operations_evidence"] = evidence;
        HttpResponse::json(200, &report)
    }

    fn submit_mission(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mission_id = match arguments.get("mission_id").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() && value.len() <= 256 => value.to_string(),
            _ => {
                return self.error(
                    422,
                    "invalid_mission",
                    "mission_id must be a non-empty string of at most 256 bytes",
                    request_id,
                )
            }
        };
        let arguments = Value::Object(arguments);
        if let Err(error) = validate_operations_gate_acceptance(&arguments) {
            return self.error(
                422,
                "invalid_operations_gate_acceptance",
                &error,
                request_id,
            );
        }
        if let Err(error) = self.mission_executor.validate_agent_mission(&arguments) {
            return self.error(422, "invalid_mission", &error, request_id);
        }
        let mut execution_provenance = None;
        if mission_execution_requested(&arguments) {
            let evidence = self.operations_gate_projection(&arguments);
            if evidence["acceptance_valid"] != json!(true)
                || evidence["decision"] != json!("review_required")
            {
                return self.error(
                    422,
                    "operations_gate_acceptance_required",
                    "execution requires a current operations_gate_acceptance covering every selected domain evidence gate",
                    request_id,
                );
            }
            let Some(provenance) = mission_execution_provenance(&mission_id, &arguments, &evidence)
            else {
                return self.error(
                    500,
                    "execution_provenance_unavailable",
                    "the validated execution acceptance could not be converted into bounded provenance",
                    request_id,
                );
            };
            execution_provenance = Some(provenance);
        }
        let total_steps = arguments
            .get("steps")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

        let mission_already_exists = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.contains_key(&mission_id),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        if mission_already_exists {
            return self.error(
                409,
                "mission_exists",
                "a mission with this mission_id already exists",
                request_id,
            );
        }

        let queue_now = match current_timestamp() {
            Ok(now) => now,
            Err(error) => {
                return self.error(500, "mission_queue_clock_unavailable", &error, request_id)
            }
        };
        let queue_idempotency = if mission_execution_requested(&arguments) {
            FactoryIdempotency::NonIdempotent
        } else {
            FactoryIdempotency::Idempotent
        };
        let queue_job = FactoryJob::new(
            mission_id.clone(),
            ResourceClass::Evaluate,
            queue_idempotency,
            arguments.clone(),
        )
        .with_priority(8);
        let queue_lease = match self
            .mission_queue_persistence
            .enqueue_and_lease(queue_job, queue_now)
        {
            Ok(lease) => lease,
            Err(error) if error.contains("duplicate work") || error.contains("already present") => {
                return self.error(409, "mission_duplicate_work", &error, request_id)
            }
            Err(error) if error.contains("admission limit") => {
                return self.error(429, "mission_queue_backpressure", &error, request_id)
            }
            Err(error) => return self.error(503, "mission_queue_unavailable", &error, request_id),
        };

        let queue_attempt = queue_lease.attempt;
        let route_review_provenance = mission_route_review_provenance(&arguments);
        let cancellation = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(MissionJobState {
            total_steps,
            trace: Vec::new(),
            progress: MissionProgressState::new(total_steps),
            status: "queued".into(),
            cancel_requested: false,
            cancel_reason: None,
            result: None,
            result_omitted: None,
            evaluator_replay_summary: None,
            route_review_provenance: route_review_provenance.clone(),
            error: None,
            recovered_after_restart: false,
            execution_provenance: execution_provenance.clone(),
        }));
        let job = Arc::new(MissionJob {
            cancellation: Arc::clone(&cancellation),
            state: Arc::clone(&state),
        });
        {
            let mut jobs = match self.mission_jobs.lock() {
                Ok(jobs) => jobs,
                Err(_) => {
                    return self.error(
                        500,
                        "mission_registry_unavailable",
                        "mission job registry is unavailable",
                        request_id,
                    )
                }
            };
            if jobs.contains_key(&mission_id) {
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "mission id already exists");
                return self.error(
                    409,
                    "mission_exists",
                    "a mission with this mission_id already exists",
                    request_id,
                );
            }
            if jobs.len() >= MAX_MISSION_JOBS {
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "mission registry capacity exhausted");
                return self.error(
                    429,
                    "mission_capacity_exhausted",
                    "the in-memory mission registry has reached its safety bound",
                    request_id,
                );
            }
            jobs.insert(mission_id.clone(), Arc::clone(&job));
        }
        if let Err(error) = self.persist_mission_registry() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.remove(&mission_id);
            }
            let _ = self
                .mission_queue_persistence
                .cancel(&mission_id, "mission checkpoint unavailable");
            return self.error(503, "mission_persistence_unavailable", &error, request_id);
        }

        if let Some(provenance) = execution_provenance.as_mut() {
            let event = {
                let mut events = match self.events.lock() {
                    Ok(events) => events,
                    Err(_) => {
                        if let Ok(mut jobs) = self.mission_jobs.lock() {
                            jobs.remove(&mission_id);
                        }
                        let _ = self.persist_mission_registry();
                        let _ = self
                            .mission_queue_persistence
                            .cancel(&mission_id, "event log unavailable");
                        return self.error(
                            500,
                            "event_log_unavailable",
                            "event log is unavailable",
                            request_id,
                        );
                    }
                };
                match events.emit(
                    "mission.execution.accepted",
                    &mission_id,
                    request_id,
                    json!({
                        "workflow": "mission_execution",
                        "schema": "bioprism-mission-execution-provenance/0.1",
                        "mission_id": mission_id,
                        "provenance": provenance,
                        "readiness_claimed": false
                    }),
                ) {
                    Ok(event) => event,
                    Err(error) => {
                        if let Ok(mut jobs) = self.mission_jobs.lock() {
                            jobs.remove(&mission_id);
                        }
                        let _ = self.persist_mission_registry();
                        let _ = self
                            .mission_queue_persistence
                            .cancel(&mission_id, "mission acceptance event failed");
                        return self.error(500, "event_emit_failed", &error, request_id);
                    }
                }
            };
            if let Err(error) = self.event_persistence.persist() {
                if let Ok(mut jobs) = self.mission_jobs.lock() {
                    jobs.remove(&mission_id);
                }
                let _ = self.persist_mission_registry();
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "event checkpoint unavailable");
                return self.error(503, "event_persistence_unavailable", &error, request_id);
            }
            provenance["accepted_event_id"] = json!(event.id);
            if let Ok(mut current) = state.lock() {
                current.execution_provenance = Some(provenance.clone());
            }
            if let Err(error) = self.persist_mission_registry() {
                if let Ok(mut jobs) = self.mission_jobs.lock() {
                    jobs.remove(&mission_id);
                }
                let _ = self.persist_mission_registry();
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "mission checkpoint unavailable");
                return self.error(503, "mission_persistence_unavailable", &error, request_id);
            }
        }

        let progress_state = Arc::clone(&state);
        let mission_events = Arc::clone(&self.events);
        let persistence = Arc::clone(&self.mission_persistence);
        let event_persistence = Arc::clone(&self.event_persistence);
        let mission_queue_persistence = Arc::clone(&self.mission_queue_persistence);
        let mission_subject = mission_id.clone();
        let mission_request_id = request_id.to_string();
        let observer = Arc::new(move |event: Value| {
            if let Ok(mut current) = progress_state.lock() {
                current.record_trace(event.clone());
            }
            if let Ok(now) = current_timestamp() {
                let _ = mission_queue_persistence.heartbeat(&mission_subject, queue_attempt, now);
            }
            let _ = persistence.persist();
            if let Ok(mut events) = mission_events.lock() {
                let _ = events.emit(
                    "mission.trace",
                    &mission_subject,
                    &mission_request_id,
                    json!({ "mission_id": mission_subject.clone(), "trace": event }),
                );
            }
            let _ = event_persistence.persist();
        });
        let executor = Arc::new(self.mission_executor.with_mission_trace_observer(observer));
        let worker_persistence = Arc::clone(&self.mission_persistence);
        let worker_reconciliation_persistence = Arc::clone(&self.reconciliation_persistence);
        let worker_artifact_persistence = Arc::clone(&self.artifact_persistence);
        let worker_workflow_execution_evidence_persistence =
            Arc::clone(&self.workflow_execution_evidence_persistence);
        let worker_queue_persistence = Arc::clone(&self.mission_queue_persistence);
        let worker_id = mission_id.clone();
        let worker_mission_id = mission_id.clone();
        let worker_arguments = arguments;
        let spawn = thread::Builder::new()
            .name(format!("mission-{worker_id}"))
            .spawn(move || {
                if let Ok(mut current) = state.lock() {
                    current.status = "running".into();
                    current.progress.phase = "running".into();
                }
                let _ = worker_persistence.persist();
                let outcome = executor
                    .execute_agent_mission_with_cancellation(&worker_arguments, &cancellation);
                // The MCP executor has already imported any workflow reconciliation. Checkpoint
                // it before publishing the terminal mission state to the in-memory job registry.
                let _ = worker_reconciliation_persistence.persist();
                let _ = worker_artifact_persistence.persist();
                let _ = worker_workflow_execution_evidence_persistence.persist();
                if let Ok(mut current) = job.state.lock() {
                    match outcome {
                        Ok(result) => {
                            let queue_commit = current_timestamp().and_then(|now| {
                                worker_queue_persistence
                                    .commit_success(
                                        &worker_mission_id,
                                        queue_attempt,
                                        result.clone(),
                                        now,
                                    )
                            });
                            if let Err(queue_error) = queue_commit {
                                current.status = "failed".into();
                                current.progress.phase = "failed".into();
                                current.progress.active_steps = 0;
                                current.error = Some(format!(
                                    "mission produced a report but the durable execution lease could not commit: {queue_error}"
                                ));
                                current.result = Some(result);
                                current.result_omitted = None;
                            } else {
                                current.progress.reconcile(&result);
                                current.status = result
                                    .get("mission_status")
                                    .and_then(Value::as_str)
                                    .unwrap_or("succeeded")
                                    .into();
                                current.evaluator_replay_summary =
                                    evaluator_replay_summary(&result, &worker_mission_id);
                                current.result = Some(result);
                                current.result_omitted = None;
                            }
                        }
                        Err(error) => {
                            let queue_error = current_timestamp()
                                .and_then(|now| {
                                    worker_queue_persistence.record_failure(
                                        &worker_mission_id,
                                        queue_attempt,
                                        error.clone(),
                                        now,
                                    )
                                })
                                .err();
                            current.status = "failed".into();
                            current.progress.phase = "failed".into();
                            current.progress.active_steps = 0;
                            current.error = Some(match queue_error {
                                Some(queue_error) => format!("{error}; queue transition failed: {queue_error}"),
                                None => error,
                            });
                        }
                    }
                }
                let _ = worker_persistence.persist();
            });
        if spawn.is_err() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.remove(&mission_id);
            }
            let _ = self.persist_mission_registry();
            let _ = self
                .mission_queue_persistence
                .cancel(&mission_id, "mission worker could not be started");
            return self.error(
                503,
                "mission_worker_unavailable",
                "the mission worker could not be started",
                request_id,
            );
        }

        HttpResponse::json(
            202,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": "queued",
                "queue": {
                    "state": "leased",
                    "attempt": queue_lease.attempt,
                    "worker_id": queue_lease.worker_id,
                    "expires_at": queue_lease.expires_at,
                    "automatic_resume": false
                },
                "cancel_requested": false,
                "progress": mission_progress_json(&MissionProgressState::new(total_steps)),
                "route_review_provenance": route_review_provenance,
                "execution_provenance": execution_provenance,
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
                "guarantees": [
                    "mission validation completed before acceptance",
                    "execution is cooperative and preserves the authoritative mission report",
                    "in-flight nested tool calls are allowed to return before future dispatch stops",
                ],
            }),
        )
    }

    fn mission_inventory(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "limit" && key != "status" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission inventory accepts only limit and status",
                    request_id,
                );
            }
        }
        let limit = match query.get("limit") {
            None => 100,
            Some(value) => match value.parse::<usize>() {
                Ok(value) if (1..=MAX_MISSION_LIST_LIMIT).contains(&value) => value,
                _ => {
                    return self.error(
                        422,
                        "invalid_query",
                        &format!("limit must be between 1 and {MAX_MISSION_LIST_LIMIT}"),
                        request_id,
                    )
                }
            },
        };
        let status_filter = match query.get("status") {
            None => None,
            Some(status) if is_known_mission_status(status) => Some(status.as_str()),
            Some(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "status is not a recognized mission status",
                    request_id,
                )
            }
        };
        let jobs = match self.mission_jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let mut entries = Vec::new();
        for (mission_id, job) in jobs.iter() {
            let state = match job_state(job) {
                Ok(state) => state,
                Err(_) => {
                    return self.error(
                        500,
                        "mission_state_unavailable",
                        "mission state is unavailable",
                        request_id,
                    )
                }
            };
            if status_filter.is_some_and(|status| status != state.status) {
                continue;
            }
            let queue = match self.mission_queue_persistence.projection(mission_id) {
                Ok(queue) => queue,
                Err(_) => {
                    return self.error(
                        500,
                        "mission_queue_unavailable",
                        "mission queue is unavailable",
                        request_id,
                    )
                }
            };
            entries.push(json!({
                "mission_id": mission_id,
                "status": state.status,
                "cancel_requested": state.cancel_requested,
                "cancel_reason": state.cancel_reason,
                "recovered_after_restart": state.recovered_after_restart,
                "route_review_provenance": state.route_review_provenance,
                "execution_provenance": state.execution_provenance,
                "queue": queue,
                "progress": mission_progress_json(&state.progress),
                "summary": mission_summary(&state),
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
            }));
        }
        let total = entries.len();
        let missions = entries.into_iter().take(limit).collect::<Vec<_>>();
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "missions": missions,
                "returned": total.min(limit),
                "total_matching": total,
                "limit": limit,
                "truncated": total > limit,
                "status_filter": status_filter,
                "guarantees": [
                    "inventory order is deterministic by mission_id",
                    "inventory entries expose summaries and links, not unbounded terminal reports",
                    "status filters are evaluated against the process-local authoritative registry"
                ]
            }),
        )
    }

    fn mission_status(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), None) else {
            return self.error(404, "not_found", "mission route does not exist", request_id);
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let current = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let queue = match self.mission_queue_persistence.projection(&mission_id) {
            Ok(queue) => queue,
            Err(error) => return self.error(500, "mission_queue_unavailable", &error, request_id),
        };
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": current.status,
                "cancel_requested": current.cancel_requested,
                "cancel_reason": current.cancel_reason,
                "recovered_after_restart": current.recovered_after_restart,
                "execution_provenance": current.execution_provenance,
                "queue": queue,
                "progress": mission_progress_json(&current.progress),
                "route_review_provenance": current.route_review_provenance,
                "result": current.result,
                "result_omitted": current.result_omitted,
                "error": current.error,
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
                "evaluator_replay": format!("/v1/missions/{mission_id}/evaluator-replay"),
                "evaluator_replay_compare": format!("/v1/missions/{mission_id}/evaluator-replay/compare"),
                "evidence_bundle": format!("/v1/missions/{mission_id}/evidence-bundle"),
            }),
        )
    }

    fn mission_provenance(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("provenance")) else {
            return self.error(
                404,
                "not_found",
                "mission provenance route does not exist",
                request_id,
            );
        };
        if request
            .query()
            .map(|query| !query.is_empty())
            .unwrap_or(true)
        {
            return self.error(
                400,
                "invalid_query",
                "mission provenance does not accept query parameters",
                request_id,
            );
        }
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let Some(provenance) = state.execution_provenance else {
            return self.error(
                404,
                "provenance_unavailable",
                "mission has no execution provenance because it was preview-only or predates this contract",
                request_id,
            );
        };
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-mission-execution-provenance/0.1",
                "mission_id": mission_id,
                "provenance": provenance,
                "readiness_claimed": false,
                "guarantees": [
                    "the projection is retained with the mission checkpoint when mission_state_path is configured",
                    "review, gate digest, evaluator binding, and accepted-dispatch event identifiers remain correlated",
                    "the underlying gate review and event routes remain authoritative for replay"
                ],
                "non_claims": [
                    "preview-only missions have no dispatch provenance",
                    "provenance is not scientific, clinical, regulatory, or deployment approval"
                ],
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "mission_trace": format!("/v1/missions/{mission_id}/trace"),
                    "operations_gates": "/v1/operations/gates?after=0&limit=256",
                    "events": "/v1/events?after=0&limit=256"
                }
            }),
        )
    }

    fn mission_evaluator_replay_compare(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(mission_id) =
            mission_id_nested(&request.path_segments(), "evaluator-replay", "compare")
        else {
            return self.error(
                404,
                "not_found",
                "mission evaluator replay comparison route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "include_fixtures" && key != "max_items" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission evaluator replay comparison accepts only include_fixtures and max_items",
                    request_id,
                );
            }
        }
        let include_fixtures = match query_bool(&query, "include_fixtures", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, "max_items", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) | Err(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "max_items must be between 1 and 512",
                    request_id,
                )
            }
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let query_value = json!({
            "include_fixtures": include_fixtures,
            "max_items": max_items
        });
        let catalogue = MissionEvaluatorCatalogue::standard();
        let comparison = if let Some(result) = state.result.clone() {
            catalogue.compare(&MissionEvaluatorReplayCompareRequest {
                mission: result,
                include_fixtures,
                max_items,
            })
        } else if let Some(summary) = state.evaluator_replay_summary.clone() {
            catalogue.compare_summary(&summary)
        } else if let Some(omitted) = state.result_omitted.clone() {
            return self.error(
                410,
                "mission_evaluator_replay_omitted",
                &format!(
                    "mission result and evaluator replay summary were omitted from the bounded registry snapshot ({} bytes, sha256 {})",
                    omitted["bytes"], omitted["sha256"]
                ),
                request_id,
            );
        } else {
            return self.error(
                409,
                "evaluator_replay_unavailable",
                "mission evaluator replay comparison is available after a terminal mission report is retained",
                request_id,
            );
        };
        let comparison = match comparison {
            Ok(comparison) => comparison,
            Err(error) => {
                return self.error(
                    422,
                    "evaluator_replay_comparison_invalid",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        let result_retained = state.result.is_some();
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-api/mission-evaluator-replay-compare/0.1",
                "workflow": "mission_evaluator_replay_compare",
                "mission_id": mission_id,
                "query": query_value,
                "retention": {
                    "mode": if result_retained { "full" } else { "summary_only" },
                    "result_retained": result_retained,
                    "summary_retained": state.evaluator_replay_summary.is_some(),
                    "result_omitted": state.result_omitted.clone()
                },
                "replay": comparison["replay"].clone(),
                "catalog_drift": comparison["catalog_drift"].clone(),
                "execution": "not_started",
                "guarantees": comparison["guarantees"].clone(),
                "limitations": comparison["limitations"].clone(),
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "replay": format!("/v1/missions/{mission_id}/evaluator-replay"),
                    "compare": format!("/v1/missions/{mission_id}/evaluator-replay/compare"),
                    "evidence_bundle": format!("/v1/missions/{mission_id}/evidence-bundle")
                }
            }),
        )
    }

    fn verify_evidence_bundle(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(bundle) = arguments.get("bundle") else {
            return self.error(
                422,
                "invalid_evidence_bundle",
                "request body must contain a bundle object",
                request_id,
            );
        };
        match verify_mission_evidence_bundle(bundle) {
            Ok(value) => HttpResponse::json(200, &value),
            Err(EvidenceBundleError::TooLarge { actual, maximum }) => self.error(
                413,
                "evidence_bundle_too_large",
                &format!("bundle is {actual} bytes; maximum is {maximum} bytes"),
                request_id,
            ),
            Err(error) => self.error(
                422,
                "invalid_evidence_bundle",
                &error.to_string(),
                request_id,
            ),
        }
    }

    fn import_evidence_bundle(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(bundle) = arguments.get("bundle") else {
            return self.error(
                422,
                "invalid_evidence_bundle",
                "request body must contain a bundle object",
                request_id,
            );
        };
        let mut registry = match self.evidence_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "evidence_registry_unavailable",
                    "evidence registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let report = registry.import(bundle);
        let report = match report {
            Ok(report) => report,
            Err(EvidenceRegistryError::Full { maximum }) => {
                return self.error(
                    413,
                    "evidence_registry_full",
                    &format!("evidence registry has reached its {maximum}-bundle limit"),
                    request_id,
                )
            }
            Err(EvidenceRegistryError::Verification(reason)) => {
                return self.error(422, "evidence_bundle_not_verified", &reason, request_id)
            }
            Err(error) => {
                return self.error(
                    422,
                    "invalid_evidence_bundle",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        let created = report
            .get("created")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if created {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "evidence_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.evidence_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(503, "evidence_persistence_unavailable", &error, request_id);
            }
        }
        let artifact_projection = self.automatic_artifact_projection(
            "mission_evidence_bundle",
            bundle
                .get("mission_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown-mission"),
            evaluator_domains_for_artifact(bundle),
            bundle.clone(),
        );
        let mut report = report;
        report["artifact_registry"] = artifact_projection;
        let status = if created { 201 } else { 200 };
        HttpResponse::json(status, &report)
    }

    fn query_evidence_bundles(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "mission_id" | "domain" | "after" | "limit" | "include_bundles"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "evidence bundle query accepts only mission_id, domain, after, limit, and include_bundles",
                    request_id,
                );
            }
        }
        let mission_id = query.get("mission_id").map(String::as_str);
        let domain = query.get("domain").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_EVIDENCE_REGISTRY_QUERY_ITEMS).contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_bundles = match query_bool(&query, "include_bundles", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.evidence_registry.lock() {
            Ok(registry) => registry.query(mission_id, domain, after, max_items, include_bundles),
            Err(_) => {
                return self.error(
                    500,
                    "evidence_registry_unavailable",
                    "evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn get_evidence_bundle(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 3 || segments[0] != "v1" || segments[1] != "evidence-bundles" {
            return self.error(
                404,
                "not_found",
                "evidence bundle route does not exist",
                request_id,
            );
        }
        let digest = &segments[2];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "bundle digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let bundle = match self.evidence_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "evidence_registry_unavailable",
                    "evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match bundle {
            Some(bundle) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-api/evidence-bundle-record/0.1",
                    "workflow": "mission_evidence_bundle_get",
                    "bundle_digest": digest,
                    "bundle": bundle,
                    "execution": "not_started"
                }),
            ),
            None => self.error(
                404,
                "not_found",
                "verified evidence bundle does not exist",
                request_id,
            ),
        }
    }

    fn evidence_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.evidence_state_path.is_some();
        let file_bytes = self
            .config
            .evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.evidence_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = EvidenceBundleRegistry::from_snapshot(&document).is_ok();
                (digest, Value::Bool(valid))
            })
            .unwrap_or((Value::Null, Value::Null));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema": bioprism_devplat::EVIDENCE_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_bundles": bioprism_devplat::MAX_EVIDENCE_REGISTRY_BUNDLES,
                "max_file_bytes": MAX_EVIDENCE_REGISTRY_BYTES,
                "recovery_policy": "only independently verified bundles restore; imported evidence never resumes execution",
                "flush": "/v1/evidence-bundles/persistence/flush"
            }),
        )
    }

    fn flush_evidence_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.evidence_state_path.is_none() {
            return self.error(
                409,
                "evidence_persistence_disabled",
                "configure --evidence-state before flushing an evidence registry snapshot",
                request_id,
            );
        }
        match self.persist_evidence_registry() {
            Ok(_) => self.evidence_persistence_status(),
            Err(error) => self.error(503, "evidence_persistence_unavailable", &error, request_id),
        }
    }

    /// Register a projection produced by an explicit verification boundary and report checkpoint
    /// health separately from in-memory registration. The source registry operation remains
    /// successful when this auxiliary projection is unavailable; the response makes that gap
    /// visible instead of presenting the cross-domain index as complete.
    fn automatic_artifact_projection(
        &self,
        kind: &str,
        subject_id: &str,
        domains: Vec<String>,
        artifact: Value,
    ) -> Value {
        let registration = json!({
            "kind": kind,
            "subject_id": subject_id,
            "domains": domains,
            "parent_digests": [],
            "artifact": artifact,
        });
        let result = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry is unavailable".to_string())
            .and_then(|mut registry| {
                registry
                    .register(&registration)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(report) => {
                let checkpoint = self.artifact_persistence.persist();
                let digest = report.get("content_digest").cloned().unwrap_or(Value::Null);
                json!({
                    "indexed": true,
                    "kind": kind,
                    "subject_id": subject_id,
                    "content_digest": digest,
                    "created": report.get("created").cloned().unwrap_or(Value::Null),
                    "already_present": report.get("already_present").cloned().unwrap_or(Value::Null),
                    "verification": report.get("verification").cloned().unwrap_or(Value::Null),
                    "lookup": digest.as_str().map(|value| format!("/v1/artifacts/{value}")).unwrap_or_default(),
                    "persistence": {
                        "enabled": self.config.artifact_state_path.is_some(),
                        "checkpointed": checkpoint.is_ok(),
                        "error": checkpoint.err()
                    },
                    "execution": "not_started",
                    "does_not_claim": [
                        "artifact integrity establishes scientific, clinical, regulatory, publication, or external-effect validity",
                        "automatic indexing establishes causal provenance or external storage authority"
                    ]
                })
            }
            Err(error) => json!({
                "indexed": false,
                "kind": kind,
                "subject_id": subject_id,
                "error": error,
                "persistence": {
                    "enabled": self.config.artifact_state_path.is_some(),
                    "checkpointed": false
                },
                "execution": "not_started",
                "does_not_claim": [
                    "the failed projection means the source record was invalid",
                    "absence from the artifact registry means the source record never existed"
                ]
            }),
        }
    }

    fn register_artifact(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let registration = match self.json_object(request) {
            Ok(arguments) => Value::Object(arguments),
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mut registry = match self.artifact_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let report = match registry.register(&registration) {
            Ok(report) => report,
            Err(ArtifactRegistryError::Full { maximum }) => {
                return self.error(
                    413,
                    "artifact_registry_full",
                    &format!("artifact registry has reached its {maximum}-record limit"),
                    request_id,
                )
            }
            Err(error @ ArtifactRegistryError::ArtifactTooLarge { .. }) => {
                return self.error(413, "artifact_too_large", &error.to_string(), request_id)
            }
            Err(error) => {
                return self.error(
                    422,
                    "invalid_artifact_registration",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        let created = report
            .get("created")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if created {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "artifact_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.artifact_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(503, "artifact_persistence_unavailable", &error, request_id);
            }
        }
        HttpResponse::json(if created { 201 } else { 200 }, &report)
    }

    /// Compare the three bounded local registries by exact digest identity.
    ///
    /// The stores are sampled independently, so the response includes each generation and
    /// checkpoint digest rather than implying a cross-store transaction.
    fn cross_store_artifact_audit(&self, request_id: &str) -> HttpResponse {
        let (artifact_records, artifact_generation, artifact_state_digest) = {
            let registry = match self.artifact_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "artifact_registry_unavailable",
                        "artifact registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("artifact registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.records_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (evidence_digests, evidence_generation, evidence_state_digest) = {
            let registry = match self.evidence_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "evidence_registry_unavailable",
                        "evidence registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("evidence registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (reconciliation_digests, reconciliation_generation, reconciliation_state_digest) = {
            let registry = match self.reconciliation_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "reconciliation_registry_unavailable",
                        "workflow reconciliation registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("workflow reconciliation registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (
            workflow_execution_evidence_digests,
            workflow_execution_evidence_generation,
            workflow_execution_evidence_state_digest,
        ) = {
            let registry = match self.workflow_execution_evidence_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "workflow_execution_evidence_registry_unavailable",
                        "workflow execution evidence registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("workflow execution evidence registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        HttpResponse::json(
            200,
            &build_cross_domain_audit(
                &artifact_records,
                &evidence_digests,
                &reconciliation_digests,
                &workflow_execution_evidence_digests,
                artifact_generation,
                evidence_generation,
                reconciliation_generation,
                workflow_execution_evidence_generation,
                artifact_state_digest,
                evidence_state_digest,
                reconciliation_state_digest,
                workflow_execution_evidence_state_digest,
            ),
        )
    }

    fn query_domain_decision_readiness(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "subject_id"
                    | "decision_state"
                    | "policy_satisfied"
                    | "after"
                    | "limit"
                    | "include_audits"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "decision-readiness query accepts only subject_id, decision_state, policy_satisfied, after, limit, and include_audits",
                    request_id,
                );
            }
        }
        let subject_id = query.get("subject_id").map(String::as_str);
        let decision_state = query.get("decision_state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let policy_satisfied = match query
            .get("policy_satisfied")
            .map(|_| query_bool(&query, "policy_satisfied", false))
        {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => return self.error(422, "invalid_query", &error, request_id),
            None => None,
        };
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_audits = match query_bool(&query, "include_audits", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.domain_decision_readiness_query(
                subject_id,
                decision_state,
                policy_satisfied,
                after,
                max_items,
                include_audits,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn control_plane_readiness_audit(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "control_plane_readiness_audit",
            Value::Object(arguments),
        )
    }

    fn control_plane_readiness_compare(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "control_plane_readiness_compare",
            Value::Object(arguments),
        )
    }

    fn control_plane_readiness_compare_retained(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "control_plane_readiness_compare_retained",
            Value::Object(arguments),
        )
    }

    fn query_control_plane_readiness(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "subject_id"
                    | "control_plane_state"
                    | "policy_satisfied"
                    | "after"
                    | "limit"
                    | "include_audits"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "control-plane readiness query accepts only subject_id, control_plane_state, policy_satisfied, after, limit, and include_audits",
                    request_id,
                );
            }
        }
        let subject_id = query.get("subject_id").map(String::as_str);
        let control_plane_state = query.get("control_plane_state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let policy_satisfied = match query
            .get("policy_satisfied")
            .map(|_| query_bool(&query, "policy_satisfied", false))
        {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => return self.error(422, "invalid_query", &error, request_id),
            None => None,
        };
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_audits = match query_bool(&query, "include_audits", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.control_plane_readiness_query(
                subject_id,
                control_plane_state,
                policy_satisfied,
                after,
                max_items,
                include_audits,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn query_artifacts(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "kind" | "domain" | "subject_id" | "after" | "limit" | "include_artifacts"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "artifact query accepts only kind, domain, subject_id, after, limit, and include_artifacts",
                    request_id,
                );
            }
        }
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_artifacts = match query_bool(&query, "include_artifacts", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.query(
                query.get("kind").map(String::as_str),
                query.get("domain").map(String::as_str),
                query.get("subject_id").map(String::as_str),
                query.get("after").map(String::as_str),
                max_items,
                include_artifacts,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    fn get_artifact(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 3 || segments[0] != "v1" || segments[1] != "artifacts" {
            return self.error(
                404,
                "not_found",
                "artifact route does not exist",
                request_id,
            );
        }
        let digest = &segments[2];
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(ArtifactRegistryError::NotFound { .. }) => {
                self.error(404, "not_found", "artifact does not exist", request_id)
            }
            Err(error) => self.error(422, "invalid_digest", &error.to_string(), request_id),
        }
    }

    fn artifact_lineage(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 4
            || segments[0] != "v1"
            || segments[1] != "artifacts"
            || segments[3] != "lineage"
        {
            return self.error(
                404,
                "not_found",
                "artifact lineage route does not exist",
                request_id,
            );
        }
        let digest = &segments[2];
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.lineage(digest),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(ArtifactRegistryError::NotFound { .. }) => {
                self.error(404, "not_found", "artifact does not exist", request_id)
            }
            Err(error) => self.error(422, "invalid_digest", &error.to_string(), request_id),
        }
    }

    fn artifact_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.artifact_state_path.is_some();
        let file_bytes = self
            .config
            .artifact_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.artifact_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .artifact_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = ArtifactRegistry::from_snapshot(&document).is_ok();
                (digest, Value::Bool(valid))
            })
            .unwrap_or((Value::Null, Value::Null));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema": bioprism_devplat::ARTIFACT_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_records": bioprism_devplat::MAX_ARTIFACT_REGISTRY_RECORDS,
                "max_file_bytes": MAX_ARTIFACT_REGISTRY_BYTES,
                "recovery_policy": "only digest-valid artifact records restore; indexed artifacts never resume execution",
                "flush": "/v1/artifacts/persistence/flush"
            }),
        )
    }

    fn flush_artifact_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.artifact_state_path.is_none() {
            return self.error(
                409,
                "artifact_persistence_disabled",
                "configure --artifact-state before flushing an artifact registry snapshot",
                request_id,
            );
        }
        match self.persist_artifact_registry() {
            Ok(_) => self.artifact_persistence_status(),
            Err(error) => self.error(503, "artifact_persistence_unavailable", &error, request_id),
        }
    }

    fn mission_evidence_bundle(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("evidence-bundle")) else {
            return self.error(
                404,
                "not_found",
                "mission evidence bundle route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "include_result" | "include_trace" | "include_fixtures" | "max_items"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "mission evidence bundle accepts only include_result, include_trace, include_fixtures, and max_items",
                    request_id,
                );
            }
        }
        let include_result = match query_bool(&query, "include_result", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_trace = match query_bool(&query, "include_trace", true) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_fixtures = match query_bool(&query, "include_fixtures", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, "max_items", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) | Err(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "max_items must be between 1 and 512",
                    request_id,
                )
            }
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        if !is_terminal_mission_status(&state.status)
            && state.result.is_none()
            && state.evaluator_replay_summary.is_none()
        {
            return self.error(
                409,
                "evidence_bundle_unavailable",
                "mission evidence bundle is available after a terminal mission report is retained",
                request_id,
            );
        }
        let catalogue = MissionEvaluatorCatalogue::standard();
        let (replay, comparison) = if let Some(result) = state.result.clone() {
            let replay = catalogue
                .replay(&MissionEvaluatorReplayRequest {
                    mission: result.clone(),
                    include_fixtures,
                    max_items,
                })
                .map_err(|error| error.to_string());
            let comparison = catalogue
                .compare(&MissionEvaluatorReplayCompareRequest {
                    mission: result,
                    include_fixtures: false,
                    max_items,
                })
                .map_err(|error| error.to_string());
            (replay, comparison)
        } else if let Some(summary) = state.evaluator_replay_summary.clone() {
            let comparison = catalogue
                .compare_summary(&summary)
                .map_err(|error| error.to_string());
            (Ok(summary), comparison)
        } else {
            (Ok(Value::Null), Ok(Value::Null))
        };
        let replay = match replay {
            Ok(replay) => replay,
            Err(error) => {
                return self.error(422, "evidence_bundle_replay_invalid", &error, request_id)
            }
        };
        let comparison = match comparison {
            Ok(comparison) => comparison,
            Err(error) => {
                return self.error(
                    422,
                    "evidence_bundle_comparison_invalid",
                    &error,
                    request_id,
                )
            }
        };
        let result_digest = state
            .result
            .as_ref()
            .and_then(|result| ContentHash::of_value(result).ok())
            .map(|digest| digest.to_string());
        let mut bundle = json!({
            "schema": "bioprism-api/mission-evidence-bundle/0.1",
            "workflow": "mission_evidence_bundle_export",
            "mission_id": mission_id,
            "retention": {
                "mode": if state.result.is_some() { "full" } else { "summary_only" },
                "result_retained": state.result.is_some(),
                "result_included": include_result && state.result.is_some(),
                "summary_retained": state.evaluator_replay_summary.is_some(),
                "result_omitted": state.result_omitted.clone()
            },
            "mission": {
                "status": state.status,
                "cancel_requested": state.cancel_requested,
                "cancel_reason": state.cancel_reason,
                "recovered_after_restart": state.recovered_after_restart,
                "error": state.error,
                "progress": mission_progress_json(&state.progress)
            },
            "result": if include_result {
                state.result.clone().unwrap_or(Value::Null)
            } else {
                Value::Null
            },
            "result_digest": result_digest,
            "execution_provenance": state.execution_provenance,
            "claim_lineage": state
                .result
                .as_ref()
                .and_then(|result| result.get("claim_lineage"))
                .cloned()
                .unwrap_or(Value::Null),
            "evaluator_replay": replay,
            "catalog_drift": comparison.get("catalog_drift").cloned().unwrap_or(Value::Null),
            "trace": if include_trace {
                json!(state.trace)
            } else {
                json!([])
            },
            "export": {
                "format": "json",
                "include_result": include_result,
                "include_trace": include_trace,
                "trace_included": include_trace,
                "include_fixtures": include_fixtures,
                "max_items": max_items,
                "execution": "not_started",
                "digest_algorithm": "sha256"
            },
            "guarantees": [
                "the bundle is assembled from the bounded local mission registry and does not execute a tool",
                "every included result, replay, trace, provenance, and omission field remains separately inspectable",
                "bundle_digest covers the canonical bundle object before the digest field is added"
            ],
            "limitations": [
                "the bundle is a local evidence export, not a signature or proof of external storage",
                "summary-only bundles cannot include omitted raw mission output or reconstruct historical catalogue rows",
                "included evidence does not establish scientific, clinical, causal, operational, regulatory, or release truth"
            ],
            "links": {
                "mission": format!("/v1/missions/{mission_id}"),
                "claims": format!("/v1/missions/{mission_id}/claims"),
                "replay": format!("/v1/missions/{mission_id}/evaluator-replay"),
                "compare": format!("/v1/missions/{mission_id}/evaluator-replay/compare"),
                "bundle": format!("/v1/missions/{mission_id}/evidence-bundle")
            }
        });
        let bundle_digest = match ContentHash::of_value(&bundle) {
            Ok(digest) => digest.to_string(),
            Err(error) => {
                return self.error(
                    500,
                    "evidence_bundle_digest_failed",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        bundle["bundle_digest"] = Value::String(bundle_digest);
        let bundle_bytes = match serde_json::to_vec(&bundle) {
            Ok(bytes) => bytes,
            Err(error) => {
                return self.error(
                    500,
                    "evidence_bundle_serialize_failed",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        if bundle_bytes.len() > MAX_MISSION_EVIDENCE_BUNDLE_BYTES {
            return self.error(
                413,
                "evidence_bundle_too_large",
                &format!(
                    "evidence bundle is {} bytes; disable include_result or narrow the export below the {}-byte bound",
                    bundle_bytes.len(),
                    MAX_MISSION_EVIDENCE_BUNDLE_BYTES
                ),
                request_id,
            );
        }
        HttpResponse::json(200, &bundle)
    }

    fn mission_evaluator_replay(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("evaluator-replay"))
        else {
            return self.error(
                404,
                "not_found",
                "mission evaluator replay route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "include_fixtures" && key != "max_items" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission evaluator replay accepts only include_fixtures and max_items",
                    request_id,
                );
            }
        }
        let include_fixtures = match query_bool(&query, "include_fixtures", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, "max_items", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) | Err(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "max_items must be between 1 and 512",
                    request_id,
                )
            }
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let query_value = json!({
            "include_fixtures": include_fixtures,
            "max_items": max_items
        });
        let retention = json!({
            "mode": "full",
            "result_retained": state.result.is_some(),
            "summary_retained": state.evaluator_replay_summary.is_some(),
            "result_omitted": state.result_omitted.clone()
        });
        let replay = if let Some(result) = state.result.clone() {
            let replay_request = MissionEvaluatorReplayRequest {
                mission: result,
                include_fixtures,
                max_items,
            };
            match MissionEvaluatorCatalogue::standard().replay(&replay_request) {
                Ok(mut replay) => {
                    if let Some(object) = replay.as_object_mut() {
                        object.insert("source".into(), json!("durable_mission_result"));
                        object.insert("query".into(), query_value.clone());
                    }
                    replay
                }
                Err(error) => {
                    return self.error(
                        422,
                        "evaluator_replay_invalid",
                        &error.to_string(),
                        request_id,
                    )
                }
            }
        } else if let Some(summary) = state.evaluator_replay_summary.clone() {
            return HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-api/mission-evaluator-replay-query/0.1",
                    "workflow": "mission_evaluator_replay_query",
                    "mission_id": mission_id,
                    "query": query_value,
                    "retention": {
                        "mode": "summary_only",
                        "result_retained": false,
                        "summary_retained": true,
                        "result_omitted": state.result_omitted.clone()
                    },
                    "replay": summary,
                    "execution": "not_started",
                    "guarantees": [
                        "the compact evaluator summary is restored from the mission checkpoint",
                        "result omission remains explicit and is never represented as replay success",
                        "no evaluator or domain tool is executed by this query"
                    ],
                    "limitations": [
                        "full bindings, retained outputs, and fixture rows require the original mission result",
                        "summary-only replay cannot revalidate omitted raw output against a changed catalogue"
                    ],
                    "links": {
                        "mission": format!("/v1/missions/{mission_id}"),
                        "claims": format!("/v1/missions/{mission_id}/claims"),
                        "replay": format!("/v1/missions/{mission_id}/evaluator-replay")
                    }
                }),
            );
        } else if let Some(omitted) = state.result_omitted.clone() {
            return self.error(
                410,
                "mission_evaluator_replay_omitted",
                &format!(
                    "mission result and evaluator replay summary were omitted from the bounded registry snapshot ({} bytes, sha256 {})",
                    omitted["bytes"], omitted["sha256"]
                ),
                request_id,
            );
        } else {
            return self.error(
                409,
                "evaluator_replay_unavailable",
                "mission evaluator replay is available after a terminal mission report is retained",
                request_id,
            );
        };
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-api/mission-evaluator-replay-query/0.1",
                "workflow": "mission_evaluator_replay_query",
                "mission_id": mission_id,
                "query": query_value,
                "retention": retention,
                "replay": replay,
                "execution": "not_started",
                "guarantees": [
                    "the replay input is read from the bounded durable mission registry",
                    "full replay rechecks the current evaluator catalogue without dispatch",
                    "the retention mode and omitted-result metadata remain explicit"
                ],
                "limitations": [
                    "replay does not rerun an evaluator or validate domain semantics",
                    "a summary-only response cannot expose omitted raw evaluator output"
                ],
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "claims": format!("/v1/missions/{mission_id}/claims"),
                    "replay": format!("/v1/missions/{mission_id}/evaluator-replay")
                }
            }),
        )
    }

    fn mission_claims(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("claims")) else {
            return self.error(
                404,
                "not_found",
                "mission claim lineage route does not exist",
                request_id,
            );
        };
        if request
            .query()
            .map(|query| !query.is_empty())
            .unwrap_or(true)
        {
            return self.error(
                400,
                "invalid_query",
                "mission claim lineage does not accept query parameters",
                request_id,
            );
        }
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let Some(result) = state.result else {
            if let Some(omitted) = state.result_omitted {
                return self.error(
                    410,
                    "mission_result_omitted",
                    &format!(
                        "mission result was omitted from the bounded registry snapshot ({} bytes, sha256 {})",
                        omitted["bytes"], omitted["sha256"]
                    ),
                    request_id,
                );
            }
            return self.error(
                409,
                "claim_lineage_unavailable",
                "mission claim lineage is available after a terminal report is retained",
                request_id,
            );
        };
        let Some(claim_lineage) = result.get("claim_lineage") else {
            return self.error(
                404,
                "claim_lineage_unavailable",
                "mission report predates the claim lineage contract or did not request claims",
                request_id,
            );
        };
        if !claim_lineage.is_object() {
            return self.error(
                500,
                "invalid_claim_lineage",
                "mission report claim_lineage is not an object",
                request_id,
            );
        }
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-mission-claim-lineage-response/0.1",
                "mission_id": mission_id,
                "claim_lineage": claim_lineage,
                "guarantees": [
                    "claim rows are correlated only to the explicitly requested mission steps",
                    "omitted outputs and non-successful steps remain visible as non-claimable evidence states",
                    "the projection preserves non-claim posture and does not interpret claim truth"
                ],
                "non_claims": [
                    "claimable means the requested evidence was retained, not that the claim is true",
                    "this route is not scientific, clinical, regulatory, or deployment approval"
                ],
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "mission_trace": format!("/v1/missions/{mission_id}/trace"),
                    "execution_provenance": format!("/v1/missions/{mission_id}/provenance")
                }
            }),
        )
    }

    fn mission_trace(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("trace")) else {
            return self.error(
                404,
                "not_found",
                "mission trace route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission trace accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=1000).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    400,
                    "invalid_query",
                    "limit must be between 1 and 1000",
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let jobs = match self.mission_jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = jobs.get(&mission_id).cloned() else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let oldest = state
            .trace
            .first()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64);
        let newest = state
            .trace
            .last()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64);
        let events = state
            .trace
            .iter()
            .filter(|event| {
                event
                    .get("sequence")
                    .and_then(Value::as_u64)
                    .is_some_and(|sequence| sequence >= after)
            })
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_after = events
            .last()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64)
            .map_or(after, |sequence| sequence.saturating_add(1));
        let dropped_events = oldest.map_or(0, |sequence| sequence.saturating_sub(after));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "trace_schema_version": bioprism_mcp::MISSION_TRACE_SCHEMA_VERSION,
                "events": events,
                "after": after,
                "next_after": next_after,
                "oldest": oldest,
                "newest": newest,
                "gap": dropped_events > 0,
                "dropped_events": dropped_events,
                "terminal": is_terminal_mission_status(&state.status),
                "limit": limit,
                "truncated": next_after < newest.map_or(next_after, |sequence| sequence.saturating_add(1)),
                "guarantees": [
                    "events are ordered by the authoritative clock-free mission sequence",
                    "after is an inclusive sequence cursor for the first page and next_after is exclusive",
                    "retention gaps are reported instead of silently presented as complete history"
                ]
            }),
        )
    }

    fn mission_control(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("cancel")) else {
            return self.error(
                404,
                "not_found",
                "mission control route does not exist",
                request_id,
            );
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let reason = if request.body.is_empty() {
            "cancellation requested by API caller".to_string()
        } else {
            let body = match self.json_object(request) {
                Ok(body) => body,
                Err(error) => return self.error(400, "invalid_json", &error, request_id),
            };
            match body.get("reason") {
                None => "cancellation requested by API caller".to_string(),
                Some(Value::String(value)) if !value.trim().is_empty() && value.len() <= 2_048 => {
                    value.clone()
                }
                Some(_) => {
                    return self.error(
                        422,
                        "invalid_cancellation",
                        "reason must be a non-empty string of at most 2048 bytes",
                        request_id,
                    )
                }
            }
        };
        let mut current = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        if is_terminal_mission_status(&current.status) {
            if current.status == "cancelled" {
                return HttpResponse::json(
                    200,
                    &json!({ "ok": true, "mission_id": mission_id, "status": current.status, "cancel_requested": true, "idempotent": true }),
                );
            }
            return self.error(
                409,
                "mission_terminal",
                "mission has already reached a terminal state",
                request_id,
            );
        }
        job.cancellation.store(true, Ordering::Release);
        current.cancel_requested = true;
        current.cancel_reason = Some(reason.clone());
        current.progress.request_cancel();
        let Ok(mut state) = job.state.lock() else {
            return self.error(
                500,
                "mission_state_unavailable",
                "mission state is unavailable",
                request_id,
            );
        };
        state.cancel_requested = current.cancel_requested;
        state.cancel_reason = current.cancel_reason.clone();
        state.progress.request_cancel();
        drop(state);
        if let Err(error) = self.mission_queue_persistence.cancel(&mission_id, &reason) {
            if !error.contains("already terminal") {
                return self.error(503, "mission_queue_unavailable", &error, request_id);
            }
        }
        let _ = self.persist_mission_registry();
        HttpResponse::json(
            202,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": current.status,
                "cancel_requested": true,
                "cancel_reason": current.cancel_reason,
                "progress": mission_progress_json(&current.progress),
                "reason": reason,
                "poll": format!("/v1/missions/{mission_id}"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
            }),
        )
    }

    fn delete_mission(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), None) else {
            return self.error(404, "not_found", "mission route does not exist", request_id);
        };
        let mut jobs = match self.mission_jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = jobs.remove(&mission_id) else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                jobs.insert(mission_id, job);
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                );
            }
        };
        if !is_terminal_mission_status(&state.status) {
            jobs.insert(mission_id, job);
            return self.error(
                409,
                "mission_running",
                "only terminal missions may be removed",
                request_id,
            );
        }
        drop(jobs);
        let queue_projection = match self.mission_queue_persistence.projection(&mission_id) {
            Ok(projection) => projection,
            Err(error) => {
                if let Ok(mut jobs) = self.mission_jobs.lock() {
                    jobs.insert(mission_id.clone(), job);
                }
                return self.error(503, "mission_queue_unavailable", &error, request_id);
            }
        };
        if !queue_projection.is_null() {
            if let Err(error) = self
                .mission_queue_persistence
                .cancel(&mission_id, "mission deleted by operator")
            {
                if !error.contains("already terminal") {
                    if let Ok(mut jobs) = self.mission_jobs.lock() {
                        jobs.insert(mission_id.clone(), job);
                    }
                    return self.error(503, "mission_queue_unavailable", &error, request_id);
                }
            }
        }
        if let Err(error) = self.persist_mission_registry() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.insert(mission_id.clone(), job);
            }
            return self.error(503, "mission_persistence_unavailable", &error, request_id);
        }
        HttpResponse::json(
            200,
            &json!({ "ok": true, "mission_id": mission_id, "deleted": true }),
        )
    }

    fn list_subscriptions(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "subscriptions": self
                    .events
                    .lock()
                    .map(|events| events.subscriptions())
                    .unwrap_or_default(),
                "secret_policy": "secrets are never returned; delivery signatures are computed over the unsigned envelope"
            }),
        )
    }

    fn create_subscription(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let body = match self.json_object(request) {
            Ok(body) => body,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let endpoint = match body.get("endpoint").and_then(Value::as_str) {
            Some(value) => value,
            None => {
                return self.error(
                    422,
                    "invalid_subscription",
                    "endpoint is required",
                    request_id,
                )
            }
        };
        let secret = match body.get("secret").and_then(Value::as_str) {
            Some(value) => value,
            None => {
                return self.error(
                    422,
                    "invalid_subscription",
                    "secret is required",
                    request_id,
                )
            }
        };
        let filters = match body.get("events") {
            None => None,
            Some(Value::Array(values)) => {
                let mut filters = Vec::with_capacity(values.len());
                for value in values {
                    let Some(value) = value.as_str() else {
                        return self.error(
                            422,
                            "invalid_subscription",
                            "events must contain strings",
                            request_id,
                        );
                    };
                    filters.push(value.to_string());
                }
                Some(filters)
            }
            Some(_) => {
                return self.error(
                    422,
                    "invalid_subscription",
                    "events must be an array",
                    request_id,
                )
            }
        };
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.register_subscription(
            body.get("id").and_then(Value::as_str),
            endpoint,
            filters.as_deref(),
            secret,
        ) {
            Ok(subscription) => {
                drop(events);
                let _ = self.event_persistence.persist();
                HttpResponse::json(
                    201,
                    &json!({
                        "ok": true,
                        "subscription": subscription,
                        "delivery": {
                            "mode": "signed_outbox",
                            "poll": "/v1/webhooks/subscriptions/{id}/deliveries",
                            "ack": "/v1/webhooks/subscriptions/{id}/ack",
                            "retry": "/v1/webhooks/subscriptions/{id}/retry",
                            "replay": "/v1/webhooks/subscriptions/{id}/replay",
                            "rebind": "/v1/webhooks/subscriptions/{id}/rebind"
                        }
                    }),
                )
            }
            Err(error) => self.error(422, "invalid_subscription", &error, request_id),
        }
    }

    fn delete_subscription(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), None) else {
            return self.error(
                404,
                "not_found",
                "subscription route does not exist",
                request_id,
            );
        };
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.remove_subscription(&id) {
            Ok(true) => {
                drop(events);
                let _ = self.event_persistence.persist();
                HttpResponse::json(200, &json!({ "ok": true, "deleted": id }))
            }
            Ok(false) => self.error(404, "not_found", "subscription does not exist", request_id),
            Err(error) => self.error(409, "subscription_error", &error, request_id),
        }
    }

    fn rebind_subscription(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), Some("rebind")) else {
            return self.error(
                404,
                "not_found",
                "subscription rebind route does not exist",
                request_id,
            );
        };
        let body = match self.json_object(request) {
            Ok(body) => body,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(secret) = body.get("secret").and_then(Value::as_str) else {
            return self.error(
                422,
                "invalid_subscription_secret",
                "secret is required for an in-memory subscription rebind",
                request_id,
            );
        };
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.rebind_subscription(&id, secret) {
            Ok((subscription, resigned_deliveries)) => {
                drop(events);
                let _ = self.event_persistence.persist();
                HttpResponse::json(
                    200,
                    &json!({
                        "ok": true,
                        "subscription": subscription,
                        "resigned_deliveries": resigned_deliveries,
                        "secret_policy": "the supplied secret is held in memory only and is never returned or persisted"
                    }),
                )
            }
            Err(error) => self.error(404, "subscription_rebind_failed", &error, request_id),
        }
    }

    fn list_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), Some("deliveries")) else {
            return self.error(
                404,
                "not_found",
                "delivery route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.deliveries(&id, after, limit) {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(404, "not_found", &error, request_id),
        }
    }

    fn list_delivery_attempts(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), Some("attempts")) else {
            return self.error(
                404,
                "not_found",
                "delivery attempt route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.delivery_attempts(&id, after, limit) {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(404, "not_found", &error, request_id),
        }
    }

    fn ack_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, false, false)
    }

    fn retry_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, true, false)
    }

    fn replay_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, false, true)
    }

    fn delivery_mutation(
        &self,
        request: &HttpRequest,
        request_id: &str,
        retry: bool,
        replay: bool,
    ) -> HttpResponse {
        let operation = if retry {
            "retry"
        } else if replay {
            "replay"
        } else {
            "ack"
        };
        let Some(id) = subscription_id(&request.path_segments(), Some(operation)) else {
            return self.error(
                404,
                "not_found",
                "delivery route does not exist",
                request_id,
            );
        };
        let body = match self.json_object(request) {
            Ok(body) => body,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(values) = body.get("delivery_ids").and_then(Value::as_array) else {
            return self.error(
                422,
                "invalid_delivery_ids",
                "delivery_ids must be an array",
                request_id,
            );
        };
        let mut ids = Vec::with_capacity(values.len());
        for value in values {
            let Some(id) = value.as_u64() else {
                return self.error(
                    422,
                    "invalid_delivery_ids",
                    "delivery_ids must contain integers",
                    request_id,
                );
            };
            ids.push(id);
        }
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        if retry {
            match events.retry(&id, &ids) {
                Ok(deliveries) => {
                    drop(events);
                    let _ = self.event_persistence.persist();
                    HttpResponse::json(200, &json!({ "ok": true, "retried": deliveries }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        } else if replay {
            match events.replay(&id, &ids) {
                Ok(deliveries) => {
                    drop(events);
                    let _ = self.event_persistence.persist();
                    HttpResponse::json(200, &json!({ "ok": true, "replayed": deliveries }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        } else {
            match events.acknowledge(&id, &ids) {
                Ok(acknowledged) => {
                    drop(events);
                    let _ = self.event_persistence.persist();
                    HttpResponse::json(200, &json!({ "ok": true, "acknowledged": acknowledged }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        }
    }

    fn json_object(&self, request: &HttpRequest) -> Result<serde_json::Map<String, Value>, String> {
        if let Some(content_type) = request.header("content-type") {
            if !content_type
                .split(';')
                .next()
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
            {
                return Err("JSON routes require Content-Type: application/json".into());
            }
        }
        let value: Value = serde_json::from_slice(&request.body)
            .map_err(|error| format!("request body is not valid JSON: {error}"))?;
        value
            .as_object()
            .cloned()
            .ok_or_else(|| "request body must be a JSON object".into())
    }

    fn record_tool_event(&self, request_id: &str, tool: &str, wire: &Value) {
        let outcome = if wire.get("error").is_some() {
            "tool.rpc_error"
        } else if wire
            .get("result")
            .and_then(|result| result.get("isError"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "tool.refused"
        } else {
            "tool.completed"
        };
        let encoded = serde_json::to_vec(wire).unwrap_or_default();
        let delivery_receipt = Self::delivery_receipt_projection(wire);
        let payload = if encoded.len() <= 64 * 1024 {
            let mut payload = json!({ "tool": tool, "response": wire });
            if let Some(projection) = delivery_receipt.clone() {
                payload["delivery_receipt"] = projection;
            }
            payload
        } else {
            let mut projection = json!({
                "tool": tool,
                "response_omitted": true,
                "response_bytes": encoded.len(),
                "response_sha256": hex_digest(&Sha256::digest(&encoded))
            });
            if tool == "agent_mission" {
                if let Some(trace) = wire
                    .pointer("/result/content/0/text")
                    .and_then(Value::as_str)
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                    .and_then(|report| {
                        Some(json!({
                            "execution_trace_schema_version": report.get("execution_trace_schema_version")?,
                            "execution_trace": report.get("execution_trace")?,
                            "mission_status": report.get("mission_status")?,
                            "returned_bytes": report.get("returned_bytes")?,
                        }))
                    })
                {
                    projection["mission_trace"] = trace;
                }
            }
            if let Some(receipt) = delivery_receipt {
                projection["delivery_receipt"] = receipt;
            }
            projection
        };
        if let Ok(mut events) = self.events.lock() {
            let _ = events.emit(outcome, tool, request_id, payload);
        }
        let _ = self.event_persistence.persist();
    }

    /// Keep a small stable join key in the event stream even when the complete receipt response
    /// is omitted by the event-size bound. This is a projection only: the receipt itself remains
    /// content-addressed and must be fetched or supplied separately for verification.
    fn delivery_receipt_projection(wire: &Value) -> Option<Value> {
        let text = wire.pointer("/result/content/0/text")?.as_str()?;
        let output = serde_json::from_str::<Value>(text).ok()?;
        let workflow = output.get("workflow")?.as_str()?;
        let is_receipt = matches!(
            workflow,
            "developer_delivery_receipt" | "developer_delivery_receipt_verify"
        );
        if !is_receipt {
            return None;
        }
        let mut projection = json!({
            "workflow": workflow,
            "receipt_id": output.get("receipt_id")?,
        });
        for field in [
            "receipt_digest",
            "supplied_receipt_digest",
            "recomputed_receipt_digest",
            "valid",
            "verified",
            "receipt_ready",
            "release_candidate",
            "target_count",
            "ready_target_count",
            "ready_evidence_count",
            "receipt_digest_match",
            "targets_match",
            "evidence_match",
        ] {
            if let Some(value) = output.get(field) {
                projection[field] = value.clone();
            }
        }
        Some(projection)
    }

    fn error(&self, status: u16, code: &str, message: &str, request_id: &str) -> HttpResponse {
        HttpResponse::json(
            status,
            &json!({
                "ok": false,
                "error": { "code": code, "message": message },
                "request_id": request_id
            }),
        )
    }

    fn openapi(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "openapi": "3.1.0",
                "info": {
                    "title": "AURORA Prism API",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Bounded REST and JSON-RPC access to the in-process MCP tool kernel, with cursor-based events and a signed webhook outbox."
                },
                "paths": {
                    "/healthz": { "get": { "responses": { "200": { "description": "liveness" } } } },
                    "/readyz": { "get": { "responses": { "200": { "description": "readiness" } } } },
                    "/v1/capabilities": { "get": { "responses": { "200": { "description": "capability and limit catalog" } } } },
                    "/v1/capabilities/dashboard": { "get": { "parameters": [{ "name": "group_id", "in": "query" }, { "name": "domain", "in": "query" }, { "name": "status", "in": "query" }, { "name": "max_groups", "in": "query" }, { "name": "include_tools", "in": "query" }, { "name": "include_gaps", "in": "query" }], "responses": { "200": { "description": "bounded digest-bound cross-domain capability dashboard" }, "400": { "description": "dashboard query was invalid" } } } },
                    "/v1/capabilities/route": { "post": { "responses": { "200": { "description": "bounded non-executing cross-domain capability route proposal" }, "400": { "description": "route request JSON was invalid" }, "422": { "description": "route request was refused" } } } },
                    "/v1/capabilities/route/review": { "post": { "responses": { "200": { "description": "bounded non-executing route review and mission handoff" }, "400": { "description": "route review JSON was invalid" }, "422": { "description": "route review was refused" } } } },
                    "/v1/capabilities/route/plan": { "post": { "responses": { "200": { "description": "bounded route review composed with authoritative non-executing mission preflight" }, "400": { "description": "route plan JSON was invalid" }, "422": { "description": "route plan was refused" } } } },
                    "/v1/capabilities/route/plan/verify": { "post": { "responses": { "200": { "description": "bounded route-plan replay and authoritative mission-preflight verification" }, "400": { "description": "route-plan verification JSON was invalid" }, "422": { "description": "route-plan verification was refused" } } } },
                    "/v1/recovery": { "get": { "responses": { "200": { "description": "operator-visible restart recovery matrix" } } } },
                    "/v1/operations/snapshot": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "bounded operator control-plane snapshot" } } } },
                    "/v1/operations/domains": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "bounded per-domain observed activity projection" } } } },
                    "/v1/operations/gates": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "bounded per-domain evidence gate projection without readiness claims" } } } },
                    "/v1/operations/gate-reviews": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "review_id", "in": "query" }], "responses": { "200": { "description": "durable cursor page of replayable operations gate reviews" } } }, "post": { "responses": { "201": { "description": "content-addressed operations gate review record" } } } },
                    "/v1/operations/handoff": { "post": { "responses": { "200": { "description": "content-addressed, non-executing domain routing handoff" } } } },
                    "/v1/evidence-bundles/verify": { "post": { "responses": { "200": { "description": "content-addressed mission evidence bundle verification report" }, "413": { "description": "bundle exceeds verification bound" }, "422": { "description": "bundle is malformed" } } } },
                    "/v1/domain-workflows": { "get": { "responses": { "200": { "description": "deterministic workflow template for every capability group" } } } },
                    "/v1/domain-workflows/scaffold": { "post": { "responses": { "200": { "description": "deterministic execution-disabled workflow scaffold with authoritative preflight" }, "422": { "description": "workflow selection or scaffold request was refused" } } } },
                    "/v1/domain-workflows/instantiate": { "post": { "responses": { "200": { "description": "group-scoped, authoritative-preflighted, no-dispatch workflow mission" }, "422": { "description": "workflow selection or mission preflight was refused" } } } },
                    "/v1/domain-workflows/portfolio": { "post": { "responses": { "200": { "description": "bounded multi-domain workflow portfolio with per-item authoritative no-dispatch preflight" }, "400": { "description": "workflow portfolio JSON was invalid" }, "422": { "description": "workflow portfolio was refused" } } } },
                    "/v1/domain-workflows/portfolio/verify": { "post": { "responses": { "200": { "description": "retained multi-domain workflow portfolio digest, replay, coverage, and authoritative mission-preflight verification" }, "400": { "description": "workflow portfolio verification JSON was invalid" }, "422": { "description": "workflow portfolio verification was refused" } } } },
                    "/v1/developer-workbench/verify": { "post": { "responses": { "200": { "description": "retained authoring/notebook workbench digest, dashboard, and optional CI-plan replay verification" }, "400": { "description": "developer workbench verification JSON was invalid" }, "422": { "description": "developer workbench verification was refused" } } } },
                    "/v1/ci/provider-evidence": { "get": { "parameters": [{ "name": "provider", "in": "query" }, { "name": "run_id", "in": "query" }, { "name": "plan_digest", "in": "query" }, { "name": "structurally_valid", "in": "query" }, { "name": "conformance_ready", "in": "query" }, { "name": "after", "in": "query" }, { "name": "max_items", "in": "query" }, { "name": "include_records", "in": "query" }], "responses": { "200": { "description": "bounded deterministic provider-observed CI evidence registry query" }, "400": { "description": "provider-evidence query was invalid" } } }, "post": { "responses": { "201": { "description": "provider-evidence audit imported" }, "200": { "description": "idempotent re-import" }, "413": { "description": "registry capacity or snapshot bound exceeded" }, "422": { "description": "provider-evidence audit failed" } } } },
                    "/v1/ci/provider-evidence/{provider_evidence_digest}": { "get": { "parameters": [{ "name": "provider_evidence_digest", "in": "path", "required": true }], "responses": { "200": { "description": "one retained provider-observed CI evidence audit with lineage joins" }, "404": { "description": "provider-evidence digest is not present" } } } },
                    "/v1/ci/provider-evidence/persistence": { "get": { "responses": { "200": { "description": "restart-aware provider-evidence registry checkpoint status" } } } },
                    "/v1/ci/provider-evidence/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded provider-evidence registry checkpoint" }, "409": { "description": "persistence is disabled" } } } },
                    "/v1/domain-workflows/verify": { "post": { "responses": { "200": { "description": "retained domain-workflow replay and authoritative mission-preflight verification" }, "400": { "description": "workflow verification JSON was invalid" }, "422": { "description": "workflow verification was refused" } } } },
                    "/v1/domain-workflows/reconcile": { "post": { "responses": { "200": { "description": "digest-bound workflow execution and evidence reconciliation" }, "422": { "description": "workflow evidence source or contract was refused" } } } },
                    "/v1/domain-workflows/reconciliations": { "get": { "parameters": [{ "name": "mission_id", "in": "query" }, { "name": "workflow_id", "in": "query" }, { "name": "mission_plan_digest", "in": "query" }, { "name": "completion_status", "in": "query" }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "include_records", "in": "query" }], "responses": { "200": { "description": "bounded deterministic workflow reconciliation registry index" } } }, "post": { "responses": { "201": { "description": "digest-valid workflow reconciliation imported" }, "200": { "description": "idempotent re-import" }, "422": { "description": "reconciliation record failed digest validation" } } } },
                    "/v1/domain-workflows/reconciliations/{reconciliation_digest}": { "get": { "parameters": [{ "name": "reconciliation_digest", "in": "path", "required": true }], "responses": { "200": { "description": "one imported workflow reconciliation report" }, "404": { "description": "reconciliation digest is not present" } } } },
                    "/v1/domain-workflows/reconciliations/persistence": { "get": { "responses": { "200": { "description": "restart-aware workflow reconciliation registry checkpoint status" } } } },
                    "/v1/domain-workflows/reconciliations/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded workflow reconciliation registry checkpoint" }, "409": { "description": "persistence is disabled" } } } },
                    "/v1/evidence-bundles": { "get": { "parameters": [{ "name": "mission_id", "in": "query" }, { "name": "domain", "in": "query" }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "include_bundles", "in": "query" }], "responses": { "200": { "description": "bounded deterministic evidence registry index" } } }, "post": { "responses": { "201": { "description": "verified evidence bundle imported into the registry" }, "200": { "description": "idempotent re-import" }, "413": { "description": "registry capacity or snapshot bound exceeded" }, "422": { "description": "bundle verification failed" } } } },
                    "/v1/evidence-bundles/{bundle_digest}": { "get": { "parameters": [{ "name": "bundle_digest", "in": "path", "required": true }], "responses": { "200": { "description": "one verified evidence bundle" }, "404": { "description": "bundle digest is not present" } } } },
                    "/v1/evidence-bundles/persistence": { "get": { "responses": { "200": { "description": "restart-aware evidence registry checkpoint status" } } } },
                    "/v1/evidence-bundles/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded evidence registry checkpoint" }, "409": { "description": "persistence is disabled" } } } },
                    "/v1/domain-decision-readiness": { "get": { "parameters": [{ "name": "subject_id", "in": "query" }, { "name": "decision_state", "in": "query" }, { "name": "policy_satisfied", "in": "query" }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "include_audits", "in": "query" }], "responses": { "200": { "description": "bounded digest-ordered retained decision-readiness posture query" } } } },
                    "/v1/control-plane-readiness": { "get": { "parameters": [{ "name": "subject_id", "in": "query" }, { "name": "control_plane_state", "in": "query" }, { "name": "policy_satisfied", "in": "query" }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "include_audits", "in": "query" }], "responses": { "200": { "description": "bounded digest-ordered retained control-plane readiness query" } } }, "post": { "responses": { "200": { "description": "digest-bound structural control-plane readiness projection" }, "422": { "description": "invalid component evidence or policy" } } } },
                    "/v1/control-plane-readiness/compare": { "post": { "responses": { "200": { "description": "digest-verified structural comparison of two control-plane readiness projections" }, "422": { "description": "invalid or mismatched readiness projections" } } } },
                    "/v1/control-plane-readiness/compare-retained": { "post": { "responses": { "200": { "description": "registry-resolved structural comparison of two retained control-plane readiness artifacts" }, "404": { "description": "one retained content digest is absent" }, "422": { "description": "retained artifacts are invalid, mismatched, or not control-plane readiness records" } } } },
                    "/v1/artifacts": { "get": { "parameters": [{ "name": "kind", "in": "query" }, { "name": "domain", "in": "query" }, { "name": "subject_id", "in": "query" }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "include_artifacts", "in": "query" }], "responses": { "200": { "description": "bounded cross-domain artifact index query" } } }, "post": { "responses": { "201": { "description": "registered content-addressed artifact" }, "200": { "description": "idempotent artifact registration" } } } },
                    "/v1/artifacts/cross-store": { "get": { "responses": { "200": { "description": "digest-only consistency audit across artifact, evidence, and workflow-reconciliation registries" } } } },
                    "/v1/artifacts/{content_digest}": { "get": { "parameters": [{ "name": "content_digest", "in": "path", "required": true }], "responses": { "200": { "description": "one artifact record with verification posture" }, "404": { "description": "artifact digest is not present" } } } },
                    "/v1/artifacts/{content_digest}/lineage": { "get": { "parameters": [{ "name": "content_digest", "in": "path", "required": true }], "responses": { "200": { "description": "bounded parent lineage with explicit missing nodes and cycles" } } } },
                    "/v1/artifacts/persistence": { "get": { "responses": { "200": { "description": "restart-aware artifact registry checkpoint status" } } } },
                    "/v1/artifacts/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded artifact registry checkpoint" }, "409": { "description": "persistence is disabled" } } } },
                    "/v1/tools": { "get": { "responses": { "200": { "description": "MCP tool catalog" } } } },
                    "/v1/tools/{name}": { "post": { "parameters": [{ "name": "name", "in": "path", "required": true }], "responses": { "200": { "description": "tool result" } } } },
                    "/v1/domain-reports": { "post": { "responses": { "200": { "description": "bounded domain-report projection" } } } },
                    "/v1/domain-reports/coverage": { "get": { "responses": { "200": { "description": "domain-report coverage diagnostic" } } } },
                    "/v1/domain-evidence/harmonize": { "post": { "responses": { "200": { "description": "digest-addressed domain evidence harmonization" }, "422": { "description": "report identity, catalogue, or traceability input was refused" } } } },
                    "/v1/domain-evidence/harmonization/coverage": { "get": { "parameters": [{ "name": "subject_id", "in": "query" }, { "name": "domain", "in": "query" }, { "name": "report_class", "in": "query" }, { "name": "bridge_mode", "in": "query" }, { "name": "traceability_state", "in": "query" }, { "name": "after", "in": "query" }, { "name": "max_items", "in": "query" }, { "name": "include_report_digests", "in": "query" }], "responses": { "200": { "description": "bounded retained harmonization coverage" }, "422": { "description": "coverage filter was invalid" } } } },
                    "/v1/domain-evidence/lineage": { "get": { "parameters": [{ "name": "content_digest", "in": "query" }, { "name": "group_id", "in": "query" }, { "name": "domain", "in": "query" }, { "name": "subject_id", "in": "query" }, { "name": "source_tool", "in": "query" }, { "name": "outcome", "in": "query" }, { "name": "request_digest", "in": "query" }, { "name": "response_digest", "in": "query" }, { "name": "intake_digest", "in": "query" }, { "name": "source_plan_digest", "in": "query" }, { "name": "after", "in": "query" }, { "name": "max_items", "in": "query" }, { "name": "include_children", "in": "query" }], "responses": { "200": { "description": "digest-bound retained intake request/response lineage and reverse child links" }, "404": { "description": "requested intake digest is not present" }, "422": { "description": "lineage filter was invalid" } } } },
                    "/v1/domain-evidence/intake": { "post": { "responses": { "200": { "description": "exact-digest raw domain evidence intake" }, "422": { "description": "envelope, outcome, or catalogue input was refused" } } } },
                    "/v1/domain-evidence/sources": { "post": { "responses": { "200": { "description": "digest-addressed, non-fetching external evidence source plan" }, "422": { "description": "source locator, policy, or catalogue input was refused" } } } },
                    "/v1/domain-evidence/sources/execute": { "post": { "responses": { "200": { "description": "bounded source execution with raw-byte and canonical-response digests plus retained intake" }, "422": { "description": "retained plan, connector policy, root confinement, or intake binding was refused" } } } },
                    "/v1/domain-evidence/coverage": { "get": { "responses": { "200": { "description": "catalogue-wide retained domain evidence intake coverage" }, "400": { "description": "coverage filter was invalid" } } } },
                    "/v1/missions/preflight": { "post": { "responses": { "200": { "description": "authoritative no-dispatch mission plan" } } } },
                    "/v1/missions": { "get": { "responses": { "200": { "description": "bounded mission inventory" } } }, "post": { "responses": { "202": { "description": "accepted asynchronous mission" } } } },
                    "/v1/missions/persistence": { "get": { "responses": { "200": { "description": "restart-aware mission snapshot status" } } } },
                    "/v1/missions/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded mission snapshot checkpoint" } } } },
                    "/v1/missions/queue": { "get": { "responses": { "200": { "description": "typed factory mission queue projection and recovery posture" } } } },
                    "/v1/missions/queue/persistence": { "get": { "responses": { "200": { "description": "content-addressed mission queue checkpoint status" } } } },
                    "/v1/missions/queue/persistence/flush": { "post": { "responses": { "200": { "description": "force an atomic mission queue checkpoint" }, "503": { "description": "queue checkpoint unavailable" } } } },
                    "/v1/missions/queue/authority/release-lock": { "post": { "responses": { "200": { "description": "explicitly release and audit an orphaned shared-authority lock" }, "409": { "description": "lock release refused or no orphaned lock exists" } } } },
                    "/v1/missions/{mission_id}": { "get": { "responses": { "200": { "description": "mission status and result" } } }, "delete": { "responses": { "200": { "description": "terminal mission removed" } } } },
                    "/v1/missions/{mission_id}/provenance": { "get": { "responses": { "200": { "description": "retained execution gate, review, evaluator, and dispatch provenance" } } } },
                     "/v1/missions/{mission_id}/claims": { "get": { "responses": { "200": { "description": "bounded claim-to-step evidence lineage projection" }, "409": { "description": "mission is not yet terminal" }, "410": { "description": "terminal report was omitted from the bounded registry" } } } },
                     "/v1/missions/{mission_id}/evaluator-replay": { "get": { "parameters": [{ "name": "include_fixtures", "in": "query" }, { "name": "max_items", "in": "query" }], "responses": { "200": { "description": "durable full or summary-only evaluator replay query" }, "409": { "description": "mission is not yet terminal" }, "410": { "description": "mission result and replay summary were omitted" } } } },
                     "/v1/missions/{mission_id}/evaluator-replay/compare": { "get": { "parameters": [{ "name": "include_fixtures", "in": "query" }, { "name": "max_items", "in": "query" }], "responses": { "200": { "description": "catalog-drift-aware replay comparison" }, "409": { "description": "mission is not yet terminal" }, "410": { "description": "mission result and replay summary were omitted" } } } },
                     "/v1/missions/{mission_id}/evidence-bundle": { "get": { "parameters": [{ "name": "include_result", "in": "query" }, { "name": "include_trace", "in": "query" }, { "name": "include_fixtures", "in": "query" }, { "name": "max_items", "in": "query" }], "responses": { "200": { "description": "bounded content-addressed mission evidence bundle" }, "409": { "description": "mission is not yet terminal" } } } },
                    "/v1/missions/{mission_id}/trace": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "bounded clock-free mission trace page" } } } },
                    "/v1/missions/{mission_id}/cancel": { "post": { "responses": { "202": { "description": "cooperative cancellation requested" } } } },
                    "/v1/rpc": { "post": { "responses": { "200": { "description": "JSON-RPC response" } } } },
                    "/v1/events": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "review_id", "in": "query" }, { "name": "receipt_id", "in": "query" }], "responses": { "200": { "description": "cursor page; review_id and receipt_id are mutually exclusive" } } } },
                    "/v1/events/stream": { "get": { "parameters": [{ "name": "review_id", "in": "query" }, { "name": "receipt_id", "in": "query" }], "responses": { "200": { "description": "bounded Server-Sent Events snapshot" } } } },
                    "/v1/delivery-receipts/{receipt_id}/events": { "get": { "parameters": [{ "name": "receipt_id", "in": "path", "required": true }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "retained delivery-receipt event page" } } } },
                    "/v1/delivery-receipts/{receipt_id}/attempts": { "get": { "parameters": [{ "name": "receipt_id", "in": "path", "required": true }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "retained delivery-attempt provenance correlated to a receipt" } } } },
                    "/v1/route-reviews/{review_id}/evidence": { "get": { "parameters": [{ "name": "review_id", "in": "path", "required": true }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "retained route-review evidence page" } } } },
                    "/v1/events/persistence": { "get": { "responses": { "200": { "description": "event cursor checkpoint status" } } } },
                    "/v1/events/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded event cursor checkpoint" } } } },
                    "/v1/webhooks/subscriptions": { "get": { "responses": { "200": { "description": "subscriptions" } } }, "post": { "responses": { "201": { "description": "subscription" } } } },
                    "/v1/webhooks/subscriptions/{id}/rebind": { "post": { "parameters": [{ "name": "id", "in": "path", "required": true }], "responses": { "200": { "description": "in-memory secret rebind and pending-envelope re-sign" } } } },
                    "/v1/webhooks/subscriptions/{id}/deliveries": { "get": { "responses": { "200": { "description": "cursor page of inspectable pending deliveries and failure metadata" } } } },
                    "/v1/webhooks/subscriptions/{id}/attempts": { "get": { "parameters": [{ "name": "id", "in": "path", "required": true }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "cursor page of durable delivery-attempt provenance" } } } },
                    "/v1/webhooks/subscriptions/{id}/ack": { "post": { "responses": { "200": { "description": "idempotent acknowledgement" } } } },
                    "/v1/webhooks/subscriptions/{id}/retry": { "post": { "responses": { "200": { "description": "advance selected deliveries by one retry attempt" } } } },
                    "/v1/webhooks/subscriptions/{id}/replay": { "post": { "responses": { "200": { "description": "reset selected deliveries for an explicit bounded replay" } } } }
                },
                "x-contract": {
                    "grpc": "not provided by this dependency-free HTTP boundary",
                    "tls": "terminate at an operator-owned proxy",
                    "delivery": "poll, send, retry, and acknowledge signed outbox envelopes"
                }
            }),
        )
    }
}

fn job_state(job: &MissionJob) -> Result<MissionJobState, ()> {
    job.state.lock().map(|state| state.clone()).map_err(|_| ())
}

fn current_timestamp() -> Result<Timestamp, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?;
    let nanos = i128::try_from(elapsed.as_nanos())
        .map_err(|_| "system clock timestamp exceeds the supported range".to_string())?;
    Ok(Timestamp::from_nanos_utc(nanos))
}

/// Recover only the bounded route-review identity from a validated mission specification. The
/// queue checkpoint keeps the complete spec private; this projection lets operators verify that
/// a reviewed route stayed attached to the queued work without exposing domain arguments.
fn mission_route_review_provenance(arguments: &Value) -> Option<Value> {
    let request: MissionRequest = serde_json::from_value(arguments.clone()).ok()?;
    let plan = plan_mission(&request).ok()?;
    plan.route_review_provenance
}

fn mission_queue_job_json(job: &FactoryJob) -> Value {
    json!({
        "mission_id": job.id,
        "resource_class": job.resource_class,
        "idempotency": job.idempotency,
        "idempotency_key": job.idempotency_key().to_string(),
        "priority": job.priority,
        "max_attempts": job.max_attempts,
        "state": job.state,
        "attempts": job.attempts,
        "attempts_remaining": job.attempts_remaining(),
        "reason": job.reason,
        "spec_digest": job.idempotency_key().to_string(),
        "route_review_provenance": mission_route_review_provenance(&job.spec),
        "spec_returned": false
    })
}

fn response_value(response: HttpResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap_or_else(|_| {
        json!({
            "ok": false,
            "error": "internal response could not be represented as JSON"
        })
    })
}

fn operations_evidence_channels(tool: &str) -> &'static [&'static str] {
    const EVALUATION: &[&str] = &["evaluation"];
    const SAFETY: &[&str] = &["safety"];
    const RELEASE: &[&str] = &["release"];
    const SAFETY_RELEASE: &[&str] = &["safety", "release"];
    if tool == "safety_release_gate" {
        return SAFETY_RELEASE;
    }
    if tool.starts_with("bioeval_")
        || tool.starts_with("evaluation_")
        || tool.starts_with("benchmark_")
        || matches!(
            tool,
            "adaptive_panel"
                | "atlas_report"
                | "atlas_surface_audit"
                | "capability_rank"
                | "context_compare"
                | "measurement_compare"
                | "metrics_analytics_audit"
                | "metrics_profile_audit"
                | "modality_comparability_check"
                | "oracle_combine"
                | "oracle_missingness"
                | "oracle_reference_panel"
                | "posterior_gate"
                | "prism_minimize"
                | "quality_gate_run"
                | "research_ci_check"
        )
    {
        return EVALUATION;
    }
    if tool.starts_with("bioethics_")
        || tool.starts_with("security_")
        || tool.starts_with("safety_")
        || tool.starts_with("sandbox_")
        || matches!(
            tool,
            "foundation_contract_check"
                | "medical_boundary_check"
                | "policy_screen"
                | "runtime_effect_check"
                | "runtime_execution_simulate"
                | "runtime_tape_verify"
        )
    {
        return SAFETY;
    }
    if tool.starts_with("release_")
        || matches!(
            tool,
            "bundle_verify"
                | "conformance_run"
                | "developer_delivery_audit"
                | "developer_delivery_receipt_verify"
                | "evaluation_reproduction_check"
                | "ops_acceptance"
                | "operational_readiness_audit"
                | "registry_gate"
        )
    {
        return RELEASE;
    }
    &[]
}

/// Return the capability groups for which a completed evaluator tool is an explicit evidence
/// witness. This is intentionally a catalogue binding, not a claim that the evaluator is valid,
/// calibrated, independent, or scientifically sufficient for the group.
fn operations_evaluator_group_bindings(tool: &str) -> &'static [&'static str] {
    const BIOLOGICAL: &[&str] = &["biological_domains"];
    const BIOEVALUATION: &[&str] = &[
        "biological_domains",
        "bioevaluation_reference_contracts",
        "evaluation_and_baselines",
    ];
    const BASELINES: &[&str] = &["evaluation_and_baselines"];
    const BENCHMARKS: &[&str] = &[
        "evaluation_and_baselines",
        "benchmark_pack_portfolio",
        "megafactory_scale_and_oracles",
        "mutation_and_causal_discovery",
    ];
    const TRAJECTORY: &[&str] = &["trajectory_and_decision_cells", "evaluation_and_baselines"];
    const ATLAS: &[&str] = &["atlas_metrics_and_research_ci", "evaluation_and_baselines"];
    const ORACLE: &[&str] = &["oracle_mesh", "evaluation_and_baselines"];
    const DECISION: &[&str] = &["decision_context", "evaluation_and_baselines"];
    const REGISTRY: &[&str] = &["registry_operations_and_infrastructure"];

    if tool.starts_with("bioeval_") {
        return BIOEVALUATION;
    }
    if tool.starts_with("benchmark_")
        || matches!(tool, "mutation_family" | "scale_family_split_verify")
    {
        return BENCHMARKS;
    }
    if tool.starts_with("trace_")
        || matches!(
            tool,
            "benchmark_trace_analyze"
                | "benchmark_decision_audit"
                | "benchmark_integrity_audit"
                | "benchmark_counterfactual_check"
                | "benchmark_oracle_review"
        )
    {
        return TRAJECTORY;
    }
    if tool.starts_with("atlas_")
        || tool.starts_with("metrics_")
        || tool == "capability_rank"
        || tool == "research_ci_check"
    {
        return ATLAS;
    }
    if tool.starts_with("oracle_") {
        return ORACLE;
    }
    if matches!(
        tool,
        "context_compare" | "adaptive_panel" | "posterior_gate" | "prism_minimize"
    ) {
        return DECISION;
    }
    if matches!(
        tool,
        "modality_comparability_check"
            | "measurement_compare"
            | "onco_response_assess"
            | "onco_outcome_analyze"
            | "oncoworlds_methylation_compare"
            | "oncoworlds_radiogenomic_check"
            | "oncoworlds_era_shift_check"
            | "oncoworlds_equity_check"
    ) {
        return BIOLOGICAL;
    }
    if tool == "quality_gate_run" {
        return REGISTRY;
    }
    if tool.starts_with("evaluation_") {
        return BASELINES;
    }
    &[]
}

fn operations_required_gates() -> &'static [&'static str] {
    &[
        "catalogue",
        "observed_activity",
        "transport_completion",
        "evaluation_evidence",
        "domain_evaluator_evidence",
        "safety_evidence",
        "release_evidence",
    ]
}

fn operations_domain_coverage() -> Value {
    let advertised_tools = bioprism_mcp::tool_definitions()
        .into_iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let capability_groups = bioprism_mcp::workspace_capabilities();
    let groups = capability_groups.as_array().cloned().unwrap_or_default();
    let mut declared_tools = BTreeSet::new();
    let mut domain_labels = BTreeSet::new();
    let mut declared_tool_memberships = 0usize;
    let mut fully_advertised_group_count = 0usize;
    let mut groups_with_gaps = 0usize;
    let mut group_rows = Vec::new();

    for (index, group) in groups.iter().enumerate() {
        let id = group
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let status = group
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let domains = group
            .get("domains")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for domain in &domains {
            domain_labels.insert(domain.clone());
        }
        let tools = group
            .get("mcp_tools")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        declared_tool_memberships += tools.len();
        declared_tools.extend(tools.iter().cloned());
        let missing_tools = tools
            .iter()
            .filter(|tool| !advertised_tools.contains(*tool))
            .cloned()
            .collect::<Vec<_>>();
        let advertised_tool_count = tools.len().saturating_sub(missing_tools.len());
        if missing_tools.is_empty() {
            fully_advertised_group_count += 1;
        } else {
            groups_with_gaps += 1;
        }
        if index < MAX_OPERATIONS_DOMAIN_GROUPS {
            let fully_advertised = missing_tools.is_empty();
            group_rows.push(json!({
                "id": id,
                "status": status,
                "domains": domains,
                "declared_tool_count": tools.len(),
                "advertised_tool_count": advertised_tool_count,
                "missing_tool_count": missing_tools.len(),
                "missing_tools": missing_tools,
                "fully_advertised": fully_advertised
            }));
        }
    }

    let declared_tools_not_advertised = declared_tools
        .difference(&advertised_tools)
        .take(MAX_OPERATIONS_DOMAIN_TOOLS)
        .cloned()
        .collect::<Vec<_>>();
    let advertised_tools_without_group = advertised_tools
        .difference(&declared_tools)
        .take(MAX_OPERATIONS_DOMAIN_TOOLS)
        .cloned()
        .collect::<Vec<_>>();
    let omitted_declared_tools_not_advertised = declared_tools
        .difference(&advertised_tools)
        .count()
        .saturating_sub(declared_tools_not_advertised.len());
    let omitted_advertised_tools_without_group = advertised_tools
        .difference(&declared_tools)
        .count()
        .saturating_sub(advertised_tools_without_group.len());

    json!({
        "schema": "bioprism-domain-coverage/0.1",
        "group_count": groups.len(),
        "returned_groups": group_rows.len(),
        "truncated": groups.len() > MAX_OPERATIONS_DOMAIN_GROUPS,
        "max_groups": MAX_OPERATIONS_DOMAIN_GROUPS,
        "groups": group_rows,
        "domain_label_count": domain_labels.len(),
        "declared_tool_memberships": declared_tool_memberships,
        "unique_declared_tools": declared_tools.len(),
        "advertised_tool_count": advertised_tools.len(),
        "fully_advertised_group_count": fully_advertised_group_count,
        "groups_with_gaps": groups_with_gaps,
        "declared_tools_not_advertised": declared_tools_not_advertised,
        "omitted_declared_tools_not_advertised": omitted_declared_tools_not_advertised,
        "advertised_tools_without_group": advertised_tools_without_group,
        "omitted_advertised_tools_without_group": omitted_advertised_tools_without_group,
        "guarantees": [
            "group rows are derived from the same workspace capability catalogue exposed by MCP",
            "advertised tool names are compared exactly without inferring semantic support",
            "omission counts make truncation and catalogue gaps explicit"
        ],
        "non_claims": [
            "domain scientific validity",
            "runtime execution health for every advertised tool",
            "performance, calibration, or external-system availability"
        ]
    })
}

fn operations_handoff_value(arguments: &serde_json::Map<String, Value>) -> Result<Value, String> {
    for key in arguments.keys() {
        if !matches!(
            key.as_str(),
            "goal" | "domains" | "group_ids" | "include_complete" | "max_groups"
        ) {
            return Err(format!(
                "operations handoff does not accept the {key:?} field"
            ));
        }
    }
    let goal = match arguments.get("goal") {
        None => "route a bounded cross-domain operator handoff".to_string(),
        Some(Value::String(value))
            if !value.trim().is_empty()
                && value.len() <= 1024
                && value.bytes().all(|byte| byte >= 0x20) =>
        {
            value.clone()
        }
        Some(_) => {
            return Err("goal must be a non-empty visible string of at most 1024 bytes".into())
        }
    };
    let selector = |name: &str| -> Result<Vec<String>, String> {
        let Some(value) = arguments.get(name) else {
            return Ok(Vec::new());
        };
        let values = value
            .as_array()
            .ok_or_else(|| format!("{name} must be an array of visible strings"))?;
        if values.len() > MAX_OPERATIONS_DOMAIN_GROUPS {
            return Err(format!(
                "{name} must contain at most {MAX_OPERATIONS_DOMAIN_GROUPS} entries"
            ));
        }
        let mut selected = Vec::with_capacity(values.len());
        for value in values {
            let Some(value) = value.as_str() else {
                return Err(format!("{name} must contain only strings"));
            };
            if value.trim().is_empty()
                || value.len() > 128
                || !value.bytes().all(|byte| byte >= 0x20)
            {
                return Err(format!(
                    "{name} entries must be non-empty visible strings of at most 128 bytes"
                ));
            }
            selected.push(value.to_string());
        }
        selected.sort();
        selected.dedup();
        Ok(selected)
    };
    let domains = selector("domains")?;
    let group_ids = selector("group_ids")?;
    let include_complete = arguments
        .get("include_complete")
        .map(|value| {
            value
                .as_bool()
                .ok_or("include_complete must be a boolean".to_string())
        })
        .transpose()?
        .unwrap_or(true);
    let max_groups = match arguments.get("max_groups") {
        None => MAX_OPERATIONS_DOMAIN_GROUPS,
        Some(value) => {
            let Some(value) = value_usize(value) else {
                return Err(format!(
                    "max_groups must be between 1 and {MAX_OPERATIONS_DOMAIN_GROUPS}"
                ));
            };
            if !(1..=MAX_OPERATIONS_DOMAIN_GROUPS).contains(&value) {
                return Err(format!(
                    "max_groups must be between 1 and {MAX_OPERATIONS_DOMAIN_GROUPS}"
                ));
            }
            value
        }
    };

    let coverage = operations_domain_coverage();
    let groups = coverage
        .get("groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selector_is_empty = domains.is_empty() && group_ids.is_empty();
    let mut matching_group_count = 0usize;
    let mut complete_groups_omitted = 0usize;
    let mut selected_with_gaps = 0usize;
    let mut selected_groups = Vec::new();
    let mut route_needs = Vec::new();
    for group in &groups {
        let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
        let id_matches = group_ids.is_empty() || group_ids.iter().any(|value| value == id);
        let domain_matches = domains.is_empty()
            || group
                .get("domains")
                .and_then(Value::as_array)
                .is_some_and(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|domain| domains.iter().any(|requested| requested == domain))
                });
        if !(id_matches && domain_matches) {
            continue;
        }
        matching_group_count += 1;
        let fully_advertised = group
            .get("fully_advertised")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !include_complete && fully_advertised {
            complete_groups_omitted += 1;
            continue;
        }
        if group
            .get("missing_tool_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > 0
        {
            selected_with_gaps += 1;
        }
        let need_id = format!("domain-group:{id}");
        let mut selected = group.clone();
        selected["route_need_id"] = json!(need_id);
        selected["next_action"] = json!(if fully_advertised {
            "submit the route_need to capability_route, then review explicit selections"
        } else {
            "inspect capability_audit and repair catalogue gaps before routing"
        });
        selected["evidence_prerequisite"] = json!({
            "kind": "operations_gate_acceptance",
            "group_id": id,
            "required_gates": operations_required_gates(),
            "gate_endpoint": "/v1/operations/gates?after=0&limit=256",
            "review_endpoint": "/v1/operations/gate-reviews",
            "acceptance_field": "operations_gate_acceptance",
            "review_required_before_dispatch": true
        });
        if selected_groups.len() < max_groups {
            selected_groups.push(selected);
            route_needs.push(json!({
                "id": need_id,
                "group_id": id,
                "query": goal,
                "max_items": 10,
                "evidence_prerequisite": {
                    "kind": "operations_gate_acceptance",
                    "group_id": id,
                    "required_gates": operations_required_gates(),
                    "gate_endpoint": "/v1/operations/gates?after=0&limit=256",
                    "acceptance_field": "operations_gate_acceptance"
                }
            }));
        }
    }
    let unresolved_group_ids = group_ids
        .iter()
        .filter(|requested| {
            !groups
                .iter()
                .any(|group| group.get("id").and_then(Value::as_str) == Some(requested.as_str()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_domains = domains
        .iter()
        .filter(|requested| {
            !groups.iter().any(|group| {
                group
                    .get("domains")
                    .and_then(Value::as_array)
                    .is_some_and(|values| {
                        values.iter().any(|value| value.as_str() == Some(requested))
                    })
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let included_group_count = selected_groups.len();
    let truncated =
        matching_group_count.saturating_sub(complete_groups_omitted) > included_group_count;
    let handoff_status = if !unresolved_group_ids.is_empty()
        || !unresolved_domains.is_empty()
        || (!selector_is_empty && matching_group_count == 0)
    {
        "unresolved_domain"
    } else if included_group_count == 0 && complete_groups_omitted > 0 {
        "no_actionable_gaps"
    } else if selected_with_gaps > 0 {
        "requires_catalogue_review"
    } else {
        "ready_for_capability_route"
    };
    let canonical = json!({
        "goal": goal,
        "domains": domains,
        "group_ids": group_ids,
        "include_complete": include_complete,
        "max_groups": max_groups,
        "coverage": coverage
    });
    let canonical_bytes = serde_json::to_vec(&canonical)
        .map_err(|error| format!("operations handoff could not be digested: {error}"))?;
    let handoff_id = hex_digest(&Sha256::digest(&canonical_bytes));
    let coverage_bytes = serde_json::to_vec(&coverage)
        .map_err(|error| format!("domain coverage could not be digested: {error}"))?;
    let domain_coverage_digest = hex_digest(&Sha256::digest(&coverage_bytes));
    let selected_group_ids = selected_groups
        .iter()
        .filter_map(|group| group.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();

    Ok(json!({
        "ok": true,
        "workflow": "operations_domain_handoff",
        "schema": "bioprism-operations-handoff/0.1",
        "handoff_id": handoff_id,
        "domain_coverage_digest": domain_coverage_digest,
        "goal": goal,
        "selection": {
            "domains": domains,
            "group_ids": group_ids,
            "include_complete": include_complete,
            "max_groups": max_groups,
            "selector_mode": if selector_is_empty { "all_groups" } else { "intersection" }
        },
        "coverage": {
            "matching_group_count": matching_group_count,
            "included_group_count": included_group_count,
            "complete_groups_omitted": complete_groups_omitted,
            "selected_groups_with_gaps": selected_with_gaps,
            "truncated": truncated,
            "unresolved_group_ids": unresolved_group_ids,
            "unresolved_domains": unresolved_domains
        },
        "groups": selected_groups,
        "route_request": {
            "goal": goal,
            "needs": route_needs,
            "max_candidates_per_need": 10,
            "max_tools": 128,
            "include_tools": false
        },
        "execution_prerequisites": {
            "kind": "operations_gate_acceptance",
            "required": true,
            "group_ids": selected_group_ids,
            "required_gates": operations_required_gates(),
            "gate_endpoint": "/v1/operations/gates?after=0&limit=256",
            "review_endpoint": "/v1/operations/gate-reviews",
            "acceptance_field": "operations_gate_acceptance",
            "review_required_before_dispatch": true,
            "readiness_claimed": false
        },
        "handoff_status": handoff_status,
        "execution": "not_started",
        "next_steps": [
            "inspect exact missing tool names and run capability_audit when handoff_status requires_catalogue_review",
            "submit route_request to capability_route for ranked candidates",
            "review caller-selected tools with capability_route_review before mission_preflight",
            "create and replay an operations_gate_review, then attach its retained acceptance to execution",
            "execute only after the returned mission plan and domain-specific evidence are accepted"
        ],
        "guarantees": [
            "the handoff is content-addressed by the normalized request and current domain coverage",
            "route_request is an explicit proposal and is never dispatched by this endpoint",
            "unresolved selectors, catalogue gaps, and pagination are retained as separate evidence"
        ],
        "non_claims": [
            "tool semantic readiness or scientific validity",
            "authorization to execute a mission",
            "runtime health, calibration, or external-system availability"
        ],
        "links": {
            "capabilities": "/v1/capabilities",
            "operations_snapshot": "/v1/operations/snapshot",
            "operations_gates": "/v1/operations/gates",
            "capability_route": "/v1/tools/capability_route",
            "capability_route_review": "/v1/tools/capability_route_review",
            "mission_preflight": "/v1/missions/preflight"
        }
    }))
}

fn mission_execution_requested(arguments: &Value) -> bool {
    arguments
        .pointer("/policy/execute")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn mission_execution_provenance(
    mission_id: &str,
    arguments: &Value,
    evidence: &Value,
) -> Option<Value> {
    let review_id = evidence.get("review_id")?.as_str()?;
    let review_event_id = evidence.get("review_event_id")?.as_u64()?;
    let gate_digest = evidence.get("gate_digest")?.as_str()?;
    let acceptance = arguments.get("operations_gate_acceptance")?.clone();
    let mut provenance = json!({
        "schema": "bioprism-mission-execution-provenance/0.1",
        "workflow": "mission_execution",
        "mission_id": mission_id,
        "dispatch": "accepted",
        "review_id": review_id,
        "review_event_id": review_event_id,
        "gate_digest": gate_digest,
        "gate_digest_scope": evidence.get("gate_digest_scope").cloned().unwrap_or(Value::Null),
        "acceptance": acceptance,
        "operations_evidence": evidence,
        "replay": {
            "operations_gates": "/v1/operations/gates?after=0&limit=256",
            "gate_review": format!("/v1/operations/gate-reviews?review_id={review_id}"),
            "event_stream": "/v1/events?after=0&limit=256",
        },
        "readiness_claimed": false,
        "provenance_digest_scope": "all_projection_fields_except_accepted_event_id",
        "non_claims": [
            "the retained review and evaluator binding do not establish scientific, clinical, regulatory, or deployment validity",
            "the bounded evidence page is not complete history when retention gaps or pagination apply",
            "dispatch acceptance does not prove successful tool execution or external effect completion"
        ]
    });
    let digest_bytes = serde_json::to_vec(&provenance).ok()?;
    provenance["provenance_digest"] = json!(hex_digest(&Sha256::digest(&digest_bytes)));
    Some(provenance)
}

fn mission_domain_group_requirements(arguments: &Value) -> Value {
    let groups = bioprism_mcp::workspace_capabilities()
        .as_array()
        .cloned()
        .unwrap_or_default();
    let steps = arguments
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut group_ids = BTreeSet::new();
    let mut unresolved_steps = Vec::new();
    for step in steps {
        let step_id = step
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let tool = step.get("tool").and_then(Value::as_str).unwrap_or("");
        let domain = step.get("domain").and_then(Value::as_str).unwrap_or("");
        let tool_matches = |group: &Value| {
            group
                .get("mcp_tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| {
                    tools
                        .iter()
                        .any(|candidate| candidate.as_str() == Some(tool))
                })
        };
        let domain_matches = |group: &Value| {
            domain.is_empty()
                || group
                    .get("domains")
                    .and_then(Value::as_array)
                    .is_some_and(|domains| {
                        domains
                            .iter()
                            .any(|candidate| candidate.as_str() == Some(domain))
                    })
        };
        let mut matches = groups
            .iter()
            .filter(|group| tool_matches(group) && domain_matches(group))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            matches = groups.iter().filter(|group| tool_matches(group)).collect();
        }
        if matches.is_empty() {
            unresolved_steps.push(json!({
                "step_id": step_id,
                "tool": tool,
                "domain": domain,
                "reason": "no workspace capability group advertises the exact tool"
            }));
            continue;
        }
        for group in matches {
            if let Some(id) = group.get("id").and_then(Value::as_str) {
                group_ids.insert(id.to_string());
            }
        }
    }
    json!({
        "group_ids": group_ids.into_iter().collect::<Vec<_>>(),
        "unresolved_steps": unresolved_steps
    })
}

fn validate_operations_gate_acceptance(arguments: &Value) -> Result<(), String> {
    let Some(value) = arguments.get("operations_gate_acceptance") else {
        return Ok(());
    };
    validate_operations_gate_acceptance_value(value, true)
}

fn validate_operations_gate_acceptance_value(
    value: &Value,
    require_review_id: bool,
) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "operations_gate_acceptance must be an object".to_string())?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "gate_digest" | "reviewer" | "rationale" | "group_ids" | "accepted_gates" | "review_id"
        ) {
            return Err(format!(
                "operations_gate_acceptance does not accept the {key:?} field"
            ));
        }
    }
    if require_review_id {
        let review_id = object
            .get("review_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "operations_gate_acceptance.review_id must be a 64-character digest".to_string()
            })?;
        if review_id.len() != 64
            || !review_id
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(
                "operations_gate_acceptance.review_id must be lowercase hexadecimal".into(),
            );
        }
    } else if object.contains_key("review_id") {
        return Err("operations gate review requests must not provide review_id".into());
    }
    let gate_digest = object
        .get("gate_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "operations_gate_acceptance.gate_digest must be a 64-character digest".to_string()
        })?;
    if gate_digest.len() != 64
        || !gate_digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err("operations_gate_acceptance.gate_digest must be lowercase hexadecimal".into());
    }
    for (field, maximum) in [("reviewer", 256usize), ("rationale", 2048usize)] {
        let value = object.get(field).and_then(Value::as_str).ok_or_else(|| {
            format!("operations_gate_acceptance.{field} must be a visible string")
        })?;
        if value.trim().is_empty() || value.len() > maximum || value.bytes().any(|byte| byte < 0x20)
        {
            return Err(format!(
                "operations_gate_acceptance.{field} must be non-empty and at most {maximum} visible bytes"
            ));
        }
    }
    let group_ids = object
        .get("group_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| "operations_gate_acceptance.group_ids must be an array".to_string())?;
    if group_ids.is_empty() || group_ids.len() > MAX_OPERATIONS_DOMAIN_GROUPS {
        return Err(format!(
            "operations_gate_acceptance.group_ids must contain between 1 and {MAX_OPERATIONS_DOMAIN_GROUPS} entries"
        ));
    }
    let mut unique_group_ids = BTreeSet::new();
    for group_id in group_ids {
        let group_id = group_id.as_str().ok_or_else(|| {
            "operations_gate_acceptance.group_ids must contain strings".to_string()
        })?;
        if group_id.trim().is_empty()
            || group_id.len() > 128
            || group_id.bytes().any(|byte| byte < 0x20)
            || !unique_group_ids.insert(group_id.to_string())
        {
            return Err(
                "operations_gate_acceptance.group_ids must contain unique visible strings".into(),
            );
        }
    }
    let accepted_gates = object
        .get("accepted_gates")
        .and_then(Value::as_object)
        .ok_or_else(|| "operations_gate_acceptance.accepted_gates must be an object".to_string())?;
    for (group_id, gates) in accepted_gates {
        if group_id.trim().is_empty() || group_id.len() > 128 {
            return Err("operations_gate_acceptance.accepted_gates has an invalid group id".into());
        }
        let gates = gates.as_array().ok_or_else(|| {
            format!("operations_gate_acceptance.accepted_gates[{group_id:?}] must be an array")
        })?;
        let mut unique_gates = BTreeSet::new();
        for gate in gates {
            let gate = gate.as_str().ok_or_else(|| {
                format!(
                    "operations_gate_acceptance.accepted_gates[{group_id:?}] must contain strings"
                )
            })?;
            if !operations_required_gates().contains(&gate) || !unique_gates.insert(gate) {
                return Err(format!(
                    "operations_gate_acceptance.accepted_gates[{group_id:?}] contains an unknown or duplicate gate"
                ));
            }
        }
    }
    Ok(())
}

fn operations_gate_acceptance_canonical(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let mut group_ids = object
        .get("group_ids")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    group_ids.sort();
    let mut accepted_gates = BTreeMap::new();
    for (group_id, gates) in object.get("accepted_gates")?.as_object()? {
        let mut gates = gates
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        gates.sort();
        accepted_gates.insert(group_id.clone(), json!(gates));
    }
    Some(json!({
        "gate_digest": object.get("gate_digest")?,
        "reviewer": object.get("reviewer")?,
        "rationale": object.get("rationale")?,
        "group_ids": group_ids,
        "accepted_gates": accepted_gates
    }))
}

fn operations_gate_review_rows(snapshot: &Value, group_ids: &[Value]) -> Vec<Value> {
    group_ids
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|group_id| {
            snapshot
                .get("groups")
                .and_then(Value::as_array)
                .and_then(|groups| {
                    groups
                        .iter()
                        .find(|group| group.get("id").and_then(Value::as_str) == Some(group_id))
                })
                .map(|group| {
                    let gates = group.get("gates").and_then(Value::as_object);
                    let missing_gates = operations_required_gates()
                        .iter()
                        .filter(|gate| {
                            let expected = if **gate == "catalogue" {
                                "pass"
                            } else {
                                "observed"
                            };
                            gates
                                .and_then(|gates| gates.get(**gate))
                                .and_then(|gate| gate.get("state"))
                                .and_then(Value::as_str)
                                != Some(expected)
                        })
                        .map(|gate| (*gate).to_string())
                        .collect::<Vec<_>>();
                    json!({
                        "group_id": group_id,
                        "gate_state": group.get("gate_state").cloned().unwrap_or_else(|| json!("insufficient_evidence")),
                        "missing_gates": missing_gates,
                        "gates": group.get("gates").cloned().unwrap_or_else(|| json!({})),
                        "last_event_id": group.get("last_event_id").cloned().unwrap_or(Value::Null),
                        "readiness_claimed": false
                    })
                })
        })
        .collect()
}

fn operations_gate_acceptance_matches(
    arguments: &Value,
    group_ids: &[Value],
    gate_digest: &Value,
    rows: &[Value],
) -> bool {
    let Some(acceptance) = arguments
        .get("operations_gate_acceptance")
        .and_then(Value::as_object)
    else {
        return false;
    };
    if acceptance.get("gate_digest") != Some(gate_digest) {
        return false;
    }
    let expected_group_ids = group_ids
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let accepted_group_ids = acceptance
        .get("group_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if accepted_group_ids != expected_group_ids {
        return false;
    }
    let Some(accepted_gates) = acceptance.get("accepted_gates").and_then(Value::as_object) else {
        return false;
    };
    let accepted_gate_group_ids = accepted_gates.keys().cloned().collect::<BTreeSet<_>>();
    if accepted_gate_group_ids != expected_group_ids {
        return false;
    }
    let required = operations_required_gates()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    rows.iter().all(|row| {
        if row.get("gate_state").and_then(Value::as_str) != Some("review_required") {
            return false;
        }
        let Some(group_id) = row.get("group_id").and_then(Value::as_str) else {
            return false;
        };
        let supplied = accepted_gates
            .get(group_id)
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        supplied == required
    })
}

fn mission_checkpoint_digest(document: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(document)
        .map_err(|error| format!("mission state digest could not be serialized: {error}"))?;
    Ok(hex_digest(&Sha256::digest(&bytes)))
}

fn verify_mission_checkpoint_digest(document: &Value) -> Result<(), String> {
    let stored = document
        .get("state_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| "mission state schema 2 requires state_digest".to_string())?;
    if stored.len() != 64
        || !stored
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(
            "mission state state_digest must be 64 lowercase hexadecimal characters".into(),
        );
    }
    let mut unsigned = document.clone();
    unsigned
        .as_object_mut()
        .ok_or_else(|| "mission state snapshot must be a JSON object".to_string())?
        .remove("state_digest");
    let computed = mission_checkpoint_digest(&unsigned)?;
    if computed != stored {
        return Err(format!(
            "mission state state_digest mismatch: expected {stored}, computed {computed}"
        ));
    }
    Ok(())
}

fn checkpoint_digest_from_path(path: Option<&Path>) -> Option<String> {
    path.and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|document| {
            document
                .get("state_digest")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn checkpoint_integrity_from_path(path: Option<&Path>, expected_schema: u64) -> Option<bool> {
    let bytes = path.and_then(|path| std::fs::read(path).ok())?;
    let document: Value = match serde_json::from_slice(&bytes) {
        Ok(document) => document,
        Err(_) => return Some(false),
    };
    let schema_version = match document.get("schema_version").and_then(Value::as_u64) {
        Some(schema_version) => schema_version,
        None => return Some(false),
    };
    if schema_version != expected_schema {
        return None;
    }
    let stored = match document.get("state_digest").and_then(Value::as_str) {
        Some(stored) => stored,
        None => return Some(false),
    };
    let mut unsigned = match document.as_object() {
        Some(object) => Value::Object(object.clone()),
        None => return Some(false),
    };
    unsigned
        .as_object_mut()
        .expect("checkpoint object was cloned from an object")
        .remove("state_digest");
    let Ok(computed) = mission_checkpoint_digest(&unsigned) else {
        return Some(false);
    };
    Some(computed == stored)
}

fn load_evidence_registry(path: Option<&Path>) -> Result<EvidenceBundleRegistry, String> {
    let Some(path) = path else {
        return Ok(EvidenceBundleRegistry::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EvidenceBundleRegistry::new())
        }
        Err(error) => {
            return Err(format!(
                "evidence state snapshot could not be read: {error}"
            ))
        }
    };
    if bytes.len() > MAX_EVIDENCE_REGISTRY_BYTES {
        return Err(format!(
            "evidence state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_EVIDENCE_REGISTRY_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("evidence state snapshot is invalid JSON: {error}"))?;
    EvidenceBundleRegistry::from_snapshot(&document)
        .map_err(|error| format!("evidence state snapshot is invalid: {error}"))
}

fn load_workflow_reconciliation_registry(
    path: Option<&Path>,
) -> Result<DomainWorkflowReconciliationRegistry, String> {
    let Some(path) = path else {
        return Ok(DomainWorkflowReconciliationRegistry::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DomainWorkflowReconciliationRegistry::new())
        }
        Err(error) => {
            return Err(format!(
                "workflow reconciliation state snapshot could not be read: {error}"
            ))
        }
    };
    if bytes.len() > MAX_WORKFLOW_RECONCILIATION_STATE_BYTES {
        return Err(format!(
            "workflow reconciliation state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_WORKFLOW_RECONCILIATION_STATE_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!("workflow reconciliation state snapshot is invalid JSON: {error}")
    })?;
    DomainWorkflowReconciliationRegistry::from_snapshot(&document)
        .map_err(|error| format!("workflow reconciliation state snapshot is invalid: {error}"))
}

fn load_artifact_registry(path: Option<&Path>) -> Result<ArtifactRegistry, String> {
    let Some(path) = path else {
        return Ok(ArtifactRegistry::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArtifactRegistry::new())
        }
        Err(error) => {
            return Err(format!(
                "artifact state snapshot could not be read: {error}"
            ))
        }
    };
    if bytes.len() > MAX_ARTIFACT_REGISTRY_BYTES {
        return Err(format!(
            "artifact state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_ARTIFACT_REGISTRY_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("artifact state snapshot is invalid JSON: {error}"))?;
    ArtifactRegistry::from_snapshot(&document)
        .map_err(|error| format!("artifact state snapshot is invalid: {error}"))
}

fn load_workflow_execution_evidence_registry(
    path: Option<&Path>,
) -> Result<WorkflowExecutionEvidenceRegistry, String> {
    let Some(path) = path else {
        return Ok(WorkflowExecutionEvidenceRegistry::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkflowExecutionEvidenceRegistry::new())
        }
        Err(error) => {
            return Err(format!(
                "workflow execution evidence state snapshot could not be read: {error}"
            ))
        }
    };
    if bytes.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES {
        return Err(format!(
            "workflow execution evidence state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!("workflow execution evidence state snapshot is invalid JSON: {error}")
    })?;
    WorkflowExecutionEvidenceRegistry::from_snapshot(&document)
        .map_err(|error| format!("workflow execution evidence state snapshot is invalid: {error}"))
}

fn load_workbench_registry(path: Option<&Path>) -> Result<WorkbenchReportRegistry, String> {
    let Some(path) = path else {
        return Ok(WorkbenchReportRegistry::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkbenchReportRegistry::new())
        }
        Err(error) => {
            return Err(format!(
                "workbench state snapshot could not be read: {error}"
            ))
        }
    };
    if bytes.len() > MAX_WORKBENCH_REGISTRY_STATE_BYTES {
        return Err(format!(
            "workbench state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_WORKBENCH_REGISTRY_STATE_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("workbench state snapshot is invalid JSON: {error}"))?;
    WorkbenchReportRegistry::from_snapshot(&document)
        .map_err(|error| format!("workbench state snapshot is invalid: {error}"))
}

fn load_ci_provider_evidence_registry(
    path: Option<&Path>,
) -> Result<CiProviderEvidenceRegistry, String> {
    let Some(path) = path else {
        return Ok(CiProviderEvidenceRegistry::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CiProviderEvidenceRegistry::new())
        }
        Err(error) => {
            return Err(format!(
                "CI provider evidence state snapshot could not be read: {error}"
            ))
        }
    };
    if bytes.len() > MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES {
        return Err(format!(
            "CI provider evidence state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("CI provider evidence state snapshot is invalid JSON: {error}"))?;
    CiProviderEvidenceRegistry::from_snapshot(&document)
        .map_err(|error| format!("CI provider evidence state snapshot is invalid: {error}"))
}

fn load_mission_jobs(path: Option<&Path>) -> Result<BTreeMap<String, Arc<MissionJob>>, String> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(format!("mission state snapshot could not be read: {error}")),
    };
    if bytes.len() > MAX_MISSION_STATE_FILE_BYTES {
        return Err(format!(
            "mission state snapshot is {} bytes, above the {}-byte bound",
            bytes.len(),
            MAX_MISSION_STATE_FILE_BYTES
        ));
    }
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("mission state snapshot is invalid JSON: {error}"))?;
    let schema_version = document
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "mission state snapshot has no schema_version".to_string())?;
    if schema_version != MISSION_STATE_SCHEMA_VERSION
        && schema_version != LEGACY_MISSION_STATE_SCHEMA_VERSION
    {
        return Err(format!(
            "unsupported mission state schema version {schema_version}; expected {LEGACY_MISSION_STATE_SCHEMA_VERSION} or {MISSION_STATE_SCHEMA_VERSION}"
        ));
    }
    if schema_version == MISSION_STATE_SCHEMA_VERSION {
        verify_mission_checkpoint_digest(&document)?;
    }
    let missions = document
        .get("missions")
        .and_then(Value::as_array)
        .ok_or_else(|| "mission state snapshot has no missions array".to_string())?;
    if missions.len() > MAX_MISSION_JOBS {
        return Err(format!(
            "mission state snapshot contains {} jobs, above the {}-job bound",
            missions.len(),
            MAX_MISSION_JOBS
        ));
    }
    let mut restored = BTreeMap::new();
    for mission in missions {
        let object = mission
            .as_object()
            .ok_or_else(|| "mission state entry must be a JSON object".to_string())?;
        let mission_id = object
            .get("mission_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty() && value.len() <= 256)
            .ok_or_else(|| "mission state entry has an invalid mission_id".to_string())?
            .to_string();
        if restored.contains_key(&mission_id) {
            return Err(format!(
                "mission state snapshot repeats mission_id {mission_id:?}"
            ));
        }
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .filter(|status| is_known_mission_status(status))
            .ok_or_else(|| format!("mission {mission_id:?} has an invalid status"))?
            .to_string();
        let total_steps = object.get("total_steps").and_then(value_usize).unwrap_or(0);
        let trace = object
            .get("trace")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("mission {mission_id:?} has no trace array"))?
            .iter()
            .rev()
            .take(MAX_MISSION_TRACE_EVENTS)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let mut state = MissionJobState {
            total_steps,
            trace,
            progress: mission_progress_from_json(object.get("progress"), total_steps),
            status,
            cancel_requested: object
                .get("cancel_requested")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            cancel_reason: object
                .get("cancel_reason")
                .and_then(Value::as_str)
                .map(str::to_string),
            result: object
                .get("result")
                .filter(|value| !value.is_null())
                .cloned(),
            result_omitted: object.get("result_omitted").cloned(),
            evaluator_replay_summary: object
                .get("evaluator_replay_summary")
                .filter(|value| value.is_object())
                .cloned(),
            route_review_provenance: object
                .get("route_review_provenance")
                .filter(|value| value.is_object())
                .cloned(),
            error: object
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
            recovered_after_restart: object
                .get("recovered_after_restart")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            execution_provenance: object
                .get("execution_provenance")
                .filter(|value| value.is_object())
                .cloned(),
        };
        if !is_terminal_mission_status(&state.status) {
            state.status = "failed".into();
            state.progress.phase = "failed".into();
            state.progress.active_steps = 0;
            state.error = Some(
                "mission was interrupted by an API process restart; execution was not resumed"
                    .into(),
            );
            state.recovered_after_restart = true;
        }
        restored.insert(
            mission_id,
            Arc::new(MissionJob {
                cancellation: Arc::new(AtomicBool::new(false)),
                state: Arc::new(Mutex::new(state)),
            }),
        );
    }
    Ok(restored)
}

fn durable_mission_state_json(mission_id: &str, state: &MissionJobState) -> Value {
    let persisted_trace = state
        .trace
        .iter()
        .map(durable_trace_event)
        .collect::<Vec<_>>();
    let (result, generated_omission) = match state.result.as_ref() {
        Some(result) => match serde_json::to_vec(result) {
            Ok(bytes) if bytes.len() <= MAX_PERSISTED_MISSION_RESULT_BYTES => {
                (result.clone(), None)
            }
            Ok(bytes) => (Value::Null, Some(value_omission(&bytes))),
            Err(_) => (Value::Null, None),
        },
        None => (Value::Null, None),
    };
    let (execution_provenance, generated_provenance_omission) =
        match state.execution_provenance.as_ref() {
            Some(provenance) => match serde_json::to_vec(provenance) {
                Ok(bytes) if bytes.len() <= MAX_PERSISTED_MISSION_PROVENANCE_BYTES => {
                    (provenance.clone(), None)
                }
                Ok(bytes) => (Value::Null, Some(value_omission(&bytes))),
                Err(_) => (Value::Null, None),
            },
            None => (Value::Null, None),
        };
    json!({
        "mission_id": mission_id,
        "total_steps": state.total_steps,
        "status": state.status,
        "cancel_requested": state.cancel_requested,
        "cancel_reason": state.cancel_reason,
        "progress": mission_progress_json(&state.progress),
        "trace": persisted_trace,
        "result": result,
        "result_omitted": state.result_omitted.clone().or(generated_omission),
        "evaluator_replay_summary": state.evaluator_replay_summary.clone(),
        "route_review_provenance": state.route_review_provenance.clone(),
        "execution_provenance": execution_provenance,
        "execution_provenance_omitted": generated_provenance_omission,
        "error": state.error,
        "recovered_after_restart": state.recovered_after_restart,
    })
}

fn durable_trace_event(event: &Value) -> Value {
    let Ok(bytes) = serde_json::to_vec(event) else {
        return json!({ "event": "trace.event_omitted", "detail_omitted": true });
    };
    if bytes.len() <= MAX_PERSISTED_MISSION_TRACE_EVENT_BYTES {
        return event.clone();
    }
    json!({
        "sequence": event.get("sequence"),
        "event": event.get("event"),
        "wave": event.get("wave"),
        "step_id": event.get("step_id"),
        "tool": event.get("tool"),
        "status": event.get("status"),
        "arguments_digest": event.get("arguments_digest"),
        "bytes": event.get("bytes"),
        "detail": Value::Null,
        "detail_omitted": value_omission(&bytes),
    })
}

/// Retain a compact evaluator replay index independently of the full terminal report. The index is
/// intentionally non-executing and non-semantic: it preserves enough accounting to explain what
/// remains queryable after the bounded mission result itself has been omitted from a checkpoint.
fn evaluator_replay_summary(report: &Value, mission_id: &str) -> Option<Value> {
    if report.get("workflow").and_then(Value::as_str) != Some("agent_mission") {
        return None;
    }
    let result_bytes = serde_json::to_vec(report).ok()?;
    let result_digest = hex_digest(&Sha256::digest(&result_bytes));
    let request = MissionEvaluatorReplayRequest {
        mission: report.clone(),
        include_fixtures: false,
        max_items: 512,
    };
    let replay = MissionEvaluatorCatalogue::standard()
        .replay(&request)
        .ok()?;
    let claims = replay
        .get("claims")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some(json!({
                        "claim_id": row.get("claim_id")?,
                        "binding_count": row.get("binding_count")?,
                        "returned_binding_count": row.get("returned_binding_count")?,
                        "outcome_counts": row.get("outcome_counts")?,
                        "distinct_output_digests": row.get("distinct_output_digests")?,
                        "disagreement_posture": row.get("disagreement_posture")?,
                        "replayed_disagreement_posture": row.get("replayed_disagreement_posture")?
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let referenced_adapter_ids = mission_evaluator_binding_adapter_ids(report);
    let review_provenance = replay
        .get("review_provenance")
        .cloned()
        .unwrap_or(Value::Null);
    Some(json!({
        "schema": "bioprism-devplat-mission-evaluator-replay-summary/0.1",
        "workflow": "mission_evaluator_replay_summary",
        "mission_id": mission_id,
        "mission_digest": replay.get("mission_digest")?,
        "mission_status": replay.get("mission_status").cloned().unwrap_or(Value::Null),
        "catalog_digest": replay.get("catalog_digest")?,
        "historical_catalog_digest": review_provenance.get("catalog_digest").cloned().unwrap_or(Value::Null),
        "historical_review_id": review_provenance.get("review_id").cloned().unwrap_or(Value::Null),
        "historical_discovery_digest": review_provenance.get("discovery_digest").cloned().unwrap_or(Value::Null),
        "historical_catalogue_snapshot": review_provenance.get("catalogue_snapshot").cloned().unwrap_or(Value::Null),
        "route_review_provenance": replay.get("route_review_provenance").cloned().unwrap_or(Value::Null),
        "route_review_status": replay.get("route_review_status").cloned().unwrap_or(json!("absent")),
        "referenced_adapter_ids": referenced_adapter_ids,
        "binding_count": replay.get("binding_count")?,
        "omitted_bindings": replay.get("omitted_bindings")?,
        "state_counts": replay.get("state_counts")?,
        "claim_count": claims.len(),
        "claims": claims,
        "coverage": replay.get("coverage")?,
        "findings": replay.get("findings")?,
        "replay_status": replay.get("replay_status")?,
        "execution": "not_started",
        "result_retained": true,
        "result_bytes": result_bytes.len(),
        "result_digest": result_digest,
        "guarantees": [
            "the summary is derived from the retained terminal mission report",
            "the summary remains persisted when the full report exceeds the result retention bound",
            "no evaluator or domain tool is executed while building the summary"
        ],
        "limitations": [
            "summary-only recovery cannot expose omitted raw evaluator output or rerun replay against it",
            "the summary is structural evidence and not scientific, clinical, causal, or release truth"
        ]
    }))
}

fn mission_evaluator_binding_adapter_ids(report: &Value) -> Vec<String> {
    report
        .get("claim_lineage")
        .and_then(|lineage| lineage.get("claims"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|claim| claim.get("evaluator_bindings"))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|binding| binding.get("adapter_id"))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn value_omission(bytes: &[u8]) -> Value {
    let mut digest = Sha256::new();
    digest.update(bytes);
    json!({ "bytes": bytes.len(), "sha256": hex_digest(&digest.finalize()) })
}

fn trim_mission_snapshot_to_bound(missions: &mut [Value]) -> Result<(), String> {
    loop {
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
        document
            .as_object_mut()
            .expect("mission checkpoint document is an object")
            .insert("state_digest".into(), Value::String(state_digest));
        let size = serde_json::to_vec(&document)
            .map_err(|error| format!("mission state could not be sized: {error}"))?
            .len();
        if size <= MAX_MISSION_STATE_FILE_BYTES {
            return Ok(());
        }
        if let Some(object) = missions.iter_mut().find_map(Value::as_object_mut) {
            if let Some(result) = object.get_mut("result") {
                if !result.is_null() {
                    let bytes = serde_json::to_vec(result)
                        .map_err(|error| format!("mission result could not be sized: {error}"))?;
                    let omission = value_omission(&bytes);
                    *result = Value::Null;
                    object.insert("result_omitted".into(), omission);
                    continue;
                }
            }
        }
        if let Some(trace) = missions.iter_mut().find_map(|mission| {
            mission
                .get_mut("trace")
                .and_then(Value::as_array_mut)
                .filter(|trace| !trace.is_empty())
        }) {
            trace.remove(0);
            continue;
        }
        return Err(format!(
            "mission state snapshot cannot fit within the {}-byte bound",
            MAX_MISSION_STATE_FILE_BYTES
        ));
    }
}

fn mission_progress_from_json(value: Option<&Value>, total_steps: usize) -> MissionProgressState {
    let mut progress = MissionProgressState::new(total_steps);
    let Some(object) = value.and_then(Value::as_object) else {
        return progress;
    };
    if let Some(phase) = object.get("phase").and_then(Value::as_str) {
        progress.phase = phase.to_string();
    }
    progress.current_wave = object.get("current_wave").and_then(value_usize);
    progress.total_steps = object
        .get("total_steps")
        .and_then(value_usize)
        .unwrap_or(total_steps);
    for (key, target) in [
        ("completed_steps", &mut progress.completed_steps),
        ("active_steps", &mut progress.active_steps),
        ("succeeded", &mut progress.succeeded),
        ("refused", &mut progress.refused),
        ("blocked", &mut progress.blocked),
        ("cancelled", &mut progress.cancelled),
        ("required_failures", &mut progress.required_failures),
        ("returned_bytes", &mut progress.returned_bytes),
    ] {
        if let Some(value) = object.get(key).and_then(value_usize) {
            *target = value;
        }
    }
    progress.trace_sequence = object.get("trace_sequence").and_then(value_usize);
    progress.last_event = object
        .get("last_event")
        .and_then(Value::as_str)
        .map(str::to_string);
    progress
}

fn value_usize(value: &Value) -> Option<usize> {
    value.as_u64().and_then(|value| usize::try_from(value).ok())
}

fn unavailable_event_metrics() -> EventMetrics {
    EventMetrics {
        retained_events: 0,
        dropped_events: 0,
        subscriptions: 0,
        active_subscriptions: 0,
        pending_deliveries: 0,
        dropped_deliveries: 0,
        next_event_id: 0,
        next_delivery_id: 0,
        retained_delivery_attempts: 0,
        dropped_delivery_attempts: 0,
        next_attempt_id: 0,
    }
}

fn progress_count(report: &Value, key: &str) -> usize {
    report
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn mission_progress_json(progress: &MissionProgressState) -> Value {
    json!({
        "phase": progress.phase,
        "current_wave": progress.current_wave,
        "total_steps": progress.total_steps,
        "completed_steps": progress.completed_steps,
        "active_steps": progress.active_steps,
        "succeeded": progress.succeeded,
        "refused": progress.refused,
        "blocked": progress.blocked,
        "cancelled": progress.cancelled,
        "required_failures": progress.required_failures,
        "returned_bytes": progress.returned_bytes,
        "trace_sequence": progress.trace_sequence,
        "last_event": progress.last_event,
    })
}

fn mission_summary(state: &MissionJobState) -> Value {
    let report = state.result.as_ref();
    let completed_steps = report
        .and_then(|report| report.get("results"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let total_steps = report
        .and_then(|report| report.pointer("/plan/ordered_steps"))
        .and_then(Value::as_array)
        .map_or(state.total_steps, Vec::len);
    let count = |key: &str| {
        report
            .and_then(|report| report.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    json!({
        "total_steps": total_steps,
        "completed_steps": completed_steps,
        "succeeded": count("succeeded"),
        "refused": count("refused"),
        "blocked": count("blocked"),
        "cancelled": count("cancelled"),
        "required_failures": count("required_failures"),
        "returned_bytes": count("returned_bytes"),
        "result_available": report.is_some(),
        "result_omitted": state.result_omitted,
        "recovered_after_restart": state.recovered_after_restart,
    })
}

fn is_known_mission_status(status: &str) -> bool {
    matches!(
        status,
        "queued" | "running" | "planned" | "succeeded" | "partial" | "failed" | "cancelled"
    )
}

fn is_terminal_mission_status(status: &str) -> bool {
    matches!(
        status,
        "planned" | "succeeded" | "partial" | "failed" | "cancelled"
    )
}

fn subscription_id(
    segments: &Result<Vec<String>, crate::http::HttpError>,
    suffix: Option<&str>,
) -> Option<String> {
    let segments = segments.as_ref().ok()?;
    let expected = if suffix.is_some() { 5 } else { 4 };
    if segments.len() != expected
        || segments[0] != "v1"
        || segments[1] != "webhooks"
        || segments[2] != "subscriptions"
    {
        return None;
    }
    if let Some(suffix) = suffix {
        if segments[4] != suffix {
            return None;
        }
    }
    Some(segments[3].clone())
}

fn mission_id(
    segments: &Result<Vec<String>, crate::http::HttpError>,
    suffix: Option<&str>,
) -> Option<String> {
    let segments = segments.as_ref().ok()?;
    let expected = if suffix.is_some() { 4 } else { 3 };
    if segments.len() != expected || segments[0] != "v1" || segments[1] != "missions" {
        return None;
    }
    if let Some(suffix) = suffix {
        if segments[3] != suffix {
            return None;
        }
    }
    if segments[2].is_empty() {
        return None;
    }
    Some(segments[2].clone())
}

fn mission_id_nested(
    segments: &Result<Vec<String>, crate::http::HttpError>,
    first_suffix: &str,
    second_suffix: &str,
) -> Option<String> {
    let segments = segments.as_ref().ok()?;
    if segments.len() != 5
        || segments[0] != "v1"
        || segments[1] != "missions"
        || segments[3] != first_suffix
        || segments[4] != second_suffix
    {
        return None;
    }
    if segments[2].is_empty() {
        return None;
    }
    Some(segments[2].clone())
}

fn route_review_id(segments: &Result<Vec<String>, crate::http::HttpError>) -> Option<String> {
    let segments = segments.as_ref().ok()?;
    if segments.len() != 4
        || segments[0] != "v1"
        || segments[1] != "route-reviews"
        || segments[3] != "evidence"
    {
        return None;
    }
    Some(segments[2].clone())
}

fn delivery_receipt_id(segments: &Result<Vec<String>, crate::http::HttpError>) -> Option<String> {
    let segments = segments.as_ref().ok()?;
    if segments.len() != 4
        || segments[0] != "v1"
        || segments[1] != "delivery-receipts"
        || segments[3] != "events"
    {
        return None;
    }
    Some(segments[2].clone())
}

fn delivery_receipt_attempts_id(
    segments: &Result<Vec<String>, crate::http::HttpError>,
) -> Option<String> {
    let segments = segments.as_ref().ok()?;
    if segments.len() != 4
        || segments[0] != "v1"
        || segments[1] != "delivery-receipts"
        || segments[3] != "attempts"
    {
        return None;
    }
    Some(segments[2].clone())
}

fn query_u64(
    query: &std::collections::BTreeMap<String, String>,
    name: &str,
    default: u64,
) -> Result<u64, String> {
    query
        .get(name)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("{name} must be an unsigned integer"))
        })
        .unwrap_or(Ok(default))
}

fn query_usize(
    query: &std::collections::BTreeMap<String, String>,
    name: &str,
    default: usize,
) -> Result<usize, String> {
    query
        .get(name)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be an unsigned integer"))
        })
        .unwrap_or(Ok(default))
}

fn query_bool(
    query: &std::collections::BTreeMap<String, String>,
    name: &str,
    default: bool,
) -> Result<bool, String> {
    query
        .get(name)
        .map(|value| match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("{name} must be true or false")),
        })
        .unwrap_or(Ok(default))
}

fn strip_artifact_transport_fields(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut object = object.clone();
    object.remove("artifact_registry");
    object.remove("__isError");
    object.remove("request_id");
    Value::Object(object)
}

fn evaluator_domains_for_artifact(value: &Value) -> Vec<String> {
    let mut domains = BTreeSet::new();
    let mut add_binding = |binding: &Value| {
        if let Some(domain) = binding.get("domain").and_then(Value::as_str) {
            if !domain.trim().is_empty() {
                domains.insert(domain.to_string());
            }
        }
    };
    if let Some(bindings) = value
        .pointer("/evaluator_replay/bindings")
        .and_then(Value::as_array)
    {
        for binding in bindings {
            add_binding(binding);
        }
    }
    if let Some(claims) = value
        .pointer("/evaluator_replay/claims")
        .and_then(Value::as_array)
    {
        for claim in claims {
            if let Some(bindings) = claim.get("bindings").and_then(Value::as_array) {
                for binding in bindings {
                    add_binding(binding);
                }
            }
        }
    }
    domains.into_iter().collect()
}

fn response_status(wire: &Value) -> u16 {
    wire.get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_i64)
        .map(|code| match code {
            -32601 => 404,
            -32602 => 422,
            -32603 => 500,
            _ => 400,
        })
        .unwrap_or(200)
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpRequest;
    use std::collections::{BTreeMap, BTreeSet};

    fn request(method: &str, target: &str, body: Value) -> HttpRequest {
        HttpRequest {
            method: method.into(),
            target: target.into(),
            version: "HTTP/1.1".into(),
            headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    fn test_state_path(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_TEST_STATE: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "bioprism-api-{label}-{}-{}.json",
            std::process::id(),
            NEXT_TEST_STATE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn rest_and_json_rpc_share_tool_dispatch_and_auth_is_fail_closed() {
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                bearer_token: Some("0123456789abcdef".into()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let denied = router.handle(request("GET", "/v1/tools", json!({})));
        assert_eq!(denied.status, 401);

        let mut rest = request("POST", "/v1/tools/modality_catalog", json!({}));
        rest.headers
            .insert("authorization".into(), "Bearer 0123456789abcdef".into());
        let response = router.handle(rest);
        assert_eq!(response.status, 200);
        let value: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(router.event_metrics().retained_events, 1);

        let mut rpc = request(
            "POST",
            "/v1/rpc",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        );
        rpc.headers
            .insert("authorization".into(), "Bearer 0123456789abcdef".into());
        assert_eq!(router.handle(rpc).status, 200);
    }

    #[test]
    fn shared_router_handles_concurrent_requests_with_unique_request_ids() {
        let router = Arc::new(
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap(),
        );
        let handles = (0..32)
            .map(|_| {
                let router = Arc::clone(&router);
                std::thread::spawn(move || {
                    let response = router.handle(request("GET", "/healthz", json!({})));
                    assert_eq!(response.status, 200);
                    response.headers.get("x-request-id").cloned()
                })
            })
            .collect::<Vec<_>>();
        let ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 32);
    }

    #[test]
    fn durable_mission_state_restores_terminal_jobs_and_fails_interrupted_jobs() {
        let path = test_state_path("restart");
        let progress = mission_progress_json(&MissionProgressState::new(1));
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "schema_version": LEGACY_MISSION_STATE_SCHEMA_VERSION,
                "missions": [
                    {
                        "mission_id": "active-before-restart",
                        "total_steps": 1,
                        "status": "running",
                        "cancel_requested": false,
                        "cancel_reason": null,
                        "progress": progress,
                        "trace": [],
                        "result": null,
                        "result_omitted": null,
                        "error": null,
                        "recovered_after_restart": false
                    },
                    {
                        "mission_id": "terminal-before-restart",
                        "total_steps": 0,
                        "status": "succeeded",
                        "cancel_requested": false,
                        "cancel_reason": null,
                        "progress": mission_progress_json(&MissionProgressState::new(0)),
                        "trace": [],
                        "result": {"mission_status": "succeeded", "results": []},
                        "result_omitted": null,
                        "error": null,
                        "recovered_after_restart": false
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_state_path: Some(path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();

        let persistence = router.handle(request("GET", "/v1/missions/persistence", json!({})));
        let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
        assert_eq!(persistence["enabled"], true);
        assert_eq!(persistence["state_digest"].as_str().unwrap().len(), 64);
        assert_eq!(persistence["integrity_verified"], true);
        assert_eq!(persistence["event_log_durable"], false);
        assert_eq!(
            router
                .handle(request("POST", "/v1/missions/persistence/flush", json!({})))
                .status,
            200
        );

        let active = router.handle(request(
            "GET",
            "/v1/missions/active-before-restart",
            json!({}),
        ));
        let active: Value = serde_json::from_slice(&active.body).unwrap();
        assert_eq!(active["status"], "failed");
        assert_eq!(active["recovered_after_restart"], true);
        assert!(active["error"].as_str().unwrap().contains("not resumed"));

        let terminal = router.handle(request(
            "GET",
            "/v1/missions/terminal-before-restart",
            json!({}),
        ));
        let terminal: Value = serde_json::from_slice(&terminal.body).unwrap();
        assert_eq!(terminal["status"], "succeeded");
        assert_eq!(terminal["result"]["mission_status"], "succeeded");

        let persisted: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted["schema_version"], MISSION_STATE_SCHEMA_VERSION);
        assert_eq!(persisted["state_digest"].as_str().unwrap().len(), 64);
        let persisted_active = persisted["missions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mission| mission["mission_id"] == "active-before-restart")
            .unwrap();
        assert_eq!(persisted_active["status"], "failed");
        assert_eq!(persisted_active["recovered_after_restart"], true);
        let mut tampered = persisted;
        tampered["missions"][0]["status"] = json!("succeeded");
        std::fs::write(&path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
        let observed = router.handle(request("GET", "/v1/missions/persistence", json!({})));
        let observed: Value = serde_json::from_slice(&observed.body).unwrap();
        assert_eq!(observed["file_present"], true);
        assert_eq!(observed["integrity_verified"], false);
        let error = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_state_path: Some(path.clone()),
                ..ApiConfig::default()
            },
        )
        .err()
        .expect("tampered mission checkpoint must be rejected");
        assert!(error.contains("state_digest"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn durable_evaluator_replay_summary_survives_result_omission() {
        let path = test_state_path("evaluator-replay-summary");
        let config = ApiConfig {
            mission_state_path: Some(path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let report = json!({
            "workflow": "agent_mission",
            "plan": {"mission_id": "summary-only"},
            "mission_status": "succeeded",
            "claim_lineage": {"claims": []}
        });
        let summary = evaluator_replay_summary(&report, "summary-only").expect("replay summary");
        router.mission_jobs.lock().unwrap().insert(
            "summary-only".into(),
            Arc::new(MissionJob {
                cancellation: Arc::new(AtomicBool::new(false)),
                state: Arc::new(Mutex::new(MissionJobState {
                    total_steps: 0,
                    trace: Vec::new(),
                    progress: MissionProgressState::new(0),
                    status: "succeeded".into(),
                    cancel_requested: false,
                    cancel_reason: None,
                    result: None,
                    result_omitted: Some(json!({"bytes": 300_000, "sha256": "a".repeat(64)})),
                    evaluator_replay_summary: Some(summary),
                    route_review_provenance: None,
                    error: None,
                    recovered_after_restart: false,
                    execution_provenance: None,
                })),
            }),
        );
        router.persist_mission_registry().unwrap();

        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let response = restored.handle(request(
            "GET",
            "/v1/missions/summary-only/evaluator-replay?include_fixtures=false&max_items=32",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        let value: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(value["workflow"], "mission_evaluator_replay_query");
        assert_eq!(value["retention"]["mode"], "summary_only");
        assert_eq!(value["retention"]["result_retained"], false);
        assert_eq!(value["retention"]["summary_retained"], true);
        assert_eq!(
            value["replay"]["workflow"],
            "mission_evaluator_replay_summary"
        );
        assert_eq!(value["replay"]["coverage"]["catalogue_group_count"], 29);
        let comparison = restored.handle(request(
            "GET",
            "/v1/missions/summary-only/evaluator-replay/compare?max_items=32",
            json!({}),
        ));
        assert_eq!(comparison.status, 200);
        let comparison: Value = serde_json::from_slice(&comparison.body).unwrap();
        assert_eq!(comparison["catalog_drift"]["status"], "not_recorded");
        let bundle = restored.handle(request(
            "GET",
            "/v1/missions/summary-only/evidence-bundle?include_result=false&include_trace=false",
            json!({}),
        ));
        assert_eq!(bundle.status, 200);
        let bundle: Value = serde_json::from_slice(&bundle.body).unwrap();
        assert_eq!(bundle["retention"]["mode"], "summary_only");
        assert_eq!(bundle["result"], Value::Null);
        assert_eq!(
            bundle["evaluator_replay"]["workflow"],
            "mission_evaluator_replay_summary"
        );
        assert_eq!(bundle["bundle_digest"].as_str().unwrap().len(), 64);
        let invalid = restored.handle(request(
            "GET",
            "/v1/missions/summary-only/evaluator-replay?max_items=513",
            json!({}),
        ));
        assert_eq!(invalid.status, 422);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn durable_mission_state_omits_large_results_with_digest_metadata() {
        let state = MissionJobState {
            total_steps: 0,
            trace: Vec::new(),
            progress: MissionProgressState::new(0),
            status: "succeeded".into(),
            cancel_requested: false,
            cancel_reason: None,
            result: Some(Value::String(
                "x".repeat(MAX_PERSISTED_MISSION_RESULT_BYTES + 1),
            )),
            result_omitted: None,
            evaluator_replay_summary: None,
            route_review_provenance: None,
            error: None,
            recovered_after_restart: false,
            execution_provenance: None,
        };
        let persisted = durable_mission_state_json("large-result", &state);
        assert!(persisted["result"].is_null());
        assert_eq!(
            persisted["result_omitted"]["bytes"],
            (MAX_PERSISTED_MISSION_RESULT_BYTES + 3) as u64
        );
        assert!(persisted["result_omitted"]["sha256"].as_str().is_some());
    }

    #[test]
    fn recovery_matrix_separates_restart_boundaries_and_non_claims() {
        let mission_path = test_state_path("recovery-mission");
        let event_path = test_state_path("recovery-event");
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_state_path: Some(mission_path.clone()),
                event_state_path: Some(event_path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let response = router.handle(request("GET", "/v1/recovery", json!({})));
        assert_eq!(response.status, 200);
        let matrix: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(matrix["schema"], "bioprism-recovery-matrix/0.1");
        assert_eq!(matrix["automatic_resume"], false);
        assert_eq!(matrix["automatic_external_delivery"], false);
        let boundaries = matrix["boundaries"].as_array().unwrap();
        let mission_jobs = boundaries
            .iter()
            .find(|boundary| boundary["id"] == "mission_jobs")
            .unwrap();
        assert_eq!(mission_jobs["checkpoint_present"], true);
        assert_eq!(mission_jobs["state_digest"].as_str().unwrap().len(), 64);
        assert_eq!(mission_jobs["integrity_verified"], true);
        let event_rows = boundaries
            .iter()
            .find(|boundary| boundary["id"] == "event_rows")
            .unwrap();
        assert_eq!(event_rows["configured"], true);
        assert_eq!(event_rows["checkpoint_present"], true);
        assert_eq!(event_rows["state_digest"].as_str().unwrap().len(), 64);
        assert_eq!(event_rows["integrity_verified"], true);
        let delivery_attempts = boundaries
            .iter()
            .find(|boundary| boundary["id"] == "delivery_attempts")
            .unwrap();
        assert_eq!(delivery_attempts["configured"], true);
        assert_eq!(delivery_attempts["checkpoint_present"], true);
        assert!(delivery_attempts["restores"][0]
            .as_str()
            .unwrap()
            .contains("provenance"));
        let secrets = boundaries
            .iter()
            .find(|boundary| boundary["id"] == "webhook_signing_secrets")
            .unwrap();
        assert_eq!(secrets["checkpoint_present"], false);
        assert!(secrets["does_not_restore"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("signing secrets")));
        assert_eq!(matrix["observed"]["retained_events"], 0);
        let _ = std::fs::remove_file(mission_path);
        let _ = std::fs::remove_file(event_path);
    }

    #[test]
    fn operations_snapshot_composes_bounded_operator_evidence() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let event = router.handle(request("POST", "/v1/tools/modality_catalog", json!({})));
        assert_eq!(event.status, 200);

        let response = router.handle(request(
            "GET",
            "/v1/operations/snapshot?after=0&limit=1",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        let snapshot: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(snapshot["schema"], "bioprism-operations-snapshot/0.1");
        assert_eq!(snapshot["after"], 0);
        assert_eq!(snapshot["limit"], 1);
        assert_eq!(
            snapshot["recent_events"]["events"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(snapshot["event_metrics"]["retained_events"], 1);
        assert_eq!(snapshot["mission_summary"]["total"], 0);
        assert_eq!(snapshot["persistence"]["events"]["enabled"], false);
        assert_eq!(
            snapshot["persistence"]["workflow_reconciliations"]["enabled"],
            false
        );
        assert_eq!(snapshot["reconciliation_summary"]["registry_size"], 0);
        assert_eq!(snapshot["reconciliation_summary"]["ready_count"], 0);
        assert_eq!(
            snapshot["reconciliation_summary"]["readiness_claimed"],
            false
        );
        assert_eq!(snapshot["recovery"]["automatic_resume"], false);
        assert_eq!(
            snapshot["domain_coverage"]["schema"],
            "bioprism-domain-coverage/0.1"
        );
        assert!(snapshot["domain_coverage"]["group_count"].as_u64().unwrap() > 0);
        assert_eq!(snapshot["domain_coverage"]["truncated"], false);
        assert_eq!(snapshot["consistency"]["cross_store_atomic"], false);
        assert_eq!(snapshot["consistency"]["clock_free"], true);
        assert_eq!(snapshot["capabilities"]["operations_snapshot"], true);
        assert_eq!(
            snapshot["capabilities"]["workflow_reconciliation_registry"],
            true
        );
        assert!(snapshot["operator_actions"].as_array().unwrap().len() >= 3);
        assert!(snapshot["non_claims"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item.as_str()
                    .is_some_and(|value| value.contains("network delivery"))
            }));
    }

    #[test]
    fn operations_snapshot_rejects_unbounded_or_unknown_queries() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let too_large = router.handle(request(
            "GET",
            "/v1/operations/snapshot?limit=257",
            json!({}),
        ));
        assert_eq!(too_large.status, 422);
        let unknown = router.handle(request(
            "GET",
            "/v1/operations/snapshot?status=running",
            json!({}),
        ));
        assert_eq!(unknown.status, 400);
    }

    #[test]
    fn operations_handoff_is_content_addressed_and_non_executing() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let response = router.handle(request(
            "POST",
            "/v1/operations/handoff",
            json!({
                "goal": "prepare an oncology evidence route",
                "group_ids": ["biological_domains"],
                "max_groups": 1
            }),
        ));
        assert_eq!(response.status, 200);
        let handoff: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(handoff["workflow"], "operations_domain_handoff");
        assert_eq!(handoff["schema"], "bioprism-operations-handoff/0.1");
        assert_eq!(handoff["execution"], "not_started");
        assert_eq!(handoff["selection"]["selector_mode"], "intersection");
        assert_eq!(handoff["coverage"]["included_group_count"], 1);
        assert_eq!(handoff["groups"][0]["id"], "biological_domains");
        assert_eq!(
            handoff["route_request"]["needs"][0]["group_id"],
            "biological_domains"
        );
        assert_eq!(handoff["handoff_id"].as_str().unwrap().len(), 64);
        assert!(handoff["next_steps"].as_array().unwrap().len() >= 3);

        let unresolved = router.handle(request(
            "POST",
            "/v1/operations/handoff",
            json!({ "domains": ["not-a-real-domain"] }),
        ));
        assert_eq!(unresolved.status, 200);
        let unresolved: Value = serde_json::from_slice(&unresolved.body).unwrap();
        assert_eq!(unresolved["handoff_status"], "unresolved_domain");
        assert_eq!(
            unresolved["coverage"]["unresolved_domains"][0],
            "not-a-real-domain"
        );

        let invalid = router.handle(request(
            "POST",
            "/v1/operations/handoff",
            json!({ "unexpected": true }),
        ));
        assert_eq!(invalid.status, 422);
    }

    #[test]
    fn operations_domain_activity_separates_catalogue_from_observation() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        assert_eq!(
            router
                .handle(request("POST", "/v1/tools/modality_catalog", json!({})))
                .status,
            200
        );
        let response = router.handle(request(
            "GET",
            "/v1/operations/domains?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        let activity: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(activity["workflow"], "operations_domain_activity");
        assert_eq!(
            activity["schema"],
            "bioprism-operations-domain-activity/0.1"
        );
        assert_eq!(activity["event_cursor"]["returned_events"], 1);
        assert_eq!(activity["summary"]["tool_events_scanned"], 1);
        assert_eq!(activity["summary"]["attributed_tool_events"], 1);
        assert!(
            activity["summary"]["groups_with_observed_activity"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(activity["observation_policy"]["readiness_claimed"], false);
        assert!(activity["groups"].as_array().unwrap().iter().any(|group| {
            group["observed_tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().any(|tool| tool == "modality_catalog"))
        }));

        let invalid = router.handle(request(
            "GET",
            "/v1/operations/domains?limit=257",
            json!({}),
        ));
        assert_eq!(invalid.status, 422);
    }

    #[test]
    fn operations_domain_gates_require_separate_evidence_channels() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let completed = json!({
            "result": {
                "isError": false,
                "content": [{"type": "text", "text": "{}"}]
            }
        });
        router.record_tool_event("gate-1", "modality_catalog", &completed);
        router.record_tool_event("gate-2", "bioeval_reference_audit", &completed);
        router.record_tool_event("gate-3", "safety_release_gate", &completed);

        let response = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        let gates: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(gates["workflow"], "operations_domain_gates");
        assert_eq!(gates["schema"], "bioprism-operations-domain-gates/0.1");
        let initial_gate_digest = gates["gate_digest"].as_str().unwrap().to_owned();
        assert_eq!(gates["summary"]["tool_events_scanned"], 3);
        assert_eq!(gates["summary"]["completed_tool_events"], 3);
        assert_eq!(gates["summary"]["readiness_claimed"], false);
        assert_eq!(gates["gate_policy"]["readiness_claimed"], false);
        assert_eq!(gates["gate_digest"].as_str().unwrap().len(), 64);
        assert_eq!(
            gates["gate_digest_scope"],
            "operations_evidence_and_reconciliation_projection_without_gate_digest"
        );
        let biological = gates["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap();
        assert_eq!(
            biological["gates"]["observed_activity"]["state"],
            "observed"
        );
        assert_eq!(
            biological["gates"]["transport_completion"]["state"],
            "observed"
        );
        assert_eq!(
            biological["gates"]["evaluation_evidence"]["state"],
            "observed"
        );
        assert_eq!(biological["gates"]["safety_evidence"]["state"], "observed");
        assert_eq!(biological["gates"]["release_evidence"]["state"], "observed");
        assert_eq!(
            biological["gates"]["reconciliation_evidence"]["state"],
            "missing"
        );
        assert_eq!(biological["gate_state"], "review_required");
        assert_eq!(
            biological["gates"]["evaluation_evidence"]["scope"],
            "cross_domain_control_plane_event_page"
        );
        assert_eq!(
            biological["gates"]["domain_evaluator_evidence"]["state"],
            "observed"
        );
        assert_eq!(
            biological["gates"]["domain_evaluator_evidence"]["scope"],
            "completed_evaluator_tool_exact_or_catalogue_group_binding"
        );
        assert_eq!(biological["gates"]["artifact_evidence"]["state"], "missing");
        assert_eq!(gates["summary"]["groups_with_artifact_evidence"], 0);
        assert_eq!(
            gates["gate_policy"]["optional_evidence_gates"],
            json!(["artifact_evidence"])
        );

        let artifact = router.handle(request(
            "POST",
            "/v1/artifacts",
            json!({
                "kind": "domain_report",
                "subject_id": "gate-artifact",
                "domains": ["oncology"],
                "parent_digests": [],
                "artifact": {"status": "review"}
            }),
        ));
        assert_eq!(artifact.status, 201);
        let with_artifact = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(with_artifact.status, 200);
        let with_artifact: Value = serde_json::from_slice(&with_artifact.body).unwrap();
        let biological = with_artifact["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap();
        assert_eq!(
            biological["gates"]["artifact_evidence"]["state"],
            "observed"
        );
        assert_eq!(
            biological["gates"]["artifact_evidence"]["matching_record_count"],
            1
        );
        assert_eq!(biological["gate_state"], "review_required");
        assert!(
            with_artifact["summary"]["groups_with_artifact_evidence"]
                .as_u64()
                .unwrap()
                > 0
        );

        let mut incomplete_reconciliation = json!({
            "ok": true,
            "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
            "workflow": "domain_workflow_reconcile",
            "workflow_id": "biological_domains",
            "workflow_digest": "a".repeat(64),
            "catalog_digest": "b".repeat(64),
            "domain_contract_digest": "c".repeat(64),
            "mission_id": "gate-reconciliation-mission",
            "mission_plan_digest": "d".repeat(64),
            "source": "mission_report",
            "completion": {"status": "partial", "ready": false, "review_required": true},
            "evidence": {"evidence_valid": true},
            "integrity": {"valid": true, "finding_count": 1},
            "execution": "not_started"
        });
        incomplete_reconciliation["reconciliation_digest"] =
            json!(ContentHash::of_value(&incomplete_reconciliation)
                .unwrap()
                .to_string());
        let imported = router.handle(request(
            "POST",
            "/v1/domain-workflows/reconciliations",
            json!({"record": incomplete_reconciliation}),
        ));
        assert_eq!(imported.status, 201);
        let blocked = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(blocked.status, 200);
        let blocked: Value = serde_json::from_slice(&blocked.body).unwrap();
        let biological = blocked["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap();
        assert_eq!(
            biological["gates"]["reconciliation_evidence"]["state"],
            "incomplete"
        );
        assert_eq!(biological["gate_state"], "insufficient_evidence");
        assert_eq!(blocked["summary"]["groups_reconciliation_blocked"], 1);
        assert_ne!(
            blocked["gate_digest"].as_str(),
            Some(initial_gate_digest.as_str())
        );

        let invalid = router.handle(request("GET", "/v1/operations/gates?limit=257", json!({})));
        assert_eq!(invalid.status, 422);
    }

    #[test]
    fn operations_gate_reconciliation_matrix_binds_every_workspace_group() {
        let reconciliation_path = test_state_path("gate-reconciliation-matrix");
        let config = ApiConfig {
            reconciliation_state_path: Some(reconciliation_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let group_ids = operations_domain_coverage()["groups"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|group| group["id"].as_str())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(!group_ids.is_empty());

        for group_id in &group_ids {
            let mut record = json!({
                "ok": true,
                "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
                "workflow": "domain_workflow_reconcile",
                "workflow_id": group_id,
                "workflow_digest": "a".repeat(64),
                "catalog_digest": "b".repeat(64),
                "domain_contract_digest": "c".repeat(64),
                "mission_id": format!("gate-reconciliation-matrix-{group_id}"),
                "mission_plan_digest": "d".repeat(64),
                "source": "mission_report",
                "completion": {"status": "partial", "ready": false, "review_required": true},
                "evidence": {"evidence_valid": true},
                "integrity": {"valid": true, "finding_count": 1},
                "execution": "not_started"
            });
            record["reconciliation_digest"] =
                json!(ContentHash::of_value(&record).unwrap().to_string());
            let imported = router.handle(request(
                "POST",
                "/v1/domain-workflows/reconciliations",
                json!({"record": record}),
            ));
            assert_eq!(imported.status, 201, "failed to import {group_id}");
        }

        let response = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=256",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        let gates: Value = serde_json::from_slice(&response.body).unwrap();
        let rows = gates["groups"].as_array().unwrap();
        assert_eq!(rows.len(), group_ids.len());
        assert_eq!(
            gates["summary"]["groups_reconciliation_blocked"],
            group_ids.len()
        );
        for group_id in &group_ids {
            let group = rows
                .iter()
                .find(|group| group["id"] == *group_id)
                .unwrap_or_else(|| panic!("missing gate row for {group_id}"));
            assert_eq!(
                group["gates"]["reconciliation_evidence"]["workflow_id"],
                *group_id
            );
            assert_eq!(
                group["gates"]["reconciliation_evidence"]["state"],
                "incomplete"
            );
            assert_eq!(
                group["gates"]["reconciliation_evidence"]["readiness_claimed"],
                false
            );
            assert_eq!(group["gate_state"], "insufficient_evidence");
        }

        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_query = restored.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?limit=256",
            json!({}),
        ));
        assert_eq!(restored_query.status, 200);
        let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
        let restored_rows = restored_query["rows"].as_array().unwrap();
        assert_eq!(restored_rows.len(), group_ids.len());
        assert!(restored_rows.iter().all(|row| {
            row["completion_status"] == "partial" && row["workflow_id"].is_string()
        }));
        let _ = std::fs::remove_file(reconciliation_path);
    }

    #[test]
    fn operations_gate_reviews_are_content_addressed_replayable_and_restart_aware() {
        let path = test_state_path("gate-reviews");
        let config = ApiConfig {
            event_state_path: Some(path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let completed = json!({
            "result": {
                "isError": false,
                "content": [{"type": "text", "text": "{}"}]
            }
        });
        router.record_tool_event("review-gate-1", "modality_catalog", &completed);
        router.record_tool_event("review-gate-2", "bioeval_reference_audit", &completed);
        router.record_tool_event("review-gate-3", "safety_release_gate", &completed);
        let gates = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=256",
            json!({}),
        ));
        let gates: Value = serde_json::from_slice(&gates.body).unwrap();
        let review = router.handle(request(
            "POST",
            "/v1/operations/gate-reviews",
            json!({
                "gate_digest": gates["gate_digest"],
                "reviewer": "operator-1",
                "rationale": "reviewed bounded evidence page",
                "group_ids": ["biological_domains"],
                "accepted_gates": {
                    "biological_domains": operations_required_gates()
                }
            }),
        ));
        assert_eq!(review.status, 201);
        let review: Value = serde_json::from_slice(&review.body).unwrap();
        let review_id = review["review_id"].as_str().unwrap().to_string();
        assert_eq!(review_id.len(), 64);
        assert_eq!(review["acceptance"]["review_id"], review["review_id"]);

        let replay = router.handle(request(
            "GET",
            &format!("/v1/operations/gate-reviews?review_id={review_id}"),
            json!({}),
        ));
        assert_eq!(replay.status, 200);
        let replay: Value = serde_json::from_slice(&replay.body).unwrap();
        assert_eq!(replay["found"], true);
        assert_eq!(replay["review_count"], 1);

        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let preflight = restored.handle(request(
            "POST",
            "/v1/missions/preflight",
            json!({
                "mission_id": "review-bound-mission",
                "goal": "preview reviewed biological route",
                "steps": [{
                    "id": "catalog",
                    "domain": "biological",
                    "capability": "catalogue",
                    "objective": "inspect modality support",
                    "tool": "modality_catalog"
                }],
                "policy": {"execute": true, "allowed_tools": ["modality_catalog"]},
                "operations_gate_acceptance": review["acceptance"]
            }),
        ));
        assert_eq!(preflight.status, 200);
        let preflight: Value = serde_json::from_slice(&preflight.body).unwrap();
        assert_eq!(preflight["operations_evidence"]["review_present"], true);
        assert_eq!(preflight["operations_evidence"]["acceptance_valid"], true);
        assert_eq!(
            preflight["operations_evidence"]["decision"],
            "review_required"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn executable_missions_retain_gate_review_and_domain_evaluator_provenance() {
        let event_path = test_state_path("mission-provenance-events");
        let mission_path = test_state_path("mission-provenance-missions");
        let config = ApiConfig {
            event_state_path: Some(event_path.clone()),
            mission_state_path: Some(mission_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let completed = json!({
            "result": {
                "isError": false,
                "content": [{"type": "text", "text": "{}"}]
            }
        });
        router.record_tool_event("provenance-gate-1", "modality_catalog", &completed);
        router.record_tool_event("provenance-gate-2", "bioeval_reference_audit", &completed);
        router.record_tool_event("provenance-gate-3", "safety_release_gate", &completed);
        let gates = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=256",
            json!({}),
        ));
        let gates: Value = serde_json::from_slice(&gates.body).unwrap();
        let review = router.handle(request(
            "POST",
            "/v1/operations/gate-reviews",
            json!({
                "gate_digest": gates["gate_digest"],
                "reviewer": "operator-provenance",
                "rationale": "reviewed domain-bound evidence",
                "group_ids": ["biological_domains"],
                "accepted_gates": {"biological_domains": operations_required_gates()}
            }),
        ));
        assert_eq!(review.status, 201);
        let review: Value = serde_json::from_slice(&review.body).unwrap();

        let submitted = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "provenance-mission",
                "goal": "execute a reviewed bounded biological inspection",
                "steps": [{
                    "id": "catalog",
                    "domain": "biological",
                    "capability": "catalogue",
                    "objective": "inspect modality support",
                    "tool": "modality_catalog"
                }],
                "policy": {"execute": true, "allowed_tools": ["modality_catalog"]},
                "operations_gate_acceptance": review["acceptance"]
            }),
        ));
        assert_eq!(submitted.status, 202);
        let submitted: Value = serde_json::from_slice(&submitted.body).unwrap();
        assert_eq!(
            submitted["execution_provenance"]["schema"],
            "bioprism-mission-execution-provenance/0.1"
        );
        assert_eq!(
            submitted["execution_provenance"]["review_id"],
            review["review_id"]
        );
        assert!(submitted["execution_provenance"]["review_event_id"].is_u64());
        assert!(submitted["execution_provenance"]["accepted_event_id"].is_u64());
        assert_eq!(
            submitted["execution_provenance"]["readiness_claimed"],
            false
        );

        let provenance = router.handle(request(
            "GET",
            "/v1/missions/provenance-mission/provenance",
            json!({}),
        ));
        assert_eq!(provenance.status, 200);
        let provenance: Value = serde_json::from_slice(&provenance.body).unwrap();
        assert_eq!(
            provenance["provenance"]["gate_digest"],
            gates["gate_digest"]
        );
        let biological_evidence = provenance["provenance"]["operations_evidence"]["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["group_id"] == "biological_domains")
            .unwrap();
        assert_eq!(
            biological_evidence["gates"]["domain_evaluator_evidence"]["state"],
            "observed"
        );

        let events = router.handle(request("GET", "/v1/events?after=0&limit=32", json!({})));
        let events: Value = serde_json::from_slice(&events.body).unwrap();
        assert!(events["page"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| {
                event["event_type"] == "mission.execution.accepted"
                    && event["payload"]["provenance"]["review_id"] == review["review_id"]
            }));
        router.handle(request("POST", "/v1/missions/persistence/flush", json!({})));

        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_provenance = restored.handle(request(
            "GET",
            "/v1/missions/provenance-mission/provenance",
            json!({}),
        ));
        assert_eq!(restored_provenance.status, 200);
        let restored_provenance: Value = serde_json::from_slice(&restored_provenance.body).unwrap();
        assert_eq!(
            restored_provenance["provenance"]["provenance_digest"],
            provenance["provenance"]["provenance_digest"]
        );
        let _ = std::fs::remove_file(event_path);
        let _ = std::fs::remove_file(mission_path);
    }

    #[test]
    fn durable_event_state_restores_cursor_and_requires_secret_rebind() {
        let path = test_state_path("events");
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                event_state_path: Some(path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let created = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({
                "id": "local",
                "endpoint": "https://example.test/hook",
                "secret": "a-secret-key"
            }),
        ));
        assert_eq!(created.status, 201);
        let call = router.handle(request("POST", "/v1/tools/modality_catalog", json!({})));
        assert_eq!(call.status, 200);
        let flush = router.handle(request("POST", "/v1/events/persistence/flush", json!({})));
        assert_eq!(flush.status, 200);
        let checkpoint = std::fs::read_to_string(&path).unwrap();
        assert!(!checkpoint.contains("a-secret-key"));
        assert!(checkpoint.contains("secrets_persisted"));
        assert!(checkpoint.contains("delivery_attempts_durable"));
        let persistence = router.handle(request("GET", "/v1/events/persistence", json!({})));
        let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
        assert_eq!(persistence["enabled"], true);
        assert_eq!(persistence["schema_version"], EVENT_STATE_SCHEMA_VERSION);
        assert_eq!(persistence["state_digest"].as_str().unwrap().len(), 64);
        assert_eq!(persistence["integrity_verified"], true);
        assert_eq!(persistence["subscriptions_durable"], true);
        assert_eq!(persistence["webhook_deliveries_durable"], true);
        assert_eq!(persistence["delivery_attempts_durable"], true);
        assert_eq!(persistence["delivery_receipt_metadata_durable"], true);
        assert_eq!(persistence["secrets_persisted"], false);
        assert_eq!(router.event_metrics().retained_events, 1);
        assert_eq!(router.event_metrics().pending_deliveries, 1);
        assert_eq!(router.event_metrics().retained_delivery_attempts, 1);

        let restored = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                event_state_path: Some(path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        assert_eq!(restored.event_metrics().retained_events, 1);
        assert_eq!(restored.event_metrics().next_event_id, 2);
        assert_eq!(restored.event_metrics().subscriptions, 1);
        assert_eq!(restored.event_metrics().active_subscriptions, 0);
        assert_eq!(restored.event_metrics().pending_deliveries, 1);
        assert_eq!(restored.event_metrics().retained_delivery_attempts, 1);
        let listed = restored.handle(request("GET", "/v1/webhooks/subscriptions", json!({})));
        let listed: Value = serde_json::from_slice(&listed.body).unwrap();
        assert_eq!(listed["subscriptions"][0]["secret_bound"], false);
        assert_eq!(listed["subscriptions"][0]["rebind_required"], true);
        let pending = restored.handle(request(
            "GET",
            "/v1/webhooks/subscriptions/local/deliveries?after=0&limit=10",
            json!({}),
        ));
        let pending: Value = serde_json::from_slice(&pending.body).unwrap();
        assert_eq!(
            pending["page"]["deliveries"][0]["state"],
            "secret_rebind_required"
        );
        let attempts = restored.handle(request(
            "GET",
            "/v1/webhooks/subscriptions/local/attempts?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(attempts.status, 200);
        let attempts: Value = serde_json::from_slice(&attempts.body).unwrap();
        assert_eq!(attempts["page"]["attempts"][0]["action"], "enqueue");
        assert_eq!(attempts["page"]["attempts"][0]["outcome"], "pending");
        let old_signature = pending["page"]["deliveries"][0]["signature"]
            .as_str()
            .unwrap()
            .to_string();
        let rebind = restored.handle(request(
            "POST",
            "/v1/webhooks/subscriptions/local/rebind",
            json!({"secret": "new-secret-key"}),
        ));
        assert_eq!(rebind.status, 200);
        let rebind: Value = serde_json::from_slice(&rebind.body).unwrap();
        assert_eq!(rebind["subscription"]["secret_bound"], true);
        assert_eq!(rebind["subscription"]["rebind_required"], false);
        assert_eq!(rebind["resigned_deliveries"], 1);
        let rebound = restored.handle(request(
            "GET",
            "/v1/webhooks/subscriptions/local/deliveries?after=0&limit=10",
            json!({}),
        ));
        let rebound: Value = serde_json::from_slice(&rebound.body).unwrap();
        assert_eq!(rebound["page"]["deliveries"][0]["state"], "pending");
        assert_ne!(rebound["page"]["deliveries"][0]["signature"], old_signature);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn tampered_event_checkpoint_is_rejected_before_restore() {
        let path = test_state_path("events-tampered");
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                event_state_path: Some(path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        assert_eq!(
            router
                .handle(request("POST", "/v1/tools/modality_catalog", json!({})))
                .status,
            200
        );
        assert_eq!(
            router
                .handle(request("POST", "/v1/events/persistence/flush", json!({})))
                .status,
            200
        );
        let mut checkpoint: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        checkpoint["dropped_events"] = json!(checkpoint["dropped_events"].as_u64().unwrap() + 1);
        std::fs::write(&path, serde_json::to_vec_pretty(&checkpoint).unwrap()).unwrap();
        let observed = router.handle(request("GET", "/v1/events/persistence", json!({})));
        let observed: Value = serde_json::from_slice(&observed.body).unwrap();
        assert_eq!(observed["file_present"], true);
        assert_eq!(observed["integrity_verified"], false);

        let error = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                event_state_path: Some(path.clone()),
                ..ApiConfig::default()
            },
        )
        .err()
        .expect("tampered event checkpoint must be rejected");
        assert!(error.contains("state_digest"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn delivery_receipt_events_keep_a_bounded_join_projection_and_cursor_filter() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let subscription = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({
                "id": "receipt-sub",
                "endpoint": "https://example.test/hook",
                "secret": "a-secret-key"
            }),
        ));
        assert_eq!(subscription.status, 201);
        let output = json!({
            "ok": true,
            "workflow": "developer_delivery_receipt",
            "receipt_id": "receipt-api-1",
            "receipt_digest": "a".repeat(64),
            "valid": true,
            "receipt_ready": true,
            "release_candidate": true,
            "target_count": 1,
            "ready_target_count": 1,
            "ready_evidence_count": 2,
            "large_detail": "x".repeat(70_000)
        });
        let wire = json!({
            "jsonrpc": "2.0",
            "id": "req-1",
            "result": { "content": [{ "type": "text", "text": output.to_string() }] }
        });
        router.record_tool_event("req-1", "developer_delivery_receipt", &wire);

        let filtered = router.handle(request(
            "GET",
            "/v1/delivery-receipts/receipt-api-1/events?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(filtered.status, 200);
        let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
        assert_eq!(filtered["workflow"], "developer_delivery_receipt_events");
        assert_eq!(filtered["found"], true);
        assert_eq!(
            filtered["page"]["events"][0]["payload"]["delivery_receipt"]["receipt_id"],
            "receipt-api-1"
        );
        let attempts = router.handle(request(
            "GET",
            "/v1/delivery-receipts/receipt-api-1/attempts?after=0&limit=10",
            json!({}),
        ));
        assert_eq!(attempts.status, 200);
        let attempts: Value = serde_json::from_slice(&attempts.body).unwrap();
        assert_eq!(attempts["found"], true);
        assert_eq!(
            attempts["page"]["attempts"][0]["receipt_id"],
            "receipt-api-1"
        );
        assert_eq!(
            attempts["page"]["attempts"][0]["receipt_digest"],
            "a".repeat(64)
        );
        assert_eq!(
            filtered["page"]["events"][0]["payload"]["response_omitted"],
            true
        );

        let query = router.handle(request(
            "GET",
            "/v1/events?after=0&limit=10&receipt_id=receipt-api-1",
            json!({}),
        ));
        assert_eq!(query.status, 200);
        let query: Value = serde_json::from_slice(&query.body).unwrap();
        assert_eq!(query["page"]["events"].as_array().unwrap().len(), 1);

        let conflict = router.handle(request(
            "GET",
            "/v1/events?after=0&limit=10&review_id=a&receipt_id=receipt-api-1",
            json!({}),
        ));
        assert_eq!(conflict.status, 400);
    }

    #[test]
    fn webhook_lifecycle_is_cursor_based_and_secrets_do_not_return() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let created = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({ "id": "local", "endpoint": "https://example.test/hook", "secret": "a-secret-key", "events": ["tool.completed"] }),
        ));
        assert_eq!(created.status, 201);
        assert!(!String::from_utf8(created.body.clone())
            .unwrap()
            .contains("a-secret-key"));
        let mut call = request("POST", "/v1/tools/modality_catalog", json!({}));
        let response = router.handle(call.clone());
        assert_eq!(response.status, 200);
        call.target = "/v1/webhooks/subscriptions/local/deliveries".into();
        call.method = "GET".into();
        call.body.clear();
        call.headers.remove("content-type");
        let deliveries = router.handle(call);
        assert_eq!(deliveries.status, 200);
        let value: Value = serde_json::from_slice(&deliveries.body).unwrap();
        assert_eq!(value["page"]["deliveries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn webhook_failures_are_inspectable_and_replay_resets_the_attempt_budget() {
        struct PermanentSender;
        impl DeliverySender for PermanentSender {
            fn send(
                &mut self,
                _endpoint: &str,
                _envelope: &Value,
            ) -> Result<(), crate::events::DeliverySendError> {
                Err(crate::events::DeliverySendError::permanent(
                    "operator blocked egress",
                ))
            }
        }

        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let created = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({ "id": "replayable", "endpoint": "https://example.test/hook", "secret": "a-secret-key" }),
        ));
        assert_eq!(created.status, 201);
        assert_eq!(
            router
                .handle(request("POST", "/v1/tools/modality_catalog", json!({})))
                .status,
            200
        );
        let report = router.deliver_once(&mut PermanentSender, 10).unwrap();
        assert_eq!(report.failed, 1);
        let deliveries = router.handle(request(
            "GET",
            "/v1/webhooks/subscriptions/replayable/deliveries?after=0&limit=10",
            json!({}),
        ));
        let deliveries: Value = serde_json::from_slice(&deliveries.body).unwrap();
        assert_eq!(deliveries["page"]["deliveries"][0]["state"], "failed");
        assert_eq!(
            deliveries["page"]["deliveries"][0]["last_error_retryable"],
            false
        );
        let replay = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions/replayable/replay",
            json!({ "delivery_ids": [1] }),
        ));
        assert_eq!(replay.status, 200);
        let replay: Value = serde_json::from_slice(&replay.body).unwrap();
        assert_eq!(replay["replayed"][0]["state"], "pending");
        assert_eq!(replay["replayed"][0]["attempt"], 1);
    }

    #[test]
    fn mission_execution_trace_survives_rest_and_event_projection() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let response = router.handle(request(
            "POST",
            "/v1/tools/agent_mission",
            json!({
                "mission_id": "api-trace-1",
                "goal": "inspect the trace contract",
                "steps": [
                    {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}
                ]
            }),
        ));
        assert_eq!(response.status, 200);
        let value: Value = serde_json::from_slice(&response.body).unwrap();
        let trace: Value = serde_json::from_str(
            value["mcp"]["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(trace["execution_trace"][0]["event"], "mission.started");
        assert_eq!(trace["execution_trace"][1]["event"], "mission.completed");

        let events = router.handle(request("GET", "/v1/events?after=0&limit=10", json!({})));
        assert_eq!(events.status, 200);
        let page: Value = serde_json::from_slice(&events.body).unwrap();
        assert_eq!(page["page"]["events"].as_array().unwrap().len(), 1);
        let projected: Value = page["page"]["events"][0]["payload"]["response"].clone();
        assert_eq!(projected["result"]["isError"], false);
        let projected_trace: Value =
            serde_json::from_str(projected["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            projected_trace["execution_trace_schema_version"],
            "bioprism-devplat-mission-trace/0.1"
        );
    }

    #[test]
    fn route_review_evidence_is_queryable_by_content_addressed_id() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let review_id = "a".repeat(64);
        router.record_tool_event(
            "review-request",
            "capability_route_review",
            &json!({
                "result": {
                    "structuredContent": {
                        "workflow": "capability_route_review",
                        "review_id": review_id.clone(),
                    }
                }
            }),
        );

        let filtered = router.handle(request(
            "GET",
            &format!("/v1/events?after=0&limit=10&review_id={review_id}"),
            json!({}),
        ));
        assert_eq!(filtered.status, 200);
        let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
        assert_eq!(filtered["page"]["events"].as_array().unwrap().len(), 1);
        assert_eq!(
            filtered["page"]["events"][0]["request_id"],
            "review-request"
        );

        let evidence = router.handle(request(
            "GET",
            &format!("/v1/route-reviews/{review_id}/evidence?after=0&limit=10"),
            json!({}),
        ));
        assert_eq!(evidence.status, 200);
        let evidence: Value = serde_json::from_slice(&evidence.body).unwrap();
        assert_eq!(evidence["workflow"], "capability_route_review_evidence");
        assert_eq!(evidence["review_id"], review_id);
        assert_eq!(evidence["found"], true);
        assert_eq!(evidence["page"]["events"].as_array().unwrap().len(), 1);

        let invalid = router.handle(request(
            "GET",
            "/v1/route-reviews/not-a-review/evidence",
            json!({}),
        ));
        assert_eq!(invalid.status, 400);
    }

    #[test]
    fn asynchronous_missions_validate_poll_and_reject_duplicate_ids() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let subscription = router.handle(request(
            "POST",
            "/v1/webhooks/subscriptions",
            json!({
                "id": "mission-events",
                "endpoint": "https://example.test/mission-events",
                "events": ["mission.trace"],
                "secret": "0123456789abcdef"
            }),
        ));
        assert_eq!(subscription.status, 201);
        let body = json!({
            "mission_id": "api-async-1",
            "goal": "plan an asynchronous cross-domain mission",
            "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}],
            "claim_requests": [{"id": "catalog-observed", "claim": "The catalogue response was returned by the named tool.", "domains": ["workspace"], "requires_steps": ["catalog"], "evidence_mode": "successful_tool_result"}]
        });
        let submitted = router.handle(request("POST", "/v1/missions", body.clone()));
        assert_eq!(submitted.status, 202);
        let duplicate = router.handle(request("POST", "/v1/missions", body));
        assert_eq!(duplicate.status, 409);

        let mut status = Value::Null;
        for _ in 0..100 {
            let response = router.handle(request("GET", "/v1/missions/api-async-1", json!({})));
            assert_eq!(response.status, 200);
            status = serde_json::from_slice(&response.body).unwrap();
            if status["status"] == "planned" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(status["status"], "planned");
        assert_eq!(status["result"]["mission_status"], "planned");
        assert_eq!(status["progress"]["phase"], "planned");
        assert_eq!(status["progress"]["total_steps"], 1);
        assert_eq!(status["progress"]["completed_steps"], 0);
        assert_eq!(status["progress"]["last_event"], "mission.completed");
        assert_eq!(
            status["result"]["claim_lineage"]["claims"][0]["id"],
            "catalog-observed"
        );
        assert_eq!(
            status["result"]["claim_lineage"]["claims"][0]["claimable"],
            false
        );
        let claims = router.handle(request("GET", "/v1/missions/api-async-1/claims", json!({})));
        assert_eq!(claims.status, 200);
        let claims: Value = serde_json::from_slice(&claims.body).unwrap();
        assert_eq!(
            claims["schema"],
            "bioprism-mission-claim-lineage-response/0.1"
        );
        assert_eq!(claims["claim_lineage"]["claims"][0]["claimable"], false);
        let replay = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/evaluator-replay?include_fixtures=false&max_items=64",
            json!({}),
        ));
        assert_eq!(replay.status, 200);
        let replay: Value = serde_json::from_slice(&replay.body).unwrap();
        assert_eq!(replay["workflow"], "mission_evaluator_replay_query");
        assert_eq!(replay["retention"]["mode"], "full");
        assert_eq!(replay["replay"]["workflow"], "mission_evaluator_replay");
        assert_eq!(replay["replay"]["fixtures"], json!([]));
        assert_eq!(replay["replay"]["omitted_fixtures"], 29);
        let invalid_replay_query = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/evaluator-replay?include_fixtures=maybe",
            json!({}),
        ));
        assert_eq!(invalid_replay_query.status, 422);
        let comparison = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/evaluator-replay/compare?include_fixtures=false&max_items=32",
            json!({}),
        ));
        assert_eq!(comparison.status, 200);
        let comparison: Value = serde_json::from_slice(&comparison.body).unwrap();
        assert_eq!(comparison["workflow"], "mission_evaluator_replay_compare");
        assert_eq!(comparison["catalog_drift"]["status"], "not_recorded");
        let bundle = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/evidence-bundle?include_result=false&include_trace=false&max_items=32",
            json!({}),
        ));
        assert_eq!(bundle.status, 200);
        let bundle: Value = serde_json::from_slice(&bundle.body).unwrap();
        assert_eq!(bundle["workflow"], "mission_evidence_bundle_export");
        assert_eq!(bundle["retention"]["mode"], "full");
        assert_eq!(bundle["result"], Value::Null);
        assert_eq!(bundle["trace"], json!([]));
        assert_eq!(bundle["bundle_digest"].as_str().unwrap().len(), 64);
        let verification = router.handle(request(
            "POST",
            "/v1/evidence-bundles/verify",
            json!({"bundle": bundle.clone()}),
        ));
        assert_eq!(verification.status, 200);
        let verification: Value = serde_json::from_slice(&verification.body).unwrap();
        assert_eq!(verification["workflow"], "mission_evidence_bundle_verify");
        assert_eq!(verification["valid"], true);
        let mut tampered = bundle;
        tampered["catalog_drift"]["status"] = json!("drifted");
        let tampered_response = router.handle(request(
            "POST",
            "/v1/evidence-bundles/verify",
            json!({"bundle": tampered}),
        ));
        assert_eq!(tampered_response.status, 200);
        let tampered: Value = serde_json::from_slice(&tampered_response.body).unwrap();
        assert_eq!(tampered["valid"], false);
        let invalid_bundle = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/evidence-bundle?include_result=maybe",
            json!({}),
        ));
        assert_eq!(invalid_bundle.status, 422);
        let trace = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/trace?after=0&limit=64",
            json!({}),
        ));
        assert_eq!(trace.status, 200);
        let trace: Value = serde_json::from_slice(&trace.body).unwrap();
        assert_eq!(
            trace["trace_schema_version"],
            "bioprism-devplat-mission-trace/0.1"
        );
        assert_eq!(trace["events"][0]["event"], "mission.started");
        assert_eq!(trace["events"][0]["sequence"], 0);
        assert_eq!(
            trace["events"].as_array().unwrap().last().unwrap()["event"],
            "mission.completed"
        );
        assert_eq!(trace["gap"], false);
        assert_eq!(trace["terminal"], true);
        let events = router.handle(request("GET", "/v1/events?after=0&limit=64", json!({})));
        assert_eq!(events.status, 200);
        let events: Value = serde_json::from_slice(&events.body).unwrap();
        let event_rows = events["page"]["events"].as_array().unwrap();
        assert!(!event_rows.is_empty());
        assert!(event_rows
            .iter()
            .all(|event| event["event_type"] == "mission.trace"));
        assert_eq!(
            event_rows[0]["payload"]["trace"]["event"],
            "mission.started"
        );
        let deliveries = router.handle(request(
            "GET",
            "/v1/webhooks/subscriptions/mission-events/deliveries?after=0&limit=64",
            json!({}),
        ));
        assert_eq!(deliveries.status, 200);
        let deliveries: Value = serde_json::from_slice(&deliveries.body).unwrap();
        assert_eq!(deliveries["page"]["pending_count"], event_rows.len());
        let next_after = trace["next_after"].as_u64().unwrap();
        let empty_trace = router.handle(request(
            "GET",
            &format!("/v1/missions/api-async-1/trace?after={next_after}&limit=64"),
            json!({}),
        ));
        assert_eq!(empty_trace.status, 200);
        let empty_trace: Value = serde_json::from_slice(&empty_trace.body).unwrap();
        assert_eq!(empty_trace["events"].as_array().unwrap().len(), 0);
        let invalid_trace = router.handle(request(
            "GET",
            "/v1/missions/api-async-1/trace?unexpected=value",
            json!({}),
        ));
        assert_eq!(invalid_trace.status, 400);
        let inventory = router.handle(request(
            "GET",
            "/v1/missions?status=planned&limit=1",
            json!({}),
        ));
        assert_eq!(inventory.status, 200);
        let inventory: Value = serde_json::from_slice(&inventory.body).unwrap();
        assert_eq!(inventory["returned"], 1);
        assert_eq!(inventory["total_matching"], 1);
        assert_eq!(inventory["missions"][0]["mission_id"], "api-async-1");
        assert_eq!(inventory["missions"][0]["summary"]["total_steps"], 1);
        assert_eq!(inventory["missions"][0]["progress"]["phase"], "planned");
        assert_eq!(inventory["missions"][0]["progress"]["total_steps"], 1);
        assert_eq!(
            inventory["missions"][0]["summary"]["result_available"],
            true
        );
        let invalid_query =
            router.handle(request("GET", "/v1/missions?unexpected=value", json!({})));
        assert_eq!(invalid_query.status, 400);
        let cancel = router.handle(request(
            "POST",
            "/v1/missions/api-async-1/cancel",
            json!({"reason": "too late"}),
        ));
        assert_eq!(cancel.status, 409);
        let deleted = router.handle(request("DELETE", "/v1/missions/api-async-1", json!({})));
        assert_eq!(deleted.status, 200);
        let missing = router.handle(request("GET", "/v1/missions/api-async-1", json!({})));
        assert_eq!(missing.status, 404);
    }

    #[test]
    fn durable_mission_queue_checkpoint_tracks_commit_and_survives_restart() {
        let queue_path = test_state_path("mission-queue");
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_queue_state_path: Some(queue_path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let submitted = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "durable-queue-1",
                "goal": "exercise the durable mission queue",
                "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities", "arguments": {}, "depends_on": [], "bindings": [], "required": true}],
                "route_review": {
                    "ok": true,
                    "workflow": "capability_route_review",
                    "review_id": "a".repeat(64),
                    "route_id": "b".repeat(64),
                    "catalog_digest": "c".repeat(64),
                    "goal": "exercise the durable mission queue",
                    "findings": [],
                    "review_status": "ready",
                    "handoff_status": "mission_preflight_required",
                    "mission_draft": {
                        "goal": "exercise the durable mission queue",
                        "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities", "arguments": {}, "depends_on": [], "bindings": [], "required": true}],
                        "dependency_waves": [["catalog"]]
                    },
                    "execution": "not_started"
                }
            }),
        ));
        assert_eq!(submitted.status, 202);
        let mut mission = Value::Null;
        for _ in 0..100 {
            let response = router.handle(request("GET", "/v1/missions/durable-queue-1", json!({})));
            mission = serde_json::from_slice(&response.body).unwrap();
            if mission["status"] == "planned" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(mission["status"], "planned");
        assert_eq!(mission["queue"]["state"], "succeeded");
        assert_eq!(mission["queue"]["attempts"], 1);
        assert_eq!(
            mission["route_review_provenance"]["route_id"],
            "b".repeat(64)
        );

        let queue = router.handle(request("GET", "/v1/missions/queue", json!({})));
        assert_eq!(queue.status, 200);
        let queue: Value = serde_json::from_slice(&queue.body).unwrap();
        assert_eq!(queue["queue"]["enabled"], true);
        assert_eq!(queue["queue"]["file_present"], true);
        assert_eq!(queue["queue"]["integrity_verified"], true);
        assert_eq!(queue["queue"]["jobs"][0]["spec_returned"], false);
        assert_eq!(queue["queue"]["jobs"][0]["state"], "succeeded");
        assert_eq!(
            queue["queue"]["jobs"][0]["route_review_provenance"]["review_id"],
            "a".repeat(64)
        );
        assert_eq!(queue["queue"]["state_digest"].as_str().unwrap().len(), 64);
        assert_eq!(queue["queue"]["authority"]["configured"], true);
        assert_eq!(queue["queue"]["authority"]["integrity_verified"], true);
        assert!(queue["queue"]["authority"]["revision"].as_u64().unwrap() >= 1);
        assert_eq!(
            queue["queue"]["authority"]["event_count"],
            queue["queue"]["authority"]["revision"]
        );
        drop(router);

        let restored = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_queue_state_path: Some(queue_path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let restored_queue =
            restored.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
        assert_eq!(restored_queue.status, 200);
        let restored_queue: Value = serde_json::from_slice(&restored_queue.body).unwrap();
        assert_eq!(restored_queue["integrity_verified"], true);
        assert_eq!(restored_queue["jobs"][0]["state"], "succeeded");
        assert_eq!(
            restored_queue["jobs"][0]["route_review_provenance"]["catalog_digest"],
            "c".repeat(64)
        );
        assert_eq!(
            restored_queue["authority"]["queue_state_digest"],
            restored_queue["state_digest"]
        );
        let _ = std::fs::remove_file(queue_path);
    }

    #[test]
    fn mission_queue_authority_reports_and_audits_explicit_orphan_lock_release() {
        let queue_path = test_state_path("mission-queue-lock-release");
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_queue_state_path: Some(queue_path.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let lock_path = queue_path.with_file_name(format!(
            ".{}.authority-lock",
            queue_path.file_name().unwrap().to_string_lossy()
        ));
        std::fs::create_dir_all(&lock_path).unwrap();
        std::fs::write(
            lock_path.join("owner.json"),
            serde_json::to_vec(&json!({
                "owner_id": "dead-api-process",
                "acquired_unix_nanos": 7
            }))
            .unwrap(),
        )
        .unwrap();
        let before = router.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
        let before: Value = serde_json::from_slice(&before.body).unwrap();
        assert_eq!(before["authority"]["lock_present"], true);
        let release = router.handle(request(
            "POST",
            "/v1/missions/queue/authority/release-lock",
            json!({
                "operator": "on-call",
                "reason": "confirmed the previous API process exited"
            }),
        ));
        assert_eq!(release.status, 200);
        let release: Value = serde_json::from_slice(&release.body).unwrap();
        assert_eq!(
            release["receipt"]["previous_owner"]["owner_id"],
            "dead-api-process"
        );
        let after = router.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
        let after: Value = serde_json::from_slice(&after.body).unwrap();
        assert_eq!(after["authority"]["lock_present"], false);
        assert!(
            after["authority"]["revision"].as_u64().unwrap()
                > before["authority"]["revision"].as_u64().unwrap()
        );
        drop(router);
        let _ = std::fs::remove_file(queue_path);
        let _ = std::fs::remove_dir_all(lock_path);
    }

    #[test]
    fn mission_queue_returns_backpressure_before_accepting_over_capacity_work() {
        let router = ApiRouter::new(
            std::env::current_dir().unwrap(),
            ApiConfig {
                mission_queue_max_jobs: 1,
                mission_queue_max_active_leases: 1,
                ..ApiConfig::default()
            },
        )
        .unwrap();
        let first = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "backpressure-1",
                "goal": "fill the bounded queue",
                "steps": [{"id": "step-1", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}]
            }),
        ));
        assert_eq!(first.status, 202);
        let second = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "backpressure-2",
                "goal": "must be refused before queue mutation",
                "steps": [{"id": "step-1", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}]
            }),
        ));
        assert_eq!(second.status, 429);
        let second: Value = serde_json::from_slice(&second.body).unwrap();
        assert_eq!(second["error"]["code"], "mission_queue_backpressure");
        let status = router.handle(request("GET", "/v1/missions/queue/persistence", json!({})));
        let status: Value = serde_json::from_slice(&status.body).unwrap();
        assert_eq!(status["admission_policy"]["max_jobs"], 1);
        assert_eq!(status["registry_size"], 1);
    }

    #[test]
    fn asynchronous_workflow_missions_checkpoint_reconciliation_before_terminal_state() {
        let event_path = test_state_path("async-workflow-events");
        let mission_path = test_state_path("async-workflow-missions");
        let reconciliation_path = test_state_path("async-workflow-reconciliation");
        let artifact_path = test_state_path("async-workflow-artifacts");
        let config = ApiConfig {
            event_state_path: Some(event_path.clone()),
            mission_state_path: Some(mission_path.clone()),
            reconciliation_state_path: Some(reconciliation_path.clone()),
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let instantiated = router.handle(request(
            "POST",
            "/v1/domain-workflows/instantiate",
            json!({
                "workflow_id": "biological_domains",
                "mission_id": "async-workflow-reconcile",
                "goal": "execute a reviewed biological workflow asynchronously",
                "steps": [{
                    "id": "catalog",
                    "domain": "biological",
                    "capability": "catalogue",
                    "objective": "inspect modality support",
                    "tool": "modality_catalog",
                    "arguments": {}
                }],
                "policy": {"execute": true}
            }),
        ));
        assert_eq!(instantiated.status, 200);
        let instantiated: Value = serde_json::from_slice(&instantiated.body).unwrap();
        let completed = json!({
            "result": {
                "isError": false,
                "content": [{"type": "text", "text": "{}"}]
            }
        });
        router.record_tool_event("async-workflow-gate-1", "modality_catalog", &completed);
        router.record_tool_event(
            "async-workflow-gate-2",
            "bioeval_reference_audit",
            &completed,
        );
        router.record_tool_event("async-workflow-gate-3", "safety_release_gate", &completed);
        let gates = router.handle(request(
            "GET",
            "/v1/operations/gates?after=0&limit=256",
            json!({}),
        ));
        assert_eq!(gates.status, 200);
        let gates: Value = serde_json::from_slice(&gates.body).unwrap();
        let review = router.handle(request(
            "POST",
            "/v1/operations/gate-reviews",
            json!({
                "gate_digest": gates["gate_digest"],
                "reviewer": "operator-async-workflow",
                "rationale": "reviewed the bounded asynchronous workflow dispatch",
                "group_ids": ["biological_domains"],
                "accepted_gates": {"biological_domains": operations_required_gates()}
            }),
        ));
        assert_eq!(review.status, 201);
        let review: Value = serde_json::from_slice(&review.body).unwrap();

        let mut mission = instantiated["mission"].clone();
        mission["operations_gate_acceptance"] = review["acceptance"].clone();
        let submitted = router.handle(request("POST", "/v1/missions", mission));
        assert_eq!(
            submitted.status,
            202,
            "{}",
            String::from_utf8_lossy(&submitted.body)
        );

        let mut terminal = Value::Null;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let response = router.handle(request(
                "GET",
                "/v1/missions/async-workflow-reconcile",
                json!({}),
            ));
            assert_eq!(response.status, 200);
            terminal = serde_json::from_slice(&response.body).unwrap();
            if ["succeeded", "failed", "cancelled"]
                .iter()
                .any(|status| terminal["status"] == *status)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(terminal["status"], "succeeded");
        assert_eq!(terminal["result"]["mission_status"], "succeeded");
        assert_eq!(
            terminal["result"]["workflow_reconciliation"]["present"],
            true
        );
        assert_eq!(
            terminal["result"]["workflow_reconciliation"]["automatic"],
            true
        );
        assert_eq!(terminal["result"]["artifact_registry"]["indexed"], true);
        let mission_artifact_digest = terminal["result"]["artifact_registry"]["content_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let reconciliation_digest = terminal["result"]["workflow_reconciliation"]
            ["reconciliation_digest"]
            .as_str()
            .unwrap()
            .to_owned();

        let persisted = std::fs::read_to_string(&reconciliation_path).unwrap();
        assert!(persisted.contains("async-workflow-reconcile"));
        let persisted_artifacts = std::fs::read_to_string(&artifact_path).unwrap();
        assert!(persisted_artifacts.contains(&mission_artifact_digest));
        let artifact_query = router.handle(request(
            "GET",
            "/v1/artifacts?kind=mission_report&subject_id=async-workflow-reconcile",
            json!({}),
        ));
        assert_eq!(artifact_query.status, 200);
        let artifact_query: Value = serde_json::from_slice(&artifact_query.body).unwrap();
        assert_eq!(artifact_query["rows"].as_array().unwrap().len(), 1);
        let query = router.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=async-workflow-reconcile&completion_status=complete",
            json!({}),
        ));
        assert_eq!(query.status, 200);
        let query: Value = serde_json::from_slice(&query.body).unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(
            query["rows"][0]["reconciliation_digest"],
            reconciliation_digest
        );

        let flush = router.handle(request("POST", "/v1/missions/persistence/flush", json!({})));
        assert_eq!(flush.status, 200);
        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_status = restored.handle(request(
            "GET",
            "/v1/missions/async-workflow-reconcile",
            json!({}),
        ));
        assert_eq!(restored_status.status, 200);
        let restored_status: Value = serde_json::from_slice(&restored_status.body).unwrap();
        assert_eq!(restored_status["status"], "succeeded");
        assert_eq!(
            restored_status["result"]["workflow_reconciliation"]["reconciliation_digest"],
            reconciliation_digest
        );
        let restored_artifact = restored.handle(request(
            "GET",
            &format!("/v1/artifacts/{mission_artifact_digest}"),
            json!({}),
        ));
        assert_eq!(restored_artifact.status, 200);
        let restored_artifact: Value = serde_json::from_slice(&restored_artifact.body).unwrap();
        assert_eq!(
            restored_artifact["record"]["content_digest"],
            mission_artifact_digest
        );
        let restored_query = restored.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=async-workflow-reconcile",
            json!({}),
        ));
        assert_eq!(restored_query.status, 200);
        let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
        assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(
            restored_query["rows"][0]["reconciliation_digest"],
            reconciliation_digest
        );
        let _ = std::fs::remove_file(event_path);
        let _ = std::fs::remove_file(mission_path);
        let _ = std::fs::remove_file(reconciliation_path);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn evidence_registry_import_is_idempotent_indexed_and_restart_safe() {
        let path = test_state_path("evidence-registry");
        let artifact_path = test_state_path("evidence-registry-artifacts");
        let config = ApiConfig {
            evidence_state_path: Some(path.clone()),
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let mut bundle = json!({
            "schema": "bioprism-api/mission-evidence-bundle/0.1",
            "workflow": "mission_evidence_bundle_export",
            "mission_id": "registry-mission",
            "retention": {"mode": "summary_only", "result_retained": false, "result_included": false},
            "result": null,
            "result_digest": null,
            "evaluator_replay": {"workflow": "mission_evaluator_replay_summary", "bindings": [{"domain": "oncology", "adapter_id": "oncoworlds.assay_fidelity"}]},
            "catalog_drift": {"status": "unchanged"},
            "trace": [],
            "export": {"format": "json", "include_result": false, "include_trace": true, "trace_included": true, "include_fixtures": false, "max_items": 16, "digest_algorithm": "sha256", "execution": "not_started"},
            "execution": "not_started"
        });
        bundle["bundle_digest"] = json!(ContentHash::of_value(&bundle).unwrap().to_string());
        let imported = router.handle(request(
            "POST",
            "/v1/evidence-bundles",
            json!({"bundle": bundle.clone()}),
        ));
        assert_eq!(imported.status, 201);
        let imported: Value = serde_json::from_slice(&imported.body).unwrap();
        assert_eq!(imported["workflow"], "mission_evidence_bundle_import");
        assert_eq!(imported["created"], true);
        assert_eq!(imported["artifact_registry"]["indexed"], true);
        assert_eq!(
            imported["artifact_registry"]["kind"],
            "mission_evidence_bundle"
        );
        let artifact_digest = imported["artifact_registry"]["content_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let digest = imported["bundle_digest"].as_str().unwrap().to_string();
        let duplicate = router.handle(request(
            "POST",
            "/v1/evidence-bundles",
            json!({"bundle": bundle}),
        ));
        assert_eq!(duplicate.status, 200);
        let duplicate: Value = serde_json::from_slice(&duplicate.body).unwrap();
        assert_eq!(duplicate["already_present"], true);
        assert_eq!(duplicate["artifact_registry"]["indexed"], true);
        assert_eq!(
            duplicate["artifact_registry"]["content_digest"],
            artifact_digest
        );
        let queried = router.handle(request(
            "GET",
            "/v1/evidence-bundles?mission_id=registry-mission&domain=oncology&limit=10",
            json!({}),
        ));
        assert_eq!(queried.status, 200);
        let queried: Value = serde_json::from_slice(&queried.body).unwrap();
        assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
        assert_eq!(queried["rows"][0]["bundle_digest"], digest);
        assert_eq!(queried["rows"][0]["domains"], json!(["oncology"]));
        let fetched = router.handle(request(
            "GET",
            &format!("/v1/evidence-bundles/{digest}"),
            json!({}),
        ));
        assert_eq!(fetched.status, 200);
        let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
        assert_eq!(fetched["bundle"]["bundle_digest"], digest);
        assert_eq!(
            router
                .handle(request(
                    "POST",
                    "/v1/evidence-bundles/persistence/flush",
                    json!({})
                ))
                .status,
            200
        );
        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let status = restored.handle(request(
            "GET",
            "/v1/evidence-bundles/persistence",
            json!({}),
        ));
        let status: Value = serde_json::from_slice(&status.body).unwrap();
        assert_eq!(status["integrity_verified"], true);
        assert_eq!(status["registry_size"], 1);
        let artifact = restored.handle(request(
            "GET",
            &format!("/v1/artifacts/{artifact_digest}"),
            json!({}),
        ));
        assert_eq!(artifact.status, 200);
        let artifact: Value = serde_json::from_slice(&artifact.body).unwrap();
        assert_eq!(artifact["record"]["content_digest"], artifact_digest);
        let restored_query = restored.handle(request(
            "GET",
            "/v1/evidence-bundles?mission_id=registry-mission&limit=10",
            json!({}),
        ));
        let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
        assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn workflow_reconciliation_registry_is_idempotent_indexed_and_restart_safe() {
        let path = test_state_path("workflow-reconciliation-registry");
        let artifact_path = test_state_path("workflow-reconciliation-artifacts");
        let config = ApiConfig {
            reconciliation_state_path: Some(path.clone()),
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let mut record = json!({
            "ok": true,
            "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
            "workflow": "domain_workflow_reconcile",
            "workflow_id": "documentation_and_knowledge",
            "workflow_digest": "a".repeat(64),
            "catalog_digest": "b".repeat(64),
            "domain_contract_digest": "c".repeat(64),
            "mission_id": "reconciliation-api-mission",
            "mission_plan_digest": "d".repeat(64),
            "source": "mission_report",
            "completion": {"status": "complete", "ready": true, "review_required": true},
            "evidence": {"evidence_valid": true},
            "integrity": {"valid": true, "finding_count": 0},
            "execution": "not_started"
        });
        record["reconciliation_digest"] =
            json!(ContentHash::of_value(&record).unwrap().to_string());
        let imported = router.handle(request(
            "POST",
            "/v1/domain-workflows/reconciliations",
            json!({"record": record.clone()}),
        ));
        assert_eq!(imported.status, 201);
        let imported: Value = serde_json::from_slice(&imported.body).unwrap();
        assert_eq!(imported["created"], true);
        assert_eq!(imported["artifact_registry"]["indexed"], true);
        let artifact_digest = imported["artifact_registry"]["content_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let digest = imported["reconciliation_digest"]
            .as_str()
            .unwrap()
            .to_string();

        let duplicate = router.handle(request(
            "POST",
            "/v1/domain-workflows/reconciliations",
            json!({"record": record}),
        ));
        assert_eq!(duplicate.status, 200);
        let duplicate: Value = serde_json::from_slice(&duplicate.body).unwrap();
        assert_eq!(duplicate["already_present"], true);
        assert_eq!(duplicate["artifact_registry"]["indexed"], true);
        assert_eq!(
            duplicate["artifact_registry"]["content_digest"],
            artifact_digest
        );
        let queried = router.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=reconciliation-api-mission&completion_status=complete&limit=10",
            json!({}),
        ));
        assert_eq!(queried.status, 200);
        let queried: Value = serde_json::from_slice(&queried.body).unwrap();
        assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
        assert_eq!(queried["rows"][0]["reconciliation_digest"], digest);
        let fetched = router.handle(request(
            "GET",
            &format!("/v1/domain-workflows/reconciliations/{digest}"),
            json!({}),
        ));
        assert_eq!(fetched.status, 200);
        let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
        assert_eq!(fetched["record"]["reconciliation_digest"], digest);
        let persistence = router.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations/persistence",
            json!({}),
        ));
        let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
        assert_eq!(persistence["enabled"], true);
        assert_eq!(persistence["integrity_verified"], true);
        let artifact = router.handle(request(
            "GET",
            &format!("/v1/artifacts/{artifact_digest}"),
            json!({}),
        ));
        assert_eq!(artifact.status, 200);

        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_query = restored.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=reconciliation-api-mission",
            json!({}),
        ));
        assert_eq!(restored_query.status, 200);
        let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
        assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(restored_query["registry_generation"], 1);
        let restored_artifact = restored.handle(request(
            "GET",
            &format!("/v1/artifacts/{artifact_digest}"),
            json!({}),
        ));
        assert_eq!(restored_artifact.status, 200);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn artifact_registry_routes_join_lineage_and_restore_only_digest_valid_records() {
        let path = test_state_path("artifact-registry");
        let config = ApiConfig {
            artifact_state_path: Some(path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let leaf = router.handle(request(
            "POST",
            "/v1/artifacts",
            json!({
                "kind": "domain_report",
                "subject_id": "leaf",
                "domains": ["oncology", "genomics"],
                "parent_digests": [],
                "artifact": {"status": "review_required"}
            }),
        ));
        assert_eq!(leaf.status, 201);
        let leaf: Value = serde_json::from_slice(&leaf.body).unwrap();
        let leaf_digest = leaf["content_digest"].as_str().unwrap().to_string();
        let root = router.handle(request(
            "POST",
            "/v1/artifacts",
            json!({
                "kind": "mission_report",
                "subject_id": "root",
                "domains": ["oncology"],
                "parent_digests": [leaf_digest, "f".repeat(64)],
                "artifact": {"status": "partial"}
            }),
        ));
        assert_eq!(root.status, 201);
        let root: Value = serde_json::from_slice(&root.body).unwrap();
        let root_digest = root["content_digest"].as_str().unwrap().to_string();
        let lineage = router.handle(request(
            "GET",
            &format!("/v1/artifacts/{root_digest}/lineage"),
            json!({}),
        ));
        assert_eq!(lineage.status, 200);
        let lineage: Value = serde_json::from_slice(&lineage.body).unwrap();
        assert_eq!(lineage["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(
            lineage["missing_parent_digests"].as_array().unwrap().len(),
            1
        );
        assert!(lineage["does_not_claim"]
            .as_array()
            .unwrap()
            .iter()
            .any(|claim| claim
                .as_str()
                .unwrap()
                .contains("causal provenance or scientific validity")));
        let query = router.handle(request(
            "GET",
            "/v1/artifacts?domain=oncology&limit=10",
            json!({}),
        ));
        assert_eq!(query.status, 200);
        let query: Value = serde_json::from_slice(&query.body).unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 2);
        let persistence = router.handle(request("GET", "/v1/artifacts/persistence", json!({})));
        let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
        assert_eq!(persistence["integrity_verified"], true);
        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_query = restored.handle(request(
            "GET",
            "/v1/artifacts?domain=oncology&limit=10",
            json!({}),
        ));
        assert_eq!(restored_query.status, 200);
        let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
        assert_eq!(restored_query["rows"].as_array().unwrap().len(), 2);
        assert_eq!(restored_query["registry_generation"], 2);
        let cross_store = restored.handle(request("GET", "/v1/artifacts/cross-store", json!({})));
        assert_eq!(cross_store.status, 200);
        let cross_store: Value = serde_json::from_slice(&cross_store.body).unwrap();
        assert_eq!(
            cross_store["workflow"],
            "artifact_registry_cross_store_audit"
        );
        assert_eq!(cross_store["consistent"], true);
        assert_eq!(
            cross_store["stores"]["artifact_registry"]["record_count"],
            2
        );
        assert_eq!(
            cross_store["stores"]["workflow_execution_evidence_registry"]["record_count"],
            0
        );
        assert_eq!(cross_store["findings"], json!([]));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn workflow_execution_evidence_api_shares_and_restores_registry() {
        let evidence_path = test_state_path("workflow-execution-evidence-registry");
        let artifact_path = test_state_path("workflow-execution-evidence-artifacts");
        let config = ApiConfig {
            artifact_state_path: Some(artifact_path.clone()),
            workflow_execution_evidence_state_path: Some(evidence_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let executed = router.handle(request(
            "POST",
            "/v1/tools/interweave_workflow_execute",
            json!({
                "workflow": "biomedical_research_data_audit",
                "problem": {
                    "actions": ["hold", "release"],
                    "models": ["safe", "unsafe"],
                    "loss": [0.0, 2.0, 2.0, 0.0]
                },
                "belief": {"mass": [0.6, 0.4]},
                "acquisitions": [{
                    "id": "screen",
                    "cost": 0.01,
                    "outcomes": [
                        {"label": "negative", "likelihood": [0.9, 0.2]},
                        {"label": "positive", "likelihood": [0.1, 0.8]}
                    ]
                }],
                "budget": 0.1,
                "max_steps": 1,
                "provider": "mcp-simulated",
                "capabilities": ["data.read", "analysis.sandbox"],
                "authorization": {"grant_id": "grant-1", "provider": "mcp-simulated"},
                "observations": [{"acquisition_id": "screen", "outcome_label": "negative"}],
                "evidence": {
                    "subject_id": "api-workflow-evidence-subject",
                    "domains": ["biomedical_research", "privacy"],
                    "parent_digests": ["a".repeat(64)]
                }
            }),
        ));
        assert_eq!(
            executed.status,
            200,
            "{}",
            String::from_utf8_lossy(&executed.body)
        );
        let envelope: Value = serde_json::from_slice(&executed.body).unwrap();
        let text = envelope["mcp"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let result: Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            result["workflow_execution_evidence"]["ok"], true,
            "{result}"
        );
        let evidence_digest = result["workflow_execution_evidence"]["evidence_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let cross_store = router.handle(request("GET", "/v1/artifacts/cross-store", json!({})));
        let cross_store: Value = serde_json::from_slice(&cross_store.body).unwrap();
        assert_eq!(
            cross_store["stores"]["workflow_execution_evidence_registry"]["record_count"],
            1
        );
        drop(router);

        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_cross_store =
            restored.handle(request("GET", "/v1/artifacts/cross-store", json!({})));
        let restored_cross_store: Value =
            serde_json::from_slice(&restored_cross_store.body).unwrap();
        assert_eq!(
            restored_cross_store["stores"]["workflow_execution_evidence_registry"]["record_count"],
            1
        );
        let fetched = restored.handle(request(
            "POST",
            "/v1/tools/interweave_workflow_execution_evidence_get",
            json!({"evidence_digest": evidence_digest}),
        ));
        assert_eq!(fetched.status, 200);
        let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
        assert!(fetched["mcp"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("review_required"));
        let _ = std::fs::remove_file(evidence_path);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn domain_report_routes_project_validate_index_and_cover_catalogue() {
        let artifact_path = test_state_path("domain-report-routes");
        let config = ApiConfig {
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
        let router = ApiRouter::new(root.clone(), config.clone()).unwrap();
        let projected = router.handle(request(
            "POST",
            "/v1/domain-reports",
            json!({
                "group_id": "biological_domains",
                "domains": ["modalities"],
                "subject_id": "api-domain-report",
                "source_tool": "modality_catalog",
                "report": {"observations": ["caller supplied"]},
                "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
            }),
        ));
        assert_eq!(projected.status, 200);
        let projected: Value = serde_json::from_slice(&projected.body).unwrap();
        assert_eq!(projected["workflow"], "domain_report_project");
        assert_eq!(projected["artifact_registry"]["indexed"], true);
        let digest = projected["artifact_registry"]["content_digest"].clone();
        let coverage = router.handle(request(
            "GET",
            "/v1/domain-reports/coverage?include_report_digests=true",
            json!({}),
        ));
        assert_eq!(coverage.status, 200);
        let coverage: Value = serde_json::from_slice(&coverage.body).unwrap();
        assert_eq!(coverage["workflow"], "domain_report_coverage");
        assert_eq!(coverage["group_count"], 30);
        assert_eq!(coverage["reported_group_count"], 1);
        assert!(coverage["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap()["report_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == &digest));
        let filtered = router.handle(request(
            "GET",
            "/v1/domain-reports/coverage?report_class=ordinary&bridge_mode=inline",
            json!({}),
        ));
        assert_eq!(filtered.status, 200);
        let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
        assert_eq!(filtered["filters"]["report_class"], "ordinary");
        assert_eq!(filtered["filters"]["bridge_mode"], "inline");
        assert_eq!(filtered["reported_group_count"], 0);
        let refused = router.handle(request(
            "POST",
            "/v1/domain-reports",
            json!({
                "group_id": "biological_domains",
                "domains": ["not_declared"],
                "subject_id": "api-domain-report-refused",
                "source_tool": "modality_catalog",
                "report": {},
                "claim_posture": {"status": "refused", "does_not_claim": ["truth"]}
            }),
        ));
        assert_eq!(refused.status, 422);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn developer_workbench_verification_route_replays_retained_report() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let digest = "a".repeat(64);
        let session = json!({
            "session_id": "api-workbench-verify",
            "owner": "agent-a",
            "goal": "verify a retained authoring handoff",
            "artifacts": [{
                "id": "artifact-1", "title": "card", "path": "card.json", "domain": "oncology",
                "capability": "verification", "state": "validated", "evidence": "reproduced", "digest": digest
            }],
            "cells": [],
            "changes": []
        });
        let ci = json!({
            "workflow": "consumer contracts", "triggers": ["pull_request"], "rust_toolchain": "stable",
            "offline": true, "checks": [{"name": "unit", "run": "cargo test -p bioprism-devplat", "required": true}]
        });
        let planned = router.handle(request(
            "POST",
            "/v1/tools/developer_workbench",
            json!({"session": session.clone(), "ci": ci.clone()}),
        ));
        assert_eq!(planned.status, 200);
        let planned: Value = serde_json::from_slice(&planned.body).unwrap();
        let retained: Value = serde_json::from_str(
            planned["mcp"]["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let verified = router.handle(request(
            "POST",
            "/v1/developer-workbench/verify",
            json!({
                "session": session,
                "report": retained,
                "ci_replay": ci,
                "policy": {"require_ci": true, "require_ci_replay": true}
            }),
        ));
        assert_eq!(verified.status, 200);
        let verified: Value = serde_json::from_slice(&verified.body).unwrap();
        assert_eq!(verified["workflow"], "developer_workbench_verify");
        assert_eq!(verified["valid"], true);
        assert_eq!(verified["status"], "verified");
        assert_eq!(verified["ci_verified"], true);
        assert_eq!(verified["execution"], "not_started");
    }

    #[test]
    fn developer_workbench_report_registry_is_queryable_and_restart_safe() {
        let path = test_state_path("workbench-registry");
        let config = ApiConfig {
            workbench_state_path: Some(path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let planned = router.handle(request(
            "POST",
            "/v1/tools/developer_workbench",
            json!({
                "session": {
                    "session_id": "api-workbench-registry",
                    "owner": "agent-a",
                    "goal": "retain a report",
                    "artifacts": [{
                        "id": "artifact-1", "title": "card", "path": "card.json",
                        "domain": "oncology", "capability": "evidence", "state": "validated",
                        "evidence": "observed", "digest": "a".repeat(64)
                    }],
                    "cells": [], "changes": []
                },
                "dashboard": {"domains": ["oncology"], "limit": 10}
            }),
        ));
        let planned: Value = serde_json::from_slice(&planned.body).unwrap();
        let retained: Value = serde_json::from_str(
            planned["mcp"]["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let imported = router.handle(request(
            "POST",
            "/v1/developer-workbench/reports",
            json!({"report": retained}),
        ));
        assert_eq!(imported.status, 201);
        let imported: Value = serde_json::from_slice(&imported.body).unwrap();
        let digest = imported["workbench_report_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let queried = router.handle(request(
            "GET",
            "/v1/developer-workbench/reports?domain=oncology&capability=evidence&limit=10",
            json!({}),
        ));
        let queried: Value = serde_json::from_slice(&queried.body).unwrap();
        assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
        assert_eq!(queried["rows"][0]["workbench_report_digest"], digest);
        let fetched = router.handle(request(
            "GET",
            &format!("/v1/developer-workbench/reports/{digest}"),
            json!({}),
        ));
        assert_eq!(fetched.status, 200);
        let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
        assert_eq!(
            fetched["report"]["schema_version"],
            "bioprism-devplat-workbench/0.1"
        );
        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let status = restored.handle(request(
            "GET",
            "/v1/developer-workbench/reports/persistence",
            json!({}),
        ));
        let status: Value = serde_json::from_slice(&status.body).unwrap();
        assert_eq!(status["integrity_verified"], true);
        assert_eq!(status["registry_size"], 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ci_provider_evidence_registry_reaudits_joins_and_restores() {
        let path = test_state_path("ci-provider-evidence-registry");
        let config = ApiConfig {
            ci_provider_evidence_state_path: Some(path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let imported = router.handle(request(
            "POST",
            "/v1/ci/provider-evidence",
            json!({
                "ci": {
                    "workflow": "api-ci",
                    "triggers": ["push"],
                    "rust_toolchain": "stable",
                    "checks": [{"name": "unit", "run": "cargo test -p bioprism-devplat", "required": true}],
                    "offline": true
                },
                "provider": "generic",
                "payload": {
                    "run_id": "api-provider-run-1",
                    "conclusion": "success",
                    "checks": [{"name": "unit", "status": "success"}]
                },
                "artifacts": [{
                    "id": "artifact-1", "kind": "test-report", "digest": "a".repeat(64),
                    "check": "unit", "run_id": "api-provider-run-1", "provider": "generic",
                    "uri": "https://example.test/artifact-1", "digest_scope": "local_response_bytes"
                }],
                "logs": [{
                    "id": "log-1", "digest": "b".repeat(64), "check": "unit",
                    "run_id": "api-provider-run-1", "provider": "generic", "truncated": false
                }],
                "attestations": [{
                    "id": "attestation-1", "subject": "artifact-1", "issuer": "test",
                    "statement_digest": "c".repeat(64), "method": "detached", "subject_digest": "a".repeat(64)
                }]
            }),
        ));
        assert_eq!(
            imported.status,
            201,
            "{}",
            String::from_utf8_lossy(&imported.body)
        );
        let imported: Value = serde_json::from_slice(&imported.body).unwrap();
        assert_eq!(imported["conformance_ready"], true);
        assert_eq!(imported["local_byte_hash_artifact_count"], 1);
        assert_eq!(imported["attestation_subject_digest_binding_count"], 1);
        assert_eq!(
            imported["artifact_record_digest"].as_str().unwrap().len(),
            64
        );
        let digest = imported["provider_evidence_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let queried = router.handle(request(
            "GET",
            "/v1/ci/provider-evidence?provider=generic&conformance_ready=true&include_records=true&min_local_byte_hash_artifacts=1&min_attestation_subject_digest_bindings=1&max_items=10",
            json!({}),
        ));
        assert_eq!(queried.status, 200);
        let queried: Value = serde_json::from_slice(&queried.body).unwrap();
        assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
        assert_eq!(queried["rows"][0]["provider_evidence_digest"], digest);
        assert_eq!(queried["rows"][0]["audit"]["artifact_count"], 1);
        assert_eq!(queried["rows"][0]["local_byte_hash_artifact_count"], 1);
        let ambiguous = router.handle(request(
            "GET",
            "/v1/ci/provider-evidence?limit=10&max_items=10",
            json!({}),
        ));
        assert_eq!(ambiguous.status, 400);
        let fetched = router.handle(request(
            "GET",
            &format!("/v1/ci/provider-evidence/{digest}"),
            json!({}),
        ));
        assert_eq!(fetched.status, 200);
        let fetched: Value = serde_json::from_slice(&fetched.body).unwrap();
        assert_eq!(fetched["audit"]["run_id"], "api-provider-run-1");
        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let status = restored.handle(request(
            "GET",
            "/v1/ci/provider-evidence/persistence",
            json!({}),
        ));
        let status: Value = serde_json::from_slice(&status.body).unwrap();
        assert_eq!(status["integrity_verified"], true);
        assert_eq!(status["registry_size"], 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn capability_dashboard_route_preserves_filters_and_refuses_unbounded_queries() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let response = router.handle(request(
            "GET",
            "/v1/capabilities/dashboard?domain=verification&max_groups=4&include_tools=true&include_gaps=false",
            json!({}),
        ));
        assert_eq!(response.status, 200);
        let payload: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(payload["workflow"], "capability_dashboard");
        assert_eq!(payload["audit"]["query"]["domain"], "verification");
        assert_eq!(payload["audit"]["query"]["max_groups"], 4);
        assert_eq!(payload["audit"]["query"]["include_tools"], true);
        assert_eq!(payload["audit"]["query"]["include_gaps"], false);
        assert_eq!(
            payload["audit"]["selected_group_count"],
            payload["audit"]["groups"].as_array().unwrap().len()
        );
        assert!(payload["audit"]["selected_group_count"].as_u64().unwrap() >= 1);
        assert_eq!(payload["audit"]["groups"][0]["readiness"], "callable");
        assert_eq!(
            payload["audit"]["groups"][0]["artifact_evidence"]["state"],
            "missing"
        );
        assert_eq!(
            payload["audit"]["groups"][0]["workflow_reconciliation_evidence"]["state"],
            "missing"
        );
        assert_eq!(
            payload["audit"]["evidence"]["groups_with_artifact_evidence"],
            0
        );
        assert_eq!(payload["evidence_digest"].as_str().unwrap().len(), 64);

        let invalid = router.handle(request(
            "GET",
            "/v1/capabilities/dashboard?max_groups=513",
            json!({}),
        ));
        assert_eq!(invalid.status, 400);
        let invalid: Value = serde_json::from_slice(&invalid.body).unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_query");
    }

    #[test]
    fn capability_route_rest_endpoints_return_raw_planning_reports() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let route = router.handle(request(
            "POST",
            "/v1/capabilities/route",
            json!({
                "goal": "audit a cross-domain evidence workflow",
                "needs": [{"id": "audit", "tool": "capability_audit"}],
                "max_candidates_per_need": 4,
                "max_tools": 8,
                "include_tools": true
            }),
        ));
        assert_eq!(route.status, 200);
        let route: Value = serde_json::from_slice(&route.body).unwrap();
        assert_eq!(route["workflow"], "capability_route");
        assert_eq!(route["needs"][0]["resolution"], "explicit");
        assert_eq!(route["execution"], "not_started");
        assert_eq!(route["evidence"]["readiness_claimed"], false);
        assert_eq!(route["evidence_digest"].as_str().unwrap().len(), 64);
        assert_eq!(
            route["evidence_digest"],
            route["evidence"]["evidence_digest"]
        );
        assert_eq!(
            route["needs"][0]["candidate_group_evidence"][0]["workflow_reconciliation_evidence"]
                ["state"],
            "missing"
        );
        assert!(route["tool_schemas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|schema| { schema["name"] == "capability_audit" }));

        let review = router.handle(request(
            "POST",
            "/v1/capabilities/route/review",
            json!({
                "route": route.clone(),
                "selections": [{
                    "need_id": "audit",
                    "tool": "capability_audit",
                    "domain": "developer_platform",
                    "capability": "capability_audit",
                    "objective": "audit the capability catalogue",
                    "arguments": {}
                }],
                "validate_schemas": true
            }),
        ));
        assert_eq!(review.status, 200);
        let review: Value = serde_json::from_slice(&review.body).unwrap();
        assert_eq!(review["workflow"], "capability_route_review");
        assert_eq!(review["review_status"], "ready");
        assert_eq!(review["execution"], "not_started");
        assert_eq!(review["evidence_binding"]["present"], true);
        assert_eq!(review["evidence_digest"], route["evidence_digest"]);
        assert_eq!(
            review["mission_draft"]["route_evidence_digest"],
            route["evidence_digest"]
        );

        let planned = router.handle(request(
            "POST",
            "/v1/capabilities/route/plan",
            json!({
                "mission_id": "route-plan-api-test",
                "route": route,
                "selections": [{
                    "need_id": "audit",
                    "tool": "capability_audit",
                    "domain": "developer_platform",
                    "capability": "capability_audit",
                    "objective": "audit the capability catalogue",
                    "arguments": {}
                }],
                "validate_schemas": true
            }),
        ));
        assert_eq!(
            planned.status,
            200,
            "{}",
            String::from_utf8_lossy(&planned.body)
        );
        let planned: Value = serde_json::from_slice(&planned.body).unwrap();
        assert_eq!(planned["workflow"], "capability_route_plan");
        assert_eq!(planned["plan_status"], "ready_for_caller_inspection");
        assert_eq!(planned["dispatch"], "not_started");
        assert_eq!(planned["preflight"]["ok"], true);
        assert_eq!(planned["preflight"]["dispatch"], "not_started");
        assert_eq!(planned["mission"]["mission_id"], "route-plan-api-test");
        assert_eq!(planned["plan_digest"].as_str().unwrap().len(), 64);
        assert_eq!(
            planned["mission"]["route_review"]["review_id"],
            review["review_id"]
        );
        assert_eq!(planned["route_id"], review["route_id"]);

        let verified = router.handle(request(
            "POST",
            "/v1/capabilities/route/plan/verify",
            json!({
                "plan": planned,
                "route": route,
                "selections": [{
                    "need_id": "audit",
                    "tool": "capability_audit",
                    "domain": "developer_platform",
                    "capability": "capability_audit",
                    "objective": "audit the capability catalogue",
                    "arguments": {}
                }]
            }),
        ));
        assert_eq!(verified.status, 200);
        let verified: Value = serde_json::from_slice(&verified.body).unwrap();
        assert_eq!(verified["workflow"], "capability_route_plan_verify");
        assert_eq!(verified["valid"], true);
        assert_eq!(verified["verification_status"], "verified");
        assert_eq!(verified["route_replay"]["status"], "matched");
        assert_eq!(verified["mission_preflight"]["status"], "matched");
        assert_eq!(verified["dispatch"], "not_started");

        let shape_only = router.handle(request(
            "POST",
            "/v1/capabilities/route/plan/verify",
            json!({"plan": planned}),
        ));
        assert_eq!(shape_only.status, 200);
        let shape_only: Value = serde_json::from_slice(&shape_only.body).unwrap();
        assert_eq!(shape_only["valid"], true);
        assert_eq!(
            shape_only["verification_status"],
            "verified_without_route_replay"
        );

        let refused = router.handle(request(
            "POST",
            "/v1/capabilities/route/plan",
            json!({
                "mission_id": "route-plan-policy-refusal",
                "route": route,
                "selections": [{
                    "need_id": "audit",
                    "tool": "capability_audit",
                    "domain": "developer_platform",
                    "capability": "capability_audit",
                    "objective": "audit the capability catalogue",
                    "arguments": {}
                }],
                "policy": {"execute": true}
            }),
        ));
        assert_eq!(refused.status, 422);
    }

    #[test]
    fn capability_route_plan_returns_a_bounded_outcome_for_every_catalogue_group() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let catalogue = bioprism_mcp::workspace_capabilities();
        let groups = catalogue.as_array().unwrap();
        assert!(
            !groups.is_empty(),
            "the cross-domain catalogue must not be empty"
        );
        for group in groups {
            let group_id = group["id"].as_str().unwrap();
            let route_response = router.handle(request(
                "POST",
                "/v1/capabilities/route",
                json!({
                    "goal": format!("prepare a bounded plan for {group_id}"),
                    "needs": [{"id": group_id, "group_id": group_id, "max_items": 1}],
                    "max_candidates_per_need": 1,
                    "max_tools": 1
                }),
            ));
            assert_eq!(route_response.status, 200, "route failed for {group_id}");
            let route: Value = serde_json::from_slice(&route_response.body).unwrap();
            let candidates = route["needs"][0]["candidate_tools"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if candidates.is_empty() {
                assert_eq!(route["unresolved_needs"][0], group_id);
                continue;
            }
            let tool = candidates[0].as_str().unwrap();
            let domain = route["needs"][0]["candidate_domains"]
                .as_array()
                .and_then(|domains| domains.first())
                .and_then(Value::as_str)
                .unwrap_or(group_id);
            let plan_response = router.handle(request(
                "POST",
                "/v1/capabilities/route/plan",
                json!({
                    "mission_id": format!("catalogue-plan-{group_id}"),
                    "route": route,
                    "selections": [{
                        "need_id": group_id,
                        "tool": tool,
                        "domain": domain,
                        "capability": group_id,
                        "objective": format!("inspect {group_id}"),
                        "arguments": {}
                    }]
                }),
            ));
            assert_eq!(plan_response.status, 200, "plan failed for {group_id}");
            let plan: Value = serde_json::from_slice(&plan_response.body).unwrap();
            assert_eq!(plan["workflow"], "capability_route_plan");
            assert_eq!(plan["dispatch"], "not_started");
            assert!(matches!(
                plan["plan_status"].as_str(),
                Some("ready_for_caller_inspection")
                    | Some("blocked_by_mission_preflight")
                    | Some("blocked_by_route_review")
            ));
        }
    }

    #[test]
    fn domain_evidence_route_harmonizes_reports_and_preserves_artifact_lineage() {
        let artifact_path = test_state_path("domain-evidence-route");
        let config = ApiConfig {
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let first = router.handle(request(
            "POST",
            "/v1/domain-reports",
            json!({
                "group_id": "biological_domains",
                "domains": ["modalities"],
                "subject_id": "api-harmonization-subject",
                "source_tool": "modality_catalog",
                "report": {"observations": ["modality contract retained"]},
                "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
            }),
        ));
        let second = router.handle(request(
            "POST",
            "/v1/domain-reports",
            json!({
                "group_id": "biological_ir_and_query",
                "domains": ["BioQL syntax"],
                "subject_id": "api-harmonization-subject",
                "source_tool": "bioql_compile",
                "report": {"observations": ["query syntax contract retained"]},
                "claim_posture": {"status": "review_required", "does_not_claim": ["execution"]}
            }),
        ));
        assert_eq!(first.status, 200);
        assert_eq!(second.status, 200);
        let first: Value = serde_json::from_slice(&first.body).unwrap();
        let second: Value = serde_json::from_slice(&second.body).unwrap();
        let harmonized = router.handle(request(
            "POST",
            "/v1/domain-evidence/harmonize",
            json!({
                "subject_id": "api-harmonization-subject",
                "claim": {"id": "api-claim-1", "statement": "opaque"},
                "reports": [first["report"].clone(), second["report"].clone()],
                "links": [
                    {"report_index": 0, "role": "supports"},
                    {"report_index": 1, "role": "qualifies", "note": "syntax is not execution"}
                ],
                "required_group_ids": ["biological_domains", "biological_ir_and_query"],
                "required_domains": ["modalities", "BioQL syntax"]
            }),
        ));
        assert_eq!(
            harmonized.status,
            200,
            "{}",
            String::from_utf8_lossy(&harmonized.body)
        );
        let harmonized: Value = serde_json::from_slice(&harmonized.body).unwrap();
        assert_eq!(harmonized["workflow"], "domain_evidence_harmonize");
        assert_eq!(
            harmonized["harmonization"]["coverage"]["traceability_state"],
            "complete"
        );
        assert_eq!(harmonized["harmonization"]["readiness_claimed"], false);
        assert_eq!(harmonized["artifact_registry"]["indexed"], true);
        assert_eq!(
            harmonized["artifact_registry"]["verification"]["method"],
            "domain_evidence_harmonization"
        );
        let artifacts = router.handle(request(
            "GET",
            "/v1/artifacts?kind=domain_evidence_harmonization&subject_id=api-harmonization-subject",
            json!({}),
        ));
        assert_eq!(artifacts.status, 200);
        let artifacts: Value = serde_json::from_slice(&artifacts.body).unwrap();
        assert_eq!(artifacts["rows"].as_array().unwrap().len(), 1);

        let coverage = router.handle(request(
            "GET",
            "/v1/domain-evidence/harmonization/coverage?subject_id=api-harmonization-subject&traceability_state=complete&include_report_digests=true",
            json!({}),
        ));
        assert_eq!(
            coverage.status,
            200,
            "{}",
            String::from_utf8_lossy(&coverage.body)
        );
        let coverage: Value = serde_json::from_slice(&coverage.body).unwrap();
        assert_eq!(
            coverage["workflow"],
            "domain_evidence_harmonization_coverage"
        );
        assert_eq!(coverage["matching_count"], 1);
        assert_eq!(
            coverage["rows"][0]["report_digests"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(coverage["rows"][0]["traceability_state"], "complete");

        let invalid_coverage = router.handle(request(
            "GET",
            "/v1/domain-evidence/harmonization/coverage?after=invalid",
            json!({}),
        ));
        assert_eq!(invalid_coverage.status, 422);

        let refused = router.handle(request(
            "POST",
            "/v1/domain-evidence/harmonize",
            json!({
                "subject_id": "api-other-subject",
                "claim": {"id": "api-claim-refused"},
                "reports": [first["report"].clone()],
                "links": [{"report_index": 0, "role": "context"}]
            }),
        ));
        assert_eq!(refused.status, 422);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn domain_evidence_intake_route_retains_raw_envelope_and_indexes_exact_digests() {
        let artifact_path = test_state_path("domain-evidence-intake-route");
        let config = ApiConfig {
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let source = router.handle(request(
            "POST",
            "/v1/domain-evidence/sources",
            json!({
                "group_id": "biological_domains",
                "domains": ["modalities"],
                "subject_id": "api-intake-subject",
                "source_tool": "modality_catalog",
                "connector_kind": "literature",
                "locator_kind": "uri",
                "locator": "https://example.org/article/1",
                "retrieval_mode": "metadata_only",
                "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
                "does_not_claim": ["retrieval occurred"]
            }),
        ));
        assert_eq!(source.status, 200);
        let source: Value = serde_json::from_slice(&source.body).unwrap();
        assert_eq!(source["workflow"], "domain_evidence_source_plan");
        assert_eq!(source["retrieval_status"], "not_started");
        assert_eq!(source["artifact_registry"]["indexed"], true);
        let source_artifacts = router.handle(request(
            "GET",
            "/v1/artifacts?kind=domain_evidence_source_plan&subject_id=api-intake-subject",
            json!({}),
        ));
        assert_eq!(source_artifacts.status, 200);
        let source_artifacts: Value = serde_json::from_slice(&source_artifacts.body).unwrap();
        assert_eq!(source_artifacts["rows"].as_array().unwrap().len(), 1);
        let response = router.handle(request(
            "POST",
            "/v1/domain-evidence/intake",
            json!({
                "group_id": "biological_domains",
                "domains": ["modalities"],
                "subject_id": "api-intake-subject",
                "source_tool": "modality_catalog",
                "request": {"modality": "single_cell"},
                "response": {"status": "bounded", "modalities": ["single_cell"]},
                "outcome": "observed",
                "source_plan_digest": source["plan_digest"].clone(),
                "claim_posture": {"status": "observed", "does_not_claim": ["truth"]}
            }),
        ));
        assert_eq!(response.status, 200);
        let response: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(response["workflow"], "domain_evidence_intake");
        assert_eq!(response["request_supplied"], true);
        assert_eq!(response["source_plan_digest"], source["plan_digest"]);
        assert_eq!(response["request_digest"].as_str().unwrap().len(), 64);
        assert_eq!(response["response_digest"].as_str().unwrap().len(), 64);
        assert_eq!(response["artifact_registry"]["indexed"], true);
        assert_eq!(
            response["artifact_registry"]["verification"]["method"],
            "domain_evidence_intake"
        );
        assert_eq!(
            response["report"]["report"]["intake"]["response"]["status"],
            "bounded"
        );
        let artifacts = router.handle(request(
            "GET",
            "/v1/artifacts?kind=domain_evidence_intake&subject_id=api-intake-subject",
            json!({}),
        ));
        assert_eq!(artifacts.status, 200);
        let artifacts: Value = serde_json::from_slice(&artifacts.body).unwrap();
        assert_eq!(artifacts["rows"].as_array().unwrap().len(), 1);
        let lineage = router.handle(request(
            "GET",
            &format!(
                "/v1/domain-evidence/lineage?content_digest={}",
                response["artifact_registry"]["content_digest"]
                    .as_str()
                    .unwrap()
            ),
            json!({}),
        ));
        assert_eq!(lineage.status, 200);
        let lineage: Value = serde_json::from_slice(&lineage.body).unwrap();
        assert_eq!(
            lineage["workflow"],
            "artifact_registry_domain_evidence_lineage"
        );
        assert_eq!(lineage["rows"].as_array().unwrap().len(), 1);
        assert_eq!(
            lineage["rows"][0]["source_plan"]["binding_state"],
            "retained_and_content_parented"
        );
        assert_eq!(
            lineage["rows"][0]["source_plan"]["content_parent_linked"],
            true
        );
        assert_eq!(lineage["rows"][0]["missing_parent_count"], 1);
        let filtered_lineage = router.handle(request(
            "GET",
            "/v1/domain-evidence/lineage?group_id=biological_domains&domain=MODALITIES&outcome=observed&max_items=1",
            json!({}),
        ));
        assert_eq!(filtered_lineage.status, 200);
        let filtered_lineage: Value = serde_json::from_slice(&filtered_lineage.body).unwrap();
        assert_eq!(filtered_lineage["rows"].as_array().unwrap().len(), 1);
        let coverage = router.handle(request(
            "GET",
            "/v1/domain-evidence/coverage?include_intake_digests=true",
            json!({}),
        ));
        assert_eq!(coverage.status, 200);
        let coverage: Value = serde_json::from_slice(&coverage.body).unwrap();
        assert_eq!(coverage["workflow"], "domain_evidence_intake_coverage");
        assert_eq!(coverage["group_count"], 30);
        assert_eq!(coverage["reported_group_count"], 1);
        assert_eq!(coverage["missing_group_count"], 29);
        assert_eq!(coverage["complete"], false);
        assert_eq!(coverage["groups_with_artifact_evidence"], 1);
        assert_eq!(coverage["artifact_evidence_records"], 2);
        let reported_group = coverage["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == "biological_domains")
            .unwrap();
        assert_eq!(
            reported_group["intake_digests"].as_array().unwrap().len(),
            1
        );
        assert_eq!(reported_group["artifact_evidence"]["state"], "observed");
        assert_eq!(
            reported_group["artifact_evidence"]["matching_record_count"],
            2
        );
        let filtered = router.handle(request(
            "GET",
            "/v1/domain-evidence/coverage?group_id=biological_domains&domain=MODALITIES",
            json!({}),
        ));
        assert_eq!(filtered.status, 200);
        let filtered: Value = serde_json::from_slice(&filtered.body).unwrap();
        assert_eq!(filtered["group_count"], 1);
        assert_eq!(filtered["reported_group_count"], 1);
        assert_eq!(filtered["complete"], true);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_artifacts = restored.handle(request(
            "GET",
            "/v1/artifacts?kind=domain_evidence_intake&subject_id=api-intake-subject",
            json!({}),
        ));
        assert_eq!(restored_artifacts.status, 200);
        let restored_artifacts: Value = serde_json::from_slice(&restored_artifacts.body).unwrap();
        assert_eq!(restored_artifacts["rows"].as_array().unwrap().len(), 1);
        let restored_lineage = restored.handle(request(
            "GET",
            &format!(
                "/v1/domain-evidence/lineage?content_digest={}",
                response["artifact_registry"]["content_digest"]
                    .as_str()
                    .unwrap()
            ),
            json!({}),
        ));
        assert_eq!(restored_lineage.status, 200);
        let restored_lineage: Value = serde_json::from_slice(&restored_lineage.body).unwrap();
        assert_eq!(restored_lineage["rows"].as_array().unwrap().len(), 1);
        assert_eq!(
            restored_lineage["rows"][0]["intake_digest"],
            response["intake_digest"]
        );

        let refused = router.handle(request(
            "POST",
            "/v1/domain-evidence/intake",
            json!({
                "group_id": "biological_domains",
                "domains": ["not-declared"],
                "subject_id": "api-intake-refused",
                "source_tool": "modality_catalog",
                "response": {},
                "outcome": "unknown",
                "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
            }),
        ));
        assert_eq!(refused.status, 422);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn domain_evidence_source_execute_route_reads_file_and_restores_intake() {
        let artifact_path = test_state_path("domain-evidence-source-execute-route");
        let config = ApiConfig {
            artifact_state_path: Some(artifact_path.clone()),
            ..ApiConfig::default()
        };
        let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
        let router = ApiRouter::new(root.clone(), config.clone()).unwrap();
        let source = router.handle(request(
            "POST",
            "/v1/domain-evidence/sources",
            json!({
                "group_id": "biological_domains",
                "domains": ["modalities"],
                "subject_id": "api-source-execute-subject",
                "source_tool": "modality_catalog",
                "connector_kind": "file",
                "locator_kind": "path",
                "locator": "fixtures/fiber-v0.1/leakage_query.json",
                "retrieval_mode": "content",
                "retrieval_policy": {"network": "disabled", "max_bytes": 65536},
                "does_not_claim": ["source truth"]
            }),
        ));
        assert_eq!(source.status, 200);
        let source: Value = serde_json::from_slice(&source.body).unwrap();
        let executed = router.handle(request(
            "POST",
            "/v1/domain-evidence/sources/execute",
            json!({"source_plan_digest": source["plan_digest"].clone()}),
        ));
        assert_eq!(
            executed.status,
            200,
            "{}",
            String::from_utf8_lossy(&executed.body)
        );
        let executed: Value = serde_json::from_slice(&executed.body).unwrap();
        assert_eq!(executed["workflow"], "domain_evidence_source_execute");
        assert_eq!(executed["outcome"], "observed");
        assert_eq!(executed["intake"]["artifact_registry"]["indexed"], true);
        assert_eq!(executed["raw_content_digest"].as_str().unwrap().len(), 64);
        let restored = ApiRouter::new(root, config).unwrap();
        let artifacts = restored.handle(request(
            "GET",
            "/v1/artifacts?kind=domain_evidence_intake&subject_id=api-source-execute-subject",
            json!({}),
        ));
        assert_eq!(artifacts.status, 200);
        let artifacts: Value = serde_json::from_slice(&artifacts.body).unwrap();
        assert_eq!(artifacts["rows"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_file(artifact_path);
    }

    #[test]
    fn domain_workflow_routes_expose_catalogue_and_scoped_preflight() {
        let reconciliation_path = test_state_path("domain-workflow-auto-reconciliation");
        let config = ApiConfig {
            reconciliation_state_path: Some(reconciliation_path.clone()),
            ..ApiConfig::default()
        };
        let router = ApiRouter::new(std::env::current_dir().unwrap(), config.clone()).unwrap();
        let catalogue = router.handle(request("GET", "/v1/domain-workflows", json!({})));
        assert_eq!(catalogue.status, 200);
        let catalogue: Value = serde_json::from_slice(&catalogue.body).unwrap();
        assert_eq!(catalogue["workflow"], "domain_workflow_catalogue");
        assert_eq!(catalogue["workflow_count"], 30);
        assert_eq!(catalogue["coverage"]["all_groups_have_workflow"], true);
        assert_eq!(
            catalogue["coverage"]["all_workflows_have_domain_contract"],
            true
        );
        assert!(catalogue["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|workflow| workflow["domain_contract"].is_object()));

        let scaffolded = router.handle(request(
            "POST",
            "/v1/domain-workflows/scaffold",
            json!({
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "api-scaffold-1",
                "goal": "prepare a repository discovery starting plan",
                "tools": ["workspace_capabilities"],
                "arguments": {"workspace_capabilities": {}}
            }),
        ));
        assert_eq!(scaffolded.status, 200);
        let scaffolded: Value = serde_json::from_slice(&scaffolded.body).unwrap();
        assert_eq!(scaffolded["workflow"], "domain_workflow_scaffold");
        assert_eq!(scaffolded["execution"], "not_started");
        assert_eq!(scaffolded["readiness_claimed"], false);
        assert_eq!(scaffolded["mission"]["policy"]["execute"], false);
        assert_eq!(scaffolded["selection"]["strategy"], "explicit_tools");
        assert_eq!(scaffolded["preflight_report"]["dispatch"], "not_started");
        assert_eq!(scaffolded["preflight_status"], "ready");

        let instantiated = router.handle(request(
            "POST",
            "/v1/domain-workflows/instantiate",
            json!({
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "api-workflow-1",
                "goal": "discover repository capabilities",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                "policy": {"execute": true}
            }),
        ));
        assert_eq!(instantiated.status, 200);
        let instantiated: Value = serde_json::from_slice(&instantiated.body).unwrap();
        assert_eq!(instantiated["workflow"], "domain_workflow_instantiate");
        assert_eq!(
            instantiated["preflight_report"]["workflow"],
            "agent_mission"
        );
        assert_eq!(instantiated["execution"], "not_started");
        assert_eq!(
            instantiated["selection"]["all_selected_tools_available"],
            true
        );
        assert_eq!(
            instantiated["evidence_plan"]["steps"][0]["step_id"],
            "catalog"
        );

        let portfolio = router.handle(request(
            "POST",
            "/v1/domain-workflows/portfolio",
            json!({
                "requests": [{
                    "workflow_id": "documentation_and_knowledge",
                    "mission_id": "api-portfolio-1",
                    "goal": "discover repository capabilities",
                    "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                    "policy": {"execute": true}
                }]
            }),
        ));
        assert_eq!(portfolio.status, 200);
        let portfolio: Value = serde_json::from_slice(&portfolio.body).unwrap();
        assert_eq!(portfolio["workflow"], "domain_workflow_portfolio");
        assert_eq!(portfolio["valid"], true);
        assert_eq!(portfolio["portfolio_ready"], true);
        assert_eq!(
            portfolio["portfolio_status"],
            "ready_for_authoritative_preflight"
        );
        assert_eq!(portfolio["summary"]["preflight_status"], "matched");
        assert_eq!(portfolio["items"][0]["status"], "instantiated");
        assert_eq!(portfolio["items"][0]["mission_preflight"]["matched"], true);
        assert_eq!(portfolio["dispatch"], "not_started");

        let portfolio_verified = router.handle(request(
            "POST",
            "/v1/domain-workflows/portfolio/verify",
            json!({
                "portfolio": portfolio.clone(),
                "replay_requests": [{
                    "workflow_id": "documentation_and_knowledge",
                    "mission_id": "api-portfolio-1",
                    "goal": "discover repository capabilities",
                    "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                    "policy": {"execute": true}
                }],
                "policy": {"require_replay": true}
            }),
        ));
        assert_eq!(portfolio_verified.status, 200);
        let portfolio_verified: Value = serde_json::from_slice(&portfolio_verified.body).unwrap();
        assert_eq!(
            portfolio_verified["workflow"],
            "domain_workflow_portfolio_verify"
        );
        assert_eq!(portfolio_verified["valid"], true);
        assert_eq!(portfolio_verified["verification_status"], "verified");
        assert_eq!(portfolio_verified["summary"]["replay_matched_count"], 1);
        assert_eq!(portfolio_verified["items"][0]["status"], "verified");
        assert_eq!(portfolio_verified["dispatch"], "not_started");

        let verified = router.handle(request(
            "POST",
            "/v1/domain-workflows/verify",
            json!({
                "instantiation": instantiated.clone(),
                "replay_request": {
                    "workflow_id": "documentation_and_knowledge",
                    "mission_id": "api-workflow-1",
                    "goal": "discover repository capabilities",
                    "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
                    "policy": {"execute": true}
                }
            }),
        ));
        assert_eq!(verified.status, 200);
        let verified: Value = serde_json::from_slice(&verified.body).unwrap();
        assert_eq!(verified["workflow"], "domain_workflow_verify");
        assert_eq!(verified["valid"], true);
        assert_eq!(verified["verification_status"], "verified");
        assert_eq!(verified["replay"]["matched"], true);
        assert_eq!(verified["mission_preflight"]["matched"], true);
        assert_eq!(verified["dispatch"], "not_started");

        let executed = router.handle(request(
            "POST",
            "/v1/tools/agent_mission",
            instantiated["mission"].clone(),
        ));
        assert_eq!(executed.status, 200);
        let executed: Value = serde_json::from_slice(&executed.body).unwrap();
        let mission_report: Value = serde_json::from_str(
            executed["mcp"]["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(mission_report["mission_status"], "succeeded");
        assert_eq!(mission_report["workflow_reconciliation"]["present"], true);
        assert_eq!(mission_report["workflow_reconciliation"]["automatic"], true);
        let automatic_digest = mission_report["workflow_reconciliation"]["reconciliation_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let automatic_query = router.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=api-workflow-1",
            json!({}),
        ));
        assert_eq!(automatic_query.status, 200);
        let automatic_query: Value = serde_json::from_slice(&automatic_query.body).unwrap();
        assert_eq!(automatic_query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(
            automatic_query["rows"][0]["reconciliation_digest"],
            automatic_digest
        );
        let reconciled = router.handle(request(
            "POST",
            "/v1/domain-workflows/reconcile",
            json!({"instantiation": instantiated, "mission_report": mission_report}),
        ));
        assert_eq!(reconciled.status, 200);
        let reconciled: Value = serde_json::from_slice(&reconciled.body).unwrap();
        assert_eq!(reconciled["workflow"], "domain_workflow_reconcile");
        assert_eq!(reconciled["completion"]["status"], "complete");
        assert_eq!(reconciled["completion"]["ready"], true);

        let refused = router.handle(request(
            "POST",
            "/v1/domain-workflows/instantiate",
            json!({
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "api-workflow-refused",
                "goal": "refuse cross-group selection",
                "steps": [{"id": "compile", "tool": "bioql_compile"}]
            }),
        ));
        assert_eq!(refused.status, 422);
        let refused: Value = serde_json::from_slice(&refused.body).unwrap();
        assert_eq!(refused["error"]["code"], "invalid_domain_workflow");
        drop(router);
        let restored = ApiRouter::new(std::env::current_dir().unwrap(), config).unwrap();
        let restored_query = restored.handle(request(
            "GET",
            "/v1/domain-workflows/reconciliations?mission_id=api-workflow-1",
            json!({}),
        ));
        assert_eq!(restored_query.status, 200);
        let restored_query: Value = serde_json::from_slice(&restored_query.body).unwrap();
        assert_eq!(restored_query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(
            restored_query["rows"][0]["reconciliation_digest"],
            automatic_digest
        );
        let _ = std::fs::remove_file(reconciliation_path);
    }

    #[test]
    fn mission_preflight_returns_authoritative_plan_without_queueing_or_dispatching() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let response = router.handle(request(
            "POST",
            "/v1/missions/preflight",
            json!({
                "mission_id": "api-preflight-1",
                "goal": "preview a cross-domain plan",
                "steps": [{
                    "id": "catalog",
                    "domain": "workspace",
                    "capability": "discovery",
                    "objective": "discover routes",
                    "tool": "workspace_capabilities"
                }],
                "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
            }),
        ));
        assert_eq!(response.status, 200);
        let body: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(body["preflight"], true);
        assert_eq!(body["dispatch"], "not_started");
        assert_eq!(body["execution"], "planned");
        assert_eq!(body["results"].as_array().unwrap().len(), 0);
        assert_eq!(
            body["operations_evidence"]["decision"],
            "insufficient_evidence"
        );
        assert_eq!(body["operations_evidence"]["acceptance_required"], true);
        assert_eq!(body["operations_evidence"]["acceptance_valid"], false);
        let reviewed = router.handle(request(
            "POST",
            "/v1/missions/preflight",
            json!({
                "mission_id": "api-route-review-1",
                "goal": "preview a reviewed route",
                "steps": [{
                    "id": "catalog",
                    "domain": "workspace",
                    "capability": "discovery",
                    "objective": "discover routes",
                    "tool": "workspace_capabilities",
                    "arguments": {},
                    "depends_on": [],
                    "bindings": [],
                    "required": true
                }],
                "route_review": {
                    "ok": true,
                    "workflow": "capability_route_review",
                    "review_id": "a".repeat(64),
                    "route_id": "b".repeat(64),
                    "catalog_digest": "c".repeat(64),
                    "goal": "preview a reviewed route",
                    "findings": [],
                    "review_status": "ready",
                    "handoff_status": "mission_preflight_required",
                    "execution": "not_started",
                    "evidence_digest": "e".repeat(64),
                    "evidence_scope": "capability_route",
                    "evidence_binding": {
                        "present": true,
                        "evidence_digest": "e".repeat(64),
                        "scope": "capability_route",
                        "summary": {"evidence_digest": "e".repeat(64), "scope": "capability_route"},
                        "posture": "carried_forward_not_recomputed",
                        "readiness_claimed": false,
                        "execution": "not_started"
                    },
                    "mission_draft": {
                        "goal": "preview a reviewed route",
                        "steps": [{
                            "id": "catalog",
                            "domain": "workspace",
                            "capability": "discovery",
                            "objective": "discover routes",
                            "tool": "workspace_capabilities",
                            "arguments": {},
                            "depends_on": [],
                            "bindings": [],
                            "required": true
                        }],
                        "dependency_waves": [["catalog"]],
                        "route_evidence_digest": "e".repeat(64),
                        "route_evidence_scope": "capability_route"
                    }
                }
            }),
        ));
        assert_eq!(reviewed.status, 200);
        let reviewed_body: Value = serde_json::from_slice(&reviewed.body).unwrap();
        assert_eq!(
            reviewed_body["plan"]["route_review_provenance"]["present"],
            true
        );
        assert_eq!(
            reviewed_body["plan"]["route_review_provenance"]["evidence_present"],
            true
        );
        let missing = router.handle(request("GET", "/v1/missions/api-preflight-1", json!({})));
        assert_eq!(missing.status, 404);

        let blocked = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "api-execute-without-gates",
                "goal": "must require reviewed domain evidence",
                "steps": [{
                    "id": "catalog",
                    "domain": "workspace",
                    "capability": "discovery",
                    "objective": "discover routes",
                    "tool": "workspace_capabilities"
                }],
                "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
            }),
        ));
        assert_eq!(blocked.status, 422);
        let blocked_body: Value = serde_json::from_slice(&blocked.body).unwrap();
        assert_eq!(
            blocked_body["error"]["code"],
            "operations_gate_acceptance_required"
        );

        let refused = router.handle(request(
            "POST",
            "/v1/missions/preflight",
            json!({
                "mission_id": "api-preflight-invalid-policy",
                "goal": "must retain execution authorization checks",
                "steps": [{
                    "id": "catalog",
                    "domain": "workspace",
                    "capability": "discovery",
                    "objective": "discover routes",
                    "tool": "workspace_capabilities"
                }],
                "policy": {"execute": true}
            }),
        ));
        assert_eq!(refused.status, 422);
        let refused_body: Value = serde_json::from_slice(&refused.body).unwrap();
        assert!(refused_body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("allow-list"));
    }

    #[test]
    fn asynchronous_mission_submission_rejects_known_tool_schema_mismatch() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let response = router.handle(request(
            "POST",
            "/v1/missions",
            json!({
                "mission_id": "api-schema-invalid",
                "goal": "refuse invalid arguments before queueing",
                "steps": [{
                    "id": "compile",
                    "domain": "fiber",
                    "capability": "compile",
                    "objective": "must be refused",
                    "tool": "fiber_compile",
                    "arguments": {"world": "fixture.json"}
                }],
                "policy": {"execute": true, "allowed_tools": ["fiber_compile"]}
            }),
        ));
        assert_eq!(response.status, 422);
        let body: Value = serde_json::from_slice(&response.body).unwrap();
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("authoritative schema validation refused"));
    }

    #[test]
    fn oversized_mission_events_keep_trace_projection_when_raw_response_is_omitted() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let report = json!({
            "execution_trace_schema_version": "bioprism-devplat-mission-trace/0.1",
            "execution_trace": [{
                "sequence": 0,
                "event": "mission.completed",
                "wave": null,
                "step_id": null,
                "tool": null,
                "status": "succeeded",
                "arguments_digest": null,
                "bytes": 0,
                "detail": null
            }],
            "mission_status": "succeeded",
            "returned_bytes": 0,
            "large_result": "x".repeat(70_000)
        });
        let wire = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": false,
                "content": [{"type": "text", "text": serde_json::to_string(&report).unwrap()}]
            }
        });
        router.record_tool_event("request-oversized", "agent_mission", &wire);
        let page = router.events.lock().unwrap().events(0, 10).unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].payload["response_omitted"], true);
        assert_eq!(
            page.events[0].payload["mission_trace"]["execution_trace"][0]["event"],
            "mission.completed"
        );
    }
}
