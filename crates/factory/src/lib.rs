//! Job, worker, lease and recovery lifecycle.
//!
//! Implements blueprint 40.30 and the scheduling half of section 35. Long-running work — builds,
//! ingests, sandboxed runs, evaluations, mutations, indexing — executes under observable leases
//! with idempotency-aware recovery.
//!
//! ## The invariant that shapes everything
//!
//! 40.30's second non-negotiable is easy to read past and expensive to get wrong:
//!
//! > Lease expiry does not imply safe retry for non-idempotent effects.
//!
//! A missed heartbeat is *ambiguous*. The worker may have crashed before starting the work, or it
//! may have completed the external effect and died before committing. Nothing in the queue can tell
//! those apart. So recovery branches on [`Idempotency`]: idempotent work requeues, compensable work
//! waits for its compensation to run, and non-idempotent work is **quarantined for a person** —
//! whose release must be attributed.
//!
//! A reported failure is different, and treated differently: the worker was alive and says the work
//! did not land, so even non-idempotent jobs requeue. Conflating the two cases is what makes queues
//! double-charge.
//!
//! ## Not implemented
//!
//! The in-memory controller is single-process and time is passed in rather than read from a clock
//! so the lifecycle is deterministically testable. [`authority::SharedExecutionAuthority`] wraps
//! it in a bounded, content-addressed JSON envelope with an atomic queue-plus-journal write,
//! hash-chained transitions, startup recovery, and a cooperative local-filesystem lock for
//! cooperating processes. A durable multi-host deployment still needs the append-only event ledger
//! of 40.09 behind a transactional backend, cross-host fencing/consensus, and authenticated worker
//! identity. Tenant fairness, provider effects, and network-partition tolerance remain absent
//! rather than stubbed.

pub mod admission;
pub mod authority;
pub mod error;
pub mod job;
pub mod lease;
pub mod federated_quality_workbench;
pub mod lease_fencing_integrity_support;
pub mod local_lease_fencing_integrity_inference;
pub mod multimodal_lease_fencing_integrity_inference;
pub mod throughput_lease_fencing_integrity_inference;
pub mod federated_continual_lease_fencing_integrity_inference;
pub mod local_lease_fencing_integrity_contract_model;
pub mod multimodal_lease_fencing_integrity_contract_model;
pub mod throughput_lease_fencing_integrity_contract_model;
pub mod federated_continual_lease_fencing_integrity_contract_model;
pub mod local_lease_fencing_integrity_research_copilot;
pub mod multimodal_lease_fencing_integrity_research_copilot;
pub mod throughput_lease_fencing_integrity_research_copilot;
pub mod federated_continual_lease_fencing_integrity_research_copilot;
pub mod local_lease_fencing_integrity_workflow_fabric;
pub mod multimodal_lease_fencing_integrity_workflow_fabric;
pub mod throughput_lease_fencing_integrity_workflow_fabric;
pub mod federated_continual_lease_fencing_integrity_workflow_fabric;
pub mod snapshot;
pub mod store;

pub use admission::QueueAdmissionPolicy;
pub use authority::{
    AuthorityLockInfo, AuthorityLockRelease, AuthorityMutation, ExecutionAuthoritySnapshot,
    ExecutionAuthorityStatus, ExecutionOperation, ExecutionTransition, SharedExecutionAuthority,
    EXECUTION_AUTHORITY_SCHEMA_VERSION, MAX_EXECUTION_AUTHORITY_BYTES,
    MAX_EXECUTION_AUTHORITY_EVENTS,
};
pub use error::FactoryError;
pub use job::{Idempotency, Job, JobState, ResourceClass};
pub use lease::{Lease, WorkerCapability};
pub use federated_quality_workbench::{
    assure_factory_federated_quality_workbench, factory_federated_quality_workbench_manifest,
    FactoryQualityPeer4, FactoryQualityVerdict5, FactoryQualityVerdict5Artifact,
    FactoryQualityWorkbenchError, FactoryQualityWorkbenchRequest, QualityEvidenceState,
    QualityObservation4, CONTENT_TYPE as FEDERATED_QUALITY_WORKBENCH_CONTENT_TYPE,
    CONTRACT_VERSION as FEDERATED_QUALITY_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_QUALITY_WORKBENCH_FEATURE_ID,
    INPUT_SCHEMA as FEDERATED_QUALITY_WORKBENCH_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_QUALITY_WORKBENCH_OUTPUT_SCHEMA,
};
pub use lease_fencing_integrity_support::{
    LeaseFencingArtifact4, LeaseFencingIntegrityCard7, LeaseFencingIntegrityError,
    LeaseFencingIntegrityRequest4, WorkerLease4,
};
pub use snapshot::{
    CompensationRecord, IdempotencyIndexEntry, JobStoreSnapshot, OutputRecord,
    JOB_STORE_SNAPSHOT_SCHEMA_VERSION, MAX_JOB_STORE_SNAPSHOT_BYTES,
    MAX_JOB_STORE_SNAPSHOT_ID_BYTES, MAX_JOB_STORE_SNAPSHOT_JOBS,
    MAX_JOB_STORE_SNAPSHOT_VALUE_BYTES, MAX_JOB_STORE_SNAPSHOT_WORKER_ID_BYTES,
};
pub use store::{JobStore, Recovery};
