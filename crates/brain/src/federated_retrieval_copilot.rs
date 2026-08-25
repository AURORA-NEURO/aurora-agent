//! Federated continual retrieval research copilot.
//!
//! Atlas feature: `AFA-brain-P02-F12`. It compiles policy-separated retrieval into a bounded
//! declared-tool plan and exports only permitted aggregate artifacts.

use crate::federated_retrieval_synthesis::{
    synthesize_federated_retrieval, FederatedRetrievalDisposition, FederatedRetrievalQuery,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F12";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "FederatedEvidenceSynthesisCopilot1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalCopilotRequest {
    pub request: FederatedRetrievalQuery,
    pub operator_id: String,
    pub action_allow_list: Vec<String>,
    pub declared_tool_id: String,
    pub approval_reference: ContentHash,
    pub max_actions: usize,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: FederatedRetrievalDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub plan_digest: ContentHash,
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
pub enum FederatedRetrievalCopilotError {
    #[error("invalid federated retrieval copilot request: {0}")]
    Invalid(String),
    #[error("federated retrieval copilot artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval copilot engine failed: {0}")]
    Engine(String),
}

impl FederatedRetrievalCopilotReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.is_empty()
            || self.budget_units == 0
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedRetrievalCopilotError::Invalid("federated copilot identity, purpose, coverage, bounded plan, tool, budget, locality, or effects are incomplete".into()));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot state is not covered".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.plan_order,
            &self.action_order,
            &self.tool_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedRetrievalCopilotError::Invalid(
                    "federated copilot ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated aggregate ordering is not canonical".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot effect is outside aggregate exchange gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))
    }
}

pub fn federated_retrieval_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "federation steward".into()].into(), behavior: "compiles federated continual retrieval into a bounded declared-tool plan with purpose, signer, approval, semantic profile, aggregate-only, replay, locality, and denial gates".into(), value: "automates consortium retrieval without raw-data egress or unreviewed federation effects".into(), inputs: vec![TypedPort { name: "federated_retrieval_copilot_request".into(), schema: "FederatedRetrievalQuery1@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_synthesis_copilot_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federated retrieval tool approver".into(), reason: "authorize purpose-bound aggregate-only retrieval exchange after signer, comparability, and locality gates close".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_federated_retrieval_copilot(
    request: &FederatedRetrievalCopilotRequest,
) -> Result<FederatedRetrievalCopilotReceipt, FederatedRetrievalCopilotError> {
    validate_request(request)?;
    let synthesis = synthesize_federated_retrieval(&request.request)
        .map_err(|error| FederatedRetrievalCopilotError::Engine(error.to_string()))?;
    let mut actions = BTreeSet::new();
    let mut plans = BTreeSet::new();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for evidence_id in &synthesis.qualified_order {
        actions.insert(format!("action:inspect-federated:{evidence_id}"));
        plans.insert(format!("plan:inspect-federated:{evidence_id}"));
    }
    actions.insert("action:exchange-permitted-aggregate".into());
    plans.insert("plan:exchange-permitted-aggregate".into());
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-federated-evidence")
    {
        negative.insert("copilot:inspect-federated-evidence-not-allowed".into());
    }
    if request.budget_units < actions.len() as u32 || actions.len() > request.max_actions {
        omissions.insert("copilot:action-budget-exhausted".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let actionable = request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-federated-evidence")
        && request.approval_reference != ContentHash::of_bytes(b"")
        && request.request.signer_valid
        && request.request.approval_valid
        && request.budget_units >= actions.len() as u32
        && actions.len() <= request.max_actions
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local;
    let disposition = if !actionable {
        FederatedRetrievalDisposition::Blocked
    } else {
        synthesis.disposition
    };
    let plan_order = plans.into_iter().collect::<Vec<_>>();
    let action_order = actions.into_iter().collect::<Vec<_>>();
    let tool_order = vec![format!("tool:{}", request.declared_tool_id)];
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| FederatedRetrievalCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "aggregate_order": synthesis.aggregate_order, "envelope_digest": synthesis.envelope_digest, "budget_units": request.budget_units, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "study_order": request.request.study_ids, "modality_order": request.request.required_modalities, "disposition": disposition, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis_digest, "plan_digest": plan_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-retrieval-copilot:{}",
            request.request.request_id
        ),
        "application/vnd.aurora.federated-evidence-synthesis-copilot+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
    let exchange_allowed = actionable && !synthesis.aggregate_order.is_empty();
    let receipt = FederatedRetrievalCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        endpoint: request.request.endpoint.clone(),
        study_order: request
            .request
            .study_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        modality_order: request
            .request
            .required_modalities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        disposition,
        plan_order,
        action_order,
        tool_order,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        aggregate_order: synthesis.aggregate_order,
        comparability_digest: synthesis.comparability_digest,
        envelope_digest: synthesis.envelope_digest,
        synthesis_digest,
        plan_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if exchange_allowed {
            vec![format!(
                "exchange:permitted-artifacts:{}",
                request.request.request_id
            )]
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

fn validate_request(
    request: &FederatedRetrievalCopilotRequest,
) -> Result<(), FederatedRetrievalCopilotError> {
    if request.operator_id.trim().is_empty()
        || request.declared_tool_id.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > 128
        || request.budget_units == 0
        || request.request.federation_id.trim().is_empty()
        || request.request.institution_id.trim().is_empty()
        || request.request.study_ids.len() < 2
        || request.request.required_modalities.len() < 2
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.candidates.is_empty()
    {
        return Err(FederatedRetrievalCopilotError::Invalid("federated copilot operator, tool, purpose, coverage, budget, candidates, or boundary is incomplete".into()));
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
    fn request() -> FederatedRetrievalCopilotRequest {
        FederatedRetrievalCopilotRequest {
            request: FederatedRetrievalQuery {
                request_id: "request:fed-copilot".into(),
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
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:a".into(),
                    scope: "organoid:neural".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signer_valid: true,
                approval_valid: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operator_id: "operator:federation".into(),
            action_allow_list: vec!["inspect-federated-evidence".into()],
            declared_tool_id: "tool:federated-review".into(),
            approval_reference: hash("approval"),
            max_actions: 8,
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let m = federated_retrieval_copilot_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn approved_plan_is_partial_or_qualified() {
        let r = compile_federated_retrieval_copilot(&request()).unwrap();
        assert!(matches!(
            r.disposition,
            FederatedRetrievalDisposition::Partial | FederatedRetrievalDisposition::Qualified
        ));
    }
    #[test]
    fn signer_blocks() {
        let mut q = request();
        q.request.signer_valid = false;
        let r = compile_federated_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Blocked);
    }
    #[test]
    fn approval_blocks() {
        let mut q = request();
        q.request.approval_valid = false;
        let r = compile_federated_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Blocked);
    }
    #[test]
    fn missing_permission_blocks() {
        let mut q = request();
        q.action_allow_list.clear();
        let r = compile_federated_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = compile_federated_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
