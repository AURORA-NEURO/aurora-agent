//! The finite domain: a factor's potential abstracted to one sign per entry.
//!
//! The two interval domains have infinite concretisations, so every law checked against them is a
//! falsification search. This one is finite, and that is its point: `γ` of a support element depends
//! on the concrete table only through its sign pattern, and the universe in
//! [`SupportDomain::concrete_universe`] realises every sign pattern of the domain's length. A law
//! checked over that universe is therefore checked over every equivalence class `γ` can
//! distinguish, which makes it a proof rather than evidence — the distinction
//! [`crate::domain::EnumerableConcretisation::universe_is_complete`] exists to record.
//!
//! ## The concretisation
//!
//! ```text
//!     γ(⊥)           =  ∅
//!     γ(⟨s₁ … sₙ⟩)   =  { φ ∈ [0, ∞)ⁿ : sign(φₖ) ⊑ sₖ for every k }
//! ```
//!
//! with `sign(0) = Zero` and `sign(v) = Positive` for `v > 0`, and `Either` above both.
//!
//! ## What it is for
//!
//! It decides a precondition, not a bound. Two of them:
//!
//! - **Whether a removal bound can be anything but vacuous.** [`crate::ratio::RatioRange::of_removal`]
//!   sends a table with a zero entry to `[0, ∞)` and thence to a bound of exactly one. That bound
//!   is sound and constrains nothing, and this crate's position is that a group carrying it should
//!   be reported as such rather than counted as a win. [`removal_may_be_vacuous`] lets the caller
//!   find out *before* computing, from an abstraction that could have come from a schema assertion
//!   instead of from the table itself.
//! - **Whether the Gibbs analysis of [`crate::gibbs`] has its positivity hypothesis.** Single-site
//!   conditionals are only defined where the joint is positive, and a region whose factors are all
//!   certainly positive has them everywhere. [`certainly_positive`] is that check.
//!
//! It deliberately does *not* have a transformer into [`crate::domains::RatioInterval`]. Signs carry
//! no magnitudes, so the only sound image of an all-positive support is `[0, ∞]` — and a conversion
//! whose every output is `⊤` is a conversion that exists to look like progress.
//!
//! ## Widening
//!
//! `widen = join`, which is legal here because the lattice has no infinite ascending chains: a
//! strict increase either raises one entry from `Zero` or `Positive` to `Either`, which can happen
//! at most `n` times, or replaces the element by `⊤`. That a widening operator is a property of the
//! domain rather than a constant of the solver is exactly why it sits on the trait.

use crate::domain::{AbstractDomain, DomainId, EnumerableConcretisation, FactClass};
use serde::{Deserialize, Serialize};

/// The registered name prefix of [`SupportDomain`]; the domain's length follows it.
pub const SUPPORT_DOMAIN_PREFIX: &str = "factor_support";

/// The largest concrete universe [`SupportDomain::concrete_universe`] will materialise.
///
/// Beyond it the universe is truncated and [`SupportDomain::universe_is_complete`] reports `false`,
/// so a law checked against it stops being called a proof. Refusing to enumerate silently is the
/// same discipline [`crate::MAX_PERTURBATION_VERTICES`] applies to brute force.
pub const MAX_UNIVERSE_TABLES: usize = 4096;

/// The three distinct values the universe draws entries from.
///
/// One zero and two distinct positives, so that an abstraction which accidentally keyed on the
/// entry's value rather than on its sign fails a law instead of passing it.
pub const UNIVERSE_ALPHABET: [f64; 3] = [0.0, 0.5, 2.0];

/// What is known about one entry of a potential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntrySign {
    /// The entry is zero. An assignment this factor forbids.
    Zero,
    /// The entry is strictly positive. No magnitude is claimed.
    Positive,
    /// Either. The state a factor with no declared potential is in.
    Either,
}

impl EntrySign {
    fn of(value: f64) -> EntrySign {
        if value > 0.0 {
            EntrySign::Positive
        } else {
            EntrySign::Zero
        }
    }

    fn leq(self, other: EntrySign) -> bool {
        other == EntrySign::Either || self == other
    }

    fn join(self, other: EntrySign) -> EntrySign {
        if self == other {
            self
        } else {
            EntrySign::Either
        }
    }

    /// `None` when the two are disjoint, which sends the whole product element to `⊥`.
    fn meet(self, other: EntrySign) -> Option<EntrySign> {
        match (self, other) {
            (EntrySign::Either, value) | (value, EntrySign::Either) => Some(value),
            (left, right) if left == right => Some(left),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EntrySign::Zero => "0",
            EntrySign::Positive => "+",
            EntrySign::Either => "?",
        }
    }
}

/// A per-entry sign pattern, or `⊥`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "element")]
pub enum Support {
    Bottom,
    Pattern { signs: Vec<EntrySign> },
}

impl Support {
    /// `α` of a concrete potential: the exact sign of every entry.
    pub fn of_table(table: &[f64]) -> Support {
        Support::Pattern {
            signs: table.iter().copied().map(EntrySign::of).collect(),
        }
    }

    /// The abstraction of a factor that declares a signature and no potential.
    ///
    /// `⊤` of the right length, which is the correct and unhelpful answer: a `fiber-world/0.1`
    /// factor document has no field a potential could be written in, so nothing is known about any
    /// entry. [`crate::reference`] is where that fact is measured rather than asserted.
    pub fn unknown(entries: usize) -> Support {
        Support::Pattern {
            signs: vec![EntrySign::Either; entries],
        }
    }

