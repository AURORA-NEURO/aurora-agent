//! Execute a selected, dependency-closed knowledge-action batch.
//!
//! Selection is useful only when the chosen research work can cross a typed boundary into a
//! local evidence-producing adapter.  This module is that boundary: it verifies the immutable
//! plan/selection inputs, executes only the selector's order, retries bounded failures, appends
//! validated local records, and recompiles typed knowledge/frontier state for the next cycle.
//! It never performs network access, moves protected data, or turns a planned action into a
//! scientific conclusion.

use super::action_compiler::{
    CompiledActionDisposition, CompiledResearchAction, KnowledgeActionPlan,
};
use super::claim_frontier::{
    FrontierActionKind, KnowledgeFrontier, KnowledgeFrontierRequest, KnowledgeFrontierScore,
};
use super::knowledge_graph::{
    compile_typed_knowledge, KnowledgeClaim, KnowledgeRequest, TypedKnowledge,
};
use super::selection_cycle::KnowledgeActionSelectionCycle;
use crate::glioma::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeActionDispatch1@1";
pub const MAX_ACTIONS: usize = 256;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionDispatchRequest {
    pub objective: String,
    pub plan_digest: ContentHash,
    pub selection_digest: ContentHash,
    pub knowledge: KnowledgeRequest,
    pub frontier: KnowledgeFrontierRequest,
    pub records: Vec<EvidenceRecord>,
    pub budget_units: u64,
    pub max_retries: u8,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeActionExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local literature, assay, organoid, imaging, or computation adapters implement
/// this seam. The score and claim are supplied by the validated plan, so an adapter cannot
/// silently execute a different claim than the selector admitted.
pub trait KnowledgeActionExecutor {
    fn execute_action(
        &mut self,
        action: &CompiledResearchAction,
        frontier: &KnowledgeFrontierScore,
        claim: &KnowledgeClaim,
        attempt: u8,
    ) -> Result<Vec<EvidenceRecord>, KnowledgeActionExecutionFailure>;

    fn simulation_only(&self) -> bool {
        false
    }
}

/// A deterministic local adapter for MCP and replay tests. It returns synthetic evidence with
/// an outcome that follows the frontier action kind, preserving negative and unresolved states.
#[derive(Debug, Default)]
pub struct DryRunKnowledgeActionExecutor;

impl KnowledgeActionExecutor for DryRunKnowledgeActionExecutor {
    fn execute_action(
        &mut self,
        action: &CompiledResearchAction,
        frontier: &KnowledgeFrontierScore,
        claim: &KnowledgeClaim,
        attempt: u8,
    ) -> Result<Vec<EvidenceRecord>, KnowledgeActionExecutionFailure> {
        let (source_kind, state) = match frontier.action_kind {
            FrontierActionKind::CloseCoverage => (
                crate::glioma::evidence::EvidenceSourceKind::Dataset,
                EvidenceState::Supported,
            ),
            FrontierActionKind::ResolveContradiction => (
                crate::glioma::evidence::EvidenceSourceKind::Replication,
                EvidenceState::Contradicted,
            ),
            FrontierActionKind::ResolveUncertainty => (
                crate::glioma::evidence::EvidenceSourceKind::Literature,
                EvidenceState::Unknown,
            ),
            FrontierActionKind::RevalidateNegative => (
                crate::glioma::evidence::EvidenceSourceKind::Replication,
                EvidenceState::Negative,
            ),
            FrontierActionKind::ValidateSupported => (
                crate::glioma::evidence::EvidenceSourceKind::Computation,
                EvidenceState::Supported,
            ),
        };
        let evidence_id = format!("dry-run-knowledge-action:{}", action.action_id);
        let content_hash = ContentHash::of_value(&json!({
            "action_id": action.action_id,
            "claim_id": claim.claim_id,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| KnowledgeActionExecutionFailure {
            reason: format!("dry-run evidence digest failed: {error}"),
            retryable: false,
        })?;
        let modality = action
            .modality_order
            .first()
            .copied()
            .or_else(|| claim.modality_order.first().copied())
            .unwrap_or(GliomaModality::Genomics);
        let model_system = action
            .model_system_order
            .first()
            .copied()
            .or_else(|| claim.model_system_order.first().copied())
            .or(Some(GliomaModelSystem::Organoid));
        Ok(vec![EvidenceRecord {
            evidence_id,
            source_artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-artifact:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.knowledge-action+json".into(),
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

    fn simulation_only(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeActionDispatchDisposition {
    Completed,
    Partial,
    Negative,
    BudgetBlocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeActionResultDisposition {
    Completed,
    Negative,
    Uncertain,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionDispatchResult {
    pub action_id: String,
    pub claim_id: String,
    pub attempts: u8,
    pub cost_units: u64,
    pub evidence_order: Vec<String>,
    pub disposition: KnowledgeActionResultDisposition,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionDispatchRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub selection_digest: ContentHash,
    pub selected_source_order: Vec<String>,
    pub executed_source_order: Vec<String>,
    pub blocked_source_order: Vec<String>,
    pub results: Vec<KnowledgeActionDispatchResult>,
    pub records: Vec<EvidenceRecord>,
    pub final_knowledge: TypedKnowledge,
    pub final_frontier: KnowledgeFrontier,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub simulation_only: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: KnowledgeActionDispatchDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeActionDispatchError {
    #[error("knowledge action dispatch request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge action dispatch input is invalid: {0}")]
    InvalidInput(String),
    #[error("knowledge action dispatch execution failed: {0}")]
    Execution(String),
    #[error("knowledge action dispatch output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge action dispatch digest failed: {0}")]
    Digest(String),
}

fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &KnowledgeActionDispatchRun) -> serde_json::Value {
    json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "selection_digest": output.selection_digest,
        "selected_source_order": output.selected_source_order,
        "executed_source_order": output.executed_source_order,
        "blocked_source_order": output.blocked_source_order,
        "results": output.results,
        "records": output.records,
        "final_knowledge": output.final_knowledge,
        "final_frontier": output.final_frontier,
        "budget_spent_units": output.budget_spent_units,
        "remaining_budget_units": output.remaining_budget_units,
        "simulation_only": output.simulation_only,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

fn cost_units(cost_milli: u64) -> u64 {
    cost_milli
        .saturating_add(999)
        .checked_div(1_000)
        .unwrap_or(u64::MAX)
        .max(1)
}

fn validate_records(records: &[EvidenceRecord]) -> Result<(), KnowledgeActionDispatchError> {
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| KnowledgeActionDispatchError::Execution(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(KnowledgeActionDispatchError::Execution(
                "executor returned invalid or duplicate evidence records".into(),
            ));
        }
    }
    Ok(())
}

fn merge_records(records: &mut Vec<EvidenceRecord>, additions: Vec<EvidenceRecord>) {
    for addition in additions {
        if let Some(existing) = records
            .iter_mut()
            .find(|record| record.evidence_id == addition.evidence_id)
        {
            *existing = addition;
        } else {
            records.push(addition);
        }
    }
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
}

fn result_disposition(records: &[EvidenceRecord]) -> KnowledgeActionResultDisposition {
    if records
        .iter()
        .any(|record| record.state == EvidenceState::Negative)
    {
        KnowledgeActionResultDisposition::Negative
    } else if records.iter().any(|record| {
        matches!(
            record.state,
            EvidenceState::Unknown | EvidenceState::Contradicted
        )
    }) {
        KnowledgeActionResultDisposition::Uncertain
    } else {
        KnowledgeActionResultDisposition::Completed
    }
}

impl KnowledgeActionDispatchRun {
    pub fn validate(&self) -> Result<(), KnowledgeActionDispatchError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !unique(&self.selected_source_order)
            || !unique(&self.executed_source_order)
            || !unique(&self.blocked_source_order)
            || self.executed_source_order.iter().any(|id| {
                !self
                    .selected_source_order
                    .iter()
                    .any(|selected| selected == id)
            })
            || self.blocked_source_order.iter().any(|id| {
                !self
                    .selected_source_order
                    .iter()
                    .any(|selected| selected == id)
            })
            || self
                .remaining_budget_units
                .saturating_add(self.budget_spent_units)
                == 0
            || self.next_step.trim().is_empty()
        {
            return Err(KnowledgeActionDispatchError::InvalidOutput(
                "identity, action partitions, budget, or next-step contract is invalid".into(),
            ));
        }
        let result_ids = self
            .results
            .iter()
            .map(|result| result.action_id.clone())
            .collect::<BTreeSet<_>>();
        if result_ids
            != self
                .executed_source_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || self.executed_source_order.iter().any(|id| {
                self.blocked_source_order
                    .iter()
                    .any(|blocked| blocked == id)
            })
        {
            return Err(KnowledgeActionDispatchError::InvalidOutput(
                "dispatch result and blocked partitions do not reconcile".into(),
            ));
        }
        self.final_knowledge
            .validate()
            .map_err(|error| KnowledgeActionDispatchError::InvalidOutput(error.to_string()))?;
        self.final_frontier
            .validate()
            .map_err(|error| KnowledgeActionDispatchError::InvalidOutput(error.to_string()))?;
        validate_records(&self.records)?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeActionDispatchError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeActionDispatchError::InvalidOutput(
                "dispatch digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn selected_actions<'a>(
    plan: &'a KnowledgeActionPlan,
    selection: &KnowledgeActionSelectionCycle,
) -> Result<BTreeMap<String, &'a CompiledResearchAction>, KnowledgeActionDispatchError> {
    plan.validate()
        .map_err(|error| KnowledgeActionDispatchError::InvalidInput(error.to_string()))?;
    selection
        .validate()
        .map_err(|error| KnowledgeActionDispatchError::InvalidInput(error.to_string()))?;
    if selection.selected_order.len() > MAX_ACTIONS {
        return Err(KnowledgeActionDispatchError::InvalidInput(
            "selected action batch exceeds the dispatch ceiling".into(),
        ));
    }
    let actions = plan
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeMap::new();
    for source_id in &selection.selected_source_order {
        let action = actions.get(source_id).ok_or_else(|| {
            KnowledgeActionDispatchError::InvalidInput(format!(
                "selected action {source_id} is absent from plan"
            ))
        })?;
        if action.disposition != CompiledActionDisposition::Selected {
            return Err(KnowledgeActionDispatchError::InvalidInput(
                "selection contains an action not admitted by the compiler".into(),
            ));
        }
        selected.insert(source_id.clone(), *action);
    }
    Ok(selected)
}

/// Execute exactly the selected source actions and recompile the local knowledge frontier.
pub fn execute_glioma_knowledge_action_dispatch<E: KnowledgeActionExecutor>(
    request: &KnowledgeActionDispatchRequest,
    plan: &KnowledgeActionPlan,
    selection: &KnowledgeActionSelectionCycle,
    executor: &mut E,
) -> Result<KnowledgeActionDispatchRun, KnowledgeActionDispatchError> {
    if request.objective.trim().is_empty()
        || request.objective != plan.objective
        || request.plan_digest != plan.digest
        || request.selection_digest != selection.digest
        || request.frontier.objective != request.objective
        || request.knowledge.objective != request.objective
        || request.budget_units == 0
        || request.max_retries > MAX_RETRIES
    {
        return Err(KnowledgeActionDispatchError::InvalidRequest(
            "objective, digests, knowledge/frontier objectives, budget, and retries must match"
                .into(),
        ));
    }
    validate_records(&request.records)?;
    let knowledge = compile_typed_knowledge(&request.knowledge, &request.records)
        .map_err(|error| KnowledgeActionDispatchError::InvalidInput(error.to_string()))?;
    let frontier =
        super::claim_frontier::prioritize_knowledge_frontier(&request.frontier, &knowledge)
            .map_err(|error| KnowledgeActionDispatchError::InvalidInput(error.to_string()))?;
    if plan.knowledge_digest != knowledge.digest || plan.frontier_digest != frontier.digest {
        return Err(KnowledgeActionDispatchError::InvalidInput(
            "plan was compiled from different knowledge or frontier inputs".into(),
        ));
    }
    let selected = selected_actions(plan, selection)?;
    let frontier_by_claim = frontier
        .ranking
        .iter()
        .map(|score| (score.claim_id.clone(), score))
        .collect::<BTreeMap<_, _>>();
    let claims = knowledge
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let mut records = request.records.clone();
    let mut results = Vec::new();
    let mut executed = Vec::new();
    let mut blocked = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_remaining = request.budget_units;
    let mut stop_negative = false;

    for source_id in &selection.selected_source_order {
        let action = selected.get(source_id).ok_or_else(|| {
            KnowledgeActionDispatchError::InvalidInput(
                "selected action order is not plan-closed".into(),
            )
        })?;
        let score = frontier_by_claim.get(&action.claim_id).ok_or_else(|| {
            KnowledgeActionDispatchError::InvalidInput(format!(
                "claim {} is absent from frontier",
                action.claim_id
            ))
        })?;
        if score.action_kind != action.action_kind {
            return Err(KnowledgeActionDispatchError::InvalidInput(
                "compiled action kind differs from current frontier".into(),
            ));
        }
        let claim = claims.get(&action.claim_id).ok_or_else(|| {
            KnowledgeActionDispatchError::InvalidInput(format!(
                "claim {} is absent from knowledge",
                action.claim_id
            ))
        })?;
        let cost = cost_units(action.cost_milli);
        if cost > budget_remaining {
            blocked.push(source_id.clone());
            continue;
        }
        if stop_negative {
            blocked.push(source_id.clone());
            continue;
        }
        let mut attempts = 0_u8;
        let mut accepted = None;
        let mut failure = None;
        while attempts <= request.max_retries {
            attempts = attempts.saturating_add(1);
            match executor.execute_action(action, score, claim, attempts) {
                Ok(batch) => {
                    validate_records(&batch)?;
                    if batch.iter().any(|record| {
                        record.claim != claim.statement || record.scope != claim.scope
                    }) {
                        return Err(KnowledgeActionDispatchError::Execution(
                            "executor evidence is not bound to the selected claim scope".into(),
                        ));
                    }
                    accepted = Some(batch);
                    break;
                }
                Err(error) => {
                    failure = Some(error.reason);
                    if !error.retryable || attempts > request.max_retries {
                        break;
                    }
                }
            }
        }
        budget_remaining = budget_remaining.saturating_sub(cost);
        let Some(batch) = accepted else {
            results.push(KnowledgeActionDispatchResult {
                action_id: action.action_id.clone(),
                claim_id: action.claim_id.clone(),
                attempts,
                cost_units: cost,
                evidence_order: Vec::new(),
                disposition: KnowledgeActionResultDisposition::Failed,
                failure,
            });
            executed.push(source_id.clone());
            continue;
        };
        let result_disposition = result_disposition(&batch);
        let evidence_order = batch
            .iter()
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        if result_disposition == KnowledgeActionResultDisposition::Negative {
            stop_negative = request.stop_on_negative;
            negative_evidence.extend(evidence_order.iter().cloned());
        }
        if result_disposition == KnowledgeActionResultDisposition::Uncertain {
            uncertainty.extend(evidence_order.iter().cloned());
        }
        merge_records(&mut records, batch);
        results.push(KnowledgeActionDispatchResult {
            action_id: action.action_id.clone(),
            claim_id: action.claim_id.clone(),
            attempts,
            cost_units: cost,
            evidence_order,
            disposition: result_disposition,
            failure: None,
        });
        executed.push(source_id.clone());
    }

    let final_knowledge = compile_typed_knowledge(&request.knowledge, &records)
        .map_err(|error| KnowledgeActionDispatchError::Execution(error.to_string()))?;
    let final_frontier =
        super::claim_frontier::prioritize_knowledge_frontier(&request.frontier, &final_knowledge)
            .map_err(|error| KnowledgeActionDispatchError::Execution(error.to_string()))?;
    let failed = results
        .iter()
        .any(|result| result.disposition == KnowledgeActionResultDisposition::Failed);
    let has_negative = results
        .iter()
        .any(|result| result.disposition == KnowledgeActionResultDisposition::Negative);
    let disposition = if failed {
        KnowledgeActionDispatchDisposition::Failed
    } else if has_negative {
        KnowledgeActionDispatchDisposition::Negative
    } else if !blocked.is_empty() {
        KnowledgeActionDispatchDisposition::BudgetBlocked
    } else if executed.len() == selection.selected_source_order.len() {
        KnowledgeActionDispatchDisposition::Completed
    } else {
        KnowledgeActionDispatchDisposition::Partial
    };
    let next_step = if has_negative {
        "inspect the negative evidence and replan a falsification or replication action".into()
    } else if !blocked.is_empty() {
        "increase the local budget or resume the blocked dependency-closed actions".into()
    } else if final_frontier.selected_order.is_empty() {
        "review the qualified knowledge state and release its bounded research object".into()
    } else {
        "recompile the action plan from the returned knowledge frontier".into()
    };
    let mut output = KnowledgeActionDispatchRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: request.plan_digest.clone(),
        selection_digest: request.selection_digest.clone(),
        selected_source_order: selection.selected_source_order.clone(),
        executed_source_order: executed,
        blocked_source_order: blocked,
        results,
        records,
        final_knowledge,
        final_frontier,
        budget_spent_units: request.budget_units.saturating_sub(budget_remaining),
        remaining_budget_units: budget_remaining,
        simulation_only: executor.simulation_only(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-knowledge-action-dispatch"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeActionDispatchError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::action_compiler::{
        digest_input as plan_digest_input, KnowledgeActionPlanDisposition,
    };
    use crate::glioma::programs::p02_evidence_knowledge::claim_frontier::KnowledgeFrontierWeights;
    use crate::glioma::programs::p02_evidence_knowledge::selection_cycle::KnowledgeActionSelectionCycleRequest;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, GliomaSelectionConfig};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn fixtures() -> (
        KnowledgeActionPlan,
        KnowledgeActionSelectionCycle,
        KnowledgeActionDispatchRequest,
    ) {
        let records = vec![EvidenceRecord {
            evidence_id: "seed".into(),
            source_artifact: artifact("seed-artifact"),
            source_kind: EvidenceSourceKind::Literature,
            claim: "EGFR signaling increases invasion".into(),
            scope: "organoid invasion".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 800,
            release_epoch: 1,
        }];
        let knowledge_request = KnowledgeRequest {
            objective: "dispatch invasion validation".into(),
            required_modalities: [GliomaModality::Genomics].into_iter().collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            min_support_milli: 100,
            min_sources_per_claim: 1,
            max_claims: 8,
        };
        let knowledge = compile_typed_knowledge(&knowledge_request, &records).unwrap();
        let frontier_request = KnowledgeFrontierRequest {
            objective: knowledge_request.objective.clone(),
            max_selected_claims: 4,
            min_priority_milli: 0,
            weights: KnowledgeFrontierWeights::default(),
        };
        let frontier = super::super::claim_frontier::prioritize_knowledge_frontier(
            &frontier_request,
            &knowledge,
        )
        .unwrap();
        let claim_id = frontier.selected_order[0].clone();
        let score = frontier
            .ranking
            .iter()
            .find(|score| score.claim_id == claim_id)
            .unwrap();
        let action = CompiledResearchAction {
            action_id: "validate-invasion".into(),
            claim_id,
            action_kind: score.action_kind,
            modality_order: vec![GliomaModality::Imaging],
            model_system_order: vec![GliomaModelSystem::Organoid],
            information_gain_milli: 800,
            cost_milli: 1_000,
            risk_milli: 100,
            dependency_order: Vec::new(),
            priority_milli: score.priority_milli,
            disposition: CompiledActionDisposition::Selected,
            reason: "validation admitted".into(),
        };
        let mut plan = KnowledgeActionPlan {
            feature_id: "GAF-GLIOMA-P02-F16".into(),
            output_schema: "GliomaKnowledgeActionCompiler1@1".into(),
            objective: knowledge_request.objective.clone(),
            knowledge_digest: knowledge.digest.clone(),
            frontier_digest: frontier.digest.clone(),
            action_order: vec![action.action_id.clone()],
            selected_order: vec![action.action_id.clone()],
            deferred_order: Vec::new(),
            blocked_order: Vec::new(),
            actions: vec![action],
            selected_cost_milli: 1_000,
            selected_risk_milli: 100,
            selected_information_milli: 800,
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: KnowledgeActionPlanDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        plan.digest = ContentHash::of_value(&plan_digest_input(&plan)).unwrap();
        let selection = super::super::selection_cycle::execute_glioma_knowledge_selection_cycle(
            &KnowledgeActionSelectionCycleRequest {
                objective: plan.objective.clone(),
                plan_digest: plan.digest.clone(),
                max_candidates: 4,
                completed_source_action_ids: BTreeSet::new(),
                selection_config: GliomaSelectionConfig::default(),
            },
            &plan,
        )
        .unwrap();
        let request = KnowledgeActionDispatchRequest {
            objective: knowledge_request.objective.clone(),
            plan_digest: plan.digest.clone(),
            selection_digest: selection.digest.clone(),
            knowledge: knowledge_request,
            frontier: frontier_request,
            records,
            budget_units: 1,
            max_retries: 1,
            stop_on_negative: true,
        };
        (plan, selection, request)
    }

    #[test]
    fn dispatch_executes_selected_action_and_recompiles_frontier() {
        let (plan, selection, request) = fixtures();
        let mut executor = DryRunKnowledgeActionExecutor;
        let output =
            execute_glioma_knowledge_action_dispatch(&request, &plan, &selection, &mut executor)
                .unwrap();
        assert_eq!(output.executed_source_order, vec!["validate-invasion"]);
        assert_eq!(output.budget_spent_units, 1);
        assert!(output.records.len() >= 2);
        assert!(output.validate().is_ok());
    }

    #[test]
    fn dispatch_blocks_when_budget_cannot_pay_selected_action() {
        let (plan, selection, mut request) = fixtures();
        request.budget_units = 0;
        let mut executor = DryRunKnowledgeActionExecutor;
        assert!(matches!(
            execute_glioma_knowledge_action_dispatch(&request, &plan, &selection, &mut executor),
            Err(KnowledgeActionDispatchError::InvalidRequest(_))
        ));
    }
}
