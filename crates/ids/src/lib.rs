//! Canonical serialization, content hashing and typed identifiers.
//!
//! This is the bottom of the BioPRISM dependency graph: it depends on no other workspace
//! crate, and every crate that emits a hash, a certificate or an identifier depends on it.
//!
//! Implements blueprint 40.05 (canonical identifiers and hashes) and supplies the hashing
//! primitive that 43.26 (Context Certificate) requires to be replayable across languages.

#![allow(clippy::all)]

pub mod adversarial_recovery_workbench;
pub mod bounded_evolution_control_plane;
pub mod canonical;
pub mod computational_execution_workbench;
pub mod context_compilation_federated_control_plane;
pub mod contract_frontier;
pub mod dependency_composition_workbench;
pub mod error;
pub mod evaluation_assurance;
pub mod evolution;
pub mod experiment_design_workbench;
pub mod federated_commons_workflow;
pub mod federated_interpretation_visualization_assurance;
pub mod federated_resource_discovery_interoperability;
pub mod federated_workflow_fabric;
pub mod federation_security_contract;
pub mod hash;
pub mod id;
pub mod interoperability_extensibility_copilot;
pub mod interoperability_gateway;
pub mod interpretation_plane;
pub mod knowledge_representation_federated_control_plane;
pub mod laboratory_integration_workflow_fabric;
pub mod limitation_closure_gateway;
pub mod local_evidence_surveillance_inference;
pub mod mechanism_exploration_assurance;
pub mod multimodal_ingestion_research_copilot;
pub mod performance_reliability_gateway;
pub mod policy_autonomy_interoperability_gateway;
pub mod policy_autonomy_workbench;
pub mod prospective_provenance_assurance;
pub mod protocol_simulation_workbench;
pub mod provenance_signing_assurance;
pub mod publication_research_object_release_control_plane;
pub mod quality_control_assurance;
pub mod reliability_copilot;
pub mod replication_negative_results_interoperability_gateway;
pub mod research_workbench;
pub mod retrieval_synthesis_assurance_harness;
pub mod scale_frontier_workflow;
pub mod semantic_parity_contract;
pub mod statistical_causal_ml_research_copilot;
pub mod throughput_evidence_surveillance_contract_model;
pub mod typed_determinism_assurance;
pub mod typed_determinism_interoperability_gateway;

