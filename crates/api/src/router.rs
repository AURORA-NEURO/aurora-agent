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
use bioprism_mcp::{Request, Response, PROTOCOL_VERSION, SERVER_NAME};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub const API_VERSION: &str = "v1";
pub const DEFAULT_MAX_HEADER_BYTES: usize = 32 * 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_EVENT_CAPACITY: usize = 4096;
pub const MAX_MISSION_JOBS: usize = 4096;
pub const MAX_MISSION_LIST_LIMIT: usize = 256;
pub const MAX_MISSION_TRACE_EVENTS: usize = 4096;
pub const MISSION_STATE_SCHEMA_VERSION: u64 = 1;
pub const MAX_MISSION_STATE_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PERSISTED_MISSION_RESULT_BYTES: usize = 256 * 1024;
pub const MAX_PERSISTED_MISSION_TRACE_EVENT_BYTES: usize = 64 * 1024;

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
    /// Optional atomic JSON checkpoint for the bounded event cursor only.
    pub event_state_path: Option<PathBuf>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_header_bytes: DEFAULT_MAX_HEADER_BYTES,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            bearer_token: None,
            mission_state_path: None,
            event_state_path: None,
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
            .event_state_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("event_state_path must not be empty".into());
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
    event_persistence: Arc<EventPersistence>,
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

