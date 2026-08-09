//! Runtime preconditions for every guarantee this crate quotes: blueprint 43.47.
//!
//! ## Why this module exists rather than a doc comment
//!
//! 43.47 is mostly prose — five theorem candidates, their assumptions, and their
//! counterconditions — and this crate proves none of them. But its runtime contract asks for two
//! things that are not prose: a "machine-checkable precondition indicating whether a theorem
//! applies to a compiled plan", and a "counterexample corpus" of "worlds violating one assumption
//! at a time". Its first non-negotiable invariant is the operative one:
//!
//! > No theorem is cited when runtime preconditions are false.
//!
//! That is a rule about code, and it needs a place to live. Every guarantee in this crate is
//! quoted through the gate functions below, each of which takes the *evidence* that a precondition was
//! returns [`Applicability::NotChecked`] when there is none. There is no path from a caller to a
//! `1 − 1/e` in a report without a [`crate::submodularity::SubmodularityReport`] having been run.
//!
//! ## Which of 43.47's candidates are represented
//!
//! - **Theorem candidate C, exact separator composition.** Represented as
//!   [`Guarantee::SeparatorCompositionExact`], gated on the junction-structure check in
//!   [`crate::separator`]. This is the one 43.47 candidate this crate can actually check the
//!   premises of, and it is checked against a centralised brute-force answer.
//! - **The adaptive-selection clause of 43.44's theorem list**, "adaptive selection guarantees
//!   under validated submodularity" — [`Guarantee::GreedyCardinality`], gated on the exhaustive
//!   check.
//! - **Decision sufficiency**, 43.50's `ε` condition — [`Guarantee::DecisionSufficiency`], gated
//!   on identification status.
//! - **Lens laws**, 43.49 — [`Guarantee::LensLawful`], gated on the property check.
//!
//! Candidates A (protected-closure conservation), B (backward-slice sufficiency) and E (quotient
//! regret) are about compiler passes that live in `bioprism-fiber`, not here. They are named in
//! [`crate::NOT_IMPLEMENTED`] and have no variant, because a variant nothing can satisfy is a
//! coverage number wearing a type.
//!
//! ## The counterexample corpus
//!
//! [`COUNTEREXAMPLES`] is 43.47's "worlds violating one assumption at a time", as a table of
//! constructions the suite builds and asserts the failure of. It is the regression half of the
//! contract: 43.47 requires counterexamples be "retained as regression tests", and a corpus that
//! is only described is not retained.

use crate::submodularity::SubmodularityReport;
use serde::{Deserialize, Serialize};

/// `1 − 1/e`, the greedy approximation factor under monotone submodularity and a cardinality
/// constraint.
///
/// Written out rather than computed so the constant in a report is byte-stable across platforms
/// with different `exp` implementations. Certificates in this workspace hash their bytes.
pub const ONE_MINUS_ONE_OVER_E: f64 = 0.632_120_558_828_557_7;

/// A named approximation or exactness claim this crate is capable of making.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guarantee {
    /// Greedy attains at least `1 − 1/e` of the optimum, under a cardinality constraint on a
    /// normalised monotone submodular objective.
    GreedyCardinality,
    /// The cost-benefit knapsack variant. **No factor is implemented for it**: the `(1−1/e)/2`
    /// result needs the partial-enumeration variant, which this crate does not have.
    GreedyKnapsackCostBenefit,
    /// Separator-message composition recovers the centralised answer exactly. 43.47 candidate C.
    SeparatorCompositionExact,
    /// A context's decision regret is within the query's tolerance. 43.50's `ε` condition.
    DecisionSufficiency,
    /// An optic satisfies the get–put, put–get and put–put laws. 43.49.
    LensLawful,
}

