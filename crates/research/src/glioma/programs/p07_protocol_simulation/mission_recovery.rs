//! Failure-aware recovery for autonomous preclinical glioma missions.
//!
//! A mission that stops on an executor failure or partial result must not simply replay the same
//! action set.  This controller extracts the failed frontier, closes its downstream dependency
//! cone, and launches a bounded recovery mission over the still-runnable alternatives.  Initial
//! and recovery missions remain separate immutable records, so a negative or failed experiment is
//! never erased by an apparently successful retry.

use super::action_execution::GliomaActionExecutor;
use super::mission::{
    execute_glioma_autonomous_research_mission, GliomaAutonomousResearchMission,
    GliomaMissionDisposition, GliomaMissionError, GliomaMissionRequest,
};
use crate::glioma_engine::GliomaActionCandidate;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchMissionRecovery1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMissionRecoveryRequest {
    pub initial: GliomaMissionRequest,
    pub recovery_budget_units: u32,
    pub recovery_max_rounds: u16,
    pub require_clean_qualification: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMissionRecoveryDisposition {
    NoRecoveryNeeded,
    Recovered,
    Partial,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMissionRecoveryStopReason {
    InitialQualified,
    RecoveryQualified,
    RecoveryPartial,
    RecoveryBlocked,
    NoRecoverableCandidates,
    RecoveryBudgetExhausted,
    RecoveryRoundsExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMissionRecovery {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub initial: GliomaAutonomousResearchMission,
    pub recovery: Option<GliomaAutonomousResearchMission>,
    pub alternate_candidate_order: Vec<String>,
    pub invalidated_action_order: Vec<String>,
    pub budget_spent_units: u32,
    pub remaining_recovery_budget_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaMissionRecoveryDisposition,
    pub stop_reason: GliomaMissionRecoveryStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaMissionRecoveryError {
    #[error("glioma mission recovery request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma mission failed: {0}")]
    Mission(#[from] GliomaMissionError),
    #[error("glioma mission recovery output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma mission recovery digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn merge_sorted(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .chain(right.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn digest_input(output: &GliomaMissionRecovery) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "initial": output.initial,
        "recovery": output.recovery,
        "alternate_candidate_order": output.alternate_candidate_order,
        "invalidated_action_order": output.invalidated_action_order,
        "budget_spent_units": output.budget_spent_units,
        "remaining_recovery_budget_units": output.remaining_recovery_budget_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

fn invalidated_frontier(mission: &GliomaAutonomousResearchMission) -> BTreeSet<String> {
    let mut roots = mission
        .failed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for round in &mission.rounds {
        roots.extend(round.execution.partial_order.iter().cloned());
        roots.extend(round.execution.skipped_order.iter().cloned());
    }
    roots
}

fn close_invalidated_dependencies(
    candidates: &[GliomaActionCandidate],
    mut invalidated: BTreeSet<String>,
) -> BTreeSet<String> {
    let mut changed = true;
    while changed {
        changed = false;
        for candidate in candidates {
            if !invalidated.contains(&candidate.action_id)
                && candidate
                    .depends_on
                    .iter()
                    .any(|dependency| invalidated.contains(dependency))
            {
                invalidated.insert(candidate.action_id.clone());
                changed = true;
            }
        }
    }
    invalidated
}

fn recovery_candidates(
    request: &GliomaMissionRecoveryRequest,
    initial: &GliomaAutonomousResearchMission,
) -> (Vec<GliomaActionCandidate>, Vec<String>) {
    let completed = initial
        .completed_action_order
        .iter()
        .chain(initial.negative_action_order.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let invalidated =
        close_invalidated_dependencies(&request.initial.candidates, invalidated_frontier(initial));
    let mut candidates = request
        .initial
        .candidates
        .iter()
        .filter(|candidate| {
            !completed.contains(&candidate.action_id) && !invalidated.contains(&candidate.action_id)
        })
        .filter(|candidate| {
            candidate
                .depends_on
                .iter()
                .all(|dependency| !invalidated.contains(dependency))
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    (candidates, invalidated.into_iter().collect())
}

fn recovery_disposition(
    mission: Option<&GliomaAutonomousResearchMission>,
    require_clean_qualification: bool,
) -> (
    GliomaMissionRecoveryDisposition,
    GliomaMissionRecoveryStopReason,
) {
    match mission {
        Some(mission) if mission.disposition == GliomaMissionDisposition::Qualified => (
            GliomaMissionRecoveryDisposition::Recovered,
            GliomaMissionRecoveryStopReason::RecoveryQualified,
        ),
        Some(mission) if mission.disposition == GliomaMissionDisposition::Blocked => (
            GliomaMissionRecoveryDisposition::Blocked,
            GliomaMissionRecoveryStopReason::RecoveryBlocked,
        ),
        Some(_) if require_clean_qualification => (
            GliomaMissionRecoveryDisposition::Failed,
            GliomaMissionRecoveryStopReason::RecoveryPartial,
        ),
        Some(_) => (
            GliomaMissionRecoveryDisposition::Partial,
            GliomaMissionRecoveryStopReason::RecoveryPartial,
        ),
        None => (
            GliomaMissionRecoveryDisposition::Blocked,
            GliomaMissionRecoveryStopReason::NoRecoverableCandidates,
        ),
    }
}

/// Execute an initial mission and, only when necessary, a dependency-safe recovery mission.
pub fn execute_glioma_mission_recovery<E: GliomaActionExecutor>(
    request: &GliomaMissionRecoveryRequest,
    executor: &mut E,
) -> Result<GliomaMissionRecovery, GliomaMissionRecoveryError> {
    if request.recovery_budget_units == 0 || request.recovery_max_rounds == 0 {
        return Err(GliomaMissionRecoveryError::InvalidRequest(
            "recovery budget and recovery rounds must be positive".into(),
        ));
    }
    if request.initial.mission_id.trim().is_empty() {
        return Err(GliomaMissionRecoveryError::InvalidRequest(
            "initial mission id is required".into(),
        ));
    }
    let initial = execute_glioma_autonomous_research_mission(&request.initial, executor)?;
    let mut recovery = None;
    let (disposition, stop_reason) = if initial.disposition == GliomaMissionDisposition::Qualified {
        (
            GliomaMissionRecoveryDisposition::NoRecoveryNeeded,
            GliomaMissionRecoveryStopReason::InitialQualified,
        )
    } else {
        let (candidates, _) = recovery_candidates(request, &initial);
        if candidates.is_empty() {
            (
                GliomaMissionRecoveryDisposition::Blocked,
                GliomaMissionRecoveryStopReason::NoRecoverableCandidates,
            )
        } else {
            let completed = initial
                .completed_action_order
                .iter()
                .chain(initial.negative_action_order.iter())
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut completed_action_order = completed.into_iter().collect::<Vec<_>>();
            completed_action_order.sort();
            let mut recovery_request = request.initial.clone();
            recovery_request.mission_id = format!("{}:recovery", request.initial.mission_id);
            recovery_request.candidates = candidates;
            recovery_request.completed_action_order = completed_action_order;
            recovery_request.selection.budget_units = request.recovery_budget_units;
            recovery_request.max_rounds = request.recovery_max_rounds;
            recovery_request.stop_on_negative = false;
            let recovered =
                execute_glioma_autonomous_research_mission(&recovery_request, executor)?;
            let result =
                recovery_disposition(Some(&recovered), request.require_clean_qualification);
            recovery = Some(recovered);
            result
        }
    };
    let (alternate_candidates, invalidated) = recovery_candidates(request, &initial);
    let alternate_candidate_order = alternate_candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let recovery_spend = recovery
        .as_ref()
        .map(|mission| mission.budget_spent_units)
        .unwrap_or(0);
    let mut output = GliomaMissionRecovery {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: initial.mission_id.clone(),
        objective: initial.objective.clone(),
        initial: initial.clone(),
        recovery,
        alternate_candidate_order,
        invalidated_action_order: invalidated,
        budget_spent_units: initial.budget_spent_units.saturating_add(recovery_spend),
        remaining_recovery_budget_units: request
            .recovery_budget_units
            .saturating_sub(recovery_spend),
        negative_evidence: initial.negative_evidence.clone(),
        uncertainty: initial.uncertainty.clone(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mission-recovery"),
    };
    if let Some(recovery) = &output.recovery {
        output.negative_evidence =
            merge_sorted(&output.negative_evidence, &recovery.negative_evidence);
        output.uncertainty = merge_sorted(&output.uncertainty, &recovery.uncertainty);
    }
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaMissionRecoveryError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl GliomaMissionRecovery {
    pub fn validate(&self) -> Result<(), GliomaMissionRecoveryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.initial.mission_id != self.mission_id
            || self.initial.objective != self.objective
            || !canonical(&self.alternate_candidate_order)
            || !canonical(&self.invalidated_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.recovery.as_ref().is_some_and(|mission| {
                mission.mission_id != format!("{}:recovery", self.mission_id)
                    || mission.objective != self.objective
            })
        {
            return Err(GliomaMissionRecoveryError::InvalidOutput(
                "identity, recovery binding, ordering, or evidence fields are invalid".into(),
            ));
        }
        self.initial.validate()?;
        if let Some(recovery) = &self.recovery {
            recovery.validate()?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaMissionRecoveryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaMissionRecoveryError::InvalidOutput(
                "mission recovery digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::DryRunGliomaActionExecutor;
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaSelectionConfig, GliomaStageKind,
    };
    use bioprism_foundation::{AutonomyTier, Effect};
    use std::collections::BTreeSet;

    fn candidate(id: &str, information: u16) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: id.into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Computational,
            model_system: GliomaModelSystem::InSilico,
            depends_on: Vec::new(),
            cost_units: 1,
            information_gain_milli: information,
            frontier_novelty_milli: 900,
            workflow_leverage_milli: 900,
            cross_stage_unlock_milli: 900,
            reproducibility_safety_milli: 900,
            federation_value_milli: 100,
            feasibility_milli: 900,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    fn request() -> GliomaMissionRecoveryRequest {
        let failed = candidate("a-failed", 1_000);
        let mut dependent = candidate("b-dependent", 800);
        dependent.depends_on = vec![failed.action_id.clone()];
        let alternate = candidate("c-alternate", 950);
        GliomaMissionRecoveryRequest {
            initial: GliomaMissionRequest {
                mission_id: "recovery-test".into(),
                objective: "find a reproducible invasion mechanism".into(),
                candidates: vec![failed, dependent, alternate],
                completed_action_order: Vec::new(),
                selection: GliomaSelectionConfig {
                    budget_units: 2,
                    max_actions: 1,
                    ..GliomaSelectionConfig::default()
                },
                gates: Default::default(),
                max_rounds: 1,
                max_retries: 0,
                require_artifacts: true,
                stop_on_negative: false,
            },
            recovery_budget_units: 2,
            recovery_max_rounds: 1,
            require_clean_qualification: false,
        }
    }

    struct FailFirst {
        failed: bool,
    }

    impl GliomaActionExecutor for FailFirst {
        fn execute_action(
            &mut self,
            candidate: &GliomaActionCandidate,
            _attempt: u8,
        ) -> Result<
            super::super::action_execution::ActionExecutionResult,
            super::super::action_execution::ActionExecutionFailure,
        > {
            if candidate.action_id == "a-failed" && !self.failed {
                self.failed = true;
                return Err(super::super::action_execution::ActionExecutionFailure {
                    reason: "synthetic worker failure".into(),
                    retryable: false,
                });
            }
            let mut dry = DryRunGliomaActionExecutor;
            dry.execute_action(candidate, 1)
        }
    }

    #[test]
    fn recovery_closes_failed_dependency_cone_and_executes_alternate() {
        let mut executor = FailFirst { failed: false };
        let output = execute_glioma_mission_recovery(&request(), &mut executor).unwrap();
        assert_eq!(
            output.initial.disposition,
            GliomaMissionDisposition::Blocked
        );
        assert!(output
            .invalidated_action_order
            .contains(&"a-failed".to_string()));
        assert!(output
            .invalidated_action_order
            .contains(&"b-dependent".to_string()));
        assert_eq!(output.alternate_candidate_order, vec!["c-alternate"]);
        assert!(output.recovery.is_some());
        output.validate().unwrap();
    }

    #[test]
    fn clean_initial_mission_does_not_dispatch_recovery() {
        let mut request = request();
        request.initial.candidates = vec![candidate("c-alternate", 950)];
        request.initial.selection.budget_units = 1;
        request.initial.gates.min_information_gain_milli = 100;
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_mission_recovery(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            GliomaMissionRecoveryDisposition::NoRecoveryNeeded
        );
        assert!(output.recovery.is_none());
    }
}
