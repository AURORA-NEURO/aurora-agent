//! Route recorded process effects through the digest-pinned Docker command boundary.
//!
//! This adapter connects one narrow `ProcessSpawn` effect to [`crate::DockerSandbox`]. It is
//! intended to be installed as the source of a `RecordingHost`, so the existing effect policy is
//! checked before Docker runs and successful outcomes are recorded for replay. Each process gets
//! a private, per-source quarantine directory. The path is exposed to the host operator through
//! [`DockerProcessSource::quarantined_outputs`] and only its opaque identifier is returned to the
//! workload and tape.
//!
//! This is not an `ExecutorProvider`; it does not scan, review, or release quarantined artifacts.

use crate::effect::{EffectOutcome, EffectRequest};
use crate::error::RuntimeError;
use crate::host::EffectSource;
use crate::oci_sandbox::{
    DockerArtifact, DockerSandbox, LinuxPlatform, OciSandboxError, SandboxLimits, SandboxRequest,
    SandboxRunResult,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SCOPE_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_TAPED_STREAM_BYTES: usize = 1024 * 1024;

/// Fixed inputs and resource bounds for all process effects from one source instance.
#[derive(Debug, Clone)]
pub struct DockerProcessConfig {
    /// Existing image reference pinned to a lowercase SHA-256 digest.
    pub image: String,
    pub platform: LinuxPlatform,
    /// Canonicalized once and mounted read-only for each process.
    pub input_dir: PathBuf,
    /// Host-owned quarantine root. A private run directory is created beneath it.
    pub quarantine_root: PathBuf,
    pub limits: SandboxLimits,
}

/// A seam for deterministic integration checks and alternate trusted Docker CLI locations.
pub trait SandboxCommandRunner {
    fn run(&self, request: &SandboxRequest) -> Result<SandboxRunResult, OciSandboxError>;
}

impl SandboxCommandRunner for DockerSandbox {
    fn run(&self, request: &SandboxRequest) -> Result<SandboxRunResult, OciSandboxError> {
        DockerSandbox::run(self, request)
    }
}

/// Operator-side locator for output that has not passed scanning or review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedProcessOutput {
    /// Opaque identifier included in the recorded effect result.
    pub id: String,
    /// Private directory containing the runner's published `output` directory, when successful.
    pub directory: PathBuf,
    /// Artifact metadata returned by the runner. Files remain in quarantine.
    pub artifacts: Vec<DockerArtifact>,
    /// Present when validation or execution failed; the private directory is retained for review.
    pub failure: Option<String>,
}

/// An `EffectSource` that sends process requests to a bounded Docker sandbox.
///
/// Use one instance per trial/run. Other effect kinds are delegated to `fallback`; the standard
/// configuration uses [`crate::InProcessWorld`] for those deterministic effects.
pub struct DockerProcessSource<R, S> {
    config: DockerProcessConfig,
    runner: R,
    fallback: S,
    input_dir: PathBuf,
    run_directory: PathBuf,
    scope_id: String,
    next_step: u64,
    quarantined: Vec<QuarantinedProcessOutput>,
}

