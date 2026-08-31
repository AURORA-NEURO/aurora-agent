//! Multimodal retrieval-and-synthesis workflow fabric.
//!
//! Atlas feature: `AFA-brain-P02-F14`. This orchestration product makes study and modality
//! closure part of the resumable workflow contract; missing coverage is compensated, never
//! silently completed.

use crate::multimodal_retrieval_synthesis::{
    synthesize_multimodal_retrieval, MultimodalRetrievalQuery,
};
use crate::retrieval_synthesis::SynthesisDisposition;
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F14";
pub const CONTRACT_VERSION: &str = "brain-multimodal-retrieval-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "MultimodalRetrievalWorkflowReceipt1@1";
const WORKFLOW_CONTENT_TYPE: &str =
    "application/vnd.aurora.multimodal-retrieval-workflow-receipt+json";
const MAX_TEXT_BYTES: usize = 512;
const STAGE_ORDER: [&str; 5] = [
    "stage:checkpoint",
    "stage:retrieve-multimodal-candidates",
    "stage:compare-modalities",
    "stage:synthesize-evidence",
    "stage:validate-output",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalWorkflowRequest {
    pub request: MultimodalRetrievalQuery,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub checkpoint_id: String,
    pub scope: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: SynthesisDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalRetrievalWorkflowError {
    #[error("invalid multimodal retrieval workflow request: {0}")]
    Invalid(String),
    #[error("multimodal retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal retrieval workflow engine failed: {0}")]
    Engine(String),
}

impl MultimodalRetrievalWorkflowReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.plan_order.is_empty()
            || self.budget_units == 0
        {
            return Err(MultimodalRetrievalWorkflowError::Invalid("multimodal workflow identity, coverage, stages, plan, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
            (&self.checkpoint_id, "checkpoint_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.stage_order != STAGE_ORDER {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow stage order is not the canonical protocol".into(),
            ));
        }
        if self.completed_order != self.stage_order {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow completed order does not cover the stage protocol".into(),
            ));
        }
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.plan_order, "plan_order")?;
        validate_sorted_unique(&self.blocked_order, "blocked_order")?;
        validate_sorted_unique(&self.compensation_order, "compensation_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        validate_unique(&self.ranked_order, "ranked_order")?;
        validate_sorted_unique(&self.qualified_order, "qualified_order")?;
        validate_sorted_unique(&self.unknown_order, "unknown_order")?;
        for (values, field) in [
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let required_plans = STAGE_ORDER
            .iter()
            .map(|stage| format!("plan:{stage}"))
            .collect::<BTreeSet<_>>();
        let plan_keys = self.plan_order.iter().cloned().collect::<BTreeSet<_>>();
        if !required_plans.is_subset(&plan_keys)
            || self
                .plan_order
                .iter()
                .filter(|plan| plan.starts_with("plan:"))
                .count()
                != required_plans.len() + 1
            || !self.plan_order.iter().any(|plan| {
                matches!(
                    plan.as_str(),
                    "plan:publish-qualified-multimodal-retrieval"
                        | "plan:retain-partial-multimodal-retrieval"
                        | "plan:retain-missing-multimodal-closure"
                )
            })
        {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow plan does not contain exactly one terminal branch".into(),
            ));
        }
        let expected_terminal_plan = if self.qualified_order.is_empty() {
            "plan:retain-missing-multimodal-closure"
        } else if self.disposition == SynthesisDisposition::Qualified {
            "plan:publish-qualified-multimodal-retrieval"
        } else {
            "plan:retain-partial-multimodal-retrieval"
        };
        if !self
            .plan_order
            .iter()
            .any(|plan| plan == expected_terminal_plan)
        {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow terminal plan does not match disposition and coverage".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow ranking must contain every candidate exactly once".into(),
            ));
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
        {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow candidate states must partition candidates".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.synthesis_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalRetrievalWorkflowError::Invalid(
                    "multimodal workflow digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == SynthesisDisposition::Qualified {
            if self.compensation_order.is_empty() {
                vec![format!(
                    "schedule:multimodal-retrieval-work:{}",
                    self.workflow_id
                )]
            } else {
                return Err(MultimodalRetrievalWorkflowError::Invalid(
                    "qualified multimodal workflow cannot carry compensation steps".into(),
                ));
            }
        } else if self.disposition != SynthesisDisposition::Blocked
            && !self.compensation_order.is_empty()
        {
            self.compensation_order.clone()
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow effects do not match disposition and compensation".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|omission| omission == "workflow:raw-data-locality-failed"))
        {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "non-local multimodal workflows must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow checkpoint digest is not bound to checkpoint state".into(),
            ));
        }
        let expected_workflow_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "disposition": self.disposition,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "checkpoint_digest": self.checkpoint_digest,
            "comparability_digest": self.comparability_digest,
            "synthesis_digest": self.synthesis_digest,
            "budget_units": self.budget_units,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow digest is not bound to workflow state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-multimodal-retrieval-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalRetrievalWorkflowError::Invalid(
                "multimodal workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalRetrievalWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalRetrievalWorkflowError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalRetrievalWorkflowError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), MultimodalRetrievalWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalRetrievalWorkflowError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), MultimodalRetrievalWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalRetrievalWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &MultimodalRetrievalWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "checkpoint_id": receipt.checkpoint_id,
        "scope": receipt.scope,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "disposition": receipt.disposition,
        "stage_order": receipt.stage_order,
        "plan_order": receipt.plan_order,
        "completed_order": receipt.completed_order,
        "blocked_order": receipt.blocked_order,
        "compensation_order": receipt.compensation_order,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "unknown_order": receipt.unknown_order,
        "comparability_digest": receipt.comparability_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn multimodal_retrieval_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["multimodal retrieval workflow operator".into(), "laboratory automation engineer".into()].into(), behavior: "schedules a checkpointed multimodal retrieval workflow with study/modality closure, deterministic stages, compensation, and replay receipts".into(), value: "makes cross-study and cross-modality coverage a first-class workflow gate without external effects or silent completion".into(), inputs: vec![TypedPort { name: "multimodal_retrieval_workflow_request".into(), schema: "ResearchWorkflowSpec2@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_retrieval_workflow_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:multimodal-retrieval-work".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_multimodal_retrieval_workflow(
    request: &MultimodalRetrievalWorkflowRequest,
) -> Result<MultimodalRetrievalWorkflowReceipt, MultimodalRetrievalWorkflowError> {
    validate_request(request)?;
    let synthesis = synthesize_multimodal_retrieval(&request.request)
        .map_err(|error| MultimodalRetrievalWorkflowError::Engine(error.to_string()))?;
    let stage_order = request.requested_stage_order.clone();
    let mut plan_order = stage_order
        .iter()
        .map(|stage| format!("plan:{stage}"))
        .collect::<BTreeSet<_>>();
    let completed_order = stage_order.clone();
    let mut blocked_order = synthesis
        .blocked_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut compensation_order = BTreeSet::new();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let locality_gate = request.raw_data_local && request.request.raw_data_local;
    let can_publish = request.policy_allow
        && request.protected_closure
        && locality_gate
        && u64::from(request.budget_units)
            >= u64::try_from(STAGE_ORDER.len())
                .unwrap_or(u64::MAX)
                .saturating_add(1)
        && synthesis.disposition == SynthesisDisposition::Qualified;
    if synthesis.qualified_order.is_empty() {
        plan_order.insert("plan:retain-missing-multimodal-closure".into());
        compensation_order
            .insert("compensate:multimodal-retrieval-work:retain-unresolved-coverage".into());
        omissions.insert("workflow:no-qualified-multimodal-retrieval-to-schedule".into());
        blocked_order.extend(synthesis.unknown_order.iter().cloned());
    } else if !can_publish {
        plan_order.insert("plan:retain-partial-multimodal-retrieval".into());
        compensation_order
            .insert("compensate:multimodal-retrieval-work:retain-partial-coverage".into());
    } else {
        plan_order.insert("plan:publish-qualified-multimodal-retrieval".into());
    }
    let plan_count = u64::try_from(plan_order.len()).unwrap_or(u64::MAX);
    if u64::from(request.budget_units) < plan_count {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !locality_gate {
        omissions.insert("workflow:raw-data-locality-failed".into());
    }
    let raw_data_local = true;
    let actionable = u64::from(request.budget_units) >= plan_count
        && request.policy_allow
        && request.protected_closure
        && locality_gate
        && synthesis.disposition != SynthesisDisposition::Blocked;
    let disposition = if actionable {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
    };
    if disposition == SynthesisDisposition::Blocked {
        compensation_order.clear();
    }
    let plan_order = plan_order.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked_order.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation_order.into_iter().collect::<Vec<_>>();
    let study_order = request
        .request
        .study_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = request
        .request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| MultimodalRetrievalWorkflowError::Engine(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "replay_identity": request.replay_identity})).map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "disposition": disposition, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "checkpoint_digest": checkpoint_digest, "comparability_digest": synthesis.comparability_digest, "synthesis_digest": synthesis_digest, "budget_units": request.budget_units, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == SynthesisDisposition::Qualified {
        vec![format!(
            "schedule:multimodal-retrieval-work:{}",
            request.workflow_id
        )]
    } else if disposition != SynthesisDisposition::Blocked && !compensation_order.is_empty() {
        compensation_order.clone()
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "scope": request.request.scope, "study_order": study_order, "modality_order": modality_order, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "unknown_order": synthesis.unknown_order, "comparability_digest": synthesis.comparability_digest, "synthesis_digest": synthesis_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-multimodal-retrieval-workflow:{}",
            request.workflow_id
        ),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalWorkflowError::Artifact(error.to_string()))?;
    let receipt = MultimodalRetrievalWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        scope: request.request.scope.clone(),
        study_order,
        modality_order,
        disposition,
        stage_order: request.requested_stage_order.clone(),
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        unknown_order: synthesis.unknown_order,
        comparability_digest: synthesis.comparability_digest,
        synthesis_digest,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
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

fn validate_request(
    request: &MultimodalRetrievalWorkflowRequest,
) -> Result<(), MultimodalRetrievalWorkflowError> {
    if request.requested_stage_order != STAGE_ORDER
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalRetrievalWorkflowError::Invalid("multimodal workflow identity, canonical stages, budget, replay, or boundary is incomplete".into()));
    }
    validate_text(&request.workflow_id, "workflow_id")?;
    validate_text(&request.checkpoint_id, "checkpoint_id")?;
    validate_text(&request.boundary, "boundary")?;
    if request.replay_identity.as_str().len() != 64
        || request.request.replay_identity.as_str().len() != 64
    {
        return Err(MultimodalRetrievalWorkflowError::Invalid(
            "multimodal workflow replay identity is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> MultimodalRetrievalWorkflowRequest {
        let candidates = [
            ("evidence:a-imaging", "study:a", "imaging"),
            ("evidence:a-omics", "study:a", "transcriptomics"),
            ("evidence:b-imaging", "study:b", "imaging"),
            ("evidence:b-omics", "study:b", "transcriptomics"),
        ]
        .into_iter()
        .map(|(evidence_id, study_id, modality)| RetrievalCandidate {
            evidence_id: evidence_id.into(),
            source_id: format!("source:{study_id}:{modality}"),
            study_id: study_id.into(),
            scope: "organoid:neural".into(),
            modality: modality.into(),
            support_milli: 900,
            state,
            semantic_digest: hash(evidence_id),
            artifact_digest: hash(&format!("artifact:{evidence_id}")),
            provenance_digest: hash(&format!("provenance:{evidence_id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .collect();
        MultimodalRetrievalWorkflowRequest {
            request: MultimodalRetrievalQuery {
                request_id: "request:multimodal-retrieval-workflow".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_support_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                candidates,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:multimodal-retrieval".into(),
            requested_stage_order: vec![
                "stage:checkpoint".into(),
                "stage:retrieve-multimodal-candidates".into(),
                "stage:compare-modalities".into(),
                "stage:synthesize-evidence".into(),
                "stage:validate-output".into(),
            ],
            checkpoint_id: "checkpoint:1".into(),
            budget_units: 12,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let manifest = multimodal_retrieval_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_coverage_is_scheduled() {
        let receipt =
            compile_multimodal_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Qualified);
        assert!(receipt.effect_receipts[0].starts_with("schedule:multimodal-retrieval-work:"));
    }
    #[test]
    fn unknown_coverage_compensates() {
        let receipt =
            compile_multimodal_retrieval_workflow(&request(EvidenceState::Unknown)).unwrap();
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_multimodal_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn stage_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_stage_order.reverse();
        assert!(compile_multimodal_retrieval_workflow(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_multimodal_retrieval_workflow(&input).is_err());
    }
    #[test]
    fn locality_failure_is_retained_as_blocked_state() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        let receipt = compile_multimodal_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workflow:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn workflow_state_and_artifact_payload_are_bound() {
        let mut state_drift =
            compile_multimodal_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        state_drift
            .plan_order
            .retain(|item| item != "plan:publish-qualified-multimodal-retrieval");
        assert!(state_drift.validate().is_err());

        let mut payload_drift =
            compile_multimodal_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        payload_drift.workflow_digest = hash("workflow-drift");
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_workflow_identity_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.workflow_id.push(' ');
        assert!(compile_multimodal_retrieval_workflow(&input).is_err());
    }
}
