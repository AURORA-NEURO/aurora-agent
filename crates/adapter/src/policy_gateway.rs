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
use serde_json::{json, Value};
use std::collections::BTreeMap;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P19-F24";
pub const CONTRACT_VERSION: &str = "federated-policy-autonomy-gateway/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;

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
    pub input: ActionAndAuthority,
    pub input_digest: ContentHash,
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
        validate_text("request_id", &self.request_id)?;
        validate_text("action_id", &self.action_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_text("effect_receipt", &self.effect_receipt)?;
        validate_sorted_strings("permitted_actions", &self.permitted_actions)?;
        validate_sorted_strings("budget_order", &self.budget_order)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        if self.effect_receipt != expected_effect_receipt(self.decision) {
            return Err(PolicyGatewayError::InvalidRequest(
                "policy effect receipt does not match its decision".into(),
            ));
        }
        if self.artifact.artifact_id != format!("policy-gateway:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.policy-autonomy-receipt+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(PolicyGatewayError::Contract(
                "policy artifact is not bound to the gateway receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| PolicyGatewayError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| PolicyGatewayError::Contract(error.to_string()))?;
        if self.input_digest != action_input_digest(&self.input)? {
            return Err(PolicyGatewayError::Contract(
                "policy gateway retained input digest mismatch".into(),
            ));
        }
        validate_input(&self.input)?;
        let expected = evaluate_policy_action(&self.input, false)?;
        if self != &expected {
            return Err(PolicyGatewayError::Contract(
                "policy gateway receipt does not match its retained action and authority input"
                    .into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, PolicyGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| PolicyGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| PolicyGatewayError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), PolicyGatewayError> {
    if value.is_empty() || value.trim() != value {
        return Err(PolicyGatewayError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(PolicyGatewayError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), PolicyGatewayError> {
    if values.len() > MAX_ITEMS {
        return Err(PolicyGatewayError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for value in values {
        validate_text(field, value)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PolicyGatewayError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn expected_effect_receipt(decision: PolicyGatewayDecision) -> &'static str {
    if decision == PolicyGatewayDecision::Allowed {
        "admit_policy_bounded_action_no_execution"
    } else {
        "block_or_localize_action_no_external_effect"
    }
}

fn receipt_payload(receipt: &PolicyGatewayReceipt) -> Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "action_id": receipt.action_id,
        "decision": receipt.decision,
        "required_tier": receipt.required_tier,
        "permitted_actions": receipt.permitted_actions,
        "budget_order": receipt.budget_order,
        "reasons": receipt.reasons,
        "uncertainty": receipt.uncertainty,
        "effect_receipt": receipt.effect_receipt,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
        "input_digest": receipt.input_digest,
    })
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
    evaluate_policy_action(input, true)
}

fn evaluate_policy_action(
    input: &ActionAndAuthority,
    validate_output: bool,
) -> Result<PolicyGatewayReceipt, PolicyGatewayError> {
    validate_input(input)?;
    let input_digest = action_input_digest(input)?;
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
    let effect_receipt = expected_effect_receipt(decision);
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": input.request_id, "action_id": input.action_id, "decision": decision, "required_tier": input.required_tier, "permitted_actions": permitted_actions, "budget_order": budget_order, "reasons": reasons, "uncertainty": uncertainty, "effect_receipt": effect_receipt, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY, "input_digest": input_digest });
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
        input: input.clone(),
        input_digest,
    };
    if validate_output {
        receipt.validate()?;
    }
    Ok(receipt)
}

fn action_input_digest(input: &ActionAndAuthority) -> Result<ContentHash, PolicyGatewayError> {
    let value = serde_json::to_value(input)
        .map_err(|error| PolicyGatewayError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| PolicyGatewayError::Serialization(error.to_string()))
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
    validate_text("request_id", &input.request_id)?;
    validate_text("actor", &input.actor)?;
    validate_text("action_id", &input.action_id)?;
    validate_text("action_kind", &input.action_kind)?;
    validate_text("scope", &input.scope)?;
    validate_text("boundary", &input.boundary)?;
    if input.resource_cost.len() > MAX_ITEMS {
        return Err(PolicyGatewayError::InvalidRequest(
            "resource cost exceeds its item bound".into(),
        ));
    }
    for resource in input.resource_cost.keys() {
        validate_text("resource", resource)?;
    }
    for action in &input.grant.permitted_actions {
        validate_text("grant.permitted_action", action)?;
    }
    for resource in input.grant.resource_budget.keys() {
        validate_text("grant.resource", resource)?;
    }
    if input.grant.autonomy_tier < input.required_tier {
        return Err(PolicyGatewayError::InvalidRequest(
            "autonomy grant does not cover the requested tier".into(),
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

    #[test]
    fn lower_tier_grant_cannot_cover_higher_requested_tier() {
        let mut input = input();
        input.required_tier = AutonomyTier::A3;
        let error = admit_policy_action(&input).unwrap_err();
        assert!(error
            .to_string()
            .contains("does not cover the requested tier"));
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = admit_policy_action(&input()).unwrap();
        receipt.reasons.push("tampered receipt".into());
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn receipt_rejects_a_decision_effect_mismatch() {
        let mut receipt = admit_policy_action(&input()).unwrap();
        receipt.effect_receipt = "block_or_localize_action_no_external_effect".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("effect receipt"));
    }

    #[test]
    fn receipt_rejects_tampered_retained_locality_input() {
        let mut receipt = admit_policy_action(&input()).unwrap();
        receipt.input.target_is_local = false;
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }

    #[test]
    fn receipt_rejects_tampered_retained_budget_input() {
        let mut receipt = admit_policy_action(&input()).unwrap();
        receipt
            .input
            .grant
            .resource_budget
            .insert("cpu_seconds".into(), 101.0);
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
