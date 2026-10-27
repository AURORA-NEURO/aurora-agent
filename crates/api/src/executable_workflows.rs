//! Repository-defined local workflow application boundary.
//!
//! This is an application extension, not an additional blueprint coverage claim. The Python SDK
//! owns executable plan validation, retained contracts, source dispatch, and durable checkpoints.
//! This module owns fixed process invocation and the same-origin browser transport. It does not
//! translate these plans into the separate Rust mission grammar, accept arbitrary commands, or
//! enable private-account, MCP-session, clinical, or effectful integrations.

use crate::http::{HttpRequest, HttpResponse};
use serde::de::{
    Deserialize, Deserializer, Error as DeserializeError, MapAccess, SeqAccess, Visitor,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PREFIX: &str = "/v1/executable-workflows";
const RESPONSE_SCHEMA: &str = "aurora-workflow-api-response/0.1";
const MAX_WORKER_OUTPUT: usize = 16_000_000;
const MAX_WORKER_ERROR: usize = 64 * 1024;
const MAX_CONTROL_WORKERS: usize = 8;
const MAX_EXECUTION_WORKERS: usize = 4;

/// Operator-owned settings. None of these values is accepted from a browser request.
#[derive(Debug, Clone)]
pub struct WorkflowApplicationConfig {
    pub python_executable: PathBuf,
    pub python_path: PathBuf,
    pub state_root: PathBuf,
    /// Exact numeric loopback `host:port` to which this listener is bound.
    pub authority: String,
    pub model: Option<String>,
    pub base_url: String,
    pub research_sources: bool,
    pub molecular_glioma: bool,
    pub glioma_mutations: bool,
    pub gene_references: bool,
    pub protein_pathways: bool,
    pub denominator_coverage: bool,
    /// Fixed reviewed Python capability IDs, selected only by the operator at startup.
    pub capabilities: Vec<String>,
}

impl WorkflowApplicationConfig {
    pub fn validate(&self) -> Result<(), String> {
        let address: SocketAddr = self.authority.parse().map_err(|_| {
            "workflow application authority must be an exact numeric loopback host:port".to_string()
        })?;
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err("workflow application requires a fixed nonzero loopback port".into());
        }
        if address.to_string() != self.authority {
            return Err(
                "workflow application authority must use canonical host:port spelling".into(),
            );
        }
        for (label, path) in [
            ("python_executable", &self.python_executable),
            ("python_path", &self.python_path),
            ("state_root", &self.state_root),
        ] {
            if !path.is_absolute() {
                return Err(format!(
                    "workflow {label} must be an absolute operator-owned path"
                ));
            }
        }
        if !self.python_executable.is_file()
            || !self.python_path.is_dir()
            || !self.state_root.is_dir()
        {
            return Err(
                "workflow executable, SDK directory, and state directory must exist".into(),
            );
        }
        if !self.base_url.starts_with("http://127.0.0.1:")
            && !self.base_url.starts_with("http://[::1]:")
        {
            return Err(
                "workflow model base URL must be an operator-configured loopback HTTP URL".into(),
            );
        }
        if self.base_url.len() > 2048
            || self
                .base_url
                .bytes()
                .any(|byte| byte <= 0x20 || byte == 0x7f)
            || self.base_url.contains('@')
            || self.base_url.contains('?')
            || self.base_url.contains('#')
        {
            return Err(
                "workflow model base URL must not contain credentials, query, or controls".into(),
            );
        }
        let endpoint = self
            .base_url
            .strip_prefix("http://")
            .ok_or("workflow model endpoint must use loopback HTTP")?;
        let (authority, path) = endpoint
            .split_once('/')
            .ok_or("workflow model endpoint requires its /v1 base path")?;
        let model_address: SocketAddr = authority
            .parse()
            .map_err(|_| "workflow model endpoint requires an exact numeric host:port")?;
        if !model_address.ip().is_loopback()
            || model_address.port() == 0
            || !matches!(path, "v1" | "v1/")
        {
            return Err(
                "workflow model endpoint requires a nonzero loopback port and /v1 base path".into(),
            );
        }
        if self.model.as_ref().is_some_and(|model| {
            model.trim().is_empty()
                || model.chars().count() > 200
                || model.bytes().any(|byte| byte < 0x20)
        }) {
            return Err("workflow model must contain 1..=200 non-control characters".into());
        }
        if self.capabilities.len() > 32
            || self.capabilities.iter().enumerate().any(|(index, name)| {
                name.is_empty()
                    || name.len() > 64
                    || !name.as_bytes()[0].is_ascii_lowercase()
                    || name.bytes().any(|byte| {
                        !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && byte != b'-'
                    })
                    || self.capabilities[..index].contains(name)
            })
        {
            return Err("workflow capabilities require unique bounded reviewed profile IDs".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct WorkflowApplication {
    config: WorkflowApplicationConfig,
    control_workers: Arc<AtomicUsize>,
    planning_workers: Arc<AtomicUsize>,
    execution_workers: Arc<AtomicUsize>,
    runs: Arc<Mutex<BTreeMap<String, Value>>>,
}

struct WorkerPermit(Arc<AtomicUsize>);
impl Drop for WorkerPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn admit(counter: &Arc<AtomicUsize>, maximum: usize) -> Result<WorkerPermit, &'static str> {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        if current >= maximum {
            return Err("workflow worker capacity is currently occupied");
        }
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(WorkerPermit(Arc::clone(counter))),
            Err(observed) => current = observed,
        }
    }
}

