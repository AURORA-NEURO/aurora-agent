//! A domain execution engine for preclinical glioma research programs.
//!
//! This is deliberately a workflow engine, not a conversational wrapper.  A validated intent is
//! compiled into a deterministic, 14-stage pipeline spanning evidence, multimodal quality,
//! molecular landscape, mechanism, experiment design, simulation, computation, replication and
//! research-object release.  The engine owns ordering, dependency gates, retries, checkpoints and
//! the preclinical boundary; a caller-owned [`GliomaStageExecutor`] owns the actual local
//! retrievers, analysis tools, simulators, instrument gateways and artifact store.
//!
//! The separation is important.  It makes this module useful in a notebook, a batch worker, an
//! MCP server or an institution-local instrument service without letting a planner invent an
//! observation or silently acquire authority.  A dry-run executor is included for integration
//! tests and demonstrations, but its outputs are explicitly simulated and never count as
//! biological evidence.
//!
//! The stage graph is informed by the multi-dimensional molecular view of glioblastoma reported
//! by TCGA and by the integrated molecular features used in CNS tumour classification.  Those
//! sources motivate the modality slots; they do not turn this research engine into a clinical
//! classifier.  The engine remains aggregate/preclinical only.

use bioprism_foundation::{
    ApprovalRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ExecutionEvent, ExecutionRun, ExecutionStatus, ResearchSurface,
    ResearchWorkflowSpec, SemanticLoss, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, RunId};
use bioprism_onco::{BoundaryRequest, ConsentBasis, OutputUse, RequestContext, ResearchBoundary};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-research-P01-F01";
pub const CONTRACT_VERSION: &str = "research-glioma-autonomous-engine/1.0";
pub const OUTPUT_SCHEMA: &str = "GliomaExecutionReceipt1@1";
pub const ACTION_SELECTION_OUTPUT_SCHEMA: &str = "GliomaActionSelection1@1";
pub const MAX_ARTIFACTS: usize = 4096;
pub const MAX_STAGES: usize = 32;

/// Data and assay families understood by the glioma workflow planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaModality {
    Literature,
    Histopathology,
    Genomics,
    Transcriptomics,
    Epigenomics,
    Proteomics,
    Imaging,
    SingleCell,
    Spatial,
    FunctionalPerturbation,
    OrganoidAssay,
    AnimalModel,
    Computational,
    Instrument,
    Replication,
}

impl GliomaModality {
    pub const fn is_molecular(self) -> bool {
        matches!(
            self,
            Self::Genomics
                | Self::Transcriptomics
                | Self::Epigenomics
                | Self::Proteomics
                | Self::SingleCell
                | Self::Spatial
        )
    }
}

/// Preclinical model systems.  No human-subject record is accepted as a model system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaModelSystem {
    CellLine,
    Organoid,
    PatientDerivedXenograft,
    MouseModel,
    ZebrafishModel,
    InSilico,
}

/// A local, value-only input reference.  Payload bytes remain in the institution's store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalArtifactRef {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub content_type: String,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
}

impl LocalArtifactRef {
    pub(crate) fn validate(&self) -> Result<(), GliomaEngineError> {
        if self.artifact_id.trim().is_empty()
            || self.content_type.trim().is_empty()
            || !self.local_only
            || self.contains_human_data
            || self.contains_direct_identifiers
            || self.content_hash.as_str().len() != 64
        {
            return Err(GliomaEngineError::InvalidIntent(
                "every input artifact must be local, de-identified, and content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// A bounded request to investigate a glioma research question over local preclinical inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaResearchIntent {
    pub research_id: String,
    pub study_id: String,
    pub objective: String,
    pub output_uses: BTreeSet<OutputUse>,
    pub model_systems: BTreeSet<GliomaModelSystem>,
    pub modalities: BTreeSet<GliomaModality>,
    pub input_artifacts: Vec<LocalArtifactRef>,
    pub requested_autonomy: AutonomyTier,
    pub approval_reference: Option<String>,
    pub budget_units: u32,
    pub max_retries: u8,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

/// The fixed stage vocabulary is intentionally closed: a provider cannot smuggle in an
/// unreviewed capability by inventing a stage identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaStageKind {
    IntentNormalization,
    EvidenceSurveillance,
    EvidenceCompilation,
    MultimodalIngestionQc,
    MolecularLandscape,
    MechanismExploration,
    ExperimentDesign,
    ProtocolSimulation,
    InstrumentPreflight,
    ComputationalExecution,
    StatisticalInterpretation,
    ReplicationRobustness,
    ResearchObjectRelease,
    FederationBenchmarking,
}

impl GliomaStageKind {
    pub const ALL: [Self; 14] = [
        Self::IntentNormalization,
        Self::EvidenceSurveillance,
        Self::EvidenceCompilation,
        Self::MultimodalIngestionQc,
        Self::MolecularLandscape,
        Self::MechanismExploration,
        Self::ExperimentDesign,
        Self::ProtocolSimulation,
        Self::InstrumentPreflight,
        Self::ComputationalExecution,
        Self::StatisticalInterpretation,
        Self::ReplicationRobustness,
        Self::ResearchObjectRelease,
        Self::FederationBenchmarking,
    ];

    pub const fn stage_id(self) -> &'static str {
        match self {
            Self::IntentNormalization => "intent-normalization",
            Self::EvidenceSurveillance => "evidence-surveillance",
            Self::EvidenceCompilation => "evidence-compilation",
            Self::MultimodalIngestionQc => "multimodal-ingestion-qc",
            Self::MolecularLandscape => "molecular-landscape",
            Self::MechanismExploration => "mechanism-exploration",
            Self::ExperimentDesign => "experiment-design",
            Self::ProtocolSimulation => "protocol-simulation",
            Self::InstrumentPreflight => "instrument-preflight",
            Self::ComputationalExecution => "computational-execution",
            Self::StatisticalInterpretation => "statistical-interpretation",
            Self::ReplicationRobustness => "replication-robustness",
            Self::ResearchObjectRelease => "research-object-release",
            Self::FederationBenchmarking => "federation-benchmarking",
        }
    }

    pub const fn output_schema(self) -> &'static str {
        match self {
            Self::IntentNormalization => "GliomaIntent1@1",
            Self::EvidenceSurveillance => "EvidenceCandidateSet1@1",
            Self::EvidenceCompilation => "TypedEvidenceContext1@1",
            Self::MultimodalIngestionQc => "HarmonizedGliomaObject1@1",
            Self::MolecularLandscape => "GliomaMolecularLandscape1@1",
            Self::MechanismExploration => "GliomaMechanismGraph1@1",
            Self::ExperimentDesign => "GliomaExperimentDesign1@1",
            Self::ProtocolSimulation => "GliomaProtocolSimulation1@1",
            Self::InstrumentPreflight => "GliomaInstrumentPreflight1@1",
            Self::ComputationalExecution => "GliomaComputationRun1@1",
            Self::StatisticalInterpretation => "GliomaAnalysisResult1@1",
            Self::ReplicationRobustness => "GliomaReplicationAssessment1@1",
            Self::ResearchObjectRelease => "GliomaResearchObject1@1",
            Self::FederationBenchmarking => "GliomaFederationBenchmark1@1",
        }
    }

    fn dependencies(self) -> &'static [&'static str] {
        match self {
            Self::IntentNormalization => &[],
            Self::EvidenceSurveillance => &["intent-normalization"],
            Self::EvidenceCompilation => &["evidence-surveillance"],
            Self::MultimodalIngestionQc => &["intent-normalization"],
            Self::MolecularLandscape => &["multimodal-ingestion-qc"],
            Self::MechanismExploration => &["evidence-compilation", "molecular-landscape"],
            Self::ExperimentDesign => &["mechanism-exploration"],
            Self::ProtocolSimulation => &["experiment-design"],
            Self::InstrumentPreflight => &["protocol-simulation"],
            Self::ComputationalExecution => &["multimodal-ingestion-qc", "protocol-simulation"],
            Self::StatisticalInterpretation => &["computational-execution"],
            Self::ReplicationRobustness => &["statistical-interpretation"],
            Self::ResearchObjectRelease => &["replication-robustness"],
            Self::FederationBenchmarking => &["research-object-release"],
        }
    }

    fn budget_units(self) -> u32 {
        match self {
            Self::IntentNormalization => 1,
            Self::EvidenceSurveillance => 8,
            Self::EvidenceCompilation => 8,
            Self::MultimodalIngestionQc => 12,
            Self::MolecularLandscape => 16,
            Self::MechanismExploration => 20,
            Self::ExperimentDesign => 16,
            Self::ProtocolSimulation => 12,
            Self::InstrumentPreflight => 8,
            Self::ComputationalExecution => 24,
            Self::StatisticalInterpretation => 20,
            Self::ReplicationRobustness => 16,
            Self::ResearchObjectRelease => 8,
            Self::FederationBenchmarking => 16,
        }
    }

    fn effects(self) -> BTreeSet<Effect> {
        let mut effects = BTreeSet::from([
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
        ]);
        if matches!(self, Self::InstrumentPreflight) {
            effects.insert(Effect::InstrumentExecution);
        }
        if matches!(self, Self::FederationBenchmarking) {
            effects.insert(Effect::FederationExport);
        }
        effects
    }
}

/// Why a stage is not currently runnable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageReadiness {
    Ready,
    MissingInput,
    ApprovalRequired,
    Disabled,
}

