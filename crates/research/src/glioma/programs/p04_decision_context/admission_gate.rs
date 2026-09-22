//! Evidence- and authority-gated admission of generated glioma research actions.
//!
//! Context compilation produces useful action candidates, but autonomous execution needs a
//! second decision: is each action sufficiently supported, fresh, covered, reproducible, and
//! authorized under the current effect boundary? This feature performs that admission decision
//! deterministically. It never upgrades an uncertain claim, grants authority from autonomy tier,
//! or performs the admitted action.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, GliomaStageKind};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionAdmissionGate1@1";
pub const MAX_ACTIONS: usize = 512;
pub const MAX_REASON_CODES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAdmissionAction {
    pub action_id: String,
    pub stage_kind: GliomaStageKind,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub depends_on: Vec<String>,
    pub cost_units: u32,
    pub autonomy_tier: AutonomyTier,
    pub effects: BTreeSet<Effect>,
    pub evidence_milli: u16,
    pub freshness_milli: u16,
    pub coverage_milli: u16,
    pub contradiction_milli: u16,
    pub reproducibility_milli: u16,
    pub approval_granted: bool,
    pub signed_preflight: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAdmissionRequest {
    pub objective: String,
    pub actions: Vec<DecisionAdmissionAction>,
    pub budget_units: u32,
    pub max_admitted_actions: u16,
    pub require_dependency_closure: bool,
    pub min_evidence_milli: u16,
    pub min_freshness_milli: u16,
    pub min_coverage_milli: u16,
    pub max_contradiction_milli: u16,
    pub min_reproducibility_milli: u16,
    pub allow_instrument_execution: bool,
    pub allow_federation_export: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAdmissionDisposition {
    Admitted,
    ApprovalRequired,
    Blocked,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAdmissionRecord {
    pub action_id: String,
    pub disposition: DecisionAdmissionDisposition,
    pub reason_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub cumulative_cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAdmissionCampaignDisposition {
    Ready,
    Partial,
    Blocked,
    Denied,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAdmissionResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub action_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub approval_required_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub records: Vec<DecisionAdmissionRecord>,
    pub total_admitted_cost_units: u32,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionAdmissionCampaignDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionAdmissionError {
    #[error("decision admission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision admission action is invalid: {0}")]
    InvalidAction(String),
    #[error("decision admission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision admission digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(result: &DecisionAdmissionResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "action_order": result.action_order,
        "admitted_order": result.admitted_order,
        "approval_required_order": result.approval_required_order,
        "blocked_order": result.blocked_order,
        "denied_order": result.denied_order,
        "records": result.records,
        "total_admitted_cost_units": result.total_admitted_cost_units,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(request: &DecisionAdmissionRequest) -> Result<(), DecisionAdmissionError> {
    if request.objective.trim().is_empty()
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.budget_units == 0
        || request.max_admitted_actions == 0
        || usize::from(request.max_admitted_actions) > MAX_ACTIONS
        || request.min_evidence_milli > 1_000
        || request.min_freshness_milli > 1_000
        || request.min_coverage_milli > 1_000
        || request.max_contradiction_milli > 1_000
        || request.min_reproducibility_milli > 1_000
    {
        return Err(DecisionAdmissionError::InvalidRequest(
            "objective, bounded action set, positive budget, and bounded admission gates are required".into(),
        ));
    }
    let known = request
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>();
    if known.len() != request.actions.len() {
        return Err(DecisionAdmissionError::InvalidAction(
            "action IDs must be unique".into(),
        ));
    }
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || action.cost_units == 0
            || action.effects.is_empty()
            || !canonical(&action.depends_on)
            || action.depends_on.iter().any(|dependency| {
                dependency.trim().is_empty()
                    || dependency == &action.action_id
                    || !known.contains(dependency)
                    || dependency >= &action.action_id
            })
            || action.evidence_milli > 1_000
            || action.freshness_milli > 1_000
            || action.coverage_milli > 1_000
            || action.contradiction_milli > 1_000
            || action.reproducibility_milli > 1_000
            || (action.effects.contains(&Effect::FederationExport) && action.local_only)
            || (action.effects.contains(&Effect::InstrumentExecution) && action.local_only)
        {
            return Err(DecisionAdmissionError::InvalidAction(
                "actions require unique ordered dependencies, positive cost, bounded evidence metrics, and coherent effect/locality declarations".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(result: &DecisionAdmissionResult) -> Result<(), DecisionAdmissionError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || !canonical(&result.action_order)
        || !canonical(&result.admitted_order)
        || !canonical(&result.approval_required_order)
        || !canonical(&result.blocked_order)
        || !canonical(&result.denied_order)
        || !canonical(&result.negative_evidence_order)
        || !canonical(&result.uncertainty_order)
        || result.records.len() != result.action_order.len()
        || result
            .records
            .windows(2)
            .any(|pair| pair[0].action_id >= pair[1].action_id)
        || result.records.iter().any(|record| {
            record.action_id.trim().is_empty()
                || record.reason_order.is_empty()
                || record.reason_order.len() > MAX_REASON_CODES
                || !canonical(&record.reason_order)
                || !canonical(&record.dependency_order)
        })
    {
        return Err(DecisionAdmissionError::InvalidOutput(
            "identity, partition ordering, record coverage, reason, or dependency invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| DecisionAdmissionError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(DecisionAdmissionError::InvalidOutput(
            "decision admission digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl DecisionAdmissionResult {
    pub fn validate(&self) -> Result<(), DecisionAdmissionError> {
        validate_output(self)
    }
}

fn intrinsic_decision(
    action: &DecisionAdmissionAction,
    request: &DecisionAdmissionRequest,
) -> (DecisionAdmissionDisposition, BTreeSet<String>) {
    let mut reasons = BTreeSet::new();
    let mut disposition = DecisionAdmissionDisposition::Admitted;
    if action.contradiction_milli > request.max_contradiction_milli {
        reasons.insert("contradiction-above-ceiling".into());
        disposition = DecisionAdmissionDisposition::Denied;
    }
    if action.evidence_milli < request.min_evidence_milli {
        reasons.insert("evidence-below-floor".into());
        disposition = DecisionAdmissionDisposition::Blocked;
    }
    if action.freshness_milli < request.min_freshness_milli {
        reasons.insert("evidence-stale".into());
        disposition = DecisionAdmissionDisposition::Blocked;
    }
    if action.coverage_milli < request.min_coverage_milli {
        reasons.insert("coverage-below-floor".into());
        disposition = DecisionAdmissionDisposition::Blocked;
    }
    if action.reproducibility_milli < request.min_reproducibility_milli {
        reasons.insert("reproducibility-below-floor".into());
        disposition = DecisionAdmissionDisposition::Blocked;
    }
    if action.effects.contains(&Effect::InstrumentExecution) && !request.allow_instrument_execution
    {
        reasons.insert("instrument-effect-not-permitted".into());
        disposition = DecisionAdmissionDisposition::Denied;
    }
    if action.effects.contains(&Effect::FederationExport) && !request.allow_federation_export {
        reasons.insert("federation-export-not-permitted".into());
        disposition = DecisionAdmissionDisposition::Denied;
    }
    if action.autonomy_tier.requires_signed_preflight() && !action.signed_preflight {
        reasons.insert("signed-preflight-required".into());
        disposition = DecisionAdmissionDisposition::Blocked;
    }
    if action.autonomy_tier.requires_approval() && !action.approval_granted {
        reasons.insert("approval-required".into());
        if disposition == DecisionAdmissionDisposition::Admitted {
            disposition = DecisionAdmissionDisposition::ApprovalRequired;
        }
    }
    if reasons.is_empty() {
        reasons.insert("all-declared-admission-gates-passed".into());
    }
    (disposition, reasons)
}

/// Admit generated research actions only when their evidence, authority, effects, dependencies,
/// and budget satisfy the current bounded policy.
pub fn admit_glioma_decision_actions(
    request: &DecisionAdmissionRequest,
) -> Result<DecisionAdmissionResult, DecisionAdmissionError> {
    validate_request(request)?;
    let mut actions = request.actions.clone();
    actions.sort_by_key(|action| action.action_id.clone());
    let mut statuses = BTreeMap::<String, DecisionAdmissionDisposition>::new();
    let mut records = Vec::new();
    let mut admitted = BTreeSet::new();
    let mut approval_required = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut total_cost = 0_u32;
    for action in actions {
        let (mut disposition, mut reasons) = intrinsic_decision(&action, request);
        let unresolved_dependencies = action
            .depends_on
            .iter()
            .filter(|dependency| {
                statuses
                    .get(*dependency)
                    .is_none_or(|status| *status != DecisionAdmissionDisposition::Admitted)
            })
            .cloned()
            .collect::<Vec<_>>();
        if request.require_dependency_closure && !unresolved_dependencies.is_empty() {
            reasons.insert("dependency-not-admitted".into());
            disposition = DecisionAdmissionDisposition::Blocked;
            uncertainty.extend(
                unresolved_dependencies
                    .iter()
                    .map(|dependency| format!("dependency-unresolved:{dependency}")),
            );
        }
        if disposition == DecisionAdmissionDisposition::Admitted
            && (admitted.len() >= usize::from(request.max_admitted_actions)
                || total_cost.saturating_add(action.cost_units) > request.budget_units)
        {
            reasons.insert("admission-budget-exhausted".into());
            disposition = DecisionAdmissionDisposition::Blocked;
            negative.insert(format!("budget-rejected:{}", action.action_id));
        }
        if disposition == DecisionAdmissionDisposition::Admitted {
            total_cost = total_cost.saturating_add(action.cost_units);
            admitted.insert(action.action_id.clone());
        } else if disposition == DecisionAdmissionDisposition::ApprovalRequired {
            approval_required.insert(action.action_id.clone());
        } else if disposition == DecisionAdmissionDisposition::Blocked {
            blocked.insert(action.action_id.clone());
        } else {
            denied.insert(action.action_id.clone());
            negative.insert(format!("denied:{}", action.action_id));
        }
        let cumulative_cost_units = if disposition == DecisionAdmissionDisposition::Admitted {
            total_cost
        } else {
            total_cost
        };
        statuses.insert(action.action_id.clone(), disposition);
        records.push(DecisionAdmissionRecord {
            action_id: action.action_id,
            disposition,
            reason_order: reasons.into_iter().collect(),
            dependency_order: action.depends_on,
            cumulative_cost_units,
        });
    }
    let action_order = records
        .iter()
        .map(|record| record.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if !denied.is_empty() {
        DecisionAdmissionCampaignDisposition::Denied
    } else if !blocked.is_empty() {
        if admitted.is_empty() {
            DecisionAdmissionCampaignDisposition::Blocked
        } else {
            DecisionAdmissionCampaignDisposition::Partial
        }
    } else if !approval_required.is_empty() {
        if admitted.is_empty() {
            DecisionAdmissionCampaignDisposition::Blocked
        } else {
            DecisionAdmissionCampaignDisposition::Partial
        }
    } else if !admitted.is_empty() {
        DecisionAdmissionCampaignDisposition::Ready
    } else {
        DecisionAdmissionCampaignDisposition::Unresolved
    };
    let next_action = match disposition {
        DecisionAdmissionCampaignDisposition::Ready => {
            "send admitted local research actions to the approval-bound execution planner"
        }
        DecisionAdmissionCampaignDisposition::Partial => {
            "execute only admitted actions and resolve blocked or approval-required actions before dependent work"
        }
        DecisionAdmissionCampaignDisposition::Blocked => {
            "resolve evidence, dependency, budget, preflight, or approval gates before execution"
        }
        DecisionAdmissionCampaignDisposition::Denied => {
            "hold denied actions and revise permissions or evidence without bypassing the gate"
        }
        DecisionAdmissionCampaignDisposition::Unresolved => {
            "collect typed evidence and action candidates before attempting admission"
        }
    }
    .into();
    let mut result = DecisionAdmissionResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        action_order,
        admitted_order: admitted.into_iter().collect(),
        approval_required_order: approval_required.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        records,
        total_admitted_cost_units: total_cost,
        negative_evidence_order: negative.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-admission"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| DecisionAdmissionError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, effects: BTreeSet<Effect>) -> DecisionAdmissionAction {
        DecisionAdmissionAction {
            action_id: id.into(),
            stage_kind: GliomaStageKind::ComputationalExecution,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 3,
            autonomy_tier: AutonomyTier::A1,
            effects,
            evidence_milli: 900,
            freshness_milli: 900,
            coverage_milli: 900,
            contradiction_milli: 50,
            reproducibility_milli: 900,
            approval_granted: false,
            signed_preflight: true,
            local_only: true,
        }
    }

    fn request(actions: Vec<DecisionAdmissionAction>) -> DecisionAdmissionRequest {
        DecisionAdmissionRequest {
            objective: "admit a bounded preclinical glioma research workflow".into(),
            actions,
            budget_units: 10,
            max_admitted_actions: 4,
            require_dependency_closure: true,
            min_evidence_milli: 700,
            min_freshness_milli: 700,
            min_coverage_milli: 700,
            max_contradiction_milli: 200,
            min_reproducibility_milli: 700,
            allow_instrument_execution: false,
            allow_federation_export: false,
        }
    }

    #[test]
    fn admits_local_evidence_qualified_action() {
        let result = admit_glioma_decision_actions(&request(vec![action(
            "compute-glioma-state",
            BTreeSet::from([Effect::ExecuteLocalComputation]),
        )]))
        .expect("admission");
        assert_eq!(
            result.disposition,
            DecisionAdmissionCampaignDisposition::Ready
        );
        assert_eq!(result.admitted_order, vec!["compute-glioma-state"]);
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn blocks_stale_or_undercovered_action() {
        let mut candidate = action(
            "stale-computation",
            BTreeSet::from([Effect::ExecuteLocalComputation]),
        );
        candidate.freshness_milli = 100;
        let result = admit_glioma_decision_actions(&request(vec![candidate])).expect("admission");
        assert_eq!(
            result.disposition,
            DecisionAdmissionCampaignDisposition::Blocked
        );
        assert!(result.blocked_order.contains(&"stale-computation".into()));
    }

    #[test]
    fn denies_unpermitted_federation_effect() {
        let mut candidate = action("export-summary", BTreeSet::from([Effect::FederationExport]));
        candidate.local_only = false;
        let result = admit_glioma_decision_actions(&request(vec![candidate])).expect("admission");
        assert_eq!(
            result.disposition,
            DecisionAdmissionCampaignDisposition::Denied
        );
        assert!(result.denied_order.contains(&"export-summary".into()));
    }
}
