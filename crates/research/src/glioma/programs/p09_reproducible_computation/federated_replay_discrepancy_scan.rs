//! Federated replay discrepancy localization for preclinical glioma computation.
//!
//! This feature compares signed, aggregate execution attestations without moving raw
//! study inputs.  It tells a consortium which replay identity changed (workflow, data
//! version, environment, dependency lock, numerical kernel, seed, or output) and emits
//! bounded site-local diagnostic tasks.  It is deliberately a localization tool, not a
//! claim that a computation is biologically correct and never a clinical decision path.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedReplayDiscrepancy1@1";
pub const MAX_ATTESTATIONS: usize = 256;
pub const MAX_STAGES: usize = 256;
pub const MAX_DIAGNOSTICS_PER_SITE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReplayAttestation {
    pub site_id: String,
    pub replay_group_id: String,
    pub run_id: String,
    pub workflow_digest: ContentHash,
    pub input_data_version_digest: ContentHash,
    pub environment_digest: ContentHash,
    pub dependency_lock_digest: ContentHash,
    pub numeric_kernel_digest: ContentHash,
    pub seed_digest: ContentHash,
    pub output_digest: ContentHash,
    pub stage_digests: BTreeMap<String, ContentHash>,
    pub missing_fields: Vec<String>,
    pub signer_id: String,
    pub signature_digest: ContentHash,
    pub signature_valid: bool,
    pub permitted_summary_only: bool,
    pub contains_raw_inputs: bool,
    pub contains_human_data: bool,
    pub contains_clinical_decision: bool,
    pub generated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReplayDiscrepancyScanRequest {
    pub objective: String,
    pub replay_group_id: String,
    pub reference_site_id: String,
    pub required_stage_order: Vec<String>,
    pub attestations: Vec<FederatedReplayAttestation>,
    pub permitted_site_order: Vec<String>,
    pub federation_policy_digest: ContentHash,
    pub max_diagnostics_per_site: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayDivergenceKind {
    NoDivergence,
    InputDataVersion,
    Environment,
    Dependency,
    NumericKernel,
    Seed,
    Workflow,
    Output,
    Multiple,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayDiagnosticTask {
    pub task_id: String,
    pub site_id: String,
    pub stage: String,
    pub action: String,
    pub reason: String,
    pub local_only: bool,
    pub requires_raw_data: bool,
    pub dispatchable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReplayDiscrepancy {
    pub site_id: String,
    pub reference_site_id: String,
    pub divergence_kind: ReplayDivergenceKind,
    pub observed_difference_fields: Vec<String>,
    pub divergence_stage: Option<String>,
    pub confidence_milli: u16,
    pub missing_fields: Vec<String>,
    pub diagnostic_task_order: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedReplayDiscrepancyDisposition {
    Conformant,
    Divergent,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReplayDiscrepancyReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_group_id: String,
    pub reference_site_id: String,
    pub attestation_order: Vec<String>,
    pub discrepancies: Vec<FederatedReplayDiscrepancy>,
    pub diagnostics: Vec<ReplayDiagnosticTask>,
    pub unresolved_site_order: Vec<String>,
    pub divergent_site_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedReplayDiscrepancyDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedReplayDiscrepancyError {
    #[error("federated replay discrepancy request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated replay discrepancy output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated replay discrepancy digest failed: {0}")]
    Digest(String),
}

fn canonical_strings(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_non_empty(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn required_field_names() -> [&'static str; 8] {
    [
        "workflow_digest",
        "input_data_version_digest",
        "environment_digest",
        "dependency_lock_digest",
        "numeric_kernel_digest",
        "seed_digest",
        "output_digest",
        "stage_digests",
    ]
}

fn validate_attestation(
    attestation: &FederatedReplayAttestation,
    request: &FederatedReplayDiscrepancyScanRequest,
) -> Result<(), FederatedReplayDiscrepancyError> {
    if attestation.site_id.trim().is_empty()
        || attestation.replay_group_id != request.replay_group_id
        || attestation.run_id.trim().is_empty()
        || attestation.signer_id.trim().is_empty()
        || !valid_hash(&attestation.workflow_digest)
        || !valid_hash(&attestation.input_data_version_digest)
        || !valid_hash(&attestation.environment_digest)
        || !valid_hash(&attestation.dependency_lock_digest)
        || !valid_hash(&attestation.numeric_kernel_digest)
        || !valid_hash(&attestation.seed_digest)
        || !valid_hash(&attestation.output_digest)
        || !valid_hash(&attestation.signature_digest)
        || !valid_hash(&request.federation_policy_digest)
        || !valid_hashes(&attestation.stage_digests)
        || !canonical_strings(&attestation.missing_fields)
        || !unique_non_empty(&attestation.missing_fields)
        || !attestation.signature_valid
        || !attestation.permitted_summary_only
        || attestation.contains_raw_inputs
        || attestation.contains_human_data
        || attestation.contains_clinical_decision
        || attestation.generated_at_unix_seconds == 0
    {
        return Err(FederatedReplayDiscrepancyError::InvalidRequest(
            "attestation must be signed, aggregate-only, non-clinical, content-addressed, and scoped to the request replay group".into(),
        ));
    }
    let missing = attestation
        .missing_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for field in required_field_names() {
        if missing.contains(field) {
            continue;
        }
        if field == "stage_digests"
            && request
                .required_stage_order
                .iter()
                .any(|stage| !attestation.stage_digests.contains_key(stage))
            {
                return Err(FederatedReplayDiscrepancyError::InvalidRequest(
                    "missing stage digests must be declared explicitly".into(),
                ));
            }
    }
    Ok(())
}

fn valid_hashes(values: &BTreeMap<String, ContentHash>) -> bool {
    values
        .iter()
        .all(|(key, value)| !key.trim().is_empty() && valid_hash(value))
}

fn validate_request(
    request: &FederatedReplayDiscrepancyScanRequest,
) -> Result<(), FederatedReplayDiscrepancyError> {
    if request.objective.trim().is_empty()
        || request.replay_group_id.trim().is_empty()
        || request.reference_site_id.trim().is_empty()
        || request.required_stage_order.is_empty()
        || request.required_stage_order.len() > MAX_STAGES
        || !canonical_strings(&request.required_stage_order)
        || !unique_non_empty(&request.required_stage_order)
        || request.attestations.len() < 2
        || request.attestations.len() > MAX_ATTESTATIONS
        || !canonical_strings(&request.permitted_site_order)
        || !unique_non_empty(&request.permitted_site_order)
        || request.max_diagnostics_per_site == 0
        || request.max_diagnostics_per_site > MAX_DIAGNOSTICS_PER_SITE
        || !valid_hash(&request.federation_policy_digest)
        || !request
            .permitted_site_order
            .contains(&request.reference_site_id)
    {
        return Err(FederatedReplayDiscrepancyError::InvalidRequest(
            "objective, replay group, canonical stages/sites, bounded attestations, and policy scope are required".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    for attestation in &request.attestations {
        if !sites.insert(attestation.site_id.clone())
            || !request.permitted_site_order.contains(&attestation.site_id)
        {
            return Err(FederatedReplayDiscrepancyError::InvalidRequest(
                "attestation sites must be unique and explicitly permitted".into(),
            ));
        }
        validate_attestation(attestation, request)?;
    }
    if !sites.contains(&request.reference_site_id) {
        return Err(FederatedReplayDiscrepancyError::InvalidRequest(
            "reference site must have an attestation".into(),
        ));
    }
    Ok(())
}

fn attestation_field_digest<'a>(
    attestation: &'a FederatedReplayAttestation,
    field: &str,
) -> Option<&'a ContentHash> {
    match field {
        "workflow_digest" => Some(&attestation.workflow_digest),
        "input_data_version_digest" => Some(&attestation.input_data_version_digest),
        "environment_digest" => Some(&attestation.environment_digest),
        "dependency_lock_digest" => Some(&attestation.dependency_lock_digest),
        "numeric_kernel_digest" => Some(&attestation.numeric_kernel_digest),
        "seed_digest" => Some(&attestation.seed_digest),
        "output_digest" => Some(&attestation.output_digest),
        _ => None,
    }
}

fn diagnostic_action(kind: ReplayDivergenceKind, _stage: &str) -> (&'static str, &'static str) {
    match kind {
        ReplayDivergenceKind::InputDataVersion => (
            "verify_local_input_version_and_replay",
            "input data-version identity differs from the reference",
        ),
        ReplayDivergenceKind::Environment => (
            "verify_local_environment_lock_and_replay",
            "compute environment identity differs from the reference",
        ),
        ReplayDivergenceKind::Dependency => (
            "verify_local_dependency_lock_and_replay",
            "dependency lock identity differs from the reference",
        ),
        ReplayDivergenceKind::NumericKernel => (
            "compare_local_numeric_kernel_and_replay",
            "numeric-kernel identity differs from the reference",
        ),
        ReplayDivergenceKind::Seed => (
            "verify_local_seed_binding_and_replay",
            "randomness seed identity differs from the reference",
        ),
        ReplayDivergenceKind::Workflow => (
            "compare_local_workflow_stage_and_replay",
            "workflow or stage identity differs from the reference",
        ),
        ReplayDivergenceKind::Output => (
            "compare_local_output_canonicalization",
            "inputs and execution identities match but output identity differs",
        ),
        ReplayDivergenceKind::Multiple => (
            "run_local_identity_bisection_replay",
            "multiple replay identities differ from the reference",
        ),
        ReplayDivergenceKind::InsufficientEvidence => (
            "request_missing_local_attestation_fields",
            "permitted summaries are insufficient to localize divergence",
        ),
        ReplayDivergenceKind::NoDivergence => ("no_action", "replay identities match"),
    }
}

fn digest_input(report: &FederatedReplayDiscrepancyReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "objective": report.objective,
        "replay_group_id": report.replay_group_id,
        "reference_site_id": report.reference_site_id,
        "attestation_order": report.attestation_order,
        "discrepancies": report.discrepancies,
        "diagnostics": report.diagnostics,
        "unresolved_site_order": report.unresolved_site_order,
        "divergent_site_order": report.divergent_site_order,
        "negative_evidence": report.negative_evidence,
        "uncertainty": report.uncertainty,
        "disposition": report.disposition,
    })
}

impl FederatedReplayDiscrepancyReport {
    pub fn validate(&self) -> Result<(), FederatedReplayDiscrepancyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replay_group_id.trim().is_empty()
            || self.reference_site_id.trim().is_empty()
            || !canonical_strings(&self.attestation_order)
            || !canonical_strings(&self.unresolved_site_order)
            || !canonical_strings(&self.divergent_site_order)
            || !canonical_strings(&self.negative_evidence)
            || !canonical_strings(&self.uncertainty)
            || self
                .discrepancies
                .windows(2)
                .any(|pair| pair[0].site_id >= pair[1].site_id)
            || self.discrepancies.iter().any(|item| {
                item.site_id.trim().is_empty()
                    || item.reference_site_id != self.reference_site_id
                    || !canonical_strings(&item.observed_difference_fields)
                    || !canonical_strings(&item.missing_fields)
                    || !canonical_strings(&item.diagnostic_task_order)
                    || !canonical_strings(&item.reasons)
                    || item.confidence_milli > 1_000
                    || item
                        .divergence_stage
                        .as_ref()
                        .is_some_and(|stage| stage.trim().is_empty())
            })
            || self
                .diagnostics
                .windows(2)
                .any(|pair| pair[0].task_id >= pair[1].task_id)
            || self.diagnostics.iter().any(|task| {
                task.task_id.trim().is_empty()
                    || task.site_id.trim().is_empty()
                    || task.stage.trim().is_empty()
                    || task.action.trim().is_empty()
                    || task.reason.trim().is_empty()
                    || !task.local_only
                    || task.requires_raw_data
                    || task.dispatchable
            })
        {
            return Err(FederatedReplayDiscrepancyError::InvalidOutput(
                "report identity, canonical ordering, confidence, diagnostic locality, and evidence invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedReplayDiscrepancyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedReplayDiscrepancyError::InvalidOutput(
                "report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compare aggregate replay identities and produce bounded site-local diagnostics.
pub fn scan_glioma_federated_replay_discrepancy(
    request: &FederatedReplayDiscrepancyScanRequest,
) -> Result<FederatedReplayDiscrepancyReport, FederatedReplayDiscrepancyError> {
    validate_request(request)?;
    let mut attestations = request.attestations.clone();
    attestations.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let reference = attestations
        .iter()
        .find(|attestation| attestation.site_id == request.reference_site_id)
        .expect("validated reference attestation");
    let mut discrepancies = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut divergent = BTreeSet::new();
    let field_order = [
        "workflow_digest",
        "input_data_version_digest",
        "environment_digest",
        "dependency_lock_digest",
        "numeric_kernel_digest",
        "seed_digest",
        "output_digest",
    ];
    for attestation in &attestations {
        if attestation.site_id == request.reference_site_id {
            continue;
        }
        let mut missing = reference
            .missing_fields
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        missing.extend(attestation.missing_fields.iter().cloned());
        let mut differences = field_order
            .iter()
            .filter(|field| {
                !missing.contains(**field)
                    && attestation_field_digest(reference, field)
                        != attestation_field_digest(attestation, field)
            })
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        let stage_differences = request
            .required_stage_order
            .iter()
            .filter(|stage| {
                !missing.contains("stage_digests")
                    && reference.stage_digests.get(*stage) != attestation.stage_digests.get(*stage)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !stage_differences.is_empty() {
            differences.push("stage_digests".into());
        }
        differences.sort();
        let (kind, confidence, stage, reasons) = if !missing.is_empty() {
            unresolved.insert(attestation.site_id.clone());
            (
                ReplayDivergenceKind::InsufficientEvidence,
                0,
                None,
                vec![format!(
                    "missing-permitted-summary-fields:{}",
                    missing.iter().cloned().collect::<Vec<_>>().join(",")
                )],
            )
        } else if differences.is_empty() {
            (
                ReplayDivergenceKind::NoDivergence,
                1_000,
                None,
                vec!["all replay identities match reference".into()],
            )
        } else {
            divergent.insert(attestation.site_id.clone());
            let kind = if differences.len() > 1 {
                ReplayDivergenceKind::Multiple
            } else {
                match differences[0].as_str() {
                    "input_data_version_digest" => ReplayDivergenceKind::InputDataVersion,
                    "environment_digest" => ReplayDivergenceKind::Environment,
                    "dependency_lock_digest" => ReplayDivergenceKind::Dependency,
                    "numeric_kernel_digest" => ReplayDivergenceKind::NumericKernel,
                    "seed_digest" => ReplayDivergenceKind::Seed,
                    "workflow_digest" | "stage_digests" => ReplayDivergenceKind::Workflow,
                    "output_digest" => ReplayDivergenceKind::Output,
                    _ => ReplayDivergenceKind::Multiple,
                }
            };
            let stage = stage_differences.first().cloned().or_else(|| {
                Some(match kind {
                    ReplayDivergenceKind::Environment => "environment".into(),
                    ReplayDivergenceKind::Dependency => "dependency_lock".into(),
                    ReplayDivergenceKind::NumericKernel => "numeric_kernel".into(),
                    ReplayDivergenceKind::Seed => "seed".into(),
                    ReplayDivergenceKind::InputDataVersion => "input_data_version".into(),
                    ReplayDivergenceKind::Output => "output".into(),
                    ReplayDivergenceKind::Workflow => "workflow".into(),
                    _ => "replay_identity".into(),
                })
            });
            (
                kind,
                if kind == ReplayDivergenceKind::Multiple {
                    700
                } else {
                    950
                },
                stage,
                vec![format!("reference-differs-in:{}", differences.join(","))],
            )
        };
        let mut task_ids = Vec::new();
        if kind != ReplayDivergenceKind::NoDivergence {
            let diagnostic_stage = stage.clone().unwrap_or_else(|| "attestation".into());
            let (action, reason) = diagnostic_action(kind, &diagnostic_stage);
            let task_id = format!(
                "diagnose:{}:{}:{}",
                attestation.site_id, diagnostic_stage, action
            );
            task_ids.push(task_id.clone());
            if diagnostics.len() < request.max_diagnostics_per_site * request.attestations.len() {
                diagnostics.push(ReplayDiagnosticTask {
                    task_id,
                    site_id: attestation.site_id.clone(),
                    stage: diagnostic_stage,
                    action: action.into(),
                    reason: reason.into(),
                    local_only: true,
                    requires_raw_data: false,
                    dispatchable: false,
                });
            }
        }
        discrepancies.push(FederatedReplayDiscrepancy {
            site_id: attestation.site_id.clone(),
            reference_site_id: request.reference_site_id.clone(),
            divergence_kind: kind,
            observed_difference_fields: differences,
            divergence_stage: stage,
            confidence_milli: confidence,
            missing_fields: missing.into_iter().collect(),
            diagnostic_task_order: task_ids,
            reasons,
        });
    }
    discrepancies.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    diagnostics.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let unresolved_site_order = unresolved.into_iter().collect::<Vec<_>>();
    let divergent_site_order = divergent.into_iter().collect::<Vec<_>>();
    let all_conformant = discrepancies
        .iter()
        .all(|item| item.divergence_kind == ReplayDivergenceKind::NoDivergence);
    let disposition = if all_conformant {
        FederatedReplayDiscrepancyDisposition::Conformant
    } else if !unresolved_site_order.is_empty() && divergent_site_order.is_empty() {
        FederatedReplayDiscrepancyDisposition::Unresolved
    } else if !unresolved_site_order.is_empty() {
        FederatedReplayDiscrepancyDisposition::Partial
    } else if !divergent_site_order.is_empty() {
        FederatedReplayDiscrepancyDisposition::Divergent
    } else {
        FederatedReplayDiscrepancyDisposition::Blocked
    };
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !unresolved_site_order.is_empty() {
        negative_evidence
            .push("insufficient-permitted-summary-evidence-prevents-localization".into());
        uncertainty.push("missing-fields-remain-site-local-and-unresolved".into());
    }
    if !divergent_site_order.is_empty() {
        negative_evidence
            .push("replay-identity-divergence-requires-local-diagnostic-replay".into());
    }
    let mut report = FederatedReplayDiscrepancyReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replay_group_id: request.replay_group_id.clone(),
        reference_site_id: request.reference_site_id.clone(),
        attestation_order: attestations.into_iter().map(|item| item.site_id).collect(),
        discrepancies,
        diagnostics,
        unresolved_site_order,
        divergent_site_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-replay-discrepancy"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| FederatedReplayDiscrepancyError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn attestation(site_id: &str, environment: &str, seed: &str) -> FederatedReplayAttestation {
        FederatedReplayAttestation {
            site_id: site_id.into(),
            replay_group_id: "glioma-replay-1".into(),
            run_id: "run-1".into(),
            workflow_digest: hash("workflow"),
            input_data_version_digest: hash("inputs-v1"),
            environment_digest: hash(environment),
            dependency_lock_digest: hash("deps-v1"),
            numeric_kernel_digest: hash("kernel-v1"),
            seed_digest: hash(seed),
            output_digest: hash("output-v1"),
            stage_digests: [("normalize".into(), hash("normalize-v1"))]
                .into_iter()
                .collect(),
            missing_fields: Vec::new(),
            signer_id: format!("signer-{site_id}"),
            signature_digest: hash(&format!("sig-{site_id}")),
            signature_valid: true,
            permitted_summary_only: true,
            contains_raw_inputs: false,
            contains_human_data: false,
            contains_clinical_decision: false,
            generated_at_unix_seconds: 1,
        }
    }

    fn request(
        attestations: Vec<FederatedReplayAttestation>,
    ) -> FederatedReplayDiscrepancyScanRequest {
        FederatedReplayDiscrepancyScanRequest {
            objective: "localize glioma replay divergence".into(),
            replay_group_id: "glioma-replay-1".into(),
            reference_site_id: "site-a".into(),
            required_stage_order: vec!["normalize".into()],
            attestations,
            permitted_site_order: vec!["site-a".into(), "site-b".into()],
            federation_policy_digest: hash("policy-v1"),
            max_diagnostics_per_site: 4,
        }
    }

    #[test]
    fn localizes_environment_difference_without_raw_data() {
        let output = scan_glioma_federated_replay_discrepancy(&request(vec![
            attestation("site-a", "env-v1", "seed-v1"),
            attestation("site-b", "env-v2", "seed-v1"),
        ]))
        .expect("scan succeeds");
        assert_eq!(
            output.disposition,
            FederatedReplayDiscrepancyDisposition::Divergent
        );
        assert_eq!(
            output.discrepancies[0].divergence_kind,
            ReplayDivergenceKind::Environment
        );
        assert!(output
            .diagnostics
            .iter()
            .all(|task| task.local_only && !task.requires_raw_data));
        output.validate().expect("output validates");
    }

    #[test]
    fn localizes_seed_difference_and_is_permutation_stable() {
        let first = request(vec![
            attestation("site-a", "env-v1", "seed-v1"),
            attestation("site-b", "env-v1", "seed-v2"),
        ]);
        let mut reversed = first.clone();
        reversed.attestations.reverse();
        let left = scan_glioma_federated_replay_discrepancy(&first).expect("left");
        let right = scan_glioma_federated_replay_discrepancy(&reversed).expect("right");
        assert_eq!(left.digest, right.digest);
        assert_eq!(
            left.discrepancies[0].divergence_kind,
            ReplayDivergenceKind::Seed
        );
    }

    #[test]
    fn insufficient_summary_remains_unresolved() {
        let mut site_b = attestation("site-b", "env-v1", "seed-v1");
        site_b.missing_fields = vec!["output_digest".into()];
        let output = scan_glioma_federated_replay_discrepancy(&request(vec![
            attestation("site-a", "env-v1", "seed-v1"),
            site_b,
        ]))
        .expect("scan succeeds");
        assert_eq!(
            output.disposition,
            FederatedReplayDiscrepancyDisposition::Unresolved
        );
        assert_eq!(
            output.discrepancies[0].divergence_kind,
            ReplayDivergenceKind::InsufficientEvidence
        );
        assert!(output.discrepancies[0]
            .missing_fields
            .contains(&"output_digest".into()));
    }

    #[test]
    fn rejects_raw_or_clinical_payloads() {
        let mut site_b = attestation("site-b", "env-v1", "seed-v1");
        site_b.contains_raw_inputs = true;
        assert!(matches!(
            scan_glioma_federated_replay_discrepancy(&request(vec![
                attestation("site-a", "env-v1", "seed-v1"),
                site_b,
            ])),
            Err(FederatedReplayDiscrepancyError::InvalidRequest(_))
        ));
    }

    #[test]
    fn matching_replays_are_conformant() {
        let output = scan_glioma_federated_replay_discrepancy(&request(vec![
            attestation("site-a", "env-v1", "seed-v1"),
            attestation("site-b", "env-v1", "seed-v1"),
        ]))
        .expect("scan succeeds");
        assert_eq!(
            output.disposition,
            FederatedReplayDiscrepancyDisposition::Conformant
        );
        assert!(output.diagnostics.is_empty());
    }
}