/// A typed stage in the compiled workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStage {
    pub stage_id: String,
    pub kind: GliomaStageKind,
    pub depends_on: Vec<String>,
    pub input_schemas: Vec<String>,
    pub output_schema: String,
    pub required: bool,
    pub readiness: StageReadiness,
    pub autonomy_tier: AutonomyTier,
    pub effects: BTreeSet<Effect>,
    pub budget_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaPlanDisposition {
    Admitted,
    NeedsInputs,
    ApprovalRequired,
    Blocked,
}

/// A validated, deterministic plan that can be handed to any local execution host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaResearchPlan {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub research_id: String,
    pub study_id: String,
    pub objective: String,
    pub workflow: ResearchWorkflowSpec,
    pub stages: Vec<GliomaStage>,
    pub stage_order: Vec<String>,
    pub ready_order: Vec<String>,
    pub missing_input_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub disabled_order: Vec<String>,
    pub disposition: GliomaPlanDisposition,
    pub omission_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub plan_digest: ContentHash,
    pub boundary: String,
}

impl GliomaResearchPlan {
    pub fn validate(&self) -> Result<(), GliomaEngineError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.stages.is_empty()
            || self.stage_order.len() != self.stages.len()
        {
            return Err(GliomaEngineError::InvalidPlan(
                "plan identity, boundary, objective, or stage graph is incomplete".into(),
            ));
        }
        self.workflow
            .validate()
            .map_err(|error| GliomaEngineError::InvalidPlan(error.to_string()))?;
        let ids = self
            .stages
            .iter()
            .map(|stage| stage.stage_id.clone())
            .collect::<BTreeSet<_>>();
        if ids.len() != self.stages.len()
            || self.stage_order.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(GliomaEngineError::InvalidPlan(
                "stage identifiers and canonical order do not reconcile".into(),
            ));
        }
        if self.stages.iter().any(|stage| {
            stage
                .depends_on
                .iter()
                .any(|dependency| !ids.contains(dependency))
        }) {
            return Err(GliomaEngineError::InvalidPlan(
                "stage dependency references an unknown stage".into(),
            ));
        }
        let expected = plan_digest_input(self);
        let expected_digest = ContentHash::of_value(&expected)
            .map_err(|error| GliomaEngineError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_digest {
            return Err(GliomaEngineError::InvalidPlan(
                "plan digest is not bound to its workflow and stage graph".into(),
            ));
        }
        if self.replay_identity.as_str().len() != 64 {
            return Err(GliomaEngineError::InvalidPlan(
                "replay identity is not a SHA-256 digest".into(),
            ));
        }
        Ok(())
    }
}

/// A stage invocation carries only hashes and metadata; payload bytes stay in local stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageInput {
    pub research_id: String,
    pub study_id: String,
    pub stage_id: String,
    pub kind: GliomaStageKind,
    pub upstream_artifacts: Vec<ContentHash>,
    pub source_artifacts: Vec<LocalArtifactRef>,
    pub replay_identity: ContentHash,
    pub attempt: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaStageDisposition {
    Completed,
    Negative,
    Partial,
    Blocked,
}

/// A provider result.  `Negative` is a valid research result; it is not a failed execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageOutput {
    pub artifact: TypedResearchArtifact,
    pub disposition: GliomaStageDisposition,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

