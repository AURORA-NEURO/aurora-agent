//! Federated policy and autonomy interoperability gateway.
//!
//! Atlas feature: `AFA-adapter-P19-F24`.
//!
//! This gateway evaluates a typed action against a bounded autonomy grant, policy receipt,
//! resource budget, locality, and preclinical boundary. It emits an admission receipt only; it
//! never executes an external action or upgrades an unresolved policy into authority.

use bioprism_foundation::{
    AutonomyGrant, AutonomyTier, PolicyDecision, PolicyReceipt, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P19-F24";
pub const CONTRACT_VERSION: &str = "federated-policy-autonomy-gateway/1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionAndAuthority {
    pub request_id: String,
    pub actor: String,
    pub action_id: String,
    pub action_kind: String,
    pub required_tier: AutonomyTier,
    pub scope: String,
    pub resource_cost: BTreeMap<String, f64>,
    pub grant: AutonomyGrant,
    pub policy: PolicyReceipt,
    pub target_is_local: bool,
    pub signed_preflight: bool,
    pub independent_gate: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyGatewayDecision {
    Allowed,
    ApprovalRequired,
    LocalOnly,
    Denied,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub action_id: String,
    pub decision: PolicyGatewayDecision,
    pub required_tier: AutonomyTier,
    pub permitted_actions: Vec<String>,
    pub budget_order: Vec<String>,
    pub reasons: Vec<String>,
    pub uncertainty: Vec<String>,
    pub effect_receipt: String,
    pub artifact: bioprism_foundation::TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl PolicyGatewayReceipt {
    pub fn validate(&self) -> Result<(), PolicyGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(PolicyGatewayError::Contract(
                "policy gateway identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.action_id.trim().is_empty()
            || self.permitted_actions.is_empty()
            || self.budget_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(PolicyGatewayError::InvalidRequest("policy identity, action, grant, budget, reasons, locality, effects, and boundary are required".into()));
        }
        if self
            .permitted_actions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self.budget_order.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(PolicyGatewayError::InvalidRequest(
                "policy output ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| PolicyGatewayError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, PolicyGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| PolicyGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| PolicyGatewayError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum PolicyGatewayError {
    #[error("invalid policy gateway request: {0}")]
    InvalidRequest(String),
    #[error("policy gateway contract rejected: {0}")]
    Contract(String),
    #[error("policy gateway serialization failed: {0}")]
    Serialization(String),
}

pub fn admit_policy_action(
    input: &ActionAndAuthority,
) -> Result<PolicyGatewayReceipt, PolicyGatewayError> {
    validate_input(input)?;
    let permitted_actions = input
        .grant
        .permitted_actions
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let budget_order = input.resource_cost.keys().cloned().collect::<Vec<_>>();
    let mut reasons = vec![format!(
        "action {} evaluated against {} grant actions",
        input.action_id,
        permitted_actions.len()
    )];
    let mut uncertainty = Vec::new();
    let decision = if input.policy.decision == PolicyDecision::Unresolved {
        PolicyGatewayDecision::Unresolved
    } else if input.policy.decision != PolicyDecision::Allow {
        PolicyGatewayDecision::Denied
    } else if !input.target_is_local {
        PolicyGatewayDecision::LocalOnly
    } else if input.required_tier.requires_signed_preflight() && !input.signed_preflight {
        uncertainty.push("A3/A4 action lacks signed preflight evidence".into());
        PolicyGatewayDecision::ApprovalRequired
    } else if input.required_tier == AutonomyTier::A4 && !input.independent_gate {
        uncertainty.push("A4 action lacks independent safety/replay gate".into());
        PolicyGatewayDecision::ApprovalRequired
    } else if input.required_tier.requires_approval() && input.grant.approval_reference.is_none() {
        uncertainty.push("required autonomy approval reference is absent".into());
        PolicyGatewayDecision::ApprovalRequired
    } else {
        PolicyGatewayDecision::Allowed
    };
    if decision == PolicyGatewayDecision::LocalOnly {
        reasons
            .push("federated target is not permitted; retain the action institution-local".into());
    }
    if decision == PolicyGatewayDecision::Denied {
        reasons.push("policy decision denied the requested action".into());
    }
    if decision == PolicyGatewayDecision::Unresolved {
        reasons.push("unresolved policy cannot authorize an effect".into());
    }
    if !uncertainty.is_empty() {
        reasons.push("authority or safety closure is incomplete".into());
    }
    let effect_receipt = if decision == PolicyGatewayDecision::Allowed {
        "admit_policy_bounded_action_no_execution"
    } else {
        "block_or_localize_action_no_external_effect"
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": input.request_id, "action_id": input.action_id, "decision": decision, "required_tier": input.required_tier, "permitted_actions": permitted_actions, "budget_order": budget_order, "reasons": reasons, "uncertainty": uncertainty, "effect_receipt": effect_receipt, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = bioprism_foundation::TypedResearchArtifact::from_payload(
        format!("policy-gateway:{}", input.request_id),
        "application/vnd.aurora.policy-autonomy-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| PolicyGatewayError::Contract(error.to_string()))?;
    let receipt = PolicyGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: input.request_id.clone(),
        action_id: input.action_id.clone(),
        decision,
        required_tier: input.required_tier,
        permitted_actions,
        budget_order,
        reasons,
        uncertainty,
        effect_receipt: effect_receipt.into(),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_input(input: &ActionAndAuthority) -> Result<(), PolicyGatewayError> {
    if input.request_id.trim().is_empty()
        || input.actor.trim().is_empty()
        || input.action_id.trim().is_empty()
        || input.action_kind.trim().is_empty()
        || input.scope.trim().is_empty()
        || input.resource_cost.is_empty()
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(PolicyGatewayError::InvalidRequest(
            "request, actor, action, scope, budget, and boundary are required".into(),
        ));
    }
    if input.grant.actor != input.actor
        || input.grant.scope != input.scope
        || input.grant.revoked
        || !input.grant.permitted_actions.contains(&input.action_kind)
    {
        return Err(PolicyGatewayError::InvalidRequest(
            "grant actor, scope, revocation, or permitted action mismatch".into(),
        ));
    }
    input
        .grant
        .validate()
        .map_err(|error| PolicyGatewayError::Contract(error.to_string()))?;
    input
        .policy
        .validate()
        .map_err(|error| PolicyGatewayError::Contract(error.to_string()))?;
    for (resource, cost) in &input.resource_cost {
        if !cost.is_finite()
            || *cost < 0.0
            || input
                .grant
                .resource_budget
                .get(resource)
                .copied()
                .unwrap_or(0.0)
                < *cost
        {
            return Err(PolicyGatewayError::InvalidRequest(format!(
                "resource budget is insufficient for {resource}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> ActionAndAuthority {
        ActionAndAuthority {
            request_id: "request:qc".into(),
            actor: "agent:local".into(),
            action_id: "action:compute".into(),
            action_kind: "compute_local".into(),
            required_tier: AutonomyTier::A1,
            scope: "study:local".into(),
            resource_cost: [("cpu_seconds".into(), 10.0)].into_iter().collect(),
            grant: AutonomyGrant {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                actor: "agent:local".into(),
                permitted_actions: ["compute_local".into()].into_iter().collect(),
                resource_budget: [("cpu_seconds".into(), 100.0)].into_iter().collect(),
                scope: "study:local".into(),
                expires_at: "2026-12-31T00:00:00Z".into(),
                revoked: false,
                autonomy_tier: AutonomyTier::A1,
                approval_reference: None,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            policy: PolicyReceipt {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                receipt_id: "policy:qc".into(),
                decision: PolicyDecision::Allow,
                reasons: vec!["bounded local computation".into()],
                evaluated_artifacts: Vec::new(),
                authority_reference: None,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            target_is_local: true,
            signed_preflight: false,
            independent_gate: false,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn allowed_action_is_deterministic() {
        let first = admit_policy_action(&input()).unwrap();
        let second = admit_policy_action(&input()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.decision, PolicyGatewayDecision::Allowed);
    }
    #[test]
    fn unresolved_policy_never_allows() {
        let mut input = input();
        input.policy.decision = PolicyDecision::Unresolved;
        let receipt = admit_policy_action(&input).unwrap();
        assert_eq!(receipt.decision, PolicyGatewayDecision::Unresolved);
    }
    #[test]
    fn external_target_is_local_only() {
        let mut input = input();
        input.target_is_local = false;
        assert_eq!(
            admit_policy_action(&input).unwrap().decision,
            PolicyGatewayDecision::LocalOnly
        );
    }
    #[test]
    fn high_tier_requires_preflight() {
        let mut input = input();
        input.required_tier = AutonomyTier::A3;
        input.grant.autonomy_tier = AutonomyTier::A3;
        input.grant.approval_reference = Some("approval:1".into());
        assert_eq!(
            admit_policy_action(&input).unwrap().decision,
            PolicyGatewayDecision::ApprovalRequired
        );
    }
}
