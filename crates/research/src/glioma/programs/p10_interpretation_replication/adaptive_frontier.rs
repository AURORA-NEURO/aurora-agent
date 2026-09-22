//! Outcome-conditioned next-step planning for the autonomous preclinical glioma engine.
//!
//! A synthesis verdict is useful only if it changes what the research program does next.  This
//! feature turns the typed cross-family interpretation state into a bounded action frontier.  It
//! estimates which scientific debt dominates (contradiction, instability, missing replication,
//! missing modality, or an unresolved negative result), creates typed local actions to retire that
//! debt, and sends the candidates through the same dependency-safe multi-objective selector used
//! by the protocol director.  It is an adaptive research controller, not a clinical recommender:
//! it never promotes a hypothesis, invents an observation, or authorizes an instrument effect.

use super::{
    InterpretationEvidenceFamily, InterpretationSynthesis, InterpretationSynthesisDisposition,
};
use crate::glioma_engine::{
    select_glioma_actions, GliomaActionCandidate, GliomaActionSelection, GliomaEngineError,
    GliomaModality, GliomaModelSystem, GliomaSelectionWeights, GliomaStageKind,
};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveResearchFrontier1@1";
pub const MAX_CANDIDATES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveTarget {
    ContradictionResolution,
    ReplicationStrengthening,
    StabilityStressTest,
    EvidenceGapClosure,
    NegativeResultConfirmation,
    CrossModelExtension,
    MechanismDiscrimination,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveFrontierRequest {
    pub synthesis: InterpretationSynthesis,
    pub completed_actions: BTreeSet<String>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveFrontierCandidate {
    pub action: GliomaActionCandidate,
    pub target: AdaptiveTarget,
    pub rationale: String,
    pub expected_uncertainty_reduction_milli: u16,
    pub required_observation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveFrontierDisposition {
    Ready,
    Partial,
    Hold,
    NoRunnableActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveResearchFrontier {
    pub feature_id: String,
    pub output_schema: String,
    pub hypothesis: String,
    pub model_system: GliomaModelSystem,
    pub synthesis_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub candidates: Vec<AdaptiveFrontierCandidate>,
    pub selection: GliomaActionSelection,
    pub next_action_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: AdaptiveFrontierDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveFrontierError {
    #[error("adaptive frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive frontier selection failed: {0}")]
    Selection(String),
    #[error("adaptive frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive frontier digest failed: {0}")]
    Digest(String),
}

fn clamp(value: u32) -> u16 {
    value.min(1_000) as u16
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn score_from(parts: &[u16]) -> u16 {
    if parts.is_empty() {
        return 0;
    }
    clamp(parts.iter().map(|value| u32::from(*value)).sum::<u32>() / parts.len() as u32)
}

fn action_id(target: AdaptiveTarget) -> &'static str {
    match target {
        AdaptiveTarget::ContradictionResolution => {
            "adaptive:interpretation:contradiction-resolution"
        }
        AdaptiveTarget::ReplicationStrengthening => {
            "adaptive:interpretation:replication-strengthening"
        }
        AdaptiveTarget::StabilityStressTest => "adaptive:interpretation:stability-stress-test",
        AdaptiveTarget::EvidenceGapClosure => "adaptive:interpretation:evidence-gap-closure",
        AdaptiveTarget::NegativeResultConfirmation => {
            "adaptive:interpretation:negative-confirmation"
        }
        AdaptiveTarget::CrossModelExtension => "adaptive:interpretation:cross-model-extension",
        AdaptiveTarget::MechanismDiscrimination => {
            "adaptive:interpretation:mechanism-discrimination"
        }
    }
}

fn required_observation(target: AdaptiveTarget) -> &'static str {
    match target {
        AdaptiveTarget::ContradictionResolution =>
            "an independent preclinical assay or reanalysis that resolves the signed family disagreement",
        AdaptiveTarget::ReplicationStrengthening =>
            "an independent-group replication with the same estimand and model binding",
        AdaptiveTarget::StabilityStressTest =>
            "a bounded leave-one-family, batch, or modality stress result with its omission ledger",
        AdaptiveTarget::EvidenceGapClosure =>
            "a qualified local source or multimodal observation covering the missing evidence family",
        AdaptiveTarget::NegativeResultConfirmation =>
            "an independent null/negative observation that preserves the original estimand and power record",
        AdaptiveTarget::CrossModelExtension =>
            "a preclinical model-system transfer with explicit transportability and comparability checks",
        AdaptiveTarget::MechanismDiscrimination =>
            "a perturbation or computational contrast that separates the leading competing mechanisms",
    }
}

fn action_template(
    target: AdaptiveTarget,
    model_system: GliomaModelSystem,
    scores: [u16; 7],
) -> GliomaActionCandidate {
    let (stage_kind, modality, cost_units, autonomy_tier) = match target {
        AdaptiveTarget::ContradictionResolution => (
            GliomaStageKind::StatisticalInterpretation,
            GliomaModality::Computational,
            14,
            AutonomyTier::A1,
        ),
        AdaptiveTarget::ReplicationStrengthening => (
            GliomaStageKind::ReplicationRobustness,
            GliomaModality::Replication,
            28,
            AutonomyTier::A1,
        ),
        AdaptiveTarget::StabilityStressTest => (
            GliomaStageKind::StatisticalInterpretation,
            GliomaModality::Computational,
            10,
            AutonomyTier::A1,
        ),
        AdaptiveTarget::EvidenceGapClosure => (
            GliomaStageKind::EvidenceCompilation,
            GliomaModality::Literature,
            16,
            AutonomyTier::A1,
        ),
        AdaptiveTarget::NegativeResultConfirmation => (
            GliomaStageKind::ReplicationRobustness,
            GliomaModality::FunctionalPerturbation,
            32,
            AutonomyTier::A2,
        ),
        AdaptiveTarget::CrossModelExtension => (
            GliomaStageKind::MolecularLandscape,
            GliomaModality::SingleCell,
            36,
            AutonomyTier::A2,
        ),
        AdaptiveTarget::MechanismDiscrimination => (
            GliomaStageKind::MechanismExploration,
            GliomaModality::FunctionalPerturbation,
            24,
            AutonomyTier::A2,
        ),
    };
    let action_model_system = if target == AdaptiveTarget::CrossModelExtension {
        match model_system {
            GliomaModelSystem::CellLine => GliomaModelSystem::Organoid,
            GliomaModelSystem::Organoid => GliomaModelSystem::PatientDerivedXenograft,
            GliomaModelSystem::PatientDerivedXenograft => GliomaModelSystem::MouseModel,
            GliomaModelSystem::MouseModel => GliomaModelSystem::Organoid,
            GliomaModelSystem::ZebrafishModel => GliomaModelSystem::Organoid,
            GliomaModelSystem::InSilico => GliomaModelSystem::Organoid,
        }
    } else {
        model_system
    };
    GliomaActionCandidate {
        action_id: action_id(target).into(),
        stage_kind,
        modality,
        model_system: action_model_system,
        depends_on: Vec::new(),
        cost_units,
        information_gain_milli: scores[0],
        frontier_novelty_milli: scores[1],
        workflow_leverage_milli: scores[2],
        cross_stage_unlock_milli: scores[3],
        reproducibility_safety_milli: scores[4],
        federation_value_milli: scores[5],
        feasibility_milli: scores[6],
        autonomy_tier,
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
        ]
        .into_iter()
        .collect(),
    }
}

fn digest_input(output: &AdaptiveResearchFrontier) -> serde_json::Value {
    json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "hypothesis": output.hypothesis,
        "model_system": output.model_system,
        "synthesis_digest": output.synthesis_digest,
        "candidate_order": output.candidate_order,
        "candidates": output.candidates,
        "selection": output.selection,
        "next_action_order": output.next_action_order,
        "hold_order": output.hold_order,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl AdaptiveResearchFrontier {
    pub fn validate(&self) -> Result<(), AdaptiveFrontierError> {
        self.selection
            .validate()
            .map_err(|error| AdaptiveFrontierError::InvalidOutput(error.to_string()))?;
        let ids = self
            .candidates
            .iter()
            .map(|candidate| candidate.action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveFrontierError::Digest(error.to_string()))?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.hypothesis.trim().is_empty()
            || self.candidates.is_empty()
            || self.candidates.len() > MAX_CANDIDATES
            || self.candidate_order != ids.iter().cloned().collect::<Vec<_>>()
            || !canonical(&self.candidate_order)
            || self.selection.candidate_order != self.candidate_order
            || self.next_action_order != self.selection.selected_order
            || !canonical(&self.hold_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.next_step.trim().is_empty()
            || self.candidates.iter().any(|candidate| {
                candidate.action.action_id.trim().is_empty()
                    || candidate.rationale.trim().is_empty()
                    || candidate.required_observation.trim().is_empty()
                    || candidate.expected_uncertainty_reduction_milli > 1_000
            })
            || self.digest != expected
        {
            return Err(AdaptiveFrontierError::InvalidOutput(
                "identity, candidate, selector, ordering, limitation, or digest invariant failed"
                    .into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &AdaptiveFrontierRequest) -> Result<(), AdaptiveFrontierError> {
    request
        .synthesis
        .validate()
        .map_err(|error| AdaptiveFrontierError::InvalidRequest(error.to_string()))?;
    if request.budget_units == 0 || request.max_actions == 0 {
        return Err(AdaptiveFrontierError::InvalidRequest(
            "adaptive frontier requires a positive budget and max_actions".into(),
        ));
    }
    if request
        .completed_actions
        .iter()
        .any(|id| id.trim().is_empty())
    {
        return Err(AdaptiveFrontierError::InvalidRequest(
            "completed action identifiers cannot be empty".into(),
        ));
    }
    Ok(())
}

fn build_candidates(synthesis: &InterpretationSynthesis) -> Vec<AdaptiveFrontierCandidate> {
    let contradiction = u32::from(synthesis.contradiction_milli);
    let disagreement = clamp((synthesis.disagreement_milli / 2) as u32);
    let instability = 1_000u16.saturating_sub(synthesis.stability_milli);
    let replication_gap = if synthesis
        .families
        .iter()
        .any(|family| family.family == InterpretationEvidenceFamily::Replication)
    {
        250
    } else {
        950
    };
    let family_gap = clamp((3usize.saturating_sub(synthesis.family_order.len()) * 260) as u32);
    let unresolved = clamp((synthesis.unresolved_evidence_order.len() * 180) as u32);
    let negative = if matches!(
        synthesis.disposition,
        InterpretationSynthesisDisposition::Negative
    ) {
        950
    } else if synthesis.negative_evidence.is_empty() {
        180
    } else {
        650
    };
    let transport_gap = if synthesis.independent_group_order.len() < 2 {
        820
    } else {
        360
    };
    let mechanism_gap = score_from(&[contradiction as u16, instability, unresolved]);
    let model = synthesis.model_system;
    let specs = [
        (
            AdaptiveTarget::ContradictionResolution,
            [
                score_from(&[contradiction as u16, disagreement, unresolved]),
                820,
                900,
                760,
                860,
                520,
                720,
            ],
            format!(
                "contradiction={} disagreement={} unresolved={}; isolate the evidence family that drives the signed conflict",
                synthesis.contradiction_milli, synthesis.disagreement_milli, synthesis.unresolved_evidence_order.len()
            ),
        ),
        (
            AdaptiveTarget::ReplicationStrengthening,
            [replication_gap, 700, 860, 900, 950, 820, 560],
            format!(
                "replication-family gap={} independent-groups={}; repeat the declared estimand before any claim promotion",
                replication_gap, synthesis.independent_group_order.len()
            ),
        ),
        (
            AdaptiveTarget::StabilityStressTest,
            [instability, 740, 780, 680, 980, 620, 840],
            format!(
                "stability={} and leave-one-family range=[{},{}]; stress the weakest support before interpreting the direction",
                synthesis.stability_milli, synthesis.leave_one_out_low_milli, synthesis.leave_one_out_high_milli
            ),
        ),
        (
            AdaptiveTarget::EvidenceGapClosure,
            [family_gap.max(unresolved), 680, 820, 800, 900, 580, 680],
            format!(
                "families={} unresolved-evidence={}; acquire only the missing typed evidence needed to close the frontier",
                synthesis.family_order.len(), synthesis.unresolved_evidence_order.len()
            ),
        ),
        (
            AdaptiveTarget::NegativeResultConfirmation,
            [negative, 610, 730, 690, 990, 500, 500],
            format!(
                "disposition={:?} negative-items={}; independently test whether the null/negative result is reproducible",
                synthesis.disposition, synthesis.negative_evidence.len()
            ),
        ),
        (
            AdaptiveTarget::CrossModelExtension,
            [transport_gap, 780, 720, 760, 900, 900, 420],
            format!(
                "independent-groups={} model={:?}; add a preclinical model transfer with explicit comparability gates",
                synthesis.independent_group_order.len(), synthesis.model_system
            ),
        ),
        (
            AdaptiveTarget::MechanismDiscrimination,
            [mechanism_gap, 930, 880, 870, 860, 540, 600],
            "the synthesis remains compatible with competing mechanisms; run a discriminating computational or perturbational contrast".into(),
        ),
    ];
    specs
        .into_iter()
        .map(|(target, scores, rationale)| AdaptiveFrontierCandidate {
            action: action_template(target, model, scores),
            target,
            rationale,
            expected_uncertainty_reduction_milli: score_from(&scores[..4]),
            required_observation: required_observation(target).into(),
        })
        .collect()
}

/// Compile an interpretation outcome into the next bounded, executable research frontier.
pub fn plan_glioma_adaptive_research_frontier(
    request: &AdaptiveFrontierRequest,
) -> Result<AdaptiveResearchFrontier, AdaptiveFrontierError> {
    validate_request(request)?;
    let mut candidates = build_candidates(&request.synthesis);
    candidates.sort_by(|left, right| left.action.action_id.cmp(&right.action.action_id));
    let actions = candidates
        .iter()
        .map(|candidate| candidate.action.clone())
        .collect::<Vec<_>>();
    let selection_config = crate::glioma_engine::GliomaSelectionConfig {
        budget_units: request.budget_units,
        max_actions: request.max_actions,
        approval_granted: request.approval_granted,
        allow_instrument_execution: request.allow_instrument_execution,
        allow_federation: request.allow_federation,
        weights: request.selection_weights,
    };
    let selection = select_glioma_actions(&actions, &request.completed_actions, &selection_config)
        .map_err(|error| AdaptiveFrontierError::Selection(error.to_string()))?;
    let mut holds = BTreeSet::new();
    if matches!(
        request.synthesis.disposition,
        InterpretationSynthesisDisposition::Unresolved
    ) {
        holds.insert("synthesis-unresolved:do-not-promote-a-direction".into());
    }
    if matches!(
        request.synthesis.disposition,
        InterpretationSynthesisDisposition::Negative
    ) {
        holds.insert("negative-result:confirmation-required-before-reframing".into());
    }
    if selection.selected_order.is_empty() {
        holds.insert("selector-returned-no-runnable-actions".into());
    }
    let mut uncertainty = BTreeSet::new();
    if request.synthesis.disagreement_milli > 0 {
        uncertainty.insert(format!(
            "cross-family-disagreement:{}",
            request.synthesis.disagreement_milli
        ));
    }
    uncertainty.extend(request.synthesis.uncertainty.iter().cloned());
    let mut negative_evidence = request
        .synthesis
        .negative_evidence
        .iter()
        .map(|item| format!("synthesis:{item}"))
        .collect::<BTreeSet<_>>();
    if matches!(
        request.synthesis.disposition,
        InterpretationSynthesisDisposition::Negative
    ) {
        negative_evidence
            .insert("negative-synthesis-is-not-a-clinical-or-treatment-decision".into());
    }
    let disposition = if selection.selected_order.is_empty() {
        AdaptiveFrontierDisposition::NoRunnableActions
    } else if matches!(
        request.synthesis.disposition,
        InterpretationSynthesisDisposition::Unresolved
    ) {
        AdaptiveFrontierDisposition::Hold
    } else if matches!(
        request.synthesis.disposition,
        InterpretationSynthesisDisposition::Qualified
    ) {
        AdaptiveFrontierDisposition::Ready
    } else {
        AdaptiveFrontierDisposition::Partial
    };
    let next_step = if selection.selected_order.is_empty() {
        "obtain the missing approval, local artifact, or budget before dispatching an adaptive action".into()
    } else if holds.is_empty() {
        format!(
            "dispatch the selected adaptive actions through an institution-local executor, then resynthesize {}",
            selection.selected_order.join(", ")
        )
    } else {
        format!(
            "resolve the declared hold(s) before interpreting the selected actions: {}",
            holds.iter().cloned().collect::<Vec<_>>().join("; ")
        )
    };
    let mut output = AdaptiveResearchFrontier {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        hypothesis: request.synthesis.hypothesis.clone(),
        model_system: request.synthesis.model_system,
        synthesis_digest: request.synthesis.digest.clone(),
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.action.action_id.clone())
            .collect(),
        candidates,
        selection,
        next_action_order: Vec::new(),
        hold_order: holds.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-adaptive-glioma-frontier"),
    };
    output.next_action_order = output.selection.selected_order.clone();
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl From<GliomaEngineError> for AdaptiveFrontierError {
    fn from(error: GliomaEngineError) -> Self {
        Self::Selection(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::{
        synthesize_glioma_interpretation, InterpretationEvidence, InterpretationEvidenceDirection,
        InterpretationSynthesisRequest,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn synthesis(disposition: InterpretationSynthesisDisposition) -> InterpretationSynthesis {
        let hash = ContentHash::of_bytes(b"adaptive-frontier-test");
        let family = |id: &str, family, direction| InterpretationEvidence {
            evidence_id: id.into(),
            family,
            independent_group: id.into(),
            model_system: GliomaModelSystem::Organoid,
            direction,
            effect_milli: 400,
            uncertainty_milli: 40,
            quality_milli: 900,
            sample_count: 8,
            artifact: LocalArtifactRef {
                artifact_id: id.into(),
                content_hash: hash.clone(),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            negative_evidence: Vec::new(),
        };
        let mut evidence = vec![
            family(
                "causal",
                InterpretationEvidenceFamily::CausalContrast,
                InterpretationEvidenceDirection::Positive,
            ),
            family(
                "replication",
                InterpretationEvidenceFamily::Replication,
                InterpretationEvidenceDirection::Positive,
            ),
            family(
                "sensitivity",
                InterpretationEvidenceFamily::Sensitivity,
                InterpretationEvidenceDirection::Positive,
            ),
        ];
        if disposition == InterpretationSynthesisDisposition::Negative {
            for item in &mut evidence {
                item.direction = InterpretationEvidenceDirection::Null;
                item.effect_milli = 20;
            }
        }
        let request = InterpretationSynthesisRequest {
            objective: "adaptive test".into(),
            hypothesis: "invasion depends on a preclinical mechanism".into(),
            model_system: GliomaModelSystem::Organoid,
            min_evidence: 2,
            min_independent_groups: 2,
            min_families: if disposition == InterpretationSynthesisDisposition::Unresolved {
                4
            } else {
                2
            },
            min_quality_milli: 700,
            effect_threshold_milli: 100,
            max_disagreement_milli: 700,
            max_leave_one_out_shift_milli: 700,
            require_replication_family: disposition
                != InterpretationSynthesisDisposition::Unresolved,
            replay_identity: hash,
            evidence,
        };
        let output = synthesize_glioma_interpretation(&request).unwrap();
        assert_eq!(output.disposition, disposition);
        output
    }

    fn request(output: InterpretationSynthesis) -> AdaptiveFrontierRequest {
        AdaptiveFrontierRequest {
            synthesis: output,
            completed_actions: BTreeSet::new(),
            budget_units: 80,
            max_actions: 3,
            approval_granted: true,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
        }
    }

    #[test]
    fn frontier_is_deterministic_and_selects_scientific_followups() {
        let request = request(synthesis(InterpretationSynthesisDisposition::Qualified));
        let first = plan_glioma_adaptive_research_frontier(&request).unwrap();
        let second = plan_glioma_adaptive_research_frontier(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.candidates.len(), 7);
        assert!(!first.next_action_order.is_empty());
        assert!(first.next_step.contains("resynthesize"));
        let cross_model = first
            .candidates
            .iter()
            .find(|candidate| candidate.target == AdaptiveTarget::CrossModelExtension)
            .expect("cross-model candidate");
        assert_ne!(cross_model.action.model_system, first.model_system);
    }

    #[test]
    fn unresolved_synthesis_holds_claim_promotion_but_still_plans_information_gain() {
        let output = synthesis(InterpretationSynthesisDisposition::Unresolved);
        let frontier = plan_glioma_adaptive_research_frontier(&request(output)).unwrap();
        assert_eq!(frontier.disposition, AdaptiveFrontierDisposition::Hold);
        assert!(frontier
            .hold_order
            .iter()
            .any(|item| item.contains("synthesis-unresolved")));
        assert!(!frontier.next_action_order.is_empty());
    }

    #[test]
    fn completed_action_is_not_reselected() {
        let mut request = request(synthesis(InterpretationSynthesisDisposition::Qualified));
        request
            .completed_actions
            .insert(action_id(AdaptiveTarget::ReplicationStrengthening).into());
        let frontier = plan_glioma_adaptive_research_frontier(&request).unwrap();
        assert!(!frontier
            .next_action_order
            .contains(&action_id(AdaptiveTarget::ReplicationStrengthening).into()));
        assert!(frontier
            .selection
            .blocked_order
            .contains(&action_id(AdaptiveTarget::ReplicationStrengthening).into()));
    }

    #[test]
    fn negative_synthesis_keeps_negative_evidence_first_class() {
        let frontier = plan_glioma_adaptive_research_frontier(&request(synthesis(
            InterpretationSynthesisDisposition::Negative,
        )))
        .unwrap();
        assert_eq!(frontier.disposition, AdaptiveFrontierDisposition::Partial);
        assert!(frontier
            .negative_evidence
            .iter()
            .any(|item| item.contains("negative-synthesis")));
    }

    #[test]
    fn rejects_zero_budget_before_selector() {
        let mut request = request(synthesis(InterpretationSynthesisDisposition::Qualified));
        request.budget_units = 0;
        assert!(matches!(
            plan_glioma_adaptive_research_frontier(&request),
            Err(AdaptiveFrontierError::InvalidRequest(_))
        ));
    }
}