impl<R, S> DockerProcessSource<R, S>
where
    R: SandboxCommandRunner,
    S: EffectSource,
{
    /// Validate and prepare a private run scope beneath `config.quarantine_root`.
    pub fn new(
        mut config: DockerProcessConfig,
        runner: R,
        fallback: S,
    ) -> Result<Self, RuntimeError> {
        let input_dir = fs::canonicalize(&config.input_dir).map_err(|error| {
            source_failure(
                "docker_process_source_init",
                format!("cannot resolve input directory: {error}"),
            )
        })?;
        if !input_dir.is_dir() {
            return Err(source_failure(
                "docker_process_source_init",
                "input_dir is not a directory",
            ));
        }

        let quarantine_candidate =
            prospective_canonical_path(&config.quarantine_root).map_err(|error| {
                source_failure(
                    "docker_process_source_init",
                    format!("cannot resolve quarantine root path: {error}"),
                )
            })?;
        if paths_overlap(&input_dir, &quarantine_candidate) {
            return Err(source_failure(
                "docker_process_source_init",
                "quarantine_root must be separate from the read-only input tree",
            ));
        }
        fs::create_dir_all(&config.quarantine_root).map_err(|error| {
            source_failure(
                "docker_process_source_init",
                format!("cannot create quarantine root: {error}"),
            )
        })?;
        let quarantine_root = fs::canonicalize(&config.quarantine_root).map_err(|error| {
            source_failure(
                "docker_process_source_init",
                format!("cannot resolve quarantine root: {error}"),
            )
        })?;
        if !quarantine_root.is_dir() {
            return Err(source_failure(
                "docker_process_source_init",
                "quarantine_root is not a directory",
            ));
        }
        if paths_overlap(&input_dir, &quarantine_root) {
            return Err(source_failure(
                "docker_process_source_init",
                "quarantine_root must be separate from the read-only input tree",
            ));
        }
        config.input_dir = input_dir.clone();
        config.quarantine_root = quarantine_root.clone();
        if config.limits.max_output_bytes > MAX_TAPED_STREAM_BYTES {
            return Err(source_failure(
                "docker_process_source_init",
                format!(
                    "max_output_bytes exceeds the {MAX_TAPED_STREAM_BYTES} byte per-stream tape bound"
                ),
            ));
        }
        validate_config(&config)?;

        let (scope_id, run_directory) =
            create_run_directory(&quarantine_root).map_err(|error| {
                source_failure(
                    "docker_process_source_init",
                    format!("cannot create private run quarantine: {error}"),
                )
            })?;

        Ok(DockerProcessSource {
            config,
            runner,
            fallback,
            input_dir,
            run_directory,
            scope_id,
            next_step: 0,
            quarantined: Vec::new(),
        })
    }

    /// Host-side records of outputs and failures retained in quarantine.
    pub fn quarantined_outputs(&self) -> &[QuarantinedProcessOutput] {
        &self.quarantined
    }

    /// Directory for this source instance. Access is intended for the trusted host operator.
    pub fn run_directory(&self) -> &Path {
        &self.run_directory
    }

    fn perform_process(&mut self, request: &EffectRequest) -> Result<EffectOutcome, RuntimeError> {
        let step = self.next_step;
        self.next_step = self
            .next_step
            .checked_add(1)
            .ok_or_else(|| source_failure(request, "process effect step counter overflowed"))?;
        let id = format!("{}-{step:08}", self.scope_id);
        let step_directory = self.run_directory.join(format!("step-{step:08}"));
        create_private_directory(&step_directory).map_err(|error| {
            source_failure(
                request,
                format!("cannot create private step quarantine: {error}"),
            )
        })?;
        let output_dir = step_directory.join("output");

        let EffectRequest::ProcessSpawn { program, args } = request else {
            return Err(source_failure(request, "internal process routing mismatch"));
        };
        let sandbox_request = SandboxRequest {
            image: self.config.image.clone(),
            platform: self.config.platform,
            command: program.clone(),
            arguments: args.clone(),
            input_dir: self.input_dir.clone(),
            output_dir: output_dir.clone(),
            limits: self.config.limits.clone(),
        };
        if let Err(error) = sandbox_request.validate() {
            let reason = error.to_string();
            self.quarantined.push(QuarantinedProcessOutput {
                id,
                directory: step_directory,
                artifacts: Vec::new(),
                failure: Some(reason.clone()),
            });
            return Err(source_failure(request, reason));
        }

        let result = match self.runner.run(&sandbox_request) {
            Ok(result) => result,
            Err(error) => {
                let reason = error.to_string();
                self.quarantined.push(QuarantinedProcessOutput {
                    id,
                    directory: step_directory,
                    artifacts: Vec::new(),
                    failure: Some(reason.clone()),
                });
                return Err(source_failure(request, reason));
            }
        };

        if !result.cleanup_verified {
            let reason = "runner returned without verified container cleanup".to_owned();
            self.quarantined.push(QuarantinedProcessOutput {
                id,
                directory: step_directory,
                artifacts: Vec::new(),
                failure: Some(reason.clone()),
            });
            return Err(source_failure(request, reason));
        }
        if (result.timed_out || result.output_limit_exceeded) && !result.artifacts.is_empty() {
            let reason = "interrupted runner result unexpectedly contains artifacts".to_owned();
            self.quarantined.push(QuarantinedProcessOutput {
                id,
                directory: step_directory,
                artifacts: Vec::new(),
                failure: Some(reason.clone()),
            });
            return Err(source_failure(request, reason));
        }

        let artifacts = if result.timed_out || result.output_limit_exceeded {
            match ensure_no_published_output(&output_dir) {
                Ok(()) => Vec::new(),
                Err(reason) => {
                    self.quarantined.push(QuarantinedProcessOutput {
                        id,
                        directory: step_directory,
                        artifacts: Vec::new(),
                        failure: Some(reason.clone()),
                    });
                    return Err(source_failure(request, reason));
                }
            }
        } else {
            match verify_published_output(&output_dir, &result.artifacts, &self.config.limits) {
                Ok(()) => result.artifacts.clone(),
                Err(reason) => {
                    self.quarantined.push(QuarantinedProcessOutput {
                        id,
                        directory: step_directory,
                        artifacts: Vec::new(),
                        failure: Some(reason.clone()),
                    });
                    return Err(source_failure(request, reason));
                }
            }
        };

        self.quarantined.push(QuarantinedProcessOutput {
            id: id.clone(),
            directory: step_directory,
            artifacts: artifacts.clone(),
            failure: None,
        });

        // Text is lossily decoded for useful agent output. Exact artifact bytes remain in the
        // quarantined files and are represented by their byte count and SHA-256 digest.
        Ok(EffectOutcome::new(json!({
            "exit_code": result.exit_code,
            "timed_out": result.timed_out,
            "output_limit_exceeded": result.output_limit_exceeded,
            "cleanup_verified": result.cleanup_verified,
            "stdout": String::from_utf8_lossy(&result.stdout),
            "stderr": String::from_utf8_lossy(&result.stderr),
            "quarantine_id": id,
            "artifacts": artifacts.iter().map(|artifact| json!({
                "path": artifact.path,
                "bytes": artifact.bytes,
                "sha256": artifact.sha256,
            })).collect::<Vec<_>>(),
        })))
    }
}

