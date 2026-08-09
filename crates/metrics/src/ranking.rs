//! Ranking, and the refusal that makes ranking honest.
//!
//! ADR-006 is the constitution for this module: *"PRISM will publish capability profiles,
//! uncertainty, cost curves, safety constraints, mutation robustness, and Pareto comparisons.
//! Pack-specific aggregates are allowed when their construction is transparent; no universal 'agent
//! IQ' will be canonical."* Its consequences are listed in the ADR itself and one of them is
//! "Leaderboards are less superficially simple". This module is where that cost is paid.
//!
//! # Three answers, not two
//!
//! [`compare`] returns [`Dominance`], and the interesting variant is
//! [`Dominance::Incomparable`]. Two systems, one better at verification and one better at safety,
//! are genuinely two answers; a function that picks one has thrown the finding away.
//! `bioprism-lab` made exactly this choice for architecture search and its reasoning transfers
//! unchanged, so the vocabulary is deliberately the same.
//!
//! An unmeasured cell is a second, stronger source of incomparability: a system with a hole can
//! neither dominate nor be dominated, whatever it does on the axes that *were* measured. It stays
//! in the maximal set, and [`PartialRanking::unresolved`] names it and the hole that put it there.
//! Dropping it would report "we compared these" about a comparison nobody made; scoring the hole as
//! zero would report a measurement nobody took.
//!
//! # When a total order is demanded
//!
//! Sometimes one has to be produced, and the honest response is not to refuse forever but to make
//! the arbitrary step visible. [`PartialRanking::totalise`] requires a [`DeclaredWeighting`] —
//! predeclared, digested, citable — and the [`TotalRanking`] it returns records:
//!
//! - the weighting and its content hash, so the construction is transparent in ADR-006's sense;
//! - every pair the partial order called **incomparable** that the weighting has now ordered, in
//!   [`TotalRanking::collapsed`], so a reader can see precisely which findings the total order
//!   overwrote;
//! - each rank's [`CoveredAggregate`] rather than a number, so no row of a leaderboard can show a
//!   score without its coverage.
//!
//! Ties are not broken. Two systems with the same weighted value share a rank, because a tiebreak
//! that nobody declared is the same defect as a weighting nobody declared.
//!
//! # Rank instability and weight sensitivity
//!
//! 33.01 lists "rank instability" and "weight sensitivity" among its primary metrics and defines
//! neither — as it does for every metric it names. [`RankInstability`] therefore states *a*
//! definition rather than claiming *the* definition: the fraction of the *evaluable*
//! leave-one-capability-out perturbations of the declared weighting under which the top-ranked
//! system changes, together with the capabilities responsible. Leave-one-out rather than a random
//! reweighting because there is no RNG in this crate and a sensitivity number a reader cannot
//! regenerate from the declaration is not a sensitivity analysis.
//!
//! The word *evaluable* is load-bearing and [`Instability`] is what enforces it. A perturbation
//! nobody could evaluate is not evidence of stability, and neither is a weighting with nothing to
//! leave out; both once entered the denominator of a fraction they could never enter the numerator
//! of, and both are now states rather than a zero.
//!
//! # Not implemented
//!
//! No statistical dominance. Where cells carry intervals, [`compare`] still compares point values,
//! so two systems differing by less than their uncertainty read as a trade-off rather than a tie —
//! the conservative direction, and the same choice `bioprism-lab` documents. No Elo, no
//! bradley-terry, no aggregation of pairwise wins: those manufacture a total order out of pairwise
//! preferences, which is the universal scalar score with extra steps.

use crate::aggregate::CoveredAggregate;
use crate::comparability::{grids_comparable, ComparabilityPolicy};
use crate::conditions::Direction;
use crate::error::{MetricsError, ScoreIncomparability};
use crate::grid::CapabilityGrid;
use crate::weighting::DeclaredWeighting;
use bioprism_atlas::{CapabilityId, UnmeasuredReason};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Identifies a system under evaluation.
///
/// A newtype rather than a `String` for the reason 03.09 gives about capability identifiers: a
/// system, an architecture, a model and a run are four different things and conflating them is how
/// a comparison ends up spanning two of them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SystemId(String);

