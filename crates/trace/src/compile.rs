//! Compiling a located decision into a reviewable cell.
//!
//! Critical-path Gate 2 is explicit: **"Humans approve cells; the system does not silently publish
//! model-generated tests."** That is enforced structurally here rather than by convention. A
//! [`CellProposal`] cannot be turned into a `bioprism_prism::DecisionCell` except through
//! [`CellProposal::approve`], which requires a named reviewer — there is no other constructor and
//! no `From` impl.
//!
//! The proposal carries everything a reviewer needs to judge it: the step, why it was chosen, what
//! the run did there, what the passing run did instead, and what evidence the failing run could not
//! see. A reviewer approving without that context is approving a hash.

use crate::divergence::Divergence;
use crate::error::TraceError;
use crate::event::Trace;
use crate::segment::Candidate;
use bioprism_ids::ContentHash;
use bioprism_prism::{DecisionCell, InputRef};
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellProposal {
    pub proposal_id: String,
    pub trace_id: String,
    pub step: usize,
    /// Why this step was proposed, in a reviewer's terms.
    pub rationale: String,
    pub what_the_run_did: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what_a_passing_run_did: Option<String>,
    /// Evidence visible to the passing run and not to this one. Often the whole story.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visibility_gap: Vec<String>,
    pub trace_digest: String,
    pub score: f64,
}

impl CellProposal {
    /// Builds a proposal from a ranked candidate and an optional divergence.
    pub fn from_candidate(
        trace: &Trace,
        candidate: &Candidate,
        divergence: Option<&Divergence>,
    ) -> Result<Self, TraceError> {
        crate::ingest::validate(trace)?;
        if trace.at(candidate.step).is_none() {
            return Err(TraceError::StepNotInTrace {
                step: candidate.step,
            });
        }

        let (passing, gap) = match divergence {
            Some(Divergence::Diverged {
                passing_did,
                visibility_gap,
                ..
            }) => (Some(passing_did.clone()), visibility_gap.clone()),
            _ => (None, Vec::new()),
        };

        let rationale = if candidate.score.is_divergence {
            format!(
                "first divergence from a passing run; {} step(s) of the run follow it",
                candidate.score.downstream_steps
            )
        } else {
            format!(
                "{} recorded alternative(s), {} newly visible item(s), {} step(s) follow",
                candidate.score.alternatives,
                candidate.score.newly_visible,
                candidate.score.downstream_steps
            )
        };

        Ok(CellProposal {
            proposal_id: format!("{}#step{}", trace.trace_id, candidate.step),
            trace_id: trace.trace_id.clone(),
            step: candidate.step,
            rationale,
            what_the_run_did: candidate.summary.clone(),
            what_a_passing_run_did: passing,
            visibility_gap: gap,
            trace_digest: trace.digest().as_str().to_string(),
            score: candidate.score.total,
        })
    }

    /// The only path from a proposal to a cell.
    ///
    /// Requires a named reviewer and the world and query the cell will freeze. Refuses an empty
    /// reviewer: an unattributed approval is indistinguishable from no approval, and Gate 2's
    /// requirement is accountability, not a checkbox.
    pub fn approve(
        self,
        reviewer: &str,
        world: InputRef,
        query: InputRef,
        acceptable: OracleStatus,
    ) -> Result<Approved, TraceError> {
        if reviewer.trim().is_empty() {
            return Err(TraceError::UnattributedApproval);
        }

        let cell = DecisionCell::new(
            format!("dc_{}", self.proposal_id),
            self.rationale.clone(),
            world,
            query,
        )
        .accepting(acceptable);

        let record = json!({
            "proposal": self,
            "reviewer": reviewer,
        });
        let approval_digest = ContentHash::of_value(&record)
            .map_err(|_| TraceError::UnattributedApproval)?
            .as_str()
            .to_string();

        Ok(Approved {
            cell,
            reviewer: reviewer.to_string(),
            proposal: self,
            approval_digest,
        })
    }
}

/// A cell plus the approval that authorised it.
///
/// The two travel together so a published cell can always be traced to who accepted it and from
/// which trajectory — a cell with no provenance is exactly what Gate 2 forbids.
#[derive(Debug, Clone)]
pub struct Approved {
    pub cell: DecisionCell,
    pub reviewer: String,
    pub proposal: CellProposal,
    pub approval_digest: String,
}

impl Approved {
    pub fn provenance(&self) -> serde_json::Value {
        json!({
            "cell_id": self.cell.cell_id,
            "reviewer": self.reviewer,
            "from_trace": self.proposal.trace_id,
            "at_step": self.proposal.step,
            "trace_digest": self.proposal.trace_digest,
            "approval_digest": self.approval_digest,
        })
    }
}
