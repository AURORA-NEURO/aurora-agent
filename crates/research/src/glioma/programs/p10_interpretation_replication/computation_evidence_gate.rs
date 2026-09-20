//! Evidence-aware handoff from the P09 computation frontier into P10 interpretation.
//!
//! A computation run is not a scientific conclusion. This feature binds returned, typed
//! computation summaries to their routed frontier actions, refuses observations for actions that
//! did not complete, and then applies the existing cross-family synthesis algorithm. Missing
//! summaries, negative results, replay drift, and incomplete frontier coverage remain explicit.

use super::synthesis::{
    synthesize_glioma_interpretation, InterpretationEvidence, InterpretationEvidenceDirection,
    InterpretationEvidenceFamily, InterpretationSynthesis, InterpretationSynthesisDisposition,
    InterpretationSynthesisError, InterpretationSynthesisRequest,
};
use crate::glioma::programs::p09_reproducible_computation::{
    ComputationInterpretationFrontierError, ComputationInterpretationFrontierRun,
};
use crate::glioma_engine::{GliomaModelSystem, GliomaStageKind, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationInterpretationEvidenceGate1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationSynthesisPolicy {
    pub objective: String,
    pub hypothesis: String,
    pub model_system: GliomaModelSystem,
    pub min_evidence: usize,
    pub min_independent_groups: usize,
    pub min_families: usize,
    pub min_quality_milli: u16,
    pub effect_threshold_milli: u64,
    pub max_disagreement_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub require_replication_family: bool,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationObservation {
    pub action_id: String,
    pub independent_group: String,
    pub direction: InterpretationEvidenceDirection,
    pub effect_milli: u64,
    pub uncertainty_milli: u64,
    pub quality_milli: u16,
    pub sample_count: u32,
    pub artifact: LocalArtifactRef,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationEvidenceGateRequest {
    pub frontier: ComputationInterpretationFrontierRun,
    pub policy: ComputationInterpretationSynthesisPolicy,
    pub observations: Vec<ComputationInterpretationObservation>,
    pub require_complete_frontier: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationInterpretationEvidenceGateDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationEvidenceGate {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub hypothesis: String,
    pub replay_identity: ContentHash,
    pub frontier_digest: ContentHash,
    pub admitted_action_order: Vec<String>,
    pub observed_action_order: Vec<String>,
    pub omitted_action_order: Vec<String>,
    pub negative_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub synthesis: Option<InterpretationSynthesis>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationInterpretationEvidenceGateDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationInterpretationEvidenceGateError {
    #[error("computation interpretation evidence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation interpretation frontier is invalid: {0}")]
    Frontier(#[from] ComputationInterpretationFrontierError),
    #[error("computation interpretation synthesis failed: {0}")]
    Synthesis(#[from] InterpretationSynthesisError),
    #[error("computation interpretation evidence output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation interpretation evidence digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ComputationInterpretationEvidenceGate) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "hypothesis": output.hypothesis,
        "replay_identity": output.replay_identity,
        "frontier_digest": output.frontier_digest,
        "admitted_action_order": output.admitted_action_order,
        "observed_action_order": output.observed_action_order,
        "omitted_action_order": output.omitted_action_order,
        "negative_action_order": output.negative_action_order,
        "unresolved_action_order": output.unresolved_action_order,
        "next_action_order": output.next_action_order,
        "synthesis": output.synthesis,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_policy(
    policy: &ComputationInterpretationSynthesisPolicy,
) -> Result<(), ComputationInterpretationEvidenceGateError> {
    if policy.objective.trim().is_empty()
        || policy.hypothesis.trim().is_empty()
        || policy.min_evidence == 0
        || policy.min_independent_groups == 0
        || policy.min_families == 0
        || policy.min_quality_milli > 1_000
        || policy.replay_identity.as_str().len() != 64
    {
        return Err(ComputationInterpretationEvidenceGateError::InvalidRequest(
            "objective, hypothesis, synthesis floors, and replay identity are required".into(),
        ));
    }
    Ok(())
}

fn validate_observation(
    observation: &ComputationInterpretationObservation,
) -> Result<(), ComputationInterpretationEvidenceGateError> {
    if observation.action_id.trim().is_empty()
        || observation.independent_group.trim().is_empty()
        || observation.sample_count == 0
        || observation.quality_milli > 1_000
        || observation.uncertainty_milli > 1_000_000_000
        || matches!(
            observation.direction,
            InterpretationEvidenceDirection::Positive | InterpretationEvidenceDirection::Negative
        ) && observation.effect_milli == 0
        || observation
            .negative_evidence
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(ComputationInterpretationEvidenceGateError::InvalidRequest(
            "observation identity, sample/quality/uncertainty bounds, direction, and negative-evidence explanations are invalid".into(),
        ));
    }
    observation.artifact.validate().map_err(|error| {
        ComputationInterpretationEvidenceGateError::InvalidRequest(error.to_string())
    })
}

impl ComputationInterpretationEvidenceGate {
    pub fn validate(&self) -> Result<(), ComputationInterpretationEvidenceGateError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || self.frontier_digest.as_str().len() != 64
            || !canonical(&self.admitted_action_order)
            || !canonical(&self.observed_action_order)
            || !canonical(&self.omitted_action_order)
            || !canonical(&self.negative_action_order)
            || !canonical(&self.unresolved_action_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .observed_action_order
                .iter()
                .any(|id| !self.admitted_action_order.contains(id))
            || self
                .negative_action_order
                .iter()
                .any(|id| !self.observed_action_order.contains(id))
        {
            return Err(ComputationInterpretationEvidenceGateError::InvalidOutput(
                "identity, replay binding, ordering, and action partitions are invalid".into(),
            ));
        }
        if let Some(synthesis) = &self.synthesis {
            synthesis.validate().map_err(|error| {
                ComputationInterpretationEvidenceGateError::InvalidOutput(error.to_string())
            })?;
            if synthesis.objective != self.objective
                || synthesis.hypothesis != self.hypothesis
                || synthesis.replay_identity != self.replay_identity
            {
                return Err(ComputationInterpretationEvidenceGateError::InvalidOutput(
                    "nested synthesis is not bound to the frontier gate".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self)).map_err(|error| {
            ComputationInterpretationEvidenceGateError::Digest(error.to_string())
        })?;
        if expected != self.digest {
            return Err(ComputationInterpretationEvidenceGateError::InvalidOutput(
                "evidence gate digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn family_for(stage: GliomaStageKind) -> InterpretationEvidenceFamily {
    match stage {
        GliomaStageKind::ReplicationRobustness => InterpretationEvidenceFamily::Replication,
        _ => InterpretationEvidenceFamily::Computation,
    }
}

fn disposition_for(
    frontier: &ComputationInterpretationFrontierRun,
    synthesis: Option<&InterpretationSynthesis>,
    omitted: &[String],
    unresolved: &[String],
) -> ComputationInterpretationEvidenceGateDisposition {
    if matches!(
        frontier.disposition,
        crate::glioma::programs::p09_reproducible_computation::ComputationInterpretationFrontierDisposition::Blocked
    ) {
        return ComputationInterpretationEvidenceGateDisposition::Blocked;
    }
    if synthesis.is_none() || !omitted.is_empty() || !unresolved.is_empty() {
        return ComputationInterpretationEvidenceGateDisposition::Unresolved;
    }
    match synthesis.expect("checked above").disposition {
        InterpretationSynthesisDisposition::Qualified if omitted.is_empty() => {
            ComputationInterpretationEvidenceGateDisposition::Qualified
        }
        InterpretationSynthesisDisposition::Negative => {
            ComputationInterpretationEvidenceGateDisposition::Negative
        }
        InterpretationSynthesisDisposition::Partial => {
            ComputationInterpretationEvidenceGateDisposition::Partial
        }
        InterpretationSynthesisDisposition::Unresolved => {
            ComputationInterpretationEvidenceGateDisposition::Unresolved
        }
        InterpretationSynthesisDisposition::Qualified => {
            ComputationInterpretationEvidenceGateDisposition::Partial
        }
    }
}

/// Bind computation outcomes to their routed actions and run the P10 evidence synthesis gate.
pub fn execute_glioma_computation_interpretation_evidence_gate(
    request: &ComputationInterpretationEvidenceGateRequest,
) -> Result<ComputationInterpretationEvidenceGate, ComputationInterpretationEvidenceGateError> {
    request.frontier.frontier.validate()?;
    request.frontier.validate()?;
    validate_policy(&request.policy)?;
    if request.frontier.objective != request.policy.objective
        || request.frontier.frontier.replay_identity != request.policy.replay_identity
    {
        return Err(ComputationInterpretationEvidenceGateError::InvalidRequest(
            "frontier objective and replay identity must match the interpretation policy".into(),
        ));
    }
    if request.observations.len() > MAX_OBSERVATIONS {
        return Err(ComputationInterpretationEvidenceGateError::InvalidRequest(
            "observation count exceeds bounded evidence gate capacity".into(),
        ));
    }
    let candidates = request
        .frontier
        .frontier
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate.stage_kind))
        .collect::<BTreeMap<_, _>>();
    let admitted = request.frontier.frontier.action_order.clone();
    let completed = request
        .frontier
        .mission
        .as_ref()
        .map(|mission| {
            mission
                .completed_action_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let negative = request
        .frontier
        .mission
        .as_ref()
        .map(|mission| {
            mission
                .negative_action_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let executable = completed.union(&negative).cloned().collect::<BTreeSet<_>>();
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut seen = BTreeSet::new();
    let mut evidence = Vec::new();
    let mut observed = BTreeSet::new();
    let mut negative_actions = BTreeSet::new();
    let mut unresolved_actions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for observation in &observations {
        validate_observation(observation)?;
        let Some(stage) = candidates.get(&observation.action_id).copied() else {
            return Err(ComputationInterpretationEvidenceGateError::InvalidRequest(
                "observation action is not present in the computation frontier".into(),
            ));
        };
        if !seen.insert(observation.action_id.clone()) {
            return Err(ComputationInterpretationEvidenceGateError::InvalidRequest(
                "each computation frontier action may have one canonical observation".into(),
            ));
        }
        if !executable.contains(&observation.action_id) {
            unresolved_actions.insert(observation.action_id.clone());
            uncertainty.insert(format!(
                "{}:observation-before-computation-completion",
                observation.action_id
            ));
            continue;
        }
        if matches!(
            observation.direction,
            InterpretationEvidenceDirection::Negative | InterpretationEvidenceDirection::Null
        ) || negative.contains(&observation.action_id)
        {
            negative_actions.insert(observation.action_id.clone());
        }
        negative_evidence.extend(observation.negative_evidence.iter().cloned());
        observed.insert(observation.action_id.clone());
        evidence.push(InterpretationEvidence {
            evidence_id: format!("computation:{}", observation.action_id),
            family: family_for(stage),
            independent_group: observation.independent_group.clone(),
            model_system: request.policy.model_system,
            direction: observation.direction,
            effect_milli: observation.effect_milli,
            uncertainty_milli: observation.uncertainty_milli,
            quality_milli: observation.quality_milli,
            sample_count: observation.sample_count,
            artifact: observation.artifact.clone(),
            negative_evidence: observation.negative_evidence.clone(),
        });
    }
    let omitted = admitted
        .iter()
        .filter(|action_id| !observed.contains(*action_id))
        .cloned()
        .collect::<BTreeSet<_>>();
    if request.require_complete_frontier && !omitted.is_empty() {
        unresolved_actions.extend(omitted.iter().cloned());
        uncertainty.extend(
            omitted
                .iter()
                .map(|action_id| format!("{action_id}:frontier-observation-missing")),
        );
    }
    let synthesis = if evidence.is_empty() {
        None
    } else {
        evidence.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
        Some(synthesize_glioma_interpretation(
            &InterpretationSynthesisRequest {
                objective: request.policy.objective.clone(),
                hypothesis: request.policy.hypothesis.clone(),
                model_system: request.policy.model_system,
                min_evidence: request.policy.min_evidence,
                min_independent_groups: request.policy.min_independent_groups,
                min_families: request.policy.min_families,
                min_quality_milli: request.policy.min_quality_milli,
                effect_threshold_milli: request.policy.effect_threshold_milli,
                max_disagreement_milli: request.policy.max_disagreement_milli,
                max_leave_one_out_shift_milli: request.policy.max_leave_one_out_shift_milli,
                require_replication_family: request.policy.require_replication_family,
                replay_identity: request.policy.replay_identity.clone(),
                evidence,
            },
        )?)
    };
    let mut next_action = BTreeSet::new();
    next_action.extend(
        omitted
            .iter()
            .map(|action_id| format!("collect-observation:{action_id}")),
    );
    next_action.extend(
        negative_actions
            .iter()
            .map(|action_id| format!("replicate-or-falsify:{action_id}")),
    );
    next_action.extend(
        uncertainty
            .iter()
            .map(|item| format!("resolve-uncertainty:{item}")),
    );
    let mut negative_evidence = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence.sort();
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty.sort();
    let mut output = ComputationInterpretationEvidenceGate {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.policy.objective.clone(),
        hypothesis: request.policy.hypothesis.clone(),
        replay_identity: request.policy.replay_identity.clone(),
        frontier_digest: request.frontier.digest.clone(),
        admitted_action_order: admitted,
        observed_action_order: observed.into_iter().collect(),
        omitted_action_order: omitted.into_iter().collect(),
        negative_action_order: negative_actions.into_iter().collect(),
        unresolved_action_order: unresolved_actions.into_iter().collect(),
        next_action_order: next_action.into_iter().collect(),
        synthesis,
        negative_evidence,
        uncertainty,
        disposition: ComputationInterpretationEvidenceGateDisposition::Unresolved,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-interpretation-evidence"),
    };
    output.disposition = disposition_for(
        &request.frontier,
        output.synthesis.as_ref(),
        &output.omitted_action_order,
        &output.unresolved_action_order,
    );
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationInterpretationEvidenceGateError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routed_stage_keeps_computation_and_replication_families_distinct() {
        assert_eq!(
            family_for(GliomaStageKind::StatisticalInterpretation),
            InterpretationEvidenceFamily::Computation
        );
        assert_eq!(
            family_for(GliomaStageKind::ReplicationRobustness),
            InterpretationEvidenceFamily::Replication
        );
        assert_eq!(
            family_for(GliomaStageKind::ComputationalExecution),
            InterpretationEvidenceFamily::Computation
        );
    }
}
