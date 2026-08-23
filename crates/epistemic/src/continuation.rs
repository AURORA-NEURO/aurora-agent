//! Rebasing a continuation, in the calculus's own terms: blueprint 43.30.
//!
//! ## This is not a second capsule
//!
//! `bioprism-weave` owns the continuation. `ContinuationHandle`, `Fidelity`, `ResumeError`, the
//! `ContextCapsule` a recipient actually receives, authority with transitive revocation, and the
//! affine `Budget` that does not implement `Clone` so that copying one is a compile error — all of
//! it is already built, in a crate whose size is a deliberate design constraint. Building a second
//! one here would be a second trusted computing base to keep in parity, and this workspace has
//! paid for cross-implementation parity once already.
//!
//! What 43.30 asks for that weave does not supply is the *verdict*. Its runtime contract's last
//! row is "Rebase result: equivalent, refined, obstructed, stale, or invalid", and deciding
//! between the first three is a decision-theoretic question, not a kernel one. Two of them are
//! indistinguishable without a loss:
//!
//! - **Equivalent** — the new world does not change what to do, within tolerance. The continuation
//!   resumes as it stood.
//! - **Refined** — the same action is still chosen but the retained context no longer keeps regret
//!   inside tolerance. Resumption is possible and a refinement move is owed.
//! - **Obstructed** — the new world changes the action. Resuming would execute a decision the
//!   checkpoint's evidence does not support.
//!
//! A version-key comparison cannot tell those apart; it collapses all three into "the world
//! changed". So this module takes the loss from the caller — same absence as everywhere else in
//! this crate, under the same explicit-contract rule — and classifies. The other two verdicts, [`Rebase::Stale`] and
//! [`Rebase::Invalid`], are decided on keys alone and are checked first, because 43.30's invariant
//! is that "stale continuations never silently execute" and a stale checkpoint must be rejected
//! before anyone computes a regret from it.
//!
//! ## Conservation
//!
//! 43.30 requires that "authority cannot expand during transfer" and "budget consumption is
//! conserved". Weave enforces both at the point of transfer, structurally. [`conservation`] states
//! the same two properties as arithmetic over a whole fork tree, which is what an auditor
//! replaying a completed run has to check and what a kernel enforcing one transfer at a time
//! cannot see. It is a different quantifier over the same rule, not a reimplementation of it.

use crate::decision::{Belief, DecisionProblem};
use crate::error::EpistemicError;
use crate::evidence::EvidencePool;
use crate::ratedistortion::{evaluate_context, DistortionCriterion};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The sealed state a continuation resumes from.
///
/// Deliberately not a capsule: there is no payload, no recipient projection and no transport. It
/// is the set of keys and commitments a rebase has to reason about, which is the subset weave's
/// handle exposes to a decider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub query_id: String,
    /// Content address of the Decision Section that was sealed.
    pub section_digest: String,
    /// Monotone version of the world cut. A resume against a lower number is a resume into the past.
    pub world_cut: u64,
    /// Semantic version of the wire schema. A change here invalidates rather than staling.
    pub schema_version: String,
    /// The action the agent was about to take.
    pub action: usize,
    /// The regret the query declared it would accept.
    pub tolerance: f64,
    /// Evidence retained in the sealed context, by pool index.
    pub retained: Vec<usize>,
    /// Capabilities held. Never grows across a transfer.
    pub authority: BTreeSet<String>,
    /// Budget not yet consumed.
    pub budget_remaining: f64,
}

/// The five outcomes 43.30's runtime contract enumerates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "rebase")]
pub enum Rebase {
    /// The world moved and the decision did not. Resume as sealed.
    Equivalent { action: usize, distortion: f64 },
    /// Same action, but the retained context no longer holds regret inside tolerance. Resumable,
    /// with a refinement owed.
    Refined {
        action: usize,
        distortion: f64,
        tolerance: f64,
    },
    /// The new world changes the action. Resuming would execute an unsupported decision.
    Obstructed {
        sealed_action: usize,
        current_action: usize,
        distortion: f64,
    },
    /// The target world cut is older than the checkpoint's.
    Stale { sealed_cut: u64, target_cut: u64 },
    /// A version key does not match. No decision-theoretic question is even well posed.
    Invalid { detail: String },
}

impl Rebase {
    /// Whether the continuation may resume at all.
    ///
    /// `Refined` is resumable — it owes a move, it is not void. `Obstructed`, `Stale` and
    /// `Invalid` are not.
    pub fn resumable(&self) -> bool {
        matches!(self, Rebase::Equivalent { .. } | Rebase::Refined { .. })
    }

