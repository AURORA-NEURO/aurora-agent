//! Execute a selected evidence-acquisition portfolio through a caller-owned local adapter.
//!
//! The acquisition planner deliberately stops at a dependency-closed portfolio. This controller
//! is the next product boundary: it orders that portfolio, retries only retryable adapter
//! failures, stops or continues on negative evidence according to policy, and preserves every
//! partial/unknown/blocked outcome. It never opens a network connection or treats a dry-run
//! artifact as biological evidence.

use super::acquisition::{
    EvidenceAcquisitionCandidate, EvidenceAcquisitionDisposition, EvidenceAcquisitionPlan,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceAcquisitionCampaign1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionCampaignRequest {
    pub objective: String,
    pub plan: EvidenceAcquisitionPlan,
    pub candidates: Vec<EvidenceAcquisitionCandidate>,
    pub budget_units: u64,
    pub max_retries: u8,
    pub stop_on_negative: bool,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceAcquisitionExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local literature, dataset, assay, simulation, or replication adapters implement
/// this seam. The controller only supplies typed metadata and receives a local result envelope.
pub trait EvidenceAcquisitionExecutor {
    fn acquire(
        &mut self,
        candidate: &EvidenceAcquisitionCandidate,
        attempt: u8,
    ) -> Result<EvidenceAcquisitionResult, EvidenceAcquisitionExecutionFailure>;
}

/// Deterministic sandbox adapter. Unknown simulation outcomes are intentionally not promoted to
/// evidence; production adapters must supply their own validated local result.
#[derive(Debug, Default)]
pub struct DryRunEvidenceAcquisitionExecutor;

impl EvidenceAcquisitionExecutor for DryRunEvidenceAcquisitionExecutor {
    fn acquire(
        &mut self,
        candidate: &EvidenceAcquisitionCandidate,
        attempt: u8,
    ) -> Result<EvidenceAcquisitionResult, EvidenceAcquisitionExecutionFailure> {
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "candidate_id": candidate.candidate_id,
            "source_kind": candidate.source_kind,
            "modality": candidate.modality,
            "model_system": candidate.model_system,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| EvidenceAcquisitionExecutionFailure {
            reason: format!("dry-run acquisition digest failed: {error}"),
            retryable: false,
        })?;
        Ok(EvidenceAcquisitionResult {
            candidate_id: candidate.candidate_id.clone(),
            disposition: EvidenceAcquisitionResultDisposition::Unknown,
            attempt_count: attempt,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("dry-run-acquisition:{}", candidate.candidate_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.evidence-acquisition+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            note: "dry-run acquisition completed without retrieving or asserting evidence".into(),
            uncertainty: vec!["simulation-only-result".into()],
            negative_evidence: vec!["synthetic-dry-run-not-biological-evidence".into()],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionResultDisposition {
    Completed,
    Negative,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionResult {
    pub candidate_id: String,
    pub disposition: EvidenceAcquisitionResultDisposition,
    pub attempt_count: u8,
    pub artifact: Option<LocalArtifactRef>,
    pub note: String,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionCampaignDisposition {
    Completed,
    Partial,
    Negative,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcquisitionCampaignStopReason {
    Completed,
    NegativeEvidence,
    BudgetExhausted,
    ExecutorFailed,
    DependencyBlocked,
    EmptySelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAcquisitionCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub execution_order: Vec<String>,
    pub results: Vec<EvidenceAcquisitionResult>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub simulation_only: bool,
    pub disposition: EvidenceAcquisitionCampaignDisposition,
    pub stop_reason: EvidenceAcquisitionCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceAcquisitionCampaignError {
    #[error("evidence-acquisition campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-acquisition plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("evidence-acquisition campaign execution failed: {0}")]
    Execution(String),
    #[error("evidence-acquisition campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-acquisition campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(output: &EvidenceAcquisitionCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "candidate_order": output.candidate_order,
        "execution_order": output.execution_order,
        "results": output.results,
        "completed_order": output.completed_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "unknown_order": output.unknown_order,
        "failed_order": output.failed_order,
        "blocked_order": output.blocked_order,
        "deferred_order": output.deferred_order,
        "retry_count": output.retry_count,
        "budget_spent_units": output.budget_spent_units,
        "remaining_budget_units": output.remaining_budget_units,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl EvidenceAcquisitionCampaign {
    pub fn validate(&self) -> Result<(), EvidenceAcquisitionCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.plan_digest.as_str().len() != 64
            || !canonical(&self.candidate_order)
            || !unique(&self.execution_order)
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.unknown_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.results.iter().any(|result| {
                result.candidate_id.trim().is_empty()
                    || result.attempt_count == 0
                    || result.note.trim().is_empty()
                    || result.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
                    || result
                        .negative_evidence
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
        {
            return Err(EvidenceAcquisitionCampaignError::InvalidOutput(
                "identity, canonical partitions, result envelopes, or digest fields are invalid"
                    .into(),
            ));
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let executed = self
            .execution_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let completed = self
            .completed_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let negative = self.negative_order.iter().cloned().collect::<BTreeSet<_>>();
        let partial = self.partial_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        let failed = self.failed_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let statuses = completed
            .union(&negative)
            .chain(partial.iter())
            .chain(unknown.iter())
            .chain(failed.iter())
            .chain(blocked.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let result_ids = self
            .results
            .iter()
            .map(|result| result.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if candidates.len() != self.candidate_order.len()
            || executed.len() != self.execution_order.len()
            || completed.len() != self.completed_order.len()
            || negative.len() != self.negative_order.len()
            || partial.len() != self.partial_order.len()
            || unknown.len() != self.unknown_order.len()
            || failed.len() != self.failed_order.len()
            || blocked.len() != self.blocked_order.len()
            || deferred.len() != self.deferred_order.len()
            || result_ids
                != completed
                    .union(&negative)
                    .chain(partial.iter())
                    .chain(unknown.iter())
                    .chain(failed.iter())
                    .cloned()
                    .collect()
            || statuses.union(&deferred).cloned().collect::<BTreeSet<_>>() != candidates
            || executed != result_ids
            || statuses.intersection(&deferred).next().is_some()
        {
            return Err(EvidenceAcquisitionCampaignError::InvalidOutput(
                "execution results and candidate partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceAcquisitionCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceAcquisitionCampaignError::InvalidOutput(
                "acquisition campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &EvidenceAcquisitionCampaignRequest,
) -> Result<BTreeMap<String, EvidenceAcquisitionCandidate>, EvidenceAcquisitionCampaignError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_retries > MAX_RETRIES
        || request.plan.objective != request.objective
    {
        return Err(EvidenceAcquisitionCampaignError::InvalidRequest(
            "objective, bounded budget, candidates, retries, and plan objective are required"
                .into(),
        ));
    }
    request
        .plan
        .validate()
        .map_err(|error| EvidenceAcquisitionCampaignError::InvalidPlan(error.to_string()))?;
    let mut map = BTreeMap::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.target_claim.trim().is_empty()
            || candidate.source_family.trim().is_empty()
            || candidate.independence_group.trim().is_empty()
            || candidate.cost_units == 0
            || candidate.cost_units > request.budget_units
            || candidate.contains_human_data
            || !candidate.local_only
            || candidate
                .depends_on
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || map
                .insert(candidate.candidate_id.clone(), candidate.clone())
                .is_some()
        {
            return Err(EvidenceAcquisitionCampaignError::InvalidRequest(
                "candidate identity, local-only policy, bounded cost, and uniqueness are required"
                    .into(),
            ));
        }
    }
    let declared = map.keys().cloned().collect::<Vec<_>>();
    if declared != request.plan.candidate_order {
        return Err(EvidenceAcquisitionCampaignError::InvalidPlan(
            "candidate order does not match the content-addressed acquisition plan".into(),
        ));
    }
    for candidate in map.values() {
        if candidate
            .depends_on
            .iter()
            .any(|dependency| !map.contains_key(dependency))
        {
            return Err(EvidenceAcquisitionCampaignError::InvalidRequest(
                "candidate dependency references an unknown candidate".into(),
            ));
        }
    }
    let selected = request
        .plan
        .selected_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request
        .plan
        .selected_order
        .iter()
        .any(|id| !map.contains_key(id))
        || request.plan.selected_order.iter().any(|id| {
            map[id]
                .depends_on
                .iter()
                .any(|dependency| !selected.contains(dependency))
        })
    {
        return Err(EvidenceAcquisitionCampaignError::InvalidPlan(
            "selected acquisition portfolio is not dependency-closed".into(),
        ));
    }
    Ok(map)
}

fn topological_order(
    selected: &BTreeSet<String>,
    map: &BTreeMap<String, EvidenceAcquisitionCandidate>,
) -> Result<Vec<String>, EvidenceAcquisitionCampaignError> {
    fn visit(
        id: &str,
        selected: &BTreeSet<String>,
        map: &BTreeMap<String, EvidenceAcquisitionCandidate>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        output: &mut Vec<String>,
    ) -> bool {
        if visited.contains(id) {
            return true;
        }
        if !visiting.insert(id.to_string()) {
            return false;
        }
        let ok = map[id]
            .depends_on
            .iter()
            .filter(|dependency| selected.contains(*dependency))
            .all(|dependency| visit(dependency, selected, map, visiting, visited, output));
        visiting.remove(id);
        if ok {
            visited.insert(id.to_string());
            output.push(id.to_string());
        }
        ok
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut output = Vec::new();
    for id in selected {
        if !visit(id, selected, map, &mut visiting, &mut visited, &mut output) {
            return Err(EvidenceAcquisitionCampaignError::InvalidPlan(
                "selected acquisition portfolio contains a dependency cycle".into(),
            ));
        }
    }
    Ok(output)
}

fn classify_result(result: &EvidenceAcquisitionResult, output: &mut EvidenceAcquisitionCampaign) {
    match result.disposition {
        EvidenceAcquisitionResultDisposition::Completed => {
            output.completed_order.push(result.candidate_id.clone())
        }
        EvidenceAcquisitionResultDisposition::Negative => {
            output.negative_order.push(result.candidate_id.clone())
        }
        EvidenceAcquisitionResultDisposition::Partial => {
            output.partial_order.push(result.candidate_id.clone())
        }
        EvidenceAcquisitionResultDisposition::Unknown => {
            output.unknown_order.push(result.candidate_id.clone())
        }
    }
    output
        .uncertainty
        .extend(result.uncertainty.iter().cloned());
    output
        .negative_evidence
        .extend(result.negative_evidence.iter().cloned());
}

/// Execute the selected acquisition portfolio through a local executor. This is the only
/// effectful seam; callers choose whether the executor is a simulator, a local retrieval service,
/// an assay gateway, or a replication facility.
pub fn execute_glioma_evidence_acquisition_campaign<E: EvidenceAcquisitionExecutor>(
    request: &EvidenceAcquisitionCampaignRequest,
    executor: &mut E,
) -> Result<EvidenceAcquisitionCampaign, EvidenceAcquisitionCampaignError> {
    let map = validate_request(request)?;
    let selected = request
        .plan
        .selected_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let order = topological_order(&selected, &map)?;
    let mut output = EvidenceAcquisitionCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: request.plan.digest.clone(),
        candidate_order: request.plan.candidate_order.clone(),
        execution_order: Vec::new(),
        results: Vec::new(),
        completed_order: Vec::new(),
        negative_order: Vec::new(),
        partial_order: Vec::new(),
        unknown_order: Vec::new(),
        failed_order: Vec::new(),
        blocked_order: Vec::new(),
        deferred_order: request
            .plan
            .candidate_order
            .iter()
            .filter(|id| !selected.contains(*id))
            .cloned()
            .collect(),
        retry_count: 0,
        budget_spent_units: 0,
        remaining_budget_units: request.budget_units,
        uncertainty: Vec::new(),
        negative_evidence: request.plan.negative_evidence.clone(),
        simulation_only: false,
        disposition: EvidenceAcquisitionCampaignDisposition::Unresolved,
        stop_reason: EvidenceAcquisitionCampaignStopReason::EmptySelection,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-acquisition-campaign"),
    };
    if order.is_empty() {
        output.uncertainty.push("no-acquisition-selected".into());
        output.disposition = if request.plan.disposition == EvidenceAcquisitionDisposition::Blocked
        {
            EvidenceAcquisitionCampaignDisposition::Blocked
        } else {
            EvidenceAcquisitionCampaignDisposition::Unresolved
        };
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| EvidenceAcquisitionCampaignError::Digest(error.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    let mut terminal = BTreeSet::new();
    let mut stop = false;
    for id in &order {
        if stop {
            output.blocked_order.push(id.clone());
            continue;
        }
        let candidate = &map[id];
        if candidate
            .depends_on
            .iter()
            .any(|dependency| terminal.contains(dependency))
        {
            output.blocked_order.push(id.clone());
            output.uncertainty.push(format!("{id}:dependency-terminal"));
            continue;
        }
        if candidate.cost_units > output.remaining_budget_units {
            output.blocked_order.push(id.clone());
            output.uncertainty.push(format!("{id}:budget-exhausted"));
            stop = true;
            output.stop_reason = EvidenceAcquisitionCampaignStopReason::BudgetExhausted;
            continue;
        }
        let mut attempt = 0_u8;
        let mut failed = false;
        let mut result = loop {
            attempt = attempt.saturating_add(1);
            match executor.acquire(candidate, attempt) {
                Ok(result) => {
                    if result.candidate_id != *id
                        || result.attempt_count != attempt
                        || (request.require_artifacts && result.artifact.is_none())
                    {
                        return Err(EvidenceAcquisitionCampaignError::Execution(format!(
                            "adapter returned an invalid result envelope for {id}"
                        )));
                    }
                    if let Some(artifact) = &result.artifact {
                        artifact.validate().map_err(|error| {
                            EvidenceAcquisitionCampaignError::Execution(format!(
                                "adapter artifact for {id} is invalid: {error}"
                            ))
                        })?;
                        if !artifact.local_only || artifact.contains_human_data {
                            return Err(EvidenceAcquisitionCampaignError::Execution(format!(
                                "adapter artifact for {id} violates local preclinical policy"
                            )));
                        }
                    }
                    break result;
                }
                Err(failure) if failure.retryable && attempt <= request.max_retries => {
                    output.retry_count = output.retry_count.saturating_add(1);
                }
                Err(failure) => {
                    output.failed_order.push(id.clone());
                    output
                        .negative_evidence
                        .push(format!("{id}:{}", failure.reason));
                    terminal.insert(id.clone());
                    output.stop_reason = EvidenceAcquisitionCampaignStopReason::ExecutorFailed;
                    stop = true;
                    failed = true;
                    break EvidenceAcquisitionResult {
                        candidate_id: id.clone(),
                        disposition: EvidenceAcquisitionResultDisposition::Unknown,
                        attempt_count: attempt,
                        artifact: None,
                        note: format!("acquisition failed: {}", failure.reason),
                        uncertainty: vec!["executor-failure".into()],
                        negative_evidence: vec![failure.reason],
                    };
                }
            }
        };
        output.execution_order.push(id.clone());
        output.budget_spent_units = output
            .budget_spent_units
            .saturating_add(candidate.cost_units);
        output.remaining_budget_units = output
            .remaining_budget_units
            .saturating_sub(candidate.cost_units);
        if result.artifact.is_none() && request.require_artifacts {
            return Err(EvidenceAcquisitionCampaignError::Execution(format!(
                "acquisition {id} returned no required artifact"
            )));
        }
        result.uncertainty.sort();
        result.uncertainty.dedup();
        result.negative_evidence.sort();
        result.negative_evidence.dedup();
        if !failed {
            classify_result(&result, &mut output);
        } else {
            output
                .uncertainty
                .extend(result.uncertainty.iter().cloned());
            output
                .negative_evidence
                .extend(result.negative_evidence.iter().cloned());
        }
        output.results.push(result.clone());
        if output.failed_order.binary_search(id).is_ok() {
            continue;
        }
        if matches!(
            result.disposition,
            EvidenceAcquisitionResultDisposition::Negative
        ) {
            terminal.insert(id.clone());
            if request.stop_on_negative {
                output.stop_reason = EvidenceAcquisitionCampaignStopReason::NegativeEvidence;
                stop = true;
            }
        }
    }
    output.blocked_order.sort();
    output.completed_order.sort();
    output.negative_order.sort();
    output.partial_order.sort();
    output.unknown_order.sort();
    output.failed_order.sort();
    output.uncertainty.sort();
    output.uncertainty.dedup();
    output.negative_evidence.sort();
    output.negative_evidence.dedup();
    if output.stop_reason == EvidenceAcquisitionCampaignStopReason::EmptySelection {
        output.stop_reason = if output.blocked_order.is_empty() {
            EvidenceAcquisitionCampaignStopReason::Completed
        } else {
            EvidenceAcquisitionCampaignStopReason::DependencyBlocked
        };
    }
    output.disposition = if !output.failed_order.is_empty() {
        EvidenceAcquisitionCampaignDisposition::Failed
    } else if !output.negative_order.is_empty() && request.stop_on_negative {
        EvidenceAcquisitionCampaignDisposition::Negative
    } else if !output.blocked_order.is_empty()
        || !output.unknown_order.is_empty()
        || !output.partial_order.is_empty()
    {
        EvidenceAcquisitionCampaignDisposition::Partial
    } else {
        EvidenceAcquisitionCampaignDisposition::Completed
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceAcquisitionCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p01_evidence_surveillance::acquisition::{
        plan_glioma_evidence_acquisition, EvidenceAcquisitionRequest,
        EvidenceAcquisitionSourceKind, EvidenceAcquisitionWeights,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn candidate(
        id: &str,
        family: &str,
        modality: GliomaModality,
        depends_on: Vec<String>,
    ) -> EvidenceAcquisitionCandidate {
        EvidenceAcquisitionCandidate {
            candidate_id: id.into(),
            target_claim: "EGFR signaling and invasion".into(),
            source_family: family.into(),
            source_kind: EvidenceAcquisitionSourceKind::Literature,
            modality,
            model_system: Some(GliomaModelSystem::Organoid),
            independence_group: family.into(),
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

    fn request(candidates: &[EvidenceAcquisitionCandidate]) -> EvidenceAcquisitionCampaignRequest {
        let acquisition_request = EvidenceAcquisitionRequest {
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
        };
        let plan = plan_glioma_evidence_acquisition(&acquisition_request, candidates).unwrap();
        EvidenceAcquisitionCampaignRequest {
            objective: acquisition_request.objective,
            plan,
            candidates: candidates.to_vec(),
            budget_units: 6,
            max_retries: 2,
            stop_on_negative: true,
            require_artifacts: true,
        }
    }

    #[test]
    fn campaign_executes_dependency_order_and_keeps_dry_run_unknown() {
        let candidates = vec![
            candidate(
                "literature-a",
                "pubmed",
                GliomaModality::Genomics,
                Vec::new(),
            ),
            candidate(
                "dataset-b",
                "atlas",
                GliomaModality::Imaging,
                vec!["literature-a".into()],
            ),
            candidate(
                "replicate-c",
                "consortium",
                GliomaModality::Imaging,
                Vec::new(),
            ),
        ];
        let mut executor = DryRunEvidenceAcquisitionExecutor;
        let output =
            execute_glioma_evidence_acquisition_campaign(&request(&candidates), &mut executor)
                .unwrap();
        assert_eq!(
            output.execution_order,
            vec!["literature-a", "dataset-b", "replicate-c"]
        );
        assert_eq!(output.unknown_order.len(), 3);
        assert_eq!(
            output.disposition,
            EvidenceAcquisitionCampaignDisposition::Partial
        );
        output.validate().unwrap();
    }

    #[test]
    fn campaign_stops_on_negative_and_blocks_remaining_work() {
        struct NegativeExecutor;
        impl EvidenceAcquisitionExecutor for NegativeExecutor {
            fn acquire(
                &mut self,
                candidate: &EvidenceAcquisitionCandidate,
                attempt: u8,
            ) -> Result<EvidenceAcquisitionResult, EvidenceAcquisitionExecutionFailure>
            {
                Ok(EvidenceAcquisitionResult {
                    candidate_id: candidate.candidate_id.clone(),
                    disposition: EvidenceAcquisitionResultDisposition::Negative,
                    attempt_count: attempt,
                    artifact: None,
                    note: "negative preclinical result".into(),
                    uncertainty: vec!["negative-result-requires-revalidation".into()],
                    negative_evidence: vec!["null-effect".into()],
                })
            }
        }
        let candidates = vec![
            candidate(
                "literature-a",
                "pubmed",
                GliomaModality::Genomics,
                Vec::new(),
            ),
            candidate(
                "dataset-b",
                "atlas",
                GliomaModality::Imaging,
                vec!["literature-a".into()],
            ),
            candidate(
                "replicate-c",
                "consortium",
                GliomaModality::Imaging,
                Vec::new(),
            ),
        ];
        let mut executor = NegativeExecutor;
        let mut request = request(&candidates);
        request.require_artifacts = false;
        let output = execute_glioma_evidence_acquisition_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            EvidenceAcquisitionCampaignDisposition::Negative
        );
        assert_eq!(
            output.stop_reason,
            EvidenceAcquisitionCampaignStopReason::NegativeEvidence
        );
        assert!(!output.blocked_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn campaign_retries_retryable_failure_without_duplicate_dispatch() {
        struct RetryExecutor {
            calls: u8,
        }
        impl EvidenceAcquisitionExecutor for RetryExecutor {
            fn acquire(
                &mut self,
                candidate: &EvidenceAcquisitionCandidate,
                attempt: u8,
            ) -> Result<EvidenceAcquisitionResult, EvidenceAcquisitionExecutionFailure>
            {
                self.calls = self.calls.saturating_add(1);
                if self.calls == 1 {
                    return Err(EvidenceAcquisitionExecutionFailure {
                        reason: "temporary adapter outage".into(),
                        retryable: true,
                    });
                }
                Ok(EvidenceAcquisitionResult {
                    candidate_id: candidate.candidate_id.clone(),
                    disposition: EvidenceAcquisitionResultDisposition::Completed,
                    attempt_count: attempt,
                    artifact: None,
                    note: "adapter completed".into(),
                    uncertainty: Vec::new(),
                    negative_evidence: Vec::new(),
                })
            }
        }
        let candidates = vec![candidate(
            "literature-a",
            "pubmed",
            GliomaModality::Genomics,
            Vec::new(),
        )];
        let mut executor = RetryExecutor { calls: 0 };
        let mut request = request(&candidates);
        request.require_artifacts = false;
        let output = execute_glioma_evidence_acquisition_campaign(&request, &mut executor).unwrap();
        assert_eq!(output.retry_count, 1);
        assert_eq!(output.completed_order, vec!["literature-a"]);
        output.validate().unwrap();
    }
}
