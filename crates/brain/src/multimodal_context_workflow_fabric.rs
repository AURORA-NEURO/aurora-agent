//! Multimodal multi-study context workflow fabric.
//!
//! Atlas feature: `AFA-brain-P03-F14`. The fabric schedules a context build only
//! when every requested study×modality cell has a comparable, local, typed input.
//! Missing modalities, semantic incompatibility, negative evidence, and policy
//! failures remain explicit release evidence.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F14";
pub const CONTRACT_VERSION: &str = "brain-multimodal-context-workflow-fabric/1.0";
const WORKFLOW_CONTENT_TYPE: &str = "application/vnd.aurora.multimodal-context-workflow+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalContextInput {
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
pub struct MultimodalContextWorkflowRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub goal: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub inputs: Vec<ModalContextInput>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub goal: String,
    pub disposition: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cell_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub consumed_budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalContextWorkflowError {
    #[error("invalid multimodal context workflow request: {0}")]
    Invalid(String),
    #[error("multimodal context workflow artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalContextWorkflowReceipt {
    pub fn validate(&self) -> Result<(), MultimodalContextWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.cell_order.is_empty()
            || self.plan_order.is_empty()
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalContextWorkflowError::Invalid("multimodal workflow identity, closure, budget, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
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
            (&self.accepted_order, "accepted_order"),
            (&self.missing_order, "missing_order"),
            (&self.incompatible_order, "incompatible_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.plan_order, "plan_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let cells = self.cell_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.accepted_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.missing_order.iter().cloned());
        classified.extend(self.incompatible_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != cells
            || !identity_keys(&self.accepted_order)
                .is_disjoint(&identity_keys(&self.incompatible_order))
            || !identity_keys(&self.accepted_order).is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.incompatible_order)
                .is_disjoint(&identity_keys(&self.unknown_order))
        {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal cells do not partition outcomes".into(),
            ));
        }
        for digest in [
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalContextWorkflowError::Invalid(
                    "multimodal workflow digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == "admitted" {
            vec![format!(
                "schedule:multimodal-context-workflow:{}",
                self.workflow_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal workflow effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal context workflow receipts must declare local emitted data".into(),
            ));
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "cell_order": self.cell_order,
            "accepted_order": self.accepted_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal workflow checkpoint digest is not bound to cell state".into(),
            ));
        }
        let expected_workflow_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "disposition": self.disposition,
            "plan_order": self.plan_order,
            "checkpoint_digest": self.checkpoint_digest,
            "budget_units": self.budget_units,
            "consumed_budget_units": self.consumed_budget_units,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal workflow digest is not bound to workflow state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-multimodal-context-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalContextWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))
    }
}

