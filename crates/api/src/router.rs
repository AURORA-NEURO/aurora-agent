//! HTTP routes over the existing MCP server.
//!
//! The router intentionally delegates domain semantics to `bioprism-mcp::Server`.  The HTTP
//! layer owns transport concerns only: authentication, request bounds, route shape, cursors, and
//! the event/webhook outbox.  A REST call and an MCP `tools/call` therefore reach exactly the same
//! Rust implementation and produce the same evidence-bearing result.

use crate::events::{EventLog, MAX_FILTERS};
use crate::http::{HttpRequest, HttpResponse};
use bioprism_mcp::{Request, Response, PROTOCOL_VERSION, SERVER_NAME};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub const API_VERSION: &str = "v1";
pub const DEFAULT_MAX_HEADER_BYTES: usize = 32 * 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_EVENT_CAPACITY: usize = 4096;
pub const MAX_MISSION_JOBS: usize = 4096;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub event_capacity: usize,
    pub bearer_token: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_header_bytes: DEFAULT_MAX_HEADER_BYTES,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            bearer_token: None,
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
        Ok(())
    }
}

pub struct ApiRouter {
    server: bioprism_mcp::Server,
    mission_executor: Arc<bioprism_mcp::Server>,
    config: ApiConfig,
    events: EventLog,
    next_request_id: u64,
    mission_jobs: Arc<Mutex<BTreeMap<String, Arc<MissionJob>>>>,
}

struct MissionJob {
    cancellation: Arc<AtomicBool>,
    state: Arc<Mutex<MissionJobState>>,
}

#[derive(Clone)]
struct MissionJobState {
    status: String,
    cancel_requested: bool,
    cancel_reason: Option<String>,
    result: Option<Value>,
    error: Option<String>,
}

impl ApiRouter {
    pub fn new(root: PathBuf, config: ApiConfig) -> Result<Self, String> {
        config.validate()?;
        let events = EventLog::new(config.event_capacity)?;
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
        Ok(Self {
            server,
            mission_executor,
            config,
            events,
            next_request_id: 1,
            mission_jobs: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn handle(&mut self, request: HttpRequest) -> HttpResponse {
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
            ("POST", "/v1/missions/preflight") => self.preflight_mission(&request, &request_id),
            ("POST", "/v1/missions") => self.submit_mission(&request, &request_id),
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
            ("GET", path) if path.ends_with("/deliveries") => {
                self.list_deliveries(&request, &request_id)
            }
            ("POST", path) if path.ends_with("/ack") => self.ack_deliveries(&request, &request_id),
            ("POST", path) if path.ends_with("/retry") => {
                self.retry_deliveries(&request, &request_id)
            }
            _ => self.error(404, "not_found", "route does not exist", &request_id),
        };
        self.finish(response, &request_id)
    }

    pub fn event_metrics(&self) -> crate::events::EventMetrics {
        self.events.metrics()
    }

    pub fn limits(&self) -> (usize, usize) {
        (self.config.max_header_bytes, self.config.max_body_bytes)
    }

    fn request_id(&mut self, request: &HttpRequest) -> String {
        if let Some(value) = request.header("x-request-id") {
            if !value.is_empty() && value.len() <= 256 && value.bytes().all(|byte| byte >= 0x20) {
                return value.to_string();
            }
        }
        let id = format!("http-{}", self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);
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
        let metrics = self.events.metrics();
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
                    "mission_preflight": "/v1/missions/preflight",
                    "events": "/v1/events",
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
                    "cooperative_mission_cancellation": true,
                    "signed_webhook_outbox": true,
                    "grpc": false,
                    "tls": false,
                    "external_delivery_worker": false
                },
                "limits": {
                    "max_header_bytes": self.config.max_header_bytes,
                    "max_body_bytes": self.config.max_body_bytes,
                    "event_capacity": self.config.event_capacity,
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
        HttpResponse::json(
            200,
            &json!({ "ok": true, "metrics": self.events.metrics() }),
        )
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
        match self.events.events(after, limit) {
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
        match self.events.events(after, limit) {
            Ok(page) => HttpResponse::text(
                200,
                "text/event-stream; charset=utf-8",
                self.events.sse(&page),
            )
            .with_header("x-next-after", page.next_after.to_string()),
            Err(error) => self.error(400, "invalid_query", &error, "query"),
        }
    }

    fn rpc(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
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
        let Some(response) = self.server.handle(&parsed) else {
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

    fn rest_tool(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
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
        let Some(response) = self.server.handle(&call) else {
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

    fn submit_mission(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
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

        let cancellation = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(MissionJobState {
            status: "queued".into(),
            cancel_requested: false,
            cancel_reason: None,
            result: None,
            error: None,
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

        let executor = Arc::clone(&self.mission_executor);
        let worker_id = mission_id.clone();
        let worker_arguments = arguments;
        let spawn = thread::Builder::new()
            .name(format!("mission-{worker_id}"))
            .spawn(move || {
                if let Ok(mut current) = state.lock() {
                    current.status = "running".into();
                }
                let outcome = executor
                    .execute_agent_mission_with_cancellation(&worker_arguments, &cancellation);
                if let Ok(mut current) = job.state.lock() {
                    match outcome {
                        Ok(result) => {
                            current.status = result
                                .get("mission_status")
                                .and_then(Value::as_str)
                                .unwrap_or("succeeded")
                                .into();
                            current.result = Some(result);
                        }
                        Err(error) => {
                            current.status = "failed".into();
                            current.error = Some(error);
                        }
                    }
                }
            });
        if spawn.is_err() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.remove(&mission_id);
            }
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
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "guarantees": [
                    "mission validation completed before acceptance",
                    "execution is cooperative and preserves the authoritative mission report",
                    "in-flight nested tool calls are allowed to return before future dispatch stops",
                ],
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
                "result": current.result,
                "error": current.error,
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
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
        HttpResponse::json(
            202,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": current.status,
                "cancel_requested": true,
                "cancel_reason": current.cancel_reason,
                "reason": reason,
                "poll": format!("/v1/missions/{mission_id}"),
            }),
        )
    }

    fn delete_mission(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
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
                "subscriptions": self.events.subscriptions(),
                "secret_policy": "secrets are never returned; delivery signatures are computed over the unsigned envelope"
            }),
        )
    }

    fn create_subscription(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
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
        match self.events.register_subscription(
            body.get("id").and_then(Value::as_str),
            endpoint,
            filters.as_deref(),
            secret,
        ) {
            Ok(subscription) => HttpResponse::json(
                201,
                &json!({
                    "ok": true,
                    "subscription": subscription,
                    "delivery": {
                        "mode": "signed_outbox",
                        "poll": "/v1/webhooks/subscriptions/{id}/deliveries",
                        "ack": "/v1/webhooks/subscriptions/{id}/ack",
                        "retry": "/v1/webhooks/subscriptions/{id}/retry"
                    }
                }),
            ),
            Err(error) => self.error(422, "invalid_subscription", &error, request_id),
        }
    }

    fn delete_subscription(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), None) else {
            return self.error(
                404,
                "not_found",
                "subscription route does not exist",
                request_id,
            );
        };
        match self.events.remove_subscription(&id) {
            Ok(true) => HttpResponse::json(200, &json!({ "ok": true, "deleted": id })),
            Ok(false) => self.error(404, "not_found", "subscription does not exist", request_id),
            Err(error) => self.error(409, "subscription_error", &error, request_id),
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
        match self.events.deliveries(&id, after, limit) {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(404, "not_found", &error, request_id),
        }
    }

    fn ack_deliveries(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, false)
    }

    fn retry_deliveries(&mut self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, true)
    }