impl WorkflowApplication {
    pub(crate) fn new(config: WorkflowApplicationConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            config,
            control_workers: Arc::new(AtomicUsize::new(0)),
            planning_workers: Arc::new(AtomicUsize::new(0)),
            execution_workers: Arc::new(AtomicUsize::new(0)),
            runs: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    /// Applied to every request in application mode, before legacy OPTIONS or any worker starts.
    pub(crate) fn guard(&self, request: &HttpRequest) -> Result<(), &'static str> {
        // Browsers omit the default HTTP port in both Host and serialized Origin.
        let authority = self
            .config
            .authority
            .strip_suffix(":80")
            .unwrap_or(&self.config.authority);
        if request.header("host") != Some(authority) {
            return Err("Host must match the configured workflow listener exactly");
        }
        if request
            .header("origin")
            .is_some_and(|origin| origin != format!("http://{authority}"))
        {
            return Err("cross-origin workflow requests are refused");
        }
        if request
            .header("sec-fetch-site")
            .is_some_and(|site| !matches!(site, "same-origin" | "none"))
        {
            return Err("cross-site workflow requests are refused");
        }
        Ok(())
    }

    pub(crate) fn owns_route(path: &str) -> bool {
        path == "/workflows" || path.starts_with("/workflows/") || path.starts_with(PREFIX)
    }

    pub(crate) fn public_asset(request: &HttpRequest) -> bool {
        request.method == "GET"
            && matches!(
                request.path(),
                "/workflows"
                    | "/workflows/"
                    | "/workflows/app.js"
                    | "/workflows/json.js"
                    | "/workflows/style.css"
            )
    }

    pub(crate) fn secure(response: HttpResponse) -> HttpResponse {
        response
            .with_header("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")
            .with_header("x-content-type-options", "nosniff")
            .with_header("x-frame-options", "DENY")
            .with_header("referrer-policy", "no-referrer")
            .with_header("cross-origin-resource-policy", "same-origin")
    }

