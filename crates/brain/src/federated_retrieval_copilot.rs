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
const COPILOT_CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-evidence-synthesis-copilot+json";
const MAX_TEXT_BYTES: usize = 512;

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
    pub scope: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub allowed_artifact_order: Vec<String>,
    pub disposition: FederatedRetrievalDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
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
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.len() != 1
            || self.allowed_artifact_order.is_empty()
            || self.budget_units == 0
            || self.effect_receipts.len() != 1
        {
            return Err(FederatedRetrievalCopilotError::Invalid("federated copilot identity, purpose, coverage, bounded plan, tool, budget, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.operator_id, "operator_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self
            .plan_order
            .iter()
            .zip(&self.action_order)
            .any(|(plan, action)| plan.strip_prefix("plan:") != action.strip_prefix("action:"))
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot plan and action orders are not paired".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.plan_order,
            &self.action_order,
            &self.tool_order,
            &self.candidate_order,
            &self.ranked_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.allowed_artifact_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            validate_sorted_unique(values, "federated retrieval copilot collection")?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let ranked_keys = identity_keys(&self.ranked_order);
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if ranked_keys != candidate_keys
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
            || !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self.aggregate_order.len() != self.qualified_order.len()
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot retrieval states are not a disjoint ranked partition".into(),
            ));
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self
                .aggregate_order
                .iter()
                .any(|value| value.as_str().len() != 64)
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated aggregate ordering or digest is invalid".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != FederatedRetrievalDisposition::Blocked
                || !self
                    .negative_evidence
                    .iter()
                    .any(|item| item == "request:raw-data-locality-failed"))
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "non-local retrieval copilots must be blocked and retain locality evidence".into(),
            ));
        }
        let expected_effect = if self.disposition != FederatedRetrievalDisposition::Blocked
            && !self.aggregate_order.is_empty()
        {
            format!("exchange:permitted-artifacts:{}", self.request_id)
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot effect does not match disposition and aggregate".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.synthesis_digest,
            &self.plan_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalCopilotError::Invalid(
                    "federated retrieval copilot digest is invalid".into(),
                ));
            }
        }
        let expected_comparability_digest = ContentHash::of_value(&json!({
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "scope": self.scope,
            "semantic_profile": self.semantic_profile,
        }))
        .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
        if self.comparability_digest != expected_comparability_digest {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot comparability digest is not bound to query".into(),
            ));
        }
        let expected_envelope_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "allowed_artifacts": self.allowed_artifact_order,
            "aggregate_order": self.aggregate_order,
        }))
        .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope_digest {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot envelope digest is not bound to aggregate".into(),
            ));
        }
        let expected_synthesis_digest = ContentHash::of_value(&json!({
            "feature_id": crate::federated_retrieval_synthesis::FEATURE_ID,
            "request_id": self.request_id,
            "ranked_order": self.ranked_order,
            "qualified_order": self.qualified_order,
            "comparability_digest": self.comparability_digest,
            "envelope_digest": self.envelope_digest,
            "disposition": self.disposition,
        }))
        .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
        if self.synthesis_digest != expected_synthesis_digest {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot synthesis digest is not bound to retrieval".into(),
            ));
        }
        let expected_plan_digest = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "plan_order": self.plan_order,
            "action_order": self.action_order,
            "tool_order": self.tool_order,
            "aggregate_order": self.aggregate_order,
            "envelope_digest": self.envelope_digest,
            "budget_units": self.budget_units,
            "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_plan_digest {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot plan digest is not bound to plan".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-retrieval-copilot:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != COPILOT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
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

fn validate_text(value: &str, field: &str) -> Result<(), FederatedRetrievalCopilotError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedRetrievalCopilotError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedRetrievalCopilotError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedRetrievalCopilotError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalCopilotError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedRetrievalCopilotError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}

