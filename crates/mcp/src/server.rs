//! The MCP server.
//!
//! Implements blueprint 11.11 (MCP server and agent tools) over the FIBER compiler. Four of that
//! module's requirements are structural here rather than advisory:
//!
//! * **Progressive disclosure.** `fiber_compile` returns the L0 contract and a refinement handle,
//!   never the whole section. An agent asks for evidence when it decides it needs evidence.
//! * **Least authority.** Every path is resolved inside a root directory chosen at startup.
//!   Traversal outside it is refused, so an agent cannot turn the server into a file reader.
//! * **Side-effect preview.** Tools that write require an explicit `confirm`, and describe what
//!   they would do without it.
//! * **Audit trail.** Every call is written to stderr as a structured record, keeping stdout a
//!   clean JSON-RPC channel.
//!
//! Omissions travel with every response at every layer. An agent that reads only L0 still learns
//! how much was excluded and whether the sufficiency claim holds.

use crate::brain_control::BrainControlState;
use crate::federated_quality_control_assurance::{
    CONTRACT_VERSION as FEDERATED_QUALITY_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_QUALITY_CONTROL_FEATURE_ID,
};
use crate::rpc::{code, Request, Response};
use crate::tool_definitions::find_tool_definition;
pub use crate::tool_definitions::tool_definitions;
use bioprism_adapter::{
    certify, AdapterPlanRequest, AdapterRegistry, Source, SourceProvenance, TabularAdapter,
    TabularProfile,
};
use bioprism_adaptive::{
    AdaptivePanel, Candidate as AdaptiveCandidate, CapabilityId as AdaptiveCapabilityId,
};
use bioprism_atlas::{
    composite as atlas_composite, Atlas, CapabilityId, CoverageReport, FailureRecord,
    WeightingPolicy,
};
use bioprism_atlashub::{CiReport, ResultUnderReview, FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID};
use bioprism_atlasx::{
    audit as atlasx_audit, browse_with_visibility as atlasx_browse_with_visibility, named_in_scope,
    DebtStatement as AtlasxDebtStatement, Facet as AtlasxFacet, Surface as AtlasxSurface,
    Visibility as AtlasxVisibility, ATLASX_MECHANISM_CONTRACT_VERSION, ATLASX_MECHANISM_FEATURE_ID,
    ATLASX_SCHEMA_VERSION, DEFINED_HERE, NAMED_NEVER_DEFINED,
};
use bioprism_backends::{Budget as InfluenceBudget, QueryRegion, RegionFactor};
use bioprism_benchcompiler::counterfactual::pair as benchmark_counterfactual_pair;
use bioprism_benchcompiler::{
    analyse as analyse_benchmark, assess_contamination as benchmark_assess_contamination,
    assign_holdout as benchmark_assign_holdout, boundaries as benchmark_boundaries,
    calibrate as benchmark_calibrate, compile as benchmark_compile, contrast as benchmark_contrast,
    deduplicate as benchmark_deduplicate, effective_diversity as benchmark_effective_diversity,
    episodes as benchmark_episodes, failure_card as benchmark_failure_card,
    repetitions as benchmark_repetitions, Assertion as BenchmarkAssertion,
    BenchInstance as BenchmarkBenchInstance, CandidateAction as BenchmarkCandidateAction,
    CandidateActionSet as BenchmarkCandidateActionSet, Compilation as BenchmarkCompilation,
    ConstraintRecord as BenchmarkConstraintRecord, ContaminationRisk as BenchmarkContaminationRisk,
    ContextItem as BenchmarkContextItem, ExpectedResponse as BenchmarkExpectedResponse,
    ExposureLedger as BenchmarkExposureLedger, Instance as BenchmarkInstance,
    InterestSignature as BenchmarkInterestSignature, Intervention as BenchmarkIntervention,
    LeakProbe as BenchmarkLeakProbe, MinimizeBudget as BenchmarkMinimizeBudget,
    NoRealismReview as BenchmarkNoRealismReview, PanelRun as BenchmarkPanelRun,
    ProposedOracle as BenchmarkProposedOracle, SYNTHESIS_ORDER as BENCHMARK_SYNTHESIS_ORDER,
};
use bioprism_bioethics::action::{refer as refer_physical_action, ActionPlan, Authorisation};
use bioprism_bioethics::dualuse::{refer as refer_dual_use, CapabilityRelease};
use bioprism_bioethics::humansubject::{screen as screen_human_subject, StudyDescription};
use bioprism_bioethics::representation::{
    attribute as attribute_representation, summarise, ContextAxis, StratumObservation,
};
use bioprism_bioethics::validation::ValidationDossier;
use bioprism_bioethics::{
    EVIDENCE_SURVEILLANCE_ASSURANCE_CONTRACT_VERSION, EVIDENCE_SURVEILLANCE_ASSURANCE_FEATURE_ID,
};
use bioprism_bioeval::{Dispersion, ReferenceDistribution, ReferenceStandard};
use bioprism_bioevalx::acquisition::{
    AcquisitionKind, Action as AcquisitionTraceAction, Obligation as AcquisitionObligation,
    ReferencePolicy as AcquisitionReferencePolicy, Trace as AcquisitionTrace,
};
use bioprism_bioevalx::boundary::{
    Assessment as BioevalBoundaryAssessment, Flow as BioevalFlow, Policy as BioevalBoundaryPolicy,
};
use bioprism_bioevalx::burden::{
    BranchLedger as BioevalBranchLedger, Draw as BioevalDraw, Ledger as BioevalLedger,
    Resource as BioevalResource,
};
use bioprism_bioevalx::design::{
    Arm as BioevalDesignArm, FactorialDesign as BioevalFactorialDesign,
};
use bioprism_bioevalx::estimand::{
    ClaimKind as BioevalClaimKind, Corroboration as BioevalCorroboration,
    Estimand as BioevalEstimand, Evidentiary as BioevalEvidentiary, Finding as BioevalFinding,
    Identification as BioevalIdentification,
};
use bioprism_bioevalx::evaluator::{
    EvaluatorRun as BioevalEvaluatorRun, Panel as BioevalEvaluatorPanel,
    TaskOutcome as BioevalTaskOutcome,
};
use bioprism_bioevalx::grounding::{
    ClaimState as GroundingClaimState, EdgeKind as GroundingEdgeKind,
    Evidence as GroundingEvidence, Grounding, LocatorStatus as GroundingLocatorStatus,
    SupportEdge as GroundingSupportEdge,
};
use bioprism_bioevalx::mesh::{
    Disagreement as BioevalDisagreement, EvaluatorDecl as BioevalEvaluatorDecl,
    EvaluatorVerdict as BioevalEvaluatorVerdict, Mesh as BioevalMesh,
};
use bioprism_bioevalx::metamorphic::{
    verdict as bioeval_metamorphic_verdict, Family as BioevalMetamorphicFamily,
    Relation as BioevalMetamorphicRelation, Suite as BioevalMetamorphicSuite,
    Trial as BioevalMetamorphicTrial, TrialVerdict as BioevalTrialVerdict,
};
use bioprism_bioevalx::plane::{
    Cell as BioevalCell, FoldPolicy as BioevalFoldPolicy, ScorePlane as BioevalScorePlane,
};
use bioprism_bioevalx::reveal::{
    Commitment as BioevalCommitment, Outcome as BioevalOutcome, Registration as BioevalRegistration,
};
use bioprism_bioevalx::waiver::{
    Gate as BioevalReleaseGate, ReleaseDecision as BioevalReleaseDecision,
    Waiver as BioevalReleaseWaiver,
};
use bioprism_bioevalx::{OutputVerdict, Reexecution, Trajectory, Worldline as EvaluationWorldline};
use bioprism_biolang::{compile as compile_bioql, QuerySchema};
use bioprism_bioworlds::SliceCatalog;
use bioprism_brain::{
    assemble_prompt, plan_autonomous, select_bandit_arm, select_bandit_arm_contextual,
    select_model, select_model_contextual, update_bandit, AutonomousPlanRequest, BanditState,
    BanditUpdate, ContextualModelSelectionRequest, ModelSelectionRequest, PromptAssemblyRequest,
};
use bioprism_bundle::{
    KeyRegistry, PubliclyAttestedBundle, ResultBundle, TrustPolicy, VerificationKey,
};
use bioprism_choreography::{
    ExplorationBound, GlobalType, System as ChoreographySystem, WellFormedGlobal,
};
use bioprism_conformance::fiber_suite::workspace_fixture_root;
use bioprism_conformance::{
    assess as assess_conformance, fiber_suite, shipped_baseline, FiberReference, FixtureStore,
};
use bioprism_cookbook::{standard_cookbook, Workspace as CookbookWorkspace};
use bioprism_dataops::{
    declared_objective_names, parity as topology_parity, reference_local, reference_team,
    DataClass, Plane, TenantPattern,
};
use bioprism_devplat::{
    apply_binding, audit_ci_execution_evidence, audit_ci_provider_evidence,
    audit_domain_decision_readiness, audit_domain_evidence_provider_external_payload_execution,
    audit_domain_evidence_provider_external_payload_lineage, audit_execution_provenance,
    build_dashboard, build_delivery_receipt, build_domain_acquisition_catalogue,
    build_domain_workflow_catalogue, build_domain_workflow_portfolio,
    build_workflow_execution_evidence, classify_domain_report_bridge,
    execute_domain_evidence_source_with_http_policy, handoff_domain_evidence_provider,
    instantiate_domain_workflow, mission_claim_lineage_with_review, normalize_ci_provider_payload,
    normalize_domain_evidence_provider, normalize_domain_evidence_provider_external_payload,
    plan_domain_evidence_source, plan_mission, query_adapter_execution_evidence,
    query_domain_evidence_provider_external_payload_evidence, reconcile_domain_workflow,
    record_adapter_execution_evidence, record_domain_evidence_provider_external_payload,
    run_workbench, scaffold_domain_workflow, standard_walkthroughs,
    validate_domain_decision_readiness, validate_workflow_execution_evidence,
    verify_delivery_receipt, verify_domain_evidence_provider_external_payload_replay,
    verify_domain_evidence_provider_replay, verify_domain_workflow_portfolio,
    verify_mission_evidence_bundle, verify_workbench, AdapterExecutionEvidenceQueryRequest,
    AdapterExecutionEvidenceRequest, ArtifactRegistry, CapabilityCatalogue,
    CapabilityDashboardQuery, CapabilityQuery, CapabilityRouteRequest, CiExecutionEvidenceRequest,
    CiProviderEvidenceRegistry, CiProviderEvidenceRequest, CiProviderNormalizationRequest,
    DeliveryReceiptRequest, DeliveryReceiptVerificationRequest, DevPlatReport,
    DomainAcquisitionQuery, DomainEvidenceProviderExternalPayloadEvidenceQueryRequest,
    DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest,
    DomainEvidenceProviderExternalPayloadLineageAuditRequest,
    DomainEvidenceProviderExternalPayloadNormalizationRequest,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderExternalPayloadReplayRequest, DomainEvidenceProviderHandoffRequest,
    DomainEvidenceProviderNormalizationRequest, DomainEvidenceProviderReplayRequest,
    DomainEvidenceSourceHttpPolicy, DomainWorkflowReconciliationRegistry, EngineeringManifest,
    EngineeringPlanRequest, EvidenceBundleRegistry, ExecutionProvenanceRequest,
    MissionEvaluatorCatalogue, MissionEvaluatorQuery, MissionEvaluatorReplayCompareRequest,
    MissionEvaluatorReplayRequest, MissionEvaluatorReviewRequest, MissionReport, MissionRequest,
    MissionStep, MissionStepResult, MissionTraceEvent, MissionTraceObserver,
    OperationalReadinessManifest, ReleasePipelineManifest, SandboxManifest, SandboxRuntimeManifest,
    SecurityPrivacyManifest, SecurityProgramManifest, WorkbenchReportRegistry, WorkbenchRequest,
    WorkbenchVerificationRequest, WorkflowExecutionEvidenceQuery,
    WorkflowExecutionEvidenceRegistry, ADAPTER_DOMAIN_REPORT_SCHEMA_VERSION,
    ADAPTER_DOMAIN_REPORT_WORKFLOW, CAPABILITY_SCHEMA_VERSION,
    DEVPLAT_MULTIMODAL_LIMITATION_CLOSURE_CONTRACT_VERSION,
    DEVPLAT_MULTIMODAL_LIMITATION_CLOSURE_FEATURE_ID, DOMAIN_ACQUISITION_SCHEMA_VERSION,
    DOMAIN_ACQUISITION_WORKFLOW, DOMAIN_DECISION_READINESS_SCHEMA_VERSION,
    DOMAIN_DECISION_READINESS_WORKFLOW, DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW, DOMAIN_EVIDENCE_INTAKE_COVERAGE_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_INTAKE_COVERAGE_WORKFLOW, DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_INTAKE_WORKFLOW, DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW, DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW, DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW, DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA,
    DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW, DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW, DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
    DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW, DOMAIN_REPORT_COVERAGE_SCHEMA_VERSION,
    DOMAIN_REPORT_COVERAGE_WORKFLOW, DOMAIN_REPORT_PROJECT_SCHEMA_VERSION,
    DOMAIN_REPORT_PROJECT_WORKFLOW, DOMAIN_REPORT_SCHEMA_VERSION, ENGINEERING_AUDIT_SCHEMA,
    ENGINEERING_PLAN_AUDIT_SCHEMA, MAX_EVIDENCE_REGISTRY_QUERY_ITEMS,
    MISSION_EVALUATOR_SCHEMA_VERSION, MISSION_SCHEMA_VERSION, OPERATIONAL_READINESS_AUDIT_SCHEMA,
    PROVIDER_DOMAIN_REPORT_SCHEMA_VERSION, PROVIDER_DOMAIN_REPORT_WORKFLOW,
    RELEASE_PIPELINE_AUDIT_SCHEMA, SANDBOX_AUDIT_SCHEMA, SANDBOX_RUNTIME_AUDIT_SCHEMA,
    SECURITY_PRIVACY_AUDIT_SCHEMA, SECURITY_PROGRAM_AUDIT_SCHEMA, WORKBENCH_SCHEMA_VERSION,
    WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA_VERSION, WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION,
    WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW,
};
use bioprism_devx::{
    audit as devx_audit, lint_catalogue, workspace_contract,
    CONTEXT_COMPILATION_CONTRACT_FEATURE_ID, CONTEXT_COMPILATION_CONTRACT_VERSION,
};
use bioprism_docgraph::{
    compile_bundle, impact_of, lint, scan_markdown_tree, DocEdgeType, ModuleId, NodeStatus,
    ScanOptions, TaskRoute, TraversalPolicy,
};
use bioprism_domain::DomainPack;
use bioprism_epistemic::submodularity::check_with_tolerance as epistemic_submodularity_check;
use bioprism_epistemic::{
    adaptive_policy as epistemic_adaptive_policy,
    adaptive_policy_with_cost_vectors as epistemic_vector_adaptive_policy,
    brute_force_optimum as epistemic_brute_force_optimum,
    complementarity as epistemic_complementarity,
    decision_equivalence_quotient as epistemic_decision_equivalence_quotient,
    evaluate_context as epistemic_evaluate_context, frontier as epistemic_frontier,
    greedy as epistemic_greedy, identification as epistemic_identification,
    joint_value as epistemic_joint_value, lazy_greedy as epistemic_lazy_greedy,
    minimal_sufficient_context as epistemic_minimal_sufficient_context,
    value_of_information as epistemic_value_of_information, Acquisition as EpistemicAcquisition,
    AdaptiveExecutionReceipt as EpistemicAdaptiveExecutionReceipt,
    AdaptiveNode as EpistemicAdaptiveNode, AdaptivePlan as EpistemicAdaptivePlan,
    Belief as EpistemicBelief, Constraint as EpistemicConstraint,
    CostVector as EpistemicCostVector, CostWeights as EpistemicCostWeights,
    CostedAcquisition as EpistemicCostedAcquisition, DecisionProblem as EpistemicDecisionProblem,
    DistortionCriterion as EpistemicDistortionCriterion, EvidencePool as EpistemicEvidencePool,
    ExecutionGrant as EpistemicExecutionGrant, Outcome as EpistemicOutcome,
    RegretReduction as EpistemicRegretReduction, ScriptedExecutor as EpistemicScriptedExecutor,
    SetFunction as EpistemicSetFunction, ADAPTIVE_EXECUTION_SCHEMA,
};
use bioprism_epistemic::{
    RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_CONTRACT_VERSION,
    RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_FEATURE_ID,
};
use bioprism_evalengine::attribute as bioeval_design_attribute;
use bioprism_evalengine::{
    CapabilityPosterior, CreditPolicy, Observation, ReleaseGate as EvalReleaseGate,
};
use bioprism_fabric::synth::{
    synthesize as synthesize_fabric, unimplemented_stages, Candidate as FabricCandidate,
    Goal as FabricGoal,
};
use bioprism_factory::{ExecutionAuthoritySnapshot, Job as FactoryJob, JobStore, WorkerCapability};
use bioprism_factory::{PROSPECTIVE_EVIDENCE_CONTRACT_VERSION, PROSPECTIVE_EVIDENCE_FEATURE_ID};
use bioprism_fiber::{
    compile, compile_with_oracle, AdaptiveAcquisitionTrace, DecisionOracle, Query,
};
use bioprism_fiber::{FEDERATED_RESOURCE_CONTRACT_VERSION, FEDERATED_RESOURCE_FEATURE_ID};
use bioprism_foundation::contract::{ContractDraft, FalsifiableContract};
use bioprism_foundation::maturity::ApplicabilityEnvelope;
use bioprism_foundation::worldclass::{BioWorldDeclaration, CounterfactualClaim, Transition};
use bioprism_governance::known as known_schemas;
use bioprism_graph::{project_all as project_graph_bundle, ProjectionSource};
use bioprism_hub::{
    accept as accept_hub_submission, BioAtlasCard, Board as HubBoard, ContaminationWitness,
    Decision as HubDecision, DisclosureLedger, Entry as HubEntry, Epoch as HubEpoch,
    ModerationLedger, ModerationState, Score as HubScore, SubmissionDraft, SubmissionId, Submitter,
    VerificationStatus,
};
use bioprism_hubapi::{
    resolve_dependencies as hub_resolve_dependencies, resolve_in as hub_resolve_in,
    search as hub_search_query, Catalog as HubCatalog, Federation as HubFederation,
    Query as HubQuery, Request as HubRequest,
};
use bioprism_ids::{
    IDS_ADVERSARIAL_RECOVERY_CONTRACT_VERSION, IDS_ADVERSARIAL_RECOVERY_FEATURE_ID,
};
use bioprism_ids::{IDS_BOUNDED_EVOLUTION_CONTRACT_VERSION, IDS_BOUNDED_EVOLUTION_FEATURE_ID};
use bioprism_ids::{
    IDS_COMPUTATIONAL_EXECUTION_CONTRACT_VERSION, IDS_COMPUTATIONAL_EXECUTION_FEATURE_ID,
};
use bioprism_ids::{IDS_CONTEXT_COMPILATION_CONTRACT_VERSION, IDS_CONTEXT_COMPILATION_FEATURE_ID};
use bioprism_ids::{IDS_CONTRACT_FRONTIER_CONTRACT_VERSION, IDS_CONTRACT_FRONTIER_FEATURE_ID};
use bioprism_ids::{
    IDS_DEPENDENCY_COMPOSITION_CONTRACT_VERSION, IDS_DEPENDENCY_COMPOSITION_FEATURE_ID,
};
use bioprism_ids::{
    IDS_EVALUATION_ASSURANCE_CONTRACT_VERSION, IDS_EVALUATION_ASSURANCE_FEATURE_ID,
};
use bioprism_ids::{IDS_EXPERIMENT_DESIGN_CONTRACT_VERSION, IDS_EXPERIMENT_DESIGN_FEATURE_ID};
use bioprism_ids::{IDS_FEDERATED_COMMONS_CONTRACT_VERSION, IDS_FEDERATED_COMMONS_FEATURE_ID};
use bioprism_ids::{IDS_FEDERATED_WORKFLOW_CONTRACT_VERSION, IDS_FEDERATED_WORKFLOW_FEATURE_ID};
use bioprism_ids::{IDS_FEDERATION_SECURITY_CONTRACT_VERSION, IDS_FEDERATION_SECURITY_FEATURE_ID};
use bioprism_ids::{
    IDS_INTEROPERABILITY_EXTENSIBILITY_CONTRACT_VERSION,
    IDS_INTEROPERABILITY_EXTENSIBILITY_FEATURE_ID,
};
use bioprism_ids::{
    IDS_INTEROPERABILITY_GATEWAY_CONTRACT_VERSION, IDS_INTEROPERABILITY_GATEWAY_FEATURE_ID,
};
use bioprism_ids::{
    IDS_KNOWLEDGE_REPRESENTATION_CONTRACT_VERSION, IDS_KNOWLEDGE_REPRESENTATION_FEATURE_ID,
};
use bioprism_ids::{
    IDS_LABORATORY_INTEGRATION_CONTRACT_VERSION, IDS_LABORATORY_INTEGRATION_FEATURE_ID,
};
use bioprism_ids::{IDS_LIMITATION_CLOSURE_CONTRACT_VERSION, IDS_LIMITATION_CLOSURE_FEATURE_ID};
use bioprism_ids::{
    IDS_MECHANISM_EXPLORATION_CONTRACT_VERSION, IDS_MECHANISM_EXPLORATION_FEATURE_ID,
};
use bioprism_ids::{
    IDS_MULTIMODAL_INGESTION_CONTRACT_VERSION, IDS_MULTIMODAL_INGESTION_FEATURE_ID,
};
use bioprism_ids::{
    IDS_PERFORMANCE_RELIABILITY_CONTRACT_VERSION, IDS_PERFORMANCE_RELIABILITY_FEATURE_ID,
};
use bioprism_ids::{IDS_POLICY_AUTONOMY_CONTRACT_VERSION, IDS_POLICY_AUTONOMY_FEATURE_ID};
use bioprism_ids::{
    IDS_POLICY_AUTONOMY_WORKBENCH_CONTRACT_VERSION, IDS_POLICY_AUTONOMY_WORKBENCH_FEATURE_ID,
};
use bioprism_ids::{
    IDS_PROSPECTIVE_PROVENANCE_CONTRACT_VERSION, IDS_PROSPECTIVE_PROVENANCE_FEATURE_ID,
};
use bioprism_ids::{IDS_PROTOCOL_SIMULATION_CONTRACT_VERSION, IDS_PROTOCOL_SIMULATION_FEATURE_ID};
use bioprism_ids::{IDS_PROVENANCE_SIGNING_CONTRACT_VERSION, IDS_PROVENANCE_SIGNING_FEATURE_ID};
use bioprism_ids::{IDS_PUBLICATION_RELEASE_CONTRACT_VERSION, IDS_PUBLICATION_RELEASE_FEATURE_ID};
use bioprism_ids::{IDS_QUALITY_CONTROL_CONTRACT_VERSION, IDS_QUALITY_CONTROL_FEATURE_ID};
use bioprism_ids::{IDS_RELIABILITY_COPILOT_CONTRACT_VERSION, IDS_RELIABILITY_COPILOT_FEATURE_ID};
use bioprism_ids::{
    IDS_REPLICATION_INTEROPERABILITY_CONTRACT_VERSION, IDS_REPLICATION_INTEROPERABILITY_FEATURE_ID,
};
use bioprism_ids::{IDS_RESEARCH_WORKBENCH_CONTRACT_VERSION, IDS_RESEARCH_WORKBENCH_FEATURE_ID};
use bioprism_ids::{
    IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_CONTRACT_VERSION,
    IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_FEATURE_ID,
};
use bioprism_ids::{IDS_SCALE_FRONTIER_CONTRACT_VERSION, IDS_SCALE_FRONTIER_FEATURE_ID};
use bioprism_ids::{IDS_SEMANTIC_PARITY_CONTRACT_VERSION, IDS_SEMANTIC_PARITY_FEATURE_ID};
use bioprism_ids::{
    IDS_STATISTICAL_CAUSAL_ML_CONTRACT_VERSION, IDS_STATISTICAL_CAUSAL_ML_FEATURE_ID,
};
use bioprism_ids::{
    IDS_TYPED_DETERMINISM_ASSURANCE_CONTRACT_VERSION, IDS_TYPED_DETERMINISM_ASSURANCE_FEATURE_ID,
};
use bioprism_ids::{IDS_TYPED_DETERMINISM_CONTRACT_VERSION, IDS_TYPED_DETERMINISM_FEATURE_ID};
use bioprism_influence::{
    InfluenceAnalyzer, Perturbation, INFLUENCE_LOCAL_EVIDENCE_SURVEILLANCE_FEATURE_ID,
};
use bioprism_infra::{
    AccessRecord, Cache, CodeIdentity, ComputationKey, Dataset as QualityDataset,
    DependencyDeclaration, DependencyGraph, Epoch, Gate as QualityGate, InvalidationPlan,
    KeySchema, Purpose, ReferenceSets, ResourceId, ReuseRule, StorageClass, StorageQuota, Tier,
    TieringPolicy,
};
use bioprism_interweave::interweave_contract_frontier_federated_control_plane::feature_id as INTERWEAVE_FRONTIER_FEATURE_ID;
use bioprism_interweave::workflow::{
    catalogue as interweave_catalogue, outstanding_deliverables, WorkflowId as InterweaveWorkflowId,
};
use bioprism_interweave::workflow_execution::{
    WorkflowExecutionBinding as InterweaveWorkflowExecutionBinding,
    WorkflowExecutionReceipt as InterweaveWorkflowExecutionReceipt,
    WORKFLOW_EXECUTION_SCHEMA as INTERWEAVE_WORKFLOW_EXECUTION_SCHEMA,
};
use bioprism_interweave::{
    FEDERATED_COMMONS_ASSURANCE_CONTRACT_VERSION, FEDERATED_COMMONS_ASSURANCE_FEATURE_ID,
};
use bioprism_lab::{
    evolution::{ChangeProposal, ContaminationRecord, EvolutionCard},
    expand as expand_acquisitions,
    holdout::{Holdout, HoldoutId, HoldoutLedger, Partition},
    pareto::{Direction as LabParetoDirection, ParetoFront, Profile as LabParetoProfile},
    risk::{BranchLedger, BranchOutcome, BranchPolicy, RiskFeatures},
    rollback::{Checkpoint, Deployment},
    separate as separate_hypotheses,
    space::{ArchitectureSpace, CandidateArchitecture, ConfigurationId},
    AcquisitionAction, AcquisitionCost, HypothesisSet as LabHypothesisSet, Observations,
    RETRIEVAL_SYNTHESIS_OPERATIONS_CONTRACT_VERSION, RETRIEVAL_SYNTHESIS_OPERATIONS_FEATURE_ID,
};
use bioprism_ledger::{ClassCounts, Event, EventLedger, SubjectLatest, TemporalCut};
use bioprism_lens::{catalogue as lens_catalogue, run as run_lens, CohortLeakageLens, CohortSplit};
use bioprism_megafactory::{
    place as megafactory_place, DiscrepancyProbe, ExecutionLedger, FenceRegistry, MechanisticModel,
    WorkRequest, WorkerProfile,
};
use bioprism_metrics::{
    analyse_analytics, breakdown as metrics_breakdown, AnalyticsInput, CapabilityGrid,
    CapabilityVector, ComparabilityPolicy as MetricsComparabilityPolicy, DeclaredWeighting,
    PartialRanking, RankInstability, ANALYTICS_SCHEMA_VERSION, METRICS_SCHEMA_VERSION,
};
use bioprism_modalities::{
    analysis_unit as modality_analysis_unit, catalog::all as all_modalities,
    cites as modality_cites, independent_unit as modality_independent_unit,
    report as modality_comparability_report, supported_claims as modality_supported_claims,
    supports_descriptor as modality_supports_descriptor, ClaimKind, EvaluationHorizon,
    EvidenceTier, LiteratureClaim, ModalMeasurement, Modality, ModalityDescriptor,
    ModalityTransport, Resolution, TransportKind,
};
use bioprism_mutation::{
    generate as generate_mutations, measure as measure_diversity, standard_suite,
    MUTATION_PUBLICATION_CONTRACT_VERSION, MUTATION_PUBLICATION_FEATURE_ID,
};
use bioprism_neurosurgery::{
    CaseAssetManifest, CaseAssetManifestQuery, CaseAssetReviewDecision,
    CaseAssetReviewDispositionReport, CaseRequest, DicomCaseImport, DicomEvidenceWorkflowQuery,
    EvidenceAcquisitionQuery, EvidenceAcquisitionSession, EvidenceGraphQuery, EvidenceProgramQuery,
    EvidenceSynthesisQuery, FhirCaseImport, GliomaMolecularMapQuery, LiteratureLinkAuditQuery,
    NeurosurgicalAgent, NeurosurgicalIntakePortfolioQuery, NeurosurgicalIntakeQuery,
    NeurosurgicalMissionResult, NeurosurgicalResearchBriefQuery, NeurosurgicalSession,
    PublicLiteratureBundle, PublicLiteratureDraftAuditRequest, PublicLiteratureEvidencePacketQuery,
    PublicLiteratureIntegrityAuditQuery, PublicLiteratureMatrixQuery,
    PublicLiteraturePortfolioQuery, PublicLiteratureQuery, PublicLiteratureReasoningContextQuery,
    PublicLiteratureRefreshAuditQuery, PublicLiteratureReviewQueueQuery,
    PublicLiteratureWorkbenchQuery, RealDataAutonomousWorkflowQuery, RealDataCohortLandscapeQuery,
    RealDataCoverageQuery, RealDataDiffQuery, RealDataDraftAuditRequest,
    RealDataEvidencePacketQuery, RealDataFreshnessQuery, RealDataMolecularCoverageQuery,
    RealDataQuery, RealDataReasoningContextQuery, RealDataReconciliationQuery,
    RealDataRefreshAuditQuery, RealDataReviewDecision, RealDataReviewDispositionRequest,
    RealDataReviewQueueQuery, RealDataTrialLandscapeQuery, RealGliomaBundle,
    MAX_CASE_ASSET_REVIEW_DISPOSITIONS, MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS,
    MAX_EVIDENCE_ACQUISITION_REFERENCES, MAX_EVIDENCE_ACQUISITION_STEPS, MAX_EVIDENCE_GRAPH_EDGES,
    MAX_EVIDENCE_GRAPH_NODES, MAX_REAL_DATA_DIFF_CHANGES, MAX_REAL_DATA_REVIEW_DISPOSITIONS,
    MAX_REAL_DATA_REVIEW_ITEMS, MAX_RESEARCH_PLAN_REFERENCES, MAX_RESEARCH_PLAN_TASKS,
    MAX_SESSION_STEPS, NEUROSURGERY_SCHEMA_VERSION,
};
use bioprism_obligation::{may_perform, Action as ObligationAction, ObligationGraph};
use bioprism_onco::{
    assess as onco_assess, classify as onco_classify, AcquisitionTime, AvailabilityTime,
    BoundaryRequest, ClinicalObservation, Estimand, FollowUp, Histology, ImagingObservation,
    MarkerPanel, MolecularMarker, ProgressionEvidence, ResearchBoundary, ResponseCriterion,
    ResponseRequest, Timepoint, TreatmentContext, TumourWorldline,
    FEDERATED_PROVENANCE_CONTRACT_VERSION, FEDERATED_PROVENANCE_FEATURE_ID,
    ONCO_INSTRUMENT_CONTRACT_VERSION, ONCO_INSTRUMENT_FEATURE_ID,
};
use bioprism_oncoworlds::entities::{
    declare_cluster as onco_declare_cluster, handle_event as onco_handle_event,
    pool_alterations as onco_pool_alterations, pool_provenance as onco_pool_provenance,
    AlterationMechanism, EventHandling, FollowUpEvent, LesionEndpoint, LesionSet,
    RarePerformanceReport, TissueProvenance,
};
use bioprism_oncoworlds::models::REQUIRED_ASSUMPTIONS as MODEL_REQUIRED_ASSUMPTIONS;
use bioprism_oncoworlds::radiogenomics::{MECHANISM_STRATA, REQUIRED_ASSUMPTIONS};
use bioprism_oncoworlds::{
    as_negative_call as onco_as_negative_call, assert_claim as onco_assert_radiogenomic_claim,
    attribute_to_treatment as onco_attribute_to_treatment, classify as onco_classify_methylation,
    comparable_cohorts as onco_comparable_cohorts,
    compatible_histories as onco_compatible_histories, equity_report as onco_equity_report,
    explain_new_alteration as onco_explain_new_alteration, joinable_with_bridge,
    reconcile_versions as onco_reconcile_methylation,
    transport_to_patients as onco_transport_model, use_descriptor as onco_use_descriptor,
    AnalysisUnit, Artifact, CausalDesign, ClassifierVersion, ClonalHistory,
    Cohort as OncoShiftCohort, DeclaredTransport, DescriptorUse, EntityMapping, EpochBridge,
    EstablishmentCohort, EvaluationDesign, FidelityEvidence, IdentityEvidence, JoinReport,
    JoinVerdict, MethylationClass, ModelResult, PooledScore, PopulationDescriptor,
    RadiogenomicClaim, SampleContext, SiteAssayContext, SpecimenObservation, TumourPopulation,
    VersionedResult,
};
use bioprism_ops::{
    audit_statement as telemetry_audit_statement, CapacityModel, DegradationPlan, Demand,
    DomainEvent, MetricDefinition, Observations as TelemetryObservations, RedactionPolicy, TraceId,
    Workload,
};
use bioprism_oracle::{EvidenceTier as OracleEvidenceTier, Judgement, MeshPolicy, UtcTimestamp};
use bioprism_oraclex::missing::{
    complete_case_admissible, egress as oraclex_egress, informativeness, AbsencePattern, Boundary,
    Field, MissingnessMechanism,
};
use bioprism_oraclex::panel::{ConsensusRule, ReaderPanel};
use bioprism_packs::health::{
    assess as assess_pack_health, HealthPolicy, Observations as PackObservations,
};
use bioprism_packs::ir::PackIr;
use bioprism_packs::portfolio::{
    all as all_packs, duplicate_signatures as duplicate_pack_signatures,
    release_order as pack_release_order, unsequenced as pack_unsequenced,
};
use bioprism_packs::{coverage as pack_coverage, matrix as pack_coverage_matrix};
use bioprism_policy::{PolicyLabel, PolicyLattice, PolicyRule, Request as PolicyRequest};
use bioprism_prism::{minimize, minimize_world, preserves};
use bioprism_project::{
    AssemblyOptions as ProjectAssemblyOptions, Issue as ProjectIssue, ProjectScan, ProjectWorld,
    ScanOptions as ProjectScanOptions,
};
use bioprism_registry::{
    gate_document, BenchmarkPack, Policy as RegistryPolicy, RegistryIndex, TierPolicy, TrustTier,
    REPLICATION_WORKBENCH_CONTRACT_VERSION, REPLICATION_WORKBENCH_FEATURE_ID,
};
use bioprism_repair::{
    plan_for_issue, predicate_from_json, verify as verify_repair,
    verify_successor as verify_repair_successor, AcceptanceReport,
    DeclaredItem as RepairDeclaredItem, PlanOptions as RepairPlanOptions, RepairPlan, Succession,
};
use bioprism_research::{
    adjudicate_glioma_assay_evidence, adjudicate_glioma_evidence_novelty,
    adjudicate_glioma_multimodal_contradictions, admit_glioma_decision_actions,
    admit_glioma_research_workflow, aggregate_glioma_federated_decision_context,
    align_glioma_multi_study_context_artifacts, allocate_glioma_assays, analyze_causal_sensitivity,
    analyze_federated_benchmark, analyze_federated_benchmark_power,
    analyze_federated_continual_knowledge, analyze_federated_evidence_shifts,
    analyze_federated_knowledge, analyze_federated_mechanism_transport,
    analyze_glioma_causal_contrast, analyze_glioma_clonal_evolution,
    analyze_glioma_clone_panel_outcomes, analyze_glioma_combination_synergy,
    analyze_glioma_computation_reproducibility, analyze_glioma_cross_model_claim_envelope,
    analyze_glioma_dose_response,
    analyze_glioma_federated_instrument_consensus, analyze_glioma_instrument_batch_stability,
    analyze_glioma_instrument_multichannel_concordance, analyze_glioma_latent_factors,
    analyze_glioma_lineage_propagation, analyze_glioma_lineage_response_decomposition,
    analyze_glioma_lineage_transport, analyze_glioma_longitudinal_transport,
    analyze_glioma_mechanism_identifiability,
    analyze_glioma_mechanism_intervention_value, analyze_glioma_mechanism_invariance,
    analyze_glioma_mediation, analyze_glioma_multimodal_decision_gate,
    analyze_glioma_multimodal_dropout_stress, analyze_glioma_multimodal_evidence_fusion,
    analyze_glioma_multimodal_graph_fusion, analyze_glioma_multimodal_missingness,
    analyze_glioma_multimodal_sensitivity, analyze_glioma_multistudy_concordance,
    analyze_glioma_outcome_missingness_sensitivity, analyze_glioma_pathway_activity,
    analyze_glioma_prospective_contradiction, analyze_glioma_research_object_dependency_closure,
    analyze_glioma_spatial_communication, analyze_glioma_spatial_niches,
    analyze_glioma_spatial_state_propagation, analyze_glioma_state_transitions,
    analyze_glioma_temporal_multimodal_fusion, analyze_glioma_temporal_spatial_alignment,
    analyze_glioma_trajectories, analyze_glioma_transportability, analyze_instrument_calibration,
    analyze_multimodal_concordance, analyze_multimodal_consensus, analyze_preclinical_outcomes,
    analyze_replication_meta_analysis, analyze_stratified_causal_adjustment,
    assess_glioma_robustness, assess_glioma_validation_batch, assess_replication,
    assimilate_glioma_acquisition_feedback, assimilate_glioma_decision_branch_evidence,
    assimilate_glioma_knowledge_action_outcomes, assimilate_glioma_mechanism_evidence,
    assure_glioma_mechanism_workflow, attribute_glioma_multimodal_quality_root_cause,
    audit_glioma_registered_outcome_reporting, bridge_glioma_evidence_to_knowledge,
    bridge_glioma_knowledge_actions, bridge_glioma_mechanism_fidelity,
    build_glioma_multimodal_ingestion_manifest, build_glioma_registered_outcome_evidence_panel,
    build_glioma_registered_outcome_record, build_research_object_manifest,
    calibrate_glioma_beliefs_prospectively, calibrate_glioma_decision_value,
    calibrate_glioma_evidence, calibrate_glioma_evidence_long_horizon, calibrate_glioma_mechanisms,
    calibrate_glioma_multimodal_quality_transport, calibrate_glioma_multimodal_reliability,
    certify_decision_omissions, close_glioma_claims_to_experiments, cluster_glioma_evidence,
    compile_decision_action_graph, compile_decision_context,
    compile_federated_glioma_execution_handoff, compile_glioma_computation_interpretation_frontier,
    compile_glioma_computation_workflow, compile_glioma_federated_evidence_operating_cycle,
    compile_glioma_federated_outcome_transport, compile_glioma_knowledge_actions,
    compile_glioma_knowledge_closure, compile_glioma_knowledge_consistency,
    compile_glioma_knowledge_gaps, compile_glioma_local_release_review_packet,
    compile_glioma_mechanism_consensus, compile_glioma_mechanism_validation_protocol,
    compile_glioma_mechanism_workflow, compile_glioma_multi_study_mechanism_workflow,
    compile_glioma_multimodal_research_object, compile_glioma_protocol_evidence_surface,
    compile_glioma_release_disclosure_register, compile_glioma_replication_protocol,
    compile_local_research_workflow, compile_mechanism_action_plan, compile_multi_study_knowledge,
    compile_multimodal_knowledge_workflow, compile_typed_knowledge, compose_knowledge_graph,
    control_glioma_mechanism_prospective_batch, design_glioma_contrast_panel,
    design_glioma_robust_experiment, design_preclinical_experiment,
    detect_glioma_evidence_temporal_shifts, detect_glioma_knowledge_drift, discriminate_mechanisms,
    dry_run_adaptive_instrument_executor, dry_run_glioma_adaptive_frontier_executor,
    dry_run_glioma_research, dry_run_instrument_executor_from_request,
    dry_run_robustness_guided_computation_executor, evaluate_glioma_autonomous_research_engine,
    evaluate_glioma_autonomous_research_engine_scenarios,
    evaluate_glioma_autonomous_research_engine_traces, evaluate_glioma_continual_promotion,
    evaluate_glioma_dynamic_policies, evaluate_glioma_release_gate,
    evaluate_glioma_release_trust_policy, execute_federated_benchmark_adaptive_campaign_dry_run,
    execute_federated_benchmark_campaign, execute_federated_benchmark_operating_cycle_dry_run,
    execute_federated_mechanism_transport_campaign_dry_run, execute_glioma_action_portfolio,
    execute_glioma_active_learning_campaign, execute_glioma_adaptive_allocation_campaign,
    execute_glioma_adaptive_clone_campaign_dry_run,
    execute_glioma_adaptive_decision_branch_campaign_dry_run,
    execute_glioma_adaptive_decision_controller, execute_glioma_adaptive_frontier,
    execute_glioma_adaptive_instrument_campaign,
    execute_glioma_adaptive_interpretation_campaign_dry_run,
    execute_glioma_adaptive_mechanism_campaign, execute_glioma_autonomous_campaign,
    execute_glioma_autonomous_gap_cycle, execute_glioma_autonomous_program_cycle,
    execute_glioma_autonomous_protocol, execute_glioma_autonomous_research_engine,
    execute_glioma_autonomous_research_mission,
    execute_glioma_temporal_multimodal_mechanism_fusion,
    execute_glioma_cross_model_replication_mission_dry_run,
    compile_glioma_stage_worker_routes,
    execute_glioma_autonomous_research_engine_with_stage_workers,
    execute_glioma_evidence_gated_stage_engine,
    materialize_glioma_cross_model_replication_actions,
    plan_glioma_cross_model_replication_frontier,
    plan_glioma_cross_model_replication_mission,
    execute_glioma_calibrated_mechanism_campaign_dry_run, execute_glioma_causal_claim_adjudication,
    execute_glioma_computation, execute_glioma_computation_campaign,
    execute_glioma_computation_interpretation_evidence_gate,
    execute_glioma_computation_interpretation_frontier,
    execute_glioma_computation_operating_cycle_dry_run, execute_glioma_computation_portfolio,
    execute_glioma_computation_recovery, execute_glioma_decision_branch_campaign,
    execute_glioma_decision_context_campaign, execute_glioma_decision_mission,
    execute_glioma_decision_operating_cycle, execute_glioma_evidence_acquisition_campaign,
    execute_glioma_evidence_campaign, execute_glioma_evidence_gated_research,
    execute_glioma_evidence_operating_cycle_dry_run, execute_glioma_evidence_refresh_campaign,
    execute_glioma_experiment_frontier_controller, execute_glioma_experiment_operating_cycle,
    execute_glioma_instrument_campaign, execute_glioma_instrument_fleet,
    execute_glioma_instrument_operating_cycle, execute_glioma_instrument_plan,
    execute_glioma_instrument_research_frontier, execute_glioma_instrument_science_loop,
    execute_glioma_intent_mission, execute_glioma_interpretation_operating_cycle,
    execute_glioma_knowledge_action_dispatch, execute_glioma_knowledge_resolution_campaign,
    execute_glioma_knowledge_selection_cycle, execute_glioma_knowledge_synthesis_operating_cycle,
    execute_glioma_local_release_workflow_dry_run, execute_glioma_mechanism_autopilot,
    execute_glioma_mechanism_discovery_engine, execute_glioma_mechanism_discrimination_campaign,
    execute_glioma_mechanism_operating_cycle, execute_glioma_mechanism_validation_protocol,
    execute_glioma_mission_recovery, execute_glioma_multi_fidelity_campaign,
    execute_glioma_multimodal_ingestion_campaign, execute_glioma_multimodal_mechanism_campaign,
    execute_glioma_multimodal_mechanism_campaign_with_executor, execute_glioma_multimodal_mission,
    execute_glioma_multimodal_operating_cycle_dry_run,
    execute_glioma_multimodal_quality_adaptive_campaign,
    execute_glioma_multimodal_quality_schedule, execute_glioma_multimodal_readiness_gate,
    execute_glioma_program_scheduler_dry_run, execute_glioma_protocol,
    execute_glioma_release_batch, execute_glioma_release_operating_cycle_dry_run,
    execute_glioma_replay_campaign, execute_glioma_replication_campaign,
    execute_glioma_replication_closure, execute_glioma_replication_closure_campaign,
    execute_glioma_research_autopilot, execute_glioma_research_director,
    execute_glioma_robust_active_learning_campaign, execute_glioma_robustness_guided_computation,
    execute_glioma_scientific_frontier, execute_glioma_sequential_campaign,
    execute_glioma_validation_campaign, execute_glioma_validation_replication_campaign,
    execute_validation_replication_transport, explore_mechanisms, extract_glioma_instrument_signal,
    filter_glioma_mechanism_states, forecast_glioma_multimodal_quality,
    fuse_glioma_protocol_evidence, gate_glioma_protocol_transport, generate_feature_catalog,
    glioma_program_catalog, govern_glioma_decision_loop, harmonize_glioma_multimodal_batches,
    harmonize_multimodal_inputs, interpret_glioma_federated_closure,
    interpret_glioma_replication_closure, join_glioma_computation_lineage,
    join_glioma_evidence_frontier, materialize_glioma_decision_context_artifact,
    monitor_prospective_knowledge, negotiate_glioma_knowledge_protocol,
    negotiate_glioma_multimodal_knowledge_protocol, optimize_glioma_decision_value,
    optimize_glioma_protocol_branches, plan_adaptive_glioma_dose_surface, plan_decision_actions,
    plan_federated_benchmark_sites, plan_federated_continual_agent,
    plan_federated_glioma_evidence_acquisition, plan_glioma_active_learning,
    plan_glioma_adaptive_information_campaign, plan_glioma_adaptive_mechanism_policy,
    plan_glioma_adaptive_panel, plan_glioma_adaptive_research_frontier,
    plan_glioma_adaptive_workflow, plan_glioma_blocked_randomization,
    plan_glioma_carryover_sequence, plan_glioma_clone_continuation,
    plan_glioma_clone_perturbation_panel, plan_glioma_closed_loop_campaign,
    plan_glioma_computation_portfolio, plan_glioma_decision_branches,
    plan_glioma_evidence_acquisition, plan_glioma_evidence_contradiction_cut,
    plan_glioma_information_design_with_objective, plan_glioma_instrument_recovery,
    plan_glioma_mechanism_closed_loop, plan_glioma_mechanism_multi_fidelity_control,
    plan_glioma_mechanism_validation, plan_glioma_multi_fidelity_optimization,
    plan_glioma_multi_study_workflow, plan_glioma_multimodal_portfolio,
    plan_glioma_multimodal_quality_remediation, plan_glioma_multimodal_quality_schedule,
    plan_glioma_posterior_batch, plan_glioma_power_reestimation, plan_glioma_power_stress_surface,
    plan_glioma_prospective_evidence_triage, plan_glioma_protocol_compensation,
    plan_glioma_replication, plan_glioma_replication_closure_frontier,
    plan_glioma_replication_continuation, plan_glioma_research_object_migration,
    plan_glioma_robust_active_learning, plan_glioma_robust_intervention_portfolio,
    plan_glioma_scientific_frontier, plan_glioma_sequential_design,
    plan_glioma_validation_replication_gate, plan_glioma_workflow, plan_glioma_workflow_recovery,
    preflight_glioma_instrument, prepare_glioma_local_release_signature_payload,
    prepare_glioma_release_trust_policy_payload, prioritize_glioma_evidence,
    prioritize_knowledge_frontier, promote_glioma_closed_loop_frontier,
    promote_glioma_federated_continual_context, propagate_glioma_mechanism_graph, qualify_evidence,
    query_glioma_evidence_workbench, query_glioma_multimodal_researcher_workbench,
    rank_glioma_evidence_novelty, reconcile_glioma_claim_evidence,
    reconcile_glioma_federated_continual_release, reconcile_glioma_multi_study_execution_receipts,
    reconcile_glioma_multisite_outcomes, reconcile_glioma_multistudy_release,
    reconcile_glioma_portfolio_review_workbench, reconcile_glioma_release_batch_review_workbench,
    reconcile_glioma_release_disclosure_batch, reconcile_glioma_release_disclosure_panel,
    reconcile_glioma_replay_history, register_glioma_spatial_samples,
    replan_glioma_mechanism_feedback, replay_glioma_decision_context,
    replay_glioma_multi_study_context_epochs, revise_glioma_beliefs,
    route_glioma_multimodal_evidence_gaps, schedule_glioma_computation_placement,
    schedule_glioma_federated_evidence_batch, schedule_glioma_frontier_campaign,
    audit_glioma_assay_provenance, approve_glioma_instrument_action,
    monitor_glioma_instrument_fleet_health, plan_glioma_acquisition_capacity,
    plan_glioma_acquisition_operations, plan_glioma_instrument_maintenance,
    schedule_glioma_instrument_fleet, select_glioma_actions, simulate_glioma_counterfactual,
    simulate_glioma_counterfactual_ensemble, simulate_glioma_mechanism_dynamics,
    simulate_glioma_protocol, simulate_glioma_protocol_scenario_ensemble,
    smooth_glioma_mechanism_states, snapshot_glioma_evidence_stream,
    stress_glioma_mechanism_robustness, surveil_glioma_evidence, surveil_glioma_multimodal_drift,
    synthesize_glioma_interpretation, triangulate_glioma_evidence,
    update_glioma_mechanism_posterior, validate_feature_catalog, verify_glioma_evidence,
    verify_glioma_local_release_signature, verify_glioma_multimodal_quality_recovery,
    compile_glioma_decision_budget_snapshot, compile_glioma_reproducibility_bundle,
    evaluate_glioma_release_shareability,
    evaluate_glioma_computation_placement_stress,
    govern_glioma_compute_cache, lock_glioma_compute_environment,
    partition_glioma_multistudy_cache, plan_glioma_compute_capacity,
    resolve_glioma_compute_environment, resolve_glioma_registry_artifact,
    score_glioma_reproducibility_completeness, stream_glioma_computation_events,
    submit_glioma_reproducible_task,
    audit_glioma_qualification_preservation, execute_glioma_prospective_replay_fidelity,
    scan_glioma_artifact_integrity,
    attest_glioma_release, evaluate_glioma_research_object_conformance,
    normalize_glioma_release_metadata, preview_glioma_release_audience,
    snapshot_glioma_release_queue, verify_glioma_release_signature,
    audit_glioma_release_dependency_leakage, compile_glioma_continuous_release,
    compose_glioma_multistudy_release, evaluate_glioma_distributed_archive_mirror,
    explore_glioma_comparative_release, migrate_glioma_archive_object,
    plan_glioma_research_object_exchange, plan_glioma_version_retention,
    replay_glioma_release_event_protocol, schedule_glioma_release_queue,
    plan_glioma_heterogeneity_adaptive_benchmark_power,
    plan_glioma_heterogeneity_aware_experiment_portfolio,
    plan_glioma_heterogeneity_portfolio_mission,
    AcquisitionFeedbackRequest, ActionPortfolioExecutionRequest, ActiveLearningCampaignRequest,
    ActiveLearningCandidate, ActiveLearningObservation, ActiveLearningRequest,
    AdaptiveAllocationCampaignRequest, AdaptiveAllocationRequest, AdaptiveArmObservation,
    AdaptiveCloneCampaignRequest, AdaptiveDecisionBranchCampaignRequest,
    AdaptiveDecisionControllerRequest, AdaptiveDoseSurfaceRequest,
    AdaptiveFrontierExecutionRequest, AdaptiveFrontierRequest, AdaptiveInformationCampaignRequest,
    AdaptiveInformationObservation, AdaptiveInstrumentCampaignRequest,
    AdaptiveInterpretationCampaignRequest, AdaptiveMechanismCampaignRequest,
    AdaptiveMechanismPolicyRequest, AdaptivePanelRequest, AnalysisDataset, AnalysisRequest,
    AssayEvidenceObservation, AssayEvidenceRequest, AutonomousGapCycleRequest,
    AutonomousProgramCycleRequest, AutonomousProtocolControllerRequest,
    BayesianMechanismHypothesis, BayesianMechanismUpdateRequest, BeliefConflict,
    BeliefRevisionRequest, BlockedRandomizationRequest, CalibratedMechanismCampaignRequest,
    CalibrationRequest, CalibrationRun, CampaignAction, CampaignMechanism, CampaignObservation,
    CarryoverSequenceRequest, CausalContrastRequest, ClaimEvidenceReconciliationRequest,
    ClaimExperimentClosureRequest, ClonalEvolutionGraph, ClonalEvolutionRequest,
    CloneContinuationCandidate, CloneContinuationRequest, ClonePanelObservation,
    ClonePanelOutcomeAnalysis, ClonePanelOutcomeRequest, ClonePerturbationCandidate,
    ClonePerturbationPanel, ClonePerturbationPanelRequest, CloneProfile, ClosedLoopCampaignRequest,
    ClosedLoopFrontierRequest, ClosureInterpretationRequest, CombinationObservation,
    CombinationSynergyRequest, ComputationCandidate, ComputationExecutionMode,
    ComputationExecutionRequest, ComputationInterpretationEvidenceGateRequest,
    ComputationInterpretationFrontierRequest, ComputationLineageRequest,
    ComputationPlacementRequest, ComputationPlacementStressEvaluationRequest,
    ComputationPortfolioExecutionRequest, ComputationPortfolioRequest,
    ComputationRecoveryRequest, ComputationReproducibilityRequest, ComputationReproducibilityRun,
    ComputationEventStreamRequest, ComputeCacheGovernorRequest, ComputeCapacityRequest,
    ComputeEnvironmentLockRequest, DecisionBudgetRequest, EnvironmentResolutionRequest,
    ArtifactIntegrityRequest, ArtifactRegistryResolutionRequest, MultiStudyCachePartitionRequest,
    QualificationPreservationRequest, ReplayFidelityRequest,
    ReproducibilityBundleRequest, ReproducibilityCompletenessRequest,
    ReproducibleTaskSubmission, ReleaseShareabilityRequest,
    ConformanceRequest, ReleaseMetadataNormalizationRequest, ReleasePreviewRequest,
    ReleaseQueueRequest, ReleaseSignatureVerificationRequest, SignedReleaseAttestationRequest,
    ArchiveMigrationRequest, ComparativeReleaseExplorerRequest, ComparativeReleaseRequest,
    ContinuousReleaseRequest, DistributedMirrorRequest, ReleaseDependencyLeakageRequest,
    ReleaseEventProtocolRequest, ReleaseQueueScheduleRequest, ResearchObjectExchangeRequest,
    RetentionGovernorRequest,
    query_glioma_decision_context, store_glioma_decision_context_snapshots,
    update_glioma_decision_context,
    ConcordanceRequest, ConsensusRequest, ContinualPromotionRequest,
    ContradictionAdjudicationRequest, ContradictionCutRequest, ContradictionEvidence,
    ContrastDesignRequest, CounterfactualEnsembleRequest, CounterfactualIntervention,
    CounterfactualModel, CounterfactualRequest, DecisionActionGraphRequest,
    DecisionActionPlanRequest, DecisionAdmissionRequest, DecisionBranchCampaignRequest,
    DecisionBranchEvidenceRequest, DecisionBranchPlannerRequest, DecisionContext,
    DecisionContextArtifactRequest, DecisionContextCampaignRequest, DecisionContextReplayRequest,
    DecisionContextQueryRequest, DecisionContextRequest, DecisionContextSnapshotStoreRequest,
    DecisionContextUpdateRequest, DecisionLoopGovernorRequest, DecisionMissionBridgeRequest,
    HeterogeneityAdaptivePowerRequest, HeterogeneityAwareExperimentPortfolioRequest,
    HeterogeneityPortfolioMissionRequest,
    DecisionOmissionCertificateRequest, DecisionOperatingCycleRequest,
    DecisionValueCalibrationRequest, DecisionValueRequest, DependencyClosureRequest, DesignAction,
    DesignMechanism, DoseResponseObservation, DoseResponseRequest, DriftSurveillanceRequest,
    DropoutStressRequest, DryRunActiveLearningCampaignExecutor,
    DryRunAdaptiveAllocationCampaignExecutor, DryRunAdaptiveMechanismPolicyExecutor,
    DryRunDecisionContextCampaignExecutor, DryRunEvidenceAcquisitionExecutor,
    DryRunEvidenceRefreshCampaignExecutor, DryRunExperimentOperatingCycleExecutor,
    DryRunFederatedBenchmarkCampaignExecutor, DryRunFederatedMechanismTransportExecutor,
    DryRunGliomaActionExecutor, DryRunGliomaComputationExecutor, DryRunReplayFidelityExecutor,
    DryRunGliomaExperimentFrontierExecutor, DryRunGliomaProtocolExecutor,
    DryRunGliomaReplicationCampaignExecutor, DryRunGliomaStageWorker, DryRunInstrumentExecutor,
    DryRunKnowledgeActionExecutor, DryRunKnowledgeResolutionCampaignExecutor,
    DryRunMechanismDiscriminationCampaignExecutor, DryRunMultiFidelityCampaignExecutor,
    DryRunMultimodalIngestionCampaignExecutor, DryRunQualityScheduleExecutor,
    DryRunReplayCampaignExecutor, DryRunRobustActiveLearningCampaignExecutor,
    DryRunSequentialCampaignExecutor, DynamicPolicyCandidate, DynamicPolicyRequest,
    DynamicPolicyTrajectory, EvidenceAcquisitionCampaignRequest, EvidenceAcquisitionCandidate,
    EvidenceAcquisitionRequest, EvidenceCalibrationObservation, EvidenceCalibrationRequest,
    EvidenceClusterRequest, EvidenceExecutionMode, EvidenceFrontierJoinRequest,
    EvidenceFusionRequest, EvidenceKnowledgeBridgeRequest, EvidenceNoveltyRadarRequest,
    EvidencePriorityRequest, EvidenceProspectiveTriageRequest, EvidenceRecord,
    EvidenceRefreshCampaignRequest, EvidenceRequest, EvidenceStreamRequest,
    EvidenceSurveillanceRequest, EvidenceTemporalShiftRequest, EvidenceTriangulationRequest,
    EvidenceVerificationRequest, EvidenceWorkbenchRequest, ExperimentArm,
    ExperimentOperatingCycleRequest, ExperimentRequest, FederatedAcquisitionPolicyRequest,
    FederatedBatchSchedulerRequest, FederatedBenchmarkAdaptiveCampaignRequest,
    FederatedBenchmarkCampaignRequest, FederatedBenchmarkExecutionMode,
    FederatedBenchmarkOperatingCycleRequest, FederatedBenchmarkPowerRequest,
    FederatedBenchmarkRequest, FederatedBenchmarkSite, FederatedBenchmarkSitePlannerRequest,
    FederatedContinualAgentRequest, FederatedContinualKnowledgeRequest,
    FederatedContinualPromotionRequest, FederatedContinualReleaseRequest,
    FederatedDecisionContextRequest, FederatedEvidenceOperatingCycleRequest,
    FederatedEvidenceShiftRequest, FederatedEvidenceShiftSite, FederatedExecutionHandoffRequest,
    FederatedInstrumentConsensusRequest, FederatedInstrumentSite, FederatedInterpretationRequest,
    FederatedKnowledgeRequest, FederatedKnowledgeSiteClaim, FederatedMechanismSite,
    FederatedMechanismTransportCampaignRequest, FederatedMechanismTransportRequest,
    FederatedOutcomeTransportRequest, FidelityCandidate, FidelityObservation,
    FrontierCampaignRequest, GliomaActionCandidate, GliomaActionExecutor,
    GliomaAdaptiveWorkflowSchedulerRequest, GliomaAutonomousCampaignRequest,
    GliomaAutonomousResearchEngineRequest, GliomaCausalClaimAdjudicationRequest,
    GliomaEngineTraceOutcome,
    GliomaEvidenceGatedStageExecutionRequest, GliomaStageExecutor, GliomaStageWorkerProfile,
    GliomaStageWorkerRouteRequest,
    GliomaComputationCampaignRequest, GliomaComputationOperatingCycleRequest,
    GliomaComputationWorkflowRequest, GliomaEvidenceCampaignRequest,
    GliomaEvidenceGatedResearchRequest, GliomaEvidenceOperatingCycleRequest,
    GliomaExperimentFrontierRequest, GliomaIntentMissionRequest,
    GliomaInterpretationOperatingCycleRequest, GliomaLocalReleaseWorkflowRequest,
    GliomaMechanismAutopilotRequest, GliomaMechanismDiscoveryRequest, GliomaMissionRecoveryRequest,
    GliomaMissionRequest, GliomaMultimodalMissionRequest, GliomaMultimodalOperatingCycleRequest,
    GliomaMultimodalSensitivityRequest, GliomaProgramSchedulerRequest,
    GliomaReleaseOperatingCycleRequest, GliomaReplicationCampaignRequest,
    GliomaResearchAutopilotRequest, GliomaResearchDirectorRequest, GliomaResearchIntent,
    CrossModelClaimEnvelopeRequest, CrossModelReplicationFrontierRequest,
    CrossModelReplicationMissionExecutionRequest,
    CrossModelReplicationMissionRequest, GliomaWorkflowRequest, GraphFusionRequest,
    GraphFusionVector, HarmonizationRequest,
    HarmonizationVector, IdentifiabilityFeature, IdentifiabilityMechanism,
    InformationAcquisitionObjective, InformationDesignRequest,
    InstrumentAssayEvidenceRunObservation, InstrumentCampaignRequest,
    InstrumentExecutionMode, InstrumentExecutionRequest, InstrumentExecutionRun,
    AcquisitionCapacityRequest, AcquisitionOperationsRequest, AssayProvenanceAuditRequest,
    FleetHealthMonitorRequest, InstrumentFleetExecutionRequest, InstrumentFleetScheduleRequest,
    InstrumentInterlockSnapshot,
    InstrumentOperatingCycleRequest, InstrumentPreflightRequest, InstrumentRecoveryRequest,
    InstrumentResearchFrontierRequest, InstrumentScienceLoopRequest, InstrumentSignalPoint,
    InstrumentSignalRun, InterpretationSynthesisRequest, InvarianceMechanism,
    KnowledgeActionBridgeRequest, KnowledgeActionCompilerRequest, KnowledgeActionDispatchRequest,
    KnowledgeActionOutcomeAssimilationRequest, KnowledgeActionPlan, KnowledgeActionSelectionCycle,
    KnowledgeActionSelectionCycleRequest, KnowledgeActionTemplate, KnowledgeClosureRequest,
    KnowledgeCompositionRequest, KnowledgeConsistencyRequest, KnowledgeDriftRequest,
    KnowledgeFrontier, KnowledgeFrontierRequest, KnowledgeGapCompilerRequest,
    KnowledgeProtocolRequest, KnowledgeRelation, KnowledgeRequest,
    KnowledgeResolutionCampaignRequest, KnowledgeSynthesisOperatingCycleRequest,
    LatentFactorRequest, LatentFactorVector, LigandReceptorPair, LocalReleaseReviewPacketRequest,
    LocalReleaseSignatureContextRequest, LocalReleaseSignatureVerificationRequest,
    LocalWorkflowRequest, LongHorizonCalibrationRequest, LongitudinalTransportObservation,
    LongitudinalTransportRequest, MechanismActionPlannerConfig, MechanismCalibration,
    MechanismCalibrationObservation, MechanismCalibrationRequest, MechanismCandidate,
    MechanismClosedLoopRequest, MechanismConsensusRequest, MechanismDiscrimination,
    MechanismDiscriminationCampaignRequest, MechanismDiscriminationRequest,
    MechanismDiscriminatorAction, MechanismDynamicsEdge, MechanismDynamicsIntervention,
    MechanismDynamicsNode, MechanismDynamicsRequest, MechanismEvidenceAssimilationRequest,
    MechanismFeatureObservation, MechanismFeedbackReplanRequest, MechanismFidelityBridgeRequest,
    MechanismGraphEdge, MechanismGraphNode, MechanismGraphRequest, MechanismHypothesis,
    MechanismIdentifiabilityRequest, MechanismInterventionCandidate,
    MechanismInterventionValueRequest, MechanismInvarianceContext, MechanismInvarianceRequest,
    MechanismMultiStudyWorkflowRequest, MechanismOperatingCycleRequest,
    MechanismProspectiveControllerRequest, MechanismRequest, MechanismRobustnessStressRequest,
    MechanismSignature, MechanismStateFilterRequest, MechanismStateSmootherRequest,
    MechanismValidationExecutionRequest, MechanismValidationPlanRequest,
    MechanismValidationProtocolCompileRequest, MechanismWorkflowAssuranceRequest,
    MechanismWorkflowRequest, MediationObservation, MediationRequest, MetaAnalysisRequest,
    MissingnessAuditRequest, ModalityPortfolioRequest, ModalityVector,
    MultiFidelityCampaignRequest, MultiFidelityControlRequest, MultiFidelityOptimizationRequest,
    MultiSiteOutcomeReconciliationRequest, MultiStudyConcordanceRequest,
    MultiStudyContextEpochReplayRequest, MultiStudyContextRequest, MultiStudyEffect,
    MultiStudyExecutionReceiptRequest, MultiStudyKnowledgeRequest, MultiStudyReleaseRequest,
    MultiStudyWorkflowRequest, MultichannelConcordanceRequest, MultichannelInput,
    MultimodalDecisionGateRequest, MultimodalExecutionMode, MultimodalGapRouterRequest,
    MultimodalIngestionCampaignRequest, MultimodalIngestionManifestRequest,
    MultimodalKnowledgeProtocolRequest, MultimodalMechanismCampaignRequest, MultimodalObservation,
    MultimodalReadinessRequest, MultimodalRequest, MultimodalResearchObjectRequest,
    MultimodalWorkbenchRequest, MultimodalWorkflowRequest, NoveltyAdjudicationRequest,
    OutcomeReportingAuditRequest, OutcomeSensitivityRequest, OutcomeSensitivityStudy,
    PathwayActivityDefinition, PathwayActivityObservation, PathwayActivityRequest,
    PortfolioReviewWorkbenchRequest, PosteriorBatchCandidate, PosteriorBatchRequest,
    PowerArmObservation, PowerReestimationRequest, PowerStressSurfaceRequest,
    ProspectiveBeliefCalibrationRequest, ProspectiveContradictionEvidence,
    ProspectiveContradictionRequest, ProspectiveKnowledgeRequest, ProspectiveQualityRequest,
    ProtocolBranchOptimizationRequest, ProtocolCompensationRequest, ProtocolEvidenceFusionRequest,
    ProtocolEvidenceSurfaceRequest, ProtocolExecutionRequest, ProtocolScenarioEnsembleRequest,
    ProtocolSimulationRequest, ProtocolTransportGateRequest, QualityAdaptiveCampaignRequest,
    QualityExecutionMode, QualityExecutionRequest, QualityRecoveryRequest,
    QualityRemediationRequest, QualityRootCauseRequest, QualityScheduleRequest,
    QualityTransportRequest, RegisteredOutcomeEvidencePanelRequest, RegisteredOutcomeRecord,
    RegisteredOutcomeStudy, ReleaseBatchRequest, ReleaseBatchReviewWorkbenchRequest,
    ReleaseDisclosureBatchInput, ReleaseDisclosureBatchRequest, ReleaseDisclosurePanelRequest,
    ReleaseDisclosureStudyInput, ReleaseExecutionMode, ReleaseGateRequest, ReleaseTrustPolicy,
    ReleaseTrustPolicyEvaluationRequest, ReliabilityCalibrationRequest, ReplayCampaign,
    ReplayCampaignRequest, ReplayHistoryRequest, ReplicationClosureCampaignRequest,
    ReplicationClosureExecutionRequest, ReplicationClosureFrontierRequest,
    ReplicationContinuationRequest, ReplicationObservation, ReplicationPlanRequest,
    ReplicationProtocolCompileRequest, ReplicationRequest, ReplicationStudy,
    ResearchObjectManifest, ResearchObjectMigrationRequest, ResearchObjectRequest,
    ResolverCandidate as ProspectiveResolverCandidate, RobustActiveLearningCampaignRequest,
    RobustActiveLearningCandidate, RobustActiveLearningObservation, RobustActiveLearningRequest,
    RobustExperimentDesignRequest, RobustInterventionCandidate, RobustInterventionRequest,
    RobustnessGuidedComputationRequest, RobustnessRequest, ScientificFrontierExecutionRequest,
    ScientificFrontierRequest, SensitivityObservation, SensitivityRequest,
    SequentialArmObservation, SequentialCampaignRequest, SequentialDesignRequest,
    SignalBatchStabilityRequest, SignalExtractionRequest, SpatialCell, SpatialCommunicationCell,
    SpatialCommunicationRequest, SpatialNicheRequest, SpatialPropagationRequest,
    SpatialRegistrationCell, SpatialRegistrationRequest, StateTransitionObservation,
    StateTransitionRequest, StaticGliomaActionPlanner, StaticGliomaComputationPlanner,
    StratifiedCausalRequest, StratifiedObservation, TemporalFusionRequest,
    TemporalMultimodalMechanismFusionRequest, TemporalObservation,
    LineagePropagationAnalysis, LineagePropagationRequest, LineagePropagationSnapshot,
    LineageResponseDecompositionRequest, LineageTransportRequest, LineageTransportStudy,
    MaintenanceWindowRequest, OperatorApprovalRequest, TemporalSpatialAlignmentRequest,
    TrajectoryObservation, TrajectoryRequest, TransportStudy, TransportabilityRequest, TypedKnowledge,
    ValidationBatchAssessmentRequest,
    ValidationCampaignRequest, ValidationReplicationCampaignRequest,
    ValidationReplicationGateRequest, ValidationReplicationTransportRequest,
    WorkflowAdmissionRequest, WorkflowRecoveryRequest,
};
use bioprism_routing::{
    lab::{run as run_routing_lab, LabSettings, Task},
    EvidenceLedger, Fingerprint, RoutingPolicy, FEDERATED_EXECUTION_COPILOT_CONTRACT_VERSION,
    FEDERATED_EXECUTION_COPILOT_FEATURE_ID, LABORATORY_INFERENCE_CONTRACT_VERSION,
    LABORATORY_INFERENCE_FEATURE_ID,
};
use bioprism_runtime::{
    compare_suffixes, observable_state, open_suffix, BudgetController, BudgetPlan, EffectPolicy,
    EffectRequest, Fault, Host, InProcessWorld, RecordingHost, ReplayHost, RuntimeResource,
    WorldTape,
};
use bioprism_safety::attest::{
    Attestation, AttestationClaim, AuditEvent, AuditLog, AuditRecord,
    Observation as SafetyObservation, Statement as SafetyStatement,
};
use bioprism_safety::boundary::{ArtifactKind, BoundaryModel, Channel, MovingArtifact, TrustZone};
use bioprism_safety::disclosure::{
    Advisory, Corpus as DisclosureCorpus, Finding, FindingStatus, ImpactAxes,
    Severity as SafetySeverity, Stage, Transition as DisclosureTransition, Vulnerability,
    VulnerabilityClass,
};
use bioprism_safety::incident::{
    BlastRadius, ContainmentAction, ContainmentRequest, Incident, IncidentClass, ResultDisposition,
};
use bioprism_safety::model::section_13;
use bioprism_safety::release::{MedicalBoundary, ReleaseGate, RequestedOutput, RiskAssessment};
use bioprism_safety::{
    InstrumentActionReceipt7, PROSPECTIVE_LABORATORY_INTEGRATION_CONTRACT_VERSION,
    PROSPECTIVE_LABORATORY_INTEGRATION_FEATURE_ID,
};
use bioprism_scale::corpus::{Corpus, GeneratedItem};
use bioprism_scale::split::{verify_item_assignment, Tier as ScaleTier};
use bioprism_scale::{FEDERATION_TRUST_CONTRACT_VERSION, FEDERATION_TRUST_FEATURE_ID};
use bioprism_scope::{DimensionRegistry, ScopeKey, Timestamp as FactoryTimestamp};
use bioprism_sdk::{
    conformance_note, PluginManifest, PluginRegistry, RegistryPolicy as SdkRegistryPolicy,
};
use bioprism_section::{CertificateProfile, ContextCertificate, Layer, RenderContext};
use bioprism_services::{
    audit as service_audit, AuditSummary, MULTIMODAL_INTERPRETATION_CONTRACT_VERSION,
    MULTIMODAL_INTERPRETATION_FEATURE_ID,
};
use bioprism_services::{
    CONTEXT_COMPILATION_COPILOT_CONTRACT_VERSION, CONTEXT_COMPILATION_COPILOT_FEATURE_ID,
};
use bioprism_standards::{
    report as comparability_report, ComparabilityPolicy as StandardsComparabilityPolicy,
    Measurement,
};
use bioprism_stewardship::review::ReviewRecord;
use bioprism_store::LazyWorld;
use bioprism_stress::{
    profile as stress_profile_run, standard_panel, Cohort, Procedure, Stress, StressReport,
};
use bioprism_sweep::conform::{
    differential as sweep_differential, gate as sweep_gate, CapabilityCard as SweepCapabilityCard,
    Check as SweepCheck,
};
use bioprism_tokens::{
    compare as compare_token_plans, plan as plan_token_context, ContextRequest, PlanCandidate,
};
use bioprism_trace::{
    excluded as excluded_trace, first_divergence, from_jsonl as trace_from_jsonl, from_otlp_json,
    is_actionable as divergence_is_actionable, review_reduction, segment as segment_trace,
    validate as validate_trace, CellProposal, Trace as TraceIr, MAX_SPANS as MAX_OTEL_SPANS,
};
use bioprism_weave::ActKind;
use bioprism_weavelang::{compile as compile_weave, ExecutionMode, Machine};
use bioprism_world::{validate, Severity, World, WorldSource};
use bioprism_worldfactory::contradiction::{
    check_intent, cue_scan, expectedness as contradiction_expectedness,
    next_actions as contradiction_next_actions, pose as pose_contradiction,
    validate as validate_contradiction, ContradictionProgram, DiscordanceClass,
    DiscriminatingAction, EvidenceId, Hypothesis, HypothesisSet, MissingEvidence, Reading,
    ReferenceDiscordance,
};
use bioprism_worldfactory::lineage::{audit as audit_lineage, SpecimenRegistry};
use bioprism_worldfactory::observed::{
    declare as declare_observed_world, SourceRef as ObservedSourceRef, StudyDesign,
};
use bioprism_worldfactory::preanalytic::{
    apply as apply_preanalytic, check_response as check_preanalytic_response, detectability_floor,
    validate_family, PreanalyticMutation, Specimen,
};
use bioprism_worldfactory::provenance::{
    support as support_world_claim, Claim, Provenance as WorldProvenance,
};
use bioprism_worldgen::{
    generate as generate_world, WorldSpec, WORLDGEN_MULTIMODAL_EXECUTION_CONTRACT_VERSION,
    WORLDGEN_MULTIMODAL_EXECUTION_FEATURE_ID, WORLDGEN_MULTIMODAL_INGESTION_CONTRACT_VERSION,
    WORLDGEN_MULTIMODAL_INGESTION_FEATURE_ID,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub use bioprism_devplat::MISSION_TRACE_SCHEMA_VERSION;

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "bioprism";
pub const QUERY_SCHEMA_URI: &str = "bioprism://schema/fiber-query/0.2";
pub const ADAPTIVE_QUERY_SCHEMA_URI: &str = "bioprism://schema/fiber-query/0.5";
pub const CERTIFICATE_SCHEMA_URI: &str = "bioprism://schema/fiber-context-certificate/0.1";
pub const WORLD_SCHEMA_URI: &str = "bioprism://schema/fiber-world/0.1";
pub const CAPABILITIES_URI: &str = "bioprism://capabilities/0.1";

/// The schema of the document `repair_plan`'s `criteria` parameter names.
///
/// Its own version rather than the plan's: it is an *authoring* document, carrying the items a
/// caller declares before a plan exists, and versioning it with `bioprism-repair-plan/0.1` would
/// tie an input format to an output format that has no reason to move with it. The same document
/// is read by `bioprism project plan --criteria`.
pub const REPAIR_DECLARATIONS_SCHEMA_VERSION: &str = "bioprism-repair-declarations/0.1";

const QUERY_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fiber-v0.1/query.schema.json"
));
const ADAPTIVE_QUERY_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fiber-v0.5/query.schema.json"
));
const CERTIFICATE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fiber-v0.1/context_certificate.schema.json"
));
const WORLD_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fiber-v0.1/world.schema.json"
));

