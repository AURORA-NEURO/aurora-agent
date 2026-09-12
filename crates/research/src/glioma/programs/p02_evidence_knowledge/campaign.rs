//! Bounded autonomous claim-resolution campaigns for preclinical glioma research.
//!
//! P02 compiles local evidence into typed claims, ranks the unresolved frontier, dispatches
//! caller-owned local actions, and recompiles from returned evidence after every round. The
//! controller never treats a planned action as a result: contradictions, negative results,
//! missing coverage, unknown states, retries, budget exhaustion, and no-progress stops remain
//! first-class output.

use super::claim_frontier::{
    prioritize_knowledge_frontier, FrontierActionKind, KnowledgeFrontier, KnowledgeFrontierRequest,
    KnowledgeFrontierScore,
};
use super::knowledge_graph::{
    compile_typed_knowledge, KnowledgeDisposition, KnowledgeRequest, TypedKnowledge,
};
use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeResolutionCampaign1@1";
pub const MAX_ROUNDS: u16 = 24;
pub const MAX_ACTIONS_PER_ROUND: usize = 8;
pub const MAX_RETRIES: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeResolutionCampaignRequest {
    pub knowledge: KnowledgeRequest,
    pub frontier: KnowledgeFrontierRequest,
    pub records: Vec<EvidenceRecord>,
    pub budget_units: u64,
    pub cost_per_action_units: u32,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeResolutionExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, replication, literature, or computation adapters implement this seam.
/// The campaign supplies a typed frontier score and receives only validated local evidence rows.
pub trait KnowledgeResolutionCampaignExecutor {
    fn resolve_action(
        &mut self,
        action: &KnowledgeFrontierScore,
        knowledge: &TypedKnowledge,
        attempt: u8,
    ) -> Result<Vec<EvidenceRecord>, KnowledgeResolutionExecutionFailure>;
}

/// Deterministic sandbox adapter for MCP and replay tests. It emits synthetic, local-only
/// metadata and deliberately preserves contradiction/negative outcomes rather than erasing them.
#[derive(Debug, Default)]
pub struct DryRunKnowledgeResolutionCampaignExecutor;

impl KnowledgeResolutionCampaignExecutor for DryRunKnowledgeResolutionCampaignExecutor {
    fn resolve_action(
        &mut self,
        action: &KnowledgeFrontierScore,
        knowledge: &TypedKnowledge,
        attempt: u8,
    ) -> Result<Vec<EvidenceRecord>, KnowledgeResolutionExecutionFailure> {
        let claim = knowledge
            .claims
            .iter()
            .find(|claim| claim.claim_id == action.claim_id)
            .ok_or_else(|| KnowledgeResolutionExecutionFailure {
                reason: format!("claim {} is absent from typed knowledge", action.claim_id),
                retryable: false,
            })?;
        let modality = claim
            .modality_order
            .first()
            .copied()
            .unwrap_or(GliomaModality::Genomics);
        let model_system = claim
            .model_system_order
            .first()
            .copied()
            .or(Some(GliomaModelSystem::Organoid));
        let (source_kind, state) = match action.action_kind {
            FrontierActionKind::CloseCoverage => {
                (EvidenceSourceKind::Dataset, EvidenceState::Supported)
            }
            FrontierActionKind::ResolveContradiction => {
                (EvidenceSourceKind::Replication, EvidenceState::Contradicted)
            }
            FrontierActionKind::ResolveUncertainty => {
                (EvidenceSourceKind::Literature, EvidenceState::Unknown)
            }
            FrontierActionKind::RevalidateNegative => {
                (EvidenceSourceKind::Replication, EvidenceState::Negative)
            }
            FrontierActionKind::ValidateSupported => {
                (EvidenceSourceKind::Computation, EvidenceState::Supported)
            }
        };
        let evidence_id = format!("dry-run-knowledge:{}", action.claim_id);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "claim_id": action.claim_id,
            "action_kind": action.action_kind,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| KnowledgeResolutionExecutionFailure {
            reason: format!("dry-run knowledge digest failed: {error}"),
            retryable: false,
        })?;
        Ok(vec![EvidenceRecord {
            evidence_id,
            source_artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-knowledge:{}", action.claim_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.knowledge-resolution+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind,
            claim: claim.statement.clone(),
            scope: claim.scope.clone(),
            modality,
            model_system,
            state,
            relevance_milli: 800,
            quality_milli: 800,
            reproducibility_milli: 800,
            release_epoch: 1,
        }])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeResolutionCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_evidence_order: Vec<String>,
    pub knowledge: TypedKnowledge,
    pub frontier: KnowledgeFrontier,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeResolutionCampaignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    NoActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeResolutionCampaignStopReason {
    Qualified,
    BudgetExhausted,
    NoActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeResolutionCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<KnowledgeResolutionCampaignRound>,
    pub records: Vec<EvidenceRecord>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_evidence_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_knowledge: TypedKnowledge,
    pub final_frontier: KnowledgeFrontier,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: KnowledgeResolutionCampaignDisposition,
    pub stop_reason: KnowledgeResolutionCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeResolutionCampaignError {
    #[error("knowledge-resolution campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge-resolution campaign planning failed: {0}")]
    Planning(String),
    #[error("knowledge-resolution campaign execution failed: {0}")]
    Execution(String),
    #[error("knowledge-resolution campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge-resolution campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &KnowledgeResolutionCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "records": campaign.records,
        "completed_order": campaign.completed_order,
        "failed_order": campaign.failed_order,
        "accepted_evidence_order": campaign.accepted_evidence_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_knowledge": campaign.final_knowledge,
        "final_frontier": campaign.final_frontier,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_records(records: &[EvidenceRecord]) -> Result<(), KnowledgeResolutionCampaignError> {
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| KnowledgeResolutionCampaignError::InvalidRequest(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(KnowledgeResolutionCampaignError::InvalidRequest(
                "evidence identity, claim, scope, scores, artifact, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn validate_request(
    request: &KnowledgeResolutionCampaignRequest,
) -> Result<(), KnowledgeResolutionCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.budget_units == 0
        || request.cost_per_action_units == 0
        || request.frontier.max_selected_claims == 0
        || request.frontier.max_selected_claims > MAX_ACTIONS_PER_ROUND
    {
        return Err(KnowledgeResolutionCampaignError::InvalidRequest(
            "bounded rounds, retries, budget, cost, and frontier actions are required".into(),
        ));
    }
    if request.knowledge.objective.trim() != request.frontier.objective.trim() {
        return Err(KnowledgeResolutionCampaignError::InvalidRequest(
            "knowledge and frontier objectives must match".into(),
        ));
    }
    validate_records(&request.records)?;
    let knowledge = compile_typed_knowledge(&request.knowledge, &request.records)
        .map_err(|error| KnowledgeResolutionCampaignError::InvalidRequest(error.to_string()))?;
    prioritize_knowledge_frontier(&request.frontier, &knowledge)
        .map_err(|error| KnowledgeResolutionCampaignError::InvalidRequest(error.to_string()))?;
    Ok(())
}

fn replace_records(records: &mut Vec<EvidenceRecord>, replacements: Vec<EvidenceRecord>) {
    for replacement in replacements {
        if let Some(existing) = records
            .iter_mut()
            .find(|record| record.evidence_id == replacement.evidence_id)
        {
            *existing = replacement;
        } else {
            records.push(replacement);
        }
    }
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
}

fn validate_returned_records(
    records: &[EvidenceRecord],
    action: &KnowledgeFrontierScore,
) -> Result<(), KnowledgeResolutionCampaignError> {
    if records.is_empty() {
        return Err(KnowledgeResolutionCampaignError::Execution(format!(
            "executor returned no evidence for claim {}",
            action.claim_id
        )));
    }
    validate_records(records).map_err(|error| {
        KnowledgeResolutionCampaignError::Execution(format!(
            "executor returned invalid evidence for claim {}: {error}",
            action.claim_id
        ))
    })
}

impl KnowledgeResolutionCampaign {
    pub fn validate(&self) -> Result<(), KnowledgeResolutionCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.completed_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.accepted_evidence_order)
            || self
                .completed_order
                .iter()
                .any(|id| self.failed_order.binary_search(id).is_ok())
        {
            return Err(KnowledgeResolutionCampaignError::InvalidOutput(
                "identity, canonical partitions, or campaign fields are invalid".into(),
            ));
        }
        validate_records(&self.records)
            .map_err(|error| KnowledgeResolutionCampaignError::InvalidOutput(error.to_string()))?;
        self.final_knowledge
            .validate()
            .map_err(|error| KnowledgeResolutionCampaignError::InvalidOutput(error.to_string()))?;
        self.final_frontier
            .validate()
            .map_err(|error| KnowledgeResolutionCampaignError::InvalidOutput(error.to_string()))?;
        if self.final_knowledge.objective != self.objective
            || self.final_frontier.objective != self.objective
            || self.final_frontier.knowledge_digest != self.final_knowledge.digest
        {
            return Err(KnowledgeResolutionCampaignError::InvalidOutput(
                "final knowledge/frontier objectives or digest do not reconcile".into(),
            ));
        }
        let mut rounds = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !rounds.insert(round.round)
                || !canonical(&round.action_order)
                || !canonical(&round.completed_order)
                || !canonical(&round.failed_order)
                || !canonical(&round.accepted_evidence_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(KnowledgeResolutionCampaignError::InvalidOutput(
                    "round ordering or budget invariants are invalid".into(),
                ));
            }
            round.knowledge.validate().map_err(|error| {
                KnowledgeResolutionCampaignError::InvalidOutput(error.to_string())
            })?;
            round.frontier.validate().map_err(|error| {
                KnowledgeResolutionCampaignError::InvalidOutput(error.to_string())
            })?;
            spent = spent.saturating_add(u64::from(round.cost_units));
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(KnowledgeResolutionCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeResolutionCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeResolutionCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute bounded claim-resolution rounds, recompiling typed knowledge from every accepted
/// local evidence response before selecting the next frontier actions.
pub fn execute_glioma_knowledge_resolution_campaign<E: KnowledgeResolutionCampaignExecutor>(
    request: &KnowledgeResolutionCampaignRequest,
    executor: &mut E,
) -> Result<KnowledgeResolutionCampaign, KnowledgeResolutionCampaignError> {
    validate_request(request)?;
    let mut records = request.records.clone();
    let mut rounds = Vec::new();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut accepted_evidence = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_spent = 0_u64;
    let mut retry_count = 0_u32;
    let mut stop_reason = KnowledgeResolutionCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let knowledge = compile_typed_knowledge(&request.knowledge, &records)
            .map_err(|error| KnowledgeResolutionCampaignError::Planning(error.to_string()))?;
        let frontier = prioritize_knowledge_frontier(&request.frontier, &knowledge)
            .map_err(|error| KnowledgeResolutionCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(knowledge.negative_evidence_order.iter().cloned());
        uncertainty.extend(knowledge.uncertainty_order.iter().cloned());
        let eligible = frontier
            .selected_order
            .iter()
            .filter(|claim_id| !completed.contains(*claim_id) && !failed.contains(*claim_id))
            .cloned()
            .collect::<Vec<_>>();
        if request.stop_on_qualified
            && knowledge.disposition == KnowledgeDisposition::Qualified
            && eligible.is_empty()
        {
            stop_reason = KnowledgeResolutionCampaignStopReason::Qualified;
            break;
        }
        let remaining = request.budget_units.saturating_sub(budget_spent);
        if remaining < u64::from(request.cost_per_action_units) {
            stop_reason = KnowledgeResolutionCampaignStopReason::BudgetExhausted;
            break;
        }
        if eligible.is_empty() {
            stop_reason = KnowledgeResolutionCampaignStopReason::NoActions;
            break;
        }
        let max_batch = (remaining / u64::from(request.cost_per_action_units)) as usize;
        let selected = eligible
            .into_iter()
            .take(max_batch.clamp(1, MAX_ACTIONS_PER_ROUND))
            .collect::<Vec<_>>();
        let actions = selected
            .iter()
            .filter_map(|claim_id| {
                frontier
                    .ranking
                    .iter()
                    .find(|score| &score.claim_id == claim_id)
            })
            .collect::<Vec<_>>();
        let before_budget = remaining;
        let mut completed_round = Vec::new();
        let mut failed_round = Vec::new();
        let mut accepted_round = Vec::new();
        let mut retry_round = 0_u32;
        let mut progress = false;
        for action in actions {
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.resolve_action(action, &knowledge, attempt) {
                    Ok(returned) => {
                        validate_returned_records(&returned, action)?;
                        accepted = Some(returned);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(KnowledgeResolutionCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            retry_round = retry_round.saturating_add(1);
                            continue;
                        }
                        failed_round.push(action.claim_id.clone());
                        failed.insert(action.claim_id.clone());
                        break;
                    }
                }
            }
            if let Some(returned) = accepted {
                completed_round.push(action.claim_id.clone());
                completed.insert(action.claim_id.clone());
                for record in &returned {
                    accepted_round.push(record.evidence_id.clone());
                    accepted_evidence.insert(record.evidence_id.clone());
                }
                replace_records(&mut records, returned);
                progress = true;
            } else {
                break;
            }
        }
        completed_round.sort();
        failed_round.sort();
        accepted_round.sort();
        let cost = request
            .cost_per_action_units
            .saturating_mul(selected.len() as u32);
        budget_spent = budget_spent.saturating_add(u64::from(cost));
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let updated_knowledge = compile_typed_knowledge(&request.knowledge, &records)
            .map_err(|error| KnowledgeResolutionCampaignError::Planning(error.to_string()))?;
        let updated_frontier = prioritize_knowledge_frontier(&request.frontier, &updated_knowledge)
            .map_err(|error| KnowledgeResolutionCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(updated_knowledge.negative_evidence_order.iter().cloned());
        uncertainty.extend(updated_knowledge.uncertainty_order.iter().cloned());
        let mut action_order = selected;
        action_order.sort();
        rounds.push(KnowledgeResolutionCampaignRound {
            round: round_number,
            action_order,
            completed_order: completed_round,
            failed_order: failed_round.clone(),
            accepted_evidence_order: accepted_round,
            knowledge: updated_knowledge.clone(),
            frontier: updated_frontier.clone(),
            cost_units: cost,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: retry_round,
        });
        if !failed_round.is_empty() {
            stop_reason = KnowledgeResolutionCampaignStopReason::ExecutorFailed;
            break;
        }
        if !progress {
            stop_reason = KnowledgeResolutionCampaignStopReason::NoProgress;
            break;
        }
        let next_eligible = updated_frontier
            .selected_order
            .iter()
            .any(|claim_id| !completed.contains(claim_id) && !failed.contains(claim_id));
        if request.stop_on_qualified
            && updated_knowledge.disposition == KnowledgeDisposition::Qualified
            && !next_eligible
        {
            stop_reason = KnowledgeResolutionCampaignStopReason::Qualified;
            break;
        }
        if after_budget == 0 {
            stop_reason = KnowledgeResolutionCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let final_knowledge = compile_typed_knowledge(&request.knowledge, &records)
        .map_err(|error| KnowledgeResolutionCampaignError::Planning(error.to_string()))?;
    let final_frontier = prioritize_knowledge_frontier(&request.frontier, &final_knowledge)
        .map_err(|error| KnowledgeResolutionCampaignError::Planning(error.to_string()))?;
    negative_evidence.extend(final_knowledge.negative_evidence_order.iter().cloned());
    uncertainty.extend(final_knowledge.uncertainty_order.iter().cloned());
    let final_eligible = final_frontier
        .selected_order
        .iter()
        .any(|claim_id| !completed.contains(claim_id) && !failed.contains(claim_id));
    if request.stop_on_qualified
        && final_knowledge.disposition == KnowledgeDisposition::Qualified
        && !final_eligible
    {
        stop_reason = KnowledgeResolutionCampaignStopReason::Qualified;
    }
    let disposition = match stop_reason {
        KnowledgeResolutionCampaignStopReason::Qualified => {
            KnowledgeResolutionCampaignDisposition::Qualified
        }
        KnowledgeResolutionCampaignStopReason::BudgetExhausted => {
            KnowledgeResolutionCampaignDisposition::BudgetBlocked
        }
        KnowledgeResolutionCampaignStopReason::ExecutorFailed => {
            KnowledgeResolutionCampaignDisposition::Failed
        }
        KnowledgeResolutionCampaignStopReason::NoActions => {
            KnowledgeResolutionCampaignDisposition::NoActions
        }
        _ if final_knowledge.disposition == KnowledgeDisposition::Unresolved => {
            KnowledgeResolutionCampaignDisposition::Unresolved
        }
        _ => KnowledgeResolutionCampaignDisposition::Partial,
    };
    let mut output = KnowledgeResolutionCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.knowledge.objective.clone(),
        rounds,
        records,
        completed_order: completed.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        accepted_evidence_order: accepted_evidence.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        final_knowledge,
        final_frontier,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: true,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-knowledge-resolution-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeResolutionCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, claim: &str, state: EvidenceState) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: claim.into(),
            scope: "preclinical glioma invasion".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request() -> KnowledgeResolutionCampaignRequest {
        KnowledgeResolutionCampaignRequest {
            knowledge: KnowledgeRequest {
                objective: "resolve preclinical glioma invasion claims".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            frontier: KnowledgeFrontierRequest {
                objective: "resolve preclinical glioma invasion claims".into(),
                max_selected_claims: 2,
                min_priority_milli: 0,
                weights: Default::default(),
            },
            records: vec![record(
                "e1",
                "EGFR signaling increases invasion",
                EvidenceState::Supported,
            )],
            budget_units: 2,
            cost_per_action_units: 1,
            max_rounds: 3,
            max_retries: 1,
            stop_on_qualified: true,
        }
    }

    #[test]
    fn campaign_recompiles_and_replays_deterministically() {
        let request = request();
        let mut first_executor = DryRunKnowledgeResolutionCampaignExecutor;
        let mut second_executor = DryRunKnowledgeResolutionCampaignExecutor;
        let first =
            execute_glioma_knowledge_resolution_campaign(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_knowledge_resolution_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            KnowledgeResolutionCampaignDisposition::Qualified
        );
        assert!(!first.completed_order.is_empty());
        first.validate().unwrap();
    }

    #[test]
    fn contradiction_resolution_preserves_negative_state_and_does_not_fabricate_support() {
        let mut request = request();
        request.records[0].state = EvidenceState::Contradicted;
        request.stop_on_qualified = false;
        let mut executor = DryRunKnowledgeResolutionCampaignExecutor;
        let output = execute_glioma_knowledge_resolution_campaign(&request, &mut executor).unwrap();
        assert!(output
            .final_knowledge
            .uncertainty_order
            .iter()
            .any(|value| value.contains("e1")));
        assert_ne!(
            output.disposition,
            KnowledgeResolutionCampaignDisposition::Qualified
        );
        output.validate().unwrap();
    }

    #[test]
    fn budget_exhaustion_is_explicit() {
        let mut request = request();
        request.records.push(record(
            "e2",
            "EGFR signaling increases invasion",
            EvidenceState::Unknown,
        ));
        request.budget_units = 1;
        request.max_rounds = 4;
        let mut executor = DryRunKnowledgeResolutionCampaignExecutor;
        let output = execute_glioma_knowledge_resolution_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            KnowledgeResolutionCampaignStopReason::BudgetExhausted
        );
        assert_eq!(
            output.disposition,
            KnowledgeResolutionCampaignDisposition::BudgetBlocked
        );
        output.validate().unwrap();
    }
}