struct EventPersistence {
    path: Option<PathBuf>,
    events: Arc<Mutex<EventLog>>,
    lock: Mutex<()>,
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
        let document = json!({
            "schema_version": MISSION_STATE_SCHEMA_VERSION,
            "missions": missions,
            "guarantees": [
                "terminal reports are restored only when their bounded JSON was retained",
                "queued and running jobs are marked failed after a process restart",
                "event cursors and webhook deliveries remain process-local"
            ]
        });
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
        let temporary = path.with_file_name(format!(".{filename}.tmp"));
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
    error: Option<String>,
    recovered_after_restart: bool,
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
        let events = Arc::new(Mutex::new(EventLog::from_checkpoint_path(
            config.event_capacity,
            config.event_state_path.as_deref(),
        )?));
        let restored_jobs = load_mission_jobs(config.mission_state_path.as_deref())?;
        let mut server = bioprism_mcp::Server::new(root);
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
        let router = Self {
            server,
            mission_executor,
            config,
            events,
            next_request_id: AtomicU64::new(1),
            mission_jobs,
            mission_persistence,
            event_persistence,
        };
        if router.config.mission_state_path.is_some() {
            router.persist_mission_registry()?;
        }
        if router.config.event_state_path.is_some() {
            router.event_persistence.persist().map_err(|error| {
                format!("event state checkpoint failed during startup: {error}")
            })?;
        }
        Ok(router)
    }

    fn persist_mission_registry(&self) -> Result<(), String> {
        self.mission_persistence.persist()
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
            ("GET", "/v1/tools") => self.tools(),
            ("GET", "/v1/metrics") => self.metrics(),
            ("GET", "/v1/events") => self.events(&request),
            ("GET", "/v1/events/stream") => self.event_stream(&request),
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
            ("POST", "/v1/missions/preflight") => self.preflight_mission(&request, &request_id),
            ("POST", "/v1/missions") => self.submit_mission(&request, &request_id),
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
        let registry_size = self.mission_jobs.lock().map(|jobs| jobs.len()).unwrap_or(0);
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema_version": MISSION_STATE_SCHEMA_VERSION,
                "max_file_bytes": MAX_MISSION_STATE_FILE_BYTES,
                "max_result_bytes": MAX_PERSISTED_MISSION_RESULT_BYTES,
                "registry_size": registry_size,
                "event_log_durable": false,
                "webhook_deliveries_durable": false,
                "recovery_policy": "terminal snapshots restore; queued and running jobs fail explicitly after restart",
                "flush": "/v1/missions/persistence/flush"
            }),
        )
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
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema_version": EVENT_STATE_SCHEMA_VERSION,
                "max_file_bytes": MAX_EVENT_STATE_FILE_BYTES,
                "retained_events": metrics.retained_events,
                "next_event_id": metrics.next_event_id,
                "dropped_events": metrics.dropped_events,
                "subscriptions_durable": true,
                "webhook_deliveries_durable": true,
                "secrets_persisted": false,
                "recovery_policy": "events, subscription metadata, and signed outbox rows restore; subscriptions pause until explicit in-memory secret rebind",
                "flush": "/v1/events/persistence/flush"
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
                    "tools": "/v1/tools",
                    "missions": "/v1/missions",
                    "mission_persistence": "/v1/missions/persistence",
                    "mission_preflight": "/v1/missions/preflight",
                    "events": "/v1/events",
                    "delivery_receipt_events": "/v1/delivery-receipts/{receipt_id}/events",
                    "route_review_evidence": "/v1/route-reviews/{review_id}/evidence",
                    "event_persistence": "/v1/events/persistence",
                    "webhooks": "/v1/webhooks/subscriptions"
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
                    "mission_trace": true,
                    "delivery_receipt_events": true,
                    "route_review_evidence": true,
                    "max_mission_trace_events": MAX_MISSION_TRACE_EVENTS,
                    "cooperative_mission_cancellation": true,
                    "durable_mission_snapshots": self.config.mission_state_path.is_some(),
                    "durable_event_snapshots": self.config.event_state_path.is_some(),
                    "signed_webhook_outbox": true,
                    "delivery_failure_inspection": true,
                    "bounded_delivery_replay": true,
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
                    "event_state_file_bytes": MAX_EVENT_STATE_FILE_BYTES,
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

    fn preflight_mission(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mut report = match self
            .mission_executor
            .preflight_agent_mission(&Value::Object(arguments))
        {
            Ok(report) => report,
            Err(error) => return self.error(422, "invalid_mission", &error, request_id),
        };
        report["request_id"] = json!(request_id);
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
        if let Err(error) = self.mission_executor.validate_agent_mission(&arguments) {
            return self.error(422, "invalid_mission", &error, request_id);
        }
        let total_steps = arguments
            .get("steps")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

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
            error: None,
            recovered_after_restart: false,
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
                return self.error(
                    409,
                    "mission_exists",
                    "a mission with this mission_id already exists",
                    request_id,
                );
            }
            if jobs.len() >= MAX_MISSION_JOBS {
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
            return self.error(503, "mission_persistence_unavailable", &error, request_id);
        }

        let progress_state = Arc::clone(&state);
        let mission_events = Arc::clone(&self.events);
        let persistence = Arc::clone(&self.mission_persistence);
        let event_persistence = Arc::clone(&self.event_persistence);
        let mission_subject = mission_id.clone();
        let mission_request_id = request_id.to_string();
        let observer = Arc::new(move |event: Value| {
            if let Ok(mut current) = progress_state.lock() {
                current.record_trace(event.clone());
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
        let worker_id = mission_id.clone();
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
                if let Ok(mut current) = job.state.lock() {
                    match outcome {
                        Ok(result) => {
                            current.progress.reconcile(&result);
                            current.status = result
                                .get("mission_status")
                                .and_then(Value::as_str)
                                .unwrap_or("succeeded")
                                .into();
                            current.result = Some(result);
                            current.result_omitted = None;
                        }
                        Err(error) => {
                            current.status = "failed".into();
                            current.progress.phase = "failed".into();
                            current.progress.active_steps = 0;
                            current.error = Some(error);
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
                "cancel_requested": false,
                "progress": mission_progress_json(&MissionProgressState::new(total_steps)),
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
            entries.push(json!({
                "mission_id": mission_id,
                "status": state.status,
                "cancel_requested": state.cancel_requested,
                "cancel_reason": state.cancel_reason,
                "recovered_after_restart": state.recovered_after_restart,
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
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": current.status,
                "cancel_requested": current.cancel_requested,
                "cancel_reason": current.cancel_reason,
                "recovered_after_restart": current.recovered_after_restart,
                "progress": mission_progress_json(&current.progress),
                "result": current.result,
                "result_omitted": current.result_omitted,
                "error": current.error,
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
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
                    "/v1/tools": { "get": { "responses": { "200": { "description": "MCP tool catalog" } } } },
                    "/v1/tools/{name}": { "post": { "parameters": [{ "name": "name", "in": "path", "required": true }], "responses": { "200": { "description": "tool result" } } } },
                    "/v1/missions/preflight": { "post": { "responses": { "200": { "description": "authoritative no-dispatch mission plan" } } } },
                    "/v1/missions": { "get": { "responses": { "200": { "description": "bounded mission inventory" } } }, "post": { "responses": { "202": { "description": "accepted asynchronous mission" } } } },
                    "/v1/missions/persistence": { "get": { "responses": { "200": { "description": "restart-aware mission snapshot status" } } } },
                    "/v1/missions/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded mission snapshot checkpoint" } } } },
                    "/v1/missions/{mission_id}": { "get": { "responses": { "200": { "description": "mission status and result" } } }, "delete": { "responses": { "200": { "description": "terminal mission removed" } } } },
                    "/v1/missions/{mission_id}/trace": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "bounded clock-free mission trace page" } } } },
                    "/v1/missions/{mission_id}/cancel": { "post": { "responses": { "202": { "description": "cooperative cancellation requested" } } } },
                    "/v1/rpc": { "post": { "responses": { "200": { "description": "JSON-RPC response" } } } },
                    "/v1/events": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }, { "name": "review_id", "in": "query" }, { "name": "receipt_id", "in": "query" }], "responses": { "200": { "description": "cursor page; review_id and receipt_id are mutually exclusive" } } } },
                    "/v1/events/stream": { "get": { "parameters": [{ "name": "review_id", "in": "query" }, { "name": "receipt_id", "in": "query" }], "responses": { "200": { "description": "bounded Server-Sent Events snapshot" } } } },
                    "/v1/delivery-receipts/{receipt_id}/events": { "get": { "parameters": [{ "name": "receipt_id", "in": "path", "required": true }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "retained delivery-receipt event page" } } } },
                    "/v1/route-reviews/{review_id}/evidence": { "get": { "parameters": [{ "name": "review_id", "in": "path", "required": true }, { "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "retained route-review evidence page" } } } },
                    "/v1/events/persistence": { "get": { "responses": { "200": { "description": "event cursor checkpoint status" } } } },
                    "/v1/events/persistence/flush": { "post": { "responses": { "200": { "description": "force a bounded event cursor checkpoint" } } } },
                    "/v1/webhooks/subscriptions": { "get": { "responses": { "200": { "description": "subscriptions" } } }, "post": { "responses": { "201": { "description": "subscription" } } } },
                    "/v1/webhooks/subscriptions/{id}/rebind": { "post": { "parameters": [{ "name": "id", "in": "path", "required": true }], "responses": { "200": { "description": "in-memory secret rebind and pending-envelope re-sign" } } } },
                    "/v1/webhooks/subscriptions/{id}/deliveries": { "get": { "responses": { "200": { "description": "cursor page of inspectable pending deliveries and failure metadata" } } } },
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
    if schema_version != MISSION_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported mission state schema version {schema_version}; expected {MISSION_STATE_SCHEMA_VERSION}"
        ));
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
            error: object
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
            recovered_after_restart: object
                .get("recovered_after_restart")
                .and_then(Value::as_bool)
                .unwrap_or(false),
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

fn value_omission(bytes: &[u8]) -> Value {
    let mut digest = Sha256::new();
    digest.update(bytes);
    json!({ "bytes": bytes.len(), "sha256": hex_digest(&digest.finalize()) })
}

fn trim_mission_snapshot_to_bound(missions: &mut [Value]) -> Result<(), String> {
    loop {
        let size = serde_json::to_vec(&json!({
            "schema_version": MISSION_STATE_SCHEMA_VERSION,
            "missions": missions,
        }))
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
                "schema_version": MISSION_STATE_SCHEMA_VERSION,
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
        let persisted_active = persisted["missions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mission| mission["mission_id"] == "active-before-restart")
            .unwrap();
        assert_eq!(persisted_active["status"], "failed");
        assert_eq!(persisted_active["recovered_after_restart"], true);
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
            error: None,
            recovered_after_restart: false,
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
        let persistence = router.handle(request("GET", "/v1/events/persistence", json!({})));
        let persistence: Value = serde_json::from_slice(&persistence.body).unwrap();
        assert_eq!(persistence["enabled"], true);
        assert_eq!(persistence["schema_version"], 2);
        assert_eq!(persistence["subscriptions_durable"], true);
        assert_eq!(persistence["webhook_deliveries_durable"], true);
        assert_eq!(persistence["secrets_persisted"], false);
        assert_eq!(router.event_metrics().retained_events, 1);
        assert_eq!(router.event_metrics().pending_deliveries, 1);

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
    fn delivery_receipt_events_keep_a_bounded_join_projection_and_cursor_filter() {
        let router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
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
            "steps": [{"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}]
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
        let missing = router.handle(request("GET", "/v1/missions/api-preflight-1", json!({})));
        assert_eq!(missing.status, 404);

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
