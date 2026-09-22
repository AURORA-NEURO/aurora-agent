//! Instrument-result to autonomous-research-frontier compilation.
//!
//! P08's science loop deliberately stops at assay adjudication.  This module turns that typed
//! result into the next executable glioma work: qualified assays become local computation,
//! unresolved assays become replication/QC work, and negative assays become falsification
//! experiments.  The compiler never changes an assay verdict and never treats hardware
//! completion as evidence.  Its candidates are fed into the existing P07 mission controller.

use super::assay_adjudication::InstrumentAssayEvidenceAssessment;
use super::science_loop::InstrumentScienceLoop;
use crate::glioma::programs::p07_protocol_simulation::{
    execute_glioma_autonomous_research_mission, GliomaActionExecutor,
    GliomaAutonomousResearchMission, GliomaMissionDisposition, GliomaMissionError,
    GliomaMissionGates, GliomaMissionRequest,
};
use crate::glioma_engine::{
    GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaSelectionConfig,
    GliomaStageKind,
};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentResearchFrontier1@1";
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentResearchFrontierRequest {
    pub mission_id: String,
    pub objective: String,
    pub science_loop: InstrumentScienceLoop,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub max_actions: usize,
    pub default_cost_units: u32,
    #[serde(default)]
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
    pub gates: GliomaMissionGates,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_negative: bool,
    pub allow_empty_frontier: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentResearchFrontier {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_loop_digest: ContentHash,
    pub action_order: Vec<String>,
    pub candidates: Vec<GliomaActionCandidate>,
    pub qualified_source_order: Vec<String>,
    pub negative_source_order: Vec<String>,
    pub unresolved_source_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentResearchFrontierDisposition {
    Executed,
    Held,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentResearchFrontierRun {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub frontier: InstrumentResearchFrontier,
    pub mission: Option<GliomaAutonomousResearchMission>,
    pub disposition: InstrumentResearchFrontierDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentResearchFrontierError {
    #[error("instrument research frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument research frontier input is invalid: {0}")]
    InvalidInput(String),
    #[error("instrument research frontier mission failed: {0}")]
    Mission(#[from] GliomaMissionError),
    #[error("instrument research frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument research frontier digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(frontier: &InstrumentResearchFrontier) -> serde_json::Value {
    serde_json::json!({
        "feature_id": frontier.feature_id,
        "output_schema": frontier.output_schema,
        "objective": frontier.objective,
        "source_loop_digest": frontier.source_loop_digest,
        "action_order": frontier.action_order,
        "candidates": frontier.candidates,
        "qualified_source_order": frontier.qualified_source_order,
        "negative_source_order": frontier.negative_source_order,
        "unresolved_source_order": frontier.unresolved_source_order,
        "negative_evidence": frontier.negative_evidence,
        "uncertainty": frontier.uncertainty,
    })
}

fn run_digest_input(run: &InstrumentResearchFrontierRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "mission_id": run.mission_id,
        "objective": run.objective,
        "frontier": run.frontier,
        "mission": run.mission,
        "disposition": run.disposition,
        "next_step": run.next_step,
    })
}

fn bounded(value: u64) -> u16 {
    value.min(1_000) as u16
}

fn source_id(run_id: &str, action_id: &str) -> String {
    format!("{run_id}:{action_id}")
}

fn candidate_id(run_id: &str, action_id: &str, stage: GliomaStageKind) -> String {
    format!(
        "instrument-frontier:{run_id}:{action_id}:{}",
        stage.stage_id()
    )
}

fn assessment_records(
    run_id: &str,
    assessment: &InstrumentAssayEvidenceAssessment,
    stage: GliomaStageKind,
    model_system: GliomaModelSystem,
    modality: GliomaModality,
    default_cost_units: u32,
) -> Vec<GliomaActionCandidate> {
    let qualified = assessment
        .qualified_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = assessment
        .negative_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let unresolved = assessment
        .unresolved_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    assessment
        .records
        .iter()
        .filter_map(|record| {
            let (target_stage, information, novelty, leverage) =
                if qualified.contains(&record.action_id) {
                    (
                        GliomaStageKind::ComputationalExecution,
                        bounded(record.absolute_effect_milli.max(250)),
                        700,
                        900,
                    )
                } else if negative.contains(&record.action_id) {
                    (GliomaStageKind::ExperimentDesign, 850, 900, 850)
                } else if unresolved.contains(&record.action_id) {
                    (GliomaStageKind::ReplicationRobustness, 900, 800, 850)
                } else {
                    return None;
                };
            let reproducibility = bounded(
                u64::from(record.qc_milli)
                    .saturating_mul(7)
                    .saturating_add(u64::from(record.replicate_count.min(3)) * 100)
                    .saturating_sub(record.uncertainty_milli.min(700)),
            );
            let cost = default_cost_units.max(1);
            Some(GliomaActionCandidate {
                action_id: candidate_id(run_id, &record.action_id, target_stage),
                stage_kind: target_stage,
                modality,
                model_system,
                depends_on: Vec::new(),
                cost_units: cost,
                information_gain_milli: information,
                frontier_novelty_milli: novelty,
                workflow_leverage_milli: leverage,
                cross_stage_unlock_milli: if target_stage == stage { 700 } else { 900 },
                reproducibility_safety_milli: reproducibility,
                federation_value_milli: reproducibility,
                feasibility_milli: 900,
                autonomy_tier: AutonomyTier::A1,
                effects: BTreeSet::from([
                    Effect::ReadLocalData,
                    Effect::ExecuteLocalComputation,
                    Effect::WriteLocalArtifact,
                ]),
            })
        })
        .collect()
}

fn validate_request(
    request: &InstrumentResearchFrontierRequest,
) -> Result<(), InstrumentResearchFrontierError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.default_cost_units == 0
        || request.selection.budget_units == 0
        || request.selection.max_actions == 0
        || request.max_rounds == 0
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(InstrumentResearchFrontierError::InvalidRequest(
            "mission identity, objective, bounded actions, positive costs/budget, rounds, and canonical completed actions are required".into(),
        ));
    }
    request
        .science_loop
        .validate()
        .map_err(|error| InstrumentResearchFrontierError::InvalidInput(error.to_string()))?;
    if request.objective != request.science_loop.objective {
        return Err(InstrumentResearchFrontierError::InvalidRequest(
            "frontier objective must match the validated instrument science-loop objective".into(),
        ));
    }
    Ok(())
}

impl InstrumentResearchFrontier {
    pub fn validate(&self) -> Result<(), InstrumentResearchFrontierError> {
        let ids = self
            .candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.source_loop_digest.as_str().len() != 64
            || self.action_order != ids
            || !canonical(&self.action_order)
            || self.candidates.len() > MAX_ACTIONS
            || !canonical(&self.qualified_source_order)
            || !canonical(&self.negative_source_order)
            || !canonical(&self.unresolved_source_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.candidates.iter().any(|candidate| {
                candidate.cost_units == 0
                    || candidate.autonomy_tier != AutonomyTier::A1
                    || candidate.effects
                        != BTreeSet::from([
                            Effect::ReadLocalData,
                            Effect::ExecuteLocalComputation,
                            Effect::WriteLocalArtifact,
                        ])
            })
        {
            return Err(InstrumentResearchFrontierError::InvalidOutput(
                "frontier identity, ordering, bounds, local-effect, or provenance invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentResearchFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentResearchFrontierError::InvalidOutput(
                "frontier digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

impl InstrumentResearchFrontierRun {
    pub fn validate(&self) -> Result<(), InstrumentResearchFrontierError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.frontier.objective != self.objective
            || matches!(
                self.disposition,
                InstrumentResearchFrontierDisposition::Held
            ) == self.mission.is_some()
            || self.next_step.trim().is_empty()
        {
            return Err(InstrumentResearchFrontierError::InvalidOutput(
                "run identity, frontier binding, mission presence, or next-step invariants are invalid".into(),
            ));
        }
        self.frontier.validate()?;
        if let Some(mission) = &self.mission {
            mission.validate()?;
            if mission.mission_id != self.mission_id || mission.objective != self.objective {
                return Err(InstrumentResearchFrontierError::InvalidOutput(
                    "nested mission identity does not match the instrument frontier".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&run_digest_input(self))
            .map_err(|error| InstrumentResearchFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentResearchFrontierError::InvalidOutput(
                "frontier run digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile adjudicated instrument results into an executable, status-aware frontier.
pub fn compile_glioma_instrument_research_frontier(
    request: &InstrumentResearchFrontierRequest,
) -> Result<InstrumentResearchFrontier, InstrumentResearchFrontierError> {
    validate_request(request)?;
    let mut candidates = BTreeMap::<String, GliomaActionCandidate>::new();
    let mut qualified_source = BTreeSet::new();
    let mut negative_source = BTreeSet::new();
    let mut unresolved_source = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (index, assessment) in request.science_loop.assessments.iter().enumerate() {
        let run_id = request
            .science_loop
            .assessment_run_order
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("assessment-{index:04}"));
        for action in &assessment.qualified_order {
            qualified_source.insert(source_id(&run_id, action));
        }
        for action in &assessment.negative_order {
            negative_source.insert(source_id(&run_id, action));
        }
        for action in &assessment.unresolved_order {
            unresolved_source.insert(source_id(&run_id, action));
        }
        negative_evidence.extend(assessment.negative_evidence.iter().cloned());
        uncertainty.extend(assessment.uncertainty.iter().cloned());
        for candidate in assessment_records(
            &run_id,
            assessment,
            GliomaStageKind::InstrumentPreflight,
            request.model_system,
            request.modality,
            request.default_cost_units,
        ) {
            candidates
                .entry(candidate.action_id.clone())
                .or_insert(candidate);
        }
    }
    let mut ranked = candidates.into_values().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let left_score = u64::from(left.information_gain_milli)
            .saturating_mul(u64::from(left.reproducibility_safety_milli.max(1)))
            .saturating_div(u64::from(left.cost_units));
        let right_score = u64::from(right.information_gain_milli)
            .saturating_mul(u64::from(right.reproducibility_safety_milli.max(1)))
            .saturating_div(u64::from(right.cost_units));
        right_score
            .cmp(&left_score)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    ranked.truncate(request.max_actions);
    ranked.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    if ranked.is_empty() && !request.allow_empty_frontier {
        return Err(InstrumentResearchFrontierError::InvalidInput(
            "instrument science loop produced no qualified, negative, or unresolved action to route".into(),
        ));
    }
    let action_order = ranked
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let mut frontier = InstrumentResearchFrontier {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_loop_digest: request.science_loop.digest.clone(),
        action_order,
        candidates: ranked,
        qualified_source_order: qualified_source.into_iter().collect(),
        negative_source_order: negative_source.into_iter().collect(),
        unresolved_source_order: unresolved_source.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-research-frontier"),
    };
    frontier.digest = ContentHash::of_value(&digest_input(&frontier))
        .map_err(|error| InstrumentResearchFrontierError::Digest(error.to_string()))?;
    frontier.validate()?;
    Ok(frontier)
}

/// Compile and execute the instrument-derived frontier through the P07 mission controller.
pub fn execute_glioma_instrument_research_frontier<E: GliomaActionExecutor>(
    request: &InstrumentResearchFrontierRequest,
    executor: &mut E,
) -> Result<InstrumentResearchFrontierRun, InstrumentResearchFrontierError> {
    let frontier = compile_glioma_instrument_research_frontier(request)?;
    if request
        .completed_action_order
        .iter()
        .any(|id| frontier.action_order.binary_search(id).is_err())
    {
        return Err(InstrumentResearchFrontierError::InvalidRequest(
            "completed action is not present in the compiled instrument frontier".into(),
        ));
    }
    if frontier.candidates.is_empty() {
        let mut output = InstrumentResearchFrontierRun {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            mission_id: request.mission_id.clone(),
            objective: request.objective.clone(),
            frontier,
            mission: None,
            disposition: InstrumentResearchFrontierDisposition::Held,
            next_step: "hold until the instrument science loop produces a routable qualified, negative, or unresolved action".into(),
            digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-research-frontier-run"),
        };
        output.digest = ContentHash::of_value(&run_digest_input(&output))
            .map_err(|error| InstrumentResearchFrontierError::Digest(error.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    let mission = execute_glioma_autonomous_research_mission(
        &GliomaMissionRequest {
            mission_id: request.mission_id.clone(),
            objective: request.objective.clone(),
            candidates: frontier.candidates.clone(),
            completed_action_order: request.completed_action_order.clone(),
            selection: request.selection.clone(),
            gates: request.gates.clone(),
            max_rounds: request.max_rounds,
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
            stop_on_negative: request.stop_on_negative,
        },
        executor,
    )?;
    let disposition = if mission.disposition == GliomaMissionDisposition::Blocked {
        InstrumentResearchFrontierDisposition::Blocked
    } else {
        InstrumentResearchFrontierDisposition::Executed
    };
    let next_step = if matches!(disposition, InstrumentResearchFrontierDisposition::Blocked) {
        "resolve the mission's failed, unsafe, or budget-blocked frontier before scheduling more work"
    } else {
        "inspect mission outcomes and feed new typed assay/computation evidence into the next science loop"
    };
    let mut output = InstrumentResearchFrontierRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        frontier,
        mission: Some(mission),
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-research-frontier-run"),
    };
    output.digest = ContentHash::of_value(&run_digest_input(&output))
        .map_err(|error| InstrumentResearchFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::assay_adjudication::{
        AssayEvidenceDisposition, AssayEvidenceRecord,
    };

    fn record(action_id: &str, eligible: bool, effect_milli: u64) -> AssayEvidenceRecord {
        AssayEvidenceRecord {
            action_id: action_id.into(),
            effect_milli: effect_milli as i64,
            absolute_effect_milli: effect_milli,
            qc_milli: 900,
            uncertainty_milli: 50,
            replicate_count: 3,
            negative_control_milli: Some(10),
            artifact: None,
            eligible,
            reason_order: if eligible {
                Vec::new()
            } else {
                vec!["gate".into()]
            },
        }
    }

    #[test]
    fn routes_qualified_negative_and_unresolved_assay_states() {
        let assessment = InstrumentAssayEvidenceAssessment {
            feature_id: "GAF-GLIOMA-P08-F20".into(),
            output_schema: "GliomaInstrumentAssayEvidence1@1".into(),
            objective: "map invasion".into(),
            instrument_id: "imager-1".into(),
            modality: GliomaModality::Imaging,
            execution_digest: ContentHash::of_bytes(b"execution"),
            action_order: vec!["negative".into(), "qualified".into(), "unresolved".into()],
            records: vec![
                record("negative", false, 50),
                record("qualified", true, 600),
                record("unresolved", false, 0),
            ],
            qualified_order: vec!["qualified".into()],
            negative_order: vec!["negative".into()],
            unresolved_order: vec!["unresolved".into()],
            next_action_order: vec!["repeat".into()],
            evidence_eligible: false,
            limitations: vec!["test".into()],
            negative_evidence: vec!["negative-signal".into()],
            uncertainty: vec!["unresolved-qc".into()],
            disposition: AssayEvidenceDisposition::Partial,
            digest: ContentHash::of_bytes(b"assessment"),
        };
        let candidates = assessment_records(
            "run-1",
            &assessment,
            GliomaStageKind::InstrumentPreflight,
            GliomaModelSystem::Organoid,
            GliomaModality::Imaging,
            2,
        );
        assert_eq!(candidates.len(), 3);
        assert!(candidates
            .iter()
            .any(|candidate| { candidate.stage_kind == GliomaStageKind::ComputationalExecution }));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.stage_kind == GliomaStageKind::ExperimentDesign));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.stage_kind == GliomaStageKind::ReplicationRobustness));
    }
}
