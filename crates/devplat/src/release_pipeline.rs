//! Deterministic audit of a release-pipeline declaration.
//!
//! The pipeline manifest is the artifact-level companion to the workbench's review-only CI plan.
//! It checks that stages form a closed DAG, artifacts have content digests and declared lineage,
//! attestations bind to the artifact bytes they name, promotions respect environment order, and
//! production transitions carry explicit protection, approval, provenance, signature, and rollback
//! declarations. It never runs a command, contacts a registry, verifies a cryptographic signature,
//! mutates a deployment, or claims that a named external runner performed the work.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const RELEASE_PIPELINE_MANIFEST_SCHEMA: &str = "bioprism-release-pipeline/0.1";
pub const RELEASE_PIPELINE_AUDIT_SCHEMA: &str = "bioprism-release-pipeline-audit/0.1";

const MAX_ENVIRONMENTS: usize = 256;
const MAX_STAGES: usize = 4_096;
const MAX_ARTIFACTS: usize = 8_192;
const MAX_ATTESTATIONS: usize = 16_384;
const MAX_PROMOTIONS: usize = 4_096;
const MAX_LIST: usize = 16_384;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasePipelineManifest {
    pub schema: String,
    pub project: PipelineProject,
    pub source: PipelineSource,
    #[serde(default)]
    pub environments: Vec<PipelineEnvironment>,
    #[serde(default)]
    pub stages: Vec<PipelineStage>,
    #[serde(default)]
    pub artifacts: Vec<PipelineArtifact>,
    #[serde(default)]
    pub attestations: Vec<PipelineAttestation>,
    #[serde(default)]
    pub promotions: Vec<PipelinePromotion>,
    #[serde(default)]
    pub policies: ReleasePipelinePolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineProject {
    pub id: String,
    pub version: String,
    pub repository: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineSource {
    pub ref_name: String,
    pub commit_digest: String,
    pub workflow: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentClass {
    Development,
    Staging,
    Production,
}

impl EnvironmentClass {
    fn rank(self) -> u8 {
        match self {
            Self::Development => 0,
            Self::Staging => 1,
            Self::Production => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineEnvironment {
    pub id: String,
    pub class: EnvironmentClass,
    #[serde(default)]
    pub protected: bool,
    #[serde(default)]
    pub required_approvals: usize,
    #[serde(default)]
    pub secrets_allowed: bool,
    #[serde(default)]
    pub immutable_artifacts: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStageKind {
    Verify,
    Build,
    Test,
    Package,
    Sign,
    Publish,
    Deploy,
    Smoke,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStage {
    pub id: String,
    pub kind: PipelineStageKind,
    pub environment: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub produces: Vec<String>,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PipelineArtifactKind {
    Source,
    Binary,
    Container,
    Package,
    Manifest,
    Sbom,
    Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineArtifact {
    pub id: String,
    pub kind: PipelineArtifactKind,
    pub digest: String,
    pub produced_by: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub attestations: Vec<String>,
    #[serde(default)]
    pub immutable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PipelineAttestationKind {
    Test,
    Provenance,
    Signature,
    Approval,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineAttestation {
    pub id: String,
    pub kind: PipelineAttestationKind,
    pub artifact: String,
    pub digest: String,
    pub issuer: String,
    pub statement: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PipelinePromotionKind {
    Advance,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelinePromotion {
    pub id: String,
    pub kind: PipelinePromotionKind,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub required_attestations: Vec<String>,
    #[serde(default)]
    pub approvals: Vec<String>,
    #[serde(default)]
    pub rollback_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasePipelinePolicies {
    #[serde(default = "default_true")]
    pub require_stage_dag: bool,
    #[serde(default = "default_true")]
    pub require_provenance: bool,
    #[serde(default = "default_true")]
    pub require_production_signature: bool,
    #[serde(default = "default_true")]
    pub require_protected_production: bool,
    #[serde(default = "default_true")]
    pub require_rollback: bool,
    #[serde(default = "default_true")]
    pub require_approval: bool,
}

impl Default for ReleasePipelinePolicies {
    fn default() -> Self {
        Self {
            require_stage_dag: true,
            require_provenance: true,
            require_production_signature: true,
            require_protected_production: true,
            require_rollback: true,
            require_approval: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasePipelineIssue {
    pub code: String,
    pub severity: PipelineIssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineIssueSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStageReadiness {
    pub stage_id: String,
    pub state: String,
    pub dependency_ready: bool,
    pub blocking_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineArtifactAudit {
    pub artifact_id: String,
    pub digest_valid: bool,
    pub producer_valid: bool,
    pub inputs_valid: bool,
    pub attestations_valid: bool,
    pub provenance_present: bool,
    pub signature_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelinePromotionAudit {
    pub promotion_id: String,
    pub from: String,
    pub to: String,
    pub valid: bool,
    pub production: bool,
    pub missing_attestations: Vec<String>,
    pub missing_approvals: Vec<String>,
    pub rollback_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasePipelineCounts {
    pub environments: usize,
    pub protected_environments: usize,
    pub stages: usize,
    pub required_stages: usize,
    pub artifacts: usize,
    pub attestations: usize,
    pub promotions: usize,
    pub production_promotions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasePipelineAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub digest: String,
    pub valid: bool,
    pub counts: ReleasePipelineCounts,
    pub stage_order: Vec<String>,
    pub cyclic_stages: Vec<Vec<String>>,
    pub stage_readiness: Vec<PipelineStageReadiness>,
    pub artifact_audits: Vec<PipelineArtifactAudit>,
    pub promotion_audits: Vec<PipelinePromotionAudit>,
    pub issues: Vec<ReleasePipelineIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ReleasePipelineError {
    #[error("cannot canonicalize release pipeline manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize release pipeline manifest: {0}")]
    Serialization(String),
}

impl ReleasePipelineManifest {
    pub fn digest(&self) -> Result<ContentHash, ReleasePipelineError> {
        let value = serde_json::to_value(self)
            .map_err(|error| ReleasePipelineError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<ReleasePipelineAudit, ReleasePipelineError> {
        let digest = self.digest()?.to_string();
        let mut issues = Vec::new();
        let mut environments = BTreeMap::<String, &PipelineEnvironment>::new();
        let mut stages = BTreeMap::<String, &PipelineStage>::new();
        let mut artifacts = BTreeMap::<String, &PipelineArtifact>::new();
        let mut attestations = BTreeMap::<String, &PipelineAttestation>::new();
        let mut promotions = BTreeMap::<String, &PipelinePromotion>::new();

        bound(&mut issues, "environments", self.environments.len(), MAX_ENVIRONMENTS);
        bound(&mut issues, "stages", self.stages.len(), MAX_STAGES);
        bound(&mut issues, "artifacts", self.artifacts.len(), MAX_ARTIFACTS);
        bound(&mut issues, "attestations", self.attestations.len(), MAX_ATTESTATIONS);
        bound(&mut issues, "promotions", self.promotions.len(), MAX_PROMOTIONS);
        if self.schema != RELEASE_PIPELINE_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!("expected {RELEASE_PIPELINE_MANIFEST_SCHEMA}, got {}", self.schema),
                "regenerate the manifest with the published release-pipeline schema",
            );
        }
        for (field, value) in [
            ("project.id", &self.project.id),
            ("project.version", &self.project.version),
            ("project.repository", &self.project.repository),
            ("source.ref_name", &self.source.ref_name),
            ("source.commit_digest", &self.source.commit_digest),
            ("source.workflow", &self.source.workflow),
        ] {
            if value.trim().is_empty() {
                blocking(
                    &mut issues,
                    "required_field_empty",
                    field,
                    format!("{field} is empty"),
                    "supply the pinned project or source identity",
                );
            }
        }
        if !valid_digest(&self.source.commit_digest) {
            blocking(
                &mut issues,
                "invalid_source_digest",
                "source.commit_digest",
                "the source commit digest must be 64 hexadecimal characters",
                "pin the pipeline to a canonical commit digest",
            );
        }

        for environment in &self.environments {
            if !insert_unique(&mut environments, &environment.id, "environment", &mut issues) {
                continue;
            }
            environments.insert(environment.id.clone(), environment);
            if environment.id.trim().is_empty() {
                blocking(
                    &mut issues,
                    "environment_id_empty",
                    "environment",
                    "environment id is empty",
                    "name each deployment environment",
                );
            }
            if environment.class == EnvironmentClass::Production
                && self.policies.require_protected_production
                && !environment.protected
            {
                blocking(
                    &mut issues,
                    "production_not_protected",
                    &environment.id,
                    "production environment is not marked protected",
                    "require an external protection boundary before production promotion",
                );
            }
            if environment.class == EnvironmentClass::Production
                && environment.required_approvals == 0
                && self.policies.require_approval
            {
                blocking(
                    &mut issues,
                    "production_approval_floor_missing",
                    &environment.id,
                    "production environment requires no approvals",
                    "declare a positive approval floor for production",
                );
            }
            if environment.class == EnvironmentClass::Production
                && !environment.immutable_artifacts
            {
                warning(
                    &mut issues,
                    "production_artifacts_mutable",
                    &environment.id,
                    "production does not declare immutable artifacts",
                    "prefer digest-addressed immutable artifacts for promotion",
                );
            }
        }

        for stage in &self.stages {
            if !insert_unique(&mut stages, &stage.id, "stage", &mut issues) {
                continue;
            }
            stages.insert(stage.id.clone(), stage);
            if stage.id.trim().is_empty() || stage.environment.trim().is_empty() {
                blocking(
                    &mut issues,
                    "stage_identity_incomplete",
                    &stage.id,
                    "stage id and environment are required",
                    "name the stage and bind it to an environment",
                );
            }
            if !environments.contains_key(&stage.environment) {
                blocking(
                    &mut issues,
                    "stage_environment_missing",
                    &stage.id,
                    format!("stage names undeclared environment {}", stage.environment),
                    "declare the environment before using it",
                );
            }
            if let Some(command) = &stage.command {
                if command.trim().is_empty() {
                    blocking(
                        &mut issues,
                        "stage_command_empty",
                        &stage.id,
                        "a declared stage command is empty",
                        "supply a command or omit it for a provider-owned step",
                    );
                }
            }
            if stage.produces.len() > MAX_LIST {
                bound(&mut issues, "stage.produces", stage.produces.len(), MAX_LIST);
            }
        }

        let graph = self.stage_graph(&stages, &mut issues);
        let (stage_order, cyclic_stages) = topo_order(&graph);
        if self.policies.require_stage_dag && !cyclic_stages.is_empty() {
            for cycle in &cyclic_stages {
                blocking(
                    &mut issues,
                    "stage_cycle",
                    cycle.join(" -> "),
                    "stage dependencies contain a cycle",
                    "break the cycle with a one-way artifact or promotion boundary",
                );
            }
        }
        let stage_readiness = self
            .stages
            .iter()
            .map(|stage| {
                let blocking_dependencies = stage
                    .depends_on
                    .iter()
                    .filter(|dependency| !stages.contains_key(*dependency))
                    .cloned()
                    .collect::<Vec<_>>();
                PipelineStageReadiness {
                    stage_id: stage.id.clone(),
                    state: if !blocking_dependencies.is_empty() {
                        "blocked".into()
                    } else if cyclic_stages.iter().any(|cycle| cycle.contains(&stage.id)) {
                        "cyclic".into()
                    } else {
                        "ready_to_schedule".into()
                    },
                    dependency_ready: blocking_dependencies.is_empty(),
                    blocking_dependencies,
                }
            })
            .collect::<Vec<_>>();

        for artifact in &self.artifacts {
            if !insert_unique(&mut artifacts, &artifact.id, "artifact", &mut issues) {
                continue;
            }
            artifacts.insert(artifact.id.clone(), artifact);
            if artifact.id.trim().is_empty() || artifact.produced_by.trim().is_empty() {
                blocking(
                    &mut issues,
                    "artifact_identity_incomplete",
                    &artifact.id,
                    "artifact id and producing stage are required",
                    "name an artifact and the stage that produced it",
                );
            }
            if !valid_digest(&artifact.digest) {
                blocking(
                    &mut issues,
                    "invalid_artifact_digest",
                    &artifact.id,
                    "artifact digest must be 64 hexadecimal characters",
                    "record the canonical digest of the produced bytes",
                );
            }
            if !stages.contains_key(&artifact.produced_by) {
                blocking(
                    &mut issues,
                    "artifact_producer_missing",
                    &artifact.id,
                    format!("producer stage {} is undeclared", artifact.produced_by),
                    "declare the producing stage or correct the artifact binding",
                );
            }
        }
        for stage in &self.stages {
            for produced in &stage.produces {
                if !artifacts.contains_key(produced) {
                    blocking(
                        &mut issues,
                        "stage_output_missing",
                        &stage.id,
                        format!("stage output artifact {produced} is undeclared"),
                        "declare the artifact and bind its producer to this stage",
                    );
                }
            }
        }
        for artifact in &self.artifacts {
            for input in &artifact.inputs {
                if input == &artifact.id {
                    blocking(
                        &mut issues,
                        "artifact_self_input",
                        &artifact.id,
                        "an artifact cannot list itself as an input",
                        "remove the self-edge from artifact lineage",
                    );
                } else if !artifacts.contains_key(input) {
                    blocking(
                        &mut issues,
                        "artifact_input_missing",
                        &artifact.id,
                        format!("input artifact {input} is undeclared"),
                        "declare the input artifact before deriving this one",
                    );
                }
            }
        }

        for attestation in &self.attestations {
            if !insert_unique(&mut attestations, &attestation.id, "attestation", &mut issues) {
                continue;
            }
            attestations.insert(attestation.id.clone(), attestation);
            if attestation.id.trim().is_empty()
                || attestation.issuer.trim().is_empty()
                || attestation.statement.trim().is_empty()
            {
                blocking(
                    &mut issues,
                    "attestation_incomplete",
                    &attestation.id,
                    "attestation id, issuer, and statement are required",
                    "retain a human-auditable issuer and statement",
                );
            }
            if !valid_digest(&attestation.digest) {
                blocking(
                    &mut issues,
                    "invalid_attestation_digest",
                    &attestation.id,
                    "attestation digest must be 64 hexadecimal characters",
                    "bind the attestation to the artifact digest",
                );
            }
            if !artifacts.contains_key(&attestation.artifact) {
                blocking(
                    &mut issues,
                    "attestation_artifact_missing",
                    &attestation.id,
                    format!("attestation names undeclared artifact {}", attestation.artifact),
                    "attach the attestation to a declared artifact",
                );
            }
        }
        for attestation in &self.attestations {
            if let Some(artifact) = artifacts.get(&attestation.artifact) {
                if artifact.digest != attestation.digest {
                    blocking(
                        &mut issues,
                        "attestation_digest_mismatch",
                        &attestation.id,
                        "attestation digest does not equal its artifact digest",
                        "reissue the attestation for the exact artifact bytes",
                    );
                }
            }
        }

        for promotion in &self.promotions {
            if !insert_unique(&mut promotions, &promotion.id, "promotion", &mut issues) {
                continue;
            }
            promotions.insert(promotion.id.clone(), promotion);
            self.validate_promotion_shape(promotion, &environments, &artifacts, &attestations, &mut issues);
        }
        for promotion in &self.promotions {
            if let Some(target) = &promotion.rollback_target {
                match promotions.get(target) {
                    Some(candidate) if candidate.kind == PipelinePromotionKind::Rollback => {}
                    Some(_) => blocking(
                        &mut issues,
                        "rollback_target_not_rollback",
                        &promotion.id,
                        format!("rollback target {target} is not a rollback promotion"),
                        "point the production advance at a rollback transition",
                    ),
                    None => blocking(
                        &mut issues,
                        "rollback_target_missing",
                        &promotion.id,
                        format!("rollback target {target} is undeclared"),
                        "declare the rollback promotion before referencing it",
                    ),
                }
            }
        }

        let artifact_audits = self
            .artifacts
            .iter()
            .map(|artifact| {
                let artifact_attestations = artifact
                    .attestations
                    .iter()
                    .filter_map(|id| attestations.get(id).copied())
                    .collect::<Vec<_>>();
                let provenance_present = artifact_attestations
                    .iter()
                    .any(|item| item.kind == PipelineAttestationKind::Provenance);
                let signature_present = artifact_attestations
                    .iter()
                    .any(|item| item.kind == PipelineAttestationKind::Signature);
                let attestations_valid = artifact.attestations.iter().all(|id| {
                    attestations
                        .get(id)
                        .map(|item| item.artifact == artifact.id && item.digest == artifact.digest)
                        .unwrap_or(false)
                });
                if self.policies.require_provenance && !provenance_present {
                    blocking(
                        &mut issues,
                        "artifact_provenance_missing",
                        &artifact.id,
                        "artifact has no provenance attestation",
                        "attach an attestation that names how the artifact was produced",
                    );
                }
                if !attestations_valid {
                    blocking(
                        &mut issues,
                        "artifact_attestation_invalid",
                        &artifact.id,
                        "one or more artifact attestation references are absent or mismatched",
                        "attach only attestations bound to this artifact digest",
                    );
                }
                PipelineArtifactAudit {
                    artifact_id: artifact.id.clone(),
                    digest_valid: valid_digest(&artifact.digest),
                    producer_valid: stages.contains_key(&artifact.produced_by),
                    inputs_valid: artifact.inputs.iter().all(|id| artifacts.contains_key(id)),
                    attestations_valid,
                    provenance_present,
                    signature_present,
                }
            })
            .collect::<Vec<_>>();

        let promotion_audits = self
            .promotions
            .iter()
            .map(|promotion| {
                let Some(to) = environments.get(&promotion.to) else {
                    return PipelinePromotionAudit {
                        promotion_id: promotion.id.clone(),
                        from: promotion.from.clone(),
                        to: promotion.to.clone(),
                        valid: false,
                        production: false,
                        missing_attestations: promotion.required_attestations.clone(),
                        missing_approvals: promotion.approvals.clone(),
                        rollback_present: promotion.rollback_target.is_some(),
                    };
                };
                let production = to.class == EnvironmentClass::Production;
                let missing_attestations = promotion
                    .required_attestations
                    .iter()
                    .filter(|id| {
                        attestations
                            .get(*id)
                            .map(|attestation| {
                                !promotion.artifacts.contains(&attestation.artifact)
                                    || !valid_digest(&attestation.digest)
                            })
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let approval_count = promotion
                    .approvals
                    .iter()
                    .filter(|id| {
                        attestations
                            .get(*id)
                            .map(|attestation| attestation.kind == PipelineAttestationKind::Approval)
                            .unwrap_or(false)
                    })
                    .count();
                let required_approvals = if production { to.required_approvals } else { 0 };
                let missing_approvals = if approval_count >= required_approvals {
                    Vec::new()
                } else {
                    vec![format!("need {required_approvals} approvals, found {approval_count}")]
                };
                let signature_ok = !production
                    || !self.policies.require_production_signature
                    || promotion.artifacts.iter().all(|artifact_id| {
                        artifact_audits
                            .iter()
                            .find(|row| &row.artifact_id == artifact_id)
                            .map(|row| row.signature_present)
                            .unwrap_or(false)
                    });
                if production && !signature_ok {
                    blocking(
                        &mut issues,
                        "production_signature_missing",
                        &promotion.id,
                        "every artifact promoted to production needs a signature attestation",
                        "attach a digest-matching signature before production promotion",
                    );
                }
                if !missing_attestations.is_empty() {
                    blocking(
                        &mut issues,
                        "promotion_attestation_missing",
                        &promotion.id,
                        format!("required attestations missing or do not bind to promoted artifacts: {missing_attestations:?}"),
                        "bind every required attestation to an artifact in this promotion",
                    );
                }
                if !missing_approvals.is_empty() && self.policies.require_approval && production {
                    blocking(
                        &mut issues,
                        "promotion_approval_missing",
                        &promotion.id,
                        missing_approvals.join(", "),
                        "record the required external approvals before promotion",
                    );
                }
                if production && self.policies.require_rollback && promotion.rollback_target.is_none() {
                    blocking(
                        &mut issues,
                        "production_rollback_missing",
                        &promotion.id,
                        "production advance has no rollback target",
                        "declare a tested rollback promotion before advancing production",
                    );
                }
                PipelinePromotionAudit {
                    promotion_id: promotion.id.clone(),
                    from: promotion.from.clone(),
                    to: promotion.to.clone(),
                    valid: missing_attestations.is_empty() && missing_approvals.is_empty() && signature_ok,
                    production,
                    missing_attestations,
                    missing_approvals,
                    rollback_present: promotion.rollback_target.is_some(),
                }
            })
            .collect::<Vec<_>>();

        let counts = ReleasePipelineCounts {
            environments: self.environments.len(),
            protected_environments: self.environments.iter().filter(|item| item.protected).count(),
            stages: self.stages.len(),
            required_stages: self.stages.iter().filter(|item| item.required).count(),
            artifacts: self.artifacts.len(),
            attestations: self.attestations.len(),
            promotions: self.promotions.len(),
            production_promotions: self
                .promotions
                .iter()
                .filter(|item| environments.get(&item.to).map(|env| env.class == EnvironmentClass::Production).unwrap_or(false))
                .count(),
        };
        let valid = !issues.iter().any(|issue| issue.severity == PipelineIssueSeverity::Blocking);
        Ok(ReleasePipelineAudit {
            schema: RELEASE_PIPELINE_AUDIT_SCHEMA.into(),
            manifest_schema: self.schema.clone(),
            digest,
            valid,
            counts,
            stage_order,
            cyclic_stages,
            stage_readiness,
            artifact_audits,
            promotion_audits,
            issues,
            guarantees: vec![
                "the digest binds the canonical pipeline declaration, not an external runner".into(),
                "stage dependencies, artifact lineage, attestations, promotions, and rollback are checked as separate layers".into(),
                "production protection, approval, provenance, signature, and rollback policies remain explicit".into(),
            ],
            limitations: vec![
                "the audit does not execute commands, contact CI, verify signatures, or query a registry".into(),
                "attestation issuer identity and cryptographic validity are caller-declared".into(),
                "a valid manifest is a coherent release plan, not evidence of a successful deployment or approval".into(),
            ],
        })
    }

    fn stage_graph(
        &self,
        stages: &BTreeMap<String, &PipelineStage>,
        issues: &mut Vec<ReleasePipelineIssue>,
    ) -> BTreeMap<String, Vec<String>> {
        let mut graph = BTreeMap::new();
        for stage in &self.stages {
            let mut dependencies = stage.depends_on.clone();
            dependencies.sort();
            dependencies.dedup();
            for dependency in &dependencies {
                if dependency == &stage.id {
                    blocking(
                        issues,
                        "stage_self_dependency",
                        &stage.id,
                        "a stage cannot depend on itself",
                        "remove the self-edge or split the stage boundary",
                    );
                } else if !stages.contains_key(dependency) {
                    blocking(
                        issues,
                        "stage_dependency_missing",
                        &stage.id,
                        format!("dependency {dependency} is undeclared"),
                        "declare the dependency stage or remove the edge",
                    );
                }
            }
            graph.insert(stage.id.clone(), dependencies);
        }
        graph
    }

    fn validate_promotion_shape(
        &self,
        promotion: &PipelinePromotion,
        environments: &BTreeMap<String, &PipelineEnvironment>,
        artifacts: &BTreeMap<String, &PipelineArtifact>,
        attestations: &BTreeMap<String, &PipelineAttestation>,
        issues: &mut Vec<ReleasePipelineIssue>,
    ) {
        let Some(from) = environments.get(&promotion.from) else {
            blocking(issues, "promotion_source_missing", &promotion.id, format!("source environment {} is undeclared", promotion.from), "declare the source environment");
            return;
        };
        let Some(to) = environments.get(&promotion.to) else {
            blocking(issues, "promotion_target_missing", &promotion.id, format!("target environment {} is undeclared", promotion.to), "declare the target environment");
            return;
        };
        if promotion.from == promotion.to {
            blocking(issues, "promotion_same_environment", &promotion.id, "source and target environments are identical", "name a real promotion boundary");
        }
        match promotion.kind {
            PipelinePromotionKind::Advance if from.class.rank() >= to.class.rank() => blocking(issues, "advance_order_invalid", &promotion.id, "an advance must move to a higher environment class", "use a forward environment transition"),
            PipelinePromotionKind::Rollback if from.class.rank() <= to.class.rank() => blocking(issues, "rollback_order_invalid", &promotion.id, "a rollback must move to a lower environment class", "target a lower environment class"),
            _ => {}
        }
        if promotion.artifacts.is_empty() {
            blocking(issues, "promotion_artifacts_missing", &promotion.id, "promotion names no artifacts", "promote explicit immutable artifact identifiers");
        }
        for artifact in &promotion.artifacts {
            if !artifacts.contains_key(artifact) {
                blocking(issues, "promotion_artifact_missing", &promotion.id, format!("artifact {artifact} is undeclared"), "declare the artifact before promotion");
            }
        }
        for attestation in &promotion.required_attestations {
            if !attestations.contains_key(attestation) {
                blocking(issues, "promotion_attestation_reference_missing", &promotion.id, format!("attestation {attestation} is undeclared"), "declare the attestation before requiring it");
            }
        }
        for approval in &promotion.approvals {
            if !attestations.get(approval).map(|item| item.kind == PipelineAttestationKind::Approval).unwrap_or(false) {
                blocking(issues, "promotion_approval_reference_invalid", &promotion.id, format!("approval {approval} is not a declared approval attestation"), "reference an approval attestation, not an arbitrary string");
            }
        }
        if to.class == EnvironmentClass::Production && self.policies.require_protected_production && !to.protected {
            blocking(issues, "promotion_target_unprotected", &promotion.id, "production target is not protected", "protect the production environment");
        }
    }
}

fn insert_unique<'a, T>(
    map: &mut BTreeMap<String, &'a T>,
    id: &str,
    kind: &'static str,
    issues: &mut Vec<ReleasePipelineIssue>,
) -> bool {
    if map.contains_key(id) {
        blocking(issues, &format!("duplicate_{kind}_id"), id, format!("{kind} identifier occurs more than once"), format!("assign one stable identifier to exactly one {kind}"));
        false
    } else {
        true
    }
}

fn bound(issues: &mut Vec<ReleasePipelineIssue>, subject: &str, count: usize, maximum: usize) {
    if count > maximum {
        blocking(issues, "input_bound_exceeded", subject, format!("{count} entries exceed maximum {maximum}"), "split the manifest or reduce the declared surface");
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn blocking(issues: &mut Vec<ReleasePipelineIssue>, code: &str, subject: impl Into<String>, detail: impl Into<String>, remediation: impl Into<String>) {
    issues.push(ReleasePipelineIssue { code: code.into(), severity: PipelineIssueSeverity::Blocking, subject: subject.into(), detail: detail.into(), remediation: remediation.into() });
}

fn warning(issues: &mut Vec<ReleasePipelineIssue>, code: &str, subject: impl Into<String>, detail: impl Into<String>, remediation: impl Into<String>) {
    issues.push(ReleasePipelineIssue { code: code.into(), severity: PipelineIssueSeverity::Warning, subject: subject.into(), detail: detail.into(), remediation: remediation.into() });
}

fn topo_order(graph: &BTreeMap<String, Vec<String>>) -> (Vec<String>, Vec<Vec<String>>) {
    let mut incoming = graph.keys().map(|key| (key.clone(), 0usize)).collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for (node, dependencies) in graph {
        for dependency in dependencies {
            if graph.contains_key(dependency) {
                *incoming.entry(node.clone()).or_default() += 1;
                outgoing.entry(dependency.clone()).or_default().push(node.clone());
            }
        }
    }
    for values in outgoing.values_mut() { values.sort(); }
    let mut ready = incoming.iter().filter(|(_, degree)| **degree == 0).map(|(node, _)| node.clone()).collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    while let Some(node) = ready.pop_first() {
        order.push(node.clone());
        for dependent in outgoing.get(&node).into_iter().flatten() {
            let degree = incoming.get_mut(dependent).expect("outgoing target exists");
            *degree -= 1;
            if *degree == 0 { ready.insert(dependent.clone()); }
        }
    }
    let remaining = incoming.iter().filter(|(_, degree)| **degree > 0).map(|(node, _)| node.clone()).collect::<BTreeSet<_>>();
    if remaining.is_empty() { return (order, Vec::new()); }
    let mut undirected = BTreeMap::<String, BTreeSet<String>>::new();
    for node in &remaining {
        for dependency in graph.get(node).into_iter().flatten() {
            if remaining.contains(dependency) {
                undirected.entry(node.clone()).or_default().insert(dependency.clone());
                undirected.entry(dependency.clone()).or_default().insert(node.clone());
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut cycles = Vec::new();
    for node in &remaining {
        if !seen.insert(node.clone()) { continue; }
        let mut stack = vec![node.clone()];
        let mut component = Vec::new();
        while let Some(current) = stack.pop() {
            component.push(current.clone());
            for neighbor in undirected.get(&current).into_iter().flatten() {
                if seen.insert(neighbor.clone()) { stack.push(neighbor.clone()); }
            }
        }
        component.sort();
        cycles.push(component);
    }
    (order, cycles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ReleasePipelineManifest {
        let digest = "a".repeat(64);
        ReleasePipelineManifest {
            schema: RELEASE_PIPELINE_MANIFEST_SCHEMA.into(),
            project: PipelineProject { id: "aurora-agent".into(), version: "0.1.0".into(), repository: "github.com/AURORA-NEURO/aurora-agent".into() },
            source: PipelineSource { ref_name: "main".into(), commit_digest: digest.clone(), workflow: "release.yml".into() },
            environments: vec![
                PipelineEnvironment { id: "staging".into(), class: EnvironmentClass::Staging, protected: true, required_approvals: 0, secrets_allowed: true, immutable_artifacts: true },
                PipelineEnvironment { id: "production".into(), class: EnvironmentClass::Production, protected: true, required_approvals: 1, secrets_allowed: true, immutable_artifacts: true },
            ],
            stages: vec![
                PipelineStage { id: "build".into(), kind: PipelineStageKind::Build, environment: "staging".into(), depends_on: vec![], command: Some("cargo build --locked".into()), produces: vec!["binary".into()], required: true },
                PipelineStage { id: "test".into(), kind: PipelineStageKind::Test, environment: "staging".into(), depends_on: vec!["build".into()], command: Some("cargo test --locked".into()), produces: vec![], required: true },
            ],
            artifacts: vec![PipelineArtifact { id: "binary".into(), kind: PipelineArtifactKind::Binary, digest: digest.clone(), produced_by: "build".into(), inputs: vec![], attestations: vec!["prov".into(), "sig".into()], immutable: true }],
            attestations: vec![
                PipelineAttestation { id: "prov".into(), kind: PipelineAttestationKind::Provenance, artifact: "binary".into(), digest: digest.clone(), issuer: "ci".into(), statement: "built from pinned source".into() },
                PipelineAttestation { id: "sig".into(), kind: PipelineAttestationKind::Signature, artifact: "binary".into(), digest: digest.clone(), issuer: "release-key".into(), statement: "signed artifact".into() },
                PipelineAttestation { id: "approval".into(), kind: PipelineAttestationKind::Approval, artifact: "binary".into(), digest, issuer: "release-board".into(), statement: "approved".into() },
            ],
            promotions: vec![PipelinePromotion { id: "to-production".into(), kind: PipelinePromotionKind::Advance, from: "staging".into(), to: "production".into(), artifacts: vec!["binary".into()], required_attestations: vec!["prov".into(), "sig".into()], approvals: vec!["approval".into()], rollback_target: Some("rollback".into()) }, PipelinePromotion { id: "rollback".into(), kind: PipelinePromotionKind::Rollback, from: "production".into(), to: "staging".into(), artifacts: vec!["binary".into()], required_attestations: vec!["prov".into()], approvals: vec![], rollback_target: None }],
            policies: ReleasePipelinePolicies::default(),
        }
    }

    #[test]
    fn valid_pipeline_has_ordered_stages_and_release_evidence() {
        let report = manifest().audit().unwrap();
        assert!(report.valid);
        assert_eq!(report.stage_order, vec!["build", "test"]);
        assert_eq!(report.counts.production_promotions, 1);
        assert!(report.promotion_audits[0].rollback_present);
    }

    #[test]
    fn a_cycle_and_unbound_attestation_are_blocking() {
        let mut value = manifest();
        value.stages[0].depends_on = vec!["test".into()];
        value.promotions[0].required_attestations = vec!["missing".into()];
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "stage_cycle"));
        assert!(report.issues.iter().any(|issue| issue.code == "promotion_attestation_reference_missing"));
    }

    #[test]
    fn production_signature_digest_mismatch_is_not_a_release_pass() {
        let mut value = manifest();
        value.attestations[1].digest = "b".repeat(64);
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "attestation_digest_mismatch"));
    }

    #[test]
    fn artifact_lineage_and_stage_outputs_are_closed_independent_of_declaration_order() {
        let mut value = manifest();
        value.policies.require_provenance = false;
        value.stages[0].produces.push("derived".into());
        let derived = PipelineArtifact {
            id: "derived".into(),
            kind: PipelineArtifactKind::Package,
            digest: "c".repeat(64),
            produced_by: "build".into(),
            inputs: vec!["binary".into()],
            attestations: vec![],
            immutable: true,
        };
        let binary = value.artifacts.remove(0);
        value.artifacts = vec![derived, binary];
        let report = value.audit().unwrap();
        assert!(report.valid);
        assert!(report.artifact_audits.iter().all(|audit| audit.inputs_valid));
        assert!(!report.issues.iter().any(|issue| issue.code == "stage_output_missing"));
    }
}
