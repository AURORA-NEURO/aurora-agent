//! Explicit, bounded command execution through a local Docker Engine.
//!
//! This is an opt-in host API, separate from [`crate::ExecutorProvider`]: it does not make a
//! `WorldTape` provider available, and SDK isolation declarations still require the caller to
//! choose and invoke this runner. It invokes the Docker CLI directly with argument vectors (no
//! shell), requires a locally available digest-pinned Linux image, resolves the configured Docker
//! endpoint and refuses remote daemons, and fixes the isolation options here so callers cannot
//! accidentally omit the network, filesystem, privilege, or resource boundaries. The local Docker
//! daemon and host kernel remain trusted parts of the boundary.
//!
//! A command receives one read-only input directory, a private tmpfs artifact directory, and a
//! separate bounded stdout capture directory. Output has byte and entry limits; validated files are
//! atomically published to a new caller-selected host path only after container cleanup is verified.
//! The command receives no host environment variables or stdin. Its root filesystem is read-only,
//! networking is disabled, Linux capabilities are dropped, privilege escalation is disabled, and
//! CPU, memory, process count, runtime, and captured output are bounded. A timed-out or output-capped
//! run is forcibly removed and checked absent before the call returns; if that cannot be verified,
//! the runner returns an error.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const MIN_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MEMORY_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_CPU_MILLIS: u32 = 32_000;
const MAX_PIDS: u32 = 4_096;
const MAX_TIMEOUT_MS: u64 = 60 * 60 * 1000;
const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ARTIFACT_ENTRIES: u32 = 4_096;
const MAX_ARTIFACT_DEPTH: usize = 64;
const MAX_ARTIFACT_PATH_BYTES: usize = 256;
const MAX_SCRATCH_INODES: u32 = 4_096;
const ENGINE_OUTPUT_BYTES: usize = 8 * 1024;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);
const RUN_LABEL_KEY: &str = "io.aurora.sandbox.run";
const SANDBOX_ENTRYPOINT: &str = "/bin/sh";
const ARTIFACT_ARCHIVE_SUCCESS_MARKER: &[u8] = b"\nAURORA-OCI-TAR-OK\n";
const ARTIFACT_ARCHIVE_SCRIPT: &str = concat!(
    "set +e\n",
    "\"$@\" > /aurora/capture/stdout\n",
    "command_status=$?\n",
    "tar --format=ustar -C /aurora -cf - output capture\n",
    "archive_status=$?\n",
    "if [ \"$archive_status\" -ne 0 ]; then exit 125; fi\n",
    "printf '\\nAURORA-OCI-TAR-OK\\n'\n",
    "exit \"$command_status\"\n",
);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Linux image architecture selected explicitly by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPlatform {
    Amd64,
    Arm64,
}

impl LinuxPlatform {
    fn as_str(self) -> &'static str {
        match self {
            LinuxPlatform::Amd64 => "linux/amd64",
            LinuxPlatform::Arm64 => "linux/arm64",
        }
    }
}

/// Hard resource ceilings for one container command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxLimits {
    pub memory_bytes: u64,
    pub cpu_millis: u32,
    /// Includes the container's init process; use at least two for a command process.
    pub max_processes: u32,
    pub timeout_ms: u64,
    /// Per-stream ceiling for captured command stdout and stderr.
    pub max_output_bytes: usize,
    /// Aggregate regular-file bytes the command may return through `/aurora/output`.
    pub max_artifact_bytes: u64,
    /// Maximum number of files and directories returned through `/aurora/output`.
    pub max_artifact_entries: u32,
    /// Non-root numeric identity used inside the container and as owner of `/aurora/output`.
    pub container_uid: u32,
    pub container_gid: u32,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        SandboxLimits {
            memory_bytes: 512 * 1024 * 1024,
            cpu_millis: 1_000,
            max_processes: 64,
            timeout_ms: 5 * 60 * 1000,
            max_output_bytes: 1024 * 1024,
            max_artifact_bytes: 16 * 1024 * 1024,
            max_artifact_entries: 1_024,
            container_uid: 65_532,
            container_gid: 65_532,
        }
    }
}

/// A single command and the only host directories it may access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxRequest {
    /// Must end in `@sha256:` followed by exactly 64 lowercase hexadecimal digits.
    pub image: String,
    pub platform: LinuxPlatform,
    /// Absolute executable path inside the image. The fixed runner shell invokes it as data.
    pub command: String,
    pub arguments: Vec<String>,
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub limits: SandboxLimits,
}

impl SandboxRequest {
    /// Validate paths, image identity, arguments, and resource limits without starting Docker.
    pub fn validate(&self) -> Result<(), OciSandboxError> {
        validate_request(self).map(|_| ())
    }
}

/// A bounded result. Captured streams stay in memory; file contents are published only to the
/// caller's new output path, while this receipt retains their exact path, size, and digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxRunResult {
    pub container_id: Option<String>,
    /// The requested program's status after the archive wrapper completed successfully.
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_limit_exceeded: bool,
    pub cleanup_verified: bool,
    /// Captured stdout from the requested program, excluding the transport archive.
    pub stdout: Vec<u8>,
    /// Captured stderr from the requested program and Docker CLI diagnostics.
    pub stderr: Vec<u8>,
    /// Sorted metadata for files atomically published to the requested output directory.
    pub artifacts: Vec<DockerArtifact>,
}

/// One regular file accepted from the bounded container output filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerArtifact {
    /// Relative path using `/` separators, independent of the host platform.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Refusals and operational failures from the Docker-backed command boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OciSandboxError {
    #[error("invalid sandbox request field {field}: {reason}")]
    InvalidRequest { field: &'static str, reason: String },
    #[error("Docker Engine is unavailable: {reason}")]
    EngineUnavailable { reason: String },
    #[error("Docker Engine {operation} failed with exit code {exit_code:?}")]
    EngineFailure {
        operation: &'static str,
        exit_code: Option<i32>,
    },
    #[error("Docker image is not eligible for bounded execution: {reason}")]
    ImageRejected { reason: String },
    #[error("container cleanup could not be verified for {container_id:?}")]
    CleanupUnverified { container_id: Option<String> },
    #[error("sandbox output capture failed: {reason}")]
    CaptureFailure { reason: String },
    #[error("sandbox artifact collection failed: {reason}")]
    ArtifactCollectionFailed { reason: String },
}

/// A Docker CLI path. No command is routed through a shell.
#[derive(Debug, Clone)]
pub struct DockerSandbox {
    executable: PathBuf,
}

impl Default for DockerSandbox {
    fn default() -> Self {
        DockerSandbox::new("docker")
    }
}

