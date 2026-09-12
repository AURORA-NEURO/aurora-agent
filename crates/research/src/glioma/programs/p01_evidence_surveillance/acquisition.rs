//! Budgeted evidence-acquisition portfolio planning for autonomous glioma research.
//!
//! Surveillance and priority queues identify evidence debt one record at a time. This feature
//! solves the harder autonomous-engine problem: choose a dependency-closed, source-diverse
//! portfolio of literature, dataset, assay, and replication acquisitions under cost, privacy,
//! and independence constraints. It ranks expected support, uncertainty reduction, contradiction
//! resolution, freshness, workflow leverage, reproducibility, and failure risk without claiming
//! that an acquisition has happened or that a hypothesis is true.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceAcquisitionPlan1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_SELECTED: usize = 256;
pub const MAX_BEAM_WIDTH: usize = 256;
const SCORE_SCALE: i64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionSourceKind {
    Literature,
    Dataset,
    Assay,
    Replication,
    Simulation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionWeights {
    pub support_milli: u16,
    pub uncertainty_reduction_milli: u16,
    pub contradiction_resolution_milli: u16,
    pub freshness_milli: u16,
    pub workflow_leverage_milli: u16,
    pub reproducibility_milli: u16,
    pub failure_penalty_milli: u16,
    pub cost_penalty_milli: u16,
}

impl Default for EvidenceAcquisitionWeights {
    fn default() -> Self {
        Self {
            support_milli: 180,
            uncertainty_reduction_milli: 180,
            contradiction_resolution_milli: 160,
            freshness_milli: 90,
            workflow_leverage_milli: 150,
            reproducibility_milli: 120,
            failure_penalty_milli: 70,
            cost_penalty_milli: 50,
        }
    }
}

impl EvidenceAcquisitionWeights {
    fn validate(self) -> Result<(), EvidenceAcquisitionError> {
        let total = u32::from(self.support_milli)
            + u32::from(self.uncertainty_reduction_milli)
            + u32::from(self.contradiction_resolution_milli)
            + u32::from(self.freshness_milli)
            + u32::from(self.workflow_leverage_milli)
            + u32::from(self.reproducibility_milli)
            + u32::from(self.failure_penalty_milli)
            + u32::from(self.cost_penalty_milli);
        if total != SCORE_SCALE as u32 {
            return Err(EvidenceAcquisitionError::InvalidRequest(
                "acquisition weights must sum to 1,000 milli-units".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionCandidate {
    pub candidate_id: String,
    pub target_claim: String,
    pub source_family: String,
    pub source_kind: EvidenceAcquisitionSourceKind,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub independence_group: String,
    pub depends_on: Vec<String>,
    pub cost_units: u64,
    pub expected_support_milli: u16,
    pub expected_uncertainty_reduction_milli: u16,
    pub contradiction_resolution_milli: u16,
    pub freshness_milli: u16,
    pub workflow_leverage_milli: u16,
    pub reproducibility_milli: u16,
    pub failure_probability_milli: u16,
    pub privacy_risk_milli: u16,
    pub local_only: bool,
    pub contains_human_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionRequest {
    pub objective: String,
    pub budget_units: u64,
    pub max_candidates: usize,
    pub max_selected: usize,
    pub beam_width: usize,
    pub min_source_families: usize,
    pub max_per_independence_group: usize,
    pub max_privacy_risk_milli: u32,
    pub min_portfolio_score_milli: u16,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub weights: EvidenceAcquisitionWeights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionSelection {
    pub candidate_id: String,
    pub rank: u16,
    pub marginal_value_milli: i32,
    pub cumulative_cost_units: u64,
    pub cumulative_privacy_risk_milli: u32,
    pub expected_value_milli: i32,
    pub worst_case_value_milli: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub selections: Vec<EvidenceAcquisitionSelection>,
    pub total_cost_units: u64,
    pub total_privacy_risk_milli: u32,
    pub expected_value_milli: i32,
    pub worst_case_value_milli: i32,
    pub source_family_order: Vec<String>,
    pub covered_modality_order: Vec<GliomaModality>,
    pub covered_model_system_order: Vec<GliomaModelSystem>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceAcquisitionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceAcquisitionError {
    #[error("evidence-acquisition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-acquisition candidate graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("evidence-acquisition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-acquisition digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &EvidenceAcquisitionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "frontier_order": output.frontier_order,
        "selections": output.selections,
        "total_cost_units": output.total_cost_units,
        "total_privacy_risk_milli": output.total_privacy_risk_milli,
        "expected_value_milli": output.expected_value_milli,
        "worst_case_value_milli": output.worst_case_value_milli,
        "source_family_order": output.source_family_order,
        "covered_modality_order": output.covered_modality_order,
        "covered_model_system_order": output.covered_model_system_order,
        "missing_modality_order": output.missing_modality_order,
        "missing_model_system_order": output.missing_model_system_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl EvidenceAcquisitionPlan {
    pub fn validate(&self) -> Result<(), EvidenceAcquisitionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.frontier_order)
            || !canonical(&self.source_family_order)
            || !canonical(&self.covered_modality_order)
            || !canonical(&self.covered_model_system_order)
            || !canonical(&self.missing_modality_order)
            || !canonical(&self.missing_model_system_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.digest.as_str().len() != 64
            || self.selections.len() != self.selected_order.len()
            || self.selections.iter().any(|selection| {
                selection.candidate_id.trim().is_empty()
                    || selection.rank == 0
                    || selection.marginal_value_milli < -1_000
                    || selection.marginal_value_milli > 1_000
                    || selection.expected_value_milli < -1_000_000
                    || selection.expected_value_milli > 1_000_000
                    || selection.worst_case_value_milli < -1_000_000
                    || selection.worst_case_value_milli > 1_000_000
            })
        {
            return Err(EvidenceAcquisitionError::InvalidOutput(
                "identity, canonical partitions, scores, selection rows, or digest fields are invalid"
                    .into(),
            ));
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        if candidates.len() != self.candidate_order.len()
            || selected.len() != self.selected_order.len()
            || deferred.len() != self.deferred_order.len()
            || blocked.len() != self.blocked_order.len()
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&blocked).next().is_some()
            || deferred.intersection(&blocked).next().is_some()
            || selected
                .union(&deferred)
                .cloned()
                .collect::<BTreeSet<_>>()
                .union(&blocked)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidates
            || self
                .selections
                .iter()
                .map(|selection| selection.candidate_id.clone())
                .collect::<BTreeSet<_>>()
                != selected
            || self
                .frontier_order
                .iter()
                .any(|candidate| !candidates.contains(candidate))
        {
            return Err(EvidenceAcquisitionError::InvalidOutput(
                "candidate partitions or selection identities do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceAcquisitionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceAcquisitionError::InvalidOutput(
                "evidence-acquisition digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &EvidenceAcquisitionRequest) -> Result<(), EvidenceAcquisitionError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_candidates == 0
        || request.max_candidates > MAX_CANDIDATES
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
        || request.beam_width == 0
        || request.beam_width > MAX_BEAM_WIDTH
        || request.min_source_families > request.max_selected
        || request.max_per_independence_group == 0
        || request.max_privacy_risk_milli > 1_000_000
        || request.min_portfolio_score_milli > SCORE_SCALE as u16
    {
        return Err(EvidenceAcquisitionError::InvalidRequest(
            "objective, bounded budget, candidate/selection/beam limits, source floor, risk ceiling, and score floor are required".into(),
        ));
    }
    request.weights.validate()
}

fn validate_candidates(
    request: &EvidenceAcquisitionRequest,
    candidates: &[EvidenceAcquisitionCandidate],
) -> Result<BTreeMap<String, EvidenceAcquisitionCandidate>, EvidenceAcquisitionError> {
    if candidates.is_empty() || candidates.len() > request.max_candidates {
        return Err(EvidenceAcquisitionError::InvalidGraph(
            "a non-empty candidate set within the declared bound is required".into(),
        ));
    }
    let mut map = BTreeMap::new();
    for candidate in candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.target_claim.trim().is_empty()
            || candidate.source_family.trim().is_empty()
            || candidate.independence_group.trim().is_empty()
            || candidate.cost_units == 0
            || candidate.cost_units > request.budget_units
            || candidate.expected_support_milli > SCORE_SCALE as u16
            || candidate.expected_uncertainty_reduction_milli > SCORE_SCALE as u16
            || candidate.contradiction_resolution_milli > SCORE_SCALE as u16
            || candidate.freshness_milli > SCORE_SCALE as u16
            || candidate.workflow_leverage_milli > SCORE_SCALE as u16
            || candidate.reproducibility_milli > SCORE_SCALE as u16
            || candidate.failure_probability_milli > SCORE_SCALE as u16
            || candidate.privacy_risk_milli > SCORE_SCALE as u16
            || candidate
                .depends_on
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate.depends_on.iter().any(|dependency| {
                dependency.trim().is_empty() || dependency == &candidate.candidate_id
            })
            || map
                .insert(candidate.candidate_id.clone(), candidate.clone())
                .is_some()
        {
            return Err(EvidenceAcquisitionError::InvalidGraph(
                "candidate identity, claim/source scope, bounded scores/cost, and canonical dependencies are required".into(),
            ));
        }
    }
    for candidate in map.values() {
        if candidate
            .depends_on
            .iter()
            .any(|dependency| !map.contains_key(dependency))
        {
            return Err(EvidenceAcquisitionError::InvalidGraph(
                "candidate dependency references an unknown acquisition".into(),
            ));
        }
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    fn visit(
        id: &str,
        map: &BTreeMap<String, EvidenceAcquisitionCandidate>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visited.contains(id) {
            return true;
        }
        if !visiting.insert(id.to_string()) {
            return false;
        }
        let acyclic = map[id]
            .depends_on
            .iter()
            .all(|dependency| visit(dependency, map, visiting, visited));
        visiting.remove(id);
        if acyclic {
            visited.insert(id.to_string());
        }
        acyclic
    }
    if map
        .keys()
        .any(|id| !visit(id, &map, &mut visiting, &mut visited))
    {
        return Err(EvidenceAcquisitionError::InvalidGraph(
            "candidate dependency graph contains a cycle".into(),
        ));
    }
    Ok(map)
}

fn closure_for(
    id: &str,
    map: &BTreeMap<String, EvidenceAcquisitionCandidate>,
    selected: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    if selected.contains(id) || output.contains(id) {
        return;
    }
    for dependency in &map[id].depends_on {
        closure_for(dependency, map, selected, output);
    }
    output.insert(id.to_string());
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortfolioScore {
    cost_units: u64,
    privacy_risk_milli: u32,
    expected_value_milli: i32,
    worst_case_value_milli: i32,
    source_families: BTreeSet<String>,
    modalities: BTreeSet<GliomaModality>,
    model_systems: BTreeSet<GliomaModelSystem>,
}

fn candidate_value(
    candidate: &EvidenceAcquisitionCandidate,
    weights: EvidenceAcquisitionWeights,
    budget_units: u64,
    duplicate_target: bool,
    duplicate_family: bool,
) -> i32 {
    let values = [
        i64::from(candidate.expected_support_milli),
        i64::from(candidate.expected_uncertainty_reduction_milli),
        i64::from(candidate.contradiction_resolution_milli),
        i64::from(candidate.freshness_milli),
        i64::from(candidate.workflow_leverage_milli),
        i64::from(candidate.reproducibility_milli),
    ];
    let positive_weights = [
        i64::from(weights.support_milli),
        i64::from(weights.uncertainty_reduction_milli),
        i64::from(weights.contradiction_resolution_milli),
        i64::from(weights.freshness_milli),
        i64::from(weights.workflow_leverage_milli),
        i64::from(weights.reproducibility_milli),
    ];
    let positive = values
        .iter()
        .zip(positive_weights)
        .map(|(value, weight)| value * weight)
        .sum::<i64>()
        .saturating_div(SCORE_SCALE);
    let failure_penalty = i64::from(candidate.failure_probability_milli)
        * i64::from(weights.failure_penalty_milli)
        / SCORE_SCALE;
    let normalized_cost = candidate
        .cost_units
        .saturating_mul(SCORE_SCALE as u64)
        .checked_div(budget_units.max(1))
        .unwrap_or(SCORE_SCALE as u64)
        .min(SCORE_SCALE as u64) as i64;
    let cost_penalty = normalized_cost * i64::from(weights.cost_penalty_milli) / SCORE_SCALE;
    let mut value = positive - failure_penalty - cost_penalty;
    if duplicate_target {
        value = value * 500 / SCORE_SCALE;
    } else if duplicate_family {
        value = value * 750 / SCORE_SCALE;
    }
    value.clamp(-1_000, 1_000) as i32
}

fn score_portfolio(
    selected: &BTreeSet<String>,
    map: &BTreeMap<String, EvidenceAcquisitionCandidate>,
    request: &EvidenceAcquisitionRequest,
) -> PortfolioScore {
    let mut cost_units = 0_u64;
    let mut privacy_risk_milli = 0_u32;
    let mut expected_value = 0_i32;
    let mut worst_case_value = 0_i32;
    let mut source_families = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut model_systems = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for id in selected {
        let candidate = &map[id];
        let duplicate_target = !claims.insert(candidate.target_claim.clone());
        let duplicate_family = !source_families.insert(candidate.source_family.clone());
        cost_units = cost_units.saturating_add(candidate.cost_units);
        privacy_risk_milli =
            privacy_risk_milli.saturating_add(u32::from(candidate.privacy_risk_milli));
        let value = candidate_value(
            candidate,
            request.weights,
            request.budget_units,
            duplicate_target,
            duplicate_family,
        );
        expected_value = expected_value.saturating_add(value);
        let failure_discount = i32::from(SCORE_SCALE as u16 - candidate.failure_probability_milli);
        worst_case_value = worst_case_value
            .saturating_add(value.saturating_mul(failure_discount) / SCORE_SCALE as i32);
        modalities.insert(candidate.modality);
        if let Some(model_system) = candidate.model_system {
            model_systems.insert(model_system);
        }
    }
    let diversity_bonus = (source_families.len() as i32)
        .saturating_mul(40)
        .saturating_add((modalities.len() as i32).saturating_mul(20));
    expected_value = expected_value.saturating_add(diversity_bonus);
    worst_case_value = worst_case_value.saturating_add(diversity_bonus / 2);
    PortfolioScore {
        cost_units,
        privacy_risk_milli,
        expected_value_milli: expected_value,
        worst_case_value_milli: worst_case_value,
        source_families,
        modalities,
        model_systems,
    }
}

fn state_key(selected: &BTreeSet<String>) -> String {
    selected.iter().cloned().collect::<Vec<_>>().join("|")
}

fn better_state(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    map: &BTreeMap<String, EvidenceAcquisitionCandidate>,
    request: &EvidenceAcquisitionRequest,
) -> bool {
    let left_score = score_portfolio(left, map, request);
    let right_score = score_portfolio(right, map, request);
    (
        left_score.worst_case_value_milli,
        left_score.expected_value_milli,
        -(left_score.cost_units as i64),
        state_key(left),
    ) > (
        right_score.worst_case_value_milli,
        right_score.expected_value_milli,
        -(right_score.cost_units as i64),
        state_key(right),
    )
}

fn covers_requirements(score: &PortfolioScore, request: &EvidenceAcquisitionRequest) -> bool {
    request
        .required_modalities
        .iter()
        .all(|modality| score.modalities.contains(modality))
        && request
            .required_model_systems
            .iter()
            .all(|model_system| score.model_systems.contains(model_system))
        && score.source_families.len() >= request.min_source_families
        && score.expected_value_milli >= i32::from(request.min_portfolio_score_milli)
        && score.privacy_risk_milli <= request.max_privacy_risk_milli
}

/// Select a robust, dependency-closed evidence-acquisition portfolio for the autonomous
/// research engine. The planner never fetches a source or claims that a candidate succeeded.
pub fn plan_glioma_evidence_acquisition(
    request: &EvidenceAcquisitionRequest,
    candidates: &[EvidenceAcquisitionCandidate],
) -> Result<EvidenceAcquisitionPlan, EvidenceAcquisitionError> {
    validate_request(request)?;
    let map = validate_candidates(request, candidates)?;
    let candidate_order = map.keys().cloned().collect::<Vec<_>>();
    let mut policy_blocked = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for candidate in map.values() {
        if candidate.contains_human_data {
            policy_blocked.insert(candidate.candidate_id.clone());
            negative_evidence.insert(format!(
                "{}:human-data-out-of-scope",
                candidate.candidate_id
            ));
        } else if !candidate.local_only {
            policy_blocked.insert(candidate.candidate_id.clone());
            negative_evidence.insert(format!(
                "{}:non-local-payload-requires-federation-gate",
                candidate.candidate_id
            ));
        } else if u32::from(candidate.privacy_risk_milli) > request.max_privacy_risk_milli {
            policy_blocked.insert(candidate.candidate_id.clone());
            negative_evidence.insert(format!(
                "{}:privacy-risk-exceeds-ceiling",
                candidate.candidate_id
            ));
        }
    }
    let available = map
        .keys()
        .filter(|id| !policy_blocked.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut beam = vec![BTreeSet::new()];
    let mut seen = BTreeSet::new();
    seen.insert(String::new());
    for _ in 0..request.max_selected {
        let mut next = Vec::new();
        for state in &beam {
            for candidate_id in &available {
                if state.contains(candidate_id) {
                    continue;
                }
                let mut expanded = BTreeSet::new();
                closure_for(candidate_id, &map, state, &mut expanded);
                if expanded.iter().any(|id| policy_blocked.contains(id))
                    || state.len().saturating_add(expanded.len()) > request.max_selected
                {
                    continue;
                }
                let mut proposed = state.clone();
                proposed.extend(expanded);
                let score = score_portfolio(&proposed, &map, request);
                if score.cost_units > request.budget_units
                    || score.privacy_risk_milli > request.max_privacy_risk_milli
                    || map
                        .values()
                        .filter(|candidate| {
                            proposed.contains(&candidate.candidate_id)
                                && candidate.independence_group
                                    == map[candidate_id].independence_group
                        })
                        .count()
                        > request.max_per_independence_group
                {
                    continue;
                }
                let key = state_key(&proposed);
                if seen.insert(key) {
                    next.push(proposed);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by(|left, right| {
            if better_state(left, right, &map, request) {
                std::cmp::Ordering::Less
            } else if better_state(right, left, &map, request) {
                std::cmp::Ordering::Greater
            } else {
                state_key(left).cmp(&state_key(right))
            }
        });
        next.truncate(request.beam_width);
        beam = next;
    }
    let mut best = BTreeSet::new();
    for state in beam {
        if (covers_requirements(&score_portfolio(&state, &map, request), request)
            && !covers_requirements(&score_portfolio(&best, &map, request), request))
            || (covers_requirements(&score_portfolio(&state, &map, request), request)
                == covers_requirements(&score_portfolio(&best, &map, request), request)
                && better_state(&state, &best, &map, request))
        {
            best = state;
        }
    }
    let best_score = score_portfolio(&best, &map, request);
    let selected_order = best.iter().cloned().collect::<Vec<_>>();
    let blocked_order = policy_blocked.iter().cloned().collect::<Vec<_>>();
    let deferred_order = candidate_order
        .iter()
        .filter(|id| !best.contains(*id) && !policy_blocked.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut uncertainty = BTreeSet::new();
    let covered_modalities = best_score.modalities.iter().copied().collect::<Vec<_>>();
    let covered_models = best_score.model_systems.iter().copied().collect::<Vec<_>>();
    let missing_modalities = request
        .required_modalities
        .difference(&best_score.modalities)
        .copied()
        .collect::<Vec<_>>();
    let missing_models = request
        .required_model_systems
        .difference(&best_score.model_systems)
        .copied()
        .collect::<Vec<_>>();
    if !missing_modalities.is_empty() {
        uncertainty.insert("required-modality-coverage-incomplete".into());
    }
    if !missing_models.is_empty() {
        uncertainty.insert("required-model-system-coverage-incomplete".into());
    }
    if best_score.source_families.len() < request.min_source_families {
        uncertainty.insert("independent-source-family-floor-unmet".into());
    }
    if best.is_empty() {
        uncertainty.insert("no-acquisition-admitted".into());
    }
    if !deferred_order.is_empty() {
        uncertainty.insert("budget-or-diversity-deferred-candidates".into());
    }
    let frontier_order = available
        .iter()
        .filter(|id| !best.contains(*id))
        .take(request.beam_width)
        .cloned()
        .collect::<Vec<_>>();
    let mut selections = best
        .iter()
        .map(|candidate_id| {
            let candidate = &map[candidate_id];
            let marginal = candidate_value(
                candidate,
                request.weights,
                request.budget_units,
                false,
                false,
            );
            EvidenceAcquisitionSelection {
                candidate_id: candidate_id.clone(),
                rank: 0,
                marginal_value_milli: marginal,
                cumulative_cost_units: 0,
                cumulative_privacy_risk_milli: 0,
                expected_value_milli: best_score.expected_value_milli,
                worst_case_value_milli: best_score.worst_case_value_milli,
            }
        })
        .collect::<Vec<_>>();
    selections.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let mut cumulative_cost = 0_u64;
    let mut cumulative_privacy = 0_u32;
    for (index, selection) in selections.iter_mut().enumerate() {
        let candidate = &map[&selection.candidate_id];
        cumulative_cost = cumulative_cost.saturating_add(candidate.cost_units);
        cumulative_privacy =
            cumulative_privacy.saturating_add(u32::from(candidate.privacy_risk_milli));
        selection.rank = (index + 1) as u16;
        selection.cumulative_cost_units = cumulative_cost;
        selection.cumulative_privacy_risk_milli = cumulative_privacy;
    }
    let disposition = if best.is_empty() && blocked_order.len() == candidate_order.len() {
        EvidenceAcquisitionDisposition::Blocked
    } else if covers_requirements(&best_score, request) && blocked_order.is_empty() {
        EvidenceAcquisitionDisposition::Ready
    } else if !best.is_empty() {
        EvidenceAcquisitionDisposition::Partial
    } else {
        EvidenceAcquisitionDisposition::Unresolved
    };
    let mut output = EvidenceAcquisitionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        candidate_order,
        selected_order,
        deferred_order,
        blocked_order,
        frontier_order,
        selections,
        total_cost_units: best_score.cost_units,
        total_privacy_risk_milli: best_score.privacy_risk_milli,
        expected_value_milli: best_score.expected_value_milli,
        worst_case_value_milli: best_score.worst_case_value_milli,
        source_family_order: best_score.source_families.into_iter().collect(),
        covered_modality_order: covered_modalities,
        covered_model_system_order: covered_models,
        missing_modality_order: missing_modalities,
        missing_model_system_order: missing_models,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-acquisition"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceAcquisitionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        id: &str,
        family: &str,
        modality: GliomaModality,
        model_system: Option<GliomaModelSystem>,
        depends_on: Vec<String>,
    ) -> EvidenceAcquisitionCandidate {
        EvidenceAcquisitionCandidate {
            candidate_id: id.into(),
            target_claim: "EGFR signaling and invasion".into(),
            source_family: family.into(),
            source_kind: EvidenceAcquisitionSourceKind::Literature,
            modality,
            model_system,
            independence_group: id.split('-').next().unwrap_or(id).into(),
            depends_on,
            cost_units: 2,
            expected_support_milli: 800,
            expected_uncertainty_reduction_milli: 700,
            contradiction_resolution_milli: 600,
            freshness_milli: 600,
            workflow_leverage_milli: 700,
            reproducibility_milli: 800,
            failure_probability_milli: 100,
            privacy_risk_milli: 50,
            local_only: true,
            contains_human_data: false,
        }
    }

    fn request() -> EvidenceAcquisitionRequest {
        EvidenceAcquisitionRequest {
            objective: "close glioma invasion evidence debt".into(),
            budget_units: 6,
            max_candidates: 8,
            max_selected: 3,
            beam_width: 8,
            min_source_families: 2,
            max_per_independence_group: 2,
            max_privacy_risk_milli: 500,
            min_portfolio_score_milli: 100,
            required_modalities: [GliomaModality::Genomics, GliomaModality::Imaging]
                .into_iter()
                .collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            weights: EvidenceAcquisitionWeights::default(),
        }
    }

    #[test]
    fn acquisition_is_permutation_stable_and_closes_dependencies() {
        let mut candidates = vec![
            candidate(
                "literature-a",
                "pubmed",
                GliomaModality::Genomics,
                Some(GliomaModelSystem::Organoid),
                Vec::new(),
            ),
            candidate(
                "dataset-b",
                "atlas",
                GliomaModality::Imaging,
                Some(GliomaModelSystem::Organoid),
                vec!["literature-a".into()],
            ),
            candidate(
                "replicate-c",
                "consortium",
                GliomaModality::Imaging,
                Some(GliomaModelSystem::Organoid),
                Vec::new(),
            ),
        ];
        let first = plan_glioma_evidence_acquisition(&request(), &candidates).unwrap();
        candidates.reverse();
        let second = plan_glioma_evidence_acquisition(&request(), &candidates).unwrap();
        assert_eq!(first, second);
        assert!(first.selected_order.contains(&"literature-a".into()));
        assert!(first.selected_order.contains(&"dataset-b".into()));
        assert_eq!(first.disposition, EvidenceAcquisitionDisposition::Ready);
        first.validate().unwrap();
    }

    #[test]
    fn acquisition_blocks_human_data_and_preserves_missing_coverage() {
        let mut request = request();
        request.required_modalities = [GliomaModality::Proteomics].into_iter().collect();
        let mut human = candidate(
            "human-cohort",
            "clinical",
            GliomaModality::Proteomics,
            None,
            Vec::new(),
        );
        human.contains_human_data = true;
        let output = plan_glioma_evidence_acquisition(&request, &[human]).unwrap();
        assert_eq!(output.disposition, EvidenceAcquisitionDisposition::Blocked);
        assert_eq!(output.blocked_order, vec!["human-cohort"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("human-data")));
        output.validate().unwrap();
    }

    #[test]
    fn acquisition_refuses_dependency_cycles() {
        let mut a = candidate(
            "a",
            "family-a",
            GliomaModality::Genomics,
            Some(GliomaModelSystem::Organoid),
            vec!["b".into()],
        );
        let mut b = candidate(
            "b",
            "family-b",
            GliomaModality::Imaging,
            Some(GliomaModelSystem::Organoid),
            vec!["a".into()],
        );
        a.independence_group = "a".into();
        b.independence_group = "b".into();
        assert!(matches!(
            plan_glioma_evidence_acquisition(&request(), &[a, b]),
            Err(EvidenceAcquisitionError::InvalidGraph(_))
        ));
    }
}