impl Guarantee {
    pub fn as_str(self) -> &'static str {
        match self {
            Guarantee::GreedyCardinality => "greedy_cardinality",
            Guarantee::GreedyKnapsackCostBenefit => "greedy_knapsack_cost_benefit",
            Guarantee::SeparatorCompositionExact => "separator_composition_exact",
            Guarantee::DecisionSufficiency => "decision_sufficiency",
            Guarantee::LensLawful => "lens_lawful",
        }
    }

    /// The blueprint module that states the guarantee.
    pub fn blueprint_module(self) -> &'static str {
        match self {
            Guarantee::GreedyCardinality | Guarantee::GreedyKnapsackCostBenefit => "43.14",
            Guarantee::SeparatorCompositionExact => "43.29",
            Guarantee::DecisionSufficiency => "43.50",
            Guarantee::LensLawful => "43.49",
        }
    }

    /// What must be true, in one sentence a reader can evaluate.
    pub fn precondition(self) -> &'static str {
        match self {
            Guarantee::GreedyCardinality => {
                "the objective is normalised, monotone and submodular, and the constraint is a \
                 cardinality bound"
            }
            Guarantee::GreedyKnapsackCostBenefit => {
                "no factor is implemented for the knapsack case; the (1-1/e)/2 result requires \
                 the partial-enumeration variant"
            }
            Guarantee::SeparatorCompositionExact => {
                "the agent graph is acyclic and satisfies the running-intersection property, and \
                 the algebra is a commutative semiring"
            }
            Guarantee::DecisionSufficiency => {
                "the compatible models agree on the action, or the minimax regret over them is \
                 within the stated tolerance"
            }
            Guarantee::LensLawful => {
                "get-put, put-get and put-put all held on the sample corpus, and the optic has a \
                 total put"
            }
        }
    }
}

/// Whether a guarantee may be quoted, and why not when it may not.
///
/// The three-way split is the same one `bioprism-influence` draws between a bound, no bound, and
/// nobody having looked. [`Applicability::NotChecked`] exists so that "the precondition failed"
/// and "the precondition was never evaluated" cannot share a representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "applicability")]
pub enum Applicability {
    /// The precondition held. `factor` is the approximation factor, absent when the guarantee is
    /// an exactness claim rather than a ratio.
    Applies {
        guarantee: Guarantee,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        factor: Option<f64>,
        /// What was actually checked, for a reader who wants to re-run it.
        evidence: String,
    },
    /// The precondition was evaluated and failed.
    DoesNotApply {
        guarantee: Guarantee,
        failed_precondition: String,
    },
    /// Nobody evaluated the precondition. Never counts toward a claim.
    NotChecked {
        guarantee: Guarantee,
        missing_check: String,
    },
}

impl Applicability {
    /// The factor to quote, or `None`. Returns `None` for both failure states, so a caller cannot
    /// distinguish them by the number — they have to read the variant.
    pub fn factor(&self) -> Option<f64> {
        match self {
            Applicability::Applies { factor, .. } => *factor,
            _ => None,
        }
    }

    pub fn holds(&self) -> bool {
        matches!(self, Applicability::Applies { .. })
    }

    pub fn guarantee(&self) -> Guarantee {
        match self {
            Applicability::Applies { guarantee, .. }
            | Applicability::DoesNotApply { guarantee, .. }
            | Applicability::NotChecked { guarantee, .. } => *guarantee,
        }
    }
}

/// Decides whether the greedy cardinality factor may be quoted.
///
/// `report` is `None` when no exhaustive check was run. That is the [`Applicability::NotChecked`]
/// case and it is deliberately reachable: a caller may legitimately want a greedy selection over a
/// ground set too large to check, and the correct answer is a selection with no factor attached.
pub fn greedy_cardinality(
    report: Option<&SubmodularityReport>,
    cardinality_constrained: bool,
) -> Applicability {
    let guarantee = Guarantee::GreedyCardinality;
    if !cardinality_constrained {
        return Applicability::DoesNotApply {
            guarantee,
            failed_precondition:
                "the constraint is not a cardinality bound; see Guarantee::GreedyKnapsackCostBenefit"
                    .to_string(),
        };
    }
    match report {
        None => Applicability::NotChecked {
            guarantee,
            missing_check:
                "no submodularity report was supplied; the ground set may be above the exhaustive \
                 cap, or the caller did not run the check"
                    .to_string(),
        },
        Some(report) if report.monotone_submodular() => Applicability::Applies {
            guarantee,
            factor: Some(ONE_MINUS_ONE_OVER_E),
            evidence: format!(
                "{}: exhaustive check over a ground set of {} at tolerance {} found no violation \
                 in {} global and {} local triples",
                report.function,
                report.ground,
                report.tolerance,
                report.global_triples,
                report.local_triples
            ),
        },
        Some(report) => Applicability::DoesNotApply {
            guarantee,
            failed_precondition: report
                .why_no_guarantee()
                .unwrap_or_else(|| "unspecified".to_string()),
        },
    }
}