impl DockerSandbox {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        DockerSandbox {
            executable: executable.into(),
        }
    }

    /// Verify that the selected CLI can reach a running Engine.
    pub fn probe(&self) -> Result<String, OciSandboxError> {
        let endpoint = local_docker_endpoint(&self.executable)
            .map_err(|reason| OciSandboxError::EngineUnavailable { reason })?;
        self.probe_endpoint(&endpoint)
    }

    fn probe_endpoint(&self, endpoint: &str) -> Result<String, OciSandboxError> {
        let output = run_bounded(
            &self.executable,
            &[
                OsString::from("info"),
                OsString::from("--format"),
                OsString::from("{{.ServerVersion}}"),
            ],
            Duration::from_secs(5),
            ENGINE_OUTPUT_BYTES,
            Some(endpoint),
        )
        .map_err(|reason| OciSandboxError::EngineUnavailable { reason })?;
        if output.timed_out || output.output_limit_exceeded || !successful(&output.status) {
            return Err(OciSandboxError::EngineUnavailable {
                reason: "docker info did not complete successfully".into(),
            });
        }
        if let Some(reason) = output.capture_error {
            return Err(OciSandboxError::EngineUnavailable { reason });
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if version.is_empty() || version.len() > 128 || version.contains('\0') {
            return Err(OciSandboxError::EngineUnavailable {
                reason: "docker info returned no bounded server version".into(),
            });
        }
        Ok(version)
    }

    fn ensure_image_has_no_volumes(
        &self,
        endpoint: &str,
        image: &str,
        platform: LinuxPlatform,
    ) -> Result<(), OciSandboxError> {
        let args = image_volume_inspection_arguments(image, platform);
        let output = run_bounded(
            &self.executable,
            &args,
            Duration::from_secs(5),
            ENGINE_OUTPUT_BYTES,
            Some(endpoint),
        )
        .map_err(|reason| OciSandboxError::EngineUnavailable { reason })?;
        if output.timed_out || output.output_limit_exceeded || !successful(&output.status) {
            return Err(OciSandboxError::EngineFailure {
                operation: "image inspection",
                exit_code: output.status.and_then(|status| status.code()),
            });
        }
        if let Some(reason) = output.capture_error {
            return Err(OciSandboxError::EngineUnavailable { reason });
        }
        ensure_no_image_volumes(&output.stdout)
    }

    /// Run one digest-pinned command with fixed isolation controls and bounded lifetime/output.
    pub fn run(&self, request: &SandboxRequest) -> Result<SandboxRunResult, OciSandboxError> {
        let normalized = validate_request(request)?;
        let endpoint = local_docker_endpoint(&self.executable)
            .map_err(|reason| OciSandboxError::EngineUnavailable { reason })?;
        self.probe_endpoint(&endpoint)?;
        self.ensure_image_has_no_volumes(&endpoint, &request.image, request.platform)?;
        let temp = TemporaryDirectory::create_in(
            normalized
                .output_dir
                .parent()
                .expect("validated output directory has a parent"),
        )?;
        let cid_file = temp.path.join("container-id");
        let staging_dir = temp.path.join("artifacts");
        let output_staging_dir = staging_dir.join("output");
        create_private_directory(&staging_dir).map_err(|error| {
            OciSandboxError::EngineUnavailable {
                reason: format!("cannot create private artifact staging directory: {error}"),
            }
        })?;
        create_private_directory(&output_staging_dir).map_err(|error| {
            OciSandboxError::EngineUnavailable {
                reason: format!("cannot create private output staging directory: {error}"),
            }
        })?;
        let run_label = unique_run_label();
        let args = build_run_arguments(request, &normalized, &cid_file, &run_label);
        let archive_limit = maximum_artifact_archive_bytes(&request.limits)?;
        let stderr_limit = request.limits.max_output_bytes;
        let total_stream_limit = archive_limit
            .checked_add(stderr_limit)
            .ok_or_else(|| artifact_archive_error("Docker stream byte ceiling overflowed"))?;
        let run_output = run_bounded_with_stream_limits(
            &self.executable,
            &args,
            Duration::from_millis(request.limits.timeout_ms),
            archive_limit,
            stderr_limit,
            total_stream_limit,
            Some(&endpoint),
        );

        let container_id = read_container_id(&cid_file).ok();
        let mut artifact_error = None;
        let mut parsed_archive = None;
        if let Ok(output) = &run_output {
            if !output.timed_out && !output.output_limit_exceeded && output.status.is_some() {
                let parsed = strip_archive_success_marker(&output.stdout).and_then(|archive| {
                    extract_artifact_archive(archive, &output_staging_dir, &request.limits)
                });
                match parsed {
                    Ok(parsed) => parsed_archive = Some(parsed),
                    Err(error) => artifact_error = Some(error),
                }
            }
        }
        let (output, cleanup_verified) =
            finish_run_output(run_output, container_id.clone(), |id| match id {
                Some(id) => self.remove_and_verify(&endpoint, id),
                None => self.remove_and_verify_label(&endpoint, &run_label),
            })?;
        let interrupted = output.timed_out || output.output_limit_exceeded;
        if interrupted && !cleanup_verified {
            return Err(OciSandboxError::CleanupUnverified { container_id });
        }
        if !cleanup_verified {
            return Err(OciSandboxError::CleanupUnverified { container_id });
        }
        if let Some(error) = artifact_error {
            return Err(error);
        }
        if let Some(reason) = output.capture_error {
            return Err(OciSandboxError::CaptureFailure { reason });
        }

        let artifacts = if interrupted || output.status.is_none() {
            Vec::new()
        } else {
            let parsed =
                parsed_archive.ok_or_else(|| OciSandboxError::ArtifactCollectionFailed {
                    reason: "sandbox output was not collected before cleanup".into(),
                })?;
            if parsed.stdout_limit_exceeded {
                return Ok(SandboxRunResult {
                    container_id,
                    exit_code: output.status.as_ref().and_then(ExitStatus::code),
                    timed_out: false,
                    output_limit_exceeded: true,
                    cleanup_verified,
                    stdout: parsed.stdout,
                    stderr: output.stderr,
                    artifacts: Vec::new(),
                });
            }
            let published =
                publish_artifacts(&output_staging_dir, &normalized.output_dir, &request.limits)?;
            if published != parsed.artifacts {
                return Err(artifact_archive_error(
                    "staged output changed after archive verification",
                ));
            }
            return Ok(SandboxRunResult {
                container_id,
                exit_code: output.status.as_ref().and_then(ExitStatus::code),
                timed_out: output.timed_out,
                output_limit_exceeded: output.output_limit_exceeded,
                cleanup_verified,
                stdout: parsed.stdout,
                stderr: output.stderr,
                artifacts: published,
            });
        };

        Ok(SandboxRunResult {
            container_id,
            exit_code: output.status.as_ref().and_then(ExitStatus::code),
            timed_out: output.timed_out,
            output_limit_exceeded: output.output_limit_exceeded,
            cleanup_verified,
            stdout: Vec::new(),
            stderr: output.stderr,
            artifacts,
        })
    }

    fn remove_and_verify(&self, endpoint: &str, container_id: &str) -> bool {
        let remove = container_remove_arguments(container_id);
        let _ = run_bounded(
            &self.executable,
            &remove,
            CLEANUP_TIMEOUT,
            ENGINE_OUTPUT_BYTES,
            Some(endpoint),
        );
        let inspect = [
            OsString::from("ps"),
            OsString::from("--all"),
            OsString::from("--quiet"),
            OsString::from("--no-trunc"),
            OsString::from("--filter"),
            OsString::from(format!("id={container_id}")),
        ];
        run_bounded(
            &self.executable,
            &inspect,
            CLEANUP_TIMEOUT,
            ENGINE_OUTPUT_BYTES,
            Some(endpoint),
        )
        .is_ok_and(|output| {
            !output.timed_out
                && !output.output_limit_exceeded
                && output.capture_error.is_none()
                && successful(&output.status)
                && output.stdout.iter().all(u8::is_ascii_whitespace)
        })
    }

    fn remove_and_verify_label(&self, endpoint: &str, label: &str) -> bool {
        let deadline = Instant::now() + CLEANUP_TIMEOUT;
        let mut observed_empty = false;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let ids = match self.container_ids_for_label(endpoint, label, remaining) {
                Some(ids) => ids,
                None => return false,
            };
            if ids.is_empty() {
                if observed_empty {
                    return true;
                }
                observed_empty = true;
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            observed_empty = false;
            for id in ids {
                if !self.remove_and_verify(endpoint, &id) {
                    return false;
                }
            }
        }
        false
    }

    fn container_ids_for_label(
        &self,
        endpoint: &str,
        label: &str,
        timeout: Duration,
    ) -> Option<Vec<String>> {
        let filter = format!("label={RUN_LABEL_KEY}={label}");
        let args = [
            OsString::from("ps"),
            OsString::from("--all"),
            OsString::from("--quiet"),
            OsString::from("--no-trunc"),
            OsString::from("--filter"),
            OsString::from(filter),
        ];
        let output = run_bounded(
            &self.executable,
            &args,
            timeout.min(CLEANUP_TIMEOUT),
            ENGINE_OUTPUT_BYTES,
            Some(endpoint),
        )
        .ok()?;
        if output.timed_out
            || output.output_limit_exceeded
            || output.capture_error.is_some()
            || !successful(&output.status)
        {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|id| {
                if valid_container_id(id) {
                    Some(id.to_owned())
                } else {
                    None
                }
            })
            .collect()
    }
}

fn local_docker_endpoint(executable: &Path) -> Result<String, String> {
    if std::env::var_os("DOCKER_CONTEXT").is_none() {
        if let Some(endpoint) = std::env::var_os("DOCKER_HOST") {
            let endpoint = endpoint
                .into_string()
                .map_err(|_| "DOCKER_HOST is not valid Unicode".to_owned())?;
            let endpoint = endpoint.trim();
            if is_local_docker_endpoint(endpoint) {
                return Ok(endpoint.to_owned());
            }
            return Err(
                "remote Docker endpoints are refused because bind paths are local to this host"
                    .into(),
            );
        }
    }

    let context = match std::env::var("DOCKER_CONTEXT") {
        Ok(context) => context,
        Err(std::env::VarError::NotPresent) => {
            let output = run_bounded(
                executable,
                &[OsString::from("context"), OsString::from("show")],
                Duration::from_secs(5),
                ENGINE_OUTPUT_BYTES,
                None,
            )
            .map_err(|reason| format!("cannot read the selected Docker context: {reason}"))?;
            if output.timed_out
                || output.output_limit_exceeded
                || output.capture_error.is_some()
                || !successful(&output.status)
            {
                return Err("Docker context show did not complete successfully".into());
            }
            String::from_utf8(output.stdout)
                .map_err(|_| "Docker context name is not valid UTF-8".to_owned())?
                .trim()
                .to_owned()
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err("DOCKER_CONTEXT is not valid Unicode".into());
        }
    };
    if context.is_empty()
        || context.len() > 128
        || context.starts_with('-')
        || context.chars().any(char::is_control)
    {
        return Err("selected Docker context name is malformed".into());
    }

    let output = run_bounded(
        executable,
        &[
            OsString::from("context"),
            OsString::from("inspect"),
            OsString::from("--format=json"),
            OsString::from(context),
        ],
        Duration::from_secs(5),
        ENGINE_OUTPUT_BYTES,
        None,
    )
    .map_err(|reason| format!("cannot inspect the selected Docker context: {reason}"))?;
    if output.timed_out
        || output.output_limit_exceeded
        || output.capture_error.is_some()
        || !successful(&output.status)
    {
        return Err("Docker context inspect did not complete successfully".into());
    }
    docker_context_endpoint(&output.stdout)
}

fn docker_context_endpoint(document: &[u8]) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(document)
        .map_err(|error| format!("Docker context inspect returned invalid JSON: {error}"))?;
    let contexts = value
        .as_array()
        .filter(|contexts| contexts.len() == 1)
        .ok_or_else(|| "Docker context inspect did not return exactly one context".to_owned())?;
    let endpoint = contexts[0]
        .get("Endpoints")
        .and_then(|endpoints| endpoints.get("docker"))
        .and_then(|docker| docker.get("Host"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "selected Docker context has no Docker endpoint".to_owned())?;
    if is_local_docker_endpoint(endpoint) {
        Ok(endpoint.to_owned())
    } else {
        Err("remote Docker endpoints are refused because bind paths are local to this host".into())
    }
}

