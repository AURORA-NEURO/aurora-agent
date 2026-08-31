//! Deterministic multi-action autonomy admission ledger.
//!
//! Atlas feature: `AFA-policy-P19-F02`.

use crate::autonomy::{admit_autonomy, AutonomyAdmissionRequest};
use bioprism_foundation::{
    AutonomyGrant, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-policy-P19-F02";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchAdmissionAction {
    pub action_id: String,
    pub requested_action: String,
    pub requested_tier: AutonomyTier,
    pub effect: Effect,
    pub evidence_state: EvidenceState,
    pub signed_preflight: Option<ContentHash>,
    pub replay_identity: Option<ContentHash>,
    pub independent_safety_review: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchAdmissionRequest {
    pub grant: AutonomyGrant,
    pub actions: Vec<BatchAdmissionAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchActionDecision {
    Allowed,
    ApprovalRequired,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchActionReceipt {
    pub action_id: String,
    pub requested_action: String,
    pub requested_tier: AutonomyTier,
    pub decision: BatchActionDecision,
    pub reasons: Vec<String>,
    pub evaluated_artifacts: Vec<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchAdmissionReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub actor: String,
    pub total_actions: usize,
    pub allowed_actions: usize,
    pub approval_actions: usize,
    pub denied_actions: usize,
    pub actions: Vec<BatchActionReceipt>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl BatchAdmissionReceipt {
    pub fn validate(&self) -> Result<(), BatchAdmissionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.actor.trim().is_empty()
        {
            return Err(BatchAdmissionError::InvalidField(
                "schema, actor, feature, or boundary".into(),
            ));
        }
        if self.total_actions == 0
            || self.total_actions != self.actions.len()
            || self.allowed_actions + self.approval_actions + self.denied_actions
                != self.total_actions
            || self
                .actions
                .iter()
                .any(|action| action.action_id.trim().is_empty() || action.reasons.is_empty())
        {
            return Err(BatchAdmissionError::InvalidField(
                "action counts or reasons".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| BatchAdmissionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, BatchAdmissionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| BatchAdmissionError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| BatchAdmissionError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum BatchAdmissionError {
    #[error("invalid batch admission field: {0}")]
    InvalidField(String),
    #[error("duplicate batch action {0}")]
    DuplicateAction(String),
    #[error("autonomy grant error: {0}")]
    Grant(String),
    #[error("artifact error: {0}")]
    Artifact(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn autonomy_batch_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "policy".into(),
        consumers: ["research data steward".into(), "runtime scheduler".into()].into(),
        behavior: "evaluates a deterministic batch of typed autonomy actions against one grant and preserves allowed, approval-required, and denied decisions".into(),
        value: "prevents partial batch admission from hiding denied actions or consuming authority beyond a grant".into(),
        inputs: vec![TypedPort { name: "batch_admission_request".into(), schema: "BatchAdmissionRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "batch_admission_receipt".into(), schema: "BatchAdmissionReceipt@1".into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-policy-grant".into(), "write:local-policy-receipt".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: Vec::new(),
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn admit_autonomy_batch(
    request: &BatchAdmissionRequest,
) -> Result<BatchAdmissionReceipt, BatchAdmissionError> {
    request
        .grant
        .validate()
        .map_err(|error| BatchAdmissionError::Grant(error.to_string()))?;
    validate_request(request)?;
    let mut actions = request.actions.clone();
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut results = Vec::with_capacity(actions.len());
    for action in actions {
        let admission = admit_autonomy(&AutonomyAdmissionRequest {
            grant: request.grant.clone(),
            requested_action: action.requested_action.clone(),
            requested_tier: action.requested_tier,
            effect: action.effect,
            evidence_state: action.evidence_state,
            signed_preflight: action.signed_preflight.clone(),
            replay_identity: action.replay_identity.clone(),
            independent_safety_review: action.independent_safety_review.clone(),
        });
        let result = match admission {
            Ok(receipt) => BatchActionReceipt {
                action_id: action.action_id,
                requested_action: action.requested_action,
                requested_tier: action.requested_tier,
                decision: if receipt.decision == bioprism_foundation::PolicyDecision::Allow {
                    BatchActionDecision::Allowed
                } else {
                    BatchActionDecision::ApprovalRequired
                },
                reasons: receipt.reasons,
                evaluated_artifacts: receipt.evaluated_artifacts,
            },
            Err(error) => BatchActionReceipt {
                action_id: action.action_id,
                requested_action: action.requested_action,
                requested_tier: action.requested_tier,
                decision: BatchActionDecision::Denied,
                reasons: vec![error.to_string()],
                evaluated_artifacts: Vec::new(),
            },
        };
        results.push(result);
    }
    let allowed_actions = results
        .iter()
        .filter(|action| action.decision == BatchActionDecision::Allowed)
        .count();
    let approval_actions = results
        .iter()
        .filter(|action| action.decision == BatchActionDecision::ApprovalRequired)
        .count();
    let denied_actions = results
        .iter()
        .filter(|action| action.decision == BatchActionDecision::Denied)
        .count();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "actor": request.grant.actor,
        "total_actions": results.len(),
        "allowed_actions": allowed_actions,
        "approval_actions": approval_actions,
        "denied_actions": denied_actions,
        "actions": results,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("autonomy-batch:{}", request.grant.actor),
        "application/vnd.aurora.autonomy-batch+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| BatchAdmissionError::Artifact(error.to_string()))?;
    let receipt = BatchAdmissionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        actor: request.grant.actor.clone(),
        total_actions: results.len(),
        allowed_actions,
        approval_actions,
        denied_actions,
        actions: serde_json::from_value(payload["actions"].clone())
            .map_err(|error| BatchAdmissionError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &BatchAdmissionRequest) -> Result<(), BatchAdmissionError> {
    if request.actions.is_empty() {
        return Err(BatchAdmissionError::InvalidField(
            "at least one action is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty() || !ids.insert(action.action_id.clone()) {
            return Err(BatchAdmissionError::DuplicateAction(
                action.action_id.clone(),
            ));
        }
        if action.requested_action.trim().is_empty() {
            return Err(BatchAdmissionError::InvalidField(
                "requested action is required".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn grant() -> AutonomyGrant {
        AutonomyGrant {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            actor: "agent:batch".into(),
            permitted_actions: ["local.compute".into(), "instrument.run".into()].into(),
            resource_budget: BTreeMap::from([(String::from("cpu_seconds"), 10.0)]),
            scope: "study:preclinical".into(),
            expires_at: "2027-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: AutonomyTier::A3,
            approval_reference: Some("approval:institution".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn batch_retains_allow_approval_and_denial() {
        let receipt = admit_autonomy_batch(&BatchAdmissionRequest {
            grant: grant(),
            actions: vec![
                BatchAdmissionAction {
                    action_id: "a-allow".into(),
                    requested_action: "local.compute".into(),
                    requested_tier: AutonomyTier::A1,
                    effect: Effect::ExecuteLocalComputation,
                    evidence_state: EvidenceState::Supported,
                    signed_preflight: None,
                    replay_identity: None,
                    independent_safety_review: None,
                },
                BatchAdmissionAction {
                    action_id: "b-approval".into(),
                    requested_action: "instrument.run".into(),
                    requested_tier: AutonomyTier::A3,
                    effect: Effect::InstrumentExecution,
                    evidence_state: EvidenceState::Supported,
                    signed_preflight: None,
                    replay_identity: None,
                    independent_safety_review: None,
                },
                BatchAdmissionAction {
                    action_id: "c-denied".into(),
                    requested_action: "local.compute".into(),
                    requested_tier: AutonomyTier::A1,
                    effect: Effect::ExecuteLocalComputation,
                    evidence_state: EvidenceState::Unknown,
                    signed_preflight: None,
                    replay_identity: None,
                    independent_safety_review: None,
                },
            ],
        })
        .unwrap();
        assert_eq!(
            (
                receipt.allowed_actions,
                receipt.approval_actions,
                receipt.denied_actions
            ),
            (1, 1, 1)
        );
    }
    #[test]
    fn batch_order_is_digest_stable() {
        let mut actions = vec![
            BatchAdmissionAction {
                action_id: "b".into(),
                requested_action: "local.compute".into(),
                requested_tier: AutonomyTier::A1,
                effect: Effect::ExecuteLocalComputation,
                evidence_state: EvidenceState::Supported,
                signed_preflight: None,
                replay_identity: None,
                independent_safety_review: None,
            },
            BatchAdmissionAction {
                action_id: "a".into(),
                requested_action: "local.compute".into(),
                requested_tier: AutonomyTier::A1,
                effect: Effect::ExecuteLocalComputation,
                evidence_state: EvidenceState::Supported,
                signed_preflight: None,
                replay_identity: None,
                independent_safety_review: None,
            },
        ];
        let left = admit_autonomy_batch(&BatchAdmissionRequest {
            grant: grant(),
            actions: actions.clone(),
        })
        .unwrap();
        actions.reverse();
        let right = admit_autonomy_batch(&BatchAdmissionRequest {
            grant: grant(),
            actions,
        })
        .unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }
}
