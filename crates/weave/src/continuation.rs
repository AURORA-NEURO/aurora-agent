//! Continuations.
//!
//! Blueprint 23.10: a participant may yield a forkable continuation that another resumes from the
//! same world state. The conformance requirement in 23.49 is "continuation integrity and stale-state
//! detection", and that second half is what makes the first meaningful.
//!
//! A handle binds the ledger head it was taken at. If the world moved on, resuming would silently
//! rebase work onto a state its author never saw, so a stale handle is refused rather than
//! rebased. Forking is the supported way to proceed from a superseded point.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    /// Resumable with byte-identical state.
    Exact,
    /// Resumable, but some non-semantic detail was dropped.
    Lossy,
    /// Inspectable only.
    Frozen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContinuationHandle {
    pub id: String,
    pub owner: String,
    /// Ledger head at the moment the handle was taken.
    pub taken_at_head: String,
    pub fidelity: Fidelity,
    /// Commitments still open when the handle was taken; a resumer inherits them.
    pub open_obligations: Vec<String>,
    pub state: Value,
    pub handle_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResumeError {
    #[error("handle {id} was taken at head {taken_at_head} but the ledger head is now {current}")]
    Stale {
        id: String,
        taken_at_head: String,
        current: String,
    },

    #[error("handle {0} does not verify; its contents have been altered")]
    Tampered(String),

    #[error("handle {id} has fidelity {fidelity:?} and cannot be resumed")]
    NotResumable { id: String, fidelity: Fidelity },
}

impl ContinuationHandle {
    pub fn take(
        id: impl Into<String>,
        owner: impl Into<String>,
        head: impl Into<String>,
        fidelity: Fidelity,
        open_obligations: Vec<String>,
        state: Value,
    ) -> Self {
        let id = id.into();
        let owner = owner.into();
        let taken_at_head = head.into();

        let body = json!({
            "id": id,
            "owner": owner,
            "taken_at_head": taken_at_head,
            "fidelity": fidelity,
            "open_obligations": open_obligations,
            "state": state,
        });
        let handle_sha256 = ContentHash::of_value(&body)
            .expect("state is finite JSON")
            .as_str()
            .to_string();

        ContinuationHandle {
            id,
            owner,
            taken_at_head,
            fidelity,
            open_obligations,
            state,
            handle_sha256,
        }
    }

    fn recomputed_digest(&self) -> String {
        let body = json!({
            "id": self.id,
            "owner": self.owner,
            "taken_at_head": self.taken_at_head,
            "fidelity": self.fidelity,
            "open_obligations": self.open_obligations,
            "state": self.state,
        });
        ContentHash::of_value(&body)
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default()
    }

    pub fn is_intact(&self) -> bool {
        self.recomputed_digest() == self.handle_sha256
    }

    /// Resumes at `current_head`, refusing if the handle is stale, altered or frozen.
    pub fn resume(&self, current_head: &str) -> Result<&Value, ResumeError> {
        if !self.is_intact() {
            return Err(ResumeError::Tampered(self.id.clone()));
        }
        if self.fidelity == Fidelity::Frozen {
            return Err(ResumeError::NotResumable {
                id: self.id.clone(),
                fidelity: self.fidelity,
            });
        }
        if self.taken_at_head != current_head {
            return Err(ResumeError::Stale {
                id: self.id.clone(),
                taken_at_head: self.taken_at_head.clone(),
                current: current_head.to_string(),
            });
        }
        Ok(&self.state)
    }

    /// Branches from a superseded handle.
    ///
    /// Always permitted, because a fork makes no claim to be a continuation of the current line —
    /// it declares a divergence, which is exactly what a braided worldline records (23.43).
    pub fn fork(&self, id: impl Into<String>, owner: impl Into<String>) -> ContinuationHandle {
        ContinuationHandle::take(
            id,
            owner,
            self.taken_at_head.clone(),
            self.fidelity,
            self.open_obligations.clone(),
            self.state.clone(),
        )
    }
}