fn is_local_docker_endpoint(endpoint: &str) -> bool {
    if endpoint.is_empty()
        || endpoint.len() > 512
        || endpoint.contains('\0')
        || endpoint.contains('\n')
        || endpoint.contains('\r')
    {
        return false;
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return path.starts_with('/');
    }

    let lowercase = endpoint.to_ascii_lowercase();
    for prefix in ["npipe:////./pipe/", "npipe:////localhost/pipe/"] {
        if let Some(pipe) = lowercase.strip_prefix(prefix) {
            return !pipe.is_empty() && !pipe.contains('/') && !pipe.contains('\\');
        }
    }

    let Some(authority) = endpoint.strip_prefix("tcp://") else {
        return false;
    };
    if authority.is_empty()
        || authority
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'?' | b'#' | b'@'))
    {
        return false;
    }

    let (host, port_valid) = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((address, suffix)) = bracketed.split_once(']') else {
            return false;
        };
        let port_valid = suffix.is_empty() || suffix.strip_prefix(':').is_some_and(valid_tcp_port);
        (address, port_valid)
    } else {
        if authority.matches(':').count() > 1 {
            return false;
        }
        match authority.split_once(':') {
            Some((host, port)) => (host, valid_tcp_port(port)),
            None => (authority, true),
        }
    };
    if !port_valid {
        return false;
    }
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn valid_tcp_port(port: &str) -> bool {
    port.parse::<u16>().is_ok_and(|port| port != 0)
}

fn finish_run_output(
    output: Result<ProcessOutput, String>,
    container_id: Option<String>,
    cleanup: impl FnOnce(Option<&str>) -> bool,
) -> Result<(ProcessOutput, bool), OciSandboxError> {
    let cleanup_verified = cleanup(container_id.as_deref());
    match output {
        Ok(output) => Ok((output, cleanup_verified)),
        Err(reason) if cleanup_verified => Err(OciSandboxError::EngineUnavailable { reason }),
        Err(_) => Err(OciSandboxError::CleanupUnverified { container_id }),
    }
}

fn unique_run_label() -> String {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{timestamp}-{sequence}", std::process::id())
}

struct NormalizedRequest {
    input_dir: PathBuf,
    output_dir: PathBuf,
}

fn validate_request(request: &SandboxRequest) -> Result<NormalizedRequest, OciSandboxError> {
    if !valid_digest_pinned_image(&request.image) {
        return Err(invalid(
            "image",
            "must be a repository reference pinned to a lowercase SHA-256 digest",
        ));
    }
    if request.command.is_empty()
        || request.command.len() > 4_096
        || !request.command.starts_with('/')
        || request.command.contains('\0')
        || request.command.contains('\n')
        || request.command.contains('\r')
    {
        return Err(invalid("command", "must be a bounded absolute image path"));
    }
    if request.arguments.len() > 128
        || request.arguments.iter().any(|arg| arg.contains('\0'))
        || request.arguments.iter().map(String::len).sum::<usize>() > 32_768
    {
        return Err(invalid(
            "arguments",
            "exceeds the argument count or byte bound",
        ));
    }
    validate_limits(&request.limits)?;
    let input_dir = canonical_directory(&request.input_dir, "input_dir")?;
    let output_dir = new_output_directory(&request.output_dir)?;
    let input_key = path_key(&input_dir)?;
    let output_key = path_key(&output_dir)?;
    if path_contains(&input_key, &output_key) || path_contains(&output_key, &input_key) {
        return Err(invalid(
            "output_dir",
            "must be separate from and outside the read-only input directory",
        ));
    }
    for (field, path) in [
        ("input_dir", input_dir.as_path()),
        ("output_dir", output_dir.as_path()),
    ] {
        let text = path
            .to_str()
            .ok_or_else(|| invalid(field, "must be representable as Unicode for Docker"))?;
        if text.contains('\n') || text.contains('\r') || text.contains('\0') {
            return Err(invalid(field, "contains a control character"));
        }
        if field == "input_dir" && text.contains(',') {
            return Err(invalid(field, "contains a Docker mount delimiter"));
        }
    }
    Ok(NormalizedRequest {
        input_dir,
        output_dir,
    })
}

fn validate_limits(limits: &SandboxLimits) -> Result<(), OciSandboxError> {
    if !(MIN_MEMORY_BYTES..=MAX_MEMORY_BYTES).contains(&limits.memory_bytes) {
        return Err(invalid(
            "memory_bytes",
            "is outside the 64 MiB..16 GiB bound",
        ));
    }
    if limits.cpu_millis == 0 || limits.cpu_millis > MAX_CPU_MILLIS {
        return Err(invalid("cpu_millis", "must be between 1 and 32000"));
    }
    if !(2..=MAX_PIDS).contains(&limits.max_processes) {
        return Err(invalid("max_processes", "must be between 2 and 4096"));
    }
    if !(100..=MAX_TIMEOUT_MS).contains(&limits.timeout_ms) {
        return Err(invalid("timeout_ms", "must be between 100 and 3600000"));
    }
    if !(1_024..=MAX_CAPTURE_BYTES).contains(&limits.max_output_bytes) {
        return Err(invalid(
            "max_output_bytes",
            "must be between 1024 and 64 MiB",
        ));
    }
    if !(1_024..=MAX_ARTIFACT_BYTES).contains(&limits.max_artifact_bytes) {
        return Err(invalid(
            "max_artifact_bytes",
            "must be between 1024 bytes and 64 MiB",
        ));
    }
    if !(1..=MAX_ARTIFACT_ENTRIES).contains(&limits.max_artifact_entries) {
        return Err(invalid(
            "max_artifact_entries",
            "must be between 1 and 4096",
        ));
    }
    if limits.container_uid == 0 || limits.container_gid == 0 {
        return Err(invalid("container identity", "root uid or gid is refused"));
    }
    Ok(())
}