/// Decides the knapsack case. Always [`Applicability::DoesNotApply`], with the reason.
///
/// A function that always returns the same answer looks like dead code, and it is not: it is the
/// difference between a selection that carries no guarantee because none was implemented and one
/// that carries no guarantee because the check failed. Callers get the reason either way.
pub fn greedy_knapsack() -> Applicability {
    Applicability::DoesNotApply {
        guarantee: Guarantee::GreedyKnapsackCostBenefit,
        failed_precondition: Guarantee::GreedyKnapsackCostBenefit.precondition().to_string(),
    }
}

/// Decides whether one collection pass over a partition may be called exact.
///
/// 43.47 candidate C's premise is a junction structure. [`crate::separator::structure`] decides it;
/// this turns that decision into the same gated form as everything else, so a caller reading a
/// `Collection` cannot conclude exactness from the absence of an error.
pub fn separator_exact(report: &crate::separator::StructureReport) -> Applicability {
    let guarantee = Guarantee::SeparatorCompositionExact;
    if report.is_exact() {
        Applicability::Applies {
            guarantee,
            factor: None,
            evidence: format!(
                "acyclic agent graph with running intersection; widest separator {} variables",
                report.max_separator_width()
            ),
        }
    } else {
        Applicability::DoesNotApply {
            guarantee,
            failed_precondition: match (&report.cycle, &report.disconnected_variable) {
                (Some(cycle), _) => format!("the agent graph has a cycle through {cycle:?}"),
                (None, Some(variable)) => format!(
                    "variable {variable:?} is held by agents not connected through agents that also hold it"
                ),
                (None, None) => "the structure check did not pass".to_string(),
            },
        }
    }
}

/// One of 43.47's "worlds violating one assumption at a time".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Counterexample {
    /// The guarantee whose premise is broken.
    pub guarantee: Guarantee,
    /// The single assumption violated.
    pub assumption: &'static str,
    /// The construction, precise enough to rebuild from.
    pub construction: &'static str,
    /// What the system must do instead of quoting the guarantee.
    pub required_behaviour: &'static str,
}

/// The retained counterexample corpus.
///
/// Each entry has a test that builds it and asserts `required_behaviour`. 43.47 requires
/// counterexamples be kept as regressions; a corpus that is only described is not kept.
pub const COUNTEREXAMPLES: &[Counterexample] = &[
    Counterexample {
        guarantee: Guarantee::GreedyCardinality,
        assumption: "submodularity",
        construction:
            "two evidence items that are individually uninformative about the decision and jointly \
             decisive, plus one moderately informative item; the pair's marginal gain rises after \
             the first is taken",
        required_behaviour:
            "the exhaustive check reports a violation and no approximation factor is quoted",
    },
    Counterexample {
        guarantee: Guarantee::GreedyCardinality,
        assumption: "monotonicity",
        construction:
            "a partially informative item whose posterior moves the Bayes action away from the \
             full-evidence action, so regret reduction falls when it is added",
        required_behaviour:
            "the check reports a monotonicity violation, and greedy declines the non-positive step",
    },
    Counterexample {
        guarantee: Guarantee::SeparatorCompositionExact,
        assumption: "acyclic agent graph",
        construction:
            "three agents pairwise sharing a variable, forming a cycle with non-idempotent \
             sum-product aggregation",
        required_behaviour:
            "the structure check reports the cycle, composition is labelled approximate, and its \
             answer is allowed to differ from the centralised one",
    },
    Counterexample {
        guarantee: Guarantee::LensLawful,
        assumption: "put-get",
        construction:
            "a traversal focused by a predicate, given a replacement value that fails the \
             predicate; the subsequent get does not return what was put",
        required_behaviour:
            "the law check reports the failing law with the document that produced it, and the \
             optic is not reported as lawful",
    },
    Counterexample {
        guarantee: Guarantee::DecisionSufficiency,
        assumption: "identification",
        construction:
            "two models with equal posterior mass that prefer different actions, with a regret \
             gap wider than the tolerance",
        required_behaviour:
            "identification reports NonIdentified and the sufficiency query abstains rather than \
             returning the cheapest context",
    },
];
