//! Federated continual retrieval workflow fabric.
//!
//! Atlas feature: `AFA-brain-P02-F16`. Federation admission, aggregate-only export, and raw
//! locality are checkpointed workflow stages with explicit compensation and denial receipts.

use crate::federated_retrieval_synthesis::{
    synthesize_federated_retrieval, FederatedRetrievalDisposition, FederatedRetrievalQuery,
};
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F16";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "FederatedRetrievalWorkflowReceipt1@1";
const WORKFLOW_CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-retrieval-workflow-receipt+json";
const MAX_TEXT_BYTES: usize = 512;
const STAGE_ORDER: [&str; 5] = [
    "stage:checkpoint",
    "stage:admit-federation",
    "stage:retrieve-local-candidates",
    "stage:synthesize-aggregate",
    "stage:validate-envelope",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalWorkflowRequest {
    pub request: FederatedRetrievalQuery,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub checkpoint_seq: u64,
    pub operator_id: String,
    pub approval_reference: ContentHash,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: FederatedRetrievalDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub approval_reference: ContentHash,
    pub replay_identity: ContentHash,
    pub checkpoint_seq: u64,
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
pub enum FederatedRetrievalWorkflowError {
    #[error("invalid federated retrieval workflow request: {0}")]
    Invalid(String),
    #[error("federated retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval workflow engine failed: {0}")]
    Engine(String),
}

impl FederatedRetrievalWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.plan_order.is_empty()
            || self.checkpoint_seq == 0
            || self.budget_units == 0
        {
            return Err(FederatedRetrievalWorkflowError::Invalid("federated workflow identity, coverage, stages, plan, checkpoint, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.stage_order != STAGE_ORDER {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow stage order is not canonical".into(),
            ));
        }
        if self.completed_order != self.stage_order {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow completed order does not cover stages".into(),
            ));
        }
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.plan_order, "plan_order")?;
        validate_sorted_unique(&self.blocked_order, "blocked_order")?;
        validate_sorted_unique(&self.compensation_order, "compensation_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        validate_unique(&self.ranked_order, "ranked_order")?;
        validate_unique(&self.qualified_order, "qualified_order")?;
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
        {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow plan does not contain exactly one terminal branch".into(),
            ));
        }
        let expected_terminal_plan = if self.aggregate_order.is_empty() {
            "plan:retain-local-federated-closure"
        } else if self.disposition == FederatedRetrievalDisposition::Qualified {
            "plan:publish-permitted-aggregate"
        } else {
            "plan:retain-partial-federated-closure"
        };
        if !self
            .plan_order
            .iter()
            .any(|plan| plan == expected_terminal_plan)
        {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow terminal plan does not match disposition and aggregate state"
                    .into(),
            ));
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranked_values = self.ranked_order.iter().cloned().collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        if ranked_values != candidate_values
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
            || !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || !qualified_values.is_disjoint(&unknown_values)
            || self.aggregate_order.len() != self.qualified_order.len()
        {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow ranking, candidate states, and aggregate coverage are inconsistent".into(),
            ));
        }
        validate_digest_order(&self.aggregate_order)?;
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.synthesis_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalWorkflowError::Invalid(
                    "federated workflow digest is invalid".into(),
                ));
            }
        }
        if !self.raw_data_local {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow receipts must declare that emitted data is local".into(),
            ));
        }
        let expected_effect_receipts =
            if self.disposition == FederatedRetrievalDisposition::Qualified {
                if self.compensation_order.is_empty() {
                    vec![format!(
                        "schedule:federated-retrieval-work:{}",
                        self.workflow_id
                    )]
                } else {
                    return Err(FederatedRetrievalWorkflowError::Invalid(
                        "qualified federated workflows cannot carry compensation steps".into(),
                    ));
                }
            } else if self.disposition != FederatedRetrievalDisposition::Blocked
                && !self.compensation_order.is_empty()
            {
                self.compensation_order.clone()
            } else {
                vec!["block:unsafe-release".into()]
            };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow effects do not match disposition and compensation".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-retrieval-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedRetrievalWorkflowError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedRetrievalWorkflowError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedRetrievalWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedRetrievalWorkflowError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedRetrievalWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn validate_digest_order(values: &[ContentHash]) -> Result<(), FederatedRetrievalWorkflowError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1])
        || values.iter().any(|value| value.as_str().len() != 64)
    {
        return Err(FederatedRetrievalWorkflowError::Invalid(
            "federated aggregate ordering or digest is invalid".into(),
        ));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedRetrievalWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
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
        "aggregate_order": receipt.aggregate_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "approval_reference": receipt.approval_reference,
        "replay_identity": receipt.replay_identity,
        "checkpoint_seq": receipt.checkpoint_seq,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn federated_retrieval_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["federation steward".into(), "multisite retrieval workflow operator".into()].into(), behavior: "schedules a checkpointed federated retrieval workflow with signer, approval, comparability, aggregate-only export, locality, compensation, and replay receipts".into(), value: "turns consortium retrieval into a resumable governed workflow without raw-data movement or hidden federation denial".into(), inputs: vec![TypedPort { name: "federated_retrieval_workflow_request".into(), schema: "ResearchWorkflowSpec4@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_retrieval_workflow_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["schedule:federated-retrieval-work".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![bioprism_foundation::AuthorityRequirement { role: "federated retrieval workflow approver".into(), reason: "approve a purpose-bound aggregate-only workflow after signer, comparability, locality, and replay gates close".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_federated_retrieval_workflow(
    request: &FederatedRetrievalWorkflowRequest,
) -> Result<FederatedRetrievalWorkflowReceipt, FederatedRetrievalWorkflowError> {
    validate_request(request)?;
    let synthesis = synthesize_federated_retrieval(&request.request)
        .map_err(|error| FederatedRetrievalWorkflowError::Engine(error.to_string()))?;
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
    let can_publish = request.policy_allow
        && request.request.policy_allow
        && request.protected_closure
        && request.request.protected_closure
        && request.raw_data_local
        && request.request.raw_data_local
        && request.request.signer_valid
        && request.request.approval_valid
        && u64::from(request.budget_units)
            >= u64::try_from(STAGE_ORDER.len())
                .unwrap_or(u64::MAX)
                .saturating_add(1)
        && request.approval_reference != ContentHash::of_bytes(b"")
        && synthesis.disposition == FederatedRetrievalDisposition::Qualified;
    if synthesis.aggregate_order.is_empty() {
        plan_order.insert("plan:retain-local-federated-closure".into());
        compensation_order
            .insert("compensate:federated-retrieval-work:retain-denied-aggregate".into());
        omissions.insert("workflow:no-permitted-aggregate-to-schedule".into());
        blocked_order.extend(synthesis.unknown_order.iter().cloned());
    } else if !can_publish {
        plan_order.insert("plan:retain-partial-federated-closure".into());
        compensation_order
            .insert("compensate:federated-retrieval-work:retain-partial-aggregate".into());
    } else {
        plan_order.insert("plan:publish-permitted-aggregate".into());
    }
    let plan_count = u64::try_from(plan_order.len()).unwrap_or(u64::MAX);
    if u64::from(request.budget_units) < plan_count {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if !request.policy_allow || !request.request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure || !request.request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.raw_data_local || !request.request.raw_data_local {
        omissions.insert("workflow:raw-data-locality-failed".into());
    }
    if !request.request.signer_valid {
        omissions.insert("workflow:signer-invalid".into());
    }
    if !request.request.approval_valid {
        omissions.insert("workflow:approval-invalid".into());
    }
    let actionable = u64::from(request.budget_units) >= plan_count
        && request.policy_allow
        && request.request.policy_allow
        && request.protected_closure
        && request.request.protected_closure
        && request.raw_data_local
        && request.request.raw_data_local
        && request.request.signer_valid
        && request.request.approval_valid
        && request.approval_reference != ContentHash::of_bytes(b"")
        && synthesis.disposition != FederatedRetrievalDisposition::Blocked;
    let disposition = if actionable {
        synthesis.disposition
    } else {
        FederatedRetrievalDisposition::Blocked
    };
    if disposition == FederatedRetrievalDisposition::Blocked {
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
        .map_err(|error| FederatedRetrievalWorkflowError::Engine(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "checkpoint_seq": request.checkpoint_seq, "stage_order": stage_order, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_order, "completed_order": completed_order, "checkpoint_digest": checkpoint_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis_digest, "budget_units": request.budget_units, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == FederatedRetrievalDisposition::Qualified {
        vec![format!(
            "schedule:federated-retrieval-work:{}",
            request.workflow_id
        )]
    } else if disposition != FederatedRetrievalDisposition::Blocked
        && !compensation_order.is_empty()
    {
        compensation_order.clone()
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "study_order": study_order, "modality_order": modality_order, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "checkpoint_seq": request.checkpoint_seq, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-retrieval-workflow:{}", request.workflow_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalWorkflowError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        endpoint: request.request.endpoint.clone(),
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
        aggregate_order: synthesis.aggregate_order,
        comparability_digest: synthesis.comparability_digest,
        envelope_digest: synthesis.envelope_digest,
        synthesis_digest,
        checkpoint_digest,
        workflow_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        checkpoint_seq: request.checkpoint_seq,
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedRetrievalWorkflowRequest,
) -> Result<(), FederatedRetrievalWorkflowError> {
    if request.requested_stage_order != STAGE_ORDER
        || request.checkpoint_seq == 0
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedRetrievalWorkflowError::Invalid("federated workflow identity, canonical stages, checkpoint, budget, replay, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.workflow_id, "workflow_id"),
        (&request.checkpoint_id, "checkpoint_id"),
        (&request.operator_id, "operator_id"),
        (&request.boundary, "boundary"),
        (&request.request.boundary, "request.boundary"),
    ] {
        validate_text(value, field)?;
    }
    for digest in [&request.approval_reference, &request.replay_identity] {
        if digest.as_str().len() != 64 {
            return Err(FederatedRetrievalWorkflowError::Invalid(
                "federated workflow digest is invalid".into(),
            ));
        }
    }
    if request.approval_reference == ContentHash::of_bytes(b"") {
        return Err(FederatedRetrievalWorkflowError::Invalid(
            "federated workflow approval reference is empty".into(),
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
    fn request() -> FederatedRetrievalWorkflowRequest {
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
            state: EvidenceState::Supported,
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
        FederatedRetrievalWorkflowRequest {
            request: FederatedRetrievalQuery {
                request_id: "request:federated-retrieval-workflow".into(),
                federation_id: "federation:consortium".into(),
                institution_id: "institution:local".into(),
                purpose: "preclinical replication benchmark".into(),
                semantic_profile: "ome-ngff:5".into(),
                endpoint: "https://federation.invalid/admit".into(),
                allowed_artifacts: vec!["qualified-evidence-summary".into()],
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                minimum_support_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                candidates,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signer_valid: true,
                approval_valid: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:federated-retrieval".into(),
            requested_stage_order: vec![
                "stage:checkpoint".into(),
                "stage:admit-federation".into(),
                "stage:retrieve-local-candidates".into(),
                "stage:synthesize-aggregate".into(),
                "stage:validate-envelope".into(),
            ],
            checkpoint_id: "checkpoint:1".into(),
            checkpoint_seq: 1,
            operator_id: "operator:federation".into(),
            approval_reference: hash("approval"),
            budget_units: 12,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let manifest = federated_retrieval_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn approved_workflow_schedules() {
        let receipt = compile_federated_retrieval_workflow(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            FederatedRetrievalDisposition::Qualified
        );
        assert!(receipt.effect_receipts[0].starts_with("schedule:federated-retrieval-work:"));
    }
    #[test]
    fn signer_denial_blocks() {
        let mut input = request();
        input.request.signer_valid = false;
        let receipt = compile_federated_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request();
        input.policy_allow = false;
        let receipt = compile_federated_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
    }
    #[test]
    fn stage_protocol_is_required() {
        let mut input = request();
        input.requested_stage_order.reverse();
        assert!(compile_federated_retrieval_workflow(&input).is_err());
    }
    #[test]
    fn checkpoint_is_required() {
        let mut input = request();
        input.checkpoint_seq = 0;
        assert!(compile_federated_retrieval_workflow(&input).is_err());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request();
        input.raw_data_local = false;
        let receipt = compile_federated_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value == "workflow:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn workflow_plan_and_artifact_payload_are_bound() {
        let mut plan_drift = compile_federated_retrieval_workflow(&request()).unwrap();
        plan_drift
            .plan_order
            .retain(|item| item != "plan:publish-permitted-aggregate");
        assert!(plan_drift.validate().is_err());

        let mut payload_drift = compile_federated_retrieval_workflow(&request()).unwrap();
        payload_drift.endpoint = "https://federation.invalid/other".into();
        assert!(payload_drift.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = compile_federated_retrieval_workflow(&request()).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn padded_operator_identity_is_rejected() {
        let mut input = request();
        input.operator_id.push(' ');
        assert!(compile_federated_retrieval_workflow(&input).is_err());
    }
}
