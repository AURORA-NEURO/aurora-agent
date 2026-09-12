//! Evidence-to-action portfolio selection for the autonomous glioma workflow.
//!
//! The decision-context compiler names the work a claim needs.  This module makes that work
//! operational: it validates the compiler output, applies the same dependency-aware portfolio
//! selector used by the campaign executor, and returns a typed next batch that can be handed to a
//! local worker.  It does not execute an assay, invent evidence, or turn an unresolved claim into
//! a scientific conclusion.

use super::context_compiler::{DecisionContext, DecisionContextDisposition};
use crate::glioma_engine::{select_glioma_actions, GliomaActionSelection, GliomaSelectionConfig};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionActionPlan1@1";

/// A caller-owned selection policy is applied to already-compiled decision actions.  Keeping the
/// policy separate from the context digest lets a lab change its budget without rewriting the
/// scientific context or its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionActionPlanRequest {
    pub objective: String,
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionActionPlanDisposition {
    Qualified,
    Partial,
    NoRunnableActions,
    Unresolved,
}

/// The result is deliberately consumable by `glioma_action_portfolio_execute` or
/// `glioma_autonomous_campaign_execute`; the bridge itself remains planning-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionActionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub context_disposition: DecisionContextDisposition,
    pub action_order: Vec<String>,
    pub context_deferred_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub selection: Option<GliomaActionSelection>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionActionPlanDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionActionPlanError {
    #[error("decision-action plan request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-action context is invalid: {0}")]
    InvalidContext(String),
    #[error("decision-action selection failed: {0}")]
    Selection(String),
    #[error("decision-action output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-action digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DecisionActionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "context_disposition": output.context_disposition,
        "action_order": output.action_order,
        "context_deferred_order": output.context_deferred_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "completed_order": output.completed_order,
        "selection": output.selection,
        "omission_order": output.omission_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl DecisionActionPlan {
    pub fn validate(&self) -> Result<(), DecisionActionPlanError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.action_order)
            || !canonical(&self.context_deferred_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.completed_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.next_step.trim().is_empty()
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .chain(self.blocked_order.iter())
                .any(|id| self.action_order.binary_search(id).is_err())
        {
            return Err(DecisionActionPlanError::InvalidOutput(
                "identity, canonical ordering, action partition, or next-step contract is invalid"
                    .into(),
            ));
        }
        let selected = self.selected_order.iter().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().collect::<BTreeSet<_>>();
        if self
            .context_deferred_order
            .iter()
            .any(|id| self.action_order.binary_search(id).is_ok())
        {
            return Err(DecisionActionPlanError::InvalidOutput(
                "context-deferred actions must not also appear in runnable action order".into(),
            ));
        }
        if selected.len() != self.selected_order.len()
            || deferred.len() != self.deferred_order.len()
            || blocked.len() != self.blocked_order.len()
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&blocked).next().is_some()
            || deferred.intersection(&blocked).next().is_some()
            || self.selection.as_ref().is_some_and(|selection| {
                selection.selected_order != self.selected_order
                    || selection.deferred_order != self.deferred_order
                    || selection.blocked_order != self.blocked_order
            })
        {
            return Err(DecisionActionPlanError::InvalidOutput(
                "action partitions or nested selection do not reconcile".into(),
            ));
        }
        if let Some(selection) = &self.selection {
            selection
                .validate()
                .map_err(|error| DecisionActionPlanError::InvalidOutput(error.to_string()))?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionActionPlanError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionActionPlanError::InvalidOutput(
                "digest is not bound to decision-action planning output".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &DecisionActionPlanRequest) -> Result<(), DecisionActionPlanError> {
    if request.objective.trim().is_empty()
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(DecisionActionPlanError::InvalidRequest(
            "objective and a canonical completed-action list are required".into(),
        ));
    }
    Ok(())
}

/// Select a deterministic next action portfolio from a compiled decision context.
pub fn plan_decision_actions(
    request: &DecisionActionPlanRequest,
    context: &DecisionContext,
) -> Result<DecisionActionPlan, DecisionActionPlanError> {
    validate_request(request)?;
    context
        .validate()
        .map_err(|error| DecisionActionPlanError::InvalidContext(error.to_string()))?;
    if request.objective.trim() != context.objective.trim() {
        return Err(DecisionActionPlanError::InvalidRequest(
            "decision-action objective must match the compiled context objective".into(),
        ));
    }
    let action_order = context.action_order.clone();
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if completed
        .iter()
        .any(|id| action_order.binary_search(id).is_err())
    {
        return Err(DecisionActionPlanError::InvalidRequest(
            "completed-action list contains an id outside the compiled context".into(),
        ));
    }
    let candidates = context
        .actions
        .iter()
        .map(|action| action.candidate.clone())
        .collect::<Vec<_>>();
    let selection = if candidates.is_empty() {
        None
    } else {
        Some(
            select_glioma_actions(&candidates, &completed, &request.selection)
                .map_err(|error| DecisionActionPlanError::Selection(error.to_string()))?,
        )
    };
    let (selected_order, deferred_order, blocked_order, disposition, next_step) = if let Some(
        selection,
    ) = &selection
    {
        let disposition = if selection.selected_order.is_empty() {
            DecisionActionPlanDisposition::NoRunnableActions
        } else if context.disposition == DecisionContextDisposition::Qualified
            && context.deferred_action_order.is_empty()
            && selection.blocked_order.is_empty()
            && selection.deferred_order.is_empty()
        {
            DecisionActionPlanDisposition::Qualified
        } else {
            DecisionActionPlanDisposition::Partial
        };
        let next_step = if selection.selected_order.is_empty() {
            "hold: no action satisfies the current dependency, budget, approval, or boundary gates"
        } else {
            "dispatch selected action ids to an approved local executor; keep this plan as the immutable batch input"
        };
        (
            selection.selected_order.clone(),
            selection.deferred_order.clone(),
            selection.blocked_order.clone(),
            disposition,
            next_step.to_string(),
        )
    } else {
        (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                if context.disposition == DecisionContextDisposition::Unresolved {
                    DecisionActionPlanDisposition::Unresolved
                } else {
                    DecisionActionPlanDisposition::NoRunnableActions
                },
                "hold: the compiled context contains no runnable action; resolve omissions before dispatch"
                    .into(),
            )
    };
    let mut omission_order = context.omission_order.clone();
    if !context.deferred_action_order.is_empty() {
        omission_order.push("context-action-cap-deferred-actions".into());
    }
    if !deferred_order.is_empty() {
        omission_order.push("decision-action-selection-deferred-by-budget-or-dependency".into());
    }
    omission_order.sort();
    omission_order.dedup();
    let mut output = DecisionActionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_digest: context.digest.clone(),
        context_disposition: context.disposition,
        action_order,
        context_deferred_order: context.deferred_action_order.clone(),
        selected_order,
        deferred_order,
        blocked_order,
        completed_order: request.completed_action_order.clone(),
        selection,
        omission_order,
        negative_evidence_order: context.negative_evidence_order.clone(),
        uncertainty_order: context.uncertainty_order.clone(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-action-plan"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionActionPlanError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceState;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::{
        compile_decision_context, DecisionContextRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

    fn knowledge() -> crate::glioma::programs::p02_evidence_knowledge::TypedKnowledge {
        let artifact = LocalArtifactRef {
            artifact_id: "artifact:context-bridge".into(),
            content_hash: ContentHash::of_bytes(b"context-bridge"),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "prioritize invasion assays".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 4,
            },
            &[EvidenceRecord {
                evidence_id: "evidence:context-bridge".into(),
                source_artifact: artifact,
                source_kind: EvidenceSourceKind::Dataset,
                claim: "invasion pathway is active".into(),
                scope: "preclinical glioma".into(),
                modality: GliomaModality::Genomics,
                model_system: Some(GliomaModelSystem::Organoid),
                state: EvidenceState::Supported,
                relevance_milli: 900,
                quality_milli: 900,
                reproducibility_milli: 900,
                release_epoch: 1,
            }],
        )
        .unwrap()
    }

    fn context() -> DecisionContext {
        compile_decision_context(
            &DecisionContextRequest {
                objective: "prioritize invasion assays".into(),
                max_actions: 4,
                default_cost_units: 5,
            },
            &knowledge(),
        )
        .unwrap()
    }

    fn request() -> DecisionActionPlanRequest {
        DecisionActionPlanRequest {
            objective: "prioritize invasion assays".into(),
            completed_action_order: Vec::new(),
            selection: GliomaSelectionConfig {
                budget_units: 10,
                max_actions: 1,
                approval_granted: true,
                allow_instrument_execution: false,
                allow_federation: false,
                weights: Default::default(),
            },
        }
    }

    #[test]
    fn compiled_context_becomes_dispatchable_next_batch() {
        let output = plan_decision_actions(&request(), &context()).unwrap();
        assert_eq!(output.selected_order.len(), 1);
        assert_eq!(output.disposition, DecisionActionPlanDisposition::Qualified);
        assert!(output.next_step.contains("dispatch selected"));
        output.validate().unwrap();
    }

    #[test]
    fn completed_context_action_is_not_selected_again() {
        let compiled = context();
        let action_id = compiled.action_order[0].clone();
        let mut request = request();
        request.completed_action_order = vec![action_id.clone()];
        let output = plan_decision_actions(&request, &compiled).unwrap();
        assert!(output.selected_order.is_empty());
        assert!(output.blocked_order.contains(&action_id));
        assert_eq!(
            output.disposition,
            DecisionActionPlanDisposition::NoRunnableActions
        );
    }

    #[test]
    fn tampered_context_is_rejected_before_selection() {
        let mut compiled = context();
        compiled.objective = "tampered".into();
        let error = plan_decision_actions(&request(), &compiled).unwrap_err();
        assert!(error.to_string().contains("context"));
    }
}
