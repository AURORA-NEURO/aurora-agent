//! Checking submodularity by exhaustion, rather than asserting it.
//!
//! Blueprint 43.14's normative decision is conditional — "use greedy or lazy-greedy evidence
//! selection **only when** the utility exhibits validated diminishing returns" — and its
//! non-negotiable invariants say "approximation guarantees are reported only under verified
//! assumptions". This module is the verification. Nothing in [`crate::greedy`](mod@crate::greedy) may quote a factor
//! that this module has not returned a clean report for.
//!
//! ## The two characterisations, and why both are computed
//!
//! The definition 43.14 states is the global one: for all `A ⊆ B` and `e ∉ B`,
//!
//! ```text
//! F(A ∪ {e}) − F(A)  ≥  F(B ∪ {e}) − F(B)
//! ```
//!
//! There is a standard equivalent local form, quantified over single-element extensions: for all
//! `A` and distinct `e, x ∉ A`,
//!
//! ```text
//! F(A ∪ {e}) − F(A)  ≥  F(A ∪ {x, e}) − F(A ∪ {x})
//! ```
//!
//! The local form is cheaper — `O(n² 2^n)` against `O(n 3^n)` — and it is the one a production
//! checker would use. This module computes **both**, on every instance, and the suite asserts they
//! agree. That is not redundancy: the equivalence is a textbook fact about the mathematical
//! object, and running only the cheap form would mean trusting that this code implements the fact
//! rather than checking that it does. A checker nobody checked is the same class of thing as a
//! guarantee nobody checked.
//!
//! ## Tolerance
//!
//! Comparisons carry an absolute tolerance because the objectives that matter are computed through
//! posterior normalisation, and a violation of `1e-16` is a floating-point artefact rather than
//! complementarity. The tolerance is a parameter, not a constant, and every report states the
//! value it was run at — a check whose sensitivity is invisible can be tuned until it passes.
//!
//! ## The caps
//!
//! [`MAX_GLOBAL_GROUND`] and [`MAX_LOCAL_GROUND`] refuse rather than sample, for the reason
//! `bioprism-influence` gives about its brute-force cap: a soundness check that degrades to a
//! sample when the space grows is weakest exactly where the guarantee is most interesting.

use crate::error::EpistemicError;
use crate::objective::{SetFunction, Tabulated};
use serde::{Deserialize, Serialize};

/// Largest ground set the `O(n · 3^n)` global check will enumerate.
pub const MAX_GLOBAL_GROUND: usize = 12;

/// Largest ground set the `O(n² · 2^n)` local check will enumerate.
pub const MAX_LOCAL_GROUND: usize = 16;

/// Default absolute tolerance for diminishing-returns comparisons.
pub const DEFAULT_TOLERANCE: f64 = 1e-9;

/// A concrete failure of diminishing returns, with the sets that produced it.
///
/// Carries the witness rather than a count, for the reason `AGENTS.md` gives about witnesses in
/// general: a reader has to be able to check it by hand, and "3 violations" cannot be checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Violation {
    /// The smaller set, as element indices.
    pub smaller: Vec<usize>,
    /// The larger set, a superset of `smaller`.
    pub larger: Vec<usize>,
    /// The element whose marginal gain grew.
    pub element: usize,
    /// `F(smaller ∪ {e}) − F(smaller)`.
    pub gain_on_smaller: f64,
    /// `F(larger ∪ {e}) − F(larger)`. Larger than `gain_on_smaller` by `excess`.
    pub gain_on_larger: f64,
    /// How badly diminishing returns failed: `gain_on_larger − gain_on_smaller`.
    pub excess: f64,
}

/// A failure of monotonicity: adding an element lowered the value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonotonicityViolation {
    pub subset: Vec<usize>,
    pub element: usize,
    pub before: f64,
    pub after: f64,
    /// `before − after`, the amount the value fell.
    pub drop: f64,
}

/// The result of checking a set function's structural conditions.
///
/// Every field is a *measured* outcome. `submodular_global: None` means the exhaustive search
/// found no violation at the stated tolerance over the stated ground set — not that submodularity
/// was proved for the family the instance was drawn from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmodularityReport {
    pub function: String,
    pub ground: usize,
    pub tolerance: f64,
    /// `F(∅) = 0` within tolerance.
    pub normalised: bool,
    /// The first monotonicity violation found, in enumeration order. `None` means none was found.
    pub monotone_violation: Option<MonotonicityViolation>,
    /// The worst diminishing-returns violation found by the global `A ⊆ B` search.
    pub global_violation: Option<Violation>,
    /// The worst violation found by the local single-element-extension search.
    pub local_violation: Option<Violation>,
    /// Triples examined by the global search.
    pub global_triples: usize,
    /// Triples examined by the local search.
    pub local_triples: usize,
}

impl SubmodularityReport {
    /// Whether the function passed every condition the `1 − 1/e` factor needs.
    ///
    /// All three: normalised, monotone, submodular. This is the only predicate
    /// [`crate::theorem`] consults, and [`crate::greedy`](mod@crate::greedy) reaches the factor only through it.
    pub fn monotone_submodular(&self) -> bool {
        self.normalised
            && self.monotone_violation.is_none()
            && self.global_violation.is_none()
            && self.local_violation.is_none()
    }

