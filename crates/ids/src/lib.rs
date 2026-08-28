//! Canonical serialization, content hashing and typed identifiers.
//!
//! This is the bottom of the BioPRISM dependency graph: it depends on no other workspace
//! crate, and every crate that emits a hash, a certificate or an identifier depends on it.
//!
//! Implements blueprint 40.05 (canonical identifiers and hashes) and supplies the hashing
//! primitive that 43.26 (Context Certificate) requires to be replayable across languages.

pub mod canonical;
pub mod computational_execution_workbench;
pub mod context_compilation_federated_control_plane;
pub mod error;
pub mod evolution;
pub mod experiment_design_workbench;
pub mod federated_resource_discovery_interoperability;
pub mod hash;
pub mod id;
pub mod interpretation_plane;
pub mod knowledge_representation_federated_control_plane;
pub mod laboratory_integration_workflow_fabric;
pub mod mechanism_exploration_assurance;
pub mod multimodal_ingestion_research_copilot;
pub mod protocol_simulation_workbench;
pub mod quality_control_assurance;
pub mod retrieval_synthesis_assurance_harness;
pub mod statistical_causal_ml_research_copilot;
pub mod throughput_evidence_surveillance_contract_model;

pub use canonical::{python_repr_f64, to_canonical_bytes, to_canonical_string};
pub use computational_execution_workbench::{
    compile_computational_execution, computational_execution_manifest, ComputationNode6,
    ComputationPeer6, ComputationalExecutionArtifact9, ComputationalExecutionError,
    ComputationalExecutionReport9, ComputationalExecutionRequest6, ExecutionEvidenceState,
    CONTRACT_VERSION as IDS_COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
    FEATURE_ID as IDS_COMPUTATIONAL_EXECUTION_FEATURE_ID,
};
pub use context_compilation_federated_control_plane::{
    context_compilation_manifest, operate_context_compilation, CertifiedDecisionSection1,
    CertifiedDecisionSection1Artifact, ContextCompilationError, ContextEvidenceState, ContextFact4,
    ContextPeer4, DecisionQuery4, CONTRACT_VERSION as IDS_CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_CONTEXT_COMPILATION_FEATURE_ID,
};
pub use error::{CanonicalError, IdError};
pub use evolution::{
    EvolutionIdentity, EvolutionIdentityError,
    CONTRACT_VERSION as EVOLUTION_IDENTITY_CONTRACT_VERSION,
    FEATURE_ID as EVOLUTION_IDENTITY_FEATURE_ID,
    PRECLINICAL_BOUNDARY as EVOLUTION_IDENTITY_BOUNDARY,
};
pub use experiment_design_workbench::{
    design_experiment, experiment_design_manifest, DesignCandidate4, DesignEvidenceState,
    DesignFrontier8, DesignFrontier8Artifact, ExperimentDesignError, ExperimentDesignRequest4,
    CONTRACT_VERSION as IDS_EXPERIMENT_DESIGN_CONTRACT_VERSION,
    FEATURE_ID as IDS_EXPERIMENT_DESIGN_FEATURE_ID,
};
pub use federated_resource_discovery_interoperability::{
    interoperability_manifest, interoperate_resources, EndpointStatus as ResourceEndpointStatus,
    EvidenceState as ResourceEvidenceState,
    InteroperabilityManifest as ResourceInteroperabilityManifest, PeerResourceSummary4,
    QualifiedResource6, QualifiedResourceSet6, ResourceArtifact6, ResourceDisposition,
    ResourceEndpoint4, ResourceInteroperabilityError, ResourceNeed4,
    CONTRACT_VERSION as IDS_RESOURCE_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as IDS_RESOURCE_INTEROPERABILITY_FEATURE_ID,
};
pub use hash::{sha256_hex_of_value, ContentHash};
pub use id::{EventId, FactId, FactorId, QueryId, RunId, VariableName, WorldId};
pub use interpretation_plane::{
    operate_interpretation_plane, EvidenceBackedResult, InterpretationArtifact,
    InterpretationDisposition, InterpretationPlaneError, InterpretationPlaneReceipt,
    InterpretationPlaneRequest, CONTRACT_VERSION as INTERPRETATION_PLANE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_PLANE_FEATURE_ID,
    PRECLINICAL_BOUNDARY as INTERPRETATION_PLANE_BOUNDARY,
};
pub use knowledge_representation_federated_control_plane::{
    knowledge_representation_manifest, operate_knowledge_representation, KnowledgeClaim4,
    KnowledgeEvidenceState, KnowledgePeer4, KnowledgeRepresentationError, ScopedKnowledgeClaims4,
    TypedKnowledgeWorld7, TypedKnowledgeWorld7Artifact,
    CONTRACT_VERSION as IDS_KNOWLEDGE_REPRESENTATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_KNOWLEDGE_REPRESENTATION_FEATURE_ID,
};
pub use laboratory_integration_workflow_fabric::{
    integrate_laboratory_workflow, laboratory_integration_manifest, InstrumentEndpoint6,
    LabAction6, LaboratoryEvidenceState, LaboratoryIntegrationArtifact9,
    LaboratoryIntegrationError, LaboratoryIntegrationReport9, LaboratoryIntegrationRequest6,
    LaboratoryPeer6, CONTRACT_VERSION as IDS_LABORATORY_INTEGRATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_LABORATORY_INTEGRATION_FEATURE_ID,
};
pub use mechanism_exploration_assurance::{
    assure_mechanism_exploration, mechanism_exploration_manifest, MechanismCandidate4,
    MechanismEvidenceState, MechanismExplorationError, MechanismPortfolio7,
    MechanismPortfolio7Artifact, MechanismQuestion2, PeerMechanismSummary4,
    CONTRACT_VERSION as IDS_MECHANISM_EXPLORATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_MECHANISM_EXPLORATION_FEATURE_ID,
};
pub use multimodal_ingestion_research_copilot::{
    multimodal_ingestion_manifest, operate_multimodal_ingestion, HarmonizedResearchObject8,
    HarmonizedResearchObject8Artifact, IngestionEvidenceState, ModalityObservation4,
    MultimodalIngestionError, MultimodalIngestionRequest4,
    CONTRACT_VERSION as IDS_MULTIMODAL_INGESTION_CONTRACT_VERSION,
    FEATURE_ID as IDS_MULTIMODAL_INGESTION_FEATURE_ID,
};
pub use protocol_simulation_workbench::{
    protocol_workbench_manifest, simulate_protocol_workbench, ProtocolEvidenceState, ProtocolPeer5,
    ProtocolScenario5, ProtocolStage5, ProtocolWorkbenchArtifact9, ProtocolWorkbenchError,
    ProtocolWorkbenchReport9, ProtocolWorkbenchRequest5,
    CONTRACT_VERSION as IDS_PROTOCOL_SIMULATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_PROTOCOL_SIMULATION_FEATURE_ID,
};
pub use quality_control_assurance::{
    assure_quality_control, quality_control_manifest, QualityControlBatch4, QualityControlError,
    QualityControlReport8, QualityControlReport8Artifact, QualityEvidenceState,
    QualityObservation4, CONTRACT_VERSION as IDS_QUALITY_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as IDS_QUALITY_CONTROL_FEATURE_ID,
};
pub use retrieval_synthesis_assurance_harness::{
    assure_retrieval_synthesis, retrieval_synthesis_assurance_manifest, EvidenceSynthesis11,
    EvidenceSynthesis11Artifact, RetrievalEvidence7, RetrievalEvidenceState, RetrievalPeer6,
    RetrievalSynthesisAssuranceError, ScopedRetrievalQuery6,
    CONTRACT_VERSION as IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_FEATURE_ID,
};
pub use statistical_causal_ml_research_copilot::{
    compile_statistical_causal_ml, statistical_causal_ml_manifest, AnalysisCandidate8,
    AnalysisCopilotRequest7, AnalysisEvidenceState, QualifiedAnalysisResult10,
    QualifiedAnalysisResult10Artifact, StatisticalCausalMlError,
    CONTRACT_VERSION as IDS_STATISTICAL_CAUSAL_ML_CONTRACT_VERSION,
    FEATURE_ID as IDS_STATISTICAL_CAUSAL_ML_FEATURE_ID,
};
pub use throughput_evidence_surveillance_contract_model::{
    model_throughput_evidence_surveillance_contract,
    throughput_evidence_surveillance_contract_model_manifest,
    ContractClaim as IdsThroughputContractClaim,
    ContractDisposition as IdsThroughputContractDisposition,
    ContractModelError as IdsThroughputContractModelError,
    EvidenceFeedRequest as IdsEvidenceFeedRequest, EvidenceState as IdsEvidenceState,
    EvidenceSurveillanceContractReceipt as IdsEvidenceSurveillanceContractReceipt,
    TypedArtifact as IdsTypedArtifact, CONTRACT_VERSION as IDS_THROUGHPUT_CONTRACT_VERSION,
    FEATURE_ID as IDS_THROUGHPUT_FEATURE_ID,
};
