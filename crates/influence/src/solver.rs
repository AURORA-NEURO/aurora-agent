//! The fixed-point solver, and the part of 43.11's "refinement scheduler" that is determined.
//!
//! ## The three phases, and why a caller must be able to tell them apart
//!
//! Kleene iteration from `⊥` computes the least fixed point of a monotone transformer when the
//! lattice has no infinite ascending chains. Both interval domains in [`crate::domains`] have them,
//! and the accumulation this crate actually runs — `u ↦ b + C·u` with `‖C‖ < 1` — produces one:
//! the iterates converge to `(I − C)⁻¹b` and reach it after no finite number of joins. A solver
//! that stopped at the tenth join and reported the iterate would be reporting a *pre*-fixpoint,
//! which over-approximates nothing at all, so [`ascend_by_join_only`] returns
//! [`crate::DomainError::AscendingChainDidNotStabilise`] rather than a number.
//!
//! Widening terminates the chain by construction, and the element it terminates on is a
//! post-fixpoint — sound, and strictly weaker than the least one. That weakness is the trade, and
//! it is recorded in [`Convergence`] on the result rather than described in a doc comment, because
//! a caller comparing two bounds needs to know whether the larger one means more influence or a
//! cheaper argument. [`crate::BoundMethod`] carries the same distinction onto a certificate.
//!
//! Narrowing is the descending phase. From a post-fixpoint `y ⊒ lfp`, monotonicity gives
//! `F(y) ⊒ F(lfp) = lfp`, so `y ⊓ F(y) ⊒ lfp` as well: every iterate of the descending sequence is
//! still an over-approximation, and stopping at any of them is sound. That is what makes a *budget*
//! an honest way to terminate a phase that has no termination guarantee of its own.
//!
//! ## What 43.11 under-specifies, and what this schedules instead
//!
//! 43.11 asks for a refinement scheduler and does not say what refinement refines. Two readings are
//! available and only one of them is determined by anything else in the module:
//!
//! - **Refining the iterate.** Widening gives away precision; narrowing takes some of it back. The
//!   scheduler decides how many joins to take before widening (delayed widening keeps precision on
//!   chains that stabilise quickly) and how many descending steps to spend afterwards. This is
//!   [`RefinementSchedule`], and it is implemented.
//! - **Refining the domain.** A result that is still `⊤` after narrowing could be recomputed in a
//!   more precise abstraction — a disjunctive completion, a partition of the state space, a
//!   relational domain over several sites at once. That needs an ordering on *domains*, and 43.11
//!   neither supplies one nor says which of its abstractions refines which. It is not implemented,
//!   it is named in [`crate::NOT_IMPLEMENTED`], and the registry deliberately does not pretend
//!   otherwise: [`crate::registry::DomainRegistry::abstracting`] returns the domains sharing a fact
//!   class as an unordered list, because an ordered one would be an invention presented as spec.

use crate::domain::{AbstractDomain, DomainError};
use serde::{Deserialize, Serialize};

/// How a post-fixpoint was reached, and therefore how much was given away to reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Convergence {
    /// The ascending chain stabilised under join. The result is the least fixed point the domain
    /// can express and no precision was traded for termination.
    Join,
    /// Widening was applied and no descending step improved on it. The result is a post-fixpoint,
    /// weaker than the least one by an amount nothing here measures.
    Widening,
    /// Widening, then a bounded descending iteration that strictly improved the result. Still a
    /// post-fixpoint, still not the least one, and tighter than what widening alone returned.
    WideningThenNarrowing,
}

impl Convergence {
    pub fn as_str(self) -> &'static str {
        match self {
            Convergence::Join => "join",
            Convergence::Widening => "widening",
            Convergence::WideningThenNarrowing => "widening_then_narrowing",
        }
    }

    /// Whether precision was traded for termination anywhere on the way to this result.
    pub fn traded_precision_for_termination(self) -> bool {
        !matches!(self, Convergence::Join)
    }
}

/// The refinement scheduler: how many steps each phase may spend.
///
/// Every field is a budget rather than a tolerance. A tolerance would make the reported bound
/// depend on the magnitudes in the region, and two runs of the same compile would be entitled to
/// disagree; a step count makes the whole schedule deterministic, which is what
/// `AGENTS.md` means by byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefinementSchedule {
    /// Joins to take before widening starts. Delaying widening costs iterations and keeps the exact
    /// least fixed point on any chain that stabilises within the budget.
    pub joins_before_widening: usize,
    /// Widening steps allowed before the operator is declared broken. A correct widening terminates
    /// every ascending chain, so exhausting this budget is a defect in a domain and is reported as
    /// [`crate::DomainError::WideningDidNotStabilise`], not as a bound.
    pub widening_steps: usize,
    /// Descending steps spent recovering precision. Stopping early is sound; stopping at zero
    /// leaves the widened result, which for the accumulation in [`crate::gibbs`] is a bound of one.
    pub narrowing_steps: usize,
}

impl Default for RefinementSchedule {
    /// Four joins, sixteen widenings, sixty-four narrowings.
    ///
    /// Four joins buy nothing on the accumulation of [`crate::gibbs`] — conditional dependence is
    /// symmetric, so `C` is never nilpotent and the chain ascends forever on an acyclic region as
    /// readily as on a cyclic one. They are spent anyway because they are cheap and because the
    /// chains that *do* stabilise, the ones where the perturbation reaches nothing, then stabilise
    /// under join and are reported as having done so. The narrowing budget is where the descending
    /// sequence for a contraction of rate `r` has closed the gap by `r⁶⁴`; at the `r ≈ 0.9` end of
    /// the accepted class that is a factor of about a thousand, and the residual slack is measured
    /// by the suite rather than assumed away.
    fn default() -> Self {
        RefinementSchedule {
            joins_before_widening: 4,
            widening_steps: 16,
            narrowing_steps: 64,
        }
    }
}