/// Convert the epistemic kernel's Rust-tagged policy tree into the stable named projection used
/// by both the standalone adaptive route and the versioned FIBER compiler contract.
fn project_adaptive_node(
    node: &EpistemicAdaptiveNode,
    problem: &EpistemicDecisionProblem,
) -> Value {
    match node {
        EpistemicAdaptiveNode::Stop { action, risk } => json!({
            "kind": "stop",
            "action_index": action,
            "action": problem.actions().get(*action),
            "risk": risk,
        }),
        EpistemicAdaptiveNode::Acquire {
            acquisition,
            id,
            cost,
            expected_total,
            expected_terminal_risk,
            expected_acquisition_cost,
            outcomes,
        } => json!({
            "kind": "acquire",
            "acquisition_index": acquisition,
            "id": id,
            "cost": cost,
            "expected_total": expected_total,
            "expected_terminal_risk": expected_terminal_risk,
            "expected_acquisition_cost": expected_acquisition_cost,
            "outcomes": outcomes.iter().map(|outcome| json!({
                "label": outcome.label,
                "probability": outcome.probability,
                "posterior": outcome.posterior,
                "next": project_adaptive_node(&outcome.next, problem),
            })).collect::<Vec<_>>(),
        }),
    }
}

/// The domain object a pack-judged compile carries: who judged, what it protects, and what it
/// could only advise on.
///
/// Advisories exist because a pack cannot rewrite the query it judges — the certificate binds
/// the query's bytes by hash, so a missing protected tag or goal is reported to the caller
/// instead of being silently injected.
fn domain_surface(pack: &DomainPack, query: &Query) -> Value {
    let mut advisories = Vec::new();
    let missing: Vec<&str> = pack
        .protected_tags()
        .iter()
        .filter(|tag| !query.protected_tags.contains(tag.as_str()))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        advisories.push(format!(
            "the query's protected_tags do not include the pack's protected tag(s) {missing:?}; \
             the pack cannot inject them, so the protected closure may be narrower than the \
             domain expects"
        ));
    }
    if query.goal.is_none() {
        advisories.push(match pack.goal() {
            Some(goal) => format!(
                "the query declares no goal; the pack declares {goal:?}, which the caller must \
                 copy into the query because a pack cannot change the bytes the certificate binds"
            ),
            None => "the query declares no goal, and the pack declares none either".to_string(),
        });
    }
    json!({
        "name": pack.name(),
        "oracle_kind": pack.oracle().kind(),
        "protected_tags": pack.protected_tags(),
        "advisories": advisories,
    })
}