fn new_output_directory(path: &Path) -> Result<PathBuf, OciSandboxError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = canonical_directory(parent, "output_dir parent")?;
    let name = path
        .file_name()
        .filter(|name| *name != "." && *name != "..")
        .ok_or_else(|| invalid("output_dir", "must include a new directory name"))?;
    let output = parent.join(name);
    match fs::symlink_metadata(&output) {
        Ok(_) => {
            return Err(invalid(
                "output_dir",
                "must name a new directory path that does not already exist",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(invalid(
                "output_dir",
                "cannot verify that the new directory path is absent",
            ));
        }
    }
    Ok(output)
}

fn build_run_arguments(
    request: &SandboxRequest,
    normalized: &NormalizedRequest,
    cid_file: &Path,
    run_label: &str,
) -> Vec<OsString> {
    let limits = &request.limits;
    let input_mount = format!(
        "type=bind,source={},target=/aurora/input,readonly,bind-recursive=readonly",
        normalized.input_dir.display()
    );
    let cpu = format_cpu(limits.cpu_millis);
    let output_tmpfs = format!(
        "--tmpfs=/aurora/output:rw,noexec,nosuid,nodev,size={},nr_inodes={},uid={},gid={},mode=0700",
        limits.max_artifact_bytes.saturating_add(1),
        limits.max_artifact_entries.saturating_add(2),
        limits.container_uid,
        limits.container_gid,
    );
    let capture_tmpfs = format!(
        "--tmpfs=/aurora/capture:rw,noexec,nosuid,nodev,size={},nr_inodes=3,uid={},gid={},mode=0700",
        limits.max_output_bytes.saturating_add(1),
        limits.container_uid,
        limits.container_gid,
    );
    let mut args = vec![
        "run".into(),
        "--pull=never".into(),
        "--no-healthcheck".into(),
        "--network=none".into(),
        "--ipc=private".into(),
        "--cgroupns=private".into(),
        "--read-only".into(),
        "--cap-drop=ALL".into(),
        "--security-opt=no-new-privileges=true".into(),
        "--security-opt=seccomp=builtin".into(),
        "--pids-limit".into(),
        limits.max_processes.to_string().into(),
        "--memory".into(),
        limits.memory_bytes.to_string().into(),
        "--memory-swap".into(),
        limits.memory_bytes.to_string().into(),
        "--cpus".into(),
        cpu.into(),
        "--ulimit".into(),
        "nofile=1024:1024".into(),
        "--ulimit".into(),
        "core=0".into(),
        format!(
            "--tmpfs=/tmp:rw,noexec,nosuid,nodev,size=16m,nr_inodes={},uid={},gid={},mode=0700",
            MAX_SCRATCH_INODES, limits.container_uid, limits.container_gid
        )
        .into(),
        "--shm-size=16m".into(),
        "--user".into(),
        format!("{}:{}", limits.container_uid, limits.container_gid).into(),
        "--init".into(),
        "--workdir=/aurora/input".into(),
        "--label".into(),
        format!("{RUN_LABEL_KEY}={run_label}").into(),
        "--mount".into(),
        input_mount.into(),
        output_tmpfs.into(),
        capture_tmpfs.into(),
        "--platform".into(),
        request.platform.as_str().into(),
        "--cidfile".into(),
        cid_file.as_os_str().to_owned(),
        "--entrypoint".into(),
        SANDBOX_ENTRYPOINT.into(),
        request.image.clone().into(),
        "-c".into(),
        ARTIFACT_ARCHIVE_SCRIPT.into(),
        "aurora-command".into(),
        request.command.clone().into(),
    ];
    args.extend(request.arguments.iter().cloned().map(OsString::from));
    args
}

fn publish_artifacts(
    staging_dir: &Path,
    output_dir: &Path,
    limits: &SandboxLimits,
) -> Result<Vec<DockerArtifact>, OciSandboxError> {
    match fs::symlink_metadata(output_dir) {
        Ok(_) => {
            return Err(artifact_archive_error(
                "requested output path appeared before artifact publication",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(artifact_archive_error(format!(
                "cannot verify the output path before publication: {error}"
            )));
        }
    }
    let artifacts = validate_artifact_tree(staging_dir, limits)?;
    fs::rename(staging_dir, output_dir).map_err(|error| {
        OciSandboxError::ArtifactCollectionFailed {
            reason: format!("cannot atomically publish validated artifacts: {error}"),
        }
    })?;
    Ok(artifacts)
}

fn maximum_artifact_archive_bytes(limits: &SandboxLimits) -> Result<usize, OciSandboxError> {
    let artifact_bytes = limits
        .max_artifact_bytes
        .checked_add(1)
        .ok_or_else(|| artifact_archive_error("artifact byte ceiling overflowed"))?;
    let stdout_bytes = u64::try_from(limits.max_output_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or_else(|| artifact_archive_error("stdout byte ceiling overflowed"))?;
    let archive_overhead = u64::from(limits.max_artifact_entries)
        .checked_add(4)
        .and_then(|entries| entries.checked_mul(1_024))
        .and_then(|overhead| overhead.checked_add(10_240))
        .ok_or_else(|| artifact_archive_error("artifact archive overhead overflowed"))?;
    let total = artifact_bytes
        .checked_add(stdout_bytes)
        .and_then(|bytes| bytes.checked_add(archive_overhead))
        .and_then(|bytes| bytes.checked_add(ARTIFACT_ARCHIVE_SUCCESS_MARKER.len() as u64))
        .ok_or_else(|| artifact_archive_error("artifact archive byte ceiling overflowed"))?;
    usize::try_from(total)
        .map_err(|_| artifact_archive_error("artifact archive byte ceiling exceeds this platform"))
}

fn strip_archive_success_marker(stream: &[u8]) -> Result<&[u8], OciSandboxError> {
    stream
        .strip_suffix(ARTIFACT_ARCHIVE_SUCCESS_MARKER)
        .ok_or_else(|| {
            artifact_archive_error("container did not confirm successful USTAR archive creation")
        })
}

struct ParsedArtifactArchive {
    artifacts: Vec<DockerArtifact>,
    stdout: Vec<u8>,
    stdout_limit_exceeded: bool,
}

fn extract_artifact_archive(
    archive: &[u8],
    staging_dir: &Path,
    limits: &SandboxLimits,
) -> Result<ParsedArtifactArchive, OciSandboxError> {
    const TAR_BLOCK: usize = 512;
    let mut offset = 0_usize;
    let mut zero_blocks = 0_usize;
    let mut entries = BTreeMap::<String, bool>::new();
    let mut explicit_paths = BTreeSet::new();
    let mut portable_prefixes = BTreeMap::<String, String>::new();
    let mut artifacts = Vec::<DockerArtifact>::new();
    let mut artifact_bytes = 0_u64;
    let mut output_stdout = None;
    let mut stdout_limit_exceeded = false;

    while offset + TAR_BLOCK <= archive.len() {
        let header = &archive[offset..offset + TAR_BLOCK];
        if header.iter().all(|byte| *byte == 0) {
            zero_blocks += 1;
            offset += TAR_BLOCK;
            if archive[offset..].iter().any(|byte| *byte != 0) {
                return Err(artifact_archive_error(
                    "tar data follows the end-of-archive marker",
                ));
            }
            if zero_blocks >= 2 {
                if (archive.len() - offset) % TAR_BLOCK != 0 {
                    return Err(artifact_archive_error(
                        "tar end padding is not a whole number of blocks",
                    ));
                }
                if output_stdout.is_none() {
                    return Err(artifact_archive_error(
                        "tar archive does not contain the captured command stdout",
                    ));
                }
                artifacts.sort_by(|left, right| left.path.cmp(&right.path));
                return Ok(ParsedArtifactArchive {
                    artifacts,
                    stdout: output_stdout.unwrap_or_default(),
                    stdout_limit_exceeded,
                });
            }
            continue;
        }
        if zero_blocks != 0 {
            return Err(artifact_archive_error(
                "tar contains an entry after an end-of-archive block",
            ));
        }
        verify_tar_checksum(header)?;
        let type_flag = header[156];
        let is_directory = type_flag == b'5';
        let is_regular = type_flag == 0 || type_flag == b'0';
        if !is_directory && !is_regular {
            return Err(artifact_archive_error(
                "only USTAR regular files and directories are accepted",
            ));
        }

        let name = decode_tar_field(&header[..100])?;
        if &header[257..263] != b"ustar\0" && &header[257..263] != b"ustar " {
            return Err(artifact_archive_error("tar entry is not in USTAR format"));
        }
        let prefix = decode_tar_field(&header[345..500])?;
        let mut raw_path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        while let Some(path) = raw_path.strip_prefix("./") {
            raw_path = path.to_owned();
        }
        if is_directory {
            raw_path = raw_path.trim_end_matches('/').to_owned();
        }
        let size = parse_tar_octal(&header[124..136])?;
        if is_directory && size != 0 {
            return Err(artifact_archive_error(
                "tar directory entries must not contain file data",
            ));
        }
        let data_start = offset
            .checked_add(TAR_BLOCK)
            .ok_or_else(|| artifact_archive_error("tar entry offset overflowed"))?;
        let size_usize = usize::try_from(size)
            .map_err(|_| artifact_archive_error("tar file size exceeds this platform"))?;
        let data_end = data_start
            .checked_add(size_usize)
            .ok_or_else(|| artifact_archive_error("tar file extent overflowed"))?;
        let padded_size = size_usize
            .checked_add(TAR_BLOCK - 1)
            .map(|size| size / TAR_BLOCK * TAR_BLOCK)
            .ok_or_else(|| artifact_archive_error("tar padding overflowed"))?;
        let next = data_start
            .checked_add(padded_size)
            .ok_or_else(|| artifact_archive_error("tar entry extent overflowed"))?;
        if data_end > archive.len() || next > archive.len() {
            return Err(artifact_archive_error("tar entry is truncated"));
        }

        if raw_path.is_empty() || raw_path == "." {
            if !is_directory || size != 0 {
                return Err(artifact_archive_error(
                    "only a zero-length directory may name the archive root",
                ));
            }
            offset = next;
            continue;
        }
        if raw_path == "output" || raw_path == "capture" {
            if !is_directory {
                return Err(artifact_archive_error(
                    "the output and capture roots must be directories",
                ));
            }
            if !explicit_paths.insert(raw_path.clone()) {
                return Err(artifact_archive_error("tar contains a duplicate path"));
            }
            offset = next;
            continue;
        }
        if raw_path == "capture/stdout" {
            if !is_regular || output_stdout.is_some() {
                return Err(artifact_archive_error(
                    "capture/stdout must be present once as a regular file",
                ));
            }
            let contents = &archive[data_start..data_end];
            let max_stdout = limits.max_output_bytes;
            stdout_limit_exceeded = contents.len() > max_stdout;
            output_stdout = Some(contents[..contents.len().min(max_stdout)].to_vec());
            offset = next;
            continue;
        }
        let relative = raw_path.strip_prefix("output/").ok_or_else(|| {
            artifact_archive_error("tar contains an entry outside output and capture")
        })?;
        let relative = parse_artifact_path(relative)?;
        let component_count = relative.split('/').count();
        let directory_depth = if is_directory {
            component_count
        } else {
            component_count.saturating_sub(1)
        };
        if directory_depth > MAX_ARTIFACT_DEPTH {
            return Err(artifact_archive_error(
                "artifact directory depth exceeds the admitted bound",
            ));
        }
        register_portable_path_prefixes(&mut portable_prefixes, &relative)?;
        if !explicit_paths.insert(relative.clone()) {
            return Err(artifact_archive_error("tar contains a duplicate path"));
        }

        let relative_components: Vec<_> = relative.split('/').collect();
        let mut parent = PathBuf::from(staging_dir);
        for (index, component) in relative_components
            .iter()
            .take(relative_components.len() - 1)
            .enumerate()
        {
            parent.push(component);
            let parent_key = relative_components[..=index].join("/");
            match entries.get(&parent_key) {
                Some(false) => {
                    return Err(artifact_archive_error(
                        "a regular file is used as an artifact parent",
                    ));
                }
                Some(true) => {}
                None => {
                    register_artifact_entry(
                        &mut entries,
                        parent_key,
                        true,
                        limits.max_artifact_entries,
                    )?;
                    create_private_directory(&parent).map_err(|error| {
                        artifact_archive_error(format!(
                            "cannot create a staged artifact directory: {error}"
                        ))
                    })?;
                }
            }
        }

        let destination = staging_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if is_directory {
            match entries.get(&relative) {
                Some(false) => {
                    return Err(artifact_archive_error(
                        "a directory conflicts with an artifact file",
                    ));
                }
                Some(true) => {}
                None => {
                    register_artifact_entry(
                        &mut entries,
                        relative.clone(),
                        true,
                        limits.max_artifact_entries,
                    )?;
                    create_private_directory(&destination).map_err(|error| {
                        artifact_archive_error(format!(
                            "cannot create a staged artifact directory: {error}"
                        ))
                    })?;
                }
            }
        } else {
            if entries.get(&relative) == Some(&true) {
                return Err(artifact_archive_error(
                    "a regular file conflicts with an artifact directory",
                ));
            }
            register_artifact_entry(
                &mut entries,
                relative.clone(),
                false,
                limits.max_artifact_entries,
            )?;
            artifact_bytes = artifact_bytes.checked_add(size).ok_or_else(|| {
                artifact_archive_error("aggregate artifact byte count overflowed")
            })?;
            if artifact_bytes > limits.max_artifact_bytes {
                return Err(artifact_archive_error(
                    "aggregate artifact bytes exceed the admitted bound",
                ));
            }
            let contents = &archive[data_start..data_end];
            let mut file = create_private_file(&destination).map_err(|error| {
                artifact_archive_error(format!("cannot create a staged artifact file: {error}"))
            })?;
            file.write_all(contents).map_err(|error| {
                artifact_archive_error(format!("cannot write a staged artifact file: {error}"))
            })?;
            artifacts.push(DockerArtifact {
                path: relative,
                bytes: size,
                sha256: bioprism_ids::ContentHash::of_bytes(contents).to_string(),
            });
        }
        offset = next;
    }

    Err(artifact_archive_error(
        "tar is missing its complete end-of-archive marker",
    ))
}

fn register_artifact_entry(
    entries: &mut BTreeMap<String, bool>,
    path: String,
    is_directory: bool,
    maximum_entries: u32,
) -> Result<(), OciSandboxError> {
    if !entries.contains_key(&path) && entries.len() >= maximum_entries as usize {
        return Err(artifact_archive_error(
            "artifact entry count exceeds the admitted bound",
        ));
    }
    if let Some(existing) = entries.get(&path) {
        if *existing != is_directory {
            return Err(artifact_archive_error(
                "artifact path is used as both a file and a directory",
            ));
        }
    } else {
        entries.insert(path, is_directory);
    }
    Ok(())
}

fn parse_artifact_path(path: &str) -> Result<String, OciSandboxError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || (path.len() >= 2
            && path.as_bytes()[0].is_ascii_alphabetic()
            && path.as_bytes()[1] == b':')
    {
        return Err(artifact_archive_error(
            "tar path is absolute, empty, or not portable",
        ));
    }
    let components: Vec<_> = path.split('/').collect();
    if components.iter().any(|component| {
        component.is_empty()
            || *component == "."
            || *component == ".."
            || component.contains(':')
            || component.len() > 255
            || component.encode_utf16().count() > 255
            || component.ends_with(['.', ' '])
            || component
                .chars()
                .any(|character| char::is_control(character) || "?*<>|\"".contains(character))
            || is_reserved_windows_component(component)
    }) {
        return Err(artifact_archive_error(
            "tar path contains an unsafe component",
        ));
    }
    if path.len() > MAX_ARTIFACT_PATH_BYTES {
        return Err(artifact_archive_error(
            "artifact path exceeds the admitted byte bound",
        ));
    }
    Ok(path.to_owned())
}

fn is_reserved_windows_component(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    )
}

fn register_portable_path_prefixes(
    prefixes: &mut BTreeMap<String, String>,
    path: &str,
) -> Result<(), OciSandboxError> {
    let mut prefix = String::new();
    for component in path.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        let case_key = prefix.to_lowercase();
        if let Some(existing) = prefixes.get(&case_key) {
            if existing != &prefix {
                return Err(artifact_archive_error(
                    "artifact paths collide under case-insensitive filesystem rules",
                ));
            }
        } else {
            prefixes.insert(case_key, prefix.clone());
        }
    }
    Ok(())
}

fn verify_tar_checksum(header: &[u8]) -> Result<(), OciSandboxError> {
    let supplied = parse_tar_octal(&header[148..156])?;
    let computed = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum::<u64>();
    if supplied != computed {
        return Err(artifact_archive_error("tar header checksum is invalid"));
    }
    Ok(())
}

fn parse_tar_octal(field: &[u8]) -> Result<u64, OciSandboxError> {
    let text = field
        .iter()
        .copied()
        .skip_while(|byte| *byte == b' ' || *byte == 0)
        .take_while(|byte| *byte != b' ' && *byte != 0)
        .collect::<Vec<_>>();
    if text.is_empty() || text.iter().any(|byte| !(b'0'..=b'7').contains(byte)) {
        return Err(artifact_archive_error(
            "tar contains a non-octal or empty numeric field",
        ));
    }
    text.into_iter().try_fold(0_u64, |value, byte| {
        value
            .checked_mul(8)
            .and_then(|value| value.checked_add(u64::from(byte - b'0')))
            .ok_or_else(|| artifact_archive_error("tar numeric field overflowed"))
    })
}

fn decode_tar_field(field: &[u8]) -> Result<String, OciSandboxError> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    std::str::from_utf8(&field[..end])
        .map(str::to_owned)
        .map_err(|_| artifact_archive_error("tar path is not valid UTF-8"))
}