    fn delivery_mutation(
        &mut self,
        request: &HttpRequest,
        request_id: &str,
        retry: bool,
    ) -> HttpResponse {
        let Some(id) = subscription_id(
            &request.path_segments(),
            Some(if retry { "retry" } else { "ack" }),
        ) else {
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
        if retry {
            match self.events.retry(&id, &ids) {
                Ok(deliveries) => {
                    HttpResponse::json(200, &json!({ "ok": true, "retried": deliveries }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        } else {
            match self.events.acknowledge(&id, &ids) {
                Ok(acknowledged) => {
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

    fn record_tool_event(&mut self, request_id: &str, tool: &str, wire: &Value) {
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
        let payload = if encoded.len() <= 64 * 1024 {
            json!({ "tool": tool, "response": wire })
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
            projection
        };
        let _ = self.events.emit(outcome, tool, request_id, payload);
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
                    "/v1/missions": { "post": { "responses": { "202": { "description": "accepted asynchronous mission" } } } },
                    "/v1/missions/{mission_id}": { "get": { "responses": { "200": { "description": "mission status and result" } } }, "delete": { "responses": { "200": { "description": "terminal mission removed" } } } },
                    "/v1/missions/{mission_id}/cancel": { "post": { "responses": { "202": { "description": "cooperative cancellation requested" } } } },
                    "/v1/rpc": { "post": { "responses": { "200": { "description": "JSON-RPC response" } } } },
                    "/v1/events": { "get": { "parameters": [{ "name": "after", "in": "query" }, { "name": "limit", "in": "query" }], "responses": { "200": { "description": "cursor page" } } } },
                    "/v1/events/stream": { "get": { "responses": { "200": { "description": "bounded Server-Sent Events snapshot" } } } },
                    "/v1/webhooks/subscriptions": { "get": { "responses": { "200": { "description": "subscriptions" } } }, "post": { "responses": { "201": { "description": "subscription" } } } }
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
    use std::collections::BTreeMap;

    fn request(method: &str, target: &str, body: Value) -> HttpRequest {
        HttpRequest {
            method: method.into(),
            target: target.into(),
            version: "HTTP/1.1".into(),
            headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    #[test]
    fn rest_and_json_rpc_share_tool_dispatch_and_auth_is_fail_closed() {
        let mut router = ApiRouter::new(
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
    fn webhook_lifecycle_is_cursor_based_and_secrets_do_not_return() {
        let mut router =
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
    fn mission_execution_trace_survives_rest_and_event_projection() {
        let mut router =
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
    fn asynchronous_missions_validate_poll_and_reject_duplicate_ids() {
        let mut router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
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
        let mut router =
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
        let mut router =
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
        let mut router =
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
        let page = router.events.events(0, 10).unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].payload["response_omitted"], true);
        assert_eq!(
            page.events[0].payload["mission_trace"]["execution_trace"][0]["event"],
            "mission.completed"
        );
    }
}