/// A post-fixpoint together with the record of how it was obtained.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedPoint<E> {
    pub value: E,
    pub reached_by: Convergence,
    pub joins: usize,
    pub widenings: usize,
    pub narrowings: usize,
}

/// Kleene iteration with no widening at all.
///
/// Returns the least fixed point when the chain stabilises within `max_steps`, and
/// [`DomainError::AscendingChainDidNotStabilise`] when it does not. The error is the point: a
/// solver that returned the last iterate here would return a pre-fixpoint, and a pre-fixpoint of a
/// sound transformer bounds nothing.
pub fn ascend_by_join_only<D, F>(
    domain: &D,
    transfer: F,
    max_steps: usize,
) -> Result<FixedPoint<D::Element>, DomainError>
where
    D: AbstractDomain,
    F: Fn(&D::Element) -> D::Element,
{
    ascend_by_join_only_from(domain, domain.bottom(), transfer, max_steps)
}

/// [`ascend_by_join_only`] started somewhere other than `⊥`.
///
/// A seed is not a convenience. `⊥` means *unreachable*, and the accumulation of [`crate::gibbs`]
/// adds rather than joins, so an unreachable summand makes the whole sum unreachable and the
/// iteration would stabilise instantly at `⊥` with a bound of zero. Seeding at the zero
/// displacement says the right thing — every site is reachable and nothing has accumulated yet —
/// and costs nothing in soundness: a post-fixpoint above a seed that is itself below the least
/// fixed point still dominates the least fixed point.
pub fn ascend_by_join_only_from<D, F>(
    domain: &D,
    seed: D::Element,
    transfer: F,
    max_steps: usize,
) -> Result<FixedPoint<D::Element>, DomainError>
where
    D: AbstractDomain,
    F: Fn(&D::Element) -> D::Element,
{
    let mut current = seed;
    for step in 0..max_steps {
        let proposed = transfer(&current);
        if domain.leq(&proposed, &current) {
            return Ok(FixedPoint {
                value: current,
                reached_by: Convergence::Join,
                joins: step,
                widenings: 0,
                narrowings: 0,
            });
        }
        current = domain.join(&current, &proposed);
    }
    Err(DomainError::AscendingChainDidNotStabilise { steps: max_steps })
}

/// The iterates of the join-only ascent, for a caller that wants to see the chain rather than be
/// told about it.
///
/// Used by the suite to show that a chain is *strictly* ascending at every step, which is what
/// makes "join alone does not converge" a demonstration rather than an assertion about a budget.
pub fn join_iterates<D, F>(
    domain: &D,
    seed: D::Element,
    transfer: F,
    steps: usize,
) -> Vec<D::Element>
where
    D: AbstractDomain,
    F: Fn(&D::Element) -> D::Element,
{
    let mut iterates = Vec::with_capacity(steps + 1);
    let mut current = seed;
    iterates.push(current.clone());
    for _ in 0..steps {
        current = domain.join(&current, &transfer(&current));
        iterates.push(current.clone());
    }
    iterates
}

/// The scheduled solve: join, then widen, then narrow.
///
/// The result is always a post-fixpoint of `transfer` in the sense `transfer(value) ⊑ value`,
/// whichever phase produced it, and [`FixedPoint::reached_by`] says which did.
pub fn solve<D, F>(
    domain: &D,
    transfer: F,
    schedule: RefinementSchedule,
) -> Result<FixedPoint<D::Element>, DomainError>
where
    D: AbstractDomain,
    F: Fn(&D::Element) -> D::Element,
{
    solve_from(domain, domain.bottom(), transfer, schedule)
}

/// [`solve`] started somewhere other than `⊥`; see [`ascend_by_join_only_from`] for why that is
/// needed and why it is sound.
pub fn solve_from<D, F>(
    domain: &D,
    seed: D::Element,
    transfer: F,
    schedule: RefinementSchedule,
) -> Result<FixedPoint<D::Element>, DomainError>
where
    D: AbstractDomain,
    F: Fn(&D::Element) -> D::Element,
{
    let mut current = seed;
    let mut joins = 0usize;
    for _ in 0..schedule.joins_before_widening {
        let proposed = transfer(&current);
        if domain.leq(&proposed, &current) {
            return Ok(FixedPoint {
                value: current,
                reached_by: Convergence::Join,
                joins,
                widenings: 0,
                narrowings: 0,
            });
        }
        current = domain.join(&current, &proposed);
        joins += 1;
    }

    let mut widenings = 0usize;
    let mut stabilised = false;
    for _ in 0..schedule.widening_steps {
        let proposed = transfer(&current);
        if domain.leq(&proposed, &current) {
            stabilised = true;
            break;
        }
        current = domain.widen(&current, &domain.join(&current, &proposed));
        widenings += 1;
    }
    if !stabilised {
        return Err(DomainError::WideningDidNotStabilise {
            id: domain.id(),
            steps: schedule.widening_steps,
        });
    }

    let widened = current.clone();
    let mut narrowings = 0usize;
    for _ in 0..schedule.narrowing_steps {
        let descended = domain.meet(&current, &transfer(&current));
        if descended == current {
            break;
        }
        current = descended;
        narrowings += 1;
    }

    let reached_by = if narrowings > 0 && current != widened {
        Convergence::WideningThenNarrowing
    } else {
        Convergence::Widening
    };
    Ok(FixedPoint {
        value: current,
        reached_by,
        joins,
        widenings,
        narrowings,
    })
}