    pub fn signs(&self) -> Option<&[EntrySign]> {
        match self {
            Support::Bottom => None,
            Support::Pattern { signs } => Some(signs),
        }
    }
}

/// Whether every concretisation of this element is strictly positive in every entry.
///
/// The positivity hypothesis of [`crate::gibbs`], discharged abstractly.
pub fn certainly_positive(element: &Support) -> bool {
    match element.signs() {
        None => false,
        Some(signs) => signs.iter().all(|sign| *sign == EntrySign::Positive),
    }
}

/// Whether some concretisation of this element sends the removal bound to the vacuous one.
///
/// `true` for an entry that is `Zero` and for one that is merely `Either`, because the abstraction
/// does not rule the zero out. A caller that gets `true` has learned that computing the removal
/// bound may be a waste of time, and — more usefully — that a `Bounded` group derived from it may
/// promise nothing.
pub fn removal_may_be_vacuous(element: &Support) -> bool {
    !certainly_positive(element)
}

/// The finite lattice of sign patterns over potentials of a fixed length.
///
/// The length is part of the [`DomainId`], so two lengths are two registry entries and cannot be
/// mixed through the registry. An element of the wrong length reaching an operation directly is a
/// caller who bypassed the registry; the operation degrades to `⊤` rather than panicking, which is
/// sound for this domain's `γ` and is never silently *tighter* than the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportDomain {
    entries: usize,
}

impl SupportDomain {
    pub fn of_length(entries: usize) -> Self {
        SupportDomain { entries }
    }

    pub fn entries(self) -> usize {
        self.entries
    }

    fn normalise(self, element: &Support) -> Support {
        match element.signs() {
            None => Support::Bottom,
            Some(signs) if signs.len() == self.entries => element.clone(),
            Some(_) => self.top(),
        }
    }
}

impl AbstractDomain for SupportDomain {
    type Element = Support;
    type Concrete = Vec<f64>;

    fn id(&self) -> DomainId {
        DomainId::new(format!("{SUPPORT_DOMAIN_PREFIX}/{}", self.entries))
    }

    fn abstracts(&self) -> FactClass {
        FactClass::FactorPotential
    }

    fn bottom(&self) -> Support {
        Support::Bottom
    }

    fn top(&self) -> Support {
        Support::unknown(self.entries)
    }

    fn leq(&self, left: &Support, right: &Support) -> bool {
        let (left, right) = (self.normalise(left), self.normalise(right));
        match (left.signs(), right.signs()) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(lower), Some(upper)) => lower
                .iter()
                .zip(upper)
                .all(|(low, high)| low.leq(*high)),
        }
    }

    fn join(&self, left: &Support, right: &Support) -> Support {
        let (left, right) = (self.normalise(left), self.normalise(right));
        match (left.signs(), right.signs()) {
            (None, _) => right.clone(),
            (_, None) => left.clone(),
            (Some(lower), Some(upper)) => Support::Pattern {
                signs: lower
                    .iter()
                    .zip(upper)
                    .map(|(low, high)| low.join(*high))
                    .collect(),
            },
        }
    }

    fn meet(&self, left: &Support, right: &Support) -> Support {
        let (left, right) = (self.normalise(left), self.normalise(right));
        let (Some(lower), Some(upper)) = (left.signs(), right.signs()) else {
            return Support::Bottom;
        };
        let mut signs = Vec::with_capacity(lower.len());
        for (low, high) in lower.iter().zip(upper) {
            match low.meet(*high) {
                Some(sign) => signs.push(sign),
                None => return Support::Bottom,
            }
        }
        Support::Pattern { signs }
    }

    fn widen(&self, previous: &Support, next: &Support) -> Support {
        self.join(previous, next)
    }

    fn concretises(&self, element: &Support, concrete: &Self::Concrete) -> bool {
        match self.normalise(element).signs() {
            None => false,
            Some(signs) => {
                signs.len() == concrete.len()
                    && signs
                        .iter()
                        .zip(concrete)
                        .all(|(sign, value)| value.is_finite() && EntrySign::of(*value).leq(*sign))
            }
        }
    }

    fn render(&self, element: &Support) -> String {
        match self.normalise(element).signs() {
            None => "⊥".to_string(),
            Some(signs) => signs.iter().map(|sign| sign.as_str()).collect(),
        }
    }
}

impl EnumerableConcretisation for SupportDomain {
    /// Every table of the domain's length over [`UNIVERSE_ALPHABET`].
    ///
    /// Complete for `γ` when it is not truncated: membership depends on the sign pattern alone and
    /// the alphabet realises both signs, so every pattern the domain can distinguish appears.
    fn concrete_universe(&self) -> Vec<Vec<f64>> {
        let alphabet = UNIVERSE_ALPHABET.len();
        let mut total = 1usize;
        for _ in 0..self.entries {
            total = total.saturating_mul(alphabet);
            if total > MAX_UNIVERSE_TABLES {
                total = MAX_UNIVERSE_TABLES;
                break;
            }
        }
        (0..total)
            .map(|index| {
                let mut remaining = index;
                let mut table = vec![0.0; self.entries];
                for slot in table.iter_mut().rev() {
                    *slot = UNIVERSE_ALPHABET[remaining % alphabet];
                    remaining /= alphabet;
                }
                table
            })
            .collect()
    }

    fn universe_is_complete(&self) -> bool {
        UNIVERSE_ALPHABET
            .len()
            .checked_pow(self.entries as u32)
            .is_some_and(|total| total <= MAX_UNIVERSE_TABLES)
    }
}
