//! Deterministic process-side simulation for an admitted sandbox declaration.
//!
//! This module is intentionally one boundary later than [`crate::sandbox_admission`].  Admission
//! answers whether a declared profile is eligible to be handed to an external runtime.  Runtime
//! simulation answers whether a bounded sequence of requested effects would be accepted by that
//! profile, charging resources and preserving the first refusal in a replayable trace.  It never
//! starts a process, resolves a host path, opens a socket, reads a secret, applies a cgroup, or
//! claims that a kernel/container boundary exists.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::sandbox_admission::{
    SandboxCapability, SandboxCapabilityKind, SandboxDecision, SandboxError, SandboxIssue,
    SandboxIssueSeverity, SandboxManifest, SandboxNetworkMode, SandboxResourceLimits,
};

pub const SANDBOX_RUNTIME_MANIFEST_SCHEMA: &str = "bioprism-sandbox-runtime/0.1";
pub const SANDBOX_RUNTIME_AUDIT_SCHEMA: &str = "bioprism-sandbox-runtime-audit/0.1";
pub const MAX_RUNTIME_REQUESTS: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRuntimeManifest {
    pub schema: String,
    pub admission: SandboxManifest,
    pub profile: String,
    #[serde(default)]
    pub requests: Vec<SandboxRuntimeRequest>,
    #[serde(default)]
    pub policies: SandboxRuntimePolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRuntimeRequest {
    pub id: String,
    pub kind: SandboxCapabilityKind,
    pub target: String,
    pub cpu_millis: u64,
    pub memory_mb: u64,
    pub wall_time_seconds: u64,
    pub processes: u32,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRuntimePolicies {
    #[serde(default = "default_true")]
    pub stop_on_refusal: bool,
    #[serde(default = "default_true")]
    pub require_admission: bool,
    #[serde(default = "default_max_requests")]
    pub max_requests: usize,
}

impl Default for SandboxRuntimePolicies {
    fn default() -> Self {
        Self {
            stop_on_refusal: true,
            require_admission: true,
            max_requests: MAX_RUNTIME_REQUESTS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxRuntimeDecision {
    Simulated,
    Refused,
    NotRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxRuntimeUsage {
    pub cpu_millis: u64,
    pub memory_mb_peak: u64,
    pub wall_time_seconds: u64,
    pub processes_peak: u32,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRuntimeStepAudit {
    pub request_id: String,
    pub kind: SandboxCapabilityKind,
    pub target: String,
    pub capability_id: Option<String>,
    pub capability_valid: bool,
    pub target_valid: bool,
    pub resource_valid: bool,
    pub decision: SandboxRuntimeDecision,
    pub charged: bool,
    pub usage_after: SandboxRuntimeUsage,
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRuntimeIssue {
    pub code: String,
    pub severity: SandboxIssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRuntimeAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub admission_digest: String,
    pub trace_digest: String,
    pub valid: bool,
    pub profile_id: String,
    pub admission_valid: bool,
    pub simulation_started: bool,
    pub completed: bool,
    pub stopped_on_refusal: bool,
    pub request_count: usize,
    pub simulated_count: usize,
    pub refused_count: usize,
    pub not_run_count: usize,
    pub usage: SandboxRuntimeUsage,
    pub steps: Vec<SandboxRuntimeStepAudit>,
    pub admission_issues: Vec<SandboxIssue>,
    pub issues: Vec<SandboxRuntimeIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SandboxRuntimeError {
    #[error("cannot audit sandbox admission: {0}")]
    Admission(#[from] SandboxError),
    #[error("cannot canonicalize sandbox runtime manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize sandbox runtime manifest: {0}")]
    Serialization(String),
}

impl SandboxRuntimeManifest {
    pub fn digest(&self) -> Result<ContentHash, SandboxRuntimeError> {
        let value = serde_json::to_value(self)
            .map_err(|error| SandboxRuntimeError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<SandboxRuntimeAudit, SandboxRuntimeError> {
        let admission = self.admission.audit()?;
        let admission_digest = admission.digest.clone();
        let mut issues = Vec::new();
        let mut steps = Vec::new();
        let mut usage = SandboxRuntimeUsage::default();
        let mut simulation_started = false;
        let mut stopped_on_refusal = false;

        if self.schema != SANDBOX_RUNTIME_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!(
                    "expected {SANDBOX_RUNTIME_MANIFEST_SCHEMA}, got {}",
                    self.schema
                ),
                "regenerate the runtime request with the published schema",
            );
        }
        if !valid_text(&self.profile) {
            blocking(
                &mut issues,
                "profile_missing",
                "profile",
                "a non-empty execution profile is required",
                "select one declared admission profile",
            );
        }
        if self.policies.max_requests == 0 || self.policies.max_requests > MAX_RUNTIME_REQUESTS {
            blocking(
                &mut issues,
                "request_bound_invalid",
                "policies.max_requests",
                format!("max_requests must be between 1 and {MAX_RUNTIME_REQUESTS}"),
                "use a bounded request window within the published maximum",
            );
        }
        let request_limit = self.policies.max_requests.clamp(1, MAX_RUNTIME_REQUESTS);
        if self.requests.len() > request_limit {
            blocking(
                &mut issues,
                "request_bound_exceeded",
                "requests",
                format!(
                    "{} requests exceed the configured maximum of {request_limit}",
                    self.requests.len()
                ),
                "split the simulation into bounded traces",
            );
        }
        if !admission.valid {
            blocking(
                &mut issues,
                "admission_invalid",
                &self.profile,
                "the admission declaration contains blocking findings",
                "resolve admission findings before requesting runtime simulation",
            );
        }
        if !self.policies.require_admission && !admission.valid {
            warning(
                &mut issues,
                "admission_bypass_requested",
                &self.profile,
                "simulation was explicitly requested without a valid admission result",
                "require a valid admission result for any readiness or release decision",
            );
        }

        let profiles = self
            .admission
            .profiles
            .iter()
            .map(|profile| (profile.id.as_str(), profile))
            .collect::<BTreeMap<_, _>>();
        let capabilities = self
            .admission
            .capabilities
            .iter()
            .map(|capability| (capability.id.as_str(), capability))
            .collect::<BTreeMap<_, _>>();
        let profile = profiles.get(self.profile.as_str()).copied();
        if profile.is_none() {
            blocking(
                &mut issues,
                "profile_unknown",
                &self.profile,
                "the requested profile is not declared by the admission manifest",
                "bind the trace to one declared execution profile",
            );
        }

        let can_simulate = profile.is_some()
            && (!self.policies.require_admission || admission.valid)
            && !has_blocking(&issues, "profile_missing")
            && !has_blocking(&issues, "schema_mismatch");
        if can_simulate {
            simulation_started = true;
            let profile = profile.expect("checked above");
            let profile_capabilities = profile
                .capabilities
                .iter()
                .filter_map(|id| capabilities.get(id.as_str()).copied())
                .collect::<Vec<_>>();
            let requests = self.requests.iter().take(request_limit);
            let mut seen_ids = BTreeMap::<String, ()>::new();
            for request in requests {
                if !seen_ids.insert(request.id.clone(), ()).is_none() {
                    blocking(
                        &mut issues,
                        "request_duplicate",
                        &request.id,
                        "request identifiers must be unique within a trace",
                        "assign one stable identifier to each requested effect",
                    );
                }
                let capability = matching_capability(&profile_capabilities, request);
                let capability_valid = capability
                    .map(|value| {
                        value.profile == profile.id
                            && value.decision == SandboxDecision::Allow
                            && valid_capability_target(value)
                            && profile_target_allowed(profile, request)
                    })
                    .unwrap_or(false);
                let target_valid = valid_request_target(request);
                let resource_valid = request_resources_valid(request, &profile.resources, &usage);
                let mut refusal = None;
                if !valid_text(&request.id) {
                    refusal = Some("request_identity_missing".to_string());
                    blocking(
                        &mut issues,
                        "request_identity_missing",
                        &request.id,
                        "request id must be non-empty and bounded",
                        "assign a stable bounded request identifier",
                    );
                } else if !target_valid {
                    refusal = Some("request_target_invalid".to_string());
                    blocking(
                        &mut issues,
                        "request_target_invalid",
                        &request.id,
                        "request target is empty, broad, traversing, or names a host namespace",
                        "name one exact private path, destination, or capability target",
                    );
                } else if !capability_valid {
                    refusal = Some("capability_not_approved".to_string());
                    blocking(
                        &mut issues,
                        "capability_not_approved",
                        &request.id,
                        "no exact approved capability in the selected profile matches this request",
                        "declare and evidence one exact allowlisted capability before simulation",
                    );
                } else if !resource_valid {
                    refusal = Some("resource_budget_exceeded".to_string());
                    blocking(
                        &mut issues,
                        "resource_budget_exceeded",
                        &request.id,
                        "the request is zero, exceeds a profile ceiling, or exceeds cumulative budget",
                        "reduce the charge or request a separately reviewed bounded profile",
                    );
                }

                if let Some(refusal_code) = refusal {
                    steps.push(step(
                        request,
                        capability.map(|value| value.id.clone()),
                        capability_valid,
                        target_valid,
                        resource_valid,
                        SandboxRuntimeDecision::Refused,
                        false,
                        usage.clone(),
                        Some(refusal_code),
                    ));
                    if self.policies.stop_on_refusal {
                        stopped_on_refusal = true;
                        break;
                    }
                    continue;
                }

                charge(&mut usage, request);
                steps.push(step(
                    request,
                    capability.map(|value| value.id.clone()),
                    true,
                    true,
                    true,
                    SandboxRuntimeDecision::Simulated,
                    true,
                    usage.clone(),
                    None,
                ));
            }
            if stopped_on_refusal {
                let ran = steps.len();
                if ran < self.requests.len().min(request_limit) {
                    for request in self.requests.iter().skip(ran).take(request_limit - ran) {
                        steps.push(step(
                            request,
                            None,
                            false,
                            false,
                            false,
                            SandboxRuntimeDecision::NotRun,
                            false,
                            usage.clone(),
                            Some("stopped_on_refusal".into()),
                        ));
                    }
                }
            }
        } else {
            for request in self.requests.iter().take(request_limit) {
                steps.push(step(
                    request,
                    None,
                    false,
                    false,
                    false,
                    SandboxRuntimeDecision::NotRun,
                    false,
                    usage.clone(),
                    Some("admission_not_ready".into()),
                ));
            }
        }

        let request_count = self.requests.len();
        let simulated_count = steps
            .iter()
            .filter(|step| step.decision == SandboxRuntimeDecision::Simulated)
            .count();
        let refused_count = steps
            .iter()
            .filter(|step| step.decision == SandboxRuntimeDecision::Refused)
            .count();
        let not_run_count = steps
            .iter()
            .filter(|step| step.decision == SandboxRuntimeDecision::NotRun)
            .count();
        let completed = simulation_started
            && refused_count == 0
            && not_run_count == 0
            && simulated_count == request_count
            && request_count <= request_limit;
        let admission_valid = admission.valid;
        let valid = self.schema == SANDBOX_RUNTIME_MANIFEST_SCHEMA
            && admission_valid
            && completed
            && !issues
                .iter()
                .any(|issue| issue.severity == SandboxIssueSeverity::Blocking);
        issues.sort_by(|left, right| {
            (&left.code, &left.subject, &left.detail).cmp(&(
                &right.code,
                &right.subject,
                &right.detail,
            ))
        });
        let trace_value = serde_json::to_value(TraceDigestInput {
            steps: &steps,
            usage: &usage,
            profile: &self.profile,
            admission_digest: &admission_digest,
        })
        .map_err(|error| SandboxRuntimeError::Serialization(error.to_string()))?;
        let trace_digest = ContentHash::of_value(&trace_value)?.to_string();

        Ok(SandboxRuntimeAudit {
            schema: SANDBOX_RUNTIME_AUDIT_SCHEMA.into(),
            manifest_schema: self.schema.clone(),
            admission_digest,
            trace_digest,
            valid,
            profile_id: self.profile.clone(),
            admission_valid,
            simulation_started,
            completed,
            stopped_on_refusal,
            request_count,
            simulated_count,
            refused_count,
            not_run_count,
            usage,
            steps,
            admission_issues: admission.issues,
            issues,
            guarantees: vec![
                "admission is evaluated before a required simulation starts".into(),
                "every simulated step has one exact capability, target, and bounded resource charge".into(),
                "refusal is preserved in the trace and can stop all later requests deterministically".into(),
                "cumulative CPU, wall-time, and output budgets plus memory and process peaks are charged".into(),
            ],
            limitations: vec![
                "simulation does not start a process, execute code, resolve a host path, or open a socket".into(),
                "simulation does not enforce syscalls, cgroups, namespaces, credentials, secrets, or network policy".into(),
                "the trace is a decision artifact and is not evidence that an external runtime enforced the decision".into(),
            ],
        })
    }
}

#[derive(Serialize)]
struct TraceDigestInput<'a> {
    steps: &'a [SandboxRuntimeStepAudit],
    usage: &'a SandboxRuntimeUsage,
    profile: &'a str,
    admission_digest: &'a str,
}

fn matching_capability<'a>(
    capabilities: &'a [&SandboxCapability],
    request: &SandboxRuntimeRequest,
) -> Option<&'a SandboxCapability> {
    capabilities
        .iter()
        .copied()
        .find(|capability| capability.kind == request.kind && capability.target == request.target)
}

fn valid_capability_target(capability: &SandboxCapability) -> bool {
    valid_text(&capability.target)
        && capability.target != "*"
        && !capability.target.contains("..")
        && !capability.target.contains('\\')
        && match capability.kind {
            SandboxCapabilityKind::FilesystemRead | SandboxCapabilityKind::FilesystemWrite => {
                private_path(&capability.target)
            }
            SandboxCapabilityKind::NetworkEgress | SandboxCapabilityKind::NetworkIngress => {
                valid_network_target(&capability.target)
            }
            _ => true,
        }
}

fn valid_request_target(request: &SandboxRuntimeRequest) -> bool {
    if !valid_text(&request.target)
        || request.target == "*"
        || request.target.contains("..")
        || request.target.contains('\\')
    {
        return false;
    }
    match request.kind {
        SandboxCapabilityKind::FilesystemRead | SandboxCapabilityKind::FilesystemWrite => {
            private_path(&request.target)
        }
        SandboxCapabilityKind::NetworkEgress | SandboxCapabilityKind::NetworkIngress => {
            valid_network_target(&request.target)
        }
        _ => true,
    }
}

fn profile_target_allowed(
    profile: &crate::sandbox_admission::SandboxExecutionProfile,
    request: &SandboxRuntimeRequest,
) -> bool {
    match request.kind {
        SandboxCapabilityKind::NetworkEgress | SandboxCapabilityKind::NetworkIngress => {
            profile.network == SandboxNetworkMode::Allowlist
                && profile
                    .network_allowlist
                    .iter()
                    .any(|target| target == &request.target)
        }
        _ => true,
    }
}

fn private_path(value: &str) -> bool {
    value.starts_with('/')
        && value != "/"
        && !value.starts_with("/proc")
        && !value.starts_with("/sys")
        && !value.starts_with("/dev")
}

fn valid_network_target(value: &str) -> bool {
    valid_text(value)
        && value != "*"
        && value != "0.0.0.0/0"
        && value != "::/0"
        && !value.contains('*')
        && !value.contains("..")
}

fn request_resources_valid(
    request: &SandboxRuntimeRequest,
    limits: &SandboxResourceLimits,
    usage: &SandboxRuntimeUsage,
) -> bool {
    let (Some(cpu), Some(memory), Some(wall), Some(processes), Some(output)) = (
        limits.cpu_millis,
        limits.memory_mb,
        limits.wall_time_seconds,
        limits.processes,
        limits.output_bytes,
    ) else {
        return false;
    };
    request.cpu_millis > 0
        && request.memory_mb > 0
        && request.wall_time_seconds > 0
        && request.processes > 0
        && request.output_bytes > 0
        && request.memory_mb <= memory
        && request.processes <= processes
        && request.cpu_millis <= cpu.saturating_sub(usage.cpu_millis)
        && request.wall_time_seconds <= wall.saturating_sub(usage.wall_time_seconds)
        && request.output_bytes <= output.saturating_sub(usage.output_bytes)
}

fn charge(usage: &mut SandboxRuntimeUsage, request: &SandboxRuntimeRequest) {
    usage.cpu_millis = usage.cpu_millis.saturating_add(request.cpu_millis);
    usage.memory_mb_peak = usage.memory_mb_peak.max(request.memory_mb);
    usage.wall_time_seconds = usage
        .wall_time_seconds
        .saturating_add(request.wall_time_seconds);
    usage.processes_peak = usage.processes_peak.max(request.processes);
    usage.output_bytes = usage.output_bytes.saturating_add(request.output_bytes);
}

#[allow(clippy::too_many_arguments)]
fn step(
    request: &SandboxRuntimeRequest,
    capability_id: Option<String>,
    capability_valid: bool,
    target_valid: bool,
    resource_valid: bool,
    decision: SandboxRuntimeDecision,
    charged: bool,
    usage_after: SandboxRuntimeUsage,
    refusal: Option<String>,
) -> SandboxRuntimeStepAudit {
    SandboxRuntimeStepAudit {
        request_id: request.id.clone(),
        kind: request.kind,
        target: request.target.clone(),
        capability_id,
        capability_valid,
        target_valid,
        resource_valid,
        decision,
        charged,
        usage_after,
        refusal,
    }
}

fn default_true() -> bool {
    true
}

fn default_max_requests() -> usize {
    MAX_RUNTIME_REQUESTS
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}

fn has_blocking(issues: &[SandboxRuntimeIssue], code: &str) -> bool {
    issues
        .iter()
        .any(|issue| issue.code == code && issue.severity == SandboxIssueSeverity::Blocking)
}

fn blocking(
    issues: &mut Vec<SandboxRuntimeIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SandboxRuntimeIssue {
        code: code.into(),
        severity: SandboxIssueSeverity::Blocking,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

fn warning(
    issues: &mut Vec<SandboxRuntimeIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SandboxRuntimeIssue {
        code: code.into(),
        severity: SandboxIssueSeverity::Warning,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox_admission::{
        SandboxArtifact, SandboxArtifactKind, SandboxExecutionProfile, SandboxNetworkMode,
        SandboxSystem, SandboxTrust, SANDBOX_MANIFEST_SCHEMA,
    };

    fn admission() -> SandboxManifest {
        SandboxManifest {
            schema: SANDBOX_MANIFEST_SCHEMA.into(),
            system: SandboxSystem {
                id: "runtime-test".into(),
                version: "0.1.0".into(),
                owner: "platform".into(),
            },
            artifacts: vec![
                SandboxArtifact {
                    id: "source".into(),
                    kind: SandboxArtifactKind::SourceCode,
                    digest: "a".repeat(64),
                    source: "repo/source.py".into(),
                    producer: "ci".into(),
                    trust: SandboxTrust::Reviewed,
                    inputs: vec![],
                },
                SandboxArtifact {
                    id: "dataset".into(),
                    kind: SandboxArtifactKind::Dataset,
                    digest: "b".repeat(64),
                    source: "registry/dataset".into(),
                    producer: "registry".into(),
                    trust: SandboxTrust::Untrusted,
                    inputs: vec!["source".into()],
                },
            ],
            profiles: vec![SandboxExecutionProfile {
                id: "profile".into(),
                artifact: "dataset".into(),
                runtime: "oci".into(),
                image_digest: Some("b".repeat(64)),
                environment_digest: Some("c".repeat(64)),
                user: "runner".into(),
                rootless: true,
                read_only_root: true,
                no_privilege_escalation: true,
                network: SandboxNetworkMode::Allowlist,
                network_allowlist: vec!["packages.example".into()],
                mounts: vec![],
                capabilities: vec!["read".into(), "network".into()],
                resources: SandboxResourceLimits {
                    cpu_millis: Some(1000),
                    memory_mb: Some(1024),
                    wall_time_seconds: Some(60),
                    processes: Some(8),
                    output_bytes: Some(1_000_000),
                },
                output_quarantine: true,
                release_requires_review: true,
            }],
            capabilities: vec![
                SandboxCapability {
                    id: "read".into(),
                    profile: "profile".into(),
                    kind: SandboxCapabilityKind::FilesystemRead,
                    target: "/inputs/data".into(),
                    decision: SandboxDecision::Allow,
                    evidence_digest: None,
                },
                SandboxCapability {
                    id: "network".into(),
                    profile: "profile".into(),
                    kind: SandboxCapabilityKind::NetworkEgress,
                    target: "packages.example".into(),
                    decision: SandboxDecision::Allow,
                    evidence_digest: Some("d".repeat(64)),
                },
            ],
            outputs: vec![],
            policies: Default::default(),
        }
    }

    fn request(id: &str, kind: SandboxCapabilityKind, target: &str) -> SandboxRuntimeRequest {
        SandboxRuntimeRequest {
            id: id.into(),
            kind,
            target: target.into(),
            cpu_millis: 100,
            memory_mb: 128,
            wall_time_seconds: 5,
            processes: 1,
            output_bytes: 1000,
        }
    }

    fn manifest(requests: Vec<SandboxRuntimeRequest>) -> SandboxRuntimeManifest {
        SandboxRuntimeManifest {
            schema: SANDBOX_RUNTIME_MANIFEST_SCHEMA.into(),
            admission: admission(),
            profile: "profile".into(),
            requests,
            policies: Default::default(),
        }
    }

    #[test]
    fn valid_trace_charges_each_exact_capability_and_is_replayable() {
        let audit = manifest(vec![
            request(
                "read-input",
                SandboxCapabilityKind::FilesystemRead,
                "/inputs/data",
            ),
            request(
                "fetch-package",
                SandboxCapabilityKind::NetworkEgress,
                "packages.example",
            ),
        ])
        .audit()
        .expect("audit");
        assert!(
            audit.valid,
            "issues: {:?}; admission: {:?}",
            audit.issues, audit.admission_issues
        );
        assert_eq!(audit.simulated_count, 2);
        assert_eq!(audit.refused_count, 0);
        assert_eq!(audit.usage.cpu_millis, 200);
        assert_eq!(audit.usage.output_bytes, 2000);
        assert_eq!(audit.steps[0].decision, SandboxRuntimeDecision::Simulated);
        assert_ne!(audit.trace_digest, audit.admission_digest);
    }

    #[test]
    fn refusal_is_charged_never_and_stops_later_requests() {
        let audit = manifest(vec![
            request(
                "read-input",
                SandboxCapabilityKind::FilesystemRead,
                "/inputs/data",
            ),
            request(
                "escape",
                SandboxCapabilityKind::FilesystemRead,
                "/proc/self",
            ),
            request(
                "never-run",
                SandboxCapabilityKind::NetworkEgress,
                "packages.example",
            ),
        ])
        .audit()
        .expect("audit");
        assert!(!audit.valid);
        assert_eq!(audit.simulated_count, 1);
        assert_eq!(audit.refused_count, 1);
        assert_eq!(audit.not_run_count, 1);
        assert!(audit.stopped_on_refusal);
        assert_eq!(audit.usage.cpu_millis, 100);
        assert_eq!(
            audit.steps[1].refusal.as_deref(),
            Some("request_target_invalid")
        );
        assert_eq!(audit.steps[2].decision, SandboxRuntimeDecision::NotRun);
    }

    #[test]
    fn invalid_admission_never_becomes_runtime_ready() {
        let mut value = manifest(vec![request(
            "read-input",
            SandboxCapabilityKind::FilesystemRead,
            "/inputs/data",
        )]);
        value.admission.profiles[0].rootless = false;
        let audit = value.audit().expect("audit");
        assert!(!audit.admission_valid);
        assert!(!audit.simulation_started);
        assert_eq!(audit.not_run_count, 1);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "admission_invalid"));
    }
}