impl GliomaStageOutput {
    fn validate(&self, stage: &GliomaStage) -> Result<(), GliomaEngineError> {
        self.artifact
            .validate_metadata()
            .map_err(|error| GliomaEngineError::Provider(error.to_string()))?;
        if self.artifact.content_type != stage.output_schema
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
            || self
                .negative_evidence
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err(GliomaEngineError::Provider(format!(
                "stage {} returned an invalid typed output",
                stage.stage_id
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaStageFailure {
    pub reason: String,
    pub retryable: bool,
}

/// The only effectful seam.  Implementations may invoke local software or an approved gateway;
/// this crate never opens a socket, touches a specimen or manufactures an observation.
pub trait GliomaStageExecutor {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageExecution {
    pub stage_id: String,
    pub attempts: u8,
    pub disposition: GliomaStageDisposition,
    pub artifact_digest: Option<ContentHash>,
    pub error: Option<String>,
}

/// The append-only, replayable outcome of an engine run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaExecutionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub research_id: String,
    pub study_id: String,
    pub plan_digest: ContentHash,
    pub run: ExecutionRun,
    pub stages: Vec<GliomaStageExecution>,
    pub completed_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub omission_order: Vec<String>,
    pub disposition: String,
    pub execution_digest: ContentHash,
    pub boundary: String,
}

impl GliomaExecutionReceipt {
    pub fn validate(&self) -> Result<(), GliomaEngineError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.plan_digest != self.run.plan_hash
            || self.stages.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "succeeded" | "partial" | "failed"
            )
        {
            return Err(GliomaEngineError::InvalidExecution(
                "execution identity, boundary, status, or stage outcomes are invalid".into(),
            ));
        }
        self.run
            .validate()
            .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
        let expected = ContentHash::of_value(&json!({
            "plan_digest": self.plan_digest,
            "run": self.run,
            "stages": self.stages,
            "completed_order": self.completed_order,
            "negative_evidence": self.negative_evidence,
            "uncertainty": self.uncertainty,
            "omission_order": self.omission_order,
            "disposition": self.disposition,
        }))
        .map_err(|error| GliomaEngineError::Artifact(error.to_string()))?;
        if expected != self.execution_digest {
            return Err(GliomaEngineError::InvalidExecution(
                "execution digest is not bound to the append-only run".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaEngineError {
    #[error("invalid glioma research intent: {0}")]
    InvalidIntent(String),
    #[error("glioma plan is not admissible: {0}")]
    InvalidPlan(String),
    #[error("glioma execution is invalid: {0}")]
    InvalidExecution(String),
    #[error("glioma provider failed: {0}")]
    Provider(String),
    #[error("glioma artifact failed: {0}")]
    Artifact(String),
    #[error("glioma run identifier is invalid: {0}")]
    RunId(String),
}

fn has_modality(intent: &GliomaResearchIntent, modality: GliomaModality) -> bool {
    intent.modalities.contains(&modality)
}

fn has_molecular_modality(intent: &GliomaResearchIntent) -> bool {
    intent
        .modalities
        .iter()
        .any(|modality| modality.is_molecular())
}

fn stage_readiness(kind: GliomaStageKind, intent: &GliomaResearchIntent) -> StageReadiness {
    if kind == GliomaStageKind::FederationBenchmarking
        && intent.allow_federation
        && !intent.aggregate_only
    {
        return StageReadiness::Disabled;
    }
    let input_ready = match kind {
        GliomaStageKind::IntentNormalization | GliomaStageKind::ResearchObjectRelease => true,
        GliomaStageKind::EvidenceSurveillance => has_modality(intent, GliomaModality::Literature),
        GliomaStageKind::EvidenceCompilation => {
            has_modality(intent, GliomaModality::Literature) || !intent.input_artifacts.is_empty()
        }
        GliomaStageKind::MultimodalIngestionQc => !intent.input_artifacts.is_empty(),
        GliomaStageKind::MolecularLandscape => has_molecular_modality(intent),
        GliomaStageKind::MechanismExploration => {
            has_molecular_modality(intent) || has_modality(intent, GliomaModality::Literature)
        }
        GliomaStageKind::ExperimentDesign | GliomaStageKind::ProtocolSimulation => {
            !intent.model_systems.is_empty()
        }
        GliomaStageKind::InstrumentPreflight => {
            intent.allow_instrument_execution && has_modality(intent, GliomaModality::Instrument)
        }
        GliomaStageKind::ComputationalExecution | GliomaStageKind::StatisticalInterpretation => {
            has_modality(intent, GliomaModality::Computational)
                || !intent.input_artifacts.is_empty()
        }
        GliomaStageKind::ReplicationRobustness => {
            has_modality(intent, GliomaModality::Replication) || !intent.model_systems.is_empty()
        }
        GliomaStageKind::FederationBenchmarking => intent.allow_federation && intent.aggregate_only,
    };
    if !input_ready {
        return StageReadiness::MissingInput;
    }
    let gated = kind == GliomaStageKind::InstrumentPreflight
        || intent.requested_autonomy.requires_approval();
    if gated && intent.approval_reference.is_none() {
        StageReadiness::ApprovalRequired
    } else {
        StageReadiness::Ready
    }
}

fn stage_is_required(kind: GliomaStageKind, intent: &GliomaResearchIntent) -> bool {
    !matches!(kind, GliomaStageKind::InstrumentPreflight if !intent.allow_instrument_execution)
        && !matches!(kind, GliomaStageKind::FederationBenchmarking if !intent.allow_federation)
}

fn stage_input_schemas(kind: GliomaStageKind) -> Vec<String> {
    kind.dependencies()
        .iter()
        .map(|dependency| format!("{dependency}@1"))
        .collect()
}

fn stage_autonomy(kind: GliomaStageKind, intent: &GliomaResearchIntent) -> AutonomyTier {
    if kind == GliomaStageKind::InstrumentPreflight {
        AutonomyTier::A3
    } else if kind == GliomaStageKind::FederationBenchmarking {
        AutonomyTier::A2
    } else {
        intent.requested_autonomy
    }
}

fn plan_digest_input(plan: &GliomaResearchPlan) -> serde_json::Value {
    json!({
        "schema_version": plan.schema_version,
        "contract_version": plan.contract_version,
        "feature_id": plan.feature_id,
        "research_id": plan.research_id,
        "study_id": plan.study_id,
        "objective": plan.objective,
        "workflow": plan.workflow,
        "stages": plan.stages,
        "stage_order": plan.stage_order,
        "ready_order": plan.ready_order,
        "missing_input_order": plan.missing_input_order,
        "approval_order": plan.approval_order,
        "disabled_order": plan.disabled_order,
        "disposition": plan.disposition,
        "omission_order": plan.omission_order,
        "replay_identity": plan.replay_identity,
        "boundary": plan.boundary,
    })
}

fn validate_intent(intent: &GliomaResearchIntent) -> Result<(), GliomaEngineError> {
    if intent.research_id.trim().is_empty()
        || intent.study_id.trim().is_empty()
        || intent.objective.trim().is_empty()
        || intent.output_uses.is_empty()
        || intent.budget_units == 0
        || !intent.raw_data_local
        || intent.boundary != PRECLINICAL_BOUNDARY
        || intent.replay_identity.as_str().len() != 64
    {
        return Err(GliomaEngineError::InvalidIntent(
            "identity, research uses, modalities, budget, locality, aggregate posture, replay, or boundary is invalid".into(),
        ));
    }
    let boundary = ResearchBoundary::research_only();
    let request = BoundaryRequest {
        purpose: intent.objective.clone(),
        context: RequestContext::Research,
        claimed_role: "glioma research operator".into(),
        claimed_urgency: false,
        consent: ConsentBasis::BroadResearchConsent,
        requested_uses: intent.output_uses.iter().copied().collect(),
        direct_identifier_fields: Vec::new(),
    };
    let disposition = boundary
        .triage(&request)
        .map_err(|error| GliomaEngineError::InvalidIntent(error.to_string()))?;
    if !disposition.refused().is_empty() {
        return Err(GliomaEngineError::InvalidIntent(
            "clinical or person-level output uses are outside the glioma research boundary".into(),
        ));
    }
    if intent.input_artifacts.len() > MAX_ARTIFACTS {
        return Err(GliomaEngineError::InvalidIntent(
            "input artifact count exceeds the bounded engine capacity".into(),
        ));
    }
    for artifact in &intent.input_artifacts {
        artifact.validate()?;
    }
    if let Some(reference) = &intent.approval_reference {
        if reference.trim().is_empty() {
            return Err(GliomaEngineError::InvalidIntent(
                "approval_reference cannot be blank".into(),
            ));
        }
    }
    Ok(())
}

/// Compile a full glioma research program without contacting a provider.
pub fn compile_glioma_research(
    intent: &GliomaResearchIntent,
) -> Result<GliomaResearchPlan, GliomaEngineError> {
    validate_intent(intent)?;
    let mut stages = Vec::with_capacity(GliomaStageKind::ALL.len());
    let mut nodes = Vec::with_capacity(GliomaStageKind::ALL.len());
    let mut edges = Vec::new();
    let mut checkpoints = Vec::new();
    let mut approvals = Vec::new();
    let mut ready_order = Vec::new();
    let mut missing_input_order = Vec::new();
    let mut approval_order = Vec::new();
    let mut disabled_order = Vec::new();
    let mut omission_order = BTreeSet::new();

    for kind in GliomaStageKind::ALL {
        let stage_id = kind.stage_id().to_string();
        let required = stage_is_required(kind, intent);
        let readiness = if !required {
            StageReadiness::Disabled
        } else {
            stage_readiness(kind, intent)
        };
        match readiness {
            StageReadiness::Ready => ready_order.push(stage_id.clone()),
            StageReadiness::MissingInput => {
                missing_input_order.push(stage_id.clone());
                omission_order.insert(format!("{stage_id}:required-input-missing"));
            }
            StageReadiness::ApprovalRequired => {
                approval_order.push(stage_id.clone());
                omission_order.insert(format!("{stage_id}:approval-missing"));
            }
            StageReadiness::Disabled => {
                disabled_order.push(stage_id.clone());
                omission_order.insert(format!("{stage_id}:not-requested"));
            }
        }
        let autonomy_tier = stage_autonomy(kind, intent);
        let requires_approval = required
            && (readiness == StageReadiness::ApprovalRequired || autonomy_tier.requires_approval());
        if requires_approval {
            approvals.push(ApprovalRequirement {
                approval_id: format!("approval:glioma:{stage_id}"),
                actor: "research-operator".into(),
                action: format!("approve:{stage_id}"),
            });
        }
        let stage = GliomaStage {
            stage_id: stage_id.clone(),
            kind,
            depends_on: kind.dependencies().iter().map(|id| (*id).into()).collect(),
            input_schemas: stage_input_schemas(kind),
            output_schema: kind.output_schema().into(),
            required,
            readiness,
            autonomy_tier,
            effects: kind.effects(),
            budget_units: kind.budget_units(),
        };
        for dependency in &stage.depends_on {
            edges.push(bioprism_foundation::WorkflowEdge {
                from: dependency.clone(),
                to: stage_id.clone(),
            });
        }
        nodes.push(bioprism_foundation::WorkflowNode {
            node_id: stage_id,
            capability_id: format!("glioma-stage:{}", kind.stage_id()),
            actor: "aurora-glioma-engine".into(),
            requires_approval,
        });
        checkpoints.push(bioprism_foundation::WorkflowCheckpoint {
            checkpoint_id: format!("checkpoint:{}", kind.stage_id()),
            after_nodes: [kind.stage_id().into()].into(),
        });
        stages.push(stage);
    }

    let workflow = ResearchWorkflowSpec {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        workflow_id: format!("glioma:{}:workflow", intent.research_id),
        intent: intent.objective.clone(),
        nodes,
        edges,
        checkpoints,
        budgets: vec![
            bioprism_foundation::ResourceBudget {
                resource: "research-budget-units".into(),
                amount: intent.budget_units as f64,
            },
            bioprism_foundation::ResourceBudget {
                resource: "max-retries".into(),
                amount: intent.max_retries as f64,
            },
        ],
        compensations: vec![
            bioprism_foundation::Compensation {
                effect: "local-artifact-write".into(),
                action: "quarantine-partial-artifact".into(),
            },
            bioprism_foundation::Compensation {
                effect: "instrument-reservation".into(),
                action: "release-reservation-on-failure".into(),
            },
        ],
        approvals,
        autonomy_tier: intent.requested_autonomy,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    workflow
        .validate()
        .map_err(|error| GliomaEngineError::InvalidPlan(error.to_string()))?;

    let disposition =
        if !intent.raw_data_local || (intent.allow_federation && !intent.aggregate_only) {
            GliomaPlanDisposition::Blocked
        } else if !approval_order.is_empty() {
            GliomaPlanDisposition::ApprovalRequired
        } else if !missing_input_order.is_empty() {
            GliomaPlanDisposition::NeedsInputs
        } else {
            GliomaPlanDisposition::Admitted
        };
    if disposition == GliomaPlanDisposition::Blocked {
        if !intent.raw_data_local {
            omission_order.insert("request:raw-data-must-remain-local".into());
        }
        if intent.allow_federation && !intent.aggregate_only {
            omission_order.insert("request:federation-requires-aggregate-only-export".into());
        }
    }
    let mut plan = GliomaResearchPlan {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        research_id: intent.research_id.clone(),
        study_id: intent.study_id.clone(),
        objective: intent.objective.clone(),
        workflow,
        stages,
        stage_order: GliomaStageKind::ALL
            .iter()
            .map(|kind| kind.stage_id().into())
            .collect(),
        ready_order,
        missing_input_order,
        approval_order,
        disabled_order,
        disposition,
        omission_order: omission_order.into_iter().collect(),
        replay_identity: intent.replay_identity.clone(),
        plan_digest: ContentHash::of_bytes(b"unsealed-glioma-plan"),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    plan.plan_digest = ContentHash::of_value(&plan_digest_input(&plan))
        .map_err(|error| GliomaEngineError::Artifact(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

fn append_event(
    run: &mut ExecutionRun,
    event_type: &str,
    effect: Option<Effect>,
    payload_hash: Option<ContentHash>,
) -> Result<(), GliomaEngineError> {
    run.append_event(ExecutionEvent {
        sequence: run.events.len() as u64,
        event_type: event_type.into(),
        effect,
        payload_hash,
    })
    .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))
}

/// Execute a compiled program through a caller-owned provider seam.
pub fn execute_glioma_research<E: GliomaStageExecutor>(
    plan: &GliomaResearchPlan,
    run_id: impl Into<String>,
    executor: &mut E,
    max_retries: u8,
    source_artifacts: Vec<LocalArtifactRef>,
) -> Result<GliomaExecutionReceipt, GliomaEngineError> {
    plan.validate()?;
    if plan.disposition != GliomaPlanDisposition::Admitted {
        return Err(GliomaEngineError::InvalidPlan(format!(
            "plan disposition {:?} cannot execute",
            plan.disposition
        )));
    }
    for artifact in &source_artifacts {
        artifact.validate()?;
    }
    let run_id =
        RunId::parse(run_id.into()).map_err(|error| GliomaEngineError::RunId(error.to_string()))?;
    let mut run = ExecutionRun::planned(
        run_id,
        plan.workflow.workflow_id.clone(),
        plan.plan_digest.clone(),
    )
    .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
    let stages_by_id = plan
        .stages
        .iter()
        .map(|stage| (stage.stage_id.clone(), stage))
        .collect::<BTreeMap<_, _>>();
    let mut outputs = BTreeMap::<String, ContentHash>::new();
    let mut stage_records = Vec::new();
    let mut completed_order = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut omission_order = BTreeSet::new();
    let mut hard_failure = false;
    let mut partial = false;

    for stage_id in &plan.stage_order {
        let stage = stages_by_id.get(stage_id).ok_or_else(|| {
            GliomaEngineError::InvalidExecution(format!("unknown planned stage {stage_id}"))
        })?;
        if stage.readiness != StageReadiness::Ready {
            omission_order.insert(format!(
                "{}:not-executed:{:?}",
                stage.stage_id, stage.readiness
            ));
            append_event(&mut run, "stage_blocked", None, None)?;
            run.checkpoint(format!("checkpoint:{}", stage.stage_id))
                .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
            stage_records.push(GliomaStageExecution {
                stage_id: stage.stage_id.clone(),
                attempts: 0,
                disposition: GliomaStageDisposition::Blocked,
                artifact_digest: None,
                error: Some(format!("stage readiness is {:?}", stage.readiness)),
            });
            if stage.required {
                hard_failure = true;
            }
            continue;
        }
        if stage
            .depends_on
            .iter()
            .any(|dependency| !outputs.contains_key(dependency))
        {
            omission_order.insert(format!("{}:dependency-not-complete", stage.stage_id));
            append_event(&mut run, "stage_dependency_blocked", None, None)?;
            run.checkpoint(format!("checkpoint:{}", stage.stage_id))
                .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
            stage_records.push(GliomaStageExecution {
                stage_id: stage.stage_id.clone(),
                attempts: 0,
                disposition: GliomaStageDisposition::Blocked,
                artifact_digest: None,
                error: Some("required dependency did not complete".into()),
            });
            hard_failure = true;
            continue;
        }
        let upstream_artifacts = stage
            .depends_on
            .iter()
            .filter_map(|dependency| outputs.get(dependency).cloned())
            .collect::<Vec<_>>();
        let mut attempts = 0u8;
        let output = loop {
            attempts = attempts.saturating_add(1);
            let input = GliomaStageInput {
                research_id: plan.research_id.clone(),
                study_id: plan.study_id.clone(),
                stage_id: stage.stage_id.clone(),
                kind: stage.kind,
                upstream_artifacts: upstream_artifacts.clone(),
                source_artifacts: source_artifacts.clone(),
                replay_identity: plan.replay_identity.clone(),
                attempt: attempts,
            };
            match executor.execute(stage, &input) {
                Ok(output) => break Ok(output),
                Err(failure) if failure.retryable && attempts <= max_retries => {
                    run.retry_count = run.retry_count.saturating_add(1);
                    let payload_hash = ContentHash::of_bytes(failure.reason.as_bytes());
                    append_event(&mut run, "stage_retry", None, Some(payload_hash))?;
                }
                Err(failure) => break Err(failure),
            }
        };
        match output {
            Ok(output) => {
                output.validate(stage)?;
                let artifact_digest = output.artifact.content_hash.clone();
                // Record one replay event for every declared effect. A single
                // summary event would silently drop instrument, federation, or
                // policy effects when a stage has a multi-effect contract.
                for effect in &stage.effects {
                    append_event(
                        &mut run,
                        "stage_succeeded",
                        Some(*effect),
                        Some(artifact_digest.clone()),
                    )?;
                }
                run.checkpoint(format!("checkpoint:{}", stage.stage_id))
                    .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
                outputs.insert(stage.stage_id.clone(), artifact_digest.clone());
                completed_order.push(stage.stage_id.clone());
                uncertainty.extend(output.uncertainty);
                negative_evidence.extend(output.negative_evidence);
                if output.disposition == GliomaStageDisposition::Partial {
                    partial = true;
                }
                if output.disposition == GliomaStageDisposition::Blocked {
                    hard_failure = true;
                }
                stage_records.push(GliomaStageExecution {
                    stage_id: stage.stage_id.clone(),
                    attempts,
                    disposition: output.disposition,
                    artifact_digest: Some(artifact_digest),
                    error: None,
                });
            }
            Err(failure) => {
                let payload_hash = ContentHash::of_bytes(failure.reason.as_bytes());
                append_event(&mut run, "stage_failed", None, Some(payload_hash))?;
                run.checkpoint(format!("checkpoint:{}", stage.stage_id))
                    .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
                omission_order.insert(format!("{}:provider-failure", stage.stage_id));
                hard_failure = true;
                stage_records.push(GliomaStageExecution {
                    stage_id: stage.stage_id.clone(),
                    attempts,
                    disposition: GliomaStageDisposition::Blocked,
                    artifact_digest: None,
                    error: Some(failure.reason),
                });
            }
        }
    }
    run.finish(if hard_failure {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Succeeded
    })
    .map_err(|error| GliomaEngineError::InvalidExecution(error.to_string()))?;
    let disposition = if hard_failure {
        "failed"
    } else if partial {
        "partial"
    } else {
        "succeeded"
    };
    let mut receipt = GliomaExecutionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        research_id: plan.research_id.clone(),
        study_id: plan.study_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        run,
        stages: stage_records,
        completed_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        omission_order: omission_order.into_iter().collect(),
        disposition: disposition.into(),
        execution_digest: ContentHash::of_bytes(b"unsealed-glioma-execution"),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.execution_digest = ContentHash::of_value(&json!({
        "plan_digest": receipt.plan_digest,
        "run": receipt.run,
        "stages": receipt.stages,
        "completed_order": receipt.completed_order,
        "negative_evidence": receipt.negative_evidence,
        "uncertainty": receipt.uncertainty,
        "omission_order": receipt.omission_order,
        "disposition": receipt.disposition,
    }))
    .map_err(|error| GliomaEngineError::Artifact(error.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

/// Deterministic local executor for integration tests and workflow rehearsals.
#[derive(Debug, Default)]
pub struct DryRunGliomaExecutor;

impl GliomaStageExecutor for DryRunGliomaExecutor {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "stage_id": stage.stage_id,
            "kind": stage.kind,
            "research_id": input.research_id,
            "study_id": input.study_id,
            "upstream_artifacts": input.upstream_artifacts,
            "source_artifacts": input.source_artifacts,
            "attempt": input.attempt,
            "simulated": true,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        let artifact = TypedResearchArtifact::from_payload(
            format!("glioma-dry-run:{}", stage.stage_id),
            stage.output_schema.clone(),
            &payload,
            vec![SemanticLoss {
                field: "scientific-observation".into(),
                reason: "dry-run executor does not contact a provider or observe biology".into(),
                severity: bioprism_foundation::LossSeverity::Unknown,
            }],
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: error.to_string(),
            retryable: false,
        })?;
        Ok(GliomaStageOutput {
            artifact,
            disposition: GliomaStageDisposition::Negative,
            uncertainty: vec![format!("{}:simulation-only", stage.stage_id)],
            negative_evidence: vec![format!("{}:no-external-observation", stage.stage_id)],
        })
    }
}

/// Compile and execute a deterministic rehearsal using the supplied intent's identity.
pub fn dry_run_glioma_research(
    intent: &GliomaResearchIntent,
) -> Result<GliomaExecutionReceipt, GliomaEngineError> {
    let plan = compile_glioma_research(intent)?;
    let mut executor = DryRunGliomaExecutor;
    execute_glioma_research(
        &plan,
        format!("glioma-run:{}", intent.research_id),
        &mut executor,
        intent.max_retries,
        intent.input_artifacts.clone(),
    )
}

/// Weights for the autonomous next-action selector. Values are percentages and must sum to 100.
/// Keeping the weights explicit makes a selection policy reviewable and lets a consortium change
/// priorities without changing the candidate schema or the deterministic ranking algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaSelectionWeights {
    pub information_gain: u16,
    pub frontier_novelty: u16,
    pub workflow_leverage: u16,
    pub cross_stage_unlock: u16,
    pub reproducibility_safety: u16,
    pub federation_value: u16,
    pub feasibility: u16,
}

impl Default for GliomaSelectionWeights {
    fn default() -> Self {
        Self {
            information_gain: 25,
            frontier_novelty: 20,
            workflow_leverage: 15,
            cross_stage_unlock: 15,
            reproducibility_safety: 10,
            federation_value: 10,
            feasibility: 5,
        }
    }
}

impl GliomaSelectionWeights {
    fn validate(self) -> Result<(), GliomaEngineError> {
        let total = self.information_gain as u32
            + self.frontier_novelty as u32
            + self.workflow_leverage as u32
            + self.cross_stage_unlock as u32
            + self.reproducibility_safety as u32
            + self.federation_value as u32
            + self.feasibility as u32;
        if total != 100
            || [
                self.information_gain,
                self.frontier_novelty,
                self.workflow_leverage,
                self.cross_stage_unlock,
                self.reproducibility_safety,
                self.federation_value,
                self.feasibility,
            ]
            .iter()
            .any(|score| *score > 1000)
        {
            return Err(GliomaEngineError::InvalidIntent(
                "glioma selection weights must sum to 100 and use bounded milli-scores".into(),
            ));
        }
        Ok(())
    }
}

/// One possible local assay, analysis, simulation, or instrument action.
///
/// The candidate is deliberately a product contract rather than a hypothesis: it names a stage,
/// model system, modality, cost, effects, prerequisites, and measurable multi-objective value.
/// Providers can derive these candidates from a protocol library, an instrument gateway, or a
/// researcher workbench and then hand them to the selector without giving it arbitrary code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaActionCandidate {
    pub action_id: String,
    pub stage_kind: GliomaStageKind,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub depends_on: Vec<String>,
    pub cost_units: u32,
    pub information_gain_milli: u16,
    pub frontier_novelty_milli: u16,
    pub workflow_leverage_milli: u16,
    pub cross_stage_unlock_milli: u16,
    pub reproducibility_safety_milli: u16,
    pub federation_value_milli: u16,
    pub feasibility_milli: u16,
    pub autonomy_tier: AutonomyTier,
    pub effects: BTreeSet<Effect>,
}

impl GliomaActionCandidate {
    fn validate(&self, known_actions: &BTreeSet<String>) -> Result<(), GliomaEngineError> {
        if self.action_id.trim().is_empty()
            || self.cost_units == 0
            || self.effects.is_empty()
            || self.depends_on.iter().any(|dependency| {
                dependency.trim().is_empty()
                    || dependency == &self.action_id
                    || !known_actions.contains(dependency)
            })
            || !self.depends_on.windows(2).all(|pair| pair[0] < pair[1])
            || self
                .information_gain_milli
                .max(self.frontier_novelty_milli)
                .max(self.workflow_leverage_milli)
                .max(self.cross_stage_unlock_milli)
                .max(self.reproducibility_safety_milli)
                .max(self.federation_value_milli)
                .max(self.feasibility_milli)
                > 1000
        {
            return Err(GliomaEngineError::InvalidIntent(format!(
                "glioma action {} has an invalid identity, dependency list, cost, effect set, or score",
                self.action_id
            )));
        }
        Ok(())
    }

    fn base_score(&self, weights: GliomaSelectionWeights) -> u64 {
        self.information_gain_milli as u64 * weights.information_gain as u64
            + self.frontier_novelty_milli as u64 * weights.frontier_novelty as u64
            + self.workflow_leverage_milli as u64 * weights.workflow_leverage as u64
            + self.cross_stage_unlock_milli as u64 * weights.cross_stage_unlock as u64
            + self.reproducibility_safety_milli as u64 * weights.reproducibility_safety as u64
            + self.federation_value_milli as u64 * weights.federation_value as u64
            + self.feasibility_milli as u64 * weights.feasibility as u64
    }
}

/// Controls the autonomous selector. Approval and effect switches are explicit; a high autonomy
/// tier never silently grants permission to touch an instrument or export data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GliomaSelectionConfig {
    pub budget_units: u32,
    pub max_actions: u16,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub weights: GliomaSelectionWeights,
}

impl Default for GliomaSelectionConfig {
    fn default() -> Self {
        Self {
            budget_units: 100,
            max_actions: 8,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            weights: GliomaSelectionWeights::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaActionDecision {
    pub action_id: String,
    pub score_milli_per_cost: u64,
    pub base_score_milli: u64,
    pub selected: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaActionSelection {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub decisions: Vec<GliomaActionDecision>,
    pub remaining_budget_units: u32,
    pub selection_digest: ContentHash,
    pub boundary: String,
}

impl GliomaActionSelection {
    pub fn validate(&self) -> Result<(), GliomaEngineError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.candidate_order.is_empty()
            || !self
                .candidate_order
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || self
                .selected_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .len()
                != self.selected_order.len()
            || self.decisions.len() != self.candidate_order.len()
        {
            return Err(GliomaEngineError::InvalidExecution(
                "glioma action selection identity or ordering is invalid".into(),
            ));
        }
        let candidate_ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let decision_ids = self
            .decisions
            .iter()
            .map(|decision| decision.action_id.clone())
            .collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        if decision_ids != candidate_ids
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&blocked).next().is_some()
            || deferred.intersection(&blocked).next().is_some()
            || selected.union(&deferred).chain(blocked.iter()).count() != candidate_ids.len()
        {
            return Err(GliomaEngineError::InvalidExecution(
                "glioma action decisions do not partition candidates".into(),
            ));
        }
        let expected = ContentHash::of_value(&json!({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "candidate_order": self.candidate_order,
            "selected_order": self.selected_order,
            "deferred_order": self.deferred_order,
            "blocked_order": self.blocked_order,
            "decisions": self.decisions,
            "remaining_budget_units": self.remaining_budget_units,
            "boundary": self.boundary,
        }))
        .map_err(|error| GliomaEngineError::Artifact(error.to_string()))?;
        if self.selection_digest != expected {
            return Err(GliomaEngineError::InvalidExecution(
                "glioma action selection digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn action_block_reason(
    candidate: &GliomaActionCandidate,
    config: &GliomaSelectionConfig,
) -> Option<String> {
    if !config.approval_granted && candidate.autonomy_tier.requires_approval() {
        return Some("approval-required".into());
    }
    if candidate.effects.contains(&Effect::InstrumentExecution)
        && !config.allow_instrument_execution
    {
        return Some("instrument-execution-disabled".into());
    }
    if candidate.effects.contains(&Effect::FederationExport) && !config.allow_federation {
        return Some("federation-export-disabled".into());
    }
    if candidate.effects.contains(&Effect::ExternalDataAccess) {
        return Some("external-data-access-disabled".into());
    }
    None
}

/// Select the next bounded batch of local glioma actions.
///
/// The selector is an integer, greedy approximation to information-directed experimental design:
/// a weighted value score is divided by cost, then discounted by the number of already-selected
/// actions sharing the same modality/model pair. Dependencies must be complete before a child is
/// eligible, so the returned order is executable as-is. Scores, blocked actions, and deferred
/// actions are all returned; the caller never has to infer why an assay was omitted.
pub fn select_glioma_actions(
    candidates: &[GliomaActionCandidate],
    completed_actions: &BTreeSet<String>,
    config: &GliomaSelectionConfig,
) -> Result<GliomaActionSelection, GliomaEngineError> {
    if candidates.is_empty() || config.budget_units == 0 || config.max_actions == 0 {
        return Err(GliomaEngineError::InvalidIntent(
            "glioma action selection requires candidates, budget, and max_actions".into(),
        ));
    }
    config.weights.validate()?;
    let ids = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    if ids.len() != candidates.len() || completed_actions.iter().any(|id| id.trim().is_empty()) {
        return Err(GliomaEngineError::InvalidIntent(
            "glioma action identifiers must be unique and non-empty".into(),
        ));
    }
    let known_actions = ids
        .iter()
        .cloned()
        .chain(completed_actions.iter().cloned())
        .collect::<BTreeSet<_>>();
    for candidate in candidates {
        candidate.validate(&known_actions)?;
    }
    let candidate_order = ids.iter().cloned().collect::<Vec<_>>();
    let candidate_map = candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut blocked = BTreeMap::<String, String>::new();
    for candidate in candidates {
        if completed_actions.contains(&candidate.action_id) {
            blocked.insert(candidate.action_id.clone(), "already-completed".into());
            continue;
        }
        if let Some(reason) = action_block_reason(candidate, config) {
            blocked.insert(candidate.action_id.clone(), reason);
            continue;
        }
        for dependency in &candidate.depends_on {
            if !ids.contains(dependency) && !completed_actions.contains(dependency) {
                blocked.insert(
                    candidate.action_id.clone(),
                    format!("missing-dependency:{dependency}"),
                );
                break;
            }
        }
    }
    let base_scores = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.action_id.clone(),
                candidate.base_score(config.weights),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut selected = Vec::new();
    let mut selected_set = completed_actions.clone();
    let mut remaining_budget = config.budget_units;
    let mut decisions = BTreeMap::<String, GliomaActionDecision>::new();
    let mut diversity_counts = BTreeMap::<(GliomaModality, GliomaModelSystem), u32>::new();

    while selected.len() < config.max_actions as usize {
        let mut ranked = Vec::<(u64, String)>::new();
        for candidate in candidates {
            if blocked.contains_key(&candidate.action_id)
                || selected_set.contains(&candidate.action_id)
            {
                continue;
            }
            if candidate.cost_units > remaining_budget
                || candidate
                    .depends_on
                    .iter()
                    .any(|dependency| !selected_set.contains(dependency))
            {
                continue;
            }
            let siblings = diversity_counts
                .get(&(candidate.modality, candidate.model_system))
                .copied()
                .unwrap_or(0);
            let diversity_milli = 1000u64 / (1 + siblings as u64);
            let score = base_scores[&candidate.action_id]
                .saturating_mul(diversity_milli)
                .saturating_mul(1_000_000)
                / (candidate.cost_units as u64 * 1000);
            ranked.push((score, candidate.action_id.clone()));
        }
        ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let Some((score, action_id)) = ranked.into_iter().next() else {
            break;
        };
        let candidate = candidate_map[&action_id];
        remaining_budget -= candidate.cost_units;
        selected.push(action_id.clone());
        selected_set.insert(action_id.clone());
        *diversity_counts
            .entry((candidate.modality, candidate.model_system))
            .or_default() += 1;
        decisions.insert(
            action_id.clone(),
            GliomaActionDecision {
                action_id,
                score_milli_per_cost: score,
                base_score_milli: base_scores[&candidate.action_id],
                selected: true,
                reason: None,
            },
        );
    }
    let mut deferred = Vec::new();
    for candidate in candidates {
        if !blocked.contains_key(&candidate.action_id)
            && !selected_set.contains(&candidate.action_id)
        {
            deferred.push(candidate.action_id.clone());
            decisions.insert(
                candidate.action_id.clone(),
                GliomaActionDecision {
                    action_id: candidate.action_id.clone(),
                    score_milli_per_cost: 0,
                    base_score_milli: base_scores[&candidate.action_id],
                    selected: false,
                    reason: Some(if candidate.cost_units > remaining_budget {
                        "budget-exhausted".into()
                    } else {
                        "dependency-not-selected".into()
                    }),
                },
            );
        }
    }
    deferred.sort();
    let blocked_order = blocked.keys().cloned().collect::<Vec<_>>();
    for (action_id, reason) in &blocked {
        decisions.insert(
            action_id.clone(),
            GliomaActionDecision {
                action_id: action_id.clone(),
                score_milli_per_cost: 0,
                base_score_milli: base_scores[action_id],
                selected: false,
                reason: Some(reason.clone()),
            },
        );
    }
    let decisions = candidate_order
        .iter()
        .map(|action_id| decisions[action_id].clone())
        .collect::<Vec<_>>();
    let mut selection = GliomaActionSelection {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        candidate_order,
        selected_order: selected,
        deferred_order: deferred,
        blocked_order,
        decisions,
        remaining_budget_units: remaining_budget,
        selection_digest: ContentHash::of_bytes(b"unsealed-glioma-selection"),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    selection.selection_digest = ContentHash::of_value(&json!({
        "schema_version": selection.schema_version,
        "contract_version": selection.contract_version,
        "feature_id": selection.feature_id,
        "candidate_order": selection.candidate_order,
        "selected_order": selection.selected_order,
        "deferred_order": selection.deferred_order,
        "blocked_order": selection.blocked_order,
        "decisions": selection.decisions,
        "remaining_budget_units": selection.remaining_budget_units,
        "boundary": selection.boundary,
    }))
    .map_err(|error| GliomaEngineError::Artifact(error.to_string()))?;
    selection.validate()?;
    Ok(selection)
}

pub fn glioma_research_engine_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "research".into(),
        consumers: [
            "glioma research program lead".into(),
            "preclinical workflow operator".into(),
            "local analysis executor".into(),
            "instrument gateway".into(),
            "adaptive glioma scheduler".into(),
        ]
        .into(),
        behavior: "compile an adaptive, checkpoint-oriented glioma campaign, continuously diff local evidence snapshots into prioritized review actions, compile typed claims and evidence-gap actions, choose the next dependency-safe batch, execute an admitted full preclinical program through caller-owned local stage executors, detect instrument-control drift before assay admission, compare and batch-harmonize declared multimodal vectors before consensus, extract robust complete-case latent states with convergence and reconstruction gates, build same-lineage spatial niches and cross-lineage interaction enrichment, compare and cluster multimodal vectors, discriminate competing mechanisms against local features and rank information-gain assays, propagate signed activating/inhibiting mechanism networks with convergence gates, fit longitudinal, causal-contrast, stratified-overlap-adjusted, dose-response, combination-synergy, replication-meta-analysis, and hidden-confounding sensitivity effects, allocate the next bounded assay replicate batch from conservative Beta-posterior effect probabilities, compare aggregate federated benchmark outcomes with robust site consensus, and stress-test endpoint effects under deterministic omission batteries; every unresolved, contradictory, underpowered, heterogeneous, budget, locality, and approval state routes to an explicit hold or abstain branch".into(),
        value: "turns a glioma research objective into a usable end-to-end evidence, multimodal, mechanism, experiment, computation, replication, and release workflow while keeping autonomous progress auditable and outside clinical decision support".into(),
        inputs: vec![TypedPort {
            name: "glioma_research_intent".into(),
            schema: "GliomaResearchIntent1@1".into(),
            required: true,
        }],
        outputs: vec![
            TypedPort {
                name: "glioma_execution_receipt".into(),
                schema: OUTPUT_SCHEMA.into(),
                required: true,
            },
            TypedPort {
                name: "glioma_action_selection".into(),
                schema: ACTION_SELECTION_OUTPUT_SCHEMA.into(),
                required: false,
            },
            TypedPort {
                name: "glioma_adaptive_workflow_plan".into(),
                schema: "GliomaAdaptiveWorkflow1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_protocol_simulation".into(),
                schema: "GliomaProtocolSimulation1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_robustness_suite".into(),
                schema: "GliomaRobustnessSuite1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_trajectory_analysis".into(),
                schema: "GliomaTrajectoryAnalysis1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_causal_contrast".into(),
                schema: "GliomaCausalContrast1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_stratified_causal_adjustment".into(),
                schema: "GliomaStratifiedCausalAdjustment1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_dose_response_analysis".into(),
                schema: "GliomaDoseResponseAnalysis1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_combination_synergy".into(),
                schema: "GliomaCombinationSynergy1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_adaptive_allocation".into(),
                schema: "GliomaAdaptiveAllocation1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_multimodal_concordance".into(),
                schema: "GliomaMultimodalConcordance1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_multimodal_consensus".into(),
                schema: "GliomaMultimodalConsensus1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_multimodal_harmonization".into(),
                schema: "GliomaMultimodalHarmonization1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_multimodal_latent_factors".into(),
                schema: "GliomaLatentFactorization1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_spatial_niches".into(),
                schema: "GliomaSpatialNiche1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_causal_sensitivity".into(),
                schema: "GliomaCausalSensitivity1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_mechanism_graph_propagation".into(),
                schema: "GliomaMechanismGraphPropagation1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_typed_knowledge".into(),
                schema: "GliomaTypedKnowledge1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_evidence_surveillance".into(),
                schema: "GliomaEvidenceSurveillance1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_decision_context".into(),
                schema: "GliomaDecisionContext1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_mechanism_discrimination".into(),
                schema: "GliomaMechanismDiscrimination1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_instrument_calibration".into(),
                schema: "GliomaInstrumentCalibration1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_replication_meta_analysis".into(),
                schema: "GliomaReplicationMetaAnalysis1@1".into(),
                required: false,
            },
            TypedPort {
                name: "glioma_federated_benchmark_consensus".into(),
                schema: "GliomaFederatedBenchmarkConsensus1@1".into(),
                required: false,
            },
        ],
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::InstrumentExecution,
            Effect::FederationExport,
        ]),
        permissions: BTreeSet::from([
            "execute:glioma-local-research".into(),
            "approve:glioma-instrument-preflight".into(),
            "export:aggregate-research-artifacts".into(),
        ]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "tcga-glioblastoma-nature-2008".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.nature.com/articles/nature07385".into()),
            },
            EvidenceReference {
                source_id: "who-cns5-integrated-molecular-classification".into(),
                state: EvidenceState::Supported,
                locator: Some("https://pmc.ncbi.nlm.nih.gov/articles/PMC8328013/".into()),
            },
        ],
        authority_requirements: vec![bioprism_foundation::AuthorityRequirement {
            role: "preclinical research workflow approver".into(),
            reason: "approve any external-data, instrument, or aggregate federation effect before dispatch".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([
            ResearchSurface::Ui,
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn intent() -> GliomaResearchIntent {
        GliomaResearchIntent {
            research_id: "glioma-engine-test".into(),
            study_id: "study-a".into(),
            objective: "map molecular mechanisms in a preclinical glioma model".into(),
            output_uses: [OutputUse::CohortAnalysis, OutputUse::MethodDevelopment]
                .into_iter()
                .collect(),
            model_systems: [GliomaModelSystem::Organoid, GliomaModelSystem::InSilico]
                .into_iter()
                .collect(),
            modalities: [
                GliomaModality::Literature,
                GliomaModality::Genomics,
                GliomaModality::Transcriptomics,
                GliomaModality::Imaging,
                GliomaModality::Computational,
                GliomaModality::Replication,
            ]
            .into_iter()
            .collect(),
            input_artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact:glioma".into(),
                content_hash: digest("artifact"),
                content_type: "application/vnd.glioma.input+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 300,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: digest("replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn action(
        action_id: &str,
        stage_kind: GliomaStageKind,
        modality: GliomaModality,
        model_system: GliomaModelSystem,
        cost_units: u32,
        information_gain_milli: u16,
    ) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: action_id.into(),
            stage_kind,
            modality,
            model_system,
            depends_on: Vec::new(),
            cost_units,
            information_gain_milli,
            frontier_novelty_milli: 700,
            workflow_leverage_milli: 800,
            cross_stage_unlock_milli: 750,
            reproducibility_safety_milli: 900,
            federation_value_milli: 500,
            feasibility_milli: 850,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    #[test]
    fn full_pipeline_is_compiled_and_digest_stable() {
        let first = compile_glioma_research(&intent()).unwrap();
        let second = compile_glioma_research(&intent()).unwrap();
        assert_eq!(first.plan_digest, second.plan_digest);
        assert_eq!(first.stages.len(), 14);
        assert_eq!(first.disposition, GliomaPlanDisposition::Admitted);
        assert!(first.stage_order.contains(&"mechanism-exploration".into()));
        assert!(first.workflow.validate().is_ok());
        assert!(glioma_research_engine_manifest().validate().is_ok());
    }

    #[test]
    fn clinical_output_use_is_refused_before_planning() {
        let mut request = intent();
        request
            .output_uses
            .insert(OutputUse::TreatmentRecommendation);
        let error = compile_glioma_research(&request).unwrap_err();
        assert!(error.to_string().contains("clinical"));
    }

    #[test]
    fn missing_inputs_are_explicit_and_not_silently_skipped() {
        let mut request = intent();
        request.modalities.clear();
        request.input_artifacts.clear();
        request.model_systems.clear();
        let plan = compile_glioma_research(&request).unwrap();
        assert_eq!(plan.disposition, GliomaPlanDisposition::NeedsInputs);
        assert!(!plan.missing_input_order.is_empty());
        assert!(plan
            .omission_order
            .iter()
            .any(|item| item.contains("required-input-missing")));
    }

    #[test]
    fn local_record_level_processing_is_allowed_without_federation() {
        let mut request = intent();
        request.aggregate_only = false;
        let plan = compile_glioma_research(&request).unwrap();
        assert_eq!(plan.disposition, GliomaPlanDisposition::Admitted);
        assert!(plan
            .disabled_order
            .contains(&"federation-benchmarking".into()));
    }

    #[test]
    fn federation_requires_an_aggregate_only_export_posture() {
        let mut request = intent();
        request.aggregate_only = false;
        request.allow_federation = true;
        let plan = compile_glioma_research(&request).unwrap();
        assert_eq!(plan.disposition, GliomaPlanDisposition::Blocked);
        assert!(plan
            .omission_order
            .iter()
            .any(|item| item.contains("federation-requires-aggregate-only-export")));
        assert_eq!(
            plan.stages
                .iter()
                .find(|stage| stage.kind == GliomaStageKind::FederationBenchmarking)
                .map(|stage| stage.readiness),
            Some(StageReadiness::Disabled)
        );
    }

    #[test]
    fn dry_run_executes_every_stage_but_marks_simulation_negative() {
        let receipt = dry_run_glioma_research(&intent()).unwrap();
        assert_eq!(receipt.disposition, "succeeded");
        assert_eq!(receipt.completed_order.len(), 12);
        assert!(!receipt.negative_evidence.is_empty());
        assert!(receipt.validate().is_ok());
    }

    struct RetryOnceExecutor {
        attempts: u8,
    }

    impl GliomaStageExecutor for RetryOnceExecutor {
        fn execute(
            &mut self,
            stage: &GliomaStage,
            input: &GliomaStageInput,
        ) -> Result<GliomaStageOutput, GliomaStageFailure> {
            self.attempts = self.attempts.saturating_add(1);
            if stage.kind == GliomaStageKind::EvidenceSurveillance && input.attempt == 1 {
                return Err(GliomaStageFailure {
                    reason: "transient local index lock".into(),
                    retryable: true,
                });
            }
            let payload = json!({"stage": stage.stage_id, "attempt": input.attempt});
            let artifact = TypedResearchArtifact::from_payload(
                format!("artifact:{}", stage.stage_id),
                stage.output_schema.clone(),
                &payload,
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            Ok(GliomaStageOutput {
                artifact,
                disposition: GliomaStageDisposition::Completed,
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            })
        }
    }

    #[test]
    fn retryable_provider_failure_is_retried_without_duplicate_stage_success() {
        let plan = compile_glioma_research(&intent()).unwrap();
        let mut executor = RetryOnceExecutor { attempts: 0 };
        let receipt = execute_glioma_research(
            &plan,
            "glioma-run:retry",
            &mut executor,
            1,
            intent().input_artifacts,
        )
        .unwrap();
        assert_eq!(receipt.run.retry_count, 1);
        assert_eq!(receipt.completed_order.len(), 12);
        assert!(receipt
            .run
            .events
            .iter()
            .any(|event| event.event_type == "stage_retry"));
    }

    #[test]
    fn adaptive_action_selection_is_dependency_safe_and_reproducible() {
        let first = action(
            "action-a",
            GliomaStageKind::MolecularLandscape,
            GliomaModality::Genomics,
            GliomaModelSystem::Organoid,
            10,
            950,
        );
        let mut second = action(
            "action-b",
            GliomaStageKind::MechanismExploration,
            GliomaModality::Transcriptomics,
            GliomaModelSystem::Organoid,
            10,
            800,
        );
        second.depends_on = vec!["action-a".into()];
        let mut blocked = action(
            "action-instrument",
            GliomaStageKind::InstrumentPreflight,
            GliomaModality::Instrument,
            GliomaModelSystem::Organoid,
            1,
            1000,
        );
        blocked.effects.insert(Effect::InstrumentExecution);
        let config = GliomaSelectionConfig {
            budget_units: 25,
            max_actions: 3,
            ..GliomaSelectionConfig::default()
        };
        let first_selection = select_glioma_actions(
            &[first.clone(), second.clone(), blocked.clone()],
            &BTreeSet::new(),
            &config,
        )
        .unwrap();
        let second_selection =
            select_glioma_actions(&[blocked, second, first], &BTreeSet::new(), &config).unwrap();
        assert_eq!(first_selection, second_selection);
        assert_eq!(first_selection.selected_order, vec!["action-a", "action-b"]);
        assert_eq!(first_selection.blocked_order, vec!["action-instrument"]);
        assert!(first_selection
            .decisions
            .iter()
            .any(|decision| decision.action_id == "action-instrument"
                && decision.reason.as_deref() == Some("instrument-execution-disabled")));
        first_selection.validate().unwrap();
    }

    #[test]
    fn adaptive_action_selection_respects_budget_and_approval() {
        let mut expensive = action(
            "expensive",
            GliomaStageKind::ComputationalExecution,
            GliomaModality::Computational,
            GliomaModelSystem::InSilico,
            50,
            1000,
        );
        expensive.autonomy_tier = AutonomyTier::A2;
        let selection = select_glioma_actions(
            &[expensive],
            &BTreeSet::new(),
            &GliomaSelectionConfig {
                budget_units: 10,
                max_actions: 1,
                ..GliomaSelectionConfig::default()
            },
        )
        .unwrap();
        assert_eq!(selection.blocked_order, vec!["expensive"]);
        assert_eq!(selection.deferred_order, Vec::<String>::new());
        assert_eq!(
            selection.decisions[0].reason.as_deref(),
            Some("approval-required")
        );
        selection.validate().unwrap();
    }
}