fn project_fiber_adaptive_trace(
    trace: &AdaptiveAcquisitionTrace,
    query_sha256: &str,
    certificate_sha256: &str,
) -> Value {
    json!({
        "schema": "bioprism-mcp/fiber-adaptive-acquisition/0.1",
        "budget": trace.budget,
        "max_steps": trace.max_steps,
        "prior": trace.prior,
        "problem": {
            "actions": trace.problem.actions(),
            "models": trace.problem.models(),
            "action_count": trace.problem.action_count(),
            "model_count": trace.problem.model_count(),
        },
        "acquisitions": trace.acquisitions.iter().map(|acquisition| json!({
            "id": acquisition.id,
            "cost": acquisition.cost,
            "outcomes": acquisition.outcomes().iter().map(|outcome| json!({
                "label": outcome.label,
                "likelihood": outcome.likelihoods(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "policy": {
            "expected_total": trace.policy.expected_total,
            "expected_terminal_risk": trace.policy.expected_terminal_risk,
            "expected_acquisition_cost": trace.policy.expected_acquisition_cost,
            "nodes_evaluated": trace.policy.nodes_evaluated,
            "selected_depth": trace.policy.selected_depth,
            "root": project_adaptive_node(&trace.policy.root, &trace.problem),
        },
        "certificate_binding": {
            "query_sha256": query_sha256,
            "certificate_sha256": certificate_sha256,
        },
        "execution": "not_started",
        "authorization": "not_granted",
        "provenance": {
            "planner": "bioprism-epistemic::adaptive_policy",
            "input_posture": "caller_declared_unperformed_acquisitions",
            "independence_assumption": "conditionally_independent_given_supplied_models",
        },
        "guarantees": [
            "the selected policy is exact under the 16-acquisition, 16-step, and 65536-state caps",
            "each acquisition is used at most once and branch-dependent next choices are explicit",
            "expected terminal risk and expected declared cost remain separate",
            "certificate binding makes the policy input and result replayable"
        ],
        "limitations": [
            "the planner assumes conditional independence given the caller-supplied models",
            "the compiler plans but does not schedule, authorize, authenticate, execute, or observe an acquisition",
            "costs are caller-supplied scalarizations rather than inferred resource vectors",
            "the decision-relative policy is not causal, clinical, biological, or predictive truth"
        ]
    })
}

mod adapter;
mod artifact_registry;
mod autopilot;
mod benchmark;
mod bioethics;
mod bioeval;
mod bioeval_coverage;
mod brain;
mod capability;
mod capability_catalog;
mod conformance;
mod control_plane;
mod delivery;
mod developer;
mod dispatch;
mod mission;
mod mission_evidence;
use mission::{on_dispatch_stack, schema_failure_detail, validate_mission_tool_arguments};
mod domain;
mod domain_evidence;
mod domain_evidence_providers;
mod domain_reports;
mod domain_workflows;
mod epistemic;
mod evaluation;
mod evidence;
mod factory;
mod glioma;
mod glioma_analysis;
mod glioma_computation;
mod glioma_decision;
mod glioma_evidence;
mod glioma_experiments;
mod glioma_instrument_ops;
mod glioma_knowledge;
mod glioma_mechanism;
mod glioma_missions;
mod glioma_multimodal;
mod glioma_orchestration;
mod glioma_release;
mod hub;
mod ids;
mod lab;
mod lifecycle;
mod modality;
mod neurosurgery;
mod neurosurgery_literature;
mod neurosurgery_real_data;
mod neurosurgery_sessions;
mod onco;
mod oncoworlds;
mod operations;
mod pack_assurance;
mod project;
mod quality_assurance;
mod registry;
mod repository;
mod research_federation;
mod research_surfaces;
mod routing;
mod runtime;
mod security;
mod workflow;
mod world;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    New,
    Initialized,
    Ready,
}

#[derive(Clone)]
pub struct Server {
    root: PathBuf,
    lifecycle: Lifecycle,
    mission_trace_observer: Option<MissionTraceObserver>,
    evidence_registry: Arc<Mutex<EvidenceBundleRegistry>>,
    workflow_reconciliation_registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
    workflow_execution_evidence_registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
    workbench_registry: Arc<Mutex<WorkbenchReportRegistry>>,
    ci_provider_evidence_registry: Arc<Mutex<CiProviderEvidenceRegistry>>,
    artifact_registry: Arc<Mutex<ArtifactRegistry>>,
    domain_evidence_source_http_policy: DomainEvidenceSourceHttpPolicy,
    brain_control_state: Arc<Mutex<BrainControlState>>,
    glioma_action_executor: Option<Arc<Mutex<Box<dyn GliomaActionExecutor + Send>>>>,
}

impl Server {
    pub fn new(root: PathBuf) -> Self {
        Self::with_registries_and_artifacts(
            root,
            Arc::new(Mutex::new(EvidenceBundleRegistry::new())),
            Arc::new(Mutex::new(DomainWorkflowReconciliationRegistry::new())),
            Arc::new(Mutex::new(ArtifactRegistry::new())),
        )
    }

    /// Construct a server over all caller-owned cross-domain registries.
    ///
    /// The artifact index is separate from evidence and reconciliation storage because it joins
    /// their content identities without owning their semantic schemas. Sharing it here keeps MCP
    /// and HTTP calls in one process on the same index while preserving each domain registry's
    /// independent verification rules.
    pub fn with_registries_and_artifacts(
        root: PathBuf,
        evidence_registry: Arc<Mutex<EvidenceBundleRegistry>>,
        workflow_reconciliation_registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
        artifact_registry: Arc<Mutex<ArtifactRegistry>>,
    ) -> Self {
        Self::with_registries_and_artifacts_and_workflow_execution_evidence(
            root,
            evidence_registry,
            workflow_reconciliation_registry,
            Arc::new(Mutex::new(WorkflowExecutionEvidenceRegistry::new())),
            artifact_registry,
        )
    }

    /// Construct a server over every caller-owned registry, including portable workflow
    /// execution evidence. The API gateway uses this seam so MCP and HTTP requests share both
    /// the live digest indexes and their restart-aware persistence boundary.
    pub fn with_registries_and_artifacts_and_workflow_execution_evidence(
        root: PathBuf,
        evidence_registry: Arc<Mutex<EvidenceBundleRegistry>>,
        workflow_reconciliation_registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
        workflow_execution_evidence_registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
        artifact_registry: Arc<Mutex<ArtifactRegistry>>,
    ) -> Self {
        Self::with_all_registries(
            root,
            evidence_registry,
            workflow_reconciliation_registry,
            workflow_execution_evidence_registry,
            Arc::new(Mutex::new(WorkbenchReportRegistry::new())),
            artifact_registry,
        )
    }

    /// Construct a server over every caller-owned registry, including retained workbench reports.
    ///
    /// The API gateway uses this seam to share its restart-restored workbench index with both
    /// direct HTTP handlers and the MCP dispatcher in the same process.
    pub fn with_all_registries(
        root: PathBuf,
        evidence_registry: Arc<Mutex<EvidenceBundleRegistry>>,
        workflow_reconciliation_registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
        workflow_execution_evidence_registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
        workbench_registry: Arc<Mutex<WorkbenchReportRegistry>>,
        artifact_registry: Arc<Mutex<ArtifactRegistry>>,
    ) -> Self {
        Self::with_all_registries_and_ci_provider_evidence(
            root,
            evidence_registry,
            workflow_reconciliation_registry,
            workflow_execution_evidence_registry,
            workbench_registry,
            Arc::new(Mutex::new(CiProviderEvidenceRegistry::new())),
            artifact_registry,
        )
    }

    /// Construct a server over every caller-owned registry, including retained provider evidence.
    ///
    /// The API gateway uses this seam so provider imports and operator queries share the same
    /// restart-restored index with MCP, while artifact storage remains an independent join.
    pub fn with_all_registries_and_ci_provider_evidence(
        root: PathBuf,
        evidence_registry: Arc<Mutex<EvidenceBundleRegistry>>,
        workflow_reconciliation_registry: Arc<Mutex<DomainWorkflowReconciliationRegistry>>,
        workflow_execution_evidence_registry: Arc<Mutex<WorkflowExecutionEvidenceRegistry>>,
        workbench_registry: Arc<Mutex<WorkbenchReportRegistry>>,
        ci_provider_evidence_registry: Arc<Mutex<CiProviderEvidenceRegistry>>,
        artifact_registry: Arc<Mutex<ArtifactRegistry>>,
    ) -> Self {
        Server {
            root: std::fs::canonicalize(&root).unwrap_or(root),
            lifecycle: Lifecycle::New,
            mission_trace_observer: None,
            evidence_registry,
            workflow_reconciliation_registry,
            workflow_execution_evidence_registry,
            workbench_registry,
            ci_provider_evidence_registry,
            artifact_registry,
            domain_evidence_source_http_policy: DomainEvidenceSourceHttpPolicy::default(),
            brain_control_state: Arc::new(Mutex::new(BrainControlState::default())),
            glioma_action_executor: None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Allow explicit source retrieval from operator-approved plain HTTP origins.
    ///
    /// Plans remain caller-controlled and cannot grant themselves network access. Each request
    /// must also opt into networking and name the requested host in its own plan.
    pub fn with_domain_evidence_source_http_origins<I, S>(
        mut self,
        origins: I,
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.domain_evidence_source_http_policy = DomainEvidenceSourceHttpPolicy::new(origins)?;
        Ok(self)
    }

    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    /// Attach a caller-owned institution-local worker for the autonomous glioma engine.
    ///
    /// The default server remains synthetic-only. This opt-in seam is shared across cloned
    /// servers and is invoked only after the research engine has admitted typed actions under its
    /// request policy and budget.
    pub fn with_glioma_action_executor<E>(mut self, executor: E) -> Self
    where
        E: GliomaActionExecutor + Send + 'static,
    {
        self.glioma_action_executor = Some(Arc::new(Mutex::new(Box::new(executor))));
        self
    }

    /// Resolves a client-supplied path inside the root.
    ///
    /// Rejects absolute paths, traversal, and symlink escapes from the root.
    ///
    /// The lexical check runs before touching the filesystem. For an existing path, the resolved
    /// target is then canonicalised; for a new write, the nearest existing ancestor is checked.
    /// This closes the common `root/link/outside` escape without pretending that a later filesystem
    /// rename can be made race-free by a pure path helper.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, String> {
        let candidate = Path::new(relative);
        // `Path` follows the host platform. The MCP boundary is cross-platform,
        // though, so a Windows drive path must not become an ordinary filename
        // when the server happens to run on Unix (and vice versa).
        let bytes = relative.as_bytes();
        let has_drive_prefix =
            bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if candidate.is_absolute()
            || relative.starts_with('/')
            || relative.starts_with('\\')
            || has_drive_prefix
        {
            return Err(format!("absolute paths are refused: {relative:?}"));
        }
        // Check both separators lexically. On Unix, `..\\outside` would
        // otherwise be treated as a literal filename instead of traversal.
        for component in relative.split(['/', '\\']) {
            if component == ".." {
                return Err(format!(
                    "path escapes the server root and is refused: {relative:?}"
                ));
            }
        }
        for component in candidate.components() {
            match component {
                Component::Normal(_) | Component::CurDir => {}
                _ => {
                    return Err(format!(
                        "path escapes the server root and is refused: {relative:?}"
                    ));
                }
            }
        }

        let resolved = self.root.join(candidate);
        self.ensure_inside_root(&resolved)?;
        Ok(resolved)
    }

    fn ensure_inside_root(&self, path: &Path) -> Result<(), String> {
        let mut existing = path;
        while !existing.exists() {
            existing = existing.parent().ok_or_else(|| {
                format!(
                    "path cannot be checked against the server root: {}",
                    path.display()
                )
            })?;
        }

        let canonical = existing.canonicalize().map_err(|error| {
            format!(
                "cannot resolve path against the server root: {} ({error})",
                path.display()
            )
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(format!(
                "path escapes the server root and is refused: {}",
                path.display()
            ));
        }
        Ok(())
    }

    fn load_source(&self, relative: &str) -> Result<Box<dyn WorldSource>, String> {
        let path = self.resolve(relative)?;
        if path.is_dir() && path.join("manifest.json").exists() {
            LazyWorld::open(&path)
                .map(|lazy| Box::new(lazy) as Box<dyn WorldSource>)
                .map_err(|e| e.to_string())
        } else {
            let raw = self.read_json(&path)?;
            World::from_json(raw)
                .map(|world| Box::new(world) as Box<dyn WorldSource>)
                .map_err(|e| e.to_string())
        }
    }

    fn read_json(&self, path: &Path) -> Result<Value, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
    }

    fn load_query(&self, relative: &str) -> Result<Query, String> {
        let path = self.resolve(relative)?;
        let raw = self.read_json(&path)?;
        Query::from_json(raw).map_err(|e| e.to_string())
    }

    pub fn handle(&mut self, request: &Request) -> Option<Response> {
        if request.is_notification() {
            if request.method == "notifications/initialized"
                && self.lifecycle == Lifecycle::Initialized
            {
                self.lifecycle = Lifecycle::Ready;
            }
            return None;
        }

        let id = request.id.clone();
        let response = match request.method.as_str() {
            "initialize" => {
                if self.lifecycle != Lifecycle::New {
                    Response::error(
                        id,
                        code::INVALID_REQUEST,
                        "initialize may only be called once per server session".into(),
                        None,
                    )
                } else {
                    let result = self.initialize();
                    self.lifecycle = Lifecycle::Initialized;
                    Response::result(id, result)
                }
            }
            "ping" => Response::result(id, json!({})),
            "tools/list" if self.ready() => {
                Response::result(id, json!({ "tools": tool_definitions() }))
            }
            "tools/call" if self.ready() => self.call_tool(request),
            "resources/list" if self.ready() => {
                Response::result(id, json!({ "resources": resource_definitions() }))
            }
            "resources/read" if self.ready() => self.read_resource(request),
            "tools/list" | "tools/call" | "resources/list" | "resources/read" => Response::error(
                id,
                code::INVALID_REQUEST,
                "server is not ready; call initialize and then send notifications/initialized"
                    .into(),
                None,
            ),
            other => Response::error(
                id,
                code::METHOD_NOT_FOUND,
                format!("unknown method {other:?}"),
                None,
            ),
        };
        Some(response)
    }

    fn ready(&self) -> bool {
        self.lifecycle == Lifecycle::Ready
    }

    fn initialize(&self) -> Value {
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false }
            },
            "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Compiles a typed decision query against a FIBER world into the \
                smallest decision-sufficient context, with a machine-verifiable certificate of \
                what was omitted. Call fiber_compile first: it returns the decision contract and \
                a content-addressed refinement handle, not the evidence. Call fiber_refine with that \
                handle to descend layers only \
                when the contract is insufficient to act. Every response reports what was omitted \
                and whether the sufficiency claim holds. Research infrastructure, not a medical \
                device."
        })
    }

    fn read_resource(&self, request: &Request) -> Response {
        let id = request.id.clone();
        let Some(uri) = request.params.get("uri").and_then(Value::as_str) else {
            return Response::error(
                id,
                code::INVALID_PARAMS,
                "resources/read requires a uri".into(),
                None,
            );
        };

        let (mime_type, text) = match uri {
            QUERY_SCHEMA_URI => ("application/schema+json", QUERY_SCHEMA.to_string()),
            ADAPTIVE_QUERY_SCHEMA_URI => {
                ("application/schema+json", ADAPTIVE_QUERY_SCHEMA.to_string())
            }
            CERTIFICATE_SCHEMA_URI => ("application/schema+json", CERTIFICATE_SCHEMA.to_string()),
            WORLD_SCHEMA_URI => ("application/schema+json", WORLD_SCHEMA.to_string()),
            CAPABILITIES_URI => ("application/json", workspace_capabilities().to_string()),
            other => {
                return Response::error(
                    id,
                    code::INVALID_PARAMS,
                    format!("unknown resource uri {other:?}"),
                    None,
                );
            }
        };

        Response::result(
            id,
            json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": mime_type,
                    "text": text,
                }]
            }),
        )
    }

    /// Dispatch a tool call on a dedicated thread with an explicit stack reservation.
    ///
    /// Unoptimised builds give the widest tool frames multi-megabyte activation records, and a
    /// mission re-enters this dispatcher from inside its own execution frame; on the default
    /// thread stack that aborted with `STATUS_STACK_OVERFLOW` rather than refusing. Reserving
    /// the stack here makes every dispatch level independent of the caller's remaining stack —
    /// the same guard the CLI applies at its entry point. A panic inside the tool is resumed on
    /// the caller so panic behaviour is unchanged; only a failure to start the thread becomes a
    /// new, explicit error response.
    fn call_tool(&self, request: &Request) -> Response {
        match on_dispatch_stack("bioprism-mcp-dispatch", || self.call_tool_inner(request)) {
            Ok(response) => response,
            Err(error) => Response::error(
                request.id.clone(),
                code::INTERNAL_ERROR,
                format!("cannot start the tool dispatch thread: {error}"),
                None,
            ),
        }
    }

    fn compiled(
        &self,
        arguments: &Value,
    ) -> Result<(bioprism_fiber::CompileOutput, RenderContext), String> {
        let (out, context, _) = self.compiled_with_domain(arguments)?;
        Ok((out, context))
    }

    /// The compile pipeline with an optional caller-chosen domain pack.
    ///
    /// Without a `domain` argument this is [`Server::compiled`] exactly: the reference
    /// split-integrity oracle judges the world and the response bytes are unchanged. With one,
    /// the pack's declared rule oracle judges it instead, and the returned surface carries what
    /// the pack could only advise on — a pack cannot rewrite the query it judges, because that
    /// would change the bytes the certificate binds.
    fn compiled_with_domain(
        &self,
        arguments: &Value,
    ) -> Result<(bioprism_fiber::CompileOutput, RenderContext, Option<Value>), String> {
        let (world, query, domain, expected_digest) = self.compile_paths(arguments)?;

        let source = self.load_source(&world)?;
        let query_document = self.load_query(&query)?;
        let pack = domain
            .as_deref()
            .map(|relative| self.load_domain_pack(relative))
            .transpose()?;
        let out = match &pack {
            Some(pack) => compile_with_oracle(source.as_ref(), &query_document, pack.oracle()),
            None => compile(source.as_ref(), &query_document),
        }
        .map_err(|e| e.to_string())?;
        let surface = pack
            .as_ref()
            .map(|pack| domain_surface(pack, &query_document));

        let context = RenderContext {
            omitted_facts: out.certificate.omissions.total_facts,
            total_facts: out.certificate.plan.total_fact_count,
            supports_sufficiency_claim: out.certificate.manifest.supports_sufficiency_claim(),
            protected_closure_satisfied: out.protected_closure_satisfied(),
            certificate_sha256: out
                .certificate
                .digest(CertificateProfile::Reference)
                .ok()
                .map(|digest| digest.as_str().to_string()),
        };

        if let Some(expected) = expected_digest {
            let actual = context
                .certificate_sha256
                .as_deref()
                .ok_or("compiled certificate has no reference digest")?;
            if actual != expected {
                return Err(format!(
                    "refinement handle is stale: it names certificate {expected}, but the current inputs compile to {actual}"
                ));
            }
        }

        Ok((out, context, surface))
    }

    /// Loads and validates a domain pack from a root-confined path.
    fn load_domain_pack(&self, relative: &str) -> Result<DomainPack, String> {
        let path = self.resolve(relative)?;
        let raw = self.read_json(&path)?;
        DomainPack::from_json(&raw).map_err(|error| error.to_string())
    }

    fn compile_paths(
        &self,
        arguments: &Value,
    ) -> Result<(String, String, Option<String>, Option<String>), String> {
        if let Some(handle) = arguments.get("handle") {
            let handle = handle
                .as_object()
                .ok_or("handle must be an object returned by fiber_compile")?;
            let version = handle
                .get("version")
                .and_then(Value::as_u64)
                .ok_or("refinement handle is missing its version")?;
            if version != 1 {
                return Err(format!("unsupported refinement handle version {version}"));
            }
            let world = handle
                .get("world")
                .and_then(Value::as_str)
                .ok_or("refinement handle is missing world")?;
            let query = handle
                .get("query")
                .and_then(Value::as_str)
                .ok_or("refinement handle is missing query")?;
            let digest = handle
                .get("certificate_sha256")
                .and_then(Value::as_str)
                .ok_or("refinement handle is missing certificate_sha256")?;
            let domain = handle
                .get("domain")
                .and_then(Value::as_str)
                .map(str::to_string);
            return Ok((world.into(), query.into(), domain, Some(digest.into())));
        }

        let world = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a path relative to the server root)")?;
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or("query is required (a path relative to the server root)")?;
        let domain = match arguments.get("domain") {
            None => None,
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or("domain must be a string path relative to the server root")?
                    .to_string(),
            ),
        };
        Ok((world.into(), query.into(), domain, None))
    }
}

