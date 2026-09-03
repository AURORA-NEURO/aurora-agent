//! Local single-study context compilation research workbench.
//!
//! Atlas feature: `AFA-brain-P03-F17`. The workbench exposes evidence-bearing
//! context and Decision-Section state to a researcher without upgrading
//! unresolved context or causing external effects.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F17";
pub const CONTRACT_VERSION: &str = "brain-context-research-workbench/1.0";
const MAX_IDENTIFIERS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkbenchRequest {
    pub session_id: String,
    pub query_id: String,
    pub goal: String,
    pub projection_disposition: String,
    pub selected_context_ids: Vec<String>,
    pub unresolved_obligation_ids: Vec<String>,
    pub refinement_frontier_ids: Vec<String>,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub session_id: String,
    pub query_id: String,
    pub goal: String,
    pub disposition: String,
    pub view_order: Vec<String>,
    pub action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub selected_context_order: Vec<String>,
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
pub enum ContextWorkbenchError {
    #[error("invalid context workbench request: {0}")]
    Invalid(String),
    #[error("context workbench artifact failed: {0}")]
    Artifact(String),
}

impl ContextWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ContextWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.session_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.view_order.is_empty()
            || self.action_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "ready" | "needs_refinement" | "blocked"
            )
        {
            return Err(ContextWorkbenchError::Invalid("workbench identity, view, action, locality, disposition, or effects are incomplete".into()));
        }
        if [
            &self.view_order,
            &self.action_order,
            &self.blocked_action_order,
            &self.selected_context_order,
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
            return Err(ContextWorkbenchError::Invalid(
                "workbench vectors exceed their bounded size".into(),
            ));
        }
        for values in [
            &self.view_order,
            &self.action_order,
            &self.blocked_action_order,
            &self.selected_context_order,
            &self.unresolved_obligation_order,
            &self.refinement_frontier_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextWorkbenchError::Invalid(
                    "workbench vectors are not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.context_digest,
            &self.section_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextWorkbenchError::Invalid(
                    "workbench digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:local-context-workbench:") && effect != "block:unsafe-release"
        }) {
            return Err(ContextWorkbenchError::Invalid(
                "workbench effect is outside read-only view gate".into(),
            ));
        }
        let expected_effects = if self.disposition == "blocked" {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!("view:local-context-workbench:{}", self.session_id)]
        };
        if self.effect_receipts != expected_effects {
            return Err(ContextWorkbenchError::Invalid(
                "workbench effect does not match disposition".into(),
            ));
        }
        let expected_artifact_id = format!("brain-context-workbench:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != "application/vnd.aurora.context-workbench+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextWorkbenchError::Invalid(
                "workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))
    }
}

