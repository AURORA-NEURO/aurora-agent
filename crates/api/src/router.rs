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
use std::time::{SystemTime, UNIX_EPOCH};

mod developer_artifact_routes;
mod domain_routes;
mod event_routes;
mod evidence;
mod missions;
mod operations;
mod persistence;
mod reconciliation_routes;

use persistence::{
    ArtifactPersistence, CiProviderEvidencePersistence, EventPersistence, EvidencePersistence,
    MissionPersistence, MissionQueuePersistence, ReconciliationPersistence, WorkbenchPersistence,
    WorkflowExecutionEvidencePersistence,
};

pub const API_VERSION: &str = "v1";
pub const DEFAULT_MAX_HEADER_BYTES: usize = 32 * 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_EVENT_CAPACITY: usize = 4096;
pub const MAX_MISSION_JOBS: usize = 4096;
const MISSION_REQUEST_STACK_BYTES: usize = 16 * 1024 * 1024;

fn bounded_tool_definitions() -> Vec<Value> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("bioprism-api-tool-catalogue".into())
            .stack_size(MISSION_REQUEST_STACK_BYTES)
            .spawn_scoped(scope, bioprism_mcp::tool_definitions)
            .expect("the bounded API tool catalogue thread must start");
        handle
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    })
}
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

    /// Handle one HTTP request.
    ///
    /// Mission submission and preflight validate the complete cross-domain tool catalogue and
    /// reviewed route evidence. Those frames are intentionally bounded, but are larger than the
    /// default Windows test/listener stack. Keep the API transport boundary on an explicit stack
    /// so correctness does not depend on which embedding thread called any deep domain route.
    pub fn handle(&self, request: HttpRequest) -> HttpResponse {
        let request_id = self.request_id(&request);
        std::thread::scope(|scope| {
            match std::thread::Builder::new()
                .name("bioprism-api-request".into())
                .stack_size(MISSION_REQUEST_STACK_BYTES)
                .spawn_scoped(scope, || self.handle_inner(request))
            {
                Ok(handle) => handle
                    .join()
                    .unwrap_or_else(|payload| std::panic::resume_unwind(payload)),
                Err(error) => self.finish(
                    self.error(
                        503,
                        "api_request_unavailable",
                        &format!("cannot start the API request thread: {error}"),
                        &request_id,
                    ),
                    &request_id,
                ),
            }
        })
    }

    fn handle_inner(&self, request: HttpRequest) -> HttpResponse {
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
            ("POST", "/v1/research/evolution/admit") => {
                self.bounded_evolution_admit(&request, &request_id)
            }
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

    fn bounded_evolution_admit(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let call = Request {
            id: Some(Value::String(request_id.to_string())),
            method: "tools/call".into(),
            params: json!({
                "name": "adapter_bounded_evolution",
                "arguments": arguments,
            }),
        };
        let mut server = self.server.clone();
        let Some(response) = server.handle(&call) else {
            return self.error(
                500,
                "dispatch_failed",
                "bounded evolution dispatch produced no response",
                request_id,
            );
        };
        let wire = response.to_json();
        self.record_tool_event(request_id, "adapter_bounded_evolution", &wire);
        let transport_ok = wire.get("error").is_none();
        HttpResponse::json(
            if transport_ok {
                200
            } else {
                response_status(&wire)
            },
            &json!({
                "ok": transport_ok,
                "api_version": API_VERSION,
                "feature_id": bioprism_adapter::BOUNDED_EVOLUTION_FEATURE_ID,
                "tool": "adapter_bounded_evolution",
                "request_id": request_id,
                "mcp": wire,
                "guarantees": [
                    "REST and MCP use the same bounded evolution dispatcher and receipt contract",
                    "replay, evidence, safety, policy, budget, protected-closure, and preclinical gates remain explicit",
                    "the endpoint admits receipt metadata only and never mutates or deploys candidate artifacts"
                ],
                "limitations": [
                    "sandbox execution, independent review, signing, and release governance remain outside this transport endpoint"
                ]
            }),
        )
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
                    "/v1/research/evolution/admit": { "post": { "responses": { "200": { "description": "bounded-evolution admission receipt shared with MCP" }, "400": { "description": "evolution request JSON was invalid" }, "422": { "description": "candidate admission was blocked or unresolved" } } } },
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
    let advertised_tools = bounded_tool_definitions()
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
    let Some(unsigned_object) = unsigned.as_object_mut() else {
        return Some(false);
    };
    unsigned_object.remove("state_digest");
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
        let Some(document_object) = document.as_object_mut() else {
            return Err("mission checkpoint document is not an object".into());
        };
        document_object.insert("state_digest".into(), Value::String(state_digest));
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
mod tests;
