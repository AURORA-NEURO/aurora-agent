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
        descriptor(GliomaProgramId::ExperimentDesign, "Power-aware experiment design", "experiment-design", "p06-experiment-design", "experimentalist", "falsifiable allocation, power, blocking, and null-result release plan", "ExecutableExperimentDesign", "design workbench and SDK", EXPERIMENT),
        descriptor(GliomaProgramId::ProtocolSimulation, "Protocol simulation", "protocol-simulation", "p07-protocol-simulation", "lab operations lead", "resource-feasible protocol branches and compensation plan before physical effects", "ProtocolSimulationReport", "simulation service and operator UI", SIMULATION),
        descriptor(GliomaProgramId::InstrumentRobotics, "Instrument and robotics preflight", "instrument-robotics", "p08-instrument-robotics", "instrument operator", "signed, interlocked, human-authorized instrument action plan", "InstrumentPreflight", "instrument gateway and operator console", INSTRUMENT),
        descriptor(GliomaProgramId::ReproducibleComputation, "Reproducible computation", "reproducible-computation", "p09-reproducible-computation", "computational scientist", "checkpointed, replayable multimodal computation with bounded resources", "ComputationRun", "workflow API, SDK, and CLI", COMPUTATION),
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
}