fn parse_mechanistic_model(raw: &Value, label: &str) -> Result<MechanisticModel, String> {
    let parsed: MechanisticModel = serde_json::from_value(raw.clone())
        .map_err(|error| format!("invalid {label} mechanistic model: {error}"))?;
    MechanisticModel::new(
        parsed.id,
        parsed.compartments,
        parsed.rates,
        parsed.known_misspecification,
    )
    .map_err(|error| format!("invalid {label} mechanistic model: {error}"))
}

fn runtime_world_from_config(config: &Value, label: &str) -> Result<InProcessWorld, String> {
    let object = match config {
        Value::Null => None,
        Value::Object(object) => Some(object),
        _ => return Err(format!("{label} must be an object when supplied")),
    };
    let Some(object) = object else {
        return Ok(InProcessWorld::new());
    };
    let encoded = serde_json::to_vec(config)
        .map_err(|error| format!("cannot measure {label} configuration: {error}"))?;
    if encoded.len() > 2_000_000 {
        return Err(format!("{label} exceeds the 2000000-byte safety bound"));
    }
    let seed = object.get("seed").and_then(Value::as_u64).unwrap_or(0);
    let clock_start = object
        .get("clock_start")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let clock_tick = object
        .get("clock_tick")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut world = InProcessWorld::new()
        .with_seed(seed)
        .with_clock_start(clock_start)
        .with_clock_tick(clock_tick);

    if let Some(raw_files) = object.get("base_files") {
        let files = raw_files
            .as_object()
            .ok_or_else(|| format!("{label}.base_files must be an object of path to string"))?;
        if files.len() > 1_000 {
            return Err(format!(
                "{label}.base_files exceeds the 1000-file safety bound"
            ));
        }
        for (path, content) in files {
            let content = content
                .as_str()
                .ok_or_else(|| format!("{label}.base_files values must be strings"))?;
            world = world.with_base_file(path, content);
        }
    }
    if let Some(raw_fixtures) = object.get("fixtures") {
        let fixtures = raw_fixtures
            .as_object()
            .ok_or_else(|| format!("{label}.fixtures must map `METHOD URL` to string body"))?;
        if fixtures.len() > 1_000 {
            return Err(format!(
                "{label}.fixtures exceeds the 1000-fixture safety bound"
            ));
        }
        for (key, body) in fixtures {
            let (method, url) = key
                .split_once(' ')
                .ok_or_else(|| format!("{label}.fixtures keys must be `METHOD URL`"))?;
            let body = body
                .as_str()
                .ok_or_else(|| format!("{label}.fixtures values must be strings"))?;
            world = world.with_fixture(method, url, body);
        }
    }
    if let Some(raw_services) = object.get("services") {
        let services = raw_services
            .as_object()
            .ok_or_else(|| format!("{label}.services must map `service.operation` to JSON"))?;
        if services.len() > 1_000 {
            return Err(format!(
                "{label}.services exceeds the 1000-service safety bound"
            ));
        }
        for (key, response) in services {
            let (service, operation) = key
                .split_once('.')
                .ok_or_else(|| format!("{label}.services keys must be `service.operation`"))?;
            world = world.with_service(service, operation, response.clone());
        }
    }
    if let Some(raw_faults) = object.get("faults") {
        let faults = raw_faults
            .as_array()
            .ok_or_else(|| format!("{label}.faults must be an array"))?;
        if faults.len() > 1_000 {
            return Err(format!(
                "{label}.faults exceeds the 1000-fault safety bound"
            ));
        }
        for raw_fault in faults {
            let item = raw_fault
                .as_object()
                .ok_or_else(|| format!("{label}.faults entries must be objects"))?;
            let call = item
                .get("call")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("{label}.faults.call is required"))?;
            if call > 100_000 {
                return Err(format!(
                    "{label}.faults.call exceeds the 100000-call safety bound"
                ));
            }
            let fault: Fault = serde_json::from_value(
                item.get("fault")
                    .cloned()
                    .ok_or_else(|| format!("{label}.faults.fault is required"))?,
            )
            .map_err(|error| format!("invalid {label} fault: {error}"))?;
            world = world.with_fault_at(call, fault);
        }
    }
    Ok(world)
}