pub fn multimodal_context_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "research workflow operator".into()].into(), behavior: "schedules a multimodal context workflow only after study×modality closure and comparability gates".into(), value: "prevents silent modality substitution while making imaging and omics context preparation repeatable and locally auditable".into(), inputs: vec![TypedPort { name: "multimodal_context_workflow_request".into(), schema: "ResearchWorkflowSpec1@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_context_workflow_receipt".into(), schema: "MultimodalContextWorkflowReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:multimodal-context-workflow".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_multimodal_context_workflow(
    request: &MultimodalContextWorkflowRequest,
) -> Result<MultimodalContextWorkflowReceipt, MultimodalContextWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalContextWorkflowError::Invalid(
            "multimodal workflow identity, closure, budget, replay, or boundary is invalid".into(),
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
        return Err(MultimodalContextWorkflowError::Invalid(
            "study and modality identifiers must be unique and non-empty".into(),
        ));
    }
    let mut input_map = BTreeMap::new();
    for input in &request.inputs {
        let key = format!("{}|{}", input.study_id, input.modality);
        if input_map.insert(key.clone(), input).is_some() {
            return Err(MultimodalContextWorkflowError::Invalid(
                "multimodal input cells must be unique".into(),
            ));
        }
    }
    let mut cells = BTreeSet::new();
    let mut accepted = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for study in &studies {
        for modality in &modalities {
            let key = format!("{}|{}", study, modality);
            cells.insert(key.clone());
            match input_map.get(&key) {
                None => {
                    missing.insert(key.clone());
                    omissions.insert(format!("cell:{}:missing-modality", key));
                }
                Some(input)
                    if !request.policy_allow
                        || !request.protected_closure
                        || !request.raw_data_local
                        || !input.raw_data_local
                        || input.boundary != PRECLINICAL_BOUNDARY =>
                {
                    incompatible.insert(key.clone());
                    omissions.insert(format!("cell:{}:local-policy-gate-blocked", key));
                }
                Some(input) if !input.comparable => {
                    incompatible.insert(key.clone());
                    negative.insert(format!("cell:{}:incomparable", key));
                }
                Some(input) if input.replay_identity != request.replay_identity => {
                    unknown.insert(key.clone());
                    uncertainty.insert(format!("cell:{}:replay-mismatch", key));
                }
                Some(input)
                    if matches!(
                        input.state,
                        EvidenceState::Proven | EvidenceState::Supported
                    ) =>
                {
                    accepted.insert(key.clone());
                }
                Some(input)
                    if matches!(
                        input.state,
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
        !request.raw_data_local || input_map.values().any(|input| !input.raw_data_local);
    let cell_count = u32::try_from(cells.len()).map_err(|_| {
        MultimodalContextWorkflowError::Invalid(
            "multimodal cell count exceeds workflow budget width".into(),
        )
    })?;
    let required_budget = cell_count.checked_add(2).ok_or_else(|| {
        MultimodalContextWorkflowError::Invalid(
            "multimodal workflow budget exceeds representable range".into(),
        )
    })?;
    let gates_open = request.policy_allow && request.protected_closure && !locality_failure;
    let disposition = if !gates_open {
        "blocked"
    } else if accepted.len() == cells.len() && request.budget_units >= required_budget {
        "admitted"
    } else {
        "refinement_required"
    };
    let consumed = request.budget_units.min(required_budget);
    if request.budget_units < required_budget {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if locality_failure {
        omissions.insert("workflow:raw-data-locality-failed".into());
        omissions.insert("workflow:policy-or-locality-blocked".into());
    }
    let plan = (0..required_budget)
        .map(|index| format!("plan:multimodal-context-stage:{index:02}"))
        .collect::<Vec<_>>();
    let raw_data_local = true;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "cell_order": cells, "accepted_order": accepted, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "disposition": disposition, "plan_order": plan, "checkpoint_digest": checkpoint_digest, "budget_units": request.budget_units, "consumed_budget_units": consumed, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "admitted" {
        vec![format!(
            "schedule:multimodal-context-workflow:{}",
            request.workflow_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "query_id": request.query_id, "goal": request.goal, "disposition": disposition, "study_order": studies, "modality_order": modalities, "cell_order": cells, "accepted_order": accepted, "missing_order": missing, "incompatible_order": incompatible, "unknown_order": unknown, "plan_order": plan, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "consumed_budget_units": consumed, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-context-workflow:{}", request.workflow_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalContextWorkflowError::Artifact(error.to_string()))?;
    let receipt = MultimodalContextWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        cell_order: cells.into_iter().collect(),
        accepted_order: accepted.into_iter().collect(),
        missing_order: missing.into_iter().collect(),
        incompatible_order: incompatible.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        plan_order: plan,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        consumed_budget_units: consumed,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
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

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalContextWorkflowError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalContextWorkflowError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), MultimodalContextWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalContextWorkflowError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), MultimodalContextWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalContextWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &MultimodalContextWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "cell_order": receipt.cell_order,
        "accepted_order": receipt.accepted_order,
        "missing_order": receipt.missing_order,
        "incompatible_order": receipt.incompatible_order,
        "unknown_order": receipt.unknown_order,
        "plan_order": receipt.plan_order,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "consumed_budget_units": receipt.consumed_budget_units,
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
    fn input(study: &str, modality: &str, comparable: bool) -> ModalContextInput {
        ModalContextInput {
            study_id: study.into(),
            modality: modality.into(),
            artifact_digest: hash("artifact"),
            semantic_digest: hash("semantic"),
            replay_identity: hash("replay"),
            state: EvidenceState::Supported,
            comparable,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request() -> MultimodalContextWorkflowRequest {
        let replay = hash("replay");
        let mut inputs = Vec::new();
        for study in ["study:a", "study:b"] {
            for modality in ["imaging", "transcriptomics"] {
                let mut cell = input(study, modality, true);
                cell.artifact_digest = replay.clone();
                cell.semantic_digest = replay.clone();
                inputs.push(cell);
            }
        }
        MultimodalContextWorkflowRequest {
            request_id: "request:multimodal-workflow".into(),
            workflow_id: "workflow:multi".into(),
            query_id: "query:multi".into(),
            goal: "compile comparable imaging and omics context".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            inputs,
            budget_units: 6,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_context_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn complete_multimodal_closure_admits() {
        let receipt = compile_multimodal_context_workflow(&request()).unwrap();
        assert_eq!(receipt.disposition, "admitted");
        assert_eq!(receipt.accepted_order.len(), 4);
    }
    #[test]
    fn missing_modality_is_explicit() {
        let mut value = request();
        value.inputs.pop();
        let receipt = compile_multimodal_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "refinement_required");
        assert!(!receipt.missing_order.is_empty());
    }
    #[test]
    fn incomparability_is_negative_evidence() {
        let mut value = request();
        value.inputs[0].comparable = false;
        let receipt = compile_multimodal_context_workflow(&value).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("incomparable")));
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_multimodal_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_multimodal_context_workflow(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn input_locality_failure_blocks_release() {
        let mut input = request();
        input.inputs[0].raw_data_local = false;
        let receipt = compile_multimodal_context_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn workflow_artifact_payload_is_bound() {
        let mut receipt = compile_multimodal_context_workflow(&request()).unwrap();
        receipt.query_id = "query:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
