//! Research-object release program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod archive_migration_adapter;
pub mod artifact_integrity_scanner;
pub mod autonomous_stage_bridge;
pub mod comparative_release_explorer;
pub mod consortium_publication_steward;
pub mod continuous_release_pipeline;
pub mod dependency_closure;
pub mod distributed_archive_mirror;
pub mod federated_release_bundle;
pub mod federated_release_sharing_gate;
pub mod leakage_audit;
pub mod license_scope_checker;
pub mod metadata_normalizer;
pub mod migration;
pub mod multimodal_bundle;
pub mod multistudy_release_composer;
pub mod operating_cycle;
pub mod prospective_replay_fidelity_gate;
pub mod qualification_preserver;
pub mod release_bundle_compiler;
pub mod release_event_protocol;
pub mod release_gate;
pub mod release_preview_workbench;
pub mod release_queue_console;
pub mod release_queue_scheduler;
pub mod release_signature_verifier;
pub mod replay;
pub mod reproducibility_score;
pub mod research_object_conformance_suite;
pub mod research_object_exchange_api;
pub mod signed_attestation;
pub mod version_retention_governor;

pub use archive_migration_adapter::{
    migrate_glioma_archive_object, ArchiveArtifactRef, ArchiveMigrationDisposition,
    ArchiveMigrationError, ArchiveMigrationFieldDecision, ArchiveMigrationFieldStatus,
    ArchiveMigrationReport, ArchiveMigrationRequest, ArchiveMigrationRule,
    ArchiveMigrationTransform, ArchiveObject, ArchiveSchemaProfile, MigratedArchiveObject,
};
pub use artifact_integrity_scanner::{
    scan_glioma_artifact_integrity, ArtifactIntegrityCandidate, ArtifactIntegrityError,
    ArtifactIntegrityReport, ArtifactIntegrityRequest, IntegrityDisposition, IntegrityFinding,
    IntegrityFindingKind,
};
pub use autonomous_stage_bridge::{
    dry_run_glioma_release_stage_worker, GliomaReleaseStageBridgeError,
    GliomaReleaseStageBridgeReceipt, GliomaReleaseStageWorker,
};
pub use comparative_release_explorer::{
    explore_glioma_comparative_release, ComparativeExplorerError,
    ComparativeReleaseExplorerRequest, ComparativeReleaseView, ComparativeViewCell,
};
pub use consortium_publication_steward::{
    steward_glioma_consortium_publication, ConsortiumPublicationDisposition,
    ConsortiumPublicationError, ConsortiumPublicationRequest, ConsortiumPublicationState,
    ConsortiumSiteDecision, SitePublicationDecision, SitePublicationInput,
};
pub use continuous_release_pipeline::{
    compile_glioma_continuous_release, ContinuousReleaseCandidate, ContinuousReleaseDisposition,
    ContinuousReleaseError, ContinuousReleaseRequest, ContinuousReleaseRules, ReleasePipelineEvent,
    ReleasePipelineEventKind, ReleasePipelinePrior,
};
pub use distributed_archive_mirror::{
    evaluate_glioma_distributed_archive_mirror, ArchiveMirrorPolicy, ArchiveReplica,
    DistributedMirrorError, DistributedMirrorRequest, MirrorDisposition, MirrorSourceVersion,
    MirrorStatus, MirrorVersionStatus, ReplicaSetStatus,
};
pub use federated_release_bundle::{
    compile_glioma_federated_release, FederatedReleaseDisposition, FederatedReleaseError,
    FederatedReleasePolicy, FederatedReleaseRequest, FederatedResearchObject,
    FederatedSiteAggregate,
};
pub use federated_release_sharing_gate::{
    evaluate_glioma_federated_release_sharing, FederatedReleaseSharingDecision,
    FederatedReleaseSharingError, FederatedReleaseSharingPolicy, FederatedReleaseSharingRequest,
    ShareFieldRequest, SharingAction, SharingDisposition, SharingFieldDecision,
};
pub use license_scope_checker::{
    evaluate_glioma_release_shareability, FieldClassification, FieldShareabilityDecision,
    LicenseDependency, LicenseScopeError, LicenseScopePolicy, ReleaseShareabilityDecision,
    ReleaseShareabilityRequest, ShareabilityDecision, ShareabilityDisposition, ShareableField,
};
pub use release_bundle_compiler::{
    compile_glioma_reproducibility_bundle, BundleMember, BundleOmission, ReleaseBundleError,
    ReproducibilityBundle, ReproducibilityBundleDisposition, ReproducibilityBundleRequest,
};

