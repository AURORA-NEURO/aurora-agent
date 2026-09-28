//! Constrained environment-resolution proposals for autonomous glioma computation.
//!
//! This agent proposes exact repairs against a previously declared environment request. It does
//! not install packages, fetch hardware, or silently rewrite scientific dependencies. Candidates
//! are evaluated against the same trusted-source, integrity, architecture, portability, and
//! locality gates as [`super::execution_environment_lock`], and only an explicit approval can
//! change a pinned version or build.

use super::execution_environment_lock::{
    lock_glioma_compute_environment, ComputeEnvironmentDependency, ComputeEnvironmentLock,
    ComputeEnvironmentLockDisposition, ComputeEnvironmentLockRequest, EnvironmentDependencyKind,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaEnvironmentResolutionProposal1@1";
pub const MAX_CANDIDATES: usize = 2_048;
pub const MAX_CHANGES: usize = 512;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentResolutionChangeKind {
    AvailabilityRepair,
    VersionChange,
    SourceChange,
    BuildChange,
    ArchitectureFallback,
    PortabilityChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentResolutionCandidate {
    pub candidate_id: String,
    pub dependency_name: String,
    pub kind: EnvironmentDependencyKind,
    pub proposed_version: String,
    pub proposed_source: String,
    pub proposed_source_digest: ContentHash,
    pub proposed_build_digest: ContentHash,
    pub proposed_runtime_abi: String,
    pub proposed_available: bool,
    pub proposed_portable: bool,
    pub proposed_metadata_signed: bool,
    pub proposed_source_mutable: bool,
    pub proposed_compromised: bool,
    pub proposed_compatible_architecture_order: Vec<String>,
    pub rationale: String,
    pub cost_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentResolutionRequest {
    pub objective: String,
    pub base: ComputeEnvironmentLockRequest,
    pub candidates: Vec<EnvironmentResolutionCandidate>,
    pub approval_granted: bool,
    pub allow_version_changes: bool,
    pub allow_source_changes: bool,
    pub max_changes: usize,
    pub max_cost_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentResolutionChange {
    pub candidate_id: String,
    pub dependency_name: String,
    pub kind: EnvironmentResolutionChangeKind,
    pub from_version: String,
    pub to_version: String,
    pub from_source: String,
    pub to_source: String,
    pub approved: bool,
    pub compatibility_evidence: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentResolutionDisposition {
    NoChange,
    Proposed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentResolutionProposal {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub base_lock: ComputeEnvironmentLock,
    pub changes: Vec<EnvironmentResolutionChange>,
    pub selected_candidate_order: Vec<String>,
    pub rejected_candidate_order: Vec<String>,
    pub unresolved_dependency_order: Vec<String>,
    pub required_approval: bool,
    pub resulting_lock: Option<ComputeEnvironmentLock>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EnvironmentResolutionDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnvironmentResolutionError {
    #[error("environment resolution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("environment resolution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("environment resolution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_non_empty(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn digest_input(proposal: &EnvironmentResolutionProposal) -> serde_json::Value {
    serde_json::json!({
        "feature_id": proposal.feature_id,
        "output_schema": proposal.output_schema,
        "objective": proposal.objective,
        "base_lock": proposal.base_lock,
        "changes": proposal.changes,
        "selected_candidate_order": proposal.selected_candidate_order,
        "rejected_candidate_order": proposal.rejected_candidate_order,
        "unresolved_dependency_order": proposal.unresolved_dependency_order,
        "required_approval": proposal.required_approval,
        "resulting_lock": proposal.resulting_lock,
        "negative_evidence": proposal.negative_evidence,
        "uncertainty": proposal.uncertainty,
        "disposition": proposal.disposition,
        "next_action": proposal.next_action,
    })
}

fn validate_candidate(
    candidate: &EnvironmentResolutionCandidate,
) -> Result<(), EnvironmentResolutionError> {
    if !unique_non_empty(&[candidate.candidate_id.clone()], 1)
        || !unique_non_empty(&[candidate.dependency_name.clone()], 1)
        || candidate.proposed_version.trim().is_empty()
        || candidate.proposed_version.len() > MAX_TEXT_LEN
        || candidate.proposed_source.trim().is_empty()
        || candidate.proposed_source.len() > MAX_TEXT_LEN
        || candidate.proposed_runtime_abi.trim().is_empty()
        || candidate.proposed_runtime_abi.len() > MAX_TEXT_LEN
        || candidate.rationale.trim().is_empty()
        || candidate.rationale.len() > MAX_TEXT_LEN
        || !valid_hash(&candidate.proposed_source_digest)
        || !valid_hash(&candidate.proposed_build_digest)
        || !unique_non_empty(&candidate.proposed_compatible_architecture_order, 256)
        || !canonical(&candidate.proposed_compatible_architecture_order)
    {
        return Err(EnvironmentResolutionError::InvalidRequest(
            "resolution candidates require bounded identities, exact digests, architecture coverage, and rationale".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &EnvironmentResolutionRequest,
) -> Result<(), EnvironmentResolutionError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > MAX_TEXT_LEN
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_changes == 0
        || request.max_changes > MAX_CHANGES
        || request.max_cost_units == 0
    {
        return Err(EnvironmentResolutionError::InvalidRequest(
            "bounded objective, candidate set, change budget, and cost budget are required".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if !candidate_ids.insert(candidate.candidate_id.clone()) {
            return Err(EnvironmentResolutionError::InvalidRequest(
                "candidate identities must be unique".into(),
            ));
        }
        validate_candidate(candidate)?;
    }
    Ok(())
}

fn dependency_from_candidate(
    current: &ComputeEnvironmentDependency,
    candidate: &EnvironmentResolutionCandidate,
) -> ComputeEnvironmentDependency {
    ComputeEnvironmentDependency {
        name: current.name.clone(),
        kind: current.kind,
        version_constraint: format!("={}", candidate.proposed_version),
        resolved_version: candidate.proposed_version.clone(),
        source: candidate.proposed_source.clone(),
        source_digest: candidate.proposed_source_digest.clone(),
        build_digest: candidate.proposed_build_digest.clone(),
        runtime_abi: candidate.proposed_runtime_abi.clone(),
        required: current.required,
        available: candidate.proposed_available,
        portable: candidate.proposed_portable,
        metadata_signed: candidate.proposed_metadata_signed,
        source_mutable: candidate.proposed_source_mutable,
        compromised: candidate.proposed_compromised,
        compatible_architecture_order: candidate.proposed_compatible_architecture_order.clone(),
        contains_human_data: false,
        contains_clinical_decision: false,
    }
}

fn change_kind(
    current: &ComputeEnvironmentDependency,
    candidate: &EnvironmentResolutionCandidate,
) -> EnvironmentResolutionChangeKind {
    if current.resolved_version != candidate.proposed_version {
        EnvironmentResolutionChangeKind::VersionChange
    } else if current.source != candidate.proposed_source {
        EnvironmentResolutionChangeKind::SourceChange
    } else if current.build_digest != candidate.proposed_build_digest {
        EnvironmentResolutionChangeKind::BuildChange
    } else if !current.available && candidate.proposed_available {
        EnvironmentResolutionChangeKind::AvailabilityRepair
    } else if current.portable != candidate.proposed_portable {
        EnvironmentResolutionChangeKind::PortabilityChange
    } else {
        EnvironmentResolutionChangeKind::ArchitectureFallback
    }
}

fn candidate_change(
    current: &ComputeEnvironmentDependency,
    candidate: &EnvironmentResolutionCandidate,
    approval_granted: bool,
) -> EnvironmentResolutionChange {
    let kind = change_kind(current, candidate);
    let mut reasons = vec![candidate.rationale.clone()];
    let mut evidence = vec![
        "candidate evaluated against the base workflow manifest and local architecture profile".into(),
        "candidate must pass trusted-source, signature, integrity, availability, and portability gates".into(),
    ];
    if current.resolved_version != candidate.proposed_version && !approval_granted {
        reasons.push("pinned scientific version cannot change without explicit approval".into());
    }
    if candidate.proposed_source_mutable {
        reasons.push("mutable candidate source is not replayable".into());
    }
    if candidate.proposed_compromised {
        reasons.push("candidate is marked compromised".into());
    }
    evidence.sort();
    reasons.sort();
    EnvironmentResolutionChange {
        candidate_id: candidate.candidate_id.clone(),
        dependency_name: current.name.clone(),
        kind,
        from_version: current.resolved_version.clone(),
        to_version: candidate.proposed_version.clone(),
        from_source: current.source.clone(),
        to_source: candidate.proposed_source.clone(),
        approved: approval_granted,
        compatibility_evidence: evidence,
        reasons,
    }
}

impl EnvironmentResolutionProposal {
    pub fn validate(&self) -> Result<(), EnvironmentResolutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.base_lock.validate().is_err()
            || !canonical(&self.selected_candidate_order)
            || !canonical(&self.rejected_candidate_order)
            || !canonical(&self.unresolved_dependency_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.changes.windows(2).any(|pair| {
                (
                    pair[0].dependency_name.clone(),
                    pair[0].candidate_id.clone(),
                ) >= (
                    pair[1].dependency_name.clone(),
                    pair[1].candidate_id.clone(),
                )
            })
            || self.changes.iter().any(|change| {
                change.candidate_id.trim().is_empty()
                    || change.dependency_name.trim().is_empty()
                    || change.from_version.trim().is_empty()
                    || change.to_version.trim().is_empty()
                    || !canonical(&change.compatibility_evidence)
                    || !canonical(&change.reasons)
            })
            || self.next_action.trim().is_empty()
        {
            return Err(EnvironmentResolutionError::InvalidOutput(
                "proposal identity, base lock, canonical candidate/change ordering, evidence, and next action are invalid".into(),
            ));
        }
        if let Some(lock) = &self.resulting_lock {
            lock.validate().map_err(|_| {
                EnvironmentResolutionError::InvalidOutput(
                    "resulting environment lock is invalid".into(),
                )
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EnvironmentResolutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EnvironmentResolutionError::InvalidOutput(
                "resolution proposal digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Propose bounded environment repairs without silently changing a pinned workflow.
pub fn resolve_glioma_compute_environment(
    request: &EnvironmentResolutionRequest,
) -> Result<EnvironmentResolutionProposal, EnvironmentResolutionError> {
    validate_request(request)?;
    let base_lock = lock_glioma_compute_environment(&request.base)
        .map_err(|error| EnvironmentResolutionError::InvalidRequest(error.to_string()))?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let mut dependency_map = request
        .base
        .dependencies
        .iter()
        .map(|dependency| (dependency.name.clone(), dependency.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut changes = Vec::new();
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut total_cost = 0_u64;
    let mut changed_dependencies = BTreeSet::new();
    for candidate in &candidates {
        let Some(current) = dependency_map.get(&candidate.dependency_name).cloned() else {
            rejected.push(candidate.candidate_id.clone());
            continue;
        };
        if changed_dependencies.contains(&candidate.dependency_name) {
            rejected.push(candidate.candidate_id.clone());
            continue;
        }
        let change = candidate_change(&current, candidate, request.approval_granted);
        let version_changed = current.resolved_version != candidate.proposed_version;
        let source_changed = current.source != candidate.proposed_source;
        let budget_ok = changes.len() < request.max_changes
            && total_cost.saturating_add(candidate.cost_units) <= request.max_cost_units;
        let policy_ok = (!version_changed || request.allow_version_changes)
            && (!source_changed || request.allow_source_changes)
            && request.approval_granted;
        let safe_candidate = !candidate.proposed_source_mutable
            && !candidate.proposed_compromised
            && candidate.proposed_metadata_signed
            && candidate.proposed_available
            && candidate
                .proposed_compatible_architecture_order
                .binary_search(&request.base.architecture.architecture)
                .is_ok()
            && request
                .base
                .trusted_source_order
                .contains(&candidate.proposed_source);
        if budget_ok && policy_ok && safe_candidate {
            total_cost = total_cost.saturating_add(candidate.cost_units);
            dependency_map.insert(
                candidate.dependency_name.clone(),
                dependency_from_candidate(&current, candidate),
            );
            changed_dependencies.insert(candidate.dependency_name.clone());
            selected.push(candidate.candidate_id.clone());
            changes.push(change);
        } else {
            if !safe_candidate || !policy_ok {
                unresolved.insert(candidate.dependency_name.clone());
            }
            rejected.push(candidate.candidate_id.clone());
        }
    }
    changes.sort_by(|left, right| {
        (left.dependency_name.clone(), left.candidate_id.clone())
            .cmp(&(right.dependency_name.clone(), right.candidate_id.clone()))
    });
    selected.sort();
    rejected.sort();
    let mut resulting_lock = None;
    if !changes.is_empty() {
        let mut repaired_request = request.base.clone();
        repaired_request.dependencies = dependency_map.into_values().collect();
        repaired_request
            .dependencies
            .sort_by(|left, right| left.name.cmp(&right.name));
        resulting_lock = Some(
            lock_glioma_compute_environment(&repaired_request)
                .map_err(|error| EnvironmentResolutionError::InvalidRequest(error.to_string()))?,
        );
    }
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !rejected.is_empty() {
        negative_evidence.push("one-or-more-resolution-candidates-rejected".into());
    }
    if !unresolved.is_empty() {
        uncertainty.push("candidate-conflicts-or-missing-approval-remain-unresolved".into());
    }
    if !changes.is_empty() && !request.approval_granted {
        uncertainty.push("explicit-approval-required-before-pinned-environment-change".into());
    }
    let disposition = if changes.is_empty() {
        match base_lock.disposition {
            ComputeEnvironmentLockDisposition::Qualified => {
                EnvironmentResolutionDisposition::NoChange
            }
            ComputeEnvironmentLockDisposition::Blocked => EnvironmentResolutionDisposition::Blocked,
            ComputeEnvironmentLockDisposition::Partial
            | ComputeEnvironmentLockDisposition::Unresolved => {
                EnvironmentResolutionDisposition::Unresolved
            }
        }
    } else if resulting_lock
        .as_ref()
        .is_some_and(|lock| lock.disposition == ComputeEnvironmentLockDisposition::Qualified)
    {
        EnvironmentResolutionDisposition::Proposed
    } else {
        EnvironmentResolutionDisposition::Unresolved
    };
    let next_action = match disposition {
        EnvironmentResolutionDisposition::NoChange => "reuse_existing_qualified_environment",
        EnvironmentResolutionDisposition::Proposed => "review_and_admit_resulting_environment_lock",
        EnvironmentResolutionDisposition::Blocked => "obtain_safe_candidate_or_operator_repair",
        EnvironmentResolutionDisposition::Unresolved => {
            "resolve_conflicts_and_obtain_required_approval"
        }
    };
    let mut proposal = EnvironmentResolutionProposal {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        base_lock,
        changes,
        selected_candidate_order: selected,
        rejected_candidate_order: rejected,
        unresolved_dependency_order: unresolved.into_iter().collect(),
        required_approval: !request.approval_granted && !request.candidates.is_empty(),
        resulting_lock,
        negative_evidence,
        uncertainty,
        disposition,
        next_action: next_action.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-environment-resolution"),
    };
    proposal.digest = ContentHash::of_value(&digest_input(&proposal))
        .map_err(|error| EnvironmentResolutionError::Digest(error.to_string()))?;
    proposal.validate()?;
    Ok(proposal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::execution_environment_lock::{
        EnvironmentArchitectureProfile, EnvironmentDependencyKind,
    };

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn dependency(name: &str, version: &str, available: bool) -> ComputeEnvironmentDependency {
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
            available,
            portable: true,
            metadata_signed: true,
            source_mutable: false,
            compromised: false,
            compatible_architecture_order: vec!["x86_64".into()],
            contains_human_data: false,
            contains_clinical_decision: false,
        }
    }

    fn base(dependencies: Vec<ComputeEnvironmentDependency>) -> ComputeEnvironmentLockRequest {
        ComputeEnvironmentLockRequest {
            objective: "prepare a pinned autonomous glioma analysis".into(),
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

    fn candidate(name: &str, version: &str, compromised: bool) -> EnvironmentResolutionCandidate {
        EnvironmentResolutionCandidate {
            candidate_id: format!("candidate-{name}-{version}"),
            dependency_name: name.into(),
            kind: EnvironmentDependencyKind::Library,
            proposed_version: version.into(),
            proposed_source: "registry://trusted".into(),
            proposed_source_digest: hash(&format!("candidate-source-{name}-{version}")),
            proposed_build_digest: hash(&format!("candidate-build-{name}-{version}")),
            proposed_runtime_abi: "abi-v1".into(),
            proposed_available: true,
            proposed_portable: true,
            proposed_metadata_signed: true,
            proposed_source_mutable: false,
            proposed_compromised: compromised,
            proposed_compatible_architecture_order: vec!["x86_64".into()],
            rationale: "trusted mirror contains a reproducible compatible build".into(),
            cost_units: 1,
        }
    }

    #[test]
    fn approved_repair_produces_qualified_result() {
        let request = EnvironmentResolutionRequest {
            objective: "repair missing numpy".into(),
            base: base(vec![dependency("numpy", "2.1.0", false)]),
            candidates: vec![candidate("numpy", "2.1.0", false)],
            approval_granted: true,
            allow_version_changes: false,
            allow_source_changes: false,
            max_changes: 2,
            max_cost_units: 4,
        };
        let proposal = resolve_glioma_compute_environment(&request).expect("proposal succeeds");
        assert_eq!(
            proposal.disposition,
            EnvironmentResolutionDisposition::Proposed
        );
        assert_eq!(proposal.selected_candidate_order.len(), 1);
        assert_eq!(
            proposal.resulting_lock.as_ref().unwrap().disposition,
            ComputeEnvironmentLockDisposition::Qualified
        );
    }

    #[test]
    fn pinned_version_stays_unchanged_without_approval() {
        let request = EnvironmentResolutionRequest {
            objective: "do not silently upgrade numpy".into(),
            base: base(vec![dependency("numpy", "2.1.0", true)]),
            candidates: vec![candidate("numpy", "2.2.0", false)],
            approval_granted: false,
            allow_version_changes: true,
            allow_source_changes: false,
            max_changes: 2,
            max_cost_units: 4,
        };
        let proposal = resolve_glioma_compute_environment(&request).expect("proposal succeeds");
        assert!(proposal.changes.is_empty());
        assert!(proposal
            .rejected_candidate_order
            .contains(&"candidate-numpy-2.2.0".into()));
        assert_eq!(
            proposal.base_lock.dependency_order[0].resolved_version,
            "2.1.0"
        );
    }

    #[test]
    fn poisoned_candidate_is_rejected_and_unresolved() {
        let request = EnvironmentResolutionRequest {
            objective: "reject poisoned candidate".into(),
            base: base(vec![dependency("cuda", "12.4.0", false)]),
            candidates: vec![candidate("cuda", "12.4.0", true)],
            approval_granted: true,
            allow_version_changes: false,
            allow_source_changes: false,
            max_changes: 2,
            max_cost_units: 4,
        };
        let proposal = resolve_glioma_compute_environment(&request).expect("proposal succeeds");
        assert_eq!(
            proposal.disposition,
            EnvironmentResolutionDisposition::Blocked
        );
        assert!(proposal
            .unresolved_dependency_order
            .contains(&"cuda".into()));
    }

    #[test]
    fn candidate_order_does_not_change_proposal_digest() {
        let mut request = EnvironmentResolutionRequest {
            objective: "stable proposal".into(),
            base: base(vec![dependency("numpy", "2.1.0", false)]),
            candidates: vec![candidate("numpy", "2.1.0", false)],
            approval_granted: true,
            allow_version_changes: false,
            allow_source_changes: false,
            max_changes: 2,
            max_cost_units: 4,
        };
        let left = resolve_glioma_compute_environment(&request).expect("left");
        request.candidates.reverse();
        let right = resolve_glioma_compute_environment(&request).expect("right");
        assert_eq!(left.digest, right.digest);
    }
}