    pub(crate) fn handle(&self, request: &HttpRequest) -> HttpResponse {
        if request.target.contains('?') {
            return failure(
                400,
                "invalid_request",
                "workflow routes do not accept query parameters",
            );
        }
        if request.method == "OPTIONS" {
            return HttpResponse::empty(204).with_header("allow", "GET, POST, OPTIONS");
        }
        match (request.method.as_str(), request.path()) {
            ("GET", "/workflows") => {
                return HttpResponse::empty(302).with_header("location", "/workflows/")
            }
            ("GET", "/workflows/") => {
                return HttpResponse::text(
                    200,
                    "text/html; charset=utf-8",
                    include_bytes!("../assets/workflows/index.html").to_vec(),
                )
            }
            ("GET", "/workflows/app.js") => {
                return HttpResponse::text(
                    200,
                    "text/javascript; charset=utf-8",
                    include_bytes!("../assets/workflows/app.js").to_vec(),
                )
            }
            ("GET", "/workflows/style.css") => {
                return HttpResponse::text(
                    200,
                    "text/css; charset=utf-8",
                    include_bytes!("../assets/workflows/style.css").to_vec(),
                )
            }
            ("GET", "/workflows/json.js") => {
                return HttpResponse::text(
                    200,
                    "text/javascript; charset=utf-8",
                    include_bytes!("../assets/workflows/json.js").to_vec(),
                )
            }
            _ => {}
        }
        match self.route(request) {
            Ok(response) => response,
            Err((status, message)) => failure(status, "invalid_request", &message),
        }
    }