pub use replay::{
    execute_glioma_replay_campaign, DryRunReplayCampaignExecutor, ReplayCampaign,
    ReplayCampaignDisposition, ReplayCampaignError, ReplayCampaignExecutor, ReplayCampaignRequest,
    ReplayCampaignRound, ReplayCampaignStopReason, ReplayExecutionFailure, ReplayObservation,
    ReplayObservationStatus, ReplayTask,
};

pub use release_gate::{
    evaluate_glioma_release_gate, ReleaseGateError, ReleaseGateEvaluation, ReleaseGateRequest,
    ReleaseGateStatus, ReleaseReviewAttestation, ReleaseReviewDecision,
};

pub use dependency_closure::{
    analyze_glioma_research_object_dependency_closure, DependencyClosureDisposition,
    DependencyClosureError, DependencyClosureNode, DependencyClosureNodeStatus,
    DependencyClosurePlan, DependencyClosureRequest,
};
pub use migration::{
    plan_glioma_research_object_migration, ResearchObjectMigrationAction,
    ResearchObjectMigrationDecision, ResearchObjectMigrationDisposition,
    ResearchObjectMigrationError, ResearchObjectMigrationPlan, ResearchObjectMigrationRequest,
};
pub use multimodal_bundle::{
    compile_glioma_multimodal_research_object, MultimodalResearchObjectBundle,
    MultimodalResearchObjectDisposition, MultimodalResearchObjectEntry,
    MultimodalResearchObjectError, MultimodalResearchObjectInput, MultimodalResearchObjectRequest,
};

