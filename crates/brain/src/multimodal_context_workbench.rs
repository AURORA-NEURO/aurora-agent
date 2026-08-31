//! Multimodal multi-study context compilation research workbench.
//!
//! Atlas feature: `AFA-brain-P03-F18`. The workbench exposes study×modality
//! closure and comparability evidence for imaging and omics context without
//! silently promoting an incomplete multimodal projection.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F18";
pub const CONTRACT_VERSION: &str = "brain-multimodal-context-workbench/1.0";
const WORKBENCH_CONTENT_TYPE: &str = "application/vnd.aurora.multimodal-context-workbench+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextWorkbenchCell {
    pub study_id: String,
    pub modality: String,
    pub artifact_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub comparable: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextWorkbenchRequest {
    pub session_id: String,
    pub query_id: String,
    pub goal: String,
    pub projection_disposition: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub cells: Vec<MultimodalContextWorkbenchCell>,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub session_id: String,
    pub query_id: String,
    pub goal: String,
    pub disposition: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cell_order: Vec<String>,
    pub qualified_cell_order: Vec<String>,
    pub missing_cell_order: Vec<String>,
    pub incompatible_cell_order: Vec<String>,
    pub unknown_cell_order: Vec<String>,
    pub view_order: Vec<String>,
    pub action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalContextWorkbenchError {
    #[error("invalid multimodal workbench request: {0}")]
    Invalid(String),
    #[error("multimodal workbench artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalContextWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), MultimodalContextWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.session_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.cell_order.is_empty()
            || self.view_order.is_empty()
            || self.action_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "ready" | "needs_refinement" | "blocked"
            )
        {
            return Err(MultimodalContextWorkbenchError::Invalid("multimodal workbench identity, cell closure, view, action, locality, disposition, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.session_id, "session_id"),
            (&self.query_id, "query_id"),
            (&self.goal, "goal"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.study_order, "study_order"),
            (&self.modality_order, "modality_order"),
            (&self.cell_order, "cell_order"),
            (&self.qualified_cell_order, "qualified_cell_order"),
            (&self.missing_cell_order, "missing_cell_order"),
            (&self.incompatible_cell_order, "incompatible_cell_order"),
            (&self.unknown_cell_order, "unknown_cell_order"),
            (&self.view_order, "view_order"),
            (&self.action_order, "action_order"),
            (&self.blocked_action_order, "blocked_action_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let cells = self.cell_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self
            .qualified_cell_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.missing_cell_order.iter().cloned());
        classified.extend(self.incompatible_cell_order.iter().cloned());
        classified.extend(self.unknown_cell_order.iter().cloned());
        if classified != cells {
            return Err(MultimodalContextWorkbenchError::Invalid(
                "multimodal cells do not partition outcomes".into(),
            ));
        }
        if !identity_keys(&self.action_order)
            .is_disjoint(&identity_keys(&self.blocked_action_order))
        {
            return Err(MultimodalContextWorkbenchError::Invalid(
                "multimodal workbench actions cannot be both available and blocked".into(),
            ));
        }
        for digest in [
            &self.context_digest,
            &self.section_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalContextWorkbenchError::Invalid(
                    "multimodal workbench digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == "blocked" {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "view:local-multimodal-workbench:{}",
                self.session_id
            )]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(MultimodalContextWorkbenchError::Invalid(
                "multimodal workbench effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(MultimodalContextWorkbenchError::Invalid(
                "multimodal context workbench receipts must declare local emitted data".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-multimodal-context-workbench:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKBENCH_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalContextWorkbenchError::Invalid(
                "multimodal workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalContextWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalContextWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalContextWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalContextWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalContextWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn multimodal_context_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "laboratory automation engineer".into()].into(), behavior: "renders multimodal study×modality context, comparability, evidence, omissions, and safe researcher actions".into(), value: "lets researchers inspect imaging and omics context without silently substituting missing modalities or exporting unsafe decisions".into(), inputs: vec![TypedPort { name: "multimodal_workbench_request".into(), schema: "ResearchWorkbenchSession1@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_workbench_receipt".into(), schema: "MultimodalContextWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:local-multimodal-workbench".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn render_multimodal_context_workbench(
    request: &MultimodalContextWorkbenchRequest,
) -> Result<MultimodalContextWorkbenchReceipt, MultimodalContextWorkbenchError> {
    if request.session_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.context_digest.as_str().len() != 64
        || request.section_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalContextWorkbenchError::Invalid(
            "multimodal workbench identity, closure, digest, or boundary is invalid".into(),
        ));
    }
    let studies = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if studies.len() != request.study_ids.len()
        || modalities.len() != request.required_modalities.len()
        || studies.iter().any(|value| value.trim().is_empty())
        || modalities.iter().any(|value| value.trim().is_empty())
    {
        return Err(MultimodalContextWorkbenchError::Invalid(
            "study and modality identifiers must be unique and non-empty".into(),
        ));
    }
    let mut cell_map = BTreeMap::new();
    for cell in &request.cells {
        let key = format!("{}|{}", cell.study_id, cell.modality);
        if cell_map.insert(key.clone(), cell).is_some() {
            return Err(MultimodalContextWorkbenchError::Invalid(
                "multimodal workbench cells must be unique".into(),
            ));
        }
    }
    let mut cells = BTreeSet::new();
    let mut qualified = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut views = BTreeSet::from([
        "view:multimodal-summary".to_string(),
        "view:comparability-matrix".to_string(),
        "view:evidence-lineage".to_string(),
        "view:replay-identity".to_string(),
    ]);
    let mut actions = BTreeSet::from([
        "action:inspect-cell".to_string(),
        "action:replay-local-projection".to_string(),
    ]);
    let mut blocked_actions = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for study in &studies {
        for modality in &modalities {
            let key = format!("{}|{}", study, modality);
            cells.insert(key.clone());
            match cell_map.get(&key) {
                None => {
                    missing.insert(key.clone());
                    omissions.insert(format!("cell:{}:missing-modality", key));
                }
                Some(cell)
                    if !request.policy_allow
                        || !request.raw_data_local
                        || !cell.raw_data_local
                        || cell.boundary != PRECLINICAL_BOUNDARY =>
                {
                    incompatible.insert(key.clone());
                    omissions.insert(format!("cell:{}:policy-locality-blocked", key));
                }
                Some(cell) if !cell.comparable => {
                    incompatible.insert(key.clone());
                    negative.insert(format!("cell:{}:incomparable", key));
                }
                Some(cell) if cell.replay_identity != request.replay_identity => {
                    unknown.insert(key.clone());
                    uncertainty.insert(format!("cell:{}:replay-mismatch", key));
                }
                Some(cell)
                    if matches!(cell.state, EvidenceState::Proven | EvidenceState::Supported) =>
                {
                    qualified.insert(key.clone());
                }
                Some(cell)
                    if matches!(
                        cell.state,
                        EvidenceState::Speculative | EvidenceState::Unknown
                    ) =>
                {
                    unknown.insert(key.clone());
                    uncertainty.insert(format!("cell:{}:evidence-uncertain", key));
                }
                Some(_) => {
                    incompatible.insert(key.clone());
                    negative.insert(format!("cell:{}:contradicted", key));
                }
            }
        }
    }
    let locality_failure =
        !request.raw_data_local || cell_map.values().any(|cell| !cell.raw_data_local);
    if locality_failure {
        omissions.insert("workbench:policy-or-locality-blocked".into());
    }
    let disposition = if !request.policy_allow || locality_failure {
        omissions.insert("workbench:policy-or-locality-blocked".into());
        "blocked"
    } else if request.projection_disposition == "admitted" && qualified.len() == cells.len() {
        actions.insert("action:open-decision-section".into());
        actions.insert("action:export-local-research-object".into());
        "ready"
    } else {
        actions.insert("action:review-comparability".into());
        actions.insert("action:request-modality-refinement".into());
        uncertainty.insert("workbench:multimodal-projection-not-admitted".into());
        "needs_refinement"
    };
    if disposition == "blocked" {
        blocked_actions.extend([
            "action:open-decision-section".to_string(),
            "action:export-local-research-object".to_string(),
            "action:replay-local-projection".to_string(),
        ]);
        actions.clear();
        actions.insert("action:inspect-block-reason".into());
    }
    if !missing.is_empty() {
        views.insert("view:missing-modalities".into());
    }
    if !incompatible.is_empty() {
        views.insert("view:incompatibility-evidence".into());
    }
    if !unknown.is_empty() {
        views.insert("view:uncertain-cells".into());
    }
    let effects = if disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "view:local-multimodal-workbench:{}",
            request.session_id
        )]
    };
    let raw_data_local = true;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "session_id": request.session_id, "query_id": request.query_id, "goal": request.goal, "disposition": disposition, "study_order": studies, "modality_order": modalities, "cell_order": cells, "qualified_cell_order": qualified, "missing_cell_order": missing, "incompatible_cell_order": incompatible, "unknown_cell_order": unknown, "view_order": views, "action_order": actions, "blocked_action_order": blocked_actions, "context_digest": request.context_digest, "section_digest": request.section_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effects, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-context-workbench:{}", request.session_id),
        WORKBENCH_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalContextWorkbenchError::Artifact(error.to_string()))?;
    let receipt = MultimodalContextWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        session_id: request.session_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        cell_order: cells.into_iter().collect(),
        qualified_cell_order: qualified.into_iter().collect(),
        missing_cell_order: missing.into_iter().collect(),
        incompatible_cell_order: incompatible.into_iter().collect(),
        unknown_cell_order: unknown.into_iter().collect(),
        view_order: views.into_iter().collect(),
        action_order: actions.into_iter().collect(),
        blocked_action_order: blocked_actions.into_iter().collect(),
        context_digest: request.context_digest.clone(),
        section_digest: request.section_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: effects,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalContextWorkbenchError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalContextWorkbenchError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), MultimodalContextWorkbenchError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalContextWorkbenchError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), MultimodalContextWorkbenchError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalContextWorkbenchError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &MultimodalContextWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "session_id": receipt.session_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "cell_order": receipt.cell_order,
        "qualified_cell_order": receipt.qualified_cell_order,
        "missing_cell_order": receipt.missing_cell_order,
        "incompatible_cell_order": receipt.incompatible_cell_order,
        "unknown_cell_order": receipt.unknown_cell_order,
        "view_order": receipt.view_order,
        "action_order": receipt.action_order,
        "blocked_action_order": receipt.blocked_action_order,
        "context_digest": receipt.context_digest,
        "section_digest": receipt.section_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> MultimodalContextWorkbenchRequest {
        let h = hash("digest");
        let cells = vec![
            cell("study:a", "imaging", &h),
            cell("study:a", "omics", &h),
            cell("study:b", "imaging", &h),
            cell("study:b", "omics", &h),
        ];
        MultimodalContextWorkbenchRequest {
            session_id: "session:multi".into(),
            query_id: "query:multi".into(),
            goal: "inspect multimodal context".into(),
            projection_disposition: "admitted".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "omics".into()],
            cells,
            context_digest: h.clone(),
            section_digest: h.clone(),
            replay_identity: h,
            policy_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn cell(study: &str, modality: &str, replay: &ContentHash) -> MultimodalContextWorkbenchCell {
        MultimodalContextWorkbenchCell {
            study_id: study.into(),
            modality: modality.into(),
            artifact_digest: replay.clone(),
            semantic_digest: replay.clone(),
            replay_identity: replay.clone(),
            state: EvidenceState::Supported,
            comparable: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            multimodal_context_workbench_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn complete_matrix_is_ready() {
        let receipt = render_multimodal_context_workbench(&request()).unwrap();
        assert_eq!(receipt.disposition, "ready");
        assert_eq!(receipt.qualified_cell_order.len(), 4);
    }
    #[test]
    fn missing_modality_is_visible() {
        let mut value = request();
        value.cells.pop();
        let receipt = render_multimodal_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "needs_refinement");
        assert!(receipt
            .view_order
            .iter()
            .any(|item| item == "view:missing-modalities"));
    }
    #[test]
    fn incomparable_cell_blocks_release_action() {
        let mut value = request();
        value.cells[0].comparable = false;
        let receipt = render_multimodal_context_workbench(&value).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("incomparable")));
        assert!(!receipt
            .action_order
            .iter()
            .any(|item| item == "action:open-decision-section"));
    }
    #[test]
    fn digest_is_stable() {
        let receipt = render_multimodal_context_workbench(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn cell_locality_failure_blocks_release() {
        let mut input = request();
        input.cells[0].raw_data_local = false;
        let receipt = render_multimodal_context_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn workbench_artifact_payload_is_bound() {
        let mut receipt = render_multimodal_context_workbench(&request()).unwrap();
        receipt.query_id = "query:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