impl SystemId {
    pub fn parse(value: impl Into<String>) -> Result<Self, MetricsError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MetricsError::MalformedWeighting {
                detail: "empty system identifier".to_string(),
            });
        }
        Ok(SystemId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SystemId {
    type Error = MetricsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SystemId::parse(value)
    }
}

impl From<SystemId> for String {
    fn from(value: SystemId) -> Self {
        value.0
    }
}

/// One system's grid, named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityVector {
    pub system: SystemId,
    pub grid: CapabilityGrid,
}

impl CapabilityVector {
    pub fn new(system: SystemId, grid: CapabilityGrid) -> Self {
        CapabilityVector { system, grid }
    }
}

/// Why two systems could not be ordered.
///
/// Three genuinely different findings, kept apart because they call for three different responses:
/// run more evaluation, fix the labelling, or accept that there are two answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "because", rename_all = "snake_case")]
pub enum Unorderable {
    /// Each system is better on at least one capability. The real Pareto trade-off, and the only
    /// one of the three that more evaluation will not resolve.
    TradeOff {
        left_better_on: Vec<CapabilityId>,
        right_better_on: Vec<CapabilityId>,
    },
    /// At least one capability is a hole on one side. Nothing about the measured axes rescues this:
    /// the comparison is over a grid one system never filled in.
    UnmeasuredAxis {
        system: SystemId,
        capability: CapabilityId,
        reason: UnmeasuredReason,
    },
    /// The two grids were produced under conditions that make comparison undefined.
    ConditionsDiffer { reason: ScoreIncomparability },
    /// The grids are defined over different capability sets, so one is not a reading of the other.
    DifferentCapabilitySets {
        only_left: Vec<CapabilityId>,
        only_right: Vec<CapabilityId>,
    },
}

/// The four outcomes of comparing two capability vectors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "dominance", rename_all = "snake_case")]
pub enum Dominance {
    /// Left is at least as good everywhere and strictly better somewhere.
    LeftDominates,
    RightDominates,
    /// Identical on every capability. Not "tied on a score" — equal on every axis.
    Equivalent,
    Incomparable {
        because: Unorderable,
    },
}

impl Dominance {
    pub fn is_incomparable(&self) -> bool {
        matches!(self, Dominance::Incomparable { .. })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Dominance::LeftDominates => "left_dominates",
            Dominance::RightDominates => "right_dominates",
            Dominance::Equivalent => "equivalent",
            Dominance::Incomparable { .. } => "incomparable",
        }
    }
}

/// Compares two capability vectors under the strict comparability policy.
pub fn compare(left: &CapabilityVector, right: &CapabilityVector) -> Dominance {
    compare_under(left, right, &ComparabilityPolicy::strict())
}