fn receipt_payload(receipt: &FederatedRetrievalCopilotReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "operator_id": receipt.operator_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "scope": receipt.scope,
        "allowed_artifact_order": receipt.allowed_artifact_order,
        "disposition": receipt.disposition,
        "plan_order": receipt.plan_order,
        "action_order": receipt.action_order,
        "tool_order": receipt.tool_order,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "plan_digest": receipt.plan_digest,
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
    if u64::from(request.budget_units) < u64::try_from(actions.len()).unwrap_or(u64::MAX)
        || actions.len() > request.max_actions
    {
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
        && u64::from(request.budget_units) >= u64::try_from(actions.len()).unwrap_or(u64::MAX)
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
    let exchange_allowed = disposition != FederatedRetrievalDisposition::Blocked
        && !synthesis.aggregate_order.is_empty();
    let effect_receipts = if exchange_allowed {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            request.request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let synthesis_digest = ContentHash::of_value(&json!({
        "feature_id": crate::federated_retrieval_synthesis::FEATURE_ID,
        "request_id": request.request.request_id,
        "ranked_order": synthesis.ranked_order,
        "qualified_order": synthesis.qualified_order,
        "comparability_digest": synthesis.comparability_digest,
        "envelope_digest": synthesis.envelope_digest,
        "disposition": disposition,
    }))
    .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "aggregate_order": synthesis.aggregate_order, "envelope_digest": synthesis.envelope_digest, "budget_units": request.budget_units, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "study_order": request.request.study_ids, "modality_order": request.request.required_modalities, "scope": request.request.scope, "allowed_artifact_order": request.request.allowed_artifacts, "disposition": disposition, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis_digest, "plan_digest": plan_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-retrieval-copilot:{}",
            request.request.request_id
        ),
        COPILOT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalCopilotError::Artifact(error.to_string()))?;
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
        scope: request.request.scope.clone(),
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
        allowed_artifact_order: request
            .request
            .allowed_artifacts
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
        ranked_order: synthesis.ranked_order,
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
        effect_receipts,
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
    if request.max_actions == 0
        || request.max_actions > 128
        || request.budget_units == 0
        || request.request.study_ids.len() < 2
        || request.request.required_modalities.len() < 2
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.replay_identity != request.replay_identity
        || request.request.policy_allow != request.policy_allow
        || request.request.protected_closure != request.protected_closure
        || request.request.raw_data_local != request.raw_data_local
        || request.approval_reference == ContentHash::of_bytes(&[])
        || request.request.candidates.is_empty()
    {
        return Err(FederatedRetrievalCopilotError::Invalid("federated copilot operator, tool, purpose, coverage, budget, candidates, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.request.request_id, "request_id"),
        (&request.operator_id, "operator_id"),
        (&request.request.federation_id, "federation_id"),
        (&request.request.institution_id, "institution_id"),
        (&request.request.purpose, "purpose"),
        (&request.request.semantic_profile, "semantic_profile"),
        (&request.request.endpoint, "endpoint"),
        (&request.request.scope, "scope"),
        (&request.declared_tool_id, "declared_tool_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_sorted_unique(&request.action_allow_list, "action_allow_list")?;
    validate_sorted_unique(&request.request.study_ids, "study_ids")?;
    validate_sorted_unique(&request.request.required_modalities, "required_modalities")?;
    validate_sorted_unique(&request.request.allowed_artifacts, "allowed_artifacts")?;
    for digest in [&request.approval_reference, &request.replay_identity] {
        if digest.as_str().len() != 64 {
            return Err(FederatedRetrievalCopilotError::Invalid(
                "federated copilot approval or replay digest is invalid".into(),
            ));
        }
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
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request();
        q.raw_data_local = false;
        q.request.raw_data_local = false;
        let r = compile_federated_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Blocked);
        assert!(r.raw_data_local);
        assert!(r
            .negative_evidence
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        r.validate().unwrap();
    }
    #[test]
    fn plan_and_payload_drift_are_rejected() {
        let r = compile_federated_retrieval_copilot(&request()).unwrap();
        let mut plan_drift = r.clone();
        plan_drift.action_order.pop();
        assert!(plan_drift.validate().is_err());

        let mut payload_drift = r;
        payload_drift.scope = "organoid:other".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_operator_identity_is_rejected() {
        let mut q = request();
        q.operator_id = " operator:federation".into();
        assert!(compile_federated_retrieval_copilot(&q).is_err());
    }
    #[test]
    fn case_mismatched_ranked_identity_is_rejected() {
        let mut receipt = compile_federated_retrieval_copilot(&request()).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn digest_is_stable() {
        let r = compile_federated_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
