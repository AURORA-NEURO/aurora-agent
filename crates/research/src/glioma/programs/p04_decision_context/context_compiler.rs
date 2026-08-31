//! Evidence-gap to next-action compilation for the autonomous glioma workflow.
//!
//! The compiler is the bridge between scientific state and executable work.  It turns each
//! typed claim into a bounded, stage-aware action candidate: missing modality/model coverage
//! becomes a local harmonization action, contradiction becomes a replication action, a negative
//! result becomes a falsification-oriented experiment action, and a qualified claim becomes a
//! mechanism-validation action.  These are real, typed product candidates that can be passed
//! directly to `select_glioma_actions`; they are not hypotheses or free-form recommendations.

use crate::glioma::programs::p02_evidence_knowledge::{KnowledgeClaimDisposition, TypedKnowledge};
use crate::glioma_engine::{
    GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaStageKind,
};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContext1@1";
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextRequest {
    pub objective: String,
    pub max_actions: usize,
    pub default_cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionActionKind {
    CloseCoverage,
    ResolveContradiction,
    FalsifyNegative,
    ResolveEvidence,
    ValidateMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAction {
    pub action_id: String,
    pub claim_id: String,
    pub kind: DecisionActionKind,
    pub rationale: String,
    pub target_modality: GliomaModality,
    pub target_model_system: GliomaModelSystem,
    pub priority_milli: u16,
    pub candidate: GliomaActionCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContext {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub actions: Vec<DecisionAction>,
    pub action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionContextDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextError {
    #[error("decision-context request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-context input is invalid: {0}")]
    InvalidInput(String),
    #[error("decision-context output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-context digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &DecisionContext) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "actions": output.actions,
        "action_order": output.action_order,
        "deferred_action_order": output.deferred_action_order,
        "omission_order": output.omission_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

fn ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl DecisionContext {
    pub fn validate(&self) -> Result<(), DecisionContextError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !ordered_unique(&self.claim_order)
            || self.actions.len() != self.action_order.len()
            || self
                .actions
                .iter()
                .map(|action| action.action_id.clone())
                .collect::<Vec<_>>()
                != self.action_order
            || !ordered_unique(&self.action_order)
            || !ordered_unique(&self.deferred_action_order)
            || !ordered_unique(&self.omission_order)
            || !ordered_unique(&self.negative_evidence_order)
            || !ordered_unique(&self.uncertainty_order)
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.claim_id.trim().is_empty()
                    || action.rationale.trim().is_empty()
                    || action.priority_milli > 1_000
                    || action.candidate.action_id != action.action_id
                    || action.candidate.cost_units == 0
                    || action.candidate.effects.is_empty()
                    || !action.candidate.depends_on.is_empty()
            })
            || self
                .action_order
                .iter()
                .any(|id| self.deferred_action_order.binary_search(id).is_ok())
        {
            return Err(DecisionContextError::InvalidOutput(
                "identity, action partition, ordering, or candidate contract is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionContextError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextError::InvalidOutput(
                "digest is not bound to decision context".into(),
            ));
        }
        Ok(())
    }
}

fn action_for_claim(
    claim: &crate::glioma::programs::p02_evidence_knowledge::KnowledgeClaim,
    cost_units: u32,
) -> DecisionAction {
    let (kind, stage_kind, modality, rationale, priority) = if !claim
        .missing_modality_order
        .is_empty()
        || !claim.missing_model_system_order.is_empty()
    {
        (
                DecisionActionKind::CloseCoverage,
                GliomaStageKind::MultimodalIngestionQc,
                claim
                    .missing_modality_order
                    .first()
                    .copied()
                    .unwrap_or(GliomaModality::Computational),
                "close the claim's missing supporting modality/model coverage before mechanism execution".into(),
                950,
            )
    } else if !claim.contradictory_evidence_order.is_empty() {
        (
                DecisionActionKind::ResolveContradiction,
                GliomaStageKind::ReplicationRobustness,
                claim
                    .modality_order
                    .first()
                    .copied()
                    .unwrap_or(GliomaModality::Replication),
                "run an independent preclinical replication battery to separate contradiction from batch or assay effects".into(),
                900,
            )
    } else if claim.disposition == KnowledgeClaimDisposition::Negative {
        (
                DecisionActionKind::FalsifyNegative,
                GliomaStageKind::ExperimentDesign,
                claim
                    .modality_order
                    .first()
                    .copied()
                    .unwrap_or(GliomaModality::FunctionalPerturbation),
                "design a falsification-oriented follow-up that preserves the negative result as a first-class outcome".into(),
                850,
            )
    } else if !claim.unresolved_evidence_order.is_empty()
        || claim.disposition == KnowledgeClaimDisposition::Unresolved
    {
        (
            DecisionActionKind::ResolveEvidence,
            GliomaStageKind::EvidenceCompilation,
            GliomaModality::Literature,
            "resolve stale, unknown, or unmeasured evidence before promoting this claim".into(),
            875,
        )
    } else {
        (
            DecisionActionKind::ValidateMechanism,
            GliomaStageKind::MechanismExploration,
            claim
                .modality_order
                .first()
                .copied()
                .unwrap_or(GliomaModality::Computational),
            "validate the supported claim against a discriminating mechanism action".into(),
            600,
        )
    };
    let target_model_system = claim
        .missing_model_system_order
        .first()
        .copied()
        .or_else(|| claim.model_system_order.first().copied())
        .unwrap_or(GliomaModelSystem::InSilico);
    let action_id = format!("decision-{}", claim.claim_id);
    let candidate = GliomaActionCandidate {
        action_id: action_id.clone(),
        stage_kind,
        modality,
        model_system: target_model_system,
        depends_on: Vec::new(),
        cost_units,
        information_gain_milli: priority,
        frontier_novelty_milli: claim.confidence_milli,
        workflow_leverage_milli: priority,
        cross_stage_unlock_milli: if kind == DecisionActionKind::ValidateMechanism {
            650
        } else {
            900
        },
        reproducibility_safety_milli: 900,
        federation_value_milli: 500,
        feasibility_milli: 800,
        autonomy_tier: AutonomyTier::A1,
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
        ]),
    };
    DecisionAction {
        action_id,
        claim_id: claim.claim_id.clone(),
        kind,
        rationale,
        target_modality: modality,
        target_model_system,
        priority_milli: priority,
        candidate,
    }
}