/// [`compare`] with an explicit waiver policy for the conditions check.
pub fn compare_under(
    left: &CapabilityVector,
    right: &CapabilityVector,
    policy: &ComparabilityPolicy,
) -> Dominance {
    if let Err(reason) = grids_comparable(&left.grid, &right.grid, policy) {
        return Dominance::Incomparable {
            because: Unorderable::ConditionsDiffer { reason },
        };
    }

    let left_ids: BTreeSet<&CapabilityId> = left.grid.capabilities().collect();
    let right_ids: BTreeSet<&CapabilityId> = right.grid.capabilities().collect();
    if left_ids != right_ids {
        return Dominance::Incomparable {
            because: Unorderable::DifferentCapabilitySets {
                only_left: left_ids
                    .difference(&right_ids)
                    .map(|id| (*id).clone())
                    .collect(),
                only_right: right_ids
                    .difference(&left_ids)
                    .map(|id| (*id).clone())
                    .collect(),
            },
        };
    }

    let direction = left.grid.conditions.scoring_rule.direction;
    let mut left_better = Vec::new();
    let mut right_better = Vec::new();

    for capability in left_ids {
        let left_cell = left.grid.cell(capability);
        let right_cell = right.grid.cell(capability);
        for (vector, cell) in [(left, left_cell), (right, right_cell)] {
            if let Some(reason) = cell.and_then(|c| c.unmeasured_reason()) {
                return Dominance::Incomparable {
                    because: Unorderable::UnmeasuredAxis {
                        system: vector.system.clone(),
                        capability: capability.clone(),
                        reason,
                    },
                };
            }
        }
        let (Some(a), Some(b)) = (
            left_cell.and_then(crate::grid::GridCell::value),
            right_cell.and_then(crate::grid::GridCell::value),
        ) else {
            return Dominance::Incomparable {
                because: Unorderable::UnmeasuredAxis {
                    system: left.system.clone(),
                    capability: capability.clone(),
                    reason: UnmeasuredReason::NotAttempted,
                },
            };
        };
        if direction.is_better(a, b) {
            left_better.push(capability.clone());
        } else if direction.is_better(b, a) {
            right_better.push(capability.clone());
        }
    }

    match (left_better.is_empty(), right_better.is_empty()) {
        (true, true) => Dominance::Equivalent,
        (false, true) => Dominance::LeftDominates,
        (true, false) => Dominance::RightDominates,
        (false, false) => Dominance::Incomparable {
            because: Unorderable::TradeOff {
                left_better_on: left_better,
                right_better_on: right_better,
            },
        },
    }
}

/// One ordered pair's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairRelation {
    pub left: SystemId,
    pub right: SystemId,
    pub dominance: Dominance,
}

/// A partial order over capability vectors, with its refusals intact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartialRanking {
    vectors: Vec<CapabilityVector>,
    relations: Vec<PairRelation>,
}

impl PartialRanking {
    /// Builds the partial order over every unordered pair.
    ///
    /// Refuses fewer than two systems and refuses a duplicated system identifier: a ranking that
    /// contains one system twice will report it as equivalent to itself and inflate the maximal
    /// set, which is a silent corruption rather than a visible one.
    pub fn over(vectors: Vec<CapabilityVector>) -> Result<Self, MetricsError> {
        PartialRanking::over_under(vectors, &ComparabilityPolicy::strict())
    }

    /// [`PartialRanking::over`] with an explicit waiver policy.
    pub fn over_under(
        vectors: Vec<CapabilityVector>,
        policy: &ComparabilityPolicy,
    ) -> Result<Self, MetricsError> {
        if vectors.len() < 2 {
            return Err(MetricsError::RankingTooSmall(vectors.len()));
        }
        let mut seen = BTreeSet::new();
        for vector in &vectors {
            if !seen.insert(vector.system.clone()) {
                return Err(MetricsError::DuplicateSystem(vector.system.to_string()));
            }
        }
        let mut relations = Vec::new();
        for (position, left) in vectors.iter().enumerate() {
            for right in vectors.iter().skip(position + 1) {
                relations.push(PairRelation {
                    left: left.system.clone(),
                    right: right.system.clone(),
                    dominance: compare_under(left, right, policy),
                });
            }
        }
        Ok(PartialRanking { vectors, relations })
    }

    pub fn vectors(&self) -> &[CapabilityVector] {
        &self.vectors
    }

    pub fn relations(&self) -> &[PairRelation] {
        &self.relations
    }

    /// Systems no other system dominates. The published answer when no total order is licensed.
    ///
    /// A system with a hole is here by construction — nothing dominates it, because nothing can —
    /// and that is the truthful placement, not a generous one. Read it next to
    /// [`PartialRanking::unresolved`].
    pub fn maximal(&self) -> Vec<SystemId> {
        let mut dominated: BTreeSet<&SystemId> = BTreeSet::new();
        for relation in &self.relations {
            match relation.dominance {
                Dominance::LeftDominates => {
                    dominated.insert(&relation.right);
                }
                Dominance::RightDominates => {
                    dominated.insert(&relation.left);
                }
                _ => {}
            }
        }
        self.vectors
            .iter()
            .map(|vector| &vector.system)
            .filter(|system| !dominated.contains(system))
            .cloned()
            .collect()
    }