    fn route(&self, request: &HttpRequest) -> Result<HttpResponse, (u16, String)> {
        let suffix = request
            .path()
            .strip_prefix(PREFIX)
            .ok_or((404, "route does not exist".into()))?;
        let segments: Vec<&str> = suffix.split('/').filter(|part| !part.is_empty()).collect();
        if request.method == "GET" {
            let command = match segments.as_slice() {
                ["catalogue"] => json!({"action":"catalogue"}),
                ["providers"] => json!({"action":"providers"}),
                ["intent-reviews", id] => {
                    valid_id(id)?;
                    json!({"action":"get_review","review_id":id})
                }
                ["plans", id] => {
                    valid_id(id)?;
                    json!({"action":"get_plan","plan_id":id})
                }
                ["plans", id, "intent-evidence"] => {
                    valid_id(id)?;
                    json!({"action":"export_evidence","plan_id":id,"format":"json_text"})
                }
                ["runs", id, "intent-evidence"] => {
                    valid_id(id)?;
                    json!({"action":"export_evidence","run_id":id,"format":"json_text"})
                }
                ["runs", id] => {
                    valid_id(id)?;
                    json!({"action":"inspect","run_id":id})
                }
                ["runs", id, kind @ ("outputs" | "checkpoint")] => {
                    valid_id(id)?;
                    json!({"action":"export","run_id":id,"kind":kind,"format":"json_text"})
                }
                _ => return Err((404, "route does not exist".into())),
            };
            let mut response = self.call(command, Duration::from_secs(30));
            if let [owner @ ("plans" | "runs"), id, "intent-evidence"] = segments.as_slice() {
                if response.status == 200 {
                    let exported: Value = serde_json::from_slice(&response.body)
                        .map_err(|_| (503, "worker evidence export was invalid".into()))?;
                    let data = exported
                        .get("data")
                        .ok_or((503, "worker evidence export omitted data".into()))?;
                    let matches_owner = if *owner == "plans" {
                        data.get("plan_id").and_then(Value::as_str) == Some(*id)
                            && data.get("run_id") == Some(&Value::Null)
                    } else {
                        data.get("run_id").and_then(Value::as_str) == Some(*id)
                    };
                    if data.get("kind").and_then(Value::as_str) != Some("intent_evidence")
                        || !matches_owner
                    {
                        return Err((
                            503,
                            "worker evidence export identity was inconsistent".into(),
                        ));
                    }
                    let artifact = data.get("json_text").and_then(Value::as_str).ok_or((
                        503,
                        "worker evidence export omitted authoritative JSON text".into(),
                    ))?;
                    response = HttpResponse::text(
                        200,
                        "application/json; charset=utf-8",
                        artifact.as_bytes().to_vec(),
                    )
                    .with_header(
                        "content-disposition",
                        format!("attachment; filename=aurora-{id}-intent-evidence.json"),
                    );
                }
            }
            if let ["runs", id] = segments.as_slice() {
                if let Ok(runs) = self.runs.lock() {
                    if let Some(observed) = runs.get(*id) {
                        if let Ok(mut envelope) = serde_json::from_slice::<Value>(&response.body) {
                            if let Some(data) =
                                envelope.get_mut("data").and_then(Value::as_object_mut)
                            {
                                data.insert("worker_process".into(), observed.clone());
                                response = HttpResponse::json(response.status, &envelope);
                            }
                        }
                    }
                }
            }
            if let ["runs", id, kind @ ("outputs" | "checkpoint")] = segments.as_slice() {
                if response.status == 200 {
                    let exported: Value = serde_json::from_slice(&response.body)
                        .map_err(|_| (503, "worker export was invalid".into()))?;
                    let artifact = exported
                        .pointer("/data/json_text")
                        .and_then(Value::as_str)
                        .ok_or((
                            503,
                            "worker export omitted its authoritative JSON text".into(),
                        ))?;
                    if exported.pointer("/data/kind").and_then(Value::as_str) != Some(*kind)
                        || exported.pointer("/data/run_id").and_then(Value::as_str) != Some(*id)
                    {
                        return Err((503, "worker export identity was inconsistent".into()));
                    }
                    response = HttpResponse::text(
                        200,
                        "application/json; charset=utf-8",
                        artifact.as_bytes().to_vec(),
                    )
                    .with_header(
                        "content-disposition",
                        format!("attachment; filename=aurora-{id}-{kind}.json"),
                    );
                }
            }
            return Ok(response);
        }
        if request.method != "POST" {
            return Err((405, "workflow routes accept GET or POST".into()));
        }
        if !request.header("content-type").is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        }) {
            return Err((415, "Content-Type: application/json is required".into()));
        }
        let mut body: Map<String, Value> = serde_json::from_slice::<StrictJson>(&request.body)
            .map_err(|_| (400, "request body is not valid JSON".into()))?
            .0
            .as_object()
            .cloned()
            .ok_or((400, "request body must be a JSON object".into()))?;
        match segments.as_slice() {
            ["intent-reviews", "prepare"] => {
                only_keys(
                    &body,
                    &[
                        "task",
                        "planner",
                        "obligations",
                        "requirements",
                        "max_steps",
                        "planning_attempts",
                        "max_model_calls",
                        "decision_attempts",
                        "operation_ids",
                    ],
                )?;
                Ok(self.call_object("prepare_review", &request.body, Duration::from_secs(30)))
            }
            ["intent-reviews", "suggest"] => {
                only_keys(
                    &body,
                    &[
                        "task",
                        "planner",
                        "requirements",
                        "max_steps",
                        "planning_attempts",
                        "max_model_calls",
                        "decision_attempts",
                        "operation_ids",
                        "approve_provider_call",
                    ],
                )?;
                if body.get("approve_provider_call") != Some(&Value::Bool(true)) {
                    return Err((
                        422,
                        "suggestions require an explicit literal true provider-call grant".into(),
                    ));
                }
                let _planning_permit =
                    admit(&self.planning_workers, 2).map_err(|error| (429, error.into()))?;
                Ok(self.call_object("suggest_review", &request.body, Duration::from_secs(300)))
            }
            ["intent-reviews", "accept"] => {
                only_keys(
                    &body,
                    &[
                        "review_id",
                        "expected_review_digest",
                        "accept_caller_declarations",
                    ],
                )?;
                if body.get("accept_caller_declarations") != Some(&Value::Bool(true)) {
                    return Err((422, "review acceptance requires an explicit literal true caller-declaration acceptance".into()));
                }
                Ok(self.call_object("accept_review", &request.body, Duration::from_secs(30)))
            }
            ["interpret"] => {
                only_keys(&body, &["task"])?;
                Ok(self.call_object("interpret", &request.body, Duration::from_secs(30)))
            }
            ["plans"] => {
                only_keys(
                    &body,
                    &[
                        "task",
                        "planner",
                        "requirements",
                        "approve_provider_call",
                        "max_steps",
                        "planning_attempts",
                        "max_model_calls",
                        "decision_attempts",
                        "operation_ids",
                        "intent_review_id",
                        "expected_acceptance_digest",
                    ],
                )?;
                // Long model requests cannot consume every short control slot needed to cancel
                // or inspect an already admitted source run.
                let _planning_permit =
                    admit(&self.planning_workers, 2).map_err(|error| (429, error.into()))?;
                Ok(self.call_object("plan", &request.body, Duration::from_secs(300)))
            }
            ["plans", "validate"] => {
                only_keys(&body, &["plan_id", "expected_plan_digest", "proposal"])?;
                Ok(self.call_object("validate", &request.body, Duration::from_secs(30)))
            }
            ["runs"] => {
                only_keys(
                    &body,
                    &[
                        "plan_id",
                        "expected_plan_digest",
                        "approve_source_dispatch",
                        "max_step_calls",
                    ],
                )?;
                require_source_grant(&body)?;
                let execution_permit = admit(&self.execution_workers, MAX_EXECUTION_WORKERS)
                    .map_err(|error| (429, error.into()))?;
                if self
                    .runs
                    .lock()
                    .map_err(|_| (503, "worker registry is unavailable".into()))?
                    .len()
                    > 4096 - MAX_EXECUTION_WORKERS
                {
                    return Err((429, "process run observation capacity is full".into()));
                }
                let created = self.call(json!({"action":"create_run","plan_id":body.get("plan_id"),"expected_plan_digest":body.get("expected_plan_digest")}), Duration::from_secs(30));
                if created.status != 200 {
                    return Ok(created);
                }
                let envelope: Value = serde_json::from_slice(&created.body)
                    .map_err(|_| (503, "worker response was invalid".into()))?;
                let run_id = envelope
                    .pointer("/data/run_id")
                    .and_then(Value::as_str)
                    .ok_or((503, "worker did not create a run".into()))?;
                valid_id(run_id)?;
                body.remove("plan_id");
                body.insert("action".into(), json!("execute"));
                body.insert("run_id".into(), json!(run_id));
                self.start_execution(run_id, Value::Object(body), execution_permit)?;
                Ok(HttpResponse::json(202, &envelope))
            }
            ["runs", id, "cancel"] => {
                valid_id(id)?;
                only_keys(&body, &[])?;
                Ok(self.call(
                    json!({"action":"cancel","run_id":id}),
                    Duration::from_secs(30),
                ))
            }
            ["runs", id, "resume"] => {
                valid_id(id)?;
                only_keys(
                    &body,
                    &[
                        "expected_plan_digest",
                        "approve_source_dispatch",
                        "retry_failed",
                        "resume_cancelled",
                        "max_step_calls",
                    ],
                )?;
                require_source_grant(&body)?;
                for key in ["retry_failed", "resume_cancelled"] {
                    if body.get(key).is_some_and(|value| !value.is_boolean()) {
                        return Err((422, format!("{key} must be a literal boolean")));
                    }
                }
                let inspected = self.call(
                    json!({"action":"inspect","run_id":id}),
                    Duration::from_secs(30),
                );
                if inspected.status != 200 {
                    return Ok(inspected);
                }
                let inspected: Value = serde_json::from_slice(&inspected.body)
                    .map_err(|_| (503, "worker response was invalid".into()))?;
                if inspected.pointer("/data/active").and_then(Value::as_bool) == Some(true) {
                    return Err((409, "this run already has an active worker".into()));
                }
                if inspected.pointer("/data/plan_digest") != body.get("expected_plan_digest") {
                    return Err((
                        409,
                        "execution approval does not match the retained run".into(),
                    ));
                }
                let resume =
                    inspected.pointer("/data/status").and_then(Value::as_str) != Some("queued");
                if !resume
                    && ["retry_failed", "resume_cancelled"]
                        .into_iter()
                        .any(|key| body.get(key) == Some(&Value::Bool(true)))
                {
                    return Err((422, "a queued run has no failed attempt or durable checkpoint cancellation to release".into()));
                }
                let permit = admit(&self.execution_workers, MAX_EXECUTION_WORKERS)
                    .map_err(|error| (429, error.into()))?;
                body.insert("action".into(), json!("execute"));
                body.insert("run_id".into(), json!(id));
                body.insert("resume".into(), json!(resume));
                self.start_execution(id, Value::Object(body), permit)?;
                Ok(HttpResponse::json(
                    202,
                    &json!({"schema":RESPONSE_SCHEMA,"ok":true,"data":{"run_id":id,"status":"resume_requested","metadata_only":true}}),
                ))
            }
            _ => Err((404, "route does not exist".into())),
        }
    }

    fn start_execution(
        &self,
        id: &str,
        command: Value,
        permit: WorkerPermit,
    ) -> Result<(), (u16, String)> {
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| (503, "worker registry is unavailable".into()))?;
        if runs
            .get(id)
            .is_some_and(|state| state.get("state").and_then(Value::as_str) == Some("running"))
        {
            return Err((409, "this run already has an active worker".into()));
        }
        if runs.len() >= 4096 && !runs.contains_key(id) {
            return Err((
                429,
                "process run observation capacity is full; restart preserves durable runs".into(),
            ));
        }
        runs.insert(id.to_string(), json!({"state":"running"}));
        let application = self.clone();
        let run_id = id.to_string();
        std::thread::Builder::new().name("aurora-executable-workflow".into()).spawn(move || {
            let _permit = permit;
            let observed = match application.invoke(&command, Duration::from_secs(900)) {
                Ok(envelope) => {
                    let mut observed = json!({"state":"finished","response_ok":envelope.get("ok").and_then(Value::as_bool)==Some(true)});
                    if envelope.get("ok").and_then(Value::as_bool)==Some(false) {
                        observed["error"] = envelope["error"].clone();
                    }
                    observed
                },
                Err(error) => json!({"state":"interrupted","error":error,"automatic_retry":false}),
            };
            if let Ok(mut runs) = application.runs.lock() { runs.insert(run_id, observed); }
        }).map_err(|_| {
            runs.insert(id.to_string(), json!({"state":"not_started","automatic_retry":false}));
            (503, "cannot start workflow worker thread".into())
        })?;
        Ok(())
    }

    fn call(&self, command: Value, timeout: Duration) -> HttpResponse {
        match serde_json::to_vec(&command) {
            Ok(bytes) => self.call_encoded(bytes, timeout),
            Err(_) => failure(400, "invalid_request", "cannot encode worker request"),
        }
    }

    fn call_object(&self, action: &str, original: &[u8], timeout: Duration) -> HttpResponse {
        // `route` has already checked the complete object, recursive uniqueness, and allowed keys.
        // Retain its original JSON number bytes rather than narrowing Python integers via Value.
        let object = original.trim_ascii();
        if object.first() != Some(&b'{') || object.last() != Some(&b'}') {
            return failure(400, "invalid_request", "request body must be a JSON object");
        }
        let mut bytes = match serde_json::to_vec(&json!({"action":action})) {
            Ok(bytes) => bytes,
            Err(_) => return failure(400, "invalid_request", "cannot encode worker action"),
        };
        bytes.pop();
        let fields = &object[1..object.len() - 1];
        if !fields.trim_ascii().is_empty() {
            bytes.push(b',');
            bytes.extend_from_slice(fields);
        }
        bytes.push(b'}');
        self.call_encoded(bytes, timeout)
    }

    fn call_encoded(&self, bytes: Vec<u8>, timeout: Duration) -> HttpResponse {
        let _permit = match admit(&self.control_workers, MAX_CONTROL_WORKERS) {
            Ok(permit) => permit,
            Err(error) => return failure(429, "worker_capacity", error),
        };
        match self.invoke_encoded(bytes, timeout) {
            Ok((envelope, bytes)) => {
                let status = if envelope.get("ok").and_then(Value::as_bool) == Some(true) {
                    200
                } else {
                    422
                };
                // Python owns canonical plan/checkpoint values, including large JSON
                // integers. Parsing for protocol checks must not rewrite the exported artifact.
                HttpResponse::text(status, "application/json; charset=utf-8", bytes)
            }
            Err(error) => failure(503, "worker_unavailable", &error),
        }
    }

    fn invoke(&self, request: &Value, timeout: Duration) -> Result<Value, String> {
        self.invoke_bytes(request, timeout).map(|(value, _)| value)
    }

    fn invoke_bytes(&self, request: &Value, timeout: Duration) -> Result<(Value, Vec<u8>), String> {
        let bytes = serde_json::to_vec(request).map_err(|_| "cannot encode worker request")?;
        self.invoke_encoded(bytes, timeout)
    }

    fn invoke_encoded(
        &self,
        bytes: Vec<u8>,
        timeout: Duration,
    ) -> Result<(Value, Vec<u8>), String> {
        if bytes.len() > 1_000_000 {
            return Err("worker request exceeds the application bound".into());
        }
        let mut command = Command::new(&self.config.python_executable);
        command
            .args(["-B", "-P", "-m", "prism_sdk.workflow_api_worker", "--root"])
            .arg(&self.config.state_root)
            .arg("--base-url")
            .arg(&self.config.base_url)
            .current_dir(&self.config.state_root)
            .env_clear()
            .env("PYTHONPATH", &self.config.python_path)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("PYTHONNOUSERSITE", "1")
            .env_remove("PYTHONSTARTUP")
            .env_remove("PYTHONINSPECT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Preserve runtime and certificate locations, not API tokens or unrelated service keys.
        for key in [
            "SystemRoot",
            "WINDIR",
            "PATH",
            "TEMP",
            "TMP",
            "TMPDIR",
            "HOME",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "LANG",
            "LC_ALL",
            "SSL_CERT_FILE",
            "SSL_CERT_DIR",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        if let Some(model) = &self.config.model {
            command.arg("--model").arg(model);
        }
        if self.config.research_sources {
            command.arg("--research-sources");
        }
        if self.config.molecular_glioma {
            command.arg("--molecular-glioma");
        }
        if self.config.glioma_mutations {
            command.arg("--glioma-mutations");
        }
        if self.config.gene_references {
            command.arg("--gene-references");
        }
        if self.config.protein_pathways {
            command.arg("--protein-pathways");
        }
        if self.config.denominator_coverage {
            command.arg("--denominator-coverage");
        }
        for capability in &self.config.capabilities {
            command.arg("--capability").arg(capability);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|_| "cannot start the configured workflow worker".to_string())?;
        let stdout = child.stdout.take().ok_or("worker stdout is unavailable")?;
        let stderr = child.stderr.take().ok_or("worker stderr is unavailable")?;
        let overflow = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let out_overflow = Arc::clone(&overflow);
        let err_overflow = Arc::clone(&overflow);
        let output =
            std::thread::spawn(move || bounded_read(stdout, MAX_WORKER_OUTPUT, &out_overflow));
        let errors =
            std::thread::spawn(move || bounded_read(stderr, MAX_WORKER_ERROR, &err_overflow));
        let mut stdin = child.stdin.take().ok_or("worker stdin is unavailable")?;
        let input = std::thread::spawn(move || stdin.write_all(&bytes));
        let started = Instant::now();
        let process_status = loop {
            if overflow.load(Ordering::Acquire) || started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                let _ = input.join();
                let _ = output.join();
                let _ = errors.join();
                return Err("worker exceeded its transport or time bound; inspect before explicitly resuming".into());
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = input.join();
                    let _ = output.join();
                    let _ = errors.join();
                    return Err("cannot observe the workflow worker process".into());
                }
            }
        };
        input
            .join()
            .map_err(|_| "worker input thread failed")?
            .map_err(|_| "worker refused its request")?;
        let stdout = output
            .join()
            .map_err(|_| "worker output thread failed")?
            .map_err(|_| "cannot read worker output")?;
        let _ = errors
            .join()
            .map_err(|_| "worker diagnostics thread failed")?
            .map_err(|_| "cannot read worker diagnostics")?;
        if overflow.load(Ordering::Acquire) {
            return Err("worker response exceeds the application bound".into());
        }
        let envelope: Value = serde_json::from_slice(&stdout)
            .map_err(|_| "worker returned an invalid response".to_string())?;
        if envelope.get("schema").and_then(Value::as_str) != Some(RESPONSE_SCHEMA)
            || !envelope.get("ok").is_some_and(Value::is_boolean)
            || (envelope.get("ok").and_then(Value::as_bool) == Some(true)
                && (!process_status.success()
                    || !envelope.get("data").is_some_and(Value::is_object)))
            || (envelope.get("ok").and_then(Value::as_bool) == Some(false)
                && !envelope.get("error").is_some_and(Value::is_object))
        {
            return Err("worker response did not satisfy the fixed protocol".into());
        }
        Ok((envelope, stdout))
    }
}