fn json_i128(raw: Option<&Value>, field: &str) -> Result<i128, String> {
    let value = raw.ok_or_else(|| format!("{field} is required and must be an integer"))?;
    if let Some(value) = value.as_i64() {
        return Ok(value as i128);
    }
    if let Some(value) = value.as_u64() {
        return Ok(value as i128);
    }
    Err(format!("{field} is required and must be an integer"))
}

fn json_u32(raw: Option<&Value>, field: &str) -> Result<u32, String> {
    let value = raw
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{field} is required and must be a non-negative 32-bit integer"))?;
    u32::try_from(value)
        .map_err(|_| format!("{field} is required and must be a non-negative 32-bit integer"))
}

fn content_hash_argument(
    raw: Option<&Value>,
    field: &str,
) -> Result<bioprism_ids::ContentHash, String> {
    let value = raw.ok_or_else(|| format!("{field} is required and must be a content hash"))?;
    serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid {field} content hash: {error}"))
}

fn input_array(arguments: &Value, field: &str, maximum: usize) -> Result<Vec<Value>, String> {
    let Some(raw) = arguments.get(field) else {
        return Ok(Vec::new());
    };
    let values = raw
        .as_array()
        .ok_or_else(|| format!("{field} must be an array when supplied"))?;
    if values.len() > maximum {
        return Err(format!("{field} must contain at most {maximum} items"));
    }
    Ok(values.clone())
}

fn object_array(
    object: &serde_json::Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<Value>, String> {
    let Some(raw) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = raw
        .as_array()
        .ok_or_else(|| format!("{field} must be an array when supplied"))?;
    if values.len() > maximum {
        return Err(format!("{field} must contain at most {maximum} items"));
    }
    Ok(values.clone())
}

fn input_enum_array<T: serde::de::DeserializeOwned>(
    raw: &Value,
    field: &str,
    maximum: usize,
) -> Result<Vec<T>, String> {
    let values = raw
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?;
    if values.len() > maximum {
        return Err(format!("{field} must contain at most {maximum} items"));
    }
    values
        .iter()
        .cloned()
        .map(|value| {
            serde_json::from_value(value).map_err(|error| format!("invalid {field} value: {error}"))
        })
        .collect()
}

fn required_string(object: &serde_json::Map<String, Value>, field: &str) -> Result<String, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{field} is required and must be a non-empty string"))?;
    Ok(value.to_string())
}

fn required_u64(object: &serde_json::Map<String, Value>, field: &str) -> Result<u64, String> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{field} is required and must be a non-negative integer"))
}

fn parse_value<T: serde::de::DeserializeOwned>(value: Value, field: &str) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("invalid {field}: {error}"))
}

fn parse_field<T: serde::de::DeserializeOwned>(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<T, String> {
    let value = object
        .get(field)
        .cloned()
        .ok_or_else(|| format!("{field} is required"))?;
    parse_value(value, field)
}

fn parse_impact_axes(raw: Value) -> Result<ImpactAxes, String> {
    let object = raw
        .as_object()
        .ok_or("impact must be an object with boolean axes")?;
    let axis = |field: &str| {
        object
            .get(field)
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| format!("impact.{field} must be boolean"))
            })
            .transpose()
            .map(|value| value.unwrap_or(false))
    };
    Ok(ImpactAxes {
        infrastructure: axis("infrastructure")?,
        data: axis("data")?,
        result_integrity: axis("result_integrity")?,
    })
}

fn parse_blast_radius(raw: &Value) -> Result<BlastRadius, String> {
    let object = raw.as_object().ok_or("blast_radius must be an object")?;
    let completeness = required_string(object, "completeness")?;
    let mut radius = match completeness.as_str() {
        "complete" => BlastRadius::complete(),
        "unknown" => BlastRadius::unknown(),
        "partial" => BlastRadius::partial(required_u64(object, "unreachable_edges")? as usize),
        other => return Err(format!("unknown blast_radius completeness {other:?}")),
    };
    if let Some(raw_dispositions) = object.get("dispositions") {
        let dispositions = raw_dispositions
            .as_object()
            .ok_or("blast_radius.dispositions must be an object")?;
        if dispositions.len() > 1_000 {
            return Err("blast_radius.dispositions must contain at most 1000 results".into());
        }
        for (result, raw_disposition) in dispositions {
            let disposition: ResultDisposition =
                parse_value(raw_disposition.clone(), "disposition")?;
            radius.dispose(result, disposition);
        }
    }
    Ok(radius)
}

fn parse_safety_statement(raw: &Value) -> Result<SafetyStatement, String> {
    let object = raw
        .as_object()
        .ok_or("audit record statement must be an object")?;
    let kind = required_string(object, "kind")?;
    match kind.as_str() {
        "asserted" => Ok(SafetyStatement::asserted(
            required_string(object, "by")?,
            required_string(object, "claim")?,
        )),
        "observed" => {
            let observation = object
                .get("observation")
                .ok_or("observed statement requires observation")?;
            Ok(SafetyStatement::observed(parse_safety_observation(
                observation,
            )?))
        }
        other => Err(format!("unknown audit statement kind {other:?}")),
    }
}

fn parse_safety_observation(raw: &Value) -> Result<SafetyObservation, String> {
    let object = raw
        .as_object()
        .ok_or("audit observation must be an object")?;
    let kind = required_string(object, "kind")?;
    match kind.as_str() {
        "digests_compared" => Ok(SafetyObservation::DigestsCompared {
            component: required_string(object, "component")?,
            equal: object
                .get("equal")
                .and_then(Value::as_bool)
                .ok_or("digests_compared requires boolean equal")?,
        }),
        "boundary_crossing_refused" => Ok(SafetyObservation::BoundaryCrossingRefused {
            artifact: required_string(object, "artifact")?,
            from: required_string(object, "from")?,
            to: required_string(object, "to")?,
        }),
        "witness_produced" => Ok(SafetyObservation::WitnessProduced {
            check: required_string(object, "check")?,
            kind: required_string(object, "witness_kind")?,
        }),
        "injection_path_found" => Ok(SafetyObservation::InjectionPathFound {
            origin: required_string(object, "origin")?,
            sink: required_string(object, "sink")?,
        }),
        "chain_link_recomputed" => {
            let index = required_u64(object, "index")?;
            let index = usize::try_from(index).map_err(|_| "observation index is too large")?;
            Ok(SafetyObservation::ChainLinkRecomputed { index })
        }
        other => Err(format!("unknown audit observation kind {other:?}")),
    }
}

fn parse_attestation_claim(
    object: &serde_json::Map<String, Value>,
) -> Result<AttestationClaim, String> {
    let kind = required_string(object, "kind")?;
    match kind.as_str() {
        "built_from_manifest" => Ok(AttestationClaim::BuiltFromManifest {
            manifest: required_string(object, "manifest")?,
            runner: required_string(object, "runner")?,
        }),
        "bundle_closure_verified" => Ok(AttestationClaim::BundleClosureVerified {
            bundle: required_string(object, "bundle")?,
        }),
        "independently_reproduced" => Ok(AttestationClaim::IndependentlyReproduced {
            run: required_string(object, "run")?,
            by: required_string(object, "reproduced_by")?,
        }),
        "digests_compared" => Ok(AttestationClaim::DigestsCompared {
            component: required_string(object, "component")?,
        }),
        other => Err(format!("unknown attestation claim kind {other:?}")),
    }
}

const BIOCAPABILITY_EVIDENCE_DIMENSIONS: &[&str] = &[
    "evidence_grounding",
    "information_acquisition",
    "resource_efficiency",
    "temporal_validity",
    "cross_modal_consistency",
    "causal_identification",
    "reproducibility",
    "translation_maturity",
    "multi_agent_coordination",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EvidenceState {
    Observed,
    Reproduced,
    Declared,
    Missing,
    Blocked,
    NotApplicable,
}

impl EvidenceState {
    fn as_str(self) -> &'static str {
        match self {
            EvidenceState::Observed => "observed",
            EvidenceState::Reproduced => "reproduced",
            EvidenceState::Declared => "declared",
            EvidenceState::Missing => "missing",
            EvidenceState::Blocked => "blocked",
            EvidenceState::NotApplicable => "not_applicable",
        }
    }

    fn is_measured(self) -> bool {
        matches!(self, EvidenceState::Observed | EvidenceState::Reproduced)
    }
}

fn parse_evidence_state(raw: &Value) -> Result<EvidenceState, String> {
    let value = raw.as_str().ok_or("evidence status must be a string")?;
    match value {
        "observed" => Ok(EvidenceState::Observed),
        "reproduced" => Ok(EvidenceState::Reproduced),
        "declared" => Ok(EvidenceState::Declared),
        "missing" => Ok(EvidenceState::Missing),
        "blocked" => Ok(EvidenceState::Blocked),
        "not_applicable" => Ok(EvidenceState::NotApplicable),
        other => Err(format!("unknown evidence status {other:?}")),
    }
}

fn rollup_evidence_state(states: &[EvidenceState]) -> EvidenceState {
    if states.contains(&EvidenceState::Blocked) {
        EvidenceState::Blocked
    } else if states.contains(&EvidenceState::Missing) || states.is_empty() {
        EvidenceState::Missing
    } else if states.contains(&EvidenceState::Reproduced) {
        EvidenceState::Reproduced
    } else if states.contains(&EvidenceState::Observed) {
        EvidenceState::Observed
    } else if states.contains(&EvidenceState::Declared) {
        EvidenceState::Declared
    } else {
        EvidenceState::NotApplicable
    }
}

