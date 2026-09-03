//! Runtime catalog for the glioma program portfolio.
//!
//! The catalog is deliberately smaller than the separate 80,896-entry AURORA Feature Atlas.  It
//! is the executable product slice: twelve glioma verticals, each expanded into the eight
//! capability archetypes at four operating scales.  Every generated entry names a consumer,
//! observable behavior, artifact, and acceptance gate so it cannot collapse into a research
//! question or an implementation task.

use crate::glioma_engine::GliomaStageKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const GLIOMA_PROGRAM_COUNT: usize = 12;
pub const GLIOMA_FEATURES_PER_PROGRAM: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaProgramId {
    EvidenceSurveillance,
    EvidenceKnowledge,
    MultimodalIngestionQc,
    DecisionContext,
    MechanismExploration,
    ExperimentDesign,
    ProtocolSimulation,
    InstrumentRobotics,
    ReproducibleComputation,
    InterpretationReplication,
    ResearchObjectRelease,
    FederatedBenchmarking,
}

impl GliomaProgramId {
    pub const ALL: [Self; GLIOMA_PROGRAM_COUNT] = [
        Self::EvidenceSurveillance,
        Self::EvidenceKnowledge,
        Self::MultimodalIngestionQc,
        Self::DecisionContext,
        Self::MechanismExploration,
        Self::ExperimentDesign,
        Self::ProtocolSimulation,
        Self::InstrumentRobotics,
        Self::ReproducibleComputation,
        Self::InterpretationReplication,
        Self::ResearchObjectRelease,
        Self::FederatedBenchmarking,
    ];