fn bounded_read(
    mut reader: impl Read,
    maximum: usize,
    overflow: &std::sync::atomic::AtomicBool,
) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if result.len().saturating_add(count) > maximum {
            overflow.store(true, Ordering::Release);
            return Ok(result);
        }
        result.extend_from_slice(&buffer[..count]);
    }
    Ok(result)
}

fn valid_id(id: &str) -> Result<(), (u16, String)> {
    if id.len() != 32
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Err((
            422,
            "workflow IDs must be 32 lowercase hexadecimal characters".into(),
        ))
    } else {
        Ok(())
    }
}

fn only_keys(body: &Map<String, Value>, keys: &[&str]) -> Result<(), (u16, String)> {
    if body.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err((
            422,
            "request contains unsupported application fields".into(),
        ));
    }
    Ok(())
}

fn require_source_grant(body: &Map<String, Value>) -> Result<(), (u16, String)> {
    if body.get("approve_source_dispatch") != Some(&Value::Bool(true)) {
        return Err((
            422,
            "execution requires an explicit literal true source-dispatch grant".into(),
        ));
    }
    if !body
        .get("expected_plan_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        return Err((
            422,
            "execution requires the exact retained plan digest".into(),
        ));
    }
    if body.get("max_step_calls").is_some_and(|value| {
        !value
            .as_u64()
            .is_some_and(|value| (1..=32).contains(&value))
    }) {
        return Err((
            422,
            "max_step_calls must be an integer between 1 and 32".into(),
        ));
    }
    Ok(())
}