    /// Every pair the order refused to resolve, with the reason.
    pub fn unresolved(&self) -> Vec<&PairRelation> {
        self.relations
            .iter()
            .filter(|relation| relation.dominance.is_incomparable())
            .collect()
    }

    /// Whether the partial order happens to be total already, in which case a weighting buys
    /// nothing and demanding one would be theatre.
    pub fn is_total(&self) -> bool {
        self.unresolved().is_empty()
    }

    /// Turns the partial order into a total one under a predeclared weighting, recording what the
    /// weighting overwrote.
    ///
    /// Refuses when any system leaves a weighted capability unmeasured. There is no renormalisation
    /// over the survivors: a composite computed from three of four declared capabilities is a
    /// different composite, and letting it wear the declared name is the failure this crate exists
    /// to prevent.
    pub fn totalise(&self, weighting: &DeclaredWeighting) -> Result<TotalRanking, MetricsError> {
        let mut rows = Vec::new();
        for vector in &self.vectors {
            let aggregate = CoveredAggregate::weighted(&vector.grid, weighting)?;
            rows.push((vector.system.clone(), aggregate));
        }

        let direction = self
            .vectors
            .first()
            .map(|vector| vector.grid.conditions.scoring_rule.direction)
            .unwrap_or(Direction::HigherIsBetter);

        rows.sort_by(|(left_system, left), (right_system, right)| {
            let a = left.value().get();
            let b = right.value().get();
            if direction.is_better(a, b) {
                std::cmp::Ordering::Less
            } else if direction.is_better(b, a) {
                std::cmp::Ordering::Greater
            } else {
                left_system.cmp(right_system)
            }
        });

        let mut order: Vec<RankedSystem> = Vec::new();
        let mut rank = 0usize;
        let mut previous: Option<f64> = None;
        for (system, aggregate) in rows {
            let value = aggregate.value().get();
            let tied = previous
                .is_some_and(|p| !direction.is_better(p, value) && !direction.is_better(value, p));
            if !tied {
                rank += 1;
            }
            previous = Some(value);
            order.push(RankedSystem {
                rank,
                system,
                aggregate,
            });
        }

        let collapsed = self
            .unresolved()
            .into_iter()
            .map(|relation| CollapsedIncomparability {
                left: relation.left.clone(),
                right: relation.right.clone(),
                dominance: relation.dominance.clone(),
            })
            .collect();

        Ok(TotalRanking {
            weighting: weighting.clone(),
            weighting_digest: weighting.digest().as_str().to_string(),
            order,
            collapsed,
        })
    }
}

/// One row of a total ranking.
///
/// Carries the whole [`CoveredAggregate`], not a number: a leaderboard row that could show a score
/// without its coverage would undo the aggregation module in the presentation layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedSystem {
    /// Dense rank, starting at 1. Tied systems share a rank; nothing here invents a tiebreak.
    pub rank: usize,
    pub system: SystemId,
    pub aggregate: CoveredAggregate,
}

/// A pair the partial order refused to resolve and the weighting resolved anyway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollapsedIncomparability {
    pub left: SystemId,
    pub right: SystemId,
    /// The refusal that was overwritten, kept verbatim so the finding survives the ordering.
    pub dominance: Dominance,
}

/// A total order, with the arbitrary step attached to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TotalRanking {
    /// The weighting that produced this order. Present in full, not by reference: an order that
    /// cited a weighting nobody could retrieve would be exactly the opaque construction ADR-006
    /// refuses to make canonical.
    pub weighting: DeclaredWeighting,
    pub weighting_digest: String,
    pub order: Vec<RankedSystem>,
    /// Pairs that were genuinely incomparable before the weighting was applied. Never elided, for
    /// the same reason `bioprism_atlas::CoverageReport::holes` is never elided.
    pub collapsed: Vec<CollapsedIncomparability>,
}

impl TotalRanking {
    pub fn top(&self) -> Option<&RankedSystem> {
        self.order.first()
    }

