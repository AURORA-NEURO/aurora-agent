//! Federated validated-workflow template exchange for preclinical glioma research.
//!
//! This feature shares a workflow's executable contract and aggregate conformance evidence,
//! never its local inputs, credentials, or raw outputs. A template becomes portable only when
//! independent permitted sites pass the same pinned contract; a failed or missing conformance
//! record remains visible and prevents a portability claim.

use super::execution_environment_lock::{
    ComputeEnvironmentLock, ComputeEnvironmentLockDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedWorkflowTemplate1@1";
pub const MAX_ITEMS: usize = 256;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowTemplateManifest {
    pub template_id: String,
    pub version: String,
    pub workflow_manifest_digest: ContentHash,
    pub task_order: Vec<String>,
    pub input_schema_order: Vec<String>,
    pub output_schema_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub deterministic: bool,
    pub local_only: bool,
    pub source_site_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowValidationCard {
    pub card_id: String,
    pub template_digest: ContentHash,
    pub benchmark_digest: ContentHash,
    pub metric_order: Vec<String>,
    pub required_gate_order: Vec<String>,
    pub held_out_run_count: u32,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSiteAttestation {
    pub site_id: String,
    pub template_digest: ContentHash,
    pub environment_build_identity: ContentHash,
    pub conformant: bool,
    pub failed_check_order: Vec<String>,
    pub aggregate_only: bool,
    pub signer_id: String,
    pub signature_digest: ContentHash,
    pub attestation_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSharingPolicy {
    pub allowed_site_order: Vec<String>,
    pub revoked_site_order: Vec<String>,
    pub allow_adaptations: bool,
    pub allow_environment_metadata: bool,
    pub allow_aggregate_attestations: bool,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowTemplateExchangeRequest {
    pub manifest: FederatedWorkflowTemplateManifest,
    pub environment_lock: ComputeEnvironmentLock,
    pub validation_card: WorkflowValidationCard,
    pub sharing_policy: WorkflowSharingPolicy,
    pub attestations: Vec<WorkflowSiteAttestation>,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSiteConformance {
    Conformant,
    Failed,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSiteCoverage {
    pub site_id: String,
    pub conformance: WorkflowSiteConformance,
    pub failed_check_order: Vec<String>,
    pub evidence_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTemplateAdaptation {
    pub adaptation_id: String,
    pub field: String,
    pub permitted_value_order: Vec<String>,
    pub requires_revalidation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedWorkflowTemplateDisposition {
    Published,
    LocalOnly,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowTemplatePackage {
    pub feature_id: String,
    pub output_schema: String,
    pub template_id: String,
    pub version: String,
    pub template_digest: ContentHash,
    pub workflow_manifest_digest: ContentHash,
    pub environment_build_identity: Option<ContentHash>,
    pub task_order: Vec<String>,
    pub input_schema_order: Vec<String>,
    pub output_schema_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub allowed_adaptations: Vec<WorkflowTemplateAdaptation>,
    pub site_coverage: Vec<WorkflowSiteCoverage>,
    pub rejected_site_order: Vec<String>,
    pub portability_claim: bool,
    pub compatibility_warning_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedWorkflowTemplateDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedWorkflowTemplateExchangeError {
    #[error("federated workflow template request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated workflow template package is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated workflow template digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn digest_input(package: &FederatedWorkflowTemplatePackage) -> serde_json::Value {
    serde_json::json!({
        "feature_id": package.feature_id,
        "output_schema": package.output_schema,
        "template_id": package.template_id,
        "version": package.version,
        "template_digest": package.template_digest,
        "workflow_manifest_digest": package.workflow_manifest_digest,
        "environment_build_identity": package.environment_build_identity,
        "task_order": package.task_order,
        "input_schema_order": package.input_schema_order,
        "output_schema_order": package.output_schema_order,
        "effect_order": package.effect_order,
        "allowed_adaptations": package.allowed_adaptations,
        "site_coverage": package.site_coverage,
        "rejected_site_order": package.rejected_site_order,
        "portability_claim": package.portability_claim,
        "compatibility_warning_order": package.compatibility_warning_order,
        "negative_evidence": package.negative_evidence,
        "uncertainty": package.uncertainty,
        "disposition": package.disposition,
    })
}

fn template_identity(
    manifest: &FederatedWorkflowTemplateManifest,
    environment_lock: &ComputeEnvironmentLock,
    validation_card: &WorkflowValidationCard,
) -> Result<ContentHash, FederatedWorkflowTemplateExchangeError> {
    ContentHash::of_value(&serde_json::json!({
        "manifest": manifest,
        "environment_build_identity": environment_lock.build_identity,
        "validation_card_id": validation_card.card_id,
        "benchmark_digest": validation_card.benchmark_digest,
    }))
    .map_err(|error| FederatedWorkflowTemplateExchangeError::Digest(error.to_string()))
}

fn attestation_body(attestation: &WorkflowSiteAttestation) -> serde_json::Value {
    serde_json::json!({
        "site_id": attestation.site_id,
        "template_digest": attestation.template_digest,
        "environment_build_identity": attestation.environment_build_identity,
        "conformant": attestation.conformant,
        "failed_check_order": attestation.failed_check_order,
        "aggregate_only": attestation.aggregate_only,
        "signer_id": attestation.signer_id,
    })
}

fn validate_manifest(
    manifest: &FederatedWorkflowTemplateManifest,
) -> Result<(), FederatedWorkflowTemplateExchangeError> {
    if manifest.template_id.trim().is_empty()
        || manifest.template_id.len() > MAX_TEXT_LEN
        || manifest.version.trim().is_empty()
        || manifest.version.len() > MAX_TEXT_LEN
        || !valid_hash(&manifest.workflow_manifest_digest)
        || !unique_bounded(&manifest.task_order, MAX_ITEMS)
        || !canonical(&manifest.task_order)
        || !unique_bounded(&manifest.input_schema_order, MAX_ITEMS)
        || !canonical(&manifest.input_schema_order)
        || !unique_bounded(&manifest.output_schema_order, MAX_ITEMS)
        || !canonical(&manifest.output_schema_order)
        || !unique_bounded(&manifest.effect_order, MAX_ITEMS)
        || !canonical(&manifest.effect_order)
        || !manifest.deterministic
        || !manifest.local_only
        || manifest.source_site_id.trim().is_empty()
        || manifest.source_site_id.len() > MAX_TEXT_LEN
        || manifest.effect_order.iter().any(|effect| {
            matches!(
                effect.as_str(),
                "external_effect"
                    | "raw_data_export"
                    | "clinical_decision"
                    | "human_subject_action"
            )
        })
    {
        return Err(FederatedWorkflowTemplateExchangeError::InvalidRequest(
            "templates require bounded canonical deterministic local-only contracts without external or clinical effects".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &FederatedWorkflowTemplateExchangeRequest,
) -> Result<ContentHash, FederatedWorkflowTemplateExchangeError> {
    validate_manifest(&request.manifest)?;
    request.environment_lock.validate().map_err(|error| {
        FederatedWorkflowTemplateExchangeError::InvalidRequest(error.to_string())
    })?;
    if request.environment_lock.disposition != ComputeEnvironmentLockDisposition::Qualified
        || request.environment_lock.workflow_manifest_digest
            != request.manifest.workflow_manifest_digest
        || request.validation_card.card_id.trim().is_empty()
        || request.validation_card.card_id.len() > MAX_TEXT_LEN
        || !valid_hash(&request.validation_card.template_digest)
        || !valid_hash(&request.validation_card.benchmark_digest)
        || !unique_bounded(&request.validation_card.metric_order, MAX_ITEMS)
        || !canonical(&request.validation_card.metric_order)
        || !unique_bounded(&request.validation_card.required_gate_order, MAX_ITEMS)
        || !canonical(&request.validation_card.required_gate_order)
        || request.validation_card.held_out_run_count == 0
        || request.validation_card.template_digest
            != request.environment_lock.workflow_manifest_digest
        || request.current_tick > request.sharing_policy.expires_at_tick
        || request.sharing_policy.expires_at_tick == 0
        || !unique_bounded(&request.sharing_policy.allowed_site_order, MAX_ITEMS)
        || !canonical(&request.sharing_policy.allowed_site_order)
        || !unique_bounded(&request.sharing_policy.revoked_site_order, MAX_ITEMS)
        || !canonical(&request.sharing_policy.revoked_site_order)
        || !request.sharing_policy.allow_aggregate_attestations
        || request.attestations.len() > MAX_ITEMS
    {
        return Err(FederatedWorkflowTemplateExchangeError::InvalidRequest(
            "qualified environment, validation card, non-expired aggregate policy, and canonical site scopes are required".into(),
        ));
    }
    let template_digest = template_identity(
        &request.manifest,
        &request.environment_lock,
        &request.validation_card,
    )?;
    for attestation in &request.attestations {
        if attestation.site_id.trim().is_empty()
            || attestation.site_id.len() > MAX_TEXT_LEN
            || !valid_hash(&attestation.template_digest)
            || !valid_hash(&attestation.environment_build_identity)
            || !unique_bounded(&attestation.failed_check_order, MAX_ITEMS)
            || !canonical(&attestation.failed_check_order)
            || attestation.signer_id.trim().is_empty()
            || attestation.signer_id.len() > MAX_TEXT_LEN
            || !valid_hash(&attestation.signature_digest)
            || !valid_hash(&attestation.attestation_digest)
            || attestation.template_digest != template_digest
            || attestation.environment_build_identity != request.environment_lock.build_identity
        {
            return Err(FederatedWorkflowTemplateExchangeError::InvalidRequest(
                "site attestations must bind to the exact template and qualified environment"
                    .into(),
            ));
        }
        let expected_body = ContentHash::of_value(&attestation_body(attestation))
            .map_err(|error| FederatedWorkflowTemplateExchangeError::Digest(error.to_string()))?;
        if expected_body != attestation.attestation_digest
            || attestation.signature_digest != expected_body
        {
            return Err(FederatedWorkflowTemplateExchangeError::InvalidRequest(
                "site attestation signature and content digest do not verify".into(),
            ));
        }
    }
    Ok(template_digest)
}

impl FederatedWorkflowTemplatePackage {
    pub fn validate(&self) -> Result<(), FederatedWorkflowTemplateExchangeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.template_id.trim().is_empty()
            || self.version.trim().is_empty()
            || !valid_hash(&self.template_digest)
            || !valid_hash(&self.workflow_manifest_digest)
            || !unique_bounded(&self.task_order, MAX_ITEMS)
            || !canonical(&self.task_order)
            || !unique_bounded(&self.input_schema_order, MAX_ITEMS)
            || !canonical(&self.input_schema_order)
            || !unique_bounded(&self.output_schema_order, MAX_ITEMS)
            || !canonical(&self.output_schema_order)
            || !unique_bounded(&self.effect_order, MAX_ITEMS)
            || !canonical(&self.effect_order)
            || !unique_bounded(&self.rejected_site_order, MAX_ITEMS)
            || !canonical(&self.rejected_site_order)
            || !unique_bounded(&self.compatibility_warning_order, MAX_ITEMS)
            || !canonical(&self.compatibility_warning_order)
            || !unique_bounded(&self.negative_evidence, MAX_ITEMS)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty, MAX_ITEMS)
            || !canonical(&self.uncertainty)
            || self.site_coverage.len() > MAX_ITEMS
            || self
                .site_coverage
                .windows(2)
                .any(|pair| pair[0].site_id >= pair[1].site_id)
            || self
                .allowed_adaptations
                .windows(2)
                .any(|pair| pair[0].adaptation_id >= pair[1].adaptation_id)
            || self.allowed_adaptations.iter().any(|adaptation| {
                adaptation.adaptation_id.trim().is_empty()
                    || adaptation.field.trim().is_empty()
                    || !unique_bounded(&adaptation.permitted_value_order, MAX_ITEMS)
                    || !canonical(&adaptation.permitted_value_order)
            })
            || !valid_hash(&self.digest)
        {
            return Err(FederatedWorkflowTemplateExchangeError::InvalidOutput(
                "template identity, canonical contract lists, site coverage, adaptations, and digest are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedWorkflowTemplateExchangeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedWorkflowTemplateExchangeError::InvalidOutput(
                "workflow template package digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Exchange a validated workflow contract and aggregate site evidence without exporting raw data.
pub fn exchange_glioma_federated_workflow_template(
    request: &FederatedWorkflowTemplateExchangeRequest,
) -> Result<FederatedWorkflowTemplatePackage, FederatedWorkflowTemplateExchangeError> {
    let template_digest = validate_request(request)?;
    let allowed = request
        .sharing_policy
        .allowed_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let revoked = request
        .sharing_policy
        .revoked_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut seen_sites = BTreeSet::new();
    let mut site_coverage = Vec::new();
    let mut rejected_sites = BTreeSet::new();
    for attestation in &request.attestations {
        if !seen_sites.insert(attestation.site_id.clone()) {
            return Err(FederatedWorkflowTemplateExchangeError::InvalidRequest(
                "each site may contribute at most one attestation".into(),
            ));
        }
        if !allowed.contains(&attestation.site_id) || revoked.contains(&attestation.site_id) {
            rejected_sites.insert(attestation.site_id.clone());
            site_coverage.push(WorkflowSiteCoverage {
                site_id: attestation.site_id.clone(),
                conformance: WorkflowSiteConformance::Rejected,
                failed_check_order: vec!["site-not-permitted-by-sharing-policy".into()],
                evidence_digest: attestation.attestation_digest.clone(),
            });
            continue;
        }
        let conformance = if attestation.conformant {
            WorkflowSiteConformance::Conformant
        } else {
            WorkflowSiteConformance::Failed
        };
        if !attestation.conformant {
            rejected_sites.insert(attestation.site_id.clone());
        }
        site_coverage.push(WorkflowSiteCoverage {
            site_id: attestation.site_id.clone(),
            conformance,
            failed_check_order: attestation.failed_check_order.clone(),
            evidence_digest: attestation.attestation_digest.clone(),
        });
    }
    site_coverage.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let conformant_count = site_coverage
        .iter()
        .filter(|site| site.conformance == WorkflowSiteConformance::Conformant)
        .count();
    let failed_count = site_coverage
        .iter()
        .filter(|site| site.conformance == WorkflowSiteConformance::Failed)
        .count();
    let portability_claim = request.validation_card.passed
        && failed_count == 0
        && conformant_count >= 2
        && !site_coverage.is_empty();
    let mut warnings = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if site_coverage.is_empty() {
        warnings.push("no-permitted-site-conformance-evidence".into());
        uncertainty.push("portability-is-unresolved-without-independent-site-evidence".into());
    }
    if failed_count > 0 {
        warnings.push("failed-site-conformance-blocks-portability-claim".into());
        negative_evidence.push("at-least-one-permitted-site-failed-conformance".into());
    }
    if conformant_count < 2 {
        warnings.push("fewer-than-two-conformant-sites-for-federated-portability".into());
    }
    if !request.sharing_policy.allow_environment_metadata {
        warnings.push("environment-metadata-withheld-by-sharing-policy".into());
    }
    let mut adaptations = Vec::new();
    if portability_claim && request.sharing_policy.allow_adaptations {
        adaptations.push(WorkflowTemplateAdaptation {
            adaptation_id: "site-local-resource-binding".into(),
            field: "resource_binding".into(),
            permitted_value_order: vec!["cpu_only".into(), "site_local_accelerator".into()],
            requires_revalidation: true,
        });
        adaptations.push(WorkflowTemplateAdaptation {
            adaptation_id: "site-local-storage-binding".into(),
            field: "storage_binding".into(),
            permitted_value_order: vec!["institution_local".into()],
            requires_revalidation: true,
        });
    }
    let environment_build_identity = request
        .sharing_policy
        .allow_environment_metadata
        .then(|| request.environment_lock.build_identity.clone());
    let disposition = if portability_claim {
        FederatedWorkflowTemplateDisposition::Published
    } else if request.validation_card.passed && failed_count == 0 {
        FederatedWorkflowTemplateDisposition::LocalOnly
    } else if failed_count > 0 {
        FederatedWorkflowTemplateDisposition::Blocked
    } else {
        FederatedWorkflowTemplateDisposition::Unresolved
    };
    warnings.sort();
    negative_evidence.sort();
    uncertainty.sort();
    let mut package = FederatedWorkflowTemplatePackage {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        template_id: request.manifest.template_id.clone(),
        version: request.manifest.version.clone(),
        template_digest,
        workflow_manifest_digest: request.manifest.workflow_manifest_digest.clone(),
        environment_build_identity,
        task_order: request.manifest.task_order.clone(),
        input_schema_order: request.manifest.input_schema_order.clone(),
        output_schema_order: request.manifest.output_schema_order.clone(),
        effect_order: request.manifest.effect_order.clone(),
        allowed_adaptations: adaptations,
        site_coverage,
        rejected_site_order: rejected_sites.into_iter().collect(),
        portability_claim,
        compatibility_warning_order: warnings,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-template"),
    };
    package.digest = ContentHash::of_value(&digest_input(&package))
        .map_err(|error| FederatedWorkflowTemplateExchangeError::Digest(error.to_string()))?;
    package.validate()?;
    Ok(package)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::execution_environment_lock::{
        lock_glioma_compute_environment, ComputeEnvironmentDependency,
        ComputeEnvironmentLockRequest, EnvironmentArchitectureProfile, EnvironmentDependencyKind,
    };

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn lock() -> ComputeEnvironmentLock {
        lock_glioma_compute_environment(&ComputeEnvironmentLockRequest {
            objective: "template-test".into(),
            workflow_manifest_digest: hash("workflow"),
            workflow_task_order: vec!["normalize".into()],
            architecture: EnvironmentArchitectureProfile {
                os_family: "linux".into(),
                os_version: "6.8".into(),
                architecture: "x86_64".into(),
                abi: "gnu".into(),
                cpu_feature_order: vec!["avx2".into()],
                accelerator_order: vec!["none".into()],
                host_digest: hash("host"),
            },
            dependencies: vec![ComputeEnvironmentDependency {
                name: "numpy".into(),
                kind: EnvironmentDependencyKind::Library,
                version_constraint: "=2.1.0".into(),
                resolved_version: "2.1.0".into(),
                source: "registry://trusted".into(),
                source_digest: hash("source"),
                build_digest: hash("build"),
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
            }],
            trusted_source_order: vec!["registry://trusted".into()],
            require_signed_metadata: true,
            require_portable_dependencies: true,
        })
        .expect("lock")
    }

    fn request(
        attestations: Vec<WorkflowSiteAttestation>,
    ) -> FederatedWorkflowTemplateExchangeRequest {
        let environment_lock = lock();
        let manifest = FederatedWorkflowTemplateManifest {
            template_id: "glioma-normalize".into(),
            version: "1.0.0".into(),
            workflow_manifest_digest: hash("workflow"),
            task_order: vec!["normalize".into()],
            input_schema_order: vec!["image-stack".into()],
            output_schema_order: vec!["normalized-image-stack".into()],
            effect_order: vec![
                "execute_local_computation".into(),
                "read_local_artifact".into(),
            ],
            deterministic: true,
            local_only: true,
            source_site_id: "site-a".into(),
        };
        FederatedWorkflowTemplateExchangeRequest {
            manifest,
            environment_lock,
            validation_card: WorkflowValidationCard {
                card_id: "card-1".into(),
                template_digest: hash("workflow"),
                benchmark_digest: hash("benchmark"),
                metric_order: vec!["exact_replay_rate".into()],
                required_gate_order: vec!["held_out_replay".into()],
                held_out_run_count: 2,
                passed: true,
            },
            sharing_policy: WorkflowSharingPolicy {
                allowed_site_order: vec!["site-a".into(), "site-b".into()],
                revoked_site_order: vec![],
                allow_adaptations: true,
                allow_environment_metadata: true,
                allow_aggregate_attestations: true,
                expires_at_tick: 100,
            },
            attestations,
            current_tick: 1,
        }
    }

    fn attestation(
        site_id: &str,
        template_digest: ContentHash,
        conformant: bool,
    ) -> WorkflowSiteAttestation {
        let mut attestation = WorkflowSiteAttestation {
            site_id: site_id.into(),
            template_digest,
            environment_build_identity: lock().build_identity,
            conformant,
            failed_check_order: if conformant {
                vec![]
            } else {
                vec!["metric-drift".into()]
            },
            aggregate_only: true,
            signer_id: format!("signer-{site_id}"),
            signature_digest: hash("placeholder"),
            attestation_digest: hash("placeholder"),
        };
        let digest =
            ContentHash::of_value(&attestation_body(&attestation)).expect("attestation digest");
        attestation.signature_digest = digest.clone();
        attestation.attestation_digest = digest;
        attestation
    }

    #[test]
    fn two_conformant_sites_publish_portable_template() {
        let mut req = request(vec![]);
        let identity =
            template_identity(&req.manifest, &req.environment_lock, &req.validation_card)
                .expect("identity");
        req.attestations = vec![
            attestation("site-a", identity.clone(), true),
            attestation("site-b", identity, true),
        ];
        let package = exchange_glioma_federated_workflow_template(&req).expect("package");
        assert_eq!(
            package.disposition,
            FederatedWorkflowTemplateDisposition::Published
        );
        assert!(package.portability_claim);
        assert_eq!(package.allowed_adaptations.len(), 2);
    }

    #[test]
    fn failed_site_conformance_blocks_portability_claim() {
        let mut req = request(vec![]);
        let identity =
            template_identity(&req.manifest, &req.environment_lock, &req.validation_card)
                .expect("identity");
        req.attestations = vec![
            attestation("site-a", identity.clone(), true),
            attestation("site-b", identity, false),
        ];
        let package = exchange_glioma_federated_workflow_template(&req).expect("package");
        assert_eq!(
            package.disposition,
            FederatedWorkflowTemplateDisposition::Blocked
        );
        assert!(!package.portability_claim);
        assert!(package
            .compatibility_warning_order
            .iter()
            .any(|warning| warning.contains("failed-site")));
    }

    #[test]
    fn no_site_evidence_remains_local_only_and_uncertain() {
        let package =
            exchange_glioma_federated_workflow_template(&request(vec![])).expect("package");
        assert_eq!(
            package.disposition,
            FederatedWorkflowTemplateDisposition::LocalOnly
        );
        assert!(!package.portability_claim);
        assert!(package
            .uncertainty
            .iter()
            .any(|item| item.contains("independent-site")));
    }

    #[test]
    fn forbidden_effects_fail_closed() {
        let mut req = request(vec![]);
        req.manifest.effect_order = vec!["clinical_decision".into()];
        assert!(matches!(
            exchange_glioma_federated_workflow_template(&req),
            Err(FederatedWorkflowTemplateExchangeError::InvalidRequest(_))
        ));
    }
}
