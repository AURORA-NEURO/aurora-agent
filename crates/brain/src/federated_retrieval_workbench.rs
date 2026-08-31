//! Federated retrieval researcher workbench.
//!
//! Atlas feature: `AFA-brain-P02-F20`. The workbench renders aggregate comparison and exchange
//! lineage while keeping raw observations local and federation denial visible.

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

pub const FEATURE_ID: &str = "AFA-brain-P02-F20";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-research-workbench/1.0";
const WORKBENCH_CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-retrieval-workbench-receipt+json";
const MAX_TEXT_BYTES: usize = 512;
pub const VIEW_ORDER: [&str; 3] = [
    "view:aggregate-comparison",
    "view:exchange-lineage",
    "view:institution-coverage",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalWorkbenchRequest {
    pub request: FederatedRetrievalQuery,
    pub workspace_id: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: FederatedRetrievalDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub workbench_digest: ContentHash,
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
pub enum FederatedRetrievalWorkbenchError {
    #[error("invalid federated retrieval workbench request: {0}")]
    Invalid(String),
    #[error("federated retrieval workbench artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval workbench engine failed: {0}")]
    Engine(String),
}

impl FederatedRetrievalWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.panel_order.is_empty()
            || self.action_receipts.is_empty()
            || self.candidate_order.is_empty()
            || self.budget_units == 0
        {
            return Err(FederatedRetrievalWorkbenchError::Invalid("federated workbench identity, coverage, views, panels, retrieval, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workspace_id, "workspace_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.view_order != VIEW_ORDER {
            return Err(FederatedRetrievalWorkbenchError::Invalid(
                "federated workbench view order is not canonical".into(),
            ));
        }
        validate_sorted_unique(&self.panel_order, "panel_order")?;
        validate_sorted_unique(&self.action_receipts, "action_receipts")?;
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        validate_unique(&self.ranked_order, "ranked_order")?;
        validate_unique(&self.qualified_order, "qualified_order")?;
        validate_sorted_unique(&self.blocked_order, "blocked_order")?;
        validate_sorted_unique(&self.unknown_order, "unknown_order")?;
        for (values, field) in [
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
        ] {
            validate_sorted_unique(values, field)?;
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
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
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
            || self.aggregate_order.len() != self.qualified_order.len()
        {
            return Err(FederatedRetrievalWorkbenchError::Invalid(
                "federated workbench ranking and state do not partition candidates".into(),
            ));
        }
        validate_digest_order(&self.aggregate_order)?;
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.synthesis_digest,
            &self.workbench_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalWorkbenchError::Invalid(
                    "federated workbench digest is invalid".into(),
                ));
            }
        }
        if !self.raw_data_local {
            return Err(FederatedRetrievalWorkbenchError::Invalid(
                "federated workbench receipts must declare that emitted data is local".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition != FederatedRetrievalDisposition::Blocked
        {
            vec![format!(
                "view:local-federated-retrieval-artifacts:{}",
                self.workspace_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedRetrievalWorkbenchError::Invalid(
                "federated workbench effect does not match disposition".into(),
            ));
        }
        let expected_workbench_digest = ContentHash::of_value(&json!({
            "workspace_id": self.workspace_id,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "action_receipts": self.action_receipts,
            "comparability_digest": self.comparability_digest,
            "envelope_digest": self.envelope_digest,
            "synthesis_digest": self.synthesis_digest,
            "replay_identity": self.replay_identity,
            "budget_units": self.budget_units,
        }))
        .map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))?;
        if self.workbench_digest != expected_workbench_digest {
            return Err(FederatedRetrievalWorkbenchError::Invalid(
                "federated workbench digest is not bound to its view state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-retrieval-workbench:{}", self.workspace_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKBENCH_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedRetrievalWorkbenchError::Invalid(
                "federated workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedRetrievalWorkbenchError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedRetrievalWorkbenchError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedRetrievalWorkbenchError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedRetrievalWorkbenchError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalWorkbenchError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedRetrievalWorkbenchError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn validate_digest_order(values: &[ContentHash]) -> Result<(), FederatedRetrievalWorkbenchError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1])
        || values.iter().any(|value| value.as_str().len() != 64)
    {
        return Err(FederatedRetrievalWorkbenchError::Invalid(
            "federated aggregate ordering or digest is invalid".into(),
        ));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedRetrievalWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workspace_id": receipt.workspace_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "endpoint": receipt.endpoint,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "disposition": receipt.disposition,
        "view_order": receipt.view_order,
        "panel_order": receipt.panel_order,
        "action_receipts": receipt.action_receipts,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "workbench_digest": receipt.workbench_digest,
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

pub fn federated_retrieval_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["federation steward".into(), "multisite retrieval researcher".into()].into(), behavior: "renders aggregate comparison, exchange lineage, and institution coverage views with deterministic read-only receipts and explicit federation denial".into(), value: "gives consortium researchers an auditable federated retrieval surface without exposing raw observations or hiding denied closure".into(), inputs: vec![TypedPort { name: "federated_retrieval_workbench_request".into(), schema: "ResearchWorkbenchSpec4@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_retrieval_workbench_receipt".into(), schema: "FederatedRetrievalWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:local-federated-retrieval-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_federated_retrieval_workbench(
    request: &FederatedRetrievalWorkbenchRequest,
) -> Result<FederatedRetrievalWorkbenchReceipt, FederatedRetrievalWorkbenchError> {
    validate_request(request)?;
    let synthesis = synthesize_federated_retrieval(&request.request)
        .map_err(|error| FederatedRetrievalWorkbenchError::Engine(error.to_string()))?;
    let view_order = request.requested_view_order.clone();
    let panel_order = request.requested_panel_order.clone();
    let action_receipts = [
        "action:render-aggregate-comparison",
        "action:render-exchange-lineage",
        "action:render-institution-coverage",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
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
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.request.policy_allow
        && request.request.protected_closure
        && request.request.raw_data_local
        && u64::from(request.budget_units)
            >= u64::try_from(action_receipts.len()).unwrap_or(u64::MAX)
        && synthesis.disposition != FederatedRetrievalDisposition::Blocked;
    if u64::from(request.budget_units) < u64::try_from(action_receipts.len()).unwrap_or(u64::MAX) {
        omissions.insert("workbench:budget-exhausted".into());
    }
    if !request.policy_allow || !request.request.policy_allow {
        omissions.insert("workbench:policy-denied".into());
    }
    if !request.protected_closure || !request.request.protected_closure {
        omissions.insert("workbench:protected-closure-incomplete".into());
    }
    if !request.raw_data_local || !request.request.raw_data_local {
        omissions.insert("workbench:raw-data-locality-failed".into());
    }
    let disposition = if actionable {
        synthesis.disposition
    } else {
        FederatedRetrievalDisposition::Blocked
    };
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| FederatedRetrievalWorkbenchError::Engine(error.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({"workspace_id": request.workspace_id, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units})).map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition != FederatedRetrievalDisposition::Blocked {
        vec![format!(
            "view:local-federated-retrieval-artifacts:{}",
            request.workspace_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workspace_id": request.workspace_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "endpoint": request.request.endpoint, "study_order": request.request.study_ids, "modality_order": request.request.required_modalities, "disposition": disposition, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis_digest, "workbench_digest": workbench_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-retrieval-workbench:{}",
            request.workspace_id
        ),
        WORKBENCH_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalWorkbenchError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
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
        view_order,
        panel_order,
        action_receipts,
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        aggregate_order: synthesis.aggregate_order,
        comparability_digest: synthesis.comparability_digest,
        envelope_digest: synthesis.envelope_digest,
        synthesis_digest,
        workbench_digest,
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
    request: &FederatedRetrievalWorkbenchRequest,
) -> Result<(), FederatedRetrievalWorkbenchError> {
    if request.requested_view_order
        != VIEW_ORDER
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
        || request.requested_panel_order.is_empty()
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedRetrievalWorkbenchError::Invalid("federated workbench identity, canonical views, panels, budget, replay, or boundary is incomplete".into()));
    }
    validate_text(&request.workspace_id, "workspace_id")?;
    validate_text(&request.boundary, "boundary")?;
    validate_sorted_unique(&request.requested_panel_order, "requested_panel_order")?;
    if request.replay_identity.as_str().len() != 64
        || request.request.replay_identity.as_str().len() != 64
    {
        return Err(FederatedRetrievalWorkbenchError::Invalid(
            "federated workbench replay identity is invalid".into(),
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
    fn request() -> FederatedRetrievalWorkbenchRequest {
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
        FederatedRetrievalWorkbenchRequest {
            request: FederatedRetrievalQuery {
                request_id: "request:federated-workbench".into(),
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
            workspace_id: "workspace:federated-retrieval".into(),
            requested_view_order: VIEW_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            requested_panel_order: vec!["panel:aggregate".into(), "panel:lineage".into()]
                .into_iter()
                .collect(),
            replay_identity: hash("replay"),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        let manifest = federated_retrieval_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn approved_view_is_read_only() {
        let receipt = compile_federated_retrieval_workbench(&request()).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("view:local-federated-retrieval-artifacts:"));
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request();
        input.policy_allow = false;
        let receipt = compile_federated_retrieval_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
    }
    #[test]
    fn denied_exchange_remains_visible() {
        let mut input = request();
        input.request.signer_valid = false;
        let receipt = compile_federated_retrieval_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn view_protocol_is_required() {
        let mut input = request();
        input.requested_view_order.reverse();
        assert!(compile_federated_retrieval_workbench(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request();
        input.replay_identity = hash("different");
        assert!(compile_federated_retrieval_workbench(&input).is_err());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request();
        input.raw_data_local = false;
        let receipt = compile_federated_retrieval_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value == "workbench:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn workbench_state_and_artifact_payload_are_bound() {
        let mut state_drift = compile_federated_retrieval_workbench(&request()).unwrap();
        state_drift.panel_order.reverse();
        assert!(state_drift.validate().is_err());

        let mut payload_drift = compile_federated_retrieval_workbench(&request()).unwrap();
        payload_drift.endpoint = "https://federation.invalid/other".into();
        assert!(payload_drift.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = compile_federated_retrieval_workbench(&request()).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn padded_workspace_identity_is_rejected() {
        let mut input = request();
        input.workspace_id.push(' ');
        assert!(compile_federated_retrieval_workbench(&input).is_err());
    }
}