fn validate_evidence_support(
    object: &serde_json::Map<String, Value>,
    dimension: &str,
    issues: &mut Vec<String>,
) {
    let required_string_field = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .is_some()
    };
    let required_bool_field = |field: &str| object.get(field).and_then(Value::as_bool);
    let required_number_field = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_f64)
            .is_some_and(f64::is_finite)
    };
    match dimension {
        "evidence_grounding" => {
            if !required_string_field("source") {
                issues.push("measured grounding requires a non-empty source".into());
            }
            if !required_string_field("scope") {
                issues.push("measured grounding requires an explicit scope".into());
            }
        }
        "information_acquisition" => {
            if required_bool_field("action_changed").is_none() {
                issues.push("information acquisition requires boolean action_changed".into());
            }
            if !required_number_field("cost") {
                issues.push("information acquisition requires finite numeric cost".into());
            }
        }
        "resource_efficiency" => {
            if !required_string_field("denominator") {
                issues.push("resource efficiency requires a named denominator".into());
            }
            if !required_number_field("cost") {
                issues.push("resource efficiency requires finite numeric cost".into());
            }
        }
        "temporal_validity" => {
            let decision_epoch = object.get("decision_epoch").and_then(Value::as_u64);
            let evidence_epoch = object.get("evidence_epoch").and_then(Value::as_u64);
            match (decision_epoch, evidence_epoch) {
                (Some(decision), Some(evidence)) if evidence <= decision => {}
                (Some(_), Some(_)) => {
                    issues.push("evidence was available after the decision epoch".into())
                }
                _ => issues
                    .push("temporal validity requires decision_epoch and evidence_epoch".into()),
            }
        }
        "cross_modal_consistency" => {
            let modality_count = object
                .get("modalities")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if modality_count < 2 {
                issues.push("cross-modal evidence requires at least two modalities".into());
            }
            if required_bool_field("agreement").is_none() {
                issues.push("cross-modal evidence requires boolean agreement".into());
            }
        }
        "causal_identification" => {
            if object.get("identification").and_then(Value::as_str) != Some("identified") {
                issues.push(
                    "causal evidence requires identification=identified; association is not mechanism"
                        .into(),
                );
            }
            if !required_string_field("estimand") {
                issues.push("causal evidence requires a named estimand".into());
            }
        }
        "reproducibility" => {
            if object
                .get("replications")
                .and_then(Value::as_u64)
                .is_none_or(|replications| replications < 2)
            {
                issues.push("reproducibility requires at least two replications".into());
            }
            if required_bool_field("environment_pinned") != Some(true) {
                issues.push("reproducibility requires environment_pinned=true".into());
            }
        }
        "translation_maturity" => {
            if !required_string_field("source_population")
                || !required_string_field("target_population")
            {
                issues.push("translation evidence requires source and target populations".into());
            }
            if required_bool_field("bridge") != Some(true) {
                issues.push("translation evidence requires bridge=true".into());
            }
        }
        "multi_agent_coordination" => {
            let agent_count = object
                .get("agents")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if agent_count < 2 {
                issues.push("coordination evidence requires at least two agents".into());
            }
            if !required_number_field("coordination_overhead") {
                issues.push("coordination evidence requires finite coordination_overhead".into());
            }
        }
        _ => issues.push(format!("unsupported evidence dimension {dimension:?}")),
    }
}

fn documentation_policy(arguments: &Value) -> Result<TraversalPolicy, String> {
    let mode = arguments
        .get("policy")
        .and_then(Value::as_str)
        .unwrap_or("normative");
    let mut policy = match mode {
        "normative" => TraversalPolicy::normative(),
        "exhaustive" => TraversalPolicy::exhaustive(),
        other => return Err(format!("unknown documentation policy {other:?}")),
    };
    if let Some(depth) = arguments.get("max_depth") {
        let depth = depth
            .as_u64()
            .ok_or("max_depth must be a non-negative integer")?;
        if depth > u32::MAX as u64 {
            return Err("max_depth is too large".into());
        }
        policy = policy.with_max_depth(depth as u32);
    }
    if let Some(labels) = arguments.get("denied_labels") {
        let labels = labels
            .as_array()
            .ok_or("denied_labels must be an array of strings")?;
        for label in labels {
            policy = policy.denying(
                label
                    .as_str()
                    .ok_or("denied_labels must be an array of strings")?,
            );
        }
    }
    if let Some(follow) = arguments.get("follow") {
        let follow = follow
            .as_array()
            .ok_or("follow must be an array of documentation edge names")?;
        policy.follow.clear();
        for edge in follow {
            let edge = edge
                .as_str()
                .ok_or("follow must be an array of documentation edge names")?;
            let kind = DocEdgeType::parse(edge).map_err(|error| error.to_string())?;
            policy = policy.following(kind);
        }
    }
    Ok(policy)
}

fn tool_content(value: &Value, is_error: bool) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into()),
        }],
        "isError": is_error,
    })
}

fn without_artifact_projection(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut object = object.clone();
    object.remove("artifact_registry");
    object.remove("__isError");
    object.remove("request_id");
    Value::Object(object)
}

fn projection_parent(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|projection| projection.get("content_digest"))
        .and_then(Value::as_str)
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(|digest| vec![digest.to_string()])
        .unwrap_or_default()
}

fn mission_domains(value: &Value) -> Vec<String> {
    let mut domains = BTreeSet::new();
    for path in ["/plan/steps", "/steps"] {
        if let Some(steps) = value.pointer(path).and_then(Value::as_array) {
            for step in steps {
                if let Some(domain) = step.get("domain").and_then(Value::as_str) {
                    if !domain.trim().is_empty() {
                        domains.insert(domain.to_string());
                    }
                }
            }
        }
    }
    domains.into_iter().collect()
}

fn evaluator_domains(value: &Value) -> Vec<String> {
    let mut domains = BTreeSet::new();
    let mut add_binding = |binding: &Value| {
        if let Some(domain) = binding.get("domain").and_then(Value::as_str) {
            if !domain.trim().is_empty() {
                domains.insert(domain.to_string());
            }
        }
    };
    if let Some(bindings) = value
        .pointer("/evaluator_replay/bindings")
        .and_then(Value::as_array)
    {
        for binding in bindings {
            add_binding(binding);
        }
    }
    if let Some(claims) = value
        .pointer("/evaluator_replay/claims")
        .and_then(Value::as_array)
    {
        for claim in claims {
            if let Some(bindings) = claim.get("bindings").and_then(Value::as_array) {
                for binding in bindings {
                    add_binding(binding);
                }
            }
        }
    }
    if let Some(bindings) = value.pointer("/bindings").and_then(Value::as_array) {
        for binding in bindings {
            add_binding(binding);
        }
    }
    domains.into_iter().collect()
}