fn receipt_payload(receipt: &ContextWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "session_id": receipt.session_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "view_order": receipt.view_order,
        "action_order": receipt.action_order,
        "blocked_action_order": receipt.blocked_action_order,
        "selected_context_order": receipt.selected_context_order,
        "unresolved_obligation_order": receipt.unresolved_obligation_order,
        "refinement_frontier_order": receipt.refinement_frontier_order,
        "context_digest": receipt.context_digest,
        "section_digest": receipt.section_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn context_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "research workflow operator".into()].into(), behavior: "renders local context, omissions, refinement frontier, and Decision-Section actions as an evidence-bearing read-only workbench view".into(), value: "lets researchers inspect and refine context without confusing unresolved evidence with a completed decision or triggering external effects".into(), inputs: vec![TypedPort { name: "context_workbench_request".into(), schema: "ResearchWorkbenchSession1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_workbench_receipt".into(), schema: "ContextWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:local-context-workbench".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn render_context_workbench(
    request: &ContextWorkbenchRequest,
) -> Result<ContextWorkbenchReceipt, ContextWorkbenchError> {
    if request.session_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.selected_context_ids.is_empty()
        || request.selected_context_ids.len() > MAX_IDENTIFIERS
        || request.unresolved_obligation_ids.len() > MAX_IDENTIFIERS
        || request.refinement_frontier_ids.len() > MAX_IDENTIFIERS
        || !matches!(
            request.projection_disposition.as_str(),
            "admitted" | "refinement_required" | "blocked"
        )
        || request.context_digest.as_str().len() != 64
        || request.section_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextWorkbenchError::Invalid(
            "workbench identity, selected context, digest, or boundary is invalid".into(),
        ));
    }
    let selected = request
        .selected_context_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let obligations = request
        .unresolved_obligation_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let frontier = request
        .refinement_frontier_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if selected.len() != request.selected_context_ids.len()
        || selected.iter().any(|value| value.trim().is_empty())
    {
        return Err(ContextWorkbenchError::Invalid(
            "workbench selected context identifiers must be unique and non-empty".into(),
        ));
    }
    for (values, field) in [
        (
            &request.unresolved_obligation_ids,
            "unresolved_obligation_ids",
        ),
        (&request.refinement_frontier_ids, "refinement_frontier_ids"),
    ] {
        let mut seen = BTreeSet::new();
        if values
            .iter()
            .any(|value| value.trim().is_empty() || !seen.insert(value.to_ascii_lowercase()))
        {
            return Err(ContextWorkbenchError::Invalid(format!(
                "{field} must contain unique, non-empty identifiers"
            )));
        }
    }
    let mut views = BTreeSet::from([
        "view:context-summary".to_string(),
        "view:evidence-lineage".to_string(),
        "view:replay-identity".to_string(),
        "view:uncertainty-and-omissions".to_string(),
    ]);
    let mut actions = BTreeSet::from([
        "action:inspect-context".to_string(),
        "action:replay-local-projection".to_string(),
    ]);
    let mut blocked_actions = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let negative = BTreeSet::new();
    let disposition = if !request.policy_allow || !request.raw_data_local {
        omissions.insert("workbench:policy-or-locality-blocked".into());
        "blocked"
    } else if request.projection_disposition == "admitted" && obligations.is_empty() {
        actions.insert("action:open-decision-section".into());
        actions.insert("action:export-local-research-object".into());
        "ready"
    } else {
        actions.insert("action:review-omissions".into());
        actions.insert("action:request-context-refinement".into());
        uncertainty.insert("workbench:projection-not-admitted".into());
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
    if !obligations.is_empty() {
        views.insert("view:unresolved-obligations".into());
    }
    if !frontier.is_empty() || !obligations.is_empty() {
        views.insert("view:refinement-frontier".into());
    }
    let effects = if disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "view:local-context-workbench:{}",
            request.session_id
        )]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "session_id": request.session_id, "query_id": request.query_id, "goal": request.goal, "disposition": disposition, "view_order": views, "action_order": actions, "blocked_action_order": blocked_actions, "selected_context_order": selected, "unresolved_obligation_order": obligations, "refinement_frontier_order": frontier, "context_digest": request.context_digest, "section_digest": request.section_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-workbench:{}", request.session_id),
        "application/vnd.aurora.context-workbench+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))?;
    let receipt = ContextWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        session_id: request.session_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        view_order: views.into_iter().collect(),
        action_order: actions.into_iter().collect(),
        blocked_action_order: blocked_actions.into_iter().collect(),
        selected_context_order: selected.into_iter().collect(),
        unresolved_obligation_order: obligations.into_iter().collect(),
        refinement_frontier_order: frontier.into_iter().collect(),
        context_digest: request.context_digest.clone(),
        section_digest: request.section_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: effects,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
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
    fn request() -> ContextWorkbenchRequest {
        let h = hash("digest");
        ContextWorkbenchRequest {
            session_id: "session:one".into(),
            query_id: "query:one".into(),
            goal: "inspect context".into(),
            projection_disposition: "admitted".into(),
            selected_context_ids: vec!["context:a".into()],
            unresolved_obligation_ids: Vec::new(),
            refinement_frontier_ids: Vec::new(),
            context_digest: h.clone(),
            section_digest: h.clone(),
            replay_identity: h,
            policy_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            context_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn admitted_context_exposes_safe_actions() {
        let receipt = render_context_workbench(&request()).unwrap();
        assert_eq!(receipt.disposition, "ready");
        assert!(receipt
            .action_order
            .iter()
            .any(|item| item == "action:open-decision-section"));
    }
    #[test]
    fn obligations_expose_refinement_view() {
        let mut value = request();
        value.projection_disposition = "refinement_required".into();
        value
            .unresolved_obligation_ids
            .push("obligation:missing-evidence".into());
        let receipt = render_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "needs_refinement");
        assert!(receipt
            .view_order
            .iter()
            .any(|item| item == "view:refinement-frontier"));
    }
    #[test]
    fn policy_denial_blocks_actions() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = render_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = render_context_workbench(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