    /// Whether the sealed action is still the right one.
    pub fn preserves_the_decision(&self) -> bool {
        matches!(self, Rebase::Equivalent { .. } | Rebase::Refined { .. })
    }
}

/// Classifies a rebase of `checkpoint` against a world at `target_cut`.
///
/// `pool` is the evidence as it stands *now*; `checkpoint.retained` indexes into it. An index
/// outside the current pool means the evidence the checkpoint relied on is gone, which is
/// [`Rebase::Invalid`] rather than a smaller context — a continuation that quietly resumed on less
/// evidence than it sealed would be exactly the silent execution 43.30 forbids.
#[allow(clippy::too_many_arguments)]
pub fn rebase(
    checkpoint: &Checkpoint,
    target_cut: u64,
    target_schema: &str,
    problem: &DecisionProblem,
    prior: &Belief,
    pool: &EvidencePool,
    criterion: DistortionCriterion,
    compatibility_floor: f64,
) -> Result<Rebase, EpistemicError> {
    if checkpoint.schema_version != target_schema {
        return Ok(Rebase::Invalid {
            detail: format!(
                "checkpoint sealed against {} and the target world declares {}",
                checkpoint.schema_version, target_schema
            ),
        });
    }
    if target_cut < checkpoint.world_cut {
        return Ok(Rebase::Stale {
            sealed_cut: checkpoint.world_cut,
            target_cut,
        });
    }
    if checkpoint.action >= problem.action_count() {
        return Ok(Rebase::Invalid {
            detail: format!(
                "checkpoint sealed action {} and the current problem has {} actions",
                checkpoint.action,
                problem.action_count()
            ),
        });
    }
    for index in &checkpoint.retained {
        if *index >= pool.len() {
            return Ok(Rebase::Invalid {
                detail: format!(
                    "checkpoint retained evidence index {index} and the current pool holds {}",
                    pool.len()
                ),
            });
        }
    }

    let retained: BTreeSet<usize> = checkpoint.retained.iter().copied().collect();
    let evaluation = evaluate_context(
        problem,
        prior,
        pool,
        &retained,
        criterion,
        compatibility_floor,
    )?;

    if evaluation.action != checkpoint.action {
        return Ok(Rebase::Obstructed {
            sealed_action: checkpoint.action,
            current_action: evaluation.action,
            distortion: evaluation.distortion,
        });
    }
    if evaluation.distortion <= checkpoint.tolerance + crate::decision::LOSS_EPSILON {
        Ok(Rebase::Equivalent {
            action: evaluation.action,
            distortion: evaluation.distortion,
        })
    } else {
        Ok(Rebase::Refined {
            action: evaluation.action,
            distortion: evaluation.distortion,
            tolerance: checkpoint.tolerance,
        })
    }
}

/// A conservation rule broken across a fork.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "breach")]
pub enum ConservationBreach {
    /// A child holds a capability its parent did not. 43.30: authority cannot expand during transfer.
    AuthorityExpanded { child: String, capability: String },
    /// The children's budgets sum above the parent's. 43.30: budget consumption is conserved.
    BudgetOverdrawn { children: f64, parent: f64 },
    /// A child sealed against an earlier world cut than its parent.
    CutWentBackwards {
        child: String,
        child_cut: u64,
        parent_cut: u64,
    },
}

/// Checks authority and budget conservation across a fork.
///
/// Empty is the healthy state. Every breach names the child and the quantity, because "budget
/// violated" without the two numbers cannot be adjudicated.
pub fn conservation(parent: &Checkpoint, children: &[Checkpoint]) -> Vec<ConservationBreach> {
    let mut breaches = Vec::new();
    for child in children {
        for capability in &child.authority {
            if !parent.authority.contains(capability) {
                breaches.push(ConservationBreach::AuthorityExpanded {
                    child: child.query_id.clone(),
                    capability: capability.clone(),
                });
            }
        }
        if child.world_cut < parent.world_cut {
            breaches.push(ConservationBreach::CutWentBackwards {
                child: child.query_id.clone(),
                child_cut: child.world_cut,
                parent_cut: parent.world_cut,
            });
        }
    }
    let total: f64 = children.iter().map(|c| c.budget_remaining).sum();
    if total > parent.budget_remaining + crate::decision::LOSS_EPSILON {
        breaches.push(ConservationBreach::BudgetOverdrawn {
            children: total,
            parent: parent.budget_remaining,
        });
    }
    breaches
}