fn artifact_archive_error(reason: impl Into<String>) -> OciSandboxError {
    OciSandboxError::ArtifactCollectionFailed {
        reason: reason.into(),
    }
}

fn validate_artifact_tree(
    staging_dir: &Path,
    limits: &SandboxLimits,
) -> Result<Vec<DockerArtifact>, OciSandboxError> {
    let mut pending = vec![(staging_dir.to_path_buf(), PathBuf::new(), 0_usize)];
    let mut artifacts = Vec::new();
    let mut entry_count = 0_u32;
    let mut byte_count = 0_u64;

    while let Some((directory, relative_directory, depth)) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            OciSandboxError::ArtifactCollectionFailed {
                reason: format!("cannot inspect staged artifacts: {error}"),
            }
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| OciSandboxError::ArtifactCollectionFailed {
                reason: format!("cannot inspect a staged artifact: {error}"),
            })?;
            entry_count = entry_count.saturating_add(1);
            if entry_count > limits.max_artifact_entries {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "artifact entry count exceeds the admitted bound".into(),
                });
            }

            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| OciSandboxError::ArtifactCollectionFailed {
                    reason: "artifact path is not valid Unicode".into(),
                })?;
            if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "artifact contains a non-portable path component".into(),
                });
            }
            let relative_path = relative_directory.join(name);
            let path = portable_relative_path(&relative_path)?;
            if path.len() > MAX_ARTIFACT_PATH_BYTES {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "artifact path exceeds the admitted byte bound".into(),
                });
            }

            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
                OciSandboxError::ArtifactCollectionFailed {
                    reason: format!("cannot inspect staged artifact metadata: {error}"),
                }
            })?;
            if metadata.file_type().is_symlink() {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "symbolic links are refused in sandbox output".into(),
                });
            }
            if metadata.is_dir() {
                if depth >= MAX_ARTIFACT_DEPTH {
                    return Err(OciSandboxError::ArtifactCollectionFailed {
                        reason: "artifact directory depth exceeds the admitted bound".into(),
                    });
                }
                pending.push((entry.path(), relative_path, depth + 1));
                continue;
            }
            if !metadata.is_file() {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "only regular files and directories are accepted as artifacts".into(),
                });
            }

            byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
                OciSandboxError::ArtifactCollectionFailed {
                    reason: "aggregate artifact byte count overflowed".into(),
                }
            })?;
            if byte_count > limits.max_artifact_bytes {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "aggregate artifact bytes exceed the admitted bound".into(),
                });
            }
            let contents = fs::read(entry.path()).map_err(|error| {
                OciSandboxError::ArtifactCollectionFailed {
                    reason: format!("cannot read a staged artifact: {error}"),
                }
            })?;
            if contents.len() as u64 != metadata.len() {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "artifact size changed while it was being verified".into(),
                });
            }
            artifacts.push(DockerArtifact {
                path,
                bytes: metadata.len(),
                sha256: bioprism_ids::ContentHash::of_bytes(&contents).to_string(),
            });
        }
    }

    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(artifacts)
}

fn portable_relative_path(path: &Path) -> Result<String, OciSandboxError> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value =
                    value
                        .to_str()
                        .ok_or_else(|| OciSandboxError::ArtifactCollectionFailed {
                            reason: "artifact path is not valid Unicode".into(),
                        })?;
                components.push(value);
            }
            _ => {
                return Err(OciSandboxError::ArtifactCollectionFailed {
                    reason: "artifact path is not relative and normalized".into(),
                });
            }
        }
    }
    if components.is_empty() {
        return Err(OciSandboxError::ArtifactCollectionFailed {
            reason: "artifact path is empty".into(),
        });
    }
    Ok(components.join("/"))
}