pub fn compile_decision_context(
    request: &DecisionContextRequest,
    knowledge: &TypedKnowledge,
) -> Result<DecisionContext, DecisionContextError> {
    if request.objective.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.default_cost_units == 0
    {
        return Err(DecisionContextError::InvalidRequest(
            "objective, action bound, and positive default cost are required".into(),
        ));
    }
    knowledge
        .validate()
        .map_err(|error| DecisionContextError::InvalidInput(error.to_string()))?;
    if request.objective.trim() != knowledge.objective.trim() {
        return Err(DecisionContextError::InvalidInput(
            "decision objective must match typed-knowledge objective".into(),
        ));
    }
    let claim_order = knowledge.claim_order.clone();
    let mut all_actions = knowledge
        .claims
        .iter()
        .map(|claim| action_for_claim(claim, request.default_cost_units))
        .collect::<Vec<_>>();
    all_actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let selected = all_actions
        .iter()
        .take(request.max_actions)
        .cloned()
        .collect::<Vec<_>>();
    let deferred_action_order = all_actions
        .iter()
        .skip(request.max_actions)
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut actions = selected;
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut omission_order = knowledge.omission_order.clone();
    if !deferred_action_order.is_empty() {
        omission_order.push("decision-action-cap-reached".into());
    }
    omission_order.sort();
    omission_order.dedup();
    let disposition = if actions.is_empty() {
        DecisionContextDisposition::Unresolved
    } else if knowledge.disposition
        == crate::glioma::programs::p02_evidence_knowledge::KnowledgeDisposition::Qualified
        && deferred_action_order.is_empty()
    {
        DecisionContextDisposition::Qualified
    } else {
        DecisionContextDisposition::Partial
    };
    let mut output = DecisionContext {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        actions,
        action_order,
        deferred_action_order,
        omission_order,
        negative_evidence_order: knowledge.negative_evidence_order.clone(),
        uncertainty_order: knowledge.uncertainty_order.clone(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| DecisionContextError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionContextError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn knowledge() -> TypedKnowledge {
        let record = EvidenceRecord {
            evidence_id: "e1".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "a1".into(),
                content_hash: hash("a1"),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "rank invasion mechanisms".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record],
        )
        .unwrap()
    }

    #[test]
    fn compiles_typed_action_candidate_for_supported_claim() {
        let output = compile_decision_context(
            &DecisionContextRequest {
                objective: "rank invasion mechanisms".into(),
                max_actions: 8,
                default_cost_units: 5,
            },
            &knowledge(),
        )
        .unwrap();
        assert_eq!(output.actions.len(), 1);
        assert_eq!(
            output.actions[0].candidate.stage_kind,
            GliomaStageKind::MechanismExploration
        );
        assert_eq!(output.disposition, DecisionContextDisposition::Qualified);
        let candidates = output
            .actions
            .iter()
            .map(|action| action.candidate.clone())
            .collect::<Vec<_>>();
        let selection = crate::glioma_engine::select_glioma_actions(
            &candidates,
            &BTreeSet::new(),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(selection.selected_order, output.action_order);
        output.validate().unwrap();
    }

    #[test]
    fn objective_mismatch_is_rejected_before_action_generation() {
        let error = compile_decision_context(
            &DecisionContextRequest {
                objective: "different objective".into(),
                max_actions: 8,
                default_cost_units: 5,
            },
            &knowledge(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("objective"));
    }
}
