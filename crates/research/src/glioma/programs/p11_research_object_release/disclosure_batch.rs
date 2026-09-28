//! Prospective, bounded ledger for high-throughput P11-F03 disclosure-panel requests.
//!
//! The ledger validates each submitted panel independently and keeps unsubmitted work visible.
//! It reports queue-level operational counts only; it never pools disclosure counts across
//! research identities or dispatches work to an executor.

use super::disclosure_panel::{
    ReleaseDisclosurePanel, ReleaseDisclosurePanelDisposition, ReleaseDisclosurePanelError,
    ReleaseDisclosurePanelRequest, ReleaseDisclosureStudyInput,
    reconcile_glioma_release_disclosure_panel,
};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseDisclosureBatch1@1";
const MIN_ITEMS: usize = 2;
const MAX_ITEMS: usize = 128;
const MAX_PRIORITY: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedDisclosurePanel {
    pub item_id: String,
    pub priority: u32,
    pub research_id: String,
    pub request_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureBatchRequest {
    pub batch_id: String,
    pub expected_panels: Vec<ExpectedDisclosurePanel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureBatchInput {
    pub item_id: String,
    pub request: ReleaseDisclosurePanelRequest,
    pub studies: Vec<ReleaseDisclosureStudyInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisclosureBatchItemStatus {
    AwaitingSubmission,
    PanelComplete,
    PanelPartial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureBatchItem {
    pub item_id: String,
    pub priority: u32,
    pub research_id: String,
    pub request_digest: ContentHash,
    pub status: DisclosureBatchItemStatus,
    pub panel_digest: Option<ContentHash>,
    pub panel_disposition: Option<ReleaseDisclosurePanelDisposition>,
    pub expected_studies: Option<usize>,
    pub registered_studies: Option<usize>,
    pub missing_studies: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureBatchCounts {
    pub expected_panels: usize,
    pub submitted_panels: usize,
    pub awaiting_submission: usize,
    pub complete_panels: usize,
    pub partial_panels: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseDisclosureBatchDisposition {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDisclosureBatch {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub batch_id: String,
    pub request_digest: ContentHash,
    pub items: Vec<ReleaseDisclosureBatchItem>,
    pub counts: ReleaseDisclosureBatchCounts,
    pub disposition: ReleaseDisclosureBatchDisposition,
    pub dispatch_started: bool,
    pub disclosure_counts_pooled: bool,
    pub independence_authenticated: bool,
    pub scientific_truth_verified: bool,
    pub release_authorized: bool,
    pub disclosure_text_included: bool,
    pub statement_digests_are_unkeyed: bool,
    pub batch_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseDisclosureBatchError {
    #[error("release disclosure batch request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release disclosure batch item is invalid: {0}")]
    InvalidItem(String),
    #[error("release disclosure batch output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release disclosure batch digest failed: {0}")]
    Digest(String),
}

fn validate_request(
    request: &ReleaseDisclosureBatchRequest,
) -> Result<Vec<ExpectedDisclosurePanel>, ReleaseDisclosureBatchError> {
    if request.batch_id.trim().is_empty()
        || request.batch_id.len() > 128
        || !(MIN_ITEMS..=MAX_ITEMS).contains(&request.expected_panels.len())
    {
        return Err(ReleaseDisclosureBatchError::InvalidRequest(
            "batch identity and 2 to 128 expected panel submissions are required".into(),
        ));
    }
    let mut item_ids = BTreeSet::new();
    let mut request_digests = BTreeSet::new();
    for expected in &request.expected_panels {
        if expected.item_id.trim().is_empty()
            || expected.item_id.len() > 128
            || expected.priority > MAX_PRIORITY
            || expected.research_id.trim().is_empty()
            || expected.research_id.len() > 256
            || expected.request_digest.as_str().len() != 64
            || !item_ids.insert(expected.item_id.as_str())
            || !request_digests.insert(expected.request_digest.as_str())
        {
            return Err(ReleaseDisclosureBatchError::InvalidRequest(
                "item IDs and request digests must be valid and unique; priorities must be bounded"
                    .into(),
            ));
        }
    }
    let mut expected = request.expected_panels.clone();
    expected.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.item_id.cmp(&right.item_id))
    });
    Ok(expected)
}

fn request_digest(
    request: &ReleaseDisclosureBatchRequest,
    expected: &[ExpectedDisclosurePanel],
) -> Result<ContentHash, ReleaseDisclosureBatchError> {
    ContentHash::of_value(&serde_json::json!({
        "batch_id": request.batch_id,
        "expected_panels": expected,
    }))
    .map_err(|error| ReleaseDisclosureBatchError::Digest(error.to_string()))
}

fn counts(items: &[ReleaseDisclosureBatchItem]) -> ReleaseDisclosureBatchCounts {
    let awaiting_submission = items
        .iter()
        .filter(|item| item.status == DisclosureBatchItemStatus::AwaitingSubmission)
        .count();
    let complete_panels = items
        .iter()
        .filter(|item| item.status == DisclosureBatchItemStatus::PanelComplete)
        .count();
    let partial_panels = items
        .iter()
        .filter(|item| item.status == DisclosureBatchItemStatus::PanelPartial)
        .count();
    ReleaseDisclosureBatchCounts {
        expected_panels: items.len(),
        submitted_panels: complete_panels + partial_panels,
        awaiting_submission,
        complete_panels,
        partial_panels,
    }
}

fn digest_input(batch: &ReleaseDisclosureBatch) -> serde_json::Value {
    serde_json::json!({
        "feature_id": batch.feature_id,
        "output_schema": batch.output_schema,
        "boundary": batch.boundary,
        "batch_id": batch.batch_id,
        "request_digest": batch.request_digest,
        "items": batch.items,
        "counts": batch.counts,
        "disposition": batch.disposition,
        "dispatch_started": batch.dispatch_started,
        "disclosure_counts_pooled": batch.disclosure_counts_pooled,
        "independence_authenticated": batch.independence_authenticated,
        "scientific_truth_verified": batch.scientific_truth_verified,
        "release_authorized": batch.release_authorized,
        "disclosure_text_included": batch.disclosure_text_included,
        "statement_digests_are_unkeyed": batch.statement_digests_are_unkeyed,
    })
}

fn compile_inner(
    request: &ReleaseDisclosureBatchRequest,
    inputs: &[ReleaseDisclosureBatchInput],
) -> Result<ReleaseDisclosureBatch, ReleaseDisclosureBatchError> {
    let expected = validate_request(request)?;
    if inputs.len() > expected.len() {
        return Err(ReleaseDisclosureBatchError::InvalidItem(
            "submitted panels cannot exceed the declared batch".into(),
        ));
    }
    let expected_by_id = expected
        .iter()
        .map(|item| (item.item_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut submitted_by_id = BTreeMap::new();
    let mut compiled_panels = BTreeMap::<String, ReleaseDisclosurePanel>::new();
    for input in inputs {
        let Some(expected_item) = expected_by_id.get(input.item_id.as_str()) else {
            return Err(ReleaseDisclosureBatchError::InvalidItem(
                "submitted item is not present in the expected batch".into(),
            ));
        };
        if input.request.research_id != expected_item.research_id {
            return Err(ReleaseDisclosureBatchError::InvalidItem(
                "submitted panel research identity differs from its expected item".into(),
            ));
        }
        let panel = reconcile_glioma_release_disclosure_panel(&input.request, &input.studies)
            .map_err(|error: ReleaseDisclosurePanelError| {
                ReleaseDisclosureBatchError::InvalidItem(error.to_string())
            })?;
        if panel.request_digest != expected_item.request_digest {
            return Err(ReleaseDisclosureBatchError::InvalidItem(
                "submitted panel request differs from its expected digest".into(),
            ));
        }
        if submitted_by_id
            .insert(input.item_id.clone(), input)
            .is_some()
        {
            return Err(ReleaseDisclosureBatchError::InvalidItem(
                "submitted item IDs must be unique".into(),
            ));
        }
        compiled_panels.insert(input.item_id.clone(), panel);
    }

    let items = expected
        .iter()
        .map(|expected_item| {
            let panel = compiled_panels.get(&expected_item.item_id);
            ReleaseDisclosureBatchItem {
                item_id: expected_item.item_id.clone(),
                priority: expected_item.priority,
                research_id: expected_item.research_id.clone(),
                request_digest: expected_item.request_digest.clone(),
                status: match panel.map(|value| value.disposition) {
                    None => DisclosureBatchItemStatus::AwaitingSubmission,
                    Some(ReleaseDisclosurePanelDisposition::Complete) => {
                        DisclosureBatchItemStatus::PanelComplete
                    }
                    Some(ReleaseDisclosurePanelDisposition::Partial) => {
                        DisclosureBatchItemStatus::PanelPartial
                    }
                },
                panel_digest: panel.map(|value| value.panel_digest.clone()),
                panel_disposition: panel.map(|value| value.disposition),
                expected_studies: panel.map(|value| value.counts.expected_studies),
                registered_studies: panel.map(|value| value.counts.registered_studies),
                missing_studies: panel.map(|value| value.counts.missing_studies),
            }
        })
        .collect::<Vec<_>>();
    let counts = counts(&items);
    let disposition = if counts.awaiting_submission == 0 && counts.partial_panels == 0 {
        ReleaseDisclosureBatchDisposition::Complete
    } else {
        ReleaseDisclosureBatchDisposition::Partial
    };
    let mut batch = ReleaseDisclosureBatch {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        batch_id: request.batch_id.clone(),
        request_digest: request_digest(request, &expected)?,
        items,
        counts,
        disposition,
        dispatch_started: false,
        disclosure_counts_pooled: false,
        independence_authenticated: false,
        scientific_truth_verified: false,
        release_authorized: false,
        disclosure_text_included: false,
        statement_digests_are_unkeyed: true,
        batch_digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| ReleaseDisclosureBatchError::Digest(error.to_string()))?,
    };
    batch.batch_digest = ContentHash::of_value(&digest_input(&batch))
        .map_err(|error| ReleaseDisclosureBatchError::Digest(error.to_string()))?;
    Ok(batch)
}

impl ReleaseDisclosureBatch {
    pub fn validate(
        &self,
        request: &ReleaseDisclosureBatchRequest,
        inputs: &[ReleaseDisclosureBatchInput],
    ) -> Result<(), ReleaseDisclosureBatchError> {
        let expected = compile_inner(request, inputs)?;
        if self != &expected {
            return Err(ReleaseDisclosureBatchError::InvalidOutput(
                "batch does not replay from its predeclared queue and validated panel requests"
                    .into(),
            ));
        }
        let expected_digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseDisclosureBatchError::Digest(error.to_string()))?;
        if expected_digest != self.batch_digest {
            return Err(ReleaseDisclosureBatchError::InvalidOutput(
                "batch digest is not bound to the ordered queue results".into(),
            ));
        }
        Ok(())
    }
}

pub fn reconcile_glioma_release_disclosure_batch(
    request: &ReleaseDisclosureBatchRequest,
    inputs: &[ReleaseDisclosureBatchInput],
) -> Result<ReleaseDisclosureBatch, ReleaseDisclosureBatchError> {
    let batch = compile_inner(request, inputs)?;
    batch.validate(request, inputs)?;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::disclosure_panel::{
        ExpectedDisclosureStudy, ReleaseDisclosurePanelRequest,
    };
    use crate::glioma::programs::p11_research_object_release::disclosure_register::compile_glioma_release_disclosure_register;
    use crate::glioma::release::{ResearchObjectRequest, build_research_object_manifest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({ "label": label })).unwrap()
    }

    fn study_input(research_id: &str, study_id: &str) -> ReleaseDisclosureStudyInput {
        let manifest = build_research_object_manifest(&ResearchObjectRequest {
            research_id: research_id.into(),
            study_id: study_id.into(),
            objective: "prepare a bounded preclinical disclosure batch".into(),
            plan_digest: hash(&format!("plan-{study_id}")),
            execution_digest: hash(&format!("execution-{study_id}")),
            replay_identity: hash(&format!("replay-{study_id}")),
            program_order: vec!["p10-interpretation".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: format!("artifact-{study_id}"),
                content_hash: hash(&format!("artifact-hash-{study_id}")),
                content_type: "application/vnd.aurora.glioma-result+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec![format!("negative-{study_id}")],
            limitations: vec![format!("limitation-{study_id}")],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap();
        let register = compile_glioma_release_disclosure_register(&manifest).unwrap();
        ReleaseDisclosureStudyInput { manifest, register }
    }

    fn panel_request(research_id: &str) -> ReleaseDisclosurePanelRequest {
        let first = study_input(research_id, &format!("{research_id}-study-a"));
        let second = study_input(research_id, &format!("{research_id}-study-b"));
        ReleaseDisclosurePanelRequest {
            research_id: research_id.into(),
            expected_studies: vec![
                ExpectedDisclosureStudy {
                    study_id: first.manifest.study_id.clone(),
                    independence_group: format!("{research_id}-group-a"),
                    manifest_digest: first.manifest.manifest_digest.clone(),
                },
                ExpectedDisclosureStudy {
                    study_id: second.manifest.study_id.clone(),
                    independence_group: format!("{research_id}-group-b"),
                    manifest_digest: second.manifest.manifest_digest.clone(),
                },
            ],
        }
    }

    fn expected_item(
        item_id: &str,
        priority: u32,
        request: &ReleaseDisclosurePanelRequest,
    ) -> ExpectedDisclosurePanel {
        let panel = reconcile_glioma_release_disclosure_panel(request, &[]).unwrap();
        ExpectedDisclosurePanel {
            item_id: item_id.into(),
            priority,
            research_id: request.research_id.clone(),
            request_digest: panel.request_digest,
        }
    }

    fn input(
        item_id: &str,
        request: ReleaseDisclosurePanelRequest,
        studies: Vec<ReleaseDisclosureStudyInput>,
    ) -> ReleaseDisclosureBatchInput {
        ReleaseDisclosureBatchInput {
            item_id: item_id.into(),
            request,
            studies,
        }
    }

    #[test]
    fn batch_is_priority_stable_and_separates_complete_partial_and_unsubmitted_panels() {
        let alpha_request = panel_request("research-alpha");
        let beta_request = panel_request("research-beta");
        let gamma_request = panel_request("research-gamma");
        let request = ReleaseDisclosureBatchRequest {
            batch_id: "release-batch-1".into(),
            expected_panels: vec![
                expected_item("panel-alpha", 20, &alpha_request),
                expected_item("panel-beta", 10, &beta_request),
                expected_item("panel-gamma", 30, &gamma_request),
            ],
        };
        let alpha_studies = vec![
            study_input("research-alpha", "research-alpha-study-a"),
            study_input("research-alpha", "research-alpha-study-b"),
        ];
        let beta_studies = vec![study_input("research-beta", "research-beta-study-a")];
        let inputs = vec![
            input("panel-alpha", alpha_request, alpha_studies),
            input("panel-beta", beta_request, beta_studies),
        ];
        let batch = reconcile_glioma_release_disclosure_batch(&request, &inputs).unwrap();
        let reversed = reconcile_glioma_release_disclosure_batch(
            &request,
            &inputs.into_iter().rev().collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(batch, reversed);
        assert_eq!(
            batch.disposition,
            ReleaseDisclosureBatchDisposition::Partial
        );
        assert_eq!(batch.items[0].item_id, "panel-beta");
        assert_eq!(
            batch.items[0].status,
            DisclosureBatchItemStatus::PanelPartial
        );
        assert_eq!(
            batch.items[1].status,
            DisclosureBatchItemStatus::PanelComplete
        );
        assert_eq!(
            batch.items[2].status,
            DisclosureBatchItemStatus::AwaitingSubmission
        );
        assert_eq!(batch.counts.expected_panels, 3);
        assert_eq!(batch.counts.submitted_panels, 2);
        assert_eq!(batch.counts.awaiting_submission, 1);
        assert_eq!(batch.counts.complete_panels, 1);
        assert_eq!(batch.counts.partial_panels, 1);
        assert!(!batch.dispatch_started);
        assert!(!batch.disclosure_counts_pooled);
        assert!(!batch.independence_authenticated);
        assert!(!batch.scientific_truth_verified);
        assert!(!batch.release_authorized);
        assert!(!batch.disclosure_text_included);
        assert!(batch.statement_digests_are_unkeyed);
    }

    #[test]
    fn batch_rejects_unexpected_or_mismatched_inputs_and_detects_report_tampering() {
        let alpha_request = panel_request("research-alpha");
        let beta_request = panel_request("research-beta");
        let request = ReleaseDisclosureBatchRequest {
            batch_id: "release-batch-2".into(),
            expected_panels: vec![
                expected_item("panel-alpha", 0, &alpha_request),
                expected_item("panel-beta", 1, &beta_request),
            ],
        };
        let valid_input = input(
            "panel-alpha",
            alpha_request.clone(),
            vec![
                study_input("research-alpha", "research-alpha-study-a"),
                study_input("research-alpha", "research-alpha-study-b"),
            ],
        );
        let mut unexpected = valid_input.clone();
        unexpected.item_id = "unknown-panel".into();
        assert!(reconcile_glioma_release_disclosure_batch(&request, &[unexpected]).is_err());

        let mut mismatch = request.clone();
        mismatch.expected_panels[0].request_digest = hash("wrong-panel-request");
        assert!(
            reconcile_glioma_release_disclosure_batch(&mismatch, &[valid_input.clone()]).is_err()
        );

        let mut duplicate = request.clone();
        duplicate.expected_panels[1].item_id = duplicate.expected_panels[0].item_id.clone();
        assert!(reconcile_glioma_release_disclosure_batch(&duplicate, &[]).is_err());

        let mut batch =
            reconcile_glioma_release_disclosure_batch(&request, &[valid_input.clone()]).unwrap();
        batch.items[0].panel_digest = Some(hash("forged-panel"));
        assert!(batch.validate(&request, &[valid_input]).is_err());
    }
}