fn format_cpu(cpu_millis: u32) -> String {
    let whole = cpu_millis / 1_000;
    let remainder = cpu_millis % 1_000;
    if remainder == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{remainder:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn valid_digest_pinned_image(image: &str) -> bool {
    let Some((repository, digest)) = image.rsplit_once("@sha256:") else {
        return false;
    };
    !repository.is_empty()
        && repository.len() <= 512
        && !repository.contains(char::is_whitespace)
        && !repository.contains('@')
        && !repository.contains('\0')
        && digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn ensure_no_image_volumes(output: &[u8]) -> Result<(), OciSandboxError> {
    let volumes: serde_json::Value =
        serde_json::from_slice(output).map_err(|_| OciSandboxError::EngineUnavailable {
            reason: "docker image inspect returned invalid volume metadata".into(),
        })?;
    match volumes {
        serde_json::Value::Null => Ok(()),
        serde_json::Value::Object(values) if values.is_empty() => Ok(()),
        serde_json::Value::Object(_) => Err(OciSandboxError::ImageRejected {
            reason: "image declares writable Docker volumes outside the admitted tmpfs mounts"
                .into(),
        }),
        _ => Err(OciSandboxError::EngineUnavailable {
            reason: "docker image inspect returned an unexpected volume metadata shape".into(),
        }),
    }
}

fn image_volume_inspection_arguments(image: &str, platform: LinuxPlatform) -> [OsString; 6] {
    [
        OsString::from("image"),
        OsString::from("inspect"),
        OsString::from(format!("--platform={}", platform.as_str())),
        OsString::from("--format"),
        OsString::from("{{json .Config.Volumes}}"),
        OsString::from(image),
    ]
}

fn container_remove_arguments(container_id: &str) -> [OsString; 4] {
    [
        OsString::from("rm"),
        OsString::from("--force"),
        OsString::from("--volumes"),
        OsString::from(container_id),
    ]
}

fn canonical_directory(path: &Path, field: &'static str) -> Result<PathBuf, OciSandboxError> {
    let canonical = fs::canonicalize(path)
        .map_err(|_| invalid(field, "must name an existing readable directory"))?;
    if !canonical.is_dir() {
        return Err(invalid(field, "must name a directory"));
    }
    Ok(canonical)
}

fn path_key(path: &Path) -> Result<String, OciSandboxError> {
    let value = path
        .to_str()
        .ok_or_else(|| invalid("directory", "must be representable as Unicode"))?;
    #[cfg(windows)]
    let value = value.to_ascii_lowercase();
    Ok(value.replace('\\', "/").trim_end_matches('/').to_owned())
}

fn path_contains(parent: &str, child: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn read_container_id(path: &Path) -> Result<String, OciSandboxError> {
    let metadata = fs::metadata(path)
        .map_err(|_| OciSandboxError::CleanupUnverified { container_id: None })?;
    if !metadata.is_file() || metadata.len() > 128 {
        return Err(OciSandboxError::CleanupUnverified { container_id: None });
    }
    let value = fs::read_to_string(path)
        .map_err(|_| OciSandboxError::CleanupUnverified { container_id: None })?;
    let value = value.trim();
    if !valid_container_id(value) {
        return Err(OciSandboxError::CleanupUnverified { container_id: None });
    }
    Ok(value.to_owned())
}

fn valid_container_id(value: &str) -> bool {
    (12..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(field: &'static str, reason: impl Into<String>) -> OciSandboxError {
    OciSandboxError::InvalidRequest {
        field,
        reason: reason.into(),
    }
}

struct TemporaryDirectory {
    path: PathBuf,
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

fn create_private_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

impl TemporaryDirectory {
    fn create_in(root: &Path) -> Result<Self, OciSandboxError> {
        for _ in 0..128 {
            let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("aurora-oci-{}-{sequence}", std::process::id()));
            let created = create_private_directory(&path);
            match created {
                Ok(()) => {
                    return Ok(TemporaryDirectory { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(OciSandboxError::EngineUnavailable {
                        reason: format!(
                            "cannot create bounded temporary state in {}: {error}",
                            root.display()
                        ),
                    });
                }
            }
        }
        Err(OciSandboxError::EngineUnavailable {
            reason: format!(
                "cannot allocate a unique bounded temporary directory in {}",
                root.display()
            ),
        })
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
struct ProcessOutput {
    status: Option<ExitStatus>,
    timed_out: bool,
    output_limit_exceeded: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    capture_error: Option<String>,
}

struct CapturedPipe {
    bytes: Vec<u8>,
    overflowed: bool,
    read_error: Option<String>,
}

fn read_pipe<R: Read>(
    mut reader: R,
    stream_captured: Arc<AtomicUsize>,
    combined_captured: Arc<AtomicUsize>,
    capture_failed: Arc<AtomicBool>,
    output_limit_exceeded: Arc<AtomicBool>,
    stream_maximum: usize,
    combined_maximum: usize,
) -> CapturedPipe {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8_192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                return CapturedPipe {
                    bytes,
                    overflowed: false,
                    read_error: None,
                };
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                capture_failed.store(true, Ordering::Relaxed);
                return CapturedPipe {
                    bytes,
                    overflowed: false,
                    read_error: Some(format!("cannot capture Docker output: {error}")),
                };
            }
            Ok(length) => {
                let stream_previous = stream_captured.fetch_add(length, Ordering::Relaxed);
                let combined_previous = combined_captured.fetch_add(length, Ordering::Relaxed);
                if stream_previous.saturating_add(length) > stream_maximum
                    || combined_previous.saturating_add(length) > combined_maximum
                {
                    output_limit_exceeded.store(true, Ordering::Relaxed);
                    return CapturedPipe {
                        bytes,
                        overflowed: true,
                        read_error: None,
                    };
                }
                bytes.extend_from_slice(&buffer[..length]);
            }
        }
    }
}

fn run_bounded(
    executable: &Path,
    args: &[OsString],
    timeout: Duration,
    maximum_output: usize,
    fixed_endpoint: Option<&str>,
) -> Result<ProcessOutput, String> {
    run_bounded_with_stream_limits(
        executable,
        args,
        timeout,
        maximum_output,
        maximum_output,
        maximum_output,
        fixed_endpoint,
    )
}

fn run_bounded_with_stream_limits(
    executable: &Path,
    args: &[OsString],
    timeout: Duration,
    maximum_stdout: usize,
    maximum_stderr: usize,
    maximum_combined: usize,
    fixed_endpoint: Option<&str>,
) -> Result<ProcessOutput, String> {
    let mut command = Command::new(executable);
    if fixed_endpoint.is_some() {
        command
            .env_remove("DOCKER_HOST")
            .env_remove("DOCKER_CONTEXT");
    }
    let mut child = command
        .args(docker_cli_arguments(fixed_endpoint, args))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start Docker CLI: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Docker CLI stdout pipe was not created".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Docker CLI stderr pipe was not created".to_owned())?;
    let combined_captured = Arc::new(AtomicUsize::new(0));
    let capture_failed = Arc::new(AtomicBool::new(false));
    let output_limit_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_captured = Arc::new(AtomicUsize::new(0));
    let stdout_thread = {
        let stream_captured = Arc::clone(&stdout_captured);
        let combined_captured = Arc::clone(&combined_captured);
        let capture_failed = Arc::clone(&capture_failed);
        let output_limit_exceeded = Arc::clone(&output_limit_exceeded);
        thread::spawn(move || {
            read_pipe(
                stdout,
                stream_captured,
                combined_captured,
                capture_failed,
                output_limit_exceeded,
                maximum_stdout,
                maximum_combined,
            )
        })
    };
    let stderr_thread = {
        let stream_captured = Arc::new(AtomicUsize::new(0));
        let combined_captured = Arc::clone(&combined_captured);
        let capture_failed = Arc::clone(&capture_failed);
        let output_limit_exceeded = Arc::clone(&output_limit_exceeded);
        thread::spawn(move || {
            read_pipe(
                stderr,
                stream_captured,
                combined_captured,
                capture_failed,
                output_limit_exceeded,
                maximum_stderr,
                maximum_combined,
            )
        })
    };
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut status = None;
    loop {
        if output_limit_exceeded.load(Ordering::Relaxed) || capture_failed.load(Ordering::Relaxed) {
            let _ = child.kill();
            break;
        }
        match child.try_wait() {
            Ok(Some(observed)) => {
                status = Some(observed);
                break;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(format!("cannot observe Docker CLI: {error}"));
            }
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if status.is_none() {
        status = child.wait().ok();
    }
    let stdout = stdout_thread
        .join()
        .map_err(|_| "Docker stdout collector panicked".to_owned())?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "Docker stderr collector panicked".to_owned())?;
    let capture_error = stdout.read_error.or(stderr.read_error);
    Ok(ProcessOutput {
        status,
        timed_out,
        output_limit_exceeded: stdout.overflowed || stderr.overflowed,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        capture_error,
    })
}

fn docker_cli_arguments(endpoint: Option<&str>, args: &[OsString]) -> Vec<OsString> {
    let extra_arguments = if endpoint.is_some() { 2 } else { 0 };
    let mut command_args = Vec::with_capacity(args.len() + extra_arguments);
    if let Some(endpoint) = endpoint {
        command_args.push(OsString::from("--host"));
        command_args.push(OsString::from(endpoint));
    }
    command_args.extend(args.iter().cloned());
    command_args
}

fn successful(status: &Option<ExitStatus>) -> bool {
    status.as_ref().is_some_and(ExitStatus::success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn request(input_dir: PathBuf, output_dir: PathBuf) -> SandboxRequest {
        SandboxRequest {
            image: format!("registry.example/research/tool@sha256:{}", "a".repeat(64)),
            platform: LinuxPlatform::Amd64,
            command: "/usr/bin/python3".into(),
            arguments: vec![
                "/aurora/input/run.py".into(),
                "--output".into(),
                "/aurora/output".into(),
            ],
            input_dir,
            output_dir,
            limits: SandboxLimits::default(),
        }
    }

    fn tar_entry(archive: &mut Vec<u8>, name: &str, kind: u8, contents: &[u8]) {
        let (prefix, name) = if name.len() < 100 {
            ("", name)
        } else {
            name.rsplit_once('/').expect("long tar path has a prefix")
        };
        assert!(name.len() < 100);
        assert!(prefix.len() < 155);
        let mut header = [0_u8; 512];
        header[..name.len()].copy_from_slice(name.as_bytes());
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        let size = format!("{:011o}\0", contents.len());
        header[124..136].copy_from_slice(size.as_bytes());
        header[136..148].copy_from_slice(b"00000000000\0");
        header[148..156].fill(b' ');
        header[156] = kind;
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        header[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
        let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
        let checksum_field = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum_field.as_bytes());
        archive.extend_from_slice(&header);
        archive.extend_from_slice(contents);
        let padding = (512 - contents.len() % 512) % 512;
        archive.resize(archive.len() + padding, 0);
    }

    fn valid_tar_archive() -> Vec<u8> {
        let mut archive = Vec::new();
        tar_entry(&mut archive, "output/", b'5', b"");
        tar_entry(&mut archive, "output/nested/", b'5', b"");
        tar_entry(&mut archive, "output/nested/a.txt", b'0', b"a");
        tar_entry(&mut archive, "output/z.txt", b'0', b"z");
        let prefixed_path = format!("output/{}/prefixed.txt", "p".repeat(101));
        tar_entry(&mut archive, &prefixed_path, b'0', b"prefix");
        tar_entry(&mut archive, "capture/", b'5', b"");
        tar_entry(&mut archive, "capture/stdout", b'0', b"hello");
        archive.resize(archive.len() + 1_024, 0);
        archive
    }

    fn archive_staging_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aurora-oci-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("fresh archive staging directory");
        path
    }

    #[test]
    fn command_is_safely_wrapped_and_carries_every_enforcement_flag() {
        let root = std::env::temp_dir().join(format!("aurora-oci-test-{}", std::process::id()));
        let input = root.join("input");
        let output = root.join("output");
        fs::create_dir_all(&input).expect("input directory");
        let request = request(input.clone(), output.clone());
        let normalized = validate_request(&request).expect("valid request");
        let args = build_run_arguments(&request, &normalized, &root.join("cid"), "test-run");
        let args: Vec<String> = args
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        for required in [
            "--pull=never",
            "--network=none",
            "--cgroupns=private",
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges=true",
            "--security-opt=seccomp=builtin",
            "--pids-limit",
            "--memory",
            "--memory-swap",
            "--cpus",
            "--ulimit",
            "--label",
            "--cidfile",
            "--entrypoint",
        ] {
            assert!(
                args.iter().any(|argument| argument == required),
                "{required}"
            );
        }
        assert!(!args.iter().any(|argument| argument == "--rm"));
        assert!(args.iter().any(|argument| {
            argument.contains("--tmpfs=/aurora/output:")
                && argument.contains("size=16777217")
                && argument.contains("nr_inodes=1026")
                && argument.contains("mode=0700")
        }));
        assert!(args.iter().any(|argument| {
            argument.contains("--tmpfs=/aurora/capture:")
                && argument.contains("size=1048577")
                && argument.contains("nr_inodes=3")
        }));
        assert!(args.iter().any(|argument| {
            argument.contains("--tmpfs=/tmp:")
                && argument.contains("size=16m")
                && argument.contains("nr_inodes=4096")
        }));
        assert!(args.iter().any(|argument| argument == "--shm-size=16m"));
        assert!(!args
            .iter()
            .any(|argument| argument.contains(&output.to_string_lossy().to_string())));
        assert!(args.iter().any(|argument| argument.contains("readonly")));
        assert!(args.iter().any(|argument| {
            argument.contains("target=/aurora/input,readonly,bind-recursive=readonly")
        }));
        assert!(args
            .iter()
            .any(|argument| argument == &format!("{RUN_LABEL_KEY}=test-run")));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--entrypoint" && pair[1] == SANDBOX_ENTRYPOINT));
        let image_index = args
            .iter()
            .position(|argument| argument == &request.image)
            .expect("image argument");
        assert_eq!(args[image_index + 1], "-c");
        assert_eq!(args[image_index + 2], ARTIFACT_ARCHIVE_SCRIPT);
        assert_eq!(args[image_index + 3], "aurora-command");
        assert_eq!(args[image_index + 4], request.command);
        assert_eq!(args[image_index + 5], request.arguments[0]);
        assert!(args.iter().any(|argument| argument == &request.image));
        assert_eq!(format_cpu(1_250), "1.25");
        assert_eq!(format_cpu(500), "0.5");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tar_archive_collects_hashed_artifacts_and_separate_stdout() {
        let staging = archive_staging_dir("valid-tar");
        let parsed =
            extract_artifact_archive(&valid_tar_archive(), &staging, &SandboxLimits::default())
                .expect("valid bounded archive");
        assert_eq!(parsed.stdout, b"hello");
        assert!(!parsed.stdout_limit_exceeded);
        assert_eq!(parsed.artifacts.len(), 3);
        let nested = parsed
            .artifacts
            .iter()
            .find(|artifact| artifact.path == "nested/a.txt")
            .expect("nested artifact");
        assert_eq!(nested.bytes, 1);
        assert_eq!(
            nested.sha256,
            bioprism_ids::ContentHash::of_bytes(b"a").to_string()
        );
        assert!(parsed
            .artifacts
            .iter()
            .any(|artifact| artifact.path == format!("{}/prefixed.txt", "p".repeat(101))));
        assert!(parsed
            .artifacts
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path));
        assert_eq!(
            fs::read(staging.join("nested").join("a.txt")).unwrap(),
            b"a"
        );

        let second_staging = archive_staging_dir("stdout-limit");
        let limits = SandboxLimits {
            max_output_bytes: 4,
            ..SandboxLimits::default()
        };
        let parsed = extract_artifact_archive(&valid_tar_archive(), &second_staging, &limits)
            .expect("stdout limit is reported in the parse receipt");
        assert_eq!(parsed.stdout, b"hell");
        assert!(parsed.stdout_limit_exceeded);
        let _ = fs::remove_dir_all(staging);
        let _ = fs::remove_dir_all(second_staging);
    }

    #[test]
    fn wrapper_archive_success_marker_is_required_and_removed_before_tar_parsing() {
        let archive = valid_tar_archive();
        assert!(strip_archive_success_marker(&archive).is_err());
        let mut stream = archive.clone();
        stream.extend_from_slice(ARTIFACT_ARCHIVE_SUCCESS_MARKER);
        assert_eq!(strip_archive_success_marker(&stream).unwrap(), archive);
    }

    #[test]
    fn process_capture_enforces_both_stream_and_combined_ceilings() {
        let combined = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let exceeded = Arc::new(AtomicBool::new(false));
        let stdout = read_pipe(
            io::Cursor::new(b"12345"),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&combined),
            Arc::clone(&failed),
            Arc::clone(&exceeded),
            4,
            8,
        );
        assert!(stdout.overflowed);
        assert!(stdout.bytes.is_empty());
        assert!(exceeded.load(Ordering::Relaxed));

        let combined = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let exceeded = Arc::new(AtomicBool::new(false));
        let stdout = read_pipe(
            io::Cursor::new(b"abc"),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&combined),
            Arc::clone(&failed),
            Arc::clone(&exceeded),
            4,
            5,
        );
        assert!(!stdout.overflowed);
        let stderr = read_pipe(
            io::Cursor::new(b"def"),
            Arc::new(AtomicUsize::new(0)),
            combined,
            failed,
            exceeded.clone(),
            4,
            5,
        );
        assert!(stderr.overflowed);
        assert!(exceeded.load(Ordering::Relaxed));
    }

    #[test]
    fn tar_archive_rejects_unsafe_paths_special_files_bad_checksums_and_truncation() {
        let staging = archive_staging_dir("invalid-tar");
        let mut traversal = Vec::new();
        tar_entry(&mut traversal, "output/../escape", b'0', b"x");
        traversal.resize(traversal.len() + 1_024, 0);
        assert!(extract_artifact_archive(&traversal, &staging, &SandboxLimits::default()).is_err());

        let mut absolute = Vec::new();
        tar_entry(&mut absolute, "/output/escape", b'0', b"x");
        absolute.resize(absolute.len() + 1_024, 0);
        assert!(extract_artifact_archive(&absolute, &staging, &SandboxLimits::default()).is_err());

        let mut symlink = Vec::new();
        tar_entry(&mut symlink, "output/link", b'2', b"target");
        symlink.resize(symlink.len() + 1_024, 0);
        assert!(extract_artifact_archive(&symlink, &staging, &SandboxLimits::default()).is_err());

        let mut bad_checksum = valid_tar_archive();
        bad_checksum[0] ^= 1;
        assert!(
            extract_artifact_archive(&bad_checksum, &staging, &SandboxLimits::default()).is_err()
        );

        let mut truncated = valid_tar_archive();
        truncated.truncate(truncated.len() - 512);
        assert!(extract_artifact_archive(&truncated, &staging, &SandboxLimits::default()).is_err());
        let _ = fs::remove_dir_all(staging);
    }

    #[test]
    fn tar_archive_enforces_artifact_bytes_and_entry_count() {
        let staging = archive_staging_dir("limited-tar");
        let mut oversized = Vec::new();
        tar_entry(&mut oversized, "output/large.bin", b'0', &vec![0_u8; 1_025]);
        tar_entry(&mut oversized, "capture/", b'5', b"");
        tar_entry(&mut oversized, "capture/stdout", b'0', b"");
        oversized.resize(oversized.len() + 1_024, 0);
        let artifact_limits = SandboxLimits {
            max_artifact_bytes: 1_024,
            ..SandboxLimits::default()
        };
        assert!(extract_artifact_archive(&oversized, &staging, &artifact_limits).is_err());

        let entry_limits = SandboxLimits {
            max_artifact_entries: 2,
            ..SandboxLimits::default()
        };
        assert!(extract_artifact_archive(&valid_tar_archive(), &staging, &entry_limits).is_err());
        let _ = fs::remove_dir_all(staging);
    }

    #[test]
    fn artifact_paths_reject_hostile_names_depth_and_case_collisions() {
        for path in ["CON.txt", "nul", "COM1.log", "bad?.txt", "trailing."] {
            assert!(parse_artifact_path(path).is_err(), "accepted {path:?}");
        }

        let staging = archive_staging_dir("portable-tar");
        let mut depth = Vec::new();
        let deep_path = format!("output/{}/leaf", "a/".repeat(MAX_ARTIFACT_DEPTH + 1));
        tar_entry(&mut depth, &deep_path, b'0', b"x");
        tar_entry(&mut depth, "capture/stdout", b'0', b"");
        depth.resize(depth.len() + 1_024, 0);
        assert!(extract_artifact_archive(&depth, &staging, &SandboxLimits::default()).is_err());

        let mut case_collision = Vec::new();
        tar_entry(&mut case_collision, "output/A.txt", b'0', b"a");
        tar_entry(&mut case_collision, "output/a.txt", b'0', b"b");
        tar_entry(&mut case_collision, "capture/stdout", b'0', b"");
        case_collision.resize(case_collision.len() + 1_024, 0);
        assert!(
            extract_artifact_archive(&case_collision, &staging, &SandboxLimits::default()).is_err()
        );
        let _ = fs::remove_dir_all(staging);
    }

    #[test]
    fn docker_endpoints_must_be_local_because_mount_paths_are_local() {
        for endpoint in [
            "unix:///var/run/docker.sock",
            "npipe:////./pipe/docker_engine",
            "npipe:////localhost/pipe/docker_engine",
            "tcp://localhost:2376",
            "tcp://127.0.0.1:2375",
            "tcp://[::1]:2376",
        ] {
            assert!(
                is_local_docker_endpoint(endpoint),
                "local endpoint rejected: {endpoint}"
            );
        }

        for endpoint in [
            "ssh://user@docker.example.org",
            "tcp://docker.example.org:2376",
            "tcp://192.0.2.10:2376",
            "tcp://127.0.0.1:not-a-port",
            "tcp://127.0.0.1:0",
            "tcp://127.0.0.1:65536",
            "tcp://[::1]:65536",
            "tcp://localhost:2376/remote-path",
            "https://localhost:2376",
        ] {
            assert!(
                !is_local_docker_endpoint(endpoint),
                "remote or malformed endpoint accepted: {endpoint}"
            );
        }

        let pinned = docker_cli_arguments(
            Some("unix:///var/run/docker.sock"),
            &[OsString::from("info")],
        );
        assert_eq!(
            pinned,
            vec![
                OsString::from("--host"),
                OsString::from("unix:///var/run/docker.sock"),
                OsString::from("info"),
            ]
        );
        assert_eq!(
            docker_cli_arguments(None, &[OsString::from("context"), OsString::from("show")]),
            vec![OsString::from("context"), OsString::from("show")]
        );
    }

    #[test]
    fn docker_context_inspection_must_name_exactly_one_local_endpoint() {
        let valid = br#"[{"Endpoints":{"docker":{"Host":"unix:///var/run/docker.sock"}}}]"#;
        assert_eq!(
            docker_context_endpoint(valid).expect("one local Docker endpoint"),
            "unix:///var/run/docker.sock"
        );

        for document in [
            br#"[]"#.as_slice(),
            br#"[{"Endpoints":{"docker":{"Host":"unix:///one.sock"}}},{"Endpoints":{"docker":{"Host":"unix:///two.sock"}}}]"#,
            br#"[{}]"#,
            br#"[{"Endpoints":{"docker":{"Host":"ssh://docker.example.org"}}}]"#,
        ] {
            assert!(
                docker_context_endpoint(document).is_err(),
                "malformed, ambiguous, or remote context was accepted"
            );
        }
    }

    #[test]
    fn a_docker_cli_capture_error_fails_closed_when_cleanup_is_unverified() {
        let cleanup_attempted = Cell::new(false);
        let error = finish_run_output(
            Err("Docker CLI could not be observed".into()),
            Some("0123456789ab".into()),
            |container_id| {
                assert_eq!(container_id, Some("0123456789ab"));
                cleanup_attempted.set(true);
                false
            },
        )
        .expect_err("an uncertain cleanup must override the run error");
        assert!(
            cleanup_attempted.get(),
            "cleanup must run after CLI failure"
        );
        assert_eq!(
            error,
            OciSandboxError::CleanupUnverified {
                container_id: Some("0123456789ab".into()),
            }
        );
    }

    #[test]
    fn a_docker_cli_capture_error_is_reported_after_cleanup_is_verified() {
        let cleanup_attempted = Cell::new(false);
        let error = finish_run_output(
            Err("Docker CLI could not be observed".into()),
            None,
            |container_id| {
                assert_eq!(container_id, None);
                cleanup_attempted.set(true);
                true
            },
        )
        .expect_err("the original run error is still reportable after cleanup");
        assert!(
            cleanup_attempted.get(),
            "cleanup must run after CLI failure"
        );
        assert_eq!(
            error,
            OciSandboxError::EngineUnavailable {
                reason: "Docker CLI could not be observed".into(),
            }
        );
    }

    #[test]
    fn mutable_or_ambiguous_request_values_are_refused() {
        let root = std::env::temp_dir().join(format!("aurora-oci-invalid-{}", std::process::id()));
        let input = root.join("input");
        let output = root.join("output");
        fs::create_dir_all(&input).expect("input directory");

        let mut candidate = request(input.clone(), output.clone());
        candidate.image = "registry.example/research/tool:latest".into();
        assert!(matches!(
            validate_request(&candidate),
            Err(OciSandboxError::InvalidRequest { field: "image", .. })
        ));

        candidate = request(input.clone(), output.clone());
        candidate.command = "bin/sh".into();
        assert!(matches!(
            validate_request(&candidate),
            Err(OciSandboxError::InvalidRequest {
                field: "command",
                ..
            })
        ));

        candidate = request(input.clone(), input.join("output"));
        fs::create_dir_all(&candidate.output_dir).expect("nested output directory");
        assert!(matches!(
            validate_request(&candidate),
            Err(OciSandboxError::InvalidRequest {
                field: "output_dir",
                ..
            })
        ));

        candidate = request(input.clone(), output.clone());
        candidate.limits.container_uid = 0;
        assert!(matches!(
            validate_request(&candidate),
            Err(OciSandboxError::InvalidRequest {
                field: "container identity",
                ..
            })
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn output_directory_must_be_new_and_artifact_entries_are_hashed_in_stable_order() {
        let root =
            std::env::temp_dir().join(format!("aurora-oci-artifacts-{}", std::process::id()));
        let input = root.join("input");
        let staging = root.join("staging");
        let output = root.join("output");
        fs::create_dir_all(&input).expect("input directory");
        fs::create_dir_all(staging.join("nested")).expect("staging tree");

        let mut candidate = request(input.clone(), output.clone());
        assert!(validate_request(&candidate).is_ok());
        fs::create_dir(&output).expect("pre-existing output directory");
        assert!(matches!(
            validate_request(&candidate),
            Err(OciSandboxError::InvalidRequest {
                field: "output_dir",
                ..
            })
        ));
        fs::remove_dir(&output).expect("remove pre-existing output directory");

        candidate.limits.max_artifact_bytes = 1_024;
        fs::write(staging.join("z.txt"), b"z").expect("top-level artifact");
        fs::write(staging.join("nested").join("a.txt"), b"a").expect("nested artifact");
        let artifacts = validate_artifact_tree(&staging, &candidate.limits)
            .expect("bounded output tree is valid");
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].path, "nested/a.txt");
        assert_eq!(artifacts[0].bytes, 1);
        assert_eq!(
            artifacts[0].sha256,
            bioprism_ids::ContentHash::of_bytes(b"a").to_string()
        );
        assert_eq!(artifacts[1].path, "z.txt");

        candidate.limits.max_artifact_bytes = 1_024;
        candidate.limits.max_artifact_entries = 1;
        assert!(matches!(
            validate_artifact_tree(&staging, &candidate.limits),
            Err(OciSandboxError::ArtifactCollectionFailed { .. })
        ));

        candidate.limits.max_artifact_entries = 1_024;
        candidate.limits.max_artifact_bytes = 1_024;
        fs::write(staging.join("oversized.bin"), vec![0_u8; 1_025]).expect("oversized artifact");
        assert!(matches!(
            validate_artifact_tree(&staging, &candidate.limits),
            Err(OciSandboxError::ArtifactCollectionFailed { .. })
        ));
        fs::remove_file(staging.join("oversized.bin")).expect("remove oversized artifact");

        fs::create_dir(&output).expect("concurrent output path");
        assert!(matches!(
            publish_artifacts(&staging, &output, &candidate.limits),
            Err(OciSandboxError::ArtifactCollectionFailed { .. })
        ));
        assert!(fs::read_dir(&output)
            .expect("unmodified concurrent output directory")
            .next()
            .is_none());
        fs::remove_dir(&output).expect("remove concurrent output directory");

        let artifacts = publish_artifacts(&staging, &output, &candidate.limits)
            .expect("validated output publishes atomically");
        assert_eq!(artifacts.len(), 2);
        assert_eq!(fs::read(output.join("nested").join("a.txt")).unwrap(), b"a");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn image_digest_and_all_resource_limits_are_bounded() {
        assert!(valid_digest_pinned_image(&format!(
            "repo/image@sha256:{}",
            "0".repeat(64)
        )));
        assert!(!valid_digest_pinned_image(&format!(
            "repo/image@sha256:{}",
            "A".repeat(64)
        )));
        let mut limits = SandboxLimits::default();
        validate_limits(&limits).expect("defaults are bounded");
        limits.max_processes = 1;
        assert!(matches!(
            validate_limits(&limits),
            Err(OciSandboxError::InvalidRequest {
                field: "max_processes",
                ..
            })
        ));
    }

    #[test]
    fn image_volume_metadata_is_validated_fail_closed() {
        assert!(ensure_no_image_volumes(b"null").is_ok());
        assert!(ensure_no_image_volumes(b"{}").is_ok());
        assert!(matches!(
            ensure_no_image_volumes(br#"{"/var/lib/data":{}}"#),
            Err(OciSandboxError::ImageRejected { .. })
        ));
        assert!(matches!(
            ensure_no_image_volumes(b"not-json"),
            Err(OciSandboxError::EngineUnavailable { .. })
        ));
        assert_eq!(
            image_volume_inspection_arguments(
                "registry.example/image@sha256:abc",
                LinuxPlatform::Arm64
            ),
            [
                OsString::from("image"),
                OsString::from("inspect"),
                OsString::from("--platform=linux/arm64"),
                OsString::from("--format"),
                OsString::from("{{json .Config.Volumes}}"),
                OsString::from("registry.example/image@sha256:abc"),
            ]
        );
        assert_eq!(
            container_remove_arguments("0123456789ab"),
            [
                OsString::from("rm"),
                OsString::from("--force"),
                OsString::from("--volumes"),
                OsString::from("0123456789ab"),
            ]
        );
    }
}
