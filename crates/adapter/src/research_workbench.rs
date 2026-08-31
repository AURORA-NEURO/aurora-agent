//! Multimodal multi-study researcher workbench compiler.
//!
//! Atlas feature: `AFA-adapter-P24-F18`.
//!
//! This module compiles authorized study manifests into an accessible, deterministic workspace
//! projection. It is a view/compiler boundary, not a data mover or scientific inference engine:
//! cross-study comparability, missing modalities, provenance gaps, negative results, and denied
//! actions stay visible in the workspace receipt.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P24-F18";
pub const CONTRACT_VERSION: &str = "multimodal-research-workbench/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_STUDIES: usize = 8192;
const MAX_VIEWS: usize = 8192;
const MAX_ITEMS: usize = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparabilityStatus {
    Comparable,
    Conditional,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyWorkspaceEntry {
    pub study_id: String,
    pub modalities: Vec<String>,
    pub artifact_ids: Vec<ContentHash>,
    pub comparability: ComparabilityStatus,
    pub provenance_complete: bool,
    pub authorized: bool,
    pub negative_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceViewRequest {
    pub view_id: String,
    pub view_kind: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkspaceState {
    pub schema_version: String,
    pub workspace_id: String,
    pub studies: Vec<StudyWorkspaceEntry>,
    pub views: Vec<WorkspaceViewRequest>,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceDisposition {
    Ready,
    Partial,
    Blocked,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveResearchWorkspace {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub workspace_id: String,
    pub policy_allow: bool,
    pub disposition: WorkspaceDisposition,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub action_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl InteractiveResearchWorkspace {
    pub fn validate(&self) -> Result<(), ResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ResearchWorkbenchError::Contract(
                "research workbench identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.workspace_id.trim().is_empty()
            || self.study_order.is_empty()
            || self.view_order.is_empty()
            || self.panel_order.is_empty()
            || self.action_receipts.is_empty()
        {
            return Err(ResearchWorkbenchError::InvalidRequest("workspace identity, studies, views, panels, actions, locality, and boundary are required".into()));
        }
        validate_sorted_strings(&self.study_order, "study_order")?;
        validate_sorted_strings(&self.modality_order, "modality_order")?;
        validate_sorted_strings(&self.view_order, "view_order")?;
        validate_sorted_strings(&self.panel_order, "panel_order")?;
        validate_sorted_strings(&self.action_receipts, "action_receipts")?;
        validate_sorted_strings(&self.omissions, "omissions")?;
        validate_sorted_strings(&self.uncertainty, "uncertainty")?;
        validate_sorted_strings(&self.negative_evidence, "negative_evidence")?;
        if self.artifact_order.len() > MAX_ITEMS
            || self
                .artifact_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .artifact_order
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(ResearchWorkbenchError::InvalidRequest(
                "workspace artifact ordering or digest identity is invalid".into(),
            ));
        }
        if self.panel_order
            != self
                .view_order
                .iter()
                .map(|view| format!("panel:{view}"))
                .collect::<Vec<_>>()
        {
            return Err(ResearchWorkbenchError::InvalidRequest(
                "workspace panels are not bound to view order".into(),
            ));
        }
        if !self.policy_allow
            && (self.disposition != WorkspaceDisposition::Blocked
                || self
                    .action_receipts
                    .iter()
                    .any(|receipt| !receipt.ends_with(":blocked-policy")))
        {
            return Err(ResearchWorkbenchError::InvalidRequest(
                "policy-denied workspaces must block every view action".into(),
            ));
        }
        if self.policy_allow
            && self
                .action_receipts
                .iter()
                .any(|receipt| receipt.ends_with(":blocked-policy"))
        {
            return Err(ResearchWorkbenchError::InvalidRequest(
                "policy-allowed workspaces cannot retain policy-blocked actions".into(),
            ));
        }
        if self.artifact.artifact_id != format!("research-workbench:{}", self.workspace_id)
            || self.artifact.content_type
                != "application/vnd.aurora.interactive-research-workspace+json"
            || !self.artifact.semantic_loss.is_empty()
        {
            return Err(ResearchWorkbenchError::Contract(
                "workspace artifact is not bound to the projection".into(),
            ));
        }
        let expected_provenance = workspace_provenance(&self.workspace_id, &self.artifact_order);
        if self.artifact.provenance != expected_provenance {
            return Err(ResearchWorkbenchError::Contract(
                "workspace artifact provenance is not bound to input artifacts".into(),
            ));
        }
        let payload = workspace_payload(self);
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| ResearchWorkbenchError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ResearchWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResearchWorkbenchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResearchWorkbenchError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResearchWorkbenchError {
    #[error("invalid research workbench state: {0}")]
    InvalidRequest(String),
    #[error("research workbench contract rejected: {0}")]
    Contract(String),
    #[error("research workbench serialization failed: {0}")]
    Serialization(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), ResearchWorkbenchError> {
    if value.is_empty() || value.trim() != value {
        return Err(ResearchWorkbenchError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ResearchWorkbenchError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), ResearchWorkbenchError> {
    if values.len() > max_items {
        return Err(ResearchWorkbenchError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ResearchWorkbenchError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(values: &[String], field: &str) -> Result<(), ResearchWorkbenchError> {
    validate_unique_strings(values, field, MAX_ITEMS)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ResearchWorkbenchError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn workspace_provenance(workspace_id: &str, artifact_order: &[ContentHash]) -> Vec<ProvenanceLink> {
    artifact_order
        .iter()
        .cloned()
        .map(|digest| ProvenanceLink {
            source_id: workspace_id.into(),
            relation: "workspace-input-artifact".into(),
            digest,
        })
        .collect()
}

fn workspace_payload(workspace: &InteractiveResearchWorkspace) -> serde_json::Value {
    workspace_payload_from_parts(
        &workspace.workspace_id,
        workspace.policy_allow,
        workspace.disposition,
        &workspace.study_order,
        &workspace.modality_order,
        &workspace.view_order,
        &workspace.panel_order,
        &workspace.artifact_order,
        &workspace.omissions,
        &workspace.uncertainty,
        &workspace.negative_evidence,
        &workspace.action_receipts,
        workspace.raw_data_local,
        &workspace.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn workspace_payload_from_parts(
    workspace_id: &str,
    policy_allow: bool,
    disposition: WorkspaceDisposition,
    study_order: &[String],
    modality_order: &[String],
    view_order: &[String],
    panel_order: &[String],
    artifact_order: &[ContentHash],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    action_receipts: &[String],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "workspace_id": workspace_id,
        "policy_allow": policy_allow,
        "disposition": disposition,
        "study_order": study_order,
        "modality_order": modality_order,
        "view_order": view_order,
        "panel_order": panel_order,
        "artifact_order": artifact_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "action_receipts": action_receipts,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

pub fn compile_research_workbench(
    state: &ResearchWorkspaceState,
) -> Result<InteractiveResearchWorkspace, ResearchWorkbenchError> {
    validate_state(state)?;
    let mut studies = state.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = studies
        .iter()
        .filter(|study| study.authorized)
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let mut modalities = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut negative_evidence = Vec::new();
    let study_map = studies
        .iter()
        .map(|study| (study.study_id.as_str(), study))
        .collect::<BTreeMap<_, _>>();
    let mut omissions = studies
        .iter()
        .filter(|study| !study.authorized)
        .map(|study| format!("study:{}:authorization-denied", study.study_id))
        .collect::<Vec<_>>();
    let mut uncertainty = studies
        .iter()
        .filter(|study| {
            study.comparability == ComparabilityStatus::Unknown || !study.provenance_complete
        })
        .map(|study| {
            format!(
                "study:{}:comparability-or-provenance-unknown",
                study.study_id
            )
        })
        .collect::<Vec<_>>();
    for study in &studies {
        if study.authorized {
            modalities.extend(study.modalities.iter().cloned());
            artifacts.extend(study.artifact_ids.iter().cloned());
        }
        if let Some(result) = &study.negative_result {
            negative_evidence.push(format!("study:{}:{}", study.study_id, result));
        }
    }
    let mut views = state.views.clone();
    views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
    let view_order = views
        .iter()
        .map(|view| view.view_id.clone())
        .collect::<Vec<_>>();
    let panel_order = views
        .iter()
        .map(|view| format!("panel:{}", view.view_id))
        .collect::<Vec<_>>();
    let mut action_receipts = Vec::new();
    for view in &views {
        let selected = view
            .study_ids
            .iter()
            .filter_map(|id| study_map.get(id.as_str()))
            .collect::<Vec<_>>();
        if selected.iter().any(|study| !study.authorized) {
            omissions.push(format!("view:{}:unauthorized-study", view.view_id));
            action_receipts.push(format!("view:{}:blocked-unauthorized", view.view_id));
            continue;
        }
        if selected.is_empty() {
            omissions.push(format!("view:{}:no-authorized-studies", view.view_id));
            action_receipts.push(format!("view:{}:blocked-empty", view.view_id));
            continue;
        }
        if selected.len() > 1
            && selected
                .iter()
                .any(|study| study.comparability != ComparabilityStatus::Comparable)
        {
            uncertainty.push(format!(
                "view:{}:cross-study-comparability-not-established",
                view.view_id
            ));
            action_receipts.push(format!("view:{}:conditional-comparability", view.view_id));
        } else {
            action_receipts.push(format!("view:{}:rendered-local", view.view_id));
        }
        for modality in &view.required_modalities {
            if !selected
                .iter()
                .all(|study| study.modalities.contains(modality))
            {
                omissions.push(format!(
                    "view:{}:missing-modality:{}",
                    view.view_id, modality
                ));
            }
        }
    }
    let disposition = if !state.policy_allow {
        action_receipts = views
            .iter()
            .map(|view| format!("view:{}:blocked-policy", view.view_id))
            .collect();
        WorkspaceDisposition::Blocked
    } else if action_receipts
        .iter()
        .all(|receipt| receipt.contains("blocked"))
    {
        WorkspaceDisposition::Blocked
    } else if !omissions.is_empty() || !uncertainty.is_empty() {
        WorkspaceDisposition::Partial
    } else if studies.iter().any(|study| !study.authorized) {
        WorkspaceDisposition::LocalOnly
    } else {
        WorkspaceDisposition::Ready
    };
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    action_receipts.sort();
    action_receipts.dedup();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let payload = workspace_payload_from_parts(
        &state.workspace_id,
        state.policy_allow,
        disposition,
        &study_order,
        &modality_order,
        &view_order,
        &panel_order,
        &artifact_order,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &action_receipts,
        state.raw_data_local,
        &state.boundary,
    );
    let provenance = workspace_provenance(&state.workspace_id, &artifact_order);
    let artifact = TypedResearchArtifact::from_payload(
        format!("research-workbench:{}", state.workspace_id),
        "application/vnd.aurora.interactive-research-workspace+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| ResearchWorkbenchError::Contract(error.to_string()))?;
    let result = InteractiveResearchWorkspace {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        workspace_id: state.workspace_id.clone(),
        policy_allow: state.policy_allow,
        disposition,
        study_order,
        modality_order,
        view_order,
        panel_order,
        artifact_order,
        omissions,
        uncertainty,
        negative_evidence,
        action_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

fn validate_state(state: &ResearchWorkspaceState) -> Result<(), ResearchWorkbenchError> {
    if state.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || state.workspace_id.trim().is_empty()
        || state.studies.is_empty()
        || state.views.is_empty()
        || !state.raw_data_local
        || state.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ResearchWorkbenchError::InvalidRequest(
            "workspace identity, studies, views, locality, and boundary are required".into(),
        ));
    }
    validate_text("workspace_id", &state.workspace_id)?;
    validate_text("boundary", &state.boundary)?;
    if state.studies.len() > MAX_STUDIES || state.views.len() > MAX_VIEWS {
        return Err(ResearchWorkbenchError::InvalidRequest(
            "workspace studies or views exceed their bounds".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for study in &state.studies {
        validate_text("study_id", &study.study_id)?;
        if !ids.insert(study.study_id.clone())
            || study.modalities.is_empty()
            || study.artifact_ids.is_empty()
        {
            return Err(ResearchWorkbenchError::InvalidRequest(format!(
                "study {} is invalid or duplicated",
                study.study_id
            )));
        }
        validate_unique_strings(&study.modalities, "study.modalities", MAX_ITEMS)?;
        if study
            .artifact_ids
            .iter()
            .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(ResearchWorkbenchError::InvalidRequest(format!(
                "study {} contains an empty artifact digest",
                study.study_id
            )));
        }
        let unique_artifacts = study.artifact_ids.iter().collect::<BTreeSet<_>>();
        if study.artifact_ids.len() > MAX_ITEMS
            || unique_artifacts.len() != study.artifact_ids.len()
        {
            return Err(ResearchWorkbenchError::InvalidRequest(format!(
                "study {} contains duplicate or excessive artifacts",
                study.study_id
            )));
        }
        if let Some(result) = &study.negative_result {
            validate_text("study.negative_result", result)?;
        }
    }
    let mut view_ids = BTreeSet::new();
    for view in &state.views {
        validate_text("view_id", &view.view_id)?;
        validate_text("view_kind", &view.view_kind)?;
        if !view_ids.insert(view.view_id.clone())
            || view.study_ids.is_empty()
            || view.study_ids.iter().any(|id| !ids.contains(id))
        {
            return Err(ResearchWorkbenchError::InvalidRequest(format!(
                "view {} is invalid or references an unknown study",
                view.view_id
            )));
        }
        validate_unique_strings(&view.study_ids, "view.study_ids", MAX_STUDIES)?;
        validate_unique_strings(
            &view.required_modalities,
            "view.required_modalities",
            MAX_ITEMS,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> ResearchWorkspaceState {
        ResearchWorkspaceState {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workspace_id: "workspace:atlas".into(),
            studies: vec![
                StudyWorkspaceEntry {
                    study_id: "study:imaging".into(),
                    modalities: vec!["imaging".into(), "omics".into()],
                    artifact_ids: vec![ContentHash::of_bytes(b"imaging")],
                    comparability: ComparabilityStatus::Comparable,
                    provenance_complete: true,
                    authorized: true,
                    negative_result: Some("null secondary endpoint".into()),
                },
                StudyWorkspaceEntry {
                    study_id: "study:omics".into(),
                    modalities: vec!["imaging".into(), "omics".into()],
                    artifact_ids: vec![ContentHash::of_bytes(b"omics")],
                    comparability: ComparabilityStatus::Comparable,
                    provenance_complete: true,
                    authorized: true,
                    negative_result: None,
                },
            ],
            views: vec![WorkspaceViewRequest {
                view_id: "view:comparison".into(),
                view_kind: "linked-study".into(),
                study_ids: vec!["study:imaging".into(), "study:omics".into()],
                required_modalities: vec!["imaging".into(), "omics".into()],
            }],
            policy_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn workbench_is_deterministic_and_local() {
        let first = compile_research_workbench(&state()).unwrap();
        let second = compile_research_workbench(&state()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.disposition, WorkspaceDisposition::Ready);
        assert!(first
            .action_receipts
            .iter()
            .all(|receipt| receipt.contains("local")));
    }
    #[test]
    fn missing_modality_is_partial() {
        let mut state = state();
        state.views[0]
            .required_modalities
            .push("spatial-transcriptomics".into());
        let result = compile_research_workbench(&state).unwrap();
        assert_eq!(result.disposition, WorkspaceDisposition::Partial);
        assert!(!result.omissions.is_empty());
    }
    #[test]
    fn unauthorized_study_is_blocked_or_partial() {
        let mut state = state();
        state.studies[1].authorized = false;
        let result = compile_research_workbench(&state).unwrap();
        assert!(matches!(
            result.disposition,
            WorkspaceDisposition::Blocked
                | WorkspaceDisposition::Partial
                | WorkspaceDisposition::LocalOnly
        ));
        assert!(!result.omissions.is_empty());
    }
    #[test]
    fn denied_policy_blocks_views() {
        let mut state = state();
        state.policy_allow = false;
        let result = compile_research_workbench(&state).unwrap();
        assert_eq!(result.disposition, WorkspaceDisposition::Blocked);
    }
    #[test]
    fn conditional_comparability_is_explicit() {
        let mut state = state();
        state.studies[1].comparability = ComparabilityStatus::Conditional;
        let result = compile_research_workbench(&state).unwrap();
        assert_eq!(result.disposition, WorkspaceDisposition::Partial);
        assert!(result
            .uncertainty
            .iter()
            .any(|value| value.contains("comparability")));
    }

    #[test]
    fn workspace_artifact_payload_is_verified() {
        let mut workspace = compile_research_workbench(&state()).unwrap();
        workspace.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(workspace.validate().is_err());
    }

    #[test]
    fn workspace_panels_cannot_be_reassigned_to_views() {
        let mut workspace = compile_research_workbench(&state()).unwrap();
        workspace.panel_order[0] = "panel:view:other".into();
        assert!(workspace.validate().is_err());
    }

    #[test]
    fn duplicate_study_artifacts_are_rejected() {
        let mut value = state();
        let artifact = value.studies[0].artifact_ids[0].clone();
        value.studies[0].artifact_ids.push(artifact);
        assert!(compile_research_workbench(&value).is_err());
    }
}