impl<R, S> EffectSource for DockerProcessSource<R, S>
where
    R: SandboxCommandRunner,
    S: EffectSource,
{
    fn perform(&mut self, request: &EffectRequest) -> Result<EffectOutcome, RuntimeError> {
        if matches!(request, EffectRequest::ProcessSpawn { .. }) {
            self.perform_process(request)
        } else {
            self.fallback.perform(request)
        }
    }
}

fn verify_published_output(
    output_dir: &Path,
    reported: &[DockerArtifact],
    limits: &SandboxLimits,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(output_dir)
        .map_err(|error| format!("runner did not publish its output directory: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("runner output is not a real directory".into());
    }
    let canonical_output = fs::canonicalize(output_dir)
        .map_err(|error| format!("cannot resolve runner output directory: {error}"))?;
    if canonical_output != output_dir {
        return Err("runner output directory resolved outside its quarantine path".into());
    }

    let mut observed = BTreeMap::new();
    let mut entries = 0u32;
    let mut total_bytes = 0u64;
    let mut pending = vec![(output_dir.to_path_buf(), String::new())];
    while let Some((directory, relative_parent)) = pending.pop() {
        let rows = fs::read_dir(&directory)
            .map_err(|error| format!("cannot enumerate quarantined output: {error}"))?;
        for row in rows {
            let row = row.map_err(|error| format!("cannot read quarantined entry: {error}"))?;
            let name = row
                .file_name()
                .into_string()
                .map_err(|_| "quarantine output contains a non-Unicode filename".to_owned())?;
            let relative = if relative_parent.is_empty() {
                name
            } else {
                format!("{relative_parent}/{name}")
            };
            let metadata = fs::symlink_metadata(row.path())
                .map_err(|error| format!("cannot inspect quarantined entry: {error}"))?;
            entries = entries
                .checked_add(1)
                .ok_or_else(|| "quarantine entry count overflowed".to_owned())?;
            if entries > limits.max_artifact_entries {
                return Err("quarantine output exceeds the admitted entry count".into());
            }
            if metadata.file_type().is_symlink() {
                return Err(format!("quarantine output contains a symlink: {relative}"));
            }
            if metadata.is_dir() {
                pending.push((row.path(), relative));
                continue;
            }
            if !metadata.is_file() {
                return Err(format!(
                    "quarantine output contains a special file: {relative}"
                ));
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| "quarantine byte count overflowed".to_owned())?;
            if total_bytes > limits.max_artifact_bytes {
                return Err("quarantine output exceeds the admitted byte count".into());
            }
            let bytes = fs::read(row.path())
                .map_err(|error| format!("cannot read quarantined artifact: {error}"))?;
            let digest = bioprism_ids::ContentHash::of_bytes(&bytes).to_string();
            observed.insert(relative.clone(), (bytes.len() as u64, digest));
        }
    }

    let expected: BTreeMap<_, _> = reported
        .iter()
        .map(|artifact| {
            (
                artifact.path.clone(),
                (artifact.bytes, artifact.sha256.clone()),
            )
        })
        .collect();
    if expected.len() != reported.len() || expected != observed {
        return Err("runner artifact metadata does not match quarantined files".into());
    }
    Ok(())
}

fn ensure_no_published_output(output_dir: &Path) -> Result<(), String> {
    match fs::symlink_metadata(output_dir) {
        Ok(_) => Err("interrupted runner result unexpectedly published an output directory".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot verify interrupted runner output path: {error}"
        )),
    }
}

