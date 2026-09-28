//! Content-addressed compute-environment locking for autonomous preclinical glioma workflows.
//!
//! The lock is an admission product, not a package installer.  It turns a workflow manifest,
//! an institution's architecture profile, and trusted package metadata into an exact, replayable
//! environment identity.  Mutable or compromised sources fail closed; unavailable and
//! non-portable dependencies remain visible instead of being silently substituted.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaComputeEnvironmentLock1@1";
pub const MAX_DEPENDENCIES: usize = 2_048;
pub const MAX_WORKFLOW_TASKS: usize = 4_096;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentDependencyKind {
    OperatingSystem,
    Compiler,
    Library,
    Model,
    Accelerator,
    Runtime,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentArchitectureProfile {
    pub os_family: String,
    pub os_version: String,
    pub architecture: String,
    pub abi: String,
    pub cpu_feature_order: Vec<String>,
    pub accelerator_order: Vec<String>,
    pub host_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeEnvironmentDependency {
    pub name: String,
    pub kind: EnvironmentDependencyKind,
    pub version_constraint: String,
    pub resolved_version: String,
    pub source: String,
    pub source_digest: ContentHash,
    pub build_digest: ContentHash,
    pub runtime_abi: String,
    pub required: bool,
    pub available: bool,
    pub portable: bool,
    pub metadata_signed: bool,
    pub source_mutable: bool,
    pub compromised: bool,
    pub compatible_architecture_order: Vec<String>,
    pub contains_human_data: bool,
    pub contains_clinical_decision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeEnvironmentLockRequest {
    pub objective: String,
    pub workflow_manifest_digest: ContentHash,
    pub workflow_task_order: Vec<String>,
    pub architecture: EnvironmentArchitectureProfile,
    pub dependencies: Vec<ComputeEnvironmentDependency>,
    pub trusted_source_order: Vec<String>,
    pub require_signed_metadata: bool,
    pub require_portable_dependencies: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentDependencyStatus {
    Qualified,
    Unavailable,
    Untrusted,
    MutableSource,
    Compromised,
    VersionConflict,
    ArchitectureConflict,
    NonPortable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeEnvironmentDependencyRecord {
    pub name: String,
    pub kind: EnvironmentDependencyKind,
    pub version_constraint: String,
    pub resolved_version: String,
    pub source: String,
    pub source_digest: ContentHash,
    pub build_digest: ContentHash,
    pub runtime_abi: String,
    pub required: bool,
    pub status: EnvironmentDependencyStatus,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeEnvironmentLockDisposition {
    Qualified,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeEnvironmentLock {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub workflow_manifest_digest: ContentHash,
    pub workflow_task_order: Vec<String>,
    pub architecture: EnvironmentArchitectureProfile,
    pub dependency_order: Vec<ComputeEnvironmentDependencyRecord>,
    pub unavailable_dependency_order: Vec<String>,
    pub non_portable_dependency_order: Vec<String>,
    pub incompatible_dependency_order: Vec<String>,
    pub untrusted_dependency_order: Vec<String>,
    pub portability_limit_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputeEnvironmentLockDisposition,
    pub next_action: String,
    pub build_identity: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputeEnvironmentLockError {
    #[error("compute-environment lock request is invalid: {0}")]
    InvalidRequest(String),
    #[error("compute-environment lock output is invalid: {0}")]
    InvalidOutput(String),
    #[error("compute-environment lock digest failed: {0}")]
    Digest(String),
}

fn canonical_strings(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_non_empty(values: &[String], max_len: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max_len
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn exact_version_match(constraint: &str, resolved: &str) -> bool {
    let constraint = constraint.trim();
    if constraint == "*" || constraint.eq_ignore_ascii_case("any") {
        return true;
    }
    constraint.strip_prefix('=').unwrap_or(constraint).trim() == resolved.trim()
}

fn digest_input(lock: &ComputeEnvironmentLock) -> serde_json::Value {
    serde_json::json!({
        "feature_id": lock.feature_id,
        "output_schema": lock.output_schema,
        "objective": lock.objective,
        "workflow_manifest_digest": lock.workflow_manifest_digest,
        "workflow_task_order": lock.workflow_task_order,
        "architecture": lock.architecture,
        "dependency_order": lock.dependency_order,
        "unavailable_dependency_order": lock.unavailable_dependency_order,
        "non_portable_dependency_order": lock.non_portable_dependency_order,
        "incompatible_dependency_order": lock.incompatible_dependency_order,
        "untrusted_dependency_order": lock.untrusted_dependency_order,
        "portability_limit_order": lock.portability_limit_order,
        "negative_evidence": lock.negative_evidence,
        "uncertainty": lock.uncertainty,
        "disposition": lock.disposition,
        "next_action": lock.next_action,
        "build_identity": lock.build_identity,
    })
}

fn build_identity_input(
    request: &ComputeEnvironmentLockRequest,
    dependencies: &[ComputeEnvironmentDependencyRecord],
) -> serde_json::Value {
    serde_json::json!({
        "workflow_manifest_digest": request.workflow_manifest_digest,
        "workflow_task_order": request.workflow_task_order,
        "architecture": request.architecture,
        "dependencies": dependencies,
    })
}

fn validate_dependency(
    dependency: &ComputeEnvironmentDependency,
    request: &ComputeEnvironmentLockRequest,
) -> Result<(), ComputeEnvironmentLockError> {
    if dependency.name.trim().is_empty()
        || dependency.name.len() > MAX_TEXT_LEN
        || dependency.version_constraint.trim().is_empty()
        || dependency.version_constraint.len() > MAX_TEXT_LEN
        || dependency.resolved_version.trim().is_empty()
        || dependency.resolved_version.len() > MAX_TEXT_LEN
        || dependency.source.trim().is_empty()
        || dependency.source.len() > MAX_TEXT_LEN
        || dependency.runtime_abi.trim().is_empty()
        || dependency.runtime_abi.len() > MAX_TEXT_LEN
        || !valid_hash(&dependency.source_digest)
        || !valid_hash(&dependency.build_digest)
        || !unique_non_empty(&dependency.compatible_architecture_order, 128)
        || !canonical_strings(&dependency.compatible_architecture_order)
        || dependency.contains_human_data
        || dependency.contains_clinical_decision
    {
        return Err(ComputeEnvironmentLockError::InvalidRequest(
            "dependency metadata must be bounded, content-addressed, explicitly architecture-compatible, and non-clinical".into(),
        ));
    }
    if request.require_signed_metadata && !dependency.metadata_signed {
        return Err(ComputeEnvironmentLockError::InvalidRequest(
            "signed metadata is required for every dependency".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &ComputeEnvironmentLockRequest,
) -> Result<(), ComputeEnvironmentLockError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > MAX_TEXT_LEN
        || !valid_hash(&request.workflow_manifest_digest)
        || request.workflow_task_order.is_empty()
        || !unique_non_empty(&request.workflow_task_order, MAX_WORKFLOW_TASKS)
        || !canonical_strings(&request.workflow_task_order)
        || request.dependencies.is_empty()
        || request.dependencies.len() > MAX_DEPENDENCIES
        || !unique_non_empty(&request.trusted_source_order, MAX_DEPENDENCIES)
        || !canonical_strings(&request.trusted_source_order)
        || request.architecture.os_family.trim().is_empty()
        || request.architecture.os_version.trim().is_empty()
        || request.architecture.architecture.trim().is_empty()
        || request.architecture.abi.trim().is_empty()
        || !valid_hash(&request.architecture.host_digest)
        || !unique_non_empty(&request.architecture.cpu_feature_order, 256)
        || !canonical_strings(&request.architecture.cpu_feature_order)
        || !unique_non_empty(&request.architecture.accelerator_order, 256)
        || !canonical_strings(&request.architecture.accelerator_order)
    {
        return Err(ComputeEnvironmentLockError::InvalidRequest(
            "workflow identity, canonical task/dependency/source lists, architecture profile, and trusted host digest are required".into(),
        ));
    }
    let mut names = BTreeSet::new();
    for dependency in &request.dependencies {
        if !names.insert(dependency.name.clone()) {
            return Err(ComputeEnvironmentLockError::InvalidRequest(
                "dependency names must be unique".into(),
            ));
        }
        validate_dependency(dependency, request)?;
    }
    Ok(())
}

fn classify_dependency(
    dependency: &ComputeEnvironmentDependency,
    request: &ComputeEnvironmentLockRequest,
) -> (EnvironmentDependencyStatus, Vec<String>) {
    let mut reasons = Vec::new();
    if dependency.compromised {
        reasons.push("trusted metadata marks source or build as compromised".into());
        return (EnvironmentDependencyStatus::Compromised, reasons);
    }
    if dependency.source_mutable {
        reasons.push("source is mutable and cannot establish a replay identity".into());
        return (EnvironmentDependencyStatus::MutableSource, reasons);
    }
    if !request.trusted_source_order.contains(&dependency.source) || !dependency.metadata_signed {
        reasons.push("source or metadata is outside the trusted package policy".into());
        return (EnvironmentDependencyStatus::Untrusted, reasons);
    }
    if !dependency.available {
        reasons.push("dependency is unavailable at the institution".into());
        return (EnvironmentDependencyStatus::Unavailable, reasons);
    }
    if !exact_version_match(&dependency.version_constraint, &dependency.resolved_version) {
        reasons.push("resolved version does not satisfy the declared constraint".into());
        return (EnvironmentDependencyStatus::VersionConflict, reasons);
    }
    let architecture = &request.architecture.architecture;
    if !dependency
        .compatible_architecture_order
        .binary_search(architecture)
        .is_ok()
    {
        reasons.push(format!("architecture-unavailable:{architecture}"));
        return (EnvironmentDependencyStatus::ArchitectureConflict, reasons);
    }
    if request.require_portable_dependencies && !dependency.portable {
        reasons.push("dependency is explicitly non-portable under this lock policy".into());
        return (EnvironmentDependencyStatus::NonPortable, reasons);
    }
    reasons.push("exact version, source, build, ABI, and architecture metadata qualified".into());
    (EnvironmentDependencyStatus::Qualified, reasons)
}

impl ComputeEnvironmentLock {
    pub fn validate(&self) -> Result<(), ComputeEnvironmentLockError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !valid_hash(&self.workflow_manifest_digest)
            || self.workflow_task_order.is_empty()
            || !canonical_strings(&self.workflow_task_order)
            || self
                .dependency_order
                .windows(2)
                .any(|pair| pair[0].name >= pair[1].name)
            || !canonical_strings(&self.unavailable_dependency_order)
            || !canonical_strings(&self.non_portable_dependency_order)
            || !canonical_strings(&self.incompatible_dependency_order)
            || !canonical_strings(&self.untrusted_dependency_order)
            || !canonical_strings(&self.portability_limit_order)
            || !canonical_strings(&self.negative_evidence)
            || !canonical_strings(&self.uncertainty)
            || self.next_action.trim().is_empty()
            || !valid_hash(&self.build_identity)
            || self.dependency_order.iter().any(|dependency| {
                dependency.name.trim().is_empty()
                    || !valid_hash(&dependency.source_digest)
                    || !valid_hash(&dependency.build_digest)
                    || !canonical_strings(&dependency.reasons)
            })
        {
            return Err(ComputeEnvironmentLockError::InvalidOutput(
                "lock identity, canonical dependency records, classification lists, and build identity are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputeEnvironmentLockError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputeEnvironmentLockError::InvalidOutput(
                "compute-environment lock digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Resolve a reproducible environment admission lock without installing packages or executing code.
pub fn lock_glioma_compute_environment(
    request: &ComputeEnvironmentLockRequest,
) -> Result<ComputeEnvironmentLock, ComputeEnvironmentLockError> {
    validate_request(request)?;
    let mut dependencies = request.dependencies.clone();
    dependencies.sort_by(|left, right| left.name.cmp(&right.name));
    let mut records = Vec::with_capacity(dependencies.len());
    for dependency in &dependencies {
        let (status, mut reasons) = classify_dependency(dependency, request);
        reasons.sort();
        records.push(ComputeEnvironmentDependencyRecord {
            name: dependency.name.clone(),
            kind: dependency.kind,
            version_constraint: dependency.version_constraint.clone(),
            resolved_version: dependency.resolved_version.clone(),
            source: dependency.source.clone(),
            source_digest: dependency.source_digest.clone(),
            build_digest: dependency.build_digest.clone(),
            runtime_abi: dependency.runtime_abi.clone(),
            required: dependency.required,
            status,
            reasons,
        });
    }
    let unavailable = records
        .iter()
        .filter(|record| record.status == EnvironmentDependencyStatus::Unavailable)
        .map(|record| record.name.clone())
        .collect::<Vec<_>>();
    let non_portable = records
        .iter()
        .filter(|record| record.status == EnvironmentDependencyStatus::NonPortable)
        .map(|record| record.name.clone())
        .collect::<Vec<_>>();
    let incompatible = records
        .iter()
        .filter(|record| {
            matches!(
                record.status,
                EnvironmentDependencyStatus::ArchitectureConflict
                    | EnvironmentDependencyStatus::VersionConflict
            )
        })
        .map(|record| record.name.clone())
        .collect::<Vec<_>>();
    let untrusted = records
        .iter()
        .filter(|record| {
            matches!(
                record.status,
                EnvironmentDependencyStatus::Untrusted
                    | EnvironmentDependencyStatus::MutableSource
                    | EnvironmentDependencyStatus::Compromised
            )
        })
        .map(|record| record.name.clone())
        .collect::<Vec<_>>();
    let portability_limit_order = records
        .iter()
        .filter(|record| record.status != EnvironmentDependencyStatus::Qualified)
        .map(|record| format!("{}:{:?}", record.name, record.status))
        .collect::<Vec<_>>();
    let required_blockers = records
        .iter()
        .filter(|record| record.required && record.status != EnvironmentDependencyStatus::Qualified)
        .collect::<Vec<_>>();
    let has_compromise = records.iter().any(|record| {
        matches!(
            record.status,
            EnvironmentDependencyStatus::Compromised | EnvironmentDependencyStatus::MutableSource
        )
    });
    let has_untrusted = records
        .iter()
        .any(|record| record.status == EnvironmentDependencyStatus::Untrusted);
    let disposition = if has_compromise || !required_blockers.is_empty() {
        ComputeEnvironmentLockDisposition::Blocked
    } else if has_untrusted {
        ComputeEnvironmentLockDisposition::Unresolved
    } else if records
        .iter()
        .any(|record| record.status != EnvironmentDependencyStatus::Qualified)
    {
        ComputeEnvironmentLockDisposition::Partial
    } else {
        ComputeEnvironmentLockDisposition::Qualified
    };
    let mut negative_evidence = Vec::new();
    if !unavailable.is_empty() {
        negative_evidence.push("dependency-unavailable-at-local-institution".into());
    }
    if !incompatible.is_empty() {
        negative_evidence.push("version-or-architecture-compatibility-failure".into());
    }
    if !untrusted.is_empty() {
        negative_evidence.push("trusted-source-or-integrity-gate-not-satisfied".into());
    }
    let mut uncertainty = Vec::new();
    if !non_portable.is_empty() {
        uncertainty.push("non-portable-dependencies-limit-cross-site-replay".into());
    }
    if !unavailable.is_empty() {
        uncertainty.push("unavailable-dependencies-prevent-complete-environment-claim".into());
    }
    let next_action = match disposition {
        ComputeEnvironmentLockDisposition::Qualified => "admit_workflow_to_local_replay",
        ComputeEnvironmentLockDisposition::Partial => {
            "run_only_qualified_tasks_and_request_environment_repair"
        }
        ComputeEnvironmentLockDisposition::Blocked => "stop_before_dispatch_and_repair_environment",
        ComputeEnvironmentLockDisposition::Unresolved => "obtain_trusted_metadata_before_dispatch",
    };
    let build_identity = ContentHash::of_value(&build_identity_input(request, &records))
        .map_err(|error| ComputeEnvironmentLockError::Digest(error.to_string()))?;
    let mut lock = ComputeEnvironmentLock {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        workflow_manifest_digest: request.workflow_manifest_digest.clone(),
        workflow_task_order: request.workflow_task_order.clone(),
        architecture: request.architecture.clone(),
        dependency_order: records,
        unavailable_dependency_order: unavailable,
        non_portable_dependency_order: non_portable,
        incompatible_dependency_order: incompatible,
        untrusted_dependency_order: untrusted,
        portability_limit_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_action: next_action.into(),
        build_identity,
        digest: ContentHash::of_bytes(b"unsealed-glioma-compute-environment-lock"),
    };
    lock.digest = ContentHash::of_value(&digest_input(&lock))
        .map_err(|error| ComputeEnvironmentLockError::Digest(error.to_string()))?;
    lock.validate()?;
    Ok(lock)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn dependency(name: &str, version: &str) -> ComputeEnvironmentDependency {
        ComputeEnvironmentDependency {
            name: name.into(),
            kind: EnvironmentDependencyKind::Library,
            version_constraint: format!("={version}"),
            resolved_version: version.into(),
            source: "registry://trusted".into(),
            source_digest: hash(&format!("source-{name}")),
            build_digest: hash(&format!("build-{name}")),
            runtime_abi: "abi-v1".into(),
            required: true,
            available: true,
            portable: true,
            metadata_signed: true,
            source_mutable: false,
            compromised: false,
            compatible_architecture_order: vec!["x86_64".into()],
            contains_human_data: false,
            contains_clinical_decision: false,
        }
    }

    fn request(dependencies: Vec<ComputeEnvironmentDependency>) -> ComputeEnvironmentLockRequest {
        ComputeEnvironmentLockRequest {
            objective: "lock an autonomous glioma computation environment".into(),
            workflow_manifest_digest: hash("workflow-v1"),
            workflow_task_order: vec!["model".into(), "normalize".into()],
            architecture: EnvironmentArchitectureProfile {
                os_family: "linux".into(),
                os_version: "6.8".into(),
                architecture: "x86_64".into(),
                abi: "gnu".into(),
                cpu_feature_order: vec!["avx2".into()],
                accelerator_order: vec!["none".into()],
                host_digest: hash("host-v1"),
            },
            dependencies,
            trusted_source_order: vec!["registry://trusted".into()],
            require_signed_metadata: true,
            require_portable_dependencies: true,
        }
    }

    #[test]
    fn clean_lock_is_qualified_and_replay_stable() {
        let first = lock_glioma_compute_environment(&request(vec![
            dependency("numpy", "2.1.0"),
            dependency("pytorch", "2.5.0"),
        ]))
        .expect("lock succeeds");
        let second = lock_glioma_compute_environment(&request(vec![
            dependency("pytorch", "2.5.0"),
            dependency("numpy", "2.1.0"),
        ]))
        .expect("lock succeeds");
        assert_eq!(
            first.disposition,
            ComputeEnvironmentLockDisposition::Qualified
        );
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.build_identity, second.build_identity);
    }

    #[test]
    fn mutable_or_compromised_dependency_blocks_before_dispatch() {
        let mut mutable = dependency("mutable-lib", "1.0.0");
        mutable.source_mutable = true;
        let output = lock_glioma_compute_environment(&request(vec![mutable])).expect("lock report");
        assert_eq!(
            output.disposition,
            ComputeEnvironmentLockDisposition::Blocked
        );
        assert!(output
            .untrusted_dependency_order
            .contains(&"mutable-lib".into()));
        assert_eq!(
            output.next_action,
            "stop_before_dispatch_and_repair_environment"
        );
    }

    #[test]
    fn optional_unavailable_dependency_is_explicitly_partial() {
        let mut optional = dependency("optional-gpu", "1.0.0");
        optional.required = false;
        optional.available = false;
        let output =
            lock_glioma_compute_environment(&request(vec![dependency("core", "1.0.0"), optional]))
                .expect("lock report");
        assert_eq!(
            output.disposition,
            ComputeEnvironmentLockDisposition::Partial
        );
        assert!(output
            .unavailable_dependency_order
            .contains(&"optional-gpu".into()));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("unavailable")));
    }

    #[test]
    fn architecture_and_version_conflicts_never_qualify() {
        let mut conflict = dependency("accelerator-runtime", "2.0.0");
        conflict.version_constraint = "=1.0.0".into();
        conflict.compatible_architecture_order = vec!["aarch64".into()];
        let output =
            lock_glioma_compute_environment(&request(vec![conflict])).expect("lock report");
        assert_eq!(
            output.disposition,
            ComputeEnvironmentLockDisposition::Blocked
        );
        assert!(output
            .incompatible_dependency_order
            .contains(&"accelerator-runtime".into()));
    }

    #[test]
    fn human_or_clinical_dependency_metadata_is_rejected() {
        let mut prohibited = dependency("bad-model", "1.0.0");
        prohibited.contains_human_data = true;
        assert!(matches!(
            lock_glioma_compute_environment(&request(vec![prohibited])),
            Err(ComputeEnvironmentLockError::InvalidRequest(_))
        ));
    }
}
