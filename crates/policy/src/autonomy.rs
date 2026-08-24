//! Risk-tiered autonomy admission for preclinical research workflows.
//!
//! Atlas feature: `AFA-policy-P19-F01`.
//!
//! This is an admission gate, not an agent planner. It proves that a proposed
//! action is inside a grant and that the evidence and safety artifacts needed
//! by its risk tier exist before a runner can obtain an allow receipt.

use bioprism_foundation::{
    AutonomyGrant, AutonomyTier, Effect, EvidenceState, PolicyDecision, PolicyReceipt,
    ResearchContractError, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-policy-P19-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AutonomyError {
    #[error(transparent)]
    Contract(#[from] ResearchContractError),
    #[error("requested action `{action}` is not present in the autonomy grant")]
    ActionNotGranted { action: String },
    #[error("requested tier {requested:?} exceeds grant tier {granted:?}")]
    TierExceedsGrant {
        requested: AutonomyTier,
        granted: AutonomyTier,
    },
    #[error("grant has been revoked")]
    Revoked,
    #[error("unknown or contradictory evidence cannot authorize an autonomous action")]
    EvidenceNotAdmissible { state: EvidenceState },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomyAdmissionRequest {
    pub grant: AutonomyGrant,
    pub requested_action: String,
    pub requested_tier: AutonomyTier,
    pub effect: Effect,
    pub evidence_state: EvidenceState,
    pub signed_preflight: Option<ContentHash>,
    pub replay_identity: Option<ContentHash>,
    pub independent_safety_review: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyAdmissionReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub actor: String,
    pub requested_action: String,
    pub requested_tier: AutonomyTier,
    pub decision: PolicyDecision,
    pub reasons: Vec<String>,
    pub evaluated_artifacts: Vec<ContentHash>,
    pub policy_receipt: PolicyReceipt,
    pub boundary: String,
}

impl AutonomyAdmissionReceipt {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.feature_id != FEATURE_ID || self.actor.trim().is_empty() {
            return Err(ResearchContractError::MissingField { field: "actor" });
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || self.policy_receipt.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.actor.clone(),
            });
        }
        self.policy_receipt.validate()
    }

    pub fn digest(&self) -> Result<ContentHash, AutonomyError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            AutonomyError::Contract(ResearchContractError::Serialization {
                item: "autonomy_admission_receipt".into(),
                message: error.to_string(),
            })
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            AutonomyError::Contract(ResearchContractError::Serialization {
                item: "autonomy_admission_receipt".into(),
                message: error.to_string(),
            })
        })
    }
}

