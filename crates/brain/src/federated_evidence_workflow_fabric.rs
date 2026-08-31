//! Federated continual evidence-surveillance workflow fabric.
//!
//! Atlas feature: `AFA-brain-P01-F16`. This product schedules an aggregate-only,
//! checkpointed federation workflow. Raw experimental data stays at the originating
//! institution and every exchange is policy-, signer-, budget-, and replay-bound.

use crate::federated_evidence_surveillance::{
    admit_federated_evidence, FederatedEvidenceDisposition, FederatedEvidenceFeedRequest,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F16";
pub const CONTRACT_VERSION: &str = "brain-federated-evidence-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "FederationEnvelope1@1";
const WORKFLOW_CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-research-workflow-receipt+json";
const MAX_TEXT_BYTES: usize = 512;
pub const STAGE_ORDER: [&str; 4] = [
    "stage:admit-federation",
    "stage:checkpoint",
    "stage:publish-aggregate",
    "stage:validate-input",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowRequest {
    pub request: FederatedEvidenceFeedRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub approval_reference: ContentHash,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub checkpoint_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub disposition: FederatedEvidenceDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub approval_reference: ContentHash,
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
pub enum FederatedWorkflowError {
    #[error("invalid federated workflow request: {0}")]
    Invalid(String),
    #[error("federated workflow artifact failed: {0}")]
    Artifact(String),
    #[error("federated workflow engine failed: {0}")]
    Engine(String),
}

impl FederatedWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederatedWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.plan_order.is_empty()
            || self.budget_units == 0
        {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow identity, stages, plan, locality, budget, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
            (&self.checkpoint_id, "checkpoint_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.stage_order != STAGE_ORDER {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow stage order is not canonical".into(),
            ));
        }
        if self.completed_order != self.stage_order {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow completed order does not cover stages".into(),
            ));
        }
        validate_sorted_unique(&self.plan_order, "plan_order")?;
        validate_sorted_unique(&self.blocked_order, "blocked_order")?;
        validate_sorted_unique(&self.compensation_order, "compensation_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        validate_sorted_unique(&self.admitted_order, "admitted_order")?;
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
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow plan does not contain exactly one terminal branch".into(),
            ));
        }
        let expected_terminal_plan = if self.admitted_order.is_empty() {
            "plan:retain-unresolved-federation"
        } else if self.disposition == FederatedEvidenceDisposition::Qualified {
            "plan:publish-permitted-aggregate"
        } else {
            "plan:retain-partial-federation"
        };
        if !self
            .plan_order
            .iter()
            .any(|plan| plan == expected_terminal_plan)
        {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow terminal plan does not match disposition and admission".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let admitted_keys = identity_keys(&self.admitted_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if admitted_keys
            .union(&blocked_keys)
            .cloned()
            .collect::<BTreeSet<_>>()
            != candidate_keys
            || !admitted_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self.aggregate_order.len() != self.admitted_order.len()
        {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow states and aggregates do not partition candidates".into(),
            ));
        }
        validate_digest_order(&self.aggregate_order)?;
        if !self.raw_data_local {
            return Err(FederatedWorkflowError::Invalid(
                "federated evidence workflow receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect_receipts =
            if self.disposition == FederatedEvidenceDisposition::Qualified {
                if self.compensation_order.is_empty() {
                    vec![format!("schedule:research-work:{}", self.workflow_id)]
                } else {
                    return Err(FederatedWorkflowError::Invalid(
                        "qualified federation workflows cannot carry compensation".into(),
                    ));
                }
            } else if self.disposition != FederatedEvidenceDisposition::Blocked
                && !self.compensation_order.is_empty()
            {
                vec![format!("compensate:research-work:{}", self.workflow_id)]
            } else {
                vec!["block:unsafe-release".into()]
            };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow effects do not match disposition and compensation".into(),
            ));
        }
        for digest in [
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedWorkflowError::Invalid(
                    "federation workflow digest is invalid".into(),
                ));
            }
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
            "federation_id": self.federation_id,
        }))
        .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(FederatedWorkflowError::Invalid(
                "federation checkpoint digest is not bound to checkpoint state".into(),
            ));
        }
        let expected_workflow_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "checkpoint_digest": self.checkpoint_digest,
            "approval_reference": self.approval_reference,
            "budget_units": self.budget_units,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow digest is not bound to workflow state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-evidence-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedWorkflowError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedWorkflowError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedWorkflowError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), FederatedWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedWorkflowError::Invalid(format!(
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

fn validate_digest_order(values: &[ContentHash]) -> Result<(), FederatedWorkflowError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1])
        || values.iter().any(|value| value.as_str().len() != 64)
    {
        return Err(FederatedWorkflowError::Invalid(
            "federated aggregate ordering or digest is invalid".into(),
        ));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "checkpoint_id": receipt.checkpoint_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "endpoint": receipt.endpoint,
        "disposition": receipt.disposition,
        "stage_order": receipt.stage_order,
        "plan_order": receipt.plan_order,
        "completed_order": receipt.completed_order,
        "blocked_order": receipt.blocked_order,
        "compensation_order": receipt.compensation_order,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "approval_reference": receipt.approval_reference,
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

pub fn federated_evidence_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["federation workflow steward".into(), "consortium research operator".into()].into(),
        behavior: "schedules a checkpointed aggregate-only federation workflow with signed approval and replay receipts".into(),
        value: "turns continual consortium evidence exchange into a bounded, compensating workflow without raw-data movement".into(),
        inputs: vec![TypedPort { name: "federated_workflow_request".into(), schema: "ResearchWorkflowSpec4@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federation_envelope".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["schedule:research-work".into(), "export:permitted-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federation workflow approver".into(), reason: "approve purpose-bound aggregate exchange and compensation policy before scheduling".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_federated_evidence_workflow(
    request: &FederatedWorkflowRequest,
) -> Result<FederatedWorkflowReceipt, FederatedWorkflowError> {
    validate_request(request)?;
    let evidence = admit_federated_evidence(&request.request)
        .map_err(|error| FederatedWorkflowError::Engine(error.to_string()))?;
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let stage_order = STAGE_ORDER
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let mut plan_order = BTreeSet::new();
    let completed_order = stage_order.clone();
    let mut compensation_order = BTreeSet::new();
    for stage in &stage_order {
        plan_order.insert(format!("plan:{stage}"));
    }
    if evidence.disposition == FederatedEvidenceDisposition::Partial
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
    {
        compensation_order.insert("compensate:research-work:federation-partial".into());
        omissions.insert("workflow:federation-partial-requires-compensation".into());
    }
    if evidence.admitted_order.is_empty() {
        plan_order.insert("plan:retain-unresolved-federation".into());
        omissions.insert("workflow:no-admitted-aggregate-to-publish".into());
    } else if request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.approval_reference != ContentHash::of_bytes(&[])
        && u64::from(request.budget_units)
            >= u64::try_from(STAGE_ORDER.len())
                .unwrap_or(u64::MAX)
                .saturating_add(1)
        && evidence.disposition == FederatedEvidenceDisposition::Qualified
    {
        plan_order.insert("plan:publish-permitted-aggregate".into());
    } else {
        plan_order.insert("plan:retain-partial-federation".into());
    }
    let plan_count = u64::try_from(plan_order.len()).unwrap_or(u64::MAX);
    if u64::from(request.budget_units) < plan_count {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if request.approval_reference == ContentHash::of_bytes(&[]) {
        omissions.insert("workflow:approval-missing".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("workflow:raw-data-locality-failed".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || request.approval_reference == ContentHash::of_bytes(&[])
        || u64::from(request.budget_units) < plan_count
        || evidence.disposition == FederatedEvidenceDisposition::Blocked
    {
        FederatedEvidenceDisposition::Blocked
    } else {
        evidence.disposition
    };
    if disposition == FederatedEvidenceDisposition::Blocked {
        compensation_order.clear();
    }
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let completed_vec = completed_order;
    let compensation_vec = compensation_order.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == FederatedEvidenceDisposition::Qualified {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    } else if disposition != FederatedEvidenceDisposition::Blocked && !compensation_vec.is_empty() {
        vec![format!("compensate:research-work:{}", request.workflow_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "replay_identity": request.replay_identity, "federation_id": request.request.federation_id})).map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_vec, "completed_order": completed_vec, "checkpoint_digest": checkpoint_digest, "approval_reference": request.approval_reference, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "endpoint": request.request.endpoint, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_vec, "completed_order": completed_vec, "blocked_order": evidence.blocked_order, "compensation_order": compensation_vec, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-evidence-workflow:{}", request.workflow_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
    let receipt = FederatedWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        endpoint: request.request.endpoint.clone(),
        disposition,
        stage_order: stage_order.clone(),
        plan_order: plan_vec.clone(),
        completed_order: completed_vec,
        blocked_order: evidence.blocked_order.clone(),
        compensation_order: compensation_vec,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        aggregate_order: evidence.aggregate_order.clone(),
        checkpoint_digest,
        workflow_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
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

fn validate_request(request: &FederatedWorkflowRequest) -> Result<(), FederatedWorkflowError> {
    let expected_stage_order = STAGE_ORDER
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if request.requested_stage_order != expected_stage_order
        || request.budget_units == 0
        || request.approval_reference == ContentHash::of_bytes(&[])
        || request.request.replay_identity != request.replay_identity
        || request.request.policy_allow != request.policy_allow
        || request.request.protected_closure != request.protected_closure
        || request.request.raw_data_local != request.raw_data_local
        || !request.request.signer_valid
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedWorkflowError::Invalid(
            "federation workflow identity, canonical stages, approval, budget, replay, policy, signer, locality, or boundary is incomplete".into(),
        ));
    }
    for (value, field) in [
        (&request.request.request_id, "request_id"),
        (&request.workflow_id, "workflow_id"),
        (&request.checkpoint_id, "checkpoint_id"),
        (&request.request.federation_id, "federation_id"),
        (&request.request.institution_id, "institution_id"),
        (&request.request.purpose, "purpose"),
        (&request.request.semantic_profile, "semantic_profile"),
        (&request.request.endpoint, "endpoint"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.request.allowed_artifacts, "allowed_artifacts")?;
    for digest in [&request.approval_reference, &request.replay_identity] {
        if digest.as_str().len() != 64 {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow approval or replay digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(id: &str, state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(observations: Vec<EvidenceObservation>) -> FederatedWorkflowRequest {
        FederatedWorkflowRequest {
            request: FederatedEvidenceFeedRequest {
                request_id: "request:federated-workflow".into(),
                federation_id: "federation:commons".into(),
                institution_id: "institution:a".into(),
                purpose: "benchmarking".into(),
                semantic_profile: "preclinical-evidence/v1".into(),
                endpoint: "https://hub.example/research".into(),
                allowed_artifacts: vec!["qualified-evidence-summary".into()],
                observations,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signer_valid: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:federated".into(),
            requested_stage_order: STAGE_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            checkpoint_id: "checkpoint:1".into(),
            approval_reference: hash("approval"),
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_export_scoped() {
        let manifest = federated_evidence_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
    #[test]
    fn signed_aggregate_workflow_is_scheduled() {
        let receipt = compile_federated_evidence_workflow(&request(vec![observation(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Qualified);
        assert!(receipt.effect_receipts[0].starts_with("schedule:"));
    }
    #[test]
    fn partial_exchange_retains_compensation() {
        let receipt = compile_federated_evidence_workflow(&request(vec![
            observation("a", EvidenceState::Supported),
            observation("b", EvidenceState::Unknown),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Partial);
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn missing_permitted_artifact_blocks() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.request.allowed_artifacts = vec!["raw-data".into()];
        let receipt = compile_federated_evidence_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.raw_data_local = false;
        input.request.raw_data_local = false;
        let receipt = compile_federated_evidence_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workflow:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn plan_and_payload_drift_are_rejected() {
        let receipt = compile_federated_evidence_workflow(&request(vec![observation(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        let mut plan_drift = receipt.clone();
        plan_drift
            .plan_order
            .retain(|item| item != "plan:publish-permitted-aggregate");
        assert!(plan_drift.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.endpoint = "https://other.example/research".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_workflow_identity_is_rejected() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.workflow_id = " workflow:federated".into();
        assert!(compile_federated_evidence_workflow(&input).is_err());
    }
    #[test]
    fn approval_failure_is_explicit() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_federated_evidence_workflow(&input).is_err());
    }
    #[test]
    fn canonical_digest_is_stable() {
        let receipt = compile_federated_evidence_workflow(&request(vec![
            observation("b", EvidenceState::Supported),
            observation("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