fn validate_config(config: &DockerProcessConfig) -> Result<(), RuntimeError> {
    for _ in 0..128 {
        let sequence = SCOPE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let output_dir = config.quarantine_root.join(format!(
            ".config-check-{:x}-{sequence:016x}",
            std::process::id()
        ));
        match fs::symlink_metadata(&output_dir) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(source_failure(
                    "docker_process_source_init",
                    format!("cannot validate quarantine output path: {error}"),
                ));
            }
        }
        let request = SandboxRequest {
            image: config.image.clone(),
            platform: config.platform,
            command: "/bin/true".into(),
            arguments: Vec::new(),
            input_dir: config.input_dir.clone(),
            output_dir,
            limits: config.limits.clone(),
        };
        return request
            .validate()
            .map_err(|error| source_failure("docker_process_source_init", error.to_string()));
    }
    Err(source_failure(
        "docker_process_source_init",
        "could not allocate a temporary path for validating sandbox settings",
    ))
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    let left = comparable_path(left);
    let right = comparable_path(right);
    left.starts_with(&right) || right.starts_with(&left)
}

fn prospective_canonical_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    let mut ancestor = normalized.clone();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(&ancestor)?;
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = ancestor.file_name().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "no existing parent found for quarantine root",
                    )
                })?;
                missing.push(name.to_os_string());
                if !ancestor.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "no existing parent found for quarantine root",
                    ));
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn comparable_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn create_run_directory(root: &Path) -> io::Result<(String, PathBuf)> {
    for _ in 0..128 {
        let sequence = SCOPE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("{:x}-{sequence:016x}", std::process::id());
        let path = root.join(format!("run-{id}"));
        match create_private_directory(&path) {
            Ok(()) => return Ok((id, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique run quarantine directory",
    ))
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

fn source_failure(request: impl ToString, reason: impl ToString) -> RuntimeError {
    RuntimeError::SourceFailure {
        request: request.to_string(),
        reason: reason.to_string(),
    }
}
