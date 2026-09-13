//! Program-level control for autonomous preclinical glioma research.
//!
//! P07 already supplies the bounded autonomous engine. This feature turns its cycle history into
//! a researcher-facing control surface: every stage gets a gate, active work is separated from
//! approval and dependency holds, progress is measurable, and the next operator action is
//! explicit. It does not reinterpret a completed action as a scientific conclusion and it never
//! bypasses the existing local executor, autonomy, or preclinical boundary.

use super::action_execution::GliomaActionExecutor;
use super::autonomous_engine::{
    execute_glioma_autonomous_research_engine, GliomaAutonomousResearchEngineDisposition,
    GliomaAutonomousResearchEngineError, GliomaAutonomousResearchEngineRequest,
    GliomaAutonomousResearchEngineRun,
};
use crate::glioma_engine::GliomaStageKind;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousProgramCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramGateStatus {
    Cleared,
    Active,
    Held,
    ApprovalRequired,
    Blocked,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramGate {
    pub stage_kind: GliomaStageKind,
    pub stage_id: String,
    pub status: ProgramGateStatus,
    pub action_order: Vec<String>,
    pub dependency_stage_order: Vec<String>,
    pub reason: String,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomousProgramCycleRequest {
    pub engine: GliomaAutonomousResearchEngineRequest,
    pub execution_mode: ProgramExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomousProgramCycleDisposition {
    Completed,
    Partial,
    Blocked,
    NoRunnableActions,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomousProgramCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub engine: GliomaAutonomousResearchEngineRun,
    pub gates: Vec<ProgramGate>,
    pub cleared_stage_order: Vec<String>,
    pub active_action_order: Vec<String>,
    pub held_stage_order: Vec<String>,
    pub approval_stage_order: Vec<String>,
    pub blocked_stage_order: Vec<String>,
    pub progress_milli: u16,
    pub next_operator_action: String,
    pub execution_mode: ProgramExecutionMode,
    pub simulation_only: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AutonomousProgramCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutonomousProgramCycleError {
    #[error("autonomous program-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("autonomous program-cycle engine failed: {0}")]
    Engine(#[from] GliomaAutonomousResearchEngineError),
    #[error("autonomous program-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("autonomous program-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AutonomousProgramCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "engine": output.engine,
        "gates": output.gates,
        "cleared_stage_order": output.cleared_stage_order,
        "active_action_order": output.active_action_order,
        "held_stage_order": output.held_stage_order,
        "approval_stage_order": output.approval_stage_order,
        "blocked_stage_order": output.blocked_stage_order,
        "progress_milli": output.progress_milli,
        "next_operator_action": output.next_operator_action,
        "execution_mode": output.execution_mode,
        "simulation_only": output.simulation_only,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn stage_dependencies(stage: GliomaStageKind) -> Vec<String> {
    let dependencies = match stage {
        GliomaStageKind::IntentNormalization => &[][..],
        GliomaStageKind::EvidenceSurveillance => &["intent-normalization"][..],
        GliomaStageKind::EvidenceCompilation => &["evidence-surveillance"][..],
        GliomaStageKind::MultimodalIngestionQc => &["intent-normalization"][..],
        GliomaStageKind::MolecularLandscape => &["multimodal-ingestion-qc"][..],
        GliomaStageKind::MechanismExploration => {
            &["evidence-compilation", "molecular-landscape"][..]
        }
        GliomaStageKind::ExperimentDesign => &["mechanism-exploration"][..],
        GliomaStageKind::ProtocolSimulation => &["experiment-design"][..],
        GliomaStageKind::InstrumentPreflight => &["protocol-simulation"][..],
        GliomaStageKind::ComputationalExecution => {
            &["multimodal-ingestion-qc", "protocol-simulation"][..]
        }
        GliomaStageKind::StatisticalInterpretation => &["computational-execution"][..],
        GliomaStageKind::ReplicationRobustness => &["statistical-interpretation"][..],
        GliomaStageKind::ResearchObjectRelease => &["replication-robustness"][..],
        GliomaStageKind::FederationBenchmarking => &["research-object-release"][..],
    };
    dependencies
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn stage_action_index(
    run: &GliomaAutonomousResearchEngineRun,
) -> std::collections::BTreeMap<GliomaStageKind, BTreeSet<String>> {
    let mut index = std::collections::BTreeMap::new();
    for cycle in &run.cycles {
        for action in &cycle.director.actions {
            index
                .entry(action.candidate.stage_kind)
                .or_insert_with(BTreeSet::new)
                .insert(action.candidate.action_id.clone());
        }
    }
    index
}

fn active_actions(run: &GliomaAutonomousResearchEngineRun) -> BTreeSet<String> {
    run.cycles
        .last()
        .map(|cycle| cycle.director.next_stage_order.iter().cloned().collect())
        .unwrap_or_else(|| run.pending_stage_order.iter().cloned().collect())
}

fn compile_gates(run: &GliomaAutonomousResearchEngineRun) -> Vec<ProgramGate> {
    let action_index = stage_action_index(run);
    let active = active_actions(run);
    GliomaStageKind::ALL
        .into_iter()
        .map(|stage| {
            let actions = action_index.get(&stage).cloned().unwrap_or_default();
            let status = if run
                .completed_stage_order
                .iter()
                .any(|value| value == stage.stage_id())
            {
                ProgramGateStatus::Cleared
            } else if run.blocked_order.iter().any(|value| {
                value == stage.stage_id()
                    || actions.iter().any(|action| value.contains(action.as_str()))
            }) {
                ProgramGateStatus::Blocked
            } else if run
                .approval_order
                .iter()
                .any(|value| value == stage.stage_id())
            {
                ProgramGateStatus::ApprovalRequired
            } else if run.hold_order.iter().any(|value| value == stage.stage_id()) {
                ProgramGateStatus::Held
            } else if actions.iter().any(|action| active.contains(action)) {
                ProgramGateStatus::Active
            } else {
                ProgramGateStatus::Pending
            };
            let reason = match status {
                ProgramGateStatus::Cleared => "stage checkpoint admitted by the engine".into(),
                ProgramGateStatus::Active => {
                    "stage has a selected action in the current frontier".into()
                }
                ProgramGateStatus::Held => {
                    "stage is waiting for an upstream evidence or quality condition".into()
                }
                ProgramGateStatus::ApprovalRequired => {
                    "stage requires explicit authority before dispatch".into()
                }
                ProgramGateStatus::Blocked => "stage or one of its dependencies is blocked".into(),
                ProgramGateStatus::Pending => {
                    "stage has not yet entered the executable frontier".into()
                }
            };
            ProgramGate {
                stage_kind: stage,
                stage_id: stage.stage_id().into(),
                status,
                action_order: actions.into_iter().collect(),
                dependency_stage_order: stage_dependencies(stage),
                reason,
                uncertainty: run.uncertainty.clone(),
            }
        })
        .collect()
}

fn next_operator_action(run: &GliomaAutonomousResearchEngineRun, gates: &[ProgramGate]) -> String {
    if !run.blocked_order.is_empty() {
        "resolve blocked dependencies or missing evidence before granting more autonomy".into()
    } else if !run.approval_order.is_empty() {
        "review and explicitly approve the held local actions".into()
    } else if !run.pending_stage_order.is_empty() {
        "inspect the selected local batch and continue the bounded engine cycle".into()
    } else if gates
        .iter()
        .all(|gate| gate.status == ProgramGateStatus::Cleared)
    {
        "review the completed preclinical program and prepare its reproducibility release".into()
    } else {
        "supply the next missing typed artifact or evidence record".into()
    }
}

fn disposition(
    value: GliomaAutonomousResearchEngineDisposition,
) -> AutonomousProgramCycleDisposition {
    match value {
        GliomaAutonomousResearchEngineDisposition::Completed => {
            AutonomousProgramCycleDisposition::Completed
        }
        GliomaAutonomousResearchEngineDisposition::Partial => {
            AutonomousProgramCycleDisposition::Partial
        }
        GliomaAutonomousResearchEngineDisposition::Blocked => {
            AutonomousProgramCycleDisposition::Blocked
        }
        GliomaAutonomousResearchEngineDisposition::NoRunnableActions => {
            AutonomousProgramCycleDisposition::NoRunnableActions
        }
        GliomaAutonomousResearchEngineDisposition::BudgetExhausted => {
            AutonomousProgramCycleDisposition::BudgetExhausted
        }
    }
}

/// Run the autonomous engine and compile a stage-gated program handoff.
pub fn execute_glioma_autonomous_program_cycle<E: GliomaActionExecutor>(
    request: &AutonomousProgramCycleRequest,
    executor: &mut E,
) -> Result<AutonomousProgramCycle, AutonomousProgramCycleError> {
    if request.engine.mission_id.trim().is_empty()
        || request.engine.intent.objective.trim().is_empty()
    {
        return Err(AutonomousProgramCycleError::InvalidRequest(
            "mission identity and research objective are required".into(),
        ));
    }
    let engine = execute_glioma_autonomous_research_engine(&request.engine, executor)?;
    let gates = compile_gates(&engine);
    let cleared_stage_order = gates
        .iter()
        .filter(|gate| gate.status == ProgramGateStatus::Cleared)
        .map(|gate| gate.stage_id.clone())
        .collect::<Vec<_>>();
    let active_action_order = active_actions(&engine).into_iter().collect::<Vec<_>>();
    let held_stage_order = gates
        .iter()
        .filter(|gate| gate.status == ProgramGateStatus::Held)
        .map(|gate| gate.stage_id.clone())
        .collect::<Vec<_>>();
    let approval_stage_order = gates
        .iter()
        .filter(|gate| gate.status == ProgramGateStatus::ApprovalRequired)
        .map(|gate| gate.stage_id.clone())
        .collect::<Vec<_>>();
    let blocked_stage_order = gates
        .iter()
        .filter(|gate| gate.status == ProgramGateStatus::Blocked)
        .map(|gate| gate.stage_id.clone())
        .collect::<Vec<_>>();
    let mut cleared_stage_order = cleared_stage_order;
    let mut active_action_order = active_action_order;
    let mut held_stage_order = held_stage_order;
    let mut approval_stage_order = approval_stage_order;
    let mut blocked_stage_order = blocked_stage_order;
    cleared_stage_order.sort();
    active_action_order.sort();
    held_stage_order.sort();
    approval_stage_order.sort();
    blocked_stage_order.sort();
    let progress_milli =
        ((cleared_stage_order.len() as u32 * 1_000) / GliomaStageKind::ALL.len() as u32) as u16;
    let next_operator_action = next_operator_action(&engine, &gates);
    let mut negative_evidence = engine.negative_evidence.clone();
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = engine.uncertainty.clone();
    uncertainty.sort();
    uncertainty.dedup();
    let engine_disposition = disposition(engine.disposition);
    let mut output = AutonomousProgramCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: engine.mission_id.clone(),
        objective: engine.objective.clone(),
        phase_order: vec![
            "autonomous_engine".into(),
            "stage_gate_compilation".into(),
            "operator_handoff".into(),
        ],
        engine,
        gates,
        cleared_stage_order,
        active_action_order,
        held_stage_order,
        approval_stage_order,
        blocked_stage_order,
        progress_milli,
        next_operator_action,
        simulation_only: matches!(
            request.execution_mode,
            ProgramExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        negative_evidence,
        uncertainty,
        disposition: engine_disposition,
        digest: ContentHash::of_bytes(b"unsealed-autonomous-program-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AutonomousProgramCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl AutonomousProgramCycle {
    pub fn validate(&self) -> Result<(), AutonomousProgramCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.engine.mission_id != self.mission_id
            || self.engine.objective != self.objective
            || self.phase_order
                != [
                    "autonomous_engine".to_string(),
                    "stage_gate_compilation".to_string(),
                    "operator_handoff".to_string(),
                ]
            || self.gates.len() != GliomaStageKind::ALL.len()
            || self.progress_milli > 1_000
            || !canonical(&self.cleared_stage_order)
            || !canonical(&self.active_action_order)
            || !canonical(&self.held_stage_order)
            || !canonical(&self.approval_stage_order)
            || !canonical(&self.blocked_stage_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.simulation_only
                != matches!(self.execution_mode, ProgramExecutionMode::LocalSimulation)
        {
            return Err(AutonomousProgramCycleError::InvalidOutput(
                "identity, engine binding, gate cardinality, progress, ordering, or execution mode is invalid".into(),
            ));
        }
        self.engine
            .validate()
            .map_err(|error| AutonomousProgramCycleError::InvalidOutput(error.to_string()))?;
        for (expected, gate) in GliomaStageKind::ALL.iter().zip(&self.gates) {
            if gate.stage_kind != *expected
                || gate.stage_id != expected.stage_id()
                || !canonical(&gate.action_order)
                || !canonical(&gate.dependency_stage_order)
                || !canonical(&gate.uncertainty)
                || gate.reason.trim().is_empty()
            {
                return Err(AutonomousProgramCycleError::InvalidOutput(
                    "stage gates are not canonical or do not match the stage graph".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AutonomousProgramCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AutonomousProgramCycleError::InvalidOutput(
                "autonomous program-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaResearchIntent, GliomaSelectionWeights,
        LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_ids::ContentHash;
    use bioprism_onco::OutputUse;
    use std::collections::BTreeSet;

    fn request() -> AutonomousProgramCycleRequest {
        let hash = ContentHash::of_bytes(b"program-cycle-input");
        AutonomousProgramCycleRequest {
            engine: GliomaAutonomousResearchEngineRequest {
                mission_id: "program-cycle-test".into(),
                intent: GliomaResearchIntent {
                    research_id: "program-cycle-research".into(),
                    study_id: "program-cycle-study".into(),
                    objective: "identify reproducible invasion mechanisms in organoids".into(),
                    output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
                    model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                    modalities: BTreeSet::from([
                        GliomaModality::Transcriptomics,
                        GliomaModality::Imaging,
                        GliomaModality::Spatial,
                    ]),
                    input_artifacts: vec![LocalArtifactRef {
                        artifact_id: "program-cycle-input".into(),
                        content_hash: hash.clone(),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    }],
                    requested_autonomy: AutonomyTier::A1,
                    approval_reference: None,
                    budget_units: 160,
                    max_retries: 1,
                    allow_instrument_execution: false,
                    allow_federation: false,
                    raw_data_local: true,
                    aggregate_only: true,
                    replay_identity: hash,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                focus: super::super::director::GliomaDirectorFocus::MechanismFirst,
                completed_checkpoints: Vec::new(),
                budget_units: 160,
                max_actions: 2,
                max_cycles: 8,
                approval_granted: false,
                allow_instrument_execution: false,
                allow_federation: false,
                selection_weights: GliomaSelectionWeights::default(),
                max_retries: 1,
                require_artifacts: true,
            },
            execution_mode: ProgramExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn program_cycle_compiles_all_stage_gates_and_replays() {
        let request = request();
        let mut first_executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let mut second_executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let first = execute_glioma_autonomous_program_cycle(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_autonomous_program_cycle(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.gates.len(), GliomaStageKind::ALL.len());
        assert!(first.simulation_only);
        assert!(first
            .gates
            .iter()
            .any(|gate| gate.status == ProgramGateStatus::Cleared));
        assert!(first.progress_milli > 0);
        first.validate().unwrap();
    }

    #[test]
    fn program_cycle_preserves_governed_local_mode() {
        let mut request = request();
        request.execution_mode = ProgramExecutionMode::GovernedLocal;
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_autonomous_program_cycle(&request, &mut executor).unwrap();
        assert!(!output.simulation_only);
        output.validate().unwrap();
    }
}
