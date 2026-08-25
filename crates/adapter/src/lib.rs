//! The data adapter contract.
//!
//! Implements blueprint 40.17 (Data Adapter Contract) and the ingestion half of section 28
//! (Biology Data and Standards) — specifically 28.00's "no silent coercion" list and 04.06's
//! conformance ladder.
//!
//! An adapter turns external bytes into local evidence sections in the sense of 43.04. The
//! hard part is not the turning; it is that the turning always loses something, and a pipeline
//! that does not say what it lost produces worlds that are confidently, undetectably wrong.
//! A table of tumour volumes ingested without its units, a variant table ingested without its
//! reference build, a gene column ingested without the version of the symbol mapping — each
//! yields facts that parse, validate, compile and answer queries, and each answers them wrong
//! in a way no downstream check can see, because the evidence needed to notice is exactly the
//! evidence that was dropped.
//!
//! Everything here follows from that.
//!
//! - [`Adapter::ingest`] returns an [`Ingestion`], which carries facts *and* a [`SemanticLoss`]
//!   in one sealed value. There is no way to return the facts alone.
//! - [`SemanticLoss::Lossless`] and [`SemanticLoss::Unaudited`] are different variants, because
//!   an empty list of losses conflates "we checked and nothing was dropped" with "nobody
//!   looked", and the conflation always favours the adapter that did not look.
//! - [`LossReport`] cannot be empty, so [`SemanticLoss::Lossy`] cannot smuggle the same
//!   conflation back in through a zero-length vector.
//! - Every loss entry names a [`SourceLocation`]. A count is not actionable; a locator is.
//! - [`conformance::certify`] checks determinism and loss completeness against an *independent*
//!   reading of the source from [`probe`], so an adapter cannot supply both sides of its own
//!   audit.
//!
//! # What is here
//!
//! [`TabularAdapter`] reads CSV and TSV under an explicit mapping policy, using the hand-rolled
//! reader in [`csv`] — there is no CSV crate in this workspace's dependency set, and a
//! permissive third-party reader would make "what did the source actually say" a question
//! about someone else's heuristics. [`InventoryAdapter`] walks a directory and produces facts
//! about files without interpreting their contents.
//!
//! # What is not here, and why
//!
//! DICOM, NIfTI/BIDS, AnnData/MuData, OME-Zarr, BAM/CRAM and VCF readers are absent by design,
//! not by omission. They belong to the Python adapter layer of 40.14. Their reference
//! implementations are the de-facto specification for those formats, and a second Rust reading
//! of a DICOM header would surface its disagreements as biology rather than as a parser bug.
//! This crate's job at that boundary is [`InventoryAdapter`]: establish that the bytes exist,
//! hash them so they stay addressable, and declare every unread byte as
//! [`LossKind::ContentUninterpreted`] so that the gap is in the record rather than in
//! somebody's memory.
//!
//! Also absent: credentials and fetching. A source arrives here as bytes the caller already
//! holds. Keeping network authority out of the adapter is what makes it safe to run an
//! untrusted mapping policy.
//!
//! # Example
//!
//! ```
//! use bioprism_adapter::{
//!     Adapter, ColumnRole, LossKind, Source, TabularAdapter, TabularProfile, VariableMapping,
//!     ValueType, conformance,
//! };
//!
//! let profile = TabularProfile::new("RG-DEMO-001")
//!     .scope("subject", "subject")
//!     .variable(
//!         "age",
//!         VariableMapping::new("age_at_diagnosis").typed(ValueType::Integer),
//!     );
//!
//! let source = Source::bytes("cohort", b"subject,age,comment\nS1,41,ok\n".to_vec())
//!     .with_format("text/csv");
//! let adapter = TabularAdapter::new(profile);
//!
//! let (report, ingestion) = conformance::certify(&adapter, &source).unwrap();
//! assert!(report.verified());
//! assert_eq!(ingestion.fact_count(), 1);
//!
//! // `comment` has no rule, so it is reported rather than dropped in silence.
//! assert!(ingestion.loss().kinds().contains(&LossKind::UnmappedColumn));
//! # let _ = ColumnRole::Provenance;
//! ```

pub mod adapter;
pub mod analysis_portfolio;
pub mod conformance;
pub mod context_assurance;
pub mod contract_frontier;
pub mod csv;
pub mod dependency_composition;
pub mod determinism_gateway;
pub mod error;
pub mod evaluation_assurance;
pub mod evidence_surveillance;
pub mod execution_control;
pub mod experiment_design_control;
pub mod fact;
pub mod federation_workflow;
pub mod ingestion;
pub mod ingestion_gateway;
pub mod instrument_mesh;
pub mod interoperability_gateway;
pub mod interpretation_assurance;
pub mod inventory;
pub mod knowledge_workflow;
pub mod limitation_closure;
pub mod location;
pub mod loss;
pub mod mechanism_control_plane;
pub mod multimodal_harmonization;
pub mod policy_gateway;
pub mod probe;
pub mod protocol_simulation;
pub mod provenance_assurance;
pub mod quality_control;
pub mod quality_drift;
pub mod quality_envelope;
pub mod registry;
pub mod release_assurance;
pub mod reliability_copilot;
pub mod replication_assurance;
pub mod research_ingest;
pub mod research_workbench;
pub mod resource_workbench;
pub mod retrieval_synthesis;
pub mod scale_frontier;
pub mod semantic_parity;
pub mod source;
pub mod tabular;