fn explicit_domains(value: &Value) -> Vec<String> {
    let mut domains = BTreeSet::new();
    for path in ["/domains", "/domain_contract/domains"] {
        match value.pointer(path) {
            Some(Value::String(domain)) if !domain.trim().is_empty() => {
                domains.insert(domain.clone());
            }
            Some(Value::Array(values)) => {
                for domain in values.iter().filter_map(Value::as_str) {
                    if !domain.trim().is_empty() {
                        domains.insert(domain.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    domains.into_iter().collect()
}

/// Structured audit record on stderr, keeping stdout a clean protocol channel.
fn audit(tool: &str, arguments: &Value) {
    eprintln!(
        "{}",
        json!({ "audit": "tool_call", "tool": tool, "arguments": arguments })
    );
}

fn weave_protocol_catalog() -> Value {
    let kinds = [
        ActKind::Ask,
        ActKind::Claim,
        ActKind::Propose,
        ActKind::Accept,
        ActKind::Reject,
        ActKind::Challenge,
        ActKind::Discharge,
        ActKind::Delegate,
        ActKind::Revoke,
        ActKind::Attest,
    ];
    json!({
        "ok": true,
        "protocol": "bioprism-weave",
        "acts": kinds.into_iter().map(|kind| json!({
            "kind": kind.as_str(),
            "requires_antecedent": kind.requires_antecedent().map(|antecedents| {
                antecedents.iter().map(|antecedent| antecedent.as_str()).collect::<Vec<_>>()
            }).unwrap_or_default(),
        })).collect::<Vec<_>>(),
        "invariants": [
            "accept and reject require a prior proposal",
            "challenge requires a prior claim",
            "discharge requires a prior acceptance",
            "claims and challenges remain distinct ledger entries",
            "payloads are opaque to the protocol kernel",
        ],
    })
}

pub fn resource_definitions() -> Vec<Value> {
    vec![
        json!({
            "uri": QUERY_SCHEMA_URI,
            "name": "fiber-query/0.2 schema",
            "description": "The strict query document schema accepted by the FIBER compiler.",
            "mimeType": "application/schema+json",
        }),
        json!({
            "uri": ADAPTIVE_QUERY_SCHEMA_URI,
            "name": "fiber-query/0.5 schema",
            "description": "The strict adaptive-acquisition query schema accepted by the FIBER compiler.",
            "mimeType": "application/schema+json",
        }),
        json!({
            "uri": CERTIFICATE_SCHEMA_URI,
            "name": "fiber-context-certificate/0.1 schema",
            "description": "The certificate contract emitted by a FIBER compilation.",
            "mimeType": "application/schema+json",
        }),
        json!({
            "uri": WORLD_SCHEMA_URI,
            "name": "fiber-world/0.1 schema",
            "description": "The world document schema consumed by the compiler.",
            "mimeType": "application/schema+json",
        }),
        json!({
            "uri": CAPABILITIES_URI,
            "name": "workspace capability catalog",
            "description": "The domain capabilities, executable entrypoints, and agent-facing surfaces shipped by this workspace.",
            "mimeType": "application/json",
        }),
    ]
}

/// Stable, honest routing metadata for agents choosing among the workspace's domains.
///
/// This is deliberately a catalog rather than a claim that every domain is exposed over MCP:
/// `mcp_tools` contains only callable tools, while `cli_entrypoints` and `crates` identify the
/// available local surfaces. A consumer can therefore plan across all domains without confusing
/// a library's existence with a transport it can invoke remotely.
fn dashboard_schema_is_valid(definition: &Value) -> bool {
    const MAX_SCHEMA_BYTES: usize = 1_000_000;
    let Some(schema) = definition.get("inputSchema") else {
        return false;
    };
    let Ok(encoded) = serde_json::to_vec(schema) else {
        return false;
    };
    if encoded.len() > MAX_SCHEMA_BYTES {
        return false;
    }
    let Some(object) = schema.as_object() else {
        return false;
    };
    if object.get("type").and_then(Value::as_str) != Some("object") {
        return false;
    }
    let properties = object.get("properties").and_then(Value::as_object);
    object
        .get("required")
        .map(|required| {
            required.as_array().is_some_and(|required| {
                required.iter().all(|name| {
                    name.as_str().is_some_and(|name| {
                        properties.is_some_and(|properties| properties.contains_key(name))
                    })
                })
            })
        })
        .unwrap_or(true)
}

pub fn workspace_capabilities() -> Value {
    let mut catalogue = json!([
        {
            "id": "world_and_ingestion",
            "domains": ["world modeling", "data ingestion", "provenance"],
            "crates": ["bioprism-world", "bioprism-adapter", "bioprism-worldfactory", "bioprism-standards", "bioprism-project"],
            "mcp_tools": ["world_validate", "world_index", "adapter_plan", "tabular_ingest", "observed_world_declare", "world_claim_check", "lineage_audit", "preanalytic_apply", "project_ingest"],
            "cli_entrypoints": ["world validate", "world show", "world generate", "world index"],
            "status": "available"
        },
        {
            "id": "decision_context",
            "domains": ["typed queries", "evidence selection", "omission accounting", "repair planning", "three-valued acceptance"],
            "crates": ["bioprism-fiber", "bioprism-section", "bioprism-obligation", "bioprism-scope", "bioprism-graph", "bioprism-domain", "bioprism-project", "bioprism-repair"],
            "mcp_tools": ["fiber_compile", "fiber_refine", "fiber_explain", "fiber_verify", "projection_bundle", "obligation_gate_check", "domain_validate", "project_audit", "repair_plan", "repair_verify"],
            "cli_entrypoints": ["context explain", "context compile", "context verify", "context compare"],
            "status": "available"
        },
        {
            "id": "token_efficient_context",
            "domains": ["context budgets", "mandatory closure planning", "dry-run privacy", "policy-only comparisons", "estimation provenance"],
            "crates": ["bioprism-tokens", "bioprism-obligation", "bioprism-section"],
            "mcp_tools": ["token_context_plan"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "trajectory_and_decision_cells",
            "domains": ["JSONL trajectory ingestion", "OTLP JSON span ingestion", "semantic-loss accounting", "divergence localization", "decision segmentation", "review-gated cell proposals"],
            "crates": ["bioprism-trace", "bioprism-prism", "bioprism-benchcompiler"],
            "mcp_tools": ["trace_analyze", "trace_otel_ingest", "benchmark_trace_analyze", "benchmark_decision_audit", "benchmark_integrity_audit", "benchmark_counterfactual_check", "benchmark_oracle_review", "benchmark_compile", "benchmark_compile_review"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_federated_continual_retrieval_workflow",
            "domains": ["federated continual retrieval", "purpose-bound quorum", "aggregate-only federation", "checkpointed evidence synthesis"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_federated_continual_retrieval_synthesis_workflow_fabric"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_local_retrieval_research_workbench",
            "domains": ["local retrieval synthesis", "researcher interaction", "omission-aware views", "provenance replay"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_local_retrieval_synthesis_research_workbench"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_multimodal_retrieval_research_workbench",
            "domains": ["multimodal retrieval synthesis", "multi-study researcher interaction", "comparability views", "omission-aware provenance"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_multimodal_retrieval_synthesis_research_workbench"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_throughput_retrieval_research_workbench",
            "domains": ["throughput retrieval synthesis", "queue interaction", "overflow evidence", "replay provenance"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_throughput_retrieval_synthesis_research_workbench"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_federated_continual_retrieval_synthesis_research_workbench",
            "domains": ["federated continual retrieval synthesis", "quorum workbench", "aggregate-only researcher interaction", "omission and provenance views"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_federated_continual_retrieval_synthesis_research_workbench"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_local_retrieval_synthesis_interoperability_gateway",
            "domains": ["local retrieval synthesis interoperability", "version negotiation", "semantic-loss receipts", "protocol conformance"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_local_retrieval_synthesis_interoperability_gateway"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_multimodal_retrieval_synthesis_interoperability_gateway",
            "domains": ["multimodal retrieval synthesis interoperability", "comparability negotiation", "semantic-loss receipts", "protocol conformance"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_multimodal_retrieval_synthesis_interoperability_gateway"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_throughput_retrieval_synthesis_interoperability_gateway",
            "domains": ["throughput retrieval synthesis interoperability", "batch checkpoint admission", "semantic-loss receipts", "protocol conformance"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_throughput_retrieval_synthesis_interoperability_gateway"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_federated_continual_retrieval_synthesis_interoperability_gateway",
            "domains": ["federated continual retrieval synthesis interoperability", "purpose-bound quorum", "aggregate-only exchange", "semantic-loss receipts"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_federated_continual_retrieval_synthesis_interoperability_gateway"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_local_retrieval_synthesis_assurance_harness",
            "domains": ["local retrieval synthesis assurance", "counterexample checking", "omission-aware release gates", "replay verification"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_local_retrieval_synthesis_assurance_harness"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_multimodal_retrieval_synthesis_assurance_harness",
            "domains": ["multimodal retrieval synthesis assurance", "comparability verification", "counterexample checking", "omission-aware release gates"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_multimodal_retrieval_synthesis_assurance_harness"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_throughput_retrieval_synthesis_assurance_harness",
            "domains": ["throughput retrieval synthesis assurance", "queue and checkpoint verification", "overflow witnesses", "omission-aware release gates"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_throughput_retrieval_synthesis_assurance_harness"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_federated_continual_retrieval_synthesis_assurance_harness",
            "domains": ["federated continual retrieval synthesis assurance", "purpose-bound quorum verification", "aggregate-only locality", "omission-aware release gates"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_federated_continual_retrieval_synthesis_assurance_harness"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_local_retrieval_synthesis_federated_control_plane",
            "domains": ["local retrieval synthesis operations", "federated control plane", "capacity and health admission", "signed-approval gates"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_local_retrieval_synthesis_federated_control_plane"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_multimodal_retrieval_synthesis_federated_control_plane",
            "domains": ["multimodal multi-study retrieval synthesis operations", "federated control plane", "modality comparability", "peer quorum", "aggregate-only locality", "capacity and health admission"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_multimodal_retrieval_synthesis_federated_control_plane"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_throughput_retrieval_synthesis_federated_control_plane",
            "domains": ["prospective high-throughput retrieval synthesis operations", "federated control plane", "queue and checkpoint continuity", "peer quorum", "aggregate-only locality", "capacity admission"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_throughput_retrieval_synthesis_federated_control_plane"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "adapter_federated_continual_retrieval_synthesis_federated_control_plane",
            "domains": ["federated continual retrieval synthesis operations", "purpose-bound control plane", "checkpoint continuity", "peer quorum", "aggregate-only locality", "bounded autonomy"],
            "crates": ["bioprism-adapter", "bioprism-mcp"],
            "mcp_tools": ["adapter_federated_continual_retrieval_synthesis_federated_control_plane"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "foundation_mechanism_exploration_assurance",
            "domains": ["prospective mechanism exploration", "high-throughput assurance", "evidence and provenance closure", "baseline and replay gates", "negative-result retention"],
            "crates": ["bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": ["foundation_mechanism_exploration_assurance"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "oraclex_publication_release",
            "domains": ["publication and research-object release", "federated continual context compilation", "bounded research-copilot automation", "RO-Crate and PROV-O metadata", "provenance and evaluation closure", "negative-result disclosure", "digest-only federation", "contract migration"],
            "crates": ["bioprism-oraclex", "bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": ["oraclex_publication_release", "oraclex_context_compilation_research_copilot"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "interweave_frontier_control",
            "domains": ["prospective high-throughput Interweave control plane", "federated peer quorum", "checkpoint and queue continuity", "capacity and health admission", "aggregate-only locality", "bounded A2 operations"],
            "crates": ["bioprism-interweave", "bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": ["interweave_frontier_control"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "evaluation_and_baselines",
            "domains": ["matched evaluation", "equal engineering", "claim ladders", "adaptive panels", "capability posteriors", "release gates", "bounded waivers", "safety vetoes", "factorial designs", "component attribution", "interaction coverage", "evaluator independence", "disagreement witnesses", "abstention handling", "nonrenewable resource accounting", "fork feasibility", "failed-action waste", "prospective commitments", "rubric digest integrity", "selective publication", "contextual integrity", "channel exposure", "utility-safety Pareto"],
            "crates": ["bioprism-prism", "bioprism-baseline", "bioprism-adaptive", "bioprism-evalengine", "bioprism-bioeval", "bioprism-bioevalx", "bioprism-epistemic"],
            "mcp_tools": ["context_compare", "prism_minimize", "adaptive_panel", "posterior_gate", "evaluation_worldline_audit", "evaluation_reproduction_check", "evaluation_trajectory_check", "evaluation_observability_card", "federated_evaluation_consensus", "resource_workbench_discover", "resource_discovery_contract_v2", "governance_research_release_compile", "release_assurance_harness", "protocol_assurance_harness", "federated_multimodal_assurance", "federated_knowledge_gateway", "federated_lens_assurance", "lab_semantic_parity", "federated_retrieval_assurance", "federated_continual_retrieval_copilot", "federated_context_compilation_assurance", "federated_knowledge_representation_assurance", "federated_resource_control_plane", "weavelang_release_assurance", "federated_mechanism_control_plane", "federated_mechanism_gateway", "evidence_surveillance_copilot", "adapter_local_evidence_surveillance_research_copilot", "adapter_multimodal_evidence_surveillance_research_copilot", "adapter_throughput_evidence_surveillance_research_copilot", "adapter_federated_continual_evidence_surveillance_research_copilot", "adapter_local_evidence_surveillance_workflow_fabric", "adapter_multimodal_evidence_surveillance_workflow_fabric", "adapter_throughput_evidence_surveillance_workflow_fabric", "adapter_federated_continual_evidence_surveillance_workflow_fabric", "multimodal_retrieval_synthesis", "adapter_local_retrieval_synthesis_inference_engine", "adapter_local_retrieval_synthesis_contract_model", "adapter_local_retrieval_synthesis_research_copilot", "adapter_multimodal_retrieval_synthesis_research_copilot", "adapter_throughput_retrieval_synthesis_research_copilot", "adapter_federated_continual_retrieval_synthesis_research_copilot", "adapter_local_retrieval_synthesis_workflow_fabric", "adapter_multimodal_retrieval_synthesis_workflow_fabric", "adapter_throughput_retrieval_synthesis_workflow_fabric", "adapter_multimodal_retrieval_synthesis_inference_engine", "adapter_throughput_retrieval_synthesis_inference_engine", "adapter_throughput_retrieval_synthesis_contract_model", "adapter_federated_retrieval_synthesis_inference_engine", "adapter_federated_retrieval_synthesis_contract_model", "adapter_context_compilation_assurance", "multimodal_knowledge_workflow", "adapter_resource_workbench", "adapter_ingestion_gateway", "adapter_quality_envelope", "adapter_experiment_design_control", "adapter_protocol_simulation", "adapter_instrument_mesh", "adapter_execution_control", "adapter_analysis_portfolio", "adapter_interpretation_assurance", "governance_federated_continual_interpretation_assurance", "adapter_replication_assurance", "adapter_release_assurance", "adapter_determinism_gateway", "adapter_provenance_assurance", "adapter_policy_gateway", "adapter_federation_workflow", "adapter_reliability_copilot", "adapter_interoperability_gateway", "adapter_evaluation_assurance", "adapter_research_workbench", "adapter_contract_frontier", "adapter_limitation_closure", "adapter_dependency_composition", "adapter_semantic_parity", "adapter_scale_frontier", "adapter_adversarial_recovery", "adapter_federated_commons", "adapter_bounded_evolution", "mcp_bounded_evolution_assurance", "analysis_qualify", "bioeval_reference_audit", "bioeval_acquisition_audit", "bioeval_grounding_audit", "bioeval_estimand_audit", "bioeval_evaluator_audit", "bioeval_plane_audit", "bioeval_metamorphic_audit", "bioeval_waiver_audit", "bioeval_design_audit", "bioeval_mesh_audit", "bioeval_burden_audit", "bioeval_reveal_audit", "bioeval_boundary_audit", "epistemic_voi", "epistemic_adaptive_acquisition", "epistemic_adaptive_costed", "epistemic_adaptive_execute", "epistemic_decision_quotient", "epistemic_context_audit", "epistemic_selection_audit"],
            "cli_entrypoints": ["prism fork", "prism minimize", "context compare"],
            "status": "available"
        },
        {
            "id": "benchmark_pack_portfolio",
            "domains": ["agent benchmark packs", "biological benchmark packs", "capability coverage", "oracle tiers", "release sequencing"],
            "crates": ["bioprism-packs", "bioprism-scale", "bioprism-mutation", "bioprism-benchcompiler"],
            "mcp_tools": ["pack_catalogue", "pack_coverage_audit", "pack_release_audit", "pack_health_assess", "benchmark_trace_analyze", "benchmark_decision_audit", "benchmark_integrity_audit", "benchmark_counterfactual_check", "benchmark_oracle_review", "benchmark_compile", "benchmark_compile_review", "mutation_family", "scale_family_split_verify"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "megafactory_scale_and_oracles",
            "domains": ["mechanistic twin simulation", "model discrepancy", "distributed placement", "oracle independence", "fencing and duplicate execution"],
            "crates": ["bioprism-megafactory", "bioprism-scale", "bioprism-factory"],
            "mcp_tools": ["megafactory_twin_audit", "megafactory_placement_audit", "scale_family_split_verify", "factory_lifecycle_simulate", "megafactory_mechanism_exploration_federated_control_plane"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "mutation_and_causal_discovery",
            "domains": ["metamorphic testing", "causal divergence", "robustness"],
            "crates": ["bioprism-mutation", "bioprism-benchcompiler", "bioprism-stress", "bioprism-influence"],
            "mcp_tools": ["mutation_family", "benchmark_counterfactual_check", "benchmark_oracle_review", "benchmark_compile", "benchmark_compile_review", "stress_profile", "stress_report", "influence_analyze", "influence_federated_continual_interpretation"],
            "cli_entrypoints": ["mutate family"],
            "status": "available"
        },
        {
            "id": "bioevaluation_reference_contracts",
            "domains": ["reference distributions", "reference resolution", "dispersion attribution", "claim grounding", "contradiction edges", "specimen lineage", "stale evidence", "estimand declaration", "identification posture", "evaluator health", "harness failure", "scoring plane", "unscored dimensions", "metamorphic response", "false sensitivity", "false invariance", "release-gate waivers", "safety vetoes", "factorial designs", "component contrasts", "interaction cell coverage", "evaluator independence", "shared-input classes", "disagreement witnesses", "abstention preservation", "nonrenewable resource accounting", "branch inheritance", "unit-safe draws", "prospective commitments", "rubric digest integrity", "selective publication", "contextual integrity", "channel exposure", "utility-safety Pareto", "evaluation refusal boundaries"],
            "crates": ["bioprism-bioeval", "bioprism-bioevalx"],
            "mcp_tools": ["bioeval_reference_audit", "bioeval_acquisition_audit", "bioeval_grounding_audit", "bioeval_estimand_audit", "bioeval_evaluator_audit", "bioeval_plane_audit", "bioeval_metamorphic_audit", "bioeval_waiver_audit", "bioeval_design_audit", "bioeval_mesh_audit", "bioeval_burden_audit", "bioeval_reveal_audit", "bioeval_boundary_audit"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "biological_domains",
            "domains": ["BioIR", "oncology", "neurosurgical research", "modalities", "specimen lineage", "reference worlds"],
            "crates": ["bioprism-bioir", "bioprism-onco", "bioprism-neurosurgery", "bioprism-oncoworlds", "bioprism-modalities", "bioprism-bioworlds", "bioprism-worldfactory"],
            "mcp_tools": ["bioworlds_catalog", "world_generate", "modality_catalog", "modality_support_check", "modality_transport_check", "modality_comparability_check", "literature_bind_check", "measurement_compare", "contradiction_review", "onco_boundary_check", "onco_response_assess", "onco_worldline_view", "onco_classification_check", "onco_outcome_analyze", "oncoworlds_methylation_classify", "oncoworlds_methylation_compare", "oncoworlds_radiogenomic_check", "oncoworlds_era_shift_check", "oncoworlds_equity_check", "oncoworlds_entity_world_check", "neurosurgery_intake_plan", "neurosurgery_intake_mission", "neurosurgery_intake_portfolio", "neurosurgery_catalogue", "neurosurgery_evidence_audit", "neurosurgery_specialty_evidence_map", "neurosurgery_case_asset_manifest", "neurosurgery_case_fhir_import", "neurosurgery_case_dicom_import", "neurosurgery_case_dicom_evidence_workflow", "neurosurgery_case_asset_review_disposition", "neurosurgery_evidence_synthesis", "neurosurgery_glioma_molecular_map", "neurosurgery_evidence_graph", "neurosurgery_real_data_coverage", "neurosurgery_real_data_cohort_landscape", "neurosurgery_real_data_reconciliation", "neurosurgery_real_data_freshness", "neurosurgery_real_data_diff", "neurosurgery_real_data_refresh_audit", "neurosurgery_real_data_review_queue", "neurosurgery_real_data_review_disposition", "neurosurgery_real_data_evidence_packet", "neurosurgery_real_data_autonomous_workflow", "neurosurgery_real_data_reasoning_context", "neurosurgery_real_data_draft_audit", "neurosurgery_public_literature_evidence_packet", "neurosurgery_public_literature_reasoning_context", "neurosurgery_public_literature_draft_audit", "neurosurgery_public_literature_matrix", "neurosurgery_public_literature_freshness", "neurosurgery_public_literature_refresh_audit", "neurosurgery_literature_link_audit", "neurosurgery_public_literature_integrity_audit", "neurosurgery_public_literature_review_queue", "neurosurgery_public_literature_workbench", "neurosurgery_public_literature_portfolio", "neurosurgery_research_brief", "neurosurgery_research_plan", "neurosurgery_evidence_acquisition", "neurosurgery_evidence_program", "neurosurgery_plan", "neurosurgery_real_data_query", "neurosurgery_real_data_trial_landscape", "neurosurgery_real_data_molecular_coverage", "neurosurgery_public_literature_query", "neurosurgery_session", "neurosurgery_mission"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "biological_ir_and_query",
            "domains": ["BioQL syntax", "typed biological fields", "units and frames", "genome builds", "temporal semantics", "query access declarations"],
            "crates": ["bioprism-biolang", "bioprism-bioir", "bioprism-standards", "bioprism-scope"],
            "mcp_tools": ["bioql_compile"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "foundation_contracts",
            "domains": ["falsifiable biological contracts", "applicability envelopes", "world classes", "counterfactual boundaries", "plane consistency"],
            "crates": ["bioprism-foundation", "bioprism-scope", "bioprism-bioir", "bioprism-standards"],
            "mcp_tools": ["foundation_contract_check", "bioql_compile", "world_validate"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "conformance_context_compilation_federation",
            "domains": ["prospective high-throughput context compilation", "federated conformance", "peer quorum", "fixture identity", "checkpoint and capacity admission", "aggregate-only locality"],
            "crates": ["bioprism-conformance", "bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": ["conformance_context_compilation_federated_control"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "federated_publication_release_inference",
            "domains": ["federated continual publication", "research-object release inference", "deterministic ranking", "origin quorum", "negative evidence", "aggregate-only locality"],
            "crates": ["bioprism-services", "bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": ["federated_publication_release_inference"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "oncoworlds_identity_and_transport",
            "domains": ["participant identity", "specimen and lesion joins", "disease epochs", "cross-scope transport"],
            "crates": ["bioprism-oncoworlds", "bioprism-scope", "bioprism-standards"],
            "mcp_tools": ["oncoworlds_identity_join"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "mutation_knowledge_federated_control",
            "domains": ["federated mutation knowledge representation", "continual candidate admission", "origin quorum", "deterministic ranking", "negative evidence", "aggregate-only locality"],
            "crates": ["bioprism-mutation", "bioprism-oracle", "bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": ["mutation_knowledge_federated_control", "mcp_knowledge_representation_contract"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "oncoworlds_models_and_assays",
            "domains": ["patient-derived models", "methylation classification", "classifier versioning", "radiogenomics"],
            "crates": ["bioprism-oncoworlds", "bioprism-onco", "bioprism-scope"],
            "mcp_tools": ["oncoworlds_methylation_classify", "oncoworlds_methylation_compare", "oncoworlds_radiogenomic_check", "oncoworlds_model_transport"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "oncoworlds_clonal_evolution",
            "domains": ["clonal populations", "phylogeny compatibility", "resistance hypotheses"],
            "crates": ["bioprism-oncoworlds", "bioprism-onco"],
            "mcp_tools": ["oncoworlds_clonal_history_check", "oncoworlds_clonal_evidence_check"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "oncoworlds_shift_and_equity",
            "domains": ["classification-era mapping", "site resource absence", "population descriptors", "subgroup equity evidence"],
            "crates": ["bioprism-oncoworlds", "bioprism-onco"],
            "mcp_tools": ["oncoworlds_era_shift_check", "oncoworlds_equity_check", "oncoworlds_entity_world_check"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "safety_privacy_and_policy",
            "domains": ["consent", "privacy", "information flow", "threat modeling", "sandbox posture"],
            "crates": ["bioprism-policy", "bioprism-safety", "bioprism-bioethics", "bioprism-governance"],
            "mcp_tools": ["policy_screen", "autonomy_batch_admit", "workflow_batch_execute", "safety_posture", "security_redteam_simulate", "safety_release_gate", "medical_boundary_check", "bioethics_action_review", "bioethics_human_subject_screen", "bioethics_dual_use_review", "bioethics_validation_check", "bioethics_representation_audit", "bioethics_scale_frontier_contract"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "influence_bounds_and_abstract_analysis",
            "domains": ["numeric influence bounds", "structural omission analysis", "abstract interpretation", "sound perturbation analysis"],
            "crates": ["bioprism-influence", "bioprism-backends", "bioprism-section"],
            "mcp_tools": ["influence_analyze"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "agent_orchestration",
            "domains": ["typed acts", "session types", "budgets", "sagas", "quorum"],
            "crates": ["bioprism-weave", "bioprism-weavelang", "bioprism-choreography", "bioprism-fabric", "bioprism-interweave"],
            "mcp_tools": ["weave_protocol_catalog", "weavelang_compile", "weavelang_computational_execution_assurance", "choreography_check", "fabric_synthesize", "interweave_workflow_catalogue", "interweave_workflow_execute", "interweave_workflow_execution_evidence", "interweave_workflow_execution_evidence_import", "interweave_workflow_execution_evidence_query", "interweave_workflow_execution_evidence_get", "mission_evaluator_discover", "mission_evaluator_review", "mission_evaluator_replay", "mission_evaluator_replay_compare", "mission_evidence_bundle_verify", "mission_evidence_bundle_import", "mission_evidence_bundle_query", "mission_evidence_bundle_get"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "autonomous_research_campaigns",
            "domains": ["bounded research DAGs", "durable action authorization", "metadata-only checkpoints", "provider-free research execution"],
            "crates": ["bioprism-research-campaign", "bioprism-research", "bioprism-brain"],
            "mcp_tools": ["research_campaign_run_offline"],
            "cli_entrypoints": [],
            "status": "available_with_declared_limits"
        },
        {
            "id": "autonomous_brain",
            "domains": ["model selection", "prompt assembly", "bounded autonomous planning", "online bandit adaptation", "evaluator-backed learning evidence", "provider-neutral invocation contracts"],
            "crates": ["bioprism-brain", "bioprism-runtime", "bioprism-routing", "bioprism-adaptive"],
            "python_artifacts": ["python/prism_sdk/llm_runtime.py", "python/prism_sdk/brain.py"],
            "mcp_tools": ["brain_model_select", "brain_model_select_contextual", "brain_prompt_assemble", "brain_plan", "brain_bandit_select", "brain_bandit_update", "brain_outcome_record", "brain_job_submit", "brain_job_status", "brain_job_events", "brain_job_approval", "brain_job_claim", "brain_job_claim_next", "brain_job_renew", "brain_job_checkpoint", "brain_job_complete", "brain_job_fail", "brain_job_reconcile", "brain_job_cancel", "brain_model_health", "brain_replay_evaluate"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "glioma_autonomous_research_engine",
            "domains": ["preclinical glioma research", "evidence surveillance", "multimodal QC", "molecular mechanism exploration", "experiment design", "protocol simulation", "instrument preflight", "reproducible computation", "longitudinal trajectory analysis", "state-transition interpretation", "omission-stress robustness", "replication and negative results", "adaptive next-action selection"],
            "crates": ["bioprism-research", "bioprism-onco", "bioprism-foundation", "bioprism-mcp"],
            "mcp_tools": [
                "glioma_research_dry_run",
                "glioma_workflow_plan",
                "glioma_protocol_simulate",
                "glioma_protocol_scenario_ensemble",
                "glioma_protocol_branch_optimize",
                "glioma_protocol_autonomous_execute",
                "glioma_protocol_evidence_surface",
                "glioma_protocol_multistudy_fusion",
                "glioma_protocol_transport_gate",
                "glioma_protocol_execute",
                "glioma_protocol_compensation",
                "glioma_action_portfolio_execute",
                "glioma_autonomous_campaign_execute",
                "glioma_research_autopilot_execute",
                "glioma_evidence_campaign_execute",
                "glioma_evidence_refresh_campaign_execute",
                "glioma_knowledge_resolution_campaign_execute",
                "glioma_decision_context_campaign_execute",
                "glioma_decision_branch_campaign_execute",
                "glioma_adaptive_decision_branch_campaign_execute",
                "glioma_decision_operating_cycle",
                "glioma_multimodal_ingestion_campaign_execute",
                "glioma_multimodal_readiness_gate",
                "glioma_multimodal_operating_cycle",
                "glioma_computation_execute",
                "glioma_computation_lineage",
                "glioma_computation_reproducibility",
                "glioma_federated_replay_discrepancy_scan",
                "glioma_compute_environment_lock",
                "glioma_environment_resolution",
                "glioma_reproducible_task_submit",
                "glioma_computation_event_stream",
                "glioma_federated_workflow_template_exchange",
                "glioma_registry_artifact_resolve",
                "glioma_federated_replay_conformance",
                "glioma_compute_cache_govern",
                "glioma_multistudy_cache_partition",
                "glioma_compute_capacity_plan",
                "glioma_federated_compute_capacity_exchange",
                "glioma_computation_portfolio_plan",
                "glioma_computation_placement",
                "glioma_computation_placement_stress_evaluate",
                "glioma_computation_portfolio_execute",
                "glioma_computation_campaign_execute",
                "glioma_computation_recovery_execute",
                "glioma_robustness_guided_computation_execute",
                "glioma_computation_workflow_execute",
                "glioma_computation_operating_cycle",
                "glioma_partial_result_semantics",
                "glioma_artifact_lineage_index",
                "glioma_computation_run_inspector",
                "glioma_high_throughput_compute_timeline",
                "glioma_computation_interpretation_frontier_compile",
                "glioma_computation_interpretation_frontier_execute",
                "glioma_computation_interpretation_evidence_gate",
                "glioma_research_director_execute",
                "glioma_program_scheduler_execute",
                "glioma_experiment_frontier_controller_execute",
                "glioma_causal_claim_adjudication_execute",
                "glioma_mechanism_discovery_engine_execute",
                "glioma_evidence_gated_research_execute",
                "glioma_autonomous_research_engine_execute",
                "glioma_autonomous_research_engine_evaluate",
                "glioma_autonomous_research_engine_stress_evaluate",
                "glioma_autonomous_research_engine_trace_evaluate",
                "glioma_stage_worker_routes_compile",
                "glioma_autonomous_research_engine_stage_execute",
                "glioma_autonomous_research_workflow_execute",
                "glioma_evidence_gated_stage_engine_execute",
                "glioma_evidence_gated_stage_engine_instrument_execute",
                "glioma_evidence_gated_stage_engine_computation_execute",
                "glioma_evidence_gated_stage_engine_interpretation_execute",
                "glioma_evidence_gated_stage_engine_replication_execute",
                "glioma_evidence_gated_stage_engine_release_execute",
                "glioma_evidence_gated_stage_engine_federation_execute",
                "glioma_autonomous_program_cycle",
                "glioma_adaptive_workflow",
                "glioma_interpretation_synthesize",
                "glioma_interpretation_operating_cycle",
                "glioma_adaptive_research_frontier",
                "glioma_adaptive_frontier_execute",
                "glioma_adaptive_interpretation_campaign_execute",
                "glioma_robustness_suite",
                "glioma_trajectory_analyze",
                "glioma_state_transition_analyze",
                "glioma_lineage_propagation_analyze",
                "glioma_lineage_response_decompose",
                "glioma_lineage_transport_analyze",
                "glioma_transportability_analyze",
                "glioma_longitudinal_transport_analyze",
                "glioma_multistudy_concordance_analyze",
                "glioma_prospective_contradiction_plan",
                "glioma_registered_outcome_reporting_audit",
                "glioma_registered_outcome_record_build",
                "glioma_registered_outcome_evidence_panel",
                "glioma_registered_outcome_sensitivity_analyze",
                "glioma_causal_contrast",
                "glioma_cross_model_claim_envelope",
                "glioma_cross_model_replication_frontier",
                "glioma_cross_model_replication_mission",
                "glioma_cross_model_replication_mission_execute",
                "glioma_causal_mediation",
                "glioma_stratified_causal_adjustment",
                "glioma_dynamic_policy_evaluate",
                "glioma_dose_response",
                "glioma_adaptive_allocation",
                "glioma_adaptive_allocation_campaign_execute",
                "glioma_sequential_design",
                "glioma_power_reestimate",
                "glioma_power_stress_surface",
                "glioma_carryover_sequence_design",
                "glioma_sequential_campaign_execute",
                "glioma_closed_loop_campaign",
                "glioma_experiment_operating_cycle",
                "glioma_combination_synergy",
                "glioma_adaptive_dose_surface",
                "glioma_multimodal_concordance",
                "glioma_multimodal_dropout_stress",
                "glioma_multimodal_missingness",
                "glioma_multimodal_reliability",
                "glioma_multimodal_portfolio",
                "glioma_multimodal_drift",
                "glioma_multimodal_evidence_fusion",
                "glioma_multimodal_sensitivity",
                "glioma_multimodal_decision_gate",
                "glioma_multimodal_contradiction_adjudication",
                "glioma_multimodal_quality_forecast",
                "glioma_multimodal_quality_scheduler",
                "glioma_multimodal_quality_execute",
                "glioma_multimodal_quality_adaptive_campaign",
                "glioma_multimodal_quality_transport",
                "glioma_multimodal_quality_root_cause",
                "glioma_multimodal_quality_remediation",
                "glioma_multimodal_quality_recovery",
                "glioma_multimodal_consensus",
                "glioma_multimodal_harmonize",
                "glioma_multimodal_latent_factors",
                "glioma_multimodal_graph_fusion",
                "glioma_temporal_multimodal_fusion",
                "glioma_temporal_spatial_alignment",
                "glioma_clonal_evolution",
                "glioma_clone_perturbation_panel",
                "glioma_clone_panel_outcomes",
                "glioma_clone_continuation",
                "glioma_adaptive_clone_campaign_execute",
                "glioma_spatial_niches",
                "glioma_spatial_communication",
                "glioma_spatial_state_propagation",
                "glioma_spatial_registration",
                "glioma_causal_sensitivity",
                "glioma_research_select_actions",
                "glioma_scientific_frontier",
                "glioma_scientific_frontier_execute",
                "glioma_program_catalog",
                "glioma_evidence_qualify",
                "glioma_evidence_cluster_index",
                "glioma_evidence_novelty_adjudication",
                "glioma_evidence_stream_snapshot",
                "glioma_evidence_prospective_triage",
                "glioma_evidence_researcher_workbench",
                "glioma_multimodal_researcher_workbench",
                "glioma_evidence_verification_gate",
                "glioma_multisite_outcome_reconciliation",
                "glioma_evidence_knowledge_bridge",
                "glioma_long_horizon_evidence_calibration",
                "glioma_federated_outcome_transport",
                "glioma_federated_evidence_operating_cycle",
                "glioma_federated_batch_scheduler",
                "glioma_continual_promotion_control",
                "glioma_federated_evidence_acquisition_policy",
                "glioma_evidence_frontier_join",
                "glioma_multimodal_evidence_gap_router",
                "glioma_evidence_acquisition_feedback",
                "glioma_federated_execution_handoff",
                "glioma_evidence_surveillance",
                "glioma_evidence_novelty_radar",
                "glioma_evidence_temporal_shift",
                "glioma_federated_evidence_shift",
                "glioma_evidence_priority",
                "glioma_evidence_acquisition_plan",
                "glioma_evidence_acquisition_campaign_execute",
                "glioma_evidence_operating_cycle",
                "glioma_evidence_calibrate",
                "glioma_evidence_triangulate",
                "glioma_evidence_contradiction_cut",
                "glioma_knowledge_compile",
                "glioma_knowledge_compose",
                "glioma_knowledge_consistency",
                "glioma_knowledge_drift",
                "glioma_knowledge_closure",
                "glioma_multi_study_knowledge",
                "glioma_prospective_knowledge_monitor",
                "glioma_federated_continual_knowledge",
                "glioma_federated_continual_agent",
                "glioma_local_research_workflow",
                "glioma_multimodal_knowledge_workflow",
                "glioma_workflow_recovery",
                "glioma_knowledge_action_outcome_assimilation",
                "glioma_claim_experiment_closure",
                "glioma_claim_evidence_reconciliation",
                "glioma_closed_loop_frontier",
                "glioma_prospective_belief_calibration",
                "glioma_frontier_campaign",
                "glioma_knowledge_protocol_gateway",
                "glioma_multimodal_knowledge_protocol_gateway",
                "glioma_multimodal_ingestion_manifest",
                "glioma_research_workflow_admission",
                "glioma_federated_knowledge",
                "glioma_belief_revision",
                "glioma_knowledge_frontier",
                "glioma_knowledge_gap_compile",
                "glioma_knowledge_action_compile",
                "glioma_knowledge_action_bridge",
                "glioma_knowledge_selection_cycle",
                "glioma_knowledge_action_dispatch",
                "glioma_autonomous_gap_cycle",
                "glioma_knowledge_synthesis_operating_cycle",
                "glioma_decision_context",
                "glioma_decision_context_artifact",
                "glioma_decision_context_snapshot_store",
                "glioma_partition_resilient_context_checkpoint",
                "glioma_federated_context_access_govern",
                "glioma_multi_study_context_artifact",
                "glioma_cross_study_context_difference",
                "glioma_cross_study_context_invariance",
                "glioma_multi_study_workflow_plan",
                "glioma_multi_study_execution_receipt",
                "glioma_multi_study_context_epoch_replay",
                "glioma_federated_decision_context",
                "glioma_federated_continual_context_promotion",
                "glioma_decision_context_replay",
                "glioma_decision_branch_evidence",
                "glioma_decision_admission_gate",
                "glioma_decision_value_optimizer",
                "glioma_decision_value_calibrator",
                "glioma_adaptive_decision_controller",
                "glioma_decision_loop_governor",
                "glioma_decision_action_graph",
                "glioma_decision_mission_execute",
                "glioma_decision_omission_certificate",
                "glioma_decision_branch_plan",
                "glioma_uncertainty_branch_explorer",
                "glioma_decision_action_plan",
                "glioma_multimodal_qc",
                "glioma_mechanism_explore",
                "glioma_mechanism_discriminate",
                "glioma_temporal_multimodal_mechanism_fusion",
                "glioma_mechanism_identifiability",
                "glioma_mechanism_invariance",
                "glioma_mechanism_intervention_value",
                "glioma_mechanism_bayesian_update",
                "glioma_mechanism_evidence_assimilation",
                "glioma_mechanism_closed_loop",
                "glioma_mechanism_multi_fidelity_control",
                "glioma_mechanism_feedback_replan",
                "glioma_mechanism_workflow_compile",
                "glioma_mechanism_workflow_assure",
                "glioma_mechanism_multi_study_workflow_compile",
                "glioma_mechanism_prospective_control",
                "glioma_mechanism_fidelity_bridge",
                "glioma_mechanism_robustness_stress",
                "glioma_mechanism_state_filter",
                "glioma_mechanism_state_smoother",
                "glioma_mechanism_consensus",
                "glioma_mechanism_calibrate",
                "glioma_mechanism_action_plan",
                "glioma_adaptive_mechanism_policy",
                "glioma_adaptive_mechanism_campaign_execute",
                "glioma_calibrated_mechanism_campaign_execute",
                "glioma_mechanism_discrimination_campaign_execute",
                "glioma_mechanism_operating_cycle",
                "glioma_mechanism_graph_propagate",
                "glioma_pathway_activity",
                "glioma_multimodal_mechanism_campaign",
                "glioma_multimodal_mechanism_campaign_execute",
                "glioma_mechanism_autopilot_execute",
                "glioma_mechanism_dynamics",
                "glioma_mechanism_counterfactual",
                "glioma_mechanism_ensemble_counterfactual",
                "glioma_robust_intervention_portfolio",
                "glioma_mechanism_validation_plan",
                "glioma_validation_batch_assess",
                "glioma_validation_campaign_execute",
                "glioma_validation_replication_gate",
                "glioma_validation_replication_campaign_execute",
                "glioma_replication_federated_transport_execute",
                "glioma_replication_closure_frontier",
                "glioma_replication_closure_execute",
                "glioma_replication_closure_campaign_execute",
                "glioma_replication_closure_interpret",
                "glioma_mechanism_validation_protocol_compile",
                "glioma_mechanism_validation_protocol_execute",
                "glioma_information_design",
                "glioma_adaptive_panel",
                "glioma_replication_plan",
                "glioma_replication_continuation",
                "glioma_replication_protocol_compile",
                "glioma_robust_experiment_design",
                "glioma_heterogeneity_aware_experiment_portfolio",
                "glioma_heterogeneity_portfolio_mission",
                "glioma_blocked_randomization_design",
                "glioma_adaptive_information_campaign",
                "glioma_active_learning",
                "glioma_posterior_batch",
                "glioma_active_learning_campaign_execute",
                "glioma_robust_active_learning",
                "glioma_robust_active_learning_campaign_execute",
                "glioma_multi_fidelity_optimize",
                "glioma_instrument_calibration",
                "glioma_instrument_signal_extract",
                "glioma_instrument_batch_stability",
                "glioma_instrument_multichannel_concordance",
                "glioma_federated_instrument_consensus",
                "glioma_instrument_preflight",
                "glioma_instrument_fleet_schedule",
                "glioma_instrument_fleet_health",
                "glioma_acquisition_capacity_plan",
                "glioma_cross_site_protocol_conformance",
                "glioma_instrument_maintenance_plan",
                "glioma_assay_provenance_audit",
                "glioma_acquisition_operations_snapshot",
                "glioma_instrument_operator_approval",
                "glioma_federated_device_capability_manifest",
                "glioma_federated_instrument_operations",
                "glioma_instrument_fleet_execute",
                "glioma_instrument_execute",
                "glioma_instrument_recovery_plan",
                "glioma_instrument_campaign_execute",
                "glioma_adaptive_instrument_campaign_execute",
                "glioma_instrument_operating_cycle",
                "glioma_instrument_assay_adjudicate",
                "glioma_instrument_science_loop_execute",
                "glioma_instrument_research_frontier_execute",
                "glioma_experiment_design",
                "glioma_contrast_panel_design",
                "glioma_analysis_run",
                "glioma_replication_assess",
                "glioma_replication_meta_analyze",
                "glioma_replication_campaign_execute",
                "glioma_autonomous_research_mission_execute",
                "glioma_autonomous_research_mission_recover",
                "glioma_intent_mission_execute",
                "glioma_multimodal_mission_execute",
                "glioma_multi_fidelity_campaign_execute",
                "glioma_federated_benchmark_consensus",
                "glioma_federated_benchmark_power",
                "glioma_heterogeneity_adaptive_benchmark_power",
                "glioma_federated_aggregate_anomaly_detect",
                "glioma_federated_site_selection_plan",
                "glioma_continual_benchmark_monitor",
                "glioma_federation_capacity_plan",
                "glioma_federated_benchmark_dry_run",
                "glioma_federated_interpretation",
                "glioma_federated_benchmark_site_plan",
                "glioma_federated_mechanism_transport",
                "glioma_federated_mechanism_transport_campaign_execute",
                "glioma_federated_benchmark_campaign_execute",
                "glioma_federated_benchmark_operating_cycle",
                "glioma_multisite_benchmark_workflow",
                "glioma_benchmark_director_snapshot",
                "glioma_benchmark_job_execute",
                "glioma_cross_site_evidence_explore",
                "glioma_quorum_admission_assess",
                "glioma_site_participation_review",
                "glioma_contribution_integrity_verify",
                "glioma_federation_operations_snapshot",
                "glioma_participant_exchange_execute",
                "glioma_signed_aggregate_submit",
                "glioma_federated_adaptive_campaign_execute",
                "glioma_replay_campaign_execute",
                "glioma_replay_history_reconcile",
                "glioma_local_release_workflow_execute",
                "glioma_multistudy_release_reconcile",
                "glioma_release_batch_execute",
                "glioma_federated_continual_release_reconcile",
                "glioma_local_release_review_packet_compile",
                "glioma_portfolio_review_workbench_reconcile",
                "glioma_release_batch_review_workbench_reconcile",
                "glioma_local_release_signature_payload_prepare",
                "glioma_local_release_signature_verify",
                "glioma_release_trust_policy_payload_prepare",
                "glioma_release_trust_policy_evaluate",
                "glioma_research_object_release_gate",
                "glioma_reproducibility_completeness_score",
                "glioma_qualification_preservation_audit",
                "glioma_release_metadata_normalize",
                "glioma_release_attestation_issue",
                "glioma_artifact_integrity_scan",
                "glioma_release_preview",
                "glioma_release_queue_snapshot",
                "glioma_release_shareability_check",
                "glioma_reproducibility_bundle_compile",
                "glioma_multistudy_release_compose",
                "glioma_comparative_release_explore",
                "glioma_continuous_release_compile",
                "glioma_federated_release_compile",
                "glioma_site_capability_envelope_compile",
                "glioma_site_provenance_attest",
                "glioma_federated_benchmark_record_execute",
                "glioma_benchmark_governance_cycle_compile",
                "glioma_aggregate_phenotype_summary_compile",
                "glioma_research_object_dependency_leakage_audit",
                "glioma_release_operating_cycle",
                "glioma_research_object_prepare",
                "glioma_release_disclosure_register_build",
                "glioma_release_disclosure_panel_reconcile",
                "glioma_release_disclosure_batch_reconcile",
                "glioma_multimodal_research_object_prepare",
                "glioma_research_object_migration_plan",
                "glioma_archive_migration_execute",
                "glioma_release_signature_verify",
                "glioma_research_object_conformance_check",
                "glioma_federated_release_sharing_check",
                "glioma_release_event_protocol_replay",
                "glioma_version_retention_plan",
                "glioma_distributed_archive_mirror",
                "glioma_release_queue_schedule",
                "glioma_consortium_publication_steward",
                "glioma_research_object_exchange_plan",
                "glioma_research_object_dependency_closure"
            ],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "frontier_capability_extensions",
            "domains": ["crate-specific frontier capabilities", "cross-domain research workflows", "production capability extensions", "MCP transport parity"],
            "crates": ["workspace frontier crates", "bioprism-mcp"],
            "mcp_tools": [
                "adapter_federated_context_copilot",
                "adapter_federated_continual_evidence_surveillance_research_workbench",
                "adapter_local_evidence_surveillance_research_workbench",
                "adapter_multimodal_evidence_surveillance_research_workbench",
                "adapter_throughput_evidence_surveillance_research_workbench",
                "atlashub_mechanism_exploration_assurance",
                "atlashub_provenance_signing_inference_engine",
                "atlashub_quality_control_contract_model",
                "atlashub_quality_control_research_copilot",
                "atlashub_replication_negative_results_federated_control_plane",
                "atlasx_computational_execution_assurance",
                "atlasx_context_compilation_assurance",
                "atlasx_federated_execution_control_plane",
                "atlasx_mechanism_contract",
                "backends_federated_retrieval_synthesis_workflow",
                "bioethics_evidence_surveillance",
                "bioethics_experiment_design_workflow_fabric",
                "bioethics_multimodal_bounded_evolution_assurance",
                "bioethics_multimodal_context_compilation_assurance",
                "bioethics_prospective_computational_execution_assurance",
                "bioethics_statistical_analysis_assurance",
                "bioworlds_federated_context_research_workbench",
                "bioworlds_knowledge_workflow_fabric",
                "bioworlds_resource_discovery_copilot",
                "conformance_context_compilation_assurance",
                "conformance_retrieval_synthesis_contract_model",
                "dataops_provenance_signing_workflow_fabric",
                "devplat_multimodal_limitation_closure_assurance",
                "devx_context_compilation_contract",
                "devx_evidence_surveillance_control",
                "docgraph_instrument_action_contract",
                "epistemic_experiment_design_research_workbench",
                "epistemic_retrieval_synthesis_federated_control_plane",
                "evalengine_federated_protocol_simulation_copilot",
                "evalengine_local_mechanism_exploration_assurance",
                "fabric_experiment_design_contract_model",
                "fabric_experiment_design_interoperability_gateway",
                "factory_federated_quality_workbench",
                "factory_prospective_evidence_surveillance",
                "fiber_federated_analysis_control_plane",
                "fiber_federated_resource_workbench",
                "governance_experiment_design_assurance",
                "hub_policy_autonomy_inference_engine",
                "hubapi_federated_experiment_design_assurance",
                "ids_adversarial_recovery",
                "ids_bounded_evolution",
                "ids_computational_execution_workbench",
                "ids_context_compilation_federated_control_plane",
                "ids_contract_frontier",
                "ids_dependency_composition",
                "ids_evaluation_assurance",
                "ids_experiment_design_workbench",
                "ids_federated_commons",
                "ids_federated_interpretation_visualization_assurance",
                "ids_federated_resource_discovery_interoperability",
                "ids_federated_workflow_fabric",
                "ids_federation_security_contract",
                "ids_interoperability_extensibility_copilot",
                "ids_interoperability_gateway",
                "ids_knowledge_representation_federated_control_plane",
                "ids_laboratory_integration_workflow_fabric",
                "ids_limitation_closure",
                "ids_local_evidence_surveillance_inference",
                "ids_mechanism_exploration_assurance",
                "ids_multimodal_ingestion_research_copilot",
                "ids_performance_reliability_gateway",
                "ids_policy_autonomy_interoperability_gateway",
                "ids_policy_autonomy_workbench",
                "ids_prospective_provenance_assurance",
                "ids_protocol_simulation_workbench",
                "ids_provenance_signing_assurance",
                "ids_publication_research_object_release_control_plane",
                "ids_quality_control_assurance",
                "ids_reliability_copilot",
                "ids_replication_negative_results_interoperability_gateway",
                "ids_research_workbench",
                "ids_retrieval_synthesis_assurance_harness",
                "ids_scale_frontier",
                "ids_semantic_parity",
                "ids_statistical_causal_ml_research_copilot",
                "ids_typed_determinism_assurance",
                "ids_typed_determinism_interoperability_gateway",
                "influence_local_evidence_surveillance_assurance",
                "interweave_federated_commons_assurance",
                "interweave_federated_interpretation_engine",
                "lab_federated_experiment_design_interoperability_gateway",
                "lab_instrument_interoperability_gateway",
                "lens_provenance_signing_copilot",
                "mcp_federated_quality_control",
                "mcp_replication_negative_results_assurance",
                "mutation_federated_continual_bounded_evolution_assurance",
                "mutation_federated_publication_release",
                "mutation_federated_resource_discovery_control_plane",
                "obligation_knowledge_representation_assurance",
                "obligation_prospective_release_assurance",
                "obligation_security_federation_interoperability_gateway",
                "onco_computational_execution_contract_model",
                "onco_federated_provenance_signing",
                "onco_instrument_research_workbench",
                "oncoworlds_federated_resource_discovery_assurance",
                "oncoworlds_federated_statistical_analysis_workbench",
                "oncoworlds_prospective_evidence_surveillance_copilot",
                "oncoworlds_prospective_replication_negative_results_assurance",
                "oracle_evidence_surveillance_workflow_fabric",
                "oracle_interoperability_research_workbench",
                "oraclex_interpretation_inference",
                "oraclex_performance_reliability_interoperability_gateway",
                "oraclex_statistical_analysis_research_workbench",
                "packs_local_quality_control_assurance",
                "packs_protocol_simulation_workbench",
                "policy_federated_analysis_copilot",
                "prism_analysis_workbench",
                "prism_laboratory_integration_copilot",
                "prism_protocol_simulation_assurance",
                "registry_replication_workbench",
                "retrieval_synthesis_operations",
                "routing_execution_copilot",
                "routing_laboratory_inference_engine",
                "routing_limitation_closure_workflow",
                "runtime_interpretation_assurance",
                "runtime_knowledge_representation_assurance",
                "safety_prospective_laboratory_integration_assurance",
                "scale_federation_trust_control_plane",
                "scale_interpretation_interoperability_gateway",
                "scale_interpretation_visualization_assurance",
                "scale_quality_control_contract_model",
                "scope_federated_commons_interoperability_gateway",
                "scope_federated_evidence_control",
                "services_context_compilation_research_copilot",
                "services_multimodal_interpretation",
                "stress_federated_multimodal_ingestion_contract_model",
                "stress_publication_research_object_workbench",
                "weavelang_federated_commons_assurance",
                "worldfactory_computational_execution_federated_control_plane",
                "worldfactory_protocol_simulation_federated_control_plane",
                "worldgen_multimodal_execution",
                "worldgen_multimodal_ingestion"
            ],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "registry_operations_and_infrastructure",
            "domains": ["registry", "deployment", "storage", "cache", "leases", "observability"],
            "crates": ["bioprism-registry", "bioprism-hubapi", "bioprism-infra", "bioprism-ledger", "bioprism-factory", "bioprism-ops", "bioprism-services"],
            "mcp_tools": ["registry_gate", "registry_lifecycle_simulate", "registry_multimodal_scale_frontier_assurance", "registry_knowledge_representation_assurance", "ops_context_compilation_federated_control_plane", "cache_invalidation_simulate", "storage_lifecycle_simulate", "release_audit", "operations_catalog", "ops_acceptance", "ops_capacity", "quality_gate_run", "ledger_ingest", "factory_lifecycle_simulate", "factory_authority_verify", "artifact_registry_audit", "domain_report_project", "domain_evidence_harmonize", "domain_evidence_harmonization_coverage", "domain_evidence_intake", "domain_evidence_coverage", "domain_evidence_source_plan", "domain_evidence_source_execute", "hub_search", "hub_resolve", "hub_lock", "telemetry_project"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "atlas_metrics_and_research_ci",
            "domains": ["capability metrics", "coverage debt", "failure atlas browsing", "partial rankings", "weight sensitivity", "research CI", "claim publication checks"],
            "crates": ["bioprism-atlas", "bioprism-metrics", "bioprism-atlasx", "bioprism-atlashub"],
            "mcp_tools": ["atlas_report", "atlas_surface_audit", "capability_rank", "metrics_profile_audit", "metrics_analytics_audit", "biocapability_evidence_audit", "research_ci_check"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "release_and_reproduction",
            "domains": ["result bundles", "digest verification", "operational acceptance", "release evidence"],
            "crates": ["bioprism-bundle", "bioprism-ops", "bioprism-registry", "bioprism-safety"],
            "mcp_tools": ["bundle_verify", "release_audit", "ops_acceptance", "safety_release_gate", "registry_gate", "evaluation_reproduction_check"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "public_hub_submission_and_moderation",
            "domains": ["submission contracts", "provenance and licensing", "limitations cards", "append-only moderation", "independent verification"],
            "crates": ["bioprism-hub", "bioprism-hubapi"],
            "mcp_tools": ["hub_submission_review", "hub_disclosure_review", "hub_card_render", "hub_leaderboard_render", "bioatlas_publication_audit", "hub_search", "hub_resolve", "hub_lock"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "observability_and_telemetry_boundaries",
            "domains": ["redaction policy", "semantic-loss reporting", "observed-versus-asserted metrics", "trace correlation", "cardinality-safe operations"],
            "crates": ["bioprism-ops", "bioprism-safety", "bioprism-scope"],
            "mcp_tools": ["telemetry_project", "operations_catalog", "ops_capacity"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "inference_lab",
            "domains": ["hypothesis separation", "evidence acquisition", "holdout-aware improvement", "risk-triggered research"],
            "crates": ["bioprism-lab", "bioprism-obligation", "bioprism-routing", "bioprism-evalengine"],
            "mcp_tools": ["lab_plan", "lab_space_audit", "lab_pareto_audit", "lab_branch_audit", "lab_holdout_audit", "lab_evolution_audit", "design_frontier_evaluate", "routing_decide", "routing_lab_run"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "oracle_mesh",
            "domains": ["tiered evidence", "oracle disagreement", "reference standards", "adjudication inputs"],
            "crates": ["bioprism-oracle", "bioprism-oraclex", "bioprism-evalengine"],
            "mcp_tools": ["oracle_combine", "oracle_reference_panel", "oracle_missingness"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "laboratory_integration",
            "domains": ["instrument preflight", "interlocks", "physical-effect authorization", "protocol evidence"],
            "crates": ["bioprism-lab", "bioprism-runtime", "bioprism-policy"],
            "mcp_tools": ["instrument_preflight", "protocol_matrix_simulate"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "multimodal_ingestion",
            "domains": ["modality harmonization", "unit alignment", "coordinate systems", "semantic loss", "local research objects"],
            "crates": ["bioprism-adapter", "bioprism-modalities", "bioprism-foundation"],
            "mcp_tools": ["multimodal_harmonize", "mcp_multimodal_ingestion_assurance", "multimodal_replication_evaluate", "quality_drift_evaluate"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "runtime_execution_and_replay",
            "domains": ["effect authorization", "sandbox posture", "hash-chained replay", "checkpoint verification"],
            "crates": ["bioprism-runtime", "bioprism-trace", "bioprism-store"],
            "mcp_tools": ["runtime_effect_check", "runtime_tape_verify", "runtime_execution_simulate", "runtime_workflow_execute"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "documentation_and_knowledge",
            "domains": ["repository navigation", "documentation graph", "task routes", "context bundles"],
            "crates": ["bioprism-docgraph", "bioprism-graph", "bioprism-lens"],
            "mcp_tools": ["workspace_capabilities", "capability_audit", "capability_dashboard", "capability_discover", "capability_route", "capability_route_review", "capability_route_plan", "capability_route_plan_verify", "domain_workflow_catalogue", "domain_workflow_scaffold", "domain_workflow_instantiate", "domain_workflow_portfolio", "domain_workflow_portfolio_verify", "domain_workflow_verify", "domain_workflow_reconcile", "domain_workflow_reconciliation_import", "domain_workflow_reconciliation_query", "domain_workflow_reconciliation_get", "repository_catalog", "repository_bundle", "repository_impact", "lens_catalogue", "lens_leakage_check", "projection_bundle"],
            "cli_entrypoints": [],
            "status": "available"
        },
        {
            "id": "developer_and_release_contracts",
            "domains": ["diagnostics", "conformance", "cookbook", "SDK contracts", "signed bundles"],
            "crates": ["bioprism-devx", "bioprism-devplat", "bioprism-conformance", "bioprism-cookbook", "bioprism-sdk", "bioprism-bundle", "bioprism-scale", "bioprism-stewardship"],
            "python_artifacts": ["python/prism_sdk"],
            "mcp_tools": ["governance_schema_check", "developer_platform_status", "engineering_manifest_audit", "engineering_execution_plan", "release_pipeline_audit", "operational_readiness_audit", "security_privacy_audit", "sandbox_admission_audit", "sandbox_runtime_simulate", "security_program_audit", "agent_mission", "autopilot_drive", "autopilot_verify", "autopilot_goal_step", "autopilot_goal_verify", "developer_workbench", "developer_workbench_verify", "developer_workbench_import", "developer_workbench_query", "developer_workbench_get", "ci_provider_normalize", "ci_provider_evidence_audit", "ci_provider_evidence_import", "ci_provider_evidence_query", "ci_provider_evidence_get", "ci_execution_evidence_audit", "execution_provenance_audit", "developer_delivery_audit", "developer_delivery_receipt", "developer_delivery_receipt_verify", "release_audit", "research_release_validate", "research_release_batch_validate", "sdk_registry_check", "conformance_run", "provider_capability_gate", "scale_family_split_verify", "stewardship_review_check"],
            "cli_entrypoints": ["--help", "--json"],
            "status": "available"
        }
    ]);
    // Evidence acquisition is a cross-cutting capability: every declared domain can retain a
    // bounded source plan, execute the in-process file/HTTP subset, and bind the response to
    // intake. Keeping these memberships explicit lets the intake handlers enforce group/domain
    // scope instead of treating the registry-operations group as a hidden universal escape hatch.
    let cross_domain_tools = [
        "domain_evidence_source_plan",
        "domain_evidence_source_execute",
        "domain_evidence_provider_normalize",
        "domain_evidence_provider_replay_verify",
        "domain_evidence_provider_connector_handoff",
        "domain_evidence_provider_external_payload_receipt",
        "domain_evidence_provider_external_payload_replay_verify",
        "domain_evidence_provider_external_payload_normalize",
        "domain_evidence_provider_external_payload_lineage_audit",
        "domain_evidence_provider_external_payload_execution_evidence",
        "domain_evidence_provider_external_payload_evidence_query",
        "adapter_execution_evidence",
        "adapter_execution_evidence_query",
        "domain_evidence_intake",
        "domain_evidence_coverage",
        "domain_decision_readiness_audit",
        "domain_decision_readiness_query",
        "control_plane_readiness_audit",
        "control_plane_readiness_compare",
        "control_plane_readiness_compare_retained",
        "control_plane_readiness_query",
    ];
    if let Some(groups) = catalogue.as_array_mut() {
        for group in groups {
            if let Some(tools) = group.get_mut("mcp_tools").and_then(Value::as_array_mut) {
                for tool in cross_domain_tools {
                    if !tools
                        .iter()
                        .any(|candidate| candidate.as_str() == Some(tool))
                    {
                        tools.push(Value::String(tool.into()));
                    }
                }
            }
            if group.get("id").and_then(Value::as_str) == Some("documentation_and_knowledge") {
                if let Some(tools) = group.get_mut("mcp_tools").and_then(Value::as_array_mut) {
                    if !tools
                        .iter()
                        .any(|candidate| candidate.as_str() == Some("domain_acquisition_catalogue"))
                    {
                        tools.push(Value::String("domain_acquisition_catalogue".into()));
                    }
                }
            }
        }
    }
    catalogue
}