pub use leakage_audit::{
    audit_glioma_release_dependency_leakage, DependencyExportScope, LeakageAuditDisposition,
    LeakageAuditError, LeakageKind, LeakageSeverity, ReleaseDependencyLeakageAudit,
    ReleaseDependencyLeakageRequest, ReleaseDependencyNode, ReleaseLeakFinding,
};
pub use metadata_normalizer::{
    normalize_glioma_release_metadata, ControlledVocabulary, MetadataConflict, MetadataField,
    MetadataMappingRule, MetadataNormalizationError, MetadataSource, MetadataTransform,
    MetadataUnresolved, NormalizationDisposition, NormalizedFieldStatus, NormalizedMetadataField,
    ReleaseMetadataNormalization, ReleaseMetadataNormalizationRequest, ReversibleMetadataChange,
    TargetMetadataField, VocabularyTerm,
};
pub use multistudy_release_composer::{
    compose_glioma_multistudy_release, ComparativeAssayMapping, ComparativeFieldBinding,
    ComparativeReleaseDisposition, ComparativeReleaseError, ComparativeReleaseRequest,
    ComparativeResearchObject, ComparativeStudyField, ComparativeStudyObject, MappingRelation,
    StudyReleaseProvenance,
};
pub use operating_cycle::{
    execute_glioma_engine_release_operating_cycle, execute_glioma_release_operating_cycle,
    execute_glioma_release_operating_cycle_dry_run, GliomaReleaseOperatingCycle,
    GliomaReleaseOperatingCycleDisposition, GliomaReleaseOperatingCycleError,
    GliomaReleaseOperatingCycleRequest, ReleaseExecutionMode,
};
pub use prospective_replay_fidelity_gate::{
    execute_glioma_prospective_replay_fidelity, DryRunReplayFidelityExecutor,
    ReplayFidelityDisposition, ReplayFidelityError, ReplayFidelityExecutionFailure,
    ReplayFidelityExecutor, ReplayFidelityMetric, ReplayFidelityObservation, ReplayFidelityReport,
    ReplayFidelityRequest, ReplayFidelityStatus, ReplayFidelityStopReason, ReplayFidelityTask,
    ReplayFidelityTaskReport,
};
pub use qualification_preserver::{
    audit_glioma_qualification_preservation, QualificationFinding, QualificationFindingKind,
    QualificationFindingSeverity, QualificationKind, QualificationPreservationAudit,
    QualificationPreservationDisposition, QualificationPreservationError,
    QualificationPreservationRequest, QualificationRecord, ReleaseClaimRecord,
};
pub use release_event_protocol::{
    replay_glioma_release_event_protocol, ReleaseEvent, ReleaseEventDisposition, ReleaseEventKind,
    ReleaseEventObservation, ReleaseEventProtocolError, ReleaseEventProtocolReport,
    ReleaseEventProtocolRequest, ReleaseLifecycleState,
};
pub use release_preview_workbench::{
    preview_glioma_release_audience, PreviewComparison, ReleaseAudiencePreview,
    ReleasePreviewError, ReleasePreviewRequest,
};
pub use release_queue_console::{
    snapshot_glioma_release_queue, ReleaseCandidateState, ReleaseQueueCandidate, ReleaseQueueError,
    ReleaseQueueEvent, ReleaseQueueEventKind, ReleaseQueueReorderProposal, ReleaseQueueRequest,
    ReleaseQueueSnapshot, ReleaseReviewerAssignment,
};
pub use release_queue_scheduler::{
    schedule_glioma_release_queue, QueueCapacity, QueueScheduleAction, QueueScheduleDisposition,
    QueueScheduleEntry, QueueSchedulerError, ReleaseQueueItem, ReleaseQueueSchedule,
    ReleaseQueueScheduleRequest,
};
pub use release_signature_verifier::{
    verify_glioma_release_signature, ReleaseSignatureVerificationError,
    ReleaseSignatureVerificationRequest, ReleaseVerificationDisposition, ReleaseVerificationReport,
    VerificationCheck, VerificationCheckStatus, VerifierTrustRoot,
};
pub use reproducibility_score::{
    score_glioma_reproducibility_completeness, ReproducibilityCompletenessDisposition,
    ReproducibilityCompletenessError, ReproducibilityCompletenessProfile,
    ReproducibilityCompletenessRequest, ReproducibilityDimension, ReproducibilityDimensionScore,
    ReproducibilityEvidence, ReproducibilitySensitivity,
};
pub use research_object_conformance_suite::{
    evaluate_glioma_research_object_conformance, ConformanceDisposition, ConformanceError,
    ConformanceFinding, ConformanceProfile, ConformanceReport, ConformanceRequest,
    ConformanceSeverity,
};
pub use research_object_exchange_api::{
    plan_glioma_research_object_exchange, ExchangeApiError, ExchangeChunk, ExchangeDisposition,
    ExchangeManifest, ExchangePolicy, ResearchObjectExchangeRecord, ResearchObjectExchangeRequest,
};
pub use signed_attestation::{
    attest_glioma_release, AttestationStatus, ReleaseSigningAuthority, ReleaseVerificationResult,
    SignedAttestationError, SignedReleaseAttestation, SignedReleaseAttestationRequest,
};
pub use version_retention_governor::{
    plan_glioma_version_retention, ReleaseVersion, RetentionAction, RetentionActionPlan,
    RetentionDecision, RetentionDisposition, RetentionGovernorError, RetentionGovernorRequest,
    RetentionPolicy, VersionStorageHealth,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ResearchObjectRelease;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P11")
}
