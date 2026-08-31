//! Deterministic preflight for governed preclinical instrument actions.
//!
//! Atlas feature: `AFA-lab-P11-F01`.
//!
//! This module is an approval and interlock product, not a hardware driver. It validates a typed
//! action plan, policy receipt, evidence requirements, resource budgets, and emergency-stop state,
//! then emits a content-addressed receipt. It performs no instrument, network, filesystem, or
//! material effect.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, PolicyDecision,
    PolicyReceipt, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P11-F01";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentAction {
    pub action_id: String,
    pub instrument_id: String,
    pub operation: String,
    pub resource: String,
    pub cost: f64,
    pub reversible: bool,
    pub evidence_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentPreflightRequest {
    pub run_id: String,
    pub study_id: String,
    pub actions: Vec<InstrumentAction>,
    pub policy: PolicyReceipt,
    pub approval_reference: Option<String>,
    pub emergency_stop_asserted: bool,
    pub declared_interlocks: BTreeSet<String>,
    pub required_interlocks: BTreeSet<String>,
    pub resource_budget: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightDecision {
    Ready,
    Blocked,
    RequiresApproval,
    EmergencyStop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentPreflightReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub run_id: String,
    pub study_id: String,
    pub decision: PreflightDecision,
    pub ordered_actions: Vec<String>,
    pub action_digests: BTreeMap<String, ContentHash>,
    pub remaining_budget: BTreeMap<String, f64>,
    pub omissions: Vec<String>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl InstrumentPreflightReceipt {
    pub fn validate(&self) -> Result<(), InstrumentPreflightError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(InstrumentPreflightError::Contract(
                "research contract schema mismatch".into(),
            ));
        }
        if self.feature_id != FEATURE_ID
            || self.run_id.trim().is_empty()
            || self.study_id.trim().is_empty()
        {
            return Err(InstrumentPreflightError::InvalidRequest(
                "feature, run, and study identity are required".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(InstrumentPreflightError::Contract(
                "instrument preflight crossed the preclinical boundary".into(),
            ));
        }
        if self.ordered_actions.is_empty()
            || self.action_digests.is_empty()
            || self.reasons.is_empty()
        {
            return Err(InstrumentPreflightError::InvalidRequest(
                "preflight actions, digests, and reasons are required".into(),
            ));
        }
        if self
            .ordered_actions
            .iter()
            .any(|action| !self.action_digests.contains_key(action))
        {
            return Err(InstrumentPreflightError::InvalidRequest(
                "every ordered action needs a digest".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InstrumentPreflightError::Contract(error.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum InstrumentPreflightError {
    #[error("invalid instrument preflight request: {0}")]
    InvalidRequest(String),
    #[error("instrument preflight contract rejected: {0}")]
    Contract(String),
    #[error("instrument policy is not allow: {0:?}")]
    PolicyBlocked(PolicyDecision),
    #[error("missing required instrument interlock {0}")]
    MissingInterlock(String),
    #[error("instrument resource budget exceeded for {0}")]
    BudgetExceeded(String),
    #[error("duplicate instrument action {0}")]
    DuplicateAction(String),
    #[error("non-reversible action {0} has no evidence digest")]
    MissingEvidence(String),
}

pub fn instrument_preflight_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_CONTRACT_VERSION.into(),
        owner_crate: "lab".into(),
        consumers: ["bioinformatician".into(), "instrument operator".into()].into(),
        behavior: "validates a typed instrument action plan, policy, evidence, interlocks, and budgets without reaching hardware".into(),
        value: "prevents unauthorized or unsafe preclinical instrument effects while preserving a deterministic approval receipt".into(),
        inputs: vec![TypedPort {
            name: "instrument_preflight_request".into(),
            schema: "InstrumentPreflightRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "instrument_preflight_receipt".into(),
            schema: "InstrumentPreflightReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::InstrumentExecution]
            .into(),
        permissions: [
            "read:local-instrument-plan".into(),
            "approve:preclinical-instrument-preflight".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: Vec::new(),
        authority_requirements: vec![AuthorityRequirement {
            role: "authorized instrument operator".into(),
            reason: "physical execution requires a human-approved signed preflight".into(),
        }],
        autonomy_tier: AutonomyTier::A3,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn instrument_preflight(
    request: &InstrumentPreflightRequest,
) -> Result<InstrumentPreflightReceipt, InstrumentPreflightError> {
    validate_request(request)?;
    let mut actions = request.actions.clone();
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut action_digests = BTreeMap::new();
    let mut remaining_budget = request.resource_budget.clone();
    for action in &actions {
        let value = serde_json::to_value(action)
            .map_err(|error| InstrumentPreflightError::Contract(error.to_string()))?;
        let digest = ContentHash::of_value(&value)
            .map_err(|error| InstrumentPreflightError::Contract(error.to_string()))?;
        action_digests.insert(action.action_id.clone(), digest);
        let balance = remaining_budget
            .get_mut(&action.resource)
            .ok_or_else(|| InstrumentPreflightError::BudgetExceeded(action.resource.clone()))?;
        *balance -= action.cost;
    }
    let omissions = Vec::new();
    let decision = if request.emergency_stop_asserted {
        PreflightDecision::EmergencyStop
    } else if request.policy.decision != PolicyDecision::Allow {
        PreflightDecision::Blocked
    } else if request.approval_reference.is_none() {
        PreflightDecision::RequiresApproval
    } else {
        PreflightDecision::Ready
    };
    let reasons = match decision {
        PreflightDecision::Ready => vec![
            "policy allow, approval reference, interlocks, evidence, and resource budgets validated; no hardware effect performed".into(),
        ],
        PreflightDecision::Blocked => vec![format!(
            "instrument policy decision {:?} blocks physical execution",
            request.policy.decision
        )],
        PreflightDecision::RequiresApproval => vec![
            "all deterministic checks passed but an authorized operator approval reference is required".into(),
        ],
        PreflightDecision::EmergencyStop => vec![
            "emergency stop is asserted; every physical action is refused".into(),
        ],
    };
    let ordered_actions = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "run_id": request.run_id,
        "study_id": request.study_id,
        "decision": decision,
        "ordered_actions": ordered_actions,
        "action_digests": action_digests,
        "remaining_budget": remaining_budget,
        "omissions": omissions,
        "reasons": reasons,
        "hardware_effect_performed": false,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("instrument-preflight:{}", request.run_id),
        "application/vnd.aurora.instrument-preflight+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| InstrumentPreflightError::Contract(error.to_string()))?;
    let receipt = InstrumentPreflightReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        run_id: request.run_id.clone(),
        study_id: request.study_id.clone(),
        decision,
        ordered_actions: actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect(),
        action_digests,
        remaining_budget,
        omissions,
        reasons,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &InstrumentPreflightRequest) -> Result<(), InstrumentPreflightError> {
    if request.run_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.actions.is_empty()
    {
        return Err(InstrumentPreflightError::InvalidRequest(
            "run, study, and at least one action are required".into(),
        ));
    }
    request
        .policy
        .validate()
        .map_err(|error| InstrumentPreflightError::Contract(error.to_string()))?;
    if request
        .required_interlocks
        .iter()
        .any(|interlock| !request.declared_interlocks.contains(interlock))
    {
        let Some(missing) = request
            .required_interlocks
            .iter()
            .find(|interlock| !request.declared_interlocks.contains(*interlock))
        else {
            return Err(InstrumentPreflightError::InvalidRequest(
                "interlock validation state changed while identifying the missing interlock".into(),
            ));
        };
        return Err(InstrumentPreflightError::MissingInterlock(missing.clone()));
    }
    if request
        .resource_budget
        .values()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(InstrumentPreflightError::InvalidRequest(
            "resource budgets must be finite and non-negative".into(),
        ));
    }
    let mut action_ids = BTreeSet::new();
    let mut totals = BTreeMap::<String, f64>::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || action.instrument_id.trim().is_empty()
            || action.operation.trim().is_empty()
            || action.resource.trim().is_empty()
        {
            return Err(InstrumentPreflightError::InvalidRequest(
                "instrument action identity and operation are required".into(),
            ));
        }
        if !action_ids.insert(action.action_id.clone()) {
            return Err(InstrumentPreflightError::DuplicateAction(
                action.action_id.clone(),
            ));
        }
        if !action.cost.is_finite() || action.cost < 0.0 {
            return Err(InstrumentPreflightError::InvalidRequest(format!(
                "invalid cost for action {}",
                action.action_id
            )));
        }
        if !action.reversible && action.evidence_digest.is_none() {
            return Err(InstrumentPreflightError::MissingEvidence(
                action.action_id.clone(),
            ));
        }
        *totals.entry(action.resource.clone()).or_default() += action.cost;
    }
    for (resource, total) in totals {
        let Some(limit) = request.resource_budget.get(&resource) else {
            return Err(InstrumentPreflightError::BudgetExceeded(resource));
        };
        if total > *limit {
            return Err(InstrumentPreflightError::BudgetExceeded(resource));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::PRECLINICAL_BOUNDARY;

    fn request() -> InstrumentPreflightRequest {
        InstrumentPreflightRequest {
            run_id: "run:instrument-1".into(),
            study_id: "study:organoid-1".into(),
            actions: vec![
                InstrumentAction {
                    action_id: "action-2".into(),
                    instrument_id: "microscope-1".into(),
                    operation: "capture".into(),
                    resource: "minutes".into(),
                    cost: 3.0,
                    reversible: true,
                    evidence_digest: None,
                },
                InstrumentAction {
                    action_id: "action-1".into(),
                    instrument_id: "robot-1".into(),
                    operation: "dispense".into(),
                    resource: "ul".into(),
                    cost: 2.0,
                    reversible: false,
                    evidence_digest: Some(ContentHash::of_bytes(b"protocol")),
                },
            ],
            policy: PolicyReceipt {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                receipt_id: "policy:instrument-1".into(),
                decision: PolicyDecision::Allow,
                reasons: vec!["approved preclinical protocol".into()],
                evaluated_artifacts: vec![],
                authority_reference: Some("approval:operator-1".into()),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            approval_reference: Some("approval:operator-1".into()),
            emergency_stop_asserted: false,
            declared_interlocks: ["door-closed".into(), "temperature-safe".into()].into(),
            required_interlocks: ["door-closed".into(), "temperature-safe".into()].into(),
            resource_budget: [("minutes".into(), 5.0), ("ul".into(), 4.0)].into(),
        }
    }

    #[test]
    fn preflight_is_deterministic_and_never_executes_hardware() {
        let receipt = instrument_preflight(&request()).unwrap();
        assert_eq!(receipt.decision, PreflightDecision::Ready);
        assert_eq!(receipt.ordered_actions, vec!["action-1", "action-2"]);
        assert_eq!(receipt.remaining_budget["minutes"], 2.0);
        assert_eq!(receipt.remaining_budget["ul"], 2.0);
    }

    #[test]
    fn missing_interlock_blocks_before_budget_consumption() {
        let mut request = request();
        request.required_interlocks.insert("shield-closed".into());
        assert!(matches!(
            instrument_preflight(&request).unwrap_err(),
            InstrumentPreflightError::MissingInterlock(_)
        ));
    }

    #[test]
    fn emergency_stop_is_explicit_and_policy_cannot_override_it() {
        let mut request = request();
        request.emergency_stop_asserted = true;
        let receipt = instrument_preflight(&request).unwrap();
        assert_eq!(receipt.decision, PreflightDecision::EmergencyStop);
        assert!(receipt
            .reasons
            .iter()
            .any(|reason| reason.contains("emergency stop")));
    }

    #[test]
    fn non_reversible_action_requires_evidence() {
        let mut request = request();
        request.actions[1].evidence_digest = None;
        assert!(matches!(
            instrument_preflight(&request).unwrap_err(),
            InstrumentPreflightError::MissingEvidence(_)
        ));
    }
}
