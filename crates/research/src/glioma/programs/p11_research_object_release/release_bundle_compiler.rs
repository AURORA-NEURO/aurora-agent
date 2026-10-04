//! Dependency-closed offline reproducibility-bundle compilation for preclinical glioma results.
//!
//! The compiler creates a deterministic bundle plan from already-validated local metadata. It
//! performs no file copy or network fetch: every member must be supplied with a digest, safe path,
//! dependency list, and explicit export permission. Missing or excluded local inputs remain in a
//! replay boundary so a clean-room consumer cannot mistake an incomplete bundle for a complete
//! result.

use super::license_scope_checker::ReleaseShareabilityDecision;
use crate::glioma::release::ResearchObjectManifest;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaReproducibilityBundle1@1";
pub const MAX_MEMBERS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleMember {
    pub artifact_id: String,
    pub relative_path: String,
    pub content_hash: ContentHash,
    pub content_type: String,
    pub dependency_order: Vec<String>,
    pub export_permitted: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityBundleRequest {
    pub manifest: ResearchObjectManifest,
    pub shareability: ReleaseShareabilityDecision,
    pub members: Vec<BundleMember>,
    pub workflow_digest: ContentHash,
    pub environment_digest: ContentHash,
    pub replay_instruction_order: Vec<String>,
    pub target_profile: String,
    pub max_members: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleOmission {
    pub artifact_id: String,
    pub reason: String,
    pub replay_boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilityBundleDisposition {
    Complete,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilityBundle {
    pub feature_id: String,
    pub output_schema: String,
    pub manifest_digest: ContentHash,
    pub shareability_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub environment_digest: ContentHash,
    pub target_profile: String,
    pub member_order: Vec<String>,
    pub omitted: Vec<BundleOmission>,
    pub missing_dependency_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub replay_instruction_order: Vec<String>,
    pub offline_guarantee: bool,
    pub network_requirements: Vec<String>,
    pub limitations: Vec<String>,
    pub disposition: ReproducibilityBundleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseBundleError {
    #[error("release bundle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release bundle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release bundle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn valid_path(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 1024
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

fn digest_input(bundle: &ReproducibilityBundle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": bundle.feature_id,
        "output_schema": bundle.output_schema,
        "manifest_digest": bundle.manifest_digest,
        "shareability_digest": bundle.shareability_digest,
        "workflow_digest": bundle.workflow_digest,
        "environment_digest": bundle.environment_digest,
        "target_profile": bundle.target_profile,
        "member_order": bundle.member_order,
        "omitted": bundle.omitted,
        "missing_dependency_order": bundle.missing_dependency_order,
        "cycle_order": bundle.cycle_order,
        "replay_instruction_order": bundle.replay_instruction_order,
        "offline_guarantee": bundle.offline_guarantee,
        "network_requirements": bundle.network_requirements,
        "limitations": bundle.limitations,
        "disposition": bundle.disposition,
    })
}

impl ReproducibilityBundle {
    pub fn validate(&self) -> Result<(), ReleaseBundleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || [
                &self.manifest_digest,
                &self.shareability_digest,
                &self.workflow_digest,
                &self.environment_digest,
            ]
            .iter()
            .any(|digest| digest.as_str().len() != 64)
            || self.target_profile.trim().is_empty()
            || !canonical(&self.member_order)
            || !canonical(&self.missing_dependency_order)
            || !canonical(&self.cycle_order)
            || !canonical(&self.replay_instruction_order)
            || !canonical(&self.network_requirements)
            || !canonical(&self.limitations)
            || self
                .omitted
                .windows(2)
                .any(|pair| pair[0].artifact_id >= pair[1].artifact_id)
        {
            return Err(ReleaseBundleError::InvalidOutput(
                "bundle identity, digests, ordering, or profile invariants are invalid".into(),
            ));
        }
        if self.offline_guarantee && !self.network_requirements.is_empty() {
            return Err(ReleaseBundleError::InvalidOutput(
                "offline bundles cannot declare network requirements".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseBundleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReleaseBundleError::InvalidOutput(
                "bundle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ReproducibilityBundleRequest) -> Result<(), ReleaseBundleError> {
    request
        .manifest
        .validate()
        .map_err(|error| ReleaseBundleError::InvalidRequest(error.to_string()))?;
    request
        .shareability
        .validate()
        .map_err(|error| ReleaseBundleError::InvalidRequest(error.to_string()))?;
    if request.workflow_digest.as_str().len() != 64
        || request.environment_digest.as_str().len() != 64
        || request.target_profile.trim().is_empty()
        || request.max_members == 0
        || request.max_members > MAX_MEMBERS
        || request.members.is_empty()
        || request.members.len() > request.max_members
        || !canonical(&request.replay_instruction_order)
        || request
            .replay_instruction_order
            .iter()
            .any(|instruction| instruction.trim().is_empty())
    {
        return Err(ReleaseBundleError::InvalidRequest(
            "validated manifest/shareability, digests, bounded members, profile, and replay instructions are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for member in &request.members {
        if !valid_identifier(&member.artifact_id)
            || !ids.insert(member.artifact_id.clone())
            || !valid_path(&member.relative_path)
            || member.content_hash.as_str().len() != 64
            || member.content_type.trim().is_empty()
            || member
                .dependency_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || member
                .dependency_order
                .iter()
                .any(|id| !valid_identifier(id))
        {
            return Err(ReleaseBundleError::InvalidRequest(
                "bundle member identity, safe path, digest, format, and canonical dependencies are required".into(),
            ));
        }
    }
    Ok(())
}

struct BundleGraphWalk<'a> {
    members: &'a BTreeMap<String, &'a BundleMember>,
    allowed_fields: &'a BTreeSet<String>,
    included: BTreeSet<String>,
    omissions: Vec<BundleOmission>,
    missing: BTreeSet<String>,
    cycles: BTreeSet<String>,
    visiting: BTreeSet<String>,
    visited: BTreeSet<String>,
}

impl BundleGraphWalk<'_> {
    fn walk(&mut self, id: &str) {
        if self.visiting.contains(id) {
            self.cycles.insert(id.to_owned());
            return;
        }
        if !self.visited.insert(id.to_owned()) {
            return;
        }
        let Some(member) = self.members.get(id) else {
            self.missing.insert(id.to_owned());
            return;
        };
        let permitted =
            member.export_permitted && !member.local_only && self.allowed_fields.contains(id);
        let dependencies = member.dependency_order.clone();
        if !permitted {
            self.omissions.push(BundleOmission {
                artifact_id: id.to_owned(),
                reason: "shareability-or-locality-policy-excludes-member".into(),
                replay_boundary:
                    "reproduce-at-originating-institution-or-use-an-approved-aggregate".into(),
            });
            return;
        }
        self.visiting.insert(id.to_owned());
        for dependency in &dependencies {
            self.walk(dependency);
        }
        self.visiting.remove(id);
        self.included.insert(id.to_owned());
    }
}

/// Compile a deterministic offline bundle plan from local, already-authorized metadata.
pub fn compile_glioma_reproducibility_bundle(
    request: &ReproducibilityBundleRequest,
) -> Result<ReproducibilityBundle, ReleaseBundleError> {
    validate_request(request)?;
    let members = request
        .members
        .iter()
        .map(|member| (member.artifact_id.clone(), member))
        .collect::<BTreeMap<_, _>>();
    let allowed_fields = request
        .shareability
        .allow_order
        .iter()
        .filter_map(|id| id.split_once(':').map(|(artifact, _)| artifact.to_owned()))
        .collect::<BTreeSet<_>>();
    let mut graph = BundleGraphWalk {
        members: &members,
        allowed_fields: &allowed_fields,
        included: BTreeSet::new(),
        omissions: Vec::new(),
        missing: BTreeSet::new(),
        cycles: BTreeSet::new(),
        visiting: BTreeSet::new(),
        visited: BTreeSet::new(),
    };
    let mut network_requirements = BTreeSet::new();
    let mut limitations = request
        .manifest
        .limitations
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    for root in &request.manifest.artifact_order {
        graph.walk(root);
    }
    let BundleGraphWalk {
        included,
        mut omissions,
        missing,
        cycles,
        ..
    } = graph;
    for member in request.members.iter().filter(|member| member.local_only) {
        network_requirements.insert(format!("local-input:{}", member.artifact_id));
    }
    if !missing.is_empty() {
        limitations.insert("bundle-has-missing-dependencies".into());
    }
    if !cycles.is_empty() {
        limitations.insert("bundle-has-dependency-cycle".into());
    }
    omissions.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let disposition = if !missing.is_empty() || !cycles.is_empty() {
        ReproducibilityBundleDisposition::Blocked
    } else if !omissions.is_empty() {
        ReproducibilityBundleDisposition::Partial
    } else {
        ReproducibilityBundleDisposition::Complete
    };
    let mut output = ReproducibilityBundle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        manifest_digest: request.manifest.manifest_digest.clone(),
        shareability_digest: request.shareability.digest.clone(),
        workflow_digest: request.workflow_digest.clone(),
        environment_digest: request.environment_digest.clone(),
        target_profile: request.target_profile.clone(),
        member_order: included.into_iter().collect(),
        omitted: omissions,
        missing_dependency_order: missing.into_iter().collect(),
        cycle_order: cycles.into_iter().collect(),
        replay_instruction_order: request.replay_instruction_order.clone(),
        offline_guarantee: network_requirements.is_empty(),
        network_requirements: network_requirements.into_iter().collect(),
        limitations: limitations.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-reproducibility-bundle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReleaseBundleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::license_scope_checker::{
        evaluate_glioma_release_shareability, FieldClassification, LicenseDependency,
        LicenseScopePolicy, ShareableField,
    };
    use crate::glioma::release::{build_research_object_manifest, ResearchObjectRequest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> ReproducibilityBundleRequest {
        let manifest = build_research_object_manifest(&ResearchObjectRequest {
            research_id: "bundle-research".into(),
            study_id: "bundle-study".into(),
            objective: "offline reproducibility".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "root".into(),
                content_hash: hash("root"),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec!["null".into()],
            limitations: vec!["preclinical".into()],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap();
        let field = |id: &str| ShareableField {
            field_id: id.into(),
            classification: FieldClassification::AggregateResult,
            requested_export: true,
            source_digest: hash(id),
        };
        let shareability = evaluate_glioma_release_shareability(&crate::glioma::programs::p11_research_object_release::license_scope_checker::ReleaseShareabilityRequest {
            candidate_manifest_digest: hash("manifest"), root_order: vec!["root".into()], dependencies: vec![LicenseDependency { artifact_id: "root".into(), dependency_order: vec!["upstream".into()], license_id: Some("MIT".into()), fields: vec![field("summary")], local_only: false, embargo_until_epoch: None, rights_confirmed: true, contains_human_data: false, intended_audience: "consortium".into() }, LicenseDependency { artifact_id: "upstream".into(), dependency_order: Vec::new(), license_id: Some("MIT".into()), fields: vec![field("methods")], local_only: false, embargo_until_epoch: None, rights_confirmed: true, contains_human_data: false, intended_audience: "consortium".into() }], policy: LicenseScopePolicy { allowed_license_order: vec!["MIT".into()], forbidden_license_order: Vec::new(), audience: "consortium".into(), now_epoch: 1, permit_aggregate_export: true, permit_local_only_export: false, permit_human_data: false },
        }).unwrap();
        ReproducibilityBundleRequest {
            manifest,
            shareability,
            members: vec![
                BundleMember {
                    artifact_id: "root".into(),
                    relative_path: "root.json".into(),
                    content_hash: hash("root"),
                    content_type: "application/json".into(),
                    dependency_order: vec!["upstream".into()],
                    export_permitted: true,
                    local_only: false,
                },
                BundleMember {
                    artifact_id: "upstream".into(),
                    relative_path: "upstream.json".into(),
                    content_hash: hash("upstream"),
                    content_type: "application/json".into(),
                    dependency_order: Vec::new(),
                    export_permitted: true,
                    local_only: false,
                },
            ],
            workflow_digest: hash("workflow"),
            environment_digest: hash("environment"),
            replay_instruction_order: vec!["run-workflow".into()],
            target_profile: "offline-linux-x86_64".into(),
            max_members: 8,
        }
    }

    #[test]
    fn dependency_closed_bundle_is_complete_and_deterministic() {
        let left = compile_glioma_reproducibility_bundle(&request()).unwrap();
        let right = compile_glioma_reproducibility_bundle(&request()).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.disposition, ReproducibilityBundleDisposition::Complete);
        assert!(left.offline_guarantee);
    }

    #[test]
    fn excluded_local_member_creates_explicit_partial_boundary() {
        let mut request = request();
        request.members[1].local_only = true;
        let output = compile_glioma_reproducibility_bundle(&request).unwrap();
        assert_eq!(
            output.disposition,
            ReproducibilityBundleDisposition::Partial
        );
        assert!(output
            .omitted
            .iter()
            .any(|item| item.artifact_id == "upstream"));
        assert!(output
            .network_requirements
            .iter()
            .any(|item| item.contains("upstream")));
    }

    #[test]
    fn missing_dependency_and_cycle_are_blocked() {
        let mut missing_request = request();
        missing_request.members[0].dependency_order = vec!["missing".into()];
        let output = compile_glioma_reproducibility_bundle(&missing_request).unwrap();
        assert_eq!(
            output.disposition,
            ReproducibilityBundleDisposition::Blocked
        );
        assert_eq!(output.missing_dependency_order, vec!["missing"]);
        let mut cycle = request();
        cycle.members[1].dependency_order = vec!["root".into()];
        let output = compile_glioma_reproducibility_bundle(&cycle).unwrap();
        assert_eq!(
            output.disposition,
            ReproducibilityBundleDisposition::Blocked
        );
        assert!(!output.cycle_order.is_empty());
    }

    #[test]
    fn unsafe_paths_are_rejected_before_bundle_planning() {
        let mut request = request();
        request.members[0].relative_path = "../root.json".into();
        assert!(matches!(
            compile_glioma_reproducibility_bundle(&request),
            Err(ReleaseBundleError::InvalidRequest(_))
        ));
    }

    #[test]
    fn post_compile_mutation_breaks_bundle_digest() {
        let mut output = compile_glioma_reproducibility_bundle(&request()).unwrap();
        output.target_profile = "mutated".into();
        assert!(matches!(
            output.validate(),
            Err(ReleaseBundleError::InvalidOutput(_))
        ));
    }
}
