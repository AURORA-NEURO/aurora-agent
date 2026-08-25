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
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow identity, stages, plan, locality, budget, or effects are incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.stage_order,
            &self.plan_order,
            &self.completed_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.candidate_order,
            &self.admitted_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedWorkflowError::Invalid(
                    "federation workflow ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedWorkflowError::Invalid(
                "aggregate ordering is not canonical".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:research-work:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedWorkflowError::Invalid(
                "federation workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        if self.disposition == FederatedEvidenceDisposition::Qualified
            && !self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("schedule:research-work:"))
        {
            return Err(FederatedWorkflowError::Invalid(
                "qualified federation workflow requires schedule receipt".into(),
            ));
        }
        if self.disposition == FederatedEvidenceDisposition::Blocked
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(FederatedWorkflowError::Invalid(
                "blocked federation workflow must be explicitly blocked".into(),
            ));
        }
        self.artifact
            .validate_metadata()
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
    let completed_order = stage_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut compensation_order = BTreeSet::new();
    for stage in &stage_order {
        plan_order.insert(format!("plan:{stage}"));
    }
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.approval_reference != ContentHash::of_bytes(&[])
        && request.budget_units >= plan_order.len() as u32
        && evidence.disposition != FederatedEvidenceDisposition::Blocked;
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
    } else {
        plan_order.insert("plan:publish-permitted-aggregate".into());
    }
    if request.budget_units < plan_order.len() as u32 {
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
    let disposition = if !actionable {
        FederatedEvidenceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let completed_vec = completed_order.into_iter().collect::<Vec<_>>();
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "replay_identity": request.replay_identity, "federation_id": request.request.federation_id})).map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_vec, "completed_order": completed_vec, "checkpoint_digest": checkpoint_digest, "approval_reference": request.approval_reference, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "endpoint": request.request.endpoint, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_vec, "completed_order": completed_vec, "blocked_order": evidence.blocked_order, "compensation_order": compensation_order, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-evidence-workflow:{}", request.workflow_id),
        "application/vnd.aurora.federated-research-workflow-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedWorkflowError::Artifact(error.to_string()))?;
    let has_compensation = !compensation_order.is_empty();
    let receipt = FederatedWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        endpoint: request.request.endpoint.clone(),
        disposition,
        stage_order: stage_order.clone(),
        plan_order: plan_vec.clone(),
        completed_order: completed_vec,
        blocked_order: evidence.blocked_order.clone(),
        compensation_order: compensation_order.into_iter().collect(),
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
        effect_receipts: if disposition == FederatedEvidenceDisposition::Qualified {
            vec![format!("schedule:research-work:{}", request.workflow_id)]
        } else if has_compensation {
            vec![format!("compensate:research-work:{}", request.workflow_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedWorkflowRequest) -> Result<(), FederatedWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.requested_stage_order
            != STAGE_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || request.budget_units == 0
        || request.approval_reference == ContentHash::of_bytes(&[])
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedWorkflowError::Invalid("federation workflow identity, canonical stages, approval, budget, replay, or boundary is incomplete".into()));
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
