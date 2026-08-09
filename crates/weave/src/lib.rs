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
