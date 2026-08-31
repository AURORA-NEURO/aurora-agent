//! Context-to-Decision-Section projection with refinement obligations.
//!
//! Atlas feature: `AFA-brain-P03-F11`. This is a typed projection boundary,
//! not a decision engine: incomplete context becomes a refinement frontier.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F11";
pub const CONTRACT_VERSION: &str = "brain-context-decision-projection/1.0";
const PROJECTION_CONTENT_TYPE: &str = "application/vnd.aurora.context-decision-projection+json";
const MAX_TEXT_BYTES: usize = 512;
const MAX_IDENTIFIERS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDecisionProjectionRequest {
    pub request_id: String,
    pub query_id: String,
    pub goal: String,
    pub context_disposition: String,
    pub selected_context_ids: Vec<String>,
    pub omission_certificate_ids: Vec<String>,
    pub uncertainty_ids: Vec<String>,
    pub dependency_order: Vec<String>,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDecisionProjectionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub query_id: String,
    pub goal: String,
    pub disposition: String,
    pub selected_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub unresolved_obligation_order: Vec<String>,
    pub refinement_frontier_order: Vec<String>,
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
pub enum ContextDecisionProjectionError {
    #[error("invalid context decision projection request: {0}")]
    Invalid(String),
    #[error("context decision projection artifact failed: {0}")]
    Artifact(String),
}

impl ContextDecisionProjectionReceipt {
    pub fn validate(&self) -> Result<(), ContextDecisionProjectionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.selected_order.is_empty()
            || self.refinement_frontier_order.is_empty()
            || self.effect_receipts.is_empty()
            || !self.raw_data_local
            || !matches!(
                self.disposition.as_str(),
                "admitted" | "refinement_required" | "blocked"
            )
        {
            return Err(ContextDecisionProjectionError::Invalid("decision projection identity, obligations, frontier, locality, disposition, or effects are incomplete".into()));
        }
        if [
            &self.selected_order,
            &self.dependency_order,
            &self.unresolved_obligation_order,
            &self.refinement_frontier_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ]
        .iter()
        .any(|values| values.len() > MAX_IDENTIFIERS)
        {
            return Err(ContextDecisionProjectionError::Invalid(
                "decision projection vectors exceed their bounded size".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.query_id, "query_id"),
            (&self.goal, "goal"),
            (&self.disposition, "disposition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.disposition != "admitted" && self.unresolved_obligation_order.is_empty() {
            return Err(ContextDecisionProjectionError::Invalid(
                "non-admitted projection must retain an unresolved obligation".into(),
            ));
        }
        let has_dependency_gap = self
            .selected_order
            .iter()
            .any(|selected| !self.dependency_order.contains(selected));
        let has_dependency_obligation = self
            .unresolved_obligation_order
            .iter()
            .any(|obligation| obligation == "obligation:dependency-closure-incomplete");
        if has_dependency_gap != has_dependency_obligation {
            return Err(ContextDecisionProjectionError::Invalid(
                "decision projection dependency closure and obligation disagree".into(),
            ));
        }
        let frontier_is_none = self.refinement_frontier_order == ["frontier:none"];
        if frontier_is_none != self.unresolved_obligation_order.is_empty() {
            return Err(ContextDecisionProjectionError::Invalid(
                "decision projection refinement frontier does not match obligations".into(),
            ));
        }
        for (values, field) in [
            (&self.selected_order, "selected_order"),
            (&self.dependency_order, "dependency_order"),
            (
                &self.unresolved_obligation_order,
                "unresolved_obligation_order",
            ),
            (&self.refinement_frontier_order, "refinement_frontier_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for digest in [
            &self.context_digest,
            &self.section_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextDecisionProjectionError::Invalid(
                    "decision projection digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == "blocked" {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "project:local-decision-section:{}",
                self.request_id
            )]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextDecisionProjectionError::Invalid(
                "decision projection effect does not match disposition".into(),
            ));
        }
        let expected_section_digest = ContentHash::of_value(&json!({
            "query_id": self.query_id,
            "goal": self.goal,
            "selected_order": self.selected_order,
            "dependency_order": self.dependency_order,
            "obligation_order": self.unresolved_obligation_order,
            "frontier_order": self.refinement_frontier_order,
            "context_digest": self.context_digest,
            "replay_identity": self.replay_identity,
            "disposition": self.disposition,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))?;
        if self.section_digest != expected_section_digest {
            return Err(ContextDecisionProjectionError::Invalid(
                "decision section digest is not bound to projection state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-context-decision-projection:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != PROJECTION_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextDecisionProjectionError::Invalid(
                "decision projection artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextDecisionProjectionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), ContextDecisionProjectionError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ContextDecisionProjectionError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ContextDecisionProjectionError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextDecisionProjectionError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ContextDecisionProjectionError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContextDecisionProjectionError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ContextDecisionProjectionReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "selected_order": receipt.selected_order,
        "dependency_order": receipt.dependency_order,
        "unresolved_obligation_order": receipt.unresolved_obligation_order,
        "refinement_frontier_order": receipt.refinement_frontier_order,
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

pub fn context_decision_projection_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["Decision Section consumer".into(), "researcher".into(), "refinement planner".into()].into(), behavior: "projects qualified or incomplete typed context into a deterministic Decision-Section envelope with unresolved obligations and refinement frontier".into(), value: "makes context usable by downstream research interfaces without manufacturing conclusions from omissions or uncertainty".into(), inputs: vec![TypedPort { name: "context_decision_projection_request".into(), schema: "ContextDecisionProjectionRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_decision_projection_receipt".into(), schema: "ContextDecisionProjectionReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["project:local-decision-section".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn project_context_to_decision_section(
    request: &ContextDecisionProjectionRequest,
) -> Result<ContextDecisionProjectionReceipt, ContextDecisionProjectionError> {
    if request.request_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.selected_context_ids.is_empty()
        || request.selected_context_ids.len() > MAX_IDENTIFIERS
        || request.omission_certificate_ids.len() > MAX_IDENTIFIERS
        || request.uncertainty_ids.len() > MAX_IDENTIFIERS
        || request.dependency_order.len() > MAX_IDENTIFIERS
        || request.context_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextDecisionProjectionError::Invalid(
            "decision projection identity, selected context, digests, or boundary is invalid"
                .into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.query_id, "query_id"),
        (&request.goal, "goal"),
        (&request.context_disposition, "context_disposition"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.selected_context_ids, "selected_context_ids")?;
    validate_unique(
        &request.omission_certificate_ids,
        "omission_certificate_ids",
    )?;
    validate_unique(&request.uncertainty_ids, "uncertainty_ids")?;
    validate_unique(&request.dependency_order, "dependency_order")?;
    let selected = request
        .selected_context_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let dependencies = request
        .dependency_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if selected.len() != request.selected_context_ids.len()
        || dependencies.len() != request.dependency_order.len()
    {
        return Err(ContextDecisionProjectionError::Invalid(
            "decision projection identifiers must be unique and non-empty".into(),
        ));
    }
    let mut obligations = BTreeSet::new();
    let mut frontier = BTreeSet::new();
    let omissions = request
        .omission_certificate_ids
        .iter()
        .map(|id| format!("omission-certificate:{}", id))
        .collect::<BTreeSet<_>>();
    let uncertainty = request
        .uncertainty_ids
        .iter()
        .map(|id| format!("uncertainty:{}", id))
        .collect::<BTreeSet<_>>();
    let negative = BTreeSet::new();
    for id in &omissions {
        obligations.insert(format!("obligation:{}", id));
        frontier.insert("refine:resolve-omission-certificates".into());
    }
    for id in &uncertainty {
        obligations.insert(format!("obligation:{}", id));
        frontier.insert("refine:resolve-uncertainty".into());
    }
    if request.context_disposition != "qualified" {
        obligations.insert(format!(
            "obligation:context-disposition:{}",
            request.context_disposition
        ));
        frontier.insert("refine:compile-qualified-context".into());
    }
    if !selected.is_subset(&dependencies) {
        obligations.insert("obligation:dependency-closure-incomplete".into());
        frontier.insert("refine:complete-dependency-closure".into());
    }
    if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
        obligations.insert("obligation:policy-protected-closure-locality-blocked".into());
        frontier.insert("refine:obtain-policy-and-closure".into());
    }
    if obligations.is_empty() {
        frontier.insert("frontier:none".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            "blocked"
        } else if obligations.is_empty() {
            "admitted"
        } else {
            "refinement_required"
        };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let dependency_order = dependencies.into_iter().collect::<Vec<_>>();
    let unresolved_obligation_order = obligations.into_iter().collect::<Vec<_>>();
    let refinement_frontier_order = frontier.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let section_digest = ContentHash::of_value(&json!({
        "query_id": request.query_id,
        "goal": request.goal,
        "selected_order": selected_order,
        "dependency_order": dependency_order,
        "obligation_order": unresolved_obligation_order,
        "frontier_order": refinement_frontier_order,
        "context_digest": request.context_digest,
        "replay_identity": request.replay_identity,
        "disposition": disposition,
            "raw_data_local": true,
    }))
    .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))?;
    let effects = if disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "project:local-decision-section:{}",
            request.request_id
        )]
    };
    let receipt_without_artifact = ContextDecisionProjectionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        selected_order,
        dependency_order,
        unresolved_obligation_order,
        refinement_frontier_order,
        context_digest: request.context_digest.clone(),
        section_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts: effects,
        artifact: TypedResearchArtifact::from_payload(
            "placeholder",
            PROJECTION_CONTENT_TYPE,
            &json!({}),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))?,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = receipt_payload(&receipt_without_artifact);
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-decision-projection:{}", request.request_id),
        PROJECTION_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextDecisionProjectionError::Artifact(error.to_string()))?;
    let receipt = ContextDecisionProjectionReceipt {
        artifact,
        ..receipt_without_artifact
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ContextDecisionProjectionRequest {
        ContextDecisionProjectionRequest {
            request_id: "request:section".into(),
            query_id: "query:mechanism".into(),
            goal: "inspect preclinical mechanism context".into(),
            context_disposition: "qualified".into(),
            selected_context_ids: vec!["context:a".into()],
            omission_certificate_ids: Vec::new(),
            uncertainty_ids: Vec::new(),
            dependency_order: vec!["context:a".into()],
            context_digest: hash("context"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            context_decision_projection_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_context_is_admitted() {
        let receipt = project_context_to_decision_section(&request()).unwrap();
        assert_eq!(receipt.disposition, "admitted");
        assert_eq!(receipt.refinement_frontier_order, vec!["frontier:none"]);
    }
    #[test]
    fn partial_context_requires_refinement() {
        let mut value = request();
        value.context_disposition = "partial".into();
        value.uncertainty_ids.push("evidence:u".into());
        let receipt = project_context_to_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "refinement_required");
        assert!(!receipt.unresolved_obligation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = project_context_to_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
    }
    #[test]
    fn non_local_context_is_blocked_and_retained() {
        let mut value = request();
        value.raw_data_local = false;
        let receipt = project_context_to_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt
            .unresolved_obligation_order
            .contains(&"obligation:policy-protected-closure-locality-blocked".into()));
    }

    #[test]
    fn projection_artifact_payload_is_bound() {
        let mut receipt = project_context_to_decision_section(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered");
        assert!(matches!(
            receipt.validate(),
            Err(ContextDecisionProjectionError::Artifact(_))
        ));
    }

    #[test]
    fn digest_is_stable() {
        let receipt = project_context_to_decision_section(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