    /// Systems sharing rank 1. More than one means the weighting did not decide, and no further
    /// tiebreak is applied.
    pub fn leaders(&self) -> Vec<&SystemId> {
        self.order
            .iter()
            .filter(|row| row.rank == 1)
            .map(|row| &row.system)
            .collect()
    }

    /// Whether this order rests on collapsing a genuine trade-off.
    ///
    /// `true` is not a defect; it is the sentence a methods section owes its reader.
    pub fn overwrote_a_refusal(&self) -> bool {
        !self.collapsed.is_empty()
    }

    /// The disclosure a leaderboard must carry.
    pub fn headline(&self) -> String {
        format!(
            "total order over {} systems under declared weighting {} ({}); {} pairs were \
             incomparable before weighting and are ordered here only by that declaration",
            self.order.len(),
            self.weighting.intended_use(),
            self.weighting_digest,
            self.collapsed.len()
        )
    }
}

/// How often the leader moved, and over what.
///
/// Three states rather than an `f64`, because two of them have no fraction and a fraction is the
/// only thing an `f64` can say. The defect this replaced put every perturbation in the denominator
/// and only the evaluable ones in the numerator, so an unevaluable perturbation entered the
/// published number as a stable one — the exact reading [`RankInstability::unevaluable`]'s own doc
/// comment forbids — and a weighting with nothing to leave out reported `0.0`, which reads as a
/// leader that survived everything when nothing was tried.
///
/// An `Option<f64>` would fix the arithmetic and stop there: it collapses "attempted, nothing
/// judged" into "never attempted", which is the distinction `bioprism_atlas::UnmeasuredReason` and
/// [`crate::gate::GateVerdict`] both exist to keep, and it hands a renderer a nullable number to
/// coerce. This is tagged, so the two empty cases are different JSON *shapes* from a fraction and
/// from each other, and it carries the denominator the fraction was taken over. The fraction itself
/// is not a field: it is derived by [`Instability::fraction`] from counts that are on the wire, so
/// unlike [`crate::aggregate::CoveredAggregate`] — which re-derives its coverage on the way in —
/// there is no stored number here that could need revalidating, because there is no way to write
/// one down that disagrees with its own denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "instability", rename_all = "snake_case")]
pub enum Instability {
    /// The leader moved in `top_changed_in` of the `evaluated` perturbations that produced an
    /// order. Unevaluable perturbations are not in `evaluated`.
    Measured {
        top_changed_in: usize,
        evaluated: usize,
    },
    /// Perturbations were attempted and none of them produced an order, so there is no denominator.
    /// Not a fraction of zero: nobody checked.
    NothingEvaluable { attempted: usize },
    /// The weighting names one capability, so leave-one-out has no members. Sensitivity to a weight
    /// that has nothing to trade against is not a question this metric can be asked.
    NotPerturbable { weighted_capabilities: usize },
}

impl Instability {
    /// `top_changed_in / evaluated`, or `None` where there is no denominator.
    ///
    /// The two `None` cases are the ones a bare `f64` reported as `0.0`. A caller that wants a
    /// number for a chart has to decide what to draw for them, which is the point.
    pub fn fraction(&self) -> Option<f64> {
        match self {
            Instability::Measured {
                top_changed_in,
                evaluated,
            } if *evaluated > 0 => Some(*top_changed_in as f64 / *evaluated as f64),
            _ => None,
        }
    }

    /// Whether a fraction exists at all. Read this before believing a zero.
    pub fn is_measured(&self) -> bool {
        self.fraction().is_some()
    }

    /// The sentence a methods section owes its reader.
    pub fn headline(&self) -> String {
        match self {
            Instability::Measured {
                top_changed_in,
                evaluated,
            } => format!(
                "the leader changed under {top_changed_in} of {evaluated} evaluable \
                 leave-one-out perturbation(s)"
            ),
            Instability::NothingEvaluable { attempted } => format!(
                "none of the {attempted} leave-one-out perturbation(s) could be evaluated, so this \
                 ranking carries no evidence either way about its leader"
            ),
            Instability::NotPerturbable {
                weighted_capabilities,
            } => format!(
                "the weighting names {weighted_capabilities} capability, so there is no \
                 leave-one-out perturbation set and no sensitivity to report"
            ),
        }
    }
}

