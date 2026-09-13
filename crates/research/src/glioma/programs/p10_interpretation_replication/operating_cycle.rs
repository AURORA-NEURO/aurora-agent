//! Interpretation-gate and adaptive-frontier operating cycle for preclinical glioma research.
//!
//! P10 has rigorous family analyses, cross-family synthesis, and a next-action frontier. This
//! feature makes their handoff one deterministic product capability: compile the interpretation
//! gate, preserve contradiction/negative/omission state, and emit a bounded executable frontier
//! for the next replication, stress, mechanism, or cross-model action. The returned frontier is
//! a research-program handoff; it is never a clinical recommendation or an unreviewed instrument
//! authorization.

use super::adaptive_frontier::{
    plan_glioma_adaptive_research_frontier, AdaptiveFrontierError, AdaptiveFrontierRequest,
    AdaptiveResearchFrontier,
};
use super::synthesis::{
    synthesize_glioma_interpretation, InterpretationSynthesis, InterpretationSynthesisDisposition,
    InterpretationSynthesisError, InterpretationSynthesisRequest,
};
use crate::glioma_engine::GliomaSelectionWeights;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaInterpretationOperatingCycle1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaInterpretationOperatingCycleRequest {
    pub synthesis: InterpretationSynthesisRequest,
    pub completed_actions: BTreeSet<String>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationOperatingCycleDisposition {
    Ready,
    Negative,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaInterpretationOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub hypothesis: String,
    pub phase_order: Vec<String>,
    pub synthesis: InterpretationSynthesis,
    pub frontier: AdaptiveResearchFrontier,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: InterpretationOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaInterpretationOperatingCycleError {
    #[error("interpretation operating-cycle synthesis failed: {0}")]
    Synthesis(#[from] InterpretationSynthesisError),
    #[error("interpretation operating-cycle frontier failed: {0}")]
    Frontier(#[from] AdaptiveFrontierError),
    #[error("interpretation operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("interpretation operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaInterpretationOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "hypothesis": output.hypothesis,
        "phase_order": output.phase_order,
        "synthesis": output.synthesis,
        "frontier": output.frontier,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn disposition(
    synthesis: &InterpretationSynthesis,
    frontier: &AdaptiveResearchFrontier,
) -> InterpretationOperatingCycleDisposition {
    if matches!(
        synthesis.disposition,
        InterpretationSynthesisDisposition::Negative
    ) {
        InterpretationOperatingCycleDisposition::Negative
    } else if matches!(
        synthesis.disposition,
        InterpretationSynthesisDisposition::Unresolved
    ) || matches!(
        frontier.disposition,
        super::adaptive_frontier::AdaptiveFrontierDisposition::Hold
    ) {
        InterpretationOperatingCycleDisposition::Unresolved
    } else if matches!(
        frontier.disposition,
        super::adaptive_frontier::AdaptiveFrontierDisposition::NoRunnableActions
    ) {
        InterpretationOperatingCycleDisposition::Blocked
    } else if matches!(
        synthesis.disposition,
        InterpretationSynthesisDisposition::Partial
    ) || matches!(
        frontier.disposition,
        super::adaptive_frontier::AdaptiveFrontierDisposition::Partial
    ) {
        InterpretationOperatingCycleDisposition::Partial
    } else {
        InterpretationOperatingCycleDisposition::Ready
    }
}

impl GliomaInterpretationOperatingCycle {
    pub fn validate(&self) -> Result<(), GliomaInterpretationOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.phase_order
                != [
                    "interpretation_gate".to_string(),
                    "adaptive_frontier".to_string(),
                    "operator_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self.synthesis.objective != self.objective
            || self.synthesis.hypothesis != self.hypothesis
            || self.frontier.hypothesis != self.hypothesis
            || self.frontier.synthesis_digest != self.synthesis.digest
        {
            return Err(GliomaInterpretationOperatingCycleError::InvalidOutput(
                "identity, phase order, synthesis/frontier binding, or canonical evidence is invalid".into(),
            ));
        }
        self.synthesis.validate().map_err(|error| {
            GliomaInterpretationOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        self.frontier.validate().map_err(|error| {
            GliomaInterpretationOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaInterpretationOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaInterpretationOperatingCycleError::InvalidOutput(
                "interpretation operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a cross-family interpretation gate and its bounded adaptive next-action frontier.
pub fn execute_glioma_interpretation_operating_cycle(
    request: &GliomaInterpretationOperatingCycleRequest,
) -> Result<GliomaInterpretationOperatingCycle, GliomaInterpretationOperatingCycleError> {
    let synthesis = synthesize_glioma_interpretation(&request.synthesis)?;
    let frontier = plan_glioma_adaptive_research_frontier(&AdaptiveFrontierRequest {
        synthesis: synthesis.clone(),
        completed_actions: request.completed_actions.clone(),
        budget_units: request.budget_units,
        max_actions: request.max_actions,
        approval_granted: request.approval_granted,
        allow_instrument_execution: request.allow_instrument_execution,
        allow_federation: request.allow_federation,
        selection_weights: request.selection_weights,
    })?;
    let disposition = disposition(&synthesis, &frontier);
    let mut negative_evidence = synthesis.negative_evidence.clone();
    negative_evidence.extend(frontier.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = synthesis.uncertainty.clone();
    uncertainty.extend(frontier.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = match disposition {
        InterpretationOperatingCycleDisposition::Ready
        | InterpretationOperatingCycleDisposition::Partial => frontier.next_step.clone(),
        InterpretationOperatingCycleDisposition::Negative => {
            "publish the null/negative interpretation and require independent confirmation before reframing".into()
        }
        InterpretationOperatingCycleDisposition::Blocked => {
            "resolve approval, local-artifact, budget, or policy holds before dispatching the frontier".into()
        }
        InterpretationOperatingCycleDisposition::Unresolved => frontier.next_step.clone(),
    };
    let mut output = GliomaInterpretationOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: synthesis.objective.clone(),
        hypothesis: synthesis.hypothesis.clone(),
        phase_order: vec![
            "interpretation_gate".into(),
            "adaptive_frontier".into(),
            "operator_handoff".into(),
        ],
        synthesis,
        frontier,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-interpretation-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaInterpretationOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn request() -> GliomaInterpretationOperatingCycleRequest {
        let hash = ContentHash::of_bytes(b"interpretation-cycle-test");
        let evidence = [
            (
                "causal",
                super::super::synthesis::InterpretationEvidenceFamily::CausalContrast,
            ),
            (
                "replication",
                super::super::synthesis::InterpretationEvidenceFamily::Replication,
            ),
            (
                "sensitivity",
                super::super::synthesis::InterpretationEvidenceFamily::Sensitivity,
            ),
        ]
        .into_iter()
        .map(
            |(id, family)| super::super::synthesis::InterpretationEvidence {
                evidence_id: id.into(),
                family,
                independent_group: id.into(),
                model_system: GliomaModelSystem::Organoid,
                direction: super::super::synthesis::InterpretationEvidenceDirection::Positive,
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
            },
        )
        .collect();
        GliomaInterpretationOperatingCycleRequest {
            synthesis: InterpretationSynthesisRequest {
                objective: "decide whether invasion is reproducibly supported".into(),
                hypothesis: "integrated invasion program is activated".into(),
                model_system: GliomaModelSystem::Organoid,
                min_evidence: 2,
                min_independent_groups: 2,
                min_families: 2,
                min_quality_milli: 700,
                effect_threshold_milli: 100,
                max_disagreement_milli: 700,
                max_leave_one_out_shift_milli: 700,
                require_replication_family: true,
                replay_identity: hash,
                evidence,
            },
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
    fn operating_cycle_gates_and_replays_adaptive_frontier() {
        let first = execute_glioma_interpretation_operating_cycle(&request()).unwrap();
        let second = execute_glioma_interpretation_operating_cycle(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            InterpretationOperatingCycleDisposition::Ready
        );
        assert!(!first.frontier.next_action_order.is_empty());
        first.validate().unwrap();
    }
}