pub use adapter::{Adapter, AdapterManifest, ConformanceLevel};
pub use analysis_portfolio::{
    qualify_analysis_portfolio, AnalysisCandidate, AnalysisPortfolioError,
    AnalysisPortfolioReceipt, AnalysisPortfolioRequest, AnalysisPortfolioVerdict, AnalysisQuestion,
    IdentificationStatus, CONTRACT_VERSION as ANALYSIS_PORTFOLIO_CONTRACT_VERSION,
    FEATURE_ID as ANALYSIS_PORTFOLIO_FEATURE_ID,
};
pub use conformance::{certify, Check, CheckOutcome, ConformanceReport, Status};
pub use context_assurance::{
    assure_context_compilation, ContextCompilationDisposition, ContextCompilationError,
    ContextCompilationReceipt, ContextCompilationRequest, DecisionQuery,
    CONTRACT_VERSION as CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_COMPILATION_FEATURE_ID,
};
pub use contract_frontier::{
    compile_adapter_capability_manifest, AdapterCapabilityManifest, AdapterContractInput,
    ContractFrontierError, ManifestDisposition,
    COMPATIBLE_CONTRACT_VERSION as CONTRACT_FRONTIER_COMPATIBLE_VERSION,
    CONTRACT_VERSION as CONTRACT_FRONTIER_CONTRACT_VERSION,
    CURRENT_CONTRACT_VERSION as CONTRACT_FRONTIER_CURRENT_VERSION,
    FEATURE_ID as CONTRACT_FRONTIER_FEATURE_ID,
};
pub use csv::Table;
pub use dependency_composition::{
    infer_adapter_dependency_composition, AdapterCompositionReceipt, AdapterCompositionRequest,
    AdapterDependencyComponent, CompositionDisposition, DependencyCompositionError,
    CONTRACT_VERSION as DEPENDENCY_COMPOSITION_CONTRACT_VERSION,
    FEATURE_ID as DEPENDENCY_COMPOSITION_FEATURE_ID,
};
pub use determinism_gateway::{
    negotiate_capability, CanonicalCapabilityOutput, DeterminismGatewayError,
    DeterminismGatewayVerdict, TypedCapabilityInput,
    CONTRACT_VERSION as DETERMINISM_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as DETERMINISM_GATEWAY_FEATURE_ID,
};
pub use error::{AdapterError, CsvError};
pub use evaluation_assurance::{
    assure_evaluation_run, AssuranceVerdict, AssuranceWitness, CapabilityRun,
    EvaluationAssuranceError, EvaluationAssuranceReceipt, MetricObservation,
    CONTRACT_VERSION as EVALUATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as EVALUATION_ASSURANCE_FEATURE_ID,
};
pub use evidence_surveillance::{
    run_evidence_surveillance, EffectReceipt, EvidenceFeedItem, EvidenceFeedRequest,
    EvidenceSurveillanceDisposition, EvidenceSurveillanceError, EvidenceSurveillanceReceipt,
    QualifiedEvidenceSet, CONTRACT_VERSION as EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_SURVEILLANCE_FEATURE_ID,
};
pub use execution_control::{
    admit_computational_execution, AuthorizedExecutionEffect, ComputationalExecutionReceipt,
    ComputationalExecutionRequest, ExecutionAdmissionMode, ExecutionControlDecision,
    ExecutionControlError, CONTRACT_VERSION as EXECUTION_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as EXECUTION_CONTROL_FEATURE_ID,
};
pub use experiment_design_control::{
    compile_experiment_design, DesignDecision, DesignSite, ExperimentAssignment,
    ExperimentDesignError, ExperimentDesignReceipt, ExperimentObjective,
    FederatedExperimentDesignRequest,
    CONTRACT_VERSION as EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as EXPERIMENT_DESIGN_CONTROL_FEATURE_ID,
};
pub use fact::{FactDraft, ValueQualifiers};
pub use federation_workflow::{
    schedule_federation_workflow, FederationRequest, FederationTask, FederationWorkflowDecision,
    FederationWorkflowError, FederationWorkflowReceipt,
    CONTRACT_VERSION as FEDERATION_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as FEDERATION_WORKFLOW_FEATURE_ID,
};
pub use ingestion::Ingestion;
pub use ingestion_gateway::{
    run_ingestion_gateway, IngestionEffectReceipt, IngestionGatewayDecision, IngestionGatewayError,
    IngestionGatewayReceipt, IngestionGatewayRequest, RawModalityBundle,
    CONTRACT_VERSION as INGESTION_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as INGESTION_GATEWAY_FEATURE_ID,
};
pub use instrument_mesh::{
    integrate_instrument_mesh, InstrumentActionRequest, InstrumentCapability,
    InstrumentEffectReceipt, InstrumentMeshDecision, InstrumentMeshError, InstrumentMeshReceipt,
    CONTRACT_VERSION as INSTRUMENT_MESH_CONTRACT_VERSION, FEATURE_ID as INSTRUMENT_MESH_FEATURE_ID,
};
pub use interoperability_gateway::{
    negotiate_interoperability, ExternalCapability, InteroperabilityDisposition,
    InteroperabilityGatewayError, InteroperabilityRequest, NegotiatedIntegration,
    COMPATIBLE_CONTRACT_VERSION as INTEROPERABILITY_COMPATIBLE_CONTRACT_VERSION,
    CONTRACT_VERSION as INTEROPERABILITY_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as INTEROPERABILITY_GATEWAY_FEATURE_ID,
    TARGET_CONTRACT_VERSION as INTEROPERABILITY_TARGET_CONTRACT_VERSION,
};
pub use interpretation_assurance::{
    assure_interpretation, EvidenceBackedResult, InterpretationAssuranceError,
    InterpretationAssuranceReceipt, InterpretationClaim, InterpretationVerdict,
    CONTRACT_VERSION as INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_ASSURANCE_FEATURE_ID,
};
pub use inventory::{InventoryAdapter, InventoryProfile};
pub use knowledge_workflow::{
    run_knowledge_workflow, ClaimsWorkflowRequest, KnowledgeWorkflowDisposition,
    KnowledgeWorkflowError, KnowledgeWorkflowReceipt, TypedKnowledgeWorld,
    CONTRACT_VERSION as KNOWLEDGE_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as KNOWLEDGE_WORKFLOW_FEATURE_ID,
};
pub use limitation_closure::{
    close_adapter_limitations, AdapterClosureReceipt, AdapterLimitationCase, ClosureDisposition,
    LimitationClosureError, LimitationClosureRequest, LimitationStatus,
    CONTRACT_VERSION as LIMITATION_CLOSURE_CONTRACT_VERSION,
    FEATURE_ID as LIMITATION_CLOSURE_FEATURE_ID,
};
pub use location::{LocationSet, SourceLocation};
pub use loss::{LossAudit, LossEntry, LossKind, LossReport, LossSeverity, SemanticLoss};
pub use mechanism_control_plane::{
    operate_mechanism_control_plane, MechanismControlDisposition, MechanismControlError,
    MechanismControlPlaneReceipt, MechanismControlPlaneRequest,
    CONTRACT_VERSION as MECHANISM_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_CONTROL_PLANE_FEATURE_ID,
};
pub use multimodal_harmonization::{
    harmonize_multimodal, HarmonizationDecision, HarmonizationError, HarmonizedResearchObject,
    ModalityManifest, MultimodalHarmonizationRequest,
    FEATURE_ID as MULTIMODAL_HARMONIZATION_FEATURE_ID,
    FEATURE_VERSION as MULTIMODAL_HARMONIZATION_FEATURE_VERSION,
};
pub use policy_gateway::{
    admit_policy_action, ActionAndAuthority, PolicyGatewayDecision, PolicyGatewayError,
    PolicyGatewayReceipt, CONTRACT_VERSION as POLICY_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as POLICY_GATEWAY_FEATURE_ID,
};
pub use probe::{field_inventory, Inventory};
pub use protocol_simulation::{
    simulate_protocol_draft, ProtocolDraft, ProtocolOperation, ProtocolScenario,
    ProtocolScenarioResult, ProtocolSimulationError, ProtocolSimulationReceipt,
    ProtocolSimulationState, ProtocolStep,
    CONTRACT_VERSION as PROTOCOL_SIMULATION_CONTRACT_VERSION,
    FEATURE_ID as PROTOCOL_SIMULATION_FEATURE_ID,
};
pub use provenance_assurance::{
    assure_provenance, ArtifactAndDerivation, DerivationStep, ProvenanceArtifact,
    ProvenanceAssuranceError, ProvenanceAssuranceVerdict, SignedProvenanceEnvelope,
    CONTRACT_VERSION as PROVENANCE_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as PROVENANCE_ASSURANCE_FEATURE_ID,
};
pub use quality_control::{
    evaluate_quality_control, manifest as quality_control_manifest, MetricDirection, MetricStatus,
    QualityControlError, QualityControlReceipt, QualityControlRequest, QualityControlSummary,
    QualityDisposition, QualityMetric,
};
pub use quality_drift::{
    evaluate_quality_drift, quality_drift_manifest, DriftDisposition, DriftMetric,
    DriftMetricResult, DriftMetricStatus, QualityDriftError, QualityDriftPolicy,
    QualityDriftReceipt, QualityDriftRequest, QualityDriftSummary,
    FEATURE_ID as QUALITY_DRIFT_FEATURE_ID, FEATURE_VERSION as QUALITY_DRIFT_FEATURE_VERSION,
};
pub use quality_envelope::{
    evaluate_quality_envelope, QualityEnvelopeDecision, QualityEnvelopeError,
    QualityEnvelopeReceipt, QualityEnvelopeRequest, StudyQualityRecord, StudyQualityVerdict,
    CONTRACT_VERSION as QUALITY_ENVELOPE_CONTRACT_VERSION,
    FEATURE_ID as QUALITY_ENVELOPE_FEATURE_ID,
};
pub use registry::{
    AdapterDescriptor, AdapterExecution, AdapterPlan, AdapterPlanCandidate, AdapterPlanRequest,
    AdapterRegistry, PlanStatus, RegistryError, SourceKind, ADAPTER_REGISTRY_SCHEMA_VERSION,
};
pub use release_assurance::{
    assure_release, ReleaseAssuranceError, ReleaseAssuranceReceipt, ReleaseAssuranceVerdict,
    ReleaseStudyManifest, ValidatedResearchRun,
    CONTRACT_VERSION as RELEASE_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as RELEASE_ASSURANCE_FEATURE_ID,
};
pub use reliability_copilot::{
    plan_reliable_capability, CapabilityWorkload, ReliabilityCopilotError, ReliabilityDecision,
    ReliableCapabilityResult, ToolInvocation, ToolManifest,
    CONTRACT_VERSION as RELIABILITY_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as RELIABILITY_COPILOT_FEATURE_ID,
};
pub use replication_assurance::{
    assure_replication, ClaimAndProtocol, ReplicationAssuranceError, ReplicationAssuranceReceipt,
    ReplicationAssuranceRequest, ReplicationObservation, ReplicationOutcome, ReplicationVerdict,
    CONTRACT_VERSION as REPLICATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as REPLICATION_ASSURANCE_FEATURE_ID,
};
pub use research_ingest::{
    certify_research_ingest, ResearchIngestionBundle, ResearchIngestionError,
};
pub use research_workbench::{
    compile_research_workbench, ComparabilityStatus, InteractiveResearchWorkspace,
    ResearchWorkbenchError, ResearchWorkspaceState, StudyWorkspaceEntry, WorkspaceDisposition,
    WorkspaceViewRequest, CONTRACT_VERSION as RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use resource_workbench::{
    discover_resources, QualifiedResource, ResourceCandidate, ResourceNeed, ResourceOmission,
    ResourceWorkbenchDisposition, ResourceWorkbenchError, ResourceWorkbenchReceipt,
    CONTRACT_VERSION as RESOURCE_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as RESOURCE_WORKBENCH_FEATURE_ID,
};
pub use retrieval_synthesis::{
    compile_evidence_synthesis, EvidenceSynthesis, EvidenceSynthesisDisposition,
    EvidenceSynthesisRequest, RetrievalCandidate, RetrievalSynthesisError,
    RetrievalSynthesisReceipt, ScopedRetrievalQuery, SynthesisEffectReceipt,
    CONTRACT_VERSION as RETRIEVAL_SYNTHESIS_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
pub use scale_frontier::{
    plan_adapter_scale_frontier, ScaleDisposition, ScaleFrontierError, ScaleFrontierReceipt,
    ScaleFrontierRequest, ScaleScenario,
    CONTRACT_VERSION as ADAPTER_SCALE_FRONTIER_CONTRACT_VERSION,
    FEATURE_ID as ADAPTER_SCALE_FRONTIER_FEATURE_ID,
};
pub use semantic_parity::{
    evaluate_adapter_semantic_parity, AdapterSemanticParityReceipt, AdapterSemanticParityRequest,
    AdapterSemanticReport, SemanticParityDisposition, SemanticParityError,
    CONTRACT_VERSION as ADAPTER_SEMANTIC_PARITY_CONTRACT_VERSION,
    FEATURE_ID as ADAPTER_SEMANTIC_PARITY_FEATURE_ID,
};
pub use source::{Locator, Source, SourceManifest, SourceProvenance};
pub use tabular::{
    ColumnRole, FramePolicy, OntologyPolicy, TabularAdapter, TabularProfile, TypePolicy,
    UnitPolicy, ValueType, VariableMapping,
};
