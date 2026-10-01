//! Expected-cohort reconciliation for local P11-F02 disclosure registers.
//!
//! The panel preserves each study's register digest and category counts. It keeps missing
//! expected studies visible, never pools disclosure counts, and treats independence groups as
//! caller declarations rather than authenticated site identities.

use super::disclosure_register::{
    ReleaseDisclosureCounts, ReleaseDisclosureRegister, ReleaseDisclosureRegisterError,
};
use crate::glioma::release::ResearchObjectManifest;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::leakage_audit::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseDisclosurePanel1@1";
const MIN_STUDIES: usize = 2;
const MAX_STUDIES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedDisclosureStudy {
    pub study_id: String,
    pub independence_group: String,
    pub manifest_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosurePanelRequest {
    pub research_id: String,
    pub expected_studies: Vec<ExpectedDisclosureStudy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureStudyInput {
    pub manifest: ResearchObjectManifest,
    pub register: ReleaseDisclosureRegister,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisclosurePanelStudyStatus {
    Registered,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosurePanelStudy {
    pub study_id: String,
    pub independence_group: String,
    pub manifest_digest: ContentHash,
    pub status: DisclosurePanelStudyStatus,
    pub register_digest: Option<ContentHash>,
    pub disclosure_counts: Option<ReleaseDisclosureCounts>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosurePanelCounts {
    pub expected_studies: usize,
    pub registered_studies: usize,
    pub missing_studies: usize,
    pub expected_independence_groups: usize,
    pub registered_independence_groups: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseDisclosurePanelDisposition {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosurePanel {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub research_id: String,
    pub request_digest: ContentHash,
    pub studies: Vec<DisclosurePanelStudy>,
    pub counts: ReleaseDisclosurePanelCounts,
    pub disposition: ReleaseDisclosurePanelDisposition,
    pub independence_authenticated: bool,
    pub scientific_truth_verified: bool,
    pub release_authorized: bool,
    pub disclosure_text_included: bool,
    pub statement_digests_are_unkeyed: bool,
    pub panel_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseDisclosurePanelError {
    #[error("release disclosure panel request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release disclosure study is invalid: {0}")]
    InvalidStudy(String),
    #[error("release disclosure panel output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release disclosure panel digest failed: {0}")]
    Digest(String),
}

fn validate_request(
    request: &ReleaseDisclosurePanelRequest,
) -> Result<Vec<ExpectedDisclosureStudy>, ReleaseDisclosurePanelError> {
    if request.research_id.trim().is_empty()
        || request.research_id.len() > 256
        || !(MIN_STUDIES..=MAX_STUDIES).contains(&request.expected_studies.len())
    {
        return Err(ReleaseDisclosurePanelError::InvalidRequest(
            "research identity and 2 to 128 expected studies are required".into(),
        ));
    }

    let mut study_ids = BTreeSet::new();
    let mut manifest_digests = BTreeSet::new();
    let mut groups = BTreeSet::new();
    for expected in &request.expected_studies {
        if expected.study_id.trim().is_empty()
            || expected.study_id.len() > 128
            || expected.independence_group.trim().is_empty()
            || expected.independence_group.len() > 128
            || expected.manifest_digest.as_str().len() != 64
            || !study_ids.insert(expected.study_id.as_str())
            || !manifest_digests.insert(expected.manifest_digest.as_str())
        {
            return Err(ReleaseDisclosurePanelError::InvalidRequest(
                "expected study IDs and manifest digests must be valid and unique".into(),
            ));
        }
        groups.insert(expected.independence_group.as_str());
    }
    if groups.len() < MIN_STUDIES {
        return Err(ReleaseDisclosurePanelError::InvalidRequest(
            "the expected cohort must declare at least two independence groups".into(),
        ));
    }

    let mut expected = request.expected_studies.clone();
    expected.sort_by(|left, right| {
        left.study_id
            .cmp(&right.study_id)
            .then_with(|| left.manifest_digest.cmp(&right.manifest_digest))
    });
    Ok(expected)
}

fn request_digest(
    request: &ReleaseDisclosurePanelRequest,
    expected: &[ExpectedDisclosureStudy],
) -> Result<ContentHash, ReleaseDisclosurePanelError> {
    ContentHash::of_value(&serde_json::json!({
        "research_id": request.research_id,
        "expected_studies": expected,
    }))
    .map_err(|error| ReleaseDisclosurePanelError::Digest(error.to_string()))
}

fn counts(studies: &[DisclosurePanelStudy]) -> ReleaseDisclosurePanelCounts {
    let registered = studies
        .iter()
        .filter(|study| study.status == DisclosurePanelStudyStatus::Registered)
        .collect::<Vec<_>>();
    ReleaseDisclosurePanelCounts {
        expected_studies: studies.len(),
        registered_studies: registered.len(),
        missing_studies: studies.len().saturating_sub(registered.len()),
        expected_independence_groups: studies
            .iter()
            .map(|study| study.independence_group.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        registered_independence_groups: registered
            .iter()
            .map(|study| study.independence_group.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
    }
}

fn digest_input(panel: &ReleaseDisclosurePanel) -> serde_json::Value {
    serde_json::json!({
        "feature_id": panel.feature_id,
        "output_schema": panel.output_schema,
        "boundary": panel.boundary,
        "research_id": panel.research_id,
        "request_digest": panel.request_digest,
        "studies": panel.studies,
        "counts": panel.counts,
        "disposition": panel.disposition,
        "independence_authenticated": panel.independence_authenticated,
        "scientific_truth_verified": panel.scientific_truth_verified,
        "release_authorized": panel.release_authorized,
        "disclosure_text_included": panel.disclosure_text_included,
        "statement_digests_are_unkeyed": panel.statement_digests_are_unkeyed,
    })
}

fn compile_inner(
    request: &ReleaseDisclosurePanelRequest,
    inputs: &[ReleaseDisclosureStudyInput],
) -> Result<ReleaseDisclosurePanel, ReleaseDisclosurePanelError> {
    let expected = validate_request(request)?;
    if inputs.len() > expected.len() {
        return Err(ReleaseDisclosurePanelError::InvalidStudy(
            "observed studies cannot exceed the declared cohort".into(),
        ));
    }

    let expected_by_id = expected
        .iter()
        .map(|item| (item.study_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut inputs_by_id = BTreeMap::new();
    for input in inputs {
        input
            .manifest
            .validate()
            .map_err(|error| ReleaseDisclosurePanelError::InvalidStudy(error.to_string()))?;
        input.register.validate(&input.manifest).map_err(
            |error: ReleaseDisclosureRegisterError| {
                ReleaseDisclosurePanelError::InvalidStudy(error.to_string())
            },
        )?;
        if input.manifest.research_id != request.research_id {
            return Err(ReleaseDisclosurePanelError::InvalidStudy(
                "study research identity differs from the panel request".into(),
            ));
        }
        let study_id = input.manifest.study_id.as_str();
        let Some(expected_study) = expected_by_id.get(study_id) else {
            return Err(ReleaseDisclosurePanelError::InvalidStudy(
                "observed study is not present in the declared cohort".into(),
            ));
        };
        if expected_study.manifest_digest != input.manifest.manifest_digest {
            return Err(ReleaseDisclosurePanelError::InvalidStudy(
                "observed manifest digest differs from its expected commitment".into(),
            ));
        }
        if inputs_by_id.insert(study_id.to_owned(), input).is_some() {
            return Err(ReleaseDisclosurePanelError::InvalidStudy(
                "observed study IDs must be unique".into(),
            ));
        }
    }

    let studies = expected
        .iter()
        .map(|expected_study| {
            let input = inputs_by_id.get(&expected_study.study_id);
            DisclosurePanelStudy {
                study_id: expected_study.study_id.clone(),
                independence_group: expected_study.independence_group.clone(),
                manifest_digest: expected_study.manifest_digest.clone(),
                status: if input.is_some() {
                    DisclosurePanelStudyStatus::Registered
                } else {
                    DisclosurePanelStudyStatus::Missing
                },
                register_digest: input.map(|value| value.register.register_digest.clone()),
                disclosure_counts: input.map(|value| value.register.counts),
            }
        })
        .collect::<Vec<_>>();
    let counts = counts(&studies);
    let mut panel = ReleaseDisclosurePanel {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        research_id: request.research_id.clone(),
        request_digest: request_digest(request, &expected)?,
        disposition: if counts.missing_studies == 0 {
            ReleaseDisclosurePanelDisposition::Complete
        } else {
            ReleaseDisclosurePanelDisposition::Partial
        },
        studies,
        counts,
        independence_authenticated: false,
        scientific_truth_verified: false,
        release_authorized: false,
        disclosure_text_included: false,
        statement_digests_are_unkeyed: true,
        panel_digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| ReleaseDisclosurePanelError::Digest(error.to_string()))?,
    };
    panel.panel_digest = ContentHash::of_value(&digest_input(&panel))
        .map_err(|error| ReleaseDisclosurePanelError::Digest(error.to_string()))?;
    Ok(panel)
}

impl ReleaseDisclosurePanel {
    pub fn validate(
        &self,
        request: &ReleaseDisclosurePanelRequest,
        inputs: &[ReleaseDisclosureStudyInput],
    ) -> Result<(), ReleaseDisclosurePanelError> {
        let expected = compile_inner(request, inputs)?;
        if self != &expected {
            return Err(ReleaseDisclosurePanelError::InvalidOutput(
                "panel does not exactly replay from the declared cohort and validated child registers".into(),
            ));
        }
        let expected_digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseDisclosurePanelError::Digest(error.to_string()))?;
        if expected_digest != self.panel_digest {
            return Err(ReleaseDisclosurePanelError::InvalidOutput(
                "panel digest is not bound to the reconciled disclosure rows".into(),
            ));
        }
        Ok(())
    }
}

pub fn reconcile_glioma_release_disclosure_panel(
    request: &ReleaseDisclosurePanelRequest,
    inputs: &[ReleaseDisclosureStudyInput],
) -> Result<ReleaseDisclosurePanel, ReleaseDisclosurePanelError> {
    let panel = compile_inner(request, inputs)?;
    panel.validate(request, inputs)?;
    Ok(panel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::disclosure_register::compile_glioma_release_disclosure_register;
    use crate::glioma::release::{ResearchObjectRequest, build_research_object_manifest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({ "label": label })).unwrap()
    }

    fn input(study_id: &str, label: &str) -> ReleaseDisclosureStudyInput {
        let manifest = build_research_object_manifest(&ResearchObjectRequest {
            research_id: "research-disclosure-panel".into(),
            study_id: study_id.into(),
            objective: "compile a preclinical disclosure panel".into(),
            plan_digest: hash(&format!("plan-{label}")),
            execution_digest: hash(&format!("execution-{label}")),
            replay_identity: hash(&format!("replay-{label}")),
            program_order: vec!["p10-interpretation".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: format!("artifact-{label}"),
                content_hash: hash(&format!("artifact-hash-{label}")),
                content_type: "application/vnd.aurora.glioma-result+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec![format!("negative-{label}")],
            limitations: vec![format!("limitation-{label}")],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap();
        let register = compile_glioma_release_disclosure_register(&manifest).unwrap();
        ReleaseDisclosureStudyInput { manifest, register }
    }

    fn request(inputs: &[ReleaseDisclosureStudyInput]) -> ReleaseDisclosurePanelRequest {
        ReleaseDisclosurePanelRequest {
            research_id: "research-disclosure-panel".into(),
            expected_studies: inputs
                .iter()
                .enumerate()
                .map(|(index, input)| ExpectedDisclosureStudy {
                    study_id: input.manifest.study_id.clone(),
                    independence_group: format!("group-{index}"),
                    manifest_digest: input.manifest.manifest_digest.clone(),
                })
                .collect(),
        }
    }

    #[test]
    fn panel_is_permutation_stable_and_keeps_missing_studies_visible() {
        let studies = vec![
            input("study-a", "a"),
            input("study-b", "b"),
            input("study-c", "c"),
        ];
        let request = request(&studies);
        let observed = vec![studies[2].clone(), studies[0].clone()];
        let panel = reconcile_glioma_release_disclosure_panel(&request, &observed).unwrap();
        let reversed = reconcile_glioma_release_disclosure_panel(
            &request,
            &observed.into_iter().rev().collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(panel, reversed);
        assert_eq!(
            panel.disposition,
            ReleaseDisclosurePanelDisposition::Partial
        );
        assert_eq!(panel.counts.expected_studies, 3);
        assert_eq!(panel.counts.registered_studies, 2);
        assert_eq!(panel.counts.missing_studies, 1);
        assert_eq!(panel.counts.registered_independence_groups, 2);
        assert_eq!(panel.studies[1].status, DisclosurePanelStudyStatus::Missing);
        assert!(!panel.independence_authenticated);
        assert!(!panel.scientific_truth_verified);
        assert!(!panel.release_authorized);
        assert!(!panel.disclosure_text_included);
        assert!(panel.statement_digests_are_unkeyed);
    }

    #[test]
    fn complete_panel_revalidates_each_child_and_detects_tampering() {
        let studies = vec![input("study-a", "a"), input("study-b", "b")];
        let request = request(&studies);
        let panel = reconcile_glioma_release_disclosure_panel(&request, &studies).unwrap();
        assert_eq!(
            panel.disposition,
            ReleaseDisclosurePanelDisposition::Complete
        );
        assert_eq!(panel.counts.registered_studies, 2);
        panel.validate(&request, &studies).unwrap();

        let mut tampered_panel = panel.clone();
        tampered_panel.studies[0].register_digest = Some(hash("forged-register"));
        assert!(tampered_panel.validate(&request, &studies).is_err());

        let mut tampered_child = studies.clone();
        tampered_child[0].register.release_authorized = true;
        assert!(reconcile_glioma_release_disclosure_panel(&request, &tampered_child).is_err());
    }

    #[test]
    fn panel_rejects_duplicate_commitments_unverified_single_group_and_foreign_studies() {
        let studies = vec![input("study-a", "a"), input("study-b", "b")];
        let mut duplicate = request(&studies);
        duplicate.expected_studies[1].study_id = duplicate.expected_studies[0].study_id.clone();
        assert!(reconcile_glioma_release_disclosure_panel(&duplicate, &studies).is_err());

        let mut one_group = request(&studies);
        one_group.expected_studies[1].independence_group =
            one_group.expected_studies[0].independence_group.clone();
        assert!(reconcile_glioma_release_disclosure_panel(&one_group, &studies).is_err());

        let mut foreign = studies.clone();
        foreign[0] = input("study-outside", "outside");
        assert!(reconcile_glioma_release_disclosure_panel(&request(&studies), &foreign).is_err());
    }
}