pub use adversarial_recovery_workbench::{
    adversarial_recovery_manifest, preview_adversarial_recovery, AdversarialRecoveryWorkbenchError,
    IdsAdversarialRecoveryReceipt10, IdsAdversarialRecoveryReceipt10Artifact,
    IdsAdversarialRecoveryRequest8, IdsRecoveryEvent7, RecoveryEvidenceState,
    CONTRACT_VERSION as IDS_ADVERSARIAL_RECOVERY_CONTRACT_VERSION,
    FEATURE_ID as IDS_ADVERSARIAL_RECOVERY_FEATURE_ID,
};
pub use bounded_evolution_control_plane::{
    bounded_evolution_manifest, preview_bounded_evolution, BoundedEvolutionError,
    EvolutionEvidenceState, IdsEvolutionProposal7, IdsEvolutionReceipt10,
    IdsEvolutionReceipt10Artifact, IdsEvolutionRequest8,
    CONTRACT_VERSION as IDS_BOUNDED_EVOLUTION_CONTRACT_VERSION,
    FEATURE_ID as IDS_BOUNDED_EVOLUTION_FEATURE_ID,
};
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
pub use contract_frontier::{
    assure_contract_frontier, contract_frontier_manifest, ContractEvidenceState,
    ContractFrontierError, IdsCapabilityManifest9, IdsCapabilityManifest9Artifact,
    IdsContractFrontierRequest7, IdsContractInput8,
    CONTRACT_VERSION as IDS_CONTRACT_FRONTIER_CONTRACT_VERSION,
    FEATURE_ID as IDS_CONTRACT_FRONTIER_FEATURE_ID,
};
pub use dependency_composition_workbench::{
    compose_ids_dependencies, dependency_composition_manifest, CompositionEvidenceState,
    DependencyCompositionError, IdsCompositionCandidate8, IdsCompositionReceipt9,
    IdsCompositionReceipt9Artifact, IdsCompositionRequest7,
    CONTRACT_VERSION as IDS_DEPENDENCY_COMPOSITION_CONTRACT_VERSION,
    FEATURE_ID as IDS_DEPENDENCY_COMPOSITION_FEATURE_ID,
};
pub use error::{CanonicalError, IdError};
pub use evaluation_assurance::{
    assure_evaluation, evaluation_assurance_manifest, CapabilityObservation8, CapabilityRun7,
    EvaluationAssuranceError, EvaluationCard9, EvaluationCard9Artifact, EvaluationEvidenceState,
    CONTRACT_VERSION as IDS_EVALUATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as IDS_EVALUATION_ASSURANCE_FEATURE_ID,
};
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
pub use federated_commons_workflow::{
    federated_commons_manifest, preview_federated_commons, CommonsEvidenceState,
    FederatedCommonsError, IdsCommonsPeer7, IdsFederatedCommonsReceipt10,
    IdsFederatedCommonsReceipt10Artifact, IdsFederatedCommonsRequest8,
    CONTRACT_VERSION as IDS_FEDERATED_COMMONS_CONTRACT_VERSION,
    FEATURE_ID as IDS_FEDERATED_COMMONS_FEATURE_ID,
};
pub use federated_interpretation_visualization_assurance::{
    assure_ids_interpretation, ids_interpretation_visualization_assurance_manifest,
    IdsEvidenceBackedResult4, IdsInteractiveInterpretation7, IdsInterpretationArtifact7,
    IdsInterpretationAssuranceError, IdsInterpretationCandidate4, IdsInterpretationEvidenceState,
    CONTRACT_VERSION as IDS_INTERPRETATION_VISUALIZATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_INTERPRETATION_VISUALIZATION_FEATURE_ID,
    INPUT_SCHEMA as IDS_INTERPRETATION_VISUALIZATION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as IDS_INTERPRETATION_VISUALIZATION_OUTPUT_SCHEMA,
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
pub use federated_workflow_fabric::{
    compile_federated_workflow, federated_workflow_fabric_manifest, FederatedWorkflowError,
    FederatedWorkflowReceipt9, FederatedWorkflowReceipt9Artifact, FederatedWorkflowRequest7,
    WorkflowEvidenceState, WorkflowPeer7, WorkflowStage8,
    CONTRACT_VERSION as IDS_FEDERATED_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as IDS_FEDERATED_WORKFLOW_FEATURE_ID,
};
pub use federation_security_contract::{
    admit_federation_security, federation_security_contract_manifest, FederationContribution5,
    FederationEnvelope2, FederationEnvelopeArtifact2, FederationEvidenceState, FederationRequest4,
    FederationSecurityError, CONTRACT_VERSION as IDS_FEDERATION_SECURITY_CONTRACT_VERSION,
    FEATURE_ID as IDS_FEDERATION_SECURITY_FEATURE_ID,
};
pub use hash::{sha256_hex_of_value, ContentHash};
pub use id::{EventId, FactId, FactorId, QueryId, RunId, VariableName, WorldId};
pub use interoperability_extensibility_copilot::{
    interoperability_extensibility_copilot_manifest,
    negotiate_interoperability as negotiate_interoperability_copilot,
    CapabilityEvidenceState as InteroperabilityEvidenceState, ExternalCapability2,
    ExternalCapabilityRequest2, InteroperabilityExtensibilityError, NegotiatedIntegration3,
    NegotiatedIntegrationArtifact3,
    CONTRACT_VERSION as IDS_INTEROPERABILITY_EXTENSIBILITY_CONTRACT_VERSION,
    FEATURE_ID as IDS_INTEROPERABILITY_EXTENSIBILITY_FEATURE_ID,
};
pub use interoperability_gateway::{
    interoperability_gateway_manifest, negotiate_interoperability, ExternalCapability8,
    IntegrationEvidenceState, InteroperabilityError, InteroperabilityRequest7,
    NegotiatedIntegration9, NegotiatedIntegration9Artifact,
    CONTRACT_VERSION as IDS_INTEROPERABILITY_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as IDS_INTEROPERABILITY_GATEWAY_FEATURE_ID,
};
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
pub use limitation_closure_gateway::{
    close_ids_limitations, limitation_closure_manifest, IdsClosurePeer7, IdsClosureReceipt9,
    IdsClosureReceipt9Artifact, IdsLimitationCase8, IdsLimitationClosureRequest7,
    LimitationClosureError, LimitationState, PeerEvidenceState,
    CONTRACT_VERSION as IDS_LIMITATION_CLOSURE_CONTRACT_VERSION,
    FEATURE_ID as IDS_LIMITATION_CLOSURE_FEATURE_ID,
};
pub use local_evidence_surveillance_inference::{
    infer_local_evidence_surveillance, local_evidence_surveillance_manifest, EvidenceFeed1,
    EvidenceInferenceError, EvidenceObservation1, EvidenceState1, QualifiedEvidenceSet1,
    QualifiedEvidenceSet1Artifact,
    CONTRACT_VERSION as IDS_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
    FEATURE_ID as IDS_LOCAL_EVIDENCE_SURVEILLANCE_FEATURE_ID,
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
pub use performance_reliability_gateway::{
    assess_performance_reliability, performance_reliability_gateway_manifest, CapabilityWorkload4,
    CapabilityWorkloadRequest4, PerformanceReliabilityError, ReliableCapabilityArtifact6,
    ReliableCapabilityResult6, WorkloadEvidenceState,
    CONTRACT_VERSION as IDS_PERFORMANCE_RELIABILITY_CONTRACT_VERSION,
    FEATURE_ID as IDS_PERFORMANCE_RELIABILITY_FEATURE_ID,
};
pub use policy_autonomy_interoperability_gateway::{
    admit_policy_autonomy, policy_autonomy_interoperability_manifest, AutonomyActor8,
    AutonomyPolicyReceipt9, AutonomyPolicyReceipt9Artifact, AutonomyPolicyRequest7,
    PolicyAutonomyError, CONTRACT_VERSION as IDS_POLICY_AUTONOMY_CONTRACT_VERSION,
    FEATURE_ID as IDS_POLICY_AUTONOMY_FEATURE_ID,
};
pub use policy_autonomy_workbench::{
    operate_policy_autonomy, policy_autonomy_workbench_manifest, ActionAndAuthority4,
    ActionAndAuthorityRequest4, ActionEvidenceState, PolicyAutonomyWorkbenchError, PolicyReceipt5,
    PolicyReceiptArtifact5, CONTRACT_VERSION as IDS_POLICY_AUTONOMY_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as IDS_POLICY_AUTONOMY_WORKBENCH_FEATURE_ID,
};
pub use prospective_provenance_assurance::{
    assure_prospective_provenance, prospective_provenance_assurance_manifest,
    ArtifactAndDerivation3, ArtifactAndDerivationRequest3, DerivationEvidenceState,
    ProspectiveProvenanceError, SignedProvenanceEnvelope7, SignedProvenanceEnvelopeArtifact7,
    CONTRACT_VERSION as IDS_PROSPECTIVE_PROVENANCE_CONTRACT_VERSION,
    FEATURE_ID as IDS_PROSPECTIVE_PROVENANCE_FEATURE_ID,
};
pub use protocol_simulation_workbench::{
    protocol_workbench_manifest, simulate_protocol_workbench, ProtocolEvidenceState, ProtocolPeer5,
    ProtocolScenario5, ProtocolStage5, ProtocolWorkbenchArtifact9, ProtocolWorkbenchError,
    ProtocolWorkbenchReport9, ProtocolWorkbenchRequest5,
    CONTRACT_VERSION as IDS_PROTOCOL_SIMULATION_CONTRACT_VERSION,
    FEATURE_ID as IDS_PROTOCOL_SIMULATION_FEATURE_ID,
};
pub use provenance_signing_assurance::{
    assure_provenance_signing, provenance_signing_assurance_manifest, ProvenanceBundleRequest7,
    ProvenanceEvidenceState, ProvenanceNode8, ProvenanceSigningError, SignedProvenanceReceipt9,
    SignedProvenanceReceipt9Artifact, CONTRACT_VERSION as IDS_PROVENANCE_SIGNING_CONTRACT_VERSION,
    FEATURE_ID as IDS_PROVENANCE_SIGNING_FEATURE_ID,
};
pub use publication_research_object_release_control_plane::{
    compile_publication_release, publication_release_control_plane_manifest,
    PublicationReleaseError, ReleaseEvidenceState, ReleasePeer7, ResearchArtifact8,
    SignedResearchObject11, SignedResearchObject11Artifact, ValidatedResearchRun7,
    CONTRACT_VERSION as IDS_PUBLICATION_RELEASE_CONTRACT_VERSION,
    FEATURE_ID as IDS_PUBLICATION_RELEASE_FEATURE_ID,
};
pub use quality_control_assurance::{
    assure_quality_control, quality_control_manifest, QualityControlBatch4, QualityControlError,
    QualityControlReport8, QualityControlReport8Artifact, QualityEvidenceState,
    QualityObservation4, CONTRACT_VERSION as IDS_QUALITY_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as IDS_QUALITY_CONTROL_FEATURE_ID,
};
pub use reliability_copilot::{
    preflight_reliability, reliability_copilot_manifest, CapabilityWorkUnit8, CapabilityWorkload7,
    ReliabilityCopilotError, ReliabilityEvidenceState, ReliableCapabilityResult9,
    ReliableCapabilityResult9Artifact,
    CONTRACT_VERSION as IDS_RELIABILITY_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as IDS_RELIABILITY_COPILOT_FEATURE_ID,
};
pub use replication_negative_results_interoperability_gateway::{
    interoperate_replication, replication_interoperability_manifest, ClaimAndProtocol7,
    ClaimAndProtocol7Request, ReplicationEvidenceState, ReplicationInteroperabilityError,
    ReplicationObservation7, ReplicationPeer7, ReplicationRecord9, ReplicationRecord9Artifact,
    CONTRACT_VERSION as IDS_REPLICATION_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as IDS_REPLICATION_INTEROPERABILITY_FEATURE_ID,
};
pub use research_workbench::{
    compile_research_workbench, research_workbench_manifest, InteractiveResearchWorkspace9,
    InteractiveResearchWorkspace9Artifact, ResearchWorkbenchError, ResearchWorkspaceState7,
    WorkspaceEvidenceState, WorkspacePanel8,
    CONTRACT_VERSION as IDS_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as IDS_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use retrieval_synthesis_assurance_harness::{
    assure_retrieval_synthesis, retrieval_synthesis_assurance_manifest, EvidenceSynthesis11,
    EvidenceSynthesis11Artifact, RetrievalEvidence7, RetrievalEvidenceState, RetrievalPeer6,
    RetrievalSynthesisAssuranceError, ScopedRetrievalQuery6,
    CONTRACT_VERSION as IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_FEATURE_ID,
};
pub use scale_frontier_workflow::{
    preview_ids_scale_frontier, scale_frontier_manifest, CapacityEvidenceState, IdsCapacityReport9,
    IdsCapacityReport9Artifact, IdsScaleCell7, IdsScaleWorkload8, ScaleFrontierError,
    CONTRACT_VERSION as IDS_SCALE_FRONTIER_CONTRACT_VERSION,
    FEATURE_ID as IDS_SCALE_FRONTIER_FEATURE_ID,
};
pub use semantic_parity_contract::{
    evaluate_ids_semantic_parity, semantic_parity_manifest, IdsParityFixture8, IdsParityRequest7,
    IdsParityWitness9, IdsParityWitness9Artifact, ParityEvidenceState, SemanticParityError,
    CONTRACT_VERSION as IDS_SEMANTIC_PARITY_CONTRACT_VERSION,
    FEATURE_ID as IDS_SEMANTIC_PARITY_FEATURE_ID,
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
pub use typed_determinism_assurance::{
    assure_typed_determinism, typed_determinism_assurance_manifest, CanonicalCapabilityArtifact7,
    CanonicalCapabilityOutput7, CapabilityEvidenceState, CapabilityImplementation5,
    TypedCapabilityInput4, TypedDeterminismAssuranceError,
    CONTRACT_VERSION as IDS_TYPED_DETERMINISM_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as IDS_TYPED_DETERMINISM_ASSURANCE_FEATURE_ID,
};
pub use typed_determinism_interoperability_gateway::{
    negotiate_typed_determinism, typed_determinism_interoperability_manifest, DeterminismEndpoint6,
    DeterminismEvidenceState, TypedDeterminismError, TypedDeterminismReceipt8,
    TypedDeterminismReceipt8Artifact, TypedDeterminismRequest7,
    CONTRACT_VERSION as IDS_TYPED_DETERMINISM_CONTRACT_VERSION,
    FEATURE_ID as IDS_TYPED_DETERMINISM_FEATURE_ID,
};
pub mod federated_continual_identity_continuity_contract_model;
pub mod federated_continual_identity_continuity_inference;
pub mod federated_continual_identity_continuity_research_copilot;
pub mod federated_continual_identity_continuity_workflow_fabric;
pub mod identity_continuity_support;
pub mod local_identity_continuity_contract_model;
pub mod local_identity_continuity_inference;
pub mod local_identity_continuity_research_copilot;
pub mod local_identity_continuity_workflow_fabric;
pub mod multimodal_identity_continuity_contract_model;
pub mod multimodal_identity_continuity_inference;
pub mod multimodal_identity_continuity_research_copilot;
pub mod multimodal_identity_continuity_workflow_fabric;
pub mod throughput_identity_continuity_contract_model;
pub mod throughput_identity_continuity_inference;
pub mod throughput_identity_continuity_research_copilot;
pub mod throughput_identity_continuity_workflow_fabric;
pub use federated_continual_identity_continuity_contract_model::{
    ids_federated_continual_identity_continuity_contract_model_manifest,
    qualify_ids_federated_identity_continuity_contract,
};
pub use federated_continual_identity_continuity_inference::{
    ids_federated_continual_identity_continuity_inference_manifest,
    qualify_ids_federated_identity_continuity,
};
pub use federated_continual_identity_continuity_research_copilot::{
    ids_federated_continual_identity_continuity_research_copilot_manifest,
    qualify_ids_federated_identity_continuity_copilot,
};
pub use federated_continual_identity_continuity_workflow_fabric::{
    ids_federated_continual_identity_continuity_workflow_fabric_manifest,
    qualify_ids_federated_identity_continuity_workflow,
};
pub use identity_continuity_support::{
    IdentityAssertion4, IdentityContinuityArtifact4, IdentityContinuityCard7,
    IdentityContinuityError, IdentityContinuityRequest4,
};
pub use local_identity_continuity_contract_model::{
    ids_local_identity_continuity_contract_model_manifest,
    qualify_ids_local_identity_continuity_contract,
};
pub use local_identity_continuity_inference::{
    ids_local_identity_continuity_inference_manifest, qualify_ids_local_identity_continuity,
};
pub use local_identity_continuity_research_copilot::{
    ids_local_identity_continuity_research_copilot_manifest,
    qualify_ids_local_identity_continuity_copilot,
};
pub use local_identity_continuity_workflow_fabric::{
    ids_local_identity_continuity_workflow_fabric_manifest,
    qualify_ids_local_identity_continuity_workflow,
};
pub use multimodal_identity_continuity_contract_model::{
    ids_multimodal_identity_continuity_contract_model_manifest,
    qualify_ids_multimodal_identity_continuity_contract,
};
pub use multimodal_identity_continuity_inference::{
    ids_multimodal_identity_continuity_inference_manifest,
    qualify_ids_multimodal_identity_continuity,
};
pub use multimodal_identity_continuity_research_copilot::{
    ids_multimodal_identity_continuity_research_copilot_manifest,
    qualify_ids_multimodal_identity_continuity_copilot,
};
pub use multimodal_identity_continuity_workflow_fabric::{
    ids_multimodal_identity_continuity_workflow_fabric_manifest,
    qualify_ids_multimodal_identity_continuity_workflow,
};
pub use throughput_identity_continuity_contract_model::{
    ids_throughput_identity_continuity_contract_model_manifest,
    qualify_ids_throughput_identity_continuity_contract,
};
pub use throughput_identity_continuity_inference::{
    ids_throughput_identity_continuity_inference_manifest,
    qualify_ids_throughput_identity_continuity,
};
pub use throughput_identity_continuity_research_copilot::{
    ids_throughput_identity_continuity_research_copilot_manifest,
    qualify_ids_throughput_identity_continuity_copilot,
};
pub use throughput_identity_continuity_workflow_fabric::{
    ids_throughput_identity_continuity_workflow_fabric_manifest,
    qualify_ids_throughput_identity_continuity_workflow,
};