    pub const fn ordinal(self) -> u8 {
        match self {
            Self::EvidenceSurveillance => 1,
            Self::EvidenceKnowledge => 2,
            Self::MultimodalIngestionQc => 3,
            Self::DecisionContext => 4,
            Self::MechanismExploration => 5,
            Self::ExperimentDesign => 6,
            Self::ProtocolSimulation => 7,
            Self::InstrumentRobotics => 8,
            Self::ReproducibleComputation => 9,
            Self::InterpretationReplication => 10,
            Self::ResearchObjectRelease => 11,
            Self::FederatedBenchmarking => 12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaFeatureArchetype {
    ScientificAlgorithm,
    TypedDataPrimitive,
    AgentAutomation,
    WorkflowOrchestration,
    ResearcherInteraction,
    ApiProtocolIntegration,
    VerificationSafety,
    OperationsFederation,
}

impl GliomaFeatureArchetype {
    pub const ALL: [Self; 8] = [
        Self::ScientificAlgorithm,
        Self::TypedDataPrimitive,
        Self::AgentAutomation,
        Self::WorkflowOrchestration,
        Self::ResearcherInteraction,
        Self::ApiProtocolIntegration,
        Self::VerificationSafety,
        Self::OperationsFederation,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::ScientificAlgorithm => "scientific algorithm",
            Self::TypedDataPrimitive => "typed data primitive",
            Self::AgentAutomation => "agent automation",
            Self::WorkflowOrchestration => "workflow orchestration",
            Self::ResearcherInteraction => "researcher interaction",
            Self::ApiProtocolIntegration => "API/protocol integration",
            Self::VerificationSafety => "verification/safety system",
            Self::OperationsFederation => "operations/federation capability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaOperatingScale {
    LocalSingleStudy,
    MultimodalMultiStudy,
    ProspectiveHighThroughput,
    FederatedContinual,
}

impl GliomaOperatingScale {
    pub const ALL: [Self; 4] = [
        Self::LocalSingleStudy,
        Self::MultimodalMultiStudy,
        Self::ProspectiveHighThroughput,
        Self::FederatedContinual,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::LocalSingleStudy => "local single-study",
            Self::MultimodalMultiStudy => "multimodal multi-study",
            Self::ProspectiveHighThroughput => "prospective high-throughput",
            Self::FederatedContinual => "federated continual autonomous",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GliomaProgramDescriptor {
    pub program_id: GliomaProgramId,
    pub title: &'static str,
    pub slug: &'static str,
    pub folder: &'static str,
    pub consumer: &'static str,
    pub observable_outcome: &'static str,
    pub artifact: &'static str,
    pub surface: &'static str,
    pub stages: &'static [GliomaStageKind],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFeatureSpec {
    pub feature_id: String,
    pub program_id: GliomaProgramId,
    pub program_slug: String,
    pub archetype: GliomaFeatureArchetype,
    pub scale: GliomaOperatingScale,
    pub consumer: String,
    pub behavior: String,
    pub artifact: String,
    pub surface: String,
    pub acceptance_gate: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CatalogError {
    #[error("catalog has {actual} programs; expected {expected}")]
    ProgramCount { actual: usize, expected: usize },
    #[error("catalog has {actual} features; expected {expected}")]
    FeatureCount { actual: usize, expected: usize },
    #[error("catalog contains a duplicate feature id: {0}")]
    DuplicateFeature(String),
    #[error("catalog feature is incomplete: {0}")]
    IncompleteFeature(String),
}

const EVIDENCE: &[GliomaStageKind] = &[GliomaStageKind::EvidenceSurveillance];
const KNOWLEDGE: &[GliomaStageKind] = &[GliomaStageKind::EvidenceCompilation];
const MULTIMODAL: &[GliomaStageKind] = &[GliomaStageKind::MultimodalIngestionQc];
const DECISION: &[GliomaStageKind] = &[
    GliomaStageKind::IntentNormalization,
    GliomaStageKind::EvidenceCompilation,
];
const MECHANISM: &[GliomaStageKind] = &[
    GliomaStageKind::MolecularLandscape,
    GliomaStageKind::MechanismExploration,
];
const EXPERIMENT: &[GliomaStageKind] = &[GliomaStageKind::ExperimentDesign];
const SIMULATION: &[GliomaStageKind] = &[GliomaStageKind::ProtocolSimulation];
const INSTRUMENT: &[GliomaStageKind] = &[GliomaStageKind::InstrumentPreflight];
const COMPUTATION: &[GliomaStageKind] = &[GliomaStageKind::ComputationalExecution];
const INTERPRETATION: &[GliomaStageKind] = &[
    GliomaStageKind::StatisticalInterpretation,
    GliomaStageKind::ReplicationRobustness,
];
const RELEASE: &[GliomaStageKind] = &[GliomaStageKind::ResearchObjectRelease];
const FEDERATION: &[GliomaStageKind] = &[GliomaStageKind::FederationBenchmarking];

pub fn glioma_program_catalog() -> Vec<GliomaProgramDescriptor> {
    vec![
        descriptor(GliomaProgramId::EvidenceSurveillance, "Evidence surveillance", "evidence-surveillance", "p01-evidence-surveillance", "evidence curator", "qualified source candidates with stale, unknown, contradictory, and negative states", "QualifiedEvidenceSet", "local workbench, MCP, Rust/Python/TypeScript SDK", EVIDENCE),
        descriptor(GliomaProgramId::EvidenceKnowledge, "Evidence-to-typed-knowledge", "evidence-knowledge", "p02-evidence-knowledge", "knowledge engineer", "scoped claims and competing explanations bound to source artifacts", "TypedKnowledgeWorld", "knowledge API and research workbench", KNOWLEDGE),
        descriptor(GliomaProgramId::MultimodalIngestionQc, "Multimodal ingestion and QC", "multimodal-ingestion-qc", "p03-multimodal-ingestion-qc", "data steward", "comparable study-by-modality cells with explicit missingness and semantic defects", "HarmonizedGliomaObject", "local adapter, QC console, SDK", MULTIMODAL),
        descriptor(GliomaProgramId::DecisionContext, "Question-to-decision context", "decision-context", "p04-decision-context", "principal investigator", "bounded decision context with required inputs and unresolved omissions", "DecisionContext", "research workbench and MCP", DECISION),
        descriptor(GliomaProgramId::MechanismExploration, "Mechanism exploration", "mechanism-exploration", "p05-mechanism-exploration", "mechanism scientist", "ranked competing mechanism portfolio and discriminating next actions", "MechanismPortfolio", "analysis API and workbench", MECHANISM),
        descriptor(GliomaProgramId::ExperimentDesign, "Power-aware experiment design", "experiment-design", "p06-experiment-design", "experimentalist", "falsifiable allocation, power, blocking, dose-response fitting, and null-result release plan", "ExecutableExperimentDesign and DoseResponseAnalysis", "design workbench and SDK", EXPERIMENT),
        descriptor(GliomaProgramId::ProtocolSimulation, "Protocol simulation", "protocol-simulation", "p07-protocol-simulation", "lab operations lead", "resource-feasible protocol branches and compensation plan before physical effects", "ProtocolSimulationReport", "simulation service and operator UI", SIMULATION),
        descriptor(GliomaProgramId::InstrumentRobotics, "Instrument and robotics preflight", "instrument-robotics", "p08-instrument-robotics", "instrument operator", "signed, interlocked, human-authorized instrument action plan", "InstrumentPreflight", "instrument gateway and operator console", INSTRUMENT),
        descriptor(GliomaProgramId::ReproducibleComputation, "Reproducible computation", "reproducible-computation", "p09-reproducible-computation", "computational scientist", "checkpointed, replayable multimodal computation with bounded resources plus omission-stress robustness", "ComputationRun and RobustnessSuite", "workflow API, SDK, and CLI", COMPUTATION),
        descriptor(GliomaProgramId::InterpretationReplication, "Causal interpretation and replication", "interpretation-replication", "p10-interpretation-replication", "methods reviewer", "uncertainty-aware effects, contradiction analysis, replication verdicts, and negative results", "AnalysisReplicationRecord", "analysis workbench and evaluation API", INTERPRETATION),
        descriptor(GliomaProgramId::ResearchObjectRelease, "Research-object release", "research-object-release", "p11-research-object-release", "reproducibility steward", "portable research object with methods, limitations, replay metadata, and release gates", "SignedResearchObject", "release CLI, API, and registry", RELEASE),
        descriptor(GliomaProgramId::FederatedBenchmarking, "Federated benchmarking and governance", "federated-benchmarking", "p12-federated-benchmarking", "consortium administrator", "aggregate-only cross-site benchmark with policy, quorum, and localization evidence", "FederationBenchmark", "local control plane and federation API", FEDERATION),
    ]
}

#[allow(clippy::too_many_arguments)]
fn descriptor(
    program_id: GliomaProgramId,
    title: &'static str,
    slug: &'static str,
    folder: &'static str,
    consumer: &'static str,
    observable_outcome: &'static str,
    artifact: &'static str,
    surface: &'static str,
    stages: &'static [GliomaStageKind],
) -> GliomaProgramDescriptor {
    GliomaProgramDescriptor {
        program_id,
        title,
        slug,
        folder,
        consumer,
        observable_outcome,
        artifact,
        surface,
        stages,
    }
}

pub fn generate_feature_catalog() -> Vec<GliomaFeatureSpec> {
    glioma_program_catalog()
        .into_iter()
        .flat_map(|program| {
            GliomaFeatureArchetype::ALL.into_iter().enumerate().flat_map(move |(archetype_index, archetype)| {
                GliomaOperatingScale::ALL.into_iter().enumerate().map(move |(scale_index, scale)| {
                    let slot = archetype_index * GliomaOperatingScale::ALL.len() + scale_index + 1;
                    let feature_id = format!("GAF-GLIOMA-P{:02}-F{:02}", program.program_id.ordinal(), slot);
                    let behavior = format!("{} {} at {} scale; emits {} for the named {} consumer", program.title, archetype.label(), scale.label(), program.observable_outcome, program.consumer);
                    let acceptance_gate = format!("{} is independently replayable, preserves unknown and negative states, and meets the {} gate", feature_id, program.observable_outcome);
                    GliomaFeatureSpec {
                        feature_id,
                        program_id: program.program_id,
                        program_slug: program.slug.into(),
                        archetype,
                        scale,
                        consumer: program.consumer.into(),
                        behavior,
                        artifact: program.artifact.into(),
                        surface: program.surface.into(),
                        acceptance_gate,
                    }
                })
            })
        })
        .collect()
}

pub fn validate_feature_catalog(features: &[GliomaFeatureSpec]) -> Result<(), CatalogError> {
    let programs = glioma_program_catalog();
    if programs.len() != GLIOMA_PROGRAM_COUNT {
        return Err(CatalogError::ProgramCount {
            actual: programs.len(),
            expected: GLIOMA_PROGRAM_COUNT,
        });
    }
    let expected = GLIOMA_PROGRAM_COUNT * GLIOMA_FEATURES_PER_PROGRAM;
    if features.len() != expected {
        return Err(CatalogError::FeatureCount {
            actual: features.len(),
            expected,
        });
    }
    let mut ids = BTreeSet::new();
    for feature in features {
        if !ids.insert(feature.feature_id.clone()) {
            return Err(CatalogError::DuplicateFeature(feature.feature_id.clone()));
        }
        if feature.consumer.trim().is_empty()
            || feature.behavior.trim().is_empty()
            || feature.artifact.trim().is_empty()
            || feature.surface.trim().is_empty()
            || feature.acceptance_gate.trim().is_empty()
        {
            return Err(CatalogError::IncompleteFeature(feature.feature_id.clone()));
        }
    }
    Ok(())
}

/// Stable identifiers for executable glioma capabilities implemented in the research crate.
///
/// The generated 384-entry portfolio is intentionally broader than the currently implemented
/// slice.  Keeping this manifest next to the catalog lets release checks prove that concrete
/// algorithms do not accidentally claim the same product slot (a particularly easy mistake when
/// several analyses live in one program folder).
pub fn implemented_feature_ids() -> Vec<&'static str> {
    vec![
        crate::glioma::analysis::FEATURE_ID,
        crate::glioma::evidence::FEATURE_ID,
        crate::glioma::experiment::FEATURE_ID,
        crate::glioma::mechanism::FEATURE_ID,
        crate::glioma::multimodal::FEATURE_ID,
        crate::glioma::workflow::FEATURE_ID,
        crate::glioma::release::FEATURE_ID,
        crate::glioma::replication::FEATURE_ID,
        crate::glioma::programs::p01_evidence_surveillance::surveillance::FEATURE_ID,
        crate::glioma::programs::p01_evidence_surveillance::priority::FEATURE_ID,
        crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::FEATURE_ID,
        crate::glioma::programs::p02_evidence_knowledge::claim_frontier::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::concordance::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::consensus::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::harmonization::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::latent_factors::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::spatial_niche::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::spatial_communication::FEATURE_ID,
        crate::glioma::programs::p03_multimodal_ingestion_qc::spatial_propagation::FEATURE_ID,
        crate::glioma::programs::p04_decision_context::context_compiler::FEATURE_ID,
        crate::glioma::programs::p04_decision_context::action_bridge::FEATURE_ID,
        crate::glioma::programs::p05_mechanism_exploration::discrimination::FEATURE_ID,
        crate::glioma::programs::p05_mechanism_exploration::graph_propagation::FEATURE_ID,
        crate::glioma::programs::p05_mechanism_exploration::counterfactual::FEATURE_ID,
        crate::glioma::programs::p05_mechanism_exploration::ensemble_counterfactual::FEATURE_ID,
        crate::glioma::programs::p05_mechanism_exploration::robust_portfolio::FEATURE_ID,
        crate::glioma::programs::p05_mechanism_exploration::action_planner::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::adaptive_allocation::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::campaign::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::dose_response::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::synergy::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::information_design::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::adaptive_information_campaign::FEATURE_ID,
        crate::glioma::programs::p06_experiment_design::multi_fidelity::FEATURE_ID,
        crate::glioma::programs::p07_protocol_simulation::simulator::FEATURE_ID,
        crate::glioma::programs::p07_protocol_simulation::execution::FEATURE_ID,
        crate::glioma::programs::p07_protocol_simulation::action_execution::FEATURE_ID,
        crate::glioma::programs::p07_protocol_simulation::autonomous_campaign::FEATURE_ID,
        crate::glioma::programs::p07_protocol_simulation::research_autopilot::FEATURE_ID,
        crate::glioma::programs::p07_protocol_simulation::evidence_campaign::FEATURE_ID,
        crate::glioma::programs::p08_instrument_robotics::calibration::FEATURE_ID,
        crate::glioma::programs::p08_instrument_robotics::preflight::FEATURE_ID,
        crate::glioma::programs::p09_reproducible_computation::robustness::FEATURE_ID,
        crate::glioma::programs::p09_reproducible_computation::execution::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::causal_adjustment::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::causal_contrast::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::meta_analysis::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::sensitivity::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::mediation::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::state_transition::FEATURE_ID,
        crate::glioma::programs::p10_interpretation_replication::trajectory::FEATURE_ID,
        crate::glioma::programs::p12_federated_benchmarking::consensus::FEATURE_ID,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_expands_every_glioma_program_into_thirty_two_product_features() {
        let features = generate_feature_catalog();
        validate_feature_catalog(&features).unwrap();
        assert_eq!(features.len(), 384);
        for program in GliomaProgramId::ALL {
            assert_eq!(
                features
                    .iter()
                    .filter(|feature| feature.program_id == program)
                    .count(),
                32
            );
        }
    }

    #[test]
    fn feature_ids_are_stable_and_not_blueprint_coverage_ids() {
        let first = generate_feature_catalog();
        let second = generate_feature_catalog();
        assert_eq!(first, second);
        assert!(first
            .iter()
            .all(|feature| feature.feature_id.starts_with("GAF-GLIOMA-P")));
        assert!(first
            .iter()
            .all(|feature| !feature.feature_id.contains('.')));
    }

    #[test]
    fn implemented_feature_ids_are_unique_and_portfolio_backed() {
        let ids = implemented_feature_ids();
        let unique = ids.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), unique.len());
        let portfolio = generate_feature_catalog()
            .into_iter()
            .map(|feature| feature.feature_id)
            .collect::<BTreeSet<_>>();
        assert!(ids.iter().all(|id| portfolio.contains(*id)));
    }
}