impl fmt::Display for Instability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.headline())
    }
}

/// How much the top of a ranking depends on the exact weights — 33.01's "rank instability" and
/// "weight sensitivity", given a definition because the blueprint gives none.
///
/// The perturbation set is every leave-one-capability-out weighting, which is deterministic,
/// exactly as large as the weighting, and reproducible by any reader holding the declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankInstability {
    pub baseline_top: Vec<SystemId>,
    /// Size of the leave-one-out perturbation set, evaluable or not.
    pub perturbations: usize,
    pub top_changed_in: usize,
    /// The fraction and the denominator it was taken over, or the reason there is neither.
    pub instability: Instability,
    /// The capabilities whose removal changes the leader. The actionable half of the metric.
    pub flipping_capabilities: Vec<CapabilityId>,
    /// Perturbations that could not be evaluated — usually because dropping a capability leaves a
    /// weighting over a capability some system never measured. Reported rather than counted as
    /// stable, because an unevaluable perturbation is not evidence of stability.
    ///
    /// No perturbation currently lands here, for a narrower reason than "totalise only refuses
    /// absent or unmeasured capabilities" — it can also raise
    /// [`MetricsError::IntervalExcludesEstimate`] and [`MetricsError::CoverageAccountingMismatch`].
    /// What holds is that every perturbation is a *subset* of a weighting that already totalised:
    /// [`MetricsError::WeightedCapabilityAbsent`] and [`MetricsError::WeightedCapabilityUnmeasured`]
    /// name a weighted capability, so a subset cannot raise one the whole did not; dropping a
    /// weight moves a cell from `contributed` to `measured_but_excluded` and leaves the coverage
    /// accounting balanced; and a weighted mean cannot escape the weighted mean of intervals that
    /// each contain their own term, up to rounding. The branch is kept because that is a property
    /// of today's refusals rather than of leave-one-out, and the denominator must not silently
    /// reabsorb these the day it stops holding.
    pub unevaluable: Vec<CapabilityId>,
}

impl RankInstability {
    pub fn measure(
        ranking: &PartialRanking,
        weighting: &DeclaredWeighting,
    ) -> Result<Self, MetricsError> {
        let baseline = ranking.totalise(weighting)?;
        let baseline_top: Vec<SystemId> = baseline.leaders().into_iter().cloned().collect();

        let mut perturbations = 0usize;
        let mut top_changed_in = 0usize;
        let mut flipping = Vec::new();
        let mut unevaluable = Vec::new();

        for (dropped, perturbed) in weighting.leave_one_out()? {
            perturbations += 1;
            match ranking.totalise(&perturbed) {
                Ok(order) => {
                    let leaders: Vec<SystemId> = order.leaders().into_iter().cloned().collect();
                    if leaders != baseline_top {
                        top_changed_in += 1;
                        flipping.push(dropped);
                    }
                }
                Err(_) => unevaluable.push(dropped),
            }
        }

        let evaluated = perturbations - unevaluable.len();
        let instability = if perturbations == 0 {
            Instability::NotPerturbable {
                weighted_capabilities: weighting.len(),
            }
        } else if evaluated == 0 {
            Instability::NothingEvaluable {
                attempted: perturbations,
            }
        } else {
            Instability::Measured {
                top_changed_in,
                evaluated,
            }
        };

        Ok(RankInstability {
            instability,
            baseline_top,
            perturbations,
            top_changed_in,
            flipping_capabilities: flipping,
            unevaluable,
        })
    }