fn failure(status: u16, code: &str, message: &str) -> HttpResponse {
    HttpResponse::json(
        status,
        &json!({"schema":RESPONSE_SCHEMA,"ok":false,"error":{"code":code,"message":message}}),
    )
}

/// Preserve unambiguous approval and contract fields at every JSON nesting level before the
/// request is transformed into the worker protocol. `Value` alone silently keeps the last key.
struct StrictJson(Value);
impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = StrictJson;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("JSON without duplicate object fields")
            }
            fn visit_bool<E: DeserializeError>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictJson(json!(value)))
            }
            fn visit_i64<E: DeserializeError>(self, value: i64) -> Result<Self::Value, E> {
                Ok(StrictJson(json!(value)))
            }
            fn visit_u64<E: DeserializeError>(self, value: u64) -> Result<Self::Value, E> {
                Ok(StrictJson(json!(value)))
            }
            fn visit_f64<E: DeserializeError>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| StrictJson(Value::Number(number)))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: DeserializeError>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictJson(json!(value)))
            }
            fn visit_string<E: DeserializeError>(self, value: String) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::String(value)))
            }
            fn visit_unit<E: DeserializeError>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<StrictJson>()? {
                    values.push(value.0);
                }
                Ok(StrictJson(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = mapping.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(A::Error::custom("duplicate JSON field"));
                    }
                    values.insert(key, mapping.next_value::<StrictJson>()?.0);
                }
                Ok(StrictJson(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(JsonVisitor)
    }
}

#[cfg(test)]
mod tests;
