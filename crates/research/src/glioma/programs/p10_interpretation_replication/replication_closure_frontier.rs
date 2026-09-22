//! Replication-closure frontier for autonomous preclinical glioma research.
//!
//! A replication result is not useful if the engine can only label it. This feature converts the
//! typed P10 validation/replication outcome into a ranked next-action frontier. It scores
//! independent-site extension, heterogeneity reconciliation, target-model acquisition,
//! influential-study stress tests, negative-result confirmation, and methods review under an
//! explicit cost/risk budget. The controller preserves qualified and negative holds instead of
//! treating either as permission to spend indefinitely or as a clinical conclusion.

use super::campaign::{GliomaReplicationCampaignDisposition, GliomaReplicationCampaignStopReason};
use super::validation_replication_campaign::{
    ValidationReplicationCampaignDisposition, ValidationReplicationCampaignRun,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationClosureFrontier1@1";
pub const MAX_CANDIDATES: usize = 64;
pub const MAX_SELECTED: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationClosureTarget {
    ExtendIndependentSites,
    ReconcileHeterogeneity,
    AcquireTargetModel,
    StressTestInfluentialStudy,
    ConfirmNegativeResult,
    MethodsReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureCandidate {
    pub action_id: String,
    pub target: ReplicationClosureTarget,
    pub route: String,
    pub model_system: GliomaModelSystem,
    pub rationale: String,
    pub expected_information_milli: u32,
    pub reproducibility_milli: u16,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub requires_independent_site: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureFrontierRequest {
    pub replication: ValidationReplicationCampaignRun,
    pub candidates: Vec<ReplicationClosureCandidate>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub max_risk_milli: u16,
    pub min_utility_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationClosureDisposition {
    Ready,
    QualifiedHold,
    NegativeHold,
    Partial,
    Blocked,
    NoRunnableActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureScore {
    pub action_id: String,
    pub route: String,
    pub model_system: GliomaModelSystem,
    pub target: ReplicationClosureTarget,
    pub utility_milli: u32,
    pub evidence_gap_milli: u32,
    pub risk_adjusted_information_milli: u32,
    pub cost_units: u32,
    pub decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureFrontier {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub source_replication_digest: ContentHash,
    pub source_disposition: ValidationReplicationCampaignDisposition,
    pub source_stop_reason: Option<GliomaReplicationCampaignStopReason>,
    pub candidate_order: Vec<String>,
    pub scores: Vec<ReplicationClosureScore>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ReplicationClosureDisposition,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationClosureFrontierError {
    #[error("replication closure request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication closure output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication closure digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn source_campaign(
    run: &ValidationReplicationCampaignRun,
) -> Option<&super::campaign::GliomaReplicationCampaign> {
    run.campaign.as_ref()
}

fn evidence_gap(run: &ValidationReplicationCampaignRun, target: ReplicationClosureTarget) -> u32 {
    let mut gap: u32 = match run.disposition {
        ValidationReplicationCampaignDisposition::Qualified => 180,
        ValidationReplicationCampaignDisposition::Executed => 650,
        ValidationReplicationCampaignDisposition::Negative => 780,
        ValidationReplicationCampaignDisposition::HoldIndependentSites => 920,
        ValidationReplicationCampaignDisposition::BlockedByValidation
        | ValidationReplicationCampaignDisposition::Blocked => 1_000,
    };
    if let Some(campaign) = source_campaign(run) {
        gap = gap.saturating_add(match campaign.disposition {
            GliomaReplicationCampaignDisposition::Qualified => 0,
            GliomaReplicationCampaignDisposition::Partial => 140,
            GliomaReplicationCampaignDisposition::Negative => 220,
            GliomaReplicationCampaignDisposition::Unresolved => 300,
            GliomaReplicationCampaignDisposition::Failed
            | GliomaReplicationCampaignDisposition::Blocked => 360,
        });
        gap = gap.saturating_add((campaign.uncertainty.len() as u32).saturating_mul(45));
        gap = gap.saturating_add((campaign.negative_evidence.len() as u32).saturating_mul(35));
        if campaign.final_transportability.is_none() {
            gap = gap.saturating_add(120);
        }
    }
    let target_bonus = match target {
        ReplicationClosureTarget::ExtendIndependentSites => 120,
        ReplicationClosureTarget::ReconcileHeterogeneity => 190,
        ReplicationClosureTarget::AcquireTargetModel => 160,
        ReplicationClosureTarget::StressTestInfluentialStudy => 130,
        ReplicationClosureTarget::ConfirmNegativeResult => {
            if matches!(
                run.disposition,
                ValidationReplicationCampaignDisposition::Negative
            ) {
                240
            } else {
                40
            }
        }
        ReplicationClosureTarget::MethodsReview => {
            if matches!(
                run.disposition,
                ValidationReplicationCampaignDisposition::Qualified
            ) {
                440
            } else {
                80
            }
        }
    };
    gap.saturating_add(target_bonus).min(1_000)
}

fn target_allowed(
    run: &ValidationReplicationCampaignRun,
    target: ReplicationClosureTarget,
) -> bool {
    match run.disposition {
        ValidationReplicationCampaignDisposition::BlockedByValidation
        | ValidationReplicationCampaignDisposition::HoldIndependentSites
        | ValidationReplicationCampaignDisposition::Blocked => false,
        ValidationReplicationCampaignDisposition::Qualified => matches!(
            target,
            ReplicationClosureTarget::MethodsReview
                | ReplicationClosureTarget::StressTestInfluentialStudy
                | ReplicationClosureTarget::AcquireTargetModel
        ),
        ValidationReplicationCampaignDisposition::Negative => matches!(
            target,
            ReplicationClosureTarget::ConfirmNegativeResult
                | ReplicationClosureTarget::StressTestInfluentialStudy
                | ReplicationClosureTarget::MethodsReview
        ),
        ValidationReplicationCampaignDisposition::Executed => true,
    }
}

fn utility(candidate: &ReplicationClosureCandidate, gap: u32) -> (u32, u32) {
    let reliability = u32::from(candidate.reproducibility_milli.min(1_000));
    let feasibility = u32::from(candidate.feasibility_milli.min(1_000));
    let risk_discount = 1_000_u32.saturating_sub(u32::from(candidate.risk_milli));
    let value = candidate
        .expected_information_milli
        .min(1_000_000)
        .saturating_mul(gap)
        .saturating_mul(reliability.saturating_add(feasibility) / 2)
        .saturating_mul(risk_discount)
        / 1_000_000_000;
    let cost_penalty = candidate.cost_units.saturating_mul(35);
    (value.saturating_sub(cost_penalty), value)
}

fn digest_input(frontier: &ReplicationClosureFrontier) -> serde_json::Value {
    serde_json::json!({
        "feature_id": frontier.feature_id,
        "output_schema": frontier.output_schema,
        "objective": frontier.objective,
        "model_system": frontier.model_system,
        "source_replication_digest": frontier.source_replication_digest,
        "source_disposition": frontier.source_disposition,
        "source_stop_reason": frontier.source_stop_reason,
        "candidate_order": frontier.candidate_order,
        "scores": frontier.scores,
        "selected_order": frontier.selected_order,
        "deferred_order": frontier.deferred_order,
        "blocked_order": frontier.blocked_order,
        "negative_evidence": frontier.negative_evidence,
        "uncertainty": frontier.uncertainty,
        "disposition": frontier.disposition,
        "next_operator_action": frontier.next_operator_action,
    })
}

fn validate_request(
    request: &ReplicationClosureFrontierRequest,
) -> Result<(), ReplicationClosureFrontierError> {
    request
        .replication
        .validate()
        .map_err(|error| ReplicationClosureFrontierError::InvalidRequest(error.to_string()))?;
    if request.budget_units == 0
        || request.max_actions == 0
        || usize::from(request.max_actions) > MAX_SELECTED
        || request.max_risk_milli > 1_000
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
    {
        return Err(ReplicationClosureFrontierError::InvalidRequest(
            "positive budget/action bounds and a bounded candidate frontier are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || !ids.insert(candidate.action_id.clone())
            || candidate.route.trim().is_empty()
            || candidate.rationale.trim().is_empty()
            || candidate.expected_information_milli == 0
            || candidate.reproducibility_milli > 1_000
            || candidate.feasibility_milli > 1_000
            || candidate.risk_milli > 1_000
            || candidate.cost_units == 0
        {
            return Err(ReplicationClosureFrontierError::InvalidRequest(
                "candidate identity, route, rationale, information, cost, feasibility, risk, and reproducibility bounds are invalid".into(),
            ));
        }
    }
    Ok(())
}

impl ReplicationClosureFrontier {
    pub fn validate(&self) -> Result<(), ReplicationClosureFrontierError> {
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&score_ids)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self
                .scores
                .iter()
                .any(|score| score.route.trim().is_empty())
            || self
                .selected_order
                .iter()
                .any(|id| self.blocked_order.binary_search(id).is_ok())
        {
            return Err(ReplicationClosureFrontierError::InvalidOutput(
                "identity, ordering, action partitions, evidence, or operator action is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReplicationClosureFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReplicationClosureFrontierError::InvalidOutput(
                "replication closure frontier digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Rank the next bounded scientific actions after an independent-site replication result.
pub fn plan_glioma_replication_closure_frontier(
    request: &ReplicationClosureFrontierRequest,
) -> Result<ReplicationClosureFrontier, ReplicationClosureFrontierError> {
    validate_request(request)?;
    let mut scores = request
        .candidates
        .iter()
        .map(|candidate| {
            let gap = evidence_gap(&request.replication, candidate.target);
            let (utility, risk_adjusted_information) = utility(candidate, gap);
            (candidate, gap, utility, risk_adjusted_information)
        })
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.0.action_id.cmp(&right.0.action_id))
    });

    let mut selected = Vec::new();
    let mut deferred = Vec::new();
    let mut blocked = Vec::new();
    let mut spent = 0_u32;
    let mut risk = 0_u32;
    let mut score_output = Vec::new();
    for (candidate, gap, utility, risk_adjusted_information) in scores {
        let allowed = target_allowed(&request.replication, candidate.target);
        let exceeds_risk = risk.saturating_add(u32::from(candidate.risk_milli))
            > u32::from(request.max_risk_milli);
        let exceeds_budget = spent.saturating_add(candidate.cost_units) > request.budget_units;
        let score_decision = if !allowed {
            blocked.push(candidate.action_id.clone());
            "blocked_by_replication_state"
        } else if candidate.feasibility_milli == 0 {
            blocked.push(candidate.action_id.clone());
            "blocked_by_feasibility"
        } else if utility < request.min_utility_milli || exceeds_risk || exceeds_budget {
            deferred.push(candidate.action_id.clone());
            if utility < request.min_utility_milli {
                "deferred_below_utility_gate"
            } else if exceeds_risk {
                "deferred_risk_budget"
            } else {
                "deferred_cost_budget"
            }
        } else if selected.len() >= usize::from(request.max_actions) {
            deferred.push(candidate.action_id.clone());
            "deferred_action_capacity"
        } else {
            spent = spent.saturating_add(candidate.cost_units);
            risk = risk.saturating_add(u32::from(candidate.risk_milli));
            selected.push(candidate.action_id.clone());
            "selected"
        };
        score_output.push(ReplicationClosureScore {
            action_id: candidate.action_id.clone(),
            route: candidate.route.clone(),
            model_system: candidate.model_system,
            target: candidate.target,
            utility_milli: utility,
            evidence_gap_milli: gap,
            risk_adjusted_information_milli: risk_adjusted_information,
            cost_units: candidate.cost_units,
            decision: score_decision.into(),
        });
    }
    selected.sort();
    deferred.sort();
    blocked.sort();
    score_output.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let source_disposition = request.replication.disposition;
    let disposition = if matches!(
        source_disposition,
        ValidationReplicationCampaignDisposition::BlockedByValidation
            | ValidationReplicationCampaignDisposition::HoldIndependentSites
            | ValidationReplicationCampaignDisposition::Blocked
    ) {
        ReplicationClosureDisposition::Blocked
    } else if matches!(
        source_disposition,
        ValidationReplicationCampaignDisposition::Qualified
    ) && selected.is_empty()
    {
        ReplicationClosureDisposition::QualifiedHold
    } else if matches!(
        source_disposition,
        ValidationReplicationCampaignDisposition::Negative
    ) && selected.is_empty()
    {
        ReplicationClosureDisposition::NegativeHold
    } else if selected.is_empty() {
        ReplicationClosureDisposition::NoRunnableActions
    } else if matches!(
        source_disposition,
        ValidationReplicationCampaignDisposition::Qualified
    ) {
        ReplicationClosureDisposition::QualifiedHold
    } else if matches!(
        source_disposition,
        ValidationReplicationCampaignDisposition::Negative
    ) {
        ReplicationClosureDisposition::NegativeHold
    } else if deferred.is_empty() && blocked.is_empty() {
        ReplicationClosureDisposition::Ready
    } else {
        ReplicationClosureDisposition::Partial
    };
    let mut negative_evidence = request.replication.negative_evidence.clone();
    let mut uncertainty = request.replication.uncertainty.clone();
    if matches!(disposition, ReplicationClosureDisposition::Blocked) {
        negative_evidence.push("replication-closure-blocked-by-upstream-state".into());
    }
    if selected.is_empty() {
        uncertainty
            .push("no closure action passed the declared utility, risk, and budget gates".into());
    }
    let next_operator_action = match disposition {
        ReplicationClosureDisposition::Ready => {
            "execute the selected closure actions through their typed local workflow routes"
        }
        ReplicationClosureDisposition::QualifiedHold => {
            "hold the qualified replication result for methods review and signed release evidence"
        }
        ReplicationClosureDisposition::NegativeHold => {
            "preserve the negative result and only authorize an independent confirmation or falsification action"
        }
        ReplicationClosureDisposition::Partial => {
            "execute the selected actions and retain deferred candidates for a later bounded wave"
        }
        ReplicationClosureDisposition::Blocked => {
            "resolve the upstream validation or independent-site gate before selecting closure work"
        }
        ReplicationClosureDisposition::NoRunnableActions => {
            "supply a new typed closure candidate or revise the declared resource bounds"
        }
    };
    let mut output = ReplicationClosureFrontier {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.replication.objective.clone(),
        model_system: request.replication.model_system,
        source_replication_digest: request.replication.digest.clone(),
        source_disposition,
        source_stop_reason: source_campaign(&request.replication)
            .map(|campaign| campaign.stop_reason),
        candidate_order: request
            .candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        scores: score_output,
        selected_order: selected,
        deferred_order: deferred,
        blocked_order: blocked,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        next_operator_action: next_operator_action.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-closure-frontier"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReplicationClosureFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::PRECLINICAL_BOUNDARY;

    fn blocked_run() -> ValidationReplicationCampaignRun {
        let mut run = ValidationReplicationCampaignRun {
            feature_id: "GAF-GLIOMA-P10-F23".into(),
            output_schema: "GliomaValidationReplicationCampaign1@1".into(),
            objective: "replicate an organoid invasion mechanism".into(),
            model_system: GliomaModelSystem::Organoid,
            validation_campaign_digest: ContentHash::of_bytes(b"validation"),
            origin_site_id: "origin".into(),
            independent_site_order: Vec::new(),
            gate_disposition: super::super::validation_replication_gate::ValidationReplicationGateDisposition::HoldValidation,
            campaign: None,
            disposition: ValidationReplicationCampaignDisposition::BlockedByValidation,
            stop_reason: None,
            negative_evidence: vec!["validation-not-qualified".into()],
            uncertainty: vec!["awaiting independent-site admission".into()],
            next_action: "complete validation".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let input = serde_json::json!({
            "feature_id": run.feature_id,
            "output_schema": run.output_schema,
            "objective": run.objective,
            "model_system": run.model_system,
            "validation_campaign_digest": run.validation_campaign_digest,
            "origin_site_id": run.origin_site_id,
            "independent_site_order": run.independent_site_order,
            "gate_disposition": run.gate_disposition,
            "campaign": run.campaign,
            "disposition": run.disposition,
            "stop_reason": run.stop_reason,
            "negative_evidence": run.negative_evidence,
            "uncertainty": run.uncertainty,
            "next_action": run.next_action,
            "boundary": run.boundary,
        });
        run.digest = ContentHash::of_value(&input).unwrap();
        run
    }

    #[test]
    fn blocked_replication_cannot_emit_closure_actions() {
        let request = ReplicationClosureFrontierRequest {
            replication: blocked_run(),
            candidates: vec![ReplicationClosureCandidate {
                action_id: "confirm".into(),
                target: ReplicationClosureTarget::ConfirmNegativeResult,
                route: "glioma_validation_replication_campaign_execute".into(),
                model_system: GliomaModelSystem::Organoid,
                rationale: "confirm only after admission".into(),
                expected_information_milli: 800,
                reproducibility_milli: 900,
                feasibility_milli: 900,
                risk_milli: 100,
                cost_units: 1,
                requires_independent_site: true,
            }],
            budget_units: 2,
            max_actions: 1,
            max_risk_milli: 500,
            min_utility_milli: 0,
        };
        let frontier = plan_glioma_replication_closure_frontier(&request).unwrap();
        assert_eq!(frontier.disposition, ReplicationClosureDisposition::Blocked);
        assert!(frontier.selected_order.is_empty());
        assert_eq!(frontier.blocked_order, vec!["confirm"]);
        frontier.validate().unwrap();
    }
}
