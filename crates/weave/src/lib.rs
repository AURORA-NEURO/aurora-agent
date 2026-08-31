//! AURORA Weave: the agent interweave fabric.
//!
//! Implements the microkernel of blueprint 23.49 and the pieces of §23 the MVP cut line calls for:
//! typed communicative acts (23.05), commitment (23.07) and epistemic (23.08) ledgers, recipient
//! Context Capsules (23.09), continuations with fork and stale-state detection (23.10),
//! attenuating authority with transitive revocation (23.13), and affine budgets (23.16).
//!
//! The kernel is a *trusted computing base*, so its size is a design constraint rather than an
//! accident. It enforces the semantics that cannot be delegated to untrusted participants and
//! refuses to do anything else — no planning, no summarising, no deciding what is true. Claims and
//! their challenges both stay in the ledger; adjudication is somebody else's job.
//!
//! Where this connects to the rest of the platform: a Context Capsule is built from a compiled
//! Decision Section, so a participant's projection inherits the compiler's certificate. It learns
//! what the compiler omitted from the world *and* what the projection withheld from it, separately.

pub mod act;
pub mod authority;
pub mod budget;
pub mod capsule;
pub mod continuation;
pub mod kernel;
pub mod ledger;
pub mod resource_control_plane;
pub mod capability_manifest_integrity_support;
pub mod local_capability_manifest_integrity_inference;
pub mod multimodal_capability_manifest_integrity_inference;
pub mod throughput_capability_manifest_integrity_inference;
pub mod federated_continual_capability_manifest_integrity_inference;
pub mod local_capability_manifest_integrity_contract_model;
pub mod multimodal_capability_manifest_integrity_contract_model;
pub mod throughput_capability_manifest_integrity_contract_model;
pub mod federated_continual_capability_manifest_integrity_contract_model;
pub mod local_capability_manifest_integrity_research_copilot;
pub mod multimodal_capability_manifest_integrity_research_copilot;
pub mod throughput_capability_manifest_integrity_research_copilot;
pub mod federated_continual_capability_manifest_integrity_research_copilot;
pub mod local_capability_manifest_integrity_workflow_fabric;
pub mod multimodal_capability_manifest_integrity_workflow_fabric;
pub mod throughput_capability_manifest_integrity_workflow_fabric;
pub mod federated_continual_capability_manifest_integrity_workflow_fabric;
pub use act::{Act, ActKind};
pub use authority::{AuthorityError, AuthorityTable, Capability, Grant};
pub use budget::{Budget, BudgetError, Resource};
pub use capsule::{ContextCapsule, Label, Recipient, WithheldItem};
pub use continuation::{ContinuationHandle, Fidelity, ResumeError};
pub use kernel::{Kernel, KernelError, Participant};
pub use ledger::{
    commitments, epistemic_state, ChainStatus, Commitment, CommitmentState, EpistemicEntry, Ledger,
    LedgerEvent,
};
pub use resource_control_plane::{
    operate_resource_control_plane, ResourceControlDisposition, ResourceControlError,
    ResourceControlPlaneReceipt, ResourceControlPlaneRequest,
    CONTRACT_VERSION as RESOURCE_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as RESOURCE_CONTROL_PLANE_FEATURE_ID,
};
pub use capability_manifest_integrity_support::{
    admit as admit_capability_manifest_integrity,
    manifest as capability_manifest_integrity_manifest,
    CapabilityArtifact4, CapabilityCandidate4, CapabilityManifestCard7,
    CapabilityManifestIntegrityError, CapabilityManifestRequest4,
};
