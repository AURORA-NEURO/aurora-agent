//! Information-aware instrument campaign selection for preclinical glioma research.
//!
//! A fleet scheduler answers *where* a run can execute and the campaign controller answers
//! *whether* a queue may continue. This feature answers the scientific question in between:
//! given several already-preflighted assay plans, which dependency-closed subset buys the most
//! reproducible information per unit of instrument time and risk? The selector is deterministic,
//! accounts for endpoint diversity, and then hands only the selected plans to the existing guarded
//! campaign executor. It never turns an instrument completion into biological evidence.

use super::campaign::{
    execute_glioma_instrument_campaign, InstrumentCampaign, InstrumentCampaignDisposition,
    InstrumentCampaignError, InstrumentCampaignRequest, InstrumentCampaignRunRequest,
};
use super::execution::{DryRunInstrumentExecutor, InstrumentExecutionRequest, InstrumentExecutor};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveInstrumentCampaign1@1";
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_SELECTED: usize = 256;
pub const MAX_ENDPOINTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInstrumentCandidate {
    pub candidate_id: String,
    pub run_id: String,
    pub endpoint: String,
    pub execution: InstrumentExecutionRequest,
    pub expected_information_milli: u64,
    pub frontier_novelty_milli: u64,
    pub reproducibility_milli: u64,
    pub estimated_cost_ticks: u64,
    pub risk_milli: u64,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInstrumentCampaignRequest {
    pub objective: String,
    pub candidates: Vec<AdaptiveInstrumentCandidate>,
    pub cost_budget_ticks: u64,
    pub risk_budget_milli: u64,
    pub minimum_information_milli: u64,
    pub minimum_endpoint_count: usize,
    pub max_selected: usize,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInstrumentDecision {
    pub candidate_id: String,
    pub selected: bool,
    pub score_milli: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveInstrumentCampaignDisposition {
    Executed,
    Negative,
    Partial,
    Failed,
    Blocked,
    Unresolved,
    NoFeasiblePlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInstrumentCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub endpoint_order: Vec<String>,
    pub candidates: Vec<AdaptiveInstrumentCandidate>,
    pub decisions: Vec<AdaptiveInstrumentDecision>,
    pub planned_information_milli: u64,
    pub planned_cost_ticks: u64,
    pub planned_risk_milli: u64,
    pub minimum_information_milli: u64,
    pub minimum_endpoint_count: usize,
    pub campaign: Option<InstrumentCampaign>,
    pub simulation_only: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AdaptiveInstrumentCampaignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveInstrumentCampaignError {
    #[error("adaptive instrument campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive instrument campaign dependency graph is invalid: {0}")]
    Dependency(String),
    #[error("adaptive instrument campaign execution failed: {0}")]
    Campaign(#[from] InstrumentCampaignError),
    #[error("adaptive instrument campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive instrument campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn utility(candidate: &AdaptiveInstrumentCandidate) -> u128 {
    (candidate.expected_information_milli as u128)
        .saturating_mul(600)
        .saturating_add((candidate.frontier_novelty_milli as u128).saturating_mul(250))
        .saturating_add((candidate.reproducibility_milli as u128).saturating_mul(150))
}

fn score(candidate: &AdaptiveInstrumentCandidate, endpoint_is_new: bool) -> u64 {
    let risk_factor = 1_000_000_u128.saturating_add(candidate.risk_milli as u128);
    let denominator = (candidate.estimated_cost_ticks as u128)
        .saturating_mul(risk_factor)
        .max(1);
    let diversity_factor = if endpoint_is_new { 2_u128 } else { 1_u128 };
    utility(candidate)
        .saturating_mul(1_000_000)
        .saturating_mul(diversity_factor)
        .checked_div(denominator)
        .unwrap_or(0)
        .min(u64::MAX as u128) as u64
}

fn digest_input(output: &AdaptiveInstrumentCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "endpoint_order": output.endpoint_order,
        "candidates": output.candidates,
        "decisions": output.decisions,
        "planned_information_milli": output.planned_information_milli,
        "planned_cost_ticks": output.planned_cost_ticks,
        "planned_risk_milli": output.planned_risk_milli,
        "minimum_information_milli": output.minimum_information_milli,
        "minimum_endpoint_count": output.minimum_endpoint_count,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &AdaptiveInstrumentCampaignRequest,
) -> Result<BTreeMap<String, AdaptiveInstrumentCandidate>, AdaptiveInstrumentCampaignError> {
    if request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.cost_budget_ticks == 0
        || request.risk_budget_milli == 0
        || request.minimum_information_milli == 0
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
        || request.max_selected > request.candidates.len()
        || request.minimum_endpoint_count > MAX_ENDPOINTS
    {
        return Err(AdaptiveInstrumentCampaignError::InvalidRequest(
            "objective, bounded candidates, positive budgets, and a valid selection bound are required"
                .into(),
        ));
    }
    let mut candidates = BTreeMap::new();
    let mut run_ids = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.run_id.trim().is_empty()
            || candidate.endpoint.trim().is_empty()
            || candidates.contains_key(&candidate.candidate_id)
            || !run_ids.insert(candidate.run_id.clone())
            || candidate.expected_information_milli == 0
            || candidate.estimated_cost_ticks == 0
            || candidate.risk_milli > 1_000_000
            || candidate.frontier_novelty_milli > 1_000_000
            || candidate.reproducibility_milli > 1_000_000
            || !canonical(&candidate.depends_on)
            || candidate.depends_on.iter().any(|dependency| {
                dependency == &candidate.candidate_id || dependency.trim().is_empty()
            })
        {
            return Err(AdaptiveInstrumentCampaignError::InvalidRequest(
                "candidate identity, endpoint, score inputs, run identity, and canonical dependencies are required"
                    .into(),
            ));
        }
        candidates.insert(candidate.candidate_id.clone(), candidate.clone());
        endpoints.insert(candidate.endpoint.clone());
    }
    if request.minimum_endpoint_count > endpoints.len() {
        return Err(AdaptiveInstrumentCampaignError::InvalidRequest(
            "minimum endpoint coverage exceeds the candidate endpoint set".into(),
        ));
    }
    for candidate in candidates.values() {
        if candidate
            .depends_on
            .iter()
            .any(|dependency| !candidates.contains_key(dependency))
        {
            return Err(AdaptiveInstrumentCampaignError::Dependency(format!(
                "candidate {} references an unknown dependency",
                candidate.candidate_id
            )));
        }
    }
    Ok(candidates)
}

fn dependency_closure(
    candidate_id: &str,
    candidates: &BTreeMap<String, AdaptiveInstrumentCandidate>,
    selected: &BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
    output: &mut Vec<String>,
) -> Result<(), AdaptiveInstrumentCampaignError> {
    if selected.contains(candidate_id) || output.iter().any(|id| id == candidate_id) {
        return Ok(());
    }
    if !visiting.insert(candidate_id.to_string()) {
        return Err(AdaptiveInstrumentCampaignError::Dependency(format!(
            "dependency cycle reaches {candidate_id}"
        )));
    }
    let candidate = candidates.get(candidate_id).ok_or_else(|| {
        AdaptiveInstrumentCampaignError::Dependency(format!("unknown candidate {candidate_id}"))
    })?;
    for dependency in &candidate.depends_on {
        dependency_closure(dependency, candidates, selected, visiting, output)?;
    }
    visiting.remove(candidate_id);
    output.push(candidate_id.to_string());
    Ok(())
}

fn campaign_disposition(
    disposition: InstrumentCampaignDisposition,
) -> AdaptiveInstrumentCampaignDisposition {
    match disposition {
        InstrumentCampaignDisposition::Completed => AdaptiveInstrumentCampaignDisposition::Executed,
        InstrumentCampaignDisposition::Negative => AdaptiveInstrumentCampaignDisposition::Negative,
        InstrumentCampaignDisposition::Partial => AdaptiveInstrumentCampaignDisposition::Partial,
        InstrumentCampaignDisposition::Failed => AdaptiveInstrumentCampaignDisposition::Failed,
        InstrumentCampaignDisposition::Blocked => AdaptiveInstrumentCampaignDisposition::Blocked,
        InstrumentCampaignDisposition::Unresolved => {
            AdaptiveInstrumentCampaignDisposition::Unresolved
        }
    }
}

impl AdaptiveInstrumentCampaign {
    pub fn validate(&self) -> Result<(), AdaptiveInstrumentCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.candidate_order.is_empty()
            || !canonical(&self.candidate_order)
            || self
                .selected_order
                .iter()
                .any(|id| !self.candidate_order.binary_search(id).is_ok())
            || self
                .selected_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || !canonical(&self.endpoint_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.minimum_information_milli == 0
            || self.minimum_endpoint_count > MAX_ENDPOINTS
        {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "identity, ordering, selection bounds, or evidence fields are invalid".into(),
            ));
        }
        let candidate_ids = self
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if candidate_ids.len() != self.candidates.len()
            || self.candidate_order != candidate_ids.iter().cloned().collect::<Vec<_>>()
        {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "candidate snapshots do not match the canonical candidate order".into(),
            ));
        }
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        if selected.len() != self.selected_order.len() {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "selected candidates are duplicated".into(),
            ));
        }
        let mut decisions = BTreeSet::new();
        for decision in &self.decisions {
            if !candidate_ids.contains(&decision.candidate_id)
                || !decisions.insert(decision.candidate_id.clone())
                || decision.reason.trim().is_empty()
                || decision.selected != selected.contains(&decision.candidate_id)
            {
                return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                    "selection decisions do not reconcile with candidate ids".into(),
                ));
            }
        }
        if decisions != candidate_ids {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "every candidate requires exactly one selection decision".into(),
            ));
        }
        let by_id = self
            .candidates
            .iter()
            .map(|candidate| (candidate.candidate_id.as_str(), candidate))
            .collect::<BTreeMap<_, _>>();
        let mut information = 0_u64;
        let mut cost = 0_u64;
        let mut risk = 0_u64;
        let mut endpoints = BTreeSet::new();
        for id in &self.selected_order {
            let candidate = by_id[id.as_str()];
            information = information.saturating_add(candidate.expected_information_milli);
            cost = cost.saturating_add(candidate.estimated_cost_ticks);
            risk = risk.saturating_add(candidate.risk_milli);
            endpoints.insert(candidate.endpoint.clone());
        }
        if (information, cost, risk)
            != (
                self.planned_information_milli,
                self.planned_cost_ticks,
                self.planned_risk_milli,
            )
            || endpoints.iter().cloned().collect::<Vec<_>>() != self.endpoint_order
        {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "planned budget and endpoint summaries do not reconcile".into(),
            ));
        }
        if let Some(campaign) = &self.campaign {
            campaign.validate().map_err(|error| {
                AdaptiveInstrumentCampaignError::InvalidOutput(error.to_string())
            })?;
            let expected_runs = self
                .selected_order
                .iter()
                .map(|id| by_id[id.as_str()].run_id.clone())
                .collect::<Vec<_>>();
            if campaign.run_order != expected_runs || campaign.objective != self.objective {
                return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                    "executed campaign is not bound to the selected candidate order".into(),
                ));
            }
        } else if !matches!(
            self.disposition,
            AdaptiveInstrumentCampaignDisposition::NoFeasiblePlan
        ) {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "only a no-feasible-plan output may omit execution".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveInstrumentCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveInstrumentCampaignError::InvalidOutput(
                "adaptive instrument campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Select a dependency-closed, endpoint-diverse instrument subset and execute it through the
/// guarded campaign controller. The executor remains caller-owned, so the MCP adapter can only
/// rehearse the algorithm with synthetic local artifacts.
pub fn execute_glioma_adaptive_instrument_campaign<E: InstrumentExecutor>(
    request: &AdaptiveInstrumentCampaignRequest,
    executor: &mut E,
) -> Result<AdaptiveInstrumentCampaign, AdaptiveInstrumentCampaignError> {
    let candidates = validate_request(request)?;
    let candidate_order = candidates.keys().cloned().collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut selected_order = Vec::new();
    let mut planned_information = 0_u64;
    let mut planned_cost = 0_u64;
    let mut planned_risk = 0_u64;
    let mut endpoints = BTreeSet::new();
    let mut scored = BTreeMap::new();
    for candidate in candidates.values() {
        scored.insert(candidate.candidate_id.clone(), score(candidate, true));
    }

    while selected.len() < request.max_selected {
        let mut best: Option<(u64, String, Vec<String>)> = None;
        for candidate in candidates.values() {
            if selected.contains(&candidate.candidate_id) {
                continue;
            }
            let mut closure = Vec::new();
            dependency_closure(
                &candidate.candidate_id,
                &candidates,
                &selected,
                &mut BTreeSet::new(),
                &mut closure,
            )?;
            if selected.len().saturating_add(closure.len()) > request.max_selected {
                continue;
            }
            let mut closure_cost = 0_u64;
            let mut closure_risk = 0_u64;
            for id in &closure {
                let item = &candidates[id];
                closure_cost = closure_cost.saturating_add(item.estimated_cost_ticks);
                closure_risk = closure_risk.saturating_add(item.risk_milli);
            }
            if planned_cost.saturating_add(closure_cost) > request.cost_budget_ticks
                || planned_risk.saturating_add(closure_risk) > request.risk_budget_milli
            {
                continue;
            }
            let endpoint_is_new = !endpoints.contains(&candidate.endpoint);
            let candidate_score = score(candidate, endpoint_is_new);
            let better = best.as_ref().is_none_or(|(best_score, best_id, _)| {
                candidate_score > *best_score
                    || (candidate_score == *best_score && candidate.candidate_id < *best_id)
            });
            if better {
                best = Some((candidate_score, candidate.candidate_id.clone(), closure));
            }
        }
        let Some((chosen_score, root_id, closure)) = best else {
            break;
        };
        for id in closure {
            if selected.insert(id.clone()) {
                let candidate = &candidates[&id];
                planned_information =
                    planned_information.saturating_add(candidate.expected_information_milli);
                planned_cost = planned_cost.saturating_add(candidate.estimated_cost_ticks);
                planned_risk = planned_risk.saturating_add(candidate.risk_milli);
                endpoints.insert(candidate.endpoint.clone());
                selected_order.push(id);
            }
        }
        scored.insert(root_id, chosen_score);
    }

    let mut uncertainty = Vec::new();
    if planned_information < request.minimum_information_milli {
        uncertainty.push(format!(
            "information-floor-unmet:{planned_information}<{}",
            request.minimum_information_milli
        ));
    }
    if endpoints.len() < request.minimum_endpoint_count {
        uncertainty.push(format!(
            "endpoint-coverage-unmet:{}<{}",
            endpoints.len(),
            request.minimum_endpoint_count
        ));
    }
    uncertainty.sort();
    let feasible = !selected_order.is_empty()
        && planned_information >= request.minimum_information_milli
        && endpoints.len() >= request.minimum_endpoint_count;
    let mut negative_evidence = Vec::new();
    let (campaign, disposition) = if feasible {
        let runs = selected_order
            .iter()
            .map(|id| InstrumentCampaignRunRequest {
                run_id: candidates[id].run_id.clone(),
                execution: candidates[id].execution.clone(),
            })
            .collect::<Vec<_>>();
        let campaign_request = InstrumentCampaignRequest {
            objective: request.objective.clone(),
            max_runs: runs.len(),
            runs,
            stop_on_negative: request.stop_on_negative,
        };
        let campaign = execute_glioma_instrument_campaign(&campaign_request, executor)?;
        negative_evidence.extend(campaign.negative_evidence.iter().cloned());
        (
            Some(campaign.clone()),
            campaign_disposition(campaign.disposition),
        )
    } else {
        uncertainty
            .push("no dependency-closed plan met information, endpoint, and budget gates".into());
        (None, AdaptiveInstrumentCampaignDisposition::NoFeasiblePlan)
    };
    let mut decisions = candidate_order
        .iter()
        .map(|candidate_id| AdaptiveInstrumentDecision {
            candidate_id: candidate_id.clone(),
            selected: selected.contains(candidate_id),
            score_milli: scored.get(candidate_id).copied().unwrap_or(0),
            reason: if selected.contains(candidate_id) {
                "selected:dependency-closed information-per-cost frontier".into()
            } else {
                "not-selected:budget, dependency closure, or dominated frontier".into()
            },
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = AdaptiveInstrumentCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        candidate_order,
        selected_order,
        endpoint_order: endpoints.into_iter().collect(),
        candidates: request.candidates.clone(),
        decisions,
        planned_information_milli: planned_information,
        planned_cost_ticks: planned_cost,
        planned_risk_milli: planned_risk,
        minimum_information_milli: request.minimum_information_milli,
        minimum_endpoint_count: request.minimum_endpoint_count,
        campaign,
        simulation_only: executor.simulation_only(),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-adaptive-instrument-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveInstrumentCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn dry_run_adaptive_instrument_executor(
    request: &AdaptiveInstrumentCampaignRequest,
) -> Result<DryRunInstrumentExecutor, AdaptiveInstrumentCampaignError> {
    let interlocks = request
        .candidates
        .first()
        .ok_or_else(|| {
            AdaptiveInstrumentCampaignError::InvalidRequest("candidates are required".into())
        })?
        .execution
        .live_interlocks
        .clone();
    Ok(DryRunInstrumentExecutor {
        interlocks,
        emergency_stop_called: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        InstrumentActionDecision, InstrumentActionDisposition, InstrumentAuthorization,
        InstrumentInterlockSnapshot, InstrumentPreflightDisposition, InstrumentPreflightPlan,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn candidate(id: &str, endpoint: &str, dependency: Vec<String>) -> AdaptiveInstrumentCandidate {
        let mut plan = InstrumentPreflightPlan {
            feature_id: "GAF-GLIOMA-P08-F10".into(),
            output_schema: "GliomaInstrumentPreflight1@1".into(),
            objective: format!("assay {id}"),
            instrument_id: format!("imager-{id}"),
            model_system: GliomaModelSystem::Organoid,
            authorization_id: format!("approval-{id}"),
            action_order: vec!["acquire".into()],
            admitted_order: vec!["acquire".into()],
            blocked_order: Vec::new(),
            unresolved_order: Vec::new(),
            decisions: vec![InstrumentActionDecision {
                action_id: "acquire".into(),
                disposition: InstrumentActionDisposition::Admitted,
                scheduled_start_tick: Some(1),
                scheduled_end_tick: Some(2),
                reasons: Vec::new(),
            }],
            total_risk_milli: 10,
            total_duration_ticks: 1,
            required_interlocks: Vec::new(),
            compensation_order: Vec::new(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            dispatch_permitted: true,
            disposition: InstrumentPreflightDisposition::Admitted,
            digest: hash("unsealed-plan"),
        };
        plan.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": plan.feature_id,
            "output_schema": plan.output_schema,
            "objective": plan.objective,
            "instrument_id": plan.instrument_id,
            "model_system": plan.model_system,
            "authorization_id": plan.authorization_id,
            "action_order": plan.action_order,
            "admitted_order": plan.admitted_order,
            "blocked_order": plan.blocked_order,
            "unresolved_order": plan.unresolved_order,
            "decisions": plan.decisions,
            "total_risk_milli": plan.total_risk_milli,
            "total_duration_ticks": plan.total_duration_ticks,
            "required_interlocks": plan.required_interlocks,
            "compensation_order": plan.compensation_order,
            "negative_evidence": plan.negative_evidence,
            "uncertainty": plan.uncertainty,
            "dispatch_permitted": plan.dispatch_permitted,
            "disposition": plan.disposition,
        }))
        .unwrap();
        let execution = InstrumentExecutionRequest {
            objective: format!("assay {id}"),
            plan,
            actions: vec![crate::glioma::programs::p08_instrument_robotics::preflight::InstrumentAction {
                action_id: "acquire".into(),
                instrument_id: format!("imager-{id}"),
                model_system: GliomaModelSystem::Organoid,
                    operation: crate::glioma::programs::p08_instrument_robotics::preflight::InstrumentOperation::AcquireImage,
                requested_start_tick: 1,
                duration_ticks: 1,
                risk_milli: 10,
                parameters: Vec::new(),
                    requires_operator: false,
                    output_schema: "application/json".into(),
            }],
            authorization: InstrumentAuthorization {
                authorization_id: format!("approval-{id}"),
                operator_id: "operator".into(),
                instrument_scope: format!("imager-{id}"),
                approval_digest: hash("approval"),
                issued_tick: 0,
                expires_tick: 10,
                revoked: false,
            },
            live_interlocks: InstrumentInterlockSnapshot {
                observed_tick: 1,
                emergency_stop_clear: true,
                guard_closed: true,
                deck_clear: true,
                consumables_available: true,
                waste_capacity_milli: 100,
                temperature_milli: None,
                minimum_temperature_milli: None,
                maximum_temperature_milli: None,
                calibration_valid_until_tick: 10,
                calibration_sequence_index: 1,
            },
            current_tick: 1,
            minimum_waste_capacity_milli: 1,
            max_retries: 1,
            require_artifacts: true,
        };
        AdaptiveInstrumentCandidate {
            candidate_id: id.into(),
            run_id: format!("run-{id}"),
            endpoint: endpoint.into(),
            execution,
            expected_information_milli: 700,
            frontier_novelty_milli: 500,
            reproducibility_milli: 900,
            estimated_cost_ticks: 2,
            risk_milli: 10,
            depends_on: dependency,
        }
    }

    #[test]
    fn selects_dependency_closed_endpoint_diverse_frontier_and_replays() {
        let request = AdaptiveInstrumentCampaignRequest {
            objective: "adaptive glioma imaging campaign".into(),
            candidates: vec![
                candidate("a", "viability", Vec::new()),
                candidate("b", "invasion", vec!["a".into()]),
                candidate("c", "state", Vec::new()),
            ],
            cost_budget_ticks: 8,
            risk_budget_milli: 100,
            minimum_information_milli: 1_000,
            minimum_endpoint_count: 1,
            max_selected: 3,
            stop_on_negative: true,
        };
        let mut executor = dry_run_adaptive_instrument_executor(&request).unwrap();
        let output = execute_glioma_adaptive_instrument_campaign(&request, &mut executor).unwrap();
        output.validate().unwrap();
        assert_eq!(
            output.disposition,
            AdaptiveInstrumentCampaignDisposition::Executed
        );
        assert_eq!(output.selected_order, vec!["a", "b", "c"]);
        assert_eq!(
            output.endpoint_order,
            vec!["invasion", "state", "viability"]
        );
        assert_eq!(
            output.campaign.as_ref().unwrap().completed_run_order.len(),
            3
        );
        let mut replay_executor = dry_run_adaptive_instrument_executor(&request).unwrap();
        let replay =
            execute_glioma_adaptive_instrument_campaign(&request, &mut replay_executor).unwrap();
        assert_eq!(output, replay);
    }

    #[test]
    fn holds_when_information_or_endpoint_gate_cannot_be_met() {
        let request = AdaptiveInstrumentCampaignRequest {
            objective: "underpowered glioma assay campaign".into(),
            candidates: vec![candidate("a", "viability", Vec::new())],
            cost_budget_ticks: 1,
            risk_budget_milli: 100,
            minimum_information_milli: 10_000,
            minimum_endpoint_count: 1,
            max_selected: 1,
            stop_on_negative: false,
        };
        assert!(matches!(
            execute_glioma_adaptive_instrument_campaign(
                &request,
                &mut dry_run_adaptive_instrument_executor(&request).unwrap()
            )
            .unwrap()
            .disposition,
            AdaptiveInstrumentCampaignDisposition::NoFeasiblePlan
        ));
    }
}
