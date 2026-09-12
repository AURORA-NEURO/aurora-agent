//! Bounded, checkpointed composition for caller-owned research campaigns.
//!
//! This is a repository-defined composition contract, not a claim of implementing a BioPRISM
//! blueprint module. It coordinates already-validated receipts from the research, brain,
//! autopilot, and optionally neurosurgery crates without becoming a second verifier for them.
//!
//! # What is not implemented
//!
//! * No model, provider, network, scheduler, clock, credential store, tool dispatcher, execution
//!   journal, or durable checkpoint coordinator implementation exists; callers provide the shared
//!   authorities.
//! * No campaign action is blindly retried or semantically replanned. Positive journal absence
//!   evidence may requeue the same fixed-DAG stage under a newly charged authorization.
//! * Checkpoints are content-addressed, not signed, and contain metadata rather than raw artifacts.
//! * Journal reconciliation proves execution disposition, not scientific truth or clinical
//!   acceptance.
//! * Neurosurgical receipts require the non-default `neurosurgery-adapter` feature and always stop
//!   at a human-review boundary; they can never complete a workflow.
//!
//! # Durable authorization boundary
//!
//! Every action authorization is linear and is released only after a caller-owned
//! [`CampaignCheckpointCoordinator`] atomically stores the exact in-flight checkpoint. The first
//! action uses create-if-absent (`None -> generation 1`), so independently started workers cannot
//! both dispatch when they share a correct coordinator. A lost storage acknowledgement returns no
//! token; restoring any checkpoint that was actually written enters reconciliation instead of
//! redispatching. The trait is a contract, not a bundled database: production callers must provide
//! one shared durable transaction over checkpoint payload and trusted head.
//!
//! Native artifact integrity is also kept distinct from execution proof. Synthetic research is
//! accepted only after exact deterministic replay. A raw self-digested autopilot report can stop a
//! campaign as exhausted or refused, but only a rehydrated grant/history that reaches the native
//! terminal planner can produce a successful autopilot receipt.

mod adapters;
mod checkpoint;
mod coordination;
mod error;
mod kernel;
mod model;
mod reconciliation;

pub use adapters::VerifiedCampaignReceipt;
pub use checkpoint::{
    restore_campaign, seal_campaign_checkpoint, validate_campaign_checkpoint,
    ValidatedCampaignCheckpoint, MAX_CAMPAIGN_CHECKPOINT_BYTES,
    RESEARCH_CAMPAIGN_CHECKPOINT_RETENTION, RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA,
};
pub use coordination::{
    CampaignAuthorizationClaim, CampaignCheckpointCoordinator, CampaignCheckpointHead,
};
pub use error::CampaignError;
pub use kernel::{start_campaign, CampaignActionAuthorization, ResearchCampaign};
pub use model::{
    CampaignActionKind, CampaignAdapterAvailability, CampaignReceiptDisposition,
    CampaignReconciliationAuthorityDocument, CampaignStageDocument, CampaignStageSpec,
    CampaignStatus, ResearchCampaignSpec, ResearchCampaignSpecDocument, MAX_CAMPAIGN_ACTIONS,
    MAX_CAMPAIGN_EVENTS, MAX_CAMPAIGN_ID_BYTES, MAX_CAMPAIGN_STAGES, MAX_OBJECTIVE_BYTES,
    MAX_RECONCILIATION_AUTHORITY_ID_BYTES, MAX_RECONCILIATION_AUTHORITY_VERSION_BYTES,
    MAX_STAGE_DEPENDENCIES, MAX_STAGE_ID_BYTES,
};
pub use reconciliation::{
    seal_campaign_reconciliation_receipt, verify_campaign_reconciliation, CampaignExecutionJournal,
    CampaignReconciliationDecisionDocument, CampaignReconciliationQuery,
    CampaignReconciliationReceiptDocument, CampaignReconciliationResult,
    ValidatedCampaignReconciliationReceipt, MAX_CAMPAIGN_RECONCILIATION_RECEIPT_BYTES,
    RESEARCH_CAMPAIGN_RECONCILIATION_RETENTION, RESEARCH_CAMPAIGN_RECONCILIATION_SCHEMA,
};