pub fn admit_autonomy(
    request: &AutonomyAdmissionRequest,
) -> Result<AutonomyAdmissionReceipt, AutonomyError> {
    request.grant.validate()?;
    if request.grant.revoked {
        return Err(AutonomyError::Revoked);
    }
    if !request
        .grant
        .permitted_actions
        .contains(&request.requested_action)
    {
        return Err(AutonomyError::ActionNotGranted {
            action: request.requested_action.clone(),
        });
    }
    if request.requested_tier > request.grant.autonomy_tier {
        return Err(AutonomyError::TierExceedsGrant {
            requested: request.requested_tier,
            granted: request.grant.autonomy_tier,
        });
    }
    if matches!(
        request.evidence_state,
        EvidenceState::Unknown | EvidenceState::Contradicted
    ) {
        return Err(AutonomyError::EvidenceNotAdmissible {
            state: request.evidence_state,
        });
    }

    let mut reasons = vec![format!("grant {} admits action", request.grant.actor)];
    let mut artifacts = Vec::new();
    let mut decision = PolicyDecision::Allow;
    if request.requested_tier.requires_signed_preflight() {
        match request.signed_preflight.clone() {
            Some(hash) => artifacts.push(hash),
            None => {
                decision = PolicyDecision::ApprovalRequired;
                reasons.push("signed instrument preflight is required".into());
            }
        }
        match request.replay_identity.clone() {
            Some(hash) => artifacts.push(hash),
            None => {
                decision = PolicyDecision::ApprovalRequired;
                reasons.push("replay identity is required for high-autonomy execution".into());
            }
        }
        match request.independent_safety_review.clone() {
            Some(hash) => artifacts.push(hash),
            None => {
                decision = PolicyDecision::ApprovalRequired;
                reasons.push("independent safety review is required".into());
            }
        }
    }
    if matches!(
        request.effect,
        Effect::InstrumentExecution | Effect::ConsumeMaterial
    ) && request.signed_preflight.is_none()
    {
        decision = PolicyDecision::ApprovalRequired;
        reasons.push("physical effects require signed preflight".into());
    }
    let receipt = PolicyReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        receipt_id: format!(
            "autonomy:{}:{}",
            request.grant.actor, request.requested_action
        ),
        decision,
        reasons: reasons.clone(),
        evaluated_artifacts: artifacts.clone(),
        authority_reference: if decision == PolicyDecision::Allow {
            request.grant.approval_reference.clone()
        } else {
            None
        },
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    let output = AutonomyAdmissionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        actor: request.grant.actor.clone(),
        requested_action: request.requested_action.clone(),
        requested_tier: request.requested_tier,
        decision,
        reasons,
        evaluated_artifacts: artifacts,
        policy_receipt: receipt,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn grant(tier: AutonomyTier) -> AutonomyGrant {
        AutonomyGrant {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            actor: "agent:researcher".into(),
            permitted_actions: ["local.compute".into()].into(),
            resource_budget: BTreeMap::from([(String::from("cpu_seconds"), 10.0)]),
            scope: "study:synthetic-preclinical".into(),
            expires_at: "2027-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: tier,
            approval_reference: if tier.requires_approval() {
                Some("approval:institutional-review".into())
            } else {
                None
            },
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn a_bounded_local_action_can_be_admitted_at_a1() {
        let output = admit_autonomy(&AutonomyAdmissionRequest {
            grant: grant(AutonomyTier::A1),
            requested_action: "local.compute".into(),
            requested_tier: AutonomyTier::A1,
            effect: Effect::ExecuteLocalComputation,
            evidence_state: EvidenceState::Supported,
            signed_preflight: None,
            replay_identity: None,
            independent_safety_review: None,
        })
        .unwrap();
        assert_eq!(output.decision, PolicyDecision::Allow);
        output.validate().unwrap();
    }

    #[test]
    fn a3_refuses_to_allow_without_all_high_risk_artifacts() {
        let output = admit_autonomy(&AutonomyAdmissionRequest {
            grant: grant(AutonomyTier::A3),
            requested_action: "local.compute".into(),
            requested_tier: AutonomyTier::A3,
            effect: Effect::InstrumentExecution,
            evidence_state: EvidenceState::Supported,
            signed_preflight: None,
            replay_identity: None,
            independent_safety_review: None,
        })
        .unwrap();
        assert_eq!(output.decision, PolicyDecision::ApprovalRequired);
        assert!(output.reasons.len() >= 3);
    }

    #[test]
    fn unknown_evidence_cannot_be_converted_into_an_allow() {
        let result = admit_autonomy(&AutonomyAdmissionRequest {
            grant: grant(AutonomyTier::A1),
            requested_action: "local.compute".into(),
            requested_tier: AutonomyTier::A1,
            effect: Effect::ExecuteLocalComputation,
            evidence_state: EvidenceState::Unknown,
            signed_preflight: None,
            replay_identity: None,
            independent_safety_review: None,
        });
        assert!(matches!(
            result,
            Err(AutonomyError::EvidenceNotAdmissible { .. })
        ));
    }

    #[test]
    fn identical_receipts_have_identical_digests() {
        let request = AutonomyAdmissionRequest {
            grant: grant(AutonomyTier::A1),
            requested_action: "local.compute".into(),
            requested_tier: AutonomyTier::A1,
            effect: Effect::ExecuteLocalComputation,
            evidence_state: EvidenceState::Supported,
            signed_preflight: None,
            replay_identity: None,
            independent_safety_review: None,
        };
        assert_eq!(
            admit_autonomy(&request).unwrap().digest().unwrap(),
            admit_autonomy(&request).unwrap().digest().unwrap()
        );
    }
}