    /// Whether the leader survived every perturbation that could be evaluated.
    ///
    /// Deliberately false when any perturbation was unevaluable: "we could not check" is not
    /// "we checked and it held". Deliberately false too when nothing was evaluated at all, for the
    /// same reason [`crate::ranking::PartialRanking`]'s sibling
    /// `bioprism_prism::ForkResult::is_regression_free` refuses an empty panel — a claim of
    /// robustness resting on zero checks is the emptiest form of the same lie.
    pub fn leader_is_robust(&self) -> bool {
        self.instability.is_measured() && self.top_changed_in == 0 && self.unevaluable.is_empty()
    }
}

/// Which systems lead on one capability, and the field the lead was taken over — 33.01's public
/// card asks for a per-capability heatmap.
///
/// The doc this replaced promised "wins and losses" counted over "pairs the order could resolve",
/// and no such tally is computed or ever was: [`breakdown`] reads each system's cell directly and
/// never consults [`PartialRanking::relations`]. It also said a system that never measured the
/// capability "contributes to neither column", which understates what its absence does. Removing
/// it from the comparison is precisely what promotes whoever remains into `best`.
///
/// So the row carries the population it was chosen from, for the reason [`Instability::Measured`]
/// carries `evaluated` and [`crate::aggregate::CoveredAggregate`] carries its
/// [`crate::aggregate::Coverage`]. `best` and `unmeasured_for` alone cannot answer it: a system
/// that measured the capability and lost appears in neither list, so a row showing one leader and
/// one hole is the same shape whether the leader beat three systems or none. A heatmap cell
/// rendered from `best` is a claim about a system's strength, and that claim travels with the
/// number of systems anyone was able to check it against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityBreakdown {
    pub capability: CapabilityId,
    /// The systems tied at the best value among `measured_for`. Empty when nobody measured it,
    /// rather than naming an arbitrary system as the best of nothing.
    pub best: Vec<SystemId>,
    /// Every system whose cell carried a value, `best` included. The field `best` won.
    pub measured_for: Vec<SystemId>,
    /// Systems with no value here. Not scored as zero, and not counted as losses either.
    pub unmeasured_for: Vec<SystemId>,
}

impl CapabilityBreakdown {
    /// Whether `best` names a lead that no other system could be compared against.
    ///
    /// True is not a finding about the system; it is a finding about the evaluation, and a card
    /// that renders `best` without consulting this reports a win nobody contested.
    pub fn lead_is_uncontested(&self) -> bool {
        self.measured_for.len() < 2
    }
}

/// Which systems lead on each capability, over which field, and which never measured it.
///
/// One row per capability appearing in any system's grid, so a capability only one system declared
/// is visible as a row where everyone else is unmeasured rather than absent from the card.
pub fn breakdown(ranking: &PartialRanking) -> Vec<CapabilityBreakdown> {
    let mut capabilities: BTreeSet<&CapabilityId> = BTreeSet::new();
    for vector in ranking.vectors() {
        capabilities.extend(vector.grid.capabilities());
    }
    let direction = ranking
        .vectors()
        .first()
        .map(|vector| vector.grid.conditions.scoring_rule.direction)
        .unwrap_or(Direction::HigherIsBetter);

    let mut out = Vec::new();
    for capability in capabilities {
        let mut measured: BTreeMap<SystemId, f64> = BTreeMap::new();
        let mut leader: Option<f64> = None;
        let mut unmeasured_for = Vec::new();
        for vector in ranking.vectors() {
            match vector
                .grid
                .cell(capability)
                .and_then(crate::grid::GridCell::value)
            {
                Some(value) => {
                    measured.insert(vector.system.clone(), value);
                    if leader.is_none_or(|current| direction.is_better(value, current)) {
                        leader = Some(value);
                    }
                }
                None => unmeasured_for.push(vector.system.clone()),
            }
        }
        let winners = leader
            .map(|target| {
                measured
                    .iter()
                    .filter(|(_, value)| {
                        !direction.is_better(target, **value)
                            && !direction.is_better(**value, target)
                    })
                    .map(|(system, _)| system.clone())
                    .collect()
            })
            .unwrap_or_default();
        out.push(CapabilityBreakdown {
            capability: capability.clone(),
            best: winners,
            measured_for: measured.into_keys().collect(),
            unmeasured_for,
        });
    }
    out
}