    /// Whether the two characterisations reached the same verdict.
    ///
    /// They must, and the suite asserts it. A `false` here is a bug in this module, not a property
    /// of the function under test.
    pub fn characterisations_agree(&self) -> bool {
        self.global_violation.is_some() == self.local_violation.is_some()
    }

    /// One sentence naming the reason no factor may be quoted, when none may.
    pub fn why_no_guarantee(&self) -> Option<String> {
        if self.monotone_submodular() {
            return None;
        }
        let mut reasons = Vec::new();
        if !self.normalised {
            reasons.push("F(empty) is not zero".to_string());
        }
        if let Some(v) = &self.monotone_violation {
            reasons.push(format!(
                "adding element {} to {:?} lowered the value by {}",
                v.element, v.subset, v.drop
            ));
        }
        if let Some(v) = &self.global_violation {
            reasons.push(format!(
                "element {} gained {} on {:?} but {} on the superset {:?}",
                v.element, v.gain_on_smaller, v.smaller, v.gain_on_larger, v.larger
            ));
        }
        Some(reasons.join("; "))
    }
}

fn bits(mask: usize, ground: usize) -> Vec<usize> {
    (0..ground).filter(|i| (mask >> i) & 1 == 1).collect()
}

/// Checks normalisation, monotonicity and submodularity by exhaustion, at [`DEFAULT_TOLERANCE`].
pub fn check<F: SetFunction + ?Sized>(function: &F) -> Result<SubmodularityReport, EpistemicError> {
    check_with_tolerance(function, DEFAULT_TOLERANCE)
}

/// Checks at a caller-chosen tolerance.
pub fn check_with_tolerance<F: SetFunction + ?Sized>(
    function: &F,
    tolerance: f64,
) -> Result<SubmodularityReport, EpistemicError> {
    let ground = function.ground_size();
    if ground > MAX_GLOBAL_GROUND {
        return Err(EpistemicError::ExhaustiveCapExceeded {
            ground,
            needed: 3u64.saturating_pow(ground.min(40) as u32),
            cap: 3u64.saturating_pow(MAX_GLOBAL_GROUND as u32),
        });
    }
    let table = Tabulated::of(function)?;
    let total = 1usize << ground;

    let normalised = table.at(0).abs() <= tolerance;

    let mut monotone_violation: Option<MonotonicityViolation> = None;
    for mask in 0..total {
        for element in 0..ground {
            if (mask >> element) & 1 == 1 {
                continue;
            }
            let gain = table.gain(mask, element);
            if gain < -tolerance {
                let candidate = MonotonicityViolation {
                    subset: bits(mask, ground),
                    element,
                    before: table.at(mask),
                    after: table.at(mask | (1 << element)),
                    drop: -gain,
                };
                if monotone_violation
                    .as_ref()
                    .is_none_or(|worst| candidate.drop > worst.drop)
                {
                    monotone_violation = Some(candidate);
                }
            }
        }
    }

    let mut global_violation: Option<Violation> = None;
    let mut global_triples = 0usize;
    for element in 0..ground {
        let bit = 1usize << element;
        for larger in 0..total {
            if larger & bit != 0 {
                continue;
            }
            let gain_larger = table.gain(larger, element);
            let mut smaller = larger;
            loop {
                global_triples += 1;
                let gain_smaller = table.gain(smaller, element);
                let excess = gain_larger - gain_smaller;
                if excess > tolerance
                    && global_violation
                        .as_ref()
                        .is_none_or(|worst| excess > worst.excess)
                {
                    global_violation = Some(Violation {
                        smaller: bits(smaller, ground),
                        larger: bits(larger, ground),
                        element,
                        gain_on_smaller: gain_smaller,
                        gain_on_larger: gain_larger,
                        excess,
                    });
                }
                if smaller == 0 {
                    break;
                }
                smaller = (smaller - 1) & larger;
            }
        }
    }

    let mut local_violation: Option<Violation> = None;
    let mut local_triples = 0usize;
    if ground <= MAX_LOCAL_GROUND {
        for element in 0..ground {
            for other in 0..ground {
                if other == element {
                    continue;
                }
                let taken = (1usize << element) | (1usize << other);
                for base in 0..total {
                    if base & taken != 0 {
                        continue;
                    }
                    local_triples += 1;
                    let gain_base = table.gain(base, element);
                    let gain_extended = table.gain(base | (1 << other), element);
                    let excess = gain_extended - gain_base;
                    if excess > tolerance
                        && local_violation
                            .as_ref()
                            .is_none_or(|worst| excess > worst.excess)
                    {
                        local_violation = Some(Violation {
                            smaller: bits(base, ground),
                            larger: bits(base | (1 << other), ground),
                            element,
                            gain_on_smaller: gain_base,
                            gain_on_larger: gain_extended,
                            excess,
                        });
                    }
                }
            }
        }
    }

    Ok(SubmodularityReport {
        function: function.name(),
        ground,
        tolerance,
        normalised,
        monotone_violation,
        global_violation,
        local_violation,
        global_triples,
        local_triples,
    })
}
